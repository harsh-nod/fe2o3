//! Exact V20 observational CPU admission, never source or deployment authority.
use crate::{
    AdmittedSimulationModuleV1, SimulationAdmissionErrorV1, SimulationKernelIrIdentityV1,
    SimulationLimitsV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV20,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Gfx942PhysicalEntryDeclarationVNext,
    Module, VerifiedCanonicalKernelIrModuleV20, gfx942_physical_entry_declaration_v20,
};
use std::{error::Error, fmt, mem::size_of};

/// Bounded resident-count traversal after the exact allocation-metered Reader.
/// This excludes decoding work, which is charged cumulatively by that Reader.
pub const PHYSICAL_ENTRY_ADMISSION_WORK_V20: usize = 16_384;

#[derive(Debug)]
pub enum PhysicalEntrySimulationAdmissionErrorV20 {
    Resource(Resource),
    CanonicalView(CanonicalKernelIrReplayAdmissionErrorV20),
    Admission(SimulationAdmissionErrorV1),
    Profile,
}
impl From<Resource> for PhysicalEntrySimulationAdmissionErrorV20 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<SimulationAdmissionErrorV1> for PhysicalEntrySimulationAdmissionErrorV20 {
    fn from(value: SimulationAdmissionErrorV1) -> Self {
        Self::Admission(value)
    }
}
impl fmt::Display for PhysicalEntrySimulationAdmissionErrorV20 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::CanonicalView(e) => e.fmt(f),
            Self::Admission(e) => e.fmt(f),
            Self::Profile => f.write_str("simulation requires exact physical-entry V20 membership"),
        }
    }
}
impl Error for PhysicalEntrySimulationAdmissionErrorV20 {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalEntrySimulationStorageV20 {
    retained: usize,
}
impl PhysicalEntrySimulationStorageV20 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

impl AdmittedSimulationModuleV1 {
    /// Retains an independent CPU view of the exact borrowed immutable owner.
    /// The original immutable canonical owner remains with the caller; no source custody is created. This does not authenticate host
    /// aliases or admit device execution. Reserve the output receipt while it lives.
    ///
    /// The exact Reader prepays every decoded allocation on this same ledger;
    /// accepted work, peak and denied history are never reset. Construction
    /// scratch is bounded by the caller resource budget. Simulator resident
    /// limits additionally bound the retained view plus borrowed input after
    /// decode; neither ledger is a host allocator/RSS cap.
    pub fn admit_v20_with_verification_budget(
        canonical: &VerifiedCanonicalKernelIrModuleV20,
        limits: SimulationLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, PhysicalEntrySimulationStorageV20), PhysicalEntrySimulationAdmissionErrorV20>
    {
        budget.charge_work(1)?;
        let _: &Gfx942PhysicalEntryDeclarationVNext =
            gfx942_physical_entry_declaration_v20(canonical)
                .ok_or(PhysicalEntrySimulationAdmissionErrorV20::Profile)?;
        let limits = limits
            .validate()
            .map_err(SimulationAdmissionErrorV1::InvalidLimits)?;
        let bytes = canonical.canonical_bytes();
        if bytes.len() > limits.max_canonical_bytes {
            return Err(SimulationAdmissionErrorV1::CanonicalBytesLimit {
                actual: bytes.len(),
                limit: limits.max_canonical_bytes,
            }
            .into());
        }
        // Whole-owner membership bounds one function, at most four blocks/64 native
        // operations and 768 definitions. Resident traversal only visits these
        // containers and fixed shallow types; strings are charged by capacity,
        // not traversed. The Reader accounts its actual allocations separately.
        let floor = budget.storage();
        budget.with_prepaid_scope(floor, 0, 0, 0, |budget| {
            let header = size_of::<Self>()
                .checked_sub(size_of::<Module>())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(header)?;
            let (module, view_storage) = canonical
                .decoded_inert_view_with_verification_budget_v20(budget)
                .map_err(PhysicalEntrySimulationAdmissionErrorV20::CanonicalView)?;
            // The inner scope transfers this independent view unreserved;
            // immediately adopt its exact receipt on the same caller ledger.
            budget.reserve_storage(view_storage.retained_storage())?;
            budget.charge_work(PHYSICAL_ENTRY_ADMISSION_WORK_V20)?;
            let resident = size_of::<Self>()
                .checked_add(
                    crate::resident::module_retained_heap_bytes(&module)
                        .ok_or(Resource::Arithmetic)?,
                )
                .ok_or(Resource::Arithmetic)?;
            let decoded_retained = header
                .checked_add(view_storage.retained_storage())
                .ok_or(Resource::Arithmetic)?;
            // The two existing ledgers use conservative B-tree payload formulas.
            // Retain the larger whole-view charge, never refund the Reader's.
            let retained = decoded_retained.max(resident);
            budget.reserve_storage(retained - decoded_retained)?;
            let with_input = retained
                .checked_add(bytes.len())
                .ok_or(Resource::Arithmetic)?;
            if with_input > limits.max_resident_bytes {
                return Err(SimulationAdmissionErrorV1::ResidentBytesLimit {
                    phase: "post-decode V20 retained view and borrowed input",
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
                PhysicalEntrySimulationStorageV20 { retained },
            ))
        })
    }
}
