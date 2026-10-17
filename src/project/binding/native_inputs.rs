//! Immutable native binder inputs shared by loading and semantic requests.
//!
//! This owner exposes captured node/header/table associations only. It has no
//! alias, merge, resolved-export, type or global cache; the evaluator owns all
//! observations of current checker state.

use super::collect::{DeclKind, RefBase};
#[cfg(test)]
use super::ids::NativeTableId;
use super::ids::{DeclId, NativeHeaderId};
use super::inputs::BindingFiles;
use super::link::flags;
use super::member::MemberKind;
use super::raw::RawBindingSnapshot;
use super::semantic::*;
use super::store::AliasRef;
use crate::{Span, project::sources::FileId, syntax_kind::SyntaxKind};

#[derive(Clone, Copy)]
pub(in crate::project) struct NativeInputs<'p> {
    pub(super) files: BindingFiles<'p>,
    pub(super) raw: &'p RawBindingSnapshot,
    // Only the ephemeral view borrows the current request capability.
}

/// A raw declaration-list entry; copying it observes no syntax projection.
#[derive(Clone, Copy)]
pub(in crate::project) struct RawDeclaration(pub(super) super::store::SymbolDeclaration);

impl RawDeclaration {
    pub(super) fn kind(self, files: BindingFiles<'_>) -> SyntaxKind {
        use super::store::SymbolDeclaration;
        match self.0 {
            SymbolDeclaration::Bound(file, id) => files.file(file).decls[id.index()].native_kind,
            SymbolDeclaration::Alias(alias) => {
                files.file(alias.file).aliases[alias.id.index()].native_kind
            }
            SymbolDeclaration::ExportStar { .. } => SyntaxKind::ExportDeclaration,
            SymbolDeclaration::Module { .. } => SyntaxKind::ModuleDeclaration,
            SymbolDeclaration::SourceFile { .. } => SyntaxKind::SourceFile,
            SymbolDeclaration::Json { kind, .. } => kind,
        }
    }

    pub(super) fn is_default(self, files: BindingFiles<'_>) -> bool {
        match self.0 {
            super::store::SymbolDeclaration::Bound(file, id) => files.file(file).decls[id.index()]
                .export
                .as_ref()
                .is_some_and(|(_, name)| name == "default"),
            _ => false,
        }
    }
}

impl<'p> NativeInputs<'p> {
    /// `getMembersOfSymbol` / `getExportsOfSymbol`: table observations keep
    /// the late-binding dependency and actual nil table in one retained owner.
    #[cfg(test)]
    fn native_symbol_table(
        &self,
        symbol: BoundSymbol,
        exports: bool,
    ) -> Result<Option<NativeTableId>, TypeBindingFailure> {
        let data = self.raw.native.header(symbol.0);
        if !data.late_members.is_empty() {
            return Err(TypeBindingFailure::Pending(
                "native late member table resolution is pending",
            ));
        }
        Ok(if exports { data.exports } else { data.members })
    }

    /// `getMembersOfSymbol` before the TypeLiteral factory's empty branch.
    /// This asks table presence, not getNamedMembers/filtering/display names.
    /// Late binding remains an actual required semantic question.
    #[cfg(test)]
    pub(in crate::project) fn type_literal_members_empty(
        &self,
        bound: BoundTypeLiteral,
    ) -> Result<bool, TypeBindingFailure> {
        self.members_empty(bound.symbol)
    }

    /// `getMembersOfSymbol` table presence, including reserved signatures.
    /// This is shared by TypeLiteral emptiness and nonaugmenting base checks;
    /// an uncomputed late table cannot be treated as an empty inventory.
    #[cfg(test)]
    pub(in crate::project) fn members_empty(
        &self,
        symbol: BoundSymbol,
    ) -> Result<bool, TypeBindingFailure> {
        match self.native_symbol_table(symbol, false)? {
            Some(table) => Ok(self.raw.native.entries(table)?.is_empty()),
            None => Ok(true),
        }
    }

    /// `getMembersOfSymbol`: retain the own named table inventory. The
    /// evaluator's setStructuredTypeMembers/getNamedMembers operation owns
    /// declaration containment and sorting at the actual demand point.
    #[cfg(test)]
    pub(in crate::project) fn named_members(
        &self,
        owner: BoundSymbol,
    ) -> Result<Vec<BoundNamedMember>, TypeBindingFailure> {
        self.named_symbol_members(owner, false)
    }

    #[cfg(test)]
    fn named_symbol_members(
        &self,
        owner: BoundSymbol,
        exports: bool,
    ) -> Result<Vec<BoundNamedMember>, TypeBindingFailure> {
        let Some(table) = self.native_symbol_table(owner, exports)? else {
            return Ok(Vec::new());
        };
        let native = &self.raw.native;
        let mut result = Vec::new();
        for (key, &symbol) in native.entries(table)? {
            if key.is_reserved_member_name() {
                continue;
            }
            let flags = native.header(symbol).flags;
            if flags & flags::VALUE != 0 {
                result.push(BoundNamedMember {
                    symbol: BoundSymbol(symbol),
                });
            } else if flags & flags::ALIAS != 0 {
                return Err(TypeBindingFailure::Pending(
                    "native named alias member requires symbolIsValueEx acquisition",
                ));
            }
        }
        Ok(result)
    }

    /// Native reserved signature symbols retain their own declaration list;
    /// they are not string-key named members and cannot collide with a user
    /// property spelled like a compiler internal signature name.
    #[cfg(test)]
    pub(in crate::project) fn signature_declarations(
        &self,
        owner: BoundSymbol,
        kind: MemberKind,
    ) -> Result<Vec<BoundMemberDeclaration>, TypeBindingFailure> {
        let Some(symbol) = self.signature_symbol(owner, kind)? else {
            return Ok(Vec::new());
        };
        Ok(self
            .own_declarations(symbol)
            .filter(|(role, _)| *role == DeclKind::Member(kind))
            .map(|(_, declaration)| BoundMemberDeclaration {
                native_kind: self.files.file(declaration.file).decls[declaration.id.index()]
                    .native_kind,
                declaration,
            })
            .collect())
    }

    /// Actual reserved `symbol.Members` entry for getSignaturesOfSymbol.
    #[cfg(test)]
    pub(in crate::project) fn signature_symbol(
        &self,
        owner: BoundSymbol,
        kind: MemberKind,
    ) -> Result<Option<BoundSymbol>, TypeBindingFailure> {
        let key: &[u8] = match kind {
            MemberKind::Call => b"\xfecall",
            MemberKind::Construct => b"\xfenew",
            MemberKind::Constructor => b"\xfeconstructor",
            MemberKind::Index => b"\xfeindex",
            _ => unreachable!("only reserved signature roles have a signature-table entry"),
        };
        let Some(table) = self.raw.native.header(owner.0).members else {
            return Ok(None);
        };
        Ok(self
            .raw
            .native
            .lookup(table, &super::NativePropertyKey::fixed(key, None))?
            .map(BoundSymbol))
    }

