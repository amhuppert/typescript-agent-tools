//! Directed native reference relationships and their complete candidate ledger.
//! Symbol factories remain checker operations; output never supplies identity.

use super::evaluation::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use super::model::DemandError;
use super::reference_imports::{ComingFrom, Continuation, ExportInfo};
use super::selection::references::{ReferenceSelected, ReferenceSelection};
use super::source::{
    DeclarationIdentity, ExpressionRoot, ReferenceExportKind, ReferenceMeaning, ReferenceSite,
};
use crate::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason, SemanticReason};
use crate::project::binding::NativeSymbolIdentity;
use crate::project::binding::ReferenceToken;
use crate::project::sources::FileId;
use crate::project::{Project, references};
use crate::references::{
    Location, NamedLocation, Occurrence, ReferenceAnswer, ReferenceBaseline, ReferenceCause,
    ReferenceCoverage, ReferenceFilters, ReferenceGap, ReferenceGroup, ReferenceOperation,
    ReferenceOutcome, ReferenceRequest, ReferenceResult, ReferenceSelection as SelectionDocument,
    ReferenceSelectionFailure, UnsearchedMembers,
};
use crate::resources::FixedMap;
use crate::syntax_kind::SyntaxKind;
mod ledger;
use crate::references::metadata::copy_location;
use ledger::Ledger;

pub(super) fn search(
    project: &Project,
    request: &ReferenceRequest,
    evaluator: &mut Evaluator<'_, '_>,
) -> Result<ReferenceAnswer, AnalysisError> {
    let selected = super::selection::references::select(project, evaluator, &request.query)?;
    let outcome = match selected {
        ReferenceSelection::One(ReferenceSelected::Lexical(query)) => {
            return references::lexical(project, query, request.filters)
                .map(ReferenceAnswer::Selected);
        }
        ReferenceSelection::One(ReferenceSelected::Member { site, symbol }) => {
            return member_search(project, evaluator, site, symbol, request.filters)
                .map(ReferenceAnswer::Selected);
        }
        ReferenceSelection::Absent => ReferenceSelectionFailure::NotFound {
            candidates: Vec::new(),
        },
        ReferenceSelection::Ambiguous(candidates) => {
            ReferenceSelectionFailure::Ambiguous { candidates }
        }
    };
    Ok(ReferenceAnswer::Unselected(SelectionDocument {
        operation: ReferenceOperation::References,
        project: project.config.clone(),
        selection: request.query.clone(),
        outcome,
    }))
}

