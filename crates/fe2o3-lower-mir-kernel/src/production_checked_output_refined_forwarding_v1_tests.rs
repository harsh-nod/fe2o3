//! Genuine source fixtures retain exact no-op controls and guarded dual rewrites.
use super::*;
use crate::{
    ProductionCheckedOutputAdmissionErrorPolicy6V1 as Policy6Error,
    ProductionCommutativeContinuationErrorV1 as CommutativeError,
    ProductionCrossBlockForwardingOriginV1 as FinalOrigin,
    ProductionInductionRefinementOriginV1 as FirstOrigin,
    ProductionOwnedRefinedCrossBlockForwardingContinuationV1 as DirectComposed,
    ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1 as ErasedComposed,
    ProductionPrivateCellPromotionContinuationErrorV1 as PromotionError,
    ProductionRedundantStoreAdmissionErrorV1 as RedundantStoreError,
    ProductionRefinedCrossBlockForwardingErrorV1 as Error,
};
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingLimitsV1 as ForwardLimits,
    CanonicalKirInductionRefinementOriginV1 as Origin, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirLoopLimitsV1 as RefineLimits,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKirBlockCoordinateV1, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirFunctionCoordinateV1, CanonicalKirOperationCoordinateV1 as Coordinate,
    CheckedBinaryOperator, Constant, ScalarType, Type,
};
use fe2o3_kernel_opt::{
    OwnedCrossBlockForwardingV1 as ForwardTail, prepare_owned_cross_block_forwarding_v1,
};

#[path = "production_checked_output_canonical_formal_adapter_v1_tests.rs"]
mod canonical_output_formal_adapter_tests;

