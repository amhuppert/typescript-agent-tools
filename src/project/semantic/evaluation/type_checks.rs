//! Demand-driven source-element checking keeps diagnostic dependencies distinct
//! from lazy type-node acquisition. No whole-file validation is triggered.
mod class_heritage;
mod classes;
mod indexes;
mod members;
mod signatures;
use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, TupleElementFlags as Flags,
    TypeId, TypeView,
};
use super::super::source::{
    ExpressionRoot, TypeExpression, TypeExpressionForm, type_checks::TypeSourceCheck,
};
use super::Evaluator;
use crate::analysis::SemanticFailureReason;

#[derive(Default)]
pub(super) struct TypeChecksState {
    class_indexes:
        std::collections::HashMap<crate::project::binding::NativeSymbolIdentity, NativeResult<()>>,
    functions: std::collections::HashMap<
        crate::project::binding::NativeSymbolIdentity,
        Result<(), DemandError>,
    >,
}

impl Evaluator<'_, '_> {
    /// `checkSourceElement` / type-node workers preserve their child schedule.
    /// An unsupported diagnostic worker is not silently replaced by getType.
    pub(super) fn check_type_source_element_completion(
        &mut self,
        root: ExpressionRoot,
    ) -> NativeResult<()> {
        enum Work {
            Visit(ExpressionRoot),
            Type(TypeExpression),
            Reference(TypeExpression),
            IndexedAccess(TypeExpression),
        }
        let mut work = vec![Work::Visit(root)];
        let mut completed = NativeCompletion::clean(());
        while let Some(task) = work.pop() {
            completed = completed.and_then(|()| {
                let root = match task {
                    Work::Type(ty) => return Ok(self.check_type_completion(&ty)?.map(|_| ())),
                    Work::Reference(ty) => {
                        return self.check_type_reference_or_import_completion(&ty);
                    }
                    Work::IndexedAccess(ty) => {
                        return self.check_type_completion(&ty)?.and_then(|value| {
                            self.check_indexed_access_index_type_completion(value, &ty.location)
                                .map(|result| result.map(|_| ()))
                        });
                    }
                    Work::Visit(root) => root,
                };
                // Native starts a fresh instantiation-count window at each
                // child entry, not at the parent's postorder continuation.
                self.reset_instantiation_count();
                let mut checked = NativeCompletion::clean(());
                let (children, result) = match self.source.type_source_check(root)? {
                    TypeSourceCheck::Visit { children, result } => (children, result),
                    TypeSourceCheck::TypeParameter => {
                        return self.check_type_parameter_declaration_completion(root);
                    }
                    TypeSourceCheck::Infer => return self.check_infer_type_completion(root),
                    TypeSourceCheck::Enum => {
                        let declaration = self.source.type_node_declaration(root)?;
                        return self.check_enum_declaration_completion(declaration);
                    }
                    TypeSourceCheck::EnumMember => {
                        let declaration = self.source.type_node_declaration(root)?;
                        return self.check_enum_member_completion(declaration);
                    }
                    TypeSourceCheck::TemplateLiteral => {
                        return self.check_template_literal_type_completion(root);
                    }
                    TypeSourceCheck::Mapped => return self.check_mapped_type_completion(root),
                    TypeSourceCheck::Signature(signature) => {
                        return self.check_signature_declaration_completion(signature);
                    }
                    TypeSourceCheck::Predicate {
                        signature,
                        asserted_type,
                    } => {
                        return self.check_type_predicate_completion(
                            root,
                            signature,
                            asserted_type,
                        );
                    }
                    TypeSourceCheck::TypeLiteral {
                        members,
                        result,
                        declaration,
                    } => {
                        return self.check_type_literal_completion(&members, &result, declaration);
                    }
                    TypeSourceCheck::Property => {
                        return self.check_property_signature_completion(root);
                    }
                    TypeSourceCheck::BindingElement => {
                        let declaration = self.source.type_node_declaration(root)?;
                        return self.check_variable_like_declaration_completion(declaration);
                    }
                    TypeSourceCheck::Method => return self.check_method_signature_completion(root),
                    TypeSourceCheck::Accessor => {
                        return self.check_accessor_declaration_completion(root);
                    }
                    TypeSourceCheck::Reference { children, result } => {
                        work.push(Work::Reference(result));
                        (children, None)
                    }
                    TypeSourceCheck::IndexedAccess { children, result } => {
                        work.push(Work::IndexedAccess(result));
                        (children, None)
                    }
                    TypeSourceCheck::Tuple {
                        elements,
                        children,
                        result,
                    } => {
                        let mut seen_optional = false;
                        let mut seen_rest = false;
                        for element in elements {
                            let mut flags = element.flags;
                            if let Some(variadic) = element.variadic {
                                let value = checked
                                    .and_then(|()| self.check_type_completion(&variadic))?
                                    .and_then_value(|ty| {
                                        if !self.is_array_like_type(ty, &variadic.location)? {
                                            return Ok(false);
                                        }
                                        if self.is_array_type(ty)?
                                            || self.model.tuple_target(ty)?.is_some_and(|tuple| {
                                                tuple.combined_flags.intersects(Flags::REST)
                                            })
                                        {
                                            flags = flags | Flags::REST;
                                        }
                                        Ok(true)
                                    })?;
                                let valid = *value.value();
                                checked = value.map(|_| ());
                                if !valid {
                                    break;
                                }
                            }
                            if flags.intersects(Flags::REST) {
                                if seen_rest {
                                    break;
                                }
                                seen_rest = true;
                            } else if flags.intersects(Flags::OPTIONAL) {
                                if seen_rest {
                                    break;
                                }
                                seen_optional = true;
                            } else if flags.intersects(Flags::REQUIRED) && seen_optional {
                                break;
                            }
                        }
                        (children, Some(result))
                    }
                    TypeSourceCheck::Pending => {
                        return Err(super::super::unsupported_demand_at_location(
                            self.inputs,
                            &super::super::location(self.inputs, root.file, root.site.span),
                            SemanticFailureReason::UnsupportedSemanticForm,
                            "native demanded type-node source-element checking is pending",
                        ));
                    }
                };
                if let Some(result) = result {
                    work.push(Work::Type(result));
                }
                for site in children.into_iter().rev() {
                    work.push(Work::Visit(ExpressionRoot {
                        file: root.file,
                        site,
                    }));
                }
                Ok(checked)
            })?;
        }
        Ok(completed)
    }

