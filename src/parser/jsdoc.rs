//! `parseJSDocForNode` and `parseJSDocCommentWorker`. Tag facts are private
//! semantic metadata; they do not replace the legacy serialized AST flags.

use super::*;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct Attachment {
    pub(crate) full_start: usize,
    pub(crate) token_start: usize,
    pub(crate) possible_deprecated: bool,
    pub(crate) parsed: Option<Arc<Tags>>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Tags {
    pub(crate) deprecated: Option<Span>,
    // withJSDoc also sees tags parsed during child/speculative grammar work.
    has_deprecated: bool,
    /// Each comment's own tags (`JSDoc.Tags`), in order: what TypeScript's
    /// JavaScript reparser reads (`reparseTags`).
    pub(crate) comments: Vec<Vec<TagFacts>>,
    /// Where each import type whose argument is a string literal begins, in
    /// the type expressions of the comments' own tags
    /// (`IsLiteralImportTypeNode`), which TypeScript's loader follows from a
    /// JavaScript file (`ForEachDynamicImportOrRequireCall`).
    pub(crate) import_types: Vec<usize>,
}

/// One of a comment's own tags, as the JavaScript reparser reads it.
#[derive(Debug, Clone)]
pub(crate) struct TagFacts {
    /// From its `@` to its end.
    pub(crate) span: Span,
    /// The tag's name as written, such as `returns`.
    pub(crate) name: String,
    pub(crate) syntax: TagSyntax,
    /// The range of its type expression's type, the node `reparseHosted`
    /// clones (`TypeExpression().Type()`), for a tag with one.
    pub(crate) type_span: Option<Span>,
    /// Whether that type needed recovery from a syntax error, which leaves
    /// a tree this parser does not shape as TypeScript's does.
    pub(crate) type_recovered: bool,
}

/// What a tag holds, for each tag `reparseHosted` or `reparseUnhosted` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TagSyntax {
    /// `@type`, whose type expression is never absent.
    Type,
    Satisfies,
    This,
    /// `@return` or `@returns`, with or without a type expression.
    Return {
        typed: bool,
    },
    Template,
    /// `@param`, `@arg` or `@argument`.
    Parameter {
        /// The identifier it names (empty where missing), or None for a
        /// qualified name.
        name: Option<String>,
        bracketed: bool,
        typed: bool,
        /// Its type is `T=`.
        optional_type: bool,
    },
    /// `@public`, `@private`, `@protected`, `@readonly` or `@override`.
    Modifier,
    Implements,
    /// `@augments` or `@extends`: the class name's identifiers, and whether
    /// it has type arguments.
    Augments {
        class: Vec<String>,
        type_arguments: bool,
    },
    /// `@typedef`, with a type expression where it has a type or children.
    Typedef {
        typed: bool,
    },
    Callback,
    Overload,
    /// `@import`, with or without an import clause.
    Import {
        clause: bool,
    },
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Beginning,
    Asterisk,
    Text,
    Backticks,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TagKind {
    Deprecated,
    Parameter,
    Property,
    Template,
    Type,
    This,
    Return,
    Other,
}

struct Tag {
    kind: TagKind,
    span: Span,
    name: Option<NodeId>,
    ty: Option<NodeId>,
    syntax: TagSyntax,
    /// `ty` needed recovery from a syntax error (`TagFacts::type_recovered`).
    recovered: bool,
}

/// A hosted `@type`'s type, cloned out of its comment's own arena: its
/// nodes in pre-order, the type first, linked by index within this list.
pub(super) struct ReparsedType<'src> {
    nodes: Vec<Node<'src>>,
}

/// `addDeepCloneReparse`'s copy of the subtree at `root` of `nodes`: each
/// node once, in pre-order, its links renumbered within the copy.
fn clone_subtree<'src>(nodes: &[Node<'src>], root: NodeId) -> ReparsedType<'src> {
    let mut order = Vec::new();
    let mut index = std::collections::HashMap::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if index.contains_key(&id.0) {
            continue;
        }
        index.insert(id.0, order.len());
        order.push(id);
        let start = stack.len();
        for (_, value) in &nodes[id.0].fields {
            match value {
                Value::Node(child) => stack.push(*child),
                Value::Nodes(children) => stack.extend(children.iter().copied()),
                _ => {}
            }
        }
        stack[start..].reverse();
    }
    let renumber = |id: &mut NodeId| id.0 = index[&id.0];
    let nodes = order
        .iter()
        .map(|id| {
            let mut node = nodes[id.0].clone();
            for (_, value) in &mut node.fields {
                match value {
                    Value::Node(child) => renumber(child),
                    Value::Nodes(children) => children.iter_mut().for_each(renumber),
                    _ => {}
                }
            }
            node
        })
        .collect();
    ReparsedType { nodes }
}

/// The declaration a hosted `@type` of `host`, whose type spans
/// `type_span`, gave its type (`reparseHosted`, `Parser::reparse_hosted_type`):
/// the variable declaration, or one of the variable statement's, whose
/// annotation is that reparsed type.
pub(crate) fn reparsed_type_declaration(
    ast: &Ast<'_>,
    host: NodeId,
    type_span: Span,
) -> Option<NodeId> {
    let node = ast.node(host);
    let declarations = match node.syntax_kind {
        VariableDeclaration => vec![host],
        VariableStatement => crate::syntax::id(node, "declarationList")
            .map(|list| crate::syntax::ids(ast.node(list), "declarations").to_vec())
            .unwrap_or_default(),
        _ => return None,
    };
    declarations.into_iter().find(|&declaration| {
        crate::syntax::id(ast.node(declaration), "type")
            .is_some_and(|ty| ast.is_reparsed(ty) && ast.node(ty).span == type_span)
    })
}

