//! Native semantic requests over one captured generation. Binding identity,
//! source access, inference and finite presentation have separate owners.

mod constants;
mod evaluation;
mod inputs;
mod model;
mod presentation;
mod reference_import_tracker;
mod reference_imports;
mod reference_search;
mod selection;
mod source;

pub(in crate::project) use evaluation::initialization::{
    Observer as InitializationObserver, Step as InitializationStep,
};

/// Loading borrows the actual checker only through completed projection steps
/// and final lexical admission. All semantic arenas drop before publication.
pub(in crate::project) fn initialize_loading(
    program: &super::program::Program,
    raw: &super::binding::raw::RawBindingSnapshot,
    parser: &Parser,
    state: &mut super::binding::LinkState,
) -> Result<super::binding::NativeLinkOutput, AnalysisError> {
    let inputs = CheckerInputs::during_loading(program, raw);
    let mut evaluator = evaluation::Evaluator::new(inputs, parser);
    evaluator.initialize_with_observer(&mut super::binding::ProjectionObserver {
        program,
        raw,
        state,
    })?;
    state.finish_with_native_facts(program, raw, &evaluator.current_native_facts())
}

use super::Project;
use super::sources::FileId;
use crate::Span;
use crate::analysis::{
    AnalysisError, InputError, InputErrorKind, Parser, SemanticError, SemanticErrorKind,
    SemanticFailureReason, SemanticResourceReason,
};
use crate::references::Location;
use crate::references::{ReferenceAnswer, ReferenceRequest};
use crate::types::{
    SelectionFailure, TypeAnswer, TypeOperation, TypeOutcome, TypeRequest, TypeResult,
    TypeSelection,
};
use inputs::CheckerInputs;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(super) fn references(
    project: &Project,
    request: ReferenceRequest,
    superseded: Option<&Superseded>,
) -> Result<ReferenceAnswer, AnalysisError> {
    worker(project, superseded, |parser, control| {
        let mut evaluator = evaluation::Evaluator::new(
            CheckerInputs::from_project(project, Some(&control)),
            parser,
        );
        evaluator.ensure_initialized()?;
        reference_search::search(project, &request, &mut evaluator)
    })
}

/// One semantic worker owns its request-local checker and parser authority.
/// Ordinary panics become internal failures inside the worker; this does not
/// promise recovery from stack overflow or close the resource calibration gate.
pub(super) fn type_of(
    project: &Project,
    request: TypeRequest,
    superseded: Option<&Superseded>,
) -> Result<TypeAnswer, AnalysisError> {
    worker(project, superseded, |parser, control| {
        type_of_here(project, &request, parser, control)
    })
}

/// A flag another thread sets when a request's answer is no longer wanted
/// (`Project::execute_unless`).
pub(super) type Superseded = std::sync::Arc<std::sync::atomic::AtomicBool>;

/// Shared execution boundary for type inspection and member search. The
/// request creates one checker inside this worker, including candidate work.
fn worker<T: Send>(
    project: &Project,
    superseded: Option<&Superseded>,
    work: impl FnOnce(&Parser, crate::resources::Control) -> Result<T, AnalysisError> + Send,
) -> Result<T, AnalysisError> {
    worker_until(
        project,
        crate::resources::Limits::default(),
        superseded.cloned(),
        work,
    )
}

#[cfg(test)]
fn worker_with_limits<T: Send>(
    project: &Project,
    limits: crate::resources::Limits,
    work: impl FnOnce(&Parser, crate::resources::Control) -> Result<T, AnalysisError> + Send,
) -> Result<T, AnalysisError> {
    worker_until(project, limits, None, work)
}

