//! Checker-owned module spelling: native symbol acquisitions, enclosing-node
//! mode selection, and the request-local symbol cache share this operation.
use super::super::symbol_accessibility::ScopeLocation;
use super::super::{ReferenceDeclaration, ReferenceSymbol};
use super::{Context, DemandError, Evaluator, Location, NativeCompletion, NativeResult};
use crate::project::binding::{NativePropertyKey, NativeSymbolIdentity, ResolutionMode};
use crate::project::semantic::model::NativeSequence;
use crate::project::semantic::source::symbol_aliases::SymbolAliasTarget;
use crate::project::sources::FileId;
use crate::syntax_kind::SyntaxKind as K;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Specifiers {
    // `SymbolLinks.specifierCache`: the original enclosing node selects mode,
    // but only the fixed context file and effective mode belong in its key.
    values:
        HashMap<(NativeSymbolIdentity, FileId, Option<ResolutionMode>), NativeCompletion<String>>,
}

impl Evaluator<'_, '_> {
    /// `symbolToTypeNode`'s import-type mode and portability branch. Emit
    /// formats select explicit attributes; resolution modes select spelling.
    /// A failed portable retry restores the original spelling and attributes.
    pub(super) fn diagnostic_import_type_specifier(
        &mut self,
        symbol: ReferenceSymbol,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<(String, Option<ResolutionMode>)> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            use crate::project::program::{ModuleKind, ModuleResolutionKind};
            let context_file = self
                .diagnostic_captured_enclosing(context.enclosing)?
                .map(|node| node.file);
            let target_file = self.source_file_of_module(symbol, consumer)?;
            let node_resolution = matches!(
                self.inputs.semantic_options().module_resolution,
                ModuleResolutionKind::Node16 | ModuleResolutionKind::NodeNext
            );
            let mut specifier = String::new();
            let mut attributes = None;
            if node_resolution && let (Some(target), Some(enclosing)) = (target_file, context_file)
            {
                let target_emit = self
                    .inputs
                    .bindings
                    .file_module_semantics(target)
                    .emit_module_kind;
                let enclosing_emit = self
                    .inputs
                    .bindings
                    .file_module_semantics(enclosing)
                    .emit_module_kind;
                if target_emit == ModuleKind::EsNext && target_emit != enclosing_emit {
                    specifier = sequence.read(self.diagnostic_module_specifier_completion(
                        symbol,
                        context,
                        Some(ResolutionMode::Import),
                        consumer,
                    ))?;
                    attributes = Some(ResolutionMode::Import);
                }
            }
            if specifier.is_empty() {
                specifier = sequence.read(
                    self.diagnostic_module_specifier_completion(symbol, context, None, consumer),
                )?;
            }
            if !context.flags.has(1 << 26) && specifier.contains("/node_modules/") {
                let original = specifier.clone();
                if node_resolution {
                    let enclosing = context_file.ok_or_else(|| {
                        super::super::super::internal(
                            "portable import retry has no enclosing source file",
                        )
                    })?;
                    let mode = if self
                        .inputs
                        .bindings
                        .file_module_semantics(enclosing)
                        .emit_module_kind
                        == ModuleKind::EsNext
                    {
                        ResolutionMode::Require
                    } else {
                        ResolutionMode::Import
                    };
                    specifier = sequence.read(self.diagnostic_module_specifier_completion(
                        symbol,
                        context,
                        Some(mode),
                        consumer,
                    ))?;
                    if specifier.contains("/node_modules/") {
                        specifier = original;
                    } else {
                        attributes = Some(mode);
                    }
                }
                if attributes.is_none() {
                    context.encountered_error = true;
                    context.report_diagnostic();
                }
            }
            Ok((specifier, attributes))
        })();
        sequence.finish(result)
    }

    /// `getSpecifierForModuleSymbol`. Declaration/export= probing happens on
    /// every call, before shortcuts or cache hits. Cached completions retain
    /// the evidence of the spelling worker, separately from that prefix.
    pub(super) fn diagnostic_module_specifier_completion(
        &mut self,
        symbol: ReferenceSymbol,
        context: &Context,
        override_mode: Option<ResolutionMode>,
        consumer: &Location,
    ) -> NativeResult<String> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let declarations = self.reference_symbol_declarations(symbol, consumer)?;
            let mut file = false;
            for declaration in &declarations {
                if self.diagnostic_source_file_declaration(*declaration)? {
                    file = true;
                    break;
                }
            }
            if !file {
                for declaration in declarations {
                    if let Some(equivalent) =
                        sequence.read(self.file_symbol_if_export_equals_container_completion(
                            declaration,
                            symbol,
                            consumer,
                        ))?
                    {
                        for declaration in
                            self.reference_symbol_declarations(equivalent, consumer)?
                        {
                            if self.diagnostic_source_file_declaration(declaration)? {
                                file = true;
                                break;
                            }
                        }
                        break;
                    }
                }
            }
            let native = self.reference_symbol_name(symbol, consumer)?;
            let ambient = native.first() == Some(&b'"') && native.last() == Some(&b'"');
            if (!file || context.enclosing_file.is_none()) && ambient {
                let name = String::from_utf8(native).map_err(|_| {
                    self.diagnostic_dependency(
                        consumer,
                        "native module display name is not representable as exact text",
                    )
                })?;
                return Ok(crate::strings::strip_quotes(&name).to_owned());
            }
            let Some(enclosing_file) = context.enclosing_file else {
                let file = self
                    .source_file_of_module(symbol, consumer)?
                    .ok_or_else(|| {
                        self.diagnostic_dependency(
                            consumer,
                            "native module source-file declaration is unavailable",
                        )
                    })?;
                return self
                    .inputs
                    .sources
                    .get(file)
                    .path
                    .to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        self.diagnostic_dependency(
                            consumer,
                            "native module file name is not representable as exact text",
                        )
                    });
            };
            // MostOriginal(fake Block) is still the fake Block, not its parent
            // import. Captured nodes have their original retained identity.
            let original = match context.enclosing {
                Some(ScopeLocation::Captured(declaration)) => self
                    .source
                    .diagnostic_module_specifier_mode(enclosing_file, declaration)?,
                Some(ScopeLocation::Synthetic(_)) | None => None,
            };
            let resolution_mode = override_mode.or_else(|| {
                original.unwrap_or(
                    self.inputs
                        .bindings
                        .file_module_semantics(enclosing_file)
                        .default_resolution_mode,
                )
            });
            let key = (
                self.reference_symbol_identity(symbol, consumer)?,
                enclosing_file,
                resolution_mode,
            );
            if let Some(value) = self.native_display.specifiers.values.get(&key).cloned() {
                return sequence.read(Ok(value));
            }
            // The effective mode selects preferences/cache identity. Native
            // GetModuleSpecifiers receives the original override separately.
            let value = self.diagnostic_module_specifier_worker(
                symbol,
                enclosing_file,
                resolution_mode,
                override_mode,
                consumer,
            )?;
            self.native_display
                .specifiers
                .values
                .insert(key, value.clone());
            sequence.read(Ok(value))
        })();
        sequence.finish(result)
    }

    fn diagnostic_module_specifier_worker(
        &mut self,
        symbol: ReferenceSymbol,
        enclosing_file: FileId,
        preference_mode: Option<ResolutionMode>,
        override_mode: Option<ResolutionMode>,
        consumer: &Location,
    ) -> NativeResult<String> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(name) =
                sequence.read(self.diagnostic_ambient_module_specifier(symbol, consumer))?
            {
                if !name.is_empty() {
                    return Ok(name);
                }
            }
            let Some(target) = self.source_file_of_module(symbol, consumer)? else {
                return Ok(String::new());
            };
            use crate::project::program::specifier_environment::SpecifierInputError;
            use crate::project::program::specifiers::{
                ImportingFile, WrittenImport, module_specifiers,
            };
            let mut imports = Vec::new();
            for (site, _) in self.source.reference_source_imports(enclosing_file)? {
                let bindings = self.inputs.bindings;
                imports.push(WrittenImport {
                    text: bindings.reference_module_specifier(site).to_owned(),
                    resolved_file_name: bindings
                        .reference_resolved_module_path(site)
                        .map(str::to_owned),
                    mode: bindings.reference_module_site(site).resolution_mode,
                });
            }
            let importer_name =
                crate::input::exact_path(&self.inputs.sources.get(enclosing_file).path)
                    .map_err(crate::analysis::AnalysisError::from)?;
            let target_name = crate::input::exact_path(&self.inputs.sources.get(target).path)
                .map_err(crate::analysis::AnalysisError::from)?;
            let importer = ImportingFile {
                file_name: importer_name,
                default_mode: self
                    .inputs
                    .bindings
                    .file_module_semantics(enclosing_file)
                    .default_resolution_mode,
                imports: &imports,
            };
            let specifiers = module_specifiers(
                self.inputs.specifier_environment(),
                importer,
                target_name,
                preference_mode,
                override_mode,
            )
            .map_err(|error| -> DemandError {
                match error {
                    SpecifierInputError::Input(error) => {
                        crate::analysis::AnalysisError::from(error).into()
                    }
                    SpecifierInputError::NotCaptured { operation, path } => {
                        super::super::super::internal(&format!(
                            "module specifier {operation} input was not captured: {path}"
                        ))
                        .into()
                    }
                }
            })?;
            Ok(specifiers.into_iter().next().unwrap_or_default())
        })();
        sequence.finish(result)
    }

    /// `tryGetModuleNameFromAmbientModule`. Its second pass uses raw exports,
    /// the actual export-assignment value declaration, GetSymbolAtLocation,
    /// and finally raw declaration-symbol identity (without merging it).
    fn diagnostic_ambient_module_specifier(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<String>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let declarations = self.reference_symbol_declarations(symbol, consumer)?;
            for declaration in &declarations {
                let ReferenceDeclaration::Captured(declaration) = *declaration else {
                    continue;
                };
                if let Some(name) = self.source.diagnostic_ambient_module_name(declaration)?
                    && (!self
                        .source
                        .reference_is_external_augmentation(declaration)?
                        || !crate::project::program::is_relative_name(&name))
                {
                    return Ok(Some(name));
                }
            }
            for declaration in declarations {
                let ReferenceDeclaration::Captured(declaration) = declaration else {
                    continue;
                };
                let Some(container) = self
                    .source
                    .diagnostic_ambient_module_container(declaration)?
                else {
                    continue;
                };
                let parent = self
                    .reference_raw_symbol_of_declaration(container)?
                    .ok_or_else(|| {
                        super::super::super::internal("ambient module has no raw symbol")
                    })?;
                let Some(exports) = self.reference_raw_symbol_table(parent, true)? else {
                    continue;
                };
                let Some(export) = self.reference_raw_table_lookup(
                    exports,
                    &NativePropertyKey::fixed(b"export=".as_slice()),
                    consumer,
                )?
                else {
                    continue;
                };
                let Some(ReferenceDeclaration::Captured(value)) =
                    self.reference_symbol_value_declaration(export, consumer)?
                else {
                    continue;
                };
                if self.source.declaration_kind(value)? != K::ExportAssignment {
                    continue;
                }
                let SymbolAliasTarget::Expression { root, .. } =
                    self.source.symbol_alias_target(value)?
                else {
                    return Err(super::super::super::internal(
                        "export assignment lost its expression",
                    )
                    .into());
                };
                let Some(mut exported) =
                    sequence.read(self.reference_symbol_at_node_completion(root, consumer))?
                else {
                    continue;
                };
                if self.reference_symbol_flags(exported, consumer)?.alias {
                    let Some(target) =
                        sequence.read(self.resolve_alias_target_completion(exported, consumer))?
                    else {
                        return Err(super::super::super::internal(
                            "ambient alias resolution returned nil",
                        )
                        .into());
                    };
                    exported = target;
                }
                if let Some(raw) = self.reference_raw_symbol_of_declaration(declaration)?
                    && self.reference_symbol_identity(exported, consumer)?
                        == self.reference_symbol_identity(raw, consumer)?
                {
                    return self.source.diagnostic_ambient_module_name(container);
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    fn diagnostic_source_file_declaration(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> Result<bool, DemandError> {
        Ok(match declaration {
            ReferenceDeclaration::Captured(declaration) => {
                self.source.declaration_kind(declaration)? == K::SourceFile
            }
            ReferenceDeclaration::Json { declaration, .. } => {
                declaration == crate::project::binding::JsonDeclaration::SourceFile
            }
        })
    }
}
