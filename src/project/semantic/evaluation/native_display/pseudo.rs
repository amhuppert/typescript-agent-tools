//! Semantic realization, equivalence and output of native PseudoChecker
//! recipes remain distinct operations with their own acquisition schedules.
use super::super::super::model::{NativeSequence, Predicate, TupleElementFlags};
use super::super::super::source::pseudo_types::{PseudoMemberKind, PseudoParameter, PseudoType};
use super::super::ReferenceDeclaration;
use super::super::facts::TypeFacts;
use super::super::relations::RelationKind;
use super::*;
use crate::syntax_kind::SyntaxKind as K;

impl Evaluator<'_, '_> {
    /// `pseudoTypeToType`: structural recipes have actual nil; an acquired
    /// recovery is never substituted for that native absence.
    pub(super) fn pseudo_type_to_type(
        &mut self,
        pseudo: &PseudoType,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            Ok(Some(match pseudo {
                PseudoType::Direct(node) => {
                    let annotation = self.source.pseudo_annotation(*node)?;
                    sequence.read(self.check_type_completion(&annotation))?
                }
                PseudoType::Inferred {
                    expression,
                    signature_return,
                    ..
                } => {
                    if *signature_return {
                        let recipe = self.source.signature_from_declaration(*expression)?;
                        let signature = sequence.read(self.signature_completion(&recipe))?;
                        sequence.read(self.raw_return_type_of_signature_completion(&signature))?
                    } else {
                        let root = self.source.pseudo_node_expression(*expression)?;
                        let expression = self.source.expression_at(root.file, root.site)?;
                        let regular = sequence
                            .read(self.regular_type_of_expression_completion(&expression))?;
                        sequence.read(self.widened_type_completion(regular, consumer))?
                    }
                }
                PseudoType::NoResult(_)
                | PseudoType::Object(_)
                | PseudoType::Tuple(_)
                | PseudoType::Function(_) => return Ok(None),
                PseudoType::MaybeConst {
                    node,
                    constant,
                    regular,
                } => {
                    let root = self.source.pseudo_node_expression(*node)?;
                    let constant_context =
                        sequence.read(self.is_const_context_completion(root, consumer))?;
                    return sequence.read(self.pseudo_type_to_type(
                        if constant_context { constant } else { regular },
                        consumer,
                    ));
                }
                PseudoType::Union(types) => {
                    let mut retained = Vec::new();
                    let mut elided = false;
                    for ty in types {
                        if !self.inputs.semantic_options().strict_null_checks
                            && matches!(ty, PseudoType::Atom(K::UndefinedKeyword | K::NullKeyword))
                        {
                            elided = true;
                            continue;
                        }
                        let Some(ty) = sequence.read(self.pseudo_type_to_type(ty, consumer))?
                        else {
                            return Ok(None);
                        };
                        retained.push(ty);
                    }
                    match retained.len() {
                        0 => self.model.special(if elided {
                            SpecialName::Any
                        } else {
                            SpecialName::Never
                        }),
                        1 => retained[0],
                        _ => sequence
                            .read(self.union_type_ex_completion(retained, None, None, consumer))?,
                    }
                }
                PseudoType::Atom(kind) => match kind {
                    K::UndefinedKeyword => self.model.undefined_initializer(),
                    K::NullKeyword => self.model.null_initializer(),
                    K::AnyKeyword => self.model.special(SpecialName::Any),
                    K::StringKeyword => self.model.primitive(PrimitiveName::String),
                    K::NumberKeyword => self.model.primitive(PrimitiveName::Number),
                    K::BigIntKeyword => self.model.primitive(PrimitiveName::Bigint),
                    K::BooleanKeyword => self.model.primitive(PrimitiveName::Boolean),
                    K::TrueKeyword | K::FalseKeyword => self.model.literal(
                        PrimitiveName::Boolean,
                        if *kind == K::TrueKeyword {
                            "true"
                        } else {
                            "false"
                        }
                        .to_owned(),
                    ),
                    _ => {
                        return Err(super::super::super::internal(
                            "pseudo atom is not a native pseudo primitive",
                        )
                        .into());
                    }
                },
                PseudoType::Literal { node, .. } => {
                    let root = self.source.pseudo_node_expression(*node)?;
                    let expression = self.source.expression_at(root.file, root.site)?;
                    sequence.read(self.regular_type_of_expression_completion(&expression))?
                }
            }))
        })();
        sequence.finish(result)
    }

    /// `pseudoTypeEquivalentToType` first uses actual types, then compares
    /// structural recipes in declaration order. Reports are renderer metadata;
    /// they cannot erase already consumed checker evidence.
    pub(super) fn pseudo_type_equivalent(
        &mut self,
        pseudo: &PseudoType,
        target: Option<TypeId>,
        optional: bool,
        report_errors: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if target.is_some_and(|ty| self.model.is_error_type(ty)) {
                return Ok(true);
            }
            let realized = sequence.read(self.pseudo_type_to_type(pseudo, consumer))?;
            if realized == target {
                return Ok(true);
            }
            let stripped = if optional {
                match target {
                    Some(ty) => Some(sequence.read(self.type_with_facts_completion(
                        ty,
                        TypeFacts::NOT_UNDEFINED,
                        consumer,
                    ))?),
                    None => None,
                }
            } else {
                target
            };
            if let (Some(realized), Some(target)) = (realized, target) {
                if optional && let Some(stripped) = stripped {
                    if realized == stripped {
                        return Ok(true);
                    }
                    if self.model.has_type_flags(realized, TypeFlags::UNION)
                        && self.model.has_type_flags(stripped, TypeFlags::UNION)
                        && sequence.read(self.raw_type_related_completion(
                            realized,
                            stripped,
                            RelationKind::Identity,
                            consumer,
                        ))?
                    {
                        return Ok(true);
                    }
                }
                let left = self.model.regular_literal(realized);
                let right = self.model.regular_literal(target);
                if left == right {
                    return Ok(true);
                }
                if self.model.has_type_flags(realized, TypeFlags::UNION)
                    && self.model.has_type_flags(target, TypeFlags::UNION)
                    && sequence.read(self.raw_type_related_completion(
                        realized,
                        target,
                        RelationKind::Identity,
                        consumer,
                    ))?
                {
                    return Ok(true);
                }
            }
            match pseudo {
                PseudoType::Object(elements) => {
                    let Some(target) = stripped else {
                        return Ok(false);
                    };
                    let properties =
                        sequence.read(self.properties_of_type_completion(target, consumer))?;
                    let mut declaration_count = 0;
                    for property in &properties {
                        let symbol = self.property_symbol(property, consumer)?;
                        declaration_count += self
                            .property_reference_declarations(symbol, consumer)?
                            .len();
                    }
                    if elements.len() != declaration_count {
                        return Ok(false);
                    }
                    for element in elements {
                        let declaration = self
                            .source
                            .declaration_parent(element.name)?
                            .ok_or_else(|| {
                                super::super::super::internal("pseudo member name has no parent")
                            })?;
                        let raw = self.reference_raw_symbol_of_declaration(declaration)?;
                        let mut property = match raw {
                            Some(symbol) => {
                                let key = self.reference_symbol_key(symbol, consumer)?;
                                sequence.read(
                                    self.property_of_type_by_key_completion(target, &key, consumer),
                                )?
                            }
                            None => None,
                        };
                        if property.is_none() {
                            for candidate in &properties {
                                let symbol = self.property_symbol(candidate, consumer)?;
                                if let Some(ReferenceDeclaration::Captured(declaration)) =
                                    self.property_reference_value_declaration(symbol, consumer)?
                                    && self.source.pseudo_child(declaration, "name")?
                                        == Some(element.name)
                                {
                                    property = Some(candidate.clone());
                                    break;
                                }
                            }
                        }
                        let Some(property) = property else {
                            self.pseudo_report_if(report_errors, declaration, context);
                            return Ok(false);
                        };
                        let symbol = self.property_symbol(&property, consumer)?;
                        let target_optional = self.property_symbol_flags(symbol)?.optional;
                        if element.optional != target_optional {
                            self.pseudo_report_if(report_errors, declaration, context);
                            return Ok(false);
                        }
                        let raw =
                            sequence.read(self.property_raw_type_completion(symbol, consumer))?;
                        let ty = self
                            .model
                            .remove_missing_type(raw, target_optional)
                            .map_err(|failure| failure.at(self.inputs, consumer))?;
                        match &element.kind {
                            PseudoMemberKind::Property { ty: pseudo, .. } => {
                                if !sequence.read(self.pseudo_type_equivalent(
                                    pseudo,
                                    Some(ty),
                                    element.optional,
                                    false,
                                    context,
                                    consumer,
                                ))? {
                                    if let PseudoType::Inferred { errors, .. } = pseudo
                                        && !errors.is_empty()
                                    {
                                        for &node in errors {
                                            self.pseudo_report_if(report_errors, node, context);
                                        }
                                    } else if !pseudo_structural(pseudo) {
                                        self.pseudo_report_if(report_errors, declaration, context);
                                    }
                                    return Ok(false);
                                }
                            }
                            PseudoMemberKind::Method(signature) => {
                                let Some(target_signature) = sequence.read(
                                    self.single_signature_completion(ty, false, false, consumer),
                                )?
                                else {
                                    continue;
                                };
                                if !sequence.read(self.pseudo_parameters_equivalent(
                                    &signature.parameters,
                                    &target_signature,
                                    report_errors,
                                    declaration,
                                    context,
                                    consumer,
                                ))? {
                                    return Ok(false);
                                }
                                if !sequence.read(self.pseudo_return_equivalent(
                                    &signature.returns,
                                    &target_signature,
                                    false,
                                    context,
                                    consumer,
                                ))? {
                                    self.pseudo_report_if(report_errors, declaration, context);
                                    return Ok(false);
                                }
                            }
                            PseudoMemberKind::Get { ty: pseudo, .. } => {
                                if !sequence.read(self.pseudo_type_equivalent(
                                    pseudo,
                                    Some(ty),
                                    false,
                                    false,
                                    context,
                                    consumer,
                                ))? {
                                    self.pseudo_report_if(report_errors, declaration, context);
                                    return Ok(false);
                                }
                            }
                            PseudoMemberKind::Set { parameter, .. } => {
                                let write = sequence
                                    .read(self.property_write_type_completion(symbol, consumer))?;
                                if !sequence.read(self.pseudo_type_equivalent(
                                    &parameter.ty,
                                    Some(write),
                                    false,
                                    false,
                                    context,
                                    consumer,
                                ))? {
                                    self.pseudo_report_if(report_errors, declaration, context);
                                    return Ok(false);
                                }
                            }
                        }
                    }
                    Ok(true)
                }
                PseudoType::Tuple(elements) => {
                    let Some(target) = stripped else {
                        return Ok(false);
                    };
                    let Some(tuple) = self.model.tuple_target(target)? else {
                        return Ok(false);
                    };
                    if tuple
                        .combined_flags
                        .intersects(TupleElementFlags::NON_REQUIRED)
                    {
                        return Ok(false);
                    }
                    let arguments = sequence.read(self.get_type_arguments_completion(target))?;
                    if arguments.len() != elements.len() {
                        return Ok(false);
                    }
                    for (element, &argument) in elements.iter().zip(arguments.iter()) {
                        if !sequence.read(self.pseudo_type_equivalent(
                            element,
                            Some(argument),
                            false,
                            report_errors,
                            context,
                            consumer,
                        ))? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                PseudoType::Function(signature) => {
                    let Some(target) = stripped else {
                        return Ok(false);
                    };
                    let Some(target) = sequence
                        .read(self.single_signature_completion(target, false, false, consumer))?
                    else {
                        return Ok(false);
                    };
                    if target.type_parameters().len() != signature.type_parameters.len() {
                        self.pseudo_report_if(report_errors, signature.declaration, context);
                        return Ok(false);
                    }
                    if !sequence.read(self.pseudo_parameters_equivalent(
                        &signature.parameters,
                        &target,
                        report_errors,
                        signature.declaration,
                        context,
                        consumer,
                    ))? {
                        return Ok(false);
                    }
                    sequence.read(self.pseudo_return_equivalent(
                        &signature.returns,
                        &target,
                        report_errors,
                        context,
                        consumer,
                    ))
                }
                PseudoType::Inferred {
                    expression, errors, ..
                } => {
                    if errors.is_empty() {
                        self.pseudo_report_if(report_errors, *expression, context);
                    } else {
                        for &node in errors {
                            self.pseudo_report_if(report_errors, node, context);
                        }
                    }
                    Ok(false)
                }
                PseudoType::NoResult(node) => {
                    self.pseudo_report_if(report_errors, *node, context);
                    Ok(false)
                }
                _ => Ok(false),
            }
        })();
        sequence.finish(result)
    }

    fn pseudo_parameters_equivalent(
        &mut self,
        mut parameters: &[PseudoParameter],
        target: &Signature,
        report_errors: bool,
        location: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(this) = target.this_parameter() {
                let Some(parameter) = parameters.first() else {
                    self.pseudo_report_if(report_errors, location, context);
                    return Ok(false);
                };
                if !self.source.pseudo_is_this_name(parameter.name)? {
                    self.pseudo_report_if(report_errors, location, context);
                    return Ok(false);
                }
                let ty = sequence.read(self.demand_completion(this.call_type))?;
                if !sequence.read(self.pseudo_type_equivalent(
                    &parameter.ty,
                    Some(ty),
                    parameter.optional,
                    false,
                    context,
                    consumer,
                ))? {
                    let declaration = self
                        .source
                        .declaration_parent(parameter.name)?
                        .unwrap_or(location);
                    self.pseudo_report_if(report_errors, declaration, context);
                    return Ok(false);
                }
                parameters = &parameters[1..];
            }
            if target.parameters.len() != parameters.len() {
                self.pseudo_report_if(report_errors, location, context);
                return Ok(false);
            }
            for (parameter, target) in parameters.iter().zip(target.parameters.iter()) {
                let optional = sequence
                    .read(self.diagnostic_optional_parameter(target.declaration, consumer))?;
                if optional != parameter.optional {
                    let node = self
                        .source
                        .declaration_parent(parameter.name)?
                        .unwrap_or(location);
                    self.pseudo_report_if(report_errors, node, context);
                    return Ok(false);
                }
                let ty = sequence.read(self.demand_completion(target.call_type))?;
                if !sequence.read(self.pseudo_type_equivalent(
                    &parameter.ty,
                    Some(ty),
                    parameter.optional,
                    false,
                    context,
                    consumer,
                ))? {
                    let node = self
                        .source
                        .declaration_parent(parameter.name)?
                        .unwrap_or(location);
                    self.pseudo_report_if(report_errors, node, context);
                    return Ok(false);
                }
            }
            Ok(true)
        })();
        sequence.finish(result)
    }
    fn pseudo_return_equivalent(
        &mut self,
        pseudo: &PseudoType,
        signature: &Signature,
        report_errors: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if let Some(predicate) = self.type_predicate_of_signature(signature, consumer)? {
            return self.pseudo_predicate_equivalent(pseudo, &predicate, consumer);
        }
        self.raw_return_type_of_signature_completion(signature)?
            .and_then(|ty| {
                self.pseudo_type_equivalent(
                    pseudo,
                    Some(ty),
                    false,
                    report_errors,
                    context,
                    consumer,
                )
            })
    }
    pub(super) fn pseudo_predicate_equivalent(
        &mut self,
        pseudo: &PseudoType,
        predicate: &Predicate,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let PseudoType::Direct(node) = pseudo else {
                return Ok(false);
            };
            let Some(facts) = self.source.pseudo_predicate(*node)? else {
                return Ok(false);
            };
            use crate::types::TypePredicateKind as PK;
            if facts.asserts == matches!(predicate.kind, PK::Is) {
                return Ok(false);
            }
            let is_this = matches!(
                predicate.subject,
                crate::types::TypePredicateSubject::This(_)
            );
            if facts.this_ != is_this
                || !is_this && facts.name.as_deref() != predicate.parameter_name.as_deref()
            {
                return Ok(false);
            }
            match (predicate.value_type, facts.ty) {
                (Some(target), Some(annotation)) => {
                    // Predicate factory owns its type; annotation comparison
                    // then asks the direct type node independently.
                    let target = sequence.read(self.demand_completion(target))?;
                    let annotation = self.source.pseudo_annotation(annotation)?;
                    let ty = sequence.read(self.check_type_completion(&annotation))?;
                    if ty == target {
                        return Ok(true);
                    }
                    sequence.read(self.raw_type_related_completion(
                        ty,
                        target,
                        RelationKind::Identity,
                        consumer,
                    ))
                }
                (None, None) => Ok(true),
                _ => Ok(false),
            }
        })();
        sequence.finish(result)
    }

    /// Current diagnostic entries have a nil outer tracker. Native
    /// ReportInferenceFallback neither changes reportedDiagnostic nor performs
    /// checker work in that context, including inside wrapping reuse trackers.
    fn pseudo_report_if(&mut self, report: bool, node: DeclarationIdentity, context: &mut Context) {
        if report {
            self.diagnostic_report_inference_fallback(node, context);
        }
    }
    pub(super) fn diagnostic_report_inference_fallback(
        &mut self,
        _node: DeclarationIdentity,
        _context: &mut Context,
    ) {
    }
}

fn pseudo_structural(ty: &PseudoType) -> bool {
    match ty {
        PseudoType::Object(_) | PseudoType::Tuple(_) | PseudoType::Function(_) => true,
        PseudoType::MaybeConst {
            constant, regular, ..
        } => pseudo_structural(constant) || pseudo_structural(regular),
        _ => false,
    }
}
