//! Contextual checking is a distinct expression question from normal caches.

mod elements;
mod returns;

use super::super::model::{
    DemandError, InferenceContextId, NativeCompletion, NativeResult, NativeSequence, SpecialName,
    TypeFlags, TypeId,
};
pub(super) use super::super::source::ExpressionNode as ContextNode;
use super::super::source::{ContextRecipe, Expression, ExpressionRoot};
use super::Evaluator;
use crate::analysis::SemanticFailureReason;
use crate::references::Location;

/// Native expression-check flags are invocation arguments. Dynamic contextual
/// and inference stacks are separate state and never imply an ambient mode.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct CheckMode(u8);

impl CheckMode {
    pub(super) const NORMAL: Self = Self(0);
    pub(super) const CONTEXTUAL: Self = Self(1);
    pub(super) const INFERENTIAL: Self = Self(1 << 1);
    pub(super) const SKIP_CONTEXT_SENSITIVE: Self = Self(1 << 2);
    pub(super) const SKIP_GENERIC_FUNCTIONS: Self = Self(1 << 3);
    pub(super) const REST_BINDING_ELEMENT: Self = Self(1 << 5);
    pub(super) const TYPE_ONLY: Self = Self(1 << 6);
    pub(super) const FORCE_TUPLE: Self = Self(1 << 7);

    pub(super) fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }
    pub(super) fn without(self, flag: Self) -> Self {
        Self(self.0 & !flag.0)
    }
}

impl std::ops::BitOr for CheckMode {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Clone, Copy)]
pub(super) struct ContextFlags(u8);

impl ContextFlags {
    pub(super) const NONE: Self = Self(0);
    pub(super) const SIGNATURE: Self = Self(1);
    pub(super) const NO_CONSTRAINTS: Self = Self(1 << 1);
    pub(super) const SKIP_BINDING_PATTERNS: Self = Self(1 << 3);

    pub(super) fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }
}

struct ContextFrame {
    node: ContextNode,
    ty: Option<TypeId>,
    is_cache: bool,
}

struct InferenceFrame {
    node: ContextNode,
    inference: Option<InferenceContextId>,
}

#[derive(Default)]
pub(super) struct ContextualState {
    frames: Vec<ContextFrame>,
    inference_frames: Vec<InferenceFrame>,
    next_synthetic_node: usize,
}

impl Evaluator<'_, '_> {
    /// `getContextualTypeForBinaryOperand`: assignment, fallback and right-only
    /// forwarding are distinct native questions. The source recipe has already
    /// honored a binary annotation for either operand.
    fn contextual_binary_operand_completion(
        &mut self,
        parent: ExpressionRoot,
        operator: crate::syntax_kind::SyntaxKind,
        left: ExpressionRoot,
        is_right: bool,
        flags: ContextFlags,
    ) -> NativeResult<Option<TypeId>> {
        use crate::syntax_kind::SyntaxKind as K;
        match binary_context_rule(operator) {
            BinaryContextRule::Assignment if is_right => {
                let target = self.source.leftmost_expression(left, false)?;
                if target.site.kind == K::Identifier {
                    let location =
                        super::super::location(self.inputs, target.file, target.site.span);
                    // `ModuleExports` is created by the excluded JS binder.
                    // The raw getResolvedSymbol acquisition still occurs for
                    // TS identifiers, including an actual unknownSymbol.
                    return self
                        .reference_resolved_identifier_symbol_completion(target, &location)?
                        .and_then(|_| self.contextual_assignment_type_completion(left));
                }
                self.contextual_assignment_type_completion(left)
            }
            BinaryContextRule::Fallback => self
                .contextual_type_completion(parent, flags)?
                .and_then(|contextual| {
                    if is_right && self.fallback_uses_left_operand(contextual) {
                        Ok(self
                            .type_of_expression_completion(left.file, left.site)?
                            .map(Some))
                    } else {
                        Ok(NativeCompletion::clean(contextual))
                    }
                }),
            BinaryContextRule::RightOnly if is_right => {
                self.contextual_type_completion(parent, flags)
            }
            _ => Ok(NativeCompletion::clean(None)),
        }
    }

