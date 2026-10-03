use std::collections::BTreeMap;

use genco::prelude::*;
use wit_bindgen_core::{
    abi::{AbiVariant, LiftLower},
    wit_parser::{
        Case, Function, InterfaceId, Param, Resolve, SizeAlign, Type, TypeDefKind, TypeId, World,
        WorldItem,
    },
};

use crate::{
    codegen::{
        func::Func,
        ir::{
            AnalyzedFunction, AnalyzedImports, AnalyzedInterface, AnalyzedType, CaseDispatch,
            InterfaceMethod, Parameter, TypeDefinition, VariantCase, WitReturn,
        },
    },
    go::{
        GoIdentifier, GoResult, GoType,
        imports::{CONTEXT_CONTEXT, WAZERO_API_MODULE},
    },
    resolve_param_type, resolve_type, resolve_wasm_type,
};

/// Analyzer for imports - only does analysis, no code generation
pub struct ImportAnalyzer<'a> {
    resolve: &'a Resolve,
    world: &'a World,
}

impl<'a> ImportAnalyzer<'a> {
    pub fn new(resolve: &'a Resolve, world: &'a World) -> Self {
        Self { resolve, world }
    }

    pub fn analyze(&self) -> AnalyzedImports {
        let world_imports = &self.world.imports;
        let mut interfaces = Vec::new();
        let mut standalone_types = Vec::new();
        let mut standalone_functions = Vec::new();

        for (_import_name, world_item) in world_imports.iter() {
            match world_item {
                WorldItem::Interface { id, .. } => {
                    interfaces.push(self.analyze_interface(*id));
                }
                WorldItem::Type { id: type_id, .. } => {
                    if let Some(t) = self.analyze_type(*type_id) {
                        standalone_types.push(t);
                    }
                }
                WorldItem::Function(func) => {
                    standalone_functions.push(self.analyze_function(func));
                }
            }
        }

        // Generate factory-related identifiers
        let factory_name = GoIdentifier::public(format!("{}-factory", self.world.name));
        let instance_name = GoIdentifier::public(format!("{}-instance", self.world.name));
        let constructor_name = GoIdentifier::public(format!("new-{}-factory", self.world.name));

        AnalyzedImports {
            interfaces,
            standalone_types,
            standalone_functions,
            factory_name,
            instance_name,
            constructor_name,
        }
    }

    fn analyze_interface(&self, interface_id: InterfaceId) -> AnalyzedInterface {
        let interface = &self.resolve.interfaces[interface_id];
        let interface_name = interface.name.as_ref().expect("interface missing name");

        // Analyze methods
        let methods = interface
            .functions
            .values()
            .map(|func| self.analyze_interface_method(func, interface_name))
            .collect();

        // Analyze interface types
        let types = interface
            .types
            .values()
            .filter_map(|&id| self.analyze_type(id))
            .collect();

        // Generate names
        let go_interface_name =
            GoIdentifier::public(format!("i-{}-{}", self.world.name, interface_name));

        let wazero_module_name = if let Some(package_id) = interface.package {
            let package = &self.resolve.packages[package_id];
            format!(
                "{}:{}/{}",
                package.name.namespace, package.name.name, interface_name
            )
        } else {
            interface_name.to_string()
        };

        AnalyzedInterface {
            name: interface_name.clone(),
            methods,
            types,
            constructor_param_name: GoIdentifier::private(interface_name),
            go_interface_name,
            wazero_module_name,
        }
    }

    fn analyze_interface_method(&self, func: &Function, _interface_name: &str) -> InterfaceMethod {
        let parameters = func
            .params
            .iter()
            .map(|Param { name, ty, .. }| Parameter {
                name: GoIdentifier::private(name),
                go_type: resolve_param_type(ty, self.resolve),
                wit_type: *ty,
            })
            .collect();

        let return_type = func.result.as_ref().map(|wit_type| WitReturn {
            go_type: resolve_type(wit_type, self.resolve),
            wit_type: *wit_type,
        });

        InterfaceMethod {
            name: func.name.clone(),
            go_method_name: GoIdentifier::public(&func.name),
            parameters,
            return_type,
            wit_function: func.clone(),
        }
    }

