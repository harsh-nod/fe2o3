// Independent semantic anchors exclude coordinated permutations of inverse views.
verus! {

proof fn kind_projection_exact(value: ContextWriterKindV1)
    ensures match value {
        ContextWriterKindV1::Synchronous => kind_view(value) == logical::WriterKindV1::Synchronous,
        ContextWriterKindV1::Submission => kind_view(value) == logical::WriterKindV1::Submission,
    },
{}

proof fn writer_key_projection_exact(value: ContextWriterKeyV1)
    ensures
        writer_key_view(value).context_generation == value.context_generation,
        writer_key_view(value).local == value.local,
        writer_key_view(value).kind == kind_view(value.kind),
{}

proof fn writer_reference_projection_exact(value: ContextWriterReferenceV1)
    ensures
        writer_reference_view(value).slot == value.slot,
        writer_reference_view(value).key == writer_key_view(value.key),
{}

proof fn allocation_key_projection_exact(value: ContextAllocationKeyV1)
    ensures
        allocation_key_view(value).context_generation == value.context_generation,
        allocation_key_view(value).local == value.local,
{}

proof fn device_key_projection_exact(value: ContextJournalDeviceKeyV1)
    ensures
        device_key_view(value).context_generation == value.context_generation,
        device_key_view(value).local == value.local,
{}

proof fn allocation_reference_projection_exact(value: ContextAllocationReferenceV1)
    ensures
        allocation_reference_view(value).slot == value.slot,
        allocation_reference_view(value).key == allocation_key_view(value.key),
{}

proof fn allocation_write_projection_exact(value: ContextAllocationWriteV1)
    ensures
        allocation_write_view(value).allocation == allocation_reference_view(value.allocation),
        allocation_write_view(value).device == device_key_view(value.device),
        allocation_write_view(value).byte_extent == value.byte_extent,
{}

proof fn allocation_entry_projection_exact(value: AllocationEntryV1)
    ensures
        allocation_entry_view(value).key == allocation_key_view(value.key),
        allocation_entry_view(value).device == device_key_view(value.device),
        allocation_entry_view(value).byte_extent == value.byte_extent,
        allocation_entry_view(value).attempt_epoch == value.attempt_epoch,
        allocation_entry_view(value).content_lineage == value.content_lineage,
        allocation_entry_view(value).pending_member == value.pending_member,
{}

proof fn member_entry_projection_exact(value: MemberEntryV1)
    ensures
        member_entry_view(value).writer == writer_reference_view(value.writer),
        member_entry_view(value).allocation == allocation_reference_view(value.allocation),
        (member_entry_view(value).prior_lineage, member_entry_view(value).attempt_epoch)
            == (value.prior_lineage, value.attempt_epoch),
        member_entry_view(value).next == value.next,
{}

proof fn begin_plan_projection_exact(value: BeginMemberPlanV1)
    ensures
        begin_plan_view(value).member_slot == value.member_slot,
        begin_plan_view(value).allocation == allocation_reference_view(value.allocation),
        begin_plan_view(value).prior_lineage == value.prior_lineage,
        begin_plan_view(value).attempt_epoch == value.attempt_epoch,
{}

proof fn writer_entry_projection_exact(value: WriterEntryV1)
    ensures match value {
        WriterEntryV1::Reserved(key) => writer_entry_view(value)
            == logical::WriterEntryV1::Reserved(writer_key_view(key)),
        WriterEntryV1::Pending { key, head, count } => writer_entry_view(value)
            == logical::WriterEntryV1::Pending { key: writer_key_view(key), head, count },
        WriterEntryV1::Unknown { key, head, count } => writer_entry_view(value)
            == logical::WriterEntryV1::Unknown { key: writer_key_view(key), head, count },
    },
{}

proof fn error_variant_anchors()
    ensures
        journal_error_embed(logical::JournalErrorV1::InvalidContextGeneration) == ContextVersionJournalErrorV1::InvalidContextGeneration,
        journal_error_embed(logical::JournalErrorV1::InvalidCapacity) == ContextVersionJournalErrorV1::InvalidCapacity,
        journal_error_embed(logical::JournalErrorV1::ForeignContext) == ContextVersionJournalErrorV1::ForeignContext,
        journal_error_embed(logical::JournalErrorV1::InvalidWriterId) == ContextVersionJournalErrorV1::InvalidWriterId,
        journal_error_embed(logical::JournalErrorV1::WriterReplay) == ContextVersionJournalErrorV1::WriterReplay,
        journal_error_embed(logical::JournalErrorV1::WriterCapacity) == ContextVersionJournalErrorV1::WriterCapacity,
        (journal_error_embed(logical::JournalErrorV1::InvalidReference),
            journal_error_embed(logical::JournalErrorV1::InvalidState))
            == (ContextVersionJournalErrorV1::InvalidReference, ContextVersionJournalErrorV1::InvalidState),
        read_error_embed(logical::ReadErrorV1::InvalidContextGeneration) == ContextVersionJournalErrorV1::InvalidContextGeneration,
        read_error_embed(logical::ReadErrorV1::InvalidCapacity) == ContextVersionJournalErrorV1::InvalidCapacity,
        read_error_embed(logical::ReadErrorV1::ForeignContext) == ContextVersionJournalErrorV1::ForeignContext,
        read_error_embed(logical::ReadErrorV1::InvalidWriterId) == ContextVersionJournalErrorV1::InvalidWriterId,
        read_error_embed(logical::ReadErrorV1::WriterReplay) == ContextVersionJournalErrorV1::WriterReplay,
        read_error_embed(logical::ReadErrorV1::WriterCapacity) == ContextVersionJournalErrorV1::WriterCapacity,
        read_error_embed(logical::ReadErrorV1::InvalidReference) == ContextVersionJournalErrorV1::InvalidReference,
        read_error_embed(logical::ReadErrorV1::InvalidState) == ContextVersionJournalErrorV1::InvalidState,
        read_error_embed(logical::ReadErrorV1::InvalidAllocationReference) == ContextVersionJournalErrorV1::InvalidAllocationReference,
        read_error_embed(logical::ReadErrorV1::AllocationDeviceMismatch) == ContextVersionJournalErrorV1::AllocationDeviceMismatch,
        read_error_embed(logical::ReadErrorV1::AllocationExtentMismatch) == ContextVersionJournalErrorV1::AllocationExtentMismatch,
        read_error_embed(logical::ReadErrorV1::InvalidExtent) == ContextVersionJournalErrorV1::InvalidExtent,
        read_error_embed(logical::ReadErrorV1::AllocationBusy) == ContextVersionJournalErrorV1::AllocationBusy,
        read_error_embed(logical::ReadErrorV1::RosterCapacity) == ContextVersionJournalErrorV1::RosterCapacity,
        read_error_embed(logical::ReadErrorV1::MemberCapacity) == ContextVersionJournalErrorV1::MemberCapacity,
        read_error_embed(logical::ReadErrorV1::EpochExhausted) == ContextVersionJournalErrorV1::EpochExhausted,
        read_error_embed(logical::ReadErrorV1::NonCanonicalRoster) == ContextVersionJournalErrorV1::NonCanonicalRoster,
        read_error_embed(logical::ReadErrorV1::SettlementEvidenceMismatch) == ContextVersionJournalErrorV1::SettlementEvidenceMismatch,
        enrollment_error_embed(logical::EnrollmentErrorV1::RosterCapacity) == ContextVersionJournalErrorV1::RosterCapacity,
        enrollment_error_embed(logical::EnrollmentErrorV1::InvalidState) == ContextVersionJournalErrorV1::InvalidState,
        enrollment_error_embed(logical::EnrollmentErrorV1::ForeignContext) == ContextVersionJournalErrorV1::ForeignContext,
        enrollment_error_embed(logical::EnrollmentErrorV1::InvalidAllocationId) == ContextVersionJournalErrorV1::InvalidAllocationId,
        enrollment_error_embed(logical::EnrollmentErrorV1::InvalidDeviceId) == ContextVersionJournalErrorV1::InvalidDeviceId,
        enrollment_error_embed(logical::EnrollmentErrorV1::InvalidExtent) == ContextVersionJournalErrorV1::InvalidExtent,
        enrollment_error_embed(logical::EnrollmentErrorV1::NonCanonicalRoster) == ContextVersionJournalErrorV1::NonCanonicalRoster,
        enrollment_error_embed(logical::EnrollmentErrorV1::AllocationReplay) == ContextVersionJournalErrorV1::AllocationReplay,
        enrollment_error_embed(logical::EnrollmentErrorV1::AllocationCapacity) == ContextVersionJournalErrorV1::AllocationCapacity,
{}

}
