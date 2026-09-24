//! Metered custody over the shared fixed process and namespace observations.

use crate::{ProtectedServiceCredentialProfileV1 as Credentials, observations};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use rustix::process::Pid;
use std::{error::Error, fmt, mem::size_of};

const ENTRY_WORK: usize = 8;
type Result<T> = std::result::Result<T, ProtectedServiceProfileErrorV2>;
type Storage = ProtectedServiceProfileStorageV2;

/// Fixed native resource or observation failure; constructing it allocates nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedServiceProfileErrorV2 {
    /// The caller's ledger refused the work, scratch, or retained-owner floor.
    Resource(Resource),
    /// The shared fixed observation refused the process or namespace facts.
    Observation(observations::Error),
}

impl From<Resource> for ProtectedServiceProfileErrorV2 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

impl From<observations::Error> for ProtectedServiceProfileErrorV2 {
    fn from(error: observations::Error) -> Self {
        Self::Observation(error)
    }
}

impl fmt::Display for ProtectedServiceProfileErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Observation(error) => error.fmt(f),
        }
    }
}

impl Error for ProtectedServiceProfileErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Observation(error) => Some(error),
        }
    }
}

/// Additional unreserved retained storage returned by a native capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedServiceProfileStorageV2(usize);

impl ProtectedServiceProfileStorageV2 {
    /// Reserve this delta before retaining the returned owner. Retire its full
    /// charge only after dropping or transferring it; unrelated floors stay live.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

// Logical envelopes include staged return values and fixed error records. The
// shared core accounts for its own bounded buffers and mechanical observations.
const fn scratch(core: usize, retained: usize) -> usize {
    core + 4 * retained + 4 * size_of::<ProtectedServiceProfileErrorV2>()
}

fn scope<T>(
    budget: &mut Budget<'_>,
    floor: usize,
    work: usize,
    scratch: usize,
    observe: impl FnOnce() -> Result<T>,
) -> Result<T> {
    budget.with_prepaid_scope(floor, ENTRY_WORK, work, scratch, |_| observe())
}

/// Move-only exact process-profile facts freshly observed on the caller's ledger.
///
/// This grants no launch or execution authority. It exposes only inert derived
/// credential/capability facts, never its unmetered observation owner. There is
/// no legacy-owner conversion or fallback.
///
/// Each operation restores entry storage on success, refusal, and unwind while
/// preserving accepted work, peak storage, and denial history. Capture returns
/// an unreserved full retained charge because it consumes no prepaid owner.
/// Borrowing operations require that full charge to remain prepaid. Quotas are
/// logical work/storage bounds, not allocator, RSS, stack, or latency bounds.
///
/// ```compile_fail
/// use fe2o3_protected_service_profile::ProtectedServiceProcessProfileV2;
/// fn duplicate(profile: ProtectedServiceProcessProfileV2) { let _ = profile.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_profile::{ProtectedServiceProcessProfileV1, ProtectedServiceProcessProfileV2};
/// fn upgrade(old: ProtectedServiceProcessProfileV1) -> ProtectedServiceProcessProfileV2 {
///     old.into()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_profile::{observations, ProtectedServiceProcessProfileV2};
/// fn expose(profile: &ProtectedServiceProcessProfileV2) -> &observations::ProcessProfile {
///     &profile.0
/// }
/// ```
pub struct ProtectedServiceProcessProfileV2(observations::ProcessProfile);

impl ProtectedServiceProcessProfileV2 {
    const RETAINED: usize = size_of::<(Self, Storage)>();

