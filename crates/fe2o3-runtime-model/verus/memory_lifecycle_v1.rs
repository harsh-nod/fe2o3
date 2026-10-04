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

pub struct MemoryBindingV1 {
    pub vm: VmKeyV1,
    pub allocation_vm: VmKeyV1,
    pub allocation_id: nat,
    pub allocation_generation: nat,
    pub allocation_handle_observation: nat,
    pub mapping_allocation_vm: VmKeyV1,
    pub mapping_allocation_id: nat,
    pub mapping_allocation_generation: nat,
    pub vm_mapping_devices: Seq<DeviceKeyV1>,
    pub mapping_devices: Seq<DeviceKeyV1>,
}

pub open spec fn canonical_device_set_v1(devices: Seq<DeviceKeyV1>) -> bool {
    &&& 0 < devices.len() <= 16
    &&& forall |i: int| 0 <= i < devices.len() ==> {
        &&& #[trigger] devices[i].physical > 0
        &&& devices[i].generation > 0
    }
    &&& forall |left: int, right: int|
        0 <= left < devices.len()
        && 0 <= right < devices.len()
        && left != right
        ==> {
            &&& #[trigger] devices[left] != #[trigger] devices[right]
            &&& devices[left].physical != devices[right].physical
        }
}

pub open spec fn memory_binding_valid_v1(binding: MemoryBindingV1) -> bool {
    &&& binding.vm.device.physical > 0
    &&& binding.vm.device.generation > 0
    &&& binding.vm.id > 0
    &&& binding.allocation_vm == binding.vm
    &&& binding.allocation_id > 0
    &&& binding.allocation_generation > 0
    &&& binding.allocation_handle_observation > 0
    &&& binding.mapping_allocation_vm == binding.allocation_vm
    &&& binding.mapping_allocation_id == binding.allocation_id
    &&& binding.mapping_allocation_generation == binding.allocation_generation
    &&& canonical_device_set_v1(binding.vm_mapping_devices)
    &&& binding.mapping_devices =~= binding.vm_mapping_devices
    &&& exists |i: int| 0 <= i < binding.vm_mapping_devices.len()
        && binding.vm_mapping_devices[i] == binding.vm.device
}

#[derive(PartialEq, Eq)]
pub enum MappingProgressStateV1 {
    MapPending,
    MapFailed,
    Mapped,
    UnmapPending,
    UnmapFailed,
    Unmapped,
    Ambiguous,
    Released,
}

pub struct MappingProgressV1 {
    pub device_count: nat,
    pub mapped_start: nat,
    pub mapped_end: nat,
    pub state: MappingProgressStateV1,
}

pub open spec fn mapping_progress_valid_v1(progress: MappingProgressV1) -> bool {
    &&& 0 < progress.device_count <= 16
    &&& progress.mapped_start <= progress.mapped_end <= progress.device_count
    &&& match progress.state {
        MappingProgressStateV1::MapPending => {
            progress.mapped_start == 0 && progress.mapped_end == 0
        },
        MappingProgressStateV1::MapFailed => progress.mapped_start == 0,
        MappingProgressStateV1::Mapped => {
            progress.mapped_start == 0 && progress.mapped_end == progress.device_count
        },
        MappingProgressStateV1::UnmapPending => progress.mapped_start < progress.mapped_end,
        MappingProgressStateV1::Released => progress.mapped_start == progress.mapped_end,
        MappingProgressStateV1::UnmapFailed | MappingProgressStateV1::Ambiguous => true,
        MappingProgressStateV1::Unmapped => progress.mapped_start == progress.mapped_end,
    }
}

pub open spec fn observe_failed_map_prefix_v1(
    old: MappingProgressV1,
    n_success: nat,
) -> MappingProgressV1 {
    MappingProgressV1 {
        device_count: old.device_count,
        mapped_start: 0,
        mapped_end: n_success,
        state: MappingProgressStateV1::MapFailed,
    }
}

pub open spec fn observe_failed_unmap_cumulative_v1(
    old: MappingProgressV1,
    n_success: nat,
) -> MappingProgressV1 {
    if old.mapped_start <= n_success && n_success < old.mapped_end {
        MappingProgressV1 {
            device_count: old.device_count,
            mapped_start: n_success,
            mapped_end: old.mapped_end,
            state: MappingProgressStateV1::UnmapFailed,
        }
    } else {
        MappingProgressV1 {
            device_count: old.device_count,
            mapped_start: old.mapped_start,
            mapped_end: old.mapped_end,
            state: MappingProgressStateV1::Ambiguous,
        }
    }
}

