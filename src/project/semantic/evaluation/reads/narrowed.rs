//! `getNarrowedTypeOfSymbol` owns correlated destructuring before identifier
//! assignment and ordinary reference-flow checking.
use super::*;
use crate::project::semantic::evaluation::{
    ReferenceDeclaration, ReferenceSymbol as SemanticSymbol, contextual::CheckMode,
};
use crate::project::semantic::model::{NativeCompletion, NativeResult, SpecialName};
use crate::project::semantic::source::{
    DeclarationIdentity, ExpressionRoot, identifiers::NarrowedSymbolRecipe,
};
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct NarrowedState {
    checking: HashSet<DeclarationIdentity>,
    initializer_undefined: HashMap<DeclarationIdentity, NativeCompletion<bool>>,
}

impl Evaluator<'_, '_> {
    /// `getNarrowedTypeOfSymbol`: acquire the raw type before inspecting the
    /// actual ValueDeclaration. A correlated read changes only its returned
    /// value; it does not overwrite the declaration's native Type slot.
    pub(super) fn narrowed_symbol_type_completion(
        &mut self,
        symbol: SemanticSymbol,
        location: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = super::super::super::model::NativeSequence::new();
        let result = (|| {
            let ty = sequence.read(self.value_reference_type_completion(symbol, consumer))?;
            let Some(ReferenceDeclaration::Captured(declaration)) =
                self.reference_symbol_value_declaration(symbol, consumer)?
            else {
                return Ok(ty);
            };
            match self.source.narrowed_symbol_recipe(declaration)? {
                NarrowedSymbolRecipe::Ordinary => Ok(ty),
                NarrowedSymbolRecipe::Binding {
                    parent,
                    pattern,
                    root,
                } => {
                    let facts = self.source.variable_facts(root)?;
                    if !(facts.kind == SyntaxKind::VariableDeclaration && facts.constant
                        || facts.kind == SyntaxKind::Parameter)
                        || !self.reads.narrowed.checking.insert(parent)
                    {
                        return Ok(ty);
                    }
                    let result = (|| {
                        let Some(parent_type) = sequence
                            .read(self.binding_parent_type_completion(parent, CheckMode::NORMAL))?
                        else {
                            return Ok(ty);
                        };
                        let constrained = self.map_type(parent_type, consumer, |checker, ty| {
                            checker.base_constraint_or_type(ty, consumer)
                        })?;
                        if !self.model.has_type_flags(constrained, TypeFlags::UNION)
                            || facts.kind == SyntaxKind::Parameter
                                && sequence
                                    .read(self.some_symbol_assigned_completion(root, consumer))?
                        {
                            return Ok(ty);
                        }
                        let flow = self
                            .reference_flow_node(location.into(), consumer)?
                            .map(Into::into);
                        let narrowed = sequence.read(self.flow_type_of_reference_completion(
                            FlowReferenceQuestion {
                                file: location.file,
                                reference: pattern.into(),
                                declared: constrained,
                                initial: constrained,
                                admitted: constrained,
                                container: None,
                                flow,
                            },
                            consumer,
                        ))?;
                        if self.model.has_type_flags(narrowed, TypeFlags::NEVER) {
                            Ok(self.model.special(SpecialName::Never))
                        } else {
                            sequence.read(self.binding_element_from_parent_completion(
                                declaration,
                                narrowed,
                                true,
                            ))
                        }
                    })();
                    self.reads.narrowed.checking.remove(&parent);
                    result
                }
                NarrowedSymbolRecipe::Parameter {
                    function,
                    parameters,
                    index,
                } => {
                    if !self.source.expression_is_context_sensitive(function)? {
                        return Ok(ty);
                    }
                    let Some(signature) = self.contextual_signature(function, consumer)? else {
                        return Ok(ty);
                    };
                    if signature.parameters.len() != 1 || !signature.flags.has_rest() {
                        return Ok(ty);
                    }
                    let mapper = self
                        .inference_context_for(function)?
                        .map(|context| self.inference_context_mapper(context, true))
                        .transpose()?;
                    let rest = sequence
                        .read(self.demand_completion(signature.parameters[0].value_type))?;
                    let rest = match mapper {
                        Some(mapper) => self.instantiate_type(rest, mapper, consumer)?,
                        None => rest,
                    };
                    let rest = self.reduced_apparent_type(rest, consumer)?;
                    if !self.model.has_type_flags(rest, TypeFlags::UNION) {
                        return Ok(ty);
                    }
                    for child in self
                        .model
                        .union_constituents(rest)
                        .expect("Union flag")
                        .iter()
                        .copied()
                    {
                        if !self.model.is_tuple_type(child) {
                            return Ok(ty);
                        }
                    }
                    for parameter in parameters {
                        if sequence
                            .read(self.some_symbol_assigned_completion(parameter, consumer))?
                        {
                            return Ok(ty);
                        }
                    }
                    let flow = self
                        .reference_flow_node(location.into(), consumer)?
                        .map(Into::into);
                    let narrowed = sequence.read(self.flow_type_of_reference_completion(
                        FlowReferenceQuestion {
                            file: location.file,
                            reference: function.into(),
                            declared: rest,
                            initial: rest,
                            admitted: rest,
                            container: None,
                            flow,
                        },
                        consumer,
                    ))?;
                    let index = self.model.regular_literal_type(
                        crate::types::PrimitiveName::Number,
                        index.to_string(),
                    );
                    self.indexed_access_type(narrowed, index, consumer)
                }
            }
        })();
        sequence.finish(result)
    }

