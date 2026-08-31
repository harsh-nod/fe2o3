//! Protected process, executable, and key admission for compiler-execution attestation.
//!
//! This module deliberately exposes no signing operation. The admitted value becomes usable only
//! after the durable rollback state machine and supervised compiler-occurrence token consume it.

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::FileExt;

use ed25519_dalek::SigningKey;
use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV1;
use fe2o3_runtime_protocol::{
    CompilerExecutionAttestationErrorV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyV1, SealedStaticApplicationErrorV1,
    sealed_static_application_identity_v1,
};
pub use fe2o3_runtime_protocol::{
    SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1, sealed_static_issuer_runtime_measurement_v1,
};
use rustix::fs::FileType;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::{
    ProtectedCompilerExecutionExternalAnchorV1, ProtectedServiceAdmissionErrorV1,
    ProtectedServiceAdmissionV1,
};

/// Maximum admitted byte length of the protected issuer executable.
pub const MAX_COMPILER_EXECUTION_ISSUER_IMAGE_BYTES_V1: u64 = 256 * 1024 * 1024;

/// This admission establishes only the protected issuer process, executable, and key boundary.
pub const PROTECTED_COMPILER_EXECUTION_ISSUER_AUTHORITY_V1: &str = "admission-only";

const KEY_BYTES: usize = 32;

/// Independently measured current static issuer executable and fixed empty runtime closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentStaticIssuerMeasurementsV1 {
    executable: CompilerExecutionIssuerMeasurementV1,
    runtime: CompilerExecutionIssuerMeasurementV1,
    sealed_static_identity: [u8; 32],
}

impl CurrentStaticIssuerMeasurementsV1 {
    /// Returns SHA-256 and byte length of the exact running executable image.
    pub const fn executable(self) -> CompilerExecutionIssuerMeasurementV1 {
        self.executable
    }

    /// Returns the canonical loader-independent runtime-closure measurement.
    pub const fn runtime(self) -> CompilerExecutionIssuerMeasurementV1 {
        self.runtime
    }

    /// Returns the domain-separated identity of the validated sealed-static ELF image.
    pub const fn sealed_static_identity(self) -> [u8; 32] {
        self.sealed_static_identity
    }
}

/// Irreversible, move-only evidence that the current service process was hardened for key custody.
///
/// ```compile_fail
/// fn duplicate(value: fe2o3_broker_authority_service::ProtectedIssuerProcessV1) {
///     let moved = value;
///     drop(value);
///     drop(moved);
/// }
/// ```
pub struct ProtectedIssuerProcessV1 {
    _private: (),
}

impl fmt::Debug for ProtectedIssuerProcessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedIssuerProcessV1")
            .field("authority", &"process-hardening-only")
            .finish_non_exhaustive()
    }
}

impl ProtectedIssuerProcessV1 {
    /// Disables core dumps, makes the process nondumpable, and permanently enables
    /// `no_new_privs`. The hard core limit and `no_new_privs` transition are irreversible.
    pub fn harden() -> Result<Self, ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
        let zero = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: `zero` is a valid immutable rlimit and the call does not retain its pointer.
        if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &raw const zero) } != 0 {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::io(
                IssuerAdmissionErrorKindV1::ProcessHardening,
                "cannot disable issuer core dumps",
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: these documented scalar prctl operations use no pointer arguments.
        if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0 {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::io(
                IssuerAdmissionErrorKindV1::ProcessHardening,
                "cannot make issuer process nondumpable",
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: `PR_SET_NO_NEW_PRIVS` accepts the scalar value one and zero trailing arguments.
        if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::io(
                IssuerAdmissionErrorKindV1::ProcessHardening,
                "cannot enable issuer no_new_privs",
                io::Error::last_os_error(),
            ));
        }
        let process = Self { _private: () };
        process.validate()?;
        Ok(process)
    }

    fn validate(&self) -> Result<(), ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
        let mut core = std::mem::MaybeUninit::<libc::rlimit>::uninit();
        // SAFETY: the pointer references writable storage for one rlimit result.
        if unsafe { libc::getrlimit(libc::RLIMIT_CORE, core.as_mut_ptr()) } != 0 {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::io(
                IssuerAdmissionErrorKindV1::ProcessHardeningChanged,
                "cannot revalidate issuer core-dump limits",
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: getrlimit initialized `core` after its successful return.
        let core = unsafe { core.assume_init() };
        // SAFETY: these documented scalar query operations use no pointer arguments.
        let dumpable = unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) };
        // SAFETY: these documented scalar query operations use no pointer arguments.
        let no_new_privs = unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) };
        if dumpable < 0 || no_new_privs < 0 {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::io(
                IssuerAdmissionErrorKindV1::ProcessHardeningChanged,
                "cannot revalidate issuer process security state",
                io::Error::last_os_error(),
            ));
        }
        if core.rlim_cur != 0 || core.rlim_max != 0 || dumpable != 0 || no_new_privs != 1 {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                IssuerAdmissionErrorKindV1::ProcessHardeningChanged,
                "issuer process security state changed after hardening",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    uid: u32,
    gid: u32,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: u64,
    changed_seconds: i64,
    changed_nanoseconds: u64,
}

