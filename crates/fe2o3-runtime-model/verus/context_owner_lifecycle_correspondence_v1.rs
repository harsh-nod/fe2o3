// Actual and independent logical results are separate inputs, never assumed equal.
verus! {

#[allow(inconsistent_fields)]
enum LifecycleActualStepV1 {
    EnrollScalar { entry: EnrollmentV1, result: Result<AllocationReferenceV1, ReadErrorV1> },
    EnrollBatch { entries: Seq<EnrollmentV1>, original: Seq<Option<AllocationReferenceV1>>,
        output: Seq<Option<AllocationReferenceV1>>, result: Result<(), EnrollmentErrorV1> },
    Retire { roster: Seq<AllocationReferenceV1>, capacity: usize, result: Result<(), ReadErrorV1> },
    Register { key: WriterKeyV1, result: Result<WriterReferenceV1, ReadErrorV1> },
    Abort { reference: WriterReferenceV1, capacity: usize, result: Result<(), ReadErrorV1> },
    Begin { writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, result: Result<(), ReadErrorV1> },
    AcquireStable { consumer: WriterKeyV1, requests: Seq<ContextAllocationReadV1>, original: Seq<Option<ContextReadLeaseReferenceV1>>,
        output: Seq<Option<ContextReadLeaseReferenceV1>>, result: Result<(), ReadErrorV1> },
    ReleaseStable { consumer: WriterKeyV1, references: Seq<ContextReadLeaseReferenceV1>, evidence: WriterKeyV1,
        capacity: usize, result: Result<(), ReadErrorV1> },
    AcquireProducer { consumer: WriterKeyV1, requests: Seq<ContextProducerReadV1>, original: Seq<Option<ContextProducerReadReferenceV1>>,
        output: Seq<Option<ContextProducerReadReferenceV1>>, result: Result<(), ReadErrorV1> },
    ReleaseProducer { consumer: WriterKeyV1, references: Seq<ContextProducerReadReferenceV1>, evidence: WriterKeyV1,
        capacity: usize, result: Result<(), ReadErrorV1> },
    SettleSuccess { writer: WriterReferenceV1, evidence: WriterReferenceV1, writer_storage: usize,
        member_storage: usize, result: Result<(), ReadErrorV1> },
    SettleNoEffect { writer: WriterReferenceV1, evidence: WriterReferenceV1, writer_storage: usize,
        member_storage: usize, result: Result<(), ReadErrorV1> },
    Unknown { writer: WriterReferenceV1, result: Result<(), ReadErrorV1> },
    Dispose { writer: WriterReferenceV1, evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
        writer_storage: usize, member_storage: usize, allocation_storage: usize, result: Result<(), ReadErrorV1> },
}

spec fn lifecycle_actual_step_relation_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    step: LifecycleActualStepV1) -> bool
{
    match step {
        LifecycleActualStepV1::EnrollScalar { entry, result } => owner_writer_producer_frame_v1(before, after)
            && scalar_enrollment_relation_v1(before.stable.journal, after.stable.journal, entry.key, entry.device, entry.byte_extent, result),
        LifecycleActualStepV1::EnrollBatch { entries, original, output, result } =>
            producer_enrollment_relation_v1(before, after, entries, original, output, result),
        LifecycleActualStepV1::Retire { roster, capacity, result } => retirement_producer_relation_v1(before, after, roster, capacity, result),
        LifecycleActualStepV1::Register { key, result } => owner_writer_producer_frame_v1(before, after)
            && owner_register_relation_v1(before.stable.journal, after.stable.journal, key, result),
        LifecycleActualStepV1::Abort { reference, capacity, result } => owner_writer_producer_frame_v1(before, after)
            && owner_abort_relation_v1(before.stable.journal, after.stable.journal, reference, capacity, result),
        LifecycleActualStepV1::Begin { writer, roster, result } => producer_begin_relation_v1(before, after, writer, roster, result),
        LifecycleActualStepV1::AcquireStable { consumer, requests, original, output, result } =>
            producer_stable_acquire_relation_v1(before, after, consumer, requests, original, output, result),
        LifecycleActualStepV1::ReleaseStable { consumer, references, evidence, capacity, result } =>
            producer_stable_release_relation_v1(before, after, consumer, references, evidence, capacity, result),
        LifecycleActualStepV1::AcquireProducer { consumer, requests, original, output, result } =>
            producer_acquire_execution_relation_v1(before, after, consumer, requests, original, output, result),
        LifecycleActualStepV1::ReleaseProducer { consumer, references, evidence, capacity, result } =>
            producer_release_execution_relation_v1(before, after, consumer, references, evidence, capacity, result),
        LifecycleActualStepV1::SettleSuccess { writer, evidence, writer_storage, member_storage, result } =>
            owner_writer_producer_frame_v1(before, after) && settlement_execution_relation_v1(before.stable.journal,
                after.stable.journal, writer, evidence, writer_storage, member_storage, true, result),
        LifecycleActualStepV1::SettleNoEffect { writer, evidence, writer_storage, member_storage, result } =>
            owner_writer_producer_frame_v1(before, after) && settlement_execution_relation_v1(before.stable.journal,
                after.stable.journal, writer, evidence, writer_storage, member_storage, false, result),
        LifecycleActualStepV1::Unknown { writer, result } => producer_unknown_relation_v1(before, after, writer, result),
        LifecycleActualStepV1::Dispose { writer, evidence, roster, writer_storage, member_storage, allocation_storage, result } =>
            disposal_producer_relation_v1(before, after, writer, evidence, roster, writer_storage, member_storage, allocation_storage, result),
    }
}

