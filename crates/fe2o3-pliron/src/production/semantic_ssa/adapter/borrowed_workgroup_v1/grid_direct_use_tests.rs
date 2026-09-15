//! Classifier inventory tests, not a synthetic Grid issuance certificate.
use super::super::super::grid_read_borrows_v1::inventory_probe::Probe;
use super::*;

const GRID_OWNER: u32 = 18;
const GRID_REF: u32 = 19;
const GRID_OWNED_TYPE: u32 = 6;
const GRID_REFERENCE_TYPE: u32 = 7;
const RANK_TYPE: u32 = 8;

fn mixed(
    context: bool,
) -> (
    Fixture,
    SemanticTransparentBorrowSiteV1,
    [SemanticTransparentBorrowSiteV1; 3],
) {
    let mut fixture = Fixture::new();
    fixture.types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([26; 32]),
        SemanticLayoutIdentityV1::from_sha256([26; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![ty(RANK_TYPE); 2]).unwrap(),
        ),
    ));
    fixture.types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([27; 32]),
        SemanticLayoutIdentityV1::from_sha256([27; 32]),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(GRID_OWNED_TYPE),
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    fixture.types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([28; 32]),
        SemanticLayoutIdentityV1::from_sha256([28; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX as u128),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ));
    for (local_id, ty_id) in [
        (18, GRID_OWNED_TYPE),
        (19, GRID_REFERENCE_TYPE),
        (20, RANK_TYPE),
        (21, RANK_TYPE),
        (22, RANK_TYPE),
    ] {
        fixture
            .locals
            .push(local(local_id, ty_id, SemanticLocalRoleV1::Temporary));
    }
    let rank = |value| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(RANK_TYPE),
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 8).unwrap()),
        ))
    };
    let read = |to, field| {
        assign(
            to,
            RANK_TYPE,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(GRID_REF),
                    vec![
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Dereference,
                            ty(GRID_OWNED_TYPE),
                        )
                        .unwrap(),
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Field(field),
                            ty(RANK_TYPE),
                        )
                        .unwrap(),
                    ],
                    ty(RANK_TYPE),
                )
                .unwrap(),
            )),
        )
    };
    let statements = vec![
        assign(
            GRID_OWNER,
            GRID_OWNED_TYPE,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Aggregate,
                    vec![rank(0), rank(64)],
                )
                .unwrap(),
            ),
        ),
        borrow(
            GRID_REF,
            GRID_REFERENCE_TYPE,
            place(GRID_OWNER, GRID_OWNED_TYPE),
            SemanticBorrowKindV1::Shared,
        ),
        read(20, 0),
        read(21, 1),
        read(22, 0),
    ];
    let b = if context { 6 } else { 0 };
    if context {
        fixture.blocks[6] = block(6, statements, jump(7));
    } else {
        fixture.blocks = vec![block(0, statements, SemanticTerminatorKindV1::Return)];
        fixture.callables.clear();
    }
    (fixture, site(b, 1), [site(b, 2), site(b, 3), site(b, 4)])
}

fn run(
    context: bool,
    limit: usize,
) -> (
    Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1>,
    Probe,
    SemanticFunctionDeclV1,
    SemanticTransparentBorrowSiteV1,
    [SemanticTransparentBorrowSiteV1; 3],
) {
    let (fixture, borrow, reads) = mixed(context);
    let body = fixture.body();
    let before = body.clone();
    let mut probe = Probe::new(
        &body,
        ty(GRID_REFERENCE_TYPE),
        SemanticLocalIdV1::from_index(GRID_OWNER),
        SemanticLocalIdV1::from_index(GRID_REF),
        borrow,
        reads,
    );
    let result = sites_with_phase(
        &body,
        &fixture.callables,
        &[],
        limit,
        Some(&fixture.types),
        &MathBorrowSitesV1::default(),
        &MatrixBorrowSitesV1::default(),
        None,
        None,
        None,
        Some(&mut probe),
    );
    assert_eq!(
        body, before,
        "inventory observation must not change original statements or CFG"
    );
    (result, probe, body, borrow, reads)
}

