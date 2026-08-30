#![deny(missing_docs, unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("fe2o3-external-anchor-coordinator requires Linux x86-64");

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, OwnedFd};
use std::time::{Duration, Instant};

use fe2o3_broker_authority_service::{
    ProtectedExternalAnchorServiceAdmissionV1, ProtectedServiceAdmissionErrorV1,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionExternalAnchorDeploymentCapabilityV1,
    CompilerExecutionExternalAnchorProvisioningCapabilityV1,
    CompilerExecutionExternalAnchorSigningKeyCapabilityV1,
};
use fe2o3_compiler_execution_lifecycle::{
    CompilerExecutionServiceLifecycleLeaseV1, LifecycleLeaseErrorV1,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentIdentityV1,
    CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionExternalAnchorProvisioningV1,
    CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerPolicyIdentityV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentIdentityV1,
    CompilerExecutionSupervisorDeploymentV1,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
};
use fe2o3_external_anchor_provisioner::{
    EXTERNAL_ANCHOR_HELPER_LIFECYCLE_FD_V1, EXTERNAL_ANCHOR_PROVISIONING_READY_BYTES_V1,
    ExternalAnchorProvisioningReadyDispositionV1, ExternalAnchorProvisioningReadyV1,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileErrorV1, ProtectedServiceCredentialProfileV1,
    ProtectedServiceNamespaceSetV1, ProtectedServiceProfileErrorV1,
    validate_protected_service_process_v1,
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
use rustix::fs::{FileType, OFlags};
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SocketFlags,
    SocketType, recv, recvmsg, socketpair,
};
use rustix::pipe::{PipeFlags, pipe_with};

const MAX_LAUNCH_TIMEOUT_V1: Duration = Duration::from_secs(120);
const POLL_INTERVAL_V1: Duration = Duration::from_millis(1);
const STATE_ROOT_MODE_V1: u32 = 0o700;

/// Immutable root-prepared inputs for one exact external-anchor occurrence.
///
/// This value is move-only and exposes no descriptor, key, process, compiler, publication, load,
/// launch, or GPU operation other than consuming the complete set into [`Self::launch`].
pub struct PreparedExternalAnchorOccurrenceV1 {
    helper: ProtectedStaticExecutableV1,
    daemon: ProtectedStaticExecutableV1,
    root: File,
    lifecycle: CompilerExecutionServiceLifecycleLeaseV1,
    deployment: CompilerExecutionExternalAnchorDeploymentCapabilityV1,
    provisioning: CompilerExecutionExternalAnchorProvisioningCapabilityV1,
    key_template: CompilerExecutionExternalAnchorSigningKeyCapabilityV1,
    deployment_manifest: CompilerExecutionExternalAnchorDeploymentV1,
    provisioning_manifest: CompilerExecutionExternalAnchorProvisioningV1,
    root_snapshot: StateRootSnapshotV1,
    prepared_by: rustix::process::Pid,
}

impl fmt::Debug for PreparedExternalAnchorOccurrenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedExternalAnchorOccurrenceV1")
            .field("authority", &"root-owned-anchor-launch-custody-only")
            .field("service", &self.deployment_manifest.service())
            .field("deployment", &self.deployment_manifest.identity())
            .field("provisioning", &self.provisioning_manifest.identity())
            .finish_non_exhaustive()
    }
}

