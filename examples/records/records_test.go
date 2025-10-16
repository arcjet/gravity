package records

import (
	"math"
	"testing"
)

type types struct{}

func TestRecord(t *testing.T) {
	tys := types{}
	fac, err := NewRecordsFactory(t.Context(), tys)
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	foo := Foo{
		Float32: 1.0,
		Float64: 1.0,
		Uint32:  1,
		Uint64:  uint64(math.MaxUint32),
		S:       "hello",
		Vf32:    []float32{1.0, 2.0, 3.0},
		Vf64:    []float64{1.0, 2.0, 3.0},
	}
	got := ins.ModifyFoo(t.Context(), foo)
	want := Foo{
		Float32: foo.Float32 * 2.0,
		Float64: foo.Float64 * 2.0,
		Uint32:  foo.Uint32 + 1,
		Uint64:  foo.Uint64 + 1,
		S:       "received hello",
		Vf32:    []float32{2.0, 4.0, 6.0},
		Vf64:    []float64{2.0, 4.0, 6.0},
	}
	if !fooCmp(got, want) {
		t.Fatalf("got %+v, want %+v", got, want)
	}
}

func fooCmp(a, b Foo) bool {
	if a.Float32 != b.Float32 || a.Float64 != b.Float64 || a.Uint32 != b.Uint32 || a.Uint64 != b.Uint64 || a.S != b.S {
		return false
	}
	if len(a.Vf32) != len(b.Vf32) || len(a.Vf64) != len(b.Vf64) {
		return false
	}
	for i := range a.Vf32 {
		if a.Vf32[i] != b.Vf32[i] {
			return false
		}
	}
	for i := range a.Vf64 {
		if a.Vf64[i] != b.Vf64[i] {
			return false
		}
	}
	return true
}
