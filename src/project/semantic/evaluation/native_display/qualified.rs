//! Native symbol access construction. Acquisition and qualification happen
//! here; the output syntax/printer only retains the resulting node kinds.
use super::super::super::model::NativeSequence;
use super::super::{ReferenceDeclaration, ReferenceSymbol, SymbolMeaning};
use super::*;
use crate::project::binding::NativePropertyKey;

enum Access {
    Entity(Node),
    Indexed(Node),
    Expression(Node),
}
impl Access {
    fn node(self) -> Node {
        match self {
            Self::Entity(node) | Self::Indexed(node) | Self::Expression(node) => node,
        }
    }
}

impl Evaluator<'_, '_> {
    /// `symbolToStringEx` with DoNotIncludeSymbolChain retains enclosing
    /// context for module specifiers and default names while bypassing lexical
    /// qualification. Its nil tracker only observes the raw symbol flags.
    pub(in crate::project::semantic::evaluation) fn diagnostic_unchained_symbol_display_completion(
        &mut self,
        symbol: ReferenceSymbol,
        enclosing: Option<DeclarationIdentity>,
        consumer: &Location,
    ) -> NativeResult<RenderedTypeName> {
        let mut context = Context::new(DiagnosticTypeFormat(0), enclosing, false);
        self.reference_symbol_flags(symbol, consumer)?;
        Ok(self
            .expression_from_symbol_chain(&[symbol], 0, &mut context, consumer)?
            .map(|node| RenderedTypeName(node.print())))
    }

