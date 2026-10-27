//! Compiler expression classification over final-tree parent links.

use super::{id, number};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId};

/// Checker `isLateBindableAST`: only a computed name or an element-access
/// argument whose expression is an entity name can acquire a late name type.
pub(crate) fn is_late_bindable_ast(ast: &Ast<'_>, node: NodeId) -> bool {
    let node = ast.node(node);
    let expression = match node.syntax_kind {
        SyntaxKind::ComputedPropertyName => id(node, "expression"),
        SyntaxKind::ElementAccessExpression => id(node, "argumentExpression"),
        _ => None,
    };
    expression.is_some_and(|expression| is_entity_name_expression(ast, expression))
}

/// TypeScript `IsEntityNameExpressionEx` with the TypeScript-only allowJS
/// profile. Every receiver is direct syntax; parenthesized receivers and
/// private names do not form an entity name.
pub(crate) fn is_entity_name_expression(ast: &Ast<'_>, node: NodeId) -> bool {
    is_entity_name_expression_ex(ast, node, false)
}

/// TypeScript `IsEntityNameExpressionEx`: with `allow_js`, as in a
/// JavaScript file, `this` and an element access with a string or numeric
/// literal argument also form an entity name.
pub(crate) fn is_entity_name_expression_ex(
    ast: &Ast<'_>,
    mut node: NodeId,
    allow_js: bool,
) -> bool {
    loop {
        let syntax = ast.node(node);
        match syntax.syntax_kind {
            SyntaxKind::Identifier => return true,
            SyntaxKind::ThisKeyword => return allow_js,
            SyntaxKind::PropertyAccessExpression => {
                if !id(syntax, "name")
                    .is_some_and(|name| ast.node(name).syntax_kind == SyntaxKind::Identifier)
                {
                    return false;
                }
                let Some(receiver) = id(syntax, "expression") else {
                    return false;
                };
                node = receiver;
            }
            SyntaxKind::ElementAccessExpression if allow_js => {
                if !id(syntax, "argumentExpression")
                    .is_some_and(|argument| super::is_string_or_numeric_literal_like(ast, argument))
                {
                    return false;
                }
                let Some(receiver) = id(syntax, "expression") else {
                    return false;
                };
                node = receiver;
            }
            _ => return false,
        }
    }
}

/// TypeScript `IsDottedName`: parentheses preserve a dotted call target;
/// this, super and meta-properties qualify without an identifier-only gate.
/// Element accesses and non-null assertions do not satisfy this question.
/// The one-node decision also feeds the binder's postorder fact table.
pub(crate) fn dotted_name_step(ast: &Ast<'_>, node: NodeId) -> DottedNameStep {
    let current = ast.node(node);
    match current.syntax_kind {
        SyntaxKind::Identifier
        | SyntaxKind::ThisKeyword
        | SyntaxKind::SuperKeyword
        | SyntaxKind::MetaProperty => DottedNameStep::Result(true),
        SyntaxKind::PropertyAccessExpression | SyntaxKind::ParenthesizedExpression => {
            match id(current, "expression") {
                Some(expression) => DottedNameStep::Receiver(expression),
                None => DottedNameStep::Result(false),
            }
        }
        _ => DottedNameStep::Result(false),
    }
}

pub(crate) enum DottedNameStep {
    Result(bool),
    Receiver(NodeId),
}

/// `IsDottedName` for a single expression, without a recursive Rust stack.
pub(crate) fn is_dotted_name(ast: &Ast<'_>, mut node: NodeId) -> bool {
    loop {
        match dotted_name_step(ast, node) {
            DottedNameStep::Result(result) => return result,
            DottedNameStep::Receiver(expression) => node = expression,
        }
    }
}

/// Checker `isFalseExpression`: this is the syntactic reachability question
/// for an assertion argument, not general constant evaluation or truthiness.
/// Preserve native AND/OR short-circuit order without a recursive Rust stack.
pub(crate) fn is_false_expression(ast: &Ast<'_>, root: NodeId) -> bool {
    enum Work {
        Visit(NodeId),
        AfterLeft { right: NodeId, and: bool },
    }
    let mut work = vec![Work::Visit(root)];
    let mut value = false;
    while let Some(task) = work.pop() {
        match task {
            Work::AfterLeft { right, and } => {
                // Native && asks whether either side is syntactically false;
                // || asks whether both sides are. The other side is untouched
                // when the left already settles that exact question.
                if value != and {
                    work.push(Work::Visit(right));
                }
            }
            Work::Visit(node) => {
                let node = ast.node(super::skip_parentheses(ast, node));
                value = node.syntax_kind == SyntaxKind::FalseKeyword;
                if node.syntax_kind == SyntaxKind::BinaryExpression {
                    let operator = id(node, "operatorToken").map(|id| ast.node(id).syntax_kind);
                    if matches!(
                        operator,
                        Some(SyntaxKind::AmpersandAmpersandToken | SyntaxKind::BarBarToken)
                    ) {
                        let left = id(node, "left")
                            .expect("a parsed binary expression has a left operand");
                        let right = id(node, "right")
                            .expect("a parsed binary expression has a right operand");
                        work.push(Work::AfterLeft {
                            right,
                            and: operator == Some(SyntaxKind::AmpersandAmpersandToken),
                        });
                        work.push(Work::Visit(left));
                    }
                }
            }
        }
    }
    value
}