impl FileSnapshotV1 {
    fn inspect(
        file: &impl AsFd,
        kind: IssuerAdmissionErrorKindV1,
        label: &'static str,
    ) -> Result<Self, ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
        let stat = rustix::fs::fstat(file).map_err(|error| {
            ProtectedCompilerExecutionIssuerAdmissionErrorV1::io(
                kind,
                format!("cannot inspect {label}"),
                io::Error::from(error),
            )
        })?;
        Ok(Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            links: stat.st_nlink,
            uid: stat.st_uid,
            gid: stat.st_gid,
            size: stat.st_size.try_into().map_err(|_| {
                ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                    kind,
                    format!("{label} has a negative size"),
                )
            })?,
            modified_seconds: stat.st_mtime,
            modified_nanoseconds: stat.st_mtime_nsec,
            changed_seconds: stat.st_ctime,
            changed_nanoseconds: stat.st_ctime_nsec,
        })
    }

    fn file_type(self) -> FileType {
        FileType::from_raw_mode(self.mode)
    }
}

struct RetainedStaticIssuerExecutableV1 {
    image: File,
    snapshot: FileSnapshotV1,
    measurements: CurrentStaticIssuerMeasurementsV1,
}

impl RetainedStaticIssuerExecutableV1 {
    fn observe() -> Result<Self, ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
        let image = File::open("/proc/self/exe").map_err(|error| {
            ProtectedCompilerExecutionIssuerAdmissionErrorV1::io(
                IssuerAdmissionErrorKindV1::ExecutableInspect,
                "cannot open the running issuer executable",
                error,
            )
        })?;
        require_close_on_exec(
            &image,
            IssuerAdmissionErrorKindV1::ExecutableCloseOnExec,
            "issuer executable",
        )?;
        let snapshot = validate_executable_snapshot(&image)?;
        let measurements = measure_static_executable(&image, snapshot)?;
        let retained = Self {
            image,
            snapshot,
            measurements,
        };
        retained.validate()?;
        Ok(retained)
    }

    fn validate(&self) -> Result<(), ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
        require_close_on_exec(
            &self.image,
            IssuerAdmissionErrorKindV1::ExecutableCloseOnExec,
            "issuer executable",
        )?;
        let snapshot = validate_executable_snapshot(&self.image)?;
        if snapshot != self.snapshot {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                IssuerAdmissionErrorKindV1::ExecutableChanged,
                "retained issuer executable identity or metadata changed",
            ));
        }
        if measure_static_executable(&self.image, snapshot)? != self.measurements {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                IssuerAdmissionErrorKindV1::ExecutableChanged,
                "retained issuer executable bytes or static identity changed",
            ));
        }
        Ok(())
    }
}

/// Measures and validates the exact `/proc/self/exe` image without trusting caller input.
pub fn current_static_issuer_measurements_v1()
-> Result<CurrentStaticIssuerMeasurementsV1, ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
    Ok(RetainedStaticIssuerExecutableV1::observe()?.measurements)
}

fn validate_executable_snapshot(
    image: &File,
) -> Result<FileSnapshotV1, ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
    let snapshot = FileSnapshotV1::inspect(
        image,
        IssuerAdmissionErrorKindV1::ExecutableInspect,
        "running issuer executable",
    )?;
    if snapshot.file_type() != FileType::RegularFile || snapshot.mode & 0o111 == 0 {
        return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
            IssuerAdmissionErrorKindV1::ExecutableShape,
            "running issuer executable is not an executable regular file",
        ));
    }
    if snapshot.size == 0 || snapshot.size > MAX_COMPILER_EXECUTION_ISSUER_IMAGE_BYTES_V1 {
        return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
            IssuerAdmissionErrorKindV1::ExecutableSize,
            "running issuer executable has an invalid bounded size",
        ));
    }
    Ok(snapshot)
}

