//! Diagnostic-only alias work can resolve symbols and acquire display types.
//! These owners preserve those native dependencies before unknownSymbol is
//! installed; product selection owns rejection of that final target.
use super::super::model::{NativeCompletion, NativeResult, NativeSequence};
use super::super::source::DeclarationIdentity;
use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use crate::project::binding::NativePropertyKey;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;

impl Evaluator<'_, '_> {
    /// `getFullyQualifiedName` recurses through raw Parent. Only the root
    /// keeps the containing location and suppresses symbol-chain lookup.
    pub(super) fn fully_qualified_symbol_display_completion(
        &mut self,
        symbol: ReferenceSymbol,
        enclosing: Option<DeclarationIdentity>,
        consumer: &Location,
    ) -> NativeResult<()> {
        if let Some(parent) = self.reference_raw_parent_symbol(symbol, consumer)? {
            let mut sequence = NativeSequence::new();
            let result = (|| {
                sequence.read(
                    self.fully_qualified_symbol_display_completion(parent, enclosing, consumer),
                )?;
                sequence.read(self.diagnostic_symbol_display_completion(symbol, consumer))?;
                Ok(())
            })();
            sequence.finish(result)
        } else {
            Ok(self
                .diagnostic_unchained_symbol_display_completion(symbol, enclosing, consumer)?
                .map(|_| ()))
        }
    }

    /// `errorNoModuleMemberSymbol`: NoCheck gates the entire diagnostic
    /// acquisition, and a string-literal name never asks for a suggestion.
    pub(super) fn missing_module_member_completion(
        &mut self,
        module: ReferenceSymbol,
        target: ReferenceSymbol,
        alias: DeclarationIdentity,
        enclosing: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<()> {
        if self.inputs.semantic_options().no_check {
            return Ok(NativeCompletion::clean(()));
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            sequence.read(self.fully_qualified_symbol_display_completion(
                module,
                Some(enclosing),
                consumer,
            ))?;
            let (name, identifier) = self.source.alias_diagnostic_name(alias)?;
            let suggestion = if identifier {
                let exports =
                    sequence.read(self.reference_exports_of_module_completion(target, consumer))?;
                let candidates = exports
                    .iter()
                    .flat_map(|table| table.iter().map(|(_, symbol)| *symbol))
                    .collect();
                sequence.read(self.spelling_suggestion_for_symbols_completion(
                    &name,
                    candidates,
                    super::spelling::SuggestionMeaning::ModuleMember,
                    consumer,
                ))?
            } else {
                None
            };
            if let Some(suggestion) = suggestion {
                sequence.read(self.diagnostic_symbol_display_completion(suggestion, consumer))?;
                // Related-info attaches its already-retained declaration; it
                // neither resolves the alias nor consumes the declared type.
                self.reference_symbol_value_declaration(suggestion, consumer)?;
            } else if self
                .reference_own_module_export(module, "default", consumer)?
                .is_none()
            {
                sequence.read(self.non_exported_member_completion(module, &name, consumer))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `reportNonExportedMember` reads ValueDeclaration.Locals[name] and
    /// raw Exports. Only an actual local triggers alias/reference comparison.
    fn non_exported_member_completion(
        &mut self,
        module: ReferenceSymbol,
        name: &str,
        consumer: &Location,
    ) -> NativeResult<()> {
        let local = match self.reference_symbol_value_declaration(module, consumer)? {
            Some(ReferenceDeclaration::Captured(declaration)) => {
                let native = self.source.declaration_ordering(declaration)?;
                let kind = self.source.declaration_kind(declaration)?;
                self.source
                    .bindings()
                    .reference_raw_local_lookup(
                        native.file,
                        kind,
                        native.span,
                        &NativePropertyKey::fixed(name.as_bytes()),
                    )
                    .map_err(|failure| self.generic_binding_failure(failure, consumer))?
                    .map(ReferenceSymbol::from_origin)
            }
            // JSON SourceFiles do not have a Locals producer.
            Some(ReferenceDeclaration::Json { .. }) => None,
            None => {
                return Err(super::super::internal("alias module has no ValueDeclaration").into());
            }
        };
        let Some(local) = local else {
            return Ok(NativeCompletion::clean(()));
        };
        if let Some(export) = self.reference_own_module_export(module, "export=", consumer)? {
            // reportInvalidImportEqualsExportMember formats only syntax and
            // moduleKind after this exact same-reference acquisition.
            return self
                .get_symbol_if_same_reference_completion(export, local, consumer)
                .map(|value| value.map(|_| ()));
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(raw) = self.reference_raw_symbol_table(module, true)? {
                for (_, exported) in self.reference_raw_table_entries(raw, consumer)?.iter() {
                    if sequence
                        .read(
                            self.get_symbol_if_same_reference_completion(
                                *exported, local, consumer,
                            ),
                        )?
                        .is_some()
                    {
                        sequence
                            .read(self.diagnostic_symbol_display_completion(*exported, consumer))?;
                        break;
                    }
                }
            }
            self.reference_symbol_declarations(local, consumer)?;
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `reportNonDefaultExport` has no NoCheck early return. Symbol display
    /// and export-star module resolution precede suppressed diagnostics too.
    pub(super) fn missing_default_export_completion(
        &mut self,
        module: ReferenceSymbol,
        alias: ReferenceSymbol,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<()> {
        if self.source.declaration_kind(declaration)? != K::ImportClause {
            return self.missing_module_member_completion(
                module,
                module,
                declaration,
                declaration,
                consumer,
            );
        }
        let key = self.reference_symbol_key(alias, consumer)?;
        let raw = self.reference_raw_symbol_table(module, true)?;
        let named = raw
            .map(|table| self.reference_raw_table_lookup(table, &key, consumer))
            .transpose()?
            .flatten();
        let mut sequence = NativeSequence::new();
        let result = (|| {
            sequence.read(self.diagnostic_symbol_display_completion(module, consumer))?;
            if named.is_some() {
                sequence.read(self.diagnostic_symbol_display_completion(alias, consumer))?;
            } else if let Some(raw) = raw {
                let star = self.reference_raw_table_lookup(
                    raw,
                    &NativePropertyKey::fixed(b"\xfeexport".as_slice()),
                    consumer,
                )?;
                if let Some(star) = star {
                    for declaration in self.reference_symbol_declarations(star, consumer)? {
                        let ReferenceDeclaration::Captured(declaration) = declaration else {
                            return Err(super::super::internal(
                                "export-star declaration is not captured syntax",
                            )
                            .into());
                        };
                        let Some(site) = self.source.export_declaration_module_site(declaration)?
                        else {
                            continue;
                        };
                        let module = self.reference_module_symbol(site, consumer)?;
                        if self
                            .reference_own_module_export(module, "default", consumer)?
                            .is_some()
                        {
                            break;
                        }
                    }
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }
}