pub open spec fn begin_map_if_exact_device_set_v1(
    binding: MemoryBindingV1,
    requested_devices: Seq<DeviceKeyV1>,
) -> Option<MappingProgressV1> {
    if requested_devices =~= binding.vm_mapping_devices {
        Some(MappingProgressV1 {
            device_count: requested_devices.len(),
            mapped_start: 0,
            mapped_end: 0,
            state: MappingProgressStateV1::MapPending,
        })
    } else {
        None
    }
}

pub struct MappingRetentionV1 {
    pub allocation_id: nat,
    pub allocation_generation: nat,
    pub state: MappingProgressStateV1,
    pub live_publications: nat,
}

pub open spec fn mapping_retains_allocation_v1(mapping: MappingRetentionV1) -> bool {
    mapping.state != MappingProgressStateV1::Released || mapping.live_publications > 0
}

pub open spec fn can_free_allocation_v1(
    allocation_id: nat,
    allocation_generation: nat,
    mappings: Seq<MappingRetentionV1>,
) -> bool {
    forall |i: int| 0 <= i < mappings.len()
        && #[trigger] mappings[i].allocation_id == allocation_id
        && mappings[i].allocation_generation == allocation_generation
        ==> !mapping_retains_allocation_v1(mappings[i])
}

pub proof fn exact_vm_allocation_mapping_generation_is_retained_v1(
    binding: MemoryBindingV1,
)
    requires
        memory_binding_valid_v1(binding),
    ensures
        binding.mapping_allocation_vm == binding.vm,
        binding.mapping_allocation_id == binding.allocation_id,
        binding.mapping_allocation_generation == binding.allocation_generation,
        binding.mapping_devices =~= binding.vm_mapping_devices,
        binding.vm.device.generation > 0,
        binding.allocation_generation > 0,
{
}

pub proof fn failed_map_records_exact_success_prefix_v1(
    old: MappingProgressV1,
    n_success: nat,
)
    requires
        mapping_progress_valid_v1(old),
        old.state == MappingProgressStateV1::MapPending,
        n_success <= old.device_count,
    ensures
        mapping_progress_valid_v1(observe_failed_map_prefix_v1(old, n_success)),
        observe_failed_map_prefix_v1(old, n_success).mapped_start == 0,
        observe_failed_map_prefix_v1(old, n_success).mapped_end == n_success,
        observe_failed_map_prefix_v1(old, n_success).device_count == old.device_count,
{
}

pub proof fn failed_unmap_uses_absolute_cumulative_progress_v1(
    old: MappingProgressV1,
    n_success: nat,
)
    requires
        mapping_progress_valid_v1(old),
        old.state == MappingProgressStateV1::UnmapPending,
        old.mapped_start <= n_success < old.mapped_end,
    ensures
        mapping_progress_valid_v1(observe_failed_unmap_cumulative_v1(old, n_success)),
        observe_failed_unmap_cumulative_v1(old, n_success).mapped_start == n_success,
        observe_failed_unmap_cumulative_v1(old, n_success).mapped_end == old.mapped_end,
        observe_failed_unmap_cumulative_v1(old, n_success).device_count == old.device_count,
{
}

pub proof fn failed_full_cumulative_unmap_is_ambiguous_v1(old: MappingProgressV1)
    requires
        mapping_progress_valid_v1(old),
        old.state == MappingProgressStateV1::UnmapPending,
    ensures
        mapping_progress_valid_v1(
            observe_failed_unmap_cumulative_v1(old, old.mapped_end),
        ),
        observe_failed_unmap_cumulative_v1(old, old.mapped_end).state
            == MappingProgressStateV1::Ambiguous,
        observe_failed_unmap_cumulative_v1(old, old.mapped_end).mapped_start
            == old.mapped_start,
        observe_failed_unmap_cumulative_v1(old, old.mapped_end).mapped_end
            == old.mapped_end,
{
}

pub proof fn wrong_device_set_is_rejected_without_a_map_state_v1(
    binding: MemoryBindingV1,
    requested_devices: Seq<DeviceKeyV1>,
)
    requires
        memory_binding_valid_v1(binding),
        !(requested_devices =~= binding.vm_mapping_devices),
    ensures
        begin_map_if_exact_device_set_v1(binding, requested_devices).is_none(),
{
}

pub proof fn retained_mapping_or_publication_blocks_allocation_free_v1(
    allocation_id: nat,
    allocation_generation: nat,
    mappings: Seq<MappingRetentionV1>,
    retained_index: int,
)
    requires
        0 <= retained_index < mappings.len(),
        mappings[retained_index].allocation_id == allocation_id,
        mappings[retained_index].allocation_generation == allocation_generation,
        mapping_retains_allocation_v1(mappings[retained_index]),
    ensures
        !can_free_allocation_v1(allocation_id, allocation_generation, mappings),
{
}

} // verus!
