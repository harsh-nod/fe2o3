//! Root-staged proof-helper bytes, not proof-role or process admission.
//!
//! The approved runtime supplies the sole source measurement and retains its
//! original immutable inventory. The sealed image is a different kernel object.
//! Credentials are caller configuration: the current runtime profile contains
//! supervisor/anchor roles, not an independently approved proof-helper identity.

use fe2o3_build_authority::{
    COMPILER_RUNTIME_MANIFEST_MAX_BYTES_V1 as MANIFEST_BYTES,
    COMPILER_RUNTIME_MANIFEST_MAX_ENTRIES_V1 as MAX_ENTRIES,
    COMPILER_RUNTIME_MANIFEST_MAX_FILE_BYTES_V1 as MAX_IMAGE, CompilerRuntimeEntryV1 as Entry,
    CompilerRuntimeRoleV1 as Role,
};
use fe2o3_compiler_closure_capability::{
    RetainedCompilerRuntimeErrorV1 as RuntimeError, RetainedCompilerRuntimeV1 as Runtime,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_profile::ProtectedServiceCredentialProfileV1 as Credentials;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV2 as ImageError,
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOwnerV1 as Owner, ProtectedStaticExecutableStorageV2 as ImageStorage,
    ProtectedStaticExecutableV2 as Image,
};
use std::{fmt, fs::File, mem::size_of};

const ENTRY_WORK: usize = 8;
const IMAGE_ROLE: &str = "root-staged proof executor helper";
type Result<T> = std::result::Result<T, ProofHelperBackingError>;

/// Fixed errors preserve nested accounting/origin failures without new strings.
#[derive(Debug)]
pub(crate) enum ProofHelperBackingError {
    Resource(Resource),
    Runtime(RuntimeError),
    Image(ImageError),
    RootRequired,
    CoordinatorChanged,
    BindingMismatch,
    Manifest(&'static str),
    Io {
        operation: &'static str,
        source: rustix::io::Errno,
    },
}
impl From<Resource> for ProofHelperBackingError {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<RuntimeError> for ProofHelperBackingError {
    fn from(error: RuntimeError) -> Self {
        Self::Runtime(error)
    }
}
impl From<ImageError> for ProofHelperBackingError {
    fn from(error: ImageError) -> Self {
        Self::Image(error)
    }
}
impl fmt::Display for ProofHelperBackingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Runtime(e) => e.fmt(f),
            Self::Image(e) => e.fmt(f),
            Self::RootRequired => f.write_str("proof helper staging requires exact root IDs"),
            Self::CoordinatorChanged => f.write_str("proof helper staging process changed"),
            Self::BindingMismatch => f.write_str("sealed proof helper binding changed"),
            Self::Manifest(reason) => f.write_str(reason),
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
        }
    }
}
impl std::error::Error for ProofHelperBackingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Runtime(e) => Some(e),
            Self::Image(e) => Some(e),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Inert ledger binding, not a source of runtime or executable approval.
struct Account {
    ledger: Ledger,
    address: usize,
}
impl Account {
    fn capture(b: &Budget<'_>) -> Self {
        Self {
            ledger: b.work_ledger_identity_v1(),
            address: b as *const Budget<'_> as usize,
        }
    }
    fn require(&self, b: &Budget<'_>, floor: usize) -> Result<()> {
        if self.ledger != b.work_ledger_identity_v1()
            || self.address != b as *const Budget<'_> as usize
            || b.storage() < floor
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}

/// Move-only complete inventory plus independently sealed executable custody.
/// No provider, raw owner import, role selection or deployment assertion exists.
pub(crate) struct ProofHelperBacking {
    image: Image,
    runtime: Runtime,
    credentials: Credentials,
    prepared_by: rustix::process::Pid,
    account: Account,
    retained: usize,
}

impl ProofHelperBacking {
    const ENVELOPE: usize = size_of::<(Self, usize)>() - size_of::<Runtime>() - size_of::<Image>();
    /// Local scalar/credential/FD work, at most two bounded manifest selections,
    /// and descriptor retirement on a consuming refusal.
    /// Runtime and Image operations additionally charge their existing envelopes
    /// on the same ledger; this is NOT an aggregate preparation/launch quota.
    pub(crate) const LOCAL_WORK: usize =
        ENTRY_WORK + (MAX_ENTRIES + 64) * 1088 + 8 * MANIFEST_BYTES;
    /// Local fixed frames only; nested scratch and coexisting owners are separate.
    pub(crate) const FRAME_STORAGE: usize = 4 * size_of::<(Self, usize)>()
        + 8 * size_of::<ProofHelperBackingError>()
        + 4 * size_of::<rustix::fs::Stat>()
        + 8192;

