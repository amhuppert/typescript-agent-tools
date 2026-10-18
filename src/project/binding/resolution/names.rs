//! `resolveNameHelper` keeps scope, default-name and lookup-meaning policy here.
//! The evaluator owns current symbols and the acquisitions made by getSymbol.

use super::{NameTable, ScopeId, flags};
use crate::project::binding::native_inputs::NativeInputs;
use crate::project::binding::semantic::NativeSelectionFailure;
use crate::project::binding::{
    BoundSymbol, NativeLookupMeaning, NativePropertyKey, NativeRawSymbolTable, NativeSymbolFlags,
    TypeBindingFailure,
};
use crate::project::sources::FileId;
use crate::syntax_kind::SyntaxKind;

/// Raw current-header observations shared by checker name resolution and the
/// load-only lexical precheck. Declaration handles must be copied without
/// attaching syntax or projecting every declaration's location.
pub(in crate::project) trait ScopeSymbols {
    type Symbol: Copy;
    type Table: Copy;
    type Declaration: Copy;
    type Error;

    fn control(&self) -> Option<&crate::resources::Control>;
    fn lookup(
        &mut self,
        table: Self::Table,
        key: &NativePropertyKey,
    ) -> Result<Option<Self::Symbol>, Self::Error>;
    fn flags(&mut self, symbol: Self::Symbol) -> Result<NativeSymbolFlags, Self::Error>;
    fn transient(&mut self, symbol: Self::Symbol) -> Result<bool, Self::Error>;
    fn declarations(&mut self, symbol: Self::Symbol)
    -> Result<Vec<Self::Declaration>, Self::Error>;
    fn declaration_kind(
        &mut self,
        declaration: Self::Declaration,
    ) -> Result<SyntaxKind, Self::Error>;
    fn declaration_is_default(
        &mut self,
        declaration: Self::Declaration,
    ) -> Result<bool, Self::Error>;
    fn declaration_local(
        &mut self,
        declaration: Self::Declaration,
    ) -> Result<Option<Self::Symbol>, Self::Error>;
    fn name_matches(&mut self, symbol: Self::Symbol, name: &str) -> Result<bool, Self::Error>;
}

/// A reference's selected symbol, if any, with the failure that explains an
/// absent one; the operation's own failure stays separate.
type ReferenceNameSelection<O> = Result<
    (Option<<O as ScopeSymbols>::Symbol>, TypeBindingFailure),
    NativeSelectionFailure<<O as ScopeSymbols>::Error>,
>;

/// A single mutable adapter carries evaluator state and its NativeSequence.
/// No callback attempts to mutate an evaluator held through another shared
/// borrow. The load-only lexical reader implements ScopeSymbols alone.
pub(in crate::project) trait Names: ScopeSymbols {
    fn bound(&mut self, symbol: BoundSymbol) -> Self::Symbol;
    fn retained_table(&mut self, table: NativeRawSymbolTable) -> Self::Table;
    fn globals(&mut self) -> Result<Self::Table, Self::Error>;
    fn merged(&mut self, symbol: Self::Symbol) -> Result<Self::Symbol, Self::Error>;
    fn table(
        &mut self,
        symbol: Self::Symbol,
        exports: bool,
    ) -> Result<Option<Self::Table>, Self::Error>;
    fn select(
        &mut self,
        symbol: Self::Symbol,
        meaning: NativeLookupMeaning,
        direct: bool,
    ) -> Result<Option<Self::Symbol>, Self::Error>;
    fn arguments(&mut self) -> Result<Self::Symbol, Self::Error>;
}

pub(in crate::project) enum ScopeExportLookup<S, T> {
    NamedDefault(S),
    Table(Option<T>),
    Skip,
}

/// Binder `GetLocalSymbolForExportDefault`: only the first declaration's
/// syntactic Default modifier opens the scan for the first LocalSymbol.
fn local_symbol_for_export_default<O: ScopeSymbols>(
    operations: &mut O,
    symbol: O::Symbol,
) -> Result<Option<O::Symbol>, O::Error> {
    let declarations = operations.declarations(symbol)?;
    let Some(&first) = declarations.first() else {
        return Ok(None);
    };
    if !operations.declaration_is_default(first)? {
        return Ok(None);
    }
    for declaration in declarations {
        if let Some(local) = operations.declaration_local(declaration)? {
            return Ok(Some(local));
        }
    }
    Ok(None)
}

