use crate::lexer::{Kind, Lexer, Token};
use crate::{Ast, Diagnostic, Node, NodeId, ParseOptions, Span, Value};

type Fields<'src> = crate::ast::Fields<'src>;
macro_rules! fields {
    ($($value:tt)*) => {{
        let fields: Fields<'_> = smallvec::smallvec![$($value)*];
        fields
    }};
}

mod context;
mod expressions;
mod jsx;
mod statements;
mod types;

use context::{
    AMBIENT, AWAIT, DECORATOR, DISALLOW_CONDITIONAL_TYPES, DISALLOW_IN, GrammarContext, IN_WITH,
    YIELD,
};

type Result<T> = std::result::Result<T, Diagnostic>;
pub(crate) struct Parser<'src> {
    lexer: Lexer<'src>,
    current: Token<'src>,
    end: usize,
    nodes: Vec<Node<'src>>,
    options: ParseOptions,
    depth: usize,
    context: GrammarContext,
    source_flags: u32,
    type_probe: bool,
    type_probe_error: Option<Diagnostic>,
}

/// A speculative parse owns newly appended nodes. Existing nodes are read-only
/// until it commits; rolling back restores the cursor and every grammar setting.
struct Checkpoint<'src> {
    lexer: Lexer<'src>,
    current: Token<'src>,
    end: usize,
    nodes_len: usize,
    depth: usize,
    context: GrammarContext,
    source_flags: u32,
    type_probe: bool,
    type_probe_error: Option<Diagnostic>,
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str, options: ParseOptions) -> Self {
        let mut lexer = Lexer {
            src: source,
            pos: 0,
        };
        let current = lexer.next();
        let context = GrammarContext::new(options.declaration_file());
        Self {
            lexer,
            current,
            end: 0,
            nodes: Vec::with_capacity((source.len() / 16).min(16_384)),
            options,
            depth: 0,
            context,
            source_flags: 0,
            type_probe: false,
            type_probe_error: None,
        }
    }

    pub fn parse(mut self) -> Result<Ast<'src>> {
        self.validate_pragmas()?;
        let mut statements = Vec::new();
        let mut deferred_error = None;
        let has_await = self.lexer.src.contains("await");
        while self.current.kind != Kind::Eof {
            let parsed = if has_await {
                self.attempt_result(Self::statement)
            } else {
                self.statement()
            };
            let statement = match parsed {
                Ok(statement) => statement,
                Err(error) if has_await && self.module_token_present() => {
                    if deferred_error.is_none() {
                        deferred_error = Some(error);
                    }
                    self.in_context(self.context.with(AWAIT), Self::statement)?
                }
                Err(error) => return Err(error),
            };
            statements.push(statement);
        }
        self.check_token()?;
        let external = self.source_flags & 8388608 != 0
            || statements.iter().any(|id| self.external_statement(*id));
        if let Some(error) = deferred_error.filter(|_| !external) {
            return Err(error);
        }
        if !self.options.declaration_file() && has_await && external {
            statements = self.reparse_top_level_await(statements)?;
        }
        let eof = self.token_node()?;
        let root = self.make(
            "SourceFile",
            0,
            self.lexer.src.len(),
            fields![
                ("statements", statements.into()),
                ("endOfFileToken", eof.into()),
                ("text", self.lexer.src.into()),
                ("languageVersion", Value::Number(99)),
                (
                    "languageVariant",
                    Value::Number(u32::from(self.options.jsx)),
                ),
                (
                    "scriptKind",
                    Value::Number(if self.options.jsx { 4 } else { 3 }),
                ),
                ("isDeclarationFile", self.options.declaration_file().into()),
                ("fileName", self.options.file_name.clone().into()),
                ("parseDiagnostics", Vec::<NodeId>::new().into()),
            ],
        );
        self.nodes[root.0].flags |= self.source_flags;
        let mut utf16_deltas = Vec::new();
        let mut delta = 0;
        if !self.lexer.src.is_ascii() {
            for (byte, c) in self.lexer.src.char_indices() {
                if !c.is_ascii() {
                    delta += c.len_utf8() - c.len_utf16();
                    utf16_deltas.push((byte + c.len_utf8(), delta));
                }
            }
        }
        if self.nodes.capacity() > self.nodes.len() + self.nodes.len() / 2 {
            self.nodes.shrink_to_fit();
        }
        Ok(Ast {
            source: self.lexer.src,
            nodes: self.nodes,
            root,
            utf16_deltas,
        })
    }

    fn external_statement(&self, id: NodeId) -> bool {
        let node = &self.nodes[id.0];
        matches!(
            node.kind,
            "ImportDeclaration"
                | "ImportEqualsDeclaration"
                | "ExportDeclaration"
                | "ExportAssignment"
                | "NamespaceExportDeclaration"
        ) || matches!(node.field("modifiers"), Some(Value::Nodes(ids)) if ids.iter().any(|id| self.nodes[id.0].kind == "ExportKeyword"))
    }
    fn validate_pragmas(&self) -> Result<()> {
        let leading = &self.lexer.src[..self.current.start];
        let mut pos = 0;
        while pos < leading.len() {
            let rest = &leading[pos..];
            let trimmed = rest.trim_start_matches(crate::lexer::whitespace);
            pos += rest.len() - trimmed.len();
            let rest = &leading[pos..];
            if rest.starts_with("/*") {
                let Some(end) = rest.find("*/") else { break };
                pos += end + 2;
                continue;
            }
            if rest.starts_with("#!") {
                pos += rest.find(crate::lexer::line_break).unwrap_or(rest.len());
                continue;
            }
            if !rest.starts_with("//") {
                break;
            }
            let length = rest.find(crate::lexer::line_break).unwrap_or(rest.len());
            let comment = &rest[..length];
            if let Some(xml) = comment
                .strip_prefix("///")
                .map(|text| text.trim_start_matches(char::is_whitespace))
                .and_then(|text| text.strip_prefix('<'))
                .filter(|text| text.contains("/>"))
            {
                let name_end = xml.find(char::is_whitespace).unwrap_or(xml.len());
                let name = &xml[..name_end];
                let attributes = &xml[name_end..];
                let error = if name.eq_ignore_ascii_case("reference") {
                    let types = pragma_attribute(attributes, "types");
                    if pragma_attribute(attributes, "no-default-lib") == Some("true") {
                        None
                    } else if types.is_some() {
                        match pragma_attribute(attributes, "resolution-mode") {
                            Some(mode) if mode != "require" && mode != "import" => {
                                Some("'resolution-mode' must be 'require' or 'import'")
                            }
                            _ => None,
                        }
                    } else if pragma_attribute(attributes, "lib").is_some()
                        || pragma_attribute(attributes, "path").is_some()
                    {
                        None
                    } else {
                        Some("Invalid 'reference' directive syntax")
                    }
                } else {
                    None
                };
                if let Some(message) = error {
                    return Err(Diagnostic {
                        message: message.into(),
                        span: Span {
                            start: pos,
                            end: pos + length,
                        },
                    });
                }
            }
            pos += length;
        }
        Ok(())
    }

    fn module_token_present(&self) -> bool {
        let mut lexer = Lexer {
            src: self.lexer.src,
            pos: 0,
        };
        loop {
            let token = lexer.next();
            if token.kind == Kind::Eof {
                return false;
            }
            if token.raw == "export" {
                return true;
            }
            if token.raw == "import" && lexer.next().raw != "(" {
                return true;
            }
        }
    }

    fn possible_top_level_await(&self, id: NodeId) -> bool {
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            let node = &self.nodes[id.0];
            if matches!(
                node.kind,
                "FunctionDeclaration"
                    | "FunctionExpression"
                    | "ArrowFunction"
                    | "MethodDeclaration"
                    | "GetAccessor"
                    | "SetAccessor"
                    | "Constructor"
                    | "TypeAliasDeclaration"
                    | "InterfaceDeclaration"
                    | "ModuleDeclaration"
                    | "ClassStaticBlockDeclaration"
                    | "EnumDeclaration"
                    | "ImportDeclaration"
                    | "ImportEqualsDeclaration"
                    | "ExportDeclaration"
            ) {
                continue;
            }
            if node.kind == "Identifier"
                && node.flags & AWAIT == 0
                && matches!(node.field("escapedText"), Some(Value::Text(text)) if text == "await")
            {
                return true;
            }
            for (key, value) in &node.fields {
                // Type positions and class member bodies do not introduce top-level await.
                if matches!(*key, "type" | "typeArguments" | "typeParameters")
                    || *key == "members"
                        && matches!(node.kind, "ClassDeclaration" | "ClassExpression")
                    || *key == "name"
                        && matches!(
                            node.kind,
                            "PropertyAccessExpression"
                                | "PropertyAssignment"
                                | "JsxAttribute"
                                | "JsxNamespacedName"
                        )
                    || matches!(*key, "name" | "propertyName")
                        && matches!(
                            node.kind,
                            "VariableDeclaration"
                                | "BindingElement"
                                | "ClassDeclaration"
                                | "ClassExpression"
                                | "PropertyDeclaration"
                        )
                        && matches!(value, Value::Node(id) if self.nodes[id.0].kind == "Identifier")
                {
                    continue;
                }
                match value {
                    Value::Node(id) => pending.push(*id),
                    Value::Nodes(ids) => pending.extend(ids),
                    _ => {}
                }
            }
        }
        false
    }

    fn reparse_top_level_await(&mut self, original: Vec<NodeId>) -> Result<Vec<NodeId>> {
        let lexer = self.lexer;
        let current = self.current;
        let end = self.end;
        // Reparsing commits new nodes and source flags but borrows the cursor and
        // await context. The EOF cursor is needed to finish the SourceFile.
        let result = self.in_context(self.context.with(AWAIT), |p| {
            let mut statements = Vec::with_capacity(original.len());
            let mut index = 0;
            while index < original.len() {
                if !p.possible_top_level_await(original[index]) {
                    statements.push(original[index]);
                    index += 1;
                    continue;
                }
                p.lexer.pos = p.start(original[index]);
                p.current = p.lexer.next();
                p.end = p.current.full_start;
                let mut limit = p.nodes[original[index].0].span.end;
                loop {
                    statements.push(p.statement()?);
                    let merged = p.end > limit;
                    while index < original.len() && p.nodes[original[index].0].span.end <= p.end {
                        index += 1;
                    }
                    if merged && index < original.len() {
                        limit = p.nodes[original[index].0].span.end;
                    }
                    if index < original.len() && p.start(original[index]) < p.end {
                        limit = p.nodes[original[index].0].span.end;
                    }
                    if p.end >= limit {
                        break;
                    }
                }
            }
            Ok(statements)
        });
        self.lexer = lexer;
        self.current = current;
        self.end = end;
        result
    }

    fn error(&self, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            message: message.into(),
            span: Span {
                start: self.current.start,
                end: self.current.end,
            },
        }
    }
    fn check_token(&self) -> Result<()> {
        match self.current.error {
            Some(message) => Err(self.error(message)),
            None => Ok(()),
        }
    }
    fn bump(&mut self) -> Result<Token<'src>> {
        self.check_token()?;
        let previous = self.current;
        self.end = previous.end;
        self.current = self.lexer.next();
        Ok(previous)
    }
    fn at(&self, raw: &str) -> bool {
        self.current.raw == raw
    }
    fn peek(&self) -> Token<'src> {
        let mut lexer = self.lexer;
        lexer.next()
    }
    fn peek_at(&self, raw: &str) -> bool {
        self.peek().raw == raw
    }
    fn peek_second(&self) -> Token<'src> {
        let mut lexer = self.lexer;
        lexer.next();
        lexer.next()
    }
    fn eat(&mut self, raw: &str) -> Result<bool> {
        if self.at(raw) {
            self.bump()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn expect(&mut self, raw: &str) -> Result<Token<'src>> {
        if self.at(raw) {
            self.bump()
        } else {
            Err(self.error(format!("Expected '{raw}', found '{}'", self.current.raw)))
        }
    }
    fn pos(&self) -> usize {
        self.current.full_start
    }
    fn start(&self, id: NodeId) -> usize {
        self.nodes[id.0].span.start
    }
    fn make(
        &mut self,
        kind: &'static str,
        start: usize,
        end: usize,
        fields: impl Into<Fields<'src>>,
    ) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node {
            kind,
            span: Span { start, end },
            flags: self.context.node_flags(),
            fields: fields.into(),
        });
        id
    }
    fn finish(
        &mut self,
        kind: &'static str,
        start: usize,
        fields: impl Into<Fields<'src>>,
    ) -> NodeId {
        self.make(kind, start, self.end, fields)
    }
    fn field(&mut self, id: NodeId, key: &'static str, value: impl Into<Value<'src>>) {
        self.nodes[id.0].fields.push((key, value.into()));
    }
    fn expression_allow_in(&mut self) -> Result<NodeId> {
        self.in_context(self.context.with_in(true), Self::expression)
    }
    fn assignment_allow_in(&mut self) -> Result<NodeId> {
        self.in_context(self.context.with_in(true), Self::assignment_expression)
    }
    /// Restore the enclosing grammar context on both success and syntax error.
    /// Cursor movement, new nodes, and source-file flags belong to the parse.
    fn in_context<T>(
        &mut self,
        context: GrammarContext,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let enclosing = std::mem::replace(&mut self.context, context);
        let result = parse(self);
        self.context = enclosing;
        result
    }
    fn nest<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        if self.depth >= self.options.max_depth {
            return Err(self.error("Maximum syntax nesting depth exceeded"));
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }
    fn attempt<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Option<T> {
        self.attempt_result(f).ok()
    }
    fn attempt_result<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let saved = self.checkpoint();
        let result = f(self);
        if result.is_err() {
            self.restore(saved);
        }
        result
    }
    /// Inspect ambiguous syntax without committing cursor, nodes, or diagnostics.
    fn look_ahead<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let saved = self.checkpoint();
        let result = f(self);
        self.restore(saved);
        result
    }
    fn checkpoint(&self) -> Checkpoint<'src> {
        Checkpoint {
            lexer: self.lexer,
            current: self.current,
            end: self.end,
            nodes_len: self.nodes.len(),
            depth: self.depth,
            context: self.context,
            source_flags: self.source_flags,
            type_probe: self.type_probe,
            type_probe_error: self.type_probe_error.clone(),
        }
    }
    fn restore(&mut self, saved: Checkpoint<'src>) {
        self.lexer = saved.lexer;
        self.current = saved.current;
        self.end = saved.end;
        self.nodes.truncate(saved.nodes_len);
        self.depth = saved.depth;
        self.context = saved.context;
        self.source_flags = saved.source_flags;
        self.type_probe = saved.type_probe;
        self.type_probe_error = saved.type_probe_error;
    }
    fn type_probe_diagnostic(&mut self, message: &str) {
        if self.type_probe_error.is_none() {
            self.type_probe_error = Some(self.error(message));
        }
    }
    fn type_delimiter(&mut self, raw: &str) -> Result<()> {
        if self.type_probe && !self.at(raw) {
            self.type_probe_diagnostic(&format!("Expected '{raw}'"));
            Ok(())
        } else {
            self.expect(raw).map(|_| ())
        }
    }
    fn semi(&mut self) -> Result<()> {
        if self.eat(";")? || self.at("}") || self.current.kind == Kind::Eof || self.current.newline
        {
            Ok(())
        } else {
            Err(self.error("Expected semicolon or line break"))
        }
    }
    fn token_node(&mut self) -> Result<NodeId> {
        let token = self.bump()?;
        let kind = token_kind(token.raw, token.kind);
        Ok(self.make(kind, token.full_start, token.end, vec![]))
    }
    fn optional_token(
        &mut self,
        raw: &str,
        key: &'static str,
        fields: &mut Fields<'src>,
    ) -> Result<bool> {
        if self.at(raw) {
            fields.push((key, self.token_node()?.into()));
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn identifier(&mut self) -> Result<NodeId> {
        if !matches!(self.current.kind, Kind::Word | Kind::Private) {
            return Err(self.error("Expected identifier"));
        }
        let token = self.bump()?;
        let text = decode_identifier(token.raw).map_err(|message| Diagnostic {
            message,
            span: Span {
                start: token.start,
                end: token.end,
            },
        })?;
        let escaped = if text.starts_with("__") {
            format!("_{text}").into()
        } else {
            text
        };
        let id = self.make(
            if token.kind == Kind::Private {
                "PrivateIdentifier"
            } else {
                "Identifier"
            },
            token.full_start,
            token.end,
            fields![("escapedText", Value::Text(escaped))],
        );
        if token.kind == Kind::Word && token.raw.contains("\\u{") {
            self.nodes[id.0].flags |= 256;
        }
        Ok(id)
    }
    fn binding_identifier(&mut self) -> Result<NodeId> {
        if self.current.kind != Kind::Word {
            return Err(self.error("Expected a binding identifier"));
        }
        let name = decode_identifier(self.current.raw).map_err(|message| self.error(message))?;
        if reserved_word(&name) {
            return Err(self.error(format!(
                "Reserved word '{name}' cannot be used as a binding name"
            )));
        }
        self.identifier()
    }
    fn literal(&mut self) -> Result<NodeId> {
        self.literal_mode(false)
    }
    fn literal_mode(&mut self, allow_invalid_escape: bool) -> Result<NodeId> {
        let token = self.bump()?;
        let kind = match token.kind {
            Kind::Number if token.raw.ends_with('n') => "BigIntLiteral",
            Kind::Number => "NumericLiteral",
            Kind::String => "StringLiteral",
            Kind::NoSubstitutionTemplate => "NoSubstitutionTemplateLiteral",
            Kind::TemplateHead => "TemplateHead",
            Kind::TemplateMiddle => "TemplateMiddle",
            Kind::TemplateTail => "TemplateTail",
            _ => return Err(self.error("Expected literal")),
        };
        let mut template_flags = 0;
        let mut raw_text = None;
        let text = if token.kind == Kind::Number {
            Value::Text(numeric_text(token.raw))
        } else {
            let trim = if matches!(token.kind, Kind::TemplateHead | Kind::TemplateMiddle) {
                2
            } else {
                1
            };
            let raw = &token.raw[1..token.raw.len().saturating_sub(trim)];
            if token.kind != Kind::String {
                raw_text = Some(Value::from(raw));
            }
            match decode_string(raw) {
                Ok(value) => value,
                Err(_) if allow_invalid_escape => {
                    template_flags = 2048;
                    raw.into()
                }
                Err(message) => {
                    return Err(Diagnostic {
                        message: message.into(),
                        span: Span {
                            start: token.start,
                            end: token.end,
                        },
                    });
                }
            }
        };
        let mut fields = fields![("text", text)];
        if token.kind == Kind::String
            || template_flags & 2048 == 0 && extended_unicode_escape(token.raw)
        {
            fields.push((
                "hasExtendedUnicodeEscape",
                extended_unicode_escape(token.raw).into(),
            ));
        }
        if token.kind == Kind::Number && !token.raw.ends_with('n') {
            let lower = token.raw.to_ascii_lowercase();
            let mut flags = if lower.starts_with("0x") {
                64
            } else if lower.starts_with("0b") {
                128
            } else if lower.starts_with("0o") {
                256
            } else if lower.contains('e') {
                16
            } else {
                0
            };
            if token.raw.contains('_') {
                flags |= 512;
            }
            fields.push(("numericLiteralFlags", Value::Number(flags)));
        }
        if let Some(raw) = raw_text {
            let mut chars = token.raw.chars();
            while let Some(c) = chars.next() {
                if template_flags & 2048 != 0 {
                    break;
                }
                if c != '\\' {
                    continue;
                }
                match chars.next() {
                    Some('u') => {
                        template_flags |= if chars.clone().next() == Some('{') {
                            8
                        } else {
                            1024
                        };
                    }
                    Some('x') => template_flags |= 4096,
                    _ => {}
                }
            }
            fields.push(("rawText", raw));
            fields.push(("templateFlags", Value::Number(template_flags)));
        }
        Ok(self.make(kind, token.full_start, token.end, fields))
    }
    fn property_name(&mut self) -> Result<NodeId> {
        let start = self.pos();
        if self.eat("[")? {
            let expression = self.expression_allow_in()?;
            self.expect("]")?;
            Ok(self.finish(
                "ComputedPropertyName",
                start,
                fields![("expression", expression.into())],
            ))
        } else if matches!(self.current.kind, Kind::String | Kind::Number) {
            self.literal()
        } else {
            self.identifier()
        }
    }
    fn modifiers(&mut self, class_member: bool) -> Result<Vec<NodeId>> {
        self.modifiers_inner(class_member, false)
    }
    fn modifiers_inner(
        &mut self,
        class_member: bool,
        stop_static_block: bool,
    ) -> Result<Vec<NodeId>> {
        let mut modifiers = Vec::new();
        loop {
            if stop_static_block && self.at("static") && self.peek_at("{") {
                break;
            }
            if self.at("@") {
                let start = self.pos();
                self.bump()?;
                let expression = self.in_context(self.context.with(DECORATOR), |p| {
                    p.postfix_expression(false)
                })?;
                modifiers.push(self.finish(
                    "Decorator",
                    start,
                    fields![("expression", expression.into())],
                ));
                continue;
            }
            let modifier = matches!(
                self.current.raw,
                "export"
                    | "default"
                    | "declare"
                    | "abstract"
                    | "public"
                    | "private"
                    | "protected"
                    | "readonly"
                    | "static"
                    | "accessor"
                    | "async"
            ) || class_member
                && matches!(self.current.raw, "override" | "in" | "out")
                || self.at("const") && (stop_static_block || self.peek_at("enum"));
            if !modifier {
                break;
            }
            let next = self.peek();
            if self.at("export")
                && (matches!(next.raw, "{" | "*" | "as")
                    || next.raw == "type" && matches!(self.peek_second().raw, "{" | "*")
                    || next.raw == "default"
                        && !matches!(
                            self.peek_second().raw,
                            "class" | "function" | "interface" | "@" | "abstract" | "async"
                        ))
            {
                break;
            }
            if self.at("default")
                && !matches!(
                    next.raw,
                    "class" | "function" | "interface" | "@" | "abstract" | "async"
                )
            {
                break;
            }
            if (!matches!(self.current.raw, "export" | "default" | "static") && next.newline)
                || !(matches!(
                    next.kind,
                    Kind::Word | Kind::Private | Kind::String | Kind::Number
                ) || matches!(next.raw, "[" | "{" | "*" | "...")
                    || next.raw == "@" && matches!(self.current.raw, "export" | "default"))
                || self.at("static")
                    && modifiers
                        .iter()
                        .any(|id| self.nodes[id.0].kind == "StaticKeyword")
            {
                break;
            }
            modifiers.push(self.token_node()?);
        }
        Ok(modifiers)
    }
    fn attach_modifiers(&mut self, id: NodeId, modifiers: Vec<NodeId>) {
        if !modifiers.is_empty() {
            self.field(id, "modifiers", modifiers);
        }
    }
    fn ambient_modifiers(&mut self, modifiers: &[NodeId]) {
        if modifiers
            .iter()
            .any(|id| self.nodes[id.0].kind == "DeclareKeyword")
        {
            self.context = self.context.with(AMBIENT);
            for modifier in modifiers {
                self.nodes[modifier.0].flags |= AMBIENT;
            }
        }
    }
    fn deprecated_trivia(&self, include_trailing: bool) -> bool {
        let mut trivia = &self.lexer.src[self.current.full_start..self.current.start];
        let mut collecting = self.current.full_start == 0 || include_trailing;
        if !trivia.contains("/**") {
            return false;
        }
        while !trivia.is_empty() {
            let trimmed = trivia.trim_start_matches(crate::lexer::whitespace);
            collecting |= trivia[..trivia.len() - trimmed.len()].contains(['\r', '\n']);
            trivia = trimmed;
            if trivia.starts_with("//") {
                trivia = &trivia[trivia
                    .find(crate::lexer::line_break)
                    .unwrap_or(trivia.len())..];
            } else if let Some(comment) = trivia.strip_prefix("/*") {
                let Some(end) = comment.find("*/") else {
                    return false;
                };
                let text = &comment[..end];
                if collecting
                    && text.starts_with('*')
                    && text
                        .split("@deprecated")
                        .skip(1)
                        .any(|tail| !tail.starts_with(crate::lexer::id_continue))
                {
                    return true;
                }
                trivia = &comment[end + 2..];
            } else {
                break;
            }
        }
        false
    }
}

fn pragma_attribute<'src>(text: &'src str, name: &str) -> Option<&'src str> {
    for (pos, character) in text.char_indices() {
        if !character.is_whitespace() {
            continue;
        }
        let tail = text[pos..].trim_start_matches(char::is_whitespace);
        let Some(prefix) = tail.get(..name.len()) else {
            continue;
        };
        if !prefix.eq_ignore_ascii_case(name) {
            continue;
        }
        let tail = tail[name.len()..].trim_start_matches(char::is_whitespace);
        let Some(tail) = tail.strip_prefix('=') else {
            continue;
        };
        let tail = tail.trim_start_matches(char::is_whitespace);
        let quote = tail.chars().next()?;
        if !matches!(quote, '\'' | '"') {
            continue;
        }
        let tail = &tail[1..];
        if let Some(end) = tail.find(quote) {
            return Some(&tail[..end]);
        }
    }
    None
}

