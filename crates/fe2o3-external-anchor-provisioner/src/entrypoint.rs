//! Fixed-descriptor measured external-anchor provisioning helper.

use core::ffi::{c_char, c_void};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, IoSlice};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};

use fe2o3_compiler_closure_capability::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_FD_V1,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1,
    CompilerExecutionExternalAnchorDeploymentCapabilityV1,
    CompilerExecutionExternalAnchorProvisioningCapabilityV1,
    CompilerExecutionExternalAnchorSigningKeyCapabilityV1,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionExternalAnchorProvisioningV1,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
};
use fe2o3_external_anchor_service::{
    DurableExternalAnchorOpenDispositionV1, DurableExternalAnchorV1,
    EXTERNAL_ANCHOR_SERVICE_PEER_FD_V1, EXTERNAL_ANCHOR_SERVICE_ROOT_FD_V1,
    ExternalAnchorServiceErrorV1,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileErrorV1, ProtectedServiceCredentialProfileV1,
    ProtectedServiceNamespaceSetV1, ProtectedServiceProcessProfileV1,
    ProtectedServiceProfileErrorV1, require_owned_sigchld_v1,
};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV1, ProtectedStaticExecutableMeasurementV1,
    ProtectedStaticExecutableOwnerV1, ProtectedStaticExecutableV1,
};
use rustix::fs::OFlags;
use rustix::net::{
    AddressFamily, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketAddrAny,
    SocketAddrUnix, SocketFlags, SocketType, sendmsg, socketpair,
};

use crate::{ExternalAnchorProvisioningReadyDispositionV1, ExternalAnchorProvisioningReadyV1};

/// Private root-to-helper bootstrap endpoint.
pub const EXTERNAL_ANCHOR_HELPER_BOOTSTRAP_FD_V1: RawFd = 3;
/// Existing service-owned mode-0700 durable root.
pub const EXTERNAL_ANCHOR_HELPER_ROOT_FD_V1: RawFd = 4;
/// Exact service-owned sealed daemon executable.
pub const EXTERNAL_ANCHOR_HELPER_DAEMON_EXECUTABLE_FD_V1: RawFd = 5;

const STAGED_DESCRIPTOR_FLOOR_V1: RawFd = 300;
const CLOSE_RANGE_CLOEXEC_V1: u32 = 1 << 2;
const EXEC_FAILURE_STAGE_BASE_V1: u8 = 0xe0;

const _: () = assert!(EXTERNAL_ANCHOR_SERVICE_PEER_FD_V1 == 3);
const _: () = assert!(EXTERNAL_ANCHOR_SERVICE_ROOT_FD_V1 == 4);
const _: () = assert!(COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1 == 221);
const _: () = assert!(COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1 == 222);
const _: () = assert!(COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_FD_V1 == 223);
const _: () = assert!(STAGED_DESCRIPTOR_FLOOR_V1 > 223);

struct AdmittedProvisioningHelperProfileV1 {
    process: Option<ProtectedServiceProcessProfileV1>,
    namespaces: Option<ProtectedServiceNamespaceSetV1>,
}

impl AdmittedProvisioningHelperProfileV1 {
    fn admit(
        credentials: ProtectedServiceCredentialProfileV1,
    ) -> Result<Self, ProtectedServiceProfileErrorV1> {
        let process = ProtectedServiceProcessProfileV1::capture(credentials)?;
        require_owned_sigchld_v1()?;
        let namespaces = ProtectedServiceNamespaceSetV1::capture_self()?;
        namespaces.revalidate_self()?;
        Ok(Self {
            process: Some(process),
            namespaces: Some(namespaces),
        })
    }

    #[cfg(test)]
    const fn for_test() -> Self {
        Self {
            process: None,
            namespaces: None,
        }
    }

    fn revalidate(&self) -> Result<(), ProtectedServiceProfileErrorV1> {
        if let Some(process) = &self.process {
            process.revalidate_current()?;
        }
        if let Some(namespaces) = &self.namespaces {
            namespaces.revalidate_self()?;
        }
        if self.process.is_some() != self.namespaces.is_some() {
            return Err(ProtectedServiceProfileErrorV1::InvalidState(
                "partial external-anchor provisioning-helper profile",
            ));
        }
        Ok(())
    }
}

struct RetainedProvisioningHelperExecutableV1 {
    executable: Option<ProtectedStaticExecutableV1>,
}

