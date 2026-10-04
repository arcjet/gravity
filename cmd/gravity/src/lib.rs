pub mod codegen;
pub mod go;

use crate::go::GoType;
use wit_bindgen_core::{
    abi::WasmType,
    dealias,
    wit_parser::{Case, Resolve, Result_, Type, TypeDef, TypeDefKind, TypeId, TypeOwner},
};

// Temporary re-export while we migrate.
pub use codegen::Func;

/// How a single variant case is represented in Go.
pub enum CaseDispatchKind {
    /// The case payload's named record IS the dispatch type — the record
    /// implements the variant's marker interface directly. Constructed as
    /// `MyRecord{...}`.
    DirectRecord,
    /// A dedicated `{variant_name}-{case_name}` wrapper struct holds the
    /// optional payload in a `Value` field. Constructed as
    /// `Wrapper{Value: payload}` or `Wrapper{}` for unit cases.
    Wrapped,
}

/// Detect the WIT shorthand `case-name(case-name)` where the payload is a
/// named record sharing the case's name — the historical arcjet shape
/// (`allow-email-validation-config(allow-email-validation-config)`). We let
/// the record implement the marker interface directly so existing call
/// sites that construct the record value as the variant keep working.
pub fn case_dispatch_kind(case: &Case, resolve: &Resolve) -> CaseDispatchKind {
    if let Some(Type::Id(payload_id)) = &case.ty {
        let payload_def = &resolve.types[*payload_id];
        if matches!(payload_def.kind, TypeDefKind::Record(_))
            && payload_def.name.as_deref() == Some(case.name.as_str())
        {
            return CaseDispatchKind::DirectRecord;
        }
    }
    CaseDispatchKind::Wrapped
}

/// Kebab-case Go name for the type a variant case dispatches against in a
/// type-switch.
pub fn case_dispatch_name(variant_name: &str, case: &Case, resolve: &Resolve) -> String {
    match case_dispatch_kind(case, resolve) {
        CaseDispatchKind::DirectRecord => match case.ty {
            Some(Type::Id(payload_id)) => qualified_type_name(payload_id, resolve),
            _ => unreachable!("DirectRecord requires a Type::Id payload"),
        },
        CaseDispatchKind::Wrapped => format!("{variant_name}-{}", case.name),
    }
}

/// Returns a globally-unique kebab-case name suitable for deriving a Go
/// identifier from a WIT type. WIT lets two interfaces declare types of
/// the same name (e.g. both `email-validator-overrides` and `verify-bot`
/// declare an `enum validator-response`); we qualify only the colliding
/// names with their owning interface so stable single-instance names like
/// `algorithm-result` stay flat. The result is fed to
/// `GoIdentifier::public`, so it must remain in kebab-case.
pub fn qualified_type_name(type_id: TypeId, resolve: &Resolve) -> String {
    let canonical = dealias(resolve, type_id);
    let type_def = &resolve.types[canonical];
    let name = type_def
        .name
        .as_ref()
        .expect("expected named type for qualified_type_name");

    // Skip `Type` aliases when looking for collisions: they re-export an
    // existing type rather than introducing a new one.
    let collides = resolve.types.iter().any(|(other_id, other_def)| {
        other_id != canonical
            && other_def.name.as_deref() == Some(name.as_str())
            && !matches!(other_def.kind, TypeDefKind::Type(_))
    });

    if !collides {
        return name.clone();
    }

    match type_def.owner {
        TypeOwner::Interface(id) => {
            let interface_name = resolve.interfaces[id]
                .name
                .as_ref()
                .expect("interface missing name");
            format!("{interface_name}-{name}")
        }
        TypeOwner::World(_) | TypeOwner::None => name.clone(),
    }
}

/// Go's reserved words, which cannot name a package.
const GO_KEYWORDS: [&str; 25] = [
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
];

/// The Go package name for the bindings of `world`.
///
/// Without `--package` it is the world's name in snake case. An explicit
/// name decouples the package from the world, so a consumer can import
/// `identity` for a world called `identity-core`. It must be a name Go will
/// compile: an ASCII letter or underscore, then letters, digits and
/// underscores, not `_` alone and not a keyword.
pub fn go_package_name(world: &str, package: Option<&str>) -> Result<String, String> {
    let Some(name) = package else {
        return Ok(world.replace('-', "_"));
    };
    let mut chars = name.chars();
    let starts = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    let rest = chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !starts || !rest || name == "_" || GO_KEYWORDS.contains(&name) {
        return Err(format!("{name:?} is not a Go package name"));
    }
    Ok(name.to_owned())
}

/// Resolves a Wasm type to a Go type.
pub fn resolve_wasm_type(typ: &WasmType) -> GoType {
    match typ {
        WasmType::I32 => GoType::Uint32,
        WasmType::I64 => GoType::Uint64,
        WasmType::F32 => GoType::Float32,
        WasmType::F64 => GoType::Float64,
        WasmType::Pointer => GoType::Uint64,
        WasmType::PointerOrI64 => GoType::Uint64,
        WasmType::Length => GoType::Uint64,
    }
}

