//! The effective configuration of one `tsconfig.json`: ordered `extends`
//! inheritance, option validation, and path-valued settings resolved against
//! the configuration file that declared them.
//!
//! Files are read with the shared TypeScript JSON reader
//! (`crate::parser::json`); this module owns TypeScript's conversion of that
//! text to configuration, in three steps that ask different questions, each
//! by the rules of the dialect the installed compiler selects (`Rules`):
//!
//! - Raw conversion (`convertConfigFileToObject`, `convertToJson`, and
//!   `convertObjectLiteralExpressionToJson`): the root must be an object, and
//!   a repeated key keeps its first position and its last value, `null`
//!   included (`entries`, `get`). TypeScript 7.0.2 leaves out a property
//!   with an empty name and the `null` entries of an array, an array of only
//!   `null`s being no list (`nil_list`); 5.6.3 keeps both.
//! - A file's own option settings (`parseOwnConfigOfJsonSourceFile`'s
//!   `onPropertySet`): every occurrence of a recognized option, across every
//!   `compilerOptions` object, is validated. TypeScript 5.6.3 assigns each
//!   occurrence's converted value, so the last decides, a `null` setting the
//!   option to nothing. TypeScript 7.0.2 sets it from the last occurrence
//!   whose converted value is not nil: a `null` or an empty enumeration
//!   string leaves an earlier setting, a list's `null` clears it
//!   (`Rules::own_setting`, `Kind::sets`).
//! - Inheritance (`parseConfig`'s `applyExtendedConfig` and
//!   `mergeCompilerOptions`): `files`, `include`, and `exclude` come from a
//!   file's raw object when it has the key (7.0.2, `null` included) or a
//!   value (5.6.3), and otherwise from the last base whose value is a list
//!   (`Chain::root_list`). A file with `extends` merges each base's merged
//!   options in order, then its own (`Chain::option`, `Rules::over`): under
//!   5.6.3 every setting replaces the one before, nothing included; under
//!   7.0.2 a set value that is not the zero value does (`Kind::zero`), and
//!   the merged file's explicit `null`, the last value in its last
//!   `compilerOptions` object, unsets the option.

use super::compatibility::{self, CompilerSelection};
use super::libraries::{self, Target};
use super::paths;
use super::resolve;
use super::{Origin, disk, io_error};
use crate::analysis::Parser;
use crate::analysis::{InputError, InputErrorKind};
use crate::parser::json::{JsonErrorKind, JsonKind, JsonProperty, JsonString, JsonValue};
use crate::source::SourceText;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

mod catalog;
mod catalog_5_6;
#[cfg(test)]
mod conversion_tests;
mod typescript_5_6;

#[derive(Debug)]
pub(super) struct Config {
    /// Absolute path of the selected configuration, not canonicalized.
    pub(super) path: PathBuf,
    /// Canonical directory of the selected configuration.
    pub(super) dir: String,
    /// The selected configuration in its canonical directory.
    pub(super) file: String,
    pub(super) files: Option<Specs>,
    pub(super) include: Option<Specs>,
    pub(super) exclude: Option<Specs>,
    /// An empty selection is TypeScript's "No inputs were found" error only
    /// when neither `files` nor `references` is specified.
    pub(super) report_no_inputs: bool,
    pub(super) options: Options,
}

/// The entries of `files`, `include`, or `exclude`, made absolute.
#[derive(Debug, Default)]
pub(super) struct Specs {
    pub(super) entries: Vec<Spec>,
    /// Each entry as the list was written, as JSON, `null` included, which
    /// TS18003 lists.
    pub(super) written: Vec<String>,
}

#[derive(Debug)]
pub(super) struct Spec {
    /// As written, for messages.
    pub(super) text: String,
    /// Absolute and normalized.
    pub(super) path: String,
    pub(super) origin: Origin,
}

/// The configuration rules of one dialect (`compatibility::Dialect`): the
/// option catalog and its messages, enumeration folding, removed and
/// refused options, computed defaults, and the verification checks. Only
/// this module's operations interpret it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Rules(Profile);

/// Whose configuration rules: TypeScript 7.0.2's, which the rest of this
/// module ports, or TypeScript 5.6.3's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Profile {
    TypeScript7,
    TypeScript56,
}

impl Rules {
    pub(super) const TYPESCRIPT_7: Self = Self(Profile::TypeScript7);
    pub(super) const TYPESCRIPT_5_6: Self = Self(Profile::TypeScript56);

    /// The compiler release whose rules these are.
    pub(super) fn version(self) -> &'static str {
        match self.0 {
            Profile::TypeScript7 => super::version::REFERENCE,
            Profile::TypeScript56 => catalog_5_6::VERSION,
        }
    }

    /// The keys of enumeration option `name` in this dialect's catalog,
    /// every one `convertJsonOptionOfEnumType` looks up, and those its TS6046
    /// message lists. TypeScript 7.0.2's maps are `tsoptions`' enum maps
    /// less the keys this tool leaves out of every message.
    fn enumeration(self, name: &str) -> (Vec<&'static str>, String) {
        match self.0 {
            Profile::TypeScript7 => match name {
                "newLine" => (vec!["crlf", "lf"], "'crlf', 'lf'".into()),
                "module" => (
                    vec![
                        "commonjs", "amd", "system", "umd", "es6", "es2015", "es2020", "es2022",
                        "esnext", "node16", "node18", "node20", "nodenext", "preserve",
                    ],
                    "'commonjs', 'es6', 'es2015', 'es2020', 'es2022', 'esnext', 'node16', 'node18', 'node20', 'nodenext', 'preserve'".into(),
                ),
                "moduleResolution" => (
                    vec!["node16", "nodenext", "bundler", "classic", "node", "node10"],
                    "'node16', 'nodenext', 'bundler'".into(),
                ),
                "moduleDetection" => {
                    const DETECTIONS: [&str; 3] = ["auto", "legacy", "force"];
                    (DETECTIONS.to_vec(), quoted(&DETECTIONS))
                }
                "jsx" => {
                    const JSX: [&str; 5] =
                        ["preserve", "react-native", "react-jsx", "react-jsxdev", "react"];
                    (JSX.to_vec(), quoted(&JSX))
                }
                "target" => (Target::keys(), Target::option_names()),
                _ => unreachable!("`{name}` is no TypeScript 7 enumeration this tool reads"),
            },
            Profile::TypeScript56 => {
                let enumeration =
                    catalog::enumeration(name).expect("an enumeration of the 5.6.3 catalog");
                (
                    enumeration.entries.iter().map(|&(key, _)| key).collect(),
                    enumeration.listed.into(),
                )
            }
        }
    }

    /// The key `convertJsonOptionOfEnumType` looks up for an enumeration
    /// value or list entry, or none when it sets nothing. TypeScript 7.0.2
    /// lowers it with Go's `strings.ToLower` and leaves an empty string
    /// unset (`simple_enum_key`); TypeScript 5.6.3 lowers it with
    /// JavaScript's `toLowerCase` (`strings::to_lower_js`), which maps
    /// U+0130 to `i` and a combining dot, and looks up an empty string too,
    /// which is no key.
    fn enum_key(self, text: &str) -> Option<String> {
        match self.0 {
            Profile::TypeScript7 => simple_enum_key(text),
            Profile::TypeScript56 => Some(
                String::from_utf8(crate::strings::to_lower_js(text.as_bytes()))
                    .expect("lowering text keeps it text"),
            ),
        }
    }

    /// TS6046 for option `name`'s value, naming the option as the dialect's
    /// wording does (`Wording::option_prefix`).
    fn invalid_value(self, declared: Declared, name: &str, listed: &str) -> InputError {
        declared.file.error(
            declared.value,
            format!(
                "Argument for '{}{name}' option must be: {listed}",
                self.wording().option_prefix
            ),
        )
    }

    /// The messages of the checks both dialects make that each words its
    /// own way: TypeScript 5.6.3's are its own, and the TypeScript 7
    /// profile keeps the wording this tool has always used.
    fn wording(self) -> &'static Wording {
        match self.0 {
            Profile::TypeScript7 => &Wording {
                extends_type: "'extends' must be a string or an array of strings",
                extends_entry: "'extends' entries must be strings",
                extends_empty: "'extends' cannot be an empty string",
                compiler_options_type: "'compilerOptions' must be an object",
                non_relative_substitution: "Non-relative paths are not allowed. Did you forget a leading './'?",
                option_prefix: "",
            },
            Profile::TypeScript56 => &Wording {
                extends_type: "Compiler option 'extends' requires a value of type string or Array",
                extends_entry: "Compiler option 'extends' requires a value of type string",
                extends_empty: "Compiler option 'extends' cannot be given an empty string",
                compiler_options_type: "Compiler option 'compilerOptions' requires a value of type object",
                non_relative_substitution: "Non-relative paths are not allowed when 'baseUrl' is not set. Did you forget a leading './'?",
                option_prefix: "--",
            },
        }
    }
}

/// One dialect's messages for the checks `Rules::wording` lists.
struct Wording {
    /// TS5024 for an `extends` that is neither a string nor a list.
    extends_type: &'static str,
    /// TS5024 for an `extends` entry that is not a string.
    extends_entry: &'static str,
    /// TS18051 for an empty `extends` string.
    extends_empty: &'static str,
    /// TS5024 for a `compilerOptions` the dialect does not read as an
    /// object.
    compiler_options_type: &'static str,
    /// TS5090 for a non-relative `paths` substitution without `baseUrl`.
    non_relative_substitution: &'static str,
    /// What TS6046 writes before an option's name: TypeScript 5.6.3 names
    /// its command-line flag.
    option_prefix: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Module {
    CommonJs,
    /// `es2015` through `esnext`.
    Es,
    /// `node16`, `node18`, `node20`, or `nodenext`.
    Node,
    Preserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Resolution {
    Node16,
    NodeNext,
    Bundler,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Detection {
    Auto,
    Legacy,
    Force,
}

/// The settings that affect program inputs, module edges, or file binding.
/// Directory-valued settings (`rootDirs`, `typeRoots`, `outDir`,
/// `declarationDir`, `rootDir`) are absolute and normalized, without a
/// trailing separator except at a root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Options {
    /// The installed compiler and the dialect whose rules made these
    /// options (`compatibility::select`).
    pub(super) compiler: CompilerSelection,
    pub(crate) semantic: SemanticOptions,
    pub(super) resolution: Resolution,
    pub(super) detection: Detection,
    /// `jsx` is set, to any mode: a `.tsx` file may then be a program input
    /// that a module request reaches (`GetResolutionDiagnostic`).
    pub(super) jsx: bool,
    pub(super) jsx_preserve: bool,
    /// `jsx: react-jsx` or `react-jsxdev`.
    pub(super) jsx_automatic: bool,
    /// `jsx: react-jsxdev`, whose runtime module is `jsx-dev-runtime`.
    pub(super) jsx_dev: bool,
    /// `jsxImportSource`, the package providing an automatic JSX runtime.
    pub(super) jsx_import_source: Option<String>,
    pub(super) paths: Option<Paths>,
    /// `baseUrl`, absolute and normalized, which only the TypeScript 5.6
    /// dialect accepts: resolution looks a bare name no `paths` pattern
    /// matches up under it, and it is the `paths` base (`Paths::base`).
    pub(super) base_url: Option<String>,
    pub(super) root_dirs: Vec<String>,
    /// Explicit `typeRoots`; `None` means every ancestor `node_modules/@types`.
    pub(super) type_roots: Option<Vec<String>>,
    pub(super) types: Types,
    pub(super) custom_conditions: Vec<String>,
    pub(super) module_suffixes: Vec<String>,
    pub(super) preserve_symlinks: bool,
    /// `noUncheckedSideEffectImports`: an unresolved side-effect import is an
    /// error unless this is false.
    pub(super) check_side_effect_imports: bool,
    /// `skipLibCheck`: TypeScript reports no errors in declaration files.
    pub(super) skip_lib_check: bool,
    /// `noCheck`: TypeScript reports no binding or checker errors at all.
    pub(super) no_check: bool,
    pub(super) allow_js: bool,
    /// `maxNodeModuleJsDepth`, 0 when unset: how many package lookups deep a
    /// JavaScript file in `node_modules` may be and still be a program
    /// input (`filesParser.start`). Both compilers compare it with a whole
    /// depth, 5.6.3 as written and 7.0.2 truncated to an integer; the
    /// greatest whole number not above it orders every whole depth as both
    /// do.
    pub(super) max_node_module_js_depth: i64,
    /// `allowArbitraryExtensions`: a module request from a file that is not a
    /// declaration file may reach a declaration file for an arbitrary
    /// extension, such as `styles.d.css.ts` (`GetResolutionDiagnostic`).
    pub(super) allow_arbitrary_extensions: bool,
    pub(super) allow_importing_ts_extensions: bool,
    pub(super) rewrite_relative_import_extensions: bool,
    pub(super) resolve_json_module: bool,
    pub(super) package_exports: bool,
    pub(super) package_imports: bool,
    /// `target`, which selects the default standard library.
    pub(super) target: Target,
    /// The files `lib` names, deduplicated by file and in written order;
    /// `None` when unset, so the target's default applies, and empty for an
    /// empty list, which selects no library.
    pub(super) lib: Option<Vec<&'static str>>,
    /// `noLib`: no standard library, and every library reference ignored.
    pub(super) no_lib: bool,
    /// `libReplacement`: `@typescript/lib-*` packages may replace libraries.
    pub(super) lib_replacement: bool,
    /// `skipDefaultLibCheck`: TypeScript reports no errors in libraries.
    pub(super) skip_default_lib_check: bool,
    pub(super) out_dir: Option<String>,
    pub(super) declaration_dir: Option<String>,
    pub(super) root_dir: Option<String>,
    pub(super) composite: bool,
    /// TypeScript's `shouldPreserveConstEnums`: `preserveConstEnums`,
    /// `isolatedModules`, or `verbatimModuleSyntax`. A namespace of only
    /// const enums then counts as instantiated in merge-order checks.
    pub(super) preserve_const_enums: bool,
}

/// Effective checker inputs from TypeScript 7.0.2's
/// `CompilerOptions.GetStrictOptionValue` and `OptionsDeclarations`.
/// Inheritance and explicit per-option settings have already been applied;
/// consumers never interpret raw `strict` or reopen configuration files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticOptions {
    target: Target,
    pub(crate) module_kind: super::ModuleKind,
    pub(crate) module_resolution: Resolution,
    pub(crate) no_check: bool,
    pub(crate) no_implicit_any: bool,
    pub(crate) no_implicit_this: bool,
    pub(crate) no_implicit_override: bool,
    pub(crate) strict_null_checks: bool,
    pub(crate) strict_function_types: bool,
    pub(crate) strict_bind_call_apply: bool,
    pub(crate) strict_property_initialization: bool,
    pub(crate) strict_builtin_iterator_return: bool,
    pub(crate) use_unknown_in_catch_variables: bool,
    pub(crate) exact_optional_property_types: bool,
    pub(crate) no_unchecked_indexed_access: bool,
    pub(crate) no_property_access_from_index_signature: bool,
    /// `typeToStringEx` consumes this even when only diagnostic formatting
    /// causes a lazy semantic dependency. It is not a strict-family option.
    pub(crate) no_error_truncation: bool,
    pub(crate) stable_type_ordering: bool,
    pub(crate) deduplicate_packages: bool,
    pub(crate) allow_js: bool,
    /// `checkJs` as set: unset is not `false`, since TypeScript checks a
    /// plain JavaScript file (`isPlainJsFile`) only when it is unset.
    pub(crate) check_js: Option<bool>,
    pub(crate) experimental_decorators: bool,
    pub(crate) emit_decorator_metadata: bool,
    pub(crate) use_define_for_class_fields: bool,
    pub(crate) emit_standard_class_fields: bool,
    /// Native `needsImportHelpersImportSpecifier` input for program capture.
    pub(crate) import_helpers: bool,
    /// `CompilerOptions.GetIsolatedModules`: effective isolated compilation.
    pub(crate) isolated_modules: bool,
    /// Raw `VerbatimModuleSyntax`: alias-reference collection uses this alone.
    pub(crate) verbatim_module_syntax: bool,
    pub(crate) preserve_const_enums: bool,
    explicit_dom_library: bool,
    pub(crate) jsx_factory: Option<String>,
    pub(crate) jsx_fragment_factory: Option<String>,
    pub(crate) react_namespace: Option<String>,
}

impl SemanticOptions {
    /// Ask the retained effective ScriptTarget; no consumer reinterprets the
    /// configured target string or independently supplies its default.
    pub(crate) fn target_supports_iterables(&self) -> bool {
        self.target.supports_iterables()
    }
    pub(crate) fn target_supports_object_rest(&self) -> bool {
        self.target.supports_object_rest()
    }
    /// Native private-access helper gate combines the two feature targets
    /// with define semantics; ESNext is the later ClassElementDecorators gate.
    pub(crate) fn private_access_needs_emit_helpers(&self) -> bool {
        !self.target.supports_class_element_decorators() || !self.use_define_for_class_fields
    }
    /// `getFirstTransformableStaticClassElement` compares both native feature targets.
    pub(crate) fn class_static_elements_need_transform(&self) -> bool {
        !self.target.is_at_least_es2022() || !self.target.supports_class_element_decorators()
    }
    /// `checkDecorators` compares the decorator feature target independently
    /// of the static-field and useDefineForClassFields transformation gates.
    pub(crate) fn decorators_need_emit_helpers(&self) -> bool {
        !self.target.supports_class_element_decorators()
    }
    /// `containerSeemsToBeEmptyDomElement` asks the configured Lib names,
    /// independently of the loaded/default library set.
    pub(crate) fn explicit_dom_library(&self) -> bool {
        self.explicit_dom_library
    }
    /// Defaults from `OptionsDeclarations` and `GetStrictOptionValue`.
    /// `alwaysStrict` is a removed false mode, not a strict-family flag.
    const DEFAULT: Self = Self {
        target: Target::LATEST_STANDARD,
        module_kind: super::ModuleKind::EsNext,
        module_resolution: Resolution::Bundler,
        no_check: false,
        no_implicit_any: true,
        no_implicit_this: true,
        no_implicit_override: false,
        strict_null_checks: true,
        strict_function_types: true,
        strict_bind_call_apply: true,
        strict_property_initialization: true,
        strict_builtin_iterator_return: true,
        use_unknown_in_catch_variables: true,
        exact_optional_property_types: false,
        no_unchecked_indexed_access: false,
        no_property_access_from_index_signature: false,
        no_error_truncation: false,
        stable_type_ordering: true,
        deduplicate_packages: true,
        allow_js: false,
        check_js: None,
        experimental_decorators: false,
        emit_decorator_metadata: false,
        use_define_for_class_fields: true,
        emit_standard_class_fields: true,
        import_helpers: false,
        isolated_modules: false,
        verbatim_module_syntax: false,
        preserve_const_enums: false,
        explicit_dom_library: false,
        jsx_factory: None,
        jsx_fragment_factory: None,
        react_namespace: None,
    };

