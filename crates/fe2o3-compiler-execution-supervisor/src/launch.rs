//! Exact launch-materialization custody for one admitted compiler occurrence.

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use fe2o3_broker_authority_service::{
    ExpectedClientProcessIdentityV1, LiveClientPidfdIdentityV1, ProtectedServiceAdmissionErrorV1,
    current_process_start_time_ticks_v1,
};
use fe2o3_compiler_closure_capability::CompilerExecutionServiceLaunchCapabilityV1;
use fe2o3_compiler_execution_issuer::{
    COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PEER_FD_V1,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PIDFD_V1,
    COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1, COMPILER_EXECUTION_ISSUER_PEER_FD_V1,
    COMPILER_EXECUTION_ISSUER_POLICY_FD_V1, COMPILER_EXECUTION_ISSUER_READY_FD_V1,
    COMPILER_EXECUTION_ISSUER_ROOT_FD_V1, COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV1;
use fe2o3_static_preexec_manifest::{
    PREEXEC_MANIFEST_BYTES_V1, StaticPreexecDescriptorV1, StaticPreexecManifestErrorV1,
    StaticPreexecManifestV1, StaticPreexecObjectIdentityV1,
};
use rustix::fs::{FileType, MemfdFlags, Mode, OFlags, SealFlags};
use rustix::pipe::{PipeFlags, pipe_with};

use crate::authority::ExternalAnchorLaunchClonesV1;
use crate::handoff::validate_service_peer;
use crate::{
    AcceptedCompilerExecutionHandoffV1, ProtectedIssuerHandoffErrorV1,
    ProtectedIssuerSupervisorErrorV1, ProtectedIssuerSupervisorV1,
};

const SOURCE_COUNT_V1: usize = 12;
const STDIN_SOURCE_INDEX: usize = 0;
const STDOUT_SOURCE_INDEX: usize = 1;
const STDERR_SOURCE_INDEX: usize = 2;
const ROOT_SOURCE_INDEX: usize = 3;
const SERVICE_PEER_SOURCE_INDEX: usize = 4;
const CLIENT_PIDFD_SOURCE_INDEX: usize = 5;
const POLICY_SOURCE_INDEX: usize = 6;
const SIGNING_KEY_SOURCE_INDEX: usize = 7;
const LAUNCH_MANIFEST_SOURCE_INDEX: usize = 8;
const READINESS_SOURCE_INDEX: usize = 9;
const EXTERNAL_ANCHOR_PEER_SOURCE_INDEX: usize = 10;
const EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX: usize = 11;
const MANIFEST_MODE_V1: u32 = 0o400;
const REQUIRED_MANIFEST_SEALS_V1: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

const DESTINATION_FDS_V1: [i32; SOURCE_COUNT_V1] = [
    libc::STDIN_FILENO,
    libc::STDOUT_FILENO,
    libc::STDERR_FILENO,
    COMPILER_EXECUTION_ISSUER_ROOT_FD_V1,
    COMPILER_EXECUTION_ISSUER_PEER_FD_V1,
    COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1,
    COMPILER_EXECUTION_ISSUER_POLICY_FD_V1,
    COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1,
    COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1,
    COMPILER_EXECUTION_ISSUER_READY_FD_V1,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PEER_FD_V1,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PIDFD_V1,
];

const _: () = assert!(DESTINATION_FDS_V1[0] == 0);
const _: () = assert!(DESTINATION_FDS_V1[1] == 1);
const _: () = assert!(DESTINATION_FDS_V1[2] == 2);
const _: () = assert!(DESTINATION_FDS_V1[3] == 3);
const _: () = assert!(DESTINATION_FDS_V1[4] == 4);
const _: () = assert!(DESTINATION_FDS_V1[5] == 5);
const _: () = assert!(DESTINATION_FDS_V1[6] == 6);
const _: () = assert!(DESTINATION_FDS_V1[7] == 7);
const _: () = assert!(DESTINATION_FDS_V1[8] == 8);
const _: () = assert!(DESTINATION_FDS_V1[9] == 9);
const _: () = assert!(DESTINATION_FDS_V1[10] == 10);
const _: () = assert!(DESTINATION_FDS_V1[11] == 11);

/// Move-only, fully materialized input to the static protected-issuer launcher.
///
/// This value retains the authenticated handoff, exact launcher and issuer
/// images, sealed 704-byte pre-exec manifest, all twelve ordered source objects,
/// and the supervisor sides of output and readiness pipes. It exposes only
/// inert canonical manifests. Process creation consumes this value in the next
/// supervisor checkpoint.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<PreparedProtectedIssuerLaunchV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<PreparedProtectedIssuerLaunchV1>();
/// ```
pub struct PreparedProtectedIssuerLaunchV1 {
    pub(super) accepted: AcceptedCompilerExecutionHandoffV1,
    pub(super) launch_capability: CompilerExecutionServiceLaunchCapabilityV1,
    pub(super) launcher: File,
    pub(super) issuer: File,
    pub(super) static_manifest_file: File,
    pub(super) sources: [File; SOURCE_COUNT_V1],
    pub(super) stdout_reader: OwnedFd,
    pub(super) stderr_reader: OwnedFd,
    pub(super) readiness_reader: OwnedFd,
    pub(super) static_manifest: StaticPreexecManifestV1,
    manifest_object: StaticPreexecObjectIdentityV1,
}

impl fmt::Debug for PreparedProtectedIssuerLaunchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedProtectedIssuerLaunchV1")
            .field("authority", &"launch-custody-only")
            .field("parent_pid", &self.static_manifest.parent_pid())
            .field(
                "descriptor_count",
                &self.static_manifest.descriptors().len(),
            )
            .field("launch_identity", &self.accepted.handoff.identity())
            .finish_non_exhaustive()
    }
}

