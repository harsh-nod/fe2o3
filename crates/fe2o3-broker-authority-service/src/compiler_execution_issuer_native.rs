//! Native issuer custody admission and its bounded durable service consumer.
use super::{
    CurrentStaticIssuerMeasurementsV1 as Measurements, FileSnapshotV1 as Snapshot,
    IssuerAdmissionErrorKindV1 as Kind, ProtectedIssuerProcessV1 as Process,
    RetainedStaticIssuerExecutableV1 as Executable, native_checks::IssuerInspectionError,
    require_close_on_exec, validate_executable_snapshot,
};
use crate::{
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor,
    ProtectedExternalAnchorServiceErrorV2 as AnchorError,
    ProtectedServiceAdmissionErrorV2 as ServiceError, ProtectedServiceAdmissionV2 as Service,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2 as KeyError, CompilerExecutionSigningKeyCapabilityV2 as Key,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2 as Policy;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_runtime_protocol::{
    CompilerExecutionAttestationErrorV1, CompilerExecutionIssuerMeasurementV1,
    SEALED_STATIC_APPLICATION_WORKSPACE_BYTES_V1, SealedStaticApplicationErrorV1,
    sealed_static_application_identity_v1, sealed_static_application_work_bound_v1,
    sealed_static_issuer_runtime_measurement_v1,
};
use sha2::{Digest, Sha256};
use std::{fmt, fs::File, marker::PhantomData, mem::size_of};

#[path = "compiler_execution_issuer_native_service.rs"]
mod service;
pub use service::NativeIssuerServiceError as ProtectedCompilerExecutionIssuerServiceErrorV2;

const ENTRY_WORK: usize = 8;
// Fixed comparisons, process-security queries, opens/fstats/flags, and closes.
// Each image pass and nested native owner separately prepays its own operation.
const OUTER_WORK: usize = ENTRY_WORK + 64 * 1024;
const IMAGE_IO_WORK: usize = ENTRY_WORK + 16 * 1024;
const IMAGE_FRAME: usize = SEALED_STATIC_APPLICATION_WORKSPACE_BYTES_V1 + 8192;

/// Unreserved growth over all consumed inputs, never a second full owner charge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedCompilerExecutionIssuerStorageV2(usize);
impl ProtectedCompilerExecutionIssuerStorageV2 {
    /// Reserve immediately on the same ledger before retaining the returned owner.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}
use ProtectedCompilerExecutionIssuerStorageV2 as Storage;

/// Protected process, static executable and exact native key/policy/transport custody.
///
/// This consumes native owners without legacy conversion or retry. Process and
/// executable predicates are shared with V1; image reads are bounded positional
/// reads and refuse short reads/EINTR instead of retrying. The key stays inside
/// its existing native capability: no seed is read or duplicated here.
///
/// Admission checks a caller-pinned policy, not its provisioning provenance.
/// The anchor is retained transport custody only: no exchange has authenticated
/// an observation under the policy's anchor key. `serve_native_preparation`
/// consumes this owner into singleton recovery, independently observed issuance,
/// and durable native Worker publication/currentness exchanges. It verifies the
/// separately pinned anchor response and exact journal joins before replying.
/// It does not publish readiness or alter deployment. Raw signing,
/// descriptor extraction and V1 conversions remain unavailable. The V1 serving
/// entrypoint is unchanged.
///
/// Keep input reservations live and use the original cumulative budget for all
/// calls. This owner borrows the work meter's lifetime and rejects a different
/// live meter before revalidation performs I/O. Input storage reservations remain
/// the caller's obligation. Scopes restore entry storage on success, refusal and unwind without
/// refunding work or denial history. Returned storage is only growth; retire
/// the full charge after drop. Logical quotas exclude allocator overhead, kernel
/// objects, generated stack/RSS, latency and diagnostic rendering. Continuity
/// is a point-in-time observation, not exclusive custody or perpetual liveness.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionV2;
/// fn clone<T: Clone>() {}
/// clone::<ProtectedCompilerExecutionIssuerAdmissionV2<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionV2;
/// fn fd<T: std::os::fd::AsFd>() {}
/// fd::<ProtectedCompilerExecutionIssuerAdmissionV2<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::{ProtectedCompilerExecutionIssuerAdmissionV1, ProtectedCompilerExecutionIssuerAdmissionV2};
/// fn upgrade(old: ProtectedCompilerExecutionIssuerAdmissionV1) -> ProtectedCompilerExecutionIssuerAdmissionV2<'static> {
///     old.into()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::{ProtectedCompilerExecutionIssuerV1, ProtectedCompilerExecutionIssuerAdmissionV2};
/// fn activate(native: ProtectedCompilerExecutionIssuerAdmissionV2) {
///     let _ = ProtectedCompilerExecutionIssuerV1::admit(native);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionV2;
/// fn sign(native: ProtectedCompilerExecutionIssuerAdmissionV2) { let _ = native.signing_key(); }
/// ```
/// The originating work meter cannot be replaced while this owner is live:
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::{ProtectedCompilerExecutionIssuerAdmissionV2 as Admission,
///     ProtectedIssuerProcessV1, ProtectedServiceAdmissionV2, ProtectedExternalAnchorServiceAdmissionV2};
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV2;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2;
/// use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn replace_work(process: ProtectedIssuerProcessV1, service: ProtectedServiceAdmissionV2,
///     policy: CompilerExecutionIssuerPolicyV2, key: CompilerExecutionSigningKeyCapabilityV2,
///     anchor: ProtectedExternalAnchorServiceAdmissionV2) {
///     let mut work = Work::new(usize::MAX);
///     let mut budget = Budget::new(&mut work, usize::MAX);
///     let (admission, _) = Admission::admit(process, service, policy, key, anchor, &mut budget).unwrap();
///     drop(budget);
///     work = Work::new(usize::MAX);
///     assert!(!admission.grants_compiler_authority());
/// }
/// ```
pub struct ProtectedCompilerExecutionIssuerAdmissionV2<'work> {
    process: Process,
    service: Service,
    policy: Policy,
    signing_key: Key,
    anchor: Anchor,
    executable: Executable,
    work_ledger: Ledger,
    // Ledger identities are addresses, so their original borrow must stay live.
    _work: PhantomData<&'work Work>,
}
type Admission<'work> = ProtectedCompilerExecutionIssuerAdmissionV2<'work>;

impl<'work> Admission<'work> {
    /// Logical charge for the incoming hardened process token and its receipt.
    pub const PROCESS_STORAGE: usize = size_of::<(Process, Storage)>();
    /// Extra owner growth, including the retained executable and output receipt.
    pub const ADDITIONAL_STORAGE: usize = size_of::<(Executable, Ledger, Storage)>() + 64;
    /// Outer staging and inspection scratch. Nested checks reserve separately.
    pub const FRAME_STORAGE: usize = 4 * size_of::<Self>() + 8192;

    /// Consumes a hardened process and four prepaid, native policy/transport/key
    /// owners. No caller-supplied executable, measurement, subject or receipt is
    /// accepted. Failure drops every consumed input even on entry denial.
    pub fn admit(
        process: Process,
        service: Service,
        policy: Policy,
        signing_key: Key,
        anchor: Anchor,
        budget: &mut Budget<'work>,
    ) -> Result<(Self, Storage)> {
        let floor = Self::input_storage(&service, &policy, &signing_key, &anchor);
        Self::scope(budget, floor, |budget| {
            process.validate()?;
            service.validate_continuity(budget)?;
            anchor.validate_continuity(budget)?;
            signing_key.revalidate(&policy, budget)?;
            let executable = observe_executable(budget)?;
            check_policy(executable.measurements, &policy)?;
            let admitted = Self {
                process,
                service,
                policy,
                signing_key,
                anchor,
                executable,
                work_ledger: budget.work_ledger_identity_v1(),
                _work: PhantomData,
            };
            // The enclosing prepaid frame covers the staged owner until return.
            admitted.check(budget)?;
            Ok((admitted, Storage(Self::ADDITIONAL_STORAGE)))
        })
    }

    /// Revalidates the same retained owners and current process profile, under
    /// the original ledger. This does not mint execution or publication authority.
    pub fn validate_continuity(&self, budget: &mut Budget<'_>) -> Result<()> {
        require_work_ledger(self.work_ledger, budget)?;
        Self::scope(budget, self.retained_storage(), |budget| self.check(budget))
    }

    fn check(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.process.validate()?;
        self.service.validate_continuity(budget)?;
        self.anchor.validate_continuity(budget)?;
        self.signing_key.revalidate(&self.policy, budget)?;
        validate_executable(&self.executable, budget)?;
        check_policy(self.executable.measurements, &self.policy)?;
        self.service.validate_continuity(budget)?;
        self.anchor.validate_continuity(budget)?;
        self.process.validate()?;
        Ok(())
    }

    /// Caller-pinned native policy; a borrow does not grant signing authority.
    pub const fn policy(&self) -> &Policy {
        &self.policy
    }

    /// Measurements independently derived from this process's running image.
    pub const fn measurements(&self) -> Measurements {
        self.executable.measurements
    }

    /// Admission alone cannot authenticate a supervised compiler occurrence.
    pub const fn authenticates_protected_compiler_execution(&self) -> bool {
        false
    }

    /// Admission alone grants no compiler, publication, load or GPU authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// Full retained logical charge. Retire only after dropping the whole owner.
    pub const fn retained_storage(&self) -> usize {
        Self::input_storage(&self.service, &self.policy, &self.signing_key, &self.anchor)
            + Self::ADDITIONAL_STORAGE
    }

    const fn input_storage(
        service: &Service,
        policy: &Policy,
        key: &Key,
        anchor: &Anchor,
    ) -> usize {
        Self::PROCESS_STORAGE
            + service.retained_storage()
            + policy.retained_storage()
            + key.retained_storage()
            + anchor.retained_storage()
    }

    fn scope<T>(
        budget: &mut Budget<'_>,
        floor: usize,
        operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
    ) -> Result<T> {
        budget.with_prepaid_scope(
            floor,
            ENTRY_WORK,
            OUTER_WORK,
            Self::FRAME_STORAGE,
            operation,
        )
    }
}

impl fmt::Debug for Admission<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProtectedCompilerExecutionIssuerAdmissionV2")
            .field("authority", &"admission-only")
            .field("policy", &self.policy)
            .field("measurements", &self.executable.measurements)
            .finish_non_exhaustive()
    }
}