    /// Capture checker inputs after configuration inheritance and target
    /// selection, with the dialect's rule for each strict-family flag
    /// (`strict_option`). Ports `GetUseDefineForClassFields` and the
    /// separate `GetEmitStandardClassFields` operation, which TypeScript
    /// 5.6.3's `computedOptions.useDefineForClassFields` and
    /// `getEmitStandardClassFields` match.
    fn from_reader(
        read: &Reader<'_, '_>,
        target: Target,
        strict_option: &dyn Fn(&str) -> bool,
    ) -> Result<Self, InputError> {
        let use_define = read.bool("useDefineForClassFields");

        Ok(Self {
            module_kind: super::ModuleKind::EsNext,
            module_resolution: Resolution::Bundler,
            target,
            no_check: read.bool("noCheck").unwrap_or(false),
            no_implicit_any: strict_option("noImplicitAny"),
            no_implicit_this: strict_option("noImplicitThis"),
            no_implicit_override: read.bool("noImplicitOverride").unwrap_or(false),
            strict_null_checks: strict_option("strictNullChecks"),
            strict_function_types: strict_option("strictFunctionTypes"),
            strict_bind_call_apply: strict_option("strictBindCallApply"),
            strict_property_initialization: strict_option("strictPropertyInitialization"),
            strict_builtin_iterator_return: strict_option("strictBuiltinIteratorReturn"),
            use_unknown_in_catch_variables: strict_option("useUnknownInCatchVariables"),
            exact_optional_property_types: read.bool("exactOptionalPropertyTypes").unwrap_or(false),
            no_unchecked_indexed_access: read.bool("noUncheckedIndexedAccess").unwrap_or(false),
            no_property_access_from_index_signature: read
                .bool("noPropertyAccessFromIndexSignature")
                .unwrap_or(false),
            no_error_truncation: read.bool("noErrorTruncation").unwrap_or(false),
            stable_type_ordering: read.bool("stableTypeOrdering").unwrap_or(true),
            deduplicate_packages: read.bool("deduplicatePackages").unwrap_or(true),
            allow_js: read.allow_js(),
            check_js: read.bool("checkJs"),
            experimental_decorators: read.bool("experimentalDecorators").unwrap_or(false),
            emit_decorator_metadata: read.bool("emitDecoratorMetadata").unwrap_or(false),
            // `GetUseDefineForClassFields` is distinct from emission:
            // explicit true can select define semantics below ES2022.
            use_define_for_class_fields: use_define.unwrap_or(target.is_at_least_es2022()),
            // `GetEmitStandardClassFields`: native fields additionally need
            // the target to support them, even for an explicit true setting.
            emit_standard_class_fields: use_define != Some(false) && target.is_at_least_es2022(),
            import_helpers: read.bool("importHelpers").unwrap_or(false),
            verbatim_module_syntax: read.bool("verbatimModuleSyntax").unwrap_or(false),
            preserve_const_enums: false,
            explicit_dom_library: false,
            isolated_modules: read.bool("isolatedModules").unwrap_or(false)
                || read.bool("verbatimModuleSyntax").unwrap_or(false),
            jsx_factory: read.text("jsxFactory")?,
            jsx_fragment_factory: read.text("jsxFragmentFactory")?,
            react_namespace: read.text("reactNamespace")?,
        })
    }
}

/// The type packages a program includes, as each dialect reads `types`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Types {
    /// Every package in the type roots (`getAutomaticTypeDirectiveNames`):
    /// TypeScript 5.6.3's unset `types`.
    Automatic,
    /// The listed packages, in order; TypeScript 7.0.2's unset `types` is an
    /// empty list.
    Listed(Vec<(TypeName, Origin)>),
}

/// One `types` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TypeName {
    /// A package's name, which under TypeScript 5.6.3 `*` is too.
    Package(String),
    /// TypeScript 7.0.2's `*`: every package in the type roots, in its
    /// place.
    Wildcard,
}

/// `paths` patterns and their substitutions, relative to `base`: `baseUrl`
/// when it is set, else the directory of the configuration that declared
/// them (`getPathsBasePath`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Paths {
    pub(super) base: String,
    pub(super) patterns: Vec<(String, Vec<String>)>,
}

impl Options {
    /// The settings TypeScript uses to resolve package-name `extends`, not
    /// the compiler defaults, which `convert` applies, for the selected
    /// `compiler`, whose version a package's `typesVersions` may consult.
    pub(super) fn for_extends_lookup(compiler: CompilerSelection) -> Self {
        Self {
            compiler,
            semantic: SemanticOptions {
                module_kind: super::ModuleKind::NodeNext,
                ..SemanticOptions::DEFAULT
            },
            resolution: Resolution::NodeNext,
            detection: Detection::Force,
            jsx: false,
            jsx_preserve: false,
            jsx_automatic: false,
            jsx_dev: false,
            jsx_import_source: None,
            paths: None,
            base_url: None,
            root_dirs: Vec::new(),
            type_roots: None,
            types: Types::Listed(Vec::new()),
            custom_conditions: Vec::new(),
            module_suffixes: Vec::new(),
            preserve_symlinks: false,
            check_side_effect_imports: true,
            skip_lib_check: false,
            no_check: false,
            allow_js: false,
            max_node_module_js_depth: 0,
            allow_arbitrary_extensions: false,
            allow_importing_ts_extensions: false,
            rewrite_relative_import_extensions: false,
            resolve_json_module: true,
            package_exports: true,
            package_imports: true,
            target: Target::LATEST_STANDARD,
            lib: None,
            no_lib: false,
            lib_replacement: false,
            skip_default_lib_check: false,
            out_dir: None,
            declaration_dir: None,
            root_dir: None,
            composite: false,
            preserve_const_enums: false,
        }
    }
}

/// The canonical directory of the configuration file at `path`, against
/// which its paths resolve and above which its compiler is looked for.
pub(super) fn directory(path: &Path, disk: &disk::Disk) -> Result<String, InputError> {
    let parent = path.parent().unwrap_or(Path::new("/"));
    let dir = disk
        .canonicalize(parent)
        .map_err(|error| io_error(parent, &error))?;
    paths::from_path(&dir)
}

/// One parsed configuration file.
#[derive(Debug)]
struct File {
    /// Absolute, as reached; its directory is canonical.
    path: String,
    dir: String,
    /// Fully canonical, for inheritance-cycle detection.
    identity: PathBuf,
    text: SourceText,
    /// The configuration object, after `config_root`.
    root: JsonValue,
}

impl File {
    fn read(path: &Path, disk: &disk::Disk, parser: &Parser) -> Result<Rc<File>, InputError> {
        let text = disk
            .read_to_string(path)
            .map_err(|error| io_error(path, &error))?;
        let identity = disk
            .canonicalize(path)
            .map_err(|error| io_error(path, &error))?;
        let dir = directory(path, disk)?;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        let text = SourceText::shared(text);
        let parsed = parser
            .parse_json(text.as_str())
            .map_err(|error| InputError {
                kind: match error.kind {
                    JsonErrorKind::Syntax => InputErrorKind::Config,
                    JsonErrorKind::TooDeep => InputErrorKind::Unsupported,
                },
                message: error.message,
                path: Some(path.to_path_buf()),
                position: Some(text.position(error.offset)),
            })?;
        let mut file = File {
            path: paths::combine(&dir, &name),
            dir,
            identity,
            text,
            root: JsonValue {
                native_span: None,
                span: crate::Span { start: 0, end: 0 },
                kind: JsonKind::Object(Vec::new()),
            },
        };
        file.root = file.config_root(parsed.root)?;
        file.check_root()?;
        Ok(Rc::new(file))
    }

    /// TypeScript's `convertConfigFileToObject`: empty or trivia-only text is
    /// an empty configuration, and a root that is not an object is TS5092,
    /// unless it is an array holding an object, which the compiler reads in
    /// its place without a diagnostic.
    fn config_root(&self, root: Option<JsonValue>) -> Result<JsonValue, InputError> {
        let Some(root) = root else {
            return Ok(JsonValue {
                native_span: None,
                span: crate::Span { start: 0, end: 0 },
                kind: JsonKind::Object(Vec::new()),
            });
        };
        let error = self.error(
            &root,
            "The root value of a configuration file must be an object",
        );
        match root.kind {
            JsonKind::Object(_) => Ok(root),
            JsonKind::Array(items) => items
                .into_iter()
                .find(|item| matches!(item.kind, JsonKind::Object(_)))
                .ok_or(error),
            _ => Err(error),
        }
    }

    /// The checks `convertToJson` and `onPropertySet` make as they convert
    /// each property, whichever occurrence of a key applies: no `?` after a
    /// key (TS8009), and the type of every `files`, `include`, `exclude`, and
    /// `references` occurrence. `extends` and `compilerOptions` occurrences
    /// are checked where they are read.
    fn check_root(&self) -> Result<(), InputError> {
        let mut pending = vec![&self.root];
        while let Some(value) = pending.pop() {
            match &value.kind {
                JsonKind::Array(items) => pending.extend(items.iter().rev()),
                JsonKind::Object(properties) => {
                    if let Some(question) = properties.iter().find_map(|p| p.question) {
                        return Err(self.origin_at(question).error(
                            InputErrorKind::Config,
                            "The '?' modifier can only be used in TypeScript files",
                        ));
                    }
                    pending.extend(properties.iter().rev().map(|p| &p.value));
                }
                _ => {}
            }
        }
        for name in ["files", "include", "exclude"] {
            for value in occurrences(&self.root, name).filter(|value| !value.is_null()) {
                string_list(self, value, name, false)?;
            }
        }
        for value in occurrences(&self.root, "references").filter(|value| !value.is_null()) {
            if value.as_array().is_none() {
                return Err(self.error(value, "'references' must be an array"));
            }
        }
        Ok(())
    }

