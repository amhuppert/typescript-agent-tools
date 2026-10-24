//! Native node-builder scope lifetime. Semantic acquisition stays in the
//! serializer callback; mapper/context restoration also runs after failures.
use super::super::super::model::NativeSequence;
use super::super::symbol_accessibility::ScopeLocation;
use super::super::{ReferenceDeclaration, ReferenceSymbol};
use super::*;
use crate::project::binding::NativePropertyKey;

/// Identity of an actual node-builder-created Block. It stays stable while
/// pushFakeScope temporarily changes that block's locals.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic::evaluation) struct DiagnosticScopeId(usize);

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Parameters,
    TypeParameters,
}

pub(super) struct DiagnosticScope {
    parent: ScopeLocation,
    kind: ScopeKind,
    locals: Vec<(NativePropertyKey, ReferenceSymbol)>,
}

struct ScopeUndo {
    scope: DiagnosticScopeId,
    added: Vec<NativePropertyKey>,
    replaced: Vec<(NativePropertyKey, ReferenceSymbol)>,
}

struct ScopeEdit {
    existing: Option<ScopeUndo>,
    locals: Vec<(NativePropertyKey, ReferenceSymbol)>,
}

impl Evaluator<'_, '_> {
    /// Parent and established local table of a native fake Block. Reading it
    /// never resolves symbols or constructs a replacement captured node.
    pub(in crate::project::semantic::evaluation) fn diagnostic_scope_parent(
        &self,
        id: DiagnosticScopeId,
    ) -> Result<ScopeLocation, DemandError> {
        self.native_display
            .scopes
            .get(id.0)
            .map(|scope| scope.parent)
            .ok_or_else(|| {
                super::super::super::internal("diagnostic scope identity is not retained").into()
            })
    }
    pub(in crate::project::semantic::evaluation) fn diagnostic_scope_locals(
        &self,
        id: DiagnosticScopeId,
    ) -> Result<&[(NativePropertyKey, ReferenceSymbol)], DemandError> {
        self.native_display
            .scopes
            .get(id.0)
            .map(|scope| scope.locals.as_slice())
            .ok_or_else(|| {
                super::super::super::internal("diagnostic scope identity is not retained").into()
            })
    }
    /// Synthetic blocks have no source text. Native source-node questions use
    /// the first actual captured ancestor; nil remains an actual nil entry.
    pub(in crate::project::semantic::evaluation) fn diagnostic_captured_enclosing(
        &self,
        mut scope: Option<ScopeLocation>,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        while let Some(current) = scope {
            match current {
                ScopeLocation::Captured(declaration) => return Ok(Some(declaration)),
                ScopeLocation::Synthetic(id) => scope = Some(self.diagnostic_scope_parent(id)?),
            }
        }
        Ok(None)
    }
}

