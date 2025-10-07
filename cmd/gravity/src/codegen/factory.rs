use std::collections::BTreeMap;

use genco::prelude::*;

use crate::{
    codegen::ir::AnalyzedImports,
    go::{GoIdentifier, GoImports, comment},
};

/// Configuration for factory generation
pub struct FactoryConfig<'a> {
    pub analyzed_imports: &'a AnalyzedImports,
    pub go_imports: &'a GoImports,
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
        let go_imports = self.config.go_imports;

        // Add writeString helper function for interface string returns
        quote_in! { *tokens =>
            $(comment(&[
                "writeString will put a Go string into the Wasm memory following the Component",
                "Model calling convetions, such as allocating memory with the realloc function",
            ]))
            func writeString(
                ctx $(&go_imports.context),
                s string,
                memory $(&go_imports.wazero_api_memory),
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
                    return 1, 0, err
                }
                return uint64(ptr), uint64(len(s)), nil
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
        let go_imports = self.config.go_imports;
        let wasm_var_name = self.config.wasm_var_name;
        // Build the parameter list
        let params = self.build_parameters();
        quote_in! { *tokens =>
            $['\n']
            type $factory_name struct {
                runtime $(&go_imports.wazero_runtime)
                module  $(&go_imports.wazero_compiled_module)
            }
            $['\n']
            func $constructor_name(
                $['\r']
                $params
                $['\r']
            ) (*$factory_name, error) {
                wazeroRuntime := $(&go_imports.wazero_new_runtime)(ctx)

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
                }, nil
            }
            $['\n']
            func (f *$factory_name) Instantiate(ctx $(&go_imports.context)) (*$instance_name, error) {
                if module, err := f.runtime.InstantiateModule(ctx, f.module, $(&go_imports.wazero_new_module_config)()); err != nil {
                    return nil, err
                } else {
                    return &$instance_name{module}, nil
                }
            }
            $['\n']
            func (f *$factory_name) Close(ctx $(&go_imports.context)) {
                f.runtime.Close(ctx)
            }
            $['\n']
        };
    }

    /// Generate the Instance struct, and methods.
    fn generate_instance(&self, tokens: &mut Tokens<Go>) {
        let go_imports = self.config.go_imports;
        let instance_name = &self.config.analyzed_imports.instance_name;
        quote_in! { *tokens =>
            type $instance_name struct {
                module $(&go_imports.wazero_api_module)
            }
            $['\n']
            func (i *$instance_name) Close(ctx $(&go_imports.context)) error {
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
        let go_imports = self.config.go_imports;
        let interfaces = &self.config.analyzed_imports.interfaces;

        quote! {
            ctx $(&go_imports.context),
            $(for interface in interfaces.iter() =>
            $(&interface.constructor_param_name) $(&interface.go_interface_name),)
        }
    }
}

impl<'a> FormatInto<Go> for &FactoryGenerator<'a> {
    fn format_into(self, tokens: &mut Tokens<Go>) {
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
    use genco::lang::go::Tokens;

    use crate::{
        codegen::{FactoryGenerator, factory::FactoryConfig, ir::AnalyzedImports},
        go::{GoIdentifier, GoImports},
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
        let go_imports = &GoImports::new();
        let config = FactoryConfig {
            analyzed_imports,
            import_chains: Default::default(),
            go_imports,
            wasm_var_name: &GoIdentifier::public("test-wasm"),
        };
        let generator = FactoryGenerator::new(config);
        let mut tokens = Tokens::new();
        generator.generate_write_string(&mut tokens);

        assert!(tokens.to_string().unwrap().contains("func writeString"));
    }
}
