use super::*;
use crate::*;
use Gfx942InlineAssemblyInstructionV1 as Instruction;

const INSTRUCTIONS: [Instruction; 6] = [
    Instruction::VMovB32,
    Instruction::VAddU32,
    Instruction::VSubU32,
    Instruction::VAndB32,
    Instruction::VOrB32,
    Instruction::VXorB32,
];

fn instruction(kind: Instruction, scalar: ScalarType) -> Operation {
    let mut operands = vec![AssemblyOperand::output(0, kind.constraint())];
    operands.extend(
        (0..kind.input_count())
            .map(|index| AssemblyOperand::input(ValueId(index as u32), kind.constraint())),
    );
    Operation::effect_free(
        ValueDef::new(ValueId(2), Type::Scalar(scalar)),
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: kind.mnemonic().to_owned(),
            operands,
            options: BTreeSet::from([AssemblyOption::NoMemory]),
            declared_effects: BTreeSet::new(),
        }),
    )
}

fn assembly(operation: &mut Operation) -> &mut InlineAssembly {
    let OperationKind::InlineAssembly(assembly) = &mut operation.kind else {
        unreachable!()
    };
    assembly
}

fn helper(name: &str, operations: Vec<Operation>) -> Function {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::internal_helper(
        name,
        Signature::new(vec![Type::Scalar(ScalarType::U32); 2], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();
    function
}

fn module(functions: Vec<Function>) -> Module {
    let mut module = Module::new("closed_assembly_effects");
    module.required_capabilities.extend(
        functions
            .iter()
            .flat_map(|function| function.required_capabilities.iter().cloned()),
    );
    module.functions = functions;
    module
}

fn call(callee: &str) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new(callee),
            arguments: vec![ValueId(0), ValueId(1)],
        },
    )
}

#[test]
fn six_verified_u32_instructions_have_closed_transitive_memory_and_ordering_effects() {
    for kind in INSTRUCTIONS {
        let operation = instruction(kind, ScalarType::U32);
        assert!(!operation.has_complete_effect_summary());
        let module = module(vec![
            helper("root", vec![call("middle")]),
            helper("middle", vec![call("leaf")]),
            helper("leaf", vec![operation]),
        ]);
        let analysis = analyze_interprocedural_effects_v1(&module).unwrap();
        for name in ["root", "middle", "leaf"] {
            let decision = analysis.function(&FunctionId::new(name)).unwrap();
            assert!(decision.is_complete_and_pure(), "{kind:?}: {decision:?}");
            assert!(decision.summary().compiler_ordering().is_empty());
        }
    }
}

#[test]
fn assembly_preflight_keeps_deep_call_chains_within_the_default_stack_bound() {
    // Match the existing lowerer's 1,025-function limit regression. An explicit
    // 2 MiB thread keeps this check independent of a runner's RUST_MIN_STACK.
    std::thread::Builder::new()
        .name("effect-summary-default-stack".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            const COUNT: usize = 1_025;
            for with_assembly_leaf in [false, true] {
                let mut functions = Vec::with_capacity(COUNT);
                for index in 0..COUNT {
                    let operations = if index + 1 < COUNT {
                        vec![call(&format!("helper_{}", index + 1))]
                    } else if with_assembly_leaf {
                        vec![instruction(Instruction::VAddU32, ScalarType::U32)]
                    } else {
                        vec![]
                    };
                    functions.push(helper(&format!("helper_{index}"), operations));
                }
                let analysis = analyze_interprocedural_effects_v1(&module(functions)).unwrap();
                assert!(
                    analysis
                        .function(&FunctionId::new("helper_0"))
                        .unwrap()
                        .is_complete_and_pure()
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn verified_block_parameters_and_operation_results_supply_actual_operand_types() {
    let mut function = helper(
        "leaf",
        vec![Operation::effect_free(
            ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(u32::MAX)),
        )],
    );
    let body = function.body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0), ValueId(2)],
    });
    let mut block = BasicBlock::new(BlockId(1));
    block.parameters = vec![
        ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
        ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
    ];
    let mut operation = instruction(Instruction::VAddU32, ScalarType::U32);
    operation.results[0].id = ValueId(5);
    assembly(&mut operation).operands[1].kind = AssemblyOperandKind::Input(ValueId(3));
    assembly(&mut operation).operands[2].kind = AssemblyOperandKind::Input(ValueId(4));
    block.operations.push(operation);
    block.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(block);
    function.required_capabilities = function.derived_capabilities();
    let module = module(vec![function]);
    let decision = analyze_interprocedural_effects_v1(&module).unwrap();
    assert!(
        decision
            .function(&FunctionId::new("leaf"))
            .unwrap()
            .is_complete_and_pure()
    );
}

