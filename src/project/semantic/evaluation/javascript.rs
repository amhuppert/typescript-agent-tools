//! The checker's decisions that ask whether a node is in a JavaScript file
//! (`IsInJSFile`, `crate::project::semantic::inputs::CheckerInputs::
//! in_javascript_file`) or whether a JavaScript file is checked
//! (`crate::checking`), where they change a type: `isJSLiteralType`,
//! `isUncheckedJSSuggestion`, and `widenTypeInferredFromInitializer`'s
//! JavaScript branch. docs/references.md lists every such branch of
//! TypeScript 7's checker and binder with what this tool does there.

use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use crate::project::semantic::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, ObjectFlags, SpecialName,
    TypeFlags, TypeId, TypeView,
};
use crate::project::semantic::source::ExpressionRoot;
use crate::project::sources::FileId;
use crate::references::Location;

impl Evaluator<'_, '_> {
    /// `isJSLiteralType`: a JavaScript literal (`ObjectFlags::JS_LITERAL`),
    /// a union of only them, an intersection with one, or an instantiable
    /// type constrained to one, whose missing members read as `any`; never
    /// under `noImplicitAny`.
    pub(super) fn is_js_literal_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if self.inputs.semantic_options().no_implicit_any {
            return Ok(NativeCompletion::clean(false));
        }
        if self
            .model
            .object_flags(ty)?
            .intersects(ObjectFlags::JS_LITERAL)
        {
            return Ok(NativeCompletion::clean(true));
        }
        let mut sequence = NativeSequence::new();
        let result = (|| match self.model.get(ty)? {
            TypeView::Union(types) => {
                for &member in types.iter() {
                    if !sequence.read(self.is_js_literal_type_completion(member, consumer))? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            TypeView::Intersection(types) => {
                for &member in types.iter() {
                    if sequence.read(self.is_js_literal_type_completion(member, consumer))? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            _ if self.model.has_type_flags(ty, TypeFlags::INSTANTIABLE) => {
                let constraint =
                    sequence.read(self.base_constraint_or_type_completion(ty, consumer))?;
                Ok(constraint != ty
                    && sequence.read(self.is_js_literal_type_completion(constraint, consumer))?)
            }
            _ => Ok(false),
        })();
        sequence.finish(result)
    }

    /// `isUncheckedJSSuggestion`: whether a missing name at `node` is only a
    /// suggestion, in a plain JavaScript file (`crate::checking`), unless
    /// the suggestion is declared in another script, or, with
    /// `exclude_classes`, is a class, or `node` reads `this`, where the
    /// suggestion has no `extends` clause or decorators.
    pub(super) fn is_unchecked_js_suggestion(
        &mut self,
        node: ExpressionRoot,
        suggestion: Option<ReferenceSymbol>,
        exclude_classes: bool,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        if !self.inputs.file_checking(node.file).plain_javascript() {
            return Ok(false);
        }
        let (declaration_file, value_declaration, class) = match suggestion {
            None => (None, None, false),
            Some(suggestion) => {
                let first = self
                    .reference_symbol_declarations(suggestion, consumer)?
                    .into_iter()
                    .next();
                let file = first.map(|declaration| match declaration {
                    ReferenceDeclaration::Captured(declaration) => declaration.file,
                    ReferenceDeclaration::Json { file, .. } => file,
                });
                let value = match self.reference_symbol_value_declaration(suggestion, consumer)? {
                    Some(ReferenceDeclaration::Captured(declaration)) => Some(declaration),
                    _ => None,
                };
                let class = self.reference_symbol_flags(suggestion, consumer)?.class;
                (file, value, class)
            }
        };
        let declared_in_another_script = declaration_file.is_some_and(|file: FileId| {
            file != node.file && !self.inputs.bindings.file_is_module(file)
        });
        let no_extends_or_decorators = self
            .source
            .suggestion_has_no_extends_or_decorators(value_declaration)?;
        let this_access = exclude_classes && self.source.is_this_property_access(node)?;
        let class_without_heritage = exclude_classes && class && no_extends_or_decorators;
        let this_without_heritage = this_access && no_extends_or_decorators;
        Ok(!(declared_in_another_script || class_without_heritage || this_without_heritage))
    }

    /// `widenTypeInferredFromInitializer`'s JavaScript branch, after
    /// `getWidenedLiteralTypeForInitializer` gave `widened` for a declaration
    /// that is `javascript` (`IsInJSFile`): there an empty initializer's
    /// type is `any` and an empty array's
    /// `any[]` (`isEmptyLiteralType`, `isEmptyArrayLiteralType`). Its
    /// implicit-any report is a diagnostic this tool does not report.
    pub(super) fn widen_javascript_initializer(
        &mut self,
        javascript: bool,
        widened: TypeId,
    ) -> Result<TypeId, DemandError> {
        if !javascript {
            return Ok(widened);
        }
        if widened == self.model.empty_array_element() {
            return Ok(self.model.special(SpecialName::Any));
        }
        if self.is_empty_array_literal_type(widened)? {
            return self.installed_any_array_type();
        }
        Ok(widened)
    }
}