impl PreparedProtectedIssuerLaunchV1 {
    /// Returns the exact static pre-exec manifest without exposing descriptors.
    pub const fn static_manifest(&self) -> &StaticPreexecManifestV1 {
        &self.static_manifest
    }

    /// Returns the exact rustc and issuer-policy launch manifest.
    pub const fn service_manifest(&self) -> &CompilerExecutionServiceLaunchManifestV1 {
        self.launch_capability.manifest()
    }

    /// Revalidates every retained authority, object, pipe, and canonical byte record.
    pub fn revalidate(
        &self,
        supervisor: &ProtectedIssuerSupervisorV1,
    ) -> Result<(), ProtectedIssuerLaunchPreparationErrorV1> {
        supervisor
            .revalidate()
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::Supervisor)?;
        self.accepted
            .revalidate(supervisor)
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::Handoff)?;
        self.launch_capability
            .revalidate()
            .map_err(|source| capability_error("service launch manifest", source))?;
        if self.launch_capability.manifest() != self.accepted.manifest() {
            return Err(ProtectedIssuerLaunchPreparationErrorV1::LaunchManifestMismatch);
        }
        revalidate_parent(&self.static_manifest)?;

        supervisor
            .revalidate_launch_clones(
                &self.launcher,
                &self.issuer,
                &self.sources[ROOT_SOURCE_INDEX],
                &self.sources[POLICY_SOURCE_INDEX],
                &self.sources[SIGNING_KEY_SOURCE_INDEX],
                ExternalAnchorLaunchClonesV1 {
                    peer: &self.sources[EXTERNAL_ANCHOR_PEER_SOURCE_INDEX],
                    pidfd: &self.sources[EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX],
                },
            )
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::Supervisor)?;
        validate_service_peer(
            &self.sources[SERVICE_PEER_SOURCE_INDEX],
            self.accepted.manifest().client(),
        )
        .map_err(ProtectedIssuerLaunchPreparationErrorV1::Handoff)?;
        validate_client_pidfd(
            &self.sources[CLIENT_PIDFD_SOURCE_INDEX],
            self.accepted.manifest().client(),
        )?;
        validate_launch_capability_source(
            &self.sources[LAUNCH_MANIFEST_SOURCE_INDEX],
            &self.launch_capability,
        )?;

        validate_pipe_pair(
            &self.sources[STDOUT_SOURCE_INDEX],
            &self.stdout_reader,
            "stdout",
        )?;
        validate_pipe_pair(
            &self.sources[STDERR_SOURCE_INDEX],
            &self.stderr_reader,
            "stderr",
        )?;
        validate_pipe_pair(
            &self.sources[READINESS_SOURCE_INDEX],
            &self.readiness_reader,
            "readiness",
        )?;
        validate_pipe_end(&self.sources[STDIN_SOURCE_INDEX], OFlags::RDONLY, "stdin")?;
        validate_pipe_end(
            &self.sources[STDOUT_SOURCE_INDEX],
            OFlags::WRONLY,
            "stdout writer",
        )?;
        validate_pipe_end(
            &self.sources[STDERR_SOURCE_INDEX],
            OFlags::WRONLY,
            "stderr writer",
        )?;
        validate_pipe_end(
            &self.sources[READINESS_SOURCE_INDEX],
            OFlags::WRONLY,
            "readiness writer",
        )?;
        if rustix::pipe::fcntl_getpipe_size(&self.sources[READINESS_SOURCE_INDEX])
            .map_err(|source| io_error("inspect readiness pipe capacity", source.into()))?
            < fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SERVICE_READY_BYTES_V1
        {
            return Err(ProtectedIssuerLaunchPreparationErrorV1::InvalidDescriptor {
                role: "readiness writer",
                reason: "pipe capacity is smaller than one atomic readiness record",
            });
        }

        let source_objects = source_identities(&self.sources)?;
        validate_static_manifest_sources(&self.static_manifest, &self.issuer, &source_objects)?;
        validate_manifest_file(
            &self.static_manifest_file,
            &self.static_manifest,
            self.manifest_object,
        )?;
        require_launcher_non_aliasing(
            &self.launcher,
            &self.issuer,
            &self.static_manifest_file,
            &self.sources,
        )?;
        Ok(())
    }
}

