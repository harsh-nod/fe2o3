//! Shared, bounded image mechanics. No supplied image or measurement is authority.

use super::{
    CurrentStaticIssuerMeasurementsV1 as Measurements, FileSnapshotV1 as Snapshot,
    IssuerAdmissionErrorKindV1 as Kind, MAX_COMPILER_EXECUTION_ISSUER_IMAGE_BYTES_V1,
    RetainedStaticIssuerExecutableV1 as Executable, native_checks::IssuerInspectionError,
    require_close_on_exec, validate_executable_snapshot,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3 as Policy;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_spawn::native_spawn::{
    ProtectedServiceSpawnErrorV2 as SpawnError, RootOwnedProtectedServiceChildV2 as Child,
    RootOwnedRetainedServiceChildV2 as RetainedChild,
};
use fe2o3_runtime_protocol::{
    CompilerExecutionAttestationErrorV1, CompilerExecutionIssuerMeasurementV1 as Measurement,
    SEALED_STATIC_APPLICATION_WORKSPACE_BYTES_V1, SealedStaticApplicationErrorV1,
    sealed_static_application_identity_v1, sealed_static_application_work_bound_v1,
    sealed_static_issuer_runtime_measurement_v1,
};
use rustix::{
    fs::{Mode, OFlags},
    path::DecInt,
};
use sha2::{Digest, Sha256};
use std::{fmt, fs::File, mem::size_of};

const ENTRY_WORK: usize = 8;
const IMAGE_IO_WORK: usize = ENTRY_WORK + 16 * 1024;
const IMAGE_FRAME: usize = SEALED_STATIC_APPLICATION_WORKSPACE_BYTES_V1 + 8192;
// Fixed procfs opens, filesystem/identity checks, image checks, comparisons and drops.
const CHILD_IO_WORK: usize = ENTRY_WORK + 32 * 1024;
const CHILD_FRAME: usize = 8192;

#[derive(Debug)]
pub(super) enum ImageError {
    Resource(Resource),
    Inspection(IssuerInspectionError),
    StaticImage(SealedStaticApplicationErrorV1),
    Protocol(CompilerExecutionAttestationErrorV1),
}
impl From<Resource> for ImageError {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<IssuerInspectionError> for ImageError {
    fn from(error: IssuerInspectionError) -> Self {
        Self::Inspection(error)
    }
}
impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Inspection(e) => e.fmt(f),
            Self::StaticImage(e) => e.fmt(f),
            Self::Protocol(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for ImageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Resource(e) => e,
            Self::Inspection(e) => e,
            Self::StaticImage(e) => e,
            Self::Protocol(e) => e,
        })
    }
}

pub(super) fn check_policy(
    measurements: Measurements,
    executable: Measurement,
    runtime: Measurement,
) -> Result<(), ImageError> {
    if measurements.executable != executable {
        return Err(IssuerInspectionError::new(
            Kind::ExecutablePolicyMismatch,
            "running issuer executable does not match the pinned native policy",
        )
        .into());
    }
    if measurements.runtime != runtime {
        return Err(IssuerInspectionError::new(
            Kind::RuntimePolicyMismatch,
            "issuer runtime closure does not match the pinned native policy",
        )
        .into());
    }
    Ok(())
}

pub(super) fn observe_self(budget: &mut Budget<'_>) -> Result<Executable, ImageError> {
    let image = rustix::fs::open(
        "/proc/self/exe",
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| {
        IssuerInspectionError::io(
            Kind::ExecutableInspect,
            "cannot open the running issuer executable",
            error.into(),
        )
    })?;
    let snapshot = inspect_executable(&image)?;
    let measurements = measure_executable(&image, snapshot, budget)?;
    Ok(Executable {
        image,
        snapshot,
        measurements,
    })
}

pub(super) fn inspect_executable(image: &File) -> Result<Snapshot, ImageError> {
    require_close_on_exec(image, Kind::ExecutableCloseOnExec)?;
    Ok(validate_executable_snapshot(image)?)
}

pub(super) fn validate_executable(
    executable: &Executable,
    budget: &mut Budget<'_>,
) -> Result<(), ImageError> {
    let snapshot = inspect_executable(&executable.image)?;
    if snapshot != executable.snapshot
        || measure_executable(&executable.image, snapshot, budget)? != executable.measurements
    {
        return Err(IssuerInspectionError::new(
            Kind::ExecutableChanged,
            "retained issuer executable metadata or bytes changed",
        )
        .into());
    }
    Ok(())
}

