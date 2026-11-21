#!/usr/bin/env bash
# Checks that a build script embedding the git commit, cmd/gravity/build.rs by
# default or the one named as $1, leaves an unchanged build fresh and rebuilds
# when HEAD moves. Cargo treats a rerun-if-changed path that does not exist as
# changed, so a script naming a wrong path recompiles the crate on every build.
# The check builds a minimal package that uses the script from a subdirectory
# of a git repository, as cmd/gravity is, in a fresh repository and in a linked
# worktree of it, where .git is a file.
set -euo pipefail

build_rs=${1:-"$(dirname "$0")/../cmd/gravity/build.rs"}
build_rs=$(cd "$(dirname "$build_rs")" && pwd)/$(basename "$build_rs")
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
failures=0

git_() {
  git -c user.name=check -c user.email=check@example.invalid -c commit.gpgsign=false "$@"
}

# Builds the package in checkout $1 with target directory $2 and prints cargo's
# verbose output.
build() {
  CARGO_TARGET_DIR=$2 cargo build --manifest-path "$1/cmd/probe/Cargo.toml" -v 2>&1
}

fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

# Checks the package checked out in $1, building into target directory $2.
check() {
  local name=$1 target=$2 out hash
  build "$name" "$target" > /dev/null
  out=$(build "$name" "$target")
  if [[ "$out" == *"Compiling probe"* ]]; then
    fail "$name: an unchanged second build recompiled: $(grep -m1 Dirty <<<"$out")"
  else
    echo "ok   $name: an unchanged second build is fresh"
  fi

  git_ -C "$name" commit -q --allow-empty -m "move HEAD"
  build "$name" "$target" > /dev/null
  hash=$("$target/debug/probe")
  if [[ "$hash" != "$(git -C "$name" rev-parse HEAD)" ]]; then
    fail "$name: after a commit the embedded hash is $hash, want $(git -C "$name" rev-parse HEAD)"
  else
    echo "ok   $name: a commit updates the embedded hash"
  fi
}

mkdir -p "$dir/repo/cmd/probe/src"
cat > "$dir/repo/cmd/probe/Cargo.toml" <<'EOF'
[package]
name = "probe"
version = "0.0.0"
edition = "2024"

[workspace]
EOF
echo 'fn main() { println!("{}", env!("GIT_HASH")); }' > "$dir/repo/cmd/probe/src/main.rs"
cp "$build_rs" "$dir/repo/cmd/probe/build.rs"
echo /target > "$dir/repo/.gitignore"
git_ -C "$dir/repo" init -q
git_ -C "$dir/repo" add -A
git_ -C "$dir/repo" commit -q -m "probe"

check "$dir/repo" "$dir/target-repo"

git_ -C "$dir/repo" worktree add -q -b worktree "$dir/worktree"
check "$dir/worktree" "$dir/target-worktree"

[[ $failures -eq 0 ]]
