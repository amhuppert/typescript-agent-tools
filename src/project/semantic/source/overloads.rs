//! Actual declaration/group and context-sensitive syntax for call selection.

use super::super::model::DemandError;
use super::{CapturedSyntax, SignatureRoot, Source};
use crate::analysis::AnalysisError;
use crate::project::sources::FileId;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, syntax};

/// Native declaration.Parent identity in Source's one accepted tree. Equal
/// spans, display origins and parameter/return types cannot identify a group.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::project::semantic) struct SignatureParent {
    file: FileId,
    node: NodeId,
}

pub(in crate::project::semantic) struct SignatureDeclarationFacts {
    pub(in crate::project::semantic) parent: SignatureParent,
    pub(in crate::project::semantic) declaration: super::DeclarationIdentity,
}

impl Source<'_, '_> {
    /// `addImplementationSuccessElaboration` tests actual declaration kind
    /// and body presence before requesting its signature.
    pub(in crate::project::semantic) fn function_declaration_has_body(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        Ok(syntax::is_function_like_declaration_kind(node.syntax_kind)
            && syntax::node_is_present(&captured.ast, syntax::id(node, "body")))
    }

    /// `reorderCandidates`' actual getSymbolOfDeclaration/Parent inputs. The
    /// existing binding operation owns canonical merge and late binding; this
    /// source operation supplies the actual captured declaration identity.
    pub(in crate::project::semantic) fn signature_declaration_facts(
        &mut self,
        root: SignatureRoot,
    ) -> Result<SignatureDeclarationFacts, DemandError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site())?;
        let parent = captured.parents[id.index()]
            .ok_or_else(|| super::super::internal("signature declaration has no actual parent"))?;
        Ok(SignatureDeclarationFacts {
            parent: SignatureParent {
                file: root.file,
                node: parent,
            },
            declaration: super::DeclarationIdentity::new(root.file, id),
        })
    }

    /// Native HasContextSensitiveParameters observes the binder's final
    /// ContainsThis flag only after its syntactic parameter gates.
    pub(in crate::project::semantic) fn has_context_sensitive_parameters(
        &mut self,
        root: super::ExpressionRoot,
    ) -> Result<bool, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        context_sensitive_parameters(inputs, root.file, &captured.ast, id)
    }

    /// `isContextSensitive` / `isContextSensitiveFunctionLikeDeclaration` /
    /// `hasContextSensitiveReturnExpression`: native Some short circuit and
    /// actual node-role traversal, independent of argument type inference.
    pub(in crate::project::semantic) fn expression_is_context_sensitive(
        &mut self,
        root: super::ExpressionRoot,
    ) -> Result<bool, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let mut work = vec![ContextWork::Visit(id)];
        while let Some(task) = work.pop() {
            let id = match task {
                ContextWork::Yield(id) => {
                    let node = ast.node(id);
                    if node.syntax_kind == SyntaxKind::YieldExpression {
                        if let Some(operand) = syntax::id(node, "expression") {
                            work.push(ContextWork::Yield(operand));
                        }
                        work.push(ContextWork::Visit(id));
                    } else if !matches!(
                        node.syntax_kind,
                        SyntaxKind::EnumDeclaration
                            | SyntaxKind::InterfaceDeclaration
                            | SyntaxKind::ModuleDeclaration
                            | SyntaxKind::TypeAliasDeclaration
                    ) {
                        if syntax::is_function_like_kind(node.syntax_kind) {
                            if let Some(name) = syntax::id(node, "name").filter(|&name| {
                                ast.node(name).syntax_kind == SyntaxKind::ComputedPropertyName
                            }) {
                                let expression = syntax::id(ast.node(name), "expression")
                                    .ok_or_else(|| {
                                        super::super::internal(
                                            "computed function name has no expression",
                                        )
                                    })?;
                                work.push(ContextWork::Yield(expression));
                            }
                        } else if !syntax::is_part_of_type_node(ast, parents, id) {
                            let children: Vec<_> = ast.children(id).collect();
                            work.extend(children.into_iter().rev().map(ContextWork::Yield));
                        }
                    }
                    continue;
                }
                ContextWork::Visit(id) => id,
            };
            let node = ast.node(id);
            let children = match node.syntax_kind {
                SyntaxKind::FunctionExpression
                | SyntaxKind::ArrowFunction
                | SyntaxKind::MethodDeclaration
                | SyntaxKind::FunctionDeclaration => {
                    if context_sensitive_parameters(inputs, root.file, ast, id)? {
                        return Ok(true);
                    }
                    if syntax::function_flags(ast, id).is_generator() {
                        if let Some(body) = syntax::id(node, "body") {
                            work.push(ContextWork::Yield(body));
                        }
                    }
                    if node.field("typeParameters").is_some() || syntax::id(node, "type").is_some()
                    {
                        Vec::new()
                    } else if let Some(body) = syntax::id(node, "body") {
                        if ast.node(body).syntax_kind == SyntaxKind::Block {
                            syntax::return_statements(ast, body)
                                .into_iter()
                                .filter_map(|statement| {
                                    syntax::id(ast.node(statement), "expression")
                                })
                                .collect()
                        } else {
                            vec![body]
                        }
                    } else {
                        Vec::new()
                    }
                }
                SyntaxKind::ObjectLiteralExpression | SyntaxKind::JsxAttributes => {
                    let mut properties = syntax::ids(node, "properties").to_vec();
                    if node.syntax_kind == SyntaxKind::JsxAttributes {
                        if let Some(opening) = parents[id.index()].filter(|&parent| {
                            ast.node(parent).syntax_kind == SyntaxKind::JsxOpeningElement
                        }) {
                            let element = parents[opening.index()].ok_or_else(|| {
                                super::super::internal("JSX opening has no actual element")
                            })?;
                            properties
                                .extend_from_slice(syntax::ids(ast.node(element), "children"));
                        }
                    }
                    properties
                }
                SyntaxKind::ArrayLiteralExpression => syntax::ids(node, "elements").to_vec(),
                SyntaxKind::ConditionalExpression => {
                    required_children(ast, id, &["whenTrue", "whenFalse"])?
                }
                SyntaxKind::BinaryExpression => {
                    if syntax::id(node, "operatorToken").is_some_and(|operator| {
                        matches!(
                            ast.node(operator).syntax_kind,
                            SyntaxKind::BarBarToken | SyntaxKind::QuestionQuestionToken
                        )
                    }) {
                        required_children(ast, id, &["left", "right"])?
                    } else {
                        Vec::new()
                    }
                }
                SyntaxKind::PropertyAssignment => required_children(ast, id, &["initializer"])?,
                SyntaxKind::ParenthesizedExpression => required_children(ast, id, &["expression"])?,
                SyntaxKind::JsxAttribute => syntax::id(node, "initializer").into_iter().collect(),
                SyntaxKind::JsxExpression | SyntaxKind::YieldExpression => {
                    syntax::id(node, "expression").into_iter().collect()
                }
                _ => Vec::new(),
            };
            work.extend(children.into_iter().rev().map(ContextWork::Visit));
        }
        Ok(false)
    }
}

