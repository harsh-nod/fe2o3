//! Inert projection of bounded, wave-scoped physical register observations.
//!
//! This DTO is not a stop, device, allocation, or control capability. Its target
//! is supplied only by the checked in-process producer; constructing an equal
//! DTO cannot create that producer owner. Existing V4/V5 wire formats are unchanged.

use crate::{
    LiveGpuAvailabilityV3, LiveGpuContentIdentityV3, LiveGpuEvidenceKindV3, LiveGpuRegisterClassV3,
    LiveGpuTruthOriginV3, LiveGpuUnavailableReasonV3, LiveGpuValueEncodingV3, LiveGpuValueKindV3,
    OpaqueIdentityV1, RocgdbMiNativeInspectionUnavailableReasonV5,
    RocgdbMiNativeUnavailableFieldV5, RocgdbMiRegisterSnapshotV3, RocgdbMiStoppedScopeV3,
    RocgdbMiWorkgroupCoordinateV4,
};
use serde::Serialize;

/// Exactly the currently admitted producer profile; this is not browser intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbHardwareCheckedTargetV1 {
    Gfx942XnackMinusWave64,
}

/// A historical value after it is copied or serialized. Only the process owner's
/// borrowed accessor establishes that it is still the currently retained stop.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RocgdbHardwareStopResourcesV1 {
    pub target: RocgdbHardwareCheckedTargetV1,
    pub session_identity: OpaqueIdentityV1,
    pub stop_revision: u64,
    pub association_identity: OpaqueIdentityV1,
    pub queue_occurrence_identity: OpaqueIdentityV1,
    pub process_instance_identity: OpaqueIdentityV1,
    pub dispatch_identity: OpaqueIdentityV1,
    pub artifact: LiveGpuContentIdentityV3,
    pub grid: [u32; 3],
    pub workgroup: [u32; 3],
    pub workgroup_coordinate: RocgdbMiWorkgroupCoordinateV4,
    pub wave_in_workgroup: u32,
    pub scope: RocgdbMiStoppedScopeV3,
    pub register_evidence_identity: OpaqueIdentityV1,
    pub registers: RocgdbMiRegisterSnapshotV3,
    pub source: RocgdbMiNativeUnavailableFieldV5,
    pub isa: RocgdbMiNativeUnavailableFieldV5,
    pub memory: RocgdbMiNativeUnavailableFieldV5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RocgdbHardwareStopResourceErrorV1 {
    InvalidBinding,
    InvalidGeometry,
    InvalidRegister,
    InvalidUnavailableBoundary,
}

impl RocgdbHardwareStopResourcesV1 {
    /// Structural validation only; never authenticates a copied observation.
    pub fn validate(&self) -> Result<(), RocgdbHardwareStopResourceErrorV1> {
        use RocgdbHardwareStopResourceErrorV1 as E;
        if self.stop_revision == 0
            || self.artifact.canonical_bytes == 0
            || self.scope.validate().is_err()
            || self.scope.lane.is_some()
            || self.registers.scope != self.scope
        {
            return Err(E::InvalidBinding);
        }
        if self.grid.contains(&0) || self.workgroup.contains(&0) {
            return Err(E::InvalidGeometry);
        }
        let coordinate = [
            self.workgroup_coordinate.x,
            self.workgroup_coordinate.y,
            self.workgroup_coordinate.z,
        ];
        let mut actual_volume = 1_u32;
        let mut declared_volume = 1_u32;
        for (axis, coordinate) in coordinate.into_iter().enumerate() {
            if self.workgroup[axis] > self.grid[axis] {
                return Err(E::InvalidGeometry);
            }
            declared_volume = declared_volume
                .checked_mul(self.workgroup[axis])
                .ok_or(E::InvalidGeometry)?;
            let start = coordinate
                .checked_mul(self.workgroup[axis])
                .filter(|start| *start < self.grid[axis])
                .ok_or(E::InvalidGeometry)?;
            actual_volume = actual_volume
                .checked_mul(self.workgroup[axis].min(self.grid[axis] - start))
                .ok_or(E::InvalidGeometry)?;
        }
        if declared_volume > 1024
            || self
                .wave_in_workgroup
                .checked_mul(64)
                .is_none_or(|first| first >= actual_volume)
        {
            return Err(E::InvalidGeometry);
        }
        self.registers.validate().map_err(|_| E::InvalidRegister)?;
        for register in &self.registers.registers {
            if register.lane.is_some() || register.kind != LiveGpuValueKindV3::UnsignedInteger {
                return Err(E::InvalidRegister);
            }
            let scalar = register.name.strip_prefix('s').is_some_and(|digits| {
                !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
            });
            let predicate = matches!(register.name.as_str(), "exec" | "vcc" | "scc");
            let supported = match register.class {
                LiveGpuRegisterClassV3::Scalar => scalar,
                LiveGpuRegisterClassV3::Predicate => predicate,
                LiveGpuRegisterClassV3::Vector | LiveGpuRegisterClassV3::Special => false,
            };
            match &register.value {
                LiveGpuAvailabilityV3::Available { value, truth } => {
                    if !supported || !observed(truth, self.register_evidence_identity) {
                        return Err(E::InvalidRegister);
                    }
                    let LiveGpuValueEncodingV3::Bits { bit_width, bits } = value else {
                        return Err(E::InvalidRegister);
                    };
                    if *bit_width == 0
                        || *bit_width > 64
                        || *bit_width % 4 != 0
                        || bits.len() != usize::from(*bit_width / 4)
                        || !bits
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                    {
                        return Err(E::InvalidRegister);
                    }
                }
                LiveGpuAvailabilityV3::Redacted { reason, truth } => {
                    if !matches!(register.name.as_str(), "pc" | "pc_all")
                        || *reason != crate::LiveGpuRedactionReasonV3::AbsoluteTargetLocation
                        || !observed(truth, self.register_evidence_identity)
                    {
                        return Err(E::InvalidRegister);
                    }
                }
                LiveGpuAvailabilityV3::Unavailable { reason, truth } => {
                    if truth.origin != LiveGpuTruthOriginV3::Unavailable
                        || !truth.evidence.is_empty()
                        || (!supported && *reason != LiveGpuUnavailableReasonV3::Unsupported)
                    {
                        return Err(E::InvalidRegister);
                    }
                }
            }
        }
        if !matches!(self.source, RocgdbMiNativeUnavailableFieldV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresAuthenticatedSourceMap
        }) || !matches!(self.isa, RocgdbMiNativeUnavailableFieldV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresArtifactRelativeInstructionBinding
        }) || !matches!(self.memory, RocgdbMiNativeUnavailableFieldV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresAllocationRelativeAuthority
        }) {
            return Err(E::InvalidUnavailableBoundary);
        }
        Ok(())
    }
}

fn observed(truth: &crate::LiveGpuTruthV3, evidence: OpaqueIdentityV1) -> bool {
    truth.origin == LiveGpuTruthOriginV3::Observed
        && truth.evidence.len() == 1
        && truth.evidence[0].kind == LiveGpuEvidenceKindV3::RuntimeObservation
        && truth.evidence[0].identity == evidence
}

#[cfg(test)]
#[path = "hardware_stop_resources_v1_tests.rs"]
mod tests;
