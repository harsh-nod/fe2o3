//! Native launch-input custody, not activation of the protected issuer service.
use crate::{
    COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1 as LAUNCH_FD,
    COMPILER_EXECUTION_ISSUER_POLICY_FD_V1 as POLICY_FD,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2 as CapabilityError,
    CompilerExecutionCapabilityStorageV2 as CapabilityStorage,
    CompilerExecutionPolicyCapabilityV2 as PolicyCapability,
    CompilerExecutionServiceLaunchCapabilityV2 as LaunchCapability,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V2 as POLICY_BYTES,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V2 as MANIFEST_BYTES,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionServiceLaunchManifestErrorV2 as ManifestError,
    CompilerExecutionServiceLaunchManifestV2 as Manifest,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of, os::fd::RawFd};

const ENTRY_WORK: usize = 8;
const RETAINED: usize = size_of::<(PolicyCapability, CapabilityStorage)>()
    + POLICY_BYTES
    + size_of::<(LaunchCapability, CapabilityStorage)>()
    + MANIFEST_BYTES;

/// Inert, move-only native inputs freshly admitted from fixed launch slots 6/8.
/// The two independently decoded images must name the same native policy.
/// No legacy-policy fallback or conversion of an admitted legacy owner exists.
/// Agreement does not independently pin policy provenance: a consistently
/// replaced pair also agrees. Trusted installation and program admission must
/// supply that pin before service activation.
///
/// This does not authenticate the supervisor, client, anchor, executable, key,
/// process profile or durable state. It cannot sign, publish readiness, serve,
/// load, or launch a kernel. The production V1 serving entrypoint is unchanged.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_issuer::CompilerExecutionIssuerLaunchInputsV2;
/// fn duplicate(value: CompilerExecutionIssuerLaunchInputsV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_issuer::CompilerExecutionIssuerLaunchInputsV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<CompilerExecutionIssuerLaunchInputsV2>();
/// ```
pub struct CompilerExecutionIssuerLaunchInputsV2 {
    policy: PolicyCapability,
    launch: LaunchCapability,
}

/// Unreserved full retained charge for the two newly duplicated input owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionIssuerLaunchInputStorageV2(usize);
impl CompilerExecutionIssuerLaunchInputStorageV2 {
    /// Reserve this charge on the same ledger before retaining the result.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

impl CompilerExecutionIssuerLaunchInputsV2 {
    /// Prepaid borrowed File/image charge for fixed slots 6 and 8.
    pub const INPUT_STORAGE: usize =
        PolicyCapability::FILE_STORAGE + LaunchCapability::FILE_STORAGE;
    /// Additional outer logical frame. Nested capability and codec operations
    /// charge separately on the same ledger. Not allocator, RSS or stack bounds.
    pub const FRAME_STORAGE: usize = 4 * RETAINED + 4096;

    /// Borrows the fixed non-CLOEXEC input slots and retains private CLOEXEC
    /// duplicates. Never closes the source slots, including on refusal.
    /// The caller must keep them live, stable and prepaid at INPUT_STORAGE.
    /// All operations restore entry storage; accepted work is never refunded.
    pub fn from_inherited(
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CompilerExecutionIssuerLaunchInputStorageV2)> {
        Self::read_at(POLICY_FD, LAUNCH_FD, budget)
    }

    fn read_at(
        policy_fd: RawFd,
        launch_fd: RawFd,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CompilerExecutionIssuerLaunchInputStorageV2)> {
        budget.with_prepaid_scope(
            Self::INPUT_STORAGE,
            ENTRY_WORK,
            ENTRY_WORK,
            Self::FRAME_STORAGE,
            |budget| {
                let (policy, charge) = PolicyCapability::from_inherited_at(policy_fd, budget)?;
                budget.reserve_storage(charge.additional_storage())?;
                let (launch, charge) = LaunchCapability::from_inherited_at(launch_fd, budget)?;
                budget.reserve_storage(charge.additional_storage())?;
                let inputs = Self { policy, launch };
                inputs.check(budget)?;
                Ok((
                    inputs,
                    CompilerExecutionIssuerLaunchInputStorageV2(RETAINED),
                ))
            },
        )
    }

