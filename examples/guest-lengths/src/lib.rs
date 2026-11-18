//! A guest whose exports are written against the canonical ABI by hand, so
//! they can return lengths and blocks a wit-bindgen guest never would.
//! `generate!` only embeds the world for gravity to read.

use std::{
    alloc::{Layout, alloc},
    sync::atomic::{AtomicBool, Ordering},
};

wit_bindgen::generate!({
    world: "guest-lengths",
});

/// An address past the end of guest memory.
const OUTSIDE_MEMORY: usize = 0xFFFF_FF00;

/// Elements in the list `long-list` returns: 4 MiB of u32s, more than guest
/// memory holds.
const LONG: u32 = 1 << 20;

static BREAK_REALLOC: AtomicBool = AtomicBool::new(false);

/// Leaks `words` and returns its address, as a return area.
fn return_area<const N: usize>(words: [u32; N]) -> *const u32 {
    Box::leak(Box::new(words)).as_ptr()
}

/// A `result<u32, string>`'s ok case: tag 0 and the value at offset 4.
fn ok_u32(value: u32) -> *const u32 {
    return_area([0, value])
}

/// Reads `len` u32s at `ptr`.
///
/// # Safety
///
/// `ptr` and `len` must describe u32s in guest memory.
unsafe fn read_u32s<'a>(ptr: *const u32, len: usize) -> &'a [u32] {
    if len == 0 {
        return &[];
    }
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

#[unsafe(export_name = "long-list")]
extern "C" fn long_list() -> *const u32 {
    return_area([0, LONG])
}

#[unsafe(export_name = "long-list-result")]
extern "C" fn long_list_result() -> *const u32 {
    // Tag 0 (ok), then the list's pointer and length.
    return_area([0, 0, LONG])
}

#[unsafe(export_name = "sum")]
extern "C" fn sum(ptr: *const u32, len: usize) -> *const u32 {
    // SAFETY: the host lowered `len` u32s at `ptr`.
    ok_u32(unsafe { read_u32s(ptr, len) }.iter().sum())
}

#[unsafe(export_name = "count")]
extern "C" fn count(_ptr: *const u8, len: usize) -> *const u32 {
    ok_u32(len as u32)
}

#[unsafe(export_name = "wide")]
extern "C" fn wide(params: *const u32) -> *const u32 {
    // SAFETY: the host stored the 17 u32 parameters at `params`.
    ok_u32(unsafe { read_u32s(params, 17) }.iter().sum())
}

#[unsafe(export_name = "break-realloc")]
extern "C" fn break_realloc() {
    BREAK_REALLOC.store(true, Ordering::Relaxed);
}

/// Allocates fresh memory for the host; the host never grows a block.
#[unsafe(export_name = "cabi_realloc")]
extern "C" fn cabi_realloc(
    _old: *mut u8,
    _old_len: usize,
    align: usize,
    new_len: usize,
) -> *mut u8 {
    if BREAK_REALLOC.swap(false, Ordering::Relaxed) {
        return OUTSIDE_MEMORY as *mut u8;
    }
    if new_len == 0 {
        return align as *mut u8;
    }
    let layout = Layout::from_size_align(new_len, align).expect("a valid layout");
    // SAFETY: the layout's size is not zero.
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() {
        std::process::abort();
    }
    ptr
}
