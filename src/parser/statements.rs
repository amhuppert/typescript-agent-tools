use super::*;

impl<'src> Parser<'src> {
    pub(super) fn statement(&mut self) -> Result<NodeId> {
        let deprecated = self.deprecated_trivia(false);
        let result = self.in_context(self.context, |p| p.nest(Self::statement_inner));
        if deprecated {
            if let Ok(id) = result {
                self.nodes[id.0].flags |= 536870912;
            }
        }
        result
    }
    fn statement_inner(&mut self) -> Result<NodeId> {
        self.check_token()?;
        let start = self.pos();
        match self.current.raw {
            ";" => {
                self.bump()?;
                return Ok(self.finish("EmptyStatement", start, vec![]));
            }
            "{" => return self.block("Block"),
            "if" => {
                self.bump()?;
                let expression = self.parenthesized_expression()?;
                let then_statement = self.statement()?;
                let mut fields = fields![
                    ("expression", expression.into()),
                    ("thenStatement", then_statement.into()),
                ];
                if self.eat("else")? {
                    fields.push(("elseStatement", self.statement()?.into()));
                }
                return Ok(self.finish("IfStatement", start, fields));
            }
            "while" | "with" => {
                let kind = if self.at("while") {
                    "WhileStatement"
                } else {
                    "WithStatement"
                };
                self.bump()?;
                let expression = self.parenthesized_expression()?;
                let context = if kind == "WithStatement" {
                    self.context.with(IN_WITH)
                } else {
                    self.context
                };
                let statement = self.in_context(context, Self::statement)?;
                return Ok(self.finish(
                    kind,
                    start,
                    fields![
                        ("expression", expression.into()),
                        ("statement", statement.into()),
                    ],
                ));
            }
            "do" => {
                self.bump()?;
                let statement = self.statement()?;
                self.expect("while")?;
                let expression = self.parenthesized_expression()?;
                self.eat(";")?;
                return Ok(self.finish(
                    "DoStatement",
                    start,
                    fields![
                        ("statement", statement.into()),
                        ("expression", expression.into()),
                    ],
                ));
            }
            "for" => return self.for_statement(),
            "switch" => return self.switch_statement(),
            "try" => return self.try_statement(),
            "return" | "throw" | "break" | "continue" => {
                let keyword = self.bump()?.raw;
                let kind = match keyword {
                    "return" => "ReturnStatement",
                    "throw" => "ThrowStatement",
                    "break" => "BreakStatement",
                    _ => "ContinueStatement",
                };
                let mut fields: Fields<'src> = Fields::new();
                if keyword == "throw" && self.current.newline {
                    return Err(self.error("Line break is not permitted after throw"));
                }
                if !self.current.newline
                    && !self.at(";")
                    && !self.at("}")
                    && self.current.kind != Kind::Eof
                {
                    if keyword == "break" || keyword == "continue" {
                        fields.push(("label", self.binding_identifier()?.into()));
                    } else {
                        fields.push(("expression", self.expression_allow_in()?.into()));
                    }
                } else if keyword == "throw" {
                    return Err(self.error("Expected an expression after throw"));
                }
                self.semi()?;
                return Ok(self.finish(kind, start, fields));
            }
            "debugger" => {
                self.bump()?;
                self.semi()?;
                return Ok(self.finish("DebuggerStatement", start, vec![]));
            }
            "import" if !matches!(self.peek().raw, "(" | "." | "<" | "<<") => {
                return self.import_declaration(start, vec![]);
            }
            "export" => return self.export_statement(),
            _ => {}
        }
        let mut modifiers = self
            .attempt(|p| {
                let modifiers = p.modifiers(false)?;
                if modifiers.is_empty() || p.declaration_head(&modifiers) {
                    Ok(modifiers)
                } else {
                    Err(p.error("Expected declaration after modifiers"))
                }
            })
            .unwrap_or_default();
        self.ambient_modifiers(&modifiers);
        if modifiers
            .iter()
            .any(|id| self.nodes[id.0].kind == "DefaultKeyword")
            && !modifiers
                .iter()
                .any(|id| matches!(self.nodes[id.0].kind, "ExportKeyword" | "Decorator"))
        {
            return Err(self.error("Expected 'export' before a default declaration"));
        }
        let declaration = match self.current.raw {
            "function" => {
                Some(self.function(start, "FunctionDeclaration", std::mem::take(&mut modifiers))?)
            }
            "class" => {
                Some(self.class(start, "ClassDeclaration", std::mem::take(&mut modifiers))?)
            }
            "interface" if self.peek().kind == Kind::Word && !self.peek().newline => {
                Some(self.interface(start, std::mem::take(&mut modifiers))?)
            }
            "type"
                if self.peek().kind == Kind::Word
                    && (!self.peek().newline
                        || modifiers
                            .iter()
                            .any(|id| self.nodes[id.0].kind == "DeclareKeyword")) =>
            {
                Some(self.type_alias(start, std::mem::take(&mut modifiers))?)
            }
            "enum" => Some(self.enumeration(start, std::mem::take(&mut modifiers))?),
            "const" if self.peek_at("enum") => {
                let mut modifiers = std::mem::take(&mut modifiers);
                modifiers.push(self.token_node()?);
                Some(self.enumeration(start, modifiers)?)
            }
            "namespace" | "module" | "global" if self.declaration_head(&modifiers) => {
                Some(self.namespace(start, std::mem::take(&mut modifiers))?)
            }
            "var" | "let" | "const" | "using" if self.declaration_head(&modifiers) => {
                Some(self.variable_statement(start, std::mem::take(&mut modifiers))?)
            }
            "await" if self.using_declaration(false) => {
                Some(self.variable_statement(start, std::mem::take(&mut modifiers))?)
            }
            "import" if !modifiers.is_empty() => {
                Some(self.import_declaration(start, std::mem::take(&mut modifiers))?)
            }
            "export" if !modifiers.is_empty() => {
                let id = self.export_statement()?;
                self.nodes[id.0].span.start = start;
                if let Some((_, Value::Nodes(existing))) = self.nodes[id.0]
                    .fields
                    .iter_mut()
                    .find(|(key, _)| *key == "modifiers")
                {
                    modifiers.append(existing);
                    *existing = std::mem::take(&mut modifiers);
                } else {
                    self.attach_modifiers(id, std::mem::take(&mut modifiers));
                }
                Some(id)
            }
            _ => None,
        };
        if let Some(id) = declaration {
            return Ok(id);
        }
        if !modifiers.is_empty() {
            return Err(self.error("Expected declaration after modifiers"));
        }
        if self.current.kind == Kind::Word && self.peek_at(":") {
            let label = self.binding_identifier()?;
            self.expect(":")?;
            let statement = self.statement()?;
            return Ok(self.finish(
                "LabeledStatement",
                start,
                fields![("label", label.into()), ("statement", statement.into())],
            ));
        }
        let expression = self.expression_allow_in()?;
        self.semi()?;
        Ok(self.finish(
            "ExpressionStatement",
            start,
            fields![("expression", expression.into())],
        ))
    }

