//! Native Relater diagnostic state. Chain rollback changes diagnostic state
//! only: semantic reads, relation cache entries and failures remain acquired.

use crate::project::semantic::model::{NativeCompletion, NativeResult, NativeSequence};

use super::super::Evaluator;
use super::super::native_display::DiagnosticTypeFormat;
use super::{
    IntersectionState, RecursionFlags, RelationKind, RelationOverflow, RelationSessionId, Ternary,
};
use crate::project::semantic::model::{
    DemandError, ObjectFlags, Predicate, Property, PropertySymbolId, Signature, TypeFlags, TypeId,
    TypeView,
};
use crate::project::semantic::source::ExpressionNode;
use crate::references::Location;
use crate::types::{PrimitiveName, TypePredicateKind, TypePredicateSubject};
use std::sync::Arc;

/// A comparison's reporting policy travels with its live session handle.
/// Native nested comparisons can disable reporting without changing the
/// session's error node or the error chain they may later inspect or restore.
#[derive(Clone, Copy)]
pub(super) struct RelationInvocation {
    pub(super) session: RelationSessionId,
    pub(super) report: bool,
    pub(super) head: Option<Message>,
}

impl RelationInvocation {
    pub(super) fn silent(session: RelationSessionId) -> Self {
        Self {
            session,
            report: false,
            head: None,
        }
    }

    pub(super) fn reporting(self, report: bool) -> Self {
        Self {
            report,
            head: None,
            ..self
        }
    }

    pub(super) fn without_head(self) -> Self {
        Self { head: None, ..self }
    }
}

/// These variants denote pinned diagnostic constants, independently of their
/// English text and numeric code. Native suppression compares their identity.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Message {
    EnumMissingProperty,
    EnumDifferentValues,
    EnumUnknownNumericValue,
    ComputedEnumMember,
    UsingInitializer,
    AwaitUsingInitializer,
    Constraint,
    IterationForOf,
    IterationSpread,
    IterationDestructuring,
    Assignable,
    Satisfies,
    ComputedProperty,
    Comparable,
    DifferentTypesWithSameName,
    ExactOptionalAssignable,
    Argument,
    ExactOptionalArgument,
    SuggestedLiteral,
    AssignableToConstraint,
    ArbitraryInstantiation,
    ExcessProperty,
    SuggestedExcessProperty,
    Complexity,
    Depth,
    Readonly,
    MissingProperty,
    MissingProperties,
    MissingPropertiesAndMore,
    PropertyTypes,
    NestedPropertyTypes,
    NestedReturnTypes,
    CallReturnNoArguments,
    ConstructReturnNoArguments,
    CallReturn,
    ConstructReturn,
    ClassImplementsInterface,
    ClassImplementsClass,
    ClassExtends,
    ClassStaticExtends,
    NoCommonProperties,
    DidYouMeanToCall,
    AbstractConstructor,
    ConstructorVisibility,
    TooFewArguments,
    ThisTypes,
    ThisContext,
    ParameterTypes,
    NoMatchingSignature,
    MustBePredicate,
    PredicateKinds,
    PredicateParameterPosition,
    PredicateTypes,
    PrimitiveWrapper,
    ObjectInsteadOfAny,
    NeverDiscriminant,
    NeverPrivate,
    SeparatePrivateDeclarations,
    PrivateProperty,
    ProtectedNotDerived,
    ProtectedProperty,
    OptionalProperty,
    PropertyDoesNotExist,
    SuggestedProperty,
    MissingIndex,
    PropertyIndex,
    IndexTypes,
    DifferentIndexTypes,
    TupleTooShort,
    TupleTooLong,
    TupleMayBeShorter,
    TupleMayBeLonger,
    TupleMissingVariadic,
    TupleUnexpectedVariadic,
    TupleMissingRequired,
    TupleElementTypes,
    TupleRangeTypes,
}

