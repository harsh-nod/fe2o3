use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_kernel_opt::{
    OwnedCrossBlockForwardingErrorV1 as FError, OwnedInductionRefinementErrorV1 as RError,
};
use std::mem::size_of;

#[derive(Debug)]
enum Failure {
    Refinement(RError),
    Forwarding(FError),
}
#[derive(Debug)]
struct Run {
    result: Result<(), Failure>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
const PREFIX: usize = 17;

fn run(
    input: &Owner,
    input_storage: usize,
    sibling: &Vec<u8>,
    work_limit: usize,
    storage_limit: usize,
) -> Run {
    let mut work = Work::new(work_limit);
    work.charge_work(PREFIX).unwrap();
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        let ledger = budget.work_ledger_identity_v1();
        let sibling_storage = size_of::<Vec<u8>>() + sibling.capacity();
        budget
            .reserve_storage(sibling_storage + input_storage)
            .unwrap();
        let floor = budget.storage();
        let result = match refine(input, Default::default(), &mut budget) {
            Err(error) => Err(Failure::Refinement(error)),
            Ok(refined) => {
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(refined.retained_storage()).unwrap();
                let middle_floor = budget.storage();
                let result = match forward(refined.output(), Default::default(), &mut budget) {
                    Err(error) => Err(Failure::Forwarding(error)),
                    Ok(final_owner) => {
                        assert_eq!(budget.storage(), middle_floor);
                        // No subsequent controlled work: inspect and drop the
                        // unreserved addition before returning to the caller.
                        assert!(!final_owner.grants_authority());
                        drop(final_owner);
                        Ok(())
                    }
                };
                assert_eq!(budget.storage(), middle_floor);
                let size = refined.retained_storage();
                drop(refined);
                budget.release_storage(size).unwrap();
                result
            }
        };
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling.as_slice(), &[0x3c; 43]);
        let metrics = (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        );
        // Both borrowed input and sibling remain live in their caller; this
        // trial performs no controlled work after ending its temporary coverage.
        budget.release_storage(input_storage).unwrap();
        budget.release_storage(sibling_storage).unwrap();
        assert_eq!(budget.storage(), 0);
        metrics
    };
    Run {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

#[test]
fn canonical_wire_chain_exact_resources_and_final_work_denial() {
    for case in [CASES[0], CASES[3], CASES[11]] {
        let mut admission_work = Work::new(WORK);
        let mut admission_budget = Budget::new(&mut admission_work, STORAGE);
        let (input, input_storage) = decode(&hex(case.wire), &mut admission_budget);
        admission_budget.reserve_storage(input_storage).unwrap();
        let sibling = vec![0x3c; 43];
        let measured = run(&input, input_storage, &sibling, WORK, STORAGE);
        assert!(measured.result.is_ok(), "{measured:?}");
        assert_eq!(measured.failed_work, None);
        assert_eq!(measured.failed_storage, None);
        let exact = run(
            &input,
            input_storage,
            &sibling,
            measured.work,
            measured.peak,
        );
        assert!(exact.result.is_ok(), "{exact:?}");
        assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
        assert_eq!(exact.failed_work, None);
        assert_eq!(exact.failed_storage, None);
        let short = run(
            &input,
            input_storage,
            &sibling,
            measured.work - 1,
            measured.peak,
        );
        let Err(Failure::Forwarding(FError::Resource(Resource::Work(error)))) = short.result else {
            panic!("last owning forwarding charge, not any earlier resource route")
        };
        // Both public factories end with charge_work(1); F is last in L->R->F.
        assert_eq!(
            (error.actual(), error.limit()),
            (measured.work, measured.work - 1)
        );
        assert_eq!(short.work, measured.work - 1);
        assert_eq!(short.peak, measured.peak);
        assert_eq!(short.failed_work, Some(measured.work));
        assert_eq!(short.failed_storage, None);
        drop(input);
        admission_budget.release_storage(input_storage).unwrap();
        assert_eq!(admission_budget.storage(), 0);
    }
}

#[test]
fn canonical_wire_chain_header_storage_denial_precedes_selection() {
    let mut admission_work = Work::new(WORK);
    let mut admission_budget = Budget::new(&mut admission_work, STORAGE);
    let (input, input_storage) = decode(&hex(CASES[0].wire), &mut admission_budget);
    admission_budget.reserve_storage(input_storage).unwrap();
    let sibling = vec![0x3c; 43];
    // At the inherited floor, the private scope's first header reservation
    // must fail before selection. Its byte extent is deliberately not mirrored.
    let floor = input_storage + size_of::<Vec<u8>>() + sibling.capacity();
    let short = run(&input, input_storage, &sibling, WORK, floor);
    let Err(Failure::Refinement(RError::Resource(Resource::Storage(error)))) = short.result else {
        panic!("exact first scope header storage refusal")
    };
    assert_eq!(error.limit(), floor);
    assert!(error.actual() > floor);
    assert_eq!(short.work, PREFIX);
    assert_eq!(short.peak, floor);
    assert_eq!(short.failed_work, None);
    assert_eq!(short.failed_storage, Some(error.actual()));
    drop(input);
    admission_budget.release_storage(input_storage).unwrap();
    assert_eq!(admission_budget.storage(), 0);
}
