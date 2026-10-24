//! Request-local reference result storage. Native symbols establish group identity;
//! exact public Location identity establishes occurrence identity. No display key
//! replaces either. Checked maps own collisions; this owner owns retained payloads,
//! scalar buffers and the existing public output order.
use super::*;
use crate::references::metadata::{
    copy_location, copy_text, location_hash, location_order, option_location_order, text_order,
};
use crate::resources::{CheckedKey, CheckedMap, Control, complete_or_abort};
use std::cmp::Ordering;
use std::collections::HashMap;

pub(super) struct Ledger {
    identities: HashMap<NativeSymbolIdentity, usize>,
    groups: Vec<Group>,
    gaps: Vec<ReferenceGap>,
    control: Option<Control>,
}
struct Group {
    declarations: Vec<NamedLocation>,
    positions: CheckedMap<OccurrenceKey, usize>,
    occurrences: Vec<Occurrence>,
}
struct OccurrenceKey {
    location: Location,
    fingerprint: u64,
}
impl CheckedKey for OccurrenceKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    fn equivalent(&self, other: &Self) -> bool {
        location_order(&self.location, &other.location) == Ordering::Equal
    }
}
impl Ledger {
    pub(super) fn new(control: Option<Control>) -> Self {
        Self {
            identities: HashMap::new(),
            groups: Vec::new(),
            gaps: Vec::new(),
            control,
        }
    }
    /// Seed-only groups use precisely the same cache and backing as occurrence groups.
    pub(super) fn ensure_group(
        &mut self,
        identity: NativeSymbolIdentity,
        declarations: Vec<NamedLocation>,
    ) {
        if !self.identities.contains_key(&identity) {
            self.install_group(identity, declarations);
        }
    }
    fn install_group(
        &mut self,
        identity: NativeSymbolIdentity,
        declarations: Vec<NamedLocation>,
    ) -> usize {
        let index = self.groups.len();
        let group = Group {
            declarations,
            positions: CheckedMap::new(),
            occurrences: Vec::new(),
        };
        (self.groups).push(group);
        self.identities.insert(identity, index);
        index
    }
    pub(super) fn reference(
        &mut self,
        evaluator: &mut Evaluator<'_, '_>,
        symbol: ReferenceSymbol,
        site: &ReferenceSite,
    ) -> Result<(), DemandError> {
        let identity = evaluator.reference_symbol_identity(symbol, &site.selection)?;
        let index = if let Some(index) = self.identities.get(&identity) {
            *index
        } else {
            let declarations =
                evaluator.reference_symbol_named_declarations(symbol, &site.selection)?;
            self.install_group(identity, declarations)
        };

        let group = &mut self.groups[index];
        let occurrence_index = {
            let fingerprint = location_hash(&site.occurrence);
            let found = group
                .positions
                .get_by(fingerprint, |key| {
                    location_order(&key.location, &site.occurrence) == Ordering::Equal
                })
                .copied();
            if let Some(index) = found {
                index
            } else {
                let index = group.occurrences.len();
                let (key, occurrence) = {
                    (
                        OccurrenceKey {
                            location: copy_location(&site.occurrence),
                            fingerprint,
                        },
                        Occurrence {
                            name: copy_text(&site.key),
                            location: copy_location(&site.occurrence),
                            roles: Vec::new(),
                        },
                    )
                };
                (group.occurrences).push(occurrence);
                group.positions.insert(key, index);
                index
            }
        };
        let occurrence = &mut group.occurrences[occurrence_index];
        for role in references::role_list(site.roles) {
            (occurrence.roles).push(role);
        }
        // Four native role bits bound this list; duplicate occurrences add at most
        // another four before the immediately following sort/dedup.
        occurrence.roles.sort();
        occurrence.roles.dedup();
        Ok(())
    }
    pub(super) fn gap(
        &mut self,
        location: Location,
        failure: DemandError,
    ) -> Result<(), AnalysisError> {
        let error = AnalysisError::from(failure);
        let AnalysisError::Semantic(semantic) = &error else {
            return Err(error);
        };
        if !matches!(
            semantic.kind,
            SemanticErrorKind::UnsupportedSemantic | SemanticErrorKind::InvalidSemantic
        ) {
            return Err(error);
        }
        let SemanticReason::Failure(reason) = semantic.reason else {
            return Err(error);
        };
        let control = self.control.as_ref();

        let mut bounded = semantic.clone();
        // The gap belongs to the candidate; a consumed prerequisite can belong
        // to a different declaration (or have no native source site). Preserve
        // that primary dependency before its already ordered nested causes.
        // A same-site primary is already represented by the gap itself.
        if option_location_order(semantic.exact_location.as_ref(), Some(&location))
            != Ordering::Equal
        {
            bounded.causes.clear();
            if let Some(control) = control {
                complete_or_abort(control.check());
            }
            bounded.omitted_causes = None;
            bounded.append_dependency(semantic)?;
        } else {
            bounded.bound_causes()?;
        }
        let mut causes = Vec::new();
        for cause in bounded.causes {
            let SemanticReason::Failure(reason) = cause.reason else {
                return Err(error);
            };
            (causes).push(ReferenceCause {
                reason,
                location: cause.location,
                message: cause.message,
            });
        }
        (self.gaps).push(ReferenceGap {
            reason,
            location,
            message: bounded.message,
            causes,
            omitted_causes: bounded.omitted_causes,
        });
        Ok(())
    }

