//! Native symbol qualification uses raw lexical tables, then the shared
//! alias/member owners. Display spelling and output syntax stay in nodebuilder.

mod containers;
mod visibility;

use super::super::source::DeclarationIdentity;
use super::native_display::DiagnosticScopeId;
use super::reference_symbols::ReferenceSymbolFlags;

/// Native lexical location identity includes node-builder-created Blocks.
/// Their parent/locals live in the shared display scope arena, never a
/// fabricated captured declaration or a cache key derived from table contents.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum ScopeLocation {
    Captured(DeclarationIdentity),
    Synthetic(DiagnosticScopeId),
}

impl From<DeclarationIdentity> for ScopeLocation {
    fn from(declaration: DeclarationIdentity) -> Self {
        Self::Captured(declaration)
    }
}

/// Native SymbolFlags meaning at a node-builder lookup. This is distinct from
/// binder declaration spaces and from LS reference-search meanings.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic) enum SymbolMeaning {
    Value,
    /// `getMeaningOfEntityNameReference` includes the separate exported
    /// local marker; this is deliberately not equal to the plain Value mask.
    ValueExport,
    Type,
    Namespace,
}

impl SymbolMeaning {
    fn binding_mask(self) -> u8 {
        use crate::project::binding::meaning;
        match self {
            Self::Value | Self::ValueExport => meaning::VALUE,
            Self::Type => meaning::TYPE,
            Self::Namespace => meaning::NAMESPACE,
        }
    }
    /// `getQualifiedLeftMeaning` preserves Value only for the exact Value
    /// mask; type and namespace qualifiers both ask the Namespace question.
    pub(super) fn qualifier(self) -> Self {
        if self == Self::Value {
            Self::Value
        } else {
            Self::Namespace
        }
    }

    pub(super) fn matches(self, flags: ReferenceSymbolFlags) -> bool {
        match self {
            Self::Value => flags.value,
            Self::ValueExport => flags.value || flags.export_value,
            Self::Type => flags.type_,
            Self::Namespace => flags.namespace,
        }
    }
}

impl Evaluator<'_, '_> {
    /// `resolveName` at a node-builder location uses synthetic locals only
    /// for the actual fake Blocks. Captured nodes delegate to the binder's
    /// retained lexical context and existing resumable resolver operation.
    pub(super) fn diagnostic_resolve_name_completion(
        &mut self,
        mut scope: Option<ScopeLocation>,
        name: &str,
        meaning: SymbolMeaning,
        include_alias: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            while let Some(ScopeLocation::Synthetic(id)) = scope {
                let key = NativePropertyKey::fixed(name.as_bytes(), self.control.as_ref());
                let symbol = self
                    .diagnostic_scope_locals(id)?
                    .iter()
                    .find(|(name, _)| name == &key)
                    .map(|(_, symbol)| *symbol);
                if let Some(symbol) = symbol {
                    let flags = self.reference_symbol_flags(symbol, consumer)?;
                    if meaning.matches(flags) || include_alias && flags.alias {
                        return Ok(Some(symbol));
                    }
                }
                scope = Some(self.diagnostic_scope_parent(id)?);
            }
            let native_meaning = crate::project::binding::NativeLookupMeaning::projected(
                meaning.binding_mask(),
                include_alias,
                meaning == SymbolMeaning::ValueExport,
            );
            match scope {
                Some(ScopeLocation::Captured(declaration)) => {
                    let (kind, span) = self.source.diagnostic_lookup_node(declaration)?;
                    sequence.read(self.reference_name_at_node_completion(
                        declaration.file,
                        kind,
                        span,
                        name,
                        native_meaning,
                        consumer,
                    ))
                }
                None => sequence.read(self.reference_global_name_completion(
                    name,
                    native_meaning,
                    Some(consumer),
                )),
                Some(ScopeLocation::Synthetic(_)) => {
                    unreachable!("synthetic scopes are traversed above")
                }
            }
        })();
        sequence.finish(result)
    }

    /// `resolveEntityName` for a first Identifier/This with ignoreErrors and
    /// dontResolveAlias. Reuse's override location is distinct from the source
    /// identifier; both run one raw-symbol lookup, not a type acquisition.
    pub(super) fn diagnostic_resolve_entity_name(
        &mut self,
        identifier: DeclarationIdentity,
        meaning: SymbolMeaning,
        override_scope: Option<ScopeLocation>,
        consumer: &Location,
    ) -> super::super::model::NativeResult<Option<ReferenceSymbol>> {
        let name = self.source.diagnostic_identifier_text(identifier)?;
        self.diagnostic_resolve_name_completion(
            override_scope.or(Some(identifier.into())),
            &name,
            meaning,
            true,
            consumer,
        )
    }
}

