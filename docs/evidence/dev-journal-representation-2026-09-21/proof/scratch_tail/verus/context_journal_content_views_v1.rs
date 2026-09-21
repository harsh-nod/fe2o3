// Ordered sequence contents only: no capacity, address, allocator or unwind model.
verus! {

struct JournalSequenceContentsV1<W, A, M, P> {
    context_generation: u64,
    allocation_capacity: usize,
    writer_capacity: usize,
    registration_watermark: u64,
    reserved_count: usize,
    writers: Seq<Option<W>>,
    free: Seq<usize>,
    allocations: Seq<Option<A>>,
    allocation_free: Seq<usize>,
    members: Seq<Option<M>>,
    member_free: Seq<usize>,
    scratch: Seq<Option<P>>,
}

type ConcreteJournalViewV1 = JournalSequenceContentsV1<WriterEntryV1, AllocationEntryV1, MemberEntryV1, BeginMemberPlanV1>;
type LogicalJournalViewV1 = JournalSequenceContentsV1<logical::WriterEntryV1, logical::AllocationEntryV1,
    logical::MemberEntryV1, logical::BeginMemberPlanV1>;

// The cfg(test) access counter is intentionally outside the content view.
spec fn concrete_contents(value: ContextVersionJournalV1) -> ConcreteJournalViewV1 {
    JournalSequenceContentsV1 {
        context_generation: value.context_generation,
        allocation_capacity: value.allocation_capacity,
        writer_capacity: value.writer_capacity,
        registration_watermark: value.registration_watermark,
        reserved_count: value.reserved_count,
        writers: value.writers@,
        free: value.free@,
        allocations: value.allocations@,
        allocation_free: value.allocation_free@,
        members: value.members@,
        member_free: value.member_free@,
        scratch: value.scratch@.map(|_i, _entry| None),
    }
}

spec fn logical_contents(value: logical::JournalContentsV1) -> LogicalJournalViewV1 {
    JournalSequenceContentsV1 {
        context_generation: value.context_generation,
        allocation_capacity: value.allocation_capacity,
        writer_capacity: value.writer_capacity,
        registration_watermark: value.registration_watermark,
        reserved_count: value.reserved_count,
        writers: value.writers@,
        free: value.free@,
        allocations: value.allocations@,
        allocation_free: value.allocation_free@,
        members: value.members@,
        member_free: value.member_free@,
        scratch: value.scratch@,
    }
}

spec fn project_contents(value: ConcreteJournalViewV1) -> LogicalJournalViewV1 {
    JournalSequenceContentsV1 {
        context_generation: value.context_generation,
        allocation_capacity: value.allocation_capacity,
        writer_capacity: value.writer_capacity,
        registration_watermark: value.registration_watermark,
        reserved_count: value.reserved_count,
        writers: value.writers.map(|_i, entry| writer_entry_slot_view(entry)),
        free: value.free,
        allocations: value.allocations.map(|_i, entry| allocation_entry_slot_view(entry)),
        allocation_free: value.allocation_free,
        members: value.members.map(|_i, entry| member_entry_slot_view(entry)),
        member_free: value.member_free,
        scratch: value.scratch.map(|_i, entry| begin_plan_slot_view(entry)),
    }
}

spec fn recover_contents(value: LogicalJournalViewV1) -> ConcreteJournalViewV1 {
    JournalSequenceContentsV1 {
        context_generation: value.context_generation,
        allocation_capacity: value.allocation_capacity,
        writer_capacity: value.writer_capacity,
        registration_watermark: value.registration_watermark,
        reserved_count: value.reserved_count,
        writers: value.writers.map(|_i, entry| writer_entry_slot_from(entry)),
        free: value.free,
        allocations: value.allocations.map(|_i, entry| allocation_entry_slot_from(entry)),
        allocation_free: value.allocation_free,
        members: value.members.map(|_i, entry| member_entry_slot_from(entry)),
        member_free: value.member_free,
        scratch: value.scratch.map(|_i, entry| begin_plan_slot_from(entry)),
    }
}

spec fn journal_view(value: ContextVersionJournalV1) -> LogicalJournalViewV1 {
    project_contents(concrete_contents(value))
}

spec fn represents(value: ContextVersionJournalV1, model: logical::JournalContentsV1) -> bool {
    journal_view(value) == logical_contents(model)
}

// This inverse constructs ghost sequences, never an allocated historical Vec.
proof fn sequence_round_trip(value: ConcreteJournalViewV1, model: LogicalJournalViewV1)
    ensures recover_contents(project_contents(value)) == value,
        project_contents(recover_contents(model)) == model,
{
    assert(recover_contents(project_contents(value)).writers =~= value.writers) by {
        assert forall|i: int| 0 <= i < value.writers.len() implies
            recover_contents(project_contents(value)).writers[i] == value.writers[i] by {
            writer_entry_slot_round_trip(value.writers[i], writer_entry_slot_view(value.writers[i]));
        }
    }
    assert(project_contents(recover_contents(model)).writers =~= model.writers) by {
        assert forall|i: int| 0 <= i < model.writers.len() implies
            project_contents(recover_contents(model)).writers[i] == model.writers[i] by {
            writer_entry_slot_round_trip(writer_entry_slot_from(model.writers[i]), model.writers[i]);
        }
    }
    assert(recover_contents(project_contents(value)).allocations =~= value.allocations) by {
        assert forall|i: int| 0 <= i < value.allocations.len() implies
            recover_contents(project_contents(value)).allocations[i] == value.allocations[i] by {
            allocation_entry_slot_round_trip(value.allocations[i], allocation_entry_slot_view(value.allocations[i]));
        }
    }
    assert(project_contents(recover_contents(model)).allocations =~= model.allocations) by {
        assert forall|i: int| 0 <= i < model.allocations.len() implies
            project_contents(recover_contents(model)).allocations[i] == model.allocations[i] by {
            allocation_entry_slot_round_trip(allocation_entry_slot_from(model.allocations[i]), model.allocations[i]);
        }
    }
    assert(recover_contents(project_contents(value)).members =~= value.members) by {
        assert forall|i: int| 0 <= i < value.members.len() implies
            recover_contents(project_contents(value)).members[i] == value.members[i] by {
            member_entry_slot_round_trip(value.members[i], member_entry_slot_view(value.members[i]));
        }
    }
    assert(project_contents(recover_contents(model)).members =~= model.members) by {
        assert forall|i: int| 0 <= i < model.members.len() implies
            project_contents(recover_contents(model)).members[i] == model.members[i] by {
            member_entry_slot_round_trip(member_entry_slot_from(model.members[i]), model.members[i]);
        }
    }
    assert(recover_contents(project_contents(value)).scratch =~= value.scratch) by {
        assert forall|i: int| 0 <= i < value.scratch.len() implies
            recover_contents(project_contents(value)).scratch[i] == value.scratch[i] by {
            begin_plan_slot_round_trip(value.scratch[i], begin_plan_slot_view(value.scratch[i]));
        }
    }
    assert(project_contents(recover_contents(model)).scratch =~= model.scratch) by {
        assert forall|i: int| 0 <= i < model.scratch.len() implies
            project_contents(recover_contents(model)).scratch[i] == model.scratch[i] by {
            begin_plan_slot_round_trip(begin_plan_slot_from(model.scratch[i]), model.scratch[i]);
        }
    }
}

proof fn journal_view_is_lossless(left: ContextVersionJournalV1, right: ContextVersionJournalV1)
    ensures (journal_view(left) == journal_view(right))
        <==> (concrete_contents(left) == concrete_contents(right)),
{
    sequence_round_trip(concrete_contents(left), journal_view(left));
    sequence_round_trip(concrete_contents(right), journal_view(right));
}

proof fn journal_projection_exact(value: ContextVersionJournalV1)
    ensures
        journal_view(value).context_generation == value.context_generation,
        journal_view(value).allocation_capacity == value.allocation_capacity,
        journal_view(value).writer_capacity == value.writer_capacity,
        journal_view(value).registration_watermark == value.registration_watermark,
        journal_view(value).reserved_count == value.reserved_count,
        journal_view(value).writers.len() == value.writers@.len(),
        forall|i: int| 0 <= i < value.writers@.len() ==> #[trigger] journal_view(value).writers[i]
            == writer_entry_slot_view(value.writers@[i]),
        journal_view(value).free == value.free@,
        journal_view(value).allocations.len() == value.allocations@.len(),
        forall|i: int| 0 <= i < value.allocations@.len() ==> #[trigger] journal_view(value).allocations[i]
            == allocation_entry_slot_view(value.allocations@[i]),
        journal_view(value).allocation_free == value.allocation_free@,
        journal_view(value).members.len() == value.members@.len(),
        forall|i: int| 0 <= i < value.members@.len() ==> #[trigger] journal_view(value).members[i]
            == member_entry_slot_view(value.members@[i]),
        journal_view(value).member_free == value.member_free@,
        journal_view(value).scratch.len() == value.scratch@.len(),
        forall|i: int| 0 <= i < value.scratch@.len() ==> #[trigger] journal_view(value).scratch[i]
            == begin_plan_slot_view(value.scratch@[i]),
{}

proof fn historical_projection_exact(value: logical::JournalContentsV1)
    ensures
        logical_contents(value).context_generation == value.context_generation,
        logical_contents(value).allocation_capacity == value.allocation_capacity,
        logical_contents(value).writer_capacity == value.writer_capacity,
        logical_contents(value).registration_watermark == value.registration_watermark,
        logical_contents(value).reserved_count == value.reserved_count,
        logical_contents(value).writers == value.writers@,
        logical_contents(value).free == value.free@,
        logical_contents(value).allocations == value.allocations@,
        logical_contents(value).allocation_free == value.allocation_free@,
        logical_contents(value).members == value.members@,
        logical_contents(value).member_free == value.member_free@,
        logical_contents(value).scratch == value.scratch@,
{}

}