    fn analyze_type(&self, type_id: TypeId) -> Option<AnalyzedType> {
        let type_def = &self.resolve.types[type_id];
        let qualified = crate::qualified_type_name(type_id, self.resolve);
        let go_type_name = GoIdentifier::public(&qualified);
        // Variants live here (not in `analyze_type_definition`) because
        // their case wrapper names need the qualified variant name.
        let definition = match &type_def.kind {
            TypeDefKind::Variant(variant) => Some(TypeDefinition::Variant {
                cases: variant
                    .cases
                    .iter()
                    .map(|case| self.analyze_variant_case(&qualified, case))
                    .collect(),
            }),
            kind => self.analyze_type_definition(kind),
        };

        definition.map(|definition| AnalyzedType {
            name: qualified,
            go_type_name,
            definition,
        })
    }

    fn analyze_variant_case(&self, variant_name: &str, case: &Case) -> VariantCase {
        let payload = case.ty.as_ref().map(|t| resolve_type(t, self.resolve));
        let dispatch = match crate::case_dispatch_kind(case, self.resolve) {
            crate::CaseDispatchKind::DirectRecord => CaseDispatch::DirectRecord {
                record_type: payload.clone().expect("DirectRecord case has a payload"),
            },
            crate::CaseDispatchKind::Wrapped => CaseDispatch::Wrapped {
                wrapper_name: GoIdentifier::public(format!("{variant_name}-{}", case.name)),
            },
        };
        VariantCase {
            name: case.name.clone(),
            payload,
            dispatch,
        }
    }

    /// Analyze a type definition. Returns `None` for `Type::Id` aliases
    /// that just re-export an already-analyzed type.
    fn analyze_type_definition(&self, kind: &TypeDefKind) -> Option<TypeDefinition> {
        Some(match kind {
            TypeDefKind::Record(record) => TypeDefinition::Record {
                fields: record
                    .fields
                    .iter()
                    .map(|field| {
                        (
                            GoIdentifier::public(&field.name),
                            resolve_type(&field.ty, self.resolve),
                        )
                    })
                    .collect(),
            },
            TypeDefKind::Enum(enum_def) => TypeDefinition::Enum {
                cases: enum_def.cases.iter().map(|c| c.name.clone()).collect(),
            },
            TypeDefKind::Variant(_) => unreachable!(
                "Variant analysis is handled in `analyze_type` where the qualified name is in scope"
            ),
            TypeDefKind::Type(Type::Id(_)) => {
                // TODO(#4):  Only skip this if we have already generated the type
                return None;
            }
            TypeDefKind::Type(Type::String) => TypeDefinition::Alias {
                target: GoType::String,
            },
            TypeDefKind::Type(Type::Bool) => todo!("TODO(#4): generate bool type alias"),
            TypeDefKind::Type(Type::U8) => todo!("TODO(#4): generate u8 type alias"),
            TypeDefKind::Type(Type::U16) => todo!("TODO(#4): generate u16 type alias"),
            TypeDefKind::Type(Type::U32) => todo!("TODO(#4): generate u32 type alias"),
            TypeDefKind::Type(Type::U64) => todo!("TODO(#4): generate u64 type alias"),
            TypeDefKind::Type(Type::S8) => todo!("TODO(#4): generate s8 type alias"),
            TypeDefKind::Type(Type::S16) => todo!("TODO(#4): generate s16 type alias"),
            TypeDefKind::Type(Type::S32) => todo!("TODO(#4): generate s32 type alias"),
            TypeDefKind::Type(Type::S64) => todo!("TODO(#4): generate s64 type alias"),
            TypeDefKind::Type(Type::F32) => todo!("TODO(#4): generate f32 type alias"),
            TypeDefKind::Type(Type::F64) => todo!("TODO(#4): generate f64 type alias"),
            TypeDefKind::Type(Type::Char) => todo!("TODO(#4): generate char type alias"),
            TypeDefKind::Type(Type::ErrorContext) => {
                todo!("TODO(#4): generate error context definition")
            }
            TypeDefKind::FixedLengthList(_, _) => {
                todo!("TODO(#4): generate fixed length list definition")
            }
            TypeDefKind::Option(_) => todo!("TODO(#4): generate option type definition"),
            TypeDefKind::Result(_) => todo!("TODO(#4): generate result type definition"),
            TypeDefKind::List(_) => todo!("TODO(#4): generate list type definition"),
            TypeDefKind::Future(_) => todo!("TODO(#4): generate future type definition"),
            TypeDefKind::Stream(_) => todo!("TODO(#4): generate stream type definition"),
            TypeDefKind::Flags(_) => todo!("TODO(#4):generate flags type definition"),
            TypeDefKind::Tuple(_) => todo!("TODO(#4):generate tuple type definition"),
            TypeDefKind::Resource => todo!("TODO(#5): implement resources"),
            TypeDefKind::Handle(_) => todo!("TODO(#5): implement resources"),
            TypeDefKind::Map(_, _) => todo!("TODO(#4): generate map type definition"),
            TypeDefKind::Unknown => panic!("cannot generate Unknown type"),
        })
    }

