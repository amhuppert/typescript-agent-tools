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
    parse_for_node_in_context(source, kind, attachment, options, control, None)
}

/// `parseJSDocComment` preserves the live parser context when called by
/// `withJSDoc`. The lazy `parseJSDocForNode` entry instead initializes a new
/// parser; notably, its Await/Yield restrictions do not come from the node.
fn parse_for_node_in_context(
    source: &str,
    kind: SyntaxKind,
    attachment: &Attachment,
    options: ParseOptions,
    control: Option<Control>,
    context: Option<GrammarContext>,
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
        parser.jsdoc_comment(comment.start, &mut facts)?;
        parser.check_budget()?;
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
        if flags.see_or_link {
            let facts = parse_for_node_in_context(
                self.lexer.src,
                self.nodes[id.0].syntax_kind,
                &attachment,
                self.options.clone(),
                self.control.clone(),
                Some(self.context),
            )?;
            attachment.possible_deprecated |= facts.has_deprecated;
            attachment.parsed = Some(facts);
        }
        self.nodes[id.0].native_jsdoc = Some(attachment);
        Ok(())
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

    /// Native parser `isIdentifier`, distinct from IdentifierName, which
    /// also admits reserved keywords. JSDoc's scanner decodes keyword escapes.
    pub(super) fn jsdoc_is_identifier(&self) -> Result<bool> {
        if self.current.kind != Kind::Word {
            return Ok(false);
        }
        let name = decode_identifier(
            self.current.raw,
            Span {
                start: self.current.start,
                end: self.current.end,
            },
        )?;
        Ok(!(reserved_word(&name)
            || name == "yield" && self.context.contains(YIELD)
            || name == "await" && self.context.contains(AWAIT)))
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
    fn jsdoc_comment(&mut self, start: usize, facts: &mut Tags) -> Result<()> {
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
                let tag = self.jsdoc_tag(indent, facts)?;
                if tag.kind == TagKind::Deprecated && facts.deprecated.is_none() {
                    facts.deprecated = Some(tag.span);
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

    fn jsdoc_import(&mut self) -> Result<()> {
        let pos = self.pos();
        let name = if self.jsdoc_is_identifier()? {
            Some(self.identifier()?)
        } else {
            None
        };
        if name.is_some() || self.at("*") || self.at("{") {
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
        Ok(())
    }

    fn jsdoc_tag(&mut self, indent: usize, facts: &mut Tags) -> Result<Tag> {
        self.nest(|p| {
            let start = p.current.start;
            p.jsdoc_next()?;
            let tag_name = p.jsdoc_identifier()?;
            let indent_text = p.jsdoc_skip_indent()?;
            p.jsdoc_tag_body(start, tag_name, indent, indent_text, facts)
        })
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
        match self.jsdoc_name(tag_name) {
            "deprecated" => {
                kind = TagKind::Deprecated;
                facts.has_deprecated = true;
            }
            "arg" | "argument" | "param" => {
                return self.jsdoc_parameter_tag(start, false, indent, facts);
            }
            "return" | "returns" | "exception" | "throws" => {
                if matches!(self.jsdoc_name(tag_name), "return" | "returns") {
                    kind = TagKind::Return;
                }
                ty = self.jsdoc_try_type()?;
            }
            "type" | "this" | "satisfies" => {
                kind = if self.jsdoc_name(tag_name) == "this" {
                    TagKind::This
                } else {
                    TagKind::Type
                };
                ty = Some(self.jsdoc_type_expression(self.jsdoc_name(tag_name) != "satisfies")?);
                if kind == TagKind::This {
                    self.jsdoc_skip_space()?;
                }
            }
            "template" => {
                kind = TagKind::Template;
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
                self.jsdoc_identifier()?;
                while self.eat(".")? {
                    self.jsdoc_identifier()?;
                }
                self.lexer.set_jsdoc_leading_asterisks(true);
                let result = if self.type_open() {
                    self.type_arguments().map(|_| ())
                } else {
                    Ok(())
                };
                self.lexer.set_jsdoc_leading_asterisks(false);
                result?;
                if braced {
                    self.jsdoc_skip_space()?;
                    self.expect("}")?;
                }
            }
            "see" => {
                let plain = self.jsdoc_is_identifier()?
                    && !self.lexer.src[self.current.end..].starts_with("://");
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
                });
            }
            "import" => {
                self.jsdoc_import()?;
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
        })
    }
}