/// `resolveNameHelper`'s PURE export-alias gate. Native Transient is part of
/// Symbol.Flags; the local representation stores it separately. Mixed Alias
/// plus another meaning and transient aliases remain eligible.
fn pure_export_specifier<O: ScopeSymbols>(
    operations: &mut O,
    symbol: O::Symbol,
) -> Result<bool, O::Error> {
    if operations.flags(symbol)?.bits != flags::ALIAS || operations.transient(symbol)? {
        return Ok(false);
    }
    let declarations = operations.declarations(symbol)?;
    // Native GetDeclarationOfKind(ExportSpecifier) precedes the second scan.
    for &declaration in &declarations {
        if operations.declaration_kind(declaration)? == SyntaxKind::ExportSpecifier {
            return Ok(true);
        }
    }
    for declaration in declarations {
        if operations.declaration_kind(declaration)? == SyntaxKind::NamespaceExport {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Complete raw module-exports precheck from `resolveNameHelper`: named
/// default uses the original meaning; pure aliases are checked without
/// getSymbol; reserved `default` is excluded before the ordinary table probe.
pub(in crate::project) fn scope_export_lookup<O: ScopeSymbols>(
    operations: &mut O,
    table: Option<O::Table>,
    external: bool,
    name: &str,
    meaning: NativeLookupMeaning,
) -> Result<ScopeExportLookup<O::Symbol, O::Table>, O::Error> {
    if external && let Some(table) = table {
        if let Some(exported) = operations.lookup(
            table,
            &NativePropertyKey::fixed(b"default".as_slice(), operations.control()),
        )? {
            if let Some(local) = local_symbol_for_export_default(operations, exported)? {
                if meaning.matches(operations.flags(exported)?)
                    && operations.name_matches(local, name)?
                {
                    return Ok(ScopeExportLookup::NamedDefault(exported));
                }
            }
        }
        if let Some(exported) = operations.lookup(
            table,
            &NativePropertyKey::fixed(name.as_bytes(), operations.control()),
        )? && pure_export_specifier(operations, exported)?
        {
            return Ok(ScopeExportLookup::Skip);
        }
    }
    Ok(if name == "default" {
        ScopeExportLookup::Skip
    } else {
        ScopeExportLookup::Table(table)
    })
}

impl NativeInputs<'_> {
    pub(in crate::project) fn select_entity_head<O: Names>(
        self,
        operations: &mut O,
        recipe: &super::super::semantic::EntityNameRecipe,
        meaning: NativeLookupMeaning,
    ) -> Result<Option<O::Symbol>, NativeSelectionFailure<O::Error>> {
        self.name(
            operations,
            recipe.file,
            recipe.scope,
            &recipe.head.name,
            meaning,
        )
    }

    pub(in crate::project) fn select_name_at_node<O: Names>(
        self,
        operations: &mut O,
        file: FileId,
        kind: SyntaxKind,
        span: crate::Span,
        name: &str,
        meaning: NativeLookupMeaning,
    ) -> Result<Option<O::Symbol>, NativeSelectionFailure<O::Error>> {
        let scope = self
            .files
            .file(file)
            .lookup_scopes()
            .get((kind, span))
            .ok_or(NativeSelectionFailure::Binding(
                TypeBindingFailure::NotRetained,
            ))?;
        self.name(operations, file, scope, name, meaning)
    }

    pub(in crate::project) fn select_global_name<O: Names>(
        self,
        operations: &mut O,
        name: &str,
        meaning: NativeLookupMeaning,
    ) -> Result<Option<O::Symbol>, NativeSelectionFailure<O::Error>> {
        use NativeSelectionFailure::Selection;
        let table = operations.globals().map_err(Selection)?;
        let symbol = operations
            .lookup(
                table,
                &NativePropertyKey::fixed(name.as_bytes(), operations.control()),
            )
            .map_err(Selection)?;
        match symbol {
            Some(symbol) => operations.select(symbol, meaning, false).map_err(Selection),
            None => Ok(None),
        }
    }

    pub(in crate::project) fn select_reference_name<O: Names>(
        self,
        operations: &mut O,
        file: FileId,
        anchor: crate::Span,
        meaning: NativeLookupMeaning,
    ) -> ReferenceNameSelection<O> {
        let index = self
            .files
            .lexical_reference_index(file, anchor, None)
            .map_err(NativeSelectionFailure::Binding)?;
        let reference = self.files.reference(super::RefRef {
            file,
            id: super::super::ids::RefId::new(index),
        });
        let super::super::collect::RefBase::Lexical(scope) = reference.base else {
            unreachable!("lexical_reference_index selected a lexical reference");
        };
        let symbol = self.name(operations, file, scope, reference.name, meaning)?;
        Ok((symbol, self.missing_reference(file, index)))
    }

    /// `reportImplicitAny` starts from the actual parameter Name scope. It
    /// never substitutes the parameter's value/declared type for this lookup.
    pub(in crate::project) fn select_parameter_type_name<O: Names>(
        self,
        operations: &mut O,
        file: FileId,
        native_span: crate::Span,
    ) -> Result<Option<O::Symbol>, NativeSelectionFailure<O::Error>> {
        use NativeSelectionFailure::Binding;
        let facts = self.files.file(file);
        let mut declarations = facts.decls().iter().filter(|declaration| {
            declaration.native_kind == SyntaxKind::Parameter
                && declaration.native_span == Some(native_span)
        });
        let declaration = declarations
            .next()
            .ok_or(Binding(TypeBindingFailure::NotRetained))?;
        if declarations.next().is_some() {
            return Err(Binding(TypeBindingFailure::NotRetained));
        }
        let reference = declaration
            .name_reference
            .ok_or(Binding(TypeBindingFailure::NotRetained))?;
        let reference = self.files.reference(super::RefRef {
            file,
            id: reference,
        });
        let super::super::collect::RefBase::Lexical(scope) = reference.base else {
            return Err(Binding(TypeBindingFailure::NotRetained));
        };
        self.name(
            operations,
            file,
            scope,
            &declaration.name,
            NativeLookupMeaning::TYPE,
        )
    }

    /// `resolveNameHelper` for captured lexical scopes. Rejected candidates
    /// resume the same walk. Scope recipes and Host never leave binding.
    fn name<O: Names>(
        self,
        operations: &mut O,
        file: FileId,
        scope: ScopeId,
        name: &str,
        meaning: NativeLookupMeaning,
    ) -> Result<Option<O::Symbol>, NativeSelectionFailure<O::Error>> {
        use NativeSelectionFailure::{Binding, Selection};
        let key = NativePropertyKey::fixed(name.as_bytes(), operations.control());
        for index in self.files.lookup_scope_chain(file, scope) {
            let plan = self.files.name_scope(file, index);
            if let Some(declaration) = plan.infer
                && *self.files.file(file).decls()[declaration.index()].name == *name
                && meaning.0 & flags::TYPE_PARAMETER != 0
            {
                let raw = self
                    .declaration_header(file, declaration)
                    .ok_or(Binding(TypeBindingFailure::NotRetained))?;
                let symbol = operations.bound(BoundSymbol(raw));
                let symbol = operations.merged(symbol).map_err(Selection)?;
                return operations.select(symbol, meaning, true).map_err(Selection);
            }
            if let Some(declaration) = plan.enum_
                && meaning.0 & flags::ENUM_MEMBER != 0
            {
                let raw = self
                    .declaration_header(file, declaration)
                    .ok_or(Binding(TypeBindingFailure::NotRetained))?;
                let symbol = operations.bound(BoundSymbol(raw));
                let symbol = operations.merged(symbol).map_err(Selection)?;
                if let Some(table) = operations.table(symbol, true).map_err(Selection)?
                    && let Some(symbol) = operations.lookup(table, &key).map_err(Selection)?
                    && let Some(symbol) = operations
                        .select(
                            symbol,
                            NativeLookupMeaning(meaning.0 & flags::ENUM_MEMBER),
                            false,
                        )
                        .map_err(Selection)?
                {
                    return Ok(Some(symbol));
                }
            }
            for recipe in plan.tables {
                let table = match recipe {
                    NameTable::Global => Some(operations.globals().map_err(Selection)?),
                    NameTable::Locals(scope) => self.raw.scope_tables[file.index()][scope.index()]
                        .map(|table| operations.retained_table(NativeRawSymbolTable(table))),
                    NameTable::Members(declaration) => {
                        let raw = self
                            .declaration_header(file, declaration)
                            .ok_or(Binding(TypeBindingFailure::NotRetained))?;
                        let symbol = operations.bound(BoundSymbol(raw));
                        let symbol = operations.merged(symbol).map_err(Selection)?;
                        operations.table(symbol, false).map_err(Selection)?
                    }
                    NameTable::Exports { host, external } => {
                        let raw = self
                            .raw_host(file, host)
                            .ok_or(Binding(TypeBindingFailure::NotRetained))?;
                        let symbol = operations.bound(BoundSymbol(raw));
                        let symbol = operations.merged(symbol).map_err(Selection)?;
                        let table = operations.table(symbol, true).map_err(Selection)?;
                        match scope_export_lookup(operations, table, external, name, meaning)
                            .map_err(Selection)?
                        {
                            ScopeExportLookup::NamedDefault(symbol) => {
                                return operations.select(symbol, meaning, true).map_err(Selection);
                            }
                            ScopeExportLookup::Table(table) => table,
                            ScopeExportLookup::Skip => continue,
                        }
                    }
                };
                let meaning = NativeLookupMeaning(recipe.lookup_meaning(meaning.0));
                if meaning.0 != 0
                    && let Some(table) = table
                    && let Some(symbol) = operations.lookup(table, &key).map_err(Selection)?
                    && let Some(symbol) = operations
                        .select(symbol, meaning, false)
                        .map_err(Selection)?
                {
                    return Ok(Some(symbol));
                }
            }
            if plan.arguments && name == "arguments" && meaning.0 & flags::FUNCTION_SCOPED != 0 {
                return operations.arguments().map(Some).map_err(Selection);
            }
        }
        Ok(None)
    }
}
