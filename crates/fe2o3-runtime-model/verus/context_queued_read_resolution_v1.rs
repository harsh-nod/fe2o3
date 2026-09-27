//! Shared resolution over an explicit storage projection. Attached-list validity,
//! complete producer membership, settled lookup correspondence and outcome authentication are premises,
//! not a proof of outer queue admission, the complete journal, or native execution.
use vstd::prelude::*;

include!("../src/context_queued_writers/read_resolution_body.rs");

macro_rules! checked_resolution {
    ($owner:ident, $root:ident, $status:ident, $order:ident) => {
    queued_read_resolution_body!(verus_exec_expr, $owner, $root, $status,
        next, index, slot, entry, state, retained,
        [let ghost before = *$owner;
         proof { prefix_initial(before, $order, $status); reveal(ready); }],
        [
            invariant index <= $root.read_count, $order.len() == $root.read_count,
                before.queued_reads@.len() <= usize::MAX,
                ready(before, $root, $status, $order),
                prefix(before, *$owner, $order, index as int, $status),
                next == if index < $order.len() { Some($order[index as int]) } else { None },
            decreases $root.read_count - index,
        ],
        [
            proof {
                prefix_entry(before, *$owner, $root, $status, $order, index as int);
                reveal(prefix);
                assert(slot == $order[index as int]);
                assert($owner.queued_reads@[slot as int] == before.queued_reads@[slot as int]);
            }
            let ghost prior = *$owner;
        ],
        [
            proof {
                reveal(resolved);
                assert(*entry == resolved(before.queued_reads@[slot as int].unwrap(), before.inner, $status));
                assert($owner.queued_reads@ == prior.queued_reads@.update(slot as int,
                    Some(resolved(before.queued_reads@[slot as int].unwrap(), before.inner, $status))));
                prefix_advance(before, prior, *$owner, $status, $order, index as int);
            }
        ],
        [
            proof {
                prefix_complete(before, *$owner, $root, $status, $order);
            }
        ], [])
    };
}

