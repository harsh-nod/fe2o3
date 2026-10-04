use vstd::prelude::*;

verus! {

pub open spec fn max_native_mapping_devices_v1() -> nat {
    64
}

pub open spec fn canonical_gpu_ids_v1(gpu_ids: Seq<nat>) -> bool {
    &&& 0 < gpu_ids.len() <= max_native_mapping_devices_v1()
    &&& forall|index: int| 0 <= index < gpu_ids.len() ==> gpu_ids[index] > 0
    &&& forall|left: int, right: int|
        0 <= left < right < gpu_ids.len() ==> gpu_ids[left] < gpu_ids[right]
}

pub proof fn canonical_gpu_ids_are_unique_v1(gpu_ids: Seq<nat>)
    requires canonical_gpu_ids_v1(gpu_ids),
    ensures forall|left: int, right: int|
        0 <= left < gpu_ids.len() && 0 <= right < gpu_ids.len()
            && gpu_ids[left] == gpu_ids[right] ==> left == right,
{
    assert forall|left: int, right: int|
        0 <= left < gpu_ids.len() && 0 <= right < gpu_ids.len()
            && gpu_ids[left] == gpu_ids[right] implies left == right by {
        if left < right {
            assert(gpu_ids[left] < gpu_ids[right]);
        } else if right < left {
            assert(gpu_ids[right] < gpu_ids[left]);
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum NativeMappingPhaseV1 {
    Mapping,
    Active,
    Compensating,
    Compensated,
    Quarantined,
}

pub struct NativeMappingV1 {
    pub operation: nat,
    pub allocation: nat,
    pub gpu_ids: Seq<nat>,
    pub mapped_prefix: nat,
    pub unmapped_prefix: nat,
    pub phase: NativeMappingPhaseV1,
}

pub open spec fn valid_mapping_v1(mapping: NativeMappingV1) -> bool {
    &&& mapping.operation > 0
    &&& mapping.allocation > 0
    &&& canonical_gpu_ids_v1(mapping.gpu_ids)
    &&& mapping.unmapped_prefix <= mapping.mapped_prefix <= mapping.gpu_ids.len()
    &&& (mapping.phase == NativeMappingPhaseV1::Mapping ==>
        mapping.unmapped_prefix == 0)
    &&& (mapping.phase == NativeMappingPhaseV1::Active ==>
        mapping.mapped_prefix == mapping.gpu_ids.len() && mapping.unmapped_prefix == 0)
    &&& (mapping.phase == NativeMappingPhaseV1::Compensating ==>
        mapping.unmapped_prefix < mapping.mapped_prefix)
    &&& (mapping.phase == NativeMappingPhaseV1::Compensated ==>
        mapping.unmapped_prefix == mapping.mapped_prefix)
}

pub open spec fn begin_mapping_v1(
    operation: nat,
    allocation: nat,
    gpu_ids: Seq<nat>,
) -> NativeMappingV1 {
    NativeMappingV1 {
        operation,
        allocation,
        gpu_ids,
        mapped_prefix: 0,
        unmapped_prefix: 0,
        phase: NativeMappingPhaseV1::Mapping,
    }
}

pub proof fn mapping_begins_with_exact_canonical_array_and_zero_prefixes_v1(
    operation: nat,
    allocation: nat,
    gpu_ids: Seq<nat>,
)
    requires operation > 0, allocation > 0, canonical_gpu_ids_v1(gpu_ids),
    ensures {
        let mapping = begin_mapping_v1(operation, allocation, gpu_ids);
        &&& valid_mapping_v1(mapping)
        &&& mapping.operation == operation
        &&& mapping.allocation == allocation
        &&& mapping.gpu_ids == gpu_ids
        &&& mapping.mapped_prefix == 0
        &&& mapping.unmapped_prefix == 0
        &&& mapping.phase == NativeMappingPhaseV1::Mapping
    },
{
}

pub open spec fn observe_failed_map_prefix_v1(
    mapping: NativeMappingV1,
    cumulative_n_success: nat,
) -> NativeMappingV1 {
    NativeMappingV1 {
        mapped_prefix: cumulative_n_success,
        phase: if cumulative_n_success == 0 {
            NativeMappingPhaseV1::Compensated
        } else {
            NativeMappingPhaseV1::Compensating
        },
        ..mapping
    }
}

pub proof fn failed_map_retains_exact_cumulative_prefix_v1(
    mapping: NativeMappingV1,
    cumulative_n_success: nat,
)
    requires
        valid_mapping_v1(mapping),
        mapping.phase == NativeMappingPhaseV1::Mapping,
        mapping.mapped_prefix <= cumulative_n_success <= mapping.gpu_ids.len(),
        cumulative_n_success > 0,
    ensures {
        let failed = observe_failed_map_prefix_v1(mapping, cumulative_n_success);
        &&& valid_mapping_v1(failed)
        &&& failed.gpu_ids == mapping.gpu_ids
        &&& failed.mapped_prefix == cumulative_n_success
        &&& failed.unmapped_prefix == mapping.unmapped_prefix
        &&& failed.phase == NativeMappingPhaseV1::Compensating
    },
{
}

pub open spec fn compensate_prefix_v1(
    mapping: NativeMappingV1,
    cumulative_n_success: nat,
) -> NativeMappingV1 {
    NativeMappingV1 {
        unmapped_prefix: cumulative_n_success,
        phase: if cumulative_n_success == mapping.mapped_prefix {
            NativeMappingPhaseV1::Compensated
        } else {
            NativeMappingPhaseV1::Compensating
        },
        ..mapping
    }
}

pub open spec fn mapping_releasable_v1(mapping: NativeMappingV1) -> bool {
    mapping.phase == NativeMappingPhaseV1::Compensated
        && mapping.unmapped_prefix == mapping.mapped_prefix
}

pub proof fn partial_compensation_retains_prefix_and_blocks_release_v1(
    mapping: NativeMappingV1,
    cumulative_n_success: nat,
)
    requires
        valid_mapping_v1(mapping),
        mapping.phase == NativeMappingPhaseV1::Compensating,
        mapping.unmapped_prefix <= cumulative_n_success < mapping.mapped_prefix,
    ensures {
        let next = compensate_prefix_v1(mapping, cumulative_n_success);
        &&& valid_mapping_v1(next)
        &&& next.mapped_prefix == mapping.mapped_prefix
        &&& next.unmapped_prefix == cumulative_n_success
        &&& next.phase == NativeMappingPhaseV1::Compensating
        &&& !mapping_releasable_v1(next)
    },
{
}

pub proof fn complete_compensation_releases_only_the_exact_mapped_prefix_v1(
    mapping: NativeMappingV1,
)
    requires
        valid_mapping_v1(mapping),
        mapping.phase == NativeMappingPhaseV1::Compensating,
    ensures {
        let next = compensate_prefix_v1(mapping, mapping.mapped_prefix);
        &&& valid_mapping_v1(next)
        &&& next.gpu_ids == mapping.gpu_ids
        &&& next.mapped_prefix == mapping.mapped_prefix
        &&& next.unmapped_prefix == mapping.mapped_prefix
        &&& mapping_releasable_v1(next)
    },
{
}

#[derive(PartialEq, Eq)]
pub struct XgmiRouteV1 {
    pub identity: nat,
    pub topology: nat,
    pub topology_generation: nat,
    pub observation_epoch: nat,
    pub source_device: nat,
    pub source_generation: nat,
    pub destination_device: nat,
    pub destination_generation: nat,
    pub source_gpu_id: nat,
    pub destination_gpu_id: nat,
    pub source_node: nat,
    pub destination_node: nat,
    pub hive: nat,
    pub io_link_index: nat,
    pub link_type: nat,
    pub min_bandwidth: nat,
    pub max_bandwidth: nat,
    pub recommended_transfer_size: nat,
    pub recommended_engine_mask: nat,
    pub selected_engine: nat,
    pub link_flags: nat,
    pub peer_access: bool,
    pub xgmi_queue: bool,
}

#[derive(PartialEq, Eq)]
pub struct XgmiCurrentnessV1 {
    pub route_identity: nat,
    pub topology: nat,
    pub topology_generation: nat,
    pub observation_epoch: nat,
    pub source_device: nat,
    pub source_generation: nat,
    pub destination_device: nat,
    pub destination_generation: nat,
    pub source_gpu_id: nat,
    pub destination_gpu_id: nat,
    pub source_node: nat,
    pub destination_node: nat,
    pub hive: nat,
    pub io_link_index: nat,
    pub link_type: nat,
    pub min_bandwidth: nat,
    pub max_bandwidth: nat,
    pub recommended_transfer_size: nat,
    pub recommended_engine_mask: nat,
    pub selected_engine: nat,
    pub link_flags: nat,
    pub reset_fence_current: bool,
}

pub open spec fn valid_xgmi_route_v1(route: XgmiRouteV1) -> bool {
    &&& route.identity > 0
    &&& route.topology > 0
    &&& route.topology_generation > 0
    &&& route.observation_epoch > 0
    &&& route.source_device > 0
    &&& route.source_generation > 0
    &&& route.destination_device > 0
    &&& route.destination_generation > 0
    &&& route.source_device != route.destination_device
    &&& route.source_gpu_id > 0
    &&& route.destination_gpu_id > 0
    &&& route.source_gpu_id != route.destination_gpu_id
    &&& route.source_node != route.destination_node
    &&& route.hive > 0
    &&& route.link_type == 11
    &&& route.max_bandwidth > 0
    &&& route.min_bandwidth <= route.max_bandwidth
    &&& route.recommended_engine_mask > 0
    &&& 2 <= route.selected_engine < 16
    &&& route.link_flags % 2 == 1
    &&& route.peer_access
    &&& route.xgmi_queue
}

pub open spec fn xgmi_route_current_v1(
    route: XgmiRouteV1,
    current: XgmiCurrentnessV1,
) -> bool {
    &&& current.route_identity == route.identity
    &&& current.topology == route.topology
    &&& current.topology_generation == route.topology_generation
    &&& current.observation_epoch == route.observation_epoch
    &&& current.source_device == route.source_device
    &&& current.source_generation == route.source_generation
    &&& current.destination_device == route.destination_device
    &&& current.destination_generation == route.destination_generation
    &&& current.source_gpu_id == route.source_gpu_id
    &&& current.destination_gpu_id == route.destination_gpu_id
    &&& current.source_node == route.source_node
    &&& current.destination_node == route.destination_node
    &&& current.hive == route.hive
    &&& current.io_link_index == route.io_link_index
    &&& current.link_type == route.link_type
    &&& current.min_bandwidth == route.min_bandwidth
    &&& current.max_bandwidth == route.max_bandwidth
    &&& current.recommended_transfer_size == route.recommended_transfer_size
    &&& current.recommended_engine_mask == route.recommended_engine_mask
    &&& current.selected_engine == route.selected_engine
    &&& current.link_flags == route.link_flags
    &&& current.reset_fence_current
}

pub open spec fn xgmi_route_admitted_v1(
    route: XgmiRouteV1,
    current: XgmiCurrentnessV1,
) -> bool {
    valid_xgmi_route_v1(route) && xgmi_route_current_v1(route, current)
}

pub proof fn admitted_xgmi_route_retains_exact_direction_and_currentness_v1(
    route: XgmiRouteV1,
    current: XgmiCurrentnessV1,
)
    requires xgmi_route_admitted_v1(route, current),
    ensures
        current.route_identity == route.identity,
        current.source_device == route.source_device,
        current.source_generation == route.source_generation,
        current.destination_device == route.destination_device,
        current.destination_generation == route.destination_generation,
        current.topology == route.topology,
        current.topology_generation == route.topology_generation,
        current.hive == route.hive,
        current.link_type == route.link_type,
        current.min_bandwidth == route.min_bandwidth,
        current.max_bandwidth == route.max_bandwidth,
        current.recommended_engine_mask == route.recommended_engine_mask,
        current.selected_engine == route.selected_engine,
        current.reset_fence_current,
{
}

pub proof fn reversed_xgmi_direction_is_not_current_v1(
    route: XgmiRouteV1,
    current: XgmiCurrentnessV1,
)
    requires
        valid_xgmi_route_v1(route),
        current.source_device == route.destination_device,
        current.destination_device == route.source_device,
    ensures !xgmi_route_current_v1(route, current),
{
}

pub proof fn stale_xgmi_topology_generation_blocks_admission_v1(
    route: XgmiRouteV1,
    current: XgmiCurrentnessV1,
)
    requires current.topology_generation != route.topology_generation,
    ensures !xgmi_route_admitted_v1(route, current),
{
}

#[derive(PartialEq, Eq)]
pub struct NativeCopyRangeV1 {
    pub owner: nat,
    pub start: nat,
    pub length: nat,
}

pub open spec fn native_copy_ranges_nonoverlapping_v1(
    source: NativeCopyRangeV1,
    destination: NativeCopyRangeV1,
) -> bool {
    &&& source.length > 0
    &&& destination.length == source.length
    &&& source != destination
    &&& (source.owner != destination.owner
        || source.start + source.length <= destination.start
        || destination.start + destination.length <= source.start)
}

#[derive(PartialEq, Eq)]
pub enum NativeXgmiCopyPhaseV1 {
    Reserved,
    Published,
    Complete,
    Quarantined,
}

pub struct NativeXgmiCopyV1 {
    pub identity: nat,
    pub route_identity: nat,
    pub selected_engine: nat,
    pub source_mapping_operation: nat,
    pub destination_mapping_operation: nat,
    pub source: NativeCopyRangeV1,
    pub destination: NativeCopyRangeV1,
    pub phase: NativeXgmiCopyPhaseV1,
}

pub open spec fn native_xgmi_copy_ready_v1(
    copy: NativeXgmiCopyV1,
    route: XgmiRouteV1,
    current: XgmiCurrentnessV1,
    source_mapping: NativeMappingV1,
    destination_mapping: NativeMappingV1,
) -> bool {
    &&& copy.identity > 0
    &&& copy.phase == NativeXgmiCopyPhaseV1::Reserved
    &&& copy.route_identity == route.identity
    &&& copy.selected_engine == route.selected_engine
    &&& xgmi_route_admitted_v1(route, current)
    &&& valid_mapping_v1(source_mapping)
    &&& valid_mapping_v1(destination_mapping)
    &&& source_mapping.phase == NativeMappingPhaseV1::Active
    &&& destination_mapping.phase == NativeMappingPhaseV1::Active
    &&& source_mapping.mapped_prefix == source_mapping.gpu_ids.len()
    &&& destination_mapping.mapped_prefix == destination_mapping.gpu_ids.len()
    &&& source_mapping.gpu_ids == destination_mapping.gpu_ids
    &&& copy.source_mapping_operation == source_mapping.operation
    &&& copy.destination_mapping_operation == destination_mapping.operation
    &&& copy.source.owner == source_mapping.allocation
    &&& copy.destination.owner == destination_mapping.allocation
    &&& native_copy_ranges_nonoverlapping_v1(copy.source, copy.destination)
}

pub open spec fn publish_native_xgmi_copy_v1(
    copy: NativeXgmiCopyV1,
    route: XgmiRouteV1,
    current: XgmiCurrentnessV1,
    source_mapping: NativeMappingV1,
    destination_mapping: NativeMappingV1,
) -> NativeXgmiCopyV1 {
    if native_xgmi_copy_ready_v1(
        copy,
        route,
        current,
        source_mapping,
        destination_mapping,
    ) {
        NativeXgmiCopyV1 { phase: NativeXgmiCopyPhaseV1::Published, ..copy }
    } else {
        copy
    }
}

pub proof fn native_xgmi_copy_publication_requires_current_full_mappings_v1(
    copy: NativeXgmiCopyV1,
    route: XgmiRouteV1,
    current: XgmiCurrentnessV1,
    source_mapping: NativeMappingV1,
    destination_mapping: NativeMappingV1,
)
    requires native_xgmi_copy_ready_v1(
        copy,
        route,
        current,
        source_mapping,
        destination_mapping,
    ),
    ensures {
        let published = publish_native_xgmi_copy_v1(
            copy,
            route,
            current,
            source_mapping,
            destination_mapping,
        );
        &&& published.phase == NativeXgmiCopyPhaseV1::Published
        &&& published.identity == copy.identity
        &&& published.route_identity == route.identity
        &&& published.selected_engine == route.selected_engine
        &&& published.source_mapping_operation == source_mapping.operation
        &&& published.destination_mapping_operation == destination_mapping.operation
        &&& published.source == copy.source
        &&& published.destination == copy.destination
        &&& published.source.owner == source_mapping.allocation
        &&& published.destination.owner == destination_mapping.allocation
        &&& native_copy_ranges_nonoverlapping_v1(published.source, published.destination)
        &&& source_mapping.phase == NativeMappingPhaseV1::Active
        &&& destination_mapping.phase == NativeMappingPhaseV1::Active
        &&& xgmi_route_current_v1(route, current)
    },
{
}

#[derive(PartialEq, Eq)]
pub enum NativeXgmiCompletionV1 {
    Succeeded,
    TimedOut,
    Indeterminate,
}

pub open spec fn observe_native_xgmi_completion_v1(
    copy: NativeXgmiCopyV1,
    completion: NativeXgmiCompletionV1,
) -> NativeXgmiCopyV1 {
    if copy.phase == NativeXgmiCopyPhaseV1::Published {
        if completion == NativeXgmiCompletionV1::Succeeded {
            NativeXgmiCopyV1 { phase: NativeXgmiCopyPhaseV1::Complete, ..copy }
        } else {
            NativeXgmiCopyV1 { phase: NativeXgmiCopyPhaseV1::Quarantined, ..copy }
        }
    } else {
        copy
    }
}

pub open spec fn native_xgmi_copy_owners_releasable_v1(copy: NativeXgmiCopyV1) -> bool {
    copy.phase == NativeXgmiCopyPhaseV1::Complete
}

pub proof fn uncertain_xgmi_completion_retains_both_owners_v1(
    copy: NativeXgmiCopyV1,
    completion: NativeXgmiCompletionV1,
)
    requires
        copy.phase == NativeXgmiCopyPhaseV1::Published,
        completion != NativeXgmiCompletionV1::Succeeded,
    ensures {
        let quarantined = observe_native_xgmi_completion_v1(copy, completion);
        &&& quarantined.phase == NativeXgmiCopyPhaseV1::Quarantined
        &&& quarantined.source_mapping_operation == copy.source_mapping_operation
        &&& quarantined.destination_mapping_operation == copy.destination_mapping_operation
        &&& quarantined.source.owner == copy.source.owner
        &&& quarantined.destination.owner == copy.destination.owner
        &&& !native_xgmi_copy_owners_releasable_v1(quarantined)
    },
{
}

#[derive(PartialEq, Eq)]
pub struct MachineEvidenceV1 {
    pub attestation: nat,
    pub artifact: nat,
    pub elf_machine: nat,
    pub code_object_version: nat,
    pub architecture: nat,
    pub wavefront_size: nat,
    pub xnack_disabled: bool,
    pub symbol: nat,
    pub descriptor: nat,
    pub machine_code: nat,
    // This is only the identity of a checked instruction-class receipt.
    pub checked_instruction_class_receipt: nat,
    pub semantic_contract: nat,
    pub kernel_identity: nat,
    pub toolchain: nat,
}

#[derive(PartialEq, Eq)]
pub struct LoadedMachineEvidenceV1 {
    pub loaded_code: nat,
    pub device: nat,
    pub artifact: nat,
    pub elf_machine: nat,
    pub code_object_version: nat,
    pub architecture: nat,
    pub wavefront_size: nat,
    pub xnack_disabled: bool,
    pub symbol: nat,
    pub descriptor: nat,
    pub machine_code: nat,
    pub checked_instruction_class_receipt: nat,
}

pub open spec fn exact_gfx942_cov6_target_v1(evidence: MachineEvidenceV1) -> bool {
    &&& evidence.elf_machine == 224
    &&& evidence.code_object_version == 6
    &&& evidence.architecture == 942
    &&& evidence.wavefront_size == 64
    &&& evidence.xnack_disabled
}

pub open spec fn machine_evidence_matches_v1(
    evidence: MachineEvidenceV1,
    loaded: LoadedMachineEvidenceV1,
    semantic_contract: nat,
    kernel_identity: nat,
) -> bool {
    &&& evidence.attestation > 0
    &&& evidence.artifact > 0
    &&& evidence.symbol > 0
    &&& evidence.descriptor > 0
    &&& evidence.machine_code > 0
    &&& evidence.checked_instruction_class_receipt > 0
    &&& evidence.toolchain > 0
    &&& exact_gfx942_cov6_target_v1(evidence)
    &&& evidence.semantic_contract == semantic_contract
    &&& evidence.kernel_identity == kernel_identity
    &&& loaded.loaded_code > 0
    &&& loaded.device > 0
    &&& loaded.artifact == evidence.artifact
    &&& loaded.elf_machine == evidence.elf_machine
    &&& loaded.code_object_version == evidence.code_object_version
    &&& loaded.architecture == evidence.architecture
    &&& loaded.wavefront_size == evidence.wavefront_size
    &&& loaded.xnack_disabled == evidence.xnack_disabled
    &&& loaded.symbol == evidence.symbol
    &&& loaded.descriptor == evidence.descriptor
    &&& loaded.machine_code == evidence.machine_code
    &&& loaded.checked_instruction_class_receipt
        == evidence.checked_instruction_class_receipt
}

pub proof fn matching_machine_evidence_retains_every_exact_coordinate_v1(
    evidence: MachineEvidenceV1,
    loaded: LoadedMachineEvidenceV1,
    semantic_contract: nat,
    kernel_identity: nat,
)
    requires machine_evidence_matches_v1(
        evidence,
        loaded,
        semantic_contract,
        kernel_identity,
    ),
    ensures
        loaded.artifact == evidence.artifact,
        loaded.symbol == evidence.symbol,
        loaded.descriptor == evidence.descriptor,
        loaded.machine_code == evidence.machine_code,
        loaded.checked_instruction_class_receipt
            == evidence.checked_instruction_class_receipt,
        evidence.semantic_contract == semantic_contract,
        evidence.kernel_identity == kernel_identity,
        exact_gfx942_cov6_target_v1(evidence),
{
}

pub proof fn substituted_instruction_class_receipt_rejects_binding_v1(
    evidence: MachineEvidenceV1,
    loaded: LoadedMachineEvidenceV1,
    semantic_contract: nat,
    kernel_identity: nat,
)
    requires loaded.checked_instruction_class_receipt
        != evidence.checked_instruction_class_receipt,
    ensures !machine_evidence_matches_v1(
        evidence,
        loaded,
        semantic_contract,
        kernel_identity,
    ),
{
}

#[derive(PartialEq, Eq)]
pub struct DispatchCurrentnessV1 {
    pub loaded_code: nat,
    pub device: nat,
    pub artifact: nat,
    pub elf_machine: nat,
    pub code_object_version: nat,
    pub architecture: nat,
    pub wavefront_size: nat,
    pub xnack_disabled: bool,
    pub attestation: nat,
    pub symbol: nat,
    pub descriptor: nat,
    pub machine_code: nat,
    pub checked_instruction_class_receipt: nat,
    pub semantic_contract: nat,
    pub kernel_identity: nat,
    pub toolchain: nat,
    pub device_current: bool,
    pub code_current: bool,
    pub mappings_current: bool,
    pub queue_current: bool,
    pub reset_fence_current: bool,
    pub dependency_frontier: nat,
    pub completed_frontier: nat,
}

pub open spec fn dispatch_evidence_current_v1(
    evidence: MachineEvidenceV1,
    loaded: LoadedMachineEvidenceV1,
    current: DispatchCurrentnessV1,
) -> bool {
    &&& current.loaded_code == loaded.loaded_code
    &&& current.device == loaded.device
    &&& current.artifact == loaded.artifact
    &&& current.elf_machine == loaded.elf_machine
    &&& current.code_object_version == loaded.code_object_version
    &&& current.architecture == loaded.architecture
    &&& current.wavefront_size == loaded.wavefront_size
    &&& current.xnack_disabled == loaded.xnack_disabled
    &&& current.attestation == evidence.attestation
    &&& current.symbol == loaded.symbol
    &&& current.descriptor == loaded.descriptor
    &&& current.machine_code == loaded.machine_code
    &&& current.checked_instruction_class_receipt
        == loaded.checked_instruction_class_receipt
    &&& current.semantic_contract == evidence.semantic_contract
    &&& current.kernel_identity == evidence.kernel_identity
    &&& current.toolchain == evidence.toolchain
    &&& current.device_current
    &&& current.code_current
    &&& current.mappings_current
    &&& current.queue_current
    &&& current.reset_fence_current
    &&& current.completed_frontier >= current.dependency_frontier
}

pub proof fn any_stale_execution_surface_blocks_dispatch_v1(
    evidence: MachineEvidenceV1,
    loaded: LoadedMachineEvidenceV1,
    current: DispatchCurrentnessV1,
)
    requires
        !current.device_current || !current.code_current || !current.mappings_current
            || !current.queue_current || !current.reset_fence_current,
    ensures !dispatch_evidence_current_v1(evidence, loaded, current),
{
}

#[derive(PartialEq, Eq)]
pub enum DispatchPhaseV1 {
    Reserved,
    Published,
}

pub open spec fn publish_evidence_bound_dispatch_v1(
    evidence: MachineEvidenceV1,
    loaded: LoadedMachineEvidenceV1,
    current: DispatchCurrentnessV1,
) -> DispatchPhaseV1 {
    if dispatch_evidence_current_v1(evidence, loaded, current) {
        DispatchPhaseV1::Published
    } else {
        DispatchPhaseV1::Reserved
    }
}

pub proof fn dispatch_publishes_only_after_exact_current_evidence_v1(
    evidence: MachineEvidenceV1,
    loaded: LoadedMachineEvidenceV1,
    current: DispatchCurrentnessV1,
)
    ensures publish_evidence_bound_dispatch_v1(evidence, loaded, current)
        == DispatchPhaseV1::Published ==> dispatch_evidence_current_v1(evidence, loaded, current),
{
}

} // verus!
