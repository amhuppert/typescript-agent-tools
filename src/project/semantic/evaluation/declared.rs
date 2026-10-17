//! Declared roots and type-node operations within one request-local evaluator.
//! Canonical associations and checking order stay inside complete operations.

use super::super::model::{DemandError, NativeCompletion, NativeResult, TypeId};
use super::super::source::{TypeExpression, TypeExpressionForm};
use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol, TypeState};
use crate::analysis::SemanticFailureReason;
use crate::project::binding::BoundDeclaredType;

impl Evaluator<'_, '_> {
    /// `tryGetDeclaredTypeOfSymbol` selects a family from the actual header.
    /// Captured declarations supply syntax only; their node.Symbol never
    /// replaces the requested symbol's declared-type cache identity.
    pub(super) fn declared_reference_family(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &crate::references::Location,
    ) -> Result<BoundDeclaredType, DemandError> {
        use crate::syntax_kind::SyntaxKind as K;
        let flags = self
            .reference_symbol_native_flags(symbol, consumer)?
            .type_flags();
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            let ReferenceDeclaration::Captured(declaration) = declaration else {
                continue;
            };
            let kind = self.source.declaration_kind(declaration)?;
            let selected = if flags.class || flags.interface {
                matches!(
                    kind,
                    K::ClassDeclaration | K::ClassExpression | K::InterfaceDeclaration
                )
            } else if flags.type_parameter {
                kind == K::TypeParameter
            } else if flags.type_alias {
                kind == K::TypeAliasDeclaration
            } else if flags.enum_ {
                kind == K::EnumDeclaration
            } else if flags.enum_member {
                kind == K::EnumMember
            } else {
                false
            };
            if !selected {
                continue;
            }
            let carrier = self
                .source
                .reference_declaration_origin(declaration)?
                .symbol();
            let declarations = self
                .source
                .bindings()
                .native_declarations(carrier)
                .map_err(|failure| self.generic_binding_failure(failure, consumer))?;
            for syntax in declarations {
                if self.source.declaration_identity(syntax)? != declaration {
                    continue;
                }
                return Ok(if flags.class || flags.interface {
                    BoundDeclaredType::ClassOrInterface(syntax)
                } else if flags.type_parameter {
                    BoundDeclaredType::Parameter(syntax)
                } else if flags.type_alias {
                    BoundDeclaredType::Alias(syntax)
                } else if flags.enum_ {
                    BoundDeclaredType::Enum(syntax)
                } else {
                    BoundDeclaredType::EnumMember(syntax)
                });
            }
            return Err(super::super::internal(
                "declared family syntax lacks its exact retained declaration",
            )
            .into());
        }
        Err(super::super::internal("declared family has no matching captured declaration").into())
    }

    /// Alias metadata keeps a stable actual identity even when a retained
    /// transient header acquires a request-local property-arena overlay.
    pub(super) fn reference_alias_symbol(
        &self,
        symbol: ReferenceSymbol,
        consumer: &crate::references::Location,
    ) -> Result<super::super::model::AliasSymbol, DemandError> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        if let Some(retained) = crate::project::binding::BoundSymbol::from_native_identity(identity)
        {
            return Ok(retained.into());
        }
        self.reference_current_header(symbol)
            .property()
            .map(super::super::model::AliasSymbol::Created)
            .ok_or_else(|| super::super::internal("alias has no actual symbol identity").into())
    }

    /// `getDeclaredTypeOfSymbol`, `getDeclaredTypeOfTypeAlias` and
    /// `getDeclaredTypeOfClassOrInterface` cache each declared family by
    /// canonical symbol. Alias cycles use the native target/property stack;
    /// interfaces construct their deferred members through a distinct protocol.
    pub(super) fn declared_type_completion(
        &mut self,
        symbol: impl Into<ReferenceSymbol>,
        declaration: BoundDeclaredType,
    ) -> NativeResult<TypeId> {
        let symbol = symbol.into();
        let first = declaration.declaration();
        let consumer = super::super::location(self.inputs, first.file, first.anchor);
        let identity = self.reference_symbol_identity(symbol, &consumer)?;
        // Enum member writes and the final enum write have their own native
        // publication points, including reentry through constant initializers.
        match declaration {
            BoundDeclaredType::Enum(first) => {
                return self.declared_enum_type_completion(symbol, first);
            }
            BoundDeclaredType::EnumMember(first) => {
                return self.declared_enum_member_type_completion(symbol, first);
            }
            _ => {}
        }
        match self.declared_types.get(&identity) {
            Some(TypeState::Installed(value)) => return Ok(value.clone()),
            Some(TypeState::Failed(error)) => return Err(error.clone()),
            Some(TypeState::Resolving) | None => {}
        }
        let result = match declaration {
            BoundDeclaredType::Enum(_) | BoundDeclaredType::EnumMember(_) => {
                unreachable!("enum cache owner returned")
            }
            BoundDeclaredType::Parameter(first) => self
                .parameter_type(symbol, first)
                .map(NativeCompletion::clean),
            BoundDeclaredType::Alias(declaration) => {
                let consumer =
                    super::super::location(self.inputs, declaration.file, declaration.anchor);
                if !self.push_resolution(super::resolution::Key::DeclaredType(identity)) {
                    return NativeCompletion::recovered(
                        self.model.native_error_type(),
                        super::super::invalid_at_location(
                            self.inputs,
                            &consumer,
                            SemanticFailureReason::CircularType,
                            "type alias circularly references itself",
                        )
                        .into(),
                    );
                }
                self.declared_types.insert(identity, TypeState::Resolving);
                let result = self
                    .source
                    .type_alias(declaration)
                    .map_err(DemandError::from)
                    .and_then(|body| self.check_type_completion(&body));
                let resolved = self
                    .resolution
                    .pop()
                    .ok_or_else(|| super::super::internal("missing alias resolution frame"))?;
                if result.is_err() {
                    result
                } else if resolved {
                    result.and_then(|value| {
                        self.finish_alias_declaration_completion(symbol, value, &consumer)
                    })
                } else {
                    // A completed child lets pop=false establish errorType.
                    // An interrupted child never reaches diagnostic continuation.
                    let error_type = self.model.native_error_type();
                    let value = result?.map(|_| error_type);
                    value.and_then(|ty| {
                        let display =
                            self.diagnostic_symbol_display_completion(symbol, &consumer)?;
                        display.and_then(|_| {
                            NativeCompletion::recovered(
                                ty,
                                super::super::invalid_at_location(
                                    self.inputs,
                                    &consumer,
                                    SemanticFailureReason::CircularType,
                                    "type alias circularly references itself",
                                )
                                .into(),
                            )
                        })
                    })
                }
            }
            BoundDeclaredType::ClassOrInterface(declaration) => self
                .class_or_interface_type(
                    symbol,
                    &super::super::location(self.inputs, declaration.file, declaration.anchor),
                )
                .map(NativeCompletion::clean),
        };
        match result {
            Ok(value) => {
                // Native installs only if absent. Local acquisitions cannot be
                // discarded merely because reentry installed a successful value.
                if let Some(TypeState::Installed(existing)) = self.declared_types.get(&identity) {
                    return Ok(value.map(|_| *existing.value()));
                }
                self.declared_types
                    .insert(identity, TypeState::Installed(value.clone()));
                Ok(value)
            }
            Err(error) => {
                if !self
                    .declared_types
                    .get(&identity)
                    .is_some_and(TypeState::has_type)
                {
                    self.declared_types
                        .insert(identity, TypeState::Failed(error.clone()));
                }
                Err(error)
            }
        }
    }

    /// `getTypeFromTypeNodeWorker` / `getTypeFromTypeReference`: primitive
    /// keywords and nongeneric aliases consume the existing binding's
    /// established entity association. An unvisited dependency remains a
    /// pending demand, never a missing symbol or fabricated any type.
    pub(super) fn check_type(
        &mut self,
        expression: &TypeExpression,
    ) -> Result<TypeId, DemandError> {
        self.check_type_completion(expression)?.admit()
    }

    pub(super) fn check_type_completion(
        &mut self,
        expression: &TypeExpression,
    ) -> NativeResult<TypeId> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        self.check_type_worker_completion(expression)?
            .and_then(|ty| match expression.root {
                Some(root) => self.conditional_flow_type_completion(ty, root, &expression.location),
                None => Ok(NativeCompletion::clean(ty)),
            })
    }

    pub(super) fn check_type_worker_completion(
        &mut self,
        expression: &TypeExpression,
    ) -> NativeResult<TypeId> {
        match &expression.form {
            TypeExpressionForm::Reference {
                site,
                anchor,
                arguments,
                alias,
                deferred,
            } => self.type_reference_completion(
                expression,
                *site,
                *anchor,
                arguments,
                alias.as_ref(),
                deferred.as_ref(),
            ),
            TypeExpressionForm::Union { site, types, alias } => {
                self.union_type_node_completion(expression, *site, types, alias.as_ref())
            }
            TypeExpressionForm::Intersection { site, types, alias } => {
                self.intersection_type_node_completion(expression, *site, types, alias.as_ref())
            }
            TypeExpressionForm::Literal(literal) => Ok(self
                .check_expression_completion(literal, super::contextual::CheckMode::NORMAL)?
                .map(|ty| self.model.regular_literal(ty))),
            TypeExpressionForm::Unique(root) => {
                self.unique_type_node_completion(*root, &expression.location)
            }
            TypeExpressionForm::Mapped(root) => {
                self.mapped_type_node_completion(*root, &expression.location)
            }
            TypeExpressionForm::Conditional(root) => {
                self.conditional_type_node_completion(*root, &expression.location)
            }
            TypeExpressionForm::Infer(root) => {
                self.infer_type_node_completion(*root, &expression.location)
            }
            TypeExpressionForm::IntrinsicMarker => {
                Ok(NativeCompletion::clean(self.model.intrinsic_marker_type()))
            }
            TypeExpressionForm::TemplateLiteral(root) => {
                self.type_from_template_node_completion(*root, &expression.location)
            }
            TypeExpressionForm::Primitive(name) => {
                Ok(NativeCompletion::clean(self.model.primitive(*name)))
            }
            TypeExpressionForm::Special(name) => {
                Ok(NativeCompletion::clean(self.model.special(*name)))
            }
            TypeExpressionForm::This(context) => self
                .this_type(expression, context)
                .map(NativeCompletion::clean),
            TypeExpressionForm::ArrayOrTuple(recipe) => {
                self.array_or_tuple_type_node_completion(expression, recipe)
            }
            TypeExpressionForm::TupleElement(recipe) => {
                self.tuple_element_type_node_completion(expression, recipe)
            }
            TypeExpressionForm::Readonly { site, value } => {
                self.readonly_type_node_completion(expression, *site, value)
            }
            TypeExpressionForm::Query(root) => {
                self.type_from_type_query_completion(*root, &expression.location)
            }
            TypeExpressionForm::TypeLiteral { anchor, alias } => {
                self.type_literal_type_completion(expression, *anchor, alias.as_ref())
            }
            TypeExpressionForm::Keyof { site, value } => {
                self.keyof_type_node_completion(expression, *site, value)
            }
            TypeExpressionForm::IndexedAccess {
                site,
                object,
                index,
                alias,
            } => self.indexed_access_type_node_completion(
                expression,
                *site,
                object,
                index,
                alias.as_ref(),
            ),
            TypeExpressionForm::Pending { reason, message } => {
                Err(super::super::unsupported_demand_at_location(
                    self.inputs,
                    &expression.location,
                    *reason,
                    message.clone(),
                ))
            }
        }
    }
}
