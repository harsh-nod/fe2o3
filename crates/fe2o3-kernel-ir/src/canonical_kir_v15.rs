//! Exact V15 canonical custody, without source, schedule or launch authority.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    KernelIrDecodeError, KernelIrEncodeError, MeteredKernelIrVerificationErrorV1, Module,
    VerificationErrors, verify_exact_decoded_module_with_budget_v1,
};

const IDENTITY_DOMAIN: &[u8] = b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V15\0";
type Budget<'a> = CanonicalKernelIrVerificationResourceBudgetV1<'a>;
type ResourceError = CanonicalKernelIrVerificationResourceErrorV1;
type AdmissionError = CanonicalKernelIrReplayAdmissionErrorV15;

/// Identity of exact V15 bytes admitted by the full semantic/lifecycle verifier.
/// The SHA256 preimage is domain length (u32 LE), domain, canonical length
/// (u64 LE), then the complete encoding. This is distinct from V12 identities.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VerifiedCanonicalKernelIrIdentityV15 {
    digest: [u8; 32],
    canonical_length: u64,
}

impl VerifiedCanonicalKernelIrIdentityV15 {
    /// Borrows the domain-bound SHA256 digest of the admitted V15 encoding.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Returns the exact admitted encoding length in bytes.
    pub const fn canonical_length(&self) -> u64 {
        self.canonical_length
    }
}

/// Retained owner header, exact wire buffer and fresh decoded payload.
/// Decoder B-tree bounds remain conservative; this is not an RSS measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKernelIrReplayStorageV15 {
    retained: usize,
}

impl CanonicalKernelIrReplayStorageV15 {
    /// Reserve this transfer before another allocation while the owner lives.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Mutable candidate header and decoded payload, excluding the original owner
/// and any allocations made by subsequent transformations. Not verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKernelIrCandidateStorageV15 {
    retained: usize,
}

impl CanonicalKernelIrCandidateStorageV15 {
    /// Reserve this transfer before another allocation while the candidate lives.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Typed failure of exact V15 canonical admission or unverified candidate copy.
#[derive(Debug)]
pub enum CanonicalKernelIrReplayAdmissionErrorV15 {
    /// Full wire encoding or its cumulative work charge failed.
    Encode(KernelIrEncodeError),
    /// Exact wire decoding, inverse re-encoding or allocation admission failed.
    Decode(KernelIrDecodeError),
    /// Full semantic or execution lifecycle verification rejected the inverse.
    Verification(VerificationErrors),
    /// Shared work, storage, allocation or checked arithmetic failed.
    Resource(CanonicalKernelIrVerificationResourceErrorV1),
    /// The encoded extent or freshly decoded module differs from its subject.
    CanonicalMismatch,
}

impl From<ResourceError> for AdmissionError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => write!(formatter, "canonical V15 encoding failed: {error}"),
            Self::Decode(error) => write!(formatter, "canonical V15 decoding failed: {error}"),
            Self::Verification(error) => {
                write!(formatter, "canonical V15 verification failed: {error}")
            }
            Self::Resource(error) => error.fmt(formatter),
            Self::CanonicalMismatch => {
                formatter.write_str("canonical V15 inverse differs from its exact subject")
            }
        }
    }
}

impl Error for AdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Verification(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::CanonicalMismatch => None,
        }
    }
}

/// Move-only immutable custody of exact V15 bytes and their verified inverse.
/// This owner does not establish source correspondence, schedule correctness,
/// execution lowering, target support, artifact admission or launch authority.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV15;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<VerifiedCanonicalKernelIrModuleV15>();
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV15;
/// fn mutate(owner: &mut VerifiedCanonicalKernelIrModuleV15) {
///     owner.module().functions.clear();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV15;
/// fn mutate_bytes(owner: &mut VerifiedCanonicalKernelIrModuleV15) {
///     owner.canonical_bytes()[0] = 0;
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV15;
/// fn separate(owner: VerifiedCanonicalKernelIrModuleV15) {
///     let _ = owner.into_parts();
/// }
/// ```
#[derive(Debug)]
pub struct VerifiedCanonicalKernelIrModuleV15 {
    module: Module,
    canonical_bytes: Vec<u8>,
    identity: VerifiedCanonicalKernelIrIdentityV15,
}

