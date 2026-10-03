package example

import (
	"context"
	"fmt"
	"math"
	"runtime"
	"testing"
)

type Runtime struct {
	msg string

	u8   uint8
	s8   int8
	u16  uint16
	s16  int16
	s32  int32
	u64  uint64
	s64  int64
	f32  float32
	f64  float64
	char rune
}

func (Runtime) Os(context.Context) string             { return runtime.GOOS }
func (Runtime) Arch(context.Context) string           { return runtime.GOARCH }
func (Runtime) GetU32(context.Context) uint32         { return 42 }
func (r *Runtime) GetU8(context.Context) uint8        { return r.u8 }
func (r *Runtime) GetS8(context.Context) int8         { return r.s8 }
func (r *Runtime) GetU16(context.Context) uint16      { return r.u16 }
func (r *Runtime) GetS16(context.Context) int16       { return r.s16 }
func (r *Runtime) GetS32(context.Context) int32       { return r.s32 }
func (r *Runtime) GetU64(context.Context) uint64      { return r.u64 }
func (r *Runtime) GetS64(context.Context) int64       { return r.s64 }
func (r *Runtime) GetF32(context.Context) float32     { return r.f32 }
func (r *Runtime) GetF64(context.Context) float64     { return r.f64 }
func (r *Runtime) GetChar(context.Context) rune       { return r.char }
func (r *Runtime) Puts(_ context.Context, msg string) { r.msg = msg }

func TestBasic(t *testing.T) {
	r := &Runtime{}
	fac, err := NewExampleFactory(t.Context(), r)
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	message, err := ins.Hello(t.Context())
	if err != nil {
		t.Fatal(err)
	}

	const want = "Hello, world!"
	if message != want {
		t.Errorf("wanted: %s, but got: %s", want, message)
	}

	wantPutsMsg := fmt.Sprintf("%s/%s", runtime.GOOS, runtime.GOARCH)
	if r.msg != wantPutsMsg {
		t.Errorf("wanted: %s, but got: %s", wantPutsMsg, r.msg)
	}
}

func TestCallGetU32(t *testing.T) {
	r := &Runtime{}
	fac, err := NewExampleFactory(t.Context(), r)
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	value := ins.CallGetU32(t.Context())

	var want uint32 = 42
	if value != want {
		t.Errorf("wanted: %d, but got: %d", want, value)
	}
}

// roundTrip sets each value as the host import's return value, calls the
// export that returns the import's result, and checks the value survives the
// trip through the guest.
func roundTrip[T comparable](t *testing.T, values []T, set func(*Runtime, T), call func(*ExampleInstance, context.Context) T) {
	t.Helper()
	r := &Runtime{}
	fac, err := NewExampleFactory(t.Context(), r)
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	for _, want := range values {
		set(r, want)
		if got := call(ins, t.Context()); got != want {
			t.Errorf("wanted: %v, but got: %v", want, got)
		}
	}
}

func TestImportReturnsU8(t *testing.T) {
	roundTrip(t, []uint8{0, 1, 200, math.MaxUint8},
		func(r *Runtime, v uint8) { r.u8 = v }, (*ExampleInstance).CallGetU8)
}

func TestImportReturnsS8(t *testing.T) {
	roundTrip(t, []int8{math.MinInt8, -1, 0, 1, math.MaxInt8},
		func(r *Runtime, v int8) { r.s8 = v }, (*ExampleInstance).CallGetS8)
}

func TestImportReturnsU16(t *testing.T) {
	roundTrip(t, []uint16{0, 1, 65000, math.MaxUint16},
		func(r *Runtime, v uint16) { r.u16 = v }, (*ExampleInstance).CallGetU16)
}

func TestImportReturnsS16(t *testing.T) {
	roundTrip(t, []int16{math.MinInt16, -1, 0, 1, math.MaxInt16},
		func(r *Runtime, v int16) { r.s16 = v }, (*ExampleInstance).CallGetS16)
}

func TestImportReturnsS32(t *testing.T) {
	roundTrip(t, []int32{math.MinInt32, -42, -1, 0, 1, math.MaxInt32},
		func(r *Runtime, v int32) { r.s32 = v }, (*ExampleInstance).CallGetS32)
}

func TestImportReturnsU64(t *testing.T) {
	roundTrip(t, []uint64{0, 1, 1 << 40, math.MaxUint64},
		func(r *Runtime, v uint64) { r.u64 = v }, (*ExampleInstance).CallGetU64)
}

func TestImportReturnsS64(t *testing.T) {
	roundTrip(t, []int64{math.MinInt64, -(1 << 40), -1, 0, 1, math.MaxInt64},
		func(r *Runtime, v int64) { r.s64 = v }, (*ExampleInstance).CallGetS64)
}

func TestImportReturnsF32(t *testing.T) {
	roundTrip(t, []float32{0, 1.5, -2.25, math.SmallestNonzeroFloat32, math.MaxFloat32, float32(math.Inf(1)), float32(math.Inf(-1))},
		func(r *Runtime, v float32) { r.f32 = v }, (*ExampleInstance).CallGetF32)
}

func TestImportReturnsF64(t *testing.T) {
	roundTrip(t, []float64{0, 1.5, -2.25, math.SmallestNonzeroFloat64, math.MaxFloat64, math.Inf(1), math.Inf(-1)},
		func(r *Runtime, v float64) { r.f64 = v }, (*ExampleInstance).CallGetF64)
}

func TestImportReturnsChar(t *testing.T) {
	roundTrip(t, []rune{0, 'a', 'λ', '😀', 0xD7FF, 0xE000, 0x10FFFF},
		func(r *Runtime, v rune) { r.char = v }, (*ExampleInstance).CallGetChar)
}
