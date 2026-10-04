use super::*;
use fe2o3_amdgcn_model::{
    lower_compiler_module_to_gfx942_xnack_minus_llvm_ir,
    lower_compiler_module_to_gfx950_xnack_minus_llvm_ir,
};
use fe2o3_kernel_ir::{
    ScalarType, gfx942_xnack_minus_target_capability, gfx950_xnack_minus_target_capability,
    verify_module,
};

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

fn pair() -> Vec<Type> {
    vec![Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U64)]
}

fn pair_values(first: u32) -> Vec<ValueDef> {
    pair()
        .into_iter()
        .enumerate()
        .map(|(index, ty)| ValueDef::new(ValueId(first + index as u32), ty))
        .collect()
}

fn pair_call(first: u32, callee: &str, arguments: &[u32]) -> Operation {
    Operation::new(
        pair_values(first),
        OperationKind::Call {
            callee: FunctionId::new(callee),
            arguments: arguments.iter().copied().map(ValueId).collect(),
        },
    )
}

fn pair_module() -> Module {
    let leaf = Function::internal_helper(
        "pair_leaf",
        Signature::new(pair(), pair()),
        vec![ValueId(0), ValueId(1)],
        vec![returning_block(vec![], vec![ValueId(0), ValueId(1)])],
    );
    let mut parameters = pair();
    parameters.push(Type::Scalar(ScalarType::Bool));
    let mut start = BasicBlock::new(BlockId(0));
    start.operations = vec![pair_call(3, "pair_leaf", &[0, 1])];
    start.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(3), ValueId(4)],
        else_target: BlockId(1),
        else_arguments: vec![ValueId(0), ValueId(1)],
    });
    let mut join = BasicBlock::new(BlockId(1));
    join.parameters = pair_values(5);
    join.operations = vec![pair_call(7, "pair_leaf", &[5, 6])];
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(7), ValueId(8)],
    });
    let forwarding = Function::internal_helper(
        "pair_forward",
        Signature::new(parameters.clone(), pair()),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![start, join],
    );
    let mut operations = vec![
        pair_call(3, "pair_forward", &[0, 1, 2]),
        pair_call(5, "pair_leaf", &[3, 4]),
    ];
    for (id, ty, lhs, rhs) in [
        (7, Type::Scalar(ScalarType::U32), 3, 5),
        (8, Type::Scalar(ScalarType::U64), 4, 6),
    ] {
        operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), ty),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        ));
    }
    let entry = Function::kernel_entry(
        "pair_entry",
        Signature::new(parameters, vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![returning_block(operations, vec![])],
    );
    let mut module = Module::new("tests::ordinary_helper_results");
    module.functions = vec![entry, forwarding, leaf];
    module.kernels = vec![kernel("pair_kernel", "pair_entry", 64)];
    module
}

fn loop_module() -> Module {
    let mut module = pair_module();
    let body = module.functions[1].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(3), ValueId(4)],
    });
    body.blocks[1].operations.clear();
    body.blocks[1].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(5), ValueId(6)],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.operations = vec![pair_call(7, "pair_leaf", &[5, 6])];
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(7), ValueId(8)],
    });
    body.blocks.push(exit);
    module
}

fn multiple_return_module() -> Module {
    let mut module = pair_module();
    let body = module.functions[1].body.as_mut().unwrap();
    let Some(Terminator::ConditionalBranch { else_target, .. }) = &mut body.blocks[0].terminator
    else {
        panic!("fixture branch")
    };
    *else_target = BlockId(2);
    body.blocks[1].operations.clear();
    body.blocks[1].terminator = Some(Terminator::Return {
        values: vec![ValueId(5), ValueId(6)],
    });
    let mut second = BasicBlock::new(BlockId(2));
    second.parameters = pair_values(7);
    second.terminator = Some(Terminator::Return {
        values: vec![ValueId(7), ValueId(8)],
    });
    body.blocks.push(second);
    module
}

