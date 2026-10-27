//! Typed reference-site attachment. Native token roles come from binding's
//! single inventory; captured AST identity is acquired only in this owner.

use super::super::model::DemandError;
use super::{DeclarationIdentity, Expression, ExpressionRoot, Source, TypeExpression};
use crate::analysis::SemanticFailureReason;
use crate::project::binding::{
    BoundJson, JsonPropertyId, ReferenceTokenFacts, ReferenceTokenRole, ReferenceTokenSource,
};
use crate::references::Location;

#[derive(Clone)]
pub(in crate::project::semantic) struct ReferenceSite {
    pub(in crate::project::semantic) key: String,
    pub(in crate::project::semantic) selection: Location,
    pub(in crate::project::semantic) occurrence: Location,
    pub(in crate::project::semantic) roles: u8,
    pub(in crate::project::semantic) call: bool,
    pub(in crate::project::semantic) target: ReferenceSiteTarget,
}

#[derive(Clone, Copy)]
pub(in crate::project::semantic) enum ReferenceSiteTarget {
    Captured {
        root: ExpressionRoot,
        role: ReferenceTokenRole,
    },
    Json {
        property: JsonPropertyId,
        object: BoundJson,
    },
}

pub(in crate::project::semantic) enum ReferenceSymbolRecipe {
    Declaration(DeclarationIdentity),
    Node(DeclarationIdentity),
    RawNode {
        declaration: DeclarationIdentity,
        required: bool,
    },
    ImmediateAlias(DeclarationIdentity),
    Entity {
        root: ExpressionRoot,
        meaning: crate::project::binding::NativeLookupMeaning,
        missing: EntitySymbolMissing,
    },
    Private {
        root: ExpressionRoot,
        description: String,
    },
    Identifier {
        root: ExpressionRoot,
        export_assignment: bool,
    },
    Access {
        root: ExpressionRoot,
        export_assignment: bool,
    },
    BindingProperty {
        pattern: ExpressionRoot,
        name: String,
    },
    ExpressionProperty {
        object: Expression,
        name: String,
    },
    TypeProperty {
        object: TypeExpression,
        name: String,
    },
    /// `isImportTypeQualifierPart`: the symbol `getTypeFromImportTypeNode`
    /// links to this qualifier name.
    ImportTypeQualifier {
        import: TypeExpression,
        name: ExpressionRoot,
    },
    Absent,
    Pending(&'static str),
}

#[derive(Clone, Copy)]
pub(in crate::project::semantic) enum EntitySymbolMissing {
    Quiet,
    Report,
    Unresolved,
}

impl Source<'_, '_> {
    pub(in crate::project::semantic) fn reference_entity_name_anchor(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<crate::Span, DemandError> {
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        super::entity_name_anchor(&captured.ast, node).ok_or_else(|| {
            super::super::internal("entity-symbol query is not an actual entity name").into()
        })
    }

    /// Product reference selection normalizes a parameter-property modifier
    /// to its actual declaration. This is not raw `getSymbolAtLocation` on
    /// that keyword; checker callers never inherit this selection policy.
    pub(in crate::project::semantic) fn selected_parameter_property_declaration(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<DeclarationIdentity, DemandError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let parent = captured.parents[id.index()]
            .ok_or_else(|| super::super::internal("selected parameter modifier lacks parent"))?;
        if captured.ast.node(parent).syntax_kind != crate::syntax_kind::SyntaxKind::Parameter {
            return Err(super::super::internal(
                "selected parameter modifier has nonparameter parent",
            )
            .into());
        }
        Ok(DeclarationIdentity::new(root.file, parent))
    }

    /// `getSymbolAtLocation` / `getSymbolOfNameOrPropertyAccessExpression`
    /// syntax dispatch. Recipes retain actual declaration/expression nodes;
    /// the evaluator owns all symbol/type questions and their order.
    pub(in crate::project::semantic) fn reference_symbol_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ReferenceSymbolRecipe, DemandError> {
        use crate::syntax_kind::SyntaxKind as K;
        use ReferenceSymbolRecipe as R;
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let mut id = captured.node_at(root.site)?;
        if ast.node(id).syntax_kind == K::SourceFile {
            return Ok(if inputs.bindings.file_is_module(root.file) {
                R::Node(DeclarationIdentity::new(root.file, id))
            } else {
                R::Absent
            });
        }
        if ast.node(id).flags & crate::compiler::node::IN_WITH_STATEMENT != 0 {
            return Ok(R::Absent);
        }
        let Some(parent_id) = parents[id.index()] else {
            return Ok(R::Absent);
        };
        let parent = ast.node(parent_id);
        if crate::syntax::is_declaration_name(ast, |node| parents[node.index()], id) {
            return Ok(R::Declaration(DeclarationIdentity::new(
                root.file, parent_id,
            )));
        }
        if matches!(parent.syntax_kind, K::ImportSpecifier | K::ExportSpecifier)
            && crate::syntax::id(parent, "propertyName") == Some(id)
        {
            return Ok(R::ImmediateAlias(DeclarationIdentity::new(
                root.file, parent_id,
            )));
        }
        if parent.syntax_kind == K::ComputedPropertyName
            && crate::syntax::is_string_or_numeric_literal_like(ast, id)
        {
            return Ok(match parents[parent_id.index()] {
                Some(declaration) => {
                    R::Declaration(DeclarationIdentity::new(root.file, declaration))
                }
                None => R::Absent,
            });
        }
        if matches!(ast.node(id).syntax_kind, K::Identifier | K::QualifiedName)
            && super::node_types::in_import_or_export_rhs(ast, parents, id)
            && parent.syntax_kind != K::ExportAssignment
        {
            // getSymbolOfPartOfRightHandSideOfImportEquals advances only an
            // identifier on the right, preserving namespace-only prefixes.
            if ast.node(id).syntax_kind == K::Identifier
                && parent.syntax_kind == K::QualifiedName
                && crate::syntax::id(parent, "right") == Some(id)
            {
                id = parent_id;
            }
            let owner = parents[id.index()]
                .ok_or_else(|| super::super::internal("import-equals entity lacks parent"))?;
            let meaning = if ast.node(id).syntax_kind == K::Identifier
                || ast.node(owner).syntax_kind == K::QualifiedName
            {
                crate::project::binding::NativeLookupMeaning::NAMESPACE
            } else {
                crate::project::binding::NativeLookupMeaning::ALL
            };
            return Ok(R::Entity {
                root: ExpressionRoot {
                    file: root.file,
                    site: crate::project::binding::flow::Site::new(ast, id),
                },
                meaning,
                missing: EntitySymbolMissing::Report,
            });
        }
        if ast.node(id).syntax_kind == K::Identifier
            && parent.syntax_kind == K::BindingElement
            && crate::syntax::id(parent, "propertyName") == Some(id)
            && parents[parent_id.index()]
                .is_some_and(|pattern| ast.node(pattern).syntax_kind == K::ObjectBindingPattern)
        {
            let pattern = parents[parent_id.index()].expect("object binding parent checked");
            return Ok(R::BindingProperty {
                pattern: ExpressionRoot {
                    file: root.file,
                    site: crate::project::binding::flow::Site::new(ast, pattern),
                },
                name: super::reference_imports::exact_text(inputs, root.file, ast, id)?,
            });
        }
        if ast.node(id).syntax_kind == K::Identifier && parent.syntax_kind == K::MetaProperty {
            return Ok(R::Pending(
                "native meta-property name requires its dedicated type/symbol producer",
            ));
        }
        // getSymbolOfNameOrPropertyAccessExpression asks an import type for
        // any of its qualifier names before climbing to a qualified parent.
        if matches!(ast.node(id).syntax_kind, K::Identifier | K::QualifiedName)
            && let Some(import) = crate::syntax::import_type_of_qualifier_part(ast, parents, id)
        {
            return Ok(R::ImportTypeQualifier {
                import: super::type_expression(inputs, root.file, captured, parents, import),
                name: ExpressionRoot {
                    file: root.file,
                    site: crate::project::binding::flow::Site::new(ast, id),
                },
            });
        }
        if matches!(
            ast.node(id).syntax_kind,
            K::Identifier | K::PrivateIdentifier | K::PropertyAccessExpression | K::QualifiedName
        ) {
            let export_assignment = parent.syntax_kind == K::ExportAssignment
                && crate::syntax::is_entity_name_expression(ast, id);
            while let Some(owner) = parents[id.index()] {
                let owner_node = ast.node(owner);
                let right = match owner_node.syntax_kind {
                    K::PropertyAccessExpression => crate::syntax::id(owner_node, "name"),
                    K::QualifiedName => crate::syntax::id(owner_node, "right"),
                    _ => None,
                };
                if right != Some(id) {
                    break;
                }
                id = owner;
            }
            let root = ExpressionRoot {
                file: root.file,
                site: crate::project::binding::flow::Site::new(ast, id),
            };
            if ast.node(id).syntax_kind == K::Identifier
                && crate::syntax::is_this_identifier(ast, id)
                && crate::syntax::is_in_type_query(ast, |node| parents[node.index()], id)
            {
                return Ok(R::Pending(
                    "native this-in-type-query symbol requires signature-this acquisition",
                ));
            }
            // isTypeReferenceIdentifier climbs every qualified parent, while
            // the selected entity above only climbed right-hand positions.
            let mut type_owner = id;
            while let Some(owner) = parents[type_owner.index()]
                .filter(|&owner| ast.node(owner).syntax_kind == K::QualifiedName)
            {
                type_owner = owner;
            }
            let type_reference = parents[type_owner.index()]
                .is_some_and(|owner| ast.node(owner).syntax_kind == K::TypeReference);
            return Ok(match ast.node(id).syntax_kind {
                K::PropertyAccessExpression | K::QualifiedName
                    if crate::syntax::is_expression_node(ast, parents, id) =>
                {
                    R::Access {
                        root,
                        export_assignment,
                    }
                }
                K::PrivateIdentifier if crate::syntax::is_expression_node(ast, parents, id) => {
                    R::Private {
                        root,
                        description: super::reference_imports::exact_text(
                            inputs, root.file, ast, id,
                        )?,
                    }
                }
                K::Identifier if crate::syntax::is_expression_node(ast, parents, id) => {
                    R::Identifier {
                        root,
                        export_assignment,
                    }
                }
                K::Identifier | K::QualifiedName if type_reference => R::Entity {
                    root,
                    meaning: if parents[id.index()]
                        .is_some_and(|owner| ast.node(owner).syntax_kind == K::TypeReference)
                    {
                        crate::project::binding::NativeLookupMeaning::TYPE
                    } else {
                        crate::project::binding::NativeLookupMeaning::NAMESPACE
                    },
                    missing: EntitySymbolMissing::Unresolved,
                },
                _ if parents[id.index()]
                    .is_some_and(|owner| ast.node(owner).syntax_kind == K::TypePredicate) =>
                {
                    R::Entity {
                        root,
                        meaning:
                            crate::project::binding::NativeLookupMeaning::FUNCTION_SCOPED_VARIABLE,
                        missing: EntitySymbolMissing::Quiet,
                    }
                }
                _ if crate::syntax::is_part_of_type_node(ast, parents, id) => {
                    R::Pending("native import-type/heritage symbol acquisition is pending")
                }
                _ => R::Absent,
            });
        }
        match ast.node(id).syntax_kind {
            K::ConstructorKeyword if parent.syntax_kind == K::Constructor => {
                let class = parents[parent_id.index()]
                    .ok_or_else(|| super::super::internal("constructor lacks class parent"))?;
                return Ok(R::RawNode {
                    declaration: DeclarationIdentity::new(root.file, class),
                    required: false,
                });
            }
            K::DefaultKeyword
            | K::FunctionKeyword
            | K::EqualsGreaterThanToken
            | K::ClassKeyword => {
                return Ok(R::Node(DeclarationIdentity::new(root.file, parent_id)));
            }
            K::ExportKeyword if parent.syntax_kind == K::ExportAssignment => {
                return Ok(R::RawNode {
                    declaration: DeclarationIdentity::new(root.file, parent_id),
                    required: true,
                });
            }
            K::ThisKeyword
            | K::ThisType
            | K::SuperKeyword
            | K::MetaProperty
            | K::InstanceOfKeyword => {
                return Ok(R::Pending(
                    "native symbol-at-location requires its this/super/meta/operator type producer",
                ));
            }
            K::ImportType => {
                return Ok(R::Pending(
                    "native import-type symbol requires its actual argument module",
                ));
            }
            K::ImportKeyword | K::NewKeyword if parent.syntax_kind == K::MetaProperty => {
                return Ok(R::Pending(
                    "native meta-property keyword requires its dedicated type producer",
                ));
            }
            _ => {}
        }
        if crate::syntax::is_string_or_numeric_literal_like(ast, id) {
            if parent.syntax_kind == K::ElementAccessExpression
                && crate::syntax::id(parent, "argumentExpression") == Some(id)
            {
                let object = crate::syntax::id(parent, "expression")
                    .ok_or_else(|| super::super::internal("element access lacks its receiver"))?;
                return Ok(R::ExpressionProperty {
                    object: super::expression(inputs, root.file, captured, object),
                    name: super::reference_imports::exact_text(inputs, root.file, ast, id)?
                        .to_owned(),
                });
            }
            if parent.syntax_kind == K::LiteralType
                && let Some(owner) = parents[parent_id.index()]
                && ast.node(owner).syntax_kind == K::IndexedAccessType
            {
                let object = crate::syntax::id(ast.node(owner), "objectType").ok_or_else(|| {
                    super::super::internal("indexed access lacks its object type")
                })?;
                return Ok(R::TypeProperty {
                    object: super::type_expression(inputs, root.file, captured, parents, object),
                    name: super::reference_imports::exact_text(inputs, root.file, ast, id)?
                        .to_owned(),
                });
            }
            if matches!(
                parent.syntax_kind,
                K::ExternalModuleReference
                    | K::ImportDeclaration
                    | K::ExportDeclaration
                    | K::CallExpression
            ) {
                return Ok(R::Pending(
                    "native module/Object.defineProperty literal symbol acquisition is pending",
                ));
            }
        }
        Ok(R::Absent)
    }

    pub(in crate::project::semantic) fn reference_declaration_origin(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<crate::project::binding::ReadOrigin, DemandError> {
        let bindings = self.bindings();
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        bindings
            .native_declaration_read_origin(declaration.file, node.syntax_kind, node.span)
            .map_err(|error| {
                super::super::binding_failure_at(
                    inputs,
                    &super::super::location(
                        inputs,
                        declaration.file,
                        captured.ast.token_span(declaration.node),
                    ),
                    error,
                )
            })
    }

    pub(in crate::project::semantic) fn reference_declaration_identity(
        &mut self,
        declaration: crate::project::binding::NativeSymbolDeclaration,
    ) -> Result<DeclarationIdentity, DemandError> {
        if declaration.json.is_some() {
            return Err(super::super::unsupported_demand_at_location(
                self.inputs,
                &super::super::location(self.inputs, declaration.file, declaration.span),
                SemanticFailureReason::UnsupportedSemanticForm,
                "captured TypeScript syntax question requires the JSON declaration adapter",
            ));
        }
        let captured = self.syntax(declaration.file)?;
        let node = captured.native_node_at(declaration.kind, declaration.span)?;
        Ok(DeclarationIdentity::new(declaration.file, node))
    }
    /// Project an immutable inventory handle onto its actual request-captured
    /// source node. Complete native span/kind selects the retained node role;
    /// neither the token span nor a load-parser NodeId becomes new identity.
    pub(in crate::project::semantic) fn reference_site(
        &mut self,
        facts: ReferenceTokenFacts<'_>,
    ) -> Result<ReferenceSite, DemandError> {
        let selection = super::super::location(self.inputs, facts.file, facts.selection_span);
        let occurrence = super::super::location(self.inputs, facts.file, facts.occurrence_span);
        let key = match facts.key {
            Ok(key) => key.to_owned(),
            Err(message) => {
                return Err(super::super::unsupported_demand_at_location(
                    self.inputs,
                    &selection,
                    SemanticFailureReason::UnsupportedSemanticForm,
                    message,
                ));
            }
        };
        let file = facts.file;
        let roles = facts.roles;
        let call = facts.call;
        let target = match facts.source {
            ReferenceTokenSource::Syntax(syntax) => {
                let captured = self.syntax(file)?;
                let node = captured.native_node_at(syntax.kind, syntax.native_span)?;
                ReferenceSiteTarget::Captured {
                    root: ExpressionRoot {
                        file,
                        site: crate::project::binding::flow::Site::new(&captured.ast, node),
                    },
                    role: syntax.role,
                }
            }
            ReferenceTokenSource::Json {
                property, object, ..
            } => ReferenceSiteTarget::Json { property, object },
        };
        Ok(ReferenceSite {
            key,
            selection,
            occurrence,
            roles,
            call,
            target,
        })
    }
}
