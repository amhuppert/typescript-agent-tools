//! Project navigation costs on one configuration: construction time,
//! requested heap during and after loading; outline and show latency and
//! peak requested heap on a small and the largest searched file; and
//! project-wide show by a missing, an ambiguous, and a unique name in each
//! of those files, whose outcomes the benchmark asserts; each from a
//! snapshot and through a warm session, which also validates freshness.
//! `docs/navigation-performance.md` records the storage comparison this
//! measured.
//!
//! cargo bench --offline --bench navigation [-- path/to/tsconfig.json]
//!
//! Without an argument it measures a generated project of 300 modules.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use typescript_parser::navigation::{Outline, ShowOutcome, ShowQuery};
use typescript_parser::project::{Project, ProjectRequest, ProjectResponse, ProjectSession};

struct Allocator;
static ENABLED: AtomicBool = AtomicBool::new(false);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
fn allocated(size: usize) {
    if ENABLED.load(Ordering::Relaxed) {
        let live = LIVE.fetch_add(size, Ordering::Relaxed) + size;
        PEAK.fetch_max(live, Ordering::Relaxed);
    }
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            allocated(layout.size());
        }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            allocated(layout.size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ENABLED.load(Ordering::Relaxed) {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(ptr, layout, size) };
        if !result.is_null() && ENABLED.load(Ordering::Relaxed) {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            allocated(size);
        }
        result
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

/// Requested heap while `work` runs, above what was live before it, and
/// what it leaves live.
fn accounted<T>(work: impl FnOnce() -> T) -> (T, usize, usize) {
    LIVE.store(0, Ordering::Relaxed);
    PEAK.store(0, Ordering::Relaxed);
    ENABLED.store(true, Ordering::Relaxed);
    let result = work();
    ENABLED.store(false, Ordering::Relaxed);
    (
        result,
        PEAK.load(Ordering::Relaxed),
        LIVE.load(Ordering::Relaxed),
    )
}

/// A chain of modules, each with a namespace, a class, and functions.
fn synthetic_project(modules: usize) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("tsparse-navigation-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("tsconfig.json"),
        r#"{ "compilerOptions": { "noLib": true }, "include": ["src"] }"#,
    )
    .unwrap();
    for index in 0..modules {
        std::fs::write(
            root.join(format!("src/module{index}.ts")),
            format!(
                "export interface Shape{index} {{ size: number }}\n\
                 export namespace Tools{index} {{ export const scale = {index}; }}\n\
                 export class Widget{index} implements Shape{index} {{\n  size = Tools{index}.scale;\n  grow(by: number) {{ const size = this.size + by; return size; }}\n}}\n\
                 export function run{index}(input: number): number {{ return input; }}\n"
            ),
        )
        .unwrap();
    }
    root.join("tsconfig.json")
}

/// TypeScript files under the configuration directory, largest first,
/// outside `node_modules` and hidden directories.
fn candidates(config: &Path) -> Vec<(u64, PathBuf)> {
    let mut files = Vec::new();
    let mut stack = vec![config.parent().unwrap().to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                if name != "node_modules" && !name.starts_with('.') {
                    stack.push(entry.path());
                }
            } else if kind.is_file() && (name.ends_with(".ts") || name.ends_with(".tsx")) {
                files.push((entry.metadata().unwrap().len(), entry.path()));
            }
        }
    }
    files.sort_by(|a, b| b.cmp(a));
    files
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

fn millis(time: Duration) -> f64 {
    time.as_secs_f64() * 1e3
}

fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1 << 20) as f64
}

/// The outcome a name search must have, so that a changed project cannot
/// silently change what a case measures.
#[derive(Clone, Copy)]
enum Expect<'a> {
    NotFound,
    Ambiguous,
    /// Found in this file, named relative to the configuration directory.
    FoundIn(&'a str),
}

impl Expect<'_> {
    /// Check `outcome` and describe it.
    fn check(self, name: &str, outcome: ShowOutcome) -> String {
        match (self, outcome) {
            (Self::NotFound, ShowOutcome::NotFound) => "not found".into(),
            (Self::Ambiguous, ShowOutcome::Ambiguous(candidates)) => {
                format!("{} candidates", candidates.len())
            }
            (Self::FoundIn(file), ShowOutcome::Found(found)) if found.file == file => {
                format!("found, {} bytes", found.source.len())
            }
            (_, outcome) => panic!("`{name}` answered {outcome:?}"),
        }
    }
}

/// Median time and peak requested heap above the snapshot of `runs` calls.
fn measure<T>(runs: usize, mut work: impl FnMut() -> T) -> (Duration, usize) {
    let mut times = Vec::new();
    let mut peak = 0;
    for _ in 0..runs {
        let start = Instant::now();
        let (_, query_peak, _) = accounted(&mut work);
        times.push(start.elapsed());
        peak = peak.max(query_peak);
    }
    (median(times), peak)
}

