//! Constructed source admission is not an authenticated ordinary-rustc wrapper.
use super::*;
use crate::production_ranked_projection_v1::{
    AuthenticatedRankedVerificationRosterV1, prepare_backend_unroll_direct_prefix_v1,
    prepare_backend_unroll_erased_prefix_v1, with_backend_unroll_direct_prefix_v1,
    with_backend_unroll_erased_prefix_v1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirInductionRefinementOriginV1 as RefineOrigin,
    CanonicalKirLoopUnrollCopyV1 as CopyRole,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
#[path = "production_pipeline_loop_unroll_native_joins_v1_tests.rs"]
mod joins;
#[path = "production_pipeline_loop_unroll_native_resources_v1_tests.rs"]
mod resources;
#[path = "production_pipeline_loop_unroll_native_sim_v1_tests.rs"]
mod sim;

const WORK: usize = usize::MAX;
const STORAGE: usize = usize::MAX;
const PROFILES: [Profile; 2] = [Profile::Gfx942, Profile::Gfx950];
fn with_prefix(
    erased: bool,
    profile: Profile,
    bound: Option<u64>,
    next: impl FnOnce(Prefix6, &mut Budget<'_>),
) {
    if erased {
        with_backend_unroll_erased_prefix_v1(profile, bound, |v, ranked, b| {
            assert_eq!(ranked.root_count(), 2);
            next(Prefix6::Erased(v), b);
        });
    } else {
        with_backend_unroll_direct_prefix_v1(profile, bound, |v, ranked, b| {
            assert_eq!(ranked.root_count(), 2);
            next(Prefix6::Direct(v), b);
        });
    }
}
fn tail(owner: &Unrolled) -> &fe2o3_kernel_opt::OwnedLoopUnrollV1 {
    match owner {
        Unrolled::Direct(v) => v.continuation(),
        Unrolled::Erased(v) => v.continuation(),
    }
}
// Retire construction temporaries before a callback constructs another owner.
#[inline(never)]
fn prepare_checked_fixture(
    prefix: Prefix6,
    profile: Profile,
    bound: Option<u64>,
    budget: &mut Budget<'_>,
) -> (PreparedLoopUnrollNativeOutputV1, LoopUnrollNativeStorageV1) {
    let floor = budget.storage();
    let original = match &prefix {
        Prefix6::Direct(v) => v.source_semantic_kir().pre_ranked_executable().unwrap(),
        Prefix6::Erased(v) => v.original_source().executable(),
    };
    let original_pointer = original.canonical().canonical_bytes().as_ptr();
    let original_identity = *original.canonical().identity().digest();
    let (owner, receipt) = prepare(
        prefix,
        profile,
        Limits::default(),
        ForwardingLimits::default(),
        UnrollLimits::default(),
        budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), owner.retained_storage_floor_v1());
    assert_eq!(
        owner
            .original()
            .unwrap()
            .canonical()
            .canonical_bytes()
            .as_ptr(),
        original_pointer
    );
    assert_eq!(
        *owner.original().unwrap().canonical().identity().digest(),
        original_identity
    );
    shape(&owner, bound, budget);
    (owner, receipt)
}
fn with_prepared(
    erased: bool,
    profile: Profile,
    bound: Option<u64>,
    run: impl FnOnce(&mut PreparedLoopUnrollNativeOutputV1, &mut Budget<'_>),
) {
    with_prefix(erased, profile, bound, |prefix, budget| {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let (mut owner, receipt) = prepare_checked_fixture(prefix, profile, bound, budget);
        run(&mut owner, budget);
        drop(owner);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    });
}

// Caller-owned meters stay live while each complete construction scope returns.
struct PreparedPairFixture {
    native: PreparedLoopUnrollNativeOutputV1,
    ranked: AuthenticatedRankedVerificationRosterV1,
    native_storage: LoopUnrollNativeStorageV1,
    prefix_floor: usize,
    p6_storage: usize,
    source_storage: usize,
}
#[inline(never)]
fn prepare_pair_fixture(
    erased: bool,
    profile: Profile,
    budget: &mut Budget<'_>,
) -> PreparedPairFixture {
    assert_eq!(budget.storage(), 0);
    let ledger = budget.work_ledger_identity_v1();
    let (prefix, ranked, p6_storage, source_storage) = if erased {
        let (owner, ranked, p6_storage, source_storage) =
            prepare_backend_unroll_erased_prefix_v1(profile, Some(3), budget);
        (Prefix6::Erased(owner), ranked, p6_storage, source_storage)
    } else {
        let (owner, ranked, p6_storage, source_storage) =
            prepare_backend_unroll_direct_prefix_v1(profile, Some(3), budget);
        (Prefix6::Direct(owner), ranked, p6_storage, source_storage)
    };
    assert_eq!(ranked.root_count(), 2);
    let prefix_floor = budget.storage();
    assert_eq!(prefix_floor, 29 + source_storage + p6_storage);
    let (native, native_storage) = prepare_checked_fixture(prefix, profile, Some(3), budget);
    assert!(budget.work_ledger_identity_v1() == ledger);
    PreparedPairFixture {
        native,
        ranked,
        native_storage,
        prefix_floor,
        p6_storage,
        source_storage,
    }
}
fn release_pair_fixture(fixture: PreparedPairFixture, budget: &mut Budget<'_>) {
    let PreparedPairFixture {
        native,
        ranked,
        native_storage,
        prefix_floor,
        p6_storage,
        source_storage,
    } = fixture;
    assert_eq!(
        budget.storage(),
        prefix_floor + native_storage.retained_storage()
    );
    assert_eq!(ranked.root_count(), 2);
    drop(native);
    budget
        .release_storage(native_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), prefix_floor);
    drop(ranked);
    budget.release_storage(p6_storage).unwrap();
    assert_eq!(budget.storage(), 29 + source_storage);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(budget.storage(), 29);
}
fn assert_no_loops(graph: &Graph, budget: &mut Budget<'_>) {
    use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, CanonicalKirLoopsV1};

    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let (inventory, inventory_receipt) = CanonicalKirInventoryV1::derive(graph, budget).unwrap();
    budget
        .reserve_storage(inventory_receipt.retained_storage())
        .unwrap();
    let (loops, loop_receipt) =
        CanonicalKirLoopsV1::derive(&inventory, Default::default(), budget).unwrap();
    budget
        .reserve_storage(loop_receipt.retained_storage())
        .unwrap();
    loops
        .replay(&inventory, Default::default(), budget)
        .unwrap();
    assert_eq!(loops.loop_count(), 0);
    drop(loops);
    budget
        .release_storage(loop_receipt.retained_storage())
        .unwrap();
    drop(inventory);
    budget
        .release_storage(inventory_receipt.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
}
fn shape(owner: &PreparedLoopUnrollNativeOutputV1, bound: Option<u64>, budget: &mut Budget<'_>) {
    // The genuine zero-bound source is erased by the earlier checked CFG prefix.
    // Canonical zero-trip cloning is not claimed for this source route.
    if bound == Some(0) {
        assert_no_loops(owner.forwarding_output(), budget);
    }
    let selected = bound
        .and_then(|v| u8::try_from(v).ok())
        .filter(|n| *n > 0 && *n <= 8);
    assert_eq!(
        tail(&owner.owner).origins().selection.map(|s| s.iterations),
        selected
    );
    assert_eq!(owner.limits(), UnrollLimits::default());
    assert!(!std::ptr::eq(owner.forwarding_output(), owner.output()));
    assert_eq!(owner.output().module().kernels.len(), 2);
    assert_eq!(
        owner.output().module().kernels,
        owner.forwarding_output().module().kernels
    );
    assert_eq!(owner.prefix_execution.policy_version(), 7);
    assert!(!owner.grants_artifact_or_launch_authority());
    assert!(!owner.llvm_ir().contains(".fe2o3.kd.v1"));
    if selected.is_some() {
        assert_ne!(
            owner.forwarding_output().canonical().identity(),
            owner.output().canonical().identity()
        );
    } else {
        assert_eq!(
            owner.forwarding_output().canonical().canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
    }
    macro_rules! lineage {
        ($u:expr) => {{
            let u = $u;
            assert_eq!(u.kernels().len(), 2);
            if bound.is_none_or(|n| n != 0) {
                assert_eq!(
                    u.prefix()
                        .refinement_origins()
                        .iter()
                        .filter(|r| matches!(
                            r.canonical_origin(),
                            RefineOrigin::CheckedAddSplit { .. }
                        ))
                        .count(),
                    2
                );
                assert_eq!(
                    u.prefix()
                        .origins()
                        .iter()
                        .filter(|r| r.canonical_origin().store.is_some())
                        .count(),
                    2
                );
            }
            let mut destinations = std::collections::BTreeSet::new();
            for source in u.origins() {
                let row = source.canonical_origin();
                let mut old = u
                    .prefix()
                    .origins()
                    .iter()
                    .filter(|r| r.canonical_origin().output == row.input);
                let predecessor = old.next().unwrap();
                assert!(old.next().is_none());
                assert_eq!(
                    source.original_source_statement(),
                    predecessor.original_source_statement()
                );
                if let Some(output) = row.output {
                    assert!(destinations.insert(output));
                } else {
                    assert_eq!(selected, Some(0));
                    assert_eq!(row.copy, CopyRole::OmittedBody);
                }
            }
            let count: usize = u
                .output()
                .module()
                .functions
                .iter()
                .filter_map(|f| f.body.as_ref())
                .flat_map(|b| &b.blocks)
                .map(|b| b.operations.len())
                .sum();
            assert_eq!(destinations.len(), count);
        }};
    }
    match &owner.owner {
        Unrolled::Direct(v) => lineage!(v),
        Unrolled::Erased(v) => lineage!(v),
    }
    let floor = budget.storage();
    let receipt = {
        let (pair, receipt) = tail(&owner.owner)
            .replay(owner.forwarding_output(), owner.limits(), budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(std::ptr::eq(pair.input(), owner.forwarding_output()));
        assert!(std::ptr::eq(pair.output(), owner.output()));
        receipt
    };
    budget.release_storage(receipt.retained_storage()).unwrap();
    owner.verify_equivalence(budget).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn loop_unroll_native_actual_source_nonzero_clones_before_first_emission() {
    for erased in [false, true] {
        for profile in PROFILES {
            for n in [1, 3, 8] {
                with_prepared(erased, profile, Some(n), |owner, budget| {
                    assert!(
                        tail(&owner.owner)
                            .origins()
                            .operations
                            .iter()
                            .any(|r| matches!(r.copy, CopyRole::Body(0)))
                    );
                    assert!(
                        tail(&owner.owner)
                            .origins()
                            .operations
                            .iter()
                            .any(|r| r.copy == CopyRole::Header(n as u8))
                    );
                    let (f_text, receipt) =
                        lower_native(owner.forwarding_output(), profile, budget).unwrap();
                    budget.reserve_storage(receipt).unwrap();
                    assert_ne!(owner.llvm_ir(), f_text);
                    drop(f_text);
                    budget.release_storage(receipt).unwrap();
                });
            }
        }
    }
}
#[test]
fn loop_unroll_native_dynamic_and_over_ceiling_remain_exact_real_noops() {
    for erased in [false, true] {
        for profile in PROFILES {
            for bound in [None, Some(9)] {
                with_prepared(erased, profile, bound, |owner, _| {
                    assert!(
                        tail(&owner.owner)
                            .origins()
                            .operations
                            .iter()
                            .all(|r| r.copy == CopyRole::Retained)
                    );
                });
            }
        }
    }
}
#[test]
fn loop_unroll_native_zero_trip_prefix_erasure_remains_a_checked_noop() {
    for erased in [false, true] {
        for profile in PROFILES {
            with_prepared(erased, profile, Some(0), |owner, _| {
                assert_eq!(tail(&owner.owner).origins().selection, None);
                assert!(
                    tail(&owner.owner)
                        .origins()
                        .operations
                        .iter()
                        .all(|r| r.copy == CopyRole::Retained)
                );
                assert_eq!(
                    owner.forwarding_output().canonical().canonical_bytes(),
                    owner.output().canonical().canonical_bytes()
                );
            });
        }
    }
}
fn native_refusal(result: Result<()>) {
    assert!(
        matches!(
            &result,
            Err(ProductionPipelineError::InductionRefinementNativeStage(
                InductionRefinementNativeStageErrorV1::ForwardingComposition(
                    RefinedForwardingNativeStageErrorV1::BoundedUnroll(
                        LoopUnrollNativeStageErrorV1::Mismatch("exact actual U native LLVM")
                    )
                )
            ))
        ),
        "expected final U text mismatch, got {result:?}"
    );
}
#[test]
fn loop_unroll_native_replay_rejects_f_text_and_wrong_target_without_losing_custody() {
    for erased in [false, true] {
        for profile in PROFILES {
            with_prepared(erased, profile, Some(3), |owner, budget| {
                let floor = budget.storage();
                let (mut foreign, receipt) =
                    lower_native(owner.forwarding_output(), profile, budget).unwrap();
                budget.reserve_storage(receipt).unwrap();
                std::mem::swap(&mut foreign, &mut owner.llvm);
                native_refusal(owner.verify_equivalence(budget));
                std::mem::swap(&mut foreign, &mut owner.llvm);
                drop(foreign);
                budget.release_storage(receipt).unwrap();
                let original = owner.profile;
                owner.profile = if original == Profile::Gfx942 {
                    Profile::Gfx950
                } else {
                    Profile::Gfx942
                };
                match owner.verify_equivalence(budget) {
                    Err(ProductionPipelineError::TargetLowering(diagnostics)) => assert!(
                        diagnostics.contains(
                            dialect_amdgcn::LoweringDiagnosticCode::UnsupportedCapability
                        ),
                        "wrong target must fail capability preflight: {diagnostics:?}"
                    ),
                    other => panic!("wrong target must fail native target preflight: {other:?}"),
                }
                owner.profile = original;
                owner.verify_equivalence(budget).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}
