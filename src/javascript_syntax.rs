//! The errors TypeScript 7's parser gives TypeScript-only syntax in a
//! JavaScript file: `checkJSSyntax` and `checkJSDecoratorSyntax`
//! (internal/parser/parser.go), which it records apart from parse errors
//! (`jsDiagnostics`) while the tree stays whole. They are syntactic
//! diagnostics: neither `checkJs`, `// @ts-nocheck`, `noCheck`, comment
//! directives, nor `skipLibCheck` removes them. This module owns which
//! nodes get one, where, and with which message; the file-local syntax check
//! (`crate::source_errors`) and project loading report them.
//!
//! TypeScript checks each node as it finishes it, so a node a speculative
//! parse discards has none; walking the final tree finds the same nodes.
//! It checks no node inside a type, except an expression there, and in a
//! type's signature neither a parameter nor an accessor (`ParseFlagsType`).
//! A JSDoc type the parser reparses into a declaration is not written
//! syntax (`Ast::source_children`), which TypeScript checks before the
//! reparse, so neither rule sees it.
//!
//! TypeScript 5.6.3 decides these errors otherwise
//! (`getJSSyntacticDiagnosticsForFile`, `errors_5_6`): it walks the finished
//! tree, skips what lies under a node it reports, reports where
//! `getErrorSpanForNode` places a node, and checks an ambient module too.
//! Capture keeps each rule's first error (`Candidates`), and the dialect's
//! source-diagnostic policy selects one.

use crate::compiler::node as node_flags;
use crate::syntax::{flag, id as field, ids, is_function_like_kind, is_type_node_kind};
use crate::syntax_kind::SyntaxKind::{self, *};
use crate::{Ast, Diagnostic, NodeId, Span, Value};

/// The first error each dialect's rule gives a JavaScript file's
/// TypeScript-only syntax, which capture keeps whatever the dialect.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Candidates {
    pub(crate) typescript_7: Option<Diagnostic>,
    pub(crate) typescript_5_6: Option<Diagnostic>,
}

impl Candidates {
    /// The first of `errors` (7.0.2's `checkJSSyntax`) and of `errors_5_6`
    /// (5.6.3's `getJSSyntacticDiagnosticsForFile`) for `ast`.
    pub(crate) fn of(ast: &Ast<'_>) -> Self {
        Self {
            typescript_7: errors(ast).into_iter().next(),
            typescript_5_6: errors_5_6(ast).into_iter().next(),
        }
    }
}

/// Whether `ast` is a JavaScript file's tree: its root carries the
/// `JavaScriptFile` context flag (`IsInJSFile`).
pub(crate) fn is_javascript(ast: &Ast<'_>) -> bool {
    ast.root().flags & node_flags::JAVA_SCRIPT_FILE != 0
}

/// TypeScript 7's errors for the TypeScript-only syntax of a JavaScript
/// file's tree, in the order its diagnostics sort (`compareDiagnostics`:
/// by start, then by length); none for any other file.
pub(crate) fn errors(ast: &Ast<'_>) -> Vec<Diagnostic> {
    if !is_javascript(ast) {
        return Vec::new();
    }
    let mut checker = Checker {
        ast,
        errors: Vec::new(),
    };
    let root = ast.root_id();
    let mut stack: Vec<(NodeId, SyntaxKind)> = ast
        .source_children(root)
        .map(|child| (child, SourceFile))
        .collect();
    stack.reverse();
    while let Some((id, parent)) = stack.pop() {
        let kind = ast.node(id).syntax_kind;
        if checked(kind, parent) {
            checker.check(id);
        }
        let first = stack.len();
        stack.extend(ast.source_children(id).map(|child| (child, kind)));
        stack[first..].reverse();
    }
    let mut errors = checker.errors;
    errors.sort_by_key(|error| (error.span.start, error.span.end - error.span.start));
    errors
}