impl ProtectedIssuerSupervisorV1 {
    /// Materializes one admitted rustc handoff into the exact static-launcher input set.
    ///
    /// The ordered source table is fixed to destinations `0..=11`: isolated
    /// standard streams, root, service peer, rustc pidfd, policy, signing key,
    /// service launch manifest, readiness writer, external-anchor endpoint, and
    /// external-anchor pidfd. Every source descriptor is close-on-exec until the
    /// static launcher installs its destination table.
    pub fn prepare_launch(
        &self,
        accepted: AcceptedCompilerExecutionHandoffV1,
    ) -> Result<PreparedProtectedIssuerLaunchV1, ProtectedIssuerLaunchPreparationErrorV1> {
        self.revalidate()
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::Supervisor)?;
        accepted
            .revalidate(self)
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::Handoff)?;

        let launcher = self
            .clone_launcher_for_launch()
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::Supervisor)?;
        let issuer = self
            .clone_issuer_for_launch()
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::Supervisor)?;
        let root = self
            .clone_root_for_launch()
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::Supervisor)?;
        let policy = self
            .clone_policy_for_launch()
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::Supervisor)?;
        let signing_key = self
            .clone_signing_key_for_launch()
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::Supervisor)?;
        let (external_anchor_peer, external_anchor_pidfd) = self
            .clone_external_anchor_for_launch()
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::Supervisor)?;
        let service_peer = clone_owned_descriptor(
            &accepted.service_peer,
            "clone admitted rustc service peer for launch",
        )?;
        let client_pidfd = clone_owned_descriptor(
            &accepted.client_pidfd,
            "clone admitted rustc pidfd for launch",
        )?;

        let launch_capability =
            CompilerExecutionServiceLaunchCapabilityV1::create(accepted.manifest().clone())
                .map_err(|source| capability_error("service launch manifest", source))?;
        let launch_manifest = launch_capability
            .try_clone_for_transfer()
            .map_err(|source| capability_error("service launch manifest", source))?;

        let (stdin_reader, stdin_writer) = protected_pipe("stdin")?;
        drop(stdin_writer);
        let (stdout_reader, stdout_writer) = protected_pipe("stdout")?;
        let (stderr_reader, stderr_writer) = protected_pipe("stderr")?;
        let (readiness_reader, readiness_writer) = protected_pipe("readiness")?;

        let sources = [
            File::from(stdin_reader),
            File::from(stdout_writer),
            File::from(stderr_writer),
            root,
            service_peer,
            client_pidfd,
            policy,
            signing_key,
            launch_manifest,
            File::from(readiness_writer),
            external_anchor_peer,
            external_anchor_pidfd,
        ];
        let source_objects = source_identities(&sources)?;
        let descriptors = DESTINATION_FDS_V1
            .into_iter()
            .zip(source_objects)
            .enumerate()
            .map(|(index, (destination, object))| {
                StaticPreexecDescriptorV1::for_index(index, destination, object)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::StaticManifest)?;
        let parent_pid = i32::try_from(std::process::id())
            .map_err(|_| ProtectedIssuerLaunchPreparationErrorV1::InvalidParentIdentity)?;
        let parent_start_time = current_process_start_time_ticks_v1()
            .map_err(ProtectedIssuerLaunchPreparationErrorV1::ParentIdentity)?;
        let static_manifest = StaticPreexecManifestV1::new(
            parent_pid,
            parent_start_time,
            object_identity(&issuer, "compiler issuer")?,
            descriptors,
        )
        .map_err(ProtectedIssuerLaunchPreparationErrorV1::StaticManifest)?;
        let (static_manifest_file, manifest_object) = create_manifest_file(&static_manifest)?;

        let prepared = PreparedProtectedIssuerLaunchV1 {
            accepted,
            launch_capability,
            launcher,
            issuer,
            static_manifest_file,
            sources,
            stdout_reader,
            stderr_reader,
            readiness_reader,
            static_manifest,
            manifest_object,
        };
        prepared.revalidate(self)?;
        Ok(prepared)
    }
}

