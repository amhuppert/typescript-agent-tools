//! Loading many sibling index modules must not allocate a path for every
//! importer/target pair. Count every load worker, not just the calling thread.
//! Keep this integration binary single-test: its counters are process-wide.
mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use support::Fixture;
use typescript_parser::references::ReferenceQuery;

struct Allocator;
static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

fn allocated(bytes: usize) {
    if COUNTING.load(Ordering::Relaxed) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(bytes, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc(layout) };
        if !result.is_null() {
            allocated(layout.size());
        }
        result
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc_zeroed(layout) };
        if !result.is_null() {
            allocated(layout.size());
        }
        result
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(ptr, layout, size) };
        if !result.is_null() {
            allocated(size);
        }
        result
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

fn load_siblings(count: usize) -> (usize, usize) {
    let fixture = Fixture::new(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"noLib":true,"types":[]},"include":["src"]}"#,
    )]);
    for i in 0..count {
        let file = fixture.path(&format!("src/module{i:04}/index.ts"));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "export const value = 1; value;\n").unwrap();
    }
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    let project = fixture.load();
    COUNTING.store(false, Ordering::Relaxed);
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes = ALLOCATED_BYTES.load(Ordering::Relaxed);

    // TypeScript gives each module its own symbol, with one declaration and
    // one use. Assert the final query, so skipping inputs cannot meet the budget.
    let result = project
        .find_references(ReferenceQuery::Name {
            name: "value".into(),
            file: None,
        })
        .unwrap();
    assert_eq!(result.groups.len(), count);
    for (i, group) in result.groups.iter().enumerate() {
        assert_eq!(group.declarations.len(), 1);
        assert_eq!(group.occurrences.len(), 2);
        assert_eq!(
            group.declarations[0].location.file,
            format!("src/module{i:04}/index.ts")
        );
    }
    (allocations, bytes)
}

#[test]
fn sibling_index_modules_do_not_allocate_quadratically_when_loaded() {
    const SMALL: usize = 64;
    const LARGE: usize = 256;
    const GROWTH: usize = LARGE / SMALL;
    const EXTRA_PAIRS: usize = LARGE * LARGE - GROWTH * SMALL * SMALL;
    load_siblings(1); // Exclude one-time initialization from the scaling check.
    let (small, small_bytes) = load_siblings(SMALL);
    let (large, large_bytes) = load_siblings(LARGE);
    eprintln!(
        "{SMALL} -> {LARGE} modules: {small} -> {large} allocations; {small_bytes} -> {large_bytes} requested allocation bytes (including full reallocation sizes)"
    );
    // Allow bounded bookkeeping growth beyond 4x, but less than one string
    // allocation or one stored Shape per extra directory/target pair. A loose
    // ratio can hide that quadratic work behind the rest of project loading.
    assert!(
        large.saturating_sub(GROWTH * small) < EXTRA_PAIRS / 4,
        "allocation growth exceeds the linear-load allowance: {small} -> {large}"
    );
    assert!(
        large_bytes.saturating_sub(GROWTH * small_bytes) < EXTRA_PAIRS * 8,
        "allocated-byte growth exceeds the linear-load allowance: {small_bytes} -> {large_bytes}"
    );
}
