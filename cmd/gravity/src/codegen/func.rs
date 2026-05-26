use std::mem;

use genco::prelude::*;
use wit_bindgen_core::{
    abi::{Bindgen, Instruction},
    wit_parser::{Alignment, ArchitectureSize, Resolve, Result_, SizeAlign, Type},
};

use crate::{
    go::{
        comment,
        imports::{
            ERRORS_NEW, WAZERO_API_DECODE_F32, WAZERO_API_DECODE_F64, WAZERO_API_DECODE_I32,
            WAZERO_API_DECODE_U32, WAZERO_API_ENCODE_F32, WAZERO_API_ENCODE_F64,
            WAZERO_API_ENCODE_I32,
        },
        GoIdentifier, GoResult, GoType, Operand,
    },
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
}

impl<'a> Func<'a> {
    /// Create a new exported function.
    #[allow(dead_code, reason = "halfway through refactor of func bindings")]
    pub fn export(result: GoResult, sizes: &'a SizeAlign) -> Self {
        Self {
            direction: Direction::Export,
            args: Vec::new(),
            result,
            tmp: 0,
            body: Tokens::new(),
            block_storage: Vec::new(),
            blocks: Vec::new(),
            sizes,
        }
    }

    /// Create a new exported function.
    pub fn import(param_name: &'a GoIdentifier, result: GoResult, sizes: &'a SizeAlign) -> Self {
        Self {
            direction: Direction::Import { param_name },
            args: Vec::new(),
            result,
            tmp: 0,
            body: Tokens::new(),
            block_storage: Vec::new(),
            blocks: Vec::new(),
            sizes,
        }
    }

    fn tmp(&mut self) -> usize {
        let ret = self.tmp;
        self.tmp += 1;
        ret
    }

