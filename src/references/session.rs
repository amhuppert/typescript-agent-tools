//! Refreshable project ownership for long-lived reference callers.

use super::{ProjectError, QueryError, ReferenceProject, ReferenceQuery, ReferenceResult};
use std::path::{Path, PathBuf};

/// A project's current reference generation. Queries refresh changed disk inputs
/// before answering; failed refreshes never answer from an earlier generation.
pub struct ReferenceSession {
    config: PathBuf,
    generation: u64,
    current: Option<(ReferenceProject, super::project::disk::Observations)>,
}

impl ReferenceSession {
    /// Fix the configuration path against the current directory. Construction
    /// does not read the project; the first query loads it.
    pub fn new(config: &Path) -> Result<Self, ProjectError> {
        Ok(Self {
            config: std::path::absolute(config).map_err(|error| ProjectError {
                kind: super::ProjectErrorKind::Io,
                message: error.to_string(),
                path: Some(config.to_path_buf()),
                position: None,
            })?,
            generation: 0,
            current: None,
        })
    }

    /// The number of successfully published indexes, initially zero.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Validate the captured filesystem inputs and load a fresh generation when
    /// necessary, then query it. Relative query paths use the current directory.
    /// Inputs changing during construction produce an error, not a partial index.
    pub fn find_references(
        &mut self,
        query: ReferenceQuery,
    ) -> Result<ReferenceResult, ReferenceError> {
        self.refresh().map_err(ReferenceError::Project)?;
        self.current
            .as_ref()
            .expect("refresh publishes a complete generation")
            .0
            .find_references(query)
            .map_err(ReferenceError::Query)
    }

    fn refresh(&mut self) -> Result<(), ProjectError> {
        if let Some((_, observations)) = &self.current {
            match observations.unchanged() {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(_previous_input_error) => {
                    // This path belonged to the previous program. Current
                    // configuration/imports may no longer select it. Discard the
                    // old generation and let a full load decide which inputs are
                    // required and report their failures. This never licenses a
                    // cached answer or treats the unreadable path as absent.
                }
            }
        }
        // Release the obsolete index before loading: large projects already have
        // a much higher construction peak than their retained query state.
        self.current = None;
        let captured = ReferenceProject::on_loading_thread(&self.config, || {
            let disk = super::project::disk::Disk::recording();
            let project = ReferenceProject::load_here(&self.config, &disk)?;
            let observations = disk.finish();
            if !observations.unchanged()? {
                return Err(ProjectError {
                    kind: super::ProjectErrorKind::Io,
                    message:
                        "Project inputs changed during indexing; retry when writes have completed"
                            .into(),
                    path: Some(self.config.clone()),
                    position: None,
                });
            }
            Ok((project, observations))
        })?;
        self.current = Some(captured);
        self.generation += 1;
        Ok(())
    }
}

/// Project refresh and query failures retain their distinct kinds and locations.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ReferenceError {
    Project(ProjectError),
    Query(QueryError),
}

impl std::fmt::Display for ReferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project(error) => error.fmt(f),
            Self::Query(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ReferenceError {}
