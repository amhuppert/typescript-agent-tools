//! Missing-property diagnostics perform native semantic acquisitions even
//! though the query product does not publish a project diagnostic list.
#[path = "missing_features.rs"]
mod features;

use super::super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use crate::project::binding::NativePropertyKey;
use crate::project::semantic::model::{
    NativeCompletion, NativeResult, NativeSequence, TypeFlags, TypeId, TypeView,
};
use crate::project::semantic::source::DeclarationIdentity;
use crate::references::Location;
use std::collections::HashSet;

#[derive(Default)]
pub(in crate::project::semantic::evaluation) struct MissingPropertiesState {
    reported: HashSet<(DeclarationIdentity, TypeId, bool)>,
}

impl Evaluator<'_, '_> {
    /// `checkAndReportErrorForExtendingInterface` suppresses the ordinary
    /// missing-property report only after Interface-only entity resolution.
    pub(in crate::project::semantic::evaluation) fn check_and_report_error_for_extending_interface_completion(
        &mut self,
        root: crate::project::semantic::source::ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let Some(entity) = self.source.entity_name_for_extending_interface(root)? else {
            return Ok(NativeCompletion::clean(false));
        };
        let anchor = self.source.reference_entity_name_anchor(entity)?;
        self.reference_entity_at_completion(
            entity.file,
            anchor,
            false,
            crate::project::binding::NativeLookupMeaning::INTERFACE,
            false,
            consumer,
        )
        .map(|completion| completion.map(|(symbol, _)| symbol.is_some()))
    }

