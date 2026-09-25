use super::ContextVersionJournalErrorV1;

include!("settlement_return_body.rs");

macro_rules! settlement_storage_declarations_v1 {
    ($($items:tt)*) => { $($items)* };
}
include!("settlement_storage_declarations.rs");

impl SettlementReturnStorageV1 {
    #[inline]
    pub(super) fn check(&self, count: usize) -> Result<(), ContextVersionJournalErrorV1> {
        settlement_return_admission_body!(self, count, ContextVersionJournalErrorV1::InvalidState)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};

    #[test]
    fn settlement_proof_uses_actual_declarations_and_shared_bodies() {
        let adapter = include_str!("settlement_storage.rs");
        let declarations = include_str!("settlement_storage_declarations.rs");
        let root = include_str!("../../verus/context_journal_settlement_execution_v1.rs");
        let bodies = include_str!("../../verus/context_journal_settlement_bodies_v1.rs");
        assert!(adapter.contains("include!(\"settlement_storage_declarations.rs\")"));
        assert!(root.contains(
            "include!(\"../src/context_version_journal/settlement_storage_declarations.rs\")"
        ));
        assert!(root.contains("include!(\"../src/context_version_journal/declarations.rs\")"));
        assert_eq!(declarations.matches(": usize,").count(), 7);
        for name in [
            "writer_free_len",
            "member_free_len",
            "writer_limit",
            "writer_storage",
            "member_limit",
            "member_storage",
            "scratch_len",
        ] {
            assert!(declarations.contains(&alloc::format!("pub(super) {name}: usize,")));
        }
        for name in [
            "settlement_return_admission_body",
            "settlement_scratch_scan_body",
            "settlement_scratch_stage_body",
            "settlement_commit_body",
        ] {
            assert_eq!(bodies.matches(&alloc::format!("{name}!")).count(), 1);
        }
        assert!(bodies.contains("journal: &mut ContextVersionJournalV1"));
        assert!(bodies.contains("storage: &SettlementReturnStorageV1"));
        for text in [root, bodies] {
            assert!(!text.contains("struct ContextVersionJournalV1"));
            assert!(!text.contains("struct SettlementReturnStorageV1"));
        }
    }

    #[test]
    fn nonadjacent_aliases_preserve_staging_order_and_current_lineage() {
        use crate::context_version_journal::*;
        for success in [false, true] {
            let key = ContextWriterKeyV1 {
                context_generation: 0,
                local: u64::MAX,
                kind: ContextWriterKindV1::Submission,
            };
            let writer = ContextWriterReferenceV1 { slot: 0, key };
            let references = [0, 1].map(|slot| ContextAllocationReferenceV1 {
                slot,
                key: ContextAllocationKeyV1 {
                    context_generation: u64::MAX,
                    local: slot as u64,
                },
            });
            let allocations = references.map(|reference| {
                Some(AllocationEntryV1 {
                    key: reference.key,
                    device: ContextJournalDeviceKeyV1 {
                        context_generation: 0,
                        local: 0,
                    },
                    byte_extent: 0,
                    attempt_epoch: u64::MAX,
                    content_lineage: 101 + reference.slot as u64,
                    pending_member: Some(usize::MAX),
                })
            });
            let slots = [2, 0, 1];
            let destinations = [references[0], references[1], references[0]];
            let epochs = [19, 23, 31];
            let mut members = vec![None; 3];
            for (index, &slot) in slots.iter().enumerate() {
                members[slot] = Some(MemberEntryV1 {
                    writer,
                    allocation: destinations[index],
                    prior_lineage: 701 + index as u64,
                    attempt_epoch: epochs[index],
                    next: slots.get(index + 1).copied(),
                });
            }
            let tail = BeginMemberPlanV1 {
                member_slot: usize::MAX,
                allocation: references[1],
                prior_lineage: u64::MAX,
                attempt_epoch: 0,
            };
            let mut journal = ContextVersionJournalV1 {
                context_generation: 0,
                allocation_capacity: 0,
                writer_capacity: 0,
                registration_watermark: u64::MAX,
                reserved_count: usize::MAX,
                writers: vec![Some(WriterEntryV1::Reserved(key))],
                free: vec![usize::MAX],
                allocations: allocations.to_vec(),
                allocation_free: vec![usize::MAX],
                members,
                member_free: vec![usize::MAX],
                scratch: vec![None, None, None, Some(tail)],
                indexed_accesses: core::cell::Cell::new(0),
            };
            journal.free.reserve(1);
            journal.member_free.reserve(3);
            let storage = (
                journal.free.as_ptr(),
                journal.free.capacity(),
                journal.member_free.as_ptr(),
                journal.member_free.capacity(),
            );
            settlement_scratch::shared_settlement_scratch_stage_v1(&mut journal, Some(slots[0]), 3);
            assert_eq!(journal.indexed_accesses.get(), 6);
            for (index, &slot) in slots.iter().enumerate() {
                assert_eq!(
                    journal.scratch[index],
                    Some(BeginMemberPlanV1 {
                        member_slot: slot,
                        allocation: destinations[index],
                        prior_lineage: 701 + index as u64,
                        attempt_epoch: epochs[index],
                    })
                );
            }
            journal.indexed_accesses.set(0);
            settlement_commit::shared_settlement_commit_v1(&mut journal, writer, 3, success);
            assert_eq!(journal.indexed_accesses.get(), 14);
            for (index, entry) in allocations.into_iter().enumerate() {
                assert_eq!(
                    journal.allocations[index],
                    Some(AllocationEntryV1 {
                        content_lineage: if success {
                            [31, 23][index]
                        } else {
                            101 + index as u64
                        },
                        pending_member: None,
                        ..entry.unwrap()
                    })
                );
            }
            assert_eq!(journal.members, vec![None; 3]);
            assert_eq!(journal.member_free, vec![usize::MAX, 2, 0, 1]);
            assert_eq!(journal.writers, vec![None]);
            assert_eq!(journal.free, vec![usize::MAX, 0]);
            assert_eq!(journal.scratch, vec![None, None, None, Some(tail)]);
            assert_eq!(journal.allocation_free, vec![usize::MAX]);
            assert_eq!(
                (
                    journal.context_generation,
                    journal.allocation_capacity,
                    journal.writer_capacity,
                    journal.registration_watermark,
                    journal.reserved_count
                ),
                (0, 0, 0, u64::MAX, usize::MAX)
            );
            assert_eq!(
                storage,
                (
                    journal.free.as_ptr(),
                    journal.free.capacity(),
                    journal.member_free.as_ptr(),
                    journal.member_free.capacity()
                )
            );
        }
    }

