//! Typed field access over syntax nodes, shared by binding and declaration
//! projection. `Ast::walk` and `Ast::children` traverse the final tree.
//! Name text whose identity matters has its own owners: binding's exact
//! names and navigation's optional lookup keys.

use crate::syntax_kind::SyntaxKind;
use crate::{Ast, Node, NodeId, Value};

mod assignment;
mod declarations;
mod expression;
mod import_export;
pub(crate) use import_export::{
    is_reference_import, is_reference_import_type_literal, is_reference_traversal_header,
    reference_export_node,
};
mod optional;
mod reference_search;
mod returns;
pub(crate) use assignment::{
    AccessKind, access_kind, assignment_target, is_assignment_expression, is_assignment_operator,
    is_destructuring_assignment, is_literal_destructuring_pattern,
};
pub(crate) use declarations::{
    can_have_symbol, class_of_heritage_element, has_inferred_type, is_declaration,
    is_declaration_name, is_declaration_name_or_import_property_name, is_type_declaration,
    is_type_declaration_name, module_export_name_is_default, walk_up_binding_elements_and_patterns,
};

/// TypeScript `isVarConstLike`: unlike `isConstantVariable`'s Constant bit
/// test, the declaration's entire BlockScoped kind must be one of these
/// three native states. AwaitUsing contains both Const and Using bits.
pub(crate) fn is_var_const_like_flags(flags: u32) -> bool {
    use crate::compiler::node::{AWAIT_USING, CONST, LET, USING};
    matches!(flags & (LET | CONST | USING), CONST | USING | AWAIT_USING)
}
/// TypeScript `IsVarConst` uses equality with the full BlockScoped mask;
/// `await using` carries a Const bit but is not a const declaration.
pub(crate) fn is_var_const_flags(flags: u32) -> bool {
    use crate::compiler::node::{CONST, LET, USING};
    flags & (LET | CONST | USING) == CONST
}
pub(crate) use expression::{
    DottedNameStep, dotted_name_step, is_dotted_name, is_entity_name_expression, is_expression,
    is_expression_node, is_false_expression, is_import_call, is_late_bindable_ast,
    is_left_hand_side_expression, is_part_of_type_node,
};
pub(crate) use optional::{
    is_expression_of_optional_chain_root, is_optional_chain, is_optional_chain_root,
    is_outermost_optional_chain,
};
pub(crate) use reference_search::{
    is_string_or_numeric_literal_like, is_valid_reference_position,
    parameter_property_reference_anchor, reference_occurrence_range,
};
pub(crate) use returns::{FunctionFlags, function_flags, return_statements};
pub(crate) mod modifiers;

/// TypeScript `NodeCanBeDecorated`. This is the semantic eligibility gate,
/// distinct from merely having a decorator among a node's modifier tokens.
pub(crate) fn node_can_be_decorated(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    node: NodeId,
    legacy: bool,
) -> bool {
    use SyntaxKind as K;
    let current = ast.node(node);
    if legacy
        && id(current, "name").is_some_and(|id| ast.node(id).syntax_kind == K::PrivateIdentifier)
    {
        return false;
    }
    let parent = parents[node.index()];
    let class_like = |id| {
        matches!(
            ast.node(id).syntax_kind,
            K::ClassDeclaration | K::ClassExpression
        )
    };
    match current.syntax_kind {
        K::ClassDeclaration => true,
        K::ClassExpression => !legacy,
        K::PropertyDeclaration => parent.is_some_and(|parent| {
            if legacy {
                ast.node(parent).syntax_kind == K::ClassDeclaration
            } else {
                class_like(parent)
                    && !has_modifier(ast, current, K::AbstractKeyword)
                    && !has_modifier(ast, current, K::DeclareKeyword)
            }
        }),
        K::GetAccessor | K::SetAccessor | K::MethodDeclaration => parent.is_some_and(|parent| {
            id(current, "body").is_some()
                && if legacy {
                    ast.node(parent).syntax_kind == K::ClassDeclaration
                } else {
                    class_like(parent)
                }
        }),
        K::Parameter if legacy => parent.is_some_and(|parent| {
            let declaration = ast.node(parent);
            id(declaration, "body").is_some()
                && matches!(
                    declaration.syntax_kind,
                    K::Constructor | K::MethodDeclaration | K::SetAccessor
                )
                && !(ids(declaration, "parameters").first() == Some(&node)
                    && is_this_parameter(ast, node))
                && parents[parent.index()].is_some_and(|grandparent| {
                    ast.node(grandparent).syntax_kind == K::ClassDeclaration
                })
        }),
        _ => false,
    }
}

/// TypeScript `GetHeritageElements` / `GetHeritageClause`: only the first
/// clause with the requested token supplies the ordered native elements.
pub(crate) fn heritage_elements<'a>(
    ast: &'a Ast<'_>,
    declaration: NodeId,
    token: SyntaxKind,
) -> &'a [NodeId] {
    if !matches!(
        ast.node(declaration).syntax_kind,
        SyntaxKind::ClassDeclaration
            | SyntaxKind::ClassExpression
            | SyntaxKind::InterfaceDeclaration
    ) {
        return &[];
    }
    ids(ast.node(declaration), "heritageClauses")
        .iter()
        .find_map(|&clause| {
            (number(ast.node(clause), "token") == Some(token.number()))
                .then(|| ids(ast.node(clause), "types"))
        })
        .unwrap_or(&[])
}

/// TypeScript `IsAmbientModule`: string-named modules and global augmentations,
/// independently of the broader ambient declaration/context flag.
pub(crate) fn is_ambient_module(ast: &Ast<'_>, node: NodeId) -> bool {
    is_module_with_string_literal_name(ast, node)
        || ast.node(node).syntax_kind == SyntaxKind::ModuleDeclaration
            && ast.node(node).flags & crate::compiler::node::GLOBAL_AUGMENTATION != 0
}

/// TypeScript `IsModuleWithStringLiteralName`: global augmentations alone do
/// not qualify. ImportTracker's `isAmbientModuleDeclaration` asks this same
/// narrower question, independently of declaration ambient flags.
pub(crate) fn is_module_with_string_literal_name(ast: &Ast<'_>, node: NodeId) -> bool {
    let node = ast.node(node);
    node.syntax_kind == SyntaxKind::ModuleDeclaration
        && id(node, "name")
            .is_some_and(|name| ast.node(name).syntax_kind == SyntaxKind::StringLiteral)
}