fn worker_until<T: Send>(
    project: &Project,
    limits: crate::resources::Limits,
    superseded: Option<Superseded>,
    work: impl FnOnce(&Parser, crate::resources::Control) -> Result<T, AnalysisError> + Send,
) -> Result<T, AnalysisError> {
    crate::analysis::on_parsing_thread("tsparse-semantic", |parser| {
        let control = crate::resources::Control::new(limits, superseded);
        catch_unwind(AssertUnwindSafe(|| {
            control.check().map_err(resource_failure)?;
            let value = work(parser, control.clone())?;
            // Once an operation has failed, preserve that failure. This final
            // deadline observation guards only successful completion, after
            // the request-local evaluator and source captures were dropped.
            control.check().map_err(resource_failure)?;
            Ok(value)
        }))
        .unwrap_or_else(|payload| {
            Err(
                match crate::resources::request_interruption(payload.as_ref()) {
                    Some(stop) => resource_failure(stop),
                    None => internal("native semantic worker panicked"),
                },
            )
        })
    })
    .map_err(|error| InputError {
        kind: InputErrorKind::Io,
        message: format!("cannot start the semantic thread: {error}"),
        path: Some(project.config.clone()),
        position: None,
    })?
}

fn type_of_here(
    project: &Project,
    request: &TypeRequest,
    parser: &Parser,
    control: crate::resources::Control,
) -> Result<TypeAnswer, AnalysisError> {
    let mut evaluator =
        evaluation::Evaluator::new(CheckerInputs::from_project(project, Some(&control)), parser);
    evaluator.ensure_initialized()?;
    let selected = match selection::select(project, &mut evaluator, &request.query)? {
        selection::Selection::One(selected) => selected,
        selection::Selection::Absent => {
            return Ok(TypeAnswer::Unselected(TypeSelection {
                operation: TypeOperation::Type,
                project: project.config.clone(),
                selection: request.query.clone(),
                outcome: SelectionFailure::NotFound {
                    candidates: Vec::new(),
                },
            }));
        }
        selection::Selection::Ambiguous(candidates) => {
            return Ok(TypeAnswer::Unselected(TypeSelection {
                operation: TypeOperation::Type,
                project: project.config.clone(),
                selection: request.query.clone(),
                outcome: SelectionFailure::Ambiguous { candidates },
            }));
        }
    };
    let root = evaluator.establish(&selected)?;
    let presented = presentation::present(&mut evaluator, root, request, &selected.target.location)
        .map_err(|failure| {
            observe_recovery(
                evaluator.checker_inputs(),
                failure.into(),
                &selected.target.location,
            )
        })?;
    Ok(TypeAnswer::Selected(TypeResult {
        operation: TypeOperation::Type,
        outcome: if presented.issues.is_empty() {
            TypeOutcome::Found
        } else {
            TypeOutcome::Partial
        },
        project: project.config.clone(),
        target: selected.target,
        description: presented.description,
        issues: presented.issues,
    }))
}

/// Copy the captured exact filename under this view's existing request
/// capability. SourceText owns line/UTF-16 rules; loading has no request guard.
fn location(inputs: CheckerInputs<'_>, file: FileId, span: Span) -> Location {
    let source = inputs.sources.get(file);

    let mut file = String::new();
    file.push_str(&source.display);
    Location {
        file,
        start: source.text.position(span.start),
        end: source.text.position(span.end),
    }
}

fn failure(
    inputs: CheckerInputs<'_>,
    file: FileId,
    span: Span,
    reason: SemanticFailureReason,
    message: impl Into<String>,
) -> AnalysisError {
    failure_at_location(inputs, &location(inputs, file, span), reason, message)
}

fn failure_at_location(
    inputs: CheckerInputs<'_>,
    location: &Location,
    reason: SemanticFailureReason,
    message: impl Into<String>,
) -> AnalysisError {
    classified_at_location(
        inputs,
        location,
        SemanticErrorKind::UnsupportedSemantic,
        reason,
        message,
    )
}

fn unsupported_demand_at_location(
    inputs: CheckerInputs<'_>,
    location: &Location,
    reason: SemanticFailureReason,
    message: impl Into<String>,
) -> model::DemandError {
    let message = message.into();
    model::DemandError::new(
        failure_at_location(inputs, location, reason, message.clone()),
        Some(crate::types::TypeIssue {
            reason,
            location: location.clone(),
            message,
        }),
    )
}