fn measure_static_executable(
    image: &File,
    before: FileSnapshotV1,
) -> Result<CurrentStaticIssuerMeasurementsV1, ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
    let length = usize::try_from(before.size).map_err(|_| {
        ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
            IssuerAdmissionErrorKindV1::ExecutableSize,
            "running issuer executable does not fit in addressable memory",
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| {
        ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
            IssuerAdmissionErrorKindV1::ExecutableSize,
            "cannot reserve the bounded issuer executable image",
        )
    })?;
    bytes.resize(length, 0);
    image.read_exact_at(&mut bytes, 0).map_err(|error| {
        ProtectedCompilerExecutionIssuerAdmissionErrorV1::io(
            IssuerAdmissionErrorKindV1::ExecutableRead,
            "cannot read the exact issuer executable image",
            error,
        )
    })?;
    let mut trailing = [0_u8; 1];
    if image.read_at(&mut trailing, before.size).map_err(|error| {
        ProtectedCompilerExecutionIssuerAdmissionErrorV1::io(
            IssuerAdmissionErrorKindV1::ExecutableRead,
            "cannot check the issuer executable boundary",
            error,
        )
    })? != 0
    {
        return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
            IssuerAdmissionErrorKindV1::ExecutableChanged,
            "issuer executable grew while it was measured",
        ));
    }
    let after = FileSnapshotV1::inspect(
        image,
        IssuerAdmissionErrorKindV1::ExecutableInspect,
        "running issuer executable",
    )?;
    if after != before {
        return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
            IssuerAdmissionErrorKindV1::ExecutableChanged,
            "issuer executable identity or metadata changed while it was measured",
        ));
    }
    let sealed_static_identity =
        sealed_static_application_identity_v1(&bytes).map_err(|error| {
            ProtectedCompilerExecutionIssuerAdmissionErrorV1::static_image(
                "running issuer executable is not loader independent",
                error,
            )
        })?;
    let executable =
        CompilerExecutionIssuerMeasurementV1::new(Sha256::digest(&bytes).into(), before.size)
            .map_err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::Protocol)?;
    bytes.zeroize();
    Ok(CurrentStaticIssuerMeasurementsV1 {
        executable,
        runtime: sealed_static_issuer_runtime_measurement_v1(),
        sealed_static_identity,
    })
}

struct ProtectedIssuerSigningKeyV1 {
    key: SigningKey,
    capability: CompilerExecutionSigningKeyCapabilityV1,
}

impl fmt::Debug for ProtectedIssuerSigningKeyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedIssuerSigningKeyV1")
            .field("authority", &"key-custody-only")
            .field("capability", &self.capability)
            .finish_non_exhaustive()
    }
}

impl ProtectedIssuerSigningKeyV1 {
    fn admit(
        capability: CompilerExecutionSigningKeyCapabilityV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
        capability.revalidate(policy).map_err(|error| {
            ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                IssuerAdmissionErrorKindV1::KeyCapability,
                format!("issuer signing-key capability is invalid: {error}"),
            )
        })?;
        let key = read_capability_signing_key(&capability)?;
        if key.verifying_key().as_bytes() != policy.verifying_key() {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                IssuerAdmissionErrorKindV1::SigningKeyMismatch,
                "issuer signing key does not match the caller-pinned policy",
            ));
        }
        let admitted = Self { key, capability };
        admitted.validate(policy)?;
        Ok(admitted)
    }

    fn validate(
        &self,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<(), ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
        self.capability.revalidate(policy).map_err(|error| {
            ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                IssuerAdmissionErrorKindV1::KeyChanged,
                format!("issuer signing-key capability changed: {error}"),
            )
        })?;
        let current = read_capability_signing_key(&self.capability)?;
        let matches = self.key.as_bytes() == current.as_bytes()
            && self.key.verifying_key().as_bytes() == policy.verifying_key();
        if !matches {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                IssuerAdmissionErrorKindV1::KeyChanged,
                "issuer signing-key bytes or policy binding changed",
            ));
        }
        Ok(())
    }
}

