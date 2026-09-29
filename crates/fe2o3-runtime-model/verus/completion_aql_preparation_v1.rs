include!("../../fe2o3-aql/src/preparation_body.rs");

verus! {
const AMD_SIGNAL_BYTES_V1: usize = 64;
const AMD_SIGNAL_ALIGNMENT_V1: usize = 64;
const AQL_INVALID_PACKET_HEADER_V1: u16 = 1;
const AQL_MAX_FIXED_BATCH_PACKETS_V2: u32 = 8192;
#[derive(Clone, Copy)]
struct ObservedGpuAddressV1(u64);
#[derive(Clone, Copy)]
struct AqlDispatchGeometryV1 { grid: [u32; 3], workgroup: [u16; 3], dimensions: u16 }
#[derive(Clone, Copy)]
enum AqlDispatchOrderingV1 { Independent, WaitForPrior }
struct AqlKernelDispatchPacketV1 {
    full_header: u32, workgroup_size_x: u16, workgroup_size_y: u16, workgroup_size_z: u16,
    reserved0: u16, grid_size_x: u32, grid_size_y: u32, grid_size_z: u32,
    private_segment_size: u32, group_segment_size: u32, kernel_object: u64,
    kernarg_address: u64, reserved2: u64, completion_signal: u64,
}
struct AqlPreparedKernelDispatchV1 { packet: AqlKernelDispatchPacketV1, ordering: AqlDispatchOrderingV1 }
struct AqlPreparedKernelDispatchBatchV2<const N: usize> { packets: Box<[AqlPreparedKernelDispatchV1; N]> }

spec fn alignment_error(raw: u64, alignment: u64) -> Option<AqlAddressObservationError> {
    if alignment == 0 || alignment > 4096 || alignment & ((alignment - 1) as u64) != 0 {
        Some(AqlAddressObservationError::InvalidRequiredAlignment)
    } else if raw & ((alignment - 1) as u64) != 0 { Some(AqlAddressObservationError::Misaligned) }
    else { None }
}
proof fn alignment_domain(alignment: u64)
    ensures (alignment != 0 && alignment <= 4096 && alignment & ((alignment - 1) as u64) == 0)
        == (alignment == 1 || alignment == 2 || alignment == 4 || alignment == 8
            || alignment == 16 || alignment == 32 || alignment == 64 || alignment == 128
            || alignment == 256 || alignment == 512 || alignment == 1024 || alignment == 2048 || alignment == 4096),
{
    assert((alignment != 0 && alignment <= 4096 && alignment & ((alignment - 1) as u64) == 0)
        == (alignment == 1 || alignment == 2 || alignment == 4 || alignment == 8
            || alignment == 16 || alignment == 32 || alignment == 64 || alignment == 128
            || alignment == 256 || alignment == 512 || alignment == 1024 || alignment == 2048 || alignment == 4096)) by (bit_vector);
}
impl ObservedGpuAddressV1 {
    fn new(raw: u64) -> (out: Result<Self, AqlAddressObservationError>)
        ensures out == if raw == 0 { Err(AqlAddressObservationError::Zero) } else { Ok(Self(raw)) },
    { aql_address_body!(verus_exec_expr, raw) }
    fn raw(self) -> (out: u64) ensures out == self.0,
    { aql_field_body!(verus_exec_expr, self, 0) }
    fn require_alignment(self, alignment: u64) -> (out: Result<Self, AqlAddressObservationError>)
        ensures out == match alignment_error(self.0, alignment) { Some(error) => Err(error), None => Ok(self) },
    { aql_alignment_body!(verus_exec_expr, self, alignment) }
}
impl AqlDispatchGeometryV1 {
    fn grid(self) -> (out: [u32; 3]) ensures out == self.grid,
    { aql_field_body!(verus_exec_expr, self, grid) }
    fn workgroup(self) -> (out: [u16; 3]) ensures out == self.workgroup,
    { aql_field_body!(verus_exec_expr, self, workgroup) }
    fn dimensions(self) -> (out: u16) ensures out == self.dimensions,
    { aql_field_body!(verus_exec_expr, self, dimensions) }
}
spec fn packet_error(kernel: u64, kernarg: u64, alignment: u64, signal: u64) -> Option<AqlDispatchPacketError> {
    if alignment_error(kernel, 64).is_some() { Some(AqlDispatchPacketError::KernelObject(alignment_error(kernel, 64).unwrap())) }
    else if alignment_error(kernarg, alignment).is_some() { Some(AqlDispatchPacketError::Kernarg(alignment_error(kernarg, alignment).unwrap())) }
    else if alignment_error(signal, 64).is_some() { Some(AqlDispatchPacketError::CompletionSignal(alignment_error(signal, 64).unwrap())) }
    else { None }
}
spec fn packet_value(geometry: AqlDispatchGeometryV1, private: u32, group: u32,
    kernel: u64, kernarg: u64, signal: u64, ordering: AqlDispatchOrderingV1) -> AqlPreparedKernelDispatchV1 {
    AqlPreparedKernelDispatchV1 { packet: AqlKernelDispatchPacketV1 {
        full_header: ((geometry.dimensions as u32) << 16) | 1u32,
        workgroup_size_x: geometry.workgroup[0], workgroup_size_y: geometry.workgroup[1],
        workgroup_size_z: geometry.workgroup[2], reserved0: 0,
        grid_size_x: geometry.grid[0], grid_size_y: geometry.grid[1], grid_size_z: geometry.grid[2],
        private_segment_size: private, group_segment_size: group, kernel_object: kernel,
        kernarg_address: kernarg, reserved2: 0, completion_signal: signal,
    }, ordering }
}
impl AqlKernelDispatchPacketV1 {
    fn new_unpublished_with_ordering(geometry: AqlDispatchGeometryV1, private: u32, group: u32,
        kernel: ObservedGpuAddressV1, kernarg: ObservedGpuAddressV1, alignment: u64,
        signal: ObservedGpuAddressV1, ordering: AqlDispatchOrderingV1)
        -> (out: Result<AqlPreparedKernelDispatchV1, AqlDispatchPacketError>)
        ensures out == match packet_error(kernel.0, kernarg.0, alignment, signal.0) {
            Some(error) => Err(error),
            None => Ok(packet_value(geometry, private, group, kernel.0, kernarg.0, signal.0, ordering)),
        },
    { aql_dispatch_preparation_body!(verus_exec_expr, geometry, private, group, kernel, kernarg, alignment, signal, ordering) }
}
impl<const N: usize> AqlPreparedKernelDispatchBatchV2<N> {
    fn try_from_boxed_packets(packets: Box<[AqlPreparedKernelDispatchV1; N]>)
        -> (out: Result<Self, AqlPreparedKernelDispatchBatchErrorV1>)
        ensures out == if N == 0 { Err(AqlPreparedKernelDispatchBatchErrorV1::ZeroPacketCount) }
            else if N > 8192 { Err(AqlPreparedKernelDispatchBatchErrorV1::PacketCountExceedsReviewedMaximum { requested: N, maximum: 8192 }) }
            else { Ok(Self { packets }) },
    { aql_boxed_batch_body!(verus_exec_expr, packets, N) }
}
}
