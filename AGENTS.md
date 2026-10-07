# Gravity

Gravity is a host code generator for WebAssembly Components. It takes a Core
Wasm file (with embedded WIT custom section) and generates Go bindings targeting
[wazero](https://wazero.io/), a zero-dependency WebAssembly runtime for Go.

## Pull Requests

Keep no more than 3 pull requests open against gravity at once. This applies
to agents and to human developers.

## Build & Run

Every routine operation is a `just` recipe; `just --list` shows them all.

### Prerequisites

- [just](https://just.systems/)
- Rust toolchain (version pinned in `rust-toolchain.toml`).
  `just install` installs it with the `wasm32-unknown-unknown` and
  `wasm32-wasip1` targets the file configures.
- Go (for running example tests)

The recipes ignore an inherited `RUSTUP_TOOLCHAIN` or `CARGO_TARGET_DIR`, so
they use the toolchain pinned in `rust-toolchain.toml` and the `target/`
directory that `examples/generate.go` reads from.

### Building

```sh
just build
```

### Running

Gravity takes a Core Wasm file and produces Go source + an optimized `.wasm`
file. `just run` passes its arguments to gravity:

```sh
just run --world <world-name> --output <output.go> <input.wasm>
```

## Testing

`just check` runs the same steps as `.github/workflows/ci.yml`, in the same
order: format check, build, unit tests, the Go example tests and the CLI
snapshot tests. Each step is also its own recipe, described below.

### Unit Tests

Run all unit tests (does NOT include snapshot/CLI tests):

```sh
just test
```

### Snapshot / CLI Tests

The CLI tests use `trycmd` for snapshot testing and read the example Wasm
files, so `just test-cli` builds every example first. They are excluded from
`just test`.

```sh
just test-cli
```

To update snapshot expectations when output changes intentionally, then read
the diff before committing it:

```sh
just update-snapshots
```

Snapshot files live in `cmd/gravity/tests/cmd/` (`.toml` for config, `.stdout`
and `.stderr` for expected output).

**Important**: The `trycmd` dependency has the `filesystem` feature disabled.
This means `just update-snapshots` can only update _existing_
`.stdout`/`.stderr` files — it cannot create new ones. `just new-snapshot
<name>` creates all three files for a new example from gravity's current
output; after that, `just update-snapshots` keeps them in sync.

### Example Go Tests

The examples include Go test files that verify the generated bindings work
end-to-end with wazero. `examples/generate.go` has `//go:generate` directives
that build each example's Wasm and run Gravity on it. `just generate` runs
them, and `just test-go` runs them and then the Go tests:

```sh
just test-go
```

## Project Structure

```text
cmd/gravity/
  src/
    main.rs              # CLI entry point (clap argument parsing)
    lib.rs               # Library entry point
    codegen/
      mod.rs             # Codegen module root
      bindings.rs        # Top-level bindings generation (file structure, wasm embed)
      factory.rs         # Factory function codegen (instantiation boilerplate)
      imports.rs         # Import function analysis and host function codegen
      exports.rs         # Export function codegen (calling guest functions from Go)
      func.rs            # Instruction handler (canonical ABI instruction → Go code)
      ir.rs              # Intermediate representation types
      wasm.rs            # Wasm file processing and optimization
    go/
      mod.rs             # Go type representations module root
      type.rs            # Go type system (GoType enum, resolve_wasm_type)
      identifier.rs      # Go identifier naming conventions
      comment.rs         # Go comment formatting
      embed.rs           # Go embed directive generation
      imports.rs         # Go import path management
      operand.rs         # Operand type for code generation (Literal, SingleValue, MultiValue)
      result.rs          # GoResult type (Empty, Anon, Named)
  tests/
    cli.rs               # trycmd-based CLI snapshot test runner
    cmd/                  # Snapshot test data (*.toml, *.stdout, *.stderr)

examples/
  generate.go            # go:generate directives for building examples
  .gitignore             # Ignores generated *.go and *.wasm; keeps *_test.go
  basic/                 # Simple world with basic types
  iface-method-returns-string/  # Interface method returning a string
  instructions/          # Tests various canonical ABI instructions
  regressions/           # Regression tests for import codegen edge cases

justfile                 # Recipes for building, testing and the CI gate
```

**Note on generated files**: `examples/.gitignore` ignores all `*/*.go` files
(except `*/*_test.go`) and all `*/*.wasm` files. This means the generated Go
bindings and Wasm binaries are not committed — only the test files, WIT
definitions, and Rust source are tracked. The Go tests compile only after
`just generate` has run, which `just test-go` does first. After switching to a
branch without an example, its directory holds only these generated files, and
the `examples/*` workspace glob then fails every cargo command with "failed to
load manifest"; `just clean-examples` removes them.

## Architecture

### Key Concepts

- **WIT (WebAssembly Interface Types)**: The interface definition language for
  the Component Model. Gravity reads WIT embedded in Core Wasm files.
- **wit-bindgen-core**: Bytecode Alliance library (version pinned in
  `cmd/gravity/Cargo.toml`) that Gravity depends on for canonical ABI
  instruction generation. It provides the `Instruction` enum that `func.rs`
  handles.
- **Direction (Import vs Export)**: Gravity generates different code depending on
  whether a function is an import (Go host function called by Wasm guest) or an
  export (Wasm guest function called from Go). The `Func` struct tracks this via
  its `Direction` enum.
- **genco**: Code generation library used for building Go source via `quote_in!`
  macros.

### Codegen Pipeline

1. `wasm.rs` — Reads and optimizes the input Wasm file
2. `bindings.rs` — Orchestrates overall file generation
3. `imports.rs` — Analyzes import functions, determines Go signatures (params,
   results), and generates host function registration code
4. `exports.rs` — Generates Go wrapper functions for calling Wasm exports
5. `func.rs` — Handles individual canonical ABI instructions
   (`Instruction::I32FromU32`, `Instruction::CallWasm`, etc.), converting them
   to Go code snippets
6. `factory.rs` — Generates the factory/instantiation boilerplate

### Important Implementation Details

- **Value representation.** Lowering instructions produce Go values of the core
  Wasm type their consumer expects. Integer lowerings (`I32FromS32`,
  `I32FromU8`, …, `I64FromS64`) emit plain `uint32(x)`/`uint64(x)` conversions
  in both directions, and `CallWasm` widens every export argument with
  `uint64(...)`. Float lowerings depend on `Direction`: imports pass
  `float32`/`float64` through, because a host function's Go signature is its
  Wasm signature, while exports encode the IEEE bits with
  `api.EncodeF32`/`EncodeF64`. Integer lifts are Go conversions (`int8(x)`,
  `uint16(x)`), which accept both the `uint64` that `CallWasm` returns and the
  narrower value a load produces. Every store writes exactly its own width with
  an explicit conversion. When you change the Go type an instruction produces,
  check every instruction that consumes it (stores, `VariantLower`, `CallWasm`)
  in both directions. The `memory` example sends every scalar type through all
  four memory paths.
- Import functions with simple return types (bool, enum) that map to Wasm i32
  results use `resolve_wasm_type()` on `wasm_sig.results` to determine the Go
  return type.
- In `generate_host_function_builder` (imports.rs), all host function parameters
  (the fixed `ctx`/`mod` params and any WIT-level params) are collected into a
  single `Vec` before the `quote!` template. This ensures the `join` macro
  produces correct commas even when there are zero WIT-level parameters.

## Key Dependencies

| Crate            | Purpose                                  |
| ---------------- | ---------------------------------------- |
| wit-bindgen-core | Canonical ABI instruction generation     |
| wit-component    | Wasm component model processing          |
| genco            | Code generation with Go language support |
| clap             | CLI argument parsing                     |
| trycmd           | CLI snapshot testing (dev)               |

`wit-bindgen`, `wit-bindgen-core` and `wit-component` move as one set: each
`wit-bindgen` release requires exactly one `wit-parser`/`wit-component` minor
(read `wit-bindgen-rust`'s dependencies on crates.io), and two `wit-parser`
versions in one build fail to compile with mismatched `Resolve`/`World` types.
Bump every example's pins with them.

## Style & Conventions

- Use conventional commits: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, etc.
- Format code before committing with `just format`
- Lint with `just clippy` (CI does not run clippy)
- When adding new instruction handlers in `func.rs`, add corresponding entries
  in the `instructions` example and update snapshot tests
- When changing codegen output, update snapshot `.stdout` files with
  `just update-snapshots`

## Adding a New Example

1. Create `examples/<name>/` with `Cargo.toml` (`crate-type = ["cdylib"]`),
   `wit/<name>.wit`, and `src/lib.rs`
2. The workspace `Cargo.toml` uses `members = ["examples/*"]`, so new crates are
   picked up automatically
3. Add `//go:generate` directives to `examples/generate.go` (one for
   `cargo build`, one for `cargo run --bin gravity`)
4. Write `examples/<name>/<name>_test.go` (this file IS committed)
5. Create the snapshot test files with `just new-snapshot <name>`, or
   `just new-snapshot <name> <world>` when the world is not named for the
   example. It builds the example and writes three files to
   `cmd/gravity/tests/cmd/`:
   - `<name>.toml` — trycmd config (`bin.name = "gravity"`, `args = "..."`)
   - `<name>.stdout` — gravity's output for the example
   - `<name>.stderr` — empty
6. Verify: `just check`
