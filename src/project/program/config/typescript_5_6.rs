//! TypeScript 5.6.3's configuration rules where they differ from 7.0.2's,
//! for the TypeScript 5.6 dialect (`Rules::TYPESCRIPT_5_6`): conversion of
//! each occurrence by 5.6.3's catalog (`validate_option`), the effective
//! values its `computedOptions` compute, the options 5.5 removed
//! (`verifyDeprecatedCompilerOptions`), the 5.6.3 counterparts of the
//! `verifyCompilerOptions` checks the TypeScript 7 path ports, and the
//! settings this tool refuses under the dialect (`effective`).

use super::catalog::{self, Type};
use super::{
    Conversion, Declared, Detection, Effective, File, Kind, Reader, Resolution, Rules,
    SemanticOptions, Target, Types,
};
use crate::analysis::{InputError, InputErrorKind};
use crate::parser::json::{JsonKind, JsonProperty, JsonString, JsonValue};

/// `convertJsonOption`'s checks on one occurrence of `compilerOptions`
/// property `name`, by 5.6.3's catalog. An unknown name is an error even
/// when its value is `null` (`parseOwnConfigOfJsonSourceFile`'s
/// `onPropertySet`; `createUnknownOptionError`): TS5093 for a `--build`
/// option, TS5025 with a spelling suggestion, otherwise TS5023. A
/// command-line option is TS6266, also when `null`. A value is converted by
/// its declaration (`convert`). A property with an empty name is neither
/// set nor reported.
pub(super) fn validate_option(
    name: &str,
    declared: Declared<'_>,
    key_start: usize,
) -> Result<(), InputError> {
    if name.is_empty() {
        return Ok(());
    }
    let key = declared.file.origin_at(key_start);
    let Some(option) = catalog::declaration(name) else {
        return Err(key.error(InputErrorKind::Config, unknown_option(name)));
    };
    if option.command_line_only {
        return Err(key.error(
            InputErrorKind::Config,
            format!("Option '{name}' can only be specified on command line"),
        ));
    }
    convert(&option.kind, name, declared)
}

/// `createUnknownOptionError`'s message for compiler option `name`: TS5093
/// when `name` lowered is a key of `compilerOptionsAlternateMode`'s name map,
/// any `--build` option's lowered name, so `Incremental` is TS5093 too; else
/// TS5025 with 5.6.3's `getSpellingSuggestion` over `optionDeclarations`
/// (`spelling::typescript_5_6_suggestion`); else TS5023.
fn unknown_option(name: &str) -> String {
    let lowered = Rules::TYPESCRIPT_5_6
        .enum_key(name)
        .expect("lowering always gives a key");
    if catalog::build_option(&lowered) {
        return format!("Compiler option '--{name}' may only be used with '--build'");
    }
    match crate::spelling::typescript_5_6_suggestion(name, catalog::names()) {
        Some(suggestion) => {
            format!("Unknown compiler option '{name}'. Did you mean '{suggestion}'?")
        }
        None => format!("Unknown compiler option '{name}'"),
    }
}

/// `convertJsonOption` of one value by a declaration of `kind` named
/// `name`: `null` converts to nothing; a value whose JavaScript type is
/// another is TS5024 (`isCompilerOptionsValue`, where an object is any
/// object, an array included); an enumeration value that lowers to none of
/// its keys is TS6046 (`convertJsonOptionOfCustomType`); and each entry of
/// a list is converted by its element declaration, whose name diagnostics
/// use (`convertJsonOptionOfListType`).
fn convert(kind: &Type, name: &str, declared: Declared<'_>) -> Result<(), InputError> {
    let fits = match (kind, &declared.value.kind) {
        (_, JsonKind::Null)
        | (Type::Boolean, JsonKind::Bool(_))
        | (Type::Number, JsonKind::Number)
        | (Type::String, JsonKind::String(_))
        | (Type::Object, JsonKind::Object(_) | JsonKind::Array(_)) => true,
        (Type::Enum(enumeration), JsonKind::String(text)) => {
            let known = match text {
                JsonString::Text(text) => Rules::TYPESCRIPT_5_6
                    .enum_key(text)
                    .is_some_and(|key| enumeration.value(&key).is_some()),
                // No key is an unpaired surrogate.
                JsonString::NonScalar(_) => false,
            };
            if !known {
                return Err(Rules::TYPESCRIPT_5_6.invalid_value(
                    declared,
                    name,
                    enumeration.listed,
                ));
            }
            true
        }
        (Type::List(element), JsonKind::Array(items)) => {
            for item in items {
                convert(
                    &element.kind,
                    element.name,
                    Declared {
                        value: item,
                        ..declared
                    },
                )?;
            }
            true
        }
        _ => false,
    };
    if fits {
        Ok(())
    } else {
        Err(declared.file.error(
            declared.value,
            format!(
                "Compiler option '{name}' requires a value of type {}",
                kind.name()
            ),
        ))
    }
}

