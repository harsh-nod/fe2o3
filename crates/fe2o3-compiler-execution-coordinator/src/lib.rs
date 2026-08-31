#![deny(missing_docs, unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("fe2o3-compiler-execution-coordinator requires Linux x86-64");

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, OwnedFd};
use std::time::{Duration, Instant};

use fe2o3_compiler_closure_capability::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1, CompilerExecutionPolicyCapabilityV1,
    CompilerExecutionSigningKeyCapabilityV1, CompilerExecutionSupervisorDeploymentCapabilityV1,
};
use fe2o3_compiler_execution_lifecycle::{
    CompilerExecutionServiceLifecycleLeaseV1, LifecycleLeaseErrorV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionSupervisorDeploymentV1, CompilerExecutionSupervisorReadyErrorV1,
    CompilerExecutionSupervisorReadyV1, MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
};
use fe2o3_compiler_execution_supervisor::{
    COMPILER_EXECUTION_SUPERVISOR_BOOTSTRAP_FD_V1,
    COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PEER_FD_V1,
    COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PIDFD_V1,
    COMPILER_EXECUTION_SUPERVISOR_ISSUER_FD_V1, COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_FD_V1,
    COMPILER_EXECUTION_SUPERVISOR_LIFECYCLE_FD_V1, COMPILER_EXECUTION_SUPERVISOR_LISTENER_FD_V1,
    COMPILER_EXECUTION_SUPERVISOR_POLICY_FD_V1, COMPILER_EXECUTION_SUPERVISOR_ROOT_FD_V1,
    COMPILER_EXECUTION_SUPERVISOR_SIGNING_KEY_FD_V1, IssuerServiceCredentialProfileV1,
    ProtectedIssuerServiceDeploymentInputsV1, ProtectedIssuerServiceProvisioningErrorV1,
    ProvisionedProtectedIssuerServiceInputsV1,
};
use fe2o3_external_anchor_coordinator::{
    ExternalAnchorCoordinatorErrorV1, RootManagedExternalAnchorV1,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileErrorV1, ProtectedServiceNamespaceSetV1,
    ProtectedServiceProfileErrorV1, validate_protected_service_process_v1,
};
use fe2o3_protected_service_spawn::{
    PROTECTED_SERVICE_GATE_RELEASE_V1, PROTECTED_SERVICE_PROFILE_READY_V1,
    ProtectedServiceDescriptorBindingV1, ProtectedServiceSpawnErrorV1,
    RootOwnedProtectedServiceChildV1, StagedProtectedServiceExecV1, require_exact_root_identity_v1,
};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV1, ProtectedStaticExecutableMeasurementV1,
    ProtectedStaticExecutableOwnerV1, ProtectedStaticExecutableV1,
};
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SocketFlags,
    SocketType, recv, recvmsg, socketpair,
};
use rustix::pipe::{PipeFlags, pipe_with};

#[allow(unsafe_code)]
mod entrypoint;
#[allow(unsafe_code)]
mod inherited;
mod lifecycle;
mod provisioning;
#[allow(unsafe_code)]
mod provisioning_entrypoint;

pub use entrypoint::run_inherited_compiler_execution_coordinator_v1;

pub use inherited::{
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_DAEMON_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_DEPLOYMENT_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_HELPER_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_KEY_SEED_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_PROVISIONING_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_ROOT_FD_V1, COMPILER_EXECUTION_COORDINATOR_ISSUER_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ISSUER_KEY_SEED_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_LAUNCHER_FD_V1, COMPILER_EXECUTION_COORDINATOR_POLICY_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_RUNTIME_ROOT_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_DEPLOYMENT_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_ROOT_FD_V1, InheritedCompilerExecutionDeploymentV1,
};
pub use provisioning::{
    CompilerExecutionProvisioningBundleV1, CompilerExecutionProvisioningErrorV1,
    CompilerExecutionProvisioningInputsV1,
};
pub use provisioning_entrypoint::{
    CompilerExecutionProvisioningInstallErrorV1, run_compiler_execution_reference_provisioner_v1,
};

const MAX_DEPLOYMENT_TIMEOUT_V1: Duration = Duration::from_secs(120);
const POLL_INTERVAL_V1: Duration = Duration::from_millis(1);

/// Move-only untrusted source descriptors for the complete static supervisor program chain.
///
/// The sources grant no launch authority and are measured, copied, sealed, and rebound to the
/// dedicated service identity only during root-owned preparation.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::CompilerExecutionSupervisorProgramSourcesV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CompilerExecutionSupervisorProgramSourcesV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_coordinator::CompilerExecutionSupervisorProgramSourcesV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<CompilerExecutionSupervisorProgramSourcesV1>();
/// ```
pub struct CompilerExecutionSupervisorProgramSourcesV1 {
    supervisor: File,
    launcher: File,
    issuer: File,
}

impl CompilerExecutionSupervisorProgramSourcesV1 {
    /// Groups the protected supervisor, pre-exec launcher, and issuer sources in role order.
    pub fn new(supervisor: File, launcher: File, issuer: File) -> Self {
        Self {
            supervisor,
            launcher,
            issuer,
        }
    }
}

impl fmt::Debug for CompilerExecutionSupervisorProgramSourcesV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionSupervisorProgramSourcesV1")
            .field("authority", &"untrusted-source-custody-only")
            .finish_non_exhaustive()
    }
}

/// Move-only canonical deployment, issuer-policy, and root signing-key-template custody.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::CompilerExecutionSupervisorTrustV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CompilerExecutionSupervisorTrustV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_coordinator::CompilerExecutionSupervisorTrustV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<CompilerExecutionSupervisorTrustV1>();
/// ```
pub struct CompilerExecutionSupervisorTrustV1 {
    deployment: CompilerExecutionSupervisorDeploymentCapabilityV1,
    policy: CompilerExecutionPolicyCapabilityV1,
    key_template: CompilerExecutionSigningKeyCapabilityV1,
}