    /// Rechecks both retained kernel objects, exact bytes and their native join.
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        budget.with_prepaid_scope(
            RETAINED,
            ENTRY_WORK,
            ENTRY_WORK,
            Self::FRAME_STORAGE,
            |budget| self.check(budget),
        )
    }

    fn check(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.policy.revalidate(budget)?;
        self.launch.revalidate(budget)?;
        if !self
            .launch
            .manifest()
            .matches_policy(self.policy.policy(), budget)?
        {
            return Err(CompilerExecutionIssuerLaunchInputErrorV2::PolicyMismatch);
        }
        Ok(())
    }

    /// Returns the separately admitted native policy, not signing authority.
    pub const fn policy(&self) -> &Policy {
        self.policy.policy()
    }
    /// Returns the policy-matched inert launch binding.
    pub const fn manifest(&self) -> &Manifest {
        self.launch.manifest()
    }
    /// Full retained logical charge; retire it only after dropping this owner.
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
}

impl fmt::Debug for CompilerExecutionIssuerLaunchInputsV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionIssuerLaunchInputsV2")
            .field("authority", &"none")
            .field("policy", &self.policy().identity())
            .field("manifest", &self.manifest().identity())
            .finish_non_exhaustive()
    }
}

/// Bounded native launch-input diagnostic, without supplied paths or strings.
#[derive(Debug)]
pub enum CompilerExecutionIssuerLaunchInputErrorV2 {
    /// Shared ledger refused this operation.
    Resource(Resource),
    /// A sealed capability could not be independently admitted or revalidated.
    Capability(CapabilityError),
    /// Native manifest comparison was refused.
    Manifest(ManifestError),
    /// The manifest does not name the separately admitted native policy.
    PolicyMismatch,
}
type Result<T> = std::result::Result<T, CompilerExecutionIssuerLaunchInputErrorV2>;
impl From<Resource> for CompilerExecutionIssuerLaunchInputErrorV2 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<CapabilityError> for CompilerExecutionIssuerLaunchInputErrorV2 {
    fn from(e: CapabilityError) -> Self {
        Self::Capability(e)
    }
}
impl From<ManifestError> for CompilerExecutionIssuerLaunchInputErrorV2 {
    fn from(e: ManifestError) -> Self {
        Self::Manifest(e)
    }
}
impl fmt::Display for CompilerExecutionIssuerLaunchInputErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Capability(e) => e.fmt(f),
            Self::Manifest(e) => e.fmt(f),
            Self::PolicyMismatch => f.write_str("native issuer launch policy mismatch"),
        }
    }
}
impl Error for CompilerExecutionIssuerLaunchInputErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Capability(e) => Some(e),
            Self::Manifest(e) => Some(e),
            Self::PolicyMismatch => None,
        }
    }
}

const _: () = {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as LedgerIdentity;
    const fn envelopes<T>() -> usize {
        size_of::<Result<T>>().saturating_sub(size_of::<T>())
            + size_of::<std::thread::Result<Result<T>>>().saturating_sub(size_of::<T>())
    }
    assert!(
        envelopes::<(
            CompilerExecutionIssuerLaunchInputsV2,
            CompilerExecutionIssuerLaunchInputStorageV2
        )>() + envelopes::<()>()
            + size_of::<LedgerIdentity>()
            + size_of::<std::result::Result<(), Resource>>()
            + 2 * size_of::<bool>()
            <= 512
    );
    assert!(
        size_of::<(
            CompilerExecutionIssuerLaunchInputsV2,
            CompilerExecutionIssuerLaunchInputStorageV2
        )>() <= size_of::<(PolicyCapability, CapabilityStorage)>()
            + size_of::<(LaunchCapability, CapabilityStorage)>()
    );
    assert!(
        8 * size_of::<CompilerExecutionIssuerLaunchInputErrorV2>() + 64 * size_of::<usize>() + 512
            <= 4096
    );
};

#[cfg(test)]
#[path = "launch_inputs_v2_tests.rs"]
mod tests;