/// How 5.6.3 converts option `name`'s string value or list entries
/// (`Rules::conversion`), as its declaration says: `isFilePath`, of the
/// option or of its list's element, makes each value a path
/// (`normalizeNonListOptionValue`); `${configDir}` applies to the options
/// `configDirTemplateSubstitutionOptions` selects, those that allow it
/// (`allowConfigDirTemplateSubstitution`) and the paths that are not
/// command-line only; and a list keeps its falsy entries where
/// `listPreserveFalsyValues` says so (`convertJsonOptionOfListType`). An
/// option 5.6.3 does not declare is never set, every occurrence of it being
/// an error (`validate_option`), so it converts as text.
pub(super) fn conversion(name: &str) -> Conversion {
    let Some(option) = catalog::declaration(name) else {
        return Conversion::TEXT;
    };
    Conversion {
        path: match &option.kind {
            Type::List(element) => element.file_path,
            _ => option.file_path,
        },
        template: option.template || (option.file_path && !option.command_line_only),
        preserve_falsy: option.preserve_falsy,
    }
}

/// 5.6.3's `ScriptTarget`, `ModuleKind`, and `ModuleResolutionKind` values
/// that its rules compare, as the catalog's enumerations map keys to them.
/// Its `ModuleDetectionKind` values are all truthy, so `detection` keeps
/// the keys.
mod value {
    pub(super) const ES3: u32 = 0;
    pub(super) const ES5: u32 = 1;
    pub(super) const ES2015: u32 = 2;
    pub(super) const ES2022: u32 = 9;
    pub(super) const ESNEXT: u32 = 99;

    pub(super) const MODULE_NONE: u32 = 0;
    pub(super) const COMMONJS: u32 = 1;
    pub(super) const AMD: u32 = 2;
    pub(super) const UMD: u32 = 3;
    pub(super) const SYSTEM: u32 = 4;
    pub(super) const MODULE_ES2015: u32 = 5;
    pub(super) const MODULE_ESNEXT: u32 = 99;
    pub(super) const NODE16: u32 = 100;
    pub(super) const NODENEXT: u32 = 199;
    pub(super) const PRESERVE: u32 = 200;

    pub(super) const CLASSIC: u32 = 1;
    pub(super) const NODE10: u32 = 2;
    pub(super) const RESOLUTION_NODE16: u32 = 3;
    pub(super) const RESOLUTION_NODENEXT: u32 = 99;
    pub(super) const BUNDLER: u32 = 100;
}

/// An enumeration option after inheritance: the value its key converts to,
/// the key, and the occurrence that set it.
type Numbered<'a> = Option<(u32, String, Declared<'a>)>;

fn numbered<'a>(read: &Reader<'a, '_>, name: &str) -> Result<Numbered<'a>, InputError> {
    Ok(read.choice(name)?.map(|(key, declared)| {
        let value = catalog::enumeration(name)
            .and_then(|enumeration| enumeration.value(&key))
            .and_then(|value| value.parse().ok())
            .expect("`choice` accepts only the option's keys, which map to numbers");
        (value, key, declared)
    }))
}

