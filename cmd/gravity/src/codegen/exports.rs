use genco::prelude::*;
use wit_bindgen_core::wit_parser::{
    Function, InterfaceId, LiftLowerAbi, ManglingAndAbi, Param, Resolve, SizeAlign, WasmExport,
    WasmExportKind, World, WorldItem, WorldKey,
};

use crate::{
    codegen::{
        imports::{ImportAnalyzer, ImportCodeGenerator},
        ir::AnalyzedImports,
    },
    go::{GoIdentifier, GoResult, GoType, comment, imports::CONTEXT_CONTEXT},
};

pub struct ExportConfig<'a> {
    pub instance: &'a GoIdentifier,
    pub world: &'a World,
    pub resolve: &'a Resolve,
    pub sizes: &'a SizeAlign,
    /// The analyzed imports, whose type generation exported interfaces share.
    pub analyzed_imports: &'a AnalyzedImports,
}

pub struct ExportGenerator<'a> {
    config: ExportConfig<'a>,
}

impl<'a> ExportGenerator<'a> {
    pub fn new(config: ExportConfig<'a>) -> Self {
        Self { config }
    }

    /// Generate the Go function code for the given function.
    ///
    /// The signature is obtained by:
    /// - getting the function parameters from the `wit_parser::Function`, converting
    ///   names to to Go identifiers and types to Go types.
    /// - similar for the result
    ///
    /// To implement the body, we:
    /// - creating a `Func` struct which implements `Bindgen` and passing it to the
    ///   `wit_bindgen_core::abi::call` function. This will call `Func::emit` lots of
    ///   times, one for each instruction in the function, and `Func::emit` will generate
    ///   Go code for each instruction
    ///
    /// The method's receiver is `receiver`, and it calls the core wasm export
    /// `wasm_name`.
    fn generate_function(
        &self,
        func: &Function,
        receiver: &GoIdentifier,
        wasm_name: String,
        tokens: &mut Tokens<Go>,
    ) {
        let params = func
            .params
            .iter()
            .map(|Param { name, ty, .. }| {
                match crate::resolve_param_type(ty, self.config.resolve) {
                    GoType::ValueOrOk(t) => (GoIdentifier::local(name), *t),
                    t => (GoIdentifier::local(name), t),
                }
            })
            .collect::<Vec<_>>();

        let result = if let Some(wit_type) = &func.result {
            GoResult::Anon(crate::resolve_type(wit_type, self.config.resolve))
        } else {
            GoResult::Empty
        };

        let mut f = crate::Func::export(result, self.config.sizes).with_wasm_name(wasm_name);
        wit_bindgen_core::abi::call(
            self.config.resolve,
            wit_bindgen_core::abi::AbiVariant::GuestExport,
            wit_bindgen_core::abi::LiftLower::LowerArgsLiftResults,
            func,
            &mut f,
            // async is not currently supported
            false,
        );

        let arg_assignments = f
            .args()
            .iter()
            .zip(&params)
            .map(|(arg, (param, _))| (arg, param))
            .collect::<Vec<_>>();
        let fn_name = &GoIdentifier::public(&func.name);
        quote_in! { *tokens =>
            $['\n']
            func (i *$receiver) $fn_name(
                $['\r']
                ctx $CONTEXT_CONTEXT,
                $(for (name, typ) in &params join ($['\r']) => $name $typ,)
            ) $(f.result()) {
                $(for (arg, param) in arg_assignments join ($['\r']) => $arg := $param)
                $(f.body())
            }
        }
    }

    /// The core wasm export name of `func`, exported from the world (`None`)
    /// or from the interface the world exports as `key`.
    fn wasm_name(&self, key: Option<&WorldKey>, func: &Function) -> String {
        self.config.resolve.wasm_export_name(
            ManglingAndAbi::Legacy(LiftLowerAbi::Sync),
            WasmExport::Func {
                interface: key,
                func,
                kind: WasmExportKind::Normal,
            },
        )
    }

