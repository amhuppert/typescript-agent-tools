// Allocation accounting is isolated from throughput benchmarks.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
mod support;
struct Allocator;
static ENABLED: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
fn allocated(size: usize) {
    if ENABLED.load(Ordering::Relaxed) {
        CALLS.fetch_add(1, Ordering::Relaxed);
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
fn main() {
    let args: Vec<_> = std::env::args()
        .skip(1)
        .filter(|arg| arg != "--bench")
        .collect();
    let sources = if args.is_empty() {
        vec![support::Source {
            name: "functions.ts".into(),
            jsx: false,
            text: "export function sum<T extends number>(x: T, y: T): number { return x + y; }\n"
                .repeat(1_000),
        }]
    } else {
        support::sources(&args)
    };
    assert!(!sources.is_empty(), "Supply TypeScript source paths");
    println!("file,input_bytes,nodes,allocation_calls,retained_bytes,peak_bytes");
    for source in &sources {
        drop(support::parse(source)); // Warm up runtime data before accounting.
        CALLS.store(0, Ordering::Relaxed);
        LIVE.store(0, Ordering::Relaxed);
        PEAK.store(0, Ordering::Relaxed);
        ENABLED.store(true, Ordering::Relaxed);
        let ast = support::parse(source);
        let nodes = ast.nodes().len();
        let calls = CALLS.load(Ordering::Relaxed);
        let retained = LIVE.load(Ordering::Relaxed);
        let peak = PEAK.load(Ordering::Relaxed);
        drop(ast);
        let remaining = LIVE.load(Ordering::Relaxed);
        ENABLED.store(false, Ordering::Relaxed);
        assert_eq!(remaining, 0, "Unbalanced parser allocations");
        println!(
            "{},{},{},{},{},{}",
            source.name,
            source.text.len(),
            nodes,
            calls,
            retained,
            peak
        );
    }
}
