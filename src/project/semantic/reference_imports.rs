//! Language-service direction rules around the checker's alias and module
//! facts. The checker owns raw, immediate, final and merged symbol questions;
//! reference search owns which of those questions continuation requires.

use super::evaluation::SymbolParent;
use super::evaluation::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use super::model::DemandError;
use super::source::{ExpressionRoot, ReferenceExportKind, ReferenceExportSyntax, ReferenceSite};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ComingFrom {
    Unknown,
    Import,
    Export,
}

#[derive(Clone, Copy)]
pub(super) struct ExportInfo {
    pub(super) module: SymbolParent,
    pub(super) kind: ReferenceExportKind,
}

pub(super) enum Continuation {
    Import(ReferenceSymbol),
    Export {
        symbol: ReferenceSymbol,
        info: ExportInfo,
    },
}

/// Native `getExportInfo`: namespace exports use global search; only the
/// merged external-module parent starts ImportTracker traversal.
pub(super) fn export_info(
    evaluator: &mut Evaluator<'_, '_>,
    symbol: ReferenceSymbol,
    kind: ReferenceExportKind,
    consumer: &Location,
) -> Result<Option<ExportInfo>, DemandError> {
    let Some(module) = evaluator.reference_merged_symbol_parent(symbol, consumer)? else {
        return Ok(None);
    };
    Ok(evaluator
        .reference_parent_is_external_module(module, consumer)?
        .then_some(ExportInfo { module, kind }))
}

/// Native `getImportOrExportSymbol`, for the admitted TypeScript syntax.
/// Export acquisition precedes import acquisition; comingFromExport prevents
/// walking immediately back through the alias just followed into this file.
pub(super) fn continuation(
    inventory: &crate::project::binding::Bindings,
    evaluator: &mut Evaluator<'_, '_>,
    site: &ReferenceSite,
    symbol: ReferenceSymbol,
    coming_from: ComingFrom,
) -> Result<Option<Continuation>, DemandError> {
    let Some(syntax) = evaluator.source().reference_import_export_syntax(site)? else {
        return Ok(None);
    };
    let consumer = &site.selection;
    let exported = if let Some(exported) = evaluator.reference_export_symbol(symbol, consumer)? {
        if syntax.property_access {
            // The remaining native property-access branch is CommonJS
            // assignment declaration checking, outside the TS input profile.
            None
        } else {
            export_info(evaluator, exported, syntax.parent_export_kind, consumer)?.map(|info| {
                Continuation::Export {
                    symbol: exported,
                    info,
                }
            })
        }
    } else {
        match syntax.export {
            ReferenceExportSyntax::None => None,
            ReferenceExportSyntax::Declaration {
                kind: _,
                import_equals_lhs: Some(lhs),
            } => {
                if coming_from == ComingFrom::Export {
                    None
                } else {
                    symbol_at_root(inventory, evaluator, lhs)?.map(Continuation::Import)
                }
            }
            ReferenceExportSyntax::Declaration {
                kind,
                import_equals_lhs: None,
            } => export_info(evaluator, symbol, kind, consumer)?
                .map(|info| Continuation::Export { symbol, info }),
            ReferenceExportSyntax::Assignment { declaration, kind } => {
                let assignment = evaluator
                    .reference_raw_symbol_of_declaration(declaration)?
                    .ok_or_else(|| {
                        super::internal("export assignment lacks its raw declaration symbol")
                    })?;
                evaluator
                    .reference_symbol_parent(assignment, consumer)?
                    .map(|module| Continuation::Export {
                        symbol,
                        info: ExportInfo { module, kind },
                    })
            }
        }
    };
    if exported.is_some() || coming_from == ComingFrom::Export || !syntax.import {
        return Ok(exported);
    }
    let imported = if evaluator.reference_symbol_flags(symbol, consumer)?.alias {
        evaluator.reference_immediate_aliased_symbol(symbol, consumer)?
    } else {
        // The native non-alias import case is a JavaScript require binding.
        // isNodeImport never admits that syntax in this input profile.
        None
    };
    let Some(imported) = imported else {
        return Ok(None);
    };
    let imported = skip_export_specifier(evaluator, imported, consumer)?;
    let imported = if evaluator.reference_symbol_name(imported, consumer)? == b"export=" {
        let Some(imported) = export_equals_local_symbol(evaluator, imported, consumer)? else {
            return Ok(None);
        };
        imported
    } else {
        imported
    };
    let name = symbol_name_no_default(evaluator, imported, consumer)?;
    Ok((name.is_empty()
        || name == b"default"
        || name == evaluator.reference_symbol_name(symbol, consumer)?)
    .then_some(Continuation::Import(imported)))
}