impl CompilerExecutionSupervisorTrustV1 {
    /// Binds and revalidates one exact deployment, policy, and root-owned key template.
    pub fn new(
        deployment: CompilerExecutionSupervisorDeploymentCapabilityV1,
        policy: CompilerExecutionPolicyCapabilityV1,
        key_template: CompilerExecutionSigningKeyCapabilityV1,
    ) -> Result<Self, CompilerExecutionCoordinatorErrorV1> {
        let trust = Self {
            deployment,
            policy,
            key_template,
        };
        trust.revalidate()?;
        Ok(trust)
    }

    fn revalidate(&self) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
        self.deployment
            .revalidate()
            .map_err(CompilerExecutionCoordinatorErrorV1::DeploymentCapability)?;
        self.policy
            .revalidate()
            .map_err(CompilerExecutionCoordinatorErrorV1::PolicyCapability)?;
        if !self
            .deployment
            .deployment()
            .matches_policy(self.policy.policy())
        {
            return Err(CompilerExecutionCoordinatorErrorV1::PolicyMismatch);
        }
        self.key_template
            .revalidate(self.policy.policy())
            .map_err(CompilerExecutionCoordinatorErrorV1::KeyTemplate)?;
        if self.key_template.verifying_key() != *self.policy.policy().verifying_key() {
            return Err(CompilerExecutionCoordinatorErrorV1::KeyMismatch);
        }
        Ok(())
    }
}

impl fmt::Debug for CompilerExecutionSupervisorTrustV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionSupervisorTrustV1")
            .field("authority", &"root-deployment-trust-custody-only")
            .field("deployment", &self.deployment.deployment().identity())
            .field("policy", &self.policy.policy().identity())
            .finish_non_exhaustive()
    }
}

struct PinnedDeploymentExecutableV1 {
    executable: ProtectedStaticExecutableV1,
    expected: ProtectedStaticExecutableMeasurementV1,
    owner: ProtectedStaticExecutableOwnerV1,
    role: &'static str,
}

impl PinnedDeploymentExecutableV1 {
    fn admit(
        source: File,
        measurement: CompilerExecutionIssuerMeasurementV1,
        maximum: u64,
        owner: ProtectedStaticExecutableOwnerV1,
        role: &'static str,
    ) -> Result<Self, CompilerExecutionCoordinatorErrorV1> {
        let expected = ProtectedStaticExecutableMeasurementV1::new(
            measurement.sha256(),
            measurement.byte_len(),
            maximum,
        )
        .map_err(CompilerExecutionCoordinatorErrorV1::Executable)?;
        let executable =
            ProtectedStaticExecutableV1::seal_source_for_owner(source, expected, owner, role)
                .map_err(CompilerExecutionCoordinatorErrorV1::Executable)?;
        let pinned = Self {
            executable,
            expected,
            owner,
            role,
        };
        pinned.revalidate()?;
        Ok(pinned)
    }

    fn revalidate(&self) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
        self.executable
            .revalidate()
            .map_err(CompilerExecutionCoordinatorErrorV1::Executable)?;
        let object = self.executable.object_identity();
        if self.executable.measurement() != self.expected
            || object.uid() != self.owner.uid()
            || object.gid() != self.owner.gid()
        {
            return Err(CompilerExecutionCoordinatorErrorV1::ExecutableRole(
                self.role,
            ));
        }
        Ok(())
    }

    fn try_clone_for_exec(&self) -> Result<File, CompilerExecutionCoordinatorErrorV1> {
        self.revalidate()?;
        self.executable
            .try_clone_for_exec()
            .map_err(CompilerExecutionCoordinatorErrorV1::Executable)
    }
}

/// Root-retained, fully admitted inputs for one protected-supervisor occurrence.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::PreparedCompilerExecutionSupervisorV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<PreparedCompilerExecutionSupervisorV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_coordinator::PreparedCompilerExecutionSupervisorV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<PreparedCompilerExecutionSupervisorV1>();
/// ```
pub struct PreparedCompilerExecutionSupervisorV1 {
    supervisor: PinnedDeploymentExecutableV1,
    launcher: PinnedDeploymentExecutableV1,
    issuer: PinnedDeploymentExecutableV1,
    trust: CompilerExecutionSupervisorTrustV1,
    service_inputs: ProtectedIssuerServiceDeploymentInputsV1,
    supervisor_lifecycle: CompilerExecutionServiceLifecycleLeaseV1,
    anchor: RootManagedExternalAnchorV1,
    credentials: IssuerServiceCredentialProfileV1,
    namespaces: ProtectedServiceNamespaceSetV1,
    prepared_by: rustix::process::Pid,
    // Keep last so implicit drop reaps the retained anchor before unlocking provisioning.
    lifecycle: lifecycle::CompilerExecutionLifecycleLeaseV1,
}

impl fmt::Debug for PreparedCompilerExecutionSupervisorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedCompilerExecutionSupervisorV1")
            .field("authority", &"root-protected-supervisor-launch-only")
            .field("deployment", &self.deployment().identity())
            .field("credentials", &self.credentials)
            .finish_non_exhaustive()
    }
}