/// Native `parseJSDocForNode`. Each fragment borrows the original source
/// prefix, retaining actual byte ranges without adding JSDoc nodes to the
/// public tree. Grammar diagnostics are discarded by the TS parser; a sticky
/// resource/depth interruption remains a failure.
pub(crate) fn parse_for_node(
    source: &str,
    kind: SyntaxKind,
    attachment: &Attachment,
    options: ParseOptions,
    control: Option<Control>,
) -> Result<Arc<Tags>> {
    parse_for_node_in_context(source, kind, attachment, options, control, None, None)
}

/// `parseJSDocComment` preserves the live parser context when called by
/// `withJSDoc`. The lazy `parseJSDocForNode` entry instead initializes a new
/// parser; notably, its Await/Yield restrictions do not come from the node.
/// With `hosted`, it receives the types of the last comment's `@type` tags,
/// in order, which `reparseHosted` reads.
fn parse_for_node_in_context<'src>(
    source: &'src str,
    kind: SyntaxKind,
    attachment: &Attachment,
    options: ParseOptions,
    control: Option<Control>,
    context: Option<GrammarContext>,
    mut hosted: Option<&mut Vec<ReparsedType<'src>>>,
) -> Result<Arc<Tags>> {
    let mut facts = Tags::default();
    for comment in crate::lexer::jsdoc_comments_controlled(
        source,
        attachment.full_start,
        attachment.token_start,
        crate::syntax::trailing_jsdoc(kind),
        control.as_ref(),
    ) {
        let comment = comment?;
        let end = comment.start + comment.text.len() - 2;
        let mut parser = Parser::initialize_at(
            &source[..end],
            options.clone(),
            control.clone(),
            Some(comment.start + 3),
        )?;
        if let Some(context) = context {
            parser.context = context.with(node_flags::JSDOC);
        }
        let mut types = Vec::new();
        parser.jsdoc_comment(comment.start, &mut facts, &mut types)?;
        parser.check_budget()?;
        if let Some(hosted) = hosted.as_deref_mut() {
            *hosted = types
                .into_iter()
                .map(|ty| clone_subtree(&parser.nodes, ty))
                .collect();
        }
    }

    Ok(Arc::new(facts))
}

impl<'s> Parser<'s> {
    pub(super) fn finish_native_jsdoc(
        &mut self,
        kind: SyntaxKind,
        start: usize,
        fields: impl Into<Fields<'s>>,
    ) -> Result<NodeId> {
        let id = self.finish(kind, start, fields)?;
        self.native_jsdoc_at_start(start, id)?;
        Ok(id)
    }

    pub(super) fn native_jsdoc_at_start(&mut self, start: usize, id: NodeId) -> Result<()> {
        if self.context.contains(node_flags::JSDOC) {
            return Ok(());
        }
        let token = Lexer::new(self.lexer.src, start).next_controlled(self.control.as_ref())?;
        self.native_jsdoc(
            JsdocSite {
                full_start: start,
                start: token.start,
            },
            id,
        )
    }

    /// Native `withJSDoc`, at the actual source factory. The legacy public
    /// flags are left to their existing owner. TS7 eagerly parses only when
    /// the scanner saw a see/link hint; ordinary tags remain lazy.
    pub(super) fn native_jsdoc(&mut self, site: JsdocSite, id: NodeId) -> Result<()> {
        if self.context.contains(node_flags::JSDOC) {
            return Ok(());
        }
        if self.nodes[id.0].native_jsdoc.is_some() {
            return Ok(());
        }
        let flags = crate::lexer::jsdoc::preceding_comment_flags(
            self.lexer.src,
            site.full_start,
            site.start,
            self.control.as_ref(),
        )?;
        if !flags.has_jsdoc {
            return Ok(());
        }
        let mut attachment = Attachment {
            full_start: site.full_start,
            token_start: site.start,
            possible_deprecated: flags.deprecated,
            parsed: None,
        };
        let kind = self.nodes[id.0].syntax_kind;
        // TypeScript parses a JavaScript file's JSDoc eagerly and reparses
        // its tags (`reparseTags`); this parser reparses a variable's
        // `@type` alone, which every other tag's refusal leaves the only one
        // with meaning (`crate::project::binding::javascript`).
        let reparsed = self.javascript && matches!(kind, VariableStatement | VariableDeclaration);
        let mut types = Vec::new();
        if flags.see_or_link || reparsed {
            let facts = parse_for_node_in_context(
                self.lexer.src,
                kind,
                &attachment,
                self.options.clone(),
                self.control.clone(),
                Some(self.context),
                reparsed.then_some(&mut types),
            )?;
            attachment.possible_deprecated |= facts.has_deprecated;
            attachment.parsed = Some(facts);
        }
        self.nodes[id.0].native_jsdoc = Some(attachment);
        for ty in types {
            self.reparse_hosted_type(id, ty)?;
        }
        Ok(())
    }

