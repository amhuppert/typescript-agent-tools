//! Declaration serialization owns the PseudoChecker attempt and fallback as
//! one operation. Syntax equivalence, output and ordinary checker rendering
//! intentionally ask different questions and retain their acquisition order.
use super::super::super::model::{NativeSequence, PropertySymbolId};
use super::super::super::source::pseudo_types::PseudoType;
use super::super::{ReferenceDeclaration, ReferenceSymbol};
use super::*;
use crate::syntax_kind::SyntaxKind as K;

impl Evaluator<'_, '_> {
    /// `serializeTypeForDeclaration(nil, propertyType, propertySymbol, true)`.
    pub(super) fn diagnostic_property_type(
        &mut self,
        symbol: PropertySymbolId,
        ty: TypeId,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        self.diagnostic_declaration_type(None, Some(ty), Some(symbol), true, context, consumer)
    }

    /// `serializeTypeForDeclaration` retains both the temporary enclosing
    /// symbol-type map and flag restoration on every completion/interruption.
    pub(super) fn diagnostic_declaration_type(
        &mut self,
        mut declaration: Option<ReferenceDeclaration>,
        supplied: Option<TypeId>,
        mut symbol: Option<PropertySymbolId>,
        try_reuse: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        let mut sequence = NativeSequence::new();
        let saved_flags = context.flags;
        let result = (|| {
            if declaration.is_none()
                && let Some(symbol) = symbol
            {
                declaration = self.property_reference_value_declaration(symbol, consumer)?;
                if declaration.is_none() {
                    declaration = self
                        .property_reference_declarations(symbol, consumer)?
                        .first()
                        .copied();
                }
            }
            if symbol.is_none()
                && let Some(ReferenceDeclaration::Captured(declaration)) = declaration
            {
                symbol = sequence
                    .read(self.reference_node_declaration_symbol_completion(declaration))?
                    .map(|symbol| self.register_reference_symbol(symbol, consumer))
                    .transpose()?;
            }
            let kind = declaration
                .map(|declaration| self.reference_declaration_kind(declaration))
                .transpose()?;
            let identity = symbol
                .map(|symbol| {
                    self.property_native_symbol(symbol, consumer)
                        .map(|symbol| symbol.identity())
                })
                .transpose()?;
            let mut ty = if let Some(ty) = supplied {
                ty
            } else if let Some(ty) =
                identity.and_then(|symbol| context.enclosing_symbol_types.get(&symbol).copied())
            {
                ty
            } else {
                let raw = if let Some(symbol) = symbol {
                    let flags = self.property_symbol_flags(symbol)?;
                    if (flags.get_accessor || flags.set_accessor) && kind == Some(K::SetAccessor) {
                        sequence.read(self.property_write_type_completion(symbol, consumer))?
                    } else {
                        let reference = ReferenceSymbol::from_property(symbol);
                        let flags = self.reference_symbol_flags(reference, consumer)?;
                        if flags.type_literal || flags.signature {
                            self.model.native_error_type()
                        } else {
                            let ty = sequence
                                .read(self.property_raw_type_completion(symbol, consumer))?;
                            sequence.read(self.widened_literal_type_completion(ty, consumer))?
                        }
                    }
                } else {
                    self.model.native_error_type()
                };
                if let Some(mapper) = context.mapper {
                    sequence.read(self.instantiate_type_completion(raw, mapper, consumer))?
                } else {
                    raw
                }
            };
            let requires_undefined = match declaration {
                Some(ReferenceDeclaration::Captured(declaration)) if kind == Some(K::Parameter) => {
                    sequence.read(self.diagnostic_parameter_requires_undefined(
                        declaration,
                        context,
                        consumer,
                    ))?
                }
                Some(declaration)
                    if matches!(kind, Some(K::PropertyDeclaration | K::PropertySignature)) =>
                {
                    let symbol = symbol.ok_or_else(|| {
                        super::super::super::internal("property serialization has no actual symbol")
                    })?;
                    sequence.read(self.diagnostic_property_requires_undefined(
                        symbol,
                        declaration,
                        consumer,
                    ))?
                }
                _ => false,
            };
            if requires_undefined && kind == Some(K::Parameter) {
                ty = sequence.read(self.optional_type_completion(ty, false, consumer))?;
            }
            if self.model.has_type_flags(ty, TypeFlags::UNIQUE_SYMBOL)
                && self.type_symbol_identity(ty)? == identity
            {
                let mut same_file = context.enclosing.is_none();
                if !same_file && let Some(symbol) = symbol {
                    for declaration in self.property_reference_declarations(symbol, consumer)? {
                        let file = match declaration {
                            ReferenceDeclaration::Captured(declaration) => declaration.file,
                            ReferenceDeclaration::Json { file, .. } => file,
                        };
                        if Some(file) == context.enclosing_file {
                            same_file = true;
                            break;
                        }
                    }
                }
                if same_file {
                    context.flags.0 |= 1 << 20;
                }
            }
            let mut output = None;
            let mut reported_fallback = false;
            if try_reuse
                && context.enclosing.is_some()
                && let Some(ReferenceDeclaration::Captured(declaration)) = declaration
                && (matches!(kind, Some(K::GetAccessor | K::SetAccessor))
                    || kind.is_some_and(crate::syntax::has_inferred_type)
                        && !self
                            .model
                            .object_flags(ty)?
                            .intersects(ObjectFlags::REQUIRES_WIDENING))
            {
                let old =
                    identity.and_then(|symbol| context.enclosing_symbol_types.insert(symbol, ty));
                let attempted = (|| {
                    let mut pseudo = if matches!(kind, Some(K::GetAccessor | K::SetAccessor)) {
                        self.source.pseudo_accessor_type(declaration)?
                    } else {
                        self.source.pseudo_declaration_type(declaration)?
                    };
                    if matches!(pseudo, PseudoType::NoResult(_))
                        && kind == Some(K::BinaryExpression)
                        && let Some(symbol) = symbol
                    {
                        for declaration in self.property_reference_declarations(symbol, consumer)? {
                            if let ReferenceDeclaration::Captured(declaration) = declaration
                                && self.source.pseudo_child(declaration, "type")?.is_some()
                            {
                                pseudo = self.source.pseudo_declaration_type(declaration)?;
                                break;
                            }
                        }
                    }
                    let report = !context.suppress_report_inference_fallback;
                    let optional = !requires_undefined
                        && matches!(
                            kind,
                            Some(K::Parameter | K::PropertySignature | K::PropertyDeclaration)
                        )
                        && self.source.diagnostic_optional_declaration(declaration)?;
                    if sequence.read(self.pseudo_type_equivalent(
                        &pseudo,
                        Some(ty),
                        optional,
                        report,
                        context,
                        consumer,
                    ))? {
                        let realized =
                            sequence.read(self.pseudo_type_to_type(&pseudo, consumer))?;
                        if requires_undefined
                            && self.model.contains_non_missing_undefined(ty)
                            && realized
                                .is_some_and(|ty| !self.model.contains_non_missing_undefined(ty))
                        {
                            pseudo = PseudoType::Union(vec![
                                pseudo,
                                PseudoType::Atom(K::UndefinedKeyword),
                            ]);
                        }
                        output =
                            Some(sequence.read(self.pseudo_type_node_with_checker_fallback(
                                &pseudo, ty, context, consumer,
                            ))?);
                    } else {
                        reported_fallback = report
                            && matches!(&pseudo, PseudoType::Inferred { errors, .. } if !errors.is_empty());
                        let add = if requires_undefined {
                            match sequence.read(self.pseudo_type_to_type(&pseudo, consumer))? {
                                Some(ty) => !self.model.contains_non_missing_undefined(ty),
                                None => !self.source.pseudo_could_refer_to_undefined(&pseudo)?,
                            }
                        } else {
                            false
                        };
                        if add {
                            pseudo = PseudoType::Union(vec![
                                pseudo,
                                PseudoType::Atom(K::UndefinedKeyword),
                            ]);
                            if sequence.read(self.pseudo_type_equivalent(
                                &pseudo,
                                Some(ty),
                                false,
                                report,
                                context,
                                consumer,
                            ))? {
                                output = Some(sequence.read(
                                    self.pseudo_type_node_with_checker_fallback(
                                        &pseudo, ty, context, consumer,
                                    ),
                                )?);
                                reported_fallback = false;
                            }
                        }
                    }
                    Ok::<_, DemandError>(())
                })();
                if let Some(symbol) = identity {
                    match old {
                        Some(ty) => {
                            context.enclosing_symbol_types.insert(symbol, ty);
                        }
                        None => {
                            context.enclosing_symbol_types.remove(&symbol);
                        }
                    }
                }
                attempted?;
            }
            if let Some(output) = output {
                return Ok(output);
            }
            let saved = context.suppress_report_inference_fallback;
            if reported_fallback {
                context.suppress_report_inference_fallback = true;
            }
            let output = sequence.read(self.diagnostic_type_node(ty, context, consumer));
            context.suppress_report_inference_fallback = saved;
            output
        })();
        context.flags = saved_flags;
        sequence.finish(result)
    }