impl PreparedExternalAnchorOccurrenceV1 {
    /// Seals and binds every input for one root-supervised external-anchor occurrence.
    ///
    /// The caller must be UID/GID 0. `helper_source` and `daemon_source` are measured against the
    /// canonical provisioning and deployment manifests and copied into anonymous mode-0555
    /// service-owned executable images. `root` must already be the exact service-owned mode-0700
    /// state directory. `key_template` must be root-owned and bound to `deployment`.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        helper_source: File,
        daemon_source: File,
        root: File,
        lifecycle: CompilerExecutionServiceLifecycleLeaseV1,
        deployment: CompilerExecutionExternalAnchorDeploymentV1,
        provisioning: CompilerExecutionExternalAnchorProvisioningV1,
        key_template: CompilerExecutionExternalAnchorSigningKeyCapabilityV1,
    ) -> Result<Self, ExternalAnchorCoordinatorErrorV1> {
        Self::prepare_inner::<true>(
            helper_source,
            daemon_source,
            root,
            lifecycle,
            deployment,
            provisioning,
            key_template,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_inner<const REQUIRE_ROOT: bool>(
        helper_source: File,
        daemon_source: File,
        root: File,
        lifecycle: CompilerExecutionServiceLifecycleLeaseV1,
        deployment: CompilerExecutionExternalAnchorDeploymentV1,
        provisioning: CompilerExecutionExternalAnchorProvisioningV1,
        key_template: CompilerExecutionExternalAnchorSigningKeyCapabilityV1,
    ) -> Result<Self, ExternalAnchorCoordinatorErrorV1> {
        require_coordinator_identity::<REQUIRE_ROOT>()?;
        if !provisioning.matches_deployment(&deployment) {
            return Err(ExternalAnchorCoordinatorErrorV1::ProvisioningMismatch);
        }
        let service = deployment.service();
        let credentials = ProtectedServiceCredentialProfileV1::new(service.uid(), service.gid())
            .map_err(ExternalAnchorCoordinatorErrorV1::Credentials)?;
        let owner = ProtectedStaticExecutableOwnerV1::new(credentials.uid(), credentials.gid())
            .map_err(ExternalAnchorCoordinatorErrorV1::Executable)?;
        let helper_measurement = provisioning.helper();
        let helper = ProtectedStaticExecutableV1::seal_source_for_owner(
            helper_source,
            ProtectedStaticExecutableMeasurementV1::new(
                helper_measurement.sha256(),
                helper_measurement.byte_len(),
                MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
            )
            .map_err(ExternalAnchorCoordinatorErrorV1::Executable)?,
            owner,
            "external-anchor provisioning helper",
        )
        .map_err(ExternalAnchorCoordinatorErrorV1::Executable)?;
        let daemon_measurement = deployment.executable();
        let daemon = ProtectedStaticExecutableV1::seal_source_for_owner(
            daemon_source,
            ProtectedStaticExecutableMeasurementV1::new(
                daemon_measurement.sha256(),
                daemon_measurement.byte_len(),
                MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
            )
            .map_err(ExternalAnchorCoordinatorErrorV1::Executable)?,
            owner,
            "external-anchor daemon",
        )
        .map_err(ExternalAnchorCoordinatorErrorV1::Executable)?;
        let root_snapshot = validate_state_root(&root, service)?;
        lifecycle
            .revalidate()
            .map_err(ExternalAnchorCoordinatorErrorV1::Lifecycle)?;
        key_template
            .revalidate(&deployment)
            .map_err(ExternalAnchorCoordinatorErrorV1::KeyTemplate)?;
        if key_template.verifying_key() != *deployment.verifying_key() {
            return Err(ExternalAnchorCoordinatorErrorV1::KeyMismatch);
        }
        let deployment_capability =
            CompilerExecutionExternalAnchorDeploymentCapabilityV1::create(deployment.clone())
                .map_err(ExternalAnchorCoordinatorErrorV1::DeploymentCapability)?;
        let provisioning_capability =
            CompilerExecutionExternalAnchorProvisioningCapabilityV1::create(provisioning.clone())
                .map_err(ExternalAnchorCoordinatorErrorV1::ProvisioningCapability)?;
        let prepared = Self {
            helper,
            daemon,
            root,
            lifecycle,
            deployment: deployment_capability,
            provisioning: provisioning_capability,
            key_template,
            deployment_manifest: deployment,
            provisioning_manifest: provisioning,
            root_snapshot,
            prepared_by: rustix::process::getpid(),
        };
        prepared.revalidate_inner::<REQUIRE_ROOT>()?;
        Ok(prepared)
    }

    /// Launches the measured helper and returns root-owned live daemon custody.
    ///
    /// The child cannot execute the helper until it has established the complete locked service
    /// profile and the parent has independently checked proc-visible profile and namespace facts.
    /// Success additionally requires the canonical ready record, one exact endpoint, bootstrap
    /// close-on-exec EOF, pidfd liveness, and protected endpoint admission against the deployment.
    pub fn launch(
        self,
        timeout: Duration,
    ) -> Result<RootManagedExternalAnchorV1, ExternalAnchorCoordinatorErrorV1> {
        let deadline = bounded_deadline(timeout)?;
        self.revalidate_inner::<true>()?;
        let namespaces = ProtectedServiceNamespaceSetV1::capture_self()
            .map_err(ExternalAnchorCoordinatorErrorV1::Profile)?;
        let credentials = ProtectedServiceCredentialProfileV1::new(
            self.deployment_manifest.service().uid(),
            self.deployment_manifest.service().gid(),
        )
        .map_err(ExternalAnchorCoordinatorErrorV1::Credentials)?;

        let (root_bootstrap, child_bootstrap) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|source| io_error("create root-to-helper bootstrap", source.into()))?;
        let (profile_reader, profile_writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)
            .map_err(|source| io_error("create child-profile channel", source.into()))?;
        let (gate_reader, gate_writer) = pipe_with(PipeFlags::CLOEXEC)
            .map_err(|source| io_error("create child release gate", source.into()))?;

        let helper = self
            .helper
            .try_clone_for_exec()
            .map_err(ExternalAnchorCoordinatorErrorV1::Executable)?;
        let daemon = self
            .daemon
            .try_clone_for_exec()
            .map_err(ExternalAnchorCoordinatorErrorV1::Executable)?;
        let deployment = self
            .deployment
            .try_clone_for_transfer()
            .map_err(ExternalAnchorCoordinatorErrorV1::DeploymentCapability)?;
        let provisioning = self
            .provisioning
            .try_clone_for_transfer()
            .map_err(ExternalAnchorCoordinatorErrorV1::ProvisioningCapability)?;
        let key_template = self
            .key_template
            .try_clone_for_transfer()
            .map_err(ExternalAnchorCoordinatorErrorV1::KeyTemplate)?;
        self.lifecycle
            .revalidate()
            .map_err(ExternalAnchorCoordinatorErrorV1::Lifecycle)?;
        let bindings = [
            ProtectedServiceDescriptorBindingV1::new(
                child_bootstrap.as_fd(),
                fe2o3_external_anchor_provisioner::EXTERNAL_ANCHOR_HELPER_BOOTSTRAP_FD_V1,
            )
            .map_err(ExternalAnchorCoordinatorErrorV1::Spawn)?,
            ProtectedServiceDescriptorBindingV1::new(
                self.root.as_fd(),
                fe2o3_external_anchor_provisioner::EXTERNAL_ANCHOR_HELPER_ROOT_FD_V1,
            )
            .map_err(ExternalAnchorCoordinatorErrorV1::Spawn)?,
            ProtectedServiceDescriptorBindingV1::new(
                self.lifecycle.as_fd(),
                EXTERNAL_ANCHOR_HELPER_LIFECYCLE_FD_V1,
            )
            .map_err(ExternalAnchorCoordinatorErrorV1::Spawn)?,
            ProtectedServiceDescriptorBindingV1::new(
                daemon.as_fd(),
                fe2o3_external_anchor_provisioner::EXTERNAL_ANCHOR_HELPER_DAEMON_EXECUTABLE_FD_V1,
            )
            .map_err(ExternalAnchorCoordinatorErrorV1::Spawn)?,
            ProtectedServiceDescriptorBindingV1::new(
                deployment.as_fd(),
                fe2o3_compiler_closure_capability::COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1,
            )
            .map_err(ExternalAnchorCoordinatorErrorV1::Spawn)?,
            ProtectedServiceDescriptorBindingV1::new(
                key_template.as_fd(),
                fe2o3_compiler_closure_capability::COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1,
            )
            .map_err(ExternalAnchorCoordinatorErrorV1::Spawn)?,
            ProtectedServiceDescriptorBindingV1::new(
                provisioning.as_fd(),
                fe2o3_compiler_closure_capability::COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_FD_V1,
            )
            .map_err(ExternalAnchorCoordinatorErrorV1::Spawn)?,
        ];
        let staged = StagedProtectedServiceExecV1::new(
            &helper,
            &bindings,
            profile_writer.as_fd(),
            gate_reader.as_fd(),
            child_bootstrap.as_fd(),
        )
        .map_err(ExternalAnchorCoordinatorErrorV1::Spawn)?;
        let spawned = fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| {
            staged
                .spawn(credentials)
                .map_err(ExternalAnchorCoordinatorErrorV1::Spawn)
        })?;
        let mut child = RootOwnedAnchorChildV1(spawned);
        drop(staged);
        drop(child_bootstrap);
        drop(profile_writer);
        drop(gate_reader);

        let result = (|| {
            await_profile_ready(&profile_reader, &root_bootstrap, &child, deadline)?;
            namespaces
                .revalidate_self()
                .map_err(ExternalAnchorCoordinatorErrorV1::Profile)?;
            namespaces
                .revalidate_process(child.pid())
                .map_err(ExternalAnchorCoordinatorErrorV1::Profile)?;
            validate_protected_service_process_v1(credentials, child.pid())
                .map_err(ExternalAnchorCoordinatorErrorV1::Profile)?;
            self.revalidate_inner::<true>()?;
            release_child(&gate_writer)?;
            drop(gate_writer);

            let (ready, endpoint) = receive_ready(&root_bootstrap, &child, deadline)?;
            await_exec_eof(&root_bootstrap, &child, deadline)?;
            if !child.is_live()? {
                return Err(child.exited_error("daemon exited immediately after exec"));
            }
            let admission_pidfd = child.try_clone_pidfd()?;
            let admission = ProtectedExternalAnchorServiceAdmissionV1::admit(
                endpoint,
                admission_pidfd,
                self.deployment_manifest.service(),
            )
            .map_err(ExternalAnchorCoordinatorErrorV1::Admission)?;
            admission
                .validate_continuity()
                .map_err(ExternalAnchorCoordinatorErrorV1::Admission)?;
            Ok((admission, ready.disposition()))
        })();
        match result {
            Ok((admission, disposition)) => Ok(RootManagedExternalAnchorV1 {
                admission,
                child,
                disposition,
                deployment: self.deployment_manifest,
            }),
            Err(error) => match child.cancel_and_reap() {
                Ok(()) => Err(error),
                Err(cleanup) => Err(cleanup),
            },
        }
    }

    fn revalidate_inner<const REQUIRE_ROOT: bool>(
        &self,
    ) -> Result<(), ExternalAnchorCoordinatorErrorV1> {
        require_coordinator_identity::<REQUIRE_ROOT>()?;
        if rustix::process::getpid() != self.prepared_by {
            return Err(ExternalAnchorCoordinatorErrorV1::CoordinatorChanged);
        }
        if !self
            .provisioning_manifest
            .matches_deployment(&self.deployment_manifest)
        {
            return Err(ExternalAnchorCoordinatorErrorV1::ProvisioningMismatch);
        }
        self.helper
            .revalidate()
            .map_err(ExternalAnchorCoordinatorErrorV1::Executable)?;
        self.daemon
            .revalidate()
            .map_err(ExternalAnchorCoordinatorErrorV1::Executable)?;
        if validate_state_root(&self.root, self.deployment_manifest.service())?
            != self.root_snapshot
        {
            return Err(ExternalAnchorCoordinatorErrorV1::StateRootChanged);
        }
        self.lifecycle
            .revalidate()
            .map_err(ExternalAnchorCoordinatorErrorV1::Lifecycle)?;
        self.deployment
            .revalidate()
            .map_err(ExternalAnchorCoordinatorErrorV1::DeploymentCapability)?;
        self.provisioning
            .revalidate()
            .map_err(ExternalAnchorCoordinatorErrorV1::ProvisioningCapability)?;
        self.key_template
            .revalidate(&self.deployment_manifest)
            .map_err(ExternalAnchorCoordinatorErrorV1::KeyTemplate)?;
        if self.key_template.verifying_key() != *self.deployment_manifest.verifying_key() {
            return Err(ExternalAnchorCoordinatorErrorV1::KeyMismatch);
        }
        Ok(())
    }
}

