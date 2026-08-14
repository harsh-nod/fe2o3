use vstd::prelude::*;

#[path = "../tiled_gemm_host_contract.rs"]
mod model;

verus! {

/// Wrong two-bit block permutation: bounded and involutive, but not AMD XOR.
pub open spec fn mutated_two_bit_permutation_v1(left: nat, right: nat) -> nat
    recommends left < 4, right < 4,
{
    if right == 0 {
        1
    } else if right == 1 {
        0
    } else if right == 2 {
        3
    } else {
        2
    }
}

pub proof fn mutated_two_bit_permutation_is_bounded_and_involutive_v1(
    left: nat,
    right: nat,
)
    requires left < 4, right < 4,
    ensures
        mutated_two_bit_permutation_v1(left, right) < 4,
        mutated_two_bit_permutation_v1(
            left,
            mutated_two_bit_permutation_v1(left, right),
        ) == right,
{
    assert(right == 0 || right == 1 || right == 2 || right == 3);
    if right == 0 {
    } else if right == 1 {
    } else if right == 2 {
    } else {
    }
}

pub open spec fn mutated_xor4_column_v1(row: nat, col: nat) -> nat {
    mutated_two_bit_permutation_v1(row % 4, col / 4) * 4 + col % 4
}

pub open spec fn mutated_xor4_index_v1(row: nat, col: nat) -> nat {
    row * 16 + mutated_xor4_column_v1(row, col)
}

/// This mutation swizzles row zero, so it is not the old row-major mutation.
pub proof fn mutated_two_bit_storage_is_not_row_major_v1()
    ensures
        mutated_xor4_index_v1(0, 0) == 4,
        mutated_xor4_index_v1(0, 0) != 0,
{
}

/// Expected failure marker: mutated_two_bit_permutation_matches_official_xor2_v1.
pub proof fn mutated_two_bit_permutation_matches_official_xor2_v1(left: nat, right: nat)
    requires left < 4, right < 4,
    ensures
        mutated_two_bit_permutation_v1(left, right) == model::xor2_v1(left, right),
{
}

} // verus!
