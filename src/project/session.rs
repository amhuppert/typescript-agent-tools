//! Refreshable project ownership for long-lived callers.

use super::{Project, ProjectRequest, ProjectResponse, program::disk};
use crate::analysis::{AnalysisError, InputError, InputErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A project's current generation. Every request refreshes changed disk
/// inputs before answering, through one boundary (`execute`); failed
/// refreshes never answer from an earlier generation.
pub struct ProjectSession {
    config: PathBuf,
    generation: u64,
    current: Option<(Project, disk::Observations)>,
    reuse: Option<super::program::Reuse>,
    #[cfg(test)]
    tests: tests::Hooks,
}

type Answer = Result<ProjectResponse, AnalysisError>;

#[cfg(test)]
mod reuse_tests;

/// The answer a thread `answering` started gives, resuming its panic; without
/// a thread of its own the answer is made here.
fn answered(
    answering: std::io::Result<std::thread::ScopedJoinHandle<'_, Answer>>,
    project: &Project,
    request: &ProjectRequest,
) -> Answer {
    match answering {
        Ok(answering) => answering
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
        Err(_) => project.execute(request.clone()),
    }
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
            reuse: Some(super::program::Reuse::default()),
            #[cfg(test)]
            tests: tests::Hooks::default(),
        })
    }

    /// A session for a caller that answers one request and exits. It performs
    /// the same input validation, but retains no file products or resolution
    /// decisions for a future refresh. Repeated use remains correct.
    pub fn for_one_request(config: &Path) -> Result<Self, InputError> {
        let mut session = Self::new(config)?;
        session.reuse = None;
        Ok(session)
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
        if let Some(answer) = self.answer_if_current(&request) {
            return answer;
        }
        self.reload()?;
        let (project, _) = self
            .current
            .as_ref()
            .expect("reloading publishes a complete generation");
        std::thread::scope(|scope| {
            answered(
                self.answering(scope, project, &request, None),
                project,
                &request,
            )
        })
    }

    /// Start answering `request` from `project` on its own analysis thread, the
    /// thread every request is answered on (`answered`).
    fn answering<'scope>(
        &self,
        scope: &'scope std::thread::Scope<'scope, '_>,
        project: &'scope Project,
        request: &'scope ProjectRequest,
        superseded: Option<&'scope Arc<AtomicBool>>,
    ) -> std::io::Result<std::thread::ScopedJoinHandle<'scope, Answer>> {
        #[cfg(test)]
        let fails = superseded.is_some() && self.tests.panic_answering == Some(self.generation);
        #[cfg(test)]
        let once_stopped = self
            .tests
            .answer_once_stopped
            .filter(|_| superseded.is_some());
        crate::analysis::spawn_parsing_thread(scope, "tsparse-answer", move |_| {
            #[cfg(test)]
            if fails {
                panic!("a test's prepared answer panics");
            }
            #[cfg(test)]
            if let (Some(limit), Some(superseded)) = (once_stopped, superseded) {
                let started = std::time::Instant::now();
                while !superseded.load(Ordering::SeqCst) && started.elapsed() < limit {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
            project.execute_unless(request.clone(), superseded)
        })
    }

    /// The current generation's answer to `request`, if its inputs are
    /// unchanged. The answer is prepared while the inputs are validated; an
    /// answer from a generation that validation does not confirm is stopped
    /// as soon as some input differs, and discarded, never delivered.
    fn answer_if_current(
        &self,
        request: &ProjectRequest,
    ) -> Option<Result<ProjectResponse, AnalysisError>> {
        let (project, observations) = self.current.as_ref()?;
        let superseded = Arc::new(AtomicBool::new(false));
        std::thread::scope(|scope| {
            let speculative = self.answering(scope, project, request, Some(&superseded));
            // An unreadable input belonged to the previous program. Current
            // configuration/imports may no longer select it, so it means a
            // reload, which decides which inputs are required and reports
            // their failures; it never licenses an answer or makes the
            // unreadable path absent.
            let stale = || superseded.store(true, Ordering::Relaxed);
            #[cfg(test)]
            if self.tests.stale_while_validating {
                stale();
            }
            let current = matches!(observations.unchanged_with(&stale), Ok(true));
            if !current {
                // The stopped answer, a panic included, is for nobody: an
                // ordinary reload never prepared it.
                if let Ok(answering) = speculative {
                    let _ = answering.join();
                }
                return None;
            }
            if superseded.load(Ordering::Relaxed) {
                // Validation reported the generation stale and still
                // confirmed it: the generation is current, but its answer
                // was stopped.
                if let Ok(answering) = speculative {
                    let _ = answering.join();
                }
                return Some(answered(
                    self.answering(scope, project, request, None),
                    project,
                    request,
                ));
            }
            Some(answered(speculative, project, request))
        })
    }

    fn reload(&mut self) -> Result<(), AnalysisError> {
        // Release the obsolete project before loading. Reusable file products
        // survive capture; load_reusing releases the unused ones before binding.
        self.current = None;
        let (project, observations, reuse) = Project::on_loading_thread(&self.config, |parser| {
            let disk = disk::Disk::recording();
            let (project, reuse) = match self.reuse.as_mut() {
                Some(previous) => {
                    let (project, reuse) = Project::load_reusing(
                        &self.config,
                        &disk,
                        parser,
                        previous,
                        super::Release::Background,
                    )?;
                    (project, Some(reuse))
                }
                None => (
                    Project::load_here(&self.config, &disk, parser, super::Release::Background)?,
                    None,
                ),
            };
            #[cfg(test)]
            if let Some(while_loading) = &self.tests.while_loading {
                while_loading();
            }
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
            Ok((project, observations, reuse))
        })?;
        self.current = Some((project, observations));
        self.reuse = reuse;
        self.generation += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Project, ProjectRequest, ProjectResponse};
    use super::ProjectSession;
    use crate::references::ReferenceQuery;

    /// Faults a test makes a session commit.
    #[derive(Default)]
    pub(super) struct Hooks {
        /// The generation whose answers, prepared while validating, panic.
        pub(super) panic_answering: Option<u64>,
        /// Answers prepared while validating begin only once stopped, for at
        /// most this long.
        pub(super) answer_once_stopped: Option<std::time::Duration>,
        /// Validation reports the generation stale, as it does on finding an
        /// observation that differs, and goes on to confirm it.
        pub(super) stale_while_validating: bool,
        /// Runs once loading has read every input, before the check after
        /// loading repeats its observations.
        pub(super) while_loading: Option<Box<dyn Fn() + Send + Sync>>,
    }

    /// A temporary directory, by its real path, with `files`.
    fn directory(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!("tsparse-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let directory = std::fs::canonicalize(&directory).unwrap();
        for (path, text) in files {
            std::fs::write(directory.join(path), text).unwrap();
        }
        directory
    }

    /// An answer prepared while validating that was stopped is never
    /// delivered, also when validation confirms the generation: the session
    /// answers again. Validation in one pass reports staleness only for a
    /// generation it does not confirm (`Observations::unchanged_with`), so a
    /// hook reports it here.
    #[test]
    fn a_stopped_answer_from_a_current_generation_is_made_again() {
        let directory = directory(
            "answer-again",
            &[
                ("tsconfig.json", r#"{"compilerOptions":{"noLib":true}}"#),
                ("z.ts", "export const value = 1;\n"),
            ],
        );
        let source = directory.join("z.ts");
        let mut session = ProjectSession::new(&directory.join("tsconfig.json")).unwrap();
        let summary = |session: &mut ProjectSession| match session.execute(ProjectRequest::Type(
            TypeRequest::new(TypeQuery::At {
                file: source.clone(),
                position: SourcePosition {
                    line: 1,
                    column: 14,
                },
            }),
        )) {
            Ok(ProjectResponse::Type(answer)) => match *answer {
                TypeAnswer::Selected(result) => Ok(result.description.summary),
                other => panic!("{other:?}"),
            },
            Ok(other) => panic!("{other:?}"),
            Err(error) => Err(error),
        };
        assert_eq!(summary(&mut session).unwrap(), "1");
        session.tests.answer_once_stopped = Some(std::time::Duration::from_secs(20));
        session.tests.stale_while_validating = true;
        let answer = summary(&mut session);
        let _ = std::fs::remove_dir_all(&directory);
        assert_eq!(answer.unwrap(), "1");
        assert_eq!(session.generation(), 1);
    }

    /// Inputs that change after loading read them, while loading still runs,
    /// fail the refresh rather than publish what was read: the check after
    /// loading repeats every observation. Here a source's text changes, or a
    /// symbolic link is retargeted, once loading has read it; the next
    /// request publishes what is there.
    #[test]
    fn inputs_changed_while_loading_fail_the_refresh_then_publish() {
        for retarget in [false, true] {
            let directory = directory(
                "changed-while-loading",
                &[
                    (
                        "tsconfig.json",
                        r#"{"compilerOptions":{"noLib":true},"files":["a.ts","link.ts"]}"#,
                    ),
                    ("a.ts", "export const before = 1;\n"),
                    ("t1.ts", "export const firstTarget = 1;\n"),
                    ("t2.ts", "export const secondTarget = 1;\n"),
                ],
            );
            let link = directory.join("link.ts");
            std::os::unix::fs::symlink(directory.join("t1.ts"), &link).unwrap();
            let config = directory.join("tsconfig.json");
            let mut session = ProjectSession::new(&config).unwrap();
            let (source, target) = (directory.join("a.ts"), directory.join("t2.ts"));
            session.tests.while_loading = Some(Box::new(move || {
                if retarget {
                    std::fs::remove_file(&link).unwrap();
                    std::os::unix::fs::symlink(&target, &link).unwrap();
                } else {
                    std::fs::write(&source, "export const after_ = 1;\n").unwrap();
                }
            }));
            let groups = |session: &mut ProjectSession, name: &str| {
                session
                    .execute(ProjectRequest::References(ReferenceQuery::Name {
                        name: name.into(),
                        file: None,
                    }))
                    .map(|response| match response {
                        ProjectResponse::References(result) => result.groups.len(),
                        other => panic!("{other:?}"),
                    })
            };
            let refreshed = groups(&mut session, "before");
            let generation = session.generation();
            session.tests.while_loading = None;
            let names = if retarget {
                ["secondTarget", "firstTarget"]
            } else {
                ["after_", "before"]
            };
            let published = names.map(|name| groups(&mut session, name).unwrap());
            let _ = std::fs::remove_dir_all(&directory);
            match refreshed {
                Err(AnalysisError::Input(error)) => {
                    assert_eq!(error.kind, crate::analysis::InputErrorKind::Io);
                    assert_eq!(
                        error.message,
                        "Project inputs changed during indexing; retry when writes have completed"
                    );
                    assert_eq!(error.path.as_ref(), Some(&config));
                }
                other => panic!("a refresh that read a changed input published: {other:?}"),
            }
            assert_eq!(generation, 0, "retarget: {retarget}");
            assert_eq!(published, [1, 0], "retarget: {retarget}");
            assert_eq!(session.generation(), 1, "retarget: {retarget}");
        }
    }

    /// An answer prepared from a generation validation rejects is discarded,
    /// a panic included, and the reloaded generation answers; a current
    /// generation's panic remains the caller's.
    #[test]
    fn a_panicking_answer_from_a_stale_generation_is_discarded() {
        let directory =
            std::env::temp_dir().join(format!("tsparse-stale-panic-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("tsconfig.json"),
            r#"{ "compilerOptions": { "noLib": true } }"#,
        )
        .unwrap();
        std::fs::write(directory.join("a.ts"), "export const before = 1;\n").unwrap();
        let mut session = ProjectSession::new(&directory.join("tsconfig.json")).unwrap();
        let groups = |session: &mut ProjectSession, name: &str| match session
            .execute(ProjectRequest::References(ReferenceQuery::Name {
                name: name.into(),
                file: None,
            }))
            .unwrap()
        {
            ProjectResponse::References(result) => result.groups.len(),
            other => panic!("{other:?}"),
        };
        assert_eq!(groups(&mut session, "before"), 1);
        session.tests.panic_answering = Some(1);
        std::fs::write(directory.join("a.ts"), "export const after_ = 1;\n").unwrap();
        assert_eq!(groups(&mut session, "after_"), 1);
        assert_eq!(session.generation(), 2);
        session.tests.panic_answering = Some(2);
        let current = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            groups(&mut session, "after_")
        }));
        assert!(
            current.is_err(),
            "a current generation's panic is delivered"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }
    use crate::analysis::{AnalysisError, SemanticErrorKind};
    use crate::source::SourcePosition;
    use crate::types::{TypeAnswer, TypeQuery, TypeRequest};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    /// A semantic request whose answer is no longer wanted stops at its next
    /// check, with an internal failure that nobody delivers; unless it is
    /// superseded, the same request answers.
    #[test]
    fn a_superseded_semantic_request_stops_without_answering() {
        let directory =
            std::env::temp_dir().join(format!("tsparse-superseded-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("tsconfig.json"),
            r#"{ "compilerOptions": { "noLib": true } }"#,
        )
        .unwrap();
        std::fs::write(directory.join("a.ts"), "export const value = 1;\n").unwrap();
        let project = Project::load(&directory.join("tsconfig.json")).unwrap();
        let request = || {
            ProjectRequest::Type(TypeRequest::new(TypeQuery::At {
                file: directory.join("a.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 14,
                },
            }))
        };
        let superseded = Arc::new(AtomicBool::new(true));
        let stopped = project.execute_unless(request(), Some(&superseded));
        assert!(
            matches!(&stopped, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::Internal),
            "{stopped:?}"
        );
        let wanted = Arc::new(AtomicBool::new(false));
        let answered = project.execute_unless(request(), Some(&wanted));
        assert!(
            matches!(&answered, Ok(ProjectResponse::Type(answer)) if matches!(**answer, TypeAnswer::Selected(_))),
            "{answered:?}"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }
}
