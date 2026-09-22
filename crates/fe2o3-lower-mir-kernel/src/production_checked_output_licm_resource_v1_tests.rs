use super::*;
use crate::ProductionLicmErrorV1 as LicmError;

#[test]
fn source_licm_refuses_one_short_prefix_and_output_floors_before_work() {
    for mutation in [false, true] {
        let (prefix, _) = direct::prefix(Profile::Gfx942, mutation);
        let minimum = prefix.retained_input_storage_floor_v1().unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(minimum - 1).unwrap();
        assert!(matches!(
            prefix.continue_licm_v1(&mut budget),
            Err(LicmError::Resource(AssertOriginResourceV1::Accounting))
        ));
        assert_eq!((budget.storage(), budget.work()), (minimum - 1, 0));

        let (prefix, inherited) = direct::prefix(Profile::Gfx942, mutation);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (owner, _) = prefix.continue_licm_v1(&mut budget).unwrap();
        let minimum = owner.retained_input_storage_floor_v1().unwrap();
        let mut short_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut short = AssertOriginBudgetV1::new(&mut short_work, STORAGE);
        short.reserve_storage(minimum - 1).unwrap();
        assert!(matches!(
            owner.verify_equivalence(&mut short),
            Err(LicmError::Resource(AssertOriginResourceV1::Accounting))
        ));
        assert_eq!((short.storage(), short.work()), (minimum - 1, 0));
    }
}

type Measurements = (
    Result<(), LicmError>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
);

fn measured(profile: Profile, mutation: bool, limit: usize, storage: usize) -> Measurements {
    let (prefix, inherited) = direct::prefix(profile, mutation);
    let sibling = vec![0x6d_u8; 43];
    let floor = inherited + std::mem::size_of_val(&sibling) + sibling.capacity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = prefix.continue_licm_v1(&mut budget).map(|(owner, added)| {
            assert_eq!(
                owner.additional_retained_storage_v1(),
                added.retained_storage()
            );
            if mutation {
                actual_mutation(
                    owner.prefix().output(),
                    owner.output(),
                    owner.operation_origins(),
                );
            } else {
                assert_eq!(
                    owner.prefix().output().canonical().canonical_bytes(),
                    owner.output().canonical().canonical_bytes()
                );
                assert!(!std::ptr::eq(owner.prefix().output(), owner.output()));
            }
            drop(owner);
        });
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(sibling.iter().all(|byte| *byte == 0x6d));
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    (result, accepted, peak, failed_storage, work.failed_work())
}

#[test]
fn source_licm_exact_work_and_one_short_keep_first_denial_and_live_sibling() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let (result, work, peak, storage_denial, work_denial) =
                measured(profile, mutation, WORK, STORAGE);
            result.unwrap();
            assert_eq!((storage_denial, work_denial), (None, None));
            let (result, accepted, actual_peak, storage_denial, work_denial) =
                measured(profile, mutation, work, peak);
            result.unwrap();
            assert_eq!(
                (accepted, actual_peak, storage_denial, work_denial),
                (work, peak, None, None)
            );
            let (result, accepted, actual_peak, storage_denial, work_denial) =
                measured(profile, mutation, work - 1, peak);
            match result {
                Err(LicmError::Resource(AssertOriginResourceV1::Work(error))) => {
                    assert_eq!(error.actual(), work);
                    assert_eq!(error.limit(), work - 1);
                }
                other => panic!("exact final one-unit source-LICM work refusal: {other:?}"),
            }
            assert_eq!(
                (accepted, actual_peak, storage_denial, work_denial),
                (work - 1, peak, None, Some(work))
            );
        }
    }
}

