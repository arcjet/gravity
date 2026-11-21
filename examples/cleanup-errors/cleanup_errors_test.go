package cleanup_errors

import (
	"context"
	"strings"
	"testing"

	"github.com/tetratelabs/wazero"
)

// After an export returns, the bindings call its cabi_post_* function so the
// guest can free the result's memory. A failed cleanup is returned as an
// error that carries the wazero error, from every export.

func newInstance(t *testing.T) *CleanupErrorsInstance {
	t.Helper()
	fac, err := NewCleanupErrorsFactory(t.Context(), wazero.NewRuntimeConfig())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { fac.Close(context.Background()) })
	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { ins.Close(context.Background()) })
	return ins
}

func requireCleanupError(t *testing.T, err error, export string) {
	t.Helper()
	if err == nil {
		t.Fatal("expected an error, got nil")
	}
	for _, want := range []string{
		"failed to clean up after " + export,
		// The trap wazero reported is kept, not discarded.
		"unreachable",
	} {
		if !strings.Contains(err.Error(), want) {
			t.Fatalf("expected an error containing %q, got %q", want, err)
		}
	}
}

func TestValue(t *testing.T) {
	ins := newInstance(t)
	got, err := ins.Value(t.Context())
	if err != nil || got != "value" {
		t.Fatalf("Value = %q, %v", got, err)
	}
}

func TestFailedCleanupAfterValueIsReturned(t *testing.T) {
	ins := newInstance(t)
	_, err := ins.FailValue(t.Context())
	requireCleanupError(t, err, "fail-value")
}

// The guest's own error is kept beside the cleanup's.
func TestFailedCleanupAfterGuestErrorIsJoined(t *testing.T) {
	ins := newInstance(t)
	err := ins.FailError(t.Context())
	requireCleanupError(t, err, "fail-error")
	if !strings.Contains(err.Error(), "guest error") {
		t.Fatalf("expected the guest's error to be kept, got %q", err)
	}
}

// An export whose WIT result is not a result returns the failure too.
func TestFailedCleanupWithoutWITResultIsReturned(t *testing.T) {
	ins := newInstance(t)
	_, err := ins.FailPlain(t.Context())
	requireCleanupError(t, err, "fail-plain")
}
