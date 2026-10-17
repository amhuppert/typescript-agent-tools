//! Immutable checker context independent of lexical index publication.
//!
//! Loading and requests borrow the same captured program inputs. This view
//! owns no checker cache, parser arena, navigation index, or filesystem access.

use crate::analysis::InputError;
use crate::project::Project;
use crate::project::binding::inputs::BindingFiles;
use crate::project::program::specifier_environment::SpecifierEnvironment;
use crate::project::program::{Program, SemanticOptions};
use crate::project::sources::Sources;
use std::path::Path;

/// Inputs to the shared initialization and semantic operation owners. The
/// file-facts view keeps Loading and Retained storage distinct without
/// requiring a fabricated completed Project during initialization.
#[derive(Clone, Copy)]
pub(super) struct CheckerInputs<'p> {
    pub(super) sources: &'p Sources,
    // A borrowed request recursion guard; absent during project loading.
    pub(super) control: Option<&'p crate::resources::Control>,
    #[allow(dead_code)] // Shared initialization retains configuration provenance.
    pub(super) config: &'p Path,
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
            config: &project.config,
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
            config: &program.config,
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
}
