wit_bindgen::generate!({
    world: "records",
});

struct RecordsWorld;

export!(RecordsWorld);

impl Guest for RecordsWorld {
    fn modify_foo(
        Foo {
            float64,
            float32,
            uint32,
            uint64,
            s,
            vf32,
            vf64,
        }: Foo,
    ) -> Foo {
        Foo {
            float64: float64 * 2.0,
            float32: float32 * 2.0,
            uint32: uint32 + 1,
            uint64: uint64 + 1,
            s: format!("received {s}"),
            vf32: vf32.iter().map(|v| v * 2.0).collect(),
            vf64: vf64.iter().map(|v| v * 2.0).collect(),
        }
    }
}
