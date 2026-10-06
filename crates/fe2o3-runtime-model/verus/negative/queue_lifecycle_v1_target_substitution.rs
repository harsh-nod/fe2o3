use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum TargetProfileV1 {
    Gfx942,
    Gfx950,
}

#[derive(PartialEq, Eq)]
pub struct DeviceKeyV1 {
    pub physical: nat,
    pub generation: nat,
}

pub struct QueueBindingV1 {
    pub queue_device: DeviceKeyV1,
    pub admitted_device: DeviceKeyV1,
    pub queue_target: TargetProfileV1,
    pub admitted_target: TargetProfileV1,
    pub profile_identity: nat,
}

pub open spec fn mutated_queue_binding_without_target_v1(plan: QueueBindingV1) -> bool {
    plan.queue_device == plan.admitted_device
}

pub proof fn mutated_queue_target_substitution_is_rejected_v1(plan: QueueBindingV1)
    requires
        plan.queue_device == plan.admitted_device,
        plan.queue_device.physical > 0,
        plan.queue_device.generation > 0,
        plan.queue_target != plan.admitted_target,
    ensures
        !mutated_queue_binding_without_target_v1(plan),
{
}

} // verus!