    fn origin(&self, value: &JsonValue) -> Origin {
        self.origin_at(value.span.start)
    }

    fn origin_at(&self, offset: usize) -> Origin {
        Origin {
            path: PathBuf::from(&self.path),
            position: Some(self.text.position(offset)),
        }
    }

    fn error(&self, value: &JsonValue, message: impl Into<String>) -> InputError {
        self.origin(value).error(InputErrorKind::Config, message)
    }

    fn unsupported(&self, value: &JsonValue, message: impl Into<String>) -> InputError {
        self.origin(value)
            .error(InputErrorKind::Unsupported, message)
    }

    /// `message` for a value that is not the string expected, unless it is a
    /// string this tool cannot represent exactly (`JsonString::NonScalar`).
    fn not_string(&self, value: &JsonValue, message: impl Into<String>) -> InputError {
        match value.kind {
            JsonKind::String(JsonString::NonScalar(_)) => self.unsupported(
                value,
                "This string has an unpaired UTF-16 surrogate, which cannot name a file, module, or option value",
            ),
            _ => self.error(value, message),
        }
    }
}

/// Every value of `key` in a configuration object, in source order.
fn occurrences<'a>(object: &'a JsonValue, key: &'a str) -> impl Iterator<Item = &'a JsonValue> {
    object
        .properties()
        .unwrap_or_default()
        .iter()
        .filter(move |property| property.key.as_str() == Some(key))
        .map(|property| &property.value)
}

/// The value of `key` that applies: TypeScript's `OrderedMap` keeps the last.
fn get<'a>(object: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    object
        .properties()?
        .iter()
        .rev()
        .find(|property| property.key.as_str() == Some(key))
        .map(|property| &property.value)
}

/// A property that applies and is not `null`.
fn field<'a>(object: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    get(object, key).filter(|value| !value.is_null())
}

/// An object as TypeScript's `convertObjectLiteralExpressionToJson` builds
/// it: each key once, at its first position (the first property with it),
/// with its last value. TypeScript 7.0.2 neither inserts nor converts a
/// property whose name is empty; TypeScript 5.6.3 keeps it, as the shared
/// JSON reader, JSON modules, and package manifests do.
fn entries(rules: Rules, object: &JsonValue) -> Option<Vec<(&JsonProperty, &JsonValue)>> {
    let mut entries: Vec<(&JsonProperty, &JsonValue)> = Vec::new();
    let mut positions: HashMap<&JsonString, usize> = HashMap::new();
    for property in object.properties()? {
        if property.key.as_str() == Some("") && rules.0 == Profile::TypeScript7 {
            continue;
        }
        match positions.get(&property.key) {
            Some(&index) => entries[index].1 = &property.value,
            None => {
                positions.insert(&property.key, entries.len());
                entries.push((property, &property.value));
            }
        }
    }
    Some(entries)
}

/// The strings of a `files`, `include`, or `exclude` list. A `null` entry
/// converts to nothing (`convertJsonOptionOfListType`), so it is left out.
/// An entry this tool cannot represent exactly is a string all the same, so
/// it is `Unsupported` only where the list `applies` and its entries name
/// files.
fn string_list<'a>(
    file: &File,
    value: &'a JsonValue,
    name: &str,
    applies: bool,
) -> Result<Vec<(&'a str, &'a JsonValue)>, InputError> {
    let Some(items) = value.as_array() else {
        return Err(file.error(value, format!("'{name}' must be an array")));
    };
    let mut strings = Vec::new();
    for item in items {
        match &item.kind {
            JsonKind::String(JsonString::Text(text)) => strings.push((text.as_str(), item)),
            JsonKind::String(JsonString::NonScalar(_)) if !applies => {}
            JsonKind::Null => {}
            _ => return Err(file.not_string(item, format!("'{name}' entries must be strings"))),
        }
    }
    Ok(strings)
}

/// An option value with the file that declared it.
#[derive(Clone, Copy)]
struct Declared<'a> {
    value: &'a JsonValue,
    file: &'a File,
}

/// TypeScript 7.0.2's declaration of the option named exactly `name`, from
/// its `OptionsDeclarations`: the TypeScript 7 dialect's catalog
/// (TypeScript 5.6.3's is `catalog`), as far as this tool's checks of each
/// occurrence (`validate_option`) and its conversion (`conversion_7`) need
/// it. Command-line aliases are not configuration keys.
fn shape_7(name: &str) -> Option<Shape7> {
    match name {
        "preserveWatchOutput"
        | "listFiles"
        | "explainFiles"
        | "listEmittedFiles"
        | "pretty"
        | "traceResolution"
        | "diagnostics"
        | "extendedDiagnostics"
        | "incremental"
        | "declaration"
        | "declarationMap"
        | "emitDeclarationOnly"
        | "sourceMap"
        | "inlineSourceMap"
        | "noCheck"
        | "deduplicatePackages"
        | "noEmit"
        | "assumeChangesOnlyAffectDirectDependencies"
        | "quiet"
        | "singleThreaded"
        | "all"
        | "version"
        | "init"
        | "allowJs"
        | "checkJs"
        | "composite"
        | "removeComments"
        | "importHelpers"
        | "downlevelIteration"
        | "isolatedModules"
        | "verbatimModuleSyntax"
        | "isolatedDeclarations"
        | "erasableSyntaxOnly"
        | "libReplacement"
        | "strict"
        | "noImplicitAny"
        | "strictNullChecks"
        | "strictFunctionTypes"
        | "strictBindCallApply"
        | "strictPropertyInitialization"
        | "strictBuiltinIteratorReturn"
        | "noImplicitThis"
        | "useUnknownInCatchVariables"
        | "alwaysStrict"
        | "stableTypeOrdering"
        | "noUnusedLocals"
        | "noUnusedParameters"
        | "exactOptionalPropertyTypes"
        | "noImplicitReturns"
        | "noFallthroughCasesInSwitch"
        | "noUncheckedIndexedAccess"
        | "noImplicitOverride"
        | "noPropertyAccessFromIndexSignature"
        | "allowSyntheticDefaultImports"
        | "esModuleInterop"
        | "preserveSymlinks"
        | "allowUmdGlobalAccess"
        | "allowImportingTsExtensions"
        | "rewriteRelativeImportExtensions"
        | "resolvePackageJsonExports"
        | "resolvePackageJsonImports"
        | "noUncheckedSideEffectImports"
        | "inlineSources"
        | "experimentalDecorators"
        | "emitDecoratorMetadata"
        | "resolveJsonModule"
        | "allowArbitraryExtensions"
        | "skipDefaultLibCheck"
        | "emitBOM"
        | "noErrorTruncation"
        | "noLib"
        | "noResolve"
        | "stripInternal"
        | "disableSizeLimit"
        | "disableSourceOfProjectReferenceRedirect"
        | "disableSolutionSearching"
        | "disableReferencedProjectLoad"
        | "noEmitHelpers"
        | "noEmitOnError"
        | "preserveConstEnums"
        | "skipLibCheck"
        | "allowUnusedLabels"
        | "allowUnreachableCode"
        | "forceConsistentCasingInFileNames"
        | "useDefineForClassFields" => Some(Shape7::Value(Kind::Bool)),
        // `IsFilePath`.
        "generateCpuProfile" | "generateTrace" | "pprofDir" | "project" | "outFile" | "outDir"
        | "rootDir" | "tsBuildInfoFile" | "baseUrl" | "declarationDir" => {
            Some(Shape7::Value(Kind::Path))
        }
        "sourceRoot" | "mapRoot" | "jsxFactory" | "jsxFragmentFactory" | "jsxImportSource"
        | "reactNamespace" | "ignoreDeprecations" => Some(Shape7::Value(Kind::String)),
        "checkers" | "maxNodeModuleJsDepth" => Some(Shape7::Value(Kind::Number)),
        "help" | "watch" | "locale" | "showConfig" | "listFilesOnly" | "ignoreConfig" => {
            Some(Shape7::CommandLine)
        }
        "target" | "module" | "jsx" | "moduleResolution" | "newLine" | "moduleDetection" => {
            Some(Shape7::Value(Kind::Enum))
        }
        "lib" | "rootDirs" | "typeRoots" | "types" | "moduleSuffixes" | "customConditions" => {
            Some(Shape7::Value(Kind::List))
        }
        "paths" => Some(Shape7::Value(Kind::Object)),
        "plugins" => Some(Shape7::Plugins),
        // Existing navigation-only profile: an internal compiler setting
        // cannot turn off declaration-file resolution in this tool.
        "noDtsResolution" => Some(Shape7::Value(Kind::Bool)),
        _ => None,
    }
}

/// An option's declaration in TypeScript 7.0.2 (`shape_7`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape7 {
    /// A value of this kind: one of a primitive kind is checked on every
    /// occurrence (`validate_option`); an enumeration, list, or object is
    /// converted, and checked, where `Reader` reads it.
    Value(Kind),
    /// `plugins`, a list of objects, which only `validate_option` checks.
    Plugins,
    /// `IsCommandLineOnly`.
    CommandLine,
}

/// How TypeScript 7.0.2 converts option `name`'s string value or list
/// entries (`Rules::conversion`): its declarations mark the paths
/// (`IsFilePath`: the options `shape_7` declares `Kind::Path`, and the
/// entries of `rootDirs` and `typeRoots` in `commandLineOptionElements`)
/// and the one list that keeps falsy entries (`listPreserveFalsyValues`:
/// `moduleSuffixes`), and `handleOptionConfigDirTemplateSubstitution`
/// substitutes `${configDir}` in the options it names: `paths`,
/// `rootDirs`, `typeRoots`, and every path but `project` and `pprofDir`.
fn conversion_7(name: &str) -> Conversion {
    let path = shape_7(name) == Some(Shape7::Value(Kind::Path));
    match name {
        "rootDirs" | "typeRoots" => Conversion {
            path: true,
            template: true,
            ..Conversion::TEXT
        },
        "paths" => Conversion {
            template: true,
            ..Conversion::TEXT
        },
        "moduleSuffixes" => Conversion {
            preserve_falsy: true,
            ..Conversion::TEXT
        },
        "project" | "pprofDir" => Conversion {
            path,
            ..Conversion::TEXT
        },
        _ => Conversion {
            path,
            template: path,
            ..Conversion::TEXT
        },
    }
}

/// The primitive checks `convertJsonOption` makes on each occurrence,
/// before effective inheritance, by the dialect's catalog. Under TypeScript
/// 7, unknown nulls and empty keys are ignored by
/// `parseOwnConfigOfJsonSourceFile.onPropertySet`, and command-line-only
/// options are rejected even when null; TypeScript 5.6.3's checks are
/// `typescript_5_6::validate_option`'s.
fn validate_option(
    rules: Rules,
    name: &str,
    declared: Declared<'_>,
    key_start: usize,
) -> Result<(), InputError> {
    if rules.0 == Profile::TypeScript56 {
        return typescript_5_6::validate_option(name, declared, key_start);
    }
    let shape = shape_7(name);
    if shape == Some(Shape7::CommandLine) {
        return Err(declared.file.origin_at(key_start).error(
            InputErrorKind::Config,
            format!("Option '{name}' can only be specified on command line"),
        ));
    }
    if name.is_empty() || declared.value.is_null() {
        return Ok(());
    }
    let kind = match shape {
        // TypeScript 7.0.2 reports TS5025 naming the option for a key that
        // differs from one only by case, which `CommandLineOptionNameMap.Get`
        // finds lowered, and TS5023 otherwise, having no distance-based
        // suggestion (`createUnknownOptionError`). This profile words both as
        // TS5023, as it always has: a known difference, kept so that projects
        // read with TypeScript 7's rules behave as before (docs/references.md).
        None => {
            return Err(declared.file.origin_at(key_start).error(
                InputErrorKind::Config,
                format!("Unknown compiler option '{name}'"),
            ));
        }
        Some(Shape7::Value(kind)) => kind,
        Some(Shape7::Plugins) => {
            let Some(items) = declared.value.as_array() else {
                return Err(type_error(declared, name, Kind::List));
            };
            for item in items {
                if !item.is_null() && !matches!(item.kind, JsonKind::Object(_)) {
                    return Err(type_error(
                        Declared {
                            value: item,
                            ..declared
                        },
                        "plugin",
                        Kind::Object,
                    ));
                }
            }
            return Ok(());
        }
        Some(Shape7::CommandLine) => unreachable!("handled before null conversion"),
    };
    let fits = match kind {
        Kind::Bool => matches!(declared.value.kind, JsonKind::Bool(_)),
        Kind::Number => matches!(declared.value.kind, JsonKind::Number),
        Kind::String | Kind::Path => matches!(declared.value.kind, JsonKind::String(_)),
        Kind::Enum | Kind::List | Kind::Object => true,
    };
    if fits {
        Ok(())
    } else {
        Err(type_error(declared, name, kind))
    }
}

