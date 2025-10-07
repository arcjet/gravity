use std::str::Chars;

use genco::{prelude::*, tokens::ItemStr};

/// Represents a Go identifier with appropriate casing rules.
///
/// Go identifiers follow specific naming conventions:
/// - Public identifiers start with uppercase (exported)
/// - Private identifiers start with lowercase (unexported)
/// - Local identifiers are used as-is without transformation
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
        let mut chars = self.chars();

        // TODO(#12): Check for invalid first character

        if let GoIdentifier::Public { .. } = self {
            // https://stackoverflow.com/a/38406885
            match chars.next() {
                Some(c) => tokens.append(ItemStr::from(c.to_uppercase().to_string())),
                None => panic!("No function name"),
            };
        };

        while let Some(c) = chars.next() {
            match c {
                ' ' | '-' | '_' => {
                    if let Some(c) = chars.next() {
                        tokens.append(ItemStr::from(c.to_uppercase().to_string()));
                    }
                }
                _ => tokens.append(ItemStr::from(c.to_string())),
            }
        }
    }
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
}
