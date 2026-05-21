package variants

import (
	"testing"
)

// TestClassify_UnitCase exercises lifting a unit-case variant returned
// from the guest. The wrapper struct must be zero-sized and the type
// switch on the Go side must accept it as the variant interface.
func TestClassify_UnitCase(t *testing.T) {
	ins := newInstance(t)

	got := ins.Classify(t.Context(), "email")
	if _, ok := got.(EntityEmail); !ok {
		t.Fatalf("Classify(\"email\") = %T, want EntityEmail", got)
	}
}

// TestClassify_PayloadCase exercises lifting a payload-bearing case
// where the payload is a primitive — the wrapper carries it in `Value`.
// Regression: the original sensitive-info-entity `custom(string)` shape.
func TestClassify_PayloadCase(t *testing.T) {
	ins := newInstance(t)

	got := ins.Classify(t.Context(), "anything-else")
	custom, ok := got.(EntityCustom)
	if !ok {
		t.Fatalf("Classify(\"anything-else\") = %T, want EntityCustom", got)
	}
	if custom.Value != "anything-else" {
		t.Errorf("EntityCustom.Value = %q, want \"anything-else\"", custom.Value)
	}
}

// TestTagAll exercises:
//   - returning a list of records from the guest (list lift of record)
//   - records that contain a variant field (variant lift inside record)
//   - the variant having both unit and payload cases in the same list
func TestTagAll(t *testing.T) {
	ins := newInstance(t)

	got := ins.TagAll(t.Context(), []string{"email", "custom-thing", "ip"})
	if len(got) != 3 {
		t.Fatalf("TagAll len = %d, want 3", len(got))
	}
	if _, ok := got[0].Kind.(EntityEmail); !ok {
		t.Errorf("got[0].Kind = %T, want EntityEmail", got[0].Kind)
	}
	if got[0].Start != 0 || got[0].End != 1 {
		t.Errorf("got[0] indices = %d/%d, want 0/1", got[0].Start, got[0].End)
	}
	custom, ok := got[1].Kind.(EntityCustom)
	if !ok {
		t.Errorf("got[1].Kind = %T, want EntityCustom", got[1].Kind)
	} else if custom.Value != "custom-thing" {
		t.Errorf("got[1] Custom.Value = %q, want \"custom-thing\"", custom.Value)
	}
	if _, ok := got[2].Kind.(EntityIpAddress); !ok {
		t.Errorf("got[2].Kind = %T, want EntityIpAddress", got[2].Kind)
	}
}

// TestChoose_DirectRecordDispatch exercises the WIT shorthand
// `case(case)` — when a variant case's only payload is a named record
// sharing its name, gravity should let the record satisfy the variant
// marker directly. Callers MUST be able to pass `Allow{...}` (not
// `ConfigAllow{Value: Allow{...}}`).
func TestChoose_DirectRecordDispatch(t *testing.T) {
	ins := newInstance(t)

	window := uint32(3)
	got := ins.Choose(t.Context(), Allow{
		Entities: []Entity{
			EntityEmail{},
			EntityCustom{Value: "tagged"},
		},
		ContextWindowSize: &window,
	})
	want := "allow:2:ctx=Some(3)"
	if got != want {
		t.Errorf("Choose(Allow{...}) = %q, want %q", got, want)
	}

	got = ins.Choose(t.Context(), Deny{
		Entities: []Entity{EntityIpAddress{}},
	})
	want = "deny:1"
	if got != want {
		t.Errorf("Choose(Deny{...}) = %q, want %q", got, want)
	}
}

// TestChooseMany exercises a variant whose payload is `list<variant>`
// (e.g. `allow-all(list<entity>)`). Gravity must wrap the list in a
// `EntitiesAllowAll{Value: []Entity{...}}` struct.
func TestChooseMany(t *testing.T) {
	ins := newInstance(t)

	got := ins.ChooseMany(t.Context(), EntitiesAllowAll{
		Value: []Entity{
			EntityEmail{},
			EntityPhoneNumber{},
			EntityCustom{Value: "x"},
		},
	})
	if got != "allow-all:3" {
		t.Errorf("ChooseMany allow-all = %q, want \"allow-all:3\"", got)
	}

	got = ins.ChooseMany(t.Context(), EntitiesDenyAll{
		Value: []Entity{EntityIpAddress{}, EntityCreditCardNumber{}},
	})
	if got != "deny-all:2" {
		t.Errorf("ChooseMany deny-all = %q, want \"deny-all:2\"", got)
	}
}

func newInstance(t *testing.T) *VariantsInstance {
	t.Helper()
	fac, err := NewVariantsFactory(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { fac.Close(t.Context()) })

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := ins.Close(t.Context()); err != nil {
			t.Error(err)
		}
	})

	return ins
}
