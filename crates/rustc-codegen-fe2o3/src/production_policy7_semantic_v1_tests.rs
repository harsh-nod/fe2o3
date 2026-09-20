use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, InertCanonicalKirTransitionReceiptV1 as Wire,
};
use fe2o3_kernel_opt::encode_checked_canonical_policy4_execution_receipt_v1;

#[path = "production_policy7_semantic_parity_v1_tests.rs"]
mod parity;

fn validate<'a>(
    stage: &'a PreparedPolicy7ArtifactsV1,
    policy4_wire: &'a [u8],
    claims: CanonicalPolicy6ContinuationClaimsV1<'a>,
    record: &'a [u8],
    budget: &mut Budget<'_>,
) -> Result7<usize> {
    let (_, checked) = stage.admitted.portable_prefix();
    let p5 = checked.intermediate_policy5();
    let receipt = stage.check_portable_execution_relation_v1(
        policy4_wire,
        p5.execution().canonical_bytes(),
        p5.load_forwarding_rows(),
        claims,
        record,
        budget,
    )?;
    assert!(std::ptr::eq(receipt.actual_stage(), stage));
    assert!(std::ptr::eq(
        receipt.policy6_execution().execution_owner(),
        checked
    ));
    assert!(std::ptr::eq(
        receipt.continuation().relation().input(),
        stage.admitted.input()
    ));
    assert!(std::ptr::eq(
        receipt.continuation().relation().output(),
        stage.output()
    ));
    assert_eq!(
        receipt.continuation().unauthenticated_execution_record(),
        stage.execution().canonical_bytes()
    );
    assert!(receipt.authenticates_execution());
    assert!(!receipt.grants_artifact_or_launch_authority());
    let retained = receipt.retained_storage();
    assert_eq!(
        retained,
        receipt.policy6_execution().storage().retained_storage()
            + receipt.continuation().storage().retained_storage()
            + size_of::<AuthenticatedPolicy7ExecutionRelationV1<'_>>()
            - size_of::<CheckedCanonicalPolicy6ExecutionRelationV1<'_>>()
            - size_of::<CheckedCanonicalPolicy7ContinuationRelationV1<'_>>()
    );
    budget.reserve_storage(retained).map_err(resource)?;
    drop(receipt);
    budget.release_storage(retained).map_err(resource)?;
    Ok(retained)
}

/// Invoked by the existing genuine Direct/UnitLocal, mutation/no-op, both-profile
/// artifact fixtures. These are unsigned source components, not signed Native7.
pub(crate) fn exercise(stage: &PreparedPolicy7ArtifactsV1, budget: &mut Budget<'_>) {
    let entry = budget.storage();
    let (bound, checked) = stage.admitted.portable_prefix();
    let p5 = checked.intermediate_policy5();
    let p4 = encode_checked_canonical_policy4_execution_receipt_v1(
        bound,
        p5.intermediate_policy4(),
        budget,
    )
    .unwrap();
    let p4_storage = p4.storage().retained_storage();
    budget.reserve_storage(p4_storage).unwrap();
    let (transition, receipt) = Wire::from_candidate_with_budget(
        p5.owner().canonical().identity(),
        checked.owner().canonical().identity(),
        checked.continuation().occurrences().candidate(),
        budget,
    )
    .unwrap();
    let transition_storage = receipt.retained_storage();
    budget.reserve_storage(transition_storage).unwrap();
    let claims = CanonicalPolicy6ContinuationClaimsV1 {
        composition_record: checked.execution().canonical_bytes(),
        integer_record: checked.continuation().execution().canonical_bytes(),
        transition_wire: transition.canonical_bytes(),
    };
    parity::exercise(stage, p4.canonical_bytes(), claims, budget);
    let floor = budget.storage();
    // Decode genuine producer bytes, then borrow the owned rows into the real
    // independent I/J checker. This is not a raw-to-sealed witness constructor.
    let decoded = fe2o3_kernel_opt::decode_canonical_policy7_rows_v1(
        stage.execution().canonical_bytes(),
        budget,
    )
    .unwrap();
    let decoded_storage = decoded.storage().retained_storage();
    budget.reserve_storage(decoded_storage).unwrap();
    {
        let rows = decoded.claims();
        let actual = stage.admitted.continuation();
        assert_eq!(rows.execution_record, stage.execution().canonical_bytes());
        assert_eq!(rows.deletion_rows, actual.rows());
        assert_eq!(rows.retained_operations, actual.retained_operations());
        assert!(!decoded.grants_authority());
        let retained = {
            let relation = check_canonical_policy7_continuation_relation_v1(
                checked.execution().canonical_bytes(),
                stage.admitted.input(),
                stage.output(),
                rows,
                budget,
            )
            .unwrap();
            assert_eq!(
                relation.relation().rows().as_ptr(),
                rows.deletion_rows.as_ptr()
            );
            let retained = relation.storage().retained_storage();
            budget.reserve_storage(retained).unwrap();
            retained
        };
        budget.release_storage(retained).unwrap();
    }
    drop(decoded);
    budget.release_storage(decoded_storage).unwrap();
    assert_eq!(budget.storage(), floor);
    validate(
        stage,
        p4.canonical_bytes(),
        claims,
        stage.execution().canonical_bytes(),
        budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    // The complete work measurement includes inherited source replay, not just
    // portable P6 decoding and the new transcript scan.
    let measure = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = validate(
            stage,
            p4.canonical_bytes(),
            claims,
            stage.execution().canonical_bytes(),
            &mut budget,
        );
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (result, budget.work(), budget.peak_storage())
    };
    let measured = measure(1_000_000_000, 512 << 20);
    measured.0.unwrap();
    let exact = measure(measured.1, measured.2);
    exact.0.unwrap();
    assert_eq!((measured.1, measured.2), (exact.1, exact.2));
    assert!(measure(measured.1 - 1, measured.2).0.is_err());
    assert!(measure(measured.1, measured.2 - 1).0.is_err());
    let scratch = 256 + 416 + stage.execution().canonical_bytes().len();
    budget.reserve_storage(scratch).unwrap();
    {
        for map in [false, true] {
            let mut composition = *checked.execution().canonical_bytes();
            let mut integer = *checked.continuation().execution().canonical_bytes();
            let mut record = stage.execution().canonical_bytes().to_vec();
            if map {
                composition[216] ^= 1;
                integer[264] ^= 1;
            } else {
                integer[88] ^= 1;
            }
            record[16..272].copy_from_slice(&composition);
            assert!(
                validate(
                    stage,
                    p4.canonical_bytes(),
                    CanonicalPolicy6ContinuationClaimsV1 {
                        composition_record: &composition,
                        integer_record: &integer,
                        ..claims
                    },
                    &record,
                    budget
                )
                .is_err()
            );
            assert_eq!(budget.storage(), floor + scratch);
        }
        for offset in [
            8,
            272,
            312,
            352,
            360,
            368,
            376,
            stage.execution().canonical_bytes().len() - 1,
        ] {
            let mut record = stage.execution().canonical_bytes().to_vec();
            record[offset] ^= 1;
            assert!(validate(stage, p4.canonical_bytes(), claims, &record, budget).is_err());
            assert_eq!(budget.storage(), floor + scratch);
        }
    }
    budget.release_storage(scratch).unwrap();
    drop(transition);
    budget.release_storage(transition_storage).unwrap();
    drop(p4);
    budget.release_storage(p4_storage).unwrap();
    assert_eq!(budget.storage(), entry);
}
