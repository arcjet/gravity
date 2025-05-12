fn main() {
    assert!(
        rustversion::cfg!(nightly),
        "Gravity must be compiled with the nightly release of Rust"
    );
}
