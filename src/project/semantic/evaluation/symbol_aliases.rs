//! Native checker alias links. Lexical navigation's resolved occurrence is
//! deliberately not a checker alias target: an export assignment can resolve
//! through expression checking to a property that navigation never follows.
use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, ObjectFlags, TypeId, TypeSymbol,
};
use super::super::source::ExpressionRoot;
use super::super::source::symbol_aliases::SymbolAliasTarget;
use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol, TypeState};
use crate::project::binding::{
    NativeSymbolIdentity, ReadOrigin, ReferenceModuleSite, ResolutionMode,
};
use crate::project::program::ModuleKind;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;
use std::collections::HashMap;

#[derive(Clone)]
struct TargetState {
    result: NativeResult<Option<ReferenceSymbol>>,
    installed: bool,
}

#[derive(Default)]
pub(super) struct SymbolAliasesState {
    targets: HashMap<NativeSymbolIdentity, TargetState>,
    unknown: Option<super::super::model::PropertySymbolId>,
    default_only_types: HashMap<TypeId, NativeResult<TypeId>>,
    synthetic_types: HashMap<TypeId, NativeResult<TypeId>>,
    type_only: HashMap<NativeSymbolIdentity, ReferenceDeclaration>,
}
impl SymbolAliasesState {
    pub(super) fn has_target(&self, identity: NativeSymbolIdentity) -> bool {
        self.targets
            .get(&identity)
            .is_some_and(|state| state.installed)
    }
}

