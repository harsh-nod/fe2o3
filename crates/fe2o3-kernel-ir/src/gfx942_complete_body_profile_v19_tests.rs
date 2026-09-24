//! Inert canonical-graph fixtures. These are not authenticated source owners,
//! instruction execution, native validation or hardware observations.
use super::*;
use crate::{
    CanonicalKernelIrWorkBudgetV1 as Work, Gfx942CompleteBodyOriginVNext as Origin,
    Gfx942CompleteBodyStepVNext as Step, Gfx942OrderedProgramRegistersV1 as Registers,
    Gfx942ProgramBinaryOpcodeV1 as Opcode, Gfx942ProgramDestinationV1 as Destination, Kernel,
    LaunchDomain, LaunchExtent, MemoryAccess, Signature, ValueDef,
};

fn u32_ty() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn pointer_ty() -> Type {
    Type::pointer(u32_ty(), AddressSpace::Global, AccessMode::ReadWrite)
}
fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::new(vec![ValueDef::new(ValueId(id), ty)], kind)
}
fn movement(
    block: u8,
    index: u8,
    id: u32,
    destination: Destination,
    source: Role,
    value: u32,
) -> Operation {
    op(
        id,
        u32_ty(),
        OperationKind::Gfx942CompleteBodyStep(Step {
            authored_block: block,
            authored_instruction: index,
            instruction: Instruction::Move {
                destination,
                source,
            },
            operands: [Some(ValueId(value)), None],
        }),
    )
}
fn declaration(blocks: u8, count: u8) -> Operation {
    let mut labels = [0; 8];
    labels[..usize::from(blocks)].copy_from_slice(if blocks == 1 {
        &[251]
    } else {
        &[250, 4, 0, 7]
    });
    Operation::new(
        vec![],
        OperationKind::Gfx942CompleteBodyDeclaration(Gfx942CompleteBodyDeclarationVNext {
            origin: Origin {
                root_axes: [[1; 32]; 5],
                mir_body: [2; 32],
                semantic_block: [3; 32],
                source_signature: [4; 32],
                rustc_fn_abi: [5; 32],
                frontend_bytes_sha256: [6; 32],
                raw_block: 9,
            },
            registers: Registers::new(32, 33, [34, 35, 63]).unwrap(),
            parameters: [ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
            labels,
            block_count: blocks,
            instruction_count: count,
        }),
    )
}
fn tail(base: u32, value: u32) -> Vec<Operation> {
    vec![
        op(
            base,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            base + 1,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            base + 2,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(base),
                rhs: ValueId(base + 1),
            },
        ),
        op(
            base + 3,
            pointer_ty(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            base + 4,
            pointer_ty(),
            OperationKind::GetElementPointer {
                base: ValueId(base + 3),
                offset: ValueId(base),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(base + 4),
                predicate: ValueId(base + 2),
                value: ValueId(value),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]
}
fn module(blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("synthetic_complete_body");
    let function = Function::kernel_entry(
        "synthetic_entry",
        Signature::new(
            vec![
                Type::slice(u32_ty(), AddressSpace::Global, AccessMode::ReadWrite),
                u32_ty(),
                u32_ty(),
                u32_ty(),
                u32_ty(),
            ],
            vec![],
        ),
        (0..5).map(ValueId).collect(),
        blocks,
    );
    let mut kernel = Kernel::new(
        "synthetic_kernel",
        function.id.clone(),
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}
pub(crate) fn single() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        declaration(1, 1),
        movement(0, 0, 5, Destination::Output, Role::Input0, 1),
    ];
    block.operations.extend(tail(6, 5));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module(vec![block])
}
pub(crate) fn diamond() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        declaration(4, 3),
        movement(0, 0, 5, Destination::Scratch, Role::Input0, 1),
        op(6, u32_ty(), OperationKind::Constant(Constant::U32(0))),
        op(
            7,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(4),
                rhs: ValueId(6),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(5)],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(5)],
    });
    let mut zero = BasicBlock::new(BlockId(1));
    zero.parameters = vec![ValueDef::new(ValueId(8), u32_ty())];
    zero.operations = vec![movement(1, 1, 9, Destination::Output, Role::Scratch, 8)];
    zero.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(8), ValueId(9)],
    });
    let mut nonzero = BasicBlock::new(BlockId(2));
    nonzero.parameters = vec![ValueDef::new(ValueId(10), u32_ty())];
    nonzero.operations = vec![movement(2, 2, 11, Destination::Output, Role::Input2, 3)];
    nonzero.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(10), ValueId(11)],
    });
    let mut merge = BasicBlock::new(BlockId(3));
    merge.parameters = vec![
        ValueDef::new(ValueId(12), u32_ty()),
        ValueDef::new(ValueId(13), u32_ty()),
    ];
    merge.operations = tail(14, 13);
    merge.terminator = Some(Terminator::Return { values: vec![] });
    module(vec![entry, zero, nonzero, merge])
}
fn blocks(module: &mut Module) -> &mut [BasicBlock] {
    &mut module.functions[0].body.as_mut().unwrap().blocks
}
fn decl(module: &mut Module) -> &mut Gfx942CompleteBodyDeclarationVNext {
    let OperationKind::Gfx942CompleteBodyDeclaration(declaration) =
        &mut blocks(module)[0].operations[0].kind
    else {
        panic!("fixture declaration");
    };
    declaration
}
fn step(module: &mut Module, block: usize, operation: usize) -> &mut Step {
    let OperationKind::Gfx942CompleteBodyStep(step) =
        &mut blocks(module)[block].operations[operation].kind
    else {
        panic!("fixture step");
    };
    step
}
fn check(module: &Module) -> Result<()> {
    let mut work = Work::new(COMPLETE_BODY_PROFILE_WORK_V19 + 7);
    let mut budget = Budget::new(&mut work, 73);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(73).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = check_gfx942_complete_body_function_v19(module, &module.functions[0], &mut budget);
    assert_eq!(budget.storage(), 73);
    assert_eq!(budget.work(), COMPLETE_BODY_PROFILE_WORK_V19 + 7);
    assert!(budget.work_ledger_identity_v1() == ledger);
    result
}
fn invalid(module: &Module) {
    assert!(matches!(check(module), Err(Error::Invalid(_))));
}

