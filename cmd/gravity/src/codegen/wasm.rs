use genco::prelude::*;

use crate::go::{GoIdentifier, embed};

/// The WebAssembly data for a world, either inline or embedded using go:embed.
pub enum WasmData<'a> {
    /// The WebAssembly file is inlined as a byte array.
    Inline(&'a [u8]),
    /// The WebAssembly file is embedded using go:embed.
    Embedded(&'a str),
}

pub(crate) struct Wasm<'a> {
    var: &'a GoIdentifier,
    data: WasmData<'a>,
}

impl<'a> Wasm<'a> {
    pub(crate) fn new(var: &'a GoIdentifier, data: WasmData<'a>) -> Self {
        Self { var, data }
    }
}

impl FormatInto<Go> for Wasm<'_> {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match self.data {
            WasmData::Inline(bytes) => {
                let hex_rows = bytes
                    .chunks(16)
                    .map(|bytes| {
                        quote! {
                            $(for b in bytes join ( ) => $(format!("0x{b:02x},")))
                        }
                    })
                    .collect::<Vec<Tokens<Go>>>();

                // TODO(#16): Don't use the internal bindings.out field
                quote_in! { *tokens =>
                    var $(self.var) = []byte{
                        $(for row in hex_rows join ($['\r']) => $row)
                    }
                };
            }
            WasmData::Embedded(name) => {
                // TODO(#16): Don't use the internal bindings.out field
                quote_in! { *tokens =>
                    import _ "embed"

                    $(embed(name))
                    var $(self.var) []byte
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use genco::{prelude::*, tokens::Tokens};

    use crate::{
        codegen::wasm::{Wasm, WasmData},
        go::GoIdentifier,
    };

    #[test]
    fn test_inline_wasm() {
        let var = GoIdentifier::private("wasm");
        let wasm = WasmData::Inline(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let mut tokens = Tokens::<Go>::new();
        Wasm::new(&var, wasm).format_into(&mut tokens);
        assert_eq!(
            tokens.to_string().unwrap(),
            r#"var wasm = []byte{
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
}"#
        );
    }

    #[test]
    fn test_embedded_wasm() {
        let var = GoIdentifier::private("wasm");
        let wasm = WasmData::Embedded("hello.wasm");
        let mut tokens = Tokens::<Go>::new();
        Wasm::new(&var, wasm).format_into(&mut tokens);
        assert_eq!(
            tokens.to_string().unwrap(),
            "import _ \"embed\"\n\n//go:embed hello.wasm\nvar wasm []byte"
        );
    }
}