    /// `reportNonexistentProperty`: the report-key and TypeChecked flag are
    /// installed before any diagnostic dependency. Reentry returns without
    /// replaying earlier acquisitions; interruptions preserve installed flags.
    pub(in crate::project::semantic::evaluation) fn report_nonexistent_property_completion(
        &mut self,
        prop_node: DeclarationIdentity,
        containing_type: TypeId,
        is_unchecked_js: bool,
        consumer: &Location,
    ) -> NativeResult<()> {
        if !self
            .missing_properties
            .reported
            .insert((prop_node, containing_type, is_unchecked_js))
            || !self.node_types.mark_type_checked(prop_node)
        {
            return Ok(NativeCompletion::clean(()));
        }
        let syntax = self.source.missing_property_syntax(prop_node)?;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !syntax.private
                && self.model.has_type_flags(containing_type, TypeFlags::UNION)
                && !self
                    .model
                    .has_type_flags(containing_type, TypeFlags::PRIMITIVE)
            {
                let TypeView::Union(types) = self.model.get(containing_type)? else {
                    unreachable!()
                };
                for &subtype in types.iter() {
                    if sequence
                        .read(self.property_of_type_completion(subtype, &syntax.name, consumer))?
                        .is_none()
                        && sequence
                            .read(self.applicable_index_info_for_key_completion(
                                subtype,
                                &NativePropertyKey::fixed(syntax.name.as_bytes()),
                                consumer,
                            ))?
                            .is_none()
                    {
                        sequence.read(
                            self.check_diagnostic_type_display_completion(subtype, consumer),
                        )?;
                        break;
                    }
                }
            }
            if sequence.read(self.type_has_static_property_completion(
                &syntax.name,
                containing_type,
                consumer,
            ))? {
                sequence.read(
                    self.check_diagnostic_type_display_completion(containing_type, consumer),
                )?;
                return Ok(());
            }
            let promised = sequence
                .read(self.promised_type_of_promise_completion(containing_type, consumer))?;
            if let Some(promised) = promised
                && sequence
                    .read(self.property_of_type_completion(promised, &syntax.name, consumer))?
                    .is_some()
            {
                sequence.read(
                    self.check_diagnostic_type_display_completion(containing_type, consumer),
                )?;
                return Ok(());
            }
            sequence
                .read(self.check_diagnostic_type_display_completion(containing_type, consumer))?;
            if sequence
                .read(self.suggested_property_library_completion(
                    &syntax.display_name,
                    containing_type,
                    consumer,
                ))?
                .is_some()
            {
                return Ok(());
            }
            let properties =
                sequence.read(self.properties_of_type_completion(containing_type, consumer))?;
            let mut symbols = Vec::with_capacity(properties.len());
            for property in properties {
                let symbol = self.property_symbol(&property, consumer)?;
                if let Some((access, is_super)) = syntax.access
                    && !sequence.read(self.is_property_accessible_completion(
                        access,
                        is_super,
                        false,
                        containing_type,
                        symbol,
                        consumer,
                    ))?
                {
                    continue;
                }
                symbols.push(symbol);
            }
            if let Some(suggestion) = sequence.read(
                self.spelling_suggestion_for_properties_completion(&syntax.name, symbols, consumer),
            )? {
                // Native SymbolName is read again after the spelling helper.
                self.reference_symbol_display_name_bytes(
                    ReferenceSymbol::from_property(suggestion),
                    consumer,
                )?;
                self.reference_symbol_value_declaration(
                    ReferenceSymbol::from_property(suggestion),
                    consumer,
                )?;
                return Ok(());
            }
            sequence
                .read(self.elaborate_never_intersection_completion(containing_type, consumer))?;
            sequence.read(
                self.container_seems_empty_dom_element_completion(containing_type, consumer),
            )?;
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `typeHasStaticProperty` asks the containing type's actual Symbol's value
    /// type, then its exact named property and selected ValueDeclaration.
    fn type_has_static_property_completion(
        &mut self,
        name: &str,
        containing: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let Some(symbol) = self.reference_symbol_of_type(containing, consumer)? else {
            return Ok(NativeCompletion::clean(false));
        };
        self.value_reference_type_completion(symbol, consumer)?
            .and_then(|ty| {
                self.property_of_type_completion(ty, name, consumer)?
                    .and_then_value(|property| {
                        let Some(property) = property else {
                            return Ok(false);
                        };
                        let symbol = self.property_symbol(&property, consumer)?;
                        match self.reference_symbol_value_declaration(
                            ReferenceSymbol::from_property(symbol),
                            consumer,
                        )? {
                            Some(ReferenceDeclaration::Captured(declaration)) => {
                                self.source.declaration_is_static(declaration)
                            }
                            Some(ReferenceDeclaration::Json { .. }) | None => Ok(false),
                        }
                    })
            })
    }

    /// `getSuggestedLibForNonExistentProperty` observes apparent type before
    /// using the shared exact feature map; declared annotations are not names.
    fn suggested_property_library_completion(
        &mut self,
        name: &str,
        containing: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<&'static str>> {
        self.apparent_type_completion(containing, consumer)?
            .and_then_value(|apparent| {
                let Some(symbol) = self.reference_symbol_of_type(apparent, consumer)? else {
                    return Ok(None);
                };
                let name_of_type = self.reference_symbol_name(symbol, consumer)?;
                Ok(features::library_for_property(&name_of_type, name))
            })
    }

    /// `containerSeemsToBeEmptyDomElement` / `hasCommonDomTypeName` inspect
    /// explicitly configured lib before the shallow everyContainedType walk.
    /// The final empty-object question alone resolves member inventories.
    fn container_seems_empty_dom_element_completion(
        &mut self,
        containing: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if self.inputs.semantic_options().explicit_dom_library() {
            return Ok(NativeCompletion::clean(false));
        }
        let types = match self.model.get(containing)? {
            TypeView::Union(types) | TypeView::Intersection(types) => types.to_vec(),
            _ => vec![containing],
        };
        for ty in types {
            let Some(symbol) = self.reference_symbol_of_type(ty, consumer)? else {
                return Ok(NativeCompletion::clean(false));
            };
            let name = self.reference_symbol_name(symbol, consumer)?;
            if !(matches!(name.as_slice(), b"EventTarget" | b"Node" | b"Element")
                || name.starts_with(b"HTML") && name.ends_with(b"Element"))
            {
                return Ok(NativeCompletion::clean(false));
            }
        }
        self.is_empty_object_type_completion(containing, consumer)
    }
}
