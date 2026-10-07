// Lossless value projections. These are ghost functions, not runtime conversions.
use super as logical;

verus! {

spec fn kind_view(value: ContextWriterKindV1) -> logical::WriterKindV1 {
    match value {
        ContextWriterKindV1::Synchronous => logical::WriterKindV1::Synchronous,
        ContextWriterKindV1::Submission => logical::WriterKindV1::Submission,
    }
}

spec fn kind_from(value: logical::WriterKindV1) -> ContextWriterKindV1 {
    match value {
        logical::WriterKindV1::Synchronous => ContextWriterKindV1::Synchronous,
        logical::WriterKindV1::Submission => ContextWriterKindV1::Submission,
    }
}

proof fn kind_round_trip(value: ContextWriterKindV1, model: logical::WriterKindV1)
    ensures kind_from(kind_view(value)) == value, kind_view(kind_from(model)) == model,
{}

spec fn writer_key_view(value: ContextWriterKeyV1) -> logical::WriterKeyV1 {
    logical::WriterKeyV1 {
        context_generation: value.context_generation,
        local: value.local,
        kind: kind_view(value.kind),
    }
}

spec fn writer_key_from(value: logical::WriterKeyV1) -> ContextWriterKeyV1 {
    ContextWriterKeyV1 {
        context_generation: value.context_generation,
        local: value.local,
        kind: kind_from(value.kind),
    }
}

proof fn writer_key_round_trip(value: ContextWriterKeyV1, model: logical::WriterKeyV1)
    ensures writer_key_from(writer_key_view(value)) == value,
        writer_key_view(writer_key_from(model)) == model,
{}

spec fn writer_reference_view(value: ContextWriterReferenceV1) -> logical::WriterReferenceV1 {
    logical::WriterReferenceV1 {
        slot: value.slot,
        key: writer_key_view(value.key),
    }
}

spec fn writer_reference_from(value: logical::WriterReferenceV1) -> ContextWriterReferenceV1 {
    ContextWriterReferenceV1 {
        slot: value.slot,
        key: writer_key_from(value.key),
    }
}

proof fn writer_reference_round_trip(value: ContextWriterReferenceV1, model: logical::WriterReferenceV1)
    ensures writer_reference_from(writer_reference_view(value)) == value,
        writer_reference_view(writer_reference_from(model)) == model,
{}

spec fn allocation_key_view(value: ContextAllocationKeyV1) -> logical::AllocationKeyV1 {
    logical::AllocationKeyV1 {
        context_generation: value.context_generation,
        local: value.local,
    }
}

spec fn allocation_key_from(value: logical::AllocationKeyV1) -> ContextAllocationKeyV1 {
    ContextAllocationKeyV1 {
        context_generation: value.context_generation,
        local: value.local,
    }
}

proof fn allocation_key_round_trip(value: ContextAllocationKeyV1, model: logical::AllocationKeyV1)
    ensures allocation_key_from(allocation_key_view(value)) == value,
        allocation_key_view(allocation_key_from(model)) == model,
{}

spec fn device_key_view(value: ContextJournalDeviceKeyV1) -> logical::DeviceKeyV1 {
    logical::DeviceKeyV1 {
        context_generation: value.context_generation,
        local: value.local,
    }
}

spec fn device_key_from(value: logical::DeviceKeyV1) -> ContextJournalDeviceKeyV1 {
    ContextJournalDeviceKeyV1 {
        context_generation: value.context_generation,
        local: value.local,
    }
}

proof fn device_key_round_trip(value: ContextJournalDeviceKeyV1, model: logical::DeviceKeyV1)
    ensures device_key_from(device_key_view(value)) == value,
        device_key_view(device_key_from(model)) == model,
{}

spec fn allocation_reference_view(value: ContextAllocationReferenceV1) -> logical::AllocationReferenceV1 {
    logical::AllocationReferenceV1 {
        slot: value.slot,
        key: allocation_key_view(value.key),
    }
}

spec fn allocation_reference_from(value: logical::AllocationReferenceV1) -> ContextAllocationReferenceV1 {
    ContextAllocationReferenceV1 {
        slot: value.slot,
        key: allocation_key_from(value.key),
    }
}

proof fn allocation_reference_round_trip(value: ContextAllocationReferenceV1, model: logical::AllocationReferenceV1)
    ensures allocation_reference_from(allocation_reference_view(value)) == value,
        allocation_reference_view(allocation_reference_from(model)) == model,
{}

spec fn allocation_write_view(value: ContextAllocationWriteV1) -> logical::AllocationWriteV1 {
    logical::AllocationWriteV1 {
        allocation: allocation_reference_view(value.allocation),
        device: device_key_view(value.device),
        byte_extent: value.byte_extent,
    }
}

spec fn allocation_write_from(value: logical::AllocationWriteV1) -> ContextAllocationWriteV1 {
    ContextAllocationWriteV1 {
        allocation: allocation_reference_from(value.allocation),
        device: device_key_from(value.device),
        byte_extent: value.byte_extent,
    }
}

proof fn allocation_write_round_trip(value: ContextAllocationWriteV1, model: logical::AllocationWriteV1)
    ensures allocation_write_from(allocation_write_view(value)) == value,
        allocation_write_view(allocation_write_from(model)) == model,
{}

spec fn allocation_entry_view(value: AllocationEntryV1) -> logical::AllocationEntryV1 {
    logical::AllocationEntryV1 {
        key: allocation_key_view(value.key),
        device: device_key_view(value.device),
        byte_extent: value.byte_extent,
        attempt_epoch: value.attempt_epoch,
        content_lineage: value.content_lineage,
        pending_member: value.pending_member,
    }
}

spec fn allocation_entry_from(value: logical::AllocationEntryV1) -> AllocationEntryV1 {
    AllocationEntryV1 {
        key: allocation_key_from(value.key),
        device: device_key_from(value.device),
        byte_extent: value.byte_extent,
        attempt_epoch: value.attempt_epoch,
        content_lineage: value.content_lineage,
        pending_member: value.pending_member,
    }
}

proof fn allocation_entry_round_trip(value: AllocationEntryV1, model: logical::AllocationEntryV1)
    ensures allocation_entry_from(allocation_entry_view(value)) == value,
        allocation_entry_view(allocation_entry_from(model)) == model,
{}

spec fn member_entry_view(value: MemberEntryV1) -> logical::MemberEntryV1 {
    logical::MemberEntryV1 {
        writer: writer_reference_view(value.writer),
        allocation: allocation_reference_view(value.allocation),
        prior_lineage: value.prior_lineage,
        attempt_epoch: value.attempt_epoch,
        next: value.next,
    }
}

spec fn member_entry_from(value: logical::MemberEntryV1) -> MemberEntryV1 {
    MemberEntryV1 {
        writer: writer_reference_from(value.writer),
        allocation: allocation_reference_from(value.allocation),
        prior_lineage: value.prior_lineage,
        attempt_epoch: value.attempt_epoch,
        next: value.next,
    }
}

proof fn member_entry_round_trip(value: MemberEntryV1, model: logical::MemberEntryV1)
    ensures member_entry_from(member_entry_view(value)) == value,
        member_entry_view(member_entry_from(model)) == model,
{}

spec fn begin_plan_view(value: BeginMemberPlanV1) -> logical::BeginMemberPlanV1 {
    logical::BeginMemberPlanV1 {
        member_slot: value.member_slot,
        allocation: allocation_reference_view(value.allocation),
        prior_lineage: value.prior_lineage,
        attempt_epoch: value.attempt_epoch,
    }
}

spec fn begin_plan_from(value: logical::BeginMemberPlanV1) -> BeginMemberPlanV1 {
    BeginMemberPlanV1 {
        member_slot: value.member_slot,
        allocation: allocation_reference_from(value.allocation),
        prior_lineage: value.prior_lineage,
        attempt_epoch: value.attempt_epoch,
    }
}

proof fn begin_plan_round_trip(value: BeginMemberPlanV1, model: logical::BeginMemberPlanV1)
    ensures begin_plan_from(begin_plan_view(value)) == value,
        begin_plan_view(begin_plan_from(model)) == model,
{}

spec fn writer_entry_view(value: WriterEntryV1) -> logical::WriterEntryV1 {
    match value {
        WriterEntryV1::Reserved(key) => logical::WriterEntryV1::Reserved(writer_key_view(key)),
        WriterEntryV1::Pending { key, head, count } =>
            logical::WriterEntryV1::Pending { key: writer_key_view(key), head, count },
        WriterEntryV1::Unknown { key, head, count } =>
            logical::WriterEntryV1::Unknown { key: writer_key_view(key), head, count },
    }
}

spec fn writer_entry_from(value: logical::WriterEntryV1) -> WriterEntryV1 {
    match value {
        logical::WriterEntryV1::Reserved(key) => WriterEntryV1::Reserved(writer_key_from(key)),
        logical::WriterEntryV1::Pending { key, head, count } =>
            WriterEntryV1::Pending { key: writer_key_from(key), head, count },
        logical::WriterEntryV1::Unknown { key, head, count } =>
            WriterEntryV1::Unknown { key: writer_key_from(key), head, count },
    }
}

proof fn writer_entry_round_trip(value: WriterEntryV1, model: logical::WriterEntryV1)
    ensures writer_entry_from(writer_entry_view(value)) == value,
        writer_entry_view(writer_entry_from(model)) == model,
{}

spec fn writer_entry_slot_view(value: Option<WriterEntryV1>) -> Option<logical::WriterEntryV1> {
    match value { Some(entry) => Some(writer_entry_view(entry)), None => None }
}

spec fn writer_entry_slot_from(value: Option<logical::WriterEntryV1>) -> Option<WriterEntryV1> {
    match value { Some(entry) => Some(writer_entry_from(entry)), None => None }
}

proof fn writer_entry_slot_round_trip(value: Option<WriterEntryV1>, model: Option<logical::WriterEntryV1>)
    ensures writer_entry_slot_from(writer_entry_slot_view(value)) == value,
        writer_entry_slot_view(writer_entry_slot_from(model)) == model,
{
    if let Some(entry) = value { writer_entry_round_trip(entry, writer_entry_view(entry)); }
    if let Some(entry) = model { writer_entry_round_trip(writer_entry_from(entry), entry); }
}

spec fn allocation_entry_slot_view(value: Option<AllocationEntryV1>) -> Option<logical::AllocationEntryV1> {
    match value { Some(entry) => Some(allocation_entry_view(entry)), None => None }
}

spec fn allocation_entry_slot_from(value: Option<logical::AllocationEntryV1>) -> Option<AllocationEntryV1> {
    match value { Some(entry) => Some(allocation_entry_from(entry)), None => None }
}

proof fn allocation_entry_slot_round_trip(value: Option<AllocationEntryV1>, model: Option<logical::AllocationEntryV1>)
    ensures allocation_entry_slot_from(allocation_entry_slot_view(value)) == value,
        allocation_entry_slot_view(allocation_entry_slot_from(model)) == model,
{
    if let Some(entry) = value { allocation_entry_round_trip(entry, allocation_entry_view(entry)); }
    if let Some(entry) = model { allocation_entry_round_trip(allocation_entry_from(entry), entry); }
}

spec fn member_entry_slot_view(value: Option<MemberEntryV1>) -> Option<logical::MemberEntryV1> {
    match value { Some(entry) => Some(member_entry_view(entry)), None => None }
}

spec fn member_entry_slot_from(value: Option<logical::MemberEntryV1>) -> Option<MemberEntryV1> {
    match value { Some(entry) => Some(member_entry_from(entry)), None => None }
}

proof fn member_entry_slot_round_trip(value: Option<MemberEntryV1>, model: Option<logical::MemberEntryV1>)
    ensures member_entry_slot_from(member_entry_slot_view(value)) == value,
        member_entry_slot_view(member_entry_slot_from(model)) == model,
{
    if let Some(entry) = value { member_entry_round_trip(entry, member_entry_view(entry)); }
    if let Some(entry) = model { member_entry_round_trip(member_entry_from(entry), entry); }
}

spec fn begin_plan_slot_view(value: Option<BeginMemberPlanV1>) -> Option<logical::BeginMemberPlanV1> {
    match value { Some(entry) => Some(begin_plan_view(entry)), None => None }
}

spec fn begin_plan_slot_from(value: Option<logical::BeginMemberPlanV1>) -> Option<BeginMemberPlanV1> {
    match value { Some(entry) => Some(begin_plan_from(entry)), None => None }
}

proof fn begin_plan_slot_round_trip(value: Option<BeginMemberPlanV1>, model: Option<logical::BeginMemberPlanV1>)
    ensures begin_plan_slot_from(begin_plan_slot_view(value)) == value,
        begin_plan_slot_view(begin_plan_slot_from(model)) == model,
{
    if let Some(entry) = value { begin_plan_round_trip(entry, begin_plan_view(entry)); }
    if let Some(entry) = model { begin_plan_round_trip(begin_plan_from(entry), entry); }
}

}
