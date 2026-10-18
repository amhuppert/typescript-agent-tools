//! On-demand nom scanner. Regex, templates and JSX are rescanned in the
//! grammatical context supplied by the parser, rather than guessed lexically.

use crate::resources::{Control, Stop};

pub(crate) mod jsdoc;

use nom::{
    IResult, Parser as _,
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{anychar, char},
    combinator::recognize,
    sequence::{delimited, pair},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Eof,
    Word,
    Private,
    Number,
    String,
    TemplateHead,
    TemplateMiddle,
    TemplateTail,
    NoSubstitutionTemplate,
    Punct,
    Unknown,
}

/// Native `GetIdentifierToken`, retaining the partitions used by scanner
/// Identifier tests and parser `isIdentifier`. `raw` is already decoded text;
/// the public AST's serialized token spelling remains a separate projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdentifierClass {
    Identifier,
    ReservedWord,
    Keyword,
}

enum IdentifierKeyword {
    Public(crate::syntax_kind::SyntaxKind),
    Immediate,
}

/// `GetIdentifierToken` / `IdentifierToKeywordKind` share one exact keyword
/// table. `immediate` is native 7 syntax without a keyword kind in the public
/// TypeScript 6 JSON AST baseline; its lexical class remains Keyword.
fn identifier_keyword(raw: &str) -> Option<IdentifierKeyword> {
    use crate::syntax_kind::SyntaxKind::*;
    Some(IdentifierKeyword::Public(match raw {
        "break" => BreakKeyword,
        "case" => CaseKeyword,
        "catch" => CatchKeyword,
        "class" => ClassKeyword,
        "const" => ConstKeyword,
        "continue" => ContinueKeyword,
        "debugger" => DebuggerKeyword,
        "default" => DefaultKeyword,
        "delete" => DeleteKeyword,
        "do" => DoKeyword,
        "else" => ElseKeyword,
        "enum" => EnumKeyword,
        "export" => ExportKeyword,
        "extends" => ExtendsKeyword,
        "false" => FalseKeyword,
        "finally" => FinallyKeyword,
        "for" => ForKeyword,
        "function" => FunctionKeyword,
        "if" => IfKeyword,
        "import" => ImportKeyword,
        "in" => InKeyword,
        "instanceof" => InstanceOfKeyword,
        "new" => NewKeyword,
        "null" => NullKeyword,
        "return" => ReturnKeyword,
        "super" => SuperKeyword,
        "switch" => SwitchKeyword,
        "this" => ThisKeyword,
        "throw" => ThrowKeyword,
        "true" => TrueKeyword,
        "try" => TryKeyword,
        "typeof" => TypeOfKeyword,
        "var" => VarKeyword,
        "void" => VoidKeyword,
        "while" => WhileKeyword,
        "with" => WithKeyword,
        "implements" => ImplementsKeyword,
        "interface" => InterfaceKeyword,
        "let" => LetKeyword,
        "package" => PackageKeyword,
        "private" => PrivateKeyword,
        "protected" => ProtectedKeyword,
        "public" => PublicKeyword,
        "static" => StaticKeyword,
        "yield" => YieldKeyword,
        "abstract" => AbstractKeyword,
        "accessor" => AccessorKeyword,
        "as" => AsKeyword,
        "asserts" => AssertsKeyword,
        "assert" => AssertKeyword,
        "any" => AnyKeyword,
        "async" => AsyncKeyword,
        "await" => AwaitKeyword,
        "boolean" => BooleanKeyword,
        "constructor" => ConstructorKeyword,
        "declare" => DeclareKeyword,
        "get" => GetKeyword,
        "infer" => InferKeyword,
        "intrinsic" => IntrinsicKeyword,
        "is" => IsKeyword,
        "keyof" => KeyOfKeyword,
        "module" => ModuleKeyword,
        "namespace" => NamespaceKeyword,
        "never" => NeverKeyword,
        "out" => OutKeyword,
        "readonly" => ReadonlyKeyword,
        "require" => RequireKeyword,
        "number" => NumberKeyword,
        "object" => ObjectKeyword,
        "satisfies" => SatisfiesKeyword,
        "set" => SetKeyword,
        "string" => StringKeyword,
        "symbol" => SymbolKeyword,
        "type" => TypeKeyword,
        "undefined" => UndefinedKeyword,
        "unique" => UniqueKeyword,
        "unknown" => UnknownKeyword,
        "using" => UsingKeyword,
        "from" => FromKeyword,
        "global" => GlobalKeyword,
        "bigint" => BigIntKeyword,
        "override" => OverrideKeyword,
        "of" => OfKeyword,
        "defer" => DeferKeyword,
        "immediate" => return Some(IdentifierKeyword::Immediate),
        _ => return None,
    }))
}