    /// The Go expression that resolves to the wasm `api.Module` in the
    /// current direction. Exports live on a Go-side instance struct
    /// (`i.module`); imports receive the module as a `mod` parameter from
    /// wazero's host-function builder.
    fn module_handle(&self) -> &'static str {
        match self.direction {
            Direction::Export => "i.module",
            Direction::Import { .. } => "mod",
        }
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
        let iter_element = "e";
        let iter_base = "base";
        // Hoist to avoid borrow-checker conflict with `quote_in! { self.body => ... }`.
        let module_handle = self.module_handle();

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
                            $raw, $err := $module_handle.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            if $err != nil {
                                var $default $(typ.as_ref())
                                return $default, $err
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            $raw, $err := $module_handle.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            if $err != nil {
                                return $err
                            }
                        }
                        GoResult::Anon(_) => {
                            $raw, $err := $module_handle.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if $err != nil {
                                panic($err)
                            }
                        }
                        GoResult::Empty => {
                            _, $err := $module_handle.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
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
                            if postFn := $module_handle.ExportedFunction($(quoted(format!("cabi_post_{name}")))); postFn != nil {
                                if _, err := postFn.Call(ctx, $raw...); err != nil {
                                    $(comment(&[
                                        "If we get an error during cleanup, something really bad is",
                                        "going on, so we panic. Also, you can't return the error from",
                                        "the `defer`"
                                    ]))
                                    panic($ERRORS_NEW("failed to cleanup"))
                                }
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
                    $value, $ok := $module_handle.Memory().ReadByte(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read byte from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read byte from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read byte from memory"))
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
                // I32FromU32 is a no-op reinterpretation (same 32-bit value,
                // different signedness). Use uint32() identity cast in both
                // directions — api.EncodeU32 returns uint64 which causes type
                // mismatches when assigned to uint32 variables (e.g. VariantLower).
                quote_in! { self.body =>
                    $['\r']
                    $result := uint32($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::U32FromI32 => {
                // U32FromI32 is a no-op reinterpretation (same 32-bit value,
                // different signedness). Use uint32() identity cast —
                // api.DecodeU32(uint64(...)) is a needless round-trip through
                // uint64 when the operand is already uint32.
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := uint32($operand)
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
                    $ptr, $ok := $module_handle.Memory().ReadUint32Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read pointer from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read pointer from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read pointer from memory"))
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
                    $len, $ok := $module_handle.Memory().ReadUint32Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read length from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read length from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read length from memory"))
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
                    $value, $ok := $module_handle.Memory().ReadUint32Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read i32 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read i32 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read i32 from memory"))
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
                                        return $default, $ERRORS_NEW("failed to read bytes from memory")
                                    }
                                }
                                GoResult::Anon(GoType::Error) => {
                                    if !$ok {
                                        return $ERRORS_NEW("failed to read bytes from memory")
                                    }
                                }
                                GoResult::Anon(_) | GoResult::Empty => {
                                    $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                                    if !$ok {
                                        panic($ERRORS_NEW("failed to read bytes from memory"))
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
                                panic($ERRORS_NEW("failed to read bytes from memory"))
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
                        $err = $ERRORS_NEW($err_op)
                    default:
                        $err = $ERRORS_NEW("invalid variant discriminant for expected")
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
                        $err = $ERRORS_NEW($err_op)
                    default:
                        $err = $ERRORS_NEW("invalid variant discriminant for expected")
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
                // `ValueOrError`/`ValueOrOk` are the only two Go shapes that
                // come back as multiple return values — everything else (a
                // primitive, string, slice, pointer-to-T, interface, or a
                // user-defined record/enum/alias) lands in a single
                // identifier that subsequent ABI instructions will lower.
                match self.direction {
                    Direction::Export { .. } => todo!("TODO(#10): handle export direction"),
                    Direction::Import { param_name, .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            $(match returns {
                                GoType::Nothing => $param_name.$ident(ctx, $args),
                                GoType::Error => $err := $param_name.$ident(ctx, $args),
                                GoType::ValueOrError(_) => {
                                    $value, $err := $param_name.$ident(ctx, $args)
                                }
                                GoType::ValueOrOk(_) => {
                                    $value, $ok := $param_name.$ident(ctx, $args)
                                }
                                _ => $value := $param_name.$ident(ctx, $args),
                            })
                        }
                    }
                }
                match returns {
                    GoType::Nothing => (),
                    GoType::Error => {
                        results.push(Operand::SingleValue(err.into()));
                    }
                    GoType::ValueOrError(_) => {
                        results.push(Operand::MultiValue((value.into(), err.into())));
                    }
                    GoType::ValueOrOk(_) => {
                        results.push(Operand::MultiValue((value.into(), ok.into())))
                    }
                    _ => {
                        results.push(Operand::SingleValue(value.into()));
                    }
                }
            }
            Instruction::VariantPayloadName => {
                // `VariantLower` and `OptionLower` both bind `variantPayload`
                // to the case payload before invoking the per-case block.
                results.push(Operand::SingleValue("variantPayload".into()));
            }
            Instruction::I32Const { val } => results.push(Operand::Literal(val.to_string())),
            Instruction::I32Store8 { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tag = &operands[0];
                let ptr = &operands[1];
                if let Operand::Literal(byte) = tag {
                    quote_in! { self.body =>
                        $['\r']
                        $module_handle.Memory().WriteByte($ptr+$offset, $byte)
                    }
                } else {
                    let tmp = self.tmp();
                    let byte = format!("byte{tmp}");
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
                            panic($ERRORS_NEW("invalid int8 value encountered"))
                        }
                        $module_handle.Memory().WriteByte($ptr+$offset, $byte)
                    }
                }
            }
            Instruction::I32Store { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tag = &operands[0];
                let ptr = &operands[1];
                quote_in! { self.body =>
                    $['\r']
                    $module_handle.Memory().WriteUint32Le($ptr+$offset, $tag)
                }
            }
            Instruction::LengthStore { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let len = &operands[0];
                let ptr = &operands[1];
                quote_in! { self.body =>
                    $['\r']
                    $module_handle.Memory().WriteUint32Le($ptr+$offset, uint32($len))
                }
            }
            Instruction::PointerStore { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let value = &operands[0];
                let ptr = &operands[1];
                quote_in! { self.body =>
                    $['\r']
                    $module_handle.Memory().WriteUint32Le($ptr+$offset, uint32($value))
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
                let (_none, _) = self.blocks.pop().unwrap();
                let some_result = &some_results[0];

                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let inner_typ = resolve_type(payload, resolve);
                let op = &operands[0];

                quote_in! { self.body =>
                    $['\r']
                    var $result *$inner_typ
                    if $op != 0 {
                        $some
                        someValue$tmp := $some_result
                        $result = &someValue$tmp
                    }
                };

                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::OptionLower {
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

                let Operand::SingleValue(value) = &operands[0] else {
                    unreachable!("OptionLower expects a single `*T` operand");
                };
                quote_in! { self.body =>
                    $['\r']
                    $vars
                    if $value == nil {
                        $none_block
                    } else {
                        variantPayload := *$value
                        $some_block
                    }
                };
            }
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
                    $result, $err := $module_handle.ExportedFunction($(quoted(*realloc_name))).Call(ctx, 0, 0, $align, $len * $size)
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
                ty,
                results: result_types,
                ..
            } => {
                let name = crate::qualified_type_name(*ty, resolve);
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

                // Collapse the type-switch when every case is `DirectRecord`:
                // the case-struct binder IS the payload, so we can bind
                // `variantPayload` once in the switch header instead of
                // re-aliasing it per arm. Mixed variants need a separate
                // binder so `Wrapped` cases can unwrap via `.Value`.
                let all_direct = variant.cases.iter().all(|case| {
                    matches!(
                        crate::case_dispatch_kind(case, resolve),
                        crate::CaseDispatchKind::DirectRecord
                    )
                });
                let case_binder = if all_direct {
                    "variantPayload".to_string()
                } else {
                    format!("case{tmp}")
                };
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

                    let case_type = GoIdentifier::public(crate::case_dispatch_name(
                        &name, case, resolve,
                    ));
                    let payload_intro = if all_direct {
                        quote!()
                    } else {
                        match crate::case_dispatch_kind(case, resolve) {
                            crate::CaseDispatchKind::DirectRecord => {
                                quote!(variantPayload := $(&case_binder)$['\r'])
                            }
                            crate::CaseDispatchKind::Wrapped if case.ty.is_some() => {
                                quote!(variantPayload := $(&case_binder).Value$['\r'])
                            }
                            crate::CaseDispatchKind::Wrapped => {
                                quote!(_ = $(&case_binder)$['\r'])
                            }
                        }
                    };
                    quote_in! { cases =>
                        $['\r']
                        case $case_type:
                            $payload_intro
                            $block
                            $assignments
                    }
                }

                quote_in! { self.body =>
                    $['\r']
                    switch $(&case_binder) := $value.(type) {
                        $cases
                        default:
                            $(match &self.result {
                                GoResult::Anon(GoType::ValueOrError(typ)) => {
                                    var $default $(typ.as_ref())
                                    return $default, $ERRORS_NEW("invalid variant type provided")
                                }
                                GoResult::Anon(GoType::Error) => {
                                    return $ERRORS_NEW("invalid variant type provided")
                                }
                                GoResult::Anon(_) | GoResult::Empty => {
                                    $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                                    panic($ERRORS_NEW("invalid variant type provided"))
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
                        panic($ERRORS_NEW("invalid enum type provided"))
                    }
                };

                results.push(Operand::SingleValue(enum_tmp.to_string()));
            }
            Instruction::Bitcasts { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Load8S { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Load16U { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Load16S { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I64Load { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := $module_handle.Memory().ReadUint64Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read i64 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read i64 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read i64 from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::F32Load { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := $module_handle.Memory().ReadUint64Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read f32 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read f32 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read f32 from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::F64Load { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := $module_handle.Memory().ReadUint64Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read f64 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read f64 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read f64 from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32Store16 { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I64Store { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::F32Store { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tag = &operands[0];
                let ptr = &operands[1];
                quote_in! { self.body =>
                    $['\r']
                    $module_handle.Memory().WriteUint64Le($ptr+$offset, $tag)
                }
            }
            Instruction::F64Store { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tag = &operands[0];
                let ptr = &operands[1];
                quote_in! { self.body =>
                    $['\r']
                    $module_handle.Memory().WriteUint64Le($ptr+$offset, $tag)
                }
            }
            Instruction::I32FromChar => todo!("implement instruction: {inst:?}"),
            Instruction::I64FromU64 => {
                // I64FromU64 is a no-op reinterpretation (same 64-bit value,
                // different signedness). Use uint64() identity cast — int64()
                // returns int64 which causes type mismatches when assigned to
                // uint64 variables (e.g. VariantLower).
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := uint64($operand)
                }
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I64FromS64 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := $operand
                }
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32FromS32 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := $WAZERO_API_ENCODE_I32($operand)
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
                    $(&value) := $WAZERO_API_ENCODE_I32(int32($operand))
                }
                results.push(Operand::SingleValue(value))
            }
            Instruction::CoreF32FromF32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $WAZERO_API_ENCODE_F32($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::CoreF64FromF64 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $WAZERO_API_ENCODE_F64($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            // TODO: Validate the Go cast truncates the upper bits in the I32
            Instruction::S8FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := int8($WAZERO_API_DECODE_I32($operand))
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
                    $result := uint8($WAZERO_API_DECODE_U32($operand))
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
                    $result := int16($WAZERO_API_DECODE_I32($operand))
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
                    $result := uint16($WAZERO_API_DECODE_U32($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::S32FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $WAZERO_API_DECODE_I32($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::S64FromI64 => todo!("implement instruction: {inst:?}"),
            Instruction::U64FromI64 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := uint64($operand)
                }
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::CharFromI32 => todo!("implement instruction: {inst:?}"),
            Instruction::F32FromCoreF32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $WAZERO_API_DECODE_F32($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::F64FromCoreF64 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $WAZERO_API_DECODE_F64($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::TupleLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::TupleLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FlagsLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FlagsLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::VariantLift { variant, ty, .. } => {
                let name = crate::qualified_type_name(*ty, resolve);
                let blocks = self
                    .blocks
                    .drain(self.blocks.len() - variant.cases.len()..)
                    .collect::<Vec<_>>();
                let discriminant = &operands[0];
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let variant_type = GoType::UserDefined(name.clone());

                let mut cases: Tokens<Go> = Tokens::new();
                for (i, (case, (block, block_results))) in
                    variant.cases.iter().zip(blocks).enumerate()
                {
                    let case_type =
                        GoIdentifier::public(crate::case_dispatch_name(&name, case, resolve));
                    let payload = block_results.first();
                    let construction = match crate::case_dispatch_kind(case, resolve) {
                        crate::CaseDispatchKind::DirectRecord => {
                            let payload = payload.expect("DirectRecord case has a payload");
                            quote!($payload)
                        }
                        crate::CaseDispatchKind::Wrapped => match payload {
                            None => quote!($(&case_type){}),
                            Some(payload) => quote!($(&case_type){Value: $payload}),
                        },
                    };
                    quote_in! { cases =>
                        $['\r']
                        case $i:
                            $block
                            $value = $construction
                    };
                }

                let err_msg = format!("\"invalid {name} discriminant\"");
                quote_in! { self.body =>
                    $['\r']
                    var $value $variant_type
                    switch $discriminant {
                    $cases
                    default:
                        $(match &self.result {
                            GoResult::Anon(GoType::ValueOrError(typ)) => {
                                var default0 $(typ.as_ref())
                                return default0, $ERRORS_NEW($(&err_msg))
                            }
                            GoResult::Anon(GoType::Error) => {
                                return $ERRORS_NEW($(&err_msg))
                            }
                            GoResult::Anon(_) | GoResult::Empty => {
                                $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                                panic($ERRORS_NEW($(&err_msg)))
                            }
                        })
                    }
                };

                results.push(Operand::SingleValue(value.to_string()));
            }
            Instruction::EnumLift { enum_, ty, .. } => {
                let name = crate::qualified_type_name(*ty, resolve);
                let discriminant = &operands[0];
                let tmp = self.tmp();
                let enum_value = &format!("enum{tmp}");
                let go_type = GoType::UserDefined(name.clone());

                let mut cases: Tokens<Go> = Tokens::new();
                for (i, case) in enum_.cases.iter().enumerate() {
                    let case_name = GoIdentifier::public(case.name.clone());
                    quote_in! { cases =>
                        $['\r']
                        case $i:
                            $enum_value = $case_name
                    };
                }

                quote_in! { self.body =>
                    $['\r']
                    var $enum_value $go_type
                    switch $discriminant {
                    $cases
                    default:
                        $(match &self.result {
                            GoResult::Anon(GoType::ValueOrError(typ)) => {
                                var default0 $(typ.as_ref())
                                return default0, $ERRORS_NEW($(format!("\"invalid {name} discriminant\"")))
                            }
                            GoResult::Anon(GoType::Error) => {
                                return $ERRORS_NEW($(format!("\"invalid {name} discriminant\"")))
                            }
                            GoResult::Anon(_) | GoResult::Empty => {
                                $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                                panic($ERRORS_NEW($(format!("\"invalid {name} discriminant\""))))
                            }
                        })
                    }
                };

                results.push(Operand::SingleValue(enum_value.to_string()));
            }
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
            | Instruction::GuestDeallocateMap { .. }
            | Instruction::GuestDeallocateVariant { .. } => {
                unimplemented!("gravity doesn't generate the Guest code")
            }
            Instruction::MapLower { .. }
            | Instruction::MapLift { .. }
            | Instruction::IterMapKey { .. }
            | Instruction::IterMapValue { .. } => {
                todo!("implement instruction: {inst:?}")
            }
            Instruction::FutureLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FutureLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::StreamLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::StreamLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::ErrorContextLower => todo!("implement instruction: {inst:?}"),
            Instruction::ErrorContextLift => todo!("implement instruction: {inst:?}"),
            Instruction::AsyncTaskReturn { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::DropHandle { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FixedLengthListLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FixedLengthListLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FixedLengthListLowerToMemory { .. } => {
                todo!("implement instruction: {inst:?}")
            }
            Instruction::FixedLengthListLiftFromMemory { .. } => {
                todo!("implement instruction: {inst:?}")
            }
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
