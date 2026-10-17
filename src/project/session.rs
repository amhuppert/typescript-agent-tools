//! Refreshable project ownership for long-lived callers.

use super::{Project, ProjectRequest, ProjectResponse, program::disk};
use crate::analysis::{AnalysisError, InputError, InputErrorKind};
use std::path::{Path, PathBuf};

/// A project's current generation. Every request refreshes changed disk
/// inputs before answering, through one boundary (`execute`); failed
/// refreshes never answer from an earlier generation.
pub struct ProjectSession {
    config: PathBuf,
    generation: u64,
    current: Option<(Project, disk::Observations)>,
}

impl ProjectSession {
    /// Fix the configuration path against the current directory. Construction
    /// does not read the project; the first request loads it.
    pub fn new(config: &Path) -> Result<Self, InputError> {
        Ok(Self {
            config: std::path::absolute(config).map_err(|error| InputError {
                kind: InputErrorKind::Io,
                message: error.to_string(),
                path: Some(config.to_path_buf()),
                position: None,
            })?,
            generation: 0,
            current: None,
        })
    }

    /// The number of successfully published snapshots, initially zero.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Validate the captured filesystem inputs and load a fresh generation
    /// when necessary, then answer `request` from it (`Project::execute`).
    /// Relative request paths use the current directory. Inputs changing
    /// during construction produce an error, not a partial snapshot.
    pub fn execute(&mut self, request: ProjectRequest) -> Result<ProjectResponse, AnalysisError> {
        self.refresh()?;
        self.current
            .as_ref()
            .expect("refresh publishes a complete generation")
            .0
            .execute(request)
    }

    fn refresh(&mut self) -> Result<(), AnalysisError> {
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
        // Release the obsolete snapshot before loading: large projects already
        // have a much higher construction peak than their retained state.
        self.current = None;
        let captured = Project::on_loading_thread(&self.config, |parser| {
            let disk = disk::Disk::recording();
            let project = Project::load_here(&self.config, &disk, parser)?;
            let observations = disk.finish();
            if !observations.unchanged()? {
                return Err(InputError {
                    kind: InputErrorKind::Io,
                    message:
                        "Project inputs changed during indexing; retry when writes have completed"
                            .into(),
                    path: Some(self.config.clone()),
                    position: None,
                }
                .into());
            }
            Ok((project, observations))
        })?;
        self.current = Some(captured);
        self.generation += 1;
        Ok(())
    }
}