fn member_search(
    project: &Project,
    evaluator: &mut Evaluator<'_, '_>,
    selected: ReferenceSite,
    original: ReferenceSymbol,
    filters: ReferenceFilters,
) -> Result<ReferenceResult, AnalysisError> {
    let symbol = evaluator
        .reference_inline_union_symbol(original, &selected.selection)?
        .unwrap_or(original);
    let seed_declarations =
        evaluator.reference_symbol_named_declarations(symbol, &selected.selection)?;
    let declaration_meanings = evaluator
        .reference_symbol_declarations(symbol, &selected.selection)?
        .into_iter()
        .map(|declaration| match declaration {
            ReferenceDeclaration::Captured(declaration) => evaluator
                .source()
                .reference_declaration_meaning(declaration),
            ReferenceDeclaration::Json { .. } => Ok(ReferenceMeaning::VALUE),
        })
        .collect::<Result<Vec<_>, DemandError>>()?;
    let meaning = evaluator
        .source()
        .reference_meaning(&selected)?
        .intersecting_declarations(&declaration_meanings);
    let mut symbols = SearchSymbols::new();
    let seed_identity = evaluator.reference_symbol_identity(symbol, &selected.selection)?;
    let mut seed_group = None;
    for_each_related(
        evaluator,
        symbol,
        &selected,
        &mut |evaluator, sym, root, base| {
            if seed_group.is_none()
                && evaluator.reference_symbol_identity(sym, &selected.selection)? == seed_identity
            {
                seed_group = Some(reference_group_symbol(
                    evaluator,
                    sym,
                    root,
                    &selected.selection,
                )?);
            }
            let base = matching_static_base(evaluator, symbol, base, &selected.selection)?;
            symbols.insert(
                evaluator
                    .reference_symbol_identity(base.or(root).unwrap_or(sym), &selected.selection)?,
            );
            Ok(None)
        },
    )?;
    if symbols.is_empty() {
        symbols.insert(evaluator.reference_symbol_identity(symbol, &selected.selection)?);
    }

    let search = std::rc::Rc::new(Search {
        key: super::reference_imports::search_key(
            evaluator,
            symbol,
            &selected.selection,
            Some(&selected.key),
        )?,
        symbols,
        coming_from: ComingFrom::Unknown,
    });
    let scope = symbol_scope(evaluator, symbol, &selected.selection)?;
    let seed_group = seed_group.unwrap_or(symbol);
    let seed_group_identity =
        evaluator.reference_symbol_identity(seed_group, &selected.selection)?;
    let seed_declarations = if seed_group_identity == seed_identity {
        seed_declarations
    } else {
        evaluator.reference_symbol_named_declarations(seed_group, &selected.selection)?
    };
    let mut state = SearchState {
        project,
        filters,
        meaning,
        control: evaluator.checker_inputs().control.cloned(),
        ledger: Ledger::new(evaluator.checker_inputs().control.cloned()),
        searched: FixedMap::new(),
        reexports: FixedMap::new(),
        imports: super::reference_import_tracker::ImportTracker::new(project),
    };
    let work = if let Some(scope) = scope {
        state.work([Work::Container {
            search,
            file: scope.file,
            span: Some(evaluator.source().reference_container_span(scope)?),
            add: project.bindings.reference_file_is_searched(scope.file),
            require_name: false,
            at: copy_location(&selected.selection),
        }])
    } else {
        state.work(
            project
                .sources
                .ids()
                .filter(|&file| project.bindings.reference_file_is_searched(file))
                .map(|file| Work::Container {
                    search: search.clone(),
                    file,
                    span: None,
                    add: true,
                    require_name: true,
                    at: copy_location(&selected.selection),
                }),
        )
    };
    state.run(evaluator, work)?;
    // Native groups are created by established occurrences. Product filters
    // additionally retain the selected declarations even when its own group
    // has no surviving occurrence.
    if filters.calls || filters.exclude_tests {
        state
            .ledger
            .ensure_group(seed_group_identity, seed_declarations);
    }
    let (groups, gaps) = state.ledger.finish();
    let outcome = if gaps.is_empty() {
        ReferenceOutcome::Complete
    } else {
        ReferenceOutcome::Partial
    };
    Ok(ReferenceResult {
        operation: ReferenceOperation::References,
        outcome,
        project: project.config.clone(),
        groups,
        filters,
        coverage: ReferenceCoverage {
            baseline: ReferenceBaseline::Member,
            status: outcome,
            gaps,
        },
        unsearched_members: UnsearchedMembers::default(),
    })
}

/// A native refSearch has its own symbol set and spelling. Import searches
/// use the imported alias only; they do not repopulate related symbols from a
/// discovered occurrence or replace the selected operation's search meaning.
struct Search {
    symbols: SearchSymbols,
    key: String,
    coming_from: ComingFrom,
}

/// The native refSearch symbol set only needs exact membership and a complete
/// visitation list. Iteration marks searched identities without semantic child
/// acquisition; insertion order has no effect on that commutative union.
struct SearchSymbols {
    identities: FixedMap<NativeSymbolIdentity, ()>,
    ordered: Vec<NativeSymbolIdentity>,
}
impl SearchSymbols {
    fn new() -> Self {
        Self {
            identities: FixedMap::new(),
            ordered: Vec::new(),
        }
    }
    fn insert(&mut self, symbol: NativeSymbolIdentity) {
        if self.identities.insert(symbol, ()).is_none() {
            self.ordered.push(symbol);
        }
    }
    fn is_empty(&self) -> bool {
        self.ordered.is_empty()
    }
    fn contains(&self, symbol: &NativeSymbolIdentity) -> bool {
        self.identities.contains_key(symbol)
    }
}

