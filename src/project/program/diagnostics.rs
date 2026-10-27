//! Whether a diagnostic at a place in a captured input is reported, the one
//! decision every load failure that TypeScript would report as a diagnostic
//! goes through. Search inclusion shares file provenance with it, but has its
//! own contract.

use crate::checking::{Code, FileChecking};
use crate::pragmas;
use crate::project::binding::FileFacts;
use crate::source::SourceText;

/// How a captured input entered the program. Search membership and the
/// reporting decision each read it under their own rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FileOrigin {
    /// A searched file: a root or a file it reaches without crossing into
    /// an external library.
    Project,
    /// Reached only through external libraries or other dependencies.
    Dependency,
    /// A standard library, vendored or replaced: TypeScript's
    /// `IsSourceFileDefaultLibrary`. Never searched and never external.
    DefaultLibrary,
}

/// One file's reporting decision (`reports`). Provenance is known only once
/// the whole dependency graph is discovered, so loading captures
/// TypeScript's part, `Checking`, first.
#[derive(Debug)]
pub(crate) struct Diagnostics {
    origin: FileOrigin,
    checking: Checking,
}

impl Diagnostics {
    /// `Program.IsSourceFileDefaultLibrary` uses captured load provenance,
    /// including library replacements, rather than a file-name convention.
    pub(in crate::project) fn is_default_library(&self) -> bool {
        self.origin == FileOrigin::DefaultLibrary
    }

    pub(super) fn new(origin: FileOrigin, checking: Checking) -> Self {
        Self { origin, checking }
    }

    #[cfg(test)]
    pub(super) fn origin(&self) -> FileOrigin {
        self.origin
    }

    /// Whether a binding or checker diagnostic with `code` at `offset` of
    /// this file fails the load: the one decision for every binding
    /// diagnostic (duplicate and conflicting declarations, merge checks,
    /// misplaced module declarations, UMD globals), every error TypeScript's
    /// checker reports from the file's syntax alone under the dialect
    /// (`crate::source_diagnostics::Policy`), and every missing module (an
    /// unresolved, untyped, or augmented module) (`getBindAndCheckDiagnostics`).
    /// Four terms decide it:
    ///
    /// - TypeScript's `SkipTypeChecking` for the file, under which it reports
    ///   neither binding nor checker diagnostics (`Checking`);
    /// - a plain JavaScript file's filter, which keeps the codes of
    ///   `plainJSErrors` alone (`crate::checking`), and which TypeScript
    ///   applies instead of comment directives;
    /// - otherwise a comment directive, `@ts-ignore` or `@ts-expect-error`,
    ///   that suppresses the line (`getDiagnosticsWithPrecedingDirectives`),
    ///   as `crate::pragmas` recognizes directives and the blank or
    ///   comment-only lines between a directive and the line it covers;
    /// - this tool's dependency leniency: only a project file's diagnostics
    ///   are reported. A dependency or standard library is bound as its own
    ///   declarations allow, whatever TypeScript would report there
    ///   (docs/references.md, Binding model).
    ///
    /// Where TypeScript reports one diagnostic at several declarations, it
    /// fails the load when any of them is reported.
    pub(crate) fn reports(&self, source: &SourceText, offset: usize, code: Code) -> bool {
        self.origin == FileOrigin::Project
            && !self.checking.skipped
            && if self.checking.file.plain_javascript() {
                self.checking.file.keeps(code)
            } else {
                !self.checking.suppressions.covers(source, offset)
            }
    }

    /// `reports` for one side of a merge conflict between declarations of
    /// two symbols: TypeScript's `reportMergeSymbolError` reports none at a
    /// plain JavaScript file's declarations, whatever the code.
    pub(crate) fn reports_merge_conflict(
        &self,
        source: &SourceText,
        offset: usize,
        code: Code,
    ) -> bool {
        !self.checking.file.plain_javascript() && self.reports(source, offset, code)
    }

    /// Whether a file-processing diagnostic at `offset` of this file, such as
    /// a missing reference directive target, fails the load: TypeScript's
    /// `GetIncludeProcessorDiagnostics`, which `SkipTypeChecking` and comment
    /// directives decide, with no plain JavaScript filter.
    pub(crate) fn reports_inclusion(&self, source: &SourceText, offset: usize) -> bool {
        self.origin == FileOrigin::Project
            && !self.checking.skipped
            && !self.checking.suppressions.covers(source, offset)
    }

    /// Whether an input outside the supported profile, such as a JavaScript
    /// reference without `allowJs`, that this file reaches is skipped: a
    /// product limit, not a diagnostic, tolerated only beyond the project's
    /// own files.
    pub(crate) fn tolerates_unsupported_input(&self) -> bool {
        self.origin != FileOrigin::Project
    }

