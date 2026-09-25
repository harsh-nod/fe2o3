// Executable selected-entry projections, not a proof of std::HashMap or Context
// prevalidation/quarantine. The unchanged entries are outside this projection.
verus! {

struct SelectedEntryV1<T> {
    id: u64,
    value: Option<T>,
}

impl<T> SelectedEntryV1<T> {
    fn get(&self, id: &u64) -> (value: Option<&T>)
        requires *id == self.id,
        ensures match value {
            Some(value) => self.value == Some(*value),
            None => self.value.is_none(),
        },
    { self.value.as_ref() }

    fn get_mut(&mut self, id: &u64) -> (value: Option<&mut T>)
        requires *id == old(self).id,
        ensures final(self).id == old(self).id,
            match value {
                Some(value) => old(self).value == Some(*value)
                    && final(self).value == Some(*final(value)),
                None => old(self).value.is_none() && final(self).value.is_none(),
            },
    { self.value.as_mut() }

    fn remove(&mut self, id: &u64) -> (value: Option<T>)
        requires *id == old(self).id,
        ensures value == old(self).value, final(self).value.is_none(),
            final(self).id == old(self).id,
    { self.value.take() }
}

#[derive(Copy)]
struct SelectedReaderMarkerV1<R: Copy> {
    first: R,
    count: usize,
}

impl<R: Copy> Clone for SelectedReaderMarkerV1<R> {
    fn clone(&self) -> (result: Self)
        ensures result == *self,
    { *self }
}

struct SelectedReaderRootV1<R: Copy> {
    marker: Option<SelectedReaderMarkerV1<R>>,
    references: Vec<R>,
}

#[derive(Clone, Copy)]
struct SelectedReaderRecordV1 {
    journal_read: Option<SelectedReaderMarkerV1<ContextReadLeaseReferenceV1>>,
    journal_producer_read: Option<SelectedReaderMarkerV1<ContextProducerReadReferenceV1>>,
    journal_writer: Option<WriterReferenceV1>,
}

fn selected_consumer_view_v1(value: WriterKeyV1) -> (model: logical::WriterKeyV1)
    ensures model == writer_key_view(value),
{
    logical::WriterKeyV1 { context_generation: value.context_generation, local: value.local,
        kind: match value.kind {
            WriterKindV1::Synchronous => logical::WriterKindV1::Synchronous,
            WriterKindV1::Submission => logical::WriterKindV1::Submission,
        } }
}

fn selected_stable_views_v1(references: &[ContextReadLeaseReferenceV1])
    -> (model: Vec<logical::ReadReferenceV1>)
    ensures model@ == stable_references_view(references@),
{
    let mut model = Vec::new();
    let mut i = 0usize;
    while i < references.len()
        invariant i <= references.len(), model@.len() == i,
            forall|j: int| 0 <= j < i ==> model@[j] == read_reference_view(references@[j]),
        decreases references.len() - i,
    {
        let reference = references[i];
        let consumer = selected_consumer_view_v1(reference.consumer);
        model.push(logical::ReadReferenceV1 { slot: reference.slot,
            incarnation: reference.incarnation, consumer });
        i += 1;
    }
    assert(model@ =~= stable_references_view(references@));
    model
}

fn selected_producer_views_v1(references: &[ContextProducerReadReferenceV1])
    -> (model: Vec<logical::ProducerReadReferenceV1>)
    ensures model@ == producer_references_view(references@),
{
    let mut model = Vec::new();
    let mut i = 0usize;
    while i < references.len()
        invariant i <= references.len(), model@.len() == i,
            forall|j: int| 0 <= j < i ==> model@[j] == producer_reference_view(references@[j]),
        decreases references.len() - i,
    {
        let reference = references[i];
        let consumer = selected_consumer_view_v1(reference.consumer);
        model.push(logical::ProducerReadReferenceV1 { slot: reference.slot,
            incarnation: reference.incarnation, consumer });
        i += 1;
    }
    assert(model@ =~= producer_references_view(references@));
    model
}

struct SelectedReaderJournalV1 {
    actual: ContextProducerReadJournalV1,
    model: logical::ProducerReadContentsV1,
    last_model_result: Result<(), logical::ReadErrorV1>,
}

spec fn selected_journal_wf_v1(journal: SelectedReaderJournalV1) -> bool {
    producer_represents(journal.actual, journal.model) && logical::producer_invariant_v1(journal.model)
}

impl SelectedReaderJournalV1 {
    fn release_reads(&mut self, consumer: WriterKeyV1,
        references: &[ContextReadLeaseReferenceV1], evidence: &ContextReadQuiescenceEvidenceV1)
        -> (result: Result<(), ReadErrorV1>)
        requires selected_journal_wf_v1(*old(self)),
        ensures selected_journal_wf_v1(*final(self)),
            result == begin_result_from(final(self).last_model_result),
            logical::producer_outer_frame_v1(old(self).model, final(self).model),
            producer_stable_release_relation_v1(old(self).actual, final(self).actual, consumer, references@,
                evidence.consumer, old(self).actual.stable.leases@.len() as usize, result),
            logical::release_execution_relation_v1(old(self).model.stable, final(self).model.stable,
                writer_key_view(consumer), stable_references_view(references@), writer_key_view(evidence.consumer),
                old(self).actual.stable.leases@.len() as usize, final(self).last_model_result),
    {
        let model_consumer = selected_consumer_view_v1(consumer);
        let model_evidence = selected_consumer_view_v1(evidence.consumer);
        let model_references = selected_stable_views_v1(references);
        // Constructor storage reserves at least the arena length. Physical Vec
        // capacity correspondence remains an external storage boundary.
        let storage = self.actual.stable.leases.len();
        let (actual, model) = producer_stable_release_historical_exec_v1(
            &mut self.actual, &mut self.model, consumer, model_consumer,
            references, &model_references, evidence, model_evidence, storage);
        self.last_model_result = model;
        actual
    }

    fn release_producer_reads(&mut self, consumer: WriterKeyV1,
        references: &[ContextProducerReadReferenceV1], evidence: &ContextReadQuiescenceEvidenceV1)
        -> (result: Result<(), ReadErrorV1>)
        requires selected_journal_wf_v1(*old(self)),
        ensures selected_journal_wf_v1(*final(self)),
            result == begin_result_from(final(self).last_model_result),
            producer_release_execution_relation_v1(old(self).actual, final(self).actual, consumer, references@,
                evidence.consumer, old(self).actual.reservations@.len() as usize, result),
            logical::producer_release_execution_relation_v1(old(self).model, final(self).model,
                writer_key_view(consumer), producer_references_view(references@), writer_key_view(evidence.consumer),
                old(self).actual.reservations@.len() as usize, final(self).last_model_result),
    {
        let model_consumer = selected_consumer_view_v1(consumer);
        let model_evidence = selected_consumer_view_v1(evidence.consumer);
        let model_references = selected_producer_views_v1(references);
        let storage = self.actual.reservations.len();
        let (actual, model) = producer_release_historical_exec_v1(
            &mut self.actual, &mut self.model, consumer, model_consumer,
            references, &model_references, evidence, model_evidence, storage);
        self.last_model_result = model;
        actual
    }
}

struct SelectedReadersV1 {
    stable: SelectedEntryV1<SelectedReaderRootV1<ContextReadLeaseReferenceV1>>,
    producer: SelectedEntryV1<SelectedReaderRootV1<ContextProducerReadReferenceV1>>,
    records: SelectedEntryV1<SelectedReaderRecordV1>,
    journal: SelectedReaderJournalV1,
}

spec fn selected_readers_wf_v1(state: SelectedReadersV1) -> bool {
    &&& selected_journal_wf_v1(state.journal)
    &&& state.stable.id == state.producer.id == state.records.id
    &&& match state.stable.value {
        None => true,
        Some(root) => selected_root_wf_v1(root) && root.marker.unwrap().first.consumer.local == state.stable.id,
    }
    &&& match state.producer.value {
        None => true,
        Some(root) => selected_root_wf_v1(root) && root.marker.unwrap().first.consumer.local == state.producer.id,
    }
    &&& match state.records.value {
        None => true,
        Some(record) => record.journal_read == match state.stable.value { None => None, Some(root) => root.marker }
            && record.journal_producer_read == match state.producer.value { None => None, Some(root) => root.marker },
    }
}

spec fn selected_root_wf_v1<R: Copy>(root: SelectedReaderRootV1<R>) -> bool {
    &&& root.marker.is_some()
    &&& root.marker.unwrap().count > 0
    &&& root.marker.unwrap().count == root.references@.len()
    &&& root.marker.unwrap().first == root.references@[0]
}

spec fn selected_stable_record_v1(before: Option<SelectedReaderRecordV1>, after: Option<SelectedReaderRecordV1>) -> bool {
    match before {
        None => after.is_none(),
        Some(record) => after == Some(SelectedReaderRecordV1 { journal_read: None, ..record }),
    }
}

spec fn selected_producer_record_v1(before: Option<SelectedReaderRecordV1>, after: Option<SelectedReaderRecordV1>) -> bool {
    match before {
        None => after.is_none(),
        Some(record) => after == Some(SelectedReaderRecordV1 { journal_producer_read: None, ..record }),
    }
}

spec fn selected_stable_step_v1(before: SelectedReadersV1, after: SelectedReadersV1,
    result: Result<(), ReadErrorV1>) -> bool {
    &&& producer_represents(after.journal.actual, after.journal.model)
    &&& after.producer == before.producer
    &&& after.stable.id == before.stable.id && after.records.id == before.records.id
    &&& result == begin_result_from(after.journal.last_model_result)
    &&& match before.stable.value {
        None => result == Ok(()) && after.stable == before.stable && after.records == before.records
            && after.journal.actual == before.journal.actual && after.journal.model == before.journal.model,
        Some(root) => {
            &&& logical::producer_outer_frame_v1(before.journal.model, after.journal.model)
            &&& producer_stable_release_relation_v1(before.journal.actual, after.journal.actual,
                root.marker.unwrap().first.consumer, root.references@, root.marker.unwrap().first.consumer,
                before.journal.actual.stable.leases@.len() as usize, result)
            &&& logical::release_execution_relation_v1(before.journal.model.stable, after.journal.model.stable,
                writer_key_view(root.marker.unwrap().first.consumer), stable_references_view(root.references@),
                writer_key_view(root.marker.unwrap().first.consumer),
                before.journal.actual.stable.leases@.len() as usize, after.journal.last_model_result)
            &&& if result.is_ok() {
                after.stable.value.is_none() && selected_stable_record_v1(before.records.value, after.records.value)
            } else { after.stable == before.stable && after.records == before.records }
        },
    }
}

spec fn selected_producer_step_v1(before: SelectedReadersV1, after: SelectedReadersV1,
    result: Result<(), ReadErrorV1>) -> bool {
    &&& producer_represents(after.journal.actual, after.journal.model)
    &&& after.stable == before.stable
    &&& after.producer.id == before.producer.id && after.records.id == before.records.id
    &&& result == begin_result_from(after.journal.last_model_result)
    &&& match before.producer.value {
        None => result == Ok(()) && after.producer == before.producer && after.records == before.records
            && after.journal.actual == before.journal.actual && after.journal.model == before.journal.model,
        Some(root) => {
            &&& producer_release_execution_relation_v1(before.journal.actual, after.journal.actual,
                root.marker.unwrap().first.consumer, root.references@, root.marker.unwrap().first.consumer,
                before.journal.actual.reservations@.len() as usize, result)
            &&& logical::producer_release_execution_relation_v1(before.journal.model, after.journal.model,
                writer_key_view(root.marker.unwrap().first.consumer), producer_references_view(root.references@),
                writer_key_view(root.marker.unwrap().first.consumer),
                before.journal.actual.reservations@.len() as usize, after.journal.last_model_result)
            &&& if result.is_ok() {
                after.producer.value.is_none() && selected_producer_record_v1(before.records.value, after.records.value)
            } else { after.producer == before.producer && after.records == before.records }
        },
    }
}

spec fn selected_inputs_step_v1(before: SelectedReadersV1, after: SelectedReadersV1,
    result: Result<(), ReadErrorV1>) -> bool {
    ||| result.is_err() && selected_stable_step_v1(before, after, result)
    ||| exists|middle: SelectedReadersV1| selected_stable_step_v1(before, middle, Ok(()))
        && selected_producer_step_v1(middle, after, result)
}

proof fn selected_inputs_join_v1(before: SelectedReadersV1, middle: SelectedReadersV1,
    after: SelectedReadersV1, result: Result<(), ReadErrorV1>)
    requires selected_stable_step_v1(before, middle, Ok(())), selected_producer_step_v1(middle, after, result),
    ensures selected_inputs_step_v1(before, after, result),
{}

impl SelectedReadersV1 {
    fn release_prevalidated_submission_readers_v1(&mut self, id: u64) -> (result: Result<(), ReadErrorV1>)
        requires selected_readers_wf_v1(*old(self)), id == old(self).stable.id,
        ensures selected_readers_wf_v1(*final(self)), final(self).stable.id == old(self).stable.id,
            selected_stable_step_v1(*old(self), *final(self), result),
    {
        if self.stable.value.is_none() {
            self.journal.last_model_result = Ok(());
            return Ok(());
        }
        self.release_stable_v1(id)
    }

    fn release_validated_submission_producer_readers_v1(&mut self, id: u64) -> (result: Result<(), ReadErrorV1>)
        requires selected_readers_wf_v1(*old(self)), id == old(self).producer.id,
            old(self).producer.value.is_some(),
        ensures selected_readers_wf_v1(*final(self)), final(self).stable.id == old(self).stable.id,
            selected_producer_step_v1(*old(self), *final(self), result),
    { self.release_producer_v1(id) }

    fn release_inputs_v1(&mut self, id: u64) -> (result: Result<(), ReadErrorV1>)
        requires selected_readers_wf_v1(*old(self)), id == old(self).stable.id,
        ensures selected_readers_wf_v1(*final(self)), selected_inputs_step_v1(*old(self), *final(self), result),
    {
        let ghost before = *self;
        let producer = self.producer.value.is_some();
        completion_input_release_body!(verus_exec_expr, self, id, producer, [
            let ghost middle = *self;
            proof {
                if !producer { selected_inputs_join_v1(before, middle, *self, Ok(())); }
            }
        ], [
            proof { selected_inputs_join_v1(before, middle, *self, begin_result_from(self.journal.last_model_result)); }
        ])
    }

    fn release_stable_v1(&mut self, id: u64) -> (result: Result<(), ReadErrorV1>)
        requires selected_readers_wf_v1(*old(self)), id == old(self).stable.id,
            old(self).stable.value.is_some(),
        ensures selected_readers_wf_v1(*final(self)),
            final(self).stable.id == old(self).stable.id, final(self).records.id == old(self).records.id,
            final(self).producer == old(self).producer,
            result == begin_result_from(final(self).journal.last_model_result),
            producer_stable_release_relation_v1(old(self).journal.actual, final(self).journal.actual,
                old(self).stable.value.unwrap().marker.unwrap().first.consumer,
                old(self).stable.value.unwrap().references@,
                old(self).stable.value.unwrap().marker.unwrap().first.consumer,
                old(self).journal.actual.stable.leases@.len() as usize, result),
            logical::producer_outer_frame_v1(old(self).journal.model, final(self).journal.model),
            logical::release_execution_relation_v1(old(self).journal.model.stable, final(self).journal.model.stable,
                writer_key_view(old(self).stable.value.unwrap().marker.unwrap().first.consumer),
                stable_references_view(old(self).stable.value.unwrap().references@),
                writer_key_view(old(self).stable.value.unwrap().marker.unwrap().first.consumer),
                old(self).journal.actual.stable.leases@.len() as usize, final(self).journal.last_model_result),
            if result.is_ok() {
                final(self).stable.value.is_none()
                    && selected_stable_record_v1(old(self).records.value, final(self).records.value)
            } else {
                final(self).stable == old(self).stable && final(self).records == old(self).records
            },
    {
        completion_selected_reader_release_body!(verus_exec_expr,
            self.stable, self.records, self.journal, id, journal_read, release_reads, [])
    }

    fn release_producer_v1(&mut self, id: u64) -> (result: Result<(), ReadErrorV1>)
        requires selected_readers_wf_v1(*old(self)), id == old(self).producer.id,
            old(self).producer.value.is_some(),
        ensures selected_readers_wf_v1(*final(self)),
            final(self).producer.id == old(self).producer.id, final(self).records.id == old(self).records.id,
            final(self).stable == old(self).stable,
            result == begin_result_from(final(self).journal.last_model_result),
            producer_release_execution_relation_v1(old(self).journal.actual, final(self).journal.actual,
                old(self).producer.value.unwrap().marker.unwrap().first.consumer,
                old(self).producer.value.unwrap().references@,
                old(self).producer.value.unwrap().marker.unwrap().first.consumer,
                old(self).journal.actual.reservations@.len() as usize, result),
            logical::producer_release_execution_relation_v1(old(self).journal.model, final(self).journal.model,
                writer_key_view(old(self).producer.value.unwrap().marker.unwrap().first.consumer),
                producer_references_view(old(self).producer.value.unwrap().references@),
                writer_key_view(old(self).producer.value.unwrap().marker.unwrap().first.consumer),
                old(self).journal.actual.reservations@.len() as usize, final(self).journal.last_model_result),
            if result.is_ok() {
                final(self).producer.value.is_none()
                    && selected_producer_record_v1(old(self).records.value, final(self).records.value)
            } else {
                final(self).producer == old(self).producer && final(self).records == old(self).records
            },
    {
        completion_selected_reader_release_body!(verus_exec_expr,
            self.producer, self.records, self.journal, id, journal_producer_read, release_producer_reads, [])
    }
}

}
