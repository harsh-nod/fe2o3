//! Immutable canonical storage transport. No source/init/runtime authority.
use crate::{
    BorrowedKernelIrVerificationErrorV1, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError, KernelIrDecodeError,
    KernelIrEncodeError, Module, StorageLayoutErrorV1, StorageLayoutIdV1, StorageLayoutLimitsV1,
    VerifiedStorageKernelIrModuleV1,
};
use std::{error::Error, fmt};

#[path = "canonical_kir_v18_admission.rs"]
mod admission;
#[path = "canonical_kir_v18_resource.rs"]
mod resource;
#[path = "canonical_storage_table_identity_v18.rs"]
mod table_identity;
pub use table_identity::CanonicalStorageTableIdentityV18;

pub const VERIFIED_CANONICAL_KERNEL_IR_V18_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V18\0";
pub const VERIFIED_CANONICAL_KERNEL_IR_V18_IDENTITY_POLICY_V1: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VerifiedCanonicalKernelIrIdentityV18 {
    digest: [u8; 32],
    canonical_length: u64,
}
impl VerifiedCanonicalKernelIrIdentityV18 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub const fn canonical_length(&self) -> u64 {
        self.canonical_length
    }
}

/// Inert identity only; the exact owner borrow is still required for a row query.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalStorageLayoutIdentityV18 {
    module: VerifiedCanonicalKernelIrIdentityV18,
    row: StorageLayoutIdV1,
}
impl CanonicalStorageLayoutIdentityV18 {
    pub const fn module(&self) -> &VerifiedCanonicalKernelIrIdentityV18 {
        &self.module
    }
    pub const fn row(&self) -> StorageLayoutIdV1 {
        self.row
    }
}

/// Move-only custody of exact bytes and their actual decoded, checked Module.
///
/// This grants structural/CFG/type validity only. Source ABI, initialized bytes,
/// active variant, pointer provenance, lifetime, overlap and target/SIM support
/// require their own consuming joins. Equal row ordinals are not type identity.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18;
/// fn clone_required<T: Clone>() {}
/// clone_required::<VerifiedCanonicalKernelIrModuleV18>();
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18;
/// fn mutate(owner: &mut VerifiedCanonicalKernelIrModuleV18) {
///     owner.module().storage_layouts.clear();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV18, VerifiedKernelIrModuleV1};
/// fn legacy(owner: &VerifiedCanonicalKernelIrModuleV18) -> VerifiedKernelIrModuleV1<'_> {
///     owner.verified_storage_module_ref_v1()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18;
/// fn separate(owner: VerifiedCanonicalKernelIrModuleV18) { owner.into_parts(); }
/// ```
#[derive(Debug)]
pub struct VerifiedCanonicalKernelIrModuleV18 {
    module: Module,
    canonical_bytes: Vec<u8>,
    identity: VerifiedCanonicalKernelIrIdentityV18,
}

impl VerifiedCanonicalKernelIrModuleV18 {
    /// Source is borrowed, never cloned or retained. One encode/inverse decode
    /// and shared structural admission retain the actual freshly decoded graph.
    /// Return storage transfers by receipt; reserve it before further allocation
    /// while the owner lives. All failures/unwind drop temporary owners before
    /// restoring the caller floor, without resetting work/peak/first denials.
    pub fn from_module_ref_with_verification_budget_v18(
        module: &Module,
        limits: StorageLayoutLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKernelIrReplayStorageV18), CanonicalKernelIrReplayAdmissionErrorV18>
    {
        admission::from_module(module, limits, budget)
    }

