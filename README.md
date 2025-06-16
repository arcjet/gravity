> [!IMPORTANT]
> This is a very early release of Gravity. Many WIT features are not yet implemented
> and the project is is likely to change significantly as we develop it further.

# Gravity

Gravity is a host generator for WebAssembly Components. It currently targets
[Wazero][wazero], a zero dependency WebAssembly runtime for Go.

## What?

This crate provides the `gravity` tool—a code generator that produces Wazero
host code for WebAssembly Components. Currently, we only process Wasm core
modules with a WIT metadata custom section.

## Why?

Much of Arcjet's protection rules are written in Rust & compiled to WebAssembly.
To allow us to use rich types at the Wasm boundary, we leverage the [WebAssembly
Interface Type][wit] format (or WIT). Our Rust code consumes the
[wit-bindgen][wit-bindgen] project which generates the lifting and lowering of
these types inside the "guest" WebAssembly module. However, the only way to
"host" one of these WebAssembly Components is via [Wasmtime][wasmtime] or
[jco][jco].

We were able to leverage `jco transpile` to translate our WebAssembly Components
to Core Wasm that runs in a JavaScript environment, but we don't have easy
access to Wasmtime in our server environment. Most of our server logic is
written in Go, which has fantastic Core Wasm support via [Wazero][wazero].
Wazero has [rejected the Component Model][wazero-component-model], but we can
still translate Components to Core today.

By adopting a similar strategy as `jco transpile`, we've built this tool to
produced Wazero output that adheres to the Component Model's [Canonical
ABI][canonical-abi].

## Installation

To produce Go files with good indentation, this tool should be installed with a
Rust nightly toolchain. You can install one with:

```bash
rustup toolchain install nightly-2025-01-01
```

From inside this directory, you can install using the command:

```bash
cargo +nightly-2025-01-01 install --path cmd/gravity
```

Or alternatively, you can install the latest published version from crates.io
using this command:

```bash
cargo +nightly-2025-01-01 install arcjet-gravity
```

## Usage

To generate the bindings, you run something like:

```bash
gravity example/example.wasm --world example --output example/example.go
```

After you generate the code, you'll want to ensure you have all the necessary
dependencies. You can run:

```bash
go mod tidy
```

## Example

An runnable example in our [examples/](./examples/) directory. Please see the
[README](./examples/README.md) for instructions on running it.

## Status


Currently, that means we support:

- `string`
- `u32`
- `result<string, string>`
- `result<_, string>`
- `option<string>`

This list is likely to grow quickly, as one of our goals is to avoid working
with JSON serialized as a string and instead leverage more concrete types that
we can codegen.

## Output

The generated output consists of a bindings file and a Wasm file which
is placed next to it. The bindings file loads the Wasm file using `go:embed`.

Alternatively, if you set the `inline-wasm` flag Gravity will output the Wasm
file contents encoded as hex if you wish to avoid using `go:embed`. This will likely
result in much larger file sizes.

We produce a "factory" and "instance" per world. Given an `example` world:

```txt
package arcjet:example;

interface logger {
  debug: func(msg: string);
  log: func(msg: string);
  warn: func(msg: string);
  error: func(msg: string);
}

world example {
  import logger;

  export foobar: func() -> result<string, string>;
}
```

The generated code will define the `ExampleFactory` and `ExampleInstance`. Generally,
the factory is constructed once upon startup because it prepares all of the
imports and compiles the WebAssembly, which can take a long time. In the example
above, the `ExampleFactory` can be constructed with `NewExampleFactory` which is
provided with a `context.Context` and a type implementing the `IExampleLogger`
interface.

Any interfaces defined as imports to the world will have a corresponding
interface definition in Go, as we saw the `IExampleLogger` above. This defines the
high-level functions that must be available to call from Wasm. The `logger`
interface was translated to:

```go
type IExampleLogger interface {
  Debug(ctx context.Context, msg string)
  Log(ctx context.Context, msg string)
  Warn(ctx context.Context, msg string)
  Error(ctx context.Context, msg string)
}
```

Factories can produce instances using the `Instantiate` function, which only
takes a `context.Context`. This function prepares the WebAssembly to be executed
but is generally very fast, since the factory pre-compiles the Wasm module.

Exported functions are called on an instance, such as our `foobar` function. You
would call this like
`inst.Foobar(ctx)`. Since
the return value is defined as a `result<string, string>`, it is translated into
the idiomatic Go return type `(string, error)`.

When you are done with an instance, you are expected to call `Close` but you'll
probably just want to `defer` it, like `defer inst.Close(ctx)`.

### Testing

Consuming the generated bindings should be pretty straightforward. As such,
writing a test for the above would look something like:

```go
package example

import (
  "context"
  "testing"

  "github.com/stretchr/testify/require"
)

func Test_Generated_Example(t *testing.T) {
  // Assuming you've generated mocks with Mockery
  logger := NewMockIBotsLogger(t)
  ctx := context.Background()
  factory, err := NewExampleFactory(ctx, logger)
  require.NoError(t, err)

  instance, err := factory.Instantiate(ctx)
  require.NoError(t, err)
  defer instance.Close(ctx)

  result, err := instance.Foobar(ctx)
  require.NoError(t, err)
  require.NotEqual(t, result, "")
}
```

[wit]: https://github.com/WebAssembly/component-model/blob/a74225c12c152df59f745cfc0fbde79b5310ccd9/design/mvp/WIT.md
[wit-bindgen]: https://github.com/bytecodealliance/wit-bindgen
[wasmtime]: https://wasmtime.dev/
[jco]: https://github.com/bytecodealliance/jco
[wazero]: https://github.com/tetratelabs/wazero
[canonical-abi]: https://github.com/WebAssembly/component-model/blob/a74225c12c152df59f745cfc0fbde79b5310ccd9/design/mvp/CanonicalABI.md
[wazero-component-model]: https://github.com/tetratelabs/wazero/issues/2200
