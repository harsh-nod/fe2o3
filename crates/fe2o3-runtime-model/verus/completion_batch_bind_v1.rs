// Host preparation and commit only; std heap conversions are explicit contracts.
#![allow(unused_macros)]
#![feature(allocator_api)]
use vstd::prelude::*;
include!("completion_owner_schema_v1.rs");
include!("completion_bound_cancel_execution_v1.rs");
include!("completion_box_contracts_v1.rs");
include!("completion_aql_preparation_v1.rs");
include!("../../fe2o3-kfd/src/queue_completion/event_release_body.rs");
include!("../../fe2o3-kfd/src/queue_completion/batch_bind_body.rs");

verus! {
#[derive(Clone, Copy)]
struct CompletionPacketTemplateV1 {
    geometry: AqlDispatchGeometryV1, ordering: AqlDispatchOrderingV1,
    private_segment_size: u32, group_segment_size: u32,
    kernel_object: ObservedGpuAddressV1, kernarg_address: ObservedGpuAddressV1,
    kernarg_alignment: u64, generations: CompletionDispatchGenerationBindingV1,
}
struct CompletionPacketTemplatesV1<const N: usize> { values: Box<[CompletionPacketTemplateV1; N]> }
struct BoundCompletionBatchV1<const N: usize> {
    packets: AqlPreparedKernelDispatchBatchV2<N>, retention: CompletionBatchRetentionV1<N>,
}

spec fn pick(s: Seq<CompletionSlotRecordV1>, count: int, cursor: int) -> Seq<CompletionSlotLeaseV1>
    decreases s.len() - cursor,
{
    if count <= 0 || cursor < 0 || cursor >= s.len() { Seq::empty() }
    else if s[cursor].phase == CompletionSlotPhaseV1::Available {
        seq![CompletionSlotLeaseV1 { index: cursor as u32, generation: s[cursor].generation }]
            + pick(s, count - 1, cursor + 1)
    } else { pick(s, count, cursor + 1) }
}
spec fn selected(s: Seq<CompletionSlotRecordV1>, rows: Seq<CompletionSlotLeaseV1>) -> bool {
    &&& forall|i: int| 0 <= i < rows.len() ==> rows[i].index < s.len()
        && s[rows[i].index as int].phase == CompletionSlotPhaseV1::Available
        && s[rows[i].index as int].generation == rows[i].generation
    &&& forall|i: int, j: int| 0 <= i < j < rows.len() ==> rows[i].index < rows[j].index
}
spec fn committed<D>(s: OwnerState<D>, rows: Seq<CompletionSlotLeaseV1>, n: int) -> OwnerState<D> {
    OwnerState { slots: Seq::new(s.slots.len(), |i: int| {
        if present(rows, n, i as u32) {
            CompletionSlotRecordV1 { phase: CompletionSlotPhaseV1::Bound { batch_id: s.next_batch_id }, ..s.slots[i] }
        } else { s.slots[i] }
    }), ..s }
}
spec fn dispatch_error(queue: QueueKeyV1, binding: CompletionDispatchGenerationBindingV1) -> Option<Gfx942CompletionErrorV1> {
    if binding.queue != queue { Some(Gfx942CompletionErrorV1::WrongQueueGeneration) }
    else if binding.dispatch_generation == 0 || binding.code.allocation.vm != queue.vm
        || binding.kernarg.allocation.vm != queue.vm { Some(Gfx942CompletionErrorV1::WrongVmGeneration) }
    else { None }
}
spec fn preparation_error<D>(s: OwnerState<D>, t: CompletionPacketTemplateV1, slot: CompletionSlotLeaseV1)
    -> Option<Gfx942CompletionErrorV1> {
    let raw = s.gpu_base + slot.index as int * 64;
    if dispatch_error(s.queue, t.generations).is_some() { dispatch_error(s.queue, t.generations) }
    else if raw > u64::MAX { Some(Gfx942CompletionErrorV1::InvalidArena("completion slot address")) }
    else if raw == 0 { Some(Gfx942CompletionErrorV1::InvalidArena("completion address")) }
    else { match packet_error(t.kernel_object.0, t.kernarg_address.0, t.kernarg_alignment, raw as u64) {
        Some(error) => Some(Gfx942CompletionErrorV1::PacketBinding(error)), None => None,
    } }
}
spec fn prepared_packet<D>(s: OwnerState<D>, t: CompletionPacketTemplateV1, slot: CompletionSlotLeaseV1)
    -> AqlPreparedKernelDispatchV1 {
    packet_value(t.geometry, t.private_segment_size, t.group_segment_size,
        t.kernel_object.0, t.kernarg_address.0, (s.gpu_base + slot.index as int * 64) as u64, t.ordering)
}
spec fn prepare_scan_error<D>(s: OwnerState<D>, templates: Seq<CompletionPacketTemplateV1>,
    slots: Seq<CompletionSlotLeaseV1>, i: int) -> Option<Gfx942CompletionErrorV1>
    decreases templates.len() - i,
{
    if i < 0 || i >= templates.len() { None }
    else { match preparation_error(s, templates[i], slots[i]) {
        Some(error) => Some(error), None => prepare_scan_error(s, templates, slots, i + 1),
    } }
}
spec fn bind_error<D>(s: OwnerState<D>, templates: Seq<CompletionPacketTemplateV1>) -> Option<Gfx942CompletionErrorV1> {
    let n = templates.len();
    let slots = pick(s.slots, n as int, 0);
    if s.phase != CompletionOwnerPhaseV1::Ready { Some(Gfx942CompletionErrorV1::Poisoned) }
    else if n == 0 { Some(Gfx942CompletionErrorV1::ZeroPacketCount) }
    else if n > 8192 { Some(Gfx942CompletionErrorV1::PacketCountExceedsMaximum { requested: n as usize, maximum: 8192 }) }
    else if s.next_batch_id == u64::MAX { Some(Gfx942CompletionErrorV1::BatchIdentityExhausted) }
    else if slots.len() != n { Some(Gfx942CompletionErrorV1::InsufficientSignals) }
    else { prepare_scan_error(s, templates, slots, 0) }
}

impl<D> CompletionSignalArenaOwnerV1<D> {
    fn bind_fixed_batch<const N: usize>(&mut self, templates: CompletionPacketTemplatesV1<N>)
        -> (out: Result<BoundCompletionBatchV1<N>, Gfx942CompletionErrorV1>)
        ensures out.is_ok() == bind_error(old(self).owner_state(), templates.values@).is_none(),
            match out {
                Err(error) => final(self).owner_state() == old(self).owner_state()
                    && Some(error) == bind_error(old(self).owner_state(), templates.values@),
                Ok(batch) => {
                    let initial = old(self).owner_state();
                    &&& batch.retention.batch_id == initial.next_batch_id
                    &&& batch.retention.queue == initial.queue
                    &&& batch.retention.signal_mapping == initial.signal_mapping
                    &&& batch.retention.last_packet_id.is_none()
                    &&& batch.retention.slots@ == pick(initial.slots, N as int, 0)
                    &&& forall|i: int| 0 <= i < N ==> batch.retention.dispatches@[i] == templates.values@[i].generations
                    &&& forall|i: int| 0 <= i < N ==> batch.packets.packets@[i]
                        == prepared_packet(initial, templates.values@[i], batch.retention.slots@[i])
                    &&& final(self).owner_state() == (OwnerState { next_batch_id: (initial.next_batch_id + 1) as u64,
                        ..committed(initial, batch.retention.slots@, N as int) })
                    &&& bound(final(self).owner_state(), batch.retention)
                },
            },
    {
        completion_bind_batch_body!(@annotated verus_exec_expr, self, templates, N,
            next_id, slots, prepared, index, dispatches, packets, batch,
            [let ghost initial = self.owner_state();],
            [invariant 0 <= index <= N, 0 < N <= 8192, prepared.len() == index,
                self.owner_state() == old(self).owner_state(), initial == old(self).owner_state(),
                self.phase == CompletionOwnerPhaseV1::Ready,
                self.next_batch_id < u64::MAX, next_id == self.next_batch_id + 1,
                slots.len() == N, selected(self.slots@, slots@),
                slots@ == pick(self.slots@, N as int, 0),
                bind_error(initial, templates.values@) == prepare_scan_error(initial, templates.values@, slots@, index as int),
                forall|i: int| 0 <= i < index ==> preparation_error(initial, templates.values@[i], slots@[i]).is_none()
                    && prepared@[i] == prepared_packet(initial, templates.values@[i], slots@[i]),
             decreases N - index,],
            [proof {
                reveal(prepare_scan_error);
                reveal(preparation_error);
                assert(slots@[index as int].index < 8192);
                assert(preparation_error(initial, templates.values@[index as int], slots@[index as int]).is_some()
                    ==> bind_error(initial, templates.values@)
                        == preparation_error(initial, templates.values@[index as int], slots@[index as int]));
            }],
            [proof {
                assert(preparation_error(initial, templates.values@[index as int], slots@[index as int]).is_none());
                assert(prepared@[index as int] == prepared_packet(initial, templates.values@[index as int], slots@[index as int]));
            }],
            [invariant 0 <= index <= N, dispatches.len() == index,
                0 < N <= 8192, self.owner_state() == old(self).owner_state(),
                initial == old(self).owner_state(), self.phase == CompletionOwnerPhaseV1::Ready,
                self.next_batch_id < u64::MAX, next_id == self.next_batch_id + 1,
                selected(self.slots@, slots@), slots@ == pick(self.slots@, N as int, 0),
                bind_error(initial, templates.values@).is_none(),
                forall|i: int| 0 <= i < N ==> preparation_error(initial, templates.values@[i], slots@[i]).is_none()
                    && packets.packets@[i] == prepared_packet(initial, templates.values@[i], slots@[i]),
                forall|i: int| 0 <= i < index ==> dispatches@[i] == templates.values@[i].generations,
             decreases N - index,],
            [],
            [proof {
                let state = committed(initial, batch.retention.slots@, N as int);
                assert forall|i: int| 0 <= i < N implies entry(state, batch.retention,
                    CompletionSlotPhaseV1::Bound { batch_id: initial.next_batch_id }, i) by {
                    assert(batch.packets.packets@[i] == prepared_packet(initial, templates.values@[i], batch.retention.slots@[i]));
                    assert(preparation_error(initial, templates.values@[i], batch.retention.slots@[i]).is_none());
                    assert(present(batch.retention.slots@, N as int, batch.retention.slots@[i].index));
                }
                assert(bound(state, batch.retention));
            }])
    }

    fn require_ready(&self) -> (out: Result<(), Gfx942CompletionErrorV1>)
        ensures out == if self.phase == CompletionOwnerPhaseV1::Ready { Ok(()) } else { Err(Gfx942CompletionErrorV1::Poisoned) },
    { completion_require_ready_body!(verus_exec_expr, self) }

    fn validate_dispatch_binding(&self, binding: CompletionDispatchGenerationBindingV1)
        -> (out: Result<(), Gfx942CompletionErrorV1>)
        ensures out == match dispatch_error(self.queue, binding) { Some(error) => Err(error), None => Ok(()) },
    { completion_dispatch_binding_body!(verus_exec_expr, self, binding) }

    fn select_available_slots(&self, count: usize) -> (out: Vec<CompletionSlotLeaseV1>)
        ensures out@ == pick(self.slots@, count as int, 0), out.len() <= count,
            selected(self.slots@, out@),
    {
        completion_select_slots_body!(@annotated verus_exec_expr, self, count, slots, index,
            [proof { assert(slots@ + pick(self.slots@, count as int, 0) =~= pick(self.slots@, count as int, 0)); }],
            [invariant 0 <= index <= 8192, slots.len() <= count,
                selected(self.slots@, slots@),
                forall|j: int| 0 <= j < slots.len() ==> slots@[j].index < index,
                slots@ + pick(self.slots@, count - slots.len(), index as int) == pick(self.slots@, count as int, 0),
             decreases 8192 - index,],
            [let ghost before = slots@;],
            [proof {
                assert(selected(self.slots@, slots@));
                assert forall|j: int| 0 <= j < slots.len() implies slots@[j].index < index + 1 by {
                    if j < before.len() { assert(slots@[j] == before[j]); }
                }
                assert(slots@ + pick(self.slots@, count - slots.len(), index as int + 1)
                    =~= before + pick(self.slots@, count - before.len(), index as int));
            }])
    }

    fn commit_bound_batch<const N: usize>(&mut self, bound: BoundCompletionBatchV1<N>, next_batch_id: u64)
        -> (out: BoundCompletionBatchV1<N>)
        requires selected(old(self).slots@, bound.retention.slots@),
        ensures out == bound, final(self).owner_state() == (OwnerState {
            next_batch_id, ..committed(old(self).owner_state(), bound.retention.slots@, N as int) }),
    {
        completion_commit_batch_body!(@annotated verus_exec_expr, self, bound, next_batch_id, N, index,
            [let ghost initial = self.owner_state();
             proof { assert(initial.slots =~= committed(initial, bound.retention.slots@, 0).slots); }],
            [invariant 0 <= index <= N, initial == old(self).owner_state(),
                selected(initial.slots, bound.retention.slots@),
                self.owner_state() == committed(initial, bound.retention.slots@, index as int),
             decreases N - index,],
            [proof {
                assert(self.owner_state().slots =~= committed(initial, bound.retention.slots@, index as int + 1).slots);
                assert(self.owner_state() == committed(initial, bound.retention.slots@, index as int + 1));
            }])
    }
}
}
