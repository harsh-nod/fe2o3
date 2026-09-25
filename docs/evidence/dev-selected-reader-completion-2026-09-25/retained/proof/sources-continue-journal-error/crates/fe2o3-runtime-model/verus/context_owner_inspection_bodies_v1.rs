verus! {

type JournalInspectionV1 = (u64, usize, usize, u64, usize, usize, usize);
type OwnerInspectionV1 = (JournalInspectionV1, usize);

spec fn inspection_journal_v1(owner: ContextVersionJournalV1) -> JournalInspectionV1 {
    (owner.context_generation, owner.allocation_capacity, owner.writer_capacity,
        owner.registration_watermark, owner.free@.len() as usize, owner.reserved_count,
        owner.allocation_free@.len() as usize)
}

spec fn inspection_stable_v1(owner: ContextReadLeasedJournalV1) -> OwnerInspectionV1 {
    (inspection_journal_v1(owner.journal), owner.free_reads@.len() as usize)
}

spec fn inspection_producer_v1(owner: ContextProducerReadJournalV1) -> OwnerInspectionV1 {
    inspection_stable_v1(owner.stable)
}

// Closed public projections permit precise trait contracts over private fields.
pub closed spec fn inspection_stable_projection_v1(owner: ContextReadLeasedJournalV1) -> ContextVersionJournalV1 {
    owner.journal
}

pub closed spec fn inspection_producer_projection_v1(owner: ContextProducerReadJournalV1) -> ContextReadLeasedJournalV1 {
    owner.stable
}

impl ContextVersionJournalV1 {
    const fn context_generation(&self) -> (result: u64)
        ensures result == self.context_generation,
    { owner_inspection_scalar_body!(self, context_generation) }

    const fn allocation_capacity(&self) -> (result: usize)
        ensures result == self.allocation_capacity,
    { owner_inspection_scalar_body!(self, allocation_capacity) }

    const fn writer_capacity(&self) -> (result: usize)
        ensures result == self.writer_capacity,
    { owner_inspection_scalar_body!(self, writer_capacity) }

    const fn registration_watermark(&self) -> (result: u64)
        ensures result == self.registration_watermark,
    { owner_inspection_scalar_body!(self, registration_watermark) }

    fn remaining_writer_slots(&self) -> (result: usize)
        ensures result == self.free@.len(),
    { owner_inspection_length_body!(self, free) }

    fn reserved_writer_count(&self) -> (result: usize)
        ensures result == self.reserved_count,
    { owner_inspection_scalar_body!(self, reserved_count) }

    fn remaining_allocation_slots(&self) -> (result: usize)
        ensures result == self.allocation_free@.len(),
    { owner_inspection_length_body!(self, allocation_free) }
}

impl ContextReadLeasedJournalV1 {
    fn remaining_read_slots(&self) -> (result: usize)
        ensures result == self.free_reads@.len(),
    { owner_inspection_length_body!(self, free_reads) }
}

impl core::ops::Deref for ContextReadLeasedJournalV1 {
    type Target = ContextVersionJournalV1;

    fn deref(&self) -> (result: &Self::Target)
        ensures *result == inspection_stable_projection_v1(*self),
    {
        proof { reveal(inspection_stable_projection_v1); }
        owner_inspection_deref_body!(self, journal)
    }
}

impl core::ops::Deref for ContextProducerReadJournalV1 {
    type Target = ContextReadLeasedJournalV1;

    fn deref(&self) -> (result: &Self::Target)
        ensures *result == inspection_producer_projection_v1(*self),
    {
        proof { reveal(inspection_producer_projection_v1); }
        owner_inspection_deref_body!(self, stable)
    }
}

fn inspection_journal_exec_v1(owner: &ContextVersionJournalV1) -> (result: JournalInspectionV1)
    ensures result == inspection_journal_v1(*owner),
{
    (owner.context_generation(), owner.allocation_capacity(), owner.writer_capacity(),
        owner.registration_watermark(), owner.remaining_writer_slots(), owner.reserved_writer_count(),
        owner.remaining_allocation_slots())
}

fn inspection_stable_explicit_exec_v1(owner: &ContextReadLeasedJournalV1) -> (result: OwnerInspectionV1)
    ensures result == inspection_stable_v1(*owner),
{
    proof { reveal(inspection_stable_projection_v1); }
    let journal = <ContextReadLeasedJournalV1 as core::ops::Deref>::deref(owner);
    (inspection_journal_exec_v1(journal), owner.remaining_read_slots())
}

fn inspection_stable_auto_exec_v1(owner: &ContextReadLeasedJournalV1) -> (result: OwnerInspectionV1)
    ensures result == inspection_stable_v1(*owner),
{
    proof { reveal(inspection_stable_projection_v1); }
    ((owner.context_generation(), owner.allocation_capacity(), owner.writer_capacity(),
        owner.registration_watermark(), owner.remaining_writer_slots(), owner.reserved_writer_count(),
        owner.remaining_allocation_slots()), owner.remaining_read_slots())
}

fn inspection_producer_explicit_exec_v1(owner: &ContextProducerReadJournalV1) -> (result: OwnerInspectionV1)
    ensures result == inspection_producer_v1(*owner),
{
    proof { reveal(inspection_producer_projection_v1); }
    let stable = <ContextProducerReadJournalV1 as core::ops::Deref>::deref(owner);
    inspection_stable_explicit_exec_v1(stable)
}

fn inspection_producer_auto_exec_v1(owner: &ContextProducerReadJournalV1) -> (result: OwnerInspectionV1)
    ensures result == inspection_producer_v1(*owner),
{
    proof { reveal(inspection_stable_projection_v1); reveal(inspection_producer_projection_v1); }
    // The producer's budget method shadows the stable owner's free-list getter.
    let stable: &ContextReadLeasedJournalV1 = owner;
    ((owner.context_generation(), owner.allocation_capacity(), owner.writer_capacity(),
        owner.registration_watermark(), owner.remaining_writer_slots(), owner.reserved_writer_count(),
        owner.remaining_allocation_slots()), stable.remaining_read_slots())
}

}
