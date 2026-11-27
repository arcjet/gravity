#!/usr/bin/env bash
# Runs scripts/check-wit.sh against lockfile fixtures: one version of each wit
# crate passes, and two versions of either crate fail, naming the crate and the
# version wit-bindgen requires.
set -euo pipefail

check="$(dirname "$0")/check-wit.sh"
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
failures=0

# Writes a lockfile fixture to "$dir/$1.lock" from the package entries on stdin.
fixture() {
  { echo 'version = 4'; echo; cat; } > "$dir/$1.lock"
}

# Runs the check on fixture $1, expecting exit status $2 and, when given, $3 in
# its output.
expect() {
  local name=$1 want=$2 text=${3:-} out status
  out=$(env -u GITHUB_ACTIONS bash "$check" "$dir/$name.lock" 2>&1) && status=0 || status=$?
  if [[ $status -ne $want ]]; then
    echo "FAIL $name: exit $status, want $want: $out"
    failures=$((failures + 1))
  elif [[ -n "$text" && "$out" != *"$text"* ]]; then
    echo "FAIL $name: output lacks \"$text\": $out"
    failures=$((failures + 1))
  else
    echo "ok   $name"
  fi
}

fixture one-version <<'EOF'
[[package]]
name = "wit-bindgen-core"
version = "0.63.0"
dependencies = [
 "wit-parser",
]

[[package]]
name = "wit-bindgen-rust"
version = "0.63.0"
dependencies = [
 "wit-component",
]

[[package]]
name = "wit-component"
version = "0.262.0"

[[package]]
name = "wit-parser"
version = "0.262.0"
EOF

fixture split-wit-component <<'EOF'
[[package]]
name = "wit-bindgen-rust"
version = "0.63.0"
dependencies = [
 "wit-component 0.262.0",
]

[[package]]
name = "wit-component"
version = "0.262.0"

[[package]]
name = "wit-component"
version = "0.263.0"
EOF

fixture split-wit-parser <<'EOF'
[[package]]
name = "wit-bindgen-core"
version = "0.63.0"
dependencies = [
 "wit-parser 0.262.0",
]

[[package]]
name = "wit-parser"
version = "0.262.0"

[[package]]
name = "wit-parser"
version = "0.263.0"
EOF

expect one-version 0
expect split-wit-component 1 "wit-bindgen-rust requires wit-component 0.262.0: pin wit-component to =0.262.0 in cmd/gravity/Cargo.toml."
expect split-wit-parser 1 "Cargo.lock holds wit-parser 0.262.0 0.263.0. wit-bindgen-core requires wit-parser 0.262.0"

exit $((failures > 0))