// Preserve the self-image schedule: one payload pread and one EOF probe, no retries.
pub(super) fn measure_executable(
    image: &File,
    before: Snapshot,
    budget: &mut Budget<'_>,
) -> Result<Measurements, ImageError> {
    let length = usize::try_from(before.size).map_err(|_| Resource::Arithmetic)?;
    let (work, storage) = image_resources(length)?;
    budget.with_prepaid_scope(size_of::<File>(), ENTRY_WORK, work, storage, |_| {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Resource::Allocation)?;
        bytes.resize(length, 0);
        read_image(image, &mut bytes, before)?;
        let sealed_static_identity =
            sealed_static_application_identity_v1(&bytes).map_err(ImageError::StaticImage)?;
        let executable = Measurement::new(Sha256::digest(&bytes).into(), before.size)
            .map_err(ImageError::Protocol)?;
        Ok(Measurements {
            executable,
            runtime: sealed_static_issuer_runtime_measurement_v1(),
            sealed_static_identity,
        })
    })
}

pub(super) fn image_resources(length: usize) -> Result<(usize, usize), ImageError> {
    let parser = sealed_static_application_work_bound_v1(length).ok_or(Resource::Arithmetic)?;
    let work = length
        .checked_mul(64)
        .and_then(|bytes| bytes.checked_add(parser))
        .and_then(|work| work.checked_add(IMAGE_IO_WORK))
        .ok_or(Resource::Arithmetic)?;
    let storage = length
        .checked_add(IMAGE_FRAME)
        .ok_or(Resource::Arithmetic)?;
    Ok((work, storage))
}

fn read_image(image: &File, bytes: &mut [u8], before: Snapshot) -> Result<(), ImageError> {
    let read = rustix::io::pread(image, &mut *bytes, 0).map_err(|error| {
        IssuerInspectionError::io(
            Kind::ExecutableRead,
            "cannot read issuer executable",
            error.into(),
        )
    })?;
    let mut trailing = [0_u8; 1];
    if read != bytes.len()
        || rustix::io::pread(image, &mut trailing, before.size).map_err(|error| {
            IssuerInspectionError::io(
                Kind::ExecutableRead,
                "cannot check issuer executable EOF",
                error.into(),
            )
        })? != 0
        || Snapshot::inspect(image, Kind::ExecutableInspect)? != before
    {
        return Err(IssuerInspectionError::new(
            Kind::ExecutableChanged,
            "issuer executable changed or returned a short read",
        )
        .into());
    }
    Ok(())
}

/// Logical work and extra scratch for one retained-child image observation.
/// Input owners stay prepaid; this quote grants no authority or storage ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootIssuerImageQuotaV3 {
    work: usize,
    scratch: usize,
}
impl RootIssuerImageQuotaV3 {
    pub const fn work(self) -> usize {
        self.work
    }
    pub const fn scratch(self) -> usize {
        self.scratch
    }
}

/// Quotes from the pinned policy's executable byte length, not a probed image size.
/// Includes both child liveness checks, procfs opens/checks, image measurement and
/// reopening the current executable. Scratch excludes already prepaid inputs,
/// allocator overhead, kernel objects, generated stack/RSS and error rendering.
pub fn retained_issuer_image_quota_v3(
    length: u64,
) -> Result<RootIssuerImageQuotaV3, RootIssuerImageErrorV3> {
    if length == 0 || length > MAX_COMPILER_EXECUTION_ISSUER_IMAGE_BYTES_V1 {
        return Err(IssuerInspectionError::new(
            Kind::ExecutableSize,
            "issuer policy image length is out of bounds",
        )
        .into());
    }
    let length = usize::try_from(length).map_err(|_| Resource::Arithmetic)?;
    let (work, scratch) = image_resources(length)?;
    Ok(RootIssuerImageQuotaV3 {
        work: work
            .checked_add(CHILD_IO_WORK)
            .and_then(|n| n.checked_add(2 * Child::OPERATION_WORK))
            .ok_or(Resource::Arithmetic)?,
        scratch: scratch
            .max(Child::OPERATION_SCRATCH)
            .checked_add(CHILD_FRAME)
            .ok_or(Resource::Arithmetic)?,
    })
}

