//! Descriptor-finalization accounting on a real finite protocol ledger.

use crate::mixed_worker_resources_family::mixed_worker_resources_family;

mixed_worker_resources_family!(
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53,
    NominalWorkerFinalizationErrorV53,
    PreparedFinalizedNominalWorkerHsacoV53,
    derive_unfinalized_nominal_hsaco_v53,
    finalize_protected_worker_nominal_hsaco_v53,
    MIXED_WORKER_FINALIZATION_WORK_LIMIT_V53,
    MIXED_WORKER_FINALIZATION_STORAGE_LIMIT_V53,
    MixedWorkerFinalizationBudgetErrorV53,
    NominalFinalizationErrorV53,
    finalize_protected_worker_nominal_hsaco_on_budget_v53,
    derive_unfinalized_nominal_hsaco_on_budget_v53
);

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

    #[test]
    fn mixed_finalization_budget_scope_preserves_floor_and_unwind_history() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = Budget::new(&mut work, MIXED_WORKER_FINALIZATION_STORAGE_LIMIT_V53);
        budget.reserve_storage(19).unwrap();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scope::<(), _>(&mut budget, |scratch, work| {
                assert_eq!(scratch, NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53);
                work(7)?;
                panic!("injected finalization failure")
            })
        }));
        assert!(panic.is_err());
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.work(), 8);
        assert!(budget.peak_storage() > 19 + NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53);
    }

    #[test]
    fn mixed_finalization_budget_scope_headers_are_independently_accounted() {
        fn inspect(scratch: usize, work: &mut Work<'_>) -> Result<u32, Failure> {
            assert_eq!(scratch, NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53);
            work(7)?;
            Ok(23)
        }
        type Callback = fn(usize, &mut Work<'_>) -> Result<u32, Failure>;
        let expected = NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53
            + size_of::<Callback>()
            + size_of::<Budget<'_>>()
            + size_of::<&mut Work<'_>>()
            + 3 * size_of::<usize>()
            + size_of::<u32>()
            + 2 * size_of::<Result<u32, Failure>>()
            + size_of::<Result<Result<u32, Failure>, Box<dyn std::any::Any + Send>>>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
        let mut budget = Budget::new(&mut work, expected + 19);
        budget.reserve_storage(19).unwrap();
        assert_eq!(scope(&mut budget, inspect as Callback).unwrap(), 23);
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (8, 19, expected + 19)
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
        let mut budget = Budget::new(&mut work, expected + 18);
        budget.reserve_storage(19).unwrap();
        assert!(matches!(scope(&mut budget, inspect as Callback),
            Err(Failure::Resource(Resource::Storage(e)))
            if e.actual() == expected + 19 && e.limit() == expected + 18));
        assert_eq!(budget.work(), 1);
        assert_eq!(budget.storage(), 19);
        assert!(matches!(scope(&mut budget, inspect as Callback),
            Err(Failure::Resource(Resource::Storage(e)))
            if e.actual() == expected + 19 && e.limit() == expected + 18));
        assert_eq!(budget.work(), 1);
    }

    #[test]
    fn mixed_finalization_payment_failure_precedes_callback_mutation() {
        fn run(
            work_limit: usize,
            storage_limit: usize,
        ) -> (Result<(), Failure>, [u8; 8], usize, usize) {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            let mut output = [0x5a; 8];
            let result = scope(&mut budget, |_, work| {
                work(7)?;
                output.fill(0xa5);
                Ok(())
            });
            (result, output, budget.work(), budget.peak_storage())
        }
        let measured = run(8, MIXED_WORKER_FINALIZATION_STORAGE_LIMIT_V53);
        measured.0.unwrap();
        assert_eq!(measured.1, [0xa5; 8]);
        let exact = run(8, measured.3);
        exact.0.unwrap();
        assert_eq!(exact.1, [0xa5; 8]);
        let short = run(7, measured.3);
        assert!(matches!(short.0, Err(Failure::Resource(Resource::Work(e)))
            if e.actual() == 8 && e.limit() == 7));
        assert_eq!(short.1, [0x5a; 8]);
        let short = run(8, measured.3 - 1);
        assert!(
            matches!(short.0, Err(Failure::Resource(Resource::Storage(e)))
            if e.actual() == measured.3 && e.limit() == measured.3 - 1)
        );
        assert_eq!(short.1, [0x5a; 8]);
    }

    #[test]
    fn mixed_recovery_uses_paid_raw_and_finalization_without_legacy_callback() {
        let schemas = include_str!("worker_v3_finalized_schema.rs");
        let mixed = schemas
            .split_once(
                "Self::MixedV53 => crate::mixed_worker_resources_v53::derive_raw_default(bytes)",
            )
            .expect("compact reconstruction must enter the finite default account");
        assert!(
            !mixed
                .1
                .split_once("pub(crate) fn derive_raw_on_mixed_budget")
                .unwrap()
                .0
                .contains("Infallible")
        );
        let replay = include_str!("worker_v3_hsaco_publication.rs");
        assert!(replay.contains(
            "schema.derive_raw_on_mixed_budget(exact_finalized_hsaco, &mut mixed_budget)"
        ));
        let branch = replay
            .split_once("DescriptorSchema::MixedV53 => FinalizedOwner::MixedV53(")
            .unwrap()
            .1
            .split_once("let view = finalized.view();")
            .unwrap()
            .0;
        assert!(branch.contains("finalize_protected_worker_nominal_hsaco_on_budget_v53("));
        assert!(branch.contains("&mut mixed_budget"));
        assert!(
            !branch.contains("Infallible")
                && !branch.contains("NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53")
        );
    }
}
