//! Synthetic model owners only; no source, artifact, or production authority.
use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirFunctionCoordinateV1,
    Function, Operation, Signature, Terminator, ValueDef, ValueId,
};

const LIMIT: usize = 20_000_000;
const U32: Type = Type::Scalar(ScalarType::U32);

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

fn source() -> Module {
    let mut block = BasicBlock::new(BlockId(19));
    block.operations = vec![
        binary(4, BinaryOp::BitXor, 0, 1),
        binary(5, BinaryOp::BitOr, 2, 3),
        binary(6, BinaryOp::BitAnd, 4, 5),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(6)],
    });
    let mut module = Module::new("owned-local-order-model");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![U32; 4], vec![U32]),
        (0..4).map(ValueId).collect(),
        vec![block],
    ));
    module
}

fn body(module: &mut Module) -> &mut BasicBlock {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0]
}

fn admit(mut module: Module) -> (Owner, usize) {
    for function in &mut module.functions {
        function.required_capabilities = function.derived_capabilities();
    }
    module.required_capabilities = module
        .functions
        .iter()
        .flat_map(|function| function.required_capabilities.iter().cloned())
        .collect();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, storage.retained_storage())
}

fn region(input: &Owner) -> U32LocalOrderRegionV1 {
    U32LocalOrderRegionV1 {
        expected_input: *input.canonical().identity(),
        block: Block {
            function: CanonicalKirFunctionCoordinateV1(0),
            block: 0,
        },
        first_operation: 0,
        operation_count: 3,
    }
}

fn make(
    input: &Owner,
    input_storage: usize,
    preference: U32LocalOrderPreferenceV1,
) -> OwnedU32LocalOrderContinuationV1 {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(input_storage + 17).unwrap();
    let tail = prepare_owned_u32_local_order_continuation_v1(
        input,
        region(input),
        preference,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), input_storage + 17);
    tail
}

fn ids(owner: &Owner) -> Vec<u32> {
    owner.module().functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .map(|operation| operation.results[0].id.0)
        .collect()
}

#[test]
fn owned_orders_match_existing_service_and_deterministically_replay() {
    let (input, input_storage) = admit(source());
    let initial = input.canonical().canonical_bytes().to_vec();
    for preference in [
        U32LocalOrderPreferenceV1::SourceOrder,
        U32LocalOrderPreferenceV1::ReverseReady,
    ] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let floor = input_storage + 17;
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(13).unwrap();
        let borrowed =
            schedule_checked_u32_local_order_v1(&input, region(&input), preference, &mut budget)
                .unwrap();
        budget.reserve_storage(borrowed.retained_storage()).unwrap();
        let owned = prepare_owned_u32_local_order_continuation_v1(
            &input,
            region(&input),
            preference,
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor + borrowed.retained_storage());
        assert_eq!(owned.input_identity(), input.canonical().identity());
        assert_eq!(owned.region(), region(&input));
        assert_eq!(owned.preference(), preference);
        assert!(!owned.grants_authority());
        assert!(!owned.transition_receipt().grants_authority());
        assert_eq!(
            owned.output().canonical().canonical_bytes(),
            borrowed.output().canonical().canonical_bytes()
        );
        assert_eq!(
            owned.transition_receipt().canonical_bytes(),
            borrowed.transition_receipt().canonical_bytes()
        );
        assert_eq!(
            owned.retained_storage(),
            borrowed.retained_storage() - wrapper::<CheckedU32LocalOrderOutputV1<'_>>().unwrap()
                + wrapper::<OwnedU32LocalOrderContinuationV1>().unwrap()
        );
        budget.reserve_storage(owned.retained_storage()).unwrap();
        let live = budget.storage();
        let before_replay = budget.work();
        owned.replay(&input, &mut budget).unwrap();
        assert_eq!(budget.storage(), live);
        assert!(budget.work() > before_replay);
        let again = prepare_owned_u32_local_order_continuation_v1(
            &input,
            region(&input),
            preference,
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), live);
        assert_eq!(
            owned.output().canonical().canonical_bytes(),
            again.output().canonical().canonical_bytes()
        );
        assert_eq!(
            owned.transition_receipt().canonical_bytes(),
            again.transition_receipt().canonical_bytes()
        );
        assert_eq!(
            ids(owned.output()),
            if preference == U32LocalOrderPreferenceV1::SourceOrder {
                vec![4, 5, 6]
            } else {
                vec![5, 4, 6]
            }
        );
    }
    assert_eq!(input.canonical().canonical_bytes(), initial);
}

