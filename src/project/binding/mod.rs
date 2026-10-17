//! Declarations, scopes, aliases, and the semantic index.
//!
//! `collect` records one file's owned facts while its AST is alive. `bind`
//! completes project-wide declaration tables, export and alias relationships,
//! and occurrence resolution, then freezes the index served to queries.

mod collect;
pub(super) mod flow;
mod ids;
mod index;
pub(super) mod inputs;
mod instances;
mod json;
mod link;
pub(super) use link::{LinkState, NativeLinkOutput, ProjectionObserver};
mod member;
pub(in crate::project) mod native_inputs;
mod ordering;
pub(super) use native_inputs::{NativeInputs, RawDeclaration};
pub(super) use resolution::names::{Names as NameSymbols, ScopeSymbols};
mod native_facts;
pub(super) use native_facts::{LookupFailure, NativeFacts, ScopeExportRead};
pub(super) use resolution::names::{ScopeExportLookup, scope_export_lookup};
mod native_symbol_table;
pub(super) use native_symbol_table::NativeSymbolTable;
pub(in crate::project) mod module_exports;
pub(super) mod raw;
mod reference_modules;
mod reference_tokens;
mod resolution;
mod retained;
mod semantic;
mod semantic_flow;
mod site_map;
mod store;
mod targets;
mod text;
pub(super) use instances::{Instance as ModuleInstanceState, module_instance_state};
pub(super) use json::{JsonDeclaration, JsonPropertyId};
pub(super) use member::{declaration_anchor, declaration_anchor_controlled};
pub(super) use ordering::{
    IdentityOrder, NativeByteInput, NativeDeclaration, NativeNodeIdentity, NativePropertyKey,
    NativePropertyKeyKind, NativeStringError, NativeStringPart, NativeStringPosition,
    NativeStringSegment, NativeStringValue, NativeSymbol, NativeSymbolIdentity, try_sort_by,
};
pub(super) use reference_modules::ReferenceModuleSite;
pub(super) use reference_tokens::{
    ReferenceToken, ReferenceTokenFacts, ReferenceTokenRole, ReferenceTokenSource,
};

use super::program::Program;
use super::sources::FileId;
use crate::analysis::InputError;
use crate::parser::json::JsonText;
use crate::source::SourceText;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, Span};
use std::sync::Arc;

/// A module one file requests, including the program's synthetic imports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModuleRequest {
    pub(super) specifier: String,
    pub(super) kind: RequestKind,
    /// An explicit `resolution-mode` import attribute.
    pub(super) mode: Option<ResolutionMode>,
    /// `GetEmitSyntaxForUsageLocation`, captured independently of an explicit
    /// resolution override; None is native ModuleKindNone, never unbound input
    /// after the loader publishes this request.
    pub(super) emit_mode: Option<ResolutionMode>,
    /// `getModeForUsageLocation`, including explicit type-only overrides and
    /// native None. Captured independently from the emit syntax above.
    pub(super) resolution_mode: Option<ResolutionMode>,
    /// Raw `ResolvedModule.ResolvedFileName`, before program admission.
    /// None means the resolver found no file, even for unchecked requests.
    pub(super) resolved_path: Option<String>,
    /// Where diagnostics report the request: the specifier's string literal
    /// token, or the start of the first JSX tag for the JSX runtime.
    pub(super) span: Span,
    /// Program-created specifiers are actual synthetic import nodes, not a
    /// source literal whose token happens to have a zero-width range.
    pub(super) implicit: Option<ImplicitImport>,
}

/// `forEachImport` visits JSX before helpers; program resolution deliberately
/// uses the reverse order. Retain the factory role instead of recognizing its
/// specifier text, which may also occur in an ordinary source import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ImplicitImport {
    JsxRuntime,
    Helpers,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum ResolutionMode {
    Import,
    Require,
}

/// File-level settings decided by the project configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct FileOptions {
    /// The configuration and file format force module semantics.
    pub(super) force_module: bool,
    /// `jsx: react-jsx` makes a file that uses JSX a module.
    pub(super) jsx_module: bool,
}

