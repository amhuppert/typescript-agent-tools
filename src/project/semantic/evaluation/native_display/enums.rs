//! Enum naming is based on the actual enum/member symbols, not its payload.
use super::super::super::model::NativeSequence;
use super::super::SymbolMeaning;
use super::*;

impl Evaluator<'_, '_> {
    /// `typeToTypeNode`'s EnumLike branch. Published diagnostic entries have
    /// maxExpansionDepth=0, so `shouldExpandType` never expands enum unions.
    pub(super) fn diagnostic_enum_type_node(
        &mut self,
        ty: TypeId,
        context: &mut Context,
        consumer: &Location,
    ) -> NativeResult<Node> {
        let symbol = self
            .reference_symbol_of_type(ty, consumer)?
            .ok_or_else(|| super::super::super::internal("enum type has no symbol"))?;
        if !self.reference_symbol_flags(symbol, consumer)?.enum_member {
            return self.symbol_type_node(symbol, SymbolMeaning::Type, None, context, consumer);
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let parent = sequence
                .read(self.reference_parent_of_symbol_completion(symbol, consumer))?
                .ok_or_else(|| super::super::super::internal("enum member has no parent symbol"))?;
            let parent_name = sequence.read(self.symbol_type_node(
                parent,
                SymbolMeaning::Type,
                None,
                context,
                consumer,
            ))?;
            if sequence.read(self.declared_reference_type_completion(parent, consumer))? == ty {
                return Ok(parent_name);
            }
            // `ast.SymbolName` observes the actual member name, without the
            // getNameOfSymbolAsWritten declaration/qualification operation.
            let name =
                String::from_utf8(self.reference_symbol_name(symbol, consumer)?).map_err(|_| {
                    self.diagnostic_dependency(consumer, "enum member name is not scalar text")
                })?;
            if crate::lexer::is_identifier_text(&name) {
                return Ok(parent_name.append_reference(
                    Node::text(name),
                    Vec::new(),
                    context.flags.has(1 << 30),
                ));
            }
            let parent_name = match parent_name {
                Node::Import {
                    specifier,
                    qualifier,
                    arguments,
                    resolution_mode,
                    ..
                } => Node::Import {
                    specifier,
                    typeof_: true,
                    qualifier,
                    arguments,
                    resolution_mode,
                },
                Node::Named(name, _) => Node::Prefix("typeof ", name),
                Node::Reference(name, _) => Node::Prefix("typeof ", Box::new(Node::text(name))),
                _ => {
                    return Err(super::super::super::internal(
                        "native enum parent name is neither ImportType nor TypeReference",
                    )
                    .into());
                }
            };
            Ok(Node::Indexed(
                Box::new(parent_name),
                Box::new(Node::text(crate::printer::quoted_text(
                    &name,
                    if context.flags.has(1 << 28) {
                        '\''
                    } else {
                        '"'
                    },
                ))),
            ))
        })();
        sequence.finish(result)
    }
}
