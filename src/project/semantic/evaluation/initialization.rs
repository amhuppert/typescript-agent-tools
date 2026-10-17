//! One `initializeChecker` schedule for loading and every fresh request.
//! Binder inputs are immutable; all current tables, headers and completions
//! belong to this Evaluator and are discarded with it.

use super::super::model::{DemandError, NativeCompletion, NativeResult, NativeSequence};
use super::reference_symbols::ReferenceRawSymbolTable;
use super::{Evaluator, ReferenceSymbol, TypeState};
use crate::analysis::AnalysisError;
use crate::project::binding::{self, NativePropertyKey, NativeSymbolFlags};
use crate::project::sources::FileId;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Initialization {
    phase: Phase,
    globals: Option<usize>,
    global_this: Option<ReferenceSymbol>,
    undefined: Option<ReferenceSymbol>,
    arguments: Option<ReferenceSymbol>,
    pub(super) pattern_augmentations: HashMap<String, ReferenceSymbol>,
}

#[derive(Default)]
enum Phase {
    #[default]
    NotStarted,
    Running,
    Complete(NativeCompletion<()>),
    Failed(DemandError),
}

impl Initialization {
    pub(super) fn globals_table(&self) -> Result<usize, DemandError> {
        self.globals
            .ok_or_else(|| super::super::internal("checker global table is not installed").into())
    }
    pub(super) fn global_this_symbol(&self) -> ReferenceSymbol {
        self.global_this
            .expect("checker globalThis is installed before native operations")
    }
    pub(super) fn arguments_symbol(&self) -> Result<ReferenceSymbol, DemandError> {
        self.arguments.ok_or_else(|| {
            super::super::internal("checker arguments symbol is not installed").into()
        })
    }
    pub(super) fn undefined_symbol(&self) -> Option<ReferenceSymbol> {
        self.undefined
    }
}

/// Typed completed-phase observations. Loading mutates only its separate
/// lexical projection; requests provide a no-op observer. The checker is
/// borrowed read-only while an observation executes.
pub(in crate::project) enum Step {
    GlobalThis,
    Script(FileId),
    Umd(FileId),
    GlobalAugmentation(FileId),
    Undefined,
    Ambient(binding::BoundSymbol),
    Module(binding::ReferenceModuleSite),
    Complete,
}

pub(in crate::project) trait Observer {
    fn observe(
        &mut self,
        step: Step,
        facts: &dyn binding::NativeFacts,
    ) -> Result<(), AnalysisError>;
}

struct NoProjection;
impl Observer for NoProjection {
    fn observe(
        &mut self,
        _step: Step,
        _facts: &dyn binding::NativeFacts,
    ) -> Result<(), AnalysisError> {
        Ok(())
    }
}

impl Evaluator<'_, '_> {
    pub(in crate::project::semantic) fn ensure_initialized(&mut self) -> Result<(), DemandError> {
        self.initialize_with_observer(&mut NoProjection)
    }

    /// Installation observes the startup completion without admitting its
    /// recovery as a dependency of every subsequently selected query. Only
    /// actual reads of installed type/value/alias slots consume their evidence.
    pub(in crate::project) fn initialize_with_observer(
        &mut self,
        observer: &mut dyn Observer,
    ) -> Result<(), DemandError> {
        match &self.initialization.phase {
            Phase::Complete(completion) => {
                let _installed = completion.value();
                return Ok(());
            }
            Phase::Failed(failure) => return Err(failure.clone()),
            Phase::Running => {
                return Err(super::super::internal(
                    "checker initialization was reentered before completion",
                )
                .into());
            }
            Phase::NotStarted => {}
        }
        self.initialization.phase = Phase::Running;
        match self.initialize_worker(observer) {
            Ok(completion) => {
                self.initialization.phase = Phase::Complete(completion);
                Ok(())
            }
            Err(failure) => {
                self.initialization.phase = Phase::Failed(failure.clone());
                Err(failure)
            }
        }
    }

    fn observe_initialization(
        &self,
        observer: &mut dyn Observer,
        step: Step,
    ) -> Result<(), DemandError> {
        observer
            .observe(step, &super::names::CurrentFacts { checker: self })
            .map_err(Into::into)
    }

