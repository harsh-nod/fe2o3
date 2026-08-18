use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct DeviceKeyV1 {
    pub physical: nat,
    pub generation: nat,
}

pub struct VmRecordV1 {
    pub device: DeviceKeyV1,
}

pub proof fn mutated_vm_generation_substitution_is_exact_v1()
    ensures
        (VmRecordV1 {
            device: DeviceKeyV1 { physical: 9, generation: 1 },
        }).device == (DeviceKeyV1 { physical: 9, generation: 2 }),
{
}

} // verus!
