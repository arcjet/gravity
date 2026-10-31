package enum_discriminants

import (
	"context"
	"fmt"
	"reflect"
	"testing"
)

// Every case of the five-case Level crosses every position an enum can take.
// Stock gravity 0.0.3 (2dc1000) panicked storing any case of 2 or more into
// guest memory: "invalid int8 value encountered".

var levels = []Level{Trace, Debug, Info, Warn, Fatal}

// Host answers the guest with exactly what it was given, so a lost or
// reordered discriminant shows on the way back.
type Host struct{ entries []Entry }

func (*Host) Pass(_ context.Context, l Level) Level { return l }

func (h *Host) Entries(context.Context) []Entry { return h.entries }

func (*Host) Maybe(_ context.Context, l *Level) *Level { return l }

func newInstance(t *testing.T, h *Host) *EnumDiscriminantsInstance {
	t.Helper()
	fac, err := NewEnumDiscriminantsFactory(t.Context(), h)
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

func TestEveryCaseInEveryPosition(t *testing.T) {
	h := &Host{}
	ins := newInstance(t, h)
	for i, l := range levels {
		t.Run(fmt.Sprint(i), func(t *testing.T) {
			ctx := t.Context()
			if got := ins.Echo(ctx, l); got != l {
				t.Errorf("Echo: got %v, want %v", got, l)
			}
			if got := ins.EchoEntry(ctx, Entry{At: l, Msg: "m"}); got != (Entry{At: l, Msg: "m"}) {
				t.Errorf("EchoEntry: got %+v", got)
			}
			if got := ins.EchoOption(ctx, &l); got == nil || *got != l {
				t.Errorf("EchoOption: got %v, want %v", got, l)
			}
			if got := ins.ViaHost(ctx, l); got != l {
				t.Errorf("ViaHost: got %v, want %v", got, l)
			}
			if got := ins.ViaHostOption(ctx, &l); got == nil || *got != l {
				t.Errorf("ViaHostOption: got %v, want %v", got, l)
			}
		})
	}
	if got := ins.EchoOption(t.Context(), nil); got != nil {
		t.Errorf("EchoOption(nil): got %v", got)
	}
	if got := ins.ViaHostOption(t.Context(), nil); got != nil {
		t.Errorf("ViaHostOption(nil): got %v", got)
	}
}

func TestEveryCaseInLists(t *testing.T) {
	h := &Host{}
	ins := newInstance(t, h)
	// Reversed, so an index mistaken for a discriminant shows.
	want := []Level{Fatal, Warn, Info, Debug, Trace, Fatal}
	if got := ins.EchoList(t.Context(), want); !reflect.DeepEqual(got, want) {
		t.Errorf("EchoList: got %v, want %v", got, want)
	}
	for _, l := range want {
		h.entries = append(h.entries, Entry{At: l, Msg: fmt.Sprint(l)})
	}
	if got := ins.HostEntries(t.Context()); !reflect.DeepEqual(got, h.entries) {
		t.Errorf("HostEntries: got %+v, want %+v", got, h.entries)
	}
}