    fn analyze_function(&self, func: &Function) -> AnalyzedFunction {
        let parameters = func
            .params
            .iter()
            .map(|Param { name, ty, .. }| Parameter {
                name: GoIdentifier::private(name),
                go_type: resolve_param_type(ty, self.resolve),
                wit_type: *ty,
            })
            .collect();

        let return_type = func
            .result
            .as_ref()
            .map(|wit_type| resolve_type(wit_type, self.resolve));

        AnalyzedFunction {
            name: func.name.clone(),
            go_name: GoIdentifier::public(&func.name),
            parameters,
            return_type,
        }
    }
}

/// Code generator for imports - takes analysis results and generates Go code
pub struct ImportCodeGenerator<'a> {
    resolve: &'a Resolve,
    analyzed: &'a AnalyzedImports,
    sizes: &'a SizeAlign,
}

impl<'a> ImportCodeGenerator<'a> {
    /// Create a new import code generator with the given imports and analyzed results.
    pub fn new(resolve: &'a Resolve, analyzed: &'a AnalyzedImports, sizes: &'a SizeAlign) -> Self {
        Self {
            resolve,
            analyzed,
            sizes,
        }
    }

    /// Extract import chains for host module builders
    pub fn import_chains(&self) -> BTreeMap<String, Tokens<Go>> {
        let mut chains = BTreeMap::new();

        for (i, interface) in self.analyzed.interfaces.iter().enumerate() {
            let err = &GoIdentifier::private(format!("err{i}"));
            let mut chain = quote! {
                _, $err := wazeroRuntime.NewHostModuleBuilder($(quoted(&interface.wazero_module_name))).
            };

            for method in &interface.methods {
                chain.push();
                let func_builder =
                    self.generate_host_function_builder(method, &interface.constructor_param_name);
                quote_in! { chain =>
                    $func_builder
                };
            }

            chain.push();
            quote_in! { chain =>
                Instantiate(ctx)
                if $err != nil {
                    return nil, $err
                }
            };

            chains.insert(interface.wazero_module_name.clone(), chain);
        }

        chains
    }
}

impl FormatInto<Go> for ImportCodeGenerator<'_> {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        // Generate interface type definitions
        for interface in &self.analyzed.interfaces {
            self.generate_interface_type(interface, tokens);

            for typ in &interface.types {
                self.generate_type_definition(typ, tokens);
            }
        }

        // Generate standalone types
        for typ in &self.analyzed.standalone_types {
            self.generate_type_definition(typ, tokens);
        }
    }
}

impl<'a> ImportCodeGenerator<'a> {
    fn generate_interface_type(&self, interface: &AnalyzedInterface, tokens: &mut Tokens<Go>) {
        let methods = interface
            .methods
            .iter()
            .map(|method| self.generate_method_signature(method));

        quote_in! { *tokens =>
            $['\n']
            type $(&interface.go_interface_name) interface {
                $(for method in methods join ($['\r']) => $method)
            }
        }
    }

