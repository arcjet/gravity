package concurrent_instances

import (
	"context"
	"fmt"
	"sync"
	"testing"
)

// One factory holds two live instances at once, and each answers calls. wazero
// refuses a second module of the same name in one runtime, so this fails with
// "has already been instantiated" unless every instance is anonymous.
func TestTwoLiveInstancesFromOneFactory(t *testing.T) {
	fac, err := NewConcurrentInstancesFactory(t.Context())
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

// One factory serves an instance per goroutine.
func TestInstancesOnSeparateGoroutines(t *testing.T) {
	fac, err := NewConcurrentInstancesFactory(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(context.Background())
	var wg sync.WaitGroup
	for g := range 4 {
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
			}
		}()
	}
	wg.Wait()
}
