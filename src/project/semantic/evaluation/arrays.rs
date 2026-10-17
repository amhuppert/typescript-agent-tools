//! Array expressions compose contextual checking with the shared tuple,
//! iteration, indexed-access and subtype-reduction owners.

use super::super::model::{
    AccessFlags, DemandError, NativeCompletion, NativeResult, NativeSequence, SpecialName,
    TupleElementFlags as Flags, TupleElementInfo, TypeFlags, TypeId, UnionReduction,
};
use super::super::source::{Expression, ExpressionRoot, arrays::ConstArgument};
use super::Evaluator;
use super::contextual::{CheckMode, ContextFlags};
use super::iteration::IterationUse;
use super::relations::RelationKind;
use crate::references::Location;
use crate::types::PrimitiveName;

struct ArrayElements {
    types: Vec<TypeId>,
    infos: Vec<TupleElementInfo>,
    destructuring: bool,
    constant: bool,
    tuple_context: bool,
    contextual: Option<TypeId>,
}

impl Evaluator<'_, '_> {
    /// `checkArrayLiteral`: the cached contextual entry covers acquisition of
    /// all elements and is popped before tuple normalization or subtype union.
    pub(super) fn check_array_literal(
        &mut self,
        root: ExpressionRoot,
        mode: CheckMode,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        let mut elements = self.with_cached_contextual_type(root, consumer, |checker| {
            checker.array_literal_elements(root, mode, consumer)
        })?;
        if elements.destructuring {
            return self.create_tuple_type_ex(elements.types, elements.infos, false, consumer);
        }
        if mode.contains(CheckMode::FORCE_TUPLE) || elements.constant || elements.tuple_context {
            let mut readonly = elements.constant;
            if readonly && let Some(contextual) = elements.contextual {
                let types = self
                    .model
                    .union_constituents(contextual)
                    .map(|types| types.to_vec())
                    .unwrap_or_else(|| vec![contextual]);
                for ty in types {
                    if self.is_mutable_array_like_type(ty, consumer)? {
                        readonly = false;
                        break;
                    }
                }
            }
            let tuple =
                self.create_tuple_type_ex(elements.types, elements.infos, readonly, consumer)?;
            return self.model.array_literal_type(tuple);
        }
        let element = if elements.types.is_empty() {
            self.model.empty_array_element()
        } else {
            for (ty, info) in elements.types.iter_mut().zip(&elements.infos) {
                if info.flags.intersects(Flags::VARIADIC) {
                    let number = self.model.primitive(PrimitiveName::Number);
                    *ty = self
                        .indexed_access_without_node_optional(
                            *ty,
                            number,
                            AccessFlags::NONE,
                            consumer,
                        )?
                        .unwrap_or_else(|| self.model.special(SpecialName::Any));
                }
            }
            self.union_type_reduced(
                elements.types,
                UnionReduction::Subtype,
                None,
                None,
                consumer,
            )?
        };
        let array = if elements.constant {
            self.readonly_array_type(element, consumer)?
        } else {
            self.array_type(element, consumer)?
        };
        self.model.array_literal_type(array)
    }