impl RetainedProvisioningHelperExecutableV1 {
    fn admit(
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
        provisioning: &CompilerExecutionExternalAnchorProvisioningV1,
    ) -> Result<Self, ProtectedStaticExecutableErrorV1> {
        let helper = provisioning.helper();
        let service = deployment.service();
        let measurement = ProtectedStaticExecutableMeasurementV1::new(
            helper.sha256(),
            helper.byte_len(),
            MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
        )?;
        let owner = ProtectedStaticExecutableOwnerV1::new(service.uid(), service.gid())?;
        ProtectedStaticExecutableV1::admit_running(
            measurement,
            owner,
            "external-anchor provisioning helper",
        )
        .map(|executable| Self {
            executable: Some(executable),
        })
    }

    #[cfg(test)]
    const fn for_test() -> Self {
        Self { executable: None }
    }

    fn revalidate(&self) -> Result<(), ProtectedStaticExecutableErrorV1> {
        if let Some(executable) = &self.executable {
            executable.revalidate()?;
        }
        Ok(())
    }
}

struct StagedDaemonExecV1 {
    daemon: OwnedFd,
    peer: OwnedFd,
    root: OwnedFd,
    deployment: OwnedFd,
    key: OwnedFd,
    bootstrap: OwnedFd,
}

/// Runs the measured helper and replaces it with the external-anchor daemon on success.
pub fn run_inherited_external_anchor_provisioning_helper_v1()
-> Result<(), ExternalAnchorProvisioningHelperErrorV1> {
    require_descriptor_only_invocation_v1()?;
    run_inherited_with_admission_v1::<true>(
        AdmittedProvisioningHelperProfileV1::admit,
        RetainedProvisioningHelperExecutableV1::admit,
        CompilerExecutionExternalAnchorSigningKeyCapabilityV1::reissue_root_template_for_current_service,
    )
}

