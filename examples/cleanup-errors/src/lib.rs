use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, Ordering},
};

wit_bindgen::generate!({
    world: "cleanup-errors",
});

/// When set, the next deallocation traps. `cabi_post_*` frees the string an
/// export returned, so setting it as an export returns makes the cleanup
/// trap.
static TRAP_NEXT_DEALLOC: AtomicBool = AtomicBool::new(false);

struct TrapOnDealloc;

// SAFETY: every call is forwarded to the system allocator unchanged.
unsafe impl GlobalAlloc for TrapOnDealloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if TRAP_NEXT_DEALLOC.swap(false, Ordering::Relaxed) {
            std::process::abort();
        }
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: TrapOnDealloc = TrapOnDealloc;

/// Returns `value`, arranging for the next deallocation, which is the
/// cleanup's, to trap.
fn then_trap<T>(value: T) -> T {
    TRAP_NEXT_DEALLOC.store(true, Ordering::Relaxed);
    value
}

struct CleanupErrors;

export!(CleanupErrors);

impl Guest for CleanupErrors {
    fn value() -> Result<String, String> {
        Ok("value".into())
    }

    fn fail_value() -> Result<String, String> {
        let value = "value".to_string();
        then_trap(Ok(value))
    }

    fn fail_error() -> Result<(), String> {
        let message = "guest error".to_string();
        then_trap(Err(message))
    }

    fn fail_plain() -> String {
        let value = "value".to_string();
        then_trap(value)
    }
}