    /// `getSymbolOfDeclaration` for the exact early member role. An ordinary
    /// lexical parameter at the same anchor is a different native association.
    #[cfg(test)]
    pub(in crate::project) fn member_symbol(
        &self,
        file: FileId,
        anchor: Span,
    ) -> Result<BoundSymbol, TypeBindingFailure> {
        let mut symbols = self
            .files
            .file(file)
            .decls
            .iter()
            .enumerate()
            .filter(|(_, declaration)| {
                matches!(declaration.kind, DeclKind::Member(_)) && declaration.anchor == anchor
            })
            .map(|(index, _)| {
                self.declaration_header(file, DeclId::new(index))
                    .map(BoundSymbol)
                    .ok_or(TypeBindingFailure::NotRetained)
            });
        let first = symbols.next().ok_or(TypeBindingFailure::NotRetained)??;
        for next in symbols {
            if next? != first {
                return Err(TypeBindingFailure::Pending(
                    "member anchor has distinct native roles",
                ));
            }
        }
        Ok(first)
    }

    pub(in crate::project) fn new(files: BindingFiles<'p>, raw: &'p RawBindingSnapshot) -> Self {
        Self { files, raw }
    }

    pub(super) fn declaration_header(
        &self,
        file: FileId,
        declaration: DeclId,
    ) -> Option<NativeHeaderId> {
        self.raw
            .native
            .declaration_symbol(super::store::SymbolDeclaration::Bound(file, declaration))
    }
}

impl<'p> NativeInputs<'p> {
    pub(super) fn raw_host(
        &self,
        file: FileId,
        host: super::collect::Host,
    ) -> Option<NativeHeaderId> {
        use super::collect::Host;
        match host {
            Host::File => self.raw.file_modules[file.index()],
            Host::AmbientModule(index) => Some(self.raw.ambient_hosts[file.index()][index.index()]),
            Host::Augmentation(request) => self.raw.augmentations[file.index()][request.index()],
            Host::Namespace(declaration) => self.declaration_header(file, declaration),
            Host::Global => self.raw.global_augmentations[file.index()],
        }
    }

    /// `Symbol.Declarations`: borrow the immutable raw header's exact ordered
    /// nodes. Semantic attachment/fact validation belongs to its later owner.
    pub(in crate::project) fn raw_symbol_declarations(
        &self,
        symbol: BoundSymbol,
    ) -> impl ExactSizeIterator<Item = RawDeclaration> + 'p + use<'p> {
        self.raw
            .native
            .header(symbol.0)
            .declarations
            .iter()
            .copied()
            .map(RawDeclaration)
    }

    pub(in crate::project) fn raw_declaration_kind(
        &self,
        declaration: RawDeclaration,
    ) -> SyntaxKind {
        declaration.kind(self.files)
    }

    pub(in crate::project) fn raw_declaration_default(&self, declaration: RawDeclaration) -> bool {
        declaration.is_default(self.files)
    }

    pub(in crate::project) fn raw_declaration_local(
        &self,
        declaration: RawDeclaration,
    ) -> Option<BoundSymbol> {
        match declaration.0 {
            super::store::SymbolDeclaration::Bound(file, id) => {
                self.raw.declaration_locals[file.index()][id.index()].map(BoundSymbol)
            }
            _ => None,
        }
    }

    /// A raw header carries all native meanings; Alias is only an operation
    /// dispatch tag and never a declaration recipe or a second identity.
    /// Reacquiring this handle does not follow aliases or the checker merge map.
    pub(in crate::project) fn read_origin(&self, header: NativeHeaderId) -> ReadOrigin {
        if self.raw.native.header(header).flags & flags::ALIAS != 0 {
            ReadOrigin::Alias(BoundAliasSymbol(header))
        } else {
            ReadOrigin::Direct(BoundSymbol(header))
        }
    }

    /// `getInferTypeParameters` consumes the conditional node's actual Locals
    /// table. Repeated infer declarations already share the binder's symbol.
    pub(in crate::project) fn conditional_parameters(
        &self,
        file: FileId,
        span: Span,
    ) -> Result<Vec<BoundSymbol>, TypeBindingFailure> {
        let scope = self
            .files
            .file(file)
            .conditional_scopes
            .get(&span)
            .ok_or(TypeBindingFailure::NotRetained)?;
        let Some(table) = self.raw.scope_tables[file.index()][scope.index()] else {
            return Ok(Vec::new());
        };
        let native = &self.raw.native;
        Ok(native
            .entries(table)?
            .values()
            .copied()
            .filter(|&header| native.header(header).flags & flags::TYPE_PARAMETER != 0)
            .map(BoundSymbol)
            .collect())
    }
    pub(in crate::project) fn native_symbol_flags(&self, symbol: BoundSymbol) -> NativeSymbolFlags {
        let header = self.raw.native.header(symbol.0);
        NativeSymbolFlags {
            bits: header.flags,
            assignment: header.assignment,
            // ReplaceableByMethod belongs to the excluded expando-assignment producer.
            replaceable_by_method: false,
        }
    }

    /// Actual independent Symbol.Flags bits, including declarations merged
    /// into a binder prototype after its creation.
    pub(in crate::project) fn native_property_flags(
        &self,
        symbol: BoundSymbol,
    ) -> Result<NativePropertyFlags, TypeBindingFailure> {
        Ok(self.native_symbol_flags(symbol).property_flags())
    }

    /// `newSymbol` and `mergeSymbol` produce transient checker headers.
    /// Ordinary binder headers have no transient bit.
    pub(in crate::project) fn native_symbol_is_transient(&self, symbol: BoundSymbol) -> bool {
        self.raw.native.header(symbol.0).transient
    }

    pub(in crate::project) fn native_symbol_has_assignment_flag(
        &self,
        symbol: BoundSymbol,
    ) -> bool {
        self.raw.native.header(symbol.0).assignment
    }

    /// `isObjectTypeWithInferableIndex` and other Type.symbol consumers
    /// observe actual retained Symbol.Flags, never printed type kind.
    pub(in crate::project) fn type_symbol_flags(
        &self,
        symbol: BoundSymbol,
    ) -> Result<TypeSymbolFlags, TypeBindingFailure> {
        Ok(self.native_symbol_flags(symbol).type_flags())
    }

    /// `getTypeOfSymbol` selects Accessor, Variable|Property, Function|Method,
    /// Class|Enum|Module, then EnumMember on the actual combined native flags.
    pub(in crate::project) fn value_type_symbol(
        &self,
        symbol: BoundSymbol,
    ) -> Result<BoundSymbol, TypeBindingFailure> {
        Ok(symbol)
    }

    /// `IsExternalModuleSymbol` observes the raw parent's Module flags and
    /// actual symbol name, before any canonical parent type acquisition.
    pub(in crate::project) fn parent_is_external_module(
        &self,
        parent: NativePropertyParent,
    ) -> Result<bool, TypeBindingFailure> {
        let header = self.raw.native.header(parent.0);
        if header.flags & flags::NAMESPACE == 0 {
            return Ok(false);
        }
        let name = header
            .name
            .as_ref()
            .ok_or(TypeBindingFailure::NotRetained)?
            .bytes()?;
        Ok(name.first() == Some(&b'"'))
    }

    /// `isSourceFileWithGlobalExports` observes the actual source-file
    /// module factory and its retained `bindNamespaceExportDeclaration`
    /// results, not whether the file happened to export an ordinary member.
    pub(in crate::project) fn parent_has_source_file_global_exports(
        &self,
        parent: NativePropertyParent,
    ) -> bool {
        self.raw
            .native
            .header(parent.0)
            .declarations
            .iter()
            .any(|declaration| {
                let super::store::SymbolDeclaration::SourceFile { file, .. } = declaration else {
                    return false;
                };
                self.files
                    .file(*file)
                    .aliases
                    .iter()
                    .any(|alias| matches!(alias.bind, super::collect::AliasBind::Global { .. }))
            })
    }

