//! Synthetic typed modules exercise formal memory extraction only. Nonzero
//! assembly source IDs are structural fixtures, not frontend authority.

use std::collections::BTreeSet;

use Gfx942InlineAssemblyInstructionV1 as Instruction;
use fe2o3_kernel_ir::*;

const INSTRUCTIONS: [Instruction; 6] = [
    Instruction::VMovB32,
    Instruction::VAddU32,
    Instruction::VSubU32,
    Instruction::VAndB32,
    Instruction::VOrB32,
    Instruction::VXorB32,
];

fn u32_type() -> Type {
    Type::Scalar(ScalarType::U32)
}

fn pointer() -> Type {
    Type::pointer(u32_type(), AddressSpace::Global, AccessMode::ReadWrite)
}

fn instruction(kind: Instruction, scalar: ScalarType, result: u32, inputs: [u32; 2]) -> Operation {
    let mut operands = vec![AssemblyOperand::output(0, kind.constraint())];
    operands.extend(
        inputs[..kind.input_count()]
            .iter()
            .map(|input| AssemblyOperand::input(ValueId(*input), kind.constraint())),
    );
    Operation::effect_free(
        ValueDef::new(ValueId(result), Type::Scalar(scalar)),
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

fn returning_block(id: u32, operations: Vec<Operation>) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

fn refresh_capabilities(module: &mut Module) {
    for function in &mut module.functions {
        function.required_capabilities = function.derived_capabilities();
    }
    module.required_capabilities = module
        .functions
        .iter()
        .flat_map(|function| function.required_capabilities.iter().cloned())
        .collect();
}

fn module(parameters: Vec<Type>, blocks: Vec<BasicBlock>) -> Module {
    let values = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect();
    let function =
        Function::kernel_entry("root", Signature::new(parameters, vec![]), values, blocks);
    let mut module = Module::new("formal_memory_closed_assembly");
    module.functions.push(function);
    module.kernels.push(Kernel::new(
        "kernel",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    refresh_capabilities(&mut module);
    module
}

fn analyze(module: &Module) -> FormalMemoryObligationAnalysis {
    verify_module_ref(module).expect("fixture must independently pass module verification");
    derive_kernel_memory_obligations(
        module,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(1),
        FormalIndexWidth::Bits64,
    )
    .unwrap()
}

fn store(pointer: u32, value: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(pointer),
            value: ValueId(value),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}

fn load(result: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(result), u32_type()),
        OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}

#[test]
fn all_six_close_only_assembly_uncertainty_and_preserve_surrounding_accesses() {
    for kind in INSTRUCTIONS {
        let operation = instruction(kind, ScalarType::U32, 4, [1, 3]);
        assert!(!operation.has_complete_effect_summary());
        let module = module(
            vec![pointer(), u32_type(), u32_type()],
            vec![returning_block(
                0,
                vec![load(3), operation.clone(), store(0, 4)],
            )],
        );
        let analysis = analyze(&module);
        assert!(analysis.is_complete(), "{kind:?}: {analysis:?}");
        let accesses = analysis.obligations().accesses();
        assert_eq!(accesses.len(), 2);
        assert_eq!(accesses[0].kind(), FormalMemoryAccessKind::Read);
        assert_eq!(accesses[1].kind(), FormalMemoryAccessKind::Write);
        assert_eq!(
            accesses[0].location(),
            FunctionOperationLocation::new(BlockId(0), 0)
        );
        assert_eq!(
            accesses[1].location(),
            FunctionOperationLocation::new(BlockId(0), 2)
        );
        assert!(
            accesses
                .iter()
                .all(|access| access.allocation().parameter_index() == 0)
        );
        assert!(!analysis.obligations().bounds_requirements().is_empty());
        assert_eq!(
            module.functions[0].body.as_ref().unwrap().blocks[0].operations[1],
            operation
        );
    }
}

#[test]
fn full_type_table_includes_function_block_and_constant_definitions() {
    for kind in INSTRUCTIONS {
        let mut entry = returning_block(
            0,
            vec![Operation::effect_free(
                ValueDef::new(ValueId(2), u32_type()),
                OperationKind::Constant(Constant::U32(u32::MAX)),
            )],
        );
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(0), ValueId(2)],
        });
        let mut successor = returning_block(
            1,
            vec![
                instruction(kind, ScalarType::U32, 5, [3, 4]),
                instruction(kind, ScalarType::U32, 6, [1, 2]),
            ],
        );
        successor.parameters = vec![
            ValueDef::new(ValueId(3), u32_type()),
            ValueDef::new(ValueId(4), u32_type()),
        ];
        let analysis = analyze(&module(vec![u32_type(); 2], vec![entry, successor]));
        assert!(analysis.is_complete(), "{kind:?}: {analysis:?}");
        assert!(analysis.obligations().accesses().is_empty());
    }
}

