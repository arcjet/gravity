package interface_exports

import (
	"context"
	"runtime"
	"testing"
)

// A world that exports interfaces: each becomes a Go type holding its
// functions, answered by an instance method.

type Log struct{ lines []string }

func (l *Log) Info(_ context.Context, msg string) { l.lines = append(l.lines, msg) }

func newInstance(t *testing.T, l *Log) *InterfaceExportsInstance {
	t.Helper()
	fac, err := NewInterfaceExportsFactory(t.Context(), l)
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

func TestInterfaceTypesAndFunctions(t *testing.T) {
	geo := newInstance(t, &Log{}).Geometry()
	for p, want := range map[Point]Quadrant{
		{0, 0}:   QuadrantOrigin,
		{2, 3}:   QuadrantFirst,
		{-2, 3}:  QuadrantSecond,
		{-2, -3}: QuadrantThird,
		{2, -3}:  QuadrantFourth,
		{0, 5}:   QuadrantAxis,
	} {
		if got := geo.QuadrantOf(t.Context(), p); got != want {
			t.Errorf("QuadrantOf(%+v) = %v, want %v", p, got, want)
		}
	}
	s := Segment{Start: Point{X: -4, Y: 2}, End: Point{X: 6, Y: -8}}
	if got := geo.Midpoint(t.Context(), s); got != (Point{X: 1, Y: -3}) {
		t.Errorf("Midpoint = %+v", got)
	}
	if got := geo.LengthSquared(t.Context(), s); got != 200 {
		t.Errorf("LengthSquared = %d, want 200", got)
	}
}

func TestInterfaceCallsBackIntoTheHost(t *testing.T) {
	l := &Log{}
	ins := newInstance(t, l)
	if got := ins.Greeter().Greet(t.Context(), "rob"); got != "hello, rob" {
		t.Errorf("Greet = %q", got)
	}
	if len(l.lines) != 1 || l.lines[0] != "greeting rob" {
		t.Errorf("host log = %q", l.lines)
	}
}

func TestWorldFunctionsBesideInterfaces(t *testing.T) {
	if got := newInstance(t, &Log{}).Version(t.Context()); got != "0.1.0" {
		t.Errorf("Version = %q", got)
	}
}

// bytesPerCall is the heap allocated per call of f, averaged over calls after
// a warm-up call has filled the instance's export memo.
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

// A memoized call allocates a few hundred bytes; 4 KB is under a third of what
// Midpoint, the smaller of the two calls, allocated unmemoized.
const budget = 4 << 10

// An exported interface's methods look up the guest's exports through the
// instance's memo, as world-level exports do. api.Module.ExportedFunction
// builds a new call engine each time it is called, and a call reaches the
// export, its cabi_post_* and, for a string argument, cabi_realloc.
func TestInterfaceLookupsAreMemoized(t *testing.T) {
	ins := newInstance(t, &Log{})
	geo := ins.Geometry()
	s := Segment{Start: Point{X: -4, Y: 2}, End: Point{X: 6, Y: -8}}
	midpoint := bytesPerCall(func() {
		if got := geo.Midpoint(t.Context(), s); got != (Point{X: 1, Y: -3}) {
			t.Fatalf("Midpoint = %+v", got)
		}
	})
	greeter := ins.Greeter()
	greet := bytesPerCall(func() {
		if got := greeter.Greet(t.Context(), "rob"); got != "hello, rob" {
			t.Fatalf("Greet = %q", got)
		}
	})
	t.Logf("Midpoint: %d bytes per call; Greet: %d bytes per call", midpoint, greet)
	if midpoint > budget {
		t.Errorf("Midpoint allocated %d bytes per call, want at most %d: interface export lookups are not memoized", midpoint, budget)
	}
	if greet > budget {
		t.Errorf("Greet allocated %d bytes per call, want at most %d: interface export lookups are not memoized", greet, budget)
	}
}
