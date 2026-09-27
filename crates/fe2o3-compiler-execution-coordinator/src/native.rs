//! Policy-neutral native preparation mechanics, not deployment provenance or launch authority.
use crate::{
    CompilerExecutionSupervisorProgramSourcesV1 as Sources,
    native_trust_adapter::CompilerExecutionSupervisorTrustErrorV2 as TrustError,
};
use fe2o3_compiler_execution_lifecycle::{
    CompilerExecutionServiceLifecycleLeaseV2 as Lease, LifecycleLeaseErrorV2 as LifecycleError,
};
use fe2o3_compiler_execution_supervisor::{
    IssuerServiceCredentialProfileErrorV1 as CredentialsError,
    IssuerServiceCredentialProfileV1 as Credentials,
    ProtectedIssuerServiceProvisioningErrorV2 as InputsError,
};
use fe2o3_external_anchor_coordinator::ExternalAnchorLaunchErrorV2 as AnchorError;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_profile::ProtectedServiceProfileErrorV2 as ProfileError;
use fe2o3_protected_service_spawn::{
    ProtectedServiceCleanupErrorV2 as CleanupError, ProtectedServiceCleanupServiceV2 as Cleanup,
};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV2 as ImageError,
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOwnerV1 as Owner, ProtectedStaticExecutableStorageV2 as ImageStorage,
    ProtectedStaticExecutableV2 as Image,
};
use std::{error::Error, fmt, fs::File, mem::size_of};

pub(crate) type Result<T> = std::result::Result<T, CompilerExecutionPreparationErrorV2>;
pub(crate) const ENTRY: usize = 8;
// Fixed credential/PID/owner checks and descriptor retirement; nested work is separate.
pub(crate) const LOCAL_WORK: usize = ENTRY + 64 * 1024;
pub(crate) const IMAGE_GROWTH: usize =
    size_of::<(Image, ImageStorage)>() - size_of::<(File, ImageStorage)>();

/// Additional unreserved growth above every consumed native input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionPreparationStorageV2(pub(crate) usize);
impl CompilerExecutionPreparationStorageV2 {
    /// Reserve before retaining the result, keeping consumed input charges live.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Complete logical work and extra peak above the full input-owner reservation.
/// Not a bound on elapsed time, instructions, generated stack or process RSS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionPreparationQuotaV2 {
    pub(crate) work: usize,
    pub(crate) scratch: usize,
}
impl CompilerExecutionPreparationQuotaV2 {
    /// Includes all nested native checks on the original ledger.
    pub const fn work(self) -> usize {
        self.work
    }
    /// Includes simultaneous retained growth, local frame and nested scratch.
    pub const fn scratch(self) -> usize {
        self.scratch
    }
}

/// Fixed-shape native preparation refusal; no owned diagnostic strings.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionPreparationErrorV2 {
    /// Original account or checked arithmetic refused.
    Resource(Resource),
    /// Native deployment, policy or signing-key binding refused.
    Trust(TrustError),
    /// Native static executable admission or continuity refused.
    Executable(ImageError),
    /// Exact listener/root admission or continuity refused.
    ServiceInputs(InputsError),
    /// Native managed anchor continuity or context refused.
    Anchor(AnchorError),
    /// The existing cleanup pool guard could not be borrowed for validation.
    Cleanup(CleanupError),
    /// The cleanup guard does not validate against the actual retained lifecycle.
    Lifecycle(LifecycleError),
    /// Native namespace observation refused.
    Profile(ProfileError),
    /// Dedicated service credentials are invalid.
    Credentials(CredentialsError),
    /// Exact real/effective/saved/filesystem root credentials are required.
    RootRequired,
    /// The preparing process identity changed.
    CoordinatorChanged,
    /// The service inputs do not match the actual deployment credentials.
    ServiceIdentityMismatch,
    /// A retained image does not match the actual native context and target owner.
    ExecutableBindingMismatch,
}
use CompilerExecutionPreparationErrorV2 as Failure;
macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for Failure {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(Resource, Resource);
from_error!(TrustError, Trust);
from_error!(ImageError, Executable);
from_error!(InputsError, ServiceInputs);
from_error!(AnchorError, Anchor);
from_error!(CleanupError, Cleanup);
from_error!(LifecycleError, Lifecycle);
from_error!(ProfileError, Profile);
from_error!(CredentialsError, Credentials);
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Trust(e) => e.fmt(f),
            Self::Executable(e) => e.fmt(f),
            Self::ServiceInputs(e) => e.fmt(f),
            Self::Anchor(e) => e.fmt(f),
            Self::Cleanup(e) => e.fmt(f),
            Self::Lifecycle(e) => e.fmt(f),
            Self::Profile(e) => e.fmt(f),
            Self::Credentials(e) => e.fmt(f),
            Self::RootRequired => {
                f.write_str("native compiler coordinator requires exact root identity")
            }
            Self::CoordinatorChanged => f.write_str("native compiler coordinator PID changed"),
            Self::ServiceIdentityMismatch => {
                f.write_str("native compiler service inputs have another owner")
            }
            Self::ExecutableBindingMismatch => {
                f.write_str("native compiler executable binding changed")
            }
        }
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Trust(e) => Some(e),
            Self::Executable(e) => Some(e),
            Self::ServiceInputs(e) => Some(e),
            Self::Anchor(e) => Some(e),
            Self::Cleanup(e) => Some(e),
            Self::Lifecycle(e) => Some(e),
            Self::Profile(e) => Some(e),
            Self::Credentials(e) => Some(e),
            _ => None,
        }
    }
}

