use std::fs::File;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fe2o3_compiler_closure_capability::{
    CompilerExecutionExternalAnchorSigningKeyCapabilityV1, CompilerExecutionPolicyCapabilityV1,
    CompilerExecutionSigningKeyCapabilityV1, CompilerExecutionSupervisorDeploymentCapabilityV1,
};
use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV1;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1,
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1, COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1,
    COMPILER_EXECUTION_SUPERVISOR_RUNTIME_DIRECTORY_MODE_V1,
    COMPILER_EXECUTION_SUPERVISOR_RUNTIME_DIRECTORY_V1,
    COMPILER_EXECUTION_SUPERVISOR_SOCKET_MODE_V1, COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1,
    CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionExternalAnchorProvisioningV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentV1,
};
use fe2o3_compiler_execution_supervisor::{
    IssuerServiceCredentialProfileV1, ProvisionedProtectedIssuerServiceInputsV1,
};
use fe2o3_external_anchor_coordinator::PreparedExternalAnchorOccurrenceV1;
use fe2o3_protected_service_spawn::require_exact_root_identity_v1;
use rustix::fs::{AtFlags, FileType, Mode, OFlags};
use rustix::net::{
    AddressFamily, SocketAddrAny, SocketAddrUnix, SocketFlags, SocketType, bind, socket_with,
};
use rustix::process::{Gid, Uid};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use crate::lifecycle::CompilerExecutionLifecycleLeaseV1;
use crate::{
    CompilerExecutionCoordinatorErrorV1, CompilerExecutionSupervisorProgramSourcesV1,
    CompilerExecutionSupervisorTrustV1, PreparedCompilerExecutionSupervisorV1,
    RootManagedCompilerExecutionServiceV1,
};

/// System-manager-owned production runtime root.
pub const COMPILER_EXECUTION_COORDINATOR_RUNTIME_ROOT_FD_V1: RawFd = 3;
/// Existing protected-supervisor service-owned mode-0700 state root.
pub const COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_ROOT_FD_V1: RawFd = 4;
/// Existing external-anchor service-owned mode-0700 state root.
pub const COMPILER_EXECUTION_COORDINATOR_ANCHOR_ROOT_FD_V1: RawFd = 5;
/// Root-provisioned static protected-supervisor image.
pub const COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_FD_V1: RawFd = 6;
/// Root-provisioned static issuer pre-exec launcher image.
pub const COMPILER_EXECUTION_COORDINATOR_LAUNCHER_FD_V1: RawFd = 7;
/// Root-provisioned static compiler-execution issuer image.
pub const COMPILER_EXECUTION_COORDINATOR_ISSUER_FD_V1: RawFd = 8;
/// Root-provisioned static external-anchor helper image.
pub const COMPILER_EXECUTION_COORDINATOR_ANCHOR_HELPER_FD_V1: RawFd = 9;
/// Root-provisioned static external-anchor daemon image.
pub const COMPILER_EXECUTION_COORDINATOR_ANCHOR_DAEMON_FD_V1: RawFd = 10;
/// Canonical protected-supervisor deployment record.
pub const COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_DEPLOYMENT_FD_V1: RawFd = 11;
/// Canonical compiler-execution issuer policy record.
pub const COMPILER_EXECUTION_COORDINATOR_POLICY_FD_V1: RawFd = 12;
/// Canonical external-anchor deployment record.
pub const COMPILER_EXECUTION_COORDINATOR_ANCHOR_DEPLOYMENT_FD_V1: RawFd = 13;
/// Canonical external-anchor provisioning record.
pub const COMPILER_EXECUTION_COORDINATOR_ANCHOR_PROVISIONING_FD_V1: RawFd = 14;
/// Root-owned raw issuer signing-key seed.
pub const COMPILER_EXECUTION_COORDINATOR_ISSUER_KEY_SEED_FD_V1: RawFd = 15;
/// Root-owned raw external-anchor signing-key seed.
pub const COMPILER_EXECUTION_COORDINATOR_ANCHOR_KEY_SEED_FD_V1: RawFd = 16;

const ROOT_ID_V1: u32 = 0;
const EXECUTABLE_MODE_V1: u32 = 0o555;
const PUBLIC_RECORD_MODE_V1: u32 = 0o444;
const SECRET_SEED_MODE_V1: u32 = 0o400;
const KEY_SEED_BYTES_V1: usize = 32;
const COMPILER_EXECUTION_SUPERVISOR_SOCKET_ENTRY_V1: &str = "compiler-execution-supervisor.sock";

const DESCRIPTORS_V1: [RawFd; 14] = [
    COMPILER_EXECUTION_COORDINATOR_RUNTIME_ROOT_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_ROOT_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_ROOT_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_LAUNCHER_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ISSUER_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_HELPER_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_DAEMON_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_DEPLOYMENT_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_POLICY_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_DEPLOYMENT_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_PROVISIONING_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ISSUER_KEY_SEED_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_KEY_SEED_FD_V1,
];