pub(super) fn load(path: &Path, disk: &disk::Disk, parser: &Parser) -> Result<Config, InputError> {
    let leaf = File::read(path, disk, parser)?;
    // The compiler installed for the selected configuration decides the
    // rules that read the whole chain.
    let compiler = compatibility::select(&leaf.dir, disk)?;
    let rules = compiler.dialect().config();
    let chain = inheritance(&leaf, &compiler, disk, parser)?;
    // Every occurrence of each compiler option in every file, which
    // TypeScript validates as it reads each file: every `compilerOptions`
    // object of a file and each repeated key.
    let mut options: HashMap<&str, Vec<Declared>> = HashMap::new();
    for file in &chain.order {
        rules.check_root_prototype(file)?;
        for compiler_options in occurrences(&file.root, "compilerOptions") {
            if compiler_options.is_null() {
                continue;
            }
            for property in rules.compiler_options(file, compiler_options)? {
                let declared = Declared {
                    value: &property.value,
                    file,
                };
                if let Some(name) = property.key.as_str() {
                    validate_option(rules, name, declared, property.key_span.start)?;
                    options.entry(name).or_default().push(declared);
                } else if !property.value.is_null() || rules.0 == Profile::TypeScript56 {
                    // No option's name is such a key, so it is unknown, which
                    // TypeScript 5.6.3 reports even when `null`.
                    return Err(file.origin_at(property.key_span.start).error(
                        InputErrorKind::Unsupported,
                        "A compiler option key has an unpaired UTF-16 surrogate",
                    ));
                }
            }
        }
    }
    let leaf_dir = leaf.dir.clone();
    let [files, include, exclude] =
        ["files", "include", "exclude"].map(|key| chain.root_list(key, rules));
    // A `null` may keep its key in the raw configuration, which blocks
    // inheritance under TypeScript 7, but then names no list
    // (`getPropFromRaw`).
    fn list(list: Option<RootList<'_>>) -> Option<RootList<'_>> {
        list.filter(|list| !list.declared.value.is_null())
    }
    let files_specs = list(files)
        .map(|list| specs_of(rules, list, "files", &leaf_dir, false))
        .transpose()?
        .flatten();
    if let Some(references) = field(&leaf.root, "references") {
        let items = references.as_array().expect("checked when read");
        if !items.is_empty() {
            return Err(leaf.unsupported(
                references,
                "Project references are not supported; load each referenced project separately",
            ));
        }
    }
    // TypeScript's TS18002 takes the list after inheritance, entries that are
    // not strings included, and needs a `null` or absent `extends`.
    if let Some(RootList { declared, .. }) = list(files) {
        let references = field(&leaf.root, "references").and_then(JsonValue::as_array);
        if declared
            .value
            .as_array()
            .is_some_and(<[JsonValue]>::is_empty)
            && references.is_none_or(<[JsonValue]>::is_empty)
            && field(&leaf.root, "extends").is_none()
        {
            return Err(declared.file.error(
                declared.value,
                format!("The 'files' list in config file '{}' is empty", leaf.path),
            ));
        }
    }
    let options = convert(&options, &chain, &leaf, compiler)?;
    Ok(Config {
        path: path.to_path_buf(),
        dir: leaf_dir.clone(),
        file: leaf.path.clone(),
        // `canJsonReportNoInputFiles`: the raw configuration, after
        // inheritance, has neither key, though either may be `null`.
        report_no_inputs: get(&leaf.root, "files").is_none()
            && files.is_none()
            && get(&leaf.root, "references").is_none(),
        files: files_specs,
        include: list(include)
            .map(|list| specs_of(rules, list, "include", &leaf_dir, true))
            .transpose()?
            .flatten(),
        // An `exclude` that names no list still keeps the output
        // directories from being excluded by default.
        exclude: list(exclude)
            .map(|list| specs_of(rules, list, "exclude", &leaf_dir, false))
            .transpose()?
            .map(Option::unwrap_or_default),
        options,
    })
}

/// A configuration's `extends` graph.
struct Chain {
    /// Every file once, each `extends` target, in array order, before the
    /// file that names it, and a target shared by several files at its last
    /// application. Occurrences are validated in this order.
    order: Vec<Rc<File>>,
    /// Every file once, each after every file it names, which is the order
    /// in which TypeScript's `parseConfig` completes them, with the
    /// positions in this list of the files its applying `extends` names, in
    /// order. Each option is folded over the whole graph, so a step must not
    /// hash paths: a 10,000-file chain is read once per option.
    bases_first: Vec<(Rc<File>, Vec<usize>)>,
}

impl Chain {
    /// TypeScript merges a file's own options over its bases'
    /// (`mergeCompilerOptions`) whenever it has an `extends` property, even
    /// an empty list, and only then do its explicit `null`s unset inherited
    /// values.
    fn extends(file: &File) -> bool {
        get(&file.root, "extends").is_some()
    }

    /// A fold over the graph, bases first: `step` computes a file's value
    /// from its bases and their values, in `extends` order, or from none
    /// when the file has no `extends`. Returns the selected configuration's
    /// value.
    fn fold<'a, T: Copy>(
        &'a self,
        mut step: impl FnMut(&'a File, Option<&[(&'a File, T)]>) -> T,
    ) -> T {
        let mut values: Vec<T> = Vec::with_capacity(self.bases_first.len());
        let mut bases = Vec::new();
        for (file, targets) in &self.bases_first {
            let inherited = if Self::extends(file) {
                bases.clear();
                bases.extend(
                    targets
                        .iter()
                        .map(|&base| (&*self.bases_first[base].0, values[base])),
                );
                Some(bases.as_slice())
            } else {
                None
            };
            let value = step(file, inherited);
            values.push(value);
        }
        *values.last().expect("the selected configuration")
    }

    /// `files`, `include`, or `exclude` as TypeScript's raw configuration
    /// holds it after inheritance (`applyExtendedConfig`'s
    /// `setPropertyValue`): a file's own value when the dialect lets it keep
    /// the key from its bases (`Rules::blocks_root_list`); otherwise the
    /// value of the last base whose own value, after its own inheritance,
    /// the dialect copies (`Rules::copies_root_list`). Paths in it stay
    /// relative to the file that declared them.
    fn root_list(&self, key: &str, rules: Rules) -> Option<RootList<'_>> {
        self.fold(|file, bases: Option<&[(&File, Option<RootList>)]>| {
            if let Some(value) = get(&file.root, key)
                && rules.blocks_root_list(value)
            {
                let declared = Declared { value, file };
                return Some(RootList {
                    declared,
                    rebased: false,
                });
            }
            bases?
                .iter()
                .filter_map(|&(base, list)| Some((base, list?)))
                .rfind(|(_, list)| rules.copies_root_list(list.declared.value))
                .map(|(base, list)| RootList {
                    rebased: list.rebased || base.dir != file.dir,
                    ..list
                })
        })
    }

    /// Compiler option `name` after inheritance (`parseConfig`): a file
    /// without `extends` has its own setting; otherwise each base's merged
    /// setting, in order, then the file's own, applies over the one before
    /// it by the dialect's rule (`Rules::over`).
    fn option<'a>(&'a self, name: &str, kind: Kind, rules: Rules) -> Option<Declared<'a>> {
        self.fold(|file, bases| {
            let own = rules.own_setting(file, name, kind);
            let Some(bases) = bases else {
                return own;
            };
            let inherited = bases
                .iter()
                .fold(Setting::Absent, |into, &(base, setting)| {
                    rules.over(into, base, name, setting)
                });
            rules.over(inherited, file, name, own)
        })
        .applied()
    }
}

impl Rules {
    /// A root-level `__proto__` property: TypeScript 5.6.3's raw conversion
    /// assigns it, which sets the raw configuration's prototype to an
    /// object value, where its inheritance tests then see lists its own
    /// lookups do not (`typescript_5_6::prototype_root`, refused).
    /// TypeScript 7.0.2 reads an ordinary key.
    fn check_root_prototype(self, file: &File) -> Result<(), InputError> {
        match self.0 {
            Profile::TypeScript56 => typescript_5_6::prototype_root(file),
            Profile::TypeScript7 => Ok(()),
        }
    }

    /// The properties a non-null `compilerOptions` occurrence sets options
    /// with (`onPropertySet`). An object's; TypeScript 5.6.3 accepts any
    /// JavaScript object (`convertJsonOption`), so an array too, whose
    /// conversion notifies no option, so it sets none and none of its
    /// entries is converted. Any other value is TS5024, as an array is
    /// under 7.0.2.
    fn compiler_options<'v>(
        self,
        file: &File,
        value: &'v JsonValue,
    ) -> Result<&'v [JsonProperty], InputError> {
        match (self.0, &value.kind) {
            (_, JsonKind::Object(properties)) => Ok(properties),
            (Profile::TypeScript56, JsonKind::Array(_)) => Ok(&[]),
            _ => Err(file.error(value, self.wording().compiler_options_type)),
        }
    }
}

/// A `files`, `include`, or `exclude` value after inheritance, with the
/// file that declared it.
#[derive(Clone, Copy)]
struct RootList<'a> {
    declared: Declared<'a>,
    /// Whether an `extends` step from the declaring file to the selected
    /// configuration changes directory. TypeScript 5.6.3 rebases each
    /// inherited entry onto the directory difference before filtering it
    /// (`applyExtendedConfig`), so a `null` or empty entry then names the
    /// declaring file's directory, where without a difference it stays an
    /// empty spec.
    rebased: bool,
}

/// One entry of a `files`, `include`, or `exclude` list as written, beside
/// the spec it names, if any (`Rules::root_entries`).
type RootEntry<'a> = (&'a JsonValue, Option<&'a str>);

impl Rules {
    /// Whether a file's own raw `files`, `include`, or `exclude` value keeps
    /// it from inheriting the key (`applyExtendedConfig`): TypeScript 5.6.3
    /// tests the value for truthiness, so an own `null` inherits; 7.0.2
    /// tests that the key is present.
    fn blocks_root_list(self, value: &JsonValue) -> bool {
        match self.0 {
            Profile::TypeScript56 => !value.is_null(),
            Profile::TypeScript7 => true,
        }
    }

    /// Whether a base's value, after its own inheritance, is copied into
    /// the file that extends it: a list that its raw conversion keeps
    /// (`nil_list`).
    fn copies_root_list(self, value: &JsonValue) -> bool {
        value
            .as_array()
            .is_some_and(|items| self.keeps_null_entries() || !nil_list(items))
    }

    /// Whether raw conversion keeps an array's `null` entries, as
    /// TypeScript 5.6.3's does; TypeScript 7.0.2's
    /// `convertArrayLiteralExpressionToJson` drops them, so that an array of
    /// only `null`s converts to no list at all (`nil_list`).
    fn keeps_null_entries(self) -> bool {
        match self.0 {
            Profile::TypeScript56 => true,
            Profile::TypeScript7 => false,
        }
    }

    /// The entries of a `files`, `include`, or `exclude` list after
    /// inheritance, as `getConfigFileSpecs` reads them: each entry raw
    /// conversion keeps (`keeps_null_entries`), as written, beside the spec
    /// it names, if any; or none when raw conversion makes the value no
    /// list. TypeScript 7.0.2 resolves an empty entry to the declaring
    /// file's directory. TypeScript 5.6.3 keeps only strings as specs, so a
    /// `null` names nothing, and matches nothing with an empty `include` or
    /// `exclude` spec (`getRegularExpressionsForWildcards`), while an empty
    /// `files` entry names the configuration's directory; an entry it
    /// rebased across directories (`RootList::rebased`) names the declaring
    /// file's directory instead. Entries are read one by one, in written
    /// order, so that a caller checks each before the next is read and the
    /// first that fails decides.
    fn root_entries<'a>(
        self,
        list: RootList<'a>,
        name: &'a str,
    ) -> Option<impl Iterator<Item = Result<RootEntry<'a>, InputError>> + 'a> {
        let Declared { value, file } = list.declared;
        let items = value.as_array().expect("checked when read");
        if !self.keeps_null_entries() && nil_list(items) {
            return None;
        }
        Some(items.iter().filter_map(move |item| {
            let text = match &item.kind {
                JsonKind::String(JsonString::Text(text)) => Some(text.as_str()),
                JsonKind::Null if self.keeps_null_entries() => None,
                JsonKind::Null => return None,
                _ => {
                    let message = format!("'{name}' entries must be strings");
                    return Some(Err(file.not_string(item, message)));
                }
            };
            let spec = match (self.0, text) {
                (_, Some(text)) if !text.is_empty() => Some(text),
                (Profile::TypeScript7, _) => Some(""),
                (Profile::TypeScript56, _) if list.rebased => Some(""),
                (Profile::TypeScript56, Some(_)) if name == "files" => Some(""),
                (Profile::TypeScript56, _) => None,
            };
            Some(Ok((item, spec)))
        }))
    }
}

/// Whether TypeScript 7.0.2's `convertArrayLiteralExpressionToJson`, which
/// drops an array's `null` entries, converts `items` to no list at all:
/// every entry, and at least one, is `null`. An empty array is an empty
/// list. 5.6.3's raw conversion keeps each `null`.
fn nil_list(items: &[JsonValue]) -> bool {
    !items.is_empty() && items.iter().all(JsonValue::is_null)
}

/// What an option's value is, as TypeScript 7.0.2 declares it (`shape_7`)
/// and converts it (`convertJsonOption`): the type TS5024 requires of it,
/// and whether an occurrence sets the option and whether the setting is the
/// zero value that inheritance passes over. TypeScript 5.6.3's declarations
/// have their own type (`catalog::Type`), and its own settings and
/// inheritance do not depend on what a value is (`Rules::own_setting`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Bool,
    Number,
    /// An enumeration, whose empty string converts to nothing.
    Enum,
    String,
    /// A string naming a file or directory, which an empty string names too:
    /// the declaring configuration's directory.
    Path,
    List,
    /// An object, such as `paths`.
    Object,
}

impl Kind {
    /// The type the TypeScript 7 profile's TS5024 names, in the words this
    /// tool has always used (5.6.3's are `catalog::Type::name`).
    fn name(self) -> &'static str {
        match self {
            Kind::Bool => "boolean",
            Kind::Number => "number",
            Kind::Enum | Kind::String | Kind::Path => "string",
            Kind::List => "list",
            Kind::Object => "object",
        }
    }

    /// Whether an occurrence sets the option: `onPropertySet` sets one only
    /// from a non-nil converted value. A `null` converts to nil, except for a
    /// list, whose conversion is a nil list that still sets it, and so clears
    /// an earlier setting; an empty enumeration string converts to nil.
    fn sets(self, value: &JsonValue) -> bool {
        match self {
            Kind::List => true,
            Kind::Enum => !value.is_null() && value.as_str() != Some(""),
            Kind::Bool | Kind::Number | Kind::String | Kind::Path | Kind::Object => {
                !value.is_null()
            }
        }
    }

    /// Whether the setting an occurrence makes is the option's zero value, a
    /// nil list or an empty string, which leaves the option unset and which
    /// `mergeCompilerOptions` does not copy over an inherited value.
    fn zero(self, value: &JsonValue) -> bool {
        match self {
            Kind::List => value.is_null(),
            Kind::String => value.as_str() == Some(""),
            Kind::Bool | Kind::Number | Kind::Enum | Kind::Path | Kind::Object => false,
        }
    }
}

/// An option's setting in one file, or through a file's bases, until
/// inheritance completes. TypeScript 5.6.3's `onPropertySet` assigns a
/// converted `undefined`, which `parseConfig`'s `assign` then copies over an
/// inherited value, so a set-to-nothing option differs from one no file sets.
#[derive(Clone, Copy)]
enum Setting<'a> {
    /// No occurrence sets the option.
    Absent,
    /// Set to nothing: 5.6.3's converted `undefined`, or 7.0.2's explicit
    /// `null` merge (`explicit_null`).
    Undefined,
    Present(Declared<'a>),
}

