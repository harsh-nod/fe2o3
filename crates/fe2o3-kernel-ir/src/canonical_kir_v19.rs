//! Exact immutable V19 custody through the shared allocation-budgeted verifier.
//! No source authentication, V12 conversion, optimizer or final artifact authority.

use crate::canonical_kir_bounded_inverse_v1::{InverseError, Profile, verified_inverse};
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    KernelIrDecodeError, KernelIrEncodeError, MeteredKernelIrVerificationErrorV1, Module,
    VerificationErrors, VerifiedKernelIrModuleV1, verify_exact_decoded_module_with_budget_v1,
};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

pub const VERIFIED_CANONICAL_KERNEL_IR_V19_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V19\0";
pub const VERIFIED_CANONICAL_KERNEL_IR_V19_IDENTITY_POLICY_V1: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VerifiedCanonicalKernelIrIdentityV19 {
    digest: [u8; 32],
    canonical_length: u64,
}
impl VerifiedCanonicalKernelIrIdentityV19 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub const fn canonical_length(&self) -> u64 {
        self.canonical_length
    }
}

/// Read-only canonical bytes retained inseparably by the V19 module owner.
#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedCanonicalKernelIrV19 {
    canonical_bytes: Vec<u8>,
    identity: VerifiedCanonicalKernelIrIdentityV19,
}
impl VerifiedCanonicalKernelIrV19 {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV19 {
        &self.identity
    }
}

/// Move-only custody of one exact V19 encoding and the actual freshly decoded,
/// semantically verified Module. Only complete budgeted admission constructs it.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV19;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<VerifiedCanonicalKernelIrModuleV19>();
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV19;
/// fn mutate(owner: &mut VerifiedCanonicalKernelIrModuleV19) { owner.module().functions.clear(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV19;
/// fn separate(owner: VerifiedCanonicalKernelIrModuleV19) { let _ = owner.into_parts(); }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedCanonicalKernelIrModuleV19 {
    canonical: VerifiedCanonicalKernelIrV19,
    module: Module,
}

