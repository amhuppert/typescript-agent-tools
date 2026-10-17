//! `tryReuseExistingNodeHelper` owns native recovery boundaries. Node-copy
//! failure is a native nil result; checker/resource interruption stays Err.
use super::super::super::model::NativeSequence;
use super::*;
use crate::syntax_kind::SyntaxKind as K;

#[derive(Default)]
pub(super) struct RecoveryBoundary {
    had_error: bool,
    deferred_reports: usize,
    pub(super) tracks: Vec<cache::TrackedSymbol>,
}
impl Context {
    /// Every deferred report in current printer entries reaches the same nil
    /// outer tracker. Payloads affect neither diagnostics nor acquisitions.
    pub(super) fn report_diagnostic(&mut self) {
        self.reported_diagnostic = true;
        if let Some(boundary) = self.recovery_boundaries.last_mut() {
            boundary.had_error = true;
            boundary.deferred_reports += 1;
        }
    }
}

impl Evaluator<'_, '_> {
    /// createAnonymousTypeNodeEx's instantiation-expression branch reuses
    /// only a TypeQuery whose mapped type is this exact identity. Constituents
    /// of composites keep the structural fallback. The visited entry bounds
    /// reentry through an inaccessible-name reuse fallback.
    pub(super) fn diagnostic_instantiation_expression_node(
        &mut self,
        ty: TypeId,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Option<Node>> {
        let declaration = self.instantiation_expression_declaration(ty)?;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.source.declaration_kind(declaration)? == K::TypeQuery
                && sequence.read(self.diagnostic_type_from_node(
                    declaration,
                    context,
                    false,
                    consumer,
                ))? == Some(ty)
            {
                if context.visited.contains(&ty) {
                    return Ok(Some(context.elision(None)));
                }
                context.visited.insert(ty);
                let reused = self.diagnostic_try_reuse(declaration, context, consumer);
                context.visited.remove(&ty);
                if let Some(node) = sequence.read(reused)? {
                    return Ok(Some(node));
                }
            }
            if context.visited.contains(&ty) {
                return Ok(Some(context.elision(None)));
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    pub(super) fn diagnostic_reuse_type_node(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        self.diagnostic_try_reuse(node, context, consumer)?
            .and_then(|output| match output {
                Some(output) => Ok(NativeCompletion::clean(output)),
                None => {
                    self.diagnostic_report_inference_fallback(node, context);
                    self.diagnostic_type_from_node(node, context, false, consumer)?
                        .and_then(|ty| {
                            let ty = ty.ok_or_else(|| {
                                super::super::super::internal(
                                    "mapped type acquisition unexpectedly nil",
                                )
                            })?;
                            self.diagnostic_type_node(ty, context, consumer)
                        })
                }
            })
    }

    fn diagnostic_try_reuse(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Option<Node>> {
        self.diagnostic_reuse_boundary(node, context, consumer, |checker, context| {
            checker.diagnostic_reuse_visit(node, context, consumer)
        })
    }
    fn diagnostic_reuse_boundary<T>(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
        worker: impl FnOnce(&mut Self, &mut Context) -> NativeResult<T>,
    ) -> NativeResult<Option<T>> {
        let old_tracks = std::mem::take(&mut context.tracked_symbols);
        let old_error = context.encountered_error;
        let old_length = context.length;
        context
            .recovery_boundaries
            .push(RecoveryBoundary::default());
        let result = worker(self, context);
        let boundary = context
            .recovery_boundaries
            .pop()
            .expect("native reuse boundary");
        context.tracked_symbols = old_tracks;
        context.encountered_error = old_error;
        context.length = old_length;
        // An interrupted checker operation never reaches native finalize's
        // reports/replay, but its local scope is still unwound.
        result?.and_then(|result| {
            for _ in 0..boundary.deferred_reports {
                context.report_diagnostic();
            }
            if boundary.had_error {
                return Ok(NativeCompletion::clean(None));
            }
            for tracked in boundary.tracks {
                self.diagnostic_track_symbol(
                    tracked.symbol,
                    tracked.enclosing,
                    tracked.meaning,
                    context,
                    consumer,
                )?;
            }
            context.length += self.source.pseudo_node_length(node)?;
            Ok(NativeCompletion::clean(Some(result)))
        })
    }
    /// `getTypeFromTypeNode`: mapper application is a separate acquired child.
    pub(super) fn diagnostic_type_from_node(
        &mut self,
        node: DeclarationIdentity,
        context: &Context,
        no_mapped: bool,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let annotation = self.source.pseudo_annotation(node)?;
        self.check_type_completion(&annotation)?.and_then(|ty| {
            if let Some(mapper) = context.mapper {
                Ok(self
                    .instantiate_type_completion(ty, mapper, consumer)?
                    .map(|mapped| {
                        if no_mapped && mapped != ty {
                            None
                        } else {
                            Some(mapped)
                        }
                    }))
            } else {
                Ok(NativeCompletion::clean(Some(ty)))
            }
        })
    }
    fn diagnostic_reuse_visit(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        let boundary = context
            .recovery_boundaries
            .last()
            .ok_or_else(|| super::super::super::internal("reuse visitor lacks boundary"))?;
        if boundary.had_error {
            return Ok(NativeCompletion::clean(Node::text(
                self.source.pseudo_source_text(node)?,
            )));
        }
        let checkpoint = (
            context.tracked_symbols.len(),
            boundary.deferred_reports,
            boundary.had_error,
        );
        let result = self.diagnostic_reuse_worker(node, context, consumer);
        result?.and_then(|output| {
            let boundary = context
                .recovery_boundaries
                .last_mut()
                .expect("native reuse visitor boundary");
            if boundary.had_error
                && self.source.pseudo_is_type_node(node)?
                && self.source.declaration_kind(node)? != K::TypePredicate
            {
                boundary.had_error = checkpoint.2;
                boundary.deferred_reports = checkpoint.1;
                context.tracked_symbols.truncate(checkpoint.0);
                // Boundary tracks deliberately do not roll back with the
                // context list: native wrappingTracker retains both channels.
                return self
                    .diagnostic_type_from_node(node, context, false, consumer)?
                    .and_then(|ty| {
                        let ty = ty.ok_or_else(|| {
                            super::super::super::internal(
                                "mapped type acquisition unexpectedly nil",
                            )
                        })?;
                        self.diagnostic_type_node(ty, context, consumer)
                    });
            }
            Ok(NativeCompletion::clean(output))
        })
    }
    fn diagnostic_reuse_worker(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let kind = self.source.declaration_kind(node)?;
            let child = |checker: &mut Self, field| -> Result<DeclarationIdentity, DemandError> {
                checker.source.pseudo_child(node, field)?.ok_or_else(|| {
                    super::super::super::internal("native copied syntax lacks required child")
                        .into()
                })
            };
            Ok(match kind {
                K::AnyKeyword => Node::text("any"), K::UnknownKeyword => Node::text("unknown"), K::NeverKeyword => Node::text("never"),
                K::VoidKeyword => Node::text("void"), K::UndefinedKeyword => Node::text("undefined"), K::NullKeyword => Node::text("null"),
                K::NumberKeyword => Node::text("number"), K::StringKeyword => Node::text("string"), K::BooleanKeyword => Node::text("boolean"), K::BigIntKeyword => Node::text("bigint"), K::SymbolKeyword => Node::text("symbol"), K::ObjectKeyword => Node::text("object"),
                K::ThisType | K::ThisKeyword => Node::text("this"), K::TrueKeyword => Node::text("true"), K::FalseKeyword => Node::text("false"),
                K::Identifier | K::PrivateIdentifier => Node::text(self.source.pseudo_exact_name(node)?),
                K::StringLiteral => Node::text(crate::printer::quoted_text(&self.source.pseudo_exact_name(node)?, if context.flags.has(1 << 28) || self.source.pseudo_single_quote(node)? { '\'' } else { '"' })),
                K::NoSubstitutionTemplateLiteral | K::PrefixUnaryExpression => Node::text(self.source.pseudo_source_text(node)?),
                K::NumericLiteral | K::BigIntLiteral => Node::text(self.source.pseudo_exact_name(node)?),
                K::LiteralType => { let child = child(self, "literal")?; sequence.read(self.diagnostic_reuse_visit(child, context, consumer))? }
                K::ParenthesizedType => { let child = child(self, "type")?; Node::Parenthesized(Box::new(sequence.read(self.diagnostic_reuse_visit(child, context, consumer))?)) }
                K::ArrayType => { let child = child(self, "elementType")?; Node::Array(Box::new(sequence.read(self.diagnostic_reuse_visit(child, context, consumer))?)) }
                K::UnionType | K::IntersectionType => {
                    let mut output = Vec::new();
                    for child in self.source.pseudo_children(node, "types")? { output.push(sequence.read(self.diagnostic_reuse_visit(child, context, consumer))?); }
                    if kind == K::UnionType { Node::Union(output) } else { Node::Intersection(output) }
                }
                K::TupleType => {
                    let mut output = Vec::new();
                    for child in self.source.pseudo_children(node, "elements")? {
                        let kind = self.source.declaration_kind(child)?;
                        let named = kind == K::NamedTupleMember;
                        let (name, ty) = if named { (self.source.pseudo_child(child, "name")?.map(|name| self.source.pseudo_exact_name(name)).transpose()?, self.source.pseudo_child(child, "type")?.ok_or_else(|| super::super::super::internal("named tuple member has no type"))?) }
                            else if matches!(kind, K::RestType | K::OptionalType) { (None, self.source.pseudo_child(child, "type")?.ok_or_else(|| super::super::super::internal("tuple wrapper has no type"))?) } else { (None, child) };
                        output.push(Element { name, optional: kind == K::OptionalType || named && self.source.pseudo_child(child, "questionToken")?.is_some(), rest: kind == K::RestType || named && self.source.pseudo_child(child, "dotDotDotToken")?.is_some(), ty: sequence.read(self.diagnostic_reuse_visit(ty, context, consumer))? });
                    }
                    Node::Tuple(output)
                }
                K::TypeOperator => {
                    let operand = child(self, "type")?;
                    let operator = self.source.pseudo_operator(node)?;
                    if operator == K::UniqueKeyword as u32 && self.source.declaration_kind(operand)? == K::SymbolKeyword {
                        let enclosing = self.diagnostic_captured_enclosing(context.enclosing)?;
                        let mut parent = self.source.declaration_parent(node)?;
                        while parent.is_some() && parent != enclosing { parent = self.source.declaration_parent(parent.expect("present parent"))?; }
                        if parent.is_none() { context.recovery_boundaries.last_mut().expect("reuse boundary").had_error = true; return Ok(Node::text("unique symbol")); }
                    }
                    if operator == K::KeyOfKeyword as u32 {
                        return Ok(match sequence.read(self.diagnostic_try_reuse_simple(node, context, consumer))? {
                            Some(output) => output,
                            None => { context.recovery_boundaries.last_mut().expect("reuse boundary").had_error = true; Node::text(self.source.pseudo_source_text(node)?) }
                        });
                    }
                    let prefix = if operator == K::ReadonlyKeyword as u32 { "readonly " } else if operator == K::UniqueKeyword as u32 { "unique " } else { return Err(super::super::super::internal("unknown native type operator").into()); };
                    Node::Prefix(prefix, Box::new(sequence.read(self.diagnostic_reuse_visit(operand, context, consumer))?))
                }
                K::IndexedAccessType | K::TypeReference | K::TypeQuery => {
                    match sequence.read(self.diagnostic_try_reuse_simple(node, context, consumer))? {
                        Some(output) => output,
                        None => { context.recovery_boundaries.last_mut().expect("reuse boundary").had_error = true; Node::text(self.source.pseudo_source_text(node)?) }
                    }
                }
                K::ConditionalType => {
                    let check = child(self, "checkType")?;
                    let check = sequence.read(self.diagnostic_reuse_visit(check, context, consumer))?;
                    let root = self.source.pseudo_node_expression(node)?;
                    let parameters = self.infer_type_parameters(root, consumer)?;
                    let (extends, yes) = sequence.read(self.diagnostic_scope(context, Some(node), &[], &parameters, None, None, consumer, |checker, context| {
                        let extends = child(checker, "extendsType")?;
                        checker.diagnostic_reuse_visit(extends, context, consumer)?.and_then(|extends| {
                            let yes = child(checker, "trueType")?;
                            Ok(checker.diagnostic_reuse_visit(yes, context, consumer)?.map(|yes| (extends, yes)))
                        })
                    }))?;
                    let no = child(self, "falseType")?;
                    let no = sequence.read(self.diagnostic_reuse_visit(no, context, consumer))?;
                    Node::Conditional(Box::new([check, extends, yes, no]))
                }
                K::MappedType => sequence.read(self.diagnostic_reuse_mapped(node, context, consumer))?,
                K::TemplateLiteralType => {
                    // Existing template tokens retain native RawText. Semantic
                    // template nodes use the distinct cooked-text printer arm.
                    let head = child(self, "head")?;
                    let mut tokens = vec![self.source.pseudo_source_text(head)?];
                    let mut types = Vec::new();
                    for span in self.source.pseudo_children(node, "templateSpans")? {
                        let ty = self.source.pseudo_child(span, "type")?.ok_or_else(|| super::super::super::internal("template span lacks type"))?;
                        types.push(sequence.read(self.diagnostic_reuse_visit(ty, context, consumer))?);
                        let literal = self.source.pseudo_child(span, "literal")?.ok_or_else(|| super::super::super::internal("template span lacks literal"))?;
                        tokens.push(self.source.pseudo_source_text(literal)?);
                    }
                    Node::ReusedTemplate(tokens, types)
                }
                K::TypeLiteral => sequence.read(self.diagnostic_reuse_type_literal(node, context, consumer))?,
                K::FunctionType | K::ConstructorType => Node::Function(Box::new(sequence.read(self.diagnostic_reuse_signature(node, context, consumer))?), kind == K::ConstructorType),
                K::InferType => { let parameter = child(self, "typeParameter")?; Node::Infer(sequence.read(self.diagnostic_reuse_type_parameter_contents(parameter, context, consumer))?) },
                K::TypePredicate => {
                    let facts = self.source.pseudo_predicate(node)?.ok_or_else(|| super::super::super::internal("predicate recipe missing"))?;
                    let name = if facts.this_ { "this".to_owned() } else {
                        let name = child(self, "parameterName")?;
                        sequence.read(self.diagnostic_reuse_entity(name, context, consumer))?
                    };
                    let ty = match facts.ty { Some(ty) => Some(Box::new(sequence.read(self.diagnostic_reuse_visit(ty, context, consumer))?)), None => None };
                    Node::Predicate { asserts: facts.asserts, name, ty }
                }
                _ => return Err(self.diagnostic_dependency(consumer, "native existing-node visitor requires this syntax family's scope and output factory")),
            })
        })();
        sequence.finish(result)
    }
    pub(super) fn diagnostic_reuse_literal(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        self.diagnostic_try_reuse(node, context, consumer)?
            .and_then_value(|node| {
                node.ok_or_else(|| {
                    super::super::super::internal("primitive literal reuse has a native nil result")
                        .into()
                })
            })
    }
    pub(super) fn diagnostic_reuse_name(
        &mut self,
        node: DeclarationIdentity,
        method: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<String> {
        self.diagnostic_try_reuse(node, context, consumer)?
            .and_then_value(|output| {
                let output = output.ok_or_else(|| {
                    self.diagnostic_dependency(
                        consumer,
                        "native name reuse returned nil requiring containing declaration recovery",
                    )
                })?;
                let kind = self.source.declaration_kind(node)?;
                if matches!(kind, K::Identifier | K::StringLiteral | K::NumericLiteral) {
                    let text = self.source.pseudo_exact_name(node)?;
                    let classified =
                        diagnostic_property_name_kind(&text, kind == K::StringLiteral, method);
                    if !(kind == K::Identifier && classified == K::Identifier
                        || kind == K::StringLiteral && classified == K::StringLiteral)
                    {
                        // reuseName creates changed StringLiteral names with
                        // TokenFlagsNone, independently of the context quote bit.
                        if classified == K::Identifier {
                            return Ok(text);
                        }
                        if classified == K::StringLiteral {
                            return Ok(crate::printer::quoted_text(&text, '"'));
                        }
                    }
                }
                String::from_utf8(output.print()).map_err(|_| {
                    super::super::super::internal(
                        "copied syntax printer emitted non-UTF8 before truncation",
                    )
                    .into()
                })
            })
    }
    pub(super) fn diagnostic_reuse_type_parameter(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<TypeParameter> {
        self.diagnostic_reuse_boundary(node, context, consumer, |checker, context| {
            checker.diagnostic_reuse_type_parameter_contents(node, context, consumer)
        })?
        .and_then_value(|node| {
            node.ok_or_else(|| {
                self.diagnostic_dependency(
                    consumer,
                    "native copied type parameter returned nil requiring enclosing syntax fallback",
                )
            })
        })
    }
    fn diagnostic_reuse_type_parameter_contents(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<TypeParameter> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let name = self
                .source
                .pseudo_child(node, "name")?
                .ok_or_else(|| super::super::super::internal("type parameter lacks name"))?;
            let (name, _) =
                sequence.read(self.diagnostic_existing_entity(name, context, consumer))?;
            let constraint = match self.source.pseudo_child(node, "constraint")? {
                Some(node) => Some(Box::new(
                    sequence.read(self.diagnostic_reuse_visit(node, context, consumer))?,
                )),
                None => None,
            };
            let default = match self.source.pseudo_child(node, "default")? {
                Some(node) => Some(Box::new(
                    sequence.read(self.diagnostic_reuse_visit(node, context, consumer))?,
                )),
                None => None,
            };
            let mut modifiers = String::new();
            for modifier in self.source.pseudo_children(node, "modifiers")? {
                modifiers.push_str(&self.source.pseudo_source_text(modifier)?);
                modifiers.push(' ');
            }
            Ok(TypeParameter {
                name,
                modifiers,
                constraint,
                default,
            })
        })();
        sequence.finish(result)
    }
}

impl Evaluator<'_, '_> {
    /// Ordinary TS part of `canReuseExistingJSTypeNode`: the compatible
    /// reference-arity branch also applies to TS, despite the native name.
    pub(super) fn diagnostic_can_reuse_annotation(
        &mut self,
        node: DeclarationIdentity,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(target) = self.model.reference_target(ty) else {
                return Ok(true);
            };
            if self.source.declaration_kind(node)? != K::TypeReference {
                return Ok(true);
            }
            let annotation = self.source.pseudo_annotation(node)?;
            sequence.read(self.check_type_completion(&annotation))?;
            let root = self.source.pseudo_node_expression(node)?;
            let Some(symbol) = self.resolved_type_reference_symbol(root) else {
                return Ok(true);
            };
            let existing =
                sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
            if existing != target {
                return Ok(true);
            }
            let parameters = self.model.declared_target(target)?.all_parameters;
            Ok(self.source.pseudo_children(node, "typeArguments")?.len()
                >= self.minimum_type_argument_count(&parameters)?)
        })();
        sequence.finish(result)
    }
    /// `tryVisitSimpleTypeNode` keeps a failed reference/index/keyof attempt
    /// as nil until its containing type is recovered. Other syntax uses the
    /// ordinary visitor, retaining that child's own recovery boundary.
    fn diagnostic_try_reuse_simple(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Option<Node>> {
        let inner = self.source.pseudo_skip_parentheses(node)?;
        let kind = self.source.declaration_kind(inner)?;
        if matches!(kind, K::TypeReference | K::TypeQuery) {
            return self.diagnostic_reuse_reference(inner, kind == K::TypeQuery, context, consumer);
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if kind == K::IndexedAccessType {
                let object = self
                    .source
                    .pseudo_child(inner, "objectType")?
                    .ok_or_else(|| {
                        super::super::super::internal("indexed access lacks object type")
                    })?;
                let Some(object) =
                    sequence.read(self.diagnostic_try_reuse_simple(object, context, consumer))?
                else {
                    return Ok(None);
                };
                let index = self
                    .source
                    .pseudo_child(inner, "indexType")?
                    .ok_or_else(|| {
                        super::super::super::internal("indexed access lacks index type")
                    })?;
                let index = sequence.read(self.diagnostic_reuse_visit(index, context, consumer))?;
                return Ok(Some(Node::Indexed(Box::new(object), Box::new(index))));
            }
            if kind == K::TypeOperator
                && self.source.pseudo_operator(inner)? == K::KeyOfKeyword as u32
            {
                let operand = self
                    .source
                    .pseudo_child(inner, "type")?
                    .ok_or_else(|| super::super::super::internal("keyof lacks operand"))?;
                return Ok(sequence
                    .read(self.diagnostic_try_reuse_simple(operand, context, consumer))?
                    .map(|node| Node::Prefix("keyof ", Box::new(node))));
            }
            Ok(Some(sequence.read(
                self.diagnostic_reuse_visit(node, context, consumer),
            )?))
        })();
        sequence.finish(result)
    }
    fn diagnostic_reuse_reference(
        &mut self,
        node: DeclarationIdentity,
        typeof_: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Option<Node>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let field = if typeof_ { "exprName" } else { "typeName" };
            let name = self
                .source
                .pseudo_child(node, field)?
                .ok_or_else(|| super::super::super::internal("reference syntax has no name"))?;
            if !typeof_ {
                if self.source.declaration_kind(name)? == K::Identifier
                    && self.source.pseudo_exact_name(name)? == "const"
                {
                    return Ok(None);
                }
                let annotation = self.source.pseudo_annotation(node)?;
                sequence.read(self.check_type_completion(&annotation))?;
                let root = self.source.pseudo_node_expression(node)?;
                let Some(symbol) = self.resolved_type_reference_symbol(root) else {
                    return Ok(None);
                };
                if self
                    .reference_symbol_flags(symbol, consumer)?
                    .type_parameter
                {
                    let ty =
                        sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
                    if let Some(mapper) = context.mapper
                        && sequence.read(self.map_parameter_completion(ty, mapper, consumer))? != ty
                    {
                        return Ok(None);
                    }
                }
                let ty = sequence
                    .read(self.diagnostic_type_from_node(node, context, false, consumer))?
                    .ok_or_else(|| super::super::super::internal("native mapped annotation nil"))?;
                if !sequence.read(self.diagnostic_can_reuse_annotation(node, ty, consumer))? {
                    return Ok(None);
                }
            }
            let (name, introduces_error) =
                sequence.read(self.diagnostic_existing_entity(name, context, consumer))?;
            let mut arguments = Vec::new();
            for argument in self.source.pseudo_children(node, "typeArguments")? {
                arguments
                    .push(sequence.read(self.diagnostic_reuse_visit(argument, context, consumer))?);
            }
            if introduces_error {
                return Err(self.diagnostic_dependency(consumer, "native serializeTypeName requires full entity-name resolution at the original node"));
            }
            let node = Node::Reference(name, arguments);
            Ok(Some(if typeof_ {
                Node::Prefix("typeof ", Box::new(node))
            } else {
                node
            }))
        })();
        sequence.finish(result)
    }
    fn diagnostic_existing_entity(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<(String, bool)> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let (first, meaning, suffix) = self.source.pseudo_entity_name(node)?;
            if self.source.pseudo_is_this_name(first)? {
                return Err(self.diagnostic_dependency(
                    consumer,
                    "native this entity reuse requires getThisContainer symbol and visibility",
                ));
            }
            let mut symbol = sequence
                .read(self.diagnostic_resolve_entity_name(first, meaning, None, consumer))?;
            if context.enclosing.is_some()
                && !symbol
                    .map(|symbol| self.reference_symbol_flags(symbol, consumer))
                    .transpose()?
                    .is_some_and(|flags| flags.type_parameter)
            {
                symbol = symbol
                    .map(|symbol| {
                        self.reference_export_symbol_of_value_if_exported(symbol, consumer)
                    })
                    .transpose()?;
                let at = sequence.read(self.diagnostic_resolve_entity_name(
                    first,
                    meaning,
                    context.enclosing,
                    consumer,
                ))?;
                let invalid = at.is_some_and(|symbol| self.is_unknown_reference_symbol(symbol))
                    || at.is_none() && symbol.is_some()
                    || match (at, symbol) {
                        (Some(at), Some(original)) => {
                            let exported =
                                self.reference_export_symbol_of_value_if_exported(at, consumer)?;
                            sequence
                                .read(self.get_symbol_if_same_reference_completion(
                                    exported, original, consumer,
                                ))?
                                .is_none()
                        }
                        _ => false,
                    };
                if invalid {
                    if !at.is_some_and(|symbol| self.is_unknown_reference_symbol(symbol)) {
                        self.diagnostic_report_inference_fallback(node, context);
                    }
                    return Ok((
                        format!("{}{}", self.source.pseudo_exact_name(first)?, suffix),
                        true,
                    ));
                }
                symbol = at;
            }
            let mut name = self.source.pseudo_exact_name(first)?;
            let mut introduces_error = false;
            if let Some(symbol) = symbol {
                let flags = self.reference_symbol_flags(symbol, consumer)?;
                let parameter = if flags.function_scoped_variable {
                    match self.reference_symbol_value_declaration(symbol, consumer)? {
                        Some(super::super::ReferenceDeclaration::Captured(declaration)) => {
                            self.source.pseudo_part_of_parameter(declaration)?
                        }
                        _ => false,
                    }
                } else {
                    false
                };
                if !parameter {
                    if !flags.type_parameter
                        && !self.source.pseudo_declaration_name(node)?
                        && !sequence.read(self.is_symbol_accessible_completion(
                            symbol,
                            context.enclosing,
                            meaning,
                            consumer,
                        ))?
                    {
                        introduces_error = true;
                        self.diagnostic_report_inference_fallback(node, context);
                    } else {
                        self.diagnostic_track_symbol(
                            symbol,
                            context.enclosing,
                            meaning,
                            context,
                            consumer,
                        )?;
                    }
                }
                let ty =
                    sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
                if flags.type_parameter {
                    name = sequence.read(
                        self.diagnostic_type_parameter_name_completion(ty, context, consumer),
                    )?;
                }
            }
            Ok((format!("{name}{suffix}"), introduces_error))
        })();
        sequence.finish(result)
    }
    fn diagnostic_reuse_entity(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<String> {
        self.diagnostic_existing_entity(node, context, consumer)?
            .and_then_value(|(name, error)| {
                if error && let Some(boundary) = context.recovery_boundaries.last_mut() {
                    boundary.had_error = true;
                }
                Ok(name)
            })
    }
}

