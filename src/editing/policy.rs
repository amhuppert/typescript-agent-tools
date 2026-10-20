//! Operation policies: which selected sites support each operation, the
//! syntax slot an edit opens there, and what may fill it. Selection has
//! already chosen the site exactly as `show` does; a policy never searches
//! for a different site that would support the operation.

use super::Operation;
use super::compare::Slot;
use super::seam::{self, Seam};
use crate::navigation::{Kind, SiteSyntax as Site};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, Span, syntax};

/// The slot one edit opens in the original tree.
#[derive(Debug, Clone, Copy)]
pub(super) enum Opening {
    /// A callable's `body`. Only an arrow function's body may be an
    /// expression.
    Body { callable: NodeId, arrow: bool },
    /// New statements in `owner`'s statement list before its item at
    /// `index`.
    Statements { owner: NodeId, index: usize },
}

/// A site that does not support an operation, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Unsupported(pub(super) String);

/// The kinds of syntax with a body a callable's caller runs.
const CALLABLES: [SyntaxKind; 7] = [
    SyntaxKind::FunctionDeclaration,
    SyntaxKind::FunctionExpression,
    SyntaxKind::ArrowFunction,
    SyntaxKind::MethodDeclaration,
    SyntaxKind::Constructor,
    SyntaxKind::GetAccessor,
    SyntaxKind::SetAccessor,
];

/// Plan `operation` with `fragment` at `site` of `ast`, for request `edit`.
pub(super) fn plan(
    operation: Operation,
    ast: &Ast<'_>,
    site: Site,
    fragment: &str,
    edit: usize,
) -> Result<(Opening, Seam), Unsupported> {
    match operation {
        Operation::ReplaceBody => {
            let callable = callable(ast, site)?;
            let node = ast.node(callable);
            let Some(body) = syntax::id(node, "body") else {
                return Err(Unsupported(format!(
                    "this {} has no body to replace (an overload, abstract member, or signature)",
                    describe(site)
                )));
            };
            if node.flags & crate::compiler::node::AMBIENT != 0 {
                return Err(Unsupported(format!(
                    "this {} is ambient; ambient declarations have no bodies to replace",
                    describe(site)
                )));
            }
            let opening = Opening::Body {
                callable,
                arrow: node.syntax_kind == SyntaxKind::ArrowFunction,
            };
            Ok((opening, seam::body(ast, body, fragment, edit)))
        }
        Operation::InsertAfter => {
            let statement = site.designation;
            let Some((owner, index)) = statement_list(ast, statement) else {
                return Err(Unsupported(match ast.node(statement).syntax_kind {
                    SyntaxKind::VariableDeclarationList => {
                        "this binding is declared in a loop header, which has no statement list to insert into".into()
                    }
                    SyntaxKind::VariableDeclaration => {
                        "this binding is a catch clause variable, which has no statement list to insert into".into()
                    }
                    _ => format!(
                        "this {} is not a statement in a statement list (statements can be inserted only after a statement of a file, block, module block, or case clause)",
                        describe(site)
                    ),
                }));
            };
            Ok((
                Opening::Statements {
                    owner,
                    index: index + 1,
                },
                seam::after(ast, statement, fragment, edit),
            ))
        }
    }
}

impl Opening {
    /// The slot this opening gives the comparison, whose fill lies within
    /// `region` of the composed text.
    pub(super) fn slot(self, region: Span) -> Slot {
        match self {
            Self::Body { callable, .. } => Slot::Field {
                node: callable,
                field: "body",
                region,
            },
            Self::Statements { owner, index } => Slot::Gap {
                node: owner,
                field: "statements",
                index,
                region,
            },
        }
    }

    /// Whether `fill`, the composed syntax filling this opening's slot, is
    /// what the operation allows there: a callable's body is a block,
    /// except an arrow function's, which may be an expression; inserted
    /// statements are at least one statement.
    pub(super) fn accepts(self, composed: &Ast<'_>, fill: &[NodeId]) -> Result<(), String> {
        match self {
            Self::Body { arrow, .. } => match fill {
                [body] if arrow || composed.node(*body).syntax_kind == SyntaxKind::Block => Ok(()),
                [body] => Err(format!(
                    "the new body is {}, but only an arrow function's body may be an expression; supply a block {{ ... }}",
                    composed.node(*body).kind
                )),
                _ => Err("the fragment does not form one body".into()),
            },
            Self::Statements { .. } if fill.is_empty() => {
                Err("the fragment contains no statement".into())
            }
            Self::Statements { .. } => Ok(()),
        }
    }
}

/// The callable whose body a site's body replacement replaces: the site's
/// own callable, or a variable's, property's, or binding's initializer that
/// is directly a function or arrow function, inside any parentheses
/// (`syntax::skip_parentheses`). Nothing else is unwrapped, so a call,
/// condition, or assertion around a function does not make the binding
/// eligible; such a function is selected by its own anchor.
fn callable(ast: &Ast<'_>, site: Site) -> Result<NodeId, Unsupported> {
    let node = ast.node(site.node);
    if CALLABLES.contains(&node.syntax_kind) {
        return Ok(site.node);
    }
    if !matches!(site.kind, Kind::Variable | Kind::Property) {
        // A declaration whose content is a member list says so and names
        // the remedy, a declaration with a body. It never names inserting
        // after, which inserts after the whole declaration (design, step 5,
        // "Refusal wording").
        let remedy = match site.kind {
            Kind::Class | Kind::Interface | Kind::Enum => {
                "; member-list editing is not supported; select a declaration with a body"
            }
            Kind::Function
            | Kind::Variable
            | Kind::Type
            | Kind::Namespace
            | Kind::Module
            | Kind::Method
            | Kind::Accessor
            | Kind::Constructor
            | Kind::Signature
            | Kind::Property
            | Kind::EnumMember
            | Kind::Import
            | Kind::Export => "",
        };
        return Err(Unsupported(format!(
            "this {} has no body to replace{remedy}",
            describe(site)
        )));
    }
    let Some(initializer) = syntax::id(node, "initializer") else {
        return Err(Unsupported(format!(
            "this {} has no initializer whose body could be replaced",
            describe(site)
        )));
    };
    let inner = syntax::skip_parentheses(ast, initializer);
    match ast.node(inner).syntax_kind {
        SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction => Ok(inner),
        _ => Err(Unsupported(format!(
            "this {}'s initializer is {}, not directly a function or arrow function; select a function inside it by its own position",
            describe(site),
            ast.node(inner).kind
        ))),
    }
}

/// The statement list holding `statement` directly, as its owner and the
/// statement's index: the statements of a source file, block, module
/// block, or case or default clause. A statement elsewhere, such as an
/// unbraced `if` body or a labeled statement's, has none.
fn statement_list(ast: &Ast<'_>, statement: NodeId) -> Option<(NodeId, usize)> {
    let target = ast.node(statement).span;
    let mut current = ast.root_id();
    loop {
        if let Some(index) = syntax::ids(ast.node(current), "statements")
            .iter()
            .position(|&item| item == statement)
        {
            return Some((current, index));
        }
        current = ast.children(current).find(|&child| {
            let span = ast.node(child).span;
            child != statement && span.start <= target.start && target.end <= span.end
        })?;
    }
}

/// A site's kind of syntax, for messages: its navigation kind.
fn describe(site: Site) -> String {
    site.kind.as_str().replace('_', " ")
}
