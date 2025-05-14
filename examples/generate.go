package examples

//go:generate cargo build -p example-basic --target wasm32-unknown-unknown --release

//go:generate cargo run --bin gravity -- --world basic --output ./basic/basic.go ../target/wasm32-unknown-unknown/release/example_basic.wasm
