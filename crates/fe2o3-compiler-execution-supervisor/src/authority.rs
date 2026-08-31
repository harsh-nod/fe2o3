use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::AsFd;

use fe2o3_broker_authority_service::{
    ProtectedExternalAnchorServiceAdmissionV1, ProtectedServiceAdmissionErrorV1,
};
use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV1;
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_STATE_ROOT_MODE_V1;
pub use fe2o3_protected_service_profile::{
    PROTECTED_SERVICE_SECUREBITS_V1 as ISSUER_SERVICE_SECUREBITS_V1,
    ProtectedServiceCredentialProfileErrorV1 as IssuerServiceCredentialProfileErrorV1,
    ProtectedServiceCredentialProfileV1 as IssuerServiceCredentialProfileV1,
};
use rustix::fs::{FileType, OFlags};

use crate::{AdmittedIssuerProgramV1, IssuerProgramAdmissionErrorV1};

const PERMISSION_AND_SPECIAL_BITS: u32 = 0o7777;

pub(super) struct ExternalAnchorLaunchClonesV1<'a> {
    pub(super) peer: &'a File,
    pub(super) pidfd: &'a File,
}

/// Failure while binding or revalidating protected issuer launch custody.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedIssuerSupervisorErrorV1 {
    /// The binding process is not running as the configured dedicated service identity.
    ServiceIdentityMismatch,
    /// Authenticated launcher, issuer, or policy continuity failed.
    Program(IssuerProgramAdmissionErrorV1),
    /// Signing-key custody or policy binding failed.
    SigningKey(String),
    /// The supervisor-provisioned external-anchor endpoint failed continuity validation.
    ExternalAnchor(ProtectedServiceAdmissionErrorV1),
    /// The durable root has an invalid descriptor, type, access, owner, mode, or ACL.
    InvalidRoot(&'static str),
    /// The retained durable root identity or security metadata changed.
    RootChanged,
    /// A bounded root inspection or duplication operation failed.
    Io {
        /// Operation that failed.
        operation: &'static str,
        /// Kernel or filesystem failure.
        source: io::Error,
    },
}

impl fmt::Display for ProtectedIssuerSupervisorErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ServiceIdentityMismatch => formatter.write_str(
                "supervisor process does not match the protected issuer service UID and GID",
            ),
            Self::Program(error) => write!(formatter, "protected issuer program changed: {error}"),
            Self::SigningKey(error) => {
                write!(formatter, "protected issuer signing key changed: {error}")
            }
            Self::ExternalAnchor(error) => {
                write!(
                    formatter,
                    "protected external-anchor endpoint changed: {error}"
                )
            }
            Self::InvalidRoot(reason) => {
                write!(formatter, "invalid protected issuer root: {reason}")
            }
            Self::RootChanged => {
                formatter.write_str("protected issuer root identity or security metadata changed")
            }
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for ProtectedIssuerSupervisorErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Program(error) => Some(error),
            Self::ExternalAnchor(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::ServiceIdentityMismatch
            | Self::SigningKey(_)
            | Self::InvalidRoot(_)
            | Self::RootChanged => None,
        }
    }
}

/// Move-only authenticated program, policy, key, root, and credential custody.
///
/// This is the sole state from which the protected issuer launch operation may
/// be implemented. It intentionally exposes no descriptor, key, signing,
/// publication, loading, GPU, or process-launch API at this checkpoint.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerSupervisorV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<ProtectedIssuerSupervisorV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerSupervisorV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<ProtectedIssuerSupervisorV1>();
/// ```
pub struct ProtectedIssuerSupervisorV1 {
    program: AdmittedIssuerProgramV1,
    credentials: IssuerServiceCredentialProfileV1,
    root: ProtectedIssuerRootV1,
    signing_key: CompilerExecutionSigningKeyCapabilityV1,
    external_anchor: ProtectedExternalAnchorServiceAdmissionV1,
}

