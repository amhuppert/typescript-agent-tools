//! Declarations, scopes, aliases, and the semantic index.
//!
//! `collect` records one file's owned facts while its AST is alive. `bind`
//! completes project-wide declaration tables, export and alias relationships,
//! and occurrence resolution, then freezes the index served to queries.

mod collect;
mod ids;
mod index;
mod instances;
mod link;
mod syntax;
mod targets;

use super::source::{FileId, SourceText};
use super::{ProjectError, project::Program};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, Node, Span};

/// A module one file requests: a specifier written in it, or the automatic
/// JSX runtime that the project adds for its configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModuleRequest {
    pub(super) specifier: String,
    pub(super) kind: RequestKind,
    /// An explicit `resolution-mode` import attribute.
    pub(super) mode: Option<ResolutionMode>,
    /// Where diagnostics report the request: the specifier's string literal
    /// token, or the start of the first JSX tag for the JSX runtime.
    pub(super) span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RequestKind {
    /// `import ... from`, including side-effect imports without bindings.
    Import { side_effect: bool },
    /// `export ... from`.
    ReExport,
    /// `import name = require(...)`.
    Require,
    /// `import("...")` in a type.
    ImportType,
    /// `import("...")` in an expression.
    Dynamic,
    /// `declare module "..."` inside a module augments another module.
    Augmentation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResolutionMode {
    Import,
    Require,
}

/// File-level settings decided by the project configuration.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct FileOptions {
    /// The configuration and file format force module semantics.
    pub(super) force_module: bool,
    /// `jsx: react-jsx` makes a file that uses JSX a module.
    pub(super) jsx_module: bool,
}

/// Owned semantic facts captured from one parsed file.
#[derive(Debug, Default)]
pub(super) struct FileFacts {
    /// Whether top-level declarations are module-local rather than global.
    pub(super) module: bool,
    /// A declaration file, whose modules may describe CommonJS.
    pub(super) declaration: bool,
    /// The start of the first JSX element or fragment in source order.
    pub(super) jsx: Option<usize>,
    pub(super) requests: Vec<ModuleRequest>,
    /// End offsets of `@ts-ignore` and `@ts-expect-error` comments.
    directives: Vec<u32>,
    facts: collect::Facts,
}

/// How the project resolved one `ModuleRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ModuleResolution {
    File(FileId),
    /// No program input; an ambient module declaration may still satisfy it.
    Missing,
    /// An unresolved side-effect import that the configuration does not
    /// check (`noUncheckedSideEffectImports: false`).
    Unchecked,
    /// The specifier resolves only to an input outside the supported profile.
    Unsupported(String),
}

pub(super) use ids::SymbolId;
pub(super) use index::{Index, Occurrence, Target};

/// Occurrence role bits.
pub(super) mod roles {
    pub(crate) use super::collect::{DECLARATION, EXPORT, IMPORT, USE};
}

/// Meanings a name can have. A class or enum has several.
pub(super) mod meaning {
    pub(crate) const VALUE: u8 = 1;
    pub(crate) const TYPE: u8 = 2;
    pub(crate) const NAMESPACE: u8 = 4;
    pub(crate) const ALL: u8 = VALUE | TYPE | NAMESPACE;
}

/// Capture one file's semantic facts from `ast`, parsed from `source`.
/// Errors carry a position but no path.
pub(super) fn collect(
    ast: &Ast<'_>,
    source: &SourceText,
    options: FileOptions,
) -> Result<FileFacts, ProjectError> {
    let module = is_module(ast, options);
    let (facts, requests) = collect::Collector::new(ast, source, module).run()?;
    Ok(FileFacts {
        module,
        declaration: syntax::flag(ast.root(), "isDeclarationFile"),
        jsx: first_jsx(ast),
        requests,
        directives: comment_directives(ast),
        facts,
    })
}

/// Link every captured file into the frozen semantic index.
pub(super) fn bind(program: &Program) -> Result<Index, ProjectError> {
    link::link(program)
}

/// TypeScript's external-module indicators (`crate::ast::external_module`),
/// plus configuration and format rules supplied by the project.
fn is_module(ast: &Ast<'_>, options: FileOptions) -> bool {
    let root = ast.root();
    options.force_module
        || crate::ast::external_module(ast.nodes(), syntax::ids(root, "statements"), root.flags)
        || options.jsx_module && first_jsx(ast).is_some()
}

pub(super) fn has_modifier<'s>(ast: &Ast<'s>, node: &Node<'s>, kind: SyntaxKind) -> bool {
    syntax::ids(node, "modifiers")
        .iter()
        .any(|&id| ast.node(id).syntax_kind == kind)
}

fn first_jsx(ast: &Ast<'_>) -> Option<usize> {
    ast.walk()
        .find(|&id| {
            matches!(
                ast.node(id).syntax_kind,
                SyntaxKind::JsxElement
                    | SyntaxKind::JsxSelfClosingElement
                    | SyntaxKind::JsxFragment
            )
        })
        .map(|id| ast.token_span(id).start)
}

impl FileFacts {
    /// End offsets of real comment directives.
    pub(super) fn comment_directives(&self) -> &[u32] {
        &self.directives
    }
}

/// End offsets of `@ts-ignore` and `@ts-expect-error` comments anywhere in
/// the file, as TypeScript's scanner records them. Outside string, template,
/// regular expression, and JSX text tokens, `//` and `/*` always begin a
/// comment, so the source is scanned around those tokens.
fn comment_directives(ast: &Ast<'_>) -> Vec<u32> {
    let text = ast.source();
    let mut literals: Vec<Span> = ast
        .walk()
        .filter(|&id| {
            matches!(
                ast.node(id).syntax_kind,
                SyntaxKind::StringLiteral
                    | SyntaxKind::NoSubstitutionTemplateLiteral
                    | SyntaxKind::TemplateHead
                    | SyntaxKind::TemplateMiddle
                    | SyntaxKind::TemplateTail
                    | SyntaxKind::RegularExpressionLiteral
                    | SyntaxKind::JsxText
            )
        })
        .map(|id| ast.token_span(id))
        .collect();
    literals.sort_by_key(|span| span.start);
    let mut literals = literals.into_iter().peekable();
    let mut directives = Vec::new();
    // A `#!` line is not a comment.
    let mut at = crate::lexer::shebang_end(text);
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
        let Some(end) = crate::lexer::comment_end(text, start) else {
            if text[start..].starts_with("/*") {
                break;
            }
            at = start + 1;
            continue;
        };
        let comment = &text[start..end];
        let body = if comment.starts_with("//") {
            comment.trim_start_matches('/')
        } else {
            // As in TypeScript, a block comment's directive is on its last line.
            let comment = &comment[..comment.len() - 2];
            let last_line = comment
                .rfind(crate::lexer::line_break)
                .map_or(comment, |at| &comment[at..]);
            last_line.trim_start().trim_start_matches(['/', '*'])
        };
        // TypeScript 7's directive pattern allows only ASCII whitespace here.
        let body = body.trim_start_matches([' ', '\t', '\x0c']);
        if body.starts_with("@ts-ignore") || body.starts_with("@ts-expect-error") {
            directives.push(end as u32);
        }
        at = end;
    }
    directives
}