/// Resolves a WIT type to a Go type.
///
/// # Panics
///
/// This function panics if:
///
/// - The type definition cannot be found in the resolve context.
/// - The type is still unimplemented.
/// - The type does not have a name when it is expected to have one (enums, records, type aliases).
pub fn resolve_type(typ: &Type, resolve: &Resolve) -> GoType {
    match typ {
        // Basic types.
        Type::Bool => GoType::Bool,
        Type::U8 => GoType::Uint8,
        Type::U16 => GoType::Uint16,
        Type::U32 => GoType::Uint32,
        Type::U64 => GoType::Uint64,
        Type::S8 => GoType::Int8,
        Type::S16 => GoType::Int16,
        Type::S32 => GoType::Int32,
        Type::S64 => GoType::Int64,
        Type::F32 => GoType::Float32,
        Type::F64 => GoType::Float64,
        Type::Char => GoType::Rune,
        Type::String => GoType::String,
        Type::ErrorContext => todo!("TODO(#4): implement error context conversion"),

        // Complex types.
        Type::Id(id) => {
            let TypeDef { kind, .. } = resolve
                .types
                .get(*id)
                .expect("failed to find type definition");
            match kind {
                TypeDefKind::Record(_) => GoType::UserDefined(qualified_type_name(*id, resolve)),
                TypeDefKind::Resource => todo!("TODO(#5): implement resources"),
                TypeDefKind::Handle(_) => todo!("TODO(#5): implement resources"),
                TypeDefKind::Flags(_) => todo!("TODO(#4): implement flag conversion"),
                TypeDefKind::Tuple(_) => todo!("TODO(#4): implement tuple conversion"),
                TypeDefKind::Variant(_) => GoType::UserDefined(qualified_type_name(*id, resolve)),
                TypeDefKind::Enum(_) => GoType::UserDefined(qualified_type_name(*id, resolve)),
                // `option<T>` is `*T`: `nil` is `none`, `&v` is `some`. A
                // single pointer composes in every position (param, return,
                // record field, list element); the prior `(T, bool)`
                // comma-ok shape didn't.
                TypeDefKind::Option(value) => {
                    GoType::Pointer(Box::new(resolve_type(value, resolve)))
                }

                // Various results, including specialised ones.
                TypeDefKind::Result(Result_ {
                    ok: Some(ok),
                    err: Some(Type::String),
                }) => GoType::ValueOrError(Box::new(resolve_type(ok, resolve))),
                TypeDefKind::Result(Result_ {
                    ok: Some(_),
                    err: Some(_),
                }) => {
                    todo!("TODO(#4): implement remaining result conversion")
                }
                TypeDefKind::Result(Result_ {
                    ok: Some(ok),
                    err: None,
                }) => resolve_type(ok, resolve),
                TypeDefKind::Result(Result_ {
                    ok: None,
                    err: Some(Type::String),
                }) => GoType::Error,
                TypeDefKind::Result(Result_ {
                    ok: None,
                    err: Some(_),
                }) => todo!("TODO(#4): implement remaining result conversion"),
                TypeDefKind::Result(Result_ {
                    ok: None,
                    err: None,
                }) => GoType::Nothing,

                TypeDefKind::List(inner) => GoType::Slice(Box::new(resolve_type(inner, resolve))),
                TypeDefKind::Future(_) => todo!("TODO(#4): implement future conversion"),
                TypeDefKind::Stream(_) => todo!("TODO(#4): implement stream conversion"),
                TypeDefKind::Type(_) => GoType::UserDefined(qualified_type_name(*id, resolve)),
                TypeDefKind::FixedLengthList(_, _) => {
                    todo!("TODO(#4): implement fixed length list conversion")
                }
                TypeDefKind::Map(_, _) => todo!("TODO(#4): implement map conversion"),
                TypeDefKind::Unknown => todo!("TODO(#4): implement unknown conversion"),
            }
        }
    }
}

/// Like [`resolve_type`], but downgrades a top-level Variant to
/// `interface{}` so existing call sites can keep passing the variant
/// payload through `any`-typed plumbing (rule config returns, generic
/// dispatch layers). The marker interface and per-case structs are still
/// generated, and the type-switch in `VariantLower` still dispatches on
/// the concrete case types — callers who want compile-time exhaustiveness
/// just declare their value as the marker interface explicitly.
///
/// Variants nested inside records, lists, or returns stay typed so
/// generated record fields remain strongly typed.
pub fn resolve_param_type(typ: &Type, resolve: &Resolve) -> GoType {
    if let Type::Id(id) = typ {
        let def = &resolve.types[dealias(resolve, *id)];
        if matches!(def.kind, TypeDefKind::Variant(_)) {
            return GoType::Interface;
        }
    }
    resolve_type(typ, resolve)
}

#[cfg(test)]
mod tests {
    use crate::go_package_name;

    /// With no `--package` the Go package is the world's name in snake case.
    #[test]
    fn test_package_defaults_to_the_world_name() {
        assert_eq!(
            go_package_name("identity-core", None).unwrap(),
            "identity_core"
        );
        assert_eq!(go_package_name("root", None).unwrap(), "root");
    }

    /// `--package` names the Go package whatever the world is called.
    #[test]
    fn test_package_flag_overrides_the_world_name() {
        assert_eq!(
            go_package_name("identity-core", Some("identity")).unwrap(),
            "identity"
        );
        assert_eq!(go_package_name("w", Some("_x9")).unwrap(), "_x9");
    }

    /// A name that is not a Go package name is refused, naming the offender.
    #[test]
    fn test_package_flag_refuses_what_go_would_not_compile() {
        for bad in [
            "",
            "9lives",
            "has-dash",
            "a.b",
            "_",
            "func",
            "pkg name",
            "caf\u{e9}",
        ] {
            let err = go_package_name("w", Some(bad)).unwrap_err();
            assert!(err.contains(&format!("{bad:?}")), "{bad:?}: {err}");
        }
    }
}
