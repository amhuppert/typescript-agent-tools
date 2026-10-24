//! Native loop junctions retain ordered incomplete results and cache scopes.

mod key;

use super::{FlowType, ReadInvocations, ReadStack, ReadState, ReadWork};
use crate::project::binding::NativeSymbolIdentity;
use crate::project::binding::flow::{FlowId, Graph, Node, Site};
use crate::project::semantic::evaluation::Evaluator;
use crate::project::semantic::evaluation::expression::FlowExpressionCache;
use crate::project::semantic::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, TypeFlags, TypeId,
};
use crate::project::semantic::source::ExpressionRoot;
use crate::project::sources::FileId;
use crate::syntax_kind::SyntaxKind;
pub(super) use key::FlowCacheKey;
use key::FlowCacheKeyBuilder;
use std::collections::HashMap;

/// Native key prefixes distinguish symbol/this reference roots from node
/// roots. Property suffixes deliberately keep the native unescaped dots:
/// `x["a.b"]` and `x.a.b` have the same key bytes.
#[derive(Clone, PartialEq, Eq, Hash)]
struct LoopKey {
    file: FileId,
    flow: FlowId,
    reference: FlowCacheKey,
}

enum Suffix {
    Name(crate::project::binding::NativePropertyKey),
    Symbol(NativeSymbolIdentity),
}

pub(in crate::project::semantic::evaluation) struct ActiveLoop {
    key: LoopKey,
    types: Vec<TypeId>,
}

#[derive(Default)]
pub(in crate::project::semantic::evaluation) struct LoopCache {
    complete: HashMap<LoopKey, NativeCompletion<TypeId>>,
    pub(in crate::project::semantic::evaluation) active: Vec<ActiveLoop>,
    suspended: Vec<FlowExpressionCache>,
}

pub(super) struct LoopWork {
    frame: usize,
    outer_frame: usize,
    antecedent: usize,
    key: LoopKey,
    index: usize,
    types: Vec<TypeId>,
    subtype: bool,
    first: Option<FlowType>,
    scoped: bool,
}

impl LoopWork {
    pub(super) fn frame(&self) -> usize {
        self.frame
    }
}

