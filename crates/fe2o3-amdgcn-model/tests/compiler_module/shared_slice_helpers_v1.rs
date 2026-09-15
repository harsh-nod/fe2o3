use super::*;
use fe2o3_amdgcn_model::{
    lower_compiler_module_to_gfx942_xnack_minus_llvm_ir,
    lower_compiler_module_to_gfx950_xnack_minus_llvm_ir,
};
use fe2o3_kernel_ir::{
    ScalarType, gfx942_xnack_minus_target_capability, gfx950_xnack_minus_target_capability,
    verify_module,
};

fn slice() -> Type {
    Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    )
}

fn exact_module(mut module: Module, gfx950: bool) -> Module {
    let target = if gfx950 {
        gfx950_xnack_minus_target_capability()
    } else {
        gfx942_xnack_minus_target_capability()
    };
    module.required_capabilities.insert(target.clone());
    for function in &mut module.functions {
        if function.body.is_some() {
            function.required_capabilities.insert(target.clone());
        }
    }
    for kernel in &mut module.kernels {
        kernel.required_capabilities.insert(target.clone());
    }
    module
}

fn llvm(module: &Module, gfx950: bool) -> Result<String, fe2o3_amdgcn_model::LoweringErrors> {
    if gfx950 {
        lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(module)
    } else {
        lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(module)
    }
}

fn length_call(result: u32, argument: u32, offset: u32) -> Operation {
    Operation::new(
        vec![ValueDef::new(ValueId(result), Type::INDEX)],
        OperationKind::Call {
            callee: FunctionId::new("slice_length"),
            arguments: vec![ValueId(argument), ValueId(offset)],
        },
    )
}

fn forwarding_module() -> Module {
    let length = Function::internal_helper(
        "slice_length",
        Signature::new(vec![slice(), Type::INDEX], vec![Type::INDEX]),
        vec![ValueId(0), ValueId(1)],
        vec![returning_block(
            vec![
                Operation::effect_free(
                    ValueDef::new(
                        ValueId(2),
                        Type::pointer(
                            Type::Scalar(ScalarType::U32),
                            AddressSpace::Global,
                            AccessMode::ReadOnly,
                        ),
                    ),
                    OperationKind::SliceData { slice: ValueId(0) },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(3), Type::INDEX),
                    OperationKind::SliceLength { slice: ValueId(0) },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(4), Type::INDEX),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(3),
                        rhs: ValueId(1),
                    },
                ),
            ],
            vec![ValueId(4)],
        )],
    );
    let parameters = vec![
        slice(),
        Type::Scalar(ScalarType::Bool),
        slice(),
        Type::INDEX,
    ];
    let mut branch = BasicBlock::new(BlockId(0));
    branch.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(0)],
        else_target: BlockId(1),
        else_arguments: vec![ValueId(2)],
    });
    let mut join = BasicBlock::new(BlockId(1));
    join.parameters = vec![ValueDef::new(ValueId(4), slice())];
    join.operations = vec![length_call(5, 4, 3)];
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    let forwarding = Function::internal_helper(
        "slice_forward",
        Signature::new(parameters.clone(), vec![Type::INDEX]),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![branch, join],
    );
    let entry = Function::kernel_entry(
        "slice_entry",
        Signature::new(parameters, vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![returning_block(
            vec![Operation::new(
                vec![ValueDef::new(ValueId(4), Type::INDEX)],
                OperationKind::Call {
                    callee: forwarding.id.clone(),
                    arguments: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
                },
            )],
            vec![],
        )],
    );
    let mut module = Module::new("tests::immutable_slice_helper_llvm");
    module.functions = vec![entry, forwarding, length];
    module.kernels = vec![kernel("slice_kernel", "slice_entry", 64)];
    module
}

#[test]
fn immutable_slice_helper_pairs_preserve_calls_and_duplicate_edge_joins_on_both_targets() {
    for gfx950 in [false, true] {
        let module = exact_module(forwarding_module(), gfx950);
        verify_module(&module).unwrap();
        let text = llvm(&module, gfx950).unwrap();
        assert_eq!(text, llvm(&module, gfx950).unwrap());
        assert!(text.contains("define internal i64 @slice_forward(ptr addrspace(1) %arg0.data, i64 %arg0.len, i1 %arg1, ptr addrspace(1) %arg2.data, i64 %arg2.len, i64 %arg3)"));
        assert!(text.contains("call i64 @slice_forward(ptr addrspace(1) %arg0.data, i64 %arg0.len, i1 %arg1, ptr addrspace(1) %arg2.data, i64 %arg2.len, i64 %arg3)"));
        assert!(text.contains("%v4.data = phi ptr addrspace(1) [ %arg0.data, %edge_bb0_0_bb1 ], [ %arg2.data, %edge_bb0_1_bb1 ]"));
        assert!(text.contains(
            "%v4.len = phi i64 [ %arg0.len, %edge_bb0_0_bb1 ], [ %arg2.len, %edge_bb0_1_bb1 ]"
        ));
        assert!(
            text.contains(
                "call i64 @slice_length(ptr addrspace(1) %v4.data, i64 %v4.len, i64 %arg3)"
            )
        );
        assert!(text.contains("define internal i64 @slice_length(ptr addrspace(1) %arg0.data, i64 %arg0.len, i64 %arg1)"));
        assert!(text.contains("%v2 = getelementptr i8, ptr addrspace(1) %arg0.data, i64 0"));
        assert!(text.contains("%v3 = add i64 %arg0.len, 0"));
        assert!(!text.contains(" = load "));
        assert!(!text.contains("  store "));
    }
}