/// Native Binder `bindEachStatementFunctionsFirst`: SourceFile, Block and
/// ModuleBlock bind FunctionDeclarations before other statements, preserving
/// order within each pass. Case/default clauses ask ordinary bindEach.
pub(crate) fn statement_bind_order<'a>(
    ast: &'a Ast<'_>,
    statements: &'a [NodeId],
) -> impl DoubleEndedIterator<Item = NodeId> + 'a {
    statements
        .iter()
        .copied()
        .filter(move |&id| ast.node(id).syntax_kind == SyntaxKind::FunctionDeclaration)
        .chain(
            statements
                .iter()
                .copied()
                .filter(move |&id| ast.node(id).syntax_kind != SyntaxKind::FunctionDeclaration),
        )
}

/// TypeScript `IsThisIdentifier`: an actual Identifier with the exact cooked
/// native text; a string literal named "this" is a different role.
pub(crate) fn is_this_identifier(ast: &Ast<'_>, id: NodeId) -> bool {
    ast.node(id).syntax_kind == SyntaxKind::Identifier && ast.identifier_name(id) == Some("this")
}

/// TypeScript `GetFirstIdentifier` follows the left receiver of an entity
/// name. A missing result means the caller supplied a different syntax family.
pub(crate) fn first_identifier(ast: &Ast<'_>, mut node: NodeId) -> Option<NodeId> {
    loop {
        let current = ast.node(node);
        match current.syntax_kind {
            SyntaxKind::Identifier => return Some(node),
            SyntaxKind::QualifiedName => node = id(current, "left")?,
            SyntaxKind::PropertyAccessExpression => node = id(current, "expression")?,
            _ => return None,
        }
    }
}

/// TypeScript `IsObjectLiteralMethod`: only MethodDeclaration with an actual
/// ObjectLiteralExpression parent, excluding getters and setters.
pub(crate) fn is_object_literal_method(
    ast: &Ast<'_>,
    parent: impl FnOnce(NodeId) -> Option<NodeId>,
    id: NodeId,
) -> bool {
    ast.node(id).syntax_kind == SyntaxKind::MethodDeclaration
        && parent(id).is_some_and(|parent| {
            ast.node(parent).syntax_kind == SyntaxKind::ObjectLiteralExpression
        })
}

/// TypeScript `IsConstTypeReference`: exact Identifier text, with no type
/// arguments. A qualified or argument-bearing name is an ordinary reference.
pub(crate) fn is_const_type_reference(ast: &Ast<'_>, id: NodeId) -> bool {
    let node = ast.node(id);
    node.syntax_kind == SyntaxKind::TypeReference
        && ids(node, "typeArguments").is_empty()
        && self::id(node, "typeName").is_some_and(|name| {
            ast.node(name).syntax_kind == SyntaxKind::Identifier
                && ast.identifier_name(name) == Some("const")
        })
}

/// TypeScript `IsConstAssertion`: SatisfiesExpression deliberately does not
/// share the assertion rule even when its type has the same spelling.
pub(crate) fn is_const_assertion(ast: &Ast<'_>, id: NodeId) -> bool {
    let node = ast.node(id);
    matches!(
        node.syntax_kind,
        SyntaxKind::AsExpression | SyntaxKind::TypeAssertionExpression
    ) && self::id(node, "type").is_some_and(|ty| is_const_type_reference(ast, ty))
}

/// TypeScript checker `isTypeAssertion` strips parentheses before asking
/// whether the expression is an assertion. This differs from IsConstAssertion,
/// whose callers inspect an immediate parent.
pub(crate) fn is_type_assertion(ast: &Ast<'_>, id: NodeId) -> bool {
    matches!(
        ast.node(skip_parentheses(ast, id)).syntax_kind,
        SyntaxKind::AsExpression | SyntaxKind::TypeAssertionExpression
    )
}

/// TypeScript `IsThisParameter`: test the actual Parameter declaration and
/// delegate the separate exact identifier question to its common owner.
pub(crate) fn is_this_parameter(ast: &Ast<'_>, id: NodeId) -> bool {
    let node = ast.node(id);
    node.syntax_kind == SyntaxKind::Parameter
        && self::id(node, "name").is_some_and(|name| is_this_identifier(ast, name))
}

/// TypeScript checker `GetSetAccessorValueParameter`: only an exactly two-
/// parameter setter treats a leading this parameter as a separate receiver.
/// Other arities select their first actual parameter, including a first this.
pub(crate) fn set_accessor_value_parameter(ast: &Ast<'_>, accessor: NodeId) -> Option<NodeId> {
    let parameters = ids(ast.node(accessor), "parameters");
    let &first = parameters.first()?;
    if parameters.len() == 2 && is_this_parameter(ast, first) {
        Some(parameters[1])
    } else {
        Some(first)
    }
}

/// TypeScript `IsRequireCall`: this is a syntax question, independently of
/// JavaScript mode, resolved symbols or the module loader's dependency rules.
pub(crate) fn is_require_call(ast: &Ast<'_>, node: NodeId, string_argument: bool) -> bool {
    let node = ast.node(node);
    if node.syntax_kind != SyntaxKind::CallExpression {
        return false;
    }
    let Some(callee) = id(node, "expression") else {
        return false;
    };
    let arguments = ids(node, "arguments");
    ast.node(callee).syntax_kind == SyntaxKind::Identifier
        && ast.identifier_name(callee) == Some("require")
        && arguments.len() == 1
        && (!string_argument
            || matches!(
                ast.node(arguments[0]).syntax_kind,
                SyntaxKind::StringLiteral | SyntaxKind::NoSubstitutionTemplateLiteral
            ))
}

/// TypeScript `IsPushOrUnshiftIdentifier`: the syntax role and exact decoded
/// identifier spelling belong to one operation used by native array flow.
pub(crate) fn is_push_or_unshift_identifier(ast: &Ast<'_>, id: NodeId) -> bool {
    ast.node(id).syntax_kind == SyntaxKind::Identifier
        && matches!(ast.identifier_name(id), Some("push" | "unshift"))
}

/// TypeScript `IsInTypeQuery` / `IsPartOfTypeQuery`: only Identifier/QualifiedName ancestry
/// reaches a typeof type query. Type arguments and enclosing type constructs
/// stop this question; ordinary typeof expressions are a different context.
pub(crate) fn is_in_type_query(
    ast: &Ast<'_>,
    mut parent: impl FnMut(NodeId) -> Option<NodeId>,
    mut id: NodeId,
) -> bool {
    loop {
        match ast.node(id).syntax_kind {
            SyntaxKind::TypeQuery => return true,
            SyntaxKind::Identifier | SyntaxKind::QualifiedName => {}
            _ => return false,
        }
        let Some(ancestor) = parent(id) else {
            return false;
        };
        id = ancestor;
    }
}

