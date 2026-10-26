//! Imported emit helpers are symbol dependencies, not helper value types.
//! The source-file cache is shared by every checked use in this request.

use super::super::model::{DemandError, NativeResult, NativeSequence};
use super::super::source::DeclarationIdentity;
use super::{Evaluator, ReferenceSymbol};
use crate::project::sources::FileId;
use crate::references::Location;
use std::collections::HashMap;

/// Native helper bits admitted by current checked expression producers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct ExternalEmitHelpers(u32);
impl ExternalEmitHelpers {
    pub(super) const REST: Self = Self(1);
    pub(super) const DECORATE: Self = Self(1 << 1);
    pub(super) const METADATA: Self = Self(1 << 2);
    pub(super) const PRIVATE_GET: Self = Self(1 << 13);
    pub(super) const PRIVATE_SET: Self = Self(1 << 14);
    pub(super) const PRIVATE_IN: Self = Self(1 << 15);
    pub(super) const SET_FUNCTION_NAME: Self = Self(1 << 16);
    pub(super) const PROP_KEY: Self = Self(1 << 17);

    /// `getHelperNames` maps the native helper bit, including the shared
    /// Decorate/ESDecorateAndRunInitializers bit's option-dependent names.
    fn names(self, legacy_decorators: bool) -> &'static [&'static str] {
        match self {
            Self::REST => &["__rest"],
            Self::DECORATE if legacy_decorators => &["__decorate"],
            Self::DECORATE => &["__esDecorate", "__runInitializers"],
            Self::METADATA => &["__metadata"],
            Self::PRIVATE_GET => &["__classPrivateFieldGet"],
            Self::PRIVATE_SET => &["__classPrivateFieldSet"],
            Self::PRIVATE_IN => &["__classPrivateFieldIn"],
            Self::SET_FUNCTION_NAME => &["__setFunctionName"],
            Self::PROP_KEY => &["__propKey"],
            _ => unreachable!("only admitted native helper bits have names"),
        }
    }
}

pub(super) struct EmitHelpersState {
    modules: HashMap<FileId, Result<ReferenceSymbol, DemandError>>,
    requested: HashMap<FileId, u32>,
}

impl EmitHelpersState {
    /// Request-local backing for resolveHelpersModule and checkExternalEmitHelpers.
    /// Payload creation/copies and publication order stay with those operations.
    pub(super) fn new() -> Self {
        Self {
            modules: HashMap::new(),
            requested: HashMap::new(),
        }
    }
}

impl Evaluator<'_, '_> {
    /// Admission adapter for `checkExternalEmitHelpers`; native consumers
    /// that continue after a helper signature recovery use the completion.
    pub(super) fn check_external_emit_helpers(
        &mut self,
        declaration: DeclarationIdentity,
        helpers: ExternalEmitHelpers,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        self.check_external_emit_helpers_completion(declaration, helpers, consumer)?
            .admit()
    }

    /// `checkExternalEmitHelpers`: the actual module is acquired before the
    /// requested-bit gate. Private Get/Set inspect declaration signatures and
    /// native parameter counts, never a helper's value type. Publish bits only
    /// after all newly requested helpers finish; diagnostics alone keep going.
    pub(super) fn check_external_emit_helpers_completion(
        &mut self,
        declaration: DeclarationIdentity,
        helpers: ExternalEmitHelpers,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !self.inputs.semantic_options().import_helpers
                || !self.source.emit_helpers_eligible(declaration)?
            {
                return Ok(());
            }
            let module = self.resolve_helpers_module(declaration.file, consumer)?;
            if self.is_unknown_reference_symbol(module) {
                return Ok(());
            }
            let requested = self
                .emit_helpers
                .requested
                .get(&declaration.file)
                .copied()
                .unwrap_or(0);
            let unchecked = helpers.0 & !requested;
            for (bit, arity) in [
                (ExternalEmitHelpers::REST, None),
                (ExternalEmitHelpers::DECORATE, None),
                (ExternalEmitHelpers::METADATA, None),
                (ExternalEmitHelpers::PRIVATE_GET, Some(3)),
                (ExternalEmitHelpers::PRIVATE_SET, Some(4)),
                (ExternalEmitHelpers::PRIVATE_IN, None),
                (ExternalEmitHelpers::SET_FUNCTION_NAME, None),
                (ExternalEmitHelpers::PROP_KEY, None),
            ] {
                if unchecked & bit.0 == 0 {
                    continue;
                }
                for &name in bit.names(self.inputs.semantic_options().experimental_decorators) {
                    if let Some(symbol) = sequence.read(
                        self.reference_module_value_export_completion(module, name, consumer),
                    )? && let Some(symbol) =
                        sequence.read(self.resolve_reference_symbol_completion(symbol, consumer))?
                        && let Some(arity) = arity
                    {
                        for signature in sequence
                            .read(self.signatures_of_symbol_completion(Some(symbol), consumer))?
                        {
                            if sequence.read(
                                self.signature_parameter_count_completion(&signature, consumer),
                            )? > arity
                            {
                                break;
                            }
                        }
                    }
                }
            }
            self.emit_helpers
                .requested
                .insert(declaration.file, requested | helpers.0);
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `resolveHelpersModule`: the program retains the actual synthetic request.
    /// Native missing resolution installs unknownSymbol, rather than errorType;
    /// failures of an acquired dependency retain their original identity.
    fn resolve_helpers_module(
        &mut self,
        file: FileId,
        consumer: &Location,
    ) -> Result<ReferenceSymbol, DemandError> {
        if let Some(result) = self.emit_helpers.modules.get(&file) {
            return result.clone();
        }
        let result = self
            .source
            .bindings()
            .helper_module_selection(file)
            .map_err(|error| self.generic_binding_failure(error, consumer))
            .and_then(|module| match module {
                Some(module) => {
                    let symbol = module
                        .pattern_name
                        .and_then(|name| {
                            self.initialization.pattern_augmentations.get(name).copied()
                        })
                        .unwrap_or_else(|| module.symbol.into());
                    self.get_merged_reference_symbol(symbol, consumer)
                }
                None => Ok(self.unknown_reference_symbol()),
            });
        self.emit_helpers.modules.insert(file, result.clone());
        result
    }
}
