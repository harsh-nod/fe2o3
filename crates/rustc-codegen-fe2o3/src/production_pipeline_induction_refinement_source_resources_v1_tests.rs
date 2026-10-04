use super::*;
use fe2o3_kernel_analysis::CanonicalKirInventoryV1 as Inventory;
use fe2o3_kernel_opt::{
    OwnedInductionRefinementContinuationV1 as RefinementTail, prepare_owned_induction_refinement_v1,
};
use fe2o3_lower_mir_kernel::{
    ProductionCheckedOutputAdmissionErrorPolicy6V1 as Policy6Error,
    ProductionCommutativeContinuationErrorV1 as CommutativeError,
    ProductionPrivateCellPromotionContinuationErrorV1 as PromotionError,
    ProductionRedundantStoreAdmissionErrorV1 as RedundantStoreError,
};
const W: usize = 1_000_000_000;
const S: usize = 1024 * 1024 * 1024;
const PREFIX_WORK: usize = 17;
#[derive(Debug)]
struct Observation {
    result: std::result::Result<(), RefineError>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
    retained: Option<usize>,
}
fn measure_factory(
    erased: bool,
    profile: Profile,
    mutation: bool,
    w: usize,
    s: usize,
) -> Observation {
    let mut observation = None;
    with_donor(erased, profile, mutation, |prefix, _, parent| {
        let floor = parent.storage();
        let mut work = Work::new(w);
        let mut retained = None;
        let (result, accepted, peak, failed_storage) = {
            let mut budget = Budget::new(&mut work, s);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(PREFIX_WORK).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = refine(prefix, Limits::default(), &mut budget).map(|(owner, addition)| {
                assert_eq!(owner.addition(), addition);
                retained = Some(addition);
                // The unreserved returned owner is dropped before any further
                // controlled operation. Its genuine donors remain live outside.
                drop(owner);
            });
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            (
                result,
                budget.work(),
                budget.peak_storage(),
                budget.failed_storage(),
            )
        };
        observation = Some(Observation {
            result,
            work: accepted,
            peak,
            failed_work: work.failed_work(),
            failed_storage,
            retained,
        });
    });
    observation.unwrap()
}
fn measure_replay(owner: &Refined, floor: usize, w: usize, s: usize) -> Observation {
    let mut work = Work::new(w);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, s);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(PREFIX_WORK).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = owner.replay(&mut budget);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    Observation {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
        retained: None,
    }
}
fn successful(observation: &Observation) {
    assert!(observation.result.is_ok(), "{observation:?}");
    assert_eq!(observation.failed_work, None);
    assert_eq!(observation.failed_storage, None);
    assert!(observation.work > PREFIX_WORK);
}
fn exact_work_short(short: &Observation, measured: &Observation) {
    let Err(RefineError::Resource(Resource::Work(limit))) = &short.result else {
        panic!("final one-unit source refinement work refusal: {short:?}")
    };
    assert_eq!(
        (limit.actual(), limit.limit()),
        (measured.work, measured.work - 1)
    );
    assert_eq!(short.work, measured.work - 1);
    assert_eq!(short.failed_work, Some(measured.work));
    assert_eq!(short.failed_storage, None);
    assert_eq!(short.peak, measured.peak);
    assert_eq!(short.retained, None);
}
#[test]
fn source_induction_refinement_factory_exact_and_short_work_keep_actual_donors() {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let measured = measure_factory(erased, profile, mutation, W, S);
                successful(&measured);
                assert!(measured.retained.unwrap() > 0);
                let exact =
                    measure_factory(erased, profile, mutation, measured.work, measured.peak);
                successful(&exact);
                assert_eq!(
                    (exact.work, exact.peak, exact.retained),
                    (measured.work, measured.peak, measured.retained)
                );
                let short = measure_factory(erased, profile, mutation, measured.work - 1, S);
                exact_work_short(&short, &measured);
            }
        }
    }
}
#[test]
fn source_induction_refinement_factory_storage_short_observes_first_denial_pending_phase() {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let measured = measure_factory(erased, profile, mutation, W, S);
                successful(&measured);
                let reference = factory_reference(erased, profile, mutation, S);
                assert!(reference.result.is_ok());
                assert_eq!(
                    (
                        reference.peak,
                        reference.failed_work,
                        reference.failed_storage
                    ),
                    (measured.peak, None, None)
                );
                let reference = factory_reference(erased, profile, mutation, measured.peak - 1);
                assert_reference_storage(&reference, measured.peak);
                let short = measure_factory(erased, profile, mutation, W, measured.peak - 1);
                assert_refinement_storage(&short, measured.peak);
                assert_eq!(short.failed_storage, Some(measured.peak));
                assert_eq!(short.failed_work, None);
                assert_eq!(short.retained, None);
                assert_eq!((short.work, short.peak), (reference.work, reference.peak));
            }
        }
    }
}
#[test]
fn source_induction_refinement_replay_exact_and_short_work_preserve_complete_live_owner() {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_refined(erased, profile, mutation, |owner, _, budget| {
                    let floor = budget.storage();
                    let measured = measure_replay(owner, floor, W, S);
                    successful(&measured);
                    let exact = measure_replay(owner, floor, measured.work, measured.peak);
                    successful(&exact);
                    assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
                    exact_work_short(
                        &measure_replay(owner, floor, measured.work - 1, S),
                        &measured,
                    );
                    assert_eq!(budget.storage(), floor);
                });
            }
        }
    }
}
#[test]
fn source_induction_refinement_replay_storage_short_observes_first_denial_pending_phase() {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_refined(erased, profile, mutation, |owner, _, budget| {
                    let floor = budget.storage();
                    let measured = measure_replay(owner, floor, W, S);
                    successful(&measured);
                    let reference = replay_reference(owner, floor, S);
                    assert!(reference.result.is_ok());
                    assert_eq!(
                        (
                            reference.peak,
                            reference.failed_work,
                            reference.failed_storage
                        ),
                        (measured.peak, None, None)
                    );
                    let reference = replay_reference(owner, floor, measured.peak - 1);
                    assert_reference_storage(&reference, measured.peak);
                    let short = measure_replay(owner, floor, W, measured.peak - 1);
                    assert_refinement_storage(&short, measured.peak);
                    assert_eq!(short.failed_storage, Some(measured.peak));
                    assert_eq!(short.failed_work, None);
                    assert_eq!(short.retained, None);
                    assert_eq!((short.work, short.peak), (reference.work, reference.peak));
                    assert_eq!(budget.storage(), floor);
                });
            }
        }
    }
}

