//! Exact V22 observational CPU admission, never source or deployment authority.
use crate::{
    AdmittedSimulationModuleV1, SimulationAdmissionErrorV1, SimulationArgumentV1,
    SimulationKernelIrIdentityV1, SimulationLimitsV1, SimulationPreflightErrorV1,
    SimulationRequestV1, SimulationTargetV1,
};
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
    AMDGPU_GFX942_PHYSICAL_LDS_EXCHANGE_CAPABILITY_NAME_V22,
    AMDGPU_GFX942_PHYSICAL_LDS_EXCHANGE_CAPABILITY_NAMESPACE_V22,
    AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME, CanonicalKernelIrReplayAdmissionErrorV22,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    Gfx942PhysicalLdsExchangeDeclarationV1, Kernel, Module, TargetCapability,
    VerifiedCanonicalKernelIrModuleV22, WaveWidth, gfx942_physical_lds_exchange_declaration_v22,
};
use std::{collections::BTreeSet, error::Error, fmt, mem::size_of};

/// Bounded resident-count traversal after the exact allocation-metered Reader.
/// This excludes decoding work, which is charged cumulatively by that Reader.
pub const PHYSICAL_LDS_EXCHANGE_ADMISSION_WORK_V22: usize = 4096;

#[derive(Debug)]
pub enum PhysicalLdsExchangeSimulationAdmissionErrorV22 {
    Resource(Resource),
    CanonicalView(CanonicalKernelIrReplayAdmissionErrorV22),
    Admission(SimulationAdmissionErrorV1),
    Profile,
}
impl From<Resource> for PhysicalLdsExchangeSimulationAdmissionErrorV22 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<SimulationAdmissionErrorV1> for PhysicalLdsExchangeSimulationAdmissionErrorV22 {
    fn from(value: SimulationAdmissionErrorV1) -> Self {
        Self::Admission(value)
    }
}
impl fmt::Display for PhysicalLdsExchangeSimulationAdmissionErrorV22 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::CanonicalView(e) => e.fmt(f),
            Self::Admission(e) => e.fmt(f),
            Self::Profile => {
                f.write_str("simulation requires exact physical-lds-exchange V22 membership")
            }
        }
    }
}
impl Error for PhysicalLdsExchangeSimulationAdmissionErrorV22 {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalLdsExchangeSimulationStorageV22 {
    retained: usize,
}
impl PhysicalLdsExchangeSimulationStorageV22 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

impl AdmittedSimulationModuleV1 {
    /// Retains an independent CPU view of the exact borrowed immutable owner.
    /// Source custody stays with the caller. This does not authenticate host
    /// aliases or admit device execution. Reserve the output receipt while it lives.
    ///
    /// The exact Reader prepays every decoded allocation on this same ledger;
    /// accepted work, peak and denied history are never reset. Construction
    /// scratch is bounded by the caller resource budget. Simulator resident
    /// limits additionally bound the retained view plus borrowed input after
    /// decode; neither ledger is a host allocator/RSS cap.
    pub fn admit_v22_with_verification_budget(
        canonical: &VerifiedCanonicalKernelIrModuleV22,
        limits: SimulationLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<
        (Self, PhysicalLdsExchangeSimulationStorageV22),
        PhysicalLdsExchangeSimulationAdmissionErrorV22,
    > {
        budget.charge_work(1)?;
        let _: &Gfx942PhysicalLdsExchangeDeclarationV1 =
            gfx942_physical_lds_exchange_declaration_v22(canonical)
                .ok_or(PhysicalLdsExchangeSimulationAdmissionErrorV22::Profile)?;
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
        // Whole-owner membership bounds one function/block, at most 40 native
        // operations and 192 definitions. Resident traversal only visits these
        // containers and fixed shallow types; strings are charged by capacity,
        // not traversed. The Reader accounts its actual allocations separately.
        let floor = budget.storage();
        budget.with_prepaid_scope(floor, 0, 0, 0, |budget| {
            let header = size_of::<Self>()
                .checked_sub(size_of::<Module>())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(header)?;
            let (module, view_storage) = canonical
                .decoded_inert_view_with_verification_budget_v22(budget)
                .map_err(PhysicalLdsExchangeSimulationAdmissionErrorV22::CanonicalView)?;
            // The inner scope transfers this independent view unreserved;
            // immediately adopt its exact receipt on the same caller ledger.
            budget.reserve_storage(view_storage.retained_storage())?;
            budget.charge_work(PHYSICAL_LDS_EXCHANGE_ADMISSION_WORK_V22)?;
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
                    phase: "post-decode V22 retained view and borrowed input",
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
                PhysicalLdsExchangeSimulationStorageV22 { retained },
            ))
        })
    }

    pub(crate) fn uses_physical_lds_exchange_v22(&self) -> bool {
        self.identity.wire_version() == fe2o3_kernel_ir::KERNEL_IR_VERSION_V22
    }
    pub(crate) fn check_debug_capture_supported_v22(
        &self,
    ) -> Result<(), SimulationPreflightErrorV1> {
        if self.uses_physical_lds_exchange_v22() {
            Err(SimulationPreflightErrorV1::PhysicalLdsExchangeDebugUnavailableV22)
        } else {
            Ok(())
        }
    }
}
pub(crate) fn scope_profile_matches(values: &BTreeSet<TargetCapability>) -> bool {
    values.len() == 3
        && values.iter().all(|value| match value {
            TargetCapability::WaveWidth(WaveWidth::Wave64) => true,
            TargetCapability::Extension { namespace, name } => {
                (namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
                    && name == AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
                    || (namespace == AMDGPU_GFX942_PHYSICAL_LDS_EXCHANGE_CAPABILITY_NAMESPACE_V22
                        && name == AMDGPU_GFX942_PHYSICAL_LDS_EXCHANGE_CAPABILITY_NAME_V22)
            }
            _ => false,
        })
}
pub(crate) fn launch_profile_matches(
    module: &Module,
    kernel: &Kernel,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    version: u16,
) -> bool {
    version == fe2o3_kernel_ir::KERNEL_IR_VERSION_V22
        && target.index_width() == crate::IndexWidthV1::Bits64
        && kernel.workgroup_size == Some(fe2o3_kernel_ir::WorkgroupSize::new(128, 1, 1))
        && request.workgroup.0 == [128, 1, 1]
        && request.grid.0 == [128, 1, 1]
        && module.kernels.len() == 1
        && module.functions.len() == 1
        && request.grid.0[0] <= 128
        && scope_profile_matches(&module.required_capabilities)
        && scope_profile_matches(&kernel.required_capabilities)
}
/// Closed profile binding: different direct buffers create distinct allocations;
/// named views must name distinct backings even when their byte intervals differ.
/// This is a request-local allocation check, not a host alias theorem/race proof.
pub(crate) fn validate_separate_bindings(
    request: &SimulationRequestV1,
) -> Result<(), SimulationPreflightErrorV1> {
    if let [
        SimulationArgumentV1::BufferView(input),
        SimulationArgumentV1::BufferView(output),
    ] = request.arguments.as_slice()
        && input.backing() == output.backing()
    {
        return Err(SimulationPreflightErrorV1::PhysicalLdsExchangeAliasedArgumentsV22);
    }
    Ok(())
}
