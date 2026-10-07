// Exact immutable packet/code metadata preparation. The schemas below preserve
// all raw fields, including unused digests and numeric identity wrappers. This
// is not a proof of metadata construction, resource authority, outer validation,
// Box conversion, roster hashing, epoch mutation, or native dispatch. Heap cost,
// allocator failure and unwinding are outside the normal-return contract.
use vstd::prelude::*;

include!("../../fe2o3-kfd/src/queue_dispatch_binding/template_prepare_body.rs");

verus! {
#[derive(Clone, Copy)]
struct PhysicalDeviceIdV1(u64);
#[derive(Clone, Copy)]
struct DeviceGenerationV1(u64);
#[derive(Clone, Copy)]
struct VmIdV1(u64);
#[derive(Clone, Copy)]
struct AllocationIdV1(u64);
#[derive(Clone, Copy)]
struct AllocationGenerationV1(u64);
#[derive(Clone, Copy)]
struct MappingIdV1(u64);
#[derive(Clone, Copy)]
struct QueueInstanceIdV1(u64);
#[derive(Clone, Copy)]
struct QueueGenerationV1(u64);
#[derive(Clone, Copy)]
struct DeviceKeyV1 { physical: PhysicalDeviceIdV1, generation: DeviceGenerationV1 }
#[derive(Clone, Copy)]
struct VmKeyV1 { device: DeviceKeyV1, id: VmIdV1 }
#[derive(Clone, Copy)]
struct QueueKeyV1 { vm: VmKeyV1, id: QueueInstanceIdV1, generation: QueueGenerationV1 }
#[derive(Clone, Copy)]
struct MemoryAllocationKeyV1 { vm: VmKeyV1, id: AllocationIdV1, generation: AllocationGenerationV1 }
#[derive(Clone, Copy)]
struct MemoryMappingKeyV1 { allocation: MemoryAllocationKeyV1, id: MappingIdV1 }
#[derive(Clone, Copy)]
struct ObservedGpuAddressV1(u64);
#[derive(Clone, Copy)]
struct AqlDispatchGeometryV1 { grid: [u32; 3], workgroup: [u16; 3], dimensions: u16 }
#[derive(Clone, Copy)]
enum AqlDispatchOrderingV1 { Independent, WaitForPrior }
#[derive(Clone, Copy)]
struct KernelIdentityInputsV1 {
    object_sha256: [u8; 32], metadata_sha256: [u8; 32], descriptor_sha256: [u8; 32],
    entry_sha256: [u8; 32], closure_sha256: [u8; 32],
}
#[derive(Clone, Copy)]
struct ResolvedCodeIdentityV1 {
    authenticated: KernelIdentityInputsV1, dispatch_abi_identity: [u8; 32],
    materialized_sha256: [u8; 32], mapping: MemoryMappingKeyV1,
    descriptor_address: ObservedGpuAddressV1,
}
#[derive(Clone, Copy)]
struct PreparedDispatchPacketV1 {
    geometry: AqlDispatchGeometryV1, ordering: AqlDispatchOrderingV1,
    private_segment_size: u32, group_segment_size: u32,
    kernarg_address: ObservedGpuAddressV1, kernarg_alignment: u64,
    kernarg_mapping: MemoryMappingKeyV1, kernarg_layout_identity: [u8; 32],
    code_bound_kernarg_layout: bool, code_index: usize, conditional_fill: bool,
}
#[derive(Clone, Copy)]
struct CompletionDispatchGenerationBindingV1 {
    queue: QueueKeyV1, code: MemoryMappingKeyV1, kernarg: MemoryMappingKeyV1,
    dispatch_generation: u64,
}
#[derive(Clone, Copy)]
struct CompletionPacketTemplateV1 {
    geometry: AqlDispatchGeometryV1, ordering: AqlDispatchOrderingV1,
    private_segment_size: u32, group_segment_size: u32,
    kernel_object: ObservedGpuAddressV1, kernarg_address: ObservedGpuAddressV1,
    kernarg_alignment: u64, generations: CompletionDispatchGenerationBindingV1,
}
enum Gfx942DispatchBindingErrorV1 {
    InvalidCode(&'static str),
    InvalidKernarg { packet: usize, detail: &'static str },
}

fn prepared_kernarg_layout_matches_code(code_bound: bool, kernarg_layout_identity: [u8; 32],
    dispatch_abi_identity: [u8; 32]) -> (out: bool)
    ensures out == (!code_bound || kernarg_layout_identity == dispatch_abi_identity),
{
    dispatch_template_abi_matches_body!(@annotated verus_exec_expr, code_bound,
        kernarg_layout_identity, dispatch_abi_identity, difference, index,
        [invariant 0 <= index <= 32, code_bound,
            (difference == 0) == (forall|i: int| 0 <= i < index ==>
                kernarg_layout_identity[i] == dispatch_abi_identity[i]),
         decreases 32 - index,],
        [let ghost previous = difference;],
        [proof {
            byte_difference_zero(previous, kernarg_layout_identity[index as int],
                dispatch_abi_identity[index as int]);
            assert((difference == 0) == (forall|i: int| 0 <= i < index + 1 ==>
                kernarg_layout_identity[i] == dispatch_abi_identity[i]));
        }],
        [proof {
            if difference == 0 { assert(kernarg_layout_identity =~= dispatch_abi_identity); }
        }])
}

proof fn byte_difference_zero(previous: u8, left: u8, right: u8)
    ensures ((previous | (left ^ right)) == 0) == (previous == 0 && left == right),
{
    assert(((previous | (left ^ right)) == 0) == (previous == 0 && left == right)) by(bit_vector);
}

impl CompletionDispatchGenerationBindingV1 {
    fn new(queue: QueueKeyV1, code: MemoryMappingKeyV1, kernarg: MemoryMappingKeyV1,
        dispatch_generation: u64) -> (out: Self)
        ensures out == (Self { queue, code, kernarg, dispatch_generation }),
    {
        dispatch_template_generation_new_body!(verus_exec_expr, queue, code,
            kernarg, dispatch_generation)
    }
}

impl CompletionPacketTemplateV1 {
    fn new(geometry: AqlDispatchGeometryV1, ordering: AqlDispatchOrderingV1,
        private_segment_size: u32, group_segment_size: u32,
        kernel_object: ObservedGpuAddressV1, kernarg_address: ObservedGpuAddressV1,
        kernarg_alignment: u64, generations: CompletionDispatchGenerationBindingV1) -> (out: Self)
        ensures out == (Self { geometry, ordering, private_segment_size, group_segment_size,
            kernel_object, kernarg_address, kernarg_alignment, generations }),
    {
        dispatch_template_new_body!(verus_exec_expr, geometry, ordering,
            private_segment_size, group_segment_size, kernel_object, kernarg_address,
            kernarg_alignment, generations)
    }
}

spec fn template_entry_error(packets: Seq<PreparedDispatchPacketV1>, codes: Seq<ResolvedCodeIdentityV1>,
    index: int) -> Option<Gfx942DispatchBindingErrorV1>
{
    let packet = packets[index];
    if packet.code_index >= codes.len() {
        Some(Gfx942DispatchBindingErrorV1::InvalidCode("packet program index"))
    } else if packet.code_bound_kernarg_layout
        && packet.kernarg_layout_identity != codes[packet.code_index as int].dispatch_abi_identity {
        Some(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet: index as usize, detail: "prepared kernarg dispatch ABI identity",
        })
    } else { None }
}

spec fn template_scan_error(packets: Seq<PreparedDispatchPacketV1>, codes: Seq<ResolvedCodeIdentityV1>,
    index: int) -> Option<Gfx942DispatchBindingErrorV1>
    decreases packets.len() - index,
{
    if index < 0 || index >= packets.len() { None }
    else {
        match template_entry_error(packets, codes, index) {
            Some(error) => Some(error),
            None => template_scan_error(packets, codes, index + 1),
        }
    }
}

spec fn template_value(packet: PreparedDispatchPacketV1, code: ResolvedCodeIdentityV1,
    queue: QueueKeyV1, generation: u64) -> CompletionPacketTemplateV1
{
    CompletionPacketTemplateV1 {
        geometry: packet.geometry, ordering: packet.ordering,
        private_segment_size: packet.private_segment_size,
        group_segment_size: packet.group_segment_size,
        kernel_object: code.descriptor_address, kernarg_address: packet.kernarg_address,
        kernarg_alignment: packet.kernarg_alignment,
        generations: CompletionDispatchGenerationBindingV1 {
            queue, code: code.mapping, kernarg: packet.kernarg_mapping,
            dispatch_generation: generation,
        },
    }
}

fn prepare_dispatch_templates_v1(packets: &[PreparedDispatchPacketV1],
    code_identity: &[ResolvedCodeIdentityV1], queue: QueueKeyV1, generation: u64)
    -> (out: Result<Vec<CompletionPacketTemplateV1>, Gfx942DispatchBindingErrorV1>)
    ensures out.is_ok() == template_scan_error(packets@, code_identity@, 0).is_none(),
        match out {
            Err(error) => Some(error) == template_scan_error(packets@, code_identity@, 0),
            Ok(templates) => {
                &&& templates.len() == packets.len()
                &&& forall|i: int| 0 <= i < packets.len() ==> {
                    &&& packets@[i].code_index < code_identity.len()
                    &&& template_entry_error(packets@, code_identity@, i).is_none()
                    &&& templates@[i] == template_value(packets@[i],
                        code_identity@[packets@[i].code_index as int], queue, generation)
                }
            },
        },
{
    dispatch_prepare_templates_body!(@annotated verus_exec_expr, packets, code_identity,
        queue, generation, templates, index,
        [invariant 0 <= index <= packets.len(), templates.len() == index,
            template_scan_error(packets@, code_identity@, 0)
                == template_scan_error(packets@, code_identity@, index as int),
            forall|i: int| 0 <= i < index ==> {
                &&& packets@[i].code_index < code_identity.len()
                &&& template_entry_error(packets@, code_identity@, i).is_none()
                &&& templates@[i] == template_value(packets@[i],
                    code_identity@[packets@[i].code_index as int], queue, generation)
            },
         decreases packets.len() - index,],
        [proof { reveal(template_scan_error); reveal(template_entry_error); }],
        [proof {
            assert(template_entry_error(packets@, code_identity@, index as int).is_none());
            assert(templates@[index as int] == template_value(packets@[index as int],
                code_identity@[packets@[index as int].code_index as int], queue, generation));
        }])
}
}
