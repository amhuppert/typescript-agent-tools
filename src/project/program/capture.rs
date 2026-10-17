//! Capturing program inputs ahead of the loader. The loader reserves each
//! input in TypeScript's order and integrates it in that order
//! (`Loader::load_file`); meanwhile worker threads capture inputs it has
//! reserved but not reached: they read, parse, index, and collect each one.
//! A capture is file-local. It depends on the input's path and text and on
//! options the loader settled when it reserved the input, never on another
//! input or on the resolver, so the loader alone decides what a capture means
//! for the program: it observes the read where it uses it, and a capture's
//! failure surfaces only when the loader reaches its input, exactly where a
//! capture of its own would have failed. Workers read speculatively
//! (`Reading::Speculative`): an input that is not a regular file, such as a
//! pipe, is read only by the loader, when it reaches the input.

use super::disk::{Reading, TextRead, Waits};
use crate::analysis::{InputError, InputErrorKind, Parser};
use crate::parser::json::JsonErrorKind;
use crate::project::binding::{self, FileFacts, FileOptions, ModuleIndicators};
use crate::project::navigation::Captured;
use crate::project::sources::FileId;
use crate::source::SourceText;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

/// Worker threads beside the loader. Capturing is most of a load's work
/// for each input, and the loader integrates an input in about a third of
/// the time a worker takes to capture it. More workers measured no faster:
/// inputs are found only as the loader resolves their importers.
const WORKERS: usize = 4;

/// One input to capture, as the loader reserved it.
pub(super) struct Job {
    pub(super) id: FileId,
    /// The identity path, which names the input in every failure.
    pub(super) path: String,
    pub(super) library: bool,
    /// Whether the input is a JSON module, which loading decides once, when
    /// it reserves the input.
    pub(super) json: bool,
    /// The input's options as far as the loader knows them before the
    /// input's format: `Loader::file_options` with `FileFormat::undeclared`.
    pub(super) options: FileOptions,
}

/// One input's capture: everything loading derives from the input alone.
pub(super) struct Capture {
    pub(super) read: TextRead,
    /// The job's decision.
    pub(super) json: bool,
    /// Absent when the read failed.
    pub(super) parsed: Option<Parsed>,
}

pub(super) struct Parsed {
    pub(super) source: Arc<SourceText>,
    pub(super) contents: Result<Contents, InputError>,
}

pub(super) enum Contents {
    /// A JSON module (`binding::collect_json`).
    Json(FileFacts),
    /// A TypeScript input: its navigation index, unless it is a library,
    /// its module indicators, and its facts as collected with `options`,
    /// the job's.
    TypeScript {
        navigation: Option<Captured>,
        indicators: ModuleIndicators,
        options: FileOptions,
        facts: Result<FileFacts, InputError>,
    },
}

/// Capture `job`'s input with `parser`, reading as `reading` reads; nothing
/// when that reading declines to read the input.
pub(super) fn capture(parser: &Parser, job: &Job, reading: Reading<'_>) -> Option<Capture> {
    let read = TextRead::with(job.path.as_ref(), reading)?;
    let parsed = read.text().ok().map(|text| {
        let source = Arc::new(SourceText::shared(Arc::clone(text)));
        let contents = contents(parser, job, &source);
        Parsed { source, contents }
    });
    Some(Capture {
        read,
        json: job.json,
        parsed,
    })
}

fn contents(parser: &Parser, job: &Job, source: &Arc<SourceText>) -> Result<Contents, InputError> {
    let file_path = PathBuf::from(&job.path);
    if job.json {
        // TypeScript's `ParseSourceFile` with `ScriptKindJSON`, read by the
        // reader configuration files share.
        let text = parser
            .parse_json(source.as_str())
            .map_err(|error| InputError {
                kind: match error.kind {
                    JsonErrorKind::Syntax => InputErrorKind::Syntax,
                    JsonErrorKind::TooDeep => InputErrorKind::Unsupported,
                },
                message: error.message,
                path: Some(file_path.clone()),
                position: Some(source.position(error.offset)),
            })?;
        return Ok(Contents::Json(binding::collect_json(
            &text,
            source.as_str(),
        )));
    }
    let ast = parser
        .parse(source.as_str(), &job.path)
        .map_err(|error| InputError::unparsable(error, source, &file_path))?;
    let indicators = ModuleIndicators::of(&ast);
    Ok(Contents::TypeScript {
        navigation: (!job.library).then(|| Captured::typescript(&ast, source)),
        indicators,
        options: job.options,
        facts: collect(&ast, source, indicators, job.options, &file_path),
    })
}