impl<'a> Setting<'a> {
    /// The occurrence that applies once inheritance is complete.
    fn applied(self) -> Option<Declared<'a>> {
        match self {
            Setting::Present(declared) => Some(declared),
            Setting::Absent | Setting::Undefined => None,
        }
    }
}

impl Rules {
    /// A file's own setting of option `name`, before inheritance
    /// (`parseOwnConfigOfJsonSourceFile`'s `onPropertySet`), from every
    /// occurrence across its `compilerOptions` objects. TypeScript 5.6.3
    /// assigns each occurrence's converted value, so the last decides, a
    /// `null` converting to `undefined`; its conversion already rejected
    /// every value that converts to nothing otherwise, such as an empty
    /// enumeration string (`typescript_5_6::validate_option`). TypeScript
    /// 7.0.2 sets the option only from a non-nil converted value
    /// (`Kind::sets`), and the zero value it may set leaves it unset
    /// (`Kind::zero`).
    fn own_setting<'a>(self, file: &'a File, name: &str, kind: Kind) -> Setting<'a> {
        let occurrences = occurrences(&file.root, "compilerOptions")
            .flat_map(|options| options.properties().unwrap_or_default())
            .filter(|property| property.key.as_str() == Some(name))
            .map(|property| &property.value);
        let setting = match self.0 {
            Profile::TypeScript56 => match occurrences.last() {
                None => return Setting::Absent,
                Some(value) if value.is_null() => return Setting::Undefined,
                Some(value) => Some(value),
            },
            Profile::TypeScript7 => occurrences
                .filter(|value| kind.sets(value))
                .last()
                .filter(|value| !kind.zero(value)),
        };
        match setting {
            Some(value) => Setting::Present(Declared { value, file }),
            None => Setting::Absent,
        }
    }

    /// `setting`, a file's own or merged setting, applied over `into`, the
    /// setting before it, as `parseConfig` merges a file over its bases and
    /// each base over the ones before it. TypeScript 5.6.3's `assign` copies
    /// a set option, `undefined` included; TypeScript 7.0.2's
    /// `mergeCompilerOptions` copies a set value, and unsets the option when
    /// the file's last raw value for it is `null` (`explicit_null`).
    fn over<'a>(
        self,
        into: Setting<'a>,
        file: &File,
        name: &str,
        setting: Setting<'a>,
    ) -> Setting<'a> {
        match (self.0, setting) {
            (Profile::TypeScript56, Setting::Absent) => into,
            (Profile::TypeScript56, _) => setting,
            (Profile::TypeScript7, _) if explicit_null(file, name) => Setting::Undefined,
            (Profile::TypeScript7, Setting::Present(_)) => setting,
            (Profile::TypeScript7, _) => into,
        }
    }
}

/// Whether `file`'s raw `compilerOptions`, its last such object, holds an
/// explicit `null` for `name` as its last value, which `mergeCompilerOptions`
/// applies as an unset when it merges the file's options.
fn explicit_null(file: &File, name: &str) -> bool {
    get(&file.root, "compilerOptions")
        .and_then(|options| get(options, name))
        .is_some_and(JsonValue::is_null)
}

/// The configuration's `extends` graph (`Chain`). A first walk reads each
/// file once in `extends` order, reports cycles as it meets them, and
/// completes each file after the files it names (`bases_first`). `order` is
/// then the reverse of a preorder walk that visits targets from last to first
/// and skips files already visited, which keeps each file's last position in
/// linear time.
fn inheritance(
    leaf: &Rc<File>,
    compiler: &CompilerSelection,
    disk: &disk::Disk,
    parser: &Parser,
) -> Result<Chain, InputError> {
    let mut files: HashMap<String, Rc<File>> = HashMap::new();
    let mut targets: HashMap<PathBuf, Vec<Rc<File>>> = HashMap::new();
    let mut positions: HashMap<PathBuf, usize> = HashMap::new();
    let mut bases_first = Vec::new();
    let mut active = HashSet::from([leaf.identity.clone()]);
    // Each frame is a file, its resolved `extends` targets, and the next one.
    let mut stack = vec![(leaf.clone(), extends_of(compiler, leaf, disk)?, 0)];
    while let Some((file, pending, next)) = stack.last_mut() {
        let Some((target, value)) = pending.get(*next).cloned() else {
            let parents: Vec<Rc<File>> = pending
                .iter()
                .map(|(target, _)| files[target].clone())
                .collect();
            let bases = parents
                .iter()
                .map(|parent| positions[&parent.identity])
                .collect();
            active.remove(&file.identity);
            positions.insert(file.identity.clone(), bases_first.len());
            targets.insert(file.identity.clone(), parents);
            bases_first.push((file.clone(), bases));
            stack.pop();
            continue;
        };
        *next += 1;
        let file = file.clone();
        let parent = match files.get(&target) {
            Some(parent) => parent.clone(),
            None => {
                // Report an unreadable parent where the chain names it.
                let parent = File::read(Path::new(&target), disk, parser).map_err(|error| {
                    match error.kind {
                        InputErrorKind::Io => file.origin(&value).error(
                            InputErrorKind::Io,
                            format!("Cannot read file '{target}': {}", error.message),
                        ),
                        _ => error,
                    }
                })?;
                files.insert(target, parent.clone());
                parent
            }
        };
        if active.contains(&parent.identity) {
            let mut cycle: Vec<&str> = stack
                .iter()
                .map(|(active, ..)| active.path.as_str())
                .collect();
            cycle.push(&parent.path);
            return Err(file.error(
                &value,
                format!(
                    "Circularity detected while resolving configuration: {}",
                    cycle.join(" -> ")
                ),
            ));
        }
        if targets.contains_key(&parent.identity) {
            continue;
        }
        active.insert(parent.identity.clone());
        let pending = extends_of(compiler, &parent, disk)?;
        stack.push((parent, pending, 0));
    }
    let mut visited = HashSet::new();
    let mut order = Vec::new();
    let mut walk = vec![leaf.clone()];
    while let Some(file) = walk.pop() {
        if visited.insert(file.identity.clone()) {
            walk.extend(targets[&file.identity].iter().cloned());
            order.push(file);
        }
    }
    order.reverse();
    Ok(Chain { order, bases_first })
}

/// The paths named by a file's `extends`, with the values that named them.
/// As in `getExtendsConfigPathOrArray`, every occurrence of `extends` is
/// checked and its paths looked up, and the last one applies. Unlike any
/// compiler option, `extends` may not be `null` (TS5024). A `null` entry
/// that raw conversion keeps (`Rules::keeps_null_entries`), as 5.6.3's
/// does, is converted as the whole list is, which is TS5024; TypeScript
/// 7.0.2's drops it, so it names nothing. Each dialect words the errors
/// its own way (`Rules::wording`).
fn extends_of(
    compiler: &CompilerSelection,
    file: &File,
    disk: &disk::Disk,
) -> Result<Vec<(String, JsonValue)>, InputError> {
    let rules = compiler.dialect().config();
    let wording = rules.wording();
    let mut applied = Vec::new();
    for value in occurrences(&file.root, "extends") {
        applied = Vec::new();
        let names: Vec<&JsonValue> = match &value.kind {
            JsonKind::String(_) => vec![value],
            JsonKind::Array(items) => items
                .iter()
                .filter(|item| rules.keeps_null_entries() || !item.is_null())
                .collect(),
            _ => return Err(file.not_string(value, wording.extends_type)),
        };
        for name in names {
            let Some(text) = name.as_str() else {
                return Err(file.not_string(name, wording.extends_entry));
            };
            applied.push((
                extends_path(compiler, file, name, text, disk)?,
                name.clone(),
            ));
        }
    }
    Ok(applied)
}

/// TypeScript's `extends` lookup (`getExtendsConfigPath`): a relative or
/// rooted path, adding `.json` when no file exists there (`FileExists`, so a
/// pipe there is selected, and refused where it is read), or else a package
/// resolved like a NodeNext `require` of a JSON configuration.
fn extends_path(
    compiler: &CompilerSelection,
    file: &File,
    value: &JsonValue,
    name: &str,
    disk: &disk::Disk,
) -> Result<String, InputError> {
    let name = paths::separators(name);
    if name.is_empty() {
        return Err(file.error(value, compiler.dialect().config().wording().extends_empty));
    }
    if paths::is_rooted(&name) || name.starts_with("./") || name.starts_with("../") {
        let path = paths::resolve(&file.dir, &name);
        let exists = |path: &str| {
            disk.file_exists(path)
                .map_err(|error| io_error(Path::new(path), &error))
        };
        if path.ends_with(".json") || exists(&path)? {
            return Ok(path);
        }
        let with_json = format!("{path}.json");
        if exists(&with_json)? {
            return Ok(with_json);
        }
    } else if let Some(path) = resolve::config_package(&name, &file.dir, disk, compiler)? {
        return Ok(path);
    }
    Err(file.error(value, format!("File '{name}' not found")))
}

/// TypeScript's template for the selected configuration's directory.
const CONFIG_DIR: &str = "${configDir}";

impl Rules {
    /// Whether `text` starts with `${configDir}` in any case, as
    /// `startsWithConfigDirTemplate` folds it. TypeScript 7.0.2 lowers the
    /// text with Go's `strings.ToLower`, one character for one
    /// (`input::simple_lowercase`), so `${confİgDir}` starts with it.
    /// TypeScript 5.6.3 compares the first 12 UTF-16 code units, uppercased
    /// with JavaScript's `toUpperCase` (`strings::to_upper_js`), with the
    /// template uppercased, so `${confıgDir}` starts with it, and so does
    /// `${conﬁgDir}` alone, whose ligature uppercases to two letters.
    fn starts_with_config_dir(self, text: &str) -> bool {
        match self.0 {
            Profile::TypeScript7 => text
                .chars()
                .map(crate::input::simple_lowercase)
                .take(CONFIG_DIR.len())
                .eq(CONFIG_DIR.to_ascii_lowercase().chars()),
            Profile::TypeScript56 => {
                // `text.slice(0, 12)`, the whole of a shorter text.
                let mut units = 0;
                let mut end = text.len();
                for (index, character) in text.char_indices() {
                    if units >= CONFIG_DIR.len() {
                        end = index;
                        break;
                    }
                    units += character.len_utf16();
                }
                // A slice that ends inside a surrogate pair ends with a lone
                // surrogate, which uppercases to no letter.
                units <= CONFIG_DIR.len()
                    && crate::strings::to_upper_js(&text.as_bytes()[..end])
                        == CONFIG_DIR.to_ascii_uppercase().as_bytes()
            }
        }
    }
}

/// A path in an option that takes the template (`Conversion::value`) or a
/// root spec. One that starts with the template
/// (`Rules::starts_with_config_dir`) names a path under the selected
/// configuration's directory once its first exact `${configDir}` becomes
/// `./` (`getSubstitutedPathWithConfigDirTemplate`), so `${CONFIGDIR}/src`
/// names `${CONFIGDIR}/src` there and `${configDir}src` names `src`, with
/// no trailing separator (`getNormalizedAbsolutePath`). Any other path is
/// relative to the declaring file.
fn config_path(rules: Rules, file: &File, text: &str, leaf_dir: &str) -> String {
    if rules.starts_with_config_dir(text) {
        paths::directory(paths::resolve(
            leaf_dir,
            &text.replacen(CONFIG_DIR, "./", 1),
        ))
    } else {
        paths::resolve(&file.dir, text)
    }
}

/// How an option's string value, or each entry of its list, converts
/// beyond its JSON type, as the option's declaration in the dialect's
/// catalog says (`Rules::conversion`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Conversion {
    /// `isFilePath`, of the option or of its list's element: each value
    /// names a file or directory relative to the configuration that
    /// declared it, an empty one that directory
    /// (`normalizeNonListOptionValue`).
    path: bool,
    /// The option takes `${configDir}`: a value that starts with it names a
    /// path under the selected configuration (`config_path`).
    template: bool,
    /// `listPreserveFalsyValues`: the list keeps the entries that convert
    /// to a falsy value, which `convertJsonOptionOfListType` otherwise
    /// leaves out.
    preserve_falsy: bool,
}

impl Conversion {
    /// Text, as written, in a list that leaves out falsy entries.
    const TEXT: Self = Self {
        path: false,
        template: false,
        preserve_falsy: false,
    };

    /// What a string option converted this way is: an empty path names a
    /// directory, so, unlike empty text, it is no zero value (`Kind::zero`).
    fn string_kind(self) -> Kind {
        if self.path { Kind::Path } else { Kind::String }
    }

    /// `text`, a value converted this way that `file` declares. Conversion
    /// leaves one that starts with the template as written
    /// (`normalizeNonListOptionValue`), for the substitution that only an
    /// option taking the template makes (`config_path`); any other path is
    /// relative to the declaring file, and text stays as written.
    fn value(self, rules: Rules, file: &File, text: &str, leaf_dir: &str) -> String {
        let path = if rules.starts_with_config_dir(text) {
            self.template
        } else {
            self.path
        };
        if path {
            config_path(rules, file, text, leaf_dir)
        } else {
            text.to_owned()
        }
    }
}

impl Rules {
    /// How option `name`'s string value, or each entry of its list,
    /// converts, by its declaration in the dialect's catalog: TypeScript
    /// 5.6.3's (`typescript_5_6::conversion`) or 7.0.2's (`conversion_7`).
    fn conversion(self, name: &str) -> Conversion {
        match self.0 {
            Profile::TypeScript7 => conversion_7(name),
            Profile::TypeScript56 => typescript_5_6::conversion(name),
        }
    }
}