/// TypeScript `IsFunctionLikeKind`, including declaration and signature roles.
/// Index and JSDoc signatures are included even though their container flags
/// ask a different question.
pub(crate) fn is_function_like_kind(kind: SyntaxKind) -> bool {
    is_function_like_declaration_kind(kind)
        || matches!(
            kind,
            SyntaxKind::MethodSignature
                | SyntaxKind::CallSignature
                | SyntaxKind::JSDocSignature
                | SyntaxKind::ConstructSignature
                | SyntaxKind::IndexSignature
                | SyntaxKind::FunctionType
                | SyntaxKind::ConstructorType
        )
}

/// TypeScript `isFunctionLikeDeclarationKind` excludes signature declarations.
pub(crate) fn is_function_like_declaration_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::FunctionDeclaration
            | SyntaxKind::MethodDeclaration
            | SyntaxKind::Constructor
            | SyntaxKind::GetAccessor
            | SyntaxKind::SetAccessor
            | SyntaxKind::FunctionExpression
            | SyntaxKind::ArrowFunction
    )
}

/// TypeScript `IsTypeNodeKind`. Keyword and JSDoc cases are additional to
/// FirstTypeNode..LastTypeNode; ExpressionWithTypeArguments is deliberately
/// classified here even when its enclosing syntax is class heritage.
pub(crate) fn is_type_node_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::AnyKeyword
            | SyntaxKind::UnknownKeyword
            | SyntaxKind::NumberKeyword
            | SyntaxKind::BigIntKeyword
            | SyntaxKind::ObjectKeyword
            | SyntaxKind::BooleanKeyword
            | SyntaxKind::StringKeyword
            | SyntaxKind::SymbolKeyword
            | SyntaxKind::VoidKeyword
            | SyntaxKind::UndefinedKeyword
            | SyntaxKind::NeverKeyword
            | SyntaxKind::IntrinsicKeyword
            | SyntaxKind::ExpressionWithTypeArguments
            | SyntaxKind::JSDocAllType
            | SyntaxKind::JSDocNullableType
            | SyntaxKind::JSDocNonNullableType
            | SyntaxKind::JSDocOptionalType
            | SyntaxKind::JSDocVariadicType
    ) || (SyntaxKind::TypePredicate as u16..=SyntaxKind::ImportType as u16).contains(&(kind as u16))
}

/// TypeScript `IsFunctionExpressionOrArrowFunction`.
pub(crate) fn is_function_expression_or_arrow_function(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction
    )
}

/// TypeScript `IsObjectLiteralOrClassExpressionMethodOrAccessor`. A method
/// on a class declaration intentionally does not answer this question.
pub(crate) fn is_object_literal_or_class_expression_method_or_accessor(
    kind: SyntaxKind,
    parent: Option<SyntaxKind>,
) -> bool {
    matches!(
        kind,
        SyntaxKind::MethodDeclaration | SyntaxKind::GetAccessor | SyntaxKind::SetAccessor
    ) && matches!(
        parent,
        Some(SyntaxKind::ObjectLiteralExpression | SyntaxKind::ClassExpression)
    )
}

/// TypeScript `IsClassElement`; membership is distinct from modifier presence.
pub(crate) fn is_class_element(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Constructor
            | SyntaxKind::PropertyDeclaration
            | SyntaxKind::MethodDeclaration
            | SyntaxKind::GetAccessor
            | SyntaxKind::SetAccessor
            | SyntaxKind::IndexSignature
            | SyntaxKind::ClassStaticBlockDeclaration
            | SyntaxKind::SemicolonClassElement
    )
}

/// TypeScript `IsStatic`, including a static block's intrinsic static role.
pub(crate) fn is_static(ast: &Ast<'_>, id: NodeId) -> bool {
    let node = ast.node(id);
    is_class_element(node.syntax_kind) && has_modifier(ast, node, SyntaxKind::StaticKeyword)
        || node.syntax_kind == SyntaxKind::ClassStaticBlockDeclaration
}

/// TypeScript `GetThisContainer`: computed names and decorators leave their
/// class element's context; arrows are an explicit caller question. A captured
/// final tree normally reaches SourceFile; None means missing parent facts.
pub(crate) fn this_container(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    mut id: NodeId,
    include_arrows: bool,
    include_class_computed_name: bool,
) -> Option<NodeId> {
    loop {
        id = parents[id.index()]?;
        match ast.node(id).syntax_kind {
            SyntaxKind::ComputedPropertyName => {
                let element = parents[id.index()]?;
                let container = parents[element.index()]?;
                if include_class_computed_name
                    && matches!(
                        ast.node(container).syntax_kind,
                        SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
                    )
                {
                    return Some(id);
                }
                id = container;
            }
            SyntaxKind::Decorator => {
                let parent = parents[id.index()]?;
                if ast.node(parent).syntax_kind == SyntaxKind::Parameter {
                    let element = parents[parent.index()]?;
                    if is_class_element(ast.node(element).syntax_kind) {
                        id = element;
                    }
                } else if is_class_element(ast.node(parent).syntax_kind) {
                    id = parent;
                }
            }
            SyntaxKind::ArrowFunction => {
                if include_arrows {
                    return Some(id);
                }
            }
            SyntaxKind::FunctionDeclaration
            | SyntaxKind::FunctionExpression
            | SyntaxKind::ModuleDeclaration
            | SyntaxKind::ClassStaticBlockDeclaration
            | SyntaxKind::PropertyDeclaration
            | SyntaxKind::PropertySignature
            | SyntaxKind::MethodDeclaration
            | SyntaxKind::MethodSignature
            | SyntaxKind::Constructor
            | SyntaxKind::GetAccessor
            | SyntaxKind::SetAccessor
            | SyntaxKind::CallSignature
            | SyntaxKind::ConstructSignature
            | SyntaxKind::IndexSignature
            | SyntaxKind::EnumDeclaration
            | SyntaxKind::SourceFile => return Some(id),
            _ => {}
        }
    }
}

