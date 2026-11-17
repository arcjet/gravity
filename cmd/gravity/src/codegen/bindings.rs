use std::collections::BTreeMap;

use genco::{prelude::*, tokens::Tokens};
use wit_bindgen_core::wit_parser::{Resolve, Result_, SizeAlign, Type, TypeDefKind, World};

use crate::{
    codegen::{
        ExportGenerator, FactoryGenerator,
        exports::ExportConfig,
        factory::FactoryConfig,
        imports::{ImportAnalyzer, ImportCodeGenerator},
        ir::{AnalyzedImports, TypeDefinition},
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
        self.generate_exports(&imports);
    }

    /// Generates the imports for the bindings.
    fn generate_imports(&mut self) -> (AnalyzedImports, BTreeMap<String, Tokens<Go>>) {
        let analyzer = ImportAnalyzer::new(self.resolve, self.world);
        let analyzed = analyzer.analyze();
        for (helper, declared_for) in super::factory::HELPERS {
            refuse_collision(&analyzed, &self.world.name, helper, declared_for);
        }
        refuse_collision(
            &analyzed,
            &self.world.name,
            &String::from(&self.raw_wasm_var),
            "for the embedded Wasm module",
        );
        for typ in analyzed.types() {
            if matches!(typ.definition, TypeDefinition::Enum { .. }) {
                let parse_function =
                    String::from(GoIdentifier::public(crate::enum_parse_name(&typ.name)));
                refuse_collision(
                    &analyzed,
                    &self.world.name,
                    &parse_function,
                    &format!("to parse a case name of enum `{}`", typ.name),
                );
            }
        }

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
        if typed_result_errors {
            refuse_collision(
                analyzed_imports,
                &self.world.name,
                "ResultError",
                "for the err case of a result whose err type is not a string",
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
    /// Functions the world exports become instance methods; an exported
    /// interface becomes its types and a Go type holding its functions.
    fn generate_exports(&mut self, imports: &AnalyzedImports) {
        let config = ExportConfig {
            instance: &imports.instance_name,
            world: self.world,
            resolve: self.resolve,
            sizes: self.sizes,
            analyzed_imports: imports,
        };
        ExportGenerator::new(config).format_into(&mut self.out)
    }
}

/// Panics when a WIT item's Go identifier is `go_name`, which the bindings
/// also declare `declared_for`, so the generated Go would redeclare it.
fn refuse_collision(analyzed: &AnalyzedImports, world: &str, go_name: &str, declared_for: &str) {
    if let Some((_, wit_item)) = analyzed
        .declared_identifiers(world)
        .into_iter()
        .find(|(name, _)| name == go_name)
    {
        panic!(
            "the WIT {wit_item} is named {go_name} in Go, which gravity declares \
             {declared_for}; rename the WIT item"
        );
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
    #[should_panic(expected = "the WIT record `result-error` is named ResultError in Go")]
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
    #[should_panic(expected = "the WIT enum `result`'s case `error` is named ResultError in Go")]
    fn test_result_error_enum_case_collision_is_refused() {
        generate(&Fixture::parse(
            "package test:fixture;
            world test-world {
                enum %result { ok, error }
                export f: func(r: %result) -> result<u32, %result>;
            }",
        ));
    }

    #[test]
    #[should_panic(expected = "the WIT record `result-error` is named ResultError in Go")]
    fn test_result_error_exported_interface_type_collision_is_refused() {
        generate(&Fixture::parse(
            "package test:fixture;
            interface api {
                record result-error { x: u32 }
                enum refusal { a, b }
                get: func() -> result<result-error, refusal>;
            }
            world test-world {
                export api;
            }",
        ));
    }

    #[test]
    #[should_panic(
        expected = "the WIT record `parse-color` is named ParseColor in Go, which gravity \
                    declares to parse a case name of enum `color`"
    )]
    fn test_parse_function_type_name_collision_is_refused() {
        generate(&Fixture::parse(
            "package test:fixture;
            world test-world {
                enum color { red }
                record parse-color { x: u32 }
                export f: func(c: color) -> parse-color;
            }",
        ));
    }

    #[test]
    #[should_panic(
        expected = "the WIT enum `parse`'s case `color` is named ParseColor in Go, which gravity \
                    declares to parse a case name of enum `color`"
    )]
    fn test_parse_function_enum_case_collision_is_refused() {
        generate(&Fixture::parse(
            "package test:fixture;
            world test-world {
                enum color { red }
                enum parse { color }
                export f: func(c: color) -> parse;
            }",
        ));
    }

    /// The factory is named for the world, so a world `parse-x` and an enum
    /// `x-factory` would both declare ParseXFactory.
    #[test]
    #[should_panic(
        expected = "the WIT world `parse-x`'s factory is named ParseXFactory in Go, which gravity \
                    declares to parse a case name of enum `x-factory`"
    )]
    fn test_parse_function_factory_collision_is_refused() {
        generate(&Fixture::parse(
            "package test:fixture;
            world parse-x {
                enum x-factory { a }
                export f: func(c: x-factory) -> u32;
            }",
        ));
    }

    /// An exported interface's enums get a Parse function too, so its types
    /// are checked against those names like the world's own.
    #[test]
    #[should_panic(
        expected = "the WIT record `parse-color` is named ParseColor in Go, which gravity \
                    declares to parse a case name of enum `color`"
    )]
    fn test_parse_function_exported_interface_collision_is_refused() {
        generate(&Fixture::parse(
            "package test:fixture;
            interface api {
                enum color { red }
                record parse-color { x: u32 }
                paint: func(c: color) -> parse-color;
            }
            world test-world {
                export api;
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

    /// Generates the bindings for a world `test-world` with one enum named
    /// `name`.
    fn generate_with_enum(name: &str) -> String {
        generate(&Fixture::parse(&format!(
            "package test:fixture;
            world test-world {{
                enum {name} {{ a, b }}
                export f: func(x: {name}) -> u32;
            }}"
        )))
    }

    /// An enum declares an unexported Go type named for it, which must not
    /// take the name of a helper every set of bindings declares.
    #[test]
    #[should_panic(
        expected = "the WIT enum `write-string` is named writeString in Go, which gravity declares"
    )]
    fn test_enum_named_like_write_string_is_refused() {
        generate_with_enum("write-string");
    }

    #[test]
    #[should_panic(
        expected = "the WIT enum `exported-functions` is named exportedFunctions in Go, which gravity declares"
    )]
    fn test_enum_named_like_exported_functions_is_refused() {
        generate_with_enum("exported-functions");
    }

    #[test]
    #[should_panic(
        expected = "the WIT enum `new-exported-functions` is named newExportedFunctions in Go, which gravity declares"
    )]
    fn test_enum_named_like_new_exported_functions_is_refused() {
        generate_with_enum("new-exported-functions");
    }

    #[test]
    #[should_panic(
        expected = "the WIT enum `exported-function-cache` is named exportedFunctionCache in Go, which gravity declares"
    )]
    fn test_enum_named_like_exported_function_cache_is_refused() {
        generate_with_enum("exported-function-cache");
    }

    #[test]
    #[should_panic(
        expected = "the WIT enum `wasm-file-test-world` is named wasmFileTestWorld in Go, which gravity declares"
    )]
    fn test_enum_named_like_the_embedded_wasm_is_refused() {
        generate_with_enum("wasm-file-test-world");
    }

    /// Every helper gravity reserves is one the bindings declare, so the
    /// reserved names follow the templates that declare them.
    #[test]
    fn test_reserved_helpers_are_declared() {
        let generated = generate_with_enum("ordinary");
        for (name, _) in crate::codegen::factory::HELPERS {
            assert!(
                generated.contains(&format!("func {name}("))
                    || generated.contains(&format!("type {name} struct")),
                "`{name}` is reserved but not declared, got:\n{generated}"
            );
        }
        assert!(
            generated.contains("CompileModule(ctx, wasmFileTestWorld)"),
            "got:\n{generated}"
        );
    }
}
