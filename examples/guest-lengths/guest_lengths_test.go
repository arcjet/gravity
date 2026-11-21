package guest_lengths

import (
	"context"
	"strings"
	"testing"

	"github.com/tetratelabs/wazero"
)

// The guest supplies list lengths and the blocks cabi_realloc returns, and
// the host must not trust either: a length sizes a host allocation, and a
// block is written into. The host also refuses a list whose length the
// guest's 32-bit ABI cannot carry.

func newInstance(t *testing.T) *GuestLengthsInstance {
	t.Helper()
	fac, err := NewGuestLengthsFactory(t.Context())
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

func requireErrorContaining(t *testing.T, err error, want string) {
	t.Helper()
	if err == nil {
		t.Fatalf("expected an error containing %q, got nil", want)
	}
	if !strings.Contains(err.Error(), want) {
		t.Fatalf("expected an error containing %q, got %q", want, err)
	}
}

func requirePanicContaining(t *testing.T, want string, f func()) {
	t.Helper()
	defer func() {
		t.Helper()
		r := recover()
		err, ok := r.(error)
		if !ok {
			t.Fatalf("expected a panic with an error, got %v", r)
		}
		requireErrorContaining(t, err, want)
	}()
	f()
}

const doesNotFit = "list of 1048576 elements at 0 does not fit in guest memory"

// A length longer than guest memory is refused before it sizes a slice.
func TestLongListResult(t *testing.T) {
	ins := newInstance(t)
	_, err := ins.LongListResult(t.Context())
	requireErrorContaining(t, err, doesNotFit)
}

func TestLongListPanics(t *testing.T) {
	ins := newInstance(t)
	requirePanicContaining(t, doesNotFit, func() { ins.LongList(t.Context()) })
}

func TestSum(t *testing.T) {
	ins := newInstance(t)
	for _, items := range [][]uint32{nil, {}, {1, 2, 3}} {
		var want uint32
		for _, item := range items {
			want += item
		}
		got, err := ins.Sum(t.Context(), items)
		if err != nil || got != want {
			t.Fatalf("Sum(%v) = %d, %v; want %d", items, got, err, want)
		}
	}
}

// A block cabi_realloc returns outside guest memory is refused before any
// element is written into it.
func TestListBlockOutsideMemory(t *testing.T) {
	ins := newInstance(t)
	ins.BreakRealloc(t.Context())
	_, err := ins.Sum(t.Context(), []uint32{1, 2})
	requireErrorContaining(t, err, "cabi_realloc returned 8 bytes at 4294967040, outside guest memory")

	// cabi_realloc recovers, and so does the instance.
	got, err := ins.Sum(t.Context(), []uint32{1, 2})
	if err != nil || got != 3 {
		t.Fatalf("Sum after a refused block = %d, %v", got, err)
	}
}

// An option<r64> is 520 bytes in guest memory, so 8,259,553 of them, one
// more than fit in 2^32-1 bytes, are refused before cabi_realloc is called
// with a truncated size. In Go each is a nil pointer.
func TestListTooLongForGuest(t *testing.T) {
	ins := newInstance(t)
	got, err := ins.Count(t.Context(), make([]*R64, 3))
	if err != nil || got != 3 {
		t.Fatalf("Count(3) = %d, %v", got, err)
	}
	_, err = ins.Count(t.Context(), make([]*R64, 8_259_553))
	requireErrorContaining(t, err, "list of 8259553 elements is too long")
}

// The area holding indirectly passed parameters is checked as a list block is.
func TestParamAreaOutsideMemory(t *testing.T) {
	ins := newInstance(t)
	wide := func() (uint32, error) {
		return ins.Wide(t.Context(), Seventeen{
			P1: 1, P2: 2, P3: 3, P4: 4, P5: 5, P6: 6, P7: 7, P8: 8, P9: 9,
			P10: 10, P11: 11, P12: 12, P13: 13, P14: 14, P15: 15, P16: 16, P17: 17,
		})
	}
	got, err := wide()
	if err != nil || got != 153 {
		t.Fatalf("Wide = %d, %v", got, err)
	}
	ins.BreakRealloc(t.Context())
	_, err = wide()
	requireErrorContaining(t, err, "cabi_realloc returned 68 bytes at 4294967040, outside guest memory")
}

// A guest memory at the maximum 65536 pages is 4 GiB, and wazero's
// Memory.Size() reads zero for it. The checks size memory from its page
// count instead, so lists and parameter blocks still pass them. wazero's
// compiler engine cannot run a guest whose memory is at the maximum, but its
// interpreter can, so this test builds the factory with the interpreter.
func TestFullMemory(t *testing.T) {
	ctx := t.Context()
	runtime := wazero.NewRuntimeWithConfig(ctx, wazero.NewRuntimeConfigInterpreter())
	t.Cleanup(func() { runtime.Close(context.Background()) })
	module, err := runtime.CompileModule(ctx, wasmFileGuestLengths)
	if err != nil {
		t.Fatal(err)
	}
	fac := &GuestLengthsFactory{runtime: runtime, module: module, exports: &exportedFunctionCache{}}
	ins, err := fac.Instantiate(ctx)
	if err != nil {
		t.Fatal(err)
	}

	seventeen := Seventeen{
		P1: 1, P2: 2, P3: 3, P4: 4, P5: 5, P6: 6, P7: 7, P8: 8, P9: 9,
		P10: 10, P11: 11, P12: 12, P13: 13, P14: 14, P15: 15, P16: 16, P17: 17,
	}
	// The guest's allocator gets its heap with memory.grow, which fails once
	// memory is at the maximum. Calling each export first leaves it a heap
	// with room for these calls' small allocations.
	if _, err := ins.Sum(ctx, []uint32{1, 2, 3}); err != nil {
		t.Fatal(err)
	}
	if _, err := ins.Wide(ctx, seventeen); err != nil {
		t.Fatal(err)
	}

	const maxPages = 65536
	pages, _ := ins.module.Memory().Grow(0)
	if _, ok := ins.module.Memory().Grow(maxPages - pages); !ok {
		t.Fatalf("could not grow guest memory from %d to %d pages", pages, maxPages)
	}
	if size := ins.module.Memory().Size(); size != 0 {
		t.Fatalf("Memory.Size() = %d at %d pages; this test relies on it reading zero", size, maxPages)
	}

	if got, err := ins.Sum(ctx, []uint32{1, 2, 3}); err != nil || got != 6 {
		t.Fatalf("Sum = %d, %v; want 6", got, err)
	}
	got, err := ins.Wide(ctx, seventeen)
	if err != nil || got != 153 {
		t.Fatalf("Wide = %d, %v; want 153", got, err)
	}
	// 2^20 u32s at address 0 fit in 4 GiB, so the lift reads them.
	list, err := ins.LongListResult(ctx)
	if err != nil || len(list) != 1<<20 {
		t.Fatalf("LongListResult = %d elements, %v; want %d", len(list), err, 1<<20)
	}
}
