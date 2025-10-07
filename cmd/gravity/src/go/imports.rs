use genco::lang::go::Import;

/// Struct to hold Go import references
pub struct GoImports {
    pub context: Import,
    pub errors: Import,
    pub fmt: Import,
    pub wazero_runtime: Import,
    pub wazero_new_runtime: Import,
    pub wazero_new_module_config: Import,
    pub wazero_compiled_module: Import,
    pub wazero_api_module: Import,
    pub wazero_api_memory: Import,
    pub wazero_api_decode_i32: Import,
    pub wazero_api_encode_i32: Import,
    pub wazero_api_decode_u32: Import,
    pub wazero_api_encode_u32: Import,
}

impl Default for GoImports {
    fn default() -> Self {
        Self::new()
    }
}

impl GoImports {
    pub fn new() -> Self {
        Self {
            context: genco::lang::go::import("context", "Context"),
            errors: genco::lang::go::import("errors", "New"),
            fmt: genco::lang::go::import("fmt", "Printf"),
            wazero_runtime: genco::lang::go::import("github.com/tetratelabs/wazero", "Runtime"),
            wazero_new_runtime: genco::lang::go::import(
                "github.com/tetratelabs/wazero",
                "NewRuntime",
            ),
            wazero_new_module_config: genco::lang::go::import(
                "github.com/tetratelabs/wazero",
                "NewModuleConfig",
            ),
            wazero_compiled_module: genco::lang::go::import(
                "github.com/tetratelabs/wazero",
                "CompiledModule",
            ),
            wazero_api_module: genco::lang::go::import(
                "github.com/tetratelabs/wazero/api",
                "Module",
            ),
            wazero_api_memory: genco::lang::go::import(
                "github.com/tetratelabs/wazero/api",
                "Memory",
            ),
            wazero_api_decode_i32: genco::lang::go::import(
                "github.com/tetratelabs/wazero/api",
                "DecodeI32",
            ),
            wazero_api_encode_i32: genco::lang::go::import(
                "github.com/tetratelabs/wazero/api",
                "EncodeI32",
            ),
            wazero_api_decode_u32: genco::lang::go::import(
                "github.com/tetratelabs/wazero/api",
                "DecodeU32",
            ),
            wazero_api_encode_u32: genco::lang::go::import(
                "github.com/tetratelabs/wazero/api",
                "EncodeU32",
            ),
        }
    }
}
