//! Single-account bounded roster arithmetic and owner-generation preflight.
//!
//! The caller supplies account facts, not native allocation costs or disposal
//! evidence. Arena extraction, mutex linearization, actual token issuance and
//! parent/global budget composition remain adapter obligations.

use crate::{R67ResourceVectorV1, r67_resource_reserve_v1};

pub const R70_MAX_RESOURCE_BATCH_MEMBERS_V1: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R70ResourceBatchErrorV1 {
    InvalidMemberCount,
    Capacity,
    RecordCapacity,
    GenerationExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R70ResourceBatchAdmissionV1 {
    pub next_used: R67ResourceVectorV1,
    pub next_owner: u64,
}

/// Computes the complete post-admission vector without modifying the input state.
pub fn r70_resource_batch_reserve_v1(
    used: R67ResourceVectorV1,
    charges: &[R67ResourceVectorV1],
    capacity: R67ResourceVectorV1,
    free_records: usize,
    next_owner: u64,
) -> Result<R70ResourceBatchAdmissionV1, R70ResourceBatchErrorV1> {
    if charges.is_empty() || charges.len() > R70_MAX_RESOURCE_BATCH_MEMBERS_V1 {
        return Err(R70ResourceBatchErrorV1::InvalidMemberCount);
    }
    if charges.len() > free_records {
        return Err(R70ResourceBatchErrorV1::RecordCapacity);
    }
    if next_owner == 0 {
        return Err(R70ResourceBatchErrorV1::GenerationExhausted);
    }
    let owner_end = next_owner
        .checked_add(charges.len() as u64)
        .ok_or(R70ResourceBatchErrorV1::GenerationExhausted)?;
    let mut next_used = used;
    for charge in charges {
        next_used = r67_resource_reserve_v1(next_used, *charge, capacity)
            .ok_or(R70ResourceBatchErrorV1::Capacity)?;
    }
    Ok(R70ResourceBatchAdmissionV1 {
        next_used,
        next_owner: owner_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{R67ResourceKindV1 as Kind, r67_resource_release_v1};
    use alloc::vec;

    fn bytes(value: u64) -> R67ResourceVectorV1 {
        R67ResourceVectorV1::ZERO.with(Kind::ResidentDeviceAllocationBytes, value)
    }

    #[test]
    fn complete_roster_preserves_every_member_and_owner_interval() {
        let initial = bytes(3);
        let charges = [bytes(2), bytes(5), bytes(7)];
        let next = r70_resource_batch_reserve_v1(initial, &charges, bytes(17), 3, 9).unwrap();
        assert_eq!(next.next_used, bytes(17));
        assert_eq!(next.next_owner, 12);
        let mut released = next.next_used;
        for charge in [charges[1], charges[0], charges[2]] {
            released = r67_resource_release_v1(released, charge).unwrap();
        }
        assert_eq!(released, initial);
    }

    #[test]
    fn final_member_and_final_dimension_are_not_omitted() {
        let charges = [bytes(2), bytes(5), bytes(7)];
        assert_eq!(
            r70_resource_batch_reserve_v1(bytes(3), &charges, bytes(16), 3, 1),
            Err(R70ResourceBatchErrorV1::Capacity)
        );
        let final_coordinate = R67ResourceVectorV1::ZERO.with(Kind::AllocationRecords, 1);
        assert_eq!(
            r70_resource_batch_reserve_v1(
                R67ResourceVectorV1::ZERO,
                &[bytes(2), final_coordinate],
                bytes(2),
                2,
                1
            ),
            Err(R70ResourceBatchErrorV1::Capacity)
        );
    }

    #[test]
    fn individually_valid_members_cannot_wrap_the_aggregate() {
        assert_eq!(
            r70_resource_batch_reserve_v1(
                bytes(0),
                &[bytes(u64::MAX), bytes(1)],
                bytes(u64::MAX),
                2,
                1
            ),
            Err(R70ResourceBatchErrorV1::Capacity)
        );
    }

    #[test]
    fn complete_owner_interval_and_record_roster_must_fit() {
        let charges = [bytes(1), bytes(1)];
        for (records, owner, error) in [
            (1, 1, R70ResourceBatchErrorV1::RecordCapacity),
            (2, 0, R70ResourceBatchErrorV1::GenerationExhausted),
            (
                2,
                u64::MAX - 1,
                R70ResourceBatchErrorV1::GenerationExhausted,
            ),
        ] {
            assert_eq!(
                r70_resource_batch_reserve_v1(bytes(0), &charges, bytes(2), records, owner),
                Err(error)
            );
        }
        let admitted =
            r70_resource_batch_reserve_v1(bytes(0), &charges, bytes(2), 2, u64::MAX - 2).unwrap();
        assert_eq!(admitted.next_owner, u64::MAX);
    }

    #[test]
    fn member_count_has_explicit_empty_and_upper_bound_rejections() {
        assert_eq!(
            r70_resource_batch_reserve_v1(bytes(0), &[], bytes(0), 0, 1),
            Err(R70ResourceBatchErrorV1::InvalidMemberCount)
        );
        let charges = vec![bytes(0); R70_MAX_RESOURCE_BATCH_MEMBERS_V1 + 1];
        assert_eq!(
            r70_resource_batch_reserve_v1(bytes(0), &charges, bytes(0), charges.len(), 1),
            Err(R70ResourceBatchErrorV1::InvalidMemberCount)
        );
    }

    #[test]
    fn aggregate_boundary_product_matches_wider_integer_oracle() {
        let values = [0, 1, u64::MAX / 2, u64::MAX - 1, u64::MAX];
        for initial in values {
            for first in values {
                for second in values {
                    for capacity in values {
                        let sum = u128::from(initial) + u128::from(first) + u128::from(second);
                        let actual = r70_resource_batch_reserve_v1(
                            bytes(initial),
                            &[bytes(first), bytes(second)],
                            bytes(capacity),
                            2,
                            1,
                        );
                        assert_eq!(actual.is_ok(), sum <= u128::from(capacity));
                        if let Ok(admitted) = actual {
                            assert_eq!(admitted.next_used, bytes(sum as u64));
                            assert_eq!(admitted.next_owner, 3);
                        }
                    }
                }
            }
        }
    }
}
