package option_float_params

import (
	"context"
	"math"
	"testing"
)

// Before the fix, the generated Go for an export with an option<f64> or
// option<f32> parameter did not compile:
//
//	cannot use result1 (variable of type uint64) as float64 value in assignment
//
// because the flattened payload's temporary was declared float64 while the
// lowering produced api.EncodeF64's uint64.

func newInstance(t *testing.T) *OptionFloatParamsInstance {
	t.Helper()
	fac, err := NewOptionFloatParamsFactory(t.Context())
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

func ptr[T any](v T) *T { return &v }

func TestOptionF64Param(t *testing.T) {
	ins := newInstance(t)
	for _, tc := range []struct {
		value  float64
		factor *float64
		want   float64
	}{
		{1.5, nil, 1.5},
		{1.5, ptr(2.0), 3.0},
		{-0.25, ptr(-4.0), 1.0},
		{3, ptr(math.Inf(1)), math.Inf(1)},
	} {
		if got := ins.Scale(t.Context(), tc.value, tc.factor); got != tc.want {
			t.Errorf("Scale(%v, %v) = %v, want %v", tc.value, tc.factor, got, tc.want)
		}
	}
}

func TestOptionF32Param(t *testing.T) {
	ins := newInstance(t)
	if got := ins.Scale32(t.Context(), 1.5, nil); got != 1.5 {
		t.Errorf("Scale32(1.5, nil) = %v, want 1.5", got)
	}
	if got := ins.Scale32(t.Context(), 1.5, ptr[float32](-2)); got != -3 {
		t.Errorf("Scale32(1.5, -2) = %v, want -3", got)
	}
}

func TestVariantF64Param(t *testing.T) {
	ins := newInstance(t)
	if got := ins.ToMeters(t.Context(), LengthMeters{Value: 2.5}); got != 2.5 {
		t.Errorf("ToMeters(meters 2.5) = %v, want 2.5", got)
	}
	if got := ins.ToMeters(t.Context(), LengthFeet{Value: 10}); math.Abs(got-3.048) > 1e-12 {
		t.Errorf("ToMeters(feet 10) = %v, want 3.048", got)
	}
	if got := ins.ToMeters(t.Context(), LengthUnknown{}); got != -1 {
		t.Errorf("ToMeters(unknown) = %v, want -1", got)
	}
}