    fn generate_method_signature(&self, method: &InterfaceMethod) -> Tokens<Go> {
        let return_type = method
            .return_type
            .clone()
            .map(|t| GoResult::Anon(t.go_type))
            .unwrap_or(GoResult::Empty);

        quote! {
            $(&method.go_method_name)(
                ctx $CONTEXT_CONTEXT,
                $(for param in &method.parameters join ($['\r']) => $(&param.name) $(&param.go_type),)
            ) $return_type
        }
    }

    fn generate_type_definition(&self, typ: &AnalyzedType, tokens: &mut Tokens<Go>) {
        match &typ.definition {
            TypeDefinition::Record { fields } => {
                quote_in! { *tokens =>
                    $['\n']
                    type $(&typ.go_type_name) struct {
                        $(for (field_name, field_type) in fields join ($['\r']) =>
                            $field_name $field_type
                        )
                    }
                }
            }
            TypeDefinition::Enum { cases } => {
                let enum_type = &GoIdentifier::private(&typ.name);
                let enum_interface = &typ.go_type_name;
                let enum_function = &GoIdentifier::private(format!("is-{}", &typ.name));
                let variants = cases.iter().map(GoIdentifier::public);
                quote_in! { *tokens =>
                    $['\n']
                    type $(enum_interface) interface {
                        $(enum_function)()
                    }
                    $['\n']
                    type $(enum_type) int
                    $['\n']
                    func ($(enum_type)) $enum_function() {}
                    $['\n']
                    const (
                        $(for name in variants join ($['\r']) => $name $enum_type = iota)
                    )
                    $['\n']
                }
            }
            TypeDefinition::Alias { target } => {
                // TODO(#4): We might want a Type Definition (newtype) instead of Type Alias here
                quote_in! { *tokens =>
                    $['\n']
                    type $(&typ.go_type_name) = $target
                }
            }
            TypeDefinition::Primitive => {
                quote_in! { *tokens =>
                    $['\n']
                    // Primitive type: $(typ.name)
                }
            }
            TypeDefinition::Variant { cases } => {
                let variant_interface = &typ.go_type_name;
                let marker_method = &GoIdentifier::private(format!("is-{}", &typ.name));
                let case_definitions = cases.iter().map(|case| match &case.dispatch {
                    CaseDispatch::DirectRecord { record_type } => quote! {
                        $['\n']
                        func ($record_type) $marker_method() {}
                    },
                    CaseDispatch::Wrapped { wrapper_name } => {
                        let payload_field = case.payload.as_ref().map(|p| quote!(Value $p));
                        quote! {
                            $['\n']
                            type $wrapper_name struct {
                                $(if let Some(field) = payload_field => $field)
                            }
                            $['\n']
                            func ($wrapper_name) $marker_method() {}
                        }
                    }
                });
                quote_in! { *tokens =>
                    $['\n']
                    type $variant_interface interface {
                        $marker_method()
                    }
                    $(for def in case_definitions => $def)
                }
            }
        }
    }

    fn generate_host_function_builder(
        &self,
        method: &InterfaceMethod,
        // The name of the parameter representing the interface instance
        // in the generated function.
        param_name: &GoIdentifier,
    ) -> Tokens<Go> {
        let func_name = &method.name;

        let wasm_sig = self
            .resolve
            .wasm_signature(AbiVariant::GuestImport, &method.wit_function);
        let result = if wasm_sig.results.is_empty() {
            GoResult::Empty
        } else if wasm_sig.results.len() == 1 {
            GoResult::Anon(resolve_wasm_type(&wasm_sig.results[0]))
        } else {
            todo!("implement handling of wasm signatures with multiple results");
        };
        let mut f = Func::import(param_name, result, self.sizes);

        // Magic
        wit_bindgen_core::abi::call(
            self.resolve,
            AbiVariant::GuestImport,
            LiftLower::LiftArgsLowerResults,
            &method.wit_function,
            &mut f,
            // async is not currently supported
            false,
        );

        // Collect all host function parameters into a single list so
        // that the join produces correct commas even when there are no
        // WIT-level parameters (only ctx and mod).
        let mut all_params: Vec<Tokens<Go>> = vec![
            quote! { ctx $CONTEXT_CONTEXT },
            quote! { mod $WAZERO_API_MODULE },
        ];
        for arg in f.args() {
            all_params.push(quote! { $arg uint32 });
        }

        quote! {
            NewFunctionBuilder().
            WithFunc(func(
                $(for param in all_params join (,$['\r']) => $param),
            ) $(f.result()){
                $(f.body())
            }).
            Export($(quoted(func_name))).
        }
    }
}