    /// Raw Symbol.Parent preserves its original actual header even after a
    /// checker replacement. This observation does not call getMergedSymbol.
    pub(in crate::project) fn raw_parent_symbol(
        &self,
        parent: NativePropertyParent,
    ) -> Result<BoundSymbol, TypeBindingFailure> {
        Ok(BoundSymbol(parent.0))
    }

    /// An Alias's own Parent comes from the table to which its binder role
    /// declared it. Lexical imports have real nil parents; exported and UMD
    /// aliases retain the raw host before checker canonicalization.
    pub(in crate::project) fn read_origin_parent(
        &self,
        origin: ReadOrigin,
    ) -> Result<Option<NativePropertyParent>, TypeBindingFailure> {
        Ok(self
            .raw
            .native
            .header(origin.header())
            .parent
            .map(NativePropertyParent))
    }
    /// `getTypeFromTypeLiteralOrFunctionOrConstructorTypeNode` consumes its
    /// node.Symbol association directly; the anonymous symbol has no lexical
    /// name to resolve. Exact native kind distinguishes this own role.
    pub(in crate::project) fn type_literal(
        &self,
        file: FileId,
        anchor: Span,
    ) -> Result<BoundTypeLiteral, TypeBindingFailure> {
        let mut declarations =
            self.files
                .file(file)
                .decls
                .iter()
                .enumerate()
                .filter(|(_, declaration)| {
                    declaration.kind == DeclKind::TypeLiteral && declaration.anchor == anchor
                });
        let (index, _) = declarations.next().ok_or(TypeBindingFailure::NotRetained)?;
        if declarations.next().is_some() {
            return Err(TypeBindingFailure::Pending(
                "type literal has more than one retained native association",
            ));
        }
        let symbol = self
            .declaration_header(file, DeclId::new(index))
            .map(BoundSymbol)
            .ok_or(TypeBindingFailure::NotRetained)?;
        Ok(BoundTypeLiteral {
            symbol,
            declaration: self.bound_declaration(file, DeclId::new(index)),
        })
    }

    /// `maps.Clone(symbol.Members/Exports)` in cloneTypeAsModuleType copies
    /// raw tables, not getResolvedMembersOrExportsOfSymbol's computed table.
    pub(in crate::project) fn raw_symbol_table(
        &self,
        symbol: BoundSymbol,
        exports: bool,
    ) -> Option<NativeRawSymbolTable> {
        let data = self.raw.native.header(symbol.0);
        (if exports { data.exports } else { data.members }).map(NativeRawSymbolTable)
    }

    /// Checker `maps.Clone` copies raw table storage, preserving failed keys
    /// without observing a table inventory.
    pub(in crate::project) fn reference_raw_table_copy<T: Copy>(
        &self,
        table: NativeRawSymbolTable,
        mut symbol: impl FnMut(ReadOrigin) -> T,
    ) -> Result<super::NativeSymbolTable<T>, TypeBindingFailure> {
        self.raw
            .native
            .checker_table_copy(table.0, |header| symbol(self.read_origin(header)))
    }

    /// `getResolvedSymbol(...).Declarations` keeps the raw alias's actual
    /// declarations. It neither follows the alias nor acquires a value type.
    pub(in crate::project) fn reference_symbol_declarations(
        &self,
        symbol: ReferenceSymbol,
    ) -> Result<Vec<NativeSymbolDeclaration>, TypeBindingFailure> {
        let ReferenceSymbol::Known(origin) = symbol;
        // Retain the complete validated fact array before any caller attaches
        // syntax. Lazily interleaving those phases would change failure order.
        (self
            .raw
            .native
            .header(origin.header())
            .declarations
            .iter()
            .copied()
            .map(|declaration| self.native_declaration_facts(declaration)))
        .collect::<Result<Vec<_>, _>>()
    }

    fn native_declaration_facts(
        &self,
        declaration: super::store::SymbolDeclaration,
    ) -> Result<NativeSymbolDeclaration, TypeBindingFailure> {
        use super::store::SymbolDeclaration;
        Ok(match declaration {
            SymbolDeclaration::Bound(file, id) => {
                let data = &self.files.file(file).decls[id.index()];
                NativeSymbolDeclaration {
                    json: None,
                    file,
                    kind: data.native_kind,
                    span: data.native_span.ok_or(TypeBindingFailure::NotRetained)?,
                }
            }
            SymbolDeclaration::Alias(alias) => {
                let data = &self.files.file(alias.file).aliases[alias.id.index()];
                NativeSymbolDeclaration {
                    json: None,
                    file: alias.file,
                    kind: data.native_kind,
                    span: data.native_span,
                }
            }
            SymbolDeclaration::ExportStar { file, span, .. } => NativeSymbolDeclaration {
                json: None,
                file,
                kind: crate::syntax_kind::SyntaxKind::ExportDeclaration,
                span,
            },
            SymbolDeclaration::Module { file, span } => NativeSymbolDeclaration {
                json: None,
                file,
                kind: crate::syntax_kind::SyntaxKind::ModuleDeclaration,
                span,
            },
            SymbolDeclaration::SourceFile { file, span } => NativeSymbolDeclaration {
                json: self
                    .files
                    .file(file)
                    .json
                    .as_ref()
                    .map(|_| super::JsonDeclaration::SourceFile),
                file,
                kind: crate::syntax_kind::SyntaxKind::SourceFile,
                span,
            },
            SymbolDeclaration::Json {
                file,
                declaration,
                kind,
                span,
            } => NativeSymbolDeclaration {
                json: Some(declaration),
                file,
                kind,
                span,
            },
        })
    }

    /// Actual ValueDeclaration facts include factory-owned SourceFiles. The
    /// DeclId-only variable/member recipe projection is a separate operation.
    pub(in crate::project) fn symbol_value_declaration(
        &self,
        symbol: BoundSymbol,
    ) -> Result<Option<NativeSymbolDeclaration>, TypeBindingFailure> {
        self.raw
            .native
            .header(symbol.0)
            .value_declaration
            .map(|value| self.native_declaration_facts(value.declaration))
            .transpose()
    }

