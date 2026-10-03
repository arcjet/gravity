use genco::{prelude::*, tokens::static_literal};

use crate::go::GoIdentifier;

/// Represents a Go type in the code generation system.
///
/// This enum covers all the basic Go types as well as special types
/// used for WebAssembly Component Model interop.
#[derive(Debug, Clone, PartialEq)]
pub enum GoType {
    /// Boolean type
    Bool,
    /// Unsigned 8-bit integer
    Uint8,
    /// Unsigned 16-bit integer
    Uint16,
    /// Unsigned 32-bit integer
    Uint32,
    /// Unsigned 64-bit integer
    Uint64,
    /// Signed 8-bit integer
    Int8,
    /// Signed 16-bit integer
    Int16,
    /// Signed 32-bit integer
    Int32,
    /// Signed 64-bit integer
    Int64,
    /// 32-bit floating point
    Float32,
    /// 64-bit floating point
    Float64,
    /// String type
    String,
    /// Error type (represents Result<None, String>)
    Error,
    /// Interface type (for variants/discriminated unions)
    Interface,
    // Pointer to another type
    // Pointer(Box<GoType>),
    /// Result type with Ok value
    ValueOrOk(Box<GoType>),
    /// Result type with Error value
    ValueOrError(Box<GoType>),
    /// Slice/array of another type
    Slice(Box<GoType>),
    /// Multi-return type (for functions returning arbitrary multiple values)
    // MultiReturn(Vec<GoType>),
    /// Pointer to another type. Used as the canonical Go representation of
    /// `option<T>` so the same lowering composes in every position (params,
    /// return values, record fields, list elements). `nil` is `none`,
    /// `&value` is `some`.
    Pointer(Box<GoType>),
    /// User-defined type (records, enums, type aliases)
    UserDefined(String),
    /// Represents no value/void
    Nothing,
}

impl GoType {
    /// Returns true if this type needs post-return cleanup (cabi_post_* function)
    ///
    /// According to the Component Model Canonical ABI specification, cleanup is needed
    /// for types that allocate memory in the guest's linear memory when being returned.
    ///
    /// Types that need cleanup:
    /// - Strings: allocate memory for the string data
    /// - Lists/Slices: allocate memory for the array data
    /// - Types containing the above (recursively)
    ///
    /// Types that DON'T need cleanup:
    /// - Primitives (bool, integers, floats): passed by value
    /// - Enums: represented as integers
    ///
    /// Limitations:
    /// - For UserDefined types (records, type aliases), we can't determine here if they
    ///   contain strings/lists without the full type definition, so we're conservative
    /// - A perfect implementation would recursively check record fields, but that would
    ///   require passing the Resolve context here
    pub fn needs_cleanup(&self) -> bool {
        match self {
            // Primitive types don't need cleanup
            GoType::Bool
            | GoType::Uint8
            | GoType::Uint16
            | GoType::Uint32
            | GoType::Uint64
            | GoType::Int8
            | GoType::Int16
            | GoType::Int32
            | GoType::Int64
            | GoType::Float32
            | GoType::Float64 => false,

            // String and slices allocate memory and need cleanup
            GoType::String | GoType::Slice(_) => true,

            // Complex types need cleanup if their inner types do
            GoType::ValueOrOk(inner) => inner.needs_cleanup(),

            // Pointer (representing option<T>) needs cleanup only when its
            // inner type does. Strings and slices behind a pointer still own
            // memory the guest allocated.
            GoType::Pointer(inner) => inner.needs_cleanup(),

            // The inner type of `Err` is always a String so it requires cleanup
            // TODO(#91): Store the error type to check both inner types.
            GoType::ValueOrError(_) => true,

            // Interfaces (variants) might need cleanup (conservative approach)
            GoType::Interface => true,

            // User-defined types (records, enums, type aliases) need cleanup if they
            // contain strings or other allocated types. Since we don't have access to
            // the type definition here, we must be conservative and assume they might.
            //
            // This means we might generate unnecessary cleanup calls for:
            // - Enums (which are just integers)
            // - Records containing only primitives
            // - Type aliases to primitives
            //
            // TODO(#92): Improve this by either:
            // 1. Passing the Resolve context to check actual type definitions
            // 2. Tracking cleanup requirements during type resolution
            // 3. Using a different representation that carries this information
            GoType::UserDefined(_) => true,

            // Error is actually Result<None, String> - strings need cleanup!
            GoType::Error => true,

            // Nothing represents no value, so no cleanup needed
            GoType::Nothing => false,
            // TODO - figure out if a pointer needs cleanup, once implemented.
            // GoType::Pointer(_) => todo!()
        }
    }
}

