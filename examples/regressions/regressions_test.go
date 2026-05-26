package regressions

import (
	"context"
	"math"
	"testing"
)

// Checker implements IRegressionsChecker for testing.
//
// Regression 1: Import functions returning bool and enum types.
// Before the fix, gravity panicked with:
//
//	todo!("implement handling of wasm signatures with results")
//
// when generating host function bindings for import functions whose WIT
// return type (bool, enum) maps to a Wasm i32 result.
type Checker struct{}

func (Checker) IsEnabled(_ context.Context, key string) bool {
	return key == "enabled"
}

func (Checker) GetStatus(_ context.Context, key string) Status {
	switch key {
	case "active":
		return Active
	case "inactive":
		return Inactive
	default:
		return Unknown
	}
}

// Processor implements IRegressionsProcessor for testing.
//
// Regression 2: Import functions with u32 parameters.
// Before the fix, gravity generated api.DecodeU32()/api.EncodeU32() calls
// which convert between uint32 and uint64. Host function parameters are
// already uint32, so this caused type mismatches that prevented compilation.
// The fix uses simple uint32() identity casts instead.
type Processor struct{}

func (Processor) Double(_ context.Context, value uint32) uint32 {
	return value * 2
}

// Pinger implements IRegressionsPinger for testing.
//
// Regression 3: Import functions with zero WIT parameters.
// Before the fix, gravity generated a trailing comma in the host function
// signature — func(ctx context.Context, mod api.Module, ,) — which is a
// Go syntax error. The fix merges all params into a single list so the
// join produces correct commas.
type Pinger struct{}

func (Pinger) Ping(_ context.Context) bool {
	return true
}

// EmailChecker, BotVerifier, IpSource — regression 4 (cross-interface
// enum collision) and regression 5 (callback returning option<string>).
type EmailChecker struct{}

func (EmailChecker) IsAllowed(_ context.Context, email string) EmailCheckerValidatorResponse {
	switch email {
	case "allow@example.com":
		return Yes
	case "block@example.com":
		return No
	default:
		return Maybe
	}
}

type BotVerifier struct{}

func (BotVerifier) Verify(_ context.Context, botID string) BotVerifierValidatorResponse {
	switch botID {
	case "verified-bot":
		return Verified
	case "spoofed-bot":
		return Spoofed
	default:
		return Unverifiable
	}
}

type IpSource struct{}

func (IpSource) Lookup(_ context.Context, ip string) *string {
	if ip == "127.0.0.1" {
		s := "localhost"
		return &s
	}
	return nil
}

func newInstance(t *testing.T) *RegressionsInstance {
	t.Helper()
	fac, err := NewRegressionsFactory(
		t.Context(),
		Checker{},
		Processor{},
		Pinger{},
		EmailChecker{},
		BotVerifier{},
		IpSource{},
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { fac.Close(t.Context()) })

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := ins.Close(t.Context()); err != nil {
			t.Error(err)
		}
	})

	return ins
}

// TestCheckEnabled tests regression 1: import function returning bool.
// The Wasm guest calls the imported is-enabled function and returns the
// bool result. Before the fix, gravity could not generate bindings for
// this pattern because it did not handle Wasm signatures with results.
func TestCheckEnabled(t *testing.T) {
	ins := newInstance(t)

	if got := ins.CheckEnabled(t.Context(), "enabled"); got != true {
		t.Errorf("CheckEnabled(\"enabled\") = %v, want true", got)
	}
	if got := ins.CheckEnabled(t.Context(), "disabled"); got != false {
		t.Errorf("CheckEnabled(\"disabled\") = %v, want false", got)
	}
}

// TestCheckStatus tests regression 1: import function returning enum.
// This is the exact pattern that was failing in Arcjet's bot bindings,
// where verify-bot's verify function returns a validator-response enum.
// Before the fix, gravity panicked when processing the Wasm signature.
func TestCheckStatus(t *testing.T) {
	ins := newInstance(t)

	tests := []struct {
		key  string
		want uint32
	}{
		{"active", 0},   // Active
		{"inactive", 1}, // Inactive
		{"unknown", 2},  // Unknown
	}

	for _, tt := range tests {
		if got := ins.CheckStatus(t.Context(), tt.key); got != tt.want {
			t.Errorf("CheckStatus(%q) = %d, want %d", tt.key, got, tt.want)
		}
	}
}

