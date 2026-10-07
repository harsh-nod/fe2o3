//! Separate nonprivileged controller profile for supervised analyzer/Verus descendants.

use super::*;

/// Exact identity for a proof controller, not a locked signing-service identity.
///
/// All IDs equal this nonzero UID/GID; groups and every capability set (including
/// bounding) are empty, no_new_privs is set, dumpability/core limits are zero and
/// umask is 077. Securebits must be exactly zero and seccomp absent, as required
/// by the authenticated proof supervisor. The controller itself is trusted to
/// spawn and contain its measured descendants; this is not the application sandbox.
/// There is no conversion to `ProtectedServiceCredentialProfileV1`.
///
/// ```compile_fail
/// use fe2o3_protected_service_profile::{ProofControllerCredentialProfileV1,
///     ProtectedServiceCredentialProfileV1};
/// fn signing_profile(value: ProofControllerCredentialProfileV1)
///     -> ProtectedServiceCredentialProfileV1 { value }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofControllerCredentialProfileV1(ProtectedServiceCredentialProfileV1);

impl ProofControllerCredentialProfileV1 {
    /// Configures exact non-root IDs; this admits no process or deployment.
    pub const fn new(uid: u32, gid: u32) -> Result<Self, ProtectedServiceCredentialProfileErrorV1> {
        match ProtectedServiceCredentialProfileV1::new(uid, gid) {
            Ok(value) => Ok(Self(value)),
            Err(error) => Err(error),
        }
    }
    /// Returns the required real, effective, saved and filesystem UID.
    pub const fn uid(self) -> u32 {
        self.0.uid()
    }
    /// Returns the required real, effective, saved and filesystem GID.
    pub const fn gid(self) -> u32 {
        self.0.gid()
    }
    /// Returns the exact, deliberately unlocked proof-supervisor securebits.
    pub const fn securebits(self) -> u32 {
        0
    }
}

/// Current-process proof-controller profile and original namespace custody.
///
/// Capture after the fixed controller exec. This admits only process facts, not
/// deployment approval, proof execution, remote custody, or GPU authority.
/// Revalidation is bound to the originating process and thread.
///
/// ```compile_fail
/// use fe2o3_protected_service_profile::ProofControllerProcessProfileV1;
/// fn require_send<T: Send>() {}
/// require_send::<ProofControllerProcessProfileV1>();
/// ```
pub struct ProofControllerProcessProfileV1 {
    common: observations::ProcessProfile,
    namespaces: ProtectedServiceNamespaceSetV1,
    owner_pid: rustix::process::Pid,
    owner_tid: rustix::process::Pid,
    _thread: std::marker::PhantomData<*mut ()>,
}

impl ProofControllerProcessProfileV1 {
    /// Captures this thread's exact confinement and original namespace identities.
    pub fn capture(
        credentials: ProofControllerCredentialProfileV1,
    ) -> Result<Self, ProtectedServiceProfileErrorV1> {
        let result = Self {
            common: observations::ProcessProfile::capture_proof_controller(credentials.0)
                .map_err(map_observation_error)?,
            namespaces: ProtectedServiceNamespaceSetV1::capture_current_thread()?,
            owner_pid: rustix::process::getpid(),
            owner_tid: rustix::thread::gettid(),
            _thread: std::marker::PhantomData,
        };
        result.revalidate_current()?;
        Ok(result)
    }
    /// Returns configured IDs, not admission or deployment authority.
    pub const fn credentials(&self) -> ProofControllerCredentialProfileV1 {
        ProofControllerCredentialProfileV1(self.common.credentials())
    }
    /// Rechecks exact state on the original process and thread.
    pub fn revalidate_current(&self) -> Result<(), ProtectedServiceProfileErrorV1> {
        if rustix::process::getpid() != self.owner_pid || rustix::thread::gettid() != self.owner_tid
        {
            return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
                "proof controller owner changed",
            ));
        }
        self.common
            .revalidate_proof_controller()
            .map_err(map_observation_error)?;
        require_owned_sigchld_v1()?;
        self.namespaces.revalidate_current_thread()
    }
}

/// Rejects inherited state that cannot form the closed proof-controller profile.
///
/// Checks the calling thread, not its process leader. The launcher must separately
/// require exact root IDs and exclusive child-reaping ownership. A filtered systemd
/// service cannot launch this role; it needs an independently approved unfiltered
/// launch boundary, not removal of the issuer's confinement.
pub fn require_proof_controller_parent_v1() -> Result<(), ProtectedServiceProfileErrorV1> {
    observations::require_proof_controller_parent().map_err(map_observation_error)
}

/// Checks proc-visible identity/confinement, including absence of inherited seccomp.
///
/// The fixed child must separately check securebits, dumpability, core limits,
/// signal state and namespaces before opening or using proof resources.
pub fn validate_proof_controller_process_v1(
    credentials: ProofControllerCredentialProfileV1,
    pid: rustix::process::Pid,
) -> Result<(), ProtectedServiceProfileErrorV1> {
    observations::validate_proof_controller_process(credentials.0, pid)
        .map_err(map_observation_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_controller_credentials_remain_distinct_and_closed() {
        assert_eq!(
            ProofControllerCredentialProfileV1::new(1000, 1001)
                .unwrap()
                .securebits(),
            0
        );
        assert_ne!(
            ProtectedServiceCredentialProfileV1::new(1000, 1001)
                .unwrap()
                .securebits(),
            0
        );
        for (uid, gid) in [(0, 1000), (1000, 0), (u32::MAX, 1000), (1000, u32::MAX)] {
            assert!(ProofControllerCredentialProfileV1::new(uid, gid).is_err());
        }
    }

    #[test]
    fn proof_controller_requires_complete_zero_seccomp_facts() {
        assert!(observations::require_no_seccomp_fields(Some(0), Some(0)).is_ok());
        for value in [None, Some(1), Some(2), Some(u32::MAX)] {
            assert!(observations::require_no_seccomp_fields(value, Some(0)).is_err());
            assert!(observations::require_no_seccomp_fields(Some(0), value).is_err());
        }
    }
}
