//! Native lazy JSDoc questions use actual declaration identity. Scanner hints
//! and parsed top-level tags are distinct facts, not alternate parsers.

use super::super::model::DemandError;
use super::{DeclarationIdentity, Source};
use crate::Span;

impl Source<'_, '_> {
    /// `IsTypeDeclaration` uses the shared syntax rule, including the
    /// enclosing clause's type-only bit for import/export specifiers.
    pub(in crate::project::semantic) fn is_type_declaration(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(crate::syntax::is_type_declaration(
            &captured.ast,
            &captured.parents,
            declaration.node,
        ))
    }

    /// `GetJSDocDeprecatedTag`: the first top-level tag in attached comment
    /// order. The native parsed-node cache retains absence as well as tags;
    /// an acquired product failure remains a failure on later observations.
    pub(in crate::project::semantic) fn jsdoc_deprecated_tag(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<Span>, DemandError> {
        if let Some(cached) = self.jsdoc_tags.get(&declaration) {
            return cached.clone().map(|tags| tags.deprecated);
        }
        let node = self.syntax(declaration.file)?.ast.node(declaration.node);
        let kind = node.syntax_kind;
        let Some(attachment) = node.native_jsdoc.clone() else {
            return Ok(None);
        };
        let result: Result<_, DemandError> = if let Some(tags) = &attachment.parsed {
            Ok(tags.clone())
        } else {
            let input = self.inputs.sources.get(declaration.file);
            let path = input.path.to_str().expect("captured path is exact Unicode");
            self.parser
                .jsdoc_tags(
                    input.text.as_str(),
                    path,
                    kind,
                    &attachment,
                    self.control.clone(),
                )
                .map_err(|failure| match failure {
                    crate::parser::Failure::Interrupted(stop) => {
                        super::super::resource_failure(stop).into()
                    }
                    crate::parser::Failure::Syntax(error) => crate::analysis::AnalysisError::from(
                        crate::analysis::InputError::unparsable(error, &input.text, &input.path),
                    )
                    .into(),
                })
        };
        self.jsdoc_tags.reserve(1);
        self.jsdoc_tags.insert(declaration, result.clone());
        result.map(|tags| tags.deprecated)
    }

    /// `IsDeprecatedDeclarationWithCachedFlags`: combined flags only admit
    /// the question. The first directly flagged ancestor owns confirmation;
    /// a negative parsed answer there does not continue farther upward.
    pub(in crate::project::semantic) fn deprecated_declaration(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        if !crate::syntax::possibly_deprecated(&captured.ast, &captured.parents, declaration.node) {
            return Ok(false);
        }
        let mut current = Some(declaration.node);
        while let Some(node) = current {
            if captured
                .ast
                .node(node)
                .native_jsdoc
                .as_ref()
                .is_some_and(|facts| facts.possible_deprecated)
            {
                return self
                    .jsdoc_deprecated_tag(DeclarationIdentity::new(declaration.file, node))
                    .map(|tag| tag.is_some());
            }
            current = captured.parents[node.index()];
        }
        Ok(false)
    }
}
