use vstd::prelude::*;
verus! {
pub enum PhaseV1 { DeviceReady, DemotedDeviceCleanup }
pub open spec fn mutated_demotion_retry_phase_v1(_phase: PhaseV1) -> PhaseV1 {
    PhaseV1::DemotedDeviceCleanup
}
pub proof fn mutated_demotion_retry_restores_device_v1()
    ensures mutated_demotion_retry_phase_v1(PhaseV1::DeviceReady) == PhaseV1::DeviceReady, {}
}
