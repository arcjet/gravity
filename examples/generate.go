package examples

//go:generate cargo build -p example-basic --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-iface-method-returns --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-instructions --target wasm32-unknown-unknown --release

//go:generate cargo run --bin gravity -- --world basic --output ./basic/basic.go ../target/wasm32-unknown-unknown/release/example_basic.wasm
//go:generate cargo run --bin gravity -- --world example --output ./iface-method-returns/example.go ../target/wasm32-unknown-unknown/release/example_iface_method_returns.wasm
//go:generate cargo run --bin gravity -- --world instructions --output ./instructions/bindings.go ../target/wasm32-unknown-unknown/release/example_instructions.wasm
