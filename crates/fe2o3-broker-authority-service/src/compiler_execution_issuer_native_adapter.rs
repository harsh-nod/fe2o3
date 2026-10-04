// One admission and measurement implementation; callers bind actual native owners.
macro_rules! issuer_native_adapter {
    ($(#[$attrs:meta])* $Admission:ident, $Storage:ident, $Error:ident) => {
const ENTRY_WORK: usize = 8;
// Fixed comparisons, process-security queries, opens/fstats/flags, and closes.
// Each image pass and nested native owner separately prepays its own operation.
const OUTER_WORK: usize = ENTRY_WORK + 64 * 1024;

/// Unreserved growth over all consumed inputs, never a second full owner charge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct $Storage(usize);
impl $Storage {
    /// Reserve immediately on the same ledger before retaining the returned owner.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}
use $Storage as Storage;

$( #[$attrs] )*
pub struct $Admission<'work> {
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
type Admission<'work> = $Admission<'work>;

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
        f.debug_struct(stringify!($Admission))
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
    super::native_image::check_policy(measurements, policy.executable(), policy.runtime())
        .map_err(Into::into)
}

fn observe_executable(budget: &mut Budget<'_>) -> Result<Executable> {
    super::native_image::observe_self(budget).map_err(Into::into)
}

#[cfg(test)]
fn inspect_executable(image: &File) -> Result<Snapshot> {
    super::native_image::inspect_executable(image).map_err(Into::into)
}

fn validate_executable(executable: &Executable, budget: &mut Budget<'_>) -> Result<()> {
    super::native_image::validate_executable(executable, budget).map_err(Into::into)
}

#[cfg(test)]
fn measure_executable(
    image: &File,
    before: Snapshot,
    budget: &mut Budget<'_>,
) -> Result<Measurements> {
    super::native_image::measure_executable(image, before, budget).map_err(Into::into)
}

#[cfg(test)]
fn image_resources(length: usize) -> Result<(usize, usize)> {
    super::native_image::image_resources(length).map_err(Into::into)
}

/// Native refusal with preserved resource errors; formatting is a caller operation.
#[derive(Debug)]
pub struct $Error(Failure);
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
use $Error as Error;
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
impl From<super::native_image::ImageError> for Error {
    fn from(error: super::native_image::ImageError) -> Self {
        use super::native_image::ImageError;
        Self(match error {
            ImageError::Resource(e) => Failure::Resource(e),
            ImageError::Inspection(e) => Failure::Inspection(e),
            ImageError::StaticImage(e) => Failure::StaticImage(e),
            ImageError::Protocol(e) => Failure::Protocol(e),
        })
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

    };
}
pub(super) use issuer_native_adapter;