struct SearchState<'a> {
    project: &'a Project,
    filters: ReferenceFilters,
    meaning: ReferenceMeaning,
    control: Option<crate::resources::Control>,
    ledger: Ledger,
    searched: FixedMap<(FileId, NativeSymbolIdentity), ()>,
    reexports: FixedMap<ExpressionRoot, ()>,
    imports: super::reference_import_tracker::ImportTracker<'a>,
}

enum Work {
    Container {
        search: std::rc::Rc<Search>,
        file: FileId,
        span: Option<crate::Span>,
        add: bool,
        require_name: bool,
        at: Location,
    },
    Candidate {
        search: std::rc::Rc<Search>,
        token: ReferenceToken,
        add: bool,
        at: Location,
    },
    Imported {
        symbol: ReferenceSymbol,
        at: Location,
    },
    ImportExportSpecifier {
        declaration: DeclarationIdentity,
        at: Location,
    },
    ImportedFile {
        symbol: ReferenceSymbol,
        file: FileId,
        at: Location,
    },
    Exported {
        symbol: ReferenceSymbol,
        info: ExportInfo,
        at: Location,
    },
    Single {
        symbol: ReferenceSymbol,
        root: ExpressionRoot,
        at: Location,
    },
    ImportSearch {
        symbol: ReferenceSymbol,
        root: ExpressionRoot,
        at: Location,
    },
    Indirect {
        symbol: ReferenceSymbol,
        info: ExportInfo,
        files: Vec<FileId>,
        at: Location,
    },
}
impl Work {
    fn at(&self) -> &Location {
        match self {
            Self::Container { at, .. }
            | Self::Candidate { at, .. }
            | Self::Imported { at, .. }
            | Self::ImportExportSpecifier { at, .. }
            | Self::ImportedFile { at, .. }
            | Self::Exported { at, .. }
            | Self::Single { at, .. }
            | Self::ImportSearch { at, .. }
            | Self::Indirect { at, .. } => at,
        }
    }
}

