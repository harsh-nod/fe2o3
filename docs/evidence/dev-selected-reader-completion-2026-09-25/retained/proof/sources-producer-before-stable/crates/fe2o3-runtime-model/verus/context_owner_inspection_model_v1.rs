// Independent immutable inspection of logical contents, including malformed contents.
use super::*;

verus! {

pub type JournalInspectionV1 = (u64, usize, usize, u64, usize, usize, usize);
pub type OwnerInspectionV1 = (JournalInspectionV1, usize);

pub open spec fn inspection_journal_v1(owner: JournalContentsV1) -> JournalInspectionV1 {
    (owner.context_generation, owner.allocation_capacity, owner.writer_capacity,
        owner.registration_watermark, owner.free@.len() as usize, owner.reserved_count,
        owner.allocation_free@.len() as usize)
}

pub open spec fn inspection_stable_v1(owner: ReadContentsV1) -> OwnerInspectionV1 {
    (inspection_journal_v1(owner.journal), owner.free_reads@.len() as usize)
}

pub open spec fn inspection_producer_v1(owner: ProducerReadContentsV1) -> OwnerInspectionV1 {
    inspection_stable_v1(owner.stable)
}

pub fn inspection_context_generation_model_exec_v1(owner: &JournalContentsV1) -> (result: u64)
    ensures result == owner.context_generation,
{ owner.context_generation }

pub fn inspection_allocation_capacity_model_exec_v1(owner: &JournalContentsV1) -> (result: usize)
    ensures result == owner.allocation_capacity,
{ owner.allocation_capacity }

pub fn inspection_writer_capacity_model_exec_v1(owner: &JournalContentsV1) -> (result: usize)
    ensures result == owner.writer_capacity,
{ owner.writer_capacity }

pub fn inspection_registration_watermark_model_exec_v1(owner: &JournalContentsV1) -> (result: u64)
    ensures result == owner.registration_watermark,
{ owner.registration_watermark }

pub fn inspection_remaining_writer_slots_model_exec_v1(owner: &JournalContentsV1) -> (result: usize)
    ensures result == owner.free@.len(),
{ owner.free.len() }

pub fn inspection_reserved_writer_count_model_exec_v1(owner: &JournalContentsV1) -> (result: usize)
    ensures result == owner.reserved_count,
{ owner.reserved_count }

pub fn inspection_remaining_allocation_slots_model_exec_v1(owner: &JournalContentsV1) -> (result: usize)
    ensures result == owner.allocation_free@.len(),
{ owner.allocation_free.len() }

pub fn inspection_remaining_read_slots_model_exec_v1(owner: &ReadContentsV1) -> (result: usize)
    ensures result == owner.free_reads@.len(),
{ owner.free_reads.len() }

pub fn inspection_stable_project_model_exec_v1(owner: &ReadContentsV1) -> (result: &JournalContentsV1)
    ensures *result == owner.journal,
{ &owner.journal }

pub fn inspection_producer_project_model_exec_v1(owner: &ProducerReadContentsV1) -> (result: &ReadContentsV1)
    ensures *result == owner.stable,
{ &owner.stable }

pub fn inspection_journal_model_exec_v1(owner: &JournalContentsV1) -> (result: JournalInspectionV1)
    ensures result == inspection_journal_v1(*owner),
{
    (inspection_context_generation_model_exec_v1(owner),
        inspection_allocation_capacity_model_exec_v1(owner),
        inspection_writer_capacity_model_exec_v1(owner),
        inspection_registration_watermark_model_exec_v1(owner),
        inspection_remaining_writer_slots_model_exec_v1(owner),
        inspection_reserved_writer_count_model_exec_v1(owner),
        inspection_remaining_allocation_slots_model_exec_v1(owner))
}

pub fn inspection_stable_model_exec_v1(owner: &ReadContentsV1) -> (result: OwnerInspectionV1)
    ensures result == inspection_stable_v1(*owner),
{
    let journal = inspection_stable_project_model_exec_v1(owner);
    (inspection_journal_model_exec_v1(journal), inspection_remaining_read_slots_model_exec_v1(owner))
}

pub fn inspection_producer_model_exec_v1(owner: &ProducerReadContentsV1) -> (result: OwnerInspectionV1)
    ensures result == inspection_producer_v1(*owner),
{
    let stable = inspection_producer_project_model_exec_v1(owner);
    inspection_stable_model_exec_v1(stable)
}

}