/// `binding::collect`, naming the input in a failure.
pub(super) fn collect(
    ast: &crate::Ast<'_>,
    source: &SourceText,
    indicators: ModuleIndicators,
    options: FileOptions,
    path: &std::path::Path,
) -> Result<FileFacts, InputError> {
    binding::collect(ast, source, indicators, options).map_err(|mut error| {
        error.path.get_or_insert_with(|| path.to_path_buf());
        error
    })
}

/// How a job is captured: `capture`, except where a test observes it.
type CaptureWith<'c> = &'c (dyn Fn(&Parser, &Job, Reading<'_>) -> Option<Capture> + Sync);

/// The captures of one load: jobs in reservation order, which is the order
/// the loader takes them in, and workers that capture them ahead of it.
pub(super) struct Captures<'c> {
    capture: CaptureWith<'c>,
    /// The load's reads in order that may have waited: a capture begun
    /// before one of them ended is out of date.
    waits: &'c Waits,
    state: Mutex<State>,
    /// Workers wait for a job they may start.
    startable: Condvar,
    /// The loader waits for a worker to finish the job it needs.
    finished: Condvar,
}

#[derive(Default)]
struct State {
    /// Reserved jobs no worker has started, by increasing id.
    waiting: VecDeque<Job>,
    /// Finished captures the loader has not taken, each with its job and the
    /// count of `Captures::waits` when it began: a worker's capture, nothing
    /// when it declined to read, or its panic.
    finished: HashMap<FileId, (Job, u64, std::thread::Result<Option<Capture>>)>,
    /// The id after the last one the loader took.
    next: usize,
    closed: bool,
}

impl Captures<'_> {
    /// Run `load` with this load's captures, on up to `WORKERS` capturing
    /// threads beside it. A worker that cannot start leaves its share to
    /// the others and to the loader, which captures any job no worker has
    /// started when it needs it, so the result does not depend on how many
    /// workers there are.
    pub(super) fn with<'c, T>(waits: &'c Waits, load: impl FnOnce(&Captures<'c>) -> T) -> T {
        Captures::run(workers(), waits, &capture, load)
    }

    /// `with`, on `workers` workers that capture each job with `capture`.
    fn run<'c, T>(
        workers: usize,
        waits: &'c Waits,
        capture: CaptureWith<'c>,
        load: impl FnOnce(&Captures<'c>) -> T,
    ) -> T {
        let captures = Captures {
            capture,
            waits,
            state: Mutex::new(State::default()),
            startable: Condvar::new(),
            finished: Condvar::new(),
        };
        std::thread::scope(|scope| {
            // Workers stop when loading ends, also by a panic, which the
            // scope then resumes once they have stopped.
            let _close = Close(&captures);
            for _ in 0..workers {
                let _ = crate::analysis::spawn_parsing_thread(scope, "tsparse-capture", |parser| {
                    captures.work(parser)
                });
            }
            load(&captures)
        })
    }

    /// Queue a newly reserved input.
    pub(super) fn reserve(&self, job: Job) {
        let mut state = self.lock();
        debug_assert!(
            state.waiting.back().is_none_or(|last| last.id < job.id),
            "inputs are reserved in id order"
        );
        state.waiting.push_back(job);
        drop(state);
        self.startable.notify_one();
    }

    /// The capture of the next input in reservation order, `id`: a worker's,
    /// or the loader's own (`in_order`) when no worker has started it, a
    /// worker declined to read it, or a worker read it before an earlier
    /// input's read that may have waited.
    pub(super) fn take(&self, id: FileId, parser: &Parser) -> Capture {
        let mut state = self.lock();
        assert_eq!(
            id.index(),
            state.next,
            "inputs are taken in reservation order"
        );
        state.next += 1;
        loop {
            if let Some((job, waits, capture)) = state.finished.remove(&id) {
                let current = waits == self.waits.count();
                drop(state);
                // Reading in order reads this input after any such earlier
                // read; a capture read before it, and its panic, are for
                // nobody.
                if !current {
                    return self.in_order(parser, &job);
                }
                return capture
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
                    .unwrap_or_else(|| self.in_order(parser, &job));
            }
            if state.waiting.front().is_some_and(|job| job.id == id) {
                let job = state.waiting.pop_front().expect("the front job");
                drop(state);
                return self.in_order(parser, &job);
            }
            // A worker is capturing it.
            state = self
                .finished
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// The loader's capture of `job`, where loading in order reaches it
    /// (`Reading::InOrder`). An input a speculative reading declines, such as
    /// a pipe, is read as an ordinary read is, waiting as it must; captures
    /// workers began before that read ended are then out of date, as after
    /// every other read in order that may have waited, such as a manifest's.
    fn in_order(&self, parser: &Parser, job: &Job) -> Capture {
        (self.capture)(parser, job, Reading::InOrder(self.waits)).expect("a reading in order reads")
    }

    fn work(&self, parser: &Parser) {
        let mut state = self.lock();
        loop {
            if state.closed {
                return;
            }
            // Workers capture any reserved input, however far ahead of the
            // loader: every reserved input joins the program unless loading
            // fails, so a capture costs no memory the program would not hold.
            if state.waiting.is_empty() {
                state = self
                    .startable
                    .wait(state)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                continue;
            }
            let job = state.waiting.pop_front().expect("the front job");
            let waits = self.waits.count();
            drop(state);
            // A panic belongs to the loader, which resumes it when it takes
            // this input, as if it had captured the input itself.
            let capture = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (self.capture)(parser, &job, Reading::Speculative)
            }));
            state = self.lock();
            state.finished.insert(job.id, (job, waits, capture));
            self.finished.notify_all();
        }
    }

    /// Stop the workers once their current captures finish. Captures the
    /// loader did not take are discarded unobserved.
    fn close(&self) {
        self.lock().closed = true;
        self.startable.notify_all();
    }

    /// The state, also after a panic: only `take`'s order assertion can
    /// panic holding it, before changing it, and the loader's panic closes
    /// the captures (`Close`).
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

