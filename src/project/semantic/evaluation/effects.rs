//! Native call-effect acquisition, shared by reads and end reachability.
//! Absence is established by the compiler's target/signature operations;
//! unsupported or invalid acquisition never means a call has no effect.

use super::super::model::{
    NativeCompletion, NativeResult, NativeSequence, Signature, SpecialName, TypeFlags, TypeId,
};
use super::super::source::{AccessKey, DottedName, ExplicitDeclarationType, Expression};
use super::reference_symbols::ReferenceDeclaration;
use super::{Evaluator, ReferenceSymbol};
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct EffectsCache(
    HashMap<(FileId, usize, usize, SyntaxKind), NativeCompletion<Option<Signature>>>,
);

impl Evaluator<'_, '_> {
    /// `getEffectsSignature`: acquire the actual call target, choose the
    /// native signature, and cache its known effect or absence. This question
    /// does not check unrelated call arguments or an annotated function body.
    pub(super) fn effects_signature_completion(
        &mut self,
        file: FileId,
        site: Site,
    ) -> NativeResult<Option<Signature>> {
        let key = (file, site.span.start, site.span.end, site.kind);
        if let Some(value) = self.effects.0.get(&key) {
            return Ok(value.clone());
        }
        // Native has no resolving sentinel here. Only a completed selection
        // (including unknownSignature projected as None) installs this slot.
        let value = self.acquire_effects_signature_completion(file, site)?;
        self.effects.0.insert(key, value.clone());
        Ok(value)
    }

