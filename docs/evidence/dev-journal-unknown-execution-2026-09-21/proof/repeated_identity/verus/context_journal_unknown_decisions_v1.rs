// Total content transformation, with the historical opaque-value identity kept explicit.
verus! {

spec fn unknown_decision(before: LogicalJournalViewV1, writer: logical::WriterReferenceV1)
    -> Result<(), logical::ReadErrorV1>
{
    match header_decision(before, writer, true) {
        Err(error) => Err(error),
        Ok((head, count, _)) => chain_decision(before, writer, head, count),
    }
}

spec fn unknown_after(before: LogicalJournalViewV1, writer: logical::WriterReferenceV1) -> LogicalJournalViewV1 {
    match header_decision(before, writer, true) {
        Ok((head, count, false)) => if chain_decision(before, writer, head, count).is_ok() {
            JournalSequenceContentsV1 {
                writers: before.writers.update(writer.slot as int,
                    Some(logical::WriterEntryV1::Unknown { key: writer.key, head, count })),
                ..before
            }
        } else { before },
        _ => before,
    }
}

spec fn unknown_execution_view(before: LogicalJournalViewV1, after: LogicalJournalViewV1,
    writer: logical::WriterReferenceV1, result: Result<(), ContextVersionJournalErrorV1>) -> bool
{
    &&& result == read_result_from(unknown_decision(before, writer))
    &&& after == unknown_after(before, writer)
}

proof fn unknown_frame(before: LogicalJournalViewV1, writer: logical::WriterReferenceV1)
    ensures
        unknown_after(before, writer).context_generation == before.context_generation,
        unknown_after(before, writer).allocation_capacity == before.allocation_capacity,
        unknown_after(before, writer).writer_capacity == before.writer_capacity,
        unknown_after(before, writer).registration_watermark == before.registration_watermark,
        unknown_after(before, writer).reserved_count == before.reserved_count,
        unknown_after(before, writer).free == before.free,
        unknown_after(before, writer).allocations == before.allocations,
        unknown_after(before, writer).allocation_free == before.allocation_free,
        unknown_after(before, writer).members == before.members,
        unknown_after(before, writer).member_free == before.member_free,
        unknown_after(before, writer).scratch == before.scratch,
        unknown_after(before, writer).writers.len() == before.writers.len(),
        forall|i: int| 0 <= i < before.writers.len() && i != writer.slot ==>
            #[trigger] unknown_after(before, writer).writers[i] == before.writers[i],
        unknown_decision(before, writer).is_err() ==> unknown_after(before, writer) == before,
        match header_decision(before, writer, true) {
            Ok((_, _, true)) => unknown_after(before, writer) == before, _ => true,
        },
{}

proof fn unknown_decision_historical(before: logical::JournalContentsV1, writer: logical::WriterReferenceV1)
    ensures unknown_decision(logical_contents(before), writer) == logical::unknown_decision_v1(before, writer),
{
    header_historical(before, writer, true);
    if let Ok((head, count, _)) = logical::retained_header_decision_v1(before, writer, true) {
        chain_historical(before, writer, head, count);
    }
}

spec fn historical_unknown_identity(before: logical::JournalContentsV1, after: logical::JournalContentsV1,
    writer: logical::WriterReferenceV1, result: Result<(), logical::ReadErrorV1>) -> bool
{
    &&& result.is_err() ==> after == before
    &&& match logical::retained_header_decision_v1(before, writer, true) {
        Ok((_, _, true)) => true, _ => true,
    }
}

proof fn unit_result_embedding_injective(left: Result<(), logical::ReadErrorV1>, right: Result<(), logical::ReadErrorV1>)
    ensures (read_result_from(left) == read_result_from(right)) <==> (left == right),
{
    if let Err(error) = left { read_error_round_trip(read_error_embed(error), error); }
    if let Err(error) = right { read_error_round_trip(read_error_embed(error), error); }
}

// Sequence equality alone cannot discharge the old no-write branches' opaque Vec identity.
proof fn unknown_historical_factorization(before: logical::JournalContentsV1, after: logical::JournalContentsV1,
    writer: logical::WriterReferenceV1, result: Result<(), logical::ReadErrorV1>)
    ensures logical::unknown_execution_relation_v1(before, after, writer, result) <==>
        (unknown_execution_view(logical_contents(before), logical_contents(after), writer, read_result_from(result))
            && historical_unknown_identity(before, after, writer, result)),
{
    unknown_decision_historical(before, writer);
    header_historical(before, writer, true);
    unit_result_embedding_injective(result, logical::unknown_decision_v1(before, writer));
    if let Ok((head, count, unknown)) = logical::retained_header_decision_v1(before, writer, true) {
        chain_historical(before, writer, head, count);
        if unknown {
            assert(before.writers@.update(writer.slot as int,
                Some(logical::unknown_writer_v1(before.writers@[writer.slot as int].unwrap()))) =~= before.writers@);
        }
    }
}

proof fn unknown_historical_to_view(before: logical::JournalContentsV1, after: logical::JournalContentsV1,
    writer: logical::WriterReferenceV1, result: Result<(), logical::ReadErrorV1>)
    requires logical::unknown_execution_relation_v1(before, after, writer, result),
    ensures unknown_execution_view(logical_contents(before), logical_contents(after), writer, read_result_from(result)),
{
    unknown_historical_factorization(before, after, writer, result);
}

}
