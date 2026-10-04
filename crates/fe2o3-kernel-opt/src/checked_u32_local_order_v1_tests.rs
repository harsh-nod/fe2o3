//! Model-owner tests only; no source admission or production schedule claim.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AssemblyConstraint, AssemblyOperand, AssemblyOption,
    AssemblySourceIdentity, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirFunctionCoordinateV1, CastKind, Constant, Function, InlineAssembly,
    InlineAssemblyTarget, MemoryAccess, Operation, Signature, Terminator, UnaryOp, ValueDef,
    ValueId, VerificationContractKeyV12, VerificationContractOperationV12,
    WorkgroupPipelineEventKindV12,
};

const LIMIT: usize = 20_000_000;
const U32: Type = Type::Scalar(ScalarType::U32);
const F: CanonicalKirFunctionCoordinateV1 = CanonicalKirFunctionCoordinateV1(0);

fn binary(result: u32, op: BinaryOp, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(result), U32),
        OperationKind::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}
fn body(module: &mut Module) -> &mut BasicBlock {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0]
}
fn helper(
    parameters: Vec<Type>,
    result: Type,
    operations: Vec<Operation>,
    returned: u32,
) -> Module {
    let ids = (0..parameters.len() as u32).map(ValueId).collect();
    let mut block = BasicBlock::new(BlockId(19));
    block.operations = operations;
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(returned)],
    });
    let mut module = Module::new("local-order-model");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(parameters, vec![result]),
        ids,
        vec![block],
    ));
    module
}
fn source() -> Module {
    helper(
        vec![U32; 4],
        U32,
        vec![
            binary(4, BinaryOp::BitXor, 0, 1),
            binary(5, BinaryOp::BitOr, 2, 3),
            binary(6, BinaryOp::BitAnd, 4, 5),
        ],
        6,
    )
}
fn admit(mut module: Module) -> (Owner, usize) {
    for function in &mut module.functions {
        function.required_capabilities = function.derived_capabilities();
    }
    module.required_capabilities = module
        .functions
        .iter()
        .flat_map(|f| f.required_capabilities.iter().cloned())
        .collect();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, storage.retained_storage())
}
fn region(input: &Owner, count: u32) -> U32LocalOrderRegionV1 {
    U32LocalOrderRegionV1 {
        expected_input: *input.canonical().identity(),
        block: Block {
            function: F,
            block: 0,
        },
        first_operation: 0,
        operation_count: count,
    }
}
fn scheduled(
    input: &Owner,
    retained: usize,
    preference: U32LocalOrderPreferenceV1,
    count: u32,
) -> CheckedU32LocalOrderOutputV1<'_> {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(retained).unwrap();
    let result =
        schedule_checked_u32_local_order_v1(input, region(input, count), preference, &mut budget)
            .unwrap();
    assert_eq!(budget.storage(), retained);
    budget.reserve_storage(result.retained_storage()).unwrap();
    result.replay(&mut budget).unwrap();
    assert_eq!(budget.storage(), retained + result.retained_storage());
    result
}
fn ids(owner: &Owner) -> Vec<u32> {
    owner.module().functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .map(|op| op.results[0].id.0)
        .collect()
}
fn reject(module: Module, count: u32) {
    let (input, retained) = admit(module);
    for preference in [
        U32LocalOrderPreferenceV1::SourceOrder,
        U32LocalOrderPreferenceV1::ReverseReady,
    ] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(retained).unwrap();
        assert!(matches!(
            schedule_checked_u32_local_order_v1(
                &input,
                region(&input, count),
                preference,
                &mut budget
            ),
            Err(Error::UnsupportedOperation)
        ));
        assert_eq!(budget.storage(), retained);
    }
}

