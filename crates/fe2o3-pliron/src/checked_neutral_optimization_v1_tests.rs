//! Real observed execution is the only successful input to checked adoption.
//! These tests qualify custody/local rules, not production or formal authority.

use super::*;
use crate::KirPlironGraphV12;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, Constant, Function, Module, Operation,
    OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};
use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 1_000_000_000;
const PREFIX: usize = 37;
const PRIOR_WORK: usize = 3;

fn module(literal: u32) -> Module {
    let mut block = BasicBlock::new(BlockId(40));
    for (id, value) in [(0, literal), (1, 99)] {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(value)),
        ));
    }
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    let mut module = Module::new("checked-neutral-owner");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
        vec![],
        vec![block],
    ));
    module
}

struct Input {
    owner: Owner,
    storage: usize,
}
fn make_input(literal: u32) -> Input {
    make_input_from_module(&module(literal))
}
fn make_input_from_module(module: &Module) -> Input {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    Input {
        owner,
        storage: storage.retained_storage(),
    }
}

fn observed<'input>(
    input: &'input Input,
    budget: &mut Budget<'_>,
) -> KirNeutralOptimizationOutputV1<'input> {
    let floor = budget.storage();
    let (mut graph, storage) = KirPlironGraphV12::import(&input.owner, budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let output = graph
        .execute_production_neutral_optimization_v1(budget)
        .unwrap()
        .extract()
        .unwrap();
    budget
        .reserve_storage(output.storage().retained_storage())
        .unwrap();
    let graph_storage = graph.retained_storage();
    drop(graph);
    budget.release_storage(graph_storage).unwrap();
    assert_eq!(
        budget.storage(),
        floor + output.storage().retained_storage()
    );
    output
}

// Explicitly transfer this prepared observation into a separate ledger only
// for exact adoption-boundary resource tests. The end-to-end test below instead
// keeps import, execution, extraction and adoption on one ledger.
fn prepared(input: &Input) -> KirNeutralOptimizationOutputV1<'_> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(input.storage).unwrap();
    let output = observed(input, &mut budget);
    budget
        .release_storage(input.storage + output.storage().retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 0);
    output
}

#[test]
fn actual_capture_becomes_move_only_checked_successor_with_owned_origins() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(PREFIX).unwrap();
    budget.charge_work(PRIOR_WORK).unwrap();
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(&module(7), &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let input = Input {
        owner,
        storage: storage.retained_storage(),
    };
    let observation = observed(&input, &mut budget);
    let exact_input = input.owner.canonical().canonical_bytes();
    let input_audit_len = exact_input.len();
    let exact_output = observation.owner().canonical().canonical_bytes();
    assert_ne!(exact_input, exact_output);
    let output_bytes = exact_output.to_vec(); // Test oracle, outside compiler accounting.
    let called = Cell::new(false);
    let before = budget.work();
    let (checked, origins, origins_storage) = observation
        .try_check_and_finish_with_v1(&mut budget, |transition, budget| {
            called.set(true);
            assert!(transition.input().belongs_to(&input.owner));
            assert_eq!(
                transition.output().owner().canonical().canonical_bytes(),
                output_bytes
            );
            assert!(!transition.grants_authority());
            let floor = budget.storage();
            let retained = size_of::<Vec<usize>>() + 2 * size_of::<usize>();
            budget.reserve_storage(retained).unwrap();
            budget.charge_work(2).unwrap();
            let mut origins = Vec::new();
            origins.try_reserve_exact(2).unwrap();
            origins.extend_from_slice(&[
                transition.input().operations().len(),
                transition.output().operations().len(),
            ]);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), floor);
            Ok::<_, Infallible>((origins, retained))
        })
        .unwrap();
    assert_eq!(budget.storage(), PREFIX + input.storage);
    let checked_storage = checked.storage().retained_storage();
    budget.reserve_storage(checked_storage).unwrap();
    budget
        .reserve_storage(origins_storage.retained_storage())
        .unwrap();
    assert!(called.get());
    assert!(budget.work() > before);
    assert_eq!(origins, [2, 1]);
    assert_eq!(checked.native_input_audit_bytes(), exact_input);
    assert_eq!(checked.owner().canonical().canonical_bytes(), output_bytes);
    assert!(checked.map().matches_execution(checked.report()));
    assert_eq!(checked.report().passes().len(), 7);
    assert!(!checked.grants_authority());
    assert!(!origins_storage.grants_authority());
    let input_storage = input.storage;
    drop(input);
    budget.release_storage(input_storage).unwrap();
    // Checked output remains valid after the original connected owner is gone.
    assert_eq!(checked.native_input_audit_bytes().len(), input_audit_len);
    assert_eq!(checked.owner().canonical().canonical_bytes(), output_bytes);
    drop(origins);
    budget
        .release_storage(origins_storage.retained_storage())
        .unwrap();
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    assert_eq!(budget.storage(), PREFIX);
}

