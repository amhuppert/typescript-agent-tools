//! Read operations over the completed binding product. Native evaluation
//! consumes canonical declarations, never the lexical Index's display groups.

use super::JsonValueId;
use super::collect::RefBase;
use super::ids::{DeclId, NativeHeaderId, NativeTableId};
use super::inputs::BindingFiles;
use super::link::{ResolutionFailure, flags};
use super::meaning;
use super::store::AliasRef;
use crate::Span;
use crate::project::sources::FileId;
use crate::syntax_kind::SyntaxKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) struct BoundSymbol(pub(super) NativeHeaderId);
// Native header identity is one fixed u32; Eq/Hash reads no source data.

impl BoundSymbol {
    pub(in crate::project) fn from_native_identity(
        identity: super::NativeSymbolIdentity,
    ) -> Option<Self> {
        match identity {
            super::NativeSymbolIdentity::Bound(symbol) => Some(Self(symbol)),
            _ => None,
        }
    }
    pub(in crate::project) fn native_identity(self) -> super::NativeSymbolIdentity {
        super::NativeSymbolIdentity::Bound(self.0)
    }
}

/// The exact native lookup mask, including restrictions imposed by each scope.
#[derive(Clone, Copy)]
pub(in crate::project) struct NativeLookupMeaning(pub(super) u32);
impl NativeLookupMeaning {
    pub(in crate::project) const TYPE: Self = Self(flags::TYPE);
    pub(in crate::project) const TYPE_ALIAS: Self = Self(flags::TYPE_ALIAS);
    pub(in crate::project) const VALUE: Self = Self(flags::VALUE);
    pub(in crate::project) const FUNCTION_SCOPED_VARIABLE: Self = Self(flags::FUNCTION_SCOPED);
    pub(in crate::project) const VALUE_OR_EXPORT: Self = Self(flags::VALUE | flags::EXPORT_VALUE);
    pub(in crate::project) const NAMESPACE: Self = Self(flags::NAMESPACE | flags::ENUM);
    pub(in crate::project) const INTERFACE: Self = Self(flags::INTERFACE);
    pub(in crate::project) const ALL: Self = Self(flags::VALUE | flags::TYPE | flags::NAMESPACE);
    pub(in crate::project) const ALIAS: Self = Self(flags::ALIAS);
    pub(in crate::project) fn with_alias(self) -> Self {
        Self(self.0 | flags::ALIAS)
    }
    pub(in crate::project) fn from_flags(mask: u32) -> Self {
        Self(mask)
    }
    pub(in crate::project) fn matches(self, flags: NativeSymbolFlags) -> bool {
        self.0 & flags.bits != 0
    }
    pub(in crate::project) fn projected(meaning: u8, alias: bool, export_value: bool) -> Self {
        Self(
            super::resolution::LookupMeaning::Projected(meaning).native()
                | if alias { flags::ALIAS } else { 0 }
                | if export_value { flags::EXPORT_VALUE } else { 0 },
        )
    }
}

/// Binding traversal failures and selector failures retain distinct domains.
pub(in crate::project) enum NativeSelectionFailure<E> {
    Binding(TypeBindingFailure),
    Selection(E),
}

pub(in crate::project) struct EntityNameRecipe {
    pub(super) file: FileId,
    pub(super) scope: super::ids::ScopeId,
    pub(in crate::project) head: EntityNamePart,
    pub(in crate::project) members: Vec<EntityNamePart>,
}
pub(in crate::project) struct EntityNamePart {
    pub(in crate::project) name: String,
    pub(in crate::project) span: Span,
    pub(in crate::project) missing: TypeBindingFailure,
}

/// An actual raw Symbol.Parent header. Only getParentOfSymbol follows the
/// separate checker merge map; copying this handle never canonicalizes it.
#[derive(Debug, Clone, Copy)]
pub(in crate::project) struct NativePropertyParent(pub(super) NativeHeaderId);

