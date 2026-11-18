package examples

//go:generate cargo build -p example-basic --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-records --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-memory --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-iface-method-returns-string --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-instructions --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-regressions --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-variants --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-option-float-params --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-import-param-widths --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-concurrent-instances --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-export-memo --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-enum-collisions --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-enum-discriminants --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-result-errors --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-interface-exports --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-utf8-strings --target wasm32-unknown-unknown --release

//go:generate cargo run --bin gravity -- --world basic --output ./basic/basic.go ../target/wasm32-unknown-unknown/release/example_basic.wasm
//go:generate cargo run --bin gravity -- --world records --output ./records/records.go ../target/wasm32-unknown-unknown/release/example_records.wasm
//go:generate cargo run --bin gravity -- --world memory --output ./memory/memory.go ../target/wasm32-unknown-unknown/release/example_memory.wasm
//go:generate cargo run --bin gravity -- --world example --output ./iface-method-returns-string/example.go ../target/wasm32-unknown-unknown/release/example_iface_method_returns_string.wasm
//go:generate cargo run --bin gravity -- --world instructions --output ./instructions/bindings.go ../target/wasm32-unknown-unknown/release/example_instructions.wasm
//go:generate cargo run --bin gravity -- --world regressions --output ./regressions/regressions.go ../target/wasm32-unknown-unknown/release/example_regressions.wasm
//go:generate cargo run --bin gravity -- --world variants --output ./variants/variants.go ../target/wasm32-unknown-unknown/release/example_variants.wasm
//go:generate cargo run --bin gravity -- --world option-float-params --output ./option-float-params/option_float_params.go ../target/wasm32-unknown-unknown/release/example_option_float_params.wasm
//go:generate cargo run --bin gravity -- --world import-param-widths --output ./import-param-widths/import_param_widths.go ../target/wasm32-unknown-unknown/release/example_import_param_widths.wasm
//go:generate cargo run --bin gravity -- --world concurrent-instances --output ./concurrent-instances/concurrent_instances.go ../target/wasm32-unknown-unknown/release/example_concurrent_instances.wasm
//go:generate cargo run --bin gravity -- --world export-memo --output ./export-memo/export_memo.go ../target/wasm32-unknown-unknown/release/example_export_memo.wasm
//go:generate cargo run --bin gravity -- --world enum-collisions --output ./enum-collisions/enum_collisions.go ../target/wasm32-unknown-unknown/release/example_enum_collisions.wasm
//go:generate cargo run --bin gravity -- --world enum-discriminants --output ./enum-discriminants/enum_discriminants.go ../target/wasm32-unknown-unknown/release/example_enum_discriminants.wasm
//go:generate cargo run --bin gravity -- --world result-errors --output ./result-errors/result_errors.go ../target/wasm32-unknown-unknown/release/example_result_errors.wasm
//go:generate cargo run --bin gravity -- --world interface-exports --output ./interface-exports/interface_exports.go ../target/wasm32-unknown-unknown/release/example_interface_exports.wasm
//go:generate cargo run --bin gravity -- --world utf8-strings --output ./utf8-strings/utf8_strings.go ../target/wasm32-unknown-unknown/release/example_utf8_strings.wasm