#[cfg(test)]
mod tests {
    use genco::prelude::*;
    use wit_bindgen_core::wit_parser::Type;

    use crate::{
        codegen::{
            imports::{ImportAnalyzer, ImportCodeGenerator},
            ir::{AnalyzedImports, InterfaceMethod, Parameter, TypeDefinition, WitReturn},
            test_wit::Fixture,
        },
        go::{GoIdentifier, GoType},
    };

    /// A fixture whose `host` interface declares `functions`, imported by the
    /// world `test-world`.
    fn host_fixture(functions: &str) -> Fixture {
        Fixture::parse(&format!(
            "package test:fixture;
            interface host {{
                {functions}
            }}
            world test-world {{
                import host;
            }}"
        ))
    }

    fn empty_analysis() -> AnalyzedImports {
        AnalyzedImports {
            instance_name: GoIdentifier::public("TestInstance"),
            interfaces: vec![],
            standalone_functions: vec![],
            standalone_types: vec![],
            factory_name: GoIdentifier::public("TestFactory"),
            constructor_name: GoIdentifier::public("NewTestFactory"),
        }
    }

    /// Generates the host function builder for `method` with the
    /// fixture's resolve and sizes.
    fn host_function(fixture: &Fixture, method: &InterfaceMethod) -> String {
        let analyzed = empty_analysis();
        let generator = ImportCodeGenerator::new(&fixture.resolve, &analyzed, &fixture.sizes);
        let param_name = GoIdentifier::private("handler");
        generator
            .generate_host_function_builder(method, &param_name)
            .to_string()
            .unwrap()
    }

    #[test]
    fn test_wit_type_driven_generation() {
        let fixture = host_fixture("test-function: func(input: string) -> string;");
        let method = InterfaceMethod {
            name: "test-function".to_string(),
            go_method_name: GoIdentifier::public("TestFunction"),
            parameters: vec![Parameter {
                name: GoIdentifier::private("input"),
                go_type: GoType::String,
                wit_type: Type::String,
            }],
            return_type: Some(WitReturn {
                go_type: GoType::String,
                wit_type: Type::String,
            }),
            wit_function: fixture.function("host", "test-function").clone(),
        };

        // The result should contain the WIT type-driven generation
        let code_str = host_function(&fixture, &method);
        assert!(code_str.contains("NewFunctionBuilder"));
        assert!(code_str.contains("mod.Memory().Read"));
        assert!(code_str.contains("writeString"));

        println!("Generated code:\n{}", code_str);
    }

    #[test]
    fn test_different_wit_types() {
        // Test that different WIT types generate different parameter handling
        let fixture = host_fixture("test-u32: func(value: u32);");
        let u32_method = InterfaceMethod {
            name: "test-u32".to_string(),
            go_method_name: GoIdentifier::public("TestU32"),
            parameters: vec![Parameter {
                name: GoIdentifier::private("value"),
                go_type: GoType::Uint32,
                wit_type: Type::U32,
            }],
            return_type: None,
            wit_function: fixture.function("host", "test-u32").clone(),
        };

        // Should have only one uint32 parameter (plus ctx and mod)
        let code_str = host_function(&fixture, &u32_method);
        assert!(code_str.contains("arg0 uint32"));
        assert!(!code_str.contains("arg1 uint32"));
        assert!(!code_str.contains("mod.Memory().Read")); // No string reading

        println!("U32 generated code:\n{}", code_str);
    }