/// TypeScript `getSuperContainer` has its own smaller set of containers.
/// Computed names skip their member, and decorators leave a decorated class
/// element; it does not use GetThisContainer's source/module stop conditions.
pub(crate) fn super_container(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    mut node: NodeId,
    stop_on_functions: bool,
) -> Option<NodeId> {
    loop {
        node = parents[node.index()]?;
        match ast.node(node).syntax_kind {
            SyntaxKind::ComputedPropertyName => node = parents[node.index()]?,
            SyntaxKind::FunctionDeclaration
            | SyntaxKind::FunctionExpression
            | SyntaxKind::ArrowFunction => {
                if stop_on_functions {
                    return Some(node);
                }
            }
            SyntaxKind::PropertyDeclaration
            | SyntaxKind::PropertySignature
            | SyntaxKind::MethodDeclaration
            | SyntaxKind::MethodSignature
            | SyntaxKind::Constructor
            | SyntaxKind::GetAccessor
            | SyntaxKind::SetAccessor
            | SyntaxKind::ClassStaticBlockDeclaration => return Some(node),
            SyntaxKind::Decorator => {
                let parent = parents[node.index()]?;
                if ast.node(parent).syntax_kind == SyntaxKind::Parameter {
                    let element = parents[parent.index()]?;
                    if is_class_element(ast.node(element).syntax_kind) {
                        node = element;
                    }
                } else if is_class_element(ast.node(parent).syntax_kind) {
                    node = parent;
                }
            }
            _ => {}
        }
    }
}

/// TypeScript `isThisProperty` requires a direct this receiver on an access.
pub(crate) fn is_this_property(ast: &Ast<'_>, id: NodeId) -> bool {
    let node = ast.node(id);
    matches!(
        node.syntax_kind,
        SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
    ) && self::id(node, "expression")
        .is_some_and(|receiver| ast.node(receiver).syntax_kind == SyntaxKind::ThisKeyword)
}

/// TypeScript `GetImmediatelyInvokedFunctionExpression` recognizes a direct
/// call through parentheses, independently from other expression wrappers.
pub(crate) fn immediately_invoked_function(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    function: NodeId,
) -> Option<NodeId> {
    if !matches!(
        ast.node(function).syntax_kind,
        SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction
    ) {
        return None;
    }
    let mut expression = function;
    let mut parent = parents[expression.index()]?;
    while ast.node(parent).syntax_kind == SyntaxKind::ParenthesizedExpression {
        expression = parent;
        parent = parents[parent.index()]?;
    }
    (ast.node(parent).syntax_kind == SyntaxKind::CallExpression
        && id(ast.node(parent), "expression") == Some(expression))
    .then_some(parent)
}

/// TypeScript `getControlFlowContainer` skips immediately invoked functions;
/// the binder's retained flow-container inventory is a separate question.
pub(crate) fn control_flow_container(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    mut node: NodeId,
) -> Option<NodeId> {
    loop {
        node = parents[node.index()]?;
        let kind = ast.node(node).syntax_kind;
        if matches!(
            kind,
            SyntaxKind::ModuleBlock | SyntaxKind::SourceFile | SyntaxKind::PropertyDeclaration
        ) || is_function_like_kind(kind)
            && immediately_invoked_function(ast, parents, node).is_none()
        {
            return Some(node);
        }
    }
}

/// TypeScript `isInPropertyInitializerOrClassStaticBlock`: arrows are a
/// caller-controlled boundary; type queries and ordinary function bodies quit.
pub(crate) fn in_property_initializer_or_static_block(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    mut node: NodeId,
    ignore_arrows: bool,
) -> bool {
    loop {
        match ast.node(node).syntax_kind {
            SyntaxKind::PropertyDeclaration | SyntaxKind::ClassStaticBlockDeclaration => {
                return true;
            }
            SyntaxKind::TypeQuery | SyntaxKind::JsxClosingElement => return false,
            SyntaxKind::ArrowFunction if !ignore_arrows => return false,
            SyntaxKind::Block
                if parents[node.index()].is_some_and(|parent| {
                    is_function_like_kind(ast.node(parent).syntax_kind)
                        && ast.node(parent).syntax_kind != SyntaxKind::ArrowFunction
                }) =>
            {
                return false;
            }
            _ => {}
        }
        let Some(parent) = parents[node.index()] else {
            return false;
        };
        node = parent;
    }
}

/// A punctuation or keyword token: TypeScript's `FirstPunctuation` through
/// `LastPunctuation` and `FirstKeyword` through `LastKeyword`. Such a leaf
/// names nothing, whatever syntax it appears in.
pub(crate) fn is_token(kind: SyntaxKind) -> bool {
    let kind = kind.number();
    let range = |first: SyntaxKind, last: SyntaxKind| first.number()..=last.number();
    range(SyntaxKind::OpenBraceToken, SyntaxKind::CaretEqualsToken).contains(&kind)
        || range(SyntaxKind::BreakKeyword, SyntaxKind::DeferKeyword).contains(&kind)
}

pub(crate) fn child<'a, 'src>(
    ast: &'a Ast<'src>,
    node: &Node<'src>,
    field: &str,
) -> Option<&'a Node<'src>> {
    id(node, field).map(|id| ast.node(id))
}

pub(crate) fn id(node: &Node<'_>, field: &str) -> Option<NodeId> {
    match node.field(field) {
        Some(Value::Node(id)) => Some(*id),
        _ => None,
    }
}

pub(crate) fn ids<'a>(node: &'a Node<'_>, field: &str) -> &'a [NodeId] {
    match node.field(field) {
        Some(Value::Nodes(ids)) => ids,
        _ => &[],
    }
}

pub(crate) fn flag(node: &Node<'_>, field: &str) -> bool {
    matches!(node.field(field), Some(Value::Bool(true)))
}

pub(crate) fn number(node: &Node<'_>, field: &str) -> Option<u32> {
    match node.field(field) {
        Some(Value::Number(value)) => Some(*value),
        _ => None,
    }
}

/// The `parseSimpleUnaryExpression` / `parseUpdateExpression` operator
/// domains, projected from their numeric AST fields for semantic consumers.
/// Prefix and postfix expressions deliberately have different allowed sets.
pub(crate) fn unary_operator(node: &Node<'_>) -> Option<SyntaxKind> {
    let choices: &[SyntaxKind] = match node.syntax_kind {
        SyntaxKind::PrefixUnaryExpression => &[
            SyntaxKind::PlusToken,
            SyntaxKind::MinusToken,
            SyntaxKind::TildeToken,
            SyntaxKind::ExclamationToken,
            SyntaxKind::PlusPlusToken,
            SyntaxKind::MinusMinusToken,
        ],
        SyntaxKind::PostfixUnaryExpression => {
            &[SyntaxKind::PlusPlusToken, SyntaxKind::MinusMinusToken]
        }
        _ => return None,
    };
    let value = number(node, "operator")?;
    choices.iter().copied().find(|kind| kind.number() == value)
}