/// One boundary for a consumed canonical binding attachment. Source and
/// evaluation share this policy; operation-specific native recovery choices
/// (such as a type-query unknownSymbol) remain with their checking owner.
fn binding_failure_at(
    inputs: CheckerInputs<'_>,
    consumer: &Location,
    error: super::binding::TypeBindingFailure,
) -> model::DemandError {
    binding_failure(inputs, Some(consumer), error)
}

/// A source-free native factory has no consumer to attribute. Preserve the
/// binding failure's kind and any actual dependency range; only a located
/// unsupported demand can become a describable Q4 leaf.
fn binding_failure(
    inputs: CheckerInputs<'_>,
    consumer: Option<&Location>,
    error: super::binding::TypeBindingFailure,
) -> model::DemandError {
    use super::binding::TypeBindingFailure;
    let Some(consumer) = consumer else {
        let (kind, reason, message) = match error {
            TypeBindingFailure::Dependency { file, span, reason } => {
                return invalid_at_location(
                    inputs,
                    &location(inputs, file, span),
                    SemanticFailureReason::UnresolvedSymbol,
                    reason,
                )
                .into();
            }
            TypeBindingFailure::NotRetained => {
                return internal("canonical semantic declaration association is not retained")
                    .into();
            }
            TypeBindingFailure::ErrorType(message) => (
                SemanticErrorKind::InvalidSemantic,
                SemanticFailureReason::ErrorType,
                message.to_owned(),
            ),
            TypeBindingFailure::Unresolved(message) => (
                SemanticErrorKind::InvalidSemantic,
                SemanticFailureReason::UnresolvedSymbol,
                message,
            ),
            TypeBindingFailure::Pending(message) => (
                SemanticErrorKind::UnsupportedSemantic,
                SemanticFailureReason::UnsupportedSemanticForm,
                message.to_owned(),
            ),
        };
        return AnalysisError::from(SemanticError {
            kind,
            reason: reason.into(),
            message,
            path: None,
            position: None,
            exact_location: None,
            unbound_recovery: None,
            causes: Vec::new(),
            omitted_causes: None,
            cause_kinds: Default::default(),
        })
        .into();
    };
    match error {
        TypeBindingFailure::ErrorType(message) => {
            invalid_at_location(inputs, consumer, SemanticFailureReason::ErrorType, message).into()
        }
        TypeBindingFailure::NotRetained => {
            internal("canonical semantic declaration association is not retained").into()
        }
        TypeBindingFailure::Unresolved(message) => invalid_at_location(
            inputs,
            consumer,
            SemanticFailureReason::UnresolvedSymbol,
            message,
        )
        .into(),
        TypeBindingFailure::Dependency { file, span, reason } => invalid_at_location(
            inputs,
            &location(inputs, file, span),
            SemanticFailureReason::UnresolvedSymbol,
            reason,
        )
        .into(),
        TypeBindingFailure::Pending(message) => unsupported_demand_at_location(
            inputs,
            consumer,
            SemanticFailureReason::UnsupportedSemanticForm,
            message,
        ),
    }
}

fn invalid_at_location(
    inputs: CheckerInputs<'_>,
    location: &Location,
    reason: SemanticFailureReason,
    message: impl Into<String>,
) -> AnalysisError {
    classified_at_location(
        inputs,
        location,
        SemanticErrorKind::InvalidSemantic,
        reason,
        message,
    )
}

/// Associate product provenance with the first matching captured input.
/// This is exact display-name equality, not filesystem resolution or case folding.
/// Inputs are scanned in id order and compared by their exact display bytes.
fn location_file(inputs: CheckerInputs<'_>, location: &Location) -> FileId {
    inputs
        .sources
        .ids()
        .find(|&file| inputs.sources.get(file).display.as_bytes() == location.file.as_bytes())
        .expect("semantic location belongs to one captured source")
}

