use super::*;

impl<'src> Parser<'src> {
    pub(super) fn expression(&mut self) -> Result<NodeId> {
        self.in_context(self.context.without(DECORATOR), |p| {
            let mut left = p.assignment_expression()?;
            while p.at(",") {
                let operator = p.token_node()?;
                let right = p.assignment_expression()?;
                left = p.finish(
                    BinaryExpression,
                    p.start(left),
                    fields![
                        ("left", left.into()),
                        ("operatorToken", operator.into()),
                        ("right", right.into()),
                    ],
                );
            }
            Ok(left)
        })
    }
    pub(super) fn assignment_expression(&mut self) -> Result<NodeId> {
        self.expression_bp(2, true)
    }
    fn expression_bp(&mut self, minimum: u8, allow_arrow_return_type: bool) -> Result<NodeId> {
        self.nest(|p| {
            let mut left = p.unary_expression(minimum <= 2, allow_arrow_return_type)?;
            if p.nodes[left.0].syntax_kind == ArrowFunction {
                return Ok(left);
            }
            loop {
                if p.at("?") && minimum <= 3 {
                    let question = p.token_node()?;
                    let when_true =
                        p.in_context(p.context.with_in(true), |p| p.expression_bp(2, false))?;
                    if !p.at(":") {
                        return Err(p.error("Expected ':' in conditional expression"));
                    }
                    let colon = p.token_node()?;
                    let when_false = p.expression_bp(2, allow_arrow_return_type)?;
                    left = p.finish(
                        ConditionalExpression,
                        p.start(left),
                        fields![
                            ("condition", left.into()),
                            ("questionToken", question.into()),
                            ("whenTrue", when_true.into()),
                            ("colonToken", colon.into()),
                            ("whenFalse", when_false.into()),
                        ],
                    );
                    return Ok(left);
                }
                let Some((precedence, right_associative)) = binary_precedence(p.current.raw) else {
                    break;
                };
                if precedence < minimum || (p.at("in") && !p.context.allow_in) {
                    break;
                }
                if p.at("**")
                    && (matches!(
                        p.nodes[left.0].syntax_kind,
                        DeleteExpression
                            | VoidExpression
                            | TypeOfExpression
                            | AwaitExpression
                            | TypeAssertionExpression
                    ) || p.nodes[left.0].syntax_kind == PrefixUnaryExpression
                        && !matches!(
                            p.nodes[left.0].field("operator"),
                            Some(Value::Number(operator)) if *operator == PlusPlusToken.number() || *operator == MinusMinusToken.number()
                        ))
                {
                    return Err(p.error("A unary expression before '**' must be parenthesized"));
                }
                if precedence == 2
                    && matches!(
                        p.nodes[left.0].syntax_kind,
                        BinaryExpression
                            | ConditionalExpression
                            | AsExpression
                            | SatisfiesExpression
                            | TypeAssertionExpression
                            | PrefixUnaryExpression
                            | PostfixUnaryExpression
                            | AwaitExpression
                            | YieldExpression
                            | DeleteExpression
                            | VoidExpression
                            | TypeOfExpression
                            | ArrowFunction
                    )
                {
                    return Err(p.error("Expected a left-hand-side expression before assignment"));
                }
                if p.at("as") || p.at("satisfies") {
                    if p.current.newline {
                        break;
                    }
                    let kind = if p.at("as") {
                        AsExpression
                    } else {
                        SatisfiesExpression
                    };
                    p.bump()?;
                    let type_node = p.type_node()?;
                    left = p.finish(
                        kind,
                        p.start(left),
                        fields![("expression", left.into()), ("type", type_node.into())],
                    );
                    continue;
                }
                let operator = p.token_node()?;
                let right = p.expression_bp(
                    precedence + u8::from(!right_associative),
                    allow_arrow_return_type,
                )?;
                left = p.finish(
                    BinaryExpression,
                    p.start(left),
                    fields![
                        ("left", left.into()),
                        ("operatorToken", operator.into()),
                        ("right", right.into()),
                    ],
                );
                if precedence == 2 {
                    return Ok(left);
                }
            }
            Ok(left)
        })
    }
    fn unary_expression(
        &mut self,
        allow_assignment: bool,
        allow_arrow_return_type: bool,
    ) -> Result<NodeId> {
        let start = self.pos();
        let potential_arrow = self.at("(")
            || self.at("<")
            || self.at("async")
            || (self.current.kind == Kind::Word && self.peek_at("=>"));
        if allow_assignment && potential_arrow {
            if let Some(arrow) = self.probe_cached(
                ProbeKind::Arrow {
                    allow_return_type: allow_arrow_return_type,
                },
                |p| p.arrow_function(allow_arrow_return_type),
            )? {
                return Ok(arrow);
            }
        }
        let next = if self.at("await") || self.at("yield") {
            Some(self.peek())
        } else {
            None
        };
        let keyword_operand = next.is_some_and(|next| {
            !next.newline && matches!(next.kind, Kind::Word | Kind::Number | Kind::String)
        });
        if allow_assignment && self.at("yield") && (self.context.contains(YIELD) || keyword_operand)
        {
            self.bump()?;
            let mut fields: Fields<'src> = Fields::new();
            if !self.current.newline {
                let delegated = self.optional_token("*", "asteriskToken", &mut fields)?;
                if !matches!(self.current.raw, ";" | "}" | ")" | "]" | "," | ":")
                    && self.current.kind != Kind::Eof
                {
                    fields.push(("expression", self.assignment_expression()?.into()));
                } else if delegated {
                    return Err(self.error("Expected an expression after 'yield *'"));
                }
            }
            return Ok(self.finish(YieldExpression, start, fields));
        }
        if matches!(
            self.current.raw,
            "delete" | "typeof" | "void" | "+" | "-" | "!" | "~" | "++" | "--"
        ) || (self.at("await") && (self.context.contains(AWAIT) || keyword_operand))
        {
            let operator = self.bump()?;
            let operand = if matches!(operator.raw, "++" | "--") {
                self.nest(|p| p.postfix_expression(false))?
            } else {
                self.nest(|p| p.unary_expression(false, allow_arrow_return_type))?
            };
            let (kind, fields) = match operator.raw {
                "delete" => (DeleteExpression, fields![("expression", operand.into())]),
                "typeof" => (TypeOfExpression, fields![("expression", operand.into())]),
                "void" => (VoidExpression, fields![("expression", operand.into())]),
                "await" => (AwaitExpression, fields![("expression", operand.into())]),
                _ => (
                    PrefixUnaryExpression,
                    fields![
                        (
                            "operator",
                            Value::Number(token_kind(operator.raw, operator.kind).number()),
                        ),
                        ("operand", operand.into()),
                    ],
                ),
            };
            return Ok(self.finish(kind, start, fields));
        }
        if self.at("<") && !self.options.jsx {
            self.bump()?;
            let type_node = self.type_node()?;
            self.expect_type_close()?;
            let expression = self.nest(|p| p.unary_expression(false, allow_arrow_return_type))?;
            return Ok(self.finish(
                TypeAssertionExpression,
                start,
                fields![
                    ("type", type_node.into()),
                    ("expression", expression.into()),
                ],
            ));
        }
        let operand = self.postfix_expression(false)?;
        if !self.current.newline && matches!(self.current.raw, "++" | "--") {
            let operator = self.bump()?;
            return Ok(self.finish(
                PostfixUnaryExpression,
                self.start(operand),
                fields![
                    ("operand", operand.into()),
                    (
                        "operator",
                        Value::Number(token_kind(operator.raw, operator.kind).number()),
                    ),
                ],
            ));
        }
        Ok(operand)
    }
    /// The true arm of a conditional owns its ':' unless an unambiguous arrow
    /// signature establishes a return type. Keep this restriction in probe keys.
    fn arrow_function(&mut self, allow_arrow_return_type: bool) -> Result<Option<NodeId>> {
        let mut committed = false;
        let result = self.nest(|p| {
            let (start, fields, modifiers) = p.arrow_head()?;
            let has_return_type = fields.iter().any(|(key, _)| *key == "type");
            let ambiguous_return_type = !fields.iter().any(|(key, _)| *key == "typeParameters")
                && match fields.iter().find(|(key, _)| *key == "parameters") {
                    Some((_, Value::Nodes(parameters))) => {
                        !parameters.is_empty()
                            && parameters.iter().all(|id| {
                                !p.nodes[id.0].fields.iter().any(|(key, _)| {
                                    matches!(
                                        *key,
                                        "type" | "questionToken" | "dotDotDotToken" | "modifiers"
                                    )
                                })
                            })
                    }
                    _ => false,
                };
            // In a conditional's true arm, this ':' might belong to the
            // conditional instead of the arrow return type. Otherwise the
            // consumed '=>' commits the head, including errors in its body.
            let conditional_boundary =
                !allow_arrow_return_type && has_return_type && ambiguous_return_type;
            committed = !conditional_boundary;
            let async_function = modifiers
                .iter()
                .any(|id| p.nodes[id.0].syntax_kind == AsyncKeyword);
            let body = p.in_context(p.context.function(async_function, false), |p| {
                if p.at("{") {
                    p.block(Block)
                } else {
                    p.expression_bp(2, allow_arrow_return_type)
                }
            })?;
            if conditional_boundary && !p.at(":") {
                return Ok(None);
            }
            let id = p.finish(ArrowFunction, start, fields);
            p.field(id, "body", body);
            p.attach_modifiers(id, modifiers);
            Ok(Some(id))
        });
        if committed {
            result
        } else {
            result.or(Ok(None))
        }
    }

