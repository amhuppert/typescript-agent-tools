//! Native anonymous symbol identities, effective outer arguments and inventory.
//! Identity is installed before signatures; source origins never key a mapper.

use super::super::model::MapperId;
use super::super::model::{KeyBuilder, same_alias, same_types};
use super::{Evaluator, TypeState};
use crate::Span;
use crate::project::binding::{BoundMember, BoundSymbol, BoundTypeLiteral};
use crate::project::semantic::model::{
    Alias, AliasKey, DemandError, MembersView, Metadata, NativeCompletion, NativeResult,
    NativeSequence, ObjectFlags, PropertySymbolId, TypeFlags, TypeId, TypeSymbol, TypeView,
};
use crate::project::semantic::source::{
    AliasRecipe, DeclarationIdentity, DeclarationReference, TypeExpression,
};
use crate::references::Location;
use crate::resources::{CheckedKey, CheckedMap};
use std::sync::Arc;

#[derive(Clone)]
enum Association {
    Method {
        member: BoundMember,
    },
    CreatedSymbol {
        consumer: Location,
    },
    Symbol {
        symbol: super::ReferenceSymbol,
        consumer: Option<Location>,
    },
    Resolved {
        consumer: Location,
    },
    ReverseMapped {
        consumer: Location,
    },
    TypeLiteral(BoundTypeLiteral),
    Mapped {
        declaration: DeclarationIdentity,
        consumer: Location,
    },
    InstantiationExpression {
        declaration: DeclarationIdentity,
        consumer: Location,
    },
    Instantiated {
        target: TypeId,
        mapper: MapperId,
        consumer: Location,
    },
}

pub(super) struct AnonymousState {
    associations: crate::resources::FixedMap<TypeId, Association>,
    outer_parameters: crate::resources::FixedMap<DeclarationIdentity, NativeResult<Vec<TypeId>>>,
    type_literals: crate::resources::FixedMap<BoundTypeLiteral, NativeCompletion<TypeId>>,
    instantiations: CheckedMap<InstantiationKey, NativeResult<TypeId>>,
}

impl AnonymousState {
    /// Request-local backing for the existing native cache questions. Their
    /// identities, child values and provisional publication stay operation-owned.
    pub(super) fn new() -> Self {
        Self {
            associations: crate::resources::FixedMap::new(),
            outer_parameters: crate::resources::FixedMap::new(),
            type_literals: crate::resources::FixedMap::new(),
            instantiations: CheckedMap::new(),
        }
    }
}

/// `getTypeInstantiationKey` within `getObjectTypeInstantiation`: this table
/// includes the actual target that partitions native storage. Effective
/// arguments, alias and SingleSignature state retain their complete domains.
struct InstantiationKey {
    target: TypeId,
    arguments: Vec<TypeId>,
    alias: Option<AliasKey>,
    single: bool,
    fingerprint: u64,
}

impl InstantiationKey {
    fn new(target: TypeId, arguments: &[TypeId], alias: Option<&Alias>, single: bool) -> Self {
        let alias = AliasKey::of(alias);
        let mut hash = KeyBuilder::new();
        hash.fixed(&target);
        hash.types(arguments);
        hash.alias(alias.as_ref());
        hash.fixed(&single);
        Self {
            target,
            arguments: arguments.to_vec(),
            alias,
            single,
            fingerprint: hash.finish(),
        }
    }
}

impl CheckedKey for InstantiationKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    fn equivalent(&self, other: &Self) -> bool {
        self.target == other.target
            && self.single == other.single
            && { same_types(&self.arguments, &other.arguments) }
            && same_alias(self.alias.as_ref(), other.alias.as_ref())
    }
}

impl Evaluator<'_, '_> {
    /// `initializeTypeCaches` creates globalThis's anonymous type without a
    /// declaration or consumer. Its actual binding symbol may later carry
    /// merged flags; the ordinary anonymous inventory owns that whole decision.
    pub(super) fn global_this_value_type(
        &mut self,
        symbol: impl Into<super::ReferenceSymbol>,
    ) -> Result<TypeId, DemandError> {
        let symbol = symbol.into();
        let native = self.type_symbol_of_reference_at(symbol, None)?;
        let ty = self.model.anonymous_type(native, Metadata::default());
        self.anonymous.associations.insert(
            ty,
            Association::Symbol {
                symbol,
                consumer: None,
            },
        );
        Ok(ty)
    }

    pub(super) fn register_reverse_mapped_object(&mut self, ty: TypeId, consumer: &Location) {
        self.anonymous.associations.insert(
            ty,
            Association::ReverseMapped {
                consumer: consumer.clone(),
            },
        );
    }

    pub(super) fn register_mapped_object(
        &mut self,
        ty: TypeId,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) {
        self.anonymous.associations.insert(
            ty,
            Association::Mapped {
                declaration,
                consumer: consumer.clone(),
            },
        );
    }

    /// Native ObjectType.target/mapper are one association shared by anonymous
    /// and mapped types. A declaration object has no mapper and targets itself.
    pub(super) fn object_target_mapper(
        &self,
        ty: TypeId,
    ) -> Result<(TypeId, Option<MapperId>), DemandError> {
        match self.anonymous.associations.get(&ty) {
            Some(Association::Instantiated { target, mapper, .. }) => Ok((*target, Some(*mapper))),
            Some(_) => Ok((ty, None)),
            None if matches!(self.model.get(ty)?, TypeView::Object(_)) => Ok((ty, None)),
            None => Err(super::super::internal("lazy object lacks target association").into()),
        }
    }