    /// Generate an exported interface: its types, a Go type holding its
    /// functions, and the instance method that answers it.
    ///
    /// The Go type is a defined type over the instance's struct, so the
    /// accessor is a pointer conversion (no allocation) and its methods use
    /// the instance's module and export memo; the instance's own methods do
    /// not carry over to it.
    fn generate_interface(&self, key: &WorldKey, id: InterfaceId, tokens: &mut Tokens<Go>) {
        let resolve = self.config.resolve;
        let interface = &resolve.interfaces[id];
        let name = match &interface.name {
            Some(name) => name.clone(),
            None => resolve.name_world_key(key),
        };
        let accessor = GoIdentifier::public(&name);
        let exports = GoIdentifier::public(format!("{}-{name}-exports", self.config.world.name));
        let accessor_name = String::from(&accessor);
        let taken = self
            .config
            .world
            .exports
            .values()
            .filter_map(|item| match item {
                WorldItem::Function(func) => Some(String::from(GoIdentifier::public(&func.name))),
                _ => None,
            })
            .chain(["Close".to_string()]);
        for taken in taken {
            assert_ne!(
                taken, accessor_name,
                "exported interface `{name}` would be answered by {accessor_name}, \
                 which the instance already has"
            );
        }

        let analyzer = ImportAnalyzer::new(resolve, self.config.world);
        let types =
            ImportCodeGenerator::new(resolve, self.config.analyzed_imports, self.config.sizes);
        for type_id in interface.types.values() {
            if let Some(typ) = analyzer.analyze_type(*type_id) {
                types.generate_type_definition(&typ, tokens);
            }
        }

        let instance = self.config.instance;
        let wit_name = resolve.name_world_key(key);
        let exports_name = String::from(&exports);
        let instance_name = String::from(instance);
        quote_in! { *tokens =>
            $['\n']
            $(comment(&[
                format!("{exports_name} holds the functions the guest exports from {wit_name}."),
                format!("Get one with {instance_name}.{accessor_name}; like the instance, it is not safe"),
                "for concurrent use.".to_string(),
            ]))
            type $(&exports) $instance
            $['\n']
            func (i *$instance) $(&accessor)() *$(&exports) {
                return (*$(&exports))(i)
            }
        }
        for func in interface.functions.values() {
            self.generate_function(func, &exports, self.wasm_name(Some(key), func), tokens);
        }
    }
}