/// Fresh image-only observation of the actual retained child, never issuer admission.
///
/// Opens its current procfs executable, refuses a policy-length mismatch before
/// any payload read, measures the static image/runtime, then reopens its current
/// executable and compares identities. Original-child non-reaping liveness
/// brackets the observation. No PID, descriptor, bytes or digest supplied by a
/// caller can replace the child-derived image.
///
/// Success is point-in-time policy-relative equality, not protection against a
/// subsequent exec or proof of policy provenance, credentials, namespaces,
/// readiness, or authority. A trusted compatible procfs mount and exclusive
/// consuming-wait ownership remain prerequisites. The coordinator owns account,
/// profile, namespace, manifest and original-policy continuity checks.
/// The observer must have permission to read the hardened child's procfs image;
/// denied access refuses without substituting launch-time bytes or claims.
///
/// Prepay the full child and policy charges; a policy already contained in the
/// child is not charged twice. This call never locks the retained payload, so it
/// may run inside `child.with_resources`. Temporary descriptors and image bytes
/// are dropped and entry storage is restored on success, refusal and unwind;
/// accepted work and denial history are never reset. The child is not consumed,
/// cancelled or reaped on failure; its owner remains responsible for cleanup.
pub fn validate_retained_issuer_image_v3<T: Send + 'static>(
    child: &RetainedChild<T>,
    policy: &Policy,
    budget: &mut Budget<'_>,
) -> Result<(), RootIssuerImageErrorV3> {
    // Validate the quote before any I/O, including policy lengths exceeding the cap.
    let _quota = retained_issuer_image_quota_v3(policy.executable().byte_len())?;
    budget.with_prepaid_scope(
        child.retained_storage().max(policy.retained_storage()),
        ENTRY_WORK,
        CHILD_IO_WORK,
        CHILD_FRAME,
        |budget| {
            require_live(child, budget)?;
            let process = open_child_proc(child.pid())?;
            let image = open_process_image(&process)?;
            let (before, measured) =
                measure_expected_image(&image, policy.executable().byte_len(), budget)?;
            check_policy(measured, policy.executable(), policy.runtime())?;
            let current = open_process_image(&process)?;
            require_current_image(&current, before)?;
            require_live(child, budget)
        },
    )
}

fn measure_expected_image(
    image: &File,
    expected: u64,
    budget: &mut Budget<'_>,
) -> Result<(Snapshot, Measurements), ImageError> {
    let before = inspect_expected_length(image, expected)?;
    let measured = measure_executable(image, before, budget)?;
    Ok((before, measured))
}

fn inspect_expected_length(image: &File, expected: u64) -> Result<Snapshot, ImageError> {
    let before = inspect_executable(image)?;
    if before.size != expected {
        return Err(IssuerInspectionError::new(
            Kind::ExecutablePolicyMismatch,
            "running issuer image length differs from pinned policy",
        )
        .into());
    }
    Ok(before)
}

fn require_current_image(current: &File, measured: Snapshot) -> Result<(), ImageError> {
    if inspect_executable(current)? != measured {
        return Err(IssuerInspectionError::new(
            Kind::ExecutableChanged,
            "current issuer executable differs from measured image",
        )
        .into());
    }
    Ok(())
}

fn require_live<T: Send + 'static>(
    child: &RetainedChild<T>,
    b: &mut Budget<'_>,
) -> Result<(), RootIssuerImageErrorV3> {
    if !child.is_live(b)? {
        return Err(RootIssuerImageErrorV3(Failure::NotLive));
    }
    Ok(())
}

fn proc_error(error: rustix::io::Errno) -> IssuerInspectionError {
    IssuerInspectionError::io(
        Kind::ExecutableInspect,
        "cannot inspect retained issuer procfs image",
        error.into(),
    )
}

fn require_procfs(file: &File) -> Result<(), ImageError> {
    if rustix::fs::fstatfs(file).map_err(proc_error)?.f_type != rustix::fs::PROC_SUPER_MAGIC {
        return Err(IssuerInspectionError::new(
            Kind::ExecutableInspect,
            "issuer process view is not procfs",
        )
        .into());
    }
    Ok(())
}

