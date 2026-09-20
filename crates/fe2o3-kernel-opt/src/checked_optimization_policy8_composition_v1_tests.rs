//! Child of the existing P7 tests: reuse its genuine consumed prefix fixture.
#[path = "checked_optimization_policy8_transport_v1_tests.rs"]
mod transport;
use super::{Prepared, STORAGE, WORK, encode, fixture, prepared_module_with_work};
use crate::*;
use fe2o3_kernel_ir::{
    AddressSpace, BinaryOp, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, InertCanonicalKirTransitionGraphIdentityV1 as Identity,
    MemoryAccess, Module, Operation, OperationKind as Kind, ScalarType, Type, ValueDef, ValueId,
};
use fe2o3_pliron::{
    OwnedCommutativeBitwiseContinuationV1 as Tail,
    prepare_owned_commutative_bitwise_continuation_v1 as prepare_tail,
};
use std::mem::{size_of, size_of_val};
type Error = CanonicalPolicy8CompositionErrorV1;

struct Prepared8 {
    prefix: Prepared,
    tail: Tail,
    floor: usize,
}
impl Prepared8 {
    fn inputs(&self) -> CanonicalPolicy8SemanticInputsV1<'_> {
        let prefix = self.prefix.inputs();
        CanonicalPolicy8SemanticInputsV1 {
            prefix,
            output: self.tail.output(),
            continuation: CanonicalPolicy8ContinuationClaimsV1 {
                pass_name: POLICY8_COMMUTATIVE_PASS_NAME_V1,
                input: Identity::from_verified(prefix.output.canonical().identity()),
                output: Identity::from_verified(self.tail.output().canonical().identity()),
                occurrences: self.tail.occurrences().candidate(),
            },
        }
    }
}

fn prepared_module(module: &Module) -> Prepared8 {
    // Existing P7 fixture preparation allowance, separate from adapter verification.
    let (prefix, _) = prepared_module_with_work(module, 1_000_000_000);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(prefix.floor).unwrap();
    let tail = prepare_tail(prefix.inputs().output, &mut budget).unwrap();
    assert_eq!(budget.storage(), prefix.floor);
    let floor = prefix.floor + tail.retained_storage();
    Prepared8 {
        prefix,
        tail,
        floor,
    }
}
fn prepared(store_deletion: bool, swapped_pair: bool) -> Prepared8 {
    let mut module = fixture();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    if !store_deletion {
        block.operations.remove(4);
        block.operations.remove(3);
    }
    for (id, lhs, rhs) in
        [(20, 0, 1), (21, 1, 0)]
            .into_iter()
            .take(if swapped_pair { 2 } else { 1 })
    {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
            Kind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        ));
        // Both values stay live through all earlier policies; these are global
        // effects, not Policy7's private-cell store-deletion candidates.
        block.operations.push(Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(2),
                value: ValueId(id),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    let p = prepared_module(&module);
    assert_eq!(
        p.prefix.continuation.rows().len(),
        if store_deletion { 2 } else { 0 }
    );
    assert_eq!(p.tail.proved_pairs(), usize::from(swapped_pair));
    assert_eq!(p.tail.execution().changed(), swapped_pair);
    p
}