spec fn lifecycle_step_inputs_v1(actual: LifecycleActualStepV1, model: logical::LifecycleStepV1) -> bool {
    match (actual, model) {
        (LifecycleActualStepV1::EnrollScalar { entry, .. }, logical::LifecycleStepV1::EnrollScalar { entry: m, .. }) => enrollment_view(entry) == m,
        (LifecycleActualStepV1::EnrollBatch { entries, original, .. }, logical::LifecycleStepV1::EnrollBatch { entries: m, original: o, .. }) =>
            entries_view(entries) == m && references_view(original) == o,
        (LifecycleActualStepV1::Retire { roster, capacity, .. }, logical::LifecycleStepV1::Retire { roster: m, capacity: c, .. }) =>
            retirement_roster_view_v1(roster) == m && capacity == c,
        (LifecycleActualStepV1::Register { key, .. }, logical::LifecycleStepV1::Register { key: m, .. }) => writer_key_view(key) == m,
        (LifecycleActualStepV1::Abort { reference, capacity, .. }, logical::LifecycleStepV1::Abort { reference: m, capacity: c, .. }) =>
            writer_reference_view(reference) == m && capacity == c,
        (LifecycleActualStepV1::Begin { writer, roster, .. }, logical::LifecycleStepV1::Begin { writer: m, roster: r, .. }) =>
            writer_reference_view(writer) == m && begin_roster_view(roster) == r,
        (LifecycleActualStepV1::AcquireStable { consumer, requests, original, .. },
            logical::LifecycleStepV1::AcquireStable { consumer: c, requests: r, original: o, .. }) =>
            writer_key_view(consumer) == c && stable_requests_view(requests) == r && stable_output_view(original) == o,
        (LifecycleActualStepV1::ReleaseStable { consumer, references, evidence, capacity, .. },
            logical::LifecycleStepV1::ReleaseStable { consumer: c, references: r, evidence: e, capacity: n, .. }) =>
            writer_key_view(consumer) == c && stable_references_view(references) == r && writer_key_view(evidence) == e && capacity == n,
        (LifecycleActualStepV1::AcquireProducer { consumer, requests, original, .. },
            logical::LifecycleStepV1::AcquireProducer { consumer: c, requests: r, original: o, .. }) =>
            writer_key_view(consumer) == c && producer_requests_view(requests) == r && producer_output_view(original) == o,
        (LifecycleActualStepV1::ReleaseProducer { consumer, references, evidence, capacity, .. },
            logical::LifecycleStepV1::ReleaseProducer { consumer: c, references: r, evidence: e, capacity: n, .. }) =>
            writer_key_view(consumer) == c && producer_references_view(references) == r && writer_key_view(evidence) == e && capacity == n,
        (LifecycleActualStepV1::SettleSuccess { writer, evidence, writer_storage, member_storage, .. },
            logical::LifecycleStepV1::SettleSuccess { writer: w, evidence: e, writer_storage: ws, member_storage: ms, .. }) =>
            writer_reference_view(writer) == w && writer_reference_view(evidence) == e && writer_storage == ws && member_storage == ms,
        (LifecycleActualStepV1::SettleNoEffect { writer, evidence, writer_storage, member_storage, .. },
            logical::LifecycleStepV1::SettleNoEffect { writer: w, evidence: e, writer_storage: ws, member_storage: ms, .. }) =>
            writer_reference_view(writer) == w && writer_reference_view(evidence) == e && writer_storage == ws && member_storage == ms,
        (LifecycleActualStepV1::Unknown { writer, .. }, logical::LifecycleStepV1::Unknown { writer: m, .. }) => writer_reference_view(writer) == m,
        (LifecycleActualStepV1::Dispose { writer, evidence, roster, writer_storage, member_storage, allocation_storage, .. },
            logical::LifecycleStepV1::Dispose { writer: w, evidence: e, roster: r, writer_storage: ws, member_storage: ms, allocation_storage: a, .. }) =>
            writer_reference_view(writer) == w && writer_reference_view(evidence) == e && begin_roster_view(roster) == r
                && writer_storage == ws && member_storage == ms && allocation_storage == a,
        _ => false,
    }
}

