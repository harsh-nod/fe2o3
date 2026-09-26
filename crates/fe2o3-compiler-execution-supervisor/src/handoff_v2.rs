//! Native session admission, not prepared launch or service activation.
use crate::{
    ProtectedIssuerSupervisorErrorV2 as SupervisorError, ProtectedIssuerSupervisorV2 as Supervisor,
    handoff_checks::{self as checks, Snapshot},
    handoff_v2_io as transport,
};
use fe2o3_broker_authority_service::{
    ExpectedClientProcessIdentityV1, LiveClientPidfdErrorV2 as PidfdError,
    LiveClientPidfdIdentityV2 as LiveClient,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V2 as BYTES,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionServiceLaunchManifestErrorV2 as ManifestError,
    CompilerExecutionServiceLaunchManifestV2 as Manifest,
    CompilerExecutionSupervisorHandoffErrorV2 as FrameError,
    CompilerExecutionSupervisorHandoffV2 as Frame,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{
    error::Error,
    fmt,
    mem::{align_of, size_of},
    os::fd::OwnedFd,
    time::{Duration, Instant},
};

const ENTRY: usize = 8;
type Result<T> = std::result::Result<T, ProtectedIssuerHandoffErrorV2>;
use ProtectedIssuerHandoffStorageV2 as Storage;

/// Move-only native handoff, exact service peer and one admitted live client pidfd.
///
/// Admission authenticates connection-time peer identities and observes client
/// liveness. It does not prove process ancestry, exclusive endpoint ownership,
/// full child confinement, service readiness, compiler execution or GPU authority.
/// No raw descriptor, signing, V1 conversion, or launch operation is exposed.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AcceptedCompilerExecutionHandoffV2;
/// fn clone<T: Clone>() {}
/// clone::<AcceptedCompilerExecutionHandoffV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AcceptedCompilerExecutionHandoffV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<AcceptedCompilerExecutionHandoffV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{AcceptedCompilerExecutionHandoffV1, AcceptedCompilerExecutionHandoffV2};
/// fn upgrade(old: AcceptedCompilerExecutionHandoffV1) -> AcceptedCompilerExecutionHandoffV2 { old.into() }
/// ```
pub struct AcceptedCompilerExecutionHandoffV2 {
    control: OwnedFd,
    handoff: Frame,
    service_peer: OwnedFd,
    client: LiveClient,
    control_snapshot: Snapshot,
    service_snapshot: Snapshot,
    pidfd_snapshot: Snapshot,
    retained: usize,
}
type Accepted = AcceptedCompilerExecutionHandoffV2;
#[path = "handoff_native_adapter.rs"]
mod adapter;
adapter::handoff!(
    AcceptedCompilerExecutionHandoffV2,
    ProtectedIssuerHandoffStorageV2,
    ProtectedIssuerHandoffErrorV2,
    CompilerExecutionAttestationStorageV2
);

impl Accepted {
    // Consuming launch prepays closure of the unused peer/frame/client owners.
    pub(crate) fn into_control(self) -> OwnedFd {
        self.control
    }

    pub(crate) fn clone_launch_peers(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(std::fs::File, std::fs::File, usize)> {
        budget.with_prepaid_scope(self.retained, ENTRY, Self::WORK, Self::SCRATCH, |b| {
            checks::service_peer(&self.service_peer, self.manifest().client())?;
            let peer = rustix::io::fcntl_dupfd_cloexec(&self.service_peer, 0)?;
            b.reserve_storage(Self::CONTROL_STORAGE)?;
            if checks::snapshot(&peer)? != self.service_snapshot {
                return Err(ProtectedIssuerHandoffErrorV2::DescriptorChanged);
            }
            checks::service_peer(&peer, self.manifest().client())?;
            let (pidfd, delta) = self.client.try_clone_for_transfer(b)?;
            b.reserve_storage(delta.additional_storage())?;
            Ok((
                peer.into(),
                pidfd.into(),
                Self::CONTROL_STORAGE + delta.additional_storage(),
            ))
        })
    }

    pub(crate) fn revalidate_launch_peers(
        &self,
        peer: &std::fs::File,
        pidfd: &std::fs::File,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = self
            .retained
            .checked_add(Self::CONTROL_STORAGE + LiveClient::FD_STORAGE)
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, ENTRY, Self::WORK, Self::SCRATCH, |b| {
            if checks::snapshot(peer)? != self.service_snapshot {
                return Err(ProtectedIssuerHandoffErrorV2::DescriptorChanged);
            }
            checks::service_peer(peer, self.manifest().client())?;
            self.client.validate_transfer(pidfd, b)?;
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "handoff_v2_tests.rs"]
pub(crate) mod tests;