    /// `getContextualTypeForBinaryOperand`'s `||`/`??` right operand takes
    /// the left operand's type when the binary has no contextual type or one
    /// that originates in a binding pattern.
    fn fallback_uses_left_operand(&self, contextual: Option<TypeId>) -> bool {
        contextual.is_none_or(|ty| self.has_binding_pattern_type(ty))
    }

    /// Q5's spread contribution: whether an object literal's contextual type
    /// is derived, step by `getContextualType` step, from an enclosing object
    /// literal's spread operand. It follows `reference_context`'s recipes
    /// through parentheses, `!`, `as const`, conditional arms, `await` and the
    /// binary operands `getContextualTypeForBinaryOperand` gives the binary's
    /// own context, and stops at any context of its own.
    pub(super) fn spread_contribution(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, DemandError> {
        let mut reference: super::reads::ReferenceNode = root.into();
        loop {
            let file = reference.file;
            let position = self.reference_position(reference)?;
            match self.source.reference_context(file, position)?.recipe {
                ContextRecipe::Spread(_) => return Ok(true),
                ContextRecipe::Forward(parent) | ContextRecipe::AwaitOperand(parent) => {
                    reference = (file, parent).into()
                }
                ContextRecipe::BinaryOperand {
                    parent,
                    operator,
                    is_right,
                    ..
                } => {
                    let forwards = match binary_context_rule(operator) {
                        BinaryContextRule::Fallback => {
                            !is_right || {
                                let contextual = self
                                    .contextual_type_completion(
                                        ExpressionRoot { file, site: parent },
                                        ContextFlags::NONE,
                                    )?
                                    .admit()?;
                                !self.fallback_uses_left_operand(contextual)
                            }
                        }
                        BinaryContextRule::RightOnly => is_right,
                        BinaryContextRule::Assignment | BinaryContextRule::Absent => false,
                    };
                    if !forwards {
                        return Ok(false);
                    }
                    reference = (file, parent).into();
                }
                _ => return Ok(false),
            }
        }
    }

    /// `getContextualTypeForAssignmentExpression` delegates the final read to
    /// getTypeOfExpression. Access targets also consume actual assignment-symbol
    /// and this-property metadata; their absent attachment cannot be inferred
    /// merely from this project's exclusion of expando typing.
    fn contextual_assignment_type_completion(
        &mut self,
        left: ExpressionRoot,
    ) -> NativeResult<Option<TypeId>> {
        use crate::syntax_kind::SyntaxKind as K;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if matches!(
                left.site.kind,
                K::PropertyAccessExpression | K::ElementAccessExpression
            ) {
                let access = self.source.access_recipe(left)?;
                if let Some(receiver) = access.receiver.root {
                    if receiver.site.kind == K::Identifier {
                        let symbol =
                            sequence.read(self.reference_resolved_identifier_symbol_completion(
                                receiver,
                                &access.receiver.location,
                            ))?;
                        self.reference_export_symbol_of_value_if_exported(
                            symbol,
                            &access.receiver.location,
                        )?;
                    }
                    if matches!(
                        receiver.site.kind,
                        K::Identifier
                            | K::PropertyAccessExpression
                            | K::ElementAccessExpression
                            | K::ThisKeyword
                    ) {
                        return Err(super::super::unsupported_demand_at_location(
                            self.inputs,
                            &access.location,
                            SemanticFailureReason::UnsupportedSemanticForm,
                            "native contextual assignment requires its retained assignment-symbol and this-property metadata",
                        ));
                    }
                }
            }
            sequence
                .read(self.type_of_expression_completion(left.file, left.site))
                .map(Some)
        })();
        sequence.finish(result)
    }

    /// Product admission for the shared native contextual-type question.
    pub(super) fn contextual_type(
        &mut self,
        reference: impl Into<super::reads::ReferenceNode>,
        flags: ContextFlags,
    ) -> Result<Option<TypeId>, DemandError> {
        self.contextual_type_completion(reference, flags)?.admit()
    }

    /// `getContextualType`: InWith precedes active contexts; exact dynamic
    /// context-node entries precede the parent-specific semantic question.
    /// `findContextualNode` selects the oldest matching type frame; inference
    /// context lookup below deliberately has the opposite search direction.
    pub(super) fn contextual_type_completion(
        &mut self,
        reference: impl Into<super::reads::ReferenceNode>,
        flags: ContextFlags,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut reference = reference.into();
            loop {
                let file = reference.file;
                let position = self.reference_position(reference)?;
                let context = self.source.reference_context(file, position)?;
                if context.in_with {
                    return Ok(None);
                }
                if let Some(value) = reference.captured_site().and_then(|site| {
                    self.contextual_type_frame(
                        ContextNode::Captured(ExpressionRoot { file, site }),
                        flags,
                    )
                }) {
                    return Ok(value);
                }
                match context.recipe {
                    ContextRecipe::Absent => return Ok(None),
                    ContextRecipe::VariableInitializer(declaration) => {
                        return sequence.read(
                            self.contextual_variable_initializer_completion(declaration, flags),
                        );
                    }
                    ContextRecipe::Forward(parent) | ContextRecipe::Spread(parent) => {
                        reference = (file, parent).into()
                    }
                    ContextRecipe::AwaitOperand(parent) => {
                        let root = ExpressionRoot { file, site: parent };
                        let consumer = super::super::location(self.inputs, file, parent.span);
                        return sequence.read(
                            self.contextual_await_operand_completion(root, flags, &consumer),
                        );
                    }
                    ContextRecipe::Annotation(annotation) => {
                        return sequence
                            .read(self.check_type_completion(&annotation))
                            .map(Some);
                    }
                    ContextRecipe::Return(function) => {
                        let Some(function) = function else {
                            return Ok(None);
                        };
                        let consumer = super::super::location(self.inputs, file, function.span());
                        return sequence.read(self.contextual_return_expression_type_completion(
                            function, flags, &consumer,
                        ));
                    }
                    ContextRecipe::ObjectElement(site) => {
                        let consumer = super::super::location(self.inputs, file, site.span);
                        return sequence.read(self.contextual_object_element_type_completion(
                            ExpressionRoot { file, site },
                            flags,
                            &consumer,
                        ));
                    }
                    ContextRecipe::CallArgument { parent, argument } => {
                        return sequence
                            .read(self.contextual_argument_type_completion(parent, argument));
                    }
                    ContextRecipe::BinaryOperand {
                        parent,
                        operator,
                        left,
                        is_right,
                    } => {
                        return sequence.read(self.contextual_binary_operand_completion(
                            ExpressionRoot { file, site: parent },
                            operator,
                            left,
                            is_right,
                            flags,
                        ));
                    }
                    ContextRecipe::ArrayElement {
                        parent,
                        index,
                        length,
                    } => {
                        let root = ExpressionRoot { file, site: parent };
                        let consumer = super::super::location(self.inputs, file, parent.span);
                        let contextual = sequence.read(
                            self.apparent_contextual_type_completion(root, flags, &consumer),
                        )?;
                        let Some(index) = index else {
                            return Ok(None);
                        };
                        let spreads = self.source.array_spread_indices(root)?;
                        return sequence.read(self.contextual_element_type_completion(
                            contextual,
                            index,
                            Some(length),
                            spreads.first,
                            spreads.last,
                            &consumer,
                        ));
                    }
                    ContextRecipe::Pending { location, message } => {
                        return Err(super::super::unsupported_demand_at_location(
                            self.inputs,
                            &location,
                            SemanticFailureReason::UnsupportedSemanticForm,
                            message,
                        ));
                    }
                }
            }
        })();
        sequence.finish(result)
    }

    /// `getContextualTypeForAwaitOperand`: obtain the parent's context first,
    /// then its awaited type without an alias. Absent context and actual nil
    /// each stop before the PromiseLike getter; acquired recoveries keep their
    /// order. A present awaited type supplies both union constituents.
    fn contextual_await_operand_completion(
        &mut self,
        parent: ExpressionRoot,
        flags: ContextFlags,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        use crate::resources::complete_or_abort;
        let _depth = self
            .control
            .as_ref()
            .map(|control| complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(contextual) = sequence.read(self.contextual_type_completion(parent, flags))?
            else {
                return Ok(None);
            };
            let Some(awaited) =
                sequence.read(self.awaited_type_no_alias_completion(contextual, consumer))?
            else {
                return Ok(None);
            };
            let promise =
                sequence.read(self.create_promise_like_type_completion(awaited, consumer))?;

            sequence
                .read(self.union_type_ex_completion(vec![awaited, promise], None, None, consumer))
                .map(Some)
        })();
        sequence.finish(result)
    }

    /// `findContextualNode` returns the oldest exact identity, retaining an
    /// installed nil type and excluding cache frames for flagged questions.
    fn contextual_type_frame(
        &self,
        node: ContextNode,
        flags: ContextFlags,
    ) -> Option<Option<TypeId>> {
        self.contextual
            .frames
            .iter()
            .find(|frame| {
                frame.node == node && (flags.0 == ContextFlags::NONE.0 || !frame.is_cache)
            })
            .map(|frame| frame.ty)
    }

    /// `getApparentTypeOfContextualType` keeps NoConstraints, mapped-object
    /// preservation and no-reduction constituent mapping in one operation.
    pub(super) fn apparent_contextual_type(
        &mut self,
        node: impl Into<ContextNode>,
        flags: ContextFlags,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        self.apparent_contextual_type_completion(node, flags, consumer)?
            .admit()
    }

    pub(super) fn apparent_contextual_type_completion(
        &mut self,
        node: impl Into<ContextNode>,
        flags: ContextFlags,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let node = node.into();
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let Some(apparent) = sequence.read(
                self.undiscriminated_apparent_contextual_type_completion(node, flags, consumer),
            )?
            else {
                return Ok(None);
            };
            if self.model.has_type_flags(apparent, TypeFlags::UNION)
                && let ContextNode::Captured(root) = node
            {
                if root.site.kind == crate::syntax_kind::SyntaxKind::ObjectLiteralExpression {
                    return sequence
                        .read(
                            self.discriminate_contextual_object_completion(
                                root, apparent, consumer,
                            ),
                        )
                        .map(Some);
                }
                if root.site.kind == crate::syntax_kind::SyntaxKind::JsxAttributes {
                    return Err(super::super::unsupported_demand_at_location(
                        self.inputs,
                        consumer,
                        SemanticFailureReason::UnsupportedSemanticForm,
                        "native JSX contextual discriminant selection requires the JSX producer",
                    ));
                }
            }
            Ok(Some(apparent))
        })();
        sequence.finish(result)
    }

    /// `getApparentTypeOfContextualType` up to its final discrimination: the
    /// instantiated contextual type with each constituent's apparent type,
    /// unreduced.
    pub(super) fn undiscriminated_apparent_contextual_type_completion(
        &mut self,
        node: ContextNode,
        flags: ContextFlags,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let Some(instantiated) = sequence
                .read(self.instantiated_contextual_type_completion(node, flags, consumer))?
            else {
                return Ok(None);
            };
            let apparent = sequence.read(self.map_type_no_reduction_completion(
                instantiated,
                consumer,
                |checker, ty| checker.contextual_apparent_type_completion(ty, consumer),
            ))?;
            Ok(Some(apparent))
        })();
        sequence.finish(result)
    }

    /// `getApparentTypeOfContextualType`'s mapper for one constituent: a
    /// mapped type stays, any other takes `getApparentType`, which rebuilds an
    /// intersection with its `this` argument and without its alias.
    pub(super) fn contextual_apparent_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if self
            .model
            .object_flags(ty)?
            .intersects(super::super::model::ObjectFlags::MAPPED)
        {
            Ok(NativeCompletion::clean(ty))
        } else {
            self.apparent_type_completion(ty, consumer)
        }
    }

    /// `getApparentTypeOfContextualType`'s first step: the contextual type,
    /// instantiated in its inference context. None when there is none, or
    /// for a type variable that `NoConstraints` keeps from its constraint.
    pub(super) fn instantiated_contextual_type_completion(
        &mut self,
        node: ContextNode,
        flags: ContextFlags,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let contextual = match node {
                ContextNode::Captured(root) => {
                    if self.source.expression_is_object_literal_method(root)? {
                        sequence.read(
                            self.contextual_object_element_type_completion(root, flags, consumer),
                        )?
                    } else {
                        sequence.read(self.contextual_type_completion(root, flags))?
                    }
                }
                ContextNode::Synthetic { parent, .. } => {
                    // `createSyntheticExpression` does not inherit InWith flags.
                    // Its distinct node can have an active contextual frame. The
                    // current producer's SpreadElement parent has no native
                    // getContextualType fallback; the parent's type is not ours.
                    if let Some(value) = self.contextual_type_frame(node, flags) {
                        value
                    } else if parent.site.kind == crate::syntax_kind::SyntaxKind::SpreadElement {
                        None
                    } else {
                        return Err(super::super::unsupported_demand_at_location(
                            self.inputs,
                            consumer,
                            SemanticFailureReason::UnsupportedSemanticForm,
                            "native synthetic contextual parent acquisition is pending",
                        ));
                    }
                }
            };
            let Some(contextual) = contextual else {
                return Ok(None);
            };
            let instantiated = sequence.read(self.instantiate_contextual_type_completion(
                contextual,
                Some(node),
                flags,
                consumer,
            ))?;
            if flags.contains(ContextFlags::NO_CONSTRAINTS)
                && self
                    .model
                    .has_type_flags(instantiated, TypeFlags::TYPE_VARIABLE)
            {
                return Ok(None);
            }
            Ok(Some(instantiated))
        })();
        sequence.finish(result)
    }

    /// `getInferenceContext` searches actual node ancestry from the newest
    /// dynamic context, including an explicit nil context that masks an outer.
    pub(super) fn inference_context_for(
        &mut self,
        node: impl Into<ContextNode>,
    ) -> Result<Option<InferenceContextId>, DemandError> {
        let node = node.into();
        for frame in self.contextual.inference_frames.iter().rev() {
            let descendant = match (node, frame.node) {
                (left, right) if left == right => true,
                (ContextNode::Captured(root), ContextNode::Captured(parent))
                | (ContextNode::Synthetic { parent: root, .. }, ContextNode::Captured(parent)) => {
                    self.source.expression_is_descendant(root, parent)?
                }
                _ => false,
            };
            if descendant {
                return Ok(frame.inference);
            }
        }
        Ok(None)
    }

    /// `pushCachedContextualType` / `popContextualType` retain nil answers and
    /// never create inference scopes. Unwinding preserves prior frame identity.
    pub(super) fn with_cached_contextual_type<T>(
        &mut self,
        root: ExpressionRoot,
        _consumer: &Location,
        operation: impl FnOnce(&mut Self) -> Result<T, DemandError>,
    ) -> Result<T, DemandError> {
        let ty = self.contextual_type(root, ContextFlags::NONE)?;
        let start = self.contextual.frames.len();
        self.contextual.frames.push(ContextFrame {
            node: root.into(),
            ty,
            is_cache: true,
        });
        let result = operation(self);
        self.contextual.frames.truncate(start);
        result
    }

    pub(super) fn new_synthetic_context_node(&mut self, parent: ExpressionRoot) -> ContextNode {
        let id = self.contextual.next_synthetic_node;
        self.contextual.next_synthetic_node += 1;
        ContextNode::Synthetic { id, parent }
    }

    /// `pushContextualType` / `popContextualType` for a non-cache override.
    /// Unlike `checkExpressionWithContextualType`, this creates no inference
    /// scope and performs no literal regularization after the operation.
    pub(super) fn with_contextual_type_override<T>(
        &mut self,
        root: ExpressionRoot,
        ty: TypeId,
        operation: impl FnOnce(&mut Self) -> Result<T, DemandError>,
    ) -> Result<T, DemandError> {
        let start = self.contextual.frames.len();
        self.contextual.frames.push(ContextFrame {
            node: root.into(),
            ty: Some(ty),
            is_cache: false,
        });
        let result = operation(self);
        self.contextual.frames.truncate(start);
        result
    }

    /// `checkExpressionWithContextualType`: scalar checking has no contextual
    /// member lookup, but still performs the exact native literal-context
    /// regularization. Structured/context-sensitive expressions require their
    /// actual context-node/inference stack rather than a noncontextual answer.
    pub(super) fn check_expression_with_contextual_type_completion(
        &mut self,
        expression: &Expression,
        contextual: TypeId,
        inference: Option<InferenceContextId>,
        mode: CheckMode,
    ) -> NativeResult<TypeId> {
        self.check_with_contextual_type_completion(
            expression.root.map(Into::into),
            contextual,
            inference,
            mode,
            &expression.location,
            |checker, mode| checker.check_expression_completion(expression, mode),
        )
    }

    /// `checkExpressionWithContextualType` for native SyntheticExpression:
    /// the shared synthetic worker and generic-signature post-step run inside
    /// the contextual frame, before literal regularization and frame cleanup.
    #[expect(
        clippy::too_many_arguments,
        reason = "ports checkExpressionWithContextualType's parameters plus the request consumer"
    )]
    pub(super) fn check_synthetic_expression_with_contextual_type_completion(
        &mut self,
        node: ContextNode,
        ty: TypeId,
        spread: bool,
        contextual: TypeId,
        inference: Option<InferenceContextId>,
        mode: CheckMode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.check_with_contextual_type_completion(
            Some(node),
            contextual,
            inference,
            mode,
            consumer,
            |checker, mode| {
                checker.check_synthetic_expression_completion(node, ty, spread, mode, consumer)
            },
        )
    }

    fn check_with_contextual_type_completion(
        &mut self,
        node: Option<ContextNode>,
        contextual: TypeId,
        inference: Option<InferenceContextId>,
        mode: CheckMode,
        consumer: &Location,
        check: impl FnOnce(&mut Self, CheckMode) -> NativeResult<TypeId>,
    ) -> NativeResult<TypeId> {
        let start = self.contextual.frames.len();
        let inference_start = self.contextual.inference_frames.len();
        if let Some(node) = node {
            self.contextual.frames.push(ContextFrame {
                node,
                ty: Some(contextual),
                is_cache: false,
            });
            self.contextual
                .inference_frames
                .push(InferenceFrame { node, inference });
        }
        let result = (|| {
            let mode = mode
                | CheckMode::CONTEXTUAL
                | if inference.is_some() {
                    CheckMode::INFERENTIAL
                } else {
                    CheckMode::NORMAL
                };
            check(self, mode)?.and_then(|ty| {
                if let Some(inference) = inference {
                    self.clear_intra_expression_sites(inference);
                }
                if self.model.maybe_type_of_kind(ty, TypeFlags::LITERAL) {
                    return self
                        .instantiate_contextual_type_completion(
                            contextual,
                            node,
                            ContextFlags::NONE,
                            consumer,
                        )?
                        .and_then(|contextual| {
                            Ok(self
                                .literal_of_contextual_type_completion(ty, contextual, consumer)?
                                .map(|literal| {
                                    if literal {
                                        self.model.regular_literal(ty)
                                    } else {
                                        ty
                                    }
                                }))
                        });
                }
                Ok(NativeCompletion::clean(ty))
            })
        })();
        if let Some(inference) = inference {
            self.clear_intra_expression_sites(inference);
        }
        self.contextual.inference_frames.truncate(inference_start);
        self.contextual.frames.truncate(start);
        result
    }

    /// `isLiteralOfContextualType`: unions search actual constituents before
    /// type-variable base constraints, then compare native literal categories.
    /// A matching category preserves even a literal with the wrong value;
    /// applicability's subsequent relation owns rejection of that value.
    pub(super) fn literal_of_contextual_type_completion(
        &mut self,
        candidate: TypeId,
        contextual: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if let Some(types) = self
                .model
                .union_constituents(contextual)
                .or_else(|| self.model.intersection_constituents(contextual))
            {
                for &ty in types.iter() {
                    if sequence
                        .read(self.literal_of_contextual_type_completion(candidate, ty, consumer))?
                    {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            if self
                .model
                .has_type_flags(contextual, TypeFlags::INSTANTIABLE_NON_PRIMITIVE)
            {
                let constraint =
                    match sequence.read(self.base_constraint_completion(contextual, consumer))? {
                        Some(constraint) => constraint,
                        None => self.model.special(SpecialName::Unknown),
                    };
                for (base, literal) in [
                    (TypeFlags::STRING, TypeFlags::STRING_LITERAL),
                    (TypeFlags::NUMBER, TypeFlags::NUMBER_LITERAL),
                    (TypeFlags::BIGINT, TypeFlags::BIGINT_LITERAL),
                    (TypeFlags::SYMBOL, TypeFlags::UNIQUE_SYMBOL),
                ] {
                    if self.model.maybe_type_of_kind(constraint, base)
                        && self.model.maybe_type_of_kind(candidate, literal)
                    {
                        return Ok(true);
                    }
                }
                return sequence.read(
                    self.literal_of_contextual_type_completion(candidate, constraint, consumer),
                );
            }
            Ok([
                (TypeFlags::STRING_LITERAL_CONTEXT, TypeFlags::STRING_LITERAL),
                (TypeFlags::NUMBER_LITERAL, TypeFlags::NUMBER_LITERAL),
                (TypeFlags::BIGINT_LITERAL, TypeFlags::BIGINT_LITERAL),
                (TypeFlags::BOOLEAN_LITERAL, TypeFlags::BOOLEAN_LITERAL),
                (TypeFlags::UNIQUE_SYMBOL, TypeFlags::UNIQUE_SYMBOL),
            ]
            .into_iter()
            .any(|(target, source)| {
                self.model.has_type_flags(contextual, target)
                    && self.model.maybe_type_of_kind(candidate, source)
            }))
        })();
        sequence.finish(result)
    }
}

/// `getContextualTypeForBinaryOperand`'s operator rows. An annotated binary
/// is answered by `reference_context` before this rule applies.
enum BinaryContextRule {
    /// `=`, `&&=`, `||=`, `??=`: the right operand takes the assignment target's type.
    Assignment,
    /// `||`, `??`: both operands take the binary's context, except as
    /// `fallback_uses_left_operand` says for the right one.
    Fallback,
    /// `&&`, `,`: only the right operand takes the binary's context.
    RightOnly,
    Absent,
}

fn binary_context_rule(operator: crate::syntax_kind::SyntaxKind) -> BinaryContextRule {
    use crate::syntax_kind::SyntaxKind as K;
    match operator {
        K::EqualsToken
        | K::AmpersandAmpersandEqualsToken
        | K::BarBarEqualsToken
        | K::QuestionQuestionEqualsToken => BinaryContextRule::Assignment,
        K::BarBarToken | K::QuestionQuestionToken => BinaryContextRule::Fallback,
        K::AmpersandAmpersandToken | K::CommaToken => BinaryContextRule::RightOnly,
        _ => BinaryContextRule::Absent,
    }
}