#[test]
fn fixed_checker_rejects_owner_substitution_before_callback() {
    let input = make_input(7);
    let other = make_input(8);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(PREFIX + input.storage + other.storage)
        .unwrap();
    let mut observation = observed(&input, &mut budget);
    // Private hostile fixture; the public API exposes no replacement input.
    observation.input = &other.owner;
    let called = Cell::new(false);
    let result = observation.try_check_and_finish_with_v1(&mut budget, |_, _| {
        called.set(true);
        Ok::<_, Infallible>(((), 0))
    });
    assert!(matches!(
        result,
        Err(KirCheckedNeutralOptimizationErrorV1::Transition(_))
    ));
    assert!(!called.get());
    assert_eq!(budget.storage(), PREFIX + input.storage + other.storage);
}

#[test]
fn callback_error_and_panic_drop_owned_scratch_and_restore_floor() {
    let input = make_input(7);
    for panics in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(PREFIX + input.storage).unwrap();
        let observation = observed(&input, &mut budget);
        let before = budget.work();
        let result = observation.try_check_and_finish_with_v1(&mut budget, |_, budget| {
            let retained = size_of::<Vec<u8>>() + 83;
            budget.reserve_storage(retained).unwrap();
            budget.charge_work(83).unwrap();
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(83).unwrap();
            bytes.resize(83, 4u8);
            if panics {
                panic!("intentional origin callback panic");
            }
            drop(bytes);
            budget.release_storage(retained).unwrap();
            Err::<((), usize), _>("origin failure")
        });
        if panics {
            assert!(matches!(
                result,
                Err(KirCheckedNeutralOptimizationErrorV1::Panicked)
            ));
        } else {
            assert!(matches!(
                result,
                Err(KirCheckedNeutralOptimizationErrorV1::Origin(
                    "origin failure"
                ))
            ));
        }
        assert_eq!(budget.storage(), PREFIX + input.storage);
        assert!(budget.work() > before);
    }
}

#[test]
fn callback_unbalanced_or_undersized_transfer_is_rejected() {
    let input = make_input(7);
    for case in 0..4 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(PREFIX + input.storage).unwrap();
        let observation = observed(&input, &mut budget);
        let result = observation.try_check_and_finish_with_v1(&mut budget, |_, budget| {
            match case {
                0 => budget.reserve_storage(1).unwrap(),
                1 => budget.release_storage(1).unwrap(),
                2 => budget.release_storage(budget.storage()).unwrap(),
                _ => (),
            }
            Ok::<_, Infallible>((17usize, if case == 3 { 0 } else { size_of::<usize>() }))
        });
        assert!(matches!(
            result,
            Err(KirCheckedNeutralOptimizationErrorV1::OriginAccounting)
        ));
        assert_eq!(budget.storage(), PREFIX + input.storage);
    }
}

