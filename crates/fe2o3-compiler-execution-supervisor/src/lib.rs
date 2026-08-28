#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("fe2o3-compiler-execution-supervisor requires Linux x86-64");

use std::error::Error;
use std::fmt;
use std::fs::{File, Metadata};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::PathBuf;

use fe2o3_broker_authority_service::sealed_static_issuer_runtime_measurement_v1;
use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV1;
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1;
use fe2o3_runtime_protocol::{
    SealedStaticApplicationErrorV1, sealed_static_application_identity_v1,
};
use fe2o3_static_preexec_manifest::StaticPreexecObjectIdentityV1;
use rustix::fs::{MemfdFlags, Mode, OFlags, SealFlags};
use sha2::{Digest, Sha256};

mod authority;
mod handoff;
mod launch;
#[allow(unsafe_code)]
mod process;

pub use authority::{
    ISSUER_SERVICE_SECUREBITS_V1, IssuerServiceCredentialProfileErrorV1,
    IssuerServiceCredentialProfileV1, ProtectedIssuerSupervisorErrorV1,
    ProtectedIssuerSupervisorV1,
};
pub use handoff::{AcceptedCompilerExecutionHandoffV1, ProtectedIssuerHandoffErrorV1};
pub use launch::{PreparedProtectedIssuerLaunchV1, ProtectedIssuerLaunchPreparationErrorV1};
pub use process::{
    LaunchedProtectedIssuerV1, MAX_PROTECTED_ISSUER_PROCESSES_V1, ProtectedIssuerLaunchErrorV1,
    ReadyProtectedIssuerV1,
};

const MAX_PROVISIONED_EXECUTABLE_BYTES_V1: u64 = 128 * 1024 * 1024;
const REQUIRED_EXECUTABLE_SEALS_V1: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::EXEC)
    .union(SealFlags::SEAL);
const EXECUTABLE_MODE_V1: u32 = 0o555;

/// Exact trusted-provisioning measurement for one static executable image.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProvisionedStaticExecutableMeasurementV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ProvisionedStaticExecutableMeasurementV1 {
    /// Constructs one nonzero bounded executable measurement.
    pub fn new(sha256: [u8; 32], byte_len: u64) -> Result<Self, IssuerProgramAdmissionErrorV1> {
        if sha256 == [0; 32] || byte_len == 0 || byte_len > MAX_PROVISIONED_EXECUTABLE_BYTES_V1 {
            return Err(IssuerProgramAdmissionErrorV1::InvalidMeasurement);
        }
        Ok(Self { sha256, byte_len })
    }

    fn from_issuer_policy(
        measurement: CompilerExecutionIssuerMeasurementV1,
    ) -> Result<Self, IssuerProgramAdmissionErrorV1> {
        Self::new(measurement.sha256(), measurement.byte_len())
    }

    /// Returns the complete image SHA-256.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact image length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Stable failure categories for protected issuer program admission.