impl Evaluator<'_, '_> {
    fn diagnostic_reuse_signature(
        &mut self,
        declaration: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<syntax::Signature> {
        let recipe = self.source.signature_from_declaration(declaration)?;
        self.signature_completion(&recipe)?.and_then(|signature| {
            self.diagnostic_scope(
                context,
                Some(declaration),
                &signature.parameters,
                signature.type_parameter_types(),
                None,
                None,
                consumer,
                |checker, context| {
                    let mut sequence = NativeSequence::new();
                    let result = (|| {
                        let mut type_parameters = Vec::new();
                        for parameter in checker
                            .source
                            .pseudo_children(declaration, "typeParameters")?
                        {
                            type_parameters.push(sequence.read(
                                checker.diagnostic_reuse_type_parameter_contents(
                                    parameter, context, consumer,
                                ),
                            )?);
                        }
                        let mut parameters = Vec::new();
                        for parameter in
                            checker.source.pseudo_children(declaration, "parameters")?
                        {
                            let name = checker.source.pseudo_child(parameter, "name")?.ok_or_else(
                                || super::super::super::internal("copied parameter lacks name"),
                            )?;
                            let name = sequence.read(
                                checker.diagnostic_clone_binding_name(name, context, consumer),
                            )?;
                            let ty = match checker.source.pseudo_child(parameter, "type")? {
                                Some(ty) => sequence
                                    .read(checker.diagnostic_reuse_visit(ty, context, consumer))?,
                                None => Node::text("any"),
                            };
                            parameters.push(Element {
                                name: Some(name),
                                rest: checker
                                    .source
                                    .pseudo_child(parameter, "dotDotDotToken")?
                                    .is_some(),
                                optional: checker
                                    .source
                                    .pseudo_child(parameter, "questionToken")?
                                    .is_some(),
                                ty,
                            });
                        }
                        let returns = match checker.source.pseudo_child(declaration, "type")? {
                            Some(ty) => sequence
                                .read(checker.diagnostic_reuse_visit(ty, context, consumer))?,
                            None => Node::text("any"),
                        };
                        Ok(syntax::Signature {
                            type_parameters,
                            parameters,
                            returns,
                            abstract_: checker
                                .source
                                .declaration_has_modifier(declaration, K::AbstractKeyword)?,
                        })
                    })();
                    sequence.finish(result)
                },
            )
        })
    }
    fn diagnostic_reuse_type_literal(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut members = Vec::new();
            for member in self.source.pseudo_children(node, "members")? {
                let kind = self.source.declaration_kind(member)?;
                if let Some(name) = self.source.pseudo_child(member, "name")?
                    && self.source.declaration_kind(name)? == K::ComputedPropertyName
                {
                    return Err(self.diagnostic_dependency(consumer, "native copied computed member requires late-bindable-name filter before visiting children"));
                }
                if kind == K::PropertySignature {
                    let name = self.source.pseudo_child(member, "name")?.ok_or_else(|| {
                        super::super::super::internal("copied property lacks name")
                    })?;
                    let name =
                        sequence.read(self.diagnostic_reuse_visit(name, context, consumer))?;
                    let name = String::from_utf8(name.print()).map_err(|_| {
                        super::super::super::internal("copied property name is not UTF8")
                    })?;
                    let ty = match self.source.pseudo_child(member, "type")? {
                        Some(ty) => {
                            sequence.read(self.diagnostic_reuse_visit(ty, context, consumer))?
                        }
                        None => Node::text("any"),
                    };
                    members.push(Member::Property {
                        name,
                        optional: self.source.pseudo_child(member, "questionToken")?.is_some(),
                        readonly: self
                            .source
                            .declaration_has_modifier(member, K::ReadonlyKeyword)?,
                        ty,
                    });
                } else if crate::syntax::is_function_like_kind(kind) {
                    // Native visits the name inside the established function
                    // scope. The pure name has no resolution unless computed.
                    let name = self
                        .source
                        .pseudo_child(member, "name")?
                        .map(|name| self.source.pseudo_source_text(name))
                        .transpose()?
                        .unwrap_or_default();
                    let signature = sequence
                        .read(self.diagnostic_reuse_signature(member, context, consumer))?;
                    if kind == K::IndexSignature {
                        let parameter = signature.parameters.first().ok_or_else(|| {
                            super::super::super::internal("copied index lacks parameter")
                        })?;
                        members.push(Member::Index {
                            name: parameter.name.clone().unwrap_or_default(),
                            readonly: self
                                .source
                                .declaration_has_modifier(member, K::ReadonlyKeyword)?,
                            key: parameter.ty.clone(),
                            value: signature.returns,
                        });
                    } else {
                        let signature_kind = match kind {
                            K::CallSignature => SignatureKind::Call,
                            K::ConstructSignature => SignatureKind::Construct,
                            K::GetAccessor => SignatureKind::Get,
                            K::SetAccessor => SignatureKind::Set,
                            K::MethodSignature => SignatureKind::Method,
                            _ => {
                                return Err(super::super::super::internal(
                                    "unsupported copied type-literal signature kind",
                                )
                                .into());
                            }
                        };
                        members.push(Member::Signature {
                            kind: signature_kind,
                            name,
                            optional: self.source.pseudo_child(member, "questionToken")?.is_some(),
                            signature,
                        });
                    }
                } else {
                    return Err(super::super::super::internal(
                        "copied type literal has non-member syntax",
                    )
                    .into());
                }
            }
            Ok(Node::Object(members))
        })();
        sequence.finish(result)
    }
}