    /// `isSomeSymbolAssigned`: binding patterns share assignment marking with
    /// ordinary named parameters and short circuit in declaration-name order.
    pub(in crate::project::semantic::evaluation::reads) fn some_symbol_assigned_completion(
        &mut self,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = super::super::super::model::NativeSequence::new();
        let result = (|| {
            for declaration in self.source.assignment_name_declarations(declaration)? {
                let symbol =
                    sequence.read(self.reference_symbol_of_declaration_completion(declaration))?;
                if sequence.read(self.is_symbol_assigned_completion(
                    symbol,
                    declaration,
                    consumer,
                ))? {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }
}

impl NarrowedState {
    pub(super) fn has_initializer_undefined(&self, declaration: DeclarationIdentity) -> bool {
        self.initializer_undefined.contains_key(&declaration)
    }
}

impl Evaluator<'_, '_> {
    /// `getCandidateDiscriminantPropertyAccess`: correlated pseudo-reference,
    /// direct receiver, then constant alias syntax. None is only native nil.
    pub(super) fn candidate_discriminant_property_access_completion(
        &mut self,
        state: &ReadState<'_>,
        root: ExpressionRoot,
    ) -> NativeResult<Option<ExpressionRoot>> {
        let mut sequence = super::super::super::model::NativeSequence::new();
        let result = (|| {
            let pseudo = match state.reference.captured_site() {
                Some(reference) => self.source.is_correlated_reference(ExpressionRoot {
                    file: state.reference.file,
                    site: reference,
                })?,
                None => false,
            };
            if pseudo {
                if root.site.kind != SyntaxKind::Identifier {
                    return Ok(None);
                }
                let symbol = sequence.read(
                    self.reference_resolved_identifier_symbol_completion(root, state.location),
                )?;
                if self.is_unknown_reference_symbol(symbol) {
                    return Ok(None);
                }
                let symbol =
                    self.reference_export_symbol_of_value_if_exported(symbol, state.location)?;
                let Some(ReferenceDeclaration::Captured(declaration)) =
                    self.reference_symbol_value_declaration(symbol, state.location)?
                else {
                    return Ok(None);
                };
                return if self.source.correlated_discriminant_declaration(
                    declaration,
                    ExpressionRoot {
                        file: state.reference.file,
                        site: state.reference.captured()?,
                    },
                )? {
                    Ok(Some(self.source.declaration_expression_root(declaration)?))
                } else {
                    Ok(None)
                };
            }
            if matches!(
                root.site.kind,
                SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
            ) {
                let receiver = required(
                    self.source.flow_syntax(root.file, root.site)?.expression,
                    "discriminant access has no receiver",
                )?;
                return Ok(sequence
                    .read(self.matches_reference_completion(
                        state,
                        ExpressionRoot {
                            file: root.file,
                            site: receiver,
                        },
                    ))?
                    .then_some(root));
            }
            if root.site.kind != SyntaxKind::Identifier {
                return Ok(None);
            }
            let symbol = sequence
                .read(self.reference_resolved_identifier_symbol_completion(root, state.location))?;
            if self.is_unknown_reference_symbol(symbol) {
                return Ok(None);
            }
            if !self.is_constant_variable_symbol(symbol, state.location)? {
                return Ok(None);
            }
            let Some(ReferenceDeclaration::Captured(declaration)) =
                self.reference_symbol_value_declaration(symbol, state.location)?
            else {
                return Ok(None);
            };
            if let Some(initializer) = self.source.discriminant_initializer(declaration, false)?
                && matches!(
                    initializer.site.kind,
                    SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
                )
            {
                let receiver = required(
                    self.source
                        .flow_syntax(initializer.file, initializer.site)?
                        .expression,
                    "discriminant alias has no receiver",
                )?;
                if sequence.read(self.matches_reference_completion(
                    state,
                    ExpressionRoot {
                        file: initializer.file,
                        site: receiver,
                    },
                ))? {
                    return Ok(Some(initializer));
                }
            }
            if let Some(initializer) = self.source.discriminant_initializer(declaration, true)?
                && matches!(
                    initializer.site.kind,
                    SyntaxKind::Identifier
                        | SyntaxKind::PropertyAccessExpression
                        | SyntaxKind::ElementAccessExpression
                )
                && sequence.read(self.matches_reference_completion(state, initializer))?
            {
                return Ok(Some(self.source.declaration_expression_root(declaration)?));
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `removeOptionalityFromDeclaredType`: only an annotated parameter with
    /// undefined in its declared type consumes the initializer-undefined slot.
    pub(super) fn remove_parameter_optionality_completion(
        &mut self,
        declared: TypeId,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if !self.inputs.semantic_options().strict_null_checks {
            return Ok(NativeCompletion::clean(declared));
        }
        let facts = self.source.variable_facts(declaration)?;
        if facts.kind != SyntaxKind::Parameter
            || facts.initializer.is_none()
            || self.type_facts(declared, TypeFacts::ONLY_UNDEFINED, consumer)? == TypeFacts::NONE
        {
            return Ok(NativeCompletion::clean(declared));
        }
        self.parameter_initializer_contains_undefined_completion(declaration, consumer)?
            .and_then_value(|contains| {
                if contains {
                    Ok(declared)
                } else {
                    self.type_with_facts(declared, TypeFacts::NOT_UNDEFINED, consumer)
                }
            })
    }

    /// `parameterInitializerContainsUndefined`: only successful pop installs
    /// this node property; rejected/false pops report and return actual true.
    fn parameter_initializer_contains_undefined_completion(
        &mut self,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if let Some(value) = self.reads.narrowed.initializer_undefined.get(&declaration) {
            return Ok(value.clone());
        }
        let key = super::super::resolution::Key::ParameterInitializerContainsUndefined(declaration);
        if !self.push_resolution(key) {
            let symbol = self.reference_symbol_of_declaration(declaration)?;
            return self
                .report_symbol_circularity_completion(symbol, consumer)
                .map(|value| value.map(|_| true));
        }
        let result =
            (|| {
                let facts = self.source.variable_facts(declaration)?;
                self.declaration_initializer_type_completion(&facts, CheckMode::NORMAL, None)?
                    .and_then_value(|ty| {
                        Ok(self.type_facts(ty, TypeFacts::ONLY_UNDEFINED, consumer)?
                            != TypeFacts::NONE)
                    })
            })();
        let popped = self.resolution.pop().ok_or_else(|| {
            super::super::super::internal("parameter initializer resolution frame missing")
        })?;
        let result = result?;
        if !popped {
            return result.and_then(|_| {
                let symbol = self.reference_symbol_of_declaration(declaration)?;
                self.report_symbol_circularity_completion(symbol, consumer)
                    .map(|value| value.map(|_| true))
            });
        }
        if let Some(installed) = self.reads.narrowed.initializer_undefined.get(&declaration) {
            return Ok(result.map(|_| *installed.value()));
        }
        self.reads
            .narrowed
            .initializer_undefined
            .insert(declaration, result.clone());
        Ok(result)
    }
}