    fn acquire_effects_signature_completion(
        &mut self,
        file: FileId,
        site: Site,
    ) -> NativeResult<Option<Signature>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let syntax = self.source.call_syntax(file, site)?;
            let ty = if syntax.binary {
                let right = sequence.read(self.check_expression_completion(
                    &syntax.callee,
                    super::contextual::CheckMode::NORMAL,
                ))?;
                let right = sequence
                    .read(self.check_non_null_type_completion(right, &syntax.callee.location))?;
                sequence.read(
                    self.symbol_has_instance_method_type_completion(right, &syntax.callee.location),
                )?
            } else if syntax.expression_statement {
                sequence.read(self.explicit_dotted_type_completion(&syntax.callee, false))?
            } else if syntax.super_call {
                None
            } else {
                let ty = sequence.read(self.check_expression_completion(
                    &syntax.callee,
                    super::contextual::CheckMode::NORMAL,
                ))?;
                let ty = if syntax.optional {
                    sequence.read(self.optional_expression_type_completion(
                        ty,
                        syntax.callee_optional_root,
                        syntax.callee_optional_chain,
                        &syntax.callee.location,
                    ))?
                } else {
                    ty
                };
                Some(
                    sequence
                        .read(self.check_non_null_type_completion(ty, &syntax.callee.location))?,
                )
            };
            let apparent = match ty {
                Some(ty) => sequence.read(self.apparent_type_completion(ty, &syntax.location))?,
                None => self.model.special(SpecialName::Unknown),
            };
            let signatures =
                sequence.read(self.call_signatures_completion(apparent, &syntax.location))?;
            let mut signature = None;
            if signatures.len() == 1 && signatures[0].type_parameter_types().is_empty() {
                signature = Some(signatures[0].clone());
            } else {
                for candidate in &signatures {
                    if sequence
                        .read(self.signature_has_effect_completion(candidate, &syntax.location))?
                    {
                        let root = super::super::source::CallRoot::new(file, site.span, site.kind);
                        let recipe = self.source.call_recipe(root)?;
                        signature = match sequence.read(self.resolved_signature_completion(
                            &recipe,
                            super::contextual::CheckMode::NORMAL,
                        ))? {
                            super::calls::ResolvedSignature::Signature(signature) => {
                                Some(signature)
                            }
                            super::calls::ResolvedSignature::Deferred => None,
                        };
                        break;
                    }
                }
            }
            if let Some(signature) = signature {
                if sequence
                    .read(self.signature_has_effect_completion(&signature, &syntax.location))?
                {
                    return Ok(Some(signature));
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `getSymbolHasInstanceMethodOfObjectType`: the known-symbol key is
    /// acquired before NonPrimitive eligibility, then property raw type and
    /// call inventory establish whether an actual method is present.
    pub(super) fn symbol_has_instance_method_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let key = sequence
                .read(self.property_key_for_known_symbol_completion("hasInstance", consumer))?;
            if sequence.read(self.all_types_assignable_to_kind_completion(
                ty,
                TypeFlags::NON_PRIMITIVE,
                false,
                consumer,
            ))? {
                if let Some(property) =
                    sequence.read(self.property_of_type_by_key_completion(ty, &key, consumer))?
                {
                    let symbol = self.property_symbol(&property, consumer)?;
                    let ty = sequence.read(self.property_raw_type_completion(symbol, consumer))?;
                    if !sequence
                        .read(self.call_signatures_completion(ty, consumer))?
                        .is_empty()
                    {
                        return Ok(Some(ty));
                    }
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `getTypeOfDottedName` uses raw symbol resolution and explicit declaration
    /// annotations, not ordinary identifier flow or initializer inference.
    pub(super) fn explicit_dotted_type_completion(
        &mut self,
        expression: &Expression,
        report_missing_annotation: bool,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let root = expression.root.ok_or_else(|| {
                super::super::internal("dotted-name question lacks its actual expression")
            })?;
            match self.source.dotted_name(root)? {
                DottedName::Absent => Ok(None),
                DottedName::Inner(inner) => sequence
                    .read(self.explicit_dotted_type_completion(&inner, report_missing_annotation)),
                DottedName::Identifier { file, anchor } => {
                    let symbol =
                        sequence.read(self.reference_resolved_identifier_symbol_completion(
                            super::super::source::ExpressionRoot {
                                file,
                                site: crate::project::binding::flow::Site {
                                    span: anchor,
                                    kind: crate::syntax_kind::SyntaxKind::Identifier,
                                },
                            },
                            &expression.location,
                        ))?;
                    if self.is_unknown_reference_symbol(symbol) {
                        Ok(None)
                    } else {
                        let symbol = self.reference_export_symbol_of_value_if_exported(
                            symbol,
                            &expression.location,
                        )?;
                        sequence.read(self.explicit_symbol_type_completion(
                            symbol,
                            report_missing_annotation,
                            &expression.location,
                        ))
                    }
                }
                DottedName::Access(root) => {
                    let recipe = self.source.access_recipe(root)?;
                    let Some(receiver) = sequence.read(self.explicit_dotted_type_completion(
                        &recipe.receiver,
                        report_missing_annotation,
                    ))?
                    else {
                        return Ok(None);
                    };
                    let AccessKey::Name {
                        name,
                        private,
                        location,
                        ..
                    } = recipe.key
                    else {
                        return Err(super::super::internal(
                            "dotted-name property has an element key",
                        )
                        .into());
                    };
                    let symbol = if private {
                        let Some(class) = self.reference_symbol_of_type(receiver, &location)?
                        else {
                            return Ok(None);
                        };
                        // `getTypeOfDottedName` keys against the receiver Type.symbol,
                        // not lexical class lookup used by checked private access.
                        let key = self.private_identifier_key(class, &name, &location)?;
                        sequence
                            .read(
                                self.property_of_type_by_key_completion(receiver, &key, &location),
                            )?
                            .map(|property| {
                                self.property_symbol(&property, &location)
                                    .map(ReferenceSymbol::from_property)
                            })
                            .transpose()?
                    } else {
                        sequence
                            .read(self.property_of_type_completion(receiver, &name, &location))?
                            .map(|property| {
                                self.property_symbol(&property, &location)
                                    .map(ReferenceSymbol::from_property)
                            })
                            .transpose()?
                    };
                    let Some(symbol) = symbol else {
                        return Ok(None);
                    };
                    sequence.read(self.explicit_symbol_type_completion(
                        symbol,
                        report_missing_annotation,
                        &location,
                    ))
                }
                DottedName::This { signature, class } => {
                    if let Some(root) = signature {
                        let recipe = self.source.signature_from_root(root)?;
                        let signature = sequence.read(self.signature_completion(&recipe))?;
                        if let Some(parameter) = signature.this_parameter() {
                            return sequence.read(self.explicit_symbol_type_completion(
                                ReferenceSymbol::from_property(parameter.symbol),
                                false,
                                &expression.location,
                            ));
                        }
                    }
                    if let Some((class, static_)) = class {
                        let symbol = self.reference_symbol_of_declaration(class)?;
                        if static_ {
                            return sequence
                                .read(
                                    self.value_reference_type_completion(
                                        symbol,
                                        &expression.location,
                                    ),
                                )
                                .map(Some);
                        }
                        let ty = sequence.read(
                            self.declared_reference_type_completion(symbol, &expression.location),
                        )?;
                        return self.model.target_this_parameter(ty);
                    }
                    Ok(None)
                }
                DottedName::Super(root) => sequence
                    .read(self.check_super_expression_completion(
                        root,
                        &super::super::location(self.inputs, root.file, root.site.span),
                    ))
                    .map(Some),
            }
        })();
        sequence.finish(result)
    }

    /// `getExplicitTypeOfSymbol`: alias resolution precedes native combined
    /// flag dispatch. A method's signature is explicit even without a written
    /// return type; an inferred ordinary property's type is not explicit.
    fn explicit_symbol_type_completion(
        &mut self,
        symbol: ReferenceSymbol,
        report_missing_annotation: bool,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(symbol) =
                sequence.read(self.resolve_reference_symbol_completion(symbol, consumer))?
            else {
                return Ok(None);
            };
            let flags = self.reference_symbol_flags(symbol, consumer)?;
            if flags.function || flags.method || flags.class || flags.value_module {
                return sequence
                    .read(self.value_reference_type_completion(symbol, consumer))
                    .map(Some);
            }
            if !flags.variable && !flags.property {
                return Ok(None);
            }
            if let Some(property) = symbol.property()
                && self.property_check_flags(property)? & super::property_symbols::checks::MAPPED
                    != 0
                && let Some(origin) = self.property_mapped_origin(property)?
                && sequence
                    .read(self.explicit_symbol_type_completion(
                        ReferenceSymbol::from_property(origin),
                        report_missing_annotation,
                        consumer,
                    ))?
                    .is_some()
            {
                return sequence
                    .read(self.value_reference_type_completion(symbol, consumer))
                    .map(Some);
            }
            let Some(declaration) = self.reference_symbol_value_declaration(symbol, consumer)?
            else {
                return Ok(None);
            };
            let explicit = match declaration {
                ReferenceDeclaration::Captured(declaration) => {
                    self.source.explicit_declaration_type(declaration)?
                }
                ReferenceDeclaration::Json { .. } => ExplicitDeclarationType::Absent,
            };
            match explicit {
                ExplicitDeclarationType::Annotated => {
                    return sequence
                        .read(self.value_reference_type_completion(symbol, consumer))
                        .map(Some);
                }
                ExplicitDeclarationType::Absent => {}
                ExplicitDeclarationType::ForOf { expression, await_ } => {
                    // Native passes nil diagnostic to the iterable lookup even
                    // when the original variable needs an annotation diagnostic.
                    if let Some(ty) =
                        sequence.read(self.explicit_dotted_type_completion(&expression, false))?
                    {
                        return sequence
                            .read(self.checked_iteration_type_completion(
                                ty,
                                if await_ {
                                    super::iteration::IterationUse::ExplicitForAwaitOf
                                } else {
                                    super::iteration::IterationUse::ExplicitForOf
                                },
                                None,
                                consumer,
                            ))
                            .map(Some);
                    }
                }
            }
            if report_missing_annotation {
                sequence.read(self.diagnostic_symbol_display_completion(symbol, consumer))?;
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `hasTypePredicateOrNeverReturnType` / `getTypePredicateOfSignature`.
    /// Predicate mapping and the raw declaration annotation are distinct
    /// native questions; an inferred or mapped return cannot replace the latter.
    fn signature_has_effect_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<bool> {
        self.type_predicate_of_signature_completion(signature, consumer)?
            .and_then(|predicate| {
                if predicate.is_some() {
                    return Ok(NativeCompletion::clean(true));
                }
                Ok(self
                    .signature_annotated_return_completion(signature)?
                    .map(|ty| ty.is_some_and(|ty| self.model.has_type_flags(ty, TypeFlags::NEVER))))
            })
    }
}
