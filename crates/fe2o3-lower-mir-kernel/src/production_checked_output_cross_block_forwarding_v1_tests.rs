// Reuses the genuine semantic source and every original admission stage.
use super::*;
use crate::{
    ProductionCheckedOutputAdmissionErrorPolicy6V1 as Policy6Error,
    ProductionCommutativeContinuationErrorV1 as CommutativeError,
    ProductionCrossBlockForwardingErrorV1 as ForwardError,
    ProductionCrossBlockForwardingOriginV1 as ForwardOrigin,
    ProductionOwnedCrossBlockForwardingContinuationV1 as ForwardOwner,
    ProductionOwnedLicmContinuationV1 as LicmOwner,
    ProductionPrivateCellPromotionContinuationErrorV1 as PromotionError,
    ProductionRedundantStoreAdmissionErrorV1 as RedundantStoreError,
};
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingLimitsV1 as ForwardLimits, CanonicalKirInventoryV1 as Inventory,
};
use fe2o3_kernel_ir::{BinaryOp, CanonicalKirOperationCoordinateV1 as Coordinate};
use fe2o3_kernel_opt::{
    OwnedCrossBlockForwardingV1 as ForwardTail, prepare_owned_cross_block_forwarding_v1,
};

fn operation(
    owner: &VerifiedCanonicalKernelIrModuleV12,
    site: Coordinate,
) -> &fe2o3_kernel_ir::Operation {
    &owner.module().functions[site.block.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[site.block.block as usize]
        .operations[site.operation as usize]
}
fn actual(
    before: &VerifiedCanonicalKernelIrModuleV12,
    after: &VerifiedCanonicalKernelIrModuleV12,
    rows: &[ForwardOrigin],
    selected: usize,
) {
    let count = |owner: &VerifiedCanonicalKernelIrModuleV12| {
        owner
            .module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .map(|block| block.operations.len())
            .sum::<usize>()
    };
    assert_eq!(rows.len(), count(before));
    assert_eq!(count(before), count(after));
    assert_eq!(before.module().kernels, after.module().kernels);
    let mut loads = 0;
    for row in rows {
        let canonical = row.canonical_origin();
        assert_eq!(canonical.input, canonical.output);
        let original = operation(before, canonical.input);
        let final_op = operation(after, canonical.output);
        if let Some(store) = canonical.store {
            loads += 1;
            assert_ne!(store.block, canonical.input.block);
            assert!(matches!(original.kind, OperationKind::Load { .. }));
            let OperationKind::Store { value, .. } = operation(before, store).kind else {
                panic!("actual source initializing Store")
            };
            assert_eq!(
                final_op.kind,
                OperationKind::Binary {
                    op: BinaryOp::BitOr,
                    lhs: value,
                    rhs: value
                }
            );
            assert_eq!(final_op.results, original.results);
            let (function, block, statement) = row
                .original_source_statement()
                .expect("selected Load has its own source statement");
            assert!(matches!(function.index(), 0 | 1 | 3));
            assert_eq!(
                (block.index(), statement),
                (4, 0),
                "original Load, never earlier Store statement (3,2)"
            );
        } else {
            assert_eq!(original, final_op);
        }
    }
    assert_eq!(
        loads, selected,
        "nonzero cross-block Load must survive genuine prefix and promotion"
    );
    assert!(!std::ptr::eq(before, after));
    assert_eq!(
        before.canonical().canonical_bytes() == after.canonical().canonical_bytes(),
        selected == 0
    );
}

#[test]
fn source_cross_block_forwarding_direct_keeps_actual_load_statement_and_fresh_reports() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let (preheaders, inherited) = direct::prefix(profile, mutation);
            let sibling = vec![0x6au8; 47];
            let sibling_bytes = std::mem::size_of_val(&sibling) + sibling.capacity();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(inherited + sibling_bytes).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let (licm, receipt) = preheaders.continue_licm_v1(&mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let floor = budget.storage();
            let pointer = licm.output().canonical().canonical_bytes().as_ptr();
            let (mut owner, receipt) = licm
                .continue_cross_block_forwarding_v1(ForwardLimits::default(), &mut budget)
                .unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(
                receipt.retained_storage(),
                owner.additional_retained_storage_v1()
            );
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(
                owner
                    .prefix()
                    .output()
                    .canonical()
                    .canonical_bytes()
                    .as_ptr(),
                pointer
            );
            actual(
                owner.prefix().output(),
                owner.output(),
                owner.origins(),
                usize::from(mutation),
            );
            assert_eq!(owner.kernels().len(), 1);
            assert!(!owner.grants_artifact_or_launch_authority());
            owner.verify_equivalence(&mut budget).unwrap();
            owner.exercise_cross_block_forwarding_refusals_v1(&mut budget);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x6a; 47]);
            drop(owner);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}
