//! Declarations, scopes, aliases, and the semantic index.
//!
//! `collect` records one file's owned facts while its AST is alive. `bind`
//! completes project-wide declaration tables, export and alias relationships,
//! and occurrence resolution, then freezes the index served to queries.

mod collect;
mod index;
mod link;
mod syntax;

use super::source::{FileId, SourceText, token_span};
use super::{ProjectError, project::Program};
use crate::{Ast, Node, NodeId, Span};

/// A module specifier written in one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModuleRequest {
    pub(super) specifier: String,
    pub(super) kind: RequestKind,
    /// An explicit `resolution-mode` import attribute.
    pub(super) mode: Option<ResolutionMode>,
    /// The specifier string literal, for diagnostics.
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

pub(super) use index::{Index, SymbolId, Target};

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

/// Capture one file's semantic facts. Errors carry a position but no path.
pub(super) fn collect(
    _file: FileId,
    ast: &Ast<'_>,
    options: FileOptions,
) -> Result<FileFacts, ProjectError> {
    let module = is_module(ast, options);
    let (facts, requests) = collect::Collector::new(ast, module).run()?;
    Ok(FileFacts {
        module,
        declaration: ast
            .root()
            .field("isDeclarationFile")
            .is_some_and(|value| matches!(value, crate::Value::Bool(true))),
        jsx: first_jsx(ast),
        requests,
        facts,
    })
}

/// Link every captured file into the frozen semantic index.
pub(super) fn bind(program: &Program) -> Result<Index, ProjectError> {
    link::link(program)
}

/// TypeScript's external-module indicators, plus configuration and format
/// rules supplied by the project.
fn is_module(ast: &Ast<'_>, options: FileOptions) -> bool {
    let root = ast.root();
    options.force_module
        || root.flags & crate::compiler::node::POSSIBLY_CONTAINS_IMPORT_META != 0
        || syntax::ids(root, "statements")
            .iter()
            .any(|&id| module_indicator(ast, ast.node(id)))
        || options.jsx_module && first_jsx(ast).is_some()
}

fn module_indicator<'s>(ast: &Ast<'s>, node: &Node<'s>) -> bool {
    match node.kind {
        "ImportDeclaration" | "ExportDeclaration" | "ExportAssignment" => true,
        "ImportEqualsDeclaration" => syntax::child(ast, node, "moduleReference")
            .is_some_and(|reference| reference.kind == "ExternalModuleReference"),
        _ => has_modifier(ast, node, "ExportKeyword"),
    }
}

pub(super) fn has_modifier<'s>(ast: &Ast<'s>, node: &Node<'s>, kind: &str) -> bool {
    syntax::ids(node, "modifiers")
        .iter()
        .any(|&id| ast.node(id).kind == kind)
}

fn first_jsx(ast: &Ast<'_>) -> Option<usize> {
    reachable(ast)
        .map(|id| ast.node(id))
        .find(|node| {
            matches!(
                node.kind,
                "JsxElement" | "JsxSelfClosingElement" | "JsxFragment"
            )
        })
        .map(|node| token_span(ast.source(), node.span).start)
}

/// The lines whose errors `@ts-ignore` and `@ts-expect-error` comments
/// suppress, by TypeScript's rule: a directive comment ends on a preceding
/// line, and only blank or line-comment lines lie between.
#[derive(Debug, Default)]
pub(super) struct Suppressions {
    /// Indexed by zero-based line; empty without directives.
    covered: Vec<bool>,
}

impl Suppressions {
    pub(super) fn new(source: &SourceText, facts: &FileFacts) -> Self {
        let directives = &facts.facts.directives;
        if directives.is_empty() {
            return Self::default();
        }
        let mut directive = vec![false; source.line_count()];
        for &end in directives {
            directive[source.line_index(end as usize)] = true;
        }
        let mut covered = vec![false; directive.len()];
        for line in 1..covered.len() {
            let above = line - 1;
            covered[line] = directive[above]
                || covered[above] && {
                    let text = source.line(line).unwrap_or("").trim();
                    text.is_empty() || text.starts_with("//")
                };
        }
        Self { covered }
    }

    /// Whether errors at `offset` are suppressed.
    pub(super) fn covers(&self, source: &SourceText, offset: usize) -> bool {
        !self.covered.is_empty() && self.covered[source.line_index(offset)]
    }
}

/// Nodes reachable from the root, visited with an explicit stack.
fn reachable<'a>(ast: &'a Ast<'_>) -> impl Iterator<Item = NodeId> + 'a {
    let mut stack = vec![ast.root_id()];
    std::iter::from_fn(move || {
        let id = stack.pop()?;
        let start = stack.len();
        stack.extend(syntax::children(ast.node(id)));
        stack[start..].reverse();
        Some(id)
    })
}