    fn declaration_head(&self, modifiers: &[NodeId]) -> bool {
        let next = self.peek();
        match self.current.raw {
            "interface" => next.kind == Kind::Word && !next.newline,
            "type" => {
                next.kind == Kind::Word
                    && (!next.newline
                        || modifiers
                            .iter()
                            .any(|id| self.nodes[id.0].kind == "DeclareKeyword"))
            }
            "namespace" | "module" => {
                (next.kind == Kind::String || next.kind == Kind::Word && !reserved_word(next.raw))
                    && !next.newline
            }
            "global" => next.raw == "{" || next.kind == Kind::Word,
            "let" => {
                matches!(next.raw, "{" | "[") || next.kind == Kind::Word && !reserved_word(next.raw)
            }
            "using" | "await" => self.using_declaration(false),
            "function" | "class" | "enum" | "const" | "var" | "import" | "export" => true,
            _ => false,
        }
    }
    fn using_declaration(&self, disallow_of: bool) -> bool {
        let mut lexer = self.lexer;
        let mut next = lexer.next();
        if self.at("await") {
            if next.raw != "using" {
                return false;
            }
            next = lexer.next();
        }
        if disallow_of && next.raw == "of" {
            return matches!(lexer.next().raw, "=" | ";" | ":");
        }
        !next.newline && (next.raw == "{" || next.kind == Kind::Word && !reserved_word(next.raw))
    }