impl SearchState<'_> {
    // The native find-references continuation owns scheduling.

    fn extend_work(&self, output: &mut Vec<Work>, values: impl IntoIterator<Item = Work>) {
        for value in values {
            output.push(value);
        }
    }
    fn work(&self, values: impl IntoIterator<Item = Work>) -> Vec<Work> {
        let mut work = Vec::new();
        self.extend_work(&mut work, values);
        work
    }

    /// Iterative native depth-first continuation: a matched candidate finishes
    /// its import/export work before the next candidate. File/symbol marks
    /// terminate cycles without using the Rust call stack as a resource limit.
    fn run(
        &mut self,
        evaluator: &mut Evaluator<'_, '_>,
        work: Vec<Work>,
    ) -> Result<(), AnalysisError> {
        let mut stack = Vec::new();
        {
            for item in work.into_iter().rev() {
                stack.push(item);
            }
        }
        loop {
            let work = { stack.pop() };
            let Some(work) = work else {
                break;
            };
            let mut at = copy_location(work.at());
            match self.execute(evaluator, work, &mut at) {
                Ok(children) => {
                    for child in children.into_iter().rev() {
                        stack.push(child);
                    }
                }
                Err(failure) => self.ledger.gap(at, failure)?,
            }
        }
        Ok(())
    }

    fn execute(
        &mut self,
        evaluator: &mut Evaluator<'_, '_>,
        work: Work,
        failure_site: &mut Location,
    ) -> Result<Vec<Work>, DemandError> {
        match work {
            Work::Container {
                search,
                file,
                span,
                add,
                require_name,
                at: _,
            } => {
                let tokens = self
                    .project
                    .bindings
                    .reference_tokens(&search.key, self.control.as_ref());
                if require_name && !tokens.iter().any(|token| token.file() == file) {
                    return Ok(Vec::new());
                }
                let mut fresh = false;
                for &symbol in &search.symbols.ordered {
                    fresh |= self.searched.insert((file, symbol), ()).is_none();
                }
                if !fresh {
                    return Ok(Vec::new());
                }
                Ok(self.work(tokens.iter().filter_map(|token| {
                    let facts = self.project.bindings.reference_token_facts(token);
                    if facts.file != file
                        || span.is_some_and(|span| {
                            facts.selection_span.start < span.start
                                || facts.selection_span.start >= span.end
                        })
                    {
                        return None;
                    }
                    let eligible = self.occurrence_allowed(file, facts.call);
                    if !eligible && !facts.traversal_header {
                        return None;
                    }
                    Some(Work::Candidate {
                        search: search.clone(),
                        token,
                        add: add && eligible,
                        at: super::location(
                            evaluator.checker_inputs(),
                            file,
                            facts.occurrence_span,
                        ),
                    })
                })))
            }
            Work::Candidate {
                search,
                token,
                add,
                at: _,
            } => self.candidate(evaluator, &search, token, add),
            Work::ImportExportSpecifier { declaration, at } => Ok(self.work(
                evaluator
                    .reference_export_specifier_local_target(declaration, &at)?
                    .map(|symbol| Work::Imported { symbol, at }),
            )),
            Work::Imported { symbol, at } => Ok(self.work(
                evaluator
                    .reference_symbol_declarations(symbol, &at)?
                    .into_iter()
                    .map(|declaration| {
                        let file = match declaration {
                            ReferenceDeclaration::Captured(declaration) => declaration.file,
                            ReferenceDeclaration::Json { file, .. } => file,
                        };
                        Work::ImportedFile {
                            symbol,
                            file,
                            at: copy_location(&at),
                        }
                    }),
            )),
            Work::ImportedFile { symbol, file, at } => {
                let search =
                    self.create_search(evaluator, symbol, ComingFrom::Import, None, &at)?;
                Ok(self.work([Work::Container {
                    search,
                    file,
                    span: None,
                    add: self.project.bindings.reference_file_is_searched(file),
                    require_name: false,
                    at,
                }]))
            }
            Work::Exported { symbol, info, at } => {
                let imports =
                    self.imports
                        .searches(evaluator, symbol, info.module, info.kind, &at)?;
                let mut work = Vec::new();
                self.extend_work(
                    &mut work,
                    imports
                        .single_references
                        .into_iter()
                        .map(|root| Work::Single {
                            symbol,
                            root,
                            at: copy_location(&at),
                        }),
                );
                self.extend_work(
                    &mut work,
                    imports
                        .import_searches
                        .into_iter()
                        .map(|import| Work::ImportSearch {
                            symbol: import.symbol,
                            root: import.location,
                            at: copy_location(&at),
                        }),
                );
                if !imports.indirect_users.is_empty() {
                    self.extend_work(
                        &mut work,
                        [Work::Indirect {
                            symbol,
                            info,
                            files: imports.indirect_users,
                            at,
                        }],
                    );
                }
                Ok(work)
            }
            Work::Single {
                symbol,
                root,
                at: _,
            } => {
                let site = self.site(evaluator, root)?;
                // Once the native single-reference site is acquired, failures
                // belong to that candidate, not the export that reached it.
                *failure_site = copy_location(&site.occurrence);
                if self.site_allowed(&site)
                    && evaluator
                        .source()
                        .reference_meaning(&site)?
                        .intersects(self.meaning)
                {
                    self.ledger.reference(evaluator, symbol, &site)?;
                }
                Ok(Vec::new())
            }
            Work::ImportSearch { symbol, root, at } => {
                let search =
                    self.create_search(evaluator, symbol, ComingFrom::Export, None, &at)?;
                Ok(self.work([Work::Container {
                    search,
                    file: root.file,
                    span: None,
                    add: true,
                    require_name: false,
                    at,
                }]))
            }
            Work::Indirect {
                symbol,
                info,
                files,
                at,
            } => {
                let key = match info.kind {
                    ReferenceExportKind::Named => None,
                    ReferenceExportKind::Default => Some("default"),
                    ReferenceExportKind::Equals => return Ok(Vec::new()),
                };
                let search = self.create_search(evaluator, symbol, ComingFrom::Export, key, &at)?;
                Ok(self.work(files.into_iter().map(|file| Work::Container {
                    search: search.clone(),
                    file,
                    span: None,
                    add: true,
                    require_name: true,
                    at: copy_location(&at),
                })))
            }
        }
    }

    fn create_search(
        &self,
        evaluator: &mut Evaluator<'_, '_>,
        symbol: ReferenceSymbol,
        coming_from: ComingFrom,
        key: Option<&str>,
        at: &Location,
    ) -> Result<std::rc::Rc<Search>, DemandError> {
        let key = match key {
            Some(key) => crate::references::metadata::copy_text(key),
            None => super::reference_imports::search_key(evaluator, symbol, at, None)?,
        };
        let mut symbols = SearchSymbols::new();
        symbols.insert(evaluator.reference_symbol_identity(symbol, at)?);

        Ok(std::rc::Rc::new(Search {
            key,
            coming_from,
            symbols,
        }))
    }

    fn candidate(
        &mut self,
        evaluator: &mut Evaluator<'_, '_>,
        search: &Search,
        token: ReferenceToken,
        add: bool,
    ) -> Result<Vec<Work>, DemandError> {
        let site = evaluator
            .source()
            .reference_site(self.project.bindings.reference_token_facts(token))?;
        if !evaluator.source().reference_position_is_valid(&site)?
            || !evaluator
                .source()
                .reference_meaning(&site)?
                .intersects(self.meaning)
        {
            return Ok(Vec::new());
        }
        let Some(reference) = evaluator.reference_symbol_at_site(&site)? else {
            return Ok(Vec::new());
        };
        // `getReferencesAtLocation` passes the actual GetSymbolAtLocation result
        // to relatedness. Combined-member normalization selects the seed only:
        // repeating it here can replace a merged candidate with a raw parent's
        // own member and manufacture a match to a different symbol.
        if evaluator.source().reference_is_import_property(&site)? {
            return Ok(Vec::new());
        }
        if let Some(specifier) = evaluator.source().reference_export_specifier(&site)? {
            return self.export_specifier(evaluator, search, reference, &site, specifier, add);
        }
        let matched = for_each_related(
            evaluator,
            reference,
            &site,
            &mut |evaluator, sym, root, base| {
                let base = matching_static_base(evaluator, reference, base, &site.selection)?;
                let identity = evaluator
                    .reference_symbol_identity(base.or(root).unwrap_or(sym), &site.selection)?;
                if !search.symbols.contains(&identity) {
                    return Ok(None);
                }
                reference_group_symbol(evaluator, sym, root, &site.selection).map(Some)
            },
        )?;
        if let Some(matched) = matched {
            if add {
                self.ledger.reference(evaluator, matched, &site)?;
            }
            Ok(self.follow(
                super::reference_imports::continuation(
                    &self.project.bindings,
                    evaluator,
                    &site,
                    reference,
                    search.coming_from,
                )?,
                &site,
            ))
        } else {
            if let Some((shorthand, declaration)) =
                evaluator.reference_shorthand_fallback(&self.project.bindings, reference, &site)?
                && search
                    .symbols
                    .contains(&evaluator.reference_symbol_identity(shorthand, &site.selection)?)
                && self.site_allowed(&declaration)
            {
                self.ledger.reference(evaluator, shorthand, &declaration)?;
            }
            Ok(Vec::new())
        }
    }

    /// `getReferencesAtExportSpecifier` bypasses general related-symbol
    /// matching and asks its local identity before adding either name role.
    fn export_specifier(
        &mut self,
        evaluator: &mut Evaluator<'_, '_>,
        search: &Search,
        reference: ReferenceSymbol,
        site: &ReferenceSite,
        specifier: super::source::ReferenceExportSpecifier,
        add: bool,
    ) -> Result<Vec<Work>, DemandError> {
        let local = if specifier.at_property || !specifier.renamed && !specifier.has_module {
            evaluator
                .reference_export_specifier_local_target(specifier.declaration, &site.selection)?
                .unwrap_or(reference)
        } else {
            reference
        };
        if !search
            .symbols
            .contains(&evaluator.reference_symbol_identity(local, &site.selection)?)
        {
            return Ok(Vec::new());
        }
        if !specifier.renamed {
            if add {
                self.ledger.reference(evaluator, local, site)?;
            }
        } else if specifier.at_property {
            if add && !specifier.has_module {
                self.ledger.reference(evaluator, local, site)?;
            }
            if add && self.reexports.insert(specifier.name, ()).is_none() {
                let exported = evaluator
                    .reference_raw_symbol_of_declaration(specifier.declaration)?
                    .ok_or_else(|| {
                        super::internal("export specifier lacks its raw declaration symbol")
                    })?;
                let name = self.site(evaluator, specifier.name)?;
                if self.site_allowed(&name) {
                    self.ledger.reference(evaluator, exported, &name)?;
                }
            }
        } else if self.reexports.insert(specifier.name, ()).is_none() && add {
            self.ledger.reference(evaluator, local, site)?;
        }
        let exported = evaluator
            .reference_raw_symbol_of_declaration(specifier.declaration)?
            .ok_or_else(|| super::internal("export specifier lacks its raw declaration symbol"))?;
        let mut work = Vec::new();
        if let Some(info) = super::reference_imports::export_info(
            evaluator,
            exported,
            specifier.kind,
            &site.selection,
        )? {
            self.extend_work(
                &mut work,
                [Work::Exported {
                    symbol: exported,
                    info,
                    at: copy_location(&site.selection),
                }],
            );
        }
        // This lookup occurs after searching the export's imports in native
        // code. Keep it as a continuation so earlier work retains its order.
        if search.coming_from != ComingFrom::Export && specifier.has_module && !specifier.renamed {
            self.extend_work(
                &mut work,
                [Work::ImportExportSpecifier {
                    declaration: specifier.declaration,
                    at: copy_location(&site.selection),
                }],
            );
        }
        Ok(work)
    }

    fn follow(&self, continuation: Option<Continuation>, site: &ReferenceSite) -> Vec<Work> {
        match continuation {
            None => Vec::new(),
            Some(Continuation::Import(symbol)) => self.work([Work::Imported {
                symbol,
                at: copy_location(&site.selection),
            }]),
            Some(Continuation::Export { symbol, info }) => self.work([Work::Exported {
                symbol,
                info,
                at: copy_location(&site.selection),
            }]),
        }
    }

    fn site(
        &self,
        evaluator: &mut Evaluator<'_, '_>,
        root: ExpressionRoot,
    ) -> Result<ReferenceSite, DemandError> {
        Ok(evaluator
            .source()
            .reference_node_site(&self.project.bindings, root)?
            .ok_or_else(|| {
                super::internal("import/export reference lacks a captured inventory role")
            })?)
    }

    fn occurrence_allowed(&self, file: FileId, call: bool) -> bool {
        (!self.filters.calls || call)
            && (!self.filters.exclude_tests
                || !references::is_test_path(&self.project.sources.get(file).display))
    }

    fn site_allowed(&self, site: &ReferenceSite) -> bool {
        let file = match site.target {
            super::source::ReferenceSiteTarget::Captured { root, .. } => root.file,
            super::source::ReferenceSiteTarget::Json { object, .. } => object.file,
        };
        self.project.bindings.reference_file_is_searched(file)
            && self.occurrence_allowed(file, site.call)
    }
}

