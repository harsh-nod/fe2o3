// Closed mutating alphabet. These relations contain no resulting-state invariant.
use super::*;

verus! {

#[allow(inconsistent_fields)]
pub enum LifecycleStepV1 {
    EnrollScalar { entry: EnrollmentV1, result: Result<AllocationReferenceV1, EnrollmentErrorV1> },
    EnrollBatch { entries: Seq<EnrollmentV1>, original: Seq<Option<AllocationReferenceV1>>,
        output: Seq<Option<AllocationReferenceV1>>, result: Result<(), EnrollmentErrorV1> },
    Retire { roster: Seq<AllocationReferenceV1>, capacity: usize, result: Result<(), ReadErrorV1> },
    Register { key: WriterKeyV1, result: Result<WriterReferenceV1, JournalErrorV1> },
    Abort { reference: WriterReferenceV1, capacity: usize, result: Result<(), JournalErrorV1> },
    Begin { writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, result: Result<(), ReadErrorV1> },
    AcquireStable { consumer: WriterKeyV1, requests: Seq<AllocationReadV1>, original: Seq<Option<ReadReferenceV1>>,
        output: Seq<Option<ReadReferenceV1>>, result: Result<(), ReadErrorV1> },
    ReleaseStable { consumer: WriterKeyV1, references: Seq<ReadReferenceV1>, evidence: WriterKeyV1,
        capacity: usize, result: Result<(), ReadErrorV1> },
    AcquireProducer { consumer: WriterKeyV1, requests: Seq<ProducerReadV1>, original: Seq<Option<ProducerReadReferenceV1>>,
        output: Seq<Option<ProducerReadReferenceV1>>, result: Result<(), ReadErrorV1> },
    AcquireMixed { consumer: WriterKeyV1, stable: Seq<AllocationReadV1>, pending: Seq<ProducerReadV1>,
        stable_original: Seq<Option<ReadReferenceV1>>, stable_output: Seq<Option<ReadReferenceV1>>,
        producer_original: Seq<Option<ProducerReadReferenceV1>>, producer_output: Seq<Option<ProducerReadReferenceV1>>,
        result: Result<(), ReadErrorV1> },
    ReleaseProducer { consumer: WriterKeyV1, references: Seq<ProducerReadReferenceV1>, evidence: WriterKeyV1,
        capacity: usize, result: Result<(), ReadErrorV1> },
    SettleSuccess { writer: WriterReferenceV1, evidence: WriterReferenceV1, writer_storage: usize,
        member_storage: usize, result: Result<(), ReadErrorV1> },
    SettleNoEffect { writer: WriterReferenceV1, evidence: WriterReferenceV1, writer_storage: usize,
        member_storage: usize, result: Result<(), ReadErrorV1> },
    Unknown { writer: WriterReferenceV1, result: Result<(), ReadErrorV1> },
    Dispose { writer: WriterReferenceV1, evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
        writer_storage: usize, member_storage: usize, allocation_storage: usize, result: Result<(), ReadErrorV1> },
}

pub open spec fn lifecycle_register_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    key: WriterKeyV1, result: Result<WriterReferenceV1, JournalErrorV1>) -> bool
{
    &&& producer_storage_frame_v1(before, after)
    &&& issuance_contents_frame_v1(before.stable.journal, after.stable.journal)
    &&& register_execution_relation_v1(before.stable.journal.context_generation, before.stable.journal.writer_capacity,
        key, before.stable.journal.writers@, before.stable.journal.free@,
        before.stable.journal.registration_watermark, before.stable.journal.reserved_count,
        after.stable.journal.writers@, after.stable.journal.free@,
        after.stable.journal.registration_watermark, after.stable.journal.reserved_count, result)
}

