package export_memo

import (
	"context"
	"fmt"
	"runtime"
	"strings"
	"testing"
)

// Before the fix, every guest export the bindings reached — the export
// itself, its cabi_post_*, and cabi_realloc once per string or list lowered —
// was looked up with api.Module.ExportedFunction, which builds a new call
// engine each time. These tests bound the bytes a call allocates: measured
// unmemoized, Join allocated 427 KB per call and HostWords 404 KB.

type Host struct{}

func (Host) Words(_ context.Context, n uint32) []string {
	out := make([]string, n)
	for i := range out {
		out[i] = fmt.Sprintf("w%d", i)
	}
	return out
}

func newFactory(t testing.TB) *ExportMemoFactory {
	t.Helper()
	fac, err := NewExportMemoFactory(context.Background(), Host{})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { fac.Close(context.Background()) })
	return fac
}

func newInstance(t testing.TB) (*ExportMemoFactory, *ExportMemoInstance) {
	t.Helper()
	fac := newFactory(t)
	ins, err := fac.Instantiate(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { ins.Close(context.Background()) })
	return fac, ins
}

// bytesPerCall is the heap allocated per call of f, averaged over calls after
// a warm-up call has filled the memo.
func bytesPerCall(f func()) uint64 {
	f()
	const calls = 20
	var before, after runtime.MemStats
	runtime.GC()
	runtime.ReadMemStats(&before)
	for range calls {
		f()
	}
	runtime.ReadMemStats(&after)
	return (after.TotalAlloc - before.TotalAlloc) / calls
}

// A memoized call allocates a small multiple of its payload; 64 KB is under a
// sixth of what either call allocated unmemoized.
const budget = 64 << 10

func TestExportSideLookupsAreMemoized(t *testing.T) {
	_, ins := newInstance(t)
	parts := make([]string, 32)
	for i := range parts {
		parts[i] = fmt.Sprintf("part-%d", i)
	}
	want := strings.Join(parts, ", ")
	got := bytesPerCall(func() {
		if s := ins.Join(context.Background(), parts, ", "); s != want {
			t.Fatalf("Join = %q, want %q", s, want)
		}
	})
	t.Logf("Join: %d bytes per call", got)
	if got > budget {
		t.Errorf("Join allocated %d bytes per call, want at most %d: export lookups are not memoized", got, budget)
	}
}

func TestImportSideLookupsAreMemoized(t *testing.T) {
	_, ins := newInstance(t)
	got := bytesPerCall(func() {
		// w0..w31: ten 2-byte words and twenty-two 3-byte words.
		if n := ins.HostWords(context.Background(), 32); n != 10*2+22*3 {
			t.Fatalf("HostWords = %d, want %d", n, 10*2+22*3)
		}
	})
	t.Logf("HostWords: %d bytes per call", got)
	if got > budget {
		t.Errorf("HostWords allocated %d bytes per call, want at most %d: host-side lookups are not memoized", got, budget)
	}
}

// Close forgets the instance's memo, so the registry does not keep a closed
// module alive.
func TestCloseForgetsTheMemo(t *testing.T) {
	fac := newFactory(t)
	count := func() int {
		n := 0
		fac.exports.byModule.Range(func(any, any) bool { n++; return true })
		return n
	}
	before := count()
	ins, err := fac.Instantiate(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if count() != before+1 {
		t.Fatalf("registry holds %d memos after Instantiate, want %d", count(), before+1)
	}
	if err := ins.Close(context.Background()); err != nil {
		t.Fatal(err)
	}
	if count() != before {
		t.Errorf("registry holds %d memos after Close, want %d", count(), before)
	}
}
