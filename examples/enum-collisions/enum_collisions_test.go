package enum_collisions

import (
	"context"
	"fmt"
	"strings"
	"testing"

	"github.com/tetratelabs/wazero"
)

// Every enum case is a constant named for its enum, so cases that share a
// name across enums, or with a type, are distinct identifiers. Before the
// fix the bindings failed go vet: PeerSvid and Point were redeclared.

func newInstance(t *testing.T) *EnumCollisionsInstance {
	t.Helper()
	// The world imports `types` only for its types, so the factory takes no
	// host argument for it.
	fac, err := NewEnumCollisionsFactory(t.Context(), wazero.NewRuntimeConfig())
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

func TestSharedCaseNamesStayDistinct(t *testing.T) {
	ins := newInstance(t)
	for in, want := range map[CredentialKind]Via{
		CredentialKindPeerSvid:    ViaPeerSvid,
		CredentialKindSignedToken: ViaSignedToken,
		CredentialKindBearer:      ViaAnonymous,
	} {
		if got, err := ins.ViaFor(t.Context(), in); err != nil {
			t.Fatal(err)
		} else if got != want {
			t.Errorf("ViaFor(%v) = %v, want %v", in, got, want)
		}
	}
}

func TestCaseNamedLikeAType(t *testing.T) {
	ins := newInstance(t)
	p := Point{X: 3, Y: -4}
	if got, err := ins.Describe(t.Context(), ShapePoint, p); err != nil {
		t.Fatal(err)
	} else if got != "point(3, -4)" {
		t.Errorf("Describe(ShapePoint) = %q", got)
	}
	if got, err := ins.Describe(t.Context(), ShapeLine, p); err != nil {
		t.Fatal(err)
	} else if got != "line to (3, -4)" {
		t.Errorf("Describe(ShapeLine) = %q", got)
	}
}

// checkParse checks that parse returns each case, and true, for the name
// its String returns, and (nil, false) for anything that is not exactly a
// WIT case name.
func checkParse[E comparable](t *testing.T, parse func(string) (E, bool), cases ...E) {
	t.Helper()
	var none E
	for _, c := range cases {
		name := any(c).(fmt.Stringer).String()
		if got, ok := parse(name); !ok || got != c {
			t.Errorf("parse(%q) = %v, %v; want %v, true", name, got, ok, c)
		}
		for _, wrong := range []string{strings.ToUpper(name), " " + name, name + " "} {
			if got, ok := parse(wrong); ok || got != none {
				t.Errorf("parse(%q) = %v, %v; want nil, false", wrong, got, ok)
			}
		}
	}
	for _, unknown := range []string{"", "unknown", "PeerSvid"} {
		if got, ok := parse(unknown); ok || got != none {
			t.Errorf("parse(%q) = %v, %v; want nil, false", unknown, got, ok)
		}
	}
}

func TestParseInvertsString(t *testing.T) {
	checkParse[Via](t, ParseVia, ViaPeerSvid, ViaSignedToken, ViaAnonymous)
	checkParse[CredentialKind](t, ParseCredentialKind,
		CredentialKindPeerSvid, CredentialKindSignedToken, CredentialKindBearer)
	checkParse[Shape](t, ParseShape, ShapePoint, ShapeLine)
}
