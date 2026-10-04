use super::*;
use crate::{
    checked_optimization_policy4_v1::tests::fixture,
    optimize_checked_canonical_kernel_ir_policy4_v1,
};
use fe2o3_kernel_ir::{BinaryOp, CanonicalKernelIrWorkBudgetV1 as Work, Module, OperationKind};

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 128 * 1024 * 1024;
const PREFIX: usize = 19;

fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    (owner, receipt.retained_storage())
}

// Explicit preparation-ledger transfer for receipt-only budget boundaries.
fn prepared(
    module: &Module,
) -> (
    Owner,
    usize,
    CheckedOwner,
    InertCanonicalPolicy4ExecutionReceiptV1,
) {
    let (input, input_size) = admit(module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(input_size).unwrap();
    let checked = optimize_checked_canonical_kernel_ir_policy4_v1(&input, &mut budget).unwrap();
    budget.reserve_storage(checked.retained_storage()).unwrap();
    let wire = encode_checked_canonical_policy4_execution_receipt_v1(&input, &checked, &mut budget)
        .unwrap();
    assert_eq!(budget.storage(), input_size + checked.retained_storage());
    (input, input_size, checked, wire)
}

#[test]
fn policy4_receipt_replays_both_stages_and_authenticates_only_against_real_execution() {
    let (input, size, checked, wire) = prepared(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = PREFIX + size + checked.retained_storage() + wire.storage().retained_storage();
    budget.reserve_storage(floor).unwrap();
    let replay = decode_and_check_published_policy4_semantic_relation_v1(
        &input,
        checked.intermediate_policy3().owner(),
        checked.owner(),
        wire.canonical_bytes(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(replay.storage().retained_storage())
        .unwrap();
    assert!(std::ptr::eq(replay.input(), &input));
    assert!(std::ptr::eq(
        replay.intermediate(),
        checked.intermediate_policy3().owner()
    ));
    assert!(std::ptr::eq(replay.output(), checked.owner()));
    assert_eq!(replay.forwarding_rows(), checked.forwarding_rows());
    assert!(!replay.authenticates_execution());
    assert!(!replay.grants_authority());
    let receipt_size = replay.storage().retained_storage();
    drop(replay);
    budget.release_storage(receipt_size).unwrap();
    let authenticated = decode_and_check_canonical_policy4_execution_receipt_v1(
        &input,
        &checked,
        wire.canonical_bytes(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(authenticated.storage().retained_storage())
        .unwrap();
    assert!(std::ptr::eq(authenticated.execution_owner(), &checked));
    assert!(authenticated.authenticates_execution());
    assert!(!authenticated.grants_authority());
    let receipt_size = authenticated.storage().retained_storage();
    drop(authenticated);
    budget.release_storage(receipt_size).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn policy4_published_dynamic_profile_claim_cannot_authenticate_even_unchanged_graph() {
    let (input, size, checked, wire) = prepared(&Module::new("no-op"));
    let mut bytes = wire.canonical_bytes().to_vec();
    // Existing Policy3 dynamic work remains a syntactically admissible claim,
    // but cannot equal the retained actual Policy3 execution witness.
    bytes[CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1 + 32 + 96] ^= 1;
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = PREFIX + size + checked.retained_storage() + bytes.capacity();
    budget.reserve_storage(floor).unwrap();
    let replay = decode_and_check_published_policy4_semantic_relation_v1(
        &input,
        checked.intermediate_policy3().owner(),
        checked.owner(),
        &bytes,
        &mut budget,
    )
    .unwrap();
    assert!(!replay.authenticates_execution());
    drop(replay);
    assert!(matches!(
        decode_and_check_canonical_policy4_execution_receipt_v1(
            &input,
            &checked,
            &bytes,
            &mut budget
        ),
        Err(Error::Policy3(
            CanonicalPolicy3ExecutionReceiptErrorV1::ExecutionWitness
        ))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn policy4_receipt_rejects_fixed_profile_and_framing_substitution() {
    let (input, size, checked, wire) = prepared(&fixture());
    for offset in [0, 8, 10, 12, 16, 24, 32, 40, 48, 50, 52, 54, 56] {
        let mut bytes = wire.canonical_bytes().to_vec();
        bytes[offset] ^= 1;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = PREFIX + size + checked.retained_storage() + bytes.capacity();
        budget.reserve_storage(floor).unwrap();
        assert!(
            decode_and_check_published_policy4_semantic_relation_v1(
                &input,
                checked.intermediate_policy3().owner(),
                checked.owner(),
                &bytes,
                &mut budget
            )
            .is_err(),
            "offset {offset}"
        );
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn policy4_receipt_rejects_malicious_c_o_and_semantically_wrong_o_with_reframed_identity() {
    let (input, size, checked, wire) = prepared(&fixture());
    let (foreign, foreign_size) = admit(&Module::new("foreign-intermediate"));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = PREFIX
        + size
        + checked.retained_storage()
        + wire.storage().retained_storage()
        + foreign_size;
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        decode_and_check_published_policy4_semantic_relation_v1(
            &input,
            &foreign,
            checked.owner(),
            wire.canonical_bytes(),
            &mut budget
        ),
        Err(Error::ExecutionClaim)
    ));
    assert!(matches!(
        decode_and_check_published_policy4_semantic_relation_v1(
            &input,
            checked.intermediate_policy3().owner(),
            &foreign,
            wire.canonical_bytes(),
            &mut budget
        ),
        Err(Error::ExecutionClaim)
    ));
    let mut module = checked.owner().module().clone();
    let coordinate = checked.forwarding_rows()[0].load;
    let operation = &mut module.functions[coordinate.block.function.0 as usize]
        .body
        .as_mut()
        .unwrap()
        .blocks[coordinate.block.block as usize]
        .operations[coordinate.operation as usize];
    let OperationKind::Binary { lhs, rhs, .. } = operation.kind else {
        panic!("actual forwarded operation")
    };
    operation.kind = OperationKind::Binary {
        op: BinaryOp::BitXor,
        lhs,
        rhs,
    };
    let (wrong, wrong_size) = admit(&module);
    budget.reserve_storage(wrong_size).unwrap();
    let mut bytes = wire.canonical_bytes().to_vec();
    budget.reserve_storage(bytes.capacity()).unwrap();
    bytes[40..CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1].copy_from_slice(
        &policy4_record_v1(
            &input,
            checked.intermediate_policy3().owner(),
            &wrong,
            checked.forwarding_rows().len(),
        )
        .unwrap(),
    );
    assert!(matches!(
        decode_and_check_published_policy4_semantic_relation_v1(
            &input,
            checked.intermediate_policy3().owner(),
            &wrong,
            &bytes,
            &mut budget
        ),
        Err(Error::Forwarding(_))
    ));
    // Correct final identities cannot hide a foreign B/C semantic body either.
    bytes[40..CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1].copy_from_slice(
        &policy4_record_v1(
            &input,
            &foreign,
            checked.owner(),
            checked.forwarding_rows().len(),
        )
        .unwrap(),
    );
    assert!(matches!(
        decode_and_check_published_policy4_semantic_relation_v1(
            &input,
            &foreign,
            checked.owner(),
            &bytes,
            &mut budget
        ),
        Err(Error::Policy3(_))
    ));
}

#[test]
fn policy4_receipt_rejects_wrong_duplicate_and_missing_forwarding_rows() {
    let (input, size, checked, wire) = prepared(&fixture());
    let base = wire.canonical_bytes();
    let row_start = CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1
        + u64::from_le_bytes(base[24..32].try_into().unwrap()) as usize;
    assert_eq!(base.len() - row_start, ROW_BYTES);
    for mode in 0..3 {
        let mut bytes = base.to_vec();
        let count = match mode {
            0 => {
                bytes[row_start + 8] ^= 1;
                1
            }
            1 => {
                bytes.extend_from_slice(&base[row_start..]);
                2
            }
            _ => {
                bytes.truncate(row_start);
                0
            }
        };
        let total = bytes.len();
        bytes[16..24].copy_from_slice(&(total as u64).to_le_bytes());
        bytes[32..40].copy_from_slice(&(count as u64).to_le_bytes());
        bytes[40..CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1].copy_from_slice(
            &policy4_record_v1(
                &input,
                checked.intermediate_policy3().owner(),
                checked.owner(),
                count,
            )
            .unwrap(),
        );
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = PREFIX + size + checked.retained_storage() + bytes.capacity();
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            decode_and_check_published_policy4_semantic_relation_v1(
                &input,
                checked.intermediate_policy3().owner(),
                checked.owner(),
                &bytes,
                &mut budget
            ),
            Err(Error::Forwarding(_))
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn policy4_receipt_header_precharge_344_343_and_limit_failures_restore_floor() {
    let (input, size, checked, _) = prepared(&Module::new("empty"));
    for limit in [343, 344] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = PREFIX + size + checked.retained_storage();
        budget.reserve_storage(floor).unwrap();
        let error = decode_and_check_published_policy4_semantic_relation_v1(
            &input,
            checked.intermediate_policy3().owner(),
            checked.owner(),
            &[],
            &mut budget,
        )
        .err()
        .unwrap();
        if limit == 343 {
            assert!(matches!(error, Error::Resource(Resource::Work(_))));
            assert_eq!(budget.work(), 0);
        } else {
            assert!(matches!(error, Error::Header));
            assert_eq!(budget.work(), 344);
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.failed_storage(), None);
        drop(budget);
        assert_eq!(
            work.failed_work(),
            if limit == 343 { Some(344) } else { None }
        );
    }
}

#[test]
fn policy4_semantic_decode_exact_work_storage_boundaries_are_composed_from_child_fees() {
    let (input, size, checked, wire) = prepared(&fixture());
    let bytes = wire.canonical_bytes();
    let p3_length = u64::from_le_bytes(bytes[24..32].try_into().unwrap()) as usize;
    let p3_bytes = &bytes[CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1
        ..CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1 + p3_length];
    let floor = PREFIX + size + checked.retained_storage() + wire.storage().retained_storage();
    let mut p3_work = Work::new(WORK);
    let mut p3_budget = Budget::new(&mut p3_work, STORAGE);
    p3_budget.reserve_storage(floor).unwrap();
    let p3 = decode_and_check_published_policy3_semantic_relation_v1(
        &input,
        checked.intermediate_policy3().owner(),
        p3_bytes,
        &mut p3_budget,
    )
    .unwrap();
    let p3_size = p3.storage().retained_storage();
    let p3_peak = p3_budget.peak_storage() - floor;
    let mut check_work = Work::new(WORK);
    let mut check_budget = Budget::new(&mut check_work, STORAGE);
    check_budget.reserve_storage(floor).unwrap();
    let (relation, check_size) = check_canonical_kir_store_forwarding_v1(
        checked.intermediate_policy3().owner(),
        checked.owner(),
        checked.forwarding_rows(),
        &mut check_budget,
    )
    .unwrap();
    let check_peak = (check_budget.peak_storage() - floor).max(check_size.retained_storage());
    let wrapper = size_of::<ReplayedPolicy4SemanticRelationV1<'_, '_, '_, '_>>()
        - size_of::<ReplayedPolicy3SemanticRelationV1<'_, '_, '_>>();
    let mut allocator_probe = Vec::<Row>::new();
    allocator_probe
        .try_reserve_exact(checked.forwarding_rows().len())
        .unwrap();
    let row_capacity = allocator_probe.capacity() * size_of::<Row>();
    drop(allocator_probe);
    let expected_work = 344
        + p3_budget.work()
        + 3
        + ROW_BYTES * checked.forwarding_rows().len()
        + 2
        + check_budget.work();
    let expected_peak = floor + p3_peak.max(p3_size + wrapper + row_capacity + check_peak);
    drop(relation);
    drop(p3);
    for (work_limit, storage_limit, succeeds) in [
        (expected_work, expected_peak, true),
        (expected_work - 1, expected_peak, false),
        (expected_work, expected_peak - 1, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = decode_and_check_published_policy4_semantic_relation_v1(
            &input,
            checked.intermediate_policy3().owner(),
            checked.owner(),
            bytes,
            &mut budget,
        );
        if succeeds {
            assert!(result.is_ok());
            assert_eq!(budget.work(), expected_work);
            assert_eq!(budget.peak_storage(), expected_peak);
        } else {
            assert!(result.is_err());
        }
        assert_eq!(budget.storage(), floor);
        if storage_limit < expected_peak {
            assert!(budget.failed_storage().is_some());
        }
        drop(budget);
        if work_limit < expected_work {
            assert!(work.failed_work().is_some());
        }
    }
}

#[test]
fn policy4_wire_encoding_is_deterministic_and_counts_actual_owned_capacity() {
    let (input, size, checked, first) = prepared(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = PREFIX + size + checked.retained_storage() + first.storage().retained_storage();
    budget.reserve_storage(floor).unwrap();
    let second =
        encode_checked_canonical_policy4_execution_receipt_v1(&input, &checked, &mut budget)
            .unwrap();
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(
        second.storage().retained_storage(),
        size_of::<InertCanonicalPolicy4ExecutionReceiptV1>() + second.bytes.capacity()
    );
    assert_eq!(budget.storage(), floor);
}