#[test]
fn accepts_actual_one_block_and_ssa_diamond_shapes() {
    check(&single()).unwrap();
    check(&diamond()).unwrap();
}
#[test]
fn rejects_foreign_function_even_when_structurally_equal() {
    let module = single();
    let foreign = module.functions[0].clone();
    let mut work = Work::new(COMPLETE_BODY_PROFILE_WORK_V19);
    let mut budget = Budget::new(&mut work, 0);
    assert_eq!(
        check_gfx942_complete_body_function_v19(&module, &foreign, &mut budget),
        Err(Error::Invalid("complete-body actual function membership"))
    );
}
#[test]
fn rejects_extra_function_kernel_wrong_launch_or_signature() {
    let mut m = single();
    m.functions.push(m.functions[0].clone());
    invalid(&m);
    let mut m = single();
    m.kernels.push(m.kernels[0].clone());
    invalid(&m);
    let mut m = single();
    m.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
    invalid(&m);
    let mut m = single();
    m.functions[0].signature.parameters[4] = Type::BOOL;
    invalid(&m);
    let mut m = single();
    m.functions[0].signature.parameters[0] =
        Type::slice(u32_ty(), AddressSpace::Global, AccessMode::ReadOnly);
    invalid(&m);
}
#[test]
fn declaration_must_be_first_unique_and_resultless() {
    let mut m = single();
    blocks(&mut m)[0].operations.remove(0);
    invalid(&m);
    let mut m = single();
    blocks(&mut m)[0].operations.swap(0, 1);
    invalid(&m);
    let mut m = single();
    blocks(&mut m)[0].operations.insert(2, declaration(1, 1));
    invalid(&m);
    let mut m = single();
    blocks(&mut m)[0].operations[0]
        .results
        .push(ValueDef::new(ValueId(99), u32_ty()));
    invalid(&m);
}
#[test]
fn declaration_identity_counts_labels_and_registers_are_closed() {
    let mut m = single();
    decl(&mut m).origin.root_axes[3] = [0; 32];
    invalid(&m);
    let mut m = single();
    decl(&mut m).origin.rustc_fn_abi = [0; 32];
    invalid(&m);
    let mut m = single();
    decl(&mut m).parameters[0] = ValueId(99);
    invalid(&m);
    let mut m = single();
    decl(&mut m).instruction_count = 2;
    invalid(&m);
    let mut m = single();
    decl(&mut m).block_count = 8;
    invalid(&m);
    let mut m = single();
    decl(&mut m).labels[7] = 1;
    invalid(&m);
    let mut m = diamond();
    decl(&mut m).labels[1] = 250;
    invalid(&m);
    let mut m = single();
    decl(&mut m).registers = Registers::new(7, 33, [34, 35, 63]).unwrap();
    invalid(&m);
}
#[test]
fn refuses_duplicate_definitions_and_wrong_step_result_type() {
    let mut m = single();
    blocks(&mut m)[0].operations[1].results[0].id = ValueId(1);
    invalid(&m);
    let mut m = diamond();
    blocks(&mut m)[2].parameters[0].id = ValueId(8);
    invalid(&m);
    let mut m = single();
    blocks(&mut m)[0].operations[1].results[0].ty = Type::BOOL;
    invalid(&m);
}
#[test]
fn preserves_authored_step_ordinals_block_and_exact_operand_roster() {
    let mut m = diamond();
    step(&mut m, 2, 0).authored_instruction = 1;
    invalid(&m);
    let mut m = diamond();
    step(&mut m, 2, 0).authored_block = 1;
    invalid(&m);
    let mut m = single();
    step(&mut m, 0, 1).operands[1] = Some(ValueId(2));
    invalid(&m);
    let mut m = single();
    step(&mut m, 0, 1).operands[0] = Some(ValueId(2));
    invalid(&m);
    let mut m = single();
    step(&mut m, 0, 1).instruction = Instruction::Move {
        destination: Destination::Output,
        source: Role::Scratch,
    };
    invalid(&m);
}
#[test]
fn accepts_read_before_write_but_rejects_old_ssa_after_redefinition() {
    let mut m = single();
    decl(&mut m).instruction_count = 3;
    blocks(&mut m)[0]
        .operations
        .insert(1, movement(0, 0, 30, Destination::Scratch, Role::Input0, 1));
    blocks(&mut m)[0].operations.insert(
        2,
        op(
            31,
            u32_ty(),
            OperationKind::Gfx942CompleteBodyStep(Step {
                authored_block: 0,
                authored_instruction: 1,
                instruction: Instruction::Binary {
                    opcode: Opcode::Xor,
                    destination: Destination::Scratch,
                    left: Role::Scratch,
                    right: Role::Input1,
                },
                operands: [Some(ValueId(30)), Some(ValueId(2))],
            }),
        ),
    );
    let s = step(&mut m, 0, 3);
    s.authored_instruction = 2;
    s.instruction = Instruction::Move {
        destination: Destination::Output,
        source: Role::Scratch,
    };
    s.operands = [Some(ValueId(31)), None];
    check(&m).unwrap();
    step(&mut m, 0, 3).operands[0] = Some(ValueId(30));
    invalid(&m);
}
#[test]
fn selector_is_actual_parameter_zero_compare_with_exact_edges() {
    let mut m = diamond();
    blocks(&mut m)[0].operations[2].kind = OperationKind::Constant(Constant::U32(1));
    invalid(&m);
    let mut m = diamond();
    if let OperationKind::Compare { lhs, .. } = &mut blocks(&mut m)[0].operations[3].kind {
        *lhs = ValueId(1);
    }
    invalid(&m);
    let mut m = diamond();
    if let Some(Terminator::ConditionalBranch { then_target, .. }) =
        &mut blocks(&mut m)[0].terminator
    {
        *then_target = BlockId(2);
    }
    invalid(&m);
    let mut m = diamond();
    if let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
        &mut blocks(&mut m)[0].terminator
    {
        then_arguments[0] = ValueId(1);
    }
    invalid(&m);
}
#[test]
fn merge_transports_each_actual_predecessor_definition_in_role_order() {
    let mut m = diamond();
    if let Some(Terminator::Branch { arguments, .. }) = &mut blocks(&mut m)[2].terminator {
        arguments[1] = ValueId(9);
    }
    invalid(&m);
    let mut m = diamond();
    if let Some(Terminator::Branch { arguments, .. }) = &mut blocks(&mut m)[1].terminator {
        arguments.swap(0, 1);
    }
    invalid(&m);
    let mut m = diamond();
    blocks(&mut m)[3].parameters.pop();
    invalid(&m);
    let mut m = diamond();
    blocks(&mut m)[2].operations.clear();
    decl(&mut m).instruction_count = 2;
    invalid(&m);
    let mut m = diamond();
    blocks(&mut m)[1]
        .operations
        .push(op(40, u32_ty(), OperationKind::Constant(Constant::U32(1))));
    invalid(&m);
}
#[test]
fn guarded_tail_requires_real_bounds_address_output_and_memory_access() {
    let mutations: [fn(&mut OperationKind); 6] = [
        |kind| {
            if let OperationKind::GuardedStore { predicate, .. } = kind {
                *predicate = ValueId(1);
            }
        },
        |kind| {
            if let OperationKind::GuardedStore { pointer, .. } = kind {
                *pointer = ValueId(9);
            }
        },
        |kind| {
            if let OperationKind::GuardedStore { value, .. } = kind {
                *value = ValueId(1);
            }
        },
        |kind| {
            if let OperationKind::GuardedStore { access, .. } = kind {
                access.volatile = true;
            }
        },
        |kind| {
            if let OperationKind::GuardedStore { access, .. } = kind {
                access.alignment = 8;
            }
        },
        |kind| {
            if let OperationKind::GuardedStore { access, .. } = kind {
                access.address_space = AddressSpace::Private;
            }
        },
    ];
    for mutate in mutations {
        let mut m = single();
        mutate(&mut blocks(&mut m)[0].operations[7].kind);
        invalid(&m);
    }
    let mut m = single();
    if let OperationKind::Compare { predicate, .. } = &mut blocks(&mut m)[0].operations[4].kind {
        *predicate = ComparePredicate::Equal;
    }
    invalid(&m);
    let mut m = single();
    if let OperationKind::GetElementPointer { offset, .. } =
        &mut blocks(&mut m)[0].operations[6].kind
    {
        *offset = ValueId(1);
    }
    invalid(&m);
    let mut m = single();
    blocks(&mut m)[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    invalid(&m);
}
#[test]
fn maximum_sixteen_authored_steps_and_fixed_work_limit() {
    let mut m = single();
    decl(&mut m).instruction_count = 16;
    for index in 0..15 {
        blocks(&mut m)[0].operations.insert(
            index + 1,
            movement(
                0,
                index as u8,
                40 + index as u32,
                Destination::Scratch,
                Role::Input1,
                2,
            ),
        );
    }
    step(&mut m, 0, 16).authored_instruction = 15;
    check(&m).unwrap();
    let mut work = Work::new(COMPLETE_BODY_PROFILE_WORK_V19 + 6);
    let mut budget = Budget::new(&mut work, 73);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(73).unwrap();
    assert!(matches!(
        check_gfx942_complete_body_function_v19(&m, &m.functions[0], &mut budget),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.work(), 7);
    assert_eq!(budget.storage(), 73);
}
#[test]
fn all_new_operations_are_compiler_ordered_without_fabricated_memory_effects() {
    let m = single();
    for operation in &m.functions[0].body.as_ref().unwrap().blocks[0].operations[..2] {
        assert!(operation.effect_summary().is_pure());
        assert!(
            operation
                .compiler_ordering_effects_v12()
                .has_ordered_region()
        );
        assert!(!operation.combined_effect_summary_v12().is_pure());
    }
    let store = &m.functions[0].body.as_ref().unwrap().blocks[0].operations[7];
    assert!(!store.effect_summary().is_pure());
}

#[test]
fn shared_verifier_accepts_both_graphs_and_restores_nonzero_floor() {
    for module in [single(), diamond()] {
        let mut work = Work::new(2_000_000);
        let mut budget = Budget::new(&mut work, 2_000_000);
        budget.reserve_storage(97).unwrap();
        let result = crate::verify_module_ref_with_budget_v1(&module, None, &mut budget);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(budget.storage(), 97);
        assert!(budget.work() >= COMPLETE_BODY_PROFILE_WORK_V19);
    }
}
#[test]
fn shared_verifier_reports_closed_profile_failures_not_only_local_types() {
    let mut module = diamond();
    // This still satisfies ordinary SSA/type/dominance checking: the input
    // dominates both successors, but is not the authored Scratch edge value.
    if let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
        &mut blocks(&mut module)[0].terminator
    {
        then_arguments[0] = ValueId(1);
    }
    let mut work = Work::new(2_000_000);
    let mut budget = Budget::new(&mut work, 2_000_000);
    budget.reserve_storage(97).unwrap();
    let error = crate::verify_module_ref_with_budget_v1(&module, None, &mut budget)
        .err()
        .unwrap();
    let crate::BorrowedKernelIrVerificationErrorV1::Verification(errors) = error else {
        panic!("expected actual semantic diagnostics");
    };
    assert!(
        errors
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == crate::DiagnosticCode::InvalidCompleteBodyV19)
    );
    assert_eq!(budget.storage(), 97);
}
