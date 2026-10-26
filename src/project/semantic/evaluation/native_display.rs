//! Native diagnostic serialization acquires types independently of Q4 output.
//! The node builder's approximate UTF-8 length, sticky truncation and recursion
//! state govern which dependencies are read; they are not product budgets.

use super::super::model::{
    DemandError, MapperId, MembersView, NativeCompletion, NativeResult, ObjectFlags, ObjectKind,
    ParameterValue, Property, PropertyDisplay, PropertyDisplayDemand, Signature, SpecialName,
    TypeFlags, TypeId, TypeSymbol, TypeView,
};
use super::super::source::DeclarationIdentity;
use super::{Evaluator, ReferenceDisplay};
use crate::project::binding::{NativeNodeIdentity, NativeSymbolIdentity};
use crate::references::Location;
use crate::types::{PrimitiveName, TypeKey};
use std::collections::{HashMap, HashSet};
mod anonymous;
mod enums;
mod module_specifiers;
mod qualified;
mod scopes;
mod symbols;
pub(super) use scopes::DiagnosticScopeId;
mod binding_names;
mod cache;
mod declarations;
mod mapped;
mod pseudo;
mod pseudo_output;
mod reuse;
mod syntax;
use syntax::{Element, Member, Node, SignatureKind, TypeParameter};

/// Exact TypeFormatFlags; FULLY_QUALIFIED deliberately does not imply DEFAULT.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct DiagnosticTypeFormat(u32);
impl DiagnosticTypeFormat {
    pub(super) const DEFAULT: Self = Self((1 << 20) | (1 << 14));
    pub(super) const WRITE_ARRAY_AS_GENERIC_TYPE: Self = Self(1 << 1);
    pub(super) const FULLY_QUALIFIED: Self = Self(1 << 6);
    pub(super) const NO_TYPE_REDUCTION: Self = Self(1 << 29);
    fn has(self, bits: u32) -> bool {
        self.0 & bits != 0
    }
}

/// The printer's byte cutoff can split UTF-8. Reporting equality must retain
/// those bytes rather than compare replacement-character message strings.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct RenderedTypeName(Vec<u8>);
impl RenderedTypeName {
    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

/// The emitted node kind, retained independently from its text for Q4
/// precedence. `typeof import(...)` is an ImportType, not a TypeQuery.
pub(super) struct SymbolTypeDisplay {
    pub(super) name: RenderedTypeName,
    pub(super) type_query: bool,
}

/// The completed read/name prefix of addPropertyToElementList. Keeping this
/// opaque continuation also lets Q4 retain an established name if its later
/// mode question fails, without replaying the property's read evidence.
pub(in crate::project::semantic) struct PropertyDisplayPreparation {
    name: String,
    read: Option<TypeId>,
    display: PropertyDisplayDemand,
}
impl PropertyDisplayPreparation {
    pub(in crate::project::semantic) fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Default)]
pub(super) struct NativeDisplayState {
    level: usize,
    scopes: Vec<scopes::DiagnosticScope>,
    serialized: cache::SerializedTypes,
    specifiers: module_specifiers::Specifiers,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum DepthIdentity {
    Node(NativeNodeIdentity),
    Symbol(NativeSymbolIdentity, bool),
}

struct Context {
    flags: DiagnosticTypeFormat,
    enclosing: Option<super::symbol_accessibility::ScopeLocation>,
    // Native enclosingFile is fixed at entry; temporary property/fake scopes
    // change enclosingDeclaration without changing this serialization origin.
    enclosing_file: Option<crate::project::sources::FileId>,
    length: usize,
    limit: usize,
    truncating: bool,
    visited: HashSet<TypeId>,
    depths: HashMap<DepthIdentity, usize>,
    infer_parameters: Vec<TypeId>,
    mapper: Option<MapperId>,
    tracked_symbols: Vec<cache::TrackedSymbol>,
    recovery_boundaries: Vec<reuse::RecoveryBoundary>,
    reported_diagnostic: bool,
    encountered_error: bool,
    type_parameter_symbol_list: HashSet<NativeSymbolIdentity>,
    type_parameter_names: HashMap<TypeId, String>,
    type_parameter_names_by_text: HashSet<String>,
    type_parameter_name_counts: HashMap<String, usize>,
    // Native 7.0.2 initializes and reads this map; it has no remap writer.
    remapped_symbol_references: HashMap<NativeSymbolIdentity, super::ReferenceSymbol>,
    enclosing_symbol_types: HashMap<NativeSymbolIdentity, TypeId>,
    suppress_report_inference_fallback: bool,
    reverse_mapped_stack: Vec<super::super::model::PropertySymbolId>,
}
impl Context {
    fn new(
        flags: DiagnosticTypeFormat,
        enclosing: Option<DeclarationIdentity>,
        no_truncation: bool,
    ) -> Self {
        // printer.typeToStringEx/signatureToStringEx/symbolToStringEx all
        // add NodeBuilder IgnoreErrors internally. These flags affect native
        // traversal choices (including import portability), not product errors.
        let ignored =
            (1 << 15) | (1 << 16) | (1 << 17) | (1 << 18) | (1 << 19) | (1 << 21) | (1 << 26);
        let flags = DiagnosticTypeFormat(flags.0 | ignored | u32::from(no_truncation));
        Self {
            flags,
            enclosing: enclosing.map(Into::into),
            enclosing_file: enclosing.map(|declaration| declaration.file),
            length: 0,
            limit: if no_truncation { 1_000_000 } else { 160 },
            truncating: false,
            visited: HashSet::new(),
            depths: HashMap::new(),
            infer_parameters: Vec::new(),
            mapper: None,
            tracked_symbols: Vec::new(),
            recovery_boundaries: Vec::new(),
            reported_diagnostic: false,
            encountered_error: false,
            type_parameter_symbol_list: HashSet::new(),
            type_parameter_names: HashMap::new(),
            type_parameter_names_by_text: HashSet::new(),
            type_parameter_name_counts: HashMap::new(),
            remapped_symbol_references: HashMap::new(),
            enclosing_symbol_types: HashMap::new(),
            suppress_report_inference_fallback: false,
            reverse_mapped_stack: Vec::new(),
        }
    }
    fn elision(&self, count: Option<usize>) -> Node {
        if self.limit == 1_000_000 {
            Node::text("any")
        } else {
            Node::Reference(
                count.map_or_else(|| "...".to_owned(), |count| format!("... {count} more ...")),
                Vec::new(),
            )
        }
    }
    /// `checkTruncationLength`: the boundary is strictly greater and sticky.
    fn truncating(&mut self) -> bool {
        self.truncating |= self.length > self.limit;
        self.truncating
    }
    fn elide(&mut self) {
        self.length += 3;
    }
    fn symbol_name(&mut self, name: &str) {
        // createAccessFromSymbolChain counts the initial name once in its
        // initial-entity branch and once after its name fallback. Go len is
        // UTF-8 bytes, including scanner-preserved spelling and escape bytes.
        self.length += 2 * (name.len() + 1);
    }
}

impl Evaluator<'_, '_> {
    /// `getTypeNameForErrorDisplay` uses the fully-qualified flag alone and
    /// no enclosing declaration; it is distinct from default type display.
    pub(super) fn type_name_for_error_display_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<u8>> {
        Ok(self
            .diagnostic_type_display_completion(
                ty,
                None,
                DiagnosticTypeFormat::FULLY_QUALIFIED,
                consumer,
            )?
            .map(RenderedTypeName::into_bytes))
    }

    pub(super) fn raw_type_name_for_error_display_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<u8>> {
        Ok(self
            .raw_diagnostic_type_display_completion(
                ty,
                None,
                DiagnosticTypeFormat::FULLY_QUALIFIED,
                consumer,
            )?
            .map(RenderedTypeName::into_bytes))
    }

    /// `getTypeNamesForErrorDisplay`: acquire left then right with their
    /// independently selected contexts. Equal bytes trigger two fresh fully
    /// qualified displays with that flag alone, not the default bitset.
    pub(super) fn type_names_for_error_display_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<(Vec<u8>, Vec<u8>)> {
        Ok(self
            .raw_type_names_for_error_display_completion(source, target, consumer)
            .map_err(|failure| super::super::observe_recovery(self.inputs, failure, consumer))?
            .observe(|failure| super::super::observe_recovery(self.inputs, failure, consumer)))
    }

