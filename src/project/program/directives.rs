//! What a file's triple-slash reference directives ask loading to follow.
//! `crate::pragmas` recognizes the directives and decides which reference
//! each makes; the parser validates them by the same rule.

use super::super::binding::ResolutionMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Directive {
    /// `path="..."`: a file relative to the containing file.
    Path(String),
    /// `types="..."`: a type package, with an optional `resolution-mode`.
    Types(String, Option<ResolutionMode>),
    /// `lib="..."`: a standard library.
    Lib(String),
}

/// A directive and the byte offset of its value, for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Reference {
    pub(super) directive: Directive,
    pub(super) offset: usize,
}

pub(super) fn scan(text: &str) -> Vec<Reference> {
    crate::pragmas::references(text)
        .filter_map(|reference| follow(&reference))
        .collect()
}

/// What one reference directive asks loading to follow, by the rule
/// `crate::pragmas` owns. Values stay as written, even empty: TypeScript
/// reports an empty name where it looks for the target.
fn follow(reference: &crate::pragmas::Reference<'_>) -> Option<Reference> {
    use crate::pragmas::{Directive as Pragma, TypesMode};
    let (directive, offset) = match reference.directive() {
        // The parser rejects an invalid directive before loading follows it.
        Pragma::Ignored | Pragma::Invalid => return None,
        Pragma::Types(name, offset, mode) => {
            let mode = match mode {
                TypesMode::Import => Some(ResolutionMode::Import),
                TypesMode::Require => Some(ResolutionMode::Require),
                TypesMode::Unset | TypesMode::Invalid => None,
            };
            (Directive::Types(name.into(), mode), offset)
        }
        Pragma::Lib(name, offset) => (Directive::Lib(name.into()), offset),
        Pragma::Path(name, offset) => (Directive::Path(name.into()), offset),
    };
    Some(Reference { directive, offset })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_reference_directives_from_leading_comments_only() {
        let text = "#!/usr/bin/env node\n/* lead */\n/// <reference path=\"./a.ts\" />\n///<REFERENCE types='pkg' resolution-mode=\"require\"/>\n/// <reference lib=\"es2015\" />\n/// <reference no-default-lib=\"true\" />\n/// <reference path=\"\" />\n/// <amd-module name=\"x\" />\n/// <reference path=\"open\"\nconst a = 1;\n/// <reference path=\"late.ts\" />\n";
        let references = scan(text);
        assert_eq!(
            references
                .iter()
                .map(|reference| reference.directive.clone())
                .collect::<Vec<_>>(),
            [
                Directive::Path("./a.ts".into()),
                Directive::Types("pkg".into(), Some(ResolutionMode::Require)),
                Directive::Lib("es2015".into()),
                Directive::Path("".into()),
                // TypeScript 7 needs no closing `/>`.
                Directive::Path("open".into()),
            ]
        );
        assert_eq!(
            &text[references[0].offset..references[0].offset + 6],
            "./a.ts"
        );
        assert_eq!(&text[references[1].offset..references[1].offset + 3], "pkg");
    }
}
