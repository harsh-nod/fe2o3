//! Shared accounting, with concrete descriptor and Worker types per wire family.

macro_rules! mixed_worker_resources_family {
    ($scratch:ident, $worker_error:ident, $owner:ident, $derive:ident,
     $finalize:ident, $work_limit:ident, $storage_limit:ident, $error:ident,
     $artifact_error:ident, $finalize_paid:ident, $derive_paid:ident) => {
        use crate::{
            InertProtectedFirstBuildWorkerV3EvidenceV1, $derive, $finalize, $owner, $scratch,
            $worker_error,
        };
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        };
        use std::{error::Error, fmt, mem::size_of};

        /// Finite ordinary Cargo descriptor-finalization work ceiling. This is not an
        /// admission promise for every artifact below the independent format ceilings.
        pub const $work_limit: usize = 1_000_000_000;
        /// Finite ordinary Cargo descriptor scratch/header ceiling. Retained evidence,
        /// ELF/AMDHSA allocations and output bytes keep their existing bounded domains.
        pub const $storage_limit: usize = $scratch + 64 * 1024;

        /// Exact accounting or structural refusal; neither variant grants authority.
        #[derive(Debug)]
        pub enum $error {
            /// Reservation, prior denial, work debit or accounting failure.
            Resource(Resource),
            /// Unchanged raw-artifact reconstruction failure, including nested denial.
            Artifact(crate::$artifact_error<Resource>),
            /// Unchanged strict finalization failure, including nested work denial.
            Finalization($worker_error<Resource>),
        }
        impl From<Resource> for $error {
            fn from(error: Resource) -> Self {
                Self::Resource(error)
            }
        }
        impl fmt::Display for $error {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Resource(error) => {
                        write!(f, "mixed finalization resources refused: {error}")
                    }
                    Self::Artifact(error) => error.fmt(f),
                    Self::Finalization(error) => error.fmt(f),
                }
            }
        }
        impl Error for $error {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                Some(match self {
                    Self::Resource(error) => error,
                    Self::Artifact(error) => error,
                    Self::Finalization(error) => error,
                })
            }
        }
        type Failure = $error;
        type Work<'a> = dyn FnMut(usize) -> Result<(), Resource> + 'a;

        fn scope<T, F>(budget: &mut Budget<'_>, run: F) -> Result<T, Failure>
        where
            F: FnOnce(usize, &mut Work<'_>) -> Result<T, Failure>,
        {
            // Only a work callback is lent out. The callee cannot release the scratch,
            // replace the ledger, or turn a declared capacity into a reservation.
            budget.check_prior_denials_v1()?;
            let frame = $scratch
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
                run($scratch, &mut |n| budget.charge_work(n))
            })
        }

        /// Finalize the complete family-specific artifact using real scratch reservation and work
        /// debit on the caller's finite ledger. No synthetic prepaid capacity or free
        /// callback is accepted by this entry. The original input is never mutated.
        ///
        /// Scratch is restored on success, error and unwind; work/peak/first-denial
        /// history remains. Returned artifact/evidence allocations are in the existing
        /// bounded protocol domain, not charged as descriptor scratch or source IR.
        pub fn $finalize_paid(
            source: InertProtectedFirstBuildWorkerV3EvidenceV1,
            budget: &mut Budget<'_>,
        ) -> Result<$owner, Failure> {
            scope(budget, |scratch, work| {
                $finalize(source, scratch, &mut |n| work(n)).map_err(Failure::Finalization)
            })
        }

        /// Reconstruct exact raw family-specific bytes with real reservation and cumulative debit
        /// before descriptor inspection or cloning. The finalized input is immutable;
        /// only the declared code-digest field is cleared in the new bounded output.
        /// Returned bytes remain in the separate existing artifact allocation domain.
        pub fn $derive_paid(bytes: &[u8], budget: &mut Budget<'_>) -> Result<Vec<u8>, Failure> {
            scope(budget, |scratch, work| {
                $derive(bytes, scratch, &mut |n| work(n)).map_err(Failure::Artifact)
            })
        }

        pub(crate) fn derive_raw_default(bytes: &[u8]) -> Result<Vec<u8>, Failure> {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new($work_limit);
            let mut budget = Budget::new(&mut work, $storage_limit);
            $derive_paid(bytes, &mut budget)
        }
    };
}

pub(crate) use mixed_worker_resources_family;
