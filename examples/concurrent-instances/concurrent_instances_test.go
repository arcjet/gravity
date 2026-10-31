package concurrent_instances

import (
	"context"
	"fmt"
	"sync"
	"testing"
)

// One factory serves an instance per goroutine. Before the fix the second
// live instance failed: "module[example_concurrent_instances.wasm] has
// already been instantiated".
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