    fn array_literal_elements(
        &mut self,
        root: ExpressionRoot,
        mode: CheckMode,
        consumer: &Location,
    ) -> Result<ArrayElements, DemandError> {
        let recipe = self.source.array_literal_recipe(root)?;
        let constant = self.is_const_context(root, consumer)?;
        let contextual = self.apparent_contextual_type(root, ContextFlags::NONE, consumer)?;
        let mut tuple_context = recipe.spread_into_call;
        if !tuple_context && let Some(contextual) = contextual {
            let types = self
                .model
                .union_constituents(contextual)
                .map(|types| types.to_vec())
                .unwrap_or_else(|| vec![contextual]);
            for ty in types {
                if self.is_tuple_like_type(ty, consumer)? {
                    tuple_context = true;
                    break;
                }
                if self.is_generic_mapped_type(ty, consumer)?
                    && self.mapped_established_name(ty)?.is_none()
                {
                    let target = self.object_target_mapper(ty)?.0;
                    if self.homomorphic_type_variable(target, consumer)?.is_some() {
                        tuple_context = true;
                        break;
                    }
                }
            }
        }
        let mut types = Vec::with_capacity(recipe.elements.len());
        let mut infos = Vec::with_capacity(recipe.elements.len());
        let mut omitted = false;
        for element in recipe.elements {
            let location = &element.expression.location;
            let (ty, flags) = if element.spread {
                let spread = self.check_expression_ex(&element.expression, mode)?;
                if self.is_array_like_type(spread, location)? {
                    (spread, Flags::VARIADIC)
                } else if recipe.destructuring {
                    let number = self.model.primitive(PrimitiveName::Number);
                    let element = match self.index_type_of_type(spread, number, location)? {
                        Some(element) => element,
                        None => self
                            .iterated_type_or_element_type_completion(
                                spread,
                                IterationUse::Destructuring,
                                None,
                                false,
                                location,
                            )?
                            .admit()?
                            .unwrap_or_else(|| self.model.special(SpecialName::Unknown)),
                    };
                    (element, Flags::REST)
                } else {
                    (
                        self.checked_iteration_type_completion(
                            spread,
                            IterationUse::Spread,
                            Some(
                                element
                                    .expression
                                    .root
                                    .ok_or_else(|| {
                                        super::super::internal("spread operand has no actual node")
                                    })?
                                    .into(),
                            ),
                            location,
                        )?
                        .admit()?,
                        Flags::REST,
                    )
                }
            } else if self.inputs.semantic_options().exact_optional_property_types
                && element.omitted
            {
                omitted = true;
                (self.model.missing_type(), Flags::OPTIONAL)
            } else {
                let ty = self.check_expression_for_mutable_location(&element.expression, mode)?;
                let optional = self.add_optionality(ty, true, omitted, location)?;
                if tuple_context
                    && mode.contains(CheckMode::INFERENTIAL)
                    && !mode.contains(CheckMode::SKIP_CONTEXT_SENSITIVE)
                {
                    let element_root = element.expression.root.ok_or_else(|| {
                        super::super::internal("array element has no source identity")
                    })?;
                    if self.source.expression_is_context_sensitive(element_root)? {
                        let context = self.inference_context_for(root)?.ok_or_else(|| {
                            super::super::internal("inferential array has no inference context")
                        })?;
                        self.add_intra_expression_inference_site(context, element_root, ty);
                    }
                }
                (
                    optional,
                    if omitted {
                        Flags::OPTIONAL
                    } else {
                        Flags::REQUIRED
                    },
                )
            };
            types.push(ty);
            infos.push(TupleElementInfo { flags, label: None });
        }
        Ok(ArrayElements {
            types,
            infos,
            destructuring: recipe.destructuring,
            constant,
            tuple_context,
            contextual,
        })
    }