fn main() {
    let args: Vec<_> = std::env::args()
        .skip(1)
        .filter(|arg| arg != "--bench")
        .collect();
    let config = match args.first() {
        Some(path) => PathBuf::from(path),
        None => synthetic_project(300),
    };
    let loads: Vec<Duration> = (0..3)
        .map(|_| {
            let start = Instant::now();
            let project = Project::load(&config).unwrap();
            let elapsed = start.elapsed();
            drop(project);
            elapsed
        })
        .collect();
    let (project, load_peak, retained) = accounted(|| Project::load(&config).unwrap());

    // The largest searched file, and a small one: the smallest searched
    // file of at least 2 KB.
    let searched: Vec<(u64, PathBuf, Outline)> = candidates(&config)
        .into_iter()
        .filter_map(|(size, path)| {
            let outline = project.outline(&path).ok()?;
            Some((size, path, outline))
        })
        .collect();
    let entries: usize = searched.iter().map(|(_, _, o)| o.entries.len()).sum();
    let largest = &searched[0];
    let small = searched
        .iter()
        .rev()
        .find(|(size, _, outline)| *size >= 2048 && !outline.entries.is_empty())
        .unwrap_or(&searched[searched.len() - 1]);

    println!("project: {}", config.display());
    println!(
        "searched TypeScript files: {}, declarations: {entries}",
        searched.len()
    );
    println!("load median of 3: {:.0} ms", millis(median(loads)));
    println!(
        "load peak requested heap: {:.1} MiB; retained after load: {:.1} MiB",
        mib(load_peak),
        mib(retained)
    );
    for (label, (size, path, outline)) in [("small", small), ("largest", largest)] {
        let anchor = outline.entries.last().map(|entry| entry.anchor).unwrap();
        let (outline_time, outline_peak) = measure(20, || project.outline(path).unwrap());
        let (at_time, at_peak) = measure(20, || {
            project
                .show(ShowQuery::At {
                    file: path.clone(),
                    position: anchor,
                })
                .unwrap()
        });
        println!(
            "{label} file: {} ({size} bytes, {} declarations)",
            path.display(),
            outline.entries.len()
        );
        println!(
            "  outline: {:.3} ms, peak {:.2} MiB",
            millis(outline_time),
            mib(outline_peak)
        );
        println!(
            "  show --at last entry: {:.3} ms, peak {:.2} MiB",
            millis(at_time),
            mib(at_peak)
        );
    }

    // Name searches across the project, by outcome: a missing or ambiguous
    // name reads only the indexes; a unique one also projects its file.
    let search = |name: &str| {
        project
            .show(ShowQuery::Name {
                name: name.into(),
                file: None,
            })
            .unwrap()
            .outcome
    };
    // The last qualified selector of a file that the search finds once.
    let unique_in = |outline: &Outline| {
        outline
            .entries
            .iter()
            .rev()
            .filter_map(|entry| entry.selector.clone())
            .find(|selector| {
                matches!(search(selector), ShowOutcome::Found(found) if found.file == outline.file)
            })
            .expect("a declaration with a unique selector")
    };
    // The last key of the largest file that several declarations have.
    let ambiguous = largest
        .2
        .entries
        .iter()
        .rev()
        .filter_map(|entry| entry.key.clone())
        .find(|key| matches!(search(key), ShowOutcome::Ambiguous(_)))
        .expect("a key with several declarations");
    let cases = [
        (
            "missing",
            "absentFromEveryFile".to_owned(),
            Expect::NotFound,
        ),
        ("ambiguous", ambiguous, Expect::Ambiguous),
        (
            "unique in the small file",
            unique_in(&small.2),
            Expect::FoundIn(&small.2.file),
        ),
        (
            "unique in the largest file",
            unique_in(&largest.2),
            Expect::FoundIn(&largest.2.file),
        ),
    ];
    let mut described = Vec::new();
    for (label, name, expect) in &cases {
        let outcome = expect.check(name, search(name));
        let (time, peak) = measure(20, || search(name));
        described.push(format!(
            "  {label} `{name}` ({outcome}): snapshot {:.3} ms, peak {:.2} MiB",
            millis(time),
            mib(peak)
        ));
    }
    drop(project);

    // Warm session requests validate freshness, then answer.
    let mut session = ProjectSession::new(&config).unwrap();
    let show = |name: &str| {
        ProjectRequest::Show(ShowQuery::Name {
            name: name.into(),
            file: None,
        })
    };
    session.execute(show("absentFromEveryFile")).unwrap();
    println!(
        "show --name across the project: median of 20 from the snapshot, and of 10 warm session requests"
    );
    for ((_, name, expect), line) in cases.iter().zip(&described) {
        let (warm, _) = measure(10, || {
            let ProjectResponse::Show(shown) = session.execute(show(name)).unwrap() else {
                panic!("a show request answers a selection");
            };
            expect.check(name, shown.outcome);
        });
        println!("{line}; warm session {:.1} ms", millis(warm));
    }

    let (warm, _) = measure(10, || {
        session
            .execute(ProjectRequest::Outline {
                file: largest.1.clone(),
            })
            .unwrap()
    });
    let (warm_small, _) = measure(10, || {
        session
            .execute(ProjectRequest::Outline {
                file: small.1.clone(),
            })
            .unwrap()
    });
    assert_eq!(
        session.generation(),
        1,
        "warm requests reuse the generation"
    );
    println!(
        "warm session outline, median of 10: largest {:.1} ms, small {:.1} ms",
        millis(warm),
        millis(warm_small)
    );
}
