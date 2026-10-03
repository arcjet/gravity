//go:build tools

package gravity

// This file declares dependencies that are used by the project but not directly
// imported by any committed Go source files. This ensures that `go mod tidy`
// keeps these dependencies in go.mod.
//
// The generated bindings (examples/*/*.go) import wazero, but since those files
// are not committed to the repository, we need to declare the dependency here
// so that Dependabot and `go mod tidy` know to keep it.

import (
	_ "github.com/tetratelabs/wazero"
	_ "github.com/tetratelabs/wazero/api"
)
