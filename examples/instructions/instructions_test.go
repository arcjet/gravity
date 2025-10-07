package instructions

import (
	"iter"
	"math"
	"testing"
)

func inclusive[Num interface { ~int8 | ~uint8 | ~int16 | ~uint16 }](start Num, end Num) iter.Seq[Num] {
	return func(yield func(v Num) bool) {
		var next Num = start
		for {
			if !yield(next) {
				return
			}
			if next != end {
				next++
			} else {
				return
			}
		}
	}
}
func inclusiveStep[Num interface { ~int32 | ~uint32 }](start Num, end Num, step Num) iter.Seq[Num] {
	return func(yield func(v Num) bool) {
		var next Num = start
		for {
			if !yield(next) {
				return
			}
			if next == end {
				return
			}

			if end - step > next {
				next += step
			} else {
				next = end
			}
		}
	}
}

func Test_S8Roundtrip(t *testing.T) {
	fac, err := NewInstructionsFactory(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	for expected := range inclusive[int8](math.MinInt8, math.MaxInt8) {
		actual := ins.S8Roundtrip(t.Context(), expected)
		if actual != expected {
			t.Errorf("expected: %d, but got: %d", expected, actual)
		}
	}
}

func Test_U8Roundtrip(t *testing.T) {
	fac, err := NewInstructionsFactory(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	for expected := range inclusive[uint8](0, math.MaxUint8) {
		actual := ins.U8Roundtrip(t.Context(), expected)
		if actual != expected {
			t.Errorf("expected: %d, but got: %d", expected, actual)
		}
	}
}

func Test_S16Roundtrip(t *testing.T) {
	fac, err := NewInstructionsFactory(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	for expected := range inclusive[int16](math.MinInt16, math.MaxInt16) {
		actual := ins.S16Roundtrip(t.Context(), expected)
		if actual != expected {
			t.Errorf("expected: %d, but got: %d", expected, actual)
		}
	}
}

func Test_U16Roundtrip(t *testing.T) {
	fac, err := NewInstructionsFactory(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	for expected := range inclusive[uint16](0, math.MaxUint16) {
		actual := ins.U16Roundtrip(t.Context(), expected)
		if actual != expected {
			t.Errorf("expected: %d, but got: %d", expected, actual)
		}
	}
}

func Test_S32Roundtrip(t *testing.T) {
	fac, err := NewInstructionsFactory(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	for expected := range inclusiveStep[int32](math.MinInt32, math.MaxInt32, 10_000) {
		actual := ins.S32Roundtrip(t.Context(), expected)
		if actual != expected {
			t.Errorf("expected: %d, but got: %d", expected, actual)
		}
	}
}

func Test_U32Roundtrip(t *testing.T) {
	fac, err := NewInstructionsFactory(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	for expected := range inclusiveStep[uint32](0, math.MaxUint32, 10_000) {
		actual := ins.U32Roundtrip(t.Context(), expected)
		if actual != expected {
			t.Errorf("expected: %d, but got: %d", expected, actual)
		}
	}
}
