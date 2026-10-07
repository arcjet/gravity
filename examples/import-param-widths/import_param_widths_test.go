package import_param_widths

import (
	"context"
	"math"
	"testing"
)

// Before the fix every host function parameter was declared uint32, so the
// lifted u64/s64/f32/f64 arguments did not fit their Go types and the
// bindings did not compile.

type wideArgs struct {
	a uint64
	b int64
	c float32
	d float64
	e uint32
	f int8
}

// Host records what the guest passed it, so the test sees the host side of
// the boundary, not only the value the guest returns.
type Host struct {
	wide wideArgs
	opt  *int64
}

func (h *Host) Wide(_ context.Context, a uint64, b int64, c float32, d float64, e uint32, f int8) float64 {
	h.wide = wideArgs{a, b, c, d, e, f}
	return d * 2
}

func (h *Host) Opt(_ context.Context, x *int64) int64 {
	h.opt = x
	if x == nil {
		return -1
	}
	return *x
}

func newInstance(t *testing.T, h *Host) *ImportParamWidthsInstance {
	t.Helper()
	fac, err := NewImportParamWidthsFactory(t.Context(), h)
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

func TestWideImportParams(t *testing.T) {
	for name, want := range map[string]wideArgs{
		"max":   {math.MaxUint64, math.MaxInt64, math.MaxFloat32, math.MaxFloat64, math.MaxUint32, math.MaxInt8},
		"min":   {0, math.MinInt64, -math.MaxFloat32, -math.MaxFloat64, 0, math.MinInt8},
		"mixed": {1 << 40, -(1 << 40), 1.5, -2.25, 4_000_000_000, -5},
	} {
		t.Run(name, func(t *testing.T) {
			h := &Host{}
			ins := newInstance(t, h)
			got := ins.CallWide(t.Context(), want.a, want.b, want.c, want.d, want.e, want.f)
			if h.wide != want {
				t.Errorf("host received %+v, want %+v", h.wide, want)
			}
			if got != want.d*2 {
				t.Errorf("CallWide = %v, want %v", got, want.d*2)
			}
		})
	}
}

func TestOptionS64ImportParam(t *testing.T) {
	h := &Host{}
	ins := newInstance(t, h)
	if got := ins.CallOpt(t.Context(), nil); got != -1 || h.opt != nil {
		t.Errorf("CallOpt(nil) = %v (host got %v), want -1 (nil)", got, h.opt)
	}
	v := int64(math.MinInt64)
	if got := ins.CallOpt(t.Context(), &v); got != v || h.opt == nil || *h.opt != v {
		t.Errorf("CallOpt(%d) = %v (host got %v), want %d", v, got, h.opt, v)
	}
}