/// The specs of a `files`, `include`, or `exclude` list after inheritance
/// (`getConfigFileSpecs`), each entry naming what the dialect's rule says
/// (`Rules::root_entries`), or none when the value is no list.
fn specs_of<'a>(
    rules: Rules,
    list: RootList<'a>,
    name: &'a str,
    leaf_dir: &str,
    include: bool,
) -> Result<Option<Specs>, InputError> {
    let Some(entries) = rules.root_entries(list, name) else {
        return Ok(None);
    };
    let file = list.declared.file;
    let mut specs = Specs::default();
    for entry in entries {
        let (item, spec) = entry?;
        specs.written.push(
            item.as_str()
                .map_or_else(|| "null".into(), |text| format!("{text:?}")),
        );
        let Some(text) = spec else {
            continue;
        };
        if name != "files" {
            let normalized = paths::separators(text);
            let trailing_recursion =
                normalized == "**" || normalized.ends_with("/**") || normalized.ends_with("/**/");
            if include && trailing_recursion {
                return Err(file.error(
                    item,
                    format!("File specification cannot end in a recursive directory wildcard ('**'): '{text}'"),
                ));
            }
            if let Some(index) = normalized.find("**") {
                let after = &normalized[index..];
                if after.split('/').any(|part| part == "..") {
                    return Err(file.error(
                        item,
                        format!("File specification cannot contain a parent directory ('..') that appears after a recursive directory wildcard ('**'): '{text}'"),
                    ));
                }
            }
        }
        specs.entries.push(Spec {
            text: text.to_owned(),
            path: config_path(rules, file, text, leaf_dir),
            origin: file.origin(item),
        });
    }
    Ok(Some(specs))
}

/// A string-array option's entries, each with its value for diagnostics.
type StringList<'a> = (Vec<(&'a str, &'a JsonValue)>, Declared<'a>);

/// A list option's converted entries (`Reader::list`), each with its value
/// for diagnostics.
type List<'a> = (Vec<(String, &'a JsonValue)>, Declared<'a>);

/// Typed access to the compiler options. Each accessor validates every
/// occurrence of its option in every file, as `convertJsonOption` does for
/// each one TypeScript reads, and returns the occurrence that applies after
/// each file's own settings and inheritance (`Chain::option`). Only a value
/// that applies names anything, so only its strings this tool cannot
/// represent exactly are `Unsupported`.
struct Reader<'a, 'm> {
    occurrences: &'m HashMap<&'a str, Vec<Declared<'a>>>,
    chain: &'a Chain,
    leaf_dir: &'m str,
    /// The dialect's rules, whose catalog enumerations `choice` reads.
    rules: Rules,
    /// The dialect's standard libraries, which `lib` entries name.
    libraries: libraries::Catalog,
}

impl<'a> Reader<'a, '_> {
    /// The occurrence that applies, if the option is set.
    fn get(&self, name: &str, kind: Kind) -> Option<Declared<'a>> {
        self.chain.option(name, kind, self.rules)
    }

    /// The occurrence that applies, after `check` accepts every non-null
    /// occurrence in order; `check` learns whether the one it sees applies.
    fn checked(
        &self,
        name: &str,
        kind: Kind,
        mut check: impl FnMut(Declared<'a>, bool) -> Result<(), InputError>,
    ) -> Result<Option<Declared<'a>>, InputError> {
        let applies = self.get(name, kind);
        let occurrences = self.occurrences.get(name).map_or(&[][..], Vec::as_slice);
        for &declared in occurrences {
            if !declared.value.is_null() {
                let applying = applies.is_some_and(|a| std::ptr::eq(a.value, declared.value));
                check(declared, applying)?;
            }
        }
        Ok(applies)
    }

    /// A string option's effective value, converted as its declaration
    /// says (`Rules::conversion`), so that a path is absolute.
    fn string(&self, name: &str) -> Result<Option<(String, Declared<'a>)>, InputError> {
        let conversion = self.rules.conversion(name);
        let kind = conversion.string_kind();
        let declared =
            self.checked(name, kind, |declared, applies| match &declared.value.kind {
                JsonKind::String(JsonString::Text(_)) => Ok(()),
                JsonKind::String(JsonString::NonScalar(_)) if !applies => Ok(()),
                _ => Err(type_error(declared, name, kind)),
            })?;
        Ok(declared.map(|declared| {
            let text = declared.value.as_str().expect("checked");
            let value = conversion.value(self.rules, declared.file, text, self.leaf_dir);
            (value, declared)
        }))
    }

    /// The occurrence of string option `name` that applies, if it is set,
    /// without `string`'s checks: for a setting refused whatever its value.
    fn string_setting(&self, name: &str) -> Option<Declared<'a>> {
        self.get(name, self.rules.conversion(name).string_kind())
    }

    /// A string option's effective text: an empty string, which only
    /// TypeScript 5.6.3 lets replace an inherited value, selects the
    /// default, as each consumer of the options read this way
    /// (`jsxFactory`, `jsxFragmentFactory`, `reactNamespace`,
    /// `jsxImportSource`) tests it for truthiness in both compilers.
    fn text(&self, name: &str) -> Result<Option<String>, InputError> {
        Ok(self
            .string(name)?
            .map(|(text, _)| text)
            .filter(|text| !text.is_empty()))
    }

    /// Primitive types were validated on every occurrence by the catalog
    /// before inheritance; this reads the effective boolean only.
    fn bool(&self, name: &str) -> Option<bool> {
        self.get(name, Kind::Bool)
            .map(|declared| declared.value.as_bool().expect("validated boolean"))
    }

    /// A number option's effective value, as both compilers convert its
    /// literal (`convertToJson`): validated as a number by the catalog
    /// before inheritance, like `bool`.
    fn number(&self, name: &str) -> Option<f64> {
        self.get(name, Kind::Number).map(|declared| {
            let span = declared.value.span;
            let text = &declared.file.text.as_str()[span.start..span.end];
            crate::numbers::number_from_string(text)
        })
    }

    /// TypeScript's `CompilerOptions.GetAllowJS`: explicit `allowJs`
    /// wins; otherwise `checkJs` true enables JavaScript program inputs.
    fn allow_js(&self) -> bool {
        self.bool("allowJs")
            .unwrap_or_else(|| self.bool("checkJs") == Some(true))
    }

    /// A list of strings; `check_entry` validates each string entry of every
    /// occurrence, written exactly or not. A `null` entry converts to nothing
    /// and is left out (`convertJsonOptionOfListType`).
    fn strings_with(
        &self,
        name: &str,
        mut check_entry: impl FnMut(Declared<'a>, Option<&'a str>) -> Result<(), InputError>,
    ) -> Result<Option<StringList<'a>>, InputError> {
        let declared = self.checked(name, Kind::List, |declared, applies| {
            let Some(items) = declared.value.as_array() else {
                return Err(type_error(declared, name, Kind::List));
            };
            for item in items {
                let entry = Declared {
                    value: item,
                    file: declared.file,
                };
                match &item.kind {
                    JsonKind::String(JsonString::Text(text)) => check_entry(entry, Some(text))?,
                    JsonKind::String(JsonString::NonScalar(_)) if !applies => {
                        check_entry(entry, None)?;
                    }
                    JsonKind::Null => {}
                    _ => {
                        return Err(declared
                            .file
                            .not_string(item, format!("'{name}' entries must be strings")));
                    }
                }
            }
            Ok(())
        })?;
        Ok(declared.map(|declared| {
            let strings = declared
                .value
                .as_array()
                .expect("checked")
                .iter()
                .filter_map(|item| Some((item.as_str()?, item)))
                .collect();
            (strings, declared)
        }))
    }

    /// A list option's effective entries, each converted as its
    /// declaration says (`Rules::conversion`), without those
    /// `convertJsonOptionOfListType` leaves out: a `null`, and an empty
    /// string that is not a path, unless the list keeps falsy entries. A
    /// `null` that raw conversion keeps (`Rules::keeps_null_entries`)
    /// converts to `undefined`, which such a list keeps and TypeScript 5.6.3
    /// spells `undefined` in file names: refused where the list applies, as
    /// TypeScript 7.0.2 drops the entry.
    fn list(&self, name: &str) -> Result<Option<List<'a>>, InputError> {
        let conversion = self.rules.conversion(name);
        let Some((items, declared)) = self.strings_with(name, |_, _| Ok(()))? else {
            return Ok(None);
        };
        if conversion.preserve_falsy
            && self.rules.keeps_null_entries()
            && let Some(null) = declared
                .value
                .as_array()
                .and_then(|values| values.iter().find(|value| value.is_null()))
        {
            return Err(typescript_5_6::undefined_entry(declared.file, null, name));
        }
        let entries = items
            .into_iter()
            .map(|(text, item)| {
                let value = conversion.value(self.rules, declared.file, text, self.leaf_dir);
                (value, item)
            })
            .filter(|(value, _)| conversion.preserve_falsy || !value.is_empty())
            .collect();
        Ok(Some((entries, declared)))
    }

    /// A directory-valued option, in the form `Options` documents.
    fn directory(&self, name: &str) -> Result<Option<String>, InputError> {
        Ok(self.string(name)?.map(|(path, _)| paths::directory(path)))
    }

    /// A list of directories (`list`), in the form `Options` documents.
    fn directory_list(&self, name: &str) -> Result<Option<Vec<String>>, InputError> {
        Ok(self.list(name)?.map(|(entries, _)| {
            entries
                .into_iter()
                .map(|(path, _)| paths::directory(path))
                .collect()
        }))
    }

    /// A case-insensitive enumeration value (`Rules::enum_key`), which must
    /// be one of the keys of the option's whole map in the dialect's catalog
    /// (`Rules::enumeration`): removed values are keys too, and only the
    /// value that applies reports them. TS6046 lists the keys the catalog
    /// lists. Under TypeScript 7, an empty string sets nothing.
    fn choice(&self, name: &str) -> Result<Option<(String, Declared<'a>)>, InputError> {
        let rules = self.rules;
        let (keys, listed) = rules.enumeration(name);
        let declared = self.checked(name, Kind::Enum, |declared, applies| {
            match &declared.value.kind {
                JsonKind::String(JsonString::Text(text)) => match rules.enum_key(text) {
                    Some(key) if !keys.contains(&key.as_str()) => {
                        Err(rules.invalid_value(declared, name, &listed))
                    }
                    _ => Ok(()),
                },
                // No key is an unpaired surrogate.
                JsonKind::String(JsonString::NonScalar(_)) if !applies => {
                    Err(rules.invalid_value(declared, name, &listed))
                }
                _ => Err(type_error(declared, name, Kind::Enum)),
            }
        })?;
        Ok(declared.map(|declared| {
            let key = rules.enum_key(declared.value.as_str().expect("checked"));
            (key.expect("an empty string sets nothing"), declared)
        }))
    }
}

/// The key TypeScript 7's `convertJsonOptionOfEnumType` looks up for an
/// enumeration value or list entry: none for an empty string, which leaves
/// it unset without a diagnostic; otherwise Go's `strings.ToLower`, one
/// character for one, so `K` (KELVIN SIGN) is `k` and U+0130 is `i`.
fn simple_enum_key(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.chars().map(crate::input::simple_lowercase).collect())
}

/// The values an enumeration's TS6046 message lists.
fn quoted(names: &[&str]) -> String {
    let quoted: Vec<String> = names.iter().map(|name| format!("'{name}'")).collect();
    quoted.join(", ")
}

/// TS5024 for option `name`'s value, which is not of `kind`.
fn type_error(declared: Declared, name: &str, kind: Kind) -> InputError {
    let message = format!(
        "Compiler option '{name}' requires a value of type {}",
        kind.name()
    );
    // `convertJsonOption`'s expected-kind check is different from exact
    // string identity. A surrogate-valued string is still the wrong kind
    // for booleans, numbers, lists, and objects (TS5024 / Config).
    match kind {
        Kind::String | Kind::Path | Kind::Enum => declared.file.not_string(declared.value, message),
        Kind::Bool | Kind::Number | Kind::List | Kind::Object => {
            declared.file.error(declared.value, message)
        }
    }
}

fn convert<'a>(
    options: &HashMap<&'a str, Vec<Declared<'a>>>,
    chain: &'a Chain,
    leaf: &File,
    compiler: CompilerSelection,
) -> Result<Options, InputError> {
    let rules = compiler.dialect().config();
    let read = Reader {
        occurrences: options,
        chain,
        leaf_dir: &leaf.dir,
        rules,
        libraries: compiler.dialect().libraries(),
    };
    let Effective {
        resolution,
        detection,
        jsx,
        target,
        lib,
        types,
        jsx_import_source,
        paths,
        base_url,
        mut semantic,
        resolve_json_module,
        check_side_effect_imports,
        lib_replacement,
    } = match rules.0 {
        Profile::TypeScript7 => effective_7(&read, &compiler)?,
        Profile::TypeScript56 => typescript_5_6::effective(&read, leaf)?,
    };
    let jsx_set = jsx.is_some();
    let jsx_dev = jsx
        .as_ref()
        .is_some_and(|(value, _)| value == "react-jsxdev");
    let jsx_automatic = jsx
        .as_ref()
        .is_some_and(|(value, _)| matches!(value.as_str(), "react-jsx" | "react-jsxdev"));
    let allow_js = semantic.allow_js;
    let preserve_const_enums = semantic.preserve_const_enums;
    let no_lib = read.bool("noLib").unwrap_or(false);
    semantic.explicit_dom_library = lib
        .as_ref()
        .is_some_and(|files| files.contains(&"lib.dom.d.ts"));

    let strings = |name: &str| -> Result<Vec<String>, InputError> {
        Ok(read
            .list(name)?
            .map(|(entries, _)| entries.into_iter().map(|(text, _)| text).collect())
            .unwrap_or_default())
    };

    Ok(Options {
        compiler,
        semantic,
        resolution,
        detection,
        jsx_automatic,
        jsx_dev,
        jsx: jsx_set,
        jsx_preserve: jsx.as_ref().is_some_and(|(value, _)| value == "preserve"),
        jsx_import_source,
        paths,
        base_url,
        root_dirs: read.directory_list("rootDirs")?.unwrap_or_default(),
        type_roots: read.directory_list("typeRoots")?,
        types,
        custom_conditions: strings("customConditions")?,
        module_suffixes: strings("moduleSuffixes")?,
        preserve_symlinks: read.bool("preserveSymlinks").unwrap_or(false),
        check_side_effect_imports,
        skip_lib_check: read.bool("skipLibCheck").unwrap_or(false),
        no_check: read.bool("noCheck").unwrap_or(false),
        allow_js,
        max_node_module_js_depth: read
            .number("maxNodeModuleJsDepth")
            .map_or(0, |depth| depth.floor() as i64),
        allow_arbitrary_extensions: read.bool("allowArbitraryExtensions").unwrap_or(false),
        allow_importing_ts_extensions: read.bool("allowImportingTsExtensions").unwrap_or(false),
        rewrite_relative_import_extensions: read
            .bool("rewriteRelativeImportExtensions")
            .unwrap_or(false),
        resolve_json_module,
        package_exports: read.bool("resolvePackageJsonExports").unwrap_or(true),
        package_imports: read.bool("resolvePackageJsonImports").unwrap_or(true),
        target,
        lib,
        no_lib,
        lib_replacement,
        skip_default_lib_check: read.bool("skipDefaultLibCheck").unwrap_or(false),
        out_dir: read.directory("outDir")?,
        declaration_dir: read.directory("declarationDir")?,
        root_dir: read.directory("rootDir")?,
        composite: read.bool("composite").unwrap_or(false),
        preserve_const_enums,
    })
}

