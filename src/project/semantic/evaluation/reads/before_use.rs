//! Execute the semantic questions in native declaration-before-use order.
use super::super::super::model::{NativeResult, NativeSequence, PropertySymbolId};
use super::super::super::source::{DeclarationReference, ExpressionRoot};
use super::super::Evaluator;
use super::reference::{FlowReferenceQuestion, ReferenceNode};
use crate::references::Location;

impl Evaluator<'_, '_> {
    /// `isBlockScopedNameDeclaredBeforeUse` executes each static-initialization
    /// question before the next syntax alternative. False is ordinary data;
    /// consumed type and flow failures retain their original identity.
    pub(in crate::project::semantic::evaluation) fn block_scoped_name_declared_before_use_completion(
        &mut self,
        declaration: impl Into<DeclarationReference>,
        usage: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let plan = self.source.before_use_plan(declaration.into(), usage)?;
            for question in plan.questions {
                // `getTypeOfSymbol(getSymbolOfDeclaration(property))`: the
                // symbol precedes static-block filtering, even with no blocks.
                let symbol = sequence
                    .read(self.reference_symbol_of_declaration_completion(question.declaration))?;
                let prop_type =
                    sequence.read(self.value_reference_type_completion(symbol, consumer))?;
                for block in question.blocks {
                    let flow = self
                        .inputs
                        .bindings
                        .flow_graph(block.file)
                        .map_err(|error| self.read_binding_failure(consumer, error))?
                        .container_at(block.site)
                        .map_err(super::super::super::internal)?
                        .return_flow;
                    let reference: ReferenceNode =
                        self.new_this_property_reference(question.name, block, flow);
                    let initial =
                        sequence.read(self.optional_type_completion(prop_type, false, consumer))?;
                    let flow_type = sequence.read(self.flow_type_of_reference_completion(
                        FlowReferenceQuestion {
                            file: block.file,
                            reference,
                            declared: prop_type,
                            initial,
                            admitted: initial,
                            container: None,
                            flow: None,
                        },
                        consumer,
                    ))?;
                    if !self.model.contains_undefined(flow_type) {
                        return Ok(plan.if_initialized);
                    }
                }
            }
            Ok(plan.otherwise)
        })();
        sequence.finish(result)
    }

    /// `checkPropertyNotUsedBeforeDeclaration`: diagnostics do not change the
    /// access type, but their prerequisite type/flow questions are consumed.
    pub(in crate::project::semantic::evaluation) fn check_property_not_used_before_declaration_completion(
        &mut self,
        symbol: PropertySymbolId,
        access: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(declaration) = self.property_reference_value_declaration(symbol, consumer)?
            else {
                return Ok(());
            };
            let super::super::ReferenceDeclaration::Captured(declaration) = declaration else {
                // JSON declarations and supported access expressions belong to
                // different files. isBlockScopedNameDeclaredBeforeUse returns true
                // at that gate, before any static-initialization acquisition; JSON
                // also has no ClassDeclaration for the second diagnostic branch.
                return Ok(());
            };
            let context = self.source.property_before_use(declaration, access)?;
            let mut diagnosed = false;
            if let Some(usage) = context.property
                && !sequence.read(self.block_scoped_name_declared_before_use_completion(
                    declaration,
                    usage,
                    consumer,
                ))?
                && !context.static_method
            {
                diagnosed = self.inputs.semantic_options().use_define_for_class_fields
                    || !sequence.read(
                        self.property_declared_in_ancestor_class_completion(symbol, consumer),
                    )?;
            }
            if !diagnosed && let Some(usage) = context.class {
                sequence.read(self.block_scoped_name_declared_before_use_completion(
                    declaration,
                    usage,
                    consumer,
                ))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `isPropertyDeclaredInAncestorClass` asks only the first base, after the
    /// raw Parent Class gate; it does not inspect inherited declaration text.
    fn property_declared_in_ancestor_class_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            // This native question uses the raw Parent directly. getDeclaringClass
            // would additionally acquire getParentOfSymbol's late/merged parent.
            let property = super::super::ReferenceSymbol::from_property(symbol);
            let Some(parent) = self.reference_raw_parent_symbol(property, consumer)? else {
                return Ok(false);
            };
            if !self.reference_symbol_flags(parent, consumer)?.class {
                return Ok(false);
            }
            let class = sequence.read(self.declared_reference_type_completion(parent, consumer))?;
            let bases = sequence.read(self.base_types_completion(class, consumer))?;
            let Some(&base) = bases.first() else {
                return Ok(false);
            };
            let name = self.property_symbol_name_facts(symbol, consumer)?.key;
            let Some(property) =
                sequence.read(self.property_of_type_by_key_completion(base, &name, consumer))?
            else {
                return Ok(false);
            };
            let symbol = self.property_symbol(&property, consumer)?;
            Ok(self
                .property_reference_value_declaration(symbol, consumer)?
                .is_some())
        })();
        sequence.finish(result)
    }
}
