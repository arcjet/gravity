package cancellation

import (
	"context"
	"errors"
	"fmt"
	"testing"
	"time"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/sys"
)

// A guest call runs on the caller's goroutine. With wazero's default runtime
// configuration a done context does not stop it; NewCancellationFactory takes
// the configuration, so the caller can choose one that does, and bound guest
// memory.

func newInstance(t *testing.T, newFactory func() (*CancellationFactory, error)) *CancellationInstance {
	t.Helper()
	fac, err := newFactory()
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

// callWithin runs call, and ends the test binary if it has not returned after
// limit: nothing can stop the guest then, and closing its module in the
// test's cleanup would wait for it.
func callWithin(limit time.Duration, call func() error) error {
	done := make(chan error, 1)
	go func() { done <- call() }()
	select {
	case err := <-done:
		return err
	case <-time.After(limit):
		panic(fmt.Sprintf("the call was still running after %v", limit))
	}
}

func TestDoneContextStopsCall(t *testing.T) {
	config := wazero.NewRuntimeConfig().WithCloseOnContextDone(true)
	ins := newInstance(t, func() (*CancellationFactory, error) {
		return NewCancellationFactory(t.Context(), config)
	})

	ctx, cancel := context.WithTimeout(t.Context(), 100*time.Millisecond)
	defer cancel()
	err := callWithin(10*time.Second, func() error { return ins.Spin(ctx) })
	var exit *sys.ExitError
	if !errors.As(err, &exit) || exit.ExitCode() != sys.ExitCodeDeadlineExceeded {
		t.Fatalf("expected a deadline exit, got %v", err)
	}

	// The instance was closed, so a later call fails rather than running.
	_, err = ins.Allocate(t.Context(), 1)
	if err == nil {
		t.Fatal("expected a call on the closed instance to fail")
	}
}

func TestMemoryLimit(t *testing.T) {
	const page = 64 << 10
	config := wazero.NewRuntimeConfig().WithMemoryLimitPages(64)
	ins := newInstance(t, func() (*CancellationFactory, error) {
		return NewCancellationFactory(t.Context(), config)
	})
	if got, err := ins.Allocate(t.Context(), 16*page); err != nil || got != 16*page {
		t.Fatalf("Allocate(16 pages) = %d, %v", got, err)
	}
	if _, err := ins.Allocate(t.Context(), 128*page); err == nil {
		t.Fatal("expected an allocation past the memory limit to fail")
	}
}

// wazero's default configuration sets no memory limit.
func TestDefaultConfiguration(t *testing.T) {
	const page = 64 << 10
	ins := newInstance(t, func() (*CancellationFactory, error) {
		return NewCancellationFactory(t.Context(), wazero.NewRuntimeConfig())
	})
	if got, err := ins.Allocate(t.Context(), 128*page); err != nil || got != 128*page {
		t.Fatalf("Allocate(128 pages) = %d, %v", got, err)
	}
}
