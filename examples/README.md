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
This can be built via [Cargo Examples][cargo-examples]

```sh
cargo build --example examples --target wasm32-unknown-unknown
```

## 3. Run Gravity against the Core Wasm file

Gravity can be run against the Wasm file produced in Rust's `target/` directory.

```sh
cargo run target/wasm32-unknown-unknown/debug/examples/examples.wasm  -o examples/examples.go --world examples
```

## 4. Use the generate Go & Wasm files

The above command will produce a `examples.go` and `examples.wasm` file inside
the `examples/` directory. These could be used within a Go project.

[cargo-examples]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html#examples