#[test]
fn sealed_tail_survives_input_move_but_equal_bytes_do_not_establish_source_custody() {
    let (input, input_storage) = admit(source());
    let tail = make(
        &input,
        input_storage,
        U32LocalOrderPreferenceV1::ReverseReady,
    );
    let original_identity = *input.canonical().identity();
    drop(input);
    let (fresh, fresh_storage) = admit(source());
    assert_eq!(fresh.canonical().identity(), &original_identity);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = fresh_storage + tail.retained_storage() + 17;
    budget.reserve_storage(floor).unwrap();
    tail.replay(&fresh, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(!tail.grants_authority());
}

#[test]
fn private_detach_rejects_equal_byte_foreign_input_without_attaching_tail() {
    let (input, input_storage) = admit(source());
    let (foreign, foreign_storage) = admit(source());
    assert_eq!(input.canonical().identity(), foreign.canonical().identity());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = input_storage + foreign_storage + 17;
    budget.reserve_storage(floor).unwrap();
    let result = scoped(&mut budget, |budget| {
        let borrowed = schedule_checked_u32_local_order_v1(
            &input,
            region(&input),
            U32LocalOrderPreferenceV1::ReverseReady,
            budget,
        )?;
        budget.reserve_storage(borrowed.retained_storage())?;
        detach(&foreign, borrowed, budget)
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn replay_refuses_wrong_input_and_requires_tail_receipt_reserved() {
    let (input, input_storage) = admit(source());
    let tail = make(
        &input,
        input_storage,
        U32LocalOrderPreferenceV1::ReverseReady,
    );
    let mut changed = source();
    changed.id = "other-local-order-model".into();
    let (foreign, foreign_storage) = admit(changed);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(matches!(
        tail.replay(&input, &mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.work(), 0);
    let floor = input_storage + foreign_storage + tail.retained_storage() + 17;
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        tail.replay(&foreign, &mut budget),
        Err(Error::InputIdentity)
    ));
    assert_eq!(budget.storage(), floor);
    tail.replay(&input, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn producer_refuses_invalid_regions_and_unsupported_operations_without_rebinding() {
    let (input, input_storage) = admit(source());
    let mut changed = source();
    changed.id = "other-local-order-model".into();
    let (foreign, _) = admit(changed);
    for case in 0..8 {
        let mut selected = region(&input);
        match case {
            0 => selected.expected_input = *foreign.canonical().identity(),
            1 => selected.operation_count = 1,
            2 => selected.operation_count = 65,
            3 => selected.operation_count = 0,
            4 => selected.first_operation = u32::MAX,
            5 => selected.first_operation = 1,
            6 => selected.block.block = 1,
            _ => selected.block.function = CanonicalKirFunctionCoordinateV1(1),
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let floor = input_storage + 17;
        budget.reserve_storage(floor).unwrap();
        let result = prepare_owned_u32_local_order_continuation_v1(
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
        assert_eq!(budget.storage(), floor);
    }
    let mut unsupported = source();
    body(&mut unsupported).operations[0] = binary(4, BinaryOp::Add, 0, 1);
    let (input, input_storage) = admit(unsupported);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(input_storage + 17).unwrap();
    assert!(matches!(
        prepare_owned_u32_local_order_continuation_v1(
            &input,
            region(&input),
            U32LocalOrderPreferenceV1::ReverseReady,
            &mut budget,
        ),
        Err(Error::UnsupportedOperation)
    ));
    assert_eq!(budget.storage(), input_storage + 17);
}

#[test]
fn complete_reconstruction_rejects_preference_operand_and_unselected_payload_mutations() {
    let (input, input_storage) = admit(source());
    for case in 0..3 {
        let mut tail = make(
            &input,
            input_storage,
            U32LocalOrderPreferenceV1::ReverseReady,
        );
        if case == 0 {
            tail.preference = U32LocalOrderPreferenceV1::SourceOrder;
        } else {
            let mut module = tail.output.module().clone();
            if case == 1 {
                module.id = "other-local-order-model".into();
            } else {
                body(&mut module).operations[0] = binary(5, BinaryOp::BitOr, 0, 3);
            }
            tail.output = admit(module).0;
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let floor = input_storage + tail.retained_storage() + 17;
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            tail.replay(&input, &mut budget),
            Err(Error::ExactOutputMismatch)
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn replay_rejects_foreign_or_omitted_receipt_rows_even_with_actual_output() {
    let (input, input_storage) = admit(source());
    for omit in [false, true] {
        let mut tail = make(
            &input,
            input_storage,
            U32LocalOrderPreferenceV1::ReverseReady,
        );
        let mut operations = tail.receipt.candidate().operations.to_vec();
        if omit {
            operations.pop();
        } else {
            operations[0].origin = operations[1].origin;
        }
        let mut candidate = tail.receipt.candidate();
        candidate.operations = &operations;
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        tail.receipt = InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
            input.canonical().identity(),
            tail.output.canonical().identity(),
            candidate,
            &mut budget,
        )
        .unwrap()
        .0;
        let floor = input_storage + tail.retained_storage() + 17;
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            tail.replay(&input, &mut budget),
            Err(Error::Transition(_))
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn creation_exact_and_one_short_budgets_preserve_nonzero_floor_and_history() {
    let (input, input_storage) = admit(source());
    let floor = input_storage + 17;
    let (needed_work, peak) = {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(13).unwrap();
        let tail = prepare_owned_u32_local_order_continuation_v1(
            &input,
            region(&input),
            U32LocalOrderPreferenceV1::ReverseReady,
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(tail.retained_storage() > 0);
        (budget.work(), budget.peak_storage())
    };
    for case in 0..3 {
        let mut work = Work::new(needed_work - usize::from(case == 1));
        let mut budget = Budget::new(&mut work, peak - usize::from(case == 2));
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(13).unwrap();
        let result = prepare_owned_u32_local_order_continuation_v1(
            &input,
            region(&input),
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
fn replay_exact_and_one_short_budgets_include_fresh_input_admission() {
    let (input, input_storage) = admit(source());
    let tail = make(
        &input,
        input_storage,
        U32LocalOrderPreferenceV1::ReverseReady,
    );
    let floor = input_storage + tail.retained_storage() + 17;
    let (needed_work, peak) = {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(13).unwrap();
        tail.replay(&input, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        (budget.work(), budget.peak_storage())
    };
    for case in 0..3 {
        let mut work = Work::new(needed_work - usize::from(case == 1));
        let mut budget = Budget::new(&mut work, peak - usize::from(case == 2));
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(13).unwrap();
        assert_eq!(tail.replay(&input, &mut budget).is_ok(), case == 0);
        assert_eq!(budget.storage(), floor);
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
fn fresh_input_canonicalization_cost_is_not_hidden_by_a_digest_comparison() {
    let (input, input_storage) = admit(source());
    let tail = make(
        &input,
        input_storage,
        U32LocalOrderPreferenceV1::ReverseReady,
    );
    let floor = input_storage + tail.retained_storage() + 17;
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    let before = budget.work();
    scoped(&mut budget, |budget| {
        check_input(&input, tail.input_identity(), budget)
    })
    .unwrap();
    let canonical_work = budget.work() - before;
    assert!(canonical_work > input.canonical().canonical_bytes().len());
    assert_eq!(budget.storage(), floor);
    let before = budget.work();
    tail.replay(&input, &mut budget).unwrap();
    assert!(budget.work() - before > canonical_work);
    assert_eq!(budget.storage(), floor);
    let mut work = Work::new(canonical_work - 1);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    assert!(
        scoped(&mut budget, |budget| check_input(
            &input,
            tail.input_identity(),
            budget
        ))
        .is_err()
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn sequential_production_replay_and_drop_keep_one_ledger_and_exact_transfer_receipts() {
    let (input, input_storage) = admit(source());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = input_storage + 17;
    budget.reserve_storage(floor).unwrap();
    let mut bad = region(&input);
    bad.operation_count = 1;
    assert!(
        prepare_owned_u32_local_order_continuation_v1(
            &input,
            bad,
            U32LocalOrderPreferenceV1::ReverseReady,
            &mut budget,
        )
        .is_err()
    );
    let rejected_work = budget.work();
    let first = prepare_owned_u32_local_order_continuation_v1(
        &input,
        region(&input),
        U32LocalOrderPreferenceV1::SourceOrder,
        &mut budget,
    )
    .unwrap();
    assert!(budget.work() > rejected_work);
    let first_storage = first.retained_storage();
    budget.reserve_storage(first_storage).unwrap();
    let first_work = budget.work();
    let second = prepare_owned_u32_local_order_continuation_v1(
        &input,
        region(&input),
        U32LocalOrderPreferenceV1::ReverseReady,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor + first_storage);
    assert!(budget.work() > first_work);
    let second_storage = second.retained_storage();
    budget.reserve_storage(second_storage).unwrap();
    first.replay(&input, &mut budget).unwrap();
    second.replay(&input, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor + first_storage + second_storage);
    drop(first);
    budget.release_storage(first_storage).unwrap();
    drop(second);
    budget.release_storage(second_storage).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn maximum_region_and_non_self_inverse_permutation_use_the_existing_engine() {
    let mut module = source();
    body(&mut module).operations = (0..64)
        .map(|index| binary(index + 4, BinaryOp::BitXor, 0, 1))
        .collect();
    body(&mut module).terminator = Some(Terminator::Return {
        values: vec![ValueId(67)],
    });
    let (input, input_storage) = admit(module);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(input_storage + 17).unwrap();
    let mut selected = region(&input);
    selected.operation_count = 64;
    let tail = prepare_owned_u32_local_order_continuation_v1(
        &input,
        selected,
        U32LocalOrderPreferenceV1::ReverseReady,
        &mut budget,
    )
    .unwrap();
    assert_eq!(ids(tail.output()), (4..68).rev().collect::<Vec<_>>());
    budget.reserve_storage(tail.retained_storage()).unwrap();
    let floor = budget.storage();
    tail.replay(&input, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);

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
    let (input, input_storage) = admit(module);
    let tail = make(
        &input,
        input_storage,
        U32LocalOrderPreferenceV1::ReverseReady,
    );
    assert_eq!(ids(tail.output()), [6, 4, 5, 7]);
    let origins = tail
        .transition_receipt()
        .candidate()
        .operations
        .iter()
        .map(|row| {
            let fe2o3_kernel_ir::CanonicalKirOperationOriginV1::Retained(operation) = row.origin
            else {
                panic!("local permutation must retain every operation");
            };
            operation.operation
        })
        .collect::<Vec<_>>();
    assert_eq!(origins, [2, 0, 1, 3]);
}