fn decode_identifier(raw: &str) -> std::result::Result<std::borrow::Cow<'_, str>, String> {
    if !raw.contains('\\') {
        return Ok(raw.into());
    }
    let mut text = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            text.push(c);
            continue;
        }
        if chars.next() != Some('u') {
            return Err("Expected Unicode escape in identifier".into());
        }
        let braced = chars.peek() == Some(&'{');
        if braced {
            chars.next();
        }
        let mut hex = String::new();
        if braced {
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                hex.push(c);
            }
        } else {
            for _ in 0..4 {
                hex.push(chars.next().ok_or("Incomplete Unicode escape")?);
            }
        }
        let c = u32::from_str_radix(&hex, 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or("Invalid Unicode code point")?;
        let valid = if text.is_empty() || text == "#" {
            crate::lexer::id_start(c)
        } else {
            crate::lexer::id_continue(c)
        };
        if !valid {
            return Err("Invalid identifier character in Unicode escape".into());
        }
        text.push(c);
    }
    Ok(text.into())
}

fn extended_unicode_escape(raw: &str) -> bool {
    if !raw.contains("\\u{") {
        return false;
    }
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.next() == Some('u') && chars.clone().next() == Some('{') {
            return true;
        }
    }
    false
}

fn decode_string(raw: &str) -> std::result::Result<Value<'_>, &'static str> {
    if !raw.contains('\\') && !raw.contains('\r') {
        return Ok(raw.into());
    }
    let mut result: Vec<u16> = Vec::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    fn push(result: &mut Vec<u16>, c: char) {
        let mut units = [0; 2];
        result.extend_from_slice(c.encode_utf16(&mut units));
    }
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            result.push(10);
            continue;
        }
        if c != '\\' {
            push(&mut result, c);
            continue;
        }
        let Some(c) = chars.next() else {
            return Err("Incomplete escape sequence");
        };
        match c {
            'n' => result.push(10),
            'r' => result.push(13),
            't' => result.push(9),
            'b' => result.push(8),
            'f' => result.push(12),
            'v' => result.push(11),
            '0' if !chars.peek().is_some_and(char::is_ascii_digit) => result.push(0),
            '0'..='9' => return Err("Octal and decimal escape sequences are not allowed"),
            '\n' | '\u{2028}' | '\u{2029}' => {}
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            'x' | 'u' => {
                let braced = c == 'u' && chars.peek() == Some(&'{');
                if braced {
                    chars.next();
                }
                let mut hex = String::new();
                if braced {
                    let mut closed = false;
                    for c in chars.by_ref() {
                        if c == '}' {
                            closed = true;
                            break;
                        }
                        hex.push(c);
                    }
                    if !closed {
                        return Err("Unterminated Unicode escape");
                    }
                } else {
                    for _ in 0..if c == 'x' { 2 } else { 4 } {
                        hex.push(chars.next().ok_or("Incomplete hexadecimal escape")?);
                    }
                }
                let number =
                    u32::from_str_radix(&hex, 16).map_err(|_| "Invalid hexadecimal escape")?;
                if number <= 0xFFFF {
                    result.push(number as u16);
                } else if let Some(c) = char::from_u32(number) {
                    push(&mut result, c);
                } else {
                    return Err("Unicode escape exceeds maximum code point");
                }
            }
            _ => push(&mut result, c),
        }
    }
    Ok(match String::from_utf16(&result) {
        Ok(text) => text.into(),
        Err(_) => Value::Utf16(result),
    })
}

