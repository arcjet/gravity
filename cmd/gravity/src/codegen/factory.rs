use std::collections::BTreeMap;

use genco::prelude::*;

use crate::{
    codegen::ir::AnalyzedImports,
    go::{
        GoIdentifier, comment,
        imports::{
            CONTEXT_CONTEXT, ERRORS_NEW, SYNC_MAP, WAZERO_API_FUNCTION, WAZERO_API_MEMORY,
            WAZERO_API_MODULE, WAZERO_COMPILED_MODULE, WAZERO_NEW_MODULE_CONFIG,
            WAZERO_NEW_RUNTIME, WAZERO_RUNTIME,
        },
    },
};

/// Configuration for factory generation
pub struct FactoryConfig<'a> {
    pub analyzed_imports: &'a AnalyzedImports,
    pub import_chains: BTreeMap<String, Tokens<Go>>,
    pub wasm_var_name: &'a GoIdentifier,
}

/// Generator for factory and instance types
pub struct FactoryGenerator<'a> {
    config: FactoryConfig<'a>,
}

impl<'a> FactoryGenerator<'a> {
    /// Create a new factory generator with the given config.
    pub fn new(config: FactoryConfig<'a>) -> Self {
        Self { config }
    }

    /// Get the instance name from the analyzed imports.
    pub fn instance_name(&self) -> &GoIdentifier {
        &self.config.analyzed_imports.instance_name
    }

    /// Generate the `writeString` helper function.
    fn generate_write_string(&self, tokens: &mut Tokens<Go>) {
        // Add writeString helper function for interface string returns
        quote_in! { *tokens =>
            $(comment(&[
                "writeString will put a Go string into the Wasm memory following the Component",
                "Model calling conventions, such as allocating memory with the realloc function",
            ]))
            func writeString(
                ctx $CONTEXT_CONTEXT,
                s string,
                memory $WAZERO_API_MEMORY,
                realloc api.Function,
            ) (uint64, uint64, error) {
                if len(s) == 0 {
                    return 1, 0, nil
                }

                results, err := realloc.Call(ctx, 0, 0, 1, uint64(len(s)))
                if err != nil {
                    return 1, 0, err
                }
                ptr := results[0]
                ok := memory.Write(uint32(ptr), []byte(s))
                if !ok {
                    return 1, 0, $ERRORS_NEW("failed to write string to wasm memory")
                }
                return uint64(ptr), uint64(len(s)), nil
            }
            $['\n']
        };
    }

    /// Generate the per-module memo of `api.Module.ExportedFunction`.
    ///
    /// wazero builds a new function, with its own call engine, on every
    /// `ExportedFunction` call, and the canonical ABI asks for `cabi_realloc`
    /// once per string or list it lowers: unmemoized, an export taking 32
    /// strings allocated 427 KB per call (the export-memo example).
    ///
    /// An instance owns the memo for its own module. A host function only
    /// receives the calling module, so the factory keeps a registry from
    /// module to memo that `Instantiate` fills and `Close` empties; a module
    /// the registry does not know (one instantiated outside the factory) is
    /// answered without the memo. An `api.Function` is not safe for
    /// concurrent calls, and neither is a guest instance: each memo is only
    /// touched by the goroutine driving its module.
    fn generate_export_cache(&self, tokens: &mut Tokens<Go>) {
        quote_in! { *tokens =>
            $(comment(&[
                "exportedFunctions memoizes api.Module.ExportedFunction for one module",
                "instance. Like the instance, it is not safe for concurrent use.",
            ]))
            type exportedFunctions struct {
                module $WAZERO_API_MODULE
                byName map[string]$WAZERO_API_FUNCTION
            }
            $['\n']
            func newExportedFunctions(module $WAZERO_API_MODULE) *exportedFunctions {
                return &exportedFunctions{module: module, byName: map[string]$WAZERO_API_FUNCTION{}}
            }
            $['\n']
            func (e *exportedFunctions) get(name string) $WAZERO_API_FUNCTION {
                if f, ok := e.byName[name]; ok {
                    return f
                }
                f := e.module.ExportedFunction(name)
                e.byName[name] = f
                return f
            }
            $['\n']
            $(comment(&[
                "exportedFunctionCache finds the memo of the module a host function was",
                "called from.",
            ]))
            type exportedFunctionCache struct {
                byModule $SYNC_MAP
            }
            $['\n']
            func (c *exportedFunctionCache) lookup(module $WAZERO_API_MODULE, name string) $WAZERO_API_FUNCTION {
                if e, ok := c.byModule.Load(module); ok {
                    return e.(*exportedFunctions).get(name)
                }
                return module.ExportedFunction(name)
            }
            $['\n']
        };
    }