impl fmt::Debug for ProtectedIssuerSupervisorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedIssuerSupervisorV1")
            .field("authority", &"prepared-launch-custody-only")
            .field("program", &self.program)
            .field("credentials", &self.credentials)
            .field(
                "external_anchor_service",
                &self.external_anchor.service_identity(),
            )
            .field("root", &"retained-service-owned-0700-directory")
            .finish_non_exhaustive()
    }
}

impl ProtectedIssuerSupervisorV1 {
    /// Binds every pre-session authority input before a rustc client is accepted.
    pub fn bind(
        program: AdmittedIssuerProgramV1,
        credentials: IssuerServiceCredentialProfileV1,
        root: File,
        signing_key: CompilerExecutionSigningKeyCapabilityV1,
        external_anchor: ProtectedExternalAnchorServiceAdmissionV1,
    ) -> Result<Self, ProtectedIssuerSupervisorErrorV1> {
        require_current_service_identity(credentials)?;
        program
            .revalidate()
            .map_err(ProtectedIssuerSupervisorErrorV1::Program)?;
        signing_key
            .revalidate(program.policy())
            .map_err(ProtectedIssuerSupervisorErrorV1::SigningKey)?;
        external_anchor
            .validate_continuity()
            .map_err(ProtectedIssuerSupervisorErrorV1::ExternalAnchor)?;
        let root = ProtectedIssuerRootV1::admit(root, credentials)?;
        let supervisor = Self {
            program,
            credentials,
            root,
            signing_key,
            external_anchor,
        };
        supervisor.revalidate()?;
        Ok(supervisor)
    }

    /// Revalidates the complete program, policy, key, and root custody chain.
    pub fn revalidate(&self) -> Result<(), ProtectedIssuerSupervisorErrorV1> {
        require_current_service_identity(self.credentials)?;
        self.program
            .revalidate()
            .map_err(ProtectedIssuerSupervisorErrorV1::Program)?;
        self.signing_key
            .revalidate(self.program.policy())
            .map_err(ProtectedIssuerSupervisorErrorV1::SigningKey)?;
        self.external_anchor
            .validate_continuity()
            .map_err(ProtectedIssuerSupervisorErrorV1::ExternalAnchor)?;
        self.root.revalidate(self.credentials)
    }

    /// Returns the fixed service credential profile without exposing authority.
    pub const fn credentials(&self) -> IssuerServiceCredentialProfileV1 {
        self.credentials
    }

    /// Returns the caller-pinned policy without exposing its descriptor.
    pub const fn policy(
        &self,
    ) -> &fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV1 {
        self.program.policy()
    }

    /// Returns the authenticated external-anchor service identity without exposing its endpoint.
    pub const fn external_anchor_service(
        &self,
    ) -> fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorServiceIdentityV1 {
        self.external_anchor.service_identity()
    }

    pub(super) const fn external_anchor_process(
        &self,
    ) -> fe2o3_broker_authority_service::ExpectedClientProcessIdentityV1 {
        self.external_anchor.service_process_identity()
    }

    pub(super) fn clone_launcher_for_launch(
        &self,
    ) -> Result<File, ProtectedIssuerSupervisorErrorV1> {
        self.revalidate()?;
        self.program
            .try_clone_launcher_for_launch()
            .map_err(ProtectedIssuerSupervisorErrorV1::Program)
    }

    pub(super) fn clone_issuer_for_launch(&self) -> Result<File, ProtectedIssuerSupervisorErrorV1> {
        self.revalidate()?;
        self.program
            .try_clone_issuer_for_launch()
            .map_err(ProtectedIssuerSupervisorErrorV1::Program)
    }

    pub(super) fn clone_root_for_launch(&self) -> Result<File, ProtectedIssuerSupervisorErrorV1> {
        self.revalidate()?;
        self.root.try_clone_for_launch(self.credentials)
    }

    pub(super) fn clone_policy_for_launch(&self) -> Result<File, ProtectedIssuerSupervisorErrorV1> {
        self.revalidate()?;
        self.program
            .try_clone_policy_for_launch()
            .map_err(ProtectedIssuerSupervisorErrorV1::Program)
    }

