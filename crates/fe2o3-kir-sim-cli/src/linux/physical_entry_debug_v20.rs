//! Bounded V20 diagnostic input ownership; bytes grant no source or launch authority.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV20 as CanonicalError,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, KernelIrDecodeError,
    KernelIrEncodeError, VerifiedCanonicalKernelIrModuleV20 as Owner,
};
use std::mem::size_of;
pub const MAX_PHYSICAL_DEBUG_KIR_BYTES_V20: usize = 128 * 1024;
pub const MAX_PHYSICAL_DEBUG_REQUEST_BYTES_V20: usize = 16 * 1024;
/// Move-only input ownership retained while the typed capture is constructed.
pub struct PhysicalEntryDebugInputV20 {
    canonical: Owner,
    module: AdmittedSimulationModuleV1,
    request: SimulationRequestV1,
    limits: SimulationLimitsV1,
    request_digest: [u8; 32],
    request_bytes: usize,
}
impl PhysicalEntryDebugInputV20 {
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
pub struct PhysicalEntryDebugInputStorageV20 {
    retained: usize,
}
impl PhysicalEntryDebugInputStorageV20 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalEntryDebugInputErrorV20 {
    Platform,
    Input,
    Request,
    WrongVersion,
    Canonical,
    Profile,
    Admission,
    Resource(Resource),
}
impl PhysicalEntryDebugInputErrorV20 {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Platform => "kir_v20_debug_platform_unavailable",
            Self::Input => "kir_v20_debug_input_refused",
            Self::Request => "kir_v20_debug_request_refused",
            Self::WrongVersion => "kir_v20_debug_wrong_version",
            Self::Canonical => "kir_v20_debug_canonical_refused",
            Self::Profile => "kir_v20_debug_profile_refused",
            Self::Admission => "kir_v20_debug_cpu_admission_refused",
            Self::Resource(Resource::Work(_)) => "kir_v20_debug_work_limit",
            Self::Resource(Resource::Storage(_)) => "kir_v20_debug_storage_limit",
            Self::Resource(Resource::Allocation) => "kir_v20_debug_allocation_failed",
            Self::Resource(Resource::Arithmetic) => "kir_v20_debug_resource_arithmetic",
            Self::Resource(Resource::Accounting) => "kir_v20_debug_resource_accounting",
        }
    }
}
impl From<Resource> for PhysicalEntryDebugInputErrorV20 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
type Error = PhysicalEntryDebugInputErrorV20;
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
fn simulation_error(error: fe2o3_kir_sim::PhysicalEntrySimulationAdmissionErrorV20) -> Error {
    use fe2o3_kir_sim::PhysicalEntrySimulationAdmissionErrorV20 as E;
    match error {
        E::Resource(r) => Error::Resource(r),
        E::CanonicalView(e) => canonical_error(e),
        E::Profile => Error::Profile,
        E::Admission(_) => Error::Admission,
    }
}
fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: MAX_PHYSICAL_DEBUG_KIR_BYTES_V20,
        max_reachable_functions: 1,
        max_reachable_operations: 128,
        max_invocations: 128,
        max_workgroups: 2,
        max_scheduled_slots: 128,
        max_steps: 32_768,
        max_call_depth: 1,
        max_ssa_values: 768,
        max_allocations: 8,
        max_allocation_bytes: 4096,
        max_total_bytes: 8192,
        max_resident_bytes: 64 * 1024 * 1024,
        max_events: 1,
        max_memory_access_records: 1024,
    }
}
/// Conservative input/parser/materializer allowance. Every decoded list element
/// consumes at least one source byte; the factor covers geometric Vec capacity,
/// simultaneous source/destination lists and strings/decoded bytes/initialization.
/// Keep this allowance, rather than pretending an exact post-parse receipt.
fn input_envelope() -> Result<usize, Resource> {
    let cells = size_of::<RequestArgument>()
        + size_of::<RequestSharedBuffer>()
        + size_of::<SimulationArgumentV1>()
        + size_of::<SharedBufferV1>()
        + 128;
    MAX_PHYSICAL_DEBUG_REQUEST_BYTES_V20
        .checked_mul(cells)
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| n.checked_add(2 * (MAX_PHYSICAL_DEBUG_KIR_BYTES_V20 + 1)))
        .and_then(|n| n.checked_add(2 * (MAX_PHYSICAL_DEBUG_REQUEST_BYTES_V20 + 1)))
        .and_then(|n| n.checked_add(size_of::<PhysicalEntryDebugInputV20>() + 128 * 1024))
        .ok_or(Resource::Arithmetic)
}
pub fn load_physical_entry_debug_input_v20(
    kir: &Path,
    request: &Path,
    budget: &mut Budget<'_>,
) -> Result<
    (
        PhysicalEntryDebugInputV20,
        PhysicalEntryDebugInputStorageV20,
    ),
    Error,
> {
    let floor = budget.storage();
    budget.with_prepaid_scope(floor, 0, 0, 0, |budget| {
        budget.reserve_storage(input_envelope()?)?;
        budget.charge_work(
            MAX_PHYSICAL_DEBUG_KIR_BYTES_V20 + MAX_PHYSICAL_DEBUG_REQUEST_BYTES_V20 * 64,
        )?;
        let kir = secure_read(
            kir,
            MAX_PHYSICAL_DEBUG_KIR_BYTES_V20,
            InputCode::KirV20,
            "diagnostic physical-entry KIR V20",
        )
        .map_err(|_| Error::Input)?;
        let request = secure_read(
            request,
            MAX_PHYSICAL_DEBUG_REQUEST_BYTES_V20,
            InputCode::Request,
            "physical-entry simulation request",
        )
        .map_err(|_| Error::Input)?;
        let input = load_bytes(&kir, &request, budget)?;
        let retained = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?;
        Ok((input, PhysicalEntryDebugInputStorageV20 { retained }))
    })
}
fn load_bytes(
    kir: &[u8],
    request: &[u8],
    budget: &mut Budget<'_>,
) -> Result<PhysicalEntryDebugInputV20, Error> {
    if kir.len() > MAX_PHYSICAL_DEBUG_KIR_BYTES_V20
        || request.len() > MAX_PHYSICAL_DEBUG_REQUEST_BYTES_V20
    {
        return Err(Error::Input);
    }
    let (canonical, owner_receipt) =
        Owner::from_canonical_bytes_with_verification_budget_v20(kir, budget)
            .map_err(canonical_error)?;
    budget.reserve_storage(owner_receipt.retained_storage())?;
    let limits = limits();
    let (module, view_receipt) =
        AdmittedSimulationModuleV1::admit_v20_with_verification_budget(&canonical, limits, budget)
            .map_err(simulation_error)?;
    budget.reserve_storage(view_receipt.retained_storage())?;
    let document: RequestDocument = serde_json::from_slice(request).map_err(|_| Error::Request)?;
    let parsed = prepare_request(document).map_err(|_| Error::Request)?;
    // This is a bounded CPU tooling profile, not a change to the source contract.
    if parsed.arguments.len() != 5
        || parsed.shared_buffers.len() > 1
        || parsed.grid.0[0] > 128
        || parsed.grid.0[1..] != [1, 1]
        || parsed.workgroup.0 != [64, 1, 1]
    {
        return Err(Error::Request);
    }
    Ok(PhysicalEntryDebugInputV20 {
        canonical,
        module,
        request: parsed,
        limits,
        request_digest: Sha256::digest(request).into(),
        request_bytes: request.len(),
    })
}
