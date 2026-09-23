verus! {

fn producer_query_consumer_same_exec_v1(left: WriterKeyV1, right: WriterKeyV1) -> (result: bool)
    ensures result == (left == right),
{ retained_writer_key_body!(left, right) }

fn producer_writer_same_exec_v1(left: WriterReferenceV1, right: WriterReferenceV1) -> (result: bool)
    ensures result == (left == right),
{ producer_writer_same_body!(left, right) }

fn producer_reference_same_exec_v1(left: ContextProducerReadReferenceV1, right: ContextProducerReadReferenceV1) -> (result: bool)
    ensures result == (left == right),
{ producer_reference_same_body!(left, right) }

impl ContextVersionJournalV1 {
    fn lookup_writer(&self, reference: WriterReferenceV1) -> (result: Result<ContextWriterStateV1, ReadErrorV1>)
        ensures result == writer_lookup_decision_v1(*self, reference),
    {
        writer_lookup_body!(self, reference, shared_retained_writer_key_v1, begin_indexed_access_v1)
    }
}

impl ContextProducerReadJournalV1 {
    fn status(&self, request: &ContextProducerReadV1) -> (result: Result<ContextProducerReadStatusV1, ReadErrorV1>)
        ensures result == producer_status_decision_v1(self.stable.journal, *request),
    {
        proof { reveal(inspection_stable_projection_v1); }
        producer_status_body!(&self.stable, request, producer_writer_same_exec_v1)
    }

    fn validate_producer_read(&self, request: &ContextProducerReadV1) -> (result: Result<(), ReadErrorV1>)
        ensures result == producer_validate_decision_v1(*self, *request),
    { producer_validate_body!(self, request) }

    fn inspect_producer_read(&self, reference: ContextProducerReadReferenceV1)
        -> (result: Result<(ContextProducerReadV1, ContextProducerReadStatusV1), ReadErrorV1>)
        ensures result == producer_inspect_decision_v1(*self, reference),
    { producer_inspect_body!(self, reference, producer_reference_same_exec_v1) }

    fn lookup_producer_read(&self, reference: ContextProducerReadReferenceV1)
        -> (result: Result<ContextProducerReadV1, ReadErrorV1>)
        ensures result == producer_lookup_decision_v1(*self, reference),
    { producer_lookup_body!(self, reference) }

    fn producer_read_status(&self, reference: ContextProducerReadReferenceV1)
        -> (result: Result<ContextProducerReadStatusV1, ReadErrorV1>)
        ensures result == producer_query_decision_v1(*self, reference),
    { producer_query_body!(self, reference) }
}

}