fn loop_module() -> Module {
    let mut module = forwarding_module();
    let body = module.functions[1].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0)],
    });
    body.blocks[1].operations.clear();
    body.blocks[1].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(4)],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.operations = vec![length_call(5, 4, 3)];
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    body.blocks.push(exit);
    module
}

#[test]
fn immutable_slice_helper_loop_keeps_both_carrier_components_on_the_backedge() {
    for gfx950 in [false, true] {
        let module = exact_module(loop_module(), gfx950);
        verify_module(&module).unwrap();
        let text = llvm(&module, gfx950).unwrap();
        assert!(text.contains(
            "%v4.data = phi ptr addrspace(1) [ %arg0.data, %bb0 ], [ %v4.data, %edge_bb1_0_bb1 ]"
        ));
        assert!(
            text.contains("%v4.len = phi i64 [ %arg0.len, %bb0 ], [ %v4.len, %edge_bb1_0_bb1 ]")
        );
    }
}

#[test]
fn emitted_slice_helper_pairs_and_backedges_pass_llvm_verification() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    for gfx950 in [false, true] {
        for module in [forwarding_module(), loop_module()] {
            let text = llvm(&exact_module(module, gfx950), gfx950).unwrap();
            let mut child = Command::new("llvm-as")
                .args(["-", "-o", "-"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("llvm-as is required by AMDGPU model integration tests");
            let written = child.stdin.take().unwrap().write_all(text.as_bytes());
            let output = child.wait_with_output().expect("wait for llvm-as");
            assert!(
                written.is_ok() && output.status.success(),
                "LLVM rejected emitted slice helper IR: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

fn isolated_helper(ty: Type) -> Module {
    let mut module = Module::new("tests::unsupported_slice_helper_abi");
    module.functions = vec![
        void_entry("entry", &[]),
        Function::internal_helper(
            "helper",
            Signature::new(vec![ty], vec![]),
            vec![ValueId(0)],
            vec![returning_block(vec![], vec![])],
        ),
    ];
    module.kernels = vec![kernel("kernel", "entry", 64)];
    module
}

#[test]
fn slice_helper_abi_rejects_mutability_address_space_element_and_external_roles() {
    for gfx950 in [false, true] {
        for ty in [
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Workgroup,
                AccessMode::ReadOnly,
            ),
            Type::slice(
                Type::Scalar(ScalarType::F64),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
        ] {
            let module = exact_module(isolated_helper(ty), gfx950);
            verify_module(&module).unwrap();
            assert!(
                llvm(&module, gfx950)
                    .unwrap_err()
                    .contains(LoweringDiagnosticCode::UnsupportedParameter)
            );
        }
        let mut export = isolated_helper(slice());
        export.functions[1].role = FunctionRole::DeviceFfiExport;
        let export = exact_module(export, gfx950);
        verify_module(&export).unwrap();
        assert!(
            llvm(&export, gfx950)
                .unwrap_err()
                .contains(LoweringDiagnosticCode::UnsupportedParameter)
        );
        let mut declaration = isolated_helper(slice());
        declaration.functions[1] =
            Function::declaration("helper", Signature::new(vec![slice()], vec![]));
        let declaration = exact_module(declaration, gfx950);
        verify_module(&declaration).unwrap();
        assert!(
            llvm(&declaration, gfx950)
                .unwrap_err()
                .contains(LoweringDiagnosticCode::UnsupportedParameter)
        );
    }
}

#[test]
fn slice_helper_abi_keeps_result_and_call_validation_closed() {
    for gfx950 in [false, true] {
        let mut module = isolated_helper(slice());
        module.functions[1].signature.results = vec![slice()];
        module.functions[1].body.as_mut().unwrap().blocks[0].terminator =
            Some(Terminator::Return {
                values: vec![ValueId(0)],
            });
        let module = exact_module(module, gfx950);
        verify_module(&module).unwrap();
        assert!(
            llvm(&module, gfx950)
                .unwrap_err()
                .contains(LoweringDiagnosticCode::UnsupportedResults)
        );
        let pointer = Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
        let mut multiple = isolated_helper(pointer.clone());
        multiple.functions[1].signature.results = vec![pointer.clone(), pointer];
        multiple.functions[1].body.as_mut().unwrap().blocks[0].terminator =
            Some(Terminator::Return {
                values: vec![ValueId(0), ValueId(0)],
            });
        let multiple = exact_module(multiple, gfx950);
        verify_module(&multiple).unwrap();
        assert!(
            llvm(&multiple, gfx950)
                .unwrap_err()
                .contains(LoweringDiagnosticCode::UnsupportedResults)
        );
        for malformed in 0..4 {
            let mut module = forwarding_module();
            let OperationKind::Call { callee, arguments } =
                &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
            else {
                panic!("fixture call")
            };
            match malformed {
                0 => {
                    arguments.pop();
                }
                1 => {
                    arguments[0] = ValueId(1);
                }
                2 => {
                    *callee = FunctionId::new("foreign_absent_helper");
                }
                3 => {
                    module.functions[1].signature.parameters[0] = Type::slice(
                        Type::Scalar(ScalarType::F32),
                        AddressSpace::Global,
                        AccessMode::ReadOnly,
                    );
                }
                _ => unreachable!(),
            }
            let module = exact_module(module, gfx950);
            assert!(verify_module(&module).is_err());
            assert!(llvm(&module, gfx950).is_err());
        }
    }
}