    /// `TypeAlias.ToTypeReferenceNode` uses raw Parent/name entities and no
    /// qualification or approximate-length accounting, before its arguments.
    pub(super) fn diagnostic_raw_symbol_entity(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Node, DemandError> {
        let bytes = self.reference_symbol_name(symbol, consumer)?;
        let name = String::from_utf8(bytes).map_err(|_| {
            self.diagnostic_dependency(consumer, "native raw entity name is not scalar text")
        })?;
        let right = Node::text(name);
        match self.reference_raw_parent_symbol(symbol, consumer)? {
            Some(parent) => Ok(Node::ExpressionAccess {
                left: Box::new(self.diagnostic_raw_symbol_entity(parent, consumer)?),
                name: Box::new(right),
                computed: false,
            }),
            None => Ok(right),
        }
    }

    /// `lookupTypeParameterNodes` always records symbol identity; the
    /// expression variant records it only when the bit/nonfinal gate passes.
    fn diagnostic_chain_parameters(
        &mut self,
        chain: &[ReferenceSymbol],
        index: usize,
        expression: bool,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Option<Vec<Node>>> {
        let write = context.flags.has(1 << 9) && index + 1 < chain.len();
        if expression && !write {
            return Ok(NativeCompletion::clean(None));
        }
        let symbol = chain[index];
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        self.model.observe_symbol_identity(identity);
        if !context.type_parameter_symbol_list.insert(identity) || !write {
            return Ok(NativeCompletion::clean(None));
        }
        // `lookupInstantiatedTypeArgumentNodes` does not resolve an alias
        // until the next actual symbol has CheckFlagsInstantiated.
        let next = self.register_reference_symbol(chain[index + 1], consumer)?;
        if self.property_check_flags(next)? & 1 != 0 {
            let mut sequence = NativeSequence::new();
            let result = (|| {
                let target = if self.reference_symbol_flags(symbol, consumer)?.alias {
                    sequence
                        .read(self.resolve_alias_target_completion(symbol, consumer))?
                        .ok_or_else(|| {
                            super::super::super::internal("native resolveAlias returned nil")
                        })?
                } else {
                    symbol
                };
                let mut parameters = self.outer_type_parameters_of_symbol(target, consumer)?;
                parameters.extend(
                    sequence
                        .read(self.local_type_parameters_of_symbol_completion(target, consumer))?,
                );
                if let Some(mapper) = self.property_mapper(next)? {
                    for parameter in &mut parameters {
                        *parameter = sequence
                            .read(self.map_parameter_completion(*parameter, mapper, consumer))?;
                    }
                }
                let nodes = sequence.read(self.diagnostic_type_list(
                    &parameters,
                    false,
                    context,
                    consumer,
                ))?;
                Ok((!nodes.is_empty()).then_some(nodes))
            })();
            let arguments = sequence.finish(result)?;
            if arguments.value().is_some() {
                return Ok(arguments);
            }
            return arguments.and_then(|_| {
                self.diagnostic_symbol_parameter_declarations(symbol, context, consumer)
            });
        }
        self.diagnostic_symbol_parameter_declarations(symbol, context, consumer)
    }

    /// `typeParametersToTypeParameterDeclarations` uses the target for its
    /// flag decision but the original symbol's declarations/value declaration.
    fn diagnostic_symbol_parameter_declarations(
        &mut self,
        symbol: ReferenceSymbol,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Option<Vec<Node>>> {
        let property = self.register_reference_symbol(symbol, consumer)?;
        let target = self.property_target_symbol(property)?;
        let flags =
            self.reference_symbol_flags(ReferenceSymbol::from_property(target), consumer)?;
        let parameters = if flags.class || flags.interface || flags.alias {
            self.local_type_parameters_of_symbol_completion(symbol, consumer)?
        } else if flags.function {
            let sites = match self.reference_symbol_value_declaration(symbol, consumer)? {
                Some(ReferenceDeclaration::Captured(declaration)) => {
                    self.source.declaration_type_parameters(declaration)?
                }
                None => Vec::new(),
                Some(ReferenceDeclaration::Json { .. }) => {
                    return Err(self.diagnostic_dependency(
                        consumer,
                        "native function type parameters lack a captured declaration",
                    ));
                }
            };
            self.append_type_parameters_completion(Vec::new(), &sites)?
        } else {
            NativeCompletion::clean(Vec::new())
        };
        parameters.and_then(|parameters| {
            let mut sequence = NativeSequence::new();
            let result = (|| {
                let mut nodes = Vec::with_capacity(parameters.len());
                for parameter in parameters {
                    let constraint =
                        sequence.read(self.parameter_constraint_completion(parameter, consumer))?;
                    let constraint = if let Some(constraint) = constraint {
                        let annotation = self.constraint_declaration(parameter, consumer)?;
                        Some(Box::new(sequence.read(
                            self.diagnostic_type_node_with_reuse(
                                constraint, annotation, context, consumer,
                            ),
                        )?))
                    } else {
                        None
                    };
                    nodes.push(Node::TypeParameter(sequence.read(
                        self.diagnostic_parameter_declaration(
                            parameter, constraint, context, consumer,
                        ),
                    )?));
                }
                Ok((!nodes.is_empty()).then_some(nodes))
            })();
            sequence.finish(result)
        })
    }

    /// `symbolToExpression` shares the exact tracker/chain prelude, then
    /// `createExpressionFromSymbolChain` acquires names right before left.
    pub(super) fn symbol_expression_node(
        &mut self,
        symbol: ReferenceSymbol,
        meaning: SymbolMeaning,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        self.diagnostic_symbol_chain_completion(symbol, meaning, context, false, consumer)?
            .and_then(|chain| {
                self.expression_from_symbol_chain(&chain, chain.len() - 1, context, consumer)
            })
    }

    fn expression_from_symbol_chain(
        &mut self,
        chain: &[ReferenceSymbol],
        index: usize,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        self.diagnostic_chain_parameters(chain, index, true, context, consumer)?
            .and_then(|parameters| {
                let symbol = chain[index];
                if index == 0 {
                    context.flags.0 |= 1 << 24;
                }
                let name = self.symbol_name_as_written_in_context(symbol, context, consumer);
                if index == 0 {
                    context.flags.0 &= !(1 << 24);
                }
                let mut name = name?;
                if name.starts_with(['\'', '"'])
                    && self.diagnostic_external_module_symbol(symbol, consumer)?
                {
                    return Ok(self
                        .diagnostic_module_specifier_completion(symbol, context, None, consumer)?
                        .map(|specifier| {
                            context.length += 2 + specifier.len();
                            Node::text(crate::printer::quoted_text(&specifier, '"'))
                        }));
                }
                if index == 0
                    || crate::lexer::is_identifier_text(name.strip_prefix('#').unwrap_or(&name))
                {
                    context.length += name.len() + 1;
                    let right = Node::text(name);
                    let node = if index == 0 {
                        NativeCompletion::clean(right)
                    } else {
                        self.expression_from_symbol_chain(chain, index - 1, context, consumer)?
                            .map(|left| Node::ExpressionAccess {
                                left: Box::new(left),
                                name: Box::new(right),
                                computed: false,
                            })
                    };
                    return Ok(node
                        .map(|node| Self::diagnostic_expression_with_arguments(node, parameters)));
                }
                if name.starts_with('[') {
                    name = name[1..name.len() - 1].to_owned();
                }
                let right = if name.starts_with(['\'', '"'])
                    && !self.reference_symbol_flags(symbol, consumer)?.enum_member
                {
                    let literal = crate::strings::unquote_string(&name);
                    context.length += literal.len() + 2;
                    Node::text(crate::printer::quoted_text(
                        &literal,
                        if name.starts_with('\'') { '\'' } else { '"' },
                    ))
                } else {
                    context.length += name.len();
                    Node::text(name)
                };
                context.length += 2;
                Ok(self
                    .expression_from_symbol_chain(chain, index - 1, context, consumer)?
                    .map(|left| {
                        Self::diagnostic_expression_with_arguments(
                            Node::ExpressionAccess {
                                left: Box::new(left),
                                name: Box::new(right),
                                computed: true,
                            },
                            parameters,
                        )
                    }))
            })
    }

    fn diagnostic_expression_with_arguments(node: Node, arguments: Option<Vec<Node>>) -> Node {
        match arguments {
            Some(arguments) if !arguments.is_empty() => Node::Named(Box::new(node), arguments),
            _ => node,
        }
    }

    /// `symbolToTypeNode` retains entity/indexed/instantiation distinctions;
    /// module roots share the same complete access builder before specifiers.
    pub(super) fn symbol_type_node(
        &mut self,
        symbol: ReferenceSymbol,
        meaning: SymbolMeaning,
        arguments: Option<Vec<Node>>,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        self.diagnostic_symbol_chain_completion(
            symbol,
            meaning,
            context,
            !context.flags.has(1 << 14),
            consumer,
        )?
        .and_then(|chain| {
            let typeof_ = meaning == SymbolMeaning::Value;
            if self.diagnostic_external_module_symbol(chain[0], consumer)? {
                let mut sequence = NativeSequence::new();
                let result = (|| {
                    let access = if chain.len() > 1 {
                        Some(sequence.read(self.access_from_symbol_chain(
                            &chain,
                            chain.len() - 1,
                            1,
                            &arguments,
                            context,
                            consumer,
                        ))?)
                    } else {
                        None
                    };
                    let parameters = match arguments {
                        Some(arguments) => Some(arguments),
                        None => sequence.read(
                            self.diagnostic_chain_parameters(&chain, 0, false, context, consumer),
                        )?,
                    };
                    let (specifier, resolution_mode) = sequence
                        .read(self.diagnostic_import_type_specifier(chain[0], context, consumer))?;
                    context.length += specifier.len() + 10;
                    let arguments = parameters.unwrap_or_default();
                    match access {
                        None => Ok(Node::Import {
                            specifier,
                            typeof_,
                            qualifier: None,
                            arguments,
                            resolution_mode,
                        }),
                        Some(Access::Entity(name)) => Ok(Node::Import {
                            specifier,
                            typeof_,
                            qualifier: Some(Box::new(name)),
                            arguments,
                            resolution_mode,
                        }),
                        Some(Access::Indexed(mut node)) => loop {
                            match node {
                                Node::Indexed(object, index) => match *object {
                                    nested @ Node::Indexed(..) => node = nested,
                                    Node::Named(qualifier, _) => {
                                        return Ok(Node::Indexed(
                                            Box::new(Node::Import {
                                                specifier,
                                                typeof_,
                                                qualifier: Some(qualifier),
                                                arguments,
                                                resolution_mode,
                                            }),
                                            index,
                                        ));
                                    }
                                    _ => {
                                        return Err(super::super::super::internal(
                                            "native indexed import lacks reference object",
                                        )
                                        .into());
                                    }
                                },
                                _ => {
                                    return Err(super::super::super::internal(
                                        "native indexed access shape changed",
                                    )
                                    .into());
                                }
                            }
                        },
                        Some(Access::Expression(_)) => Err(self.diagnostic_dependency(
                            consumer,
                            "native import qualifier is not an entity or indexed access",
                        )),
                    }
                })();
                return sequence.finish(result);
            }
            Ok(self
                .access_from_symbol_chain(
                    &chain,
                    chain.len() - 1,
                    0,
                    &arguments,
                    context,
                    consumer,
                )?
                .map(|access| match access {
                    Access::Indexed(node) => node,
                    Access::Entity(name) => {
                        if typeof_ {
                            Node::Prefix("typeof ", Box::new(name))
                        } else {
                            Node::Named(Box::new(name), arguments.unwrap_or_default())
                        }
                    }
                    Access::Expression(node) => {
                        if typeof_ {
                            Node::Prefix("typeof ", Box::new(node))
                        } else {
                            node
                        }
                    }
                }))
        })
    }

    /// `createAccessFromSymbolChain`: parameter nodes, export-name probing,
    /// computed-name fallback and actual member identity precede left access.
    fn access_from_symbol_chain(
        &mut self,
        chain: &[ReferenceSymbol],
        index: usize,
        stopper: usize,
        override_arguments: &Option<Vec<Node>>,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Access> {
        let parameters = if index + 1 == chain.len() {
            NativeCompletion::clean(override_arguments.clone())
        } else {
            self.diagnostic_chain_parameters(chain, index, false, context, consumer)?
        };
        parameters.and_then(|parameters| {
            let mut sequence = NativeSequence::new();
            let result = (|| {
                let symbol = chain[index];
                let parent = index.checked_sub(1).map(|index| chain[index]);
                let mut name = String::new();
                if index == 0 {
                    context.flags.0 |= 1 << 24;
                    let result = self.symbol_name_as_written_in_context(symbol, context, consumer);
                    context.flags.0 &= !(1 << 24);
                    name = result?;
                    context.length += name.len() + 1;
                } else if let Some(parent) = parent
                    && let Some(exports) = sequence
                        .read(self.reference_exports_of_symbol_completion(parent, consumer))?
                {
                    let key = self.reference_symbol_key(symbol, consumer)?;
                    let direct = exports
                        .iter()
                        .find(|(name, _)| name == &key)
                        .map(|(_, symbol)| *symbol);
                    if key.fixed_bytes() != Some(b"export=".as_slice())
                        && !key.is_known_symbol()
                        && let Some(direct) = direct
                        && sequence
                            .read(
                                self.get_symbol_if_same_reference_completion(
                                    direct, symbol, consumer,
                                ),
                            )?
                            .is_some()
                    {
                        name = self.diagnostic_fixed_key_text(&key, consumer)?;
                    } else {
                        let mut candidates: Vec<(
                            crate::project::binding::NativeSymbolIdentity,
                            NativePropertyKey,
                            std::sync::Arc<crate::project::binding::NativeSymbol>,
                        )> = Vec::new();
                        for (key, exported) in exports.iter() {
                            if sequence
                                .read(self.get_symbol_if_same_reference_completion(
                                    *exported, symbol, consumer,
                                ))?
                                .is_some()
                                && !key.is_known_symbol()
                                && key.fixed_bytes() != Some(b"export=".as_slice())
                            {
                                let identity =
                                    self.reference_symbol_identity(*exported, consumer)?;
                                if let Some((_, existing_key, _)) =
                                    candidates.iter_mut().find(|(id, _, _)| *id == identity)
                                {
                                    *existing_key = key.clone();
                                } else {
                                    let property =
                                        self.register_reference_symbol(*exported, consumer)?;
                                    candidates.push((
                                        identity,
                                        key.clone(),
                                        self.property_native_symbol(property, consumer)?,
                                    ));
                                }
                            }
                        }
                        crate::project::binding::try_sort_by(&mut candidates, &|left, right| {
                            self.model
                                .compare_native_symbols(Some(&left.2), Some(&right.2))
                                .map_err(|error| self.generic_binding_failure(error, consumer))
                        })?;
                        if let Some((_, key, _)) = candidates.first() {
                            name = self.diagnostic_fixed_key_text(key, consumer)?;
                        }
                    }
                }
                if name.is_empty() {
                    if let Some(expression) =
                        self.diagnostic_computed_entity_name(symbol, consumer)?
                    {
                        if index == 0 {
                            return Err(super::super::super::internal(
                                "computed entity access has no parent chain",
                            )
                            .into());
                        }
                        return Ok(self
                            .access_from_symbol_chain(
                                chain,
                                index - 1,
                                stopper,
                                override_arguments,
                                context,
                                consumer,
                            )?
                            .map(|left| match left {
                                Access::Entity(left) => Access::Indexed(Node::Indexed(
                                    Box::new(Node::Parenthesized(Box::new(Node::Prefix(
                                        "typeof ",
                                        Box::new(left),
                                    )))),
                                    Box::new(Node::Prefix("typeof ", Box::new(expression))),
                                )),
                                other => other,
                            }));
                    }
                    name = self.symbol_name_as_written_in_context(symbol, context, consumer)?;
                }
                context.length += name.len() + 1;
                if !context.flags.has(1 << 4)
                    && let Some(parent) = parent
                    && sequence
                        .read(self.reference_members_of_symbol_completion(parent, consumer))?
                        .is_some()
                {
                    let key = self.reference_symbol_key(symbol, consumer)?;
                    let members = sequence
                        .read(self.reference_members_of_symbol_completion(parent, consumer))?;
                    if members
                        .as_ref()
                        .is_some_and(|members| members.iter().any(|(name, _)| name == &key))
                    {
                        // Native repeats getMembersOfSymbol for the nil table,
                        // nil entry and reference-equivalence operands.
                        let members = sequence
                            .read(self.reference_members_of_symbol_completion(parent, consumer))?;
                        let member = members
                            .as_ref()
                            .and_then(|members| members.iter().find(|(name, _)| name == &key))
                            .map(|(_, symbol)| *symbol)
                            .ok_or_else(|| {
                                super::super::super::internal(
                                    "native member table changed between established reads",
                                )
                            })?;
                        if sequence
                            .read(
                                self.get_symbol_if_same_reference_completion(
                                    member, symbol, consumer,
                                ),
                            )?
                            .is_some()
                        {
                            return Ok(self
                                .access_from_symbol_chain(
                                    chain,
                                    index - 1,
                                    stopper,
                                    override_arguments,
                                    context,
                                    consumer,
                                )?
                                .map(|left| {
                                    let object = match left {
                                        Access::Indexed(node) => node,
                                        other => Node::Named(
                                            Box::new(other.node()),
                                            parameters.unwrap_or_default(),
                                        ),
                                    };
                                    Access::Indexed(Node::Indexed(
                                        Box::new(object),
                                        Box::new(Node::text(crate::printer::quoted_text(
                                            &name, '"',
                                        ))),
                                    ))
                                }));
                        }
                    }
                }
                let right = Node::text(name);
                if index <= stopper {
                    return Ok(NativeCompletion::clean(Access::Entity(right)));
                }
                Ok(self
                    .access_from_symbol_chain(
                        chain,
                        index - 1,
                        stopper,
                        override_arguments,
                        context,
                        consumer,
                    )?
                    .map(|left| {
                        let entity = matches!(left, Access::Entity(_));
                        let node = Node::ExpressionAccess {
                            left: Box::new(left.node()),
                            name: Box::new(right),
                            computed: false,
                        };
                        if !context.flags.has(1 << 30)
                            || entity && parameters.as_ref().is_none_or(Vec::is_empty)
                        {
                            Access::Entity(node)
                        } else {
                            Access::Expression(Self::diagnostic_expression_with_arguments(
                                node, parameters,
                            ))
                        }
                    }))
            })();
            sequence.finish(result)?.and_then(Ok)
        })
    }

    fn diagnostic_fixed_key_text(
        &self,
        key: &NativePropertyKey,
        consumer: &Location,
    ) -> Result<String, DemandError> {
        let bytes = key.fixed_bytes().ok_or_else(|| {
            self.diagnostic_dependency(
                consumer,
                "native generated key cannot be emitted as a fixed identifier",
            )
        })?;
        std::str::from_utf8(bytes).map(str::to_owned).map_err(|_| {
            self.diagnostic_dependency(consumer, "native entity name is not scalar text")
        })
    }

    fn diagnostic_computed_entity_name(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<Node>, DemandError> {
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            if let ReferenceDeclaration::Captured(declaration) = declaration
                && self.source.declaration_name_site(declaration)?.is_some()
            {
                return self
                    .source
                    .diagnostic_computed_entity_name(declaration)
                    .map(|name| name.map(Node::text));
            }
        }
        Ok(None)
    }
}