impl Evaluator<'_, '_> {
    /// `getFlowReferenceKey` / `writeFlowCacheKey`: an ineligible reference
    /// returns the native non-dotted sentinel, not an uncached graph walk.
    fn flow_reference_key_completion(
        &mut self,
        state: &ReadState<'_>,
    ) -> NativeResult<Option<FlowCacheKey>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut site = state.reference;
            let mut names = Vec::new();
            let mut key = loop {
                let syntax = self.reference_syntax(site)?;
                let origin = match site.kind {
                    SyntaxKind::Identifier if !(syntax.this_identifier && syntax.in_type_query) => {
                        let symbol =
                            sequence.read(self.reference_resolved_identifier_symbol_completion(
                                ExpressionRoot {
                                    file: site.file,
                                    site: site.captured()?,
                                },
                                state.location,
                            ))?;
                        if self.is_unknown_reference_symbol(symbol) {
                            return Ok(None);
                        }
                        Some(self.reference_symbol_identity(symbol, state.location)?)
                    }
                    SyntaxKind::Identifier | SyntaxKind::ThisKeyword => None,
                    SyntaxKind::NonNullExpression | SyntaxKind::ParenthesizedExpression => {
                        site = super::required(
                            syntax.expression,
                            "flow key wrapper has no expression",
                        )?;
                        continue;
                    }
                    SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression => {
                        if let Some(name) = sequence
                            .read(self.reference_property_name_completion(site, state.location))?
                        {
                            names.push(Suffix::Name(name));
                            site = super::required(
                                syntax.expression,
                                "flow key access has no receiver",
                            )?;
                            continue;
                        }
                        if site.kind == SyntaxKind::ElementAccessExpression {
                            let argument = super::required(
                                syntax.argument,
                                "flow key element has no argument",
                            )?;
                            if argument.kind == SyntaxKind::Identifier {
                                let symbol = sequence.read(
                                    self.reference_resolved_identifier_symbol_completion(
                                        ExpressionRoot {
                                            file: argument.file,
                                            site: argument.captured()?,
                                        },
                                        state.location,
                                    ),
                                )?;
                                if sequence.read(
                                    self.stable_element_key_completion(symbol, state.location),
                                )? {
                                    let identity =
                                        self.reference_symbol_identity(symbol, state.location)?;
                                    names.push(Suffix::Symbol(identity));
                                    site = super::required(
                                        syntax.expression,
                                        "flow key element has no receiver",
                                    )?;
                                    continue;
                                }
                            }
                        }
                        return Ok(None);
                    }
                    SyntaxKind::QualifiedName => {
                        names.push(Suffix::Name(
                            crate::project::binding::NativePropertyKey::fixed(
                                syntax
                                    .reference_name
                                    .ok_or_else(|| {
                                        super::super::super::internal(
                                            "qualified flow key has no name",
                                        )
                                    })?
                                    .as_bytes(),
                            ),
                        ));
                        site = super::required(syntax.left, "qualified flow key has no left")?;
                        continue;
                    }
                    SyntaxKind::ObjectBindingPattern
                    | SyntaxKind::ArrayBindingPattern
                    | SyntaxKind::FunctionDeclaration
                    | SyntaxKind::FunctionExpression
                    | SyntaxKind::ArrowFunction
                    | SyntaxKind::MethodDeclaration => {
                        let mut key = FlowCacheKeyBuilder::default();
                        key.write_node(site.file, site.captured()?);
                        key.write_bytes(b"#");
                        key.write_type(state.declared);
                        break key;
                    }
                    _ => return Ok(None),
                };
                let mut key = FlowCacheKeyBuilder::default();
                if let Some(origin) = origin {
                    key.write_symbol(&self.model, origin);
                }
                key.write_bytes(b":");
                key.write_type(state.declared);
                if state.initial != state.declared {
                    key.write_bytes(b"=");
                    key.write_type(state.initial);
                }
                if let Some(container) = state.flow_container {
                    let graph = self
                        .inputs
                        .bindings
                        .flow_graph(state.file)
                        .map_err(|error| self.read_binding_failure(state.location, error))?;
                    let container = graph.container(container);
                    key.write_bytes(b"@");
                    key.write_node(
                        state.file,
                        Site {
                            span: container.site,
                            kind: container.kind,
                        },
                    );
                }
                break key;
            };
            for suffix in names.into_iter().rev() {
                match suffix {
                    Suffix::Name(name) => {
                        key.write_bytes(b".");
                        key.write_name(&self.model, &name);
                    }
                    Suffix::Symbol(symbol) => {
                        key.write_bytes(b".@");
                        key.write_symbol(&self.model, symbol);
                    }
                }
            }
            Ok(Some(key.finish()))
        })();
        sequence.finish(result)
    }

    /// `newFlowType`: an incomplete Never uses the cached silent identity.
    pub(super) fn new_flow_type(&mut self, ty: TypeId, incomplete: bool) -> FlowType {
        let ty = if incomplete && self.model.has_type_flags(ty, TypeFlags::NEVER) {
            self.model.silent_never()
        } else {
            ty
        };
        FlowType { ty, incomplete }
    }

    pub(super) fn loop_scope_checkpoint(&self) -> (usize, usize) {
        (
            self.reads.loops.active.len(),
            self.reads.loops.suspended.len(),
        )
    }

    /// Restore suspended scopes on every read failure. A successful native
    /// invocation must already have balanced its scopes; cleanup cannot turn
    /// unfinished work into a completed loop-cache entry.
    pub(super) fn restore_loop_scopes(&mut self, checkpoint: (usize, usize)) -> bool {
        let balanced = self.loop_scope_checkpoint() == checkpoint;
        while self.reads.loops.suspended.len() > checkpoint.1 {
            let saved = self
                .reads
                .loops
                .suspended
                .pop()
                .expect("checked scope length");
            self.restore_flow_expression_cache(saved);
        }
        self.reads.loops.active.truncate(checkpoint.0);
        balanced
    }

    /// `getTypeAtFlowLoopLabel`: completed cache precedes active incomplete
    /// entries. Native insertion order chooses the first nonempty match; an
    /// empty match restarts at the actual non-looping first antecedent.
    pub(super) fn begin_flow_loop(
        &mut self,
        state: &ReadState<'_>,
        flow: FlowId,
        first: FlowId,
        frame: usize,
        invocations: &mut ReadInvocations,
        work: &mut ReadStack,
    ) -> Result<(), DemandError> {
        // FlowState.refKey is installed once for the whole reference question.
        // Only the first acquisition consumes its evidence; later loop labels
        // observe the stored key bytes rather than rerunning name evaluation.
        let reference = match &invocations.reference_key {
            Some(key) => key.clone(),
            None => {
                let key = invocations.read(frame, self.flow_reference_key_completion(state))?;
                invocations.reference_key = Some(key.clone());
                key
            }
        };
        let Some(reference) = reference else {
            invocations.finish(frame, FlowType::complete(state.declared))?;
            return Ok(());
        };
        let key = LoopKey {
            file: state.file,
            flow,
            reference,
        };
        if let Some(ty) = self.reads.loops.complete.get(&key).cloned() {
            invocations.finish_with(frame, Ok(ty.map(FlowType::complete)))?;
            return Ok(());
        }
        if let Some(types) = self
            .reads
            .loops
            .active
            .iter()
            .find(|entry| entry.key == key && !entry.types.is_empty())
            .map(|entry| entry.types.clone())
        {
            let ty = invocations.read(
                frame,
                self.flow_branch_union_completion(state, types, false),
            )?;
            let result = self.new_flow_type(ty, true);
            invocations.finish(frame, result)?;
            return Ok(());
        }
        let outer_frame = frame;
        let frame = invocations.loop_operation(outer_frame);
        let antecedent = invocations.child(frame);
        work.push(ReadWork::Loop(LoopWork {
            frame,
            outer_frame,
            antecedent,
            key,
            index: 0,
            types: Vec::new(),
            subtype: false,
            first: None,
            scoped: false,
        }));
        work.push(ReadWork::Visit {
            frame: antecedent,
            flow: first,
        });
        Ok(())
    }

    /// Resume one actual loop antecedent. Later paths hide the transient
    /// expression cache; restoration precedes the restarted completed-cache
    /// lookup. Only the first antecedent controls final incompleteness.
    pub(super) fn resume_flow_loop(
        &mut self,
        state: &ReadState<'_>,
        graph: &Graph,
        mut task: LoopWork,
        invocations: &mut ReadInvocations,
        work: &mut ReadStack,
    ) -> Result<(), DemandError> {
        let result = invocations.read_result(task.frame, task.antecedent)?;
        if task.scoped {
            let saved = self.reads.loops.suspended.pop().ok_or_else(|| {
                crate::project::semantic::internal("native loop has no suspended expression cache")
            })?;
            self.restore_flow_expression_cache(saved);
            let active = self.reads.loops.active.pop().ok_or_else(|| {
                crate::project::semantic::internal("native loop has no active scope")
            })?;
            if active.key != task.key {
                return Err(crate::project::semantic::internal(
                    "native loop scope has a different reference key",
                )
                .into());
            }
            if let Some(ty) = self.reads.loops.complete.get(&task.key).cloned() {
                invocations.finish_with(task.frame, Ok(ty.map(FlowType::complete)))?;
                let result = invocations.read_result(task.outer_frame, task.frame)?;
                invocations.finish(task.outer_frame, result)?;
                return Ok(());
            }
        } else {
            task.first = Some(result);
        }
        if !task.types.contains(&result.ty) {
            task.types.push(result.ty);
        }
        task.subtype |= !invocations.read(
            task.frame,
            self.type_subset_of_completion(result.ty, state.initial, state.location),
        )?;
        let Node::Label {
            looping: true,
            antecedents,
        } = graph.node(task.key.flow)
        else {
            return Err(crate::project::semantic::internal(
                "native loop task has a different graph role",
            )
            .into());
        };
        if result.ty != state.declared
            && let Some(&next) = antecedents.get(task.index + 1)
        {
            self.reads.loops.active.push(ActiveLoop {
                key: task.key.clone(),
                types: task.types.clone(),
            });
            let saved = self.suspend_flow_expression_cache();
            self.reads.loops.suspended.push(saved);
            task.index += 1;
            task.scoped = true;
            task.antecedent = invocations.child(task.frame);
            let antecedent = task.antecedent;
            work.push(ReadWork::Loop(task));
            work.push(ReadWork::Visit {
                frame: antecedent,
                flow: next,
            });
            return Ok(());
        }
        let ty = invocations.read(
            task.frame,
            self.flow_branch_union_completion(state, task.types, task.subtype),
        )?;
        let first = task.first.ok_or_else(|| {
            crate::project::semantic::internal("native loop has no first antecedent result")
        })?;
        let result = if first.incomplete {
            self.new_flow_type(ty, true)
        } else {
            FlowType::complete(ty)
        };
        invocations.finish(task.frame, result)?;
        if !first.incomplete {
            self.reads.loops.complete.insert(
                task.key,
                invocations.result(task.frame)?.map(|flow| flow.ty),
            );
        }
        let result = invocations.read_result(task.outer_frame, task.frame)?;
        invocations.finish(task.outer_frame, result)?;
        Ok(())
    }
}
