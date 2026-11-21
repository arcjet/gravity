wit_bindgen::generate!({
    world: "invalid-values",
});

struct InvalidValues;

export!(InvalidValues);

fn name(c: Color) -> String {
    match c {
        Color::Red => "red",
        Color::Green => "green",
        Color::Blue => "blue",
    }
    .into()
}

impl Guest for InvalidValues {
    fn name(c: Color) -> Result<String, String> {
        Ok(name(c))
    }

    fn name_plain(c: Color) -> String {
        name(c)
    }

    fn surrogate() -> Result<char, String> {
        // SAFETY: deliberately invalid, standing in for a guest with a bug.
        // The char is only lowered by wit-bindgen, which passes it on as its
        // u32 value.
        Ok(unsafe { char::from_u32_unchecked(0xD800) })
    }
}
