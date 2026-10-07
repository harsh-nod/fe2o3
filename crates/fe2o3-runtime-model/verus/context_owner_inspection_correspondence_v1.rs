verus! {

proof fn inspection_journal_correspondence_v1(actual: ContextVersionJournalV1, model: logical::JournalContentsV1)
    requires represents(actual, model),
    ensures inspection_journal_v1(actual) == logical::inspection_journal_v1(model),
{}

proof fn inspection_stable_correspondence_v1(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1)
    requires stable_represents(actual, model),
    ensures inspection_stable_v1(actual) == logical::inspection_stable_v1(model),
{
    inspection_journal_correspondence_v1(actual.journal, model.journal);
}

proof fn inspection_producer_correspondence_v1(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1)
    requires producer_represents(actual, model),
    ensures inspection_producer_v1(actual) == logical::inspection_producer_v1(model),
{
    inspection_stable_correspondence_v1(actual.stable, model.stable);
}

// Mutable access makes complete owner identity an explicit, non-vacuous frame.
fn inspection_journal_paired_exec_v1(actual: &mut ContextVersionJournalV1, model: &mut logical::JournalContentsV1)
    -> (results: (JournalInspectionV1, logical::JournalInspectionV1))
    requires represents(*old(actual), *old(model)),
    ensures results.0 == results.1,
        results.0 == inspection_journal_v1(*old(actual)),
        results.1 == logical::inspection_journal_v1(*old(model)),
        *final(actual) == *old(actual), *final(model) == *old(model),
        represents(*final(actual), *final(model)),
{
    let result = inspection_journal_exec_v1(actual);
    let model_result = logical::inspection_journal_model_exec_v1(model);
    proof { inspection_journal_correspondence_v1(*actual, *model); }
    (result, model_result)
}

fn inspection_stable_explicit_paired_exec_v1(actual: &mut ContextReadLeasedJournalV1, model: &mut logical::ReadContentsV1)
    -> (results: (OwnerInspectionV1, logical::OwnerInspectionV1))
    requires stable_represents(*old(actual), *old(model)),
    ensures results.0 == results.1,
        results.0 == inspection_stable_v1(*old(actual)),
        results.1 == logical::inspection_stable_v1(*old(model)),
        *final(actual) == *old(actual), *final(model) == *old(model),
        stable_represents(*final(actual), *final(model)),
{
    let result = inspection_stable_explicit_exec_v1(actual);
    let model_result = logical::inspection_stable_model_exec_v1(model);
    proof { inspection_stable_correspondence_v1(*actual, *model); }
    (result, model_result)
}

fn inspection_stable_auto_paired_exec_v1(actual: &mut ContextReadLeasedJournalV1, model: &mut logical::ReadContentsV1)
    -> (results: (OwnerInspectionV1, logical::OwnerInspectionV1))
    requires stable_represents(*old(actual), *old(model)),
    ensures results.0 == results.1,
        results.0 == inspection_stable_v1(*old(actual)),
        results.1 == logical::inspection_stable_v1(*old(model)),
        *final(actual) == *old(actual), *final(model) == *old(model),
        stable_represents(*final(actual), *final(model)),
{
    let result = inspection_stable_auto_exec_v1(actual);
    let model_result = logical::inspection_stable_model_exec_v1(model);
    proof { inspection_stable_correspondence_v1(*actual, *model); }
    (result, model_result)
}

fn inspection_producer_explicit_paired_exec_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1)
    -> (results: (OwnerInspectionV1, logical::OwnerInspectionV1))
    requires producer_represents(*old(actual), *old(model)),
    ensures results.0 == results.1,
        results.0 == inspection_producer_v1(*old(actual)),
        results.1 == logical::inspection_producer_v1(*old(model)),
        *final(actual) == *old(actual), *final(model) == *old(model),
        producer_represents(*final(actual), *final(model)),
{
    let result = inspection_producer_explicit_exec_v1(actual);
    let model_result = logical::inspection_producer_model_exec_v1(model);
    proof { inspection_producer_correspondence_v1(*actual, *model); }
    (result, model_result)
}

fn inspection_producer_auto_paired_exec_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1)
    -> (results: (OwnerInspectionV1, logical::OwnerInspectionV1))
    requires producer_represents(*old(actual), *old(model)),
    ensures results.0 == results.1,
        results.0 == inspection_producer_v1(*old(actual)),
        results.1 == logical::inspection_producer_v1(*old(model)),
        *final(actual) == *old(actual), *final(model) == *old(model),
        producer_represents(*final(actual), *final(model)),
{
    let result = inspection_producer_auto_exec_v1(actual);
    let model_result = logical::inspection_producer_model_exec_v1(model);
    proof { inspection_producer_correspondence_v1(*actual, *model); }
    (result, model_result)
}

}
