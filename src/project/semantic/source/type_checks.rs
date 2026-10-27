//! Source-element checks are separate from acquiring a type-node value.
pub(in crate::project::semantic) mod members;
use super::super::model::TupleElementFlags;
use super::{CapturedSyntax, ExpressionRoot, Source, TypeExpression, type_expression};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::syntax;
use crate::syntax_kind::SyntaxKind as K;

pub(in crate::project::semantic) struct TupleCheckElement {
    pub(in crate::project::semantic) flags: TupleElementFlags,
    pub(in crate::project::semantic) variadic: Option<TypeExpression>,
}
pub(in crate::project::semantic) enum TypeSourceCheck {
    Visit {
        children: Vec<Site>,
        result: Option<TypeExpression>,
    },
    Tuple {
        elements: Vec<TupleCheckElement>,
        children: Vec<Site>,
        result: TypeExpression,
    },
    Reference {
        children: Vec<Site>,
        result: TypeExpression,
    },
    IndexedAccess {
        children: Vec<Site>,
        result: TypeExpression,
    },
    Signature(super::SignatureRoot),
    Predicate {
        signature: Option<super::SignatureRoot>,
        asserted_type: Option<ExpressionRoot>,
    },
    TypeLiteral {
        members: Vec<ExpressionRoot>,
        result: TypeExpression,
        declaration: super::DeclarationIdentity,
    },
    Property,
    BindingElement,
    Method,
    Accessor,
    TemplateLiteral,
    Mapped,
    TypeParameter,
    Infer,
    Enum,
    EnumMember,
    Pending,
}
impl Source<'_, '_> {
    /// `checkSourceElementWorker` type-node dispatch. Grammar-only nodes have
    /// no semantic children; unsupported semantic workers remain explicit.
    pub(in crate::project::semantic) fn type_source_check(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<TypeSourceCheck, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let node = ast.node(id);
        let sites = |nodes: &[crate::NodeId]| nodes.iter().map(|&id| Site::new(ast, id)).collect();
        let result = || type_expression(inputs, root.file, captured, parents, id);
        Ok(match node.syntax_kind {
            K::IntrinsicKeyword
            | K::AnyKeyword
            | K::UnknownKeyword
            | K::NumberKeyword
            | K::ObjectKeyword
            | K::BooleanKeyword
            | K::StringKeyword
            | K::SymbolKeyword
            | K::BigIntKeyword
            | K::VoidKeyword
            | K::UndefinedKeyword
            | K::NeverKeyword
            | K::LiteralType => TypeSourceCheck::Visit {
                children: Vec::new(),
                result: None,
            },
            K::TemplateLiteralType => TypeSourceCheck::TemplateLiteral,
            K::MappedType => TypeSourceCheck::Mapped,
            K::TypeLiteral => TypeSourceCheck::TypeLiteral {
                members: syntax::ids(node, "members")
                    .iter()
                    .map(|&member| ExpressionRoot {
                        file: root.file,
                        site: Site::new(ast, member),
                    })
                    .collect(),
                result: result(),
                declaration: super::DeclarationIdentity {
                    file: root.file,
                    node: id,
                },
            },
            K::PropertySignature => TypeSourceCheck::Property,
            K::BindingElement => TypeSourceCheck::BindingElement,
            K::OmittedExpression => TypeSourceCheck::Visit {
                children: Vec::new(),
                result: None,
            },
            K::MethodSignature => TypeSourceCheck::Method,
            K::GetAccessor | K::SetAccessor => TypeSourceCheck::Accessor,
            K::TypeParameter => TypeSourceCheck::TypeParameter,
            K::InferType => TypeSourceCheck::Infer,
            K::EnumDeclaration => TypeSourceCheck::Enum,
            K::EnumMember => TypeSourceCheck::EnumMember,
            K::ArrayType => TypeSourceCheck::Visit {
                children: sites(&[syntax::id(node, "elementType")
                    .ok_or_else(|| super::super::internal("array check lacks element"))?]),
                result: None,
            },
            K::ParenthesizedType | K::OptionalType | K::RestType | K::TypeOperator => {
                TypeSourceCheck::Visit {
                    children: sites(&[syntax::id(node, "type")
                        .ok_or_else(|| super::super::internal("type check lacks operand"))?]),
                    result: None,
                }
            }
            K::NamedTupleMember => TypeSourceCheck::Visit {
                children: sites(&[syntax::id(node, "type").ok_or_else(|| {
                    super::super::internal("named tuple member check lacks type")
                })?]),
                result: Some(result()),
            },
            K::UnionType | K::IntersectionType => TypeSourceCheck::Visit {
                children: sites(syntax::ids(node, "types")),
                result: Some(result()),
            },
            K::ThisType | K::TypeQuery => TypeSourceCheck::Visit {
                children: Vec::new(),
                result: Some(result()),
            },
            K::TypeReference => TypeSourceCheck::Reference {
                children: sites(syntax::ids(node, "typeArguments")),
                result: result(),
            },
            // checkImportType visits only its argument, never the written
            // type arguments, before checkTypeReferenceOrImport. Its
            // getResolutionModeOverride reports grammar errors only.
            K::ImportType => TypeSourceCheck::Reference {
                children: sites(&[syntax::id(node, "argument")
                    .ok_or_else(|| super::super::internal("import type check lacks argument"))?]),
                result: result(),
            },
            K::TypePredicate => {
                // `getTypePredicateParent` admits only the actual return
                // child of these seven signature declaration kinds.
                let parent = parents[id.index()].filter(|&parent| {
                    let parent = ast.node(parent);
                    matches!(
                        parent.syntax_kind,
                        K::ArrowFunction
                            | K::CallSignature
                            | K::FunctionDeclaration
                            | K::FunctionExpression
                            | K::FunctionType
                            | K::MethodDeclaration
                            | K::MethodSignature
                    ) && syntax::id(parent, "type") == Some(id)
                });
                TypeSourceCheck::Predicate {
                    signature: parent
                        .map(|parent| {
                            super::signature_root(ast, root.file, parent)
                                .map_err(super::super::internal)
                        })
                        .transpose()?,
                    asserted_type: syntax::id(node, "type").map(|node| ExpressionRoot {
                        file: root.file,
                        site: Site::new(ast, node),
                    }),
                }
            }
            K::FunctionType
            | K::ConstructorType
            | K::CallSignature
            | K::ConstructSignature
            | K::IndexSignature => TypeSourceCheck::Signature(
                super::signature_root(ast, root.file, id).map_err(super::super::internal)?,
            ),
            K::IndexedAccessType => TypeSourceCheck::IndexedAccess {
                children: sites(&[
                    syntax::id(node, "objectType")
                        .ok_or_else(|| super::super::internal("indexed check lacks object"))?,
                    syntax::id(node, "indexType")
                        .ok_or_else(|| super::super::internal("indexed check lacks index"))?,
                ]),
                result: result(),
            },
            K::ConditionalType => TypeSourceCheck::Visit {
                children: sites(&[
                    syntax::id(node, "checkType").ok_or_else(|| {
                        super::super::internal("conditional check lacks check type")
                    })?,
                    syntax::id(node, "extendsType").ok_or_else(|| {
                        super::super::internal("conditional check lacks extends type")
                    })?,
                    syntax::id(node, "trueType").ok_or_else(|| {
                        super::super::internal("conditional check lacks true type")
                    })?,
                    syntax::id(node, "falseType").ok_or_else(|| {
                        super::super::internal("conditional check lacks false type")
                    })?,
                ]),
                result: None,
            },
            K::TupleType => {
                let children = syntax::ids(node, "elements");
                let elements = children
                    .iter()
                    .map(|&element| {
                        let flags = super::tuples::element_flags(captured, element);
                        let variadic = if flags.intersects(TupleElementFlags::VARIADIC) {
                            let operand =
                                syntax::id(ast.node(element), "type").ok_or_else(|| {
                                    super::super::internal("variadic check lacks type")
                                })?;
                            Some(type_expression(
                                inputs, root.file, captured, parents, operand,
                            ))
                        } else {
                            None
                        };
                        Ok(TupleCheckElement { flags, variadic })
                    })
                    .collect::<Result<Vec<_>, AnalysisError>>()?;
                TypeSourceCheck::Tuple {
                    elements,
                    children: sites(children),
                    result: result(),
                }
            }
            _ => TypeSourceCheck::Pending,
        })
    }
}
