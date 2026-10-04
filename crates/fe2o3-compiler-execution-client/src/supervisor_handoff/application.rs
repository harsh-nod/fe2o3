//! Application-only four-right transfer. Ordinary issuer readiness cannot complete this path.

use super::*;
use crate::{
    ApplicationProofChannelErrorV1, ApplicationProofTransferPeerV1,
    RetainedApplicationServiceLaunchV1,
};
use fe2o3_runtime_protocol::{
    WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1, WorkerV3ApplicationRegistrationBindingV1,
    WorkerV3ApplicationSupervisorReadyErrorV1, WorkerV3ApplicationSupervisorReadyV1,
};

#[derive(Debug)]
pub enum ApplicationSupervisorHandoffErrorV1 {
    Handoff(CompilerExecutionHandoffErrorV1),
    ProofChannel(ApplicationProofChannelErrorV1),
    Readiness(WorkerV3ApplicationSupervisorReadyErrorV1),
    BindingMismatch,
}

impl fmt::Display for ApplicationSupervisorHandoffErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(error) => write!(f, "application handoff: {error}"),
            Self::ProofChannel(error) => write!(f, "application handoff: {error}"),
            Self::Readiness(error) => write!(f, "application handoff: {error}"),
            Self::BindingMismatch => {
                f.write_str("application binding differs from original launch custody")
            }
        }
    }
}

impl Error for ApplicationSupervisorHandoffErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(error) => Some(error),
            Self::ProofChannel(error) => Some(error),
            Self::Readiness(error) => Some(error),
            Self::BindingMismatch => None,
        }
    }
}

impl From<CompilerExecutionHandoffErrorV1> for ApplicationSupervisorHandoffErrorV1 {
    fn from(error: CompilerExecutionHandoffErrorV1) -> Self {
        Self::Handoff(error)
    }
}

/// Retains the application control connection after a single four-right transfer.
///
/// This is not readiness or authenticated root observation. In particular, it cannot be
/// converted to the ordinary compiler pending type or accept compiler-only readiness.
///
/// ```compile_fail
/// fn cloneable<T: Clone>() {}
/// cloneable::<fe2o3_compiler_execution_client::PendingApplicationSupervisorV1>();
/// ```
/// ```compile_fail
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<fe2o3_compiler_execution_client::PendingApplicationSupervisorV1>();
/// ```
/// ```compile_fail
/// fn downgrade(value: fe2o3_compiler_execution_client::PendingApplicationSupervisorV1)
///     -> fe2o3_compiler_execution_client::PendingCompilerExecutionSupervisorV1 {
///     value.into()
/// }
/// ```
pub struct PendingApplicationSupervisorV1 {
    control: OwnedFd,
    binding: WorkerV3ApplicationRegistrationBindingV1,
    expected: CompilerExecutionSupervisorCredentialsV1,
    deadline: Instant,
}

impl fmt::Debug for PendingApplicationSupervisorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _retained = self.control.as_fd();
        f.debug_struct("PendingApplicationSupervisorV1")
            .field("binding", &self.binding.identity())
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl PendingApplicationSupervisorV1 {
    pub const fn binding(&self) -> &WorkerV3ApplicationRegistrationBindingV1 {
        &self.binding
    }

    /// Requires the dedicated root-observation/issuer-readiness join and terminal EOF.
    /// This does not acknowledge application ACK, proof custody, or GPU authority.
    /// A later caller deadline cannot extend the original transfer deadline.
    pub fn await_readiness_until(
        self,
        profile: &CompilerExecutionClientProfileV1,
        deadline: Instant,
    ) -> Result<WorkerV3ApplicationSupervisorReadyV1, ApplicationSupervisorHandoffErrorV1> {
        let deadline = deadline.min(self.deadline);
        validate_boundary_deadline(deadline)?;
        let launch = self.binding.compiler_handoff().launch_manifest();
        if profile.supervisor_uid() != self.expected.uid()
            || profile.supervisor_gid() != self.expected.gid()
            || !launch.matches_policy(profile.policy())
            || !launch.matches_external_anchor_service(profile.external_anchor_service())
        {
            return Err(ApplicationSupervisorHandoffErrorV1::BindingMismatch);
        }
        validate_control(&self.control, self.expected)?;
        let bytes = receive_readiness::<WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1>(
            &self.control,
            deadline,
        )?;
        let readiness = WorkerV3ApplicationSupervisorReadyV1::decode(&bytes)
            .map_err(ApplicationSupervisorHandoffErrorV1::Readiness)?;
        if !readiness.matches_binding(&self.binding, profile.policy()) {
            return Err(ApplicationSupervisorHandoffErrorV1::BindingMismatch);
        }
        require_control_eof(&self.control, deadline)?;
        validate_control(&self.control, self.expected)?;
        require_deadline(deadline)?;
        Ok(readiness)
    }
}

