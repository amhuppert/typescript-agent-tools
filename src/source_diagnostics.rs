//! What each dialect makes of the diagnostics a file's syntax alone gives,
//! in syntax its parser accepts (`Policy`):
//!
//! - TS1540, a namespace declared with the `module` keyword, whose
//!   predicate, location, and message this module owns (`candidates`):
//!   TypeScript 7 reports it as an error, while TypeScript 5.6 reports it
//!   only as a suggestion (`Policy::is_error`).
//! - A JavaScript file's TypeScript-only syntax, which
//!   `crate::javascript_syntax` finds by each compiler's rule: the policy
//!   selects TypeScript 7's parser (`checkJSSyntax`) or 5.6.3's walk of the
//!   finished tree (`getJSSyntacticDiagnosticsForFile`)
//!   (`Policy::javascript_syntax`).
//! - TS1206 for a JavaScript parameter decorator without
//!   `experimentalDecorators`: TypeScript 7's checker reports it, at the
//!   decorator's token, where it checks the file, and otherwise its program
//!   does, syntactically, from the decorator's full start; 5.6.3's program
//!   always reports it syntactically, at the token
//!   (`Policy::parameter_decorator`).
//!
//! The file-local syntax check, which reads no configuration and so has no
//! dialect, reports TypeScript 7's errors (`source_errors`, `tsparse
//! --check`). Project loading captures every dialect's candidates and
//! classifies them with its dialect's policy each generation. It reports a
//! project file's syntactic errors whatever checks the file, and a checker
//! error where its file's reporting decision does (`Diagnostics::reports`
//! in `project::program`), so that `skipLibCheck`, `// @ts-nocheck`, and
//! comment directives apply to it as to TypeScript's other checker
//! diagnostics.

use crate::compiler::node as node_flags;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, Diagnostic, Node, NodeId};

/// TS1540's message, as TypeScript 7.0.2 words it.
const MODULE_KEYWORD: &str = "A 'namespace' declaration should not be declared using the 'module' \
                              keyword. Please use the 'namespace' keyword instead.";

/// What one dialect makes of the diagnostics a file's syntax alone gives
/// (`SourceCandidate`): whether each is an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Policy {
    module_keyword: Category,
    parameter_decorators: DecoratorRule,
    javascript_syntax: SyntaxRule,
}

/// Whose rule decides a JavaScript file's errors for TypeScript-only syntax
/// (`crate::javascript_syntax`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyntaxRule {
    /// TypeScript 7's parser (`checkJSSyntax`).
    Parser,
    /// 5.6.3's walk of the finished tree (`getJSSyntacticDiagnosticsForFile`).
    TreeWalk,
}

/// Who reports TS1206 for a JavaScript parameter decorator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecoratorRule {
    /// The checker, at the decorator's token, where it checks the file;
    /// otherwise the program, syntactically, from the decorator's full start
    /// (TypeScript 7's `getAdditionalJSSyntacticDiagnostics`).
    CheckerWhereChecked,
    /// The program, syntactically, at the decorator's token, however the
    /// file is checked (5.6.3's `getJSSyntacticDiagnosticsForFile`).
    Syntactic,
}

/// How a JavaScript parameter decorator's TS1206 is reported
/// (`Policy::parameter_decorator`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecoratorReport {
    /// As a syntactic diagnostic, of the first decorator: from its full
    /// start, or at its token.
    Syntactic { full_start: bool },
    /// As the checker's diagnostic, of each decorator at its token, as
    /// comment directives and the file's checking decide.
    Checker,
}

/// A diagnostic's category, as TypeScript's diagnostic table declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    Error,
    Suggestion,
}

impl Policy {
    /// TypeScript 7.0.2 declares TS1540 an error.
    pub(crate) const TYPESCRIPT_7: Self = Self {
        module_keyword: Category::Error,
        parameter_decorators: DecoratorRule::CheckerWhereChecked,
        javascript_syntax: SyntaxRule::Parser,
    };

    /// TypeScript 5.6.3 declares TS1540 a suggestion, which
    /// `checkModuleDeclaration` adds to `suggestionDiagnostics`.
    pub(crate) const TYPESCRIPT_5_6: Self = Self {
        module_keyword: Category::Suggestion,
        parameter_decorators: DecoratorRule::Syntactic,
        javascript_syntax: SyntaxRule::TreeWalk,
    };

    /// Whether `candidate` is an error under this policy.
    pub(crate) fn is_error(self, candidate: &SourceCandidate) -> bool {
        match candidate.kind {
            Kind::ModuleKeyword => self.module_keyword == Category::Error,
        }
    }

