package interface_exports

import (
	"context"
	"testing"
)

// A world that exports interfaces: each becomes a Go type holding its
// functions, answered by an instance method. Before the fix gravity panicked
// generating these bindings: "not yet implemented: generate interface exports".

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
