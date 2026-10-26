//! The test view: which statements of a file are test calls, and each one's
//! position, call spelling, name, and containment. The one owner of that
//! decision, for `navigation::test_outline_file`.
//!
//! It ports no TypeScript function: TypeScript asks no such question, so
//! this is a deliberate addition beside the outline, which stays a port of
//! TypeScript's document symbols. It recognizes the Vitest API by syntax
//! alone, with no import check, so Vitest globals need no project. The
//! modifier list is Vitest 3.2.4's, from its `@vitest/runner` types:
//! `ChainableTestAPI` has `concurrent`, `sequential`, `only`, `skip`,
//! `todo`, and `fails`, plus `each` and `for`; `ChainableSuiteAPI` has the
//! same with `shuffle` in place of `fails`; and `skipIf` and `runIf` sit on
//! `TestAPI` (through `ExtendedAPI`) and `SuiteAPI`. A modifier a later
//! Vitest adds is a known gap of this list, not a silent omission. The list
//! is fixed because fixture objects are often named `test`: a chain of any
//! names would report `test.runtimeHost.advance(…)` as a test.
//!
//! A test call is an expression statement whose expression is, with
//! nothing wrapping it (no parentheses, `await`, `void`, or other
//! operator), a call whose callee is a hook (`beforeAll`, `beforeEach`,
//! `afterAll`, or `afterEach`) or a test chain: `describe`, `it`, or `test`,
//! followed by plain modifiers (`X.skip`) and called modifiers
//! (`X.each(table)`), each in its own form. Names are decoded identifier
//! names, compared exactly; node kinds alone decide the rest, so `?.` and
//! type arguments change nothing, and a tagged template, element access,
//! non-null assertion, or receiver such as `this.it` forms no chain.
//!
//! Every listed position is its statement's first token, the root or hook
//! identifier, which no declaration names and no unnamed site anchors, so
//! `--at` there selects the statement as a statement site. A computed
//! property name is the one `--at` target longer than a token, which takes
//! every position inside it for its member, so nothing inside one is
//! listed.
//!
//! The walk follows the final tree with an explicit stack and a chain is
//! walked in a loop, as a member chain may be as long as the file; nothing
//! recurses natively.

use super::TestEntry;
use super::names;
use crate::source::SourceText;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId};

/// The roots of a test chain.
const ROOTS: &[&str] = &["describe", "it", "test"];

/// The modifiers of a test chain that are not called: `it.skip(…)`.
const PLAIN: &[&str] = &[
    "only",
    "skip",
    "todo",
    "concurrent",
    "sequential",
    "shuffle",
    "fails",
];

/// The modifiers of a test chain that are called before the test call:
/// `it.each(table)(…)`.
const CALLED: &[&str] = &["each", "for", "skipIf", "runIf"];

/// The hooks, which are named identifiers alone.
const HOOKS: &[&str] = &["beforeAll", "beforeEach", "afterAll", "afterEach"];

/// The test calls of `ast`, which was parsed from `source`, ordered by
/// position, each at the depth its ancestry gives: one more than the nearest
/// test call whose statement contains its statement, or 1 without one.
/// Every other syntax is transparent. The walk visits fields in storage
/// order, which reaches a decorator's test after its class's members, so
/// the entries are ordered by position afterwards. A parent's statement
/// contains its children's, so the order is a pre-order.
pub(super) fn entries(ast: &Ast<'_>, source: &SourceText) -> Vec<TestEntry> {
    // Each entry with the offset of its position.
    let mut found: Vec<(usize, TestEntry)> = Vec::new();
    // Each node to visit, with the entry of the nearest test call whose
    // statement contains it.
    let mut stack: Vec<(NodeId, Option<usize>)> = vec![(ast.root_id(), None)];
    while let Some((id, enclosing)) = stack.pop() {
        if ast.node(id).syntax_kind == SyntaxKind::ComputedPropertyName {
            continue;
        }
        let mut inner = enclosing;
        if let Some((call, name)) = test_call(ast, id) {
            let start = ast.first_token_span(id).start;
            let depth = enclosing.map_or(1, |parent| found[parent].1.depth + 1);
            inner = Some(found.len());
            found.push((
                start,
                TestEntry {
                    anchor: source.position(start),
                    call,
                    name,
                    depth,
                },
            ));
        }
        stack.extend(ast.children(id).map(|child| (child, inner)));
    }
    found.sort_by_key(|&(start, _)| start);
    found.into_iter().map(|(_, entry)| entry).collect()
}

/// The call spelling and name of `statement`, when it is a test call. A
/// test chain's name is its call's first argument as written; a hook and a
/// call without arguments have none.
fn test_call(ast: &Ast<'_>, statement: NodeId) -> Option<(String, Option<String>)> {
    let node = ast.node(statement);
    if node.syntax_kind != SyntaxKind::ExpressionStatement {
        return None;
    }
    let call = ast.node(syntax::id(node, "expression")?);
    if call.syntax_kind != SyntaxKind::CallExpression {
        return None;
    }
    let callee = syntax::id(call, "expression")?;
    if let Some(hook) = identifier(ast, callee).filter(|name| HOOKS.contains(name)) {
        return Some((hook.to_owned(), None));
    }
    let spelling = chain(ast, callee)?;
    let name = syntax::ids(call, "arguments")
        .first()
        .map(|&argument| names::written(ast, argument));
    Some((spelling, name))
}

/// The spelling of the test chain `callee`, when it is one: its root, then
/// each modifier in source order, each after a `.`, and a called modifier
/// with `()`. Its tables and conditions are not part of it.
fn chain(ast: &Ast<'_>, callee: NodeId) -> Option<String> {
    // Each modifier, outermost first, and whether it is called.
    let mut modifiers: Vec<(&str, bool)> = Vec::new();
    let mut current = callee;
    let root = loop {
        let node = ast.node(current);
        let (access, called) = match node.syntax_kind {
            SyntaxKind::Identifier => {
                break identifier(ast, current).filter(|name| ROOTS.contains(name))?;
            }
            SyntaxKind::PropertyAccessExpression => (node, false),
            SyntaxKind::CallExpression => {
                let access = ast.node(syntax::id(node, "expression")?);
                if access.syntax_kind != SyntaxKind::PropertyAccessExpression {
                    return None;
                }
                (access, true)
            }
            _ => return None,
        };
        let names = if called { CALLED } else { PLAIN };
        let modifier =
            identifier(ast, syntax::id(access, "name")?).filter(|name| names.contains(name))?;
        modifiers.push((modifier, called));
        current = syntax::id(access, "expression")?;
    };
    let mut spelling = root.to_owned();
    for (modifier, called) in modifiers.into_iter().rev() {
        spelling.push('.');
        spelling.push_str(modifier);
        if called {
            spelling.push_str("()");
        }
    }
    Some(spelling)
}

/// The decoded name of `id` when it is an identifier.
fn identifier<'a>(ast: &'a Ast<'_>, id: NodeId) -> Option<&'a str> {
    match ast.node(id).syntax_kind {
        SyntaxKind::Identifier => ast.identifier_name(id),
        _ => None,
    }
}
