//! Failure tolerance for captured program inputs. Search inclusion and
//! diagnostics share file provenance, but each has its own contract.

use crate::references::binding::FileFacts;
use crate::references::source::SourceText;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FileOrigin {
    Project,
    Dependency,
}

/// One file's failure tolerance. Provenance is known only once the whole
/// dependency graph is discovered, so loading uses `MissingInputs` alone.
#[derive(Debug)]
pub(crate) struct Diagnostics {
    origin: FileOrigin,
    missing: MissingInputs,
}

impl Diagnostics {
    pub(super) fn new(origin: FileOrigin, missing: MissingInputs) -> Self {
        Self { origin, missing }
    }

    /// Dependency-only declarations retain the existing lenient binding
    /// profile, independently of skipLibCheck and occurrence inclusion.
    pub(crate) fn checks_bindings(&self) -> bool {
        self.origin == FileOrigin::Project
    }

    pub(crate) fn tolerates_missing_input(&self, source: &SourceText, offset: usize) -> bool {
        self.missing.tolerated(source, offset)
    }

    pub(crate) fn tolerates_unsupported_input(&self) -> bool {
        self.origin == FileOrigin::Dependency
    }
}

/// Where a missing import or reference directive target is tolerated, which
/// does not depend on provenance: in a declaration file under skipLibCheck,
/// or on a line a comment directive suppresses. Dependency provenance alone
/// never suppresses a missing input.
#[derive(Debug)]
pub(super) struct MissingInputs {
    unchecked_declaration: bool,
    suppressions: Suppressions,
}

impl MissingInputs {
    pub(super) fn new(
        declaration: bool,
        skip_lib_check: bool,
        source: &SourceText,
        facts: &FileFacts,
    ) -> Self {
        Self {
            unchecked_declaration: declaration && skip_lib_check,
            suppressions: Suppressions::new(source, facts),
        }
    }

    pub(super) fn tolerated(&self, source: &SourceText, offset: usize) -> bool {
        self.unchecked_declaration || self.suppressions.covers(source, offset)
    }
}

/// The lines whose errors `@ts-ignore` and `@ts-expect-error` comments
/// suppress, by TypeScript's rule: a directive comment ends on a preceding
/// line, and only blank or line-comment lines lie between.
#[derive(Debug, Default)]
struct Suppressions {
    /// Indexed by zero-based line; empty without directives.
    covered: Vec<bool>,
}

impl Suppressions {
    fn new(source: &SourceText, facts: &FileFacts) -> Self {
        let directives = facts.comment_directives();
        if directives.is_empty() {
            return Self::default();
        }
        let mut directive = vec![false; source.line_count()];
        for &end in directives {
            directive[source.line_index(end as usize)] = true;
        }
        let mut covered = vec![false; directive.len()];
        // Zero-based line indexes; `SourceText::line` takes one-based numbers.
        for line in 1..covered.len() {
            let above = line - 1;
            covered[line] = directive[above]
                || covered[above] && {
                    let text = source.line(above + 1).unwrap_or("").trim();
                    text.is_empty() || text.starts_with("//")
                };
        }
        Self { covered }
    }

    /// Whether errors at `offset` are suppressed.
    fn covers(&self, source: &SourceText, offset: usize) -> bool {
        !self.covered.is_empty() && self.covered[source.line_index(offset)]
    }
}