/// Move-only root-admitted inputs for the complete anchor plus supervisor deployment.
///
/// This is the only public composition that consumes the fixed production descriptor set. It
/// exposes no descriptor, key, signing operation, compiler authority, or partial service launch.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::InheritedCompilerExecutionDeploymentV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<InheritedCompilerExecutionDeploymentV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_coordinator::InheritedCompilerExecutionDeploymentV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<InheritedCompilerExecutionDeploymentV1>();
/// ```
pub struct InheritedCompilerExecutionDeploymentV1 {
    programs: CompilerExecutionSupervisorProgramSourcesV1,
    trust: CompilerExecutionSupervisorTrustV1,
    service_inputs: ProvisionedProtectedIssuerServiceInputsV1,
    anchor: PreparedExternalAnchorOccurrenceV1,
    supervisor_lifecycle: CompilerExecutionServiceLifecycleLeaseV1,
    lifecycle: CompilerExecutionLifecycleLeaseV1,
}

impl std::fmt::Debug for InheritedCompilerExecutionDeploymentV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InheritedCompilerExecutionDeploymentV1")
            .field("authority", &"complete-root-deployment-only")
            .finish_non_exhaustive()
    }
}

impl InheritedCompilerExecutionDeploymentV1 {
    /// Takes and admits the exact inherited production descriptor set under UID/GID 0.
    pub fn admit() -> Result<Self, CompilerExecutionCoordinatorErrorV1> {
        require_exact_root_identity_v1().map_err(CompilerExecutionCoordinatorErrorV1::Spawn)?;
        require_single_threaded_entrypoint()?;
        let [
            runtime_root,
            supervisor_root,
            anchor_root,
            supervisor,
            launcher,
            issuer,
            anchor_helper,
            anchor_daemon,
            supervisor_deployment,
            policy,
            anchor_deployment,
            anchor_provisioning,
            issuer_key_seed,
            anchor_key_seed,
        ] = take_inherited_descriptors()?;

        let lifecycle =
            CompilerExecutionLifecycleLeaseV1::admit_service_from_root(&supervisor_root)?;
        let supervisor_lifecycle = CompilerExecutionServiceLifecycleLeaseV1::open(&supervisor_root)
            .map_err(CompilerExecutionCoordinatorErrorV1::ServiceLifecycle)?;
        let anchor_lifecycle = CompilerExecutionServiceLifecycleLeaseV1::open(&anchor_root)
            .map_err(CompilerExecutionCoordinatorErrorV1::ServiceLifecycle)?;

        let supervisor_deployment =
            decode_supervisor_deployment(File::from(supervisor_deployment))?;
        let deployment_capability =
            CompilerExecutionSupervisorDeploymentCapabilityV1::create(supervisor_deployment)
                .map_err(CompilerExecutionCoordinatorErrorV1::DeploymentCapability)?;
        let credentials = IssuerServiceCredentialProfileV1::new(
            deployment_capability.deployment().service_uid(),
            deployment_capability.deployment().service_gid(),
        )
        .map_err(CompilerExecutionCoordinatorErrorV1::Credentials)?;
        let runtime_root = AdmittedRuntimeRootV1::admit(
            runtime_root,
            Path::new(COMPILER_EXECUTION_SUPERVISOR_RUNTIME_DIRECTORY_V1),
            ROOT_ID_V1,
            ROOT_ID_V1,
            COMPILER_EXECUTION_SUPERVISOR_RUNTIME_DIRECTORY_MODE_V1,
        )?;
        let mut listener = ConstructedRuntimeListenerV1::construct(
            runtime_root,
            Path::new(COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1),
            COMPILER_EXECUTION_SUPERVISOR_SOCKET_ENTRY_V1,
            ROOT_ID_V1,
            credentials.gid(),
            COMPILER_EXECUTION_SUPERVISOR_SOCKET_MODE_V1,
        )?;
        let service_inputs = ProvisionedProtectedIssuerServiceInputsV1::admit(
            listener.take_descriptor(),
            File::from(supervisor_root),
            credentials,
        )
        .map_err(CompilerExecutionCoordinatorErrorV1::ServiceInputs)?;

        let supervisor = admit_executable(File::from(supervisor), "supervisor executable")?;
        let launcher = admit_executable(File::from(launcher), "issuer pre-exec launcher")?;
        let issuer = admit_executable(File::from(issuer), "compiler-execution issuer")?;
        let anchor_helper = admit_executable(File::from(anchor_helper), "external-anchor helper")?;
        let anchor_daemon = admit_executable(File::from(anchor_daemon), "external-anchor daemon")?;

        let policy = decode_policy(File::from(policy))?;
        let anchor_deployment = decode_anchor_deployment(File::from(anchor_deployment))?;
        let anchor_provisioning = decode_anchor_provisioning(File::from(anchor_provisioning))?;

        let mut seed = read_seed(File::from(issuer_key_seed), "issuer signing-key seed")?;
        let issuer_key =
            CompilerExecutionSigningKeyCapabilityV1::create_and_zeroize(&mut seed, &policy)
                .map_err(CompilerExecutionCoordinatorErrorV1::KeyTemplate)?;
        let mut seed = read_seed(
            File::from(anchor_key_seed),
            "external-anchor signing-key seed",
        )?;
        let anchor_key = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
            &mut seed,
            &anchor_deployment,
        )
        .map_err(CompilerExecutionCoordinatorErrorV1::ExternalAnchorKeyTemplate)?;