fn assert_exact_inventory(probe: &Probe, reads: [SemanticTransparentBorrowSiteV1; 3]) {
    assert_eq!(probe.before.len(), 3);
    assert_eq!(probe.after.len(), 3);
    let index = probe.before[0].candidate;
    for (i, (before, after)) in probe.before.iter().zip(&probe.after).enumerate() {
        assert_eq!(before.site, reads[i]);
        assert_eq!(after.site, reads[i]);
        assert_eq!(before.candidate, index);
        assert_eq!(after.candidate, index);
        assert_eq!(before.uses.as_deref(), Some(&reads[..i]));
        assert_eq!(
            after.uses.as_deref(),
            Some(&reads[..=i]),
            "live Grid branch omitted or duplicated a direct use"
        );
        assert_eq!(before.remaining - after.remaining, 1);
        assert_eq!(after.consumers, before.consumers + 1);
        assert!(after.intrinsic);
    }
    assert!(probe.finished);
    assert_eq!(probe.final_uses[index], reads);
    for (other, uses) in probe.final_uses.iter().enumerate() {
        if other != index {
            assert!(uses.iter().all(|site| !reads.contains(site)));
        }
    }
}

#[test]
fn grid_direct_uses_mixed_context_records_exact_live_inventory() {
    let (result, probe, _, borrow, reads) = run(true, MAX_FLOW_WORK);
    let accepted = result.unwrap();
    assert!(accepted.contains(&borrow));
    assert!(
        accepted.contains(&site(7, 65)),
        "existing mutable Context root must still be accepted"
    );
    assert_exact_inventory(&probe, reads);
    assert!(
        probe
            .final_uses
            .iter()
            .enumerate()
            .any(|(index, uses)| index != probe.before[0].candidate && !uses.is_empty()),
        "the inventory must include real Context terminal uses, not a manually allocated empty tracker"
    );
}

#[test]
fn grid_direct_uses_absent_tracker_preserves_consumers_without_push_debit() {
    let (result, probe, _, borrow, reads) = run(false, MAX_FLOW_WORK);
    assert_eq!(result.unwrap(), BTreeSet::from([borrow]));
    assert!(probe.finished && probe.final_uses.is_empty());
    assert_eq!(probe.after.len(), reads.len());
    for (before, after) in probe.before.iter().zip(&probe.after) {
        assert_eq!(before.uses, None);
        assert_eq!(after.uses, None);
        assert_eq!(before.remaining, after.remaining);
        assert_eq!(after.consumers, before.consumers + 1);
        assert!(after.intrinsic);
    }
}

#[test]
fn grid_direct_uses_exact_boundary_fails_before_append_without_refund() {
    let (_, calibration, _, _, _) = run(true, MAX_FLOW_WORK);
    let before = &calibration.before[0];
    let spent = MAX_FLOW_WORK - before.remaining;
    let (result, probe, _, _, _) = run(true, spent);
    assert_exact_uses_exhaustion(result.unwrap_err(), spent);
    assert_eq!(probe.before.len(), 1);
    assert!(probe.after.is_empty());
    assert!(!probe.finished);
    assert_eq!(probe.before[0].remaining, 0);
    assert_eq!(probe.before[0].uses, Some(vec![]));
    assert_eq!(probe.before[0].consumers, 0);
    let (result, probe, _, _, _) = run(true, spent + 1);
    assert_exact_uses_exhaustion(result.unwrap_err(), spent + 1);
    assert_eq!(probe.after.len(), 1);
    assert_eq!(probe.after[0].remaining, 0);
    assert_eq!(probe.after[0].uses, Some(vec![probe.after[0].site]));
    assert_eq!(probe.after[0].consumers, 1);
}

fn assert_exact_uses_exhaustion(error: ProductionSemanticSsaErrorV1, limit: usize) {
    let ProductionSemanticSsaErrorV1::BorrowFlowWork {
        stage,
        phase_work_units,
        ordered_proof_calls,
        remaining_work_units,
        requested_work_units,
        error,
    } = error
    else {
        panic!("missing original profiled work failure: {error:?}")
    };
    assert_eq!(stage, "uses");
    assert_eq!(ordered_proof_calls, 0);
    assert_eq!(remaining_work_units, 0);
    assert_eq!(requested_work_units, 1);
    assert_eq!(
        phase_work_units.iter().sum::<usize>(),
        limit,
        "no successful work may be refunded"
    );
    assert_eq!(
        *error,
        ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            required: limit + 1,
            limit
        }
    );
}

#[test]
fn grid_direct_uses_removed_push_observation_is_detected() {
    let (result, mut probe, _, _, reads) = run(true, MAX_FLOW_WORK);
    result.unwrap();
    assert_exact_inventory(&probe, reads);
    // Oracle mutation in addition to the separate compiled producer mutation.
    probe.after[0].uses = Some(vec![]);
    assert!(std::panic::catch_unwind(|| assert_exact_inventory(&probe, reads)).is_err());
}