fn measured_adoption(
    input: &Input,
    work_limit: usize,
    storage_limit: usize,
) -> (bool, usize, usize, Option<usize>, Option<usize>) {
    let observation = prepared(input);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget
        .reserve_storage(PREFIX + input.storage + observation.storage().retained_storage())
        .unwrap();
    budget.charge_work(PRIOR_WORK).unwrap();
    let result = observation.try_check_and_finish_v1(&mut budget);
    assert_eq!(budget.storage(), PREFIX + input.storage);
    let succeeded = result.is_ok();
    if let Ok(owner) = result {
        let retained = owner.storage().retained_storage();
        budget.reserve_storage(retained).unwrap();
        drop(owner);
        budget.release_storage(retained).unwrap();
    }
    let (accepted, peak, failed_storage) = (
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    );
    assert_eq!(budget.storage(), PREFIX + input.storage);
    (
        succeeded,
        accepted,
        peak,
        failed_storage,
        work.failed_work(),
    )
}

#[test]
fn exact_and_one_under_work_and_storage_preserve_nonzero_owner_floors() {
    let input = make_input(7);
    let measured = measured_adoption(&input, WORK, STORAGE);
    assert!(measured.0);
    assert!(measured.1 > PRIOR_WORK);
    let exact = measured_adoption(&input, measured.1, measured.2);
    assert_eq!(exact, measured);
    let work_short = measured_adoption(&input, measured.1 - 1, measured.2);
    assert!(!work_short.0);
    assert!(work_short.1 < measured.1);
    assert_eq!(work_short.4, Some(measured.1));
    let storage_short = measured_adoption(&input, measured.1, measured.2 - 1);
    assert!(!storage_short.0);
    assert!(storage_short.2 < measured.2);
    assert_eq!(storage_short.3, Some(measured.2));
}

#[test]
fn empty_owner_boundaries_follow_independent_header_and_work_equations() {
    use fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1;

    const NAME: &str = "m";
    const AUDIT_BYTES: usize = 37;
    const INCREMENTAL_WORK: usize = 49;
    let input = make_input_from_module(&Module::new(NAME));
    assert_eq!(input.owner.canonical().canonical_bytes().len(), AUDIT_BYTES);
    // Each empty inventory charges census(1) and fill(1); all allocation,
    // sorting, linking and roster loops are empty. The checker charges entry,
    // State::new, module header, capability header and one solve iteration,
    // plus the two module-name lengths. Empty array initializers charge zero.
    let before_copy_work = PRIOR_WORK + 1 + 2 * (1 + 1) + 5 + 2 * NAME.len();
    assert_eq!(
        before_copy_work + AUDIT_BYTES,
        PRIOR_WORK + INCREMENTAL_WORK
    );
    let exact_work = PRIOR_WORK + INCREMENTAL_WORK;
    let inventory_header = size_of::<CanonicalKirInventoryV1<'_>>();
    let checked_header = size_of::<CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>>();
    // Independently enumerate State's two inventory references, candidate
    // slice header and eighteen Vec headers. Every row vector is empty, so
    // there are no element allocations and no hidden target-dependent IDs.
    let state_header = 2 * size_of::<&CanonicalKirInventoryV1<'_>>()
        + size_of::<CanonicalKirTransitionCandidateV1<'_>>()
        + 18 * size_of::<Vec<usize>>();
    let scratch_peak = 2 * inventory_header + checked_header + state_header;
    let final_extra = checked_wrapper_storage_v1().unwrap()
        + size_of::<KirNeutralOwnedOriginStorageV1>()
        + AUDIT_BYTES;
    assert!(scratch_peak > final_extra);

    for case in 0..3 {
        let observation = prepared(&input);
        let caller_floor = PREFIX + input.storage;
        let entry_floor = caller_floor + observation.storage().retained_storage();
        let exact_storage = entry_floor + scratch_peak;
        let work_limit = exact_work - usize::from(case == 1);
        let storage_limit = exact_storage - usize::from(case == 2);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(entry_floor).unwrap();
        budget.charge_work(PRIOR_WORK).unwrap();
        let result = observation.try_check_and_finish_v1(&mut budget);
        assert_eq!(budget.storage(), caller_floor);
        match case {
            0 => {
                let checked = result.unwrap();
                let retained = checked.storage().retained_storage();
                budget.reserve_storage(retained).unwrap();
                assert_eq!(budget.work(), exact_work);
                assert_eq!(budget.peak_storage(), exact_storage);
                assert_eq!(budget.failed_storage(), None);
                drop(checked);
                budget.release_storage(retained).unwrap();
                assert_eq!(work.failed_work(), None);
            }
            1 => {
                assert!(matches!(
                    result,
                    Err(KirCheckedNeutralOptimizationErrorV1::Resource(
                        Resource::Work(_)
                    ))
                ));
                assert_eq!(budget.work(), before_copy_work);
                assert_eq!(budget.peak_storage(), exact_storage);
                assert_eq!(work.failed_work(), Some(exact_work));
            }
            _ => {
                assert!(matches!(
                    result,
                    Err(KirCheckedNeutralOptimizationErrorV1::Transition(
                        CanonicalKirTransitionErrorV1::Resource(Resource::Storage(_))
                    ))
                ));
                assert_eq!(budget.work(), PRIOR_WORK + 1 + 2 * (1 + 1) + 1 + 1);
                assert_eq!(budget.peak_storage(), exact_storage - state_header);
                assert_eq!(budget.failed_storage(), Some(exact_storage));
                assert_eq!(work.failed_work(), None);
            }
        }
    }
}

