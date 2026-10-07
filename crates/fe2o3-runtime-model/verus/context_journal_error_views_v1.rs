// Exhaustive error embeddings; fallible allocation is not collapsed to a model error.
verus! {

spec fn journal_error_embed(value: logical::JournalErrorV1) -> ContextVersionJournalErrorV1 {
    match value {
        logical::JournalErrorV1::InvalidContextGeneration => ContextVersionJournalErrorV1::InvalidContextGeneration,
        logical::JournalErrorV1::InvalidCapacity => ContextVersionJournalErrorV1::InvalidCapacity,
        logical::JournalErrorV1::ForeignContext => ContextVersionJournalErrorV1::ForeignContext,
        logical::JournalErrorV1::InvalidWriterId => ContextVersionJournalErrorV1::InvalidWriterId,
        logical::JournalErrorV1::WriterReplay => ContextVersionJournalErrorV1::WriterReplay,
        logical::JournalErrorV1::WriterCapacity => ContextVersionJournalErrorV1::WriterCapacity,
        logical::JournalErrorV1::InvalidReference => ContextVersionJournalErrorV1::InvalidReference,
        logical::JournalErrorV1::InvalidState => ContextVersionJournalErrorV1::InvalidState,
    }
}

spec fn journal_error_project(value: ContextVersionJournalErrorV1) -> Option<logical::JournalErrorV1> {
    match value {
        ContextVersionJournalErrorV1::InvalidContextGeneration => Some(logical::JournalErrorV1::InvalidContextGeneration),
        ContextVersionJournalErrorV1::InvalidCapacity => Some(logical::JournalErrorV1::InvalidCapacity),
        ContextVersionJournalErrorV1::StorageAllocationFailed => None,
        ContextVersionJournalErrorV1::ForeignContext => Some(logical::JournalErrorV1::ForeignContext),
        ContextVersionJournalErrorV1::InvalidWriterId => Some(logical::JournalErrorV1::InvalidWriterId),
        ContextVersionJournalErrorV1::WriterReplay => Some(logical::JournalErrorV1::WriterReplay),
        ContextVersionJournalErrorV1::WriterCapacity => Some(logical::JournalErrorV1::WriterCapacity),
        ContextVersionJournalErrorV1::InvalidReference => Some(logical::JournalErrorV1::InvalidReference),
        ContextVersionJournalErrorV1::InvalidState => Some(logical::JournalErrorV1::InvalidState),
        ContextVersionJournalErrorV1::InvalidAllocationId => None,
        ContextVersionJournalErrorV1::InvalidDeviceId => None,
        ContextVersionJournalErrorV1::InvalidExtent => None,
        ContextVersionJournalErrorV1::AllocationReplay => None,
        ContextVersionJournalErrorV1::AllocationCapacity => None,
        ContextVersionJournalErrorV1::InvalidAllocationReference => None,
        ContextVersionJournalErrorV1::AllocationDeviceMismatch => None,
        ContextVersionJournalErrorV1::AllocationExtentMismatch => None,
        ContextVersionJournalErrorV1::AllocationBusy => None,
        ContextVersionJournalErrorV1::RosterCapacity => None,
        ContextVersionJournalErrorV1::NonCanonicalRoster => None,
        ContextVersionJournalErrorV1::MemberCapacity => None,
        ContextVersionJournalErrorV1::EpochExhausted => None,
        ContextVersionJournalErrorV1::SettlementEvidenceMismatch => None,
    }
}

proof fn journal_error_round_trip(value: ContextVersionJournalErrorV1, model: logical::JournalErrorV1)
    ensures journal_error_project(journal_error_embed(model)) == Some(model),
        match journal_error_project(value) {
            Some(projected) => journal_error_embed(projected) == value,
            None => forall|other: logical::JournalErrorV1| #[trigger] journal_error_embed(other) != value,
        },
{}

spec fn read_error_embed(value: logical::ReadErrorV1) -> ContextVersionJournalErrorV1 {
    match value {
        logical::ReadErrorV1::InvalidContextGeneration => ContextVersionJournalErrorV1::InvalidContextGeneration,
        logical::ReadErrorV1::InvalidCapacity => ContextVersionJournalErrorV1::InvalidCapacity,
        logical::ReadErrorV1::ForeignContext => ContextVersionJournalErrorV1::ForeignContext,
        logical::ReadErrorV1::InvalidWriterId => ContextVersionJournalErrorV1::InvalidWriterId,
        logical::ReadErrorV1::WriterReplay => ContextVersionJournalErrorV1::WriterReplay,
        logical::ReadErrorV1::WriterCapacity => ContextVersionJournalErrorV1::WriterCapacity,
        logical::ReadErrorV1::InvalidReference => ContextVersionJournalErrorV1::InvalidReference,
        logical::ReadErrorV1::InvalidState => ContextVersionJournalErrorV1::InvalidState,
        logical::ReadErrorV1::InvalidAllocationReference => ContextVersionJournalErrorV1::InvalidAllocationReference,
        logical::ReadErrorV1::AllocationDeviceMismatch => ContextVersionJournalErrorV1::AllocationDeviceMismatch,
        logical::ReadErrorV1::AllocationExtentMismatch => ContextVersionJournalErrorV1::AllocationExtentMismatch,
        logical::ReadErrorV1::InvalidExtent => ContextVersionJournalErrorV1::InvalidExtent,
        logical::ReadErrorV1::AllocationBusy => ContextVersionJournalErrorV1::AllocationBusy,
        logical::ReadErrorV1::RosterCapacity => ContextVersionJournalErrorV1::RosterCapacity,
        logical::ReadErrorV1::MemberCapacity => ContextVersionJournalErrorV1::MemberCapacity,
        logical::ReadErrorV1::EpochExhausted => ContextVersionJournalErrorV1::EpochExhausted,
        logical::ReadErrorV1::NonCanonicalRoster => ContextVersionJournalErrorV1::NonCanonicalRoster,
        logical::ReadErrorV1::SettlementEvidenceMismatch => ContextVersionJournalErrorV1::SettlementEvidenceMismatch,
    }
}

spec fn read_error_project(value: ContextVersionJournalErrorV1) -> Option<logical::ReadErrorV1> {
    match value {
        ContextVersionJournalErrorV1::InvalidContextGeneration => Some(logical::ReadErrorV1::InvalidContextGeneration),
        ContextVersionJournalErrorV1::InvalidCapacity => Some(logical::ReadErrorV1::InvalidCapacity),
        ContextVersionJournalErrorV1::StorageAllocationFailed => None,
        ContextVersionJournalErrorV1::ForeignContext => Some(logical::ReadErrorV1::ForeignContext),
        ContextVersionJournalErrorV1::InvalidWriterId => Some(logical::ReadErrorV1::InvalidWriterId),
        ContextVersionJournalErrorV1::WriterReplay => Some(logical::ReadErrorV1::WriterReplay),
        ContextVersionJournalErrorV1::WriterCapacity => Some(logical::ReadErrorV1::WriterCapacity),
        ContextVersionJournalErrorV1::InvalidReference => Some(logical::ReadErrorV1::InvalidReference),
        ContextVersionJournalErrorV1::InvalidState => Some(logical::ReadErrorV1::InvalidState),
        ContextVersionJournalErrorV1::InvalidAllocationId => None,
        ContextVersionJournalErrorV1::InvalidDeviceId => None,
        ContextVersionJournalErrorV1::InvalidExtent => Some(logical::ReadErrorV1::InvalidExtent),
        ContextVersionJournalErrorV1::AllocationReplay => None,
        ContextVersionJournalErrorV1::AllocationCapacity => None,
        ContextVersionJournalErrorV1::InvalidAllocationReference => Some(logical::ReadErrorV1::InvalidAllocationReference),
        ContextVersionJournalErrorV1::AllocationDeviceMismatch => Some(logical::ReadErrorV1::AllocationDeviceMismatch),
        ContextVersionJournalErrorV1::AllocationExtentMismatch => Some(logical::ReadErrorV1::AllocationExtentMismatch),
        ContextVersionJournalErrorV1::AllocationBusy => Some(logical::ReadErrorV1::AllocationBusy),
        ContextVersionJournalErrorV1::RosterCapacity => Some(logical::ReadErrorV1::RosterCapacity),
        ContextVersionJournalErrorV1::NonCanonicalRoster => Some(logical::ReadErrorV1::NonCanonicalRoster),
        ContextVersionJournalErrorV1::MemberCapacity => Some(logical::ReadErrorV1::MemberCapacity),
        ContextVersionJournalErrorV1::EpochExhausted => Some(logical::ReadErrorV1::EpochExhausted),
        ContextVersionJournalErrorV1::SettlementEvidenceMismatch => Some(logical::ReadErrorV1::SettlementEvidenceMismatch),
    }
}

proof fn read_error_round_trip(value: ContextVersionJournalErrorV1, model: logical::ReadErrorV1)
    ensures read_error_project(read_error_embed(model)) == Some(model),
        match read_error_project(value) {
            Some(projected) => read_error_embed(projected) == value,
            None => forall|other: logical::ReadErrorV1| #[trigger] read_error_embed(other) != value,
        },
{}

spec fn enrollment_error_embed(value: logical::EnrollmentErrorV1) -> ContextVersionJournalErrorV1 {
    match value {
        logical::EnrollmentErrorV1::RosterCapacity => ContextVersionJournalErrorV1::RosterCapacity,
        logical::EnrollmentErrorV1::InvalidState => ContextVersionJournalErrorV1::InvalidState,
        logical::EnrollmentErrorV1::ForeignContext => ContextVersionJournalErrorV1::ForeignContext,
        logical::EnrollmentErrorV1::InvalidAllocationId => ContextVersionJournalErrorV1::InvalidAllocationId,
        logical::EnrollmentErrorV1::InvalidDeviceId => ContextVersionJournalErrorV1::InvalidDeviceId,
        logical::EnrollmentErrorV1::InvalidExtent => ContextVersionJournalErrorV1::InvalidExtent,
        logical::EnrollmentErrorV1::NonCanonicalRoster => ContextVersionJournalErrorV1::NonCanonicalRoster,
        logical::EnrollmentErrorV1::AllocationReplay => ContextVersionJournalErrorV1::AllocationReplay,
        logical::EnrollmentErrorV1::AllocationCapacity => ContextVersionJournalErrorV1::AllocationCapacity,
    }
}

spec fn enrollment_error_project(value: ContextVersionJournalErrorV1) -> Option<logical::EnrollmentErrorV1> {
    match value {
        ContextVersionJournalErrorV1::InvalidContextGeneration => None,
        ContextVersionJournalErrorV1::InvalidCapacity => None,
        ContextVersionJournalErrorV1::StorageAllocationFailed => None,
        ContextVersionJournalErrorV1::ForeignContext => Some(logical::EnrollmentErrorV1::ForeignContext),
        ContextVersionJournalErrorV1::InvalidWriterId => None,
        ContextVersionJournalErrorV1::WriterReplay => None,
        ContextVersionJournalErrorV1::WriterCapacity => None,
        ContextVersionJournalErrorV1::InvalidReference => None,
        ContextVersionJournalErrorV1::InvalidState => Some(logical::EnrollmentErrorV1::InvalidState),
        ContextVersionJournalErrorV1::InvalidAllocationId => Some(logical::EnrollmentErrorV1::InvalidAllocationId),
        ContextVersionJournalErrorV1::InvalidDeviceId => Some(logical::EnrollmentErrorV1::InvalidDeviceId),
        ContextVersionJournalErrorV1::InvalidExtent => Some(logical::EnrollmentErrorV1::InvalidExtent),
        ContextVersionJournalErrorV1::AllocationReplay => Some(logical::EnrollmentErrorV1::AllocationReplay),
        ContextVersionJournalErrorV1::AllocationCapacity => Some(logical::EnrollmentErrorV1::AllocationCapacity),
        ContextVersionJournalErrorV1::InvalidAllocationReference => None,
        ContextVersionJournalErrorV1::AllocationDeviceMismatch => None,
        ContextVersionJournalErrorV1::AllocationExtentMismatch => None,
        ContextVersionJournalErrorV1::AllocationBusy => None,
        ContextVersionJournalErrorV1::RosterCapacity => Some(logical::EnrollmentErrorV1::RosterCapacity),
        ContextVersionJournalErrorV1::NonCanonicalRoster => Some(logical::EnrollmentErrorV1::NonCanonicalRoster),
        ContextVersionJournalErrorV1::MemberCapacity => None,
        ContextVersionJournalErrorV1::EpochExhausted => None,
        ContextVersionJournalErrorV1::SettlementEvidenceMismatch => None,
    }
}

proof fn enrollment_error_round_trip(value: ContextVersionJournalErrorV1, model: logical::EnrollmentErrorV1)
    ensures enrollment_error_project(enrollment_error_embed(model)) == Some(model),
        match enrollment_error_project(value) {
            Some(projected) => enrollment_error_embed(projected) == value,
            None => forall|other: logical::EnrollmentErrorV1| #[trigger] enrollment_error_embed(other) != value,
        },
{}

proof fn error_domains_are_exact(value: ContextVersionJournalErrorV1)
    ensures
        (journal_error_project(value).is_none() && read_error_project(value).is_none()
            && enrollment_error_project(value).is_none())
            <==> value == ContextVersionJournalErrorV1::StorageAllocationFailed,
{}

proof fn journal_error_lift_commutes(model: logical::JournalErrorV1)
    ensures read_error_embed(logical::lift_error_v1(model)) == journal_error_embed(model),
{}

}
