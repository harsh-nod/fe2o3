use super::*;
use fe2o3_kernel_ir::{
    AssemblyConstraint, AssemblyEffect, AssemblyOperand, AssemblyOperandKind, AssemblyOption,
    AssemblySourceIdentity, Constant, Gfx942InlineAssemblyInstructionV1 as Instruction,
    InlineAssembly, InlineAssemblyTarget,
};

const KINDS: [Instruction; 6] = [
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
            mnemonic: kind.mnemonic().into(),
            operands,
            options: [AssemblyOption::NoMemory].into(),
            declared_effects: Default::default(),
        }),
    )
}

fn assembly(operation: &mut Operation) -> &mut InlineAssembly {
    let OperationKind::InlineAssembly(assembly) = &mut operation.kind else {
        panic!()
    };
    assembly
}

fn source(operation: Operation, scalar: ScalarType) -> Module {
    let mut source = module(&[
        ("root", &["middle"], 0),
        ("middle", &["leaf"], 0),
        ("leaf", &[], 0),
    ]);
    for function in &mut source.functions {
        function.signature.parameters = vec![Type::Scalar(scalar); 2];
        function
            .required_capabilities
            .extend(operation.required_capabilities());
    }
    source
        .required_capabilities
        .extend(operation.required_capabilities());
    source.kernels[0]
        .required_capabilities
        .extend(operation.required_capabilities());
    source.functions[2].body.as_mut().unwrap().blocks[0]
        .operations
        .push(operation);
    source
}

#[test]
fn all_six_original_instructions_have_owner_scoped_empty_transitive_effects() {
    for kind in KINDS {
        let operation = instruction(kind, ScalarType::U32);
        assert!(!operation.has_complete_effect_summary());
        let source = source(operation, ScalarType::U32);
        let (owner, _) = admit(&source);
        let (inventory, _) = inventory(&owner);
        let (report, _) = report(&inventory);
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        for function in 0..3 {
            assert_eq!(
                report.decision(Function(function), &mut budget).unwrap(),
                Decision::CompleteEmpty
            );
            report
                .try_visit(Function(function), &mut budget, |_| -> Result<()> {
                    panic!(
                        "closed instruction has no physical memory or compiler ordering effects"
                    );
                })
                .unwrap();
        }
        let retained = &owner.module().functions[2].body.as_ref().unwrap().blocks[0].operations[0];
        assert_eq!(
            retained,
            &source.functions[2].body.as_ref().unwrap().blocks[0].operations[0]
        );
        assert!(matches!(retained.kind, OperationKind::InlineAssembly(_)));
    }
}

#[test]
fn real_block_parameter_and_instruction_result_types_are_resolved() {
    let mut source = source(
        instruction(Instruction::VOrB32, ScalarType::U32),
        ScalarType::U32,
    );
    let body = source.functions[2].body.as_mut().unwrap();
    body.blocks[0].operations.clear();
    body.blocks[0].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(256)),
    ));
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![ValueId(0), ValueId(2)],
    });
    let mut block = BasicBlock::new(BlockId(7));
    block.parameters = vec![
        ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
        ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
    ];
    let mut op = instruction(Instruction::VOrB32, ScalarType::U32);
    op.results[0].id = ValueId(5);
    assembly(&mut op).operands[1].kind = AssemblyOperandKind::Input(ValueId(3));
    assembly(&mut op).operands[2].kind = AssemblyOperandKind::Input(ValueId(4));
    block.operations.push(op);
    block.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(block);
    let (owner, _) = admit(&source);
    let (inventory, _) = inventory(&owner);
    let (report, _) = report(&inventory);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    assert_eq!(
        report.decision(Function(0), &mut budget).unwrap(),
        Decision::CompleteEmpty
    );
}

