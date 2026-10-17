//! Function-body syntax questions used by native binder/checker operations.

use super::{has_modifier, id};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId};

#[derive(Clone, Copy)]
pub(crate) struct FunctionFlags(u8);

impl FunctionFlags {
    pub(crate) fn is_generator(self) -> bool {
        self.0 & 1 != 0
    }
    pub(crate) fn is_async(self) -> bool {
        self.0 & 2 != 0
    }
    pub(crate) fn is_invalid(self) -> bool {
        self.0 & 4 != 0
    }
}

/// TypeScript `GetFunctionFlags`: use the actual BodyData-bearing roles,
/// then the function-specific generator/async questions. A static block's
/// body field does not make it a native BodyData instance.
pub(crate) fn function_flags(ast: &Ast<'_>, node: NodeId) -> FunctionFlags {
    let node = ast.node(node);
    if !matches!(
        node.syntax_kind,
        SyntaxKind::FunctionDeclaration
            | SyntaxKind::FunctionExpression
            | SyntaxKind::ArrowFunction
            | SyntaxKind::MethodDeclaration
            | SyntaxKind::Constructor
            | SyntaxKind::GetAccessor
            | SyntaxKind::SetAccessor
            | SyntaxKind::ModuleDeclaration
    ) {
        return FunctionFlags(4);
    }
    let mut flags = 0;
    if matches!(
        node.syntax_kind,
        SyntaxKind::FunctionDeclaration
            | SyntaxKind::FunctionExpression
            | SyntaxKind::MethodDeclaration
    ) && id(node, "asteriskToken").is_some()
    {
        flags |= 1;
    }
    if matches!(
        node.syntax_kind,
        SyntaxKind::FunctionDeclaration
            | SyntaxKind::FunctionExpression
            | SyntaxKind::MethodDeclaration
            | SyntaxKind::ArrowFunction
    ) && has_modifier(ast, node, SyntaxKind::AsyncKeyword)
    {
        flags |= 2;
    }
    if id(node, "body").is_none() {
        flags |= 4;
    }
    FunctionFlags(flags)
}

/// TypeScript `ForEachReturnStatement`: visit own returns in source order
/// through the complete native control/block kinds. Nested functions and
/// other statements do not participate, including their descendant returns.
/// The explicit work stack preserves this traversal for deeply nested bodies.
pub(crate) fn return_statements(ast: &Ast<'_>, body: NodeId) -> Vec<NodeId> {
    let mut returns = Vec::new();
    let mut work = vec![body];
    while let Some(id) = work.pop() {
        match ast.node(id).syntax_kind {
            SyntaxKind::ReturnStatement => returns.push(id),
            SyntaxKind::CaseBlock
            | SyntaxKind::Block
            | SyntaxKind::IfStatement
            | SyntaxKind::DoStatement
            | SyntaxKind::WhileStatement
            | SyntaxKind::ForStatement
            | SyntaxKind::ForInStatement
            | SyntaxKind::ForOfStatement
            | SyntaxKind::WithStatement
            | SyntaxKind::SwitchStatement
            | SyntaxKind::CaseClause
            | SyntaxKind::DefaultClause
            | SyntaxKind::LabeledStatement
            | SyntaxKind::TryStatement
            | SyntaxKind::CatchClause => {
                let children: Vec<_> = ast.children(id).collect();
                work.extend(children.into_iter().rev());
            }
            _ => {}
        }
    }
    returns
}