fn operation(
    owner: &VerifiedCanonicalKernelIrModuleV12,
    coordinate: Coordinate,
) -> &fe2o3_kernel_ir::Operation {
    &owner.module().functions[coordinate.block.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[coordinate.block.block as usize]
        .operations[coordinate.operation as usize]
}
fn coordinates(owner: &VerifiedCanonicalKernelIrModuleV12) -> Vec<Coordinate> {
    let mut sites = Vec::new();
    for (function, declaration) in owner.module().functions.iter().enumerate() {
        if let Some(body) = &declaration.body {
            for (block, value) in body.blocks.iter().enumerate() {
                for operation in 0..value.operations.len() {
                    sites.push(Coordinate {
                        block: CanonicalKirBlockCoordinateV1 {
                            function: CanonicalKirFunctionCoordinateV1(
                                function.try_into().unwrap(),
                            ),
                            block: block.try_into().unwrap(),
                        },
                        operation: operation.try_into().unwrap(),
                    });
                }
            }
        }
    }
    sites
}
fn actual(
    l: &VerifiedCanonicalKernelIrModuleV12,
    r: &VerifiedCanonicalKernelIrModuleV12,
    f: &VerifiedCanonicalKernelIrModuleV12,
    first: &[FirstOrigin],
    last: &[FinalOrigin],
    expected_splits: usize,
    selected: usize,
) {
    assert_eq!(
        l.canonical().canonical_bytes() == r.canonical().canonical_bytes(),
        expected_splits == 0
    );
    let input_sites = coordinates(l);
    let intermediate_sites = coordinates(r);
    assert_eq!(first.len(), input_sites.len());
    assert_eq!(last.len(), intermediate_sites.len());
    assert_eq!(
        intermediate_sites.len(),
        input_sites.len() + expected_splits
    );
    assert_eq!(coordinates(f), intermediate_sites);
    assert_eq!(
        first
            .iter()
            .map(|row| row.canonical_origin().input())
            .collect::<Vec<_>>(),
        input_sites
    );
    assert!(!std::ptr::eq(l, r));
    assert!(!std::ptr::eq(r, f));
    assert_eq!(l.module().kernels, r.module().kernels);
    assert_eq!(r.module().kernels, f.module().kernels);
    assert_eq!(l.module().functions.len(), r.module().functions.len());
    assert_eq!(r.module().functions.len(), f.module().functions.len());
    for ((before, middle), after) in l
        .module()
        .functions
        .iter()
        .zip(&r.module().functions)
        .zip(&f.module().functions)
    {
        match (&before.body, &middle.body, &after.body) {
            (Some(before), Some(middle), Some(after)) => {
                assert_eq!(before.blocks.len(), middle.blocks.len());
                assert_eq!(middle.blocks.len(), after.blocks.len());
                for ((before, middle), after) in
                    before.blocks.iter().zip(&middle.blocks).zip(&after.blocks)
                {
                    assert_eq!(
                        (before.id, &before.parameters, &before.terminator),
                        (middle.id, &middle.parameters, &middle.terminator)
                    );
                    assert_eq!(
                        (middle.id, &middle.parameters, &middle.terminator),
                        (after.id, &after.parameters, &after.terminator)
                    );
                }
            }
            (None, None, None) => {}
            _ => panic!("the actual function body roster must remain unchanged"),
        }
    }
    let mut expanded = Vec::new();
    let mut split_roots = Vec::new();
    for source in first {
        assert_eq!(source.synthetic_false_source_statement(), None);
        match source.canonical_origin() {
            Origin::Unchanged { input, output } => {
                assert_eq!(operation(l, input), operation(r, output));
                assert_eq!(source.synthetic_overflow_definition(), None);
                expanded.push((output, source.original_source_statement()));
            }
            Origin::CheckedAddSplit {
                input,
                sum_output,
                false_output,
                ..
            } => {
                let old = operation(l, input);
                let OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    lhs,
                    rhs,
                } = old.kind
                else {
                    panic!("actual source checked U32 loop update")
                };
                assert_eq!(old.results.len(), 2);
                assert_eq!(old.results[0].ty, Type::Scalar(ScalarType::U32));
                assert_eq!(old.results[1].ty, Type::BOOL);
                assert_eq!(sum_output.block, input.block);
                assert_eq!(false_output.block, sum_output.block);
                assert_eq!(false_output.operation, sum_output.operation + 1);
                let sum = operation(r, sum_output);
                let flag = operation(r, false_output);
                assert_eq!(
                    sum.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs,
                        rhs
                    }
                );
                assert_eq!(sum.results.as_slice(), &old.results[..1]);
                assert_eq!(flag.kind, OperationKind::Constant(Constant::Bool(false)));
                assert_eq!(flag.results.as_slice(), &old.results[1..]);
                assert_eq!(
                    source.synthetic_overflow_definition(),
                    Some(Definition::Result {
                        operation: input,
                        result: 1
                    })
                );
                let (root, block, statement) = source.original_source_statement().unwrap();
                assert_eq!((block.index(), statement), (4, 1));
                split_roots.push(root.index());
                expanded.push((sum_output, source.original_source_statement()));
                expanded.push((false_output, None));
            }
        }
    }
    split_roots.sort_unstable();
    assert_eq!(split_roots.len(), expected_splits);
    assert_eq!(
        split_roots,
        match expected_splits {
            0 => vec![],
            1 => vec![0],
            2 => vec![1, 3],
            _ => panic!("fixed source root roster"),
        }
    );
    assert_eq!(
        expanded.iter().map(|(site, _)| *site).collect::<Vec<_>>(),
        intermediate_sites
    );
    let mut loads = 0;
    for ((site, original_source), last) in expanded.iter().zip(last) {
        let row = last.canonical_origin();
        assert_eq!((*site, *site), (row.input, row.output));
        assert_eq!(*original_source, last.original_source_statement());
        let before = operation(r, row.input);
        let after = operation(f, row.output);
        if let Some(store) = row.store {
            loads += 1;
            assert_ne!(store.block, row.input.block);
            assert!(matches!(before.kind, OperationKind::Load { .. }));
            let OperationKind::Store { value, .. } = operation(r, store).kind else {
                panic!("actual intermediate Store")
            };
            assert_eq!(after.results, before.results);
            assert_eq!(
                after.kind,
                OperationKind::Binary {
                    op: BinaryOp::BitOr,
                    lhs: value,
                    rhs: value
                }
            );
            let (_, block, statement) = last.original_source_statement().unwrap();
            assert_eq!((block.index(), statement), (4, 0));
        } else {
            assert_eq!(before, after);
        }
    }
    assert_eq!(loads, selected);
    assert_eq!(
        r.canonical().canonical_bytes() == f.canonical().canonical_bytes(),
        selected == 0
    );
}