pub open spec fn lifecycle_abort_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    reference: WriterReferenceV1, capacity: usize, result: Result<(), JournalErrorV1>) -> bool
{
    &&& producer_storage_frame_v1(before, after)
    &&& issuance_contents_frame_v1(before.stable.journal, after.stable.journal)
    &&& abort_execution_relation_v1(before.stable.journal.context_generation, before.stable.journal.writer_capacity,
        capacity, reference, before.stable.journal.writers@, before.stable.journal.free@,
        before.stable.journal.registration_watermark, before.stable.journal.reserved_count,
        after.stable.journal.writers@, after.stable.journal.free@,
        after.stable.journal.registration_watermark, after.stable.journal.reserved_count, result)
}

pub open spec fn lifecycle_settlement_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, writer_storage: usize, member_storage: usize,
    success: bool, result: Result<(), ReadErrorV1>) -> bool
{
    &&& producer_storage_frame_v1(before, after)
    &&& settlement_execution_relation_v1(before.stable.journal, after.stable.journal, writer, evidence,
        writer_storage, member_storage, success, result)
}

pub open spec fn lifecycle_step_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    step: LifecycleStepV1) -> bool
{
    match step {
        LifecycleStepV1::EnrollScalar { entry, result } => producer_storage_frame_v1(before, after)
            && scalar_enrollment_relation_v1(before.stable.journal, after.stable.journal, entry, result),
        LifecycleStepV1::EnrollBatch { entries, original, output, result } => entries.len() <= usize::MAX
            && original.len() <= usize::MAX && output.len() <= usize::MAX && producer_storage_frame_v1(before, after)
            && enrollment_execution_relation_v1(before.stable.journal, after.stable.journal, entries, original, output, result),
        LifecycleStepV1::Retire { roster, capacity, result } => roster.len() <= usize::MAX
            && retirement_producer_relation_v1(before, after, roster, capacity, result),
        LifecycleStepV1::Register { key, result } => lifecycle_register_relation_v1(before, after, key, result),
        LifecycleStepV1::Abort { reference, capacity, result } => lifecycle_abort_relation_v1(before, after, reference, capacity, result),
        LifecycleStepV1::Begin { writer, roster, result } => roster.len() <= usize::MAX
            && lifecycle_begin_relation_v1(before, after, writer, roster, result),
        LifecycleStepV1::AcquireStable { consumer, requests, original, output, result } =>
            requests.len() <= usize::MAX && requests.len() <= u64::MAX && original.len() <= usize::MAX && output.len() <= usize::MAX
            && stable_wrapper_acquire_relation_v1(before, after, consumer, requests, original, output, result),
        LifecycleStepV1::ReleaseStable { consumer, references, evidence, capacity, result } => references.len() <= usize::MAX
            && producer_outer_frame_v1(before, after)
            && release_execution_relation_v1(before.stable, after.stable, consumer, references, evidence, capacity, result),
        LifecycleStepV1::AcquireProducer { consumer, requests, original, output, result } =>
            requests.len() <= usize::MAX && requests.len() <= u64::MAX && original.len() <= usize::MAX && output.len() <= usize::MAX
            && producer_acquire_execution_relation_v1(before, after, consumer, requests, original, output, result),
        LifecycleStepV1::AcquireMixed { consumer, stable, pending, stable_original, stable_output,
            producer_original, producer_output, result } =>
            stable.len() <= usize::MAX && stable.len() <= u64::MAX && pending.len() <= usize::MAX && pending.len() <= u64::MAX
            && stable_original.len() <= usize::MAX && stable_output.len() <= usize::MAX
            && producer_original.len() <= usize::MAX && producer_output.len() <= usize::MAX
            && mixed_acquire_relation_v1(before, after, consumer, stable, pending,
                stable_original, stable_output, producer_original, producer_output, result),
        LifecycleStepV1::ReleaseProducer { consumer, references, evidence, capacity, result } => references.len() <= usize::MAX
            && producer_release_execution_relation_v1(before, after, consumer, references, evidence, capacity, result),
        LifecycleStepV1::SettleSuccess { writer, evidence, writer_storage, member_storage, result } =>
            lifecycle_settlement_relation_v1(before, after, writer, evidence, writer_storage, member_storage, true, result),
        LifecycleStepV1::SettleNoEffect { writer, evidence, writer_storage, member_storage, result } =>
            lifecycle_settlement_relation_v1(before, after, writer, evidence, writer_storage, member_storage, false, result),
        LifecycleStepV1::Unknown { writer, result } => producer_storage_frame_v1(before, after)
            && unknown_execution_relation_v1(before.stable.journal, after.stable.journal, writer, result),
        LifecycleStepV1::Dispose { writer, evidence, roster, writer_storage, member_storage, allocation_storage, result } =>
            roster.len() <= usize::MAX && disposal_producer_relation_v1(before, after, writer, evidence, roster,
                writer_storage, member_storage, allocation_storage, result),
    }
}

