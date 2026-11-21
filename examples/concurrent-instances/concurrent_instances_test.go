package concurrent_instances

import (
	"context"
	"fmt"
	"sync"
	"testing"

	"github.com/tetratelabs/wazero"
)

// Host keeps no state, so every instance of a factory can call it at once.
type Host struct{}

func (Host) Name(_ context.Context, n uint32) string {
	return fmt.Sprintf("guest-%d", n)
}

// One factory holds two live instances at once, and each answers calls. wazero
// refuses a second module of the same name in one runtime, so this fails with
// "has already been instantiated" unless every instance is anonymous.
func TestTwoLiveInstancesFromOneFactory(t *testing.T) {
	fac, err := NewConcurrentInstancesFactory(t.Context(), wazero.NewRuntimeConfig(), Host{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(context.Background())
	first, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer first.Close(context.Background())
	second, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer second.Close(context.Background())
	if s := first.Join(t.Context(), []string{"a", "b"}, "-"); s != "a-b" {
		t.Errorf("first.Join = %q, want %q", s, "a-b")
	}
	if s := second.Join(t.Context(), []string{"c", "d"}, "+"); s != "c+d" {
		t.Errorf("second.Join = %q, want %q", s, "c+d")
	}
}

// One factory serves an instance per goroutine. Greet's host call finds its
// instance in the factory's registry while other goroutines instantiate and
// close theirs, so under -race this checks that the registry is shared safely.
func TestInstancesOnSeparateGoroutines(t *testing.T) {
	fac, err := NewConcurrentInstancesFactory(t.Context(), wazero.NewRuntimeConfig(), Host{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(context.Background())
	var wg sync.WaitGroup
	for g := range 8 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			ins, err := fac.Instantiate(context.Background())
			if err != nil {
				t.Error(err)
				return
			}
			defer ins.Close(context.Background())
			for i := range 50 {
				want := fmt.Sprintf("%d-%d", g, i)
				if s := ins.Join(context.Background(), []string{fmt.Sprint(g), fmt.Sprint(i)}, "-"); s != want {
					t.Errorf("Join = %q, want %q", s, want)
					return
				}
				n := uint32(g*100 + i)
				if s, want := ins.Greet(context.Background(), n), fmt.Sprintf("hello, guest-%d", n); s != want {
					t.Errorf("Greet = %q, want %q", s, want)
					return
				}
			}
		}()
	}
	wg.Wait()
}