/// TypeScript 5.6.3's effective options for a configuration whose
/// occurrences `validate_option` already converted, in the order of its
/// checks: `verifyCompilerOptions`' TS5052, then
/// `verifyDeprecatedCompilerOptions`, then the `paths` checks
/// (`paths_setting`), TS5053 (`libraries`), TS5091, TS5095, and TS5109 or
/// TS5110; then the refusals of this tool's profile, which name the option
/// and whether TypeScript 5's default set it.
///
/// The values are `computedOptions`' (T5 21603–21820), in its order and
/// with its dependencies; `strict` itself defaults to false. Of its 25
/// entries, `resolvePackageJsonExports` and `resolvePackageJsonImports` are
/// not consumed (resolution reads the raw flags, as
/// `getNodeResolutionFeatures` does), nor are `declaration`, `incremental`,
/// and `declarationMap`; `allowJs` and `useDefineForClassFields` have
/// TypeScript 7's rules, which `SemanticOptions::from_reader` ports.
pub(super) fn effective<'a>(
    read: &Reader<'a, '_>,
    leaf: &File,
) -> Result<Effective<'a>, InputError> {
    use value::*;
    let raw_target = numbered(read, "target")?;
    let raw_module = numbered(read, "module")?;
    let raw_resolution = numbered(read, "moduleResolution")?;
    let raw_detection = read.choice("moduleDetection")?;
    let jsx = read.choice("jsx")?;
    let strict = read.bool("strict");
    // `getStrictOptionValue`: the flag if set, else `!!strict`.
    let strict_option = |name: &str| read.bool(name).unwrap_or(strict == Some(true));
    let flag = |name: &str| read.bool(name) == Some(true);
    let at = |name: &str, kind| read.get(name, kind).expect("present");

    // computedOptions.target: ES3 counts as unset.
    let target = match &raw_target {
        Some((target, ..)) if *target != ES3 => *target,
        _ => match raw_module.as_ref().map(|(module, ..)| *module) {
            Some(NODE16) => ES2022,
            Some(NODENEXT) => ESNEXT,
            _ => ES5,
        },
    };
    // computedOptions.module.
    let module = raw_module.as_ref().map_or(
        if target >= ES2015 {
            MODULE_ES2015
        } else {
            COMMONJS
        },
        |m| m.0,
    );
    // computedOptions.moduleResolution.
    let resolution = raw_resolution.as_ref().map_or(
        match module {
            COMMONJS => NODE10,
            NODE16 => RESOLUTION_NODE16,
            NODENEXT => RESOLUTION_NODENEXT,
            PRESERVE => BUNDLER,
            _ => CLASSIC,
        },
        |r| r.0,
    );
    // computedOptions.moduleDetection: every explicit value is truthy.
    let detection = match &raw_detection {
        Some((key, _)) => super::detection(key),
        None if matches!(module, NODE16 | NODENEXT) => Detection::Force,
        None => Detection::Auto,
    };
    // computedOptions.esModuleInterop and allowSyntheticDefaultImports.
    let interop = read
        .bool("esModuleInterop")
        .unwrap_or(matches!(module, NODE16 | NODENEXT | PRESERVE));
    let synthetic = read
        .bool("allowSyntheticDefaultImports")
        .unwrap_or(interop || module == SYSTEM || resolution == BUNDLER);
    // computedOptions.resolveJsonModule: implied for bundler resolution
    // alone, not for node16 or nodenext.
    let resolve_json_module = read
        .bool("resolveJsonModule")
        .unwrap_or(resolution == BUNDLER);
    let always_strict = strict_option("alwaysStrict");

    let mut semantic = SemanticOptions::from_reader(read, target_of(target), &strict_option)?;
    // computedOptions.isolatedModules is `from_reader`'s; preserveConstEnums.
    semantic.preserve_const_enums = flag("preserveConstEnums") || semantic.isolated_modules;

    // TS5052: the raw flag beside the computed strictNullChecks.
    if !semantic.strict_null_checks {
        for name in ["strictPropertyInitialization", "exactOptionalPropertyTypes"] {
            if flag(name) {
                let declared = at(name, Kind::Bool);
                return Err(declared.file.error(
                    declared.value,
                    format!(
                        "Option '{name}' cannot be specified without specifying option 'strictNullChecks'"
                    ),
                ));
            }
        }
    }
    removed_in_5_5(read, raw_target.as_ref())?;
    // `baseUrl` is a path option, and `paths` is relative to it when set.
    let base_url = read.directory("baseUrl")?;
    let paths = super::paths_setting(read, base_url.as_deref())?;
    // TS5053.
    let lib = super::libraries(read)?;
    // TS5091: either flag enables isolated compilation; the raw
    // `preserveConstEnums` is false.
    let verbatim = flag("verbatimModuleSyntax");
    if (flag("isolatedModules") || verbatim) && read.bool("preserveConstEnums") == Some(false) {
        let declared = at("preserveConstEnums", Kind::Bool);
        let enabled = if verbatim {
            "verbatimModuleSyntax"
        } else {
            "isolatedModules"
        };
        return Err(declared.file.error(
            declared.value,
            format!("Option 'preserveConstEnums' cannot be disabled when '{enabled}' is enabled"),
        ));
    }
    // TS5095: bundler resolution needs ESM output or `preserve`.
    if resolution == BUNDLER
        && !(MODULE_ES2015..=MODULE_ESNEXT).contains(&module)
        && module != PRESERVE
        && let Some((_, _, declared)) = &raw_resolution
    {
        return Err(declared.file.error(
            declared.value,
            "Option 'bundler' can only be used when 'module' is set to 'preserve' or to 'es2015' or later",
        ));
    }
    // TS5109 and TS5110, which name the kinds as `ModuleKind` and
    // `ModuleResolutionKind` spell them.
    let node_module = (NODE16..=NODENEXT).contains(&module);
    let node_resolution = (RESOLUTION_NODE16..=RESOLUTION_NODENEXT).contains(&resolution);
    if node_module && !node_resolution {
        let name = if module == NODE16 {
            "Node16"
        } else {
            "NodeNext"
        };
        return Err(located(
            leaf,
            raw_resolution.as_ref().map(|r| r.2),
            InputErrorKind::Config,
            format!(
                "Option 'moduleResolution' must be set to '{name}' (or left unspecified) when option 'module' is set to '{name}'"
            ),
        ));
    }
    if node_resolution && !node_module {
        let name = if resolution == RESOLUTION_NODE16 {
            "Node16"
        } else {
            "NodeNext"
        };
        return Err(located(
            leaf,
            raw_module.as_ref().map(|m| m.2),
            InputErrorKind::Config,
            format!(
                "Option 'module' must be set to '{name}' when option 'moduleResolution' is set to '{name}'"
            ),
        ));
    }

    // This tool's refusals under the dialect, each a setting whose
    // TypeScript 5 behavior is outside what this tool ports.
    let refused = |declared: Option<Declared>, message: String| {
        located(leaf, declared, InputErrorKind::Unsupported, message)
    };
    const PROFILE: &str = "is not supported under the TypeScript 5.6 profile";
    if target == ES5 {
        return Err(match &raw_target {
            Some((_, _, declared)) => refused(
                Some(*declared),
                format!("Option 'target=ES5' {PROFILE}; set 'target' to 'es2015' or later"),
            ),
            None => refused(
                None,
                format!(
                    "Option 'target' is 'es5' by TypeScript 5's default, which {PROFILE}; set 'target' to 'es2015' or later"
                ),
            ),
        });
    }
    if matches!(module, MODULE_NONE | AMD | UMD | SYSTEM) {
        let (_, key, declared) = raw_module.as_ref().expect("no default is one of these");
        return Err(refused(
            Some(*declared),
            format!(
                "Option 'module={key}' {PROFILE}; set 'module' to 'es2015' or later, 'node16', 'nodenext', or 'preserve'"
            ),
        ));
    }
    if matches!(resolution, CLASSIC | NODE10) {
        const REMEDY: &str = "set 'moduleResolution' to 'node16', 'nodenext', or 'bundler'";
        return Err(match &raw_resolution {
            Some((_, key, declared)) => refused(
                Some(*declared),
                format!("Option 'moduleResolution={key}' {PROFILE}; {REMEDY}"),
            ),
            None => {
                let default = if resolution == CLASSIC {
                    "classic"
                } else {
                    "node10"
                };
                refused(
                    raw_module.as_ref().map(|m| m.2),
                    format!(
                        "Option 'moduleResolution' is '{default}' by TypeScript 5's default for this 'module', which {PROFILE}; {REMEDY}"
                    ),
                )
            }
        });
    }
    for (name, effective) in [
        ("esModuleInterop", interop),
        ("allowSyntheticDefaultImports", synthetic),
    ] {
        if !effective {
            return Err(match read.get(name, Kind::Bool) {
                Some(declared) => refused(
                    Some(declared),
                    format!("Option '{name}=false' {PROFILE}; set '{name}' to true"),
                ),
                None => refused(
                    None,
                    format!(
                        "Option '{name}' is false by TypeScript 5's default for this 'module', which {PROFILE}; set '{name}' to true"
                    ),
                ),
            });
        }
    }
    if !always_strict {
        const REMEDY: &str = "set 'strict' or 'alwaysStrict' to true";
        return Err(match (read.get("alwaysStrict", Kind::Bool), strict) {
            (Some(declared), _) => refused(
                Some(declared),
                format!("Option 'alwaysStrict=false' {PROFILE}; {REMEDY}"),
            ),
            (None, Some(false)) => refused(
                Some(at("strict", Kind::Bool)),
                format!(
                    "Option 'alwaysStrict' is false because 'strict' is false, which {PROFILE}; {REMEDY}"
                ),
            ),
            (None, _) => refused(
                None,
                format!(
                    "Option 'alwaysStrict' is false by TypeScript 5's default when 'strict' is unset, which {PROFILE}; {REMEDY}"
                ),
            ),
        });
    }
    if let Some(declared) = read.string_setting("outFile") {
        return Err(refused(
            Some(declared),
            format!("Option 'outFile' {PROFILE}"),
        ));
    }
    if flag("noResolve") {
        return Err(refused(
            Some(at("noResolve", Kind::Bool)),
            "Option 'noResolve' changes module resolution in a way that is not supported".into(),
        ));
    }
    // An unset `types` includes every package in the type roots.
    let types = super::type_names(read)?.map_or(Types::Automatic, Types::Listed);
    let jsx_import_source = read.text("jsxImportSource")?;

    let (module_kind, module_resolution) = (module_kind_of(module), resolution_of(resolution));
    semantic.module_kind = module_kind;
    semantic.module_resolution = module_resolution;
    Ok(Effective {
        resolution: module_resolution,
        detection,
        jsx,
        target: target_of(target),
        lib,
        types,
        jsx_import_source,
        paths,
        base_url,
        semantic,
        resolve_json_module,
        // `noUncheckedSideEffectImports` is off unless set.
        check_side_effect_imports: flag("noUncheckedSideEffectImports"),
        // 5.6.3 has no `libReplacement`.
        lib_replacement: false,
    })
}