fn numeric_text(raw: &str) -> std::borrow::Cow<'_, str> {
    if raw.ends_with('n') {
        return if raw.contains('_') || raw.bytes().any(|b| b.is_ascii_uppercase()) {
            raw.replace('_', "").to_ascii_lowercase().into()
        } else {
            raw.into()
        };
    }
    if raw.len() <= 15
        && raw.bytes().all(|b| b.is_ascii_digit())
        && (raw == "0" || !raw.starts_with('0'))
    {
        return raw.into();
    }
    let clean = raw.replace('_', "");
    let value = if clean.starts_with("0x") || clean.starts_with("0X") {
        radix_number(&clean[2..], 4)
    } else if clean.starts_with("0o") || clean.starts_with("0O") {
        radix_number(&clean[2..], 3)
    } else if clean.starts_with("0b") || clean.starts_with("0B") {
        radix_number(&clean[2..], 1)
    } else {
        clean.parse::<f64>().unwrap_or(f64::NAN)
    };
    if value.is_infinite() {
        return "Infinity".into();
    }
    let decimal = value.to_string();
    if !decimal.contains('.') && decimal.len() > 21 {
        let digits = decimal.trim_end_matches('0');
        return if digits.len() == 1 {
            format!("{digits}e+{}", decimal.len() - 1).into()
        } else {
            format!("{}.{}e+{}", &digits[..1], &digits[1..], decimal.len() - 1).into()
        };
    }
    if let Some(fraction) = decimal.strip_prefix("0.") {
        let zeros = fraction.bytes().take_while(|b| *b == b'0').count();
        if zeros >= 6 {
            let digits = &fraction[zeros..];
            return if digits.len() == 1 {
                format!("{digits}e-{}", zeros + 1).into()
            } else {
                format!("{}.{}e-{}", &digits[..1], &digits[1..], zeros + 1).into()
            };
        }
    }
    decimal.into()
}

