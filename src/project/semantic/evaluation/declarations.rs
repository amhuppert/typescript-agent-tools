use super::super::model::DemandError;
use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use crate::project::binding::{self, BoundJson, JsonFacts};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;

#[derive(Clone, Copy)]
pub(super) enum DeclarationAssociation {
    Raw(binding::RawDeclaration),
    Attached(ReferenceDeclaration),
}

/// Native AST pointer identity across the retained and attached forms. The
/// syntax key is the existing complete attachment key; JSON keeps its own
/// captured node identity, including duplicate property declarations.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum HeaderDeclarationIdentity {
    Syntax(crate::project::sources::FileId, SyntaxKind, crate::Span),
    Json(crate::project::sources::FileId, binding::JsonDeclaration),
}
impl From<ReferenceDeclaration> for DeclarationAssociation {
    fn from(declaration: ReferenceDeclaration) -> Self {
        Self::Attached(declaration)
    }
}

impl Evaluator<'_, '_> {
    pub(super) fn header_declaration_identity(
        &self,
        declaration: DeclarationAssociation,
    ) -> Result<HeaderDeclarationIdentity, DemandError> {
        match declaration {
            DeclarationAssociation::Raw(raw) => {
                let facts = self
                    .source
                    .bindings()
                    .raw_declaration_facts(raw)
                    .map_err(|failure| super::super::binding_failure(self.inputs, None, failure))?;
                Ok(match facts.json {
                    Some(json) => HeaderDeclarationIdentity::Json(facts.file, json),
                    None => HeaderDeclarationIdentity::Syntax(facts.file, facts.kind, facts.span),
                })
            }
            DeclarationAssociation::Attached(ReferenceDeclaration::Captured(node)) => {
                let (kind, span) = self.source.observed_declaration_site(node)?;
                Ok(HeaderDeclarationIdentity::Syntax(node.file, kind, span))
            }
            DeclarationAssociation::Attached(ReferenceDeclaration::Json { file, declaration }) => {
                Ok(HeaderDeclarationIdentity::Json(file, declaration))
            }
        }
    }

    pub(super) fn reference_header_declarations(
        &self,
        symbol: ReferenceSymbol,
        consumer: Option<&Location>,
    ) -> Result<Vec<DeclarationAssociation>, DemandError> {
        let symbol = self.reference_current_header(symbol);
        if let Some(raw) = self.reference_raw_origin(symbol)? {
            return Ok(self.copy_header_declarations(
                self.source
                    .bindings()
                    .raw_symbol_declarations(raw.symbol())
                    .map(DeclarationAssociation::Raw),
                "declarations/header-raw",
            ));
        }
        self.property_header_declarations(
            symbol
                .property()
                .ok_or_else(|| super::super::internal("current symbol has no header"))?,
            consumer,
        )
    }

    /// Both actual header representations acquire the same ordered scalar
    /// association array here. Child syntax/materialization is a later question.
    pub(super) fn copy_header_declarations(
        &self,
        values: impl IntoIterator<Item = DeclarationAssociation>,
        _owner: &'static str,
    ) -> Vec<DeclarationAssociation> {
        values.into_iter().collect::<Vec<_>>()
    }

    pub(super) fn reference_header_value_declaration(
        &self,
        symbol: ReferenceSymbol,
        consumer: Option<&Location>,
    ) -> Result<Option<DeclarationAssociation>, DemandError> {
        let symbol = self.reference_current_header(symbol);
        if let Some(raw) = self.reference_raw_origin(symbol)? {
            return Ok(self
                .source
                .bindings()
                .raw_symbol_value_declaration(raw.symbol())
                .map(DeclarationAssociation::Raw));
        }
        self.property_header_value_declaration(
            symbol
                .property()
                .ok_or_else(|| super::super::internal("current symbol has no header"))?,
            consumer,
        )
    }

    pub(super) fn materialize_declaration(
        &mut self,
        declaration: DeclarationAssociation,
        consumer: &Location,
    ) -> Result<ReferenceDeclaration, DemandError> {
        match declaration {
            DeclarationAssociation::Raw(raw) => {
                let declaration = self
                    .source
                    .bindings()
                    .raw_declaration_facts(raw)
                    .map_err(|failure| self.generic_binding_failure(failure, consumer))?;
                self.native_reference_declaration(declaration)
            }
            DeclarationAssociation::Attached(declaration) => Ok(declaration),
        }
    }

