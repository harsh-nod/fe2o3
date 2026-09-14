//! Direct native receipt admission retains the one freshly decoded executable.

use super::*;

impl VerifiedCanonicalKernelIrModuleV12 {
    /// Admits exact V12 bytes through one allocation-metered decode, streaming
    /// re-encoding comparison, fresh semantic verification and canonical hash.
    /// The single decoded module is retained directly; it is neither cloned
    /// nor decoded again through the Module-source admission path.
    ///
    /// Input bytes are borrowed and excluded from this logical payload ledger.
    /// The returned owner and receipt transfer together: reserve the receipt
    /// before any subsequent controlled allocation while the owner lives.
    /// All Result exits restore the incoming floor after scratch/failed owners
    /// drop. Accepted work, peak and first failure history remain unchanged.
    pub fn from_canonical_bytes_with_verification_budget_v12(
        bytes: &[u8],
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(Self, CanonicalKernelIrReplayStorageV12), AdmissionError> {
        let floor = budget.storage_checkpoint();
        let result = (|| {
            budget.reserve_storage(std::mem::size_of::<Self>())?;
            // This decoder admits only wire V12 and compares the entire
            // canonical re-encoding without allocating a second byte buffer.
            let module = crate::wire::decode_module_v12_with_allocation_budget_v1(bytes, budget)
                .map_err(AdmissionError::Decode)?;
            verify_exact_decoded_module_with_budget_v1(&module, None, budget).map_err(|error| {
                match error {
                    MeteredKernelIrVerificationErrorV1::Verification(error) => {
                        AdmissionError::Verification(error)
                    }
                    MeteredKernelIrVerificationErrorV1::Resource(error) => {
                        AdmissionError::Resource(error)
                    }
                }
            })?;
            budget.reserve_storage(bytes.len())?;
            budget.charge_work(bytes.len())?;
            let mut canonical_bytes = Vec::new();
            canonical_bytes
                .try_reserve_exact(bytes.len())
                .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Allocation)?;
            if canonical_bytes.capacity() != bytes.len() {
                return Err(CanonicalKernelIrVerificationResourceErrorV1::Allocation.into());
            }
            canonical_bytes.extend_from_slice(bytes);
            let canonical = VerifiedCanonicalKernelIrV12::from_validated_bytes_with_work_v1(
                canonical_bytes,
                budget.work_budget_v1(),
            )
            .map_err(AdmissionError::Canonical)?;
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
            Ok((
                Self { canonical, module },
                CanonicalKernelIrReplayStorageV12 { retained },
            ))
        })();
        budget.rollback_storage(floor)?;
        result
    }
}

#[cfg(test)]
#[path = "canonical_kir_v12_bytes_admission_tests.rs"]
mod tests;