        let policy_capability = CompilerExecutionPolicyCapabilityV1::create(policy)
            .map_err(CompilerExecutionCoordinatorErrorV1::PolicyCapability)?;
        let trust = CompilerExecutionSupervisorTrustV1::new(
            deployment_capability,
            policy_capability,
            issuer_key,
        )?;
        let anchor = PreparedExternalAnchorOccurrenceV1::prepare(
            anchor_helper,
            anchor_daemon,
            File::from(anchor_root),
            anchor_lifecycle,
            anchor_deployment,
            anchor_provisioning,
            anchor_key,
        )
        .map_err(CompilerExecutionCoordinatorErrorV1::Anchor)?;
        listener.disarm_cleanup();
        Ok(Self {
            programs: CompilerExecutionSupervisorProgramSourcesV1::new(
                supervisor, launcher, issuer,
            ),
            trust,
            service_inputs,
            anchor,
            supervisor_lifecycle,
            lifecycle,
        })
    }

    /// Launches the anchor first and then the exact bound supervisor under one timeout per stage.
    pub fn launch(
        self,
        timeout: Duration,
    ) -> Result<RootManagedCompilerExecutionServiceV1, CompilerExecutionCoordinatorErrorV1> {
        let anchor = self
            .anchor
            .launch(timeout)
            .map_err(CompilerExecutionCoordinatorErrorV1::Anchor)?;
        PreparedCompilerExecutionSupervisorV1::prepare(
            self.programs,
            self.trust,
            self.service_inputs,
            self.supervisor_lifecycle,
            self.lifecycle,
            anchor,
        )?
        .launch(timeout)
    }
}

struct AdmittedRuntimeRootV1 {
    descriptor: OwnedFd,
    expected_path: PathBuf,
    expected_owner: u32,
    expected_group: u32,
    expected_mode: u32,
    snapshot: RootFileSnapshotV1,
}

impl AdmittedRuntimeRootV1 {
    fn admit(
        descriptor: OwnedFd,
        expected_path: &Path,
        expected_owner: u32,
        expected_group: u32,
        expected_mode: u32,
    ) -> Result<Self, CompilerExecutionCoordinatorErrorV1> {
        if !expected_path.is_absolute() {
            return Err(invalid_provisioned(
                "runtime root",
                "expected pathname is not absolute",
            ));
        }
        let snapshot = validate_runtime_root(
            &descriptor,
            expected_path,
            expected_owner,
            expected_group,
            expected_mode,
        )?;
        Ok(Self {
            descriptor,
            expected_path: expected_path.to_owned(),
            expected_owner,
            expected_group,
            expected_mode,
            snapshot,
        })
    }

    fn revalidate(&self) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
        let current = validate_runtime_root(
            &self.descriptor,
            &self.expected_path,
            self.expected_owner,
            self.expected_group,
            self.expected_mode,
        )?;
        if current.device != self.snapshot.device
            || current.inode != self.snapshot.inode
            || current.mode != self.snapshot.mode
            || current.uid != self.snapshot.uid
            || current.gid != self.snapshot.gid
            || current.links != self.snapshot.links
        {
            return Err(invalid_provisioned(
                "runtime root",
                "directory identity changed after admission",
            ));
        }
        Ok(())
    }
}

struct ConstructedRuntimeListenerV1 {
    runtime_root: AdmittedRuntimeRootV1,
    descriptor: Option<OwnedFd>,
    socket_path: PathBuf,
    socket_entry: &'static str,
    path_identity: Option<(u64, u64)>,
    cleanup_armed: bool,
}

