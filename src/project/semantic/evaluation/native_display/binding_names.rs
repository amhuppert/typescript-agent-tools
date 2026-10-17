//! Parameter declaration names retain syntax independently from Symbol.Name.
//! Q4 previews use the same clone with semantic computed-name work deferred.
use super::super::super::model::NativeSequence;
use super::*;
use crate::syntax_kind::SyntaxKind as K;

impl Evaluator<'_, '_> {
    /// `getEffectiveParameterDeclaration`: declarations, not generated names
    /// or the parameter's value recipe, choose the native declaration.
    pub(super) fn diagnostic_effective_parameter_declaration(
        &mut self,
        parameter: &ParameterValue,
        consumer: &Location,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        let declarations = self.property_reference_declarations(parameter.symbol, consumer)?;
        for declaration in &declarations {
            if let super::super::ReferenceDeclaration::Captured(declaration) = declaration
                && self.source.declaration_kind(*declaration)? == K::Parameter
            {
                return Ok(Some(*declaration));
            }
        }
        if !self.property_is_transient(parameter.symbol)? {
            for declaration in declarations {
                if let super::super::ReferenceDeclaration::Captured(declaration) = declaration
                    && self.source.declaration_kind(declaration)? == K::JSDocParameterTag
                {
                    return Ok(Some(declaration));
                }
            }
        }
        Ok(None)
    }

    /// `parameterToParameterDeclarationName` shared with Q4. Preview returns
    /// None at semantic name work, a lower bound rather than a fabricated name.
    pub(in crate::project::semantic) fn summary_parameter_name(
        &mut self,
        parameter: &ParameterValue,
        preview: bool,
        consumer: &Location,
    ) -> Result<Option<String>, DemandError> {
        let declaration = self.diagnostic_effective_parameter_declaration(parameter, consumer)?;
        let name = declaration
            .map(|declaration| self.source.pseudo_child(declaration, "name"))
            .transpose()?
            .flatten();
        let Some(name) = name else {
            return Ok(Some(parameter.name.clone()));
        };
        let mut context = Context::new(DiagnosticTypeFormat(0), None, false);
        self.diagnostic_parameter_name_worker(name, preview, &mut context, consumer)?
            .admit()
    }