/// Move-only admitted process, static executable, policy, service channels, and signing key.
///
/// This value intentionally has no signing API and grants no compiler, publication, load, or
/// launch authority. The durable issuer state machine will consume it.
///
/// ```compile_fail
/// fn require_clone<T: Clone>() {}
/// require_clone::<
///     fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionV1,
/// >();
/// ```
pub struct ProtectedCompilerExecutionIssuerAdmissionV1 {
    process: ProtectedIssuerProcessV1,
    service: ProtectedServiceAdmissionV1,
    policy: CompilerExecutionIssuerPolicyV1,
    executable: RetainedStaticIssuerExecutableV1,
    signing_key: ProtectedIssuerSigningKeyV1,
    external_anchor: ProtectedCompilerExecutionExternalAnchorV1,
}

impl fmt::Debug for ProtectedCompilerExecutionIssuerAdmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedCompilerExecutionIssuerAdmissionV1")
            .field(
                "authority",
                &PROTECTED_COMPILER_EXECUTION_ISSUER_AUTHORITY_V1,
            )
            .field("policy", &self.policy)
            .field("measurements", &self.executable.measurements)
            .finish_non_exhaustive()
    }
}

impl ProtectedCompilerExecutionIssuerAdmissionV1 {
    /// Admits one exact protected service occurrence under a caller-pinned issuer policy.
    pub fn admit(
        process: ProtectedIssuerProcessV1,
        service: ProtectedServiceAdmissionV1,
        policy: CompilerExecutionIssuerPolicyV1,
        signing_key: CompilerExecutionSigningKeyCapabilityV1,
        external_anchor: ProtectedCompilerExecutionExternalAnchorV1,
    ) -> Result<Self, ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
        process.validate()?;
        service
            .validate_continuity()
            .map_err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::ServiceAdmission)?;
        external_anchor
            .validate_continuity()
            .map_err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::ServiceAdmission)?;
        if !external_anchor.matches_policy(&policy) {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                IssuerAdmissionErrorKindV1::ExternalAnchorKeyMismatch,
                "external-anchor transport does not use the caller-pinned policy key",
            ));
        }
        let executable = RetainedStaticIssuerExecutableV1::observe()?;
        let measurements = executable.measurements;
        if measurements.executable != policy.executable() {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                IssuerAdmissionErrorKindV1::ExecutablePolicyMismatch,
                "running issuer executable does not match the caller-pinned policy",
            ));
        }
        if measurements.runtime != policy.runtime() {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                IssuerAdmissionErrorKindV1::RuntimePolicyMismatch,
                "issuer runtime closure does not match the caller-pinned policy",
            ));
        }
        let signing_key = ProtectedIssuerSigningKeyV1::admit(signing_key, &policy)?;
        process.validate()?;
        service
            .validate_continuity()
            .map_err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::ServiceAdmission)?;
        external_anchor
            .validate_continuity()
            .map_err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::ServiceAdmission)?;
        let admitted = Self {
            process,
            service,
            policy,
            executable,
            signing_key,
            external_anchor,
        };
        admitted.validate_continuity()?;
        Ok(admitted)
    }

    /// Revalidates every retained process, descriptor, key, and policy axis.
    pub fn validate_continuity(
        &self,
    ) -> Result<(), ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
        self.process.validate()?;
        self.executable.validate()?;
        self.service
            .validate_continuity()
            .map_err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::ServiceAdmission)?;
        self.external_anchor
            .validate_continuity()
            .map_err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::ServiceAdmission)?;
        self.signing_key.validate(&self.policy)?;
        if self.executable.measurements.executable != self.policy.executable()
            || self.executable.measurements.runtime != self.policy.runtime()
        {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                IssuerAdmissionErrorKindV1::PolicyChanged,
                "retained issuer measurements no longer match the pinned policy",
            ));
        }
        if !self.external_anchor.matches_policy(&self.policy) {
            return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
                IssuerAdmissionErrorKindV1::ExternalAnchorKeyMismatch,
                "retained external-anchor key no longer matches the pinned policy",
            ));
        }
        Ok(())
    }

    /// Returns the exact caller-pinned policy.
    pub const fn policy(&self) -> &CompilerExecutionIssuerPolicyV1 {
        &self.policy
    }

    /// Reports that admission alone does not authenticate a compiler occurrence.
    pub const fn authenticates_protected_compiler_execution(&self) -> bool {
        false
    }

    /// Reports that admission alone grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub(crate) fn try_clone_service_root(
        &self,
    ) -> Result<OwnedFd, ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
        self.validate_continuity()?;
        self.service
            .try_clone_service_root()
            .map_err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::ServiceAdmission)
    }

    pub(crate) const fn service_admission(&self) -> &ProtectedServiceAdmissionV1 {
        &self.service
    }

    pub(crate) fn service_peer(&self) -> std::os::fd::BorrowedFd<'_> {
        self.service.service_peer()
    }

    pub(crate) fn client_pidfd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.service.client_pidfd()
    }

    pub(crate) const fn external_anchor_mut(
        &mut self,
    ) -> &mut ProtectedCompilerExecutionExternalAnchorV1 {
        &mut self.external_anchor
    }

    pub(crate) const fn signing_key(&self) -> &SigningKey {
        &self.signing_key.key
    }
}