#[test]
fn source_licm_one_short_storage_preserves_exact_nested_phase() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let (result, work, peak, storage_denial, work_denial) =
                measured(profile, mutation, WORK, STORAGE);
            result.unwrap();
            assert_eq!((storage_denial, work_denial), (None, None));
            let full = licm_public_frontier(profile, mutation, STORAGE);
            assert!(
                full.result.is_ok(),
                "successful public frontier: {:?}",
                full.result
            );
            assert_eq!((full.failed_storage, full.failed_work), (None, None));
            assert_eq!(peak, full.peak);
            assert_eq!(work, licm_public_success_work(profile, mutation));
            let expected = licm_public_frontier(profile, mutation, peak - 1);
            assert_licm_frontier_storage(&expected.result, peak);
            assert_eq!(expected.floor, full.floor);
            assert_eq!(
                (expected.failed_storage, expected.failed_work),
                (Some(peak), None)
            );
            let (result, accepted, actual_peak, storage_denial, work_denial) =
                measured(profile, mutation, work, peak - 1);
            let error =
                result.expect_err("one-short storage cannot complete the same allocation history");
            assert_eq!((storage_denial, work_denial), (Some(peak), None));
            let LicmError::Admission(error) = error else {
                panic!("expected final-source admission Storage refusal")
            };
            let crate::ProductionPrivateCellPromotionContinuationErrorV1::Prefix(error) = *error
            else {
                panic!("expected retained private-cell prefix refusal")
            };
            let crate::ProductionCommutativeContinuationErrorV1::Prefix(error) = *error else {
                panic!("expected retained commutative prefix refusal")
            };
            let crate::ProductionRedundantStoreAdmissionErrorV1::Prefix(error) = *error else {
                panic!("expected retained redundant-store prefix refusal")
            };
            let crate::ProductionCheckedOutputAdmissionErrorPolicy6V1::Optimization(
                fe2o3_kernel_opt::CanonicalPolicy6OptimizationErrorV1::Map(
                    fe2o3_pliron::KirOptimizationMapErrorV12::Resources(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(
                            error,
                        ),
                    ),
                ),
            ) = *error
            else {
                panic!("expected exact Policy6 replay map Storage refusal")
            };
            assert_eq!((error.actual(), error.limit()), (peak, peak - 1));
            assert_eq!((accepted, actual_peak), (expected.work, expected.peak));
        }
    }
}

#[test]
fn source_licm_nested_error_and_panic_drop_actual_moved_candidate() {
    let (prefix, inherited) = direct::prefix(Profile::Gfx942, true);
    prefix.exercise_licm_failed_candidate_v1(inherited);
}

struct LicmFrontier {
    result: Result<(), crate::ProductionCommutativeContinuationErrorV1>,
    floor: usize,
    work: usize,
    peak: usize,
    failed_storage: Option<usize>,
    failed_work: Option<usize>,
}

// This reconstructs only the public frontier, not LICM's source-site checker.
// P8's opaque map internals remain a separately matched constituent refusal.
fn licm_public_frontier(profile: Profile, mutation: bool, limit: usize) -> LicmFrontier {
    use fe2o3_kernel_analysis::CanonicalKirInventoryV1 as Inventory;
    use fe2o3_kernel_opt::prepare_owned_licm_v1;
    let (prefix, inherited) = direct::prefix(profile, mutation);
    if !mutation {
        let p6 = prefix.prefix().prefix().prefix().prefix();
        for graph in [
            p6.bound(),
            p6.checked_output().intermediate_policy5().owner(),
        ] {
            licm_comparison_shape_premise(graph);
        }
    }
    let sibling = vec![0x6d_u8; 43];
    let floor = inherited
        .checked_add(std::mem::size_of_val(&sibling))
        .and_then(|n| n.checked_add(sibling.capacity()))
        .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = AssertOriginBudgetV1::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        prefix
            .verify_equivalence(&mut budget)
            .expect("preheader prefix checkpoint");
        let prefix_work = budget.work();
        let tail = prepare_owned_licm_v1(prefix.output(), &mut budget)
            .expect("standalone LICM preparation checkpoint");
        budget.reserve_storage(tail.retained_storage()).unwrap();
        let prepare_work = budget.work();
        let (licm, receipt) = tail
            .replay_against(prefix.output(), &mut budget)
            .expect("LICM pair checkpoint");
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let licm_work = budget.work();
        let (preheaders, receipt) = prefix
            .continuation()
            .replay_against(prefix.prefix().output(), &mut budget)
            .expect("preheader pair checkpoint");
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let preheader_work = budget.work();
        let (input, receipt) = Inventory::derive(prefix.output(), &mut budget)
            .expect("LICM input inventory checkpoint");
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let input_work = budget.work();
        let (output, receipt) = Inventory::derive(tail.output(), &mut budget)
            .expect("LICM output inventory checkpoint");
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let output_work = budget.work();
        for (before, after) in [
            (17, prefix_work),
            (prefix_work, prepare_work),
            (prepare_work, licm_work),
            (licm_work, preheader_work),
            (preheader_work, input_work),
            (input_work, output_work),
        ] {
            assert!(after > before, "each preceding public checkpoint completed");
        }
        budget.charge_work(3).unwrap();
        let result = prefix.prefix().prefix().verify_equivalence(&mut budget);
        drop((output, input, preheaders, licm));
        drop(tail);
        budget
            .release_storage(budget.storage().checked_sub(floor).unwrap())
            .unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x6d; 43]);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    drop(prefix);
    LicmFrontier {
        result,
        floor,
        work: accepted,
        peak,
        failed_storage,
        failed_work: work.failed_work(),
    }
}

