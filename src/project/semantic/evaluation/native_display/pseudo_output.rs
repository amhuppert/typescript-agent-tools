//! `pseudoTypeToNode` is output construction after equivalence, with its own
//! checker questions. It never substitutes the realized type for syntax.
use super::super::super::model::NativeSequence;
use super::super::super::source::pseudo_types::{
    PseudoMember, PseudoMemberKind, PseudoParameter, PseudoSignature, PseudoType,
};
use super::super::ReferenceDeclaration;
use super::super::contextual::{ContextFlags, ContextNode};
use super::*;
use crate::syntax_kind::SyntaxKind as K;

impl Evaluator<'_, '_> {
    pub(super) fn pseudo_type_node_with_checker_fallback(
        &mut self,
        pseudo: &PseudoType,
        checker_type: TypeId,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        if let PseudoType::Inferred {
            expression, errors, ..
        } = pseudo
        {
            if !context.suppress_report_inference_fallback {
                if errors.is_empty() {
                    self.diagnostic_report_inference_fallback(*expression, context);
                } else {
                    for &node in errors {
                        self.diagnostic_report_inference_fallback(node, context);
                    }
                }
            }
            let old = context.suppress_report_inference_fallback;
            context.suppress_report_inference_fallback = true;
            let result = self.diagnostic_type_node(checker_type, context, consumer);
            context.suppress_report_inference_fallback = old;
            return result;
        }
        if let PseudoType::Direct(node) = pseudo {
            return self
                .diagnostic_can_reuse_annotation(*node, checker_type, consumer)?
                .and_then(|can_reuse| {
                    if can_reuse {
                        return self.pseudo_type_node(pseudo, context, consumer);
                    }
                    if !context.suppress_report_inference_fallback {
                        self.diagnostic_report_inference_fallback(*node, context);
                    }
                    let saved = context.suppress_report_inference_fallback;
                    context.suppress_report_inference_fallback = true;
                    let result = self.diagnostic_type_node(checker_type, context, consumer);
                    context.suppress_report_inference_fallback = saved;
                    result
                });
        }
        self.pseudo_type_node(pseudo, context, consumer)
    }