/// The files `lib` names, deduplicated by file and in written order, or
/// none when unset; each entry of every occurrence must name a library
/// (TS6046), and `lib` beside `noLib` is TS5053.
fn libraries(read: &Reader<'_, '_>) -> Result<Option<Vec<&'static str>>, InputError> {
    let (rules, library_catalog) = (read.rules, read.libraries);
    let lib_entry =
        |entry: Declared, text: Option<&str>| match text.map(|text| rules.enum_key(text)) {
            Some(None) => Ok(()),
            Some(Some(key)) if library_catalog.option_file(&key).is_some() => Ok(()),
            _ => Err(rules.invalid_value(entry, "lib", &library_catalog.option_names())),
        };
    match read.strings_with("lib", lib_entry)? {
        None => Ok(None),
        Some((_, declared)) if read.bool("noLib") == Some(true) => Err(declared.file.error(
            declared.value,
            "Option 'lib' cannot be specified with option 'noLib'",
        )),
        Some((entries, _)) => {
            let mut files = Vec::new();
            for key in entries
                .into_iter()
                .filter_map(|(text, _)| rules.enum_key(text))
            {
                let file = library_catalog.option_file(&key).expect("checked");
                if !files.contains(&file) {
                    files.push(file);
                }
            }
            Ok(Some(files))
        }
    }
}

/// The entries of `types` after inheritance, without empty ones, or none
/// when it is unset; `*` is TypeScript 7.0.2's wildcard and 5.6.3's
/// package name.
fn type_names(read: &Reader<'_, '_>) -> Result<Option<Vec<(TypeName, Origin)>>, InputError> {
    Ok(read.list("types")?.map(|(items, declared)| {
        items
            .into_iter()
            .map(|(name, item)| {
                let name = match (read.rules.0, name.as_str()) {
                    (Profile::TypeScript7, "*") => TypeName::Wildcard,
                    _ => TypeName::Package(name),
                };
                (name, declared.file.origin(item))
            })
            .collect()
    }))
}

/// `paths` after inheritance, with its checks (`paths_option`), relative to
/// `base_url` when it is set, which only the TypeScript 5.6 dialect reads.
fn paths_setting(
    read: &Reader<'_, '_>,
    base_url: Option<&str>,
) -> Result<Option<Paths>, InputError> {
    let rules = read.rules;
    read.checked("paths", Kind::Object, |declared, _| {
        match declared.value.properties() {
            Some(_) => Ok(()),
            // TypeScript 5.6.3 reads an array as an object too.
            None if rules.0 == Profile::TypeScript56 && declared.value.as_array().is_some() => {
                Ok(())
            }
            None => Err(type_error(declared, "paths", Kind::Object)),
        }
    })?
    .map(|declared| paths_option(rules, declared, read.leaf_dir, base_url))
    .transpose()
}

/// What a dialect's defaults, removed options, refusals, and verification
/// checks decide (`effective_7`, `typescript_5_6::effective`), for the rest
/// of `convert`, which both dialects share.
struct Effective<'a> {
    resolution: Resolution,
    detection: Detection,
    /// `jsx`'s lowered key and the occurrence that set it.
    jsx: Option<(String, Declared<'a>)>,
    target: Target,
    /// `lib` (`libraries`), `types` (`type_names`), `jsxImportSource`,
    /// `paths` (`paths_setting`), and `baseUrl`, each read where the
    /// dialect's checks of it come.
    lib: Option<Vec<&'static str>>,
    types: Types,
    jsx_import_source: Option<String>,
    paths: Option<Paths>,
    base_url: Option<String>,
    /// With the module kind and resolution, `preserve_const_enums`, and the
    /// strict family decided; `explicit_dom_library` is `convert`'s.
    semantic: SemanticOptions,
    resolve_json_module: bool,
    check_side_effect_imports: bool,
    lib_replacement: bool,
}

/// "was removed in TypeScript 7", naming the installed compiler when it is
/// another major version: the profile and the compiler are different facts.
fn removed_in_7(compiler: &CompilerSelection) -> String {
    match compiler.installed_note() {
        None => "was removed in TypeScript 7".into(),
        Some(note) => {
            format!("was removed in TypeScript 7, whose rules read this project ({note})")
        }
    }
}

/// TypeScript 7.0.2's defaults (`GetStrictOptionValue`, `GetEmitModuleKind`,
/// `GetModuleResolutionKind`, `GetResolveJsonModule`, and the other
/// `CompilerOptions` getters), the settings it removed, and the
/// `verifyCompilerOptions` checks this tool ports, in this tool's order.
fn effective_7<'a>(
    read: &Reader<'a, '_>,
    compiler: &CompilerSelection,
) -> Result<Effective<'a>, InputError> {
    let removed = removed_in_7(compiler);
    // TypeScript's `verifyCompilerOptions` rejects removed settings only
    // after inheritance: an overridden false interop mode is harmless.
    for name in [
        "alwaysStrict",
        "esModuleInterop",
        "allowSyntheticDefaultImports",
    ] {
        if read.bool(name) == Some(false) {
            let declared = read.get(name, Kind::Bool).expect("present");
            return Err(declared
                .file
                .error(declared.value, format!("Option '{name}=false' {removed}")));
        }
    }
    for (name, declared) in [
        (
            "downlevelIteration",
            read.get("downlevelIteration", Kind::Bool),
        ),
        ("outFile", read.string_setting("outFile")),
    ] {
        if let Some(declared) = declared {
            return Err(declared
                .file
                .error(declared.value, format!("Option '{name}' {removed}")));
        }
    }
    // The remaining enumeration is emit-only, but invalid values are
    // still configuration errors (`convertJsonOptionOfEnumType`).
    read.choice("newLine")?;
    if let Some(declared) = read.string_setting("baseUrl") {
        return Err(declared.file.unsupported(
            declared.value,
            format!("Option 'baseUrl' {removed}; use 'paths' relative to the configuration"),
        ));
    }
    if let Some(declared) = read.get("noDtsResolution", Kind::Bool) {
        return Err(declared.file.unsupported(
            declared.value,
            "Option 'noDtsResolution' is an internal compiler setting outside the supported configuration profile",
        ));
    }
    if read.bool("noResolve") == Some(true) {
        let declared = read.get("noResolve", Kind::Bool).expect("present");
        return Err(declared.file.unsupported(
            declared.value,
            "Option 'noResolve' changes module resolution in a way that is not supported",
        ));
    }

    // `module` also decides `resolveJsonModule`'s default: TypeScript's
    // `GetResolveJsonModule` turns it on for `node20` and `nodenext`, off for
    // `node16` and `node18`, and otherwise only with bundler resolution.
    let (module, node_next_module, node_json_modules, module_kind) = match read.choice("module")? {
        None => (Module::Es, false, false, super::ModuleKind::EsNext),
        Some((value, declared)) => match value.as_str() {
            "commonjs" => (Module::CommonJs, false, false, super::ModuleKind::CommonJs),
            "es6" | "es2015" => (Module::Es, false, false, super::ModuleKind::Es2015),
            "es2020" => (Module::Es, false, false, super::ModuleKind::Es2020),
            "es2022" => (Module::Es, false, false, super::ModuleKind::Es2022),
            "esnext" => (Module::Es, false, false, super::ModuleKind::EsNext),
            "node16" => (Module::Node, false, false, super::ModuleKind::Node16),
            "node18" => (Module::Node, false, false, super::ModuleKind::Node18),
            "node20" => (Module::Node, false, true, super::ModuleKind::Node20),
            "nodenext" => (Module::Node, true, true, super::ModuleKind::NodeNext),
            "preserve" => (Module::Preserve, false, false, super::ModuleKind::Preserve),
            "amd" | "umd" | "system" => {
                return Err(declared
                    .file
                    .unsupported(declared.value, format!("Option 'module={value}' {removed}")));
            }
            _ => unreachable!("`choice` accepts only the option's keys"),
        },
    };
    let resolution = match read.choice("moduleResolution")? {
        None => match module {
            Module::Node if node_next_module => Resolution::NodeNext,
            Module::Node => Resolution::Node16,
            _ => Resolution::Bundler,
        },
        Some((value, declared)) => {
            let resolution = match value.as_str() {
                "node16" => Resolution::Node16,
                "nodenext" => Resolution::NodeNext,
                "bundler" => Resolution::Bundler,
                "node" | "node10" | "classic" => {
                    return Err(declared.file.unsupported(
                        declared.value,
                        format!("Option 'moduleResolution={value}' {removed}"),
                    ));
                }
                _ => unreachable!("`choice` accepts only the option's keys"),
            };
            let node_resolution = resolution != Resolution::Bundler;
            if node_resolution != (module == Module::Node) {
                return Err(declared.file.error(
                    declared.value,
                    if node_resolution {
                        format!("Option 'module' must be set to a Node.js module kind when option 'moduleResolution' is set to '{value}'")
                    } else {
                        "Option 'moduleResolution' must be 'node16' or 'nodenext' (or left unspecified) when option 'module' is a Node.js module kind".into()
                    },
                ));
            }
            resolution
        }
    };
    let detection = match read.choice("moduleDetection")? {
        None if module == Module::Node => Detection::Force,
        None => Detection::Auto,
        Some((value, _)) => detection(&value),
    };
    let jsx = read.choice("jsx")?;

    let target = match read.choice("target")? {
        None => Target::LATEST_STANDARD,
        Some((value, declared)) => match Target::from_key(&value) {
            Some(Target::Es5) => {
                return Err(declared
                    .file
                    .unsupported(declared.value, format!("Option 'target=ES5' {removed}")));
            }
            Some(target) => target,
            None => unreachable!("`choice` accepts only the option's keys"),
        },
    };
    // `GetStrictOptionValue`: the explicit flag wins, otherwise strict,
    // otherwise on.
    let strict = read.bool("strict");
    let mut semantic = SemanticOptions::from_reader(read, target, &|name| {
        read.bool(name).or(strict).unwrap_or(true)
    })?;
    semantic.module_kind = module_kind;
    semantic.module_resolution = resolution;
    if !semantic.strict_null_checks {
        for name in ["strictPropertyInitialization", "exactOptionalPropertyTypes"] {
            if read.bool(name) == Some(true) {
                let declared = read.get(name, Kind::Bool).expect("present");
                return Err(declared.file.error(
                    declared.value,
                    format!(
                        "Option '{name}' cannot be specified without option 'strictNullChecks'"
                    ),
                ));
            }
        }
    }
    semantic.preserve_const_enums =
        read.bool("preserveConstEnums").unwrap_or(false) || semantic.isolated_modules;
    let lib = libraries(read)?;
    let types = type_names(read)?;
    let jsx_import_source = read.text("jsxImportSource")?;
    let paths = paths_setting(read, None)?;
    Ok(Effective {
        resolution,
        detection,
        jsx,
        target,
        lib,
        types: types.map_or(Types::Listed(Vec::new()), Types::Listed),
        jsx_import_source,
        paths,
        base_url: None,
        semantic,
        resolve_json_module: read.bool("resolveJsonModule").unwrap_or(match module {
            Module::Node => node_json_modules,
            _ => resolution == Resolution::Bundler,
        }),
        check_side_effect_imports: read.bool("noUncheckedSideEffectImports").unwrap_or(true),
        lib_replacement: read.bool("libReplacement").unwrap_or(false),
    })
}

/// The detection a `moduleDetection` key selects, alike in both dialects.
fn detection(key: &str) -> Detection {
    match key {
        "auto" => Detection::Auto,
        "legacy" => Detection::Legacy,
        "force" => Detection::Force,
        _ => unreachable!("`choice` accepts only the option's keys"),
    }
}

/// `paths`, and `verifyCompilerOptions`' checks of it (TS5061, TS5062,
/// TS5063, TS5064, TS5066, TS5090), which TypeScript 5.6.3 words as its
/// own: an incorrect type names the substitution and its JavaScript type,
/// and TS5090 names `baseUrl`. An array, which 5.6.3 reads as an object, is
/// its `typescript_5_6::paths_array`. Substitutions are relative to
/// `base_url` when it is set, which also allows non-relative ones, and
/// otherwise to the declaring file's directory (`getPathsBasePath`); one
/// that starts with `${configDir}` is substituted first (`config_path`),
/// and checked and named as substituted.
fn paths_option(
    rules: Rules,
    declared: Declared,
    leaf_dir: &str,
    base_url: Option<&str>,
) -> Result<Paths, InputError> {
    let file = declared.file;
    let conversion = rules.conversion("paths");
    let mut patterns = Vec::new();
    let mappings = match entries(rules, declared.value) {
        Some(mappings) => mappings,
        None => {
            typescript_5_6::paths_array(declared)?;
            Vec::new()
        }
    };
    for (property, value) in mappings {
        // A pattern matches specifiers by their UTF-16 code units, so one
        // with an unpaired surrogate cannot be matched exactly here.
        let Some(pattern) = property.key.as_str() else {
            return Err(file.origin_at(property.key_span.start).error(
                InputErrorKind::Unsupported,
                "A 'paths' pattern has an unpaired UTF-16 surrogate, which cannot match a module name exactly",
            ));
        };
        if pattern == "__proto__" && rules.0 == Profile::TypeScript56 {
            return Err(typescript_5_6::prototype_pattern(file, property));
        }
        if pattern.matches('*').count() > 1 {
            return Err(file.error(
                value,
                format!("Pattern '{pattern}' can have at most one '*' character"),
            ));
        }
        let Some(items) = value.as_array() else {
            return Err(file.error(
                value,
                format!("Substitutions for pattern '{pattern}' should be an array"),
            ));
        };
        if items.is_empty() {
            return Err(file.error(
                value,
                format!("Substitutions for pattern '{pattern}' shouldn't be an empty array"),
            ));
        }
        let mut substitutions = Vec::new();
        for item in items {
            let Some(text) = item.as_str() else {
                return Err(match rules.0 {
                    Profile::TypeScript56 if !matches!(item.kind, JsonKind::String(_)) => {
                        typescript_5_6::substitution_type(file, item, pattern)
                    }
                    _ => file.not_string(
                        item,
                        format!("Substitution for pattern '{pattern}' must be a string"),
                    ),
                });
            };
            // Both compilers substitute `${configDir}` and normalize the
            // result (`getSubstitutedPathWithConfigDirTemplate`) before
            // `verifyCompilerOptions` checks the substitution, by its value
            // then: `${configDir}/x*/../*` has one `*`, and a `*` in the
            // configuration's directory counts.
            let substitution = conversion.value(rules, file, text, leaf_dir);
            if substitution.matches('*').count() > 1 {
                return Err(file.error(
                    item,
                    format!("Substitution '{substitution}' in pattern '{pattern}' can have at most one '*' character"),
                ));
            }
            if base_url.is_none()
                && !paths::is_relative(&substitution)
                && !paths::is_rooted(&substitution)
            {
                return Err(file.error(item, rules.wording().non_relative_substitution));
            }
            substitutions.push(substitution);
        }
        patterns.push((pattern.to_owned(), substitutions));
    }
    Ok(Paths {
        base: base_url.map_or_else(|| file.dir.clone(), str::to_owned),
        patterns,
    })
}