/// Whether TypeScript's parser checks a node of `kind` under a `parent` of
/// that kind: only nodes it finishes with `checkJSSyntax`. A type's
/// signatures, its members, and their parameters it never checks, nor the
/// parameters of an index signature; an index signature only as a class
/// member; an `ExpressionWithTypeArguments` only in a class's `extends`
/// clause (`checked_heritage`).
fn checked(kind: SyntaxKind, parent: SyntaxKind) -> bool {
    match kind {
        Parameter => {
            !is_type_node_kind(parent)
                && !matches!(parent, IndexSignature)
                && !matches!(
                    parent,
                    CallSignature | ConstructSignature | MethodSignature | JSDocSignature
                )
        }
        GetAccessor | SetAccessor => !matches!(parent, TypeLiteral | InterfaceDeclaration),
        IndexSignature => matches!(parent, ClassDeclaration | ClassExpression),
        // `parseModuleOrNamespaceDeclaration`; an ambient module's
        // (`parseAmbientExternalModuleDeclaration`) is not checked.
        ModuleDeclaration => true,
        VariableStatement
        | VariableDeclaration
        | FunctionDeclaration
        | Constructor
        | MethodDeclaration
        | PropertyDeclaration
        | InterfaceDeclaration
        | TypeAliasDeclaration
        | EnumDeclaration
        | ImportDeclaration
        | ImportEqualsDeclaration
        | ImportSpecifier
        | ExportAssignment
        | ExportDeclaration
        | ExportSpecifier
        | ArrowFunction
        | FunctionExpression
        | ClassDeclaration
        | ClassExpression
        | HeritageClause
        | NonNullExpression
        | AsExpression
        | SatisfiesExpression
        | CallExpression
        | NewExpression
        | TaggedTemplateExpression => true,
        _ => false,
    }
}

struct Checker<'a, 'src> {
    ast: &'a Ast<'src>,
    errors: Vec<Diagnostic>,
}

impl Checker<'_, '_> {
    /// An error over `span`, which starts after any leading trivia.
    fn report(&mut self, span: Span, message: String) {
        self.errors.push(Diagnostic { message, span });
    }

    fn at_node(&mut self, id: NodeId, message: impl Into<String>) {
        let span = self.ast.token_span(id);
        self.report(span, message.into());
    }

    /// An error over a node list, from its first element to its last.
    fn at_list(&mut self, list: &[NodeId], message: impl Into<String>) {
        let (Some(&first), Some(&last)) = (list.first(), list.last()) else {
            return;
        };
        let span = Span {
            start: self.ast.token_span(first).start,
            end: self.ast.node(last).span.end,
        };
        self.report(span, message.into());
    }