    pub(super) fn pseudo_type_node(
        &mut self,
        pseudo: &PseudoType,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            Ok(match pseudo {
                PseudoType::Direct(node) => {
                    sequence.read(self.diagnostic_reuse_type_node(*node, context, consumer))?
                }
                PseudoType::Inferred {
                    expression,
                    signature_return,
                    errors,
                } => {
                    for &node in errors {
                        self.diagnostic_report_inference_fallback(node, context);
                    }
                    if errors.is_empty() {
                        self.diagnostic_report_inference_fallback(*expression, context);
                    }
                    if *signature_return {
                        let recipe = self.source.signature_from_declaration(*expression)?;
                        let signature = sequence.read(self.signature_completion(&recipe))?;
                        sequence.read(
                            self.diagnostic_signature_return(&signature, false, context, consumer),
                        )?
                    } else {
                        let parent = self.source.declaration_parent(*expression)?;
                        if let Some(parent) = parent {
                            let kind = self.source.declaration_kind(parent)?;
                            let function = if kind == K::ReturnStatement {
                                self.source.pseudo_containing_function(parent)?
                            } else if kind == K::ArrowFunction
                                && self.source.pseudo_child(parent, "body")? == Some(*expression)
                            {
                                Some(parent)
                            } else {
                                None
                            };
                            if let Some(function) = function {
                                if matches!(
                                    self.source.declaration_kind(function)?,
                                    K::GetAccessor | K::SetAccessor
                                ) {
                                    return sequence.read(self.diagnostic_declaration_type(
                                        Some(ReferenceDeclaration::Captured(function)),
                                        None,
                                        None,
                                        false,
                                        context,
                                        consumer,
                                    ));
                                }
                                let recipe = self.source.signature_from_declaration(function)?;
                                let signature =
                                    sequence.read(self.signature_completion(&recipe))?;
                                return sequence.read(self.diagnostic_signature_return(
                                    &signature, false, context, consumer,
                                ));
                            }
                            if self.source.pseudo_is_declaration(parent)? {
                                return sequence.read(self.diagnostic_declaration_type(
                                    Some(ReferenceDeclaration::Captured(parent)),
                                    None,
                                    None,
                                    false,
                                    context,
                                    consumer,
                                ));
                            }
                        }
                        let root = self.source.pseudo_node_expression(*expression)?;
                        let expression = self.source.expression_at(root.file, root.site)?;
                        let ty = sequence
                            .read(self.type_of_source_expression_completion(&expression))?;
                        sequence.read(self.diagnostic_type_node(ty, context, consumer))?
                    }
                }
                PseudoType::NoResult(node) => {
                    self.diagnostic_report_inference_fallback(*node, context);
                    let kind = self.source.declaration_kind(*node)?;
                    if crate::syntax::is_function_like_kind(kind)
                        && !matches!(kind, K::GetAccessor | K::SetAccessor)
                    {
                        let recipe = self.source.signature_from_declaration(*node)?;
                        let signature = sequence.read(self.signature_completion(&recipe))?;
                        sequence.read(
                            self.diagnostic_signature_return(&signature, false, context, consumer),
                        )?
                    } else {
                        sequence.read(self.diagnostic_declaration_type(
                            Some(ReferenceDeclaration::Captured(*node)),
                            None,
                            None,
                            false,
                            context,
                            consumer,
                        ))?
                    }
                }
                PseudoType::MaybeConst {
                    node,
                    constant,
                    regular,
                } => {
                    let root = self.source.pseudo_node_expression(*node)?;
                    let mut is_const =
                        sequence.read(self.is_const_context_completion(root, consumer))?;
                    if !is_const && self.source.pseudo_is_in_const_context(*node)? {
                        let contextual = sequence
                            .read(self.contextual_type_completion(root, ContextFlags::NONE))?;
                        let ty = sequence.read(self.pseudo_type_to_type(constant, consumer))?;
                        if let Some(ty) = ty
                            && let Some(contextual) = contextual
                        {
                            let contextual =
                                sequence.read(self.instantiate_contextual_type_completion(
                                    contextual,
                                    Some(ContextNode::Captured(root)),
                                    ContextFlags::NONE,
                                    consumer,
                                ))?;
                            if sequence.read(
                                self.literal_of_contextual_type_completion(
                                    ty, contextual, consumer,
                                ),
                            )? {
                                is_const = true;
                            }
                        }
                    }
                    sequence.read(self.pseudo_type_node(
                        if is_const { constant } else { regular },
                        context,
                        consumer,
                    ))?
                }
                PseudoType::Union(types) => {
                    let mut nodes = Vec::new();
                    let mut elided = false;
                    let mut undefined = false;
                    for ty in types {
                        if !self.inputs.semantic_options().strict_null_checks
                            && matches!(ty, PseudoType::Atom(K::UndefinedKeyword | K::NullKeyword))
                        {
                            elided = true;
                            continue;
                        }
                        let node = sequence.read(self.pseudo_type_node(ty, context, consumer))?;
                        append_pseudo_union(&mut nodes, node, &mut undefined);
                    }
                    match nodes.len() {
                        0 => Node::text(if elided { "any" } else { "never" }),
                        1 => nodes.remove(0),
                        _ => Node::Union(nodes),
                    }
                }
                PseudoType::Atom(kind) => Node::text(match kind {
                    K::UndefinedKeyword if !self.inputs.semantic_options().strict_null_checks => {
                        "any"
                    }
                    K::NullKeyword if !self.inputs.semantic_options().strict_null_checks => "any",
                    K::UndefinedKeyword => "undefined",
                    K::NullKeyword => "null",
                    K::AnyKeyword => "any",
                    K::StringKeyword => "string",
                    K::NumberKeyword => "number",
                    K::BigIntKeyword => "bigint",
                    K::BooleanKeyword => "boolean",
                    K::TrueKeyword => "true",
                    K::FalseKeyword => "false",
                    _ => {
                        return Err(super::super::super::internal(
                            "unknown native pseudo atom output",
                        )
                        .into());
                    }
                }),
                PseudoType::Literal { node, .. } => {
                    sequence.read(self.diagnostic_reuse_literal(*node, context, consumer))?
                }
                PseudoType::Tuple(elements) => {
                    let mut output = Vec::new();
                    for element in elements {
                        output.push(Element {
                            name: None,
                            optional: false,
                            rest: false,
                            ty: sequence.read(self.pseudo_type_node(element, context, consumer))?,
                        });
                    }
                    Node::Prefix("readonly ", Box::new(Node::Tuple(output)))
                }
                PseudoType::Function(signature) => Node::Function(
                    Box::new(
                        sequence.read(self.pseudo_signature_node(signature, context, consumer))?,
                    ),
                    false,
                ),
                PseudoType::Object(elements) => {
                    if elements.is_empty() {
                        return Ok(Node::Object(Vec::new()));
                    }
                    let member = self
                        .source
                        .declaration_parent(elements[0].name)?
                        .ok_or_else(|| super::super::super::internal("pseudo name lacks member"))?;
                    let object = self.source.declaration_parent(member)?.ok_or_else(|| {
                        super::super::super::internal("pseudo member lacks object")
                    })?;
                    let root = self.source.pseudo_node_expression(object)?;
                    let constant =
                        sequence.read(self.is_const_context_completion(root, consumer))?;
                    let saved = context.flags;
                    context.flags.0 |= 1 << 22;
                    let result: Result<Node, DemandError> = (|| {
                        let mut output = Vec::new();
                        for element in elements {
                            output.push(sequence.read(
                                self.pseudo_member_node(element, constant, context, consumer),
                            )?);
                        }
                        Ok(Node::Object(output))
                    })();
                    context.flags = saved;
                    result?
                }
            })
        })();
        sequence.finish(result)
    }

    fn pseudo_signature_node(
        &mut self,
        pseudo: &PseudoSignature,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<syntax::Signature> {
        let recipe = self.source.signature_from_declaration(pseudo.declaration)?;
        self.signature_completion(&recipe)?.and_then(|signature| {
            self.expanded_parameters_completion(&signature, true, consumer)?
                .and_then(|expanded| {
                    let expanded = expanded.first().ok_or_else(|| {
                        super::super::super::internal("native expanded parameter list is empty")
                    })?;
                    self.diagnostic_scope(
                        context,
                        Some(pseudo.declaration),
                        expanded,
                        signature.type_parameter_types(),
                        Some(&signature.parameters),
                        self.signature_mapper(&signature),
                        consumer,
                        |checker, context| {
                            checker.pseudo_signature_contents(pseudo, context, consumer)
                        },
                    )
                })
        })
    }
    fn pseudo_signature_contents(
        &mut self,
        pseudo: &PseudoSignature,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<syntax::Signature> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut type_parameters = Vec::new();
            for &parameter in &pseudo.type_parameters {
                type_parameters
                    .push(sequence.read(
                        self.diagnostic_reuse_type_parameter(parameter, context, consumer),
                    )?);
            }
            let mut parameters = Vec::new();
            for parameter in &pseudo.parameters {
                parameters
                    .push(sequence.read(self.pseudo_parameter_node(parameter, context, consumer))?);
            }
            let returns =
                sequence.read(self.pseudo_type_node(&pseudo.returns, context, consumer))?;
            Ok(syntax::Signature {
                type_parameters,
                parameters,
                returns,
                abstract_: false,
            })
        })();
        sequence.finish(result)
    }
    fn pseudo_parameter_node(
        &mut self,
        parameter: &PseudoParameter,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Element> {
        self.diagnostic_parameter_name(parameter.name, context, consumer)?
            .and_then(|name| {
                Ok(self
                    .pseudo_type_node(&parameter.ty, context, consumer)?
                    .map(|ty| Element {
                        name: Some(name),
                        optional: parameter.optional,
                        rest: parameter.rest,
                        ty,
                    }))
            })
    }
    fn pseudo_member_node(
        &mut self,
        element: &PseudoMember,
        constant: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Member> {
        if !matches!(element.kind, PseudoMemberKind::Property { .. }) {
            let declaration = match &element.kind {
                PseudoMemberKind::Method(signature) => signature.declaration,
                PseudoMemberKind::Get { declaration, .. }
                | PseudoMemberKind::Set { declaration, .. } => *declaration,
                _ => unreachable!(),
            };
            let recipe = self.source.signature_from_declaration(declaration)?;
            return self.signature_completion(&recipe)?.and_then(|signature| {
                self.expanded_parameters_completion(&signature, true, consumer)?
                    .and_then(|expanded| {
                        let expanded = expanded.first().ok_or_else(|| {
                            super::super::super::internal("native expanded parameter list is empty")
                        })?;
                        self.diagnostic_scope(
                            context,
                            Some(declaration),
                            expanded,
                            signature.type_parameter_types(),
                            Some(&signature.parameters),
                            self.signature_mapper(&signature),
                            consumer,
                            |checker, context| {
                                checker.pseudo_member_contents(element, constant, context, consumer)
                            },
                        )
                    })
            });
        }
        self.pseudo_member_contents(element, constant, context, consumer)
    }
    fn pseudo_member_contents(
        &mut self,
        element: &PseudoMember,
        constant: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Member> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            // Method type parameters precede its name and parameters.
            let mut type_parameters = Vec::new();
            if let PseudoMemberKind::Method(signature) = &element.kind {
                for &parameter in &signature.type_parameters {
                    type_parameters.push(sequence.read(
                        self.diagnostic_reuse_type_parameter(parameter, context, consumer),
                    )?);
                }
            }
            let method = matches!(element.kind, PseudoMemberKind::Method(_)) && !constant;
            let name = sequence.read(self.diagnostic_reuse_name(
                element.name,
                method,
                context,
                consumer,
            ))?;
            Ok(match &element.kind {
                PseudoMemberKind::Property { readonly, ty } => Member::Property {
                    name,
                    optional: false,
                    readonly: constant || *readonly,
                    ty: sequence.read(self.pseudo_type_node(ty, context, consumer))?,
                },
                PseudoMemberKind::Method(signature) => {
                    let mut parameters = Vec::new();
                    for parameter in &signature.parameters {
                        parameters.push(
                            sequence
                                .read(self.pseudo_parameter_node(parameter, context, consumer))?,
                        );
                    }
                    let returns = sequence.read(self.pseudo_type_node(
                        &signature.returns,
                        context,
                        consumer,
                    ))?;
                    let signature = syntax::Signature {
                        type_parameters,
                        parameters,
                        returns,
                        abstract_: false,
                    };
                    if constant {
                        Member::Property {
                            name,
                            optional: false,
                            readonly: true,
                            ty: Node::Function(Box::new(signature), false),
                        }
                    } else {
                        Member::Signature {
                            kind: SignatureKind::Method,
                            name,
                            optional: false,
                            signature,
                        }
                    }
                }
                PseudoMemberKind::Get { ty, .. } => Member::Signature {
                    kind: SignatureKind::Get,
                    name,
                    optional: false,
                    signature: syntax::Signature {
                        type_parameters: Vec::new(),
                        parameters: Vec::new(),
                        returns: sequence.read(self.pseudo_type_node(ty, context, consumer))?,
                        abstract_: false,
                    },
                },
                PseudoMemberKind::Set { parameter, .. } => Member::Signature {
                    kind: SignatureKind::Set,
                    name,
                    optional: false,
                    signature: syntax::Signature {
                        type_parameters: Vec::new(),
                        parameters: vec![
                            sequence
                                .read(self.pseudo_parameter_node(parameter, context, consumer))?,
                        ],
                        returns: Node::text("void"),
                        abstract_: false,
                    },
                },
            })
        })();
        sequence.finish(result)
    }
}
fn append_pseudo_union(output: &mut Vec<Node>, node: Node, undefined: &mut bool) {
    if let Node::Union(children) = node {
        for child in children {
            append_pseudo_union(output, child, undefined);
        }
    } else if matches!(&node, Node::Text(text) if text == "undefined") {
        if !*undefined {
            *undefined = true;
            output.push(node);
        }
    } else {
        output.push(node);
    }
}