    /// Actual own raw binder declaration associations. The checker observes
    /// its replacement declaration list after an explicit current-header read;
    /// lexical representatives never reorder these captured inputs.
    fn own_declarations(
        &self,
        symbol: BoundSymbol,
    ) -> impl Iterator<Item = (DeclKind, BoundDeclaration)> + '_ {
        self.raw
            .native
            .header(symbol.0)
            .declarations
            .iter()
            .inspect(|_| {})
            .filter_map(|declaration| declaration.bound())
            .map(|(file, id)| {
                let declaration = &self.files.file(file).decls[id.index()];
                (
                    declaration.kind,
                    BoundDeclaration {
                        file,
                        id,
                        anchor: declaration.anchor,
                        contains_this: declaration.contains_this,
                    },
                )
            })
    }

    /// Bound entries of native `Symbol.Declarations`, distinct from filtered
    /// property/type-parameter roles and the full current-header declaration list.
    pub(in crate::project) fn declarations(&self, symbol: BoundSymbol) -> Vec<BoundDeclaration> {
        (self
            .own_declarations(symbol)
            .map(|(_, declaration)| declaration))
        .collect::<Vec<_>>()
    }

    /// Bound-declaration projection for declaration-family recipes. Whole
    /// Symbol.Declarations observations use reference_symbol_declarations,
    /// which also retains raw Alias and SourceFile factory entries.
    pub(in crate::project) fn native_declarations(
        &self,
        symbol: BoundSymbol,
    ) -> Result<Vec<BoundDeclaration>, TypeBindingFailure> {
        Ok(self.declarations(symbol))
    }

    /// `getDeclaredTypeOfClassOrInterface` chooses Class from the canonical
    /// symbol flags before asking its complete own declaration/parameter
    /// questions. Namespace attachments remain own associations but do not
    /// become class/interface syntax or a second member inventory.
    pub(in crate::project) fn class_or_interface(
        &self,
        symbol: BoundSymbol,
    ) -> Result<BoundClassOrInterface, TypeBindingFailure> {
        let kind = self
            .class_or_interface_kind(symbol)
            .ok_or(TypeBindingFailure::Pending(
                "canonical declared symbol is not a class or interface",
            ))?;
        let declarations: Vec<_> = self
            .own_declarations(symbol)
            .filter(|(kind, _)| matches!(kind, DeclKind::Class | DeclKind::Interface))
            .map(|(_, declaration)| declaration)
            .collect();
        if declarations.is_empty() {
            return Err(TypeBindingFailure::NotRetained);
        }
        Ok(BoundClassOrInterface {
            kind,
            has_interface: self.symbol_has_interface_flag(symbol),
            declarations,
        })
    }

    /// `isThislessInterface` asks the Interface flag before any declared-type
    /// acquisition. Aliases, enums and other Type meanings are not interfaces.
    pub(in crate::project) fn symbol_has_interface_flag(&self, symbol: BoundSymbol) -> bool {
        self.raw.native.header(symbol.0).flags & flags::INTERFACE != 0
    }

    /// `getDeclaredTypeOfClassOrInterface`: canonical Class flags take
    /// precedence over Interface on a merged symbol. Syntax declaration kinds
    /// and namespace display attachments cannot make that kind decision.
    fn class_or_interface_kind(&self, symbol: BoundSymbol) -> Option<BoundClassKind> {
        let flags = self.raw.native.header(symbol.0).flags;
        if flags & flags::CLASS != 0 {
            Some(BoundClassKind::Class)
        } else if flags & flags::INTERFACE != 0 {
            Some(BoundClassKind::Interface)
        } else {
            None
        }
    }

    pub(in crate::project) fn property_parent(
        &self,
        symbol: BoundSymbol,
    ) -> Option<NativePropertyParent> {
        self.raw
            .native
            .header(symbol.0)
            .parent
            .map(NativePropertyParent)
    }

    /// Native semantic factories assigning Symbol.Parent retain the actual
    /// checker symbol rather than its pre-merge binder parent snapshot.
    pub(in crate::project) fn checker_symbol_parent(
        &self,
        symbol: BoundSymbol,
    ) -> NativePropertyParent {
        NativePropertyParent(symbol.0)
    }

    pub(in crate::project) fn parent_is_class(&self, parent: NativePropertyParent) -> bool {
        self.raw.native.header(parent.0).flags & flags::CLASS != 0
    }

    pub(in crate::project) fn parent_is_class_or_interface(
        &self,
        parent: NativePropertyParent,
    ) -> bool {
        self.raw.native.header(parent.0).flags & (flags::CLASS | flags::INTERFACE) != 0
    }

    /// `firstValueDeclaration.Symbol().Parent` observes the original bound
    /// node's symbol, independently of the merged property's current Parent.
    pub(in crate::project) fn declaration_symbol_parent(
        &self,
        declaration: BoundDeclaration,
    ) -> Result<Option<NativePropertyParent>, TypeBindingFailure> {
        let header = self
            .declaration_header(declaration.file, declaration.id)
            .ok_or(TypeBindingFailure::NotRetained)?;
        Ok(self
            .raw
            .native
            .header(header)
            .parent
            .map(NativePropertyParent))
    }

    /// Retrieve the retained final node.Symbol role for an actual declaration.
    pub(in crate::project) fn native_declaration(
        &self,
        file: FileId,
        kind: crate::syntax_kind::SyntaxKind,
        anchor: Span,
    ) -> Result<BoundDeclaration, TypeBindingFailure> {
        let mut declarations =
            self.files
                .file(file)
                .decls
                .iter()
                .enumerate()
                .filter(|(_, declaration)| {
                    declaration.native_kind == kind
                        && declaration.anchor == anchor
                        && !(matches!(
                            kind,
                            crate::syntax_kind::SyntaxKind::FunctionType
                                | crate::syntax_kind::SyntaxKind::ConstructorType
                        ) && matches!(
                            declaration.kind,
                            DeclKind::Member(MemberKind::Call | MemberKind::Construct)
                        ))
                });
        let (index, _) = declarations.next().ok_or(TypeBindingFailure::NotRetained)?;
        if declarations.next().is_some() {
            return Err(TypeBindingFailure::Pending(
                "declaration has ambiguous native node.Symbol roles",
            ));
        }
        Ok(self.bound_declaration(file, DeclId::new(index)))
    }

    pub(in crate::project) fn property_value_declaration(
        &self,
        symbol: BoundSymbol,
    ) -> Result<Option<BoundDeclaration>, TypeBindingFailure> {
        self.raw
            .native
            .header(symbol.0)
            .value_declaration
            .map(|value| match value.declaration {
                super::store::SymbolDeclaration::Bound(file, id) => {
                    Ok(self.bound_declaration(file, id))
                }
                _ => Err(TypeBindingFailure::Pending(
                    "native non-DeclId ValueDeclaration requires full symbol metadata",
                )),
            })
            .transpose()
    }

    /// The complete captured JSON module attached to a canonical alias target.
    pub(in crate::project) fn json_node_symbol(
        &self,
        file: FileId,
        declaration: super::JsonDeclaration,
    ) -> Result<BoundSymbol, TypeBindingFailure> {
        self.raw
            .json
            .nodes
            .get(&(file, declaration))
            .copied()
            .map(BoundSymbol)
            .ok_or(TypeBindingFailure::NotRetained)
    }

    /// Actual `Symbol.Declarations` node, independent of the declaration's
    /// diagnostic/name anchor. Anonymous classes and declarations with modifiers
    /// retain the same full-span association as every other native node.
    pub(in crate::project) fn native_bound_declaration(
        &self,
        declaration: BoundDeclaration,
    ) -> Result<NativeSymbolDeclaration, TypeBindingFailure> {
        self.native_declaration_facts(super::store::SymbolDeclaration::Bound(
            declaration.file,
            declaration.id,
        ))
    }

    /// Raw binder association of the declaration whose anchor is `anchor`;
    /// tests inspect retained facts through it. No checker merge.
    #[cfg(test)]
    pub(in crate::project) fn declaration_symbol(
        &self,
        file: FileId,
        anchor: Span,
    ) -> Result<BoundSymbol, TypeBindingFailure> {
        self.declaration_symbol_matching(file, false, |declaration| declaration.anchor == anchor)
    }

    /// `getSymbolOfDeclaration` consumes the actual declaration node, not
    /// its name token. An unparenthesized arrow and its first parameter share
    /// that token but retain distinct native syntax kinds and node spans.
    pub(in crate::project) fn native_declaration_symbol(
        &self,
        file: FileId,
        kind: crate::syntax_kind::SyntaxKind,
        span: Span,
    ) -> Result<BoundSymbol, TypeBindingFailure> {
        self.native_declaration_read_origin(file, kind, span)
            .map(|origin| origin.symbol())
    }

    /// `getSymbolOfDeclaration` also returns actual Alias symbols. Alias
    /// declarations retain their own identity even when target resolution has
    /// failed or has never been requested.
    pub(in crate::project) fn native_declaration_read_origin(
        &self,
        file: FileId,
        kind: crate::syntax_kind::SyntaxKind,
        span: Span,
    ) -> Result<ReadOrigin, TypeBindingFailure> {
        self.declaration_read_origin(file, kind, span, false)
    }

    /// Raw `node.Symbol` retains the binder association before
    /// `getMergedSymbol`. Validate that snapshot before metadata consumers
    /// can observe it; the initialized merge target is a different symbol.
    pub(in crate::project) fn raw_declaration_read_origin(
        &self,
        file: FileId,
        kind: crate::syntax_kind::SyntaxKind,
        span: Span,
    ) -> Result<ReadOrigin, TypeBindingFailure> {
        let origin = self.declaration_read_origin(file, kind, span, true)?;
        self.raw_read_origin(origin)
    }

    /// `SourceFile.Symbol` has the same raw snapshot rule as declaration
    /// symbols. Scripts retain actual nil; augmentation merges do not turn a
    /// checker replacement into the source file's original binder header.
    pub(in crate::project) fn raw_file_module_read_origin(
        &self,
        file: FileId,
    ) -> Result<Option<ReadOrigin>, TypeBindingFailure> {
        Ok(self.raw.file_modules[file.index()].map(|header| self.read_origin(header)))
    }

    /// Exact native alias-declaration attachment, shared by node.Symbol and
    /// declaration-owned syntax facts. A merged symbol's last alias declaration
    /// is a different question and cannot supply the selected node's flags.
    fn alias_declaration_at(
        &self,
        file: FileId,
        kind: SyntaxKind,
        span: Span,
    ) -> Result<Option<AliasRef>, TypeBindingFailure> {
        self.files.alias_declaration_at(file, kind, span)
    }

    fn declaration_read_origin(
        &self,
        file: FileId,
        kind: crate::syntax_kind::SyntaxKind,
        span: Span,
        _raw: bool,
    ) -> Result<ReadOrigin, TypeBindingFailure> {
        if let Some(alias) = self.alias_declaration_at(file, kind, span)? {
            let header = self
                .raw
                .native
                .alias_symbol(alias)
                .ok_or(TypeBindingFailure::NotRetained)?;
            return Ok(self.read_origin(header));
        }
        // bindModuleDeclaration publishes ambient, global and augmentation
        // nodes as actual Module attachments. Ordinary namespace declarations
        // have the separate Bound declaration recipe below. Both node.Symbol
        // and getSymbolOfDeclaration observe this same original association.
        if kind == SyntaxKind::ModuleDeclaration
            && let Some(header) = self
                .raw
                .native
                .declaration_symbol(super::store::SymbolDeclaration::Module { file, span })
        {
            return Ok(self.read_origin(header));
        }
        self.declaration_symbol_matching(file, true, |declaration| {
            declaration.native_kind == kind && declaration.native_span == Some(span)
        })
        .map(ReadOrigin::Direct)
    }

    /// Raw `node.Symbol` is not getMergedSymbol(node.Symbol). A retained
    /// binder identity is usable directly only while no checker replacement
    /// has overwritten its metadata; that missing factory is never nil.
    fn raw_read_origin(&self, origin: ReadOrigin) -> Result<ReadOrigin, TypeBindingFailure> {
        Ok(origin)
    }

    /// Actual `Symbol.ExportSymbol`, without merging or alias resolution.
    pub(in crate::project) fn reference_export_symbol(
        &self,
        origin: ReadOrigin,
    ) -> Result<Option<ReadOrigin>, TypeBindingFailure> {
        Ok(self
            .raw
            .native
            .header(origin.header())
            .export_symbol
            .map(|header| self.read_origin(header)))
    }

    /// Actual declaration.LocalSymbol, distinct from declaration.Symbol.
    pub(in crate::project) fn declaration_local_symbol(
        &self,
        file: FileId,
        kind: crate::syntax_kind::SyntaxKind,
        span: Span,
    ) -> Result<Option<ReadOrigin>, TypeBindingFailure> {
        let mut local = None;
        for (index, declaration) in self.files.file(file).decls.iter().enumerate() {
            if declaration.native_kind == kind
                && declaration.native_span == Some(span)
                && let Some(symbol) = self.raw.declaration_locals[file.index()][index]
            {
                if local.is_some_and(|previous| previous != symbol) {
                    return Err(TypeBindingFailure::Pending(
                        "native declaration local-symbol association is ambiguous",
                    ));
                }
                local = Some(symbol);
            }
        }
        Ok(local.map(|symbol| self.read_origin(symbol)))
    }

    /// The raw resolved symbol's `Name`, without following an Alias target.
    pub(in crate::project) fn read_origin_name(
        &self,
        origin: ReadOrigin,
        control: Option<&crate::resources::Control>,
    ) -> Result<Vec<u8>, TypeBindingFailure> {
        self.native_symbol(BoundSymbol(origin.header()))?
            .name(control)
    }

    pub(in crate::project) fn read_origin_key(
        &self,
        origin: ReadOrigin,
    ) -> Result<super::NativePropertyKey, TypeBindingFailure> {
        self.native_symbol_key(BoundSymbol(origin.header()))
    }

    fn declaration_symbol_matching(
        &self,
        file: FileId,
        _raw: bool,
        matches: impl Fn(&super::collect::Decl) -> bool,
    ) -> Result<BoundSymbol, TypeBindingFailure> {
        let declarations = self.files.file(file).decls;
        let mut targets = declarations
            .iter()
            .enumerate()
            .filter(|(_, declaration)| {
                matches(declaration)
                    // `bindFunctionOrConstructorType` overwrites only this
                    // explicit intermediate role with its TypeLiteral symbol.
                    && !(matches!(declaration.native_kind,
                        crate::syntax_kind::SyntaxKind::FunctionType | crate::syntax_kind::SyntaxKind::ConstructorType)
                        && matches!(declaration.kind, DeclKind::Member(MemberKind::Call | MemberKind::Construct)))
                    // bindParameter declares the constructor local first,
                    // then overwrites node.Symbol with the class property.
                    // The local remains in constructor Locals for the separate
                    // parameter-property pair question.
                    && !(declaration.kind == DeclKind::Parameter
                        && declarations.iter().any(|other| {
                            other.native_kind == declaration.native_kind
                                && other.native_span == declaration.native_span
                                && matches!(other.kind, DeclKind::Member(MemberKind::ParameterProperty))
                        }))
            })
            .map(|(index, _)| {
                self.declaration_header(file, DeclId::new(index)).map(BoundSymbol)
                    .ok_or(TypeBindingFailure::NotRetained)
            });
        let first = targets.next().ok_or(TypeBindingFailure::NotRetained)??;
        for target in targets {
            if target? != first {
                return Err(TypeBindingFailure::Pending(
                    "declaration has more than one retained semantic role",
                ));
            }
        }
        Ok(first)
    }

    /// `getDeclaredTypeOfSymbol` acquires the native declared-type family
    /// from canonical symbol flags and its own declarations, not directed
    /// lexical display origins. Class/interface merges ask their one complete
    /// target owner; other merged families retain their separate operation.
    pub(in crate::project) fn declared_type(
        &self,
        symbol: BoundSymbol,
    ) -> Result<BoundDeclaredType, TypeBindingFailure> {
        let flags = self.raw.native.header(symbol.0).flags;
        let declarations: Vec<_> = self.own_declarations(symbol).collect();
        let selected = if flags & (flags::CLASS | flags::INTERFACE) != 0 {
            declarations
                .iter()
                .find(|(kind, _)| matches!(kind, DeclKind::Class | DeclKind::Interface))
                .map(|(_, declaration)| BoundDeclaredType::ClassOrInterface(*declaration))
        } else if flags & flags::TYPE_PARAMETER != 0 {
            declarations
                .iter()
                .find(|(kind, _)| *kind == DeclKind::TypeParameter)
                .map(|(_, declaration)| BoundDeclaredType::Parameter(*declaration))
        } else if flags & flags::TYPE_ALIAS != 0 {
            declarations
                .iter()
                .find(|(kind, _)| *kind == DeclKind::TypeAlias)
                .map(|(_, declaration)| BoundDeclaredType::Alias(*declaration))
        } else if flags & flags::ENUM != 0 {
            declarations
                .iter()
                .find(|(kind, _)| matches!(kind, DeclKind::Enum { .. }))
                .map(|(_, declaration)| BoundDeclaredType::Enum(*declaration))
        } else if flags & flags::ENUM_MEMBER != 0 {
            declarations
                .iter()
                .find(|(kind, _)| *kind == DeclKind::EnumMember)
                .map(|(_, declaration)| BoundDeclaredType::EnumMember(*declaration))
        } else {
            None
        };
        selected.ok_or(TypeBindingFailure::NotRetained)
    }

    /// `getTypeOfSymbol` selects the native flag family on the actual header.
    /// Its ValueDeclaration supplies variable/member syntax; merged namespace
    /// declarations remain on that header without replacing the value family.
    pub(in crate::project) fn value_type(
        &self,
        symbol: BoundSymbol,
    ) -> Result<BoundValueType, TypeBindingFailure> {
        let symbol = self.value_type_symbol(symbol)?;
        let data = self.raw.native.header(symbol.0);
        if data.intrinsic == Some(super::store::IntrinsicSymbol::Undefined) {
            return Ok(BoundValueType::Undefined);
        }
        if data.flags & flags::PROPERTY != 0
            && let Some(value) = self.symbol_value_declaration(symbol)?
            && let Some(declaration) = value.json
        {
            return Ok(BoundValueType::Json {
                file: value.file,
                declaration,
            });
        }
        // getTypeOfSymbol selects Accessor before its variable/property
        // worker, whose first branch handles an ordinary Prototype property.
        if data.flags & flags::PROTOTYPE != 0 && data.flags & (flags::GETTER | flags::SETTER) == 0 {
            let parent = data.parent.ok_or(TypeBindingFailure::NotRetained)?;
            return Ok(BoundValueType::Prototype {
                parent: BoundSymbol(parent),
            });
        }
        if data.flags & (flags::PROPERTY | flags::METHOD | flags::GETTER | flags::SETTER) != 0 {
            let value_declaration = self
                .property_value_declaration(symbol)?
                .ok_or(TypeBindingFailure::NotRetained)?;
            let kind = self
                .native_property_flags(symbol)?
                .value_kind()
                .ok_or(TypeBindingFailure::NotRetained)?;
            return Ok(BoundValueType::Member(BoundMember {
                kind,
                property: data.flags & flags::PROPERTY != 0,
                value_declaration,
                optional: data.flags & flags::OPTIONAL != 0,
            }));
        }
        if data.flags & (flags::FUNCTION_SCOPED | flags::BLOCK_SCOPED) != 0 {
            return self
                .property_value_declaration(symbol)?
                .map(BoundValueType::Variable)
                .ok_or(TypeBindingFailure::NotRetained);
        }
        let declarations: Vec<_> = self.own_declarations(symbol).collect();
        if data.flags & flags::FUNCTION != 0 {
            let declarations: Vec<_> = declarations
                .iter()
                .filter(|(kind, _)| *kind == DeclKind::Function)
                .map(|(_, declaration)| *declaration)
                .collect();
            if declarations.is_empty() {
                return Err(TypeBindingFailure::NotRetained);
            }
            return Ok(BoundValueType::Function(declarations));
        }
        if data.flags & flags::CLASS != 0 {
            return declarations
                .iter()
                .find(|(kind, _)| *kind == DeclKind::Class)
                .map(|(_, declaration)| BoundValueType::Class(*declaration))
                .ok_or(TypeBindingFailure::NotRetained);
        }
        if data.flags & (flags::NAMESPACE | flags::ENUM) != 0 {
            return Ok(BoundValueType::Module);
        }
        if data.flags & flags::ENUM_MEMBER != 0 {
            return self
                .property_value_declaration(symbol)?
                .map(BoundValueType::EnumMember)
                .ok_or(TypeBindingFailure::NotRetained);
        }
        Err(TypeBindingFailure::NotRetained)
    }

    fn bound_declaration(&self, file: FileId, id: DeclId) -> BoundDeclaration {
        let declaration = &self.files.file(file).decls[id.index()];
        BoundDeclaration {
            file,
            id,
            anchor: declaration.anchor,
            contains_this: declaration.contains_this,
        }
    }
}