#[test]
fn memory_free_assembly_does_not_prove_an_affine_pointer_index() {
    for kind in INSTRUCTIONS {
        let operation = instruction(kind, ScalarType::U32, 3, [1, 2]);
        let widen = Operation::effect_free(
            ValueDef::new(ValueId(4), Type::INDEX),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(3),
                to: Type::INDEX,
            },
        );
        let gep = Operation::effect_free(
            ValueDef::new(ValueId(5), pointer()),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(4),
            },
        );
        let analysis = analyze(&module(
            vec![pointer(), u32_type(), u32_type()],
            vec![returning_block(0, vec![operation, widen, gep, store(5, 1)])],
        ));
        assert!(
            matches!(
                analysis.incomplete_reasons(),
                [FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. },]
            ),
            "{kind:?}: {analysis:?}"
        );
        assert!(analysis.obligations().accesses().is_empty());
    }
}

fn unsupported_cases() -> Vec<(Operation, Vec<Type>)> {
    let original = instruction(Instruction::VAddU32, ScalarType::U32, 2, [0, 1]);
    let mut operations = Vec::new();
    for mnemonic in [
        "s_waitcnt",
        "global_load_dword",
        "v_add_co_u32",
        "v_add_u32_e64",
    ] {
        let mut changed = original.clone();
        assembly(&mut changed).mnemonic = mnemonic.to_owned();
        operations.push(changed);
    }
    operations.push(instruction(
        Instruction::SMovB32,
        ScalarType::U32,
        2,
        [0, 1],
    ));
    for option in [
        AssemblyOption::Pure,
        AssemblyOption::PreservesFlags,
        AssemblyOption::NoStack,
    ] {
        let mut changed = original.clone();
        assembly(&mut changed).options.insert(option);
        operations.push(changed);
    }
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
        let asm = assembly(&mut changed);
        if effect != AssemblyEffect::ControlFlow {
            asm.options.clear();
        }
        asm.declared_effects.insert(effect);
        operations.push(changed);
    }
    let mut read_only = original.clone();
    assembly(&mut read_only).options = BTreeSet::from([AssemblyOption::ReadOnly]);
    assembly(&mut read_only)
        .declared_effects
        .insert(AssemblyEffect::ReadGlobal);
    operations.push(read_only);
    for index in 0..3 {
        let mut changed = original.clone();
        assembly(&mut changed).operands[index].constraint = AssemblyConstraint::Sgpr32;
        operations.push(changed);
    }
    let mut reordered = original.clone();
    assembly(&mut reordered).operands.swap(0, 1);
    operations.push(reordered);
    let mut short = original.clone();
    assembly(&mut short).operands.pop();
    operations.push(short);
    let mut inout = original.clone();
    assembly(&mut inout).operands[0].kind = AssemblyOperandKind::InOut {
        input: ValueId(0),
        result_index: 0,
    };
    operations.push(inout);
    let mut cases: Vec<_> = operations
        .into_iter()
        .map(|operation| (operation, vec![u32_type(); 2]))
        .collect();
    cases.push((
        instruction(Instruction::VAddU32, ScalarType::I32, 2, [0, 1]),
        vec![Type::Scalar(ScalarType::I32); 2],
    ));
    cases.push((original, vec![Type::Scalar(ScalarType::I32), u32_type()]));
    cases
}

#[test]
fn verified_unknown_options_effects_operand_roles_and_types_remain_incomplete() {
    for (operation, parameters) in unsupported_cases() {
        let module = module(
            parameters,
            vec![returning_block(0, vec![operation.clone()])],
        );
        let analysis = analyze(&module);
        assert_eq!(
            analysis.incomplete_reasons(),
            &[FormalMemoryIncompleteReason::UnsupportedMemoryEffect {
                location: FunctionOperationLocation::new(BlockId(0), 0),
            },],
            "{operation:?}: {analysis:?}"
        );
    }
}

