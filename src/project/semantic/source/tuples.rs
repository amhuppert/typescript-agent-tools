//! Actual tuple/array node recipes; semantic alias probes remain lazy.

use super::{
    AliasRecipe, TypeExpression, alias_for_type_node, entity_name_anchor, type_expression,
};
use crate::analysis::SemanticFailureReason;
use crate::project::binding::NativeDeclaration;
use crate::project::binding::flow::Site;
use crate::project::semantic::inputs::CheckerInputs;
use crate::project::semantic::model::{TupleElementFlags, TupleElementInfo, TupleElementLabel};
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, Span, syntax};

#[derive(Clone)]
pub(in crate::project::semantic) struct ArrayOrTupleRecipe {
    pub(in crate::project::semantic) site: Site,
    pub(in crate::project::semantic) readonly: bool,
    /// None denotes the native Array/ReadonlyArray target, including a tuple
    /// containing one rest array. Some retains actual tuple element labels.
    pub(in crate::project::semantic) tuple: Option<Vec<TupleElementInfo>>,
    pub(in crate::project::semantic) elements: Vec<TypeExpression>,
    pub(in crate::project::semantic) variadic: bool,
    pub(in crate::project::semantic) deferred: DeferredReferenceRecipe,
}

#[derive(Clone)]
pub(in crate::project::semantic) struct TupleElementRecipe {
    pub(in crate::project::semantic) site: Site,
    pub(in crate::project::semantic) cached: bool,
    pub(in crate::project::semantic) optional: bool,
    pub(in crate::project::semantic) value: Box<TypeExpression>,
}

#[derive(Clone)]
pub(in crate::project::semantic) struct DeferredReferenceRecipe {
    pub(in crate::project::semantic) node: NativeDeclaration,
    pub(in crate::project::semantic) declaration: super::DeclarationIdentity,
    pub(in crate::project::semantic) alias: Option<AliasRecipe>,
    pub(in crate::project::semantic) resolved_by_alias: bool,
    pub(in crate::project::semantic) arguments: Vec<AliasResolutionRecipe>,
}

/// `mayResolveTypeAlias` has its own syntax traversal and short circuit. A
/// Reference asks only resolveTypeReferenceName's flags, never its type body.
#[derive(Clone)]
pub(in crate::project::semantic) enum AliasResolutionRecipe {
    No,
    Yes,
    Reference {
        file: FileId,
        anchor: Span,
        location: Location,
    },
    Some(Vec<Self>),
    Unavailable(Location),
}

