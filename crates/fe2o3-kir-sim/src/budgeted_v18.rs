//! Metered CPU view of exact V18 custody, retaining inert storage metadata.
use crate::{
    AdmittedSimulationModuleV1, SimulationAdmissionErrorV1, SimulationKernelIrIdentityV1,
    SimulationLimitsV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV18,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Module,
    VerifiedCanonicalKernelIrModuleV18,
};
use std::{error::Error, fmt, mem::size_of};

#[derive(Debug)]
pub enum SimulationViewAdmissionErrorV18 {
    Resource(Resource),
    CanonicalView(CanonicalKernelIrReplayAdmissionErrorV18),
    Admission(SimulationAdmissionErrorV1),
}
impl From<Resource> for SimulationViewAdmissionErrorV18 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<SimulationAdmissionErrorV1> for SimulationViewAdmissionErrorV18 {
    fn from(value: SimulationAdmissionErrorV1) -> Self {
        Self::Admission(value)
    }
}
impl fmt::Display for SimulationViewAdmissionErrorV18 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::CanonicalView(error) => error.fmt(f),
            Self::Admission(error) => error.fmt(f),
        }
    }
}
impl Error for SimulationViewAdmissionErrorV18 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Resource(error) => error,
            Self::CanonicalView(error) => error,
            Self::Admission(error) => error,
        })
    }
}

/// Logical retained payload, not source authority or a ledger-owned permit.
/// Reserve before further allocation; drop the returned view before release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationViewStorageV18 {
    retained: usize,
}
impl SimulationViewStorageV18 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

impl AdmittedSimulationModuleV1 {
    /// Admits an independent CPU view of this exact immutable V18 owner.
    ///
    /// The shared budgeted decoder compares the complete canonical encoding,
    /// retaining the storage table and explicit function roles. Inert metadata
    /// does not grant executable storage support: preflight still rejects storage
    /// operations and storage-object types. No legacy projection, source custody,
    /// deployment authority, or physical-register execution is created.
    ///
    /// Preserve the original owner's reservation throughout this call. All
    /// results and unwind restore the incoming storage floor while retaining
    /// work, peak and first-denial history on the original caller ledger.
    /// Reserve the returned receipt before further allocation and keep it until
    /// the view is dropped. Resident charges are logical payloads, not RSS;
    /// the simulation resident limit is checked after bounded decoding.
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::{Module, CanonicalKernelIrVerificationResourceBudgetV1};
    /// use fe2o3_kir_sim::{AdmittedSimulationModuleV1, SimulationLimitsV1};
    /// fn raw_is_not_authority(module: &Module, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
    ///     AdmittedSimulationModuleV1::admit_v18_with_verification_budget(module, SimulationLimitsV1::default(), budget);
    /// }
    /// ```
    pub fn admit_v18_with_verification_budget(
        owner: &VerifiedCanonicalKernelIrModuleV18,
        limits: SimulationLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, SimulationViewStorageV18), SimulationViewAdmissionErrorV18> {
        budget.charge_work(1)?;
        let limits = limits
            .validate()
            .map_err(SimulationAdmissionErrorV1::InvalidLimits)?;
        let bytes = owner.canonical_bytes();
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
                .copy_module_for_transformation_v18(budget)
                .map_err(SimulationViewAdmissionErrorV18::CanonicalView)?;
            budget.reserve_storage(copy.retained_storage())?;
            // Each visited node/edge has an injective wire byte. This also
            // prepays fixed container overhead without following layout IDs.
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
                    phase: "post-decode V18 same-owner view and borrowed input",
                    actual: with_input,
                    limit: limits.max_resident_bytes,
                }
                .into());
            }
            Ok((
                Self {
                    identity: SimulationKernelIrIdentityV1::from(*owner.identity()),
                    module,
                    admitted_resident_bytes: retained,
                },
                SimulationViewStorageV18 { retained },
            ))
        })
    }
}