#[test]
fn exact_three_operation_orders_are_deterministic_and_input_is_unchanged() {
    let (input, retained) = admit(source());
    let before = input.canonical().canonical_bytes().to_vec();
    let a = scheduled(&input, retained, U32LocalOrderPreferenceV1::SourceOrder, 3);
    let b = scheduled(&input, retained, U32LocalOrderPreferenceV1::ReverseReady, 3);
    let again = scheduled(&input, retained, U32LocalOrderPreferenceV1::ReverseReady, 3);
    assert!(std::ptr::eq(a.input(), &input));
    assert_eq!(ids(a.output()), [4, 5, 6]);
    assert_eq!(ids(b.output()), [5, 4, 6]);
    assert_ne!(
        a.output().canonical().identity(),
        b.output().canonical().identity()
    );
    assert_eq!(
        b.output().canonical().canonical_bytes(),
        again.output().canonical().canonical_bytes()
    );
    assert_eq!(
        b.transition_receipt().canonical_bytes(),
        again.transition_receipt().canonical_bytes()
    );
    assert_eq!(input.canonical().canonical_bytes(), before);
    assert!(!b.grants_authority());
    assert!(!b.transition_receipt().grants_authority());
}

#[test]
fn dependent_chain_never_reorders_before_its_operand() {
    let mut module = source();
    body(&mut module).operations[1] = binary(5, BinaryOp::BitOr, 4, 3);
    let (input, retained) = admit(module);
    let output = scheduled(&input, retained, U32LocalOrderPreferenceV1::ReverseReady, 3);
    assert_eq!(ids(output.output()), [4, 5, 6]);
}

#[test]
fn non_self_inverse_three_cycle_preserves_forward_and_inverse_maps() {
    let mut module = source();
    body(&mut module).operations = vec![
        binary(4, BinaryOp::BitXor, 0, 1),
        binary(5, BinaryOp::BitOr, 4, 3),
        binary(6, BinaryOp::BitAnd, 2, 3),
        binary(7, BinaryOp::BitXor, 5, 6),
    ];
    body(&mut module).terminator = Some(Terminator::Return {
        values: vec![ValueId(7)],
    });
    let (input, retained) = admit(module);
    let output = scheduled(&input, retained, U32LocalOrderPreferenceV1::ReverseReady, 3);
    assert_eq!(ids(output.output()), [6, 4, 5, 7]);
    let origins = output
        .transition_receipt()
        .candidate()
        .operations
        .iter()
        .map(|row| {
            let fe2o3_kernel_ir::CanonicalKirOperationOriginV1::Retained(original) = row.origin
            else {
                panic!("permutation synthesized an operation")
            };
            original.operation
        })
        .collect::<Vec<_>>();
    assert_eq!(origins, [2, 0, 1, 3]);
    let descendants = output
        .transition_receipt()
        .candidate()
        .definition_outputs
        .iter()
        .filter_map(|row| match row.output {
            Definition::Result { operation, .. } => Some(operation.operation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(descendants, [1, 2, 0, 3]);
}

#[test]
fn block_parameters_pre_region_values_and_successor_edge_uses_are_retained() {
    let mut entry = BasicBlock::new(BlockId(19));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(99), U32),
        OperationKind::Constant(Constant::U32(7)),
    ));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(31),
        arguments: (0..4).map(ValueId).collect(),
    });
    let mut selected = BasicBlock::new(BlockId(31));
    selected.parameters = (10..14).map(|id| ValueDef::new(ValueId(id), U32)).collect();
    selected.operations = vec![
        binary(14, BinaryOp::BitXor, 10, 11),
        binary(15, BinaryOp::BitOr, 14, 12),
        binary(16, BinaryOp::BitXor, 13, 99),
        binary(17, BinaryOp::BitAnd, 15, 16),
    ];
    selected.terminator = Some(Terminator::Branch {
        target: BlockId(47),
        arguments: vec![ValueId(17), ValueId(14), ValueId(99)],
    });
    let mut exit = BasicBlock::new(BlockId(47));
    exit.parameters = (20..23).map(|id| ValueDef::new(ValueId(id), U32)).collect();
    exit.operations = vec![
        binary(23, BinaryOp::BitXor, 20, 21),
        binary(24, BinaryOp::BitXor, 23, 22),
    ];
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(24)],
    });
    let mut module = source();
    module.functions[0].body.as_mut().unwrap().blocks = vec![entry, selected, exit];
    let (input, retained) = admit(module);
    let mut selection = region(&input, 3);
    selection.block.block = 1;
    selection.first_operation = 1;
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(retained).unwrap();
    let output = schedule_checked_u32_local_order_v1(
        &input,
        selection,
        U32LocalOrderPreferenceV1::ReverseReady,
        &mut budget,
    )
    .unwrap();
    let before = &input.module().functions[0].body.as_ref().unwrap().blocks;
    let after = &output.output().module().functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks;
    assert_eq!(before[0], after[0]);
    assert_eq!(before[2], after[2]);
    assert_eq!(before[1].parameters, after[1].parameters);
    assert_eq!(before[1].terminator, after[1].terminator);
    assert_eq!(before[1].operations[0], after[1].operations[0]);
    assert_eq!(
        after[1]
            .operations
            .iter()
            .map(|op| op.results[0].id.0)
            .collect::<Vec<_>>(),
        [14, 16, 15, 17]
    );
    assert!(
        output
            .transition_receipt()
            .candidate()
            .edges
            .iter()
            .all(|row| row.input == row.output)
    );
    assert!(
        output
            .transition_receipt()
            .candidate()
            .edge_arguments
            .iter()
            .all(|row| row.input == row.output)
    );
    budget.reserve_storage(output.retained_storage()).unwrap();
    output.replay(&mut budget).unwrap();
    assert_eq!(budget.storage(), retained + output.retained_storage());
}