    /// Consumes a genuinely admitted runtime whose FULL reservation remains live.
    /// Reserve returned growth before retaining Self. On error all consumed local
    /// owners drop, but the original caller-owned reservation is unchanged.
    /// No process, namespace, proof receipt or approved helper credentials result.
    pub(crate) fn prepare(
        runtime: Runtime,
        credentials: Credentials,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, usize)> {
        let input = runtime.required_retained_storage();
        budget.with_prepaid_scope(
            input,
            ENTRY_WORK,
            Self::LOCAL_WORK,
            Self::FRAME_STORAGE,
            |b| {
                require_root()?;
                runtime.revalidate(b)?;
                let measurement = select_helper(runtime.manifest().entries())?;
                let (source, charge) = runtime.try_clone_proof_executor_for_exec(b)?;
                let source_storage = charge.full_storage();
                b.reserve_storage(source_storage)?;
                let stat =
                    rustix::fs::fstat(&source).map_err(|source| ProofHelperBackingError::Io {
                        operation: "inspect approved proof helper source",
                        source,
                    })?;
                let original = (stat.st_dev, stat.st_ino);
                // This validator applies to the original inventory inode, never the
                // fresh sealed image. No caller-supplied role/path/pin is substituted.
                runtime.validate_proof_executor_exec_transfer(&source, b)?;
                let image =
                    seal_transferred_source(source, source_storage, measurement, credentials, b)?;
                let object = image.object_identity();
                if (object.device(), object.inode()) == original {
                    return Err(ProofHelperBackingError::BindingMismatch);
                }
                b.reserve_storage(Self::ENVELOPE)?;
                let (retained, growth) = retained_storage_for(input, image.retained_storage())?;
                let owner = Self {
                    image,
                    runtime,
                    credentials,
                    prepared_by: rustix::process::getpid(),
                    account: Account::capture(b),
                    retained,
                };
                // All final image/inventory rechecks keep both complete owners charged.
                owner.check(b)?;
                Ok((owner, growth))
            },
        )
    }

    pub(crate) const fn credentials(&self) -> Credentials {
        self.credentials
    }
    pub(crate) const fn measurement(&self) -> Measurement {
        self.image.measurement()
    }
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Rechecks original root context, runtime approval/origins and sealed image.
    pub(crate) fn revalidate(&self, b: &mut Budget<'_>) -> Result<()> {
        self.entry(b)?;
        b.with_prepaid_scope(
            self.retained,
            0,
            Self::LOCAL_WORK - ENTRY_WORK,
            Self::FRAME_STORAGE,
            |b| self.check(b),
        )
    }

    /// Returns an inert CLOEXEC exec source for the existing staging adapter.
    /// The returned charge is FULL and unreserved; preserve this owner's charge.
    pub(crate) fn try_clone_for_exec(&self, b: &mut Budget<'_>) -> Result<(File, ImageStorage)> {
        self.entry(b)?;
        let transfer = Image::file_storage(self.measurement())?;
        let scratch = Self::FRAME_STORAGE
            .checked_add(transfer)
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(
            self.retained,
            0,
            Self::LOCAL_WORK - ENTRY_WORK,
            scratch,
            |b| {
                self.check(b)?;
                let (file, charge) = self.image.try_clone_for_exec(b)?;
                if charge.additional_storage() != transfer {
                    return Err(Resource::Accounting.into());
                }
                self.image.revalidate_exec_clone(&file, b)?;
                self.check(b)?;
                self.image.revalidate_exec_clone(&file, b)?;
                Ok((file, charge))
            },
        )
    }

