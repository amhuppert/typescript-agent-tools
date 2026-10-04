use super::*;

impl<'src> Parser<'src> {
    pub(super) fn type_node(&mut self) -> Result<NodeId> {
        let saved = self.context;
        self.context &= !81920;
        let result = self.nest(|p| p.conditional_type(p.context & 131072 == 0));
        self.context = saved;
        result
    }
    fn conditional_type(&mut self, conditional: bool) -> Result<NodeId> {
        if self.function_type_ahead() {
            if let Some(node) = self.attempt(Self::function_type) {
                return Ok(node);
            }
        }
        let saved = self.allow_conditional;
        self.allow_conditional = conditional;
        let check = self.union_type()?;
        self.allow_conditional = saved;
        if conditional && !self.current.newline && self.eat("extends")? {
            let context = self.context;
            self.context |= 131072;
            let extends = self.nest(|p| p.conditional_type(false))?;
            self.context = context;
            self.expect("?")?;
            let yes = self.type_node()?;
            self.expect(":")?;
            let no = self.type_node()?;
            Ok(self.finish(
                "ConditionalType",
                self.start(check),
                fields![
                    ("checkType", check.into()),
                    ("extendsType", extends.into()),
                    ("trueType", yes.into()),
                    ("falseType", no.into()),
                ],
            ))
        } else {
            Ok(check)
        }
    }
    fn union_type(&mut self) -> Result<NodeId> {
        let start = self.pos();
        let leading = self.eat("|")?;
        let first = self.intersection_type()?;
        if !leading && !self.at("|") {
            return Ok(first);
        }
        let mut types = vec![first];
        if leading && matches!(self.nodes[first.0].kind, "FunctionType" | "ConstructorType") {
            return Err(self.error("Function or constructor type in a union must be parenthesized"));
        }
        while self.eat("|")? {
            let node = self.intersection_type()?;
            if matches!(self.nodes[node.0].kind, "FunctionType" | "ConstructorType") {
                return Err(
                    self.error("Function or constructor type in a union must be parenthesized")
                );
            }
            types.push(node);
        }
        Ok(self.finish("UnionType", start, fields![("types", types.into())]))
    }
    fn intersection_type(&mut self) -> Result<NodeId> {
        let start = self.pos();
        let leading = self.eat("&")?;
        let first = self.type_operator()?;
        if !leading && !self.at("&") {
            return Ok(first);
        }
        let mut types = vec![first];
        if leading && matches!(self.nodes[first.0].kind, "FunctionType" | "ConstructorType") {
            return Err(
                self.error("Function or constructor type in an intersection must be parenthesized")
            );
        }
        while self.eat("&")? {
            let node = self.type_operator()?;
            if matches!(self.nodes[node.0].kind, "FunctionType" | "ConstructorType") {
                return Err(self.error(
                    "Function or constructor type in an intersection must be parenthesized",
                ));
            }
            types.push(node);
        }
        Ok(self.finish("IntersectionType", start, fields![("types", types.into())]))
    }
    fn type_operator(&mut self) -> Result<NodeId> {
        let start = self.pos();
        if matches!(self.current.raw, "keyof" | "unique" | "readonly") {
            let operator = self.bump()?;
            let type_node = self.nest(Self::type_operator)?;
            let number = match operator.raw {
                "keyof" => 143,
                "unique" => 158,
                _ => 148,
            };
            return Ok(self.finish(
                "TypeOperator",
                start,
                fields![
                    ("operator", Value::Number(number)),
                    ("type", type_node.into()),
                ],
            ));
        }
        if self.eat("infer")? {
            let parameter_start = self.pos();
            let name = self.identifier()?;
            let mut fields = fields![("name", name.into())];
            if self.at("extends") {
                let disallowed = !self.allow_conditional;
                if let Some(constraint) = self.attempt(|p| {
                    p.bump()?;
                    let saved = p.context;
                    p.context = (p.context & !81920) | 131072;
                    let node = p.nest(|p| p.conditional_type(false))?;
                    p.context = saved;
                    if p.at("?") && !disallowed {
                        Err(p.error("Conditional type boundary"))
                    } else {
                        Ok(node)
                    }
                }) {
                    fields.push(("constraint", constraint.into()));
                }
            }
            let parameter = self.finish("TypeParameter", parameter_start, fields);
            return Ok(self.finish(
                "InferType",
                start,
                fields![("typeParameter", parameter.into())],
            ));
        }
        let context = self.context;
        self.context &= !131072;
        let result = self.postfix_type();
        self.context = context;
        result
    }
    fn postfix_type(&mut self) -> Result<NodeId> {
        let mut node = self.primary_type()?;
        while !self.current.newline {
            if self.at("!") || self.at("?") && !self.type_start(self.peek()) {
                let kind = if self.at("!") {
                    "JSDocNonNullableType"
                } else {
                    "JSDocNullableType"
                };
                self.bump()?;
                node = self.finish(
                    kind,
                    self.start(node),
                    fields![("type", node.into()), ("postfix", true.into())],
                );
                continue;
            }
            if !self.at("[") {
                break;
            }
            self.bump()?;
            if self.eat("]")? {
                node = self.finish(
                    "ArrayType",
                    self.start(node),
                    fields![("elementType", node.into())],
                );
                self.nodes[node.0].flags &= !131072;
            } else {
                let index = self.type_node()?;
                self.expect("]")?;
                node = self.finish(
                    "IndexedAccessType",
                    self.start(node),
                    fields![("objectType", node.into()), ("indexType", index.into())],
                );
                self.nodes[node.0].flags &= !131072;
            }
        }
        Ok(node)
    }
    fn type_start(&self, token: Token<'_>) -> bool {
        matches!(
            token.kind,
            Kind::String | Kind::Number | Kind::NoSubstitutionTemplate | Kind::TemplateHead
        ) || token.kind == Kind::Word
            && (!reserved_word(token.raw)
                || matches!(
                    token.raw,
                    "void"
                        | "null"
                        | "this"
                        | "typeof"
                        | "new"
                        | "true"
                        | "false"
                        | "import"
                        | "function"
                ))
            || matches!(
                token.raw,
                "{" | "[" | "(" | "<" | "|" | "&" | "*" | "?" | "!" | "..."
            )
            || token.raw == "-" && self.peek_second().kind == Kind::Number
    }
    fn primary_type(&mut self) -> Result<NodeId> {
        self.check_token()?;
        let start = self.pos();
        if self.function_type_ahead() {
            if let Some(node) = self.attempt(Self::function_type) {
                return Ok(node);
            }
        }
        let context = self.context;
        self.context &= !131072;
        let result = self.primary_type_inner(start);
        self.context = context;
        result
    }
    fn function_type_ahead(&self) -> bool {
        if self.at("<") || self.at("new") || self.at("abstract") && self.peek_at("new") {
            return true;
        }
        if !self.at("(") {
            return false;
        }
        let mut lexer = self.lexer;
        let mut token = lexer.next();
        if matches!(token.raw, ")" | "...") {
            return true;
        }
        while matches!(
            token.raw,
            "public"
                | "private"
                | "protected"
                | "readonly"
                | "static"
                | "abstract"
                | "declare"
                | "async"
                | "override"
                | "in"
                | "out"
                | "const"
                | "accessor"
        ) {
            let next = lexer.next();
            if matches!(next.raw, ":" | "," | "?" | "=" | ")") {
                token = next;
                break;
            }
            token = next;
        }
        if matches!(token.raw, "[" | "{") {
            let mut depth = 1usize;
            while depth != 0 {
                token = lexer.next();
                if token.kind == Kind::Eof {
                    return false;
                }
                if matches!(token.raw, "[" | "{") {
                    depth += 1;
                }
                if matches!(token.raw, "]" | "}") {
                    depth -= 1;
                }
            }
            token = lexer.next();
        } else if token.kind == Kind::Word && (!reserved_word(token.raw) || token.raw == "this") {
            token = lexer.next();
        }
        matches!(token.raw, ":" | "," | "?" | "=") || token.raw == ")" && lexer.next().raw == "=>"
    }
    fn primary_type_inner(&mut self, start: usize) -> Result<NodeId> {
        if self.at("*") || self.at("*=") {
            if self.at("*=") {
                self.split_type_token();
            }
            self.bump()?;
            return Ok(self.finish("JSDocAllType", start, vec![]));
        }
        if self.at("!") {
            self.bump()?;
            let type_node = self.nest(Self::primary_type)?;
            return Ok(self.finish(
                "JSDocNonNullableType",
                start,
                fields![("type", type_node.into()), ("postfix", false.into())],
            ));
        }
        if self.at("?") || self.at("??") {
            if self.at("??") {
                self.split_type_token();
            }
            self.bump()?;
            if self.type_close() || matches!(self.current.raw, "," | "}" | ")" | "=" | "|") {
                return Err(self.error("Expected a type after '?'"));
            }
            let type_node = self.type_node()?;
            return Ok(self.finish(
                "JSDocNullableType",
                start,
                fields![("type", type_node.into()), ("postfix", false.into())],
            ));
        }
        if self.at("function") && self.peek_at("(") {
            return Err(self.error("Use an arrow signature for a function type"));
        }
        if self.at("this") {
            self.bump()?;
            let name = self.finish("ThisType", start, vec![]);
            if !self.current.newline && self.eat("is")? {
                let type_node = self.type_node()?;
                return Ok(self.finish(
                    "TypePredicate",
                    start,
                    fields![("parameterName", name.into()), ("type", type_node.into())],
                ));
            }
            return Ok(name);
        }
        if self.at("asserts") && self.peek().kind == Kind::Word && !self.peek().newline {
            let mut fields = fields![("assertsModifier", self.token_node()?.into())];
            let name = if self.at("this") {
                let start = self.pos();
                self.bump()?;
                self.finish("ThisType", start, vec![])
            } else {
                self.identifier()?
            };
            fields.push(("parameterName", name.into()));
            if self.eat("is")? {
                fields.push(("type", self.type_node()?.into()));
            }
            return Ok(self.finish("TypePredicate", start, fields));
        }
        if self.eat("(")? {
            let type_node = self.type_node()?;
            self.expect(")")?;
            let id = self.finish(
                "ParenthesizedType",
                start,
                fields![("type", type_node.into())],
            );
            self.nodes[id.0].flags &= !131072;
            return Ok(id);
        }
        if self.at("{") {
            if let Some(mapped) = self.attempt(Self::mapped_type) {
                return Ok(mapped);
            }
            let members = self.type_members()?;
            return Ok(self.finish("TypeLiteral", start, fields![("members", members.into())]));
        }
        if self.eat("[")? {
            let mut elements = vec![];
            while !self.at("]") {
                let pos = self.pos();
                let mut fields: Fields<'src> = Fields::new();
                let labelled = self.tuple_label_ahead();
                let element = if labelled {
                    self.optional_token("...", "dotDotDotToken", &mut fields)?;
                    fields.push(("name", self.identifier()?.into()));
                    self.optional_token("?", "questionToken", &mut fields)?;
                    self.expect(":")?;
                    fields.push(("type", self.tuple_element_type()?.into()));
                    self.finish("NamedTupleMember", pos, fields)
                } else {
                    self.tuple_element_type()?
                };
                elements.push(element);
                if !self.eat(",")? {
                    break;
                }
            }
            self.expect("]")?;
            return Ok(self.finish("TupleType", start, fields![("elements", elements.into())]));
        }
        if self.at("typeof") {
            self.bump()?;
            if self.at("import") {
                return self.import_type(start, true);
            }
            let name = self.entity_name()?;
            let mut fields = fields![("exprName", name.into())];
            if !self.current.newline && self.at("<") {
                fields.push(("typeArguments", self.type_arguments()?.into()));
            }
            return Ok(self.finish("TypeQuery", start, fields));
        }
        if self.at("import") {
            return self.import_type(start, false);
        }
        if matches!(
            self.current.kind,
            Kind::TemplateHead | Kind::NoSubstitutionTemplate
        ) {
            return self.template(true);
        }
        if matches!(self.current.kind, Kind::String | Kind::Number)
            || matches!(self.current.raw, "true" | "false" | "null" | "-")
        {
            let literal = if self.eat("-")? {
                let operand = self.literal()?;
                self.finish(
                    "PrefixUnaryExpression",
                    start,
                    fields![("operator", Value::Number(41)), ("operand", operand.into())],
                )
            } else if matches!(self.current.raw, "true" | "false" | "null") {
                self.token_node()?
            } else {
                self.literal()?
            };
            return Ok(self.finish("LiteralType", start, fields![("literal", literal.into())]));
        }
        let keyword = match self.current.raw {
            "any" => Some("AnyKeyword"),
            "unknown" => Some("UnknownKeyword"),
            "number" => Some("NumberKeyword"),
            "object" => Some("ObjectKeyword"),
            "boolean" => Some("BooleanKeyword"),
            "bigint" => Some("BigIntKeyword"),
            "string" => Some("StringKeyword"),
            "symbol" => Some("SymbolKeyword"),
            "void" => Some("VoidKeyword"),
            "undefined" => Some("UndefinedKeyword"),
            "never" => Some("NeverKeyword"),
            _ => None,
        };
        if let Some(kind) = keyword {
            if kind == "VoidKeyword" || !self.peek_at(".") {
                self.bump()?;
                return Ok(self.finish(kind, start, vec![]));
            }
        }
        let name = match self.entity_name() {
            Ok(name) => name,
            Err(error) if self.type_probe => {
                if self.type_probe_error.is_none() {
                    self.type_probe_error = Some(error);
                }
                self.make(
                    "Identifier",
                    self.pos(),
                    self.pos(),
                    fields![("escapedText", "".into())],
                )
            }
            Err(error) => return Err(error),
        };
        let mut fields = fields![("typeName", name.into())];
        if !self.current.newline && self.type_open() {
            fields.push(("typeArguments", self.type_arguments()?.into()));
        }
        Ok(self.finish("TypeReference", start, fields))
    }
    fn split_type_token(&mut self) {
        self.current.end = self.current.start + 1;
        self.current.raw = &self.lexer.src[self.current.start..self.current.end];
        self.lexer.pos = self.current.end;
    }
    fn tuple_label_ahead(&self) -> bool {
        let mut lexer = self.lexer;
        let name = if self.at("...") {
            lexer.next()
        } else {
            self.current
        };
        if name.kind != Kind::Word {
            return false;
        }
        let next = lexer.next();
        next.raw == ":" || next.raw == "?" && lexer.next().raw == ":"
    }
    fn tuple_element_type(&mut self) -> Result<NodeId> {
        let start = self.pos();
        if self.eat("...")? {
            let type_node = self.type_node()?;
            return Ok(self.finish("RestType", start, fields![("type", type_node.into())]));
        }
        let id = self.type_node()?;
        if self.nodes[id.0].kind == "JSDocNullableType"
            && matches!(self.nodes[id.0].field("postfix"), Some(Value::Bool(true)))
        {
            self.nodes[id.0].kind = "OptionalType";
            self.nodes[id.0].fields.retain(|(key, _)| *key != "postfix");
        }
        Ok(id)
    }
    fn function_type(&mut self) -> Result<NodeId> {
        self.nest(|p| {
            let start = p.pos();
            let mut modifiers = vec![];
            if p.at("abstract") {
                modifiers.push(p.token_node()?);
            }
            let constructor = p.eat("new")?;
            let mut fields: Fields<'src> = Fields::new();
            if p.at("<") {
                fields.push(("typeParameters", p.type_parameters()?.into()));
            }
            fields.push(("parameters", p.parameters()?.into()));
            p.type_delimiter("=>")?;
            fields.push(("type", p.return_type()?.into()));
            let id = p.finish(
                if constructor {
                    "ConstructorType"
                } else {
                    "FunctionType"
                },
                start,
                fields,
            );
            p.attach_modifiers(id, modifiers);
            Ok(id)
        })
    }
    pub(super) fn return_type(&mut self) -> Result<NodeId> {
        let saved = self.context;
        self.context &= !131072;
        let result = self.return_type_inner();
        self.context = saved;
        result
    }
    fn return_type_inner(&mut self) -> Result<NodeId> {
        let start = self.pos();
        let mut fields: Fields<'src> = Fields::new();
        if self.current.kind == Kind::Word
            && self.peek_at("is")
            && !self.peek().newline
            && !reserved_word(self.current.raw)
            && !(self.at("await") && self.context & 65536 != 0
                || self.at("yield") && self.context & 16384 != 0)
        {
            let name = self.identifier()?;
            fields.push(("parameterName", name.into()));
            if self.eat("is")? {
                fields.push(("type", self.type_node()?.into()));
            }
            return Ok(self.finish("TypePredicate", start, fields));
        }
        self.type_node()
    }
    pub(super) fn entity_name(&mut self) -> Result<NodeId> {
        if self.current.kind != Kind::Word {
            return Err(self.error("Expected a type or namespace identifier"));
        }
        let mut name = self.identifier()?;
        while self.eat(".")? {
            if self.type_open() {
                break;
            }
            if self.current.kind != Kind::Word {
                return Err(self.error("Expected a qualified identifier"));
            }
            let right = self.identifier()?;
            name = self.finish(
                "QualifiedName",
                self.start(name),
                fields![("left", name.into()), ("right", right.into())],
            );
        }
        Ok(name)
    }
    pub(super) fn type_arguments(&mut self) -> Result<Vec<NodeId>> {
        if self.at("<<") {
            self.current.end = self.current.start + 1;
            self.current.raw = &self.lexer.src[self.current.start..self.current.end];
            self.lexer.pos = self.current.end;
        }
        self.expect("<")?;
        let mut arguments = vec![];
        if !self.type_close() {
            arguments.push(self.type_node()?);
        }
        while self.eat(",")? {
            if self.type_close() {
                break;
            }
            arguments.push(self.type_node()?);
        }
        if self.type_probe && !self.type_close() {
            return Err(self.error("Expected '>'"));
        }
        self.expect_type_close()?;
        Ok(arguments)
    }
    pub(super) fn type_open(&self) -> bool {
        self.at("<") || self.at("<<")
    }
    pub(super) fn type_parameters(&mut self) -> Result<Vec<NodeId>> {
        self.expect("<")?;
        let mut parameters = vec![];
        while !self.type_close() {
            if self.type_probe
                && (self.current.kind == Kind::Eof
                    || matches!(self.current.raw, "(" | "{" | "extends" | "implements"))
            {
                break;
            }
            let start = self.pos();
            let modifiers = self.modifiers_inner(true, true)?;
            let name = self.binding_identifier()?;
            let mut fields = fields![("name", name.into())];
            if self.eat("extends")? {
                fields.push(("constraint", self.type_node()?.into()));
            }
            if self.eat("=")? {
                fields.push(("default", self.type_node()?.into()));
            }
            let id = self.finish("TypeParameter", start, fields);
            self.attach_modifiers(id, modifiers);
            parameters.push(id);
            if !self.eat(",")? || self.type_close() {
                break;
            }
        }
        self.expect_type_close()?;
        Ok(parameters)
    }
    fn type_close(&self) -> bool {
        self.current.raw.starts_with('>')
    }
    pub(super) fn expect_type_close(&mut self) -> Result<()> {
        if self.at(">") {
            self.bump()?;
            return Ok(());
        }
        if self.type_close() {
            self.end = self.current.start + 1;
            self.current.start += 1;
            self.current.full_start = self.current.start;
            self.current.raw = &self.current.raw[1..];
            self.current.newline = false;
            Ok(())
        } else {
            if self.type_probe {
                self.type_probe_diagnostic("Expected '>'");
                Ok(())
            } else {
                Err(self.error("Expected '>'"))
            }
        }
    }
    fn import_type(&mut self, start: usize, query: bool) -> Result<NodeId> {
        self.source_flags |= 4194304;
        self.expect("import")?;
        self.expect("(")?;
        let argument = self.type_node()?;
        let mut fields = fields![("argument", argument.into()), ("isTypeOf", query.into())];
        if self.eat(",")? {
            self.expect("{")?;
            self.expect("with")?;
            self.expect(":")?;
            let attributes_start = self.pos();
            let attributes = self.import_attribute_body(attributes_start)?;
            fields.push(("attributes", attributes.into()));
            self.eat(",")?;
            self.expect("}")?;
        }
        self.expect(")")?;
        if self.eat(".")? {
            fields.push(("qualifier", self.entity_name()?.into()));
        }
        if self.type_open() {
            fields.push(("typeArguments", self.type_arguments()?.into()));
        }
        Ok(self.finish("ImportType", start, fields))
    }
    fn mapped_type(&mut self) -> Result<NodeId> {
        let start = self.pos();
        self.expect("{")?;
        let mut fields: Fields<'src> = Fields::new();
        if self.at("+") || self.at("-") {
            fields.push(("readonlyToken", self.token_node()?.into()));
            self.expect("readonly")?;
        } else {
            self.optional_token("readonly", "readonlyToken", &mut fields)?;
        }
        self.expect("[")?;
        let parameter_start = self.pos();
        let name = self.identifier()?;
        self.expect("in")?;
        let constraint = self.type_node()?;
        let parameter = self.finish(
            "TypeParameter",
            parameter_start,
            fields![("name", name.into()), ("constraint", constraint.into())],
        );
        fields.push(("typeParameter", parameter.into()));
        if self.eat("as")? {
            fields.push(("nameType", self.type_node()?.into()));
        }
        self.expect("]")?;
        if self.at("+") || self.at("-") {
            fields.push(("questionToken", self.token_node()?.into()));
            self.expect("?")?;
        } else {
            self.optional_token("?", "questionToken", &mut fields)?;
        }
        if self.eat(":")? {
            fields.push(("type", self.type_node()?.into()));
        }
        self.eat(";")?;
        let mut members = vec![];
        while !self.at("}") {
            members.push(self.member(true)?);
        }
        self.expect("}")?;
        fields.push(("members", members.into()));
        Ok(self.finish("MappedType", start, fields))
    }
    pub(super) fn type_members(&mut self) -> Result<Vec<NodeId>> {
        self.expect("{")?;
        let mut members = vec![];
        while !self.at("}") {
            if self.current.kind == Kind::Eof {
                return Err(self.error("Unterminated type literal"));
            }
            if self.eat(";")? || self.eat(",")? {
                continue;
            }
            members.push(self.member(true)?);
        }
        self.expect("}")?;
        Ok(members)
    }
    pub(super) fn member(&mut self, signature: bool) -> Result<NodeId> {
        let context = self.context;
        let deprecated = self.deprecated_trivia(false);
        let result = self.nest(|p| {
            let start = p.pos();
            let modifiers = p.modifiers_inner(true, !signature)?;
            if !signature && p.at("static") && p.peek_at("{") {
                p.bump()?;
                let context = p.context;
                p.context = (context & !16384) | 65536;
                let body = p.block("Block")?;
                p.context = context;
                let id = p.finish(
                    "ClassStaticBlockDeclaration",
                    start,
                    fields![("body", body.into())],
                );
                p.attach_modifiers(id, modifiers);
                return Ok(id);
            }
            if p.index_signature_ahead() {
                    let parameters = p.parameter_list("[", "]", false)?;
                    let mut fields = fields![("parameters", parameters.into())];
                    if p.eat(":")? {
                        fields.push(("type", p.type_node()?.into()));
                    }
                    p.member_semi()?;
                    let id = p.finish("IndexSignature", start, fields);
                    p.attach_modifiers(id, modifiers);
                    return Ok(id);
            }
            if signature
                && (p.at("(") || p.at("<") || (p.at("new") && matches!(p.peek().raw, "(" | "<")))
            {
                let construct = p.eat("new")?;
                let mut fields: Fields<'src> = Fields::new();
                if p.at("<") {
                    fields.push(("typeParameters", p.type_parameters()?.into()));
                }
                fields.push(("parameters", p.parameters()?.into()));
                if p.eat(":")? {
                    fields.push(("type", p.return_type()?.into()));
                }
                p.member_semi()?;
                let id = p.finish(
                    if construct {
                        "ConstructSignature"
                    } else {
                        "CallSignature"
                    },
                    start,
                    fields,
                );
                p.attach_modifiers(id, modifiers);
                return Ok(id);
            }
            let accessor = if matches!(p.current.raw, "get" | "set")
                && (matches!(p.peek().kind, Kind::Word | Kind::Private | Kind::String | Kind::Number)
                    || p.peek_at("[")) {
                Some(p.bump()?.raw)
            } else {
                None
            };
            let generator = p.at("*");
            let constructor_keyword = !signature && accessor.is_none() && !generator && p.at("constructor");
            let quoted_constructor = !signature && accessor.is_none() && !generator
                && p.current.kind == Kind::String && p.peek_at("(")
                && matches!(decode_string(&p.current.raw[1..p.current.raw.len() - 1]), Ok(Value::Text(text)) if text == "constructor");
            let constructor = constructor_keyword || quoted_constructor;
            if accessor.is_none() && !constructor {
                p.ambient_modifiers(&modifiers);
            }
            let mut fields: Fields<'src> = Fields::new();
            p.optional_token("*", "asteriskToken", &mut fields)?;
            let name = p.property_name()?;
            if !constructor {
                fields.push(("name", name.into()));
            }
            if (constructor || accessor.is_some()) && matches!(p.current.raw, "?" | "!") {
                return Err(p.error("Expected constructor or accessor parameters"));
            }
            let optional = p.optional_token("?", "questionToken", &mut fields)?;
            let asserted = !signature && !optional && !p.current.newline
                && p.optional_token("!", "exclamationToken", &mut fields)?;
            let method = generator || constructor || accessor.is_some() || p.at("(") || p.at("<");
            if asserted && method {
                return Err(p.error("Expected a property declaration after '!'"));
            }
            let saved_context = p.context;
            let kind = if method {
                if p.at("<") {
                    fields.push(("typeParameters", p.type_parameters()?.into()));
                }
                let async_function = accessor.is_none() && !constructor && modifiers
                    .iter()
                    .any(|id| p.nodes[id.0].kind == "AsyncKeyword");
                let generator = generator && accessor.is_none() && !constructor;
                let parameters = p.parameters_with_context(async_function, generator)?;
                fields.push(("parameters", parameters.into()));
                if p.eat(":")? {
                    fields.push(("type", p.return_type()?.into()));
                }
                if (!signature || accessor.is_some()) && p.at("{") {
                    p.context = p.function_context(async_function, generator);
                    fields.push(("body", p.block("Block")?.into()));
                } else {
                    p.member_semi()?;
                }
                match accessor {
                    Some("get") => "GetAccessor",
                    Some("set") => "SetAccessor",
                    _ if constructor => "Constructor",
                    _ if signature => "MethodSignature",
                    _ => "MethodDeclaration",
                }
            } else {
                if accessor.is_some() {
                    return Err(p.error("Expected accessor parameters"));
                }
                if p.eat(":")? {
                    fields.push(("type", p.type_node()?.into()));
                }
                if p.eat("=")? {
                    p.context &= !(81920 | 8192);
                    fields.push(("initializer", p.assignment_allow_in()?.into()));
                    p.context = saved_context;
                }
                p.member_semi()?;
                if signature {
                    "PropertySignature"
                } else {
                    "PropertyDeclaration"
                }
            };
            p.context = saved_context;
            let id = p.finish(kind, start, fields);
            p.attach_modifiers(id, modifiers);
            Ok(id)
        });
        self.context = context;
        if deprecated {
            if let Ok(id) = result {
                self.nodes[id.0].flags |= 536870912;
            }
        }
        result
    }
    fn member_semi(&mut self) -> Result<()> {
        if self.eat(",")? { Ok(()) } else { self.semi() }
    }
    fn index_signature_ahead(&self) -> bool {
        if !self.at("[") {
            return false;
        }
        let mut lexer = self.lexer;
        let first = lexer.next();
        if matches!(first.raw, "..." | "]") {
            return true;
        }
        let mut next = lexer.next();
        if matches!(
            first.raw,
            "public"
                | "private"
                | "protected"
                | "readonly"
                | "static"
                | "abstract"
                | "async"
                | "declare"
                | "export"
                | "default"
                | "const"
                | "in"
                | "out"
                | "override"
                | "accessor"
        ) {
            if next.kind == Kind::Word && !reserved_word(next.raw) {
                return true;
            }
        } else if first.kind != Kind::Word
            || reserved_word(first.raw)
            || first.raw == "await" && self.context & 65536 != 0
            || first.raw == "yield" && self.context & 16384 != 0
        {
            return false;
        }
        if matches!(next.raw, ":" | ",") {
            return true;
        }
        if next.raw != "?" {
            return false;
        }
        next = lexer.next();
        matches!(next.raw, ":" | "," | "]")
    }
}
