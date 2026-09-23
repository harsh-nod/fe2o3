use super::{
    ProtectedStaticExecutableErrorV1 as ImageError,
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableObjectIdentityV1 as Object, ProtectedStaticExecutableOwnerV1 as Owner,
    ProtectedStaticExecutableV1 as Image,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_runtime_protocol::{
    SEALED_STATIC_APPLICATION_WORKSPACE_BYTES_V1, sealed_static_application_work_bound_v1,
};
use std::{error::Error, fmt, fs::File, mem::size_of};

const ENTRY_WORK: usize = 8;
type Result<T> = std::result::Result<T, ProtectedStaticExecutableErrorV2>;

/// Operation whose logical resource envelope is queried before admission.
#[derive(Clone, Copy, Debug)]
pub enum ProtectedStaticExecutableOperationV2 {
    /// Freshly seal a provisioned source or admit an existing sealed descriptor.
    Admit,
    /// Recheck one retained executable image.
    Revalidate,
    /// Duplicate or revalidate a transferred executable descriptor.
    Transfer,
}

/// Logical work/scratch requirement, not an allocation, RSS, generated-stack,
/// syscall latency or instruction bound. Input owners remain separately prepaid.
#[derive(Clone, Copy, Debug)]
pub struct ProtectedStaticExecutableQuotaV2 {
    work: usize,
    scratch: usize,
}
impl ProtectedStaticExecutableQuotaV2 {
    /// Full work charge including entry admission; accepted work is never refunded.
    pub const fn work(self) -> usize {
        self.work
    }
    /// Additional peak logical scratch above the complete input reservation.
    pub const fn scratch(self) -> usize {
        self.scratch
    }
}

/// Additional unreserved retained charge returned by a native image operation.
#[derive(Clone, Copy, Debug)]
pub struct ProtectedStaticExecutableStorageV2(usize);
impl ProtectedStaticExecutableStorageV2 {
    /// Preserve consumed inputs' reservations and reserve this delta before retention.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Move-only native executable custody with finite-attempt I/O on a caller ledger.
/// It admits the shared sealed-static image contract, not an issuer policy,
/// protected process, readiness, signing, publication, load or GPU launch.
/// There is no conversion from an admitted legacy owner.
///
/// Every operation restores entry storage on success, failure and unwind.
/// Retire consumed-input reservations only after a consuming failure returns;
/// after success retain those reservations plus the returned delta. Retire the
/// full retained charge only after dropping/transferring its owner.
///
/// ```compile_fail
/// use fe2o3_protected_static_executable::ProtectedStaticExecutableV2;
/// fn duplicate(image: ProtectedStaticExecutableV2) { let _=image.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_protected_static_executable::ProtectedStaticExecutableV2;
/// fn descriptor<T:std::os::fd::AsFd>() {}
/// descriptor::<ProtectedStaticExecutableV2>();
/// ```
pub struct ProtectedStaticExecutableV2(Image);
impl ProtectedStaticExecutableV2 {
    /// Constant-time quota query over a trusted measurement; no I/O or image traversal.
    pub fn quota(
        measurement: Measurement,
        operation: ProtectedStaticExecutableOperationV2,
    ) -> Result<ProtectedStaticExecutableQuotaV2> {
        let n = usize::try_from(measurement.byte_len()).map_err(|_| Resource::Arithmetic)?;
        let passes = match operation {
            ProtectedStaticExecutableOperationV2::Admit => 4,
            ProtectedStaticExecutableOperationV2::Revalidate => 2,
            ProtectedStaticExecutableOperationV2::Transfer => 3,
        };
        // Each pass reads/zeroes and hashes raw and domain-separated bytes.
        // The shared parser owns its header/table/byte-visit bound. At most 128
        // direct descriptor operations (including cleanup) are weighted at 1024.
        let work = sealed_static_application_work_bound_v1(n)
            .and_then(|v| v.checked_add(n.checked_mul(4)?))
            .and_then(|v| v.checked_mul(passes))
            .and_then(|v| v.checked_add(n))
            .and_then(|v| v.checked_add(ENTRY_WORK + 128 * 1024))
            .ok_or(Resource::Arithmetic)?;
        // Two read buffers, each with capacity <=2*N, may coexist with the new
        // N-byte backing image. Allocator implementation overhead is not included.
        let scratch = n
            .checked_mul(5)
            .and_then(|v| {
                v.checked_add(
                    SEALED_STATIC_APPLICATION_WORKSPACE_BYTES_V1
                        + 4 * size_of::<(Self, ProtectedStaticExecutableStorageV2)>()
                        + 4096,
                )
            })
            .ok_or(Resource::Arithmetic)?;
        Ok(ProtectedStaticExecutableQuotaV2 { work, scratch })
    }

    /// Full incoming/outgoing File plus logical image charge, even for a shared inode.
    pub fn file_storage(measurement: Measurement) -> Result<usize> {
        usize::try_from(measurement.byte_len())
            .ok()
            .and_then(|n| n.checked_add(size_of::<(File, ProtectedStaticExecutableStorageV2)>()))
            .ok_or(Resource::Arithmetic.into())
    }

    /// Consumes a prepaid source; seals independently owned bytes for the requested owner.
    pub fn seal_source_for_owner(
        source: File,
        measurement: Measurement,
        owner: Owner,
        role: &'static str,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, ProtectedStaticExecutableStorageV2)> {
        Self::admit_scope(measurement, budget, || {
            Image::seal_source_with::<true>(source, measurement, owner, role)
        })
    }

    /// Consumes a prepaid sealed descriptor under the exact shared image contract.
    pub fn admit_sealed(
        image: File,
        measurement: Measurement,
        owner: Owner,
        role: &'static str,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, ProtectedStaticExecutableStorageV2)> {
        Self::admit_scope(measurement, budget, || {
            Image::admit_sealed_with::<true>(image, measurement, owner, role)
        })
    }

    fn admit_scope(
        measurement: Measurement,
        budget: &mut Budget<'_>,
        admit: impl FnOnce() -> std::result::Result<Image, ImageError>,
    ) -> Result<(Self, ProtectedStaticExecutableStorageV2)> {
        budget.charge_work(ENTRY_WORK)?;
        let floor = Self::file_storage(measurement)?;
        Self::scope(
            measurement,
            ProtectedStaticExecutableOperationV2::Admit,
            floor,
            budget,
            || {
                let image = Self(admit()?);
                let delta = image.retained_storage() - floor;
                Ok((image, ProtectedStaticExecutableStorageV2(delta)))
            },
        )
    }

    fn scope<T>(
        measurement: Measurement,
        operation: ProtectedStaticExecutableOperationV2,
        floor: usize,
        budget: &mut Budget<'_>,
        action: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let quota = Self::quota(measurement, operation)?;
        budget.with_prepaid_scope(floor, 0, quota.work - ENTRY_WORK, quota.scratch, |_| {
            action()
        })
    }

    /// Rechecks exact kernel object, mutable flags/metadata, content and static ELF form.
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(ENTRY_WORK)?;
        Self::scope(
            self.measurement(),
            ProtectedStaticExecutableOperationV2::Revalidate,
            self.retained_storage(),
            budget,
            || Ok(self.0.revalidate_with::<true>()?),
        )
    }

    /// Returns a separately prepaid CLOEXEC clone for a consuming launcher.
    pub fn try_clone_for_exec(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(File, ProtectedStaticExecutableStorageV2)> {
        budget.charge_work(ENTRY_WORK)?;
        Self::scope(
            self.measurement(),
            ProtectedStaticExecutableOperationV2::Transfer,
            self.retained_storage(),
            budget,
            || {
                Ok((
                    self.0.try_clone_with::<true>()?,
                    ProtectedStaticExecutableStorageV2(Self::file_storage(self.measurement())?),
                ))
            },
        )
    }

    /// Requires the transferred File to be the exact retained sealed object, not
    /// merely equal bytes. Both owner and File/image charges must remain prepaid.
    pub fn revalidate_exec_clone(&self, image: &File, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(ENTRY_WORK)?;
        let floor = self
            .retained_storage()
            .checked_add(Self::file_storage(self.measurement())?)
            .ok_or(Resource::Arithmetic)?;
        Self::scope(
            self.measurement(),
            ProtectedStaticExecutableOperationV2::Transfer,
            floor,
            budget,
            || Ok(self.0.revalidate_clone_with::<true>(image)?),
        )
    }

    /// Exact measurement admitted at construction.
    pub const fn measurement(&self) -> Measurement {
        self.0.measurement()
    }
    /// Loader-independent content identity, not policy or execution authority.
    pub const fn static_identity(&self) -> [u8; 32] {
        self.0.static_identity()
    }
    /// Inert retained kernel object identity, without descriptor access.
    pub const fn object_identity(&self) -> Object {
        self.0.object_identity()
    }
    /// Full retained logical charge; excludes unrelated callers' live owners.
    pub fn retained_storage(&self) -> usize {
        // Every admitted image already passed the stronger checked quota arithmetic.
        size_of::<(Self, ProtectedStaticExecutableStorageV2)>()
            + self.measurement().byte_len() as usize
    }
}

impl fmt::Debug for ProtectedStaticExecutableV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProtectedStaticExecutableV2")
            .field("authority", &"none")
            .field("measurement", &self.measurement())
            .field("object", &self.object_identity())
            .finish_non_exhaustive()
    }
}

