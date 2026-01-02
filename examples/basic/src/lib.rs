use arcjet::basic::{logger, utils};

wit_bindgen::generate!({
    world: "basic",
});

struct BasicWorld;

export!(BasicWorld);

impl Guest for BasicWorld {
    fn hello() -> Result<String, String> {
        logger::debug(&utils::uppercase("debug message"));

        Ok("Hello, world!".into())
    }
    fn primitive() -> bool {
        true
    }
    fn optional_primitive(_: Option<bool>) -> Option<bool> {
        Some(true)
    }
    fn result_primitive() -> Result<bool, String> {
        Ok(true)
    }
    fn optional_string(s: Option<String>) -> Option<String> {
        s
    }
}