    fn choices(required: u128) -> Vec<usize> {
        let mut values = vec![0, usize::MAX];
        for value in [required.saturating_sub(1), required, required + 1] {
            if let Ok(value) = usize::try_from(value) {
                values.push(value);
            }
        }
        values.sort_unstable();
        values.dedup();
        values
    }

    #[test]
    fn scalar_return_admission_matches_widened_boundary_oracle() {
        let mut accepted = 0;
        let mut rejected = 0;
        for (writer_free_len, member_free_len, count) in [
            (0, 0, 0),
            (0, 0, 1),
            (1, 2, 3),
            (usize::MAX - 1, 0, 0),
            (usize::MAX, 0, 0),
            (0, usize::MAX, 0),
            (0, usize::MAX, 1),
            (0, usize::MAX - 1, 1),
            (0, usize::MAX - 1, 2),
            (0, 0, usize::MAX),
            (usize::MAX, usize::MAX, usize::MAX),
        ] {
            let writer_returns = writer_free_len as u128 + 1;
            let member_returns = member_free_len as u128 + count as u128;
            for writer_limit in choices(writer_returns) {
                for writer_storage in choices(writer_returns) {
                    for member_limit in choices(member_returns) {
                        for member_storage in choices(member_returns) {
                            for scratch_len in choices(count as u128) {
                                let observed = SettlementReturnStorageV1 {
                                    writer_free_len,
                                    member_free_len,
                                    writer_limit,
                                    writer_storage,
                                    member_limit,
                                    member_storage,
                                    scratch_len,
                                };
                                let allowed = writer_returns <= usize::MAX as u128
                                    && member_returns <= usize::MAX as u128
                                    && writer_returns <= writer_limit as u128
                                    && writer_returns <= writer_storage as u128
                                    && member_returns <= member_limit as u128
                                    && member_returns <= member_storage as u128
                                    && count <= scratch_len;
                                let expected = if allowed {
                                    accepted += 1;
                                    Ok(())
                                } else {
                                    rejected += 1;
                                    Err(ContextVersionJournalErrorV1::InvalidState)
                                };
                                assert_eq!(observed.check(count), expected);
                            }
                        }
                    }
                }
            }
        }
        assert_eq!((accepted, rejected), (819, 6177));
    }
}
