package examples

//go:generate cargo build --target wasm32-unknown-unknown --release

//go:generate cargo run --bin gravity -- --world hello --output ./basic/basic.go ../target/wasm32-unknown-unknown/release/example_basic.wasm
//go:generate cargo run --bin gravity -- --world hello --output ./iface-method-returns-string/basic.go ../target/wasm32-unknown-unknown/release/example_iface_method_returns_string.wasm