impl NativeInputs<'_> {
    /// `resolveEntityName` syntax recipe. Scope walking remains in resolveNameHelper;
    /// member selection is supplied by the checker that owns completed tables.
    pub(in crate::project) fn entity_name_recipe(
        &self,
        file: FileId,
        anchor: Span,
        type_reference: bool,
    ) -> Result<EntityNameRecipe, TypeBindingFailure> {
        let index = if type_reference {
            Some(self.files.type_reference_index(file, anchor)?)
        } else {
            self.files
                .references(file)
                .position(|reference| reference.span == anchor)
        };
        let Some(mut index) = index else {
            // Local export targets are alias syntax, not reference tokens.
            // `getTargetOfExportSpecifier` and identifier export assignments
            // still resolve their actual source name in its retained scope.
            let alias = self
                .files
                .file(file)
                .aliases
                .iter()
                .find(|alias| {
                    alias.target_name == Some((SyntaxKind::Identifier, anchor))
                        && matches!(alias.kind, super::collect::AliasKind::Lexical { .. })
                })
                .ok_or(TypeBindingFailure::NotRetained)?;
            let super::collect::AliasKind::Lexical { scope, name } = &alias.kind else {
                return Err(TypeBindingFailure::NotRetained);
            };
            return Ok(EntityNameRecipe {
                file,
                scope: *scope,
                head: EntityNamePart {
                    name: name.clone(),
                    span: anchor,
                    missing: TypeBindingFailure::Unresolved(format!("cannot find `{name}`")),
                },
                members: Vec::new(),
            });
        };
        let mut members = Vec::new();
        loop {
            let reference = self.files.reference(super::resolution::RefRef {
                file,
                id: super::ids::RefId::new(index),
            });
            let part = EntityNamePart {
                name: reference.name.to_owned(),
                span: reference.span,
                missing: self.missing_reference(file, index),
            };
            match reference.base {
                RefBase::Member(base) => {
                    members.push(part);
                    index = base.index();
                }
                RefBase::Lexical(scope) => {
                    members.reverse();
                    return Ok(EntityNameRecipe {
                        file,
                        scope,
                        head: part,
                        members,
                    });
                }
                RefBase::Module(_) => {
                    return Err(TypeBindingFailure::Pending(
                        "native module entity lookup requires its import-type operation",
                    ));
                }
            }
        }
    }
    pub(super) fn missing_reference(&self, file: FileId, index: usize) -> TypeBindingFailure {
        let reference = self.files.reference(super::resolution::RefRef {
            file,
            id: super::ids::RefId::new(index),
        });
        let name = reference.name;
        TypeBindingFailure::Unresolved(match reference.base {
            RefBase::Lexical(_) => format!("cannot find `{name}`"),
            RefBase::Member(_) => format!("namespace has no exported member `{name}`"),
            RefBase::Module(_) => format!("cannot find `{name}`"),
        })
    }
    /// The module request attached to the exact selected alias declaration.
    /// A cloned/combined header's declarations may differ from node.Symbol's
    /// declarations, so this operation never reselects through that symbol.
    pub(in crate::project) fn reference_alias_module_site_at(
        &self,
        file: FileId,
        kind: SyntaxKind,
        span: Span,
    ) -> Result<Option<super::ReferenceModuleSite>, TypeBindingFailure> {
        let alias = self
            .alias_declaration_at(file, kind, span)?
            .ok_or(TypeBindingFailure::NotRetained)?;
        let request = match &self.files.file(file).aliases[alias.id.index()].kind {
            super::collect::AliasKind::Import { request, .. }
            | super::collect::AliasKind::Namespace { request } => *request,
            _ => return Ok(None),
        };
        Ok(Some(super::ReferenceModuleSite { file, request }))
    }
    pub(in crate::project) fn reference_raw_table_entries(
        &self,
        raw: NativeRawSymbolTable,
    ) -> Result<Vec<(super::NativePropertyKey, ReadOrigin)>, TypeBindingFailure> {
        Ok(self
            .raw
            .native
            .entries(raw.0)?
            .iter()
            .map(|(key, &header)| (key.clone(), self.read_origin(header)))
            .collect())
    }
    /// Native raw Members/Exports[key]. This exact keyed read does not
    /// enumerate other entries or acquire alias, late-bound, or merged state.
    pub(in crate::project) fn reference_raw_table_lookup(
        &self,
        raw: NativeRawSymbolTable,
        key: &super::NativePropertyKey,
    ) -> Result<Option<ReadOrigin>, TypeBindingFailure> {
        Ok(self
            .raw
            .native
            .lookup(raw.0, key)?
            .map(|header| self.read_origin(header)))
    }
    /// `Node.Locals`: project the actual captured native owner. Function
    /// signature/body lookup partitions share one native table observation.
    pub(in crate::project) fn reference_raw_locals(
        &self,
        file: FileId,
        kind: SyntaxKind,
        span: Span,
    ) -> Result<Option<Vec<(super::NativePropertyKey, ReadOrigin)>>, TypeBindingFailure> {
        self.reference_raw_locals_table(file, kind, span)?
            .map(|table| self.reference_raw_table_entries(table))
            .transpose()
    }
    /// `Node.Locals()[name]` does not enumerate unrelated entries or turn
    /// an unavailable table into an absent local symbol.
    pub(in crate::project) fn reference_raw_local_lookup(
        &self,
        file: FileId,
        kind: SyntaxKind,
        span: Span,
        key: &super::NativePropertyKey,
    ) -> Result<Option<ReadOrigin>, TypeBindingFailure> {
        match self.reference_raw_locals_table(file, kind, span)? {
            Some(table) => self.reference_raw_table_lookup(table, key),
            None => Ok(None),
        }
    }
    fn reference_raw_locals_table(
        &self,
        file: FileId,
        kind: SyntaxKind,
        span: Span,
    ) -> Result<Option<NativeRawSymbolTable>, TypeBindingFailure> {
        let mut table = None;
        for (index, scope) in self.files.file(file).scopes.iter().enumerate() {
            if scope.native_owner != Some((kind, span)) {
                continue;
            }
            if let Some(projected) = self.raw.scope_tables[file.index()][index] {
                let actual = projected;
                if table.is_some_and(|previous| previous != actual) {
                    return Err(TypeBindingFailure::NotRetained);
                }
                table = Some(actual);
            }
        }
        Ok(table.map(NativeRawSymbolTable))
    }
    /// `checkIdentifier` consumes the captured flow attachment after symbol
    /// selection. Reading this syntax fact does not repeat name resolution.
    pub(in crate::project) fn value_reference_flow(
        &self,
        file: FileId,
        anchor: Span,
        type_context: bool,
    ) -> Result<super::flow::Read, TypeBindingFailure> {
        let index = self
            .files
            .lexical_reference_index(file, anchor, Some(type_context))?;
        self.files
            .reference(super::resolution::RefRef {
                file,
                id: super::ids::RefId::new(index),
            })
            .flow
            .ok_or(TypeBindingFailure::Pending(
                "value reference has no captured native flow attachment",
            ))
    }
}

