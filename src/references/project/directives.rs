//! What a file's triple-slash reference directives ask loading to follow.
//! `crate::pragmas` recognizes the directives and their attributes.

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

/// What one reference directive asks loading to follow. As in TypeScript 7,
/// `no-default-lib="true"` follows nothing; otherwise the first of `types`,
/// `lib`, and `path` that is present decides, and an empty value follows
/// nothing.
fn follow(reference: &crate::pragmas::Reference<'_>) -> Option<Reference> {
    if reference
        .attribute("no-default-lib")
        .is_some_and(|(value, _)| value == "true")
    {
        return None;
    }
    let (directive, offset) = if let Some((name, offset)) = reference.attribute("types") {
        let mode = match reference
            .attribute("resolution-mode")
            .map(|(value, _)| value)
        {
            Some("import") => Some(ResolutionMode::Import),
            Some("require") => Some(ResolutionMode::Require),
            _ => None,
        };
        (Directive::Types(name.into(), mode), offset)
    } else if let Some((name, offset)) = reference.attribute("lib") {
        (Directive::Lib(name.into()), offset)
    } else {
        let (path, offset) = reference.attribute("path")?;
        (Directive::Path(path.into()), offset)
    };
    let empty = match &directive {
        Directive::Path(name) | Directive::Types(name, _) | Directive::Lib(name) => name.is_empty(),
    };
    (!empty).then_some(Reference { directive, offset })
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