    pub(super) fn clone_signing_key_for_launch(
        &self,
    ) -> Result<File, ProtectedIssuerSupervisorErrorV1> {
        self.revalidate()?;
        self.signing_key
            .try_clone_for_transfer()
            .map_err(ProtectedIssuerSupervisorErrorV1::SigningKey)
    }

    pub(super) fn clone_external_anchor_for_launch(
        &self,
    ) -> Result<(File, File), ProtectedIssuerSupervisorErrorV1> {
        self.revalidate()?;
        let (peer, service_pidfd) = self
            .external_anchor
            .try_clone_for_transfer()
            .map_err(ProtectedIssuerSupervisorErrorV1::ExternalAnchor)?;
        Ok((File::from(peer), File::from(service_pidfd)))
    }

    pub(super) fn revalidate_launch_clones(
        &self,
        launcher: &File,
        issuer: &File,
        root: &File,
        policy: &File,
        signing_key: &File,
        external_anchor: ExternalAnchorLaunchClonesV1<'_>,
    ) -> Result<(), ProtectedIssuerSupervisorErrorV1> {
        self.revalidate()?;
        self.program
            .revalidate_launcher_clone(launcher)
            .map_err(ProtectedIssuerSupervisorErrorV1::Program)?;
        self.program
            .revalidate_issuer_clone(issuer)
            .map_err(ProtectedIssuerSupervisorErrorV1::Program)?;
        if validate_root(root, self.credentials)? != self.root.snapshot {
            return Err(ProtectedIssuerSupervisorErrorV1::RootChanged);
        }
        self.program
            .revalidate_policy_clone(policy)
            .map_err(ProtectedIssuerSupervisorErrorV1::Program)?;
        let transferred =
            signing_key
                .try_clone()
                .map_err(|source| ProtectedIssuerSupervisorErrorV1::Io {
                    operation: "clone protected issuer signing key for revalidation",
                    source,
                })?;
        let observed =
            CompilerExecutionSigningKeyCapabilityV1::from_file(transferred, self.program.policy())
                .map_err(ProtectedIssuerSupervisorErrorV1::SigningKey)?;
        if observed.verifying_key() != self.signing_key.verifying_key() {
            return Err(ProtectedIssuerSupervisorErrorV1::SigningKey(
                "protected issuer signing-key identity changed".to_owned(),
            ));
        }
        self.external_anchor
            .validate_transfer(external_anchor.peer, external_anchor.pidfd)
            .map_err(ProtectedIssuerSupervisorErrorV1::ExternalAnchor)?;
        Ok(())
    }
}

fn require_current_service_identity(
    credentials: IssuerServiceCredentialProfileV1,
) -> Result<(), ProtectedIssuerSupervisorErrorV1> {
    if rustix::process::geteuid().as_raw() != credentials.uid()
        || rustix::process::getegid().as_raw() != credentials.gid()
    {
        return Err(ProtectedIssuerSupervisorErrorV1::ServiceIdentityMismatch);
    }
    Ok(())
}

pub(super) struct ProtectedIssuerRootV1 {
    root: File,
    snapshot: RootSnapshotV1,
}

impl ProtectedIssuerRootV1 {
    pub(super) fn admit(
        root: File,
        credentials: IssuerServiceCredentialProfileV1,
    ) -> Result<Self, ProtectedIssuerSupervisorErrorV1> {
        let snapshot = validate_root(&root, credentials)?;
        let admitted = Self { root, snapshot };
        admitted.revalidate(credentials)?;
        Ok(admitted)
    }

    pub(super) fn revalidate(
        &self,
        credentials: IssuerServiceCredentialProfileV1,
    ) -> Result<(), ProtectedIssuerSupervisorErrorV1> {
        if validate_root(&self.root, credentials)? != self.snapshot {
            return Err(ProtectedIssuerSupervisorErrorV1::RootChanged);
        }
        Ok(())
    }