impl RetainedApplicationServiceLaunchV1 {
    /// Transfers exactly compiler peer, original app pidfd, proof peer, original Cargo pidfd.
    ///
    /// The embedded compiler handoff is independently reconstructed and compared before send.
    /// The returned owner admits only the dedicated application readiness profile.
    pub fn transfer_to_supervisor_until(
        self,
        proof: ApplicationProofTransferPeerV1,
        binding: WorkerV3ApplicationRegistrationBindingV1,
        profile: &CompilerExecutionClientProfileV1,
        deadline: Instant,
    ) -> Result<PendingApplicationSupervisorV1, ApplicationSupervisorHandoffErrorV1> {
        validate_boundary_deadline(deadline)?;
        let expected = CompilerExecutionSupervisorCredentialsV1::new(
            profile.supervisor_uid(),
            profile.supervisor_gid(),
        )?;
        if self.client().uid() == expected.uid() {
            return Err(CompilerExecutionHandoffErrorV1::ClientAndSupervisorUidMatch.into());
        }
        validate_binding(
            &self,
            &binding,
            profile.external_anchor_service(),
            profile.policy(),
        )?;
        let control = connect_to_supervisor(
            Path::new(COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1),
            expected,
            deadline,
        )?;
        transfer(
            self,
            proof,
            binding,
            control,
            expected,
            profile.external_anchor_service(),
            profile.policy(),
            deadline,
        )
    }
}

fn validate_binding(
    launch: &RetainedApplicationServiceLaunchV1,
    binding: &WorkerV3ApplicationRegistrationBindingV1,
    anchor: CompilerExecutionExternalAnchorServiceIdentityV1,
    policy: &CompilerExecutionIssuerPolicyV1,
) -> Result<(), ApplicationSupervisorHandoffErrorV1> {
    launch
        .revalidate()
        .map_err(CompilerExecutionHandoffErrorV1::RustcLaunch)?;
    let manifest = CompilerExecutionServiceLaunchManifestV1::new(launch.client(), anchor, policy);
    let actual = CompilerExecutionSupervisorHandoffV1::new(launch.submitter(), manifest)
        .map_err(CompilerExecutionHandoffErrorV1::CanonicalHandoff)?;
    if binding.compiler_handoff() != &actual {
        return Err(ApplicationSupervisorHandoffErrorV1::BindingMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn transfer(
    launch: RetainedApplicationServiceLaunchV1,
    proof: ApplicationProofTransferPeerV1,
    binding: WorkerV3ApplicationRegistrationBindingV1,
    control: OwnedFd,
    expected: CompilerExecutionSupervisorCredentialsV1,
    anchor: CompilerExecutionExternalAnchorServiceIdentityV1,
    policy: &CompilerExecutionIssuerPolicyV1,
    deadline: Instant,
) -> Result<PendingApplicationSupervisorV1, ApplicationSupervisorHandoffErrorV1> {
    validate_boundary_deadline(deadline)?;
    validate_control(&control, expected)?;
    validate_binding(&launch, &binding, anchor, policy)?;
    let proof = proof
        .into_registration_descriptor(&binding)
        .map_err(ApplicationSupervisorHandoffErrorV1::ProofChannel)?;
    let (compiler, app_pidfd) = launch.compiler.into_descriptors();
    let rights = [
        compiler.as_fd(),
        app_pidfd.as_fd(),
        proof.as_fd(),
        launch.parent_pidfd.as_fd(),
    ];
    loop {
        wait_writable(&control, deadline)?;
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if !ancillary.push(SendAncillaryMessage::ScmRights(&rights)) {
            return Err(CompilerExecutionHandoffErrorV1::InvalidControl(
                "application rights buffer too small",
            )
            .into());
        }
        match sendmsg(
            &control,
            &[IoSlice::new(binding.canonical_bytes())],
            &mut ancillary,
            SendFlags::NOSIGNAL | SendFlags::DONTWAIT,
        ) {
            Ok(count) if count == binding.canonical_bytes().len() => break,
            Ok(_) => return Err(CompilerExecutionHandoffErrorV1::PartialSend.into()),
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => continue,
            Err(error) => return Err(CompilerExecutionHandoffErrorV1::Io(error.into()).into()),
        }
    }
    require_deadline(deadline)?;
    Ok(PendingApplicationSupervisorV1 {
        control,
        binding,
        expected,
        deadline,
    })
}

#[cfg(test)]
mod tests;