impl Message {
    /// `isConversionOrInterfaceImplementationMessage`. Its conversion and
    /// JSX head messages are never reported by this checker, so only the
    /// implementation messages can reach it. The match is exhaustive so that
    /// adding a message decides this question.
    pub(super) fn conversion_or_implementation(self) -> bool {
        match self {
            Self::ClassImplementsInterface | Self::ClassImplementsClass => true,
            Self::EnumMissingProperty
            | Self::EnumDifferentValues
            | Self::EnumUnknownNumericValue
            | Self::ComputedEnumMember
            | Self::UsingInitializer
            | Self::AwaitUsingInitializer
            | Self::Constraint
            | Self::IterationForOf
            | Self::IterationSpread
            | Self::IterationDestructuring
            | Self::Assignable
            | Self::Satisfies
            | Self::ComputedProperty
            | Self::Comparable
            | Self::DifferentTypesWithSameName
            | Self::ExactOptionalAssignable
            | Self::Argument
            | Self::ExactOptionalArgument
            | Self::SuggestedLiteral
            | Self::AssignableToConstraint
            | Self::ArbitraryInstantiation
            | Self::ExcessProperty
            | Self::SuggestedExcessProperty
            | Self::Complexity
            | Self::Depth
            | Self::Readonly
            | Self::MissingProperty
            | Self::MissingProperties
            | Self::MissingPropertiesAndMore
            | Self::PropertyTypes
            | Self::NestedPropertyTypes
            | Self::NestedReturnTypes
            | Self::CallReturnNoArguments
            | Self::ConstructReturnNoArguments
            | Self::CallReturn
            | Self::ConstructReturn
            | Self::ClassExtends
            | Self::ClassStaticExtends
            | Self::NoCommonProperties
            | Self::DidYouMeanToCall
            | Self::AbstractConstructor
            | Self::ConstructorVisibility
            | Self::TooFewArguments
            | Self::ThisTypes
            | Self::ThisContext
            | Self::ParameterTypes
            | Self::NoMatchingSignature
            | Self::MustBePredicate
            | Self::PredicateKinds
            | Self::PredicateParameterPosition
            | Self::PredicateTypes
            | Self::PrimitiveWrapper
            | Self::ObjectInsteadOfAny
            | Self::NeverDiscriminant
            | Self::NeverPrivate
            | Self::SeparatePrivateDeclarations
            | Self::PrivateProperty
            | Self::ProtectedNotDerived
            | Self::ProtectedProperty
            | Self::OptionalProperty
            | Self::PropertyDoesNotExist
            | Self::SuggestedProperty
            | Self::MissingIndex
            | Self::PropertyIndex
            | Self::IndexTypes
            | Self::DifferentIndexTypes
            | Self::TupleTooShort
            | Self::TupleTooLong
            | Self::TupleMayBeShorter
            | Self::TupleMayBeLonger
            | Self::TupleMissingVariadic
            | Self::TupleUnexpectedVariadic
            | Self::TupleMissingRequired
            | Self::TupleElementTypes
            | Self::TupleRangeTypes => false,
        }
    }