enum ContextWork {
    Visit(NodeId),
    Yield(NodeId),
}

fn required_children(
    ast: &Ast<'_>,
    id: NodeId,
    fields: &[&str],
) -> Result<Vec<NodeId>, AnalysisError> {
    fields
        .iter()
        .map(|field| {
            syntax::id(ast.node(id), field).ok_or_else(|| {
                super::super::internal("context-sensitive syntax lacks its required child")
            })
        })
        .collect()
}

fn context_sensitive_parameters(
    inputs: crate::project::semantic::inputs::CheckerInputs<'_>,
    file: crate::project::sources::FileId,
    ast: &Ast<'_>,
    id: NodeId,
) -> Result<bool, DemandError> {
    let node = ast.node(id);
    if node.field("typeParameters").is_some() {
        return Ok(false);
    }
    let parameters = syntax::ids(node, "parameters");
    if parameters
        .iter()
        .any(|&parameter| syntax::id(ast.node(parameter), "type").is_none())
    {
        return Ok(true);
    }
    if node.syntax_kind == SyntaxKind::ArrowFunction
        || parameters
            .first()
            .is_some_and(|&parameter| syntax::is_this_parameter(ast, parameter))
    {
        return Ok(false);
    }
    let location = super::super::location(inputs, file, ast.token_span(id));
    let graph = inputs
        .bindings
        .flow_graph(file)
        .map_err(|error| super::super::binding_failure_at(inputs, &location, error))?;
    Ok(graph
        .container_at(crate::project::binding::flow::Site::new(ast, id))
        .map_err(super::super::internal)?
        .contains_this)
}
