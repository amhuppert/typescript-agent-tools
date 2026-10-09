use crate::compiler::{node as node_flags, script, target, token as token_flags, variant};
use crate::lexer::{Kind, Lexer, Token};
use crate::syntax_kind::{SyntaxKind, SyntaxKind::*};
use crate::{Ast, Diagnostic, Node, NodeId, ParseOptions, Span, Value};
use std::collections::HashSet;

type Fields<'src> = crate::ast::Fields<'src>;
macro_rules! fields {
    ($($value:tt)*) => {{
        let fields: Fields<'_> = smallvec::smallvec![$($value)*];
        fields
    }};
}

mod angles;
mod context;
mod expressions;
pub(crate) mod json;
mod jsx;
mod literals;
mod statements;
mod types;

use context::{
    AMBIENT, AWAIT, DECORATOR, DISALLOW_CONDITIONAL_TYPES, DISALLOW_IN, GrammarContext, IN_WITH,
    YIELD,
};

type Result<T> = std::result::Result<T, Diagnostic>;

/// The message of the one diagnostic that reports an exhausted grammar
/// budget rather than invalid syntax (`exhaustion`).
const DEPTH_EXCEEDED: &str = "Maximum syntax nesting depth exceeded";

/// Whether a parse failed because input exceeded the grammar budget
/// (`ParseOptions::max_depth`), a resource limit, rather than because it is
/// not valid syntax.
pub(crate) fn exhaustion(diagnostic: &Diagnostic) -> bool {
    diagnostic.message == DEPTH_EXCEEDED
}