    /// `reparseHosted` of a `@type` at a variable statement or declaration:
    /// the statement's first declaration without a type, or the declaration
    /// itself without one, takes a copy of the tag's type as its annotation
    /// (`addDeepCloneReparse`), marked as reparsed. Where none is left, the
    /// tag's other meaning, a function's full signature, is not reparsed.
    fn reparse_hosted_type(&mut self, host: NodeId, ty: ReparsedType<'s>) -> Result<()> {
        let untyped =
            |p: &Self, declaration: NodeId| p.nodes[declaration.0].field("type").is_none();
        let target = match self.nodes[host.0].syntax_kind {
            VariableDeclaration => Some(host).filter(|&host| untyped(self, host)),
            _ => match self.nodes[host.0].field("declarationList") {
                Some(Value::Node(list)) => match self.nodes[list.0].field("declarations") {
                    Some(Value::Nodes(declarations)) => declarations
                        .iter()
                        .copied()
                        .find(|&declaration| untyped(self, declaration)),
                    _ => None,
                },
                _ => None,
            },
        };
        let Some(target) = target else {
            return Ok(());
        };
        let base = self.nodes.len();
        for mut node in ty.nodes {
            for (_, value) in &mut node.fields {
                match value {
                    Value::Node(child) => child.0 += base,
                    Value::Nodes(children) => children.iter_mut().for_each(|child| child.0 += base),
                    _ => {}
                }
            }
            node.reparsed = true;
            node.possible_top_level_await = false;
            self.nodes.push(node);
        }
        // In TypeScript's field order: before the initializer.
        let fields = &mut self.nodes[target.0].fields;
        let at = fields
            .iter()
            .position(|(key, _)| *key == "initializer")
            .unwrap_or(fields.len());
        fields.insert(at, ("type", Value::Node(NodeId(base))));
        self.refresh_await_summary(target)
    }

    fn jsdoc_next(&mut self) -> Result<()> {
        self.end = self.current.end;
        self.current = self.lexer.next_jsdoc(self.control.as_ref())?;
        Ok(())
    }

    fn jsdoc_next_text(&mut self, backticks: bool) -> Result<()> {
        self.end = self.current.end;
        self.current = self
            .lexer
            .next_jsdoc_text(backticks, self.control.as_ref())?;
        Ok(())
    }

    fn jsdoc_space(&self) -> bool {
        self.current.kind == Kind::Punct
            && self.current.raw.starts_with([' ', '\t', '\u{b}', '\u{c}'])
    }

    fn jsdoc_newline(&self) -> bool {
        matches!(self.current.raw, "\r" | "\n" | "\r\n")
    }

    fn jsdoc_optional(&mut self, raw: &str) -> Result<bool> {
        if !self.at(raw) {
            return Ok(false);
        }
        self.jsdoc_next()?;
        Ok(true)
    }

    /// `skipWhitespace` keeps trailing trivia before EOF outside node ranges.
    fn jsdoc_skip_space(&mut self) -> Result<()> {
        if (self.jsdoc_space() || self.jsdoc_newline()) && self.jsdoc_trailing_space()? {
            return Ok(());
        }
        while self.jsdoc_space() || self.jsdoc_newline() {
            self.jsdoc_next()?;
        }
        Ok(())
    }

    fn jsdoc_trailing_space(&mut self) -> Result<bool> {
        self.look_ahead(|p| {
            loop {
                p.jsdoc_next()?;
                if p.current.kind == Kind::Eof {
                    return Ok(true);
                }
                if !p.jsdoc_space() && !p.jsdoc_newline() {
                    return Ok(false);
                }
            }
        })
    }

    /// `skipWhitespaceOrAsterisk` returns the final line's indentation byte
    /// count. No comment text needs allocating to preserve that grammar state.
    fn jsdoc_skip_indent(&mut self) -> Result<usize> {
        if (self.jsdoc_space() || self.jsdoc_newline()) && self.jsdoc_trailing_space()? {
            return Ok(0);
        }
        let mut preceding_line = self.current.newline;
        let mut saw_line = false;
        let mut indent = 0;
        while preceding_line && self.at("*") || self.jsdoc_space() || self.jsdoc_newline() {
            indent += self.current.raw.len();
            if self.jsdoc_newline() {
                preceding_line = true;
                saw_line = true;
                indent = 0;
            } else if self.at("*") {
                preceding_line = false;
            }
            self.jsdoc_next()?;
        }
        Ok(if saw_line { indent } else { 0 })
    }

    fn jsdoc_identifier(&mut self) -> Result<NodeId> {
        if self.current.kind != Kind::Word {
            return self.make(
                Identifier,
                self.pos(),
                self.pos(),
                fields![("escapedText", "".into())],
            );
        }
        let token = self.current;
        let text = decode_identifier(
            token.raw,
            Span {
                start: token.start,
                end: token.end,
            },
        )?;
        self.jsdoc_next()?;
        self.make(
            Identifier,
            token.start,
            token.end,
            fields![("escapedText", Value::Text(text))],
        )
    }

    fn jsdoc_name(&self, id: NodeId) -> &str {
        match self.nodes[id.0].field("escapedText") {
            Some(Value::Text(text)) => text,
            _ => "",
        }
    }

    /// Native `createMissingIdentifier` does not consume the unexpected token.
    /// Only the private JSDoc parser enables this recovery; interruption stays
    /// distinct from the syntax diagnostics that native discards for comments.
    pub(super) fn missing_jsdoc_identifier(&mut self, message: &str) -> Result<NodeId> {
        self.check_budget()?;
        self.type_recovery_diagnostic(message);

        self.make(
            Identifier,
            self.pos(),
            self.pos(),
            fields![("escapedText", "".into())],
        )
    }