impl VerifiedCanonicalKernelIrModuleV15 {
    /// Full encode, fresh allocation-metered exact decode, semantic/lifecycle
    /// verification, inverse equality and hashing under one cumulative ledger.
    ///
    /// The input reservation stays caller-owned and live throughout. Every
    /// Result path restores the incoming storage floor without refunding work,
    /// peak or first-denial history. On success, reserve the returned receipt
    /// before further allocation. Failed owner payloads are dropped before
    /// restoration; returned verifier diagnostics transfer to the caller and
    /// are included in the observed peak. No unwind-cleanup guarantee is made.
    pub fn from_module_ref_with_verification_budget_v15(
        module: &Module,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(Self, CanonicalKernelIrReplayStorageV15), CanonicalKernelIrReplayAdmissionErrorV15>
    {
        let floor = budget.storage_checkpoint();
        let result = admit(module, budget);
        budget.rollback_storage(floor)?;
        result
    }

    /// Borrows the immutable, freshly decoded and verified V15 module.
    pub const fn module(&self) -> &Module {
        &self.module
    }

    /// Borrows the complete exact V15 encoding owned alongside its inverse.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Borrows the distinct domain-bound identity minted during admission.
    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV15 {
        &self.identity
    }

    /// Compares a candidate's complete V15 encoding with this immutable owner.
    ///
    /// This read-only query creates no graph, wire buffer or verified owner. A
    /// match establishes exact encoding equality, not source correspondence,
    /// rewrite validity or launch authority. Keep both inputs reserved while
    /// they live. Only temporary encoder scratch enters this query's ledger.
    /// Work, peak and denial history are cumulative; temporary storage returns
    /// to the incoming floor on success, failure or unwind. Encoding errors
    /// retain precedence over an earlier byte mismatch.
    pub fn matches_module_with_budget_v15(
        &self,
        candidate: &Module,
        budget: &mut Budget<'_>,
    ) -> Result<bool, AdmissionError> {
        let floor = budget.storage_checkpoint();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (_, auxiliary) = crate::wire::count_module_v15_wire_extent_with_work_v1(
                candidate,
                budget.work_budget_v1(),
            )
            .map_err(AdmissionError::Encode)?;
            let scratch = crate::wire::decoded_tree_payload_bound_v12::<&crate::FunctionId>(
                candidate.kernels.len(),
            )
            .map_err(AdmissionError::Decode)?
            .checked_add(auxiliary)
            .ok_or(ResourceError::Arithmetic)?;
            budget.reserve_storage(scratch)?;
            crate::wire::compare_module_encoding_v1(
                candidate,
                crate::KERNEL_IR_VERSION_V15,
                &self.canonical_bytes,
                Some(budget.work_budget_v1()),
            )
            .map_err(AdmissionError::Encode)
        }));
        budget.rollback_storage(floor)?;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Freshly decodes an independent, ordinary mutable Module and checks full
    /// equality with this owner. It grants no verification or rewrite authority;
    /// a transformed candidate requires fresh admission and separate replay.
    ///
    /// Keep this owner's reservation live. Every Result path restores that
    /// incoming floor while preserving work/peak/denial history. Reserve the
    /// returned candidate receipt before further allocation; later mutation
    /// allocations require their own accounting. No second wire buffer is made.
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::{Module, VerifiedCanonicalKernelIrModuleV15};
    /// fn verified(candidate: Module) -> VerifiedCanonicalKernelIrModuleV15 {
    ///     candidate
    /// }
    /// ```
    pub fn copy_module_for_transformation_v15(
        &self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        (Module, CanonicalKernelIrCandidateStorageV15),
        CanonicalKernelIrReplayAdmissionErrorV15,
    > {
        let floor = budget.storage_checkpoint();
        let result = (|| {
            budget.reserve_storage(std::mem::size_of::<Module>())?;
            let module = crate::wire::decode_module_v15_with_allocation_budget_v1(
                &self.canonical_bytes,
                budget,
            )
            .map_err(AdmissionError::Decode)?;
            budget.charge_work(self.canonical_bytes.len())?;
            if module != self.module {
                return Err(AdmissionError::CanonicalMismatch);
            }
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or(ResourceError::Accounting)?;
            Ok((module, CanonicalKernelIrCandidateStorageV15 { retained }))
        })();
        budget.rollback_storage(floor)?;
        result
    }
}