pub(super) fn array_or_tuple_recipe(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &super::CapturedSyntax<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> ArrayOrTupleRecipe {
    let _depth = captured.resource_depth();
    let ast = &captured.ast;
    let node = ast.node(id);
    let element_nodes = if node.syntax_kind == SyntaxKind::ArrayType {
        vec![syntax::id(node, "elementType").expect("parsed ArrayType element")]
    } else {
        syntax::ids(node, "elements").to_vec()
    };
    let tuple = array_element_type_node(captured, id).is_none().then(|| {
        element_nodes
            .iter()
            .map(|&element| element_info(inputs, file, captured, element))
            .collect()
    });
    let variadic = node.syntax_kind == SyntaxKind::TupleType
        && element_nodes.iter().any(|&element| {
            element_flags(captured, element).intersects(TupleElementFlags::VARIADIC)
        });
    ArrayOrTupleRecipe {
        site: Site::new(ast, id),
        readonly: parents[id.index()].is_some_and(|parent| readonly_operator(ast, parent)),
        tuple,
        elements: element_nodes
            .iter()
            .map(|&element| type_expression(inputs, file, captured, parents, element))
            .collect(),
        variadic,
        deferred: deferred_reference_recipe(inputs, file, captured, parents, id, &element_nodes),
    }
}

/// `getTypeFromNamedTupleTypeNode`, `getTypeFromRestTypeNode`, and
/// `getTypeFromOptionalTypeNode` retain their distinct cache/optionality rules.
pub(super) fn element_recipe(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &super::CapturedSyntax<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> TupleElementRecipe {
    let _depth = captured.resource_depth();
    let ast = &captured.ast;
    let node = ast.node(id);
    let named = node.syntax_kind == SyntaxKind::NamedTupleMember;
    let rest = node.syntax_kind == SyntaxKind::RestType
        || named && syntax::id(node, "dotDotDotToken").is_some();
    let mut value = syntax::id(node, "type").expect("parsed tuple element type");
    if rest {
        value = array_element_type_node(captured, value).unwrap_or(value);
    }
    TupleElementRecipe {
        site: Site::new(ast, id),
        cached: named,
        optional: !rest
            && (node.syntax_kind == SyntaxKind::OptionalType
                || syntax::id(node, "questionToken").is_some()),
        value: Box::new(type_expression(inputs, file, captured, parents, value)),
    }
}

/// `getArrayElementTypeNode` deliberately unwraps parentheses and singleton
/// rest tuples, but no readonly operator. Rest syntax and alias probing ask
/// different native questions and must not share an expanded traversal.
fn array_element_type_node(captured: &super::CapturedSyntax<'_>, id: NodeId) -> Option<NodeId> {
    let ast = &captured.ast;
    let _depth = captured.resource_depth();
    let node = ast.node(id);
    match node.syntax_kind {
        SyntaxKind::ParenthesizedType => {
            array_element_type_node(captured, syntax::id(node, "type")?)
        }
        SyntaxKind::ArrayType => syntax::id(node, "elementType"),
        SyntaxKind::TupleType => {
            let [element] = syntax::ids(node, "elements") else {
                return None;
            };
            let element = ast.node(*element);
            if element.syntax_kind == SyntaxKind::RestType
                || element.syntax_kind == SyntaxKind::NamedTupleMember
                    && syntax::id(element, "dotDotDotToken").is_some()
            {
                array_element_type_node(captured, syntax::id(element, "type")?)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// `getTupleElementFlags` classifies the source node before acquiring a type.
pub(super) fn element_flags(captured: &super::CapturedSyntax<'_>, id: NodeId) -> TupleElementFlags {
    let ast = &captured.ast;

    let node = ast.node(id);
    if node.syntax_kind == SyntaxKind::OptionalType
        || node.syntax_kind == SyntaxKind::NamedTupleMember
            && syntax::id(node, "questionToken").is_some()
    {
        return TupleElementFlags::OPTIONAL;
    }
    if node.syntax_kind == SyntaxKind::RestType
        || node.syntax_kind == SyntaxKind::NamedTupleMember
            && syntax::id(node, "dotDotDotToken").is_some()
    {
        return if array_element_type_node(
            captured,
            syntax::id(node, "type").expect("parsed rest type"),
        )
        .is_some()
        {
            TupleElementFlags::REST
        } else {
            TupleElementFlags::VARIADIC
        };
    }
    TupleElementFlags::REQUIRED
}

/// `getTupleElementInfo` retains node identity separately from its exact name.
fn element_info(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &super::CapturedSyntax<'_>,
    id: NodeId,
) -> TupleElementInfo {
    let ast = &captured.ast;

    let node = ast.node(id);
    let label = (node.syntax_kind == SyntaxKind::NamedTupleMember)
        .then(|| tuple_element_label(inputs, file, captured, id));
    TupleElementInfo {
        flags: element_flags(captured, id),
        label,
    }
}

/// `getTupleElementLabel` retains a valid declaration's node separately
/// from its fallible exact name; tuple keys never use that display spelling.
pub(super) fn tuple_element_label(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &super::CapturedSyntax<'_>,
    id: NodeId,
) -> TupleElementLabel {
    let ast = &captured.ast;
    let name = syntax::id(ast.node(id), "name").expect("valid tuple label declaration has a name");
    TupleElementLabel {
        declaration: super::DeclarationIdentity::new(file, id),
        name: ast
            .identifier_name(name)
            .map(std::sync::Arc::from)
            .ok_or_else(|| {
                crate::project::semantic::unsupported_demand_at_location(
                    inputs,
                    &crate::project::semantic::location(inputs, file, ast.token_span(name)),
                    SemanticFailureReason::UnrepresentableIdentity,
                    "tuple label has no exact scalar identifier name",
                )
            }),
    }
}

pub(super) fn readonly_operator(ast: &Ast<'_>, id: NodeId) -> bool {
    let node = ast.node(id);
    node.syntax_kind == SyntaxKind::TypeOperator
        && syntax::number(node, "operator") == Some(SyntaxKind::ReadonlyKeyword.number())
}

pub(super) fn deferred_reference_recipe(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &super::CapturedSyntax<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
    arguments: &[NodeId],
) -> DeferredReferenceRecipe {
    let ast = &captured.ast;
    let _depth = captured.resource_depth();
    DeferredReferenceRecipe {
        node: NativeDeclaration {
            file,
            span: ast.node(id).span,
        },
        declaration: super::DeclarationIdentity::new(file, id),
        alias: alias_for_type_node(file, captured, parents, id),
        resolved_by_alias: resolved_by_type_alias(captured, parents, id),
        arguments: arguments
            .iter()
            .map(|&argument| alias_resolution_recipe(inputs, file, captured, argument))
            .collect(),
    }
}

/// `isResolvedByTypeAlias` follows only the native eager-container list.
fn resolved_by_type_alias(
    captured: &super::CapturedSyntax<'_>,
    parents: &[Option<NodeId>],
    mut id: NodeId,
) -> bool {
    let ast = &captured.ast;

    while let Some(parent) = parents[id.index()] {
        match ast.node(parent).syntax_kind {
            SyntaxKind::ParenthesizedType
            | SyntaxKind::NamedTupleMember
            | SyntaxKind::TypeReference
            | SyntaxKind::UnionType
            | SyntaxKind::IntersectionType
            | SyntaxKind::IndexedAccessType
            | SyntaxKind::ConditionalType
            | SyntaxKind::TypeOperator
            | SyntaxKind::ArrayType
            | SyntaxKind::TupleType => id = parent,
            SyntaxKind::TypeAliasDeclaration => return true,
            _ => return false,
        }
    }
    false
}

fn alias_resolution_recipe(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &super::CapturedSyntax<'_>,
    id: NodeId,
) -> AliasResolutionRecipe {
    let ast = &captured.ast;
    let _depth = captured.resource_depth();
    let node = ast.node(id);
    let child = |id| alias_resolution_recipe(inputs, file, captured, id);
    match node.syntax_kind {
        SyntaxKind::TypeReference => {
            let Some(anchor) =
                syntax::id(node, "typeName").and_then(|name| entity_name_anchor(ast, name))
            else {
                return AliasResolutionRecipe::Unavailable(crate::project::semantic::location(
                    inputs,
                    file,
                    ast.token_span(id),
                ));
            };
            AliasResolutionRecipe::Reference {
                file,
                anchor,
                location: crate::project::semantic::location(inputs, file, ast.token_span(id)),
            }
        }
        SyntaxKind::TypeQuery => AliasResolutionRecipe::Yes,
        SyntaxKind::TypeOperator
            if syntax::number(node, "operator") == Some(SyntaxKind::UniqueKeyword.number()) =>
        {
            AliasResolutionRecipe::No
        }
        SyntaxKind::TypeOperator
        | SyntaxKind::ParenthesizedType
        | SyntaxKind::OptionalType
        | SyntaxKind::NamedTupleMember => {
            child(syntax::id(node, "type").expect("parsed wrapped type"))
        }
        SyntaxKind::RestType => {
            let inner = syntax::id(node, "type").expect("parsed rest type");
            if ast.node(inner).syntax_kind == SyntaxKind::ArrayType {
                child(syntax::id(ast.node(inner), "elementType").expect("parsed array element"))
            } else {
                AliasResolutionRecipe::Yes
            }
        }
        SyntaxKind::UnionType | SyntaxKind::IntersectionType => AliasResolutionRecipe::Some(
            syntax::ids(node, "types")
                .iter()
                .map(|&id| child(id))
                .collect(),
        ),
        SyntaxKind::IndexedAccessType => AliasResolutionRecipe::Some(
            ["objectType", "indexType"]
                .into_iter()
                .map(|field| child(syntax::id(node, field).expect("parsed indexed-access operand")))
                .collect(),
        ),
        SyntaxKind::ConditionalType => AliasResolutionRecipe::Some(
            ["checkType", "extendsType", "trueType", "falseType"]
                .into_iter()
                .map(|field| child(syntax::id(node, field).expect("parsed conditional operand")))
                .collect(),
        ),
        _ => AliasResolutionRecipe::No,
    }
}
