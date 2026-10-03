use arcjet::memory::host;

wit_bindgen::generate!({
    world: "memory",
});

struct MemoryWorld;

export!(MemoryWorld);

impl Guest for MemoryWorld {
    fn round_trip(x: Everything) -> Everything {
        x
    }

    fn call_host_echo(x: Everything) -> Everything {
        host::echo(&x)
    }

    fn round_trip_narrow(x: Narrow) -> Narrow {
        x
    }
}