#[derive(Debug)]
struct PrefixObservation {
    result: std::result::Result<(), CommutativeError>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn assert_p8_storage(error: &CommutativeError, peak: usize) {
    let CommutativeError::Prefix(redundant) = error else {
        panic!("exact commutative source-prefix refusal: {error:?}")
    };
    let RedundantStoreError::Prefix(policy6) = redundant.as_ref() else {
        panic!("exact redundant-store source-prefix refusal: {redundant:?}")
    };
    let Policy6Error::Optimization(fe2o3_kernel_opt::CanonicalPolicy6OptimizationErrorV1::Map(
        fe2o3_pliron::KirOptimizationMapErrorV12::Resources(Resource::Storage(limit)),
    )) = policy6.as_ref()
    else {
        panic!("exact checked-map scratch Storage refusal: {policy6:?}")
    };
    assert_eq!((limit.actual(), limit.limit()), (peak, peak - 1));
}
fn assert_reference_storage(reference: &PrefixObservation, peak: usize) {
    let Err(error) = &reference.result else {
        panic!("independent final P8 replay must refuse: {reference:?}")
    };
    assert_p8_storage(error, peak);
    assert_eq!(
        (reference.failed_work, reference.failed_storage),
        (None, Some(peak))
    );
}
fn assert_refinement_storage(short: &Observation, peak: usize) {
    let Err(RefineError::Admission(promotion)) = &short.result else {
        panic!("exact final-source admission refusal: {short:?}")
    };
    let PromotionError::Prefix(error) = promotion.as_ref() else {
        panic!("exact promoted source-prefix refusal: {promotion:?}")
    };
    assert_p8_storage(error, peak);
}
fn reference_measure(
    floor: usize,
    storage_limit: usize,
    run: impl FnOnce(&mut Budget<'_>) -> std::result::Result<(), CommutativeError>,
) -> PrefixObservation {
    let mut work = Work::new(W);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(PREFIX_WORK).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        // The reference's actual locals drop inside run before scope credit returns.
        let result = run(&mut budget);
        budget
            .release_storage(budget.storage().checked_sub(floor).unwrap())
            .unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    PrefixObservation {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

// This test-only expansion borrows either genuine source owner without a new
// production callback. Public tail/pair/inventory receipts remain live at P8.
macro_rules! reference_prefix {
    ($source:expr, $tail:expr, $header:expr, $budget:expr) => {{
        let source = $source;
        let saved: Option<&RefinementTail> = $tail;
        let budget = $budget;
        if saved.is_some() {
            budget.charge_work(7).unwrap();
        }
        source.verify_equivalence(budget).unwrap();
        // Existing check_limits reads the admitted source cap and charges three.
        budget.charge_work(3).unwrap();
        let candidate;
        let tail = match saved {
            Some(tail) => {
                budget
                    .reserve_storage(std::mem::size_of::<Vec<SourceOrigin>>())
                    .unwrap();
                tail
            }
            None => {
                budget.reserve_storage($header).unwrap();
                candidate = prepare_owned_induction_refinement_v1(
                    source.output(),
                    Limits::default(),
                    budget,
                )
                .unwrap();
                budget
                    .reserve_storage(candidate.retained_storage())
                    .unwrap();
                &candidate
            }
        };
        budget.charge_work(4).unwrap();
        let requested = tail
            .origins()
            .len()
            .checked_mul(std::mem::size_of::<SourceOrigin>())
            .unwrap();
        budget.reserve_storage(requested).unwrap();
        let mut origins = Vec::<SourceOrigin>::new();
        origins.try_reserve_exact(tail.origins().len()).unwrap();
        let actual = origins
            .capacity()
            .checked_mul(std::mem::size_of::<SourceOrigin>())
            .unwrap();
        budget
            .reserve_storage(actual.checked_sub(requested).unwrap())
            .unwrap();
        budget.charge_work(7).unwrap();
        assert_eq!(tail.limits(), Limits::default());
        let (refinement_pair, receipt) = tail
            .replay_against(source.output(), Limits::default(), budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (final_inventory, receipt) = Inventory::derive(tail.output(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let preheaders = source.prefix();
        let promoted = preheaders.prefix();
        let (licm_pair, receipt) = source
            .continuation()
            .replay_against(preheaders.output(), budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (preheader_pair, receipt) = preheaders
            .continuation()
            .replay_against(promoted.output(), budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (input_inventory, receipt) = Inventory::derive(preheaders.output(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (output_inventory, receipt) = Inventory::derive(source.output(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        budget.charge_work(3).unwrap();
        let result = promoted.prefix().verify_equivalence(budget);
        drop((
            output_inventory,
            input_inventory,
            preheader_pair,
            licm_pair,
            final_inventory,
            refinement_pair,
            origins,
        ));
        result
    }};
}
fn source_header<P, R>() -> usize {
    std::mem::size_of::<R>()
        .checked_sub(std::mem::size_of::<P>())
        .unwrap()
        .checked_sub(std::mem::size_of::<RefinementTail>())
        .unwrap()
}
fn factory_reference(
    erased: bool,
    profile: Profile,
    mutation: bool,
    storage_limit: usize,
) -> PrefixObservation {
    let mut observation = None;
    with_donor(erased, profile, mutation, |prefix, _, parent| {
        observation = Some(reference_measure(
            parent.storage(),
            storage_limit,
            |budget| match &prefix {
                Licm::Direct(source) => reference_prefix!(
                    source,
                    None,
                    source_header::<
                        fe2o3_lower_mir_kernel::ProductionOwnedLicmContinuationV1,
                        DirectRefined,
                    >(),
                    budget
                ),
                Licm::Erased(source) => reference_prefix!(
                    source,
                    None,
                    source_header::<
                        fe2o3_lower_mir_kernel::ProductionOwnedUnitLocalLicmContinuationV1,
                        ErasedRefined,
                    >(),
                    budget
                ),
            },
        ));
    });
    observation.unwrap()
}
fn replay_reference(owner: &Refined, floor: usize, storage_limit: usize) -> PrefixObservation {
    reference_measure(floor, storage_limit, |budget| match owner {
        Refined::Direct(value) => {
            reference_prefix!(value.prefix(), Some(value.continuation()), 0, budget)
        }
        Refined::Erased(value) => {
            reference_prefix!(value.prefix(), Some(value.continuation()), 0, budget)
        }
    })
}