    /// Requires a clone of the SEALED image, not the inventory's source inode.
    /// Both this complete backing and the full transfer must remain reserved.
    pub(crate) fn revalidate_exec_clone(&self, file: &File, b: &mut Budget<'_>) -> Result<()> {
        self.entry(b)?;
        let floor = self
            .retained
            .checked_add(Image::file_storage(self.measurement())?)
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(
            floor,
            0,
            Self::LOCAL_WORK - ENTRY_WORK,
            Self::FRAME_STORAGE,
            |b| {
                self.check(b)?;
                self.image.revalidate_exec_clone(file, b)?;
                self.check(b)?;
                self.image.revalidate_exec_clone(file, b)?;
                Ok(())
            },
        )
    }

    fn entry(&self, b: &mut Budget<'_>) -> Result<()> {
        b.charge_work(ENTRY_WORK)?;
        self.account.require(b, self.retained)
    }
    fn check(&self, b: &mut Budget<'_>) -> Result<()> {
        self.account.require(b, self.retained)?;
        require_root()?;
        if rustix::process::getpid() != self.prepared_by {
            return Err(ProofHelperBackingError::CoordinatorChanged);
        }
        self.runtime.revalidate(b)?;
        let expected = select_helper(self.runtime.manifest().entries())?;
        check_image(&self.image, expected, self.credentials, b)?;
        self.runtime.revalidate(b)?;
        require_root()
    }
}

fn require_root() -> Result<()> {
    fe2o3_protected_service_spawn::require_exact_root_identity_v1()
        .map_err(|_| ProofHelperBackingError::RootRequired)
}

// Only production's retained manifest reaches this selector. Testing inert entry
// arrays exercises framing/selection without manufacturing approval or Backing.
fn select_helper<'a>(entries: impl Iterator<Item = Entry<'a>>) -> Result<Measurement> {
    let mut measurement = None;
    for (index, entry) in entries.take(MAX_ENTRIES + 1).enumerate() {
        if index == MAX_ENTRIES {
            return Err(ProofHelperBackingError::Manifest(
                "proof helper inventory exceeds bound",
            ));
        }
        if entry.role == Role::ProofExecutorHelper {
            if measurement.is_some() {
                return Err(ProofHelperBackingError::Manifest(
                    "proof helper role is not unique",
                ));
            }
            measurement = Some(
                Measurement::new(entry.sha256, entry.length, MAX_IMAGE)
                    .map_err(ImageError::from)?,
            );
        }
    }
    measurement.ok_or(ProofHelperBackingError::Manifest(
        "proof helper role is absent",
    ))
}

fn retained_storage_for(runtime: usize, image: usize) -> Result<(usize, usize)> {
    let growth = image
        .checked_add(ProofHelperBacking::ENVELOPE)
        .ok_or(Resource::Arithmetic)?;
    Ok((
        runtime.checked_add(growth).ok_or(Resource::Arithmetic)?,
        growth,
    ))
}

// The caller prepays the complete actual transfer before entering. The two
// libraries' File-charge envelopes need not be layout-identical: supplement the
// smaller one before sealing, then retire any excess only after source consumption.
fn seal_transferred_source(
    source: File,
    source_storage: usize,
    expected: Measurement,
    credentials: Credentials,
    b: &mut Budget<'_>,
) -> Result<Image> {
    let source_floor = Image::file_storage(expected)?;
    let reserved = source_floor.max(source_storage);
    b.reserve_storage(reserved - source_storage)?;
    let owner = Owner::new(credentials.uid(), credentials.gid()).map_err(ImageError::from)?;
    let (image, charge) = Image::seal_source_for_owner(source, expected, owner, IMAGE_ROLE, b)?;
    b.reserve_storage(charge.additional_storage())?;
    if source_floor
        .checked_add(charge.additional_storage())
        .ok_or(Resource::Arithmetic)?
        != image.retained_storage()
    {
        return Err(Resource::Accounting.into());
    }
    b.release_storage(reserved - source_floor)?;
    Ok(image)
}

fn check_image(
    image: &Image,
    expected: Measurement,
    credentials: Credentials,
    b: &mut Budget<'_>,
) -> Result<()> {
    image.revalidate(b)?;
    let object = image.object_identity();
    if image.measurement() != expected
        || object.uid() != credentials.uid()
        || object.gid() != credentials.gid()
    {
        return Err(ProofHelperBackingError::BindingMismatch);
    }
    Ok(())
}

#[cfg(test)]
#[path = "proof_helper_backing_tests.rs"]
mod tests;
