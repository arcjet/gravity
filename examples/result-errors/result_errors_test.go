package result_errors

import (
	"context"
	"errors"
	"reflect"
	"strings"
	"testing"
)

// A result<T, E> whose E is not a string returns its err case as a
// *ResultError[E]; errors.As recovers the E. Before the fix gravity panicked
// generating these bindings: "TODO(#4): implement remaining result conversion".

func refusalFor(n uint32) Refusal {
	return []Refusal{RefusalNoCredential, RefusalMalformed, RefusalUntypedPeer, RefusalForeignDomain}[n%4]
}

// Host answers each import with the same err cases the guest's exports use,
// and with a plain error (one with no WIT representation) for n == 99.
type Host struct{}

var errPlain = errors.New("not a WIT error")

func (Host) Check(_ context.Context, n uint32) (uint32, error) {
	switch {
	case n == 99:
		return 0, errPlain
	case n < 4:
		return 0, &ResultError[Refusal]{Value: refusalFor(n)}
	}
	return n + 1, nil
}

func (Host) Probe(_ context.Context, n uint32) error {
	if n%2 == 1 {
		return &ResultError[Problem]{Value: Problem{Code: n, Detail: "host"}}
	}
	return nil
}

func (Host) Explain(_ context.Context, n uint32) (string, error) {
	switch n {
	case 0:
		return "", &ResultError[Fault]{Value: FaultRefused{Value: RefusalUntypedPeer}}
	case 1:
		return "", &ResultError[Fault]{Value: Problem{Code: 7, Detail: "x"}}
	case 2:
		return "", &ResultError[Fault]{Value: FaultMessage{Value: "from host"}}
	case 3:
		return "", &ResultError[Fault]{Value: FaultUnknown{}}
	}
	return "host fine", nil
}

func newInstance(t *testing.T) *ResultErrorsInstance {
	t.Helper()
	fac, err := NewResultErrorsFactory(t.Context(), Host{})
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

// errValue recovers the E of a *ResultError[E], failing the test otherwise.
func errValue[E any](t *testing.T, err error) E {
	t.Helper()
	var re *ResultError[E]
	if !errors.As(err, &re) {
		var zero E
		t.Fatalf("error %v (%T) is not a *ResultError[%T]", err, err, zero)
	}
	return re.Value
}

func TestEnumErr(t *testing.T) {
	ins := newInstance(t)
	for n := range uint32(4) {
		if _, err := ins.Decide(t.Context(), n); errValue[Refusal](t, err) != refusalFor(n) {
			t.Errorf("Decide(%d): err %v, want %v", n, err, refusalFor(n))
		}
	}
	got, err := ins.Decide(t.Context(), 7)
	if err != nil || got != (Decided{Name: "n7", N: 7}) {
		t.Errorf("Decide(7) = %+v, %v", got, err)
	}
	// The enum's String names the WIT case, so the error reads as WIT wrote it.
	if _, err := ins.Decide(t.Context(), 2); err == nil || err.Error() != "untyped-peer" {
		t.Errorf("Decide(2) error text = %v, want untyped-peer", err)
	}
}

func TestRecordErr(t *testing.T) {
	ins := newInstance(t)
	if got, err := ins.Inspect(t.Context(), 4); err != nil || got != 8 {
		t.Errorf("Inspect(4) = %d, %v", got, err)
	}
	_, err := ins.Inspect(t.Context(), 5)
	if p := errValue[Problem](t, err); p != (Problem{Code: 5, Detail: "odd"}) {
		t.Errorf("Inspect(5) err = %+v", p)
	}
}

func TestVariantErr(t *testing.T) {
	ins := newInstance(t)
	for n, want := range map[uint32]Fault{
		0: FaultRefused{Value: RefusalForeignDomain},
		1: Problem{Code: 1, Detail: "p"},
		2: FaultMessage{Value: "m"},
		3: FaultUnknown{},
	} {
		_, err := ins.Classify(t.Context(), n)
		if got := errValue[Fault](t, err); !reflect.DeepEqual(got, want) {
			t.Errorf("Classify(%d) err = %#v, want %#v", n, got, want)
		}
	}
	if got, err := ins.Classify(t.Context(), 9); err != nil || got != "fine" {
		t.Errorf("Classify(9) = %q, %v", got, err)
	}
}

func TestErrWithoutOk(t *testing.T) {
	ins := newInstance(t)
	if err := ins.OnlyErr(t.Context(), 0); err != nil {
		t.Errorf("OnlyErr(0) = %v", err)
	}
	if got := errValue[Refusal](t, ins.OnlyErr(t.Context(), 3)); got != RefusalForeignDomain {
		t.Errorf("OnlyErr(3) err = %v", got)
	}
}

func TestHostReturnsTypedErrs(t *testing.T) {
	ins := newInstance(t)
	for n := range uint32(4) {
		if _, err := ins.ViaCheck(t.Context(), n); errValue[Refusal](t, err) != refusalFor(n) {
			t.Errorf("ViaCheck(%d) err = %v", n, err)
		}
	}
	if got, err := ins.ViaCheck(t.Context(), 10); err != nil || got != 11 {
		t.Errorf("ViaCheck(10) = %d, %v", got, err)
	}
	if err := ins.ViaProbe(t.Context(), 2); err != nil {
		t.Errorf("ViaProbe(2) = %v", err)
	}
	if p := errValue[Problem](t, ins.ViaProbe(t.Context(), 3)); p != (Problem{Code: 3, Detail: "host"}) {
		t.Errorf("ViaProbe(3) err = %+v", p)
	}
	for n, want := range map[uint32]Fault{
		0: FaultRefused{Value: RefusalUntypedPeer},
		1: Problem{Code: 7, Detail: "x"},
		2: FaultMessage{Value: "from host"},
		3: FaultUnknown{},
	} {
		_, err := ins.ViaExplain(t.Context(), n)
		if got := errValue[Fault](t, err); !reflect.DeepEqual(got, want) {
			t.Errorf("ViaExplain(%d) err = %#v, want %#v", n, got, want)
		}
	}
}

// A host error with no WIT representation traps the guest: the export
// returns an error, and it is not a *ResultError.
func TestHostPlainErrorTraps(t *testing.T) {
	ins := newInstance(t)
	_, err := ins.ViaCheck(t.Context(), 99)
	var re *ResultError[Refusal]
	if err == nil || errors.As(err, &re) {
		t.Fatalf("ViaCheck(99) err = %v, want a trap", err)
	}
	if !strings.Contains(err.Error(), errPlain.Error()) {
		t.Errorf("trap %q does not carry the host's error", err)
	}
}