    /// Property arm of EmitResolver `requiresAddingImplicitUndefined`: the raw
    /// read precedes flags, token, reverse links and the undefined predicate.
    fn diagnostic_property_requires_undefined(
        &mut self,
        symbol: PropertySymbolId,
        declaration: ReferenceDeclaration,
        consumer: &Location,
    ) -> NativeResult<bool> {
        self.property_raw_type_completion(symbol, consumer)?
            .and_then_value(|ty| {
                let flags = self.property_symbol_flags(symbol)?;
                if !flags.property || !flags.optional {
                    return Ok(false);
                }
                let optional = match declaration {
                    ReferenceDeclaration::Captured(declaration) => {
                        self.source.diagnostic_optional_declaration(declaration)?
                    }
                    ReferenceDeclaration::Json { .. } => false,
                };
                if !optional || self.reverse_property_links(symbol)?.is_none() {
                    return Ok(false);
                }
                Ok(self.model.contains_non_missing_undefined(ty))
            })
    }

    /// Checker `isOptionalParameter`, independently from Symbol.Optional and
    /// call-site getTypeOfParameter optionality.
    pub(super) fn diagnostic_optional_parameter(
        &mut self,
        declaration: Option<DeclarationIdentity>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(declaration) = declaration else {
                return Ok(false);
            };
            if self.source.declaration_kind(declaration)? != K::Parameter {
                return Ok(false);
            }
            let parameter = self.source.parameter_recipe(declaration)?;
            if parameter.question_mark {
                return Ok(true);
            }
            let context = self.source.parameter_context(declaration)?;
            if parameter.initializer.is_some() {
                let parent = self
                    .source
                    .declaration_from_expression_root(context.function)?;
                let recipe = self.source.signature_from_declaration(parent)?;
                let signature = sequence.read(self.signature_completion(&recipe))?;
                return Ok(context.index
                    >= sequence.read(
                        self.minimum_argument_count_ex_completion(&signature, true, consumer),
                    )?);
            }
            if let Some(iife) = context.iife {
                return Ok(parameter.annotation.is_none()
                    && !parameter.rest
                    && context.index
                        >= sequence.read(self.effective_call_argument_count_completion(iife))?);
            }
            Ok(false)
        })();
        sequence.finish(result)
    }
    fn diagnostic_parameter_requires_undefined(
        &mut self,
        declaration: DeclarationIdentity,
        context: &Context,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !self.inputs.semantic_options().strict_null_checks {
                return Ok(false);
            }
            let parameter = self.source.parameter_recipe(declaration)?;
            let optional =
                sequence.read(self.diagnostic_optional_parameter(Some(declaration), consumer))?;
            let property = self.source.reference_parameter_property(declaration)?;
            let required_initialized = !optional
                && parameter.initializer.is_some()
                && (!property
                    || match context.enclosing {
                        Some(super::super::symbol_accessibility::ScopeLocation::Captured(
                            declaration,
                        )) => crate::syntax::is_function_like_declaration_kind(
                            self.source.declaration_kind(declaration)?,
                        ),
                        _ => false,
                    });
            // Native OR short-circuits the second isOptionalParameter query.
            let optional_property = if required_initialized {
                false
            } else {
                sequence.read(self.diagnostic_optional_parameter(Some(declaration), consumer))?
                    && parameter.initializer.is_none()
                    && property
            };
            if !required_initialized && !optional_property {
                return Ok(false);
            }
            let Some(annotation) = parameter.annotation else {
                return Ok(true);
            };
            let ty = sequence.read(self.check_type_completion(&annotation))?;
            Ok(!self.model.is_error_type(ty) && !self.model.contains_undefined(ty))
        })();
        sequence.finish(result)
    }
}

