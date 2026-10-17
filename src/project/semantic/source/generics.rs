//! Canonical generic header questions over Source's one captured syntax cache.

use super::super::model::DeclaredVariance;
use super::{CapturedSyntax, Source, TypeExpression, type_expression};
use crate::analysis::AnalysisError;
use crate::project::binding::BoundDeclaration;
use crate::references::Location;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;

pub(in crate::project::semantic) struct TypeParameterRecipe {
    pub(in crate::project::semantic) name: String,
    pub(in crate::project::semantic) constraint: Option<ParameterConstraintRecipe>,
    pub(in crate::project::semantic) default: Option<TypeExpression>,
    pub(in crate::project::semantic) variance: Option<DeclaredVariance>,
    pub(in crate::project::semantic) constant: bool,
}

pub(in crate::project::semantic) struct BaseTypeRecipe {
    pub(in crate::project::semantic) expression: TypeExpression,
    pub(in crate::project::semantic) entity_anchor: Option<crate::Span>,
}

pub(in crate::project::semantic) struct ClassRecipe {
    pub(in crate::project::semantic) abstract_: bool,
    pub(in crate::project::semantic) base: Option<ClassBaseRecipe>,
}

pub(in crate::project::semantic) struct ClassBaseRecipe {
    pub(in crate::project::semantic) expression: super::Expression,
    pub(in crate::project::semantic) reference: TypeExpression,
    pub(in crate::project::semantic) type_arguments: Vec<TypeExpression>,
    pub(in crate::project::semantic) simple_entity: bool,
}

/// `GetExtendsHeritageClauseElements` selects the actual first extends
/// clause. Implements clauses never become either interface or class bases.
fn extends_elements<'a>(ast: &'a crate::Ast<'_>, id: crate::NodeId) -> &'a [crate::NodeId] {
    syntax::heritage_elements(ast, id, SyntaxKind::ExtendsKeyword)
}

impl Source<'_, '_> {
    /// Actual Node.TypeParameters consumed by `appendTypeParameters` and
    /// `getTypeParametersFromDeclaration`; no declared-type acquisition.
    pub(in crate::project::semantic) fn declaration_type_parameters(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<Vec<super::DeclarationIdentity>, super::super::model::DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(
            syntax::ids(captured.ast.node(declaration.node), "typeParameters")
                .iter()
                .map(|&id| super::DeclarationIdentity::new(declaration.file, id))
                .collect(),
        )
    }

    /// Predicate of `getOuterTypeParametersOfClassOrInterface`'s Find.
    pub(in crate::project::semantic) fn outer_parameter_declaration_candidate(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<bool, super::super::model::DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        Ok(node.syntax_kind == SyntaxKind::InterfaceDeclaration
            || node.syntax_kind == SyntaxKind::VariableDeclaration
                && syntax::id(node, "initializer").is_some_and(|id| {
                    matches!(
                        captured.ast.node(id).syntax_kind,
                        SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction
                    )
                }))
    }
}

/// Pure ancestry/modifier facts for the native getThisType operation.
#[derive(Clone)]
pub(in crate::project::semantic) struct ThisTypeContext {
    /// The actual class/interface node whose symbol `getThisType` acquires.
    pub(in crate::project::semantic) target: Option<crate::NodeId>,
    pub(in crate::project::semantic) static_member: bool,
    pub(in crate::project::semantic) constructor: bool,
    pub(in crate::project::semantic) in_constructor_body: bool,
}

pub(super) fn this_type_context(
    ast: &crate::Ast<'_>,
    parents: &[Option<crate::NodeId>],
    id: crate::NodeId,
) -> Result<ThisTypeContext, &'static str> {
    let container = syntax::this_container(ast, parents, id, false, false)
        .ok_or("captured ThisType has no native this-container parent facts")?;
    let parent = parents[container.index()];
    let target = match parent.filter(|&parent| {
        matches!(
            ast.node(parent).syntax_kind,
            SyntaxKind::ClassDeclaration
                | SyntaxKind::ClassExpression
                | SyntaxKind::InterfaceDeclaration
        )
    }) {
        Some(parent) => {
            syntax::id(ast.node(parent), "name")
                .ok_or("native unnamed class this-symbol attachment is pending")?;
            Some(parent)
        }
        None => None,
    };
    let constructor = ast.node(container).syntax_kind == SyntaxKind::Constructor;
    let body = syntax::id(ast.node(container), "body");
    let mut ancestor = Some(id);
    let mut in_constructor_body = false;
    while let Some(current) = ancestor {
        if body == Some(current) {
            in_constructor_body = true;
            break;
        }
        if current == container {
            break;
        }
        ancestor = parents[current.index()];
    }
    Ok(ThisTypeContext {
        target,
        static_member: syntax::is_static(ast, container),
        constructor,
        in_constructor_body,
    })
}

