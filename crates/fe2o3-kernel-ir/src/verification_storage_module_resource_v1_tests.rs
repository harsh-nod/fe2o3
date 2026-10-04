use super::*;
use crate::{CanonicalKernelIrWorkBudgetV1 as Work, verify_module_ref_with_budget_v1};

const FLOOR: usize = 23;
const PRIOR_WORK: usize = 11;
const DIAGNOSTIC_ROW: usize =
    std::mem::size_of::<crate::Diagnostic>().div_ceil(std::mem::size_of::<usize>());

// This is the source-declared typed-header envelope, not a receipt or an RSS
// estimate. The existing verifier's scratch below is in its original units.
fn headers() -> usize {
    std::mem::size_of::<StorageVerificationScope<'_, '_>>()
        + std::mem::size_of::<StructurallyCheckedModuleStorageV1<'_>>()
        + std::mem::size_of::<VerifiedStorageKernelIrModuleV1<'_>>()
        + 6 * std::mem::size_of::<VerificationStorageContextV1<'_, '_>>()
        + 2 * std::mem::size_of::<Result<VerifiedStorageKernelIrModuleV1<'_>, Error>>()
        + 2 * std::mem::size_of::<Result<(), Error>>()
        + 2 * std::mem::size_of::<Result<(), MeteredKernelIrVerificationErrorV1>>()
        + 2 * std::mem::size_of::<Result<(), ResourceError>>()
}

struct Observation<'module> {
    result: Result<VerifiedStorageKernelIrModuleV1<'module>, Error>,
    work: usize,
    storage: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}

fn run(module: &Module, work_limit: usize, storage_limit: usize) -> Observation<'_> {
    // The prechecked immutable input belongs to the caller. Its structural
    // validation uses a separate setup ledger, never used to derive a quota.
    let checked = super::tests::checked(module);
    let mut work = Work::new(PRIOR_WORK + work_limit);
    work.charge_work(PRIOR_WORK).unwrap();
    let mut budget = Budget::new(&mut work, FLOOR + storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = verify_storage_module_ref_with_budget_v1(checked, None, &mut budget);
    Observation {
        result,
        work: budget.work(),
        storage: budget.storage(),
        peak: budget.peak_storage(),
        failed_work: budget.work_budget_v1().failed_work(),
        failed_storage: budget.failed_storage(),
    }
}

fn work_failure(observed: &Observation<'_>, accepted: usize, attempted: usize, limit: usize) {
    assert!(
        matches!(
            &observed.result,
            Err(Error::Resource(ResourceError::Work(error)))
                if error.actual() == PRIOR_WORK + attempted && error.limit() == PRIOR_WORK + limit
        ),
        "{:?}",
        observed.result
    );
    assert_eq!(observed.work, PRIOR_WORK + accepted);
    assert_eq!(observed.failed_work, Some(PRIOR_WORK + attempted));
    assert_eq!(observed.storage, FLOOR);
}

#[test]
fn storage_module_headers_are_explicit_full_typed_slots() {
    assert_eq!(storage_verification_headers_v1().unwrap(), headers());
    assert!(
        headers() >= 2 * std::mem::size_of::<Result<VerifiedStorageKernelIrModuleV1<'_>, Error>>()
    );
}

#[test]
fn storage_module_empty_success_has_independent_exact_work_and_live_peak() {
    // Entry1 + existing depth preflight1 + five-field module location5.
    const WORK: usize = 1 + 1 + 5;
    let module = Module::new("module");
    let observed = run(&module, WORK, headers());
    assert!(std::ptr::eq(observed.result.unwrap().module(), &module));
    assert_eq!(
        (observed.work, observed.storage, observed.peak),
        (PRIOR_WORK + WORK, FLOOR, FLOOR + headers())
    );
    assert_eq!(
        (observed.failed_work, observed.failed_storage),
        (None, None)
    );
}

#[test]
fn storage_module_empty_every_cutoff_respects_real_atomic_debits() {
    let module = Module::new("module");
    // The five-unit location component is one real debit, not five invented
    // unit debits. Limits 2..6 therefore all preserve the same accepted prefix.
    for (limit, accepted, attempted) in [
        (0, 0, 1),
        (1, 1, 2),
        (2, 2, 7),
        (3, 2, 7),
        (4, 2, 7),
        (5, 2, 7),
        (6, 2, 7),
    ] {
        let observed = run(&module, limit, headers());
        work_failure(&observed, accepted, attempted, limit);
        assert_eq!(
            observed.peak,
            FLOOR + if limit == 0 { 0 } else { headers() }
        );
        assert_eq!(observed.failed_storage, None);
    }
}

#[test]
fn storage_module_header_one_short_fails_before_depth_or_table_queries() {
    let module = Module::new("module");
    let observed = run(&module, 7, headers() - 1);
    assert!(matches!(observed.result,
        Err(Error::Resource(ResourceError::Storage(error)))
            if error.actual() == FLOOR + headers() && error.limit() == FLOOR + headers() - 1));
    assert_eq!(
        (observed.work, observed.storage, observed.peak),
        (PRIOR_WORK + 1, FLOOR, FLOOR)
    );
    assert_eq!(observed.failed_storage, Some(FLOOR + headers()));
    assert_eq!(observed.failed_work, None);
}