/// Native `skipExportSpecifierSymbol` stops on the first unrenamed local
/// export declaration, including a real absent local target's raw fallback.
fn skip_export_specifier(
    evaluator: &mut Evaluator<'_, '_>,
    symbol: ReferenceSymbol,
    consumer: &Location,
) -> Result<ReferenceSymbol, DemandError> {
    for declaration in evaluator.reference_symbol_declarations(symbol, consumer)? {
        let ReferenceDeclaration::Captured(declaration) = declaration else {
            continue;
        };
        if evaluator
            .source()
            .reference_is_unrenamed_local_export(declaration)?
        {
            return Ok(evaluator
                .reference_export_specifier_local_target(declaration, consumer)?
                .unwrap_or(symbol));
        }
    }
    Ok(symbol)
}

/// Native `getExportEqualsLocalSymbol` uses the expression's raw Symbol,
/// not GetSymbolAtLocation and not the final alias target.
fn export_equals_local_symbol(
    evaluator: &mut Evaluator<'_, '_>,
    symbol: ReferenceSymbol,
    consumer: &Location,
) -> Result<Option<ReferenceSymbol>, DemandError> {
    if evaluator.reference_symbol_flags(symbol, consumer)?.alias {
        return evaluator.reference_immediate_aliased_symbol(symbol, consumer);
    }
    let Some(ReferenceDeclaration::Captured(declaration)) =
        evaluator.reference_symbol_value_declaration(symbol, consumer)?
    else {
        return Ok(None);
    };
    let Some(root) = evaluator
        .source()
        .reference_export_equals_symbol_node(declaration)?
    else {
        return Ok(None);
    };
    evaluator.reference_raw_symbol_of_node(root)
}

fn symbol_name_no_default(
    evaluator: &mut Evaluator<'_, '_>,
    symbol: ReferenceSymbol,
    consumer: &Location,
) -> Result<Vec<u8>, DemandError> {
    let name = evaluator.reference_symbol_name(symbol, consumer)?;
    if name != b"default" {
        return Ok(name);
    }
    for declaration in evaluator.reference_symbol_declarations(symbol, consumer)? {
        if let ReferenceDeclaration::Captured(declaration) = declaration
            && let Some(name) = evaluator
                .source()
                .reference_identifier_declaration_name(declaration)?
        {
            return Ok(name.into_bytes());
        }
    }
    Ok(Vec::new())
}

/// `createSearch` obtains its text from the local default-export symbol, then
/// the non-module declaration of a merged symbol, then SymbolName itself.
/// The selected member's decoded inventory uses its exact syntax key instead
/// of this raw scanner hint. Still observe the native name in the same order:
/// a computed declaration's internal name need not be representable as UTF-8.
pub(super) fn search_key(
    evaluator: &mut Evaluator<'_, '_>,
    symbol: ReferenceSymbol,
    consumer: &Location,
    decoded_seed_key: Option<&str>,
) -> Result<String, DemandError> {
    let declarations = evaluator.reference_symbol_declarations(symbol, consumer)?;
    let mut named = symbol;
    let mut local_default = false;
    if let Some(ReferenceDeclaration::Captured(first)) = declarations.first()
        && evaluator
            .source()
            .declaration_has_modifier(*first, SyntaxKind::DefaultKeyword)?
    {
        for declaration in &declarations {
            if let ReferenceDeclaration::Captured(declaration) = declaration
                && let Some(local) = evaluator.reference_declaration_local_symbol(*declaration)?
            {
                named = local;
                local_default = true;
                break;
            }
        }
    }
    if !local_default && !declarations.is_empty() {
        let flags = evaluator.reference_symbol_flags(symbol, consumer)?;
        if flags.module || flags.transient {
            for declaration in declarations {
                if let ReferenceDeclaration::Captured(declaration) = declaration
                    && !matches!(
                        evaluator.source().declaration_kind(declaration)?,
                        SyntaxKind::SourceFile | SyntaxKind::ModuleDeclaration
                    )
                {
                    let root = evaluator
                        .source()
                        .declaration_expression_root(declaration)?;
                    if let Some(raw) = evaluator.reference_raw_symbol_of_node(root)? {
                        named = raw;
                    }
                    break;
                }
            }
        }
    }
    if let Some(key) = decoded_seed_key {
        let _native_name = evaluator.reference_symbol_display_name_bytes(named, consumer)?;
        return Ok(crate::references::metadata::copy_text(key));
    }
    let name = evaluator.reference_symbol_display_name(named, consumer)?;
    Ok(crate::strings::strip_quotes(&name).to_owned())
}

pub(super) fn symbol_at_root(
    inventory: &crate::project::binding::Bindings,
    evaluator: &mut Evaluator<'_, '_>,
    root: ExpressionRoot,
) -> Result<Option<ReferenceSymbol>, DemandError> {
    let Some(site) = evaluator.source().reference_node_site(inventory, root)? else {
        return Err(
            super::internal("reference import/export name lacks its inventory role").into(),
        );
    };
    evaluator.reference_symbol_at_site(&site)
}