    fn arrow_head(&mut self) -> Result<(usize, Fields<'src>, Vec<NodeId>)> {
        let start = self.pos();
        let mut fields: Fields<'src> = Fields::new();
        let mut modifiers = vec![];
        if self.at("async") && !self.peek_at("=>") {
            modifiers.push(self.token_node()?);
            if self.current.newline {
                return Err(self.error("Line break after async"));
            }
        }
        if self.at("<") {
            let generic_start = self.current.start;
            let type_parameters = self.type_parameters()?;
            if self.options.jsx
                && type_parameters.len() == 1
                && !self.lexer.src[generic_start..self.end].contains(',')
                && self.nodes[type_parameters[0].0]
                    .field("constraint")
                    .is_none()
                && self.nodes[type_parameters[0].0].field("default").is_none()
            {
                return Err(
                    self.error("Ambiguous JSX generic arrow; add a trailing comma or constraint")
                );
            }
            fields.push(("typeParameters", type_parameters.into()));
        }
        let simple_parameter = !self.at("(");
        let parameters = if self.at("(") {
            let async_function = modifiers
                .iter()
                .any(|id| self.nodes[id.0].syntax_kind == AsyncKeyword);
            self.parameters_with_context(async_function, false)?
        } else {
            let pos = self.pos();
            if self.at("await") && self.context.contains(AWAIT)
                || self.at("yield") && self.context.contains(YIELD)
            {
                return Err(self.error("Expected an arrow parameter identifier"));
            }
            let name = self.binding_identifier()?;
            vec![self.finish(Parameter, pos, fields![("name", name.into())])]
        };
        fields.push(("parameters", parameters.into()));
        if self.eat(":")? {
            fields.push(("type", self.return_type()?.into()));
        }
        if !self.at("=>") || self.current.newline && simple_parameter && !modifiers.is_empty() {
            return Err(self.error("Expected arrow"));
        }
        fields.push(("equalsGreaterThanToken", self.token_node()?.into()));
        Ok((start, fields, modifiers))
    }
    pub(super) fn postfix_expression(&mut self, no_call: bool) -> Result<NodeId> {
        self.nest(|p| {
            let expression = p.primary_expression()?;
            p.postfix_rest(expression, no_call)
        })
    }