#[test]
fn broader_backend_profiles_and_actual_non_u32_types_remain_incomplete() {
    let original = instruction(Instruction::VAddU32, ScalarType::U32);
    let mut candidates = vec![
        (
            instruction(Instruction::SMovB32, ScalarType::U32),
            ScalarType::U32,
        ),
        (
            instruction(Instruction::VAddU32, ScalarType::I32),
            ScalarType::I32,
        ),
    ];
    for option in [
        AssemblyOption::Pure,
        AssemblyOption::NoStack,
        AssemblyOption::PreservesFlags,
    ] {
        let mut op = original.clone();
        assembly(&mut op).options.insert(option);
        candidates.push((op, ScalarType::U32));
    }
    // The generic 32-bit register constraint is not a source U32 type proof.
    candidates.push((original, ScalarType::I32));
    for (operation, scalar) in candidates {
        let source = source(operation, scalar);
        let (owner, _) = admit(&source);
        let (inventory, _) = inventory(&owner);
        let (report, _) = report(&inventory);
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        assert_eq!(
            report.decision(Function(0), &mut budget).unwrap(),
            Decision::Incomplete
        );
        assert_eq!(
            report.decision(Function(2), &mut budget).unwrap(),
            Decision::Incomplete
        );
    }
}

#[test]
fn malformed_contracts_never_acquire_empty_facts() {
    let original = instruction(Instruction::VAddU32, ScalarType::U32);
    let source = source(original.clone(), ScalarType::U32);
    let (owner, _) = admit(&source);
    let (inventory, _) = inventory(&owner);
    let mut mutations = Vec::new();
    for effect in [
        AssemblyEffect::ReadGlobal,
        AssemblyEffect::WriteGlobal,
        AssemblyEffect::ReadWorkgroup,
        AssemblyEffect::WriteWorkgroup,
        AssemblyEffect::Atomic,
        AssemblyEffect::Barrier,
        AssemblyEffect::ControlFlow,
    ] {
        let mut op = original.clone();
        assembly(&mut op).declared_effects.insert(effect);
        mutations.push(op);
    }
    for mnemonic in [
        "v_add_co_u32",
        "s_waitcnt",
        "v_add_u32\ns_endpgm",
        "v_sub_u32_e64",
    ] {
        let mut op = original.clone();
        assembly(&mut op).mnemonic = mnemonic.into();
        mutations.push(op);
    }
    for axis in 0..4 {
        let mut ids = [[1; 32], [2; 32], [3; 32], [4; 32]];
        ids[axis] = [0; 32];
        let mut op = original.clone();
        assembly(&mut op).source = AssemblySourceIdentity::new(ids[0], ids[1], ids[2], ids[3]);
        mutations.push(op);
    }
    for mutation in 0..8 {
        let mut op = original.clone();
        match mutation {
            0 => assembly(&mut op).options.clear(),
            1 => {
                assembly(&mut op).operands.pop();
            }
            2 => {
                assembly(&mut op).operands[0].kind = AssemblyOperandKind::Output { result_index: 1 }
            }
            3 => assembly(&mut op).operands[1].constraint = AssemblyConstraint::Sgpr32,
            4 => assembly(&mut op).operands[1].kind = AssemblyOperandKind::Input(ValueId(99)),
            5 => op.results.clear(),
            6 => op.results[0].ty = Type::Scalar(ScalarType::F32),
            7 => {
                assembly(&mut op).operands[1] = AssemblyOperand {
                    kind: AssemblyOperandKind::ImmediateI32(0),
                    constraint: AssemblyConstraint::ImmediateI32,
                }
            }
            _ => unreachable!(),
        }
        mutations.push(op);
    }
    for op in mutations {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        assert!(
            !gfx942_inline_v30::has_closed_effects(&inventory, Function(2), &op, &mut budget)
                .unwrap(),
            "{:?}",
            op.kind
        );
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn closed_validator_work_is_precharged_and_lookup_failures_are_not_swallowed() {
    let op = instruction(Instruction::VAddU32, ScalarType::U32);
    let source = source(op.clone(), ScalarType::U32);
    let (owner, _) = admit(&source);
    let (inventory, _) = inventory(&owner);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(17).unwrap();
    assert!(
        gfx942_inline_v30::has_closed_effects(&inventory, Function(2), &op, &mut budget).unwrap()
    );
    let exact = budget.work();
    assert_eq!(budget.storage(), 17);
    for limit in [0, 11, 12, 523, 524, exact - 1] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 17);
        budget.reserve_storage(17).unwrap();
        assert!(matches!(
            gfx942_inline_v30::has_closed_effects(&inventory, Function(2), &op, &mut budget),
            Err(Error::Resource(_))
        ));
        assert_eq!(budget.storage(), 17);
    }
    let mut work = Work::new(exact);
    let mut budget = Budget::new(&mut work, 0);
    assert!(
        gfx942_inline_v30::has_closed_effects(&inventory, Function(2), &op, &mut budget).unwrap()
    );
}
