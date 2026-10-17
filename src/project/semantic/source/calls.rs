//! Actual call syntax is captured once from the accepted final tree. Source
//! does not choose a signature or check any of its argument dependencies.

use super::{Expression, ExpressionRoot, Source, TypeExpression, expression, type_expression};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, Span};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic) struct CallRoot {
    pub(in crate::project::semantic) file: FileId,
    start: usize,
    end: usize,
    kind: SyntaxKind,
}
// Native call-node attachment hashes only fixed file/span/kind fields; it
// cannot visit source text or allocate identity payload.

impl CallRoot {
    pub(in crate::project::semantic) fn new(file: FileId, span: Span, kind: SyntaxKind) -> Self {
        Self {
            file,
            start: span.start,
            end: span.end,
            kind,
        }
    }

    pub(in crate::project::semantic) fn site(self) -> Site {
        Site {
            kind: self.kind,
            span: Span {
                start: self.start,
                end: self.end,
            },
        }
    }
}

pub(in crate::project::semantic) struct CallTypeArgument {
    pub(in crate::project::semantic) root: ExpressionRoot,
    pub(in crate::project::semantic) expression: TypeExpression,
}

pub(in crate::project::semantic) struct CallRecipe {
    pub(in crate::project::semantic) root: CallRoot,
    pub(in crate::project::semantic) location: Location,
    pub(in crate::project::semantic) callee: Expression,
    pub(in crate::project::semantic) arguments: Vec<Expression>,
    pub(in crate::project::semantic) template: Option<Expression>,
    pub(in crate::project::semantic) has_argument_list: bool,
    pub(in crate::project::semantic) type_arguments: Vec<CallTypeArgument>,
    pub(in crate::project::semantic) kind: SyntaxKind,
    pub(in crate::project::semantic) optional: bool,
    /// `checkCallExpression` distinguishes the call's own question-dot from
    /// inherited optional-chain flags before its assertion reporting tail.
    pub(in crate::project::semantic) assertion_statement: bool,
    pub(in crate::project::semantic) dotted_callee: bool,
    pub(in crate::project::semantic) outermost_chain: bool,
    pub(in crate::project::semantic) callee_optional_root: bool,
    pub(in crate::project::semantic) callee_optional_chain: bool,
    pub(in crate::project::semantic) super_call: bool,
    pub(in crate::project::semantic) import_call: bool,
    pub(in crate::project::semantic) require_call: bool,
    pub(in crate::project::semantic) symbol_candidate: Option<ExpressionRoot>,
}

pub(in crate::project::semantic) struct CallThisArgument {
    pub(in crate::project::semantic) expression: Expression,
    pub(in crate::project::semantic) optional_root: bool,
    pub(in crate::project::semantic) optional_chain: bool,
}

pub(in crate::project::semantic) struct ConstructorAccess {
    pub(in crate::project::semantic) class: super::DeclarationIdentity,
    pub(in crate::project::semantic) private: bool,
    pub(in crate::project::semantic) protected: bool,
}

impl Source<'_, '_> {
    /// `isPromiseResolveArityError` inspects the actual raw value declaration's
    /// three parent roles before acquiring the global Promise symbol.
    pub(in crate::project::semantic) fn promise_resolve_constructor(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<Option<ExpressionRoot>, AnalysisError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        if ast.node(declaration.node).syntax_kind != SyntaxKind::Parameter {
            return Ok(None);
        }
        let Some(function) = captured.parents[declaration.node.index()] else {
            return Ok(None);
        };
        if !matches!(
            ast.node(function).syntax_kind,
            SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction
        ) {
            return Ok(None);
        }
        let Some(parent) = captured.parents[function.index()] else {
            return Ok(None);
        };
        if ast.node(parent).syntax_kind != SyntaxKind::NewExpression {
            return Ok(None);
        }
        let constructor = syntax::id(ast.node(parent), "expression")
            .ok_or_else(|| super::super::internal("new expression has no constructor"))?;
        Ok(
            (ast.node(constructor).syntax_kind == SyntaxKind::Identifier).then_some(
                ExpressionRoot {
                    file: declaration.file,
                    site: Site::new(ast, constructor),
                },
            ),
        )
    }