impl Evaluator<'_, '_> {
    /// `serializeReturnTypeForSignature` publishes no semantic slot. Temporary
    /// symbol substitutions only prevent re-reading the enclosing declaration.
    pub(super) fn diagnostic_signature_return(
        &mut self,
        signature: &Signature,
        try_reuse: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        let mut sequence = NativeSequence::new();
        let saved_flags = context.flags;
        let result = (|| {
            let suppress_any = context.flags.has(1 << 8);
            context.flags.0 &= !(1 << 8);
            let declaration = self
                .signature_declaration_root(signature.id)?
                .map(|root| self.source.diagnostic_signature_declaration(root))
                .transpose()?;
            let symbol = declaration
                .map(|node| sequence.read(self.reference_node_declaration_symbol_completion(node)))
                .transpose()?
                .flatten();
            let identity = symbol
                .map(|symbol| self.reference_symbol_identity(symbol, consumer))
                .transpose()?;
            let returns = if let Some(ty) =
                identity.and_then(|identity| context.enclosing_symbol_types.get(&identity).copied())
            {
                ty
            } else {
                let ty = sequence.read(self.raw_return_type_of_signature_completion(signature))?;
                if declaration.is_some()
                    && let Some(mapper) = context.mapper
                {
                    sequence.read(self.instantiate_type_completion(ty, mapper, consumer))?
                } else {
                    ty
                }
            };
            if suppress_any && self.model.has_type_flags(returns, TypeFlags::ANY) {
                return Err(self.diagnostic_dependency(
                    consumer,
                    "native omitted any return requires an optional output signature return node",
                ));
            }
            let mut output = None;
            if try_reuse
                && context.enclosing.is_some()
                && let Some(declaration) = declaration
            {
                let old = identity
                    .and_then(|identity| context.enclosing_symbol_types.insert(identity, returns));
                let attempted = (|| {
                    let pseudo = self.source.pseudo_return_type(declaration)?;
                    if sequence.read(self.pseudo_type_equivalent(
                        &pseudo,
                        Some(returns),
                        false,
                        !context.suppress_report_inference_fallback,
                        context,
                        consumer,
                    ))? {
                        let matches = match self.type_predicate_of_signature(signature, consumer)? {
                            Some(predicate) => sequence.read(
                                self.pseudo_predicate_equivalent(&pseudo, &predicate, consumer),
                            )?,
                            None => true,
                        };
                        if matches {
                            output = Some(sequence.read(
                                self.pseudo_type_node_with_checker_fallback(
                                    &pseudo, returns, context, consumer,
                                ),
                            )?);
                        } else if !context.suppress_report_inference_fallback {
                            self.diagnostic_report_inference_fallback(declaration, context);
                        }
                    }
                    Ok::<_, DemandError>(())
                })();
                if let Some(identity) = identity {
                    match old {
                        Some(ty) => {
                            context.enclosing_symbol_types.insert(identity, ty);
                        }
                        None => {
                            context.enclosing_symbol_types.remove(&identity);
                        }
                    }
                }
                attempted?;
            }
            if let Some(output) = output {
                return Ok(output);
            }
            if let Some(predicate) = self.type_predicate_of_signature(signature, consumer)? {
                sequence.read(self.diagnostic_predicate_node(&predicate, context, consumer))
            } else {
                sequence.read(self.diagnostic_type_node(returns, context, consumer))
            }
        })();
        context.flags = saved_flags;
        sequence.finish(result)
    }
}