    pub(super) fn header_declaration_kind(
        &self,
        declaration: DeclarationAssociation,
    ) -> Result<SyntaxKind, DemandError> {
        match declaration {
            DeclarationAssociation::Raw(raw) => {
                Ok(self.source.bindings().raw_declaration_kind(raw))
            }
            DeclarationAssociation::Attached(ReferenceDeclaration::Captured(node)) => self
                .source
                .observed_declaration_site(node)
                .map(|(kind, _)| kind),
            DeclarationAssociation::Attached(ReferenceDeclaration::Json { file, declaration }) => {
                let facts = self.inputs.bindings.json_facts(BoundJson {
                    file,
                    value: JsonFacts::ROOT,
                });
                let length = self.inputs.sources.get(file).text.as_str().len();
                Ok(facts
                    .declaration_facts(
                        declaration,
                        crate::Span {
                            start: 0,
                            end: length,
                        },
                    )
                    .0)
            }
        }
    }

    pub(super) fn header_declaration_default(
        &self,
        declaration: DeclarationAssociation,
    ) -> Result<bool, DemandError> {
        match declaration {
            DeclarationAssociation::Raw(raw) => {
                Ok(self.source.bindings().raw_declaration_default(raw))
            }
            DeclarationAssociation::Attached(ReferenceDeclaration::Captured(node)) => self
                .source
                .observed_declaration_has_modifier(node, SyntaxKind::DefaultKeyword),
            DeclarationAssociation::Attached(ReferenceDeclaration::Json { .. }) => Ok(false),
        }
    }

    pub(super) fn header_declaration_local(
        &self,
        declaration: DeclarationAssociation,
        consumer: Option<&Location>,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.header_declaration_local_facts(declaration)
            .map_err(|failure| self.current_fact_failure(failure, consumer))
    }
    pub(super) fn header_declaration_local_facts(
        &self,
        declaration: DeclarationAssociation,
    ) -> Result<Option<ReferenceSymbol>, binding::LookupFailure> {
        match declaration {
            DeclarationAssociation::Raw(raw) => Ok(self
                .source
                .bindings()
                .raw_declaration_local(raw)
                .map(Into::into)),
            DeclarationAssociation::Attached(ReferenceDeclaration::Captured(node)) => {
                let (kind, span) = self
                    .source
                    .observed_declaration_site(node)
                    .map_err(|failure| binding::LookupFailure::Native(failure.into()))?;
                self.source
                    .bindings()
                    .declaration_local_symbol(node.file, kind, span)
                    .map(|symbol| symbol.map(ReferenceSymbol::from_origin))
                    .map_err(Into::into)
            }
            DeclarationAssociation::Attached(ReferenceDeclaration::Json { .. }) => Ok(None),
        }
    }

    pub(super) fn header_declaration_location(
        &self,
        declaration: DeclarationAssociation,
    ) -> Result<Location, DemandError> {
        match declaration {
            DeclarationAssociation::Raw(raw) => {
                let (file, span) = self.source.bindings().raw_declaration_location(raw);
                Ok(super::super::location(self.inputs, file, span))
            }
            DeclarationAssociation::Attached(ReferenceDeclaration::Captured(node)) => {
                self.source.observed_declaration_location(node)
            }
            DeclarationAssociation::Attached(ReferenceDeclaration::Json { file, declaration }) => {
                let span = self.inputs.bindings.json_declaration_location_span(
                    file,
                    declaration,
                    self.inputs.sources.get(file).text.as_str().len(),
                );
                Ok(super::super::location(self.inputs, file, span))
            }
        }
    }
}

