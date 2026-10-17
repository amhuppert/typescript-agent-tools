//! Native parameter-list acquisition over actual symbol declarations. The
//! node builder, alias and class factories share these semantic questions;
//! none reconstructs parameters from printed names or a type's display shape.
use super::super::model::{DemandError, NativeResult, NativeSequence, TypeId};
use super::super::source::DeclarationIdentity;
use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;

impl Evaluator<'_, '_> {
    /// `appendTypeParameters` acquires every declaration in order through
    /// `getSymbolOfDeclaration`, so a merged global interface's declarations
    /// share one type parameter, then tests canonical type identity. Defaults
    /// and constraints remain lazy.
    pub(super) fn append_type_parameters_completion(
        &mut self,
        mut parameters: Vec<TypeId>,
        declarations: &[DeclarationIdentity],
    ) -> NativeResult<Vec<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for &declaration in declarations {
                let symbol =
                    sequence.read(self.reference_symbol_of_declaration_completion(declaration))?;
                let consumer = self.source.declaration_location(declaration)?;
                let ty =
                    sequence.read(self.declared_reference_type_completion(symbol, &consumer))?;
                if !parameters.contains(&ty) {
                    parameters.push(ty);
                }
            }
            Ok(parameters)
        })();
        sequence.finish(result)
    }

    /// `getLocalTypeParametersOfClassOrInterfaceOrTypeAlias` and its append
    /// worker select complete native declaration lists, including copied
    /// transient headers. Publication and caching belong to their callers.
    pub(super) fn local_type_parameters_of_symbol_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut parameters = Vec::new();
            for declaration in self.reference_symbol_declarations(symbol, consumer)? {
                if !matches!(
                    self.reference_declaration_kind(declaration)?,
                    K::ClassDeclaration
                        | K::ClassExpression
                        | K::InterfaceDeclaration
                        | K::TypeAliasDeclaration
                ) {
                    continue;
                }
                let ReferenceDeclaration::Captured(declaration) = declaration else {
                    return Err(super::super::internal(
                        "JSON declaration cannot own type parameters",
                    )
                    .into());
                };
                let declarations = self.source.declaration_type_parameters(declaration)?;
                parameters = sequence
                    .read(self.append_type_parameters_completion(parameters, &declarations))?;
            }
            Ok(parameters)
        })();
        sequence.finish(result)
    }

    /// `getOuterTypeParametersOfClassOrInterface`: Class/Function uses the
    /// actual ValueDeclaration; other symbols use the first interface or
    /// function-initialized variable declaration before walking ancestors.
    pub(super) fn outer_type_parameters_of_symbol(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Vec<TypeId>, DemandError> {
        let flags = self.reference_symbol_flags(symbol, consumer)?;
        let declaration = if flags.class || flags.function {
            self.reference_symbol_value_declaration(symbol, consumer)?
        } else {
            let mut found = None;
            for declaration in self.reference_symbol_declarations(symbol, consumer)? {
                if let ReferenceDeclaration::Captured(captured) = declaration
                    && self
                        .source
                        .outer_parameter_declaration_candidate(captured)?
                {
                    found = Some(declaration);
                    break;
                }
            }
            found
        };
        let Some(ReferenceDeclaration::Captured(declaration)) = declaration else {
            return Err(self.access_pending(
                consumer,
                "native outer type parameters require their actual declaration",
            ));
        };
        self.outer_type_parameters(declaration, false, consumer)
    }
    /// `getConstraintDeclaration` selects the first nonnil annotation from
    /// the actual parameter symbol, including instantiated/copied symbols.
    pub(super) fn constraint_declaration(
        &mut self,
        parameter: TypeId,
        consumer: &Location,
    ) -> Result<Option<super::super::source::DeclarationIdentity>, DemandError> {
        let Some(symbol) = self.reference_symbol_of_type(parameter, consumer)? else {
            return Ok(None);
        };
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            if let ReferenceDeclaration::Captured(declaration) = declaration
                && self.source.declaration_kind(declaration)? == K::TypeParameter
                && let Some(constraint) = self.source.pseudo_child(declaration, "constraint")?
            {
                return Ok(Some(constraint));
            }
        }
        Ok(None)
    }
}
