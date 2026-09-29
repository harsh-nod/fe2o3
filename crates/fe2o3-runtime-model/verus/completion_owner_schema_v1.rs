// Shared host completion storage projection. Numeric identity newtypes retain
// structural equality; the ledger payload remains arbitrary and non-Copy.
macro_rules! structural_eq {
    ($($name:ident),+ $(,)?) => { $(verus! {
        impl vstd::std_specs::cmp::PartialEqSpecImpl for $name {
            open spec fn obeys_eq_spec() -> bool { true }
            open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
        }
    })+ };
}

verus! {
const COMPLETION_SIGNAL_CAPACITY_V1: usize = 8192;
#[derive(Clone, Copy, PartialEq, Eq)]
struct DeviceKeyV1 { physical: u64, generation: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct VmKeyV1 { device: DeviceKeyV1, id: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct QueueKeyV1 { vm: VmKeyV1, id: u64, generation: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct MemoryAllocationKeyV1 { vm: VmKeyV1, id: u64, generation: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct MemoryMappingKeyV1 { allocation: MemoryAllocationKeyV1, id: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct CompletionSlotLeaseV1 { index: u32, generation: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
enum CompletionOwnerPhaseV1 { Ready, ProbeActive, Poisoned }
#[derive(Clone, Copy, PartialEq, Eq)]
enum CompletionSlotPhaseV1 {
    Available, Bound { batch_id: u64 }, Published { batch_id: u64 }, Completed { batch_id: u64 },
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct CompletionSlotRecordV1 {
    generation: u64, phase: CompletionSlotPhaseV1, event_pins: u32, native_reader_pins: u32,
}
#[derive(Debug)]
enum AqlAddressObservationError { Zero, InvalidRequiredAlignment, Misaligned }
#[derive(Debug)]
enum AqlDispatchPacketError {
    KernelObject(AqlAddressObservationError), Kernarg(AqlAddressObservationError),
    CompletionSignal(AqlAddressObservationError),
}
#[derive(Debug)]
enum AqlPreparedKernelDispatchBatchErrorV1 {
    ZeroPacketCount, PacketCountExceedsReviewedMaximum { requested: usize, maximum: u32 },
}
#[derive(Debug)]
enum Gfx942CompletionErrorV1 {
    ZeroPacketCount, PacketCountExceedsMaximum { requested: usize, maximum: usize },
    StaleBatchGeneration, SignalPinned { slot: u32, event_pins: u32, native_reader_pins: u32 },
    Poisoned, StaleEventOccurrence, DuplicateDependency, DependencyLedgerAllocation, EventAlreadyBound,
    InvalidSessionOccurrence, InvalidAcceptanceEpoch, EventCapacityExhausted, EventIdentityExhausted,
    SignalPinCountExhausted,
    BatchIdentityExhausted, InsufficientSignals, WrongQueueGeneration, WrongVmGeneration,
    InvalidArena(&'static str), PacketBinding(AqlDispatchPacketError),
    BatchConstruction(AqlPreparedKernelDispatchBatchErrorV1),
}
struct CompletionSignalArenaOwnerV1<D> {
    queue: QueueKeyV1, signal_mapping: MemoryMappingKeyV1, gpu_base: u64, next_batch_id: u64,
    slots: Box<[CompletionSlotRecordV1; 8192]>, dependency_ledger: Box<D>, phase: CompletionOwnerPhaseV1,
}
struct OwnerState<D> {
    queue: QueueKeyV1, signal_mapping: MemoryMappingKeyV1, gpu_base: u64, next_batch_id: u64,
    slots: Seq<CompletionSlotRecordV1>, dependency_ledger: D, phase: CompletionOwnerPhaseV1,
}
}

structural_eq!(DeviceKeyV1, VmKeyV1, QueueKeyV1, MemoryAllocationKeyV1, MemoryMappingKeyV1,
    CompletionSlotLeaseV1, CompletionOwnerPhaseV1, CompletionSlotPhaseV1, CompletionSlotRecordV1);

verus! {
impl<D> CompletionSignalArenaOwnerV1<D> {
    spec fn owner_state(&self) -> OwnerState<D> {
        OwnerState { queue: self.queue, signal_mapping: self.signal_mapping, gpu_base: self.gpu_base,
            next_batch_id: self.next_batch_id, slots: self.slots@,
            dependency_ledger: *self.dependency_ledger, phase: self.phase }
    }

}
}
