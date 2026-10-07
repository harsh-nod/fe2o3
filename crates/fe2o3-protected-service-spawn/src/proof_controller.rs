//! Separate typed lifecycle for an unfiltered, nonprivileged proof controller.

use super::*;
use fe2o3_protected_service_profile::{
    ProofControllerCredentialProfileV1, require_proof_controller_parent_v1,
};

/// Pinned proof-controller image and fixed inherited descriptors.
///
/// The caller must independently admit the executable and deployment. This is not
/// a signing-service launcher and provides no proof or GPU authority. The static
/// executable must use the secure entrypoint and revalidate its proof-controller
/// profile after exec, before opening protected resources.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::{StagedProofControllerExecV1, StagedProtectedServiceExecV1};
/// fn convert(value: StagedProofControllerExecV1) -> StagedProtectedServiceExecV1 { value }
/// ```
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::StagedProofControllerExecV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<StagedProofControllerExecV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::StagedProofControllerExecV1;
/// fn require_as_fd<T: std::os::fd::AsFd>() {}
/// require_as_fd::<StagedProofControllerExecV1>();
/// ```
pub struct StagedProofControllerExecV1 {
    inner: syscall::StagedProtectedServiceExecV1,
}

impl fmt::Debug for StagedProofControllerExecV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StagedProofControllerExecV1")
            .field("authority", &"root-proof-controller-exec-only")
            .field("descriptor_count", &self.inner.descriptor_count())
            .finish_non_exhaustive()
    }
}

impl StagedProofControllerExecV1 {
    /// Stages the exact descriptor table above the shared private descriptor floor.
    pub fn new(
        executable: &File,
        bindings: &[ProtectedServiceDescriptorBindingV1<'_>],
        profile_ready_writer: BorrowedFd<'_>,
        gate_reader: BorrowedFd<'_>,
        exec_status_writer: BorrowedFd<'_>,
    ) -> Result<Self, ProtectedServiceSpawnErrorV1> {
        StagedProtectedServiceExecV1::new(
            executable,
            bindings,
            profile_ready_writer,
            gate_reader,
            exec_status_writer,
        )
        .map(|staged| Self {
            inner: staged.inner,
        })
    }

    /// Creates one direct child with an atomically acquired original pidfd.
    ///
    /// Requires exact root IDs, owned SIGCHLD, zero securebits and no seccomp on
    /// the calling thread. Filters cannot be removed by this launcher. Use only
    /// from an independently approved unfiltered root deployment boundary.
    pub fn spawn(
        &self,
        credentials: ProofControllerCredentialProfileV1,
    ) -> Result<RootOwnedProofControllerChildV1, ProtectedServiceSpawnErrorV1> {
        require_exact_root_identity_v1()?;
        require_owned_sigchld_v1().map_err(ProtectedServiceSpawnErrorV1::ParentProfile)?;
        require_proof_controller_parent_v1()
            .map_err(ProtectedServiceSpawnErrorV1::ParentProfile)?;
        syscall::spawn_proof_controller(
            &self.inner,
            credentials,
            read_cap_last_cap()?,
            rustix::process::getpid(),
        )
        .map(|inner| RootOwnedProofControllerChildV1 { inner })
        .map_err(|source| io_error("clone proof-controller child", source))
    }
}

/// Original pidfd and exclusive direct-child reaping custody for a proof controller.
///
/// Drop kills and reaps this exact child. Neither child exit nor this owner proves
/// GPU settlement, proof completion, deployment approval or descendant containment.
/// The deployment must independently contain the complete proof process tree.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::{RootOwnedProofControllerChildV1, RootOwnedProtectedServiceChildV1};
/// fn convert(value: RootOwnedProofControllerChildV1) -> RootOwnedProtectedServiceChildV1 { value }
/// ```
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::RootOwnedProofControllerChildV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<RootOwnedProofControllerChildV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::RootOwnedProofControllerChildV1;
/// fn require_as_fd<T: std::os::fd::AsFd>() {}
/// require_as_fd::<RootOwnedProofControllerChildV1>();
/// ```
pub struct RootOwnedProofControllerChildV1 {
    pub(super) inner: syscall::RootOwnedProtectedServiceChildV1,
}

impl fmt::Debug for RootOwnedProofControllerChildV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RootOwnedProofControllerChildV1")
            .field("authority", &"root-lifecycle-custody-only")
            .field("pid", &self.pid())
            .finish_non_exhaustive()
    }
}

impl RootOwnedProofControllerChildV1 {
    /// Returns the exact direct-child PID.
    pub const fn pid(&self) -> rustix::process::Pid {
        self.inner.pid()
    }

    /// Observes the exact original pidfd without reaping.
    pub fn is_live(&self) -> Result<bool, ProtectedServiceSpawnErrorV1> {
        self.inner
            .is_live()
            .map_err(|source| io_error("observe proof-controller pidfd", source))
    }

    /// Duplicates the original pidfd for an independently validated transfer.
    pub fn try_clone_pidfd(&self) -> Result<OwnedFd, ProtectedServiceSpawnErrorV1> {
        self.inner
            .try_clone_pidfd()
            .map_err(|source| io_error("clone proof-controller pidfd", source))
    }

    /// Returns an inert immediate exit description if available.
    pub fn exit_description(&self, fallback: &'static str) -> String {
        self.inner.exit_description(fallback)
    }

    /// Sends SIGKILL through the original pidfd and reaps the exact child once.
    pub fn cancel_and_reap(&mut self) -> Result<(), ProtectedServiceSpawnErrorV1> {
        self.inner.cancel_and_reap().map_err(map_reap_error)
    }

    /// Sends SIGKILL via the original pidfd and makes one nonblocking reap attempt.
    /// False retains exclusive child custody for a later poll. True is idempotent.
    /// Neither result establishes descendant containment or GPU settlement.
    pub fn poll_cancel_and_reap(&mut self) -> Result<bool, ProtectedServiceSpawnErrorV1> {
        self.inner.poll_cancel_and_reap().map_err(map_reap_error)
    }
}
