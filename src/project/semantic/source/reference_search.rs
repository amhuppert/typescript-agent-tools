//! Syntax-side reference meaning and search containers. Symbol relationships
//! remain in the evaluator; binder declaration spaces are not these meanings.

use super::{
    DeclarationIdentity, ExpressionRoot, ReferenceSite, ReferenceSiteTarget, Source, TypeExpression,
};
use crate::project::binding::{ModuleInstanceState, ReferenceTokenRole, module_instance_state};
use crate::project::semantic::model::DemandError;
use crate::syntax::{self, id};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, Span};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::project::semantic) struct ReferenceMeaning(u8);

impl ReferenceMeaning {
    pub(in crate::project::semantic) const VALUE: Self = Self(1);
    pub(in crate::project::semantic) const TYPE: Self = Self(2);
    pub(in crate::project::semantic) const NAMESPACE: Self = Self(4);
    pub(in crate::project::semantic) const ALL: Self = Self(7);

    pub(in crate::project::semantic) fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// TypeScript `getIntersectingMeaningFromDeclarations`: declaration
    /// meanings can bridge spaces, so the union must reach a fixed point.
    pub(in crate::project::semantic) fn intersecting_declarations(
        mut self,
        declarations: &[Self],
    ) -> Self {
        loop {
            let previous = self;
            for &declaration in declarations {
                if self.intersects(declaration) {
                    self = self.union(declaration);
                }
            }
            if self == previous {
                return self;
            }
        }
    }
}

impl Source<'_, '_> {
    /// `GetTextOfPropertyName` is a text question, distinct from computing a
    /// property's literal name type. Dynamic computed names have no text.
    pub(in crate::project::semantic) fn reference_property_text(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<String>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let Some(mut name) = id(ast.node(declaration.node), "name") else {
            return Ok(None);
        };
        if ast.node(name).syntax_kind == SyntaxKind::ComputedPropertyName {
            let Some(expression) = id(ast.node(name), "expression") else {
                return Ok(None);
            };
            if !syntax::is_string_or_numeric_literal_like(ast, expression) {
                return Ok(None);
            }
            name = expression;
        }
        if !matches!(
            ast.node(name).syntax_kind,
            SyntaxKind::Identifier
                | SyntaxKind::PrivateIdentifier
                | SyntaxKind::StringLiteral
                | SyntaxKind::NumericLiteral
                | SyntaxKind::BigIntLiteral
                | SyntaxKind::NoSubstitutionTemplateLiteral
                | SyntaxKind::JsxNamespacedName
        ) {
            return Ok(None);
        }
        let text = |node| -> Result<String, DemandError> {
            let data = ast.node(node);
            match super::property_names::scalar_name_text(ast, data) {
                Ok(Some(text)) => Ok(text.to_owned()),
                _ => Err(super::super::unsupported_demand_at_location(
                    inputs,
                    &super::super::location(inputs, declaration.file, ast.token_span(node)),
                    crate::analysis::SemanticFailureReason::UnrepresentableIdentity,
                    "property text has no exact scalar identity",
                )),
            }
        };
        if ast.node(name).syntax_kind == SyntaxKind::JsxNamespacedName {
            let namespace = id(ast.node(name), "namespace")
                .ok_or_else(|| super::super::internal("JSX namespaced name lacks namespace"))?;
            let local = id(ast.node(name), "name")
                .ok_or_else(|| super::super::internal("JSX namespaced name lacks local name"))?;
            Ok(Some(format!("{}:{}", text(namespace)?, text(local)?)))
        } else {
            text(name).map(Some)
        }
    }

    /// `IsTypeInvalidDueToUnionDiscriminant` inspects the actual properties in
    /// source order. Spread elements are retained; their absent names matter.
    pub(in crate::project::semantic) fn reference_object_properties(
        &mut self,
        object: ExpressionRoot,
    ) -> Result<Vec<DeclarationIdentity>, DemandError> {
        let captured = self.syntax(object.file)?;
        let node = captured.node_at(object.site)?;
        Ok(syntax::ids(captured.ast.node(node), "properties")
            .iter()
            .map(|&property| DeclarationIdentity::new(object.file, property))
            .collect())
    }