fn require_work_ledger(expected: Ledger, budget: &Budget<'_>) -> Result<()> {
    if expected != budget.work_ledger_identity_v1() {
        return Err(Resource::Accounting.into());
    }
    Ok(())
}

fn check_policy(measurements: Measurements, policy: &Policy) -> Result<()> {
    if measurements.executable != policy.executable() {
        return Err(IssuerInspectionError::new(
            Kind::ExecutablePolicyMismatch,
            "running issuer executable does not match the pinned native policy",
        )
        .into());
    }
    if measurements.runtime != policy.runtime() {
        return Err(IssuerInspectionError::new(
            Kind::RuntimePolicyMismatch,
            "issuer runtime closure does not match the pinned native policy",
        )
        .into());
    }
    Ok(())
}

fn observe_executable(budget: &mut Budget<'_>) -> Result<Executable> {
    let image = rustix::fs::open(
        "/proc/self/exe",
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
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

fn inspect_executable(image: &File) -> Result<Snapshot> {
    require_close_on_exec(image, Kind::ExecutableCloseOnExec)?;
    Ok(validate_executable_snapshot(image)?)
}

fn validate_executable(executable: &Executable, budget: &mut Budget<'_>) -> Result<()> {
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

// Each pass reserves its whole image and parser workspace before allocation or
// I/O. Read and EOF probes each make one syscall; refusal never retries V1 I/O.
fn measure_executable(
    image: &File,
    before: Snapshot,
    budget: &mut Budget<'_>,
) -> Result<Measurements> {
    let length = usize::try_from(before.size).map_err(|_| Resource::Arithmetic)?;
    let (work, storage) = image_resources(length)?;
    budget.with_prepaid_scope(size_of::<File>(), ENTRY_WORK, work, storage, |_| {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Resource::Allocation)?;
        bytes.resize(length, 0);
        read_image(image, &mut bytes, before)?;
        let sealed_static_identity = sealed_static_application_identity_v1(&bytes)
            .map_err(|error| Error(Failure::StaticImage(error)))?;
        let executable =
            CompilerExecutionIssuerMeasurementV1::new(Sha256::digest(&bytes).into(), before.size)
                .map_err(|error| Error(Failure::Protocol(error)))?;
        Ok(Measurements {
            executable,
            runtime: sealed_static_issuer_runtime_measurement_v1(),
            sealed_static_identity,
        })
    })
}

fn image_resources(length: usize) -> Result<(usize, usize)> {
    let parser = sealed_static_application_work_bound_v1(length).ok_or(Resource::Arithmetic)?;
    // Initialization, two hashes, comparisons and deallocation, above parser visits.
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

fn read_image(image: &File, bytes: &mut [u8], before: Snapshot) -> Result<()> {
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

/// Native refusal with preserved resource errors; formatting is a caller operation.
#[derive(Debug)]
pub struct ProtectedCompilerExecutionIssuerAdmissionErrorV2(Failure);
#[derive(Debug)]
enum Failure {
    Resource(Resource),
    Inspection(IssuerInspectionError),
    Service(ServiceError),
    Anchor(AnchorError),
    Key(KeyError),
    StaticImage(SealedStaticApplicationErrorV1),
    Protocol(CompilerExecutionAttestationErrorV1),
}
use ProtectedCompilerExecutionIssuerAdmissionErrorV2 as Error;
type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Resource refusals preserve the exact cumulative ledger error.
    pub fn resource(&self) -> Option<Resource> {
        match &self.0 {
            Failure::Resource(error) => Some(*error),
            Failure::Service(error) => error.resource(),
            Failure::Anchor(error) => error.resource(),
            Failure::Key(KeyError::Resource(error)) => Some(*error),
            _ => None,
        }
    }
    /// Shared process/image category, or the native key/transport refusal class.
    pub const fn kind(&self) -> Option<Kind> {
        match &self.0 {
            Failure::Resource(_) => None,
            Failure::Inspection(error) => Some(error.kind),
            Failure::Service(_) | Failure::Anchor(_) => Some(Kind::ServiceAdmission),
            Failure::Key(_) => Some(Kind::KeyCapability),
            Failure::StaticImage(_) => Some(Kind::ExecutableNotStatic),
            Failure::Protocol(_) => Some(Kind::Protocol),
        }
    }
}

impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self(Failure::Resource(error))
    }
}
impl From<IssuerInspectionError> for Error {
    fn from(error: IssuerInspectionError) -> Self {
        Self(Failure::Inspection(error))
    }
}
impl From<ServiceError> for Error {
    fn from(error: ServiceError) -> Self {
        Self(Failure::Service(error))
    }
}
impl From<AnchorError> for Error {
    fn from(error: AnchorError) -> Self {
        Self(Failure::Anchor(error))
    }
}
impl From<KeyError> for Error {
    fn from(error: KeyError) -> Self {
        Self(Failure::Key(error))
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Failure::Resource(e) => e.fmt(f),
            Failure::Inspection(e) => e.fmt(f),
            Failure::Service(e) => e.fmt(f),
            Failure::Anchor(e) => e.fmt(f),
            Failure::Key(e) => e.fmt(f),
            Failure::StaticImage(e) => e.fmt(f),
            Failure::Protocol(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match &self.0 {
            Failure::Resource(e) => e,
            Failure::Inspection(e) => e,
            Failure::Service(e) => e,
            Failure::Anchor(e) => e,
            Failure::Key(e) => e,
            Failure::StaticImage(e) => e,
            Failure::Protocol(e) => e,
        })
    }
}

const _: () = {
    assert!(
        Admission::ADDITIONAL_STORAGE
            >= size_of::<Executable>() + size_of::<Ledger>() + size_of::<Storage>()
    );
    assert!(8 * size_of::<Error>() + 128 * size_of::<usize>() <= 8192);
    assert!(4 * size_of::<Snapshot>() + 4 * size_of::<std::fs::Metadata>() <= 8192);
};

#[cfg(test)]
#[path = "compiler_execution_issuer_native_tests.rs"]
mod tests;