#[test]
fn scalar_helper_structs_preserve_calls_component_order_and_duplicate_edge_joins() {
    for gfx950 in [false, true] {
        let module = exact_module(pair_module(), gfx950);
        verify_module(&module).unwrap();
        let text = llvm(&module, gfx950).unwrap();
        assert_eq!(text, llvm(&module, gfx950).unwrap());
        for helper in ["pair_forward", "pair_leaf"] {
            assert_eq!(
                text.matches(&format!(
                    "%fe2o3.helper.result.{helper} = type {{ i32, i64 }}"
                ))
                .count(),
                1
            );
            assert!(text.contains(&format!(
                "define internal %fe2o3.helper.result.{helper} @{helper}("
            )));
        }
        assert_eq!(text.matches("{ i32, i64 }").count(), 2);
        assert!(text.contains("%helper.call.0.0 = call %fe2o3.helper.result.pair_forward @pair_forward(i32 %arg0, i64 %arg1, i1 %arg2)"));
        assert!(
            text.contains(
                "%v3 = extractvalue %fe2o3.helper.result.pair_forward %helper.call.0.0, 0"
            )
        );
        assert!(
            text.contains(
                "%v4 = extractvalue %fe2o3.helper.result.pair_forward %helper.call.0.0, 1"
            )
        );
        assert!(text.contains(
            "%helper.call.0.1 = call %fe2o3.helper.result.pair_leaf @pair_leaf(i32 %v3, i64 %v4)"
        ));
        assert!(
            text.contains("%v5 = phi i32 [ %v3, %edge_bb0_0_bb1 ], [ %arg0, %edge_bb0_1_bb1 ]")
        );
        assert!(
            text.contains("%v6 = phi i64 [ %v4, %edge_bb0_0_bb1 ], [ %arg1, %edge_bb0_1_bb1 ]")
        );
        assert!(text.contains(
            "%helper.return.0.0 = insertvalue %fe2o3.helper.result.pair_leaf poison, i32 %arg0, 0"
        ));
        assert!(text.contains("%helper.return.0.1 = insertvalue %fe2o3.helper.result.pair_leaf %helper.return.0.0, i64 %arg1, 1"));
        assert!(text.contains("ret %fe2o3.helper.result.pair_leaf %helper.return.0.1"));
        assert!(text.contains("%v7 = add i32 %v3, %v5"));
        assert!(text.contains("%v8 = add i64 %v4, %v6"));
        assert!(!text.contains("alloca"));
    }
}

#[test]
fn helper_result_components_survive_loops_and_separate_return_blocks() {
    for gfx950 in [false, true] {
        let module = exact_module(loop_module(), gfx950);
        verify_module(&module).unwrap();
        let text = llvm(&module, gfx950).unwrap();
        assert!(text.contains("%v5 = phi i32 [ %v3, %bb0 ], [ %v5, %edge_bb1_0_bb1 ]"));
        assert!(text.contains("%v6 = phi i64 [ %v4, %bb0 ], [ %v6, %edge_bb1_0_bb1 ]"));
        assert!(text.contains("ret %fe2o3.helper.result.pair_forward %helper.return.2.1"));
        let module = exact_module(multiple_return_module(), gfx950);
        verify_module(&module).unwrap();
        let text = llvm(&module, gfx950).unwrap();
        for block in [1, 2] {
            assert!(text.contains(&format!(
                "ret %fe2o3.helper.result.pair_forward %helper.return.{block}.1"
            )));
        }
    }
}