fn assert_licm_frontier_storage(
    result: &Result<(), crate::ProductionCommutativeContinuationErrorV1>,
    peak: usize,
) {
    let Err(crate::ProductionCommutativeContinuationErrorV1::Prefix(store)) = result else {
        panic!("exact public P8 prefix refusal: {result:?}")
    };
    let crate::ProductionRedundantStoreAdmissionErrorV1::Prefix(p6) = store.as_ref() else {
        panic!("exact public P7 prefix refusal: {store:?}")
    };
    let crate::ProductionCheckedOutputAdmissionErrorPolicy6V1::Optimization(
        fe2o3_kernel_opt::CanonicalPolicy6OptimizationErrorV1::Map(
            fe2o3_pliron::KirOptimizationMapErrorV12::Resources(AssertOriginResourceV1::Storage(
                error,
            )),
        ),
    ) = p6.as_ref()
    else {
        panic!("exact final-source Policy6 map reservation: {p6:?}")
    };
    assert_eq!((error.actual(), error.limit()), (peak, peak - 1));
}

// Factory and owner replay each replay the prefix, check the actual final sites,
// and pay the final unit. Only the factory additionally prepares the LICM tail.
fn licm_public_success_work(profile: Profile, mutation: bool) -> usize {
    let (prefix, inherited) = direct::prefix(profile, mutation);
    let (owner, added) = {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (owner, receipt) = prefix.continue_licm_v1(&mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        (owner, receipt.retained_storage())
    };
    let floor = inherited.checked_add(added).unwrap();
    let replay = {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        owner.verify_equivalence(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), None);
        let accepted = budget.work();
        drop(budget);
        assert_eq!(work.failed_work(), None);
        accepted
    };
    let prepare = {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let tail =
            fe2o3_kernel_opt::prepare_owned_licm_v1(owner.prefix().output(), &mut budget).unwrap();
        let retained = tail.retained_storage();
        budget.reserve_storage(retained).unwrap();
        drop(tail);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), None);
        let accepted = budget.work();
        drop(budget);
        assert_eq!(work.failed_work(), None);
        accepted
    };
    drop(owner);
    17usize
        .checked_add(replay)
        .and_then(|n| n.checked_add(prepare))
        .unwrap()
}

fn licm_comparison_shape_premise(graph: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12) {
    use fe2o3_kernel_ir::{ComparePredicate, Constant, OperationKind};
    let mut comparisons = 0;
    for function in &graph.module().functions {
        let Some(body) = &function.body else { continue };
        for block in &body.blocks {
            for operation in &block.operations {
                let OperationKind::Compare {
                    predicate,
                    lhs,
                    rhs,
                } = operation.kind
                else {
                    continue;
                };
                comparisons += 1;
                assert_eq!(predicate, ComparePredicate::Equal);
                let constant = |value| {
                    body.blocks
                        .iter()
                        .flat_map(|b| &b.operations)
                        .find_map(|op| {
                            (op.results.iter().any(|r| r.id == value)).then_some(&op.kind)
                        })
                };
                assert!(matches!(
                    constant(rhs),
                    Some(OperationKind::Constant(Constant::U32(0)))
                ));
                assert!(!matches!(constant(lhs), Some(OperationKind::Constant(_))));
            }
        }
    }
    assert_eq!(comparisons, 1);
    // Each complete P6 replay checks B/C, its control index, the decoded B/C
    // receipt and O/I. LICM's three P8 replays therefore enter 3*4 solvers.
    // A solver may iterate: this shape check neither counts boundary
    // evaluations nor proves root depth. No work expectation uses that count.
}