impl FormatInto<Go> for ExportGenerator<'_> {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        for (key, item) in self.config.world.exports.iter() {
            match item {
                WorldItem::Function(func) => self.generate_function(
                    func,
                    self.config.instance,
                    self.wasm_name(None, func),
                    tokens,
                ),
                WorldItem::Interface { id, .. } => self.generate_interface(key, *id, tokens),
                WorldItem::Type { .. } => todo!("generate type exports"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use genco::prelude::*;

    use crate::{
        codegen::{imports::ImportAnalyzer, test_wit::Fixture},
        go::GoIdentifier,
    };

    use super::{ExportConfig, ExportGenerator};

    /// Generates the Go method for the export `name` in `fixture`'s world.
    fn generate(fixture: &Fixture, name: &str) -> String {
        let instance = GoIdentifier::public("TestInstance");
        let analyzed = ImportAnalyzer::new(&fixture.resolve, fixture.world()).analyze();
        let config = ExportConfig {
            instance: &instance,
            world: fixture.world(),
            resolve: &fixture.resolve,
            sizes: &fixture.sizes,
            analyzed_imports: &analyzed,
        };
        let generator = ExportGenerator::new(config);
        let mut tokens = Tokens::new();
        let func = fixture.export(name);
        generator.generate_function(func, &instance, func.name.clone(), &mut tokens);
        tokens.to_string().unwrap()
    }

    #[test]
    fn test_generate_function_simple_u32_param() {
        let fixture = Fixture::parse(
            "package test:fixture;
            world test-world {
                export add-number: func(value: u32) -> u32;
            }",
        );
        let generated = generate(&fixture, "add-number");
        println!("Generated: {}", generated);

        // Verify basic function structure
        assert!(generated.contains("func (i *TestInstance) AddNumber("));
        assert!(generated.contains("value uint32"));
        assert!(generated.contains("ctx context.Context"));
        assert!(generated.contains(") uint32 {"));

        // Verify function body
        assert!(generated.contains("arg0 := value"));
        assert!(
            generated
                .contains("i.module.ExportedFunction(\"add-number\").Call(ctx, uint64(result0))")
        );
        assert!(generated.contains("if err1 != nil {"));
        assert!(generated.contains("panic(err1)"));
        assert!(generated.contains("results1 := raw1[0]"));
        assert!(generated.contains("result2 := uint32(results1)"));
        assert!(generated.contains("return result2"));

        // I32FromU32 / U32FromI32 are no-op reinterpretations — they must not
        // use api.EncodeU32 or api.DecodeU32 (which round-trip through uint64,
        // causing type mismatches in VariantLower and needless widening elsewhere).
        assert!(
            !generated.contains("api.EncodeU32"),
            "Export must not use api.EncodeU32 (returns uint64 but downstream expects uint32), got:\n{generated}"
        );
        assert!(
            !generated.contains("api.DecodeU32"),
            "Export must not use api.DecodeU32 (needless uint32→uint64→uint32 round-trip), got:\n{generated}"
        );
    }

    /// Regression test: export function with a variant parameter containing
    /// a u32 payload must generate Go code where I32FromU32 produces a
    /// uint32 value matching the VariantLower variable declaration.
    /// Previously I32FromU32 used api.EncodeU32() which returns uint64,
    /// causing a Go compile error: cannot use uint64 as uint32.
    #[test]
    fn test_export_variant_u32_no_encode_u32() {
        let fixture = Fixture::parse(
            "package test:fixture;
            world test-world {
                variant u32-option { some-val(u32), none-val }
                export process-u32-option: func(opt: u32-option) -> u32;
            }",
        );
        let generated = generate(&fixture, "process-u32-option");
        println!("Generated u32-option function:\n{}", generated);

        // VariantLower declares `var variant_1 uint32` for the I32 payload slot.
        // I32FromU32 must NOT use api.EncodeU32 (returns uint64 → type mismatch)
        assert!(
            !generated.contains("api.EncodeU32"),
            "I32FromU32 must not use api.EncodeU32 in exports (returns uint64, \
             but VariantLower variable is uint32), got:\n{generated}"
        );
    }

    /// Regression test: export function with a variant parameter containing
    /// a u64 payload must generate Go code where I64FromU64 produces a
    /// uint64 value matching the VariantLower variable declaration.
    /// Previously I64FromU64 used int64() which returns int64, causing a
    /// Go compile error: cannot use int64 as uint64.
    #[test]
    fn test_export_variant_u64_no_int64_cast() {
        let fixture = Fixture::parse(
            "package test:fixture;
            world test-world {
                variant u64-option { some-val(u64), none-val }
                export process-u64-option: func(opt: u64-option) -> u64;
            }",
        );
        let generated = generate(&fixture, "process-u64-option");
        println!("Generated u64-option function:\n{}", generated);

        // VariantLower declares `var variant_1 uint64` for the I64 payload slot.
        // I64FromU64 must NOT use int64() (returns int64 → type mismatch)
        assert!(
            !generated.contains(":= int64("),
            "I64FromU64 must not use int64() cast in exports (returns int64, \
             but VariantLower variable is uint64), got:\n{generated}"
        );
    }

    /// Regression test: an export whose parameters flatten to more than the
    /// canonical ABI's 16 flat params is lowered *indirectly* - the host must
    /// `cabi_realloc` an area, store each field into it and pass a single
    /// pointer.
    #[test]
    fn test_export_indirect_params_allocates_with_realloc() {
        // A record with 17 u32 fields flattens to 17 core params, one over the
        // limit of 16, which forces indirect parameter lowering.
        let fields: String = (0..17).map(|i| format!("field{i}: u32, ")).collect();
        let fixture = Fixture::parse(&format!(
            "package test:fixture;
            world test-world {{
                record wide {{ {fields} }}
                export take-wide: func(wide: wide) -> u32;
            }}"
        ));
        let generated = generate(&fixture, "take-wide");
        println!("Generated wide-record function:\n{}", generated);

        // The param area is allocated through the guest's `cabi_realloc` with
        // the record's alignment (4) and size (17 * 4 = 68 bytes).
        assert!(
            generated.contains("ExportedFunction(\"cabi_realloc\").Call(ctx, 0, 0, 4, 68)"),
            "indirect params must allocate the param area via cabi_realloc, got:\n{generated}"
        );
        // Each field is stored into that area, and the pointer is the only
        // argument passed to the exported wasm function.
        assert!(
            generated.contains("Memory().WriteUint32Le"),
            "indirect params must be stored into the allocated area, got:\n{generated}"
        );
        assert!(
            generated.contains("ExportedFunction(\"take-wide\").Call(ctx, uint64(ptr"),
            "the wasm export must be called with the param area pointer, got:\n{generated}"
        );
    }

    /// Regression test: an export returning `result<T, E>` with a non-string
    /// E (here an enum) returns the err case as a `*ResultError[E]`. Gravity
    /// used to panic generating it: "TODO(#4): implement remaining result
    /// conversion".
    #[test]
    fn test_export_result_with_enum_err() {
        let fixture = Fixture::parse(
            "package test:fixture;
            world test-world {
                enum refusal { no-credential, malformed, untyped-peer }
                record decided { name: string }
                export decide: func(n: u32) -> result<decided, refusal>;
                export only-err: func(n: u32) -> result<_, refusal>;
            }",
        );
        let decide = generate(&fixture, "decide");
        assert!(decide.contains(") (Decided, error) {"), "got:\n{decide}");
        assert!(
            decide.contains("= &ResultError[Refusal]{Value: enum"),
            "the err case must be a *ResultError[Refusal], got:\n{decide}"
        );
        let only_err = generate(&fixture, "only-err");
        assert!(only_err.contains(") error {"), "got:\n{only_err}");
        assert!(
            only_err.contains("= &ResultError[Refusal]{Value: enum"),
            "got:\n{only_err}"
        );
    }

    /// Generates every export of `fixture`'s world.
    fn generate_all(fixture: &Fixture) -> String {
        let instance = GoIdentifier::public("TestInstance");
        let analyzed = ImportAnalyzer::new(&fixture.resolve, fixture.world()).analyze();
        let config = ExportConfig {
            instance: &instance,
            world: fixture.world(),
            resolve: &fixture.resolve,
            sizes: &fixture.sizes,
            analyzed_imports: &analyzed,
        };
        let mut tokens = Tokens::new();
        ExportGenerator::new(config).format_into(&mut tokens);
        tokens.to_string().unwrap()
    }

    /// Regression test: a world that exports an interface generates the
    /// interface's types, a Go type holding its functions, and the instance
    /// method answering it; each function calls the guest's
    /// `<interface>#<function>` export. Gravity used to panic: "not yet
    /// implemented: generate interface exports".
    #[test]
    fn test_exported_interface() {
        let fixture = Fixture::parse(
            "package test:fixture@1.0.0;
            interface api {
                enum mode { fast, slow }
                record job { name: string, mode: mode }
                run: func(j: job) -> string;
            }
            world test-world {
                export api;
                export version: func() -> u32;
            }",
        );
        let generated = generate_all(&fixture);
        for expected in [
            "type Job struct",
            "ModeFast mode = iota",
            "type TestWorldApiExports TestInstance",
            "func (i *TestInstance) Api() *TestWorldApiExports {",
            "return (*TestWorldApiExports)(i)",
            "func (i *TestWorldApiExports) Run(",
            "i.module.ExportedFunction(\"test:fixture/api@1.0.0#run\")",
            "i.module.ExportedFunction(\"cabi_post_test:fixture/api@1.0.0#run\")",
            "func (i *TestInstance) Version(",
            "i.module.ExportedFunction(\"version\")",
        ] {
            assert!(
                generated.contains(expected),
                "expected `{expected}`, got:\n{generated}"
            );
        }
    }

    /// An exported interface whose accessor would shadow an instance method
    /// is refused at generation, naming the clash.
    #[test]
    #[should_panic(expected = "would be answered by Close")]
    fn test_exported_interface_accessor_clash() {
        let fixture = Fixture::parse(
            "package test:fixture;
            interface close {
                f: func();
            }
            world test-world {
                export close;
            }",
        );
        generate_all(&fixture);
    }

    /// An exported interface whose accessor would be a world function's
    /// method name is refused at generation, naming the clash.
    #[test]
    #[should_panic(expected = "would be answered by Status")]
    fn test_exported_interface_accessor_clashes_with_world_function() {
        let fixture = Fixture::parse(
            "package test:fixture;
            interface status {
                f: func();
            }
            world test-world {
                export status;
                export status: func() -> u32;
            }",
        );
        generate_all(&fixture);
    }
}
