use arcjet::example::runtime;

wit_bindgen::generate!({
    world: "example",
});

struct ExampleWorld;

export!(ExampleWorld);

impl Guest for ExampleWorld {
    fn hello() -> Result<String, String> {
        runtime::puts(&format!("{}/{}", runtime::os(), runtime::arch()));

        Ok("Hello, world!".into())
    }
}
