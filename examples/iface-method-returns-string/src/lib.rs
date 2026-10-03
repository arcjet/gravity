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

    fn call_get_u8() -> u8 {
        runtime::get_u8()
    }

    fn call_get_s8() -> i8 {
        runtime::get_s8()
    }

    fn call_get_u16() -> u16 {
        runtime::get_u16()
    }

    fn call_get_s16() -> i16 {
        runtime::get_s16()
    }

    fn call_get_s32() -> i32 {
        runtime::get_s32()
    }

    fn call_get_u64() -> u64 {
        runtime::get_u64()
    }

    fn call_get_s64() -> i64 {
        runtime::get_s64()
    }

    fn call_get_f32() -> f32 {
        runtime::get_f32()
    }

    fn call_get_f64() -> f64 {
        runtime::get_f64()
    }

    fn call_get_char() -> char {
        runtime::get_char()
    }
}
