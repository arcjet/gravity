package example

import (
	"context"
	"fmt"
	"runtime"
	"testing"
)

type Runtime struct {
	msg string
}

func (Runtime) Os(context.Context) string             { return runtime.GOOS }
func (Runtime) Arch(context.Context) string           { return runtime.GOARCH }
func (r *Runtime) Puts(_ context.Context, msg string) { r.msg = msg }

func TestBasic(t *testing.T) {
	r := &Runtime{}
	fac, err := NewExampleFactory(t.Context(), r)
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

	wantPutsMsg := fmt.Sprintf("%s/%s", runtime.GOOS, runtime.GOARCH)
	if r.msg != wantPutsMsg {
		t.Errorf("wanted: %s, but got: %s", wantPutsMsg, r.msg)
	}
}
