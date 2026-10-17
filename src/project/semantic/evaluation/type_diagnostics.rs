//! Type-diagnostic elaboration shared by relation and property-access reports.
use super::super::model::{DemandError, NativeResult, NativeSequence, TypeId};
use super::{Evaluator, native_display::DiagnosticTypeFormat};
use crate::references::Location;

pub(super) enum NeverConflict {
    Discriminant,
    Private,
}
pub(super) struct NeverIntersectionElaboration {
    pub(super) conflict: NeverConflict,
    pub(super) type_name: Vec<u8>,
    pub(super) property_name: Vec<u8>,
}

impl Evaluator<'_, '_> {
    /// `elaborateNeverIntersection`: read the installed reduction flag, search
    /// all discriminants before any conflicting private property, then perform
    /// no-reduction type display before symbol display. Reporters own only the
    /// destination diagnostic chain; they never reconstruct this acquisition.
    pub(super) fn elaborate_never_intersection_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<NeverIntersectionElaboration>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<_, DemandError> {
            if !self.is_never_intersection(ty)? {
                return Ok(None);
            }
            let mut selected = None;
            for property in
                sequence.read(self.properties_of_composite_type_completion(ty, consumer))?
            {
                let symbol = self.property_symbol(&property, consumer)?;
                if sequence
                    .read(self.is_discriminant_with_never_type_completion(symbol, consumer))?
                {
                    selected = Some((symbol, NeverConflict::Discriminant));
                    break;
                }
            }
            if selected.is_none() {
                for property in
                    sequence.read(self.properties_of_composite_type_completion(ty, consumer))?
                {
                    let symbol = self.property_symbol(&property, consumer)?;
                    if self.is_conflicting_private_property(symbol, consumer)? {
                        selected = Some((symbol, NeverConflict::Private));
                        break;
                    }
                }
            }
            let Some((symbol, conflict)) = selected else {
                return Ok(None);
            };
            let type_name = sequence
                .read(self.raw_diagnostic_type_display_completion(
                    ty,
                    None,
                    DiagnosticTypeFormat::NO_TYPE_REDUCTION,
                    consumer,
                ))?
                .into_bytes();
            let property_name = sequence
                .read(self.raw_diagnostic_property_symbol_display_completion(symbol, consumer))?
                .into_bytes();
            Ok(Some(NeverIntersectionElaboration {
                conflict,
                type_name,
                property_name,
            }))
        })();
        sequence.finish(result)
    }
}
