//! Identifier deprecation is a symbol operation. It shares the parser's lazy
//! tag confirmation but never serializes a type merely to emit a suggestion.
use super::*;
use crate::project::semantic::evaluation::{
    ReferenceDeclaration, ReferenceSymbol as SemanticSymbol,
};
use crate::project::semantic::model::NativeSequence;
use crate::project::semantic::source::{ExpressionRoot, identifiers::UncalledReference};

impl Evaluator<'_, '_> {
    /// `resolveAliasWithDeprecationCheck` and the following checkIdentifier
    /// suggestion gate keep alias intermediates separate from the value read.
    pub(super) fn check_identifier_deprecation(
        &mut self,
        symbol: SemanticSymbol,
        node: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let target =
                sequence.read(self.alias_with_deprecation_check_completion(symbol, consumer))?;
            let declarations = self.reference_symbol_declarations(target, consumer)?;
            if !declarations.is_empty()
                && self.is_deprecated_symbol(target, consumer)?
                && self.is_uncalled_function_reference(node, target, consumer)?
            {
                self.add_deprecated_symbol_suggestion(&declarations)?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    pub(in crate::project::semantic::evaluation) fn alias_with_deprecation_check_completion(
        &mut self,
        mut symbol: SemanticSymbol,
        consumer: &Location,
    ) -> NativeResult<SemanticSymbol> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !self.reference_symbol_flags(symbol, consumer)?.alias
                || self.is_deprecated_symbol(symbol, consumer)?
            {
                return Ok(symbol);
            }
            let has_declaration = self
                .reference_alias_declaration(symbol, consumer)?
                .is_some();
            if !has_declaration {
                return Ok(symbol);
            }
            let target = sequence
                .read(self.resolve_alias_target_completion(symbol, consumer))?
                .ok_or_else(|| super::super::super::internal("resolveAlias returned native nil"))?;
            if self.is_unknown_reference_symbol(target) {
                return Ok(target);
            }
            let target_identity = self.reference_symbol_identity(target, consumer)?;
            while self.reference_symbol_flags(symbol, consumer)?.alias {
                let Some(immediate) = sequence
                    .read(self.reference_immediate_aliased_symbol_completion(symbol, consumer))?
                else {
                    break;
                };
                if self.reference_symbol_identity(immediate, consumer)? == target_identity {
                    break;
                }
                let declarations = self.reference_symbol_declarations(immediate, consumer)?;
                if !declarations.is_empty() {
                    if self.is_deprecated_symbol(immediate, consumer)? {
                        self.add_deprecated_symbol_suggestion(&declarations)?;
                        break;
                    }
                    if self.reference_symbol_identity(symbol, consumer)? == target_identity {
                        break;
                    }
                    symbol = immediate;
                } else {
                    // Native loops only with declared alias intermediates. An
                    // admitted factory with a declaration-less intermediate needs
                    // its actual progression contract, not a guessed target skip.
                    return Err(self.access_pending(
                        consumer,
                        "native undeclared alias deprecation intermediate requires its factory",
                    ));
                }
            }
            Ok(target)
        })();
        sequence.finish(result)
    }

    /// `isDeprecatedSymbol`: parent acquisition precedes declaration tests;
    /// interface overloads use Some, other merged symbols use Every.
    pub(in crate::project::semantic::evaluation) fn is_deprecated_symbol(
        &mut self,
        symbol: SemanticSymbol,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        let parent = self.reference_parent_of_symbol(symbol, consumer)?;
        let declarations = self.reference_symbol_declarations(symbol, consumer)?;
        if let Some(parent) = parent
            && declarations.len() > 1
        {
            let some = self.reference_symbol_flags(parent, consumer)?.interface;
            for declaration in declarations {
                if self.deprecated_reference_declaration(declaration)? == some {
                    return Ok(some);
                }
            }
            return Ok(!some);
        }
        if let Some(declaration) = self.reference_symbol_value_declaration(symbol, consumer)?
            && self.deprecated_reference_declaration(declaration)?
        {
            return Ok(true);
        }
        if declarations.is_empty() {
            return Ok(false);
        }
        for declaration in declarations {
            if !self.deprecated_reference_declaration(declaration)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn deprecated_reference_declaration(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> Result<bool, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => {
                self.source.deprecated_declaration(declaration)
            }
            ReferenceDeclaration::Json { .. } => Ok(false),
        }
    }

    /// `addDeprecatedSuggestionWorker` stops at the first actual tag. JSON
    /// grammar has no JSDoc attachment and contributes no invented syntax.
    pub(in crate::project::semantic::evaluation) fn add_deprecated_symbol_suggestion(
        &mut self,
        declarations: &[ReferenceDeclaration],
    ) -> Result<(), DemandError> {
        for &declaration in declarations {
            if let ReferenceDeclaration::Captured(declaration) = declaration
                && self.source.jsdoc_deprecated_tag(declaration)?.is_some()
            {
                break;
            }
        }
        Ok(())
    }

    pub(in crate::project::semantic::evaluation) fn is_uncalled_function_reference(
        &mut self,
        node: ExpressionRoot,
        symbol: SemanticSymbol,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        let flags = self.reference_symbol_flags(symbol, consumer)?;
        if !flags.function && !flags.method {
            return Ok(true);
        }
        match self.source.uncalled_reference_context(node)? {
            UncalledReference::Call(call) => {
                if node.site.kind != SyntaxKind::Identifier {
                    return Ok(false);
                }
                self.has_matching_argument(call.file, call.site, node.into(), consumer)
            }
            UncalledReference::OtherCall => Ok(false),
            UncalledReference::Declarations => {
                for declaration in self.reference_symbol_declarations(symbol, consumer)? {
                    if crate::syntax::is_function_like_kind(
                        self.reference_declaration_kind(declaration)?,
                    ) && !self.deprecated_reference_declaration(declaration)?
                    {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }
}