pub open spec fn lifecycle_writer_history_v1(history: Seq<WriterReferenceV1>, step: LifecycleStepV1) -> Seq<WriterReferenceV1> {
    match step { LifecycleStepV1::Register { result, .. } => registration_history_v1(history, result), _ => history }
}

pub open spec fn lifecycle_stable_minted_v1(step: LifecycleStepV1) -> Seq<ReadReferenceV1> {
    match step { LifecycleStepV1::AcquireStable { output, result, .. }
        | LifecycleStepV1::AcquireMixed { stable_output: output, result, .. } =>
        if result.is_ok() { Seq::new(output.len(), |i: int| output[i].unwrap()) } else { Seq::empty() }, _ => Seq::empty() }
}

pub open spec fn lifecycle_producer_minted_v1(step: LifecycleStepV1) -> Seq<ProducerReadReferenceV1> {
    match step { LifecycleStepV1::AcquireProducer { output, result, .. }
        | LifecycleStepV1::AcquireMixed { producer_output: output, result, .. } =>
        if result.is_ok() { Seq::new(output.len(), |i: int| output[i].unwrap()) } else { Seq::empty() }, _ => Seq::empty() }
}

pub struct LifecycleHistoryV1 {
    pub writers: Seq<WriterReferenceV1>,
    pub stable: Seq<ReadReferenceV1>,
    pub producer: Seq<ProducerReadReferenceV1>,
}

pub open spec fn lifecycle_empty_history_v1() -> LifecycleHistoryV1 {
    LifecycleHistoryV1 { writers: Seq::empty(), stable: Seq::empty(), producer: Seq::empty() }
}

pub open spec fn lifecycle_next_history_v1(history: LifecycleHistoryV1, step: LifecycleStepV1) -> LifecycleHistoryV1 {
    LifecycleHistoryV1 { writers: lifecycle_writer_history_v1(history.writers, step),
        stable: history.stable + lifecycle_stable_minted_v1(step),
        producer: history.producer + lifecycle_producer_minted_v1(step) }
}

pub open spec fn lifecycle_stable_history_v1(contents: ReadContentsV1, history: Seq<ReadReferenceV1>) -> bool {
    &&& contents.next_incarnation == 1 + history.len()
    &&& forall|i: int| 0 <= i < history.len() ==> (#[trigger] history[i]).incarnation == i + 1
    &&& forall|s: int| 0 <= s < contents.leases@.len() && (#[trigger] contents.leases@[s]).is_some()
        ==> history.contains(contents.leases@[s].unwrap().reference)
}

pub open spec fn lifecycle_producer_history_v1(contents: ProducerReadContentsV1, history: Seq<ProducerReadReferenceV1>) -> bool {
    &&& contents.next_incarnation == 1 + history.len()
    &&& forall|i: int| 0 <= i < history.len() ==> (#[trigger] history[i]).incarnation == i + 1
    &&& forall|s: int| 0 <= s < contents.reservations@.len() && (#[trigger] contents.reservations@[s]).is_some()
        ==> history.contains(contents.reservations@[s].unwrap().reference)
}

pub open spec fn lifecycle_invariant_v1(contents: ProducerReadContentsV1, storage: StorageCapacitiesV1,
    history: LifecycleHistoryV1) -> bool
{
    &&& issued_producer_v1(contents, storage, history.writers)
    &&& lifecycle_stable_history_v1(contents.stable, history.stable)
    &&& lifecycle_producer_history_v1(contents, history.producer)
}

}