#[test]
fn storage_module_semantic_diagnostic_uses_existing_source_derived_schedule() {
    // New entry1; old preflight1; count44; materialization111 plus empty-ID
    // copy7; finish1. The 33-byte message coexists with one diagnostic row.
    const WORK: usize = 1 + 1 + 44 + 111 + 7 + 1;
    let module = Module::new("");
    let scratch = DIAGNOSTIC_ROW + 33;
    let observed = run(&module, WORK, headers() + scratch);
    match observed.result {
        Err(Error::Verification(errors)) => {
            assert_eq!(errors, crate::verify_module_ref(&module).unwrap_err())
        }
        other => panic!("expected semantic rejection: {other:?}"),
    }
    assert_eq!(
        (observed.work, observed.storage, observed.peak),
        (PRIOR_WORK + WORK, FLOOR, FLOOR + headers() + scratch)
    );
    let denied = run(&module, WORK, headers() + scratch - 1);
    assert!(matches!(denied.result,
        Err(Error::Resource(ResourceError::Storage(error)))
            if error.actual() == FLOOR + headers() + scratch
                && error.limit() == FLOOR + headers() + scratch - 1));
    // Admission stops after old accepted158 plus the new entry1. The row
    // header is retained, but the refused message is never reserved.
    assert_eq!(
        (denied.work, denied.storage, denied.peak),
        (
            PRIOR_WORK + 1 + 158,
            FLOOR,
            FLOOR + headers() + DIAGNOSTIC_ROW
        )
    );
}

#[test]
fn storage_module_diagnostic_cutoffs_use_complete_real_debit_transcript() {
    let module = Module::new("");
    // One new entry followed by the existing empty-ID borrowed facade's
    // complete source transcript. No CFG-sized opaque component is expanded.
    let charges = [1, 1, 5, 1, 33, 5, 5, 1, 33, 7, 67, 5, 1];
    let exact: usize = charges.iter().sum();
    assert_eq!(exact, 165);
    for limit in 0..exact {
        let mut accepted = 0;
        let mut attempted = 0;
        for charge in charges {
            attempted = accepted + charge;
            if attempted > limit {
                break;
            }
            accepted = attempted;
        }
        let observed = run(&module, limit, headers() + DIAGNOSTIC_ROW + 33);
        work_failure(&observed, accepted, attempted, limit);
        assert!(observed.peak <= FLOOR + headers() + DIAGNOSTIC_ROW + 33);
    }
}

#[test]
fn storage_module_real_storage_cfg_releases_scratch_on_success_and_rejection() {
    let module = super::tests::allocation(crate::AddressSpace::Private);
    let observed = run(&module, 1_000_000, 1_000_000);
    assert!(observed.result.is_ok());
    assert_eq!(observed.storage, FLOOR);
    let mut invalid = module.clone();
    invalid.functions[0].body.as_mut().unwrap().blocks[0].terminator = None;
    let rejected = run(&invalid, 1_000_000, 1_000_000);
    assert!(matches!(rejected.result, Err(Error::Verification(_))));
    assert_eq!(rejected.storage, FLOOR);
    // This is a declared entry/preflight boundary, not a claimed arbitrary
    // cutoff oracle for the CFG and diagnostic sorting internals.
    let denied = run(&module, 1, headers());
    work_failure(&denied, 1, 2, 1);
    assert_eq!(denied.peak, FLOOR + headers());
}

#[test]
fn storage_module_preserves_prior_denials_and_does_not_reset_history() {
    let module = Module::new("module");
    let checked = super::tests::checked(&module);
    let mut work = Work::new(PRIOR_WORK + 7);
    work.charge_work(PRIOR_WORK).unwrap();
    assert!(work.charge_work(99).is_err());
    let mut budget = Budget::new(&mut work, FLOOR + headers());
    budget.reserve_storage(FLOOR).unwrap();
    assert!(budget.reserve_storage(headers() + 9).is_err());
    let first_work = budget.work_budget_v1().failed_work();
    let first_storage = budget.failed_storage();
    let result = verify_storage_module_ref_with_budget_v1(checked, None, &mut budget);
    assert!(result.is_ok());
    assert_eq!(budget.work(), PRIOR_WORK + 7);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR + headers());
    assert_eq!(budget.work_budget_v1().failed_work(), first_work);
    assert_eq!(budget.failed_storage(), first_storage);
    assert_eq!(first_work, Some(PRIOR_WORK + 99));
    assert_eq!(first_storage, Some(FLOOR + headers() + 9));
}

#[test]
fn storage_module_overflow_refusal_keeps_the_actual_caller_floor() {
    let module = Module::new("module");
    let checked = super::tests::checked(&module);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(usize::MAX).unwrap();
    assert!(
        matches!(verify_storage_module_ref_with_budget_v1(checked, None, &mut budget),
        Err(Error::Resource(ResourceError::Storage(error))) if error.actual() == usize::MAX)
    );
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (1, usize::MAX, usize::MAX)
    );
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
}

#[test]
fn storage_module_scope_unwind_restores_floor_after_inner_owner_drop() {
    // Direct guard regression only: no allocator panic or public-entry panic
    // injection is claimed. The production facade has no callback hook.
    let mut work = Work::new(17);
    let mut budget = Budget::new(&mut work, FLOOR + headers());
    budget.reserve_storage(FLOOR).unwrap();
    let dropped = std::cell::Cell::new(false);
    struct Inner<'a>(&'a std::cell::Cell<bool>);
    impl Drop for Inner<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget.reserve_storage(headers()).unwrap();
        let scope = StorageVerificationScope {
            budget: &mut budget,
            floor: Some(FLOOR),
        };
        let _inner = Inner(&dropped);
        scope.budget.charge_work(1).unwrap();
        panic!("typed scope unwind");
    }));
    assert!(unwind.is_err());
    assert!(dropped.get());
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (1, FLOOR, FLOOR + headers())
    );
}

#[test]
fn storage_module_legacy_empty_module_keeps_its_original_exact_boundary() {
    let module = Module::new("module");
    let mut work = Work::new(6);
    let mut budget = Budget::new(&mut work, 0);
    assert!(verify_module_ref_with_budget_v1(&module, None, &mut budget).is_ok());
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (6, 0, 0)
    );
}