    /// Generate the Factory struct, constructor, and methods.
    fn generate_factory(&self, tokens: &mut Tokens<Go>) {
        let AnalyzedImports {
            factory_name,
            instance_name,
            constructor_name,
            ..
        } = &self.config.analyzed_imports;
        let wasm_var_name = self.config.wasm_var_name;
        // Build the parameter list
        let params = self.build_parameters();
        quote_in! { *tokens =>
            $['\n']
            type $factory_name struct {
                runtime $WAZERO_RUNTIME
                module  $WAZERO_COMPILED_MODULE
                exports *exportedFunctionCache
            }
            $['\n']
            func $constructor_name(
                $['\r']
                $params
                $['\r']
            ) (*$factory_name, error) {
                wazeroRuntime := $WAZERO_NEW_RUNTIME(ctx)
                gravityExportCache := &exportedFunctionCache{}

                $(for chain in self.config.import_chains.values() =>
                    $chain
                    $['\r']
                )

                $(comment(&[
                    "Compiling the module takes a LONG time, so we want to do it once and hold",
                       "onto it with the Runtime",
                ]))
                module, err := wazeroRuntime.CompileModule(ctx, $wasm_var_name)
                if err != nil {
                    return nil, err
                }
                return &$factory_name{
                    runtime: wazeroRuntime,
                    module:  module,
                    exports: gravityExportCache,
                }, nil
            }
            $['\n']
            func (f *$factory_name) Instantiate(ctx $CONTEXT_CONTEXT) (*$instance_name, error) {
                module, err := f.runtime.InstantiateModule(ctx, f.module, $WAZERO_NEW_MODULE_CONFIG())
                if err != nil {
                    return nil, err
                }
                exports := newExportedFunctions(module)
                f.exports.byModule.Store(module, exports)
                return &$instance_name{module: module, exports: exports, cache: f.exports}, nil
            }
            $['\n']
            func (f *$factory_name) Close(ctx $CONTEXT_CONTEXT) {
                f.runtime.Close(ctx)
            }
            $['\n']
        };
    }

    /// Generate the Instance struct, and methods.
    fn generate_instance(&self, tokens: &mut Tokens<Go>) {
        let instance_name = &self.config.analyzed_imports.instance_name;
        quote_in! { *tokens =>
            $(comment(&[
                "An instance is one guest module. It is not safe for concurrent use:",
                "instantiate one per goroutine.",
            ]))
            type $instance_name struct {
                module  $WAZERO_API_MODULE
                exports *exportedFunctions
                cache   *exportedFunctionCache
            }
            $['\n']
            func (i *$instance_name) Close(ctx $CONTEXT_CONTEXT) error {
                i.cache.byModule.Delete(i.module)
                if err := i.module.Close(ctx); err != nil {
                    return err
                }

                return nil
            }
            $['\n']
        };
    }

    /// Build parameter list for factory constructor
    fn build_parameters(&self) -> Tokens<Go> {
        let interfaces = &self.config.analyzed_imports.interfaces;

        quote! {
            ctx $CONTEXT_CONTEXT,
            $(for interface in interfaces.iter() join ($['\r']) =>
            $(&interface.constructor_param_name) $(&interface.go_interface_name),
            )
        }
    }
}

impl<'a> FormatInto<Go> for &FactoryGenerator<'a> {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        self.generate_export_cache(tokens);
        tokens.push();
        self.generate_factory(tokens);
        tokens.push();
        self.generate_instance(tokens);
        tokens.push();
        self.generate_write_string(tokens);
        tokens.push();
    }
}

#[cfg(test)]
mod tests {
    use genco::{lang::go::Tokens, tokens::FormatInto};

    use crate::{
        codegen::{FactoryGenerator, factory::FactoryConfig, ir::AnalyzedImports},
        go::GoIdentifier,
    };

    #[test]
    fn test_generate_write_string() {
        let analyzed_imports = &AnalyzedImports {
            interfaces: vec![],
            standalone_types: vec![],
            standalone_functions: vec![],
            factory_name: GoIdentifier::public("test-factory"),
            instance_name: GoIdentifier::public("test-instance"),
            constructor_name: GoIdentifier::public("test-constructor"),
        };
        let config = FactoryConfig {
            analyzed_imports,
            import_chains: Default::default(),
            wasm_var_name: &GoIdentifier::public("test-wasm"),
        };
        let generator = FactoryGenerator::new(config);
        let mut tokens = Tokens::new();
        generator.generate_write_string(&mut tokens);

        assert!(tokens.to_string().unwrap().contains("func writeString"));
    }

    /// The factory owns the registry of per-module memos: `Instantiate`
    /// registers the new module's memo, the instance holds it, and `Close`
    /// removes it, so a closed module is not kept alive by the registry.
    #[test]
    fn test_factory_registers_and_forgets_each_instance_memo() {
        let analyzed_imports = &AnalyzedImports {
            interfaces: vec![],
            standalone_types: vec![],
            standalone_functions: vec![],
            factory_name: GoIdentifier::public("test-factory"),
            instance_name: GoIdentifier::public("test-instance"),
            constructor_name: GoIdentifier::public("test-constructor"),
        };
        let config = FactoryConfig {
            analyzed_imports,
            import_chains: Default::default(),
            wasm_var_name: &GoIdentifier::public("test-wasm"),
        };
        let generator = FactoryGenerator::new(config);
        let mut tokens = Tokens::new();
        (&generator).format_into(&mut tokens);
        let generated = tokens.to_string().unwrap();

        for expected in [
            "type exportedFunctionCache struct",
            "gravityExportCache := &exportedFunctionCache{}",
            "exports: gravityExportCache,",
            "f.exports.byModule.Store(module, exports)",
            "i.cache.byModule.Delete(i.module)",
        ] {
            assert!(
                generated.contains(expected),
                "expected `{expected}`, got:\n{generated}"
            );
        }
    }
}