fn open_child_proc(pid: rustix::process::Pid) -> Result<File, ImageError> {
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY;
    let proc = File::from(
        rustix::fs::open("/proc", flags | OFlags::NOFOLLOW, Mode::empty()).map_err(proc_error)?,
    );
    require_procfs(&proc)?;
    let self_entry =
        File::from(rustix::fs::openat(&proc, "self", flags, Mode::empty()).map_err(proc_error)?);
    let numeric_self = File::from(
        rustix::fs::openat(
            &proc,
            DecInt::new(std::process::id()),
            flags | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(proc_error)?,
    );
    require_procfs(&self_entry)?;
    require_procfs(&numeric_self)?;
    let self_stat = rustix::fs::fstat(&self_entry).map_err(proc_error)?;
    let numeric_stat = rustix::fs::fstat(&numeric_self).map_err(proc_error)?;
    if (self_stat.st_dev, self_stat.st_ino) != (numeric_stat.st_dev, numeric_stat.st_ino) {
        return Err(IssuerInspectionError::new(
            Kind::ExecutableInspect,
            "procfs does not map the current PID consistently",
        )
        .into());
    }
    let child = File::from(
        rustix::fs::openat(
            &proc,
            DecInt::new(pid.as_raw_pid()),
            flags | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(proc_error)?,
    );
    require_procfs(&child)?;
    if rustix::fs::fstat(&child).map_err(proc_error)?.st_dev != self_stat.st_dev {
        return Err(IssuerInspectionError::new(
            Kind::ExecutableInspect,
            "issuer process is on a different procfs instance",
        )
        .into());
    }
    Ok(child)
}

fn open_process_image(process: &File) -> Result<File, ImageError> {
    Ok(File::from(
        rustix::fs::openat(
            process,
            "exe",
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(proc_error)?,
    ))
}

/// Bounded image/custody refusal. Diagnostics allocate only when rendered by callers.
#[derive(Debug)]
pub struct RootIssuerImageErrorV3(Failure);
#[derive(Debug)]
enum Failure {
    Image(ImageError),
    Child(SpawnError),
    NotLive,
}
impl RootIssuerImageErrorV3 {
    /// Preserves the cumulative ledger's exact refusal, including nested liveness.
    pub fn resource(&self) -> Option<Resource> {
        match &self.0 {
            Failure::Image(ImageError::Resource(e)) | Failure::Child(SpawnError::Resource(e)) => {
                Some(*e)
            }
            _ => None,
        }
    }
    /// Shared issuer image category, or `None` for resource/child-custody refusal.
    pub const fn kind(&self) -> Option<Kind> {
        match &self.0 {
            Failure::Image(ImageError::Inspection(e)) => Some(e.kind),
            Failure::Image(ImageError::StaticImage(_)) => Some(Kind::ExecutableNotStatic),
            Failure::Image(ImageError::Protocol(_)) => Some(Kind::Protocol),
            _ => None,
        }
    }
}
impl From<Resource> for RootIssuerImageErrorV3 {
    fn from(e: Resource) -> Self {
        Self(Failure::Image(ImageError::Resource(e)))
    }
}
impl From<ImageError> for RootIssuerImageErrorV3 {
    fn from(e: ImageError) -> Self {
        Self(Failure::Image(e))
    }
}
impl From<IssuerInspectionError> for RootIssuerImageErrorV3 {
    fn from(e: IssuerInspectionError) -> Self {
        ImageError::from(e).into()
    }
}
impl From<SpawnError> for RootIssuerImageErrorV3 {
    fn from(e: SpawnError) -> Self {
        Self(Failure::Child(e))
    }
}
impl fmt::Display for RootIssuerImageErrorV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Failure::Image(e) => e.fmt(f),
            Failure::Child(e) => e.fmt(f),
            Failure::NotLive => f.write_str("retained issuer child is not live"),
        }
    }
}
impl std::error::Error for RootIssuerImageErrorV3 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.0 {
            Failure::Image(e) => Some(e),
            Failure::Child(e) => Some(e),
            Failure::NotLive => None,
        }
    }
}

const _: () = {
    assert!(8 * size_of::<RootIssuerImageErrorV3>() + 128 * size_of::<usize>() <= CHILD_FRAME);
    assert!(
        8 * size_of::<rustix::fs::Stat>()
            + 4 * size_of::<rustix::fs::StatFs>()
            + 8 * size_of::<File>()
            <= CHILD_FRAME
    );
};

#[cfg(test)]
#[path = "compiler_execution_issuer_native_image_tests.rs"]
mod tests;