impl PreparedCompilerExecutionSupervisorV1 {
    /// Admits, seals, and binds every input required by the sole deployed supervisor path.
    pub(crate) fn prepare(
        programs: CompilerExecutionSupervisorProgramSourcesV1,
        trust: CompilerExecutionSupervisorTrustV1,
        service_inputs: ProvisionedProtectedIssuerServiceInputsV1,
        supervisor_lifecycle: CompilerExecutionServiceLifecycleLeaseV1,
        lifecycle: lifecycle::CompilerExecutionLifecycleLeaseV1,
        anchor: RootManagedExternalAnchorV1,
    ) -> Result<Self, CompilerExecutionCoordinatorErrorV1> {
        require_exact_root_identity_v1().map_err(CompilerExecutionCoordinatorErrorV1::Spawn)?;
        trust.revalidate()?;
        let deployment = trust.deployment.deployment();
        let credentials = IssuerServiceCredentialProfileV1::new(
            deployment.service_uid(),
            deployment.service_gid(),
        )
        .map_err(CompilerExecutionCoordinatorErrorV1::Credentials)?;
        if service_inputs.credentials() != credentials {
            return Err(CompilerExecutionCoordinatorErrorV1::ServiceIdentityMismatch);
        }
        service_inputs
            .revalidate()
            .map_err(CompilerExecutionCoordinatorErrorV1::ServiceInputs)?;
        supervisor_lifecycle
            .revalidate()
            .map_err(CompilerExecutionCoordinatorErrorV1::ServiceLifecycle)?;
        anchor
            .validate_continuity()
            .map_err(CompilerExecutionCoordinatorErrorV1::Anchor)?;
        if !anchor
            .deployment()
            .matches_supervisor_and_policy(deployment, trust.policy.policy())
        {
            return Err(CompilerExecutionCoordinatorErrorV1::AnchorBindingMismatch);
        }
        let owner = ProtectedStaticExecutableOwnerV1::new(
            deployment.service_uid(),
            deployment.service_gid(),
        )
        .map_err(CompilerExecutionCoordinatorErrorV1::Executable)?;
        let supervisor = PinnedDeploymentExecutableV1::admit(
            programs.supervisor,
            deployment.executable(),
            MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1,
            owner,
            "protected compiler-execution supervisor",
        )?;
        let launcher = PinnedDeploymentExecutableV1::admit(
            programs.launcher,
            deployment.launcher(),
            MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
            owner,
            "compiler-execution static pre-exec launcher",
        )?;
        let issuer = PinnedDeploymentExecutableV1::admit(
            programs.issuer,
            trust.policy.policy().executable(),
            MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
            owner,
            "compiler-execution issuer",
        )?;
        let service_inputs = service_inputs
            .into_deployment_transfer()
            .map_err(CompilerExecutionCoordinatorErrorV1::ServiceInputs)?;
        let prepared = Self {
            supervisor,
            launcher,
            issuer,
            trust,
            service_inputs,
            supervisor_lifecycle,
            anchor,
            credentials,
            namespaces: ProtectedServiceNamespaceSetV1::capture_self()
                .map_err(CompilerExecutionCoordinatorErrorV1::Profile)?,
            prepared_by: rustix::process::getpid(),
            lifecycle,
        };
        prepared.revalidate()?;
        Ok(prepared)
    }

    /// Returns the exact inert deployment manifest without exposing capability custody.
    pub const fn deployment(&self) -> &CompilerExecutionSupervisorDeploymentV1 {
        self.trust.deployment.deployment()
    }