    pub(super) fn try_clone_for_launch(
        &self,
        credentials: IssuerServiceCredentialProfileV1,
    ) -> Result<File, ProtectedIssuerSupervisorErrorV1> {
        self.revalidate(credentials)?;
        let root =
            self.root
                .try_clone()
                .map_err(|source| ProtectedIssuerSupervisorErrorV1::Io {
                    operation: "clone protected issuer root for launch",
                    source,
                })?;
        rustix::io::fcntl_setfd(&root, rustix::io::FdFlags::CLOEXEC).map_err(|source| {
            ProtectedIssuerSupervisorErrorV1::Io {
                operation: "protect cloned issuer root descriptor",
                source: source.into(),
            }
        })?;
        if validate_root(&root, credentials)? != self.snapshot {
            return Err(ProtectedIssuerSupervisorErrorV1::RootChanged);
        }
        Ok(root)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RootSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
}

fn validate_root(
    root: &File,
    credentials: IssuerServiceCredentialProfileV1,
) -> Result<RootSnapshotV1, ProtectedIssuerSupervisorErrorV1> {
    let descriptor_flags =
        rustix::io::fcntl_getfd(root).map_err(|source| ProtectedIssuerSupervisorErrorV1::Io {
            operation: "inspect protected issuer root descriptor flags",
            source: source.into(),
        })?;
    let status =
        rustix::fs::fcntl_getfl(root).map_err(|source| ProtectedIssuerSupervisorErrorV1::Io {
            operation: "inspect protected issuer root status flags",
            source: source.into(),
        })?;
    let stat = rustix::fs::fstat(root).map_err(|source| ProtectedIssuerSupervisorErrorV1::Io {
        operation: "inspect protected issuer root",
        source: source.into(),
    })?;
    let snapshot = RootSnapshotV1 {
        device: stat.st_dev,
        inode: stat.st_ino,
        mode: stat.st_mode,
        uid: stat.st_uid,
        gid: stat.st_gid,
        links: stat.st_nlink,
    };
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(ProtectedIssuerSupervisorErrorV1::InvalidRoot(
            "descriptor is inheritable",
        ));
    }
    if status & OFlags::ACCMODE != OFlags::RDONLY || status.contains(OFlags::PATH) {
        return Err(ProtectedIssuerSupervisorErrorV1::InvalidRoot(
            "descriptor is not read-only directory custody",
        ));
    }
    if FileType::from_raw_mode(snapshot.mode) != FileType::Directory {
        return Err(ProtectedIssuerSupervisorErrorV1::InvalidRoot(
            "object is not a directory",
        ));
    }
    if snapshot.uid != credentials.uid() || snapshot.gid != credentials.gid() {
        return Err(ProtectedIssuerSupervisorErrorV1::InvalidRoot(
            "owner does not match the service UID and GID",
        ));
    }
    if snapshot.mode & PERMISSION_AND_SPECIAL_BITS
        != COMPILER_EXECUTION_SUPERVISOR_STATE_ROOT_MODE_V1
    {
        return Err(ProtectedIssuerSupervisorErrorV1::InvalidRoot(
            "mode is not exactly 0700",
        ));
    }
    if snapshot.links == 0 {
        return Err(ProtectedIssuerSupervisorErrorV1::InvalidRoot(
            "directory is unlinked",
        ));
    }
    for attribute in [
        "security.capability",
        "system.posix_acl_access",
        "system.posix_acl_default",
    ] {
        require_absent_xattr(root, attribute)?;
    }
    Ok(snapshot)
}

fn require_absent_xattr(
    root: &impl AsFd,
    attribute: &'static str,
) -> Result<(), ProtectedIssuerSupervisorErrorV1> {
    let mut byte = 0_u8;
    match rustix::fs::fgetxattr(root, attribute, std::slice::from_mut(&mut byte)) {
        Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => Ok(()),
        Ok(_) | Err(rustix::io::Errno::RANGE) => {
            Err(ProtectedIssuerSupervisorErrorV1::InvalidRoot(
                "directory has a forbidden capability or POSIX ACL",
            ))
        }
        Err(source) => Err(ProtectedIssuerSupervisorErrorV1::Io {
            operation: "inspect protected issuer root extended attributes",
            source: source.into(),
        }),
    }
}
