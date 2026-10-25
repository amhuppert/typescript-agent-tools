//! A request's located gaps: candidates whose semantic question failed with
//! an unsupported or invalid reason. Reference search (Q3) and construction
//! sites (Q5) share this admission, dependency attribution and order.
use super::model::DemandError;
use crate::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason, SemanticReason};
use crate::references::metadata::{location_order, option_location_order};
use crate::references::{Location, ReferenceCause, ReferenceGap};
use crate::resources::{Control, complete_or_abort};
use std::cmp::Ordering;

pub(super) struct Gaps {
    gaps: Vec<ReferenceGap>,
    control: Option<Control>,
}

impl Gaps {
    pub(super) fn new(control: Option<Control>) -> Self {
        Self {
            gaps: Vec::new(),
            control,
        }
    }

    /// Record `failure` as the gap of the candidate at `location`, or return
    /// it when it is not an unsupported/invalid semantic failure: resource,
    /// I/O and internal failures end the whole operation.
    pub(super) fn record(
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

    /// Gaps in exact candidate path/start/end, reason and cause order; one
    /// candidate/reason/cause triple occurs once.
    pub(super) fn finish(self) -> Vec<ReferenceGap> {
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
        gaps
    }
}

pub(super) fn sequence_order<T>(
    a: &[T],
    b: &[T],
    mut compare: impl FnMut(&T, &T) -> Ordering,
) -> Ordering {
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
