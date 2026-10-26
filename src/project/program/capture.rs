//! Capturing program inputs ahead of the loader. The loader reserves each
//! input in TypeScript's order and integrates it in that order
//! (`Loader::load_file`); meanwhile worker threads capture inputs it has
//! reserved but not reached: they read, parse, index, and collect each one.
//! A capture is file-local. It depends on the input's path and text and on
//! its effective module status, never on another input or on the resolver.
//! Captures may reuse an earlier generation's products after reading equal
//! text; the loader rechecks module status before using collected facts.
//! The loader alone decides what a capture means
//! for the program: it observes the read where it uses it, and a capture's
//! failure surfaces only when the loader reaches its input, exactly where a
//! capture of its own would have failed. Workers and the loader read alike
//! (`disk::TextRead`), which refuses an input that is not a regular file,
//! such as a pipe, without opening it.

use super::disk::TextRead;
use crate::analysis::{InputError, InputErrorKind, Parser};
use crate::parser::json::JsonErrorKind;
use crate::project::binding::{self, FileFacts, FileOptions, ModuleIndicators};
use crate::project::navigation::Captured;
use crate::project::sources::FileId;
use crate::source::SourceText;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
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
    /// Whether to build the input's navigation catalog, which only searched
    /// inputs keep. Whether an input is searched is known only once the
    /// whole program is (`Loader::search`): this is the loader's guess when
    /// it reserves the input, and the loader builds a catalog the guess
    /// left out (`Program`'s final pass).
    pub(super) navigation: bool,
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

#[derive(Clone)]
pub(super) struct Parsed {
    pub(super) source: Arc<SourceText>,
    pub(super) contents: Result<Contents, InputError>,
}

#[derive(Clone)]
pub(super) enum Contents {
    /// A JSON module (`binding::collect_json`).
    Json(FileFacts),
    /// A TypeScript input: its navigation index, unless its job left it
    /// out (`Job::navigation`), its module indicators, and the effective `module` decision under
    /// which its facts were collected. This is collection's whole cache key.
    TypeScript {
        navigation: Option<Captured>,
        indicators: ModuleIndicators,
        module: bool,
        facts: Result<FileFacts, InputError>,
    },
}

/// Only the last successful generation's reached inputs. Cached facts are
/// pure collection, before synthetic requests or resolution mutate a load's
/// request list; the final module decision is kept with those facts.
pub(super) type Cache = HashMap<String, Cached>;

pub(super) struct Cached {
    pub(super) text: Arc<str>,
    pub(super) library: bool,
    pub(super) json: bool,
    pub(super) parsed: Parsed,
}

/// Keep in `cache` the catalog the loader built for `path` after its
/// capture left it out (`Job::navigation`).
pub(super) fn keep_navigation(cache: &mut Cache, path: &str, catalog: &Captured) {
    if let Some(Cached {
        parsed:
            Parsed {
                contents: Ok(Contents::TypeScript { navigation, .. }),
                ..
            },
        ..
    }) = cache.get_mut(path)
    {
        *navigation = Some(catalog.clone());
    }
}

/// Capture `job`'s input with `parser`.
#[cfg(test)]
pub(super) fn capture(parser: &Parser, job: &Job) -> Capture {
    capture_reusing(parser, job, &Cache::new())
}

fn capture_reusing(parser: &Parser, job: &Job, previous: &Cache) -> Capture {
    let mut read = TextRead::new(job.path.as_ref());
    if let Some(cached) = previous.get(&job.path)
        && cached.library == job.library
        && cached.json == job.json
        && read.reuse_text(&cached.text)
    {
        return Capture {
            read,
            json: job.json,
            parsed: Some(cached.parsed.clone()),
        };
    }
    let parsed = read.text().ok().map(|text| {
        let source = Arc::new(SourceText::shared(Arc::clone(text)));
        let contents = contents(parser, job, &source);
        Parsed { source, contents }
    });
    Capture {
        read,
        json: job.json,
        parsed,
    }
}

/// Refuse a program input whose text is longer than `u32::MAX` UTF-8 bytes,
/// as `Unsupported`: a limit of tsparse, not a TypeScript rule. It is the one
/// owner of that limit; binding packs every offset into a source as a `u32`
/// on the strength of it (`binding::site_map::pack_span`). Every program
/// input's text passes here (`contents`) before it is parsed or bound.
fn check_length(length: usize, path: &Path) -> Result<(), InputError> {
    if length <= u32::MAX as usize {
        return Ok(());
    }
    Err(InputError {
        kind: InputErrorKind::Unsupported,
        message: format!(
            "source file is longer than {} bytes; tsparse supports source files up to u32::MAX bytes",
            u32::MAX
        ),
        path: Some(path.to_path_buf()),
        position: None,
    })
}

fn contents(parser: &Parser, job: &Job, source: &Arc<SourceText>) -> Result<Contents, InputError> {
    let file_path = PathBuf::from(&job.path);
    check_length(source.as_str().len(), &file_path)?;
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
    let module = indicators.module(job.options);
    Ok(Contents::TypeScript {
        navigation: job.navigation.then(|| Captured::typescript(&ast, source)),
        indicators,
        module,
        facts: collect(&ast, source, indicators, module, &file_path),
    })
}