#[test]
fn nomem_and_empty_effects_do_not_certify_unknown_scalar_or_other_assembly() {
    let original = instruction(Instruction::VAddU32, ScalarType::U32);
    let mut mutations = Vec::new();
    for mnemonic in [
        "s_waitcnt",
        "v_add_co_u32",
        "v_add_u32_e64",
        "v_add_u32\ns_endpgm",
    ] {
        let mut changed = original.clone();
        assembly(&mut changed).mnemonic = mnemonic.to_owned();
        mutations.push(changed);
    }
    mutations.push(instruction(Instruction::SMovB32, ScalarType::U32));
    mutations.push(instruction(Instruction::VAddU32, ScalarType::I32));
    for option in [
        AssemblyOption::Pure,
        AssemblyOption::PreservesFlags,
        AssemblyOption::NoStack,
        AssemblyOption::ReadOnly,
    ] {
        let mut changed = original.clone();
        assembly(&mut changed).options.insert(option);
        mutations.push(changed);
    }
    let mut changed = original.clone();
    assembly(&mut changed).options.clear();
    mutations.push(changed);
    for effect in [
        AssemblyEffect::ReadGlobal,
        AssemblyEffect::WriteGlobal,
        AssemblyEffect::ReadWorkgroup,
        AssemblyEffect::WriteWorkgroup,
        AssemblyEffect::Atomic,
        AssemblyEffect::Barrier,
        AssemblyEffect::ControlFlow,
    ] {
        let mut changed = original.clone();
        assembly(&mut changed).declared_effects.insert(effect);
        mutations.push(changed);
    }
    for constraint in [AssemblyConstraint::Sgpr32, AssemblyConstraint::ImmediateI32] {
        for index in 0..3 {
            let mut changed = original.clone();
            assembly(&mut changed).operands[index].constraint = constraint;
            mutations.push(changed);
        }
    }
    for field in 0..4 {
        let mut changed = original.clone();
        let source = &mut assembly(&mut changed).source;
        match field {
            0 => source.frontend_unit = [0; 32],
            1 => source.function = [0; 32],
            2 => source.contract = [0; 32],
            _ => source.statement = [0; 32],
        }
        mutations.push(changed);
    }
    for role in [
        AssemblyOperandKind::Input(ValueId(0)),
        AssemblyOperandKind::Output { result_index: 1 },
        AssemblyOperandKind::InOut {
            input: ValueId(0),
            result_index: 0,
        },
        AssemblyOperandKind::ImmediateI32(7),
    ] {
        let mut changed = original.clone();
        assembly(&mut changed).operands[0].kind = role;
        mutations.push(changed);
    }
    let mut changed = original.clone();
    assembly(&mut changed).operands[1].kind = AssemblyOperandKind::Input(ValueId(99));
    mutations.push(changed);
    let mut changed = original.clone();
    assembly(&mut changed).operands.pop();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.results.clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed
        .results
        .push(ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)));
    mutations.push(changed);
    for operation in mutations {
        let function = helper("leaf", vec![operation.clone()]);
        let types = collect_assembly_value_types_v1(
            &function,
            &mut AssemblyTypeBudgetV1 { used: 0, limit: 64 },
        )
        .unwrap();
        assert!(
            !is_closed_u32_assembly_effect_v30(&operation, &types),
            "{operation:?}"
        );
        let module = module(vec![helper("root", vec![call("leaf")]), function]);
        if let Ok(analysis) = analyze_interprocedural_effects_v1(&module) {
            for name in ["root", "leaf"] {
                let decision = analysis.function(&FunctionId::new(name)).unwrap();
                assert!(!decision.is_complete());
                assert!(decision.incomplete_reasons().iter().any(|reason| matches!(
                    reason,
                    InterproceduralEffectIncompleteReasonV1::InlineAssembly { .. }
                )));
            }
        }
    }
}

#[test]
fn same_width_input_type_and_missing_definition_cannot_use_result_type_as_evidence() {
    let operation = instruction(Instruction::VAddU32, ScalarType::U32);
    for scalar in [ScalarType::I32, ScalarType::F32] {
        let mut function = helper("leaf", vec![operation.clone()]);
        function.signature.parameters[0] = Type::Scalar(scalar);
        let types = collect_assembly_value_types_v1(
            &function,
            &mut AssemblyTypeBudgetV1 { used: 0, limit: 5 },
        )
        .unwrap();
        assert!(!is_closed_u32_assembly_effect_v30(&operation, &types));
        let module = module(vec![function]);
        if let Ok(analysis) = analyze_interprocedural_effects_v1(&module) {
            assert!(
                !analysis
                    .function(&FunctionId::new("leaf"))
                    .unwrap()
                    .is_complete()
            );
        }
    }
    let function = helper("leaf", vec![operation.clone()]);
    let mut types =
        collect_assembly_value_types_v1(&function, &mut AssemblyTypeBudgetV1 { used: 0, limit: 5 })
            .unwrap();
    types.remove(&ValueId(0));
    assert!(!is_closed_u32_assembly_effect_v30(&operation, &types));
}

#[test]
fn assembly_type_tables_are_preflighted_and_globally_bounded_across_callers() {
    let leaf = helper(
        "leaf",
        vec![instruction(Instruction::VMovB32, ScalarType::U32)],
    );
    let mut exact = AssemblyTypeBudgetV1 { used: 0, limit: 5 };
    assert_eq!(
        collect_assembly_value_types_v1(&leaf, &mut exact)
            .unwrap()
            .len(),
        3
    );
    assert_eq!(exact.used, 5);
    assert!(matches!(
        collect_assembly_value_types_v1(&leaf, &mut AssemblyTypeBudgetV1 { used: 0, limit: 4 }),
        Err(InterproceduralEffectIncompleteReasonV1::ResourceLimit {
            resource: "inline assembly type work",
            actual: 5,
            limit: 4
        })
    ));
    let module = module(vec![
        helper(
            "root",
            vec![
                instruction(Instruction::VMovB32, ScalarType::U32),
                call("leaf"),
            ],
        ),
        leaf,
    ]);
    let verified = verify_module_ref(&module).unwrap();
    let mut builder = EffectSummaryBuilderV1 {
        module: verified.module(),
        decisions: BTreeMap::new(),
        visiting: BTreeSet::new(),
        call_edges: 0,
        assembly_type_budget: AssemblyTypeBudgetV1 { used: 0, limit: 10 },
    };
    let decision = builder.summarize(&FunctionId::new("root"));
    assert!(!decision.is_complete());
    assert!(decision.incomplete_reasons().iter().any(|reason| matches!(
        reason,
        InterproceduralEffectIncompleteReasonV1::ResourceLimit {
            resource: "inline assembly type work",
            actual: 11,
            limit: 10
        }
    )));
}