#[derive(Clone, Copy)]
pub(in crate::project) struct InitializationFile<'p> {
    inputs: NativeInputs<'p>,
    file: &'p super::raw::InitializationFile,
}
impl<'p> InitializationFile<'p> {
    pub(in crate::project) fn file(self) -> FileId {
        self.file.file
    }
    pub(in crate::project) fn script_locals(self) -> Option<NativeRawSymbolTable> {
        self.file.script_locals.map(NativeRawSymbolTable)
    }
    pub(in crate::project) fn global_augmentation(self) -> Option<BoundSymbol> {
        self.file.global_augmentation.map(BoundSymbol)
    }
    pub(in crate::project) fn global_exports(
        self,
    ) -> impl Iterator<Item = Result<(&'p str, BoundSymbol), TypeBindingFailure>> {
        self.file.global_exports.iter().map(move |(name, alias)| {
            let header = self
                .inputs
                .raw
                .native
                .alias_symbol(*alias)
                .ok_or(TypeBindingFailure::NotRetained)?;
            Ok((name.as_str(), BoundSymbol(header)))
        })
    }
}

#[derive(Clone, Copy)]
pub(in crate::project) struct InitializationModule {
    pub(in crate::project) site: super::ReferenceModuleSite,
    pub(in crate::project) symbol: BoundSymbol,
}

