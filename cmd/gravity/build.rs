use rustc_version::{Channel, version_meta};

fn main() {
    assert!(
        version_meta().unwrap().channel == Channel::Nightly,
        "Gravity must be compiled with the nightly release of Rust"
    );
}
