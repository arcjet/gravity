//! A guest whose exports are written against the canonical ABI by hand, so
//! `surrogate` can return a code point no Rust `char` may hold without ever
//! making one. `generate!` only embeds the world for gravity to read.

wit_bindgen::generate!({
    world: "invalid-values",
});

/// U+D800, a surrogate: a `u32` no Rust `char` may hold.
const SURROGATE: u32 = 0xD800;

/// Leaks `words` and returns its address, as a return area.
fn return_area<const N: usize>(words: [u32; N]) -> *const u32 {
    Box::leak(Box::new(words)).as_ptr()
}

/// The name of the `color` case with discriminant `c`.
fn name(c: u32) -> Option<&'static str> {
    match c {
        0 => Some("red"),
        1 => Some("green"),
        2 => Some("blue"),
        _ => None,
    }
}

/// A `result<string, string>`: tag 0 with the case's name, or tag 1 with an
/// error message for a discriminant that is none of the cases.
#[unsafe(export_name = "name")]
extern "C" fn name_result(c: u32) -> *const u32 {
    let (tag, text) = match name(c) {
        Some(name) => (0, name),
        None => (1, "no such color"),
    };
    return_area([tag, text.as_ptr() as u32, text.len() as u32])
}

#[unsafe(export_name = "name-plain")]
extern "C" fn name_plain(c: u32) -> *const u32 {
    let text = name(c).unwrap_or("");
    return_area([text.as_ptr() as u32, text.len() as u32])
}

/// A `result<char, string>`'s ok case: tag 0 and the code point at offset 4.
#[unsafe(export_name = "surrogate")]
extern "C" fn surrogate() -> *const u32 {
    return_area([0, SURROGATE])
}
