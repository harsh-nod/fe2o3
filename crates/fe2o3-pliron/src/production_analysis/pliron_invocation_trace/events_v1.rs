use dialect_gpu::{AddressSpaceAttr, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr};
use dialect_kernel::{AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, MemorySpaceAttr};
use pliron::value::Value;

use super::{PlironTraceLocationV1, native_events_v1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlironTraceEventV1 {
    NativeSubject {
        location: PlironTraceLocationV1,
        occurrence: usize,
        kind: native_events_v1::NativeEventKindV1,
        address: Option<native_events_v1::NativeAddressV1>,
    },
    NativeBarrier {
        location: PlironTraceLocationV1,
        occurrence: usize,
        execution_scope: HierarchyAttr,
        address_spaces: u8,
    },
    NativeFence {
        location: PlironTraceLocationV1,
        occurrence: usize,
        address_spaces: u8,
    },
    Barrier {
        location: PlironTraceLocationV1,
        execution_scope: HierarchyAttr,
        memory_scope: MemoryScopeAttr,
        address_space: AddressSpaceAttr,
        order: MemoryOrderAttr,
    },
    Fence {
        location: PlironTraceLocationV1,
        memory_scope: MemoryScopeAttr,
        address_space: AddressSpaceAttr,
        order: MemoryOrderAttr,
    },
    TensorInstruction {
        location: PlironTraceLocationV1,
        subgroup_width: u16,
        claimed_active_lanes: u32,
    },
    Trap {
        location: PlironTraceLocationV1,
    },
    Memory {
        location: PlironTraceLocationV1,
        view: Value,
        memory_space: MemorySpaceAttr,
        access: AccessKindAttr,
        atomic_ordering: Option<AtomicOrderingAttr>,
        atomic_scope: Option<AtomicScopeAttr>,
        indices: Vec<Option<u64>>,
        allocation_origin: u64,
        noalias_class: u64,
        view_signature: (u32, Vec<u64>),
    },
    CollectiveAllocation {
        location: PlironTraceLocationV1,
        access: AccessKindAttr,
        memory_space: MemorySpaceAttr,
        allocation_origin: u64,
        noalias_class: u64,
    },
}
