package invalid_values

import (
	"context"
	"strings"
	"testing"

	"github.com/tetratelabs/wazero"
)

func newInstance(t *testing.T) *InvalidValuesInstance {
	t.Helper()
	fac, err := NewInvalidValuesFactory(t.Context(), wazero.NewRuntimeConfig())
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

func requireErrorContaining(t *testing.T, err error, want string) {
	t.Helper()
	if err == nil || !strings.Contains(err.Error(), want) {
		t.Fatalf("expected an error containing %q, got %v", want, err)
	}
}

func TestName(t *testing.T) {
	ins := newInstance(t)
	got, err := ins.Name(t.Context(), ColorBlue)
	if err != nil || got != "blue" {
		t.Fatalf("Name(ColorBlue) = %q, %v", got, err)
	}
}

// A Color that is none of the cases, which outside this package is a nil
// Color, is returned as an error, not a panic.
func TestNameRefusesInvalidEnum(t *testing.T) {
	ins := newInstance(t)
	for value, want := range map[Color]string{nil: "<nil>", color(99): "color(99)"} {
		_, err := ins.Name(t.Context(), value)
		requireErrorContaining(t, err, "invalid color value "+want)
	}
}

// An export whose WIT result is not a result returns the error too.
func TestNamePlainRefusesInvalidEnum(t *testing.T) {
	ins := newInstance(t)
	_, err := ins.NamePlain(t.Context(), nil)
	requireErrorContaining(t, err, "invalid color value <nil>")
}

func TestSurrogateIsReturnedAsError(t *testing.T) {
	ins := newInstance(t)
	_, err := ins.Surrogate(t.Context())
	requireErrorContaining(t, err, "invalid char value 0xd800")
}