    /// Decodes exactly V18, verifies the actual Module once, and retains it with
    /// an exact copy of the authenticated canonical bytes. No legacy fallback.
    pub fn from_canonical_bytes_with_verification_budget_v18(
        bytes: &[u8],
        limits: StorageLayoutLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKernelIrReplayStorageV18), CanonicalKernelIrReplayAdmissionErrorV18>
    {
        admission::from_bytes(bytes, limits, budget)
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV18 {
        &self.identity
    }

    /// Decodes an independent mutable candidate from this owner's exact V18 bytes.
    ///
    /// The shared bounded decoder preserves the full storage table, graph and
    /// explicit roles and checks their canonical roundtrip without another wire
    /// buffer. This creates no canonical owner, source or optimization authority.
    /// A transformed candidate requires fresh V18 admission and consuming replay.
    /// Keep this source owner's reservation while it lives; reserve the returned
    /// candidate receipt before further allocation, account mutation growth, and
    /// drop the candidate before refunding that reservation. The receipt is only
    /// a logical storage amount, not a source binding or ledger-owned permit.
    /// All results and unwind restore the incoming floor while retaining work,
    /// peak and first-denial history. Decode errors precede cleanup errors.
    pub fn copy_module_for_transformation_v18(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<
        (Module, CanonicalKernelIrCandidateStorageV18),
        CanonicalKernelIrReplayAdmissionErrorV18,
    > {
        let mut scope = resource::Scope::enter_copy(budget)?;
        let result = (|| {
            scope
                .budget
                .reserve_storage(std::mem::size_of::<Module>())?;
            #[cfg(test)]
            copy_tests::checkpoint(1);
            let module = crate::wire::decode_module_v18_with_allocation_budget_v1(
                &self.canonical_bytes,
                scope.budget,
            )
            .map_err(CanonicalKernelIrReplayAdmissionErrorV18::Decode)?;
            #[cfg(test)]
            copy_tests::checkpoint(2);
            let receipt = CanonicalKernelIrCandidateStorageV18 {
                retained: scope.retained()?,
            };
            Ok((module, receipt))
        })();
        #[cfg(test)]
        copy_tests::checkpoint(3);
        let released = scope.finish();
        match result {
            Err(error) => Err(error),
            Ok(candidate) => {
                released?;
                Ok(candidate)
            }
        }
    }

    /// Compares the complete V18 storage table and graph encoding with this owner.
    ///
    /// This read-only query neither admits nor retains the candidate and creates
    /// no module graph or full wire buffer. Equality grants no source, rewrite,
    /// initialization or runtime authority. Keep both inputs reserved while live.
    /// Only the V18 codec header envelope and counted temporary producer scratch
    /// enter this query's ledger; these are logical bounds, not RSS measurements.
    /// Work, peak and first denials remain cumulative. Success, failure and unwind
    /// restore the incoming storage floor. Encoding errors take precedence over
    /// any earlier byte mismatch; a different encodable candidate returns false.
    pub fn matches_module_with_budget_v18(
        &self,
        candidate: &Module,
        budget: &mut Budget<'_>,
    ) -> Result<bool, CanonicalKernelIrReplayAdmissionErrorV18> {
        let mut scope = ComparisonScopeV18::enter(budget)?;
        let result = (|| {
            #[cfg(test)]
            comparison_tests::checkpoint(1);
            let extent = crate::wire::count_module_with_work_v1(
                candidate,
                crate::KERNEL_IR_VERSION_V18,
                scope.budget.work_budget_v1(),
                false,
            )
            .map_err(CanonicalKernelIrReplayAdmissionErrorV18::Encode)?;
            scope
                .budget
                .reserve_storage(extent.peak_auxiliary_bytes())?;
            #[cfg(test)]
            comparison_tests::checkpoint(2);
            let result = crate::wire::compare_module_encoding_v1(
                candidate,
                crate::KERNEL_IR_VERSION_V18,
                &self.canonical_bytes,
                Some(scope.budget.work_budget_v1()),
            )
            .map_err(CanonicalKernelIrReplayAdmissionErrorV18::Encode);
            #[cfg(test)]
            comparison_tests::checkpoint(3);
            result
        })();
        let released = scope.finish();
        match result {
            Err(error) => Err(error),
            Ok(matches) => {
                released?;
                Ok(matches)
            }
        }
    }

    pub fn verified_storage_module_ref_v1(&self) -> VerifiedStorageKernelIrModuleV1<'_> {
        VerifiedStorageKernelIrModuleV1::from_canonical_storage_owner_v18(self)
    }
    pub fn layout_identity(
        &self,
        row: StorageLayoutIdV1,
    ) -> Option<CanonicalStorageLayoutIdentityV18> {
        self.module.storage_layouts.get(row.0 as usize)?;
        Some(CanonicalStorageLayoutIdentityV18 {
            module: self.identity,
            row,
        })
    }
}

struct ComparisonScopeV18<'a, 'work> {
    budget: &'a mut Budget<'work>,
    floor: Option<usize>,
}
impl<'a, 'work> ComparisonScopeV18<'a, 'work> {
    fn enter(budget: &'a mut Budget<'work>) -> Result<Self, ResourceError> {
        budget.charge_work(1)?;
        let floor = budget.storage_checkpoint();
        budget.reserve_storage(Self::headers()?)?;
        Ok(Self {
            budget,
            floor: Some(floor),
        })
    }