    pub(super) fn excess_property(self) -> bool {
        matches!(self, Self::ExcessProperty | Self::SuggestedExcessProperty)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum Argument {
    Text(Vec<u8>),
    Number(isize),
}

impl Argument {
    fn text(&self) -> &[u8] {
        match self {
            Self::Text(text) => text,
            Self::Number(_) => unreachable!("native property-name diagnostic argument is text"),
        }
    }
}

#[derive(Clone)]
pub(super) struct ErrorChain {
    next: Option<Arc<Self>>,
    message: Message,
    arguments: Vec<Argument>,
}

/// `errorState` / `getErrorState` / `restoreErrorState`: Arc cloning preserves
/// the native chain-head identity used by structural elaboration. Related
/// diagnostic notes have no effect on those decisions and are not public
/// output; their semantic acquisitions remain at the native reporting sites.
#[derive(Clone, Default)]
pub(super) struct ErrorState {
    pub(super) chain: Option<Arc<ErrorChain>>,
}

impl ErrorState {
    /// `createDiagnosticChainFromErrorChain` drops compatibility-pyramid-only
    /// messages. A nonnil chain is not itself proof that a diagnostic exists.
    pub(super) fn produces_diagnostic(&self) -> bool {
        let mut entry = self.chain.as_deref();
        while let Some(node) = entry {
            if !matches!(
                node.message,
                Message::CallReturn
                    | Message::ConstructReturn
                    | Message::CallReturnNoArguments
                    | Message::ConstructReturnNoArguments
            ) {
                return true;
            }
            entry = node.next.as_deref();
        }
        false
    }

    /// `getChainMessage`.
    pub(super) fn message(&self, index: usize) -> Option<Message> {
        self.entry(index).map(|entry| entry.message)
    }

    fn entry(&self, index: usize) -> Option<&ErrorChain> {
        let mut entry = self.chain.as_deref();
        for _ in 0..index {
            entry = entry?.next.as_deref();
        }
        entry
    }

    /// `chainDepth` is independent of both equality and head identity.
    pub(super) fn depth(&self) -> usize {
        let mut depth = 0;
        let mut entry = self.chain.as_deref();
        while let Some(node) = entry {
            depth += 1;
            entry = node.next.as_deref();
        }
        depth
    }

    pub(super) fn same_head(&self, other: &Self) -> bool {
        match (&self.chain, &other.chain) {
            (None, None) => true,
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    /// `chainArgsMatch`: None is a wildcard in the question, never a stored
    /// diagnostic argument. Native callers supply no more than head arity.
    pub(super) fn arguments_match(&self, expected: &[Option<Argument>]) -> bool {
        let entry = self
            .chain
            .as_ref()
            .expect("argument matching requires a diagnostic head");
        expected.iter().enumerate().all(|(index, expected)| {
            expected
                .as_ref()
                .is_none_or(|expected| expected == &entry.arguments[index])
        })
    }

    /// `reportError` owns all native property/return chain transformations.
    /// The recursive rewrite matters: a newly joined name can join again.
    pub(super) fn report(&mut self, mut message: Message, mut arguments: Vec<Argument>) {
        if message == Message::PropertyTypes {
            if self.message(0).is_some_and(Message::excess_property) {
                return;
            }
            let return_name = match self.message(1) {
                Some(Message::CallReturnNoArguments) => Some((false, &b"()"[..])),
                Some(Message::ConstructReturnNoArguments) => Some((true, &b"()"[..])),
                Some(Message::CallReturn) => Some((false, &b"(...)"[..])),
                Some(Message::ConstructReturn) => Some((true, &b"(...)"[..])),
                _ => None,
            };
            if let Some((construct, suffix)) = return_name {
                let name = property_name_argument(arguments[0].text());
                arguments[0] = Argument::Text(
                    [if construct { &b"new "[..] } else { &[] }, &name, suffix].concat(),
                );
                message = Message::NestedReturnTypes;
                self.chain = self.entry(1).expect("observed chain entry").next.clone();
            }
            if matches!(
                self.message(1),
                Some(
                    Message::PropertyTypes
                        | Message::NestedPropertyTypes
                        | Message::NestedReturnTypes
                )
            ) {
                let head = property_name_argument(arguments[0].text());
                let tail = property_name_argument(
                    self.entry(1).expect("observed chain entry").arguments[0].text(),
                );
                let joined = add_to_dotted_name(&head, &tail);
                self.chain = self.entry(1).expect("observed chain entry").next.clone();
                if message == Message::PropertyTypes {
                    message = Message::NestedPropertyTypes;
                }
                self.report(message, vec![Argument::Text(joined)]);
                return;
            }
        }
        self.chain = Some(Arc::new(ErrorChain {
            next: self.chain.clone(),
            message,
            arguments,
        }));
    }
}

/// `getPropertyNameArg` treats only the native first-byte quote test specially.
fn property_name_argument(name: &[u8]) -> Vec<u8> {
    if matches!(name.first(), Some(b'"' | b'\'' | b'`')) {
        [b"[", name, b"]"].concat()
    } else {
        name.to_vec()
    }
}

/// `addToDottedName`: scan only the actual leading call/new spelling; arbitrary
/// property text remains unchanged and is never decoded or normalized.
fn add_to_dotted_name(head: &[u8], tail: &[u8]) -> Vec<u8> {
    let head = if head.starts_with(b"new ") {
        [b"(", head, b")"].concat()
    } else {
        head.to_vec()
    };
    let mut position = 0;
    loop {
        if tail[position..].starts_with(b"(") {
            position += 1;
        } else if tail[position..].starts_with(b"new ") {
            position += 4;
        } else {
            break;
        }
    }
    let (prefix, suffix) = tail.split_at(position);
    [
        prefix,
        &head,
        if suffix.starts_with(b"[") {
            &[]
        } else {
            &b"."[..]
        },
        suffix,
    ]
    .concat()
}

/// The actual native error node is separate from the location used for a
/// consumed product failure, and is never part of rollback snapshots.
#[derive(Clone, Copy)]
pub(in crate::project::semantic::evaluation) struct ErrorContext {
    pub(super) node: ExpressionNode,
    pub(super) head: Option<Message>,
}

impl Evaluator<'_, '_> {
    /// Native overflow display is later work than the recursive False result.
    /// It contributes only its ordered argument acquisitions; the producer's
    /// limit marker already travels with the comparison completion.
    pub(super) fn report_relation_overflow_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        overflow: RelationOverflow,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let display = (|| {
            let source = sequence
                .read(self.raw_diagnostic_type_display_completion(
                    source,
                    None,
                    DiagnosticTypeFormat::DEFAULT,
                    consumer,
                ))?
                .into_bytes();
            let target = sequence
                .read(self.raw_diagnostic_type_display_completion(
                    target,
                    None,
                    DiagnosticTypeFormat::DEFAULT,
                    consumer,
                ))?
                .into_bytes();
            let message = match overflow {
                RelationOverflow::Complexity => Message::Complexity,
                RelationOverflow::Depth => Message::Depth,
            };
            self.relations.session_mut(session).error.report(
                message,
                vec![Argument::Text(source), Argument::Text(target)],
            );
            Ok::<_, DemandError>(())
        })();
        sequence.finish(display)
    }

    /// Native property diagnostics evaluate symbolToString before each ordered
    /// TypeToString argument. This helper owns only that repeated argument form.
    pub(super) fn report_property_relation_completion(
        &mut self,
        message: Message,
        property: PropertySymbolId,
        types: &[TypeId],
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let name = sequence
                .read(self.raw_diagnostic_property_symbol_display_completion(property, consumer))?
                .into_bytes();
            let mut arguments = vec![Argument::Text(name)];
            for &ty in types {
                arguments.push(Argument::Text(
                    sequence
                        .read(self.raw_diagnostic_type_display_completion(
                            ty,
                            None,
                            DiagnosticTypeFormat::DEFAULT,
                            consumer,
                        ))?
                        .into_bytes(),
                ));
            }
            self.relations
                .session_mut(session)
                .error
                .report(message, arguments);
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `shouldReportUnmatchedPropertyError` asks raw structured signatures and
    /// object properties before the apparent target signature questions.
    pub(super) fn should_report_unmatched_property_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let result = (|| -> Result<bool, DemandError> {
            let source_members = self.structured_type_members(source, consumer)?;
            let calls = source_members
                .as_ref()
                .is_some_and(|members| !members.call_signatures.is_empty());
            let source_members = self.structured_type_members(source, consumer)?;
            let constructs = source_members
                .as_ref()
                .is_some_and(|members| !members.construct_signatures.is_empty());
            let properties = self.properties_of_object_type(source, consumer)?;
            if (calls || constructs) && properties.is_empty() {
                if !self.call_signatures(target, consumer)?.is_empty() && calls {
                    return Ok(true);
                }
                return Ok(!self.construct_signatures(target, consumer)?.is_empty() && constructs);
            }
            Ok(true)
        })();
        result.map(NativeCompletion::clean)
    }

    /// `reportUnmatchedProperty` repeats the complete unmatched-property
    /// question before selecting the single, short-list or long-list message.
    pub(super) fn report_unmatched_property_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        unmatched: PropertySymbolId,
        require_optional: bool,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            if let Some(declaration) =
                self.property_reference_value_declaration(unmatched, consumer)?
                && self
                    .reference_private_identifier_declaration_name(declaration)?
                    .is_some()
                && self
                    .type_symbol_flags(source)?
                    .is_some_and(|flags| flags.class)
            {
                return Err(self.relation_pending(
                    consumer,
                    "native private identifier symbol-key relation display is pending",
                ));
            }
            let properties = sequence.read(self.unmatched_properties_completion(
                source,
                target,
                require_optional,
                false,
                consumer,
            ))?;
            if properties.len() == 1 {
                let (source_name, target_name) = sequence.read(
                    self.raw_type_names_for_error_display_completion(source, target, consumer),
                )?;
                let name = sequence
                    .read(
                        self.raw_diagnostic_property_symbol_display_completion(unmatched, consumer),
                    )?
                    .into_bytes();
                self.relations.session_mut(session).error.report(
                    Message::MissingProperty,
                    vec![
                        Argument::Text(name),
                        Argument::Text(source_name),
                        Argument::Text(target_name),
                    ],
                );
                // Native addRelatedInfo reads the actual declarations even though
                // this product does not publish the resulting diagnostic note.
                self.property_reference_declarations(unmatched, consumer)?;
            } else if sequence.read(self.elaborate_array_like_relation_completion(
                source,
                target,
                session.reporting(false),
                consumer,
            ))? {
                let (source_name, target_name) = sequence.read(
                    self.raw_type_names_for_error_display_completion(source, target, consumer),
                )?;
                let count = if properties.len() > 5 {
                    4
                } else {
                    properties.len()
                };
                let mut names = Vec::new();
                for property in &properties[..count] {
                    let symbol = self.property_symbol(property, consumer)?;
                    let name =
                        sequence
                            .read(self.raw_diagnostic_property_symbol_display_completion(
                                symbol, consumer,
                            ))?
                            .into_bytes();
                    if !names.is_empty() {
                        names.extend_from_slice(b", ");
                    }
                    names.extend_from_slice(&name);
                }
                let mut arguments = vec![
                    Argument::Text(source_name),
                    Argument::Text(target_name),
                    Argument::Text(names),
                ];
                let message = if properties.len() > 5 {
                    arguments.push(Argument::Number((properties.len() - 4) as isize));
                    Message::MissingPropertiesAndMore
                } else {
                    Message::MissingProperties
                };
                self.relations
                    .session_mut(session)
                    .error
                    .report(message, arguments);
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `indexInfoRelatedTo`'s diagnostic reads key displays only after the
    /// value comparison has rejected, and elides the repeated identical key.
    pub(super) fn report_index_relation_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let source_name = sequence
                .read(self.raw_diagnostic_type_display_completion(
                    source,
                    None,
                    DiagnosticTypeFormat::DEFAULT,
                    consumer,
                ))?
                .into_bytes();
            let mut arguments = vec![Argument::Text(source_name)];
            let message = if source == target {
                Message::IndexTypes
            } else {
                arguments.push(Argument::Text(
                    sequence
                        .read(self.raw_diagnostic_type_display_completion(
                            target,
                            None,
                            DiagnosticTypeFormat::DEFAULT,
                            consumer,
                        ))?
                        .into_bytes(),
                ));
                Message::DifferentIndexTypes
            };
            self.relations
                .session_mut(session)
                .error
                .report(message, arguments);
            Ok(())
        })();
        sequence.finish(result)
    }
    /// `reportErrorResults` restores original aliases only after its two base
    /// questions. Elaboration and synthetic-constraint reads remain semantic
    /// work even when a later chain transformation suppresses their text.
    pub(super) fn report_error_results_completion(
        &mut self,
        originals: (TypeId, TypeId),
        normalized: (TypeId, TypeId),
        head: Option<Message>,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let source_base = sequence
                .read(self.non_augmenting_reference_base_completion(originals.0, consumer))?
                .is_some();
            let target_base = sequence
                .read(self.non_augmenting_reference_base_completion(originals.1, consumer))?
                .is_some();
            let source = if self.model.metadata(originals.0)?.alias.is_some() || source_base {
                originals.0
            } else {
                normalized.0
            };
            let target = if self.model.metadata(originals.1)?.alias.is_some() || target_base {
                originals.1
            } else {
                normalized.1
            };
            if self.model.has_type_flags(source, TypeFlags::OBJECT)
                && self.model.has_type_flags(target, TypeFlags::OBJECT)
            {
                sequence.read(self.elaborate_array_like_relation_completion(
                    source,
                    target,
                    session.reporting(true),
                    consumer,
                ))?;
            }
            if self.model.has_type_flags(source, TypeFlags::OBJECT)
                && self.model.has_type_flags(target, TypeFlags::PRIMITIVE)
            {
                sequence.read(
                    self.elaborate_primitive_wrapper_completion(source, target, session, consumer),
                )?;
            } else if self.type_symbol_identity(source)?.is_some()
                && self.model.has_type_flags(source, TypeFlags::OBJECT)
                && source == self.global_object_identity(consumer)?
            {
                self.relations
                    .session_mut(session)
                    .error
                    .report(Message::ObjectInsteadOfAny, Vec::new());
            } else if self
                .model
                .object_flags(source)?
                .intersects(ObjectFlags::JSX_ATTRIBUTES)
                && self.model.has_type_flags(target, TypeFlags::INTERSECTION)
            {
                return Err(self.relation_pending(
                    consumer,
                    "native JSX intrinsic relation elaboration is pending",
                ));
            } else if let Some(elaboration) = sequence
                .read(self.elaborate_never_intersection_completion(originals.1, consumer))?
            {
                let message = match elaboration.conflict {
                    super::super::type_diagnostics::NeverConflict::Discriminant => {
                        Message::NeverDiscriminant
                    }
                    super::super::type_diagnostics::NeverConflict::Private => Message::NeverPrivate,
                };
                self.relations.session_mut(session).error.report(
                    message,
                    vec![
                        Argument::Text(elaboration.type_name),
                        Argument::Text(elaboration.property_name),
                    ],
                );
            }
            sequence.read(
                self.report_relation_error_completion(head, source, target, session, consumer),
            )?;
            if self.model.has_type_flags(source, TypeFlags::TYPE_PARAMETER)
                && self
                    .type_symbol_first_declaration(source, consumer)?
                    .is_some()
                && sequence
                    .read(self.constraint_of_type_completion(source, consumer))?
                    .is_none()
            {
                let clone = self.clone_with_reporting_constraint(source, target, consumer)?;
                if sequence
                    .read(self.has_non_circular_base_constraint_completion(clone, consumer))?
                {
                    // The note itself is not output, but its native display
                    // can consume lazy type dependencies and must still run.
                    sequence.read(
                        self.raw_check_diagnostic_type_display_completion(target, consumer),
                    )?;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `tryElaborateErrorsForPrimitivesAndObjects` observes each global before
    /// its primitive-side guard, preserving the final ESSymbol getter demand.
    fn elaborate_primitive_wrapper_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            for (name, primitive) in [
                ("String", PrimitiveName::String),
                ("Number", PrimitiveName::Number),
                ("Boolean", PrimitiveName::Boolean),
                ("Symbol", PrimitiveName::Symbol),
            ] {
                if source == self.global_object_identity_named(name, consumer)?
                    && target == self.model.primitive(primitive)
                {
                    let target_name = sequence
                        .read(self.raw_diagnostic_type_display_completion(
                            target,
                            None,
                            DiagnosticTypeFormat::DEFAULT,
                            consumer,
                        ))?
                        .into_bytes();
                    let source_name = sequence
                        .read(self.raw_diagnostic_type_display_completion(
                            source,
                            None,
                            DiagnosticTypeFormat::DEFAULT,
                            consumer,
                        ))?
                        .into_bytes();
                    self.relations.session_mut(session).error.report(
                        Message::PrimitiveWrapper,
                        vec![Argument::Text(target_name), Argument::Text(source_name)],
                    );
                    break;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `reportRelationError` owns diagnostic-dependent constraint checks,
    /// literal generalization and chain suppression in their native order.
    pub(super) fn report_relation_error_completion(
        &mut self,
        mut message: Option<Message>,
        source: TypeId,
        target: TypeId,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let (source_name, target_name) = sequence
                .read(self.raw_type_names_for_error_display_completion(source, target, consumer))?;
            let mut generalized_source = source;
            let mut generalized_name = source_name.clone();
            if !self.model.has_type_flags(target, TypeFlags::NEVER)
                && self.model.is_literal_type(source)
                && !sequence
                    .read(self.type_could_have_top_level_singletons_completion(target, consumer))?
            {
                generalized_source = self.base_literal_type(source, consumer)?;
                generalized_name = sequence
                    .read(self.raw_diagnostic_type_display_completion(
                        generalized_source,
                        None,
                        DiagnosticTypeFormat::FULLY_QUALIFIED,
                        consumer,
                    ))?
                    .into_bytes();
            }
            let target_flags = if self.model.has_type_flags(target, TypeFlags::INDEXED_ACCESS)
                && !self.model.has_type_flags(source, TypeFlags::INDEXED_ACCESS)
            {
                let TypeView::IndexedAccess(view) = self.model.get(target)? else {
                    unreachable!()
                };
                self.model.type_flags(view.object_type)
            } else {
                self.model.type_flags(target)
            };
            let markers = self.model.marker_types();
            if target_flags.intersects(TypeFlags::TYPE_PARAMETER)
                && target != markers.super_for_check
                && target != markers.sub_for_check
            {
                let constraint =
                    sequence.read(self.base_constraint_completion(target, consumer))?;
                let mut assignable = None;
                if let Some(constraint) = constraint {
                    if sequence.read(self.raw_type_related_completion(
                        generalized_source,
                        constraint,
                        RelationKind::Assignable,
                        consumer,
                    ))? {
                        assignable = Some(generalized_name.clone());
                    } else if sequence.read(self.raw_type_related_completion(
                        source,
                        constraint,
                        RelationKind::Assignable,
                        consumer,
                    ))? {
                        assignable = Some(source_name.clone());
                    }
                }
                if let (Some(constraint), Some(name)) = (constraint, assignable) {
                    let constraint_name = sequence
                        .read(self.raw_diagnostic_type_display_completion(
                            constraint,
                            None,
                            DiagnosticTypeFormat::DEFAULT,
                            consumer,
                        ))?
                        .into_bytes();
                    self.relations.session_mut(session).error.report(
                        Message::AssignableToConstraint,
                        vec![
                            Argument::Text(name),
                            Argument::Text(target_name.clone()),
                            Argument::Text(constraint_name),
                        ],
                    );
                } else {
                    let error = &mut self.relations.session_mut(session).error;
                    error.chain = None;
                    error.report(
                        Message::ArbitraryInstantiation,
                        vec![
                            Argument::Text(target_name.clone()),
                            Argument::Text(generalized_name.clone()),
                        ],
                    );
                }
            }
            let exact = self.inputs.semantic_options().exact_optional_property_types;
            if message.is_none() {
                message = Some(
                    if self.relations.session(session).kind == RelationKind::Comparable {
                        Message::Comparable
                    } else if source_name == target_name {
                        Message::DifferentTypesWithSameName
                    } else if exact
                        && !sequence
                            .read(self.exact_optional_unassignable_properties_completion(
                                source, target, consumer,
                            ))?
                            .is_empty()
                    {
                        Message::ExactOptionalAssignable
                    } else {
                        if self.model.has_type_flags(source, TypeFlags::STRING_LITERAL)
                            && self.model.has_type_flags(target, TypeFlags::UNION)
                            && let Some(suggested) =
                                sequence.read(self.suggested_string_literal_type_completion(
                                    source, target, consumer,
                                ))?
                        {
                            let suggested = sequence
                                .read(self.raw_diagnostic_type_display_completion(
                                    suggested,
                                    None,
                                    DiagnosticTypeFormat::DEFAULT,
                                    consumer,
                                ))?
                                .into_bytes();
                            self.relations.session_mut(session).error.report(
                                Message::SuggestedLiteral,
                                vec![
                                    Argument::Text(generalized_name),
                                    Argument::Text(target_name),
                                    Argument::Text(suggested),
                                ],
                            );
                            return Ok(());
                        }
                        Message::Assignable
                    },
                );
            } else if message == Some(Message::Argument)
                && exact
                && !sequence
                    .read(self.exact_optional_unassignable_properties_completion(
                        source, target, consumer,
                    ))?
                    .is_empty()
            {
                message = Some(Message::ExactOptionalArgument);
            }
            let message = message.expect("relation diagnostic selected");
            let error = &mut self.relations.session_mut(session).error;
            let suppressed = match error.message(0) {
                Some(message) if message.excess_property() => true,
                Some(Message::Complexity | Message::Depth | Message::Readonly) => error
                    .arguments_match(&[
                        Some(Argument::Text(generalized_name.clone())),
                        Some(Argument::Text(target_name.clone())),
                    ]),
                Some(Message::MissingProperty) => {
                    !message.conversion_or_implementation()
                        && error.arguments_match(&[
                            None,
                            Some(Argument::Text(generalized_name.clone())),
                            Some(Argument::Text(target_name.clone())),
                        ])
                }
                Some(Message::MissingProperties | Message::MissingPropertiesAndMore) => {
                    !message.conversion_or_implementation()
                        && error.arguments_match(&[
                            Some(Argument::Text(generalized_name.clone())),
                            Some(Argument::Text(target_name.clone())),
                        ])
                }
                _ => false,
            };
            if !suppressed {
                error.report(
                    message,
                    vec![
                        Argument::Text(generalized_name),
                        Argument::Text(target_name),
                    ],
                );
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `getSuggestedTypeForNonexistentStringLiteralType` supplies exact
    /// literal values and native TypeId tie ordering to the shared policy.
    fn suggested_string_literal_type_completion(
        &self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let TypeView::Literal { value: name, .. } = self.model.get(source)? else {
                unreachable!()
            };
            let types = self
                .model
                .union_constituents(target)
                .expect("string suggestion target union");
            let mut candidates = Vec::new();
            for &ty in types.iter() {
                if self.model.has_type_flags(ty, TypeFlags::STRING_LITERAL) {
                    let TypeView::Literal { value, .. } = self.model.get(ty)? else {
                        unreachable!()
                    };
                    candidates.push((ty, self.literal_scalar_text(&value, consumer)?.to_owned()));
                }
            }
            Ok(crate::spelling::suggestion(
                self.literal_scalar_text(&name, consumer)?,
                candidates,
                |left, right| left.cmp(&right),
            ))
        })();
        result.map(NativeCompletion::clean)
    }

    /// `compareTypePredicateRelatedTo` compares native kind before parameter
    /// position, then the already acquired asserted types. Rendering follows
    /// the rejecting branch, preserving its source-before-target schedule.
    pub(super) fn predicate_related_completion(
        &mut self,
        source: &Predicate,
        target: &Predicate,
        intersection: IntersectionState,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            let kind = |predicate: &Predicate| {
                (
                    predicate.kind == TypePredicateKind::Is,
                    matches!(predicate.subject, TypePredicateSubject::This(_)),
                )
            };
            if kind(source) != kind(target) {
                if session.report {
                    self.relations
                        .session_mut(session)
                        .error
                        .report(Message::PredicateKinds, Vec::new());
                    sequence.read(
                        self.report_predicate_types_completion(source, target, session, consumer),
                    )?;
                }
                return Ok(Ternary::False);
            }
            if let (
                TypePredicateSubject::ParameterIndex(source_index),
                TypePredicateSubject::ParameterIndex(target_index),
            ) = (source.subject, target.subject)
                && source_index != target_index
            {
                if session.report {
                    let source_name = source.parameter_name.as_ref().ok_or_else(|| {
                        super::super::super::internal(
                            "identifier predicate has no actual parameter name",
                        )
                    })?;
                    let target_name = target.parameter_name.as_ref().ok_or_else(|| {
                        super::super::super::internal(
                            "identifier predicate has no actual parameter name",
                        )
                    })?;
                    self.relations.session_mut(session).error.report(
                        Message::PredicateParameterPosition,
                        vec![
                            Argument::Text(source_name.as_bytes().to_vec()),
                            Argument::Text(target_name.as_bytes().to_vec()),
                        ],
                    );
                    sequence.read(
                        self.report_predicate_types_completion(source, target, session, consumer),
                    )?;
                }
                return Ok(Ternary::False);
            }
            let related = match (source.value_type, target.value_type) {
                (None, None) => Ternary::True,
                (Some(source), Some(target)) => {
                    let source = sequence.read(self.demand_completion(source))?;
                    let target = sequence.read(self.demand_completion(target))?;
                    if source == target {
                        Ternary::True
                    } else {
                        sequence.read(self.related_in_session_completion(
                            source,
                            target,
                            RecursionFlags::BOTH,
                            intersection,
                            session.without_head(),
                            consumer,
                        ))?
                    }
                }
                _ => Ternary::False,
            };
            if related == Ternary::False && session.report {
                sequence.read(
                    self.report_predicate_types_completion(source, target, session, consumer),
                )?;
            }
            Ok(related)
        })();
        sequence.finish(result)
    }

    fn report_predicate_types_completion(
        &mut self,
        source: &Predicate,
        target: &Predicate,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let source_name = sequence
                .read(self.raw_diagnostic_predicate_display_completion(source, consumer))?
                .into_bytes();
            let target_name = sequence
                .read(self.raw_diagnostic_predicate_display_completion(target, consumer))?
                .into_bytes();
            self.relations.session_mut(session).error.report(
                Message::PredicateTypes,
                vec![Argument::Text(source_name), Argument::Text(target_name)],
            );
            Ok(())
        })();
        sequence.finish(result)
    }

    pub(super) fn report_signature_must_be_predicate_completion(
        &mut self,
        source: &Signature,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let name = sequence
                .read(self.raw_diagnostic_signature_display_completion(source, consumer))?
                .into_bytes();
            self.relations
                .session_mut(session)
                .error
                .report(Message::MustBePredicate, vec![Argument::Text(name)]);
            Ok(())
        })();
        sequence.finish(result)
    }

