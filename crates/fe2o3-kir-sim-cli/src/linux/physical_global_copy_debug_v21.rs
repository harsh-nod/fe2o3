//! Bounded V21 diagnostic input ownership; bytes grant no source or launch authority.
use super::physical_debug_input_common as common;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV21 as CanonicalError,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, KernelIrDecodeError,
    KernelIrEncodeError, VerifiedCanonicalKernelIrModuleV21 as Owner,
};
use std::mem::size_of;
pub const MAX_PHYSICAL_GLOBAL_COPY_DEBUG_KIR_BYTES_V21: usize = 128 * 1024;
pub const MAX_PHYSICAL_GLOBAL_COPY_DEBUG_REQUEST_BYTES_V21: usize = 16 * 1024;
/// Move-only input ownership retained while the typed capture is constructed.
pub struct PhysicalGlobalCopyDebugInputV21 {
    canonical: Owner,
    module: AdmittedSimulationModuleV1,
    request: SimulationRequestV1,
    limits: SimulationLimitsV1,
    request_digest: [u8; 32],
    request_bytes: usize,
}
impl PhysicalGlobalCopyDebugInputV21 {
    pub fn canonical(&self) -> &Owner {
        &self.canonical
    }
    pub fn module(&self) -> &AdmittedSimulationModuleV1 {
        &self.module
    }
    pub fn request(&self) -> &SimulationRequestV1 {
        &self.request
    }
    pub fn limits(&self) -> SimulationLimitsV1 {
        self.limits
    }
    pub const fn request_digest(&self) -> &[u8; 32] {
        &self.request_digest
    }
    pub const fn request_bytes(&self) -> usize {
        self.request_bytes
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalGlobalCopyDebugInputStorageV21 {
    retained: usize,
}
impl PhysicalGlobalCopyDebugInputStorageV21 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalGlobalCopyDebugInputErrorV21 {
    Platform,
    Input,
    Request,
    WrongVersion,
    Canonical,
    Profile,
    Admission,
    Resource(Resource),
}
impl PhysicalGlobalCopyDebugInputErrorV21 {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Platform => "kir_v21_debug_platform_unavailable",
            Self::Input => "kir_v21_debug_input_refused",
            Self::Request => "kir_v21_debug_request_refused",
            Self::WrongVersion => "kir_v21_debug_wrong_version",
            Self::Canonical => "kir_v21_debug_canonical_refused",
            Self::Profile => "kir_v21_debug_profile_refused",
            Self::Admission => "kir_v21_debug_cpu_admission_refused",
            Self::Resource(Resource::Work(_)) => "kir_v21_debug_work_limit",
            Self::Resource(Resource::Storage(_)) => "kir_v21_debug_storage_limit",
            Self::Resource(Resource::Allocation) => "kir_v21_debug_allocation_failed",
            Self::Resource(Resource::Arithmetic) => "kir_v21_debug_resource_arithmetic",
            Self::Resource(Resource::Accounting) => "kir_v21_debug_resource_accounting",
        }
    }
}
impl From<Resource> for PhysicalGlobalCopyDebugInputErrorV21 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
type Error = PhysicalGlobalCopyDebugInputErrorV21;
fn canonical_error(error: CanonicalError) -> Error {
    match error {
        CanonicalError::Resource(r) | CanonicalError::Decode(KernelIrDecodeError::Resource(r)) => {
            Error::Resource(r)
        }
        CanonicalError::Decode(KernelIrDecodeError::WorkLimit(w))
        | CanonicalError::Decode(KernelIrDecodeError::Encode(KernelIrEncodeError::WorkLimit(w)))
        | CanonicalError::Encode(KernelIrEncodeError::WorkLimit(w)) => {
            Error::Resource(Resource::Work(w))
        }
        CanonicalError::Decode(KernelIrDecodeError::UnknownVersion(_)) => Error::WrongVersion,
        _ => Error::Canonical,
    }
}
fn simulation_error(error: fe2o3_kir_sim::PhysicalGlobalCopySimulationAdmissionErrorV21) -> Error {
    use fe2o3_kir_sim::PhysicalGlobalCopySimulationAdmissionErrorV21 as E;
    match error {
        E::Resource(r) => Error::Resource(r),
        E::CanonicalView(e) => canonical_error(e),
        E::Profile => Error::Profile,
        E::Admission(_) => Error::Admission,
    }
}
fn limits() -> SimulationLimitsV1 {
    common::limits()
}
fn input_envelope() -> Result<usize, Resource> {
    common::envelope(size_of::<PhysicalGlobalCopyDebugInputV21>())
}
pub fn load_physical_global_copy_debug_input_v21(
    kir: &Path,
    request: &Path,
    budget: &mut Budget<'_>,
) -> Result<
    (
        PhysicalGlobalCopyDebugInputV21,
        PhysicalGlobalCopyDebugInputStorageV21,
    ),
    Error,
> {
    let floor = budget.storage();
    budget.with_prepaid_scope(floor, 0, 0, 0, |budget| {
        budget.reserve_storage(input_envelope()?)?;
        budget.charge_work(
            MAX_PHYSICAL_GLOBAL_COPY_DEBUG_KIR_BYTES_V21
                + MAX_PHYSICAL_GLOBAL_COPY_DEBUG_REQUEST_BYTES_V21 * 64,
        )?;
        let (kir, request) = common::read_inputs(kir, request, common::Profile::GlobalCopyV21)
            .map_err(|_| Error::Input)?;
        let input = load_bytes(&kir, &request, budget)?;
        let retained = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?;
        Ok((input, PhysicalGlobalCopyDebugInputStorageV21 { retained }))
    })
}
fn load_bytes(
    kir: &[u8],
    request: &[u8],
    budget: &mut Budget<'_>,
) -> Result<PhysicalGlobalCopyDebugInputV21, Error> {
    if kir.len() > MAX_PHYSICAL_GLOBAL_COPY_DEBUG_KIR_BYTES_V21
        || request.len() > MAX_PHYSICAL_GLOBAL_COPY_DEBUG_REQUEST_BYTES_V21
    {
        return Err(Error::Input);
    }
    let (canonical, owner_receipt) =
        Owner::from_canonical_bytes_with_verification_budget_v21(kir, budget)
            .map_err(canonical_error)?;
    budget.reserve_storage(owner_receipt.retained_storage())?;
    let limits = limits();
    let (module, view_receipt) =
        AdmittedSimulationModuleV1::admit_v21_with_verification_budget(&canonical, limits, budget)
            .map_err(simulation_error)?;
    budget.reserve_storage(view_receipt.retained_storage())?;
    let parsed = common::parse_request(request, common::Profile::GlobalCopyV21)
        .map_err(|_| Error::Request)?;
    Ok(PhysicalGlobalCopyDebugInputV21 {
        canonical,
        module,
        request: parsed,
        limits,
        request_digest: Sha256::digest(request).into(),
        request_bytes: request.len(),
    })
}
