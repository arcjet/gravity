package traps

import (
	"context"
	"reflect"
	"strings"
	"testing"

	"github.com/tetratelabs/wazero"
)

// Every export returns an error, so a guest trap reaches the caller as an
// error, never as a panic, whatever the WIT function's result is.

func newInstance(t *testing.T) *TrapsInstance {
	t.Helper()
	fac, err := NewTrapsFactory(t.Context(), wazero.NewRuntimeConfig())
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

func requireTrap(t *testing.T, err error) {
	t.Helper()
	if err == nil || !strings.Contains(err.Error(), "unreachable") {
		t.Fatalf("expected the trap as an error, got %v", err)
	}
}

func TestCount(t *testing.T) {
	ins := newInstance(t)
	if got, err := ins.Count(t.Context(), "hello", false); err != nil || got != 5 {
		t.Fatalf("Count = %d, %v; want 5", got, err)
	}
	got, err := newInstance(t).Count(t.Context(), "hello", true)
	requireTrap(t, err)
	if got != 0 {
		t.Fatalf("expected the zero value with the trap, got %d", got)
	}
}

func TestDetect(t *testing.T) {
	ins := newInstance(t)
	got, err := ins.Detect(t.Context(), "hello", false)
	if want := []Finding{{Start: 0, End: 5, Kind: "all"}}; err != nil || !reflect.DeepEqual(got, want) {
		t.Fatalf("Detect = %v, %v; want %v", got, err, want)
	}
	_, err = newInstance(t).Detect(t.Context(), "hello", true)
	requireTrap(t, err)
}

func TestReset(t *testing.T) {
	ins := newInstance(t)
	if err := ins.Reset(t.Context(), false); err != nil {
		t.Fatal(err)
	}
	requireTrap(t, newInstance(t).Reset(t.Context(), true))
}
