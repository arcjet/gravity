package enum_collisions

import (
	"context"
	"testing"
)

// Every enum case is a constant named for its enum, so cases that share a
// name across enums, or with a type, are distinct identifiers. Before the
// fix the bindings failed go vet: PeerSvid and Point were redeclared.

func newInstance(t *testing.T) *EnumCollisionsInstance {
	t.Helper()
	// The world's `use types.{...}` makes `types` an import with no
	// functions, so its host argument has nothing to implement.
	fac, err := NewEnumCollisionsFactory(t.Context(), nil)
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
		if got := ins.ViaFor(t.Context(), in); got != want {
			t.Errorf("ViaFor(%v) = %v, want %v", in, got, want)
		}
	}
}

func TestCaseNamedLikeAType(t *testing.T) {
	ins := newInstance(t)
	p := Point{X: 3, Y: -4}
	if got := ins.Describe(t.Context(), ShapePoint, p); got != "point(3, -4)" {
		t.Errorf("Describe(ShapePoint) = %q", got)
	}
	if got := ins.Describe(t.Context(), ShapeLine, p); got != "line to (3, -4)" {
		t.Errorf("Describe(ShapeLine) = %q", got)
	}
}