fn width_module(width: usize) -> Module {
    let ty = Type::Scalar(ScalarType::U32);
    let helper = Function::internal_helper(
        "width_helper",
        Signature::new(vec![ty.clone()], vec![ty.clone(); width]),
        vec![ValueId(0)],
        vec![returning_block(vec![], vec![ValueId(0); width])],
    );
    let entry = Function::kernel_entry(
        "width_entry",
        Signature::new(vec![ty.clone()], vec![]),
        vec![ValueId(0)],
        vec![returning_block(
            vec![Operation::new(
                (0..width)
                    .map(|index| ValueDef::new(ValueId(index as u32 + 1), ty.clone()))
                    .collect(),
                OperationKind::Call {
                    callee: helper.id.clone(),
                    arguments: vec![ValueId(0)],
                },
            )],
            vec![],
        )],
    );
    let mut module = Module::new("tests::bounded_helper_result_width");
    module.functions = vec![entry, helper];
    module.kernels = vec![kernel("width_kernel", "width_entry", 64)];
    module
}

#[test]
fn result_width_boundary_is_source_derived_and_member_text_is_declared_once() {
    for gfx950 in [false, true] {
        let module = exact_module(width_module(256), gfx950);
        verify_module(&module).unwrap();
        let text = llvm(&module, gfx950).unwrap();
        assert_eq!(text.matches(" = extractvalue ").count(), 256);
        assert_eq!(text.matches(" = insertvalue ").count(), 256);
        let members = format!("{{ {} }}", vec!["i32"; 256].join(", "));
        assert_eq!(text.matches(&members).count(), 1);
        assert!(text.contains(
            "%v256 = extractvalue %fe2o3.helper.result.width_helper %helper.call.0.0, 255"
        ));
        let under = exact_module(width_module(257), gfx950);
        verify_module(&under).unwrap();
        assert!(
            llvm(&under, gfx950)
                .unwrap_err()
                .contains(LoweringDiagnosticCode::UnsupportedResults)
        );
    }
}

#[test]
fn multi_result_abi_does_not_admit_foreign_declarations_or_pointer_slice_results() {
    for gfx950 in [false, true] {
        for external in [false, true] {
            let mut module = pair_module();
            if external {
                module.functions[2] =
                    Function::declaration("pair_leaf", Signature::new(pair(), pair()));
            } else {
                module.functions[2].role = FunctionRole::DeviceFfiExport;
            }
            let module = exact_module(module, gfx950);
            verify_module(&module).unwrap();
            assert!(
                llvm(&module, gfx950)
                    .unwrap_err()
                    .contains(LoweringDiagnosticCode::UnsupportedResults)
            );
        }
        for ty in [
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
        ] {
            let mut module = Module::new("tests::closed_non_scalar_results");
            module.functions = vec![
                void_entry("entry", &[]),
                Function::internal_helper(
                    "helper",
                    Signature::new(vec![ty.clone()], vec![ty.clone(), ty]),
                    vec![ValueId(0)],
                    vec![returning_block(vec![], vec![ValueId(0), ValueId(0)])],
                ),
            ];
            module.kernels = vec![kernel("kernel", "entry", 64)];
            let module = exact_module(module, gfx950);
            verify_module(&module).unwrap();
            assert!(
                llvm(&module, gfx950)
                    .unwrap_err()
                    .contains(LoweringDiagnosticCode::UnsupportedResults)
            );
        }
        let mut unsafe_name = pair_module();
        unsafe_name.functions[1].id = FunctionId::new("bad.helper");
        let OperationKind::Call { callee, .. } =
            &mut unsafe_name.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
        else {
            panic!()
        };
        *callee = FunctionId::new("bad.helper");
        let module = exact_module(unsafe_name, gfx950);
        verify_module(&module).unwrap();
        assert!(
            llvm(&module, gfx950)
                .unwrap_err()
                .contains(LoweringDiagnosticCode::UnsafeSymbolName)
        );
    }
}

