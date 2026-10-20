//! A borrowing, arena-based TypeScript syntax parser, with project-wide
//! reference lookup and declaration navigation built on it.
//!
//! ```
//! let ast = typescript_parser::parse("const value: number = 42;")?;
//! assert_eq!(ast.root().kind, "SourceFile");
//! # Ok::<(), typescript_parser::Diagnostic>(())
//! ```
//!
//! - `project`: one configured project, as an immutable snapshot and a
//!   refreshable session.
//! - `references`: reference queries and their results.
//! - `navigation`: declaration outlines and selections, for one file or a
//!   project's searched files.
//! - `editing`: source-preserving body replacement and statement insertion,
//!   prepared as a batch, previewed, and applied.
//! - `analysis`: the failures every operation reports (`AnalysisError`).
//! - `source`: positions in analyzed text (`SourcePosition`).

pub mod analysis;
pub mod ast;
mod compiler;
pub mod editing;
mod hash;
mod identifier_exclusions;
mod input;
mod json;
mod lexer;
pub mod navigation;
mod nonblocking;
mod numbers;
mod parser;
mod pragmas;
mod printer;
pub mod project;
pub mod references;
mod resources;
pub mod source;
mod spelling;
mod strings;
mod syntax;
mod syntax_kind;
pub mod types;
pub use ast::{Ast, Diagnostic, Fields, Node, NodeId, NodeView, Span, Value};

#[derive(Debug, Clone)]
pub struct ParseOptions {
    pub jsx: bool,
    pub file_name: String,
    /// Bound recursive grammar entries for untrusted input, rather than AST
    /// levels, including entries made by interpretations that are later
    /// discarded; exceeding it returns a diagnostic. The default suits an
    /// ordinary thread stack; a larger budget needs a larger stack, such as
    /// `DEEP_MAX_DEPTH` on a thread of `DEEP_PARSE_STACK_BYTES`.
    pub max_depth: usize,
}

impl ParseOptions {
    /// A budget for deeply nested generated code, such as declaration files
    /// with nested schema types. Parse with it on a thread whose stack is at
    /// least `DEEP_PARSE_STACK_BYTES`.
    pub const DEEP_MAX_DEPTH: usize = 1024;
}

/// The thread stack that parsing with `ParseOptions::DEEP_MAX_DEPTH` needs,
/// including unoptimized builds. It is reserved address space, committed only
/// as used.
pub const DEEP_PARSE_STACK_BYTES: usize = 256 << 20;

/// The nearest of `candidates` to `name` by TypeScript's spelling rule
/// (`core.GetSpellingSuggestion`, owned by `spelling`), or `None` when none
/// is close; of equally near candidates, the earliest. TypeScript asks this
/// of an unknown command-line option against its option names
/// (`createUnknownOptionError`), so the `tsparse` binary asks it here
/// rather than keeping a second copy of the rule.
#[doc(hidden)]
pub fn spelling_suggestion<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    spelling::suggestion(
        name,
        candidates
            .into_iter()
            .enumerate()
            .map(|(index, candidate)| ((index, candidate), candidate.to_owned())),
        |(left, _), (right, _)| left.cmp(&right),
    )
    .map(|(_, candidate)| candidate)
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            jsx: false,
            file_name: "source.ts".into(),
            max_depth: 256,
        }
    }
}

impl ParseOptions {
    /// The options TypeScript derives from a file's name, with the default
    /// budget: JSX for a `.tsx` or `.jsx` extension in any case
    /// (`getScriptKindFromFileName`), and declaration context from the exact
    /// name (`isDeclarationFileName`), so `G.D.TS` is not a declaration file.
    pub fn for_file(file_name: impl Into<String>) -> Self {
        let file_name = file_name.into();
        Self {
            jsx: jsx_file_name(&file_name),
            file_name,
            ..Self::default()
        }
    }

    pub(crate) fn declaration_file(&self) -> bool {
        declaration_file_name(&self.file_name)
    }
}

/// How TypeScript parses a file, by its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScriptKind {
    Ts,
    Tsx,
    Js,
    Jsx,
    Json,
    Unknown,
}

/// TypeScript's `GetScriptKindFromFileName`: the text after the last `.`,
/// without case (`strings.ToLower`, which for these extensions is ASCII
/// case folding), so `X.JSON` parses as JSON and `A.TSX` with JSX under any
/// file-system case policy. Whether such a file is a supported input is a
/// separate question that follows the case policy (`input::InputKind`).
pub(crate) fn script_kind(file_name: &str) -> ScriptKind {
    let Some((_, extension)) = file_name.rsplit_once('.') else {
        return ScriptKind::Unknown;
    };
    [
        ("js", ScriptKind::Js),
        ("cjs", ScriptKind::Js),
        ("mjs", ScriptKind::Js),
        ("jsx", ScriptKind::Jsx),
        ("ts", ScriptKind::Ts),
        ("cts", ScriptKind::Ts),
        ("mts", ScriptKind::Ts),
        ("tsx", ScriptKind::Tsx),
        ("json", ScriptKind::Json),
    ]
    .into_iter()
    .find(|(name, _)| extension.eq_ignore_ascii_case(name))
    .map_or(ScriptKind::Unknown, |(_, kind)| kind)
}

/// Whether a file's script kind parses JSX.
fn jsx_file_name(name: &str) -> bool {
    matches!(script_kind(name), ScriptKind::Tsx | ScriptKind::Jsx)
}

/// TypeScript's declaration file names: `.d.ts`, `.d.mts`, `.d.cts`, or an
/// arbitrary-extension declaration such as `styles.d.css.ts`.
pub(crate) fn declaration_file_name(name: &str) -> bool {
    declaration_file_extension(name).is_some()
}

/// Native `GetDeclarationFileExtension`, shared by parse context, loading,
/// and filename rewriting. Arbitrary forms start at the basename's first .d.
pub(crate) fn declaration_file_extension(name: &str) -> Option<&str> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    [".d.ts", ".d.cts", ".d.mts"]
        .into_iter()
        .find(|suffix| base.ends_with(suffix))
        .or_else(|| {
            base.ends_with(".ts")
                .then(|| base.find(".d.").map(|at| &base[at..]))
                .flatten()
        })
}

pub fn parse(source: &str) -> Result<Ast<'_>, Diagnostic> {
    parse_with_options(source, ParseOptions::default())
}

pub fn parse_with_options(source: &str, options: ParseOptions) -> Result<Ast<'_>, Diagnostic> {
    parser::Parser::new(source, options).parse()
}