fn run_inherited_with_admission_v1<const REQUIRE_ROOT_BOOTSTRAP: bool>(
    admit_profile: impl FnOnce(
        ProtectedServiceCredentialProfileV1,
    ) -> Result<
        AdmittedProvisioningHelperProfileV1,
        ProtectedServiceProfileErrorV1,
    >,
    admit_helper: impl FnOnce(
        &CompilerExecutionExternalAnchorDeploymentV1,
        &CompilerExecutionExternalAnchorProvisioningV1,
    ) -> Result<
        RetainedProvisioningHelperExecutableV1,
        ProtectedStaticExecutableErrorV1,
    >,
    reissue_key: impl FnOnce(
        File,
        &CompilerExecutionExternalAnchorDeploymentV1,
    )
        -> Result<CompilerExecutionExternalAnchorSigningKeyCapabilityV1, String>,
) -> Result<(), ExternalAnchorProvisioningHelperErrorV1> {
    let deployment = CompilerExecutionExternalAnchorDeploymentCapabilityV1::from_inherited()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::DeploymentCapability)?;
    close_fixed(COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1)?;
    let manifest = deployment.deployment().clone();

    let provisioning = CompilerExecutionExternalAnchorProvisioningCapabilityV1::from_inherited()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::ProvisioningCapability)?;
    close_fixed(COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_FD_V1)?;
    if !provisioning.provisioning().matches_deployment(&manifest) {
        return Err(ExternalAnchorProvisioningHelperErrorV1::ProvisioningMismatch);
    }

    let service = manifest.service();
    let credentials = ProtectedServiceCredentialProfileV1::new(service.uid(), service.gid())
        .map_err(ExternalAnchorProvisioningHelperErrorV1::Credentials)?;
    let profile =
        admit_profile(credentials).map_err(ExternalAnchorProvisioningHelperErrorV1::Profile)?;
    let helper = admit_helper(&manifest, provisioning.provisioning())
        .map_err(ExternalAnchorProvisioningHelperErrorV1::HelperExecutable)?;
    profile
        .revalidate()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::Profile)?;
    helper
        .revalidate()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::HelperExecutable)?;

    let bootstrap = take_fixed(EXTERNAL_ANCHOR_HELPER_BOOTSTRAP_FD_V1, "bootstrap endpoint")?;
    validate_bootstrap::<REQUIRE_ROOT_BOOTSTRAP>(&bootstrap)?;
    let root = File::from(take_fixed(
        EXTERNAL_ANCHOR_HELPER_ROOT_FD_V1,
        "durable root",
    )?);
    let daemon_source = File::from(take_fixed(
        EXTERNAL_ANCHOR_HELPER_DAEMON_EXECUTABLE_FD_V1,
        "daemon executable",
    )?);
    let daemon_measurement = manifest.executable();
    let daemon = ProtectedStaticExecutableV1::admit_sealed(
        daemon_source,
        ProtectedStaticExecutableMeasurementV1::new(
            daemon_measurement.sha256(),
            daemon_measurement.byte_len(),
            MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
        )
        .map_err(ExternalAnchorProvisioningHelperErrorV1::DaemonExecutable)?,
        ProtectedStaticExecutableOwnerV1::new(service.uid(), service.gid())
            .map_err(ExternalAnchorProvisioningHelperErrorV1::DaemonExecutable)?,
        "external-anchor daemon",
    )
    .map_err(ExternalAnchorProvisioningHelperErrorV1::DaemonExecutable)?;

    let key_template = File::from(take_fixed(
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1,
        "root signing-key template",
    )?);
    let key = reissue_key(key_template, &manifest)
        .map_err(ExternalAnchorProvisioningHelperErrorV1::SigningKeyCapability)?;
    key.revalidate(&manifest)
        .map_err(ExternalAnchorProvisioningHelperErrorV1::SigningKeyCapability)?;

    let state_key = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_file(
        key.try_clone_for_transfer()
            .map_err(ExternalAnchorProvisioningHelperErrorV1::SigningKeyCapability)?,
        &manifest,
    )
    .and_then(|key| key.into_signing_key(&manifest))
    .map_err(ExternalAnchorProvisioningHelperErrorV1::SigningKeyCapability)?;
    let state_root = rustix::io::fcntl_dupfd_cloexec(&root, STAGED_DESCRIPTOR_FLOOR_V1)
        .map_err(|source| io_error("clone durable root for atomic bootstrap", source.into()))?;
    let (anchor, disposition) = DurableExternalAnchorV1::open_or_initialize(state_root, state_key)
        .map_err(ExternalAnchorProvisioningHelperErrorV1::DurableState)?;
    if anchor.verifying_key_bytes() != *manifest.verifying_key() {
        return Err(ExternalAnchorProvisioningHelperErrorV1::SigningKeyMismatch);
    }

    let (supervisor_peer, daemon_peer) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .map_err(|source| {
        io_error(
            "create service-owned external-anchor socketpair",
            source.into(),
        )
    })?;

    deployment
        .revalidate()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::DeploymentCapability)?;
    provisioning
        .revalidate()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::ProvisioningCapability)?;
    profile
        .revalidate()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::Profile)?;
    helper
        .revalidate()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::HelperExecutable)?;
    daemon
        .revalidate()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::DaemonExecutable)?;
    key.revalidate(&manifest)
        .map_err(ExternalAnchorProvisioningHelperErrorV1::SigningKeyCapability)?;

    let mut next = STAGED_DESCRIPTOR_FLOOR_V1;
    let staged = StagedDaemonExecV1 {
        daemon: stage_above(
            &daemon
                .try_clone_for_exec()
                .map_err(ExternalAnchorProvisioningHelperErrorV1::DaemonExecutable)?,
            &mut next,
            "daemon executable",
        )?,
        peer: stage_above(&daemon_peer, &mut next, "daemon peer")?,
        root: stage_above(&root, &mut next, "durable root")?,
        deployment: stage_above(
            &deployment
                .try_clone_for_transfer()
                .map_err(ExternalAnchorProvisioningHelperErrorV1::DeploymentCapability)?,
            &mut next,
            "deployment capability",
        )?,
        key: stage_above(
            &key.try_clone_for_transfer()
                .map_err(ExternalAnchorProvisioningHelperErrorV1::SigningKeyCapability)?,
            &mut next,
            "signing-key capability",
        )?,
        bootstrap: stage_above(&bootstrap, &mut next, "bootstrap endpoint")?,
    };

    let ready = ExternalAnchorProvisioningReadyV1::new(match disposition {
        DurableExternalAnchorOpenDispositionV1::Existing => {
            ExternalAnchorProvisioningReadyDispositionV1::Existing
        }
        DurableExternalAnchorOpenDispositionV1::Initialized => {
            ExternalAnchorProvisioningReadyDispositionV1::Initialized
        }
    });
    send_ready(&staged.bootstrap, &supervisor_peer, &ready)?;
    drop(supervisor_peer);
    drop(daemon_peer);
    drop(bootstrap);

    deployment
        .revalidate()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::DeploymentCapability)?;
    provisioning
        .revalidate()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::ProvisioningCapability)?;
    profile
        .revalidate()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::Profile)?;
    helper
        .revalidate()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::HelperExecutable)?;
    daemon
        .revalidate()
        .map_err(ExternalAnchorProvisioningHelperErrorV1::DaemonExecutable)?;
    key.revalidate(&manifest)
        .map_err(ExternalAnchorProvisioningHelperErrorV1::SigningKeyCapability)?;
    let _retain_atomic_state_lock_across_exec = anchor;

    // SAFETY: all inputs are privately owned staged descriptors. Success replaces this process;
    // every failure emits one private stage byte and terminates without Rust cleanup.
    unsafe { exec_daemon(&staged) }
}

