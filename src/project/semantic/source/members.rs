//! Recipes acquired from actual canonical own member declarations.

use super::{
    CapturedSyntax, SignatureRecipe, SignatureRoot, Source, TypeExpression, signature_recipe,
    signature_root, type_expression,
};
use crate::analysis::{AnalysisError, SemanticFailureReason};
use crate::project::binding::{
    BoundDeclaration, BoundSymbol, NativeDeclaration, NativePropertyParent,
};
use crate::project::semantic::model::DemandError;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;

pub(in crate::project::semantic) struct AccessorDeclaration {
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) annotation: Option<TypeExpression>,
    pub(in crate::project::semantic) body: Option<SignatureRoot>,
}

pub(in crate::project::semantic) struct AccessorRecipe {
    pub(in crate::project::semantic) getter: Option<AccessorDeclaration>,
    pub(in crate::project::semantic) setter: Option<AccessorDeclaration>,
    pub(in crate::project::semantic) auto: Option<AccessorDeclaration>,
    // getWriteTypeOfAccessors asks the first PropertyDeclaration, then tests
    // auto-accessor; the read owner instead finds the first auto-accessor.
    pub(in crate::project::semantic) write_auto: Option<AccessorDeclaration>,
}

impl Source<'_, '_> {
    /// An attached DeclarationIdentity already has a final AST in this Source.
    /// Header-only lookup gates may inspect it without parsing or changing
    /// checker state through an otherwise read-only facts view.
    pub(in crate::project::semantic) fn observed_declaration_site(
        &self,
        declaration: DeclarationIdentity,
    ) -> Result<(SyntaxKind, crate::Span), DemandError> {
        let captured = self
            .syntax
            .get(&declaration.file)
            .ok_or_else(|| super::super::internal("attached declaration has no captured syntax"))?;
        let node = captured.ast.node(declaration.node);
        Ok((node.syntax_kind, node.span))
    }

    pub(in crate::project::semantic) fn observed_declaration_has_modifier(
        &self,
        declaration: DeclarationIdentity,
        modifier: SyntaxKind,
    ) -> Result<bool, DemandError> {
        let captured = self
            .syntax
            .get(&declaration.file)
            .ok_or_else(|| super::super::internal("attached declaration has no captured syntax"))?;
        Ok(syntax::has_modifier(
            &captured.ast,
            captured.ast.node(declaration.node),
            modifier,
        ))
    }

    /// The source location of an attached declaration is observed read-only;
    /// attachment already captured the AST and the shared anchor owner applies.
    pub(in crate::project::semantic) fn observed_declaration_location(
        &self,
        declaration: DeclarationIdentity,
    ) -> Result<crate::references::Location, DemandError> {
        let captured = self
            .syntax
            .get(&declaration.file)
            .ok_or_else(|| super::super::internal("attached declaration has no captured syntax"))?;
        let anchor = captured.declaration_anchor(declaration.node);
        Ok(super::super::location(
            self.inputs,
            declaration.file,
            anchor,
        ))
    }

    /// `isShorthandAmbientModuleSymbol` observes only an already attached actual ModuleDeclaration body.
    pub(in crate::project::semantic) fn observed_declaration_is_shorthand(
        &self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self
            .syntax
            .get(&declaration.file)
            .ok_or_else(|| super::super::internal("attached declaration has no captured syntax"))?;
        let node = captured.ast.node(declaration.node);
        Ok(node.syntax_kind == SyntaxKind::ModuleDeclaration && syntax::id(node, "body").is_none())
    }

    /// `getMembersOfDeclaration` enumerates actual owner syntax in source
    /// order. Name eligibility and static partitioning belong to the checker.
    pub(in crate::project::semantic) fn members_of_declaration(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Vec<DeclarationIdentity>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        let field = match node.syntax_kind {
            SyntaxKind::InterfaceDeclaration
            | SyntaxKind::ClassDeclaration
            | SyntaxKind::ClassExpression
            | SyntaxKind::TypeLiteral => "members",
            SyntaxKind::ObjectLiteralExpression => "properties",
            _ => return Ok(Vec::new()),
        };
        Ok(syntax::ids(node, field)
            .iter()
            .map(|&node| DeclarationIdentity {
                file: declaration.file,
                node,
            })
            .collect())
    }
}

pub(in crate::project::semantic) enum OuterParameterRecipe {
    Parameter(DeclarationIdentity),
    This(DeclarationIdentity),
    Conditional(super::ExpressionRoot),
    Contextual {
        declaration: DeclarationIdentity,
        root: super::ExpressionRoot,
        parameters: Vec<DeclarationIdentity>,
    },
}

#[derive(Clone, Copy)]
pub(in crate::project::semantic) enum DeclarationReference {
    Native(crate::project::binding::NativeSymbolDeclaration),
    Bound(BoundDeclaration),
    Captured(DeclarationIdentity),
}

impl From<BoundDeclaration> for DeclarationReference {
    fn from(declaration: BoundDeclaration) -> Self {
        Self::Bound(declaration)
    }
}

impl From<DeclarationIdentity> for DeclarationReference {
    fn from(declaration: DeclarationIdentity) -> Self {
        Self::Captured(declaration)
    }
}