    pub(super) fn raw_type_names_for_error_display_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<(Vec<u8>, Vec<u8>)> {
        let context = self.error_display_context(source, consumer)?;
        self.raw_diagnostic_type_display_completion(
            source,
            context,
            DiagnosticTypeFormat::DEFAULT,
            consumer,
        )?
        .and_then(|source_name| {
            let context = self.error_display_context(target, consumer)?;
            self.raw_diagnostic_type_display_completion(
                target,
                context,
                DiagnosticTypeFormat::DEFAULT,
                consumer,
            )?
            .and_then(|target_name| {
                if source_name == target_name {
                    self.raw_type_name_for_error_display_completion(source, consumer)?
                        .and_then(|source_name| {
                            Ok(self
                                .raw_type_name_for_error_display_completion(target, consumer)?
                                .map(|target_name| (source_name, target_name)))
                        })
                } else {
                    Ok(NativeCompletion::clean((
                        source_name.into_bytes(),
                        target_name.into_bytes(),
                    )))
                }
            })
        })
    }

    /// `symbolValueDeclarationIsContextSensitive` deliberately negates the
    /// contextual predicate. It selects an enclosing declaration for native
    /// diagnostic display, never a product graph origin.
    fn error_display_context(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        let declaration = self.type_symbol_value_declaration(ty, consumer)?;
        if let Some(super::ReferenceDeclaration::Captured(declaration)) = declaration
            && let Some(expression) = self.source.declaration_expression(declaration)?
            && !self.source.expression_is_context_sensitive(expression)?
        {
            return Ok(Some(declaration));
        }
        Ok(None)
    }

    /// The trailing-argument decision in `typeReferenceToTypeNode` is shared
    /// by diagnostic and bounded product display. Callers have already read
    /// the full vector and processed any outer symbol groups. Iterator global
    /// getters short-circuit in native order even for an unrelated reference.
    pub(in crate::project::semantic) fn reference_display_argument_count(
        &mut self,
        ty: TypeId,
        arguments: &[TypeId],
    ) -> Result<usize, DemandError> {
        let TypeView::Reference(reference) = self.model.get(ty)? else {
            return Err(super::super::internal("reference display arguments lack target").into());
        };
        let target = self.model.declared_target(reference.target)?;
        let parameters: Vec<_> = target
            .outer_parameters
            .iter()
            .chain(target.local_parameters.iter())
            .copied()
            .collect();
        let mut count = parameters.len().min(arguments.len());
        if arguments.is_empty() || parameters.is_empty() {
            return Ok(count);
        }
        let consumer = self.member_consumer(ty)?;
        let mut iterator = false;
        for name in [
            "Iterable",
            "IterableIterator",
            "AsyncIterable",
            "AsyncIterableIterator",
        ] {
            if let super::globals::GlobalTypeLookup::Target { ty: target, .. } =
                self.global_type_lookup(name, 3, &consumer)?
                && target == reference.target
            {
                iterator = true;
                break;
            }
        }
        if iterator {
            let written = self
                .model
                .deferred_reference(ty)
                .map(|reference| {
                    self.source
                        .diagnostic_reference_argument_count(reference.declaration)
                })
                .transpose()?
                .flatten();
            if written.is_none_or(|written| written < count) {
                while count > 0 {
                    let Some(default) = self.default_from_type_parameter(parameters[count - 1])?
                    else {
                        break;
                    };
                    if !self.type_related(
                        arguments[count - 1],
                        default,
                        super::relations::RelationKind::Identity,
                        &consumer,
                    )? {
                        break;
                    }
                    count -= 1;
                }
            }
        }
        Ok(count)
    }

    /// `Checker.TypeToString` / `typeToStringEx` diagnostic acquisition. This
    /// entry has nil enclosingDeclaration, AllowUniqueESSymbolType,
    /// UseAliasDefinedOutsideCurrentScope and IgnoreErrors. Native's level
    /// gate is checked before another node-builder context is allocated.
    pub(super) fn check_diagnostic_type_display(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        self.check_diagnostic_type_display_completion(ty, consumer)?
            .admit()
    }

    pub(super) fn raw_check_diagnostic_type_display_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        Ok(self
            .raw_diagnostic_type_display_completion(
                ty,
                None,
                DiagnosticTypeFormat::DEFAULT,
                consumer,
            )?
            .map(|_| ()))
    }