impl ConstructedRuntimeListenerV1 {
    fn construct(
        runtime_root: AdmittedRuntimeRootV1,
        socket_path: &Path,
        socket_entry: &'static str,
        socket_owner: u32,
        socket_group: u32,
        socket_mode: u32,
    ) -> Result<Self, CompilerExecutionCoordinatorErrorV1> {
        if socket_path.parent() != Some(runtime_root.expected_path.as_path())
            || socket_path.file_name() != Some(std::ffi::OsStr::new(socket_entry))
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "socket path is not the fixed runtime-root entry",
            ));
        }
        runtime_root.revalidate()?;
        require_runtime_entry_absent(&runtime_root.descriptor, socket_entry)?;

        let descriptor = socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|source| runtime_io("create compiler-execution listener", source))?;
        let address = SocketAddrUnix::new(socket_path)
            .map_err(|source| runtime_io("encode compiler-execution listener pathname", source))?;
        bind(&descriptor, &address)
            .map_err(|source| runtime_io("bind compiler-execution listener", source))?;

        let mut constructed = Self {
            runtime_root,
            descriptor: Some(descriptor),
            socket_path: socket_path.to_owned(),
            socket_entry,
            path_identity: None,
            cleanup_armed: true,
        };
        let created = runtime_entry_snapshot(
            &constructed.runtime_root.descriptor,
            constructed.socket_entry,
            "inspect newly bound compiler-execution listener",
        )?;
        constructed.path_identity = Some((created.device, created.inode));

        let owner = (created.uid != socket_owner).then(|| Uid::from_raw(socket_owner));
        let group = (created.gid != socket_group).then(|| Gid::from_raw(socket_group));
        if owner.is_some() || group.is_some() {
            rustix::fs::chownat(
                &constructed.runtime_root.descriptor,
                constructed.socket_entry,
                owner,
                group,
                AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|source| runtime_io("set compiler-execution listener ownership", source))?;
        }
        rustix::fs::chmodat(
            &constructed.runtime_root.descriptor,
            constructed.socket_entry,
            Mode::from_raw_mode(socket_mode),
            AtFlags::empty(),
        )
        .map_err(|source| runtime_io("set compiler-execution listener mode", source))?;

        constructed.revalidate(socket_owner, socket_group, socket_mode)?;
        Ok(constructed)
    }

    fn take_descriptor(&mut self) -> OwnedFd {
        self.descriptor
            .take()
            .expect("constructed listener descriptor transfers exactly once")
    }

    fn disarm_cleanup(mut self) {
        self.cleanup_armed = false;
    }

    fn revalidate(
        &self,
        socket_owner: u32,
        socket_group: u32,
        socket_mode: u32,
    ) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
        self.runtime_root.revalidate()?;
        let descriptor = self
            .descriptor
            .as_ref()
            .expect("listener remains in construction custody during validation");
        let descriptor_flags = rustix::io::fcntl_getfd(descriptor)
            .map_err(|source| runtime_io("inspect listener descriptor flags", source))?;
        let status = rustix::fs::fcntl_getfl(descriptor)
            .map_err(|source| runtime_io("inspect listener status flags", source))?;
        let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
        if descriptor_flags != rustix::io::FdFlags::CLOEXEC
            || status & OFlags::ACCMODE != OFlags::RDWR
            || !status.contains(OFlags::NONBLOCK)
            || status.intersects(forbidden)
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "descriptor flags are not exact nonblocking close-on-exec custody",
            ));
        }
        if rustix::net::sockopt::socket_domain(descriptor)
            .map_err(|source| runtime_io("inspect listener domain", source))?
            != AddressFamily::UNIX
            || rustix::net::sockopt::socket_type(descriptor)
                .map_err(|source| runtime_io("inspect listener type", source))?
                != SocketType::SEQPACKET
            || rustix::net::sockopt::socket_protocol(descriptor)
                .map_err(|source| runtime_io("inspect listener protocol", source))?
                .is_some()
            || rustix::net::sockopt::socket_acceptconn(descriptor)
                .map_err(|source| runtime_io("inspect listener state", source))?
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "endpoint is not a bound non-listening Unix SOCK_SEQPACKET socket",
            ));
        }
        let expected_address = SocketAddrAny::from(
            SocketAddrUnix::new(&self.socket_path)
                .map_err(|source| runtime_io("encode fixed listener pathname", source))?,
        );
        if rustix::net::getsockname(descriptor)
            .map_err(|source| runtime_io("inspect listener pathname", source))?
            != expected_address
            || socket_has_peer(descriptor)?
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "endpoint is not unconnected at the fixed pathname",
            ));
        }
        if rustix::net::sockopt::socket_error(descriptor)
            .map_err(|source| runtime_io("inspect listener socket error", source))?
            .is_err()
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "endpoint has a pending socket error",
            ));
        }
        let descriptor_stat = rustix::fs::fstat(descriptor)
            .map_err(|source| runtime_io("inspect listener descriptor", source))?;
        if FileType::from_raw_mode(descriptor_stat.st_mode) != FileType::Socket
            || descriptor_stat.st_nlink == 0
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "descriptor is not a live socket",
            ));
        }
        let path = runtime_entry_snapshot(
            &self.runtime_root.descriptor,
            self.socket_entry,
            "inspect compiler-execution listener pathname",
        )?;
        if FileType::from_raw_mode(path.mode) != FileType::Socket
            || path.mode & 0o7777 != socket_mode
            || path.uid != socket_owner
            || path.gid != socket_group
            || path.links != 1
            || path.byte_len != 0
            || self.path_identity != Some((path.device, path.inode))
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "pathname type, owner, group, mode, links, length, or identity is not exact",
            ));
        }
        require_no_runtime_entry_xattrs(
            &self.runtime_root.descriptor,
            self.socket_entry,
            "compiler-execution listener",
        )?;
        self.runtime_root.revalidate()
    }
}

impl Drop for ConstructedRuntimeListenerV1 {
    fn drop(&mut self) {
        if !self.cleanup_armed {
            return;
        }
        let Ok(current) = rustix::fs::statat(
            &self.runtime_root.descriptor,
            self.socket_entry,
            AtFlags::SYMLINK_NOFOLLOW,
        ) else {
            return;
        };
        let identity_matches = self
            .path_identity
            .is_none_or(|identity| identity == (current.st_dev, current.st_ino));
        if identity_matches && FileType::from_raw_mode(current.st_mode) == FileType::Socket {
            let _ = rustix::fs::unlinkat(
                &self.runtime_root.descriptor,
                self.socket_entry,
                AtFlags::empty(),
            );
        }
    }
}