#[derive(Clone, Copy)]
pub(in crate::project) struct ModuleSelection<'p> {
    pub(in crate::project) symbol: BoundSymbol,
    pub(in crate::project) pattern_name: Option<&'p str>,
}

impl<'p> NativeInputs<'p> {
    pub(in crate::project) fn initialization_files(
        self,
    ) -> impl Iterator<Item = InitializationFile<'p>> {
        self.raw
            .initialization
            .files
            .iter()
            .map(move |file| InitializationFile { inputs: self, file })
    }
    pub(in crate::project) fn initialization_modules(
        self,
    ) -> impl Iterator<Item = InitializationModule> + 'p {
        self.raw
            .initialization
            .modules
            .iter()
            .map(|module| InitializationModule {
                site: super::ReferenceModuleSite {
                    file: module.file,
                    request: module.request,
                },
                symbol: BoundSymbol(module.source),
            })
    }
    pub(in crate::project) fn pattern_module_symbols(
        self,
    ) -> impl Iterator<Item = BoundSymbol> + 'p {
        self.raw
            .patterns
            .iter()
            .map(|(_, header)| BoundSymbol(*header))
    }
    pub(in crate::project) fn module_selection(
        self,
        site: super::ReferenceModuleSite,
    ) -> Result<ModuleSelection<'p>, TypeBindingFailure> {
        match self.raw.requests[site.file.index()][site.request.index()] {
            super::raw::RawModuleRequest::Module { header, pattern } => Ok(ModuleSelection {
                symbol: BoundSymbol(header),
                pattern_name: pattern.then(|| {
                    self.files.file(site.file).requests[site.request.index()]
                        .specifier
                        .as_str()
                }),
            }),
            super::raw::RawModuleRequest::Script => Err(TypeBindingFailure::Dependency {
                file: site.file,
                span: self.files.file(site.file).requests[site.request.index()].span,
                reason: "imported file is not a module".into(),
            }),
            super::raw::RawModuleRequest::Ignored => Err(TypeBindingFailure::Dependency {
                file: site.file,
                span: self.files.file(site.file).requests[site.request.index()].span,
                reason: "module is not resolved".into(),
            }),
        }
    }
    pub(in crate::project) fn module_selection_ignore_errors(
        self,
        site: super::ReferenceModuleSite,
    ) -> Result<Option<ModuleSelection<'p>>, TypeBindingFailure> {
        match self.raw.requests[site.file.index()][site.request.index()] {
            super::raw::RawModuleRequest::Module { .. } => self.module_selection(site).map(Some),
            super::raw::RawModuleRequest::Script => Ok(None),
            super::raw::RawModuleRequest::Ignored => {
                match &self.files.file(site.file).modules[site.request.index()] {
                    super::ModuleResolution::Missing => Ok(None),
                    super::ModuleResolution::Unchecked => Err(TypeBindingFailure::Pending(
                        "native module resolution request has not been established",
                    )),
                    super::ModuleResolution::NotAdmitted(_)
                    | super::ModuleResolution::Unsupported(_) => Err(TypeBindingFailure::Pending(
                        "module resolution is outside the admitted input profile",
                    )),
                    super::ModuleResolution::File(_) => Err(TypeBindingFailure::NotRetained),
                }
            }
        }
    }
    pub(in crate::project) fn raw_file_module(self, file: FileId) -> Option<BoundSymbol> {
        self.raw.file_modules[file.index()].map(BoundSymbol)
    }
    pub(in crate::project) fn symbol_projection(
        self,
        symbol: BoundSymbol,
    ) -> Option<super::SymbolId> {
        self.raw.native.header(symbol.0).projection
    }
    pub(in crate::project) fn raw_symbol_value_declaration(
        self,
        symbol: BoundSymbol,
    ) -> Option<RawDeclaration> {
        self.raw
            .native
            .header(symbol.0)
            .value_declaration
            .map(|value| RawDeclaration(value.declaration))
    }
    pub(in crate::project) fn raw_declaration_facts(
        self,
        declaration: RawDeclaration,
    ) -> Result<NativeSymbolDeclaration, TypeBindingFailure> {
        self.native_declaration_facts(declaration.0)
    }
}

