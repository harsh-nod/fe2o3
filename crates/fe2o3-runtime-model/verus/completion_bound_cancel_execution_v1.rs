include!("../../fe2o3-kfd/src/queue_completion/bound_cancel_body.rs");
include!("../../fe2o3-kfd/src/queue_completion/rollback_adapters_body.rs");

verus! {
#[derive(Clone, Copy, PartialEq, Eq)]
struct CompletionDispatchGenerationBindingV1 {
    queue: QueueKeyV1, code: MemoryMappingKeyV1, kernarg: MemoryMappingKeyV1,
    dispatch_generation: u64,
}
struct CompletionBatchRetentionV1<const N: usize> {
    batch_id: u64, queue: QueueKeyV1, signal_mapping: MemoryMappingKeyV1,
    slots: Box<[CompletionSlotLeaseV1; N]>,
    dispatches: Box<[CompletionDispatchGenerationBindingV1; N]>, last_packet_id: Option<u64>,
}
}

structural_eq!(CompletionDispatchGenerationBindingV1);

verus! {
spec fn present(slots: Seq<CompletionSlotLeaseV1>, n: int, key: u32) -> bool {
    exists|i: int| 0 <= i < n && slots[i].index == key
}
spec fn marked(seen: Seq<u64>, key: u32) -> bool {
    seen[key as int / 64] & (1u64 << (key % 64)) != 0
}
spec fn entry<D, const N: usize>(s: OwnerState<D>, r: CompletionBatchRetentionV1<N>,
    expected: CompletionSlotPhaseV1, i: int) -> bool {
        let slot = r.slots@[i];
        &&& slot.index < 8192
        &&& s.slots[slot.index as int].generation == slot.generation
        &&& s.slots[slot.index as int].phase == expected
        &&& r.dispatches@[i].queue == r.queue
        &&& r.dispatches@[i].dispatch_generation != 0
        &&& r.dispatches@[i].code.allocation.vm == r.queue.vm
        &&& r.dispatches@[i].kernarg.allocation.vm == r.queue.vm
        &&& forall|j: int| 0 <= j < i ==> r.slots@[j].index != slot.index
}
spec fn entries<D, const N: usize>(s: OwnerState<D>, r: CompletionBatchRetentionV1<N>,
    expected: CompletionSlotPhaseV1, n: int) -> bool {
    forall|i: int| #![trigger r.slots@[i]] 0 <= i < n ==> entry(s, r, expected, i)
}
spec fn valid<D, const N: usize>(s: OwnerState<D>, r: CompletionBatchRetentionV1<N>,
    expected: CompletionSlotPhaseV1) -> bool {
    0 < N <= 8192 && r.queue == s.queue && r.signal_mapping == s.signal_mapping
        && entries(s, r, expected, N as int)
}
spec fn bound<D, const N: usize>(s: OwnerState<D>, r: CompletionBatchRetentionV1<N>) -> bool {
    r.last_packet_id.is_none() && valid(s, r, CompletionSlotPhaseV1::Bound { batch_id: r.batch_id })
}
spec fn retention_error(n: usize) -> Gfx942CompletionErrorV1 {
    if n == 0 { Gfx942CompletionErrorV1::ZeroPacketCount }
    else if n > 8192 { Gfx942CompletionErrorV1::PacketCountExceedsMaximum { requested: n, maximum: 8192 } }
    else { Gfx942CompletionErrorV1::StaleBatchGeneration }
}
spec fn bound_error<const N: usize>(r: CompletionBatchRetentionV1<N>) -> Gfx942CompletionErrorV1 {
    if r.last_packet_id.is_some() { Gfx942CompletionErrorV1::StaleBatchGeneration }
    else { retention_error(N) }
}
spec fn unpinned<D>(s: OwnerState<D>, slots: Seq<CompletionSlotLeaseV1>, n: int) -> bool {
    forall|i: int| 0 <= i < n ==> s.slots[slots[i].index as int].event_pins == 0
        && s.slots[slots[i].index as int].native_reader_pins == 0
}
spec fn pin_error<D>(s: OwnerState<D>, slots: Seq<CompletionSlotLeaseV1>, e: Gfx942CompletionErrorV1) -> bool {
    exists|i: int| 0 <= i < slots.len() && unpinned(s, slots, i)
        && (s.slots[slots[i].index as int].event_pins != 0 || s.slots[slots[i].index as int].native_reader_pins != 0)
        && e == Gfx942CompletionErrorV1::SignalPinned {
            slot: slots[i].index, event_pins: s.slots[slots[i].index as int].event_pins,
            native_reader_pins: s.slots[slots[i].index as int].native_reader_pins,
        }
}
spec fn cancelled<D>(s: OwnerState<D>, slots: Seq<CompletionSlotLeaseV1>, n: int) -> OwnerState<D> {
    OwnerState { slots: Seq::new(s.slots.len(), |i: int| {
        if present(slots, n, i as u32) {
            CompletionSlotRecordV1 { phase: CompletionSlotPhaseV1::Available, ..s.slots[i] }
        } else { s.slots[i] }
    }), ..s }
}

proof fn bit_membership(word: u64, a: u32, b: u32)
    requires a < 64, b < 64,
    ensures ((word | (1u64 << a)) & (1u64 << b) != 0)
        == ((word & (1u64 << b) != 0) || a == b),
{
    assert(((word | (1u64 << a)) & (1u64 << b) != 0)
        == ((word & (1u64 << b) != 0) || a == b)) by (bit_vector)
        requires a < 64, b < 64;
}

proof fn bitmap_step(seen: Seq<u64>, key: u32)
    requires seen.len() == 128, key < 8192,
    ensures forall|other: u32| other < 8192 ==>
        marked(seen.update(key as int / 64, seen[key as int / 64] | (1u64 << (key % 64))), other)
        == (marked(seen, other) || other == key),
{
    let next = seen.update(key as int / 64, seen[key as int / 64] | (1u64 << (key % 64)));
    assert forall|other: u32| other < 8192 implies marked(next, other)
        == (marked(seen, other) || other == key) by {
        if other / 64 == key / 64 {
            bit_membership(seen[key as int / 64], key % 64, other % 64);
            assert((other % 64 == key % 64) == (other == key));
        }
    }
}

fn validate_packet_count<const N: usize>() -> (out: Result<(), Gfx942CompletionErrorV1>)
    ensures out == if 0 < N <= 8192 { Ok(()) } else { Err(retention_error(N)) },
{ completion_packet_count_body!(verus_exec_expr, N) }

impl<D> CompletionSignalArenaOwnerV1<D> {
    fn validate_retention<const N: usize>(&self, retention: &CompletionBatchRetentionV1<N>,
        expected: CompletionSlotPhaseV1) -> (out: Result<(), Gfx942CompletionErrorV1>)
        ensures out == if valid(self.owner_state(), *retention, expected) { Ok(()) } else { Err(retention_error(N)) },
    {
        completion_validate_retention_body!(@annotated verus_exec_expr, self, retention,
            expected, N, seen, i, slot, word, bit,
            [proof {
                assert forall|key: u32| key < 8192 implies !marked(seen@, key) by {
                    let bit = key % 64;
                    assert(0u64 & (1u64 << bit) == 0) by (bit_vector);
                }
            }],
            [invariant 0 <= i <= N, 0 < N <= 8192,
                retention.queue == self.queue, retention.signal_mapping == self.signal_mapping,
                entries(self.owner_state(), *retention, expected, i as int),
                forall|key: u32| key < 8192 ==> #[trigger] marked(seen@, key) == present(retention.slots@, i as int, key),
             decreases N - i,],
            [let ghost before_seen = seen@;
             proof {
                assert(valid(self.owner_state(), *retention, expected)
                    ==> entry(self.owner_state(), *retention, expected, i as int));
                assert(retention.slots@[i as int].index < 8192 ==>
                    marked(seen@, retention.slots@[i as int].index)
                        == present(retention.slots@, i as int, retention.slots@[i as int].index));
                assert(entry(self.owner_state(), *retention, expected, i as int)
                    ==> !present(retention.slots@, i as int, retention.slots@[i as int].index));
             }],
            [proof {
                assert(slot.index < 8192);
                assert(marked(seen@, slot.index) == present(retention.slots@, i as int, slot.index));
                assert(!marked(seen@, slot.index));
                assert(!present(retention.slots@, i as int, slot.index));
                assert(entries(self.owner_state(), *retention, expected, i as int + 1));
            }],
            [proof {
                assert(seen@ =~= before_seen.update(word as int, before_seen[word as int] | bit));
                bitmap_step(before_seen, slot.index);
                assert forall|key: u32| key < 8192 implies marked(seen@, key)
                    == present(retention.slots@, i as int + 1, key) by {
                    assert(marked(seen@, key) == (marked(before_seen, key) || key == slot.index));
                    assert(marked(before_seen, key) == present(retention.slots@, i as int, key));
                    assert(present(retention.slots@, i as int + 1, key)
                        == (present(retention.slots@, i as int, key) || slot.index == key));
                }
            }])
    }

    fn validate_bound<const N: usize>(&self, retention: &CompletionBatchRetentionV1<N>)
        -> (out: Result<(), Gfx942CompletionErrorV1>)
        ensures out == if bound(self.owner_state(), *retention) { Ok(()) } else { Err(bound_error(*retention)) },
    { completion_validate_bound_body!(verus_exec_expr, self, retention) }

    fn require_unpinned<const N: usize>(&self, slots: &[CompletionSlotLeaseV1; N])
        -> (out: Result<(), Gfx942CompletionErrorV1>)
        requires forall|i: int| 0 <= i < N ==> slots@[i].index < 8192,
        ensures out.is_ok() == unpinned(self.owner_state(), slots@, N as int),
            match out { Err(error) => pin_error(self.owner_state(), slots@, error), _ => true },
    {
        completion_require_unpinned_body!(@annotated verus_exec_expr, self, slots, N, i,
            [invariant 0 <= i <= N,
                forall|j: int| 0 <= j < N ==> slots@[j].index < 8192,
                unpinned(self.owner_state(), slots@, i as int),
             decreases N - i,])
    }

    fn cancel_bound_retaining<const N: usize>(&mut self, retention: CompletionBatchRetentionV1<N>)
        -> (out: Result<(), (Gfx942CompletionErrorV1, CompletionBatchRetentionV1<N>)>)
        ensures out.is_ok() == (bound(old(self).owner_state(), retention)
                && unpinned(old(self).owner_state(), retention.slots@, N as int)),
            match out {
                Ok(()) => final(self).owner_state() == cancelled(old(self).owner_state(), retention.slots@, N as int),
                Err((error, returned)) => final(self).owner_state() == old(self).owner_state() && returned == retention
                    && if !bound(old(self).owner_state(), retention) { error == bound_error(retention) }
                        else { pin_error(old(self).owner_state(), retention.slots@, error) },
            },
    {
        completion_cancel_bound_retaining_body!(@annotated verus_exec_expr, self, retention, N, i,
            [let ghost initial = self.owner_state();
             proof { assert(initial.slots =~= cancelled(initial, retention.slots@, 0).slots); }],
            [invariant 0 <= i <= N, bound(old(self).owner_state(), retention),
                unpinned(old(self).owner_state(), retention.slots@, N as int),
                initial == old(self).owner_state(),
                self.owner_state() == cancelled(initial, retention.slots@, i as int),
             decreases N - i,],
            [proof {
                assert(self.owner_state().slots =~= cancelled(initial, retention.slots@, i as int + 1).slots);
                assert(self.owner_state() == cancelled(initial, retention.slots@, i as int + 1));
            }])
    }
}

fn cancellation_witness<D>(ledger: D, phase: CompletionOwnerPhaseV1, duplicate: bool,
    pins: u32, readers: u32) {
    let vm = VmKeyV1 { device: DeviceKeyV1 { physical: 0, generation: 0 }, id: 0 };
    let queue = QueueKeyV1 { vm, id: 0, generation: 0 };
    let mapping = MemoryMappingKeyV1 {
        allocation: MemoryAllocationKeyV1 { vm, id: 0, generation: 0 }, id: 0,
    };
    let mut owner = CompletionSignalArenaOwnerV1 {
        queue, signal_mapping: mapping, gpu_base: 0, next_batch_id: u64::MAX,
        slots: Box::new([CompletionSlotRecordV1 {
            generation: u64::MAX, phase: CompletionSlotPhaseV1::Available,
            event_pins: u32::MAX, native_reader_pins: u32::MAX,
        }; 8192]), dependency_ledger: Box::new(ledger), phase,
    };
    let record = CompletionSlotRecordV1 {
        generation: 0, phase: CompletionSlotPhaseV1::Bound { batch_id: 0 },
        event_pins: 0, native_reader_pins: 0,
    };
    owner.slots[63] = record;
    owner.slots[64] = CompletionSlotRecordV1 { event_pins: pins, native_reader_pins: readers, ..record };
    let dispatch = CompletionDispatchGenerationBindingV1 {
        queue, code: mapping, kernarg: mapping, dispatch_generation: 1,
    };
    let retention = CompletionBatchRetentionV1 {
        batch_id: 0, queue, signal_mapping: mapping,
        slots: Box::new([CompletionSlotLeaseV1 { index: 63, generation: 0 },
            CompletionSlotLeaseV1 { index: if duplicate { 63 } else { 64 }, generation: 0 }]),
        dispatches: Box::new([dispatch; 2]), last_packet_id: None,
    };
    let ghost before = owner.owner_state();
    let result = owner.cancel_bound_retaining(retention);
    if duplicate {
        assert(result.is_err());
        assert(owner.owner_state() == before);
        assert(match result { Err((error, _)) => error == Gfx942CompletionErrorV1::StaleBatchGeneration, _ => false });
    } else if pins != 0 || readers != 0 {
        assert(result.is_err());
        assert(owner.owner_state() == before);
        assert(match result { Err((error, _)) => error == Gfx942CompletionErrorV1::SignalPinned {
            slot: 64, event_pins: pins, native_reader_pins: readers,
        }, _ => false });
    } else {
        assert(result.is_ok());
        assert(owner.slots[63].phase == CompletionSlotPhaseV1::Available);
        assert(owner.slots[64].phase == CompletionSlotPhaseV1::Available);
        assert(owner.slots[63].generation == 0);
        assert(owner.owner_state().slots[0] == before.slots[0]);
        assert(owner.owner_state().dependency_ledger == before.dependency_ledger);
        assert(owner.phase == phase && owner.next_batch_id == u64::MAX);
    }
}

fn packet_count_witness() {
    let zero = validate_packet_count::<0>();
    let largest = validate_packet_count::<8192>();
    let overflow = validate_packet_count::<8193>();
    assert(zero == Err(Gfx942CompletionErrorV1::ZeroPacketCount));
    assert(largest.is_ok());
    assert(overflow == Err(Gfx942CompletionErrorV1::PacketCountExceedsMaximum {
        requested: 8193, maximum: 8192,
    }));
}
}

verus! {
impl<D> CompletionSignalArenaOwnerV1<D> {
    fn cancel_bound<const N: usize>(&mut self, retention: CompletionBatchRetentionV1<N>)
        -> (out: Result<(), Gfx942CompletionErrorV1>)
        ensures out.is_ok() == (bound(old(self).owner_state(), retention)
                && unpinned(old(self).owner_state(), retention.slots@, N as int)),
            match out {
                Ok(()) => final(self).owner_state() == cancelled(old(self).owner_state(), retention.slots@, N as int),
                Err(error) => final(self).owner_state() == old(self).owner_state()
                    && if !bound(old(self).owner_state(), retention) { error == bound_error(retention) }
                        else { pin_error(old(self).owner_state(), retention.slots@, error) },
            },
    { completion_cancel_bound_body!(@annotated verus_exec_expr, self, retention, failure,
        [-> (error: Gfx942CompletionErrorV1) ensures error == failure.0]) }
}

}
