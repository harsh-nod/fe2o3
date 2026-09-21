use super::*;
use crate::{
    ProductionCheckedOutputAdmissionErrorPolicy6V1 as Policy6Error,
    ProductionCommutativeContinuationErrorV1 as CommutativeError,
    ProductionCrossBlockForwardingOriginV1 as ForwardOrigin,
    ProductionInductionRefinementOriginV1 as RefinedOrigin,
    ProductionLoopUnrollOriginV1 as UnrollOrigin,
    ProductionPrivateCellPromotionContinuationErrorV1 as PromotionError,
    ProductionRedundantStoreAdmissionErrorV1 as RedundantStoreError,
};
use fe2o3_kernel_analysis::CanonicalKirInventoryV1 as Inventory;
use fe2o3_kernel_opt::{OwnedLoopUnrollV1 as Tail, unroll_canonical_kir_loops_v1};
const PREFIX: usize = 17;
struct Observation {
    result: Result<(), Error>,
    floor: usize,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn measure(
    shared: bool,
    profile: Profile,
    bound: Option<u32>,
    replay: bool,
    work_limit: usize,
    storage_limit: usize,
) -> Observation {
    let (input, inherited) = forwarded(shared, profile, bound);
    let (mut input, mut ready, mut retained) = (Some(input), None, inherited);
    if replay {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (owner, receipt) = input
            .take()
            .unwrap()
            .unroll(Limits::default(), &mut budget)
            .unwrap();
        retained += receipt.retained_storage();
        ready = Some(owner);
    }
    let sibling = vec![0x79u8; 37];
    let floor = retained + std::mem::size_of_val(&sibling) + sibling.capacity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    work.charge_work(PREFIX).unwrap();
    let (result, accepted, peak, failed_storage) = {
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = if replay {
            ready.as_ref().unwrap().replay(&mut budget)
        } else {
            input
                .take()
                .unwrap()
                .unroll(Limits::default(), &mut budget)
                .map(|(owner, receipt)| {
                    // Getter-only inspection is not further controlled replay.
                    assert!(receipt.retained_storage() > 0);
                    assert_eq!(owner.limits(), Limits::default());
                    drop(owner);
                })
        };
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x79; 37]);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    drop(ready);
    Observation {
        result,
        floor,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}
#[test]
fn source_unroll_factory_and_replay_exact_work_peak_and_terminal_work_short() {
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for bound in [Some(3), None] {
                for replay in [false, true] {
                    let full = measure(shared, profile, bound, replay, WORK, STORAGE);
                    assert!(full.result.is_ok());
                    assert!(full.work > PREFIX && full.peak > full.floor);
                    let exact = measure(shared, profile, bound, replay, full.work, full.peak);
                    assert!(exact.result.is_ok());
                    assert_eq!(
                        (exact.work, exact.peak, exact.floor),
                        (full.work, full.peak, full.floor)
                    );
                    assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
                    let short = measure(shared, profile, bound, replay, full.work - 1, full.peak);
                    match short.result {
                        Err(Error::Resource(AssertOriginResourceV1::Work(error))) => {
                            assert_eq!((error.actual(), error.limit()), (full.work, full.work - 1));
                        }
                        other => panic!("exact outer final charge: {other:?}"),
                    }
                    assert_eq!(
                        (
                            short.work,
                            short.peak,
                            short.failed_work,
                            short.failed_storage
                        ),
                        (full.work - 1, full.peak, Some(full.work), None)
                    );
                }
            }
        }
    }
}
#[test]
fn source_unroll_storage_observations_require_strict_phase_successor() {
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for bound in [Some(3), None] {
                for replay in [false, true] {
                    let full = measure(shared, profile, bound, replay, WORK, STORAGE);
                    assert!(full.result.is_ok());
                    let short = measure(shared, profile, bound, replay, full.work, full.peak - 1);
                    let reference =
                        final_source_prefix(shared, profile, bound, replay, full.peak - 1);
                    let Err(Error::Admission(promotion)) = &short.result else {
                        panic!("exact U final-source admission refusal: {:?}", short.result)
                    };
                    let PromotionError::Prefix(prefix) = promotion.as_ref() else {
                        panic!("exact promoted source-prefix refusal: {promotion:?}")
                    };
                    assert_prefix_storage(prefix, full.peak);
                    let Err(prefix) = &reference.result else {
                        panic!("the independent public P8 replay must refuse Storage")
                    };
                    assert_prefix_storage(prefix, full.peak);
                    assert_eq!(short.floor, full.floor);
                    assert_eq!(
                        (short.work, short.peak, short.floor),
                        (reference.work, reference.peak, reference.floor)
                    );
                    assert_eq!(
                        (short.failed_work, short.failed_storage),
                        (None, Some(full.peak))
                    );
                    assert_eq!(
                        (reference.failed_work, reference.failed_storage),
                        (None, Some(full.peak))
                    );
                    eprintln!(
                        "SOURCE_UNROLL_STORAGE shared={shared} profile={profile:?} bound={bound:?} replay={replay} floor={} W={} P={} accepted={} prior_peak={} failed_storage={:?} error={:?}",
                        full.floor,
                        full.work,
                        full.peak,
                        short.work,
                        short.peak,
                        short.failed_storage,
                        short.result
                    );
                }
            }
        }
    }
}
fn assert_prefix_storage(error: &CommutativeError, peak: usize) {
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

struct PrefixObservation {
    result: Result<(), CommutativeError>,
    floor: usize,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}

// Reconstruct only the paid public frontier before final P8 source replay.
// All five checked pairs, five inventories and actual row capacities stay live.
fn final_source_prefix(
    shared: bool,
    profile: Profile,
    bound: Option<u32>,
    replay: bool,
    storage_limit: usize,
) -> PrefixObservation {
    let (input, inherited) = forwarded(shared, profile, bound);
    let (mut input, mut ready, mut retained) = (Some(input), None, inherited);
    if replay {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (owner, receipt) = input
            .take()
            .unwrap()
            .unroll(Limits::default(), &mut budget)
            .unwrap();
        retained = retained.checked_add(receipt.retained_storage()).unwrap();
        ready = Some(owner);
    }
    let sibling = vec![0x79u8; 37];
    let floor = retained
        .checked_add(std::mem::size_of_val(&sibling))
        .and_then(|n| n.checked_add(sibling.capacity()))
        .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    work.charge_work(PREFIX).unwrap();
    let (result, accepted, peak, failed_storage) = {
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        macro_rules! frontier {
            ($prefix:expr, $borrowed:expr, $owner:ty) => {{
                let prefix = $prefix;
                let borrowed: Option<&Tail> = $borrowed;
                let constructed = if let Some(tail) = borrowed {
                    budget.charge_work(7).unwrap();
                    assert_eq!(tail.limits(), Limits::default());
                    prefix.verify_equivalence(&mut budget).unwrap();
                    budget
                        .reserve_storage(std::mem::size_of::<Vec<UnrollOrigin>>())
                        .unwrap();
                    None
                } else {
                    budget.charge_work(3).unwrap();
                    prefix.verify_equivalence(&mut budget).unwrap();
                    let header = std::mem::size_of::<$owner>()
                        .checked_sub(std::mem::size_of_val(prefix))
                        .and_then(|n| n.checked_sub(std::mem::size_of::<Tail>()))
                        .unwrap();
                    budget.reserve_storage(header).unwrap();
                    let (tail, receipt) = unroll_canonical_kir_loops_v1(
                        prefix.output(),
                        Limits::default(),
                        &mut budget,
                    )
                    .unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    Some(tail)
                };
                let tail = borrowed.or(constructed.as_ref()).unwrap();
                let origins =
                    reference_rows::<UnrollOrigin>(tail.origins().operations.len(), &mut budget);
                budget.charge_work(7).unwrap();
                budget.charge_work(3).unwrap();
                let (unroll_pair, receipt) = tail
                    .replay(prefix.output(), Limits::default(), &mut budget)
                    .unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let (unrolled_inventory, receipt) =
                    Inventory::derive(tail.output(), &mut budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                budget
                    .reserve_storage(std::mem::size_of::<Vec<ForwardOrigin>>())
                    .unwrap();
                let forwarded_origins =
                    reference_rows::<ForwardOrigin>(prefix.origins().len(), &mut budget);
                budget.charge_work(7).unwrap();
                let refined = prefix.prefix();
                let (forwarding_pair, receipt) = prefix
                    .continuation()
                    .replay_against(refined.output(), &mut budget)
                    .unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let (forwarded_inventory, receipt) =
                    Inventory::derive(prefix.output(), &mut budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                budget
                    .reserve_storage(std::mem::size_of::<Vec<RefinedOrigin>>())
                    .unwrap();
                let refined_origins =
                    reference_rows::<RefinedOrigin>(refined.origins().len(), &mut budget);
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
                    forwarded_inventory,
                    forwarding_pair,
                    unrolled_inventory,
                    unroll_pair,
                ));
                drop((refined_origins, forwarded_origins, origins, constructed));
                result
            }};
        }
        let result = match (&input, &ready) {
            (Some(FinalFixture::Direct(prefix)), None) => frontier!(prefix, None, DirectU),
            (Some(FinalFixture::Erased(prefix)), None) => frontier!(prefix, None, ErasedU),
            (None, Some(UnrolledFixture::Direct(owner))) => {
                frontier!(owner.prefix(), Some(owner.continuation()), DirectU)
            }
            (None, Some(UnrolledFixture::Erased(owner))) => {
                frontier!(owner.prefix(), Some(owner.continuation()), ErasedU)
            }
            _ => panic!("one genuine source owner at the public replay frontier"),
        };
        budget
            .release_storage(budget.storage().checked_sub(floor).unwrap())
            .unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x79; 37]);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    drop((input, ready));
    PrefixObservation {
        result,
        floor,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}
#[test]
fn source_unroll_source_cap_and_iteration_ceiling_are_typed_refusals() {
    for shared in [false, true] {
        for oversized_source in [false, true] {
            let (input, floor) = forwarded(shared, Profile::Gfx942, Some(3));
            let mut limits = Limits::default();
            if oversized_source {
                limits.loops.operations = usize::MAX;
            } else {
                limits.max_iterations = 9;
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let result = input.unroll(limits, &mut budget);
            match result {
                Err(Error::SourceOperationsLimit {
                    requested,
                    source_limit,
                }) if oversized_source => {
                    assert_eq!(requested, usize::MAX);
                    assert!(source_limit < requested);
                }
                Err(Error::Continuation(
                    fe2o3_kernel_opt::OwnedLoopUnrollErrorV1::InvalidIterationLimit(9),
                )) if !oversized_source => {}
                Err(error) => panic!("exact configuration refusal: {error:?}"),
                Ok(_) => panic!("configuration is not a no-op"),
            }
            assert_eq!(budget.storage(), floor);
        }
    }
}
