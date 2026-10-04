//! Descriptor-finalization accounting on a real finite protocol ledger.
use crate::{
    InertProtectedFirstBuildWorkerV3EvidenceV1, NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53,
    NominalWorkerFinalizationErrorV53, PreparedFinalizedNominalWorkerHsacoV53,
    derive_unfinalized_nominal_hsaco_v53, finalize_protected_worker_nominal_hsaco_v53,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

/// Finite ordinary Cargo descriptor-finalization work ceiling. This is not an
/// admission promise for every artifact below the independent format ceilings.
pub const MIXED_WORKER_FINALIZATION_WORK_LIMIT_V53: usize = 1_000_000_000;
/// Finite ordinary Cargo descriptor scratch/header ceiling. Retained evidence,
/// ELF/AMDHSA allocations and output bytes keep their existing bounded domains.
pub const MIXED_WORKER_FINALIZATION_STORAGE_LIMIT_V53: usize =
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53 + 64 * 1024;

/// Exact accounting or structural refusal; neither variant grants authority.
#[derive(Debug)]
pub enum MixedWorkerFinalizationBudgetErrorV53 {
    /// Reservation, prior denial, work debit or accounting failure.
    Resource(Resource),
    /// Unchanged raw-artifact reconstruction failure, including nested denial.
    Artifact(crate::NominalFinalizationErrorV53<Resource>),
    /// Unchanged strict finalization failure, including nested work denial.
    Finalization(NominalWorkerFinalizationErrorV53<Resource>),
}
impl From<Resource> for MixedWorkerFinalizationBudgetErrorV53 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for MixedWorkerFinalizationBudgetErrorV53 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => write!(f, "mixed finalization resources refused: {error}"),
            Self::Artifact(error) => error.fmt(f),
            Self::Finalization(error) => error.fmt(f),
        }
    }
}
impl Error for MixedWorkerFinalizationBudgetErrorV53 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Resource(error) => error,
            Self::Artifact(error) => error,
            Self::Finalization(error) => error,
        })
    }
}
type Failure = MixedWorkerFinalizationBudgetErrorV53;
type Work<'a> = dyn FnMut(usize) -> Result<(), Resource> + 'a;

fn scope<T, F>(budget: &mut Budget<'_>, run: F) -> Result<T, Failure>
where
    F: FnOnce(usize, &mut Work<'_>) -> Result<T, Failure>,
{
    // Only a work callback is lent out. The callee cannot release the scratch,
    // replace the ledger, or turn a declared capacity into a reservation.
    budget.check_prior_denials_v1()?;
    let frame = NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53
        .checked_add(size_of::<F>())
        .and_then(|n| n.checked_add(size_of::<Budget<'_>>()))
        .and_then(|n| n.checked_add(size_of::<&mut Work<'_>>()))
        .and_then(|n| n.checked_add(3 * size_of::<usize>()))
        .and_then(|n| n.checked_add(size_of::<T>()))
        .and_then(|n| n.checked_add(2 * size_of::<Result<T, Failure>>()))
        .and_then(|n| {
            n.checked_add(size_of::<
                Result<Result<T, Failure>, Box<dyn std::any::Any + Send>>,
            >())
        })
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(budget.storage(), 1, 1, frame, |budget| {
        run(NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53, &mut |n| {
            budget.charge_work(n)
        })
    })
}

/// Finalize the complete V53 artifact using real scratch reservation and work
/// debit on the caller's finite ledger. No synthetic prepaid capacity or free
/// callback is accepted by this entry. The original input is never mutated.
///
/// Scratch is restored on success, error and unwind; work/peak/first-denial
/// history remains. Returned artifact/evidence allocations are in the existing
/// bounded protocol domain, not charged as descriptor scratch or source IR.
pub fn finalize_protected_worker_nominal_hsaco_on_budget_v53(
    source: InertProtectedFirstBuildWorkerV3EvidenceV1,
    budget: &mut Budget<'_>,
) -> Result<PreparedFinalizedNominalWorkerHsacoV53, Failure> {
    scope(budget, |scratch, work| {
        finalize_protected_worker_nominal_hsaco_v53(source, scratch, &mut |n| work(n))
            .map_err(Failure::Finalization)
    })
}

/// Reconstruct exact raw V53 bytes with real reservation and cumulative debit
/// before descriptor inspection or cloning. The finalized input is immutable;
/// only the declared code-digest field is cleared in the new bounded output.
/// Returned bytes remain in the separate existing artifact allocation domain.
pub fn derive_unfinalized_nominal_hsaco_on_budget_v53(
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<Vec<u8>, Failure> {
    scope(budget, |scratch, work| {
        derive_unfinalized_nominal_hsaco_v53(bytes, scratch, &mut |n| work(n))
            .map_err(Failure::Artifact)
    })
}

pub(crate) fn derive_raw_default(bytes: &[u8]) -> Result<Vec<u8>, Failure> {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
        MIXED_WORKER_FINALIZATION_WORK_LIMIT_V53,
    );
    let mut budget = Budget::new(&mut work, MIXED_WORKER_FINALIZATION_STORAGE_LIMIT_V53);
    derive_unfinalized_nominal_hsaco_on_budget_v53(bytes, &mut budget)
}

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
