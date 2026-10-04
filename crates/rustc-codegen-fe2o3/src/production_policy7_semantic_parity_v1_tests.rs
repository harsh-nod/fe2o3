//! Frozen pre-factor sequence as a test-only error/work/storage parity oracle.
use super::*;

fn legacy<'a>(
    stage: &'a PreparedPolicy7ArtifactsV1,
    claims: PortablePolicy7ClaimsV1<'a>,
    budget: &mut Budget<'_>,
) -> Result7<usize> {
    let receipt = scoped(stage.retained_floor, budget, |budget| {
        let wrapper = size_of::<AuthenticatedPolicy7ExecutionRelationV1<'_>>()
            .checked_sub(size_of::<CheckedCanonicalPolicy6ExecutionRelationV1<'_>>())
            .and_then(|n| {
                n.checked_sub(size_of::<CheckedCanonicalPolicy7ContinuationRelationV1<'_>>())
            })
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(wrapper).map_err(resource)?;
        stage.admitted.replay(budget)?;
        budget
            .reserve_storage(fe2o3_kernel_opt::POLICY7_EXECUTION_HEADER_BYTES_V1)
            .map_err(resource)?;
        let checked_record = stage.execution.check(&stage.admitted, budget);
        budget
            .release_storage(fe2o3_kernel_opt::POLICY7_EXECUTION_HEADER_BYTES_V1)
            .map_err(resource)?;
        checked_record?;
        let (bound, checked) = stage.admitted.portable_prefix();
        let prefix = check_canonical_policy6_execution_relation_v1(
            bound,
            checked,
            claims.policy4_wire,
            claims.policy5_record,
            claims.load_rows,
            claims.policy6,
            budget,
        )
        .map_err(|e| portable(CanonicalPolicy7SemanticErrorV1::Policy6(Box::new(e))))?;
        let prefix_storage = prefix.storage().retained_storage();
        budget.reserve_storage(prefix_storage).map_err(resource)?;
        let actual = stage.admitted.continuation();
        let continuation = check_canonical_policy7_continuation_relation_v1(
            prefix.authenticated_composition_record(),
            stage.admitted.input(),
            stage.admitted.output(),
            CanonicalPolicy7ContinuationClaimsV1 {
                execution_record: claims.policy7_record,
                deletion_rows: actual.rows(),
                retained_operations: actual.retained_operations(),
            },
            budget,
        )
        .map_err(portable)?;
        let continuation_storage = continuation.storage().retained_storage();
        budget
            .reserve_storage(continuation_storage)
            .map_err(resource)?;
        budget
            .charge_work(
                claims
                    .policy7_record
                    .len()
                    .checked_add(stage.execution.canonical_bytes().len())
                    .ok_or_else(|| resource(Resource::Arithmetic))?,
            )
            .map_err(resource)?;
        if claims.policy7_record != stage.execution.canonical_bytes() {
            return Err(execution_error("exact portable Policy7 execution record"));
        }
        let retained = wrapper
            .checked_add(prefix_storage)
            .and_then(|n| n.checked_add(continuation_storage))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        Ok(AuthenticatedPolicy7ExecutionRelationV1 {
            prefix,
            continuation,
            actual: stage,
            retained,
        })
    })?;
    assert!(std::ptr::eq(receipt.actual_stage(), stage));
    assert_eq!(
        receipt.continuation().unauthenticated_execution_record(),
        stage.execution().canonical_bytes()
    );
    let retained = receipt.retained_storage();
    budget.reserve_storage(retained).map_err(resource)?;
    drop(receipt);
    budget.release_storage(retained).map_err(resource)?;
    Ok(retained)
}

fn factored<'a>(
    stage: &'a PreparedPolicy7ArtifactsV1,
    claims: PortablePolicy7ClaimsV1<'a>,
    budget: &mut Budget<'_>,
) -> Result7<usize> {
    let receipt = stage.check_portable_execution_relation_v1(
        claims.policy4_wire,
        claims.policy5_record,
        claims.load_rows,
        claims.policy6,
        claims.policy7_record,
        budget,
    )?;
    assert!(std::ptr::eq(receipt.actual_stage(), stage));
    assert_eq!(
        receipt.continuation().unauthenticated_execution_record(),
        stage.execution().canonical_bytes()
    );
    let retained = receipt.retained_storage();
    budget.reserve_storage(retained).map_err(resource)?;
    drop(receipt);
    budget.release_storage(retained).map_err(resource)?;
    Ok(retained)
}

pub(super) fn exercise(
    stage: &PreparedPolicy7ArtifactsV1,
    policy4_wire: &[u8],
    policy6: CanonicalPolicy6ContinuationClaimsV1<'_>,
    parent: &Budget<'_>,
) {
    let (_, checked) = stage.admitted.portable_prefix();
    let p5 = checked.intermediate_policy5();
    let claims = PortablePolicy7ClaimsV1 {
        policy4_wire,
        policy5_record: p5.execution().canonical_bytes(),
        load_rows: p5.load_forwarding_rows(),
        policy6,
        policy7_record: stage.execution().canonical_bytes(),
    };
    let floor = parent.storage();
    let run = |old: bool, claims: PortablePolicy7ClaimsV1<'_>, work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = if old {
            legacy(stage, claims, &mut budget)
        } else {
            factored(stage, claims, &mut budget)
        };
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result.map_err(|error| format!("{error:?}")),
            budget.work(),
            budget.peak_storage(),
        )
    };
    let old = run(true, claims, 1_000_000_000, 512 << 20);
    assert!(old.0.is_ok());
    let new = run(false, claims, 1_000_000_000, 512 << 20);
    assert_eq!(old, new);
    for (work, storage) in [(old.1, old.2), (old.1 - 1, old.2), (old.1, old.2 - 1)] {
        assert_eq!(
            run(true, claims, work, storage),
            run(false, claims, work, storage)
        );
    }
    // Claims are external test inputs; both invocations borrow the same backing.
    // Multiple defects establish first-error parity, not merely broad is_err.
    let bad = [0_u8; 1];
    for altered in [
        PortablePolicy7ClaimsV1 {
            policy4_wire: &bad,
            policy7_record: &bad,
            ..claims
        },
        PortablePolicy7ClaimsV1 {
            policy5_record: &bad,
            policy7_record: &bad,
            ..claims
        },
        PortablePolicy7ClaimsV1 {
            policy6: CanonicalPolicy6ContinuationClaimsV1 {
                composition_record: &bad,
                integer_record: &bad,
                transition_wire: &bad,
            },
            policy7_record: &bad,
            ..claims
        },
        PortablePolicy7ClaimsV1 {
            policy7_record: &bad,
            ..claims
        },
    ] {
        let expected = run(true, altered, 1_000_000_000, 512 << 20);
        assert!(expected.0.is_err());
        assert_eq!(expected, run(false, altered, 1_000_000_000, 512 << 20));
    }
}
