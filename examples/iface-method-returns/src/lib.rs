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

    fn call_get_u32() -> u32 {
        runtime::get_u32()
    }
}
