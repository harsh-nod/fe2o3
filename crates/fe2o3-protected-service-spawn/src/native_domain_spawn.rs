//! Fresh-domain placement through the existing guarded native spawn and pool.
//! This closes aggregate cleanup custody, not proof-process isolation admission.

use super::*;
use crate::{native_cgroup::NativeCgroupDomainV1 as Domain, process_reaper::ReapSlotV1};
use fe2o3_artifact_transaction::ArtifactProcessSpawnLeaseV1 as Lease;

#[derive(Clone, Copy)]
pub(super) enum Placement {
    Current,
    Fresh,
}

impl StagedProtectedServiceExecV2 {
    /// Additional work for fresh-domain creation and atomic clone placement.
    /// Normal child/profile work and retained cleanup reservation remain separate.
    pub const FRESH_DOMAIN_WORK: usize =
        Domain::PREPARE_WORK + Domain::CREATE_WORK + Domain::CLONE_FD_WORK;
    /// Additional request peak while the prepared domain enters child custody.
    pub const FRESH_DOMAIN_SCRATCH: usize = Domain::STORAGE
        + Domain::PREPARE_SCRATCH
        + Domain::CREATE_SCRATCH
        + Domain::CLONE_FD_SCRATCH;

    /// Creates a fresh root-controlled cgroup and places the child into it
    /// atomically at clone. Neither an existing domain nor a caller FD is admitted.
    /// Retained inputs stay in the original cleanup slot until both the direct
    /// child is reaped and the aggregate domain is empty and removed. A refusal
    /// after creation can leave a domain-only record in that same funded pool.
    ///
    /// Fund `spawn_retaining_work` plus `FRESH_DOMAIN_WORK`, and
    /// `spawn_retaining_scratch` plus `FRESH_DOMAIN_SCRATCH`. Returned accounting
    /// has the same GROWTH convention as `spawn_retaining`.
    ///
    /// # Safety
    /// Every `spawn_retaining` obligation applies. The parent must be the trusted
    /// administrator in its deployment's real cgroup/user/mount context. Exclude
    /// competing mutations of its current cgroup and the fresh child domain.
    /// No staged binding, executed image or descendant may expose cgroup controls,
    /// relocate itself, delegate the domain, or create child cgroups. Keep the
    /// parent and cleanup controller privileged until aggregate retirement.
    /// This operation grants no namespace isolation, helper admission, proof
    /// acceptance or compiler/GPU execution authority.
    pub unsafe fn spawn_retaining_in_fresh_domain<T: Send + 'static>(
        &self,
        credentials: Credentials,
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
            Placement::Fresh,
            resources,
            retained_storage,
            cleanup,
            b,
        )
    }
}

// The original reserved slot owns rollback before mkdir, including unwinding
// between directory creation and descriptor/identity acquisition.
struct PreparedDomain(Option<(Domain, ReapSlotV1<'static>)>);

impl Drop for PreparedDomain {
    fn drop(&mut self) {
        if let Some((domain, slot)) = self.0.take() {
            slot.defer_domain(domain);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn clone_placed(
    staged: &syscall::StagedProtectedServiceExecV1,
    credentials: Credentials,
    ceiling: u32,
    lease: Lease,
    slot: ReapSlotV1<'static>,
    placement: Placement,
    b: &mut Budget<'_>,
) -> Result<RootOwnedProtectedServiceChildV2> {
    if matches!(placement, Placement::Current) {
        return clone_guarded(staged, credentials, ceiling, lease, slot);
    }
    b.with_prepaid_scope(
        0,
        0,
        StagedProtectedServiceExecV2::FRESH_DOMAIN_WORK,
        StagedProtectedServiceExecV2::FRESH_DOMAIN_SCRATCH,
        |_| {
            let mut pending = PreparedDomain(Some((Domain::prepare()?, slot)));
            let (domain, _) = pending.0.as_mut().expect("prepared domain custody");
            domain.create()?;
            let fd = domain.clone_cgroup_fd()?;
            let (pid, pidfd) = syscall::clone_child_with_cgroup(
                staged,
                credentials,
                ceiling,
                rustix::process::getpid(),
                Some(fd),
            )
            .map_err(|e| io("clone native child into fresh cgroup", e))?;
            // No fallible parent check or gate release precedes whole-owner adoption.
            let (domain, slot) = pending.0.take().expect("prepared domain custody");
            let child =
                RootOwnedProtectedServiceChildV2::new_with_domain(pid, pidfd, lease, domain, slot);
            child.check_pidfd()?;
            Ok(child)
        },
    )
}

#[cfg(test)]
#[path = "native_domain_spawn_tests.rs"]
mod tests;