pub(crate) fn identifier_class(raw: &str) -> IdentifierClass {
    use crate::syntax_kind::SyntaxKind;
    match identifier_keyword(raw) {
        None => IdentifierClass::Identifier,
        Some(IdentifierKeyword::Public(kind))
            if (SyntaxKind::BreakKeyword as u16..=SyntaxKind::WithKeyword as u16)
                .contains(&(kind as u16)) =>
        {
            IdentifierClass::ReservedWord
        }
        Some(_) => IdentifierClass::Keyword,
    }
}

/// The public AST keyword kind, when one exists. This projection does not
/// substitute an AST kind for native-only keywords or change source identity.
pub(crate) fn identifier_keyword_syntax(raw: &str) -> Option<crate::syntax_kind::SyntaxKind> {
    match identifier_keyword(raw) {
        Some(IdentifierKeyword::Public(kind)) => Some(kind),
        Some(IdentifierKeyword::Immediate) | None => None,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Token<'src> {
    pub kind: Kind,
    pub raw: &'src str,
    pub start: usize,
    pub full_start: usize,
    pub end: usize,
    pub newline: bool,
    pub error: Option<&'static str>,
}

#[derive(Clone, Copy)]
pub(crate) struct Lexer<'src> {
    pub src: &'src str,
    pub pos: usize,
    skip_jsdoc_leading_asterisks: usize,
}

pub(crate) fn line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

// The ASCII members of `line_break`, `whitespace`, `id_start` and
// `id_continue`, for the uncontrolled scans' per-byte fast paths: a byte is
// a character only where it is ASCII, and each is false for any other byte.
// `ascii_classes_are_their_owners` checks every byte against its owner.
fn ascii_line_break(byte: u8) -> bool {
    matches!(byte, b'\n' | b'\r')
}
fn ascii_whitespace(byte: u8) -> bool {
    matches!(byte, b'\t' | 0x0B | 0x0C | b' ') || ascii_line_break(byte)
}
fn ascii_id_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'$' || byte == b'_'
}
fn ascii_id_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'$' || byte == b'_'
}
pub(crate) fn whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\u{000B}'
            | '\u{000C}'
            | ' '
            | '\u{0085}'
            | '\u{00A0}'
            | '\u{FEFF}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200B}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    ) || line_break(c)
}

/// Where a leading `#!` line ends, or 0 without one. A byte order mark may
/// precede it: TypeScript removes the mark when it reads a file, so its
/// `getShebang` sees the `#!` at the start. Analysis reads remove it the same
/// way (`source::decode`); text given to the parser directly,
/// as in parse mode, may still begin with it.
pub(crate) fn shebang_end(text: &str) -> usize {
    scan_shebang_end(text, None)
}

pub(crate) fn shebang_end_controlled(text: &str, control: Option<&Control>) -> Result<usize, Stop> {
    let end = scan_shebang_end(text, control);
    if let Some(control) = control {
        control.poll()?;
    }
    Ok(end)
}

fn scan_shebang_end(text: &str, control: Option<&Control>) -> usize {
    let start = if text.starts_with('\u{FEFF}') { 3 } else { 0 };
    if !text[start..].starts_with("#!") {
        return 0;
    }
    text[start..]
        .char_indices()
        .find(|(_, c)| !scan_work(control) || line_break(*c))
        .map(|(at, _)| at)
        .map_or(text.len(), |length| start + length)
}

/// Where a comment starting at `pos` ends: a `//` comment before its line
/// break, a `/* */` comment after its `*/`. `None` when no comment starts at
/// `pos` or a block comment is unterminated.
pub(crate) fn comment_end(text: &str, pos: usize) -> Option<usize> {
    scan_comment_end(text, pos, None)
}

fn scan_comment_end(text: &str, pos: usize, control: Option<&Control>) -> Option<usize> {
    let rest = &text[pos..];
    if rest.starts_with("//") {
        Some(
            pos + match control {
                None => first_line_break(rest).unwrap_or(rest.len()),
                Some(_) => rest
                    .char_indices()
                    .find(|(_, c)| !scan_work(control) || line_break(*c))
                    .map_or(rest.len(), |(at, _)| at),
            },
        )
    } else if let Some(body) = rest.strip_prefix("/*") {
        Some(
            pos + match control {
                None => body.find("*/")?,
                Some(_) => body
                    .as_bytes()
                    .windows(2)
                    .position(|pair| !scan_work(control) || pair == b"*/")?,
            } + 4,
        )
    } else {
        None
    }
}

