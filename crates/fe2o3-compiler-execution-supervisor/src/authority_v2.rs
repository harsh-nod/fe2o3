//! Native pre-session custody. Process confinement and consuming launch are separate.
use crate::{
    AdmittedIssuerProgramV2 as Program, IssuerProgramAdmissionErrorV2 as ProgramError,
    IssuerServiceCredentialProfileV1 as Credentials,
    root_checks::{self, RootCheckError, RootSnapshot},
};
use fe2o3_broker_authority_service::{
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor,
    ProtectedExternalAnchorServiceErrorV2 as AnchorError,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2 as KeyError, CompilerExecutionSigningKeyCapabilityV2 as Key,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as AnchorIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{
    error::Error,
    fmt,
    fs::File,
    mem::{align_of, size_of},
};

const ENTRY: usize = 8;
#[path = "authority_v2_launch.rs"]
pub(super) mod launch;
use ProtectedIssuerSupervisorStorageV2 as Storage;
type Result<T> = std::result::Result<T, ProtectedIssuerSupervisorErrorV2>;

/// Unreserved growth over the consumed program, key, anchor and root charges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedIssuerSupervisorStorageV2(usize);
impl Storage {
    /// Preserve input reservations and reserve this delta before retaining the result.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Move-only native program, policy-bound key, anchor, root and credential custody.
///
/// Trusted provisioning supplies every input. This checks the current effective
/// UID/GID, not the full child confinement profile. It does not authenticate a
/// compiler handoff, spawn a process, establish readiness, or grant GPU authority.
/// No V1 admitted owner is upgraded, and no descriptor or signing operation escapes.
///
/// All nested operations use the caller's ledger. Entry storage is restored on
/// success, refusal and unwind without refunding work. On consuming refusal,
/// inputs close before the caller retires their old reservations.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerSupervisorV2;
/// fn clone<T: Clone>() {}
/// clone::<ProtectedIssuerSupervisorV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerSupervisorV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<ProtectedIssuerSupervisorV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV1, ProtectedIssuerSupervisorV2};
/// fn upgrade(old: ProtectedIssuerSupervisorV1) -> ProtectedIssuerSupervisorV2 { old.into() }
/// ```
pub struct ProtectedIssuerSupervisorV2 {
    program: Program,
    credentials: Credentials,
    root: File,
    root_snapshot: RootSnapshot,
    signing_key: Key,
    external_anchor: Anchor,
    retained: usize,
}

impl ProtectedIssuerSupervisorV2 {
    /// Logical input charge for the consumed root File, including receipt padding.
    pub const ROOT_FILE_STORAGE: usize = size_of::<(File, Storage)>();
    /// Fixed ownership growth for the root snapshot, credentials and outer bookkeeping.
    pub const OWNER_GROWTH: usize =
        size_of::<(RootSnapshot, Credentials, usize, Storage)>() + align_of::<Self>();
    /// Outer logical allowance: entry plus 64 weighted credential/root/cleanup calls.
    /// Program, key and anchor operations additionally charge the same ledger.
    pub const WORK: usize = ENTRY + 64 * 1024;
    /// Outer logical staging/control scratch, excluding nested operations' scratch.
    /// This is not a bound on generated stack, RSS, syscall latency or kernel memory.
    pub const SCRATCH: usize = 4 * size_of::<(Self, Storage)>() + 4096;

    /// Consumes prepaid native inputs and root custody, returning only owner growth.
    /// Validation order is credentials, program, exact-policy key, anchor, root,
    /// then full revalidation. Equal public keys do not substitute policy identities.
    pub fn bind(
        program: Program,
        credentials: Credentials,
        root: File,
        signing_key: Key,
        external_anchor: Anchor,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        budget.charge_work(ENTRY)?;
        let floor = program
            .retained_storage()
            .checked_add(signing_key.retained_storage())
            .and_then(|n| n.checked_add(external_anchor.retained_storage()))
            .and_then(|n| n.checked_add(Self::ROOT_FILE_STORAGE))
            .ok_or(Resource::Arithmetic)?;
        let retained = floor
            .checked_add(Self::OWNER_GROWTH)
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, 0, Self::WORK - ENTRY, Self::SCRATCH, |budget| {
            require_credentials(credentials)?;
            program.revalidate(budget)?;
            signing_key.revalidate(program.policy(), budget)?;
            external_anchor.validate_continuity(budget)?;
            let root_snapshot = root_checks::inspect(&root, credentials)?;
            if root_checks::inspect(&root, credentials)? != root_snapshot {
                return Err(ProtectedIssuerSupervisorErrorV2::RootChanged);
            }
            let supervisor = Self {
                program,
                credentials,
                root,
                root_snapshot,
                signing_key,
                external_anchor,
                retained,
            };
            budget.reserve_storage(Self::OWNER_GROWTH)?;
            supervisor.check(budget)?;
            Ok((supervisor, Storage(Self::OWNER_GROWTH)))
        })
    }

    /// Rechecks the complete native pre-session chain, including root object identity.
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        budget.with_prepaid_scope(self.retained, ENTRY, Self::WORK, Self::SCRATCH, |budget| {
            self.check(budget)
        })
    }

    fn check(&self, budget: &mut Budget<'_>) -> Result<()> {
        require_credentials(self.credentials)?;
        self.program.revalidate(budget)?;
        self.signing_key.revalidate(self.program.policy(), budget)?;
        self.external_anchor.validate_continuity(budget)?;
        if root_checks::inspect(&self.root, self.credentials)? != self.root_snapshot {
            return Err(ProtectedIssuerSupervisorErrorV2::RootChanged);
        }
        Ok(())
    }

    /// Returns immutable caller-pinned native policy facts, not a descriptor.
    pub const fn policy(&self) -> &Policy {
        self.program.policy()
    }
    /// Returns the configured dedicated service identity, not process confinement evidence.
    pub const fn credentials(&self) -> Credentials {
        self.credentials
    }
    /// Returns the provisioned anchor service identity, without its endpoint.
    pub const fn external_anchor_service(&self) -> AnchorIdentity {
        self.external_anchor.service_identity()
    }
    /// Returns cached anchor process facts; continuity still requires revalidation.
    pub const fn external_anchor_process(
        &self,
    ) -> fe2o3_broker_authority_service::ExpectedClientProcessIdentityV1 {
        self.external_anchor.service_process_identity()
    }
    /// Full retained charge; retire it only after this owner is dropped or transferred.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
}

