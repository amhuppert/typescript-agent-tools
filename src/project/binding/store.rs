//! The tables, symbols, and modules that linking builds and merges.
//!
//! A table entry holds one symbol per meaning plus an optional alias; symbols
//! that merge both ways form one identity, which readers reach with `find`.
//! The methods here are the only writers, and every write marks the store
//! changed: while the global table and module augmentations still merge, a
//! resolution made before a write is discarded before it is read again
//! (`Linker::force`), whichever write it was. Once merging ends the store is
//! frozen, and a write is a bug.

use super::super::sources::FileId;
use super::collect::InterfaceMembers;
use super::ids::{AliasId, ModuleId, RequestId, SymbolId, TableId, TypeParameterListId};
use super::index::Site;
use super::targets::Slots;
use smallvec::SmallVec;
use std::collections::HashMap;

/// The global table: script declarations, `declare global` blocks, and UMD
/// globals.
pub(super) const GLOBAL: TableId = TableId::new_const(0);

/// One name in a table: at most one declared symbol per meaning, plus an
/// optional import or export alias. A stored symbol may since have merged
/// into another, so readers canonicalize it with `find`.
#[derive(Debug, Default)]
pub(super) struct Entry {
    pub(super) slots: Slots,
    pub(super) alias: Option<AliasRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct AliasRef {
    pub(super) file: FileId,
    pub(super) id: AliasId,
}

#[derive(Debug, Default)]
pub(super) struct Symbol {
    pub(super) flags: u32,
    /// The symbol's own declarations: what it declares, and what merges gave
    /// it, one way or both. Compatibility checks read these.
    pub(super) sites: Vec<Site>,
    /// Alias declarations among the symbol's own, as TypeScript's merged
    /// symbol keeps an alias a checker merge met beside declarations. A
    /// group lists what they name as well (`Linker::projected_sites`), but
    /// nothing about those targets is merged into this symbol.
    pub(super) aliases: SmallVec<[AliasRef; 1]>,
    /// Type parameter lists that declare this type parameter.
    pub(super) lists: Vec<(FileId, TypeParameterListId)>,
    pub(super) exports: Option<TableId>,
    pub(super) members: Option<TableId>,
    pub(super) module: Option<ModuleId>,
    pub(super) enum_members: Vec<String>,
    /// The tables with an entry that holds this symbol in some meaning (see
    /// `hold`), whose entries gain the meanings a module augmentation merges
    /// into it.
    pub(super) homes: SmallVec<[TableId; 2]>,
}

/// An `export *` of a module: the request it names, which the module it
/// reaches is read through (`Linker::module_stars`), so that it follows the
/// request when an augmentation gives the specifier a module of its own,
/// and whether it is type-only.
#[derive(Debug, Clone, Copy)]
pub(super) struct StarRequest {
    pub(super) file: FileId,
    pub(super) request: RequestId,
    pub(super) type_only: bool,
}

#[derive(Debug)]
pub(super) struct Module {
    pub(super) exports: TableId,
    pub(super) stars: Vec<StarRequest>,
    pub(super) symbol: SymbolId,
    /// A shorthand ambient module (`declare module "x";`), as the first
    /// value declaration of its symbol in merge order decides
    /// (`isShorthandAmbientModuleSymbol`).
    pub(super) shorthand: bool,
    /// A declaration file or ambient module, which may describe CommonJS and
    /// so has a synthetic default export.
    pub(super) declaration: bool,
    /// Separate symbols a module augmentation merged into the module symbol
    /// in meanings it lacks of its own. TypeScript's merged module symbol has
    /// those meanings, so its module object, and every export of a shorthand
    /// module, has them too.
    pub(super) merged: Slots,
    /// The JSON source file whose module this is (`json_export`).
    pub(super) json: Option<FileId>,
    /// The pattern ambient module an augmented specifier's module extends one
    /// way, until it absorbs the pattern's declarations: its exports and
    /// stars complete this module's in the meantime (`Linker::probe_export`).
    pub(super) pattern: Option<ModuleId>,
    /// A checker merge has merged into the module's symbol, as TypeScript's
    /// `getMergedSymbol` then answers a merged symbol for it rather than the
    /// binder's: an augmentation of a specifier that this pattern ambient
    /// module matches then merges into it (`Linker::exact_module`).
    pub(super) merged_into: bool,
}

/// How a checker merge relates its symbols (`mergeSymbol`'s
/// `unidirectional`): both directions make one identity; one way, the source
/// keeps its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Direction {
    Both,
    OneWay,
}