    /// Full worst-case capture work, including entry admission.
    pub const CAPTURE_WORK: usize = ENTRY_WORK + observations::PROCESS_CAPTURE_WORK;
    /// Additional capture peak above all caller-owned reservations.
    pub const CAPTURE_SCRATCH: usize =
        scratch(observations::PROCESS_CAPTURE_SCRATCH, Self::RETAINED);
    /// Full worst-case current-process revalidation work, including entry.
    pub const REVALIDATE_CURRENT_WORK: usize = ENTRY_WORK + observations::PROCESS_CURRENT_WORK;
    /// Additional current-process revalidation peak above caller reservations.
    pub const REVALIDATE_CURRENT_SCRATCH: usize =
        scratch(observations::PROCESS_CURRENT_SCRATCH, Self::RETAINED);
    /// Full worst-case proc-visible process revalidation work, including entry.
    pub const REVALIDATE_PROCESS_WORK: usize = ENTRY_WORK + observations::PROCESS_VALIDATE_WORK;
    /// Additional proc-visible revalidation peak above caller reservations.
    pub const REVALIDATE_PROCESS_SCRATCH: usize =
        scratch(observations::PROCESS_VALIDATE_SCRATCH, Self::RETAINED);

    /// Observes the exact protected profile through the shared mechanical core.
    /// Credentials are inert configuration and require no incoming owner charge.
    pub fn capture(credentials: Credentials, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        scope(budget, 0, Self::CAPTURE_WORK, Self::CAPTURE_SCRATCH, || {
            Ok((
                Self(observations::ProcessProfile::capture(credentials)?),
                ProtectedServiceProfileStorageV2(Self::RETAINED),
            ))
        })
    }

    /// Rechecks all current-process profile facts against the retained profile.
    pub fn revalidate_current(&self, budget: &mut Budget<'_>) -> Result<()> {
        scope(
            budget,
            Self::RETAINED,
            Self::REVALIDATE_CURRENT_WORK,
            Self::REVALIDATE_CURRENT_SCRATCH,
            || Ok(self.0.revalidate_current()?),
        )
    }

    /// Rechecks proc-visible fields of a gated child. Securebits, dumpability,
    /// core limits, signal ownership, and namespaces need their separate checks.
    pub fn revalidate_process(&self, pid: Pid, budget: &mut Budget<'_>) -> Result<()> {
        scope(
            budget,
            Self::RETAINED,
            Self::REVALIDATE_PROCESS_WORK,
            Self::REVALIDATE_PROCESS_SCRATCH,
            || Ok(self.0.revalidate_process(pid)?),
        )
    }

    /// Inert credential configuration admitted by capture.
    pub const fn credentials(&self) -> Credentials {
        self.0.credentials()
    }

    /// Inert kernel capability ceiling admitted by capture.
    pub const fn cap_last_cap(&self) -> u32 {
        self.0.cap_last_cap()
    }

    /// Full retained logical charge, including the storage receipt and padding.
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }
}

impl fmt::Debug for ProtectedServiceProcessProfileV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProtectedServiceProcessProfileV2")
            .field("credentials", &self.credentials())
            .field("cap_last_cap", &self.cap_last_cap())
            .finish_non_exhaustive()
    }
}

/// Move-only namespace identities freshly observed on the caller's ledger.
///
/// Capture requires unchanged child namespaces. Revalidation is a point-in-time
/// observation, not namespace isolation, process custody, or launch authority.
/// The accounting/lifetime contract is the same as
/// [`ProtectedServiceProcessProfileV2`]; there is no unmetered inner accessor.
///
/// ```compile_fail
/// use fe2o3_protected_service_profile::ProtectedServiceNamespaceSetV2;
/// fn duplicate(namespaces: ProtectedServiceNamespaceSetV2) { let _ = namespaces.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_profile::{ProtectedServiceNamespaceSetV1, ProtectedServiceNamespaceSetV2};
/// fn upgrade(old: ProtectedServiceNamespaceSetV1) -> ProtectedServiceNamespaceSetV2 {
///     old.into()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_profile::{observations, ProtectedServiceNamespaceSetV2};
/// fn expose(namespaces: &ProtectedServiceNamespaceSetV2) -> &observations::NamespaceSet {
///     &namespaces.0
/// }
/// ```
pub struct ProtectedServiceNamespaceSetV2(observations::NamespaceSet);