/// An actual raw Members/Exports table. The handle preserves nil versus empty
/// at its owning field and performs no late binding, alias or merge lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) struct NativeRawSymbolTable(pub(super) NativeTableId);
// Retained table index only: no table traversal, symbol lookup or name hashing.

impl ReadOrigin {
    /// The same actual header regardless of the alias-operation tag.
    pub(in crate::project) fn symbol(self) -> BoundSymbol {
        BoundSymbol(self.header())
    }

    pub(super) fn header(self) -> NativeHeaderId {
        match self {
            Self::Direct(symbol) => symbol.0,
            Self::Alias(alias) => alias.0,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::project) struct NativePropertyFlags {
    pub(in crate::project) variable: bool,
    pub(in crate::project) property: bool,
    pub(in crate::project) method: bool,
    pub(in crate::project) getter: bool,
    pub(in crate::project) setter: bool,
    pub(in crate::project) optional: bool,
    pub(in crate::project) prototype: bool,
}

impl NativePropertyFlags {
    /// getTypeOfSymbol's ordinary member dispatch: Accessor precedes both
    /// Variable and Property, which precede Method. Prototype is a branch
    /// inside the variable/property worker, never an accessor override.
    pub(in crate::project) fn value_kind(self) -> Option<BoundMemberKind> {
        if self.getter || self.setter {
            Some(BoundMemberKind::Accessor)
        } else if self.variable || self.property {
            Some(BoundMemberKind::Property)
        } else if self.method {
            Some(BoundMemberKind::Method)
        } else {
            None
        }
    }
}

/// Native type-symbol flag observations are independent: a merged Class and
/// ValueModule carries both, and an absent symbol is a different state.
#[derive(Clone, Copy, Default)]
pub(in crate::project) struct TypeSymbolFlags {
    pub(in crate::project) alias: bool,
    pub(in crate::project) value: bool,
    pub(in crate::project) export_value: bool,
    pub(in crate::project) type_: bool,
    pub(in crate::project) namespace: bool,
    pub(in crate::project) type_or_namespace: bool,
    pub(in crate::project) function: bool,
    pub(in crate::project) method: bool,
    pub(in crate::project) type_parameter: bool,
    pub(in crate::project) function_scoped_variable: bool,
    pub(in crate::project) block_scoped_variable: bool,
    pub(in crate::project) type_alias: bool,
    pub(in crate::project) object_literal: bool,
    pub(in crate::project) type_literal: bool,
    pub(in crate::project) signature: bool,
    pub(in crate::project) enum_: bool,
    pub(in crate::project) regular_enum: bool,
    pub(in crate::project) const_enum: bool,
    pub(in crate::project) enum_member: bool,
    pub(in crate::project) value_module: bool,
    pub(in crate::project) module_exports: bool,
    pub(in crate::project) module: bool,
    pub(in crate::project) class: bool,
    pub(in crate::project) interface: bool,
}

/// Native meaning/declaration flags in the retained compact domain. Transient
/// and CheckFlags remain separate header observations. Checker factories keep
/// these flags independently of presentation/value recipes.
#[derive(Clone, Copy, Default)]
pub(in crate::project) struct NativeSymbolFlags {
    pub(super) bits: u32,
    pub(in crate::project) assignment: bool,
    pub(in crate::project) replaceable_by_method: bool,
}
impl NativeSymbolFlags {
    pub(in crate::project) fn all_meanings() -> Self {
        Self {
            bits: u32::MAX,
            ..Self::default()
        }
    }
    pub(in crate::project) fn property_flags(self) -> NativePropertyFlags {
        let flags = self.bits;
        NativePropertyFlags {
            variable: flags & (flags::FUNCTION_SCOPED | flags::BLOCK_SCOPED) != 0,
            property: flags & flags::PROPERTY != 0,
            method: flags & flags::METHOD != 0,
            getter: flags & flags::GETTER != 0,
            setter: flags & flags::SETTER != 0,
            optional: flags & flags::OPTIONAL != 0,
            prototype: flags & flags::PROTOTYPE != 0,
        }
    }
    pub(in crate::project) fn type_flags(self) -> TypeSymbolFlags {
        let flags = self.bits;
        TypeSymbolFlags {
            alias: flags & flags::ALIAS != 0,
            value: flags & flags::VALUE != 0,
            export_value: flags & flags::EXPORT_VALUE != 0,
            type_: flags & flags::TYPE != 0,
            namespace: flags & (flags::NAMESPACE | flags::ENUM) != 0,
            type_or_namespace: flags & (flags::TYPE | flags::NAMESPACE | flags::ENUM) != 0,
            function: flags & flags::FUNCTION != 0,
            method: flags & flags::METHOD != 0,
            type_parameter: flags & flags::TYPE_PARAMETER != 0,
            function_scoped_variable: flags & flags::FUNCTION_SCOPED != 0,
            block_scoped_variable: flags & flags::BLOCK_SCOPED != 0,
            type_alias: flags & flags::TYPE_ALIAS != 0,
            object_literal: flags & flags::OBJECT_LITERAL != 0,
            type_literal: flags & flags::TYPE_LITERAL != 0,
            signature: flags & flags::SIGNATURE != 0,
            enum_: flags & flags::ENUM != 0,
            regular_enum: flags & flags::REGULAR_ENUM != 0,
            const_enum: flags & flags::CONST_ENUM != 0,
            enum_member: flags & flags::ENUM_MEMBER != 0,
            value_module: flags & flags::VALUE_MODULE != 0,
            module_exports: flags & flags::MODULE_EXPORTS != 0,
            module: flags & (flags::VALUE_MODULE | flags::NAMESPACE) != 0,
            class: flags & flags::CLASS != 0,
            interface: flags & flags::INTERFACE != 0,
        }
    }
    /// `SymbolFlagsModuleMember`, distinct from broad Value/Type meanings.
    pub(in crate::project) fn has_module_member(self) -> bool {
        self.bits & flags::MODULE_MEMBER != 0
    }
    /// Flags supplied by existing checker symbol factories. Captured/merged
    /// headers use their exact retained mask instead of this factory adapter.
    pub(in crate::project) fn from_factory(
        types: TypeSymbolFlags,
        property: NativePropertyFlags,
    ) -> Self {
        let mut bits = 0;
        for (present, flag) in [
            (types.alias, flags::ALIAS),
            (types.export_value, flags::EXPORT_VALUE),
            (types.function, flags::FUNCTION),
            (types.function_scoped_variable, flags::FUNCTION_SCOPED),
            (types.block_scoped_variable, flags::BLOCK_SCOPED),
            (types.type_parameter, flags::TYPE_PARAMETER),
            (types.type_alias, flags::TYPE_ALIAS),
            (types.object_literal, flags::OBJECT_LITERAL),
            (types.type_literal, flags::TYPE_LITERAL),
            (types.signature, flags::SIGNATURE),
            (types.regular_enum, flags::REGULAR_ENUM),
            (types.const_enum, flags::CONST_ENUM),
            (types.enum_member, flags::ENUM_MEMBER),
            (types.value_module, flags::VALUE_MODULE),
            (types.module && !types.value_module, flags::NAMESPACE_MODULE),
            (types.module_exports, flags::MODULE_EXPORTS),
            (types.class, flags::CLASS),
            (types.interface, flags::INTERFACE),
            (property.property, flags::PROPERTY),
            (property.method || types.method, flags::METHOD),
            (property.getter, flags::GETTER),
            (property.setter, flags::SETTER),
            (property.optional, flags::OPTIONAL),
            (property.prototype, flags::PROTOTYPE),
        ] {
            if present {
                bits |= flag;
            }
        }
        Self {
            bits,
            ..Self::default()
        }
    }
    /// `newSymbol(SymbolFlagsModule/Property)` intrinsic checker factories.
    pub(in crate::project) fn intrinsic_module() -> Self {
        Self {
            bits: flags::NAMESPACE,
            ..Self::default()
        }
    }
    pub(in crate::project) fn intrinsic_property() -> Self {
        Self {
            bits: flags::PROPERTY,
            ..Self::default()
        }
    }
    pub(in crate::project) fn has_namespace_module(self) -> bool {
        self.bits & flags::NAMESPACE_MODULE != 0
    }
    pub(in crate::project) fn has_value(self) -> bool {
        self.bits & flags::VALUE != 0
    }
    pub(in crate::project) fn const_enum_only_module(self) -> bool {
        self.bits & flags::CONST_ENUM_ONLY_MODULE != 0
    }
    pub(in crate::project) fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
            assignment: self.assignment || other.assignment,
            replaceable_by_method: self.replaceable_by_method || other.replaceable_by_method,
        }
    }
    /// `getExcludedSymbolFlags`: both retained and request-created headers
    /// consume the binder/checker exclusion owner, including method replacement.
    pub(in crate::project) fn conflicts_with(self, incoming: Self) -> bool {
        let excluded = super::link::excluded(incoming.bits)
            & if incoming.replaceable_by_method {
                !flags::METHOD
            } else {
                u32::MAX
            };
        self.bits & excluded != 0
    }
    pub(in crate::project) fn repair_conflicting_accessor(self, incoming: Self) -> Self {
        let accessor = flags::GETTER | flags::SETTER;
        if self.bits & accessor != 0 && self.bits & accessor != incoming.bits & accessor {
            Self {
                bits: self.bits | accessor,
                ..self
            }
        } else {
            self
        }
    }
    /// `mergeSymbol` resets ConstEnumOnlyModule before merging source flags.
    pub(in crate::project) fn merged(self, source: Self) -> Self {
        let mut target = self;
        if source.bits & flags::VALUE_MODULE != 0
            && target.bits & flags::VALUE_MODULE != 0
            && target.const_enum_only_module()
            && !source.const_enum_only_module()
        {
            target.bits &= !flags::CONST_ENUM_ONLY_MODULE;
        }
        let source = if target.const_enum_only_module() {
            source
        } else {
            Self {
                bits: source.bits & !flags::CONST_ENUM_ONLY_MODULE,
                ..source
            }
        };
        target.union(source)
    }
}

