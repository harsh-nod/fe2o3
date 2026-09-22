verus! {

spec fn begin_historical_guards_pass_v1(model: logical::ProducerReadContentsV1,
    roster: Seq<logical::AllocationWriteV1>) -> bool
{
    &&& logical::producer_unread_scan_v1(model, logical::begin_read_references_v1(roster), 0) == Ok(())
    &&& logical::unread_scan_v1(model.stable, logical::begin_read_references_v1(roster), 0) == Ok(())
}

// Hybrid harness: only Begin's journal body is actual-type; the outer guards stay historical.
// None exposes that guard failure skipped actual Begin, rather than fabricating an actual result.
fn begin_guarded_issued_historical_exec_v1(actual: &mut JournalContentsV1, model: &mut logical::ProducerReadContentsV1,
    writer: WriterReferenceV1, model_writer: logical::WriterReferenceV1,
    roster: &[AllocationWriteV1], model_roster: &[logical::AllocationWriteV1],
    Ghost(storage): Ghost<logical::StorageCapacitiesV1>, Ghost(history): Ghost<Seq<logical::WriterReferenceV1>>)
    -> (results: (Option<Result<(), ReadErrorV1>>, Result<(), logical::ReadErrorV1>))
    requires represents(*old(actual), old(model).stable.journal), writer_reference_view(writer) == model_writer,
        begin_roster_view(roster@) == model_roster@, logical::issued_producer_v1(*old(model), storage, history),
    ensures represents(*final(actual), final(model).stable.journal),
        logical::begin_issued_relation_v1(*old(model), *final(model), model_writer, model_roster@, results.1, storage, history),
        results.0.is_some() == begin_historical_guards_pass_v1(*old(model), model_roster@),
        results.0.is_some() ==> results.0.unwrap() == begin_result_from(results.1)
            && begin_execution_relation_v1(*old(actual), *final(actual), writer, roster@, results.0.unwrap())
            && logical::begin_execution_relation_v1(old(model).stable.journal, final(model).stable.journal,
                model_writer, model_roster@, results.1),
        results.0.is_none() ==> *final(actual) == *old(actual) && results.1.is_err() && *final(model) == *old(model),
        results.1.is_err() ==> *final(actual) == *old(actual),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let combined: Result<(), logical::ReadErrorV1> = Ok(());
    match combined {
        Err(_) => {
            let model_result = logical::begin_issued_exec_v1(model, model_writer, model_roster, Ghost(storage), Ghost(history));
            return (None, model_result);
        },
        Ok(value) => { assert(value == ()); },
    }
    assert(combined == Ok(()));
    let stable = logical::begin_stable_unread_exec_v1(&model.stable, model_roster);
    match stable {
        Err(_) => {
            let model_result = logical::begin_issued_exec_v1(model, model_writer, model_roster, Ghost(storage), Ghost(history));
            return (None, model_result);
        },
        Ok(value) => { assert(value == ()); },
    }
    assert(stable == Ok(()));
    let result = begin_exec_v1(actual, writer, roster);
    let model_result = logical::begin_issued_exec_v1(model, model_writer, model_roster, Ghost(storage), Ghost(history));
    proof {
        assert(logical::begin_execution_relation_v1(model_before.stable.journal, model.stable.journal,
            model_writer, model_roster@, model_result));
        begin_paired_transition(before, *actual, model_before.stable.journal, model.stable.journal,
            writer, roster@, result, model_result);
    }
    (Some(result), model_result)
}

}