#[cfg(test)]
mod tests {
    use super::super::test_compiler::{MANIFEST, manifest};
    use super::SemanticOptions;
    use crate::project::Project;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const FLAGS: [&str; 8] = [
        "noImplicitAny",
        "noImplicitThis",
        "strictNullChecks",
        "strictFunctionTypes",
        "strictBindCallApply",
        "strictPropertyInitialization",
        "strictBuiltinIteratorReturn",
        "useUnknownInCatchVariables",
    ];

    fn strict_values(options: SemanticOptions) -> [bool; 8] {
        [
            options.no_implicit_any,
            options.no_implicit_this,
            options.strict_null_checks,
            options.strict_function_types,
            options.strict_bind_call_apply,
            options.strict_property_initialization,
            options.strict_builtin_iterator_return,
            options.use_unknown_in_catch_variables,
        ]
    }

    /// Loads the final Project boundary, then inspects its private snapshot;
    /// this does not create a public config API for test access.
    fn snapshot(config: &str, bases: &[(&str, &str)]) -> SemanticOptions {
        snapshot_with(config, bases, None)
    }

    /// `snapshot` of a project whose installed compiler declares `version`.
    fn snapshot_with(
        config: &str,
        bases: &[(&str, &str)],
        compiler: Option<&str>,
    ) -> SemanticOptions {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "tsparse-semantic-options-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        for (file, text) in bases
            .iter()
            .copied()
            .chain([("tsconfig.json", config), ("a.ts", "export {};\n")])
        {
            std::fs::write(root.join(file), text).unwrap();
        }
        if let Some(version) = compiler {
            let path = root.join(MANIFEST);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, manifest(version)).unwrap();
        }
        let result = Project::load(&root.join("tsconfig.json"));
        std::fs::remove_dir_all(root).unwrap();
        result.unwrap().semantic_options().clone()
    }

    fn configuration(options: &str) -> String {
        format!(r#"{{"compilerOptions":{{"noLib":true,"types":[],{options}}},"files":["a.ts"]}}"#)
    }

    #[test]
    fn semantic_options_strict_defaults_and_explicit_overrides() {
        // `GetStrictOptionValue`: true unless strict is explicitly false;
        // each strict flag overrides it in either direction.
        for (strict, default) in [("null", true), ("true", true), ("false", false)] {
            let settings = format!(r#""strict":{strict}"#);
            assert_eq!(
                strict_values(snapshot(&configuration(&settings), &[])),
                [default; 8],
                "strict={strict}"
            );
            for (index, flag) in FLAGS.iter().enumerate() {
                for (value, expected) in [("null", default), ("true", true), ("false", false)] {
                    let mut values = [default; 8];
                    values[index] = expected;
                    let mut settings = format!(r#""strict":{strict},"{flag}":{value}"#);
                    if index == 5 && expected && !default {
                        // Explicit property initialization requires null
                        // checking; TypeScript rejects the other combination.
                        settings.push_str(r#", "strictNullChecks":true"#);
                        values[2] = true;
                    }
                    assert_eq!(
                        strict_values(snapshot(&configuration(&settings), &[])),
                        values,
                        "{settings}"
                    );
                }
            }
        }
        assert_eq!(
            strict_values(snapshot(&configuration(r#""strict":null"#), &[])),
            [true; 8]
        );
        assert_eq!(
            strict_values(snapshot(
                &configuration(r#""stableTypeOrdering":null"#),
                &[]
            )),
            [true; 8]
        );
    }

    #[test]
    fn semantic_options_inherit_flags_before_effective_defaults() {
        for (index, flag) in FLAGS.iter().enumerate() {
            let base = format!(r#"{{"compilerOptions":{{"strict":false,"{flag}":false}}}}"#);
            for (setting, expected) in [
                ("", false),
                (",\"VALUE\":true", true),
                (",\"VALUE\":null", true),
            ] {
                let setting = setting.replace("VALUE", flag);
                let leaf = format!(
                    r#"{{"extends":"./base.json","compilerOptions":{{"noLib":true,"strict":true{setting}}},"files":["a.ts"]}}"#
                );
                let mut values = [true; 8];
                values[index] = expected;
                assert_eq!(
                    strict_values(snapshot(&leaf, &[("base.json", &base)])),
                    values,
                    "{flag}: {setting}"
                );
            }
        }
        let leaf = r#"{"extends":["./first.json","./second.json"],"compilerOptions":{"noLib":true},"files":["a.ts"]}"#;
        let first = r#"{"compilerOptions":{"strict":false,"noImplicitAny":true}}"#;
        let second = r#"{"compilerOptions":{"strict":true,"noImplicitAny":false}}"#;
        assert_eq!(
            strict_values(snapshot(
                leaf,
                &[("first.json", first), ("second.json", second)]
            )),
            [false, true, true, true, true, true, true, true]
        );
        let leaf = r#"{"extends":"./base.json","compilerOptions":{"noLib":true,"strict":null},"files":["a.ts"]}"#;
        assert_eq!(
            strict_values(snapshot(leaf, &[("base.json", first)])),
            [true; 8]
        );
    }

    #[test]
    fn semantic_options_non_strict_defaults_and_overrides() {
        for strict in ["null", "true", "false"] {
            for (value, expected) in [("null", false), ("true", true), ("false", false)] {
                let settings = format!(
                    r#""strict":{strict},"strictNullChecks":true,"exactOptionalPropertyTypes":{value},"noUncheckedIndexedAccess":{value},"noPropertyAccessFromIndexSignature":{value}"#
                );
                let options = snapshot(&configuration(&settings), &[]);
                assert_eq!(
                    options.exact_optional_property_types, expected,
                    "{settings}"
                );
                assert_eq!(options.no_unchecked_indexed_access, expected, "{settings}");
                assert_eq!(
                    options.no_property_access_from_index_signature, expected,
                    "{settings}"
                );
            }
        }
        let leaf = r#"{"extends":"./base.json","compilerOptions":{"noLib":true,"noUncheckedIndexedAccess":false,"exactOptionalPropertyTypes":null},"files":["a.ts"]}"#;
        let base = r#"{"compilerOptions":{"noUncheckedIndexedAccess":true,"exactOptionalPropertyTypes":true,"noPropertyAccessFromIndexSignature":true}}"#;
        let options = snapshot(leaf, &[("base.json", base)]);
        assert!(!options.exact_optional_property_types);
        assert!(!options.no_unchecked_indexed_access);
        assert!(options.no_property_access_from_index_signature);
    }

    #[test]
    fn semantic_options_stable_type_ordering_has_independent_true_default() {
        for value in ["null", "true", "false"] {
            let settings = format!(r#""strict":false,"stableTypeOrdering":{value}"#);
            assert_eq!(
                snapshot(&configuration(&settings), &[]).stable_type_ordering,
                value != "false"
            );
        }
        let base = r#"{"compilerOptions":{"stableTypeOrdering":false}}"#;
        for (setting, expected) in [
            ("", false),
            (",\"stableTypeOrdering\":true", true),
            (",\"stableTypeOrdering\":null", true),
        ] {
            let leaf = format!(
                r#"{{"extends":"./base.json","compilerOptions":{{"noLib":true{setting}}},"files":["a.ts"]}}"#
            );
            assert_eq!(
                snapshot(&leaf, &[("base.json", base)]).stable_type_ordering,
                expected
            );
        }
    }

    #[test]
    fn semantic_options_retain_demanded_deferred_inputs() {
        let options = snapshot(
            &configuration(
                r#""deduplicatePackages":false,"experimentalDecorators":true,"emitDecoratorMetadata":true,"checkJs":true,"allowJs":false,"jsxFactory":"h","jsxFragmentFactory":"Fragment","reactNamespace":"Custom""#,
            ),
            &[],
        );
        assert!(!options.deduplicate_packages);
        assert!(options.experimental_decorators);
        assert!(options.emit_decorator_metadata);
        assert_eq!(options.check_js, Some(true));
        assert!(!options.allow_js);
        assert_eq!(options.jsx_factory.as_deref(), Some("h"));
        assert_eq!(options.jsx_fragment_factory.as_deref(), Some("Fragment"));
        assert_eq!(options.react_namespace.as_deref(), Some("Custom"));
        let base = r#"{"compilerOptions":{"deduplicatePackages":false,"experimentalDecorators":true,"emitDecoratorMetadata":true,"jsxFactory":"h"}}"#;
        let leaf = r#"{"extends":"./base.json","compilerOptions":{"noLib":true,"experimentalDecorators":false,"jsxFactory":null},"files":["a.ts"]}"#;
        let options = snapshot(leaf, &[("base.json", base)]);
        assert!(!options.deduplicate_packages);
        assert!(!options.experimental_decorators);
        assert!(options.emit_decorator_metadata);
        assert!(options.jsx_factory.is_none());
    }

    #[test]
    fn semantic_options_resolve_distinct_class_field_operations() {
        for target in ["es2021", "es2022", "es2023", "esnext"] {
            for setting in ["null", "true", "false"] {
                let options = snapshot(
                    &configuration(&format!(
                        r#""target":"{target}","useDefineForClassFields":{setting}"#,
                    )),
                    &[],
                );
                let modern = target != "es2021";
                let expected = match setting {
                    "true" => true,
                    "false" => false,
                    _ => modern,
                };
                assert_eq!(
                    options.use_define_for_class_fields, expected,
                    "{target}: {setting}"
                );
                assert_eq!(
                    options.emit_standard_class_fields,
                    setting != "false" && modern,
                    "{target}: {setting}"
                );
            }
        }
    }

    /// TypeScript 5.6.3's `computedOptions` as the snapshot keeps them:
    /// `module: node16` alone targets ES2022 and `nodenext` ESNext, whose
    /// class fields use define semantics; `preserve` resolves as bundler;
    /// the strict family follows `strict`, false when it is unset, so
    /// `alwaysStrict` alone checks nothing strictly.
    #[test]
    fn typescript_5_6_computed_defaults_reach_the_snapshot() {
        use super::super::ModuleKind;
        use super::{Resolution, Target};
        let config = |options: &str| {
            format!(
                r#"{{"compilerOptions":{{"noLib":true,"types":[],{options}}},"files":["a.ts"]}}"#
            )
        };
        let five = |options: &str| snapshot_with(&config(options), &[], Some("5.6.3"));
        let node16 = five(r#""module":"node16","strict":true"#);
        assert_eq!(node16.target, Target::Es2022);
        assert_eq!(node16.module_kind, ModuleKind::Node16);
        assert_eq!(node16.module_resolution, Resolution::Node16);
        assert!(node16.use_define_for_class_fields);
        assert_eq!(strict_values(node16), [true; 8]);
        let nodenext = five(r#""module":"nodenext","strict":true"#);
        assert_eq!(nodenext.target, Target::EsNext);
        assert_eq!(nodenext.module_resolution, Resolution::NodeNext);
        let preserve = five(r#""module":"preserve","target":"es2017","strict":true"#);
        assert_eq!(preserve.module_kind, ModuleKind::Preserve);
        assert_eq!(preserve.module_resolution, Resolution::Bundler);
        assert!(!preserve.use_define_for_class_fields);
        let always = five(
            r#""module":"esnext","moduleResolution":"bundler","target":"es2022","esModuleInterop":true,"alwaysStrict":true"#,
        );
        assert_eq!(always.module_kind, ModuleKind::EsNext);
        assert_eq!(strict_values(always), [false; 8]);
        let mixed = five(r#""module":"node16","alwaysStrict":true,"strictNullChecks":true"#);
        assert_eq!(
            strict_values(mixed),
            [false, false, true, false, false, false, false, false]
        );
        // The same options under TypeScript 7: strict-family flags default on.
        let seven = snapshot_with(
            &config(
                r#""module":"esnext","moduleResolution":"bundler","target":"es2022","esModuleInterop":true,"alwaysStrict":true"#,
            ),
            &[],
            Some("7.0.2"),
        );
        assert_eq!(strict_values(seven), [true; 8]);
    }

    /// `checkJs` keeps three states through inheritance: unset, false, and
    /// true, an own `null` unsetting an inherited value.
    #[test]
    fn semantic_options_keep_check_js_unset_apart_from_false() {
        for (setting, expected) in [
            ("", None),
            (r#","checkJs":false"#, Some(false)),
            (r#","checkJs":true"#, Some(true)),
            (r#","checkJs":null"#, None),
        ] {
            let config = format!(
                r#"{{"compilerOptions":{{"noLib":true,"types":[]{setting}}},"files":["a.ts"]}}"#
            );
            assert_eq!(snapshot(&config, &[]).check_js, expected, "{setting}");
        }
        let base = r#"{"compilerOptions":{"checkJs":false}}"#;
        for (setting, expected) in [
            ("", Some(false)),
            (r#","checkJs":null"#, None),
            (r#","checkJs":true"#, Some(true)),
        ] {
            let leaf = format!(
                r#"{{"extends":"./base.json","compilerOptions":{{"noLib":true{setting}}},"files":["a.ts"]}}"#
            );
            assert_eq!(
                snapshot(&leaf, &[("base.json", base)]).check_js,
                expected,
                "inherited {setting}"
            );
        }
    }
}