/// `binding::collect`, naming the input in a failure.
pub(super) fn collect(
    ast: &crate::Ast<'_>,
    source: &SourceText,
    indicators: ModuleIndicators,
    module: bool,
    path: &std::path::Path,
) -> Result<FileFacts, InputError> {
    binding::collect(ast, source, indicators, module).map_err(|mut error| {
        error.path.get_or_insert_with(|| path.to_path_buf());
        error
    })
}

/// How a job is captured: `capture`, except where a test observes it.
type CaptureWith<'c> = &'c (dyn Fn(&Parser, &Job) -> Capture + Sync);

/// The captures of one load: jobs in reservation order, which is the order
/// the loader takes them in, and workers that capture them ahead of it.
pub(super) struct Captures<'c> {
    capture: CaptureWith<'c>,
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
    /// Finished captures the loader has not taken: a worker's capture, or
    /// its panic.
    finished: HashMap<FileId, std::thread::Result<Capture>>,
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
    pub(super) fn with_reuse<T>(previous: &Cache, load: impl FnOnce(&Captures<'_>) -> T) -> T {
        let capture = |parser: &Parser, job: &Job| capture_reusing(parser, job, previous);
        Captures::run(workers(), &capture, load)
    }

    /// `with_reuse`, on `workers` workers that capture each job with `capture`.
    fn run<'c, T>(
        workers: usize,
        capture: CaptureWith<'c>,
        load: impl FnOnce(&Captures<'c>) -> T,
    ) -> T {
        let captures = Captures {
            capture,
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
    /// or the loader's own when no worker has started it.
    pub(super) fn take(&self, id: FileId, parser: &Parser) -> Capture {
        let mut state = self.lock();
        assert_eq!(
            id.index(),
            state.next,
            "inputs are taken in reservation order"
        );
        state.next += 1;
        loop {
            if let Some(capture) = state.finished.remove(&id) {
                drop(state);
                return capture.unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            }
            if state.waiting.front().is_some_and(|job| job.id == id) {
                let job = state.waiting.pop_front().expect("the front job");
                drop(state);
                return (self.capture)(parser, &job);
            }
            // A worker is capturing it.
            state = self
                .finished
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
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
            drop(state);
            // A panic belongs to the loader, which resumes it when it takes
            // this input, as if it had captured the input itself.
            let capture = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (self.capture)(parser, &job)
            }));
            state = self.lock();
            state.finished.insert(job.id, capture);
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
    use super::{Capture, Captures, Job};
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
        struct Chosen {
            _turn: std::sync::MutexGuard<'static, ()>,
        }
        impl Drop for Chosen {
            fn drop(&mut self) {
                WORKERS.store(usize::MAX, Ordering::Relaxed);
            }
        }
        let turn = CHOOSING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        WORKERS.store(workers.unwrap_or(usize::MAX), Ordering::Relaxed);
        Chosen { _turn: turn }
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
                    navigation: true,
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
        let capture = |parser: &Parser, job: &Job| {
            if job.id.index() == 0 {
                let mut finished = later.0.lock().unwrap();
                while *finished < count - 1 {
                    finished = later.1.wait(finished).unwrap();
                }
            }
            let captured = super::capture(parser, job);
            if job.id.index() != 0 {
                *later.0.lock().unwrap() += 1;
                later.1.notify_all();
            }
            captured
        };
        crate::analysis::on_parsing_thread("tsparse-test", |parser| {
            Captures::run(4, &capture, |captures| {
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
            let capture = |parser: &Parser, job: &Job| {
                if job.id.index() == 2 {
                    panicked.store(true, Ordering::SeqCst);
                    panic!("capture 2");
                }
                super::capture(parser, job)
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::analysis::on_parsing_thread("tsparse-test", |parser| {
                    Captures::run(2, &capture, |captures| {
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

    /// The length limit at and just over `u32::MAX` bytes. No end-to-end
    /// variant runs: a source over 4 GiB cannot be written, read, and loaded
    /// within a test's time and memory, so the rule takes the length as a
    /// value, and `contents` applies it to every input before parsing.
    #[test]
    fn a_source_longer_than_u32_max_bytes_is_unsupported() {
        let path = Path::new("/project/huge.ts");
        assert!(super::check_length(u32::MAX as usize, path).is_ok());
        let error = super::check_length(u32::MAX as usize + 1, path)
            .expect_err("a source one byte over the limit is refused");
        assert_eq!(error.kind, crate::analysis::InputErrorKind::Unsupported);
        assert_eq!(error.path.as_deref(), Some(path));
        assert_eq!(error.position, None);
        assert_eq!(
            error.to_string(),
            "/project/huge.ts: source file is longer than 4294967295 bytes; \
             tsparse supports source files up to u32::MAX bytes"
        );
    }
}
