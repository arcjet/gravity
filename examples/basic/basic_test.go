package basic

import (
	"context"
	"log/slog"
	"testing"
)

type SlogLogger struct{}

func (s SlogLogger) Debug(ctx context.Context, msg string) { slog.DebugContext(ctx, msg) }
func (s SlogLogger) Info(ctx context.Context, msg string)  { slog.InfoContext(ctx, msg) }
func (s SlogLogger) Warn(ctx context.Context, msg string)  { slog.WarnContext(ctx, msg) }
func (s SlogLogger) Error(ctx context.Context, msg string) { slog.ErrorContext(ctx, msg) }

func TestBasic(t *testing.T) {
	fac, err := NewBasicFactory(t.Context(), SlogLogger{})
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
}

func TestNoPrimitiveCleanup(t *testing.T) {
	fac, err := NewBasicFactory(t.Context(), SlogLogger{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	actual := ins.Primitive(t.Context())

	const expected = true
	if actual != expected {
		t.Errorf("expected: %t, but got: %t", expected, actual)
	}
}

func TestNoOptionalPrimitiveCleanup(t *testing.T) {
	fac, err := NewBasicFactory(t.Context(), SlogLogger{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	actual, ok := ins.OptionalPrimitive(t.Context())
	if !ok {
		t.Fatal(err)
	}

	const expected = true
	if actual != expected {
		t.Errorf("expected: %t, but got: %t", expected, actual)
	}
}

func TestResultPrimitiveCleanup(t *testing.T) {
	fac, err := NewBasicFactory(t.Context(), SlogLogger{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	actual, err := ins.ResultPrimitive(t.Context())
	if err != nil {
		t.Fatal(err)
	}

	const expected = true
	if actual != expected {
		t.Errorf("expected: %t, but got: %t", expected, actual)
	}
}