/// TypeScript `SkipPartiallyEmittedExpressions`: emitted wrappers retain an
/// operand by construction. A missing operand is an internal tree failure.
fn skip_partially_emitted(ast: &Ast<'_>, mut node: NodeId) -> NodeId {
    while ast.node(node).syntax_kind == SyntaxKind::PartiallyEmittedExpression {
        node = id(ast.node(node), "expression")
            .expect("a partially emitted expression retains its actual operand");
    }
    node
}

/// TypeScript `IsLeftHandSideExpression` / `isLeftHandSideExpressionKind`.
/// This grammar question differs from position-sensitive `IsExpressionNode`.
pub(crate) fn is_left_hand_side_expression(ast: &Ast<'_>, node: NodeId) -> bool {
    let node = skip_partially_emitted(ast, node);
    matches!(
        ast.node(node).syntax_kind,
        SyntaxKind::PropertyAccessExpression
            | SyntaxKind::ElementAccessExpression
            | SyntaxKind::NewExpression
            | SyntaxKind::CallExpression
            | SyntaxKind::JsxElement
            | SyntaxKind::JsxSelfClosingElement
            | SyntaxKind::JsxFragment
            | SyntaxKind::TaggedTemplateExpression
            | SyntaxKind::ArrayLiteralExpression
            | SyntaxKind::ParenthesizedExpression
            | SyntaxKind::ObjectLiteralExpression
            | SyntaxKind::ClassExpression
            | SyntaxKind::FunctionExpression
            | SyntaxKind::Identifier
            | SyntaxKind::PrivateIdentifier
            | SyntaxKind::RegularExpressionLiteral
            | SyntaxKind::NumericLiteral
            | SyntaxKind::BigIntLiteral
            | SyntaxKind::StringLiteral
            | SyntaxKind::NoSubstitutionTemplateLiteral
            | SyntaxKind::TemplateExpression
            | SyntaxKind::FalseKeyword
            | SyntaxKind::NullKeyword
            | SyntaxKind::ThisKeyword
            | SyntaxKind::TrueKeyword
            | SyntaxKind::SuperKeyword
            | SyntaxKind::NonNullExpression
            | SyntaxKind::ExpressionWithTypeArguments
            | SyntaxKind::MetaProperty
            | SyntaxKind::ImportKeyword
            | SyntaxKind::MissingDeclaration
    )
}

/// TypeScript `IsExpression` / `isExpressionKind`: this kind-only grammar
/// question is distinct from the position-sensitive IsExpressionNode below.
/// Its unary/left-hand-side portion uses the same native lower operation.
pub(crate) fn is_expression(ast: &Ast<'_>, node: NodeId) -> bool {
    let node = skip_partially_emitted(ast, node);
    matches!(
        ast.node(node).syntax_kind,
        SyntaxKind::ConditionalExpression
            | SyntaxKind::YieldExpression
            | SyntaxKind::ArrowFunction
            | SyntaxKind::BinaryExpression
            | SyntaxKind::SpreadElement
            | SyntaxKind::AsExpression
            | SyntaxKind::OmittedExpression
            | SyntaxKind::SatisfiesExpression
            | SyntaxKind::PrefixUnaryExpression
            | SyntaxKind::PostfixUnaryExpression
            | SyntaxKind::DeleteExpression
            | SyntaxKind::TypeOfExpression
            | SyntaxKind::VoidExpression
            | SyntaxKind::AwaitExpression
            | SyntaxKind::TypeAssertionExpression
    ) || is_left_hand_side_expression(ast, node)
}