impl Evaluator<'_, '_> {
    /// `typeToTypeNodeHelperWithPossibleReusableTypeNode`: only exact mapped
    /// annotation identity admits the reuse attempt; native nil then renders.
    /// Current diagnostic entries set maxExpansionDepth=-1, so both the
    /// active-expansion test and the post-reuse expandability probe are inert.
    pub(super) fn diagnostic_type_node_with_reuse(
        &mut self,
        ty: TypeId,
        annotation: Option<DeclarationIdentity>,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(annotation) = annotation
                && sequence
                    .read(self.diagnostic_type_from_node(annotation, context, false, consumer))?
                    == Some(ty)
                && let Some(output) =
                    sequence.read(self.diagnostic_try_reuse(annotation, context, consumer))?
            {
                return Ok(output);
            }
            sequence.read(self.diagnostic_type_node(ty, context, consumer))
        })();
        sequence.finish(result)
    }
}

impl Evaluator<'_, '_> {
    /// Mapped arm of `visitExistingNodeTreeSymbols`: the declared parameter
    /// is acquired before scope entry; child visits run inside that scope.
    fn diagnostic_reuse_mapped(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        let parameter = self
            .source
            .pseudo_child(node, "typeParameter")?
            .ok_or_else(|| super::super::super::internal("mapped syntax lacks parameter"))?;
        self.reference_node_declaration_symbol_completion(parameter)?
            .and_then(|symbol| {
                let symbol = symbol.ok_or_else(|| {
                    super::super::super::internal("mapped parameter lacks native symbol")
                })?;
                self.declared_reference_type_completion(symbol, consumer)?
                    .and_then(|parameter_type| {
                        self.diagnostic_scope(
                            context,
                            Some(node),
                            &[],
                            &[parameter_type],
                            None,
                            None,
                            consumer,
                            |checker, context| {
                                let mut sequence = NativeSequence::new();
                                let result = (|| {
                                    let root = checker.source.pseudo_node_expression(node)?;
                                    let recipe = checker.source.mapped_recipe(root)?;
                                    let parameter = sequence.read(
                                        checker.diagnostic_reuse_type_parameter_contents(
                                            parameter, context, consumer,
                                        ),
                                    )?;
                                    let name = match checker
                                        .source
                                        .pseudo_child(node, "nameType")?
                                    {
                                        Some(name) => Some(Box::new(sequence.read(
                                            checker.diagnostic_reuse_visit(name, context, consumer),
                                        )?)),
                                        None => None,
                                    };
                                    let template =
                                        match checker.source.pseudo_child(node, "type")? {
                                            Some(template) => Some(Box::new(sequence.read(
                                                checker.diagnostic_reuse_visit(
                                                    template, context, consumer,
                                                ),
                                            )?)),
                                            None => None,
                                        };
                                    Ok(Node::Mapped {
                                        readonly: super::super::mapped::mapped_readonly_text(
                                            recipe.readonly_token,
                                        ),
                                        parameter,
                                        name,
                                        optional: super::super::mapped::mapped_optional_text(
                                            recipe.question_token,
                                        ),
                                        template,
                                    })
                                })();
                                sequence.finish(result)
                            },
                        )
                    })
            })
    }
}