    /// `getPropertySymbolFromBindingElement` is reached only for the immediate
    /// unrenamed object-binding element in ordinary reference search.
    pub(in crate::project::semantic) fn reference_binding_pattern(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<Option<ExpressionRoot>, DemandError> {
        let ReferenceSiteTarget::Captured { root, .. } = site.target else {
            return Ok(None);
        };
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        let Some(element) = captured.parents[node.index()] else {
            return Ok(None);
        };
        let Some(pattern) = captured.parents[element.index()] else {
            return Ok(None);
        };
        Ok(
            (captured.ast.node(element).syntax_kind == SyntaxKind::BindingElement
                && id(captured.ast.node(element), "propertyName").is_none()
                && captured.ast.node(pattern).syntax_kind == SyntaxKind::ObjectBindingPattern)
                .then(|| ExpressionRoot {
                    file: root.file,
                    site: crate::project::binding::flow::Site::new(&captured.ast, pattern),
                }),
        )
    }

    pub(in crate::project::semantic) fn reference_parameter_property(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(
            captured.parents[declaration.node.index()].is_some_and(|parent| {
                syntax::is_parameter_property_declaration(
                    &captured.ast,
                    captured.ast.node(declaration.node),
                    captured.ast.node(parent),
                )
            }),
        )
    }

    /// Find the immutable inventory handle for an actual declaration-name
    /// role. This preserves call/role/range facts when shorthand fallback adds
    /// a declaration site other than the candidate that led to it.
    pub(in crate::project::semantic) fn reference_declaration_name(
        &mut self,
        inventory: &crate::project::binding::Bindings,
        declaration: DeclarationIdentity,
    ) -> Result<Option<ReferenceSite>, DemandError> {
        let Some((file, name)) = self.declaration_name_site(declaration)? else {
            return Ok(None);
        };
        let token = inventory.reference_token_at(file, name.span.start);
        token
            .map(|token| self.reference_site(inventory.reference_token_facts(token)))
            .transpose()
    }

    /// Native `getContainingObjectLiteralElement` and its worker. Computed
    /// literal names can reach the element; computed identifiers cannot.
    pub(in crate::project::semantic) fn reference_object_literal_element(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<Option<(DeclarationIdentity, ExpressionRoot)>, DemandError> {
        let ReferenceSiteTarget::Captured { root, .. } = site.target else {
            return Ok(None);
        };
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        let Some(element) =
            containing_object_literal_element(&captured.ast, &captured.parents, node)
        else {
            return Ok(None);
        };
        let container =
            captured.parents[element.index()].expect("object literal element owner checked");
        Ok(Some((
            DeclarationIdentity::new(root.file, element),
            ExpressionRoot {
                file: root.file,
                site: crate::project::binding::flow::Site::new(&captured.ast, container),
            },
        )))
    }

    /// The shorthand acquisition in `forEachRelatedSymbol` asks the immediate
    /// location parent only after getContainingObjectLiteralElement succeeds.
    pub(in crate::project::semantic) fn reference_shorthand_declaration(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        let ReferenceSiteTarget::Captured { root, .. } = site.target else {
            return Ok(None);
        };
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        if containing_object_literal_element(&captured.ast, &captured.parents, node).is_none() {
            return Ok(None);
        }
        Ok(captured.parents[node.index()]
            .filter(|&parent| {
                captured.ast.node(parent).syntax_kind == SyntaxKind::ShorthandPropertyAssignment
            })
            .map(|parent| DeclarationIdentity::new(root.file, parent)))
    }

    /// Native `GetPropertySymbolOfDestructuringAssignment` checks the actual
    /// grandparent literal pattern, not a general assignment-target walk.
    pub(in crate::project::semantic) fn reference_destructuring_pattern(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<Option<ExpressionRoot>, DemandError> {
        let ReferenceSiteTarget::Captured { root, .. } = site.target else {
            return Ok(None);
        };
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        if containing_object_literal_element(&captured.ast, &captured.parents, node).is_none() {
            return Ok(None);
        }
        Ok(captured.parents[node.index()]
            .and_then(|parent| captured.parents[parent.index()])
            .filter(|&pattern| {
                syntax::is_literal_destructuring_pattern(
                    &captured.ast,
                    |id| captured.parents[id.index()],
                    pattern,
                )
            })
            .map(|pattern| ExpressionRoot {
                file: root.file,
                site: crate::project::binding::flow::Site::new(&captured.ast, pattern),
            }))
    }

    /// `getMergedAliasedSymbolOfNamespaceExportDeclaration` has a direct parent
    /// gate; ordinary NamespaceExport (`export * as`) is a different role.
    pub(in crate::project::semantic) fn reference_namespace_export_declaration(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        let ReferenceSiteTarget::Captured { root, .. } = site.target else {
            return Ok(None);
        };
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        Ok(captured.parents[node.index()]
            .filter(|&parent| {
                captured.ast.node(parent).syntax_kind == SyntaxKind::NamespaceExportDeclaration
            })
            .map(|parent| DeclarationIdentity::new(root.file, parent)))
    }

    /// The declaration ancestry gate of
    /// `skipPastExportOrImportSpecifierOrUnion`; parentheses and named interface
    /// constituents deliberately do not qualify as an inline union here.
    pub(in crate::project::semantic) fn reference_inline_union(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<TypeExpression>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let Some(parent) = captured.parents[declaration.node.index()] else {
            return Ok(None);
        };
        if captured.ast.node(parent).syntax_kind != SyntaxKind::TypeLiteral {
            return Ok(None);
        }
        let Some(union) = captured.parents[parent.index()] else {
            return Ok(None);
        };
        if captured.ast.node(union).syntax_kind != SyntaxKind::UnionType {
            return Ok(None);
        }
        Ok(Some(super::type_expression(
            inputs,
            declaration.file,
            captured,
            &captured.parents,
            union,
        )))
    }

    /// Native `getAllSuperTypeNodes`: every interface extends element, or the
    /// first class extends element followed by its implements elements. The
    /// evaluator must ask GetTypeAtLocation at these actual nodes, including
    /// the class heritage this-type behavior, rather than a type-only shortcut.
    pub(in crate::project::semantic) fn reference_super_type_nodes(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Vec<ExpressionRoot>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let mut nodes = Vec::new();
        match ast.node(declaration.node).syntax_kind {
            SyntaxKind::InterfaceDeclaration => nodes.extend_from_slice(syntax::heritage_elements(
                ast,
                declaration.node,
                SyntaxKind::ExtendsKeyword,
            )),
            SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression => {
                nodes.extend(
                    syntax::heritage_elements(ast, declaration.node, SyntaxKind::ExtendsKeyword)
                        .first()
                        .copied(),
                );
                nodes.extend_from_slice(syntax::heritage_elements(
                    ast,
                    declaration.node,
                    SyntaxKind::ImplementsKeyword,
                ));
            }
            _ => {}
        }
        Ok(nodes
            .into_iter()
            .map(|node| ExpressionRoot {
                file: declaration.file,
                site: crate::project::binding::flow::Site::new(ast, node),
            })
            .collect())
    }

    /// Native getReferencesAtLocation handles an import specifier's property
    /// name through ImportsResult.singleReferences, not the ordinary scan.
    pub(in crate::project::semantic) fn reference_is_import_property(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<bool, DemandError> {
        let ReferenceSiteTarget::Captured { root, .. } = site.target else {
            return Ok(false);
        };
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        Ok(captured.parents[node.index()].is_some_and(|parent| {
            let parent = captured.ast.node(parent);
            parent.syntax_kind == SyntaxKind::ImportSpecifier
                && id(parent, "propertyName") == Some(node)
        }))
    }

    /// Reattach Q2's declaration anchor to Q3's operation-specific token.
    /// A dotted namespace record denotes its final segment; a parameter
    /// property's declaration selector denotes its member modifier role.
    pub(in crate::project::semantic) fn reference_selector_anchor(
        &mut self,
        facts: crate::project::binding::ReferenceTokenFacts<'_>,
    ) -> Result<usize, DemandError> {
        let crate::project::binding::ReferenceTokenSource::Syntax(syntax) = facts.source else {
            return Ok(facts.selection_span.start);
        };
        let captured = self.syntax(facts.file)?;
        let node = captured.native_node_at(syntax.kind, syntax.native_span)?;
        let Some(declaration) = captured.parents[node.index()] else {
            return Ok(captured.ast.token_span(node).start);
        };
        if let Some(modifier) = syntax::parameter_property_reference_anchor(
            &captured.ast,
            |id| captured.parents[id.index()],
            declaration,
        ) {
            return Ok(captured.ast.token_span(modifier).start);
        }
        if captured.ast.node(declaration).syntax_kind == SyntaxKind::ModuleDeclaration
            && id(captured.ast.node(declaration), "name") == Some(node)
        {
            let mut declaration = declaration;
            while let Some(body) = id(captured.ast.node(declaration), "body") {
                if captured.ast.node(body).syntax_kind != SyntaxKind::ModuleDeclaration {
                    break;
                }
                declaration = body;
            }
            let name = id(captured.ast.node(declaration), "name")
                .ok_or_else(|| super::super::internal("namespace declaration has no name"))?;
            return Ok(captured.ast.token_span(name).start);
        }
        Ok(captured.ast.token_span(node).start)
    }

    /// Product target-kind dispatch over an actual native syntax role. Lexical
    /// shorthand, parameter names and computed value expressions retain their
    /// existing operation. Bound qualified namespace accesses are identified
    /// by the lexical selector before this member-only question is asked.
    pub(in crate::project::semantic) fn reference_is_member_site(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<bool, DemandError> {
        use ReferenceTokenRole as R;
        let ReferenceSiteTarget::Captured { root, role } = site.target else {
            return Ok(true);
        };
        match role {
            R::ParameterPropertyModifier
            | R::LiteralKey
            | R::BindingProperty
            | R::JsxAttribute
            | R::JsxNamespacedAttribute
            | R::PrivateName => return Ok(true),
            R::Shorthand | R::ParameterName | R::ComputedValue => return Ok(false),
            R::MemberName | R::Name => {}
        }
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        let Some(parent) = captured.parents[node.index()] else {
            return Ok(false);
        };
        let owner = captured.ast.node(parent);
        Ok(id(owner, "name") == Some(node)
            && matches!(
                owner.syntax_kind,
                SyntaxKind::PropertyDeclaration
                    | SyntaxKind::PropertySignature
                    | SyntaxKind::PropertyAssignment
                    | SyntaxKind::MethodDeclaration
                    | SyntaxKind::MethodSignature
                    | SyntaxKind::GetAccessor
                    | SyntaxKind::SetAccessor
                    | SyntaxKind::EnumMember
                    | SyntaxKind::PropertyAccessExpression
            ))
    }

    /// Native `isValidReferencePosition` on the attached node. Selection-only
    /// anchors do not become extra candidate roles. JSON property keys have
    /// their own captured syntax and always satisfy this property-name gate.
    pub(in crate::project::semantic) fn reference_position_is_valid(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<bool, DemandError> {
        let ReferenceSiteTarget::Captured { root, .. } = site.target else {
            return Ok(true);
        };
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        Ok(syntax::is_valid_reference_position(
            &captured.ast,
            |id| captured.parents[id.index()],
            node,
        ))
    }

    pub(in crate::project::semantic) fn reference_meaning(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<ReferenceMeaning, DemandError> {
        let ReferenceSiteTarget::Captured { root, role } = site.target else {
            return Ok(ReferenceMeaning::VALUE);
        };
        let captured = self.syntax(root.file)?;
        let mut node = captured.node_at(root.site)?;
        // getAdjustedLocation reattaches a selected parameter modifier to the
        // declaration's Name. The member/parameter symbol question stays with
        // the explicit selection role, rather than changing this meaning rule.
        if role == ReferenceTokenRole::ParameterPropertyModifier {
            let declaration = captured.parents[node.index()].ok_or_else(|| {
                super::super::internal("parameter modifier has no parent declaration")
            })?;
            node = id(captured.ast.node(declaration), "name").ok_or_else(|| {
                super::super::internal("parameter modifier declaration has no name")
            })?;
        }
        Ok(meaning_from_location(
            &captured.ast,
            &captured.parents,
            node,
        ))
    }

    pub(in crate::project::semantic) fn reference_declaration_meaning(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<ReferenceMeaning, DemandError> {
        Ok(meaning_from_declaration(
            &self.syntax(declaration.file)?.ast,
            declaration.node,
        ))
    }

    /// Native `getContainerNode`, which starts at the declaration's parent.
    /// Blocks and arrow functions deliberately do not stop this walk.
    pub(in crate::project::semantic) fn reference_container(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        use SyntaxKind::*;
        let captured = self.syntax(declaration.file)?;
        let mut parent = captured.parents[declaration.node.index()];
        while let Some(node) = parent {
            if matches!(
                captured.ast.node(node).syntax_kind,
                SourceFile
                    | MethodDeclaration
                    | MethodSignature
                    | FunctionDeclaration
                    | FunctionExpression
                    | GetAccessor
                    | SetAccessor
                    | ClassDeclaration
                    | InterfaceDeclaration
                    | EnumDeclaration
                    | ModuleDeclaration
            ) {
                return Ok(Some(DeclarationIdentity::new(declaration.file, node)));
            }
            parent = captured.parents[node.index()];
        }
        Ok(None)
    }

    /// The private-member branch of native `getSymbolScope` asks for an actual
    /// ClassDeclaration ancestor, deliberately excluding ClassExpression.
    pub(in crate::project::semantic) fn reference_private_scope(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let mut parent = captured.parents[declaration.node.index()];
        while let Some(node) = parent {
            if captured.ast.node(node).syntax_kind == SyntaxKind::ClassDeclaration {
                return Ok(Some(DeclarationIdentity::new(declaration.file, node)));
            }
            parent = captured.parents[node.index()];
        }
        Ok(None)
    }

    /// Native `getSymbolScope` private declaration test: an own private
    /// modifier or private identifier class element, not inherited modifiers.
    pub(in crate::project::semantic) fn declaration_is_private_reference_member(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        Ok(
            syntax::has_modifier(&captured.ast, node, SyntaxKind::PrivateKeyword)
                || id(node, "name").is_some_and(|name| {
                    captured.ast.node(name).syntax_kind == SyntaxKind::PrivateIdentifier
                        && captured.parents[declaration.node.index()].is_some_and(|parent| {
                            matches!(
                                captured.ast.node(parent).syntax_kind,
                                SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
                            )
                        })
                }),
        )
    }

    /// TypeScript `isObjectBindingElementWithoutPropertyName`: such symbols
    /// also refer to an object property, so their search cannot stay local.
    pub(in crate::project::semantic) fn reference_is_object_binding_without_property(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        Ok(node.syntax_kind == SyntaxKind::BindingElement
            && id(node, "propertyName").is_none()
            && id(node, "name")
                .is_some_and(|name| captured.ast.node(name).syntax_kind == SyntaxKind::Identifier)
            && captured.parents[declaration.node.index()].is_some_and(|parent| {
                captured.ast.node(parent).syntax_kind == SyntaxKind::ObjectBindingPattern
            }))
    }

    pub(in crate::project::semantic) fn reference_source_file(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<DeclarationIdentity, DemandError> {
        Ok(DeclarationIdentity::new(
            declaration.file,
            self.syntax(declaration.file)?.ast.root_id(),
        ))
    }

    /// Native container bounds in absolute source offsets. The normalized
    /// product domain intentionally corrects the native cold scanner's initial
    /// relative-offset bug; later symbol and meaning checks remain native.
    pub(in crate::project::semantic) fn reference_container_span(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Span, DemandError> {
        Ok(self
            .syntax(declaration.file)?
            .ast
            .node(declaration.node)
            .span)
    }
}

fn containing_object_literal_element(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> Option<NodeId> {
    use SyntaxKind::*;
    let parent = parents[node.index()]?;
    let kind = ast.node(node).syntax_kind;
    let element = match kind {
        StringLiteral | NoSubstitutionTemplateLiteral | NumericLiteral
            if ast.node(parent).syntax_kind == ComputedPropertyName =>
        {
            parents[parent.index()]?
        }
        StringLiteral
        | NoSubstitutionTemplateLiteral
        | NumericLiteral
        | Identifier
        | JsxNamespacedName
            if id(ast.node(parent), "name") == Some(node) =>
        {
            parent
        }
        _ => return None,
    };
    let is_element = matches!(
        ast.node(element).syntax_kind,
        PropertyAssignment
            | ShorthandPropertyAssignment
            | SpreadAssignment
            | MethodDeclaration
            | GetAccessor
            | SetAccessor
            | JsxAttribute
            | JsxSpreadAttribute
    );
    (is_element
        && parents[element.index()].is_some_and(|container| {
            matches!(
                ast.node(container).syntax_kind,
                ObjectLiteralExpression | JsxAttributes
            )
        }))
    .then_some(element)
}

/// TypeScript `getMeaningFromDeclaration`; these are language-service lookup
/// meanings, not the declaration-space flags that determine binder collisions.
fn meaning_from_declaration(ast: &Ast<'_>, node: NodeId) -> ReferenceMeaning {
    use ReferenceMeaning as M;
    use SyntaxKind::*;
    match ast.node(node).syntax_kind {
        VariableDeclaration
        | Parameter
        | BindingElement
        | PropertyDeclaration
        | PropertySignature
        | PropertyAssignment
        | ShorthandPropertyAssignment
        | MethodDeclaration
        | MethodSignature
        | Constructor
        | GetAccessor
        | SetAccessor
        | FunctionDeclaration
        | FunctionExpression
        | ArrowFunction
        | CatchClause
        | JsxAttribute => M::VALUE,
        TypeParameter | InterfaceDeclaration | TypeAliasDeclaration | TypeLiteral => M::TYPE,
        EnumMember | ClassDeclaration => M::VALUE.union(M::TYPE),
        ModuleDeclaration => {
            if syntax::is_ambient_module(ast, node)
                || module_instance_state(ast, node) == ModuleInstanceState::Instantiated
            {
                M::NAMESPACE.union(M::VALUE)
            } else {
                M::NAMESPACE
            }
        }
        SourceFile => M::NAMESPACE.union(M::VALUE),
        _ => M::ALL,
    }
}

/// TypeScript `getMeaningFromLocation` after getAdjustedLocation. JSDoc name
/// references are outside the admitted syntax profile, so no successful AST
/// can enter that compiler-only branch here.
fn meaning_from_location(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> ReferenceMeaning {
    use ReferenceMeaning as M;
    use SyntaxKind::*;
    let current = ast.node(node);
    if current.syntax_kind == SourceFile {
        return M::VALUE;
    }
    let Some(parent_id) = parents[node.index()] else {
        return M::VALUE;
    };
    let parent = ast.node(parent_id);
    if matches!(
        parent.syntax_kind,
        ExportAssignment
            | ExportSpecifier
            | ExternalModuleReference
            | ImportSpecifier
            | ImportClause
    ) || parent.syntax_kind == ImportEqualsDeclaration && id(parent, "name") == Some(node)
    {
        return M::ALL;
    }
    if in_internal_import_equals(ast, parents, node) {
        let name = if current.syntax_kind == QualifiedName {
            Some(node)
        } else if parent.syntax_kind == QualifiedName && id(parent, "right") == Some(node) {
            Some(parent_id)
        } else {
            None
        };
        return if name
            .and_then(|name| parents[name.index()])
            .is_some_and(|owner| ast.node(owner).syntax_kind == ImportEqualsDeclaration)
        {
            M::ALL
        } else {
            M::NAMESPACE
        };
    }
    if syntax::is_declaration_name(ast, |id| parents[id.index()], node) {
        return meaning_from_declaration(ast, parent_id);
    }
    if is_type_reference(ast, parents, node) {
        return M::TYPE;
    }
    if is_namespace_reference(ast, parents, node) {
        return M::NAMESPACE;
    }
    match parent.syntax_kind {
        TypeParameter => M::TYPE,
        LiteralType => M::TYPE.union(M::VALUE),
        _ => M::VALUE,
    }
}

/// TypeScript `isInRightSideOfInternalImportEqualsDeclaration`.
fn in_internal_import_equals(ast: &Ast<'_>, parents: &[Option<NodeId>], mut node: NodeId) -> bool {
    while let Some(parent) = parents[node.index()] {
        let owner = ast.node(parent);
        if owner.syntax_kind != SyntaxKind::QualifiedName {
            return owner.syntax_kind == SyntaxKind::ImportEqualsDeclaration
                && id(owner, "moduleReference") == Some(node)
                && ast.node(node).syntax_kind != SyntaxKind::ExternalModuleReference;
        }
        node = parent;
    }
    false
}

/// Native language service `isTypeReference`, distinct from IsPartOfTypeNode.
fn is_type_reference(ast: &Ast<'_>, parents: &[Option<NodeId>], mut node: NodeId) -> bool {
    use SyntaxKind::*;
    if let Some(parent) = parents[node.index()] {
        let owner = ast.node(parent);
        if owner.syntax_kind == QualifiedName && id(owner, "right") == Some(node)
            || owner.syntax_kind == PropertyAccessExpression && id(owner, "name") == Some(node)
        {
            node = parent;
        }
    }
    match ast.node(node).syntax_kind {
        ThisKeyword => return !syntax::is_expression_node(ast, parents, node),
        ThisType => return true,
        _ => {}
    }
    parents[node.index()].is_some_and(|parent| match ast.node(parent).syntax_kind {
        TypeReference => true,
        ImportType => !syntax::flag(ast.node(parent), "isTypeOf"),
        ExpressionWithTypeArguments => syntax::is_part_of_type_node(ast, parents, parent),
        _ => false,
    })
}

/// Native `isNamespaceReference`, comprising its qualified-name and property-
/// access variants. Only nonfinal namespace segments satisfy these questions.
fn is_namespace_reference(ast: &Ast<'_>, parents: &[Option<NodeId>], node: NodeId) -> bool {
    use SyntaxKind::*;
    let Some(parent) = parents[node.index()] else {
        return false;
    };
    let kind = ast.node(parent).syntax_kind;
    if !matches!(kind, QualifiedName | PropertyAccessExpression) {
        return false;
    }
    let mut root = parent;
    while let Some(ancestor) = parents[root.index()] {
        if ast.node(ancestor).syntax_kind != kind {
            break;
        }
        root = ancestor;
    }
    if id(
        ast.node(root),
        if kind == QualifiedName {
            "right"
        } else {
            "name"
        },
    ) == Some(node)
    {
        return false;
    }
    let Some(owner) = parents[root.index()] else {
        return false;
    };
    if kind == QualifiedName {
        return ast.node(owner).syntax_kind == TypeReference;
    }
    if ast.node(owner).syntax_kind != ExpressionWithTypeArguments {
        return false;
    }
    let Some(heritage) = parents[owner.index()] else {
        return false;
    };
    if ast.node(heritage).syntax_kind != HeritageClause {
        return false;
    }
    let Some(declaration) = parents[heritage.index()] else {
        return false;
    };
    let token = syntax::number(ast.node(heritage), "token");
    ast.node(declaration).syntax_kind == ClassDeclaration && token == Some(ImplementsKeyword as u32)
        || ast.node(declaration).syntax_kind == InterfaceDeclaration
            && token == Some(ExtendsKeyword as u32)
}
