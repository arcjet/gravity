# Gravity command runner.
# Run `just --list` to see available recipes, or `just <recipe>` to run one.
#
# `just check` runs the same steps as .github/workflows/ci.yml, in the same
# order. Change both together.

# show available recipes (default when no recipe is given)
[private]
default:
    @just --list

# rust-toolchain.toml pins the toolchain CI uses; an inherited override would
# replace it.
unexport RUSTUP_TOOLCHAIN

# examples/generate.go builds into and reads from ../target, so a relocated
# target directory makes `go generate` fail with "unable to read file".
unexport CARGO_TARGET_DIR

# styled header: bold cyan ✦, bold white label
h := BOLD + CYAN + "✦" + WHITE + " "
n := NORMAL

# ─── Root commands ───────────────────────────────────────────────────────────────

# run the full CI gate: format check, build, unit tests, Go example tests, CLI
# snapshot tests
[doc('run the full CI gate')]
[group('ci')]
check: lint build test test-go test-cli

# install the toolchain, components and targets pinned in rust-toolchain.toml
[group('setup')]
install:
    @echo '{{ h }}install: rustup toolchain{{ n }}'
    @rustup toolchain install --no-self-update

# auto-format Rust code and this justfile
[group('format')]
format:
    @echo '{{ h }}format: cargo fmt{{ n }}'
    @cargo fmt --all
    @echo '{{ h }}format: justfile{{ n }}'
    @just --fmt

# check formatting of Rust code and this justfile
[group('ci')]
[group('lint')]
lint:
    @echo '{{ h }}lint: cargo fmt{{ n }}'
    @cargo fmt --all --check
    @echo '{{ h }}lint: justfile{{ n }}'
    @just --fmt --check

# lint with clippy. Not part of `check`, because CI does not run clippy.
[doc('lint with clippy (not part of the CI gate)')]
[group('lint')]
clippy:
    @echo '{{ h }}lint: cargo clippy{{ n }}'
    @cargo clippy --workspace --all-targets --locked

# ─── Build and test ──────────────────────────────────────────────────────────────

# build the workspace
[group('build')]
[group('ci')]
build:
    @echo '{{ h }}build: cargo build{{ n }}'
    @cargo build --locked

# run the Rust unit tests. The CLI snapshot tests are excluded; see `test-cli`.
[doc('run the Rust unit tests')]
[group('ci')]
[group('test')]
test:
    @echo '{{ h }}test: cargo test{{ n }}'
    @cargo test --locked

# run gravity with the given arguments, e.g.
# `just run --world basic --output basic.go basic.wasm`
[doc('run gravity with the given arguments')]
[group('build')]
[positional-arguments]
run *args:
    @cargo run --quiet --locked --bin gravity -- "$@"

# build the example wasm and generate their Go bindings
[group('build')]
generate:
    @echo '{{ h }}generate: example wasm and Go bindings{{ n }}'
    @go generate ./...

# run the Go example tests against freshly generated bindings
[group('ci')]
[group('test')]
test-go: generate
    @echo '{{ h }}test: go test{{ n }}'
    @go test ./...

# run the trycmd CLI snapshot tests. They read the example wasm that
# `generate` builds.
[doc('run the trycmd CLI snapshot tests')]
[group('ci')]
[group('test')]
test-cli: generate
    @echo '{{ h }}test: cargo test --test cli{{ n }}'
    @cargo test --locked --test cli

# rewrite the CLI snapshot .stdout and .stderr files from current output. Read
# the diff before committing it. trycmd cannot create a snapshot file, so a new
# test's files must exist first; see AGENTS.md.
[doc('rewrite existing CLI snapshot files from current output')]
[group('test')]
update-snapshots: generate
    @echo '{{ h }}test: cargo test --test cli (overwrite){{ n }}'
    @TRYCMD=overwrite cargo test --locked --test cli

# create the CLI snapshot files for the example in examples/<name>: a .toml
# that runs gravity on the example's wasm, the .stdout gravity prints for it,
# and an empty .stderr. trycmd cannot create these files, only update them.
# `world` defaults to the example's name. Existing files are left alone.
[doc('create the CLI snapshot files for a new example')]
[group('test')]
[script('bash')]
new-snapshot name world=name:
    set -euo pipefail
    name={{ quote(name) }}
    world={{ quote(world) }}
    base="cmd/gravity/tests/cmd/$name"
    for ext in toml stdout stderr; do
      if [[ -e "$base.$ext" ]]; then
        echo "{{ style("error") }}error:{{ n }} $base.$ext already exists" >&2
        exit 1
      fi
    done
    wasm="target/wasm32-unknown-unknown/release/example_${name//-/_}.wasm"
    echo "{{ h }}build: example-$name (wasm){{ n }}"
    cargo build --locked -p "example-$name" --target wasm32-unknown-unknown --release
    echo "{{ h }}snapshot: $name{{ n }}"
    # Capture first, so a failing gravity run leaves no partial snapshot behind.
    stdout="$(mktemp)"
    trap 'rm -f "$stdout"' EXIT
    cargo run --quiet --locked --bin gravity -- --world "$world" "$wasm" > "$stdout"
    # trycmd runs gravity from cmd/gravity, so the .toml's path is relative to it.
    printf 'bin.name = "gravity"\nargs = "--world %s ../../%s"\n' "$world" "$wasm" > "$base.toml"
    cp "$stdout" "$base.stdout"
    : > "$base.stderr"

# ─── Cleanup ─────────────────────────────────────────────────────────────────────

# remove gitignored example output. After switching to a branch without an
# example, its directory holds only generated files, and the examples/*
# workspace glob then fails every cargo command with "failed to load manifest".
[doc('remove gitignored example output (generated Go and wasm)')]
[group('setup')]
clean-examples:
    @echo '{{ h }}clean: examples{{ n }}'
    @git clean -f -X -- examples/