    /// Capture native symbol facts without consuming an unavailable name or
    /// declaration until the native comparator actually reaches that branch.
    pub(super) fn type_symbol(&self, symbol: BoundSymbol, consumer: &Location) -> TypeSymbol {
        TypeSymbol::Bound {
            symbol,
            flags: self
                .source
                .bindings()
                .type_symbol_flags(symbol)
                .map_err(|error| self.generic_binding_failure(error, consumer)),
            native: self
                .source
                .bindings()
                .native_symbol(symbol)
                .map_err(|error| self.generic_binding_failure(error, consumer)),
        }
    }

    /// InstantiationExpressionType.node is copied by instantiateAnonymousType;
    /// the association retains that exact declaration through mapper wrappers.
    pub(super) fn instantiation_expression_declaration(
        &self,
        mut ty: TypeId,
    ) -> Result<DeclarationIdentity, DemandError> {
        loop {
            match self.anonymous.associations.get(&ty) {
                Some(Association::InstantiationExpression { declaration, .. }) => {
                    return Ok(*declaration);
                }
                Some(Association::Instantiated { target, .. }) => ty = *target,
                _ => {
                    return Err(
                        super::super::internal("instantiation type lacks its native node").into(),
                    );
                }
            }
        }
    }

    /// `getInstantiationExpressionType` records the actual node independently
    /// from its fresh declarationless symbol. Later object instantiation uses it.
    pub(super) fn register_instantiation_expression(
        &mut self,
        ty: TypeId,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) {
        self.anonymous.associations.insert(
            ty,
            Association::InstantiationExpression {
                declaration,
                consumer: consumer.clone(),
            },
        );
    }

    pub(super) fn register_function_type_consumer(
        &mut self,
        ty: TypeId,
        symbol: BoundSymbol,
        consumer: Location,
    ) {
        self.anonymous.associations.insert(
            ty,
            Association::Symbol {
                symbol: symbol.into(),
                consumer: Some(consumer),
            },
        );
    }

    /// `newAnonymousType` installs a real present member table while retaining
    /// the symbol and outer-parameter machinery of anonymous instantiation.
    /// A resolved factory has no separate lazy inventory or member cache.
    pub(super) fn new_resolved_anonymous_type_completion(
        &mut self,
        symbol: TypeSymbol,
        table: Option<super::structure::MemberTable>,
        members: MembersView,
        flags: ObjectFlags,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let ty = self.model.anonymous_type_ex(
            symbol,
            Metadata::default(),
            ObjectFlags::ANONYMOUS | flags,
            None,
        );
        self.anonymous.associations.insert(
            ty,
            Association::Resolved {
                consumer: consumer.clone(),
            },
        );
        self.set_structured_member_table_completion(
            ty,
            table,
            NativeCompletion::clean(members),
            consumer,
        )
        .map(|members| members.map(|_| ty))
    }

    /// `getTypeOfFuncClassEnumModuleWorker` for a Module preserves its actual
    /// source-file or namespace symbol. Member acquisition remains lazy.
    pub(super) fn module_value_type(&mut self, symbol: BoundSymbol) -> Result<TypeId, DemandError> {
        let declaration = self
            .source
            .bindings()
            .symbol_value_declaration(symbol)
            .map_err(|error| self.symbol_association_failure(symbol, error))?
            .ok_or_else(|| super::super::internal("module has no native ValueDeclaration"))?;
        let consumer = super::super::location(self.inputs, declaration.file, declaration.span);
        if self.reference_module_is_shorthand(symbol.into())? {
            return Ok(self
                .model
                .special(crate::project::semantic::model::SpecialName::Any));
        }
        let native_symbol = self.type_symbol(symbol, &consumer);
        let ty = self
            .model
            .anonymous_type(native_symbol, Metadata::default());
        self.anonymous.associations.insert(
            ty,
            Association::Symbol {
                symbol: symbol.into(),
                consumer: Some(consumer),
            },
        );
        Ok(ty)
    }

