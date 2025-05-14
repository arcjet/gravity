# Example

This example is provided as a guide for using a `wit-bindgen` guest written in
Rust with Gravity.

## 1. Add the WebAssembly target

Gravity doesn't use WebAssembly itself, so you'll want to add the
`wasm32-unknown-unknown` target to your toolchain.

```sh
rustup target add wasm32-unknown-unknown
```

## 2. Build the example to Core Wasm

Gravity currently needs a "Core Wasm" file with an embedded WIT custom section.
This can be built using `cargo build` using the
`--target wasm32-unknown-unknown` flag.

```sh
cargo build -p example-basic --target wasm32-unknown-unknown --release
```

## 3. Run Gravity against the Core Wasm file

Gravity can be run against the Wasm file produced in Rust's `target/` directory.

```sh
cargo run --bin gravity -- --world basic --output examples/basic/basic.go target/wasm32-unknown-unknown/release/example_basic.wasm
```

## 4. Use the generate Go & Wasm files

The above command will produce a `basic.go` and `basic.wasm` file inside the
`examples/basic` directory. These could be used within a Go project.
