use arcjet::examples::logger;

wit_bindgen::generate!({
    world: "examples",
    path: "./examples"
});

struct ExampleWorld;

export!(ExampleWorld);

impl Guest for ExampleWorld {
    fn foobar() -> Result<String, String> {
        logger::debug("DEBUG MESSAGE");

        Ok("Baz".into())
    }
}
