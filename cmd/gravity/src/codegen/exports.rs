use genco::prelude::*;
use wit_bindgen_core::wit_parser::{Function, Param, Resolve, SizeAlign, World, WorldItem};

use crate::go::{imports::CONTEXT_CONTEXT, GoIdentifier, GoResult, GoType};

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
            .map(
                |Param { name, ty, .. }| match crate::resolve_type(ty, self.config.resolve) {
                    GoType::ValueOrOk(t) => (GoIdentifier::local(name), *t),
                    t => (GoIdentifier::local(name), t),
                },
            )
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
    use wit_bindgen_core::wit_parser::{
        Function, FunctionKind, Param, Resolve, SizeAlign, Type, World, WorldItem, WorldKey,
    };

    use crate::go::GoIdentifier;

    use super::{ExportConfig, ExportGenerator};

    #[test]
    fn test_generate_function_simple_u32_param() {
        let func = Function {
            name: "add_number".to_string(),
            kind: FunctionKind::Freestanding,
            params: vec![Param {
                name: "value".to_string(),
                ty: Type::U32,
                span: Default::default(),
            }],
            result: Some(Type::U32),
            docs: Default::default(),
            stability: Default::default(),
            span: Default::default(),
        };

        let world = World {
            name: "test-world".to_string(),
            imports: [].into(),
            exports: [(
                WorldKey::Name("add-number".to_string()),
                WorldItem::Function(func.clone()),
            )]
            .into(),
            docs: Default::default(),
            stability: Default::default(),
            includes: Default::default(),
            span: Default::default(),
            package: None,
        };

        let resolve = Resolve::new();
        let mut sizes = SizeAlign::default();
        sizes.fill(&resolve);
        let instance = GoIdentifier::public("TestInstance");

        let config = ExportConfig {
            instance: &instance,
            world: &world,
            resolve: &resolve,
            sizes: &sizes,
        };

        let generator = ExportGenerator::new(config);
        let mut tokens = Tokens::new();

        // Call the actual generate_function method
        generator.generate_function(&func, &mut tokens);

        let generated = tokens.to_string().unwrap();
        println!("Generated: {}", generated);

        // Verify basic function structure
        assert!(generated.contains("func (i *TestInstance) AddNumber("));
        assert!(generated.contains("value uint32"));
        assert!(generated.contains("ctx context.Context"));
        assert!(generated.contains(") uint32 {"));

        // Verify function body
        assert!(generated.contains("arg0 := value"));
        assert!(generated
            .contains("i.module.ExportedFunction(\"add_number\").Call(ctx, uint64(result0))"));
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
        use wit_bindgen_core::wit_parser::{
            Case, TypeDef, TypeDefKind, TypeOwner, Variant,
        };

        let mut resolve = Resolve::new();

        // variant u32-option { some-val(u32), none-val }
        let variant_def = TypeDef {
            name: Some("u32-option".to_string()),
            kind: TypeDefKind::Variant(Variant {
                cases: vec![
                    Case {
                        name: "some-val".to_string(),
                        ty: Some(Type::U32),
                        docs: Default::default(),
                        span: Default::default(),
                    },
                    Case {
                        name: "none-val".to_string(),
                        ty: None,
                        docs: Default::default(),
                        span: Default::default(),
                    },
                ],
            }),
            owner: TypeOwner::None,
            docs: Default::default(),
            stability: Default::default(),
            span: Default::default(),
        };
        let variant_id = resolve.types.alloc(variant_def);

        let func = Function {
            name: "process_u32_option".to_string(),
            kind: FunctionKind::Freestanding,
            params: vec![Param {
                name: "opt".to_string(),
                ty: Type::Id(variant_id),
                span: Default::default(),
            }],
            result: Some(Type::U32),
            docs: Default::default(),
            stability: Default::default(),
            span: Default::default(),
        };

        let world = World {
            name: "test-world".to_string(),
            imports: [].into(),
            exports: [(
                WorldKey::Name("process-u32-option".to_string()),
                WorldItem::Function(func.clone()),
            )]
            .into(),
            docs: Default::default(),
            stability: Default::default(),
            includes: Default::default(),
            span: Default::default(),
            package: None,
        };

        let mut sizes = SizeAlign::default();
        sizes.fill(&resolve);
        let instance = GoIdentifier::public("TestInstance");

        let config = ExportConfig {
            instance: &instance,
            world: &world,
            resolve: &resolve,
            sizes: &sizes,
        };

        let generator = ExportGenerator::new(config);
        let mut tokens = Tokens::new();
        generator.generate_function(&func, &mut tokens);

        let generated = tokens.to_string().unwrap();
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
        use wit_bindgen_core::wit_parser::{
            Case, TypeDef, TypeDefKind, TypeOwner, Variant,
        };

        let mut resolve = Resolve::new();

        // variant u64-option { some-val(u64), none-val }
        let variant_def = TypeDef {
            name: Some("u64-option".to_string()),
            kind: TypeDefKind::Variant(Variant {
                cases: vec![
                    Case {
                        name: "some-val".to_string(),
                        ty: Some(Type::U64),
                        docs: Default::default(),
                        span: Default::default(),
                    },
                    Case {
                        name: "none-val".to_string(),
                        ty: None,
                        docs: Default::default(),
                        span: Default::default(),
                    },
                ],
            }),
            owner: TypeOwner::None,
            docs: Default::default(),
            stability: Default::default(),
            span: Default::default(),
        };
        let variant_id = resolve.types.alloc(variant_def);

        let func = Function {
            name: "process_u64_option".to_string(),
            kind: FunctionKind::Freestanding,
            params: vec![Param {
                name: "opt".to_string(),
                ty: Type::Id(variant_id),
                span: Default::default(),
            }],
            result: Some(Type::U64),
            docs: Default::default(),
            stability: Default::default(),
            span: Default::default(),
        };

        let world = World {
            name: "test-world".to_string(),
            imports: [].into(),
            exports: [(
                WorldKey::Name("process-u64-option".to_string()),
                WorldItem::Function(func.clone()),
            )]
            .into(),
            docs: Default::default(),
            stability: Default::default(),
            includes: Default::default(),
            span: Default::default(),
            package: None,
        };

        let mut sizes = SizeAlign::default();
        sizes.fill(&resolve);
        let instance = GoIdentifier::public("TestInstance");

        let config = ExportConfig {
            instance: &instance,
            world: &world,
            resolve: &resolve,
            sizes: &sizes,
        };

        let generator = ExportGenerator::new(config);
        let mut tokens = Tokens::new();
        generator.generate_function(&func, &mut tokens);

        let generated = tokens.to_string().unwrap();
        println!("Generated u64-option function:\n{}", generated);

        // VariantLower declares `var variant_1 uint64` for the I64 payload slot.
        // I64FromU64 must NOT use int64() (returns int64 → type mismatch)
        assert!(
            !generated.contains(":= int64("),
            "I64FromU64 must not use int64() cast in exports (returns int64, \
             but VariantLower variable is uint64), got:\n{generated}"
        );
    }
}