fn validate_runtime_root(
    descriptor: &OwnedFd,
    expected_path: &Path,
    expected_owner: u32,
    expected_group: u32,
    expected_mode: u32,
) -> Result<RootFileSnapshotV1, CompilerExecutionCoordinatorErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(descriptor)
        .map_err(|source| runtime_io("inspect runtime-root descriptor flags", source))?;
    let status = rustix::fs::fcntl_getfl(descriptor)
        .map_err(|source| runtime_io("inspect runtime-root status flags", source))?;
    let descriptor_snapshot = RootFileSnapshotV1::from_stat(
        rustix::fs::fstat(descriptor)
            .map_err(|source| runtime_io("inspect runtime-root descriptor", source))?,
    );
    let path_snapshot = RootFileSnapshotV1::from_stat(
        rustix::fs::lstat(expected_path)
            .map_err(|source| runtime_io("inspect fixed runtime-root pathname", source))?,
    );
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.intersects(forbidden)
        || FileType::from_raw_mode(descriptor_snapshot.mode) != FileType::Directory
        || descriptor_snapshot.mode & 0o7777 != expected_mode
        || descriptor_snapshot.uid != expected_owner
        || descriptor_snapshot.gid != expected_group
        || descriptor_snapshot.links == 0
        || descriptor_snapshot != path_snapshot
    {
        return Err(invalid_provisioned(
            "runtime root",
            "descriptor/path identity, access, owner, group, mode, or links is not exact",
        ));
    }
    require_no_descriptor_xattrs(descriptor, "runtime root")?;
    Ok(descriptor_snapshot)
}

fn require_runtime_entry_absent(
    runtime_root: &OwnedFd,
    entry: &str,
) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    match rustix::fs::statat(runtime_root, entry, AtFlags::SYMLINK_NOFOLLOW) {
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Ok(_) => Err(invalid_provisioned(
            "compiler-execution listener",
            "fixed pathname already exists",
        )),
        Err(source) => Err(runtime_io("inspect fixed listener pathname", source)),
    }
}

fn runtime_entry_snapshot(
    runtime_root: &OwnedFd,
    entry: &str,
    operation: &'static str,
) -> Result<RootFileSnapshotV1, CompilerExecutionCoordinatorErrorV1> {
    rustix::fs::statat(runtime_root, entry, AtFlags::SYMLINK_NOFOLLOW)
        .map(RootFileSnapshotV1::from_stat)
        .map_err(|source| runtime_io(operation, source))
}

fn require_no_descriptor_xattrs(
    descriptor: &impl AsFd,
    role: &'static str,
) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    let mut attributes = [0_u8; 1];
    match rustix::fs::flistxattr(descriptor, &mut attributes) {
        Ok(0) => Ok(()),
        Ok(_) | Err(rustix::io::Errno::RANGE) => Err(invalid_provisioned(
            role,
            "object carries an extended attribute",
        )),
        Err(source) => Err(runtime_io(
            "inspect runtime-root extended attributes",
            source,
        )),
    }
}

fn require_no_runtime_entry_xattrs(
    runtime_root: &OwnedFd,
    entry: &str,
    role: &'static str,
) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    let path = format!("/proc/self/fd/{}/{entry}", runtime_root.as_raw_fd());
    let mut attributes = [0_u8; 1];
    match rustix::fs::llistxattr(path, &mut attributes) {
        Ok(0) => Ok(()),
        Ok(_) | Err(rustix::io::Errno::RANGE) => Err(invalid_provisioned(
            role,
            "pathname carries an extended attribute",
        )),
        Err(source) => Err(runtime_io("inspect listener extended attributes", source)),
    }
}

fn socket_has_peer(descriptor: &OwnedFd) -> Result<bool, CompilerExecutionCoordinatorErrorV1> {
    match rustix::net::getpeername(descriptor) {
        Ok(peer) => Ok(peer.is_some()),
        Err(rustix::io::Errno::NOTCONN) => Ok(false),
        Err(source) => Err(runtime_io("inspect listener peer", source)),
    }
}

fn invalid_provisioned(
    role: &'static str,
    reason: &'static str,
) -> CompilerExecutionCoordinatorErrorV1 {
    CompilerExecutionCoordinatorErrorV1::ProvisionedInput { role, reason }
}

fn runtime_io(
    operation: &'static str,
    source: rustix::io::Errno,
) -> CompilerExecutionCoordinatorErrorV1 {
    CompilerExecutionCoordinatorErrorV1::Io {
        operation,
        source: source.into(),
    }
}

fn require_single_threaded_entrypoint() -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    let tasks = std::fs::read_dir("/proc/self/task").map_err(|_| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role: "coordinator process",
            reason: "cannot inspect process thread set",
        }
    })?;
    if tasks.take(2).count() != 1 {
        return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role: "coordinator process",
            reason: "inherited descriptors must be admitted before creating threads",
        });
    }
    Ok(())
}

fn take_inherited_descriptors() -> Result<[OwnedFd; 14], CompilerExecutionCoordinatorErrorV1> {
    for descriptor in DESCRIPTORS_V1 {
        // SAFETY: F_GETFD accepts an integer descriptor and does not dereference process memory.
        let inherited_flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
        if inherited_flags < 0 {
            return Err(CompilerExecutionCoordinatorErrorV1::InheritedDescriptor {
                descriptor,
                operation: "inspect inherited descriptor flags",
                source: std::io::Error::last_os_error(),
            });
        }
        if inherited_flags != 0 {
            return Err(CompilerExecutionCoordinatorErrorV1::InheritedDescriptor {
                descriptor,
                operation: "require inheritable input descriptor",
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "descriptor was already close-on-exec",
                ),
            });
        }
    }
    // SAFETY: the complete preflight above proved every fixed descriptor valid; this one-shot
    // entrypoint contract transfers exclusive ownership before any thread can close or reuse one.
    let descriptors = DESCRIPTORS_V1.map(|descriptor| unsafe { OwnedFd::from_raw_fd(descriptor) });
    for descriptor in &descriptors {
        rustix::io::fcntl_setfd(descriptor, rustix::io::FdFlags::CLOEXEC).map_err(|source| {
            CompilerExecutionCoordinatorErrorV1::InheritedDescriptor {
                descriptor: descriptor.as_raw_fd(),
                operation: "protect inherited descriptor",
                source: source.into(),
            }
        })?;
    }
    Ok(descriptors)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RootFileSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    pub(crate) byte_len: u64,
}

