//! Actual native property symbols, distinct from printer-question identities.
//! Canonical symbols reuse binder identity; instantiateSymbol allocates a fresh
//! transient only after its two native identity-preserving paths are tested.

use super::super::model::MapperId;
use super::{
    DemandRecipe, Evaluator, ReferenceDeclaration, ReferenceSymbol, TypeDemandState, TypeState,
};
use crate::project::binding::{BoundSymbol, BoundValueType, NativeSymbolIdentity, TypeSymbolFlags};
use crate::project::semantic::model::{
    DemandError, DemandId, NativeCompletion, NativeResult, NativeSequence, ObjectFlags, Property,
    PropertyData, PropertyDisplayDemand, PropertySymbolId, PropertyValueQuestion, TypeDemand,
    TypeFlags, TypeId, TypeSymbol, TypeView,
};
use crate::project::semantic::source::DeclarationIdentity;
use crate::references::Location;
use crate::syntax::modifiers;
use crate::types::{TypeAccessibility, TypeKey, TypePropertyKind};
use std::collections::HashMap;

mod combined;
mod composite;
mod indexes;
mod late;
mod mapped;
mod modules;
mod reverse;
mod spread;
mod unresolved;
use crate::project::binding::{NativePropertyKey, NativePropertyParent, NativeSymbol};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct ObjectLiteralPropertyOptions {
    pub(super) readonly: bool,
    pub(super) name_type: Option<TypeId>,
    pub(super) default_optional: bool,
    pub(super) contextual_pattern: Option<TypeId>,
}

// Native ast.CheckFlags shared by actual transient property factories.
pub(super) mod checks {
    pub const UNRESOLVED: u32 = 1 << 20;
    pub const SYNTHETIC_PROPERTY: u32 = 1 << 1;
    pub const SYNTHETIC_METHOD: u32 = 1 << 2;
    pub const READONLY: u32 = 1 << 3;
    pub const READ_PARTIAL: u32 = 1 << 4;
    pub const WRITE_PARTIAL: u32 = 1 << 5;
    pub const NON_UNIFORM: u32 = 1 << 6;
    pub const LITERAL: u32 = 1 << 7;
    pub const PUBLIC: u32 = 1 << 8;
    pub const PROTECTED: u32 = 1 << 9;
    pub const PRIVATE: u32 = 1 << 10;
    pub const STATIC: u32 = 1 << 11;
    pub const LATE: u32 = 1 << 12;
    pub const OPTIONAL_PARAMETER: u32 = 1 << 14;
    pub const REST_PARAMETER: u32 = 1 << 15;
    pub const DEFERRED: u32 = 1 << 16;
    pub const NEVER: u32 = 1 << 17;
    pub const REVERSE_MAPPED: u32 = 1 << 13;
    pub const MAPPED: u32 = 1 << 18;
    pub const STRIP_OPTIONAL: u32 = 1 << 19;
    pub const INDEX_SYMBOL: u32 = 1 << 23;
    pub const SYNTHETIC: u32 = SYNTHETIC_PROPERTY | SYNTHETIC_METHOD;
    pub const PARTIAL: u32 = READ_PARTIAL | WRITE_PARTIAL;
}

pub(super) struct PropertySymbols {
    index_symbols: HashMap<super::super::model::Index, PropertySymbolId>,
    unresolved_names: std::collections::HashMap<String, PropertySymbolId>,
    unresolved_declared: HashMap<PropertySymbolId, TypeId>,
    own: HashMap<NativeSymbolIdentity, PropertySymbolId>,
    entries: Vec<Symbol>,
    demands: HashMap<(PropertySymbolId, bool), DemandId>,
    composite_cache: crate::resources::CheckedMap<
        composite::CompositeKey,
        NativeCompletion<crate::project::semantic::model::Property>,
    >,
    composite_inventories:
        HashMap<TypeId, NativeCompletion<Vec<crate::project::semantic::model::Property>>>,
    discriminants: HashMap<PropertySymbolId, NativeCompletion<bool>>,
    // Evidence for this owner's actual computed-bit publication; factory-set
    // bits have no associated acquisition and remain clean observations.
    containment: HashMap<TypeId, NativeCompletion<bool>>,
}

impl PropertySymbols {
    /// Request-local backing for the existing native cache questions. Their
    /// identities, child values and provisional publication stay operation-owned.
    pub(super) fn new() -> Self {
        Self {
            index_symbols: Default::default(),
            unresolved_names: std::collections::HashMap::new(),
            unresolved_declared: HashMap::new(),
            own: HashMap::new(),
            entries: Default::default(),
            demands: HashMap::new(),
            composite_cache: crate::resources::CheckedMap::new(),
            composite_inventories: HashMap::new(),
            discriminants: HashMap::new(),
            containment: HashMap::new(),
        }
    }
}

struct Symbol {
    actual_identity: NativeSymbolIdentity,
    association: Association,
    read: Option<TypeState>,
    write: Option<TypeState>,
}

#[derive(Clone)]
enum Association {
    Own(BoundSymbol),
    Created(Arc<CreatedProperty>),
    Instantiated {
        target: PropertySymbolId,
        mapper: MapperId,
    },
}

/// `createSymbol` factories share actual symbol metadata. Their lazy value
/// recipe is separate: known tuple properties cannot accidentally acquire a
/// containing union, deferred constituent list, or a clone target.
struct CreatedProperty {
    native_flags: Option<crate::project::binding::NativeSymbolFlags>,
    flags: PropertySymbolFlags,
    checks: u32,
    declarations: Vec<super::declarations::DeclarationAssociation>,
    value_declaration: Option<super::declarations::DeclarationAssociation>,
    parent: Option<SymbolParent>,
    raw_members: Option<super::reference_symbols::ReferenceRawSymbolTable>,
    raw_exports: Option<super::reference_symbols::ReferenceRawSymbolTable>,
    name_type: Option<TypeId>,
    header: Arc<NativeSymbol>,
    mapper: Option<MapperId>,
    target: Option<PropertySymbolId>,
    synthetic_origin: Option<PropertySymbolId>,
    spread: Option<(PropertySymbolId, PropertySymbolId)>,
    export_type: Option<ExportTypeLinks>,
    value: CreatedPropertyValue,
}

#[derive(Clone, Copy)]
pub(super) struct ExportTypeLinks {
    pub(super) originating_import: crate::project::semantic::source::ExpressionRoot,
    pub(super) target: super::ReferenceSymbol,
}

enum CreatedPropertyValue {
    /// A symbol with no value flags reaches getTypeOfSymbol's final errorType
    /// branch. It has no installed Type slot and is not unknownSymbol.
    NoValue,
    UnresolvedAlias,
    /// Alias links use this symbol's actual identity. A retained binder origin
    /// supplies its declaration recipe; factory aliases may be declarationless.
    Alias(Option<crate::project::binding::ReadOrigin>),
    /// Native unknownSymbol has an installed errorType. Its identity and
    /// metadata remain queryable; consuming the value retains the failure.
    Error,
    Composite {
        containing: TypeId,
        read_types: Vec<TypeId>,
        write_types: Option<Vec<TypeId>>,
    },
    Mapped {
        containing: TypeId,
        key_type: TypeId,
    },
    ReverseMapped {
        property_type: TypeId,
        mapped_type: TypeId,
        constraint_type: TypeId,
    },
    Declaration,
    /// The factory installed the raw type before publishing this symbol.
    Known,
}

