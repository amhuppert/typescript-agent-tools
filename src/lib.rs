//! A borrowing, arena-based TypeScript syntax parser.
//!
//! ```
//! let ast = typescript_parser::parse("const value: number = 42;")?;
//! assert_eq!(ast.root().kind, "SourceFile");
//! # Ok::<(), typescript_parser::Diagnostic>(())
//! ```

pub mod ast;
mod compiler;
mod identifier_exclusions;
mod json;
mod lexer;
mod parser;
mod pragmas;
pub mod references;
mod syntax_kind;
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

/// TypeScript's `getScriptKindFromFileName` for TSX and JSX: the text after
/// the last `.`, without case.
fn jsx_file_name(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        extension.eq_ignore_ascii_case("tsx") || extension.eq_ignore_ascii_case("jsx")
    })
}

/// TypeScript's declaration file names: `.d.ts`, `.d.mts`, `.d.cts`, or an
/// arbitrary-extension declaration such as `styles.d.css.ts`.
pub(crate) fn declaration_file_name(name: &str) -> bool {
    [".d.ts", ".d.mts", ".d.cts"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
        || name.ends_with(".ts")
            && name
                .rsplit(['/', '\\'])
                .next()
                .is_some_and(|base| base.contains(".d."))
}

pub fn parse(source: &str) -> Result<Ast<'_>, Diagnostic> {
    parse_with_options(source, ParseOptions::default())
}

pub fn parse_with_options(source: &str, options: ParseOptions) -> Result<Ast<'_>, Diagnostic> {
    parser::Parser::new(source, options).parse()
}
