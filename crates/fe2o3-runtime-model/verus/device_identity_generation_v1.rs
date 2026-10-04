use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct DeviceKeyV1 {
    pub physical: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct VmKeyV1 {
    pub device: DeviceKeyV1,
    pub id: nat,
}

pub struct DeviceRecordV1 {
    pub key: DeviceKeyV1,
    pub active: bool,
}

pub struct VmRecordV1 {
    pub key: VmKeyV1,
    pub active: bool,
}

pub struct IdentityStateV1 {
    pub devices: Seq<DeviceRecordV1>,
    pub vms: Seq<VmRecordV1>,
}

pub open spec fn device_is_active_v1(state: IdentityStateV1, key: DeviceKeyV1) -> bool {
    exists |i: int| 0 <= i < state.devices.len()
        && #[trigger] state.devices[i].key == key
        && state.devices[i].active
}

pub open spec fn unique_active_device_generation_v1(state: IdentityStateV1) -> bool {
    forall |left: int, right: int|
        0 <= left < state.devices.len()
        && 0 <= right < state.devices.len()
        && #[trigger] state.devices[left].active
        && #[trigger] state.devices[right].active
        && state.devices[left].key.physical == state.devices[right].key.physical
        ==> state.devices[left].key == state.devices[right].key
}

pub open spec fn every_active_vm_has_exact_device_generation_v1(
    state: IdentityStateV1,
) -> bool {
    forall |v: int| 0 <= v < state.vms.len()
        && #[trigger] state.vms[v].active
        ==> device_is_active_v1(state, state.vms[v].key.device)
}

pub open spec fn identity_generation_invariant_v1(state: IdentityStateV1) -> bool {
    unique_active_device_generation_v1(state)
        && every_active_vm_has_exact_device_generation_v1(state)
}

pub open spec fn can_register_device_v1(
    state: IdentityStateV1,
    requested: DeviceKeyV1,
) -> bool {
    &&& requested.generation > 0
    &&& forall |i: int| 0 <= i < state.devices.len()
        && #[trigger] state.devices[i].key.physical == requested.physical
        ==> !state.devices[i].active && state.devices[i].key.generation < requested.generation
}

pub open spec fn register_device_v1(
    state: IdentityStateV1,
    requested: DeviceKeyV1,
) -> IdentityStateV1 {
    IdentityStateV1 {
        devices: state.devices.push(DeviceRecordV1 { key: requested, active: true }),
        vms: state.vms,
    }
}

pub open spec fn can_register_vm_v1(
    state: IdentityStateV1,
    observed_device: DeviceKeyV1,
    vm_id: nat,
) -> bool {
    &&& vm_id > 0
    &&& device_is_active_v1(state, observed_device)
    &&& forall |v: int| 0 <= v < state.vms.len()
        ==> #[trigger] state.vms[v].key != VmKeyV1 { device: observed_device, id: vm_id }
}

pub open spec fn register_vm_v1(
    state: IdentityStateV1,
    observed_device: DeviceKeyV1,
    vm_id: nat,
) -> IdentityStateV1 {
    IdentityStateV1 {
        devices: state.devices,
        vms: state.vms.push(VmRecordV1 {
            key: VmKeyV1 { device: observed_device, id: vm_id },
            active: true,
        }),
    }
}

pub proof fn register_device_preserves_generation_invariant_v1(
    state: IdentityStateV1,
    requested: DeviceKeyV1,
)
    requires
        identity_generation_invariant_v1(state),
        can_register_device_v1(state, requested),
    ensures
        identity_generation_invariant_v1(register_device_v1(state, requested)),
        device_is_active_v1(register_device_v1(state, requested), requested),
{
    let next = register_device_v1(state, requested);
    assert(device_is_active_v1(next, requested)) by {
        let index = state.devices.len() as int;
        assert(next.devices[index].key == requested);
        assert(next.devices[index].active);
    }
    assert(unique_active_device_generation_v1(next)) by {
        assert forall |left: int, right: int|
            0 <= left < next.devices.len()
            && 0 <= right < next.devices.len()
            && #[trigger] next.devices[left].active
            && #[trigger] next.devices[right].active
            && next.devices[left].key.physical == next.devices[right].key.physical
            implies next.devices[left].key == next.devices[right].key by {
            let old_len = state.devices.len() as int;
            if left < old_len && right < old_len {
                assert(next.devices[left] == state.devices[left]);
                assert(next.devices[right] == state.devices[right]);
            } else if left == old_len && right < old_len {
                assert(next.devices[left].key == requested);
                assert(state.devices[right].key.physical == requested.physical);
                assert(!state.devices[right].active);
                assert(next.devices[right] == state.devices[right]);
            } else if left < old_len && right == old_len {
                assert(next.devices[right].key == requested);
                assert(state.devices[left].key.physical == requested.physical);
                assert(!state.devices[left].active);
                assert(next.devices[left] == state.devices[left]);
            } else {
                assert(left == old_len && right == old_len);
            }
        }
    }
    assert(every_active_vm_has_exact_device_generation_v1(next)) by {
        assert forall |v: int| 0 <= v < next.vms.len()
            && #[trigger] next.vms[v].active
            implies device_is_active_v1(next, next.vms[v].key.device) by {
            assert(next.vms[v] == state.vms[v]);
            let key = state.vms[v].key.device;
            assert(device_is_active_v1(state, key));
            let witness = choose |i: int| 0 <= i < state.devices.len()
                && state.devices[i].key == key
                && state.devices[i].active;
            assert(next.devices[witness] == state.devices[witness]);
            assert(device_is_active_v1(next, key));
        }
    }
}

pub proof fn register_vm_preserves_exact_device_generation_v1(
    state: IdentityStateV1,
    observed_device: DeviceKeyV1,
    vm_id: nat,
)
    requires
        identity_generation_invariant_v1(state),
        can_register_vm_v1(state, observed_device, vm_id),
    ensures
        identity_generation_invariant_v1(register_vm_v1(state, observed_device, vm_id)),
        register_vm_v1(state, observed_device, vm_id).vms.last().key.device
            == observed_device,
        device_is_active_v1(register_vm_v1(state, observed_device, vm_id), observed_device),
{
    let next = register_vm_v1(state, observed_device, vm_id);
    assert(unique_active_device_generation_v1(next));
    let device_index = choose |i: int| 0 <= i < state.devices.len()
        && state.devices[i].key == observed_device
        && state.devices[i].active;
    assert(next.devices[device_index] == state.devices[device_index]);
    assert(device_is_active_v1(next, observed_device));
    assert(every_active_vm_has_exact_device_generation_v1(next)) by {
        assert forall |v: int| 0 <= v < next.vms.len()
            && #[trigger] next.vms[v].active
            implies device_is_active_v1(next, next.vms[v].key.device) by {
            let old_len = state.vms.len() as int;
            if v < old_len {
                assert(next.vms[v] == state.vms[v]);
            } else {
                assert(v == old_len);
                assert(next.vms[v].key.device == observed_device);
            }
        }
    }
}

pub proof fn active_vm_cannot_mix_device_generations_v1(
    state: IdentityStateV1,
    vm_index: int,
    substituted: DeviceKeyV1,
)
    requires
        identity_generation_invariant_v1(state),
        0 <= vm_index < state.vms.len(),
        state.vms[vm_index].active,
        substituted.physical == state.vms[vm_index].key.device.physical,
        substituted.generation != state.vms[vm_index].key.device.generation,
    ensures
        !device_is_active_v1(state, substituted),
        !can_register_vm_v1(state, substituted, state.vms[vm_index].key.id + 1),
{
    let exact = state.vms[vm_index].key.device;
    assert(device_is_active_v1(state, exact));
    let exact_index = choose |i: int| 0 <= i < state.devices.len()
        && state.devices[i].key == exact
        && state.devices[i].active;
    if device_is_active_v1(state, substituted) {
        let substituted_index = choose |i: int| 0 <= i < state.devices.len()
            && state.devices[i].key == substituted
            && state.devices[i].active;
        assert(state.devices[exact_index].key.physical
            == state.devices[substituted_index].key.physical);
        assert(state.devices[exact_index].key == state.devices[substituted_index].key);
        assert(exact == substituted);
    }
}

pub proof fn stale_device_generation_is_rejected_v1(
    state: IdentityStateV1,
    current: DeviceKeyV1,
    requested: DeviceKeyV1,
)
    requires
        device_is_active_v1(state, current),
        requested.physical == current.physical,
        requested.generation <= current.generation,
    ensures
        !can_register_device_v1(state, requested),
{
    let current_index = choose |i: int| 0 <= i < state.devices.len()
        && state.devices[i].key == current
        && state.devices[i].active;
}

} // verus!
