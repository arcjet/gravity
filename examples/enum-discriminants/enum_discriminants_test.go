package enum_discriminants

import (
	"context"
	"fmt"
	"reflect"
	"testing"

	"github.com/tetratelabs/wazero"
)

// Every case of the five-case Level crosses every position an enum can take,
// in both directions, so each discriminant from 0 to 4 must survive every
// lowering, lifting and store into guest memory.

var levels = []Level{LevelTrace, LevelDebug, LevelInfo, LevelWarn, LevelFatal}

// Host answers the guest with exactly what it was given, so a lost or
// reordered discriminant shows on the way back.
type Host struct{ entries []Entry }

func (*Host) Pass(_ context.Context, l Level) Level { return l }

func (h *Host) Entries(context.Context) []Entry { return h.entries }

func (*Host) Maybe(_ context.Context, l *Level) *Level { return l }

// show formats an optional Level for a failure message.
func show(l *Level) string {
	if l == nil {
		return "nil"
	}
	return fmt.Sprint(*l)
}

func newInstance(t *testing.T, h *Host) *EnumDiscriminantsInstance {
	t.Helper()
	fac, err := NewEnumDiscriminantsFactory(t.Context(), wazero.NewRuntimeConfig(), h)
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
				t.Errorf("EchoOption: got %s, want %v", show(got), l)
			}
			if got := ins.ViaHost(ctx, l); got != l {
				t.Errorf("ViaHost: got %v, want %v", got, l)
			}
			if got := ins.ViaHostOption(ctx, &l); got == nil || *got != l {
				t.Errorf("ViaHostOption: got %s, want %v", show(got), l)
			}
		})
	}
	if got := ins.EchoOption(t.Context(), nil); got != nil {
		t.Errorf("EchoOption(nil): got %s", show(got))
	}
	if got := ins.ViaHostOption(t.Context(), nil); got != nil {
		t.Errorf("ViaHostOption(nil): got %s", show(got))
	}
}

func TestEveryCaseInLists(t *testing.T) {
	h := &Host{}
	ins := newInstance(t, h)
	// Reversed, so an index mistaken for a discriminant shows.
	want := []Level{LevelFatal, LevelWarn, LevelInfo, LevelDebug, LevelTrace, LevelFatal}
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
