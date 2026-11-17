use gravity::concurrent_instances::host;

wit_bindgen::generate!({
    world: "concurrent-instances",
});

struct ConcurrentInstances;

export!(ConcurrentInstances);

impl Guest for ConcurrentInstances {
    fn join(parts: Vec<String>, sep: String) -> String {
        parts.join(&sep)
    }

    fn greet(n: u32) -> String {
        format!("hello, {}", host::name(n))
    }
}