    /// Revalidates every retained input and the root coordinator's identity and namespaces.
    pub fn revalidate(&self) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
        require_exact_root_identity_v1().map_err(CompilerExecutionCoordinatorErrorV1::Spawn)?;
        if rustix::process::getpid() != self.prepared_by {
            return Err(CompilerExecutionCoordinatorErrorV1::CoordinatorChanged);
        }
        self.namespaces
            .revalidate_self()
            .map_err(CompilerExecutionCoordinatorErrorV1::Profile)?;
        self.trust.revalidate()?;
        let deployment = self.deployment();
        if self.credentials.uid() != deployment.service_uid()
            || self.credentials.gid() != deployment.service_gid()
            || self.service_inputs.credentials() != self.credentials
        {
            return Err(CompilerExecutionCoordinatorErrorV1::ServiceIdentityMismatch);
        }
        self.supervisor.revalidate()?;
        self.launcher.revalidate()?;
        self.issuer.revalidate()?;
        if self.supervisor.expected.sha256() != deployment.executable().sha256()
            || self.supervisor.expected.byte_len() != deployment.executable().byte_len()
            || self.launcher.expected.sha256() != deployment.launcher().sha256()
            || self.launcher.expected.byte_len() != deployment.launcher().byte_len()
            || self.issuer.expected.sha256() != self.trust.policy.policy().executable().sha256()
            || self.issuer.expected.byte_len() != self.trust.policy.policy().executable().byte_len()
        {
            return Err(CompilerExecutionCoordinatorErrorV1::ExecutableBindingMismatch);
        }
        self.service_inputs
            .revalidate()
            .map_err(CompilerExecutionCoordinatorErrorV1::ServiceInputs)?;
        self.supervisor_lifecycle
            .revalidate()
            .map_err(CompilerExecutionCoordinatorErrorV1::ServiceLifecycle)?;
        self.lifecycle.revalidate()?;
        self.anchor
            .validate_continuity()
            .map_err(CompilerExecutionCoordinatorErrorV1::Anchor)?;
        if !self
            .anchor
            .deployment()
            .matches_supervisor_and_policy(deployment, self.trust.policy.policy())
        {
            return Err(CompilerExecutionCoordinatorErrorV1::AnchorBindingMismatch);
        }
        Ok(())
    }

    /// Launches the measured supervisor and returns root-owned live service custody.
    pub fn launch(
        self,
        timeout: Duration,
    ) -> Result<RootManagedCompilerExecutionServiceV1, CompilerExecutionCoordinatorErrorV1> {
        let deadline = bounded_deadline(timeout)?;
        self.revalidate()?;
        let (root_bootstrap, child_bootstrap) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|source| io_error("create root-to-supervisor bootstrap", source.into()))?;
        let (profile_reader, profile_writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)
            .map_err(|source| io_error("create supervisor-profile channel", source.into()))?;
        let (gate_reader, gate_writer) = pipe_with(PipeFlags::CLOEXEC)
            .map_err(|source| io_error("create supervisor release gate", source.into()))?;

        let mut child = {
            let supervisor = self.supervisor.try_clone_for_exec()?;
            let launcher = self.launcher.try_clone_for_exec()?;
            let issuer = self.issuer.try_clone_for_exec()?;
            let (listener, root) = self
                .service_inputs
                .try_clone_ordered_for_spawn()
                .map_err(CompilerExecutionCoordinatorErrorV1::ServiceInputs)?;
            let policy = self
                .trust
                .policy
                .try_clone_for_transfer()
                .map_err(CompilerExecutionCoordinatorErrorV1::PolicyCapability)?;
            self.trust
                .key_template
                .revalidate(self.trust.policy.policy())
                .map_err(CompilerExecutionCoordinatorErrorV1::KeyTemplate)?;
            let key_template = self
                .trust
                .key_template
                .try_clone_for_transfer()
                .map_err(CompilerExecutionCoordinatorErrorV1::KeyTemplate)?;
            let anchor_transfer = self
                .anchor
                .try_clone_for_supervisor(self.deployment(), self.trust.policy.policy())
                .map_err(CompilerExecutionCoordinatorErrorV1::Anchor)?;
            let (anchor_peer, anchor_pidfd) = anchor_transfer.into_ordered_descriptors();
            let deployment = self
                .trust
                .deployment
                .try_clone_for_transfer()
                .map_err(CompilerExecutionCoordinatorErrorV1::DeploymentCapability)?;
            let bindings = [
                binding(
                    listener.as_fd(),
                    COMPILER_EXECUTION_SUPERVISOR_LISTENER_FD_V1,
                )?,
                binding(root.as_fd(), COMPILER_EXECUTION_SUPERVISOR_ROOT_FD_V1)?,
                binding(
                    launcher.as_fd(),
                    COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_FD_V1,
                )?,
                binding(issuer.as_fd(), COMPILER_EXECUTION_SUPERVISOR_ISSUER_FD_V1)?,
                binding(policy.as_fd(), COMPILER_EXECUTION_SUPERVISOR_POLICY_FD_V1)?,
                binding(
                    key_template.as_fd(),
                    COMPILER_EXECUTION_SUPERVISOR_SIGNING_KEY_FD_V1,
                )?,
                binding(
                    anchor_peer.as_fd(),
                    COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PEER_FD_V1,
                )?,
                binding(
                    anchor_pidfd.as_fd(),
                    COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PIDFD_V1,
                )?,
                binding(
                    child_bootstrap.as_fd(),
                    COMPILER_EXECUTION_SUPERVISOR_BOOTSTRAP_FD_V1,
                )?,
                binding(
                    self.supervisor_lifecycle.as_fd(),
                    COMPILER_EXECUTION_SUPERVISOR_LIFECYCLE_FD_V1,
                )?,
                binding(
                    deployment.as_fd(),
                    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1,
                )?,
            ];
            let staged = StagedProtectedServiceExecV1::new(
                &supervisor,
                &bindings,
                profile_writer.as_fd(),
                gate_reader.as_fd(),
                child_bootstrap.as_fd(),
            )
            .map_err(CompilerExecutionCoordinatorErrorV1::Spawn)?;
            fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| {
                staged
                    .spawn(self.credentials)
                    .map_err(CompilerExecutionCoordinatorErrorV1::Spawn)
            })?
        };
        drop(child_bootstrap);
        drop(profile_writer);
        drop(gate_reader);

        let result = (|| {
            await_profile_ready(&profile_reader, &root_bootstrap, &child, deadline)?;
            self.namespaces
                .revalidate_self()
                .map_err(CompilerExecutionCoordinatorErrorV1::Profile)?;
            self.namespaces
                .revalidate_process(child.pid())
                .map_err(CompilerExecutionCoordinatorErrorV1::Profile)?;
            validate_protected_service_process_v1(self.credentials, child.pid())
                .map_err(CompilerExecutionCoordinatorErrorV1::Profile)?;
            self.revalidate()?;
            release_child(&gate_writer)?;
            drop(gate_writer);
            let readiness = receive_ready(&root_bootstrap, &child, self.deployment(), deadline)?;
            await_bootstrap_eof(&root_bootstrap, &child, deadline)?;
            self.revalidate()?;
            self.namespaces
                .revalidate_process(child.pid())
                .map_err(CompilerExecutionCoordinatorErrorV1::Profile)?;
            validate_protected_service_process_v1(self.credentials, child.pid())
                .map_err(CompilerExecutionCoordinatorErrorV1::Profile)?;
            if !child
                .is_live()
                .map_err(CompilerExecutionCoordinatorErrorV1::Spawn)?
            {
                return Err(child_exited(&child, "supervisor exited after readiness"));
            }
            Ok(readiness)
        })();
        match result {
            Ok(readiness) => Ok(RootManagedCompilerExecutionServiceV1 {
                child,
                provisioning: Some(self),
                readiness,
            }),
            Err(error) => match child.cancel_and_reap() {
                Ok(()) => Err(error),
                Err(cleanup) => Err(CompilerExecutionCoordinatorErrorV1::Spawn(cleanup)),
            },
        }
    }

    fn shutdown_anchor(self) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
        self.anchor
            .shutdown()
            .map_err(CompilerExecutionCoordinatorErrorV1::Anchor)
    }
}

fn binding<'a>(
    source: std::os::fd::BorrowedFd<'a>,
    destination: i32,
) -> Result<ProtectedServiceDescriptorBindingV1<'a>, CompilerExecutionCoordinatorErrorV1> {
    ProtectedServiceDescriptorBindingV1::new(source, destination)
        .map_err(CompilerExecutionCoordinatorErrorV1::Spawn)
}

/// Root-retained live supervisor and external-anchor lifecycle custody.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::RootManagedCompilerExecutionServiceV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<RootManagedCompilerExecutionServiceV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_coordinator::RootManagedCompilerExecutionServiceV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<RootManagedCompilerExecutionServiceV1>();
/// ```
pub struct RootManagedCompilerExecutionServiceV1 {
    child: RootOwnedProtectedServiceChildV1,
    provisioning: Option<PreparedCompilerExecutionSupervisorV1>,
    readiness: CompilerExecutionSupervisorReadyV1,
}