/// Where the first `line_break` of `text` is, found by bytes: `\n` and
/// `\r` are bytes no other character contains, and U+2028 and U+2029 are the
/// only line breaks whose encoding begins with 0xE2. The uncontrolled
/// scans' fast path; a controlled scan polls at each character instead.
pub(crate) fn first_line_break(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(found) = memchr::memchr3(b'\n', b'\r', 0xE2, &bytes[from..]) {
        let at = from + found;
        let rest = &bytes[at + 1..];
        if bytes[at] != 0xE2 || rest.starts_with(&[0x80, 0xA8]) || rest.starts_with(&[0x80, 0xA9]) {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

/// How long the `whitespace` run at the start of `text` is, and whether it
/// holds a `line_break`: an uncontrolled scan's fast path over ASCII, which
/// takes the general scan from the first other character.
fn whitespace_run(text: &str) -> (usize, bool) {
    let mut newline = false;
    for (at, &byte) in text.as_bytes().iter().enumerate() {
        match byte {
            byte if ascii_whitespace(byte) => newline |= ascii_line_break(byte),
            byte if byte.is_ascii() => return (at, newline),
            _ => {
                let rest = &text[at..];
                let length = rest.find(|c| !whitespace(c)).unwrap_or(rest.len());
                return (
                    at + length,
                    newline || rest[..length].chars().any(line_break),
                );
            }
        }
    }
    (text.len(), newline)
}

/// One comment in trivia.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Comment<'s> {
    /// The comment, including its delimiters.
    pub(crate) text: &'s str,
    pub(crate) start: usize,
    /// The whitespace between the previous comment, or the scan's start, and
    /// this one.
    pub(crate) space_before: &'s str,
}

/// The comments in the trivia from `pos` until the next token, as
/// TypeScript's `iterateCommentRanges` finds them. Consumers apply their own
/// selection: which comments count, and which whitespace may separate them.
pub(crate) fn comments(text: &str, pos: usize) -> impl Iterator<Item = Comment<'_>> {
    comments_controlled(text, pos, None).map(|result| result.expect("uncontrolled comment scan"))
}

pub(crate) fn comments_controlled<'s>(
    text: &'s str,
    mut pos: usize,
    control: Option<&'s Control>,
) -> impl Iterator<Item = Result<Comment<'s>, Stop>> + 's {
    let mut stopped = false;
    std::iter::from_fn(move || {
        if stopped {
            return None;
        }
        let result = (|| {
            let rest = &text[pos..];
            let start = pos + rest.len() - rest.trim_start_matches(whitespace).len();
            let end = scan_comment_end(text, start, control);
            if let Some(control) = control {
                control.poll()?;
            }
            let Some(end) = end else {
                return Ok(None);
            };
            let comment = Comment {
                text: &text[start..end],
                start,
                space_before: &text[pos..start],
            };
            pos = end;
            Ok(Some(comment))
        })();
        match result {
            Ok(Some(comment)) => Some(Ok(comment)),
            Ok(None) => {
                stopped = true;
                None
            }
            Err(stop) => {
                stopped = true;
                Some(Err(stop))
            }
        }
    })
}

/// Every comment in `text` between tokens, in source order, as spans: the
/// trivia outside `literals`, the string, template, regular expression, and
/// JSX text tokens of the final tree in source order (`Ast::literal_token_spans`),
/// outside which `//` and `/*` always begin a comment. A `#!` line is not a
/// comment, and an unterminated block comment ends the scan. TypeScript's
/// scanner meets the same comments as it skips trivia.
pub(crate) fn comment_spans(
    text: &str,
    literals: Vec<crate::Span>,
) -> impl Iterator<Item = crate::Span> + '_ {
    let mut literals = literals.into_iter().peekable();
    let mut at = shebang_end(text);
    std::iter::from_fn(move || {
        while at < text.len() {
            if let Some(literal) = literals.next_if(|literal| literal.start <= at) {
                at = at.max(literal.end);
                continue;
            }
            let limit = literals.peek().map_or(text.len(), |literal| literal.start);
            let Some(slash) = text[at..limit].find('/') else {
                at = limit;
                continue;
            };
            let start = at + slash;
            let Some(end) = comment_end(text, start) else {
                if text[start..].starts_with("/*") {
                    at = text.len();
                    return None;
                }
                at = start + 1;
                continue;
            };
            at = end;
            return Some(crate::Span { start, end });
        }
        None
    })
}