    /// `initializeChecker`: native file/phase order, including deferred ambient
    /// modules and immediate pattern-augmentation publication, has one owner.
    fn initialize_worker(&mut self, observer: &mut dyn Observer) -> NativeResult<()> {
        let globals = self.new_reference_raw_table();
        self.initialization.globals = Some(globals);
        let global_this = self.new_intrinsic_reference_symbol(
            "globalThis",
            NativeSymbolFlags::intrinsic_module(),
            true,
        )?;
        let undefined = self.new_intrinsic_reference_symbol(
            "undefined",
            NativeSymbolFlags::intrinsic_property(),
            false,
        )?;
        let arguments = self.new_intrinsic_reference_symbol(
            "arguments",
            NativeSymbolFlags::intrinsic_property(),
            false,
        )?;
        self.initialization.global_this = Some(global_this);
        self.initialization.undefined = Some(undefined);
        self.initialization.arguments = Some(arguments);
        self.set_merge_raw_table(
            global_this
                .property()
                .expect("intrinsic has actual created header"),
            true,
            Some(ReferenceRawSymbolTable::Created(globals)),
        )?;
        self.symbol_tables.raw[globals].insert(
            Ok(NativePropertyKey::fixed(
                b"globalThis".as_slice(),
                self.control.as_ref(),
            )),
            global_this,
        );
        self.observe_initialization(observer, Step::GlobalThis)?;

        let inputs = self.source.bindings();
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut ambient = Vec::new();
            for file in inputs.initialization_files() {
                if let Some(locals) = file.script_locals() {
                    let entries = self.reference_raw_table_entries_at(
                        ReferenceRawSymbolTable::Retained(locals),
                        None,
                    )?;
                    for (_, symbol) in entries.iter() {
                        let raw = self
                            .reference_raw_origin(*symbol)?
                            .ok_or_else(|| {
                                super::super::internal("script locals contain a non-binder symbol")
                            })?
                            .symbol();
                        if inputs
                            .raw_symbol_is_ambient_module(raw)
                            .map_err(|failure| {
                                super::super::binding_failure(self.inputs, None, failure)
                            })?
                        {
                            ambient.push(raw);
                        } else {
                            sequence
                                .read(self.merge_global_reference_symbol_completion(*symbol))?;
                        }
                    }
                    self.observe_initialization(observer, Step::Script(file.file()))?;
                }
                for entry in file.global_exports() {
                    let (name, symbol) = entry.map_err(|failure| {
                        super::super::binding_failure(self.inputs, None, failure)
                    })?;
                    let key = NativePropertyKey::fixed(name.as_bytes(), self.control.as_ref());
                    if self
                        .reference_raw_table_lookup_at(
                            ReferenceRawSymbolTable::Created(globals),
                            &key,
                            None,
                        )?
                        .is_none()
                    {
                        self.symbol_tables.raw[globals].insert(Ok(key), symbol.into());
                    }
                }
                self.observe_initialization(observer, Step::Umd(file.file()))?;
            }
            for file in inputs.initialization_files() {
                if let Some(source) = file.global_augmentation() {
                    let symbol: ReferenceSymbol = source.into();
                    let consumer =
                        self.reference_symbol_consumer(symbol, None)?
                            .ok_or_else(|| {
                                super::super::internal("global augmentation has no declaration")
                            })?;
                    if let Some(table) = self.reference_raw_symbol_table(symbol, true)? {
                        let entries = self.reference_raw_table_entries(table, &consumer)?;
                        sequence.read(self.merge_reference_symbol_table_completion(
                            globals, &entries, false, None, &consumer,
                        ))?;
                    }
                    self.observe_initialization(observer, Step::GlobalAugmentation(file.file()))?;
                }
            }
            // `addUndefinedToGlobalsOrErrorOnRedeclaration` is first-in-wins.
            let key = NativePropertyKey::fixed(b"undefined".as_slice(), self.control.as_ref());
            if self
                .reference_raw_table_lookup_at(
                    ReferenceRawSymbolTable::Created(globals),
                    &key,
                    None,
                )?
                .is_none()
            {
                self.symbol_tables.raw[globals].insert(Ok(key), undefined);
            }
            self.observe_initialization(observer, Step::Undefined)?;
            let undefined_type = self.model.undefined_initializer();
            self.set_property_value_state(
                undefined.property().expect("intrinsic has created header"),
                false,
                TypeState::Installed(NativeCompletion::clean(undefined_type)),
            )?;
            let arguments_lookup = self.global_type_worker_completion("IArguments", 0, None)?;
            let arguments_type = arguments_lookup.and_then(|lookup| {
                self.global_type_lookup_installed_type_completion("IArguments", 0, lookup, None)
            })?;
            sequence.read(Ok(arguments_type.clone()))?;
            self.set_property_value_state(
                arguments.property().expect("intrinsic has created header"),
                false,
                TypeState::Installed(arguments_type),
            )?;
            // unknownSymbol's existing source-free value owner installs errorType.
            let global_this_type = self.global_this_value_type(global_this)?;
            self.set_property_value_state(
                global_this
                    .property()
                    .expect("intrinsic has created header"),
                false,
                TypeState::Installed(NativeCompletion::clean(global_this_type)),
            )?;
            sequence.read(self.initialize_fixed_globals_completion())?;
            for source in ambient {
                sequence.read(self.merge_global_reference_symbol_completion(source.into()))?;
                self.observe_initialization(observer, Step::Ambient(source))?;
            }
            for module in inputs.initialization_modules() {
                sequence.read(
                    self.merge_module_augmentation_completion(module.site, module.symbol.into()),
                )?;
                self.observe_initialization(observer, Step::Module(module.site))?;
            }
            self.observe_initialization(observer, Step::Complete)?;
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `mergeGlobalSymbol` publishes the result before the next source entry.
    fn merge_global_reference_symbol_completion(
        &mut self,
        source: ReferenceSymbol,
    ) -> NativeResult<()> {
        let globals = self.initialization.globals_table()?;
        let key = self.reference_symbol_key_at(source, None)?;
        let old = self.reference_raw_table_lookup_at(
            ReferenceRawSymbolTable::Created(globals),
            &key,
            None,
        )?;
        let completed = match old {
            Some(target) => {
                let consumer = self
                    .reference_symbol_consumer(source, None)?
                    .ok_or_else(|| {
                        super::super::internal("global merge source lacks an actual declaration")
                    })?;
                self.merge_reference_symbol_completion(target, source, false, &consumer)?
            }
            None => NativeCompletion::clean(self.get_merged_reference_symbol_at(source, None)?),
        };
        let merged = *completed.value();
        self.symbol_tables.raw[globals].insert(Ok(key), merged);
        Ok(completed.map(|_| ()))
    }

    /// `mergeModuleAugmentation`: export-star collisions and pattern OneWay
    /// merges use the same actual checker header/table owner as other merges.
    fn merge_module_augmentation_completion(
        &mut self,
        site: binding::ReferenceModuleSite,
        source: ReferenceSymbol,
    ) -> NativeResult<()> {
        let consumer = super::super::location(
            self.inputs,
            site.file(),
            self.inputs.bindings.reference_module_site(site).span,
        );
        let Some(main) = self.reference_module_symbol_ignore_errors(site, &consumer)? else {
            return Ok(NativeCompletion::clean(()));
        };
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let main = sequence
                .read(self.reference_external_module_symbol_completion(main, false, &consumer))?
                .ok_or_else(|| super::super::internal("module augmentation resolved native nil"))?;
            let flags = self
                .reference_symbol_native_flags(main, &consumer)?
                .type_flags();
            if !flags.namespace && !flags.enum_ {
                return Ok(());
            }
            let identity = self.reference_symbol_identity(main, &consumer)?;
            if self
                .source
                .bindings()
                .pattern_module_symbols()
                .any(|pattern| pattern.native_identity() == identity)
            {
                let merged = sequence
                    .read(self.merge_reference_symbol_completion(source, main, true, &consumer))?;
                let name = self
                    .inputs
                    .bindings
                    .reference_module_specifier(site)
                    .to_owned();
                self.initialization
                    .pattern_augmentations
                    .insert(name, merged);
            } else {
                let additions = self
                    .reference_raw_symbol_table(source, true)?
                    .map(|table| self.reference_raw_table_entries(table, &consumer))
                    .transpose()?;
                let raw = self.reference_raw_symbol_table(main, true)?;
                let star = match raw {
                    Some(raw) => self
                        .reference_raw_table_lookup(
                            raw,
                            &NativePropertyKey::fixed(
                                b"\xfeexport".as_slice(),
                                self.control.as_ref(),
                            ),
                            &consumer,
                        )?
                        .is_some(),
                    None => false,
                };
                if star && additions.as_ref().is_some_and(|table| !table.is_empty()) {
                    let resolved = sequence
                        .read(self.resolved_symbol_table_completion(main, true, &consumer))?;
                    for (key, value) in additions.as_deref().unwrap_or_default() {
                        let target = resolved.as_ref().and_then(|table| {
                            table
                                .iter()
                                .find(|(name, _)| name == key)
                                .map(|(_, symbol)| *symbol)
                        });
                        if let Some(target) = target {
                            let own = raw
                                .map(|raw| self.reference_raw_table_lookup(raw, key, &consumer))
                                .transpose()?
                                .flatten();
                            if own.is_none() {
                                sequence.read(self.merge_reference_symbol_completion(
                                    target, *value, false, &consumer,
                                ))?;
                            }
                        }
                    }
                }
                sequence
                    .read(self.merge_reference_symbol_completion(main, source, false, &consumer))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }
}