/// `verifyDeprecatedCompilerOptions`' `checkDeprecations("5.0", "5.5")`:
/// TypeScript 5.6 is past 5.5, so each option it lists is removed
/// (`mustBeRemoved`), TS5102 or, with a value, TS5108, which no
/// `ignoreDeprecations` silences; `getIgnoreDeprecationsVersion` first
/// reports a set value other than `5.0` as TS5103. Each option is checked
/// as JavaScript truthiness reads it: `importsNotUsedAsValues: "remove"`
/// converts to 0, so it passes.
fn removed_in_5_5(
    read: &Reader<'_, '_>,
    target: Option<&(u32, String, Declared)>,
) -> Result<(), InputError> {
    let removed = |declared: Declared, name: String, instead: Option<&str>| {
        let mut message =
            format!("Option '{name}' has been removed. Please remove it from your configuration");
        if let Some(instead) = instead {
            message.push_str(&format!(". Use '{instead}' instead"));
        }
        declared.file.error(declared.value, message)
    };
    if let Some((text, declared)) = read.string("ignoreDeprecations")?
        && !text.is_empty()
        && text != "5.0"
    {
        return Err(declared
            .file
            .error(declared.value, "Invalid value for '--ignoreDeprecations'"));
    }
    if let Some((value::ES3, _, declared)) = target {
        return Err(removed(*declared, "target=ES3".into(), None));
    }
    for (name, instead) in [
        ("noImplicitUseStrict", None),
        ("keyofStringsOnly", None),
        ("suppressExcessPropertyErrors", None),
        ("suppressImplicitAnyIndexErrors", None),
        ("noStrictGenericChecks", None),
        ("charset", None),
        ("out", Some("outFile")),
        ("importsNotUsedAsValues", Some("verbatimModuleSyntax")),
        ("preserveValueImports", Some("verbatimModuleSyntax")),
    ] {
        let set = match name {
            "charset" | "out" => read
                .string(name)?
                .filter(|(text, _)| !text.is_empty())
                .map(|(_, declared)| declared),
            "importsNotUsedAsValues" => numbered(read, name)?
                .filter(|(value, ..)| *value != 0)
                .map(|(.., declared)| declared),
            _ => read
                .get(name, Kind::Bool)
                .filter(|declared| declared.value.as_bool() == Some(true)),
        };
        if let Some(declared) = set {
            return Err(removed(declared, name.into(), instead));
        }
    }
    Ok(())
}

