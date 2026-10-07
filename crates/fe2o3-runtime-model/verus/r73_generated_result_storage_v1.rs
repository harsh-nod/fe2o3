// Production-used peak/shape guards. Arc identity, synchronization, Rust storage
// disposal, complete-decoder refinement and GPU completion remain outside this proof.
use vstd::prelude::*;
use vstd::assert_seqs_equal;
verus! {
pub open spec fn peak_admitted_v1(bytes: u64, typed: bool) -> bool {
    !typed || bytes <= u64::MAX / 2
}
pub open spec fn peak_v1(bytes: u64, typed: bool) -> int {
    if typed { 2 * (bytes as int) } else { bytes as int }
}
pub open spec fn charge_v1(bytes: u64, typed: bool) -> Seq<u64> {
    Seq::new(19, |i: int| if i == 14 { peak_v1(bytes, typed) as u64 } else { 0u64 })
}
pub fn generated_result_peak_v1(bytes: u64, typed: bool) -> (result: Option<[u64; 19]>)
    ensures result.is_some() == peak_admitted_v1(bytes, typed),
        match result { Some(charge) => charge@ == charge_v1(bytes, typed), None => true },
{
    if typed && bytes > u64::MAX / 2 { return None; }
    let peak = if typed { bytes * 2 } else { bytes };
    let mut charge = [0u64; 19];
    charge[14] = peak;
    proof { assert_seqs_equal!(charge@, charge_v1(bytes, typed)); }
    Some(charge)
}
pub fn generated_result_shape_v1(expected: u64, actual: u64, capacity: u64, access: bool) -> (accepted: bool)
    ensures accepted == (expected == actual && actual == capacity && access),
{
    expected == actual && actual == capacity && access
}
pub proof fn typed_peak_retains_both_copies_v1(bytes: u64)
    requires peak_admitted_v1(bytes, true),
    ensures charge_v1(bytes, true).len() == 19,
        (charge_v1(bytes, true)[14] as int) == 2 * (bytes as int),
{}
pub proof fn read_only_peak_retains_one_copy_v1(bytes: u64)
    ensures peak_admitted_v1(bytes, false), charge_v1(bytes, false)[14] == bytes,
{}
pub proof fn other_coordinates_are_not_backing_or_owner_debits_v1(bytes: u64, typed: bool, index: int)
    requires peak_admitted_v1(bytes, typed), 0 <= index < 19, index != 14,
    ensures charge_v1(bytes, typed)[index] == 0,
{}
pub proof fn overflow_rejects_only_unrepresentable_typed_peak_v1(bytes: u64)
    requires bytes > u64::MAX / 2,
    ensures !peak_admitted_v1(bytes, true), peak_admitted_v1(bytes, false),
{}
pub proof fn empty_results_have_zero_peak_but_remain_admissible_v1(typed: bool)
    ensures peak_admitted_v1(0, typed), charge_v1(0, typed)[14] == 0,
{}
}