#[test]
fn nonzero_region_leaves_other_operations_functions_and_cfg_exactly_unchanged() {
    let mut module = source();
    body(&mut module).operations.insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(99), U32),
            OperationKind::Constant(Constant::U32(11)),
        ),
    );
    body(&mut module)
        .operations
        .push(binary(8, BinaryOp::Divide, 0, 1));
    let mut other = source().functions.remove(0);
    other.id = "untouched-helper".into();
    module.functions.push(other);
    let (input, retained) = admit(module);
    let mut selected = region(&input, 3);
    selected.first_operation = 1;
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(retained).unwrap();
    let output = schedule_checked_u32_local_order_v1(
        &input,
        selected,
        U32LocalOrderPreferenceV1::ReverseReady,
        &mut budget,
    )
    .unwrap();
    assert_eq!(ids(output.output()), [99, 5, 4, 6, 8]);
    let original = &input.module().functions[0].body.as_ref().unwrap().blocks[0];
    let scheduled = &output.output().module().functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks[0];
    assert_eq!(original.operations[0], scheduled.operations[0]);
    assert_eq!(original.operations[4], scheduled.operations[4]);
    assert_eq!(original.terminator, scheduled.terminator);
    assert_eq!(
        input.module().functions[1],
        output.output().module().functions[1]
    );
    budget.reserve_storage(output.retained_storage()).unwrap();
    output.replay(&mut budget).unwrap();
    assert_eq!(budget.storage(), retained + output.retained_storage());
}

