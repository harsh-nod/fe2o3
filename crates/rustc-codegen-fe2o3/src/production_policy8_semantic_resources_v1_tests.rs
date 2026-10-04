//! Live actual-stage accounting; no per-phase production fault selector.
use super::*;
use crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 2_000_000_000;

pub(super) fn exercise(
    stage: &PreparedPolicy8ArtifactsV1,
    claims: PortablePolicy7ClaimsV1<'_>,
    owner_floor: usize,
) {
    let measure = |work_limit, storage_limit, prior_work, unrelated| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let floor = owner_floor + unrelated;
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(prior_work).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = super::tests::validate(stage, claims, &mut budget);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result.map_err(|e| format!("{e:?}")),
            budget.work(),
            budget.peak_storage(),
        )
    };
    let baseline = measure(WORK, STORAGE, 0, 0);
    assert!(baseline.0.is_ok());
    let exact = measure(baseline.1, baseline.2, 0, 0);
    assert_eq!(baseline, exact);
    assert!(measure(baseline.1 - 1, baseline.2, 0, 0).0.is_err());
    assert!(measure(baseline.1, baseline.2 - 1, 0, 0).0.is_err());
    let cumulative = measure(baseline.1 + 17, baseline.2 + 31, 17, 31);
    assert_eq!(cumulative.0, baseline.0);
    assert_eq!(cumulative.1, baseline.1 + 17);
    assert_eq!(cumulative.2, baseline.2 + 31);
    assert!(measure(baseline.1 + 16, baseline.2 + 31, 17, 31).0.is_err());
    assert!(measure(baseline.1 + 17, baseline.2 + 30, 17, 31).0.is_err());

    // The surviving actual-stage floor, not just the new receipt size, is
    // mandatory before any work or local reservation is accepted.
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = stage.retained_floor - 1;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    assert!(matches!(
        super::tests::check(stage, claims, &mut budget),
        Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
            CheckedOutputPolicy7StageErrorV1::Resource(Resource::Accounting)
        ))
    ));
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);

    failure_after_actual_receipt(stage, claims, owner_floor);
    foreign_after_actual_receipt(stage, claims, owner_floor);
}

fn failure_after_actual_receipt(
    stage: &PreparedPolicy8ArtifactsV1,
    claims: PortablePolicy7ClaimsV1<'_>,
    owner_floor: usize,
) {
    for panic in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = owner_floor + 37;
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result: Result8<()> = scoped(stage.retained_floor, &mut budget, |budget| {
            let receipt = super::tests::check(stage, claims, budget)?;
            budget
                .reserve_storage(receipt.retained_storage())
                .map_err(resource)?;
            if panic {
                panic!("actual Policy8 receipt cleanup control");
            }
            drop(receipt);
            Err(execution_error("actual Policy8 receipt error control"))
        });
        if panic {
            assert!(matches!(
                result,
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    CheckedOutputPolicy7StageErrorV1::Panicked
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ProductionPipelineError::CheckedOutputPolicy8Stage(
                    CheckedOutputPolicy8StageErrorV1::Execution(
                        "actual Policy8 receipt error control"
                    )
                ))
            ));
        }
        assert!(budget.work() > 0);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

fn foreign_after_actual_receipt(
    stage: &PreparedPolicy8ArtifactsV1,
    claims: PortablePolicy7ClaimsV1<'_>,
    owner_floor: usize,
) {
    let mut work = Work::new(WORK);
    let mut other_work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let mut other = Budget::new(&mut other_work, STORAGE);
    budget.reserve_storage(owner_floor).unwrap();
    other.reserve_storage(73).unwrap();
    let original = budget.work_ledger_identity_v1();
    let foreign = other.work_ledger_identity_v1();
    let mut retained = 0;
    let result = scoped(stage.retained_floor, &mut budget, |budget| {
        let receipt = super::tests::check(stage, claims, budget)?;
        retained = receipt.retained_storage();
        budget.reserve_storage(retained).map_err(resource)?;
        std::mem::swap(budget, &mut other);
        Ok(receipt)
    });
    assert!(matches!(
        result,
        Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
            CheckedOutputPolicy7StageErrorV1::Resource(Resource::Accounting)
        ))
    ));
    // The rejected borrowed receipt has dropped. Cleanup never releases or
    // charges the substituted ledger, nor hides the original reservation.
    assert!(budget.work_ledger_identity_v1() == foreign);
    assert_eq!(budget.storage(), 73);
    assert_eq!(budget.work(), 0);
    assert!(other.work_ledger_identity_v1() == original);
    assert_eq!(other.storage(), owner_floor + retained);
    assert!(other.work() > 0);
    other.release_storage(retained).unwrap();
    assert_eq!(other.storage(), owner_floor);
}
