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
mod store;
mod targets;
mod text;

use super::program::Program;
use super::sources::FileId;
use crate::analysis::InputError;
use crate::parser::json::{JsonKind, JsonString, JsonText, JsonValue};
use crate::source::SourceText;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, Span};
use std::collections::HashMap;

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
    /// `declare module "..."` augmenting another module (see
    /// `collect::ModuleContext`). TypeScript reports a missing augmented
    /// module (TS2664) only where the declaration's parent is not `ambient`:
    /// in a module that is not a declaration file.
    Augmentation { ambient: bool },
}

impl RequestKind {
    /// Where TypeScript's loader takes a request among one file's module
    /// names (`resolveImportsAndModuleAugmentations`), after the file's
    /// reference directives and the automatic JSX runtime: the static imports
    /// and re-exports `collectModuleReferences` lists, then the dynamic
    /// imports and import types `ForEachDynamicImportOrRequireCall` finds,
    /// then module augmentations, for which TypeScript adds no file. Within
    /// a rank, the loader takes them in source order: the order of the
    /// statements for the first, and of the `import` keywords that
    /// `ForEachDynamicImportOrRequireCall` finds in the text for the
    /// second, which is the order of their specifiers. `FileFacts::requests`
    /// keeps them in the order binding visits them, which is not source
    /// order.
    pub(super) fn subtask_rank(self) -> u8 {
        match self {
            RequestKind::Import { .. } | RequestKind::ReExport | RequestKind::Require => 0,
            RequestKind::ImportType | RequestKind::Dynamic => 1,
            RequestKind::Augmentation { .. } => 2,
        }
    }
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
    /// Where `@ts-ignore` and `@ts-expect-error` comment directives begin
    /// (`crate::pragmas::comment_directives`).
    directives: Vec<u32>,
    facts: collect::Facts,
    /// A JSON source file's module, which declares no names.
    json: Option<JsonModule>,
}

/// What linking needs from a JSON source file. TypeScript's binder gives it
/// exactly one export, `export=`, a property whose declaration is the file
/// and whose value is the file's root (`bindSourceFileIfExternalModule`).
/// A name selects a property of a value's type (`getPropertyOfType`): a key
/// of an object, recorded here along every path of keys, or a member of the
/// library interface that is a string's, number's, boolean's, or array's
/// apparent type, which linking looks up (`JsonShape::interface`).
#[derive(Debug)]
pub(super) struct JsonModule {
    /// The file's value at `JsonModule::ROOT`, then each value an object key
    /// reaches from it; an array's elements have no names.
    values: Vec<JsonShape>,
}

/// The global interfaces that are the apparent types of JSON values
/// (`JsonShape::interface`), whose declarations' members collection records.
const APPARENT_INTERFACES: [&str; 4] = ["String", "Number", "Boolean", "Array"];

/// One JSON value as names see it.
#[derive(Debug)]
pub(super) enum JsonShape {
    /// The value of an empty or trivia-only file, which has no properties.
    Empty,
    Null,
    String,
    Number,
    Boolean,
    Array,
    /// Each distinct exact key once, in source order, with the index of its
    /// last value: a repeated key is one property, of the last value's type.
    Object(Vec<(JsonString, u32)>),
}

impl JsonShape {
    /// The global interface that is this value's apparent type, whose members
    /// are its properties (`getApparentType`); `Object`'s own members are
    /// left out (`skipObjectFunctionPropertyAugment`).
    pub(super) fn interface(&self) -> Option<&'static str> {
        match self {
            JsonShape::String => Some("String"),
            JsonShape::Number => Some("Number"),
            JsonShape::Boolean => Some("Boolean"),
            JsonShape::Array => Some("Array"),
            JsonShape::Empty | JsonShape::Null | JsonShape::Object(_) => None,
        }
    }
}

impl JsonModule {
    /// The file's value, its `export=`.
    pub(super) const ROOT: u32 = 0;

