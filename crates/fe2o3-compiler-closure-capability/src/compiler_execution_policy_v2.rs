use crate::{
    native_capability::{NativeCapability, Record, Result, Storage},
    sealed_image::CapabilityRole,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V2 as BYTES, CompilerExecutionIssuerPolicyV2 as Policy,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fs::File, os::fd::RawFd};

type Capability = NativeCapability<Policy, BYTES>;
impl Record<BYTES> for Policy {
    const ROLE: CapabilityRole = CapabilityRole {
        name: "native compiler-execution policy capability",
        memfd_name: "fe2o3-compiler-execution-policy-v2",
    };
    fn bytes(&self) -> &[u8; BYTES] {
        self.canonical_bytes()
    }
    fn retained_storage(&self) -> usize {
        self.retained_storage()
    }
    fn decode_retained(bytes: &[u8; BYTES], budget: &mut Budget<'_>) -> Result<Self> {
        let (policy, storage) = Self::decode(bytes, budget)?;
        budget.reserve_storage(storage.additional_storage())?;
        Ok(policy)
    }
}

/// Move-only sealed native policy. No V1 decode, key custody, process launch,
/// execution, or load authority. Callers must pin the policy independently.
///
/// Every operation restores entry storage. Inputs remain prepaid on the same
/// ledger. Reserve each returned delta before retaining its owner; release the
/// full retained charge only after drop/transfer. On a consuming error, retire
/// the consumed input's reservation after this call returns.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV2;
/// fn duplicate(value: CompilerExecutionPolicyCapabilityV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<CompilerExecutionPolicyCapabilityV2>();
/// ```
pub struct CompilerExecutionPolicyCapabilityV2(Capability);
impl CompilerExecutionPolicyCapabilityV2 {
    /// Outer logical work. `from_file`/`from_inherited_at` additionally charge
    /// the native policy decoder on this same ledger. Not an instruction bound.
    pub const IO_WORK: usize = Capability::IO_WORK;
    /// Additional outer scratch; decoding separately reserves its own scratch
    /// and retained policy. Not heap, generated stack, wall-time, or RSS bounds.
    pub const IO_STORAGE: usize = Capability::IO_STORAGE;
    /// Full logical charge for an incoming/outgoing File and its sealed image.
    pub const FILE_STORAGE: usize = Capability::FILE_STORAGE;

    pub fn create(policy: Policy, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Capability::create(policy, budget).map(|(value, storage)| (Self(value), storage))
    }
    /// Consumes a prepaid File and returns only the retained-storage delta.
    pub fn from_file(image: File, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Capability::from_file(image, budget).map(|(value, storage)| (Self(value), storage))
    }
    /// Borrows a non-CLOEXEC inherited fd >= 3; never owns/closes it. The
    /// caller must keep that source live and prepaid at FILE_STORAGE throughout
    /// this call. The newly duplicated capability returns its FULL charge.
    /// No descriptor number is installed or reserved by this API.
    pub fn from_inherited_at(fd: RawFd, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Capability::from_inherited_at(fd, budget).map(|(value, storage)| (Self(value), storage))
    }
    pub const fn policy(&self) -> &Policy {
        &self.0.record
    }
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.0.revalidate(budget)
    }
    /// Returns a separately charged CLOEXEC File for one transport owner.
    pub fn try_clone_for_transfer(&self, budget: &mut Budget<'_>) -> Result<(File, Storage)> {
        self.0.try_clone_for_transfer(budget)
    }
    /// Revalidates this owner and a borrowed CLOEXEC transfer against the original
    /// sealed object and bytes. Prepay `retained_storage() + FILE_STORAGE` on
    /// the same ledger. Charges IO_WORK and IO_STORAGE scratch, restoring entry
    /// storage without creating, retaining, closing or moving either descriptor.
    pub fn validate_transfer(&self, transfer: &File, budget: &mut Budget<'_>) -> Result<()> {
        self.0.validate_transfer(transfer, budget)
    }
    pub const fn retained_storage(&self) -> usize {
        Capability::RETAINED
    }
}

const _: () = {
    Capability::assert_layout::<CompilerExecutionPolicyCapabilityV2>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_capability::{
        CompilerExecutionCapabilityErrorV2 as Error,
        tests::{failure, policy, run, sealed, transfer_boundaries},
    };
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn policy_transfer_borrows_exact_object_and_preserves_one_ledger_and_offset() {
        let policy = policy(7);
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(policy.retained_storage()).unwrap();
        let (cap, charge) =
            CompilerExecutionPolicyCapabilityV2::create(policy, &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let (transfer, charge) = cap.try_clone_for_transfer(&mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let floor = cap.retained_storage() + Capability::FILE_STORAGE;
        assert_eq!(budget.storage(), floor);
        rustix::fs::seek(&transfer, rustix::fs::SeekFrom::Start(17)).unwrap();
        cap.validate_transfer(&transfer, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), 3 * Capability::IO_WORK);
        assert_eq!(
            rustix::fs::seek(&transfer, rustix::fs::SeekFrom::Current(0)).unwrap(),
            17
        );
        assert_eq!(
            rustix::io::fcntl_getfd(&transfer).unwrap(),
            rustix::io::FdFlags::CLOEXEC
        );
        transfer_boundaries(floor, Capability::IO_WORK, Capability::IO_STORAGE, |b| {
            cap.validate_transfer(&transfer, b)
        });
        drop(transfer);
        budget.release_storage(Capability::FILE_STORAGE).unwrap();
        drop(cap);
        budget.release_storage(Capability::RETAINED).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn policy_transfer_rejects_an_identical_image_and_inheritable_alias() {
        let p = policy(7);
        let (cap, _) = run(p.retained_storage(), 1_000_000, 1_000_000, |b| {
            CompilerExecutionPolicyCapabilityV2::create(p, b)
        })
        .0
        .unwrap();
        let (alias, _) = run(cap.retained_storage(), 1_000_000, 1_000_000, |b| {
            cap.try_clone_for_transfer(b)
        })
        .0
        .unwrap();
        let replacement = sealed(cap.policy().canonical_bytes());
        let floor = cap.retained_storage() + 2 * Capability::FILE_STORAGE;
        let result = run(floor, Capability::IO_WORK, 1_000_000, |b| {
            cap.validate_transfer(&replacement, b)
        });
        assert!(matches!(
            failure(result.0),
            Error::Rejected("sealed image identity or length changed")
        ));
        assert_eq!(result.2, floor);
        rustix::io::fcntl_setfd(&alias, rustix::io::FdFlags::empty()).unwrap();
        let result = run(floor, Capability::IO_WORK, 1_000_000, |b| {
            cap.validate_transfer(&alias, b)
        });
        assert!(matches!(
            failure(result.0),
            Error::Rejected(" descriptor is unexpectedly inheritable")
        ));
        assert_eq!(result.2, floor);
        assert_eq!(
            rustix::io::fcntl_getfd(&alias).unwrap(),
            rustix::io::FdFlags::empty()
        );
        assert!(replacement.metadata().is_ok());
    }
}
