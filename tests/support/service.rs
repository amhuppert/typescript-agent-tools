//! A temporary project and its CLI service in a private runtime directory,
//! which drop stops and removes.

use super::Fixture;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub struct Service {
    pub fixture: Fixture,
    pub runtime: PathBuf,
}

impl Service {
    /// A temporary `CONFIG` project with `files`.
    pub fn new(files: &[(&str, &str)]) -> Self {
        Self::of(super::project(files))
    }

    /// A service for `fixture`, not yet started.
    pub fn of(fixture: Fixture) -> Self {
        // Unix socket paths are short, so not under the temporary directory.
        let runtime = Path::new("/tmp").join(format!(
            "tsps-{}-{}",
            std::process::id(),
            fixture.root.file_name().unwrap().to_str().unwrap()
        ));
        Self { fixture, runtime }
    }

    /// `tsparse` in the project directory, using this service's runtime
    /// directory. Drop stops the service, so its idle timeout only has to
    /// outlast the test: a loaded machine must never stop a service between
    /// two requests a test expects one generation to answer.
    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tsparse"));
        command
            .current_dir(&self.fixture.root)
            .env("TSPARSE_SERVICE_DIR", &self.runtime)
            .env("TSPARSE_SERVICE_IDLE_SECS", "60")
            .stdin(Stdio::null());
        command
    }

    /// `args` for this project's `tsconfig.json`, from its directory.
    pub fn run(&self, args: &[&str]) -> Output {
        self.command()
            .args(args)
            .args(["--project", "tsconfig.json"])
            .output()
            .unwrap()
    }

    /// The groups of a reference name query, through the service.
    pub fn query(&self, name: &str) -> Value {
        json(self.run(&["--references", "--name", name]))
    }

    pub fn status(&self) -> Value {
        json(self.run(&["--references-status"]))
    }

    /// The number of generations the running service has published, or 0.
    pub fn generation(&self) -> u64 {
        self.status()["generation"].as_u64().unwrap()
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.run(&["--references-stop"]);
        let _ = std::fs::remove_dir_all(&self.runtime);
    }
}

/// The JSON document of a successful command that wrote nothing to stderr.
pub fn json(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