#[test]
fn changed_output_requires_fresh_exact_selector_and_receipts_do_not_rebind() {
    let (input, retained) = admit(source());
    let output = scheduled(&input, retained, U32LocalOrderPreferenceV1::ReverseReady, 3);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = retained + output.retained_storage();
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        schedule_checked_u32_local_order_v1(
            output.output(),
            region(&input, 3),
            U32LocalOrderPreferenceV1::ReverseReady,
            &mut budget
        ),
        Err(Error::InputIdentity)
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn full_sixty_four_operation_boundary_and_ready_ties_are_fixed() {
    let operations = (0..64)
        .map(|i| binary(i + 2, BinaryOp::BitXor, 0, 1))
        .collect();
    let (input, retained) = admit(helper(vec![U32; 2], U32, operations, 65));
    let output = scheduled(
        &input,
        retained,
        U32LocalOrderPreferenceV1::ReverseReady,
        64,
    );
    assert_eq!(ids(output.output()), (2..66).rev().collect::<Vec<_>>());
}

#[test]
fn region_cannot_escape_block_and_stale_identity_is_not_rebound() {
    let (input, retained) = admit(source());
    let mut changed = source();
    changed.id = "different-source-owner".into();
    let (other, _) = admit(changed);
    for case in 0..8 {
        let mut selected = region(&input, 3);
        match case {
            0 => selected.expected_input = *other.canonical().identity(),
            1 => selected.block.function = CanonicalKirFunctionCoordinateV1(1),
            2 => selected.block.block = 1,
            3 => selected.operation_count = 1,
            4 => selected.operation_count = 65,
            5 => selected.first_operation = 1,
            6 => selected.first_operation = u32::MAX,
            _ => selected.operation_count = 0,
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(retained).unwrap();
        let result = schedule_checked_u32_local_order_v1(
            &input,
            selected,
            U32LocalOrderPreferenceV1::ReverseReady,
            &mut budget,
        );
        assert!(if case == 0 {
            matches!(result, Err(Error::InputIdentity))
        } else {
            matches!(result, Err(Error::RegionBounds))
        });
        assert_eq!(budget.storage(), retained);
    }
}

#[test]
fn signed_and_wide_bitwise_operands_are_not_the_u32_profile() {
    for scalar in [ScalarType::I32, ScalarType::U64] {
        let ty = Type::Scalar(scalar);
        let operations = (2..4)
            .map(|result| {
                Operation::effect_free(
                    ValueDef::new(ValueId(result), ty.clone()),
                    OperationKind::Binary {
                        op: BinaryOp::BitAnd,
                        lhs: ValueId(0),
                        rhs: ValueId(1),
                    },
                )
            })
            .collect();
        reject(helper(vec![ty.clone(); 2], ty, operations, 3), 2);
    }
}

#[test]
fn arithmetic_shifts_floating_point_unary_cast_and_constants_are_excluded() {
    for op in [
        BinaryOp::Add,
        BinaryOp::Subtract,
        BinaryOp::Multiply,
        BinaryOp::Divide,
        BinaryOp::Remainder,
        BinaryOp::ShiftLeft,
        BinaryOp::ShiftRight,
    ] {
        reject(
            helper(
                vec![U32; 2],
                U32,
                vec![binary(2, op, 0, 1), binary(3, op, 0, 1)],
                3,
            ),
            2,
        );
    }
    let float = Type::Scalar(ScalarType::F32);
    reject(
        helper(
            vec![float.clone(); 2],
            float.clone(),
            (2..4)
                .map(|result| {
                    Operation::effect_free(
                        ValueDef::new(ValueId(result), float.clone()),
                        OperationKind::Binary {
                            op: BinaryOp::Add,
                            lhs: ValueId(0),
                            rhs: ValueId(1),
                        },
                    )
                })
                .collect(),
            3,
        ),
        2,
    );
    for kind in [
        OperationKind::Unary {
            op: UnaryOp::Not,
            operand: ValueId(0),
        },
        OperationKind::Constant(Constant::U32(7)),
    ] {
        reject(
            helper(
                vec![U32; 2],
                U32,
                vec![
                    Operation::effect_free(ValueDef::new(ValueId(2), U32), kind),
                    binary(3, BinaryOp::BitAnd, 0, 1),
                ],
                3,
            ),
            2,
        );
    }
    reject(
        helper(
            vec![Type::Scalar(ScalarType::U16), U32],
            U32,
            vec![
                Operation::effect_free(
                    ValueDef::new(ValueId(2), U32),
                    OperationKind::Cast {
                        kind: CastKind::ZeroExtend,
                        value: ValueId(0),
                        to: U32,
                    },
                ),
                binary(3, BinaryOp::BitAnd, 1, 1),
            ],
            3,
        ),
        2,
    );
    reject(
        helper(
            vec![U32; 2],
            U32,
            vec![
                Operation::checked_binary(
                    ValueDef::new(ValueId(2), U32),
                    ValueDef::new(ValueId(3), Type::BOOL),
                    fe2o3_kernel_ir::CheckedBinaryOperator::Add,
                    ValueId(0),
                    ValueId(1),
                ),
                binary(4, BinaryOp::BitAnd, 0, 1),
            ],
            4,
        ),
        2,
    );
}

#[test]
fn all_inline_assembly_is_excluded_even_valid_no_memory_u32() {
    let asm = Operation::effect_free(
        ValueDef::new(ValueId(2), U32),
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: "v_xor_b32".into(),
            operands: vec![
                AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
                AssemblyOperand::input(ValueId(0), AssemblyConstraint::Vgpr32),
                AssemblyOperand::input(ValueId(1), AssemblyConstraint::Vgpr32),
            ],
            options: [AssemblyOption::NoMemory].into(),
            declared_effects: Default::default(),
        }),
    );
    reject(
        helper(
            vec![U32; 2],
            U32,
            vec![asm, binary(3, BinaryOp::BitOr, 0, 1)],
            3,
        ),
        2,
    );
}