impl RootFileSnapshotV1 {
    fn from_stat(stat: rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            uid: stat.st_uid,
            gid: stat.st_gid,
            links: stat.st_nlink,
            byte_len: stat.st_size.try_into().unwrap_or(u64::MAX),
        }
    }
}

fn admit_executable(
    executable: File,
    role: &'static str,
) -> Result<File, CompilerExecutionCoordinatorErrorV1> {
    validate_root_file(&executable, role, EXECUTABLE_MODE_V1, None)?;
    Ok(executable)
}

fn decode_supervisor_deployment(
    file: File,
) -> Result<CompilerExecutionSupervisorDeploymentV1, CompilerExecutionCoordinatorErrorV1> {
    let bytes = read_stable_record::<COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1>(
        &file,
        "supervisor deployment",
    )?;
    CompilerExecutionSupervisorDeploymentV1::decode(&bytes).map_err(|error| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedRecord {
            role: "supervisor deployment",
            reason: error.to_string(),
        }
    })
}

fn decode_policy(
    file: File,
) -> Result<CompilerExecutionIssuerPolicyV1, CompilerExecutionCoordinatorErrorV1> {
    let bytes =
        read_stable_record::<COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1>(&file, "issuer policy")?;
    CompilerExecutionIssuerPolicyV1::decode(&bytes).map_err(|error| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedRecord {
            role: "issuer policy",
            reason: error.to_string(),
        }
    })
}

fn decode_anchor_deployment(
    file: File,
) -> Result<CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionCoordinatorErrorV1> {
    let bytes = read_stable_record::<COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1>(
        &file,
        "external-anchor deployment",
    )?;
    CompilerExecutionExternalAnchorDeploymentV1::decode(&bytes).map_err(|error| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedRecord {
            role: "external-anchor deployment",
            reason: error.to_string(),
        }
    })
}

fn decode_anchor_provisioning(
    file: File,
) -> Result<CompilerExecutionExternalAnchorProvisioningV1, CompilerExecutionCoordinatorErrorV1> {
    let bytes = read_stable_record::<COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1>(
        &file,
        "external-anchor provisioning",
    )?;
    CompilerExecutionExternalAnchorProvisioningV1::decode(&bytes).map_err(|error| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedRecord {
            role: "external-anchor provisioning",
            reason: error.to_string(),
        }
    })
}

fn read_stable_record<const N: usize>(
    file: &File,
    role: &'static str,
) -> Result<[u8; N], CompilerExecutionCoordinatorErrorV1> {
    let before = validate_root_file(file, role, PUBLIC_RECORD_MODE_V1, Some(N))?;
    let first = read_exact_at::<N>(file, role)?;
    let second = read_exact_at::<N>(file, role)?;
    let after = validate_root_file(file, role, PUBLIC_RECORD_MODE_V1, Some(N))?;
    if before != after || first != second {
        return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "record changed during admission",
        });
    }
    Ok(first)
}

fn read_seed(
    file: File,
    role: &'static str,
) -> Result<[u8; KEY_SEED_BYTES_V1], CompilerExecutionCoordinatorErrorV1> {
    let before = validate_root_file(&file, role, SECRET_SEED_MODE_V1, Some(KEY_SEED_BYTES_V1))?;
    let mut first = read_seed_copy(&file, role)?;
    let mut second = read_seed_copy(&file, role)?;
    let after = validate_root_file(&file, role, SECRET_SEED_MODE_V1, Some(KEY_SEED_BYTES_V1))?;
    if before != after || !bool::from(first.ct_eq(&second)) {
        first.zeroize();
        second.zeroize();
        return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "key seed changed during admission",
        });
    }
    second.zeroize();
    Ok(first)
}

fn read_seed_copy(
    file: &File,
    role: &'static str,
) -> Result<[u8; KEY_SEED_BYTES_V1], CompilerExecutionCoordinatorErrorV1> {
    let mut bytes = [0_u8; KEY_SEED_BYTES_V1];
    if file.read_exact_at(&mut bytes, 0).is_err() {
        bytes.zeroize();
        return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "cannot read exact key-seed bytes",
        });
    }
    Ok(bytes)
}

fn read_exact_at<const N: usize>(
    file: &File,
    role: &'static str,
) -> Result<[u8; N], CompilerExecutionCoordinatorErrorV1> {
    let mut bytes = [0_u8; N];
    file.read_exact_at(&mut bytes, 0).map_err(|_| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "cannot read exact bytes",
        }
    })?;
    Ok(bytes)
}

fn validate_root_file(
    file: &File,
    role: &'static str,
    expected_mode: u32,
    expected_length: Option<usize>,
) -> Result<RootFileSnapshotV1, CompilerExecutionCoordinatorErrorV1> {
    validate_provisioned_file(
        file,
        role,
        expected_mode,
        expected_length,
        ROOT_ID_V1,
        ROOT_ID_V1,
    )
}