/// Symbol.Parent can be a retained binder header or an actual checker-created
/// symbol. A created symbol never acquires a fabricated binder identity.
#[derive(Clone, Copy)]
pub(in crate::project::semantic) enum SymbolParent {
    Bound(NativePropertyParent),
    Created(PropertySymbolId),
}
impl From<NativePropertyParent> for SymbolParent {
    fn from(parent: NativePropertyParent) -> Self {
        Self::Bound(parent)
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct PropertySymbolFlags {
    pub(super) types: TypeSymbolFlags,
    pub(super) variable: bool,
    pub(super) property: bool,
    pub(super) method: bool,
    pub(super) get_accessor: bool,
    pub(super) set_accessor: bool,
    pub(super) optional: bool,
    pub(super) prototype: bool,
    pub(super) partial: bool,
}

impl PropertySymbolFlags {
    /// SymbolFlagsClassMember is a mask, not a separately stored factory bit.
    pub(super) fn class_member(self) -> bool {
        self.property || self.method || self.get_accessor || self.set_accessor
    }
    fn type_flags(self) -> TypeSymbolFlags {
        TypeSymbolFlags {
            method: self.method || self.types.method,
            value: self.types.value
                || self.variable
                || self.property
                || self.method
                || self.get_accessor
                || self.set_accessor,
            ..self.types
        }
    }
    fn native_flags(self) -> crate::project::binding::NativePropertyFlags {
        crate::project::binding::NativePropertyFlags {
            variable: self.variable,
            property: self.property,
            method: self.method,
            getter: self.get_accessor,
            setter: self.set_accessor,
            optional: self.optional,
            prototype: self.prototype,
        }
    }
}

pub(super) struct PropertySymbolNameFacts {
    pub(super) name_type: Option<TypeId>,
    pub(super) key: NativePropertyKey,
    pub(super) known_symbol: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum RecursionIdentity {
    Symbol(NativeSymbolIdentity),
    Node(crate::project::binding::NativeNodeIdentity),
    Type(TypeId),
}

impl Evaluator<'_, '_> {
    pub(super) fn symbol_parent_is_class(&self, parent: SymbolParent) -> Result<bool, DemandError> {
        match parent {
            SymbolParent::Bound(parent) => Ok(self.source.bindings().parent_is_class(parent)),
            SymbolParent::Created(parent) => Ok(self.property_symbol_flags(parent)?.types.class),
        }
    }

    /// `isStaticPrivateIdentifierProperty` observes the actual ValueDeclaration,
    /// independently of the symbol's flags or its value-type recipe.
    pub(super) fn is_static_private_identifier_property(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        match self.property_reference_value_declaration(symbol, consumer)? {
            Some(ReferenceDeclaration::Captured(declaration)) => Ok(self
                .source
                .declaration_is_private_class_element(declaration)?
                && self.source.declaration_is_static(declaration)?),
            // JSON's SourceFile, ObjectLiteralExpression and PropertyAssignment
            // declarations cannot be private-identifier class elements.
            Some(ReferenceDeclaration::Json { .. }) | None => Ok(false),
        }
    }

    /// `invocationErrorRecovery` consumes only the actual exportTypeLinks
    /// association installed by cloneTypeAsModule. Other native factories
    /// have an empty link, including declarationless synthetic type symbols.
    pub(super) fn reference_invocation_error_recovery_completion(
        &mut self,
        ty: TypeId,
        construct: bool,
        consumer: &Location,
    ) -> NativeResult<()> {
        if let Some(links) = self.type_export_links(ty)? {
            let declaration = self
                .source
                .type_node_declaration(links.originating_import)?;
            if !self.source.declaration_is_import_call(declaration)? {
                return self
                    .value_reference_type_completion(links.target, consumer)?
                    .and_then_value(|ty| {
                        self.signatures_of_type(ty, construct, consumer)?;
                        Ok(())
                    });
            }
        }
        Ok(NativeCompletion::clean(()))
    }

    /// The checker's `exportTypeLinks` observation. Only cloneTypeAsModuleType
    /// installs these links; reading them does not acquire the target's type.
    pub(super) fn type_export_links(
        &self,
        ty: TypeId,
    ) -> Result<Option<ExportTypeLinks>, DemandError> {
        Ok(match self.model.type_symbol(ty) {
            TypeSymbol::Property { symbol, .. } => match self.property_association(symbol)? {
                Association::Created(data) => data.export_type,
                Association::Own(_) | Association::Instantiated { .. } => None,
            },
            TypeSymbol::Absent | TypeSymbol::Bound { .. } | TypeSymbol::Synthetic { .. } => None,
        })
    }

    /// `isSpreadableProperty`: private-identifier and method/accessor gates
    /// precede the second, independently ordered declaration-parent scan.
    pub(super) fn is_spreadable_property(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        let declarations = self.property_reference_declarations(symbol, consumer)?;
        let mut private = false;
        for &declaration in &declarations {
            if let ReferenceDeclaration::Captured(declaration) = declaration
                && self
                    .source
                    .declaration_is_private_class_element(declaration)?
            {
                private = true;
                break;
            }
        }
        if !private {
            let flags = self.property_symbol_flags(symbol)?;
            if !flags.method && !flags.get_accessor && !flags.set_accessor {
                return Ok(true);
            }
        }
        for declaration in declarations {
            if let ReferenceDeclaration::Captured(declaration) = declaration
                && self.source.declaration_parent_is_class_like(declaration)?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `getSpreadSymbol` preserves identity when readonly agrees, except for
    /// setter-only accessors. A fresh spread owns syntheticOrigin, not target.
    pub(super) fn create_spread_symbol_completion(
        &mut self,
        source: PropertySymbolId,
        readonly: bool,
        consumer: &Location,
    ) -> NativeResult<PropertySymbolId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let flags = self.property_symbol_flags(source)?;
            let set_only = flags.set_accessor && !flags.get_accessor;
            if !set_only
                && readonly
                    == sequence.read(self.property_is_readonly_completion(source, consumer))?
            {
                return Ok(source);
            }
            let checks = self.property_check_flags(source)? & checks::LATE
                | if readonly { checks::READONLY } else { 0 };
            let identity = self.model.new_symbol_identity();
            let ty = if set_only {
                NativeCompletion::clean(
                    self.model
                        .special(crate::project::semantic::model::SpecialName::Undefined),
                )
            } else {
                self.property_raw_type_completion(source, consumer)?
            };
            sequence.read(Ok(ty.clone()))?;
            let declarations = self.property_reference_declarations(source, consumer)?;
            let name_type = self.property_name_type(source)?;
            let header = self
                .property_native_symbol(source, consumer)?
                .with_identity(identity);
            let id = self.install_created_property(CreatedProperty {
                native_flags: None,
                flags: PropertySymbolFlags {
                    property: true,
                    optional: flags.optional,
                    ..Default::default()
                },
                checks,
                declarations: declarations.into_iter().map(Into::into).collect(),
                value_declaration: None,
                parent: None,
                raw_members: None,
                raw_exports: None,
                name_type,
                header,
                mapper: None,
                target: None,
                synthetic_origin: Some(source),
                spread: None,
                export_type: None,
                value: CreatedPropertyValue::Known,
            });
            self.native_members.symbols.entries[id.index()].read = Some(TypeState::Installed(ty));
            Ok(id)
        })();
        sequence.finish(result)
    }

    /// `newSymbol` always adds SymbolFlagsTransient, including flags-None
    /// and unresolved TypeAlias factories; `newSymbolEx` delegates to it.
    /// Created entries all use those checker factories except Alias(Some),
    /// which registers the original binder symbol without allocating one.
    pub(super) fn property_is_transient(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<bool, DemandError> {
        Ok(match self.property_association(symbol)? {
            Association::Own(symbol) => self.source.bindings().native_symbol_is_transient(symbol),
            Association::Created(data) => {
                !matches!(&data.value, CreatedPropertyValue::Alias(Some(_)))
            }
            Association::Instantiated { .. } => true,
        })
    }
    /// `Symbol.Name` passed to native property-table operations retains its
    /// exact byte/identity components. Only actual text consumers project it.
    pub(super) fn property_lookup_key(
        &self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<NativePropertyKey, DemandError> {
        self.reference_symbol_key(ReferenceSymbol::from_property(symbol), consumer)
    }

    /// The actual `SymbolLinks.target` link, independent of CheckFlagsInstantiated.
    /// Known type clones and merged-property clones retain this same native link.
    pub(super) fn property_link_target(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<Option<PropertySymbolId>, DemandError> {
        Ok(match self.property_association(symbol)? {
            Association::Own(_) => None,
            Association::Created(data) => data.target,
            Association::Instantiated { target, .. } => Some(target),
        })
    }

    /// `getImmediateRootSymbols` / `tryGetTarget`: synthetic constituents,
    /// spread pairs and mapped origins precede transient target following.
    pub(super) fn property_immediate_root_symbols(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<Vec<PropertySymbolId>, DemandError> {
        if self.property_check_flags(symbol)? & checks::SYNTHETIC != 0 {
            let data = self.created_property(symbol)?;
            let CreatedPropertyValue::Composite { containing, .. } = &data.value else {
                return Err(
                    super::super::internal("synthetic property lacks containing type").into(),
                );
            };
            let (types, _) = self.composite_constituents(*containing)?;
            let name = self.property_symbol_name_facts(symbol, consumer)?.key;
            let mut roots = Vec::new();
            for &ty in types.iter() {
                if let Some(property) = self.property_of_type_by_key(ty, &name, consumer)? {
                    roots.push(self.property_symbol(&property, consumer)?);
                }
            }
            return Ok(roots);
        }
        if let Association::Created(data) = self.property_association(symbol)? {
            if let Some((left, right)) = data.spread {
                return Ok(vec![left, right]);
            }
            if let Some(origin) = data.synthetic_origin {
                return Ok(vec![origin]);
            }
        }
        if matches!(self.property_association(symbol)?, Association::Own(_)) {
            return Ok(Vec::new());
        }
        let mut target = None;
        let mut next = symbol;
        while let Some(link) = self.property_link_target(next)? {
            target = Some(link);
            next = link;
        }
        Ok(target.into_iter().collect())
    }

    /// `createSymbolWithType`: allocate a distinct actual symbol, copying only
    /// the native declaration/parent/name facts and readonly check bit. The
    /// supplied raw type is installed before publication; no source write is read.
    pub(super) fn create_property_with_type(
        &mut self,
        source: PropertySymbolId,
        ty: Option<TypeId>,
        make_optional: bool,
        consumer: &Location,
    ) -> Result<PropertySymbolId, DemandError> {
        let name_type = self.property_name_type(source)?;
        let declarations = self.property_reference_declarations(source, consumer)?;
        let value_declaration = self.property_reference_value_declaration(source, consumer)?;
        let parent = self.property_parent(source)?;
        let mut flags = self.property_symbol_flags(source)?;
        flags.optional |= make_optional;
        flags.partial = false;
        let checks = self.property_check_flags(source)? & checks::READONLY;
        let header = self
            .property_native_symbol(source, consumer)?
            .with_identity(self.model.new_symbol_identity());
        let id = self.install_created_property(CreatedProperty {
            native_flags: None,
            flags,
            checks,
            declarations: declarations.into_iter().map(Into::into).collect(),
            value_declaration: value_declaration.map(Into::into),
            parent,
            raw_members: None,
            raw_exports: None,
            name_type,
            header,
            mapper: None,
            target: Some(source),
            synthetic_origin: None,
            spread: None,
            export_type: None,
            value: if ty.is_some() {
                CreatedPropertyValue::Known
            } else {
                CreatedPropertyValue::Declaration
            },
        });
        self.native_members.symbols.entries[id.index()].read =
            ty.map(|ty| TypeState::Installed(NativeCompletion::clean(ty)));
        Ok(id)
    }

    /// `checkObjectLiteral` creates a fresh Property transient after checking
    /// the initializer. It copies the raw member's declarations and parent,
    /// retaining target independently from its installed read type.
    pub(super) fn create_object_literal_property(
        &mut self,
        source: PropertySymbolId,
        ty: NativeCompletion<TypeId>,
        options: ObjectLiteralPropertyOptions,
        consumer: &Location,
    ) -> Result<PropertySymbolId, DemandError> {
        let mut flags = self.property_symbol_flags(source)?;
        flags.property = true;
        let original = self.property_native_symbol(source, consumer)?;
        let identity = self.model.new_symbol_identity();
        let header = match options.name_type {
            Some(name) => original.with_identity_and_key(
                identity,
                self.property_key_from_type(name)?.ok_or_else(|| {
                    super::super::internal("usable object property name has no key")
                })?,
            ),
            None => original.with_identity(identity),
        };
        if options.default_optional {
            flags.optional = true;
        } else if let Some(context) = options.contextual_pattern {
            let key = original
                .key()
                .map_err(|error| self.generic_binding_failure(error, consumer))?;
            if let Some(property) = self.property_of_type_by_key(context, &key, consumer)? {
                let symbol = self.property_symbol(&property, consumer)?;
                flags.optional |= self.property_symbol_flags(symbol)?.optional;
            } else {
                let string = self.model.primitive(crate::types::PrimitiveName::String);
                if self
                    .index_info_of_type(context, string, consumer)?
                    .is_none()
                {
                    let first = self.diagnostic_property_symbol_display(source, consumer);
                    if let Err(error) = &first
                        && error.is_whole_operation_failure()
                    {
                        return Err(error.clone());
                    }
                    let second = self.check_diagnostic_type_display(context, consumer);
                    match (first, second) {
                        (Err(first), Err(second)) => return Err(first.append_failure(second)),
                        (Err(error), _) | (_, Err(error)) => return Err(error),
                        _ => {}
                    }
                }
            }
        }
        let declarations = self.property_reference_declarations(source, consumer)?;
        let value_declaration = self.property_reference_value_declaration(source, consumer)?;
        let parent = self.property_parent(source)?;
        let id = self.install_created_property(CreatedProperty {
            native_flags: None,
            flags,
            checks: if options.readonly {
                checks::READONLY
            } else {
                0
            } | if options.name_type.is_some() {
                checks::LATE
            } else {
                0
            },
            declarations: declarations.into_iter().map(Into::into).collect(),
            value_declaration: value_declaration.map(Into::into),
            parent,
            raw_members: None,
            raw_exports: None,
            name_type: options.name_type,
            header,
            mapper: None,
            target: Some(source),
            synthetic_origin: None,
            spread: None,
            export_type: None,
            value: CreatedPropertyValue::Known,
        });
        self.native_members.symbols.entries[id.index()].read = Some(TypeState::Installed(ty));
        Ok(id)
    }

    /// `isCircularMappedProperty` observes only an actual mapped property's
    /// unresolved native Type slot and the shared resolution-cycle window.
    pub(super) fn is_circular_mapped_property(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<bool, DemandError> {
        Ok(self.property_check_flags(symbol)? & checks::MAPPED != 0
            && !self.property_has_value(symbol, false)
            && self
                .resolution_cycle_start(super::resolution::Key::PropertySymbol {
                    target: symbol,
                    property: super::resolution::Property::Type,
                })
                .is_some())
    }
    /// Read the property's actual native symbol association. Producers whose
    /// symbols are not yet retained keep this as a semantic dependency.
    pub(super) fn property_symbol(
        &self,
        property: &Property,
        consumer: &Location,
    ) -> Result<PropertySymbolId, DemandError> {
        property.symbol.ok_or_else(|| {
            super::super::unsupported_demand_at_location(
                self.inputs,
                consumer,
                crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
                "native property symbol association is pending",
            )
        })
    }

    /// The actual value-symbol `nameType` link. Copying this link does not
    /// consume the symbol's native name or its generated-key representation.
    pub(super) fn property_name_type(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<Option<TypeId>, DemandError> {
        match self.property_association(symbol)? {
            Association::Own(_) => Ok(None),
            Association::Created(data) => Ok(data.name_type),
            Association::Instantiated { target, .. } => self.property_name_type(target),
        }
    }

    /// `getLateBoundSymbol`/`IsKnownSymbol` observations for the admitted
    /// early-bound and binder prototype factories. ValueDeclaration syntax is
    /// acquired separately, after the native default-name short circuit.
    pub(super) fn property_symbol_name_facts(
        &self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<PropertySymbolNameFacts, DemandError> {
        match self.property_association(symbol)? {
            Association::Instantiated { target, .. } => {
                self.property_symbol_name_facts(target, consumer)
            }
            Association::Created(_) => {
                let data = self.created_property(symbol)?;
                Ok(PropertySymbolNameFacts {
                    name_type: data.name_type,
                    key: data
                        .header
                        .key()
                        .map_err(|error| self.generic_binding_failure(error, consumer))?,
                    known_symbol: data
                        .header
                        .key()
                        .map_err(|error| self.generic_binding_failure(error, consumer))?
                        .is_known_symbol(),
                })
            }
            Association::Own(bound) => {
                let symbol = self
                    .source
                    .bindings()
                    .native_symbol(bound)
                    .map_err(|error| self.generic_binding_failure(error, consumer))?;
                let key = symbol
                    .key()
                    .map_err(|error| self.generic_binding_failure(error, consumer))?;
                let known_symbol = symbol
                    .key()
                    .map_err(|error| self.generic_binding_failure(error, consumer))?
                    .is_known_symbol();
                Ok(PropertySymbolNameFacts {
                    name_type: None,
                    key,
                    known_symbol,
                })
            }
        }
    }

    /// Actual `Type.symbol` flags. Synthetic emptyTypeLiteralType has a real
    /// TypeLiteral symbol, while nil and unavailable factory facts stay distinct.
    pub(in crate::project::semantic) fn type_symbol_flags(
        &self,
        ty: TypeId,
    ) -> Result<Option<TypeSymbolFlags>, DemandError> {
        match self.model.type_symbol(ty) {
            TypeSymbol::Absent => Ok(None),
            TypeSymbol::Bound { flags, .. } => flags.map(Some),
            TypeSymbol::Property { symbol, .. } => {
                Ok(Some(self.property_symbol_flags(symbol)?.type_flags()))
            }
            TypeSymbol::Synthetic { flags, .. } => Ok(Some(flags)),
        }
    }

    /// Actual Type.symbol declaration fields, preserving the known absence
    /// on native startup and synthetic symbol factories.
    pub(super) fn type_symbol_value_declaration(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Option<ReferenceDeclaration>, DemandError> {
        match self.model.type_symbol(ty) {
            TypeSymbol::Absent | TypeSymbol::Synthetic { .. } => Ok(None),
            TypeSymbol::Bound { symbol, .. } => {
                self.reference_symbol_value_declaration(symbol.into(), consumer)
            }
            TypeSymbol::Property { symbol, .. } => {
                self.property_reference_value_declaration(symbol, consumer)
            }
        }
    }

    /// `FirstOrNil(type.symbol.Declarations)` projects only the selected
    /// native declaration; merged symbols do not parse later declarations.
    pub(super) fn type_symbol_first_declaration(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Option<ReferenceDeclaration>, DemandError> {
        if matches!(
            self.model.type_symbol(ty),
            TypeSymbol::Absent | TypeSymbol::Synthetic { .. }
        ) {
            return Ok(None);
        }
        let Some(symbol) = self.reference_symbol_of_type(ty, consumer)? else {
            return Ok(None);
        };
        self.reference_symbol_first_declaration(symbol, consumer)
    }

    /// Native Type.symbol equality is independent from symbol name and
    /// declaration-order questions. Actual nil, bound and synthetic factories
    /// keep their separate meanings; missing factory facts remain failures.
    pub(super) fn type_symbol_identity(
        &self,
        ty: TypeId,
    ) -> Result<Option<NativeSymbolIdentity>, DemandError> {
        match self.model.type_symbol(ty) {
            TypeSymbol::Absent => Ok(None),
            TypeSymbol::Synthetic { native, .. } => Ok(Some(native.identity())),
            TypeSymbol::Property { native, .. } => Ok(Some(native.identity())),
            TypeSymbol::Bound { symbol, .. } => self
                .source
                .bindings()
                .native_symbol_identity(symbol)
                .map(Some)
                .map_err(|error| self.symbol_association_failure(symbol, error)),
        }
    }

    /// `getRecursionIdentity`: literal objects retain type identity; ordinary
    /// references and anonymous types use their actual symbol unless the
    /// factory retained FromTypeNode. Deferred references use their actual
    /// source node, ordinary tuples their target, and indexed accesses their
    /// leftmost object. Class static sides keep their own type identity.
    pub(super) fn recursion_identity(
        &mut self,
        ty: TypeId,
        _consumer: &Location,
    ) -> Result<RecursionIdentity, DemandError> {
        if self.model.has_type_flags(ty, TypeFlags::OBJECT) {
            let flags = self.model.object_flags(ty)?;
            if !flags.intersects(ObjectFlags::OBJECT_LITERAL | ObjectFlags::ARRAY_LITERAL)
                && let Some(reference) = self.model.deferred_reference(ty)
            {
                return Ok(RecursionIdentity::Node(reference.declaration));
            }
            if !(flags.intersects(ObjectFlags::OBJECT_LITERAL | ObjectFlags::ARRAY_LITERAL)
                || flags.contains(ObjectFlags::FROM_TYPE_NODE)
                || flags.contains(ObjectFlags::ANONYMOUS)
                    && self
                        .type_symbol_flags(ty)?
                        .is_some_and(|symbol| symbol.class))
            {
                if let Some(symbol) = self.type_symbol_identity(ty)? {
                    return Ok(RecursionIdentity::Symbol(symbol));
                }
            }
            if !flags.intersects(
                ObjectFlags::OBJECT_LITERAL
                    | ObjectFlags::ARRAY_LITERAL
                    | ObjectFlags::FROM_TYPE_NODE,
            ) && self.model.is_tuple_type(ty)
            {
                return Ok(RecursionIdentity::Type(
                    self.model
                        .reference_target(ty)
                        .expect("tuple has reference target"),
                ));
            }
        }
        if self.model.has_type_flags(ty, TypeFlags::TYPE_PARAMETER) {
            if let Some(symbol) = self.type_symbol_identity(ty)? {
                return Ok(RecursionIdentity::Symbol(symbol));
            }
        }
        if let TypeView::IndexedAccess(access) = self.model.get(ty)? {
            let mut leftmost = access.object_type;
            while let TypeView::IndexedAccess(access) = self.model.get(leftmost)? {
                leftmost = access.object_type;
            }
            return Ok(RecursionIdentity::Type(leftmost));
        }
        if let TypeView::Conditional(view) = self.model.get(ty)? {
            return Ok(RecursionIdentity::Node(
                self.conditional_root_node(view.root),
            ));
        }
        Ok(RecursionIdentity::Type(ty))
    }

    /// `couldContainTypeVariablesWorker` / `isNonGenericTopLevelType` consume
    /// native factory flags and alias/symbol identity without acquiring member
    /// values. Its computed bit belongs to the actual type, not a second cache.
    pub(super) fn could_contain_type_variables_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let _depth = {
            self.control
                .as_ref()
                .map(|control| crate::resources::complete_or_abort(control.enter()))
        };
        if !self
            .model
            .has_type_flags(ty, TypeFlags::STRUCTURED_OR_INSTANTIABLE)
        {
            return Ok(NativeCompletion::clean(false));
        }
        let flags = self.model.object_flags(ty)?;
        if flags.contains(ObjectFlags::COULD_CONTAIN_TYPE_VARIABLES_COMPUTED) {
            return Ok(self
                .native_members
                .symbols
                .containment
                .get(&ty)
                .cloned()
                .unwrap_or_else(|| {
                    NativeCompletion::clean(
                        flags.contains(ObjectFlags::COULD_CONTAIN_TYPE_VARIABLES),
                    )
                }));
        }
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let result = if self.model.has_type_flags(ty, TypeFlags::INSTANTIABLE) {
                true
            } else {
                let alias = self.model.metadata(ty)?.alias;
                let nongeneric = match alias {
                    Some(alias)
                        if alias
                            .arguments
                            .as_ref()
                            .is_none_or(|arguments| arguments.is_empty()) =>
                    {
                        let symbol = match alias.symbol {
                            super::super::model::AliasSymbol::Bound(symbol) => symbol.into(),
                            super::super::model::AliasSymbol::Created(symbol) => {
                                ReferenceSymbol::from_property(symbol)
                            }
                        };
                        let mut top_level = false;
                        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
                            if let ReferenceDeclaration::Captured(declaration) = declaration
                                && self.source.declaration_kind(declaration)?
                                    == crate::syntax_kind::SyntaxKind::TypeAliasDeclaration
                            {
                                top_level =
                                    self.source.alias_declaration_is_top_level(declaration)?;
                                break;
                            }
                        }
                        top_level
                    }
                    _ => false,
                };
                if nongeneric {
                    false
                } else {
                    match self.model.get(ty)? {
                        TypeView::Reference(_) if flags.contains(ObjectFlags::REFERENCE) => {
                            if self.model.deferred_reference(ty).is_some() {
                                true
                            } else {
                                let mut any = false;
                                for &argument in sequence
                                    .read(self.get_type_arguments_completion(ty))?
                                    .iter()
                                {
                                    if sequence.read(
                                        self.could_contain_type_variables_completion(
                                            argument, consumer,
                                        ),
                                    )? {
                                        any = true;
                                        break;
                                    }
                                }
                                any
                            }
                        }
                        TypeView::Object(_) | TypeView::LazyObject(_) => {
                            // `couldContainTypeVariablesWorker` observes the actual
                            // symbol's function/member/container flags and declaration
                            // presence. Created method clones are genuine symbols.
                            if flags.intersects(
                                ObjectFlags::INSTANTIATION_EXPRESSION_TYPE
                                    | ObjectFlags::OBJECT_REST_TYPE
                                    | ObjectFlags::MAPPED
                                    | ObjectFlags::REVERSE_MAPPED,
                            ) {
                                true
                            } else if !flags.contains(ObjectFlags::ANONYMOUS) {
                                false
                            } else {
                                let eligible = self.type_symbol_flags(ty)?.is_some_and(|flags| {
                                    flags.function
                                        || flags.method
                                        || flags.class
                                        || flags.type_literal
                                        || flags.object_literal
                                });
                                if !eligible {
                                    false
                                } else {
                                    match self.model.type_symbol(ty) {
                                        // Native startup synthetic symbols have no
                                        // declarations; do not fabricate a source
                                        // association merely to inspect presence.
                                        TypeSymbol::Absent | TypeSymbol::Synthetic { .. } => false,
                                        _ => {
                                            let symbol = self
                                                .reference_symbol_of_type(ty, consumer)?
                                                .ok_or_else(|| {
                                                    super::super::internal(
                                                        "present native object symbol disappeared",
                                                    )
                                                })?;
                                            self.reference_symbol_has_declarations(
                                                symbol, consumer,
                                            )?
                                        }
                                    }
                                }
                            }
                        }
                        TypeView::Union(types) | TypeView::Intersection(types) => {
                            if self.model.has_type_flags(ty, TypeFlags::ENUM_LITERAL) {
                                return Ok(false);
                            }
                            let mut any = false;
                            for &child in types.iter() {
                                if sequence.read(
                                    self.could_contain_type_variables_completion(child, consumer),
                                )? {
                                    any = true;
                                    break;
                                }
                            }
                            any
                        }
                        TypeView::Reference(_)
                        | TypeView::Special(_)
                        | TypeView::Primitive(_)
                        | TypeView::Enum
                        | TypeView::UniqueSymbol(_)
                        | TypeView::Literal { .. } => false,
                        TypeView::TypeParameter(_)
                        | TypeView::Index(_)
                        | TypeView::IndexedAccess(_)
                        | TypeView::TemplateLiteral(_)
                        | TypeView::Conditional(_)
                        | TypeView::Substitution(_)
                        | TypeView::StringMapping(_) => {
                            unreachable!("instantiable type handled above")
                        }
                    }
                }
            };
            Ok(result)
        })();
        let completed = sequence.finish(result)?;
        self.model
            .mark_could_contain_type_variables(ty, *completed.value())?;
        self.native_members
            .symbols
            .containment
            .insert(ty, completed.clone());
        Ok(completed)
    }

    /// `getSymbolOfDeclaration`: the canonical member inventory already owns
    /// binding; registration preserves that actual symbol without name lookup.
    pub(super) fn register_property_symbol(
        &mut self,
        bound: BoundSymbol,
    ) -> Result<PropertySymbolId, DemandError> {
        let identity = self
            .source
            .bindings()
            .native_symbol_identity(bound)
            .map_err(|error| self.symbol_association_failure(bound, error))?;
        if let Some(&id) = self.native_members.symbols.own.get(&identity) {
            return Ok(id);
        }
        let id = self.register_property_entry(Symbol {
            actual_identity: identity,
            association: Association::Own(bound),
            read: None,
            write: None,
        });
        self.native_members.symbols.own.insert(identity, id);
        Ok(id)
    }

    /// Find an already allocated actual symbol without registering a new one
    /// or observing identity order. A retained identity must have its arena entry.
    pub(super) fn property_symbol_for_identity(
        &self,
        identity: NativeSymbolIdentity,
    ) -> Result<PropertySymbolId, DemandError> {
        self.native_members
            .symbols
            .entries
            .iter()
            .position(|symbol| symbol.actual_identity == identity)
            .map(PropertySymbolId::new)
            .ok_or_else(|| {
                super::super::internal("native symbol identity has no property arena entry").into()
            })
    }

    pub(super) fn property_raw_origin(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<Option<crate::project::binding::ReadOrigin>, DemandError> {
        Ok(match self.property_association(symbol)? {
            Association::Own(bound) => Some(crate::project::binding::ReadOrigin::Direct(bound)),
            Association::Created(data) => match &data.value {
                CreatedPropertyValue::Alias(origin) => *origin,
                _ => None,
            },
            Association::Instantiated { .. } => None,
        })
    }

    /// Raw Symbol.Members/Exports metadata copied by native symbol factories;
    /// instantiated symbols do not copy either table from their target.
    pub(super) fn property_raw_symbol_table(
        &self,
        symbol: PropertySymbolId,
        exports: bool,
    ) -> Result<Option<super::reference_symbols::ReferenceRawSymbolTable>, DemandError> {
        Ok(match self.property_association(symbol)? {
            Association::Own(bound) => self
                .source
                .bindings()
                .raw_symbol_table(bound, exports)
                .map(super::reference_symbols::ReferenceRawSymbolTable::Retained),
            Association::Created(data) => {
                if exports {
                    data.raw_exports
                } else {
                    data.raw_members
                }
            }
            Association::Instantiated { .. } => None,
        })
    }

    pub(super) fn symbol_association_failure(
        &self,
        symbol: BoundSymbol,
        error: crate::project::binding::TypeBindingFailure,
    ) -> DemandError {
        match self.source.bindings().declarations(symbol).first() {
            Some(own) => super::super::binding_failure_at(
                self.inputs,
                &super::super::location(self.inputs, own.file, own.anchor),
                error,
            ),
            None => {
                super::super::internal("native symbol lacks a retained declaration location").into()
            }
        }
    }

    fn property_association(&self, id: PropertySymbolId) -> Result<Association, DemandError> {
        self.native_members
            .symbols
            .entries
            .get(id.index())
            .map(|entry| entry.association.clone())
            .ok_or_else(|| {
                super::super::internal("property token lacks an actual native symbol").into()
            })
    }

    fn property_original(&self, id: PropertySymbolId) -> Result<BoundSymbol, DemandError> {
        match self.property_association(id)? {
            Association::Own(symbol) => Ok(symbol),
            Association::Created(_) => {
                Err(super::super::internal("created property has no canonical BoundSymbol").into())
            }
            Association::Instantiated { target, .. } => match self.property_association(target)? {
                Association::Own(symbol) => Ok(symbol),
                Association::Created(_) => Err(super::super::internal(
                    "created property target has no canonical BoundSymbol",
                )
                .into()),
                Association::Instantiated { .. } => Err(super::super::internal(
                    "instantiated property target was not flattened",
                )
                .into()),
            },
        }
    }

    /// `getTargetSymbol` observes the actual flattened transient target;
    /// ordinary symbols preserve their canonical arena token.
    pub(super) fn property_target_symbol(
        &self,
        id: PropertySymbolId,
    ) -> Result<PropertySymbolId, DemandError> {
        Ok(match self.property_association(id)? {
            Association::Own(_) | Association::Created(_) => id,
            Association::Instantiated { target, .. } => target,
        })
    }

    /// `isPrototypeProperty` includes methods synthesized by an intersection.
    pub(super) fn is_prototype_property(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<bool, DemandError> {
        Ok(self.property_symbol_flags(symbol)?.method
            || self.property_check_flags(symbol)? & checks::SYNTHETIC_METHOD != 0)
    }

    /// `getDeclaringClass` keeps the raw Parent flag test in binding, then
    /// acquires the declared type of the actual canonical parent symbol.
    pub(super) fn property_declaring_class_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let property = ReferenceSymbol::from_property(symbol);
        let Some(raw_parent) = self.reference_raw_parent_symbol(property, consumer)? else {
            return Ok(NativeCompletion::clean(None));
        };
        if !self.reference_symbol_flags(raw_parent, consumer)?.class {
            return Ok(NativeCompletion::clean(None));
        }
        let parent = self
            .reference_parent_of_symbol(property, consumer)?
            .ok_or_else(|| {
                super::super::internal(
                    "declaring class raw parent disappeared during late/merged lookup",
                )
            })?;
        Ok(self
            .declared_reference_type_completion(parent, consumer)?
            .map(Some))
    }

    pub(super) fn property_parent(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<Option<SymbolParent>, DemandError> {
        Ok(match self.property_association(symbol)? {
            Association::Own(bound) => self
                .source
                .bindings()
                .property_parent(bound)
                .map(SymbolParent::Bound),
            Association::Instantiated { target, .. } => self.property_parent(target)?,
            Association::Created(_) => self.created_property(symbol)?.parent,
        })
    }

    fn property_parent_has_local_parameters_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let Some(parent) = self
            .reference_raw_parent_symbol(super::ReferenceSymbol::from_property(symbol), consumer)?
        else {
            return Ok(NativeCompletion::clean(false));
        };
        Ok(self
            .local_type_parameters_of_symbol_completion(parent, consumer)?
            .map(|parameters| !parameters.is_empty()))
    }

    pub(super) fn property_has_declarations(
        &self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        match self.property_association(symbol)? {
            Association::Own(bound) => {
                self.reference_symbol_has_declarations(bound.into(), consumer)
            }
            Association::Instantiated { target, .. } => {
                self.property_has_declarations(target, consumer)
            }
            Association::Created(data) => match &data.value {
                CreatedPropertyValue::Alias(Some(origin)) => self
                    .reference_symbol_has_declarations(
                        ReferenceSymbol::from_origin(*origin),
                        consumer,
                    ),
                _ => Ok(!data.declarations.is_empty()),
            },
        }
    }

    /// Actual Symbol.Declarations are shared by syntax and JSON factories.
    pub(super) fn property_header_declarations(
        &self,
        symbol: PropertySymbolId,
        consumer: Option<&Location>,
    ) -> Result<Vec<super::declarations::DeclarationAssociation>, DemandError> {
        match self.property_association(symbol)? {
            Association::Own(bound) => self.reference_header_declarations(bound.into(), consumer),
            Association::Instantiated { target, .. } => {
                self.property_header_declarations(target, consumer)
            }
            Association::Created(data) => match &data.value {
                CreatedPropertyValue::Alias(Some(origin)) => self
                    .reference_header_declarations(ReferenceSymbol::from_origin(*origin), consumer),
                _ => Ok(self.copy_header_declarations(
                    data.declarations.iter().copied(),
                    "declarations/header-created",
                )),
            },
        }
    }
    pub(super) fn property_reference_declarations(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<Vec<ReferenceDeclaration>, DemandError> {
        let declarations = self.property_header_declarations(symbol, Some(consumer))?;

        declarations
            .into_iter()
            .map(|declaration| self.materialize_declaration(declaration, consumer))
            .collect::<Result<Vec<_>, _>>()
    }

    /// `getAnnotatedAccessorTypeNode` / `getAllAccessorDeclarations` select
    /// actual accessor/class-field kinds before requesting syntax recipes.
    pub(super) fn property_accessor_syntax_declarations(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<Vec<DeclarationIdentity>, DemandError> {
        let mut declarations = Vec::new();
        for declaration in self.property_header_declarations(symbol, Some(consumer))? {
            if matches!(
                self.header_declaration_kind(declaration)?,
                crate::syntax_kind::SyntaxKind::GetAccessor
                    | crate::syntax_kind::SyntaxKind::SetAccessor
                    | crate::syntax_kind::SyntaxKind::PropertyDeclaration
            ) {
                match self.materialize_declaration(declaration, consumer)? {
                    ReferenceDeclaration::Captured(declaration) => {
                        declarations.push(declaration);
                    }
                    ReferenceDeclaration::Json { .. } => {
                        return Err(super::super::internal(
                            "JSON grammar produced an accessor declaration",
                        )
                        .into());
                    }
                }
            }
        }
        Ok(declarations)
    }

    /// `forEachProperty` follows actual Synthetic check flags. Composite
    /// constituents are visited lazily so a successful callback skips later
    /// property demands; ordinary and instantiated symbols invoke it directly.
    pub(super) fn for_each_property_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
        callback: &mut impl FnMut(&mut Self, PropertySymbolId) -> NativeResult<bool>,
    ) -> NativeResult<bool> {
        if self.property_check_flags(symbol)? & checks::SYNTHETIC == 0 {
            return callback(self, symbol);
        }
        let data = self.created_property(symbol)?;
        let CreatedPropertyValue::Composite { containing, .. } = &data.value else {
            return Err(
                super::super::internal("Synthetic property lacks composite constituents").into(),
            );
        };
        let (types, _) = self.composite_constituents(*containing)?;
        let name = self.property_symbol_name_facts(symbol, consumer)?.key;
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            for ty in types.iter().copied() {
                if let Some(property) =
                    sequence.read(self.property_of_type_by_key_completion(ty, &name, consumer))?
                {
                    let symbol = property.symbol.ok_or_else(|| {
                        super::super::internal("native constituent property has no symbol")
                    })?;
                    if sequence
                        .read(self.for_each_property_completion(symbol, consumer, callback))?
                    {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    fn property_family(
        &self,
        id: PropertySymbolId,
    ) -> Result<(BoundSymbol, BoundValueType), DemandError> {
        let bound = self.property_original(id)?;
        let family = self
            .source
            .bindings()
            .value_type(bound)
            .map_err(|error| self.symbol_association_failure(bound, error))?;
        Ok((bound, family))
    }

    /// Native table key of an established property descriptor. Actual symbols
    /// own generated identity; symbol-less fixed descriptors retain exact text.
    pub(super) fn property_key(
        &self,
        property: &Property,
        consumer: &Location,
    ) -> Result<crate::project::binding::NativePropertyKey, DemandError> {
        if let Some(symbol) = property.symbol {
            return self.reference_symbol_key(ReferenceSymbol::from_property(symbol), consumer);
        }
        match &property.key {
            TypeKey::String { name } => Ok(crate::project::binding::NativePropertyKey::fixed(
                name.as_bytes(),
            )),
            TypeKey::Private { .. } | TypeKey::Symbol { .. } => {
                Err(crate::project::semantic::internal(
                    "generated property descriptor lacks actual symbol identity",
                )
                .into())
            }
        }
    }

    /// Public graph projection consumes the actual symbol facade, including
    /// binder prototypes whose declarations may add readonly/accessor flags.
    pub(in crate::project::semantic::evaluation) fn project_property_symbol(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<Property, DemandError> {
        let flags = self.property_symbol_flags(symbol)?;
        let facts = self.property_symbol_name_facts(symbol, consumer)?;
        let readonly = self.property_is_readonly(symbol, consumer)?;
        let read = self.register_property_symbol_demand(symbol, false, consumer);
        let write = if readonly {
            None
        } else {
            Some(self.register_property_symbol_demand(symbol, true, consumer))
        };
        let origins =
            self.reference_symbol_origins(super::ReferenceSymbol::from_property(symbol), consumer)?;
        let modifiers = self.property_declaration_modifiers(symbol, false, consumer)?;
        Ok(Property::new(PropertyData {
            symbol: Some(symbol),
            key: self.project_property_key(&facts.key, facts.name_type, &origins, consumer)?,
            read_type: read,
            write_type: write,
            display: PropertyDisplayDemand::Native(
                self.register_property_serialization(symbol, consumer),
            ),
            kind: if flags.get_accessor || flags.set_accessor {
                TypePropertyKind::Accessor
            } else if flags.method {
                TypePropertyKind::Method
            } else {
                TypePropertyKind::Property
            },
            optional: flags.optional,
            readonly,
            accessibility: if modifiers & modifiers::PRIVATE != 0 {
                TypeAccessibility::Private
            } else if modifiers & modifiers::PROTECTED != 0 {
                TypeAccessibility::Protected
            } else {
                TypeAccessibility::Public
            },
            origins,
        }))
    }

    /// `instantiateSymbol`: observe resolved native caches without demanding
    /// unresolved types. A fresh result is deliberately not interned by mapper.
    pub(super) fn instantiate_property_symbol_completion(
        &mut self,
        symbol: PropertySymbolId,
        mut mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<PropertySymbolId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let flags = self.property_symbol_flags(symbol)?;
            if self.model.mapper_maps_this_only(mapper)? && {
                let declarations = self.property_reference_declarations(symbol, consumer)?;
                self.reference_declarations_are_thisless(&declarations)?
            } {
                return Ok(symbol);
            }
            if let Some(read) = self.resolved_property_type(symbol, false)? {
                if !sequence.read(self.could_contain_type_variables_completion(read, consumer))?
                    && (!flags.set_accessor
                        || match self.resolved_property_type(symbol, true)? {
                            Some(write) => !sequence.read(
                                self.could_contain_type_variables_completion(write, consumer),
                            )?,
                            None => false,
                        })
                {
                    return Ok(symbol);
                }
            }
            let target = match self.property_association(symbol)? {
                Association::Own(_) | Association::Created(_) => symbol,
                Association::Instantiated {
                    target,
                    mapper: prior,
                } => {
                    mapper = self.model.combine_mappers(prior, mapper);
                    target
                }
            };
            let actual_identity = self.model.new_symbol_identity();
            let id = self.register_property_entry(Symbol {
                actual_identity,
                association: Association::Instantiated { target, mapper },
                read: None,
                write: None,
            });
            Ok(id)
        })();
        sequence.finish(result)
    }

    /// `instantiateSymbol` reads links.resolvedType/writeType directly for
    /// its identity guard. It does not call getTypeOfSymbol or replay evidence
    /// from the operation that installed the slot. Failed work installed no value.
    fn resolved_property_type(
        &self,
        symbol: PropertySymbolId,
        writing: bool,
    ) -> Result<Option<TypeId>, DemandError> {
        if let Association::Created(data) = self.property_association(symbol)?
            && matches!(&data.value, CreatedPropertyValue::Error)
        {
            // Checker initialization installs only unknownSymbol.resolvedType.
            return Ok((!writing).then(|| self.model.native_error_type()));
        }
        match self.property_value_state(symbol, writing)? {
            Some(TypeState::Installed(value)) => Ok(Some(*value.value())),
            Some(TypeState::Resolving | TypeState::Failed(_)) | None => Ok(None),
        }
    }

    /// Native SymbolFlags and CheckFlags for actual property factories.
    /// `instantiateSymbol` copies symbol flags but resets Partial check flags.
    pub(super) fn property_symbol_flags(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<PropertySymbolFlags, DemandError> {
        match self.property_association(symbol)? {
            Association::Instantiated { target, .. } => {
                let mut flags = self.property_symbol_flags(target)?;
                flags.partial = false;
                return Ok(flags);
            }
            Association::Created(_) => return Ok(self.created_property(symbol)?.flags),
            Association::Own(_) => {}
        }
        let bound = self.property_original(symbol)?;
        let flags = self
            .source
            .bindings()
            .native_property_flags(bound)
            .map_err(|error| self.symbol_association_failure(bound, error))?;
        Ok(PropertySymbolFlags {
            types: self
                .source
                .bindings()
                .type_symbol_flags(bound)
                .map_err(|error| self.symbol_association_failure(bound, error))?,
            variable: flags.variable,
            property: flags.property,
            method: flags.method,
            get_accessor: flags.getter,
            set_accessor: flags.setter,
            optional: flags.optional,
            prototype: flags.prototype,
            partial: false,
        })
    }

    pub(super) fn property_value_kind(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<Option<crate::project::binding::BoundMemberKind>, DemandError> {
        Ok(self
            .property_symbol_flags(symbol)?
            .native_flags()
            .value_kind())
    }

    /// Actual native property header, retaining the fresh transient identity
    /// and its original declaration/name facts. This does not observe ordering.
    pub(super) fn property_native_symbol(
        &self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<Arc<NativeSymbol>, DemandError> {
        self.property_native_symbol_at(symbol, Some(consumer))
    }
    pub(super) fn property_native_symbol_at(
        &self,
        symbol: PropertySymbolId,
        consumer: Option<&Location>,
    ) -> Result<Arc<NativeSymbol>, DemandError> {
        let identity = self
            .native_members
            .symbols
            .entries
            .get(symbol.index())
            .ok_or_else(|| super::super::internal("native property header is absent"))?
            .actual_identity;
        let target = self.property_target_symbol(symbol)?;
        let header = match self.property_association(target)? {
            Association::Own(bound) => self
                .source
                .bindings()
                .native_symbol(bound)
                .map_err(|error| super::super::binding_failure(self.inputs, consumer, error))?,
            Association::Created(_) => self.created_property(target)?.header.clone(),
            Association::Instantiated { .. } => {
                return Err(super::super::internal("unflattened ordering target").into());
            }
        };
        Ok(header.with_identity(identity))
    }

    /// `Symbol.Name` on an instantiated property is its target's original
    /// name. Replacing the header identity does not rewrite private/unique
    /// identities embedded in that name. Reading it needs no ordering header
    /// or declaration-list projection.
    pub(super) fn property_native_key_facts(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<NativePropertyKey, crate::project::binding::LookupFailure> {
        use crate::project::binding::LookupFailure;
        let native_failure = |failure: DemandError| LookupFailure::Native(failure.into());
        let target = self
            .property_target_symbol(symbol)
            .map_err(native_failure)?;
        match self.property_association(target).map_err(native_failure)? {
            Association::Own(bound) => self
                .source
                .bindings()
                .read_origin_key(crate::project::binding::ReadOrigin::Direct(bound))
                .map_err(Into::into),
            Association::Created(property) => property.header.key().map_err(Into::into),
            Association::Instantiated { .. } => Err(LookupFailure::Native(super::super::internal(
                "unflattened ordering target",
            ))),
        }
    }

    /// `getNamedMembers` observes the inherited symbol's original declaration
    /// and name. Transient identity stays fresh while these facts remain the
    /// canonical target's facts, independently of presentation origins.
    pub(super) fn property_ordering_facts(
        &mut self,
        symbol: PropertySymbolId,
        container: Option<BoundSymbol>,
        consumer: &Location,
    ) -> Result<(bool, std::sync::Arc<crate::project::binding::NativeSymbol>), DemandError> {
        let header = self.property_native_symbol(symbol, consumer)?;
        let contained = match container {
            Some(container) => match self.property_reference_value_declaration(symbol, consumer)? {
                Some(declaration) => {
                    self.reference_declaration_contained_by(declaration, container, consumer)?
                }
                None => false,
            },
            None => false,
        };
        Ok((contained, header))
    }

    /// The declaration-backed branch of `getDeclarationModifierFlagsFromSymbolEx`:
    /// write setter, then getter when flagged, then actual ValueDeclaration.
    pub(super) fn property_declaration_modifiers(
        &mut self,
        symbol: PropertySymbolId,
        writing: bool,
        consumer: &Location,
    ) -> Result<u32, DemandError> {
        let value = self.property_reference_value_declaration(symbol, consumer)?;
        let flags = self.property_symbol_flags(symbol)?;
        if let Some(value) = value {
            let mut selected = None;
            let declarations = self.property_reference_declarations(symbol, consumer)?;
            if writing {
                for &declaration in &declarations {
                    if self.reference_declaration_kind(declaration)?
                        == crate::syntax_kind::SyntaxKind::SetAccessor
                    {
                        selected = Some(declaration);
                        break;
                    }
                }
            }
            if selected.is_none() && flags.get_accessor {
                for &declaration in &declarations {
                    if self.reference_declaration_kind(declaration)?
                        == crate::syntax_kind::SyntaxKind::GetAccessor
                    {
                        selected = Some(declaration);
                        break;
                    }
                }
            }
            let selected = match selected {
                Some(selected) => selected,
                None => value,
            };
            let modifiers = match selected {
                ReferenceDeclaration::Captured(declaration) => {
                    self.source.declaration_modifiers(declaration)?
                }
                ReferenceDeclaration::Json { .. } => 0,
            };
            let class = self
                .property_parent(symbol)?
                .map(|parent| self.symbol_parent_is_class(parent))
                .transpose()?
                .unwrap_or(false);
            return Ok(if class {
                modifiers
            } else {
                modifiers & !crate::syntax::modifiers::ACCESSIBILITY
            });
        }
        let check = self.property_check_flags(symbol)?;
        if check & checks::SYNTHETIC != 0 {
            let access = if check & checks::PRIVATE != 0 {
                crate::syntax::modifiers::PRIVATE
            } else if check & checks::PUBLIC != 0 {
                crate::syntax::modifiers::PUBLIC
            } else {
                crate::syntax::modifiers::PROTECTED
            };
            return Ok(access
                | if check & checks::STATIC != 0 {
                    crate::syntax::modifiers::STATIC
                } else {
                    0
                });
        }
        Ok(if flags.prototype {
            crate::syntax::modifiers::PUBLIC | crate::syntax::modifiers::STATIC
        } else {
            0
        })
    }

    pub(super) fn property_header_value_declaration(
        &self,
        symbol: PropertySymbolId,
        consumer: Option<&Location>,
    ) -> Result<Option<super::declarations::DeclarationAssociation>, DemandError> {
        match self.property_association(symbol)? {
            Association::Own(bound) => {
                self.reference_header_value_declaration(bound.into(), consumer)
            }
            Association::Instantiated { target, .. } => {
                self.property_header_value_declaration(target, consumer)
            }
            Association::Created(data) => Ok(data.value_declaration),
        }
    }
    pub(super) fn property_reference_value_declaration(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<Option<ReferenceDeclaration>, DemandError> {
        self.property_header_value_declaration(symbol, Some(consumer))?
            .map(|declaration| self.materialize_declaration(declaration, consumer))
            .transpose()
    }
    /// `isReadonlySymbol`: check flags, declaration modifiers, const variables,
    /// accessors, enum members and readonly assignment declarations in order.
    pub(super) fn property_is_readonly(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.property_is_readonly_completion(symbol, consumer)?
            .admit()
    }

    pub(super) fn property_is_readonly_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let flags = self.property_symbol_flags(symbol)?;
            if self.property_check_flags(symbol)? & checks::READONLY != 0
                || flags.property
                    && self.property_declaration_modifiers(symbol, false, consumer)?
                        & modifiers::READONLY
                        != 0
                || self
                    .is_constant_variable_symbol(ReferenceSymbol::from_property(symbol), consumer)?
                || flags.get_accessor && !flags.set_accessor
                || flags.types.enum_member
            {
                return Ok(true);
            }
            for declaration in self.property_reference_declarations(symbol, consumer)? {
                if let ReferenceDeclaration::Captured(declaration) = declaration
                    && sequence.read(
                        self.is_readonly_assignment_declaration_completion(declaration, consumer),
                    )?
                {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `isReadonlyAssignmentDeclaration` checks the descriptor's value first,
    /// then writable (its initializer when applicable), otherwise its setter.
    fn is_readonly_assignment_declaration_completion(
        &mut self,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let Some(descriptor) = self.source.readonly_assignment_descriptor(declaration)? else {
                return Ok(false);
            };
            let ty = sequence
                .read(self.check_expression_cached_completion(descriptor.file, descriptor.site))?;
            if sequence
                .read(self.type_of_property_of_type_completion(ty, "value", consumer))?
                .is_some()
            {
                if let Some(writable) =
                    sequence.read(self.property_of_type_completion(ty, "writable", consumer))?
                {
                    let writable = self.property_symbol(&writable, consumer)?;
                    let initializer =
                        match self.property_reference_value_declaration(writable, consumer)? {
                            Some(ReferenceDeclaration::Captured(declaration)) => {
                                self.source.readonly_writable_initializer(declaration)?
                            }
                            Some(ReferenceDeclaration::Json { .. }) | None => None,
                        };
                    let value = match initializer {
                        Some(root) => {
                            let expression = self.source.expression_at(root.file, root.site)?;
                            sequence.read(self.check_expression_completion(
                                &expression,
                                super::contextual::CheckMode::NORMAL,
                            ))?
                        }
                        None => {
                            sequence.read(self.property_raw_type_completion(writable, consumer))?
                        }
                    };
                    return Ok(
                        matches!(self.model.get(value)?, TypeView::Literal { base: crate::types::PrimitiveName::Boolean, value } if value.equals_text("false")),
                    );
                }
                return Ok(true);
            }
            Ok(sequence
                .read(self.type_of_property_of_type_completion(ty, "set", consumer))?
                .is_none())
        })();
        sequence.finish(result)
    }

    /// `isNeverReducedProperty` / `isDiscriminantWithNeverType` and
    /// Actual ValueDeclaration pointer equality across raw and attached views.
    pub(super) fn property_value_declaration_identity(
        &self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<Option<super::declarations::HeaderDeclarationIdentity>, DemandError> {
        self.property_header_value_declaration(symbol, Some(consumer))?
            .map(|declaration| self.header_declaration_identity(declaration))
            .transpose()
    }

    /// `isConflictingPrivateProperty` preserve the short circuit before raw
    /// type acquisition and before inspecting the conflicting declaration.
    pub(super) fn is_never_reduced_property_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            Ok(
                sequence.read(self.is_discriminant_with_never_type_completion(symbol, consumer))?
                    || self.is_conflicting_private_property(symbol, consumer)?,
            )
        })();
        sequence.finish(result)
    }

    /// `isDiscriminantWithNeverType` is also a separate reporting predicate;
    /// reporting searches all discriminants before conflicting private names.
    pub(super) fn is_discriminant_with_never_type_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let flags = self.property_symbol_flags(symbol)?;
            let check = self.property_check_flags(symbol)?;
            let discriminant = checks::NON_UNIFORM | checks::LITERAL;
            if !flags.optional && check & (discriminant | checks::NEVER) == discriminant {
                let ty = sequence.read(self.property_raw_type_completion(symbol, consumer))?;
                if self.model.has_type_flags(ty, TypeFlags::NEVER) {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `isConflictingPrivateProperty` retains its ValueDeclaration-first test.
    pub(super) fn is_conflicting_private_property(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        Ok(self
            .property_reference_value_declaration(symbol, consumer)?
            .is_none()
            && self.property_check_flags(symbol)? & checks::PRIVATE != 0)
    }

    pub(super) fn property_access_auto_type(
        &mut self,
        symbol: PropertySymbolId,
        access: crate::project::semantic::source::ExpressionRoot,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        match self.property_association(self.property_target_symbol(symbol)?)? {
            Association::Own(bound) => {
                let value = self.reference_symbol_value_declaration(bound.into(), consumer)?;
                let declarations = self.reference_symbol_declarations(bound.into(), consumer)?;
                self.reference_property_access_auto_type(value, &declarations, access, consumer)
            }
            Association::Created(_) => {
                let value = self.property_reference_value_declaration(symbol, consumer)?;
                let declarations = self.property_reference_declarations(symbol, consumer)?;
                self.reference_property_access_auto_type(value, &declarations, access, consumer)
            }
            Association::Instantiated { .. } => {
                Err(super::super::internal("unflattened property access target").into())
            }
        }
    }

    /// `getTypeOfInstantiatedSymbol` stores resolvedType on this actual fresh
    /// symbol. No extra resolution push may intercept the original's cycle.
    pub(super) fn property_raw_type(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.property_raw_type_completion(symbol, consumer)?.admit()
    }

    pub(super) fn property_raw_type_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        match self.property_association(symbol)? {
            Association::Created(_) => {
                self.created_property_type_completion(symbol, false, consumer)
            }
            Association::Own(bound) => self.value_reference_type_completion(bound.into(), consumer),
            Association::Instantiated { target, mapper } => {
                if let Some(result) = self.property_value_completion(symbol, false)? {
                    return result;
                }
                let result = self
                    .property_raw_type_completion(target, consumer)
                    .and_then(|value| {
                        value.and_then(|ty| self.instantiate_type_completion(ty, mapper, consumer))
                    })
                    .map(|value| {
                        value.observe(|failure| {
                            failure.at_property(PropertyValueQuestion::Read(symbol))
                        })
                    })
                    .map_err(|failure| failure.at_property(PropertyValueQuestion::Read(symbol)));
                self.cache_property_completion(symbol, false, &result)?;
                result
            }
        }
    }

    /// `getNonMissingTypeOfSymbol` removes Missing only for Optional symbols.
    /// This is distinct from accessor writes and graph display clauses.
    pub(super) fn non_missing_property_type_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.property_raw_type_completion(symbol, consumer)?
            .and_then_value(|raw| {
                let flags = self.property_symbol_flags(symbol)?;
                self.model
                    .remove_missing_type(raw, flags.optional)
                    .map_err(|failure| failure.at(self.inputs, consumer))
            })
    }

    /// `getWriteTypeOfSymbol`: Property takes precedence over transient
    /// mapping; accessor writes map the original independent write cache.
    pub(super) fn property_write_type_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let flags = self.property_symbol_flags(symbol)?;
        match self.property_association(symbol)? {
            Association::Own(bound) => {
                let (_, family) = self.property_family(symbol)?;
                self.write_type_completion(bound, &family)
            }
            Association::Created(_) => {
                if self.property_check_flags(symbol)? & checks::SYNTHETIC_PROPERTY != 0 {
                    self.created_property_type_completion(symbol, true, consumer)
                } else if flags.property {
                    self.non_missing_property_type_completion(symbol, consumer)
                } else if flags.get_accessor || flags.set_accessor {
                    self.created_accessor_write_type_completion(symbol, consumer)
                } else {
                    self.property_raw_type_completion(symbol, consumer)
                }
            }
            Association::Instantiated { target, mapper } => {
                if flags.property {
                    return self.non_missing_property_type_completion(symbol, consumer);
                }
                if let Some(result) = self.property_value_completion(symbol, true)? {
                    return result;
                }
                let result = self
                    .property_write_type_completion(target, consumer)
                    .and_then(|value| {
                        value.and_then(|ty| self.instantiate_type_completion(ty, mapper, consumer))
                    })
                    .map(|value| {
                        value.observe(|failure| {
                            failure.at_property(PropertyValueQuestion::Write(symbol))
                        })
                    })
                    .map_err(|failure| failure.at_property(PropertyValueQuestion::Write(symbol)));
                self.cache_property_completion(symbol, true, &result)?;
                result
            }
        }
    }

    pub(super) fn property_mapper(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<Option<MapperId>, DemandError> {
        Ok(match self.property_association(symbol)? {
            Association::Own(_) => None,
            Association::Instantiated { mapper, .. } => Some(mapper),
            Association::Created(_) => self.created_property(symbol)?.mapper,
        })
    }

    pub(super) fn register_property_symbol_demand(
        &mut self,
        symbol: PropertySymbolId,
        writing: bool,
        consumer: &Location,
    ) -> TypeDemand {
        if let Some(&id) = self.native_members.symbols.demands.get(&(symbol, writing)) {
            return TypeDemand::Deferred(id);
        }
        let id = self.register_demand(TypeDemandState {
            location: Some(consumer.clone()),
            state: None,
            recipe: DemandRecipe::PropertySymbolValue {
                symbol,
                writing,
                consumer: consumer.clone(),
            },
        });
        self.native_members
            .symbols
            .demands
            .insert((symbol, writing), id);
        TypeDemand::Deferred(id)
    }
}

impl Evaluator<'_, '_> {
    /// `typeResolutionHasProperty` observes only installed native values.
    /// Resolution frames contain IDs published by this arena; indexing is
    /// infallible under that invariant and does not convert a failure to absence.
    pub(super) fn property_has_value(&self, id: PropertySymbolId, writing: bool) -> bool {
        let entry = &self.native_members.symbols.entries[id.index()];
        let state = match &entry.association {
            Association::Own(bound) => {
                if writing {
                    self.native_members.writes.get(bound)
                } else {
                    self.value_types.get(bound)
                }
            }
            Association::Created(data) if matches!(&data.value, CreatedPropertyValue::Error) => {
                return true;
            }
            Association::Created(data) if matches!(&data.value, CreatedPropertyValue::Alias(_)) => {
                self.node_types.alias_values.get(&entry.actual_identity)
            }
            Association::Created(_) | Association::Instantiated { .. } => {
                if writing {
                    entry.write.as_ref()
                } else {
                    entry.read.as_ref()
                }
            }
        };
        state.is_some_and(TypeState::has_type)
    }

    /// Native SymbolLinks cache keyed by the actual property identity.
    /// Resolving/Failed do not establish a native resolvedType/writeType;
    /// Installed does. The same operation covers canonical symbols.
    pub(super) fn property_value_state(
        &self,
        id: PropertySymbolId,
        writing: bool,
    ) -> Result<Option<TypeState>, DemandError> {
        let state = match self.property_association(id)? {
            Association::Own(bound) => {
                if writing {
                    self.native_members.writes.get(&bound)
                } else {
                    self.value_types.get(&bound)
                }
            }
            Association::Created(data) if matches!(&data.value, CreatedPropertyValue::Alias(_)) => {
                return Ok(self
                    .node_types
                    .alias_values
                    .get(&data.header.identity())
                    .cloned());
            }
            Association::Created(_) | Association::Instantiated { .. } => {
                let entry = &self.native_members.symbols.entries[id.index()];
                if writing {
                    entry.write.as_ref()
                } else {
                    entry.read.as_ref()
                }
            }
        };
        Ok(state.cloned())
    }

    pub(super) fn set_property_value_state(
        &mut self,
        id: PropertySymbolId,
        writing: bool,
        state: TypeState,
    ) -> Result<(), DemandError> {
        self.replace_property_value_state(id, writing, Some(state))
    }

    pub(super) fn clear_property_value_state(
        &mut self,
        id: PropertySymbolId,
        writing: bool,
    ) -> Result<(), DemandError> {
        self.replace_property_value_state(id, writing, None)
    }

    fn replace_property_value_state(
        &mut self,
        id: PropertySymbolId,
        writing: bool,
        state: Option<TypeState>,
    ) -> Result<(), DemandError> {
        match self.property_association(id)? {
            Association::Own(bound) => {
                if writing {
                    if let Some(state) = state {
                        self.native_members.writes.insert(bound, state);
                    } else {
                        self.native_members.writes.remove(&bound);
                    }
                } else if let Some(state) = state {
                    self.value_types.insert(bound, state);
                } else {
                    self.value_types.remove(&bound);
                }
            }
            Association::Created(data) if matches!(&data.value, CreatedPropertyValue::Alias(_)) => {
                if let Some(state) = state {
                    self.node_types
                        .alias_values
                        .insert(data.header.identity(), state);
                } else {
                    self.node_types.alias_values.remove(&data.header.identity());
                }
            }
            Association::Created(_) | Association::Instantiated { .. } => {
                let entry = &mut self.native_members.symbols.entries[id.index()];
                if writing {
                    entry.write = state;
                } else {
                    entry.read = state;
                }
            }
        }
        Ok(())
    }

    fn property_value_completion(
        &self,
        id: PropertySymbolId,
        writing: bool,
    ) -> Result<Option<NativeResult<TypeId>>, DemandError> {
        Ok(match self.property_value_state(id, writing)? {
            Some(TypeState::Installed(value)) => Some(Ok(value)),
            Some(TypeState::Failed(error)) => Some(Err(error)),
            Some(TypeState::Resolving) | None => None,
        })
    }

    fn cache_property_completion(
        &mut self,
        id: PropertySymbolId,
        writing: bool,
        result: &NativeResult<TypeId>,
    ) -> Result<(), DemandError> {
        if result.is_err() && self.property_has_value(id, writing) {
            return Ok(());
        }
        self.set_property_value_state(
            id,
            writing,
            match result {
                Ok(value) => TypeState::Installed(value.clone()),
                Err(error) => TypeState::Failed(error.clone()),
            },
        )
    }

    /// Retain each actual property symbol before exposing its arena handle.
    /// Native symbol creation/instantiation decides the header and association;
    /// this owner only publishes storage for that already selected identity.
    fn register_property_entry(&mut self, symbol: Symbol) -> PropertySymbolId {
        let id = PropertySymbolId::new(self.native_members.symbols.entries.len());
        self.native_members.symbols.entries.push(symbol);
        id
    }

    fn install_created_property(&mut self, property: CreatedProperty) -> PropertySymbolId {
        self.register_property_entry(Symbol {
            actual_identity: property.header.identity(),
            association: Association::Created(Arc::new(property)),
            read: None,
            write: None,
        })
    }

    fn created_property(&self, id: PropertySymbolId) -> Result<Arc<CreatedProperty>, DemandError> {
        match self.property_association(id)? {
            Association::Created(data) => Ok(data),
            _ => Err(crate::project::semantic::internal("symbol is not a created property").into()),
        }
    }

    pub(in crate::project::semantic::evaluation) fn property_check_flags(
        &self,
        id: PropertySymbolId,
    ) -> Result<u32, DemandError> {
        Ok(match self.property_association(id)? {
            Association::Own(symbol) => {
                if self
                    .source
                    .bindings()
                    .native_symbol_has_readonly_check(symbol)
                {
                    checks::READONLY
                } else {
                    0
                }
            }
            Association::Instantiated { target, .. } => {
                1 | (self.property_check_flags(target)?
                    & (checks::READONLY
                        | checks::LATE
                        | checks::OPTIONAL_PARAMETER
                        | checks::REST_PARAMETER))
            }
            Association::Created(data) => data.checks,
        })
    }

    /// `getTypeOfSymbolWithDeferredType` and its write counterpart normalize
    /// the stored native constituents only when that actual value is requested.
    pub(super) fn created_property_type_completion(
        &mut self,
        id: PropertySymbolId,
        writing: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let data = self.created_property(id)?;
        if matches!(&data.value, CreatedPropertyValue::NoValue) {
            return NativeCompletion::recovered(
                self.model.native_error_type(),
                super::super::unbound_recovery(
                    crate::analysis::UnboundSemanticRecovery::NonValueSymbol,
                ),
            );
        }
        if matches!(&data.value, CreatedPropertyValue::Alias(_)) {
            return self.alias_reference_value_type_completion(
                ReferenceSymbol::from_property(id),
                consumer,
            );
        }
        if matches!(&data.value, CreatedPropertyValue::Error) {
            return NativeCompletion::recovered(
                self.model.native_error_type(),
                self.node_type_error(consumer),
            );
        }
        if matches!(&data.value, CreatedPropertyValue::UnresolvedAlias) {
            // TypeAlias-only getTypeOfSymbol falls through to errorType;
            // its separate declaredType slot is not a raw value installation.
            return NativeCompletion::recovered(
                self.model.native_error_type(),
                self.node_type_error(consumer),
            );
        }
        if matches!(&data.value, CreatedPropertyValue::Mapped { .. }) {
            return self.mapped_property_type_completion(id, consumer);
        }
        if matches!(&data.value, CreatedPropertyValue::ReverseMapped { .. }) {
            return self.reverse_mapped_property_type_completion(id, consumer);
        }
        if matches!(&data.value, CreatedPropertyValue::Declaration) && !writing {
            // The complete native worker owns this clone's resolution frame
            // and can install recovery. The facade must not overwrite it.
            return self.created_property_value_completion(id, consumer);
        }
        if let Some(result) = self.property_value_completion(id, writing)? {
            return result;
        }
        let result = (|| match &data.value {
            CreatedPropertyValue::NoValue => {
                unreachable!("non-value symbol dispatch is handled above")
            }
            CreatedPropertyValue::Alias(_) => {
                unreachable!("alias uses its existing native value links")
            }
            CreatedPropertyValue::Error => unreachable!("errorType consumption is handled above"),
            CreatedPropertyValue::UnresolvedAlias => {
                unreachable!("TypeAlias value dispatch is handled above")
            }
            CreatedPropertyValue::Known => Err(crate::project::semantic::internal(
                "known property lost its installed raw type",
            )
            .into()),
            CreatedPropertyValue::Mapped { .. } | CreatedPropertyValue::ReverseMapped { .. } => {
                unreachable!("mapped symbol uses its native resolution owner")
            }
            CreatedPropertyValue::Declaration => {
                // getWriteTypeOfSymbol's declaration-backed fallback is the
                // symbol's own raw type; cloneSymbol has no target link.
                self.created_property_value_completion(id, consumer)
            }
            CreatedPropertyValue::Composite {
                containing,
                read_types,
                write_types,
            } => {
                let types = if writing {
                    match write_types {
                        Some(types) if !types.is_empty() => types.clone(),
                        _ => return self.property_raw_type_completion(id, consumer),
                    }
                } else {
                    read_types.clone()
                };
                if self.model.has_type_flags(*containing, TypeFlags::UNION) {
                    self.union_type_ex_completion(types, None, None, consumer)
                } else {
                    self.intersection_type_completion(types, consumer)
                }
            }
        })()
        .map(|value| {
            value.observe(|failure| {
                failure.at_property(if writing {
                    PropertyValueQuestion::Write(id)
                } else {
                    PropertyValueQuestion::Read(id)
                })
            })
        })
        .map_err(|failure: DemandError| {
            failure.at_property(if writing {
                crate::project::semantic::model::PropertyValueQuestion::Write(id)
            } else {
                crate::project::semantic::model::PropertyValueQuestion::Read(id)
            })
        });
        self.cache_property_completion(id, writing, &result)?;
        result
    }

    /// `getInstantiationExpressionType`: the new anonymous type owns a real
    /// flags-None symbol, with no declarations, tables, Parent, or Type slot.
    pub(super) fn create_instantiation_expression_symbol(&mut self) -> TypeSymbol {
        let header = NativeSymbol::created(
            self.model.new_symbol_identity(),
            b"\xfeinstantiationExpression".as_slice(),
            None,
        );
        let symbol = self.install_created_property(CreatedProperty {
            native_flags: None,
            flags: PropertySymbolFlags::default(),
            checks: 0,
            declarations: Vec::new(),
            value_declaration: None,
            parent: None,
            raw_members: None,
            raw_exports: None,
            name_type: None,
            header: header.clone(),
            mapper: None,
            target: None,
            synthetic_origin: None,
            spread: None,
            export_type: None,
            value: CreatedPropertyValue::NoValue,
        });
        TypeSymbol::Property {
            symbol,
            native: header,
        }
    }

    /// Native `newSymbolEx(FunctionScopedVariable, name, parameterChecks)`.
    /// A nil type permits the caller's native allocate-before-type schedule;
    /// no declaration, target, optional property flag, or parent is invented.
    pub(super) fn create_parameter_symbol(
        &mut self,
        name: &str,
        optional: bool,
        rest: bool,
        ty: Option<TypeId>,
    ) -> PropertySymbolId {
        let header = NativeSymbol::created(self.model.new_symbol_identity(), name.as_bytes(), None);
        let symbol = self.install_created_property(CreatedProperty {
            native_flags: None,
            flags: PropertySymbolFlags {
                variable: true,
                types: TypeSymbolFlags {
                    function_scoped_variable: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            checks: if rest {
                checks::REST_PARAMETER
            } else if optional {
                checks::OPTIONAL_PARAMETER
            } else {
                0
            },
            declarations: Vec::new(),
            value_declaration: None,
            parent: None,
            raw_members: None,
            raw_exports: None,
            name_type: None,
            header,
            mapper: None,
            target: None,
            synthetic_origin: None,
            spread: None,
            export_type: None,
            value: if ty.is_some() {
                CreatedPropertyValue::Known
            } else {
                CreatedPropertyValue::Declaration
            },
        });
        self.native_members.symbols.entries[symbol.index()].read =
            ty.map(|ty| TypeState::Installed(NativeCompletion::clean(ty)));
        symbol
    }

    /// `createTupleTargetType` creates ordinary transient Property symbols.
    /// The supplied raw type is installed exactly; Optional does not wrap it.
    pub(super) fn create_known_property(
        &mut self,
        name: &str,
        optional: bool,
        readonly: bool,
        ty: TypeId,
    ) -> PropertySymbolId {
        self.create_property_symbol(
            NativePropertyKey::fixed(name.as_bytes()),
            optional,
            readonly,
            Some(ty),
        )
    }

    /// `newSymbol(Property | Optional, name)` for native anonymous member
    /// factories. A nil Type slot preserves allocate-before-child acquisition.
    pub(super) fn create_property_symbol(
        &mut self,
        name: NativePropertyKey,
        optional: bool,
        readonly: bool,
        ty: Option<TypeId>,
    ) -> PropertySymbolId {
        let header = NativeSymbol::created_with_key(self.model.new_symbol_identity(), name, None);
        let id = self.install_created_property(CreatedProperty {
            native_flags: None,
            flags: PropertySymbolFlags {
                property: true,
                optional,
                ..PropertySymbolFlags::default()
            },
            checks: if readonly { checks::READONLY } else { 0 },
            declarations: Vec::new(),
            value_declaration: None,
            parent: None,
            raw_members: None,
            raw_exports: None,
            name_type: None,
            header,
            mapper: None,
            target: None,
            synthetic_origin: None,
            spread: None,
            export_type: None,
            value: CreatedPropertyValue::Known,
        });
        self.native_members.symbols.entries[id.index()].read =
            ty.map(|ty| TypeState::Installed(NativeCompletion::clean(ty)));
        id
    }

    /// Checker initialization's `unknownSymbol`: an actual transient Property
    /// with no declaration or parent, distinct from a nil lookup result.
    pub(super) fn create_unknown_symbol(&mut self) -> PropertySymbolId {
        let header = NativeSymbol::created(
            self.model.new_symbol_identity(),
            b"unknown".as_slice(),
            None,
        );
        self.install_created_property(CreatedProperty {
            native_flags: None,
            flags: PropertySymbolFlags {
                property: true,
                ..PropertySymbolFlags::default()
            },
            checks: 0,
            declarations: Vec::new(),
            value_declaration: None,
            parent: None,
            raw_members: None,
            raw_exports: None,
            name_type: None,
            header,
            mapper: None,
            target: None,
            synthetic_origin: None,
            spread: None,
            export_type: None,
            value: CreatedPropertyValue::Error,
        })
    }
}

impl CreatedProperty {
    /// Copy-on-write belongs to the record owner, not to Clone. Arc clones stay
    /// constant work; a mutation copies only the explicitly retained scalar lists.
    fn make_mut(data: &mut Arc<Self>) -> &mut Self {
        if Arc::get_mut(data).is_none() {
            *data = Arc::new(data.copy_data());
        }
        Arc::get_mut(data).expect("replacement created record has unique ownership")
    }
    fn copy_data(&self) -> Self {
        let copy_types = |values: &[TypeId]| values.to_vec();
        let value = match &self.value {
            CreatedPropertyValue::NoValue => CreatedPropertyValue::NoValue,
            CreatedPropertyValue::UnresolvedAlias => CreatedPropertyValue::UnresolvedAlias,
            CreatedPropertyValue::Alias(origin) => CreatedPropertyValue::Alias(*origin),
            CreatedPropertyValue::Error => CreatedPropertyValue::Error,
            CreatedPropertyValue::Composite {
                containing,
                read_types,
                write_types,
            } => CreatedPropertyValue::Composite {
                containing: *containing,
                read_types: copy_types(read_types),
                write_types: write_types.as_ref().map(|types| copy_types(types)),
            },
            CreatedPropertyValue::Mapped {
                containing,
                key_type,
            } => CreatedPropertyValue::Mapped {
                containing: *containing,
                key_type: *key_type,
            },
            CreatedPropertyValue::ReverseMapped {
                property_type,
                mapped_type,
                constraint_type,
            } => CreatedPropertyValue::ReverseMapped {
                property_type: *property_type,
                mapped_type: *mapped_type,
                constraint_type: *constraint_type,
            },
            CreatedPropertyValue::Declaration => CreatedPropertyValue::Declaration,
            CreatedPropertyValue::Known => CreatedPropertyValue::Known,
        };
        Self {
            native_flags: self.native_flags,
            flags: self.flags,
            checks: self.checks,
            declarations: self.declarations.to_vec(),
            value_declaration: self.value_declaration,
            parent: self.parent,
            raw_members: self.raw_members,
            raw_exports: self.raw_exports,
            name_type: self.name_type,
            header: self.header.clone(),
            mapper: self.mapper,
            target: self.target,
            synthetic_origin: self.synthetic_origin,
            spread: self.spread,
            export_type: self.export_type,
            value,
        }
    }
}