    /// Native `parseJSDocCommentWorker`, retaining only actual tag facts while
    /// preserving scanner state, fences, links and tag-body advancement.
    fn jsdoc_comment(
        &mut self,
        start: usize,
        facts: &mut Tags,
        hosted: &mut Vec<NodeId>,
    ) -> Result<()> {
        let mut indent = start + 4 - self.lexer.src[..start].rfind('\n').map_or(0, |at| at + 1);
        let mut state = State::Asterisk;
        let mut ticks = 0;
        let mut fenced = false;
        while self.jsdoc_space() {
            self.jsdoc_next()?;
        }
        if self.jsdoc_newline() {
            self.jsdoc_next()?;
            state = State::Beginning;
            indent = 0;
        }
        facts.comments.push(Vec::new());
        loop {
            self.check_budget()?;
            if !self.at("`") && ticks != 0 {
                if ticks >= 3 {
                    fenced = !fenced;
                }
                ticks = 0;
            }
            if self.current.kind == Kind::Eof {
                break;
            }
            if self.at("@") && !fenced && self.lexer.can_follow_jsdoc_at() {
                let nodes = self.nodes.len();
                let (tag, name) = self.jsdoc_named_tag(indent, facts)?;
                if tag.kind == TagKind::Deprecated && facts.deprecated.is_none() {
                    facts.deprecated = Some(tag.span);
                }
                self.literal_import_types(nodes, &mut facts.import_types);
                let ty = tag.ty.and_then(|expression| {
                    let expression = &self.nodes[expression.0];
                    match expression.field("type") {
                        Some(Value::Node(ty)) if expression.syntax_kind == JSDocTypeExpression => {
                            Some(*ty)
                        }
                        _ => None,
                    }
                });
                if tag.syntax == TagSyntax::Type
                    && let Some(ty) = ty
                {
                    hosted.push(ty);
                }
                if let Some(tags) = facts.comments.last_mut() {
                    tags.push(TagFacts {
                        span: tag.span,
                        name,
                        syntax: tag.syntax,
                        type_span: ty.map(|ty| self.nodes[ty.0].span),
                        type_recovered: tag.recovered,
                    });
                }
                state = State::Beginning;
            } else if self.jsdoc_newline() {
                state = State::Beginning;
                indent = 0;
            } else if self.at("*") {
                if state == State::Asterisk {
                    state = State::Text;
                } else {
                    state = State::Asterisk;
                }
                indent += 1;
            } else if self.jsdoc_space() {
                indent += self.current.raw.len();
            } else if self.at("`") {
                ticks += 1;
                state = if state == State::Backticks {
                    State::Text
                } else {
                    State::Backticks
                };
                indent += 1;
            } else {
                let link = self.at("{") && !fenced && self.jsdoc_link()?;
                if !link {
                    indent += self.current.raw.len();
                }
                if state != State::Backticks {
                    state = if fenced {
                        State::Backticks
                    } else {
                        State::Text
                    };
                }
            }
            if matches!(state, State::Text | State::Backticks) {
                self.jsdoc_next_text(state == State::Backticks)?;
            } else {
                self.jsdoc_next()?;
            }
        }
        Ok(())
    }

    /// `parseTagComments`; the boolean records whether trimmed text or links
    /// exist, which controls typedef/callback ranges and further tag parsing.
    fn jsdoc_comments(&mut self, mut indent: usize, initial: Option<usize>) -> Result<bool> {
        let mut state = if initial.is_some() {
            State::Asterisk
        } else {
            State::Beginning
        };
        let mut margin = initial.filter(|&value| value != 0).map(|_| indent);
        let mut content = false;
        let mut ticks = 0;
        let mut fenced = false;
        if let Some(value) = initial {
            indent += value;
        }
        loop {
            self.check_budget()?;
            if !self.at("`") && ticks != 0 {
                if ticks >= 3 {
                    fenced = !fenced;
                }
                ticks = 0;
            }
            if self.current.kind == Kind::Eof {
                break;
            }
            if self.at("@") && !fenced && self.lexer.can_follow_jsdoc_at() {
                self.lexer.pos = self.current.end - 1;
                break;
            }
            if self.jsdoc_newline() {
                state = State::Beginning;
                indent = 0;
            } else if self.jsdoc_space() {
                if margin.is_some_and(|margin| indent + self.current.raw.len() > margin) {
                    state = if fenced {
                        State::Backticks
                    } else {
                        State::Text
                    };
                }
                indent += self.current.raw.len();
            } else if self.at("*") && state == State::Beginning {
                state = State::Asterisk;
                indent += 1;
            } else {
                if self.at("`") {
                    ticks += 1;
                    state = if state == State::Backticks {
                        State::Text
                    } else {
                        State::Backticks
                    };
                } else if state != State::Backticks {
                    state = if fenced {
                        State::Backticks
                    } else {
                        State::Text
                    };
                }
                if self.at("{") && !fenced && self.jsdoc_link()? {
                    content = true;
                } else {
                    margin.get_or_insert(indent);
                    indent += self.current.raw.len();
                    // Native removeTrailingWhitespace uses unicode.IsSpace,
                    // not ECMAScript scanner whitespace (notably U+0085/BOM).
                    for ch in self.current.raw.chars() {
                        if !ch.is_whitespace() {
                            content = true;
                            break;
                        }
                    }
                }
            }
            if matches!(state, State::Text | State::Backticks) {
                self.jsdoc_next_text(state == State::Backticks)?;
            } else {
                self.jsdoc_next()?;
            }
        }
        Ok(content)
    }