fn require_descriptor_only_invocation_v1() -> Result<(), ExternalAnchorProvisioningHelperErrorV1> {
    if std::env::args_os().count() != 1 || std::env::vars_os().next().is_some() {
        return Err(ExternalAnchorProvisioningHelperErrorV1::RuntimeConfiguration);
    }
    Ok(())
}

fn take_fixed(
    descriptor: RawFd,
    role: &'static str,
) -> Result<OwnedFd, ExternalAnchorProvisioningHelperErrorV1> {
    // SAFETY: F_GETFD observes only the scalar fixed descriptor and reports invalid values via errno.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0 {
        return Err(descriptor_error("inspect inherited helper descriptor"));
    }
    if flags & libc::FD_CLOEXEC != 0 {
        return Err(ExternalAnchorProvisioningHelperErrorV1::InvalidDescriptor {
            role,
            reason: "fixed descriptor is unexpectedly close-on-exec",
        });
    }
    // SAFETY: F_DUPFD_CLOEXEC returns one new owned descriptor or reports errno.
    let retained = unsafe {
        libc::fcntl(
            descriptor,
            libc::F_DUPFD_CLOEXEC,
            STAGED_DESCRIPTOR_FLOOR_V1,
        )
    };
    if retained < 0 {
        return Err(descriptor_error("retain inherited helper descriptor"));
    }
    // SAFETY: the successful fcntl returned a new descriptor owned by this process.
    let retained = unsafe { OwnedFd::from_raw_fd(retained) };
    close_fixed(descriptor)?;
    Ok(retained)
}

fn close_fixed(descriptor: RawFd) -> Result<(), ExternalAnchorProvisioningHelperErrorV1> {
    // SAFETY: callers close each inherited fixed descriptor once after private retention.
    if unsafe { libc::close(descriptor) } != 0 {
        return Err(descriptor_error("close inherited helper descriptor"));
    }
    Ok(())
}

fn stage_above(
    source: &impl AsFd,
    next: &mut RawFd,
    role: &'static str,
) -> Result<OwnedFd, ExternalAnchorProvisioningHelperErrorV1> {
    let staged = rustix::io::fcntl_dupfd_cloexec(source, *next)
        .map_err(|source| io_error("stage helper descriptor", source.into()))?;
    *next = staged.as_raw_fd().checked_add(1).ok_or(
        ExternalAnchorProvisioningHelperErrorV1::InvalidDescriptor {
            role,
            reason: "staged descriptor range overflowed",
        },
    )?;
    Ok(staged)
}

fn validate_bootstrap<const REQUIRE_ROOT: bool>(
    bootstrap: &OwnedFd,
) -> Result<(), ExternalAnchorProvisioningHelperErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(bootstrap)
        .map_err(|source| io_error("inspect helper bootstrap descriptor", source.into()))?;
    let status = rustix::fs::fcntl_getfl(bootstrap)
        .map_err(|source| io_error("inspect helper bootstrap status", source.into()))?;
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDWR
        || !status.contains(OFlags::NONBLOCK)
        || status.intersects(forbidden)
        || rustix::net::sockopt::socket_domain(bootstrap)
            .map_err(|source| io_error("inspect helper bootstrap domain", source.into()))?
            != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(bootstrap)
            .map_err(|source| io_error("inspect helper bootstrap type", source.into()))?
            != SocketType::SEQPACKET
        || rustix::net::sockopt::socket_acceptconn(bootstrap)
            .map_err(|source| io_error("inspect helper bootstrap listener state", source.into()))?
    {
        return Err(ExternalAnchorProvisioningHelperErrorV1::InvalidBootstrap);
    }
    let unnamed = SocketAddrAny::from(SocketAddrUnix::new_unnamed());
    let local = rustix::net::getsockname(bootstrap)
        .map_err(|source| io_error("inspect helper bootstrap local address", source.into()))?;
    let remote = rustix::net::getpeername(bootstrap)
        .map_err(|source| io_error("inspect helper bootstrap remote address", source.into()))?;
    if local != unnamed || remote.as_ref() != Some(&unnamed) {
        return Err(ExternalAnchorProvisioningHelperErrorV1::InvalidBootstrap);
    }
    match rustix::net::sockopt::socket_error(bootstrap)
        .map_err(|source| io_error("inspect helper bootstrap socket error", source.into()))?
    {
        Ok(()) => {}
        Err(source) => {
            return Err(io_error(
                "helper bootstrap has a pending error",
                source.into(),
            ));
        }
    }
    let peer = rustix::net::sockopt::socket_peercred(bootstrap)
        .map_err(|source| io_error("inspect helper bootstrap peer credentials", source.into()))?;
    let expected_parent = rustix::process::getppid();
    if Some(peer.pid) != expected_parent
        || (REQUIRE_ROOT && (!peer.uid.is_root() || !peer.gid.is_root()))
    {
        return Err(ExternalAnchorProvisioningHelperErrorV1::InvalidBootstrap);
    }
    Ok(())
}