    pub(super) fn check_diagnostic_type_display_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        Ok(self
            .diagnostic_type_display_completion(ty, None, DiagnosticTypeFormat::DEFAULT, consumer)?
            .map(|_| ()))
    }

    /// `typeToStringEx`: one node-builder traversal, then the native printer's
    /// independent final byte limit. Restores serializationLevel on failures.
    pub(super) fn diagnostic_type_display_completion(
        &mut self,
        ty: TypeId,
        enclosing: Option<DeclarationIdentity>,
        flags: DiagnosticTypeFormat,
        consumer: &Location,
    ) -> NativeResult<RenderedTypeName> {
        Ok(self
            .raw_diagnostic_type_display_completion(ty, enclosing, flags, consumer)
            .map_err(|failure| super::super::observe_recovery(self.inputs, failure, consumer))?
            .observe(|failure| super::super::observe_recovery(self.inputs, failure, consumer)))
    }

    /// A cached native operation retains this source-free completion. Its
    /// caller observes a returned copy only after its own cache boundary.
    pub(super) fn raw_diagnostic_type_display_completion(
        &mut self,
        ty: TypeId,
        enclosing: Option<DeclarationIdentity>,
        flags: DiagnosticTypeFormat,
        consumer: &Location,
    ) -> NativeResult<RenderedTypeName> {
        if self.native_display.level >= 2 {
            return Ok(NativeCompletion::clean(RenderedTypeName(b"?".to_vec())));
        }
        self.native_display.level += 1;
        let no_truncation = self.inputs.semantic_options().no_error_truncation || flags.has(1);
        let mut context = Context::new(flags, enclosing, no_truncation);
        let result = self.diagnostic_type_node(ty, &mut context, consumer);
        self.native_display.level -= 1;
        Ok(result?.map(|node| {
            let mut bytes = node.print();
            let maximum = if no_truncation { 2_000_000 } else { 320 };
            if bytes.len() >= maximum {
                bytes.truncate(maximum - 3);
                bytes.extend_from_slice(b"...");
            }
            RenderedTypeName(bytes)
        }))
    }

    /// `typeToTypeNode`: primitive dispatch precedes alias selection; an
    /// alias's type arguments precede symbol spelling except for Any aliases.
    fn diagnostic_type_node(
        &mut self,
        ty: TypeId,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        let in_type_alias = context.flags.has(1 << 23);
        context.flags.0 &= !(1 << 23);
        let ty = if context.flags.has(1 << 29) {
            ty
        } else {
            self.reduced_type(ty, consumer)?
        };
        let metadata = self.model.metadata(ty)?;
        let flags = self.model.type_flags(ty);
        if flags.intersects(TypeFlags::ANY) {
            if let Some(alias) = metadata.alias {
                let name = self.diagnostic_raw_symbol_entity(
                    self.alias_reference_symbol(alias.symbol),
                    consumer,
                )?;
                return Ok(self
                    .diagnostic_type_list(
                        alias.arguments.as_deref().unwrap_or(&[]),
                        false,
                        context,
                        consumer,
                    )?
                    .map(|arguments| Node::Named(Box::new(name), arguments)));
            }
            if ty == self.model.unresolved_type() {
                // `typeToTypeNode` attaches this synthetic leading comment
                // before the ordinary Any branch's approximate-length update.
                return Ok(NativeCompletion::clean(Node::text("/*unresolved*/ any")));
            }
            context.length += 3;
            return Ok(NativeCompletion::clean(Node::text(
                if self.model.is_intrinsic_marker(ty) {
                    "intrinsic"
                } else {
                    "any"
                },
            )));
        }
        if flags.intersects(TypeFlags::UNKNOWN) {
            return Ok(NativeCompletion::clean(Node::text("unknown")));
        }
        for (flag, name) in [
            (TypeFlags::STRING, "string"),
            (TypeFlags::NUMBER, "number"),
            (TypeFlags::BIGINT, "bigint"),
        ] {
            if flags.intersects(flag) {
                context.length += 6;
                return Ok(NativeCompletion::clean(Node::text(name)));
            }
        }
        if flags.intersects(TypeFlags::BOOLEAN) && metadata.alias.is_none() {
            context.length += 7;
            return Ok(NativeCompletion::clean(Node::text("boolean")));
        }
        if flags.intersects(TypeFlags::ENUM_LIKE) {
            return self.diagnostic_enum_type_node(ty, context, consumer);
        }
        let view = self.model.get(ty)?;
        if let TypeView::Literal { base, value } = &view {
            context.length += match base {
                PrimitiveName::String => value.display_length() + 2,
                PrimitiveName::Bigint => value.display_length() + 1,
                _ => value.display_length(),
            };
            return Ok(NativeCompletion::clean(Node::text(match base {
                PrimitiveName::String => self.quoted_literal_value(
                    value,
                    if context.flags.has(1 << 28) {
                        '\''
                    } else {
                        '"'
                    },
                    consumer,
                )?,
                PrimitiveName::Bigint => format!("{}n", self.literal_scalar_text(value, consumer)?),
                _ => self.literal_scalar_text(value, consumer)?.to_owned(),
            })));
        }
        if flags.intersects(TypeFlags::UNIQUE_SYMBOL) {
            if !context.flags.has(1 << 20) {
                let symbol = self
                    .reference_symbol_of_type(ty, consumer)?
                    .ok_or_else(|| super::super::internal("unique type has no symbol"))?;
                return self
                    .is_value_symbol_accessible_completion(symbol, context.enclosing, consumer)?
                    .and_then(|accessible| {
                        if accessible {
                            context.length += 6;
                            return self.symbol_type_node(
                                symbol,
                                super::SymbolMeaning::Value,
                                None,
                                context,
                                consumer,
                            );
                        }
                        context.report_diagnostic();
                        context.length += 13;
                        Ok(NativeCompletion::clean(Node::Prefix(
                            "unique ",
                            Box::new(Node::text("symbol")),
                        )))
                    });
            }
            context.length += 13;
            return Ok(NativeCompletion::clean(Node::Prefix(
                "unique ",
                Box::new(Node::text("symbol")),
            )));
        }
        if let TypeView::Special(name) = &view {
            context.length += if *name == SpecialName::Unknown {
                0
            } else {
                name.text().len()
            };
            return Ok(NativeCompletion::clean(Node::text(name.text())));
        }
        if let TypeView::Primitive(name) = &view {
            let text = match name {
                PrimitiveName::String => "string",
                PrimitiveName::Number => "number",
                PrimitiveName::Boolean => "boolean",
                PrimitiveName::Bigint => "bigint",
                PrimitiveName::Symbol => "symbol",
                PrimitiveName::Object => "object",
            };
            context.length += if *name == PrimitiveName::Boolean {
                7
            } else {
                6
            };
            return Ok(NativeCompletion::clean(Node::text(text)));
        }
        if let TypeView::TypeParameter(parameter) = &view
            && parameter.is_this
        {
            if context.flags.has(1 << 22) {
                if !context.flags.has(1 << 15) {
                    context.encountered_error = true;
                }
                // Even the nil-inner diagnostic tracker records this report;
                // IgnoreErrors allows the node but does not allow cache install.
                context.report_diagnostic();
            }
            context.length += 4;
            return Ok(NativeCompletion::clean(Node::text("this")));
        }
        let alias_allowed = match &metadata.alias {
            Some(alias) if !in_type_alias && !context.flags.has(1 << 14) => self
                .is_symbol_accessible_completion(
                    self.alias_reference_symbol(alias.symbol),
                    context.enclosing,
                    super::SymbolMeaning::Type,
                    consumer,
                )?,
            Some(_) if !in_type_alias => NativeCompletion::clean(true),
            _ => NativeCompletion::clean(false),
        };
        alias_allowed.and_then(|alias_allowed| {
            if alias_allowed && let Some(alias) = &metadata.alias {
                return self
                    .diagnostic_type_list(
                        alias.arguments.as_deref().unwrap_or(&[]),
                        false,
                        context,
                        consumer,
                    )?
                    .and_then(|arguments| {
                        let symbol = self.alias_reference_symbol(alias.symbol);
                        let key = self.reference_symbol_key(symbol, consumer)?;
                        if key.is_reserved_member_name()
                            && !self.reference_symbol_flags(symbol, consumer)?.class
                        {
                            return Ok(NativeCompletion::clean(Node::Reference(
                                String::new(),
                                arguments,
                            )));
                        }
                        if arguments.len() == 1
                            && let Some((array, _)) = self.fixed_global_type_identity(
                                super::globals::FixedGlobalType::Array,
                                consumer,
                            )?
                            && self.reference_symbol_identity(array, consumer)?
                                == self.reference_symbol_identity(symbol, consumer)?
                        {
                            return Ok(NativeCompletion::clean(Node::Array(Box::new(
                                arguments.into_iter().next().expect("one argument"),
                            ))));
                        }
                        self.symbol_type_node(
                            symbol,
                            super::SymbolMeaning::Type,
                            (!arguments.is_empty()).then_some(arguments),
                            context,
                            consumer,
                        )
                    });
            }
            match view {
                TypeView::Reference(reference) => {
                    if self.model.deferred_reference(ty).is_some() {
                        self.diagnostic_visit(ty, context, consumer, Self::diagnostic_reference)
                    } else {
                        self.diagnostic_reference(reference.ty, context, consumer)
                    }
                }
                TypeView::TypeParameter(parameter) => {
                    if context.infer_parameters.contains(&ty) {
                        context.length += parameter.name.len() + 6;
                        let constraint =
                            if let Some(constraint) = self.infer_constraint_for_display(ty)? {
                                context.length += 9;
                                self.diagnostic_type_node(constraint, context, consumer)?
                                    .map(|node| Some(Box::new(node)))
                            } else {
                                NativeCompletion::clean(None)
                            };
                        constraint.and_then(|constraint| {
                            Ok(self
                                .diagnostic_parameter_declaration(
                                    ty, constraint, context, consumer,
                                )?
                                .map(Node::Infer))
                        })
                    } else if context.flags.has(1 << 2) {
                        Ok(self
                            .diagnostic_type_parameter_name_completion(ty, context, consumer)?
                            .map(|name| {
                                context.length += name.len();
                                Node::Reference(name, Vec::new())
                            }))
                    } else if let Some(symbol) = self.reference_symbol_of_type(ty, consumer)? {
                        self.symbol_type_node(
                            symbol,
                            super::SymbolMeaning::Type,
                            None,
                            context,
                            consumer,
                        )
                    } else {
                        Ok(NativeCompletion::clean(Node::Reference(
                            "?".to_owned(),
                            Vec::new(),
                        )))
                    }
                }
                TypeView::Union(_) | TypeView::Intersection(_) => {
                    let displayed = metadata.summary_origin.unwrap_or(ty);
                    let (types, union) = match self.model.get(displayed)? {
                        TypeView::Union(types) => (types, true),
                        TypeView::Intersection(types) => (types, false),
                        _ => return self.diagnostic_type_node(displayed, context, consumer),
                    };
                    let types = if union {
                        self.formatted_union_types_completion(&types, false, consumer)?
                    } else {
                        NativeCompletion::clean(types.to_vec())
                    };
                    types.and_then(|types| {
                        if types.len() == 1 {
                            self.diagnostic_type_node(types[0], context, consumer)
                        } else {
                            Ok(self
                                .diagnostic_type_list(&types, true, context, consumer)?
                                .map(|nodes| {
                                    if union {
                                        Node::Union(nodes)
                                    } else {
                                        Node::Intersection(nodes)
                                    }
                                }))
                        }
                    })
                }
                TypeView::Object(_) | TypeView::LazyObject(_) => {
                    if !self
                        .model
                        .object_flags(ty)?
                        .intersects(ObjectFlags::INSTANTIATION_EXPRESSION_TYPE)
                        && let Some(symbol) =
                            self.anonymous_typeof_symbol(ty, context, false, false, consumer)?
                    {
                        return self.symbol_type_node(
                            symbol,
                            super::SymbolMeaning::Value,
                            None,
                            context,
                            consumer,
                        );
                    }
                    if let Some(name) = metadata.nominal_name {
                        let symbol = self.model.bound_type_symbol(ty)?.ok_or_else(|| {
                            self.diagnostic_dependency(
                                consumer,
                                "native nominal diagnostic symbol is unavailable",
                            )
                        })?;
                        return self.symbol_type_node(
                            symbol.into(),
                            if name.starts_with("typeof ") {
                                super::SymbolMeaning::Value
                            } else {
                                super::SymbolMeaning::Type
                            },
                            None,
                            context,
                            consumer,
                        );
                    }
                    if context.visited.contains(&ty) {
                        if let Some(symbol) = self.model.bound_type_symbol(ty)?
                            && let Some(alias) = self
                                .source
                                .diagnostic_type_literal_alias(symbol, consumer)?
                        {
                            return self
                                .reference_symbol_of_declaration_completion(alias)?
                                .and_then(|alias| {
                                    self.symbol_type_node(
                                        alias,
                                        super::SymbolMeaning::Type,
                                        None,
                                        context,
                                        consumer,
                                    )
                                });
                        }
                        context.elide();
                        return Ok(NativeCompletion::clean(context.elision(None)));
                    }
                    if self
                        .model
                        .object_flags(ty)?
                        .intersects(ObjectFlags::INSTANTIATION_EXPRESSION_TYPE)
                    {
                        return self
                            .diagnostic_instantiation_expression_node(ty, context, consumer)?
                            .and_then(|node| match node {
                                Some(node) => Ok(NativeCompletion::clean(node)),
                                None => self.diagnostic_visit(
                                    ty,
                                    context,
                                    consumer,
                                    Self::diagnostic_object,
                                ),
                            });
                    }
                    if self.model.has_type_symbol(ty)? {
                        self.diagnostic_visit(ty, context, consumer, Self::diagnostic_object)
                    } else {
                        self.diagnostic_object(ty, context, consumer)
                    }
                }
                TypeView::Index(index) => {
                    context.length += 6;
                    Ok(self
                        .diagnostic_type_node(index.target, context, consumer)?
                        .map(|node| Node::Prefix("keyof ", Box::new(node))))
                }
                TypeView::IndexedAccess(access) => self
                    .diagnostic_type_node(access.object_type, context, consumer)?
                    .and_then(|object| {
                        Ok(self
                            .diagnostic_type_node(access.index_type, context, consumer)?
                            .map(|index| {
                                context.length += 2;
                                Node::Indexed(Box::new(object), Box::new(index))
                            }))
                    }),
                TypeView::TemplateLiteral(template) => {
                    let mut types = NativeCompletion::clean(Vec::new());
                    for &ty in template.types.iter() {
                        types = types.and_then(|mut nodes| {
                            Ok(self
                                .diagnostic_type_node(ty, context, consumer)?
                                .map(|node| {
                                    nodes.push(node);
                                    nodes
                                }))
                        })?;
                    }
                    types.and_then_value(|types| {
                        let texts = template
                            .texts
                            .iter()
                            .map(|text| {
                                text.fixed_bytes()
                                    .map(<[u8]>::to_vec)
                                    .map_err(|error| self.literal_value_failure(error, consumer))
                            })
                            .collect::<Result<Vec<_>, DemandError>>()?;
                        context.length += 2;
                        Ok(Node::Template(texts, types))
                    })
                }
                TypeView::StringMapping(mapping) => self
                    .diagnostic_type_node(mapping.target, context, consumer)?
                    .and_then(|argument| {
                        self.symbol_type_node(
                            self.alias_reference_symbol(mapping.symbol),
                            super::SymbolMeaning::Type,
                            Some(vec![argument]),
                            context,
                            consumer,
                        )
                    }),
                TypeView::Conditional(_) => {
                    self.diagnostic_visit(ty, context, consumer, Self::diagnostic_conditional)
                }
                TypeView::Substitution(substitution) => self
                    .diagnostic_type_node(substitution.base, context, consumer)?
                    .and_then_value(|node| {
                        if self.display_no_infer_alias(ty)? {
                            context.symbol_name("NoInfer");
                            Ok(Node::Reference("NoInfer".to_owned(), vec![node]))
                        } else {
                            Ok(node)
                        }
                    }),
                TypeView::Enum
                | TypeView::Special(_)
                | TypeView::Primitive(_)
                | TypeView::UniqueSymbol(_)
                | TypeView::Literal { .. } => unreachable!("primitive branch returned"),
            }
        })
    }

    fn diagnostic_dependency(&self, consumer: &Location, message: &str) -> DemandError {
        super::super::unsupported_demand_at_location(
            self.inputs,
            consumer,
            crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
            message,
        )
    }

    /// `mapToTypeNodes`: the returned indices identify retained nodes; tuple
    /// labels follow those output positions, not the elided input positions.
    fn diagnostic_type_list(
        &mut self,
        types: &[TypeId],
        bare: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Vec<Node>> {
        if types.is_empty() {
            return Ok(NativeCompletion::clean(Vec::new()));
        }
        if context.truncating() {
            if !bare {
                return Ok(NativeCompletion::clean(vec![context.elision(None)]));
            }
            if types.len() > 2 {
                return self
                    .diagnostic_type_node(types[0], context, consumer)?
                    .and_then(|first| {
                        Ok(self
                            .diagnostic_type_node(types[types.len() - 1], context, consumer)?
                            .map(|last| vec![first, context.elision(Some(types.len() - 2)), last]))
                    });
            }
        }
        let mut nodes = NativeCompletion::clean(Vec::new());
        let mut named: Vec<(String, Vec<(TypeId, usize)>)> = Vec::new();
        for (index, &ty) in types.iter().enumerate() {
            if context.truncating() && index + 3 < types.len() - 1 {
                nodes = nodes.and_then(|mut nodes| {
                    nodes.push(context.elision(Some(types.len() - index - 1)));
                    Ok(self
                        .diagnostic_type_node(types[types.len() - 1], context, consumer)?
                        .map(|last| {
                            nodes.push(last);
                            nodes
                        }))
                })?;
                break;
            }
            nodes = nodes.and_then(|mut nodes| {
                context.length += 2;
                Ok(self
                    .diagnostic_type_node(ty, context, consumer)?
                    .map(|node| {
                        if !context.flags.has(1 << 6)
                            && let Some(name) = node.reference_name()
                        {
                            if let Some((_, values)) =
                                named.iter_mut().find(|(prior, _)| prior == name)
                            {
                                values.push((ty, nodes.len()));
                            } else {
                                named.push((name.to_owned(), vec![(ty, nodes.len())]));
                            }
                        }
                        nodes.push(node);
                        nodes
                    }))
            })?;
        }
        let flags = context.flags;
        context.flags.0 |= 1 << 6;
        let result = (|| {
            for (_, entries) in named {
                nodes = nodes.and_then(|output| {
                    let mut same = true;
                    for &(ty, _) in entries.iter().skip(1) {
                        if !self.diagnostic_same_reference(entries[0].0, ty)? {
                            same = false;
                            break;
                        }
                    }
                    let mut completed = NativeCompletion::clean(output);
                    if !same {
                        for (ty, index) in entries {
                            completed = completed.and_then(|mut output| {
                                Ok(self
                                    .diagnostic_type_node(ty, context, consumer)?
                                    .map(|node| {
                                        output[index] = node;
                                        output
                                    }))
                            })?;
                        }
                    }
                    Ok(completed)
                })?;
            }
            Ok(nodes)
        })();
        context.flags = flags;
        result
    }

    /// `typesAreSameReference` uses Type.symbol and actual shared alias object
    /// identity, never displayed name or deterministic identity-order ranks.
    fn diagnostic_same_reference(&self, left: TypeId, right: TypeId) -> Result<bool, DemandError> {
        if left == right {
            return Ok(true);
        }
        if let Some(left) = self.type_symbol_identity(left)?
            && Some(left) == self.type_symbol_identity(right)?
        {
            return Ok(true);
        }
        Ok(
            match (
                self.model.metadata(left)?.alias,
                self.model.metadata(right)?.alias,
            ) {
                (Some(left), Some(right)) => std::sync::Arc::ptr_eq(&left, &right),
                _ => false,
            },
        )
    }

    /// The ordinary `conditionalTypeToTypeNode` branch restores the extends
    /// infer context before acquiring and rendering true, then false. Each
    /// recovered branch remains renderable and precedes the next acquisition.
    fn diagnostic_conditional(
        &mut self,
        ty: TypeId,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        if context.truncating() {
            context.elide();
            return Ok(NativeCompletion::clean(context.elision(None)));
        }
        let TypeView::Conditional(view) = self.model.get(ty)? else {
            return Err(super::super::internal("conditional display lacks type header").into());
        };
        self.diagnostic_type_node(view.check_type, context, consumer)?
            .and_then(|check| {
                context.length += 15;
                let previous = std::mem::replace(
                    &mut context.infer_parameters,
                    self.conditional_parameters(ty)?,
                );
                let result = self.diagnostic_type_node(view.extends_type, context, consumer);
                context.infer_parameters = previous;
                result?.and_then(|extends| {
                    self.conditional_true_type_completion(ty, consumer)?
                        .and_then(|true_type| {
                            self.diagnostic_conditional_branch(true_type, context, consumer)?
                                .and_then(|yes| {
                                    self.conditional_false_type_completion(ty, consumer)?
                                        .and_then(|false_type| {
                                            Ok(self
                                                .diagnostic_conditional_branch(
                                                    false_type, context, consumer,
                                                )?
                                                .map(|no| {
                                                    Node::Conditional(Box::new([
                                                        check, extends, yes, no,
                                                    ]))
                                                }))
                                        })
                                })
                        })
                })
            })
    }
    fn diagnostic_conditional_branch(
        &mut self,
        ty: TypeId,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        if self.model.has_type_flags(ty, TypeFlags::UNION) {
            if context.visited.contains(&ty) {
                context.elide();
                Ok(NativeCompletion::clean(context.elision(None)))
            } else {
                self.diagnostic_visit(ty, context, consumer, Self::diagnostic_type_node)
            }
        } else {
            self.diagnostic_type_node(ty, context, consumer)
        }
    }

    fn diagnostic_reference(
        &mut self,
        ty: TypeId,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        let TypeView::Reference(reference) = self.model.get(ty)? else {
            return Err(super::super::internal("reference display lacks type header").into());
        };
        self.get_type_arguments_completion(ty)?.and_then(|arguments| {
        match self.reference_display(ty)? {
            ReferenceDisplay::Array { readonly } => {
                let argument = *arguments.first().ok_or_else(|| super::super::internal("array display lacks element"))?;
                Ok(self.diagnostic_type_node(argument, context, consumer)?.map(|element| {
                    if context.flags.has(1 << 1) { Node::Reference(if readonly { "ReadonlyArray" } else { "Array" }.to_owned(), vec![element]) }
                    else { let node = Node::Array(Box::new(element)); if readonly { Node::Prefix("readonly ", Box::new(node)) } else { node } }
                }))
            }
            ReferenceDisplay::Tuple => {
                let arguments = self.tuple_arguments_for_display_from(ty, &arguments)?;
                let arity = self.type_reference_arity(ty)?;
                self.diagnostic_type_list(&arguments[..arity], false, context, consumer)?.and_then_value(|nodes| {
                    let tuple = reference.tuple.ok_or_else(|| super::super::internal("tuple display lacks target"))?;
                    let mut elements = Vec::new();
                    for (node, info) in nodes.into_iter().zip(tuple.elements.iter()) {
                        use super::super::model::TupleElementFlags as F;
                        let name = info.label.as_ref().map(|label| label.name.clone().map(|name| name.to_string())).transpose()?;
                        elements.push(Element { name, optional: info.flags.intersects(F::OPTIONAL), rest: info.flags.intersects(F::VARIABLE), ty: if info.flags.intersects(F::REST) { Node::Array(Box::new(node)) } else { node } });
                    }
                    let node = Node::Tuple(elements);
                    Ok(if tuple.readonly { Node::Prefix("readonly ", Box::new(node)) } else { node })
                })
            }
            ReferenceDisplay::Ordinary => {
                let target = self.model.declared_target(reference.target)?;
                let outer = target.outer_parameters.len();
                if outer != 0 && arguments[..outer] != *target.outer_parameters { return Err(self.diagnostic_dependency(consumer, "native diagnostic outer-parameter symbol-chain serialization is pending")); }
                let count = self.reference_display_argument_count(ty, &arguments)?;
                self.diagnostic_type_list(&arguments[outer..count], false, context, consumer)?.and_then(|nodes| {
                    let symbol = self.model.bound_type_symbol(ty)?.ok_or_else(|| self.diagnostic_dependency(consumer, "native reference diagnostic symbol is unavailable"))?;
                    let saved = context.flags;
                    context.flags.0 |= 1 << 4;
                    let result = self.symbol_type_node(symbol.into(), super::SymbolMeaning::Type, (!nodes.is_empty()).then_some(nodes), context, consumer);
                    context.flags = saved;
                    result
                })
            }
        }
        })
    }

    /// `createTypeNodeFromObjectType` keeps resolved recovery members usable
    /// throughout serialization. Consumed inventory failures accompany the
    /// resulting node; only the query boundary admits the final completion.
    fn diagnostic_object(
        &mut self,
        ty: TypeId,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        if self.mapped_uses_syntax(ty, consumer)? {
            return self.diagnostic_mapped_type(ty, context, consumer);
        }
        self.structured_type_members_completion(ty, consumer)?
            .and_then(|members| {
                let members = members.ok_or_else(|| {
                    super::super::internal("object display lacks structured members")
                })?;
                if members.properties.is_empty() && members.index_signatures.is_empty() {
                    if members.call_signatures.is_empty() && members.construct_signatures.is_empty()
                    {
                        context.length += 2;
                        return Ok(NativeCompletion::clean(Node::Object(Vec::new())));
                    }
                    if members.call_signatures.len() == 1 && members.construct_signatures.is_empty()
                    {
                        return self
                            .diagnostic_signature(&members.call_signatures[0], context, consumer)
                            .map(|completed| {
                                completed
                                    .map(|signature| Node::Function(Box::new(signature), false))
                            });
                    }
                    if members.construct_signatures.len() == 1 && members.call_signatures.is_empty()
                    {
                        return self
                            .diagnostic_signature(
                                &members.construct_signatures[0],
                                context,
                                consumer,
                            )
                            .map(|completed| {
                                completed.map(|signature| Node::Function(Box::new(signature), true))
                            });
                    }
                }
                let abstracts: Vec<_> = members
                    .construct_signatures
                    .iter()
                    .filter(|signature| signature.flags.has_abstract())
                    .cloned()
                    .collect();
                if !abstracts.is_empty() {
                    let mut types = Vec::new();
                    for signature in &abstracts {
                        types.push(self.type_from_signature(signature, consumer)?);
                    }
                    if members.properties.len()
                        + members.index_signatures.len()
                        + members.call_signatures.len()
                        + members.construct_signatures.len()
                        - abstracts.len()
                        != 0
                    {
                        types.push(
                            self.model.structured(
                                TypeSymbol::Absent,
                                MembersView {
                                    properties: members.properties.clone(),
                                    call_signatures: members.call_signatures.clone(),
                                    index_signatures: members.index_signatures.clone(),
                                    construct_signatures: members
                                        .construct_signatures
                                        .iter()
                                        .filter(|signature| !signature.flags.has_abstract())
                                        .cloned()
                                        .collect::<Vec<_>>()
                                        .into(),
                                },
                                Default::default(),
                                ObjectKind::Anonymous,
                            ),
                        );
                    }
                    let intersection = self.intersection_type(types, consumer)?;
                    return self.diagnostic_type_node(intersection, context, consumer);
                }
                let saved_flags = context.flags;
                context.flags.0 |= 1 << 22;
                let result = (|| {
                    let mut output = NativeCompletion::clean(Vec::new());
                    if context.truncating() {
                        if context.limit != 1_000_000 {
                            output = output.map(|mut output| {
                                output.push(Member::Elision("...".to_owned()));
                                output
                            });
                        }
                    } else {
                        for signature in members.call_signatures.iter() {
                            output = output.and_then(|mut output| {
                                Ok(self
                                    .diagnostic_signature(signature, context, consumer)?
                                    .map(|signature| {
                                        output.push(Member::Signature {
                                            kind: SignatureKind::Call,
                                            name: String::new(),
                                            optional: false,
                                            signature,
                                        });
                                        output
                                    }))
                            })?;
                        }
                        for signature in members.construct_signatures.iter() {
                            if !signature.flags.has_abstract() {
                                output = output.and_then(|mut output| {
                                    Ok(self
                                        .diagnostic_signature(signature, context, consumer)?
                                        .map(|signature| {
                                            output.push(Member::Signature {
                                                kind: SignatureKind::Construct,
                                                name: String::new(),
                                                optional: false,
                                                signature,
                                            });
                                            output
                                        }))
                                })?;
                            }
                        }
                        for index in members.index_signatures.iter() {
                            output = output.and_then(|mut output| {
                                self.demand_completion(index.key_type)?
                                    .and_then(|key| {
                                        self.diagnostic_type_node(key, context, consumer)
                                    })?
                                    .and_then(|key| {
                                        let value = if self.is_reverse_mapped_type(ty)? {
                                            NativeCompletion::clean(context.elision(None))
                                        } else {
                                            self.demand_completion(index.value_type)?.and_then(
                                                |value| {
                                                    self.diagnostic_type_node(
                                                        value, context, consumer,
                                                    )
                                                },
                                            )?
                                        };
                                        Ok(value.map(|value| {
                                            context.length += index.parameter_name.len()
                                                + 4
                                                + if index.readonly { 9 } else { 0 };
                                            output.push(Member::Index {
                                                name: index.parameter_name.clone(),
                                                readonly: index.readonly,
                                                key,
                                                value,
                                            });
                                            output
                                        }))
                                    })
                            })?;
                        }
                        for (index, property) in members.properties.iter().enumerate() {
                            if context.truncating() && index + 3 < members.properties.len() - 1 {
                                output = output.and_then(|mut output| {
                                    if context.limit != 1_000_000 {
                                        output.push(Member::Elision(format!(
                                            "... {} more ...",
                                            members.properties.len() - index - 1
                                        )));
                                    }
                                    Ok(self
                                        .diagnostic_property(
                                            &members.properties[members.properties.len() - 1],
                                            context,
                                            consumer,
                                        )?
                                        .map(|properties| {
                                            output.extend(properties);
                                            output
                                        }))
                                })?;
                                break;
                            }
                            output = output.and_then(|mut output| {
                                Ok(self.diagnostic_property(property, context, consumer)?.map(
                                    |properties| {
                                        output.extend(properties);
                                        output
                                    },
                                ))
                            })?;
                        }
                    }
                    Ok(output.map(|output| {
                        context.length += 2;
                        Node::Object(output)
                    }))
                })();
                context.flags = saved_flags;
                result
            })
    }

    fn diagnostic_property(
        &mut self,
        property: &Property,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Vec<Member>> {
        let placeholder = property
            .symbol
            .map(|symbol| {
                self.reverse_mapped_property_placeholder(symbol, &context.reverse_mapped_stack)
            })
            .transpose()?
            .unwrap_or(false);
        self.prepare_property_display_name_completion(property, placeholder, context, consumer)?
            .and_then(|prepared| {
                let name = prepared.name.clone();
                if prepared.read.is_none() {
                    if property.readonly {
                        context.length += 9;
                    }
                    return Ok(NativeCompletion::clean(vec![Member::Property {
                        name,
                        optional: property.optional,
                        readonly: property.readonly,
                        ty: context.elision(None),
                    }]));
                }
                self.finish_property_display_completion(prepared)?
                    .and_then(|display| match display {
                        PropertyDisplay::Plain { value_type } => {
                            let reverse = property
                                .symbol
                                .map(|symbol| self.is_reverse_mapped_property(symbol))
                                .transpose()?
                                .unwrap_or(false);
                            if reverse {
                                context
                                    .reverse_mapped_stack
                                    .push(property.symbol.expect("reverse property symbol"));
                            }
                            let result = self.demand_completion(value_type).and_then(|value| {
                                value.and_then(|value| {
                                    if let Some(symbol) = property.symbol {
                                        self.diagnostic_property_type(
                                            symbol, value, context, consumer,
                                        )
                                    } else {
                                        self.diagnostic_type_node(value, context, consumer)
                                    }
                                })
                            });
                            if reverse {
                                context.reverse_mapped_stack.pop();
                            }
                            Ok(result?.map(|ty| {
                                if property.readonly {
                                    context.length += 9;
                                }
                                vec![Member::Property {
                                    name,
                                    optional: property.optional,
                                    readonly: property.readonly,
                                    ty,
                                }]
                            }))
                        }
                        PropertyDisplay::Methods { signatures } => {
                            let mut output = NativeCompletion::clean(Vec::new());
                            for signature in signatures.iter() {
                                output = output.and_then(|mut output| {
                                    Ok(self
                                        .diagnostic_signature(signature, context, consumer)?
                                        .map(|signature| {
                                            output.push(Member::Signature {
                                                kind: SignatureKind::Method,
                                                name: name.clone(),
                                                optional: property.optional,
                                                signature,
                                            });
                                            output
                                        }))
                                })?;
                            }
                            Ok(output)
                        }
                        PropertyDisplay::Accessors(accessors) => {
                            let mut output = NativeCompletion::clean(Vec::new());
                            for (signature, kind) in [
                                (accessors.getter.as_ref(), SignatureKind::Get),
                                (accessors.setter.as_ref(), SignatureKind::Set),
                            ] {
                                if let Some(signature) = signature {
                                    output = output.and_then(|mut output| {
                                        Ok(self
                                            .diagnostic_signature(signature, context, consumer)?
                                            .map(|signature| {
                                                output.push(Member::Signature {
                                                    kind,
                                                    name: name.clone(),
                                                    optional: false,
                                                    signature,
                                                });
                                                output
                                            }))
                                    })?;
                                }
                            }
                            Ok(output)
                        }
                    })
            })
    }

    fn prepare_property_display_name_completion(
        &mut self,
        property: &Property,
        placeholder: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<PropertyDisplayPreparation> {
        let read = if placeholder {
            NativeCompletion::clean(None)
        } else if let Some(symbol) = property.symbol {
            self.non_missing_property_type_completion(symbol, consumer)?
                .map(Some)
        } else {
            self.demand_completion(property.read_type)?.map(Some)
        };
        read.and_then(|read| {
            self.property_display_name_completion(property, context, consumer, false)?
                .and_then_value(|name| {
                    let name = name.ok_or_else(|| {
                        super::super::internal("completed property name remains deferred")
                    })?;
                    context.length += if let Some(symbol) = property.symbol {
                        if let TypeKey::Private { name, .. } = &property.key {
                            name.len() + 1
                        } else {
                            self.property_symbol_name_facts(symbol, consumer)?
                                .key
                                .display_length()
                                + 1
                        }
                    } else {
                        match &property.key {
                            TypeKey::String { name }
                            | TypeKey::Private { name, .. }
                            | TypeKey::Symbol { name, .. } => name.len() + 1,
                        }
                    };
                    Ok(PropertyDisplayPreparation {
                        name,
                        read,
                        display: property.display,
                    })
                })
        })
    }

    /// The mode continuation consumes the retained read exactly once; it is
    /// shared by diagnostics and Q4 after their respective admission gates.
    pub(in crate::project::semantic) fn finish_property_display_completion(
        &mut self,
        prepared: PropertyDisplayPreparation,
    ) -> NativeResult<PropertyDisplay> {
        let read = prepared
            .read
            .ok_or_else(|| super::super::internal("placeholder property has no mode question"))?;
        match prepared.display {
            PropertyDisplayDemand::Native(id) => {
                self.serialize_property_after_read_completion(id, read)
            }
            PropertyDisplayDemand::Value(_) => {
                Ok(NativeCompletion::clean(PropertyDisplay::Plain {
                    value_type: super::super::model::TypeDemand::Known(read),
                }))
            }
        }
    }

    /// Nonforcing spelling observation for Q4's property-row admission. The
    /// same native worker stops before computed-name and unique-symbol reads.
    pub(in crate::project::semantic) fn preview_property_name(
        &mut self,
        property: &Property,
    ) -> Result<Option<String>, DemandError> {
        let PropertyDisplayDemand::Native(id) = property.display else {
            return Ok(Some(property_key_name(&property.key)));
        };
        let consumer = self.property_serialization_consumer(id)?;
        let mut context = Context::new(
            DiagnosticTypeFormat::DEFAULT,
            None,
            self.inputs.semantic_options().no_error_truncation,
        );
        self.property_display_name(property, &mut context, &consumer, true)
    }

    /// Q4 enters this native question only after its row lower bound fits.
    /// getNonMissingTypeOfSymbol precedes the name, which precedes the later
    /// accessor/method decision. Placeholder reads are the native exception.
    pub(in crate::project::semantic) fn prepare_summary_property_completion(
        &mut self,
        property: &Property,
        placeholder: bool,
    ) -> NativeResult<PropertyDisplayPreparation> {
        let consumer = match property.display {
            PropertyDisplayDemand::Native(id) => self.property_serialization_consumer(id)?,
            PropertyDisplayDemand::Value(_) => {
                return Err(super::super::internal(
                    "native property preparation requires a retained serialization question",
                )
                .into());
            }
        };
        let mut context = Context::new(
            DiagnosticTypeFormat::DEFAULT,
            None,
            self.inputs.semantic_options().no_error_truncation,
        );
        Ok(self
            .prepare_property_display_name_completion(
                property,
                placeholder,
                &mut context,
                &consumer,
            )
            .map_err(|failure| super::super::observe_recovery(self.inputs, failure, &consumer))?
            .observe(|failure| super::super::observe_recovery(self.inputs, failure, &consumer)))
    }

    /// Q4 observes source-free recovery at this property's retained native
    /// serialization question. Diagnostics keep the same continuation raw
    /// until their own outer display operation observes it.
    pub(in crate::project::semantic) fn finish_summary_property_completion(
        &mut self,
        prepared: PropertyDisplayPreparation,
    ) -> NativeResult<PropertyDisplay> {
        let PropertyDisplayDemand::Native(id) = prepared.display else {
            return Err(super::super::internal(
                "summary property continuation lacks native question",
            )
            .into());
        };
        let consumer = self.property_serialization_consumer(id)?;
        Ok(self
            .finish_property_display_completion(prepared)
            .map_err(|failure| super::super::observe_recovery(self.inputs, failure, &consumer))?
            .observe(|failure| super::super::observe_recovery(self.inputs, failure, &consumer)))
    }

    /// `addPropertyToElementList` installs the property's actual enclosing
    /// declaration just for its name. Restoration also runs after failure.
    fn property_display_name(
        &mut self,
        property: &Property,
        context: &mut Context,
        consumer: &Location,
        preview: bool,
    ) -> Result<Option<String>, DemandError> {
        self.property_display_name_completion(property, context, consumer, preview)?
            .admit()
    }

    fn property_display_name_completion(
        &mut self,
        property: &Property,
        context: &mut Context,
        consumer: &Location,
        preview: bool,
    ) -> NativeResult<Option<String>> {
        let saved = context.enclosing;
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            context.enclosing = None;
            if let Some(symbol) = property.symbol {
                // `isLateBoundName` is a native name question, not CheckFlagsLate.
                // Its generated-name producer must provide tracking before
                // installing the property's temporary declaration scope.
                if self
                    .reference_symbol_key(super::ReferenceSymbol::from_property(symbol), consumer)?
                    .is_known_symbol()
                {
                    if preview {
                        return Ok(None);
                    }
                    let declarations = self.property_reference_declarations(symbol, consumer)?;
                    if let Some(declaration) = declarations.first() {
                        if let super::ReferenceDeclaration::Captured(declaration) = declaration
                            && let Some(name) = self.source.pseudo_child(*declaration, "name")?
                            && sequence.read(self.is_late_bindable_name_completion(name))?
                        {
                            // `addPropertyToElementList` tracks the first late
                            // declaration at the saved target scope, before
                            // installing ValueDeclaration for name emission.
                            let expression = self
                                .source
                                .pseudo_child(name, "expression")?
                                .ok_or_else(|| {
                                    super::super::internal("late property name has no expression")
                                })?;
                            sequence.read(self.diagnostic_track_computed_name_completion(
                                expression, saved, context, consumer,
                            ))?;
                        }
                    } else {
                        sequence.read(
                            self.raw_diagnostic_property_symbol_display_completion(
                                symbol, consumer,
                            ),
                        )?;
                        context.report_diagnostic();
                    }
                }
                let declaration =
                    match self.property_reference_value_declaration(symbol, consumer)? {
                        Some(declaration) => Some(declaration),
                        None => self
                            .property_reference_declarations(symbol, consumer)?
                            .first()
                            .copied(),
                    };
                context.enclosing = match declaration {
                    Some(super::ReferenceDeclaration::Captured(declaration)) => {
                        Some(declaration.into())
                    }
                    _ => saved,
                };
            } else {
                context.enclosing = saved;
            }
            sequence.read(self.property_name_worker(property, context, consumer, preview))
        })();
        context.enclosing = saved;
        sequence.finish(result)
    }

    /// `getPropertyNameNodeForSymbol` / `isStringNamed`: Every short-circuits
    /// at the first non-string declaration, including an absent name. Preview
    /// returns None at a semantic name dependency, never a guessed spelling.
    fn property_name_worker(
        &mut self,
        property: &Property,
        context: &mut Context,
        consumer: &Location,
        preview: bool,
    ) -> NativeResult<Option<String>> {
        let Some(symbol) = property.symbol else {
            return Ok(NativeCompletion::clean(Some(property_key_name(
                &property.key,
            ))));
        };
        if let Some(super::ReferenceDeclaration::Captured(declaration)) =
            self.property_reference_value_declaration(symbol, consumer)?
            && self
                .source
                .declaration_name_site(declaration)?
                .is_some_and(|(_, site)| {
                    site.kind == crate::syntax_kind::SyntaxKind::PrivateIdentifier
                })
        {
            let name = self
                .source
                .private_identifier_declaration_name(declaration)?
                .ok_or_else(|| super::super::internal("private declaration lost its exact name"))?;
            return Ok(NativeCompletion::clean(Some(name)));
        }
        let declarations = self.property_reference_declarations(symbol, consumer)?;
        let mut string_named = NativeCompletion::clean(!declarations.is_empty());
        for &declaration in &declarations {
            if preview
                && *string_named.value()
                && let super::ReferenceDeclaration::Captured(declaration) = declaration
                && let Some((_, name)) = self.source.declaration_name_site(declaration)?
                && matches!(
                    name.kind,
                    crate::syntax_kind::SyntaxKind::ComputedPropertyName
                        | crate::syntax_kind::SyntaxKind::ElementAccessExpression
                )
            {
                return Ok(string_named.map(|_| None));
            }
            string_named = string_named.and_then(|current| {
                if !current {
                    return Ok(NativeCompletion::clean(false));
                }
                let declaration = match declaration {
                    super::ReferenceDeclaration::Captured(declaration) => declaration,
                    super::ReferenceDeclaration::Json {
                        declaration: crate::project::binding::JsonDeclaration::Property(_),
                        ..
                    } => return Ok(NativeCompletion::clean(true)),
                    super::ReferenceDeclaration::Json { .. } => {
                        return Ok(NativeCompletion::clean(false));
                    }
                };
                let Some((file, name)) = self.source.declaration_name_site(declaration)? else {
                    return Ok(NativeCompletion::clean(false));
                };
                match name.kind {
                    crate::syntax_kind::SyntaxKind::ComputedPropertyName
                    | crate::syntax_kind::SyntaxKind::ElementAccessExpression => {
                        if preview {
                            return Err(self.diagnostic_dependency(
                                consumer,
                                "preview computed name must defer before acquisition",
                            ));
                        }
                        match self.source.property_name_recipe(file, name)?.form {
                            super::super::source::PropertyNameForm::Expression {
                                expression,
                                ..
                            } => Ok(self
                                .check_expression_completion(
                                    &expression,
                                    super::contextual::CheckMode::NORMAL,
                                )?
                                .map(|ty| self.model.has_type_flags(ty, TypeFlags::STRING_LIKE))),
                            _ => Ok(NativeCompletion::clean(false)),
                        }
                    }
                    crate::syntax_kind::SyntaxKind::StringLiteral => {
                        Ok(NativeCompletion::clean(true))
                    }
                    _ => Ok(NativeCompletion::clean(false)),
                }
            })?;
        }
        string_named.and_then(|string_named| {
            let mut sequence = super::super::model::NativeSequence::new();
            let result = (|| {
                let mut single_quote = !declarations.is_empty();
                for &declaration in &declarations {
                    let super::ReferenceDeclaration::Captured(declaration) = declaration else {
                        single_quote = false;
                        break;
                    };
                    if !self.source.diagnostic_single_quoted_name(declaration)? {
                        single_quote = false;
                        break;
                    }
                }
                let method = self.property_symbol_flags(symbol)?.method;
                let facts = self.property_symbol_name_facts(symbol, consumer)?;
                if let Some(name_type) = facts.name_type {
                    match self.model.get(name_type)? {
                        TypeView::Literal {
                            base: PrimitiveName::String | PrimitiveName::Number,
                            value,
                        } => {
                            let value = self.literal_scalar_text(&value, consumer)?;
                            let identifier = crate::lexer::is_identifier_text(value);
                            if !identifier
                                && (string_named || !crate::numbers::is_numeric_literal_name(value))
                            {
                                return Ok(Some(crate::printer::quoted_text(
                                    value,
                                    if single_quote { '\'' } else { '"' },
                                )));
                            }
                            if crate::numbers::is_numeric_literal_name(value)
                                && value.starts_with('-')
                            {
                                return Ok(Some(format!("[{value}]")));
                            }
                            return Ok(Some(diagnostic_literal_name(
                                value,
                                single_quote,
                                string_named,
                                method,
                            )));
                        }
                        _ if self
                            .model
                            .has_type_flags(name_type, TypeFlags::UNIQUE_SYMBOL) =>
                        {
                            if preview {
                                return Ok(None);
                            }
                            let symbol = self
                                .reference_symbol_of_type(name_type, consumer)?
                                .ok_or_else(|| {
                                    super::super::internal("unique name type has no symbol")
                                })?;
                            let expression = sequence.read(self.symbol_expression_node(
                                symbol,
                                super::SymbolMeaning::Value,
                                context,
                                consumer,
                            ))?;
                            let text = String::from_utf8(expression.print()).map_err(|_| {
                                super::super::internal("untruncated expression printer lost UTF-8")
                            })?;
                            return Ok(Some(format!("[{text}]")));
                        }
                        _ => {}
                    }
                }
                let name = self.property_lookup_key(symbol, consumer)?;
                let value = super::super::model::LiteralValue::from_property_key(&name);
                let name = self.literal_scalar_text(&value, consumer)?;
                Ok(Some(diagnostic_literal_name(
                    name,
                    single_quote,
                    string_named,
                    method,
                )))
            })();
            sequence.finish(result)
        })
    }

    fn diagnostic_signature(
        &mut self,
        signature: &Signature,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<syntax::Signature> {
        self.expanded_parameters_completion(signature, true, consumer)?
            .and_then(|expanded| {
                let expanded = expanded.first().ok_or_else(|| {
                    super::super::internal("signature expansion has no parameter list")
                })?;
                let declaration = self
                    .signature_declaration_root(signature.id)?
                    .map(|root| self.source.diagnostic_signature_declaration(root))
                    .transpose()?;
                self.diagnostic_scope(
                    context,
                    declaration,
                    expanded,
                    signature.type_parameter_types(),
                    Some(&signature.parameters),
                    self.signature_mapper(signature),
                    consumer,
                    |checker, context| {
                        context.length += 3;
                        let mut type_parameters = NativeCompletion::clean(Vec::new());
                        for &parameter in signature.type_parameter_types().iter() {
                            type_parameters = type_parameters.and_then(|mut parameters| {
                                let constraint = checker
                                    .parameter_constraint_completion(parameter, consumer)?
                                    .and_then(|constraint| {
                                        let Some(constraint) = constraint else {
                                            return Ok(NativeCompletion::clean(None));
                                        };
                                        let node =
                                            checker.constraint_declaration(parameter, consumer)?;
                                        Ok(checker
                                            .diagnostic_type_node_with_reuse(
                                                constraint, node, context, consumer,
                                            )?
                                            .map(|node| Some(Box::new(node))))
                                    })?;
                                constraint.and_then(|constraint| {
                                    Ok(checker
                                        .diagnostic_parameter_declaration(
                                            parameter, constraint, context, consumer,
                                        )?
                                        .map(|parameter| {
                                            parameters.push(parameter);
                                            parameters
                                        }))
                                })
                            })?;
                        }
                        type_parameters.and_then(|type_parameters| {
                            let parameters =
                                Self::signature_display_parameters(signature, expanded);
                            let mut nodes = NativeCompletion::clean(Vec::new());
                            for parameter in parameters {
                                nodes = nodes.and_then(|mut nodes| {
                                    Ok(checker
                                        .diagnostic_parameter(parameter, context, consumer)?
                                        .map(|node| {
                                            nodes.push(node);
                                            nodes
                                        }))
                                })?;
                            }
                            if !context.flags.has(1 << 25)
                                && let Some(parameter) = signature.this_parameter()
                            {
                                nodes = nodes.and_then(|mut nodes| {
                                    Ok(checker
                                        .diagnostic_parameter(parameter, context, consumer)?
                                        .map(|node| {
                                            nodes.insert(0, node);
                                            nodes
                                        }))
                                })?;
                            }
                            nodes.and_then(|parameters| {
                                Ok(checker
                                    .diagnostic_signature_return(
                                        signature, true, context, consumer,
                                    )?
                                    .map(|returns| syntax::Signature {
                                        type_parameters,
                                        parameters,
                                        returns,
                                        abstract_: signature.flags.has_abstract(),
                                    }))
                            })
                        })
                    },
                )
            })
    }

    fn diagnostic_parameter(
        &mut self,
        parameter: &ParameterValue,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Element> {
        let declaration = self.diagnostic_effective_parameter_declaration(parameter, consumer)?;
        self.property_raw_type_completion(parameter.symbol, consumer)?
            .and_then(|ty| {
                self.diagnostic_declaration_type(
                    declaration.map(super::ReferenceDeclaration::Captured),
                    Some(ty),
                    Some(parameter.symbol),
                    true,
                    context,
                    consumer,
                )?
                .and_then(|node| {
                    let name = if let Some(declaration) = declaration {
                        match self.source.pseudo_child(declaration, "name")? {
                            Some(name) => {
                                self.diagnostic_parameter_name(name, context, consumer)?
                            }
                            None => NativeCompletion::clean(parameter.name.clone()),
                        }
                    } else {
                        NativeCompletion::clean(parameter.name.clone())
                    };
                    name.and_then(|name| {
                        self.diagnostic_optional_parameter(declaration, consumer)?
                            .and_then_value(|optional| {
                                let flags = self.property_check_flags(parameter.symbol)?;
                                let optional = optional
                                    || flags & super::property_symbols::checks::OPTIONAL_PARAMETER
                                        != 0;
                                let rest = parameter.rest
                                    || flags & super::property_symbols::checks::REST_PARAMETER != 0;
                                context.length += parameter.name.len() + 3;
                                Ok(Element {
                                    name: Some(name),
                                    optional,
                                    rest,
                                    ty: node,
                                })
                            })
                    })
                })
            })
    }

    fn diagnostic_predicate_node(
        &mut self,
        predicate: &super::super::model::Predicate,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        use crate::types::{TypePredicateKind, TypePredicateSubject};
        let name = match predicate.subject {
            TypePredicateSubject::This(_) => "this".to_owned(),
            TypePredicateSubject::ParameterIndex(_) => predicate
                .parameter_name
                .as_ref()
                .ok_or_else(|| {
                    super::super::internal("predicate display lacks actual parameter name")
                })?
                .to_string(),
        };
        let ty = if let Some(ty) = predicate.value_type {
            self.demand_completion(ty)?.and_then(|mut ty| {
                if let Some(mapper) = context.mapper {
                    ty = self.instantiate_type(ty, mapper, consumer)?;
                }
                Ok(self
                    .diagnostic_type_node(ty, context, consumer)?
                    .map(|node| Some(Box::new(node))))
            })?
        } else {
            NativeCompletion::clean(None)
        };
        Ok(ty.map(|ty| Node::Predicate {
            asserts: !matches!(predicate.kind, TypePredicateKind::Is),
            name,
            ty,
        }))
    }
    /// `signatureToString` is a fresh node-builder scope, without the type
    /// entry's serialization-level increment or final byte truncation.
    pub(super) fn diagnostic_signature_display_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<RenderedTypeName> {
        Ok(self
            .raw_diagnostic_signature_display_completion(signature, consumer)
            .map_err(|failure| super::super::observe_recovery(self.inputs, failure, consumer))?
            .observe(|failure| super::super::observe_recovery(self.inputs, failure, consumer)))
    }

    pub(super) fn raw_diagnostic_signature_display_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<RenderedTypeName> {
        let mut context = Context::new(DiagnosticTypeFormat(0), None, false);
        context.flags.0 |= 1 << 9;
        let node = self.diagnostic_signature(signature, &mut context, consumer)?;
        Ok(node.map(|node| RenderedTypeName(node.print(signature.flags.is_construct()))))
    }

    pub(super) fn raw_diagnostic_predicate_display_completion(
        &mut self,
        predicate: &super::super::model::Predicate,
        consumer: &Location,
    ) -> NativeResult<RenderedTypeName> {
        let mut context = Context::new(DiagnosticTypeFormat(1 << 14), None, false);
        context.flags.0 |= 1 << 9;
        Ok(self
            .diagnostic_predicate_node(predicate, &mut context, consumer)?
            .map(|node| RenderedTypeName(node.print())))
    }
}

/// The actual key of an already constructed symbol-less property needs no
/// declaration or nameType acquisition. Partial displays also retain this key.
pub(in crate::project::semantic) fn property_key_name(key: &TypeKey) -> String {
    match key {
        TypeKey::String { name } => diagnostic_literal_name(name, false, false, false),
        TypeKey::Private { name, .. } | TypeKey::Symbol { name, .. } => name.clone(),
    }
}

/// `classifyPropertyName` is shared by fresh names and existing-node reuse.
fn diagnostic_property_name_kind(
    name: &str,
    string_named: bool,
    method: bool,
) -> crate::syntax_kind::SyntaxKind {
    use crate::syntax_kind::SyntaxKind as K;
    if method && name == "new" {
        return K::StringLiteral;
    }
    if crate::lexer::is_identifier_text(name) {
        return K::Identifier;
    }
    if !string_named
        && crate::numbers::numeric_literal_name_value(name).is_some_and(|value| value >= 0.0)
    {
        K::NumericLiteral
    } else {
        K::StringLiteral
    }
}

/// `createPropertyNameNodeForIdentifierOrLiteral` prints the classified node.
fn diagnostic_literal_name(
    name: &str,
    single_quote: bool,
    string_named: bool,
    method: bool,
) -> String {
    if diagnostic_property_name_kind(name, string_named, method)
        == crate::syntax_kind::SyntaxKind::StringLiteral
    {
        crate::printer::quoted_text(name, if single_quote { '\'' } else { '"' })
    } else {
        name.to_owned()
    }
}

/// `getTypeParameterModifiers` / `CreateModifiersFromModifierFlags` order.
fn diagnostic_parameter_modifiers(parameter: &super::super::model::Parameter) -> String {
    let mut output = String::new();
    if parameter.constant {
        output.push_str("const ");
    }
    if let Some(variance) = parameter.variance {
        output.push_str(variance.as_str());
        output.push(' ');
    }
    output
}