/// What a module request denotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RequestTarget {
    Module(ModuleId),
    Script,
    /// A tolerated failure: names imported through the request are
    /// unresolved, and an augmentation of it binds nothing.
    Ignored,
}

/// `Store::unite` was asked to make two modules one, which is unsupported.
pub(super) struct TwoModules(pub(super) SymbolId);

#[derive(Debug)]
pub(super) struct Store {
    tables: Vec<HashMap<String, Entry>>,
    symbols: Vec<Symbol>,
    parents: Vec<SymbolId>,
    modules: Vec<Module>,
    /// Parallel to each file's requests.
    requests: Vec<Vec<RequestTarget>>,
    /// By canonical symbol: the members its interface declarations declare,
    /// recorded only for the apparent types of JSON values
    /// (`collect::InterfaceMembers`), so other symbols carry nothing.
    interface_members: HashMap<SymbolId, InterfaceMembers>,
    changed: bool,
    frozen: bool,
}

impl Store {
    pub(super) fn new() -> Self {
        Self {
            tables: vec![HashMap::new()],
            symbols: Vec::new(),
            parents: Vec::new(),
            modules: Vec::new(),
            requests: Vec::new(),
            interface_members: HashMap::new(),
            changed: false,
            frozen: false,
        }
    }

    /// The members `symbol`'s interface declarations declare, where they are
    /// recorded (`add_interface_members`).
    pub(super) fn interface_members(&self, symbol: SymbolId) -> Option<&InterfaceMembers> {
        self.interface_members.get(&self.find(symbol))
    }

    // Reads

    pub(super) fn table(&self, table: TableId) -> &HashMap<String, Entry> {
        &self.tables[table.index()]
    }

    pub(super) fn entry(&self, table: TableId, name: &str) -> Option<&Entry> {
        self.tables[table.index()].get(name)
    }

    /// The canonical symbol after merges both ways.
    pub(super) fn find(&self, mut id: SymbolId) -> SymbolId {
        while self.parents[id.index()] != id {
            id = self.parents[id.index()];
        }
        id
    }