/// Root-retained custody of one admitted live external-anchor occurrence.
///
/// Dropping this value sends `SIGKILL` through the retained pidfd and synchronously reaps the exact
/// direct child. Supervisor transfers duplicate only the already admitted endpoint and pidfd; the
/// root coordinator keeps independent lifecycle and reaping custody.
pub struct RootManagedExternalAnchorV1 {
    admission: ProtectedExternalAnchorServiceAdmissionV1,
    child: RootOwnedAnchorChildV1,
    disposition: ExternalAnchorProvisioningReadyDispositionV1,
    deployment: CompilerExecutionExternalAnchorDeploymentV1,
}

impl fmt::Debug for RootManagedExternalAnchorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RootManagedExternalAnchorV1")
            .field("authority", &"root-lifecycle-custody-only")
            .field("service", &self.admission.service_identity())
            .field("process", &self.admission.service_process_identity())
            .field("disposition", &self.disposition)
            .field("deployment", &self.deployment.identity())
            .finish_non_exhaustive()
    }
}

impl RootManagedExternalAnchorV1 {
    /// Returns whether this occurrence opened existing state or initialized genesis.
    pub const fn disposition(&self) -> ExternalAnchorProvisioningReadyDispositionV1 {
        self.disposition
    }

    /// Returns the canonical deployment retained with this live occurrence.
    pub const fn deployment(&self) -> &CompilerExecutionExternalAnchorDeploymentV1 {
        &self.deployment
    }

    /// Revalidates the exact endpoint, pidfd target, credentials, and point-in-time liveness.
    pub fn validate_continuity(&self) -> Result<(), ExternalAnchorCoordinatorErrorV1> {
        self.validate_continuity_inner::<true>()
    }

    fn validate_continuity_inner<const REQUIRE_ROOT: bool>(
        &self,
    ) -> Result<(), ExternalAnchorCoordinatorErrorV1> {
        require_coordinator_identity::<REQUIRE_ROOT>()?;
        if !self.child.is_live()? {
            return Err(self.child.exited_error("managed anchor is not live"));
        }
        self.admission
            .validate_continuity()
            .map_err(ExternalAnchorCoordinatorErrorV1::Admission)
    }