    /// Regression test: import functions whose WIT return type maps to a Wasm
    /// result (e.g. `bool`, `enum`) must produce a non-empty Go return type
    /// in the host function signature. A refactoring replaced the handling
    /// with `todo!()`, which caused a panic at build time.
    #[test]
    fn test_import_with_bool_return_type() {
        // A function returning bool has a single i32 Wasm result
        let fixture = host_fixture("is-valid: func(input: string) -> bool;");
        let method = InterfaceMethod {
            name: "is-valid".to_string(),
            go_method_name: GoIdentifier::public("IsValid"),
            parameters: vec![Parameter {
                name: GoIdentifier::private("input"),
                go_type: GoType::String,
                wit_type: Type::String,
            }],
            return_type: Some(WitReturn {
                go_type: GoType::Bool,
                wit_type: Type::Bool,
            }),
            wit_function: fixture.function("host", "is-valid").clone(),
        };

        let code_str = host_function(&fixture, &method);
        // The host function must declare a uint32 return (Wasm i32 representation of bool)
        assert!(
            code_str.contains(") uint32"),
            "Expected host function to return uint32, got:\n{code_str}"
        );
        // The body must contain a return statement
        assert!(
            code_str.contains("return"),
            "Expected a return statement in the generated code, got:\n{code_str}"
        );
    }

    /// Same regression test but for enum return types, which is the exact
    /// case that was failing in Arcjet's rule code.
    /// (`verify: func(bot-id: string, ip: string) -> validator-response`).
    #[test]
    fn test_import_with_enum_return_type() {
        // A function returning an enum has a single i32 Wasm result
        let fixture = host_fixture(
            "enum status { ok, failed }
            get-status: func(id: string) -> status;",
        );
        let method = InterfaceMethod {
            name: "get-status".to_string(),
            go_method_name: GoIdentifier::public("GetStatus"),
            parameters: vec![Parameter {
                name: GoIdentifier::private("id"),
                go_type: GoType::String,
                wit_type: Type::String,
            }],
            return_type: Some(WitReturn {
                go_type: GoType::Uint32,
                wit_type: Type::Id(fixture.type_id("status")),
            }),
            wit_function: fixture.function("host", "get-status").clone(),
        };

        let code_str = host_function(&fixture, &method);
        // The host function must declare a uint32 return (Wasm i32 representation of enum)
        assert!(
            code_str.contains(") uint32"),
            "Expected host function to return uint32, got:\n{code_str}"
        );
        assert!(
            code_str.contains("return"),
            "Expected a return statement in the generated code, got:\n{code_str}"
        );
    }

    /// Regression test: import functions with u32 parameters must generate
    /// simple `uint32()` casts, not `api.DecodeU32()` / `api.EncodeU32()`.
    /// Those wazero API functions convert between uint32 and uint64 and are
    /// only appropriate for the api.Function.Call() pathway (exports). In
    /// the import (host function) pathway, params are already uint32.
    #[test]
    fn test_import_u32_params_use_identity_cast() {
        // A function that takes multiple u32 params — the same pattern as
        // rate-limit's token-bucket import.
        let fixture = host_fixture("compute: func(a: u32, b: u32);");
        let method = InterfaceMethod {
            name: "compute".to_string(),
            go_method_name: GoIdentifier::public("Compute"),
            parameters: vec![
                Parameter {
                    name: GoIdentifier::private("a"),
                    go_type: GoType::Uint32,
                    wit_type: Type::U32,
                },
                Parameter {
                    name: GoIdentifier::private("b"),
                    go_type: GoType::Uint32,
                    wit_type: Type::U32,
                },
            ],
            return_type: None,
            wit_function: fixture.function("host", "compute").clone(),
        };

        let code_str = host_function(&fixture, &method);
        // Must use simple uint32() casts, NOT api.DecodeU32() which expects uint64
        assert!(
            !code_str.contains("api.DecodeU32"),
            "Import must not use api.DecodeU32 (expects uint64 but params are uint32), got:\n{code_str}"
        );
        assert!(
            !code_str.contains("api.EncodeU32"),
            "Import must not use api.EncodeU32 (returns uint64 but context expects uint32), got:\n{code_str}"
        );
        // Should use uint32() identity casts instead
        assert!(
            code_str.contains("uint32("),
            "Expected uint32() identity cast in generated code, got:\n{code_str}"
        );
    }

