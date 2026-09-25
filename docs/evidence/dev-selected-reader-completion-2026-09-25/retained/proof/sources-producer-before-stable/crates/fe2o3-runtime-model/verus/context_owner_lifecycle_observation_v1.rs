// Immutable observation alphabet distinguishes shadowed producer/stable APIs.
verus! {

enum LifecycleGetterV1 {
    ContextGeneration, AllocationCapacity, WriterCapacity, RegistrationWatermark,
    RemainingWriterSlots, ReservedWriterCount, RemainingAllocationSlots,
    StableRemainingReadSlots, StableRetainedReadCount,
    ProducerRemainingReadSlots, ProducerRetainedReadCount, ProducerTotalReadCount,
}

enum LifecycleReceiverV1 { Journal, Stable, Producer }

#[allow(inconsistent_fields)]
enum LifecycleQueryV1 {
    Getter(LifecycleGetterV1),
    InspectJournal,
    InspectStable { explicit: bool },
    InspectProducer { explicit: bool },
    LookupReserved(WriterReferenceV1),
    LookupWriter(WriterReferenceV1),
    LookupAllocation(AllocationReferenceV1),
    ReaderCount { producer: bool, allocation: AllocationReferenceV1 },
    StableCapacity(usize), CombinedStableCapacity(usize), ProducerCapacity(usize),
    ValidateRead(ContextAllocationReadV1), ValidateProducer(ContextProducerReadV1),
    LookupRead(ContextReadLeaseReferenceV1), LookupProducer(ContextProducerReadReferenceV1),
    ProducerStatus(ContextProducerReadReferenceV1),
    ValidateRetirement { receiver: LifecycleReceiverV1, roster: Seq<AllocationReferenceV1>, capacity: usize },
    ValidateDisposal { receiver: LifecycleReceiverV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
        writer_storage: usize, member_storage: usize, allocation_storage: usize },
}

enum LifecycleAnswerV1 {
    Scalar(int), Journal(JournalInspectionV1), Owner(OwnerInspectionV1),
    Reserved(Result<WriterKeyV1, ReadErrorV1>), Writer(Result<ContextWriterStateV1, ReadErrorV1>),
    Allocation(Result<ContextAllocationStateV1, ReadErrorV1>), Count(Result<usize, ReadErrorV1>),
    Unit(Result<(), ReadErrorV1>), StableRead(Result<ContextAllocationReadV1, ReadErrorV1>),
    ProducerRead(Result<ContextProducerReadV1, ReadErrorV1>), Status(Result<ContextProducerReadStatusV1, ReadErrorV1>),
}

spec fn lifecycle_getter_actual_v1(owner: ContextProducerReadJournalV1, getter: LifecycleGetterV1) -> int {
    match getter {
        LifecycleGetterV1::ContextGeneration => owner.stable.journal.context_generation as int,
        LifecycleGetterV1::AllocationCapacity => owner.stable.journal.allocation_capacity as int,
        LifecycleGetterV1::WriterCapacity => owner.stable.journal.writer_capacity as int,
        LifecycleGetterV1::RegistrationWatermark => owner.stable.journal.registration_watermark as int,
        LifecycleGetterV1::RemainingWriterSlots => owner.stable.journal.free@.len() as int,
        LifecycleGetterV1::ReservedWriterCount => owner.stable.journal.reserved_count as int,
        LifecycleGetterV1::RemainingAllocationSlots => owner.stable.journal.allocation_free@.len() as int,
        LifecycleGetterV1::StableRemainingReadSlots => owner.stable.free_reads@.len() as int,
        LifecycleGetterV1::StableRetainedReadCount => owner.stable.leases@.len() - owner.stable.free_reads@.len(),
        LifecycleGetterV1::ProducerRemainingReadSlots => producer_remaining_v1(owner),
        LifecycleGetterV1::ProducerRetainedReadCount => producer_retained_v1(owner),
        LifecycleGetterV1::ProducerTotalReadCount => producer_total_retained_v1(owner),
    }
}

spec fn lifecycle_getter_model_v1(owner: logical::ProducerReadContentsV1, getter: LifecycleGetterV1) -> int {
    match getter {
        LifecycleGetterV1::ContextGeneration => owner.stable.journal.context_generation as int,
        LifecycleGetterV1::AllocationCapacity => owner.stable.journal.allocation_capacity as int,
        LifecycleGetterV1::WriterCapacity => owner.stable.journal.writer_capacity as int,
        LifecycleGetterV1::RegistrationWatermark => owner.stable.journal.registration_watermark as int,
        LifecycleGetterV1::RemainingWriterSlots => owner.stable.journal.free@.len() as int,
        LifecycleGetterV1::ReservedWriterCount => owner.stable.journal.reserved_count as int,
        LifecycleGetterV1::RemainingAllocationSlots => owner.stable.journal.allocation_free@.len() as int,
        LifecycleGetterV1::StableRemainingReadSlots => owner.stable.free_reads@.len() as int,
        LifecycleGetterV1::StableRetainedReadCount => owner.stable.leases@.len() - owner.stable.free_reads@.len(),
        LifecycleGetterV1::ProducerRemainingReadSlots => owner.reservations@.len() - logical::producer_live_reads_v1(owner),
        LifecycleGetterV1::ProducerRetainedReadCount => owner.reservations@.len() - owner.free@.len(),
        LifecycleGetterV1::ProducerTotalReadCount => logical::producer_live_reads_v1(owner),
    }
}

spec fn lifecycle_writer_lookup_model_v1(owner: logical::JournalContentsV1, reference: logical::WriterReferenceV1)
    -> Result<ContextWriterStateV1, ReadErrorV1>
{
    if reference.slot >= owner.writers@.len() { Err(ReadErrorV1::InvalidReference) }
    else { match owner.writers@[reference.slot as int] {
        None => Err(ReadErrorV1::InvalidReference),
        Some(entry) => if !logical::same_key_v1(logical::writer_key_v1(entry), reference.key)
            || logical::writer_key_v1(entry).context_generation != owner.context_generation { Err(ReadErrorV1::InvalidReference) }
            else { Ok(match entry {
                logical::WriterEntryV1::Reserved(_) => ContextWriterStateV1::Reserved,
                logical::WriterEntryV1::Pending { count, .. } => ContextWriterStateV1::Pending { member_count: count },
                logical::WriterEntryV1::Unknown { count, .. } => ContextWriterStateV1::Unknown { member_count: count },
            }) },
    } }
}

spec fn lifecycle_query_actual_v1(owner: ContextProducerReadJournalV1, query: LifecycleQueryV1) -> LifecycleAnswerV1 {
    match query {
        LifecycleQueryV1::Getter(getter) => LifecycleAnswerV1::Scalar(lifecycle_getter_actual_v1(owner, getter)),
        LifecycleQueryV1::InspectJournal => LifecycleAnswerV1::Journal(inspection_journal_v1(owner.stable.journal)),
        LifecycleQueryV1::InspectStable { .. } => LifecycleAnswerV1::Owner(inspection_stable_v1(owner.stable)),
        LifecycleQueryV1::InspectProducer { .. } => LifecycleAnswerV1::Owner(inspection_producer_v1(owner)),
        LifecycleQueryV1::LookupReserved(reference) => LifecycleAnswerV1::Reserved(begin_reserved_decision_v1(owner.stable.journal, reference)),
        LifecycleQueryV1::LookupWriter(reference) => LifecycleAnswerV1::Writer(writer_lookup_decision_v1(owner.stable.journal, reference)),
        LifecycleQueryV1::LookupAllocation(reference) => LifecycleAnswerV1::Allocation(allocation_lookup_decision_v1(owner.stable.journal, reference)),
        LifecycleQueryV1::ReaderCount { producer, allocation } => LifecycleAnswerV1::Count(if producer {
            producer_count_decision_v1(owner, allocation) } else { stable_count_decision_v1(owner.stable, allocation) }),
        LifecycleQueryV1::StableCapacity(count) => LifecycleAnswerV1::Unit(stable_capacity_decision_v1(owner.stable, count)),
        LifecycleQueryV1::CombinedStableCapacity(count) => LifecycleAnswerV1::Unit(producer_stable_capacity_decision_v1(owner, count)),
        LifecycleQueryV1::ProducerCapacity(count) => LifecycleAnswerV1::Unit(producer_capacity_decision_v1(owner, count)),
        LifecycleQueryV1::ValidateRead(request) => LifecycleAnswerV1::Unit(stable_read_decision_v1(owner.stable.journal, request)),
        LifecycleQueryV1::ValidateProducer(request) => LifecycleAnswerV1::Unit(producer_validate_decision_v1(owner, request)),
        LifecycleQueryV1::LookupRead(reference) => LifecycleAnswerV1::StableRead(stable_lease_decision_v1(owner.stable, reference)),
        LifecycleQueryV1::LookupProducer(reference) => LifecycleAnswerV1::ProducerRead(producer_lookup_decision_v1(owner, reference)),
        LifecycleQueryV1::ProducerStatus(reference) => LifecycleAnswerV1::Status(producer_query_decision_v1(owner, reference)),
        LifecycleQueryV1::ValidateRetirement { receiver, roster, capacity } => LifecycleAnswerV1::Unit(match receiver {
            LifecycleReceiverV1::Journal => retirement_decision_v1(owner.stable.journal, roster, capacity),
            LifecycleReceiverV1::Stable => retirement_stable_decision_v1(owner.stable, roster, capacity),
            LifecycleReceiverV1::Producer => retirement_producer_decision_v1(owner, roster, capacity),
        }),
        LifecycleQueryV1::ValidateDisposal { receiver, writer, roster, writer_storage, member_storage, allocation_storage } =>
            LifecycleAnswerV1::Unit(match receiver {
                LifecycleReceiverV1::Journal => disposal_validate_decision_v1(owner.stable.journal, writer, roster, writer_storage, member_storage, allocation_storage),
                LifecycleReceiverV1::Stable => disposal_stable_validate_v1(owner.stable, writer, roster, writer_storage, member_storage, allocation_storage),
                LifecycleReceiverV1::Producer => disposal_producer_validate_v1(owner, writer, roster, writer_storage, member_storage, allocation_storage),
            }),
    }
}

spec fn lifecycle_query_model_v1(owner: logical::ProducerReadContentsV1, query: LifecycleQueryV1) -> LifecycleAnswerV1 {
    match query {
        LifecycleQueryV1::Getter(getter) => LifecycleAnswerV1::Scalar(lifecycle_getter_model_v1(owner, getter)),
        LifecycleQueryV1::InspectJournal => LifecycleAnswerV1::Journal(logical::inspection_journal_v1(owner.stable.journal)),
        LifecycleQueryV1::InspectStable { .. } => LifecycleAnswerV1::Owner(logical::inspection_stable_v1(owner.stable)),
        LifecycleQueryV1::InspectProducer { .. } => LifecycleAnswerV1::Owner(logical::inspection_producer_v1(owner)),
        LifecycleQueryV1::LookupReserved(reference) => LifecycleAnswerV1::Reserved(begin_reserved_result_from(
            logical::lookup_decision_v1(owner.stable.journal.context_generation, owner.stable.journal.writers@, writer_reference_view(reference)))),
        LifecycleQueryV1::LookupWriter(reference) => LifecycleAnswerV1::Writer(lifecycle_writer_lookup_model_v1(owner.stable.journal, writer_reference_view(reference))),
        LifecycleQueryV1::LookupAllocation(reference) => LifecycleAnswerV1::Allocation(lookup_result_from(owner.stable.journal,
            logical::allocation_decision_v1(owner.stable.journal, allocation_reference_view(reference)))),
        LifecycleQueryV1::ReaderCount { producer, allocation } => LifecycleAnswerV1::Count(begin_result_from(if producer {
            logical::producer_reader_count_decision_v1(owner, allocation_reference_view(allocation))
        } else { logical::retirement_stable_count_decision_v1(owner.stable, allocation_reference_view(allocation)) })),
        LifecycleQueryV1::StableCapacity(count) => LifecycleAnswerV1::Unit(begin_result_from(logical::capacity_decision_v1(owner.stable, count))),
        LifecycleQueryV1::CombinedStableCapacity(count) => LifecycleAnswerV1::Unit(begin_result_from(
            if count > owner.reservations@.len() - logical::producer_live_reads_v1(owner) { Err(logical::ReadErrorV1::MemberCapacity) }
            else { logical::capacity_decision_v1(owner.stable, count) })),
        LifecycleQueryV1::ProducerCapacity(count) => LifecycleAnswerV1::Unit(begin_result_from(logical::producer_capacity_decision_v1(owner, count))),
        LifecycleQueryV1::ValidateRead(request) => LifecycleAnswerV1::Unit(begin_result_from(logical::read_decision_v1(owner.stable.journal, allocation_read_view(request)))),
        LifecycleQueryV1::ValidateProducer(request) => LifecycleAnswerV1::Unit(begin_result_from(logical::producer_validate_decision_v1(owner.stable.journal, producer_read_view(request)))),
        LifecycleQueryV1::LookupRead(reference) => LifecycleAnswerV1::StableRead(stable_read_result_from(logical::lease_decision_v1(owner.stable, read_reference_view(reference)))),
        LifecycleQueryV1::LookupProducer(reference) => LifecycleAnswerV1::ProducerRead(producer_read_result_from(logical::producer_lookup_decision_v1(owner, producer_reference_view(reference)))),
        LifecycleQueryV1::ProducerStatus(reference) => LifecycleAnswerV1::Status(producer_status_result_from(historical_producer_query_decision_v1(owner, producer_reference_view(reference)))),
        LifecycleQueryV1::ValidateRetirement { receiver, roster, capacity } => LifecycleAnswerV1::Unit(begin_result_from(match receiver {
            LifecycleReceiverV1::Journal => logical::retirement_decision_v1(owner.stable.journal, retirement_roster_view_v1(roster), capacity),
            LifecycleReceiverV1::Stable => logical::retirement_stable_decision_v1(owner.stable, retirement_roster_view_v1(roster), capacity),
            LifecycleReceiverV1::Producer => logical::retirement_producer_decision_v1(owner, retirement_roster_view_v1(roster), capacity),
        })),
        LifecycleQueryV1::ValidateDisposal { receiver, writer, roster, writer_storage, member_storage, allocation_storage } =>
            LifecycleAnswerV1::Unit(begin_result_from(match receiver {
                LifecycleReceiverV1::Journal => logical::disposal_validate_decision_v1(owner.stable.journal, writer_reference_view(writer), begin_roster_view(roster), writer_storage, member_storage, allocation_storage),
                LifecycleReceiverV1::Stable => logical::disposal_stable_validate_v1(owner.stable, writer_reference_view(writer), begin_roster_view(roster), writer_storage, member_storage, allocation_storage),
                LifecycleReceiverV1::Producer => logical::disposal_producer_validate_v1(owner, writer_reference_view(writer), begin_roster_view(roster), writer_storage, member_storage, allocation_storage),
            })),
    }
}

#[verifier::spinoff_prover]
proof fn lifecycle_query_correspondence_v1(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    query: LifecycleQueryV1)
    requires producer_represents(actual, model), logical::producer_invariant_v1(model),
    ensures lifecycle_query_actual_v1(actual, query) == lifecycle_query_model_v1(model, query),
{
    match query {
        LifecycleQueryV1::Getter(getter) => {
            match getter {
                LifecycleGetterV1::ContextGeneration => {}, LifecycleGetterV1::AllocationCapacity => {},
                LifecycleGetterV1::WriterCapacity => {}, LifecycleGetterV1::RegistrationWatermark => {},
                LifecycleGetterV1::RemainingWriterSlots => {}, LifecycleGetterV1::ReservedWriterCount => {},
                LifecycleGetterV1::RemainingAllocationSlots => {}, LifecycleGetterV1::StableRemainingReadSlots => {},
                LifecycleGetterV1::StableRetainedReadCount => {}, LifecycleGetterV1::ProducerRemainingReadSlots => {},
                LifecycleGetterV1::ProducerRetainedReadCount => {}, LifecycleGetterV1::ProducerTotalReadCount => {},
            }
        },
        LifecycleQueryV1::InspectJournal => { inspection_journal_correspondence_v1(actual.stable.journal, model.stable.journal); },
        LifecycleQueryV1::InspectStable { .. } => { inspection_stable_correspondence_v1(actual.stable, model.stable); },
        LifecycleQueryV1::InspectProducer { .. } => { inspection_producer_correspondence_v1(actual, model); },
        LifecycleQueryV1::LookupReserved(reference) => { begin_reserved_correspondence(actual.stable.journal, model.stable.journal, reference); },
        LifecycleQueryV1::LookupWriter(reference) => {
            writer_key_round_trip(reference.key, writer_key_view(reference.key));
            if reference.slot < actual.stable.journal.writers@.len() {
                if let Some(entry) = actual.stable.journal.writers@[reference.slot as int] {
                    writer_entry_round_trip(entry, writer_entry_view(entry));
                    match entry {
                        WriterEntryV1::Reserved(key) | WriterEntryV1::Pending { key, .. } | WriterEntryV1::Unknown { key, .. } => {
                            writer_key_round_trip(key, writer_key_view(key));
                        },
                    }
                }
            }
        },
        LifecycleQueryV1::LookupAllocation(reference) => { allocation_lookup_correspondence(actual.stable.journal, model.stable.journal, reference); },
        LifecycleQueryV1::ReaderCount { producer, allocation } => {
            allocation_lookup_correspondence(actual.stable.journal, model.stable.journal, allocation);
            if producer { producer_count_correspondence(actual, model, allocation); }
        },
        LifecycleQueryV1::StableCapacity(count) | LifecycleQueryV1::CombinedStableCapacity(count) => {
            stable_capacity_correspondence(actual.stable, model.stable, count);
        },
        LifecycleQueryV1::ProducerCapacity(count) => { producer_capacity_correspondence(actual, model, count); },
        LifecycleQueryV1::ValidateRead(request) => { stable_read_correspondence(actual.stable.journal, model.stable.journal, request); },
        LifecycleQueryV1::ValidateProducer(request) => { producer_status_correspondence(actual.stable.journal, model.stable.journal, request); },
        LifecycleQueryV1::LookupRead(reference) => { stable_lease_correspondence(actual.stable, model.stable, reference); },
        LifecycleQueryV1::LookupProducer(reference) | LifecycleQueryV1::ProducerStatus(reference) => { producer_lookup_correspondence(actual, model, reference); },
        LifecycleQueryV1::ValidateRetirement { roster, capacity, .. } => {
            retirement_decision_correspondence_v1(actual.stable.journal, model.stable.journal, roster, capacity);
            retirement_stable_correspondence_v1(actual.stable, model.stable, roster, 0);
            retirement_producer_correspondence_v1(actual, model, roster, 0);
        },
        LifecycleQueryV1::ValidateDisposal { writer, roster, writer_storage, member_storage, allocation_storage, .. } => {
            disposal_plan_correspondence_v1(actual.stable.journal, model.stable.journal, writer, roster, writer_storage, member_storage, allocation_storage);
            disposal_owner_validate_correspondence_v1(actual, model, writer, roster, writer_storage, member_storage, allocation_storage);
        },
    }
}

}
