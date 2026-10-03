use genco::{Tokens, lang::Go, tokens::FormatInto};

#[derive(Debug, Clone, Copy)]
pub struct GoImport(&'static str, &'static str);

impl FormatInto<Go> for GoImport {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        tokens.append(genco::lang::go::import(self.0, self.1));
    }
}

pub static CONTEXT_CONTEXT: GoImport = GoImport("context", "Context");
pub static ERRORS_NEW: GoImport = GoImport("errors", "New");
pub static FMT_PRINTF: GoImport = GoImport("fmt", "Printf");
pub static WAZERO_RUNTIME: GoImport = GoImport("github.com/tetratelabs/wazero", "Runtime");
pub static WAZERO_NEW_RUNTIME: GoImport = GoImport("github.com/tetratelabs/wazero", "NewRuntime");
pub static WAZERO_NEW_MODULE_CONFIG: GoImport =
    GoImport("github.com/tetratelabs/wazero", "NewModuleConfig");
pub static WAZERO_COMPILED_MODULE: GoImport =
    GoImport("github.com/tetratelabs/wazero", "CompiledModule");
pub static WAZERO_API_MODULE: GoImport = GoImport("github.com/tetratelabs/wazero/api", "Module");
pub static WAZERO_API_MEMORY: GoImport = GoImport("github.com/tetratelabs/wazero/api", "Memory");
pub static WAZERO_API_ENCODE_U32: GoImport =
    GoImport("github.com/tetratelabs/wazero/api", "EncodeU32");
pub static WAZERO_API_DECODE_U32: GoImport =
    GoImport("github.com/tetratelabs/wazero/api", "DecodeU32");
pub static WAZERO_API_ENCODE_I32: GoImport =
    GoImport("github.com/tetratelabs/wazero/api", "EncodeI32");
pub static WAZERO_API_DECODE_I32: GoImport =
    GoImport("github.com/tetratelabs/wazero/api", "DecodeI32");
pub static WAZERO_API_ENCODE_F32: GoImport =
    GoImport("github.com/tetratelabs/wazero/api", "EncodeF32");
pub static WAZERO_API_DECODE_F32: GoImport =
    GoImport("github.com/tetratelabs/wazero/api", "DecodeF32");
pub static WAZERO_API_ENCODE_F64: GoImport =
    GoImport("github.com/tetratelabs/wazero/api", "EncodeF64");
pub static WAZERO_API_DECODE_F64: GoImport =
    GoImport("github.com/tetratelabs/wazero/api", "DecodeF64");
pub static REFLECT_VALUE_OF: GoImport = GoImport("reflect", "ValueOf");