    /// `checkJSSyntax` for one node.
    fn check(&mut self, id: NodeId) {
        let ast = self.ast;
        let node = ast.node(id);
        let only = |construct: &str| {
            format!("'{construct}' declarations can only be used in TypeScript files.")
        };
        match node.syntax_kind {
            Parameter | PropertyDeclaration | MethodDeclaration | MethodSignature | Constructor
            | GetAccessor | SetAccessor | FunctionExpression | FunctionDeclaration
            | ArrowFunction | VariableDeclaration | IndexSignature => {
                if matches!(
                    node.syntax_kind,
                    Parameter | PropertyDeclaration | MethodDeclaration
                ) && let Some(question) = field(node, "questionToken")
                    && ast.node(question).syntax_kind == QuestionToken
                {
                    self.at_node(question, modifier_message("?"));
                }
                if is_function_like_kind(node.syntax_kind) && field(node, "body").is_none() {
                    self.at_node(
                        id,
                        "Signature declarations can only be used in TypeScript files.",
                    );
                } else if let Some(annotation) =
                    field(node, "type").filter(|&annotation| !ast.is_reparsed(annotation))
                {
                    self.at_node(
                        annotation,
                        "Type annotations can only be used in TypeScript files.",
                    );
                }
            }
            ImportDeclaration => {
                if field(node, "importClause")
                    .is_some_and(|clause| flag(ast.node(clause), "isTypeOnly"))
                {
                    self.at_node(id, only("import type"));
                }
            }
            ExportDeclaration if flag(node, "isTypeOnly") => self.at_node(id, only("export type")),
            ImportSpecifier if flag(node, "isTypeOnly") => self.at_node(id, only("import...type")),
            ExportSpecifier if flag(node, "isTypeOnly") => self.at_node(id, only("export...type")),
            ImportEqualsDeclaration => {
                self.at_node(id, "'import ... =' can only be used in TypeScript files.");
            }
            ExportAssignment if flag(node, "isExportEquals") => {
                self.at_node(id, "'export =' can only be used in TypeScript files.");
            }
            HeritageClause => {
                if crate::syntax::number(node, "token") == Some(ImplementsKeyword.number()) {
                    self.at_node(
                        id,
                        "'implements' clauses can only be used in TypeScript files.",
                    );
                }
            }
            ModuleDeclaration if is_ambient_module(ast, id) => {}
            InterfaceDeclaration | EnumDeclaration | ModuleDeclaration => {
                let construct = match node.syntax_kind {
                    InterfaceDeclaration => "interface",
                    EnumDeclaration => "enum",
                    _ if node.flags & node_flags::NAMESPACE != 0 => "namespace",
                    _ => "module",
                };
                if let Some(name) = field(node, "name") {
                    self.at_node(name, only(construct));
                }
            }
            TypeAliasDeclaration => {
                if let Some(name) = field(node, "name") {
                    self.at_node(name, "Type aliases can only be used in TypeScript files.");
                }
            }
            NonNullExpression => {
                self.at_node(
                    id,
                    "Non-null assertions can only be used in TypeScript files.",
                );
            }
            AsExpression | SatisfiesExpression => {
                let message = if node.syntax_kind == AsExpression {
                    "Type assertion expressions can only be used in TypeScript files."
                } else {
                    "Type satisfaction expressions can only be used in TypeScript files."
                };
                if let Some(annotation) = field(node, "type") {
                    self.at_node(annotation, message);
                }
            }
            ClassDeclaration | ClassExpression => self.check_heritage(id),
            _ => {}
        }
        self.check_decorators(id);
        match node.syntax_kind {
            ClassDeclaration | ClassExpression | MethodDeclaration | Constructor | GetAccessor
            | SetAccessor | FunctionExpression | FunctionDeclaration | ArrowFunction => {
                self.at_list(
                    ids(node, "typeParameters"),
                    "Type parameter declarations can only be used in TypeScript files.",
                );
                self.check_modifiers(id);
            }
            VariableStatement | PropertyDeclaration => self.check_modifiers(id),
            Parameter => {
                let modifiers = ids(node, "modifiers");
                if modifiers
                    .iter()
                    .any(|&modifier| ast.node(modifier).syntax_kind != Decorator)
                {
                    self.at_list(
                        modifiers,
                        "Parameter modifiers can only be used in TypeScript files.",
                    );
                }
            }
            CallExpression | NewExpression | TaggedTemplateExpression => {
                self.at_list(
                    ids(node, "typeArguments"),
                    "Type arguments can only be used in TypeScript files.",
                );
            }
            _ => {}
        }
    }

    /// The `ExpressionWithTypeArguments` of a class's `extends` clause, which
    /// TypeScript checks as it finishes the class.
    fn check_heritage(&mut self, class: NodeId) {
        let ast = self.ast;
        for &clause in ids(ast.node(class), "heritageClauses") {
            let clause = ast.node(clause);
            if crate::syntax::number(clause, "token") != Some(ExtendsKeyword.number()) {
                continue;
            }
            for &expression in ids(clause, "types") {
                self.at_list(
                    ids(ast.node(expression), "typeArguments"),
                    "Type arguments can only be used in TypeScript files.",
                );
            }
        }
    }

    /// Modifiers other than JavaScript's (`ModifierFlagsJavaScript`:
    /// `export`, `static`, `accessor`, `async`, `default`) and decorators.
    fn check_modifiers(&mut self, id: NodeId) {
        let ast = self.ast;
        for &modifier in ids(ast.node(id), "modifiers") {
            let kind = ast.node(modifier).syntax_kind;
            if !matches!(
                kind,
                Decorator
                    | ExportKeyword
                    | StaticKeyword
                    | AccessorKeyword
                    | AsyncKeyword
                    | DefaultKeyword
            ) {
                let span = ast.token_span(modifier);
                self.at_node(
                    modifier,
                    modifier_message(&ast.source()[span.start..span.end]),
                );
            }
        }
    }