struct DropWitness(Arc<AtomicBool>);
impl Drop for DropWitness {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn failure_after_callback_transfer_drops_origin_and_preserves_first_failure() {
    let input = make_input(7);
    let dropped = Arc::new(AtomicBool::new(false));
    let observation = prepared(&input);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(PREFIX + input.storage + observation.storage().retained_storage())
        .unwrap();
    let result = observation.try_check_and_finish_with_v1(&mut budget, |_, budget| {
        let retained = size_of::<DropWitness>();
        budget.reserve_storage(retained).unwrap();
        let owned = DropWitness(Arc::clone(&dropped));
        budget.release_storage(retained).unwrap();
        // Explicitly consume the remaining fixed allowance on this very
        // ledger. No successful adoption is run to calibrate the failure.
        budget.charge_work(WORK - budget.work()).unwrap();
        Ok::<_, Infallible>((owned, retained))
    });
    assert!(matches!(
        result,
        Err(KirCheckedNeutralOptimizationErrorV1::Resource(
            Resource::Work(_)
        ))
    ));
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(budget.storage(), PREFIX + input.storage);
    let failed_at = WORK + input.owner.canonical().canonical_bytes().len();
    assert_eq!(budget.work(), WORK);
    assert!(budget.charge_work(usize::MAX).is_err());
    assert_eq!(work.failed_work(), Some(failed_at));
}

#[test]
fn callback_transfer_is_reserved_before_any_audit_allocation() {
    let input = make_input(7);
    let observation = prepared(&input);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(PREFIX + input.storage + observation.storage().retained_storage())
        .unwrap();
    let callback_work = Cell::new(0);
    let result = observation.try_check_and_finish_with_v1(&mut budget, |_, budget| {
        callback_work.set(budget.work());
        Ok::<_, Infallible>(((), usize::MAX))
    });
    assert!(matches!(
        result,
        Err(KirCheckedNeutralOptimizationErrorV1::Resource(
            Resource::Storage(_)
        ))
    ));
    assert_eq!(budget.storage(), PREFIX + input.storage);
    assert_eq!(budget.work(), callback_work.get());
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
}

#[test]
fn wrapper_storage_replaces_observed_header_without_double_counting_components() {
    let expected = size_of::<CheckedNeutralKernelIrOwnerV1>()
        - size_of::<Owner>()
        - size_of::<PlironOptimizationReportV1>()
        - size_of::<KirBridgeOptimizedReceiptV1>()
        - size_of::<KirOptimizationMapV12>()
        - size_of::<KirNeutralOccurrenceRowsV1>();
    assert_eq!(checked_wrapper_storage_v1(), Some(expected));
    assert!(expected >= size_of::<Vec<u8>>() + size_of::<KirCheckedNeutralOptimizationStorageV1>());
}
