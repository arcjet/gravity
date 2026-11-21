package name_collisions

import (
	"context"
	"fmt"
	"testing"

	"github.com/tetratelabs/wazero"
)

// WIT names that would take a name the generated code uses get a trailing
// underscore as Go locals, so these bindings compile, and each value reaches
// the guest unchanged.

// Mod answers mod.echo. The constructor parameter it is passed as is named
// mod_, because every host function declares mod.
type Mod struct{}

func (Mod) Echo(_ context.Context, type_ string, ctx_ uint32, len_ uint32) string {
	return fmt.Sprintf("%s %d %d", type_, ctx_, len_)
}

func newInstance(t *testing.T) *NameCollisionsInstance {
	t.Helper()
	fac, err := NewNameCollisionsFactory(t.Context(), wazero.NewRuntimeConfig(), Mod{})
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

// A field named ptr1 reaches the guest as itself, not as another string's
// address.
func TestRecordFieldEndingInDigits(t *testing.T) {
	ins := newInstance(t)
	got, err := ins.Pick(t.Context(), R{
		T0: "a", T1: "b", T2: "c", T3: "d", T4: "e",
		T5: "f", T6: "g", T7: "h", T8: "i", T9: "j",
		Ptr1: 0x1234_5678_9abc,
	})
	if err != nil {
		t.Fatal(err)
	}
	if got != 0x1234_5678_9abc {
		t.Fatalf("Pick returned %#x, want 0x123456789abc", got)
	}
}

func TestEscapedParameters(t *testing.T) {
	ins := newInstance(t)
	for _, c := range []struct {
		call func() (string, error)
		want string
	}{
		{func() (string, error) { return ins.Keywords(t.Context(), "type", 1, 2, 3, true) }, "type 1 2 3 true"},
		{func() (string, error) { return ins.FixedNames(t.Context(), 4, "errors", 5, 6) }, "4 errors 5 6"},
		{func() (string, error) { return ins.Numbered(t.Context(), 7, 8, "writeString") }, "7 8 writeString"},
	} {
		if got, err := c.call(); err != nil || got != c.want {
			t.Fatalf("got %q, %v; want %q", got, err, c.want)
		}
	}
}

func TestEscapedConstructorAndHostParameters(t *testing.T) {
	ins := newInstance(t)
	got, err := ins.CallHost(t.Context(), "hello")
	if err != nil {
		t.Fatal(err)
	}
	if want := "hello 7 5"; got != want {
		t.Fatalf("CallHost = %q, want %q", got, want)
	}
}
