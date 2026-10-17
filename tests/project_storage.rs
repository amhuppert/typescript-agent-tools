//! A published project must release construction-only binding state. Requested
//! heap is measured separately from RSS and wall time, so host load does not
//! decide this resource regression test.
mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use support::{Fixture, position};
use typescript_parser::references::ReferenceQuery;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

struct CountingAllocator;
static ENABLED: AtomicBool = AtomicBool::new(false);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn allocated(bytes: usize) {
    if ENABLED.load(Ordering::Relaxed) {
        let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
        PEAK.fetch_max(live, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
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

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, bytes: usize) -> *mut u8 {
        let result = unsafe { System.realloc(ptr, layout, bytes) };
        if !result.is_null() && ENABLED.load(Ordering::Relaxed) {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            allocated(bytes);
        }
        result
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn published_project_retains_compact_facts_and_answers_queries() {
    // Keep this executable single-test: allocation accounting spans the loader
    // thread and deliberately excludes fixture construction and result output.
    let fixture = Fixture::new(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"noLib":true,"strict":true},"include":["*.ts"]}"#,
    )]);
    for index in 0..256 {
        let mut source = format!("export interface Shape{index} {{\n");
        for member in 0..16 {
            source.push_str(&format!("  member{member}: number;\n"));
        }
        source.push_str(&format!(
            "}}\nexport function run{index}(value: number | undefined): number {{\n\
             if (value === undefined) value = 0;\n\
             const result = value;\nreturn result;\n}}\n"
        ));
        std::fs::write(fixture.path(&format!("module{index}.ts")), source).unwrap();
    }
    let main = "import { run0 } from './module0';\nexport const result = run0(42);\n";
    std::fs::write(fixture.path("main.ts"), main).unwrap();

    LIVE.store(0, Ordering::Relaxed);
    PEAK.store(0, Ordering::Relaxed);
    ENABLED.store(true, Ordering::Relaxed);
    let loaded = typescript_parser::project::Project::load(&fixture.path("tsconfig.json"));
    let retained = LIVE.load(Ordering::Relaxed);
    let peak = PEAK.load(Ordering::Relaxed);
    ENABLED.store(false, Ordering::Relaxed);
    let project = loaded.unwrap();
    eprintln!("published storage: retained={retained}, peak={peak} requested bytes");

    let references = project
        .find_references(ReferenceQuery::Name {
            name: "run0".into(),
            file: None,
        })
        .unwrap();
    assert_eq!(references.groups.len(), 1);
    assert_eq!(references.groups[0].occurrences.len(), 3);
    let result = project
        .type_of(TypeRequest::new(TypeQuery::At {
            file: fixture.path("main.ts"),
            position: position(main, main.find("result").unwrap()),
        }))
        .unwrap();
    let TypeAnswer::Selected(result) = result else {
        panic!("expected selected result")
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert_eq!(result.description.summary, "number");
    assert!(result.issues.is_empty());

    // Published storage keeps no growth slack: vectors that grew while
    // binding release their spare capacity when they are frozen, each bound
    // declaration's symbol is a dense slot rather than a keyed entry, and an
    // absent record id costs no more than a present one.
    assert!(
        retained < 6_700_000,
        "published facts exceed the 6,700,000-byte retained-heap budget: {retained} bytes (peak {peak})"
    );
}
