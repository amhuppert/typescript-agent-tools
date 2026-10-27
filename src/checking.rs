//! Which of a file's binding and checker diagnostics TypeScript reports, by
//! the file's kind, `checkJs`, and the file's `// @ts-check` or
//! `// @ts-nocheck` pragma (`crate::pragmas::checks`): TypeScript 7's
//! `canIncludeBindAndCheckDiagnostics`, `IsPlainJSFile`,
//! `IsCheckJSEnabledForFile`, and the `plainJSErrors` filter of
//! `getBindAndCheckDiagnosticsWithChecker` (internal/compiler/program.go,
//! internal/ast/utilities.go). TypeScript 5.6.3's counterparts decide
//! alike, with the same codes. A diagnostic that loading reports names its
//! code (`Code`), which this policy reads.

/// A TypeScript diagnostic code, which a load failure carries to the
/// reporting decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Code(pub(crate) u32);

impl Code {
    /// TS2300, `Duplicate identifier '{0}'.`
    pub(crate) const DUPLICATE_IDENTIFIER: Self = Self(2300);
    /// TS2451, `Cannot redeclare block-scoped variable '{0}'.`
    pub(crate) const REDECLARED_BLOCK_SCOPED: Self = Self(2451);
    /// TS2528, `A module cannot have multiple default exports.`
    pub(crate) const MULTIPLE_DEFAULT_EXPORTS: Self = Self(2528);
    /// TS2567, `Enum declarations can only merge with namespace or other
    /// enum declarations.`
    pub(crate) const ENUM_MERGE: Self = Self(2567);
    /// TS2307, `Cannot find module '{0}' or its corresponding type
    /// declarations.`
    pub(crate) const MODULE_NOT_FOUND: Self = Self(2307);
    /// TS7016, an import of an untyped module under `noImplicitAny`.
    pub(crate) const UNTYPED_MODULE: Self = Self(7016);
    /// TS2665, an augmentation of an untyped module.
    pub(crate) const UNTYPED_AUGMENTATION: Self = Self(2665);
    /// TS2664, an augmentation whose module cannot be found.
    pub(crate) const AUGMENTATION_NOT_FOUND: Self = Self(2664);
    /// TS2306, `File '{0}' is not a module.`
    pub(crate) const NOT_A_MODULE: Self = Self(2306);
    /// TS2671, an augmentation of a module that resolves to a non-module
    /// entity.
    pub(crate) const AUGMENTATION_NON_MODULE: Self = Self(2671);
    /// TS2649, an augmentation with value exports of a module that resolves
    /// to a non-module entity.
    pub(crate) const AUGMENTATION_VALUES: Self = Self(2649);
    /// TS2395, declarations of one name both exported and local.
    pub(crate) const MIXED_EXPORTS: Self = Self(2395);
    /// TS2481, a `var` hoisted past a block-scoped declaration.
    pub(crate) const HOISTED_PAST_BLOCK_SCOPED: Self = Self(2481);
    /// TS2434, a namespace before the class or function it merges with (or,
    /// TS2433, in another file).
    pub(crate) const NAMESPACE_BEFORE_MERGE: Self = Self(2434);
    /// TS2440, an import that conflicts with a local declaration.
    pub(crate) const IMPORT_CONFLICT: Self = Self(2440);
    /// TS1540, a namespace declared with the `module` keyword.
    pub(crate) const MODULE_KEYWORD: Self = Self(1540);
    /// TS1035, a quoted name outside an ambient module.
    pub(crate) const QUOTED_MODULE_NAME: Self = Self(1035);
    /// TS2669, a misplaced global augmentation.
    pub(crate) const MISPLACED_GLOBAL: Self = Self(2669);
    /// TS2435, an ambient module nested in another module or namespace.
    pub(crate) const NESTED_AMBIENT_MODULE: Self = Self(2435);
    /// TS2436, an ambient module with a relative name.
    pub(crate) const RELATIVE_AMBIENT_MODULE: Self = Self(2436);
    /// TS1314, a UMD global in a script.
    pub(crate) const UMD_GLOBAL_IN_SCRIPT: Self = Self(1314);
    /// TS1315, a UMD global outside a declaration file.
    pub(crate) const UMD_GLOBAL_IN_IMPLEMENTATION: Self = Self(1315);
    /// TS1316, a UMD global below the top level.
    pub(crate) const NESTED_UMD_GLOBAL: Self = Self(1316);
    /// TS1206, `Decorators are not valid here.`
    pub(crate) const DECORATORS_NOT_VALID: Self = Self(1206);
    /// TS6142, a `.tsx` or `.jsx` module without `jsx`.
    pub(crate) const JSX_NOT_SET: Self = Self(6142);
    /// TS7042, a JSON module without `resolveJsonModule`.
    pub(crate) const RESOLVE_JSON_MODULE: Self = Self(7042);
    /// TS6263, an arbitrary-extension declaration without
    /// `allowArbitraryExtensions`.
    pub(crate) const ARBITRARY_EXTENSIONS: Self = Self(6263);
}

