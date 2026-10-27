//! TypeScript 5.6.3's compiler option catalog, the shape of the data
//! `tools/ts56-option-catalog.mjs` extracts from the pinned compiler into
//! `catalog_5_6`: each `optionDeclarations` entry as `convertJsonOption`
//! converts it, with the nested list element declaration that
//! `convertJsonOptionOfListType` converts each entry with.

use super::catalog_5_6;

/// One option declaration, by which `typescript_5_6::validate_option`
/// checks each value and `typescript_5_6::conversion` converts it.
#[derive(Debug)]
pub(in crate::project::program) struct Declaration {
    pub(super) name: &'static str,
    pub(super) kind: Type,
    /// `isCommandLineOnly`: TS6266 in a configuration file.
    pub(super) command_line_only: bool,
    /// `isFilePath`: the value names a file or directory.
    pub(super) file_path: bool,
    /// `allowConfigDirTemplateSubstitution`.
    pub(super) template: bool,
    /// `listPreserveFalsyValues`.
    pub(super) preserve_falsy: bool,
}

/// What a declaration's value converts as.
#[derive(Debug)]
pub(in crate::project::program) enum Type {
    Boolean,
    Number,
    String,
    /// Any JavaScript object, an array included (`typeof`).
    Object,
    /// A string whose lowered form must be one of the enumeration's keys.
    Enum(&'static Enumeration),
    List(&'static Element),
}

/// A list's element declaration, whose name diagnostics about an element
/// use, such as `suffix` for `moduleSuffixes`.
#[derive(Debug)]
pub(in crate::project::program) struct Element {
    pub(super) name: &'static str,
    /// Never a list.
    pub(super) kind: Type,
    pub(super) file_path: bool,
}

/// An enumeration's keys, each with the value it converts to (a number, or
/// for `lib` a file name), in declaration order, and the keys TS6046 lists:
/// all but the declaration's `deprecatedKeys`.
#[derive(Debug)]
pub(in crate::project::program) struct Enumeration {
    pub(super) entries: &'static [(&'static str, &'static str)],
    pub(super) listed: &'static str,
}

impl Enumeration {
    /// The value `key`, already lowered, converts to.
    pub(super) fn value(&self, key: &str) -> Option<&'static str> {
        self.entries
            .iter()
            .find(|(name, _)| *name == key)
            .map(|&(_, value)| value)
    }
}

impl Type {
    /// The type TS5024 names (`getCompilerOptionValueTypeString`): an
    /// enumeration is a string, a list an `Array`.
    pub(super) fn name(&self) -> &'static str {
        match self {
            Type::Boolean => "boolean",
            Type::Number => "number",
            Type::String | Type::Enum(_) => "string",
            Type::Object => "object",
            Type::List(_) => "Array",
        }
    }
}

/// The declaration of the option named exactly `name`, as
/// `getCommandLineCompilerOptionsMap` keys them.
pub(super) fn declaration(name: &str) -> Option<&'static Declaration> {
    catalog_5_6::OPTIONS
        .iter()
        .find(|option| option.name == name)
}

/// Every declared option name, in order, for spelling suggestions.
pub(super) fn names() -> impl Iterator<Item = &'static str> {
    catalog_5_6::OPTIONS.iter().map(|option| option.name)
}

/// Whether `lowered`, an unknown option's name after JavaScript's
/// `toLowerCase`, is a key of `compilerOptionsAlternateMode`'s name map:
/// any `--build` option's name, compiler options such as `incremental`
/// included, lowered.
pub(super) fn build_option(lowered: &str) -> bool {
    catalog_5_6::BUILD_KEYS.contains(&lowered)
}

/// The enumeration of option `name`, or of its list's elements.
pub(super) fn enumeration(name: &str) -> Option<&'static Enumeration> {
    match &declaration(name)?.kind {
        Type::Enum(enumeration) => Some(enumeration),
        Type::List(Element {
            kind: Type::Enum(enumeration),
            ..
        }) => Some(enumeration),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `lib` option's enumeration is `libMap`, by which the TypeScript
    /// 5.6 dialect's library catalog, the one owner of what a library name
    /// selects, reads `lib` entries: the same keys, files, and TS6046 list.
    #[test]
    fn the_lib_enumeration_is_the_library_catalogs() {
        let libraries = super::super::super::libraries::Catalog::TYPESCRIPT_5_6;
        let lib = enumeration("lib").unwrap();
        assert_eq!(lib.entries.len(), 86);
        for &(key, file) in lib.entries {
            assert_eq!(libraries.option_file(key), Some(file), "{key}");
        }
        assert_eq!(libraries.option_names(), lib.listed);
    }

    /// The extracted data is the pinned compiler's, as its provenance
    /// records; `tools/ts56-option-catalog.test.mjs` re-extracts it.
    #[test]
    fn the_catalog_is_typescript_5_6_3s() {
        assert_eq!(catalog_5_6::VERSION, "5.6.3");
        assert_eq!(
            catalog_5_6::SOURCE_SHA256,
            "f316520790d4db220a10d890c5f85310e26a1bd3c104b8d3b5eb62ba0491651b"
        );
        assert_eq!(catalog_5_6::OPTIONS.len(), 125);
        let rootdirs = declaration("rootDirs").unwrap();
        assert!(rootdirs.template);
        assert!(!rootdirs.file_path);
        assert!(matches!(
            rootdirs.kind,
            Type::List(Element {
                file_path: true,
                ..
            })
        ));
        assert!(declaration("paths").unwrap().template);
        assert!(declaration("moduleSuffixes").unwrap().preserve_falsy);
        assert!(declaration("listFilesOnly").unwrap().command_line_only);
        assert!(declaration("stableTypeOrdering").is_none());
        assert!(build_option("verbose"));
        assert!(build_option("incremental"));
        assert!(!build_option("Incremental") && !build_option("strict"));
    }
}