fn protected_pipe(
    role: &'static str,
) -> Result<(OwnedFd, OwnedFd), ProtectedIssuerLaunchPreparationErrorV1> {
    let pair = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)
        .map_err(|source| io_error("create protected issuer pipe", source.into()))?;
    validate_pipe_end(&pair.0, OFlags::RDONLY, role)?;
    validate_pipe_end(&pair.1, OFlags::WRONLY, role)?;
    Ok(pair)
}

fn clone_owned_descriptor(
    source: &OwnedFd,
    operation: &'static str,
) -> Result<File, ProtectedIssuerLaunchPreparationErrorV1> {
    rustix::io::fcntl_dupfd_cloexec(source, 0)
        .map(File::from)
        .map_err(|source| io_error(operation, source.into()))
}

fn source_identities(
    sources: &[File; SOURCE_COUNT_V1],
) -> Result<[StaticPreexecObjectIdentityV1; SOURCE_COUNT_V1], ProtectedIssuerLaunchPreparationErrorV1>
{
    let identities = sources
        .iter()
        .enumerate()
        .map(|(index, source)| source_object_identity(index, source))
        .collect::<Result<Vec<_>, _>>()?;
    identities.try_into().map_err(
        |_| ProtectedIssuerLaunchPreparationErrorV1::InvalidDescriptor {
            role: "source table",
            reason: "source descriptor cardinality changed",
        },
    )
}