use super::reference_symbols::ReferenceSymbolTable;
use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use crate::project::binding::{NativePropertyKey, NativeSymbolIdentity};
use crate::project::semantic::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence,
};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Native symbolTableID records the question's source, not a physical map
/// allocation. Resolved exports must never collide with raw Symbol.Exports.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum TableId {
    Locals(ScopeLocation),
    Exports(NativeSymbolIdentity),
    Members(NativeSymbolIdentity),
    Globals,
    ResolvedExports(NativeSymbolIdentity),
}

struct ScopeTable {
    entries: ReferenceSymbolTable,
    id: TableId,
    local: bool,
    scope: Option<ScopeLocation>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ChainKey {
    symbol: NativeSymbolIdentity,
    external_only: bool,
    scope: Option<ScopeLocation>,
    meaning: SymbolMeaning,
}

#[derive(Default)]
pub(super) struct State {
    visible: HashMap<ReferenceDeclaration, bool>,
    chains: HashMap<ChainKey, NativeResult<Vec<ReferenceSymbol>>>,
    aliases: HashMap<TableId, Result<Vec<ReferenceSymbol>, DemandError>>,
    class_names: HashMap<DeclarationIdentity, Result<ReferenceSymbolTable, DemandError>>,
    containers_by_file: HashMap<
        (NativeSymbolIdentity, crate::project::sources::FileId),
        NativeResult<Vec<ReferenceSymbol>>,
    >,
    containers: HashMap<NativeSymbolIdentity, NativeResult<Vec<ReferenceSymbol>>>,
}

#[derive(Clone, Copy)]
struct ChainQuestion {
    symbol: ReferenceSymbol,
    enclosing: Option<ScopeLocation>,
    meaning: SymbolMeaning,
    external_only: bool,
}

type Visited = HashSet<(NativeSymbolIdentity, TableId)>;

impl Evaluator<'_, '_> {
    /// `getAccessibleSymbolChain` owns one search's visited table pairs. Its
    /// recursive worker shares this set, while completed results belong to the
    /// actual symbol/first relevant scope/meaning cache, including native nil.
    pub(super) fn accessible_symbol_chain_completion(
        &mut self,
        symbol: ReferenceSymbol,
        enclosing: Option<ScopeLocation>,
        meaning: SymbolMeaning,
        external_only: bool,
        consumer: &Location,
    ) -> NativeResult<Vec<ReferenceSymbol>> {
        self.accessible_symbol_chain_worker_completion(
            ChainQuestion {
                symbol,
                enclosing,
                meaning,
                external_only,
            },
            &mut Visited::new(),
            consumer,
        )
    }