/// TypeScript `ModifiersToFlags` / `Node.ModifierFlags`: syntactic modifiers
/// alone, without effective JSDoc or inherited declaration flags.
pub(crate) fn modifier_flags(ast: &Ast<'_>, node: &Node<'_>) -> u32 {
    ids(node, "modifiers").iter().fold(0, |flags, &id| {
        flags | modifiers::flag(ast.node(id).syntax_kind)
    })
}

/// TypeScript `hasDefaultValue`: only property assignments recurse into
/// their initializer; parentheses and other expression wrappers do not.
pub(crate) fn has_default_value(ast: &Ast<'_>, mut node: NodeId) -> bool {
    loop {
        let data = ast.node(node);
        match data.syntax_kind {
            SyntaxKind::BindingElement => return id(data, "initializer").is_some(),
            SyntaxKind::PropertyAssignment => match id(data, "initializer") {
                Some(initializer) => node = initializer,
                None => return false,
            },
            SyntaxKind::ShorthandPropertyAssignment => {
                return id(data, "objectAssignmentInitializer").is_some();
            }
            SyntaxKind::BinaryExpression => {
                return id(data, "operatorToken")
                    .is_some_and(|token| ast.node(token).syntax_kind == SyntaxKind::EqualsToken);
            }
            _ => return false,
        }
    }
}

/// TypeScript `IsDynamicName`: computed and element-access names exclude
/// string/numeric literal-like and signed numeric expressions. Only element
/// access removes parentheses before that classification.
pub(crate) fn is_dynamic_name(ast: &Ast<'_>, name: NodeId) -> bool {
    let node = ast.node(name);
    let expression = match node.syntax_kind {
        SyntaxKind::ComputedPropertyName => id(node, "expression"),
        SyntaxKind::ElementAccessExpression => {
            id(node, "argumentExpression").map(|mut expression| {
                while ast.node(expression).syntax_kind == SyntaxKind::ParenthesizedExpression {
                    let Some(inner) = id(ast.node(expression), "expression") else {
                        break;
                    };
                    expression = inner;
                }
                expression
            })
        }
        _ => return false,
    };
    let Some(expression) = expression else {
        return false;
    };
    let expression = ast.node(expression);
    let signed_numeric = || {
        expression.syntax_kind == SyntaxKind::PrefixUnaryExpression
            && number(expression, "operator").is_some_and(|operator| {
                operator == SyntaxKind::PlusToken.number()
                    || operator == SyntaxKind::MinusToken.number()
            })
            && id(expression, "operand")
                .is_some_and(|operand| ast.node(operand).syntax_kind == SyntaxKind::NumericLiteral)
    };
    !(matches!(
        expression.syntax_kind,
        SyntaxKind::StringLiteral
            | SyntaxKind::NoSubstitutionTemplateLiteral
            | SyntaxKind::NumericLiteral
    ) || signed_numeric())
}

/// TypeScript `IsParameterPropertyDeclaration`: the actual declaration is
/// a Parameter, its parent is a Constructor, and it has any syntactic
/// accessibility/readonly/override parameter-property modifier.
pub(crate) fn is_parameter_property_declaration<'src>(
    ast: &Ast<'src>,
    node: &Node<'src>,
    parent: &Node<'src>,
) -> bool {
    node.syntax_kind == SyntaxKind::Parameter
        && parent.syntax_kind == SyntaxKind::Constructor
        && [
            SyntaxKind::PublicKeyword,
            SyntaxKind::PrivateKeyword,
            SyntaxKind::ProtectedKeyword,
            SyntaxKind::ReadonlyKeyword,
            SyntaxKind::OverrideKeyword,
        ]
        .into_iter()
        .any(|modifier| has_modifier(ast, node, modifier))
}

/// `HasSyntacticModifier` for one token kind through the shared modifier owner.
pub(crate) fn has_modifier<'s>(ast: &Ast<'s>, node: &Node<'s>, kind: SyntaxKind) -> bool {
    modifier_flags(ast, node) & modifiers::flag(kind) != 0
}

/// TypeScript `GetRootDeclaration`: nested binding elements climb through
/// their binding pattern to the containing declaration.
pub(crate) fn root_declaration(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    mut id: NodeId,
) -> NodeId {
    while ast.node(id).syntax_kind == SyntaxKind::BindingElement {
        let Some(root) = parents[id.index()].and_then(|parent| parents[parent.index()]) else {
            break;
        };
        id = root;
    }
    id
}