    fn postfix_rest(&mut self, mut expression: NodeId, no_call: bool) -> Result<NodeId> {
        loop {
            let start = self.start(expression);
            let optional_chain = self.nodes[expression.0].flags & node_flags::OPTIONAL_CHAIN != 0;
            if self.at(".") || self.at("?.") {
                if no_call && self.at("?.") {
                    return Err(self.error("An optional chain cannot be the target of 'new'"));
                }
                let question = if self.at("?.") {
                    Some(self.token_node()?)
                } else {
                    self.bump()?;
                    None
                };
                let chain = question.is_some() || self.try_optional_chain(expression);
                let mut fields = fields![("expression", expression.into())];
                if let Some(question) = question {
                    fields.push(("questionDotToken", question.into()));
                }
                let kind = if question.is_some() && self.eat("[")? {
                    fields.push(("argumentExpression", self.expression_allow_in()?.into()));
                    self.expect("]")?;
                    ElementAccessExpression
                } else if question.is_some() && !no_call && (self.at("(") || self.type_open()) {
                    if self.type_open() {
                        fields.push(("typeArguments", self.type_arguments()?.into()));
                    }
                    if matches!(
                        self.current.kind,
                        Kind::TemplateHead | Kind::NoSubstitutionTemplate
                    ) {
                        fields[0].0 = "tag";
                        fields.push(("template", self.tagged_template()?.into()));
                        TaggedTemplateExpression
                    } else {
                        fields.push(("arguments", self.arguments()?.into()));
                        CallExpression
                    }
                } else if question.is_some()
                    && matches!(
                        self.current.kind,
                        Kind::TemplateHead | Kind::NoSubstitutionTemplate
                    )
                {
                    fields[0].0 = "tag";
                    fields.push(("template", self.tagged_template()?.into()));
                    TaggedTemplateExpression
                } else {
                    if self.nodes[expression.0].syntax_kind == ExpressionWithTypeArguments {
                        return Err(self.error(
                            "An instantiation expression cannot be followed by a property access",
                        ));
                    }
                    if chain && self.current.kind == Kind::Private {
                        return Err(
                            self.error("An optional chain cannot contain private identifiers")
                        );
                    }
                    fields.push(("name", self.identifier()?.into()));
                    PropertyAccessExpression
                };
                expression = self.finish(kind, start, fields);
                if chain {
                    self.nodes[expression.0].flags |= node_flags::OPTIONAL_CHAIN;
                }
            } else if !self.context.contains(DECORATOR) && self.eat("[")? {
                let chain = self.try_optional_chain(expression);
                let argument = self.expression_allow_in()?;
                self.expect("]")?;
                expression = self.finish(
                    ElementAccessExpression,
                    start,
                    fields![
                        ("expression", expression.into()),
                        ("argumentExpression", argument.into()),
                    ],
                );
                if chain {
                    self.nodes[expression.0].flags |= node_flags::OPTIONAL_CHAIN;
                }
            } else if !no_call && self.at("(") {
                let chain = self.try_optional_chain(expression);
                let arguments = self.arguments()?;
                expression = self.finish(
                    CallExpression,
                    start,
                    fields![
                        ("expression", expression.into()),
                        ("arguments", arguments.into()),
                    ],
                );
                if chain {
                    self.nodes[expression.0].flags |= node_flags::OPTIONAL_CHAIN;
                }
            } else if self.type_open() {
                let arguments = self.expression_type_arguments(no_call)?;
                let Some(type_arguments) = arguments else {
                    break;
                };
                let mut fields = fields![
                    ("expression", expression.into()),
                    ("typeArguments", type_arguments.into()),
                ];
                let chain = !no_call && self.at("(") && self.try_optional_chain(expression);
                let kind = if !no_call && self.at("(") {
                    fields.push(("arguments", self.arguments()?.into()));
                    CallExpression
                } else if !no_call
                    && matches!(
                        self.current.kind,
                        Kind::TemplateHead | Kind::NoSubstitutionTemplate
                    )
                {
                    fields.push(("template", self.tagged_template()?.into()));
                    fields[0].0 = "tag";
                    TaggedTemplateExpression
                } else {
                    ExpressionWithTypeArguments
                };
                expression = self.finish(kind, start, fields);
                if kind == CallExpression && chain
                    || kind == TaggedTemplateExpression && optional_chain
                {
                    self.nodes[expression.0].flags |= node_flags::OPTIONAL_CHAIN;
                }
            } else if matches!(
                self.current.kind,
                Kind::TemplateHead | Kind::NoSubstitutionTemplate
            ) {
                let template = self.tagged_template()?;
                expression = self.finish(
                    TaggedTemplateExpression,
                    start,
                    fields![("tag", expression.into()), ("template", template.into())],
                );
                if optional_chain {
                    self.nodes[expression.0].flags |= node_flags::OPTIONAL_CHAIN;
                }
            } else if self.at("!") && !self.current.newline {
                self.bump()?;
                expression = self.finish(
                    NonNullExpression,
                    start,
                    fields![("expression", expression.into())],
                );
            } else {
                break;
            }
        }
        Ok(expression)
    }
    /// TypeScript's `parseTypeArgumentsInExpression` and
    /// `canFollowTypeArgumentsInExpression`. The closing `>` must stand alone,
    /// as `reScanGreaterToken` requires: one joined to a following `>` or `=`
    /// (`f(a < b, c >= d)`) is an operator, not a closer.
    fn can_follow_type_arguments(&self, no_call: bool) -> bool {
        let joined = self.current.start == self.end && self.current.raw.starts_with(['>', '=']);
        !joined && self.can_follow_closed_type_arguments(no_call)
    }
    fn can_follow_closed_type_arguments(&self, no_call: bool) -> bool {
        no_call
            || self.at("(")
            || matches!(
                self.current.kind,
                Kind::TemplateHead | Kind::NoSubstitutionTemplate
            )
            // TypeScript scans `>` alone here, so `>=` and `>>` also begin
            // with a rejected `>`.
            || !(self.current.raw.starts_with('>') || matches!(self.current.raw, "<" | "+" | "-"))
                && (self.current.newline
                    || binary_precedence(self.current.raw).is_some()
                    || !matches!(
                        self.current.kind,
                        Kind::Word | Kind::Private | Kind::Number | Kind::String
                    ) && !matches!(self.current.raw, "[" | "{" | "!" | "~" | "++" | "--"))
    }
    fn expression_type_arguments(&mut self, no_call: bool) -> Result<Option<Vec<NodeId>>> {
        // Attempts that `angles` rules out could only fail, so skipping them
        // changes no result.
        let angle = self.angles.at(self.lexer.src, self.current.start);
        if angle.strict {
            let strict = self.speculate(angle.stop, |p| {
                p.probe(|p| {
                    let arguments = p.type_arguments()?;
                    Ok(p.can_follow_type_arguments(no_call).then_some(arguments))
                })
            });
            if let Some(arguments) = self.matched(strict)? {
                return Ok(arguments);
            }
        }
        if self.recovering_types() || self.angles.closers(self.current.start, angle.stop) == 0 {
            return Ok(None);
        }
        // Recovery commits only when it reaches the real closing '>'
        // and the following token establishes expression type arguments.
        let recovered = self.speculate(angle.stop, |p| {
            p.look_ahead(|p| {
                let (_, error) = p.recover_types(Self::type_arguments)?;
                Ok(p.can_follow_type_arguments(no_call)
                    .then_some(error)
                    .flatten())
            })
        });
        match self.matched(recovered)?.flatten() {
            Some(error) => Err(error),
            None => Ok(None),
        }
    }