/// The JSDoc comments TypeScript collects for a node whose trivia runs from
/// `full_start` to its first token at `token_start`: port of the collection
/// half of `GetJSDocCommentRanges` (internal/parser/utilities.go), over
/// `GetLeadingCommentRanges` and, for the kinds that also take trailing
/// comments (`include_trailing`, which `crate::syntax::trailing_jsdoc`
/// decides), `GetTrailingCommentRanges`. A leading comment is collected only at the start of
/// the file (after a `#!` line), or once a CR or LF has appeared in the
/// trivia: `iterateCommentRanges` switches collection on for those
/// characters alone, so U+2028 and U+2029 do not, unlike every other
/// line-break rule (`line_break`). A JSDoc comment is a block comment
/// beginning `/**`, other than `/**/`.
pub(crate) fn jsdoc_comments(
    text: &str,
    full_start: usize,
    token_start: usize,
    include_trailing: bool,
) -> impl Iterator<Item = Comment<'_>> {
    jsdoc_comments_controlled(text, full_start, token_start, include_trailing, None)
        .map(|result| result.expect("uncontrolled JSDoc scan"))
}

pub(crate) fn jsdoc_comments_controlled<'s>(
    text: &'s str,
    full_start: usize,
    token_start: usize,
    include_trailing: bool,
    control: Option<&'s Control>,
) -> impl Iterator<Item = Result<Comment<'s>, Stop>> + 's {
    let trivia = &text[..token_start];
    let mut comments = None;
    let mut stopped = false;
    let mut collecting = full_start == 0 || include_trailing;
    std::iter::from_fn(move || {
        if stopped {
            return None;
        }
        if comments.is_none() {
            let start = match shebang_end_controlled(text, control) {
                Ok(start) => start,
                Err(stop) => {
                    stopped = true;
                    return Some(Err(stop));
                }
            };
            let from = full_start.max(start).min(trivia.len());
            comments = Some(comments_controlled(trivia, from, control));
        }
        loop {
            let comment = match comments.as_mut().expect("initialized comments").next()? {
                Ok(comment) => comment,
                Err(stop) => {
                    stopped = true;
                    return Some(Err(stop));
                }
            };
            match Ok::<_, crate::resources::Stop>(comment.space_before.find(['\r', '\n'])) {
                Ok(found) => collecting |= found.is_some(),
                Err(stop) => {
                    stopped = true;
                    return Some(Err(stop));
                }
            }
            if collecting
                && comment
                    .text
                    .strip_prefix("/*")
                    .and_then(|text| text.strip_suffix("*/"))
                    .is_some_and(|body| body.starts_with('*'))
            {
                return Some(Ok(comment));
            }
        }
    })
}

/// TypeScript `scanner.IsIdentifierText` for the standard language variant.
/// Identifier escapes belong to token scanning, not this decoded-text test.
pub(crate) fn is_identifier_text(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(id_start) && chars.all(id_continue)
}

pub(crate) fn id_start(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphabetic() || c == '$' || c == '_'
    } else {
        unicode_id_start::is_id_start(c) && !crate::identifier_exclusions::start(c)
    }
}
pub(crate) fn id_continue(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphanumeric() || c == '$' || c == '_'
    } else {
        c == '\u{200C}'
            || c == '\u{200D}'
            || unicode_id_start::is_id_continue(c) && !crate::identifier_exclusions::part(c)
    }
}

fn unicode_escape<'s>(input: &'s str, control: Option<&Control>) -> IResult<&'s str, &'s str> {
    recognize(pair(
        tag("\\u"),
        alt((
            delimited(
                char('{'),
                take_while1(|c: char| scan_work(control) && c.is_ascii_hexdigit()),
                char('}'),
            ),
            nom::bytes::complete::take_while_m_n(4, 4, |c: char| c.is_ascii_hexdigit()),
        )),
    ))
    .parse(input)
}

fn identifier<'s>(input: &'s str, control: Option<&Control>) -> IResult<&'s str, &'s str> {
    // An uncontrolled scan's fast path: an ASCII identifier followed by an
    // ASCII character other than `\`, which could begin an escape, ends
    // where the general scan below ends it.
    if control.is_none() {
        let bytes = input.as_bytes();
        if bytes.first().copied().is_some_and(ascii_id_start) {
            let end = bytes
                .iter()
                .position(|&byte| !ascii_id_continue(byte))
                .unwrap_or(bytes.len());
            if bytes
                .get(end)
                .is_none_or(|&byte| byte.is_ascii() && byte != b'\\')
            {
                return Ok((&input[end..], &input[..end]));
            }
        }
    }
    recognize(pair(
        alt((
            recognize(nom::character::complete::satisfy(id_start)),
            |input| unicode_escape(input, control),
        )),
        nom::multi::fold_many0(
            alt((
                take_while1(|c| scan_work(control) && id_continue(c)),
                |input| unicode_escape(input, control),
            )),
            || (),
            |(), _| (),
        ),
    ))
    .parse(input)
}