fn send_ready(
    bootstrap: &OwnedFd,
    supervisor_peer: &OwnedFd,
    ready: &ExternalAnchorProvisioningReadyV1,
) -> Result<(), ExternalAnchorProvisioningHelperErrorV1> {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    let descriptors = [supervisor_peer.as_fd()];
    if !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)) {
        return Err(ExternalAnchorProvisioningHelperErrorV1::ReadyTransfer);
    }
    let count = sendmsg(
        bootstrap,
        &[IoSlice::new(ready.canonical_bytes())],
        &mut ancillary,
        SendFlags::NOSIGNAL,
    )
    .map_err(|source| io_error("send helper-ready endpoint", source.into()))?;
    if count != ready.canonical_bytes().len() {
        return Err(ExternalAnchorProvisioningHelperErrorV1::ReadyTransfer);
    }
    Ok(())
}

unsafe fn exec_daemon(staged: &StagedDaemonExecV1) -> ! {
    // SAFETY: every operation below is a direct scalar Linux syscall over retained descriptors.
    unsafe {
        if libc::syscall(
            libc::SYS_close_range,
            3_u32,
            u32::MAX,
            CLOSE_RANGE_CLOEXEC_V1,
        ) != 0
        {
            exec_fail(staged.bootstrap.as_raw_fd(), 1);
        }
        for (source, target, stage) in [
            (
                staged.peer.as_raw_fd(),
                EXTERNAL_ANCHOR_SERVICE_PEER_FD_V1,
                2,
            ),
            (
                staged.root.as_raw_fd(),
                EXTERNAL_ANCHOR_SERVICE_ROOT_FD_V1,
                3,
            ),
            (
                staged.deployment.as_raw_fd(),
                COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1,
                4,
            ),
            (
                staged.key.as_raw_fd(),
                COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1,
                5,
            ),
        ] {
            if libc::dup3(source, target, 0) != target {
                exec_fail(staged.bootstrap.as_raw_fd(), stage);
            }
        }
        let name = c"fe2o3-external-anchor-service";
        let arguments = [name.as_ptr().cast_mut(), std::ptr::null_mut()];
        let environment = [std::ptr::null_mut::<c_char>()];
        libc::syscall(
            libc::SYS_execveat,
            staged.daemon.as_raw_fd(),
            c"".as_ptr(),
            arguments.as_ptr(),
            environment.as_ptr(),
            libc::AT_EMPTY_PATH,
        );
        exec_fail(staged.bootstrap.as_raw_fd(), 6)
    }
}

unsafe fn exec_fail(bootstrap: RawFd, stage: u8) -> ! {
    let message = EXEC_FAILURE_STAGE_BASE_V1.saturating_add(stage);
    // SAFETY: bootstrap is a retained connected seqpacket and message points to one live byte.
    let _ = unsafe {
        libc::send(
            bootstrap,
            (&raw const message).cast::<c_void>(),
            1,
            libc::MSG_NOSIGNAL,
        )
    };
    // SAFETY: process-local fail-closed termination after an unrecoverable exec stage.
    unsafe { libc::_exit(127) }
}

fn descriptor_error(operation: &'static str) -> ExternalAnchorProvisioningHelperErrorV1 {
    io_error(operation, io::Error::last_os_error())
}

fn io_error(operation: &'static str, source: io::Error) -> ExternalAnchorProvisioningHelperErrorV1 {
    ExternalAnchorProvisioningHelperErrorV1::Io { operation, source }
}