    /// Clones one admitted endpoint/pidfd pair for a protected supervisor transfer.
    pub fn try_clone_for_supervisor(
        &self,
        supervisor: &CompilerExecutionSupervisorDeploymentV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<ExternalAnchorSupervisorTransferV1, ExternalAnchorCoordinatorErrorV1> {
        self.try_clone_for_supervisor_inner::<true>(supervisor, policy)
    }

    fn try_clone_for_supervisor_inner<const REQUIRE_ROOT: bool>(
        &self,
        supervisor: &CompilerExecutionSupervisorDeploymentV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<ExternalAnchorSupervisorTransferV1, ExternalAnchorCoordinatorErrorV1> {
        self.validate_continuity_inner::<REQUIRE_ROOT>()?;
        if !self
            .deployment
            .matches_supervisor_and_policy(supervisor, policy)
        {
            return Err(ExternalAnchorCoordinatorErrorV1::SupervisorBindingMismatch);
        }
        let (endpoint, pidfd) = self
            .admission
            .try_clone_for_transfer()
            .map_err(ExternalAnchorCoordinatorErrorV1::Admission)?;
        self.validate_continuity_inner::<REQUIRE_ROOT>()?;
        Ok(ExternalAnchorSupervisorTransferV1 {
            endpoint,
            pidfd,
            service: self.admission.service_identity(),
            deployment: self.deployment.identity(),
            supervisor: supervisor.identity(),
            policy: policy.identity(),
        })
    }

    /// Terminates and exactly once reaps this occurrence.
    pub fn shutdown(mut self) -> Result<(), ExternalAnchorCoordinatorErrorV1> {
        self.child.cancel_and_reap()
    }
}

impl Drop for RootManagedExternalAnchorV1 {
    fn drop(&mut self) {
        let _ = self.child.cancel_and_reap();
    }
}

/// Move-only admitted endpoint and pidfd prepared for one protected-supervisor transfer.
pub struct ExternalAnchorSupervisorTransferV1 {
    endpoint: OwnedFd,
    pidfd: OwnedFd,
    service: CompilerExecutionExternalAnchorServiceIdentityV1,
    deployment: CompilerExecutionExternalAnchorDeploymentIdentityV1,
    supervisor: CompilerExecutionSupervisorDeploymentIdentityV1,
    policy: CompilerExecutionIssuerPolicyIdentityV1,
}

impl fmt::Debug for ExternalAnchorSupervisorTransferV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalAnchorSupervisorTransferV1")
            .field("authority", &"descriptor-transfer-only")
            .field("service", &self.service)
            .field("deployment", &self.deployment)
            .field("supervisor", &self.supervisor)
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl ExternalAnchorSupervisorTransferV1 {
    /// Returns the deployment-pinned service identity carried with this pair.
    pub const fn service(&self) -> CompilerExecutionExternalAnchorServiceIdentityV1 {
        self.service
    }

    /// Returns the exact external-anchor deployment identity authorized for this transfer.
    pub const fn deployment_identity(&self) -> CompilerExecutionExternalAnchorDeploymentIdentityV1 {
        self.deployment
    }

    /// Returns the exact supervisor deployment identity authorized to receive this transfer.
    pub const fn supervisor_identity(&self) -> CompilerExecutionSupervisorDeploymentIdentityV1 {
        self.supervisor
    }

    /// Returns the exact issuer-policy identity shared by the anchor and supervisor deployments.
    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        self.policy
    }

    /// Consumes the transfer into the two ordered close-on-exec descriptors.
    ///
    /// The receiver must immediately call [`ProtectedExternalAnchorServiceAdmissionV1::admit`]
    /// under the protected supervisor UID before binding launch custody.
    pub fn into_ordered_descriptors(self) -> (OwnedFd, OwnedFd) {
        (self.endpoint, self.pidfd)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StateRootSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
}

fn validate_state_root(
    root: &File,
    service: CompilerExecutionExternalAnchorServiceIdentityV1,
) -> Result<StateRootSnapshotV1, ExternalAnchorCoordinatorErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(root)
        .map_err(|source| io_error("inspect anchor state-root descriptor", source.into()))?;
    let status = rustix::fs::fcntl_getfl(root)
        .map_err(|source| io_error("inspect anchor state-root status", source.into()))?;
    let stat = rustix::fs::fstat(root)
        .map_err(|source| io_error("inspect anchor state root", source.into()))?;
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.intersects(forbidden)
        || FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_mode & 0o7777 != STATE_ROOT_MODE_V1
        || stat.st_uid != service.uid()
        || stat.st_gid != service.gid()
        || stat.st_nlink == 0
    {
        return Err(ExternalAnchorCoordinatorErrorV1::InvalidStateRoot);
    }
    Ok(StateRootSnapshotV1 {
        device: stat.st_dev,
        inode: stat.st_ino,
        mode: stat.st_mode,
        uid: stat.st_uid,
        gid: stat.st_gid,
        links: stat.st_nlink,
    })
}

struct RootOwnedAnchorChildV1(RootOwnedProtectedServiceChildV1);

impl RootOwnedAnchorChildV1 {
    #[cfg(test)]
    fn new(pid: rustix::process::Pid, pidfd: OwnedFd) -> Self {
        Self(RootOwnedProtectedServiceChildV1::admit_non_authoritative_test(pid, pidfd).unwrap())
    }

    fn pid(&self) -> rustix::process::Pid {
        self.0.pid()
    }

    fn try_clone_pidfd(&self) -> Result<OwnedFd, ExternalAnchorCoordinatorErrorV1> {
        self.0
            .try_clone_pidfd()
            .map_err(ExternalAnchorCoordinatorErrorV1::Spawn)
    }

    fn is_live(&self) -> Result<bool, ExternalAnchorCoordinatorErrorV1> {
        self.0
            .is_live()
            .map_err(ExternalAnchorCoordinatorErrorV1::Spawn)
    }

    fn exited_error(&self, context: &'static str) -> ExternalAnchorCoordinatorErrorV1 {
        ExternalAnchorCoordinatorErrorV1::ChildExited(self.0.exit_description(context))
    }

