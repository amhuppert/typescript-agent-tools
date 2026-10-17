//! Native instantiation expressions preserve non-signature members and reuse
//! the ordinary object-instantiation owner for later outer-parameter mapping.

use super::super::model::{
    MembersView, Metadata, NativeCompletion, NativeResult, NativeSequence, ObjectFlags, Signature,
    TypeFlags, TypeId, TypeView,
};
use super::super::source::{CallTypeArgument, ExpressionRoot, InstantiationExpressionRecipe};
use super::{Evaluator, relations::RelationReporting};
use crate::references::Location;

#[derive(Default)]
struct Applicability {
    any_applicable: bool,
    non_applicable: Option<TypeId>,
}
#[derive(Default)]
struct ConstituentSignatures {
    any: bool,
    applicable: bool,
}

impl Evaluator<'_, '_> {
    /// `checkExpressionWithTypeArguments`: source checks of every written
    /// argument precede ordinary operand checking in both syntax families.
    pub(super) fn check_expression_with_type_arguments_completion(
        &mut self,
        root: ExpressionRoot,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = self.source.instantiation_expression_recipe(root)?;
            if let Some(arguments) = &recipe.arguments {
                for argument in arguments {
                    sequence.read(self.check_type_source_element_completion(argument.root))?;
                }
            }
            let ty = sequence.read(self.check_expression_completion(
                &recipe.operand,
                super::contextual::CheckMode::NORMAL,
            ))?;
            sequence.read(self.instantiation_expression_type_completion(ty, &recipe))
        })();
        sequence.finish(result)
    }

    /// `getTypeFromTypeQueryNode` installs only after operand checking and
    /// widening finish. Its node slot is separate from the instantiation map.
    pub(super) fn type_from_type_query_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(value) = self.expressions.type_queries.get(&root) {
            return Ok(value.clone());
        }
        let value = self
            .check_expression_with_type_arguments_completion(root)?
            .and_then_value(|ty| self.widened_type(ty, consumer))?
            .map(|ty| self.model.regular_literal(ty));
        self.expressions.type_queries.insert(root, value.clone());
        Ok(value)
    }

    /// `getInstantiationExpressionType`: native identity is (actual node,
    /// operand type). Only the mapped factory completion is installed; the
    /// later diagnostic display belongs to this invocation's returned copy.
    fn instantiation_expression_type_completion(
        &mut self,
        ty: TypeId,
        recipe: &InstantiationExpressionRecipe,
    ) -> NativeResult<TypeId> {
        if self.model.is_silent_never(ty)
            || self.model.is_error_type(ty)
            || recipe.arguments.is_none()
        {
            return Ok(NativeCompletion::clean(ty));
        }
        let key = (recipe.declaration, ty);
        if let Some(result) = self.expressions.instantiations.get(&key) {
            return Ok(result.clone());
        }
        let mut applicability = Applicability::default();
        let result =
            self.instantiate_expression_constituent_completion(ty, recipe, &mut applicability)?;
        // Native installs before TypeToString can reenter. An interruption
        // before this point leaves the slot absent; a display cannot erase it.
        self.expressions.instantiations.insert(key, result.clone());
        result.and_then(|result| {
            let error_type = if applicability.any_applicable {
                applicability.non_applicable
            } else {
                Some(ty)
            };
            if let Some(error_type) = error_type {
                return Ok(self
                    .diagnostic_type_display_completion(
                        error_type,
                        None,
                        super::native_display::DiagnosticTypeFormat::DEFAULT,
                        &recipe.location,
                    )?
                    .map(|_| result));
            }
            Ok(NativeCompletion::clean(result))
        })
    }

    fn instantiate_expression_constituent_completion(
        &mut self,
        ty: TypeId,
        recipe: &InstantiationExpressionRecipe,
        applicability: &mut Applicability,
    ) -> NativeResult<TypeId> {
        let mut signatures = ConstituentSignatures::default();
        let result = self.instantiate_expression_type_part_completion(
            ty,
            recipe,
            &mut signatures,
            applicability,
        )?;
        applicability.any_applicable |= signatures.applicable;
        if signatures.any && !signatures.applicable && applicability.non_applicable.is_none() {
            applicability.non_applicable = Some(ty);
        }
        Ok(result)
    }

    /// Native intersections share signature-presence accumulators; each union
    /// constituent gets a fresh pair. This selects the exact later display demand.
    fn instantiate_expression_type_part_completion(
        &mut self,
        ty: TypeId,
        recipe: &InstantiationExpressionRecipe,
        signatures: &mut ConstituentSignatures,
        applicability: &mut Applicability,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.model.has_type_flags(ty, TypeFlags::OBJECT) {
                let members = sequence
                    .read(self.structured_type_members_completion(ty, &recipe.location))?
                    .ok_or_else(|| {
                        super::super::internal("object instantiation lacks structured members")
                    })?;
                let arguments = recipe.arguments.as_deref().expect("written argument list");
                let calls = sequence.read(self.instantiate_expression_signatures_completion(
                    &members.call_signatures,
                    arguments,
                    &recipe.location,
                ))?;
                let constructs =
                    sequence.read(self.instantiate_expression_signatures_completion(
                        &members.construct_signatures,
                        arguments,
                        &recipe.location,
                    ))?;
                signatures.any |=
                    !members.call_signatures.is_empty() || !members.construct_signatures.is_empty();
                signatures.applicable |= !calls.is_empty() || !constructs.is_empty();
                let same = |left: &[Signature], right: &[Signature]| {
                    left.len() == right.len()
                        && left
                            .iter()
                            .zip(right)
                            .all(|(left, right)| left.id == right.id)
                };
                if same(&calls, &members.call_signatures)
                    && same(&constructs, &members.construct_signatures)
                {
                    return Ok(ty);
                }
                let symbol = self.create_instantiation_expression_symbol();
                let result = self.model.anonymous_type_ex(
                    symbol,
                    Metadata::default(),
                    ObjectFlags::ANONYMOUS | ObjectFlags::INSTANTIATION_EXPRESSION_TYPE,
                    None,
                );
                let table = match self.resolved_member_table(ty) {
                    Some(table) => table,
                    None => Some(super::structure::MemberTable::Projected(
                        members.properties.clone(),
                    )),
                };
                let members = MembersView {
                    properties: members.properties,
                    call_signatures: calls.into(),
                    construct_signatures: constructs.into(),
                    index_signatures: members.index_signatures,
                };
                sequence.read(self.set_structured_member_table_completion(
                    result,
                    table,
                    NativeCompletion::clean(members),
                    &recipe.location,
                ))?;
                self.register_instantiation_expression(
                    result,
                    recipe.declaration,
                    &recipe.location,
                );
                return Ok(result);
            }
            if self
                .model
                .has_type_flags(ty, TypeFlags::INSTANTIABLE_NON_PRIMITIVE)
            {
                if let Some(constraint) =
                    sequence.read(self.base_constraint_completion(ty, &recipe.location))?
                {
                    let mapped =
                        sequence.read(self.instantiate_expression_type_part_completion(
                            constraint,
                            recipe,
                            signatures,
                            applicability,
                        ))?;
                    if mapped != constraint {
                        return Ok(mapped);
                    }
                }
            } else if self.model.has_type_flags(ty, TypeFlags::UNION) {
                return sequence.read(self.map_type_completion(
                    ty,
                    &recipe.location,
                    |checker, child| {
                        checker.instantiate_expression_constituent_completion(
                            child,
                            recipe,
                            applicability,
                        )
                    },
                ));
            } else if let TypeView::Intersection(types) = self.model.get(ty)? {
                let mut mapped = Vec::with_capacity(types.len());
                for &child in types.iter() {
                    mapped.push(sequence.read(
                        self.instantiate_expression_type_part_completion(
                            child,
                            recipe,
                            signatures,
                            applicability,
                        ),
                    )?);
                }
                return sequence.read(self.intersection_type_completion(mapped, &recipe.location));
            }
            Ok(ty)
        })();
        sequence.finish(result)
    }

    fn instantiate_expression_signatures_completion(
        &mut self,
        signatures: &[Signature],
        arguments: &[CallTypeArgument],
        consumer: &Location,
    ) -> NativeResult<Vec<Signature>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            // Native Filter completes before SameMap checks any argument type.
            let mut applicable = Vec::new();
            for signature in signatures {
                if !signature.type_parameter_types().is_empty()
                    && self.correct_type_argument_arity(signature, arguments.len())?
                {
                    applicable.push(signature.clone());
                }
            }
            for signature in &mut applicable {
                if let Some(arguments) = sequence.read(self.check_type_arguments_completion(
                    signature,
                    arguments,
                    RelationReporting::Report,
                    consumer,
                ))? {
                    *signature = sequence.read(
                        self.get_signature_instantiation_completion(signature, arguments, consumer),
                    )?;
                }
            }
            Ok(applicable)
        })();
        sequence.finish(result)
    }
}