    fn jsdoc_trailing_comments(
        &mut self,
        start: usize,
        end: usize,
        mut indent: usize,
        indent_text: usize,
    ) -> Result<bool> {
        if indent_text == 0 {
            indent += end - start;
        }
        self.jsdoc_comments(indent, Some(indent_text.saturating_sub(indent)))
    }

    fn jsdoc_link_prefix(&mut self) -> Result<bool> {
        self.jsdoc_skip_indent()?;
        if !self.jsdoc_optional("{")? || !self.jsdoc_optional("@")? {
            return Ok(false);
        }
        if self.current.kind != Kind::Word {
            return Ok(false);
        }
        let token = self.current;
        let decoded = decode_identifier(
            token.raw,
            Span {
                start: token.start,
                end: token.end,
            },
        )?;
        Ok(matches!(
            decoded.as_ref(),
            "link" | "linkcode" | "linkplain"
        ))
    }

    fn jsdoc_link(&mut self) -> Result<bool> {
        let saved = self.checkpoint();
        if !self.jsdoc_link_prefix()? {
            self.restore(saved);
            return Ok(false);
        }
        self.jsdoc_next()?;
        self.jsdoc_skip_space()?;
        self.jsdoc_link_name()?;
        while !self.at("}") && !self.jsdoc_newline() && self.current.kind != Kind::Eof {
            self.jsdoc_next()?;
        }
        Ok(true)
    }

    fn jsdoc_link_name(&mut self) -> Result<()> {
        if self.current.kind != Kind::Word {
            return Ok(());
        }
        self.identifier()?;
        while self.eat(".")? {
            if self.current.kind != Kind::Private {
                self.jsdoc_normal_identifier()?;
            }
        }
        while self.current.kind == Kind::Private {
            // Native ReScanHashToken retains '#' alone before returning to
            // the JSDoc scanner for the identifier that follows it.
            self.lexer.pos = self.current.start + 1;
            self.jsdoc_next()?;
            self.jsdoc_normal_identifier()?;
        }
        Ok(())
    }

    fn jsdoc_normal_identifier(&mut self) -> Result<NodeId> {
        if self.current.kind == Kind::Word {
            return self.identifier();
        }

        self.make(
            Identifier,
            self.pos(),
            self.pos(),
            fields![("escapedText", "".into())],
        )
    }

    fn jsdoc_entity_name(&mut self) -> Result<NodeId> {
        let mut name = self.jsdoc_identifier()?;
        if self.eat("[")? {
            self.expect("]")?;
        }
        while self.eat(".")? {
            let right = self.jsdoc_identifier()?;
            if self.eat("[")? {
                self.expect("]")?;
            }
            name = self.make(
                QualifiedName,
                self.nodes[name.0].span.start,
                self.pos(),
                fields![("left", name.into()), ("right", right.into())],
            )?;
        }
        Ok(name)
    }

    /// Native `parseJSDocType`: ordinary type grammar owns type advancement;
    /// its diagnostics are discarded by the TS comment parser, never controls.
    fn jsdoc_type(&mut self) -> Result<NodeId> {
        self.lexer.set_jsdoc_leading_asterisks(true);
        let start = self.pos();
        let result = (|| {
            let variadic = self.eat("...")?;
            let ty = match self.return_type() {
                Ok(ty) => ty,
                Err(Failure::Syntax(error)) => {
                    self.check_budget()?;
                    self.record_type_error(error);

                    let name = self.make(
                        Identifier,
                        self.pos(),
                        self.pos(),
                        fields![("escapedText", "".into())],
                    )?;
                    self.make(
                        TypeReference,
                        self.nodes[name.0].span.start,
                        self.pos(),
                        fields![("typeName", name.into())],
                    )?
                }
                Err(error) => return Err(error),
            };
            if variadic {
                self.make(
                    JSDocVariadicType,
                    start,
                    self.pos(),
                    fields![("type", ty.into())],
                )
            } else {
                Ok(ty)
            }
        })();
        self.lexer.set_jsdoc_leading_asterisks(false);
        let ty = result?;
        if self.eat("=")? {
            self.make(
                JSDocOptionalType,
                start,
                self.pos(),
                fields![("type", ty.into())],
            )
        } else {
            Ok(ty)
        }
    }

    fn jsdoc_type_expression(&mut self, omit_braces: bool) -> Result<NodeId> {
        let start = self.pos();
        let has_brace = if omit_braces {
            self.eat("{")?
        } else {
            let has = self.at("{");
            self.expect("{")?;
            has
        };
        let ty = self.jsdoc_type()?;
        if has_brace {
            self.jsdoc_optional("}")?;
        }

        self.make(
            JSDocTypeExpression,
            start,
            self.pos(),
            fields![("type", ty.into())],
        )
    }

    fn jsdoc_try_type(&mut self) -> Result<Option<NodeId>> {
        self.jsdoc_skip_indent()?;
        if self.at("{") {
            self.jsdoc_type_expression(false).map(Some)
        } else {
            Ok(None)
        }
    }

    fn jsdoc_object_type(&self, mut ty: NodeId) -> bool {
        loop {
            let node = &self.nodes[ty.0];
            match node.syntax_kind {
                JSDocTypeExpression => match node.field("type") {
                    Some(Value::Node(inner)) => ty = *inner,
                    _ => return false,
                },
                ObjectKeyword => return true,
                ArrayType => match node.field("elementType") {
                    Some(Value::Node(element)) => ty = *element,
                    _ => return false,
                },
                TypeReference if node.field("typeArguments").is_none() => {
                    return match node.field("typeName") {
                        Some(Value::Node(name)) => {
                            self.nodes[name.0].syntax_kind == Identifier
                                && self.jsdoc_name(*name) == "Object"
                        }
                        _ => false,
                    };
                }
                _ => return false,
            }
        }
    }