/// Port of TypeScript `IsExpressionNode` and `IsInExpressionContext`.
/// Contextual token kinds climb through expression parents; declaration
/// names, module strings and type nodes do not become expressions merely
/// because the parser represents their text as an identifier or literal.
/// The mutually recursive compiler dispatch is an iterative ancestor walk.
pub(crate) fn is_expression_node(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    mut node: NodeId,
) -> bool {
    use SyntaxKind::*;
    loop {
        let parent = parents[node.index()];
        match ast.node(node).syntax_kind {
            SuperKeyword
            | NullKeyword
            | TrueKeyword
            | FalseKeyword
            | RegularExpressionLiteral
            | ArrayLiteralExpression
            | ObjectLiteralExpression
            | PropertyAccessExpression
            | ElementAccessExpression
            | CallExpression
            | NewExpression
            | TaggedTemplateExpression
            | AsExpression
            | TypeAssertionExpression
            | SatisfiesExpression
            | NonNullExpression
            | ParenthesizedExpression
            | FunctionExpression
            | ClassExpression
            | ArrowFunction
            | VoidExpression
            | DeleteExpression
            | TypeOfExpression
            | PrefixUnaryExpression
            | PostfixUnaryExpression
            | BinaryExpression
            | ConditionalExpression
            | SpreadElement
            | TemplateExpression
            | OmittedExpression
            | JsxElement
            | JsxSelfClosingElement
            | JsxFragment
            | YieldExpression
            | AwaitExpression => return true,
            MetaProperty => {
                return !parent.is_some_and(|parent| {
                    is_import_call(ast, parent) && id(ast.node(parent), "expression") == Some(node)
                });
            }
            ExpressionWithTypeArguments => {
                return !parent
                    .is_some_and(|parent| ast.node(parent).syntax_kind == HeritageClause);
            }
            QualifiedName => {
                while let Some(parent) = parents[node.index()]
                    .filter(|&parent| ast.node(parent).syntax_kind == QualifiedName)
                {
                    node = parent;
                }
                return name_expression_context(ast, parents, node);
            }
            PrivateIdentifier => {
                return parent.is_some_and(|parent| {
                    let parent = ast.node(parent);
                    parent.syntax_kind == BinaryExpression
                        && id(parent, "left") == Some(node)
                        && id(parent, "operatorToken")
                            .is_some_and(|operator| ast.node(operator).syntax_kind == InKeyword)
                });
            }
            Identifier if name_expression_context(ast, parents, node) => return true,
            Identifier
            | NumericLiteral
            | BigIntLiteral
            | StringLiteral
            | NoSubstitutionTemplateLiteral
            | ThisKeyword => {}
            _ => return false,
        }
        match in_expression_context(ast, parents, node) {
            Context::Answer(answer) => return answer,
            Context::Expression(parent) => node = parent,
        }
    }
}

enum Context {
    Answer(bool),
    Expression(NodeId),
}

/// One step of TypeScript `IsInExpressionContext`. The default dispatch asks
/// IsExpressionNode about the parent, retaining its complete narrower rules.
fn in_expression_context(ast: &Ast<'_>, parents: &[Option<NodeId>], node: NodeId) -> Context {
    use SyntaxKind::*;
    let Some(parent_id) = parents[node.index()] else {
        return Context::Answer(false);
    };
    let parent = ast.node(parent_id);
    let field_is = |field| id(parent, field) == Some(node);
    let non_declaration_initializer =
        || field_is("initializer") && ast.node(node).syntax_kind != VariableDeclarationList;
    Context::Answer(match parent.syntax_kind {
        VariableDeclaration | Parameter | PropertyDeclaration | PropertySignature | EnumMember
        | PropertyAssignment | BindingElement => field_is("initializer"),
        ExpressionStatement
        | IfStatement
        | DoStatement
        | WhileStatement
        | ReturnStatement
        | WithStatement
        | SwitchStatement
        | CaseClause
        | DefaultClause
        | ThrowStatement
        | TypeAssertionExpression
        | AsExpression
        | TemplateSpan
        | ComputedPropertyName
        | SatisfiesExpression => field_is("expression"),
        ForStatement => {
            non_declaration_initializer() || field_is("condition") || field_is("incrementor")
        }
        ForInStatement | ForOfStatement => non_declaration_initializer() || field_is("expression"),
        Decorator | JsxExpression | JsxSpreadAttribute | SpreadAssignment => true,
        ExpressionWithTypeArguments => {
            field_is("expression") && !is_type_expression_with_arguments(ast, parents, parent_id)
        }
        ShorthandPropertyAssignment => field_is("objectAssignmentInitializer"),
        _ => return Context::Expression(parent_id),
    })
}

/// TypeScript `IsTypeQueryNode`, `IsJSDocLinkLike`, `IsJSDocNameReference` and
/// `IsJsxTagName` are the exceptional name contexts used by IsExpressionNode.
fn name_expression_context(ast: &Ast<'_>, parents: &[Option<NodeId>], node: NodeId) -> bool {
    use SyntaxKind::*;
    parents[node.index()].is_some_and(|parent| {
        let parent = ast.node(parent);
        matches!(
            parent.syntax_kind,
            TypeQuery | JSDocLink | JSDocLinkCode | JSDocLinkPlain | JSDocNameReference
        ) || matches!(
            parent.syntax_kind,
            JsxOpeningElement | JsxClosingElement | JsxSelfClosingElement
        ) && id(parent, "tagName") == Some(node)
    })
}

