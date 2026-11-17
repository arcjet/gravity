use std::collections::BTreeMap;

use genco::{prelude::*, tokens::Tokens};
use wit_bindgen_core::wit_parser::{Resolve, Result_, SizeAlign, Type, TypeDefKind, World};

use crate::{
    codegen::{
        ExportGenerator, FactoryGenerator,
        exports::ExportConfig,
        factory::FactoryConfig,
        imports::{ImportAnalyzer, ImportCodeGenerator},
        ir::AnalyzedImports,
        wasm::{Wasm, WasmData},
    },
    go::GoIdentifier,
};

/// The WIT bindings for a world.
pub struct Bindings<'a> {
    resolve: &'a Resolve,
    world: &'a World,
    /// The cumulative output tokens containing the Go bindings.
    // TODO(#16): Don't use the internal bindings.out field
    pub out: Tokens<Go>,

    /// The identifier of the Go variable containing the WebAssembly bytes.
    raw_wasm_var: GoIdentifier,

    /// The sizes of the architecture.
    sizes: &'a SizeAlign,
}

impl<'a> Bindings<'a> {
    /// Creates a new bindings generator for the selected world.
    pub fn new(resolve: &'a Resolve, world: &'a World, sizes: &'a SizeAlign) -> Self {
        let world_name = &world.name;
        let wasm_var = GoIdentifier::private(format!("wasm-file-{world_name}"));
        Self {
            resolve,
            world,
            out: Tokens::new(),
            raw_wasm_var: wasm_var,
            sizes,
        }
    }

    /// Adds the given Wasm to the bindings.
    pub fn include_wasm(&mut self, wasm: WasmData) {
        Wasm::new(&self.raw_wasm_var, wasm).format_into(&mut self.out)
    }

    /// Generate the bindings.
    ///
    /// This generates the imports (interfaces, types, functions), the factory and instance
    /// type, and the exports (functions).
    pub fn generate(&mut self) {
        let (imports, chains) = self.generate_imports();
        self.generate_factory(&imports, chains);
        self.generate_exports(&imports.instance_name);
    }

    /// Generates the imports for the bindings.
    fn generate_imports(&mut self) -> (AnalyzedImports, BTreeMap<String, Tokens<Go>>) {
        let analyzer = ImportAnalyzer::new(self.resolve, self.world);
        let analyzed = analyzer.analyze();

        let generator = ImportCodeGenerator::new(self.resolve, &analyzed, self.sizes);
        let import_chains = generator.import_chains();
        generator.format_into(&mut self.out);
        (analyzed, import_chains)
    }

    /// Generates the factory and instantiate functions, including any
    /// required interfaces.
    fn generate_factory(
        &mut self,
        analyzed_imports: &AnalyzedImports,
        import_chains: BTreeMap<String, Tokens<Go>>,
    ) {
        let typed_result_errors = self.resolve.types.iter().any(|(_, def)| {
            matches!(
                def.kind,
                TypeDefKind::Result(Result_ {
                    err: Some(err),
                    ..
                }) if err != Type::String
            )
        });
        if typed_result_errors
            && let Some((_, wit_item)) = analyzed_imports
                .type_identifiers()
                .into_iter()
                .find(|(go_name, _)| go_name == "ResultError")
        {
            panic!(
                "the WIT {wit_item} is named ResultError in Go, which gravity declares for the \
                 err case of a result whose err type is not a string; rename the WIT item"
            );
        }
        let config = FactoryConfig {
            analyzed_imports,
            import_chains,
            wasm_var_name: &self.raw_wasm_var,
            typed_result_errors,
        };
        FactoryGenerator::new(config).format_into(&mut self.out)
    }

    /// Generates all exports for the world.
    ///
    /// Note: for now this only generates functions; types and interfaces are
    /// still TODO
    fn generate_exports(&mut self, instance: &GoIdentifier) {
        let config = ExportConfig {
            instance,
            world: self.world,
            resolve: self.resolve,
            sizes: self.sizes,
        };
        ExportGenerator::new(config).format_into(&mut self.out)
    }
}

#[cfg(test)]
mod tests {
    use crate::codegen::test_wit::Fixture;

    use super::Bindings;

    /// Generates the bindings for `fixture`'s world.
    fn generate(fixture: &Fixture) -> String {
        let mut bindings = Bindings::new(&fixture.resolve, fixture.world(), &fixture.sizes);
        bindings.generate();
        bindings.out.to_string().unwrap()
    }

    #[test]
    #[should_panic(expected = "the WIT type `result-error` is named ResultError in Go")]
    fn test_result_error_type_name_collision_is_refused() {
        generate(&Fixture::parse(
            "package test:fixture;
            world test-world {
                record result-error { x: u32 }
                enum refusal { a, b }
                export f: func(r: result-error) -> result<u32, refusal>;
            }",
        ));
    }

    #[test]
    #[should_panic(expected = "the WIT case `error` of enum `result` is named ResultError in Go")]
    fn test_result_error_enum_case_collision_is_refused() {
        generate(&Fixture::parse(
            "package test:fixture;
            world test-world {
                enum %result { ok, error }
                export f: func(r: %result) -> result<u32, %result>;
            }",
        ));
    }

    /// A WIT type may be named `result-error` when no function needs
    /// `ResultError`.
    #[test]
    fn test_result_error_type_name_without_typed_result() {
        let generated = generate(&Fixture::parse(
            "package test:fixture;
            world test-world {
                record result-error { x: u32 }
                export f: func(r: result-error) -> result<u32, string>;
            }",
        ));
        assert!(
            generated.contains("type ResultError struct"),
            "got:\n{generated}"
        );
        assert!(
            !generated.contains("type ResultError[E any] struct"),
            "got:\n{generated}"
        );
    }
}