    /// Whether the import has an import clause.
    fn jsdoc_import(&mut self) -> Result<bool> {
        let pos = self.pos();
        let name = if self.is_identifier()? {
            Some(self.identifier()?)
        } else {
            None
        };
        let clause = name.is_some() || self.at("*") || self.at("{");
        if clause {
            self.import_clause(name, pos, Some(TypeKeyword), true)?;
            self.expect("from")?;
        }
        match self.module_specifier() {
            Ok(_) => {}
            Err(Failure::Syntax(error)) => {
                self.check_budget()?;
                self.record_type_error(error);
            }
            Err(error) => return Err(error),
        }
        if self.at("with") || self.at("assert") && !self.current.newline {
            let pos = self.pos();
            self.bump()?;
            self.import_attribute_body(pos)?;
        }
        Ok(clause)
    }

    fn jsdoc_tag(&mut self, indent: usize, facts: &mut Tags) -> Result<Tag> {
        self.jsdoc_named_tag(indent, facts).map(|(tag, _)| tag)
    }

    /// A tag and its name as written.
    fn jsdoc_named_tag(&mut self, indent: usize, facts: &mut Tags) -> Result<(Tag, String)> {
        self.nest(|p| {
            let start = p.current.start;
            p.jsdoc_next()?;
            let tag_name = p.jsdoc_identifier()?;
            let name = p.jsdoc_name(tag_name).to_owned();
            let indent_text = p.jsdoc_skip_indent()?;
            p.jsdoc_tag_body(start, tag_name, indent, indent_text, facts)
                .map(|tag| (tag, name))
        })
    }

    /// Where each import type with a string-literal argument among the nodes
    /// from `first` on begins (`IsLiteralImportTypeNode`). A speculative
    /// parse that was rolled back has left none of its nodes.
    fn literal_import_types(&self, first: usize, starts: &mut Vec<usize>) {
        for node in &self.nodes[first..] {
            if node.syntax_kind != ImportType {
                continue;
            }
            let Some(Value::Node(argument)) = node.field("argument") else {
                continue;
            };
            let argument = &self.nodes[argument.0];
            if argument.syntax_kind == LiteralType
                && matches!(argument.field("literal"), Some(Value::Node(literal)) if self.nodes[literal.0].syntax_kind == StringLiteral)
            {
                starts.push(node.span.start);
            }
        }
    }

    fn jsdoc_tag_body(
        &mut self,
        start: usize,
        tag_name: NodeId,
        indent: usize,
        indent_text: usize,
        facts: &mut Tags,
    ) -> Result<Tag> {
        let mut kind = TagKind::Other;
        let mut ty = None;
        let mut name = None;
        let mut syntax = TagSyntax::Other;
        let mut recovered = false;
        match self.jsdoc_name(tag_name) {
            "deprecated" => {
                kind = TagKind::Deprecated;
                facts.has_deprecated = true;
            }
            "arg" | "argument" | "param" => {
                return self.jsdoc_parameter_tag(start, false, indent, facts);
            }
            "return" | "returns" | "exception" | "throws" => {
                ty = self.jsdoc_try_type()?;
                if matches!(self.jsdoc_name(tag_name), "return" | "returns") {
                    kind = TagKind::Return;
                    syntax = TagSyntax::Return {
                        typed: ty.is_some(),
                    };
                }
            }
            "type" | "this" | "satisfies" => {
                (kind, syntax) = match self.jsdoc_name(tag_name) {
                    "this" => (TagKind::This, TagSyntax::This),
                    "type" => (TagKind::Type, TagSyntax::Type),
                    _ => (TagKind::Type, TagSyntax::Satisfies),
                };
                let omit_braces = self.jsdoc_name(tag_name) != "satisfies";
                let (expression, error) =
                    self.recover_types(|p| p.jsdoc_type_expression(omit_braces))?;
                ty = Some(expression);
                recovered = error.is_some();
                if kind == TagKind::This {
                    self.jsdoc_skip_space()?;
                }
            }
            "template" => {
                kind = TagKind::Template;
                syntax = TagSyntax::Template;
                if self.at("{") {
                    ty = Some(self.jsdoc_type_expression(false)?);
                }
                loop {
                    self.jsdoc_skip_space()?;
                    let bracketed = self.jsdoc_optional("[")?;
                    if bracketed {
                        self.jsdoc_skip_space()?;
                    }
                    self.modifiers_inner(false, true)?;
                    self.jsdoc_identifier()?;
                    if bracketed {
                        self.jsdoc_skip_space()?;
                        self.expect("=")?;
                        self.jsdoc_type()?;
                        self.expect("]")?;
                    }
                    self.jsdoc_skip_space()?;
                    self.jsdoc_skip_indent()?;
                    if !self.jsdoc_optional(",")? {
                        break;
                    }
                }
            }
            "implements" | "augments" | "extends" => {
                let braced = self.eat("{")?;
                let mut class = vec![self.jsdoc_identifier()?];
                while self.eat(".")? {
                    class.push(self.jsdoc_identifier()?);
                }
                self.lexer.set_jsdoc_leading_asterisks(true);
                let type_arguments = self.type_open();
                let result = if type_arguments {
                    self.type_arguments().map(|_| ())
                } else {
                    Ok(())
                };
                self.lexer.set_jsdoc_leading_asterisks(false);
                result?;
                syntax = if self.jsdoc_name(tag_name) == "implements" {
                    TagSyntax::Implements
                } else {
                    TagSyntax::Augments {
                        class: class
                            .into_iter()
                            .map(|name| self.jsdoc_name(name).to_owned())
                            .collect(),
                        type_arguments,
                    }
                };
                if braced {
                    self.jsdoc_skip_space()?;
                    self.expect("}")?;
                }
            }
            "see" => {
                let plain =
                    self.is_identifier()? && !self.lexer.src[self.current.end..].starts_with("://");
                let braced = self.at("{")
                    && self.look_ahead(|p| {
                        p.bump()?;
                        Ok(p.current.kind == Kind::Word)
                    })?;
                if plain || braced {
                    let braces = self.eat("{")?;
                    self.jsdoc_link_name()?;
                    if braces {
                        self.jsdoc_optional("}")?;
                    }
                    self.lexer.pos = self.current.full_start;
                    self.jsdoc_next()?;
                }
            }
            "typedef" => return self.jsdoc_typedef(start, indent, indent_text, facts),
            "public" | "private" | "protected" | "readonly" | "override" => {
                syntax = TagSyntax::Modifier;
            }
            "callback" | "overload" => {
                let callback = self.jsdoc_name(tag_name) == "callback";
                if callback {
                    name = Some(self.jsdoc_namespace_name()?);
                }
                self.jsdoc_skip_space()?;
                let mut comment = self.jsdoc_comments(indent, None)?;
                let signature_end = self.jsdoc_callback_parameters(indent, facts)?;
                if !comment {
                    comment =
                        self.jsdoc_trailing_comments(start, self.pos(), indent, indent_text)?;
                }
                return Ok(Tag {
                    kind,
                    name,
                    ty,
                    span: Span {
                        start,
                        end: if comment { self.pos() } else { signature_end },
                    },
                    syntax: if callback {
                        TagSyntax::Callback
                    } else {
                        TagSyntax::Overload
                    },
                    recovered: false,
                });
            }
            "import" => {
                syntax = TagSyntax::Import {
                    clause: self.jsdoc_import()?,
                };
            }
            _ => {}
        }
        self.jsdoc_trailing_comments(start, self.pos(), indent, indent_text)?;
        Ok(Tag {
            kind,
            name,
            ty,
            span: Span {
                start,
                end: self.pos(),
            },
            syntax,
            recovered,
        })
    }

