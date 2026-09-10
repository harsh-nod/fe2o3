// Executable N2 cost projection corresponding to src/r68_device_backing_credits.rs.
// The nineteen coordinates are R67's existing vector, not a new resource schema.
// Native layout extraction, exact domains, ownership, disposal, mutex behavior
// and root/global budgets are outside this projection and algebraic composition.
use vstd::prelude::*;
use vstd::assert_seqs_equal;
verus! {
pub open spec fn valid_backing_v1(backing: u64) -> bool {
    0 < backing <= 206158430208
}

pub open spec fn backing_charge_v1(backing: u64) -> Seq<u64> {
    Seq::new(19, |i: int| if i == 3 { backing } else if i == 18 { 1u64 } else { 0u64 })
}

pub fn device_backing_charge_v1(backing: u64) -> (result: Option<[u64; 19]>)
    ensures
        result.is_some() == valid_backing_v1(backing),
        match result {
            Some(charge) => charge@ == backing_charge_v1(backing),
            None => true,
        },
{
    if backing == 0 || backing > 206158430208 { return None; }
    let mut charge = [0u64; 19];
    charge[3] = backing;
    charge[18] = 1;
    proof { assert_seqs_equal!(charge@, backing_charge_v1(backing)); }
    Some(charge)
}

pub proof fn projected_backing_and_record_are_exact_v1(backing: u64)
    requires valid_backing_v1(backing),
    ensures
        backing_charge_v1(backing)[3] == backing,
        backing_charge_v1(backing)[18] == 1,
        backing_charge_v1(backing).len() == 19,
{}

pub proof fn no_other_dimension_is_charged_v1(backing: u64, index: int)
    requires valid_backing_v1(backing), 0 <= index < 19, index != 3, index != 18,
    ensures backing_charge_v1(backing)[index] == 0,
{}

// R67's reserve/release postconditions instantiated with the N2 projection.
// This is algebraic composition, not a proof that a native release occurred.
pub proof fn projected_reserve_release_conserves_v1(
    backing: u64, used: Seq<u64>, next: Seq<u64>, released: Seq<u64>,
)
    requires
        valid_backing_v1(backing),
        used.len() == 19, next.len() == 19, released.len() == 19,
        forall|i: int| 0 <= i < 19 ==> next[i] as int == used[i] as int + backing_charge_v1(backing)[i] as int,
        forall|i: int| 0 <= i < 19 ==> released[i] as int == next[i] as int - backing_charge_v1(backing)[i] as int,
    ensures released == used,
{
    assert_seqs_equal!(released, used);
}
}