/// TypeScript 7.0.2's `plainJSErrors`: the binding, grammar, and type errors
/// a plain JavaScript file keeps, which 5.6.3 lists too.
const PLAIN_JS_ERRORS: [u32; 91] = [
    2451, 2528, 2753, 2752, 1262, 1214, 1359, 18012, 1102, 1210, 1215, 1100, 1344, 1101, 1105,
    1116, 1211, 1248, 1171, 1104, 1115, 1113, 1258, 1255, 1182, 1054, 2501, 2566, 1186, 2462, 1048,
    1014, 1013, 18041, 1053, 1049, 1474, 1193, 1473, 1191, 1162, 1325, 2803, 2492, 1197, 18036,
    1174, 18006, 1312, 1114, 1450, 18038, 17000, 17001, 18007, 2633, 1107, 1200, 1184, 1091, 1188,
    18016, 1451, 18013, 1358, 1106, 1189, 1190, 1009, 1123, 5076, 1005, 17012, 1097, 1030, 1089,
    1044, 1090, 1031, 1042, 1029, 1156, 1155, 1172, 2480, 1368, 1341, 1308, 2852, 1111, 2839,
];

/// What decides which of one file's binding and checker diagnostics
/// TypeScript reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileChecking {
    /// A JavaScript file, by its script kind.
    javascript: bool,
    /// Its `CheckJsDirective`: `// @ts-check` (`true`) or `// @ts-nocheck`
    /// (`false`), the last one winning.
    directive: Option<bool>,
    /// `checkJs` as configured: unset is not `false`.
    check_js: Option<bool>,
}

impl FileChecking {
    /// A file's checking inputs: its kind, its `CheckJsDirective` as
    /// `processPragmasIntoFields` reads it from `text`, and `checkJs`.
    pub(crate) fn new(javascript: bool, text: &str, check_js: Option<bool>) -> Self {
        Self {
            javascript,
            directive: crate::pragmas::checks(text),
            check_js,
        }
    }

    /// `IsCheckJSEnabledForFile`: the file's pragma, else `checkJs` true.
    pub(crate) fn check_js_enabled(self) -> bool {
        self.directive.unwrap_or(self.check_js == Some(true))
    }

    /// `IsPlainJSFile`: a JavaScript file without a pragma while `checkJs` is
    /// unset, whose diagnostics TypeScript filters by code.
    pub(crate) fn plain_javascript(self) -> bool {
        self.javascript && self.directive.is_none() && self.check_js.is_none()
    }

    /// `canIncludeBindAndCheckDiagnostics` for a file that is not JSON: not
    /// under `// @ts-nocheck`, and a TypeScript file, a plain JavaScript
    /// file, or a JavaScript file `checkJs` or `// @ts-check` checks.
    pub(crate) fn includes_bind_and_check(self) -> bool {
        if self.directive == Some(false) {
            return false;
        }
        !self.javascript || self.plain_javascript() || self.check_js_enabled()
    }

    /// Whether a binding or checker diagnostic with `code` survives the
    /// file's filter: every one, except in a plain JavaScript file, which
    /// keeps `plainJSErrors` alone.
    pub(crate) fn keeps(self, code: Code) -> bool {
        !self.plain_javascript() || PLAIN_JS_ERRORS.contains(&code.0)
    }
}