#[test]
fn malformed_call_and_return_arity_type_and_identity_reject_before_llvm_emission() {
    for gfx950 in [false, true] {
        for mutation in 0..8 {
            let mut module = pair_module();
            if mutation < 4 {
                let results =
                    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results;
                match mutation {
                    0 => {
                        results.pop();
                    }
                    1 => results.push(ValueDef::new(ValueId(99), Type::Scalar(ScalarType::U32))),
                    2 => results[1].ty = Type::Scalar(ScalarType::U32),
                    3 => results[1].id = results[0].id,
                    _ => unreachable!(),
                }
            } else {
                let Some(Terminator::Return { values }) =
                    &mut module.functions[2].body.as_mut().unwrap().blocks[0].terminator
                else {
                    panic!()
                };
                match mutation {
                    4 => {
                        values.pop();
                    }
                    5 => values.push(ValueId(0)),
                    6 => values.swap(0, 1),
                    7 => values[1] = ValueId(99),
                    _ => unreachable!(),
                }
            }
            let module = exact_module(module, gfx950);
            assert!(verify_module(&module).is_err(), "mutation {mutation}");
            assert!(llvm(&module, gfx950).is_err(), "mutation {mutation}");
        }
    }
}

fn legacy_module() -> Module {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let scalar = Type::Scalar(ScalarType::U32);
    let mut module = Module::new("tests::unchanged_zero_single_result_abi");
    module.functions = vec![
        Function::kernel_entry(
            "entry",
            Signature::new(vec![scalar.clone(), pointer.clone()], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![returning_block(
                vec![
                    Operation::new(
                        vec![],
                        OperationKind::Call {
                            callee: FunctionId::new("void_helper"),
                            arguments: vec![],
                        },
                    ),
                    Operation::new(
                        vec![ValueDef::new(ValueId(2), scalar.clone())],
                        OperationKind::Call {
                            callee: FunctionId::new("scalar_helper"),
                            arguments: vec![ValueId(0)],
                        },
                    ),
                    Operation::new(
                        vec![ValueDef::new(ValueId(3), pointer.clone())],
                        OperationKind::Call {
                            callee: FunctionId::new("pointer_helper"),
                            arguments: vec![ValueId(1)],
                        },
                    ),
                ],
                vec![],
            )],
        ),
        void_helper("void_helper", &[]),
        Function::internal_helper(
            "scalar_helper",
            Signature::new(vec![scalar.clone()], vec![scalar]),
            vec![ValueId(0)],
            vec![returning_block(vec![], vec![ValueId(0)])],
        ),
        Function::internal_helper(
            "pointer_helper",
            Signature::new(vec![pointer.clone()], vec![pointer]),
            vec![ValueId(0)],
            vec![returning_block(vec![], vec![ValueId(0)])],
        ),
    ];
    module.kernels = vec![kernel("kernel", "entry", 64)];
    module
}

#[test]
fn zero_and_single_scalar_pointer_results_keep_their_existing_llvm_spelling() {
    for gfx950 in [false, true] {
        let module = exact_module(legacy_module(), gfx950);
        verify_module(&module).unwrap();
        let text = llvm(&module, gfx950).unwrap();
        for expected in [
            "define internal void @void_helper()",
            "define internal i32 @scalar_helper(i32 %arg0)",
            "define internal ptr addrspace(1) @pointer_helper(ptr addrspace(1) %arg0)",
            "call void @void_helper()",
            "%v2 = call i32 @scalar_helper(i32 %arg0)",
            "%v3 = call ptr addrspace(1) @pointer_helper(ptr addrspace(1) %arg1)",
            "  ret void",
            "  ret i32 %arg0",
            "  ret ptr addrspace(1) %arg0",
        ] {
            assert!(text.contains(expected), "missing {expected}");
        }
        assert!(!text.contains("fe2o3.helper.result"));
        assert!(!text.contains("insertvalue"));
        assert!(!text.contains("extractvalue"));
    }
}

#[test]
fn emitted_multi_result_joins_loops_returns_and_maximum_width_pass_llvm_as() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    for gfx950 in [false, true] {
        for module in [
            pair_module(),
            loop_module(),
            multiple_return_module(),
            width_module(256),
            legacy_module(),
        ] {
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
                "LLVM rejected internal multi-result helper IR: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
