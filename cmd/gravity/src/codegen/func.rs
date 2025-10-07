use std::mem;

use genco::prelude::*;
use wit_bindgen_core::{
    abi::{Bindgen, Instruction},
    wit_parser::{Alignment, ArchitectureSize, Resolve, Result_, SizeAlign, Type},
};

use crate::{
    go::{GoIdentifier, GoImports, GoResult, GoType, Operand, comment},
    resolve_type, resolve_wasm_type,
};

/// The direction of a function.
///
/// Functions in the Component Model can be imported into a world or
/// exported from a world.
enum Direction<'a> {
    /// The function is imported into the world.
    Import {
        /// The name of the parameter representing the interface instance
        /// in the generated host binding function.
        param_name: &'a GoIdentifier,
    },
    /// The function is exported from the world.
    #[allow(dead_code, reason = "halfway through refactor of func bindings")]
    Export,
}

pub struct Func<'a> {
    direction: Direction<'a>,
    args: Vec<String>,
    result: GoResult,
    tmp: usize,
    body: Tokens<Go>,
    block_storage: Vec<Tokens<Go>>,
    blocks: Vec<(Tokens<Go>, Vec<Operand>)>,
    sizes: &'a SizeAlign,
    go_imports: &'a GoImports,
}

impl<'a> Func<'a> {
    /// Create a new exported function.
    #[allow(dead_code, reason = "halfway through refactor of func bindings")]
    pub fn export(result: GoResult, sizes: &'a SizeAlign, go_imports: &'a GoImports) -> Self {
        Self {
            direction: Direction::Export,
            args: Vec::new(),
            result,
            tmp: 0,
            body: Tokens::new(),
            block_storage: Vec::new(),
            blocks: Vec::new(),
            sizes,
            go_imports,
        }
    }

    /// Create a new exported function.
    pub fn import(
        param_name: &'a GoIdentifier,
        result: GoResult,
        sizes: &'a SizeAlign,
        go_imports: &'a GoImports,
    ) -> Self {
        Self {
            direction: Direction::Import { param_name },
            args: Vec::new(),
            result,
            tmp: 0,
            body: Tokens::new(),
            block_storage: Vec::new(),
            blocks: Vec::new(),
            sizes,
            go_imports,
        }
    }

    fn tmp(&mut self) -> usize {
        let ret = self.tmp;
        self.tmp += 1;
        ret
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn result(&self) -> &GoResult {
        &self.result
    }

    pub fn body(&self) -> &Tokens<Go> {
        &self.body
    }

    fn push_arg(&mut self, value: &str) {
        self.args.push(value.into())
    }

    fn pop_block(&mut self) -> (Tokens<Go>, Vec<Operand>) {
        self.blocks.pop().expect("should have block to pop")
    }
}

impl Bindgen for Func<'_> {
    type Operand = Operand;