/// TypeScript `getCombinedFlags`: the shared root-declaration operation is
/// followed by the native variable-list/statement flag combination.
fn combined_flags(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
    flags_of: impl Fn(&Node<'_>) -> u32,
) -> u32 {
    let mut id = root_declaration(ast, parents, id);
    let mut flags = flags_of(ast.node(id));
    if ast.node(id).syntax_kind == SyntaxKind::VariableDeclaration
        && let Some(parent) = parents[id.index()]
    {
        id = parent;
    }
    if ast.node(id).syntax_kind == SyntaxKind::VariableDeclarationList {
        flags |= flags_of(ast.node(id));
        if let Some(parent) = parents[id.index()] {
            id = parent;
        }
    }
    if ast.node(id).syntax_kind == SyntaxKind::VariableStatement {
        flags |= flags_of(ast.node(id));
    }
    flags
}

/// TypeScript `GetCombinedNodeFlags`; declaration storage class is distinct
/// from syntactic modifiers and lexical scope classification.
pub(crate) fn combined_node_flags(ast: &Ast<'_>, parents: &[Option<NodeId>], id: NodeId) -> u32 {
    combined_flags(ast, parents, id, |node| node.flags)
}

/// Native TS7's private scanner hint uses the same `getCombinedFlags`
/// ancestry as public node flags, without changing their TS6 wire values.
pub(crate) fn possibly_deprecated(ast: &Ast<'_>, parents: &[Option<NodeId>], id: NodeId) -> bool {
    combined_flags(ast, parents, id, |node| {
        u32::from(
            node.native_jsdoc
                .as_ref()
                .is_some_and(|facts| facts.possible_deprecated),
        )
    }) != 0
}

/// TypeScript `GetCombinedModifierFlags`; an exported variable obtains its
/// export bit from the containing statement, never from its name's symbol.
pub(crate) fn combined_modifier_flags(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> u32 {
    combined_flags(ast, parents, id, |node| modifier_flags(ast, node))
}

/// Port of TypeScript's `SkipParentheses` (internal/ast/utilities.go): the
/// expression inside any number of parentheses around `id`, looking down
/// through parenthesized expressions only, never through assertions or
/// other wrappers. Its upward counterpart, `WalkUpParenthesizedExpressions`,
/// answers a different question and is not this operation.
pub(crate) fn skip_parentheses(ast: &Ast<'_>, mut id: NodeId) -> NodeId {
    while ast.node(id).syntax_kind == SyntaxKind::ParenthesizedExpression {
        match self::id(ast.node(id), "expression") {
            Some(inner) => id = inner,
            None => break,
        }
    }
    id
}

/// TypeScript `IsStatement`, including the parent-sensitive Block exclusion.
pub(crate) fn is_statement(ast: &Ast<'_>, parents: &[Option<NodeId>], node: NodeId) -> bool {
    use SyntaxKind as K;
    match ast.node(node).syntax_kind {
        K::BreakStatement
        | K::ContinueStatement
        | K::DebuggerStatement
        | K::DoStatement
        | K::ExpressionStatement
        | K::EmptyStatement
        | K::ForInStatement
        | K::ForOfStatement
        | K::ForStatement
        | K::IfStatement
        | K::LabeledStatement
        | K::ReturnStatement
        | K::SwitchStatement
        | K::ThrowStatement
        | K::TryStatement
        | K::VariableStatement
        | K::WhileStatement
        | K::WithStatement
        | K::NotEmittedStatement
        | K::FunctionDeclaration
        | K::MissingDeclaration
        | K::ClassDeclaration
        | K::InterfaceDeclaration
        | K::TypeAliasDeclaration
        | K::EnumDeclaration
        | K::ModuleDeclaration
        | K::ImportDeclaration
        | K::ImportEqualsDeclaration
        | K::ExportDeclaration
        | K::ExportAssignment
        | K::NamespaceExportDeclaration => true,
        K::Block => !parents[node.index()].is_some_and(|parent| {
            let kind = ast.node(parent).syntax_kind;
            matches!(kind, K::TryStatement | K::CatchClause) || is_function_like_kind(kind)
        }),
        _ => false,
    }
}

/// TypeScript `DeclarationNameToString` / `GetTextOfNode`: display retains
/// exact written spelling after the existing token/trivia owner. Nil and
/// zero-width names print the native missing-name marker; semantic identity
/// decoding is a separate question.
pub(crate) fn declaration_name_to_string(
    ast: &Ast<'_>,
    name: Option<NodeId>,
    control: Option<&crate::resources::Control>,
) -> String {
    use crate::resources::complete_or_abort;
    let text = if node_is_present(ast, name) {
        let name = name.expect("a present node is nonnil");
        let span = match control {
            Some(control) => complete_or_abort(ast.token_span_controlled(name, control)),
            None => ast.token_span(name),
        };
        &ast.source()[span.start..span.end]
    } else {
        "(Missing)"
    };
    let mut result = String::new();
    result.push_str(text);
    result
}

/// TypeScript `NodeIsPresent` / `NodeIsMissing` for captured source nodes.
/// Captured spans are nonnegative; synthetic node handles live outside Ast.
/// EOF remains present even though its source span is empty.
pub(crate) fn node_is_present(ast: &Ast<'_>, node: Option<NodeId>) -> bool {
    node.is_some_and(|node| {
        let node = ast.node(node);
        node.span.start != node.span.end || node.syntax_kind == SyntaxKind::EndOfFileToken
    })
}

/// TypeScript `FindConstructorDeclaration` / `GetFirstConstructorWithBody`:
/// select the first implementation, skipping every bodyless signature.
pub(crate) fn find_constructor_declaration(ast: &Ast<'_>, class: NodeId) -> Option<NodeId> {
    ids(ast.node(class), "members")
        .iter()
        .copied()
        .find(|&member| {
            let node = ast.node(member);
            node.syntax_kind == SyntaxKind::Constructor && node_is_present(ast, id(node, "body"))
        })
}

/// Port of TypeScript's `skipOuterExpressions` with `OuterExpressionKinds.All`.
/// Call-target inspection strips assertions and instantiation expressions;
/// `SkipParentheses` remains narrower for its existing callers.
pub(crate) fn skip_outer_expressions(ast: &Ast<'_>, node: NodeId) -> NodeId {
    skip_outer_expressions_ex(ast, node, true)
}

/// `skipOuterExpressions` with Assertions|Parentheses for reference checking,
/// or All for call targets. Instantiation is the only additional retained
/// source-node family in the latter operation.
pub(crate) fn skip_outer_expressions_ex(
    ast: &Ast<'_>,
    mut node: NodeId,
    instantiations: bool,
) -> NodeId {
    while is_outer_expression(ast, node, instantiations) {
        match id(ast.node(node), "expression") {
            Some(inner) => node = inner,
            None => break,
        }
    }
    node
}

/// `IsOuterExpression`: both upward and downward traversal share the native kinds.
pub(crate) fn is_outer_expression(ast: &Ast<'_>, node: NodeId, instantiations: bool) -> bool {
    matches!(
        ast.node(node).syntax_kind,
        SyntaxKind::ParenthesizedExpression
            | SyntaxKind::NonNullExpression
            | SyntaxKind::AsExpression
            | SyntaxKind::TypeAssertionExpression
            | SyntaxKind::SatisfiesExpression
    ) || instantiations && ast.node(node).syntax_kind == SyntaxKind::ExpressionWithTypeArguments
}

/// `IsNamedEvaluationSource`, including the narrow noncomputed `IsProtoSetter` gate.
pub(crate) fn is_named_evaluation_source(ast: &Ast<'_>, node: NodeId) -> bool {
    use SyntaxKind as K;
    let node = ast.node(node);
    let identifier_name =
        || id(node, "name").is_some_and(|name| ast.node(name).syntax_kind == K::Identifier);
    match node.syntax_kind {
        K::PropertyAssignment => {
            !id(node, "name").is_some_and(|name| match ast.node(name).syntax_kind {
                K::Identifier => ast.identifier_name(name) == Some("__proto__"),
                K::StringLiteral => {
                    ast.cooked_literal(ast.node(name)) == Some(b"__proto__".as_slice())
                }
                _ => false,
            })
        }
        K::ShorthandPropertyAssignment => id(node, "objectAssignmentInitializer").is_some(),
        K::VariableDeclaration => identifier_name() && id(node, "initializer").is_some(),
        K::Parameter | K::BindingElement => {
            identifier_name()
                && id(node, "initializer").is_some()
                && id(node, "dotDotDotToken").is_none()
        }
        K::PropertyDeclaration => id(node, "initializer").is_some(),
        K::BinaryExpression => {
            id(node, "operatorToken").is_some_and(|operator| {
                matches!(
                    ast.node(operator).syntax_kind,
                    K::EqualsToken
                        | K::AmpersandAmpersandEqualsToken
                        | K::BarBarEqualsToken
                        | K::QuestionQuestionEqualsToken
                )
            }) && id(node, "left").is_some_and(|left| ast.node(left).syntax_kind == K::Identifier)
        }
        K::ExportAssignment => true,
        _ => false,
    }
}

/// Port of `isCallOrNewExpressionTarget`, with the product's explicit tagged
/// template extension. Returns the called name token, excluding receivers,
/// arguments, implicit decorator calls and JSX tags. Literal element names
/// are retained for the member token inventory too.
pub(crate) fn call_target(ast: &Ast<'_>, call: NodeId) -> Option<NodeId> {
    if !matches!(
        ast.node(call).syntax_kind,
        SyntaxKind::CallExpression
            | SyntaxKind::NewExpression
            | SyntaxKind::TaggedTemplateExpression
    ) {
        return None;
    }
    let expression = invoked_expression(ast, call)?;
    let target = skip_outer_expressions(ast, expression);
    match ast.node(target).syntax_kind {
        SyntaxKind::Identifier | SyntaxKind::PrivateIdentifier => Some(target),
        SyntaxKind::PropertyAccessExpression => id(ast.node(target), "name"),
        SyntaxKind::ElementAccessExpression => {
            let key = id(ast.node(target), "argumentExpression")?;
            matches!(
                ast.node(key).syntax_kind,
                SyntaxKind::StringLiteral
                    | SyntaxKind::NumericLiteral
                    | SyntaxKind::NoSubstitutionTemplateLiteral
            )
            .then_some(key)
        }
        _ => None,
    }
}

/// `GetInvokedExpression`: the complete native syntax question is shared by
/// call acquisition and the narrower lexical call-target inspection. A tag,
/// JSX name, instanceof right operand or fragment is not an Expression field.
pub(crate) fn invoked_expression(ast: &Ast<'_>, call: NodeId) -> Option<NodeId> {
    let node = ast.node(call);
    match node.syntax_kind {
        SyntaxKind::TaggedTemplateExpression => id(node, "tag"),
        SyntaxKind::JsxOpeningElement | SyntaxKind::JsxSelfClosingElement => id(node, "tagName"),
        SyntaxKind::BinaryExpression => id(node, "right"),
        SyntaxKind::JsxOpeningFragment => Some(call),
        _ => id(node, "expression"),
    }
}

/// Whether a node of `kind` collects as JSDoc the comments trailing on the
/// line before it, as well as those leading it: the first half of
/// TypeScript's `GetJSDocCommentRanges` (internal/parser/utilities.go), whose
/// comment collection is `crate::lexer::jsdoc_comments`.
pub(crate) fn trailing_jsdoc(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Parameter
            | SyntaxKind::TypeParameter
            | SyntaxKind::FunctionExpression
            | SyntaxKind::ArrowFunction
            | SyntaxKind::ParenthesizedExpression
            | SyntaxKind::VariableDeclaration
            | SyntaxKind::ExportSpecifier
    )
}

/// TypeScript `IsBlockScope`, as used by `GetEnclosingBlockScopeContainer`.
/// A function body Block belongs to its function's scope, unlike other blocks.
pub(crate) fn is_block_scope(ast: &Ast<'_>, node: NodeId, parent: Option<NodeId>) -> bool {
    match ast.node(node).syntax_kind {
        SyntaxKind::SourceFile
        | SyntaxKind::CaseBlock
        | SyntaxKind::CatchClause
        | SyntaxKind::ModuleDeclaration
        | SyntaxKind::ForStatement
        | SyntaxKind::ForInStatement
        | SyntaxKind::ForOfStatement
        | SyntaxKind::Constructor
        | SyntaxKind::MethodDeclaration
        | SyntaxKind::GetAccessor
        | SyntaxKind::SetAccessor
        | SyntaxKind::FunctionDeclaration
        | SyntaxKind::FunctionExpression
        | SyntaxKind::ArrowFunction
        | SyntaxKind::PropertyDeclaration
        | SyntaxKind::ClassStaticBlockDeclaration => true,
        SyntaxKind::Block => !parent.is_some_and(|id| {
            is_function_like_kind(ast.node(id).syntax_kind)
                || ast.node(id).syntax_kind == SyntaxKind::ClassStaticBlockDeclaration
        }),
        _ => false,
    }
}

/// TypeScript `isInAmbientOrTypeNode` tests the node's ambient flag before
/// searching the enclosing type-declaration families, starting with the node.
/// Native JSTypeAliasDeclaration has no producer in the admitted TS/TSX tree.
pub(crate) fn is_in_ambient_or_type_node(
    ast: &Ast<'_>,
    mut parent: impl FnMut(NodeId) -> Option<NodeId>,
    node: NodeId,
) -> bool {
    if ast.node(node).flags & crate::compiler::node::AMBIENT != 0 {
        return true;
    }
    let mut current = Some(node);
    while let Some(id) = current {
        if matches!(
            ast.node(id).syntax_kind,
            SyntaxKind::InterfaceDeclaration
                | SyntaxKind::TypeAliasDeclaration
                | SyntaxKind::TypeLiteral
        ) {
            return true;
        }
        current = parent(id);
    }
    false
}

/// TypeScript checker `canHaveLocals`. Native JSTypeAliasDeclaration has no
/// producer in the admitted TS/TSX tree; JSDocSignature retains its own kind.
pub(crate) fn can_have_locals(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::ArrowFunction
            | SyntaxKind::Block
            | SyntaxKind::CallSignature
            | SyntaxKind::CaseBlock
            | SyntaxKind::CatchClause
            | SyntaxKind::ClassStaticBlockDeclaration
            | SyntaxKind::ConditionalType
            | SyntaxKind::Constructor
            | SyntaxKind::ConstructorType
            | SyntaxKind::ConstructSignature
            | SyntaxKind::ForStatement
            | SyntaxKind::ForInStatement
            | SyntaxKind::ForOfStatement
            | SyntaxKind::FunctionDeclaration
            | SyntaxKind::FunctionExpression
            | SyntaxKind::FunctionType
            | SyntaxKind::GetAccessor
            | SyntaxKind::IndexSignature
            | SyntaxKind::JSDocSignature
            | SyntaxKind::MappedType
            | SyntaxKind::MethodDeclaration
            | SyntaxKind::MethodSignature
            | SyntaxKind::ModuleDeclaration
            | SyntaxKind::SetAccessor
            | SyntaxKind::SourceFile
            | SyntaxKind::TypeAliasDeclaration
    )
}