    /// `checkTypeReferenceOrImport` checks constraints before the installed
    /// symbol's deprecation, including the zero-argument deprecation path.
    /// Native error types skip both operations; diagnostic-only False does not.
    fn check_type_reference_or_import_completion(
        &mut self,
        node: &TypeExpression,
    ) -> NativeResult<()> {
        self.check_type_completion(node)?.and_then(|ty| {
            // Native skips constraints and deprecated declarations for both
            // singleton errorType and unresolved alias-bearing Any.
            if self.model.is_error_type(ty) {
                return Ok(NativeCompletion::clean(()));
            }
            let TypeExpressionForm::Reference {
                arguments, site, ..
            } = &node.form
            else {
                return Err(super::super::internal(
                    "type-reference check lacks a reference recipe",
                )
                .into());
            };
            let checked = if arguments.is_empty() {
                NativeCompletion::clean(())
            } else {
                self.check_type_argument_constraints_completion(node, arguments)?
            };
            checked.and_then_value(|()| {
                let root = ExpressionRoot {
                    file: node.file,
                    site: *site,
                };
                let Some(symbol) = self.resolved_type_reference_symbol(root) else {
                    return Ok(());
                };
                let declarations = self.reference_symbol_declarations(symbol, &node.location)?;
                for &declaration in &declarations {
                    let super::ReferenceDeclaration::Captured(declaration) = declaration else {
                        continue;
                    };
                    if self.source.is_type_declaration(declaration)?
                        && self.source.deprecated_declaration(declaration)?
                    {
                        return self.add_deprecated_suggestion_declarations(&declarations);
                    }
                }
                Ok(())
            })
        })
    }

    /// `checkTypeArgumentConstraints` acquires every parameter constraint,
    /// even after a diagnostic-only relation is false. Effective arguments
    /// and their mapper are acquired once, at the first constraint.
    fn check_type_argument_constraints_completion(
        &mut self,
        node: &TypeExpression,
        arguments: &[TypeExpression],
    ) -> NativeResult<()> {
        self.type_reference_parameters_completion(node)?
            .and_then(|parameters| {
                Ok(self
                    .check_supplied_type_argument_constraints_completion(
                        node,
                        arguments,
                        &parameters,
                    )?
                    .map(|_| ()))
            })
    }

