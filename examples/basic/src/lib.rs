use arcjet::basic::logger;

wit_bindgen::generate!({
    world: "hello",
    path: "."
});

struct ExampleWorld;

export!(ExampleWorld);

impl Guest for ExampleWorld {
    fn hello() -> Result<String, String> {
        logger::debug("DEBUG MESSAGE");

        Ok("Hello, world!".into())
    }
}