/// `getRelatedSymbol` groups ordinary transient/instantiated symbols by their
/// root, while synthetic union/intersection symbols retain their own group.
/// The product's retained seed group uses the same decision as occurrences.
fn reference_group_symbol(
    evaluator: &mut Evaluator<'_, '_>,
    symbol: ReferenceSymbol,
    root: Option<ReferenceSymbol>,
    consumer: &Location,
) -> Result<ReferenceSymbol, DemandError> {
    if let Some(root) = root
        && !evaluator
            .reference_symbol_flags(symbol, consumer)?
            .synthetic
    {
        return Ok(root);
    }
    Ok(symbol)
}

/// Native `forEachRelatedSymbol`: callback order and early exit are observable
/// when a later semantic dependency fails. This is not an equivalence closure.
fn for_each_related(
    evaluator: &mut Evaluator<'_, '_>,
    symbol: ReferenceSymbol,
    site: &ReferenceSite,
    callback: &mut impl FnMut(
        &mut Evaluator<'_, '_>,
        ReferenceSymbol,
        Option<ReferenceSymbol>,
        Option<ReferenceSymbol>,
    ) -> Result<Option<ReferenceSymbol>, DemandError>,
) -> Result<Option<ReferenceSymbol>, DemandError> {
    // The native shorthand query precedes contextual acquisition, although its
    // callback follows both contextual and destructuring property callbacks.
    let shorthand = evaluator.reference_shorthand_value(site)?;
    for contextual in evaluator.reference_contextual_properties(site)? {
        if let Some(found) = from_root(evaluator, contextual, site, callback)? {
            return Ok(Some(found));
        }
    }
    if let Some(property) = evaluator.reference_destructuring_property(site)? {
        if let Some(found) = callback(evaluator, property, None, None)? {
            return Ok(Some(found));
        }
    }
    if let Some(shorthand) = shorthand {
        if let Some(found) = callback(evaluator, shorthand, None, None)? {
            return Ok(Some(found));
        }
    }
    if let Some(alias) = evaluator.reference_umd_alias(symbol, site)? {
        if let Some(found) = callback(evaluator, alias, None, None)? {
            return Ok(Some(found));
        }
    }
    if let Some(found) = from_root(evaluator, symbol, site, callback)? {
        return Ok(Some(found));
    }
    if let Some(other) = evaluator.reference_parameter_property_other(symbol, &site.selection)? {
        return from_root(evaluator, other, site, callback);
    }
    if let Some(local) = evaluator.reference_export_specifier_target(symbol, &site.selection)? {
        if let Some(found) = callback(evaluator, local, None, None)? {
            return Ok(Some(found));
        }
    }
    if let Some(property) = evaluator.reference_binding_element_property(site)? {
        return from_root(evaluator, property, site, callback);
    }
    Ok(None)
}

