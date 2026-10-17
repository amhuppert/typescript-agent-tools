//! Disposable requested-allocation accounting, confined to copied source.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
pub(crate) static LIVE: AtomicUsize = AtomicUsize::new(0);
pub(crate) static CALLS: AtomicUsize = AtomicUsize::new(0);
pub(crate) static TOTAL: AtomicUsize = AtomicUsize::new(0);
struct Allocator;
fn allocated(size: usize) {
    LIVE.fetch_add(size, Ordering::Relaxed);
    TOTAL.fetch_add(size, Ordering::Relaxed);
    CALLS.fetch_add(1, Ordering::Relaxed);
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() { allocated(layout.size()); }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() { allocated(layout.size()); }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(pointer, layout, size) };
        if !next.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            allocated(size);
        }
        next
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