impl FormatInto<Go> for &GoType {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match self {
            GoType::Bool => tokens.append(static_literal("bool")),
            GoType::Uint8 => tokens.append(static_literal("uint8")),
            GoType::Uint16 => tokens.append(static_literal("uint16")),
            GoType::Uint32 => tokens.append(static_literal("uint32")),
            GoType::Uint64 => tokens.append(static_literal("uint64")),
            GoType::Int8 => tokens.append(static_literal("int8")),
            GoType::Int16 => tokens.append(static_literal("int16")),
            GoType::Int32 => tokens.append(static_literal("int32")),
            GoType::Int64 => tokens.append(static_literal("int64")),
            GoType::Float32 => tokens.append(static_literal("float32")),
            GoType::Float64 => tokens.append(static_literal("float64")),
            GoType::String => tokens.append(static_literal("string")),
            GoType::Error => tokens.append(static_literal("error")),
            GoType::Interface => tokens.append(static_literal("interface{}")),
            GoType::ValueOrOk(value_typ) => {
                value_typ.as_ref().format_into(tokens);
                tokens.append(static_literal(","));
                tokens.space();
                tokens.append(static_literal("bool"))
            }
            GoType::ValueOrError(value_typ) => {
                value_typ.as_ref().format_into(tokens);
                tokens.append(static_literal(","));
                tokens.space();
                tokens.append(static_literal("error"))
            }
            GoType::Slice(typ) => {
                tokens.append(static_literal("[]"));
                typ.as_ref().format_into(tokens);
            }
            // GoType::MultiReturn(typs) => {
            //     tokens.append(quote!($(for typ in typs join (, ) => $typ)))
            // }
            GoType::Pointer(typ) => {
                tokens.append(static_literal("*"));
                typ.as_ref().format_into(tokens);
            }
            GoType::UserDefined(name) => {
                let id = GoIdentifier::public(name);
                id.format_into(tokens)
            }
            GoType::Nothing => (),
        }
    }
}

impl FormatInto<Go> for GoType {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        (&self).format_into(tokens)
    }
}

#[cfg(test)]
mod tests {
    use genco::{prelude::*, tokens::Tokens};

    use crate::go::GoType;

    #[test]
    fn test_basic_types() {
        let cases = vec![
            (GoType::Bool, "bool"),
            (GoType::Uint8, "uint8"),
            (GoType::Uint16, "uint16"),
            (GoType::Uint32, "uint32"),
            (GoType::Uint64, "uint64"),
            (GoType::Int8, "int8"),
            (GoType::Int16, "int16"),
            (GoType::Int32, "int32"),
            (GoType::Int64, "int64"),
            (GoType::Float32, "float32"),
            (GoType::Float64, "float64"),
            (GoType::String, "string"),
            (GoType::Error, "error"),
            (GoType::Interface, "interface{}"),
            (GoType::Nothing, ""),
        ];

        for (typ, expected) in cases {
            let mut tokens = Tokens::<Go>::new();
            (&typ).format_into(&mut tokens);
            assert_eq!(
                tokens.to_string().unwrap(),
                expected,
                "Failed for type: {:?}",
                typ
            );
        }
    }

    #[test]
    fn test_value_or_ok() {
        let typ = GoType::ValueOrOk(Box::new(GoType::Uint32));
        let mut tokens = Tokens::<Go>::new();
        (&typ).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "uint32, bool");
    }

    #[test]
    fn test_value_or_error() {
        let typ = GoType::ValueOrError(Box::new(GoType::String));
        let mut tokens = Tokens::<Go>::new();
        (&typ).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "string, error");
    }

    #[test]
    fn test_slice() {
        let typ = GoType::Slice(Box::new(GoType::Int32));
        let mut tokens = Tokens::<Go>::new();
        (&typ).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "[]int32");
    }

    // #[test]
    // fn test_pointer() {
    //     let typ = GoType::Pointer(Box::new(GoType::String));
    //     let mut tokens = Tokens::<Go>::new();
    //     (&typ).format_into(&mut tokens);
    //     assert_eq!(tokens.to_string().unwrap(), "*string");
    // }

    // #[test]
    // fn test_nested_types() {
    //     // Test *[]string
    //     let typ = GoType::Pointer(Box::new(GoType::Slice(Box::new(GoType::String))));
    //     let mut tokens = Tokens::<Go>::new();
    //     (&typ).format_into(&mut tokens);
    //     assert_eq!(tokens.to_string().unwrap(), "*[]string");

    //     // Test [][]uint8
    //     let typ = GoType::Slice(Box::new(GoType::Slice(Box::new(GoType::Uint8))));
    //     let mut tokens = Tokens::<Go>::new();
    //     (&typ).format_into(&mut tokens);
    //     assert_eq!(tokens.to_string().unwrap(), "[][]uint8");
    // }
}
