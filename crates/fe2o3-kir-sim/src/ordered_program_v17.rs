//! Observational CPU admission for a bounded ordered-program value abstraction.
//! Retained target/wave declarations are checked, not authenticated hardware facts.
//! Raw V17 may use the existing logical control-flow grammar: this is not source
//! or native-profile admission. The separate source owner/emitter require their
//! singleton unconditional acyclic occurrence; this module does not claim that.

use std::collections::BTreeSet;
use std::mem::size_of;

use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    Kernel, Module, TargetCapability, VerifiedCanonicalKernelIrModuleV17, WaveWidth, WorkgroupSize,
    decode_module_v17, encode_module_v17,
};

use crate::{
    AdmittedSimulationModuleV1, IndexWidthV1, SimulationAdmissionErrorV1,
    SimulationKernelIrIdentityV1, SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};

impl AdmittedSimulationModuleV1 {
    /// Borrows exact immutable V17 custody and retains an independent, bounded
    /// CPU inspection/execution view without projecting through an older schema.
    /// The original source owner stays intact for separate lowering/comparison.
    ///
    /// This is not source authentication or physical-register execution. As in
    /// older admission routes, resident limits are checked after bounded decode
    /// and re-encode; they are not an allocator/RSS cap on rejected attempts.
    pub fn admit_v17(
        canonical: &VerifiedCanonicalKernelIrModuleV17,
        limits: SimulationLimitsV1,
    ) -> Result<Self, SimulationAdmissionErrorV1> {
        let limits = limits
            .validate()
            .map_err(SimulationAdmissionErrorV1::InvalidLimits)?;
        let bytes = canonical.canonical().canonical_bytes();
        if bytes.len() > limits.max_canonical_bytes {
            return Err(SimulationAdmissionErrorV1::CanonicalBytesLimit {
                actual: bytes.len(),
                limit: limits.max_canonical_bytes,
            });
        }
        let module =
            decode_module_v17(bytes).map_err(SimulationAdmissionErrorV1::DecodeAfterAdmission)?;
        let reencoded =
            encode_module_v17(&module).map_err(SimulationAdmissionErrorV1::EncodeAfterAdmission)?;
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
                phase: "post-decode V17 canonical admission",
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

#[derive(Default)]
struct DeclaredProfile {
    target: bool,
    wave: bool,
    conflicting: bool,
}

impl DeclaredProfile {
    fn include(&mut self, capabilities: &BTreeSet<TargetCapability>) {
        for capability in capabilities {
            match capability {
                TargetCapability::Extension { namespace, name }
                    if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE =>
                {
                    if name == AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME {
                        self.target = true;
                    } else {
                        self.conflicting = true;
                    }
                }
                TargetCapability::WaveWidth(WaveWidth::Wave64) => self.wave = true,
                TargetCapability::WaveWidth(WaveWidth::Wave32) => self.conflicting = true,
                TargetCapability::SubgroupSize(size) if *size != 64 => self.conflicting = true,
                _ => {}
            }
        }
    }
}

/// Computed once for the selected launch, without deriving source or hardware authority.
pub(crate) fn launch_profile_matches(
    module: &Module,
    kernel: &Kernel,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
) -> bool {
    let mut profile = DeclaredProfile::default();
    profile.include(&module.required_capabilities);
    profile.include(&kernel.required_capabilities);
    profile.target
        && profile.wave
        && !profile.conflicting
        && target.index_width() == IndexWidthV1::Bits64
        && kernel.workgroup_size == Some(WorkgroupSize::new(64, 1, 1))
        && request.workgroup.0 == [64, 1, 1]
        && request.grid.0[0] != 0
        && request.grid.0[0].is_multiple_of(64)
        && request.grid.0[1..] == [1, 1]
}

/// A function may inherit the enclosing declared profile, but not contradict it.
pub(crate) fn function_profile_is_consistent(capabilities: &BTreeSet<TargetCapability>) -> bool {
    let mut profile = DeclaredProfile::default();
    profile.include(capabilities);
    !profile.conflicting
}