    fn new(root: Option<&JsonValue>) -> Self {
        let mut module = JsonModule { values: Vec::new() };
        // Breadth first, so each value's index is the order it was queued in.
        let mut pending = std::collections::VecDeque::from([root]);
        while let Some(value) = pending.pop_front() {
            let shape = match value.map(|value| &value.kind) {
                None => JsonShape::Empty,
                Some(JsonKind::Null) => JsonShape::Null,
                Some(JsonKind::Bool(_)) => JsonShape::Boolean,
                Some(JsonKind::Number) => JsonShape::Number,
                Some(JsonKind::String(_)) => JsonShape::String,
                Some(JsonKind::Array(_)) => JsonShape::Array,
                Some(JsonKind::Object(properties)) => {
                    let mut keys: Vec<(&JsonString, &JsonValue)> = Vec::new();
                    let mut positions: HashMap<&JsonString, usize> = HashMap::new();
                    for property in properties {
                        match positions.get(&property.key) {
                            Some(&index) => keys[index].1 = &property.value,
                            None => {
                                positions.insert(&property.key, keys.len());
                                keys.push((&property.key, &property.value));
                            }
                        }
                    }
                    let next = module.values.len() + 1 + pending.len();
                    let shape = JsonShape::Object(
                        keys.iter()
                            .enumerate()
                            .map(|(index, (key, _))| ((*key).clone(), (next + index) as u32))
                            .collect(),
                    );
                    pending.extend(keys.into_iter().map(|(_, value)| Some(value)));
                    shape
                }
            };
            module.values.push(shape);
        }
        module
    }

    pub(super) fn shape(&self, value: u32) -> &JsonShape {
        &self.values[value as usize]
    }

    /// The value of the property `name` of an object `value`, compared
    /// exactly.
    pub(super) fn property(&self, value: u32, name: &str) -> Option<u32> {
        match self.shape(value) {
            JsonShape::Object(keys) => keys
                .iter()
                .find(|(key, _)| key.as_str() == Some(name))
                .map(|&(_, child)| child),
            _ => None,
        }
    }
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
    /// Resolution found a file that the program does not admit for this
    /// importer (`project::resolve::admission`), such as an arbitrary-extension
    /// declaration without `allowArbitraryExtensions`: as for `Missing`, an
    /// ambient module may still satisfy it, and otherwise the error is this
    /// message.
    NotAdmitted(String),
}

pub(super) use ids::SymbolId;
pub(super) use index::{Index, IndexedLocation, Occurrence, Target};

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
) -> Result<FileFacts, InputError> {
    let module = is_module(ast, options);
    let (facts, requests) = collect::Collector::new(ast, source, module).run()?;
    Ok(FileFacts {
        module,
        declaration: syntax::flag(ast.root(), "isDeclarationFile"),
        jsx: first_jsx(ast),
        requests,
        directives: comment_directives(ast.source(), ast.literal_token_spans()),
        facts,
        json: None,
    })
}

/// The facts of a JSON source file, read from `text`: always a module
/// (`bindSourceFileIfExternalModule`), with no declarations, names, or
/// module requests of its own, and comment directives as in any file.
pub(super) fn collect_json(text: &JsonText, source: &str) -> FileFacts {
    FileFacts {
        module: true,
        directives: comment_directives(source, text.literal_spans()),
        json: Some(JsonModule::new(text.root.as_ref())),
        ..FileFacts::default()
    }
}

/// Link every captured file into the frozen semantic index.
pub(super) fn bind(program: &Program) -> Result<Index, InputError> {
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
    /// Where the file's comment directives begin.
    pub(super) fn comment_directives(&self) -> &[u32] {
        &self.directives
    }
}

/// The comment directives of `text`, outside its `literals`, as stored.
fn comment_directives(text: &str, literals: Vec<Span>) -> Vec<u32> {
    crate::pragmas::comment_directives(text, literals)
        .into_iter()
        .map(|start| start as u32)
        .collect()
}