    /// Product admission for the shared mutable array-like predicate.
    pub(super) fn is_mutable_array_like_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_mutable_array_like_type_completion(ty, consumer)?
            .admit()
    }

    /// `isMutableArrayLikeType` preserves mutable identity before the
    /// Any/Nullable exclusion and the independent Array<any> relation.
    pub(super) fn is_mutable_array_like_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if sequence.read(self.is_mutable_array_or_tuple_completion(ty))? {
                return Ok(true);
            }
            if self
                .model
                .has_type_flags(ty, TypeFlags::ANY | TypeFlags::NULLABLE)
            {
                return Ok(false);
            }
            let array = self.installed_any_array_type()?;
            sequence.read(self.type_related_completion(
                ty,
                array,
                RelationKind::Assignable,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `checkExpressionForMutableLocation` checks first, then const context,
    /// assertion identity, and contextual widening in that order.
    pub(super) fn check_expression_for_mutable_location(
        &mut self,
        expression: &Expression,
        mode: CheckMode,
    ) -> Result<TypeId, DemandError> {
        self.check_expression_for_mutable_location_completion(expression, mode)?
            .admit()
    }

    pub(super) fn check_expression_for_mutable_location_completion(
        &mut self,
        expression: &Expression,
        mode: CheckMode,
    ) -> NativeResult<TypeId> {
        self.check_expression_completion(expression, mode)?
            .and_then(|ty| {
                let root = expression.root.ok_or_else(|| {
                    super::super::internal("mutable expression has no source identity")
                })?;
                self.is_const_context_completion(root, &expression.location)?
                    .and_then(|constant| {
                        if constant {
                            return Ok(NativeCompletion::clean(self.model.regular_literal(ty)));
                        }
                        if self.source.is_type_assertion(root)? {
                            return Ok(NativeCompletion::clean(ty));
                        }
                        self.contextual_type_completion(root, ContextFlags::NONE)?
                            .and_then(|contextual| {
                                let contextual = match contextual {
                                    Some(contextual) => self
                                        .instantiate_contextual_type_completion(
                                            contextual,
                                            Some(root.into()),
                                            ContextFlags::NONE,
                                            &expression.location,
                                        )?
                                        .map(Some),
                                    None => NativeCompletion::clean(None),
                                };
                                contextual.and_then(|contextual| {
                                    self.widened_literal_like_for_contextual_type_completion(
                                        ty,
                                        contextual,
                                        &expression.location,
                                    )
                                })
                            })
                    })
            })
    }

    /// `getWidenedLiteralLikeTypeForContextualType` preserves matching literal
    /// categories before fresh/unique widening and regular-literal conversion.
    pub(super) fn widened_literal_like_for_contextual_type_completion(
        &mut self,
        mut ty: TypeId,
        contextual: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let literal = match contextual {
                Some(contextual) => sequence
                    .read(self.literal_of_contextual_type_completion(ty, contextual, consumer))?,
                None => false,
            };
            if !literal {
                ty = sequence.read(self.widened_literal_type_completion(ty, consumer))?;
                ty = sequence.read(self.widened_unique_symbol_type_completion(ty, consumer))?;
            }
            Ok(self.model.regular_literal(ty))
        })();
        sequence.finish(result)
    }

    /// `getWidenedUniqueESSymbolType` uses ordinary mapType for union origins.
    fn widened_unique_symbol_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if self.model.has_type_flags(ty, TypeFlags::UNIQUE_SYMBOL) {
                return Ok(self.model.primitive(PrimitiveName::Symbol));
            }
            if self.model.has_type_flags(ty, TypeFlags::UNION) {
                return sequence.read(self.map_type_completion(ty, consumer, |checker, child| {
                    checker.widened_unique_symbol_type_completion(child, consumer)
                }));
            }
            Ok(ty)
        })();
        sequence.finish(result)
    }

    /// `isConstContext` follows native short-circuit order and actual parent
    /// links. A const generic context is a semantic question, not a syntax flag.
    pub(super) fn is_const_context(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_const_context_completion(root, consumer)?.admit()
    }

    pub(super) fn is_const_context_completion(
        &mut self,
        mut root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            loop {
                let context = self.source.const_context(root)?;
                if context.assertion {
                    return Ok(true);
                }
                if sequence
                    .read(self.valid_const_argument_completion(context.argument, consumer))?
                    && let Some(contextual) =
                        sequence.read(self.contextual_type_completion(root, ContextFlags::NONE))?
                    && sequence
                        .read(self.is_const_type_variable_completion(contextual, 0, consumer))?
                {
                    return Ok(true);
                }
                match context.inherited {
                    Some(parent) => root = parent,
                    None => return Ok(false),
                }
            }
        })();
        sequence.finish(result)
    }

    /// `isValidConstAssertionArgument` resolves enum references only after its
    /// syntax alternatives; recovered enum eligibility remains a native value.
    fn valid_const_argument_completion(
        &mut self,
        argument: ConstArgument,
        consumer: &Location,
    ) -> NativeResult<bool> {
        match argument {
            ConstArgument::Yes => Ok(NativeCompletion::clean(true)),
            ConstArgument::No => Ok(NativeCompletion::clean(false)),
            ConstArgument::EnumReference(root) => self
                .resolve_value_entity_name_completion(root, consumer)?
                .and_then_value(|symbol| match symbol {
                    Some(symbol) => Ok(self.reference_symbol_flags(symbol, consumer)?.enum_),
                    None => Ok(false),
                }),
        }
    }

    /// `checkAssertion`: the operand is consumed before the annotation. Const
    /// assertions return regular identity; deferred comparability is diagnostic.
    pub(super) fn check_assertion_completion(
        &mut self,
        root: ExpressionRoot,
        mode: CheckMode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = self.source.assertion_recipe(root)?;
            let ty = sequence.read(self.check_expression_completion(&recipe.expression, mode))?;
            if let Some(annotation) = recipe.annotation {
                let site = recipe.annotation_site.ok_or_else(|| {
                    super::super::internal("assertion annotation lacks source identity")
                })?;
                sequence.read(self.check_type_source_element_completion(ExpressionRoot {
                    file: root.file,
                    site,
                }))?;
                return sequence.read(self.check_type_completion(&annotation));
            }
            let expression_root = recipe.expression.root.ok_or_else(|| {
                super::super::internal("const assertion operand lacks source identity")
            })?;
            let argument = self.source.const_context(expression_root)?.argument;
            sequence.read(self.valid_const_argument_completion(argument, consumer))?;
            Ok(self.model.regular_literal(ty))
        })();
        sequence.finish(result)
    }

    /// `checkSatisfiesExpression` checks the type syntax before its expression,
    /// then acquires the target. An actual error target returns immediately;
    /// otherwise even false assignability retains the expression's native type.
    pub(super) fn check_satisfies_expression_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = self.source.assertion_recipe(root)?;
            let site = recipe
                .annotation_site
                .ok_or_else(|| super::super::internal("satisfies target lacks source identity"))?;
            sequence.read(self.check_type_source_element_completion(ExpressionRoot {
                file: root.file,
                site,
            }))?;
            let ty = sequence
                .read(self.check_expression_completion(&recipe.expression, CheckMode::NORMAL))?;
            let annotation = recipe
                .annotation
                .ok_or_else(|| super::super::internal("satisfies has no target type"))?;
            let target = sequence.read(self.check_type_completion(&annotation))?;
            if self.model.is_error_type(target) {
                return Ok(target);
            }
            sequence.read(
                self.check_type_related_to_and_optionally_elaborate_completion(
                    ty,
                    target,
                    RelationKind::Assignable,
                    super::relations::RelationReporting::Report.satisfies_at(root),
                    recipe.expression.root,
                    consumer,
                ),
            )?;
            Ok(ty)
        })();
        sequence.finish(result)
    }
}