/// Recovery is used only to decide whether malformed expression type arguments
/// commit to type syntax. The first diagnostic survives within that probe.
#[derive(Clone, Default)]
enum TypeRecovery {
    #[default]
    Strict,
    Recovering(Option<Diagnostic>),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ProbeKind {
    Arrow { allow_return_type: bool },
    FunctionType,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct FailedProbe {
    kind: ProbeKind,
    offset: usize,
    context: GrammarContext,
    depth: usize,
    recovering_types: bool,
}

pub(crate) struct Parser<'src> {
    lexer: Lexer<'src>,
    current: Token<'src>,
    // `end` is the last consumed token's end; lexer.pos includes lookahead.
    end: usize,
    nodes: Vec<Node<'src>>,
    options: ParseOptions,
    depth: usize,
    context: GrammarContext,
    source_flags: u32,
    // Immutable for this input. Declaration files never reparse top-level await;
    // other files need summaries only when the source may contain that keyword.
    track_await: bool,
    type_recovery: TypeRecovery,
    // Failed interpretations depend on immutable source and these grammar
    // settings, not arena IDs. Keep them across rollback to bound repeated work.
    failed_probes: HashSet<FailedProbe>,
    // Which type-argument attempts can close; also a property of the source.
    angles: angles::Angles,
    // During a type-argument attempt in an expression: the offset its parse
    // cannot pass, and how many type-argument lists it has open.
    speculation: Option<usize>,
    type_argument_frames: usize,
    // The first depth-budget diagnostic, including one raised by a speculative
    // interpretation. Rollback keeps it, so exhaustion always ends the parse.
    exhausted: Option<Diagnostic>,
}

/// A speculative parse owns newly appended nodes. Existing nodes are read-only
/// until it commits; rolling back restores the cursor and every grammar setting,
/// but never clears budget exhaustion.
struct Checkpoint<'src> {
    lexer: Lexer<'src>,
    current: Token<'src>,
    end: usize,
    nodes_len: usize,
    depth: usize,
    context: GrammarContext,
    source_flags: u32,
    type_recovery: TypeRecovery,
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str, options: ParseOptions) -> Self {
        let mut lexer = Lexer {
            src: source,
            pos: 0,
        };
        let current = lexer.next();
        let context = GrammarContext::new(options.declaration_file());
        let track_await = !options.declaration_file() && source.contains("await");
        Self {
            lexer,
            current,
            end: 0,
            nodes: Vec::with_capacity((source.len() / 16).min(16_384)),
            options,
            depth: 0,
            context,
            source_flags: 0,
            track_await,
            type_recovery: TypeRecovery::Strict,
            failed_probes: HashSet::new(),
            angles: angles::Angles::default(),
            speculation: None,
            type_argument_frames: 0,
            exhausted: None,
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
        let external = crate::ast::external_module(&self.nodes, &statements, self.source_flags);
        if let Some(error) = deferred_error.filter(|_| !external) {
            return Err(error);
        }
        if !self.options.declaration_file() && has_await && external {
            statements = self.reparse_top_level_await(statements)?;
        }
        // Exhaustion in a discarded interpretation still fails the parse.
        self.check_budget()?;
        let eof = self.token_node()?;
        let root = self.make(
            SourceFile,
            0,
            self.lexer.src.len(),
            fields![
                ("statements", statements.into()),
                ("endOfFileToken", eof.into()),
                ("text", self.lexer.src.into()),
                ("languageVersion", Value::Number(target::LATEST)),
                (
                    "languageVariant",
                    Value::Number(if self.options.jsx {
                        variant::JSX
                    } else {
                        variant::STANDARD
                    }),
                ),
                (
                    "scriptKind",
                    Value::Number(if self.options.jsx {
                        script::TSX
                    } else {
                        script::TS
                    }),
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

    fn validate_pragmas(&self) -> Result<()> {
        let leading = &self.lexer.src[..self.current.start];
        for reference in crate::pragmas::references(leading) {
            use crate::pragmas::{Directive, TypesMode};
            // TypeScript's `processPragmasIntoFields` reports an invalid
            // `resolution-mode` at its value (`parseResolutionMode`) and any
            // other invalid directive at the whole comment.
            let error = match reference.directive() {
                Directive::Types(_, _, TypesMode::Invalid) => {
                    let (value, start) = reference
                        .attribute("resolution-mode")
                        .expect("an invalid mode is written");
                    Some((
                        "'resolution-mode' must be 'require' or 'import'",
                        Span {
                            start,
                            end: start + value.len(),
                        },
                    ))
                }
                Directive::Invalid => {
                    Some(("Invalid 'reference' directive syntax", reference.span()))
                }
                Directive::Ignored
                | Directive::Types(..)
                | Directive::Lib(..)
                | Directive::Path(..) => None,
            };
            if let Some((message, span)) = error {
                return Err(Diagnostic {
                    message: message.into(),
                    span,
                });
            }
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
        let node = &self.nodes[id.0];
        node.flags & AWAIT == 0 && node.possible_top_level_await
    }

    /// Compute the expression summary while constructing syntax, rather than
    /// rediscovering grammar scopes in a separate traversal during module reparse.
    /// A method's computed name and decorators belong to its enclosing scope;
    /// its body does not. Property initializers still trigger class reparsing,
    /// although their grammar context continues to disallow await expressions.
    fn await_summary(&self, kind: SyntaxKind, fields: &Fields<'src>) -> bool {
        if !self.track_await {
            return false;
        }
        if kind == Identifier {
            return matches!(fields.iter().find(|(key, _)| *key == "escapedText"),
                Some((_, Value::Text(text))) if text == "await");
        }
        if (TypePredicate.number()..=ImportType.number()).contains(&kind.number())
            || matches!(
                kind,
                TypeParameter
                    | PropertySignature
                    | MethodSignature
                    | CallSignature
                    | ConstructSignature
                    | IndexSignature
                    | TypeAliasDeclaration
                    | InterfaceDeclaration
                    | FunctionDeclaration
                    | FunctionExpression
                    | ArrowFunction
                    | Constructor
                    | ModuleDeclaration
                    | EnumDeclaration
                    | ImportDeclaration
                    | ImportEqualsDeclaration
                    | ImportClause
                    | ImportSpecifier
                    | NamespaceImport
                    | NamedImports
                    | ExportDeclaration
                    | ExportSpecifier
                    | NamedExports
                    | NamespaceExport
                    | NamespaceExportDeclaration
            )
        {
            return false;
        }
        let method = matches!(kind, MethodDeclaration | GetAccessor | SetAccessor);
        if method && !fields.iter().any(|(key, _)| *key == "body") {
            return false;
        }
        if matches!(kind, ClassDeclaration | ClassExpression)
            && matches!(fields.iter().find(|(key, _)| *key == "modifiers"),
                Some((_, Value::Nodes(ids))) if ids.iter().any(|id| self.nodes[id.0].syntax_kind == DeclareKeyword))
        {
            return false;
        }
        fields.iter().any(|(key, value)| {
            if matches!(*key, "type" | "typeArguments" | "typeParameters")
                || method && *key == "body"
            {
                return false;
            }
            let child = |id: &NodeId| {
                let node = &self.nodes[id.0];
                // Identifier names may be spelled `await`; computed names and
                // binding patterns retain the summaries of their expressions.
                !(matches!(*key, "name" | "propertyName" | "label")
                    && node.syntax_kind == Identifier)
                    && node.possible_top_level_await
            };
            match value {
                Value::Node(id) => child(id),
                Value::Nodes(ids) => ids.iter().any(child),
                _ => false,
            }
        })
    }

    fn refresh_await_summary(&mut self, id: NodeId) {
        let node = &self.nodes[id.0];
        let summary = self.await_summary(node.syntax_kind, &node.fields);
        self.nodes[id.0].possible_top_level_await = summary;
    }

    fn set_kind(&mut self, id: NodeId, kind: SyntaxKind) {
        self.nodes[id.0].syntax_kind = kind;
        self.nodes[id.0].kind = kind.name();
        self.refresh_await_summary(id);
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
        kind: SyntaxKind,
        start: usize,
        end: usize,
        fields: impl Into<Fields<'src>>,
    ) -> NodeId {
        let id = NodeId(self.nodes.len());
        let fields = fields.into();
        let summary = self.await_summary(kind, &fields);
        self.nodes.push(Node {
            kind: kind.name(),
            syntax_kind: kind,
            possible_top_level_await: summary,
            span: Span { start, end },
            flags: self.context.node_flags(),
            fields,
        });
        id
    }
    fn finish(
        &mut self,
        kind: SyntaxKind,
        start: usize,
        fields: impl Into<Fields<'src>>,
    ) -> NodeId {
        self.make(kind, start, self.end, fields)
    }
    fn field(&mut self, id: NodeId, key: &'static str, value: impl Into<Value<'src>>) {
        self.nodes[id.0].fields.push((key, value.into()));
        self.refresh_await_summary(id);
    }
    fn expression_allow_in(&mut self) -> Result<NodeId> {
        self.in_context(self.context.with_in(true), Self::expression)
    }
    fn assignment_allow_in(&mut self) -> Result<NodeId> {
        self.in_context(self.context.with_in(true), Self::assignment_expression)
    }
    /// Budget the callback frame and restore the enclosing grammar context on
    /// both success and syntax error. Context callbacks can themselves recurse.
    /// Cursor movement, new nodes, and source-file flags belong to the parse.
    fn in_context<T>(
        &mut self,
        context: GrammarContext,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let enclosing = std::mem::replace(&mut self.context, context);
        let result = self.nest(parse);
        self.context = enclosing;
        result
    }
    /// Charge one grammar entry against the depth budget, including entries made
    /// while speculating. Exceeding it is sticky: later entries and speculation
    /// helpers fail with the same diagnostic, and `parse` returns it.
    fn nest<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        if self.exhausted.is_none() && self.depth >= self.options.max_depth {
            self.exhausted = Some(self.error(DEPTH_EXCEEDED));
        }
        self.check_budget()?;
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }
    fn check_budget(&self) -> Result<()> {
        match &self.exhausted {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
    /// Classify a speculative result: a syntax error is a grammar mismatch
    /// (`None`), but budget exhaustion is a failure and propagates.
    fn matched<T>(&self, result: Result<T>) -> Result<Option<T>> {
        self.check_budget()?;
        Ok(result.ok())
    }
    fn attempt<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<Option<T>> {
        let result = self.attempt_result(f);
        self.matched(result)
    }
    /// Remember grammar non-matches. Committed errors remain diagnostics, and
    /// exhaustion is never a non-match. Depth stays in the key so a cached
    /// non-match answers only probes with the same remaining budget.
    fn probe_cached<T>(
        &mut self,
        kind: ProbeKind,
        parse: impl FnOnce(&mut Self) -> Result<Option<T>>,
    ) -> Result<Option<T>> {
        let probe = FailedProbe {
            kind,
            offset: self.current.start,
            context: self.context,
            depth: self.depth,
            recovering_types: self.recovering_types(),
        };
        if self.failed_probes.contains(&probe) {
            return Ok(None);
        }
        let result = self.probe(parse);
        if matches!(result, Ok(None)) {
            self.failed_probes.insert(probe);
        }
        result
    }
    // Each speculation helper returns the exhaustion diagnostic even when the
    // callback converted it into a non-match or an alternative result.
    fn attempt_result<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let saved = self.checkpoint();
        let result = f(self);
        if result.is_err() {
            self.restore(saved);
        }
        self.check_budget()?;
        result
    }
    /// Commit only a recognized interpretation. Both ordinary non-matches and
    /// syntax errors restore cursor, nodes, context, and recovery diagnostics.
    fn probe<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<Option<T>>) -> Result<Option<T>> {
        let saved = self.checkpoint();
        let result = f(self);
        if !matches!(result, Ok(Some(_))) {
            self.restore(saved);
        }
        self.check_budget()?;
        result
    }
    /// Inspect ambiguous syntax without committing cursor, nodes, or diagnostics.
    fn look_ahead<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let saved = self.checkpoint();
        let result = f(self);
        self.restore(saved);
        self.check_budget()?;
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
            type_recovery: self.type_recovery.clone(),
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
        self.type_recovery = saved.type_recovery;
    }
    fn recovering_types(&self) -> bool {
        matches!(self.type_recovery, TypeRecovery::Recovering(_))
    }
    fn record_type_error(&mut self, error: Diagnostic) {
        if let TypeRecovery::Recovering(first @ None) = &mut self.type_recovery {
            *first = Some(error);
        }
    }
    fn type_recovery_diagnostic(&mut self, message: &str) {
        self.record_type_error(self.error(message));
    }
    /// Recover missing type syntax inside a probe, returning its first diagnostic
    /// and restoring the enclosing recovery mode on both success and error.
    fn recover_types<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<(T, Option<Diagnostic>)> {
        let enclosing = std::mem::replace(&mut self.type_recovery, TypeRecovery::Recovering(None));
        let result = f(self);
        let recovered = std::mem::replace(&mut self.type_recovery, enclosing);
        let TypeRecovery::Recovering(error) = recovered else {
            unreachable!()
        };
        result.map(|value| (value, error))
    }
    /// Consume an existing delimiter. In recovery mode a missing delimiter is
    /// diagnosed and returns `false` without advancing the cursor.
    fn type_delimiter(&mut self, raw: &str) -> Result<bool> {
        if self.recovering_types() && !self.at(raw) {
            self.type_recovery_diagnostic(&format!("Expected '{raw}'"));
            Ok(false)
        } else {
            self.expect(raw).map(|_| true)
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
                PrivateIdentifier
            } else {
                Identifier
            },
            token.full_start,
            token.end,
            fields![("escapedText", Value::Text(escaped))],
        );
        if token.kind == Kind::Word && token.raw.contains("\\u{") {
            self.nodes[id.0].flags |= node_flags::IDENTIFIER_HAS_EXTENDED_UNICODE_ESCAPE;
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
    /// TypeScript's `parseLiteralLikeNode`: preserve the token's cooked text
    /// and scanner flags in the final literal node. Numeric flags follow
    /// `scanNumber` and the scanner's radix-prefixed number branches.
    fn literal_mode(&mut self, allow_invalid_escape: bool) -> Result<NodeId> {
        let token = self.bump()?;
        let kind = match token.kind {
            Kind::Number if token.raw.ends_with('n') => BigIntLiteral,
            Kind::Number => NumericLiteral,
            Kind::String => StringLiteral,
            Kind::NoSubstitutionTemplate => NoSubstitutionTemplateLiteral,
            Kind::TemplateHead => TemplateHead,
            Kind::TemplateMiddle => TemplateMiddle,
            Kind::TemplateTail => TemplateTail,
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
            let decoded = literals::decode(raw);
            template_flags = decoded.flags;
            if !allow_invalid_escape {
                if let Some(message) = decoded.error {
                    return Err(Diagnostic {
                        message: message.into(),
                        span: Span {
                            start: token.start,
                            end: token.end,
                        },
                    });
                }
            }
            decoded.text
        };
        let mut fields = fields![("text", text)];
        let extended_escape = template_flags & token_flags::EXTENDED_UNICODE_ESCAPE != 0;
        if token.kind == Kind::String || extended_escape {
            fields.push(("hasExtendedUnicodeEscape", extended_escape.into()));
        }
        if token.kind == Kind::Number && !token.raw.ends_with('n') {
            let mut flags = match token.raw.as_bytes() {
                [b'0', b'x' | b'X', ..] => token_flags::HEX_SPECIFIER,
                [b'0', b'b' | b'B', ..] => token_flags::BINARY_SPECIFIER,
                [b'0', b'o' | b'O', ..] => token_flags::OCTAL_SPECIFIER,
                _ if token.raw.contains(['e', 'E']) => token_flags::SCIENTIFIC,
                _ => 0,
            };
            if token.raw.contains('_') {
                flags |= token_flags::CONTAINS_SEPARATOR;
            }
            fields.push(("numericLiteralFlags", Value::Number(flags)));
        }
        if let Some(raw) = raw_text {
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
                ComputedPropertyName,
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
                    Decorator,
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
                        .any(|id| self.nodes[id.0].syntax_kind == StaticKeyword)
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
    fn prepend_modifiers(&mut self, id: NodeId, mut modifiers: Vec<NodeId>) {
        if let Some((_, Value::Nodes(existing))) = self.nodes[id.0]
            .fields
            .iter_mut()
            .find(|(key, _)| *key == "modifiers")
        {
            modifiers.append(existing);
            *existing = modifiers;
            self.refresh_await_summary(id);
        } else {
            self.attach_modifiers(id, modifiers);
        }
    }
    fn ambient_modifiers(&mut self, modifiers: &[NodeId]) {
        if modifiers
            .iter()
            .any(|id| self.nodes[id.0].syntax_kind == DeclareKeyword)
        {
            self.context = self.context.with(AMBIENT);
            for modifier in modifiers {
                self.nodes[modifier.0].flags |= AMBIENT;
            }
        }
    }
    /// Where the JSDoc of a node starting at the current token is
    /// collected: the trivia before it.
    fn jsdoc_site(&self) -> JsdocSite {
        JsdocSite {
            full_start: self.current.full_start,
            start: self.current.start,
        }
    }
    /// Mark `id`, which starts at `site`, deprecated when a JSDoc comment it
    /// collects carries `@deprecated`: `crate::lexer::jsdoc_comments`, with
    /// the trailing comments `crate::syntax::trailing_jsdoc` gives its kind.
    fn mark_deprecated(&mut self, site: JsdocSite, id: NodeId) {
        let src = self.lexer.src;
        let from = site.full_start.max(crate::lexer::shebang_end(src));
        if from >= site.start || !src[from..site.start].contains("/**") {
            return;
        }
        let trailing = crate::syntax::trailing_jsdoc(self.nodes[id.0].syntax_kind);
        let deprecated = crate::lexer::jsdoc_comments(src, site.full_start, site.start, trailing)
            .any(|comment| {
                comment
                    .text
                    .split("@deprecated")
                    .skip(1)
                    .any(|tail| !tail.starts_with(crate::lexer::id_continue))
            });
        if deprecated {
            self.nodes[id.0].flags |= node_flags::DEPRECATED;
        }
    }
}

/// The trivia before a node's first token (`Parser::jsdoc_site`).
#[derive(Clone, Copy)]
struct JsdocSite {
    full_start: usize,
    start: usize,
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

fn decode_string(raw: &str) -> std::result::Result<Value<'_>, &'static str> {
    let decoded = literals::decode(raw);
    match decoded.error {
        Some(error) => Err(error),
        None => Ok(decoded.text),
    }
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

fn token_kind(raw: &str, kind: Kind) -> SyntaxKind {
    match raw {
        "" if kind == Kind::Eof => EndOfFileToken,
        "{" => OpenBraceToken,
        "}" => CloseBraceToken,
        "(" => OpenParenToken,
        ")" => CloseParenToken,
        "[" => OpenBracketToken,
        "]" => CloseBracketToken,
        "." => DotToken,
        "..." => DotDotDotToken,
        ";" => SemicolonToken,
        "," => CommaToken,
        "?." => QuestionDotToken,
        "<" => LessThanToken,
        ">" => GreaterThanToken,
        "<=" => LessThanEqualsToken,
        ">=" => GreaterThanEqualsToken,
        "==" => EqualsEqualsToken,
        "!=" => ExclamationEqualsToken,
        "===" => EqualsEqualsEqualsToken,
        "!==" => ExclamationEqualsEqualsToken,
        "=>" => EqualsGreaterThanToken,
        "+" => PlusToken,
        "-" => MinusToken,
        "*" => AsteriskToken,
        "**" => AsteriskAsteriskToken,
        "/" => SlashToken,
        "%" => PercentToken,
        "++" => PlusPlusToken,
        "--" => MinusMinusToken,
        "<<" => LessThanLessThanToken,
        ">>" => GreaterThanGreaterThanToken,
        ">>>" => GreaterThanGreaterThanGreaterThanToken,
        "&" => AmpersandToken,
        "|" => BarToken,
        "^" => CaretToken,
        "!" => ExclamationToken,
        "~" => TildeToken,
        "&&" => AmpersandAmpersandToken,
        "||" => BarBarToken,
        "?" => QuestionToken,
        ":" => ColonToken,
        "@" => AtToken,
        "??" => QuestionQuestionToken,
        "=" => EqualsToken,
        "+=" => PlusEqualsToken,
        "-=" => MinusEqualsToken,
        "*=" => AsteriskEqualsToken,
        "**=" => AsteriskAsteriskEqualsToken,
        "/=" => SlashEqualsToken,
        "%=" => PercentEqualsToken,
        "<<=" => LessThanLessThanEqualsToken,
        ">>=" => GreaterThanGreaterThanEqualsToken,
        ">>>=" => GreaterThanGreaterThanGreaterThanEqualsToken,
        "&=" => AmpersandEqualsToken,
        "|=" => BarEqualsToken,
        "^=" => CaretEqualsToken,
        "||=" => BarBarEqualsToken,
        "&&=" => AmpersandAmpersandEqualsToken,
        "??=" => QuestionQuestionEqualsToken,
        "abstract" => AbstractKeyword,
        "accessor" => AccessorKeyword,
        "as" => AsKeyword,
        "asserts" => AssertsKeyword,
        "async" => AsyncKeyword,
        "await" => AwaitKeyword,
        "const" => ConstKeyword,
        "declare" => DeclareKeyword,
        "default" => DefaultKeyword,
        "export" => ExportKeyword,
        "extends" => ExtendsKeyword,
        "implements" => ImplementsKeyword,
        "import" => ImportKeyword,
        "intrinsic" => IntrinsicKeyword,
        "in" => InKeyword,
        "out" => OutKeyword,
        "instanceof" => InstanceOfKeyword,
        "new" => NewKeyword,
        "override" => OverrideKeyword,
        "private" => PrivateKeyword,
        "protected" => ProtectedKeyword,
        "public" => PublicKeyword,
        "readonly" => ReadonlyKeyword,
        "static" => StaticKeyword,
        "this" => ThisKeyword,
        "super" => SuperKeyword,
        "null" => NullKeyword,
        "true" => TrueKeyword,
        "false" => FalseKeyword,
        "satisfies" => SatisfiesKeyword,
        _ => Identifier,
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