#[derive(Clone)]
pub(in crate::project::semantic) enum ParameterConstraintRecipe {
    Annotation {
        expression: Box<TypeExpression>,
        mapped_key: bool,
    },
    Inferred {
        location: Location,
    },
}

impl ParameterConstraintRecipe {
    pub(in crate::project::semantic) fn location(&self) -> &Location {
        match self {
            Self::Annotation { expression, .. } => &expression.location,
            Self::Inferred { location } => location,
        }
    }
}

impl Source<'_, '_> {
    /// `GetClassLikeDeclarationOfSymbol` searches the actual ordered declaration
    /// list. Inspect bound kind metadata without acquiring syntax; captured
    /// declarations use their already-associated node, never a name match.
    pub(in crate::project::semantic) fn first_class_declaration(
        &mut self,
        declarations: impl IntoIterator<Item = super::DeclarationReference>,
    ) -> Result<Option<super::DeclarationReference>, AnalysisError> {
        for declaration in declarations {
            let kind = match declaration {
                super::DeclarationReference::Bound(bound) => {
                    self.inputs.bindings.declaration_syntax_kind(bound)
                }
                super::DeclarationReference::Captured(captured) => {
                    self.declaration_kind(captured)?
                }
                super::DeclarationReference::Native(native) => native.kind,
            };
            if matches!(
                kind,
                SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
            ) {
                return Ok(Some(declaration));
            }
        }
        Ok(None)
    }

    /// `GetClassLikeDeclarationOfSymbol` has already selected this actual node.
    pub(in crate::project::semantic) fn class_recipe_of_declaration(
        &mut self,
        identity: super::DeclarationIdentity,
    ) -> Result<ClassRecipe, super::super::model::DemandError> {
        let captured = self.syntax(identity.file)?;
        let abstract_ = syntax::has_modifier(
            &captured.ast,
            captured.ast.node(identity.node),
            SyntaxKind::AbstractKeyword,
        );
        Ok(ClassRecipe {
            abstract_,
            base: self.class_extends_recipe(identity)?,
        })
    }

    /// `GetExtendsHeritageClauseElement` projects the first extends element
    /// of this actual class declaration, independently of merged symbols.
    pub(in crate::project::semantic) fn class_extends_recipe(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<Option<ClassBaseRecipe>, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        extends_elements(ast, declaration.node)
            .first()
            .map(|&base| {
                let expression = syntax::id(ast.node(base), "expression").ok_or_else(|| {
                    super::super::internal("class extends element has no expression")
                })?;
                Ok::<_, AnalysisError>(ClassBaseRecipe {
                    expression: super::expression(inputs, declaration.file, captured, expression),
                    reference: type_expression(
                        inputs,
                        declaration.file,
                        captured,
                        &captured.parents,
                        base,
                    ),
                    type_arguments: syntax::ids(ast.node(base), "typeArguments")
                        .iter()
                        .map(|&argument| {
                            type_expression(
                                inputs,
                                declaration.file,
                                captured,
                                &captured.parents,
                                argument,
                            )
                        })
                        .collect(),
                    simple_entity: matches!(
                        ast.node(expression).syntax_kind,
                        SyntaxKind::Identifier | SyntaxKind::PropertyAccessExpression
                    ),
                })
            })
            .transpose()
    }

    /// The syntax recipe for an already selected IndexSignature declaration.
    /// Computed-member classification and evaluation belong to the checker.
    pub(in crate::project::semantic) fn index_signature_of_declaration(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<Option<super::IndexSignatureRecipe>, super::super::model::DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        Ok(super::index_signature_recipe(
            inputs,
            declaration.file,
            captured,
            &captured.parents,
            declaration.node,
        ))
    }