/// Owned semantic facts captured from one parsed file.
#[derive(Debug, Clone, Default)]
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
    facts: Arc<collect::Facts>,
    /// A JSON source file's module, which declares no names.
    json: Option<Arc<JsonFacts>>,
}

#[cfg(test)]
impl FileFacts {
    pub(super) fn collection_identity(&self) -> usize {
        self.json.as_ref().map_or_else(
            || Arc::as_ptr(&self.facts) as usize,
            |json| Arc::as_ptr(json) as usize,
        )
    }
}

use json::APPARENT_INTERFACES;
pub(super) use json::{JsonFacts, JsonShape, JsonValueId};

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
pub(super) use retained::Bindings;
pub(super) use semantic::{
    BoundClassKind, BoundClassOrInterface, BoundDeclaration, BoundDeclaredType, BoundJson,
    BoundMember, BoundMemberKind, BoundSymbol, BoundTypeLiteral, BoundValueType, EntityNameRecipe,
    NativeLookupMeaning, NativePropertyFlags, NativePropertyParent, NativeRawSymbolTable,
    NativeSelectionFailure, NativeSymbolDeclaration, NativeSymbolFlags, ReadOrigin,
    ReferenceSymbol, TypeBindingFailure, TypeSymbolFlags,
};
pub(super) use store::should_replace_value_declaration;

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
/// `indicators` are the file's own (`ModuleIndicators::of`); `module`, their
/// effective module decision, is collection's only configuration input.
pub(super) fn collect(
    ast: &Ast<'_>,
    source: &SourceText,
    indicators: ModuleIndicators,
    module: bool,
) -> Result<FileFacts, InputError> {
    let (facts, requests) = collect::Collector::new(ast, source, module).run()?;
    Ok(FileFacts {
        module,
        declaration: syntax::flag(ast.root(), "isDeclarationFile"),
        jsx: indicators.first_jsx,
        requests,
        directives: comment_directives(ast.source(), ast.literal_token_spans()),
        facts: Arc::new(facts),
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
        json: Some(Arc::new(JsonFacts::new(text.root.as_ref()))),
        ..FileFacts::default()
    }
}

/// Complete every existing link/check before retaining its lexical decisions.
/// The caller then transfers the owned file facts with `Linked::retain_files`.
pub(super) fn bind(
    program: &Program,
    parser: &crate::analysis::Parser,
) -> Result<retained::Linked, crate::analysis::AnalysisError> {
    link::link(program, parser)
}

/// What in one file's syntax decides, with its `FileOptions`, whether the
/// file is a module: TypeScript's external-module indicators
/// (`crate::ast::external_module`) and its first JSX element, to which the
/// project adds configuration and format rules. A file collected under some
/// options thereby shows whether others would collect it differently.
#[derive(Debug, Clone, Copy)]
pub(super) struct ModuleIndicators {
    external: bool,
    first_jsx: Option<usize>,
}

impl ModuleIndicators {
    pub(super) fn of(ast: &Ast<'_>) -> Self {
        let root = ast.root();
        Self {
            external: crate::ast::external_module(
                ast.nodes(),
                syntax::ids(root, "statements"),
                root.flags,
            ),
            first_jsx: first_jsx(ast),
        }
    }

    /// Whether `collect` with `options` treats the file as a module.
    pub(super) fn module(self, options: FileOptions) -> bool {
        options.force_module || self.external || options.jsx_module && self.first_jsx.is_some()
    }
}

fn first_jsx(ast: &Ast<'_>) -> Option<usize> {
    // The parser makes JSX nodes only for the JSX language variant, as
    // TypeScript's `parsePrimaryExpression` reaches
    // `parseJsxElementOrSelfClosingElementOrFragment` only then: any other
    // file has none, without a walk of its whole tree.
    let variant = syntax::number(ast.root(), "languageVariant");
    debug_assert!(variant.is_some(), "the parser records the language variant");
    if variant != Some(crate::compiler::variant::JSX) {
        return None;
    }
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