    /// The complete `checkTypeArgumentConstraints` loop accepts either the
    /// reference owner's parameters or an uninstantiated constructor's list.
    fn check_supplied_type_argument_constraints_completion(
        &mut self,
        node: &TypeExpression,
        arguments: &[TypeExpression],
        parameters: &[TypeId],
    ) -> NativeResult<bool> {
        let mut effective = None;
        let mut related = NativeCompletion::clean(true);
        for (index, &parameter) in parameters.iter().enumerate() {
            related = related.and_then(|related| {
                self.parameter_constraint_completion(parameter, &node.location)?
                    .and_then(|constraint| {
                        let Some(constraint) = constraint else {
                            return Ok(NativeCompletion::clean(related));
                        };
                        let acquired = if effective.is_none() {
                            self.effective_type_arguments_completion(node, parameters)?
                                .and_then_value(|arguments| {
                                    let mapper = self
                                        .model
                                        .mapper(parameters.to_vec(), arguments.clone())?;
                                    effective = Some((arguments, mapper));
                                    Ok(())
                                })?
                        } else {
                            NativeCompletion::clean(())
                        };
                        acquired.and_then(|()| {
                            if !related {
                                return Ok(NativeCompletion::clean(false));
                            }
                            let (effective_arguments, mapper) =
                                effective.as_ref().expect("effective arguments acquired");
                            let error = arguments
                                .get(index)
                                .and_then(|argument| argument.root)
                                .and_then(|root| {
                                    super::relations::RelationReporting::Report.constraint_at(root)
                                });
                            self.instantiate_type_completion(constraint, *mapper, &node.location)?
                                .and_then(|constraint| {
                                    self.check_type_related_to_completion(
                                        effective_arguments[index],
                                        constraint,
                                        super::relations::RelationKind::Assignable,
                                        error,
                                        &node.location,
                                    )
                                })
                        })
                    })
            })?;
        }
        Ok(related)
    }

    /// `getTypeParametersForTypeReferenceOrImport` and
    /// `getTypeParametersForTypeAndSymbol`: alias parameters precede the
    /// actual reference target's local parameters. Outer parameters and the
    /// synthetic this parameter are not source type-argument slots.
    pub(super) fn type_reference_parameters_completion(
        &mut self,
        node: &TypeExpression,
    ) -> NativeResult<Vec<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let ty = sequence.read(self.check_type_completion(node))?;
            let TypeExpressionForm::Reference { site, anchor, .. } = &node.form else {
                return Err(
                    super::super::internal("reference parameters lack a reference recipe").into(),
                );
            };
            let symbol = sequence.read(self.symbol_from_type_reference(node, *site, *anchor))?;
            if self
                .reference_symbol_native_flags(symbol, &node.location)?
                .type_flags()
                .type_alias
            {
                let alias = self.reference_alias_symbol(symbol, &node.location)?;
                let parameters =
                    sequence.read(self.alias_parameters_completion(alias, &node.location))?;
                if !parameters.is_empty() {
                    return Ok(parameters);
                }
            }
            if let TypeView::Reference(reference) = self.model.get(ty)?
                && reference.is_reference
            {
                return Ok(self
                    .model
                    .declared_target(reference.target)?
                    .local_parameters
                    .to_vec());
            }
            Ok(Vec::new())
        })();
        sequence.finish(result)
    }

    /// `getEffectiveTypeArguments` preserves whole-list acquisition before
    /// default filling. This is distinct from call candidate constraint checks.
    fn effective_type_arguments_completion(
        &mut self,
        node: &TypeExpression,
        parameters: &[TypeId],
    ) -> NativeResult<Vec<TypeId>> {
        let TypeExpressionForm::Reference { arguments, .. } = &node.form else {
            return Err(
                super::super::internal("effective arguments lack a reference recipe").into(),
            );
        };
        self.type_arguments_from_nodes_completion(arguments)?
            .and_then(|arguments| {
                self.minimum_type_argument_count(parameters)?;
                self.fill_missing_type_arguments_completion(arguments, parameters, &node.location)
            })
    }

    /// `getEffectiveTypeArgumentAtIndex` demands only the written child when
    /// present; missing arguments invoke the distinct whole-list/default owner.
    pub(super) fn effective_type_argument_at_completion(
        &mut self,
        node: &TypeExpression,
        parameters: &[TypeId],
        index: usize,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let TypeExpressionForm::Reference { arguments, .. } = &node.form else {
                return Err(
                    super::super::internal("effective argument lacks reference recipe").into(),
                );
            };
            if let Some(argument) = arguments.get(index) {
                sequence.read(self.check_type_completion(argument))
            } else {
                sequence
                    .read(self.effective_type_arguments_completion(node, parameters))?
                    .get(index)
                    .copied()
                    .ok_or_else(|| {
                        super::super::internal("effective argument index exceeds parameter list")
                            .into()
                    })
            }
        })();
        sequence.finish(result)
    }
}