/// TypeScript `isPartOfTypeExpressionWithTypeArguments`, the particular
/// IsPartOfTypeNode operation needed by IsInExpressionContext.
fn is_type_expression_with_arguments(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    use SyntaxKind::*;
    parents[node.index()].is_some_and(|parent| {
        let clause = ast.node(parent);
        clause.syntax_kind == HeritageClause
            && (!parents[parent.index()].is_some_and(|host| {
                matches!(
                    ast.node(host).syntax_kind,
                    ClassDeclaration | ClassExpression
                )
            }) || number(clause, "token") == Some(ImplementsKeyword.number()))
            || matches!(clause.syntax_kind, JSDocImplementsTag | JSDocAugmentsTag)
    })
}

/// TypeScript `IsPartOfTypeNode` / `isPartOfTypeNodeInParent`. This is a
/// parent-role question, distinct from the broader IsTypeNodeKind family.
pub(crate) fn is_part_of_type_node(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    mut node: NodeId,
) -> bool {
    use SyntaxKind::*;
    let in_type_range =
        |kind: SyntaxKind| (TypePredicate as u16..=ImportType as u16).contains(&(kind as u16));
    let kind = ast.node(node).syntax_kind;
    if in_type_range(kind) {
        return true;
    }
    match kind {
        AnyKeyword | UnknownKeyword | NumberKeyword | BigIntKeyword | StringKeyword
        | BooleanKeyword | SymbolKeyword | ObjectKeyword | UndefinedKeyword | NullKeyword
        | NeverKeyword => return true,
        VoidKeyword => {
            return parents[node.index()]
                .is_none_or(|parent| ast.node(parent).syntax_kind != VoidExpression);
        }
        ExpressionWithTypeArguments => {
            return is_type_expression_with_arguments(ast, parents, node);
        }
        TypeParameter => {
            return parents[node.index()].is_some_and(|parent| {
                matches!(ast.node(parent).syntax_kind, MappedType | InferType)
            });
        }
        Identifier => {
            if let Some(parent) = parents[node.index()] {
                let syntax = ast.node(parent);
                if syntax.syntax_kind == QualifiedName && id(syntax, "right") == Some(node)
                    || syntax.syntax_kind == PropertyAccessExpression
                        && id(syntax, "name") == Some(node)
                {
                    node = parent;
                }
            }
        }
        QualifiedName | PropertyAccessExpression | ThisKeyword => {}
        _ => return false,
    }
    let Some(parent) = parents[node.index()] else {
        return false;
    };
    let syntax = ast.node(parent);
    if syntax.syntax_kind == TypeQuery {
        return false;
    }
    if syntax.syntax_kind == ImportType {
        return !super::flag(syntax, "isTypeOf");
    }
    if in_type_range(syntax.syntax_kind) {
        return true;
    }
    match syntax.syntax_kind {
        ExpressionWithTypeArguments => is_type_expression_with_arguments(ast, parents, parent),
        TypeParameter => id(syntax, "constraint") == Some(node),
        VariableDeclaration
        | Parameter
        | PropertyDeclaration
        | PropertySignature
        | FunctionDeclaration
        | FunctionExpression
        | ArrowFunction
        | Constructor
        | MethodDeclaration
        | MethodSignature
        | GetAccessor
        | SetAccessor
        | CallSignature
        | ConstructSignature
        | IndexSignature
        | TypeAssertionExpression => id(syntax, "type") == Some(node),
        CallExpression | NewExpression | TaggedTemplateExpression => {
            super::ids(syntax, "typeArguments").contains(&node)
        }
        _ => false,
    }
}

/// Port of TypeScript `IsImportCall`: both import(...) and import.defer(...).
/// MetaProperty's IsExpressionNode question deliberately excludes the latter
/// callee, while treating import.meta as an expression.
pub(crate) fn is_import_call(ast: &Ast<'_>, node: NodeId) -> bool {
    use SyntaxKind::*;
    let node = ast.node(node);
    if node.syntax_kind != CallExpression {
        return false;
    }
    let Some(expression) = id(node, "expression") else {
        return false;
    };
    let expression_node = ast.node(expression);
    expression_node.syntax_kind == ImportKeyword
        || expression_node.syntax_kind == MetaProperty
            && number(expression_node, "keywordToken") == Some(ImportKeyword.number())
            && id(expression_node, "name")
                .is_some_and(|name| ast.identifier_name(name) == Some("defer"))
}
