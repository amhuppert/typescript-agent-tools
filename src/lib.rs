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
mod syntax_kind;
pub use ast::{Ast, Diagnostic, Fields, Node, NodeId, NodeView, Span, Value};

#[derive(Debug, Clone)]
pub struct ParseOptions {
    pub jsx: bool,
    pub file_name: String,
    /// Bound recursive grammar entries for untrusted input, rather than AST
    /// levels. Raising this budget requires a correspondingly larger stack.
    pub max_depth: usize,
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
    pub(crate) fn declaration_file(&self) -> bool {
        [".d.ts", ".d.mts", ".d.cts"]
            .iter()
            .any(|suffix| self.file_name.ends_with(suffix))
            || self.file_name.ends_with(".ts")
                && self
                    .file_name
                    .rsplit(['/', '\\'])
                    .next()
                    .is_some_and(|name| name.contains(".d."))
    }
}

pub fn parse(source: &str) -> Result<Ast<'_>, Diagnostic> {
    parse_with_options(source, ParseOptions::default())
}

pub fn parse_with_options(source: &str, options: ParseOptions) -> Result<Ast<'_>, Diagnostic> {
    parser::Parser::new(source, options).parse()
}