impl NativeInputs<'_> {
    /// `getDeclarationOfAliasSymbol`/header diagnostics observe latent pointers;
    /// their diagnostic anchor is the binder's existing declaration question.
    pub(in crate::project) fn raw_declaration_location(
        self,
        declaration: RawDeclaration,
    ) -> (FileId, Span) {
        use super::store::SymbolDeclaration;
        match declaration.0 {
            SymbolDeclaration::Bound(file, id) => {
                (file, self.files.file(file).decls[id.index()].anchor)
            }
            SymbolDeclaration::Alias(alias) => (
                alias.file,
                self.files.file(alias.file).aliases[alias.id.index()].span,
            ),
            SymbolDeclaration::ExportStar { file, span, .. }
            | SymbolDeclaration::Module { file, span }
            | SymbolDeclaration::SourceFile { file, span }
            | SymbolDeclaration::Json { file, span, .. } => (file, span),
        }
    }
    /// `IsAmbientModuleSymbolName` checks both quotes on the exact factory name.
    pub(in crate::project) fn raw_symbol_is_ambient_module(
        self,
        symbol: BoundSymbol,
    ) -> Result<bool, TypeBindingFailure> {
        let header = self.raw.native.header(symbol.0);
        if header.flags & flags::NAMESPACE == 0 {
            return Ok(false);
        }
        let name = header
            .name
            .as_ref()
            .ok_or(TypeBindingFailure::NotRetained)?
            .bytes()?;
        Ok(name.first() == Some(&b'"') && name.last() == Some(&b'"'))
    }
    pub(in crate::project) fn native_symbol_has_readonly_check(self, _symbol: BoundSymbol) -> bool {
        false
    }
}

impl<'p> NativeInputs<'p> {
    /// `isShorthandAmbientModuleSymbol` observes the actual first ValueDeclaration; an unrelated namespace declaration cannot substitute for it.
    pub(in crate::project) fn raw_declaration_is_shorthand(
        self,
        declaration: RawDeclaration,
    ) -> bool {
        let super::store::SymbolDeclaration::Module { file, span } = declaration.0 else {
            return false;
        };
        self.files
            .file(file)
            .module_declarations
            .iter()
            .any(|declaration| declaration.span == span && declaration.shorthand)
    }
    /// GetSymbolsOfParameterPropertyDeclaration retains the actual constructor Locals; getSymbol eligibility belongs to the evaluator.
    pub(in crate::project) fn parameter_property_table(
        self,
        file: FileId,
        declaration: Span,
    ) -> Result<Option<NativeRawSymbolTable>, TypeBindingFailure> {
        let parameter = self
            .files
            .file(file)
            .decls
            .iter()
            .find(|decl| decl.kind == DeclKind::Parameter && decl.native_span == Some(declaration))
            .ok_or(TypeBindingFailure::NotRetained)?;
        let scope = parameter.lexical.ok_or(TypeBindingFailure::NotRetained)?;
        Ok(self.raw.scope_tables[file.index()][scope.index()].map(NativeRawSymbolTable))
    }
    /// `resolveExternalModuleName` for the synthetic helpers request retains the narrower absence policy used by the helper owner.
    pub(in crate::project) fn helper_module_selection(
        self,
        file: FileId,
    ) -> Result<Option<ModuleSelection<'p>>, TypeBindingFailure> {
        let Some(request) = self
            .files
            .file(file)
            .requests
            .iter()
            .position(|request| request.implicit == Some(super::ImplicitImport::Helpers))
        else {
            return Ok(None);
        };
        let site = super::ReferenceModuleSite {
            file,
            request: super::ids::RequestId::new(request),
        };
        match self.raw.requests[file.index()][request] {
            super::raw::RawModuleRequest::Module { .. } => self.module_selection(site).map(Some),
            super::raw::RawModuleRequest::Script => Ok(None),
            super::raw::RawModuleRequest::Ignored => {
                match &self.files.file(file).modules[request] {
                    super::ModuleResolution::Missing
                    | super::ModuleResolution::Unchecked
                    | super::ModuleResolution::NotAdmitted(_) => Ok(None),
                    super::ModuleResolution::Unsupported(_) => Err(TypeBindingFailure::Pending(
                        "helper module resolves outside the admitted input profile",
                    )),
                    super::ModuleResolution::File(_) => Err(TypeBindingFailure::NotRetained),
                }
            }
        }
    }
}