// Integer-to-float conversion retains a guard bit and sticky bit so rounding
// a long radix literal happens once, even beyond the capacity of u128.
fn radix_number(digits: &str, bits_per_digit: usize) -> f64 {
    let mut significant_bits = 0usize;
    let mut top = 0u64;
    let mut guard = false;
    let mut sticky = false;
    for digit in digits.chars() {
        let value = digit.to_digit(16).unwrap();
        for shift in (0..bits_per_digit).rev() {
            let bit = value & (1 << shift) != 0;
            if significant_bits == 0 && !bit {
                continue;
            }
            significant_bits += 1;
            if significant_bits <= 53 {
                top = (top << 1) | u64::from(bit);
            } else if significant_bits == 54 {
                guard = bit;
            } else {
                sticky |= bit;
            }
        }
    }
    if significant_bits <= 53 {
        return top as f64;
    }
    if guard && (sticky || top & 1 != 0) {
        top += 1;
    }
    if significant_bits > 1024 {
        return f64::INFINITY;
    }
    (top as f64) * 2.0f64.powi(significant_bits as i32 - 53)
}

fn token_kind(raw: &str, kind: Kind) -> &'static str {
    match raw {
        "" if kind == Kind::Eof => "EndOfFileToken",
        "{" => "OpenBraceToken",
        "}" => "CloseBraceToken",
        "(" => "OpenParenToken",
        ")" => "CloseParenToken",
        "[" => "OpenBracketToken",
        "]" => "CloseBracketToken",
        "." => "DotToken",
        "..." => "DotDotDotToken",
        ";" => "SemicolonToken",
        "," => "CommaToken",
        "?." => "QuestionDotToken",
        "<" => "LessThanToken",
        ">" => "GreaterThanToken",
        "<=" => "LessThanEqualsToken",
        ">=" => "GreaterThanEqualsToken",
        "==" => "EqualsEqualsToken",
        "!=" => "ExclamationEqualsToken",
        "===" => "EqualsEqualsEqualsToken",
        "!==" => "ExclamationEqualsEqualsToken",
        "=>" => "EqualsGreaterThanToken",
        "+" => "PlusToken",
        "-" => "MinusToken",
        "*" => "AsteriskToken",
        "**" => "AsteriskAsteriskToken",
        "/" => "SlashToken",
        "%" => "PercentToken",
        "++" => "PlusPlusToken",
        "--" => "MinusMinusToken",
        "<<" => "LessThanLessThanToken",
        ">>" => "GreaterThanGreaterThanToken",
        ">>>" => "GreaterThanGreaterThanGreaterThanToken",
        "&" => "AmpersandToken",
        "|" => "BarToken",
        "^" => "CaretToken",
        "!" => "ExclamationToken",
        "~" => "TildeToken",
        "&&" => "AmpersandAmpersandToken",
        "||" => "BarBarToken",
        "?" => "QuestionToken",
        ":" => "ColonToken",
        "@" => "AtToken",
        "??" => "QuestionQuestionToken",
        "=" => "EqualsToken",
        "+=" => "PlusEqualsToken",
        "-=" => "MinusEqualsToken",
        "*=" => "AsteriskEqualsToken",
        "**=" => "AsteriskAsteriskEqualsToken",
        "/=" => "SlashEqualsToken",
        "%=" => "PercentEqualsToken",
        "<<=" => "LessThanLessThanEqualsToken",
        ">>=" => "GreaterThanGreaterThanEqualsToken",
        ">>>=" => "GreaterThanGreaterThanGreaterThanEqualsToken",
        "&=" => "AmpersandEqualsToken",
        "|=" => "BarEqualsToken",
        "^=" => "CaretEqualsToken",
        "||=" => "BarBarEqualsToken",
        "&&=" => "AmpersandAmpersandEqualsToken",
        "??=" => "QuestionQuestionEqualsToken",
        "abstract" => "AbstractKeyword",
        "accessor" => "AccessorKeyword",
        "as" => "AsKeyword",
        "asserts" => "AssertsKeyword",
        "async" => "AsyncKeyword",
        "await" => "AwaitKeyword",
        "const" => "ConstKeyword",
        "declare" => "DeclareKeyword",
        "default" => "DefaultKeyword",
        "export" => "ExportKeyword",
        "extends" => "ExtendsKeyword",
        "implements" => "ImplementsKeyword",
        "import" => "ImportKeyword",
        "intrinsic" => "IntrinsicKeyword",
        "in" => "InKeyword",
        "out" => "OutKeyword",
        "instanceof" => "InstanceOfKeyword",
        "new" => "NewKeyword",
        "override" => "OverrideKeyword",
        "private" => "PrivateKeyword",
        "protected" => "ProtectedKeyword",
        "public" => "PublicKeyword",
        "readonly" => "ReadonlyKeyword",
        "static" => "StaticKeyword",
        "this" => "ThisKeyword",
        "super" => "SuperKeyword",
        "null" => "NullKeyword",
        "true" => "TrueKeyword",
        "false" => "FalseKeyword",
        "satisfies" => "SatisfiesKeyword",
        _ => "Identifier",
    }
}

fn reserved_word(raw: &str) -> bool {
    matches!(
        raw,
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
    )
}