    pub(super) fn report_no_matching_signature_completion(
        &mut self,
        source: TypeId,
        target: &Signature,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let source_name = sequence
                .read(self.raw_diagnostic_type_display_completion(
                    source,
                    None,
                    DiagnosticTypeFormat::DEFAULT,
                    consumer,
                ))?
                .into_bytes();
            let target_name = sequence
                .read(self.raw_diagnostic_signature_display_completion(target, consumer))?
                .into_bytes();
            self.relations.session_mut(session).error.report(
                Message::NoMatchingSignature,
                vec![Argument::Text(source_name), Argument::Text(target_name)],
            );
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `compareSignaturesRelated` emits these markers using raw parameter
    /// vector emptiness, separately from expanded counts and minimum arity.
    pub(super) fn report_signature_return_types_completion(
        &mut self,
        source: &Signature,
        target: &Signature,
        returns: (TypeId, TypeId),
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let no_arguments = source.parameters.is_empty() && target.parameters.is_empty();
            let message = match (source.flags.is_construct(), no_arguments) {
                (false, true) => Message::CallReturnNoArguments,
                (false, false) => Message::CallReturn,
                (true, true) => Message::ConstructReturnNoArguments,
                (true, false) => Message::ConstructReturn,
            };
            let source_name = sequence
                .read(self.raw_diagnostic_type_display_completion(
                    returns.0,
                    None,
                    DiagnosticTypeFormat::DEFAULT,
                    consumer,
                ))?
                .into_bytes();
            let target_name = sequence
                .read(self.raw_diagnostic_type_display_completion(
                    returns.1,
                    None,
                    DiagnosticTypeFormat::DEFAULT,
                    consumer,
                ))?
                .into_bytes();
            self.relations.session_mut(session).error.report(
                message,
                vec![Argument::Text(source_name), Argument::Text(target_name)],
            );
            Ok(())
        })();
        sequence.finish(result)
    }

    /// The weak-target branch of `isRelatedToEx` consumes both signature
    /// inventories before its ordered call-return/construct-return choices.
    /// A diagnostic rollback cannot undo a failure from either visited return.
    pub(super) fn report_no_common_properties_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        originals: (TypeId, TypeId),
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let source_display = if self.model.metadata(originals.0)?.alias.is_some() {
                originals.0
            } else {
                source
            };
            let target_display = if self.model.metadata(originals.1)?.alias.is_some() {
                originals.1
            } else {
                target
            };
            let source_name = sequence
                .read(self.raw_diagnostic_type_display_completion(
                    source_display,
                    None,
                    DiagnosticTypeFormat::DEFAULT,
                    consumer,
                ))?
                .into_bytes();
            let target_name = sequence
                .read(self.raw_diagnostic_type_display_completion(
                    target_display,
                    None,
                    DiagnosticTypeFormat::DEFAULT,
                    consumer,
                ))?
                .into_bytes();
            let calls = self.call_signatures(source, consumer)?;
            let constructs = self.construct_signatures(source, consumer)?;
            let mut callable = false;
            for signature in calls.first().into_iter().chain(constructs.first()) {
                let returned =
                    sequence.read(self.raw_return_type_of_signature_completion(signature))?;
                if sequence.read(self.related_in_session_completion(
                    returned,
                    target,
                    RecursionFlags::SOURCE,
                    IntersectionState::NONE,
                    session.reporting(false),
                    consumer,
                ))? != Ternary::False
                {
                    callable = true;
                    break;
                }
            }
            self.relations.session_mut(session).error.report(
                if callable {
                    Message::DidYouMeanToCall
                } else {
                    Message::NoCommonProperties
                },
                vec![Argument::Text(source_name), Argument::Text(target_name)],
            );
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `tryElaborateArrayLikeErrors` distinguishes readonly rejection from
    /// the decision to skip ordinary unmatched-property elaboration.
    pub(super) fn elaborate_array_like_relation_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let readonly = if self.model.is_tuple_type(source) {
                let tuple = self
                    .model
                    .tuple_target(source)?
                    .expect("tuple source has target");
                if !tuple.readonly || !self.is_mutable_array_or_tuple(target)? {
                    return self.is_array_or_tuple_type(target);
                }
                true
            } else {
                self.is_readonly_array_type(source)? && self.is_mutable_array_or_tuple(target)?
            };
            if readonly {
                if session.report {
                    let source_name = sequence
                        .read(self.raw_diagnostic_type_display_completion(
                            source,
                            None,
                            DiagnosticTypeFormat::DEFAULT,
                            consumer,
                        ))?
                        .into_bytes();
                    let target_name = sequence
                        .read(self.raw_diagnostic_type_display_completion(
                            target,
                            None,
                            DiagnosticTypeFormat::DEFAULT,
                            consumer,
                        ))?
                        .into_bytes();
                    self.relations.session_mut(session).error.report(
                        Message::Readonly,
                        vec![Argument::Text(source_name), Argument::Text(target_name)],
                    );
                }
                return Ok(false);
            }
            if self.model.is_tuple_type(target) {
                return self.is_array_type(source);
            }
            Ok(true)
        })();
        sequence.finish(result)
    }

    /// `typeCouldHaveTopLevelSingletonTypes` excludes Boolean before its
    /// underlying union, and follows the guarded constraint of instantiables.
    fn type_could_have_top_level_singletons_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if self.model.has_type_flags(ty, TypeFlags::BOOLEAN) {
                return Ok(false);
            }
            if let Some(types) = self
                .model
                .union_constituents(ty)
                .or_else(|| self.model.intersection_constituents(ty))
            {
                for &ty in types.iter() {
                    if sequence
                        .read(self.type_could_have_top_level_singletons_completion(ty, consumer))?
                    {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            if self.model.has_type_flags(ty, TypeFlags::INSTANTIABLE)
                && let Some(constraint) =
                    sequence.read(self.constraint_of_type_completion(ty, consumer))?
                && constraint != ty
            {
                return sequence.read(
                    self.type_could_have_top_level_singletons_completion(constraint, consumer),
                );
            }
            Ok(self.model.has_type_flags(
                ty,
                TypeFlags::UNIT | TypeFlags::TEMPLATE_LITERAL | TypeFlags::STRING_MAPPING,
            ))
        })();
        sequence.finish(result)
    }

    /// `getExactOptionalUnassignableProperties` is a complete filter, not a
    /// Some shortcut. Target raw types are read even when source lookup is nil.
    fn exact_optional_unassignable_properties_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<Property>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Property>, DemandError> {
            if self.model.is_tuple_type(source) && self.model.is_tuple_type(target) {
                return Ok(Vec::new());
            }
            let mut unmatched = Vec::new();
            for property in self.properties_of_type(target, consumer)? {
                let symbol = self.property_symbol(&property, consumer)?;
                let name = self.property_lookup_key(symbol, consumer)?;
                let source = self.type_of_property_of_type_by_key(source, &name, consumer)?;
                let target = sequence.read(self.demand_completion(property.read_type))?;
                if self.exact_optional_property_mismatch(source, Some(target)) {
                    unmatched.push(property);
                }
            }
            Ok(unmatched)
        })();
        sequence.finish(result)
    }
}
