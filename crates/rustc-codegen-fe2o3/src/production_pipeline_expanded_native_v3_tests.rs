//! Constructed semantic components are not actual-rustc or signed evidence.
use super::*;
use crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::fixtures;
use crate::production_ranked_projection_v1::{
    AuthenticatedRankedVerificationRosterV1, prepare_backend_unroll_direct_prefix_v1,
    prepare_backend_unroll_erased_prefix_v1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[inline(never)]
fn prepare_prefix(
    erased: bool,
    profile: Profile,
    bound: Option<u64>,
    budget: &mut Budget<'_>,
) -> (
    Prefix6,
    AuthenticatedRankedVerificationRosterV1,
    usize,
    usize,
) {
    let (prefix, ranked, storage, source_storage) = if erased {
        let (owner, ranked, storage, source_storage) =
            prepare_backend_unroll_erased_prefix_v1(profile, bound, budget);
        (Prefix6::Erased(owner), ranked, storage, source_storage)
    } else {
        let (owner, ranked, storage, source_storage) =
            prepare_backend_unroll_direct_prefix_v1(profile, bound, budget);
        (Prefix6::Direct(owner), ranked, storage, source_storage)
    };
    assert_eq!(ranked.root_count(), 2);
    assert_eq!(budget.storage(), 29 + source_storage + storage);
    (prefix, ranked, storage, source_storage)
}

fn with_prefix(
    erased: bool,
    profile: Profile,
    bound: Option<u64>,
    run: impl FnOnce(Prefix6, &mut Budget<'_>),
) {
    let mut work = Work::new(
        usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
    );
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    let ledger = budget.work_ledger_identity_v1();
    let (prefix, ranked, storage, source_storage) =
        prepare_prefix(erased, profile, bound, &mut budget);
    let floor = budget.storage();
    run(prefix, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(ranked);
    budget.release_storage(storage).unwrap();
    assert_eq!(budget.storage(), 29 + source_storage);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(budget.storage(), 29);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

struct PreparedExpandedFixture {
    expanded: Expanded,
    prefix_execution: Policy7ExecutionWitnessV1,
    history: PreparedRefinedForwardingHistoryClaimsV1,
    ranked: AuthenticatedRankedVerificationRosterV1,
    prefix_floor: usize,
    prefix_storage: usize,
    source_storage: usize,
    expanded_storage: usize,
    history_storage: usize,
}

// Construction temporaries must leave the stack before a donor callback builds
// a second genuine owner. The returned fields keep the original paid custody.
#[inline(never)]
fn prepare_expanded_fixture(
    erased: bool,
    profile: Profile,
    bound: Option<u64>,
    budget: &mut Budget<'_>,
) -> PreparedExpandedFixture {
    let (prefix, ranked, prefix_storage, source_storage) =
        prepare_prefix(erased, profile, bound, budget);
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let (r, f, u) = Expanded::prefix_limits_v1();
    let (seed, receipt) = prepare_source_seed_v1(prefix, profile, r, f, u, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(seed.retained_floor, budget.storage());
    let original_pointer = seed
        .owner
        .original()
        .unwrap()
        .canonical()
        .canonical_bytes()
        .as_ptr();
    let history = PreparedRefinedForwardingHistoryClaimsV1::prepare_source_v1(
        seed.owner.source(),
        &seed.prefix_execution,
        budget,
    )
    .unwrap();
    let history_storage = history.retained_storage();
    budget.reserve_storage(history_storage).unwrap();
    let PreparedLoopUnrollSourceSeedV1 {
        owner,
        prefix_execution,
        ..
    } = seed;
    let (expanded, added) = match owner {
        Unrolled::Direct(v) => v.continue_expanded_production_policy_v1(budget),
        Unrolled::Erased(v) => v.continue_expanded_production_policy_v1(budget),
    }
    .unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    let slack = source_seed_header(erased).unwrap();
    budget.release_storage(slack).unwrap();
    let original = match expanded.source_anchor() {
        fe2o3_lower_mir_kernel::CanonicalOutputFormalSourceAnchorV1::Direct(v) => {
            v.pre_ranked_executable().unwrap()
        }
        fe2o3_lower_mir_kernel::CanonicalOutputFormalSourceAnchorV1::Erased(v) => {
            v.original_source().executable()
        }
    };
    assert_eq!(
        original.canonical().canonical_bytes().as_ptr(),
        original_pointer
    );
    prefix_execution
        .check_history_v1(final_f(&expanded).history(), budget)
        .unwrap();
    expanded.verify_equivalence(budget).unwrap();
    history
        .check_source_v1(final_f(&expanded), &prefix_execution, budget)
        .unwrap();
    let expanded_storage = receipt
        .retained_storage()
        .checked_add(added.retained_storage())
        .unwrap()
        .checked_sub(slack)
        .unwrap();
    assert_eq!(budget.storage(), floor + history_storage + expanded_storage);
    assert!(budget.work_ledger_identity_v1() == ledger);
    PreparedExpandedFixture {
        expanded,
        prefix_execution,
        history,
        ranked,
        prefix_floor: floor,
        prefix_storage,
        source_storage,
        expanded_storage,
        history_storage,
    }
}

fn with_expanded(
    erased: bool,
    profile: Profile,
    bound: Option<u64>,
    run: impl FnOnce(
        &Expanded,
        &Policy7ExecutionWitnessV1,
        &PreparedRefinedForwardingHistoryClaimsV1,
        &mut Budget<'_>,
    ),
) {
    let mut work = Work::new(
        usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
    );
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    let ledger = budget.work_ledger_identity_v1();
    let PreparedExpandedFixture {
        expanded,
        prefix_execution,
        history,
        ranked,
        prefix_floor,
        prefix_storage,
        source_storage,
        expanded_storage,
        history_storage,
    } = prepare_expanded_fixture(erased, profile, bound, &mut budget);
    run(&expanded, &prefix_execution, &history, &mut budget);
    drop(history);
    budget.release_storage(history_storage).unwrap();
    drop(expanded);
    drop(prefix_execution);
    budget.release_storage(expanded_storage).unwrap();
    assert_eq!(budget.storage(), prefix_floor);
    drop(ranked);
    budget.release_storage(prefix_storage).unwrap();
    assert_eq!(budget.storage(), 29 + source_storage);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(budget.storage(), 29);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn seed_to_expanded_components_preserve_direct_erased_source_and_final_evidence() {
    let mut changing = [0usize; 2];
    for (branch, erased) in [false, true].into_iter().enumerate() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for bound in [Some(3), None] {
                with_expanded(erased, profile, bound, |owner, _, _, budget| {
                    let incoming = budget.storage();
                    let historical = match owner.prefix() {
                        ProductionExpandedPrefixV1::Direct(v) => UnrolledOwnerV1::Direct(v),
                        ProductionExpandedPrefixV1::Erased(v) => UnrolledOwnerV1::Erased(v),
                    };
                    let roots = fixtures::typed_roots(historical.final_f());
                    descriptor::tests::exercise_actual_final(owner, &roots, profile, budget);
                    let changed = unrolled(owner).canonical().canonical_bytes()
                        != owner.output().canonical().canonical_bytes();
                    changing[branch] += usize::from(changed);
                    let ProductionExpandedHistoryV1::ScalarCleanup(core) = owner.history();
                    assert!(!core.rounds().is_empty() && core.rounds().len() <= 16);
                    assert_eq!(
                        core.output().canonical().canonical_bytes(),
                        owner.output().canonical().canonical_bytes()
                    );
                    assert!(!owner.grants_artifact_or_launch_authority());
                    let (llvm, receipt) = lower_native(owner.output(), profile, budget).unwrap();
                    budget.reserve_storage(receipt).unwrap();
                    assert!(llvm.contains("define amdgpu_kernel"));
                    descriptor::tests::exercise_common_transport(
                        owner, &roots, profile, &llvm, budget,
                    );
                    drop(llvm);
                    budget.release_storage(receipt).unwrap();
                    assert_eq!(budget.storage(), incoming);
                });
            }
        }
    }
    assert!(
        changing.iter().all(|n| *n > 0),
        "require real scalar changes in both constructed branches"
    );
}

#[test]
fn complete_history_replay_rejects_foreign_source_pairs_and_preserves_both_originals() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for erased in [false, true] {
            with_expanded(
                erased,
                profile,
                Some(3),
                |left, left_execution, left_history, left_budget| {
                    with_expanded(
                        erased,
                        profile,
                        Some(4),
                        |right, right_execution, right_history, right_budget| {
                            // This is a borrowed replay meter, not a producer transfer:
                            // both genuine constructed owners remain in their original
                            // scopes, while the checker prepays both complete inputs.
                            let floor = left_budget.storage() + right_budget.storage();
                            let mut work = Work::new(usize::MAX);
                            let mut budget = Budget::new(&mut work, usize::MAX);
                            budget.reserve_storage(floor).unwrap();
                            let left_bytes = left.output().canonical().canonical_bytes();
                            let right_bytes = right.output().canonical().canonical_bytes();
                            assert_ne!(left_bytes, right_bytes);
                            for result in [
                                left_history.check_source_v1(
                                    final_f(right),
                                    right_execution,
                                    &mut budget,
                                ),
                                right_history.check_source_v1(
                                    final_f(left),
                                    left_execution,
                                    &mut budget,
                                ),
                            ] {
                                let error =
                                    result.expect_err("complete source/history donor must refuse");
                                assert!(
                                    resource_cause(&error).is_none(),
                                    "resource failure is not a semantic oracle"
                                );
                                assert_eq!(budget.storage(), floor);
                            }
                            left_history
                                .check_source_v1(final_f(left), left_execution, &mut budget)
                                .unwrap();
                            right_history
                                .check_source_v1(final_f(right), right_execution, &mut budget)
                                .unwrap();
                            left.verify_equivalence(&mut budget).unwrap();
                            right.verify_equivalence(&mut budget).unwrap();
                            assert_eq!(left.output().canonical().canonical_bytes(), left_bytes);
                            assert_eq!(right.output().canonical().canonical_bytes(), right_bytes);
                            assert_eq!(budget.storage(), floor);
                        },
                    );
                },
            );
        }
    }
}

#[test]
fn pre_native_seed_first_work_and_storage_boundaries_have_independent_oracles() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for deny_work in [false, true] {
                with_prefix(erased, profile, Some(3), |prefix, original| {
                    let floor = original.storage();
                    let header = source_seed_header(erased).unwrap();
                    let mut work = Work::new(if deny_work { 19 } else { usize::MAX });
                    let mut budget = Budget::new(
                        &mut work,
                        if deny_work {
                            usize::MAX
                        } else {
                            floor + header - 1
                        },
                    );
                    budget.charge_work(17).unwrap();
                    budget.reserve_storage(floor).unwrap();
                    let (r, f, u) = Expanded::prefix_limits_v1();
                    let result = prepare_source_seed_v1(prefix, profile, r, f, u, &mut budget);
                    let error = match result {
                        Ok(_) => panic!("first boundary must refuse"),
                        Err(e) => e,
                    };
                    assert_eq!(budget.storage(), floor);
                    let resource = resource_cause(&error).unwrap();
                    if deny_work {
                        let Resource::Work(limit) = resource else {
                            panic!("{resource:?}")
                        };
                        assert_eq!((limit.actual(), limit.limit(), budget.work()), (20, 19, 17));
                        assert_eq!(budget.peak_storage(), floor);
                        assert_eq!(budget.failed_storage(), None);
                    } else {
                        let Resource::Storage(limit) = resource else {
                            panic!("{resource:?}")
                        };
                        assert_eq!(
                            (limit.actual(), limit.limit()),
                            (floor + header, floor + header - 1)
                        );
                        assert_eq!((budget.work(), budget.peak_storage()), (20, floor));
                        assert_eq!(budget.failed_storage(), Some(floor + header));
                    }
                    assert_eq!(original.storage(), floor);
                });
            }
        }
    }
}
fn resource_cause(error: &(dyn std::error::Error + 'static)) -> Option<Resource> {
    error
        .downcast_ref::<Resource>()
        .copied()
        .or_else(|| error.source().and_then(resource_cause))
}

#[test]
fn final_wrapper_header_and_fixed_policy_have_no_duplicate_embedded_headers() {
    assert_eq!(
        wrapper_header().unwrap()
            + size_of::<Expanded>()
            + size_of::<Policy7ExecutionWitnessV1>()
            + size_of::<PreparedRefinedForwardingHistoryClaimsV1>()
            + size_of::<String>(),
        size_of::<ExpandedNativeProductionCompilationV3>()
    );
    for erased in [false, true] {
        assert_eq!(
            source_seed_header(erased).unwrap()
                + Unrolled::active_header(erased)
                + size_of::<Policy7ExecutionWitnessV1>(),
            size_of::<PreparedLoopUnrollSourceSeedV1>()
        );
    }
    let (loops, forwarding, unroll) = Expanded::prefix_limits_v1();
    assert_eq!(loops.operations, 65_536);
    assert_eq!(forwarding.memory.operations, 65_536);
    assert_eq!(unroll.loops, loops);
    assert_eq!(unroll.max_iterations, 8);
}

#[test]
fn production_source_seed_contains_no_native_call_and_final_entry_has_fixed_order() {
    let seed = include_str!("production_pipeline_loop_unroll_native_v1.rs")
        .split("fn prepare_source_seed_v1(")
        .nth(1)
        .unwrap()
        .split("\nfn prepare(")
        .next()
        .unwrap();
    assert!(!seed.contains("lower_native(") && !seed.contains("llvm:"));
    let source = include_str!("production_pipeline_expanded_native_v3.rs");
    let body = source
        .split("fn prepare_ranked(")
        .nth(1)
        .unwrap()
        .split("impl ExpandedFinalNativeCustodyV3")
        .next()
        .unwrap();
    let seed = body.find("prepare_source_seed_v1(").unwrap();
    let scalar = body
        .find("continue_expanded_production_policy_v1(")
        .unwrap();
    let descriptor = body.find("descriptor::produce(").unwrap();
    let native = body.find("lower_native(owner.output()").unwrap();
    assert!(seed < scalar && scalar < descriptor && descriptor < native);
    assert_eq!(body.matches("lower_native(").count(), 1);
    assert!(
        !body.contains("Default::default") && !body.contains("prepare_nominal_loop_unroll_native")
    );
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn reset_expanded_source_trace_v3(&self) {
        emission_trace::reset();
    }
}
impl ExpandedNativeProductionCompilationV3 {
    pub(crate) fn expanded_source_ordered_roots_v3(&self) -> Vec<(u32, String)> {
        let semantic = match self.custody.owner.source_anchor() {
            fe2o3_lower_mir_kernel::CanonicalOutputFormalSourceAnchorV1::Direct(v) => {
                v.semantic().semantic()
            }
            fe2o3_lower_mir_kernel::CanonicalOutputFormalSourceAnchorV1::Erased(v) => {
                v.original_source().semantic_ssa().source_semantic()
            }
        };
        semantic
            .roots()
            .iter()
            .map(|id| {
                let function = &semantic.functions()[id.index() as usize];
                let name = std::str::from_utf8(
                    function.kernel_entry().unwrap().export_symbol().as_bytes(),
                )
                .unwrap()
                .to_owned();
                (id.index(), name)
            })
            .collect()
    }
    pub(crate) fn expanded_source_observation_v3(
        &self,
    ) -> (bool, usize, usize, [u8; 32], [u8; 32], (usize, [u8; 4])) {
        let erased = matches!(
            self.custody.owner.prefix(),
            ProductionExpandedPrefixV1::Erased(_)
        );
        let ProductionExpandedHistoryV1::ScalarCleanup(core) = self.custody.owner.history();
        let mut previous = unrolled(&self.custody.owner).canonical().canonical_bytes();
        let mut changes = 0;
        for round in core.rounds() {
            let next = round.scalar().owner().canonical().canonical_bytes();
            changes += usize::from(next != previous);
            previous = next;
        }
        (
            erased,
            core.rounds().len(),
            changes,
            *unrolled(&self.custody.owner)
                .canonical()
                .identity()
                .digest(),
            *self.output().canonical().identity().digest(),
            emission_trace::get(),
        )
    }
    pub(crate) fn expanded_source_hostiles_v3(&mut self, budget: &mut Budget<'_>) -> R<()> {
        let floor = budget.storage();
        let historical = match self.custody.owner.prefix() {
            ProductionExpandedPrefixV1::Direct(v) => UnrolledOwnerV1::Direct(v),
            ProductionExpandedPrefixV1::Erased(v) => UnrolledOwnerV1::Erased(v),
        };
        let mut stale_wire = crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::loop_unroll_v1::nominal_v3::produce(
            historical, &self.custody.bindings.typed_descriptor_roots,
            &self.custody.bindings.rustc_target, budget,
        ).map_err(E::Nominal)?;
        let wire_storage = stale_wire.capacity() + size_of::<Vec<u8>>();
        budget.reserve_storage(wire_storage)?;
        assert_ne!(stale_wire, self.wire);
        std::mem::swap(&mut stale_wire, &mut self.wire);
        let result = self.verify_equivalence(budget);
        std::mem::swap(&mut stale_wire, &mut self.wire);
        assert!(matches!(
            result,
            Err(E::Mismatch("actual expanded-final V3 bytes"))
        ));
        drop(stale_wire);
        budget.release_storage(wire_storage)?;
        if unrolled(&self.custody.owner).canonical().canonical_bytes()
            != self.output().canonical().canonical_bytes()
        {
            let (mut stale_llvm, storage) =
                lower_native(unrolled(&self.custody.owner), self.custody.profile, budget)
                    .map_err(pipeline)?;
            budget.reserve_storage(storage)?;
            assert_ne!(stale_llvm, self.custody.llvm);
            std::mem::swap(&mut stale_llvm, &mut self.custody.llvm);
            let result = self.verify_equivalence(budget);
            std::mem::swap(&mut stale_llvm, &mut self.custody.llvm);
            assert!(result.is_err());
            drop(stale_llvm);
            budget.release_storage(storage)?;
        }
        let old = self.custody.llvm.as_bytes()[0];
        self.custody.llvm.replace_range(..1, "!");
        assert!(self.verify_equivalence(budget).is_err());
        self.custody
            .llvm
            .replace_range(..1, std::str::from_utf8(&[old]).unwrap());
        let last = self.wire.len() - 1;
        self.wire[last] ^= 1;
        assert!(matches!(
            self.verify_equivalence(budget),
            Err(E::Mismatch("actual expanded-final V3 bytes"))
        ));
        self.wire[last] ^= 1;
        let profile = self.custody.profile;
        self.custody.profile = match profile {
            Profile::Gfx942 => Profile::Gfx950,
            Profile::Gfx950 => Profile::Gfx942,
        };
        assert!(matches!(
            self.verify_equivalence(budget),
            Err(E::Mismatch("complete expanded target/ranked custody"))
        ));
        self.custody.profile = profile;
        assert_eq!(budget.storage(), floor);
        self.verify_equivalence(budget)
    }
}

#[path = "production_expanded_history_serializer_v1_tests.rs"]
mod serialized_history;