/// Failure of native executable accounting or shared sealed-image admission.
#[derive(Debug)]
pub enum ProtectedStaticExecutableErrorV2 {
    /// The caller ledger or checked quota arithmetic refused this operation.
    Resource(Resource),
    /// The shared image contract, finite I/O or fallible buffer allocation refused.
    Image(ImageError),
}
impl From<Resource> for ProtectedStaticExecutableErrorV2 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<ImageError> for ProtectedStaticExecutableErrorV2 {
    fn from(e: ImageError) -> Self {
        Self::Image(e)
    }
}
impl fmt::Display for ProtectedStaticExecutableErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Image(e) => e.fmt(f),
        }
    }
}
impl Error for ProtectedStaticExecutableErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Resource(e) => e,
            Self::Image(e) => e,
        })
    }
}

const _: () = {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as LedgerIdentity;
    const fn envelope<T>() -> usize {
        size_of::<Result<T>>().saturating_sub(size_of::<T>())
            + size_of::<std::thread::Result<Result<T>>>().saturating_sub(size_of::<T>())
    }
    assert!(
        size_of::<(
            ProtectedStaticExecutableV2,
            ProtectedStaticExecutableStorageV2
        )>() >= size_of::<(File, ProtectedStaticExecutableStorageV2)>()
    );
    assert!(
        8 * size_of::<ProtectedStaticExecutableErrorV2>()
            + 64 * size_of::<usize>()
            + 4 * size_of::<std::fs::Metadata>()
            + 2 * size_of::<sha2::Sha256>()
            + size_of::<LedgerIdentity>()
            + size_of::<std::result::Result<(), Resource>>()
            + 2 * size_of::<bool>()
            + 512
            + envelope::<(
                ProtectedStaticExecutableV2,
                ProtectedStaticExecutableStorageV2
            )>()
            + envelope::<(File, ProtectedStaticExecutableStorageV2)>()
            + envelope::<()>()
            <= 4096
    );
};