    /// Run a type-argument attempt whose parse cannot pass `stop`, counting
    /// the type-argument lists it opens from zero.
    fn speculate<T>(&mut self, stop: usize, attempt: impl FnOnce(&mut Self) -> T) -> T {
        let enclosing = (self.speculation.replace(stop), self.type_argument_frames);
        self.type_argument_frames = 0;
        let result = attempt(self);
        (self.speculation, self.type_argument_frames) = enclosing;
        result
    }

    fn try_optional_chain(&mut self, expression: NodeId) -> bool {
        let mut id = expression;
        while self.nodes[id.0].flags & node_flags::OPTIONAL_CHAIN == 0
            && self.nodes[id.0].syntax_kind == NonNullExpression
        {
            let Some(Value::Node(inner)) = self.nodes[id.0].field("expression") else {
                return false;
            };
            id = *inner;
        }
        if self.nodes[id.0].flags & node_flags::OPTIONAL_CHAIN == 0 {
            return false;
        }
        let mut id = expression;
        while self.nodes[id.0].syntax_kind == NonNullExpression {
            self.nodes[id.0].flags |= node_flags::OPTIONAL_CHAIN;
            let Some(Value::Node(inner)) = self.nodes[id.0].field("expression") else {
                break;
            };
            id = *inner;
        }
        true
    }
    fn primary_expression(&mut self) -> Result<NodeId> {
        self.nest(|p| {
        p.check_token()?;
        let start = p.pos();
        match p.current.kind {
            Kind::Number | Kind::String => return p.literal(),
            Kind::TemplateHead | Kind::NoSubstitutionTemplate => return p.template(false),
            Kind::Eof => return Err(p.error("Expected expression, found end of input")),
            _ => {}
        }
        match p.current.raw {
            "(" => {
                p.bump()?;
                let expression = p.expression_allow_in()?;
                p.expect(")")?;
                Ok(p.finish(
                    ParenthesizedExpression,
                    start,
                    fields![("expression", expression.into())],
                ))
            }
            "[" => {
                p.bump()?;
                let mut elements = vec![];
                while !p.at("]") {
                    let pos = p.pos();
                    if p.at(",") {
                        p.bump()?;
                        elements.push(p.make(OmittedExpression, pos, pos, vec![]));
                        continue;
                    }
                    if p.eat("...")? {
                        let expression = p.assignment_expression()?;
                        elements.push(p.finish(
                            SpreadElement,
                            pos,
                            fields![("expression", expression.into())],
                        ));
                    } else {
                        elements.push(p.assignment_expression()?);
                    }
                    if !p.eat(",")? {
                        break;
                    }
                }
                p.expect("]")?;
                Ok(p.finish(
                    ArrayLiteralExpression,
                    start,
                    fields![("elements", elements.into())],
                ))
            }
            "{" => p.object_literal(),
            "function" => p.function(start, FunctionExpression, vec![]),
            "async" if p.peek_at("function") && !p.peek().newline => {
                let modifier = p.token_node()?;
                p.function(start, FunctionExpression, vec![modifier])
            }
            "class" => p.class(start, ClassExpression, vec![]),
            "@" => {
                let modifiers = p.modifiers(false)?;
                if !p.at("class") {
                    return Err(p.error("Expected a class expression after decorators"));
                }
                p.class(start, ClassExpression, modifiers)
            }
            "new" => p.new_expression(),
            "import" if p.peek_at(".") => {
                p.bump()?;
                p.expect(".")?;
                let name = p.identifier()?;
                if matches!(p.nodes[name.0].field("escapedText"), Some(Value::Text(text)) if text == "defer")
                {
                    if p.at("(") || p.at("<") {
                        p.source_flags |= node_flags::POSSIBLY_CONTAINS_DYNAMIC_IMPORT;
                    }
                } else {
                    p.source_flags |= node_flags::POSSIBLY_CONTAINS_IMPORT_META;
                }
                Ok(p.finish(
                    MetaProperty,
                    start,
                    fields![
                        ("keywordToken", Value::Number(ImportKeyword.number())),
                        ("name", name.into())
                    ],
                ))
            }
            "super" => {
                if !matches!(p.peek().raw, "(" | "." | "[") {
                    return Err(p.error("'super' must be followed by a call or member access"));
                }
                p.token_node()
            }
            "null" | "true" | "false" | "this" => p.token_node(),
            "/" | "/=" => {
                let token = p.lexer.regex(p.current.start);
                if let Some(error) = token.error {
                    return Err(Diagnostic {
                        message: error.into(),
                        span: Span {
                            start: token.start,
                            end: token.end,
                        },
                    });
                }
                p.end = token.end;
                p.current = p.lexer.next();
                Ok(p.make(
                    RegularExpressionLiteral,
                    start,
                    token.end,
                    fields![("text", token.raw.into())],
                ))
            }
            "<" if p.options.jsx => p.jsx_element(),
            "await" if p.context.contains(AWAIT) => {
                Err(p.error("Expected a left-hand-side expression before await"))
            }
            "yield" if p.context.contains(YIELD) => {
                Err(p.error("Expected a left-hand-side expression before yield"))
            }
            "import" => {
                if matches!(p.peek().raw, "(" | "<" | "<<") {
                    p.source_flags |= node_flags::POSSIBLY_CONTAINS_DYNAMIC_IMPORT;
                    return p.token_node();
                }
                Err(p.error("Expected '(' or '.' after import"))
            }
            _ if p.current.kind == Kind::Word || p.current.kind == Kind::Private => {
                if matches!(
                    p.current.raw,
                    "else"
                        | "return"
                        | "throw"
                        | "for"
                        | "while"
                        | "if"
                        | "switch"
                        | "case"
                        | "default"
                        | "try"
                        | "catch"
                        | "finally"
                        | "const"
                        | "var"
                        | "export"
                        | "extends"
                        | "implements"
                        | "break"
                        | "continue"
                        | "debugger"
                        | "do"
                        | "with"
                        | "enum"
                ) {
                    return Err(p.error("Unexpected keyword in expression"));
                }
                if p.current.kind == Kind::Private {
                    p.identifier()
                } else {
                    p.binding_identifier()
                }
            }
            _ => Err(p.error(format!("Expected expression, found '{}'", p.current.raw))),
        }
        })
    }

