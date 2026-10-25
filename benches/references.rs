//! Whole-project reference indexing costs, measured separately from parsing:
//! construction time, query time, and requested heap during and after load.
//! Queries are counted by outcome, and group-returning queries are timed
//! separately from the total, so failed queries cannot pass for fast answers.
//!
//! cargo bench --offline --bench references [-- path/to/tsconfig.json]
use std::{
    alloc::{GlobalAlloc, Layout, System},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use typescript_parser::analysis::{AnalysisError, QueryErrorKind};
use typescript_parser::project::Project;
use typescript_parser::references::ReferenceQuery;
use typescript_parser::source::SourcePosition;

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

/// A chain of modules that import their predecessor through a barrel, with
/// shadowing locals, namespaces, and classes in each.
fn synthetic_project(modules: usize) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("tsparse-references-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("tsconfig.json"),
        r#"{ "compilerOptions": { "noLib": true, "strict": true }, "include": ["src"] }"#,
    )
    .unwrap();
    let mut barrel = String::new();
    for index in 0..modules {
        let previous = index.saturating_sub(1);
        let mut text = String::new();
        if index > 0 {
            text.push_str(&format!(
                "import {{ run{previous} as previous, Shape{previous} }} from \"./barrel\";\n"
            ));
        }
        text.push_str(&format!(
            "export interface Shape{index} {{ size: number }}\n\
             export namespace Tools{index} {{ export const scale = {index}; }}\n\
             export class Widget{index} implements Shape{index} {{\n  size = Tools{index}.scale;\n  grow(by: number) {{ const size = this.size + by; return size; }}\n}}\n\
             export function run{index}(input: number): number {{\n  let total = input;\n  for (let step = 0; step < 3; step++) {{ const total2 = total + step; total = total2; }}\n"
        ));
        if index > 0 {
            text.push_str(&format!(
                "  const shape: Shape{previous} = {{ size: total }};\n  return previous(shape.size);\n"
            ));
        } else {
            text.push_str("  return total;\n");
        }
        text.push_str("}\n");
        std::fs::write(root.join(format!("src/module{index}.ts")), text).unwrap();
        barrel.push_str(&format!("export * from \"./module{index}\";\n"));
    }
    std::fs::write(root.join("src/barrel.ts"), barrel).unwrap();
    root.join("tsconfig.json")
}

/// Positions at the start of each word in the project's sources, in the
/// public `SourcePosition` form: one-based lines, broken at CR, LF, CRLF,
/// U+2028, and U+2029 as the crate's source index does, and one-based UTF-16
/// columns.
fn positions(config: &Path, limit: usize) -> Vec<(PathBuf, SourcePosition)> {
    let mut files = Vec::new();
    let mut stack = vec![config.parent().unwrap().to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name != "node_modules") {
                    stack.push(path);
                }
            } else if path
                .extension()
                .is_some_and(|ext| ext == "ts" || ext == "tsx")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut positions = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let (mut line, mut column) = (1, 1);
        let mut previous = '\n';
        for c in text.chars() {
            if matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                // CRLF is one break.
                if !(c == '\n' && previous == '\r') {
                    line += 1;
                }
                column = 1;
            } else {
                if c.is_alphabetic() && !(previous.is_alphanumeric() || previous == '_') {
                    positions.push((file.clone(), SourcePosition { line, column }));
                }
                column += c.len_utf16();
            }
            previous = c;
        }
    }
    let step = (positions.len() / limit).max(1);
    positions.into_iter().step_by(step).take(limit).collect()
}

/// Query outcomes by kind. Every query lands in exactly one count.
#[derive(Default)]
struct Outcomes {
    answered: usize,
    groups: usize,
    answered_time: Duration,
    empty: usize,
    outside_project: usize,
    invalid_position: usize,
    unsupported_target: usize,
    unresolved_target: usize,
}

fn micros_each(time: Duration, count: usize) -> f64 {
    time.as_secs_f64() * 1e6 / count.max(1) as f64
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
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
    let loads: Vec<Duration> = (0..5)
        .map(|_| {
            let start = Instant::now();
            let project = Project::load(&config).unwrap();
            let elapsed = start.elapsed();
            drop(project);
            elapsed
        })
        .collect();
    let project = Project::load(&config).unwrap();
    let queries = positions(&config, 2_000);
    let mut outcomes = Outcomes::default();
    let mut query_time = Duration::ZERO;
    for (file, position) in &queries {
        let query = ReferenceQuery::At {
            file: file.clone(),
            position: *position,
        };
        let start = Instant::now();
        let result = project.find_references(query);
        let elapsed = start.elapsed();
        query_time += elapsed;
        match result {
            Ok(result) if result.groups.is_empty() => outcomes.empty += 1,
            Ok(result) => {
                outcomes.answered += 1;
                outcomes.groups += result.groups.len();
                outcomes.answered_time += elapsed;
            }
            Err(AnalysisError::Query(error)) => match error.kind {
                QueryErrorKind::OutsideProject => outcomes.outside_project += 1,
                QueryErrorKind::InvalidPosition => outcomes.invalid_position += 1,
                QueryErrorKind::UnsupportedTarget => outcomes.unsupported_target += 1,
                QueryErrorKind::UnresolvedTarget => outcomes.unresolved_target += 1,
                QueryErrorKind::InvalidRequest | QueryErrorKind::UnknownProperty => {
                    panic!("{error}")
                }
            },
            // Queries name the captured files exactly.
            Err(AnalysisError::Input(error)) => panic!("{error}"),
            Err(AnalysisError::Semantic(error)) => panic!("{error}"),
        }
    }
    drop(project);

    LIVE.store(0, Ordering::Relaxed);
    PEAK.store(0, Ordering::Relaxed);
    ENABLED.store(true, Ordering::Relaxed);
    let project = Project::load(&config).unwrap();
    let retained = LIVE.load(Ordering::Relaxed);
    let peak = PEAK.load(Ordering::Relaxed);
    drop(project);
    ENABLED.store(false, Ordering::Relaxed);

    println!("project: {}", config.display());
    println!(
        "load median of 5: {:.2} ms",
        median(loads).as_secs_f64() * 1e3
    );
    println!(
        "queries: {} positions, {:.1} µs each",
        queries.len(),
        micros_each(query_time, queries.len())
    );
    println!(
        "  groups returned: {} queries, {} groups, {:.1} µs each",
        outcomes.answered,
        outcomes.groups,
        micros_each(outcomes.answered_time, outcomes.answered)
    );
    println!("  empty result: {}", outcomes.empty);
    println!("  OutsideProject: {}", outcomes.outside_project);
    println!("  InvalidPosition: {}", outcomes.invalid_position);
    println!("  UnsupportedTarget: {}", outcomes.unsupported_target);
    println!("  UnresolvedTarget: {}", outcomes.unresolved_target);
    println!("load peak requested heap: {peak} bytes; retained after load: {retained} bytes");
}
