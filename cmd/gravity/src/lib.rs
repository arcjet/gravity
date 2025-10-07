pub mod codegen;
pub mod go;

use crate::go::GoType;
use wit_bindgen_core::{
    abi::WasmType,
    wit_parser::{Resolve, Result_, Type, TypeDef, TypeDefKind},
};

// Temporary re-export while we migrate.
pub use codegen::Func;

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
        Type::Char => {
            // Is this a Go "rune"?
            todo!("TODO(#6): resolve char type")
        }
        Type::String => GoType::String,
        Type::ErrorContext => todo!("TODO(#4): implement error context conversion"),

        // Complex types.
        Type::Id(id) => {
            let TypeDef { name, kind, .. } = resolve
                .types
                .get(*id)
                .expect("failed to find type definition");
            match kind {
                TypeDefKind::Record(_) => {
                    GoType::UserDefined(name.clone().expect("expected record to have a name"))
                }
                TypeDefKind::Resource => todo!("TODO(#5): implement resources"),
                TypeDefKind::Handle(_) => todo!("TODO(#5): implement resources"),
                TypeDefKind::Flags(_) => todo!("TODO(#4): implement flag conversion"),
                TypeDefKind::Tuple(_) => todo!("TODO(#4): implement tuple conversion"),
                // Variants are handled as an empty interfaces in type signatures; however, that
                // means they require runtime type reflection
                TypeDefKind::Variant(_) => GoType::Interface,
                TypeDefKind::Enum(_) => {
                    GoType::UserDefined(name.clone().expect("expected enum to have a name"))
                }
                TypeDefKind::Option(value) => {
                    GoType::ValueOrOk(Box::new(resolve_type(value, resolve)))
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
                TypeDefKind::Type(_) => {
                    GoType::UserDefined(name.clone().expect("expected type alias to have a name"))
                }
                TypeDefKind::FixedSizeList(_, _) => {
                    todo!("TODO(#4): implement fixed size list conversion")
                }
                TypeDefKind::Unknown => todo!("TODO(#4): implement unknown conversion"),
            }
        }
    }
}