    fn arguments(&mut self) -> Result<Vec<NodeId>> {
        self.nest(|p| {
            p.expect("(")?;
            p.in_context(p.context.with_in(true).without(DECORATOR), |p| {
                let mut arguments = vec![];
                while !p.at(")") {
                    let start = p.pos();
                    if p.eat("...")? {
                        let expression = p.assignment_expression()?;
                        arguments.push(p.finish(
                            SpreadElement,
                            start,
                            fields![("expression", expression.into())],
                        ));
                    } else {
                        arguments.push(p.assignment_expression()?);
                    }
                    if !p.eat(",")? {
                        break;
                    }
                }
                p.expect(")")?;
                Ok(arguments)
            })
        })
    }

    fn new_expression(&mut self) -> Result<NodeId> {
        self.nest(|p| {
            let start = p.pos();
            p.expect("new")?;
            if p.eat(".")? {
                let name = p.identifier()?;
                return Ok(p.finish(
                    MetaProperty,
                    start,
                    fields![
                        ("keywordToken", Value::Number(NewKeyword.number())),
                        ("name", name.into())
                    ],
                ));
            }
            let expression = p.postfix_expression(true)?;
            let mut fields = if p.nodes[expression.0].syntax_kind == ExpressionWithTypeArguments {
                p.nodes[expression.0].fields.clone()
            } else {
                fields![("expression", expression.into())]
            };
            if p.at("(") {
                fields.push(("arguments", p.arguments()?.into()));
            }
            Ok(p.finish(NewExpression, start, fields))
        })
    }
    fn object_literal(&mut self) -> Result<NodeId> {
        let start = self.pos();
        self.expect("{")?;
        let mut properties = vec![];
        while !self.at("}") {
            let pos = self.pos();
            if self.eat("...")? {
                let expression = self.assignment_expression()?;
                properties.push(self.finish(
                    SpreadAssignment,
                    pos,
                    fields![("expression", expression.into())],
                ));
            } else {
                let modifiers = self.modifiers(true)?;
                let accessor = if matches!(self.current.raw, "get" | "set")
                    && (matches!(
                        self.peek().kind,
                        Kind::Word | Kind::Private | Kind::String | Kind::Number
                    ) || self.peek_at("["))
                {
                    Some(self.bump()?.raw)
                } else {
                    None
                };
                let mut fields: Fields<'src> = Fields::new();
                let generator = self.optional_token("*", "asteriskToken", &mut fields)?;
                let name = self.property_name()?;
                fields.push(("name", name.into()));
                self.optional_token("?", "questionToken", &mut fields)?;
                self.optional_token("!", "exclamationToken", &mut fields)?;
                let kind = if !generator && accessor.is_none() && self.eat(":")? {
                    fields.push(("initializer", self.assignment_allow_in()?.into()));
                    PropertyAssignment
                } else if generator || accessor.is_some() || self.at("(") || self.at("<") {
                    if self.at("<") {
                        fields.push(("typeParameters", self.type_parameters()?.into()));
                    }
                    let async_function = accessor.is_none()
                        && modifiers
                            .iter()
                            .any(|id| self.nodes[id.0].syntax_kind == AsyncKeyword);
                    let generator = generator && accessor.is_none();
                    fields.push((
                        "parameters",
                        self.parameters_with_context(async_function, generator)?
                            .into(),
                    ));
                    if self.eat(":")? {
                        fields.push(("type", self.return_type()?.into()));
                    }
                    self.in_context(self.context.function(async_function, generator), |p| {
                        if p.at("{") {
                            fields.push(("body", p.block(Block)?.into()));
                        } else {
                            p.semi()?;
                        }
                        Ok(())
                    })?;
                    match accessor {
                        Some("get") => GetAccessor,
                        Some("set") => SetAccessor,
                        _ => MethodDeclaration,
                    }
                } else {
                    if self.nodes[name.0].syntax_kind != Identifier {
                        return Err(self.error("Expected ':' after property name"));
                    }
                    if matches!(self.nodes[name.0].field("escapedText"), Some(Value::Text(text)) if reserved_word(text))
                    {
                        return Err(self.error("Expected ':' after reserved property name"));
                    }
                    if matches!(self.nodes[name.0].field("escapedText"), Some(Value::Text(text)) if text == "await" && self.context.contains(AWAIT) || text == "yield" && self.context.contains(YIELD))
                    {
                        return Err(self.error("Expected ':' after a contextual property name"));
                    }
                    if self.at("=") {
                        fields.push(("equalsToken", self.token_node()?.into()));
                        fields.push((
                            "objectAssignmentInitializer",
                            self.assignment_allow_in()?.into(),
                        ));
                    }
                    ShorthandPropertyAssignment
                };
                let id = self.finish(kind, pos, fields);
                self.attach_modifiers(id, modifiers);
                properties.push(id);
            }
            if !self.eat(",")? {
                break;
            }
        }
        self.expect("}")?;
        Ok(self.finish(
            ObjectLiteralExpression,
            start,
            fields![("properties", properties.into())],
        ))
    }
    pub(super) fn template(&mut self, type_context: bool) -> Result<NodeId> {
        self.template_inner(type_context, false)
    }

