verus! {

spec fn producer_status_from(status: logical::ProducerStatusV1) -> ContextProducerReadStatusV1 {
    match status {
        logical::ProducerStatusV1::Pending => ContextProducerReadStatusV1::Pending,
        logical::ProducerStatusV1::Success => ContextProducerReadStatusV1::Success,
        logical::ProducerStatusV1::NoEffect => ContextProducerReadStatusV1::NoEffect,
        logical::ProducerStatusV1::Unknown => ContextProducerReadStatusV1::Unknown,
    }
}

spec fn producer_status_result_from(result: Result<logical::ProducerStatusV1, logical::ReadErrorV1>)
    -> Result<ContextProducerReadStatusV1, ReadErrorV1>
{
    match result { Err(error) => Err(read_error_embed(error)), Ok(status) => Ok(producer_status_from(status)) }
}

spec fn producer_read_result_from(result: Result<logical::ProducerReadV1, logical::ReadErrorV1>)
    -> Result<ContextProducerReadV1, ReadErrorV1>
{
    match result { Err(error) => Err(read_error_embed(error)), Ok(request) => Ok(producer_read_from(request)) }
}

spec fn historical_producer_query_decision_v1(model: logical::ProducerReadContentsV1, reference: logical::ProducerReadReferenceV1)
    -> Result<logical::ProducerStatusV1, logical::ReadErrorV1>
{
    match logical::producer_lookup_decision_v1(model, reference) {
        Err(error) => Err(error), Ok(request) => logical::producer_status_decision_v1(model.stable.journal, request),
    }
}

proof fn producer_status_correspondence(actual: JournalContentsV1, model: logical::JournalContentsV1,
    request: ContextProducerReadV1)
    requires represents(actual, model),
    ensures producer_status_decision_v1(actual, request)
        == producer_status_result_from(logical::producer_status_decision_v1(model, producer_read_view(request))),
{
    allocation_lookup_correspondence(actual, model, request.read.allocation);
    writer_reference_round_trip(request.producer, writer_reference_view(request.producer));
    if request.producer.slot < actual.writers@.len() {
        if let Some(entry) = actual.writers@[request.producer.slot as int] {
            writer_entry_round_trip(entry, writer_entry_view(entry));
        }
    }
}

proof fn producer_lookup_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    reference: ContextProducerReadReferenceV1)
    requires producer_represents(actual, model),
    ensures producer_lookup_decision_v1(actual, reference)
        == producer_read_result_from(logical::producer_lookup_decision_v1(model, producer_reference_view(reference))),
        producer_query_decision_v1(actual, reference)
        == producer_status_result_from(historical_producer_query_decision_v1(model, producer_reference_view(reference))),
{
    producer_reference_round_trip(reference, producer_reference_view(reference));
    if reference.slot < actual.reservations@.len() {
        if let Some(entry) = actual.reservations@[reference.slot as int] {
            producer_status_correspondence(actual.stable.journal, model.stable.journal, entry.request);
            producer_read_round_trip(entry.request, producer_read_view(entry.request));
            producer_reference_round_trip(entry.reference, producer_reference_view(entry.reference));
        }
    }
}

// The historical public-status equivalent retains its original second traversal.
fn historical_producer_query_exec_v1(model: &logical::ProducerReadContentsV1, reference: logical::ProducerReadReferenceV1)
    -> (result: Result<logical::ProducerStatusV1, logical::ReadErrorV1>)
    ensures result == historical_producer_query_decision_v1(*model, reference),
{
    let request = match logical::producer_lookup_exec_v1(model, reference) {
        Err(error) => return Err(error), Ok(request) => request,
    };
    logical::producer_status_exec_v1(&model.stable.journal, request)
}

fn producer_status_historical_exec_v1(actual: &ContextProducerReadJournalV1, model: &logical::ProducerReadContentsV1,
    request: &ContextProducerReadV1, model_request: logical::ProducerReadV1)
    -> (results: (Result<ContextProducerReadStatusV1, ReadErrorV1>, Result<logical::ProducerStatusV1, logical::ReadErrorV1>))
    requires producer_represents(*actual, *model), producer_read_view(*request) == model_request,
    ensures results.0 == producer_status_result_from(results.1),
        results.0 == producer_status_decision_v1(actual.stable.journal, *request),
        results.1 == logical::producer_status_decision_v1(model.stable.journal, model_request),
{
    let result = actual.status(request);
    let model_result = logical::producer_status_exec_v1(&model.stable.journal, model_request);
    proof { producer_status_correspondence(actual.stable.journal, model.stable.journal, *request); }
    (result, model_result)
}

fn producer_validate_historical_exec_v1(actual: &ContextProducerReadJournalV1, model: &logical::ProducerReadContentsV1,
    request: &ContextProducerReadV1, model_request: logical::ProducerReadV1)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*actual, *model), producer_read_view(*request) == model_request,
    ensures results.0 == begin_result_from(results.1),
        results.0 == producer_validate_decision_v1(*actual, *request),
        results.1 == logical::producer_validate_decision_v1(model.stable.journal, model_request),
{
    let result = actual.validate_producer_read(request);
    let model_result = logical::producer_validate_exec_v1(&model.stable.journal, model_request);
    proof { producer_status_correspondence(actual.stable.journal, model.stable.journal, *request); }
    (result, model_result)
}

fn producer_lookup_historical_exec_v1(actual: &ContextProducerReadJournalV1, model: &logical::ProducerReadContentsV1,
    reference: ContextProducerReadReferenceV1, model_reference: logical::ProducerReadReferenceV1)
    -> (results: (Result<ContextProducerReadV1, ReadErrorV1>, Result<logical::ProducerReadV1, logical::ReadErrorV1>))
    requires producer_represents(*actual, *model), producer_reference_view(reference) == model_reference,
    ensures results.0 == producer_read_result_from(results.1),
        results.0 == producer_lookup_decision_v1(*actual, reference),
        results.1 == logical::producer_lookup_decision_v1(*model, model_reference),
{
    let result = actual.lookup_producer_read(reference);
    let model_result = logical::producer_lookup_exec_v1(model, model_reference);
    proof { producer_lookup_correspondence(*actual, *model, reference); }
    (result, model_result)
}

fn producer_query_historical_exec_v1(actual: &ContextProducerReadJournalV1, model: &logical::ProducerReadContentsV1,
    reference: ContextProducerReadReferenceV1, model_reference: logical::ProducerReadReferenceV1)
    -> (results: (Result<ContextProducerReadStatusV1, ReadErrorV1>, Result<logical::ProducerStatusV1, logical::ReadErrorV1>))
    requires producer_represents(*actual, *model), producer_reference_view(reference) == model_reference,
    ensures results.0 == producer_status_result_from(results.1),
        results.0 == producer_query_decision_v1(*actual, reference),
        results.1 == historical_producer_query_decision_v1(*model, model_reference),
{
    let result = actual.producer_read_status(reference);
    let model_result = historical_producer_query_exec_v1(model, model_reference);
    proof { producer_lookup_correspondence(*actual, *model, reference); }
    (result, model_result)
}

}
