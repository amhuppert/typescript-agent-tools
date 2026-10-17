//! Shared `getExportsOfModuleWorker` traversal. Loading and checker requests
//! supply their own symbol operations and keep their own native caches. A
//! suspended loader alias or interrupted checker acquisition exits unchanged;
//! a checker adapter retains recovered acquisitions until the worker finishes.
use super::NativePropertyKey;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

pub(in crate::project) type Table<S> = HashMap<NativePropertyKey, S>;

pub(in crate::project) struct Module<S, I> {
    pub(in crate::project) identity: I,
    pub(in crate::project) exports: Option<Table<S>>,
}

#[derive(Clone)]
pub(in crate::project) struct Star<D> {
    pub(in crate::project) declaration: D,
    pub(in crate::project) type_only: bool,
}

pub(in crate::project) enum Resolved<S> {
    Absent,
    Unknown,
    Symbol(S),
}

pub(in crate::project) struct Exports<S, D> {
    pub(in crate::project) symbols: Table<S>,
    pub(in crate::project) type_only: HashMap<NativePropertyKey, D>,
}

pub(in crate::project) trait Symbols {
    type Symbol: Copy;
    type Identity: Copy + Eq + Hash;
    type Declaration: Clone;
    type Error;

    fn control(&self) -> Option<&crate::resources::Control>;
    fn module(
        &mut self,
        symbol: Self::Symbol,
    ) -> Result<Module<Self::Symbol, Self::Identity>, Self::Error>;
    fn resolve(&mut self, symbol: Self::Symbol) -> Result<Resolved<Self::Symbol>, Self::Error>;
    fn merged(&mut self, symbol: Self::Symbol) -> Result<Self::Symbol, Self::Error>;
    fn name(&mut self, symbol: Self::Symbol) -> Result<NativePropertyKey, Self::Error>;
    fn stars(&mut self, symbol: Self::Symbol) -> Result<Vec<Star<Self::Declaration>>, Self::Error>;
    fn star_module(&mut self, star: &Self::Declaration) -> Result<Self::Symbol, Self::Error>;
    fn supplemental_type(&mut self, symbol: Self::Symbol) -> Result<bool, Self::Error>;
}

/// `getExportsOfModuleWorker` and `extendExportSymbols`: complete raw-table
/// visits, source-order export stars, native identity cycles, and type-only
/// overrides. Collision diagnostics use only syntax/text; their two alias
/// resolutions remain real acquisitions even though the first export wins.
pub(in crate::project) fn exports<O: Symbols>(
    operations: &mut O,
    original: Module<O::Symbol, O::Identity>,
) -> Result<Exports<O::Symbol, O::Declaration>, O::Error> {
    let own = original.exports.clone().unwrap_or_default();
    let export_equals = own
        .get(&NativePropertyKey::fixed(
            b"export=".as_slice(),
            operations.control(),
        ))
        .copied();
    let has_original = match export_equals {
        Some(symbol) => !matches!(operations.resolve(symbol)?, Resolved::Absent),
        None => false,
    };
    // resolveExternalModuleSymbol repeats resolveSymbolEx, then observes one
    // merge link. An unknown result is nonnil but has no raw exports.
    let root = match export_equals {
        Some(symbol) => match operations.resolve(symbol)? {
            Resolved::Symbol(symbol) => {
                let symbol = operations.merged(symbol)?;
                Some(operations.module(symbol)?)
            }
            Resolved::Unknown => None,
            Resolved::Absent => Some(original),
        },
        None => Some(original),
    };
    enum Visit<S, I, D> {
        Enter {
            module: Option<Module<S, I>>,
            star: Option<Star<D>>,
            type_only: bool,
        },
        Stars {
            symbols: Table<S>,
            nested: Table<S>,
            stars: std::vec::IntoIter<Star<D>>,
            star: Option<Star<D>>,
            type_only: bool,
        },
        Extend {
            symbols: Table<S>,
            nested: Table<S>,
            stars: std::vec::IntoIter<Star<D>>,
            star: Option<Star<D>>,
            type_only: bool,
        },
    }
    let mut stack = vec![Visit::Enter {
        module: root,
        star: None,
        type_only: false,
    }];
    let mut visited = HashSet::new();
    let mut non_type_only = HashSet::new();
    let mut type_only_names = HashMap::new();
    let mut returned = Table::new();
    while let Some(work) = stack.pop() {
        match work {
            Visit::Enter {
                module,
                star,
                type_only,
            } => {
                let Some(module) = module else {
                    returned.clear();
                    continue;
                };
                let Some(symbols) = module.exports else {
                    returned.clear();
                    continue;
                };
                // Native does this before the visited test: a later ordinary
                // star overrides an earlier type-only visit of the same module.
                if !type_only {
                    non_type_only.extend(symbols.keys().cloned());
                }
                if !visited.insert(module.identity) {
                    returned.clear();
                    continue;
                }
                let stars = match symbols.get(&NativePropertyKey::fixed(
                    b"\xfeexport".as_slice(),
                    operations.control(),
                )) {
                    Some(&symbol) => operations.stars(symbol)?,
                    None => Vec::new(),
                };
                stack.push(Visit::Stars {
                    symbols,
                    nested: Table::new(),
                    stars: stars.into_iter(),
                    star,
                    type_only,
                });
            }
            Visit::Stars {
                mut symbols,
                nested,
                mut stars,
                star,
                type_only,
            } => {
                if let Some(next) = stars.next() {
                    let symbol = operations.star_module(&next.declaration)?;
                    let module = operations.module(symbol)?;
                    let nested_type_only = type_only || next.type_only;
                    stack.push(Visit::Extend {
                        symbols,
                        nested,
                        stars,
                        star,
                        type_only,
                    });
                    stack.push(Visit::Enter {
                        module: Some(module),
                        star: Some(next),
                        type_only: nested_type_only,
                    });
                } else {
                    for (name, symbol) in nested {
                        if name.fixed_bytes() != Some(b"default") {
                            symbols.entry(name).or_insert(symbol);
                        }
                    }
                    if let Some(star) = star.filter(|star| star.type_only) {
                        for name in symbols.keys() {
                            type_only_names.insert(name.clone(), star.declaration.clone());
                        }
                    }
                    returned = symbols;
                }
            }
            Visit::Extend {
                symbols,
                mut nested,
                stars,
                star,
                type_only,
            } => {
                for (name, source) in std::mem::take(&mut returned) {
                    if name.fixed_bytes() == Some(b"default") {
                        continue;
                    }
                    if let Some(&target) = nested.get(&name) {
                        operations.resolve(target)?;
                        operations.resolve(source)?;
                    } else {
                        nested.insert(name, source);
                    }
                }
                stack.push(Visit::Stars {
                    symbols,
                    nested,
                    stars,
                    star,
                    type_only,
                });
            }
        }
    }
    if has_original && own.len() > 1 {
        for symbol in own.into_values() {
            let name = operations.name(symbol)?;
            if matches!(name.fixed_bytes(), Some(b"export=" | b"\xfeexport")) {
                continue;
            }
            if operations.supplemental_type(symbol)? {
                returned.entry(name).or_insert(symbol);
            }
        }
    }
    for name in non_type_only {
        type_only_names.remove(&name);
    }
    Ok(Exports {
        symbols: returned,
        type_only: type_only_names,
    })
}