#[test]
fn memory_and_order_only_events_are_not_pure_schedule_inputs() {
    let pointer = Type::pointer(U32, AddressSpace::Global, AccessMode::ReadOnly);
    reject(
        helper(
            vec![pointer, U32],
            U32,
            vec![
                Operation::effect_free(
                    ValueDef::new(ValueId(2), U32),
                    OperationKind::Load {
                        pointer: ValueId(0),
                        access: MemoryAccess::new(AddressSpace::Global, 4),
                    },
                ),
                binary(3, BinaryOp::BitOr, 1, 1),
            ],
            3,
        ),
        2,
    );
    let pointer = Type::pointer(U32, AddressSpace::Workgroup, AccessMode::ReadWrite);
    reject(
        helper(
            vec![pointer, Type::INDEX, U32, U32],
            U32,
            vec![
                Operation::new(
                    vec![],
                    OperationKind::VerificationContract(
                        VerificationContractOperationV12::WorkgroupPipelineEvent {
                            contract: VerificationContractKeyV12::new(11),
                            kind: WorkgroupPipelineEventKindV12::Stage,
                            storage: ValueId(0),
                            epoch: ValueId(1),
                        },
                    ),
                ),
                binary(4, BinaryOp::BitOr, 2, 3),
            ],
            4,
        ),
        2,
    );
}

#[test]
fn even_a_pure_helper_call_is_excluded() {
    let mut module = helper(
        vec![U32; 2],
        U32,
        vec![
            Operation::effect_free(
                ValueDef::new(ValueId(2), U32),
                OperationKind::Call {
                    callee: "g".into(),
                    arguments: vec![ValueId(0), ValueId(1)],
                },
            ),
            binary(3, BinaryOp::BitOr, 0, 1),
        ],
        3,
    );
    let mut callee = helper(
        vec![U32; 2],
        U32,
        vec![binary(2, BinaryOp::BitXor, 0, 1)],
        2,
    )
    .functions
    .remove(0);
    callee.id = "g".into();
    module.functions.push(callee);
    reject(module, 2);
}

