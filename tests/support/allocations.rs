//! Count heap operations only on the calling test thread and inside the guard.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static EVENTS: Cell<Option<usize>> = const { Cell::new(None) };
}

struct Allocator;

fn event() {
    let _ = EVENTS.try_with(|events| {
        if let Some(n) = events.get() {
            events.set(Some(n + 1));
        }
    });
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        event();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        event();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        event();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        event();
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

pub fn assert_no_heap<T>(f: impl FnOnce() -> T) -> T {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            EVENTS.with(|events| events.set(None));
        }
    }
    EVENTS.with(|events| {
        assert!(events.get().is_none());
        events.set(Some(0));
    });
    let guard = Guard;
    let result = f();
    let events = EVENTS.with(|events| events.get().unwrap());
    drop(guard);
    assert_eq!(
        events, 0,
        "audio work allocated, reallocated or freed memory"
    );
    result
}
