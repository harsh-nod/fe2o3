//! Existing expression scope resources; fixtures confer no source/proof custody.
use super::*;
use crate::conditional_reference_v1::fixtures::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn run(
    limit: usize,
    storage_limit: usize,
) -> (
    Result<(), Error>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let fixture = Fixture::new();
    let kernel = kernel();
    let gpu = gpu_constant();
    let mut write = fixture.writes[0].clone();
    write.rhs = constant();
    let mut work = Work::new(limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.charge_work(17).unwrap();
    budget.reserve_storage(31).unwrap();
    let account = budget.work_ledger_identity_v1();
    let result = with_reference_expression(
        &fixture.input(),
        &write,
        ReferenceScalarTypeV1::F32,
        &kernel,
        &gpu,
        &[],
        &mut budget,
        |actual| {
            assert_eq!(*actual, gpu);
            Ok(())
        },
    );
    assert!(budget.work_ledger_identity_v1() == account);
    assert_eq!(budget.storage(), 31);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.failed_work(),
        budget.failed_storage(),
    )
}

#[test]
fn portable_join_expression_original_account_exact_and_one_short() {
    let (result, work, peak, failed_work, failed_storage) = run(usize::MAX, usize::MAX);
    result.unwrap();
    assert_eq!((failed_work, failed_storage), (None, None));
    let exact = run(work, peak);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3, exact.4),
        (work, peak, None, None)
    );
    let short_work = run(work - 1, peak);
    assert!(short_work.0.is_err());
    assert_eq!(short_work.3, Some(work));
    assert_eq!(short_work.4, None);
    let short_storage = run(work, peak - 1);
    assert!(short_storage.0.is_err());
    assert_eq!(short_storage.1, work);
    assert_eq!(short_storage.4, Some(peak));
}

#[test]
fn portable_join_expression_preserves_prior_denials_on_success_error_and_unwind() {
    let fixture = Fixture::new();
    let kernel = kernel();
    let gpu = gpu_constant();
    let mut write = fixture.writes[0].clone();
    write.rhs = constant();
    for mode in 0..3 {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(31).unwrap();
        assert!(budget.charge_work(1_000_000).is_err());
        assert!(budget.reserve_storage(1_000_000).is_err());
        let history = (budget.failed_work(), budget.failed_storage());
        let account = budget.work_ledger_identity_v1();
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_reference_expression(
                &fixture.input(),
                &write,
                ReferenceScalarTypeV1::F32,
                &kernel,
                &gpu,
                &[],
                &mut budget,
                |actual| {
                    assert_eq!(*actual, gpu);
                    match mode {
                        0 => Ok(()),
                        1 => Err(reject("component consumer refusal")),
                        _ => panic!("component consumer unwind"),
                    }
                },
            )
        }));
        match mode {
            0 => result.unwrap().unwrap(),
            1 => assert!(matches!(
                result.unwrap(),
                Err(Error::UnsupportedReference("component consumer refusal"))
            )),
            _ => assert!(result.is_err()),
        }
        assert!(budget.work_ledger_identity_v1() == account);
        assert_eq!(budget.storage(), 31);
        assert_eq!((budget.failed_work(), budget.failed_storage()), history);
        assert!(budget.work() > 17);
        assert!(budget.peak_storage() > 31);
    }
}

#[test]
fn portable_join_expression_conversion_error_releases_owned_scratch() {
    let fixture = Fixture::new();
    let kernel = kernel();
    let mut write = fixture.writes[0].clone();
    write.rhs = constant();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(31).unwrap();
    let result = with_reference_expression::<()>(
        &fixture.input(),
        &write,
        ReferenceScalarTypeV1::U32,
        &kernel,
        &gpu_constant(),
        &[],
        &mut budget,
        |_| panic!("invalid conversion exposed"),
    );
    assert!(matches!(
        result,
        Err(Error::UnsupportedReference(
            "reference output RHS type disagrees with its logical ABI"
        ))
    ));
    assert_eq!(budget.storage(), 31);
    assert!(budget.peak_storage() > 31);
}