fn require_credentials(credentials: Credentials) -> Result<()> {
    if rustix::process::geteuid().as_raw() != credentials.uid()
        || rustix::process::getegid().as_raw() != credentials.gid()
    {
        return Err(ProtectedIssuerSupervisorErrorV2::ServiceIdentityMismatch);
    }
    Ok(())
}

impl fmt::Debug for ProtectedIssuerSupervisorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProtectedIssuerSupervisorV2")
            .field("authority", &"pre-session-custody-only")
            .field("policy", &self.policy().identity())
            .field("credentials", &self.credentials)
            .field("external_anchor_service", &self.external_anchor_service())
            .finish_non_exhaustive()
    }
}

/// Native supervisor binding or revalidation failure. No refusal retries V1.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedIssuerSupervisorErrorV2 {
    /// The caller's logical work or storage ledger refused the operation.
    Resource(Resource),
    /// Current effective UID/GID does not match the configured non-root service.
    ServiceIdentityMismatch,
    /// The independently admitted native program or policy failed revalidation.
    Program(ProgramError),
    /// Native key custody or its complete policy identity failed revalidation.
    SigningKey(KeyError),
    /// The exact provisioned endpoint or live anchor process failed continuity.
    ExternalAnchor(AnchorError),
    /// The root descriptor, access, type, owner, mode, links or xattrs are invalid.
    InvalidRoot(&'static str),
    /// Root identity or security metadata changed between observations.
    RootChanged,
    /// A single-attempt root inspection failed with a kernel errno.
    Io {
        /// Fixed operation label.
        operation: &'static str,
        /// Kernel error; no allocated diagnostic is retained.
        errno: rustix::io::Errno,
    },
}
impl From<Resource> for ProtectedIssuerSupervisorErrorV2 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<ProgramError> for ProtectedIssuerSupervisorErrorV2 {
    fn from(e: ProgramError) -> Self {
        Self::Program(e)
    }
}
impl From<KeyError> for ProtectedIssuerSupervisorErrorV2 {
    fn from(e: KeyError) -> Self {
        Self::SigningKey(e)
    }
}
impl From<AnchorError> for ProtectedIssuerSupervisorErrorV2 {
    fn from(e: AnchorError) -> Self {
        Self::ExternalAnchor(e)
    }
}
impl From<RootCheckError> for ProtectedIssuerSupervisorErrorV2 {
    fn from(e: RootCheckError) -> Self {
        match e {
            RootCheckError::Invalid(reason) => Self::InvalidRoot(reason),
            RootCheckError::Io { operation, errno } => Self::Io { operation, errno },
        }
    }
}
impl fmt::Display for ProtectedIssuerSupervisorErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::ServiceIdentityMismatch => f.write_str(
                "supervisor process does not match the protected issuer service UID and GID",
            ),
            Self::Program(e) => write!(f, "protected issuer program changed: {e}"),
            Self::SigningKey(e) => write!(f, "protected issuer signing key changed: {e}"),
            Self::ExternalAnchor(e) => write!(f, "protected external-anchor endpoint changed: {e}"),
            Self::InvalidRoot(reason) => write!(f, "invalid protected issuer root: {reason}"),
            Self::RootChanged => {
                f.write_str("protected issuer root identity or security metadata changed")
            }
            Self::Io { operation, errno } => write!(f, "{operation}: {errno}"),
        }
    }
}
impl Error for ProtectedIssuerSupervisorErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Program(e) => Some(e),
            Self::SigningKey(e) => Some(e),
            Self::ExternalAnchor(e) => Some(e),
            Self::Io { errno, .. } => Some(errno),
            _ => None,
        }
    }
}

const _: () = {
    use crate::IssuerProgramStorageV2 as ProgramStorage;
    use fe2o3_broker_authority_service::ProtectedExternalAnchorServiceStorageV2 as AnchorStorage;
    use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityStorageV2 as KeyStorage;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
    type Supervisor = ProtectedIssuerSupervisorV2;
    const fn envelope<T>() -> usize {
        size_of::<Result<T>>().saturating_sub(size_of::<T>())
            + size_of::<std::thread::Result<Result<T>>>().saturating_sub(size_of::<T>())
    }
    assert!(
        size_of::<(Supervisor, Storage)>()
            <= size_of::<(Program, ProgramStorage)>()
                + size_of::<(Key, KeyStorage)>()
                + size_of::<(Anchor, AnchorStorage)>()
                + Supervisor::ROOT_FILE_STORAGE
                + Supervisor::OWNER_GROWTH
    );
    assert!(
        8 * size_of::<ProtectedIssuerSupervisorErrorV2>()
            + 64 * size_of::<usize>()
            + 4 * size_of::<rustix::fs::Stat>()
            + size_of::<Ledger>()
            + size_of::<std::result::Result<(), Resource>>()
            + 2 * size_of::<bool>()
            + envelope::<(Supervisor, Storage)>()
            + envelope::<()>()
            <= 4096
    );
};

#[cfg(test)]
#[path = "authority_v2_tests.rs"]
pub(crate) mod tests;
