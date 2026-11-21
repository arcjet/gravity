use std::str::Chars;

use genco::{prelude::*, tokens::ItemStr};

/// Names a local built from a WIT name could otherwise take: Go's keywords
/// and predeclared identifiers, the packages the bindings import, the fixed
/// names the generated function bodies use, and the package-level helpers
/// they call.
const RESERVED: &[&str] = &[
    // Keywords.
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
    // Predeclared identifiers.
    "any",
    "bool",
    "byte",
    "comparable",
    "complex64",
    "complex128",
    "error",
    "float32",
    "float64",
    "int",
    "int8",
    "int16",
    "int32",
    "int64",
    "rune",
    "string",
    "uint",
    "uint8",
    "uint16",
    "uint32",
    "uint64",
    "uintptr",
    "true",
    "false",
    "iota",
    "nil",
    "append",
    "cap",
    "clear",
    "close",
    "complex",
    "copy",
    "delete",
    "imag",
    "len",
    "make",
    "max",
    "min",
    "new",
    "panic",
    "print",
    "println",
    "real",
    "recover",
    // Imported packages.
    "api",
    "context",
    "errors",
    "fmt",
    "reflect",
    "sync",
    "utf8",
    "wazero",
    // Fixed names in generated function bodies.
    "base",
    "config",
    "ctx",
    "e",
    "err",
    "gravityExportCache",
    "i",
    "idx",
    "mod",
    "module",
    "postFn",
    "variantPayload",
    "wazeroRuntime",
    // Package-level helpers (`factory::HELPERS`).
    "exportedFunctionCache",
    "exportedFunctions",
    "newExportedFunctions",
    "writeString",
];

/// Represents a Go identifier with appropriate casing rules.
///
/// Go identifiers follow specific naming conventions:
/// - Public identifiers start with uppercase (exported)
/// - Private identifiers start with lowercase (unexported)
/// - Local identifiers, the parameters named for WIT names, are
///   lowerCamelCase with a trailing underscore when they would take a name
///   the generated code uses. Every local the templates number ends in a
///   digit (`ptr11`, `arg1`), so a local name that ends in one is escaped
///   too, and no template name ends in an underscore.
#[derive(Debug, Clone)]
pub enum GoIdentifier {
    /// Public/exported identifier (will be converted to UpperCamelCase)
    Public { name: String },
    /// Private/unexported identifier (will be converted to lowerCamelCase)
    Private { name: String },
    /// Local identifier (will be converted to lowerCamelCase)
    Local { name: String },
}

impl GoIdentifier {
    /// Creates a new public identifier.
    pub fn public<T>(name: T) -> Self
    where
        T: Into<String>,
    {
        Self::Public { name: name.into() }
    }

    /// Creates a new private identifier.
    pub fn private<T>(name: T) -> Self
    where
        T: Into<String>,
    {
        Self::Private { name: name.into() }
    }

    /// Creates a new local identifier.
    pub fn local<T>(name: T) -> Self
    where
        T: Into<String>,
    {
        Self::Local { name: name.into() }
    }

    /// Returns an iterator over the characters of the underlying name.
    ///
    /// This provides access to the raw name without case transformations.
    ///
    /// # Returns
    /// An iterator over the characters of the identifier's name.
    pub fn chars(&self) -> Chars<'_> {
        match self {
            GoIdentifier::Public { name } => name.chars(),
            GoIdentifier::Private { name } => name.chars(),
            GoIdentifier::Local { name } => name.chars(),
        }
    }
}

impl From<GoIdentifier> for String {
    fn from(value: GoIdentifier) -> Self {
        (&value).into()
    }
}

impl From<&GoIdentifier> for String {
    fn from(value: &GoIdentifier) -> Self {
        let mut tokens: Tokens<Go> = Tokens::new();
        value.format_into(&mut tokens);
        tokens.to_string().expect("to format correctly")
    }
}

impl FormatInto<Go> for &GoIdentifier {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        let name = match self {
            GoIdentifier::Public { name }
            | GoIdentifier::Private { name }
            | GoIdentifier::Local { name } => name,
        };

        // TODO(#12): Check for invalid first character
        let mut ident = lower_camel(name);

        match self {
            GoIdentifier::Public { .. } => {
                let mut chars = ident.chars();
                let first = chars.next().expect("No function name");
                ident = first.to_uppercase().chain(chars).collect();
            }
            GoIdentifier::Local { .. } if is_reserved_local(&ident) => ident.push('_'),
            GoIdentifier::Private { .. } | GoIdentifier::Local { .. } => (),
        }

        tokens.append(ItemStr::from(ident));
    }
}

/// A kebab-case WIT name in lowerCamelCase.
fn lower_camel(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut chars = name.chars();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '-' | '_' => {
                if let Some(c) = chars.next() {
                    out.extend(c.to_uppercase());
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Whether a local named `ident` would take a name the generated code uses.
fn is_reserved_local(ident: &str) -> bool {
    RESERVED.contains(&ident) || ident.ends_with(|c: char| c.is_ascii_digit())
}

impl FormatInto<Go> for GoIdentifier {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        (&self).format_into(tokens)
    }
}

#[cfg(test)]
mod tests {

    use genco::{prelude::*, tokens::Tokens};

    use crate::go::GoIdentifier;

    #[test]
    fn test_public_identifier() {
        let id = GoIdentifier::public("hello-world");
        let mut tokens = Tokens::<Go>::new();
        (&id).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "HelloWorld");
    }

    #[test]
    fn test_private_identifier() {
        let id = GoIdentifier::private("hello-world");
        let mut tokens = Tokens::<Go>::new();
        (&id).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "helloWorld");
    }

    #[test]
    fn test_local_identifier() {
        let id = GoIdentifier::local("hello-world");
        let mut tokens = Tokens::<Go>::new();
        (&id).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "helloWorld");
    }

    /// A local that would take a name the generated code uses gets a
    /// trailing underscore; the same name as a public or private identifier
    /// does not.
    #[test]
    fn test_reserved_local_is_escaped() {
        for (wit, want) in [
            ("type", "type_"),
            ("nil", "nil_"),
            ("len", "len_"),
            ("errors", "errors_"),
            ("ctx", "ctx_"),
            ("config", "config_"),
            ("i", "i_"),
            ("variant-payload", "variantPayload_"),
            ("write-string", "writeString_"),
            ("arg1", "arg1_"),
            ("ptr-11", "ptr11_"),
            ("value", "value"),
            ("type-name", "typeName"),
        ] {
            assert_eq!(
                String::from(GoIdentifier::local(wit)),
                want,
                "local `{wit}`"
            );
        }
        assert_eq!(String::from(GoIdentifier::private("type")), "type");
        assert_eq!(String::from(GoIdentifier::private("arg1")), "arg1");
        assert_eq!(String::from(GoIdentifier::public("type")), "Type");
    }

    #[test]
    #[should_panic(expected = "No function name")]
    fn test_empty_public_identifier() {
        let _ = String::from(GoIdentifier::public(""));
    }
}