    /// The first error of a JavaScript file's TypeScript-only syntax under
    /// this policy, of each rule's (`candidates`).
    pub(crate) fn javascript_syntax(
        self,
        candidates: &crate::javascript_syntax::Candidates,
    ) -> Option<&Diagnostic> {
        match self.javascript_syntax {
            SyntaxRule::Parser => candidates.typescript_7.as_ref(),
            SyntaxRule::TreeWalk => candidates.typescript_5_6.as_ref(),
        }
    }

    /// How TS1206 for a parameter decorator in a JavaScript file is reported
    /// without `experimentalDecorators`, where the checker does (`checked`)
    /// or does not check the file.
    pub(crate) fn parameter_decorator(self, checked: bool) -> DecoratorReport {
        match self.parameter_decorators {
            DecoratorRule::CheckerWhereChecked if checked => DecoratorReport::Checker,
            DecoratorRule::CheckerWhereChecked => DecoratorReport::Syntactic { full_start: true },
            DecoratorRule::Syntactic => DecoratorReport::Syntactic { full_start: false },
        }
    }
}

/// One diagnostic of a file's syntax alone, which a dialect's `Policy`
/// classifies. Captured facts keep every candidate, whatever the dialect, so
/// that a generation reading the same text with another dialect classifies
/// it afresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceCandidate {
    kind: Kind,
    pub(crate) diagnostic: Diagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// TS1540.
    ModuleKeyword,
}

/// The errors TypeScript 7 reports from `ast`'s syntax alone, as the
/// file-local syntax check, which has no configuration, reports them: a
/// JavaScript file's TypeScript-only syntax first, as TypeScript reports
/// syntactic diagnostics before any other (`crate::javascript_syntax`),
/// then the checker's, in source order.
pub fn source_errors(ast: &Ast<'_>) -> Vec<Diagnostic> {
    let mut errors = crate::javascript_syntax::errors(ast);
    if !crate::javascript_syntax::is_javascript(ast) {
        errors.extend(
            candidates(ast)
                .into_iter()
                .filter(|candidate| Policy::TYPESCRIPT_7.is_error(candidate))
                .map(|candidate| candidate.diagnostic),
        );
    }
    errors
}

/// The diagnostics of `ast`'s syntax alone that some dialect reports, in
/// source order.
///
/// TypeScript 7.0.2's `checkModuleDeclaration` reports TS1540 at the name
/// of each identifier-named module declaration whose keyword is `module`
/// (5.6.3's suggestion is at the keyword), which each declaration of a
/// dotted name repeats
/// (`parseModuleOrNamespaceDeclaration`). It returns first where
/// `checkGrammarModuleElementContext` finds the declaration out of place,
/// in a parent other than a source file, module block, or module
/// declaration, and reports TS1235 instead, which this tool leaves to a
/// checker as it does for `namespace`.
pub(crate) fn candidates(ast: &Ast<'_>) -> Vec<SourceCandidate> {
    // Most files declare no such namespace, which a scan of the arena,
    // without a walk, shows; a node superseded by reparsing only costs the
    // walk that finds nothing.
    if !ast
        .nodes()
        .iter()
        .any(|node| module_keyword_name(ast, node).is_some())
    {
        return Vec::new();
    }
    let mut errors = Vec::new();
    // Each node beside its parent's kind, in pre-order.
    let root = ast.root();
    let mut stack: Vec<(NodeId, SyntaxKind)> = ast
        .children(ast.root_id())
        .map(|child| (child, root.syntax_kind))
        .collect();
    stack.reverse();
    while let Some((id, parent)) = stack.pop() {
        let node = ast.node(id);
        if matches!(
            parent,
            SyntaxKind::SourceFile | SyntaxKind::ModuleBlock | SyntaxKind::ModuleDeclaration
        ) && let Some(name) = module_keyword_name(ast, node)
        {
            errors.push(SourceCandidate {
                kind: Kind::ModuleKeyword,
                diagnostic: Diagnostic {
                    message: MODULE_KEYWORD.to_string(),
                    span: ast.token_span(name),
                },
            });
        }
        let first = stack.len();
        stack.extend(ast.children(id).map(|child| (child, node.syntax_kind)));
        stack[first..].reverse();
    }
    errors
}

/// The name of `node` if it is an identifier-named module declaration whose
/// keyword is `module`: in this tree, one with neither the `Namespace` nor
/// the `GlobalAugmentation` flag.
fn module_keyword_name(ast: &Ast<'_>, node: &Node<'_>) -> Option<NodeId> {
    if node.syntax_kind != SyntaxKind::ModuleDeclaration
        || node.flags & (node_flags::NAMESPACE | node_flags::GLOBAL_AUGMENTATION) != 0
    {
        return None;
    }
    crate::syntax::id(node, "name")
        .filter(|&name| ast.node(name).syntax_kind == SyntaxKind::Identifier)
}
