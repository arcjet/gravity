use arcjet::basic::logger;

wit_bindgen::generate!({
    world: "basic",
});

struct BasicWorld;

export!(BasicWorld);

impl Guest for BasicWorld {
    fn hello() -> Result<String, String> {
        logger::debug("DEBUG MESSAGE");

        Ok("Hello, world!".into())
    }
}