    /// Regression test: import functions with zero WIT parameters must not
    /// produce a trailing comma after `mod api.Module` in the host function
    /// signature. Previously, the template unconditionally emitted a comma
    /// separator between the fixed params (ctx, mod) and the WIT params,
    /// resulting in `func(ctx context.Context, mod api.Module, ,)` which
    /// is a Go syntax error.
    #[test]
    fn test_import_zero_params_no_trailing_comma() {
        // A function with no WIT parameters — only ctx and mod should appear
        // in the generated Go host function signature.
        let fixture = host_fixture("ping: func();");
        let method = InterfaceMethod {
            name: "ping".to_string(),
            go_method_name: GoIdentifier::public("Ping"),
            parameters: vec![],
            return_type: None,
            wit_function: fixture.function("host", "ping").clone(),
        };

        let code_str = host_function(&fixture, &method);
        // Must NOT contain a bare comma on its own line (the symptom of the bug)
        assert!(
            !code_str.contains(",\n\t\t,"),
            "Host function signature must not have consecutive commas, got:\n{code_str}"
        );
        // Must NOT contain ", ," which is another form of the double comma
        assert!(
            !code_str.contains(", ,"),
            "Host function signature must not have consecutive commas, got:\n{code_str}"
        );
        // The signature should close cleanly after mod api.Module
        assert!(
            code_str.contains("mod api.Module,\n)") || code_str.contains("mod api.Module,\n\t)"),
            "Expected host function params to end with 'mod api.Module,' followed by closing paren, got:\n{code_str}"
        );
    }

    /// Same as above but with a return type — zero params + bool return
    /// exercises both the zero-param fix and the result-type fix together.
    #[test]
    fn test_import_zero_params_with_return_type() {
        let fixture = host_fixture("is-ready: func() -> bool;");
        let method = InterfaceMethod {
            name: "is-ready".to_string(),
            go_method_name: GoIdentifier::public("IsReady"),
            parameters: vec![],
            return_type: Some(WitReturn {
                go_type: GoType::Bool,
                wit_type: Type::Bool,
            }),
            wit_function: fixture.function("host", "is-ready").clone(),
        };

        let code_str = host_function(&fixture, &method);
        // Must not have consecutive commas
        assert!(
            !code_str.contains(",\n\t\t,") && !code_str.contains(", ,"),
            "Host function signature must not have consecutive commas, got:\n{code_str}"
        );
        // Must have uint32 return type
        assert!(
            code_str.contains(") uint32"),
            "Expected uint32 return type, got:\n{code_str}"
        );
        // Must have a return statement
        assert!(
            code_str.contains("return"),
            "Expected a return statement, got:\n{code_str}"
        );
    }

    fn logger_fixture() -> Fixture {
        Fixture::parse(
            "package test:pkg;
            interface logger {
                log: func(message: string);
            }
            world test-world {
                import logger;
            }",
        )
    }

    #[test]
    fn test_import_analyzer() {
        let fixture = logger_fixture();
        let analyzer = ImportAnalyzer::new(&fixture.resolve, fixture.world());
        let analyzed = analyzer.analyze();

        // Check that we got one interface
        assert_eq!(analyzed.interfaces.len(), 1);
        let interface = &analyzed.interfaces[0];

        assert_eq!(interface.name, "logger");
        assert_eq!(interface.methods.len(), 1);

        let method = &interface.methods[0];
        assert_eq!(method.name, "log");
        assert_eq!(method.parameters.len(), 1);

        let param = &method.parameters[0];
        assert!(matches!(param.go_type, GoType::String));
    }

