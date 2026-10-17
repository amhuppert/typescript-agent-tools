//! `createMappedTypeNodeFromType` keeps native semantic acquisition in the
//! node builder. Its output syntax has no checker callbacks.
use super::*;

impl Evaluator<'_, '_> {
    pub(super) fn diagnostic_mapped_type(
        &mut self,
        ty: TypeId,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        let recipe = self.mapped_recipe(ty)?;
        let template = self.mapped_template(ty, consumer)?;
        let parameter = self.mapped_parameter(ty, consumer)?;
        let wrapper = self.mapped_display_needs_wrapper(ty, context.flags.has(1 << 2), consumer)?;
        if wrapper || context.flags.has(1 << 2) {
            return Err(self.diagnostic_dependency(
                consumer,
                "native generated mapped wrapper requires diagnostic type-parameter naming scope",
            ));
        }
        let constraint = if recipe.keyof_operand.is_some() {
            let modifiers = self.mapped_modifiers(ty, consumer)?;
            self.diagnostic_type_node(modifiers, context, consumer)?
                .map(|node| Node::Prefix("keyof ", Box::new(node)))
        } else {
            let constraint = self.mapped_constraint(ty, consumer)?;
            self.diagnostic_type_node(constraint, context, consumer)?
        };
        constraint.and_then(|constraint| {
            let result = self.diagnostic_scope(
                context,
                Some(recipe.declaration),
                &[],
                &[parameter],
                None,
                None,
                consumer,
                |checker, context| {
                    checker
                        .diagnostic_parameter_declaration(
                            parameter,
                            Some(Box::new(constraint)),
                            context,
                            consumer,
                        )?
                        .and_then(|parameter| {
                            let name = if recipe.name.is_some() {
                                let name = checker.mapped_name(ty, consumer)?.ok_or_else(|| {
                                    super::super::super::internal(
                                        "syntactic mapped name has no type",
                                    )
                                })?;
                                checker
                                    .diagnostic_type_node(name, context, consumer)?
                                    .map(|node| Some(Box::new(node)))
                            } else {
                                NativeCompletion::clean(None)
                            };
                            name.and_then(|name| {
                                let template = checker
                                    .model
                                    .remove_missing_type(template, recipe.optional > 0)
                                    .map_err(|error| error.at(checker.inputs, consumer))?;
                                Ok(checker
                                    .diagnostic_type_node(template, context, consumer)?
                                    .map(|template| Node::Mapped {
                                        readonly: super::super::mapped::mapped_readonly_text(
                                            recipe.readonly_token,
                                        ),
                                        parameter,
                                        name,
                                        optional: super::super::mapped::mapped_optional_text(
                                            recipe.question_token,
                                        ),
                                        template: Some(Box::new(template)),
                                    }))
                            })
                        })
                },
            )?;
            Ok(result.map(|node| {
                context.length += 10;
                node
            }))
        })
    }
}