fn read_capability_signing_key(
    capability: &CompilerExecutionSigningKeyCapabilityV1,
) -> Result<SigningKey, ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
    let image = capability.try_clone_for_transfer().map_err(|error| {
        ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
            IssuerAdmissionErrorKindV1::KeyCapability,
            format!("cannot retain issuer signing-key capability: {error}"),
        )
    })?;
    let mut bytes = [0_u8; KEY_BYTES];
    if let Err(error) = image.read_exact_at(&mut bytes, 0) {
        bytes.zeroize();
        return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::io(
            IssuerAdmissionErrorKindV1::KeyRead,
            "cannot read the exact issuer signing key",
            error,
        ));
    }
    let key = SigningKey::from_bytes(&bytes);
    bytes.zeroize();
    Ok(key)
}

fn require_close_on_exec(
    descriptor: &impl AsFd,
    kind: IssuerAdmissionErrorKindV1,
    label: &'static str,
) -> Result<(), ProtectedCompilerExecutionIssuerAdmissionErrorV1> {
    let flags = rustix::io::fcntl_getfd(descriptor).map_err(|error| {
        ProtectedCompilerExecutionIssuerAdmissionErrorV1::io(
            kind,
            format!("cannot inspect {label} descriptor flags"),
            io::Error::from(error),
        )
    })?;
    if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(ProtectedCompilerExecutionIssuerAdmissionErrorV1::new(
            kind,
            format!("{label} descriptor lacks FD_CLOEXEC"),
        ));
    }
    Ok(())
}

/// Stable category for a protected issuer admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum IssuerAdmissionErrorKindV1 {
    ProcessHardening,
    ProcessHardeningChanged,
    ExecutableCloseOnExec,
    ExecutableInspect,
    ExecutableShape,
    ExecutableSize,
    ExecutableRead,
    ExecutableChanged,
    ExecutableNotStatic,
    ExecutablePolicyMismatch,
    RuntimePolicyMismatch,
    KeyCapability,
    KeyRead,
    KeyChanged,
    SigningKeyMismatch,
    PolicyChanged,
    ServiceAdmission,
    ExternalAnchorKeyMismatch,
    Protocol,
}

/// Failure while admitting or revalidating the protected compiler-execution issuer.
#[derive(Debug)]
pub enum ProtectedCompilerExecutionIssuerAdmissionErrorV1 {
    Failure {
        kind: IssuerAdmissionErrorKindV1,
        message: String,
        source: Option<io::Error>,
    },
    StaticImage {
        message: &'static str,
        source: SealedStaticApplicationErrorV1,
    },
    ServiceAdmission(ProtectedServiceAdmissionErrorV1),
    Protocol(CompilerExecutionAttestationErrorV1),
}

impl ProtectedCompilerExecutionIssuerAdmissionErrorV1 {
    fn new(kind: IssuerAdmissionErrorKindV1, message: impl Into<String>) -> Self {
        Self::Failure {
            kind,
            message: message.into(),
            source: None,
        }
    }

    fn io(kind: IssuerAdmissionErrorKindV1, message: impl Into<String>, source: io::Error) -> Self {
        Self::Failure {
            kind,
            message: message.into(),
            source: Some(source),
        }
    }

    fn static_image(message: &'static str, source: SealedStaticApplicationErrorV1) -> Self {
        Self::StaticImage { message, source }
    }

    /// Returns the stable failure category.
    pub const fn kind(&self) -> IssuerAdmissionErrorKindV1 {
        match self {
            Self::Failure { kind, .. } => *kind,
            Self::StaticImage { .. } => IssuerAdmissionErrorKindV1::ExecutableNotStatic,
            Self::ServiceAdmission(_) => IssuerAdmissionErrorKindV1::ServiceAdmission,
            Self::Protocol(_) => IssuerAdmissionErrorKindV1::Protocol,
        }
    }
}