    fn tagged_template(&mut self) -> Result<NodeId> {
        self.template_inner(false, true)
    }
    fn template_inner(&mut self, type_context: bool, tagged: bool) -> Result<NodeId> {
        self.nest(|p| {
            let start = p.pos();
            if p.current.kind == Kind::NoSubstitutionTemplate {
                let literal = p.literal_mode(tagged)?;
                return if type_context {
                    Ok(p.finish(LiteralType, start, fields![("literal", literal.into())]))
                } else {
                    Ok(literal)
                };
            }
            let head = p.literal_mode(tagged)?;
            let mut spans = vec![];
            loop {
                let pos = p.pos();
                let value = p.in_context(p.context.with_in(true), |p| {
                    if type_context {
                        p.type_node()
                    } else {
                        p.expression()
                    }
                })?;
                if !p.at("}") {
                    return Err(p.error("Expected '}' in template substitution"));
                }
                let full_start = p.current.full_start;
                p.current = p.lexer.template(p.current.start, true, false);
                p.current.full_start = full_start;
                p.check_token()?;
                let tail = p.current.kind == Kind::TemplateTail;
                let literal = p.literal_mode(tagged)?;
                spans.push(p.finish(
                    if type_context {
                        TemplateLiteralTypeSpan
                    } else {
                        TemplateSpan
                    },
                    pos,
                    fields![
                        (
                            if type_context { "type" } else { "expression" },
                            value.into(),
                        ),
                        ("literal", literal.into()),
                    ],
                ));
                if tail {
                    break;
                }
            }
            Ok(p.finish(
                if type_context {
                    TemplateLiteralType
                } else {
                    TemplateExpression
                },
                start,
                fields![("head", head.into()), ("templateSpans", spans.into())],
            ))
        })
    }
}

fn binary_precedence(operator: &str) -> Option<(u8, bool)> {
    Some(match operator {
        "," => (1, false),
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "**=" | "<<=" | ">>=" | ">>>=" | "&=" | "|="
        | "^=" | "&&=" | "||=" | "??=" => (2, true),
        "??" | "||" => (4, false),
        "&&" => (5, false),
        "|" => (6, false),
        "^" => (7, false),
        "&" => (8, false),
        "==" | "!=" | "===" | "!==" => (9, false),
        "<" | ">" | "<=" | ">=" | "instanceof" | "in" | "as" | "satisfies" => (10, false),
        "<<" | ">>" | ">>>" => (11, false),
        "+" | "-" => (12, false),
        "*" | "/" | "%" => (13, false),
        "**" => (14, true),
        _ => return None,
    })
}