    fn emit(
        &mut self,
        resolve: &Resolve,
        inst: &Instruction<'_>,
        operands: &mut Vec<Self::Operand>,
        results: &mut Vec<Self::Operand>,
    ) {
        let errors_new = &go::import("errors", "New");
        let iter_element = "e";
        let iter_base = "base";

        match inst {
            Instruction::GetArg { nth } => {
                let arg = &format!("arg{nth}");
                self.push_arg(arg);
                results.push(Operand::SingleValue(arg.into()));
            }
            Instruction::ConstZero { tys } => {
                for _ in tys.iter() {
                    results.push(Operand::Literal("0".into()))
                }
            }
            Instruction::StringLower { realloc: None } => todo!("implement instruction: {inst:?}"),
            Instruction::StringLower {
                realloc: Some(realloc_name),
            } => {
                let tmp = self.tmp();
                let ptr = &format!("ptr{tmp}");
                let len = &format!("len{tmp}");
                let err = &format!("err{tmp}");
                let default = &format!("default{tmp}");
                let memory = &format!("memory{tmp}");
                let realloc = &format!("realloc{tmp}");
                let operand = &operands[0];
                match self.direction {
                    Direction::Export => {
                        quote_in! { self.body =>
                            $['\r']
                            $memory := i.module.Memory()
                            $realloc := i.module.ExportedFunction($(quoted(*realloc_name)))
                            $ptr, $len, $err := writeString(ctx, $operand, $memory, $realloc)
                            $(match &self.result {
                                GoResult::Anon(GoType::ValueOrError(typ)) => {
                                    if $err != nil {
                                        var $default $(typ.as_ref())
                                        return $default, $err
                                    }
                                }
                                GoResult::Anon(GoType::Error) => {
                                    if $err != nil {
                                        return $err
                                    }
                                }
                                GoResult::Anon(_) | GoResult::Empty => {
                                    $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                                    if $err != nil {
                                        panic($err)
                                    }
                                }
                            })
                        }
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            $memory := mod.Memory()
                            $realloc := mod.ExportedFunction($(quoted(*realloc_name)))
                            $ptr, $len, $err := writeString(ctx, $operand, $memory, $realloc)
                            if $err != nil {
                                panic($err)
                            }
                        };
                    }
                }
                results.push(Operand::SingleValue(ptr.into()));
                results.push(Operand::SingleValue(len.into()));
            }
            Instruction::CallWasm { name, .. } => {
                let tmp = self.tmp();
                let raw = &format!("raw{tmp}");
                let ret = &format!("results{tmp}");
                let err = &format!("err{tmp}");
                let default = &format!("default{tmp}");
                // TODO(#17): Wrapping every argument in `uint64` is bad and we should instead be looking
                // at the types and converting with proper guards in place
                quote_in! { self.body =>
                    $['\r']
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            $raw, $err := i.module.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            if $err != nil {
                                var $default $(typ.as_ref())
                                return $default, $err
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            $raw, $err := i.module.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            if $err != nil {
                                return $err
                            }
                        }
                        GoResult::Anon(_) => {
                            $raw, $err := i.module.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if $err != nil {
                                panic($err)
                            }
                        }
                        GoResult::Empty => {
                            _, $err := i.module.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if $err != nil {
                                panic($err)
                            }
                        }
                    })

                    $(if self.result.needs_cleanup() {
                        $(comment(&[
                            "The cleanup via `cabi_post_*` cleans up the memory in the guest. By",
                            "deferring this, we ensure that no memory is corrupted before the function",
                            "is done accessing it."
                        ]))
                        defer func() {
                            if _, err := i.module.ExportedFunction($(quoted(format!("cabi_post_{name}")))).Call(ctx, $raw...); err != nil {
                                $(comment(&[
                                    "If we get an error during cleanup, something really bad is",
                                    "going on, so we panic. Also, you can't return the error from",
                                    "the `defer`"
                                ]))
                                panic($errors_new("failed to cleanup"))
                            }
                        }()
                    })

                    $(match &self.result {
                        GoResult::Anon(_) => $ret := $raw[0],
                        GoResult::Empty => (),
                    })
                };
                match self.result {
                    GoResult::Empty => (),
                    GoResult::Anon(_) => results.push(Operand::SingleValue(ret.into())),
                }
            }
            Instruction::I32Load8U { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadByte(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $errors_new("failed to read byte from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $errors_new("failed to read byte from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($errors_new("failed to read byte from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32FromBool => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    var $(&value) uint32
                    if $operand {
                        $(&value) = 1
                    } else {
                        $(&value) = 0
                    }
                }
                results.push(Operand::SingleValue(value))
            }
            Instruction::BoolFromI32 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := $operand != 0
                }
                results.push(Operand::SingleValue(value))
            }
            Instruction::I32FromU32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $(&self.go_imports.wazero_api_encode_u32)($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::U32FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $(&self.go_imports.wazero_api_decode_u32)($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::PointerLoad { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let ptr = &format!("ptr{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $ptr, $ok := i.module.Memory().ReadUint32Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $errors_new("failed to read pointer from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $errors_new("failed to read pointer from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($errors_new("failed to read pointer from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(ptr.into()));
            }
            Instruction::LengthLoad { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let len = &format!("len{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $len, $ok := i.module.Memory().ReadUint32Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $errors_new("failed to read length from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $errors_new("failed to read length from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($errors_new("failed to read length from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(len.into()));
            }
            Instruction::I32Load { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint32Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $errors_new("failed to read i32 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $errors_new("failed to read i32 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($errors_new("failed to read i32 from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::StringLift => {
                let tmp = self.tmp();
                let buf = &format!("buf{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let str = &format!("str{tmp}");
                let ptr = &operands[0];
                let len = &operands[1];
                match self.direction {
                    Direction::Export { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            $buf, $ok := i.module.Memory().Read($ptr, $len)
                            $(match &self.result {
                                GoResult::Anon(GoType::ValueOrError(typ)) => {
                                    if !$ok {
                                        var $default $(typ.as_ref())
                                        return $default, $errors_new("failed to read bytes from memory")
                                    }
                                }
                                GoResult::Anon(GoType::Error) => {
                                    if !$ok {
                                        return $errors_new("failed to read bytes from memory")
                                    }
                                }
                                GoResult::Anon(_) | GoResult::Empty => {
                                    $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                                    if !$ok {
                                        panic($errors_new("failed to read bytes from memory"))
                                    }
                                }
                            })
                            $str := string($buf)
                        };
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            $buf, $ok := mod.Memory().Read($ptr, $len)
                            if !$ok {
                                panic($errors_new("failed to read bytes from memory"))
                            }
                            $str := string($buf)
                        };
                    }
                }
                results.push(Operand::SingleValue(str.into()));
            }
            Instruction::ResultLift {
                result:
                    Result_ {
                        ok: Some(typ),
                        err: Some(Type::String),
                    },
                ..
            } => {
                let (err_block, err_results) = self.pop_block();
                assert_eq!(err_results.len(), 1);
                let err_op = &err_results[0];

                let (ok_block, ok_results) = self.pop_block();
                assert_eq!(ok_results.len(), 1);
                let ok_op = &ok_results[0];

                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let err = &format!("err{tmp}");
                let typ = resolve_type(typ, resolve);
                let tag = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    var $value $typ
                    var $err error
                    switch $tag {
                    case 0:
                        $ok_block
                        $value = $ok_op
                    case 1:
                        $err_block
                        $err = $errors_new($err_op)
                    default:
                        $err = $errors_new("invalid variant discriminant for expected")
                    }
                };

                results.push(Operand::MultiValue((value.into(), err.into())));
            }
            Instruction::ResultLift {
                result:
                    Result_ {
                        ok: None,
                        err: Some(Type::String),
                    },
                ..
            } => {
                let (err_block, err_results) = self.pop_block();
                assert_eq!(err_results.len(), 1);
                let err_op = &err_results[0];

                let (ok_block, ok_results) = self.pop_block();
                assert_eq!(ok_results.len(), 0);

                let tmp = self.tmp();
                let err = &format!("err{tmp}");
                let tag = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    var $err error
                    switch $tag {
                    case 0:
                        $ok_block
                    case 1:
                        $err_block
                        $err = $errors_new($err_op)
                    default:
                        $err = $errors_new("invalid variant discriminant for expected")
                    }
                };

                results.push(Operand::SingleValue(err.into()));
            }
            Instruction::ResultLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::Return { amt, .. } => {
                if *amt != 0 {
                    let operand = &operands[0];
                    quote_in! { self.body =>
                        $['\r']
                        return $operand
                    };
                }
            }
            Instruction::CallInterface { func, .. } => {
                let ident = GoIdentifier::public(&func.name);
                let tmp = self.tmp();
                let args = quote!($(for op in operands.iter() join (, ) => $op));
                let returns = match &func.result {
                    None => GoType::Nothing,
                    Some(typ) => resolve_type(typ, resolve),
                };
                let value = &format!("value{tmp}");
                let err = &format!("err{tmp}");
                let ok = &format!("ok{tmp}");
                match self.direction {
                    Direction::Export { .. } => todo!("TODO(#10): handle export direction"),
                    Direction::Import { param_name, .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            $(match returns {
                                GoType::Nothing => $param_name.$ident(ctx, $args),
                                GoType::Bool | GoType::Uint32 | GoType::Interface | GoType::String | GoType::UserDefined(_) => $value := $param_name.$ident(ctx, $args),
                                GoType::Error => $err := $param_name.$ident(ctx, $args),
                                GoType::ValueOrError(_) => {
                                    $value, $err := $param_name.$ident(ctx, $args)
                                }
                                GoType::ValueOrOk(_) => {
                                    $value, $ok := $param_name.$ident(ctx, $args)
                                }
                                _ => $(comment(&["TODO(#9): handle return type"]))
                            })
                        }
                    }
                }
                match returns {
                    GoType::Nothing => (),
                    GoType::Bool
                    | GoType::Uint32
                    | GoType::Interface
                    | GoType::UserDefined(_)
                    | GoType::String => {
                        results.push(Operand::SingleValue(value.into()));
                    }
                    GoType::Error => {
                        results.push(Operand::SingleValue(err.into()));
                    }
                    GoType::ValueOrError(_) => {
                        results.push(Operand::MultiValue((value.into(), err.into())));
                    }
                    GoType::ValueOrOk(_) => {
                        results.push(Operand::MultiValue((value.into(), ok.into())))
                    }
                    _ => todo!("TODO(#9): handle return type - {returns:?}"),
                }
            }
            Instruction::VariantPayloadName => {
                results.push(Operand::SingleValue("variantPayload".into()));
            }
            Instruction::I32Const { val } => results.push(Operand::Literal(val.to_string())),
            Instruction::I32Store8 { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tag = &operands[0];
                let ptr = &operands[1];
                if let Operand::Literal(byte) = tag {
                    match &self.direction {
                        Direction::Export => {
                            quote_in! { self.body =>
                                $['\r']
                                i.module.Memory().WriteByte($ptr+$offset, $byte)
                            }
                        }
                        Direction::Import { .. } => {
                            quote_in! { self.body =>
                                $['\r']
                                mod.Memory().WriteByte($ptr+$offset, $byte)
                            }
                        }
                    }
                } else {
                    let tmp = self.tmp();
                    let byte = format!("byte{tmp}");
                    match &self.direction {
                        Direction::Export => {
                            quote_in! { self.body =>
                                $['\r']
                                var $(&byte) uint8
                                switch $tag {
                                case 0:
                                    $(&byte) = 0
                                case 1:
                                    $(&byte) = 1
                                default:
                                    $(comment(["TODO(#8): Return an error if the return type allows it"]))
                                    panic($errors_new("invalid int8 value encountered"))
                                }
                                i.module.Memory().WriteByte($ptr+$offset, $byte)
                            }
                        }
                        Direction::Import { .. } => {
                            quote_in! { self.body =>
                                $['\r']
                                var $(&byte) uint8
                                switch $tag {
                                case 0:
                                    $(&byte) = 0
                                case 1:
                                    $(&byte) = 1
                                default:
                                    panic($errors_new("invalid int8 value encountered"))
                                }
                                mod.Memory().WriteByte($ptr+$offset, $byte)
                            }
                        }
                    }
                }
            }
            Instruction::I32Store { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tag = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export => {
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint32Le($ptr+$offset, $tag)
                        }
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint32Le($ptr+$offset, $tag)
                        }
                    }
                }
            }
            Instruction::LengthStore { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let len = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export => {
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint32Le($ptr+$offset, uint32($len))
                        }
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint32Le($ptr+$offset, uint32($len))
                        }
                    }
                }
            }
            Instruction::PointerStore { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let value = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export => {
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint32Le($ptr+$offset, uint32($value))
                        }
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint32Le($ptr+$offset, uint32($value))
                        }
                    }
                }
            }
            Instruction::ResultLower {
                result:
                    Result_ {
                        ok: Some(_),
                        err: Some(Type::String),
                    },
                ..
            } => {
                let (err_block, _) = self.pop_block();
                let (ok_block, _) = self.pop_block();
                let operand = &operands[0];
                let (ok, err) = match operand {
                    Operand::Literal(_) => {
                        panic!("impossible: expected Operand::MultiValue but got Operand::Literal")
                    }
                    Operand::SingleValue(_) => panic!(
                        "impossible: expected Operand::MultiValue but got Operand::SingleValue"
                    ),
                    Operand::MultiValue(bindings) => bindings,
                };
                quote_in! { self.body =>
                    $['\r']
                    if $err != nil {
                        variantPayload := $err.Error()
                        $err_block
                    } else {
                        variantPayload := $ok
                        $ok_block
                    }
                };
            }
            Instruction::ResultLower {
                result:
                    Result_ {
                        ok: None,
                        err: Some(Type::String),
                    },
                ..
            } => {
                let (err, _) = self.pop_block();
                let (ok, _) = self.pop_block();
                let err_result = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    if $err_result != nil {
                        variantPayload := $err_result.Error()
                        $err
                    } else {
                        $ok
                    }
                };
            }
            Instruction::ResultLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::OptionLift { payload, .. } => {
                let (some, some_results) = self.blocks.pop().unwrap();
                let (none, _) = self.blocks.pop().unwrap();
                let some_result = &some_results[0];

                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let ok = &format!("ok{tmp}");
                let typ = resolve_type(payload, resolve);
                let op = &operands[0];

                quote_in! { self.body =>
                    $['\r']
                    var $result $typ
                    var $ok bool
                    if $op == 0 {
                        $none
                        $ok = false
                    } else {
                        $some
                        $ok = true
                        $result = $some_result
                    }
                };

                results.push(Operand::MultiValue((result.into(), ok.into())));
            }
            Instruction::OptionLower {
                payload: Type::String,
                results: result_types,
                ..
            } => {
                let (mut some_block, some_results) = self.pop_block();
                let (mut none_block, none_results) = self.pop_block();

                let tmp = self.tmp();

                let mut vars: Tokens<Go> = Tokens::new();
                for i in 0..result_types.len() {
                    let variant = &format!("variant{tmp}_{i}");
                    let typ = resolve_wasm_type(&result_types[i]);
                    results.push(Operand::SingleValue(variant.into()));

                    quote_in! { vars =>
                        $['\r']
                        var $variant $typ
                    }

                    let some_result = &some_results[i];
                    let none_result = &none_results[i];
                    quote_in! { some_block =>
                        $['\r']
                        $variant = $some_result
                    };
                    quote_in! { none_block =>
                        $['\r']
                        $variant = $none_result
                    };
                }

                let operand = &operands[0];
                match operand {
                    Operand::Literal(_) => {
                        panic!("impossible: expected Operand::MultiValue but got Operand::Literal")
                    }
                    // TODO(#7): This is a weird hack to implement `option<string>`
                    // as arguments that currently only works for strings
                    // because it checks the empty string as the zero value to
                    // consider it None
                    Operand::SingleValue(value) => {
                        quote_in! { self.body =>
                            $['\r']
                            $vars
                            if $value == "" {
                                $none_block
                            } else {
                                variantPayload := $value
                                $some_block
                            }
                        };
                    }
                    Operand::MultiValue((value, ok)) => {
                        quote_in! { self.body =>
                            $['\r']
                            if $ok {
                                variantPayload := $value
                                $some_block
                            } else {
                                $none_block
                            }
                        };
                    }
                };
            }
            Instruction::OptionLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::RecordLower { record, .. } => {
                let tmp = self.tmp();
                let operand = &operands[0];
                for field in record.fields.iter() {
                    let struct_field = GoIdentifier::public(&field.name);
                    let var = &GoIdentifier::local(format!("{}{tmp}", &field.name));
                    quote_in! { self.body =>
                        $['\r']
                        $var := $operand.$struct_field
                    }
                    results.push(Operand::SingleValue(var.into()))
                }
            }
            Instruction::RecordLift { record, name, .. } => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let fields = record
                    .fields
                    .iter()
                    .zip(operands)
                    .map(|(field, op)| (GoIdentifier::public(&field.name), op));

                quote_in! {self.body =>
                    $['\r']
                    $value := $(GoIdentifier::public(*name)){
                        $(for (name, op) in fields join ($['\r']) => $name: $op,)
                    }
                };
                results.push(Operand::SingleValue(value.into()))
            }
            Instruction::IterElem { .. } => results.push(Operand::SingleValue(iter_element.into())),
            Instruction::IterBasePointer => results.push(Operand::SingleValue(iter_base.into())),
            Instruction::ListLower { realloc: None, .. } => {
                todo!("implement instruction: {inst:?}")
            }
            Instruction::ListLower {
                element,
                realloc: Some(realloc_name),
            } => {
                let (body, _) = self.pop_block();
                let tmp = self.tmp();
                let vec = &format!("vec{tmp}");
                let result = &format!("result{tmp}");
                let err = &format!("err{tmp}");
                let default = &format!("default{tmp}");
                let ptr = &format!("ptr{tmp}");
                let len = &format!("len{tmp}");
                let operand = &operands[0];
                let size = self.sizes.size(element).size_wasm32();
                let align = self.sizes.align(element).align_wasm32();

                quote_in! { self.body =>
                    $['\r']
                    $vec := $operand
                    $len := uint64(len($vec))
                    $result, $err := i.module.ExportedFunction($(quoted(*realloc_name))).Call(ctx, 0, 0, $align, $len * $size)
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if $err != nil {
                                var $default $(typ.as_ref())
                                return $default, $err
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if $err != nil {
                                return $err
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if $err != nil {
                                panic($err)
                            }
                        }
                    })
                    $ptr := $result[0]
                    for idx := uint64(0); idx < $len; idx++ {
                        $iter_element := $vec[idx]
                        $iter_base := uint32($ptr + uint64(idx) * uint64($size))
                        $body
                    }
                };
                results.push(Operand::SingleValue(ptr.into()));
                results.push(Operand::SingleValue(len.into()));
            }
            Instruction::ListLift { element, .. } => {
                let (body, body_results) = self.pop_block();
                let tmp = self.tmp();
                let size = self.sizes.size(element).size_wasm32();
                let len = &format!("len{tmp}");
                let base = &format!("base{tmp}");
                let result = &format!("result{tmp}");
                let idx = &format!("idx{tmp}");

                let base_operand = &operands[0];
                let len_operand = &operands[1];
                let body_result = &body_results[0];

                let typ = resolve_type(element, resolve);

                quote_in! { self.body =>
                    $['\r']
                    $base := $base_operand
                    $len := $len_operand
                    $result := make([]$typ, $len)
                    for $idx := uint32(0); $idx < $len; $idx++ {
                        base := $base + $idx * $size
                        $body
                        $result[$idx] = $body_result
                    }
                }
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::VariantLower {
                variant,
                results: result_types,
                ..
            } => {
                let blocks = self
                    .blocks
                    .drain(self.blocks.len() - variant.cases.len()..)
                    .collect::<Vec<_>>();
                let tmp = self.tmp();
                let value = &operands[0];
                let default = &format!("default{tmp}");

                for (i, typ) in result_types.iter().enumerate() {
                    let variant_item = &format!("variant{tmp}_{i}");
                    let typ = resolve_wasm_type(typ);
                    quote_in! { self.body =>
                        $['\r']
                        var $variant_item $typ
                    }
                    results.push(Operand::SingleValue(variant_item.into()));
                }

                let mut cases: Tokens<Go> = Tokens::new();
                for (case, (block, block_results)) in variant.cases.iter().zip(blocks) {
                    let mut assignments: Tokens<Go> = Tokens::new();
                    for (i, result) in block_results.iter().enumerate() {
                        let variant_item = &format!("variant{tmp}_{i}");
                        quote_in! { assignments =>
                            $['\r']
                            $variant_item = $result
                        };
                    }

                    let name = GoIdentifier::public(case.name.clone());
                    quote_in! { cases =>
                        $['\r']
                        case $name:
                            $block
                            $assignments
                    }
                }

                quote_in! { self.body =>
                    $['\r']
                    switch variantPayload := $value.(type) {
                        $cases
                        default:
                            $(match &self.result {
                                GoResult::Anon(GoType::ValueOrError(typ)) => {
                                    var $default $(typ.as_ref())
                                    return $default, $errors_new("invalid variant type provided")
                                }
                                GoResult::Anon(GoType::Error) => {
                                    return $errors_new("invalid variant type provided")
                                }
                                GoResult::Anon(_) | GoResult::Empty => {
                                    $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                                    panic($errors_new("invalid variant type provided"))
                                }
                            })
                    }
                }
            }
            Instruction::EnumLower { enum_, .. } => {
                let value = &operands[0];
                let tmp = self.tmp();
                let enum_tmp = &format!("enum{tmp}");

                let mut cases: Tokens<Go> = Tokens::new();
                for (i, case) in enum_.cases.iter().enumerate() {
                    let case_name = GoIdentifier::public(case.name.clone());
                    quote_in! { cases =>
                        $['\r']
                        case $case_name:
                            $enum_tmp = $i
                    };
                }

                quote_in! { self.body =>
                    $['\r']
                    var $enum_tmp uint32
                    switch $value {
                    $cases
                    default:
                        panic($errors_new("invalid enum type provided"))
                    }
                };

                results.push(Operand::SingleValue(enum_tmp.to_string()));
            }
            Instruction::Bitcasts { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Load8S { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Load16U { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Load16S { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I64Load { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::F32Load { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::F64Load { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Store16 { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I64Store { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::F32Store { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::F64Store { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32FromChar => todo!("implement instruction: {inst:?}"),
            Instruction::I64FromU64 => todo!("implement instruction: {inst:?}"),
            Instruction::I64FromS64 => todo!("implement instruction: {inst:?}"),
            Instruction::I32FromS32 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := $(&self.go_imports.wazero_api_encode_i32)($operand)
                }
                results.push(Operand::SingleValue(value))
            }
            // All of these values should fit in Go's `int32` type which allows a safe cast
            Instruction::I32FromU16
            | Instruction::I32FromS16
            | Instruction::I32FromU8
            | Instruction::I32FromS8 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := $(&self.go_imports.wazero_api_encode_i32)(int32($operand))
                }
                results.push(Operand::SingleValue(value))
            }
            Instruction::CoreF32FromF32 => todo!("implement instruction: {inst:?}"),
            Instruction::CoreF64FromF64 => todo!("implement instruction: {inst:?}"),
            // TODO: Validate the Go cast truncates the upper bits in the I32
            Instruction::S8FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := int8($(&self.go_imports.wazero_api_decode_i32)($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            // TODO: Validate the Go cast truncates the upper bits in the I32
            Instruction::U8FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := uint8($(&self.go_imports.wazero_api_decode_u32)($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            // TODO: Validate the Go cast truncates the upper bits in the I32
            Instruction::S16FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := int16($(&self.go_imports.wazero_api_decode_i32)($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            // TODO: Validate the Go cast truncates the upper bits in the I32
            Instruction::U16FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := uint16($(&self.go_imports.wazero_api_decode_u32)($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::S32FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $(&self.go_imports.wazero_api_decode_i32)($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::S64FromI64 => todo!("implement instruction: {inst:?}"),
            Instruction::U64FromI64 => todo!("implement instruction: {inst:?}"),
            Instruction::CharFromI32 => todo!("implement instruction: {inst:?}"),
            Instruction::F32FromCoreF32 => todo!("implement instruction: {inst:?}"),
            Instruction::F64FromCoreF64 => todo!("implement instruction: {inst:?}"),
            Instruction::TupleLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::TupleLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FlagsLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FlagsLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::VariantLift { .. } => {
                todo!("implement instruction: {inst:?}")
            }
            Instruction::EnumLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::Malloc { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::HandleLower { .. } | Instruction::HandleLift { .. } => {
                todo!("implement resources: {inst:?}")
            }
            Instruction::ListCanonLower { .. } | Instruction::ListCanonLift { .. } => {
                unimplemented!("gravity doesn't represent lists as Canonical")
            }
            Instruction::GuestDeallocateString
            | Instruction::GuestDeallocate { .. }
            | Instruction::GuestDeallocateList { .. }
            | Instruction::GuestDeallocateVariant { .. } => {
                unimplemented!("gravity doesn't generate the Guest code")
            }
            Instruction::FutureLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FutureLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::StreamLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::StreamLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::ErrorContextLower => todo!("implement instruction: {inst:?}"),
            Instruction::ErrorContextLift => todo!("implement instruction: {inst:?}"),
            Instruction::AsyncTaskReturn { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::DropHandle { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::Flush { amt } => {
                for op in operands.iter().take(*amt) {
                    results.push(op.clone());
                }
            }
        }
    }

    fn return_pointer(&mut self, _size: ArchitectureSize, _align: Alignment) -> Self::Operand {
        unimplemented!("return_pointer")
    }

    fn push_block(&mut self) {
        let prev = mem::replace(&mut self.body, Tokens::new());
        self.block_storage.push(prev);
    }

    fn finish_block(&mut self, operands: &mut Vec<Self::Operand>) {
        let to_restore = self.block_storage.pop().expect("should have body");
        let src = mem::replace(&mut self.body, to_restore);
        self.blocks.push((src, mem::take(operands)));
    }

    fn sizes(&self) -> &SizeAlign {
        self.sizes
    }

    fn is_list_canonical(&self, _resolve: &Resolve, _element: &Type) -> bool {
        // Go slices are never directly in the Wasm Memory, so they are never "canonical"
        false
    }
}