    pub(super) fn finish(self) -> (Vec<ReferenceGroup>, Vec<ReferenceGap>) {
        let mut groups = Vec::new();
        for mut group in self.groups {
            (group.occurrences).sort_by(|a, b| location_order(&a.location, &b.location));
            (groups).push(ReferenceGroup {
                declarations: group.declarations,
                occurrences: group.occurrences,
            });
        }
        (groups).sort_by(|a, b| {
            sequence_order(&a.declarations, &b.declarations, |a, b| {
                location_order(&a.location, &b.location).then_with(|| text_order(&a.name, &b.name))
            })
            .then_with(|| {
                sequence_order(&a.occurrences, &b.occurrences, |a, b| {
                    location_order(&a.location, &b.location)
                })
            })
        });
        let mut gaps = self.gaps;
        (gaps).sort_by(|a, b| {
            location_order(&a.location, &b.location)
                .then_with(|| reason_name(a.reason).cmp(reason_name(b.reason)))
                .then_with(|| {
                    sequence_order(&a.causes, &b.causes, |a, b| {
                        option_location_order(a.location.as_ref(), b.location.as_ref())
                    })
                })
        });
        gaps.dedup_by(|a, b| {
            location_order(&a.location, &b.location) == Ordering::Equal
                && a.reason == b.reason
                && sequence_order(&a.causes, &b.causes, |a, b| {
                    option_location_order(a.location.as_ref(), b.location.as_ref())
                }) == Ordering::Equal
        });
        (groups, gaps)
    }
}

fn sequence_order<T>(a: &[T], b: &[T], mut compare: impl FnMut(&T, &T) -> Ordering) -> Ordering {
    for (a, b) in a.iter().zip(b) {
        let order = compare(a, b);
        if order != Ordering::Equal {
            return order;
        }
    }
    a.len().cmp(&b.len())
}

fn reason_name(reason: SemanticFailureReason) -> &'static str {
    use SemanticFailureReason::*;
    match reason {
        FlowRequired => "flow_required",
        JsxTypingRequired => "jsx_typing_required",
        GeneratorInferenceRequired => "generator_inference_required",
        DecoratorTypingRequired => "decorator_typing_required",
        JavascriptSemanticsRequired => "javascript_semantics_required",
        ExpandoSemanticsRequired => "expando_semantics_required",
        UnsupportedSemanticForm => "unsupported_semantic_form",
        UnrepresentableIdentity => "unrepresentable_identity",
        UnresolvedSymbol => "unresolved_symbol",
        ErrorType => "error_type",
        OverloadRecovery => "overload_recovery",
        CircularType => "circular_type",
        InstantiationLimit => "instantiation_limit",
        ConditionalTailLimit => "conditional_tail_limit",
        UnionComplexityLimit => "union_complexity_limit",
        TupleComplexityLimit => "tuple_complexity_limit",
        SubtypeComplexityLimit => "subtype_complexity_limit",
        RelationDepthLimit => "relation_depth_limit",
        RelationComplexityLimit => "relation_complexity_limit",
    }
}