fn source_object_identity(
    index: usize,
    source: &File,
) -> Result<StaticPreexecObjectIdentityV1, ProtectedIssuerLaunchPreparationErrorV1> {
    let identity = object_identity(source, source_role(index))?;
    if matches!(
        index,
        CLIENT_PIDFD_SOURCE_INDEX | EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX
    ) {
        return Ok(StaticPreexecObjectIdentityV1::new_process_pidfd(
            identity.device(),
            identity.inode(),
            identity.size(),
            identity.mode(),
        ));
    }
    Ok(identity)
}

fn source_role(index: usize) -> &'static str {
    const ROLES: [&str; SOURCE_COUNT_V1] = [
        "stdin",
        "stdout",
        "stderr",
        "issuer root",
        "rustc service peer",
        "rustc pidfd",
        "issuer policy",
        "issuer signing key",
        "service launch manifest",
        "readiness writer",
        "external-anchor peer",
        "external-anchor pidfd",
    ];
    ROLES[index]
}

fn object_identity(
    descriptor: &impl AsFd,
    role: &'static str,
) -> Result<StaticPreexecObjectIdentityV1, ProtectedIssuerLaunchPreparationErrorV1> {
    let stat = rustix::fs::fstat(descriptor)
        .map_err(|source| io_error("inspect protected launch object", source.into()))?;
    let size = u64::try_from(stat.st_size).map_err(|_| {
        ProtectedIssuerLaunchPreparationErrorV1::InvalidDescriptor {
            role,
            reason: "object size is negative",
        }
    })?;
    Ok(StaticPreexecObjectIdentityV1::new(
        stat.st_dev,
        stat.st_ino,
        size,
        stat.st_mode,
    ))
}

fn validate_static_manifest_sources(
    manifest: &StaticPreexecManifestV1,
    issuer: &File,
    sources: &[StaticPreexecObjectIdentityV1; SOURCE_COUNT_V1],
) -> Result<(), ProtectedIssuerLaunchPreparationErrorV1> {
    if manifest.descriptors().len() != SOURCE_COUNT_V1
        || manifest.executable() != &object_identity(issuer, "compiler issuer")?
        || manifest
            .descriptors()
            .iter()
            .zip(DESTINATION_FDS_V1)
            .zip(sources)
            .enumerate()
            .any(|(index, ((entry, destination), source))| {
                entry.source_fd()
                    != fe2o3_static_preexec_manifest::PREEXEC_SOURCE_FD_BASE + index as i32
                    || entry.destination_fd() != destination
                    || entry.object() != source
            })
    {
        return Err(ProtectedIssuerLaunchPreparationErrorV1::DescriptorChanged(
            "static pre-exec source table",
        ));
    }
    let decoded = StaticPreexecManifestV1::decode(&manifest.encode())
        .map_err(ProtectedIssuerLaunchPreparationErrorV1::StaticManifest)?;
    if decoded != *manifest {
        return Err(ProtectedIssuerLaunchPreparationErrorV1::DescriptorChanged(
            "canonical static pre-exec manifest",
        ));
    }
    Ok(())
}