fn quoted<'s>(input: &'s str, quote: char, control: Option<&Control>) -> IResult<&'s str, &'s str> {
    recognize(delimited(
        char(quote),
        nom::combinator::opt(nom::bytes::complete::escaped(
            take_while1(move |c| {
                scan_work(control) && c != quote && c != '\\' && c != '\r' && c != '\n'
            }),
            '\\',
            alt((tag("\r\n"), recognize(anychar))),
        )),
        char(quote),
    ))
    .parse(input)
}

// Nom predicates may stop scanning when the control interrupts. The public
// controlled scan checks that sticky result before publishing any token, so
// an interrupted primitive is never exposed as a grammar mismatch.
fn scan_work(control: Option<&Control>) -> bool {
    control.is_none_or(|control| control.poll().is_ok())
}

impl<'src> Lexer<'src> {
    pub(crate) const fn new(src: &'src str, pos: usize) -> Self {
        Self {
            src,
            pos,
            skip_jsdoc_leading_asterisks: 0,
        }
    }

    /// Native `SetSkipJSDocLeadingAsterisks`: the grammar owns this mode;
    /// ordinary scanning and scanner checkpoints keep their existing policy.
    pub(crate) fn set_jsdoc_leading_asterisks(&mut self, enabled: bool) {
        if enabled {
            self.skip_jsdoc_leading_asterisks += 1;
        } else {
            self.skip_jsdoc_leading_asterisks = self
                .skip_jsdoc_leading_asterisks
                .checked_sub(1)
                .expect("balanced JSDoc scanner mode");
        }
    }