/// TypeScript `IsExternalModuleImportEqualsDeclaration`.
pub(crate) fn is_external_module_import_equals_declaration(ast: &Ast<'_>, node: NodeId) -> bool {
    let node = ast.node(node);
    node.syntax_kind == SyntaxKind::ImportEqualsDeclaration
        && id(node, "moduleReference").is_some_and(|reference| {
            ast.node(reference).syntax_kind == SyntaxKind::ExternalModuleReference
        })
}

/// TypeScript `IsModuleExportsAccessExpression` uses the actual receiver and
/// GetElementOrPropertyAccessName; only an element argument skips parentheses.
pub(crate) fn is_module_exports_access_expression(ast: &Ast<'_>, node: NodeId) -> bool {
    let node = ast.node(node);
    if !matches!(
        node.syntax_kind,
        SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
    ) || id(node, "expression")
        .is_none_or(|receiver| ast.identifier_name(receiver) != Some("module"))
    {
        return false;
    }
    let name = if node.syntax_kind == SyntaxKind::PropertyAccessExpression {
        id(node, "name").filter(|&name| ast.node(name).syntax_kind == SyntaxKind::Identifier)
    } else {
        id(node, "argumentExpression")
            .map(|argument| skip_parentheses(ast, argument))
            .filter(|&argument| {
                matches!(
                    ast.node(argument).syntax_kind,
                    SyntaxKind::StringLiteral
                        | SyntaxKind::NoSubstitutionTemplateLiteral
                        | SyntaxKind::NumericLiteral
                )
            })
    };
    name.is_some_and(|name| matches!(ast.node(name).field("text").or_else(|| ast.node(name).field("escapedText")), Some(Value::Text(text)) if text.as_ref() == "exports"))
}

