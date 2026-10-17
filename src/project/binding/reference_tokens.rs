//! The one captured name-bearing inventory used by reference selection.
//!
//! This is syntax capture for native `getPossibleSymbolReferencePositions`;
//! search owns scope, validity, meaning and symbol acquisition. The decoded
//! key accelerator is deliberately separate from lexical name candidates.

use super::ids::OccurrenceId;
use super::json::JsonPropertyId;
use crate::project::sources::FileId;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) struct ReferenceToken {
    pub(super) file: FileId,
    pub(super) entry: ReferenceTokenEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum ReferenceTokenEntry {
    Syntax(OccurrenceId),
    Json(JsonPropertyId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::project) enum ReferenceTokenRole {
    Name,
    MemberName,
    LiteralKey,
    BindingProperty,
    Shorthand,
    ParameterName,
    ParameterPropertyModifier,
    JsxAttribute,
    JsxNamespacedAttribute,
    PrivateName,
    ComputedValue,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::project) enum ReferenceTokenSource {
    Syntax(ReferenceSyntax),
    Json {
        property: JsonPropertyId,
        object: super::BoundJson,
    },
}

pub(in crate::project) struct ReferenceTokenFacts<'a> {
    pub(in crate::project) file: FileId,
    pub(in crate::project) key: Result<&'a str, &'a str>,
    pub(in crate::project) selection_span: Span,
    pub(in crate::project) occurrence_span: Span,
    pub(in crate::project) roles: u8,
    pub(in crate::project) call: bool,
    pub(in crate::project) traversal_header: bool,
    pub(in crate::project) source: ReferenceTokenSource,
}

impl super::Bindings {
    /// Exact decoded candidates in public path/start/end order. Search scope,
    /// filters and native eligibility are deliberately not applied here.
    pub(in crate::project) fn reference_tokens(
        &self,
        key: &str,
        control: Option<&crate::resources::Control>,
    ) -> super::index::ReferenceTokens<'_> {
        self.index.reference_tokens(key, control)
    }

    /// Select an inventoried token or explicit role anchor, without a nearest
    /// name fallback. JSON uses its existing captured key occurrence owner.
    pub(in crate::project) fn reference_token_at(
        &self,
        file: FileId,
        offset: usize,
    ) -> Option<ReferenceToken> {
        let facts = self.files.get(file.index())?;
        match &facts.json {
            Some(json) => json.reference_key_at(offset).map(|id| ReferenceToken {
                file,
                entry: ReferenceTokenEntry::Json(id),
            }),
            None => self.index.reference_token_at(file, offset),
        }
    }

    /// `addReferencesHere` is independent of syntax availability. Dependency
    /// files can lead an import traversal to exports without emitting uses.
    pub(in crate::project) fn reference_file_is_searched(&self, file: FileId) -> bool {
        self.files[file.index()].search
    }

    /// Handles are created only by this inventory and remain within its
    /// project. Missing backing data is an internal invariant violation.
    pub(in crate::project) fn reference_token_facts(
        &self,
        token: ReferenceToken,
    ) -> ReferenceTokenFacts<'_> {
        match token.entry {
            ReferenceTokenEntry::Syntax(id) => {
                let at = super::index::OccurrenceRef {
                    file: token.file,
                    id,
                };
                let occurrence = self.index.occurrence(at);
                let reference = self
                    .index
                    .reference_data(at)
                    .expect("reference handle has inventory facts");
                ReferenceTokenFacts {
                    file: token.file,
                    key: reference.key,
                    selection_span: occurrence.span,
                    occurrence_span: reference.syntax.occurrence_span,
                    roles: reference.syntax.roles,
                    call: occurrence.call,
                    traversal_header: reference.syntax.traversal_header,
                    source: ReferenceTokenSource::Syntax(reference.syntax),
                }
            }
            ReferenceTokenEntry::Json(property) => {
                let json = self
                    .files
                    .get(token.file.index())
                    .and_then(|file| file.json.as_ref())
                    .expect("JSON reference handle has captured JSON facts");
                let key = json.reference_key(property);
                ReferenceTokenFacts {
                    file: token.file,
                    key: key
                        .key
                        .as_str()
                        .ok_or("JSON reference key contains an unpaired UTF-16 surrogate"),
                    selection_span: key.span,
                    occurrence_span: crate::syntax::reference_occurrence_range(
                        SyntaxKind::StringLiteral,
                        key.span,
                        None,
                    ),
                    roles: super::collect::DECLARATION,
                    call: false,
                    traversal_header: false,
                    source: ReferenceTokenSource::Json {
                        property,
                        object: super::BoundJson {
                            file: token.file,
                            value: key.object,
                        },
                    },
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct CollectedReferenceToken {
    /// An unavailable exact key remains a failure at selection; its display
    /// text is never inserted into a key index.
    pub(super) key: Result<String, String>,
    pub(super) syntax: ReferenceSyntax,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::project) struct ReferenceSyntax {
    pub(in crate::project) kind: SyntaxKind,
    pub(in crate::project) native_span: Span,
    pub(in crate::project) occurrence_span: Span,
    pub(in crate::project) role: ReferenceTokenRole,
    pub(in crate::project) roles: u8,
    pub(in crate::project) traversal_header: bool,
}

/// Capture the decoded spelling and actual syntax role of a candidate. This
/// does not evaluate a computed key: only its written identifier/literal is
/// inventoried. `isValidReferencePosition` remains the searcher's question.
pub(super) fn capture(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> Option<CollectedReferenceToken> {
    use ReferenceTokenRole as R;
    use SyntaxKind as K;
    let node = ast.node(id);
    let parent_id = parents[id.index()]?;
    let parent = ast.node(parent_id);
    let grandparent = parents[parent_id.index()].map(|id| ast.node(id));
    if parent.syntax_kind == K::JsxNamespacedName {
        return None;
    }
    let own_name = crate::syntax::id(parent, "name") == Some(id);
    let parameter_property = parent.syntax_kind == K::Parameter
        && grandparent.is_some_and(|owner| {
            crate::syntax::is_parameter_property_declaration(ast, parent, owner)
        });
    if parameter_property {
        let modifier = crate::syntax::parameter_property_reference_anchor(
            ast,
            |node| parents[node.index()],
            parent_id,
        );
        if modifier == Some(id) {
            let name_id = crate::syntax::id(parent, "name")?;
            let name = ast.node(name_id);
            if name.syntax_kind != K::Identifier {
                return None;
            }
            return Some(CollectedReferenceToken {
                key: super::text::exact_text(ast, name).map(|text| text.to_owned()),
                syntax: ReferenceSyntax {
                    kind: node.syntax_kind,
                    native_span: node.span,
                    occurrence_span: ast.token_span(name_id),
                    role: R::ParameterPropertyModifier,
                    roles: super::collect::DECLARATION,
                    traversal_header: false,
                },
            });
        }
    }
    if node.syntax_kind == K::JsxNamespacedName && parent.syntax_kind == K::JsxAttribute && own_name
    {
        let namespace = crate::syntax::child(ast, node, "namespace")?;
        let name = crate::syntax::child(ast, node, "name")?;
        let key = super::text::exact_text(ast, namespace).and_then(|namespace| {
            super::text::exact_text(ast, name).map(|name| format!("{namespace}:{name}"))
        });
        return Some(CollectedReferenceToken {
            key,
            syntax: ReferenceSyntax {
                kind: node.syntax_kind,
                native_span: node.span,
                occurrence_span: crate::syntax::reference_occurrence_range(
                    node.syntax_kind,
                    ast.token_span(id),
                    None,
                ),
                role: R::JsxNamespacedAttribute,
                roles: super::collect::USE,
                traversal_header: false,
            },
        });
    }
    let identifier = matches!(node.syntax_kind, K::Identifier | K::PrivateIdentifier);
    let literal = matches!(
        node.syntax_kind,
        K::StringLiteral | K::NoSubstitutionTemplateLiteral | K::NumericLiteral
    );
    if !identifier && !literal {
        return None;
    }
    let declared = own_name
        && matches!(
            parent.syntax_kind,
            K::PropertyDeclaration
                | K::PropertySignature
                | K::PropertyAssignment
                | K::ShorthandPropertyAssignment
                | K::MethodDeclaration
                | K::MethodSignature
                | K::GetAccessor
                | K::SetAccessor
                | K::EnumMember
                | K::ModuleDeclaration
        );
    let computed = parent.syntax_kind == K::ComputedPropertyName
        && crate::syntax::id(parent, "expression") == Some(id);
    let element = parent.syntax_kind == K::ElementAccessExpression
        && crate::syntax::id(parent, "argumentExpression") == Some(id);
    let indexed = parent.syntax_kind == K::LiteralType
        && grandparent.is_some_and(|owner| owner.syntax_kind == K::IndexedAccessType);
    let binding = parent.syntax_kind == K::BindingElement
        && crate::syntax::id(parent, "propertyName") == Some(id);
    if literal
        && !binding
        && !crate::syntax::is_valid_reference_position(ast, |node| parents[node.index()], id)
        && !crate::syntax::is_reference_import_type_literal(ast, parents, id)
    {
        return None;
    }
    let shorthand = node.syntax_kind == K::Identifier
        && parent.syntax_kind == K::ShorthandPropertyAssignment
        && own_name;
    let declaration = own_name
        && !matches!(
            parent.syntax_kind,
            K::PropertyAccessExpression
                | K::JsxAttribute
                | K::JsxOpeningElement
                | K::JsxClosingElement
                | K::JsxSelfClosingElement
                | K::NamedTupleMember
        );
    let role = if shorthand {
        R::Shorthand
    } else if parameter_property && own_name {
        R::ParameterName
    } else if parent.syntax_kind == K::JsxAttribute {
        R::JsxAttribute
    } else if binding {
        R::BindingProperty
    } else if computed || element {
        if literal {
            R::LiteralKey
        } else {
            R::ComputedValue
        }
    } else if indexed {
        R::LiteralKey
    } else if node.syntax_kind == K::PrivateIdentifier {
        R::PrivateName
    } else if declared {
        R::MemberName
    } else {
        R::Name
    };
    let roles = if shorthand {
        super::collect::DECLARATION | super::collect::USE
    } else if declared || computed && literal || declaration {
        super::collect::DECLARATION
    } else {
        super::collect::USE
    };
    Some(CollectedReferenceToken {
        key: super::text::exact_text(ast, node).map(|text| text.to_owned()),
        syntax: ReferenceSyntax {
            kind: node.syntax_kind,
            native_span: node.span,
            occurrence_span: crate::syntax::reference_occurrence_range(
                node.syntax_kind,
                ast.token_span(id),
                None,
            ),
            role,
            roles,
            traversal_header: crate::syntax::is_reference_traversal_header(ast, parents, id),
        },
    })
}

impl super::inputs::BindingFiles<'_> {
    pub(in crate::project) fn json_declaration_location_span(
        &self,
        file: FileId,
        declaration: super::JsonDeclaration,
        length: usize,
    ) -> Span {
        let facts = self
            .file(file)
            .json()
            .expect("JSON declaration belongs to captured JSON");
        match declaration {
            super::JsonDeclaration::SourceFile => Span {
                start: 0,
                end: length,
            },
            super::JsonDeclaration::Value(value) => facts
                .describe(value)
                .span
                .expect("JSON object has a source range"),
            super::JsonDeclaration::Property(property) => facts.reference_key(property).span,
        }
    }
}
