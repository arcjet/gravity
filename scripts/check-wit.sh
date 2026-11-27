#!/usr/bin/env bash
# Fails when a Cargo lockfile holds two versions of wit-parser or wit-component.
#
# Each wit-bindgen release requires one wit-component minor, and a build that
# mixes two wit-parser versions fails with mismatched Resolve types. When two
# versions are locked, Cargo names the version each dependent uses, so the
# message names the version wit-bindgen requires.
#
# usage: scripts/check-wit.sh [Cargo.lock]
set -euo pipefail

lockfile=${1:-Cargo.lock}

# Prints "name version" for every package in the lockfile.
packages() {
  awk '$1 == "name" { name = $3 } $1 == "version" && name != "" { print name, $3; name = "" }' "$lockfile" |
    tr -d '"'
}

# Prints the version of `crate` that package `dependent` depends on, when the
# lockfile names one.
required_by() {
  local dependent=$1 crate=$2
  awk -v dependent="\"$dependent\"" -v crate="$crate" '
    $1 == "name" { inside = ($3 == dependent) }
    inside && $0 ~ "^ \"" crate " " { sub(/^ "[^ ]+ /, ""); sub(/".*$/, ""); print }
  ' "$lockfile" | head -1
}

status=0
for pair in wit-parser:wit-bindgen-core wit-component:wit-bindgen-rust; do
  crate=${pair%%:*}
  dependent=${pair#*:}
  versions=$(packages | awk -v crate="$crate" '$1 == crate { print $2 }' | sort -u | paste -sd ' ' -)
  if [[ "$versions" != *" "* ]]; then
    continue
  fi
  required=$(required_by "$dependent" "$crate")
  message="Cargo.lock holds $crate $versions."
  if [[ -n "$required" ]]; then
    message+=" $dependent requires $crate $required: pin wit-component to =$required in cmd/gravity/Cargo.toml."
  fi
  message+=" wit-bindgen, wit-bindgen-core and wit-component move as one set (see AGENTS.md)."
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    echo "::error file=$lockfile::$message"
  else
    echo "error: $message" >&2
  fi
  status=1
done
exit $status
