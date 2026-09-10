// Executable admission projection of src/r70_resource_batch.rs: errors map to None.
// The scalar step mirrors the existing R67 checked-vector operation. This proves
// complete-roster arithmetic and owner bounds, not mutex/arena extraction, token
// issuance, native costs/disposal, parent admission or whole-account refinement.
use vstd::prelude::*;
verus! {
pub open spec fn prefix_used_v1(
    used: [u64; 19], charges: Seq<[u64; 19]>, end: int, dimension: int,
) -> int
    decreases end,
{
    if end <= 0 { used@[dimension] as int }
    else {
        prefix_used_v1(used, charges, end - 1, dimension)
            + charges[end - 1]@[dimension] as int
    }
}

pub open spec fn batch_fits_v1(
    used: [u64; 19], charges: Seq<[u64; 19]>, capacity: [u64; 19],
    free_records: usize, first_owner: u64,
) -> bool {
    &&& 0 < charges.len() <= 65536
    &&& charges.len() <= free_records
    &&& first_owner > 0
    &&& (first_owner as int) + charges.len() <= (u64::MAX as int)
    &&& forall|end: int, dimension: int|
        1 <= end <= charges.len() && 0 <= dimension < 19
        ==> prefix_used_v1(used, charges, end, dimension) <= capacity@[dimension]
}

pub fn resource_reserve_step_v1(
    used: [u64; 19], charge: [u64; 19], capacity: [u64; 19],
) -> (result: Option<[u64; 19]>)
    ensures
        result.is_some() == (forall|d: int| 0 <= d < 19 ==>
            (used@[d] as int) + (charge@[d] as int) <= (capacity@[d] as int)),
        match result {
            Some(next) => forall|d: int| 0 <= d < 19 ==>
                (next@[d] as int) == (used@[d] as int) + (charge@[d] as int),
            None => true,
        },
{
    let mut next = [0u64; 19];
    let mut d = 0usize;
    while d < 19
        invariant
            0 <= d <= 19,
            forall|i: int| 0 <= i < d ==>
                (next@[i] as int) == (used@[i] as int) + (charge@[i] as int),
            forall|i: int| 0 <= i < d ==> next@[i] <= capacity@[i],
        decreases 19 - d,
    {
        let value = match used[d].checked_add(charge[d]) {
            Some(value) => value,
            None => return None,
        };
        if value > capacity[d] { return None; }
        next[d] = value;
        d += 1;
    }
    Some(next)
}

pub struct BatchAdmissionV1 {
    pub next_used: [u64; 19],
    pub next_owner: u64,
}

pub fn resource_batch_reserve_v1(
    used: [u64; 19], charges: &[[u64; 19]], capacity: [u64; 19],
    free_records: usize, first_owner: u64,
) -> (result: Option<BatchAdmissionV1>)
    ensures
        result.is_some() == batch_fits_v1(used, charges@, capacity, free_records, first_owner),
        match result {
            Some(admitted) => {
                &&& (admitted.next_owner as int) == (first_owner as int) + charges.len()
                &&& forall|d: int| 0 <= d < 19 ==>
                    (admitted.next_used@[d] as int) == prefix_used_v1(used, charges@, charges.len() as int, d)
            },
            None => true,
        },
{
    if charges.len() == 0 || charges.len() > 65536 { return None; }
    if charges.len() > free_records { return None; }
    if first_owner == 0 { return None; }
    let owner_end = match first_owner.checked_add(charges.len() as u64) {
        Some(owner_end) => owner_end,
        None => return None,
    };
    let mut next_used = used;
    let mut member = 0usize;
    while member < charges.len()
        invariant
            0 < charges.len() <= 65536,
            charges.len() <= free_records,
            first_owner > 0,
            (owner_end as int) == (first_owner as int) + charges.len(),
            0 <= member <= charges.len(),
            forall|d: int| 0 <= d < 19 ==>
                (next_used@[d] as int) == prefix_used_v1(used, charges@, member as int, d),
            forall|end: int, d: int| 1 <= end <= member && 0 <= d < 19 ==>
                prefix_used_v1(used, charges@, end, d) <= capacity@[d],
        decreases charges.len() - member,
    {
        let next = match resource_reserve_step_v1(next_used, charges[member], capacity) {
            Some(next) => next,
            None => {
                proof {
                    let d = choose|d: int| 0 <= d < 19 &&
                        (next_used@[d] as int) + (charges@[member as int]@[d] as int)
                            > (capacity@[d] as int);
                    assert(prefix_used_v1(used, charges@, (member as int) + 1, d)
                        == (next_used@[d] as int) + (charges@[member as int]@[d] as int));
                    assert(!batch_fits_v1(used, charges@, capacity, free_records, first_owner));
                }
                return None;
            },
        };
        proof {
            assert forall|d: int| 0 <= d < 19 implies
                (next@[d] as int) == prefix_used_v1(used, charges@, (member as int) + 1, d) by {
                assert(prefix_used_v1(used, charges@, (member as int) + 1, d)
                    == prefix_used_v1(used, charges@, member as int, d)
                        + (charges@[member as int]@[d] as int));
            }
            assert forall|end: int, d: int| 1 <= end <= (member as int) + 1 && 0 <= d < 19 implies
                prefix_used_v1(used, charges@, end, d) <= capacity@[d] by {
                if end == (member as int) + 1 {
                    assert(prefix_used_v1(used, charges@, end, d) == (next@[d] as int));
                }
            }
        }
        next_used = next;
        member += 1;
    }
    Some(BatchAdmissionV1 { next_used, next_owner: owner_end })
}

pub proof fn admitted_roster_has_all_records_and_owner_bounds_v1(
    used: [u64; 19], charges: Seq<[u64; 19]>, capacity: [u64; 19],
    free_records: usize, first_owner: u64, member: int,
)
    requires
        batch_fits_v1(used, charges, capacity, free_records, first_owner),
        0 <= member < charges.len(),
    ensures
        0 < charges.len() <= 65536,
        charges.len() <= free_records,
        0 < (first_owner as int) + member < (u64::MAX as int),
{}

pub proof fn distinct_members_have_distinct_owners_v1(first_owner: u64, count: int, a: int, b: int)
    requires
        first_owner > 0, 0 < count <= 65536,
        (first_owner as int) + count <= (u64::MAX as int),
        0 <= a < count, 0 <= b < count, a != b,
    ensures (first_owner as int) + a != (first_owner as int) + b,
{}

pub proof fn late_member_capacity_failure_rejects_complete_roster_v1(
    used: [u64; 19], charges: Seq<[u64; 19]>, capacity: [u64; 19],
    free_records: usize, first_owner: u64, end: int, dimension: int,
)
    requires
        1 <= end <= charges.len(), 0 <= dimension < 19,
        prefix_used_v1(used, charges, end, dimension) > capacity@[dimension],
    ensures !batch_fits_v1(used, charges, capacity, free_records, first_owner),
{}

// R67's exact-member subtraction postcondition instantiated with two siblings.
// This does not establish that either sibling's underlying resource was disposed.
pub proof fn exact_member_refund_preserves_sibling_charge_v1(
    used: u64, first: u64, second: u64, released: u64,
)
    requires
        (released as int) == (used as int) + (first as int) + (second as int) - (first as int),
    ensures (released as int) == (used as int) + (second as int),
{}
}