    fn cancel_and_reap(&mut self) -> Result<(), ExternalAnchorCoordinatorErrorV1> {
        self.0
            .cancel_and_reap()
            .map_err(ExternalAnchorCoordinatorErrorV1::Spawn)
    }
}

fn await_profile_ready(
    profile: &OwnedFd,
    bootstrap: &OwnedFd,
    child: &RootOwnedAnchorChildV1,
    deadline: Instant,
) -> Result<(), ExternalAnchorCoordinatorErrorV1> {
    let mut bytes = [0_u8; 2];
    loop {
        match rustix::io::read(profile, &mut bytes) {
            Ok(1) if bytes[0] == PROTECTED_SERVICE_PROFILE_READY_V1 => return Ok(()),
            Ok(0) => return Err(child_failure(bootstrap, child, "child profile")),
            Ok(_) => return Err(ExternalAnchorCoordinatorErrorV1::NoncanonicalProfileReady),
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                await_profile_progress(bootstrap, child, deadline)?;
            }
            Err(source) => return Err(io_error("read child-profile record", source.into())),
        }
    }
}

fn release_child(gate: &OwnedFd) -> Result<(), ExternalAnchorCoordinatorErrorV1> {
    loop {
        match rustix::io::write(gate, &[PROTECTED_SERVICE_GATE_RELEASE_V1]) {
            Ok(1) => return Ok(()),
            Ok(_) => return Err(ExternalAnchorCoordinatorErrorV1::NoncanonicalGateRelease),
            Err(rustix::io::Errno::INTR) => {}
            Err(source) => return Err(io_error("release measured helper child", source.into())),
        }
    }
}

fn receive_ready(
    bootstrap: &OwnedFd,
    child: &RootOwnedAnchorChildV1,
    deadline: Instant,
) -> Result<(ExternalAnchorProvisioningReadyV1, OwnedFd), ExternalAnchorCoordinatorErrorV1> {
    loop {
        let mut payload = [0_u8; EXTERNAL_ANCHOR_PROVISIONING_READY_BYTES_V1];
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
                let mut descriptors = Vec::with_capacity(1);
                for message in ancillary.drain() {
                    match message {
                        RecvAncillaryMessage::ScmRights(received) => descriptors.extend(received),
                        _ => {
                            return Err(ExternalAnchorCoordinatorErrorV1::MalformedReadyTransfer);
                        }
                    }
                }
                if received.bytes == 1 && descriptors.is_empty() {
                    return Err(ExternalAnchorCoordinatorErrorV1::ChildStage(payload[0]));
                }
                if received.bytes != payload.len()
                    || received
                        .flags
                        .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
                {
                    return Err(ExternalAnchorCoordinatorErrorV1::MalformedReadyTransfer);
                }
                if descriptors.len() != 1 {
                    return Err(ExternalAnchorCoordinatorErrorV1::MalformedReadyTransfer);
                }
                let ready = ExternalAnchorProvisioningReadyV1::decode(&payload)
                    .map_err(|_| ExternalAnchorCoordinatorErrorV1::MalformedReadyTransfer)?;
                return Ok((ready, descriptors.pop().expect("length checked")));
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                await_child_progress(child, deadline, "helper-ready transfer")?;
            }
            Err(source) => return Err(io_error("receive helper-ready transfer", source.into())),
        }
    }
}

fn await_exec_eof(
    bootstrap: &OwnedFd,
    child: &RootOwnedAnchorChildV1,
    deadline: Instant,
) -> Result<(), ExternalAnchorCoordinatorErrorV1> {
    let mut payload = [0_u8; 2];
    loop {
        match recv(bootstrap, &mut payload, RecvFlags::DONTWAIT) {
            Ok((0, 0)) => return Ok(()),
            Ok((1, _)) => return Err(ExternalAnchorCoordinatorErrorV1::ChildStage(payload[0])),
            Ok(_) => return Err(ExternalAnchorCoordinatorErrorV1::MalformedExecStatus),
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                await_child_progress(child, deadline, "daemon exec EOF")?;
            }
            Err(source) => return Err(io_error("observe daemon exec EOF", source.into())),
        }
    }
}

fn await_profile_progress(
    bootstrap: &OwnedFd,
    child: &RootOwnedAnchorChildV1,
    deadline: Instant,
) -> Result<(), ExternalAnchorCoordinatorErrorV1> {
    if let Some(stage) = receive_child_stage(bootstrap)? {
        return Err(ExternalAnchorCoordinatorErrorV1::ChildStage(stage));
    }
    if !child.is_live()? {
        return Err(child.exited_error("child profile"));
    }
    if Instant::now() >= deadline {
        return Err(ExternalAnchorCoordinatorErrorV1::Timeout("child profile"));
    }
    std::thread::sleep(POLL_INTERVAL_V1);
    Ok(())
}

fn await_child_progress(
    child: &RootOwnedAnchorChildV1,
    deadline: Instant,
    boundary: &'static str,
) -> Result<(), ExternalAnchorCoordinatorErrorV1> {
    if !child.is_live()? {
        return Err(child.exited_error(boundary));
    }
    if Instant::now() >= deadline {
        return Err(ExternalAnchorCoordinatorErrorV1::Timeout(boundary));
    }
    std::thread::sleep(POLL_INTERVAL_V1);
    Ok(())
}

fn receive_child_stage(
    bootstrap: &OwnedFd,
) -> Result<Option<u8>, ExternalAnchorCoordinatorErrorV1> {
    let mut payload = [0_u8; 2];
    match recv(bootstrap, &mut payload, RecvFlags::DONTWAIT) {
        Ok((0, 0)) => Ok(None),
        Ok((1, _)) => Ok(Some(payload[0])),
        Ok(_) => Err(ExternalAnchorCoordinatorErrorV1::MalformedExecStatus),
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => Ok(None),
        Err(source) => Err(io_error("read child failure stage", source.into())),
    }
}

fn child_failure(
    bootstrap: &OwnedFd,
    child: &RootOwnedAnchorChildV1,
    boundary: &'static str,
) -> ExternalAnchorCoordinatorErrorV1 {
    match receive_child_stage(bootstrap) {
        Ok(Some(stage)) => ExternalAnchorCoordinatorErrorV1::ChildStage(stage),
        _ => child.exited_error(boundary),
    }
}

fn require_coordinator_identity<const REQUIRE_ROOT: bool>()
-> Result<(), ExternalAnchorCoordinatorErrorV1> {
    if REQUIRE_ROOT {
        require_exact_root_identity_v1().map_err(ExternalAnchorCoordinatorErrorV1::Spawn)?;
    }
    Ok(())
}

