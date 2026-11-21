package utf8_strings

import (
	"context"
	"strings"
	"testing"
)

// The Component Model requires every string to be valid UTF-8. The bindings
// refuse a string that is not, in each direction, rather than passing its
// bytes on: before the check, invalid UTF-8 from the host trapped the guest.

const notUTF8 = "a\xffb"

// Host answers Text with text, and records each string Take receives.
type Host struct {
	text  string
	taken []string
}

func (h *Host) Text(context.Context) string {
	return h.text
}

func (h *Host) Take(_ context.Context, s string) {
	h.taken = append(h.taken, s)
}

func newInstance(t *testing.T, host *Host) *Utf8StringsInstance {
	t.Helper()
	fac, err := NewUtf8StringsFactory(t.Context(), host)
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

func requireUTF8Error(t *testing.T, err error, direction string) {
	t.Helper()
	if err == nil {
		t.Fatal("expected an error, got nil")
	}
	want := "invalid UTF-8 in string " + direction
	if !strings.Contains(err.Error(), want) {
		t.Fatalf("expected an error containing %q, got %q", want, err)
	}
}

func TestEchoValid(t *testing.T) {
	ins := newInstance(t, &Host{})
	for _, s := range []string{"", "plain", "héllo, 世界 🌍"} {
		got, err := ins.Echo(t.Context(), s)
		if err != nil {
			t.Fatalf("Echo(%q): %v", s, err)
		}
		if got != s {
			t.Fatalf("Echo(%q) = %q", s, got)
		}
	}
}

// An export that returns an error returns one for an invalid argument.
func TestEchoRefusesInvalidArgument(t *testing.T) {
	ins := newInstance(t, &Host{})
	for _, s := range []string{notUTF8, "\xff", "\xf0\x9f", "\xc3", "\xed\xa0\x80"} {
		_, err := ins.Echo(t.Context(), s)
		requireUTF8Error(t, err, "passed to the guest")
	}
	// The instance is still usable: the guest never saw the bytes.
	got, err := ins.Echo(t.Context(), "after")
	if err != nil || got != "after" {
		t.Fatalf("Echo after a refusal = %q, %v", got, err)
	}
}

// An export with no error return panics for an invalid argument, with the
// same error.
func TestByteLengthPanicsOnInvalidArgument(t *testing.T) {
	ins := newInstance(t, &Host{})
	if got := ins.ByteLength(t.Context(), "héllo"); got != 6 {
		t.Fatalf("ByteLength(héllo) = %d, want 6", got)
	}
	defer func() {
		r := recover()
		err, ok := r.(error)
		if !ok {
			t.Fatalf("expected a panic with an error, got %v", r)
		}
		requireUTF8Error(t, err, "passed to the guest")
	}()
	ins.ByteLength(t.Context(), notUTF8)
}

// A string the guest returns is checked before it becomes a Go string.
func TestInvalidResultFromGuest(t *testing.T) {
	ins := newInstance(t, &Host{})
	got, err := ins.Invalid(t.Context())
	requireUTF8Error(t, err, "received from the guest")
	if got != "" {
		t.Fatalf("expected the zero value with the error, got %q", got)
	}
}

// An error message the guest returns is checked too, so the caller gets the
// UTF-8 error in place of the message.
func TestInvalidErrorFromGuest(t *testing.T) {
	ins := newInstance(t, &Host{})
	err := ins.InvalidError(t.Context())
	requireUTF8Error(t, err, "received from the guest")
}

// An export with no error return panics for a string the guest returns.
func TestInvalidPlainPanics(t *testing.T) {
	ins := newInstance(t, &Host{})
	defer func() {
		r := recover()
		err, ok := r.(error)
		if !ok {
			t.Fatalf("expected a panic with an error, got %v", r)
		}
		requireUTF8Error(t, err, "received from the guest")
	}()
	ins.InvalidPlain(t.Context())
}

// A host function that returns invalid UTF-8 traps the guest, as any failure
// in a host function does, and the export returns the trap.
func TestHostReturnsInvalid(t *testing.T) {
	ins := newInstance(t, &Host{text: "from host"})
	got, err := ins.FromHost(t.Context())
	if err != nil || got != "from host" {
		t.Fatalf("FromHost = %q, %v", got, err)
	}

	ins = newInstance(t, &Host{text: notUTF8})
	_, err = ins.FromHost(t.Context())
	requireUTF8Error(t, err, "passed to the guest")
}

// A host function is not called with a string the guest passed that is not
// valid UTF-8.
func TestGuestPassesInvalidToHost(t *testing.T) {
	host := &Host{}
	ins := newInstance(t, host)
	err := ins.ToHostInvalid(t.Context())
	requireUTF8Error(t, err, "received from the guest")
	if len(host.taken) != 0 {
		t.Fatalf("host.Take was called with %q", host.taken)
	}
}