impl Evaluator<'_, '_> {
    /// A declared acquisition requires a real source association; nil/header
    /// flag probes do not manufacture one merely to satisfy this interface.
    pub(super) fn reference_symbol_consumer(
        &self,
        symbol: ReferenceSymbol,
        consumer: Option<&Location>,
    ) -> Result<Option<Location>, DemandError> {
        if let Some(consumer) = consumer {
            return Ok(Some(consumer.clone()));
        }
        self.reference_header_declarations(symbol, None)?
            .first()
            .copied()
            .map(|declaration| self.header_declaration_location(declaration))
            .transpose()
    }
    /// `getGlobalTypeDeclaration` observes first actual declaration provenance
    /// without attaching an AST or acquiring its declared type.
    pub(super) fn reference_global_type_declaration(
        &self,
        symbol: ReferenceSymbol,
    ) -> Result<Option<Location>, DemandError> {
        for declaration in self.reference_header_declarations(symbol, None)? {
            if matches!(
                self.header_declaration_kind(declaration)?,
                SyntaxKind::ClassDeclaration
                    | SyntaxKind::InterfaceDeclaration
                    | SyntaxKind::EnumDeclaration
                    | SyntaxKind::TypeAliasDeclaration
            ) {
                return self.header_declaration_location(declaration).map(Some);
            }
        }
        Ok(None)
    }
    /// `getDeclaredTypeOfSymbol` for an unlocated global lookup uses the actual
    /// own declaration when the acquisition, rather than selection, demands it.
    pub(super) fn declared_reference_type_at_global_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: Option<&Location>,
    ) -> super::super::model::NativeResult<super::super::model::TypeId> {
        let consumer = self
            .reference_symbol_consumer(symbol, consumer)?
            .ok_or_else(|| {
                super::super::internal("declared global type lacks its actual declaration")
            })?;
        self.declared_reference_type_completion(symbol, &consumer)
    }
}

impl Evaluator<'_, '_> {
    /// `isDeclarationContainedBy` compares actual AST Loc spans. Native
    /// deliberately does not require the declaration and container files to
    /// match; JSON carries its original PropertyAssignment/ObjectLiteral Loc.
    pub(super) fn reference_declaration_contained_by(
        &mut self,
        declaration: ReferenceDeclaration,
        container: binding::BoundSymbol,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        let span = self.reference_declaration_ordering(declaration)?.span;
        self.source
            .bindings()
            .native_declaration_contained_by(span, container)
            .map_err(|failure| self.generic_binding_failure(failure, consumer))
    }

    /// `isThisless` only recognizes its enumerated parameter/member kinds.
    /// A JSON declaration is a SourceFile, ObjectLiteral, or PropertyAssignment,
    /// so none enters the variable/function annotation questions.
    pub(super) fn reference_declarations_are_thisless(
        &mut self,
        declarations: &[ReferenceDeclaration],
    ) -> Result<bool, DemandError> {
        match declarations {
            [ReferenceDeclaration::Captured(declaration)] => {
                self.source.declarations_are_thisless(&[*declaration])
            }
            _ => Ok(false),
        }
    }

    /// `isThisPropertyAccessInConstructor` / `isAutoTypedProperty` need the
    /// actual ValueDeclaration. JSON PropertyAssignments have initializers and
    /// cannot be class properties or constructor-assignment declarations.
    pub(super) fn reference_property_access_auto_type(
        &mut self,
        value: Option<ReferenceDeclaration>,
        declarations: &[ReferenceDeclaration],
        access: super::super::source::ExpressionRoot,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        let value = match value {
            Some(ReferenceDeclaration::Captured(value)) => Some(value),
            Some(ReferenceDeclaration::Json { .. }) => return Ok(false),
            None => None,
        };
        // JSON declarations cannot supply a declaring constructor. Preserve
        // the order of every actual syntax declaration that can do so.
        let declarations = declarations
            .iter()
            .filter_map(|declaration| match declaration {
                ReferenceDeclaration::Captured(declaration) => Some(*declaration),
                ReferenceDeclaration::Json { .. } => None,
            })
            .collect::<Vec<_>>();
        self.source
            .symbol_access_auto_type(value, &declarations, access, consumer)
    }