#[test]
fn unknown_assembly_does_not_hide_surrounding_memory_obligations() {
    let mut operation = instruction(Instruction::VAddU32, ScalarType::U32, 4, [1, 3]);
    assembly(&mut operation).mnemonic = "unrecognized_nomem".to_owned();
    let analysis = analyze(&module(
        vec![pointer(), u32_type(), u32_type()],
        vec![returning_block(0, vec![load(3), operation, store(0, 4)])],
    ));
    assert_eq!(analysis.obligations().accesses().len(), 2);
    assert_eq!(
        analysis.incomplete_reasons(),
        &[FormalMemoryIncompleteReason::UnsupportedMemoryEffect {
            location: FunctionOperationLocation::new(BlockId(0), 1),
        },]
    );
}

#[test]
fn mandatory_module_verification_rejects_malformed_source_and_ssa() {
    let original = instruction(Instruction::VAddU32, ScalarType::U32, 2, [0, 1]);
    let mut malformed = Vec::new();
    for index in 0..4 {
        let mut changed = original.clone();
        let source = &mut assembly(&mut changed).source;
        match index {
            0 => source.frontend_unit = [0; 32],
            1 => source.function = [0; 32],
            2 => source.contract = [0; 32],
            _ => source.statement = [0; 32],
        }
        malformed.push(changed);
    }
    let mut undefined = original.clone();
    assembly(&mut undefined).operands[1].kind = AssemblyOperandKind::Input(ValueId(99));
    malformed.push(undefined);
    let mut impossible_options = original.clone();
    assembly(&mut impossible_options)
        .options
        .insert(AssemblyOption::ReadOnly);
    malformed.push(impossible_options);
    let mut impossible_effects = original;
    assembly(&mut impossible_effects)
        .declared_effects
        .insert(AssemblyEffect::WriteGlobal);
    malformed.push(impossible_effects);
    for operation in malformed {
        let module = module(
            vec![u32_type(); 2],
            vec![returning_block(0, vec![operation])],
        );
        assert!(verify_module_ref(&module).is_err());
        assert!(matches!(
            derive_kernel_memory_obligations(
                &module,
                &KernelId::new("kernel"),
                ExplicitLaunchExtent1d::Exact(1),
                FormalIndexWidth::Bits64,
            ),
            Err(FormalMemoryObligationError::InvalidModule(_))
        ));
    }
}

fn helper_module(operation: Operation, with_store: bool) -> Module {
    let parameters = vec![pointer(), u32_type(), u32_type()];
    let call = |callee: &str| {
        Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new(callee),
                arguments: vec![ValueId(0), ValueId(1), ValueId(2)],
            },
        )
    };
    let mut module = module(
        parameters.clone(),
        vec![returning_block(0, vec![call("middle")])],
    );
    let leaf_operations = if with_store {
        vec![store(0, 1), operation]
    } else {
        vec![operation]
    };
    for (name, operations) in [("middle", vec![call("leaf")]), ("leaf", leaf_operations)] {
        module.functions.push(Function::internal_helper(
            name,
            Signature::new(parameters.clone(), vec![]),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![returning_block(0, operations)],
        ));
    }
    refresh_capabilities(&mut module);
    module
}

#[test]
fn existing_transitive_helper_summaries_close_only_pure_supported_helpers() {
    for kind in INSTRUCTIONS {
        let operation = instruction(kind, ScalarType::U32, 3, [1, 2]);
        let analysis = analyze(&helper_module(operation.clone(), false));
        assert!(analysis.is_complete(), "{kind:?}: {analysis:?}");
        assert!(analysis.obligations().accesses().is_empty());
        let analysis = analyze(&helper_module(operation, true));
        assert!(
            matches!(
                analysis.incomplete_reasons(),
                [FormalMemoryIncompleteReason::CallEffectsUnavailable { .. },]
            ),
            "memory-writing helper must remain conservative: {analysis:?}"
        );
    }
    let mut unknown = instruction(Instruction::VAddU32, ScalarType::U32, 3, [1, 2]);
    assembly(&mut unknown).mnemonic = "unknown_no_memory".to_owned();
    let analysis = analyze(&helper_module(unknown, false));
    assert!(matches!(
        analysis.incomplete_reasons(),
        [FormalMemoryIncompleteReason::CallEffectsUnavailable { .. },]
    ));
}
