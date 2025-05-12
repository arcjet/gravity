use arcjet::basic::runtime;

wit_bindgen::generate!({
    world: "hello",
    path: "."
});

struct ExampleWorld;

export!(ExampleWorld);

impl Guest for ExampleWorld {
    fn hello() -> Result<String, String> {
        runtime::puts(&format!("{}/{}", runtime::os(), runtime::arch()));

        Ok("Hello, world!".into())
    }
}
