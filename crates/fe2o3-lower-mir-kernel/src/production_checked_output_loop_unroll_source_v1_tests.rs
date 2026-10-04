use super::*;
use crate::{
    ProductionLoopUnrollErrorV1 as Error, ProductionLoopUnrollStorageV1 as Receipt,
    ProductionOwnedLoopUnrollContinuationV1 as DirectU,
    ProductionOwnedRefinedCrossBlockForwardingContinuationV1 as DirectF,
    ProductionOwnedUnitLocalLoopUnrollContinuationV1 as ErasedU,
    ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1 as ErasedF,
};
use fe2o3_kernel_analysis::{
    CanonicalKirInductionRefinementOriginV1 as RefineOrigin,
    CanonicalKirLoopUnrollCopyV1 as CopyRole, CanonicalKirLoopUnrollLimitsV1 as Limits,
};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 as Graph;

#[path = "production_checked_output_loop_unroll_resource_v1_tests.rs"]
mod resources;
#[path = "production_checked_output_loop_unroll_sim_v1_tests.rs"]
mod sim;

// Change only unadmitted semantic declarations. Every ordinary source/prefix
// gate runs after this transform; no admitted or canonical owner is replaced.
fn literal_source(shared: bool, bound: Option<u32>) -> ProductionPreRankedKirOwnerV1 {
    source_with(shared, true, |functions| {
        for (ordinal, function) in functions.iter_mut().enumerate() {
            if function.role() != SemanticFunctionRoleV1::KernelRoot {
                continue;
            }
            let tag = 30 + ordinal as u8 * 10;
            let bound = bound.map_or_else(|| value(3, U32), |n| constant(U32, n.into(), 4));
            let mut blocks = vec![
                block(
                    tag + 1,
                    vec![assignment(
                        4,
                        U32,
                        SemanticRvalueKindV1::Use(constant(U32, 0, 4)),
                    )],
                    switch(value(3, U32), 0, 1, 2),
                ),
                block(tag + 2, vec![], jump(3)),
                block(tag + 3, vec![], jump(3)),
                block(
                    tag + 4,
                    vec![binary(
                        5,
                        BOOL,
                        SemanticBinaryOpV1::LessThan,
                        value(4, U32),
                        bound,
                    )],
                    switch(value(5, BOOL), 0, 7, 4),
                ),
                block(
                    tag + 5,
                    vec![
                        binary(
                            6,
                            U32,
                            SemanticBinaryOpV1::BitAnd,
                            value(3, U32),
                            constant(U32, 1, 4),
                        ),
                        binary(
                            7,
                            BOOL,
                            SemanticBinaryOpV1::Equal,
                            value(6, U32),
                            constant(U32, 0, 4),
                        ),
                        store(99),
                    ],
                    switch(value(7, BOOL), 0, 6, 5),
                ),
                block(
                    tag + 6,
                    vec![assignment(2, U32, SemanticRvalueKindV1::Use(value(1, U32)))],
                    jump(6),
                ),
                block(
                    tag + 7,
                    vec![binary(
                        4,
                        U32,
                        SemanticBinaryOpV1::Add,
                        value(4, U32),
                        constant(U32, 1, 4),
                    )],
                    jump(3),
                ),
                block(
                    tag + 8,
                    vec![],
                    if shared {
                        call(0, 8)
                    } else {
                        SemanticTerminatorKindV1::Return
                    },
                ),
            ];
            if shared {
                blocks.push(block(
                    tag + 9,
                    vec![],
                    if ordinal == 1 {
                        call(2, 9)
                    } else {
                        SemanticTerminatorKindV1::Return
                    },
                ));
                if ordinal == 1 {
                    blocks.push(block(tag + 10, vec![], SemanticTerminatorKindV1::Return));
                }
            }
            *function = SemanticFunctionDeclV1::new(
                function.identity(),
                function.role(),
                function.item_definition_identity(),
                function.monomorphization_identity(),
                function.generic_type_arguments_identity(),
                function.const_generic_arguments_identity(),
                function.source(),
                function.abi().clone(),
                function.locals().to_vec(),
                function.entry(),
                blocks,
            )
            .unwrap()
            .with_kernel_entry(function.kernel_entry().unwrap().clone());
        }
    })
}
#[allow(
    clippy::large_enum_variant,
    reason = "test retains one actual move-only source owner without a new Box"
)]
enum FinalFixture {
    Direct(DirectF),
    Erased(ErasedF),
}
#[allow(
    clippy::large_enum_variant,
    reason = "test retains one actual move-only source owner without a new Box"
)]
enum UnrolledFixture {
    Direct(DirectU),
    Erased(ErasedU),
}
impl FinalFixture {
    fn output(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    fn unroll(
        self,
        limits: Limits,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(UnrolledFixture, Receipt), Error> {
        match self {
            Self::Direct(v) => v
                .continue_bounded_loop_unroll_v1(limits, budget)
                .map(|(v, r)| (UnrolledFixture::Direct(v), r)),
            Self::Erased(v) => v
                .continue_bounded_loop_unroll_v1(limits, budget)
                .map(|(v, r)| (UnrolledFixture::Erased(v), r)),
        }
    }
}
impl UnrolledFixture {
    fn input(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().output(),
            Self::Erased(v) => v.prefix().output(),
        }
    }
    fn output(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    fn tail(&self) -> &fe2o3_kernel_opt::OwnedLoopUnrollV1 {
        match self {
            Self::Direct(v) => v.continuation(),
            Self::Erased(v) => v.continuation(),
        }
    }
    fn origins(&self) -> &[crate::ProductionLoopUnrollOriginV1] {
        match self {
            Self::Direct(v) => v.origins(),
            Self::Erased(v) => v.origins(),
        }
    }
    fn kernels(&self) -> &[fe2o3_kernel_ir::FormalMemoryObligations] {
        match self {
            Self::Direct(v) => v.kernels(),
            Self::Erased(v) => v.kernels(),
        }
    }
    fn limits(&self) -> Limits {
        match self {
            Self::Direct(v) => v.limits(),
            Self::Erased(v) => v.limits(),
        }
    }
    fn replay(&self, budget: &mut AssertOriginBudgetV1<'_>) -> Result<(), Error> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
    }
}
fn forwarded(shared: bool, profile: Profile, bound: Option<u32>) -> (FinalFixture, usize) {
    macro_rules! route {
        ($fixture:ident, $variant:ident) => {{
            let source = literal_source(shared, bound);
            let (preheaders, inherited) =
                $fixture::prefix_from_source(source, profile, bound != Some(0));
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(inherited).unwrap();
            let (licm, r) = preheaders.continue_licm_v1(&mut budget).unwrap();
            budget.reserve_storage(r.retained_storage()).unwrap();
            let (refined, r) = licm
                .continue_induction_refinement_v1(Default::default(), &mut budget)
                .unwrap();
            budget.reserve_storage(r.retained_storage()).unwrap();
            let (owner, r) = refined
                .continue_cross_block_forwarding_v1(Default::default(), &mut budget)
                .unwrap();
            budget.reserve_storage(r.retained_storage()).unwrap();
            if bound.is_some_and(|n| n > 0) {
                assert_eq!(
                    owner
                        .refinement_origins()
                        .iter()
                        .filter(|r| matches!(
                            r.canonical_origin(),
                            RefineOrigin::CheckedAddSplit { .. }
                        ))
                        .count(),
                    if shared { 2 } else { 1 }
                );
                assert_eq!(
                    owner
                        .origins()
                        .iter()
                        .filter(|r| r.canonical_origin().store.is_some())
                        .count(),
                    if shared { 2 } else { 1 }
                );
            }
            owner.verify_equivalence(&mut budget).unwrap();
            (FinalFixture::$variant(owner), budget.storage())
        }};
    }
    if shared {
        route!(erased, Erased)
    } else {
        route!(direct, Direct)
    }
}
fn assert_source_join(owner: &UnrolledFixture, selected: Option<u8>) {
    let canonical = owner.tail().origins();
    assert_eq!(canonical.selection.map(|s| s.iterations), selected);
    assert_eq!(owner.origins().len(), canonical.operations.len());
    let previous = match owner {
        UnrolledFixture::Direct(v) => v.prefix().origins(),
        UnrolledFixture::Erased(v) => v.prefix().origins(),
    };
    let mut destinations = std::collections::BTreeSet::new();
    for (source, row) in owner.origins().iter().zip(canonical.operations) {
        assert_eq!(source.canonical_origin(), *row);
        let mut candidates = previous
            .iter()
            .filter(|r| r.canonical_origin().output == row.input);
        let old = candidates.next().unwrap();
        assert!(candidates.next().is_none());
        assert_eq!(
            source.original_source_statement(),
            old.original_source_statement()
        );
        match row.output {
            Some(at) => {
                assert_ne!(row.copy, CopyRole::OmittedBody);
                assert!(destinations.insert(at));
            }
            None => {
                assert_eq!(selected, Some(0));
                assert_eq!(row.copy, CopyRole::OmittedBody);
            }
        }
    }
    let expected: usize = owner
        .output()
        .module()
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .map(|b| b.blocks.iter().map(|b| b.operations.len()).sum::<usize>())
        .sum();
    assert_eq!(destinations.len(), expected);
    assert_eq!(
        owner.input().module().kernels,
        owner.output().module().kernels
    );
    assert_eq!(owner.kernels().len(), owner.output().module().kernels.len());
    if selected.is_some() {
        assert_ne!(
            owner.input().canonical().identity(),
            owner.output().canonical().identity()
        );
    } else {
        assert_eq!(
            owner.input().canonical().canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
    }
}
fn assert_no_loops(graph: &Graph, budget: &mut AssertOriginBudgetV1<'_>) {
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
fn with_unrolled(
    shared: bool,
    profile: Profile,
    bound: Option<u32>,
    run: impl FnOnce(&UnrolledFixture, &mut AssertOriginBudgetV1<'_>),
) {
    let (input, inherited) = forwarded(shared, profile, bound);
    let sibling = vec![0x6du8; 31];
    let floor = inherited + std::mem::size_of_val(&sibling) + sibling.capacity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let input_pointer = input.output().canonical().canonical_bytes().as_ptr();
    if bound == Some(0) {
        assert_no_loops(input.output(), &mut budget);
    }
    let (owner, receipt) = input.unroll(Limits::default(), &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(
        owner.input().canonical().canonical_bytes().as_ptr(),
        input_pointer
    );
    assert_source_join(
        &owner,
        bound
            .and_then(|n| u8::try_from(n).ok())
            .filter(|n| *n > 0 && *n <= 8),
    );
    owner.replay(&mut budget).unwrap();
    run(&owner, &mut budget);
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(sibling, [0x6d; 31]);
}

#[test]
fn source_unroll_direct_and_unit_local_keep_actual_dual_rewrite_then_clone() {
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for n in [1, 3, 8] {
                with_unrolled(shared, profile, Some(n), |owner, _| {
                    assert!(
                        owner
                            .tail()
                            .origins()
                            .operations
                            .iter()
                            .any(|r| matches!(r.copy, CopyRole::Body(0)))
                    );
                    assert!(
                        owner
                            .tail()
                            .origins()
                            .operations
                            .iter()
                            .any(|r| matches!(r.copy, CopyRole::Header(k) if k == n as u8))
                    );
                });
            }
        }
    }
}
#[test]
fn source_unroll_zero_trip_prefix_erasure_remains_a_checked_noop() {
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_unrolled(shared, profile, Some(0), |owner, _| {
                assert_eq!(owner.tail().origins().selection, None);
                assert!(!std::ptr::eq(owner.input(), owner.output()));
                assert_eq!(
                    owner.input().canonical().canonical_bytes(),
                    owner.output().canonical().canonical_bytes()
                );
                assert!(
                    owner
                        .tail()
                        .origins()
                        .operations
                        .iter()
                        .all(|r| r.copy == CopyRole::Retained && r.output == Some(r.input))
                );
            });
        }
    }
}
#[test]
fn source_unroll_dynamic_and_over_ceiling_are_real_exact_noops() {
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for bound in [None, Some(9)] {
                with_unrolled(shared, profile, bound, |owner, _| {
                    assert!(
                        owner
                            .tail()
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
fn source_unroll_preserves_other_unit_local_root_and_complete_reports() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_unrolled(true, profile, Some(3), |owner, _| {
            assert_eq!(owner.kernels().len(), 2);
            let selected_function = owner
                .tail()
                .origins()
                .blocks
                .iter()
                .find(|r| matches!(r.copy, CopyRole::Header(_)))
                .unwrap()
                .input
                .function;
            assert!(
                owner
                    .tail()
                    .origins()
                    .blocks
                    .iter()
                    .any(|r| r.input.function != selected_function)
            );
            assert!(
                owner
                    .tail()
                    .origins()
                    .blocks
                    .iter()
                    .filter(|r| r.input.function != selected_function)
                    .all(|r| r.copy == CopyRole::Retained)
            );
        });
    }
}

#[test]
fn source_unroll_replay_rejects_changed_origins_limits_and_final_reports() {
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let (owner, floor) = forwarded(shared, profile, Some(3));
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let (mut owner, receipt) = owner.unroll(Limits::default(), &mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            match &mut owner {
                UnrolledFixture::Direct(v) => v.exercise_loop_unroll_hostile_v1(&mut budget),
                UnrolledFixture::Erased(v) => v.exercise_loop_unroll_hostile_v1(&mut budget),
            }
            drop(owner);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}