spec fn lifecycle_step_answers_v1(actual: LifecycleActualStepV1, model: logical::LifecycleStepV1) -> bool {
    match (actual, model) {
        (LifecycleActualStepV1::EnrollScalar { result, .. }, logical::LifecycleStepV1::EnrollScalar { result: m, .. }) => result == scalar_enrollment_result_from_v1(m),
        (LifecycleActualStepV1::EnrollBatch { result, output, .. }, logical::LifecycleStepV1::EnrollBatch { result: m, output: o, .. }) =>
            result == enrollment_result_from(m) && references_view(output) == o,
        (LifecycleActualStepV1::Register { result, .. }, logical::LifecycleStepV1::Register { result: m, .. }) => result == owner_register_result_from_v1(m),
        (LifecycleActualStepV1::Abort { result, .. }, logical::LifecycleStepV1::Abort { result: m, .. }) => result == owner_abort_result_from_v1(m),
        (LifecycleActualStepV1::AcquireStable { result, output, .. }, logical::LifecycleStepV1::AcquireStable { result: m, output: o, .. }) =>
            result == begin_result_from(m) && stable_output_view(output) == o,
        (LifecycleActualStepV1::AcquireProducer { result, output, .. }, logical::LifecycleStepV1::AcquireProducer { result: m, output: o, .. }) =>
            result == begin_result_from(m) && producer_output_view(output) == o,
        (LifecycleActualStepV1::Retire { result, .. }, logical::LifecycleStepV1::Retire { result: m, .. })
        | (LifecycleActualStepV1::Begin { result, .. }, logical::LifecycleStepV1::Begin { result: m, .. })
        | (LifecycleActualStepV1::ReleaseStable { result, .. }, logical::LifecycleStepV1::ReleaseStable { result: m, .. })
        | (LifecycleActualStepV1::ReleaseProducer { result, .. }, logical::LifecycleStepV1::ReleaseProducer { result: m, .. })
        | (LifecycleActualStepV1::SettleSuccess { result, .. }, logical::LifecycleStepV1::SettleSuccess { result: m, .. })
        | (LifecycleActualStepV1::SettleNoEffect { result, .. }, logical::LifecycleStepV1::SettleNoEffect { result: m, .. })
        | (LifecycleActualStepV1::Unknown { result, .. }, logical::LifecycleStepV1::Unknown { result: m, .. })
        | (LifecycleActualStepV1::Dispose { result, .. }, logical::LifecycleStepV1::Dispose { result: m, .. }) => result == begin_result_from(m),
        _ => false,
    }
}