pub(crate) fn sum(values: &[usize]) -> Result<usize> {
    values.iter().try_fold(0usize, |sum, n| {
        sum.checked_add(*n).ok_or(Resource::Arithmetic.into())
    })
}
pub(crate) const fn maximum(values: &[usize]) -> usize {
    let mut maximum = 0;
    let mut i = 0;
    while i < values.len() {
        if values[i] > maximum {
            maximum = values[i];
        }
        i += 1;
    }
    maximum
}

pub(crate) fn cleanup_guard_quota(
    revalidation: CompilerExecutionPreparationQuotaV2,
    frame: usize,
) -> Result<CompilerExecutionPreparationQuotaV2> {
    Ok(CompilerExecutionPreparationQuotaV2 {
        work: sum(&[
            LOCAL_WORK,
            revalidation.work(),
            Cleanup::GUARD_CLONE_WORK,
            Lease::TRANSFER_WORK,
        ])?,
        scratch: sum(&[
            frame,
            maximum(&[
                revalidation.scratch(),
                sum(&[Cleanup::GUARD_CLONE_SCRATCH, Cleanup::GUARD_FILE_STORAGE])?,
                sum(&[Cleanup::GUARD_FILE_STORAGE, Lease::IO_STORAGE])?,
            ]),
        ])?,
    })
}

// The family adapter binds both checks to the same actual Prepared. Keeping the
// accounting schedule separate lets tests exercise custody without fabricating one.
pub(crate) fn with_cleanup_guard(
    floor: usize,
    frame: usize,
    cleanup: &mut Cleanup,
    budget: &mut Budget<'_>,
    revalidate: impl FnOnce(&mut Budget<'_>) -> Result<()>,
    validate: impl FnOnce(&File, &mut Budget<'_>) -> Result<()>,
) -> Result<()> {
    budget.with_prepaid_scope(floor, ENTRY, LOCAL_WORK, frame, |b| {
        revalidate(b)?;
        let (alias, charge) = cleanup.try_clone_deployment_guard(b)?;
        b.reserve_storage(charge.additional_storage())?;
        validate(&alias, b)?;
        drop(alias);
        b.release_storage(charge.additional_storage())?;
        Ok(())
    })
}

pub(crate) fn require_root() -> Result<()> {
    fe2o3_protected_service_spawn::require_exact_root_identity_v1()
        .map_err(|_| Failure::RootRequired)
}
pub(crate) fn measurement(
    m: fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1,
    limit: u64,
) -> Result<Measurement> {
    Measurement::new(m.sha256(), m.byte_len(), limit).map_err(|e| ImageError::from(e).into())
}

// The enclosing scope prepays all three complete sources. Reserve each native
// image's growth immediately; any later failure drops earlier images before rollback.
pub(crate) fn seal_programs(
    sources: Sources,
    expected: [Measurement; 3],
    owner: Owner,
    budget: &mut Budget<'_>,
) -> Result<[Image; 3]> {
    let mut seal = |source, measurement, role| -> Result<Image> {
        let (image, delta) =
            Image::seal_source_for_owner(source, measurement, owner, role, budget)?;
        if delta.additional_storage() != IMAGE_GROWTH {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(delta.additional_storage())?;
        Ok(image)
    };
    Ok([
        seal(
            sources.supervisor,
            expected[0],
            "native compiler supervisor",
        )?,
        seal(
            sources.launcher,
            expected[1],
            "native compiler static launcher",
        )?,
        seal(sources.issuer, expected[2], "native compiler issuer")?,
    ])
}
pub(crate) fn check_programs(
    images: &[Image; 3],
    expected: [Measurement; 3],
    credentials: Credentials,
    budget: &mut Budget<'_>,
) -> Result<()> {
    for (image, expected) in images.iter().zip(expected) {
        image.revalidate(budget)?;
        let object = image.object_identity();
        if image.measurement() != expected
            || object.uid() != credentials.uid()
            || object.gid() != credentials.gid()
        {
            return Err(Failure::ExecutableBindingMismatch);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
