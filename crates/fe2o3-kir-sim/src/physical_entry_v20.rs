//! Observational CPU admission of the actual physical-entry SSA/CFG.
//! No source authentication, physical-register file, native execution or launch authority.

use std::collections::BTreeSet;
use std::mem::size_of;

use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAME_V20,
    AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAMESPACE_V20,
    AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME, Kernel, Module, TargetCapability,
    VerifiedCanonicalKernelIrModuleV20, WaveWidth, decode_module_v20, encode_module_v20,
};

use crate::{
    AdmittedSimulationModuleV1, SimulationAdmissionErrorV1, SimulationKernelIrIdentityV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};

impl AdmittedSimulationModuleV1 {
    /// Borrows exact immutable V20 custody and retains an independent, bounded
    /// CPU inspection/execution view without projecting through an older schema.
    /// The original source owner stays intact for separate lowering/comparison.
    ///
    /// This is symbolic CPU execution, not source authentication, concrete device
    /// address bits, hardware execution, runtime deployment or physical capture. As in
    /// older admission routes, resident limits are checked after bounded decode
    /// and re-encode; they are not an allocator/RSS cap on rejected attempts.
    pub fn admit_v20(
        canonical: &VerifiedCanonicalKernelIrModuleV20,
        limits: SimulationLimitsV1,
    ) -> Result<Self, SimulationAdmissionErrorV1> {
        let limits = limits
            .validate()
            .map_err(SimulationAdmissionErrorV1::InvalidLimits)?;
        let bytes = canonical.canonical_bytes();
        if bytes.len() > limits.max_canonical_bytes {
            return Err(SimulationAdmissionErrorV1::CanonicalBytesLimit {
                actual: bytes.len(),
                limit: limits.max_canonical_bytes,
            });
        }
        let module =
            decode_module_v20(bytes).map_err(SimulationAdmissionErrorV1::DecodeAfterAdmission)?;
        let reencoded =
            encode_module_v20(&module).map_err(SimulationAdmissionErrorV1::EncodeAfterAdmission)?;
        if reencoded != bytes {
            return Err(SimulationAdmissionErrorV1::CanonicalRoundTripMismatch);
        }
        let admitted_resident_bytes = size_of::<Self>()
            .checked_add(
                crate::resident::module_retained_heap_bytes(&module)
                    .ok_or(SimulationAdmissionErrorV1::ResidentBytesOverflow)?,
            )
            .ok_or(SimulationAdmissionErrorV1::ResidentBytesOverflow)?;
        // Include the borrowed canonical input conservatively, without copying it.
        // The caller-owned source Module remains outside this simulator ledger.
        let peak = admitted_resident_bytes
            .checked_add(bytes.len())
            .and_then(|size| size.checked_add(reencoded.capacity()))
            .and_then(|size| size.checked_add(size_of::<Vec<u8>>()))
            .ok_or(SimulationAdmissionErrorV1::ResidentBytesOverflow)?;
        if peak > limits.max_resident_bytes {
            return Err(SimulationAdmissionErrorV1::ResidentBytesLimit {
                phase: "post-decode V20 canonical admission",
                actual: peak,
                limit: limits.max_resident_bytes,
            });
        }
        Ok(Self {
            identity: SimulationKernelIrIdentityV1::from(*canonical.identity()),
            module,
            admitted_resident_bytes,
        })
    }
}

impl AdmittedSimulationModuleV1 {
    pub(crate) fn uses_physical_entry_v20(&self) -> bool {
        self.module
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| {
                matches!(
                    operation.kind,
                    fe2o3_kernel_ir::OperationKind::Gfx942PhysicalEntryDeclaration(_)
                        | fe2o3_kernel_ir::OperationKind::Gfx942PhysicalEntryStep(_)
                )
            })
    }
    /// No checkpoint/reverse state can be complete in the old scalar debug schema.
    pub(crate) fn check_debug_capture_supported_v20(
        &self,
    ) -> Result<(), crate::SimulationPreflightErrorV1> {
        self.check_debug_capture_supported_v22()?;
        self.check_debug_capture_supported_v21()?;
        if self.uses_physical_entry_v20() {
            Err(crate::SimulationPreflightErrorV1::PhysicalEntrySymbolicDebugUnavailableV20)
        } else {
            Ok(())
        }
    }
}

/// The full declared profile is present at each containing scope. This does
/// not authenticate any target or prove that a device can execute the kernel.
pub(crate) fn scope_profile_matches(capabilities: &BTreeSet<TargetCapability>) -> bool {
    let has = |namespace: &str, expected: &str| {
        capabilities.iter().any(|capability| {
            matches!(capability, TargetCapability::Extension { namespace: actual, name }
            if actual == namespace && name == expected)
        })
    };
    capabilities.len() == 3
        && crate::ordered_program_v17::function_profile_is_consistent(capabilities)
        && capabilities.contains(&TargetCapability::WaveWidth(WaveWidth::Wave64))
        && has(
            AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
            AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
        )
        && has(
            AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAMESPACE_V20,
            AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAME_V20,
        )
}

pub(crate) fn launch_profile_matches(
    module: &Module,
    kernel: &Kernel,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    wire_version: u16,
) -> bool {
    wire_version == fe2o3_kernel_ir::KERNEL_IR_VERSION_V20
        && crate::ordered_program_v17::launch_profile_matches(module, kernel, request, target)
        && module.kernels.len() == 1
        && module.functions.len() == 1
        && request.grid.0[0] <= 128
        && scope_profile_matches(&module.required_capabilities)
        && scope_profile_matches(&kernel.required_capabilities)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_scope_refuses_missing_conflicting_and_benign_extra_capabilities() {
        let exact: BTreeSet<_> = [
            TargetCapability::WaveWidth(WaveWidth::Wave64),
            TargetCapability::Extension {
                namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.into(),
                name: AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME.into(),
            },
            TargetCapability::Extension {
                namespace: AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAMESPACE_V20.into(),
                name: AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAME_V20.into(),
            },
        ]
        .into_iter()
        .collect();
        assert!(scope_profile_matches(&exact));
        for removed in &exact {
            let mut missing = exact.clone();
            missing.remove(removed);
            assert!(!scope_profile_matches(&missing));
        }
        for extra in [
            TargetCapability::WaveWidth(WaveWidth::Wave32),
            TargetCapability::SubgroupSize(64),
            TargetCapability::Extension {
                namespace: "unrelated".into(),
                name: "extra".into(),
            },
        ] {
            let mut changed = exact.clone();
            changed.insert(extra);
            assert!(!scope_profile_matches(&changed));
        }
    }
}
