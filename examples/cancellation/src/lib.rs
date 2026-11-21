wit_bindgen::generate!({
    world: "cancellation",
});

struct Cancellation;

export!(Cancellation);

impl Guest for Cancellation {
    fn spin() -> Result<(), String> {
        loop {
            std::hint::black_box(());
        }
    }

    fn allocate(bytes: u32) -> Result<u32, String> {
        let filled = vec![1u8; bytes as usize];
        Ok(std::hint::black_box(filled).len() as u32)
    }
}