    /// `checkJSDecoratorSyntax`.
    fn check_decorators(&mut self, id: NodeId) {
        let ast = self.ast;
        let node = ast.node(id);
        let modifiers = ids(node, "modifiers");
        let is_decorator = |modifier: &NodeId| ast.node(*modifier).syntax_kind == Decorator;
        if can_have_illegal_decorators(node.syntax_kind) {
            if let Some(&decorator) = modifiers.iter().find(|modifier| is_decorator(modifier)) {
                self.at_node(decorator, "Decorators are not valid here.");
            }
            return;
        }
        if !matches!(node.syntax_kind, ClassDeclaration) {
            return;
        }
        let Some(decorator) = modifiers.iter().position(is_decorator) else {
            return;
        };
        let position = |kind: SyntaxKind| {
            modifiers
                .iter()
                .position(|&modifier| ast.node(modifier).syntax_kind == kind)
        };
        let Some(export) = position(ExportKeyword) else {
            return;
        };
        let default = position(DefaultKeyword);
        if decorator > export && default.is_some_and(|default| decorator < default) {
            self.at_node(modifiers[decorator], "Decorators are not valid here.");
        } else if decorator < export
            && let Some(trailing) = modifiers[export..].iter().position(is_decorator)
        {
            self.at_node(
                modifiers[export + trailing],
                "Decorators may not appear after 'export' or 'export default' if they also appear before 'export'.",
            );
        }
    }
}

/// An ambient module, `declare global` or `declare module "name"`, which
/// TypeScript parses apart from a namespace
/// (`parseAmbientExternalModuleDeclaration`).
fn is_ambient_module(ast: &Ast<'_>, id: NodeId) -> bool {
    let node = ast.node(id);
    node.flags & node_flags::GLOBAL_AUGMENTATION != 0
        || field(node, "name").is_some_and(|name| ast.node(name).syntax_kind == StringLiteral)
}

/// TypeScript 5.6.3's errors for the TypeScript-only syntax of a JavaScript
/// file's tree (`getJSSyntacticDiagnosticsForFile`, less its parameter
/// decorators, which `experimentalDecorators` decides), sorted as `errors`.
/// It walks every node of the finished tree, and a node list before its
/// elements, and skips what lies under a node or list it reports.
pub(crate) fn errors_5_6(ast: &Ast<'_>) -> Vec<Diagnostic> {
    if !is_javascript(ast) {
        return Vec::new();
    }
    let mut walk = Walk56 {
        ast,
        errors: Vec::new(),
    };
    let mut stack = Vec::new();
    walk.children(ast.root_id(), &mut stack);
    while let Some((id, parent, key)) = stack.pop() {
        if !walk.node(id, parent, key) {
            walk.children(id, &mut stack);
        }
    }
    let mut errors = walk.errors;
    errors.sort_by_key(|error| (error.span.start, error.span.end - error.span.start));
    errors.dedup();
    errors
}

struct Walk56<'a, 'src> {
    ast: &'a Ast<'src>,
    errors: Vec<Diagnostic>,
}