    fn jsdoc_parameter_tag(
        &mut self,
        start: usize,
        property: bool,
        indent: usize,
        facts: &mut Tags,
    ) -> Result<Tag> {
        let mut ty = self.jsdoc_try_type()?;
        let name_first = ty.is_none();
        self.jsdoc_skip_indent()?;
        let bracketed = self.jsdoc_optional("[")?;
        if bracketed {
            self.jsdoc_skip_space()?;
        }
        let backquoted = self.jsdoc_optional("`")?;
        let name = self.jsdoc_entity_name()?;
        if backquoted {
            self.jsdoc_optional("`")?;
        }
        if bracketed {
            self.jsdoc_skip_space()?;
            if self.eat("=")? {
                match self.expression_allow_in() {
                    Ok(_) => {}
                    Err(Failure::Syntax(error)) => {
                        self.check_budget()?;
                        self.record_type_error(error);
                    }
                    Err(error) => return Err(error),
                }
            }
            self.expect("]")?;
        }
        let indent_text = self.jsdoc_skip_indent()?;
        if name_first && !self.look_ahead(Self::jsdoc_link_prefix)? {
            ty = self.jsdoc_try_type()?;
        }
        self.jsdoc_trailing_comments(start, self.pos(), indent, indent_text)?;
        if ty.is_some_and(|ty| self.jsdoc_object_type(ty)) {
            loop {
                let saved = self.checkpoint();
                let child = self.jsdoc_child_tag(if property { 1 } else { 2 }, indent, facts)?;
                let admitted = child.as_ref().is_some_and(|child| {
                    if !matches!(child.kind, TagKind::Property | TagKind::Parameter) {
                        return true;
                    }
                    let Some(child_name) = child.name else {
                        return false;
                    };
                    match self.nodes[child_name.0].field("left") {
                        Some(Value::Node(left)) => self.jsdoc_names_equal(name, *left),
                        _ => false,
                    }
                });
                if !admitted {
                    self.restore(saved);
                    break;
                }
            }
        }
        let syntax = if property {
            TagSyntax::Other
        } else {
            TagSyntax::Parameter {
                name: (self.nodes[name.0].syntax_kind == Identifier)
                    .then(|| self.jsdoc_name(name).to_owned()),
                bracketed,
                typed: ty.is_some(),
                optional_type: ty.is_some_and(|ty| {
                    matches!(self.nodes[ty.0].field("type"), Some(Value::Node(inner)) if self.nodes[inner.0].syntax_kind == JSDocOptionalType)
                }),
            }
        };
        Ok(Tag {
            kind: if property {
                TagKind::Property
            } else {
                TagKind::Parameter
            },
            span: Span {
                start,
                end: self.pos(),
            },
            name: Some(name),
            ty,
            syntax,
            recovered: false,
        })
    }