struct Observation {
    result: Result<(usize, usize), Error>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn run(
    inputs: CanonicalPolicy8SemanticInputsV1<'_>,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> Observation {
    let mut work = Work::new(work_limit);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result =
            check_published_policy8_semantic_relation_v1(inputs, &mut budget).map(|receipt| {
                assert!(std::ptr::eq(
                    receipt
                        .policy7_relation()
                        .continuation()
                        .relation()
                        .output(),
                    inputs.prefix.output
                ));
                assert!(std::ptr::eq(
                    receipt.continuation().input(),
                    inputs.prefix.output
                ));
                assert!(std::ptr::eq(receipt.output(), inputs.output));
                assert!(!receipt.authenticates_execution());
                assert!(!receipt.grants_authority());
                assert!(!receipt.continuation().authenticates_execution());
                let retained = receipt.storage().retained_storage();
                assert_eq!(
                    retained,
                    receipt.policy7_relation().storage().retained_storage()
                        + receipt.continuation().storage().retained_storage()
                        + size_of_val(&receipt)
                        - size_of_val(receipt.policy7_relation())
                        - size_of_val(receipt.continuation())
                );
                (retained, receipt.continuation().proved_pairs())
            });
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    Observation {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

#[test]
fn genuine_full_prefix_and_tail_keep_store_deletions_distinct_from_swaps() {
    for stores in [false, true] {
        for swaps in [false, true] {
            let p = prepared(stores, swaps);
            let j = p.inputs().prefix.output.canonical().canonical_bytes();
            let k = p.tail.output().canonical().canonical_bytes();
            assert_eq!(j != k, swaps);
            let result = run(p.inputs(), p.floor, WORK, STORAGE).result.unwrap();
            assert_eq!(result.1, usize::from(swaps));
        }
    }
}

#[test]
fn empty_complete_prefix_and_tail_remain_zero_pair_semantics() {
    let p = prepared_module(&Module::new("empty-full-policy8"));
    assert!(p.prefix.continuation.rows().is_empty());
    assert!(p.tail.occurrences().candidate().operations.is_empty());
    assert_eq!(p.tail.proved_pairs(), 0);
    assert!(!p.tail.execution().changed());
    let result = run(p.inputs(), p.floor, WORK, STORAGE).result.unwrap();
    assert_eq!(result.1, 0);
}

#[test]
fn composition_matches_one_manual_prefix_and_one_tail_on_one_ledger() {
    let p = prepared(true, true);
    let inputs = p.inputs();
    let composed = run(inputs, p.floor, WORK, STORAGE);
    let expected = composed.result.unwrap();
    let wrapper = size_of::<ReplayedPolicy8SemanticRelationV1<'_>>()
        - size_of::<ReplayedPolicy7SemanticRelationV1<'_>>()
        - size_of::<CheckedCanonicalPolicy8ContinuationRelationV1<'_, '_, '_>>();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(p.floor).unwrap();
    budget.charge_work(17).unwrap();
    budget.reserve_storage(wrapper).unwrap();
    let retained;
    {
        let prefix =
            check_published_policy7_semantic_relation_v1(inputs.prefix, &mut budget).unwrap();
        budget
            .reserve_storage(prefix.storage().retained_storage())
            .unwrap();
        let continuation = check_canonical_policy8_continuation_relation_v1(
            inputs.prefix.output,
            inputs.output,
            inputs.continuation,
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(continuation.storage().retained_storage())
            .unwrap();
        retained = wrapper
            + prefix.storage().retained_storage()
            + continuation.storage().retained_storage();
        assert_eq!((retained, continuation.proved_pairs()), expected);
        assert_eq!(budget.work(), composed.work);
        assert_eq!(budget.peak_storage(), composed.peak);
    }
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), p.floor);
}

#[test]
fn deterministic_exact_work_storage_and_one_short_failures_preserve_floor() {
    let p = prepared(true, true);
    let baseline = run(p.inputs(), p.floor, WORK, STORAGE);
    let expected = baseline.result.unwrap();
    assert!(baseline.work > 17);
    assert!(baseline.peak > p.floor + expected.0);
    for _ in 0..2 {
        let actual = run(p.inputs(), p.floor, baseline.work, baseline.peak);
        assert_eq!(actual.result.unwrap(), expected);
        assert_eq!((actual.work, actual.peak), (baseline.work, baseline.peak));
        assert_eq!((actual.failed_work, actual.failed_storage), (None, None));
    }
    let actual = run(p.inputs(), p.floor, baseline.work - 1, baseline.peak);
    assert!(actual.result.is_err());
    assert!(
        actual
            .failed_work
            .is_some_and(|attempt| attempt > baseline.work - 1)
    );
    assert_eq!(actual.failed_storage, None);
    assert!(actual.work >= 17 && actual.work < baseline.work);
    let actual = run(p.inputs(), p.floor, baseline.work, baseline.peak - 1);
    assert!(actual.result.is_err());
    assert!(
        actual
            .failed_storage
            .is_some_and(|attempt| attempt > baseline.peak - 1)
    );
    assert_eq!(actual.failed_work, None);
    assert!(actual.peak < baseline.peak);
}

#[test]
fn wrong_j_locator_and_reframed_stale_k_fail_after_valid_prefix() {
    let p = prepared(true, true);
    let mut inputs = p.inputs();
    inputs.continuation.input =
        Identity::from_verified(p.prefix.checked.owner().canonical().identity());
    assert!(matches!(
        run(inputs, p.floor, WORK, STORAGE).result,
        Err(Error::Continuation(
            CanonicalPolicy8SemanticErrorV1::InputIdentity
        ))
    ));
    let mut inputs = p.inputs();
    inputs.output = inputs.prefix.output;
    inputs.continuation.output = Identity::from_verified(inputs.output.canonical().identity());
    assert!(matches!(
        run(inputs, p.floor, WORK, STORAGE).result,
        Err(Error::Continuation(
            CanonicalPolicy8SemanticErrorV1::Continuation(_)
        ))
    ));
    let mut inputs = p.inputs();
    inputs.prefix.output = p.prefix.checked.owner();
    assert!(matches!(
        run(inputs, p.floor, WORK, STORAGE).result,
        Err(Error::Policy7(_))
    ));
}

#[test]
fn changed_prefix_records_and_rows_are_not_bypassed_by_valid_tail() {
    let p = prepared(true, true);
    let mut record = p.prefix.record.clone();
    record[0] ^= 1;
    let mut inputs = p.inputs();
    inputs.prefix.continuation.execution_record = &record;
    assert!(matches!(
        run(inputs, p.floor + record.capacity(), WORK, STORAGE).result,
        Err(Error::Policy7(_))
    ));
    let mut integer = p
        .prefix
        .checked
        .continuation()
        .execution()
        .canonical_bytes()
        .to_vec();
    integer[0] ^= 1;
    let mut inputs = p.inputs();
    inputs.prefix.prefix.continuation.integer_record = &integer;
    assert!(matches!(
        run(inputs, p.floor + integer.capacity(), WORK, STORAGE).result,
        Err(Error::Policy7(_))
    ));
    let mut rows = p.prefix.continuation.rows().to_vec();
    assert!(!rows.is_empty());
    rows[0].anchor.operation = u32::MAX;
    let record = encode(
        p.prefix.checked.execution().canonical_bytes(),
        p.prefix.checked.owner(),
        p.prefix.continuation.output(),
        &rows,
        p.prefix.continuation.retained_operations(),
    );
    let mut inputs = p.inputs();
    inputs.prefix.continuation.execution_record = &record;
    inputs.prefix.continuation.deletion_rows = &rows;
    assert!(matches!(
        run(
            inputs,
            p.floor + record.capacity() + rows.capacity() * size_of_val(&rows[0]),
            WORK,
            STORAGE
        )
        .result,
        Err(Error::Policy7(_))
    ));
}

#[test]
fn tail_refusal_drops_already_checked_prefix_and_preserves_cumulative_work() {
    let p = prepared(true, true);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(p.floor).unwrap();
    budget.charge_work(17).unwrap();
    let prefix =
        check_published_policy7_semantic_relation_v1(p.inputs().prefix, &mut budget).unwrap();
    let prefix_work = budget.work();
    drop(prefix);
    let mut inputs = p.inputs();
    inputs.continuation.pass_name = "not-the-fixed-commutative-pass";
    let refused = run(inputs, p.floor, WORK, STORAGE);
    assert!(matches!(
        refused.result,
        Err(Error::Continuation(
            CanonicalPolicy8SemanticErrorV1::PassIdentity
        ))
    ));
    assert_eq!(refused.work, prefix_work + 3);
    let mut inputs = p.inputs();
    inputs.continuation.occurrences.operations = &[];
    let refused = run(inputs, p.floor, WORK, STORAGE);
    assert!(matches!(
        refused.result,
        Err(Error::Continuation(
            CanonicalPolicy8SemanticErrorV1::Continuation(_)
        ))
    ));
    assert!(refused.work > prefix_work);
}

#[test]
fn composed_receipt_transfer_keeps_all_borrowed_owners_live_until_drop() {
    let p = prepared(true, true);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(p.floor).unwrap();
    let receipt = check_published_policy8_semantic_relation_v1(p.inputs(), &mut budget).unwrap();
    assert_eq!(budget.storage(), p.floor);
    let retained = receipt.storage().retained_storage();
    budget.reserve_storage(retained).unwrap();
    assert_eq!(receipt.continuation().proved_pairs(), 1);
    assert!(std::ptr::eq(receipt.output(), p.tail.output()));
    drop(receipt);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), p.floor);
    let floor = p.floor;
    drop(p);
    budget.release_storage(floor).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn wrapper_storage_is_prepaid_before_any_prefix_work() {
    let p = prepared(true, true);
    let actual = run(p.inputs(), p.floor, WORK, p.floor);
    assert!(matches!(
        actual.result,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(actual.work, 17);
    assert_eq!(actual.peak, p.floor);
    assert!(actual.failed_storage.is_some());
}