impl Walk56<'_, '_> {
    /// Queue `parent`'s children in field order, walking each node list
    /// (`walkArray`) before its elements.
    fn children(&mut self, parent: NodeId, stack: &mut Vec<(NodeId, NodeId, &'static str)>) {
        let mut queued = Vec::new();
        for (key, value) in self.ast.logical_fields(parent) {
            match value {
                Value::Node(child) if self.ast.is_reparsed(*child) => {}
                Value::Node(child) => queued.push((*child, parent, key)),
                Value::Nodes(list) if !self.list(parent, key, list) => {
                    queued.extend(list.iter().map(|&child| (child, parent, key)));
                }
                _ => {}
            }
        }
        queued.reverse();
        stack.extend(queued);
    }

    fn report(&mut self, span: Span, message: impl Into<String>) {
        self.errors.push(Diagnostic {
            message: message.into(),
            span,
        });
    }

    /// `createDiagnosticForNode`, at `getErrorSpanForNode`: a declaration's
    /// name, a constructor up to its keyword, or else the node itself, each
    /// from its first token.
    fn at_node(&mut self, id: NodeId, message: impl Into<String>) {
        let ast = self.ast;
        let node = ast.node(id);
        let named = matches!(
            node.syntax_kind,
            VariableDeclaration
                | BindingElement
                | ClassDeclaration
                | ClassExpression
                | InterfaceDeclaration
                | ModuleDeclaration
                | EnumDeclaration
                | EnumMember
                | FunctionDeclaration
                | FunctionExpression
                | MethodDeclaration
                | GetAccessor
                | SetAccessor
                | TypeAliasDeclaration
                | PropertyDeclaration
                | PropertySignature
                | NamespaceImport
        );
        let target = if named { field(node, "name") } else { Some(id) };
        let span = match (node.syntax_kind, target) {
            (Constructor, _) => {
                let start = ast.token_span(id).start;
                let keyword = ast.source()[start..]
                    .find("constructor")
                    .map_or(start, |at| start + at + "constructor".len());
                Span {
                    start,
                    end: keyword,
                }
            }
            (_, Some(target)) => Span {
                start: ast.token_span(target).start,
                end: ast.node(target).span.end,
            },
            // A nameless declaration: the token at its start.
            (_, None) => ast.token_span(id),
        };
        self.report(span, message);
    }

    /// `createDiagnosticForNodeArray`: from where the list starts, before
    /// any comment, to its end.
    fn at_list(&mut self, list: &[NodeId], message: impl Into<String>) {
        let (Some(&first), Some(&last)) = (list.first(), list.last()) else {
            return;
        };
        let span = Span {
            start: self.ast.node(first).span.start,
            end: self.ast.node(last).span.end,
        };
        self.report(span, message);
    }

    /// `walk` for a node under `parent`'s field `key`: whether it reported,
    /// so that nothing under the node is walked.
    fn node(&mut self, id: NodeId, parent: NodeId, key: &str) -> bool {
        let ast = self.ast;
        let node = ast.node(id);
        let parent_kind = ast.node(parent).syntax_kind;
        if matches!(
            parent_kind,
            Parameter | PropertyDeclaration | MethodDeclaration
        ) && key == "questionToken"
        {
            self.at_node(id, modifier_message("?"));
            return true;
        }
        if matches!(
            parent_kind,
            Parameter
                | PropertyDeclaration
                | MethodDeclaration
                | MethodSignature
                | Constructor
                | GetAccessor
                | SetAccessor
                | FunctionExpression
                | FunctionDeclaration
                | ArrowFunction
                | VariableDeclaration
        ) && key == "type"
        {
            self.at_node(id, "Type annotations can only be used in TypeScript files.");
            return true;
        }
        let only = |construct: &str| {
            format!("'{construct}' declarations can only be used in TypeScript files.")
        };
        match node.syntax_kind {
            ImportClause if flag(node, "isTypeOnly") => self.at_node(parent, only("import type")),
            ExportDeclaration if flag(node, "isTypeOnly") => self.at_node(id, only("export type")),
            ImportSpecifier if flag(node, "isTypeOnly") => self.at_node(id, only("import...type")),
            ExportSpecifier if flag(node, "isTypeOnly") => self.at_node(id, only("export...type")),
            ImportEqualsDeclaration => {
                self.at_node(id, "'import ... =' can only be used in TypeScript files.");
            }
            ExportAssignment if flag(node, "isExportEquals") => {
                self.at_node(id, "'export =' can only be used in TypeScript files.");
            }
            HeritageClause
                if crate::syntax::number(node, "token") == Some(ImplementsKeyword.number()) =>
            {
                self.at_node(
                    id,
                    "'implements' clauses can only be used in TypeScript files.",
                );
            }
            InterfaceDeclaration => self.at_node(id, only("interface")),
            ModuleDeclaration => {
                let keyword = if node.flags & node_flags::NAMESPACE != 0 {
                    "namespace"
                } else {
                    "module"
                };
                self.at_node(id, only(keyword));
            }
            TypeAliasDeclaration => {
                self.at_node(id, "Type aliases can only be used in TypeScript files.");
            }
            Constructor | MethodDeclaration | FunctionDeclaration
                if field(node, "body").is_none() =>
            {
                self.at_node(
                    id,
                    "Signature declarations can only be used in TypeScript files.",
                );
            }
            EnumDeclaration => self.at_node(id, only("enum")),
            NonNullExpression => {
                self.at_node(
                    id,
                    "Non-null assertions can only be used in TypeScript files.",
                );
            }
            AsExpression | SatisfiesExpression => {
                let message = if node.syntax_kind == AsExpression {
                    "Type assertion expressions can only be used in TypeScript files."
                } else {
                    "Type satisfaction expressions can only be used in TypeScript files."
                };
                match field(node, "type") {
                    Some(annotation) => self.at_node(annotation, message),
                    None => return false,
                }
            }
            _ => return false,
        }
        true
    }

    /// `walkArray` for `parent`'s node list `key`: whether it reported, so
    /// that the list's elements are not walked.
    fn list(&mut self, parent: NodeId, key: &str, list: &[NodeId]) -> bool {
        let ast = self.ast;
        let node = ast.node(parent);
        let modifiers = ids(node, "modifiers");
        let is_decorator = |modifier: &NodeId| ast.node(*modifier).syntax_kind == Decorator;
        if can_have_illegal_decorators(node.syntax_kind) {
            if let Some(&decorator) = modifiers.iter().find(|modifier| is_decorator(modifier)) {
                self.at_node(decorator, "Decorators are not valid here.");
            }
        } else if node.syntax_kind == ClassDeclaration
            && let Some(decorator) = modifiers.iter().position(is_decorator)
        {
            let position = |kind: SyntaxKind| {
                modifiers
                    .iter()
                    .position(|&modifier| ast.node(modifier).syntax_kind == kind)
            };
            if let Some(export) = position(ExportKeyword) {
                let default = position(DefaultKeyword);
                if decorator > export && default.is_some_and(|default| decorator < default) {
                    self.at_node(modifiers[decorator], "Decorators are not valid here.");
                } else if decorator < export
                    && let Some(trailing) = modifiers[export..].iter().position(is_decorator)
                {
                    self.at_node(
                        modifiers[export + trailing],
                        "Decorators may not appear after 'export' or 'export default' if they also appear before 'export'.",
                    );
                }
            }
        }
        match node.syntax_kind {
            ClassDeclaration | ClassExpression | MethodDeclaration | Constructor | GetAccessor
            | SetAccessor | FunctionExpression | FunctionDeclaration | ArrowFunction
                if key == "typeParameters" =>
            {
                self.at_list(
                    list,
                    "Type parameter declarations can only be used in TypeScript files.",
                );
                true
            }
            ClassDeclaration | ClassExpression | MethodDeclaration | Constructor | GetAccessor
            | SetAccessor | FunctionExpression | FunctionDeclaration | ArrowFunction
            | VariableStatement
                if key == "modifiers" =>
            {
                let const_valid = node.syntax_kind == VariableStatement;
                for &modifier in list {
                    let kind = ast.node(modifier).syntax_kind;
                    if matches!(
                        kind,
                        PublicKeyword
                            | PrivateKeyword
                            | ProtectedKeyword
                            | ReadonlyKeyword
                            | DeclareKeyword
                            | AbstractKeyword
                            | OverrideKeyword
                            | InKeyword
                            | OutKeyword
                    ) || kind == ConstKeyword && !const_valid
                    {
                        self.at_modifier(modifier);
                    }
                }
                true
            }
            PropertyDeclaration if key == "modifiers" => {
                for &modifier in list {
                    let kind = ast.node(modifier).syntax_kind;
                    if is_modifier_keyword(kind) && !matches!(kind, StaticKeyword | AccessorKeyword)
                    {
                        self.at_modifier(modifier);
                    }
                }
                true
            }
            Parameter
                if key == "modifiers"
                    && list
                        .iter()
                        .any(|&modifier| is_modifier_keyword(ast.node(modifier).syntax_kind)) =>
            {
                self.at_list(
                    list,
                    "Parameter modifiers can only be used in TypeScript files.",
                );
                true
            }
            CallExpression
            | NewExpression
            | ExpressionWithTypeArguments
            | JsxSelfClosingElement
            | JsxOpeningElement
            | TaggedTemplateExpression
                if key == "typeArguments" =>
            {
                self.at_list(list, "Type arguments can only be used in TypeScript files.");
                true
            }
            _ => false,
        }
    }

    /// TS8009 for a modifier, spelled as written.
    fn at_modifier(&mut self, modifier: NodeId) {
        let span = self.ast.token_span(modifier);
        let message = modifier_message(&self.ast.source()[span.start..span.end]);
        self.at_node(modifier, message);
    }
}

/// TypeScript's `isModifier`: a keyword modifier, not a decorator.
fn is_modifier_keyword(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        AbstractKeyword
            | AccessorKeyword
            | AsyncKeyword
            | ConstKeyword
            | DeclareKeyword
            | DefaultKeyword
            | ExportKeyword
            | InKeyword
            | PublicKeyword
            | PrivateKeyword
            | ProtectedKeyword
            | ReadonlyKeyword
            | StaticKeyword
            | OutKeyword
            | OverrideKeyword
    )
}

