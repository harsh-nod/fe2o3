// Allocation-free event storage and authentication shared by binding and release.
verus! {
#[derive(Clone, Copy, PartialEq, Eq)]
struct ExactCompletionOccurrenceV1 {
    session_occurrence: u64, source_acceptance_epoch: u64, batch_id: u64,
    queue: QueueKeyV1, signal_mapping: MemoryMappingKeyV1,
    slot: CompletionSlotLeaseV1, dispatch_generation: u64, packet_id: Option<u64>,
}
struct Gfx942ComputeEventOccurrenceV1 { event_id: u64, exact: ExactCompletionOccurrenceV1 }
struct CompletionDependencyLedgerV1<R> {
    next_event_id: u64, next_reader_lease_id: u64,
    events: HashMap<u64, ExactCompletionOccurrenceV1>, readers: R,
}
struct State<R> {
    queue: QueueKeyV1, signal_mapping: MemoryMappingKeyV1,
    gpu_base: u64, next_batch_id: u64,
    slots: Seq<CompletionSlotRecordV1>,
    events: Map<u64, ExactCompletionOccurrenceV1>, readers: R,
    next_event_id: u64, next_reader_lease_id: u64, phase: CompletionOwnerPhaseV1,
}
}

structural_eq!(ExactCompletionOccurrenceV1);

verus! {
broadcast use vstd::std_specs::hash::group_hash_axioms;

spec fn active<R>(s: State<R>, event: Gfx942ComputeEventOccurrenceV1) -> bool {
    s.events.contains_key(event.event_id) && s.events[event.event_id] == event.exact
}

impl<R> CompletionSignalArenaOwnerV1<CompletionDependencyLedgerV1<R>> {
    spec fn state(&self) -> State<R> {
        State { queue: self.queue, signal_mapping: self.signal_mapping,
            gpu_base: self.gpu_base, next_batch_id: self.next_batch_id,
            slots: (*self.slots)@, events: self.dependency_ledger.events@,
            readers: self.dependency_ledger.readers,
            next_event_id: self.dependency_ledger.next_event_id,
            next_reader_lease_id: self.dependency_ledger.next_reader_lease_id,
            phase: self.phase }
    }
    fn require_ready(&self) -> (out: Result<(), Gfx942CompletionErrorV1>)
        ensures out == if self.phase == CompletionOwnerPhaseV1::Ready { Ok(()) }
            else { Err(Gfx942CompletionErrorV1::Poisoned) },
    { completion_require_ready_body!(verus_exec_expr, self) }

    fn validate_active_event(&self, event: &Gfx942ComputeEventOccurrenceV1)
        -> (out: Result<(), Gfx942CompletionErrorV1>)
        ensures out == if active(self.state(), *event) { Ok(()) }
            else { Err(Gfx942CompletionErrorV1::StaleEventOccurrence) },
    { completion_validate_active_event_body!(verus_exec_expr, self, event) }
}
}
