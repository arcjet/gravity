use std::collections::BTreeMap;

use genco::prelude::*;

use crate::{
    codegen::ir::AnalyzedImports,
    go::{
        GoIdentifier, comment,
        imports::{
            CONTEXT_CONTEXT, ERRORS_NEW, FMT_SPRINTF, SYNC_MAP, UTF8_VALID_STRING,
            WAZERO_API_FUNCTION, WAZERO_API_MEMORY, WAZERO_API_MODULE, WAZERO_COMPILED_MODULE,
            WAZERO_NEW_MODULE_CONFIG, WAZERO_NEW_RUNTIME_WITH_CONFIG, WAZERO_RUNTIME,
            WAZERO_RUNTIME_CONFIG,
        },
    },
};

/// The package-level helpers every set of bindings declares, each with what
/// it is for, so no WIT item may take one of their names.
pub(crate) const HELPERS: [(&str, &str); 4] = [
    ("writeString", "to write a string into guest memory"),
    ("exportedFunctions", "for an instance's export memo"),
    (
        "newExportedFunctions",
        "to create an instance's export memo",
    ),
    (
        "exportedFunctionCache",
        "to find the export memo of a calling module",
    ),
];

/// Configuration for factory generation
pub struct FactoryConfig<'a> {
    pub analyzed_imports: &'a AnalyzedImports,
    pub import_chains: BTreeMap<String, Tokens<Go>>,
    pub wasm_var_name: &'a GoIdentifier,
    /// Whether any function returns a `result` whose err type is not a
    /// string, so the bindings need `ResultError`.
    pub typed_result_errors: bool,
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
                "Model calling conventions, such as allocating memory with the realloc function.",
                "The Component Model requires a string to be valid UTF-8, and a Go string can",
                "hold any bytes, so writeString refuses one that is not.",
            ]))
            func writeString(
                ctx $CONTEXT_CONTEXT,
                s string,
                memory $WAZERO_API_MEMORY,
                realloc api.Function,
            ) (uint64, uint64, error) {
                if !$UTF8_VALID_STRING(s) {
                    return 1, 0, $ERRORS_NEW("invalid UTF-8 in string passed to the guest")
                }
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
    /// module to memo that `Instantiate` fills and `Close` empties. The
    /// runtime is private to the factory, so every calling module comes from
    /// `Instantiate`; the registry lacks it only while `InstantiateModule` is
    /// still running, and a host call made then, such as from a start
    /// function, is answered without the memo. An `api.Function` is not safe
    /// for concurrent calls, and neither is a guest instance: each memo is
    /// only touched by the goroutine driving its module.
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

    /// Generate `ResultError`, the error a function returns for the err case
    /// of a `result<T, E>` whose E is not a string.
    fn generate_result_error(&self, tokens: &mut Tokens<Go>) {
        quote_in! { *tokens =>
            $(comment(&[
                "ResultError is the error a function returns for the err case of a WIT",
                "result<T, E> whose E is not a string; Value is the E. Recover it with",
                "errors.As. Any other error is a trap or a host failure, never an E.",
            ]))
            type ResultError[E any] struct {
                Value E
            }
            $['\n']
            func (e *ResultError[E]) Error() string {
                return $FMT_SPRINTF("%v", e.Value)
            }
            $['\n']
        };
    }

    /// Generate the Factory struct, constructor, and methods.
    ///
    /// The constructor takes the wazero runtime configuration, so each caller
    /// chooses whether a done context stops a guest call and how much memory
    /// a guest may have. Closing a module on a done context cost about 340 ns
    /// and 180 bytes on every call into the guest (wazero 1.12 on arm64, the
    /// export-memo example), and the canonical ABI calls the guest once per
    /// string or list it lowers, so it is the caller's choice, not a default.
    fn generate_factory(&self, tokens: &mut Tokens<Go>) {
        let AnalyzedImports {
            factory_name,
            instance_name,
            constructor_name,
            ..
        } = &self.config.analyzed_imports;
        let wasm_var_name = self.config.wasm_var_name;
        let hosts = self.host_parameters();
        let has_host_interfaces = !self.config.analyzed_imports.interfaces.is_empty();
        let constructor = String::from(constructor_name);
        let mut constructor_doc = vec![
            format!("{constructor} returns a factory for instances of the guest, whose wazero"),
            "runtime is configured by config; wazero.NewRuntimeConfig() is wazero's default."
                .into(),
            "config.WithCloseOnContextDone(true) stops a guest call when its context is done,"
                .into(),
            "at a cost on every call into the guest, and closes the instance the call was".into(),
            "made on, which must not be used again. config.WithMemoryLimitPages bounds the".into(),
            "memory of each instance.".into(),
        ];
        if has_host_interfaces {
            constructor_doc.extend([
                "Every instance the factory creates shares the host-interface implementations"
                    .into(),
                "passed here, so they must be safe for concurrent use when instances run on".into(),
                "separate goroutines.".into(),
            ]);
        }
        quote_in! { *tokens =>
            $['\n']
            type $factory_name struct {
                runtime $WAZERO_RUNTIME
                module  $WAZERO_COMPILED_MODULE
                exports *exportedFunctionCache
            }
            $['\n']
            $(comment(&constructor_doc))
            func $constructor_name(
                $['\r']
                ctx $CONTEXT_CONTEXT,
                config $WAZERO_RUNTIME_CONFIG,
                $(for (name, typ) in &hosts join ($['\r']) => $(*name) $(*typ),)
                $['\r']
            ) (*$factory_name, error) {
                wazeroRuntime := $WAZERO_NEW_RUNTIME_WITH_CONFIG(ctx, config)
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
                $(comment(&[
                    "An anonymous module: wazero refuses a second module of the same name",
                    "in one runtime, and every instance of this factory is the same module.",
                ]))
                module, err := f.runtime.InstantiateModule(ctx, f.module, $WAZERO_NEW_MODULE_CONFIG().WithName(""))
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
                "instantiate one per goroutine. A host implementation must not call back",
                "into the instance that called it: the instance reuses one api.Function per",
                "export, and wazero's Function.Call must not be called again until the",
                "previous Call returns. An instance whose call was stopped by a done context",
                "is closed, and must not be used again.",
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

    /// The factory constructor's parameters after `ctx` and `config`: one
    /// per imported interface the host implements, as its name and Go type.
    fn host_parameters(&self) -> Vec<(&GoIdentifier, &GoIdentifier)> {
        self.config
            .analyzed_imports
            .interfaces
            .iter()
            .filter(|i| i.needs_host())
            .map(|i| (&i.constructor_param_name, &i.go_interface_name))
            .collect()
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
        if self.config.typed_result_errors {
            self.generate_result_error(tokens);
            tokens.push();
        }
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
            exported_interfaces: vec![],
        };
        let config = FactoryConfig {
            analyzed_imports,
            import_chains: Default::default(),
            wasm_var_name: &GoIdentifier::public("test-wasm"),
            typed_result_errors: false,
        };
        let generator = FactoryGenerator::new(config);
        let mut tokens = Tokens::new();
        generator.generate_write_string(&mut tokens);

        assert!(tokens.to_string().unwrap().contains("func writeString"));
    }

    /// A WIT parameter named for a helper is escaped, so it cannot shadow the
    /// helper a function body calls.
    #[test]
    fn test_helpers_are_reserved_locals() {
        for (helper, _) in super::HELPERS {
            assert_eq!(
                String::from(GoIdentifier::local(helper)),
                format!("{helper}_"),
                "a local named `{helper}` is not escaped"
            );
        }
    }

    /// Each instance is anonymous, so one factory can hold many at once.
    #[test]
    fn test_instances_are_anonymous() {
        let analyzed_imports = &AnalyzedImports {
            interfaces: vec![],
            standalone_types: vec![],
            standalone_functions: vec![],
            factory_name: GoIdentifier::public("test-factory"),
            instance_name: GoIdentifier::public("test-instance"),
            constructor_name: GoIdentifier::public("test-constructor"),
            exported_interfaces: vec![],
        };
        let config = FactoryConfig {
            analyzed_imports,
            import_chains: Default::default(),
            wasm_var_name: &GoIdentifier::public("test-wasm"),
            typed_result_errors: false,
        };
        let generator = FactoryGenerator::new(config);
        let mut tokens = Tokens::new();
        generator.generate_factory(&mut tokens);
        let generated = tokens.to_string().unwrap();
        assert!(
            generated.contains("wazero.NewModuleConfig().WithName(\"\")"),
            "got:\n{generated}"
        );
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
            exported_interfaces: vec![],
        };
        let config = FactoryConfig {
            analyzed_imports,
            import_chains: Default::default(),
            wasm_var_name: &GoIdentifier::public("test-wasm"),
            typed_result_errors: false,
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

    /// `ResultError` is emitted exactly when a function needs it.
    #[test]
    fn test_result_error_only_when_needed() {
        let analyzed_imports = &AnalyzedImports {
            interfaces: vec![],
            standalone_types: vec![],
            standalone_functions: vec![],
            factory_name: GoIdentifier::public("test-factory"),
            instance_name: GoIdentifier::public("test-instance"),
            constructor_name: GoIdentifier::public("test-constructor"),
            exported_interfaces: vec![],
        };
        for typed_result_errors in [false, true] {
            let config = FactoryConfig {
                analyzed_imports,
                import_chains: Default::default(),
                wasm_var_name: &GoIdentifier::public("test-wasm"),
                typed_result_errors,
            };
            let generator = FactoryGenerator::new(config);
            let mut tokens = Tokens::new();
            (&generator).format_into(&mut tokens);
            let generated = tokens.to_string().unwrap();
            assert_eq!(
                generated.contains("type ResultError[E any] struct"),
                typed_result_errors,
                "got:\n{generated}"
            );
        }
    }
}