/// TS8009 for a modifier, or for `?`, spelled as written.
fn modifier_message(modifier: &str) -> String {
    format!("The '{modifier}' modifier can only be used in TypeScript files.")
}

/// TypeScript's `CanHaveIllegalDecorators`.
fn can_have_illegal_decorators(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        PropertyAssignment
            | ShorthandPropertyAssignment
            | FunctionDeclaration
            | Constructor
            | IndexSignature
            | ClassStaticBlockDeclaration
            | MissingDeclaration
            | VariableStatement
            | InterfaceDeclaration
            | TypeAliasDeclaration
            | EnumDeclaration
            | ModuleDeclaration
            | ImportEqualsDeclaration
            | ImportDeclaration
            | NamespaceExportDeclaration
            | ExportDeclaration
            | ExportAssignment
    )
}

/// The first decorator of one parameter in a JavaScript file, which
/// TypeScript rejects without `experimentalDecorators` (TS1206): a
/// syntactic diagnostic or the checker's, by the file's checking and the
/// dialect (`crate::source_diagnostics::Policy::parameter_decorator`), each
/// reported where its own rule places it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParameterDecorator {
    /// Where the decorator's leading trivia begins (`decorator.Loc`).
    pub(crate) full_start: usize,
    /// Where its `@` is.
    pub(crate) token_start: usize,
}

/// TS1206's message.
pub(crate) const DECORATORS_NOT_VALID: &str = "Decorators are not valid here.";

/// Every parameter's first decorator in a JavaScript file's tree, in source
/// order (`getAdditionalJSSyntacticDiagnostics`, `checkDecorators`). Project
/// loading reports the first its policy reports, as only it knows the
/// options.
pub(crate) fn parameter_decorators(ast: &Ast<'_>) -> Vec<ParameterDecorator> {
    if !is_javascript(ast) {
        return Vec::new();
    }
    ast.walk()
        .filter(|&id| ast.node(id).syntax_kind == Parameter)
        .filter_map(|id| {
            ids(ast.node(id), "modifiers")
                .iter()
                .copied()
                .find(|&modifier| ast.node(modifier).syntax_kind == Decorator)
        })
        .map(|decorator| ParameterDecorator {
            full_start: ast.node(decorator).span.start,
            token_start: ast.token_span(decorator).start,
        })
        .collect()
}
