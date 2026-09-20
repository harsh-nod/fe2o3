//! Actual-stage controls, invoked by both ownership modes and both profiles.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1, CanonicalKirFunctionCoordinateV1,
    CanonicalKirOperationCoordinateV1, InertCanonicalKirTransitionReceiptV1 as Wire,
};
use fe2o3_kernel_opt::encode_checked_canonical_policy4_execution_receipt_v1;

pub(super) fn check<'a>(
    stage: &'a PreparedPolicy8ArtifactsV1,
    claims: PortablePolicy7ClaimsV1<'a>,
    budget: &mut Budget<'_>,
) -> Result8<AuthenticatedPolicy8ExecutionRelationV1<'a>> {
    stage.check_portable_execution_relation_v1(
        claims.policy4_wire,
        claims.policy5_record,
        claims.load_rows,
        claims.policy6,
        claims.policy7_record,
        budget,
    )
}

pub(super) fn validate(
    stage: &PreparedPolicy8ArtifactsV1,
    claims: PortablePolicy7ClaimsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result8<usize> {
    let floor = budget.storage();
    let receipt = check(stage, claims, budget)?;
    assert_eq!(budget.storage(), floor);
    let (_, checked) = stage.admitted.history().portable_prefix();
    let tail = stage.admitted.portable_tail();
    assert!(std::ptr::eq(receipt.actual_stage(), stage));
    assert!(std::ptr::eq(
        receipt.policy6_execution().execution_owner(),
        checked
    ));
    assert!(std::ptr::eq(
        receipt.policy7_continuation().relation().input(),
        checked.owner()
    ));
    assert!(std::ptr::eq(
        receipt.policy7_continuation().relation().output(),
        stage.admitted.historical_j()
    ));
    assert!(std::ptr::eq(
        receipt.continuation().input(),
        receipt.policy7_continuation().relation().output()
    ));
    assert!(std::ptr::eq(
        receipt.continuation().output(),
        stage.output()
    ));
    assert_eq!(
        receipt
            .policy7_continuation()
            .unauthenticated_execution_record(),
        stage.prefix_execution().canonical_bytes()
    );
    assert_eq!(receipt.continuation().proved_pairs(), tail.proved_pairs());
    assert_eq!(
        receipt.continuation().has_substitutions(),
        tail.execution().changed()
    );
    assert!(receipt.authenticates_execution());
    assert!(!receipt.grants_artifact_or_launch_authority());
    assert!(!receipt.continuation().authenticates_execution());
    assert!(!receipt.continuation().grants_authority());
    let actual = tail.occurrences().candidate();
    let borrowed = receipt.continuation().claims().occurrences;
    macro_rules! same_rows {
        ($($axis:ident),+ $(,)?) => {$({
            assert_eq!(borrowed.$axis, actual.$axis);
            assert_eq!(borrowed.$axis.as_ptr(), actual.$axis.as_ptr());
        })+};
    }
    same_rows!(
        functions,
        blocks,
        segments,
        operations,
        definitions,
        definition_outputs,
        uses,
        edges,
        edge_arguments,
    );
    let retained = receipt.retained_storage();
    assert_eq!(
        retained,
        wrapper_storage().unwrap()
            + receipt.policy6_execution().storage().retained_storage()
            + receipt.policy7_continuation().storage().retained_storage()
            + receipt.continuation().storage().retained_storage()
    );
    budget.reserve_storage(retained).map_err(resource)?;
    drop(receipt);
    budget.release_storage(retained).map_err(resource)?;
    assert_eq!(budget.storage(), floor);
    Ok(retained)
}

fn refused(
    stage: &PreparedPolicy8ArtifactsV1,
    claims: PortablePolicy7ClaimsV1<'_>,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    assert!(check(stage, claims, budget).is_err());
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

fn claim_controls(
    stage: &PreparedPolicy8ArtifactsV1,
    claims: PortablePolicy7ClaimsV1<'_>,
    budget: &mut Budget<'_>,
) {
    let bad = [0_u8];
    for altered in [
        PortablePolicy7ClaimsV1 {
            policy4_wire: &bad,
            ..claims
        },
        PortablePolicy7ClaimsV1 {
            policy5_record: &bad,
            ..claims
        },
        PortablePolicy7ClaimsV1 {
            policy6: CanonicalPolicy6ContinuationClaimsV1 {
                composition_record: &bad,
                ..claims.policy6
            },
            ..claims
        },
        PortablePolicy7ClaimsV1 {
            policy6: CanonicalPolicy6ContinuationClaimsV1 {
                integer_record: &bad,
                ..claims.policy6
            },
            ..claims
        },
        PortablePolicy7ClaimsV1 {
            policy6: CanonicalPolicy6ContinuationClaimsV1 {
                transition_wire: &bad,
                ..claims.policy6
            },
            ..claims
        },
        PortablePolicy7ClaimsV1 {
            policy7_record: &bad,
            ..claims
        },
        PortablePolicy7ClaimsV1 {
            policy7_record: stage.original().canonical().canonical_bytes(),
            ..claims
        },
        PortablePolicy7ClaimsV1 {
            policy7_record: stage.admitted.historical_j().canonical().canonical_bytes(),
            ..claims
        },
        PortablePolicy7ClaimsV1 {
            policy7_record: stage.output().canonical().canonical_bytes(),
            ..claims
        },
    ] {
        refused(stage, altered, budget);
    }
    // External fixed test backing is explicitly prepaid. Even an empty actual
    // P5 roster cannot make an appended caller row disappear from validation.
    let coordinate = CanonicalKirOperationCoordinateV1 {
        block: CanonicalKirBlockCoordinateV1 {
            function: CanonicalKirFunctionCoordinateV1(u32::MAX),
            block: u32::MAX,
        },
        operation: u32::MAX,
    };
    let storage = size_of::<CanonicalKirLoadForwardingRowV1>();
    budget.reserve_storage(storage).unwrap();
    {
        let rows = [CanonicalKirLoadForwardingRowV1 {
            first: coordinate,
            load: coordinate,
        }];
        refused(
            stage,
            PortablePolicy7ClaimsV1 {
                load_rows: &rows,
                ..claims
            },
            budget,
        );
    }
    budget.release_storage(storage).unwrap();
    let requested = size_of::<Vec<u8>>() + claims.policy7_record.len();
    budget.reserve_storage(requested).unwrap();
    let mut record = Vec::new();
    record
        .try_reserve_exact(claims.policy7_record.len())
        .unwrap();
    let excess = record.capacity() - claims.policy7_record.len();
    budget.reserve_storage(excess).unwrap();
    record.extend_from_slice(claims.policy7_record);
    // Preserve record extent while independently changing the policy, each
    // endpoint/map field, counts, and a final complete occurrence row byte.
    for offset in [8, 272, 312, 352, 360, 368, 376, record.len() - 1] {
        record.copy_from_slice(claims.policy7_record);
        record[offset] ^= 1;
        refused(
            stage,
            PortablePolicy7ClaimsV1 {
                policy7_record: &record,
                ..claims
            },
            budget,
        );
    }
    drop(record);
    budget.release_storage(requested + excess).unwrap();
}

fn tail_controls(stage: &PreparedPolicy8ArtifactsV1, budget: &mut Budget<'_>) {
    let tail = stage.admitted.portable_tail();
    let input = stage.admitted.historical_j();
    let output = stage.output();
    let claims = CanonicalPolicy8ContinuationClaimsV1 {
        pass_name: POLICY8_COMMUTATIVE_PASS_NAME_V1,
        input: InertCanonicalKirTransitionGraphIdentityV1::from_verified(
            input.canonical().identity(),
        ),
        output: InertCanonicalKirTransitionGraphIdentityV1::from_verified(
            output.canonical().identity(),
        ),
        occurrences: tail.occurrences().candidate(),
    };
    let floor = budget.storage();
    assert!(matches!(
        check_canonical_policy8_continuation_relation_v1(
            input,
            output,
            CanonicalPolicy8ContinuationClaimsV1 {
                pass_name: "policy7",
                ..claims
            },
            budget,
        ),
        Err(CanonicalPolicy8SemanticErrorV1::PassIdentity)
    ));
    if tail.execution().changed() {
        assert!(matches!(
            check_canonical_policy8_continuation_relation_v1(
                input,
                output,
                CanonicalPolicy8ContinuationClaimsV1 {
                    input: claims.output,
                    ..claims
                },
                budget,
            ),
            Err(CanonicalPolicy8SemanticErrorV1::InputIdentity)
        ));
        assert!(matches!(
            check_canonical_policy8_continuation_relation_v1(
                input,
                output,
                CanonicalPolicy8ContinuationClaimsV1 {
                    output: claims.input,
                    ..claims
                },
                budget,
            ),
            Err(CanonicalPolicy8SemanticErrorV1::OutputIdentity)
        ));
    }
    // The component's complete nine-axis hostile matrix is a prerequisite.
    // Here each nonempty actual-stage axis is independently truncated; empty
    // axes get pointer/equality checks above, not a vacuous refusal claim.
    let mut exercised = 0;
    macro_rules! omitted_rows {
        ($($axis:ident),+ $(,)?) => {$({
            let rows = claims.occurrences.$axis;
            if !rows.is_empty() {
                let mut altered = claims;
                altered.occurrences.$axis = &rows[..rows.len() - 1];
                assert!(check_canonical_policy8_continuation_relation_v1(
                    input, output, altered, budget,
                ).is_err());
                assert_eq!(budget.storage(), floor);
                exercised += 1;
            }
        })+};
    }
    omitted_rows!(
        functions,
        blocks,
        segments,
        operations,
        definitions,
        definition_outputs,
        uses,
        edges,
        edge_arguments,
    );
    assert!(exercised >= 7);
    assert_eq!(budget.storage(), floor);
    // A truthful equal-byte foreign pair remains a semantic-only subject.
    // It cannot replace the private actual-stage borrow authenticated above.
    let (foreign_j, j_storage) =
        Graph::from_module_ref_with_verification_budget_v12(input.module(), budget).unwrap();
    budget
        .reserve_storage(j_storage.retained_storage())
        .unwrap();
    let (foreign_k, k_storage) =
        Graph::from_module_ref_with_verification_budget_v12(output.module(), budget).unwrap();
    budget
        .reserve_storage(k_storage.retained_storage())
        .unwrap();
    assert!(!std::ptr::eq(&foreign_j, input));
    assert!(!std::ptr::eq(&foreign_k, output));
    assert_eq!(
        foreign_j.canonical().canonical_bytes(),
        input.canonical().canonical_bytes()
    );
    assert_eq!(
        foreign_k.canonical().canonical_bytes(),
        output.canonical().canonical_bytes()
    );
    let retained = {
        let semantic = check_canonical_policy8_continuation_relation_v1(
            &foreign_j, &foreign_k, claims, budget,
        )
        .unwrap();
        let retained = semantic.storage().retained_storage();
        budget.reserve_storage(retained).unwrap();
        assert!(!semantic.authenticates_execution());
        assert!(!semantic.grants_authority());
        retained
    };
    budget.release_storage(retained).unwrap();
    drop(foreign_k);
    budget
        .release_storage(k_storage.retained_storage())
        .unwrap();
    drop(foreign_j);
    budget
        .release_storage(j_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

/// Existing constructed genuine Direct/UnitLocal mutation/no-op fixtures call
/// this for gfx942 and gfx950. No ordinary-source or signed success is implied.
pub(crate) fn exercise(stage: &PreparedPolicy8ArtifactsV1, budget: &mut Budget<'_>) {
    let entry = budget.storage();
    let (bound, checked) = stage.admitted.history().portable_prefix();
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
    let claims = PortablePolicy7ClaimsV1 {
        policy4_wire: p4.canonical_bytes(),
        policy5_record: p5.execution().canonical_bytes(),
        load_rows: p5.load_forwarding_rows(),
        policy6: CanonicalPolicy6ContinuationClaimsV1 {
            composition_record: checked.execution().canonical_bytes(),
            integer_record: checked.continuation().execution().canonical_bytes(),
            transition_wire: transition.canonical_bytes(),
        },
        policy7_record: stage.prefix_execution().canonical_bytes(),
    };
    validate(stage, claims, budget).unwrap();
    claim_controls(stage, claims, budget);
    tail_controls(stage, budget);
    super::resource_tests::exercise(stage, claims, budget.storage());
    drop(transition);
    budget.release_storage(transition_storage).unwrap();
    drop(p4);
    budget.release_storage(p4_storage).unwrap();
    assert_eq!(budget.storage(), entry);
    crate::production_pipeline::checked_output_policy8_v1::history::tests::exercise(stage, budget);
}

#[test]
fn added_wrapper_excludes_each_embedded_portable_header_once() {
    assert_eq!(
        wrapper_storage().unwrap()
            + size_of::<CheckedCanonicalPolicy6ExecutionRelationV1<'_>>()
            + size_of::<CheckedCanonicalPolicy7ContinuationRelationV1<'_>>()
            + size_of::<CheckedCanonicalPolicy8ContinuationRelationV1<'_, '_, '_>>(),
        size_of::<AuthenticatedPolicy8ExecutionRelationV1<'_>>()
    );
}