    /// isLocalTypeAlias uses actual containing-function ancestry, separately
    /// from export visibility or the alias's printed name.
    pub(in crate::project::semantic) fn is_local_type_alias(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<bool, super::super::model::DemandError> {
        let captured = self.syntax(declaration.file)?;
        if captured.ast.node(declaration.node).syntax_kind != SyntaxKind::TypeAliasDeclaration {
            return Err(super::super::internal(
                "local alias question requires its actual type alias declaration",
            )
            .into());
        }
        let mut parent = captured.parents[declaration.node.index()];
        while let Some(id) = parent {
            if syntax::is_function_like_kind(captured.ast.node(id).syntax_kind) {
                return Ok(true);
            }
            parent = captured.parents[id.index()];
        }
        Ok(false)
    }
    /// `isThislessInterface` / `resolveBaseTypesOfInterface` share actual
    /// heritage syntax but ask separate semantic questions. The optional
    /// entity anchor is only the first operation's ignoreErrors lookup input.
    pub(in crate::project::semantic) fn interface_bases(
        &mut self,
        declaration: BoundDeclaration,
    ) -> Result<Option<Vec<BaseTypeRecipe>>, AnalysisError> {
        if self.inputs.bindings.declaration_syntax_kind(declaration)
            != SyntaxKind::InterfaceDeclaration
        {
            return Ok(None);
        }
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let id = captured
            .declaration_at(&[SyntaxKind::InterfaceDeclaration], declaration.anchor)?
            .ok_or_else(|| {
                super::super::internal("interface base association lacks final syntax")
            })?;
        let CapturedSyntax { ast, parents, .. } = captured;
        Ok(Some(
            extends_elements(ast, id)
                .iter()
                .map(|&id| BaseTypeRecipe {
                    expression: type_expression(inputs, declaration.file, captured, parents, id),
                    entity_anchor: syntax::id(ast.node(id), "expression")
                        .and_then(|name| super::entity_name_anchor(ast, name)),
                })
                .collect(),
        ))
    }
    /// `getConstraintDeclaration` and `getResolvedTypeParameterDefault` each
    /// select their own first nonnil field over complete canonical own
    /// associations. `getTypeParameterModifiers` ORs declared modifier bits
    /// across those same declarations; inferred relation variance is separate.
    pub(in crate::project::semantic) fn type_parameter(
        &mut self,
        declarations: &[super::DeclarationIdentity],
        first: BoundDeclaration,
    ) -> Result<TypeParameterRecipe, super::super::model::DemandError> {
        let inputs = self.inputs;
        if declarations.first().copied() != Some(self.declaration_identity(first)?) {
            return Err(super::super::internal(
                "parameter recipe lost its actual first declaration",
            )
            .into());
        }
        let mut constraint = None;
        let mut default = None;
        let mut inferred = None;
        let mut in_modifier = false;
        let mut out_modifier = false;
        let mut constant = false;
        for &declaration in declarations {
            let captured = self.syntax(declaration.file)?;
            let id = declaration.node;
            if captured.ast.node(id).syntax_kind != SyntaxKind::TypeParameter {
                return Err(super::super::internal(
                    "parameter recipe contains a non-parameter declaration",
                )
                .into());
            }
            let CapturedSyntax { ast, parents, .. } = captured;
            let node = ast.node(id);
            let parent = parents[id.index()].ok_or_else(|| {
                super::super::internal("type parameter has no final parent attachment")
            })?;
            let parent_kind = ast.node(parent).syntax_kind;
            if constraint.is_none() {
                constraint = syntax::id(node, "constraint").map(|id| {
                    ParameterConstraintRecipe::Annotation {
                        expression: Box::new(type_expression(
                            inputs,
                            declaration.file,
                            captured,
                            parents,
                            id,
                        )),
                        mapped_key: parent_kind == SyntaxKind::MappedType,
                    }
                });
            }
            if default.is_none() {
                default = syntax::id(node, "default")
                    .map(|id| type_expression(inputs, declaration.file, captured, parents, id));
            }
            // `getInferredTypeParameterConstraint` has semantic work only for
            // actual infer declarations. Ordinary parameter lists prove no
            // inferred constraint; an infer context cannot become known absent.
            if parent_kind == SyntaxKind::InferType && inferred.is_none() {
                inferred = Some(super::super::location(
                    inputs,
                    declaration.file,
                    captured.declaration_anchor(id),
                ));
            }
            in_modifier |= syntax::has_modifier(ast, node, SyntaxKind::InKeyword);
            out_modifier |= syntax::has_modifier(ast, node, SyntaxKind::OutKeyword);
            constant |= syntax::has_modifier(ast, node, SyntaxKind::ConstKeyword);
        }
        if constraint.is_none() {
            constraint = inferred.map(|location| ParameterConstraintRecipe::Inferred { location });
        }
        let variance = match (in_modifier, out_modifier) {
            (false, false) => None,
            (true, false) => Some(DeclaredVariance::In),
            (false, true) => Some(DeclaredVariance::Out),
            (true, true) => Some(DeclaredVariance::InOut),
        };
        let mut name = String::new();
        name.push_str(inputs.bindings.declaration_name(first));
        Ok(TypeParameterRecipe {
            name,
            constraint,
            default,
            variance,
            constant,
        })
    }
}