#[verifier::spinoff_prover]
proof fn lifecycle_mutation_correspondence_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    model_before: logical::ProducerReadContentsV1, model_after: logical::ProducerReadContentsV1,
    actual_step: LifecycleActualStepV1, model_step: logical::LifecycleStepV1,
    storage: logical::StorageCapacitiesV1, history: Seq<logical::WriterReferenceV1>)
    requires producer_represents(before, model_before), logical::issued_producer_v1(model_before, storage, history),
        lifecycle_actual_step_relation_v1(before, after, actual_step),
        logical::lifecycle_step_relation_v1(model_before, model_after, model_step), lifecycle_step_inputs_v1(actual_step, model_step),
    ensures producer_represents(after, model_after), lifecycle_step_answers_v1(actual_step, model_step),
{
    match (actual_step, model_step) {
        (LifecycleActualStepV1::EnrollScalar { entry, result }, logical::LifecycleStepV1::EnrollScalar { result: m, .. }) => {
            scalar_enrollment_paired_transition_v1(before.stable.journal, after.stable.journal,
                model_before.stable.journal, model_after.stable.journal, entry, result, m);
        },
        (LifecycleActualStepV1::EnrollBatch { entries, original, output, result },
            logical::LifecycleStepV1::EnrollBatch { output: o, result: m, .. }) => {
            enrollment_paired_transition(before.stable.journal, after.stable.journal, model_before.stable.journal,
                model_after.stable.journal, entries, original, output, o, result, m);
        },
        (LifecycleActualStepV1::Retire { roster, capacity, result }, logical::LifecycleStepV1::Retire { result: m, .. }) => {
            owner_retirement_invariant_domain_v1(before, model_before, roster);
            retirement_owner_paired_transition_v1(before, after, model_before, model_after, roster, capacity, result, m);
        },
        (LifecycleActualStepV1::Register { key, result }, logical::LifecycleStepV1::Register { result: m, .. }) => {
            owner_register_paired_transition_v1(before.stable.journal, after.stable.journal,
                model_before.stable.journal, model_after.stable.journal, key, result, m);
        },
        (LifecycleActualStepV1::Abort { reference, capacity, result }, logical::LifecycleStepV1::Abort { result: m, .. }) => {
            owner_abort_paired_transition_v1(before.stable.journal, after.stable.journal,
                model_before.stable.journal, model_after.stable.journal, reference, capacity, result, m);
        },
        (LifecycleActualStepV1::Begin { writer, roster, result }, logical::LifecycleStepV1::Begin { result: m, .. }) => {
            logical::lifecycle_begin_preserves_v1(model_before, model_after, writer_reference_view(writer), begin_roster_view(roster), m, storage, history);
            producer_begin_paired_transition(before, after, model_before, model_after, writer, roster, result, m, storage, history);
        },
        (LifecycleActualStepV1::AcquireStable { consumer, requests, original, output, result },
            logical::LifecycleStepV1::AcquireStable { output: o, result: m, .. }) => {
            logical::producer_capacity_arithmetic_v1(model_before);
            producer_stable_acquire_paired_transition(before, after, model_before, model_after, consumer,
                requests, original, output, o, result, m);
        },
        (LifecycleActualStepV1::ReleaseStable { consumer, references, evidence, capacity, result },
            logical::LifecycleStepV1::ReleaseStable { result: m, .. }) => {
            stable_release_paired_transition(before.stable, after.stable, model_before.stable, model_after.stable,
                consumer, references, evidence, capacity, result, m);
        },
        (LifecycleActualStepV1::AcquireProducer { consumer, requests, original, output, result },
            logical::LifecycleStepV1::AcquireProducer { output: o, result: m, .. }) => {
            logical::producer_capacity_arithmetic_v1(model_before);
            producer_acquire_paired_transition(before, after, model_before, model_after, consumer,
                requests, original, output, o, result, m);
        },
        (LifecycleActualStepV1::ReleaseProducer { consumer, references, evidence, capacity, result },
            logical::LifecycleStepV1::ReleaseProducer { result: m, .. }) => {
            producer_release_paired_transition(before, after, model_before, model_after,
                consumer, references, evidence, capacity, result, m);
        },
        (LifecycleActualStepV1::SettleSuccess { writer, evidence, writer_storage, member_storage, result },
            logical::LifecycleStepV1::SettleSuccess { result: m, .. }) => {
            owner_settlement_paired_transition_v1(before.stable.journal, after.stable.journal, model_before.stable.journal,
                model_after.stable.journal, writer, evidence, writer_storage, member_storage, true, result, m);
        },
        (LifecycleActualStepV1::SettleNoEffect { writer, evidence, writer_storage, member_storage, result },
            logical::LifecycleStepV1::SettleNoEffect { result: m, .. }) => {
            owner_settlement_paired_transition_v1(before.stable.journal, after.stable.journal, model_before.stable.journal,
                model_after.stable.journal, writer, evidence, writer_storage, member_storage, false, result, m);
        },
        (LifecycleActualStepV1::Unknown { writer, result }, logical::LifecycleStepV1::Unknown { result: m, .. }) => {
            owner_unknown_paired_transition(before.stable.journal, after.stable.journal,
                model_before.stable.journal, model_after.stable.journal, writer, result, m);
        },
        (LifecycleActualStepV1::Dispose { writer, evidence, roster, writer_storage, member_storage, allocation_storage, result },
            logical::LifecycleStepV1::Dispose { result: m, .. }) => {
            owner_disposal_invariant_domain_v1(before, model_before, roster);
            disposal_owner_paired_transition_v1(before, after, model_before, model_after, writer, evidence, roster,
                writer_storage, member_storage, allocation_storage, result, m);
        },
        _ => {},
    }
}

}