#[test]
fn output_preference_and_unselected_payload_mutations_fail_exact_replay() {
    let (input, retained) = admit(source());
    for case in 0..3 {
        let mut output = scheduled(&input, retained, U32LocalOrderPreferenceV1::ReverseReady, 3);
        if case == 0 {
            output.preference = U32LocalOrderPreferenceV1::SourceOrder;
        } else {
            let mut module = output.output.module().clone();
            if case == 1 {
                module.id = "unselected-metadata-change".into();
            } else {
                body(&mut module).operations[0] = binary(5, BinaryOp::BitOr, 0, 3);
            }
            output.output = admit(module).0;
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let floor = retained + output.retained_storage();
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            output.replay(&mut budget),
            Err(Error::ExactOutputMismatch)
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn substituted_omitted_duplicated_and_wrong_endpoint_receipts_are_rejected() {
    let (input, retained) = admit(source());
    for case in 0..4 {
        let mut output = scheduled(&input, retained, U32LocalOrderPreferenceV1::ReverseReady, 3);
        let mut operations = output.receipt.candidate().operations.to_vec();
        let mut candidate = output.receipt.candidate();
        match case {
            0 => {
                operations[0].origin = operations[1].origin;
            }
            1 => {
                operations.pop();
            }
            2 => {
                operations.swap(0, 1);
            }
            _ => {}
        }
        candidate.operations = &operations;
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let endpoint = if case == 3 {
            input.canonical().identity()
        } else {
            output.output.canonical().identity()
        };
        output.receipt = InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
            input.canonical().identity(),
            endpoint,
            candidate,
            &mut budget,
        )
        .unwrap()
        .0;
        let floor = retained + output.retained_storage();
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            output.replay(&mut budget),
            Err(Error::Transition(_))
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn exact_and_one_short_budgets_preserve_storage_work_and_failure_history() {
    let (input, retained) = admit(source());
    let floor = retained + 17;
    let (needed_work, peak) = {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(13).unwrap();
        let output = schedule_checked_u32_local_order_v1(
            &input,
            region(&input, 3),
            U32LocalOrderPreferenceV1::ReverseReady,
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(output.retained_storage() > 0);
        (budget.work(), budget.peak_storage())
    };
    for case in 0..3 {
        let mut work = Work::new(needed_work - usize::from(case == 1));
        let mut budget = Budget::new(&mut work, peak - usize::from(case == 2));
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(13).unwrap();
        let result = schedule_checked_u32_local_order_v1(
            &input,
            region(&input, 3),
            U32LocalOrderPreferenceV1::ReverseReady,
            &mut budget,
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(result.is_ok(), case == 0);
        assert!(budget.work() >= 13);
        if case == 0 {
            assert_eq!(budget.work(), needed_work);
        }
        if case == 2 {
            assert!(budget.failed_storage().is_some());
        }
    }
}

#[test]
fn sequential_calls_share_one_work_ledger_and_error_does_not_poison_state() {
    let (input, retained) = admit(source());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(retained).unwrap();
    let mut bad = region(&input, 3);
    bad.operation_count = 1;
    assert!(
        schedule_checked_u32_local_order_v1(
            &input,
            bad,
            U32LocalOrderPreferenceV1::ReverseReady,
            &mut budget
        )
        .is_err()
    );
    let after_failure = budget.work();
    let first = schedule_checked_u32_local_order_v1(
        &input,
        region(&input, 3),
        U32LocalOrderPreferenceV1::ReverseReady,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(first.retained_storage()).unwrap();
    let after_first = budget.work();
    first.replay(&mut budget).unwrap();
    assert!(after_failure > 0 && after_first > after_failure && budget.work() > after_first);
    assert_eq!(budget.storage(), retained + first.retained_storage());
}

#[test]
fn replay_exact_and_one_short_limits_preserve_nonzero_owner_floor() {
    let (input, retained) = admit(source());
    let output = scheduled(&input, retained, U32LocalOrderPreferenceV1::ReverseReady, 3);
    let floor = retained + output.retained_storage() + 17;
    let (needed_work, peak) = {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(13).unwrap();
        output.replay(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        (budget.work(), budget.peak_storage())
    };
    for case in 0..3 {
        let mut work = Work::new(needed_work - usize::from(case == 1));
        {
            let mut budget = Budget::new(&mut work, peak - usize::from(case == 2));
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(13).unwrap();
            let result = output.replay(&mut budget);
            assert_eq!(result.is_ok(), case == 0);
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() >= 13);
            if case == 0 {
                assert_eq!(budget.work(), needed_work);
                assert_eq!(budget.peak_storage(), peak);
            }
            if case == 2 {
                assert_eq!(budget.failed_storage(), Some(peak));
            }
        }
        if case == 1 {
            assert!(work.failed_work().is_some());
        }
    }
    // A denied replay does not mutate either owner or poison a later ledger.
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    output.replay(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
}