/// An error at `declared`, or, for a value TypeScript 5's default implies,
/// at the configuration that leaves it unset.
fn located(
    leaf: &File,
    declared: Option<Declared>,
    kind: InputErrorKind,
    message: String,
) -> InputError {
    match declared {
        Some(declared) => declared.file.origin(declared.value).error(kind, message),
        None => InputError {
            kind,
            message,
            path: Some(leaf.path.clone().into()),
            position: None,
        },
    }
}

/// The `ScriptTarget` 5.6.3's value names, which this tool's `Target`
/// represents for both dialects.
fn target_of(value: u32) -> Target {
    match value {
        value::ES5 => Target::Es5,
        value::ES2015 => Target::Es2015,
        3 => Target::Es2016,
        4 => Target::Es2017,
        5 => Target::Es2018,
        6 => Target::Es2019,
        7 => Target::Es2020,
        8 => Target::Es2021,
        value::ES2022 => Target::Es2022,
        10 => Target::Es2023,
        value::ESNEXT => Target::EsNext,
        _ => unreachable!("no other `ScriptTarget` reaches here"),
    }
}

fn module_kind_of(value: u32) -> super::super::ModuleKind {
    use super::super::ModuleKind;
    match value {
        value::COMMONJS => ModuleKind::CommonJs,
        value::MODULE_ES2015 => ModuleKind::Es2015,
        6 => ModuleKind::Es2020,
        7 => ModuleKind::Es2022,
        value::MODULE_ESNEXT => ModuleKind::EsNext,
        value::NODE16 => ModuleKind::Node16,
        value::NODENEXT => ModuleKind::NodeNext,
        value::PRESERVE => ModuleKind::Preserve,
        _ => unreachable!("refused before"),
    }
}