pub(in crate::project::semantic) enum ParameterReferenceSyntax {
    Known(bool),
    Reference(Box<TypeExpression>),
    Query {
        first: super::ExpressionRoot,
        arguments: Vec<DeclarationIdentity>,
    },
    Children(Vec<DeclarationIdentity>),
}

enum SymbolDeclarations<'a> {
    Captured(&'a [DeclarationIdentity]),
}

/// Syntax facts consumed by native property access. Opaque declarations
/// preserve actual class identity without exposing parser-node storage.
pub(in crate::project::semantic) struct AccessDeclarationContext {
    pub(in crate::project::semantic) readonly_constructor_write: bool,
    pub(in crate::project::semantic) assume_uninitialized: bool,
}

impl Source<'_, '_> {
    /// `ast.IsImportCall` on the actual originating-import node.
    pub(in crate::project::semantic) fn declaration_is_import_call(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(crate::syntax::is_import_call(
            &captured.ast,
            declaration.node,
        ))
    }

    /// `IsPrivateIdentifierClassElementDeclaration` observes the declaration's
    /// own Name, not its modifier flags or a merged symbol's name.
    pub(in crate::project::semantic) fn declaration_is_private_class_element(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        Ok(matches!(
            node.syntax_kind,
            SyntaxKind::PropertyDeclaration
                | SyntaxKind::MethodDeclaration
                | SyntaxKind::GetAccessor
                | SyntaxKind::SetAccessor
        ) && syntax::id(node, "name").is_some_and(|name| {
            captured.ast.node(name).syntax_kind == SyntaxKind::PrivateIdentifier
        }))
    }

    /// Native `isPrivateWithinAmbient` tests the actual member, without
    /// walking a parameter or binding element to its containing declaration.
    pub(in crate::project::semantic) fn is_private_within_ambient(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        if !self.declaration_has_modifier(declaration, SyntaxKind::PrivateKeyword)?
            && !self.declaration_is_private_class_element(declaration)?
        {
            return Ok(false);
        }
        Ok(self
            .syntax(declaration.file)?
            .ast
            .node(declaration.node)
            .flags
            & crate::compiler::node::AMBIENT
            != 0)
    }

    /// `ast.IsStatic` on the actual declaration, including static blocks.
    pub(in crate::project::semantic) fn declaration_is_static(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(syntax::is_static(&captured.ast, declaration.node))
    }

    /// `isSpreadableProperty`'s parent predicate is distinct from the private
    /// name predicate: ordinary private-modifier fields are still spreadable.
    pub(in crate::project::semantic) fn declaration_parent_is_class_like(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(
            captured.parents[declaration.node.index()].is_some_and(|parent| {
                matches!(
                    captured.ast.node(parent).syntax_kind,
                    SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
                )
            }),
        )
    }

    /// `SymbolName` / `IsPrivateIdentifierClassElementDeclaration` inspect
    /// the actual ValueDeclaration's class-element name, not accessibility.
    pub(in crate::project::semantic) fn private_identifier_declaration_name(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<String>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        if !matches!(
            node.syntax_kind,
            SyntaxKind::PropertyDeclaration
                | SyntaxKind::MethodDeclaration
                | SyntaxKind::GetAccessor
                | SyntaxKind::SetAccessor
        ) {
            return Ok(None);
        }
        let Some(name) = syntax::id(node, "name") else {
            return Ok(None);
        };
        if captured.ast.node(name).syntax_kind != SyntaxKind::PrivateIdentifier {
            return Ok(None);
        }
        captured
            .ast
            .identifier_name(name)
            .map(|name| Some(name.to_owned()))
            .ok_or_else(|| {
                super::super::internal("private identifier has no exact retained spelling").into()
            })
    }
    /// `symbolValueDeclarationIsContextSensitive` first asks IsExpression of
    /// the actual ValueDeclaration; names and declaration spans are not
    /// interchangeable with that node.
    pub(in crate::project::semantic) fn declaration_expression(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<super::ExpressionRoot>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(
            syntax::is_expression(&captured.ast, declaration.node).then(|| super::ExpressionRoot {
                file: declaration.file,
                site: crate::project::binding::flow::Site::new(&captured.ast, declaration.node),
            }),
        )
    }

    /// `isValidDeclarationForTupleLabel` retains the actual declaration node;
    /// exact label spelling is acquired lazily by the shared tuple recipe.
    pub(in crate::project::semantic) fn nameable_tuple_label(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<crate::project::semantic::model::TupleElementLabel>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        let valid = node.syntax_kind == SyntaxKind::NamedTupleMember
            || node.syntax_kind == SyntaxKind::Parameter
                && syntax::id(node, "name").is_some_and(|name| {
                    captured.ast.node(name).syntax_kind == SyntaxKind::Identifier
                });
        Ok(valid.then(|| {
            super::tuples::tuple_element_label(inputs, declaration.file, captured, declaration.node)
        }))
    }

