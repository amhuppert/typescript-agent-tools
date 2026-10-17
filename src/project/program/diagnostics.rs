//! Whether a diagnostic at a place in a captured input is reported, the one
//! decision every load failure that TypeScript would report as a diagnostic
//! goes through. Search inclusion shares file provenance with it, but has its
//! own contract.

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

    /// Whether a diagnostic at `offset` of this file fails the load: the one
    /// decision for every binding diagnostic (duplicate and conflicting
    /// declarations, merge checks, misplaced module declarations, UMD
    /// globals) and every missing input (an unresolved module, reference
    /// directive target, or augmented module). Three terms decide it:
    ///
    /// - TypeScript's `SkipTypeChecking` for the file, under which it reports
    ///   neither binding nor checker diagnostics (`Checking`);
    /// - a comment directive, `@ts-ignore` or `@ts-expect-error`, that
    ///   suppresses the line (`getDiagnosticsWithPrecedingDirectives`), as
    ///   `crate::pragmas` recognizes directives and the blank or
    ///   comment-only lines between a directive and the line it covers;
    /// - this tool's dependency leniency: only a project file's diagnostics
    ///   are reported. A dependency or standard library is bound as its own
    ///   declarations allow, whatever TypeScript would report there
    ///   (docs/references.md, Binding model).
    ///
    /// Where TypeScript reports one diagnostic at several declarations, it
    /// fails the load when any of them is reported.
    pub(crate) fn reports(&self, source: &SourceText, offset: usize) -> bool {
        self.origin == FileOrigin::Project
            && !self.checking.skipped
            && !self.checking.suppressions.covers(source, offset)
    }

    /// Whether an input outside the supported profile, such as JavaScript,
    /// that this file reaches is skipped: a product limit, not a diagnostic,
    /// tolerated only beyond the project's own files.
    pub(crate) fn tolerates_unsupported_input(&self) -> bool {
        self.origin != FileOrigin::Project
    }
}

/// TypeScript's part of the reporting decision for one file, known when the
/// file is captured: whether `SkipTypeChecking` holds, and the lines comment
/// directives suppress.
#[derive(Debug)]
pub(super) struct Checking {
    skipped: bool,
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
    /// The file's `// @ts-nocheck` pragma (`crate::pragmas::checks`), which
    /// excludes it from `canIncludeBindAndCheckDiagnostics`.
    pub(super) nocheck: bool,
}

impl Checking {
    /// TypeScript's `SkipTypeChecking`: `noCheck`, a declaration file under
    /// `skipLibCheck`, a standard library under `skipDefaultLibCheck`, or a
    /// file `canIncludeBindAndCheckDiagnostics` excludes (JSON, or
    /// `// @ts-nocheck`). Project references, its other term, are not
    /// followed (docs/references.md).
    pub(super) fn new(unchecked: Unchecked, source: &SourceText, facts: &FileFacts) -> Self {
        Self {
            skipped: unchecked.no_check
                || unchecked.declaration && unchecked.skip_lib_check
                || unchecked.library && unchecked.skip_default_lib_check
                || unchecked.json
                || unchecked.nocheck,
            suppressions: Suppressions::new(source, facts),
        }
    }
}

/// The lines whose errors comment directives suppress, as
/// `pragmas::DirectiveCoverage` decides for the file's directives.
#[derive(Debug, Default)]
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