    fn token(
        &mut self,
        kind: Kind,
        start: usize,
        end: usize,
        newline: bool,
        error: Option<&'static str>,
    ) -> Token<'src> {
        self.pos = end;
        Token {
            kind,
            raw: &self.src[start..end],
            start,
            full_start: start,
            end,
            newline,
            error,
        }
    }

    pub fn next(&mut self) -> Token<'src> {
        self.next_controlled(None)
            .expect("uncontrolled scanner cannot stop")
    }

    pub(crate) fn next_controlled(
        &mut self,
        control: Option<&Control>,
    ) -> Result<Token<'src>, Stop> {
        let full_start = self.pos;
        let mut token = self.scan_next(control);
        if let Some(control) = control {
            control.poll()?;
        }
        token.full_start = full_start;
        Ok(token)
    }

    fn scan_next(&mut self, control: Option<&Control>) -> Token<'src> {
        let mut newline = false;
        let mut skipped_jsdoc_asterisk = false;
        loop {
            if !scan_work(control) {
                return self.token(Kind::Eof, self.pos, self.pos, newline, None);
            }
            let rest = &self.src[self.pos..];
            let tail = match control {
                None => {
                    let (length, line_break) = whitespace_run(rest);
                    newline |= line_break;
                    &rest[length..]
                }
                Some(_) => {
                    let (tail, space) = take_while::<_, _, nom::error::Error<&str>>(|c| {
                        scan_work(control) && whitespace(c)
                    })
                    .parse(rest)
                    .unwrap();
                    newline |= space.chars().any(|c| !scan_work(control) || line_break(c));
                    tail
                }
            };
            self.pos = self.src.len() - tail.len();
            if self.pos <= 3
                && tail.starts_with("#!")
                && scan_shebang_end(self.src, control) > self.pos
            {
                self.pos = scan_shebang_end(self.src, control);
            } else if tail.starts_with("//") || tail.starts_with("/*") {
                if let Some(end) = scan_comment_end(self.src, self.pos, control) {
                    if control.is_some_and(|control| control.poll().is_err()) {
                        return self.token(Kind::Eof, self.pos, self.pos, newline, None);
                    }
                    newline |= match control {
                        // A line comment ends before its line break.
                        None => {
                            tail.starts_with("/*")
                                && first_line_break(&self.src[self.pos..end]).is_some()
                        }
                        Some(_) => self.src[self.pos..end]
                            .chars()
                            .any(|c| !scan_work(control) || line_break(c)),
                    };
                    self.pos = end;
                } else {
                    let start = self.pos;
                    return self.token(
                        Kind::Eof,
                        start,
                        self.src.len(),
                        newline,
                        Some("Unterminated block comment"),
                    );
                }
            } else if self.skip_jsdoc_leading_asterisks != 0
                && newline
                && !skipped_jsdoc_asterisk
                && tail.starts_with('*')
                && !tail.starts_with("**")
                && !tail.starts_with("*=")
            {
                skipped_jsdoc_asterisk = true;
                self.pos += 1;
            } else {
                break;
            }
        }
        let start = self.pos;
        let input = &self.src[start..];
        let Some(first) = input.chars().next() else {
            return self.token(Kind::Eof, start, start, newline, None);
        };
        if first == '`' {
            return self.scan_template(start, false, newline, control);
        }
        if first == '"' || first == '\'' {
            return match quoted(input, first, control) {
                Ok((rest, _)) => self.token(
                    Kind::String,
                    start,
                    self.src.len() - rest.len(),
                    newline,
                    None,
                ),
                Err(_) => {
                    let end = input[1..]
                        .char_indices()
                        .find(|(_, c)| !scan_work(control) || line_break(*c))
                        .map(|(at, _)| at)
                        .map(|i| start + i + 1)
                        .unwrap_or(self.src.len());
                    self.token(
                        Kind::String,
                        start,
                        end,
                        newline,
                        Some("Unterminated string literal"),
                    )
                }
            };
        }
        if id_start(first) || first == '\\' || first == '#' {
            let prefix = usize::from(first == '#');
            if let Ok((rest, _)) = identifier(&input[prefix..], control) {
                return self.token(
                    if prefix == 1 {
                        Kind::Private
                    } else {
                        Kind::Word
                    },
                    start,
                    self.src.len() - rest.len(),
                    newline,
                    None,
                );
            }
        }
        if first.is_ascii_digit()
            || (first == '.' && input.as_bytes().get(1).is_some_and(u8::is_ascii_digit))
        {
            return self.number(start, newline, control);
        }
        // Longest match first. Greater-than tokens are split by type argument
        // parsing without rescanning or allocating another token.
        let candidates: &[&str] = match first {
            '>' => &[">>>=", ">>>", ">>=", ">>", ">="],
            '<' => &["<<=", "<<", "<="],
            '=' => &["===", "=>", "=="],
            '!' => &["!==", "!="],
            '*' => &["**=", "**", "*="],
            '&' => &["&&=", "&&", "&="],
            '|' => &["||=", "||", "|="],
            '?' => &["??=", "??", "?."],
            '.' => &["..."],
            '+' => &["++", "+="],
            '-' => &["--", "-="],
            '/' => &["/="],
            '%' => &["%="],
            '^' => &["^="],
            _ => &[],
        };
        for punct in candidates.iter().copied() {
            if input.starts_with(punct)
                && !(punct == "?." && input.as_bytes().get(2).is_some_and(u8::is_ascii_digit))
            {
                return self.token(Kind::Punct, start, start + punct.len(), newline, None);
            }
        }
        if "{}()[];:,.?~+-*/%<>=!&|^@".contains(first) {
            return self.token(Kind::Punct, start, start + first.len_utf8(), newline, None);
        }
        self.token(
            Kind::Unknown,
            start,
            start + first.len_utf8(),
            newline,
            Some("Invalid character"),
        )
    }

    fn number(&mut self, start: usize, newline: bool, control: Option<&Control>) -> Token<'src> {
        let input = &self.src[start..];
        let bytes = input.as_bytes();
        let mut pos;
        let mut error = None;
        let mut fractional = false;
        if bytes[0] == b'0'
            && bytes
                .get(1)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'_')
        {
            error =
                Some("Legacy octal literals and separators after a leading zero are not allowed");
        }
        if bytes[0] == b'0'
            && bytes
                .get(1)
                .is_some_and(|c| matches!(c, b'x' | b'X' | b'o' | b'O' | b'b' | b'B'))
        {
            let radix = match bytes[1] {
                b'x' | b'X' => 16,
                b'o' | b'O' => 8,
                _ => 2,
            };
            pos = 2;
            let (rest, digits) = take_while::<_, _, nom::error::Error<&str>>(|c: char| {
                scan_work(control) && (c.is_digit(radix) || c == '_')
            })
            .parse(&input[pos..])
            .unwrap();
            pos = input.len() - rest.len();
            if !valid_digits(digits, control) {
                error = Some("Invalid digits or numeric separator");
            }
        } else {
            let (rest, digits) = take_while::<_, _, nom::error::Error<&str>>(|c: char| {
                scan_work(control) && (c.is_ascii_digit() || c == '_')
            })
            .parse(input)
            .unwrap();
            pos = input.len() - rest.len();
            if !digits.is_empty() && !valid_digits(digits, control) {
                error = Some("Invalid numeric separator");
            }
            if bytes.get(pos) == Some(&b'.') {
                fractional = true;
                pos += 1;
                let (rest, digits) = take_while::<_, _, nom::error::Error<&str>>(|c: char| {
                    scan_work(control) && (c.is_ascii_digit() || c == '_')
                })
                .parse(&input[pos..])
                .unwrap();
                pos = input.len() - rest.len();
                if !digits.is_empty() && !valid_digits(digits, control) {
                    error = Some("Invalid numeric separator");
                }
            }
            if bytes.get(pos).is_some_and(|c| *c == b'e' || *c == b'E') {
                fractional = true;
                pos += 1;
                if bytes.get(pos).is_some_and(|c| *c == b'+' || *c == b'-') {
                    pos += 1;
                }
                let (rest, digits) = take_while::<_, _, nom::error::Error<&str>>(|c: char| {
                    scan_work(control) && (c.is_ascii_digit() || c == '_')
                })
                .parse(&input[pos..])
                .unwrap();
                pos = input.len() - rest.len();
                if !valid_digits(digits, control) {
                    error = Some("Invalid numeric exponent");
                }
            }
        }
        if bytes.get(pos) == Some(&b'n') {
            if fractional {
                error = Some("BigInt literal must be an integer");
            }
            pos += 1;
        }
        if input[pos..].chars().next().is_some_and(id_continue) {
            error = Some("Identifier cannot immediately follow a number");
        }
        self.token(Kind::Number, start, start + pos, newline, error)
    }

    pub(crate) fn template_controlled(
        &mut self,
        start: usize,
        continuation: bool,
        newline: bool,
        control: Option<&Control>,
    ) -> Result<Token<'src>, Stop> {
        let token = self.scan_template(start, continuation, newline, control);
        if let Some(control) = control {
            control.poll()?;
        }
        Ok(token)
    }

    fn scan_template(
        &mut self,
        start: usize,
        continuation: bool,
        newline: bool,
        control: Option<&Control>,
    ) -> Token<'src> {
        let bytes = self.src.as_bytes();
        let mut pos = start + 1;
        while pos < bytes.len() && scan_work(control) {
            match bytes[pos] {
                b'\\' => {
                    pos += 1;
                    if pos < bytes.len() {
                        pos += self.src[pos..].chars().next().unwrap().len_utf8();
                    }
                }
                b'`' => {
                    return self.token(
                        if continuation {
                            Kind::TemplateTail
                        } else {
                            Kind::NoSubstitutionTemplate
                        },
                        start,
                        pos + 1,
                        newline,
                        None,
                    );
                }
                b'$' if bytes.get(pos + 1) == Some(&b'{') => {
                    return self.token(
                        if continuation {
                            Kind::TemplateMiddle
                        } else {
                            Kind::TemplateHead
                        },
                        start,
                        pos + 2,
                        newline,
                        None,
                    );
                }
                _ => pos += self.src[pos..].chars().next().unwrap().len_utf8(),
            }
        }
        self.token(
            Kind::TemplateTail,
            start,
            self.src.len(),
            newline,
            Some("Unterminated template literal"),
        )
    }

    pub(crate) fn regex_controlled(
        &mut self,
        start: usize,
        control: Option<&Control>,
    ) -> Result<Token<'src>, Stop> {
        let token = self.scan_regex(start, control);
        if let Some(control) = control {
            control.poll()?;
        }
        Ok(token)
    }

    fn scan_regex(&mut self, start: usize, control: Option<&Control>) -> Token<'src> {
        let mut pos = start + 1;
        let bytes = self.src.as_bytes();
        let mut in_class = false;
        while pos < bytes.len() && scan_work(control) {
            let c = self.src[pos..].chars().next().unwrap();
            if line_break(c) {
                break;
            }
            match c {
                '\\' => {
                    pos += 1;
                    if pos < bytes.len() {
                        pos += self.src[pos..].chars().next().unwrap().len_utf8();
                    }
                }
                '[' => {
                    in_class = true;
                    pos += 1;
                }
                ']' => {
                    in_class = false;
                    pos += 1;
                }
                '/' if !in_class => {
                    pos += 1;
                    let (rest, _) = take_while::<_, _, nom::error::Error<&str>>(|c| {
                        scan_work(control) && id_continue(c)
                    })
                    .parse(&self.src[pos..])
                    .unwrap();
                    return self.token(
                        Kind::String,
                        start,
                        self.src.len() - rest.len(),
                        false,
                        None,
                    );
                }
                _ => pos += c.len_utf8(),
            }
        }
        self.token(
            Kind::Unknown,
            start,
            pos,
            false,
            Some("Unterminated regular expression literal"),
        )
    }
}

