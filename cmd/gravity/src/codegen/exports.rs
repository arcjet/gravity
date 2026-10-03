use genco::prelude::*;
use wit_bindgen_core::wit_parser::{Function, Param, Resolve, SizeAlign, World, WorldItem};

use crate::go::{GoIdentifier, GoResult, GoType, imports::CONTEXT_CONTEXT};

pub struct ExportConfig<'a> {
    pub instance: &'a GoIdentifier,
    pub world: &'a World,
    pub resolve: &'a Resolve,
    pub sizes: &'a SizeAlign,
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
    fn generate_function(&self, func: &Function, tokens: &mut Tokens<Go>) {
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

        let mut f = crate::Func::export(result, self.config.sizes);
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
            func (i *$(self.config.instance)) $fn_name(
                $['\r']
                ctx $CONTEXT_CONTEXT,
                $(for (name, typ) in &params join ($['\r']) => $name $typ,)
            ) $(f.result()) {
                $(for (arg, param) in arg_assignments join ($['\r']) => $arg := $param)
                $(f.body())
            }
        }
    }
}

impl FormatInto<Go> for ExportGenerator<'_> {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        for item in self.config.world.exports.values() {
            match item {
                WorldItem::Function(func) => self.generate_function(func, tokens),
                WorldItem::Interface { .. } => todo!("generate interface exports"),
                WorldItem::Type { .. } => todo!("generate type exports"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use genco::prelude::*;

    use crate::{codegen::test_wit::Fixture, go::GoIdentifier};

    use super::{ExportConfig, ExportGenerator};

    /// Generates the Go method for the export `name` in `fixture`'s world.
    fn generate(fixture: &Fixture, name: &str) -> String {
        let instance = GoIdentifier::public("TestInstance");
        let config = ExportConfig {
            instance: &instance,
            world: fixture.world(),
            resolve: &fixture.resolve,
            sizes: &fixture.sizes,
        };
        let generator = ExportGenerator::new(config);
        let mut tokens = Tokens::new();
        generator.generate_function(fixture.export(name), &mut tokens);
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
}
