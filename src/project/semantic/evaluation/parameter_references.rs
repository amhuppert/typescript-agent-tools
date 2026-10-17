//! Native outer-parameter containment uses actual checker symbol identities.
//! Source supplies ordered syntax steps; it never resolves a name here.

use super::super::model::{NativeResult, NativeSequence, TypeId, TypeView};
use super::super::source::{
    DeclarationIdentity, DeclarationReference, ParameterReferenceSyntax, TypeExpressionForm,
};
use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use crate::{references::Location, syntax_kind::SyntaxKind};

impl Evaluator<'_, '_> {
    /// `isTypeParameterPossiblyReferenced` shares actual symbol equality with
    /// `getSymbolFromTypeReference`, and raw identifier declarations with
    /// `getResolvedSymbol`. Neither operation acquires a referenced value type.
    pub(super) fn parameter_possibly_referenced_completion(
        &mut self,
        declarations: &[DeclarationReference],
        parameter: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let TypeView::TypeParameter(view) = self.model.get(parameter)? else {
                return Err(super::super::internal(
                    "parameter containment requires an actual type parameter",
                )
                .into());
            };
            let Some(symbol) = self.reference_symbol_of_type(parameter, consumer)? else {
                return Ok(true);
            };
            let own = self.reference_symbol_declarations(symbol, consumer)?;
            if own.len() != 1 {
                return Ok(true);
            }
            let ReferenceDeclaration::Captured(own) = own[0] else {
                return Err(
                    super::super::internal("type parameter has a non-syntax declaration").into(),
                );
            };
            let container = self.source.declaration_parent(own)?;
            if declarations.is_empty() {
                return Err(super::super::internal(
                    "parameter reference question has no actual declaration",
                )
                .into());
            }
            for &declaration in declarations {
                let declaration = self.source.resolve_declaration_reference(declaration)?;
                let mut ancestor = Some(declaration);
                while ancestor != container {
                    let Some(current) = ancestor else {
                        return Ok(true);
                    };
                    let (parent, block, extends) =
                        self.source.parameter_reference_ancestor(current)?;
                    if block {
                        return Ok(true);
                    }
                    if let Some(extends) = extends
                        && sequence.read(self.contains_parameter_reference_completion(
                            extends,
                            symbol,
                            own,
                            view.is_this,
                            consumer,
                        ))?
                    {
                        return Ok(true);
                    }
                    ancestor = parent;
                }
                if sequence.read(self.contains_parameter_reference_completion(
                    declaration,
                    symbol,
                    own,
                    view.is_this,
                    consumer,
                ))? {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    fn contains_parameter_reference_completion(
        &mut self,
        root: DeclarationIdentity,
        parameter: ReferenceSymbol,
        own: DeclarationIdentity,
        is_this: bool,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut pending = vec![root];
            while let Some(node) = pending.pop() {
                match self.source.parameter_reference_syntax(node, is_this)? {
                    ParameterReferenceSyntax::Known(true) => return Ok(true),
                    ParameterReferenceSyntax::Known(false) => {}
                    ParameterReferenceSyntax::Reference(expression) => {
                        let (site, anchor) = match &expression.form {
                            TypeExpressionForm::Reference { site, anchor, .. } => (*site, *anchor),
                            TypeExpressionForm::Pending { reason, message } => {
                                return Err(super::super::unsupported_demand_at_location(
                                    self.inputs,
                                    &expression.location,
                                    *reason,
                                    message.clone(),
                                ));
                            }
                            _ => {
                                return Err(super::super::internal(
                                    "parameter reference syntax lost its type-reference recipe",
                                )
                                .into());
                            }
                        };
                        let referenced = sequence.read(self.symbol_from_type_reference(
                            &expression,
                            site,
                            anchor,
                        ))?;
                        if self.reference_symbol_identity(referenced, consumer)?
                            == self.reference_symbol_identity(parameter, consumer)?
                        {
                            return Ok(true);
                        }
                    }
                    ParameterReferenceSyntax::Query { first, arguments } => {
                        let first_declaration =
                            self.source.declaration_from_expression_root(first)?;
                        let location = self.source.declaration_location(first_declaration)?;
                        let referenced = sequence.read(
                            self.reference_resolved_identifier_symbol_completion(first, &location),
                        )?;
                        let scope =
                            if self.source.declaration_kind(own)? == SyntaxKind::TypeParameter {
                                self.source.declaration_parent(own)?
                            } else if is_this {
                                Some(own)
                            } else {
                                None
                            };
                        let Some(scope) = scope else {
                            return Ok(true);
                        };
                        if !self.is_unknown_reference_symbol(referenced) {
                            for declaration in
                                self.reference_symbol_declarations(referenced, &location)?
                            {
                                if let ReferenceDeclaration::Captured(declaration) = declaration
                                    && self.source.declaration_is_descendant(declaration, scope)?
                                {
                                    return Ok(true);
                                }
                            }
                        }
                        pending.extend(arguments.into_iter().rev());
                    }
                    ParameterReferenceSyntax::Children(children) => {
                        pending.extend(children.into_iter().rev())
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }
}