struct Close<'a, 'c>(&'a Captures<'c>);

impl Drop for Close<'_, '_> {
    fn drop(&mut self) {
        self.0.close();
    }
}

/// How many workers capture inputs: `WORKERS`, fewer on a smaller machine,
/// leaving one processor to the loader.
fn workers() -> usize {
    #[cfg(test)]
    if let Some(workers) = tests::workers() {
        return workers;
    }
    std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .saturating_sub(1)
        .min(WORKERS)
}

#[cfg(test)]
pub(super) mod tests {
    use super::{Capture, Captures, Job, Reading, Waits};
    use crate::analysis::Parser;
    use crate::project::binding::FileOptions;
    use crate::project::sources::Sources;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Condvar, Mutex};

    /// A test's choice of worker count, or `usize::MAX` for the default.
    /// Loads in other tests may see it; a worker count never changes what a
    /// load produces.
    static WORKERS: AtomicUsize = AtomicUsize::new(usize::MAX);
    /// Held by the one test whose choice `WORKERS` is.
    static CHOOSING: Mutex<()> = Mutex::new(());

    /// Load with `workers` workers, or the default, while this lives: tests
    /// that choose take turns, so each load sees its test's choice.
    pub(in crate::project::program) fn use_workers(workers: Option<usize>) -> impl Drop {
        struct Chosen(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);
        impl Drop for Chosen {
            fn drop(&mut self) {
                WORKERS.store(usize::MAX, Ordering::Relaxed);
            }
        }
        let turn = CHOOSING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        WORKERS.store(workers.unwrap_or(usize::MAX), Ordering::Relaxed);
        Chosen(turn)
    }

    pub(super) fn workers() -> Option<usize> {
        let workers = WORKERS.load(Ordering::Relaxed);
        (workers != usize::MAX).then_some(workers)
    }

    struct Directory(PathBuf);

    impl Directory {
        fn new(name: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "tsparse-captures-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn text(index: usize) -> String {
        format!("export const v{index} = {index};")
    }

    /// Reserve `count` inputs, each with its own text.
    fn reserve(
        captures: &Captures<'_>,
        directory: &Path,
        count: usize,
    ) -> Vec<crate::project::sources::FileId> {
        let mut sources = Sources::default();
        (0..count)
            .map(|index| {
                let path = directory.join(format!("f{index}.ts"));
                std::fs::write(&path, text(index)).unwrap();
                let id = sources.reserve();
                captures.reserve(Job {
                    id,
                    path: path.to_str().unwrap().into(),
                    library: false,
                    json: false,
                    options: FileOptions::default(),
                });
                id
            })
            .collect()
    }

    fn read(capture: &Capture) -> String {
        capture.read.text().unwrap().to_string()
    }

    /// Captures finish in any order, here the first last; the loader takes
    /// each in reservation order and receives that input's own capture.
    #[test]
    fn captures_are_taken_in_reservation_order_however_they_finish() {
        let directory = Directory::new("order");
        let count = 8;
        let later = (Mutex::new(0), Condvar::new());
        let capture = |parser: &Parser, job: &Job, reading: Reading| {
            if job.id.index() == 0 {
                let mut finished = later.0.lock().unwrap();
                while *finished < count - 1 {
                    finished = later.1.wait(finished).unwrap();
                }
            }
            let captured = super::capture(parser, job, reading);
            if job.id.index() != 0 {
                *later.0.lock().unwrap() += 1;
                later.1.notify_all();
            }
            captured
        };
        crate::analysis::on_parsing_thread("tsparse-test", |parser| {
            Captures::run(4, &Waits::default(), &capture, |captures| {
                let ids = reserve(captures, &directory.0, count);
                for (index, id) in ids.into_iter().enumerate() {
                    assert_eq!(read(&captures.take(id, parser)), text(index));
                }
            })
        })
        .unwrap();
    }

    /// A capture's panic is the loader's when it takes that input, and
    /// nobody's when loading ends before reaching it.
    #[test]
    fn a_capture_panic_surfaces_only_when_its_input_is_taken() {
        let directory = Directory::new("panic");
        for taken in [2, 3] {
            let panicked = AtomicBool::new(false);
            let capture = |parser: &Parser, job: &Job, reading: Reading| {
                if job.id.index() == 2 {
                    panicked.store(true, Ordering::SeqCst);
                    panic!("capture 2");
                }
                super::capture(parser, job, reading)
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::analysis::on_parsing_thread("tsparse-test", |parser| {
                    Captures::run(2, &Waits::default(), &capture, |captures| {
                        let ids = reserve(captures, &directory.0, 4);
                        for (index, &id) in ids.iter().enumerate().take(taken) {
                            assert_eq!(read(&captures.take(id, parser)), text(index));
                        }
                        // Loading ends only once the input it never takes
                        // has been captured, and its capture has panicked.
                        let started = std::time::Instant::now();
                        while !panicked.load(Ordering::SeqCst) {
                            assert!(started.elapsed() < std::time::Duration::from_secs(20));
                            std::thread::yield_now();
                        }
                    })
                })
                .unwrap()
            }));
            if taken == 2 {
                assert!(result.is_ok(), "an input never taken panics nobody");
            } else {
                let payload = result.expect_err("taking the input resumes its panic");
                assert_eq!(payload.downcast_ref::<&str>(), Some(&"capture 2"));
            }
        }
    }

    /// A capture a worker made before a read in order that may have waited
    /// ended, here a manifest's from a pipe whose writer changes the input
    /// first, is made again: loading in order reads the input after it.
    #[cfg(unix)]
    #[test]
    fn a_capture_made_before_a_wait_ended_is_made_again() {
        let directory = Directory::new("wait");
        let waits = Waits::default();
        let captured = (Mutex::new(false), Condvar::new());
        let capture = |parser: &Parser, job: &Job, reading: Reading<'_>| {
            let result = super::capture(parser, job, reading);
            if job.id.index() == 1 && matches!(reading, Reading::Speculative) {
                *captured.0.lock().unwrap() = true;
                captured.1.notify_all();
            }
            result
        };
        let manifest = directory.0.join("package.json");
        let made = std::process::Command::new("mkfifo")
            .arg(&manifest)
            .status()
            .unwrap();
        assert!(made.success());
        crate::analysis::on_parsing_thread("tsparse-test", |parser| {
            Captures::run(2, &waits, &capture, |captures| {
                let ids = reserve(captures, &directory.0, 2);
                assert_eq!(read(&captures.take(ids[0], parser)), text(0));
                let mut done = captured.0.lock().unwrap();
                while !*done {
                    done = captured.1.wait(done).unwrap();
                }
                drop(done);
                let writer = {
                    let (manifest, input) = (manifest.clone(), directory.0.join("f1.ts"));
                    std::thread::spawn(move || {
                        use std::io::Write;
                        let mut pipe = std::fs::OpenOptions::new()
                            .write(true)
                            .open(manifest)
                            .unwrap();
                        std::fs::write(input, "export const changed = 1;").unwrap();
                        pipe.write_all(b"{}").unwrap();
                    })
                };
                let read_in_order =
                    super::TextRead::with(&manifest, Reading::InOrder(&waits)).unwrap();
                writer.join().unwrap();
                assert_eq!(read_in_order.text().unwrap().as_ref(), "{}");
                assert_eq!(
                    read(&captures.take(ids[1], parser)),
                    "export const changed = 1;"
                );
            })
        })
        .unwrap();
    }
}
