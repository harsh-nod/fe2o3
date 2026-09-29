// Read-only host preflight on retained facts, composed with the actual epoch
// preflight body. Token/lease/role payloads and unused owner fields are arbitrary
// non-Copy values. The existing epoch identity projection unwraps numeric IDs.
// Getter bodies below are exact field reads authenticated against production;
// this does not establish that those facts describe a current native mapping.
// Packet construction, allocation, hashing, reservation and publication are out
// of scope. Raw zero generations and unusual epoch table lengths remain valid
// inputs to this boundary; later stages may reject them.
include!("dispatch_epoch_reserve_v1.rs");
include!("../../fe2o3-kfd/src/queue_dispatch_binding/template_preflight_body.rs");

verus! {
const AQL_MAX_FIXED_BATCH_PACKETS_V2: u32 = 8192;
#[derive(Clone, Copy)]
struct MemoryPublicationKeyV1 { mapping: MemoryMappingKeyV1, id: u64 }
#[derive(Clone, Copy)]
struct SharedGttMappedResourceFactsV1 {
    gpu_va: u64, logical_bytes: usize, cpu_mapping_bytes: usize, gpu_va_bytes: u64,
    mapping: MemoryMappingKeyV1, publication: MemoryPublicationKeyV1,
}
#[derive(Clone, Copy)]
struct Gfx942DeviceMemoryLayoutV1 {
    requested_bytes: u64, backing_bytes: u64, alignment: u64, uapi_flags: u32,
}
#[derive(Clone, Copy)]
struct Gfx942DeviceMemoryDispatchFactsV1 {
    id: u64, generation: u64, device: DeviceKeyV1, vm: VmKeyV1,
    gpu_va: u64, layout: Gfx942DeviceMemoryLayoutV1,
}
// C represents the untouched allocation token together with its role marker.
struct SharedGttQueueResourceAuthorityV1<C> {
    credits: C, facts: SharedGttMappedResourceFactsV1,
}
struct Gfx942DeviceMemoryDispatchAuthorityV1<D> {
    lease: D, facts: Gfx942DeviceMemoryDispatchFactsV1,
}
enum DispatchDataAuthorityV1<D, H> {
    Device(Gfx942DeviceMemoryDispatchAuthorityV1<D>),
    HostVisible(SharedGttQueueResourceAuthorityV1<H>),
}
#[derive(Clone, Copy)]
struct ObservedGpuAddressV1(u64);
#[derive(Clone, Copy)]
struct AqlDispatchGeometryV1 { grid: [u32; 3], workgroup: [u16; 3], dimensions: u16 }
#[derive(Clone, Copy, PartialEq, Eq)]
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
    code_bound_kernarg_layout: bool, code_index: usize,
}
// Code authority contents, data premises and persistent control are opaque.
// Their actual Vec cardinalities and all resource fact fields remain explicit.
struct DispatchResourceOwnerV1<C, K, D, H, P, T, E> {
    code: Vec<C>, code_identity: Vec<ResolvedCodeIdentityV1>,
    kernarg: SharedGttQueueResourceAuthorityV1<K>, packets: Vec<PreparedDispatchPacketV1>,
    data: Vec<DispatchDataAuthorityV1<D, H>>, data_premises: Vec<P>,
    generation: DispatchGenerationOwnerV1<E>, persistent_control: T,
}
}

structural_eq!(AqlDispatchOrderingV1);

