//! Fresh user-namespace mechanics; not proof-helper or memory-isolation admission.
//!
//! ```compile_fail
//! use fe2o3_protected_service_spawn::{ProtectedServiceCleanupServiceV2 as Cleanup,
//!     native_spawn::StagedProtectedServiceExecV2 as Stage};
//! use fe2o3_protected_service_profile::ProtectedServiceCredentialProfileV1 as Credentials;
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
//! fn forge(s: &Stage, c: Credentials, pool: &mut Cleanup, b: &mut Budget<'_>) {
//!     let _ = s.spawn_retaining_in_fresh_user_namespace(c, c, (), 0, pool, b);
//! }
//! ```

use super::*;
use crate::native_user_namespace::NativeUserNamespaceV1 as Namespace;
use std::os::fd::{AsFd, OwnedFd};

impl StagedProtectedServiceExecV2 {
    /// Additional work over retained spawn, including fresh-domain placement,
    /// namespace mapping/readback and the child's pre-profile mapping gate.
    pub const FRESH_NAMESPACE_WORK: usize = Self::FRESH_DOMAIN_WORK
        + Namespace::PREPARE_WORK
        + Namespace::CONFIGURE_WORK
        + Namespace::REVALIDATE_WORK
        + native_work::MAPPING_GATE_WORK
        + 8 * (1024 + 64);
    /// Additional request peak over retained spawn. Persistent namespace custody
    /// is included in the ordinary child and cleanup-pool storage charges.
    pub const FRESH_NAMESPACE_SCRATCH: usize = Self::FRESH_DOMAIN_SCRATCH
        + Namespace::STORAGE
        + Namespace::PREPARE_SCRATCH
        + Namespace::CONFIGURE_SCRATCH
        + Namespace::REVALIDATE_SCRATCH
        + 4096;

    /// Clones into a fresh user namespace and root-controlled cgroup, then maps
    /// root, helper and peer IDs before allowing the child to drop privileges.
    /// PID and time namespaces stay unchanged. This does not relax dumpability.
    /// Namespace handles remain in child custody through aggregate cleanup.
    ///
    /// Fund `spawn_retaining_work` plus `FRESH_NAMESPACE_WORK`, and
    /// `spawn_retaining_scratch` plus `FRESH_NAMESPACE_SCRATCH`.
    ///
    /// # Safety
    /// Every `spawn_retaining_in_fresh_domain` obligation applies. The caller must
    /// bind helper and peer identities to its actual administrator-controlled
    /// deployment and exclude competing namespace/map mutations. Numeric root,
    /// nsfs readback and identity maps do not establish administrator provenance.
    /// The caller must validate post-transition profile and endpoint identities
    /// before final exec release. This operation admits no proof helper, runtime,
    /// backing-writer exclusion, compiler execution or GPU launch.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn spawn_retaining_in_fresh_user_namespace<T: Send + 'static>(
        &self,
        credentials: Credentials,
        peer: Credentials,
        resources: T,
        retained_storage: usize,
        cleanup: &mut Cleanup,
        b: &mut Budget<'_>,
    ) -> Result<(
        RootOwnedRetainedServiceChildV2<T>,
        ProtectedServiceSpawnStorageV2,
    )> {
        self.spawn_retaining_placed(
            credentials,
            Placement::FreshNamespace(peer),
            resources,
            retained_storage,
            cleanup,
            b,
        )
    }
}

pub(super) struct MappingGate {
    reader: OwnedFd,
    writer: OwnedFd,
}

impl MappingGate {
    pub(super) fn new() -> Result<Self> {
        let (reader, writer) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)
            .map_err(|e| io("create namespace mapping gate", e))?;
        Ok(Self { reader, writer })
    }

    pub(super) fn child_ends(&self) -> (BorrowedFd<'_>, BorrowedFd<'_>) {
        (self.reader.as_fd(), self.writer.as_fd())
    }

    pub(super) fn release(self) -> Result<()> {
        // The parent retains a reader until after this single write, preventing
        // SIGPIPE even if the child failed before reaching the mapping gate.
        match rustix::io::write(&self.writer, &[crate::PROTECTED_SERVICE_GATE_RELEASE_V1]) {
            Ok(1) => Ok(()),
            Ok(_) => Err(ProtectedServiceSpawnErrorV2::State(
                "short namespace gate release",
            )),
            Err(e) => Err(io("release namespace mapping gate", e)),
        }
    }
}

#[cfg(test)]
#[path = "native_namespace_spawn_tests.rs"]
mod tests;