fn bounded_deadline(timeout: Duration) -> Result<Instant, ExternalAnchorCoordinatorErrorV1> {
    if timeout.is_zero() || timeout > MAX_LAUNCH_TIMEOUT_V1 {
        return Err(ExternalAnchorCoordinatorErrorV1::InvalidTimeout);
    }
    Instant::now()
        .checked_add(timeout)
        .ok_or(ExternalAnchorCoordinatorErrorV1::InvalidTimeout)
}

fn io_error(operation: &'static str, source: io::Error) -> ExternalAnchorCoordinatorErrorV1 {
    ExternalAnchorCoordinatorErrorV1::Io { operation, source }
}

/// Stable failure preparing, launching, admitting, or reaping an external-anchor occurrence.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExternalAnchorCoordinatorErrorV1 {
    /// The coordinator PID changed after input preparation.
    CoordinatorChanged,
    /// The deployment contains an invalid protected-service identity.
    Credentials(ProtectedServiceCredentialProfileErrorV1),
    /// Provisioning names another deployment.
    ProvisioningMismatch,
    /// Helper or daemon executable admission failed.
    Executable(ProtectedStaticExecutableErrorV1),
    /// The state root is not an exact service-owned mode-0700 directory descriptor.
    InvalidStateRoot,
    /// The retained state-root identity or metadata changed.
    StateRootChanged,
    /// The deployment capability is invalid or changed.
    DeploymentCapability(String),
    /// The provisioning capability is invalid or changed.
    ProvisioningCapability(String),
    /// The root signing-key template is invalid or changed.
    KeyTemplate(String),
    /// Key template and deployment verification key disagree.
    KeyMismatch,
    /// Parent or child protected-service profile validation failed.
    Profile(ProtectedServiceProfileErrorV1),
    /// Crash-retained child lifecycle custody is invalid or changed.
    Lifecycle(LifecycleLeaseErrorV1),
    /// Shared protected-service staging, spawn, or pidfd custody failed.
    Spawn(ProtectedServiceSpawnErrorV1),
    /// The bounded launch timeout is zero, excessive, or unrepresentable.
    InvalidTimeout,
    /// The gated child emitted a noncanonical profile record.
    NoncanonicalProfileReady,
    /// The gated child release was not exact.
    NoncanonicalGateRelease,
    /// Ready bytes, packet shape, or ancillary descriptors were not exact.
    MalformedReadyTransfer,
    /// Bootstrap EOF carried a noncanonical packet.
    MalformedExecStatus,
    /// The child or helper reported one fail-closed stage byte.
    ChildStage(u8),
    /// The exact child exited before the authenticated boundary completed.
    ChildExited(String),
    /// A bounded launch boundary timed out.
    Timeout(&'static str),
    /// Endpoint/pidfd protected admission failed.
    Admission(ProtectedServiceAdmissionErrorV1),
    /// The receiving supervisor deployment or issuer policy does not name this exact anchor.
    SupervisorBindingMismatch,
    /// A bounded kernel or filesystem operation failed.
    Io {
        /// Operation that failed.
        operation: &'static str,
        /// Kernel or filesystem error.
        source: io::Error,
    },
}