verus! {
impl SharedGttMappedResourceFactsV1 {
    fn mapping(&self) -> (out: MemoryMappingKeyV1)
        ensures out == self.mapping,
    { self.mapping }
}
impl<C> SharedGttQueueResourceAuthorityV1<C> {
    fn facts(&self) -> (out: &SharedGttMappedResourceFactsV1)
        ensures *out == self.facts,
    { &self.facts }
}
impl Gfx942DeviceMemoryDispatchFactsV1 {
    fn vm(&self) -> (out: VmKeyV1)
        ensures out == self.vm,
    { self.vm }
}
impl<D> Gfx942DeviceMemoryDispatchAuthorityV1<D> {
    fn facts(&self) -> (out: &Gfx942DeviceMemoryDispatchFactsV1)
        ensures *out == self.facts,
    { &self.facts }
}
spec fn data_vm<D, H>(authority: DispatchDataAuthorityV1<D, H>) -> VmKeyV1 {
    match authority {
        DispatchDataAuthorityV1::Device(value) => value.facts.vm,
        DispatchDataAuthorityV1::HostVisible(value) => value.facts.mapping.allocation.vm,
    }
}
impl<D, H> DispatchDataAuthorityV1<D, H> {
    fn vm(&self) -> (out: VmKeyV1)
        ensures out == data_vm(*self),
    { dispatch_template_data_vm_body!(verus_exec_expr, self) }
}

fn validate_packet_count<const N: usize>() -> (out: Result<(), Gfx942DispatchBindingErrorV1>)
    ensures out == if N == 0 { Err(Gfx942DispatchBindingErrorV1::ZeroPacketCount) }
        else if N > 8192 { Err(Gfx942DispatchBindingErrorV1::PacketCountExceedsMaximum {
            requested: N, maximum: 8192 }) }
        else { Ok(()) },
{ dispatch_template_packet_count_body!(verus_exec_expr, N) }

spec fn cardinalities_match<C, K, D, H, P, T, E>(owner: &DispatchResourceOwnerV1<C, K, D, H, P, T, E>, n: usize) -> bool {
    &&& owner.packets.len() == n
    &&& owner.code_identity.len() == owner.code.len()
    &&& owner.data.len() == owner.data_premises.len()
}
spec fn code_vms_match(codes: Seq<ResolvedCodeIdentityV1>, vm: VmKeyV1) -> bool {
    forall|i: int| 0 <= i < codes.len() ==> codes[i].mapping.allocation.vm == vm
}
spec fn data_vms_match<D, H>(data: Seq<DispatchDataAuthorityV1<D, H>>, vm: VmKeyV1) -> bool {
    forall|i: int| 0 <= i < data.len() ==> data_vm(data[i]) == vm
}
spec fn resource_shape_matches<C, K, D, H, P, T, E>(owner: &DispatchResourceOwnerV1<C, K, D, H, P, T, E>,
    n: usize, queue: QueueKeyV1) -> bool
{
    &&& cardinalities_match(owner, n)
    &&& code_vms_match(owner.code_identity@, queue.vm)
    &&& owner.kernarg.facts.mapping.allocation.vm == queue.vm
    &&& data_vms_match(owner.data@, queue.vm)
}
spec fn template_order_error(packets: Seq<PreparedDispatchPacketV1>, index: int)
    -> Option<Gfx942DispatchBindingErrorV1>
    decreases packets.len() - index,
{
    if index < 0 || index >= packets.len() { None }
    else if packets[index].ordering != AqlDispatchOrderingV1::WaitForPrior {
        Some(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet: index as usize, detail: "multi-inflight recipe requires wait-for-prior ordering",
        })
    } else { template_order_error(packets, index + 1) }
}
spec fn template_preflight_error<C, K, D, H, P, T, E>(owner: &DispatchResourceOwnerV1<C, K, D, H, P, T, E>,
    n: usize, queue: QueueKeyV1) -> Option<Gfx942DispatchBindingErrorV1>
{
    if owner.generation.poisoned { Some(Gfx942DispatchBindingErrorV1::Poisoned) }
    else if n == 0 { Some(Gfx942DispatchBindingErrorV1::ZeroPacketCount) }
    else if n > 8192 { Some(Gfx942DispatchBindingErrorV1::PacketCountExceedsMaximum { requested: n, maximum: 8192 }) }
    else if !resource_shape_matches(owner, n, queue) { Some(Gfx942DispatchBindingErrorV1::WrongQueueGeneration) }
    else if template_order_error(owner.packets@, 0).is_some() { template_order_error(owner.packets@, 0) }
    else { preflight_error(owner.generation.state(), queue) }
}

impl<C, K, D, H, P, T, E> DispatchResourceOwnerV1<C, K, D, H, P, T, E> {
    fn preflight_templates<const N: usize>(&self, queue: QueueKeyV1)
        -> (out: Result<u64, Gfx942DispatchBindingErrorV1>)
        ensures out == match template_preflight_error(self, N, queue) {
            Some(error) => Err(error), None => Ok(self.generation.next_generation),
        },
            out.is_ok() ==> self.generation.next_generation < u64::MAX,
    {
        dispatch_template_preflight_body!(@annotated verus_exec_expr, self, N, queue, index,
            [invariant 0 <= index <= self.code_identity.len(), !self.generation.poisoned,
                0 < N <= 8192, cardinalities_match(self, N),
                forall|i: int| 0 <= i < index ==> self.code_identity@[i].mapping.allocation.vm == queue.vm,
             decreases self.code_identity.len() - index,],
            [proof {
                assert forall|i: int| 0 <= i < index + 1 implies self.code_identity@[i].mapping.allocation.vm == queue.vm by {
                    if i < index { assert(self.code_identity@[i].mapping.allocation.vm == queue.vm); }
                }
            }],
            [invariant 0 <= index <= self.data.len(), !self.generation.poisoned,
                0 < N <= 8192, cardinalities_match(self, N), code_vms_match(self.code_identity@, queue.vm),
                self.kernarg.facts.mapping.allocation.vm == queue.vm,
                forall|i: int| 0 <= i < index ==> data_vm(self.data@[i]) == queue.vm,
             decreases self.data.len() - index,],
            [proof {
                assert forall|i: int| 0 <= i < index + 1 implies data_vm(self.data@[i]) == queue.vm by {
                    if i < index { assert(data_vm(self.data@[i]) == queue.vm); }
                }
            }],
            [proof { assert(resource_shape_matches(self, N, queue)); }],
            [invariant 0 <= index <= self.packets.len(), !self.generation.poisoned,
                0 < N <= 8192, resource_shape_matches(self, N, queue),
                template_order_error(self.packets@, 0) == template_order_error(self.packets@, index as int),
             decreases self.packets.len() - index,],
            [proof { reveal(template_order_error); }],
            [])
    }
}

fn immutable_preflight_frame_witness<C, K, D, H, P, T, E, const N: usize>(
    owner: &DispatchResourceOwnerV1<C, K, D, H, P, T, E>, queue: QueueKeyV1)
{
    let ghost before = *owner;
    let _result = owner.preflight_templates::<N>(queue);
    assert(*owner == before);
}
}