impl Evaluator<'_, '_> {
    /// Q4 selection acquires the actual `resolveAlias` target, including
    /// checker-created export= properties. Selection owns its declaration's
    /// syntax independently of propagated alias-chain markers, and owns
    /// the product's declared/value preference; no target type is read here
    /// beyond the native work required to resolve the alias itself.
    pub(in crate::project::semantic) fn selected_alias_target_completion(
        &mut self,
        declaration: super::super::source::DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<ReferenceSymbol> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let source =
                sequence.read(self.reference_symbol_of_declaration_completion(declaration))?;
            let target = sequence
                .read(self.resolve_alias_target_completion(source, consumer))?
                .ok_or_else(|| super::super::internal("selected alias lacks a completed target"))?;
            let symbol = target;
            if self.is_unknown_reference_symbol(symbol) {
                return Err(self.generic_binding_failure(
                    crate::project::binding::TypeBindingFailure::ErrorType(
                        "native alias target is unknown",
                    ),
                    consumer,
                ));
            }
            Ok(symbol)
        })();
        sequence.finish(result)
    }

    /// A native factory's installed `aliasTarget` shares the same identity
    /// keyed links as resolveAlias; it needs no fabricated declaration origin.
    pub(super) fn install_alias_target(
        &mut self,
        alias: ReferenceSymbol,
        target: NativeCompletion<ReferenceSymbol>,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        let identity = self.reference_symbol_identity(alias, consumer)?;
        self.symbol_aliases.targets.insert(
            identity,
            TargetState {
                result: Ok(target.map(Some)),
                installed: true,
            },
        );
        Ok(())
    }

    /// `getTypeOnlyAliasDeclaration` resolves the alias before observing its
    /// nontransitive marker; target-chain traversal belongs getSymbolFlagsEx.
    pub(super) fn reference_alias_is_type_only_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            Ok(sequence
                .read(self.reference_type_only_alias_declaration_completion(symbol, consumer))?
                .is_some())
        })();
        sequence.finish(result)
    }

    /// `getTypeOnlyAliasDeclaration` observes the exact installed declaration
    /// after resolving an actual Alias; ordinary symbols never resolve here.
    fn reference_type_only_alias_declaration_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceDeclaration>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !self.reference_symbol_flags(symbol, consumer)?.alias {
                return Ok(None);
            }
            sequence.read(self.resolve_alias_target_completion(symbol, consumer))?;
            let identity = self.reference_symbol_identity(symbol, consumer)?;
            Ok(self.symbol_aliases.type_only.get(&identity).copied())
        })();
        sequence.finish(result)
    }

    pub(super) fn unknown_reference_symbol(&mut self) -> ReferenceSymbol {
        let property = match self.symbol_aliases.unknown {
            Some(property) => property,
            None => {
                let property = self.create_unknown_symbol();
                self.symbol_aliases.unknown = Some(property);
                property
            }
        };
        ReferenceSymbol::from_property(property)
    }

    pub(super) fn is_unknown_reference_symbol(&self, symbol: ReferenceSymbol) -> bool {
        symbol
            .property()
            .is_some_and(|property| self.symbol_aliases.unknown == Some(property))
    }

    /// `resolveSymbolEx(false)` follows IsNonLocalAlias. A resolved unknown
    /// symbol is nonnil, so it cannot trigger missing-export fallbacks.
    pub(super) fn resolve_reference_symbol_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self
                .reference_symbol_flags(symbol, consumer)?
                .is_non_local_alias()
            {
                sequence.read(self.resolve_alias_target_completion(symbol, consumer))
            } else {
                Ok(Some(symbol))
            }
        })();
        sequence.finish(result)
    }

    /// `tryResolveAlias` observes the existing resolution window before
    /// asking resolveAlias. A currently resolving uninstalled slot is nil,
    /// not an unknownSymbol and not a newly pushed cycle.
    pub(super) fn try_resolve_alias_target_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let identity = self.reference_symbol_identity(symbol, consumer)?;
            if self.symbol_aliases.has_target(identity)
                || self
                    .resolution_cycle_start(super::resolution::Key::AliasTarget(identity))
                    .is_none()
            {
                sequence.read(self.resolve_alias_target_completion(symbol, consumer))
            } else {
                Ok(None)
            }
        })();
        sequence.finish(result)
    }

    /// `resolveAlias` owns AliasTarget's resolution barrier and installed
    /// unknown result; getImmediateAliasedSymbol retains its separate cache.
    pub(super) fn resolve_alias_target_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        if let Some(state) = self.symbol_aliases.targets.get(&identity) {
            return state.result.clone();
        }
        // resolveAlias requires a declaration only when aliasTarget is absent.
        // Synthetic default aliases already have an installed target.
        if self
            .reference_alias_declaration(symbol, consumer)?
            .is_none()
        {
            return Err(super::super::internal(
                "unresolved alias symbol lacks an admitted alias declaration",
            )
            .into());
        }
        if !self.push_resolution(super::resolution::Key::AliasTarget(identity)) {
            return Ok(NativeCompletion::clean(Some(
                self.unknown_reference_symbol(),
            )));
        }
        let result = self
            .checker_alias_target_completion(symbol, consumer)
            .and_then(|target| {
                target.and_then(|target| match target {
                    Some(target)
                        if self
                            .reference_symbol_flags(target, consumer)?
                            .is_non_local_alias() =>
                    {
                        self.resolve_indirection_alias_completion(symbol, target, consumer)
                    }
                    Some(target) => Ok(NativeCompletion::clean(Some(target))),
                    None => Ok(NativeCompletion::clean(Some(
                        self.unknown_reference_symbol(),
                    ))),
                })
            });
        // The native slot is published before pop. A product interruption
        // records the stopped operation but does not install a native target.
        self.symbol_aliases.targets.insert(
            identity,
            TargetState {
                installed: result.is_ok(),
                result: result.clone(),
            },
        );
        let popped = self
            .resolution
            .pop()
            .ok_or_else(|| super::super::internal("alias target resolution frame missing"))?;
        let completed = result?;
        if popped {
            return Ok(completed);
        }
        // Circularity display runs against the published target. Only after
        // that operation completes does native overwrite it with unknown.
        let result = completed
            .and_then(|_| self.diagnostic_symbol_display_completion(symbol, consumer))
            .map(|display| display.map(|_| Some(self.unknown_reference_symbol())));
        self.symbol_aliases.targets.insert(
            identity,
            TargetState {
                installed: true,
                result: result.clone(),
            },
        );
        result
    }

    /// `resolveIndirectionAlias` resolves and merges the pure target before
    /// propagating its type-only marker to the source's actual alias links.
    fn resolve_indirection_alias_completion(
        &mut self,
        source: ReferenceSymbol,
        target: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let result = sequence
                .read(self.resolve_alias_target_completion(target, consumer))?
                .map(|target| self.get_merged_reference_symbol(target, consumer))
                .transpose()?;
            let target_identity = self.reference_symbol_identity(target, consumer)?;
            if let Some(declaration) = self.symbol_aliases.type_only.get(&target_identity).copied()
            {
                let source_identity = self.reference_symbol_identity(source, consumer)?;
                self.symbol_aliases
                    .type_only
                    .entry(source_identity)
                    .or_insert(declaration);
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `getTargetOfAliasDeclaration` uses checker expression/module operations,
    /// not the navigation resolver's projected value target. JSON keeps the
    /// established value handle until a consumer asks for its actual symbol.
    pub(super) fn checker_alias_target_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            // getTargetOfAliasDeclaration(nil) returns nil. In particular,
            // getTypeOfAlias asks this even for declarationless factory aliases.
            // resolveAlias and getImmediateAliasedSymbol own their stricter
            // declaration preconditions when their respective links are absent.
            let Some(declaration) = self.reference_alias_declaration(symbol, consumer)? else {
                return Ok(None);
            };
            let ReferenceDeclaration::Captured(declaration) = declaration else {
                return Err(
                    super::super::internal("alias declaration is not captured syntax").into(),
                );
            };
            let recipe = self.source.symbol_alias_target(declaration)?;
            let internal_import = matches!(&recipe, SymbolAliasTarget::InternalImportEquals(_));
            let result = match recipe {
                SymbolAliasTarget::Expression {
                    root,
                    class,
                    entity,
                } => sequence.read(
                    self.alias_like_expression_target_completion(root, class, entity, consumer),
                ),
                SymbolAliasTarget::External { name } => {
                    let (kind, span) = self.source.diagnostic_lookup_node(declaration)?;
                    let site = self
                        .source
                        .bindings()
                        .reference_alias_module_site_at(declaration.file, kind, span)
                        .map_err(|error| self.generic_binding_failure(error, consumer))?
                        .ok_or_else(|| {
                            super::super::internal("external alias lacks retained module request")
                        })?;
                    let module = self.reference_module_symbol(site, consumer)?;
                    match kind {
                        K::ImportEqualsDeclaration => {
                            sequence.read(self.import_equals_module_target_completion(
                                module,
                                declaration,
                                site,
                                consumer,
                            ))
                        }
                        K::ImportClause => sequence.read(self.module_default_target_completion(
                            module,
                            declaration,
                            site,
                            true,
                            consumer,
                        )),
                        K::ImportSpecifier | K::ExportSpecifier
                            if name.as_deref() == Some("default") =>
                        {
                            sequence.read(self.module_default_target_completion(
                                module,
                                declaration,
                                site,
                                true,
                                consumer,
                            ))
                        }
                        K::ImportSpecifier | K::ExportSpecifier => {
                            sequence.read(self.external_module_member_target_completion(
                                module,
                                declaration,
                                site,
                                name.as_deref().ok_or_else(|| {
                                    super::super::internal("named import lacks imported name")
                                })?,
                                true,
                                consumer,
                            ))
                        }
                        K::NamespaceImport => sequence.read(self.es_module_target_completion(
                            declaration,
                            module,
                            site,
                            true,
                            consumer,
                        )),
                        K::NamespaceExport => sequence.read(self.es_module_target_completion(
                            declaration,
                            module,
                            site,
                            false,
                            consumer,
                        )),
                        _ => Err(super::super::internal("unexpected external alias syntax").into()),
                    }
                }
                SymbolAliasTarget::InternalImportEquals(name) => {
                    let anchor = self.source.reference_entity_name_anchor(name)?;
                    let node = self.source.declaration_from_expression_root(name)?;
                    let (kind, _) = self.source.diagnostic_lookup_node(node)?;
                    let meaning = if kind == K::Identifier {
                        crate::project::binding::NativeLookupMeaning::NAMESPACE
                    } else {
                        crate::project::binding::NativeLookupMeaning::ALL
                    };
                    let (resolved, failure) =
                        sequence.read(self.reference_entity_at_completion(
                            name.file, anchor, false, meaning, true, consumer,
                        ))?;
                    if resolved.is_none() {
                        if let Some(failure) = failure {
                            sequence.read(NativeCompletion::recovered((), failure))?;
                        }
                    }
                    sequence
                        .read(self.check_import_alias_type_only_chain_completion(name, consumer))?;
                    Ok(resolved)
                }
                SymbolAliasTarget::LocalExport(name) => {
                    if let Some(name) = name {
                        let anchor = self.source.reference_entity_name_anchor(name)?;
                        let (symbol, failure) =
                            sequence.read(self.reference_entity_at_completion(
                                name.file,
                                anchor,
                                false,
                                crate::project::binding::NativeLookupMeaning::ALL,
                                true,
                                consumer,
                            ))?;
                        if symbol.is_none()
                            && let Some(failure) = failure
                        {
                            sequence.read(NativeCompletion::recovered((), failure))?;
                        }
                        Ok(symbol)
                    } else {
                        Ok(None)
                    }
                }
                SymbolAliasTarget::NamespaceExport(parent) => {
                    let Some(parent) = parent else {
                        return Ok(None);
                    };
                    let Some(parent) = self.reference_raw_symbol_of_declaration(parent)? else {
                        return Ok(None);
                    };
                    sequence.read(
                        self.reference_external_module_symbol_completion(parent, true, consumer),
                    )
                }
                SymbolAliasTarget::Bound => Err(super::super::internal(
                    "alias declaration lacks native target recipe",
                )
                .into()),
            };
            let result = result?;
            if !internal_import {
                sequence.read(self.mark_alias_type_only_completion(declaration, None, consumer))?;
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `markSymbolOfAliasDeclarationIfTypeOnly`: a present native slot is
    /// never replaced; the declaration's own type-only flag precedes export*.
    fn mark_alias_type_only_completion(
        &mut self,
        declaration: super::super::source::DeclarationIdentity,
        star: Option<ReferenceDeclaration>,
        consumer: &Location,
    ) -> NativeResult<()> {
        self.reference_symbol_of_declaration_completion(declaration)?
            .and_then_value(|symbol| {
                let identity = self.reference_symbol_identity(symbol, consumer)?;
                if self.symbol_aliases.type_only.contains_key(&identity) {
                    return Ok(());
                }
                let (kind, span) = self.source.diagnostic_lookup_node(declaration)?;
                let own = self
                    .inputs
                    .bindings
                    .alias_syntax_is_type_only(declaration.file, kind, span)
                    .map_err(|error| self.generic_binding_failure(error, consumer))?;
                if let Some(marker) = if own {
                    Some(ReferenceDeclaration::Captured(declaration))
                } else {
                    star
                } {
                    self.symbol_aliases.type_only.insert(identity, marker);
                }
                Ok(())
            })
    }

    /// `checkAndReportErrorForResolvingImportAliasToTypeOnlySymbol` checks
    /// the complete entity and then each left prefix, stopping at the first
    /// actual type-only declaration. Reporting adds no type-display demand.
    fn check_import_alias_type_only_chain_completion(
        &mut self,
        mut name: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            loop {
                let anchor = self.source.reference_entity_name_anchor(name)?;
                let (symbol, _) = sequence.read(self.reference_entity_at_completion(
                    name.file,
                    anchor,
                    false,
                    crate::project::binding::NativeLookupMeaning::ALL,
                    true,
                    consumer,
                ))?;
                if let Some(symbol) = symbol
                    && let Some(declaration) = sequence.read(
                        self.reference_type_only_alias_declaration_completion(symbol, consumer),
                    )?
                {
                    let _is_export = matches!(
                        self.reference_declaration_kind(declaration)?,
                        K::ExportSpecifier | K::ExportDeclaration
                    );
                    let ReferenceDeclaration::Captured(declaration) = declaration else {
                        return Err(super::super::internal(
                            "type-only alias declaration is not captured syntax",
                        )
                        .into());
                    };
                    self.source
                        .import_alias_type_only_diagnostic_name(declaration)?;
                    return Ok(());
                }
                let Some(left) = self.source.import_alias_left_entity(name)? else {
                    return Ok(());
                };
                name = left;
            }
        })();
        sequence.finish(result)
    }

    /// `getTargetOfAliasLikeExpression`: entity lookup precedes checking;
    /// an unsuccessful namespace interpretation does not mean no property.
    fn alias_like_expression_target_completion(
        &mut self,
        root: ExpressionRoot,
        class: bool,
        entity: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if class {
                let ty =
                    sequence.read(self.check_expression_cached_completion(root.file, root.site))?;
                return match self.model.type_symbol(ty) {
                    TypeSymbol::Bound { symbol, .. } => Ok(Some(ReferenceSymbol::from_origin(
                        ReadOrigin::Direct(symbol),
                    ))),
                    TypeSymbol::Property { symbol, .. } => {
                        Ok(Some(ReferenceSymbol::from_property(symbol)))
                    }
                    TypeSymbol::Absent => Ok(None),
                    TypeSymbol::Synthetic { .. } => Err(self.access_pending(
                        consumer,
                        "alias class expression has an unsupported synthetic symbol",
                    )),
                };
            }
            if !entity {
                return Ok(None);
            }
            let anchor = self.source.reference_entity_name_anchor(root)?;
            let (symbol, _) = sequence.read(self.reference_entity_at_completion(
                root.file,
                anchor,
                false,
                crate::project::binding::NativeLookupMeaning::ALL,
                true,
                consumer,
            ))?;
            if let Some(symbol) = symbol {
                return Ok(Some(symbol));
            }
            sequence.read(self.check_expression_cached_completion(root.file, root.site))?;
            self.reference_resolved_symbol_if_any(root)
        })();
        sequence.finish(result)
    }

    fn own_module_export(
        &self,
        module: ReferenceSymbol,
        name: &str,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.reference_own_module_export(module, name, consumer)
    }

    /// `resolveExternalModuleSymbol` distinguishes an absent own export= from
    /// its resolved unknown target. A nil resolved target returns the module.
    pub(super) fn reference_external_module_symbol_completion(
        &mut self,
        module: ReferenceSymbol,
        dont_resolve: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(export) = self.reference_own_module_export(module, "export=", consumer)?
            else {
                return Ok(Some(module));
            };
            let Some(export) =
                sequence.read(self.resolve_symbol_ex_completion(export, dont_resolve, consumer))?
            else {
                return Ok(Some(module));
            };
            self.get_merged_reference_symbol(export, consumer).map(Some)
        })();
        sequence.finish(result)
    }

    fn resolve_symbol_ex_completion(
        &mut self,
        symbol: ReferenceSymbol,
        dont_resolve: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = if dont_resolve {
            Ok(Some(symbol))
        } else {
            sequence.read(self.resolve_reference_symbol_completion(symbol, consumer))
        };
        sequence.finish(result)
    }

    /// `getExportOfModule` uses resolved exports and its Module flag gate.
    fn alias_export_of_module_completion(
        &mut self,
        symbol: ReferenceSymbol,
        source: super::super::source::DeclarationIdentity,
        name: &str,
        dont_resolve: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !self.reference_symbol_flags(symbol, consumer)?.module {
                return Ok(None);
            }
            let export = sequence
                .read(self.reference_module_export_symbol_completion(symbol, name, consumer))?;
            let result = match export {
                Some(export) => {
                    sequence.read(self.resolve_symbol_ex_completion(export, dont_resolve, consumer))
                }
                None => Ok(None),
            }?;
            let declaration = self.reference_module_type_only_export_declaration(
                symbol,
                name.as_bytes(),
                consumer,
            )?;
            sequence.read(self.mark_alias_type_only_completion(source, declaration, consumer))?;
            Ok(result)
        })();
        sequence.finish(result)
    }

    fn alias_property_completion(
        &mut self,
        symbol: ReferenceSymbol,
        name: &str,
        skip_augmentation: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let ty = sequence.read(self.value_reference_type_completion(symbol, consumer))?;
            sequence
                .read(self.property_of_type_ex_completion(ty, name, skip_augmentation, consumer))?
                .map(|property| {
                    self.property_symbol(&property, consumer)
                        .map(ReferenceSymbol::from_property)
                })
                .transpose()
        })();
        sequence.finish(result)
    }

    /// `resolveExportByName` uses the raw export table unless export= is
    /// present, in which case its type/property question takes precedence.
    fn resolve_export_by_name_completion(
        &mut self,
        module: ReferenceSymbol,
        source: Option<super::super::source::DeclarationIdentity>,
        name: &str,
        dont_resolve: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let export = if let Some(value) = self.own_module_export(module, "export=", consumer)? {
                sequence.read(self.alias_property_completion(value, name, true, consumer))?
            } else {
                self.own_module_export(module, name, consumer)?
            };
            let result = match export {
                Some(export) => {
                    sequence.read(self.resolve_symbol_ex_completion(export, dont_resolve, consumer))
                }
                None => Ok(None),
            }?;
            if let Some(source) = source {
                sequence.read(self.mark_alias_type_only_completion(source, None, consumer))?;
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    fn import_equals_module_target_completion(
        &mut self,
        module: ReferenceSymbol,
        source: super::super::source::DeclarationIdentity,
        _site: ReferenceModuleSite,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let target = sequence
                .read(self.reference_external_module_symbol_completion(module, true, consumer))?;
            if let Some(target) = target
                && matches!(
                    self.inputs.semantic_options().module_kind,
                    ModuleKind::Node20 | ModuleKind::NodeNext
                )
                && let Some(export) = sequence.read(self.alias_export_of_module_completion(
                    target,
                    source,
                    "module.exports",
                    true,
                    consumer,
                ))?
            {
                return Ok(Some(export));
            }
            Ok(target)
        })();
        sequence.finish(result)
    }

    fn alias_module_file(
        &self,
        module: ReferenceSymbol,
        _consumer: &Location,
    ) -> Result<Option<FileId>, DemandError> {
        self.reference_module_source_file(module)
    }

    /// `isOnlyImportableAsDefault`: emit mode is separate from an explicit
    /// resolution-mode attribute. JSON source and .d.json.ts are both tested.
    fn only_importable_as_default(
        &mut self,
        module: ReferenceSymbol,
        site: ReferenceModuleSite,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        if !self.inputs.semantic_options().module_kind.is_node()
            || self.inputs.bindings.reference_module_site(site).emit_mode
                != Some(ResolutionMode::Import)
        {
            return Ok(false);
        }
        let Some(file) = self.source_file_of_module(module, consumer)? else {
            return Ok(false);
        };
        Ok(self.inputs.bindings.file_is_json(file)
            || self
                .inputs
                .sources
                .get(file)
                .path
                .as_os_str()
                .as_encoded_bytes()
                .ends_with(b".d.json.ts"))
    }

    /// `canHaveSyntheticDefault`: always asked by getTargetOfModuleDefault,
    /// even when the raw table already has a real default export.
    fn can_have_synthetic_default_completion(
        &mut self,
        file: Option<FileId>,
        module: ReferenceSymbol,
        dont_resolve: bool,
        site: ReferenceModuleSite,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(file) = file {
                let usage = self.inputs.bindings.reference_module_site(site).emit_mode;
                if usage.is_some() {
                    let target = self
                        .inputs
                        .bindings
                        .file_module_semantics(file)
                        .implied_node_format;
                    if usage == Some(ResolutionMode::Import)
                        && target == Some(ResolutionMode::Require)
                        && self.inputs.semantic_options().module_kind.is_node()
                    {
                        return Ok(true);
                    }
                    if usage == Some(ResolutionMode::Import)
                        && target == Some(ResolutionMode::Import)
                    {
                        return Ok(false);
                    }
                    // Project-reference redirects are excluded by program input
                    // admission; no filesystem/configuration probing occurs here.
                }
            }
            if file.is_none_or(|file| self.inputs.bindings.file_is_declaration(file)) {
                if let Some(default) = sequence.read(
                    self.resolve_export_by_name_completion(module, None, "default", true, consumer),
                )? {
                    for declaration in self.reference_symbol_declarations(default, consumer)? {
                        let ReferenceDeclaration::Captured(declaration) = declaration else {
                            return Err(self.access_pending(
                                consumer,
                                "JSON syntactic default declaration requires its symbol factory",
                            ));
                        };
                        if self.source.declaration_is_syntactic_default(declaration)? {
                            return Ok(false);
                        }
                    }
                }
                return Ok(sequence
                    .read(self.resolve_export_by_name_completion(
                        module,
                        None,
                        "__esModule",
                        dont_resolve,
                        consumer,
                    ))?
                    .is_none());
            }
            // A TypeScript file has a synthetic default only with `export =`;
            // a JavaScript file only where its external module indicator is
            // absent or the file itself, and it has no `__esModule` export.
            // Its `export=` is a `module.exports` assignment, which binding
            // admits only where the CommonJS indicator alone makes the file
            // a module. Another script is a module by its format or
            // `moduleDetection`, which this tool does not read.
            let Some(file) = file else {
                unreachable!("a declaration file answered above");
            };
            if !self.inputs.in_javascript_file(file) {
                return Ok(self
                    .own_module_export(module, "export=", consumer)?
                    .is_some());
            }
            if self.source.has_module_syntax(file)? {
                return Ok(false);
            }
            if self
                .own_module_export(module, "export=", consumer)?
                .is_some()
            {
                return Ok(sequence
                    .read(self.resolve_export_by_name_completion(
                        module,
                        None,
                        "__esModule",
                        dont_resolve,
                        consumer,
                    ))?
                    .is_none());
            }
            Err(super::super::unsupported_demand_at_location(
                self.inputs,
                consumer,
                crate::analysis::SemanticFailureReason::JavascriptSemanticsRequired,
                "a default import of a JavaScript module without ES module syntax needs its synthetic default",
            ))
        })();
        sequence.finish(result)
    }

    fn module_default_target_completion(
        &mut self,
        module: ReferenceSymbol,
        declaration: super::super::source::DeclarationIdentity,
        site: ReferenceModuleSite,
        dont_resolve: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let file = self.alias_module_file(module, consumer)?;
            if !self.reference_module_is_shorthand(module)?
                && let Some(file) = file
                && matches!(
                    self.inputs.semantic_options().module_kind,
                    ModuleKind::Node20 | ModuleKind::NodeNext
                )
                && self.inputs.bindings.reference_module_site(site).emit_mode
                    == Some(ResolutionMode::Require)
                && self
                    .inputs
                    .bindings
                    .file_module_semantics(file)
                    .implied_node_format
                    == Some(ResolutionMode::Import)
                && let Some(export) = sequence.read(self.resolve_export_by_name_completion(
                    module,
                    Some(declaration),
                    "module.exports",
                    dont_resolve,
                    consumer,
                ))?
            {
                return Ok(Some(export));
            }
            let default = sequence.read(self.resolve_export_by_name_completion(
                module,
                Some(declaration),
                "default",
                dont_resolve,
                consumer,
            ))?;
            let only_default = self.only_importable_as_default(module, site, consumer)?;
            let synthetic = sequence.read(self.can_have_synthetic_default_completion(
                file,
                module,
                dont_resolve,
                site,
                consumer,
            ))?;
            if default.is_none() && !synthetic && !only_default {
                let source_symbol =
                    sequence.read(self.reference_symbol_of_declaration_completion(declaration))?;
                sequence.read(self.missing_default_export_completion(
                    module,
                    source_symbol,
                    declaration,
                    consumer,
                ))?;
            }
            if synthetic || only_default {
                return match sequence.read(self.reference_external_module_symbol_completion(
                    module,
                    dont_resolve,
                    consumer,
                ))? {
                    Some(symbol) => Ok(Some(symbol)),
                    None => sequence.read(self.resolve_symbol_ex_completion(
                        module,
                        dont_resolve,
                        consumer,
                    )),
                };
            }
            Ok(default)
        })();
        sequence.finish(result)
    }

    /// `resolveESModuleSymbol`. Namespace imports have a type-producing
    /// sub-operation; named imports/exports do not accidentally acquire it.
    fn es_module_target_completion(
        &mut self,
        source: super::super::source::DeclarationIdentity,
        module: ReferenceSymbol,
        site: ReferenceModuleSite,
        namespace_import: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(mut symbol) = sequence
                .read(self.reference_external_module_symbol_completion(module, true, consumer))?
            else {
                return Ok(None);
            };
            if self
                .reference_symbol_flags(symbol, consumer)?
                .is_non_local_alias()
            {
                let source_symbol =
                    sequence.read(self.reference_symbol_of_declaration_completion(source))?;
                let Some(resolved) = sequence.read(self.resolve_indirection_alias_completion(
                    source_symbol,
                    symbol,
                    consumer,
                ))?
                else {
                    return Ok(None);
                };
                symbol = self.get_merged_reference_symbol(resolved, consumer)?;
            }
            if namespace_import {
                let ty = sequence.read(self.value_reference_type_completion(symbol, consumer))?;
                if let Some(default_only) =
                    sequence.read(self.type_with_synthetic_default_only_completion(
                        ty, symbol, module, site, consumer,
                    ))?
                {
                    let parent = self.source.module_specifier_parent(site)?;
                    return sequence
                        .read(self.clone_type_as_module_type_completion(
                            symbol,
                            default_only,
                            parent,
                            consumer,
                        ))
                        .map(Some);
                }
                let file = self.alias_module_file(module, consumer)?;
                let usage = self.inputs.bindings.reference_module_site(site).emit_mode;
                if let Some(file) = file
                    && matches!(
                        self.inputs.semantic_options().module_kind,
                        ModuleKind::Node20 | ModuleKind::NodeNext
                    )
                    && usage == Some(ResolutionMode::Require)
                    && self
                        .inputs
                        .bindings
                        .file_module_semantics(file)
                        .implied_node_format
                        == Some(ResolutionMode::Import)
                    && let Some(export) = sequence.read(self.alias_export_of_module_completion(
                        symbol,
                        source,
                        "module.exports",
                        true,
                        consumer,
                    ))?
                {
                    if sequence.read(self.alias_type_has_signatures_completion(ty, consumer))? {
                        let parent = self.source.module_specifier_parent(site)?;
                        return sequence
                            .read(
                                self.clone_type_as_module_type_completion(
                                    export, ty, parent, consumer,
                                ),
                            )
                            .map(Some);
                    }
                    return Ok(Some(export));
                }
                let esm_cjs = usage == Some(ResolutionMode::Import)
                    && file.is_some_and(|file| {
                        self.inputs
                            .bindings
                            .file_module_semantics(file)
                            .implied_node_format
                            == Some(ResolutionMode::Require)
                    });
                if sequence.read(self.alias_type_has_signatures_completion(ty, consumer))?
                    || sequence
                        .read(self.property_of_type_ex_completion(ty, "default", true, consumer))?
                        .is_some()
                    || esm_cjs
                {
                    let module_type = if self.model.has_type_flags(
                        ty,
                        super::super::model::TypeFlags::OBJECT
                            | super::super::model::TypeFlags::UNION
                            | super::super::model::TypeFlags::INTERSECTION,
                    ) {
                        sequence.read(self.type_with_synthetic_default_import_completion(
                            ty, symbol, module, site, consumer,
                        ))?
                    } else {
                        let parent = self.reference_symbol_parent(symbol, consumer)?;
                        sequence.read(
                            self.create_default_property_wrapper_for_module_completion(
                                symbol, parent, None, consumer,
                            ),
                        )?
                    };
                    let parent = self.source.module_specifier_parent(site)?;
                    return sequence
                        .read(self.clone_type_as_module_type_completion(
                            symbol,
                            module_type,
                            parent,
                            consumer,
                        ))
                        .map(Some);
                }
            }
            Ok(Some(symbol))
        })();
        sequence.finish(result)
    }

    /// `getTypeWithSyntheticDefaultOnly` tests import mode before consulting
    /// the cache keyed by the actual input type identity.
    fn type_with_synthetic_default_only_completion(
        &mut self,
        ty: TypeId,
        symbol: ReferenceSymbol,
        module: ReferenceSymbol,
        site: ReferenceModuleSite,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        if !self.only_importable_as_default(module, site, consumer)? {
            return Ok(NativeCompletion::clean(None));
        }
        if let Some(result) = self.symbol_aliases.default_only_types.get(&ty) {
            return result.clone().map(|value| value.map(Some));
        }
        let parent = self.reference_as_parent(module)?;
        let result = self.create_default_property_wrapper_for_module_completion(
            symbol,
            Some(parent),
            None,
            consumer,
        );
        self.symbol_aliases
            .default_only_types
            .insert(ty, result.clone());
        result.map(|value| value.map(Some))
    }

    /// `getTypeWithSyntheticDefaultImportType` publishes the wrapper read
    /// before spread can reenter it; the native cache retains all acquisitions.
    fn type_with_synthetic_default_import_completion(
        &mut self,
        ty: TypeId,
        symbol: ReferenceSymbol,
        module: ReferenceSymbol,
        site: ReferenceModuleSite,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(result) = self.symbol_aliases.synthetic_types.get(&ty) {
            return result.clone();
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let file = self.alias_module_file(module, consumer)?;
            if !sequence.read(
                self.can_have_synthetic_default_completion(file, module, false, site, consumer),
            )? {
                return Ok(ty);
            }
            let anonymous = self.create_synthetic_module_type_symbol();
            let parent = self.reference_as_parent(module)?;
            let wrapper = self.create_default_property_wrapper_for_module_completion(
                symbol,
                Some(parent),
                Some(anonymous),
                consumer,
            )?;
            self.set_property_value_state(anonymous, false, TypeState::Installed(wrapper.clone()))?;
            let wrapper = sequence.read(Ok(wrapper))?;
            if sequence.read(self.is_valid_spread_type_completion(ty, consumer))? {
                let type_symbol = self.type_symbol_of_reference(
                    ReferenceSymbol::from_property(anonymous),
                    consumer,
                )?;
                sequence.read(self.spread_type_completion(
                    ty,
                    wrapper,
                    type_symbol,
                    ObjectFlags::NONE,
                    false,
                    consumer,
                ))
            } else {
                Ok(wrapper)
            }
        })();
        let result = sequence.finish(result);
        self.symbol_aliases
            .synthetic_types
            .insert(ty, result.clone());
        result
    }

    fn alias_type_has_signatures_completion(
        &mut self,
        ty: super::super::model::TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            Ok(!sequence
                .read(self.signatures_of_structured_type_completion(ty, false, consumer))?
                .is_empty()
                || !sequence
                    .read(self.signatures_of_structured_type_completion(ty, true, consumer))?
                    .is_empty())
        })();
        sequence.finish(result)
    }

    fn external_module_member_target_completion(
        &mut self,
        module: ReferenceSymbol,
        declaration: super::super::source::DeclarationIdentity,
        site: ReferenceModuleSite,
        name: &str,
        dont_resolve: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(target) = sequence.read(self.es_module_target_completion(
                declaration,
                module,
                site,
                false,
                consumer,
            ))?
            else {
                return Ok(None);
            };
            if self.reference_module_is_shorthand(module)? {
                return Ok(Some(module));
            }
            let export_assignment = self
                .own_module_export(module, "export=", consumer)?
                .is_some();
            let variable = if export_assignment {
                sequence.read(self.alias_property_completion(target, name, true, consumer))?
            } else if self.reference_symbol_flags(target, consumer)?.variable {
                let declaration = self
                    .reference_symbol_value_declaration(target, consumer)?
                    .ok_or_else(|| {
                        super::super::internal("native variable symbol has no ValueDeclaration")
                    })?;
                let ReferenceDeclaration::Captured(declaration) = declaration else {
                    return Err(self.access_pending(
                        consumer,
                        "JSON variable annotation requires its symbol factory",
                    ));
                };
                if let Some(annotation) = self.source.declaration_type_annotation(declaration)? {
                    let ty = sequence.read(self.check_type_completion(&annotation))?;
                    let property = sequence
                        .read(self.reference_property_symbol_completion(ty, name, consumer))?;
                    match property {
                        Some(property) => sequence
                            .read(self.resolve_reference_symbol_completion(property, consumer))?,
                        None => None,
                    }
                } else {
                    None
                }
            } else {
                None
            };
            let variable = match variable {
                Some(variable) => sequence.read(self.resolve_symbol_ex_completion(
                    variable,
                    dont_resolve,
                    consumer,
                ))?,
                None => None,
            };
            let container = if export_assignment { module } else { target };
            let mut exported = sequence.read(self.alias_export_of_module_completion(
                container,
                declaration,
                name,
                dont_resolve,
                consumer,
            ))?;
            if exported.is_none() && name == "default" {
                let file = self.alias_module_file(module, consumer)?;
                if self.only_importable_as_default(module, site, consumer)?
                    || sequence.read(self.can_have_synthetic_default_completion(
                        file,
                        module,
                        dont_resolve,
                        site,
                        consumer,
                    ))?
                {
                    exported = sequence.read(self.reference_external_module_symbol_completion(
                        module,
                        dont_resolve,
                        consumer,
                    ))?;
                    if exported.is_none() {
                        exported = sequence.read(self.resolve_symbol_ex_completion(
                            module,
                            dont_resolve,
                            consumer,
                        ))?;
                    }
                }
            }
            let result = match (variable, exported) {
                (Some(value), Some(ty)) => {
                    Some(self.combine_value_and_type_symbols(value, ty, consumer)?)
                }
                (value, None) => value,
                (None, exported) => exported,
            };
            if self.only_importable_as_default(module, site, consumer)? && name != "default" {
                // The native diagnostic formats only moduleKind, with no checker
                // child acquisition. The selected symbol remains its return value.
            } else if result.is_none() {
                let enclosing = self.source.alias_external_container(declaration)?;
                sequence.read(self.missing_module_member_completion(
                    module,
                    target,
                    declaration,
                    enclosing,
                    consumer,
                ))?;
            }
            Ok(result)
        })();
        sequence.finish(result)
    }
}