    /// `parameterToParameterDeclarationName` clones an identifier or the
    /// right identifier of a JSDoc qualified name; all patterns use one clone.
    pub(super) fn diagnostic_parameter_name(
        &mut self,
        name: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<String> {
        self.diagnostic_parameter_name_worker(name, false, context, consumer)?
            .and_then_value(|name| {
                name.ok_or_else(|| {
                    super::super::super::internal("admitted parameter name is absent").into()
                })
            })
    }

    fn diagnostic_parameter_name_worker(
        &mut self,
        name: DeclarationIdentity,
        preview: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Option<String>> {
        let kind = self.source.declaration_kind(name)?;
        if kind == K::Identifier {
            return Ok(NativeCompletion::clean(Some(
                self.source.pseudo_exact_name(name)?,
            )));
        }
        if kind == K::QualifiedName {
            let right = self.source.pseudo_child(name, "right")?.ok_or_else(|| {
                super::super::super::internal("qualified parameter name lacks right")
            })?;
            return Ok(NativeCompletion::clean(Some(
                self.source.pseudo_exact_name(right)?,
            )));
        }
        self.diagnostic_clone_binding_name_worker(name, preview, context, consumer)
    }

    pub(super) fn diagnostic_clone_binding_name(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<String> {
        self.diagnostic_clone_binding_name_worker(node, false, context, consumer)?
            .and_then_value(|name| {
                name.ok_or_else(|| {
                    super::super::super::internal("admitted binding name is absent").into()
                })
            })
    }

    /// `cloneBindingName` visits children before removing a BindingElement's
    /// initializer. The single-line printer preserves binding-list commas,
    /// exact identifier text and the literal node's quote kind.
    fn diagnostic_clone_binding_name_worker(
        &mut self,
        node: DeclarationIdentity,
        preview: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Option<String>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let kind = self.source.declaration_kind(node)?;
            Ok(Some(match kind {
                K::Identifier | K::PrivateIdentifier | K::NumericLiteral | K::BigIntLiteral => {
                    self.source.pseudo_exact_name(node)?
                }
                K::StringLiteral => crate::printer::quoted_text(
                    &self.source.pseudo_exact_name(node)?,
                    if self.source.pseudo_single_quote(node)? {
                        '\''
                    } else {
                        '"'
                    },
                ),
                K::ObjectBindingPattern | K::ArrayBindingPattern => {
                    let mut elements = Vec::new();
                    for element in self.source.pseudo_children(node, "elements")? {
                        let Some(element) =
                            sequence.read(self.diagnostic_clone_binding_name_worker(
                                element, preview, context, consumer,
                            ))?
                        else {
                            return Ok(None);
                        };
                        elements.push(element);
                    }
                    let mut text = elements.join(", ");
                    if self.source.diagnostic_binding_trailing_comma(node)? {
                        text.push(',');
                    }
                    if kind == K::ObjectBindingPattern {
                        if elements.is_empty() {
                            "{}".into()
                        } else {
                            format!("{{ {text} }}")
                        }
                    } else {
                        format!("[{text}]")
                    }
                }
                K::OmittedExpression => String::new(),
                K::BindingElement => {
                    let mut text = if self.source.pseudo_child(node, "dotDotDotToken")?.is_some() {
                        "...".to_owned()
                    } else {
                        String::new()
                    };
                    if let Some(property) = self.source.pseudo_child(node, "propertyName")? {
                        let Some(property) =
                            sequence.read(self.diagnostic_clone_binding_name_worker(
                                property, preview, context, consumer,
                            ))?
                        else {
                            return Ok(None);
                        };
                        text.push_str(&property);
                        text.push_str(": ");
                    }
                    if let Some(name) = self.source.pseudo_child(node, "name")? {
                        let Some(name) =
                            sequence.read(self.diagnostic_clone_binding_name_worker(
                                name, preview, context, consumer,
                            ))?
                        else {
                            return Ok(None);
                        };
                        text.push_str(&name);
                    }
                    if !preview
                        && let Some(initializer) = self.source.pseudo_child(node, "initializer")?
                    {
                        sequence.read(self.diagnostic_binding_clone_effects(
                            initializer,
                            context,
                            consumer,
                        ))?;
                    }
                    text
                }
                K::ComputedPropertyName => {
                    if preview {
                        return Ok(None);
                    }
                    sequence.read(
                        self.diagnostic_track_binding_computed_name(node, context, consumer),
                    )?;
                    let expression =
                        self.source
                            .pseudo_child(node, "expression")?
                            .ok_or_else(|| {
                                super::super::super::internal(
                                    "computed binding name has no expression",
                                )
                            })?;
                    let Some(expression) =
                        sequence.read(self.diagnostic_clone_binding_name_worker(
                            expression, false, context, consumer,
                        ))?
                    else {
                        return Ok(None);
                    };
                    format!("[{expression}]")
                }
                K::PropertyAccessExpression | K::ElementAccessExpression => {
                    let expression =
                        self.source
                            .pseudo_child(node, "expression")?
                            .ok_or_else(|| {
                                super::super::super::internal("binding expression has no receiver")
                            })?;
                    let Some(left) = sequence.read(self.diagnostic_clone_binding_name_worker(
                        expression, preview, context, consumer,
                    ))?
                    else {
                        return Ok(None);
                    };
                    let field = if kind == K::PropertyAccessExpression {
                        "name"
                    } else {
                        "argumentExpression"
                    };
                    let right = self.source.pseudo_child(node, field)?.ok_or_else(|| {
                        super::super::super::internal("binding expression has no accessed name")
                    })?;
                    let Some(right) =
                        sequence.read(self.diagnostic_clone_binding_name_worker(
                            right, preview, context, consumer,
                        ))?
                    else {
                        return Ok(None);
                    };
                    if kind == K::PropertyAccessExpression {
                        format!("{left}.{right}")
                    } else {
                        format!("{left}[{right}]")
                    }
                }
                K::ParenthesizedExpression => {
                    let expression =
                        self.source
                            .pseudo_child(node, "expression")?
                            .ok_or_else(|| {
                                super::super::super::internal(
                                    "binding parentheses have no expression",
                                )
                            })?;
                    let Some(expression) =
                        sequence.read(self.diagnostic_clone_binding_name_worker(
                            expression, preview, context, consumer,
                        ))?
                    else {
                        return Ok(None);
                    };
                    format!("({expression})")
                }
                _ if preview => return Ok(None),
                _ => {
                    return Err(self.diagnostic_dependency(
                        consumer,
                        "native cloned binding-name expression syntax is pending",
                    ));
                }
            }))
        })();
        sequence.finish(result)
    }

    fn diagnostic_binding_clone_effects(
        &mut self,
        root: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut work = vec![root];
            while let Some(node) = work.pop() {
                if self.source.declaration_kind(node)? == K::ComputedPropertyName {
                    sequence.read(
                        self.diagnostic_track_binding_computed_name(node, context, consumer),
                    )?;
                }
                work.extend(
                    self.source
                        .diagnostic_syntax_children(node)?
                        .into_iter()
                        .rev(),
                );
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    fn diagnostic_track_binding_computed_name(
        &mut self,
        node: DeclarationIdentity,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<()> {
        self.is_late_bindable_name_completion(node)?
            .and_then(|late| {
                if late {
                    let expression =
                        self.source
                            .pseudo_child(node, "expression")?
                            .ok_or_else(|| {
                                super::super::super::internal("computed name has no expression")
                            })?;
                    return self.diagnostic_track_computed_name_completion(
                        expression,
                        context.enclosing,
                        context,
                        consumer,
                    );
                }
                Ok(NativeCompletion::clean(()))
            })
    }

    /// `trackComputedName` first resolves at the target enclosing location,
    /// then at the original identifier only if absent, with Value|ExportValue.
    pub(super) fn diagnostic_track_computed_name_completion(
        &mut self,
        expression: DeclarationIdentity,
        enclosing: Option<super::super::symbol_accessibility::ScopeLocation>,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let first = self.source.diagnostic_first_identifier(expression)?;
            let text = self.source.diagnostic_identifier_text(first)?;
            let meaning = super::super::SymbolMeaning::ValueExport;
            let symbol = match sequence.read(
                self.diagnostic_resolve_name_completion(enclosing, &text, meaning, false, consumer),
            )? {
                Some(symbol) => Some(symbol),
                None => sequence.read(self.diagnostic_resolve_name_completion(
                    Some(first.into()),
                    &text,
                    meaning,
                    false,
                    consumer,
                ))?,
            };
            if let Some(symbol) = symbol {
                self.diagnostic_track_symbol(
                    symbol,
                    enclosing,
                    super::super::SymbolMeaning::Value,
                    context,
                    consumer,
                )?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }
}