fn resolution_of(value: u32) -> Resolution {
    match value {
        value::RESOLUTION_NODE16 => Resolution::Node16,
        value::RESOLUTION_NODENEXT => Resolution::NodeNext,
        value::BUNDLER => Resolution::Bundler,
        _ => unreachable!("refused before"),
    }
}

/// `paths` given as an array, which TypeScript 5.6.3 accepts as an object
/// (`typeof` is `object`) and whose indices it would read as patterns
/// (`for...in`): an empty one maps nothing, as `{}` does; a JavaScript
/// artifact this tool does not reproduce otherwise.
pub(super) fn paths_array(declared: Declared) -> Result<(), InputError> {
    match declared.value.as_array() {
        Some([]) => Ok(()),
        _ => Err(declared.file.unsupported(
            declared.value,
            "TypeScript 5 would read the indices of this 'paths' array as its patterns, which is not supported",
        )),
    }
}

/// TS5064 for a `paths` substitution that is not a string, as 5.6.3 words
/// it: the value as JavaScript's `String` converts it, and its `typeof`.
pub(super) fn substitution_type(file: &File, item: &JsonValue, pattern: &str) -> InputError {
    let kind = match item.kind {
        JsonKind::Bool(_) => "boolean",
        JsonKind::Number => "number",
        _ => "object",
    };
    file.error(
        item,
        format!(
            "Substitution '{}' for pattern '{pattern}' has incorrect type, expected 'string', got '{kind}'",
            js_string(file, item)
        ),
    )
}