    fn accessible_symbol_chain_worker_completion(
        &mut self,
        question: ChainQuestion,
        visited: &mut Visited,
        consumer: &Location,
    ) -> NativeResult<Vec<ReferenceSymbol>> {
        let declarations = self.reference_symbol_declarations(question.symbol, consumer)?;
        if !declarations.is_empty() {
            let mut only_members = true;
            for declaration in declarations {
                if !matches!(
                    self.reference_declaration_kind(declaration)?,
                    K::PropertyDeclaration | K::MethodDeclaration | K::GetAccessor | K::SetAccessor
                ) {
                    only_members = false;
                    break;
                }
            }
            if only_members {
                return Ok(NativeCompletion::clean(Vec::new()));
            }
        }
        self.with_scope_tables_completion(question.enclosing, consumer, |_, table| {
            Ok(NativeCompletion::clean(Some(table.scope)))
        })?
        .and_then(|first| {
            let key = ChainKey {
                symbol: self.reference_symbol_identity(question.symbol, consumer)?,
                external_only: question.external_only,
                scope: first.flatten(),
                meaning: question.meaning,
            };
            if let Some(result) = self.symbol_accessibility.chains.get(&key) {
                return result.clone();
            }
            let result = self
                .with_scope_tables_completion(question.enclosing, consumer, |this, table| {
                    Ok(this
                        .accessible_from_table_completion(
                            question,
                            &table.entries,
                            table.id,
                            false,
                            table.local,
                            visited,
                            consumer,
                        )?
                        .map(|chain| (!chain.is_empty()).then_some(chain)))
                })
                .map(|completed| completed.map(Option::unwrap_or_default));
            self.symbol_accessibility.chains.insert(key, result.clone());
            result
        })
    }

