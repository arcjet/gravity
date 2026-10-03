package memory

import (
	"context"
	"math"
	"reflect"
	"testing"
)

// Host records the record it receives and replies with a different one, so the
// test can tell the import's parameter (loaded from guest memory) from its
// result (stored into guest memory).
type Host struct {
	got   Everything
	reply Everything
}

func (h *Host) Echo(_ context.Context, x Everything) Everything {
	h.got = x
	return h.reply
}

func ptr[T any](v T) *T { return &v }

// Each value covers every field type at a boundary: minimums, maximums, and a
// mix of signs, non-ASCII text and a present option.
var cases = map[string]Everything{
	"min": {
		AU8: 0, AS8: math.MinInt8, AU16: 0, AS16: math.MinInt16,
		AU32: 0, AS32: math.MinInt32, AU64: 0, AS64: math.MinInt64,
		AF64: -math.MaxFloat64, ABool: false, AChar: 0, AColor: Red,
		AOpt: nil, AStr: "", AF32: -math.MaxFloat32,
	},
	"max": {
		AU8: math.MaxUint8, AS8: math.MaxInt8, AU16: math.MaxUint16, AS16: math.MaxInt16,
		AU32: math.MaxUint32, AS32: math.MaxInt32, AU64: math.MaxUint64, AS64: math.MaxInt64,
		AF64: math.MaxFloat64, ABool: true, AChar: 0x10FFFF, AColor: Blue,
		AOpt: ptr[uint8](math.MaxUint8), AStr: "hello, world", AF32: math.MaxFloat32,
	},
	"mixed": {
		AU8: 200, AS8: -5, AU16: 65000, AS16: -30000,
		AU32: 4_000_000_000, AS32: -42, AU64: 1 << 40, AS64: -(1 << 40),
		AF64: -2.25, ABool: true, AChar: 'λ', AColor: Green,
		AOpt: ptr[uint8](0), AStr: "λ😀", AF32: 1.5,
	},
}

func newInstance(t *testing.T, h *Host) *MemoryInstance {
	t.Helper()
	fac, err := NewMemoryFactory(t.Context(), h)
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

func TestRoundTrip(t *testing.T) {
	ins := newInstance(t, &Host{})
	for name, want := range cases {
		t.Run(name, func(t *testing.T) {
			if got := ins.RoundTrip(t.Context(), want); !reflect.DeepEqual(got, want) {
				t.Errorf("wanted: %+v, but got: %+v", want, got)
			}
		})
	}
}

func TestCallHostEcho(t *testing.T) {
	for name, sent := range cases {
		for replyName, reply := range cases {
			t.Run(name+"/"+replyName, func(t *testing.T) {
				h := &Host{reply: reply}
				ins := newInstance(t, h)
				got := ins.CallHostEcho(t.Context(), sent)
				if !reflect.DeepEqual(h.got, sent) {
					t.Errorf("host received: %+v, but wanted: %+v", h.got, sent)
				}
				if !reflect.DeepEqual(got, reply) {
					t.Errorf("wanted reply: %+v, but got: %+v", reply, got)
				}
			})
		}
	}
}

// TestRoundTripNarrow calls the export repeatedly: a store past the end of the
// 68-byte parameter area damages the string buffer allocated after it, and the
// guest traps when it frees that buffer.
func TestRoundTripNarrow(t *testing.T) {
	ins := newInstance(t, &Host{})
	for i := range 100 {
		want := Narrow{
			N0: uint32(i), N1: 1, N2: 2, N3: 3, N4: 4, N5: 5, N6: 6, N7: 7,
			N8: 8, N9: 9, N10: 10, N11: 11, N12: 12, N13: math.MaxUint32,
			S: "a string allocated after the parameter area", F: -1.5,
		}
		if got := ins.RoundTripNarrow(t.Context(), want); got != want {
			t.Fatalf("call %d: wanted: %+v, but got: %+v", i, want, got)
		}
	}
}