fn from_root(
    evaluator: &mut Evaluator<'_, '_>,
    symbol: ReferenceSymbol,
    site: &ReferenceSite,
    callback: &mut impl FnMut(
        &mut Evaluator<'_, '_>,
        ReferenceSymbol,
        Option<ReferenceSymbol>,
        Option<ReferenceSymbol>,
    ) -> Result<Option<ReferenceSymbol>, DemandError>,
) -> Result<Option<ReferenceSymbol>, DemandError> {
    for root in evaluator.reference_root_symbols(symbol, &site.selection)? {
        if let Some(found) = callback(evaluator, symbol, Some(root), None)? {
            return Ok(Some(found));
        }
        if let Some(parent) = evaluator.reference_symbol_parent(root, &site.selection)? {
            if evaluator.reference_parent_is_class_or_interface(parent)? {
                let name = evaluator.reference_symbol_key(root, &site.selection)?;
                // Search.parents is populated only for implementation search,
                // which this product operation does not perform.
                if let Some(found) = evaluator.reference_base_properties(
                    parent,
                    &name,
                    &site.selection,
                    &mut |evaluator, base| callback(evaluator, symbol, Some(root), Some(base)),
                )? {
                    return Ok(Some(found));
                }
            }
        }
    }
    Ok(None)
}