fn create_manifest_file(
    manifest: &StaticPreexecManifestV1,
) -> Result<(File, StaticPreexecObjectIdentityV1), ProtectedIssuerLaunchPreparationErrorV1> {
    let descriptor = rustix::fs::memfd_create(
        c"fe2o3-static-preexec-manifest-v1",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .map_err(|source| io_error("create static pre-exec manifest memfd", source.into()))?;
    let writable = File::from(descriptor);
    rustix::fs::fchmod(&writable, Mode::RUSR)
        .map_err(|source| io_error("protect static pre-exec manifest mode", source.into()))?;
    writable
        .write_all_at(&manifest.encode(), 0)
        .and_then(|()| writable.sync_all())
        .map_err(|source| io_error("populate static pre-exec manifest", source))?;
    rustix::fs::fcntl_add_seals(
        &writable,
        SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK,
    )
    .and_then(|()| rustix::fs::fcntl_add_seals(&writable, SealFlags::SEAL))
    .map_err(|source| io_error("seal static pre-exec manifest", source.into()))?;
    let path = PathBuf::from(format!(
        "/proc/self/fd/{}",
        std::os::fd::AsRawFd::as_raw_fd(&writable)
    ));
    let read_only = rustix::fs::open(&path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty())
        .map(File::from)
        .map_err(|source| io_error("bind read-only static pre-exec manifest", source.into()))?;
    drop(writable);
    let identity = object_identity(&read_only, "static pre-exec manifest")?;
    validate_manifest_file(&read_only, manifest, identity)?;
    Ok((read_only, identity))
}

fn validate_manifest_file(
    file: &File,
    manifest: &StaticPreexecManifestV1,
    expected_object: StaticPreexecObjectIdentityV1,
) -> Result<(), ProtectedIssuerLaunchPreparationErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(file)
        .map_err(|source| io_error("inspect static manifest descriptor flags", source.into()))?;
    let status = rustix::fs::fcntl_getfl(file)
        .map_err(|source| io_error("inspect static manifest status flags", source.into()))?;
    let seals = rustix::fs::fcntl_get_seals(file)
        .map_err(|source| io_error("inspect static manifest seals", source.into()))?;
    let stat = rustix::fs::fstat(file)
        .map_err(|source| io_error("inspect static manifest object", source.into()))?;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || seals != REQUIRED_MANIFEST_SEALS_V1
        || FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_mode & 0o7777 != MANIFEST_MODE_V1
        || stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_gid != rustix::process::getegid().as_raw()
        || stat.st_nlink != 0
        || stat.st_size != PREEXEC_MANIFEST_BYTES_V1 as i64
        || object_identity(file, "static pre-exec manifest")? != expected_object
    {
        return Err(ProtectedIssuerLaunchPreparationErrorV1::InvalidDescriptor {
            role: "static pre-exec manifest",
            reason: "descriptor, object, access, mode, length, or seals changed",
        });
    }
    let mut bytes = [0_u8; PREEXEC_MANIFEST_BYTES_V1];
    file.read_exact_at(&mut bytes, 0)
        .map_err(|source| io_error("read static pre-exec manifest", source))?;
    let mut trailing = [0_u8; 1];
    if file
        .read_at(&mut trailing, PREEXEC_MANIFEST_BYTES_V1 as u64)
        .map_err(|source| io_error("check static pre-exec manifest boundary", source))?
        != 0
    {
        return Err(ProtectedIssuerLaunchPreparationErrorV1::DescriptorChanged(
            "static pre-exec manifest length",
        ));
    }
    let decoded = StaticPreexecManifestV1::decode(&bytes)
        .map_err(ProtectedIssuerLaunchPreparationErrorV1::StaticManifest)?;
    if &decoded != manifest || bytes != manifest.encode() {
        return Err(ProtectedIssuerLaunchPreparationErrorV1::DescriptorChanged(
            "static pre-exec manifest bytes",
        ));
    }
    manifest
        .validate_manifest_object(&expected_object)
        .map_err(ProtectedIssuerLaunchPreparationErrorV1::StaticManifest)
}

fn validate_pipe_end(
    descriptor: &impl AsFd,
    access: OFlags,
    role: &'static str,
) -> Result<(), ProtectedIssuerLaunchPreparationErrorV1> {
    let flags = rustix::io::fcntl_getfd(descriptor)
        .map_err(|source| io_error("inspect protected pipe descriptor flags", source.into()))?;
    let status = rustix::fs::fcntl_getfl(descriptor)
        .map_err(|source| io_error("inspect protected pipe status flags", source.into()))?;
    let stat = rustix::fs::fstat(descriptor)
        .map_err(|source| io_error("inspect protected pipe object", source.into()))?;
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if !flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != access
        || !status.contains(OFlags::NONBLOCK)
        || status.intersects(forbidden)
        || FileType::from_raw_mode(stat.st_mode) != FileType::Fifo
    {
        return Err(ProtectedIssuerLaunchPreparationErrorV1::InvalidDescriptor {
            role,
            reason: "pipe type, access, status, or descriptor flags changed",
        });
    }
    Ok(())
}