fn admit(
    module: &Module,
    budget: &mut Budget<'_>,
) -> Result<
    (
        VerifiedCanonicalKernelIrModuleV15,
        CanonicalKernelIrReplayStorageV15,
    ),
    AdmissionError,
> {
    let floor = budget.storage_checkpoint();
    let (wire_bytes, auxiliary_bytes) =
        crate::wire::count_module_v15_wire_extent_with_work_v1(module, budget.work_budget_v1())
            .map_err(AdmissionError::Encode)?;
    budget.reserve_storage(
        std::mem::size_of::<VerifiedCanonicalKernelIrModuleV15>()
            .checked_add(wire_bytes)
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    let scratch =
        crate::wire::decoded_tree_payload_bound_v12::<&crate::FunctionId>(module.kernels.len())
            .map_err(AdmissionError::Decode)?
            .checked_add(auxiliary_bytes)
            .ok_or(ResourceError::Arithmetic)?;
    budget.reserve_storage(scratch)?;
    let canonical_bytes =
        crate::wire::encode_module_v15_with_work_v1(module, budget.work_budget_v1())
            .map_err(AdmissionError::Encode)?;
    budget.release_storage(scratch)?;
    if canonical_bytes.len() != wire_bytes || canonical_bytes.capacity() != wire_bytes {
        return Err(AdmissionError::CanonicalMismatch);
    }
    let decoded =
        crate::wire::decode_module_v15_with_allocation_budget_v1(&canonical_bytes, budget)
            .map_err(AdmissionError::Decode)?;
    verify_exact_decoded_module_with_budget_v1(&decoded, None, budget).map_err(
        |error| match error {
            MeteredKernelIrVerificationErrorV1::Verification(error) => {
                AdmissionError::Verification(error)
            }
            MeteredKernelIrVerificationErrorV1::Resource(error) => AdmissionError::Resource(error),
        },
    )?;
    // The injective full encoding bounds both structural equality and hashing.
    budget.charge_work(wire_bytes)?;
    if &decoded != module {
        return Err(AdmissionError::CanonicalMismatch);
    }
    let canonical_length = u64::try_from(wire_bytes).map_err(|_| ResourceError::Arithmetic)?;
    let domain_length =
        u32::try_from(IDENTITY_DOMAIN.len()).map_err(|_| ResourceError::Arithmetic)?;
    let hash_work = wire_bytes
        .checked_add(4 + IDENTITY_DOMAIN.len() + 8)
        .ok_or(ResourceError::Arithmetic)?;
    budget.charge_work(hash_work)?;
    let mut hash = Sha256::new();
    hash.update(domain_length.to_le_bytes());
    hash.update(IDENTITY_DOMAIN);
    hash.update(canonical_length.to_le_bytes());
    hash.update(&canonical_bytes);
    let identity = VerifiedCanonicalKernelIrIdentityV15 {
        digest: hash.finalize().into(),
        canonical_length,
    };
    let retained = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ResourceError::Accounting)?;
    Ok((
        VerifiedCanonicalKernelIrModuleV15 {
            module: decoded,
            canonical_bytes,
            identity,
        },
        CanonicalKernelIrReplayStorageV15 { retained },
    ))
}

#[cfg(test)]
#[path = "canonical_kir_v15_tests.rs"]
mod tests;