/// Stable failure entering or executing the measured provisioning helper.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExternalAnchorProvisioningHelperErrorV1 {
    /// Arguments or environment violated the descriptor-only contract.
    RuntimeConfiguration,
    /// The sealed deployment capability is invalid or changed.
    DeploymentCapability(String),
    /// The sealed provisioning capability is invalid or changed.
    ProvisioningCapability(String),
    /// Provisioning names another deployment.
    ProvisioningMismatch,
    /// The deployment names invalid protected credentials.
    Credentials(ProtectedServiceCredentialProfileErrorV1),
    /// The helper process profile is not exact or changed.
    Profile(ProtectedServiceProfileErrorV1),
    /// The running measured helper image is invalid or changed.
    HelperExecutable(ProtectedStaticExecutableErrorV1),
    /// The measured daemon image is invalid or changed.
    DaemonExecutable(ProtectedStaticExecutableErrorV1),
    /// The root template or reissued service-owned key capability is invalid.
    SigningKeyCapability(String),
    /// The inherited bootstrap endpoint is not the exact private root channel.
    InvalidBootstrap,
    /// One inherited or staged descriptor violates its fixed role.
    InvalidDescriptor {
        /// Descriptor role.
        role: &'static str,
        /// Stable rejection reason.
        reason: &'static str,
    },
    /// Durable state could not be opened or initialized atomically.
    DurableState(ExternalAnchorServiceErrorV1),
    /// Durable state and deployment disagree on the signing key.
    SigningKeyMismatch,
    /// The exact ready record and endpoint could not be transferred once.
    ReadyTransfer,
    /// A bounded kernel operation failed.
    Io {
        /// Operation that failed.
        operation: &'static str,
        /// Kernel error.
        source: io::Error,
    },
}

