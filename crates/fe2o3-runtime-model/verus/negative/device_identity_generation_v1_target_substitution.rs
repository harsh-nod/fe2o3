use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum DeviceAdmissionTargetProfileV1 {
    Gfx942XnackMinusSpxNps1Kfd1_18Drm3_64_0,
    Gfx950XnackMinusSpxNps1Kfd1_18Drm3_64_0,
}

pub open spec fn profile_target_v1(profile: DeviceAdmissionTargetProfileV1) -> nat {
    match profile {
        DeviceAdmissionTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18Drm3_64_0 => 942,
        DeviceAdmissionTargetProfileV1::Gfx950XnackMinusSpxNps1Kfd1_18Drm3_64_0 => 950,
    }
}

pub struct CorrelationProfileV1 {
    pub target_profile: DeviceAdmissionTargetProfileV1,
    pub identity: nat,
    pub kfd_schema: nat,
    pub drm_schema: nat,
}

pub struct CorrelationObservationV1 {
    pub domain: nat,
    pub physical: nat,
    pub target: nat,
    pub kfd_schema: nat,
    pub drm_schema: nat,
}

pub open spec fn mutated_correlation_without_target_check_v1(
    profile: CorrelationProfileV1,
    observation: CorrelationObservationV1,
) -> bool {
    &&& observation.domain > 0
    &&& observation.physical > 0
    &&& observation.kfd_schema == profile.kfd_schema
    &&& observation.drm_schema == profile.drm_schema
}

pub proof fn mutated_identity_target_substitution_is_rejected_v1(
    profile: CorrelationProfileV1,
    observation: CorrelationObservationV1,
)
    requires
        observation.domain > 0,
        observation.physical > 0,
        observation.kfd_schema == profile.kfd_schema,
        observation.drm_schema == profile.drm_schema,
        observation.target == 942 || observation.target == 950,
        observation.target != profile_target_v1(profile.target_profile),
    ensures
        !mutated_correlation_without_target_check_v1(profile, observation),
{
}

} // verus!