    /// `getFlowTypeOfAccessExpression` / `isAssignmentToReadonlyEntity`:
    /// JSON nodes cannot meet the PropertyDeclaration/BinaryExpression ancestry
    /// gates. Their presence remains independent from this negative question.
    pub(super) fn reference_property_access_context(
        &mut self,
        declaration: ReferenceDeclaration,
        access: super::super::source::ExpressionRoot,
    ) -> Result<super::super::source::AccessDeclarationContext, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => {
                self.source.property_access_context(declaration, access)
            }
            ReferenceDeclaration::Json { .. } => {
                Ok(super::super::source::AccessDeclarationContext {
                    readonly_constructor_write: false,
                    assume_uninitialized: false,
                })
            }
        }
    }

    /// `isAutoTypedProperty`: JSON's three native declaration kinds do not
    /// enter class-field or this-property assignment inference.
    pub(super) fn reference_property_is_auto_typed(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> Result<bool, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => {
                self.source.property_is_auto_typed(declaration)
            }
            ReferenceDeclaration::Json { .. } => Ok(false),
        }
    }

    /// Native declaration ModifierFlags are zero on every JSON AST node.
    pub(super) fn reference_declaration_own_modifier_flags(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> Result<u32, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => {
                self.source.declaration_own_modifier_flags(declaration)
            }
            ReferenceDeclaration::Json { .. } => Ok(0),
        }
    }
    pub(super) fn reference_declaration_has_modifier(
        &mut self,
        declaration: ReferenceDeclaration,
        modifier: SyntaxKind,
    ) -> Result<bool, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => {
                self.source.declaration_has_modifier(declaration, modifier)
            }
            ReferenceDeclaration::Json { .. } => Ok(false),
        }
    }
    pub(super) fn reference_declaration_is_ambient(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> Result<bool, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => {
                self.source.declaration_is_ambient(declaration)
            }
            ReferenceDeclaration::Json { .. } => Ok(false),
        }
    }

    /// `SymbolName` / `IsPrivateIdentifierClassElementDeclaration`: JSON
    /// properties have string-literal names and are not class elements.
    pub(super) fn reference_private_identifier_declaration_name(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> Result<Option<String>, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => {
                self.source.private_identifier_declaration_name(declaration)
            }
            ReferenceDeclaration::Json { .. } => Ok(None),
        }
    }
    /// `checkIndexConstraintForProperty` tests the actual declaration's Name.
    pub(super) fn reference_declaration_name_is_private(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> Result<bool, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => Ok(self
                .source
                .declaration_name_site(declaration)?
                .is_some_and(|(_, name)| name.kind == SyntaxKind::PrivateIdentifier)),
            ReferenceDeclaration::Json { .. } => Ok(false),
        }
    }

    /// `getLiteralTypeFromPropertyName(GetNameOfDeclaration(...))` retains the
    /// original JSON key occurrence; it never substitutes the final symbol's
    /// name or a fabricated SourceFile declaration.
    pub(super) fn reference_declaration_literal_name_completion(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> super::super::model::NativeResult<Option<super::super::model::TypeId>> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => {
                match self.source.declaration_name_site(declaration)? {
                    Some((file, name)) => Ok(self
                        .literal_type_of_property_name_completion(file, name)?
                        .map(Some)),
                    None => Ok(super::super::model::NativeCompletion::clean(None)),
                }
            }
            ReferenceDeclaration::Json {
                file,
                declaration: binding::JsonDeclaration::Property(property),
            } => {
                let facts = self.inputs.bindings.json_facts(BoundJson {
                    file,
                    value: JsonFacts::ROOT,
                });
                let name = facts.reference_key(property);
                let text = name.key.as_str().ok_or_else(|| {
                    let location = super::super::location(self.inputs, file, name.span);
                    super::super::unsupported_demand_at_location(
                        self.inputs,
                        &location,
                        crate::analysis::SemanticFailureReason::UnrepresentableIdentity,
                        "JSON property identity contains an unpaired UTF-16 surrogate",
                    )
                })?;
                let key = binding::NativePropertyKey::fixed(text.as_bytes(), self.control.as_ref());
                Ok(super::super::model::NativeCompletion::clean(Some(
                    self.model.string_literal_for_key(&key),
                )))
            }
            ReferenceDeclaration::Json { .. } => {
                Ok(super::super::model::NativeCompletion::clean(None))
            }
        }
    }

    /// `shouldCheckAsExcessProperty` is an actual Parent pointer comparison.
    /// The JSON arena owns PropertyAssignment -> original ObjectLiteral
    /// attachment, including duplicates; it does not follow final-value links.
    pub(super) fn reference_should_check_excess_property(
        &mut self,
        property: ReferenceDeclaration,
        container: ReferenceDeclaration,
    ) -> Result<bool, DemandError> {
        match (property, container) {
            (
                ReferenceDeclaration::Captured(property),
                ReferenceDeclaration::Captured(container),
            ) => self
                .source
                .should_check_excess_property(property, container),
            (
                ReferenceDeclaration::Json {
                    file,
                    declaration: property,
                },
                ReferenceDeclaration::Json {
                    file: outer,
                    declaration: container,
                },
            ) if file == outer => Ok(self
                .inputs
                .bindings
                .json_facts(BoundJson {
                    file,
                    value: JsonFacts::ROOT,
                })
                .declaration_has_parent(property, container)),
            _ => Ok(false),
        }
    }
}