/// TypeScript `IsCallLikeExpression`, including the binary instanceof
/// signature question. This predicate does not imply a call argument list.
pub(crate) fn is_call_like_expression(ast: &Ast<'_>, node: NodeId) -> bool {
    match ast.node(node).syntax_kind {
        SyntaxKind::JsxOpeningElement
        | SyntaxKind::JsxSelfClosingElement
        | SyntaxKind::JsxOpeningFragment
        | SyntaxKind::CallExpression
        | SyntaxKind::NewExpression
        | SyntaxKind::TaggedTemplateExpression
        | SyntaxKind::Decorator => true,
        SyntaxKind::BinaryExpression => {
            id(ast.node(node), "operatorToken").is_some_and(|operator| {
                ast.node(operator).syntax_kind == SyntaxKind::InstanceOfKeyword
            })
        }
        _ => false,
    }
}

/// TypeScript `GetLeftmostExpression` follows the native precedence spine.
/// Parentheses, prefix operators, assertions and `new` deliberately terminate
/// this walk; this is distinct from skipping outer expressions.
pub(crate) fn leftmost_expression(
    ast: &Ast<'_>,
    mut node: NodeId,
    stop_at_calls: bool,
) -> Option<NodeId> {
    loop {
        let field = match ast.node(node).syntax_kind {
            SyntaxKind::PostfixUnaryExpression => "operand",
            SyntaxKind::BinaryExpression => "left",
            SyntaxKind::ConditionalExpression => "condition",
            SyntaxKind::TaggedTemplateExpression => "tag",
            SyntaxKind::CallExpression if stop_at_calls => return Some(node),
            SyntaxKind::CallExpression
            | SyntaxKind::AsExpression
            | SyntaxKind::ElementAccessExpression
            | SyntaxKind::PropertyAccessExpression
            | SyntaxKind::NonNullExpression
            | SyntaxKind::PartiallyEmittedExpression
            | SyntaxKind::SatisfiesExpression => "expression",
            _ => return Some(node),
        };
        node = id(ast.node(node), field)?;
    }
}
/// TypeScript `IsPrimitiveLiteralValue`: only numeric unary plus is admitted;
/// unary minus admits BigInt only when the caller includes that category.
pub(crate) fn is_primitive_literal_value(
    ast: &Ast<'_>,
    node: NodeId,
    include_bigint: bool,
) -> bool {
    let node = ast.node(node);
    match node.syntax_kind {
        SyntaxKind::TrueKeyword
        | SyntaxKind::FalseKeyword
        | SyntaxKind::NumericLiteral
        | SyntaxKind::StringLiteral
        | SyntaxKind::NoSubstitutionTemplateLiteral => true,
        SyntaxKind::BigIntLiteral => include_bigint,
        SyntaxKind::PrefixUnaryExpression => {
            let Some(operand) = id(node, "operand") else {
                return false;
            };
            let kind = ast.node(operand).syntax_kind;
            match number(node, "operator") {
                Some(operator) if operator == SyntaxKind::MinusToken as u32 => {
                    kind == SyntaxKind::NumericLiteral
                        || include_bigint && kind == SyntaxKind::BigIntLiteral
                }
                Some(operator) if operator == SyntaxKind::PlusToken as u32 => {
                    kind == SyntaxKind::NumericLiteral
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// TypeScript `isPropertyWithoutInitializer`: abstract and definite-assignment
/// properties do not enter initialization flow even when the initializer is nil.
pub(crate) fn is_property_without_initializer(ast: &Ast<'_>, node: NodeId) -> bool {
    let node = ast.node(node);
    node.syntax_kind == SyntaxKind::PropertyDeclaration
        && !has_modifier(ast, node, SyntaxKind::AbstractKeyword)
        && id(node, "exclamationToken").is_none()
        && id(node, "initializer").is_none()
}