/// One actual native TypeLiteral node/symbol association. It is not a
/// class/interface meaning or a name-table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) struct BoundTypeLiteral {
    pub(in crate::project) symbol: BoundSymbol,
    pub(in crate::project) declaration: BoundDeclaration,
}
// Header/node indices, source offsets and one optional Boolean; no AST traversal.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) struct BoundJson {
    pub(in crate::project) file: FileId,
    pub(in crate::project) value: JsonValueId,
}

/// The actual lexical alias symbol, rather than the target to which its
/// type operation delegates. Separate imports of one target retain separate
/// native `getResolvedSymbol` identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) struct BoundAliasSymbol(pub(super) NativeHeaderId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) enum ReadOrigin {
    Direct(BoundSymbol),
    Alias(BoundAliasSymbol),
}

pub(in crate::project) enum ReferenceSymbol {
    Known(ReadOrigin),
}

#[derive(Debug, Clone, Copy)]
pub(in crate::project) struct NativeSymbolDeclaration {
    pub(in crate::project) json: Option<super::JsonDeclaration>,
    pub(in crate::project) file: FileId,
    pub(in crate::project) kind: crate::syntax_kind::SyntaxKind,
    pub(in crate::project) span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) struct BoundDeclaration {
    pub(in crate::project) file: FileId,
    pub(in crate::project) id: DeclId,
    pub(in crate::project) anchor: Span,
    pub(in crate::project) contains_this: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::project) enum BoundDeclaredType {
    Alias(BoundDeclaration),
    ClassOrInterface(BoundDeclaration),
    Parameter(BoundDeclaration),
    Enum(BoundDeclaration),
    EnumMember(BoundDeclaration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::project) enum BoundClassKind {
    Class,
    Interface,
}

/// The complete canonical header associations consumed by
/// `getDeclaredTypeOfClassOrInterface`. Eligible declarations retain native
/// bind/merge order; semantic parameter acquisition belongs to the checker.
#[derive(Clone)]
pub(in crate::project) struct BoundClassOrInterface {
    pub(in crate::project) kind: BoundClassKind,
    pub(in crate::project) has_interface: bool,
    pub(in crate::project) declarations: Vec<BoundDeclaration>,
}

/// The value family selected by `getTypeOfSymbol`'s actual own declarations.
/// Function declarations retain native bind/merge order for signature lookup.
#[derive(Debug, Clone)]
pub(in crate::project) enum BoundValueType {
    Undefined,
    EnumMember(BoundDeclaration),
    Variable(BoundDeclaration),
    Function(Vec<BoundDeclaration>),
    Class(BoundDeclaration),
    Module,
    Json {
        file: FileId,
        declaration: super::JsonDeclaration,
    },
    Prototype {
        parent: BoundSymbol,
    },
    Member(BoundMember),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::project) enum BoundMemberKind {
    Property,
    Method,
    Accessor,
}

/// Binding-table inspection for unit tests; the evaluator reads declarations
/// through its own native owners.
#[cfg(test)]
#[derive(Debug, Clone)]
pub(in crate::project) struct BoundMemberDeclaration {
    pub(in crate::project) declaration: BoundDeclaration,
    pub(in crate::project) native_kind: crate::syntax_kind::SyntaxKind,
}

#[cfg(test)]
#[derive(Clone)]
pub(in crate::project) struct BoundNamedMember {
    pub(in crate::project) symbol: BoundSymbol,
}

#[derive(Debug, Clone)]
pub(in crate::project) struct BoundMember {
    pub(in crate::project) kind: BoundMemberKind,
    /// Native flags may overlap on merged declarations. The read family's
    /// accessor precedence does not answer write or flow flag questions.
    pub(in crate::project) property: bool,
    pub(in crate::project) value_declaration: BoundDeclaration,
    pub(in crate::project) optional: bool,
}

impl BoundDeclaredType {
    pub(in crate::project) fn declaration(self) -> BoundDeclaration {
        match self {
            Self::Alias(declaration)
            | Self::ClassOrInterface(declaration)
            | Self::Parameter(declaration)
            | Self::Enum(declaration)
            | Self::EnumMember(declaration) => declaration,
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::project) enum TypeBindingFailure {
    NotRetained,
    Unresolved(String),
    Dependency {
        file: FileId,
        span: Span,
        reason: String,
    },
    ErrorType(&'static str),
    Pending(&'static str),
}

impl From<&ResolutionFailure> for TypeBindingFailure {
    /// A consumed alias/module dependency owns its captured source range.
    /// Only an unresolved terminal name uses the semantic caller's location.
    fn from(failure: &ResolutionFailure) -> Self {
        match failure.dependency {
            Some((file, span)) => Self::Dependency {
                file,
                span,
                reason: failure.reason.clone(),
            },
            None => Self::Unresolved(failure.reason.clone()),
        }
    }
}

impl<'a> BindingFiles<'a> {
    /// Captured `Program.IsSourceFileDefaultLibrary` for checker diagnostics.
    pub(in crate::project) fn is_source_file_default_library(&self, file: FileId) -> bool {
        self.file(file).diagnostics.is_default_library()
    }

    /// `IsExternalOrCommonJSModule`: the accepted TS/JSON program retains
    /// its finalized module indicator, including configuration-forced files.
    pub(in crate::project) fn reference_file_is_external_or_commonjs(&self, file: FileId) -> bool {
        // Admitted TS files have no JS CommonJS indicator. JSON has an actual
        // module symbol, but getExternalModuleIndicator explicitly returns nil.
        self.reference_file_is_external(file)
    }

    pub(in crate::project) fn reference_file_is_external(&self, file: FileId) -> bool {
        self.file(file).module && self.file(file).json.is_none()
    }

    /// BoundJson is made only by the completed linker, so absence is an
    /// internal attachment violation, never an ordinary missing dependency.
    pub(in crate::project) fn json_facts(&self, value: BoundJson) -> &'a super::JsonFacts {
        self.file(value.file)
            .json
            .expect("canonical JSON target has captured JSON facts")
    }

    pub(in crate::project) fn declaration_name(&self, declaration: BoundDeclaration) -> &'a str {
        &self.file(declaration.file).decls[declaration.id.index()].name
    }

    /// The actual declaration kind retained at binding, not its lexical
    /// meaning or the kind of another declaration sharing the symbol.
    pub(in crate::project) fn declaration_syntax_kind(
        &self,
        declaration: BoundDeclaration,
    ) -> crate::syntax_kind::SyntaxKind {
        self.file(declaration.file).decls[declaration.id.index()].native_kind
    }

    /// Exact native alias-declaration attachment, shared by node.Symbol and
    /// declaration-owned syntax facts. A merged symbol's last alias declaration
    /// is a different question and cannot supply the selected node's flags.
    pub(super) fn alias_declaration_at(
        &self,
        file: FileId,
        kind: SyntaxKind,
        span: Span,
    ) -> Result<Option<AliasRef>, TypeBindingFailure> {
        let mut aliases = self
            .file(file)
            .aliases
            .iter()
            .enumerate()
            .filter(|(_, alias)| alias.native_kind == kind && alias.native_span == span);
        if let Some((index, _)) = aliases.next() {
            if aliases.next().is_some() {
                return Err(TypeBindingFailure::Pending(
                    "declaration has multiple actual alias symbol associations",
                ));
            }
            return Ok(Some(AliasRef {
                file,
                id: super::ids::AliasId::new(index),
            }));
        }
        Ok(None)
    }

    /// Retained `isTypeOnlyImportOrExportDeclaration` for this exact node;
    /// collection owns the ancestor syntax rules, not the resolved alias chain.
    pub(in crate::project) fn alias_syntax_is_type_only(
        &self,
        file: FileId,
        kind: SyntaxKind,
        span: Span,
    ) -> Result<bool, TypeBindingFailure> {
        let alias = self
            .alias_declaration_at(file, kind, span)?
            .ok_or(TypeBindingFailure::NotRetained)?;
        Ok(self.file(file).aliases[alias.id.index()].type_only)
    }
}

impl BindingFiles<'_> {
    /// `getResolvedSymbol` / `resolveName` use a retained lexical scope even
    /// when navigation classified this occurrence as Type or Namespace (for
    /// example an import-equals RHS). Its original meaning is not eligibility
    /// for a later checker lookup; the caller supplies that lookup's mask.
    pub(super) fn lexical_reference_index(
        &self,
        file: FileId,
        anchor: Span,
        type_context: Option<bool>,
    ) -> Result<usize, TypeBindingFailure> {
        self.references(file)
            .position(|reference| {
                reference.span == anchor
                    && type_context.is_none_or(|context| reference.type_context == context)
                    && matches!(reference.base, RefBase::Lexical(_))
            })
            .ok_or(TypeBindingFailure::NotRetained)
    }
    pub(super) fn type_reference_index(
        &self,
        file: FileId,
        anchor: Span,
    ) -> Result<usize, TypeBindingFailure> {
        self.references(file)
            .position(|reference| {
                reference.span == anchor && reference.meaning & meaning::TYPE != 0
            })
            .ok_or(TypeBindingFailure::NotRetained)
    }
}