    /// `someSymbolTableInScope` is a pausable semantic walk: a callback can
    /// finish before later nodes/tables or late names have been acquired.
    fn with_scope_tables_completion<T>(
        &mut self,
        mut enclosing: Option<ScopeLocation>,
        consumer: &Location,
        mut callback: impl FnMut(&mut Self, ScopeTable) -> NativeResult<Option<T>>,
    ) -> NativeResult<Option<T>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            while let Some(scope) = enclosing {
                match scope {
                    ScopeLocation::Synthetic(id) => {
                        let entries: ReferenceSymbolTable = self
                            .diagnostic_scope_locals(id)?
                            .iter()
                            .map(|(name, symbol)| (name.clone(), *symbol))
                            .collect::<Vec<_>>()
                            .into();
                        if let Some(result) = sequence.read(callback(
                            self,
                            ScopeTable {
                                entries,
                                id: TableId::Locals(scope),
                                local: true,
                                scope: Some(scope),
                            },
                        ))? {
                            return Ok(Some(result));
                        }
                        enclosing = Some(self.diagnostic_scope_parent(id)?);
                    }
                    ScopeLocation::Captured(declaration) => {
                        let facts = self.source.accessibility_scope(declaration)?;
                        if facts.locals
                            && let Some(entries) = self.reference_raw_locals(declaration)?
                        {
                            if let Some(result) = sequence.read(callback(
                                self,
                                ScopeTable {
                                    entries,
                                    id: TableId::Locals(scope),
                                    local: true,
                                    scope: Some(scope),
                                },
                            ))? {
                                return Ok(Some(result));
                            }
                        }
                        use super::super::source::symbol_accessibility::AccessibilityTable;
                        match facts.table {
                            AccessibilityTable::None => {}
                            AccessibilityTable::Exports => {
                                let symbol = sequence.read(
                                    self.reference_symbol_of_declaration_completion(declaration),
                                )?;
                                let identity = self.reference_symbol_identity(symbol, consumer)?;
                                let entries = match self.reference_raw_symbol_table(symbol, true)? {
                                    Some(raw) => self.reference_raw_table_entries(raw, consumer)?,
                                    None => Arc::from([]),
                                };
                                // Native invokes the callback even when Exports is nil.
                                if let Some(result) = sequence.read(callback(
                                    self,
                                    ScopeTable {
                                        entries,
                                        id: TableId::Exports(identity),
                                        local: true,
                                        scope: Some(scope),
                                    },
                                ))? {
                                    return Ok(Some(result));
                                }
                            }
                            AccessibilityTable::Members {
                                named_class_expression,
                            } => {
                                let symbol = sequence.read(
                                    self.reference_symbol_of_declaration_completion(declaration),
                                )?;
                                let identity = self.reference_symbol_identity(symbol, consumer)?;
                                let mut entries = Vec::new();
                                if let Some(raw) = self.reference_raw_symbol_table(symbol, false)? {
                                    for (name, member) in
                                        self.reference_raw_table_entries(raw, consumer)?.iter()
                                    {
                                        let flags =
                                            self.reference_symbol_flags(*member, consumer)?;
                                        if flags.type_ {
                                            entries.push((name.clone(), *member));
                                        }
                                    }
                                }
                                if !entries.is_empty() {
                                    if let Some(result) = sequence.read(callback(
                                        self,
                                        ScopeTable {
                                            entries: entries.into(),
                                            id: TableId::Members(identity),
                                            local: false,
                                            scope: Some(scope),
                                        },
                                    ))? {
                                        return Ok(Some(result));
                                    }
                                }
                                if named_class_expression {
                                    let entries = if let Some(result) =
                                        self.symbol_accessibility.class_names.get(&declaration)
                                    {
                                        result.clone()?
                                    } else {
                                        let result = self
                                            .source
                                            .accessibility_class_expression_name(declaration)
                                            .map(|name| {
                                                let entries: ReferenceSymbolTable = name
                                                    .into_iter()
                                                    .filter(|name| !name.is_empty())
                                                    .map(|name| {
                                                        (
                                                            NativePropertyKey::fixed(
                                                                name.into_bytes(),
                                                                self.control.as_ref(),
                                                            ),
                                                            symbol,
                                                        )
                                                    })
                                                    .collect::<Vec<_>>()
                                                    .into();
                                                entries
                                            });
                                        self.symbol_accessibility
                                            .class_names
                                            .insert(declaration, result.clone());
                                        result?
                                    };
                                    if !entries.is_empty() {
                                        if let Some(result) = sequence.read(callback(
                                            self,
                                            ScopeTable {
                                                entries,
                                                id: TableId::Locals(scope),
                                                local: true,
                                                scope: Some(scope),
                                            },
                                        ))? {
                                            return Ok(Some(result));
                                        }
                                    }
                                }
                            }
                        }
                        enclosing = facts.parent.map(Into::into);
                    }
                }
            }
            let entries = self.reference_globals(consumer)?;
            sequence.read(callback(
                self,
                ScopeTable {
                    entries,
                    id: TableId::Globals,
                    local: true,
                    scope: None,
                },
            ))
        })();
        sequence.finish(result)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "getAccessibleSymbolChainFromSymbolTable's closure state is explicit here"
    )]
    fn accessible_from_table_completion(
        &mut self,
        question: ChainQuestion,
        table: &ReferenceSymbolTable,
        id: TableId,
        ignore_qualification: bool,
        local: bool,
        visited: &mut Visited,
        consumer: &Location,
    ) -> NativeResult<Vec<ReferenceSymbol>> {
        let pair = (
            self.reference_symbol_identity(question.symbol, consumer)?,
            id,
        );
        if !visited.insert(pair) {
            return Ok(NativeCompletion::clean(Vec::new()));
        }
        let result = self.try_accessibility_table_completion(
            question,
            table,
            id,
            ignore_qualification,
            local,
            visited,
            consumer,
        );
        visited.remove(&pair);
        result
    }

    fn accessibility_table_aliases(
        &mut self,
        table: &ReferenceSymbolTable,
        id: TableId,
        consumer: &Location,
    ) -> Result<Vec<ReferenceSymbol>, DemandError> {
        if matches!(id, TableId::Members(_)) {
            return Ok(Vec::new());
        }
        let cache = !matches!(id, TableId::Locals(_));
        if cache && let Some(result) = self.symbol_accessibility.aliases.get(&id) {
            return result.clone();
        }
        let result = (|| {
            let mut aliases = Vec::new();
            for (_, symbol) in table.iter() {
                if self.reference_symbol_flags(*symbol, consumer)?.alias {
                    aliases.push(*symbol);
                }
            }
            Ok(aliases)
        })();
        if cache {
            self.symbol_accessibility.aliases.insert(id, result.clone());
        }
        result
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "getAccessibleSymbolChainFromSymbolTable's closure state is explicit here"
    )]
    fn try_accessibility_table_completion(
        &mut self,
        question: ChainQuestion,
        table: &ReferenceSymbolTable,
        id: TableId,
        ignore_qualification: bool,
        local: bool,
        visited: &mut Visited,
        consumer: &Location,
    ) -> NativeResult<Vec<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let key = self.reference_symbol_key(question.symbol, consumer)?;
            let found = table
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, symbol)| *symbol);
            if let Some(symbol) = found
                && sequence.read(self.accessibility_is_accessible_completion(
                    question,
                    symbol,
                    None,
                    ignore_qualification,
                    visited,
                    consumer,
                ))?
            {
                return Ok(vec![question.symbol]);
            }
            let mut candidates = Vec::new();
            if let Some(symbol) = found
                && let Some(export) = self.reference_export_symbol(symbol, consumer)?
            {
                let export = self.get_merged_reference_symbol(export, consumer)?;
                if sequence.read(self.accessibility_is_accessible_completion(
                    question,
                    export,
                    None,
                    ignore_qualification,
                    visited,
                    consumer,
                ))? {
                    candidates.push(vec![question.symbol]);
                }
            }
            for symbol in self.accessibility_table_aliases(table, id, consumer)? {
                let name = self.reference_symbol_key(symbol, consumer)?;
                if matches!(name.fixed_bytes(), Some(b"export=" | b"default")) {
                    continue;
                }
                let declarations = self.reference_symbol_declarations(symbol, consumer)?;
                if let Some(first) = declarations.first()
                    && self.reference_declaration_kind(*first)? == K::NamespaceExportDeclaration
                    && let Some(enclosing) =
                        self.diagnostic_captured_enclosing(question.enclosing)?
                    && self
                        .inputs
                        .bindings
                        .reference_file_is_external(enclosing.file)
                {
                    continue;
                }
                if question.external_only {
                    let mut external = false;
                    for declaration in &declarations {
                        if let ReferenceDeclaration::Captured(declaration) = declaration
                            && self
                                .source
                                .accessibility_external_import_equals(*declaration)?
                        {
                            external = true;
                            break;
                        }
                    }
                    if !external {
                        continue;
                    }
                }
                if local {
                    let mut reexport = false;
                    for declaration in &declarations {
                        if let ReferenceDeclaration::Captured(declaration) = declaration
                            && self.source.accessibility_namespace_reexport(*declaration)?
                        {
                            reexport = true;
                            break;
                        }
                    }
                    if reexport {
                        continue;
                    }
                }
                if !ignore_qualification {
                    let mut specifier = false;
                    for declaration in declarations {
                        if self.reference_declaration_kind(declaration)? == K::ExportSpecifier {
                            specifier = true;
                            break;
                        }
                    }
                    if specifier {
                        continue;
                    }
                }
                let resolved =
                    sequence.read(self.accessibility_resolve_alias_completion(symbol, consumer))?;
                let candidate = sequence.read(self.accessibility_candidate_completion(
                    question,
                    symbol,
                    resolved,
                    ignore_qualification,
                    visited,
                    consumer,
                ))?;
                if !candidate.is_empty() {
                    candidates.push(candidate);
                }
            }
            if !candidates.is_empty() {
                self.sort_accessibility_chains(&mut candidates, consumer)?;
                return Ok(candidates.remove(0));
            }
            if id == TableId::Globals {
                let global = self.global_this_reference_symbol();
                return sequence.read(self.accessibility_candidate_completion(
                    question,
                    global,
                    global,
                    ignore_qualification,
                    visited,
                    consumer,
                ));
            }
            Ok(Vec::new())
        })();
        sequence.finish(result)
    }

    fn accessibility_candidate_completion(
        &mut self,
        question: ChainQuestion,
        symbol: ReferenceSymbol,
        resolved: ReferenceSymbol,
        ignore_qualification: bool,
        visited: &mut Visited,
        consumer: &Location,
    ) -> NativeResult<Vec<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if sequence.read(self.accessibility_is_accessible_completion(
                question,
                symbol,
                Some(resolved),
                ignore_qualification,
                visited,
                consumer,
            ))? {
                return Ok(vec![symbol]);
            }
            let Some(table) =
                sequence.read(self.reference_exports_of_symbol_completion(resolved, consumer))?
            else {
                return Ok(Vec::new());
            };
            let id = TableId::ResolvedExports(self.reference_symbol_identity(resolved, consumer)?);
            let chain = sequence.read(self.accessible_from_table_completion(
                question, &table, id, true, false, visited, consumer,
            ))?;
            if chain.is_empty()
                || !sequence.read(self.accessibility_can_qualify_completion(
                    question,
                    symbol,
                    question.meaning.qualifier(),
                    visited,
                    consumer,
                ))?
            {
                return Ok(Vec::new());
            }
            Ok(std::iter::once(symbol).chain(chain).collect())
        })();
        sequence.finish(result)
    }

    fn accessibility_is_accessible_completion(
        &mut self,
        question: ChainQuestion,
        symbol: ReferenceSymbol,
        resolved: Option<ReferenceSymbol>,
        ignore_qualification: bool,
        visited: &mut Visited,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let wanted = self.reference_symbol_identity(question.symbol, consumer)?;
            let mut same = self.reference_symbol_identity(symbol, consumer)? == wanted;
            if let Some(resolved) = resolved {
                same |= self.reference_symbol_identity(resolved, consumer)? == wanted;
            }
            let wanted = self.get_merged_reference_symbol(question.symbol, consumer)?;
            let wanted = self.reference_symbol_identity(wanted, consumer)?;
            if let Some(resolved) = resolved {
                let resolved = self.get_merged_reference_symbol(resolved, consumer)?;
                same |= self.reference_symbol_identity(resolved, consumer)? == wanted;
            }
            let merged = self.get_merged_reference_symbol(symbol, consumer)?;
            same |= self.reference_symbol_identity(merged, consumer)? == wanted;
            if !same {
                return Ok(false);
            }
            for declaration in self.reference_symbol_declarations(symbol, consumer)? {
                if self.accessibility_external_declaration(declaration)? {
                    return Ok(false);
                }
            }
            Ok(ignore_qualification
                || sequence.read(self.accessibility_can_qualify_completion(
                    question,
                    merged,
                    question.meaning,
                    visited,
                    consumer,
                ))?)
        })();
        sequence.finish(result)
    }

    fn accessibility_can_qualify_completion(
        &mut self,
        question: ChainQuestion,
        symbol: ReferenceSymbol,
        meaning: SymbolMeaning,
        visited: &mut Visited,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !sequence.read(self.symbol_needs_qualification_completion(
                symbol,
                question.enclosing,
                meaning,
                consumer,
            ))? {
                return Ok(true);
            }
            // Native canQualifySymbol deliberately uses raw Parent, unlike the
            // late/merged getParentOfSymbol question in container discovery.
            let Some(parent) = self.reference_raw_parent_symbol(symbol, consumer)? else {
                return Ok(false);
            };
            Ok(!sequence
                .read(self.accessible_symbol_chain_worker_completion(
                    ChainQuestion {
                        symbol: parent,
                        meaning: meaning.qualifier(),
                        ..question
                    },
                    visited,
                    consumer,
                ))?
                .is_empty())
        })();
        sequence.finish(result)
    }

    /// `needsQualification` tests the first same-meaning symbol, resolving
    /// aliases except ExportSpecifier and preserving raw equality first.
    pub(super) fn symbol_needs_qualification_completion(
        &mut self,
        symbol: ReferenceSymbol,
        enclosing: Option<ScopeLocation>,
        meaning: SymbolMeaning,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let key = self.reference_symbol_key(symbol, consumer)?;
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        Ok(self
            .with_scope_tables_completion(enclosing, consumer, |this, table| {
                let mut sequence = NativeSequence::new();
                let result = (|| {
                    let Some(found) = table
                        .entries
                        .iter()
                        .find(|(name, _)| *name == key)
                        .map(|(_, symbol)| *symbol)
                    else {
                        return Ok(None);
                    };
                    let mut found = this.get_merged_reference_symbol(found, consumer)?;
                    if this.reference_symbol_identity(found, consumer)? == identity {
                        return Ok(Some(false));
                    }
                    let mut resolve = this.reference_symbol_flags(found, consumer)?.alias;
                    if resolve {
                        for declaration in this.reference_symbol_declarations(found, consumer)? {
                            if this.reference_declaration_kind(declaration)? == K::ExportSpecifier {
                                resolve = false;
                                break;
                            }
                        }
                    }
                    if resolve {
                        found = sequence
                            .read(this.accessibility_resolve_alias_completion(found, consumer))?;
                    }
                    let flags = if resolve {
                        sequence.read(
                            this.reference_combined_symbol_flags_completion(found, consumer),
                        )?
                    } else {
                        this.reference_symbol_flags(found, consumer)?
                    };
                    Ok(meaning.matches(flags).then_some(true))
                })();
                sequence.finish(result)
            })?
            .map(|needed| needed.unwrap_or(false)))
    }

    fn accessibility_resolve_alias_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<ReferenceSymbol> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let target = sequence
                .read(self.resolve_alias_target_completion(symbol, consumer))?
                .ok_or_else(|| {
                    super::super::internal("resolveAlias returned nil during accessibility")
                })?;
            Ok(target)
        })();
        sequence.finish(result)
    }

    pub(super) fn accessibility_external_declaration(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> Result<bool, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => self
                .source
                .diagnostic_external_module_declaration(declaration),
            // JSON has a module symbol, but native SetExternalModuleIndicator
            // explicitly leaves its SourceFile indicator nil.
            ReferenceDeclaration::Json { .. } => Ok(false),
        }
    }

    fn sort_accessibility_chains(
        &mut self,
        chains: &mut [Vec<ReferenceSymbol>],
        consumer: &Location,
    ) -> Result<(), DemandError> {
        if chains.len() < 2 {
            return Ok(());
        }
        let mut ordered = Vec::with_capacity(chains.len());
        for chain in chains.iter() {
            let mut symbols = Vec::with_capacity(chain.len());
            for symbol in chain {
                let property = self.register_reference_symbol(*symbol, consumer)?;
                symbols.push(self.property_native_symbol(property, consumer)?);
            }
            ordered.push((symbols, chain.clone()));
        }
        crate::project::binding::try_sort_by(&mut ordered, &|left,
                                                             right|
         -> Result<
            std::cmp::Ordering,
            DemandError,
        > {
            let length = left.0.len().cmp(&right.0.len());
            if length != std::cmp::Ordering::Equal {
                return Ok(length);
            }
            for (left, right) in left.0.iter().zip(&right.0) {
                let order = self
                    .model
                    .compare_native_symbols(Some(left), Some(right))
                    .map_err(|error| self.generic_binding_failure(error, consumer))?;
                if order != std::cmp::Ordering::Equal {
                    return Ok(order);
                }
            }
            Ok(std::cmp::Ordering::Equal)
        })?;
        for (slot, (_, chain)) in chains.iter_mut().zip(ordered) {
            *slot = chain;
        }
        Ok(())
    }
}

impl Evaluator<'_, '_> {
    /// `getFileSymbolIfFileSymbolExportEqualsContainer` observes only the
    /// container's own export= slot; export-star expansion is another question.
    pub(super) fn file_symbol_if_export_equals_container_completion(
        &mut self,
        declaration: ReferenceDeclaration,
        container: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(file_symbol) =
                sequence.read(self.external_module_container_completion(declaration))?
            else {
                return Ok(None);
            };
            let Some(exported) =
                self.reference_own_module_export(file_symbol, "export=", consumer)?
            else {
                return Ok(None);
            };
            Ok(sequence
                .read(self.get_symbol_if_same_reference_completion(exported, container, consumer))?
                .map(|_| file_symbol))
        })();
        sequence.finish(result)
    }
}