fn matching_static_base(
    evaluator: &mut Evaluator<'_, '_>,
    symbol: ReferenceSymbol,
    base: Option<ReferenceSymbol>,
    consumer: &Location,
) -> Result<Option<ReferenceSymbol>, DemandError> {
    if let Some(base) = base {
        if is_static(evaluator, symbol, consumer)? != is_static(evaluator, base, consumer)? {
            return Ok(None);
        }
    }
    Ok(base)
}

/// Native `isStaticSymbol` inspects the ValueDeclaration's own modifier.
fn is_static(
    evaluator: &mut Evaluator<'_, '_>,
    symbol: ReferenceSymbol,
    consumer: &Location,
) -> Result<bool, DemandError> {
    match evaluator.reference_symbol_value_declaration(symbol, consumer)? {
        Some(ReferenceDeclaration::Captured(declaration)) => evaluator
            .source()
            .declaration_has_modifier(declaration, SyntaxKind::StaticKeyword),
        Some(ReferenceDeclaration::Json { .. }) | None => Ok(false),
    }
}

/// Native `getSymbolScope`: its declaration-container question is independent
/// of the decoded-key index and precedes all candidate semantic work.
fn symbol_scope(
    evaluator: &mut Evaluator<'_, '_>,
    symbol: ReferenceSymbol,
    consumer: &Location,
) -> Result<Option<DeclarationIdentity>, DemandError> {
    if let Some(ReferenceDeclaration::Captured(value)) =
        evaluator.reference_symbol_value_declaration(symbol, consumer)?
    {
        if matches!(
            evaluator.source().declaration_kind(value)?,
            SyntaxKind::FunctionExpression | SyntaxKind::ClassExpression
        ) {
            return Ok(Some(value));
        }
    }
    let declarations = evaluator.reference_symbol_declarations(symbol, consumer)?;
    if declarations.is_empty() {
        return Ok(None);
    }
    let flags = evaluator.reference_symbol_flags(symbol, consumer)?;
    if flags.property || flags.method {
        for declaration in declarations {
            if let ReferenceDeclaration::Captured(declaration) = declaration {
                if evaluator
                    .source()
                    .declaration_is_private_reference_member(declaration)?
                {
                    return evaluator.source().reference_private_scope(declaration);
                }
            }
        }
        return Ok(None);
    }
    for declaration in &declarations {
        if let ReferenceDeclaration::Captured(declaration) = declaration {
            if evaluator
                .source()
                .reference_is_object_binding_without_property(*declaration)?
            {
                return Ok(None);
            }
        }
    }
    let parent = evaluator.reference_symbol_parent(symbol, consumer)?;
    let exposed = parent.is_some() && !flags.type_parameter;
    if exposed
        && !evaluator
            .reference_parent_limits_scope(parent.expect("parent presence checked"), consumer)?
    {
        return Ok(None);
    }
    let mut scope = None;
    for declaration in declarations {
        let ReferenceDeclaration::Captured(declaration) = declaration else {
            return Ok(None);
        };
        let Some(container) = evaluator.source().reference_container(declaration)? else {
            return Ok(None);
        };
        if scope.is_some_and(|scope| scope != container)
            || evaluator.source().declaration_kind(container)? == SyntaxKind::SourceFile
                && !evaluator.reference_file_is_external_or_commonjs(container.file)
        {
            return Ok(None);
        }
        scope = Some(container);
    }
    if exposed {
        scope
            .map(|scope| evaluator.source().reference_source_file(scope))
            .transpose()
    } else {
        Ok(scope)
    }
}