fn classified_at_location(
    inputs: CheckerInputs<'_>,
    location: &Location,
    kind: SemanticErrorKind,
    reason: SemanticFailureReason,
    message: impl Into<String>,
) -> AnalysisError {
    let file = location_file(inputs, location);
    SemanticError {
        kind,
        reason: reason.into(),
        message: message.into(),
        path: Some(inputs.sources.get(file).path.clone()),
        position: Some(location.start),
        exact_location: Some(location.clone()),
        unbound_recovery: None,
        causes: Vec::new(),
        omitted_causes: None,
        cause_kinds: Default::default(),
    }
    .into()
}

/// Native recovery without a declaration has no source until observation.
/// Only its actual observed read may attach a consumer, and only on a copy.
fn unbound_recovery(kind: crate::analysis::UnboundSemanticRecovery) -> model::DemandError {
    let (reason, message) = match kind {
        crate::analysis::UnboundSemanticRecovery::UnknownSymbolDeclaredType => (
            SemanticFailureReason::ErrorType,
            "native getDeclaredTypeOfSymbol consumed unknownSymbol recovery",
        ),
        crate::analysis::UnboundSemanticRecovery::UnknownSignatureReturn => (
            SemanticFailureReason::ErrorType,
            "native unknown signature return consumes errorType recovery",
        ),
        crate::analysis::UnboundSemanticRecovery::NonValueSymbol => (
            SemanticFailureReason::ErrorType,
            "native getTypeOfSymbol consumed a non-value symbol",
        ),
        crate::analysis::UnboundSemanticRecovery::InstalledGlobalType => (
            SemanticFailureReason::ErrorType,
            "native installed global type recovery was consumed",
        ),
        crate::analysis::UnboundSemanticRecovery::RelationDepth => (
            SemanticFailureReason::RelationDepthLimit,
            "native relation stack depth limit was consumed",
        ),
        crate::analysis::UnboundSemanticRecovery::RelationComplexity => (
            SemanticFailureReason::RelationComplexityLimit,
            "native relation complexity allowance was consumed",
        ),
    };
    AnalysisError::from(SemanticError {
        kind: SemanticErrorKind::InvalidSemantic,
        reason: reason.into(),
        message: message.into(),
        path: None,
        position: None,
        exact_location: None,
        unbound_recovery: Some(kind),
        causes: Vec::new(),
        omitted_causes: None,
        cause_kinds: Default::default(),
    })
    .into()
}

fn observe_recovery(
    inputs: CheckerInputs<'_>,
    mut failure: model::DemandError,
    consumer: &Location,
) -> model::DemandError {
    // An already acquired whole failure terminates the operation. Observation
    // cannot replace its family with a later bookkeeping interruption.
    if failure.is_whole_operation_failure() {
        return failure;
    }
    if let AnalysisError::Semantic(error) = failure.error_mut() {
        error.observe_recovery(consumer, || {
            let file = location_file(inputs, consumer);
            inputs.sources.get(file).path.as_path()
        });
    }
    failure
}

fn internal(message: &str) -> AnalysisError {
    SemanticError {
        kind: SemanticErrorKind::Internal,
        reason: SemanticResourceReason::InternalFailure.into(),
        message: message.into(),
        path: None,
        position: None,
        exact_location: None,
        unbound_recovery: None,
        causes: Vec::new(),
        omitted_causes: None,
        cause_kinds: Default::default(),
    }
    .into()
}

fn resource_failure(stop: crate::resources::Stop) -> AnalysisError {
    use crate::resources::Stop;
    let (kind, reason, message) = match stop {
        // Nobody receives a superseded request's failure.
        Stop::Superseded => return internal("the request was superseded"),
        Stop::Depth => (
            SemanticErrorKind::SemanticDepthLimit,
            SemanticResourceReason::SemanticDepthLimit,
            "semantic request exceeded its checked recursion limit",
        ),
    };
    SemanticError {
        kind,
        reason: reason.into(),
        message: message.into(),
        path: None,
        position: None,
        exact_location: None,
        unbound_recovery: None,
        causes: Vec::new(),
        omitted_causes: None,
        cause_kinds: Default::default(),
    }
    .into()
}

#[cfg(test)]
mod resources_tests;

#[cfg(test)]
mod evidence_tests;