pub(crate) fn validate_provisioned_file(
    file: &File,
    role: &'static str,
    expected_mode: u32,
    expected_length: Option<usize>,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<RootFileSnapshotV1, CompilerExecutionCoordinatorErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(file).map_err(|_| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "cannot inspect descriptor flags",
        }
    })?;
    let status = rustix::fs::fcntl_getfl(file).map_err(|_| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "cannot inspect status flags",
        }
    })?;
    let stat = rustix::fs::fstat(file).map_err(|_| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "cannot inspect object metadata",
        }
    })?;
    let snapshot = RootFileSnapshotV1::from_stat(stat);
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.intersects(forbidden)
        || FileType::from_raw_mode(snapshot.mode) != FileType::RegularFile
        || snapshot.mode & 0o7777 != expected_mode
        || snapshot.uid != expected_uid
        || snapshot.gid != expected_gid
        || snapshot.links != 1
        || expected_length.is_some_and(|length| snapshot.byte_len != length as u64)
        || expected_length.is_none() && snapshot.byte_len == 0
    {
        return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "type, access, owner, group, mode, links, or length is not exact",
        });
    }
    for attribute in [
        "security.capability",
        "system.posix_acl_access",
        "system.posix_acl_default",
    ] {
        let mut byte = 0_u8;
        match rustix::fs::fgetxattr(file, attribute, std::slice::from_mut(&mut byte)) {
            Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => {}
            Ok(_) | Err(rustix::io::Errno::RANGE) => {
                return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
                    role,
                    reason: "file has a forbidden capability or POSIX ACL",
                });
            }
            Err(_) => {
                return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
                    role,
                    reason: "cannot inspect capability or POSIX ACL metadata",
                });
            }
        }
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use super::*;

    const TEST_SOCKET_ENTRY_V1: &str = "listener.sock";

    #[test]
    fn inherited_descriptor_contract_is_dense_unique_and_fixed() {
        assert_eq!(
            DESCRIPTORS_V1,
            std::array::from_fn(|index| 3 + index as i32)
        );
    }

    #[test]
    fn inherited_deployment_is_move_only_and_descriptor_free() {
        fn assert_send<T: Send>() {}
        assert_send::<InheritedCompilerExecutionDeploymentV1>();
        assert!(!std::mem::needs_drop::<RootFileSnapshotV1>());
    }

    #[test]
    fn runtime_root_admission_requires_exact_path_mode_and_no_xattrs() {
        let fixture = tempfile::tempdir().unwrap();
        set_mode(fixture.path(), 0o755);
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        let descriptor: OwnedFd = File::open(fixture.path()).unwrap().into();
        AdmittedRuntimeRootV1::admit(descriptor, fixture.path(), uid, gid, 0o755).unwrap();

        set_mode(fixture.path(), 0o750);
        let descriptor: OwnedFd = File::open(fixture.path()).unwrap().into();
        assert!(AdmittedRuntimeRootV1::admit(descriptor, fixture.path(), uid, gid, 0o755).is_err());

        set_mode(fixture.path(), 0o755);
        let descriptor = File::open(fixture.path()).unwrap();
        if rustix::fs::fsetxattr(
            &descriptor,
            "user.fe2o3-runtime-root-test",
            b"present",
            rustix::fs::XattrFlags::CREATE,
        )
        .is_ok()
        {
            let descriptor: OwnedFd = descriptor.into();
            assert!(
                AdmittedRuntimeRootV1::admit(descriptor, fixture.path(), uid, gid, 0o755).is_err()
            );
        }
    }

    #[test]
    fn constructed_listener_is_exactly_bound_and_non_listening() {
        let fixture = tempfile::tempdir().unwrap();
        let socket_path = fixture.path().join(TEST_SOCKET_ENTRY_V1);
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        let listener = ConstructedRuntimeListenerV1::construct(
            admit_test_runtime_root(fixture.path()),
            &socket_path,
            TEST_SOCKET_ENTRY_V1,
            uid,
            gid,
            0o660,
        )
        .unwrap();
        let descriptor = listener.descriptor.as_ref().unwrap();

        assert_eq!(
            rustix::net::getsockname(descriptor).unwrap(),
            SocketAddrAny::from(SocketAddrUnix::new(&socket_path).unwrap())
        );
        assert!(!rustix::net::sockopt::socket_acceptconn(descriptor).unwrap());
        let path = rustix::fs::statat(
            &listener.runtime_root.descriptor,
            TEST_SOCKET_ENTRY_V1,
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .unwrap();
        assert_eq!(FileType::from_raw_mode(path.st_mode), FileType::Socket);
        assert_eq!(path.st_mode & 0o7777, 0o660);
        assert_eq!((path.st_uid, path.st_gid), (uid, gid));
        assert_eq!(path.st_nlink, 1);
        assert_eq!(path.st_size, 0);
        listener.revalidate(uid, gid, 0o660).unwrap();

        drop(listener);
        assert!(!socket_path.exists());
    }

    #[test]
    fn constructed_listener_rejects_an_occupied_fixed_path_without_replacement() {
        let fixture = tempfile::tempdir().unwrap();
        let socket_path = fixture.path().join(TEST_SOCKET_ENTRY_V1);
        fs::write(&socket_path, b"occupied").unwrap();
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();

        assert!(
            ConstructedRuntimeListenerV1::construct(
                admit_test_runtime_root(fixture.path()),
                &socket_path,
                TEST_SOCKET_ENTRY_V1,
                uid,
                gid,
                0o660,
            )
            .is_err()
        );
        assert_eq!(fs::read(&socket_path).unwrap(), b"occupied");
    }

    #[test]
    fn constructed_listener_cleans_the_path_on_post_bind_error() {
        let fixture = tempfile::tempdir().unwrap();
        let socket_path = fixture.path().join(TEST_SOCKET_ENTRY_V1);
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();

        assert!(
            ConstructedRuntimeListenerV1::construct(
                admit_test_runtime_root(fixture.path()),
                &socket_path,
                TEST_SOCKET_ENTRY_V1,
                uid,
                gid,
                0o10_000,
            )
            .is_err()
        );
        assert!(!socket_path.exists());
    }

    #[test]
    fn listener_cleanup_does_not_unlink_a_replacement_inode() {
        let fixture = tempfile::tempdir().unwrap();
        let socket_path = fixture.path().join(TEST_SOCKET_ENTRY_V1);
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        let listener = ConstructedRuntimeListenerV1::construct(
            admit_test_runtime_root(fixture.path()),
            &socket_path,
            TEST_SOCKET_ENTRY_V1,
            uid,
            gid,
            0o660,
        )
        .unwrap();

        fs::remove_file(&socket_path).unwrap();
        fs::write(&socket_path, b"replacement").unwrap();
        drop(listener);
        assert_eq!(fs::read(&socket_path).unwrap(), b"replacement");
    }

    #[test]
    fn listener_revalidation_rejects_any_path_xattr_when_supported() {
        let fixture = tempfile::tempdir().unwrap();
        let socket_path = fixture.path().join(TEST_SOCKET_ENTRY_V1);
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        let listener = ConstructedRuntimeListenerV1::construct(
            admit_test_runtime_root(fixture.path()),
            &socket_path,
            TEST_SOCKET_ENTRY_V1,
            uid,
            gid,
            0o660,
        )
        .unwrap();

        if rustix::fs::lsetxattr(
            &socket_path,
            "user.fe2o3-listener-test",
            b"present",
            rustix::fs::XattrFlags::CREATE,
        )
        .is_ok()
        {
            assert!(listener.revalidate(uid, gid, 0o660).is_err());
        }
    }

    #[test]
    fn provisioned_file_policy_accepts_only_exact_immutable_shape() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("record");
        fs::write(&path, [0x5a; 32]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(PUBLIC_RECORD_MODE_V1)).unwrap();
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        let file = File::open(&path).unwrap();
        validate_provisioned_file(
            &file,
            "test record",
            PUBLIC_RECORD_MODE_V1,
            Some(32),
            uid,
            gid,
        )
        .unwrap();

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            validate_provisioned_file(
                &file,
                "test record",
                PUBLIC_RECORD_MODE_V1,
                Some(32),
                uid,
                gid,
            )
            .is_err()
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(PUBLIC_RECORD_MODE_V1)).unwrap();

        assert!(
            validate_provisioned_file(
                &file,
                "test record",
                PUBLIC_RECORD_MODE_V1,
                Some(31),
                uid,
                gid,
            )
            .is_err()
        );
        assert!(
            validate_provisioned_file(
                &file,
                "test record",
                PUBLIC_RECORD_MODE_V1,
                Some(32),
                different_id(uid),
                gid,
            )
            .is_err()
        );
        assert!(
            validate_provisioned_file(
                &file,
                "test record",
                PUBLIC_RECORD_MODE_V1,
                Some(32),
                uid,
                different_id(gid),
            )
            .is_err()
        );

        fs::hard_link(&path, fixture.path().join("record-link")).unwrap();
        assert!(
            validate_provisioned_file(
                &file,
                "test record",
                PUBLIC_RECORD_MODE_V1,
                Some(32),
                uid,
                gid,
            )
            .is_err()
        );
    }

    #[test]
    fn provisioned_file_policy_rejects_writable_and_empty_executables() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("writable-record");
        let mut writable = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        writable.write_all(&[0x5a; 32]).unwrap();
        writable.flush().unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(PUBLIC_RECORD_MODE_V1)).unwrap();
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        assert!(
            validate_provisioned_file(
                &writable,
                "test record",
                PUBLIC_RECORD_MODE_V1,
                Some(32),
                uid,
                gid,
            )
            .is_err()
        );

        let empty = fixture.path().join("empty-executable");
        fs::write(&empty, []).unwrap();
        fs::set_permissions(&empty, fs::Permissions::from_mode(EXECUTABLE_MODE_V1)).unwrap();
        assert!(
            validate_provisioned_file(
                &File::open(empty).unwrap(),
                "test executable",
                EXECUTABLE_MODE_V1,
                None,
                uid,
                gid,
            )
            .is_err()
        );
    }

    fn different_id(id: u32) -> u32 {
        if id == u32::MAX - 1 {
            u32::MAX - 2
        } else {
            id + 1
        }
    }

    fn admit_test_runtime_root(path: &Path) -> AdmittedRuntimeRootV1 {
        set_mode(path, 0o755);
        let descriptor: OwnedFd = File::open(path).unwrap().into();
        AdmittedRuntimeRootV1::admit(
            descriptor,
            path,
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
            0o755,
        )
        .unwrap()
    }

    fn set_mode(path: &Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }
}
