use gravity::utf8_strings::host;

wit_bindgen::generate!({
    world: "utf8-strings",
});

struct Utf8Strings;

export!(Utf8Strings);

/// A `String` holding bytes that are not UTF-8, standing in for a guest with
/// a bug. Breaking `String`'s invariant is deliberate: the bindings only copy
/// the bytes, and the host must refuse them.
fn not_utf8() -> String {
    // SAFETY: the string is only lowered by wit-bindgen, which copies its
    // bytes without reading them as UTF-8.
    unsafe { String::from_utf8_unchecked(vec![b'a', 0xff, b'b']) }
}

impl Guest for Utf8Strings {
    fn echo(s: String) -> Result<String, String> {
        Ok(s)
    }

    fn byte_length(s: String) -> u32 {
        s.len() as u32
    }

    fn invalid() -> Result<String, String> {
        Ok(not_utf8())
    }

    fn invalid_error() -> Result<(), String> {
        Err(not_utf8())
    }

    fn invalid_plain() -> String {
        not_utf8()
    }

    fn from_host() -> Result<String, String> {
        Ok(host::text())
    }

    fn to_host_invalid() -> Result<(), String> {
        host::take(&not_utf8());
        Ok(())
    }
}
