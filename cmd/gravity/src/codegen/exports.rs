use genco::prelude::*;
use wit_bindgen_core::wit_parser::{Function, Resolve, SizeAlign, World, WorldItem};

use crate::go::{GoIdentifier, GoImports, GoResult, GoType};

pub struct ExportConfig<'a> {
    pub instance: &'a GoIdentifier,
    pub go_imports: &'a GoImports,
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
                |(name, wit_type)| match crate::resolve_type(wit_type, self.config.resolve) {
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

        let mut f = crate::Func::export(result, self.config.sizes, self.config.go_imports);
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
                ctx $(&self.config.go_imports.context),
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
                WorldItem::Type(_) => todo!("generate type exports"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use genco::prelude::*;
    use indexmap::indexmap;
    use wit_bindgen_core::wit_parser::{
        Function, FunctionKind, Resolve, SizeAlign, Type, World, WorldItem, WorldKey,
    };

    use crate::go::{GoIdentifier, GoImports};

    use super::{ExportConfig, ExportGenerator};

    #[test]
    fn test_generate_function_simple_u32_param() {
        let func = Function {
            name: "add_number".to_string(),
            kind: FunctionKind::Freestanding,
            params: vec![("value".to_string(), Type::U32)],
            result: Some(Type::U32),
            docs: Default::default(),
            stability: Default::default(),
        };

        let world = World {
            name: "test-world".to_string(),
            imports: indexmap! {},
            exports: indexmap! {
                WorldKey::Name("add-number".to_string()) => WorldItem::Function(func.clone())
            },
            docs: Default::default(),
            stability: Default::default(),
            includes: Default::default(),
            include_names: Default::default(),
            package: None,
        };

        let resolve = Resolve::new();
        let mut sizes = SizeAlign::default();
        sizes.fill(&resolve);
        let instance = GoIdentifier::public("TestInstance");
        let go_imports = GoImports::new();

        let config = ExportConfig {
            instance: &instance,
            go_imports: &go_imports,
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
        assert!(generated.contains("result0 := api.EncodeU32(arg0)"));
        assert!(
            generated
                .contains("i.module.ExportedFunction(\"add_number\").Call(ctx, uint64(result0))")
        );
        assert!(generated.contains("if err1 != nil {"));
        assert!(generated.contains("panic(err1)"));
        assert!(generated.contains("results1 := raw1[0]"));
        assert!(generated.contains("result2 := api.DecodeU32(results1)"));
        assert!(generated.contains("return result2"));
    }
}