impl fmt::Debug for RootManagedCompilerExecutionServiceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RootManagedCompilerExecutionServiceV1")
            .field("authority", &"root-service-lifecycle-custody-only")
            .field("pid", &self.child.pid())
            .field("readiness", &self.readiness.identity())
            .finish_non_exhaustive()
    }
}

impl RootManagedCompilerExecutionServiceV1 {
    /// Returns the exact child PID without exposing pidfd custody.
    pub const fn pid(&self) -> rustix::process::Pid {
        self.child.pid()
    }

    /// Returns the canonical deployment-readiness evidence.
    pub const fn readiness(&self) -> &CompilerExecutionSupervisorReadyV1 {
        &self.readiness
    }

    /// Revalidates root identity, child profile/liveness, and every retained deployment input.
    pub fn validate_continuity(&self) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
        let provisioning = self
            .provisioning
            .as_ref()
            .expect("live service retains provisioning custody");
        provisioning.revalidate()?;
        if !self
            .child
            .is_live()
            .map_err(CompilerExecutionCoordinatorErrorV1::Spawn)?
        {
            return Err(child_exited(&self.child, "managed supervisor is not live"));
        }
        provisioning
            .namespaces
            .revalidate_process(self.child.pid())
            .map_err(CompilerExecutionCoordinatorErrorV1::Profile)?;
        validate_protected_service_process_v1(provisioning.credentials, self.child.pid())
            .map_err(CompilerExecutionCoordinatorErrorV1::Profile)?;
        if !self
            .readiness
            .matches_deployment(pid_u32(self.child.pid())?, provisioning.deployment())
        {
            return Err(CompilerExecutionCoordinatorErrorV1::ReadyMismatch);
        }
        Ok(())
    }

    /// Terminates and exactly once reaps the supervisor, then shuts down the retained anchor.
    pub fn shutdown(mut self) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
        let supervisor = self
            .child
            .cancel_and_reap()
            .map_err(CompilerExecutionCoordinatorErrorV1::Spawn);
        let anchor = self
            .provisioning
            .take()
            .expect("live service retains provisioning custody")
            .shutdown_anchor();
        supervisor?;
        anchor
    }
}

impl Drop for RootManagedCompilerExecutionServiceV1 {
    fn drop(&mut self) {
        let _ = self.child.cancel_and_reap();
    }
}

fn await_profile_ready(
    profile: &OwnedFd,
    bootstrap: &OwnedFd,
    child: &RootOwnedProtectedServiceChildV1,
    deadline: Instant,
) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    let mut bytes = [0_u8; 2];
    loop {
        match rustix::io::read(profile, &mut bytes) {
            Ok(1) if bytes[0] == PROTECTED_SERVICE_PROFILE_READY_V1 => return Ok(()),
            Ok(0) => return Err(child_failure(bootstrap, child, "supervisor child profile")),
            Ok(_) => return Err(CompilerExecutionCoordinatorErrorV1::NoncanonicalProfileReady),
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                if let Some(stage) = receive_child_stage(bootstrap)? {
                    return Err(CompilerExecutionCoordinatorErrorV1::ChildStage(stage));
                }
                await_child_progress(child, deadline, "supervisor child profile")?;
            }
            Err(source) => {
                return Err(io_error(
                    "read supervisor child-profile record",
                    source.into(),
                ));
            }
        }
    }
}

fn release_child(gate: &OwnedFd) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    loop {
        match rustix::io::write(gate, &[PROTECTED_SERVICE_GATE_RELEASE_V1]) {
            Ok(1) => return Ok(()),
            Ok(_) => return Err(CompilerExecutionCoordinatorErrorV1::NoncanonicalGateRelease),
            Err(rustix::io::Errno::INTR) => {}
            Err(source) => {
                return Err(io_error("release measured supervisor child", source.into()));
            }
        }
    }
}

fn receive_ready(
    bootstrap: &OwnedFd,
    child: &RootOwnedProtectedServiceChildV1,
    deployment: &CompilerExecutionSupervisorDeploymentV1,
    deadline: Instant,
) -> Result<CompilerExecutionSupervisorReadyV1, CompilerExecutionCoordinatorErrorV1> {
    loop {
        let mut payload = [0_u8; COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1];
        let mut vectors = [IoSliceMut::new(&mut payload)];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        match recvmsg(
            bootstrap,
            &mut vectors,
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        ) {
            Ok(received) => {
                let mut has_ancillary = false;
                for message in ancillary.drain() {
                    has_ancillary = true;
                    if let RecvAncillaryMessage::ScmRights(descriptors) = message {
                        for descriptor in descriptors {
                            drop(descriptor);
                        }
                    }
                }
                if received.bytes == 1 && !has_ancillary {
                    return Err(CompilerExecutionCoordinatorErrorV1::ChildStage(payload[0]));
                }
                if received.bytes != payload.len()
                    || has_ancillary
                    || received
                        .flags
                        .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
                {
                    return Err(CompilerExecutionCoordinatorErrorV1::MalformedReady);
                }
                let readiness = CompilerExecutionSupervisorReadyV1::decode(&payload)
                    .map_err(CompilerExecutionCoordinatorErrorV1::ReadyProtocol)?;
                if !readiness.matches_deployment(pid_u32(child.pid())?, deployment) {
                    return Err(CompilerExecutionCoordinatorErrorV1::ReadyMismatch);
                }
                return Ok(readiness);
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                await_child_progress(child, deadline, "supervisor deployment readiness")?;
            }
            Err(source) => {
                return Err(io_error(
                    "receive supervisor deployment readiness",
                    source.into(),
                ));
            }
        }
    }
}