#[test]
fn source_cross_block_forwarding_unit_local_retains_original_roots_erasure_and_two_loads() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let (preheaders, inherited) = erased::prefix(profile, mutation);
            let source = preheaders.prefix().prefix().prefix().prefix();
            assert_eq!(
                (
                    source.erased_source().deleted_call_count(),
                    source.erased_source().deleted_function_count()
                ),
                (3, 2)
            );
            let original = source
                .original_source()
                .executable()
                .canonical()
                .canonical_bytes()
                .as_ptr();
            let erased = source.erased().canonical().canonical_bytes().as_ptr();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(inherited).unwrap();
            let (licm, receipt) = preheaders.continue_licm_v1(&mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let floor = budget.storage();
            let (mut owner, receipt) = licm
                .continue_cross_block_forwarding_v1(ForwardLimits::default(), &mut budget)
                .unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let source = owner.prefix().prefix().prefix().prefix().prefix().prefix();
            assert_eq!(
                source
                    .original_source()
                    .executable()
                    .canonical()
                    .canonical_bytes()
                    .as_ptr(),
                original
            );
            assert_eq!(
                source.erased().canonical().canonical_bytes().as_ptr(),
                erased
            );
            assert_eq!(
                source
                    .original_source()
                    .semantic_ssa()
                    .source_semantic()
                    .functions()
                    .len(),
                4
            );
            assert_eq!(
                (
                    source.erased_source().deleted_call_count(),
                    source.erased_source().deleted_function_count()
                ),
                (3, 2)
            );
            actual(
                owner.prefix().output(),
                owner.output(),
                owner.origins(),
                if mutation { 2 } else { 0 },
            );
            assert_eq!(owner.kernels().len(), 2);
            owner.verify_equivalence(&mut budget).unwrap();
            owner.exercise_cross_block_forwarding_refusals_v1(&mut budget);
            drop(owner);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}
fn measured(
    profile: Profile,
    mutation: bool,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ForwardError>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let (preheaders, inherited) = direct::prefix(profile, mutation);
    let (licm, floor) = {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (owner, receipt) = preheaders.continue_licm_v1(&mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        (owner, budget.storage())
    };
    let sibling = vec![0x39u8; 53];
    let floor = floor + std::mem::size_of_val(&sibling) + sibling.capacity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    work.charge_work(17).unwrap();
    let (result, used, peak, failed_storage) = {
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = licm
            .continue_cross_block_forwarding_v1(ForwardLimits::default(), &mut budget)
            .map(|(owner, receipt)| {
                assert_eq!(
                    owner.additional_retained_storage_v1(),
                    receipt.retained_storage()
                );
                assert_eq!(
                    owner
                        .origins()
                        .iter()
                        .filter(|row| row.canonical_origin().store.is_some())
                        .count(),
                    usize::from(mutation)
                );
                drop(owner);
            });
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x39; 53]);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    (result, used, peak, work.failed_work(), failed_storage)
}
#[test]
fn source_cross_block_forwarding_exact_budget_and_typed_work_short_preserve_consumed_floor() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let full = measured(profile, mutation, WORK, STORAGE);
            full.0.unwrap();
            let exact = measured(profile, mutation, full.1, full.2);
            exact.0.unwrap();
            assert_eq!(
                (exact.1, exact.2, exact.3, exact.4),
                (full.1, full.2, None, None)
            );
            let short = measured(profile, mutation, full.1 - 1, full.2);
            match short.0 {
                Err(ForwardError::Resource(AssertOriginResourceV1::Work(error))) => {
                    assert_eq!((error.actual(), error.limit()), (full.1, full.1 - 1))
                }
                other => panic!("exact last source owning work charge: {other:?}"),
            }
            assert_eq!(
                (short.1, short.2, short.3, short.4),
                (full.1 - 1, full.2, Some(full.1), None)
            );
        }
    }
}
#[test]
fn source_cross_block_forwarding_storage_first_denial_needs_strict_observed_phase() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let full = measured(profile, mutation, WORK, STORAGE);
            full.0.unwrap();
            assert_eq!((full.3, full.4), (None, None));
            let reference = measured_final_source_prefix(profile, mutation, STORAGE);
            reference.0.unwrap();
            assert_eq!(
                (reference.2, reference.3, reference.4),
                (full.2, None, None)
            );
            let reference = measured_final_source_prefix(profile, mutation, full.2 - 1);
            let Err(error) = &reference.0 else {
                panic!("exact independently replayed P8 refusal: {reference:?}")
            };
            assert_final_source_prefix_storage(error, full.2);
            assert_eq!((reference.3, reference.4), (None, Some(full.2)));
            let short = measured(profile, mutation, WORK, full.2 - 1);
            let Err(ForwardError::Admission(promotion)) = &short.0 else {
                panic!("exact forwarding final-source admission refusal: {short:?}")
            };
            let PromotionError::Prefix(error) = promotion.as_ref() else {
                panic!("exact promoted source-prefix refusal: {promotion:?}")
            };
            assert_final_source_prefix_storage(error, full.2);
            assert_eq!((short.3, short.4), (None, Some(full.2)));
            assert_eq!((short.1, short.2), (reference.1, reference.2));
        }
    }
}