impl fmt::Display for ProtectedCompilerExecutionIssuerAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failure { message, .. } => formatter.write_str(message),
            Self::StaticImage { message, .. } => formatter.write_str(message),
            Self::ServiceAdmission(error) => {
                write!(formatter, "protected service changed: {error}")
            }
            Self::Protocol(error) => write!(formatter, "issuer protocol input is invalid: {error}"),
        }
    }
}

impl Error for ProtectedCompilerExecutionIssuerAdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Failure { source, .. } => {
                source.as_ref().map(|error| error as &(dyn Error + 'static))
            }
            Self::StaticImage { source, .. } => Some(source),
            Self::ServiceAdmission(error) => Some(error),
            Self::Protocol(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};

    use super::*;
    use crate::test_process_execution;

    fn policy(key: &SigningKey) -> CompilerExecutionIssuerPolicyV1 {
        CompilerExecutionIssuerPolicyV1::new(
            1,
            CompilerExecutionIssuerMeasurementV1::new([1; 32], 1).unwrap(),
            sealed_static_issuer_runtime_measurement_v1(),
            key.verifying_key().to_bytes(),
            SigningKey::from_bytes(&[0x71; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn runtime_measurement_is_exact_and_nonzero() {
        let measurement = sealed_static_issuer_runtime_measurement_v1();
        let expected: [u8; 32] = Sha256::digest(SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1).into();
        assert_eq!(measurement.sha256(), expected);
        assert_eq!(
            measurement.byte_len(),
            SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1.len() as u64
        );
    }

    #[test]
    fn signing_key_capability_is_revalidated_and_retained() {
        let key = SigningKey::from_bytes(&[0x31; 32]);
        let policy = policy(&key);
        let mut seed = key.to_bytes();
        let capability =
            CompilerExecutionSigningKeyCapabilityV1::create_and_zeroize(&mut seed, &policy)
                .unwrap();
        assert_eq!(seed, [0; KEY_BYTES]);
        let admitted = ProtectedIssuerSigningKeyV1::admit(capability, &policy).unwrap();
        admitted.validate(&policy).unwrap();
        assert_eq!(admitted.key.as_bytes(), key.as_bytes());

        let mut other_seed = key.to_bytes();
        let other_capability =
            CompilerExecutionSigningKeyCapabilityV1::create_and_zeroize(&mut other_seed, &policy)
                .unwrap();
        let other_policy = CompilerExecutionIssuerPolicyV1::new(
            1,
            policy.executable(),
            policy.runtime(),
            SigningKey::from_bytes(&[0x7f; 32])
                .verifying_key()
                .to_bytes(),
            *policy.external_anchor_verifying_key(),
        )
        .unwrap();
        let error =
            ProtectedIssuerSigningKeyV1::admit(other_capability, &other_policy).unwrap_err();
        assert_eq!(error.kind(), IssuerAdmissionErrorKindV1::KeyCapability);
    }

    #[test]
    #[ignore = "subprocess helper for irreversible issuer process hardening"]
    fn issuer_process_hardening_helper() {
        let process = ProtectedIssuerProcessV1::harden().unwrap();
        process.validate().unwrap();
        println!("FE2O3_ISSUER_PROCESS_HARDENED");
    }

    #[test]
    fn issuer_process_hardening_is_observed_in_a_child() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "compiler_execution_issuer::tests::issuer_process_hardening_helper",
                "--exact",
                "--ignored",
                "--nocapture",
            ])
            .stdin(Stdio::null());
        let output = test_process_execution::capture_output(&mut command).unwrap();
        assert!(
            output.status.success(),
            "child stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("FE2O3_ISSUER_PROCESS_HARDENED"));
    }

    #[test]
    fn current_test_binary_measurement_fails_closed_or_is_validated_static() {
        let result = current_static_issuer_measurements_v1();
        if let Ok(measurements) = result {
            assert_ne!(measurements.sealed_static_identity(), [0; 32]);
            assert_ne!(measurements.executable().sha256(), [0; 32]);
        } else {
            assert!(matches!(
                result.unwrap_err().kind(),
                IssuerAdmissionErrorKindV1::ExecutableNotStatic
                    | IssuerAdmissionErrorKindV1::ExecutableSize
            ));
        }
    }
}
