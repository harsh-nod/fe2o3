completion_writer_outcome_declaration!(verus, pub);

verus! {

// This adapter executes journal operations, not Context maps or quarantine.
struct CompletionJournalInputsV1 {
    consumer: WriterKeyV1,
    model_consumer: logical::WriterKeyV1,
    stable_present: bool,
    producer_present: bool,
    stable: Vec<ContextReadLeaseReferenceV1>,
    model_stable: Vec<logical::ReadReferenceV1>,
    producer: Vec<ContextProducerReadReferenceV1>,
    model_producer: Vec<logical::ProducerReadReferenceV1>,
    stable_free: usize,
    producer_free: usize,
    writer: Option<WriterReferenceV1>,
    model_writer: Option<logical::WriterReferenceV1>,
    writer_free: usize,
    member_free: usize,
}

spec fn completion_inputs_wf_v1(inputs: CompletionJournalInputsV1) -> bool {
    &&& writer_key_view(inputs.consumer) == inputs.model_consumer
    &&& stable_references_view(inputs.stable@) == inputs.model_stable@
    &&& producer_references_view(inputs.producer@) == inputs.model_producer@
    &&& !inputs.stable_present ==> inputs.stable@.len() == 0
    &&& !inputs.producer_present ==> inputs.producer@.len() == 0
    &&& match (inputs.writer, inputs.model_writer) {
        (Some(actual), Some(model)) => writer_reference_view(actual) == model && actual.key == inputs.consumer,
        (None, None) => true,
        _ => false,
    }
}

spec fn completion_stable_relation_v1(before: logical::ProducerReadContentsV1,
    after: logical::ProducerReadContentsV1, inputs: CompletionJournalInputsV1,
    result: Result<(), logical::ReadErrorV1>) -> bool {
    if inputs.stable_present {
        &&& logical::producer_outer_frame_v1(before, after)
        &&& logical::release_execution_relation_v1(before.stable, after.stable,
            inputs.model_consumer, inputs.model_stable@, inputs.model_consumer, inputs.stable_free, result)
    } else {
        logical::producer_contents_frame_v1(before, after) && result == Ok(())
    }
}

spec fn completion_producer_relation_v1(before: logical::ProducerReadContentsV1,
    after: logical::ProducerReadContentsV1, inputs: CompletionJournalInputsV1,
    result: Result<(), logical::ReadErrorV1>) -> bool {
    if inputs.producer_present {
        logical::producer_release_execution_relation_v1(before, after,
            inputs.model_consumer, inputs.model_producer@, inputs.model_consumer, inputs.producer_free, result)
    } else {
        logical::producer_contents_frame_v1(before, after) && result == Ok(())
    }
}

spec fn completion_input_relation_v1(before: logical::ProducerReadContentsV1,
    after: logical::ProducerReadContentsV1, inputs: CompletionJournalInputsV1,
    result: Result<(), logical::ReadErrorV1>) -> bool {
    ||| result.is_err() && completion_stable_relation_v1(before, after, inputs, result)
    ||| exists|middle: logical::ProducerReadContentsV1|
        completion_stable_relation_v1(before, middle, inputs, Ok(()))
        && completion_producer_relation_v1(middle, after, inputs, result)
}

spec fn completion_optional_writer_relation_v1(before: logical::ProducerReadContentsV1,
    after: logical::ProducerReadContentsV1, inputs: CompletionJournalInputsV1,
    outcome: SubmissionWriterOutcomeV1, result: Result<(), logical::ReadErrorV1>) -> bool {
    match inputs.model_writer {
        Some(writer) => completion_writer_relation_v1(before, after, writer, outcome,
            inputs.writer_free, inputs.member_free, result),
        None => before == after && result == Ok(()),
    }
}

spec fn completion_journal_relation_v1(before: logical::ProducerReadContentsV1,
    after: logical::ProducerReadContentsV1, inputs: CompletionJournalInputsV1,
    outcome: SubmissionWriterOutcomeV1, result: Result<(), logical::ReadErrorV1>) -> bool {
    ||| result.is_err() && completion_input_relation_v1(before, after, inputs, result)
    ||| exists|middle: logical::ProducerReadContentsV1|
        completion_input_relation_v1(before, middle, inputs, Ok(()))
        && completion_optional_writer_relation_v1(middle, after, inputs, outcome, result)
}

proof fn completion_input_join_v1(before: logical::ProducerReadContentsV1,
    middle: logical::ProducerReadContentsV1, after: logical::ProducerReadContentsV1,
    inputs: CompletionJournalInputsV1, result: Result<(), logical::ReadErrorV1>)
    requires completion_stable_relation_v1(before, middle, inputs, Ok(())),
        completion_producer_relation_v1(middle, after, inputs, result),
    ensures completion_input_relation_v1(before, after, inputs, result),
{}

proof fn completion_journal_join_v1(before: logical::ProducerReadContentsV1,
    middle: logical::ProducerReadContentsV1, after: logical::ProducerReadContentsV1,
    inputs: CompletionJournalInputsV1, outcome: SubmissionWriterOutcomeV1,
    result: Result<(), logical::ReadErrorV1>)
    requires completion_input_relation_v1(before, middle, inputs, Ok(())),
        completion_optional_writer_relation_v1(middle, after, inputs, outcome, result),
    ensures completion_journal_relation_v1(before, after, inputs, outcome, result),
{}

struct CompletionJournalPairV1 {
    actual: ContextProducerReadJournalV1,
    model: logical::ProducerReadContentsV1,
    inputs: CompletionJournalInputsV1,
    last_model_result: Result<(), logical::ReadErrorV1>,
}

spec fn completion_pair_wf_v1(pair: CompletionJournalPairV1) -> bool {
    &&& producer_represents(pair.actual, pair.model)
    &&& logical::producer_invariant_v1(pair.model)
    &&& completion_inputs_wf_v1(pair.inputs)
}

impl CompletionJournalPairV1 {
    fn release_prevalidated_submission_readers_v1(&mut self, consumer: WriterKeyV1)
        -> (result: Result<(), ReadErrorV1>)
        requires completion_pair_wf_v1(*old(self)), consumer == old(self).inputs.consumer,
        ensures completion_pair_wf_v1(*final(self)), final(self).inputs == old(self).inputs,
            result == begin_result_from(final(self).last_model_result),
            completion_stable_relation_v1(old(self).model, final(self).model,
                old(self).inputs, final(self).last_model_result),
    {
        if !self.inputs.stable_present {
            self.last_model_result = Ok(());
            return Ok(());
        }
        let (actual, model) = producer_stable_release_historical_exec_v1(&mut self.actual, &mut self.model,
            consumer, self.inputs.model_consumer, &self.inputs.stable, &self.inputs.model_stable,
            &ContextReadQuiescenceEvidenceV1 { consumer }, self.inputs.model_consumer, self.inputs.stable_free);
        self.last_model_result = model;
        actual
    }

    fn release_validated_submission_producer_readers_v1(&mut self, consumer: WriterKeyV1)
        -> (result: Result<(), ReadErrorV1>)
        requires completion_pair_wf_v1(*old(self)), consumer == old(self).inputs.consumer,
            old(self).inputs.producer_present,
        ensures completion_pair_wf_v1(*final(self)), final(self).inputs == old(self).inputs,
            result == begin_result_from(final(self).last_model_result),
            completion_producer_relation_v1(old(self).model, final(self).model,
                old(self).inputs, final(self).last_model_result),
    {
        let (actual, model) = producer_release_historical_exec_v1(&mut self.actual, &mut self.model,
            consumer, self.inputs.model_consumer, &self.inputs.producer, &self.inputs.model_producer,
            &ContextReadQuiescenceEvidenceV1 { consumer }, self.inputs.model_consumer, self.inputs.producer_free);
        self.last_model_result = model;
        actual
    }

    fn release_submission_inputs_v1(&mut self, consumer: WriterKeyV1)
        -> (result: Result<(), ReadErrorV1>)
        requires completion_pair_wf_v1(*old(self)), consumer == old(self).inputs.consumer,
        ensures completion_pair_wf_v1(*final(self)), final(self).inputs == old(self).inputs,
            result == begin_result_from(final(self).last_model_result),
            completion_input_relation_v1(old(self).model, final(self).model,
                old(self).inputs, final(self).last_model_result),
    {
        let ghost before = self.model;
        let producer = self.inputs.producer_present;
        completion_input_release_body!(verus_exec_expr, self, consumer, producer, [
            let ghost middle = self.model;
            proof {
                if !producer {
                    completion_input_join_v1(before, middle, self.model, self.inputs, self.last_model_result);
                }
            }
        ], [
            proof { completion_input_join_v1(before, middle, self.model, self.inputs, self.last_model_result); }
        ])
    }

    fn settle_submission_writer_v1(&mut self, consumer: WriterKeyV1, outcome: SubmissionWriterOutcomeV1)
        -> (result: Result<(), ReadErrorV1>)
        requires completion_pair_wf_v1(*old(self)), consumer == old(self).inputs.consumer,
        ensures completion_pair_wf_v1(*final(self)), final(self).inputs == old(self).inputs,
            result == begin_result_from(final(self).last_model_result),
            completion_optional_writer_relation_v1(old(self).model, final(self).model,
                old(self).inputs, outcome, final(self).last_model_result),
    {
        match self.inputs.writer {
            None => {
                self.last_model_result = Ok(());
                Ok(())
            },
            Some(writer) => {
                let model_writer = self.inputs.model_writer.unwrap();
                let (actual, model) = completion_writer_paired_exec_v1(&mut self.actual, &mut self.model,
                    writer, model_writer, outcome, self.inputs.writer_free, self.inputs.member_free);
                self.last_model_result = model;
                actual
            },
        }
    }

    fn complete_journal_prefix_v1(&mut self, consumer: WriterKeyV1, outcome: SubmissionWriterOutcomeV1)
        -> (result: Result<(), ReadErrorV1>)
        requires completion_pair_wf_v1(*old(self)), consumer == old(self).inputs.consumer,
        ensures completion_pair_wf_v1(*final(self)), final(self).inputs == old(self).inputs,
            result == begin_result_from(final(self).last_model_result),
            completion_journal_relation_v1(old(self).model, final(self).model,
                old(self).inputs, outcome, final(self).last_model_result),
    {
        let ghost before = self.model;
        completion_journal_prefix_body!(verus_exec_expr, self, consumer, outcome, [
            let ghost middle = self.model;
        ], [
            proof { completion_journal_join_v1(before, middle, self.model, self.inputs, outcome, self.last_model_result); }
        ]);
        Ok(())
    }
}

spec fn completion_writer_relation_v1(before: logical::ProducerReadContentsV1,
    after: logical::ProducerReadContentsV1, writer: logical::WriterReferenceV1,
    outcome: SubmissionWriterOutcomeV1, free_storage: usize, member_free_storage: usize,
    result: Result<(), logical::ReadErrorV1>) -> bool {
    &&& owner_model_outer_frame_v1(before, after)
    &&& match outcome {
        SubmissionWriterOutcomeV1::Success => logical::settlement_execution_relation_v1(
            before.stable.journal, after.stable.journal, writer, writer, free_storage, member_free_storage, true, result),
        SubmissionWriterOutcomeV1::NoEffect => logical::settlement_execution_relation_v1(
            before.stable.journal, after.stable.journal, writer, writer, free_storage, member_free_storage, false, result),
        SubmissionWriterOutcomeV1::Unknown => logical::unknown_execution_relation_v1(
            before.stable.journal, after.stable.journal, writer, result),
    }
}

fn completion_writer_paired_exec_v1(actual: &mut ContextProducerReadJournalV1,
    model: &mut logical::ProducerReadContentsV1, writer: WriterReferenceV1,
    model_writer: logical::WriterReferenceV1, outcome: SubmissionWriterOutcomeV1,
    free_storage: usize, member_free_storage: usize)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*old(actual), *old(model)), writer_reference_view(writer) == model_writer,
    ensures results.0 == begin_result_from(results.1), producer_represents(*final(actual), *final(model)),
        completion_writer_relation_v1(*old(model), *final(model), model_writer, outcome,
            free_storage, member_free_storage, results.1),
        owner_writer_producer_frame_v1(*old(actual), *final(actual)),
        results.0.is_err() ==> *final(actual) == *old(actual),
        results.1.is_err() ==> *final(model) == *old(model),
        logical::producer_invariant_v1(*old(model)) ==> logical::producer_invariant_v1(*final(model)),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let result = completion_writer_effect_body!(verus_exec_expr, actual, writer, outcome,
        settle_success_observed_v1, settle_no_effect_observed_v1, [, free_storage, member_free_storage]);
    let model_result = match outcome {
        SubmissionWriterOutcomeV1::Success => logical::settlement_exec_v1(&mut model.stable.journal,
            model_writer, model_writer, free_storage, member_free_storage, true),
        SubmissionWriterOutcomeV1::NoEffect => logical::settlement_exec_v1(&mut model.stable.journal,
            model_writer, model_writer, free_storage, member_free_storage, false),
        SubmissionWriterOutcomeV1::Unknown => logical::unknown_exec_v1(&mut model.stable.journal, model_writer),
    };
    proof {
        match outcome {
            SubmissionWriterOutcomeV1::Unknown => {
                owner_unknown_paired_transition(before.stable.journal, actual.stable.journal,
                    model_before.stable.journal, model.stable.journal, writer, result, model_result);
                if logical::producer_invariant_v1(model_before) && model_result.is_ok() {
                    logical::unknown_preserves_producer_invariant_v1(model_before, *model, model_writer);
                }
            },
            _ => {
                let success = match outcome {
                    SubmissionWriterOutcomeV1::Success => true,
                    _ => false,
                };
                owner_settlement_paired_transition_v1(before.stable.journal, actual.stable.journal,
                    model_before.stable.journal, model.stable.journal, writer, writer,
                    free_storage, member_free_storage, success, result, model_result);
                owner_settlement_preservation_v1(model_before, *model, model_writer, model_writer,
                    free_storage, member_free_storage, success, model_result,
                    logical::witness_storage_v1(0, 0), Seq::empty());
            },
        }
    }
    (result, model_result)
}

}