fn await_bootstrap_eof(
    bootstrap: &OwnedFd,
    child: &RootOwnedProtectedServiceChildV1,
    deadline: Instant,
) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    let mut payload = [0_u8; 2];
    loop {
        match recv(bootstrap, &mut payload, RecvFlags::DONTWAIT) {
            Ok((0, 0)) => return Ok(()),
            Ok((1, _)) => return Err(CompilerExecutionCoordinatorErrorV1::ChildStage(payload[0])),
            Ok(_) => return Err(CompilerExecutionCoordinatorErrorV1::MalformedReadyEof),
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                await_child_progress(child, deadline, "supervisor bootstrap EOF")?;
            }
            Err(source) => {
                return Err(io_error("observe supervisor bootstrap EOF", source.into()));
            }
        }
    }
}

fn receive_child_stage(
    bootstrap: &OwnedFd,
) -> Result<Option<u8>, CompilerExecutionCoordinatorErrorV1> {
    let mut payload = [0_u8; 2];
    match recv(bootstrap, &mut payload, RecvFlags::DONTWAIT) {
        Ok((0, 0)) => Ok(None),
        Ok((1, _)) => Ok(Some(payload[0])),
        Ok(_) => Err(CompilerExecutionCoordinatorErrorV1::MalformedReady),
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => Ok(None),
        Err(source) => Err(io_error(
            "read supervisor child failure stage",
            source.into(),
        )),
    }
}

fn child_failure(
    bootstrap: &OwnedFd,
    child: &RootOwnedProtectedServiceChildV1,
    boundary: &'static str,
) -> CompilerExecutionCoordinatorErrorV1 {
    match receive_child_stage(bootstrap) {
        Ok(Some(stage)) => CompilerExecutionCoordinatorErrorV1::ChildStage(stage),
        _ => child_exited(child, boundary),
    }
}

fn child_exited(
    child: &RootOwnedProtectedServiceChildV1,
    fallback: &'static str,
) -> CompilerExecutionCoordinatorErrorV1 {
    CompilerExecutionCoordinatorErrorV1::ChildExited(child.exit_description(fallback))
}

fn await_child_progress(
    child: &RootOwnedProtectedServiceChildV1,
    deadline: Instant,
    boundary: &'static str,
) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    if !child
        .is_live()
        .map_err(CompilerExecutionCoordinatorErrorV1::Spawn)?
    {
        return Err(child_exited(child, boundary));
    }
    if Instant::now() >= deadline {
        return Err(CompilerExecutionCoordinatorErrorV1::Timeout(boundary));
    }
    std::thread::sleep(POLL_INTERVAL_V1);
    Ok(())
}

fn pid_u32(pid: rustix::process::Pid) -> Result<u32, CompilerExecutionCoordinatorErrorV1> {
    u32::try_from(pid.as_raw_pid()).map_err(|_| CompilerExecutionCoordinatorErrorV1::ReadyPid)
}

fn bounded_deadline(timeout: Duration) -> Result<Instant, CompilerExecutionCoordinatorErrorV1> {
    if timeout.is_zero() || timeout > MAX_DEPLOYMENT_TIMEOUT_V1 {
        return Err(CompilerExecutionCoordinatorErrorV1::InvalidTimeout);
    }
    Instant::now()
        .checked_add(timeout)
        .ok_or(CompilerExecutionCoordinatorErrorV1::InvalidTimeout)
}

fn io_error(operation: &'static str, source: io::Error) -> CompilerExecutionCoordinatorErrorV1 {
    CompilerExecutionCoordinatorErrorV1::Io { operation, source }
}

