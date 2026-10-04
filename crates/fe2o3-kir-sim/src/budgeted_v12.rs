//! Additive metered CPU view of the exact borrowed V12 owner, not source custody.
use crate::{
    AdmittedSimulationModuleV1, SimulationAdmissionErrorV1, SimulationKernelIrIdentityV1,
    SimulationLimitsV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV12,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Module,
    VerifiedCanonicalKernelIrModuleV12,
};
use std::{error::Error, fmt, mem::size_of};

#[derive(Debug)]
pub enum SimulationViewAdmissionErrorV12 {
    Resource(Resource),
    CanonicalView(CanonicalKernelIrReplayAdmissionErrorV12),
    Admission(SimulationAdmissionErrorV1),
}
impl From<Resource> for SimulationViewAdmissionErrorV12 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<SimulationAdmissionErrorV1> for SimulationViewAdmissionErrorV12 {
    fn from(value: SimulationAdmissionErrorV1) -> Self {
        Self::Admission(value)
    }
}
impl fmt::Display for SimulationViewAdmissionErrorV12 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::CanonicalView(e) => e.fmt(f),
            Self::Admission(e) => e.fmt(f),
        }
    }
}
impl Error for SimulationViewAdmissionErrorV12 {}

/// Reserve while the returned independent view lives; drop the view before release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationViewStorageV12 {
    retained: usize,
}
impl SimulationViewStorageV12 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

impl AdmittedSimulationModuleV1 {
    /// Copies and admits this exact immutable owner on the original caller ledger.
    /// No mutable intermediate escapes, and neither source nor deployment authority
    /// is created. Preserve the original owner's reservation throughout this call.
    ///
    /// Every Result exit restores the incoming storage floor, preserving work,
    /// peak and first-denial history. Reserve the returned receipt before further
    /// allocation and keep it reserved while the CPU view lives. Decoder and
    /// resident charges are logical payloads, not allocator metadata or RSS.
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::{Module, CanonicalKernelIrVerificationResourceBudgetV1};
    /// use fe2o3_kir_sim::{AdmittedSimulationModuleV1, SimulationLimitsV1};
    /// fn raw_is_not_authority(module: &Module, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
    ///     AdmittedSimulationModuleV1::admit_v12_with_verification_budget(module, SimulationLimitsV1::default(), budget);
    /// }
    /// ```
    pub fn admit_v12_with_verification_budget(
        owner: &VerifiedCanonicalKernelIrModuleV12,
        limits: SimulationLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, SimulationViewStorageV12), SimulationViewAdmissionErrorV12> {
        budget.charge_work(1)?;
        let limits = limits
            .validate()
            .map_err(SimulationAdmissionErrorV1::InvalidLimits)?;
        let canonical = owner.canonical();
        let bytes = canonical.canonical_bytes();
        if bytes.len() > limits.max_canonical_bytes {
            return Err(SimulationAdmissionErrorV1::CanonicalBytesLimit {
                actual: bytes.len(),
                limit: limits.max_canonical_bytes,
            }
            .into());
        }
        let floor = budget.storage();
        budget.with_prepaid_scope(floor, 0, 0, 0, |budget| {
            let header = size_of::<Self>()
                .checked_sub(size_of::<Module>())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(header)?;
            let (module, copy) = owner
                .copy_module_for_transformation_v12(budget)
                .map_err(SimulationViewAdmissionErrorV12::CanonicalView)?;
            budget.reserve_storage(copy.retained_storage())?;
            // Resident traversal visits each owned node/edge once and does not scan
            // string contents. Each such element has at least one injective wire
            // byte; 8 units/byte also covers fixed per-container visit overhead.
            // Decoder/equality work has already been charged, without resetting.
            budget.charge_work(bytes.len().checked_mul(8).ok_or(Resource::Arithmetic)?)?;
            let resident = size_of::<Self>()
                .checked_add(
                    crate::resident::module_retained_heap_bytes(&module)
                        .ok_or(Resource::Arithmetic)?,
                )
                .ok_or(Resource::Arithmetic)?;
            let decoded = header
                .checked_add(copy.retained_storage())
                .ok_or(Resource::Arithmetic)?;
            let retained = decoded.max(resident);
            budget.reserve_storage(retained - decoded)?;
            let with_input = retained
                .checked_add(bytes.len())
                .ok_or(Resource::Arithmetic)?;
            if with_input > limits.max_resident_bytes {
                return Err(SimulationAdmissionErrorV1::ResidentBytesLimit {
                    phase: "post-decode V12 same-owner view and borrowed input",
                    actual: with_input,
                    limit: limits.max_resident_bytes,
                }
                .into());
            }
            Ok((
                Self {
                    identity: SimulationKernelIrIdentityV1::from(*canonical.identity()),
                    module,
                    admitted_resident_bytes: retained,
                },
                SimulationViewStorageV12 { retained },
            ))
        })
    }
}