fn validate_pipe_pair(
    writer: &File,
    reader: &OwnedFd,
    role: &'static str,
) -> Result<(), ProtectedIssuerLaunchPreparationErrorV1> {
    validate_pipe_end(writer, OFlags::WRONLY, role)?;
    validate_pipe_end(reader, OFlags::RDONLY, role)?;
    if !same_object(
        &object_identity(writer, role)?,
        &object_identity(reader, role)?,
    ) {
        return Err(ProtectedIssuerLaunchPreparationErrorV1::DescriptorChanged(
            role,
        ));
    }
    Ok(())
}

fn validate_client_pidfd(
    pidfd: &File,
    client: fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1,
) -> Result<(), ProtectedIssuerLaunchPreparationErrorV1> {
    let duplicate = rustix::io::fcntl_dupfd_cloexec(pidfd, 0)
        .map_err(|source| io_error("duplicate launch pidfd for revalidation", source.into()))?;
    let expected = ExpectedClientProcessIdentityV1::new(client.pid(), client.uid(), client.gid())
        .map_err(ProtectedIssuerLaunchPreparationErrorV1::ClientPidfd)?;
    let live = LiveClientPidfdIdentityV1::admit(duplicate, expected)
        .map_err(ProtectedIssuerLaunchPreparationErrorV1::ClientPidfd)?;
    live.validate_liveness()
        .map_err(ProtectedIssuerLaunchPreparationErrorV1::ClientPidfd)
}

fn validate_launch_capability_source(
    source: &File,
    expected: &CompilerExecutionServiceLaunchCapabilityV1,
) -> Result<(), ProtectedIssuerLaunchPreparationErrorV1> {
    let clone = source
        .try_clone()
        .map_err(|source| io_error("clone service launch capability for validation", source))?;
    let observed = CompilerExecutionServiceLaunchCapabilityV1::from_file(clone)
        .map_err(|source| capability_error("service launch manifest", source))?;
    if observed.manifest() != expected.manifest() {
        return Err(ProtectedIssuerLaunchPreparationErrorV1::LaunchManifestMismatch);
    }
    Ok(())
}

fn revalidate_parent(
    manifest: &StaticPreexecManifestV1,
) -> Result<(), ProtectedIssuerLaunchPreparationErrorV1> {
    let pid = i32::try_from(std::process::id())
        .map_err(|_| ProtectedIssuerLaunchPreparationErrorV1::InvalidParentIdentity)?;
    let start_time = current_process_start_time_ticks_v1()
        .map_err(ProtectedIssuerLaunchPreparationErrorV1::ParentIdentity)?;
    if manifest.parent_pid() != pid || manifest.parent_start_time() != start_time {
        return Err(ProtectedIssuerLaunchPreparationErrorV1::ParentChanged);
    }
    Ok(())
}

fn require_launcher_non_aliasing(
    launcher: &File,
    issuer: &File,
    manifest: &File,
    sources: &[File; SOURCE_COUNT_V1],
) -> Result<(), ProtectedIssuerLaunchPreparationErrorV1> {
    let launcher = object_identity(launcher, "static launcher")?;
    if same_object(&launcher, &object_identity(issuer, "compiler issuer")?)
        || same_object(
            &launcher,
            &object_identity(manifest, "static pre-exec manifest")?,
        )
        || sources
            .iter()
            .enumerate()
            .map(|(index, source)| object_identity(source, source_role(index)))
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|source| same_object(&launcher, source))
    {
        return Err(ProtectedIssuerLaunchPreparationErrorV1::DescriptorAlias(
            "static launcher aliases another launch role",
        ));
    }
    Ok(())
}