    pub(super) fn block(&mut self, kind: &'static str) -> Result<NodeId> {
        self.nest(|p| {
            let start = p.pos();
            p.expect("{")?;
            let mut statements = Vec::new();
            while !p.at("}") {
                if p.current.kind == Kind::Eof {
                    return Err(p.error("Unterminated block"));
                }
                statements.push(p.statement()?);
            }
            p.expect("}")?;
            Ok(p.finish(kind, start, fields![("statements", statements.into())]))
        })
    }
    fn parenthesized_expression(&mut self) -> Result<NodeId> {
        self.expect("(")?;
        let expression = self.expression_allow_in()?;
        self.expect(")")?;
        Ok(expression)
    }
    fn for_statement(&mut self) -> Result<NodeId> {
        let start = self.pos();
        self.bump()?;
        let mut fields: Fields<'src> = Fields::new();
        self.optional_token("await", "awaitModifier", &mut fields)?;
        self.expect("(")?;
        self.in_context(self.context.with_in(false), |p| {
            if !p.at(";") {
                let initializer = if matches!(p.current.raw, "var" | "let" | "const")
                    || p.at("using") && p.using_declaration(true)
                    || p.at("await") && p.using_declaration(false)
                {
                    p.variable_list()?
                } else {
                    p.expression()?
                };
                fields.push(("initializer", initializer.into()));
            }
            Ok(())
        })?;
        let kind = if self.at("of") || self.at("in") {
            let keyword = self.bump()?.raw;
            if keyword == "in" && fields.iter().any(|(key, _)| *key == "awaitModifier") {
                return Err(self.error("A 'for await' loop requires 'of'"));
            }
            let expression = self.expression()?;
            fields.push(("expression", expression.into()));
            if keyword == "of" {
                "ForOfStatement"
            } else {
                "ForInStatement"
            }
        } else {
            self.expect(";")?;
            if !self.at(";") {
                fields.push(("condition", self.expression()?.into()));
            }
            self.expect(";")?;
            if !self.at(")") {
                fields.push(("incrementor", self.expression()?.into()));
            }
            "ForStatement"
        };
        self.expect(")")?;
        fields.push(("statement", self.statement()?.into()));
        Ok(self.finish(kind, start, fields))
    }
    fn switch_statement(&mut self) -> Result<NodeId> {
        let start = self.pos();
        self.bump()?;
        let expression = self.parenthesized_expression()?;
        let block_start = self.pos();
        self.expect("{")?;
        let mut clauses = vec![];
        while !self.at("}") {
            let clause_start = self.pos();
            let mut fields: Fields<'src> = Fields::new();
            let kind = if self.eat("case")? {
                fields.push(("expression", self.expression()?.into()));
                "CaseClause"
            } else {
                self.expect("default")?;
                "DefaultClause"
            };
            self.expect(":")?;
            let mut statements = vec![];
            while !matches!(self.current.raw, "case" | "default" | "}")
                && self.current.kind != Kind::Eof
            {
                statements.push(self.statement()?);
            }
            fields.push(("statements", statements.into()));
            clauses.push(self.finish(kind, clause_start, fields));
        }
        self.expect("}")?;
        let block = self.finish(
            "CaseBlock",
            block_start,
            fields![("clauses", clauses.into())],
        );
        Ok(self.finish(
            "SwitchStatement",
            start,
            fields![
                ("expression", expression.into()),
                ("caseBlock", block.into()),
                ("possiblyExhaustive", false.into()),
            ],
        ))
    }
    fn try_statement(&mut self) -> Result<NodeId> {
        let start = self.pos();
        self.bump()?;
        let mut fields = fields![("tryBlock", self.block("Block")?.into())];
        if self.at("catch") {
            let catch_start = self.pos();
            self.bump()?;
            let mut catch_fields = vec![];
            if self.eat("(")? {
                catch_fields.push(("variableDeclaration", self.variable_declaration()?.into()));
                self.expect(")")?;
            }
            catch_fields.push(("block", self.block("Block")?.into()));
            fields.push((
                "catchClause",
                self.finish("CatchClause", catch_start, catch_fields).into(),
            ));
        }
        if self.eat("finally")? {
            fields.push(("finallyBlock", self.block("Block")?.into()));
        }
        if fields.len() == 1 {
            return Err(self.error("Expected catch or finally"));
        }
        Ok(self.finish("TryStatement", start, fields))
    }
    fn variable_statement(&mut self, start: usize, modifiers: Vec<NodeId>) -> Result<NodeId> {
        let list = self.variable_list()?;
        self.semi()?;
        let id = self.finish(
            "VariableStatement",
            start,
            fields![("declarationList", list.into())],
        );
        self.attach_modifiers(id, modifiers);
        Ok(id)
    }
    fn variable_list(&mut self) -> Result<NodeId> {
        let start = self.pos();
        let keyword = self.bump()?.raw;
        let flags = match keyword {
            "let" => 1,
            "const" => 2,
            "using" => 4,
            "await" => {
                self.expect("using")?;
                6
            }
            _ => 0,
        };
        let binding_start = matches!(self.current.raw, "{" | "[")
            || self.current.kind == Kind::Word && !reserved_word(self.current.raw);
        let empty = !binding_start
            && (self.current.kind == Kind::Eof
                || matches!(self.current.raw, ";" | "}")
                || self.current.newline
                || !self.context.allow_in && matches!(self.current.raw, "in" | ")"))
            || self.at("of")
                && self.peek().kind == Kind::Word
                && !reserved_word(self.peek().raw)
                && self.peek_second().raw == ")";
        let mut declarations = vec![];
        if !empty {
            declarations.push(self.variable_declaration()?);
        }
        while self.eat(",")? {
            if self.current.kind == Kind::Eof
                || matches!(self.current.raw, ";" | "}" | "in" | "of")
                || self.current.newline
                    && !matches!(self.current.raw, "{" | "[")
                    && (self.current.kind != Kind::Word || reserved_word(self.current.raw))
            {
                break;
            }
            declarations.push(self.variable_declaration()?);
        }
        let id = self.finish(
            "VariableDeclarationList",
            start,
            fields![("declarations", declarations.into())],
        );
        self.nodes[id.0].flags = (self.nodes[id.0].flags & !DISALLOW_IN) | flags;
        Ok(id)
    }
    fn variable_declaration(&mut self) -> Result<NodeId> {
        let start = self.pos();
        let name = self.binding_name()?;
        let mut fields = fields![("name", name.into())];
        self.optional_token("!", "exclamationToken", &mut fields)?;
        if self.eat(":")? {
            fields.push(("type", self.type_node()?.into()));
        }
        if self.eat("=")? {
            fields.push(("initializer", self.assignment_expression()?.into()));
        }
        Ok(self.finish("VariableDeclaration", start, fields))
    }
    pub(super) fn binding_name(&mut self) -> Result<NodeId> {
        self.nest(|p| {
            let start = p.pos();
            let object = p.at("{");
            if !object && !p.at("[") {
                return p.binding_identifier();
            }
            p.bump()?;
            let elements = p.in_context(p.context.with_in(true), |p| {
                let close = if object { "}" } else { "]" };
                let mut elements = vec![];
                while !p.at(close) {
                    let element_start = p.pos();
                    if !object && p.at(",") {
                        elements.push(p.make(
                            "OmittedExpression",
                            element_start,
                            element_start,
                            vec![],
                        ));
                        p.bump()?;
                        continue;
                    }
                    let mut fields: Fields<'src> = Fields::new();
                    p.optional_token("...", "dotDotDotToken", &mut fields)?;
                    let name = if object {
                        let property = p.property_name()?;
                        if p.eat(":")? {
                            fields.push(("propertyName", property.into()));
                            p.binding_name()?
                        } else {
                            if p.nodes[property.0].kind != "Identifier"
                                || matches!(p.nodes[property.0].field("escapedText"), Some(Value::Text(text)) if reserved_word(text))
                            { return Err(p.error("Expected ':' and a binding name after property name")); }
                            property
                        }
                    } else {
                        p.binding_name()?
                    };
                    fields.push(("name", name.into()));
                    if p.eat("=")? {
                        fields.push(("initializer", p.assignment_expression()?.into()));
                    }
                    elements.push(p.finish("BindingElement", element_start, fields));
                    if !p.eat(",")? {
                        break;
                    }
                }
                p.expect(close)?;
                Ok(elements)
            })?;
            Ok(p.finish(
                if object {
                    "ObjectBindingPattern"
                } else {
                    "ArrayBindingPattern"
                },
                start,
                fields![("elements", elements.into())],
            ))
        })
    }
    pub(super) fn parameters(&mut self) -> Result<Vec<NodeId>> {
        self.parameters_with_context(false, false)
    }
    pub(super) fn parameters_with_context(
        &mut self,
        async_function: bool,
        generator: bool,
    ) -> Result<Vec<NodeId>> {
        let outer_await = self.context.contains(AWAIT);
        self.in_context(self.context.parameters(async_function, generator), |p| {
            p.parameter_list("(", ")", outer_await)
        })
    }
    pub(super) fn parameter_list(
        &mut self,
        open: &str,
        close: &str,
        outer_await: bool,
    ) -> Result<Vec<NodeId>> {
        self.expect(open)?;
        let mut parameters = vec![];
        while !self.at(close) {
            if self.type_probe {
                if self.current.kind == Kind::Eof
                    || self.current.raw.starts_with('>')
                    || matches!(self.current.raw, ";" | "}")
                {
                    break;
                }
                if !matches!(self.current.kind, Kind::Word)
                    && !matches!(self.current.raw, "[" | "{" | "..." | "@")
                {
                    self.type_probe_diagnostic("Expected a parameter");
                    self.bump()?;
                    continue;
                }
            }
            let start = self.pos();
            let deprecated = self.deprecated_trivia(true);
            let modifiers = self.in_context(
                self.context
                    .without(AWAIT)
                    .with(if outer_await { AWAIT } else { 0 }),
                |p| p.modifiers(true),
            )?;
            let mut fields: Fields<'src> = Fields::new();
            self.optional_token("...", "dotDotDotToken", &mut fields)?;
            let name = if self.at("this") {
                if !modifiers.is_empty() {
                    return Err(self.error("Modifiers cannot be applied to a 'this' parameter"));
                }
                self.identifier()?
            } else {
                self.binding_name()?
            };
            fields.push(("name", name.into()));
            self.optional_token("?", "questionToken", &mut fields)?;
            if self.eat(":")? {
                fields.push(("type", self.type_node()?.into()));
            }
            if self.eat("=")? {
                fields.push(("initializer", self.assignment_expression()?.into()));
            }
            let id = self.finish("Parameter", start, fields);
            if deprecated {
                self.nodes[id.0].flags |= 536870912;
            }
            self.attach_modifiers(id, modifiers);
            parameters.push(id);
            if !self.eat(",")? {
                if self.type_probe
                    && !self.at(close)
                    && !self.current.raw.starts_with('>')
                    && self.current.kind != Kind::Eof
                    && !matches!(self.current.raw, ";" | "}")
                {
                    self.type_probe_diagnostic("Expected ','");
                    continue;
                }
                break;
            }
        }
        self.type_delimiter(close)?;
        Ok(parameters)
    }
    pub(super) fn function(
        &mut self,
        start: usize,
        kind: &'static str,
        modifiers: Vec<NodeId>,
    ) -> Result<NodeId> {
        let context = if kind == "FunctionExpression" {
            self.context.without(DECORATOR)
        } else {
            self.context
        };
        let fields = self.in_context(context, |p| {
            p.expect("function")?;
            let mut fields: Fields<'src> = Fields::new();
            let generator = p.optional_token("*", "asteriskToken", &mut fields)?;
            let async_function = modifiers
                .iter()
                .any(|id| p.nodes[id.0].kind == "AsyncKeyword");
            if p.current.kind == Kind::Word {
                let context = if kind == "FunctionExpression" {
                    p.context.with(
                        if async_function { AWAIT } else { 0 } | if generator { YIELD } else { 0 },
                    )
                } else {
                    p.context
                };
                let name = p.in_context(context, Self::binding_identifier)?;
                fields.push(("name", name.into()));
            } else if kind == "FunctionDeclaration"
                && !modifiers
                    .iter()
                    .any(|id| p.nodes[id.0].kind == "DefaultKeyword")
            {
                return Err(p.error("Expected function name"));
            }
            if p.at("<") {
                fields.push(("typeParameters", p.type_parameters()?.into()));
            }
            if kind == "FunctionDeclaration"
                && modifiers
                    .iter()
                    .any(|id| p.nodes[id.0].kind == "ExportKeyword")
            {
                p.context = p.context.with(AWAIT);
            }
            fields.push((
                "parameters",
                p.parameters_with_context(async_function, generator)?.into(),
            ));
            if p.eat(":")? {
                fields.push(("type", p.return_type()?.into()));
            }
            if p.at("{") {
                let body = p.in_context(p.context.function(async_function, generator), |p| {
                    p.block("Block")
                })?;
                fields.push(("body", body.into()));
            } else if kind == "FunctionExpression" {
                return Err(p.error("Expected function body"));
            } else {
                p.semi()?;
            }
            Ok(fields)
        })?;
        let id = self.finish(kind, start, fields);
        self.attach_modifiers(id, modifiers);
        Ok(id)
    }
    pub(super) fn class(
        &mut self,
        start: usize,
        kind: &'static str,
        modifiers: Vec<NodeId>,
    ) -> Result<NodeId> {
        self.expect("class")?;
        let mut fields: Fields<'src> = Fields::new();
        if self.current.kind == Kind::Word
            && !self.at("extends")
            && !(self.at("implements") && self.peek().kind == Kind::Word)
        {
            fields.push(("name", self.binding_identifier()?.into()));
        }
        if self.at("<") {
            fields.push(("typeParameters", self.type_parameters()?.into()));
        }
        let context = if modifiers
            .iter()
            .any(|id| self.nodes[id.0].kind == "ExportKeyword")
        {
            self.context.with(AWAIT)
        } else {
            self.context
        };
        self.in_context(context, |p| {
            let heritage = p.heritage()?;
            if !heritage.is_empty() {
                fields.push(("heritageClauses", heritage.into()));
            }
            p.expect("{")?;
            let mut members = vec![];
            while !p.at("}") {
                if p.current.kind == Kind::Eof {
                    return Err(p.error("Unterminated class"));
                }
                if p.at(";") {
                    let pos = p.pos();
                    p.bump()?;
                    members.push(p.finish("SemicolonClassElement", pos, vec![]));
                    continue;
                }
                members.push(p.member(false)?);
            }
            p.expect("}")?;
            fields.push(("members", members.into()));
            Ok(())
        })?;
        let id = self.finish(kind, start, fields);
        self.attach_modifiers(id, modifiers);
        Ok(id)
    }
    fn heritage(&mut self) -> Result<Vec<NodeId>> {
        let mut clauses = vec![];
        while self.at("extends") || self.at("implements") {
            let start = self.pos();
            let token = self.bump()?;
            let mut types = vec![];
            while self.heritage_element_ahead() {
                let type_start = self.pos();
                let expression = self.postfix_expression(false)?;
                // Type arguments are represented directly on heritage expressions.
                let mut fields = fields![("expression", expression.into())];
                if self.nodes[expression.0].kind == "ExpressionWithTypeArguments" {
                    fields = self.nodes[expression.0].fields.clone();
                } else if self.type_open() {
                    fields.push(("typeArguments", self.type_arguments()?.into()));
                }
                types.push(self.finish("ExpressionWithTypeArguments", type_start, fields));
                if !self.eat(",")? {
                    break;
                }
            }
            clauses.push(self.finish(
                "HeritageClause",
                start,
                fields![
                    (
                        "token",
                        Value::Number(
                            crate::syntax_kind::number(token_kind(token.raw, token.kind)) as u32,
                        ),
                    ),
                    ("types", types.into()),
                ],
            ));
        }
        Ok(clauses)
    }
    fn heritage_element_ahead(&self) -> bool {
        if matches!(self.current.raw, "extends" | "implements") {
            return false;
        }
        if self.at("{") && self.peek_at("}") {
            return matches!(self.peek_second().raw, "," | "{" | "extends" | "implements");
        }
        self.current.kind != Kind::Eof && !matches!(self.current.raw, "}" | ";")
    }
    fn interface(&mut self, start: usize, modifiers: Vec<NodeId>) -> Result<NodeId> {
        self.bump()?;
        let name = self.binding_identifier()?;
        let mut fields = fields![("name", name.into())];
        if self.at("<") {
            fields.push(("typeParameters", self.type_parameters()?.into()));
        }
        let heritage = self.heritage()?;
        if !heritage.is_empty() {
            fields.push(("heritageClauses", heritage.into()));
        }
        fields.push(("members", self.type_members()?.into()));
        let id = self.finish("InterfaceDeclaration", start, fields);
        self.attach_modifiers(id, modifiers);
        Ok(id)
    }
    fn type_alias(&mut self, start: usize, modifiers: Vec<NodeId>) -> Result<NodeId> {
        self.bump()?;
        if self.current.newline {
            return Err(self.error("Line break is not permitted after 'type'"));
        }
        let name = self.binding_identifier()?;
        let mut fields = fields![("name", name.into())];
        if self.at("<") {
            fields.push(("typeParameters", self.type_parameters()?.into()));
        }
        self.expect("=")?;
        let type_node = if self.at("intrinsic") && !self.peek_at(".") {
            self.token_node()?
        } else {
            self.type_node()?
        };
        fields.push(("type", type_node.into()));
        self.semi()?;
        let id = self.finish("TypeAliasDeclaration", start, fields);
        self.attach_modifiers(id, modifiers);
        Ok(id)
    }
    fn enumeration(&mut self, start: usize, modifiers: Vec<NodeId>) -> Result<NodeId> {
        self.expect("enum")?;
        let name = self.binding_identifier()?;
        self.expect("{")?;
        let members = self.in_context(self.context.without(AWAIT | YIELD), |p| {
            let mut members = vec![];
            while !p.at("}") {
                let member_start = p.pos();
                let deprecated = p.deprecated_trivia(false);
                let name = p.property_name()?;
                let mut fields = fields![("name", name.into())];
                if p.eat("=")? {
                    fields.push(("initializer", p.assignment_expression()?.into()));
                }
                let member = p.finish("EnumMember", member_start, fields);
                if deprecated {
                    p.nodes[member.0].flags |= 536870912;
                }
                members.push(member);
                if !p.eat(",")? {
                    break;
                }
            }
            p.expect("}")?;
            Ok(members)
        })?;
        let id = self.finish(
            "EnumDeclaration",
            start,
            fields![("name", name.into()), ("members", members.into())],
        );
        self.attach_modifiers(id, modifiers);
        Ok(id)
    }
    fn namespace(&mut self, start: usize, modifiers: Vec<NodeId>) -> Result<NodeId> {
        let token = self.bump()?;
        if token.raw == "module" && self.current.kind != Kind::String {
            return Err(self.error("TypeScript 7 requires namespace for namespace declarations"));
        }
        let global = token.raw == "global";
        let name = if global {
            self.make(
                "Identifier",
                token.full_start,
                token.end,
                fields![("escapedText", token.raw.into())],
            )
        } else if self.current.kind == Kind::String {
            self.literal()?
        } else {
            self.binding_identifier()?
        };
        let mut fields = fields![("name", name.into())];
        if self.eat(".")? {
            let nested_start = self.pos();
            let nested_name = self.binding_identifier()?;
            let body = self.namespace_tail(nested_start, nested_name)?;
            fields.push(("body", body.into()));
        } else if self.at("{") {
            fields.push(("body", self.block("ModuleBlock")?.into()));
        } else {
            self.semi()?;
        }
        let id = self.finish("ModuleDeclaration", start, fields);
        self.nodes[id.0].flags |= if global {
            2048
        } else if token.raw == "namespace" {
            32
        } else {
            0
        };
        self.attach_modifiers(id, modifiers);
        Ok(id)
    }
    fn namespace_tail(&mut self, start: usize, name: NodeId) -> Result<NodeId> {
        self.nest(|p| {
            let body = if p.eat(".")? {
                let pos = p.pos();
                let name = p.binding_identifier()?;
                p.namespace_tail(pos, name)?
            } else {
                p.block("ModuleBlock")?
            };
            let id = p.finish(
                "ModuleDeclaration",
                start,
                fields![("name", name.into()), ("body", body.into())],
            );
            p.nodes[id.0].flags |= 40;
            Ok(id)
        })
    }
    fn import_declaration(&mut self, start: usize, modifiers: Vec<NodeId>) -> Result<NodeId> {
        self.expect("import")?;
        let mut fields: Fields<'src> = Fields::new();
        if self.current.kind == Kind::String {
            fields.push(("moduleSpecifier", self.literal()?.into()));
        } else {
            let clause_start = self.pos();
            let type_only = self.at("type")
                && (!matches!(self.peek().raw, "from" | "=" | ",")
                    || self.peek_at("from") && matches!(self.peek_second().raw, "from" | "="));
            let deferred = self.at("defer")
                && (self.peek_at("from") && self.peek_second().kind != Kind::String
                    || !matches!(self.peek().raw, "from" | "=" | ","));
            if type_only || deferred {
                self.bump()?;
            }
            let mut clause_fields = fields![("isTypeOnly", type_only.into())];
            if type_only || deferred {
                clause_fields.push((
                    "phaseModifier",
                    Value::Number(if type_only { 156 } else { 166 }),
                ));
            }
            if self.current.kind == Kind::Word {
                let name = self.binding_identifier()?;
                if self.eat("=")? {
                    let reference_start = self.pos();
                    let reference = if self.at("require") && self.peek_at("(") {
                        self.bump()?;
                        self.expect("(")?;
                        let expression = self.module_specifier()?;
                        self.expect(")")?;
                        self.finish(
                            "ExternalModuleReference",
                            reference_start,
                            fields![("expression", expression.into())],
                        )
                    } else {
                        self.entity_name()?
                    };
                    self.semi()?;
                    let id = self.finish(
                        "ImportEqualsDeclaration",
                        start,
                        fields![
                            ("isTypeOnly", type_only.into()),
                            ("name", name.into()),
                            ("moduleReference", reference.into()),
                        ],
                    );
                    self.attach_modifiers(id, modifiers);
                    return Ok(id);
                }
                clause_fields.push(("name", name.into()));
                if !self.at("from") {
                    self.expect(",")?;
                }
            }
            if self.at("*") {
                let namespace_start = self.pos();
                self.bump()?;
                self.expect("as")?;
                let name = self.binding_identifier()?;
                clause_fields.push((
                    "namedBindings",
                    self.finish(
                        "NamespaceImport",
                        namespace_start,
                        fields![("name", name.into())],
                    )
                    .into(),
                ));
            } else if self.at("{") {
                let named_start = self.pos();
                let elements = self.specifiers(false)?;
                clause_fields.push((
                    "namedBindings",
                    self.finish(
                        "NamedImports",
                        named_start,
                        fields![("elements", elements.into())],
                    )
                    .into(),
                ));
            }
            fields.push((
                "importClause",
                self.finish("ImportClause", clause_start, clause_fields)
                    .into(),
            ));
            self.expect("from")?;
            fields.push(("moduleSpecifier", self.module_specifier()?.into()));
        }
        if self.at("with") {
            let attributes = self.import_attributes()?;
            fields.push(("attributes", attributes.into()));
            fields.push(("assertClause", attributes.into()));
        }
        self.semi()?;
        let id = self.finish("ImportDeclaration", start, fields);
        self.attach_modifiers(id, modifiers);
        Ok(id)
    }
    fn export_statement(&mut self) -> Result<NodeId> {
        let start = self.pos();
        let export = self.token_node()?;
        if self.eat("=")? {
            let expression =
                self.in_context(self.context.with(AWAIT), Self::assignment_expression)?;
            self.semi()?;
            return Ok(self.finish(
                "ExportAssignment",
                start,
                fields![
                    ("isExportEquals", true.into()),
                    ("expression", expression.into()),
                ],
            ));
        }
        if self.at("as") {
            self.bump()?;
            self.expect("namespace")?;
            let name = self.identifier()?;
            self.semi()?;
            return Ok(self.finish(
                "NamespaceExportDeclaration",
                start,
                fields![("name", name.into())],
            ));
        }
        let type_only = self.at("type") && matches!(self.peek().raw, "{" | "*");
        if type_only {
            self.bump()?;
        }
        if self.at("{") || self.at("*") {
            let fields = self.in_context(self.context.with(AWAIT), |p| {
                let mut fields = fields![("isTypeOnly", type_only.into())];
                if p.at("{") {
                    let pos = p.pos();
                    let elements = p.specifiers(true)?;
                    fields.push((
                        "exportClause",
                        p.finish("NamedExports", pos, fields![("elements", elements.into())])
                            .into(),
                    ));
                } else {
                    let clause_start = p.pos();
                    p.bump()?;
                    if p.eat("as")? {
                        let name = p.module_export_name()?;
                        fields.push((
                            "exportClause",
                            p.finish(
                                "NamespaceExport",
                                clause_start,
                                fields![("name", name.into())],
                            )
                            .into(),
                        ));
                    }
                }
                if p.eat("from")? {
                    fields.push(("moduleSpecifier", p.module_specifier()?.into()));
                }
                if p.at("with") {
                    let attributes = p.import_attributes()?;
                    fields.push(("attributes", attributes.into()));
                    fields.push(("assertClause", attributes.into()));
                }
                p.semi()?;
                Ok(fields)
            })?;
            return Ok(self.finish("ExportDeclaration", start, fields));
        }
        let mut modifiers = vec![export];
        if self.at("default") {
            modifiers.push(self.token_node()?);
            let declaration = matches!(self.current.raw, "class" | "function" | "interface")
                || (self.at("abstract") && self.peek_at("class"))
                || (self.at("async") && self.peek_at("function"))
                || self.at("@");
            if !declaration {
                let expression =
                    self.in_context(self.context.with(AWAIT), Self::assignment_expression)?;
                self.semi()?;
                return Ok(self.finish(
                    "ExportAssignment",
                    start,
                    fields![("expression", expression.into())],
                ));
            }
        }
        modifiers.extend(self.modifiers(false)?);
        self.ambient_modifiers(&modifiers);
        match self.current.raw {
            "export" => {
                let id = self.export_statement()?;
                self.nodes[id.0].span.start = start;
                self.attach_modifiers(id, modifiers);
                Ok(id)
            }
            "function" => self.function(start, "FunctionDeclaration", modifiers),
            "class" => self.class(start, "ClassDeclaration", modifiers),
            "interface" => self.interface(start, modifiers),
            "type" => self.type_alias(start, modifiers),
            "enum" => self.enumeration(start, modifiers),
            "const" if self.peek_at("enum") => {
                modifiers.push(self.token_node()?);
                self.enumeration(start, modifiers)
            }
            "var" | "let" | "const" | "using" | "await" => {
                self.variable_statement(start, modifiers)
            }
            "namespace" | "module" => self.namespace(start, modifiers),
            "import" => self.import_declaration(start, modifiers),
            _ => Err(self.error("Expected exported declaration")),
        }
    }
    fn specifiers(&mut self, export: bool) -> Result<Vec<NodeId>> {
        self.expect("{")?;
        let mut elements = vec![];
        while !self.at("}") {
            let start = self.pos();
            let mut invalid_name = self.import_name_is_keyword();
            let mut name = self.module_export_name()?;
            let mut property = None;
            let mut type_only = false;
            let mut can_parse_as = true;
            if self.nodes[name.0].kind == "Identifier"
                && matches!(self.nodes[name.0].field("escapedText"), Some(Value::Text(text)) if text == "type")
            {
                if self.at("as") {
                    let first_as = self.identifier()?;
                    if self.at("as") {
                        let second_as = self.identifier()?;
                        if matches!(self.current.kind, Kind::Word | Kind::String) {
                            type_only = true;
                            property = Some(first_as);
                            invalid_name = self.import_name_is_keyword();
                            name = self.module_export_name()?;
                        } else {
                            property = Some(name);
                            name = second_as;
                            invalid_name = false;
                        }
                        can_parse_as = false;
                    } else if matches!(self.current.kind, Kind::Word | Kind::String) {
                        property = Some(name);
                        invalid_name = self.import_name_is_keyword();
                        name = self.module_export_name()?;
                        can_parse_as = false;
                    } else {
                        type_only = true;
                        name = first_as;
                        invalid_name = false;
                    }
                } else if matches!(self.current.kind, Kind::Word | Kind::String) {
                    type_only = true;
                    invalid_name = self.import_name_is_keyword();
                    name = self.module_export_name()?;
                }
            }
            if can_parse_as && self.eat("as")? {
                property = Some(name);
                invalid_name = self.import_name_is_keyword();
                name = self.module_export_name()?;
            }
            if !export && (self.nodes[name.0].kind != "Identifier" || invalid_name) {
                return Err(self.error("Expected an import binding identifier"));
            }
            let mut fields = fields![("isTypeOnly", type_only.into())];
            if let Some(property) = property {
                fields.push(("propertyName", property.into()));
            }
            fields.push(("name", name.into()));
            elements.push(self.finish(
                if export {
                    "ExportSpecifier"
                } else {
                    "ImportSpecifier"
                },
                start,
                fields,
            ));
            if !self.eat(",")? {
                break;
            }
        }
        self.expect("}")?;
        Ok(elements)
    }
    fn module_export_name(&mut self) -> Result<NodeId> {
        if self.current.kind == Kind::String {
            self.literal()
        } else {
            self.identifier()
        }
    }
    fn import_name_is_keyword(&self) -> bool {
        reserved_word(self.current.raw)
            || self.at("await") && self.context.contains(AWAIT)
            || self.at("yield") && self.context.contains(YIELD)
    }
    pub(super) fn import_attributes(&mut self) -> Result<NodeId> {
        let start = self.pos();
        self.expect("with")?;
        self.import_attribute_body(start)
    }
    pub(super) fn import_attribute_body(&mut self, start: usize) -> Result<NodeId> {
        self.expect("{")?;
        let mut elements = vec![];
        while !self.at("}") {
            let pos = self.pos();
            let name = if self.current.kind == Kind::String {
                self.literal()?
            } else {
                self.identifier()?
            };
            self.expect(":")?;
            let value = self.assignment_expression()?;
            elements.push(self.finish(
                "ImportAttribute",
                pos,
                fields![("name", name.into()), ("value", value.into())],
            ));
            if !(self.eat(",")? || self.eat(";")?) {
                break;
            }
        }
        self.expect("}")?;
        Ok(self.finish(
            "ImportAttributes",
            start,
            fields![("token", Value::Number(118)), ("elements", elements.into())],
        ))
    }
    fn module_specifier(&mut self) -> Result<NodeId> {
        if self.current.kind == Kind::String {
            self.literal()
        } else {
            self.expression()
        }
    }
}