    #[test]
    fn test_import_code_generator() {
        let fixture = logger_fixture();

        // Analyze
        let analyzer = ImportAnalyzer::new(&fixture.resolve, fixture.world());
        let analyzed = analyzer.analyze();

        // Generate
        let generator = ImportCodeGenerator::new(&fixture.resolve, &analyzed, &fixture.sizes);
        let mut tokens = Tokens::<Go>::new();
        generator.format_into(&mut tokens);

        let output = tokens.to_string().unwrap();
        assert!(output.contains("type ITestWorldLogger interface"));
        assert!(output.contains("Log("));
    }

    #[test]
    fn test_record_type_generation() {
        let fixture = Fixture::parse(
            "package test:records;
            interface types {
                record foo {
                    float32: f32,
                    float64: f64,
                    uint32: u32,
                    uint64: u64,
                    s: string,
                }
            }
            world test-world {
                import types;
            }",
        );
        let analyzer = ImportAnalyzer::new(&fixture.resolve, fixture.world());

        // Test analyze_type_definition directly with the record kind
        let type_def = &fixture.resolve.types[fixture.type_id("foo")];
        let analyzed_definition = analyzer.analyze_type_definition(&type_def.kind).unwrap();

        // This should be a Record, not an Alias
        match &analyzed_definition {
            TypeDefinition::Record { fields } => assert_eq!(fields.len(), 5),
            other => panic!("expected a Record, got: {other:?}"),
        }

        // Check analysis results
        let analyzed = analyzer.analyze();
        assert_eq!(analyzed.interfaces.len(), 1);
        let interface = &analyzed.interfaces[0];
        assert_eq!(interface.name, "types");
        assert_eq!(interface.types.len(), 1);

        let analyzed_type = &interface.types[0];
        // Interface-scoped types are qualified only when their bare name
        // would collide with another concrete type in the same world. The
        // test world's `foo` is unique, so it stays flat.
        assert_eq!(analyzed_type.name, "foo");

        // This is the key assertion - it should be a Record, not an Alias
        match &analyzed_type.definition {
            TypeDefinition::Record { fields } => {
                assert_eq!(fields.len(), 5);

                // Check that field names are correct
                let field_names: Vec<String> =
                    fields.iter().map(|(name, _)| String::from(name)).collect();
                assert!(field_names.contains(&"Float32".to_string()));
                assert!(field_names.contains(&"Float64".to_string()));
                assert!(field_names.contains(&"Uint32".to_string()));
                assert!(field_names.contains(&"Uint64".to_string()));
                assert!(field_names.contains(&"S".to_string()));
            }
            other => panic!("expected a Record, got: {other:?}"),
        }

        // Generating the record must not produce the self-referential alias
        // `type Foo Foo`.
        let generator = ImportCodeGenerator::new(&fixture.resolve, &analyzed, &fixture.sizes);
        let mut tokens = Tokens::<Go>::new();
        generator.format_into(&mut tokens);
        let output = tokens.to_string().unwrap();
        assert!(
            !output.contains("type Foo Foo"),
            "generated a self-referential alias, got:\n{output}"
        );
    }

    #[test]
    fn test_record_vs_alias_analysis() {
        let fixture = Fixture::parse(
            "package test:types;
            interface types {
                record my-record { x: u32 }
                type my-alias = string;
            }
            world test-world {
                import types;
            }",
        );
        let analyzer = ImportAnalyzer::new(&fixture.resolve, fixture.world());

        // Test record analysis
        let record_def = &fixture.resolve.types[fixture.type_id("my-record")];
        match analyzer.analyze_type_definition(&record_def.kind).unwrap() {
            TypeDefinition::Record { .. } => {}
            other => panic!("record analyzed as: {other:?}"),
        }

        // Test alias analysis
        let alias_def = &fixture.resolve.types[fixture.type_id("my-alias")];
        match analyzer.analyze_type_definition(&alias_def.kind).unwrap() {
            TypeDefinition::Alias { .. } => {}
            other => panic!("alias analyzed as: {other:?}"),
        }
    }
}