const fn same_object(
    left: &StaticPreexecObjectIdentityV1,
    right: &StaticPreexecObjectIdentityV1,
) -> bool {
    left.device() == right.device() && left.inode() == right.inode()
}

fn capability_error(role: &'static str, source: String) -> ProtectedIssuerLaunchPreparationErrorV1 {
    ProtectedIssuerLaunchPreparationErrorV1::Capability { role, source }
}

fn io_error(operation: &'static str, source: io::Error) -> ProtectedIssuerLaunchPreparationErrorV1 {
    ProtectedIssuerLaunchPreparationErrorV1::Io { operation, source }
}

/// Stable failure preparing or revalidating one exact protected issuer launch.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedIssuerLaunchPreparationErrorV1 {
    /// Bound supervisor authority changed or failed revalidation.
    Supervisor(ProtectedIssuerSupervisorErrorV1),
    /// The authenticated rustc handoff changed or its client exited.
    Handoff(ProtectedIssuerHandoffErrorV1),
    /// The canonical static pre-exec manifest is invalid.
    StaticManifest(StaticPreexecManifestErrorV1),
    /// Current supervisor process identity could not be observed.
    ParentIdentity(ProtectedServiceAdmissionErrorV1),
    /// The supervisor PID cannot be represented by the launch ABI.
    InvalidParentIdentity,
    /// The retained supervisor PID or process start time changed.
    ParentChanged,
    /// The launch-manifest capability disagrees with the authenticated handoff.
    LaunchManifestMismatch,
    /// A cloned rustc pidfd is invalid, changed, or no longer live.
    ClientPidfd(ProtectedServiceAdmissionErrorV1),
    /// A sealed capability could not be created or revalidated.
    Capability {
        /// Capability role.
        role: &'static str,
        /// Exact capability validation failure.
        source: String,
    },
    /// One launch descriptor has an invalid kernel-visible property.
    InvalidDescriptor {
        /// Descriptor role.
        role: &'static str,
        /// Stable failed invariant.
        reason: &'static str,
    },
    /// A retained descriptor or canonical object snapshot changed.
    DescriptorChanged(&'static str),
    /// Two roles that must be independent name the same kernel object.
    DescriptorAlias(&'static str),
    /// A bounded operating-system operation failed.
    Io {
        /// Operation that failed.
        operation: &'static str,
        /// Kernel or filesystem failure.
        source: io::Error,
    },
}

impl fmt::Display for ProtectedIssuerLaunchPreparationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Supervisor(error) => write!(formatter, "protected supervisor changed: {error}"),
            Self::Handoff(error) => {
                write!(formatter, "authenticated rustc handoff changed: {error}")
            }
            Self::StaticManifest(error) => {
                write!(formatter, "invalid static pre-exec manifest: {error}")
            }
            Self::ParentIdentity(error) => write!(
                formatter,
                "cannot bind supervisor process identity: {error}"
            ),
            Self::InvalidParentIdentity => {
                formatter.write_str("supervisor PID is outside the launch ABI")
            }
            Self::ParentChanged => {
                formatter.write_str("supervisor PID or process start time changed")
            }
            Self::LaunchManifestMismatch => {
                formatter.write_str("service launch manifest disagrees with authenticated handoff")
            }
            Self::ClientPidfd(error) => {
                write!(formatter, "launch rustc pidfd validation failed: {error}")
            }
            Self::Capability { role, source } => {
                write!(formatter, "invalid {role} capability: {source}")
            }
            Self::InvalidDescriptor { role, reason } => {
                write!(formatter, "invalid {role} descriptor: {reason}")
            }
            Self::DescriptorChanged(role) => write!(formatter, "retained {role} changed"),
            Self::DescriptorAlias(reason) => formatter.write_str(reason),
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for ProtectedIssuerLaunchPreparationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Supervisor(error) => Some(error),
            Self::Handoff(error) => Some(error),
            Self::StaticManifest(error) => Some(error),
            Self::ParentIdentity(error) | Self::ClientPidfd(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