macro_rules! source_case {
    ($fixture:ident, $profile:expr, $mutation:expr, $roots:expr, $selected:expr) => {{
        let (preheaders, inherited) = $fixture::prefix($profile, $mutation);
        let sibling = vec![0x51u8; 31];
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget
            .reserve_storage(inherited + std::mem::size_of_val(&sibling) + sibling.capacity())
            .unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let (licm, receipt) = preheaders.continue_licm_v1(&mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (refined, receipt) = licm
            .continue_induction_refinement_v1(RefineLimits::default(), &mut budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        refined.exercise_refined_forwarding_callbacks_v1(&mut budget);
        let pointer = refined.output().canonical().canonical_bytes().as_ptr();
        let floor = budget.storage();
        let (mut value, receipt) = refined
            .continue_cross_block_forwarding_v1(ForwardLimits::default(), &mut budget)
            .unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            receipt.retained_storage(),
            value.additional_retained_storage_v1()
        );
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(
            value
                .prefix()
                .output()
                .canonical()
                .canonical_bytes()
                .as_ptr(),
            pointer
        );
        actual(
            value.prefix().prefix().output(),
            value.prefix().output(),
            value.output(),
            value.refinement_origins(),
            value.origins(),
            if $mutation { $roots } else { 0 },
            $selected,
        );
        assert_eq!(value.kernels().len(), $roots);
        assert!(!value.grants_artifact_or_launch_authority());
        value.verify_equivalence(&mut budget).unwrap();
        value.exercise_refined_forwarding_hostile_v1(&mut budget);
        assert_eq!(sibling, [0x51; 31]);
        assert!(budget.work_ledger_identity_v1() == ledger);
        drop(value);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }};
}
#[test]
fn refined_forwarding_source_direct_keeps_no_update_control_and_real_forwarding_lineage() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            source_case!(direct, profile, mutation, 1, usize::from(mutation));
        }
    }
}
#[test]
fn refined_forwarding_source_unit_local_keeps_no_update_control_and_all_root_reports() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            source_case!(erased, profile, mutation, 2, if mutation { 2 } else { 0 });
        }
    }
}
fn measured(
    erased: bool,
    profile: Profile,
    mutation: bool,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), Error>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    macro_rules! route {
        ($fixture:ident) => {{
            let (preheaders, inherited) = $fixture::prefix(profile, mutation);
            let (refined, floor) = {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                budget.reserve_storage(inherited).unwrap();
                let (licm, receipt) = preheaders.continue_licm_v1(&mut budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let (refined, receipt) = licm
                    .continue_induction_refinement_v1(RefineLimits::default(), &mut budget)
                    .unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                (refined, budget.storage())
            };
            let sibling = vec![0x52u8; 53];
            let floor = floor + std::mem::size_of_val(&sibling) + sibling.capacity();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            work.charge_work(17).unwrap();
            let (result, used, peak, failed_storage) = {
                let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let result = refined
                    .continue_cross_block_forwarding_v1(ForwardLimits::default(), &mut budget)
                    .map(|(owner, receipt)| {
                        assert_eq!(
                            owner.additional_retained_storage_v1(),
                            receipt.retained_storage()
                        );
                        actual(
                            owner.prefix().prefix().output(),
                            owner.prefix().output(),
                            owner.output(),
                            owner.refinement_origins(),
                            owner.origins(),
                            if mutation {
                                if erased { 2 } else { 1 }
                            } else {
                                0
                            },
                            if mutation {
                                if erased { 2 } else { 1 }
                            } else {
                                0
                            },
                        );
                        drop(owner);
                    });
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(sibling, [0x52; 53]);
                (
                    result,
                    budget.work(),
                    budget.peak_storage(),
                    budget.failed_storage(),
                )
            };
            (result, used, peak, work.failed_work(), failed_storage)
        }};
    }
    if erased {
        route!(erased)
    } else {
        route!(direct)
    }
}
#[test]
fn refined_forwarding_source_exact_work_peak_and_last_work_denial_keep_consumed_floor() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for mutation in [false, true] {
                let full = measured(erased, profile, mutation, WORK, STORAGE);
                full.0.unwrap();
                let exact = measured(erased, profile, mutation, full.1, full.2);
                exact.0.unwrap();
                assert_eq!(
                    (exact.1, exact.2, exact.3, exact.4),
                    (full.1, full.2, None, None)
                );
                let short = measured(erased, profile, mutation, full.1 - 1, full.2);
                match short.0 {
                    Err(Error::Resource(AssertOriginResourceV1::Work(error))) => {
                        assert_eq!((error.actual(), error.limit()), (full.1, full.1 - 1))
                    }
                    other => panic!("exact final source owning charge: {other:?}"),
                }
                assert_eq!(
                    (short.1, short.2, short.3, short.4),
                    (full.1 - 1, full.2, Some(full.1), None)
                );
            }
        }
    }
}
#[test]
fn refined_forwarding_source_storage_observation_requires_strict_phase_successor() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for mutation in [false, true] {
                let full = measured(erased, profile, mutation, WORK, STORAGE);
                full.0.unwrap();
                assert_eq!((full.3, full.4), (None, None));
                let reference = measured_final_source_prefix(erased, profile, mutation, STORAGE);
                reference.0.unwrap();
                assert_eq!(
                    (reference.2, reference.3, reference.4),
                    (full.2, None, None)
                );
                let reference = measured_final_source_prefix(erased, profile, mutation, full.2 - 1);
                let Err(error) = &reference.0 else {
                    panic!("exact independently replayed P8 refusal: {reference:?}")
                };
                assert_final_source_prefix_storage(error, full.2);
                assert_eq!((reference.3, reference.4), (None, Some(full.2)));
                let short = measured(erased, profile, mutation, WORK, full.2 - 1);
                let Err(Error::Admission(promotion)) = &short.0 else {
                    panic!("exact composed final-source admission refusal: {short:?}")
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

fn reference_rows<T>(count: usize, budget: &mut AssertOriginBudgetV1<'_>) -> Vec<T> {
    budget.charge_work(4).unwrap();
    let requested = count.checked_mul(std::mem::size_of::<T>()).unwrap();
    budget.reserve_storage(requested).unwrap();
    let mut rows = Vec::new();
    rows.try_reserve_exact(count).unwrap();
    let actual = rows
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .unwrap();
    budget
        .reserve_storage(actual.checked_sub(requested).unwrap())
        .unwrap();
    rows
}

// Reconstruct only the paid public receipt frontier before final P8 replay.
// Both genuine rewrite pairs and both actual origin allocations remain live.
fn measured_final_source_prefix(
    erased: bool,
    profile: Profile,
    mutation: bool,
    storage_limit: usize,
) -> (
    std::result::Result<(), CommutativeError>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    macro_rules! route {
        ($fixture:ident, $owner:ty) => {{
            let (preheaders, inherited) = $fixture::prefix(profile, mutation);
            let (refined, floor) = {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                budget.reserve_storage(inherited).unwrap();
                let (licm, receipt) = preheaders.continue_licm_v1(&mut budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let (refined, receipt) = licm
                    .continue_induction_refinement_v1(RefineLimits::default(), &mut budget)
                    .unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                (refined, budget.storage())
            };
            let sibling = vec![0x52u8; 53];
            let floor = floor + std::mem::size_of_val(&sibling) + sibling.capacity();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            work.charge_work(17).unwrap();
            let (result, used, peak, failed_storage) = {
                let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let result = {
                    refined.verify_equivalence(&mut budget).unwrap();
                    let header = std::mem::size_of::<$owner>()
                        .checked_sub(std::mem::size_of_val(&refined))
                        .and_then(|bytes| bytes.checked_sub(std::mem::size_of::<ForwardTail>()))
                        .unwrap();
                    budget.reserve_storage(header).unwrap();
                    let tail = prepare_owned_cross_block_forwarding_v1(
                        refined.output(),
                        ForwardLimits::default(),
                        &mut budget,
                    )
                    .unwrap();
                    budget.reserve_storage(tail.retained_storage()).unwrap();
                    let origins = reference_rows::<FinalOrigin>(tail.origins().len(), &mut budget);
                    budget.charge_work(7).unwrap();
                    let (forward_pair, receipt) =
                        tail.replay_against(refined.output(), &mut budget).unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    let (final_inventory, receipt) =
                        Inventory::derive(tail.output(), &mut budget).unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    budget
                        .reserve_storage(std::mem::size_of::<Vec<FirstOrigin>>())
                        .unwrap();
                    let intermediate_origins =
                        reference_rows::<FirstOrigin>(refined.origins().len(), &mut budget);
                    budget.charge_work(7).unwrap();
                    let licm = refined.prefix();
                    let (refinement_pair, receipt) = refined
                        .continuation()
                        .replay_against(licm.output(), refined.limits(), &mut budget)
                        .unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    let (refined_inventory, receipt) =
                        Inventory::derive(refined.output(), &mut budget).unwrap();
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
                    budget.charge_work(3).unwrap();
                    let result = promoted.prefix().verify_equivalence(&mut budget);
                    drop((
                        output_inventory,
                        input_inventory,
                        preheader_pair,
                        licm_pair,
                        refined_inventory,
                        refinement_pair,
                        final_inventory,
                        forward_pair,
                    ));
                    drop((intermediate_origins, origins, tail));
                    result
                };
                budget
                    .release_storage(budget.storage().checked_sub(floor).unwrap())
                    .unwrap();
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(sibling, [0x52; 53]);
                (
                    result,
                    budget.work(),
                    budget.peak_storage(),
                    budget.failed_storage(),
                )
            };
            drop(refined);
            (result, used, peak, work.failed_work(), failed_storage)
        }};
    }
    if erased {
        route!(erased, ErasedComposed)
    } else {
        route!(direct, DirectComposed)
    }
}