fn valid_digits(digits: &str, control: Option<&Control>) -> bool {
    !digits.is_empty()
        && !digits.starts_with('_')
        && !digits.ends_with('_')
        && !digits
            .as_bytes()
            .windows(2)
            .any(|pair| !scan_work(control) || pair == b"__")
}

#[cfg(test)]
mod tests {
    use super::{Lexer, line_break};

    /// An uncontrolled scan takes the byte fast paths for whitespace,
    /// comments and identifiers; a controlled one takes the general scan.
    /// Both give the same tokens for text made of every character class the
    /// rules separate: ASCII and other whitespace, each line break (CR, LF,
    /// CRLF, U+2028, U+2029) and other characters encoded from 0xE2, a byte
    /// order mark, comments, escapes and non-ASCII identifier characters.
    #[test]
    fn fast_scans_find_the_general_scans_tokens() {
        const PIECES: &[&str] = &[
            " ",
            "\t",
            "\u{b}",
            "\u{c}",
            "\n",
            "\r",
            "\r\n",
            "\u{2028}",
            "\u{2029}",
            "\u{a0}",
            "\u{feff}",
            "\u{2000}",
            "\u{2026}",
            "\u{2030}",
            "\u{20ac}",
            "\u{2027}",
            "//",
            "/*",
            "*/",
            "*",
            "/",
            "a",
            "Z",
            "$",
            "_",
            "9",
            "\u{e9}",
            "\u{3bb}",
            "\u{1f600}",
            "\\u0061",
            "\\u{62}",
            "x\\u0063",
            "\\",
            "#",
            "\"",
            "'",
            "`",
            "=",
            ".",
            "#!",
        ];
        let control = crate::resources::Control::new(crate::resources::Limits::default(), None);
        let mut state = 0x2545_f491_u32;
        for case in 0..3_000 {
            let mut text = String::new();
            let length = (case % 41) + 1;
            for _ in 0..length {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                text.push_str(PIECES[state as usize % PIECES.len()]);
            }
            let (mut fast, mut general) = (Lexer::new(&text, 0), Lexer::new(&text, 0));
            for _ in 0..=text.len() {
                let expected = general.next_controlled(Some(&control)).unwrap();
                let actual = fast.next();
                let facts = |token: super::Token<'_>| {
                    (
                        format!("{:?}", token.kind),
                        token.start,
                        token.full_start,
                        token.end,
                        token.newline,
                        token.error,
                    )
                };
                assert_eq!(facts(actual), facts(expected), "{text:?}");
                if matches!(expected.kind, super::Kind::Eof) {
                    break;
                }
            }
            assert_eq!(
                super::first_line_break(&text),
                text.char_indices()
                    .find(|&(_, c)| line_break(c))
                    .map(|(at, _)| at),
                "{text:?}"
            );
        }
    }

    /// Each per-byte class is its owner's class of ASCII characters, and
    /// holds no other byte.
    #[test]
    fn ascii_classes_are_their_owners() {
        for byte in 0..=u8::MAX {
            let owner = |class: fn(char) -> bool| byte.is_ascii() && class(byte as char);
            assert_eq!(
                super::ascii_line_break(byte),
                owner(super::line_break),
                "{byte:#x}"
            );
            assert_eq!(
                super::ascii_whitespace(byte),
                owner(super::whitespace),
                "{byte:#x}"
            );
            assert_eq!(
                super::ascii_id_start(byte),
                owner(super::id_start),
                "{byte:#x}"
            );
            assert_eq!(
                super::ascii_id_continue(byte),
                owner(super::id_continue),
                "{byte:#x}"
            );
        }
    }

    #[test]
    fn identifiers_match_the_native_compiler_across_unicode() {
        #[derive(serde::Deserialize)]
        struct Ranges {
            start: Vec<[u32; 2]>,
            part: Vec<[u32; 2]>,
        }
        let golden: Ranges =
            serde_json::from_str(include_str!("../fixtures/identifier-ranges.json")).unwrap();
        let ranges = [&golden.start, &golden.part];
        let mut cursor = [0, 0];
        for cp in 0..0x110000 {
            let mut expected = 0;
            for bit in 0..2 {
                while cursor[bit] < ranges[bit].len() && ranges[bit][cursor[bit]][1] < cp {
                    cursor[bit] += 1;
                }
                if cursor[bit] < ranges[bit].len() && ranges[bit][cursor[bit]][0] <= cp {
                    expected |= 1 << bit;
                }
            }
            let actual = char::from_u32(cp).map_or(0, |c| {
                u8::from(super::id_start(c)) | (u8::from(super::id_continue(c)) << 1)
            });
            assert_eq!(actual, expected, "Identifier classification at U+{cp:04X}");
        }
    }
}