fn assert_final_source_prefix_storage(error: &CommutativeError, peak: usize) {
    let CommutativeError::Prefix(redundant) = error else {
        panic!("exact commutative source-prefix refusal: {error:?}")
    };
    let RedundantStoreError::Prefix(policy6) = redundant.as_ref() else {
        panic!("exact redundant-store source-prefix refusal: {redundant:?}")
    };
    let Policy6Error::Optimization(fe2o3_kernel_opt::CanonicalPolicy6OptimizationErrorV1::Map(
        fe2o3_pliron::KirOptimizationMapErrorV12::Resources(AssertOriginResourceV1::Storage(limit)),
    )) = policy6.as_ref()
    else {
        panic!("exact checked-map scratch Storage refusal: {policy6:?}")
    };
    assert_eq!((limit.actual(), limit.limit()), (peak, peak - 1));
}

// Independently assemble the live public receipts preceding the final source
// replay. No source-forwarding entry, metadata callback or private scope is used.
fn measured_final_source_prefix(
    profile: Profile,
    mutation: bool,
    storage_limit: usize,
) -> (
    Result<(), CommutativeError>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let (preheaders, inherited) = direct::prefix(profile, mutation);
    let (licm, floor) = {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (owner, receipt) = preheaders.continue_licm_v1(&mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        (owner, budget.storage())
    };
    let sibling = vec![0x39u8; 53];
    let floor = floor + std::mem::size_of_val(&sibling) + sibling.capacity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    work.charge_work(17).unwrap();
    let (result, used, peak, failed_storage) = {
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = {
            licm.verify_equivalence(&mut budget).unwrap();
            let header = std::mem::size_of::<ForwardOwner>()
                .checked_sub(std::mem::size_of::<LicmOwner>())
                .unwrap()
                .checked_sub(std::mem::size_of::<ForwardTail>())
                .unwrap();
            budget.reserve_storage(header).unwrap();
            let tail = prepare_owned_cross_block_forwarding_v1(
                licm.output(),
                ForwardLimits::default(),
                &mut budget,
            )
            .unwrap();
            budget.reserve_storage(tail.retained_storage()).unwrap();
            budget.charge_work(4).unwrap();
            let requested = tail
                .origins()
                .len()
                .checked_mul(std::mem::size_of::<ForwardOrigin>())
                .unwrap();
            budget.reserve_storage(requested).unwrap();
            let mut origins = Vec::<ForwardOrigin>::new();
            origins.try_reserve_exact(tail.origins().len()).unwrap();
            let actual = origins
                .capacity()
                .checked_mul(std::mem::size_of::<ForwardOrigin>())
                .unwrap();
            budget
                .reserve_storage(actual.checked_sub(requested).unwrap())
                .unwrap();
            budget.charge_work(7).unwrap();
            let (forward_pair, receipt) = tail.replay_against(licm.output(), &mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let (final_inventory, receipt) = Inventory::derive(tail.output(), &mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let preheaders = licm.prefix();
            let promoted = preheaders.prefix();
            let (licm_pair, receipt) = licm
                .continuation()
                .replay_against(preheaders.output(), &mut budget)
                .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let (preheader_pair, receipt) = preheaders
                .continuation()
                .replay_against(promoted.output(), &mut budget)
                .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let (input_inventory, receipt) =
                Inventory::derive(preheaders.output(), &mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let (output_inventory, receipt) =
                Inventory::derive(licm.output(), &mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            // with_promoted_output_sites charges three before the actual P8 replay.
            budget.charge_work(3).unwrap();
            let result = promoted.prefix().verify_equivalence(&mut budget);
            drop((
                output_inventory,
                input_inventory,
                preheader_pair,
                licm_pair,
                final_inventory,
                forward_pair,
            ));
            drop((origins, tail));
            result
        };
        // All reference backing has dropped; preserve the genuine source/sibling floor.
        budget
            .release_storage(budget.storage().checked_sub(floor).unwrap())
            .unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x39; 53]);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    (result, used, peak, work.failed_work(), failed_storage)
}