    /// `resolveAnonymousTypeMembers`, function/class/enum/module branch. The
    /// raw exports table is published before class inheritance, then indexes,
    /// calls and constructors are assigned in native order. Merged flags all
    /// participate; the intrinsic globalThis filter is identity-only.
    fn symbol_anonymous_members_completion(
        &mut self,
        ty: TypeId,
        symbol: super::ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<MembersView> {
        use super::structure::{MemberTable, MembersState};
        use crate::project::binding::NativePropertyKey;
        use crate::project::semantic::model::SpecialName;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let symbol = self.get_merged_reference_symbol(symbol, consumer)?;
            let mut exports =
                sequence.read(self.reference_exports_of_symbol_completion(symbol, consumer))?;
            let intrinsic = self.reference_symbol_identity_at(symbol, None)?
                == self.reference_symbol_identity_at(self.global_this_reference_symbol(), None)?;
            if intrinsic {
                let mut vars = Vec::new();
                if let Some(table) = exports {
                    for (key, property) in table.iter() {
                        let flags = self.reference_symbol_flags(*property, consumer)?;
                        if flags.is_block_scoped() {
                            continue;
                        }
                        if flags.value_module {
                            let declarations =
                                self.reference_symbol_declarations(*property, consumer)?;
                            let mut all_ambient = !declarations.is_empty();
                            for declaration in declarations {
                                if !matches!(declaration, super::ReferenceDeclaration::Captured(node) if self.source.declaration_is_ambient_module(node)?)
                                {
                                    all_ambient = false;
                                    break;
                                }
                            }
                            if all_ambient {
                                continue;
                            }
                        }
                        vars.push((key.clone(), *property));
                    }
                }
                exports = Some(vars.into());
            }
            let mut table = exports
                .map(|exports| {
                    exports
                        .iter()
                        .map(|(key, symbol)| {
                            Ok((
                                key.clone(),
                                self.register_reference_symbol(*symbol, consumer)?,
                            ))
                        })
                        .collect::<Result<Vec<_>, DemandError>>()
                })
                .transpose()?;
            let empty = MembersView {
                properties: Arc::from([]),
                call_signatures: Arc::from([]),
                construct_signatures: Arc::from([]),
                index_signatures: Arc::from([]),
            };
            let mut members = sequence.read(
                self.set_structured_member_table_completion(
                    ty,
                    table
                        .clone()
                        .map(|table| MemberTable::Symbols(table.into())),
                    NativeCompletion::clean(empty),
                    consumer,
                ),
            )?;
            let flags = self.reference_symbol_flags(symbol, consumer)?;
            let mut base_index = None;
            if flags.class {
                let class_type =
                    sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
                let base =
                    sequence.read(self.base_constructor_type_completion(class_type, consumer))?;
                if self.model.has_type_flags(
                    base,
                    TypeFlags::OBJECT | TypeFlags::INTERSECTION | TypeFlags::TYPE_VARIABLE,
                ) {
                    let inherited =
                        sequence.read(self.properties_of_type_completion(base, consumer))?;
                    table = self.inherit_member_table(table.take(), &inherited, consumer)?;
                    let prefix = std::mem::replace(&mut sequence, NativeSequence::new())
                        .finish(Ok(members))?;
                    members = sequence.read(
                        self.set_structured_member_table_completion(
                            ty,
                            table
                                .clone()
                                .map(|table| MemberTable::Symbols(table.into())),
                            prefix,
                            consumer,
                        ),
                    )?;
                } else if base == self.model.special(SpecialName::Any) {
                    base_index = Some(self.model.any_base_index());
                }
            }
            let index = table.as_ref().and_then(|table| {
                table
                    .iter()
                    .find(|(key, _)| key.fixed_bytes() == Some(b"\xfeindex".as_slice()))
                    .map(|(_, symbol)| *symbol)
            });
            if let Some(index) = index {
                let siblings: super::reference_symbols::ReferenceSymbolTable = table
                    .as_ref()
                    .expect("index belongs to a present table")
                    .iter()
                    .map(|(key, symbol)| {
                        (key.clone(), super::ReferenceSymbol::from_property(*symbol))
                    })
                    .collect();
                members.index_signatures = sequence
                    .read(self.index_infos_of_index_symbol_completion(
                        super::ReferenceSymbol::from_property(index),
                        Some(&siblings),
                        consumer,
                    ))?
                    .into();
            } else {
                let mut indexes = base_index.into_iter().collect::<Vec<_>>();
                if flags.enum_ {
                    let declared =
                        sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
                    let mut numeric = self.model.has_type_flags(declared, TypeFlags::ENUM);
                    if !numeric {
                        for property in members.properties.iter() {
                            let property = self.property_symbol(property, consumer)?;
                            let value = sequence
                                .read(self.property_raw_type_completion(property, consumer))?;
                            if self.model.has_type_flags(value, TypeFlags::NUMBER_LIKE) {
                                numeric = true;
                                break;
                            }
                        }
                    }
                    if numeric {
                        indexes.push(self.model.enum_number_index());
                    }
                }
                members.index_signatures = indexes.into();
            }
            let prefix =
                std::mem::replace(&mut sequence, NativeSequence::new()).finish(Ok(members))?;
            self.structure
                .members
                .insert(ty, MembersState::Resolved(prefix.clone()));
            members = sequence.read(Ok(prefix))?;
            if flags.function || flags.method {
                members.call_signatures = sequence
                    .read(self.signatures_of_symbol_completion(Some(symbol), consumer))?
                    .into();
                let prefix =
                    std::mem::replace(&mut sequence, NativeSequence::new()).finish(Ok(members))?;
                self.structure
                    .members
                    .insert(ty, MembersState::Resolved(prefix.clone()));
                members = sequence.read(Ok(prefix))?;
            }
            if flags.class {
                let class_type =
                    sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
                // Native reads symbol.Members directly here, after declared type.
                let constructor = self
                    .reference_raw_symbol_table(symbol, false)?
                    .map(|raw| self.reference_raw_table_entries(raw, consumer))
                    .transpose()?
                    .and_then(|entries| {
                        entries
                            .iter()
                            .find(|(key, _)| {
                                key == &NativePropertyKey::fixed(
                                    b"\xfeconstructor".as_slice(),
                                    self.control.as_ref(),
                                )
                            })
                            .map(|(_, symbol)| *symbol)
                    });
                let mut constructors =
                    sequence.read(self.signatures_of_symbol_completion(constructor, consumer))?;
                if constructors.is_empty() {
                    constructors = sequence
                        .read(self.default_construct_signatures_completion(class_type, consumer))?;
                }
                members.construct_signatures = constructors.into();
            }
            Ok(members)
        })();
        sequence.finish(result)
    }

    /// `getSignaturesOfSymbol` consumes the complete declaration vector; nil
    /// symbols have no signatures. Source owns overload implementation exclusion.
    pub(super) fn signatures_of_symbol_completion(
        &mut self,
        symbol: Option<super::ReferenceSymbol>,
        consumer: &Location,
    ) -> NativeResult<Vec<crate::project::semantic::model::Signature>> {
        let Some(symbol) = symbol else {
            return Ok(NativeCompletion::clean(Vec::new()));
        };
        let declarations = self.reference_symbol_declarations(symbol, consumer)?;
        let recipes =
            self.source
                .signatures_from_declarations(declarations.into_iter().filter_map(
                    |declaration| match declaration {
                        super::ReferenceDeclaration::Captured(node) => {
                            Some(DeclarationReference::Captured(node))
                        }
                        super::ReferenceDeclaration::Json { .. } => None,
                    },
                ))?;
        let mut sequence = NativeSequence::new();
        let result = recipes
            .iter()
            .map(|recipe| sequence.read(self.signature_completion(recipe)))
            .collect();
        sequence.finish(result)
    }

    /// `getTypeOfFuncClassEnumModuleWorker` allocates a class's static type
    /// before its base-constructor question. The caller caches the final type.
    pub(super) fn new_class_static_type(
        &mut self,
        symbol: BoundSymbol,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        let native_symbol = self.type_symbol(symbol, consumer);
        let ty = self
            .model
            .anonymous_type(native_symbol, Metadata::default());
        self.anonymous.associations.insert(
            ty,
            Association::Symbol {
                symbol: symbol.into(),
                consumer: Some(consumer.clone()),
            },
        );
        Ok(ty)
    }

    /// `IsEmptyAnonymousObjectType`: the resolved-field test and the actual
    /// TypeLiteral symbol table are separate OR arms. Inspecting installed
    /// fields does not acquire their cached recovery again; getMembersOfSymbol
    /// still owns the late-table question when the first arm is false.
    pub(super) fn is_empty_anonymous_object_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_empty_anonymous_object_type_completion(ty, consumer)?
            .admit()
    }

    pub(super) fn is_empty_anonymous_object_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let flags = self.model.object_flags(ty)?;
            if !flags.contains(ObjectFlags::ANONYMOUS) {
                return Ok(false);
            }
            if flags.contains(ObjectFlags::MEMBERS_RESOLVED) {
                let installed = self.resolved_members_completion(ty)?;
                let members = installed.value().as_ref().ok_or_else(|| {
                    super::super::internal(
                        "resolved anonymous object lacks installed member fields",
                    )
                })?;
                if self.model.is_empty_resolved_type(ty, members) {
                    return Ok(true);
                }
            }
            let Some(symbol) = self.reference_symbol_of_type(ty, consumer)? else {
                return Ok(false);
            };
            if !self.reference_symbol_flags(symbol, consumer)?.type_literal {
                return Ok(false);
            }
            Ok(sequence
                .read(self.reference_members_of_symbol_completion(symbol, consumer))?
                .is_none_or(|members| members.is_empty()))
        })();
        sequence.finish(result)
    }

    /// `getTypeFromTypeLiteralOrFunctionOrConstructorTypeNode` gets alias metadata
    /// before the actual node.Symbol member table. Only an empty table with no alias
    /// selects emptyTypeLiteralType. Other nodes install their own real
    /// Anonymous identity before any member value or signature is demanded.
    pub(super) fn type_literal_type_completion(
        &mut self,
        expression: &TypeExpression,
        anchor: Span,
        alias: Option<&AliasRecipe>,
    ) -> NativeResult<TypeId> {
        let bound = self
            .source
            .bindings()
            .type_literal(expression.file, anchor)
            .map_err(|error| self.generic_binding_failure(error, &expression.location))?;
        if let Some(ty) = self.anonymous.type_literals.get(&bound) {
            return Ok(ty.clone());
        }
        // getAliasSymbolForTypeNode precedes the member-table emptiness demand.
        let alias = match alias {
            Some(recipe) => self.establish_alias_completion(recipe)?.map(Some),
            None => NativeCompletion::clean(None),
        };
        let completed = alias.and_then(|alias| {
            self.reference_members_of_symbol_completion(bound.symbol.into(), &expression.location)?
                .and_then_value(|table| {
                    let empty = table.is_none_or(|table| table.is_empty());
                    let ty = if empty && alias.is_none() {
                        self.model.empty_type_literal_type()
                    } else {
                        let symbol = self.type_symbol(bound.symbol, &expression.location);
                        let ty = self.model.anonymous_type(
                            symbol,
                            Metadata {
                                alias,
                                nominal_name: None,
                            },
                        );
                        self.anonymous
                            .associations
                            .insert(ty, Association::TypeLiteral(bound));
                        ty
                    };
                    Ok(ty)
                })
        })?;
        self.anonymous
            .type_literals
            .insert(bound, completed.clone());
        Ok(completed)
    }

    /// `getAliasSymbolForTypeNode` supplies actual canonical cache identity;
    /// `getNameOfSymbolAsWritten` supplies separate exact printer spelling.
    pub(super) fn establish_alias_completion(
        &mut self,
        recipe: &AliasRecipe,
    ) -> NativeResult<Alias> {
        let consumer = self.source.declaration_location(recipe.declaration)?;
        self.reference_symbol_of_declaration_completion(recipe.declaration)?
            .and_then(|symbol| self.establish_alias_symbol_completion(symbol, &consumer))
    }

    /// getTypeAliasForTypeLiteral / getTypeArgumentsForAliasSymbol retain
    /// native canonical identity, printer spelling and actual local parameters.
    pub(super) fn establish_alias_symbol(
        &mut self,
        symbol: impl Into<super::ReferenceSymbol>,
        consumer: &Location,
    ) -> Result<Alias, DemandError> {
        self.establish_alias_symbol_completion(symbol, consumer)?
            .admit()
    }

    pub(super) fn establish_alias_symbol_completion(
        &mut self,
        symbol: impl Into<super::ReferenceSymbol>,
        consumer: &Location,
    ) -> NativeResult<Alias> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let symbol = symbol.into();
            let display_name = self.symbol_name_as_written(symbol, consumer)?;
            let name =
                String::from_utf8(self.reference_symbol_name(symbol, consumer)?).map_err(|_| {
                    self.access_pending(consumer, "alias provenance name is not scalar text")
                })?;
            let origins = self.reference_symbol_origins(symbol, consumer)?;
            let alias_symbol = self.reference_alias_symbol(symbol, consumer)?;
            let arguments =
                sequence.read(self.alias_parameters_completion(alias_symbol, consumer))?;
            Ok(Alias {
                native_symbol: self.type_symbol_of_reference(symbol, consumer)?,
                symbol: alias_symbol,
                provenance: crate::types::TypeAlias { name, origins },
                display_name,
                arguments: (!arguments.is_empty()).then(|| arguments.into()),
            })
        })();
        sequence.finish(result)
    }

    /// `getTypeOfFuncClassEnumModuleWorker` for ordinary Method symbols creates
    /// a real Anonymous before resolving signatures. Optionality uses native
    /// property semantics, preserving the underlying callable identity.
    pub(super) fn method_value_type(
        &mut self,
        symbol: BoundSymbol,
        member: &BoundMember,
    ) -> Result<TypeId, DemandError> {
        let native_symbol = self.type_symbol(
            symbol,
            &super::super::location(
                self.inputs,
                member.value_declaration.file,
                member.value_declaration.anchor,
            ),
        );
        let ty = self
            .model
            .anonymous_type(native_symbol, Metadata::default());
        self.anonymous.associations.insert(
            ty,
            Association::Method {
                member: member.clone(),
            },
        );
        let consumer = super::super::location(
            self.inputs,
            member.value_declaration.file,
            member.value_declaration.anchor,
        );
        let result = self.add_optionality(ty, true, member.optional, &consumer)?;
        self.value_types.insert(
            symbol,
            TypeState::Installed(super::super::model::NativeCompletion::clean(result)),
        );
        Ok(result)
    }

    /// `getTypeOfFuncClassEnumModuleWorker` for an actual checker-created
    /// header. The anonymous type and base-class question keep that identity.
    pub(super) fn created_function_class_enum_module_type_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let reference = super::ReferenceSymbol::from_property(symbol);
        let flags = self.reference_symbol_flags(reference, consumer)?;
        if flags.module
            && let Some(super::ReferenceDeclaration::Captured(value)) =
                self.reference_symbol_value_declaration(reference, consumer)?
            && self.source.declaration_kind(value)?
                == crate::syntax_kind::SyntaxKind::ModuleDeclaration
            && self.source.pseudo_child(value, "body")?.is_none()
        {
            return Ok(NativeCompletion::clean(
                self.model
                    .special(crate::project::semantic::model::SpecialName::Any),
            ));
        }
        // The admitted TS/JSON producer has no JS CommonJSModuleIndicator.
        let native = self.type_symbol_of_reference(reference, consumer)?;
        let ty = self.model.anonymous_type(native, Metadata::default());
        self.anonymous.associations.insert(
            ty,
            Association::CreatedSymbol {
                consumer: consumer.clone(),
            },
        );
        if flags.class {
            let mut sequence = NativeSequence::new();
            let result =
                (|| {
                    let declared = sequence
                        .read(self.declared_reference_type_completion(reference, consumer))?;
                    let base =
                        sequence.read(self.base_constructor_type_completion(declared, consumer))?;
                    let variable = if self.model.has_type_flags(base, TypeFlags::TYPE_VARIABLE) {
                        Some(base)
                    } else {
                        self.model
                            .intersection_constituents(base)
                            .and_then(|types| {
                                types.iter().copied().find(|&ty| {
                                    self.model.has_type_flags(ty, TypeFlags::TYPE_VARIABLE)
                                })
                            })
                    };
                    match variable {
                        Some(base) => sequence
                            .read(self.intersection_type_completion(vec![ty, base], consumer)),
                        None => Ok(ty),
                    }
                })();
            return sequence.finish(result);
        }
        self.add_optionality(
            ty,
            true,
            self.property_symbol_flags(symbol)?.optional,
            consumer,
        )
        .map(NativeCompletion::clean)
    }

    pub(super) fn anonymous_consumer(&self, ty: TypeId) -> Result<Location, DemandError> {
        self.anonymous_source_consumer(ty)?.ok_or_else(|| {
            super::super::internal(
                "source-free anonymous inventory requires its actual demand consumer",
            )
            .into()
        })
    }

    /// Declaration provenance is optional for native intrinsic symbols. It is
    /// never filled with the first use; inventory callers carry their own site.
    pub(super) fn anonymous_source_consumer(
        &self,
        ty: TypeId,
    ) -> Result<Option<Location>, DemandError> {
        Ok(match self.anonymous.associations.get(&ty) {
            Some(Association::Method { member, .. }) => Some(super::super::location(
                self.inputs,
                member.value_declaration.file,
                member.value_declaration.anchor,
            )),
            Some(Association::TypeLiteral(bound)) => Some(super::super::location(
                self.inputs,
                bound.declaration.file,
                bound.declaration.anchor,
            )),
            Some(Association::Symbol { consumer, .. }) => consumer.clone(),
            Some(
                Association::Resolved { consumer }
                | Association::ReverseMapped { consumer }
                | Association::CreatedSymbol { consumer, .. }
                | Association::InstantiationExpression { consumer, .. }
                | Association::Mapped { consumer, .. }
                | Association::Instantiated { consumer, .. },
            ) => Some(consumer.clone()),
            None => {
                return Err(super::super::internal(
                    "anonymous identity lacks its native declaration association",
                )
                .into());
            }
        })
    }

    /// `resolveAnonymousTypeMembers`: real method overloads use the same
    /// getSignatureFromDeclaration factory as functions. Instantiated anonymous
    /// members reuse the target inventory and native signature mapper.
    pub(super) fn anonymous_members_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<MembersView> {
        let association = self
            .anonymous
            .associations
            .get(&ty)
            .cloned()
            .ok_or_else(|| {
                super::super::internal("anonymous inventory has no native association")
            })?;
        let empty = || MembersView {
            properties: Arc::from([]),
            call_signatures: Arc::from([]),
            construct_signatures: Arc::from([]),
            index_signatures: Arc::from([]),
        };
        let mut sequence = NativeSequence::new();
        if matches!(
            &association,
            Association::TypeLiteral(_) | Association::Instantiated { .. }
        ) {
            sequence.read(self.set_structured_type_members_completion(
                ty,
                None,
                NativeCompletion::clean(empty()),
                consumer,
            ))?;
        }
        let mut final_table = None;
        let result = (|| {
            match association {
                Association::ReverseMapped { .. } => {
                    sequence.read(self.reverse_mapped_members_completion(ty))
                }
                Association::Symbol { symbol, .. } => {
                    sequence.read(self.symbol_anonymous_members_completion(ty, symbol, consumer))
                }
                Association::Method { .. } | Association::CreatedSymbol { .. } => {
                    let symbol = self
                        .reference_symbol_of_type(ty, consumer)?
                        .ok_or_else(|| super::super::internal("method type lacks symbol"))?;
                    sequence.read(self.symbol_anonymous_members_completion(ty, symbol, consumer))
                }
                Association::Resolved { .. }
                | Association::InstantiationExpression { .. }
                | Association::Mapped { .. } => Err(super::super::internal(
                    "complete inventory requested as a lazy anonymous type",
                )
                .into()),
                Association::TypeLiteral(bound) => {
                    let symbol = self.get_merged_reference_symbol(bound.symbol.into(), consumer)?;
                    let (table, members) = sequence
                        .read(self.symbol_type_members_completion(symbol, None, consumer))?;
                    final_table = Some(table.map(|_| members.properties.clone()));
                    Ok(members)
                }
                Association::Instantiated { target, mapper, .. } => {
                    // `resolveAnonymousTypeMembers` acquires and maps each
                    // inventory family before asking for the next. Eager and
                    // lazy targets share these exact inventory operations.
                    let properties = sequence
                        .read(self.properties_of_object_type_completion(target, consumer))?;
                    let properties = sequence.read(
                        self.instantiate_properties_completion(properties, mapper, consumer),
                    )?;
                    let mut calls = Vec::new();
                    for signature in
                        sequence.read(self.call_signatures_completion(target, consumer))?
                    {
                        calls.push(sequence.read(
                            self.instantiate_signature_completion(&signature, mapper, consumer),
                        )?);
                    }
                    let mut constructs = Vec::new();
                    for signature in
                        sequence.read(self.construct_signatures_completion(target, consumer))?
                    {
                        constructs.push(sequence.read(
                            self.instantiate_signature_completion(&signature, mapper, consumer),
                        )?);
                    }
                    let mut indexes = Vec::new();
                    for index in
                        sequence.read(self.index_infos_of_type_completion(target, consumer))?
                    {
                        indexes.push(sequence.read(
                            self.instantiate_index_info_completion(&index, mapper, consumer),
                        )?);
                    }
                    let members = MembersView {
                        properties: properties.into(),
                        call_signatures: calls.into(),
                        construct_signatures: constructs.into(),
                        index_signatures: indexes.into(),
                    };
                    // createInstantiatedSymbolTable returns nil for no source
                    // properties; the provisional and final nil are distinct
                    // installations owned by setStructuredTypeMembers.
                    final_table =
                        Some((!members.properties.is_empty()).then(|| members.properties.clone()));
                    Ok(members)
                }
            }
        })();
        let result = sequence.finish(result);
        let result = match final_table {
            Some(table) => result.and_then(|members| {
                self.set_structured_type_members_completion(ty, table, members, consumer)
            }),
            None => result,
        };
        // Native writes the final signature/index fields after any earlier
        // provisional table publication. Interruption preserves that publication.
        if let Ok(value) = &result {
            self.structure
                .members
                .insert(ty, super::structure::MembersState::Resolved(value.clone()));
        }
        result
    }

    /// getObjectTypeInstantiation acquires and filters actual outer parameters,
    /// then maps them directly through the combined mapper. Effective arguments,
    /// alias identity and SingleSignature state form the native target cache key.
    pub(super) fn instantiate_object_type_completion(
        &mut self,
        ty: TypeId,
        incoming: MapperId,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let prefix = (|| {
            let (target, previous) =
                if let Some(deferred) = self.model.deferred_reference(ty) {
                    let association = self.tuples.deferred.get(&ty).ok_or_else(|| {
                        super::super::internal(
                            "deferred instantiation lacks its native node association",
                        )
                    })?;
                    (association.origin, deferred.mapper)
                } else {
                    match self.anonymous.associations.get(&ty).cloned() {
                        Some(
                            Association::Method { .. }
                            | Association::CreatedSymbol { .. }
                            | Association::Symbol { .. }
                            | Association::Resolved { .. }
                            | Association::TypeLiteral(_)
                            | Association::InstantiationExpression { .. }
                            | Association::Mapped { .. },
                        ) => (ty, None),
                        Some(Association::Instantiated { target, mapper, .. }) => {
                            (target, Some(mapper))
                        }
                        Some(Association::ReverseMapped { .. }) => {
                            // instantiateTypeWorker routes this nil-symbol family to
                            // instantiateReverseMappedType, never declaration scopes.
                            return sequence
                                .read(self.instantiate_reverse_mapped_type_completion(
                                    ty, incoming, consumer,
                                ))
                                .map(|ty| (None, ty));
                        }
                        // Eager `newAnonymousType` factories already own a complete
                        // inventory. Their native target is still self, independently
                        // from our lazy-inventory association representation.
                        None if matches!(self.model.get(ty)?, TypeView::Object(_)) => (ty, None),
                        None => {
                            return Err(super::super::internal(
                                "anonymous mapper has no actual target association",
                            )
                            .into());
                        }
                    }
                };
            let parameters =
                sequence.read(self.object_outer_parameters_completion(target, consumer))?;
            if parameters.is_empty() {
                return Ok((None, ty));
            }
            let combined = previous
                .map(|mapper| self.model.combine_mappers(mapper, incoming))
                .unwrap_or(incoming);
            let arguments = parameters
                .iter()
                .map(|&parameter| {
                    sequence.read(self.map_parameter_completion(parameter, combined, consumer))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let alias = match alias {
                Some(alias) => Some(alias),
                None => sequence.read(
                    self.instantiate_type_alias_completion(
                        self.model
                            .metadata(ty)?
                            .alias
                            .map(|alias| alias.as_ref().clone()),
                        incoming,
                        consumer,
                    ),
                )?,
            };
            let target_alias = self.model.metadata(target)?.alias;
            let seed = InstantiationKey::new(target, &parameters, target_alias.as_deref(), false);
            if !self.anonymous.instantiations.contains(&seed) {
                self.anonymous
                    .instantiations
                    .insert(seed, Ok(NativeCompletion::clean(target)));
            }
            let single = self
                .model
                .object_flags(ty)?
                .intersects(ObjectFlags::SINGLE_SIGNATURE_TYPE);
            let key = InstantiationKey::new(target, &arguments, alias.as_ref(), single);
            Ok((Some((target, parameters, arguments, alias, key)), ty))
        })();
        sequence.finish(prefix)?.and_then(|(prepared, unchanged)| {
            let Some((target, parameters, arguments, alias, key)) = prepared else {
                return Ok(NativeCompletion::clean(unchanged));
            };
            if let Some(result) = self.anonymous.instantiations.get(&key) {
                return result.clone();
            }
            let mut mapper = self.model.mapper(parameters, arguments.clone())?;
            if self
                .model
                .object_flags(target)?
                .intersects(ObjectFlags::SINGLE_SIGNATURE_TYPE)
            {
                mapper = self.model.combine_mappers(mapper, incoming);
            }
            let result = if self.model.deferred_reference(target).is_some() {
                self.instantiate_deferred_reference_completion(ty, mapper, alias)
            } else if self
                .model
                .object_flags(target)?
                .intersects(ObjectFlags::MAPPED)
            {
                self.instantiate_mapped_type_completion(target, mapper, alias, consumer)
            } else {
                self.instantiate_anonymous_type_completion(target, mapper, alias, consumer)
            }?;
            // Native publishes before the computed-flag suffix. A later
            // interruption cannot erase this actual installation, and a hit
            // does not replay work that native skips after that publication.
            self.anonymous
                .instantiations
                .insert(key, Ok(result.clone()));
            result.and_then(|result| {
                let mut sequence = NativeSequence::new();
                let flags = (|| {
                    if !self
                        .model
                        .has_type_flags(result, TypeFlags::OBJECT_FLAGS_TYPE)
                        || self
                            .model
                            .object_flags(result)?
                            .intersects(ObjectFlags::COULD_CONTAIN_TYPE_VARIABLES_COMPUTED)
                    {
                        return Ok(result);
                    }
                    let mut contains = false;
                    for argument in arguments {
                        if sequence.read(
                            self.could_contain_type_variables_completion(argument, consumer),
                        )? {
                            contains = true;
                            break;
                        }
                    }
                    if !self
                        .model
                        .object_flags(result)?
                        .intersects(ObjectFlags::COULD_CONTAIN_TYPE_VARIABLES_COMPUTED)
                        && (self.model.object_flags(result)?.intersects(
                            ObjectFlags::MAPPED | ObjectFlags::ANONYMOUS | ObjectFlags::REFERENCE,
                        ) || !contains)
                    {
                        self.model
                            .mark_could_contain_type_variables(result, contains)?;
                    }
                    Ok(result)
                })();
                sequence.finish(flags)
            })
        })
    }

    /// `instantiateAnonymousType` creates a fresh native object, with mapped
    /// parameter cloning before alias substitution and final target publication.
    pub(super) fn instantiate_anonymous_type_completion(
        &mut self,
        target: TypeId,
        mut mapper: MapperId,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            // The shared newType allocation owner clears inherited caches.
            let flags = self.model.object_flags(target)? | ObjectFlags::INSTANTIATED;
            let result = self.model.anonymous_type_ex(
                self.model.type_symbol(target),
                Metadata::default(),
                flags,
                None,
            );
            if flags.intersects(ObjectFlags::MAPPED) {
                let original = sequence.read(self.mapped_parameter_completion(target, consumer))?;
                let fresh = self.model.clone_parameter(original)?;
                let local = self.model.mapper(vec![original], vec![fresh])?;
                mapper = self.model.combine_mappers(local, mapper);
                self.initialize_parameter_clone(fresh, original, mapper, consumer)?;
                self.clone_mapped_links(result, target, fresh)?;
            }
            let alias = match alias {
                Some(alias) => Some(alias),
                None => sequence.read(
                    self.instantiate_type_alias_completion(
                        self.model
                            .metadata(target)?
                            .alias
                            .map(|alias| alias.as_ref().clone()),
                        mapper,
                        consumer,
                    ),
                )?,
            };
            if let Some(arguments) = alias.as_ref().and_then(|alias| alias.arguments.as_ref()) {
                let flags = self.model.propagating_flags(arguments, TypeFlags::NONE);
                self.model.add_object_flags(result, flags)?;
            }
            self.model.install_lazy_object_alias(result, alias)?;
            self.model.install_object_mapper(result, mapper)?;
            self.anonymous.associations.insert(
                result,
                Association::Instantiated {
                    target,
                    mapper,
                    consumer: consumer.clone(),
                },
            );
            Ok(result)
        })();
        sequence.finish(result)
    }

    fn object_outer_parameters_completion(
        &mut self,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        let deferred = self.tuples.deferred.get(&target).cloned();
        let consumer = match &deferred {
            Some(deferred) => deferred.consumer.clone(),
            None => consumer.clone(),
        };
        let node_filter = deferred.is_some()
            || matches!(
                self.anonymous.associations.get(&target),
                Some(Association::InstantiationExpression { .. })
            );
        // getObjectTypeInstantiation uses symbol.Declarations[0] and filters
        // against every declaration. A method symbol can also own a type
        // parameter declaration that the member-only projection omits.
        let declarations: Vec<DeclarationReference> = if let Some(deferred) = deferred {
            vec![DeclarationReference::Captured(deferred.recipe.declaration)]
        } else if let Some(
            Association::InstantiationExpression { declaration, .. }
            | Association::Mapped { declaration, .. },
        ) = self.anonymous.associations.get(&target)
        {
            vec![DeclarationReference::Captured(*declaration)]
        } else {
            match self.model.type_symbol(target) {
                TypeSymbol::Bound { symbol, .. } => self
                    .source
                    .symbol_declaration_references(symbol, &consumer)?,
                TypeSymbol::Property { symbol, .. } => {
                    let declarations = self.property_reference_declarations(symbol, &consumer)?;
                    if matches!(
                        declarations.first(),
                        Some(super::ReferenceDeclaration::Json { .. })
                    ) {
                        return Ok(super::super::model::NativeCompletion::clean(Vec::new()));
                    }
                    declarations
                        .into_iter()
                        .filter_map(|declaration| match declaration {
                            super::ReferenceDeclaration::Captured(declaration) => {
                                Some(DeclarationReference::Captured(declaration))
                            }
                            super::ReferenceDeclaration::Json { .. } => None,
                        })
                        .collect()
                }
                TypeSymbol::Absent | TypeSymbol::Synthetic { .. } => {
                    return Err(super::super::internal(
                        "anonymous type lacks native symbol declarations",
                    )
                    .into());
                }
            }
        };
        let declaration = *declarations
            .first()
            .ok_or_else(|| super::super::internal("anonymous symbol lacks native declaration"))?;
        let node = self.source.resolve_declaration_reference(declaration)?;
        if let Some(parameters) = self.anonymous.outer_parameters.get(&node) {
            return parameters.clone();
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut parameters = sequence.read(self.outer_type_parameters_completion(
                declaration,
                true,
                &consumer,
            ))?;
            let has_alias_arguments = self
                .model
                .metadata(target)?
                .alias
                .as_ref()
                .and_then(|alias| alias.arguments.as_ref())
                .is_some_and(|arguments| !arguments.is_empty());
            let filter = if has_alias_arguments {
                false
            } else if node_filter {
                true
            } else {
                // The native decision uses the actual symbol's flags, including
                // already-resolved object-rest factories and merged symbols.
                match self.model.type_symbol(target) {
                    TypeSymbol::Bound { symbol, flags, .. } => {
                        let flags = flags?;
                        flags.type_literal
                            || self
                                .source
                                .bindings()
                                .native_property_flags(symbol)
                                .map_err(|error| self.generic_binding_failure(error, &consumer))?
                                .method
                    }
                    TypeSymbol::Property { symbol, .. } => {
                        self.property_symbol_flags(symbol)?.method
                    }
                    TypeSymbol::Synthetic { flags, .. } => flags.type_literal,
                    TypeSymbol::Absent => false,
                }
            };
            if filter {
                let mut retained = Vec::new();
                for ty in parameters {
                    if sequence.read(self.parameter_possibly_referenced_completion(
                        &declarations,
                        ty,
                        &consumer,
                    ))? {
                        retained.push(ty);
                    }
                }
                parameters = retained;
            }
            Ok(parameters)
        })();
        let result = sequence.finish(result);
        self.anonymous.outer_parameters.insert(node, result.clone());
        result
    }
}

impl Evaluator<'_, '_> {
    /// Native anonymous Type.Mapper is the retained instantiated factory input.
    pub(super) fn anonymous_type_mapper(&self, ty: TypeId) -> Result<MapperId, DemandError> {
        match self.anonymous.associations.get(&ty) {
            Some(Association::Instantiated { mapper, .. }) => Ok(*mapper),
            _ => {
                Err(super::super::internal("instantiated array member has no native mapper").into())
            }
        }
    }
}

impl Evaluator<'_, '_> {
    pub(super) fn new_resolved_anonymous_type(
        &mut self,
        symbol: TypeSymbol,
        table: Option<super::structure::MemberTable>,
        members: MembersView,
        flags: ObjectFlags,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.new_resolved_anonymous_type_completion(symbol, table, members, flags, consumer)?
            .admit()
    }
}