/// A JSON value as JavaScript's `String` converts it, as 5.6.3 formats a
/// diagnostic argument: an array joins its elements, `null` among them
/// empty, and any other object is `[object Object]`.
fn js_string(file: &File, value: &JsonValue) -> String {
    match &value.kind {
        JsonKind::Null => "null".into(),
        JsonKind::Bool(value) => value.to_string(),
        JsonKind::Number => {
            let text = &file.text.as_str()[value.span.start..value.span.end];
            crate::numbers::number_to_string(crate::numbers::number_from_string(text))
        }
        JsonKind::String(text) => text.as_str().unwrap_or_default().to_owned(),
        JsonKind::Array(items) => items
            .iter()
            .map(|item| match item.kind {
                JsonKind::Null => String::new(),
                _ => js_string(file, item),
            })
            .collect::<Vec<_>>()
            .join(","),
        JsonKind::Object(_) => "[object Object]".into(),
    }
}

/// A `paths` pattern `__proto__`, whose assignment in 5.6.3's
/// `convertObjectLiteralExpressionToJson` sets the mapping's prototype
/// rather than a pattern: a JavaScript artifact that would change which
/// names map, refused rather than reproduced.
pub(super) fn prototype_pattern(file: &File, property: &JsonProperty) -> InputError {
    file.origin_at(property.key_span.start).error(
        InputErrorKind::Unsupported,
        "TypeScript 5 would set the prototype of 'paths' with the pattern '__proto__' rather than map it, which is not supported",
    )
}

/// A root-level `__proto__` whose value is an object, which 5.6.3's
/// `convertObjectLiteralExpressionToJson` makes the raw configuration's
/// prototype: `applyExtendedConfig`'s truthiness tests then see its
/// `include`, `files`, or `exclude`, in a leaf blocking a base's list and
/// in a base supplying one, while `getPropFromRaw` does not. A JavaScript
/// artifact that changes the selected files, refused rather than
/// reproduced. Any other value sets no prototype holding such a list.
pub(super) fn prototype_root(file: &File) -> Result<(), InputError> {
    let prototype = file
        .root
        .properties()
        .unwrap_or_default()
        .iter()
        .find(|property| {
            property.key.as_str() == Some("__proto__")
                && matches!(property.value.kind, JsonKind::Object(_))
        });
    match prototype {
        Some(property) => Err(file.origin_at(property.key_span.start).error(
            InputErrorKind::Unsupported,
            "TypeScript 5 would set the configuration's prototype with '__proto__' rather than read the key, which can change the files it selects and is not supported",
        )),
        None => Ok(()),
    }
}

/// A `null` entry of a list whose declaration keeps falsy values
/// (`listPreserveFalsyValues`, `moduleSuffixes` alone), which 5.6.3 keeps
/// as `undefined` and its file names spell `undefined`: refused rather
/// than reproduced.
pub(super) fn undefined_entry(file: &File, entry: &JsonValue, name: &str) -> InputError {
    file.unsupported(
        entry,
        format!(
            "TypeScript 5 keeps a null '{name}' entry and spells it 'undefined' in file names, which is not supported"
        ),
    )
}