impl fmt::Display for ExternalAnchorCoordinatorErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinatorChanged => formatter.write_str("coordinator process identity changed"),
            Self::Credentials(error) => write!(formatter, "invalid anchor credentials: {error}"),
            Self::ProvisioningMismatch => {
                formatter.write_str("anchor provisioning names another deployment")
            }
            Self::Executable(error) => write!(formatter, "invalid anchor executable: {error}"),
            Self::InvalidStateRoot => formatter.write_str("invalid external-anchor state root"),
            Self::StateRootChanged => formatter.write_str("external-anchor state root changed"),
            Self::DeploymentCapability(error) => {
                write!(formatter, "invalid anchor deployment capability: {error}")
            }
            Self::ProvisioningCapability(error) => {
                write!(formatter, "invalid anchor provisioning capability: {error}")
            }
            Self::KeyTemplate(error) => write!(formatter, "invalid anchor key template: {error}"),
            Self::KeyMismatch => formatter.write_str("anchor key does not match deployment"),
            Self::Profile(error) => write!(formatter, "invalid anchor child profile: {error}"),
            Self::Lifecycle(error) => write!(formatter, "invalid anchor lifecycle: {error}"),
            Self::Spawn(error) => write!(formatter, "protected anchor spawn failed: {error}"),
            Self::InvalidTimeout => formatter.write_str("invalid anchor launch timeout"),
            Self::NoncanonicalProfileReady => {
                formatter.write_str("noncanonical anchor child-profile record")
            }
            Self::NoncanonicalGateRelease => {
                formatter.write_str("noncanonical anchor child gate release")
            }
            Self::MalformedReadyTransfer => {
                formatter.write_str("malformed anchor helper-ready transfer")
            }
            Self::MalformedExecStatus => formatter.write_str("malformed anchor helper exec status"),
            Self::ChildStage(stage) => write!(formatter, "anchor child failed at stage {stage:#x}"),
            Self::ChildExited(detail) => write!(formatter, "anchor child exited: {detail}"),
            Self::Timeout(boundary) => write!(formatter, "anchor launch timed out at {boundary}"),
            Self::Admission(error) => {
                write!(formatter, "anchor endpoint admission failed: {error}")
            }
            Self::SupervisorBindingMismatch => formatter
                .write_str("supervisor deployment or issuer policy does not bind this anchor"),
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for ExternalAnchorCoordinatorErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Credentials(error) => Some(error),
            Self::Executable(error) => Some(error),
            Self::Profile(error) => Some(error),
            Self::Lifecycle(error) => Some(error),
            Self::Spawn(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use std::fs;
    use std::io::IoSlice;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
        CompilerExecutionSupervisorDeploymentV1,
    };
    use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
    use rustix::process::{PidfdFlags, pidfd_open};
    use sha2::{Digest, Sha256};

    use super::*;

    const SUBPROCESS_MARKER: &str = "FE2O3_ANCHOR_COORDINATOR_SUBPROCESS_V1";

    #[test]
    fn measured_preparation_revalidates_and_rejects_root_mutation() {
        if rustix::process::geteuid().is_root() {
            return;
        }
        let bytes = static_pause_elf();
        let helper_source = ExecutableSource::new("helper", &bytes);
        let daemon_source = ExecutableSource::new("daemon", &bytes);
        let measurement = measurement(&bytes);
        let (deployment, provisioning, key_template, _, _) = manifests(measurement, measurement);
        let lifecycle_root = tempfile::tempdir().unwrap();
        fs::set_permissions(lifecycle_root.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let state_root = lifecycle_root.path().join("state");
        fs::create_dir(&state_root).unwrap();
        fs::set_permissions(&state_root, fs::Permissions::from_mode(0o700)).unwrap();
        let lifecycle_path = lifecycle_root.path().join(
            std::path::Path::new(
                fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
            )
            .file_name()
            .unwrap(),
        );
        fs::write(&lifecycle_path, []).unwrap();
        fs::set_permissions(&lifecycle_path, fs::Permissions::from_mode(0o400)).unwrap();
        let root = File::open(&state_root).unwrap();
        let lifecycle =
            CompilerExecutionServiceLifecycleLeaseV1::admit_non_authoritative_same_owner_test(
                File::open(lifecycle_path).unwrap(),
                &root,
            )
            .unwrap();
        let prepared = PreparedExternalAnchorOccurrenceV1::prepare_inner::<false>(
            File::open(&helper_source.path).unwrap(),
            File::open(&daemon_source.path).unwrap(),
            root,
            lifecycle,
            deployment,
            provisioning,
            key_template,
        )
        .unwrap();
        prepared.revalidate_inner::<false>().unwrap();
        fs::set_permissions(&state_root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(
            prepared.revalidate_inner::<false>(),
            Err(ExternalAnchorCoordinatorErrorV1::InvalidStateRoot)
        ));
    }

    #[test]
    fn ready_endpoint_pidfd_transfer_and_reaping_are_one_lifecycle() {
        if rustix::process::geteuid().is_root() {
            return;
        }
        let (process, bootstrap) = spawn_ready_helper();
        let pid = rustix::process::Pid::from_raw(i32::try_from(process.id()).unwrap()).unwrap();
        let pidfd = pidfd_open(pid, PidfdFlags::empty()).unwrap();
        let child = RootOwnedAnchorChildV1::new(pid, pidfd);
        let deadline = Instant::now() + Duration::from_secs(5);
        let (ready, endpoint) = receive_ready(&bootstrap, &child, deadline).unwrap();
        await_exec_eof(&bootstrap, &child, deadline).unwrap();
        assert_eq!(
            ready.disposition(),
            ExternalAnchorProvisioningReadyDispositionV1::Initialized
        );
        let service = CompilerExecutionExternalAnchorServiceIdentityV1::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap();
        let peer = rustix::net::sockopt::socket_peercred(&endpoint).unwrap();
        assert_eq!(peer.pid, pid);
        assert_eq!(peer.uid.as_raw(), service.uid());
        assert_eq!(peer.gid.as_raw(), service.gid());
        let admission_pidfd = child.try_clone_pidfd().unwrap();
        let admission =
            ProtectedExternalAnchorServiceAdmissionV1::admit_non_authoritative_same_uid_test(
                endpoint,
                admission_pidfd,
                service,
            )
            .unwrap();
        let bytes = static_pause_elf();
        let measurement = measurement(&bytes);
        let (deployment, _, _, supervisor, policy) = manifests(measurement, measurement);
        let managed = RootManagedExternalAnchorV1 {
            admission,
            child,
            disposition: ready.disposition(),
            deployment: deployment.clone(),
        };
        managed.validate_continuity_inner::<false>().unwrap();
        assert_eq!(managed.deployment(), &deployment);

        let wrong_supervisor = CompilerExecutionSupervisorDeploymentV1::new(
            supervisor.service_uid(),
            supervisor.service_gid(),
            service,
            supervisor.executable(),
            CompilerExecutionIssuerMeasurementV1::new([0x55; 32], 5).unwrap(),
            &policy,
        )
        .unwrap();
        assert!(matches!(
            managed.try_clone_for_supervisor_inner::<false>(&wrong_supervisor, &policy),
            Err(ExternalAnchorCoordinatorErrorV1::SupervisorBindingMismatch)
        ));

        let wrong_policy = CompilerExecutionIssuerPolicyV1::new(
            policy.generation() + 1,
            policy.executable(),
            policy.runtime(),
            *policy.verifying_key(),
            *policy.external_anchor_verifying_key(),
        )
        .unwrap();
        assert!(matches!(
            managed.try_clone_for_supervisor_inner::<false>(&supervisor, &wrong_policy),
            Err(ExternalAnchorCoordinatorErrorV1::SupervisorBindingMismatch)
        ));

        let transfer = managed
            .try_clone_for_supervisor_inner::<false>(&supervisor, &policy)
            .unwrap();
        assert_eq!(transfer.service(), service);
        assert_eq!(transfer.deployment_identity(), deployment.identity());
        assert_eq!(transfer.supervisor_identity(), supervisor.identity());
        assert_eq!(transfer.policy_identity(), policy.identity());
        let (transferred_endpoint, transferred_pidfd) = transfer.into_ordered_descriptors();
        assert!(
            rustix::io::fcntl_getfd(&transferred_endpoint)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        assert!(
            rustix::io::fcntl_getfd(&transferred_pidfd)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        drop(transferred_endpoint);
        drop(transferred_pidfd);
        std::mem::forget(process);
        managed.shutdown().unwrap();
    }

    #[test]
    fn ready_subprocess_helper() {
        if std::env::var_os(SUBPROCESS_MARKER).is_none() {
            return;
        }
        // SAFETY: the subprocess exclusively owns the installed fixed bootstrap descriptor.
        let bootstrap = unsafe {
            OwnedFd::from_raw_fd(
                fe2o3_external_anchor_provisioner::EXTERNAL_ANCHOR_HELPER_BOOTSTRAP_FD_V1,
            )
        };
        let (supervisor_endpoint, retained_service_endpoint) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        let ready = ExternalAnchorProvisioningReadyV1::new(
            ExternalAnchorProvisioningReadyDispositionV1::Initialized,
        );
        let descriptors = [supervisor_endpoint.as_fd()];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)));
        assert_eq!(
            sendmsg(
                &bootstrap,
                &[IoSlice::new(ready.canonical_bytes())],
                &mut ancillary,
                SendFlags::NOSIGNAL,
            )
            .unwrap(),
            ready.canonical_bytes().len()
        );
        drop(supervisor_endpoint);
        drop(bootstrap);
        let _retain_service_endpoint = retained_service_endpoint;
        loop {
            std::thread::park();
        }
    }

    struct ExecutableSource {
        _directory: tempfile::TempDir,
        path: std::path::PathBuf,
    }

    impl ExecutableSource {
        fn new(name: &str, bytes: &[u8]) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join(name);
            fs::write(&path, bytes).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
            Self {
                _directory: directory,
                path,
            }
        }
    }

    fn spawn_ready_helper() -> (std::process::Child, OwnedFd) {
        let (root_bootstrap, child_bootstrap) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        let source = rustix::io::fcntl_dupfd_cloexec(&child_bootstrap, 300).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("tests::ready_subprocess_helper")
            .arg("--nocapture")
            .env_clear()
            .env(SUBPROCESS_MARKER, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // SAFETY: the callback installs one retained descriptor at the fixed bootstrap slot.
        unsafe {
            command.pre_exec(move || {
                let target =
                    fe2o3_external_anchor_provisioner::EXTERNAL_ANCHOR_HELPER_BOOTSTRAP_FD_V1;
                if libc::dup2(source.as_raw_fd(), target) != target
                    || libc::fcntl(target, libc::F_SETFD, 0) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().unwrap();
        drop(child_bootstrap);
        (child, root_bootstrap)
    }

    fn manifests(
        daemon: CompilerExecutionIssuerMeasurementV1,
        helper: CompilerExecutionIssuerMeasurementV1,
    ) -> (
        CompilerExecutionExternalAnchorDeploymentV1,
        CompilerExecutionExternalAnchorProvisioningV1,
        CompilerExecutionExternalAnchorSigningKeyCapabilityV1,
        CompilerExecutionSupervisorDeploymentV1,
        CompilerExecutionIssuerPolicyV1,
    ) {
        let mut seed = [0x31; 32];
        let policy = CompilerExecutionIssuerPolicyV1::new(
            1,
            CompilerExecutionIssuerMeasurementV1::new([1; 32], 1).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([2; 32], 2).unwrap(),
            SigningKey::from_bytes(&[3; 32]).verifying_key().to_bytes(),
            SigningKey::from_bytes(&seed).verifying_key().to_bytes(),
        )
        .unwrap();
        let service = CompilerExecutionExternalAnchorServiceIdentityV1::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap();
        let supervisor = CompilerExecutionSupervisorDeploymentV1::new(
            service.uid().checked_add(1).unwrap(),
            service.gid(),
            service,
            CompilerExecutionIssuerMeasurementV1::new([3; 32], 3).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([4; 32], 4).unwrap(),
            &policy,
        )
        .unwrap();
        let deployment =
            CompilerExecutionExternalAnchorDeploymentV1::new(&supervisor, &policy, daemon).unwrap();
        let provisioning =
            CompilerExecutionExternalAnchorProvisioningV1::new(&deployment, helper).unwrap();
        let key = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
            &mut seed,
            &deployment,
        )
        .unwrap();
        (deployment, provisioning, key, supervisor, policy)
    }

    fn measurement(bytes: &[u8]) -> CompilerExecutionIssuerMeasurementV1 {
        CompilerExecutionIssuerMeasurementV1::new(Sha256::digest(bytes).into(), bytes.len() as u64)
            .unwrap()
    }

    fn static_pause_elf() -> Vec<u8> {
        const HEADER: usize = 64;
        const PROGRAM: usize = 56;
        const PROGRAMS: usize = 4;
        const CODE_OFFSET: usize = 0x1000;
        const CODE: &[u8] = b"\xb8\x22\x00\x00\x00\x0f\x05\xeb\xf7";
        let mut bytes = vec![0_u8; CODE_OFFSET + CODE.len()];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
        bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
        bytes[24..32].copy_from_slice(&0x401000_u64.to_le_bytes());
        bytes[32..40].copy_from_slice(&(HEADER as u64).to_le_bytes());
        bytes[52..54].copy_from_slice(&(HEADER as u16).to_le_bytes());
        bytes[54..56].copy_from_slice(&(PROGRAM as u16).to_le_bytes());
        bytes[56..58].copy_from_slice(&(PROGRAMS as u16).to_le_bytes());
        let table_size = (PROGRAM * PROGRAMS) as u64;
        write_program(
            &mut bytes,
            0,
            6,
            4,
            HEADER as u64,
            0x400040,
            table_size,
            table_size,
            8,
        );
        write_program(
            &mut bytes,
            1,
            1,
            4,
            0,
            0x400000,
            HEADER as u64 + table_size,
            HEADER as u64 + table_size,
            0x1000,
        );
        write_program(
            &mut bytes,
            2,
            1,
            5,
            CODE_OFFSET as u64,
            0x401000,
            CODE.len() as u64,
            CODE.len() as u64,
            0x1000,
        );
        write_program(&mut bytes, 3, 0x6474_e551, 6, 0, 0, 0, 0, 16);
        bytes[CODE_OFFSET..].copy_from_slice(CODE);
        bytes
    }

    #[allow(clippy::too_many_arguments)]
    fn write_program(
        bytes: &mut [u8],
        index: usize,
        kind: u32,
        flags: u32,
        offset: u64,
        virtual_address: u64,
        file_size: u64,
        memory_size: u64,
        alignment: u64,
    ) {
        const HEADER: usize = 64;
        const PROGRAM: usize = 56;
        let start = HEADER + index * PROGRAM;
        bytes[start..start + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&flags.to_le_bytes());
        bytes[start + 8..start + 16].copy_from_slice(&offset.to_le_bytes());
        bytes[start + 16..start + 24].copy_from_slice(&virtual_address.to_le_bytes());
        bytes[start + 32..start + 40].copy_from_slice(&file_size.to_le_bytes());
        bytes[start + 40..start + 48].copy_from_slice(&memory_size.to_le_bytes());
        bytes[start + 48..start + 56].copy_from_slice(&alignment.to_le_bytes());
    }
}
