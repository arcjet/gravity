//! A guest whose exports are written against the canonical ABI by hand, so
//! they can hand the host bytes that are not UTF-8 without ever holding them
//! in a Rust `String`. `generate!` embeds the world for gravity to read and
//! provides the `host::text` import.

use std::alloc::{Layout, alloc};

use gravity::utf8_strings::host;

wit_bindgen::generate!({
    world: "utf8-strings",
});

/// Bytes that are not UTF-8, standing in for a guest with a bug.
static NOT_UTF8: [u8; 3] = [b'a', 0xff, b'b'];

// The host's `take` import, declared here because wit-bindgen's wrapper
// takes a `&str`, which cannot hold `NOT_UTF8`.
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "gravity:utf8-strings/host")]
unsafe extern "C" {
    #[link_name = "take"]
    fn host_take(ptr: *const u8, len: usize);
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn host_take(_ptr: *const u8, _len: usize) {
    unreachable!("the guest only runs as wasm32")
}

/// Leaks `words` and returns its address, as a return area.
fn return_area<const N: usize>(words: [u32; N]) -> *const u32 {
    Box::leak(Box::new(words)).as_ptr()
}

/// A `result<string, string>`'s ok case (tag 0) or a `result<_, string>`'s
/// err case (tag 1): the tag, then the string's pointer and length.
fn result_with_string(tag: u32, bytes: &[u8]) -> *const u32 {
    return_area([tag, bytes.as_ptr() as u32, bytes.len() as u32])
}

#[unsafe(export_name = "echo")]
extern "C" fn echo(ptr: *const u8, len: usize) -> *const u32 {
    return_area([0, ptr as u32, len as u32])
}

#[unsafe(export_name = "byte-length")]
extern "C" fn byte_length(_ptr: *const u8, len: usize) -> u32 {
    len as u32
}

#[unsafe(export_name = "invalid")]
extern "C" fn invalid() -> *const u32 {
    result_with_string(0, &NOT_UTF8)
}

#[unsafe(export_name = "invalid-error")]
extern "C" fn invalid_error() -> *const u32 {
    result_with_string(1, &NOT_UTF8)
}

#[unsafe(export_name = "invalid-plain")]
extern "C" fn invalid_plain() -> *const u32 {
    return_area([NOT_UTF8.as_ptr() as u32, NOT_UTF8.len() as u32])
}

#[unsafe(export_name = "from-host")]
extern "C" fn from_host() -> *const u32 {
    let text: &'static [u8] = host::text().into_bytes().leak();
    result_with_string(0, text)
}

#[unsafe(export_name = "to-host-invalid")]
extern "C" fn to_host_invalid() -> *const u32 {
    // SAFETY: the pointer and length describe `NOT_UTF8`, which outlives the
    // call.
    unsafe { host_take(NOT_UTF8.as_ptr(), NOT_UTF8.len()) };
    // The ok case of `result<_, string>`: tag 0 and no payload.
    return_area([0, 0, 0])
}

/// Allocates fresh memory for the host; the host never grows a block.
#[unsafe(export_name = "cabi_realloc")]
extern "C" fn cabi_realloc(
    _old: *mut u8,
    _old_len: usize,
    align: usize,
    new_len: usize,
) -> *mut u8 {
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