impl ProtectedServiceNamespaceSetV2 {
    const RETAINED: usize = size_of::<(Self, Storage)>();

    /// Full worst-case capture work, including entry admission.
    pub const CAPTURE_WORK: usize = ENTRY_WORK + observations::NAMESPACE_CAPTURE_WORK;
    /// Additional capture peak above all caller-owned reservations.
    pub const CAPTURE_SCRATCH: usize =
        scratch(observations::NAMESPACE_CAPTURE_SCRATCH, Self::RETAINED);
    /// Full worst-case current-namespace revalidation work, including entry.
    pub const REVALIDATE_SELF_WORK: usize = ENTRY_WORK + observations::NAMESPACE_SELF_WORK;
    /// Additional current-namespace revalidation peak above caller reservations.
    pub const REVALIDATE_SELF_SCRATCH: usize =
        scratch(observations::NAMESPACE_SELF_SCRATCH, Self::RETAINED);
    /// Full worst-case target-namespace revalidation work, including entry.
    pub const REVALIDATE_PROCESS_WORK: usize = ENTRY_WORK + observations::NAMESPACE_PROCESS_WORK;
    /// Additional target-namespace revalidation peak above caller reservations.
    pub const REVALIDATE_PROCESS_SCRATCH: usize =
        scratch(observations::NAMESPACE_PROCESS_SCRATCH, Self::RETAINED);

    /// Captures the shared namespace observations without privilege changes.
    pub fn capture_self(budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        scope(budget, 0, Self::CAPTURE_WORK, Self::CAPTURE_SCRATCH, || {
            Ok((
                Self(observations::NamespaceSet::capture_self()?),
                ProtectedServiceProfileStorageV2(Self::RETAINED),
            ))
        })
    }

    /// Requires current and child namespaces to match the captured identities.
    pub fn revalidate_self(&self, budget: &mut Budget<'_>) -> Result<()> {
        scope(
            budget,
            Self::RETAINED,
            Self::REVALIDATE_SELF_WORK,
            Self::REVALIDATE_SELF_SCRATCH,
            || Ok(self.0.revalidate_self()?),
        )
    }

    /// Requires a target's namespaces to match the captured identities.
    /// PID stability and process custody remain the caller's responsibility.
    pub fn revalidate_process(&self, pid: Pid, budget: &mut Budget<'_>) -> Result<()> {
        scope(
            budget,
            Self::RETAINED,
            Self::REVALIDATE_PROCESS_WORK,
            Self::REVALIDATE_PROCESS_SCRATCH,
            || Ok(self.0.revalidate_process(pid)?),
        )
    }

    /// Full retained logical charge, including the storage receipt and padding.
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }
}

impl fmt::Debug for ProtectedServiceNamespaceSetV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProtectedServiceNamespaceSetV2")
            .finish_non_exhaustive()
    }
}

/// Full worst-case SIGCHLD observation work, including entry admission.
pub const REQUIRE_OWNED_SIGCHLD_WORK_V2: usize = ENTRY_WORK + observations::SIGCHLD_WORK;
/// Additional SIGCHLD logical scratch above all caller-owned reservations.
pub const REQUIRE_OWNED_SIGCHLD_SCRATCH_V2: usize = scratch(observations::SIGCHLD_SCRATCH, 0);

/// Requires the default owned SIGCHLD disposition through the fixed observer.
/// Does not change signal disposition or return a retained owner. Restores entry
/// storage while retaining accepted work, peak, and denial history.
pub fn require_owned_sigchld_v2(budget: &mut Budget<'_>) -> Result<()> {
    scope(
        budget,
        0,
        REQUIRE_OWNED_SIGCHLD_WORK_V2,
        REQUIRE_OWNED_SIGCHLD_SCRATCH_V2,
        || Ok(observations::require_owned_sigchld()?),
    )
}