    /// The canonical symbol's data.
    pub(super) fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[self.find(id).index()]
    }

    /// Every canonical symbol, in creation order.
    pub(super) fn canonical_symbols(&self) -> impl Iterator<Item = (SymbolId, &Symbol)> {
        self.symbols
            .iter()
            .enumerate()
            .filter(|&(index, _)| self.parents[index].index() == index)
            .map(|(index, symbol)| (SymbolId::new(index), symbol))
    }

    pub(super) fn module(&self, module: ModuleId) -> &Module {
        &self.modules[module.index()]
    }

    pub(super) fn request(&self, file: FileId, request: RequestId) -> RequestTarget {
        self.requests[file.index()][request.index()]
    }

    /// Whether anything was written since the last call.
    pub(super) fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// End merging: any later write is a bug.
    pub(super) fn freeze(&mut self) {
        self.frozen = true;
    }

    /// Every write goes through here.
    fn write(&mut self) {
        debug_assert!(!self.frozen, "the store is written after merging ended");
        self.changed = true;
    }

    // Writes

    pub(super) fn new_table(&mut self) -> TableId {
        self.write();
        self.tables.push(HashMap::new());
        TableId::new(self.tables.len() - 1)
    }

    pub(super) fn new_symbol(&mut self, flags: u32, site: Option<Site>) -> SymbolId {
        self.write();
        let id = SymbolId::new(self.symbols.len());
        self.symbols.push(Symbol {
            flags,
            sites: site.into_iter().collect(),
            ..Symbol::default()
        });
        self.parents.push(id);
        id
    }

    /// A module with its export table and its symbol, whose exports are that
    /// table: namespace declarations merged into the module export through
    /// it.
    pub(super) fn new_module(
        &mut self,
        flags: u32,
        site: Option<Site>,
        shorthand: bool,
        declaration: bool,
    ) -> ModuleId {
        let exports = self.new_table();
        let symbol = self.new_symbol(flags, site);
        let id = ModuleId::new(self.modules.len());
        self.symbols[symbol.index()].module = Some(id);
        self.symbols[symbol.index()].exports = Some(exports);
        self.modules.push(Module {
            exports,
            stars: Vec::new(),
            symbol,
            shorthand,
            declaration,
            merged: Slots::default(),
            json: None,
            pattern: None,
            merged_into: false,
        });
        id
    }

    pub(super) fn set_json(&mut self, module: ModuleId, file: FileId) {
        self.write();
        self.modules[module.index()].json = Some(file);
    }

    /// Another declaration of a module in the file that first declares it,
    /// as TypeScript's binder adds it to the module's symbol. The first
    /// declaration stays the one that decides whether the module is
    /// shorthand.
    pub(super) fn redeclare_module(&mut self, module: ModuleId, site: Site) {
        self.write();
        let symbol = self.modules[module.index()].symbol;
        self.symbols[symbol.index()].sites.push(site);
    }

    /// A declaration merged into `symbol` by the binder: its flags and site.
    pub(super) fn declare_into(&mut self, symbol: SymbolId, flags: u32, site: Site) {
        self.write();
        let symbol = self.find(symbol);
        let data = &mut self.symbols[symbol.index()];
        data.flags |= flags;
        data.sites.push(site);
    }

    pub(super) fn add_list(&mut self, symbol: SymbolId, list: (FileId, TypeParameterListId)) {
        self.write();
        let symbol = self.find(symbol);
        self.symbols[symbol.index()].lists.push(list);
    }

    pub(super) fn add_enum_members(&mut self, symbol: SymbolId, members: &[String]) {
        self.write();
        let symbol = self.find(symbol);
        self.symbols[symbol.index()]
            .enum_members
            .extend(members.iter().cloned());
    }

    pub(super) fn add_interface_members(&mut self, symbol: SymbolId, members: &InterfaceMembers) {
        self.write();
        let symbol = self.find(symbol);
        let own = self.interface_members.entry(symbol).or_default();
        own.names.extend(members.names.iter().cloned());
        own.inherits |= members.inherits;
    }

    /// An alias declaration a checker merge met beside `symbol`'s
    /// declarations (`Symbol::aliases`).
    pub(super) fn add_alias(&mut self, symbol: SymbolId, alias: AliasRef) {
        self.write();
        let symbol = self.find(symbol);
        let aliases = &mut self.symbols[symbol.index()].aliases;
        if !aliases.contains(&alias) {
            aliases.push(alias);
        }
    }

    /// The member table of `symbol`, created on first use.
    pub(super) fn member_table(&mut self, symbol: SymbolId) -> TableId {
        let symbol = self.find(symbol);
        if let Some(table) = self.symbols[symbol.index()].members {
            return table;
        }
        let table = self.new_table();
        self.symbols[symbol.index()].members = Some(table);
        table
    }

    /// The export table of `symbol`, created on first use.
    pub(super) fn export_table(&mut self, symbol: SymbolId) -> TableId {
        let symbol = self.find(symbol);
        if let Some(table) = self.symbols[symbol.index()].exports {
            return table;
        }
        let table = self.new_table();
        self.symbols[symbol.index()].exports = Some(table);
        table
    }

    /// Hold `symbol` in `table`'s entry for `name` in one meaning: every write
    /// of a table's symbols goes through here. It records the table among
    /// the symbol's homes, whose entries `Linker::propagate` extends with the
    /// meanings later merges add.
    pub(super) fn hold(&mut self, table: TableId, name: &str, meaning: u8, symbol: SymbolId) {
        self.write();
        match self.tables[table.index()].get_mut(name) {
            Some(entry) => entry.slots.set(meaning, symbol),
            None => {
                let mut entry = Entry::default();
                entry.slots.set(meaning, symbol);
                self.tables[table.index()].insert(name.to_string(), entry);
            }
        }
        let symbol = self.find(symbol);
        let homes = &mut self.symbols[symbol.index()].homes;
        if !homes.contains(&table) {
            homes.push(table);
        }
    }

    /// Set or clear the alias of `table`'s entry for `name`.
    pub(super) fn set_alias(&mut self, table: TableId, name: &str, alias: Option<AliasRef>) {
        self.write();
        match (self.tables[table.index()].get_mut(name), alias) {
            (Some(entry), _) => entry.alias = alias,
            (None, None) => {}
            (None, Some(_)) => {
                let entry = Entry {
                    slots: Slots::default(),
                    alias,
                };
                self.tables[table.index()].insert(name.to_string(), entry);
            }
        }
    }

    /// Take every entry of `table`, which the binder merges into another.
    pub(super) fn drain(&mut self, table: TableId) -> Vec<(String, Entry)> {
        self.write();
        self.tables[table.index()].drain().collect()
    }

    /// Make `other` part of `target`'s identity, the union every merge of
    /// both directions makes (TypeScript's `recordMergedSymbol`). A module
    /// stays the canonical symbol, so its exports remain the module's own
    /// table. Returns the canonical pair, or nothing when they are one.
    pub(super) fn unite(
        &mut self,
        target: SymbolId,
        other: SymbolId,
    ) -> Result<Option<(SymbolId, SymbolId)>, TwoModules> {
        let (mut target, mut other) = (self.find(target), self.find(other));
        if target == other {
            return Ok(None);
        }
        let module = |symbol: SymbolId| self.symbols[symbol.index()].module.is_some();
        match (module(target), module(other)) {
            (false, true) => std::mem::swap(&mut target, &mut other),
            (true, true) => return Err(TwoModules(other)),
            _ => {}
        }
        self.write();
        self.parents[other.index()] = target;
        Ok(Some((target, other)))
    }

    /// What a merge moves (both directions) or copies (one way) from
    /// `source` to `target`: flags, declaration sites and alias declarations,
    /// type parameter lists, enum and interface members, and, both ways, the tables that
    /// hold it. Its member and export tables, returned, merge by the caller's
    /// rule: the binder's for declarations of one table
    /// (`Linker::merge_symbols`), the checker's otherwise (`run_merges`).
    pub(super) fn transfer(
        &mut self,
        target: SymbolId,
        source: SymbolId,
        direction: Direction,
    ) -> (Option<TableId>, Option<TableId>) {
        self.write();
        let moved = match direction {
            Direction::Both => std::mem::take(&mut self.symbols[source.index()]),
            Direction::OneWay => {
                let from = &self.symbols[source.index()];
                Symbol {
                    flags: from.flags,
                    sites: from.sites.clone(),
                    aliases: from.aliases.clone(),
                    lists: from.lists.clone(),
                    exports: from.exports,
                    members: from.members,
                    enum_members: from.enum_members.clone(),
                    ..Symbol::default()
                }
            }
        };
        let members = match direction {
            Direction::Both => self.interface_members.remove(&source),
            Direction::OneWay => self.interface_members.get(&source).cloned(),
        };
        if let Some(members) = members {
            let to = self.interface_members.entry(target).or_default();
            to.names.extend(members.names);
            to.inherits |= members.inherits;
        }
        let to = &mut self.symbols[target.index()];
        to.flags |= moved.flags;
        to.sites.extend(moved.sites);
        for alias in moved.aliases {
            if !to.aliases.contains(&alias) {
                to.aliases.push(alias);
            }
        }
        to.lists.extend(moved.lists);
        to.enum_members.extend(moved.enum_members);
        for table in moved.homes {
            if !to.homes.contains(&table) {
                to.homes.push(table);
            }
        }
        (moved.members, moved.exports)
    }

    pub(super) fn add_stars(&mut self, module: ModuleId, stars: &[StarRequest]) {
        self.write();
        self.modules[module.index()].stars.extend_from_slice(stars);
    }

    /// Record that a checker merge merged into `module`'s symbol
    /// (`Module::merged_into`).
    pub(super) fn set_merged_into(&mut self, module: ModuleId) {
        if !self.modules[module.index()].merged_into {
            self.write();
            self.modules[module.index()].merged_into = true;
        }
    }

    /// A meaning a merge gave `module`'s symbol through a separate symbol.
    pub(super) fn set_merged(&mut self, module: ModuleId, meaning: u8, symbol: SymbolId) {
        self.write();
        self.modules[module.index()].merged.set(meaning, symbol);
    }

    pub(super) fn set_pattern(&mut self, module: ModuleId, pattern: Option<ModuleId>) {
        self.write();
        self.modules[module.index()].pattern = pattern;
    }

    /// The targets of the next file's requests.
    pub(super) fn push_requests(&mut self, targets: Vec<RequestTarget>) {
        self.write();
        self.requests.push(targets);
    }

    pub(super) fn set_request(&mut self, file: FileId, request: RequestId, target: RequestTarget) {
        self.write();
        self.requests[file.index()][request.index()] = target;
    }
}
