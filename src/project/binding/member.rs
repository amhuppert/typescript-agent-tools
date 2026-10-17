//! Native early member declaration roles and key domains. Table resolution
//! owns these identities; source locations only attach actual declarations.

use super::ids::DeclId;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, Node, NodeId, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) enum MemberKind {
    Property,
    AutoAccessor,
    Method,
    Getter,
    Setter,
    ParameterProperty,
    Call,
    Construct,
    Index,
    Constructor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum ReservedMember {
    Call,
    New,
    Index,
    Constructor,
}

/// Native internal/private names have a different key domain from exact
/// user strings. Rust cannot represent the native invalid-UTF8 prefix, so a
/// typed partition prevents it from colliding with a legal quoted key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum MemberKey {
    Text(String),
    NonScalar(Vec<u8>),
    Private(String),
    Reserved(ReservedMember),
    Dynamic,
}

#[derive(Debug)]
pub(super) struct MemberBinding {
    pub(super) owner: DeclId,
    /// `GetContainingClass` for a private name, distinct from the immediate
    /// member-table owner and retained before checker merges.
    pub(super) private_class: Option<DeclId>,
    pub(super) static_member: bool,
    pub(super) key: MemberKey,
    pub(super) optional: bool,
}

/// `getDeclarationName` / `HasDynamicName`: this early binder question keeps
/// signed numeric '+' and '-' spellings. The narrower property-name question
/// which removes unary '+' is a different operation, not this key owner.
pub(super) fn declaration_key(ast: &Ast<'_>, node: &Node<'_>) -> MemberKey {
    let Some(name) = syntax::id(node, "name") else {
        return MemberKey::Reserved(match node.syntax_kind {
            SyntaxKind::Constructor => ReservedMember::Constructor,
            SyntaxKind::CallSignature | SyntaxKind::FunctionType => ReservedMember::Call,
            SyntaxKind::ConstructSignature | SyntaxKind::ConstructorType => ReservedMember::New,
            SyntaxKind::IndexSignature => ReservedMember::Index,
            _ => unreachable!("captured native member has a name or signature role"),
        });
    };
    let name = ast.node(name);
    if name.syntax_kind == SyntaxKind::PrivateIdentifier {
        return MemberKey::Private(
            super::text::exact_text(ast, name)
                .expect("parser private identifiers retain exact scalar text")
                .to_owned(),
        );
    }
    if name.syntax_kind != SyntaxKind::ComputedPropertyName {
        return literal_key(ast, name);
    }
    let expression =
        syntax::id(name, "expression").expect("accepted computed property has its expression");
    let expression = ast.node(expression);
    if matches!(
        expression.syntax_kind,
        SyntaxKind::StringLiteral
            | SyntaxKind::NoSubstitutionTemplateLiteral
            | SyntaxKind::NumericLiteral
    ) {
        return literal_key(ast, expression);
    }
    if expression.syntax_kind == SyntaxKind::PrefixUnaryExpression
        && let Some(operand) = syntax::child(ast, expression, "operand")
        && operand.syntax_kind == SyntaxKind::NumericLiteral
        && let Some(operator) = syntax::number(expression, "operator")
        && (operator == SyntaxKind::PlusToken.number()
            || operator == SyntaxKind::MinusToken.number())
    {
        let text = super::text::exact_text(ast, operand)
            .expect("numeric literal payload has exact scalar text");
        return MemberKey::Text(format!(
            "{}{text}",
            if operator == SyntaxKind::PlusToken.number() {
                "+"
            } else {
                "-"
            }
        ));
    }
    MemberKey::Dynamic
}

fn literal_key(ast: &Ast<'_>, node: &Node<'_>) -> MemberKey {
    if let Some(bytes) = ast.cooked_literal(node) {
        return match std::str::from_utf8(bytes) {
            Ok(text) => MemberKey::Text(text.to_owned()),
            Err(_) => MemberKey::NonScalar(bytes.to_vec()),
        };
    }
    match node.field("escapedText").or_else(|| node.field("text")) {
        Some(Value::Text(text)) => MemberKey::Text(text.to_string()),
        _ => unreachable!("accepted literal member key has its parsed exact payload"),
    }
}

/// The declaration's actual member-name token, or its reserved role token.
/// Computed declarations retain the whole own name span for AST attachment;
/// Q3 key occurrence projection owns its separate literal-token question.
pub(in crate::project) fn declaration_anchor(ast: &Ast<'_>, id: NodeId) -> crate::Span {
    declaration_anchor_worker(ast, id, None).expect("uncontrolled declaration anchor cannot stop")
}

pub(in crate::project) fn declaration_anchor_controlled(
    ast: &Ast<'_>,
    id: NodeId,
    control: &crate::resources::Control,
) -> Result<crate::Span, crate::resources::Stop> {
    declaration_anchor_worker(ast, id, Some(control))
}

fn declaration_anchor_worker(
    ast: &Ast<'_>,
    id: NodeId,
    control: Option<&crate::resources::Control>,
) -> Result<crate::Span, crate::resources::Stop> {
    match (syntax::id(ast.node(id), "name"), control) {
        (Some(name), Some(control)) => ast.token_span_controlled(name, control),
        (None, Some(control)) => ast.first_token_span_controlled(id, control),
        (Some(name), None) => Ok(ast.token_span(name)),
        (None, None) => Ok(ast.first_token_span(id)),
    }
}