    /// `getTupleElementLabelFromBindingElement` follows the actual rest
    /// parameter binding pattern. A non-parameter ValueDeclaration belongs
    /// to getTupleElementLabel's symbol-name fallback in the caller.
    pub(in crate::project::semantic) fn tuple_element_label_from_binding_element(
        &mut self,
        declaration: DeclarationIdentity,
        index: usize,
        flags: crate::project::semantic::model::TupleElementFlags,
    ) -> Result<Option<std::sync::Arc<str>>, DemandError> {
        fn label(
            inputs: crate::project::semantic::inputs::CheckerInputs<'_>,
            file: crate::project::sources::FileId,
            ast: &crate::Ast<'_>,
            id: crate::NodeId,
            index: usize,
            flags: crate::project::semantic::model::TupleElementFlags,
        ) -> Result<std::sync::Arc<str>, DemandError> {
            use crate::project::semantic::model::TupleElementFlags as Flags;
            let node = ast.node(id);
            if let Some(name) = syntax::id(node, "name") {
                let rest = syntax::id(node, "dotDotDotToken").is_some();
                match ast.node(name).syntax_kind {
                    SyntaxKind::Identifier => {
                        let name = ast.identifier_name(name).ok_or_else(|| {
                            super::super::unsupported_demand_at_location(
                                inputs,
                                &super::super::location(inputs, file, ast.token_span(name)),
                                SemanticFailureReason::UnrepresentableIdentity,
                                "tuple binding label has no exact scalar identifier name",
                            )
                        })?;
                        return Ok(if rest {
                            if flags.intersects(Flags::VARIABLE) {
                                name.into()
                            } else {
                                format!("{name}_{index}").into()
                            }
                        } else if flags.intersects(Flags::FIXED) {
                            name.into()
                        } else {
                            format!("{name}_n").into()
                        });
                    }
                    SyntaxKind::ArrayBindingPattern if rest => {
                        let elements = syntax::ids(ast.node(name), "elements");
                        let last_rest = elements.last().copied().filter(|&id| {
                            let node = ast.node(id);
                            node.syntax_kind == SyntaxKind::BindingElement
                                && syntax::id(node, "dotDotDotToken").is_some()
                        });
                        let count = elements.len() - usize::from(last_rest.is_some());
                        if index < count {
                            let element = elements[index];
                            if ast.node(element).syntax_kind == SyntaxKind::BindingElement {
                                return label(inputs, file, ast, element, index, flags);
                            }
                        } else if let Some(last) = last_rest {
                            return label(inputs, file, ast, last, index - count, flags);
                        }
                    }
                    _ => {}
                }
            }
            Ok(format!("arg_{index}").into())
        }
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        if captured.ast.node(declaration.node).syntax_kind != SyntaxKind::Parameter {
            return Ok(None);
        }
        label(
            inputs,
            declaration.file,
            &captured.ast,
            declaration.node,
            index,
            flags,
        )
        .map(Some)
    }

    pub(in crate::project::semantic) fn resolve_declaration_reference(
        &mut self,
        declaration: DeclarationReference,
    ) -> Result<DeclarationIdentity, DemandError> {
        match declaration {
            DeclarationReference::Bound(declaration) => self.declaration_identity(declaration),
            DeclarationReference::Captured(declaration) => Ok(declaration),
            DeclarationReference::Native(declaration) => {
                self.reference_declaration_identity(declaration)
            }
        }
    }

    fn declaration_reference_location(
        &mut self,
        declaration: DeclarationReference,
    ) -> Result<crate::references::Location, DemandError> {
        match declaration {
            DeclarationReference::Bound(declaration) => Ok(super::super::location(
                self.inputs,
                declaration.file,
                declaration.anchor,
            )),
            DeclarationReference::Captured(declaration) => self.declaration_location(declaration),
            DeclarationReference::Native(declaration) => {
                let declaration = self.reference_declaration_identity(declaration)?;
                self.declaration_location(declaration)
            }
        }
    }

    /// Resolve a retained `Symbol.Declarations` node through its full native
    /// kind/span association. The diagnostic anchor is not declaration identity.
    pub(in crate::project::semantic) fn declaration_identity(
        &mut self,
        declaration: BoundDeclaration,
    ) -> Result<DeclarationIdentity, DemandError> {
        let native = self
            .bindings()
            .native_bound_declaration(declaration)
            .map_err(|error| {
                super::super::binding_failure_at(
                    self.inputs,
                    &super::super::location(self.inputs, declaration.file, declaration.anchor),
                    error,
                )
            })?;
        self.reference_declaration_identity(native)
    }

    /// `getObjectTypeInstantiation` consumes full Symbol.Declarations, including
    /// nonmember entries. Validate the complete native array before projecting
    /// it, so syntax projection never interleaves with fact validation.
    pub(in crate::project::semantic) fn symbol_declaration_references(
        &self,
        symbol: BoundSymbol,
        consumer: &crate::references::Location,
    ) -> Result<Vec<DeclarationReference>, DemandError> {
        self.bindings()
            .reference_symbol_declarations(crate::project::binding::ReferenceSymbol::Known(
                crate::project::binding::ReadOrigin::Direct(symbol),
            ))
            .map(|declarations| {
                (declarations.into_iter().map(DeclarationReference::Native)).collect::<Vec<_>>()
            })
            .map_err(|error| super::super::binding_failure_at(self.inputs, consumer, error))
    }

