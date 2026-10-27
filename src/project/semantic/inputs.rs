//! Immutable checker context independent of lexical index publication.
//!
//! Loading and requests borrow the same captured program inputs. This view
//! owns no checker cache, parser arena, navigation index, or filesystem access.

use crate::analysis::InputError;
use crate::project::Project;
use crate::project::binding::inputs::BindingFiles;
use crate::project::program::specifier_environment::SpecifierEnvironment;
use crate::project::program::{Program, SemanticOptions};
use crate::project::sources::{FileId, Sources};

/// Inputs to the shared initialization and semantic operation owners. The
/// file-facts view keeps Loading and Retained storage distinct without
/// requiring a fabricated completed Project during initialization.
#[derive(Clone, Copy)]
pub(super) struct CheckerInputs<'p> {
    pub(super) sources: &'p Sources,
    // A borrowed request recursion guard; absent during project loading.
    pub(super) control: Option<&'p crate::resources::Control>,
    pub(super) bindings: BindingFiles<'p>,
    pub(super) raw_bindings: &'p crate::project::binding::raw::RawBindingSnapshot,
    options: &'p SemanticOptions,
    specifiers: &'p Result<SpecifierEnvironment, InputError>,
}

impl<'p> CheckerInputs<'p> {
    pub(super) fn from_project(
        project: &'p Project,
        control: Option<&'p crate::resources::Control>,
    ) -> Self {
        Self {
            sources: &project.sources,
            control,
            bindings: BindingFiles::Retained(&project.bindings),
            raw_bindings: project.bindings.raw_binding_snapshot(),
            options: project.semantic_options(),
            specifiers: project.specifier_environment(),
        }
    }

    /// The raw snapshot does not borrow the mutable lexical Store; no missing
    /// index or reference attachment is represented as an empty one.
    pub(super) fn during_loading(
        program: &'p Program,
        raw_bindings: &'p crate::project::binding::raw::RawBindingSnapshot,
    ) -> Self {
        Self {
            sources: &program.sources,
            control: None,
            bindings: BindingFiles::Loading(program),
            raw_bindings,
            options: &program.semantic,
            specifiers: &program.specifier_environment,
        }
    }

    pub(super) fn semantic_options(&self) -> &'p SemanticOptions {
        self.options
    }

    pub(super) fn specifier_environment(&self) -> &'p Result<SpecifierEnvironment, InputError> {
        self.specifiers
    }

    /// TypeScript's `IsInJSFile` for any node of `file`: the file is
    /// JavaScript by its script kind, as the parser marks each of its nodes
    /// (`ParseOptions::javascript_file`).
    pub(super) fn in_javascript_file(&self, file: FileId) -> bool {
        self.sources
            .get(file)
            .path
            .to_str()
            .is_some_and(|path| crate::ParseOptions::for_file(path).javascript_file())
    }

    /// What decides which diagnostics TypeScript reports for `file`
    /// (`crate::checking`), which the checker asks where a JavaScript file's
    /// checking changes an answer (`isUncheckedJSSuggestion`).
    pub(super) fn file_checking(&self, file: FileId) -> crate::checking::FileChecking {
        crate::checking::FileChecking::new(
            self.in_javascript_file(file),
            self.sources.get(file).text.as_str(),
            self.options.check_js,
        )
    }
}