#[derive(Debug)]
#[non_exhaustive]
pub enum IssuerProgramAdmissionErrorV1 {
    /// The trusted expected measurement is empty, zero, or exceeds the fixed bound.
    InvalidMeasurement,
    /// A provisioned source descriptor has the wrong type, mode, access, or descriptor flags.
    InvalidSource(&'static str),
    /// A provisioned source carries a file capability.
    SourceFileCapability(&'static str),
    /// A provisioned source changed while it was copied.
    SourceChanged(&'static str),
    /// Complete source bytes do not match trusted provisioning.
    MeasurementMismatch(&'static str),
    /// An image is outside the loader-independent static ELF profile.
    InvalidStaticImage {
        /// Image role being admitted.
        role: &'static str,
        /// Exact static-image validation failure.
        source: SealedStaticApplicationErrorV1,
    },
    /// The sealed caller policy is invalid or changed.
    Policy(String),
    /// The caller policy does not name the fixed sealed-static issuer runtime closure.
    RuntimePolicyMismatch,
    /// The retained issuer image no longer agrees with its sealed caller policy.
    PolicyImageMismatch,
    /// A newly created sealed executable has an invalid kernel-visible property.
    InvalidSealedImage(&'static str),
    /// A bounded operating-system operation failed.
    Io {
        /// Operation that failed.
        operation: &'static str,
        /// Kernel or filesystem error.
        source: io::Error,
    },
}

impl fmt::Display for IssuerProgramAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMeasurement => formatter.write_str("invalid provisioned measurement"),
            Self::InvalidSource(role) => write!(formatter, "invalid {role} source descriptor"),
            Self::SourceFileCapability(role) => {
                write!(formatter, "{role} source has a file capability")
            }
            Self::SourceChanged(role) => {
                write!(formatter, "{role} source changed during admission")
            }
            Self::MeasurementMismatch(role) => {
                write!(
                    formatter,
                    "{role} source does not match trusted provisioning"
                )
            }
            Self::InvalidStaticImage { role, source } => {
                write!(formatter, "{role} source is not sealed-static: {source}")
            }
            Self::Policy(error) => write!(formatter, "invalid compiler-execution policy: {error}"),
            Self::RuntimePolicyMismatch => {
                formatter.write_str("compiler-execution policy names a different runtime closure")
            }
            Self::PolicyImageMismatch => formatter
                .write_str("sealed compiler-execution policy and issuer image do not agree"),
            Self::InvalidSealedImage(role) => {
                write!(formatter, "new sealed {role} image has an invalid property")
            }
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for IssuerProgramAdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidStaticImage { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Move-only authenticated launcher, issuer, and caller-policy custody.
///
/// This value is produced only during trusted service provisioning. Per-launch
/// requests cannot select either executable or its expected measurement.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AdmittedIssuerProgramV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<AdmittedIssuerProgramV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_supervisor::AdmittedIssuerProgramV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<AdmittedIssuerProgramV1>();
/// ```
pub struct AdmittedIssuerProgramV1 {
    launcher: PinnedSealedStaticExecutableV1,
    issuer: PinnedSealedStaticExecutableV1,
    policy: CompilerExecutionPolicyCapabilityV1,
}

impl fmt::Debug for AdmittedIssuerProgramV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedIssuerProgramV1")
            .field("authority", &"none")
            .field("launcher", &self.launcher.measurement)
            .field("issuer", &self.issuer.measurement)
            .field("policy", self.policy.policy())
            .finish_non_exhaustive()
    }
}

impl AdmittedIssuerProgramV1 {
    /// Authenticates and seals the complete executable chain before authority binding.
    ///
    /// `launcher_expected` must come from trusted service release provisioning,
    /// never from a compiler launch request. The issuer expectation and runtime
    /// closure come from `policy`.
    pub fn provision(
        launcher_source: File,
        launcher_expected: ProvisionedStaticExecutableMeasurementV1,
        issuer_source: File,
        policy: CompilerExecutionPolicyCapabilityV1,
    ) -> Result<Self, IssuerProgramAdmissionErrorV1> {
        policy
            .revalidate()
            .map_err(IssuerProgramAdmissionErrorV1::Policy)?;
        if policy.policy().runtime() != sealed_static_issuer_runtime_measurement_v1() {
            return Err(IssuerProgramAdmissionErrorV1::RuntimePolicyMismatch);
        }
        let launcher = PinnedSealedStaticExecutableV1::admit(
            launcher_source,
            launcher_expected,
            "static launcher",
        )?;
        let issuer = PinnedSealedStaticExecutableV1::admit(
            issuer_source,
            ProvisionedStaticExecutableMeasurementV1::from_issuer_policy(
                policy.policy().executable(),
            )?,
            "compiler issuer",
        )?;
        if launcher.snapshot.same_object_key(&issuer.snapshot) {
            return Err(IssuerProgramAdmissionErrorV1::InvalidSealedImage(
                "aliased launcher and issuer",
            ));
        }
        let admitted = Self {
            launcher,
            issuer,
            policy,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Revalidates both sealed images and the exact caller policy.
    pub fn revalidate(&self) -> Result<(), IssuerProgramAdmissionErrorV1> {
        self.policy
            .revalidate()
            .map_err(IssuerProgramAdmissionErrorV1::Policy)?;
        if self.policy.policy().runtime() != sealed_static_issuer_runtime_measurement_v1() {
            return Err(IssuerProgramAdmissionErrorV1::RuntimePolicyMismatch);
        }
        if ProvisionedStaticExecutableMeasurementV1::from_issuer_policy(
            self.policy.policy().executable(),
        )? != self.issuer.measurement
        {
            return Err(IssuerProgramAdmissionErrorV1::PolicyImageMismatch);
        }
        self.launcher.revalidate()?;
        self.issuer.revalidate()?;
        if self
            .launcher
            .snapshot
            .same_object_key(&self.issuer.snapshot)
        {
            return Err(IssuerProgramAdmissionErrorV1::InvalidSealedImage(
                "aliased launcher and issuer",
            ));
        }
        Ok(())
    }

    /// Returns the authenticated launcher content measurement without exposing custody.
    pub const fn launcher_measurement(&self) -> ProvisionedStaticExecutableMeasurementV1 {
        self.launcher.measurement
    }

    /// Returns the caller-pinned issuer policy without exposing its sealed descriptor.
    pub const fn policy(
        &self,
    ) -> &fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV1 {
        self.policy.policy()
    }

    /// Returns the inert launcher object identity used by the static pre-exec manifest.
    pub fn launcher_object_identity(&self) -> StaticPreexecObjectIdentityV1 {
        self.launcher.object_identity()
    }

    /// Returns the inert issuer object identity used by the static pre-exec manifest.
    pub fn issuer_object_identity(&self) -> StaticPreexecObjectIdentityV1 {
        self.issuer.object_identity()
    }

    pub(crate) fn try_clone_launcher_for_launch(
        &self,
    ) -> Result<File, IssuerProgramAdmissionErrorV1> {
        self.launcher.try_clone_for_launch()
    }

    pub(crate) fn try_clone_issuer_for_launch(
        &self,
    ) -> Result<File, IssuerProgramAdmissionErrorV1> {
        self.issuer.try_clone_for_launch()
    }

    pub(crate) fn try_clone_policy_for_launch(
        &self,
    ) -> Result<File, IssuerProgramAdmissionErrorV1> {
        self.policy
            .try_clone_for_transfer()
            .map_err(IssuerProgramAdmissionErrorV1::Policy)
    }

    pub(crate) fn revalidate_launcher_clone(
        &self,
        image: &File,
    ) -> Result<(), IssuerProgramAdmissionErrorV1> {
        self.launcher.revalidate_clone(image)
    }

    pub(crate) fn revalidate_issuer_clone(
        &self,
        image: &File,
    ) -> Result<(), IssuerProgramAdmissionErrorV1> {
        self.issuer.revalidate_clone(image)
    }

    pub(crate) fn revalidate_policy_clone(
        &self,
        image: &File,
    ) -> Result<(), IssuerProgramAdmissionErrorV1> {
        let transferred =
            image
                .try_clone()
                .map_err(|source| IssuerProgramAdmissionErrorV1::Io {
                    operation: "clone protected issuer policy for revalidation",
                    source,
                })?;
        let observed = CompilerExecutionPolicyCapabilityV1::from_file(transferred)
            .map_err(IssuerProgramAdmissionErrorV1::Policy)?;
        if observed.policy() != self.policy.policy() {
            return Err(IssuerProgramAdmissionErrorV1::PolicyImageMismatch);
        }
        Ok(())
    }
}

struct PinnedSealedStaticExecutableV1 {
    image: File,
    snapshot: FileSnapshotV1,
    measurement: ProvisionedStaticExecutableMeasurementV1,
    sealed_static_identity: [u8; 32],
    role: &'static str,
}

impl PinnedSealedStaticExecutableV1 {
    fn admit(
        source: File,
        expected: ProvisionedStaticExecutableMeasurementV1,
        role: &'static str,
    ) -> Result<Self, IssuerProgramAdmissionErrorV1> {
        let before = validate_source(&source, expected, role)?;
        let bytes = read_exact_source(&source, expected, role)?;
        let after = snapshot(&source, "inspect provisioned executable after read")?;
        if before != after {
            return Err(IssuerProgramAdmissionErrorV1::SourceChanged(role));
        }
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        if digest != expected.sha256 {
            return Err(IssuerProgramAdmissionErrorV1::MeasurementMismatch(role));
        }
        let sealed_static_identity = sealed_static_application_identity_v1(&bytes)
            .map_err(|source| IssuerProgramAdmissionErrorV1::InvalidStaticImage { role, source })?;
        let image = create_sealed_executable(&bytes, role)?;
        let snapshot = validate_sealed_executable(&image, expected, role)?;
        let admitted = Self {
            image,
            snapshot,
            measurement: expected,
            sealed_static_identity,
            role,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    fn revalidate(&self) -> Result<(), IssuerProgramAdmissionErrorV1> {
        let current = validate_sealed_executable(&self.image, self.measurement, self.role)?;
        if current != self.snapshot {
            return Err(IssuerProgramAdmissionErrorV1::InvalidSealedImage(self.role));
        }
        let bytes = read_exact_source(&self.image, self.measurement, self.role)?;
        if <[u8; 32]>::from(Sha256::digest(&bytes)) != self.measurement.sha256
            || sealed_static_application_identity_v1(&bytes).map_err(|source| {
                IssuerProgramAdmissionErrorV1::InvalidStaticImage {
                    role: self.role,
                    source,
                }
            })? != self.sealed_static_identity
        {
            return Err(IssuerProgramAdmissionErrorV1::InvalidSealedImage(self.role));
        }
        Ok(())
    }

    fn object_identity(&self) -> StaticPreexecObjectIdentityV1 {
        StaticPreexecObjectIdentityV1::new(
            self.snapshot.device,
            self.snapshot.inode,
            self.snapshot.size,
            self.snapshot.mode,
        )
    }

    fn try_clone_for_launch(&self) -> Result<File, IssuerProgramAdmissionErrorV1> {
        self.revalidate()?;
        let image = self
            .image
            .try_clone()
            .map_err(|source| IssuerProgramAdmissionErrorV1::Io {
                operation: "clone sealed executable for protected launch",
                source,
            })?;
        rustix::io::fcntl_setfd(&image, rustix::io::FdFlags::CLOEXEC).map_err(|source| {
            IssuerProgramAdmissionErrorV1::Io {
                operation: "protect cloned executable launch descriptor",
                source: source.into(),
            }
        })?;
        self.revalidate_clone(&image)?;
        Ok(image)
    }

    fn revalidate_clone(&self, image: &File) -> Result<(), IssuerProgramAdmissionErrorV1> {
        let current = validate_sealed_executable(image, self.measurement, self.role)?;
        if current != self.snapshot {
            return Err(IssuerProgramAdmissionErrorV1::InvalidSealedImage(self.role));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileSnapshotV1 {
    device: u64,
    inode: u64,
    size: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl FileSnapshotV1 {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            links: metadata.nlink(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }

    const fn same_object_key(&self, other: &Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }
}

fn snapshot(
    file: &File,
    operation: &'static str,
) -> Result<FileSnapshotV1, IssuerProgramAdmissionErrorV1> {
    file.metadata()
        .map(|metadata| FileSnapshotV1::from_metadata(&metadata))
        .map_err(|source| IssuerProgramAdmissionErrorV1::Io { operation, source })
}

fn validate_source(
    source: &File,
    expected: ProvisionedStaticExecutableMeasurementV1,
    role: &'static str,
) -> Result<FileSnapshotV1, IssuerProgramAdmissionErrorV1> {
    let descriptor_flags =
        rustix::io::fcntl_getfd(source).map_err(|source| IssuerProgramAdmissionErrorV1::Io {
            operation: "inspect provisioned executable descriptor flags",
            source: source.into(),
        })?;
    let status =
        rustix::fs::fcntl_getfl(source).map_err(|source| IssuerProgramAdmissionErrorV1::Io {
            operation: "inspect provisioned executable status flags",
            source: source.into(),
        })?;
    let observed = snapshot(source, "inspect provisioned executable")?;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || observed.mode & libc::S_IFMT != libc::S_IFREG
        || observed.mode & 0o111 == 0
        || observed.mode & (libc::S_ISUID | libc::S_ISGID) != 0
        || observed.size != expected.byte_len
    {
        return Err(IssuerProgramAdmissionErrorV1::InvalidSource(role));
    }
    require_no_file_capability(source, role)?;
    Ok(observed)
}

fn require_no_file_capability(
    file: &File,
    role: &'static str,
) -> Result<(), IssuerProgramAdmissionErrorV1> {
    let mut value = 0_u8;
    match rustix::fs::fgetxattr(
        file,
        "security.capability",
        std::slice::from_mut(&mut value),
    ) {
        Ok(_) | Err(rustix::io::Errno::RANGE) => {
            Err(IssuerProgramAdmissionErrorV1::SourceFileCapability(role))
        }
        Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => Ok(()),
        Err(source) => Err(IssuerProgramAdmissionErrorV1::Io {
            operation: "inspect provisioned executable file capability",
            source: source.into(),
        }),
    }
}

fn read_exact_source(
    source: &File,
    expected: ProvisionedStaticExecutableMeasurementV1,
    role: &'static str,
) -> Result<Vec<u8>, IssuerProgramAdmissionErrorV1> {
    let length = usize::try_from(expected.byte_len)
        .map_err(|_| IssuerProgramAdmissionErrorV1::InvalidMeasurement)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| IssuerProgramAdmissionErrorV1::InvalidMeasurement)?;
    bytes.resize(length, 0);
    source
        .read_exact_at(&mut bytes, 0)
        .map_err(|source| IssuerProgramAdmissionErrorV1::Io {
            operation: "read exact provisioned executable",
            source,
        })?;
    let mut trailing = [0_u8; 1];
    if source
        .read_at(&mut trailing, expected.byte_len)
        .map_err(|source| IssuerProgramAdmissionErrorV1::Io {
            operation: "check provisioned executable boundary",
            source,
        })?
        != 0
    {
        return Err(IssuerProgramAdmissionErrorV1::SourceChanged(role));
    }
    Ok(bytes)
}

fn create_sealed_executable(
    bytes: &[u8],
    role: &'static str,
) -> Result<File, IssuerProgramAdmissionErrorV1> {
    let descriptor = rustix::fs::memfd_create(
        c"fe2o3-protected-static-executable",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING | MemfdFlags::EXEC,
    )
    .map_err(|source| IssuerProgramAdmissionErrorV1::Io {
        operation: "create protected executable memfd",
        source: source.into(),
    })?;
    let mut writable = File::from(descriptor);
    writable
        .write_all(bytes)
        .map_err(|source| IssuerProgramAdmissionErrorV1::Io {
            operation: "populate protected executable memfd",
            source,
        })?;
    rustix::fs::fchmod(
        &writable,
        Mode::RUSR | Mode::RGRP | Mode::ROTH | Mode::XUSR | Mode::XGRP | Mode::XOTH,
    )
    .map_err(|source| IssuerProgramAdmissionErrorV1::Io {
        operation: "set protected executable mode",
        source: source.into(),
    })?;
    let content_and_exec = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::EXEC;
    rustix::fs::fcntl_add_seals(&writable, content_and_exec)
        .and_then(|()| rustix::fs::fcntl_add_seals(&writable, SealFlags::SEAL))
        .map_err(|source| IssuerProgramAdmissionErrorV1::Io {
            operation: "seal protected executable memfd",
            source: source.into(),
        })?;
    let path = PathBuf::from(format!("/proc/self/fd/{}", writable.as_raw_fd()));
    let read_only = rustix::fs::open(&path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty())
        .map_err(|source| IssuerProgramAdmissionErrorV1::Io {
            operation: "bind read-only protected executable descriptor",
            source: source.into(),
        })?;
    let image = File::from(read_only);
    drop(writable);
    let byte_len = u64::try_from(bytes.len())
        .map_err(|_| IssuerProgramAdmissionErrorV1::InvalidMeasurement)?;
    validate_sealed_executable(
        &image,
        ProvisionedStaticExecutableMeasurementV1::new(Sha256::digest(bytes).into(), byte_len)?,
        role,
    )?;
    Ok(image)
}

fn validate_sealed_executable(
    image: &File,
    expected: ProvisionedStaticExecutableMeasurementV1,
    role: &'static str,
) -> Result<FileSnapshotV1, IssuerProgramAdmissionErrorV1> {
    let descriptor_flags =
        rustix::io::fcntl_getfd(image).map_err(|source| IssuerProgramAdmissionErrorV1::Io {
            operation: "inspect sealed executable descriptor flags",
            source: source.into(),
        })?;
    let status =
        rustix::fs::fcntl_getfl(image).map_err(|source| IssuerProgramAdmissionErrorV1::Io {
            operation: "inspect sealed executable status flags",
            source: source.into(),
        })?;
    let seals =
        rustix::fs::fcntl_get_seals(image).map_err(|source| IssuerProgramAdmissionErrorV1::Io {
            operation: "inspect sealed executable seals",
            source: source.into(),
        })?;
    let observed = snapshot(image, "inspect sealed executable object")?;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || !seals.contains(REQUIRED_EXECUTABLE_SEALS_V1)
        || observed.mode & libc::S_IFMT != libc::S_IFREG
        || observed.mode & 0o7777 != EXECUTABLE_MODE_V1
        || observed.links != 0
        || observed.size != expected.byte_len
    {
        return Err(IssuerProgramAdmissionErrorV1::InvalidSealedImage(role));
    }
    let bytes = read_exact_source(image, expected, role)?;
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != expected.sha256 {
        return Err(IssuerProgramAdmissionErrorV1::InvalidSealedImage(role));
    }
    sealed_static_application_identity_v1(&bytes)
        .map_err(|source| IssuerProgramAdmissionErrorV1::InvalidStaticImage { role, source })?;
    Ok(observed)
}

#[cfg(test)]
mod tests;