    /// Whether a syntactic diagnostic of a whole tree, such as an error of a
    /// JavaScript file's TypeScript-only syntax, fails the load: TypeScript
    /// reports syntactic diagnostics whatever the file's checking
    /// (`GetSyntacticDiagnostics`), and this tool only a project file's.
    pub(crate) fn reports_syntax(&self) -> bool {
        self.origin == FileOrigin::Project
    }

    /// Where the first of a JavaScript file's parameter decorators that
    /// fails the load is reported, of `candidates` in source order, which
    /// TypeScript rejects without `experimentalDecorators` (TS1206): as the
    /// dialect's `policy` reports it where the checker does or does not
    /// check the file (`checkJs`, `// @ts-check`). A syntactic diagnostic is
    /// the first candidate's, and the checker's the first one that comment
    /// directives and `SkipTypeChecking` leave (`reports`).
    pub(crate) fn reported_parameter_decorator(
        &self,
        source: &SourceText,
        candidates: &[crate::javascript_syntax::ParameterDecorator],
        policy: crate::source_diagnostics::Policy,
    ) -> Option<ReportedDecorator> {
        use crate::source_diagnostics::DecoratorReport;
        match policy.parameter_decorator(self.checking.file.check_js_enabled()) {
            DecoratorReport::Syntactic { full_start } => candidates
                .first()
                .filter(|_| self.reports_syntax())
                .map(|first| ReportedDecorator {
                    offset: if full_start {
                        first.full_start
                    } else {
                        first.token_start
                    },
                    syntactic: true,
                }),
            DecoratorReport::Checker => candidates
                .iter()
                .map(|candidate| candidate.token_start)
                .find(|&offset| self.reports(source, offset, Code::DECORATORS_NOT_VALID))
                .map(|offset| ReportedDecorator {
                    offset,
                    syntactic: false,
                }),
        }
    }
}

/// Where a parameter decorator's TS1206 fails the load, and whether as a
/// syntactic diagnostic, which sorts among the file's others, or as the
/// checker's, which comes after every syntactic one.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReportedDecorator {
    pub(crate) offset: usize,
    pub(crate) syntactic: bool,
}

/// TypeScript's part of the reporting decision for one file, known when the
/// file is captured: whether `SkipTypeChecking` holds, the file's own
/// checking (`crate::checking`), and the lines comment directives suppress.
#[derive(Debug, Clone)]
pub(super) struct Checking {
    skipped: bool,
    file: FileChecking,
    suppressions: Suppressions,
}

/// The options and file facts `SkipTypeChecking` reads.
pub(super) struct Unchecked {
    /// `noCheck`.
    pub(super) no_check: bool,
    pub(super) declaration: bool,
    pub(super) skip_lib_check: bool,
    pub(super) library: bool,
    pub(super) skip_default_lib_check: bool,
    /// A JSON source file, whose diagnostics
    /// `canIncludeBindAndCheckDiagnostics` never includes.
    pub(super) json: bool,
    /// What else decides `canIncludeBindAndCheckDiagnostics`: the file's
    /// kind, `checkJs`, and its `// @ts-check` or `// @ts-nocheck` pragma.
    pub(super) file: FileChecking,
}

impl Checking {
    /// TypeScript's `SkipTypeChecking`: `noCheck`, a declaration file under
    /// `skipLibCheck`, a standard library under `skipDefaultLibCheck`, or a
    /// file `canIncludeBindAndCheckDiagnostics` excludes (JSON, a file under
    /// `// @ts-nocheck`, or a JavaScript file `checkJs: false` leaves
    /// unchecked). Project references, its other term, are not followed
    /// (docs/references.md).
    pub(super) fn new(unchecked: Unchecked, source: &SourceText, facts: &FileFacts) -> Self {
        Self {
            skipped: unchecked.no_check
                || unchecked.declaration && unchecked.skip_lib_check
                || unchecked.library && unchecked.skip_default_lib_check
                || unchecked.json
                || !unchecked.file.includes_bind_and_check(),
            file: unchecked.file,
            suppressions: Suppressions::new(source, facts),
        }
    }
}

/// The lines whose errors comment directives suppress, as
/// `pragmas::DirectiveCoverage` decides for the file's directives.
#[derive(Debug, Default, Clone)]
struct Suppressions(pragmas::DirectiveCoverage);

impl Suppressions {
    fn new(source: &SourceText, facts: &FileFacts) -> Self {
        Self(pragmas::DirectiveCoverage::new(
            source,
            facts
                .comment_directives()
                .iter()
                .map(|&start| start as usize),
        ))
    }

    /// Whether errors at `offset` are suppressed.
    fn covers(&self, source: &SourceText, offset: usize) -> bool {
        self.0.covers_line(source.line_index(offset))
    }
}