/// Stable preparation, launch, readiness, continuity, and shutdown failures.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionCoordinatorErrorV1 {
    /// Process arguments or system-manager activation metadata are not exact.
    InvalidActivation(&'static str),
    /// Signal-mask installation or synchronous shutdown waiting failed.
    Signal(io::Error),
    /// One fixed root-entrypoint descriptor was absent, inheritable-state changed, or inaccessible.
    InheritedDescriptor {
        /// Fixed descriptor number whose admission failed.
        descriptor: i32,
        /// Exact descriptor operation that failed.
        operation: &'static str,
        /// Kernel failure.
        source: io::Error,
    },
    /// One root-provisioned source has invalid type, access, ownership, mode, links, metadata, or bytes.
    ProvisionedInput {
        /// Stable deployment role.
        role: &'static str,
        /// Stable rejection reason.
        reason: &'static str,
    },
    /// One canonical root-provisioned record failed strict decoding.
    ProvisionedRecord {
        /// Stable deployment role.
        role: &'static str,
        /// Decoder failure.
        reason: String,
    },
    /// Another deployment occurrence owns an incompatible lifecycle lease.
    LifecycleBusy,
    /// The retained lifecycle-lock file identity or security metadata changed.
    LifecycleChanged,
    /// The root coordinator PID changed after preparation.
    CoordinatorChanged,
    /// The deployment service UID/GID is invalid.
    Credentials(ProtectedServiceCredentialProfileErrorV1),
    /// Deployment, service-input, and protected profile identities disagree.
    ServiceIdentityMismatch,
    /// A static program source or pinned image failed admission or continuity.
    Executable(ProtectedStaticExecutableErrorV1),
    /// A pinned image changed role metadata.
    ExecutableRole(&'static str),
    /// Pinned image measurements no longer match deployment and policy.
    ExecutableBindingMismatch,
    /// The sealed supervisor-deployment capability failed continuity.
    DeploymentCapability(String),
    /// The sealed issuer-policy capability failed continuity.
    PolicyCapability(String),
    /// Deployment and issuer policy disagree.
    PolicyMismatch,
    /// The root-owned signing-key template failed continuity.
    KeyTemplate(String),
    /// The root-owned external-anchor signing-key template failed admission.
    ExternalAnchorKeyTemplate(String),
    /// Signing-key template and issuer policy disagree.
    KeyMismatch,
    /// Persistent listener/root admission or continuity failed.
    ServiceInputs(ProtectedIssuerServiceProvisioningErrorV1),
    /// Crash-retained protected-service lifecycle custody failed.
    ServiceLifecycle(LifecycleLeaseErrorV1),
    /// External-anchor lifecycle, transfer, or shutdown failed.
    Anchor(ExternalAnchorCoordinatorErrorV1),
    /// External-anchor deployment does not bind this supervisor and policy.
    AnchorBindingMismatch,
    /// Protected-service staging, spawn, pidfd, or reaping failed.
    Spawn(ProtectedServiceSpawnErrorV1),
    /// Parent or child protected-service profile validation failed.
    Profile(ProtectedServiceProfileErrorV1),
    /// Launch timeout is zero, excessive, or unrepresentable.
    InvalidTimeout,
    /// Child profile channel emitted a noncanonical record.
    NoncanonicalProfileReady,
    /// Parent release gate could not emit exactly one byte.
    NoncanonicalGateRelease,
    /// Post-clone child reported one exact failed syscall stage.
    ChildStage(u8),
    /// Child exited before completing the named boundary.
    ChildExited(String),
    /// Supervisor readiness transport was malformed or carried ancillary data.
    MalformedReady,
    /// Readiness record did not match the exact child PID and deployment.
    ReadyMismatch,
    /// Readiness PID cannot be represented by the protocol.
    ReadyPid,
    /// Canonical readiness decoding failed.
    ReadyProtocol(CompilerExecutionSupervisorReadyErrorV1),
    /// Bootstrap emitted data after the canonical readiness packet.
    MalformedReadyEof,
    /// One bounded deployment boundary timed out.
    Timeout(&'static str),
    /// One bounded kernel operation failed.
    Io {
        /// Exact operation that failed.
        operation: &'static str,
        /// Kernel failure.
        source: io::Error,
    },
}

impl fmt::Display for CompilerExecutionCoordinatorErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidActivation(reason) => {
                write!(formatter, "invalid coordinator activation: {reason}")
            }
            Self::Signal(error) => write!(formatter, "coordinator signal handling failed: {error}"),
            Self::InheritedDescriptor {
                descriptor,
                operation,
                source,
            } => write!(
                formatter,
                "coordinator descriptor {descriptor} failed during {operation}: {source}"
            ),
            Self::ProvisionedInput { role, reason } => {
                write!(formatter, "invalid root-provisioned {role}: {reason}")
            }
            Self::ProvisionedRecord { role, reason } => {
                write!(
                    formatter,
                    "invalid root-provisioned {role} record: {reason}"
                )
            }
            Self::LifecycleBusy => {
                formatter.write_str("another compiler-execution lifecycle occurrence is active")
            }
            Self::LifecycleChanged => {
                formatter.write_str("compiler-execution lifecycle-lock identity or policy changed")
            }
            Self::CoordinatorChanged => formatter.write_str("root coordinator PID changed"),
            Self::Credentials(error) => {
                write!(formatter, "invalid supervisor credentials: {error}")
            }
            Self::ServiceIdentityMismatch => {
                formatter.write_str("supervisor service identities disagree")
            }
            Self::Executable(error) => {
                write!(formatter, "supervisor program admission failed: {error}")
            }
            Self::ExecutableRole(role) => write!(formatter, "pinned {role} role changed"),
            Self::ExecutableBindingMismatch => {
                formatter.write_str("pinned program measurements changed binding")
            }
            Self::DeploymentCapability(error) => write!(
                formatter,
                "supervisor deployment capability failed: {error}"
            ),
            Self::PolicyCapability(error) => {
                write!(formatter, "issuer policy capability failed: {error}")
            }
            Self::PolicyMismatch => {
                formatter.write_str("supervisor deployment names another issuer policy")
            }
            Self::KeyTemplate(error) => {
                write!(formatter, "root signing-key template failed: {error}")
            }
            Self::ExternalAnchorKeyTemplate(error) => {
                write!(
                    formatter,
                    "root external-anchor key template failed: {error}"
                )
            }
            Self::KeyMismatch => {
                formatter.write_str("root signing-key template names another policy key")
            }
            Self::ServiceInputs(error) => {
                write!(formatter, "supervisor service inputs failed: {error}")
            }
            Self::ServiceLifecycle(error) => {
                write!(formatter, "protected service lifecycle failed: {error}")
            }
            Self::Anchor(error) => write!(formatter, "external-anchor custody failed: {error}"),
            Self::AnchorBindingMismatch => {
                formatter.write_str("external-anchor deployment names another supervisor or policy")
            }
            Self::Spawn(error) => write!(formatter, "protected supervisor spawn failed: {error}"),
            Self::Profile(error) => {
                write!(formatter, "protected supervisor profile failed: {error}")
            }
            Self::InvalidTimeout => formatter.write_str("invalid supervisor deployment timeout"),
            Self::NoncanonicalProfileReady => {
                formatter.write_str("noncanonical supervisor profile-ready record")
            }
            Self::NoncanonicalGateRelease => {
                formatter.write_str("noncanonical supervisor gate release")
            }
            Self::ChildStage(stage) => {
                write!(formatter, "supervisor child setup failed at stage {stage}")
            }
            Self::ChildExited(detail) => write!(formatter, "supervisor child exited: {detail}"),
            Self::MalformedReady => formatter.write_str("malformed supervisor readiness transfer"),
            Self::ReadyMismatch => {
                formatter.write_str("supervisor readiness does not match child and deployment")
            }
            Self::ReadyPid => formatter.write_str("invalid supervisor readiness PID"),
            Self::ReadyProtocol(error) => write!(formatter, "supervisor readiness failed: {error}"),
            Self::MalformedReadyEof => {
                formatter.write_str("supervisor bootstrap has trailing data")
            }
            Self::Timeout(boundary) => {
                write!(formatter, "supervisor deployment timed out at {boundary}")
            }
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for CompilerExecutionCoordinatorErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Signal(error) => Some(error),
            Self::InheritedDescriptor { source, .. } => Some(source),
            Self::Credentials(error) => Some(error),
            Self::Executable(error) => Some(error),
            Self::ServiceInputs(error) => Some(error),
            Self::ServiceLifecycle(error) => Some(error),
            Self::Anchor(error) => Some(error),
            Self::Spawn(error) => Some(error),
            Self::Profile(error) => Some(error),
            Self::ReadyProtocol(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Child, Command};

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerPolicyV1,
    };
    use rustix::net::SendFlags;

    use super::*;

    #[test]
    fn deployment_timeout_is_strictly_bounded() {
        assert!(matches!(
            bounded_deadline(Duration::ZERO),
            Err(CompilerExecutionCoordinatorErrorV1::InvalidTimeout)
        ));
        assert!(matches!(
            bounded_deadline(MAX_DEPLOYMENT_TIMEOUT_V1 + Duration::from_nanos(1)),
            Err(CompilerExecutionCoordinatorErrorV1::InvalidTimeout)
        ));
        assert!(bounded_deadline(Duration::from_secs(1)).is_ok());
    }

    #[test]
    fn supervisor_descriptor_contract_is_unique_and_below_staging() {
        let destinations = [
            COMPILER_EXECUTION_SUPERVISOR_LISTENER_FD_V1,
            COMPILER_EXECUTION_SUPERVISOR_ROOT_FD_V1,
            COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_FD_V1,
            COMPILER_EXECUTION_SUPERVISOR_ISSUER_FD_V1,
            COMPILER_EXECUTION_SUPERVISOR_POLICY_FD_V1,
            COMPILER_EXECUTION_SUPERVISOR_SIGNING_KEY_FD_V1,
            COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PEER_FD_V1,
            COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PIDFD_V1,
            COMPILER_EXECUTION_SUPERVISOR_BOOTSTRAP_FD_V1,
            COMPILER_EXECUTION_SUPERVISOR_LIFECYCLE_FD_V1,
            COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1,
        ];
        for (index, destination) in destinations.iter().enumerate() {
            assert!(*destination >= 3);
            assert!(
                *destination
                    < fe2o3_protected_service_spawn::PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1
            );
            assert!(!destinations[..index].contains(destination));
        }
    }

    #[test]
    fn private_bootstrap_accepts_only_exact_readiness_then_eof() {
        let deployment = deployment(7);
        let (_process, mut child) = live_test_child();
        let (receiver, writer) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        let ready = CompilerExecutionSupervisorReadyV1::new(
            u32::try_from(child.pid().as_raw_pid()).unwrap(),
            &deployment,
        )
        .unwrap();
        assert_eq!(
            rustix::net::send(&writer, ready.canonical_bytes(), SendFlags::NOSIGNAL).unwrap(),
            ready.canonical_bytes().len()
        );
        drop(writer);
        let observed = receive_ready(
            &receiver,
            &child,
            &deployment,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(observed, ready);
        await_bootstrap_eof(&receiver, &child, Instant::now() + Duration::from_secs(1)).unwrap();
        child.cancel_and_reap().unwrap();
    }

    #[test]
    fn private_bootstrap_rejects_stage_pid_and_trailing_substitution() {
        let deployment = deployment(7);

        let (_process, mut child) = live_test_child();
        let (receiver, writer) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        rustix::net::send(&writer, &[0xc1], SendFlags::NOSIGNAL).unwrap();
        assert!(matches!(
            receive_ready(
                &receiver,
                &child,
                &deployment,
                Instant::now() + Duration::from_secs(1)
            ),
            Err(CompilerExecutionCoordinatorErrorV1::ChildStage(0xc1))
        ));
        child.cancel_and_reap().unwrap();

        let (_process, mut child) = live_test_child();
        let (receiver, writer) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        let wrong_pid = u32::try_from(child.pid().as_raw_pid()).unwrap() + 1;
        let ready = CompilerExecutionSupervisorReadyV1::new(wrong_pid, &deployment).unwrap();
        rustix::net::send(&writer, ready.canonical_bytes(), SendFlags::NOSIGNAL).unwrap();
        assert!(matches!(
            receive_ready(
                &receiver,
                &child,
                &deployment,
                Instant::now() + Duration::from_secs(1)
            ),
            Err(CompilerExecutionCoordinatorErrorV1::ReadyMismatch)
        ));
        child.cancel_and_reap().unwrap();

        let (_process, mut child) = live_test_child();
        let (receiver, writer) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        let ready = CompilerExecutionSupervisorReadyV1::new(
            u32::try_from(child.pid().as_raw_pid()).unwrap(),
            &deployment,
        )
        .unwrap();
        rustix::net::send(&writer, ready.canonical_bytes(), SendFlags::NOSIGNAL).unwrap();
        rustix::net::send(&writer, b"trailing", SendFlags::NOSIGNAL).unwrap();
        receive_ready(
            &receiver,
            &child,
            &deployment,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert!(matches!(
            await_bootstrap_eof(&receiver, &child, Instant::now() + Duration::from_secs(1)),
            Err(CompilerExecutionCoordinatorErrorV1::MalformedReadyEof)
        ));
        child.cancel_and_reap().unwrap();
    }

    fn live_test_child() -> (Child, RootOwnedProtectedServiceChildV1) {
        let child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let pid = rustix::process::Pid::from_raw(i32::try_from(child.id()).unwrap()).unwrap();
        let pidfd = rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()).unwrap();
        let custody =
            RootOwnedProtectedServiceChildV1::admit_non_authoritative_test(pid, pidfd).unwrap();
        (child, custody)
    }

    fn deployment(seed: u8) -> CompilerExecutionSupervisorDeploymentV1 {
        let policy = CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 4_096).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 8_192).unwrap(),
            SigningKey::from_bytes(&[seed; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[seed.wrapping_add(1); 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap();
        CompilerExecutionSupervisorDeploymentV1::new(
            1_001,
            1_002,
            CompilerExecutionExternalAnchorServiceIdentityV1::new(2_001, 2_002).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 3; 32], 12_288).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 4; 32], 16_384).unwrap(),
            &policy,
        )
        .unwrap()
    }
}