    /// `isConstructorAccessible` reads nonpublic modifiers only from the
    /// signature's actual constructor declaration; inherited signatures retain
    /// that declaration instead of borrowing the selected class's modifiers.
    pub(in crate::project::semantic) fn constructor_access(
        &mut self,
        root: super::SignatureRoot,
    ) -> Result<Option<ConstructorAccess>, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site())?;
        let node = captured.ast.node(id);
        if node.syntax_kind != SyntaxKind::Constructor {
            return Ok(None);
        }
        let private = syntax::has_modifier(&captured.ast, node, SyntaxKind::PrivateKeyword);
        let protected = syntax::has_modifier(&captured.ast, node, SyntaxKind::ProtectedKeyword);
        if !(private || protected) {
            return Ok(None);
        }
        let class = captured.parents[id.index()]
            .ok_or_else(|| super::super::internal("constructor has no class parent"))?;
        Ok(Some(ConstructorAccess {
            class: super::DeclarationIdentity::new(root.file, class),
            private,
            protected,
        }))
    }

    /// `GetContainingClass` / `forEachEnclosingClass` walk actual ancestors
    /// starting at Parent. Functions and nested classes do not stop this walk.
    pub(in crate::project::semantic) fn enclosing_classes(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Vec<super::DeclarationIdentity>, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let mut parent = captured.parents[id.index()];
        let mut classes = Vec::new();
        while let Some(id) = parent {
            if matches!(
                captured.ast.node(id).syntax_kind,
                SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
            ) {
                classes.push(super::DeclarationIdentity::new(root.file, id));
            }
            parent = captured.parents[id.index()];
        }
        Ok(classes)
    }

    /// `HasModifier(class, Abstract)` is independent from signature flags:
    /// class/interface/namespace merges may retain both kinds of declaration.
    pub(in crate::project::semantic) fn class_declaration_is_abstract(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(declaration.file)?;
        Ok(syntax::has_modifier(
            &captured.ast,
            captured.ast.node(declaration.node),
            SyntaxKind::AbstractKeyword,
        ))
    }
    /// `checkSpreadExpression`, `getEffectiveCallArguments` and
    /// `getSpreadArgumentType` read the same actual spread operand.
    pub(in crate::project::semantic) fn spread_operand(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Expression, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let node = captured.ast.node(id);
        if node.syntax_kind != SyntaxKind::SpreadElement {
            return Err(super::super::internal(
                "spread operand requires a spread node",
            ));
        }
        let operand = syntax::id(node, "expression")
            .ok_or_else(|| super::super::internal("native spread node has no expression"))?;
        Ok(expression(inputs, root.file, captured, operand))
    }

    /// `getThisArgumentOfCall`: only the actual parenthesis/assertion-skipped
    /// access receiver supplies this. New-expression callees deliberately do not.
    pub(in crate::project::semantic) fn call_this_argument(
        &mut self,
        root: CallRoot,
    ) -> Result<Option<CallThisArgument>, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site())?;
        let ast = &captured.ast;
        let node = ast.node(id);
        if node.syntax_kind == SyntaxKind::BinaryExpression {
            let right = syntax::id(node, "right").ok_or_else(|| {
                super::super::internal("instanceof signature has no right operand")
            })?;
            return Ok(Some(CallThisArgument {
                expression: expression(inputs, root.file, captured, right),
                optional_root: syntax::is_optional_chain_root(ast, right),
                optional_chain: syntax::is_optional_chain(ast, right),
            }));
        }
        let callee = match node.syntax_kind {
            SyntaxKind::CallExpression => syntax::id(node, "expression"),
            SyntaxKind::TaggedTemplateExpression => syntax::id(node, "tag"),
            _ => None,
        };
        let Some(callee) = callee else {
            return Ok(None);
        };
        let callee = syntax::skip_outer_expressions(ast, callee);
        if !matches!(
            ast.node(callee).syntax_kind,
            SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
        ) {
            return Ok(None);
        }
        let receiver = syntax::id(ast.node(callee), "expression")
            .ok_or_else(|| super::super::internal("native this access has no receiver"))?;
        Ok(Some(CallThisArgument {
            expression: expression(inputs, root.file, captured, receiver),
            optional_root: syntax::is_optional_chain_root(ast, callee),
            optional_chain: syntax::is_optional_chain(ast, callee),
        }))
    }
    /// `getEffectiveCallArguments` for tagged templates retains each actual
    /// substitution node; the checker creates the first synthetic argument.
    pub(in crate::project::semantic) fn tagged_template_substitutions(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Vec<Expression>, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = ast.node(captured.node_at(root.site)?);
        if node.syntax_kind == SyntaxKind::NoSubstitutionTemplateLiteral {
            return Ok(Vec::new());
        }
        if node.syntax_kind != SyntaxKind::TemplateExpression {
            return Err(super::super::internal(
                "tagged template has no actual template node",
            ));
        }
        syntax::ids(node, "templateSpans")
            .iter()
            .map(|&span| {
                let operand = syntax::id(ast.node(span), "expression")
                    .ok_or_else(|| super::super::internal("template span has no substitution"))?;
                Ok(expression(inputs, root.file, captured, operand))
            })
            .collect()
    }

    /// `resolveTaggedTemplateExpression` suppresses ordinary invocation
    /// diagnostics only when its immediate parent is an array literal.
    pub(in crate::project::semantic) fn call_is_array_element(
        &mut self,
        root: CallRoot,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site())?;
        Ok(captured.parents[id.index()].is_some_and(|parent| {
            captured.ast.node(parent).syntax_kind == SyntaxKind::ArrayLiteralExpression
        }))
    }

    /// `resolveSignature` / `getQuickTypeOfExpression` acquire their actual
    /// final call node and argument lists through the same captured site.
    /// Accepted ordinary argument lists have their closing ')' because the
    /// parser's complete `arguments` operation requires it. Recovery-only
    /// incomplete call lists are not silently reconstructed from raw text.
    pub(in crate::project::semantic) fn call_recipe(
        &mut self,
        root: CallRoot,
    ) -> Result<CallRecipe, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site())?;
        let ast = &captured.ast;
        let node = ast.node(id);
        let binary = node.syntax_kind == SyntaxKind::BinaryExpression;
        let callee = if binary {
            syntax::id(node, "right")
        } else {
            syntax::invoked_expression(ast, id)
        }
        .ok_or_else(|| super::super::internal("actual call syntax has no callee"))?;
        let arguments = syntax::ids(node, "arguments");
        let arguments = if binary {
            vec![expression(
                inputs,
                root.file,
                captured,
                syntax::id(node, "left").ok_or_else(|| {
                    super::super::internal("instanceof signature has no left operand")
                })?,
            )]
        } else {
            arguments
                .iter()
                .map(|&id| expression(inputs, root.file, captured, id))
                .collect()
        };
        Ok(CallRecipe {
            root,
            location: super::super::location(inputs, root.file, ast.token_span(id)),
            callee: expression(inputs, root.file, captured, callee),
            arguments,
            template: syntax::id(node, "template")
                .map(|template| expression(inputs, root.file, captured, template)),
            has_argument_list: node.field("arguments").is_some(),
            type_arguments: syntax::ids(node, "typeArguments")
                .iter()
                .map(|&id| CallTypeArgument {
                    root: ExpressionRoot {
                        file: root.file,
                        site: Site::new(ast, id),
                    },
                    expression: type_expression(inputs, root.file, captured, &captured.parents, id),
                })
                .collect(),
            kind: node.syntax_kind,
            optional: syntax::is_optional_chain(ast, id),
            assertion_statement: node.syntax_kind == SyntaxKind::CallExpression
                && syntax::id(node, "questionDotToken").is_none()
                && captured.parents[id.index()].is_some_and(|parent| {
                    ast.node(parent).syntax_kind == SyntaxKind::ExpressionStatement
                }),
            dotted_callee: syntax::is_dotted_name(ast, callee),
            outermost_chain: syntax::is_outermost_optional_chain(
                ast,
                id,
                captured.parents[id.index()],
            ),
            callee_optional_root: syntax::is_expression_of_optional_chain_root(
                ast,
                callee,
                captured.parents[callee.index()],
            ),
            callee_optional_chain: syntax::is_optional_chain(ast, callee),
            super_call: ast.node(callee).syntax_kind == SyntaxKind::SuperKeyword,
            import_call: syntax::is_import_call(ast, id),
            require_call: syntax::is_require_call(ast, id, true),
            symbol_candidate: (node.syntax_kind == SyntaxKind::CallExpression)
                .then(|| symbol_candidate(ast, callee))
                .flatten()
                .map(|id| ExpressionRoot {
                    file: root.file,
                    site: Site::new(ast, id),
                }),
        })
    }
}

/// `isSymbolOrSymbolForCall`'s syntax precondition. The evaluator must still
/// acquire and compare the actual global constructor symbol before deciding
/// this exception; a local declaration named Symbol is not that global.
fn symbol_candidate(ast: &Ast<'_>, mut callee: crate::NodeId) -> Option<crate::NodeId> {
    let node = ast.node(callee);
    if node.syntax_kind == SyntaxKind::PropertyAccessExpression
        && syntax::id(node, "name").is_some_and(|name| ast.identifier_name(name) == Some("for"))
    {
        let receiver = syntax::id(node, "expression")?;
        callee = receiver;
    }
    (ast.node(callee).syntax_kind == SyntaxKind::Identifier
        && ast.identifier_name(callee) == Some("Symbol"))
    .then_some(callee)
}