    fn jsdoc_names_equal(&self, mut left: NodeId, mut right: NodeId) -> bool {
        loop {
            let a = &self.nodes[left.0];
            let b = &self.nodes[right.0];
            if a.syntax_kind == Identifier && b.syntax_kind == Identifier {
                return self.jsdoc_name(left) == self.jsdoc_name(right);
            }
            let (
                Some(Value::Node(al)),
                Some(Value::Node(ar)),
                Some(Value::Node(bl)),
                Some(Value::Node(br)),
            ) = (
                a.field("left"),
                a.field("right"),
                b.field("left"),
                b.field("right"),
            )
            else {
                return false;
            };
            if self.jsdoc_name(*ar) != self.jsdoc_name(*br) {
                return false;
            }
            left = *al;
            right = *bl;
        }
    }

    fn jsdoc_namespace_name(&mut self) -> Result<NodeId> {
        self.nest(|p| {
            let start = p.current.start;
            let name = p.jsdoc_identifier()?;
            if p.jsdoc_optional(".")? {
                let body = p.jsdoc_namespace_name()?;

                p.make(
                    ModuleDeclaration,
                    start,
                    p.pos(),
                    fields![("name", name.into()), ("body", body.into())],
                )
            } else {
                Ok(name)
            }
        })
    }

    /// `parseChildParameterOrPropertyTag`: callers roll back a non-child;
    /// resource work and the parse's possible-tag hint are never rolled back.
    fn jsdoc_child_tag(
        &mut self,
        target: u8,
        indent: usize,
        facts: &mut Tags,
    ) -> Result<Option<Tag>> {
        let mut can_parse = true;
        let mut saw_asterisk = false;
        loop {
            self.jsdoc_next()?;
            if self.current.kind == Kind::Eof {
                return Ok(None);
            }
            if self.at("@") {
                if can_parse && self.lexer.can_follow_jsdoc_at() {
                    let start = self.current.full_start;
                    self.jsdoc_next()?;
                    let name = self.jsdoc_identifier()?;
                    let indent_text = self.jsdoc_skip_indent()?;
                    return match self.jsdoc_name(name) {
                        "type" if target == 1 => {
                            let ty = self.jsdoc_type_expression(true)?;
                            Ok(Some(Tag {
                                kind: TagKind::Type,
                                span: Span {
                                    start,
                                    end: self.pos(),
                                },
                                name: None,
                                ty: Some(ty),
                                syntax: TagSyntax::Type,
                                recovered: false,
                            }))
                        }
                        "prop" | "property" if target == 1 => self
                            .jsdoc_parameter_tag(start, true, indent, facts)
                            .map(Some),
                        "arg" | "argument" | "param" if target & 6 != 0 => self
                            .jsdoc_parameter_tag(start, false, indent, facts)
                            .map(Some),
                        "template" | "this" => self
                            .jsdoc_tag_body(start, name, indent, indent_text, facts)
                            .map(Some),
                        _ => Ok(None),
                    };
                }
                saw_asterisk = false;
            } else if self.jsdoc_newline() {
                can_parse = true;
                saw_asterisk = false;
            } else if self.at("*") {
                if saw_asterisk {
                    can_parse = false;
                }
                saw_asterisk = true;
            } else if self.current.kind == Kind::Word {
                let name = decode_identifier(
                    self.current.raw,
                    Span {
                        start: self.current.start,
                        end: self.current.end,
                    },
                )?;
                if crate::lexer::identifier_class(&name)
                    == crate::lexer::IdentifierClass::Identifier
                {
                    can_parse = false;
                }
            }
        }
    }

    fn jsdoc_callback_parameters(&mut self, indent: usize, facts: &mut Tags) -> Result<usize> {
        loop {
            let saved = self.checkpoint();
            if self.jsdoc_child_tag(4, indent, facts)?.is_none() {
                self.restore(saved);
                break;
            }
        }
        let saved = self.checkpoint();
        if !self.jsdoc_optional("@")? || self.jsdoc_tag(indent, facts)?.kind != TagKind::Return {
            self.restore(saved);
        }
        Ok(self.pos())
    }

    fn jsdoc_typedef(
        &mut self,
        start: usize,
        indent: usize,
        indent_text: usize,
        facts: &mut Tags,
    ) -> Result<Tag> {
        let ty = self.jsdoc_try_type()?;
        self.jsdoc_skip_indent()?;
        let name = self.jsdoc_namespace_name()?;
        self.jsdoc_skip_space()?;
        let comment = self.jsdoc_comments(indent, None)?;
        let mut child_type = None;
        let mut has_children = false;
        if ty.is_none_or(|ty| self.jsdoc_object_type(ty)) {
            loop {
                let saved = self.checkpoint();
                let Some(child) = self.jsdoc_child_tag(1, indent, facts)? else {
                    self.restore(saved);
                    break;
                };
                has_children = true;
                if child.kind == TagKind::Type && child_type.is_none() {
                    child_type = child.ty;
                }
            }
        }
        let end = if has_children {
            if let Some(child_type) = child_type.filter(|ty| !self.jsdoc_object_type(*ty)) {
                self.nodes[child_type.0].span.end
            } else {
                self.pos()
            }
        } else if comment {
            self.pos()
        } else {
            self.nodes[name.0].span.end
        };
        if !comment {
            self.jsdoc_trailing_comments(start, end, indent, indent_text)?;
        }
        Ok(Tag {
            kind: TagKind::Other,
            span: Span { start, end },
            name: Some(name),
            ty,
            syntax: TagSyntax::Typedef {
                typed: ty.is_some() || has_children,
            },
            recovered: false,
        })
    }
}
