//! Anonymous value names are a native serialization decision, not a name
//! cached by the type factory. Module clones retain their actual symbol here.

use super::{Context, DiagnosticTypeFormat, Evaluator};
use crate::project::semantic::evaluation::{ReferenceDeclaration, ReferenceSymbol, SymbolMeaning};
use crate::project::semantic::model::{DemandError, ObjectFlags, TypeId, TypeView};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;

impl Evaluator<'_, '_> {
    /// Bounded product summaries enter the same `shouldEmitTypeOfSymbol`
    /// decision with native default flags and their actual recursion state.
    pub(in crate::project::semantic) fn anonymous_symbol_display(
        &mut self,
        ty: TypeId,
        visited: bool,
        consumer: &Location,
    ) -> Result<Option<(Vec<u8>, bool)>, DemandError> {
        if !matches!(
            self.model.get(ty)?,
            TypeView::Object(_) | TypeView::LazyObject(_)
        ) {
            return Ok(None);
        }
        if self
            .model
            .object_flags(ty)?
            .intersects(ObjectFlags::INSTANTIATION_EXPRESSION_TYPE)
        {
            let mut context = Context::new(DiagnosticTypeFormat::DEFAULT, None, false);
            if visited {
                context.visited.insert(ty);
            }
            return self
                .diagnostic_instantiation_expression_node(ty, &mut context, consumer)?
                .map(|node| node.map(|node| (node.print(), true)))
                .admit();
        }
        let Some(flags) = self.type_symbol_flags(ty)? else {
            return Ok(None);
        };
        if !(flags.class || flags.enum_ || flags.value_module || flags.function || flags.method) {
            return Ok(None);
        }
        let mut context = Context::new(DiagnosticTypeFormat::DEFAULT, None, false);
        if visited {
            context.visited.insert(ty);
        }
        let Some(symbol) = self.anonymous_typeof_symbol(ty, &context, false, false, consumer)?
        else {
            return Ok(None);
        };
        self.symbol_type_display(symbol, SymbolMeaning::Value, consumer)
            .map(|display| Some((display.name.into_bytes(), display.type_query)))
    }

    /// `shouldEmitTypeOfSymbol` and `shouldWriteTypeOfFunctionSymbol` share
    /// one decision for product and diagnostic displays. The class base read
    /// precedes the enum/module shortcut exactly as in the native expression.
    pub(super) fn anonymous_typeof_symbol(
        &mut self,
        ty: TypeId,
        context: &Context,
        force_class_expansion: bool,
        force_expansion: bool,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        if force_expansion {
            return Ok(None);
        }
        let Some(flags) = self.type_symbol_flags(ty)? else {
            return Ok(None);
        };
        if !(flags.class || flags.enum_ || flags.value_module || flags.function || flags.method) {
            return Ok(None);
        }
        let Some(mut symbol) = self.reference_symbol_of_type(ty, consumer)? else {
            return Ok(None);
        };
        let flags = self.reference_symbol_flags(symbol, consumer)?;
        if flags.class
            && !force_class_expansion
            && self
                .base_type_variable_of_class(symbol, consumer)?
                .is_none()
        {
            let mut class_literal = false;
            if let Some(ReferenceDeclaration::Captured(declaration)) =
                self.reference_symbol_value_declaration(symbol, consumer)?
            {
                let kind = self.source.declaration_kind(declaration)?;
                if matches!(
                    kind,
                    SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
                ) && context.flags.has(1 << 11)
                {
                    if kind != SyntaxKind::ClassDeclaration {
                        class_literal = true;
                    } else {
                        return Err(self.diagnostic_dependency(consumer, "native class-expression display accessibility requires its shared accessibility result"));
                    }
                }
            }
            if !class_literal {
                return Ok(Some(symbol));
            }
        }
        if flags.enum_ || flags.value_module {
            return Ok(Some(symbol));
        }

        let mut static_method = false;
        if flags.method {
            for declaration in self.reference_symbol_declarations(symbol, consumer)? {
                if let ReferenceDeclaration::Captured(declaration) = declaration
                    && self.source.declaration_is_static(declaration)?
                    && !self.has_late_bindable_index_signature(declaration, consumer)?
                {
                    static_method = true;
                    break;
                }
            }
        }
        let mut nonlocal = false;
        let mut expression = false;
        if flags.function {
            if self.reference_symbol_parent(symbol, consumer)?.is_some() {
                nonlocal = true;
            } else {
                for declaration in self.reference_symbol_declarations(symbol, consumer)? {
                    if let ReferenceDeclaration::Captured(declaration) = declaration {
                        let (is_nonlocal, variable) =
                            self.source.function_display_location(declaration)?;
                        if is_nonlocal {
                            nonlocal = true;
                            expression = variable.is_some();
                            break;
                        }
                    }
                }
            }
        }
        if !(static_method || nonlocal) {
            return Ok(None);
        }
        if expression
            && let Some(ReferenceDeclaration::Captured(declaration)) =
                self.reference_symbol_value_declaration(symbol, consumer)?
            && let Some(parent) = self.source.declaration_parent(declaration)?
            && Some(parent.into()) != context.enclosing
        {
            let raw = self
                .reference_raw_symbol_of_declaration(parent)?
                .ok_or_else(|| {
                    super::super::super::internal(
                        "nonlocal function expression variable lacks its actual symbol",
                    )
                })?;
            symbol = self.get_merged_reference_symbol(raw, consumer)?;
        }
        if !(context.flags.has(1 << 12) || context.visited.contains(&ty)) {
            return Ok(None);
        }
        if context.flags.has(1 << 3) {
            return Err(self.diagnostic_dependency(consumer, "native function structural fallback requires its shared value-accessibility result"));
        }
        Ok(Some(symbol))
    }
}