impl VerifiedCanonicalKernelIrModuleV19 {
    /// Fresh encode/inverse-decode/verification/equality/hash with one cumulative
    /// ledger. The borrowed source is not cloned or retained. All Result exits and unwinds
    /// restore the caller storage floor, preserving work/peak/failure history.
    /// Reserve the returned retained-payload receipt before later allocation
    /// while the owner lives. Payload accounting is conservative, not RSS.
    pub fn from_module_ref_with_verification_budget_v19(
        module: &Module,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(Self, CanonicalKernelIrReplayStorageV19), CanonicalKernelIrReplayAdmissionErrorV19>
    {
        let floor = budget.storage_checkpoint();
        budget.with_prepaid_scope(floor, 0, 0, 0, |budget| {
            let inverse_inline = std::mem::size_of::<Self>()
                .checked_sub(std::mem::size_of::<VerifiedCanonicalKernelIrV19>())
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
            let inverse = verified_inverse(
                module,
                budget,
                Profile::V19,
                std::mem::size_of::<VerifiedCanonicalKernelIrV19>(),
                inverse_inline,
            )
            .map_err(CanonicalKernelIrReplayAdmissionErrorV19::from_inverse)?;
            let identity = identity(&inverse.canonical_bytes, budget)?;
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
            Ok((
                Self {
                    canonical: VerifiedCanonicalKernelIrV19 {
                        canonical_bytes: inverse.canonical_bytes,
                        identity,
                    },
                    module: inverse.module,
                },
                CanonicalKernelIrReplayStorageV19 { retained },
            ))
        })
    }

    /// One exact allocation-metered decode and streaming canonical comparison;
    /// the same verified decoded Module is retained, never cloned/redecoded.
    /// Input bytes are caller-owned. Retained owner/receipt transfer together;
    /// floor restoration and cumulative work/peak match Module-source admission.
    pub fn from_canonical_bytes_with_verification_budget_v19(
        bytes: &[u8],
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(Self, CanonicalKernelIrReplayStorageV19), CanonicalKernelIrReplayAdmissionErrorV19>
    {
        let floor = budget.storage_checkpoint();
        budget.with_prepaid_scope(floor, 0, 0, 0, |budget| {
            budget.reserve_storage(std::mem::size_of::<Self>())?;
            let module = crate::wire::decode_module_v19_with_allocation_budget_v1(bytes, budget)
                .map_err(CanonicalKernelIrReplayAdmissionErrorV19::Decode)?;
            verify_exact_decoded_module_with_budget_v1(&module, None, budget).map_err(|error| {
                match error {
                    MeteredKernelIrVerificationErrorV1::Verification(error) => {
                        CanonicalKernelIrReplayAdmissionErrorV19::Verification(error)
                    }
                    MeteredKernelIrVerificationErrorV1::Resource(error) => {
                        CanonicalKernelIrReplayAdmissionErrorV19::Resource(error)
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
            let identity = identity(&canonical_bytes, budget)?;
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
            Ok((
                Self {
                    canonical: VerifiedCanonicalKernelIrV19 {
                        canonical_bytes,
                        identity,
                    },
                    module,
                },
                CanonicalKernelIrReplayStorageV19 { retained },
            ))
        })
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }
    pub const fn canonical(&self) -> &VerifiedCanonicalKernelIrV19 {
        &self.canonical
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        self.canonical.canonical_bytes()
    }
    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV19 {
        self.canonical.identity()
    }
    pub const fn verified_module_ref_v1(&self) -> VerifiedKernelIrModuleV1<'_> {
        VerifiedKernelIrModuleV1::new_verified_v1(&self.module)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKernelIrReplayStorageV19 {
    retained: usize,
}
impl CanonicalKernelIrReplayStorageV19 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

#[derive(Debug)]
pub enum CanonicalKernelIrReplayAdmissionErrorV19 {
    Encode(KernelIrEncodeError),
    Decode(KernelIrDecodeError),
    Verification(VerificationErrors),
    Resource(CanonicalKernelIrVerificationResourceErrorV1),
    CanonicalMismatch,
}
impl CanonicalKernelIrReplayAdmissionErrorV19 {
    fn from_inverse(error: InverseError) -> Self {
        match error {
            InverseError::Encode(error) => Self::Encode(error),
            InverseError::Decode(error) => Self::Decode(error),
            InverseError::Verification(error) => Self::Verification(error),
            InverseError::Resource(error) => Self::Resource(error),
            InverseError::CanonicalMismatch => Self::CanonicalMismatch,
        }
    }
}
impl From<CanonicalKernelIrVerificationResourceErrorV1>
    for CanonicalKernelIrReplayAdmissionErrorV19
{
    fn from(error: CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for CanonicalKernelIrReplayAdmissionErrorV19 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => write!(f, "V19 canonical encode failed: {error}"),
            Self::Decode(error) => write!(f, "V19 canonical decode failed: {error}"),
            Self::Verification(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
            Self::CanonicalMismatch => f.write_str("V19 inverse differs from its exact subject"),
        }
    }
}
impl Error for CanonicalKernelIrReplayAdmissionErrorV19 {}

fn identity(
    bytes: &[u8],
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<VerifiedCanonicalKernelIrIdentityV19, CanonicalKernelIrReplayAdmissionErrorV19> {
    let domain = VERIFIED_CANONICAL_KERNEL_IR_V19_IDENTITY_DOMAIN_V1;
    let length = u64::try_from(bytes.len())
        .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    let domain_length = u32::try_from(domain.len())
        .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    let work = bytes
        .len()
        .checked_add(domain.len())
        .and_then(|n| n.checked_add(4 + 2 + 8))
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    budget.charge_work(work)?;
    let mut hash = Sha256::new();
    hash.update(domain_length.to_le_bytes());
    hash.update(domain);
    hash.update(VERIFIED_CANONICAL_KERNEL_IR_V19_IDENTITY_POLICY_V1.to_le_bytes());
    hash.update(length.to_le_bytes());
    hash.update(bytes);
    Ok(VerifiedCanonicalKernelIrIdentityV19 {
        digest: hash.finalize().into(),
        canonical_length: length,
    })
}

#[cfg(test)]
#[path = "canonical_kir_v19_tests.rs"]
mod tests;