    /// Native `IsAmbientModule`, independent from declaration ambient flags.
    pub(in crate::project::semantic) fn declaration_is_ambient_module(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(declaration.file)?;
        Ok(crate::syntax::is_ambient_module(
            &captured.ast,
            declaration.node,
        ))
    }

    pub(in crate::project::semantic) fn declaration_kind(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<SyntaxKind, DemandError> {
        Ok(self
            .syntax(declaration.file)?
            .ast
            .node(declaration.node)
            .syntax_kind)
    }

    pub(in crate::project::semantic) fn declaration_modifiers(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<u32, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(syntax::combined_modifier_flags(
            &captured.ast,
            &captured.parents,
            declaration.node,
        ))
    }

    /// Observe this declaration's own ModifierNodes, rather than inherited
    /// flags from GetCombinedModifierFlags (the auto-accessor printer gate).
    pub(in crate::project::semantic) fn declaration_has_modifier(
        &mut self,
        declaration: DeclarationIdentity,
        modifier: SyntaxKind,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(syntax::has_modifier(
            &captured.ast,
            captured.ast.node(declaration.node),
            modifier,
        ))
    }

    pub(in crate::project::semantic) fn declaration_location(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<crate::references::Location, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let anchor = captured.declaration_anchor(declaration.node);
        Ok(super::super::location(inputs, declaration.file, anchor))
    }

    pub(in crate::project::semantic) fn declaration_ordering(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<NativeDeclaration, DemandError> {
        Ok(NativeDeclaration {
            file: declaration.file,
            span: self
                .syntax(declaration.file)?
                .ast
                .node(declaration.node)
                .span,
        })
    }

    pub(in crate::project::semantic) fn declaration_symbol_parent(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<NativePropertyParent>, DemandError> {
        let bindings = self.bindings();
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let kind = captured.ast.node(declaration.node).syntax_kind;
        let anchor = captured.declaration_anchor(declaration.node);
        let location = super::super::location(inputs, declaration.file, anchor);
        let bound = bindings
            .native_declaration(declaration.file, kind, anchor)
            .map_err(|error| super::super::binding_failure_at(inputs, &location, error))?;
        bindings
            .declaration_symbol_parent(bound)
            .map_err(|error| super::super::binding_failure_at(inputs, &location, error))
    }

    /// The declaration-backed and checker-created symbol paths share the
    /// complete getSignaturesOfSymbol ordering and implementation exclusion.
    pub(in crate::project::semantic) fn signatures_from_declarations(
        &mut self,
        declarations: impl Iterator<Item = DeclarationReference>,
    ) -> Result<Vec<SignatureRecipe>, DemandError> {
        let inputs = self.inputs;
        let mut signatures = Vec::new();
        let mut previous = None;
        for declaration in declarations {
            let declaration = self.resolve_declaration_reference(declaration)?;
            let captured = self.syntax(declaration.file)?;
            let id = declaration.node;
            let kind = captured.ast.node(id).syntax_kind;
            let CapturedSyntax { ast, parents, .. } = captured;
            let node = ast.node(id);
            let implementation = syntax::id(node, "body").is_some()
                && previous.is_some_and(|(file, parent, kind, end)| {
                    file == declaration.file
                        && parent == parents[id.index()]
                        && kind == node.syntax_kind
                        && end == node.span.start
                });
            previous = Some((
                declaration.file,
                parents[id.index()],
                node.syntax_kind,
                node.span.end,
            ));
            if !syntax::is_function_like_kind(kind) || implementation {
                continue;
            }
            signatures.push(
                signature_recipe(inputs, declaration.file, captured, parents, id).map_err(
                    |message| {
                        super::super::unsupported_demand_at_location(
                            inputs,
                            &super::super::location(
                                inputs,
                                declaration.file,
                                captured.declaration_anchor(id),
                            ),
                            SemanticFailureReason::UnsupportedSemanticForm,
                            message,
                        )
                    },
                )?,
            );
        }
        Ok(signatures)
    }

    /// Acquire the existing signature recipe for the actual declaration node;
    /// choosing the first getter/setter remains the symbol caller's question.
    pub(in crate::project::semantic) fn signature_from_declaration(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<SignatureRecipe, DemandError> {
        let inputs = self.inputs;
        let location = self.declaration_location(declaration)?;
        let captured = self.syntax(declaration.file)?;
        signature_recipe(
            inputs,
            declaration.file,
            captured,
            &captured.parents,
            declaration.node,
        )
        .map_err(|message| {
            super::super::unsupported_demand_at_location(
                inputs,
                &location,
                SemanticFailureReason::UnsupportedSemanticForm,
                message,
            )
        })
    }

    /// `GetNameOfDeclaration` for an actual property ValueDeclaration. The
    /// caller acquires this only after cached nameType/default-name branches.
    pub(in crate::project::semantic) fn declaration_name_site(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<
        Option<(
            crate::project::sources::FileId,
            crate::project::binding::flow::Site,
        )>,
        DemandError,
    > {
        let captured = self.syntax(declaration.file)?;
        Ok(
            syntax::id(captured.ast.node(declaration.node), "name").map(|name| {
                (
                    declaration.file,
                    crate::project::binding::flow::Site::new(&captured.ast, name),
                )
            }),
        )
    }
    /// `isAssignmentToReadonlyEntity` and `getFlowTypeOfAccessExpression`
    /// consume actual declaration/access ancestry. Before-use ordering has
    /// its own complete operation and does not borrow these syntax facts.
    pub(in crate::project::semantic) fn property_access_context(
        &mut self,
        declaration: DeclarationIdentity,
        access: super::ExpressionRoot,
    ) -> Result<AccessDeclarationContext, DemandError> {
        let options = self.inputs.semantic_options();
        let strict_initialization =
            options.strict_null_checks && options.strict_property_initialization;
        let (parent, static_, without_initializer, ambient) = {
            let captured = self.syntax(declaration.file)?;
            let ast = &captured.ast;
            let node = ast.node(declaration.node);
            let parent = captured.parents[declaration.node.index()];
            let without_initializer =
                syntax::is_property_without_initializer(ast, declaration.node);
            (
                parent.map(|node| DeclarationIdentity {
                    file: declaration.file,
                    node,
                }),
                syntax::is_static(ast, declaration.node),
                without_initializer,
                node.flags & crate::compiler::node::AMBIENT != 0,
            )
        };
        let captured = self.syntax(access.file)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let access_id = captured.node_at(access.site)?;
        let this_access = syntax::is_this_property(ast, access_id);
        let control = syntax::control_flow_container(ast, parents, access_id);
        let constructor = control.filter(|&id| ast.node(id).syntax_kind == SyntaxKind::Constructor);
        let same_class_constructor = constructor.is_some_and(|id| {
            parents[id.index()].map(|node| DeclarationIdentity {
                file: access.file,
                node,
            }) == parent
        });
        let readonly_constructor_write = this_access
            && constructor.is_some_and(|id| {
                same_class_constructor
                    || Some(DeclarationIdentity {
                        file: access.file,
                        node: id,
                    }) == parent
            });
        let assume_uninitialized = strict_initialization
            && this_access
            && without_initializer
            && !static_
            && !ambient
            && same_class_constructor;
        Ok(AccessDeclarationContext {
            readonly_constructor_write,
            assume_uninitialized,
        })
    }

    pub(in crate::project::semantic) fn symbol_access_auto_type(
        &mut self,
        value: Option<DeclarationIdentity>,
        declarations: &[DeclarationIdentity],
        access: super::ExpressionRoot,
        consumer: &crate::references::Location,
    ) -> Result<bool, DemandError> {
        self.access_auto_type(
            value,
            SymbolDeclarations::Captured(declarations),
            access,
            consumer,
        )
    }

    fn access_auto_type(
        &mut self,
        value: Option<DeclarationIdentity>,
        declarations: SymbolDeclarations<'_>,
        access: super::ExpressionRoot,
        consumer: &crate::references::Location,
    ) -> Result<bool, DemandError> {
        let no_implicit_any = self.inputs.semantic_options().no_implicit_any;
        let auto_typed = if let Some(declaration) = value {
            let captured = self.syntax(declaration.file)?;
            let node = captured.ast.node(declaration.node);
            // The retained member factories currently establish the native
            // isConstructorDeclaredThisProperty early non-assignment branch.
            if node.syntax_kind == SyntaxKind::BinaryExpression {
                return Err(super::super::unsupported_demand_at_location(
                    self.inputs,
                    consumer,
                    SemanticFailureReason::UnsupportedSemanticForm,
                    "native constructor-assignment property metadata is pending",
                ));
            }
            auto_typed_property(&captured.ast, declaration.node, no_implicit_any)
        } else {
            false
        };
        let this_access = {
            let captured = self.syntax(access.file)?;
            let id = captured.node_at(access.site)?;
            syntax::is_this_property(&captured.ast, id)
        };
        let mut constructor = None;
        if this_access && auto_typed {
            let declarations: Vec<_> = match declarations {
                SymbolDeclarations::Captured(declarations) => declarations
                    .iter()
                    .copied()
                    .map(DeclarationReference::Captured)
                    .collect(),
            };
            for declaration in declarations {
                let declaration = match declaration {
                    DeclarationReference::Bound(declaration) => {
                        self.declaration_identity(declaration)?
                    }
                    DeclarationReference::Captured(declaration) => declaration,
                    DeclarationReference::Native(declaration) => {
                        self.reference_declaration_identity(declaration)?
                    }
                };
                let captured = self.syntax(declaration.file)?;
                let container = syntax::this_container(
                    &captured.ast,
                    &captured.parents,
                    declaration.node,
                    false,
                    false,
                );
                if let Some(container) = container.filter(|&container| {
                    captured.ast.node(container).syntax_kind == SyntaxKind::Constructor
                }) {
                    constructor = Some(DeclarationIdentity {
                        file: declaration.file,
                        node: container,
                    });
                    break;
                }
            }
        }
        let captured = self.syntax(access.file)?;
        let id = captured.node_at(access.site)?;
        let access_container =
            syntax::this_container(&captured.ast, &captured.parents, id, true, false).map(|node| {
                DeclarationIdentity {
                    file: access.file,
                    node,
                }
            });
        Ok(access_container == constructor)
    }

    fn modifier_declaration_kind(
        &mut self,
        declaration: DeclarationReference,
    ) -> Result<SyntaxKind, DemandError> {
        match declaration {
            DeclarationReference::Bound(declaration) => {
                Ok(self.inputs.bindings.declaration_syntax_kind(declaration))
            }
            DeclarationReference::Captured(declaration) => self.declaration_kind(declaration),
            DeclarationReference::Native(declaration) => Ok(declaration.kind),
        }
    }

    pub(in crate::project::semantic) fn declarations_are_thisless(
        &mut self,
        declarations: &[DeclarationIdentity],
    ) -> Result<bool, DemandError> {
        let [declaration] = declarations else {
            return Ok(false);
        };
        let kind = self.declaration_kind(*declaration)?;
        if !thisless_declaration_kind(kind) {
            return Ok(false);
        }
        let captured = self.syntax(declaration.file)?;
        let id = declaration.node;
        let ast = &captured.ast;
        Ok(
            if matches!(
                kind,
                SyntaxKind::Parameter
                    | SyntaxKind::PropertyDeclaration
                    | SyntaxKind::PropertySignature
            ) {
                is_thisless_variable(ast, id)
            } else {
                let node = ast.node(id);
                (kind == SyntaxKind::Constructor
                    || syntax::id(node, "type").is_some_and(|ty| is_thisless_type(ast, ty)))
                    && syntax::ids(node, "parameters")
                        .iter()
                        .all(|&parameter| is_thisless_variable(ast, parameter))
                    && syntax::ids(node, "typeParameters")
                        .iter()
                        .all(|&parameter| {
                            syntax::id(ast.node(parameter), "constraint")
                                .is_none_or(|ty| is_thisless_type(ast, ty))
                        })
            },
        )
    }

    /// `getAnnotatedAccessorTypeNode` / `getEffectiveSetAccessorTypeAnnotationNode`:
    /// project the first own getter/setter/auto-accessor separately. Their
    /// annotation and body dependencies remain unconsumed source recipes.
    pub(in crate::project::semantic) fn accessor_recipe(
        &mut self,
        declarations: &[DeclarationReference],
    ) -> Result<AccessorRecipe, DemandError> {
        let mut getter = None;
        let mut setter = None;
        let mut auto = None;
        let mut first_property = None;
        // GetDeclarationOfKind and Find(IsAutoAccessorPropertyDeclaration)
        // inspect the complete native list, independently of member grouping.
        for &declaration in declarations {
            if first_property.is_none()
                && self.modifier_declaration_kind(declaration)? == SyntaxKind::PropertyDeclaration
            {
                first_property = Some(declaration);
            }
            match self.modifier_declaration_kind(declaration)? {
                SyntaxKind::GetAccessor if getter.is_none() => getter = Some(declaration),
                SyntaxKind::SetAccessor if setter.is_none() => setter = Some(declaration),
                SyntaxKind::PropertyDeclaration if auto.is_none() => {
                    let own = self.resolve_declaration_reference(declaration)?;
                    if self.declaration_has_modifier(own, SyntaxKind::AccessorKeyword)? {
                        auto = Some(declaration);
                    }
                }
                _ => {}
            }
        }
        let write_auto = match first_property {
            Some(own) => {
                let declaration = self.resolve_declaration_reference(own)?;
                if self.declaration_has_modifier(declaration, SyntaxKind::AccessorKeyword)? {
                    Some(own)
                } else {
                    None
                }
            }
            None => None,
        };
        Ok(AccessorRecipe {
            getter: getter
                .map(|own| self.accessor_declaration(own))
                .transpose()?,
            setter: setter
                .map(|own| self.accessor_declaration(own))
                .transpose()?,
            auto: auto.map(|own| self.accessor_declaration(own)).transpose()?,
            write_auto: write_auto
                .map(|own| self.accessor_declaration(own))
                .transpose()?,
        })
    }

    fn accessor_declaration(
        &mut self,
        declaration: DeclarationReference,
    ) -> Result<AccessorDeclaration, DemandError> {
        let inputs = self.inputs;
        let location = self.declaration_reference_location(declaration)?;
        let declaration = self.resolve_declaration_reference(declaration)?;
        let captured = self.syntax(declaration.file)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let id = declaration.node;
        let node = ast.node(id);
        let kind = node.syntax_kind;
        let annotation = if kind == SyntaxKind::SetAccessor {
            syntax::set_accessor_value_parameter(ast, id)
                .and_then(|parameter| syntax::id(ast.node(parameter), "type"))
        } else {
            syntax::id(node, "type")
        }
        .map(|annotation| type_expression(inputs, declaration.file, captured, parents, annotation));
        let body = if kind == SyntaxKind::GetAccessor && syntax::id(node, "body").is_some() {
            Some(
                signature_root(ast, declaration.file, id).map_err(|message| {
                    super::super::unsupported_demand_at_location(
                        inputs,
                        &location,
                        SemanticFailureReason::UnsupportedSemanticForm,
                        message,
                    )
                })?,
            )
        } else {
            None
        };
        Ok(AccessorDeclaration {
            declaration,
            annotation,
            body,
        })
    }

    /// getOuterTypeParameters walks actual enclosing nodes outermost first.
    /// Declaration lists and synthetic this are ordered recipes: unavailable
    /// contextual families fail only when evaluation reaches that frame.
    pub(in crate::project::semantic) fn outer_type_parameters(
        &mut self,
        declaration: DeclarationReference,
        include_this_types: bool,
    ) -> Result<Vec<OuterParameterRecipe>, DemandError> {
        let declaration = self.resolve_declaration_reference(declaration)?;
        let captured = self.syntax(declaration.file)?;
        let id = declaration.node;
        let ast = &captured.ast;
        let mut frames = Vec::new();
        let mut parent = captured.parents[id.index()];
        while let Some(id) = parent {
            let node = ast.node(id);
            let kind = node.syntax_kind;
            if matches!(
                kind,
                SyntaxKind::ClassDeclaration
                    | SyntaxKind::ClassExpression
                    | SyntaxKind::InterfaceDeclaration
                    | SyntaxKind::CallSignature
                    | SyntaxKind::ConstructSignature
                    | SyntaxKind::MethodSignature
                    | SyntaxKind::FunctionType
                    | SyntaxKind::ConstructorType
                    | SyntaxKind::FunctionDeclaration
                    | SyntaxKind::MethodDeclaration
                    | SyntaxKind::FunctionExpression
                    | SyntaxKind::ArrowFunction
                    | SyntaxKind::TypeAliasDeclaration
                    | SyntaxKind::MappedType
                    | SyntaxKind::ConditionalType
            ) {
                let mut frame = Vec::new();
                if kind == SyntaxKind::ConditionalType {
                    frame.push(OuterParameterRecipe::Conditional(super::ExpressionRoot {
                        file: declaration.file,
                        site: crate::project::binding::flow::Site::new(ast, id),
                    }));
                } else if kind == SyntaxKind::MappedType {
                    let parameter = syntax::id(node, "typeParameter")
                        .ok_or_else(|| super::super::internal("mapped node has no parameter"))?;
                    frame.push(OuterParameterRecipe::Parameter(DeclarationIdentity::new(
                        declaration.file,
                        parameter,
                    )));
                } else if matches!(
                    kind,
                    SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction
                ) || kind == SyntaxKind::MethodDeclaration
                    && captured.parents[id.index()].is_some_and(|parent| {
                        ast.node(parent).syntax_kind == SyntaxKind::ObjectLiteralExpression
                    })
                {
                    frame.push(OuterParameterRecipe::Contextual {
                        declaration: DeclarationIdentity::new(declaration.file, id),
                        root: super::ExpressionRoot {
                            file: declaration.file,
                            site: crate::project::binding::flow::Site::new(ast, id),
                        },
                        parameters: syntax::ids(node, "typeParameters")
                            .iter()
                            .map(|&parameter| DeclarationIdentity::new(declaration.file, parameter))
                            .collect(),
                    });
                } else {
                    for &parameter in syntax::ids(node, "typeParameters") {
                        frame.push(OuterParameterRecipe::Parameter(DeclarationIdentity::new(
                            declaration.file,
                            parameter,
                        )));
                    }
                    if include_this_types
                        && matches!(
                            kind,
                            SyntaxKind::ClassDeclaration
                                | SyntaxKind::ClassExpression
                                | SyntaxKind::InterfaceDeclaration
                        )
                    {
                        // `getOuterTypeParameters` asks getSymbolOfDeclaration
                        // of this actual class/interface node. Anonymous class
                        // symbols are not associated with their `class` token.
                        frame.push(OuterParameterRecipe::This(DeclarationIdentity::new(
                            declaration.file,
                            id,
                        )));
                    }
                }
                frames.push(frame);
            }
            parent = captured.parents[id.index()];
        }
        Ok(frames.into_iter().rev().flatten().collect())
    }

    /// Syntax-only steps of `isTypeParameterPossiblyReferenced`. The evaluator
    /// owns actual parameter identity and the two native symbol acquisitions.
    pub(in crate::project::semantic) fn parameter_reference_syntax(
        &mut self,
        declaration: DeclarationIdentity,
        is_this: bool,
    ) -> Result<ParameterReferenceSyntax, DemandError> {
        use ParameterReferenceSyntax as Step;
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        let child = |node| DeclarationIdentity {
            file: declaration.file,
            node,
        };
        Ok(match node.syntax_kind {
            SyntaxKind::ThisType => Step::Known(is_this),
            SyntaxKind::TypeReference
                if !is_this && syntax::ids(node, "typeArguments").is_empty() =>
            {
                Step::Reference(Box::new(type_expression(
                    inputs,
                    declaration.file,
                    captured,
                    &captured.parents,
                    declaration.node,
                )))
            }
            SyntaxKind::TypeQuery => {
                let name = syntax::id(node, "exprName")
                    .ok_or_else(|| super::super::internal("type query lacks expression name"))?;
                let first = syntax::first_identifier(ast, name).ok_or_else(|| {
                    super::super::internal("type query has no first entity identifier")
                })?;
                if syntax::is_this_identifier(ast, first) {
                    Step::Known(true)
                } else {
                    Step::Query {
                        first: super::ExpressionRoot {
                            file: declaration.file,
                            site: crate::project::binding::flow::Site::new(ast, first),
                        },
                        arguments: syntax::ids(node, "typeArguments")
                            .iter()
                            .copied()
                            .map(child)
                            .collect(),
                    }
                }
            }
            SyntaxKind::MethodDeclaration | SyntaxKind::MethodSignature => {
                let annotation = syntax::id(node, "type");
                if annotation.is_none() && syntax::id(node, "body").is_some() {
                    Step::Known(true)
                } else {
                    Step::Children(
                        syntax::ids(node, "typeParameters")
                            .iter()
                            .chain(syntax::ids(node, "parameters"))
                            .copied()
                            .chain(annotation)
                            .map(child)
                            .collect(),
                    )
                }
            }
            _ => Step::Children(ast.children(declaration.node).map(child).collect()),
        })
    }

    /// Ancestor stop/extends facts for `isTypeParameterPossiblyReferenced`.
    pub(in crate::project::semantic) fn parameter_reference_ancestor(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<
        (
            Option<DeclarationIdentity>,
            bool,
            Option<DeclarationIdentity>,
        ),
        DemandError,
    > {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        let child = |node| DeclarationIdentity {
            file: declaration.file,
            node,
        };
        let extends = if node.syntax_kind == SyntaxKind::ConditionalType {
            Some(child(syntax::id(node, "extendsType").ok_or_else(|| {
                super::super::internal("conditional reference analysis lacks extends")
            })?))
        } else {
            None
        };
        Ok((
            captured.parents[declaration.node.index()].map(child),
            node.syntax_kind == SyntaxKind::Block,
            extends,
        ))
    }
}

fn thisless_declaration_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Parameter
            | SyntaxKind::PropertyDeclaration
            | SyntaxKind::PropertySignature
            | SyntaxKind::MethodDeclaration
            | SyntaxKind::MethodSignature
            | SyntaxKind::Constructor
            | SyntaxKind::GetAccessor
            | SyntaxKind::SetAccessor
    )
}

pub(in crate::project::semantic) use crate::project::binding::NativeNodeIdentity as DeclarationIdentity;

/// `isThislessVariableLikeDeclaration` does not inspect an initializer's value.
fn is_thisless_variable(ast: &crate::Ast<'_>, id: crate::NodeId) -> bool {
    let node = ast.node(id);
    match syntax::id(node, "type") {
        Some(ty) => is_thisless_type(ast, ty),
        None => syntax::id(node, "initializer").is_none(),
    }
}

/// `isThislessType` deliberately accepts reference names without resolving
/// them and deliberately excludes unions, intersections and parenthesized types.
fn is_thisless_type(ast: &crate::Ast<'_>, id: crate::NodeId) -> bool {
    let node = ast.node(id);
    match node.syntax_kind {
        SyntaxKind::AnyKeyword
        | SyntaxKind::UnknownKeyword
        | SyntaxKind::StringKeyword
        | SyntaxKind::NumberKeyword
        | SyntaxKind::BigIntKeyword
        | SyntaxKind::BooleanKeyword
        | SyntaxKind::SymbolKeyword
        | SyntaxKind::ObjectKeyword
        | SyntaxKind::VoidKeyword
        | SyntaxKind::UndefinedKeyword
        | SyntaxKind::NeverKeyword
        | SyntaxKind::LiteralType => true,
        SyntaxKind::ArrayType => {
            syntax::id(node, "elementType").is_some_and(|ty| is_thisless_type(ast, ty))
        }
        SyntaxKind::TypeReference => syntax::ids(node, "typeArguments")
            .iter()
            .all(|&ty| is_thisless_type(ast, ty)),
        _ => false,
    }
}

/// Native `isAutoTypedProperty` tests the actual value declaration only.
fn auto_typed_property(
    ast: &crate::Ast<'_>,
    declaration: crate::NodeId,
    no_implicit_any: bool,
) -> bool {
    let node = ast.node(declaration);
    node.syntax_kind == SyntaxKind::PropertyDeclaration
        && syntax::id(node, "type").is_none()
        && syntax::id(node, "initializer").is_none()
        && no_implicit_any
}

impl Source<'_, '_> {
    pub(in crate::project::semantic) fn property_is_auto_typed(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let no_implicit_any = self.inputs.semantic_options().no_implicit_any;
        let captured = self.syntax(declaration.file)?;
        Ok(auto_typed_property(
            &captured.ast,
            declaration.node,
            no_implicit_any,
        ))
    }

    /// Own ModifierFlags() excludes ambient flags inherited from a container.
    pub(in crate::project::semantic) fn declaration_own_modifier_flags(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<u32, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(syntax::modifier_flags(
            &captured.ast,
            captured.ast.node(declaration.node),
        ))
    }
}