impl fmt::Display for ExternalAnchorProvisioningHelperErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeConfiguration => formatter.write_str(
                "external-anchor provisioning helper accepts no arguments or environment",
            ),
            Self::DeploymentCapability(error) => {
                write!(formatter, "invalid deployment capability: {error}")
            }
            Self::ProvisioningCapability(error) => {
                write!(formatter, "invalid provisioning capability: {error}")
            }
            Self::ProvisioningMismatch => {
                formatter.write_str("provisioning capability names another deployment")
            }
            Self::Credentials(error) => write!(formatter, "invalid helper credentials: {error}"),
            Self::Profile(error) => write!(formatter, "invalid helper process profile: {error}"),
            Self::HelperExecutable(error) => {
                write!(formatter, "invalid helper executable: {error}")
            }
            Self::DaemonExecutable(error) => {
                write!(formatter, "invalid daemon executable: {error}")
            }
            Self::SigningKeyCapability(error) => {
                write!(formatter, "invalid anchor key custody: {error}")
            }
            Self::InvalidBootstrap => {
                formatter.write_str("invalid root-to-helper bootstrap endpoint")
            }
            Self::InvalidDescriptor { role, reason } => {
                write!(formatter, "invalid helper {role}: {reason}")
            }
            Self::DurableState(error) => {
                write!(formatter, "cannot bootstrap durable anchor state: {error}")
            }
            Self::SigningKeyMismatch => {
                formatter.write_str("bootstrapped anchor key differs from deployment")
            }
            Self::ReadyTransfer => {
                formatter.write_str("cannot transfer exact helper-ready endpoint")
            }
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for ExternalAnchorProvisioningHelperErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Credentials(error) => Some(error),
            Self::Profile(error) => Some(error),
            Self::HelperExecutable(error) | Self::DaemonExecutable(error) => Some(error),
            Self::DurableState(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{IoSliceMut, Read};
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionExternalAnchorProvisioningV1,
        CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentV1,
    };
    use rustix::net::{
        AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SocketAddrAny, SocketAddrUnix, SocketFlags, SocketType, recv, recvmsg, socketpair,
    };
    use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};
    use sha2::{Digest, Sha256};

    use super::*;

    const SUBPROCESS_MARKER: &str = "FE2O3_ANCHOR_PROVISIONER_SUBPROCESS_V1";
    const TEST_TIMEOUT: Duration = Duration::from_secs(5);

    #[test]
    fn helper_executes_daemon_and_reopens_initialized_state() {
        if rustix::process::geteuid().is_root() {
            return;
        }
        let fixture = Fixture::new();
        for expected in [
            ExternalAnchorProvisioningReadyDispositionV1::Initialized,
            ExternalAnchorProvisioningReadyDispositionV1::Existing,
        ] {
            let (mut child, bootstrap, pidfd) = fixture.spawn_helper();
            let ready = receive_ready(&bootstrap, &mut child);
            assert_eq!(ready.0.disposition(), expected);
            wait_for_exec_eof(&bootstrap, &mut child);

            let peer = rustix::net::sockopt::socket_peercred(&ready.1).unwrap();
            assert_eq!(
                Some(peer.pid),
                Pid::from_raw(i32::try_from(child.id()).unwrap())
            );
            assert_eq!(peer.uid, rustix::process::geteuid());
            assert_eq!(peer.gid, rustix::process::getegid());
            assert_eq!(
                rustix::net::sockopt::socket_type(&ready.1).unwrap(),
                SocketType::SEQPACKET
            );
            let unnamed = SocketAddrAny::from(SocketAddrUnix::new_unnamed());
            assert_eq!(rustix::net::getsockname(&ready.1).unwrap(), unnamed);
            assert_eq!(rustix::net::getpeername(&ready.1).unwrap(), Some(unnamed));
            assert!(
                rustix::fs::fcntl_getfl(&ready.1)
                    .unwrap()
                    .contains(OFlags::NONBLOCK)
            );
            assert!(
                rustix::io::fcntl_getfd(&ready.1)
                    .unwrap()
                    .contains(rustix::io::FdFlags::CLOEXEC)
            );
            pidfd_send_signal(&pidfd, Signal::CONT).unwrap();
            assert!(fixture.root.path().join("anchor-state-v1").is_file());

            pidfd_send_signal(&pidfd, Signal::KILL).unwrap();
            let status = child.wait().unwrap();
            assert!(status.signal().is_some());
        }
    }

    #[test]
    fn provisioning_subprocess_helper() {
        if std::env::var_os(SUBPROCESS_MARKER).is_none() {
            return;
        }
        let result = run_inherited_with_admission_v1::<false>(
            |_| Ok(AdmittedProvisioningHelperProfileV1::for_test()),
            |_, _| Ok(RetainedProvisioningHelperExecutableV1::for_test()),
            |image, deployment| {
                let admitted = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_file(
                    image, deployment,
                )?;
                let mut seed = admitted.into_signing_key(deployment)?.to_bytes();
                CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
                    &mut seed, deployment,
                )
            },
        );
        panic!("provisioning helper failed before daemon exec: {result:?}");
    }

    struct Fixture {
        root: tempfile::TempDir,
        daemon: ProtectedStaticExecutableV1,
        deployment: CompilerExecutionExternalAnchorDeploymentCapabilityV1,
        provisioning: CompilerExecutionExternalAnchorProvisioningCapabilityV1,
        key_template: CompilerExecutionExternalAnchorSigningKeyCapabilityV1,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
            let daemon_bytes = static_pause_elf();
            let digest = <[u8; 32]>::from(Sha256::digest(&daemon_bytes));
            let daemon_measurement =
                CompilerExecutionIssuerMeasurementV1::new(digest, daemon_bytes.len() as u64)
                    .unwrap();
            let source = tempfile::tempdir().unwrap();
            let source_path = source.path().join("daemon-probe");
            fs::write(&source_path, daemon_bytes).unwrap();
            fs::set_permissions(&source_path, fs::Permissions::from_mode(0o555)).unwrap();
            let daemon = ProtectedStaticExecutableV1::seal_source_for_owner(
                File::open(&source_path).unwrap(),
                ProtectedStaticExecutableMeasurementV1::new(
                    digest,
                    daemon_measurement.byte_len(),
                    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
                )
                .unwrap(),
                ProtectedStaticExecutableOwnerV1::current(),
                "test external-anchor daemon",
            )
            .unwrap();

            let mut seed = [0x17; 32];
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
            let deployment = CompilerExecutionExternalAnchorDeploymentV1::new(
                &supervisor,
                &policy,
                daemon_measurement,
            )
            .unwrap();
            let provisioning = CompilerExecutionExternalAnchorProvisioningV1::new(
                &deployment,
                CompilerExecutionIssuerMeasurementV1::new([5; 32], 5).unwrap(),
            )
            .unwrap();
            let key_template =
                CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
                    &mut seed,
                    &deployment,
                )
                .unwrap();
            Self {
                root,
                daemon,
                deployment: CompilerExecutionExternalAnchorDeploymentCapabilityV1::create(
                    deployment,
                )
                .unwrap(),
                provisioning: CompilerExecutionExternalAnchorProvisioningCapabilityV1::create(
                    provisioning,
                )
                .unwrap(),
                key_template,
            }
        }

        fn spawn_helper(&self) -> (Child, OwnedFd, OwnedFd) {
            let (root_bootstrap, helper_bootstrap) = socketpair(
                AddressFamily::UNIX,
                SocketType::SEQPACKET,
                SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
                None,
            )
            .unwrap();
            let sources = [
                duplicate_high(&helper_bootstrap),
                duplicate_high(&File::open(self.root.path()).unwrap()),
                duplicate_high(&self.daemon.try_clone_for_exec().unwrap()),
                duplicate_high(&self.deployment.try_clone_for_transfer().unwrap()),
                duplicate_high(&self.key_template.try_clone_for_transfer().unwrap()),
                duplicate_high(&self.provisioning.try_clone_for_transfer().unwrap()),
            ];
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .arg("--exact")
                .arg("entrypoint::tests::provisioning_subprocess_helper")
                .arg("--nocapture")
                .env_clear()
                .env(SUBPROCESS_MARKER, "1")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            // SAFETY: the callback only duplicates retained descriptors into fixed slots before
            // exec; dup2 and F_SETFD are async-signal-safe and report failure through `spawn`.
            unsafe {
                command.pre_exec(move || {
                    for (source, target) in sources.iter().zip([
                        EXTERNAL_ANCHOR_HELPER_BOOTSTRAP_FD_V1,
                        EXTERNAL_ANCHOR_HELPER_ROOT_FD_V1,
                        EXTERNAL_ANCHOR_HELPER_DAEMON_EXECUTABLE_FD_V1,
                        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1,
                        COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1,
                        COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_FD_V1,
                    ]) {
                        if libc::dup2(source.as_raw_fd(), target) != target
                            || libc::fcntl(target, libc::F_SETFD, 0) != 0
                        {
                            return Err(io::Error::last_os_error());
                        }
                    }
                    Ok(())
                });
            }
            let child = command.spawn().unwrap();
            drop(helper_bootstrap);
            let pid = Pid::from_raw(i32::try_from(child.id()).unwrap()).unwrap();
            let pidfd = pidfd_open(pid, PidfdFlags::empty()).unwrap();
            (child, root_bootstrap, pidfd)
        }
    }

    fn receive_ready(
        bootstrap: &OwnedFd,
        child: &mut Child,
    ) -> (ExternalAnchorProvisioningReadyV1, OwnedFd) {
        let deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            let mut payload = [0_u8; crate::EXTERNAL_ANCHOR_PROVISIONING_READY_BYTES_V1];
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
                    assert_eq!(received.bytes, payload.len());
                    assert!(
                        !received
                            .flags
                            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
                    );
                    let mut descriptors = Vec::with_capacity(1);
                    for message in ancillary.drain() {
                        match message {
                            RecvAncillaryMessage::ScmRights(received) => {
                                descriptors.extend(received);
                            }
                            _ => panic!("helper-ready transfer carried unexpected ancillary data"),
                        }
                    }
                    assert_eq!(descriptors.len(), 1);
                    return (
                        ExternalAnchorProvisioningReadyV1::decode(&payload).unwrap(),
                        descriptors.into_iter().next().unwrap(),
                    );
                }
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                    require_live_before_deadline(child, deadline, "helper-ready transfer");
                }
                Err(error) => panic!("receive helper-ready transfer: {error}"),
            }
        }
    }

    fn wait_for_exec_eof(bootstrap: &OwnedFd, child: &mut Child) {
        let deadline = Instant::now() + TEST_TIMEOUT;
        let mut payload = [0_u8; 1];
        loop {
            match recv(bootstrap, &mut payload, RecvFlags::DONTWAIT) {
                Ok((0, 0)) => return,
                Ok((count, _)) => panic!(
                    "helper reported daemon exec failure stage {:#x}",
                    payload[..count][0]
                ),
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                    require_live_before_deadline(child, deadline, "daemon exec EOF");
                }
                Err(error) => panic!("observe daemon exec EOF: {error}"),
            }
        }
    }

    fn require_live_before_deadline(child: &mut Child, deadline: Instant, operation: &str) {
        if let Some(status) = child.try_wait().unwrap() {
            let mut stderr = String::new();
            child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut stderr)
                .unwrap();
            panic!("{operation} child exited {status}: {stderr}");
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {operation}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }

    fn duplicate_high(descriptor: &impl AsFd) -> OwnedFd {
        rustix::io::fcntl_dupfd_cloexec(descriptor, 400).unwrap()
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
