// Frozen batch body from 87fa1da182ee2873a33a09f55f431ba474655f5b.
// Only the receiver/name/indentation change; this is a benchmark control, not an oracle.
use super::*;

pub(super) fn enroll_allocations(
    journal: &mut ContextVersionJournalV1,
    canonical: &[ContextAllocationEnrollmentV1],
    output: &mut [Option<ContextAllocationReferenceV1>],
) -> Result<(), ContextVersionJournalErrorV1> {
    use ContextVersionJournalErrorV1 as E;
    if canonical.len() > journal.allocation_capacity || output.len() != canonical.len() {
        return Err(E::RosterCapacity);
    }
    if output.iter().any(Option::is_some) {
        return Err(E::InvalidState);
    }
    let mut previous = None;
    for entry in canonical {
        if entry.key.context_generation != journal.context_generation
            || entry.device.context_generation != journal.context_generation
        {
            return Err(E::ForeignContext);
        }
        if !issuable_context_id(entry.key.local) {
            return Err(E::InvalidAllocationId);
        }
        if !issuable_context_id(entry.device.local) {
            return Err(E::InvalidDeviceId);
        }
        if entry.byte_extent == 0 {
            return Err(E::InvalidExtent);
        }
        if previous.is_some_and(|key| key >= entry.key) {
            return Err(E::NonCanonicalRoster);
        }
        previous = Some(entry.key);
    }
    if canonical.is_empty() {
        return Ok(());
    }
    if journal.allocation_free.len() > journal.allocation_capacity {
        return Err(E::InvalidState);
    }
    for index in 0..journal.allocations.len() {
        if journal.read_allocation(index).is_some_and(|entry| {
            canonical
                .binary_search_by_key(&entry.key, |new| new.key)
                .is_ok()
        }) {
            return Err(E::AllocationReplay);
        }
    }
    let remaining = journal
        .allocation_free
        .len()
        .checked_sub(canonical.len())
        .ok_or(E::AllocationCapacity)?;
    for &slot in &journal.allocation_free[remaining..] {
        if journal.allocations.get(slot) != Some(&None) {
            return Err(E::InvalidState);
        }
    }
    // Use the caller's inert output as temporary sort storage. On a corrupt
    // free stack, restore its original all-None contents before returning.
    for ((entry, out), &slot) in canonical
        .iter()
        .zip(output.iter_mut())
        .zip(journal.allocation_free[remaining..].iter().rev())
    {
        *out = Some(ContextAllocationReferenceV1 {
            slot,
            key: entry.key,
        });
    }
    output.sort_unstable_by_key(|reference| reference.unwrap().slot);
    if output
        .windows(2)
        .any(|pair| pair[0].unwrap().slot == pair[1].unwrap().slot)
        || journal.allocation_free[..remaining].iter().any(|slot| {
            output
                .binary_search_by_key(slot, |reference| reference.unwrap().slot)
                .is_ok()
        })
    {
        output.fill(None);
        return Err(E::InvalidState);
    }
    // Restore canonical key/slot associations from the unchanged free stack
    // in linear time, rather than sorting the temporary output a second time.
    for ((entry, out), &slot) in canonical
        .iter()
        .zip(output.iter_mut())
        .zip(journal.allocation_free[remaining..].iter().rev())
    {
        *out = Some(ContextAllocationReferenceV1 {
            slot,
            key: entry.key,
        });
    }
    // All fallible validation precedes the journal commit.
    for (entry, reference) in canonical.iter().zip(output.iter()) {
        journal.allocations[reference.unwrap().slot] = Some(AllocationEntryV1 {
            key: entry.key,
            device: entry.device,
            byte_extent: entry.byte_extent,
            attempt_epoch: 0,
            content_lineage: 0,
            pending_member: None,
        });
    }
    journal.allocation_free.truncate(remaining);
    Ok(())
}