impl Evaluator<'_, '_> {
    /// `enterNewScope` / `cloneNodeBuilderContext`. The fake Blocks have
    /// stable identities and temporarily shadow locals. The fixed enclosing
    /// file and all acquired completion evidence belong to the outer context.
    #[expect(
        clippy::too_many_arguments,
        reason = "ports enterNewScope's inputs plus the scoped worker"
    )]
    pub(super) fn diagnostic_scope<T>(
        &mut self,
        context: &mut Context,
        declaration: Option<DeclarationIdentity>,
        expanded: &[ParameterValue],
        type_parameters: &[TypeId],
        original: Option<&[ParameterValue]>,
        mapper: Option<MapperId>,
        consumer: &Location,
        worker: impl FnOnce(&mut Self, &mut Context) -> NativeResult<T>,
    ) -> NativeResult<T> {
        let old_symbols = context.type_parameter_symbol_list.clone();
        let old_names = context.type_parameter_names.clone();
        let old_names_by_text = context.type_parameter_names_by_text.clone();
        let old_counts = context.type_parameter_name_counts.clone();
        let old_enclosing = context.enclosing;
        let old_mapper = context.mapper;
        if let Some(mapper) = mapper {
            context.mapper = Some(mapper);
        }
        let mut parameters = None;
        let mut type_params = None;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if context.enclosing.is_some() && declaration.is_some() {
                if !expanded.is_empty() {
                    parameters = self.diagnostic_push_fake_scope(
                        context,
                        ScopeKind::Parameters,
                        |checker, _, edit| {
                            for (index, parameter) in expanded.iter().enumerate() {
                                let original_parameter =
                                    original.and_then(|parameters| parameters.get(index));
                                if original.is_some()
                                    && original_parameter
                                        .is_none_or(|original| original.symbol != parameter.symbol)
                                {
                                    if let Some(original) = original_parameter {
                                        checker.diagnostic_scope_add_symbol(
                                            edit,
                                            ReferenceSymbol::from_property(original.symbol),
                                            consumer,
                                        )?;
                                    }
                                    continue;
                                }
                                let symbol = ReferenceSymbol::from_property(parameter.symbol);
                                let mut pattern = false;
                                for declaration in
                                    checker.reference_symbol_declarations(symbol, consumer)?
                                {
                                    let ReferenceDeclaration::Captured(declaration) = declaration
                                    else {
                                        continue;
                                    };
                                    if let Some(binding) =
                                        checker.source.diagnostic_parameter_binding(declaration)?
                                    {
                                        if let Some(binding) = binding
                                            && let Some(symbol) = sequence.read(
                                                checker
                                                    .reference_node_declaration_symbol_completion(
                                                        binding,
                                                    ),
                                            )?
                                        {
                                            checker.diagnostic_scope_add_symbol(
                                                edit, symbol, consumer,
                                            )?;
                                        }
                                        pattern = true;
                                        break;
                                    }
                                }
                                if !pattern {
                                    checker.diagnostic_scope_add_symbol(edit, symbol, consumer)?;
                                }
                            }
                            Ok(())
                        },
                    )?;
                }
                if context.flags.has(1 << 2) && !type_parameters.is_empty() {
                    type_params = self.diagnostic_push_fake_scope(context, ScopeKind::TypeParameters, |checker, context, edit| {
                        for &parameter in type_parameters {
                            let name = sequence.read(checker.diagnostic_type_parameter_name_completion(parameter, context, consumer))?;
                            let symbol = checker.reference_symbol_of_type(parameter, consumer)?.ok_or_else(||
                                checker.diagnostic_dependency(consumer, "native synthetic type parameter requires its actual symbol"))?;
                            checker.diagnostic_scope_add(edit, NativePropertyKey::fixed(name.into_bytes()), symbol);
                        }
                        Ok(())
                    })?;
                }
            }
            sequence.read(worker(self, context))
        })();
        // Native cleanup order is params, then typeParams, then context maps.
        if let Some(undo) = parameters {
            self.diagnostic_scope_undo(undo);
        }
        if let Some(undo) = type_params {
            self.diagnostic_scope_undo(undo);
        }
        context.type_parameter_names = old_names;
        context.type_parameter_names_by_text = old_names_by_text;
        context.type_parameter_name_counts = old_counts;
        context.type_parameter_symbol_list = old_symbols;
        context.enclosing = old_enclosing;
        context.mapper = old_mapper;
        sequence.finish(result)
    }

    /// Native `pushFakeScope` reuses only the immediate or immediate-parent
    /// Block with the same role. A newly built locals table is invisible until
    /// addAll finishes; an existing table changes at each individual write.
    fn diagnostic_push_fake_scope(
        &mut self,
        context: &mut Context,
        kind: ScopeKind,
        add_all: impl FnOnce(&mut Self, &mut Context, &mut ScopeEdit) -> Result<(), DemandError>,
    ) -> Result<Option<ScopeUndo>, DemandError> {
        let enclosing = context.enclosing.ok_or_else(|| {
            super::super::super::internal("fake scope requires an enclosing node")
        })?;
        let matching = |this: &Self, scope| match scope {
            ScopeLocation::Synthetic(id) if this.native_display.scopes[id.0].kind == kind => {
                Some(id)
            }
            _ => None,
        };
        let mut existing = matching(self, enclosing);
        if existing.is_none() {
            let parent = match enclosing {
                ScopeLocation::Synthetic(id) => Some(self.diagnostic_scope_parent(id)?),
                ScopeLocation::Captured(declaration) => {
                    self.source.declaration_parent(declaration)?.map(Into::into)
                }
            };
            existing = parent.and_then(|parent| matching(self, parent));
        }
        let mut edit = ScopeEdit {
            existing: existing.map(|scope| ScopeUndo {
                scope,
                added: Vec::new(),
                replaced: Vec::new(),
            }),
            locals: Vec::new(),
        };
        if let Err(error) = add_all(self, context, &mut edit) {
            if let Some(undo) = edit.existing {
                self.diagnostic_scope_undo(undo);
            }
            return Err(error);
        }
        if let Some(undo) = edit.existing {
            return Ok(Some(undo));
        }
        let id = DiagnosticScopeId(self.native_display.scopes.len());
        self.native_display.scopes.push(DiagnosticScope {
            parent: enclosing,
            kind,
            locals: edit.locals,
        });
        context.enclosing = Some(ScopeLocation::Synthetic(id));
        Ok(None)
    }

    fn diagnostic_scope_add_symbol(
        &mut self,
        edit: &mut ScopeEdit,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        let key = self.reference_symbol_key(symbol, consumer)?;
        self.diagnostic_scope_add(edit, key, symbol);
        Ok(())
    }

    fn diagnostic_scope_add(
        &mut self,
        edit: &mut ScopeEdit,
        key: NativePropertyKey,
        symbol: ReferenceSymbol,
    ) {
        let locals = match &mut edit.existing {
            Some(undo) => &mut self.native_display.scopes[undo.scope.0].locals,
            None => &mut edit.locals,
        };
        if let Some((_, old)) = locals.iter_mut().find(|(name, _)| name == &key) {
            if let Some(undo) = &mut edit.existing {
                undo.replaced.push((key, *old));
            }
            *old = symbol;
        } else {
            if let Some(undo) = &mut edit.existing {
                undo.added.push(key.clone());
            }
            locals.push((key, symbol));
        }
    }

    fn diagnostic_scope_undo(&mut self, undo: ScopeUndo) {
        let locals = &mut self.native_display.scopes[undo.scope.0].locals;
        for key in undo.added {
            locals.retain(|(name, _)| name != &key);
        }
        for (key, symbol) in undo.replaced {
            if let Some((_, old)) = locals.iter_mut().find(|(name, _)| name == &key) {
                *old = symbol;
            } else {
                locals.push((key, symbol));
            }
        }
    }

    /// `typeParameterToName` owns generated-name maps, the symbol serializer
    /// prelude, and the shared resolver's native type-name shadow check.
    pub(super) fn diagnostic_type_parameter_name_completion(
        &mut self,
        ty: TypeId,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<String> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if context.flags.has(1 << 2)
                && let Some(name) = context.type_parameter_names.get(&ty)
            {
                return Ok(name.clone());
            }
            let Some(symbol) = self.reference_symbol_of_type(ty, consumer)? else {
                return Ok("(Missing type parameter)".into());
            };
            let chain = sequence.read(self.diagnostic_symbol_chain_completion(
                symbol,
                super::super::SymbolMeaning::Type,
                context,
                false,
                consumer,
            ))?;
            if chain.len() != 1 && !context.encountered_error && context.flags.has(1 << 19) {
                context.encountered_error = true;
            }
            // createEntityNameFromSymbolChain visits the rightmost name first.
            // It has no approximate-length increments (unlike type access nodes).
            let mut names = Vec::with_capacity(chain.len());
            for (index, &symbol) in chain.iter().enumerate().rev() {
                if index == 0 {
                    context.flags.0 |= 1 << 24;
                }
                let name = self.symbol_name_as_written_in_context(symbol, context, consumer);
                if index == 0 {
                    context.flags.0 &= !(1 << 24);
                }
                names.push(name?);
            }
            if chain.len() != 1 {
                return Ok("(Missing type parameter)".into());
            }
            let mut name = names.pop().ok_or_else(|| {
                super::super::super::internal("type parameter name chain is empty")
            })?;
            if context.flags.has(1 << 2) {
                let raw = name.clone();
                let mut count = context
                    .type_parameter_name_counts
                    .get(&raw)
                    .copied()
                    .unwrap_or(0);
                loop {
                    let shadows = if context.type_parameter_names_by_text.contains(&name) {
                        true
                    } else if let Some(found) =
                        sequence.read(self.diagnostic_resolve_name_completion(
                            context.enclosing,
                            &name,
                            super::super::SymbolMeaning::Type,
                            false,
                            consumer,
                        ))?
                    {
                        self.reference_symbol_flags(found, consumer)?.type_parameter
                            && self.reference_symbol_identity(found, consumer)?
                                != self.reference_symbol_identity(symbol, consumer)?
                    } else {
                        false
                    };
                    if !shadows {
                        break;
                    }
                    count += 1;
                    name = format!("{raw}_{count}");
                }
                context.type_parameter_name_counts.insert(raw, count);
                context.type_parameter_names.insert(ty, name.clone());
                context.type_parameter_names_by_text.insert(name.clone());
            }
            Ok(name)
        })();
        sequence.finish(result)
    }

    /// `typeParameterToDeclarationWithConstraint` names the actual parameter
    /// before asking its default; callers own constraint acquisition and scope.
    pub(super) fn diagnostic_parameter_declaration(
        &mut self,
        ty: TypeId,
        constraint: Option<Box<Node>>,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<TypeParameter> {
        let saved = context.flags;
        context.flags.0 &= !(1 << 9);
        let result = (|| {
            let TypeView::TypeParameter(parameter) = self.model.get(ty)? else {
                return Err(super::super::super::internal(
                    "native type parameter declaration lacks a parameter",
                )
                .into());
            };
            self.diagnostic_type_parameter_name_completion(ty, context, consumer)?
                .and_then(|name| {
                    let default =
                        self.default_from_type_parameter_completion(ty)?
                            .and_then(|default| match default {
                                Some(ty) => Ok(self
                                    .diagnostic_type_node(ty, context, consumer)?
                                    .map(|node| Some(Box::new(node)))),
                                None => Ok(NativeCompletion::clean(None)),
                            })?;
                    Ok(default.map(|default| TypeParameter {
                        modifiers: diagnostic_parameter_modifiers(&parameter),
                        name,
                        constraint,
                        default,
                    }))
                })
        })();
        context.flags = saved;
        result
    }
}