    fn headers() -> Result<usize, ResourceError> {
        let slots = [
            (1, std::mem::size_of::<Self>()),
            (2, std::mem::size_of::<Result<Self, ResourceError>>()),
            // Public method, closure, stored outcome and caller result.
            (
                4,
                std::mem::size_of::<Result<bool, CanonicalKernelIrReplayAdmissionErrorV18>>(),
            ),
            (2, std::mem::size_of::<Result<(), ResourceError>>()),
            (2, std::mem::size_of::<Result<usize, ResourceError>>()),
        ];
        slots.into_iter().try_fold(
            crate::wire::storage_codec_headers_v18()?,
            |sum, (count, size)| {
                size.checked_mul(count)
                    .and_then(|n| sum.checked_add(n))
                    .ok_or(ResourceError::Arithmetic)
            },
        )
    }

    fn finish(&mut self) -> Result<(), ResourceError> {
        match self.floor.take() {
            Some(floor) => self.budget.rollback_storage(floor),
            None => Ok(()),
        }
    }
}
impl Drop for ComparisonScopeV18<'_, '_> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKernelIrReplayStorageV18 {
    retained: usize,
}
impl CanonicalKernelIrReplayStorageV18 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Storage transferred with one ordinary mutable candidate, not verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKernelIrCandidateStorageV18 {
    retained: usize,
}
impl CanonicalKernelIrCandidateStorageV18 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

#[derive(Debug)]
pub enum CanonicalKernelIrReplayAdmissionErrorV18 {
    Encode(KernelIrEncodeError),
    Decode(KernelIrDecodeError),
    Layout(StorageLayoutErrorV1),
    Verification(BorrowedKernelIrVerificationErrorV1),
    Resource(ResourceError),
}
impl From<ResourceError> for CanonicalKernelIrReplayAdmissionErrorV18 {
    fn from(value: ResourceError) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for CanonicalKernelIrReplayAdmissionErrorV18 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => write!(f, "V18 canonical encode failed: {error}"),
            Self::Decode(error) => write!(f, "V18 canonical decode failed: {error}"),
            Self::Layout(error) => error.fmt(f),
            Self::Verification(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
        }
    }
}
impl Error for CanonicalKernelIrReplayAdmissionErrorV18 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Layout(error) => Some(error),
            Self::Verification(error) => Some(error),
            Self::Resource(error) => Some(error),
        }
    }
}

#[cfg(test)]
#[path = "canonical_kir_v18_resource_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "canonical_kir_v18_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "canonical_kir_v18_comparison_tests.rs"]
mod comparison_tests;

#[cfg(test)]
#[path = "canonical_kir_v18_copy_tests.rs"]
mod copy_tests;