// TestDoubleValue tests regression 2: import function with u32 parameters.
// Before the fix, gravity generated api.EncodeU32()/api.DecodeU32() for
// I32FromU32/U32FromI32 instructions. Those wazero API functions convert
// between uint32 and uint64, but host function parameters are already
// uint32, causing compilation errors. The fix uses uint32() casts.
func TestDoubleValue(t *testing.T) {
	ins := newInstance(t)

	tests := []struct {
		input uint32
		want  uint32
	}{
		{0, 0},
		{1, 2},
		{21, 42},
		{1000, 2000},
		// Verify the full uint32 range works (large values that would
		// fail if incorrectly truncated or widened to uint64).
		{math.MaxUint32 / 2, math.MaxUint32 - 1},
	}

	for _, tt := range tests {
		if got := ins.DoubleValue(t.Context(), tt.input); got != tt.want {
			t.Errorf("DoubleValue(%d) = %d, want %d", tt.input, got, tt.want)
		}
	}
}

// TestRunPing tests regression 3: import function with zero WIT parameters.
// The pinger.ping import takes no arguments (only the implicit ctx and mod
// params in the host function). Before the fix, gravity generated invalid
// Go with a trailing comma after mod api.Module, preventing compilation.
// This test also exercises regression 1 (bool return) in combination with
// the zero-param case.
func TestRunPing(t *testing.T) {
	ins := newInstance(t)

	if got := ins.RunPing(t.Context()); got != true {
		t.Errorf("RunPing() = %v, want true", got)
	}
}

// TestCrossInterfaceEnumCollision covers regression 4. Both
// `email-checker` and `bot-verifier` define `enum validator-response`
// inside the same world; without interface-scoped qualification the
// generated Go contained two `type ValidatorResponse interface { ... }`
// declarations and refused to compile. We verify (a) both host method
// signatures use qualified Go type names that exist alongside each other
// and (b) the wasm guest can dispatch on each independently.
func TestCrossInterfaceEnumCollision(t *testing.T) {
	ins := newInstance(t)

	emailTests := []struct {
		input string
		want  uint32
	}{
		{"allow@example.com", 0},
		{"block@example.com", 1},
		{"other", 2},
	}
	for _, tt := range emailTests {
		if got := ins.CheckEmailAllowed(t.Context(), tt.input); got != tt.want {
			t.Errorf("CheckEmailAllowed(%q) = %d, want %d", tt.input, got, tt.want)
		}
	}

	botTests := []struct {
		input string
		want  uint32
	}{
		{"verified-bot", 0},
		{"spoofed-bot", 1},
		{"other", 2},
	}
	for _, tt := range botTests {
		if got := ins.CheckBotVerified(t.Context(), tt.input); got != tt.want {
			t.Errorf("CheckBotVerified(%q) = %d, want %d", tt.input, got, tt.want)
		}
	}
}

// TestImportCallbackOptionString covers regression 5. The `ip-source`
// import returns `option<string>`. Lowering it into wasm memory must run
// against the IMPORT-side module handle (`mod.Memory()` /
// `mod.ExportedFunction("cabi_realloc")`) — gravity previously
// hard-coded the export-side `i.module.*` handle in list and option
// lowering, producing `undefined: i` from the generated host wrapper.
func TestImportCallbackOptionString(t *testing.T) {
	ins := newInstance(t)

	if got := ins.RunIpLookup(t.Context(), "127.0.0.1"); got != "localhost" {
		t.Errorf("RunIpLookup(\"127.0.0.1\") = %q, want \"localhost\"", got)
	}
	if got := ins.RunIpLookup(t.Context(), "0.0.0.0"); got != "absent" {
		t.Errorf("RunIpLookup(\"0.0.0.0\") = %q, want \"absent\"", got)
	}
}
