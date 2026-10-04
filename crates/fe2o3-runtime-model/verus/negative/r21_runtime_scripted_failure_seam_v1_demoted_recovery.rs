use vstd::prelude::*;
verus! {
pub enum PhaseV1 { DeviceReady, DemotedDeviceCleanup }
pub open spec fn mutated_recovered_demoted_phase_v1(_phase: PhaseV1) -> PhaseV1 {
    PhaseV1::DeviceReady
}
pub proof fn mutated_recovered_demoted_owner_enters_cleanup_v1()
    ensures mutated_recovered_demoted_phase_v1(PhaseV1::DeviceReady)
        == PhaseV1::DemotedDeviceCleanup, {}
}