verus! {

#[derive(Clone, Copy, PartialEq, Eq)]
enum WriterKind { Synchronous, Submission }

#[derive(Clone, Copy, PartialEq, Eq)]
struct WriterKey { context_generation: u64, local: u64, kind: WriterKind }

#[derive(Clone, Copy, PartialEq, Eq)]
struct WriterReference { slot: usize, key: WriterKey }

#[derive(Clone, Copy, PartialEq, Eq)]
struct AllocationKey { context_generation: u64, local: u64 }

#[derive(Clone, Copy, PartialEq, Eq)]
struct AllocationReference { slot: usize, key: AllocationKey }

impl vstd::std_specs::cmp::PartialEqSpecImpl for AllocationReference {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct AllocationWrite { allocation: AllocationReference, device: AllocationKey, byte_extent: u64 }

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextProducerReadStatusV1 { Pending, Success, NoEffect, Unknown }

impl vstd::std_specs::cmp::PartialEqSpecImpl for ContextProducerReadStatusV1 {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

#[derive(Clone, Copy)]
struct Request { allocation: AllocationWrite, byte_offset: u64, byte_len: u64, producer: WriterReference }

#[derive(Clone, Copy)]
struct ReadReference { slot: usize, incarnation: u64, consumer: WriterKey }

#[derive(Clone, Copy)]
struct Reservation {
    reference: ReadReference, request: Request, status: ContextProducerReadStatusV1,
    version: Option<(u64, u64)>, previous: Option<usize>, next: Option<usize>,
}

#[derive(Clone, Copy)]
enum Phase { Active, Queued, Unknown }

#[derive(Clone, Copy)]
struct Root {
    writer: WriterReference, head: Option<usize>, count: usize, phase: Phase,
    read_head: Option<usize>, read_count: usize,
}

#[derive(Clone, Copy)]
struct Allocation {
    reference: AllocationReference, attempt_epoch: u64, content_lineage: u64,
    pending_writer: Option<WriterReference>,
}

struct SettledAllocations { allocations: Vec<Option<Allocation>> }

impl SettledAllocations {
    fn lookup_allocation(&self, reference: AllocationReference) -> (result: Option<Allocation>)
        ensures result == lookup(*self, reference),
    {
        if reference.slot >= self.allocations.len() { return None; }
        match self.allocations[reference.slot] {
            Some(allocation) => if allocation.reference == reference { Some(allocation) } else { None },
            None => None,
        }
    }
}

spec fn lookup(inner: SettledAllocations, reference: AllocationReference) -> Option<Allocation> {
    if reference.slot < inner.allocations@.len()
        && inner.allocations@[reference.slot as int].is_some()
        && inner.allocations@[reference.slot as int].unwrap().reference == reference {
        inner.allocations@[reference.slot as int]
    } else { None }
}

struct QueuedReadProjection {
    inner: SettledAllocations, roots: Vec<Option<Root>>, queued_reads: Vec<Option<Reservation>>,
    free_reads: Vec<usize>, read_counts: Vec<usize>, next_read_incarnation: u64,
    disposal_terminal: bool,
}

spec fn selected(entry: Option<Reservation>, writer: WriterReference) -> bool {
    entry.is_some() && entry.unwrap().status == ContextProducerReadStatusV1::Pending
        && entry.unwrap().request.producer == writer
}

#[verifier::opaque]
spec fn ready(before: QueuedReadProjection, root: Root, status: ContextProducerReadStatusV1,
    order: Seq<usize>) -> bool {
    &&& status != ContextProducerReadStatusV1::Pending
    &&& before.queued_reads@.len() <= usize::MAX
    &&& root.writer.slot < before.roots@.len()
    &&& before.roots@[root.writer.slot as int].is_some()
    &&& before.roots@[root.writer.slot as int].unwrap().writer == root.writer
    &&& before.roots@[root.writer.slot as int].unwrap().read_head == root.read_head
    &&& before.roots@[root.writer.slot as int].unwrap().read_count == root.read_count
    &&& order.len() == root.read_count
    &&& order.no_duplicates()
    &&& root.read_head == if order.len() == 0 { None } else { Some(order[0]) }
    &&& forall|i: int| 0 <= i < order.len() ==> {
        let slot = (#[trigger] order[i]) as int;
        &&& slot < before.queued_reads@.len()
        &&& selected(before.queued_reads@[slot], root.writer)
        &&& before.queued_reads@[slot].unwrap().reference.slot == slot
        &&& before.queued_reads@[slot].unwrap().version.is_none()
        &&& before.queued_reads@[slot].unwrap().previous == if i == 0 { None } else { Some(order[i - 1]) }
        &&& before.queued_reads@[slot].unwrap().next == if i + 1 == order.len() { None } else { Some(order[i + 1]) }
        &&& if status == ContextProducerReadStatusV1::Success {
            let allocation = lookup(before.inner, before.queued_reads@[slot].unwrap().request.allocation.allocation);
            allocation.is_some() && allocation.unwrap().pending_writer.is_none()
                && allocation.unwrap().attempt_epoch == allocation.unwrap().content_lineage
        } else { true }
    }
    &&& forall|slot: int| 0 <= slot < before.queued_reads@.len()
        && selected(before.queued_reads@[slot], root.writer) ==> order.contains(slot as usize)
}

// The logical selection is by immutable producer identity, not the traversed list.
#[verifier::opaque]
spec fn resolved(entry: Reservation, inner: SettledAllocations, status: ContextProducerReadStatusV1) -> Reservation {
    Reservation {
        status,
        version: if status == ContextProducerReadStatusV1::Success {
            let allocation = lookup(inner, entry.request.allocation.allocation).unwrap();
            Some((allocation.attempt_epoch, allocation.content_lineage))
        } else { None },
        previous: None, next: None, ..entry
    }
}

#[verifier::opaque]
spec fn expected_reads(before: QueuedReadProjection, root: Root, status: ContextProducerReadStatusV1)
    -> Seq<Option<Reservation>> {
    Seq::new(before.queued_reads@.len(), |slot: int|
        if selected(before.queued_reads@[slot], root.writer) {
            Some(resolved(before.queued_reads@[slot].unwrap(), before.inner, status))
        } else { before.queued_reads@[slot] })
}

spec fn custody_frame(before: QueuedReadProjection, after: QueuedReadProjection) -> bool {
    &&& after.inner == before.inner
    &&& after.free_reads@ == before.free_reads@
    &&& after.read_counts@ == before.read_counts@
    &&& after.next_read_incarnation == before.next_read_incarnation
    &&& after.disposal_terminal == before.disposal_terminal
}

#[verifier::opaque]
spec fn prefix(before: QueuedReadProjection, after: QueuedReadProjection, order: Seq<usize>,
    count: int, status: ContextProducerReadStatusV1) -> bool {
    &&& custody_frame(before, after)
    &&& after.roots@ == before.roots@
    &&& after.queued_reads@.len() == before.queued_reads@.len()
    &&& forall|slot: int| 0 <= slot < before.queued_reads@.len() ==>
        after.queued_reads@[slot] == if order.take(count).contains(slot as usize) {
            Some(resolved(before.queued_reads@[slot].unwrap(), before.inner, status))
        } else { before.queued_reads@[slot] }
}

proof fn prefix_initial(before: QueuedReadProjection, order: Seq<usize>, status: ContextProducerReadStatusV1)
    ensures prefix(before, before, order, 0, status),
{
    reveal(prefix);
    assert(order.take(0) =~= Seq::<usize>::empty());
}

proof fn prefix_entry(before: QueuedReadProjection, current: QueuedReadProjection,
    root: Root, status: ContextProducerReadStatusV1, order: Seq<usize>, index: int)
    requires ready(before, root, status, order), prefix(before, current, order, index, status),
        0 <= index < root.read_count,
    ensures
        index < order.len(), order[index] < current.queued_reads@.len(),
        order[index] < before.queued_reads@.len(),
        before.queued_reads@[order[index] as int].is_some(),
        current.queued_reads@[order[index] as int] == before.queued_reads@[order[index] as int],
        current.queued_reads@[order[index] as int].is_some(),
        current.queued_reads@[order[index] as int].unwrap().version.is_none(),
        current.queued_reads@[order[index] as int].unwrap().next ==
            if index + 1 == order.len() { None } else { Some(order[index + 1]) },
        status == ContextProducerReadStatusV1::Success ==>
            lookup(current.inner, current.queued_reads@[order[index] as int].unwrap().request.allocation.allocation).is_some(),
        custody_frame(before, current), current.roots@ == before.roots@,
{
    reveal(ready);
    reveal(prefix);
    assert(!order.take(index).contains(order[index]));
}

proof fn prefix_advance(before: QueuedReadProjection, prior: QueuedReadProjection,
    after: QueuedReadProjection, status: ContextProducerReadStatusV1, order: Seq<usize>, index: int)
    requires prefix(before, prior, order, index, status),
        0 <= index < order.len(), order[index] < before.queued_reads@.len(),
        prior.queued_reads@.len() == before.queued_reads@.len(),
        before.queued_reads@.len() <= usize::MAX,
        before.queued_reads@[order[index] as int].is_some(),
        custody_frame(prior, after), after.roots@ == prior.roots@,
        after.queued_reads@ == prior.queued_reads@.update(order[index] as int,
            Some(resolved(before.queued_reads@[order[index] as int].unwrap(), before.inner, status))),
    ensures prefix(before, after, order, index + 1, status),
{
    reveal(prefix);
    assert(order.take(index + 1) =~= order.take(index).push(order[index]));
    assert forall|s: int| 0 <= s < before.queued_reads@.len() implies
        after.queued_reads@[s] == if order.take(index + 1).contains(s as usize) {
            Some(resolved(before.queued_reads@[s].unwrap(), before.inner, status))
        } else { before.queued_reads@[s] } by {
        vstd::seq_lib::lemma_seq_contains_after_push(order.take(index), order[index], s as usize);
        assert(prior.queued_reads@[s] == if order.take(index).contains(s as usize) {
            Some(resolved(before.queued_reads@[s].unwrap(), before.inner, status))
        } else { before.queued_reads@[s] });
        if s == order[index] {
            assert(order.take(index + 1).contains(s as usize));
        } else {
            assert(order.take(index + 1).contains(s as usize) == order.take(index).contains(s as usize));
        }
    }
}

proof fn selection_complete_at(before: QueuedReadProjection, root: Root,
    status: ContextProducerReadStatusV1, order: Seq<usize>, slot: int)
    requires ready(before, root, status, order), 0 <= slot < before.queued_reads@.len(),
    ensures order.contains(slot as usize) == selected(before.queued_reads@[slot], root.writer),
{
    reveal(ready);
    if order.contains(slot as usize) {
        let i = choose|i: int| 0 <= i < order.len() && order[i] == slot as usize;
        assert(order[i] as int == slot);
        assert(selected(before.queued_reads@[order[i] as int], root.writer));
    }
}

proof fn prefix_complete(before: QueuedReadProjection, after: QueuedReadProjection,
    root: Root, status: ContextProducerReadStatusV1, order: Seq<usize>)
    requires ready(before, root, status, order), prefix(before, after, order, root.read_count as int, status),
    ensures after.queued_reads@ == expected_reads(before, root, status), custody_frame(before, after),
        after.roots@ == before.roots@, root.writer.slot < after.roots@.len(),
        after.roots@[root.writer.slot as int].is_some(),
{
    reveal(ready);
    reveal(prefix);
    reveal(expected_reads);
    assert(order.take(root.read_count as int) =~= order);
    assert forall|s: int| 0 <= s < before.queued_reads@.len() implies
        order.contains(s as usize) == selected(before.queued_reads@[s], root.writer) by {
        selection_complete_at(before, root, status, order, s);
    }
    assert(after.queued_reads@ =~= expected_reads(before, root, status));
}

fn resolve_reads(owner: &mut QueuedReadProjection, root: Root, status: ContextProducerReadStatusV1,
    Ghost(order): Ghost<Seq<usize>>)
    requires ready(*old(owner), root, status, order),
    ensures custody_frame(*old(owner), *final(owner)),
        final(owner).queued_reads@ == expected_reads(*old(owner), root, status),
        final(owner).roots@ == old(owner).roots@.update(root.writer.slot as int,
            Some(Root { read_head: None, read_count: 0, ..old(owner).roots@[root.writer.slot as int].unwrap() })),
{
    checked_resolution!(owner, root, status, order)
}

fn empty_resolution_witness() {
    let writer = WriterReference { slot: 0, key: WriterKey { context_generation: 7, local: 4, kind: WriterKind::Submission } };
    let root = Root { writer, head: None, count: 0, phase: Phase::Queued, read_head: None, read_count: 0 };
    let mut owner = QueuedReadProjection {
        inner: SettledAllocations { allocations: Vec::new() }, roots: vec![Some(root)],
        queued_reads: Vec::new(), free_reads: Vec::new(), read_counts: Vec::new(),
        next_read_incarnation: 1, disposal_terminal: true,
    };
    let ghost before = owner;
    proof { reveal(ready); }
    resolve_reads(&mut owner, root, ContextProducerReadStatusV1::Unknown, Ghost(Seq::empty()));
    proof { reveal(expected_reads); }
    assert(owner.queued_reads@ == before.queued_reads@);
    assert(owner.roots@ == before.roots@);
    assert(owner.disposal_terminal);
}

fn chain_resolution_witness(status: ContextProducerReadStatusV1, singleton: bool)
    requires status != ContextProducerReadStatusV1::Pending,
{
    let writer = WriterReference { slot: 1, key: WriterKey { context_generation: 7, local: 4, kind: WriterKind::Submission } };
    let other = WriterReference { slot: 0, key: WriterKey { local: 5, ..writer.key } };
    let consumer = WriterKey { local: 20, ..writer.key };
    let allocation = AllocationReference { slot: 0, key: AllocationKey { context_generation: 7, local: 1 } };
    let second = AllocationReference { slot: 1, key: AllocationKey { local: 2, ..allocation.key } };
    let request = Request {
        allocation: AllocationWrite { allocation, device: allocation.key, byte_extent: 64 },
        byte_offset: 0, byte_len: 16, producer: writer,
    };
    let root = Root { writer, head: Some(6), count: 2, phase: Phase::Queued,
        read_head: Some(2), read_count: if singleton { 1 } else { 3 } };
    let live = Root { phase: Phase::Unknown, ..root };
    let unrelated_root = Root { writer: other, head: None, count: 0, phase: Phase::Active,
        read_head: Some(1), read_count: 1 };
    let head = Reservation { reference: ReadReference { slot: 2, incarnation: 10, consumer },
        request, status: ContextProducerReadStatusV1::Pending, version: None,
        previous: None, next: if singleton { None } else { Some(0) } };
    let middle = Reservation {
        reference: ReadReference { slot: 0, incarnation: 11, consumer: WriterKey { local: 21, ..consumer } },
        request: Request { allocation: AllocationWrite { allocation: second, ..request.allocation }, ..request },
        previous: Some(2), next: Some(3), ..head
    };
    let tail = Reservation { reference: ReadReference { slot: 3, incarnation: 12, consumer },
        previous: Some(0), next: None, ..head };
    let unrelated = Reservation { reference: ReadReference { slot: 1, incarnation: 8, consumer },
        request: Request { producer: other, ..request }, previous: None, next: None, ..head };
    let already_resolved = Reservation { reference: ReadReference { slot: 4, incarnation: 9, consumer },
        status: ContextProducerReadStatusV1::NoEffect, previous: None, next: None, ..head };
    let mut storage = QueuedReadProjection {
        inner: SettledAllocations { allocations: vec![
            Some(Allocation { reference: allocation, attempt_epoch: 5, content_lineage: 5, pending_writer: None }),
            Some(Allocation { reference: second, attempt_epoch: 11, content_lineage: 11, pending_writer: None }),
        ] },
        roots: vec![Some(unrelated_root), Some(live)],
        queued_reads: vec![if singleton { None } else { Some(middle) }, Some(unrelated), Some(head),
            if singleton { None } else { Some(tail) }, Some(already_resolved), None],
        free_reads: if singleton { vec![5, 3, 0] } else { vec![5] },
        read_counts: if singleton { vec![3, 0] } else { vec![4, 1] },
        next_read_incarnation: 13, disposal_terminal: true,
    };
    let ghost order = if singleton { seq![2usize] } else { seq![2usize, 0usize, 3usize] };
    let owner = &mut storage;
    let ghost before = *owner;
    proof { reveal(ready); assert(ready(*owner, root, status, order)); }
    checked_resolution!(owner, root, status, order);
    proof { reveal(expected_reads); reveal(resolved); }
    assert(owner.queued_reads@ == expected_reads(before, root, status));
    assert(custody_frame(before, *owner));
    assert(owner.queued_reads@[1] == Some(unrelated));
    assert(owner.queued_reads@[4] == Some(already_resolved));
    assert(owner.queued_reads@[2].unwrap().reference == head.reference);
    assert(owner.queued_reads@[2].unwrap().status == status);
    assert(owner.queued_reads@[2].unwrap().previous.is_none());
    assert(owner.queued_reads@[2].unwrap().next.is_none());
    assert(owner.queued_reads@[2].unwrap().version ==
        if status == ContextProducerReadStatusV1::Success { Some((5u64, 5u64)) } else { None });
    if !singleton {
        assert(owner.queued_reads@[0].unwrap().version ==
            if status == ContextProducerReadStatusV1::Success { Some((11u64, 11u64)) } else { None });
        assert(owner.queued_reads@[3].unwrap().status == status);
    }
    assert(owner.roots@[0] == Some(unrelated_root));
    assert(owner.roots@[1].unwrap().phase is Unknown);
    assert(owner.roots@[1].unwrap().read_head.is_none());
    assert(owner.roots@[1].unwrap().read_count == 0);
    assert(owner.free_reads@ == before.free_reads@);
    assert(owner.read_counts@ == before.read_counts@);
    assert(owner.next_read_incarnation == 13);
    assert(owner.disposal_terminal);
}

fn terminal_outcomes_witness() {
    chain_resolution_witness(ContextProducerReadStatusV1::Success, false);
    chain_resolution_witness(ContextProducerReadStatusV1::NoEffect, false);
    chain_resolution_witness(ContextProducerReadStatusV1::Unknown, false);
    chain_resolution_witness(ContextProducerReadStatusV1::Success, true);
    chain_resolution_witness(ContextProducerReadStatusV1::NoEffect, true);
    chain_resolution_witness(ContextProducerReadStatusV1::Unknown, true);
    empty_resolution_witness();
}

}
