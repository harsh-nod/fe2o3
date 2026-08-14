use vstd::prelude::*;

#[path = "../tiled_gemm_host_contract.rs"]
mod model;

verus! {

/// Mutated LDS layout is internally injective row-major without XOR4.
pub open spec fn mutated_row_major_lds_index_v1(row: nat, col: nat) -> nat {
    row * 16 + col
}

pub proof fn mutated_row_major_lds_remains_injective_v1(
    left_row: nat,
    left_col: nat,
    right_row: nat,
    right_col: nat,
)
    requires
        left_row < 16,
        left_col < 16,
        right_row < 16,
        right_col < 16,
        left_row != right_row || left_col != right_col,
    ensures
        mutated_row_major_lds_index_v1(left_row, left_col)
            != mutated_row_major_lds_index_v1(right_row, right_col),
{
    model::distinct_logical_coordinates_have_distinct_row_major_v1(
        left_row, left_col, right_row, right_col,
    );
}

/// Expected failure marker: mutated_xor4_matches_official_storage_v1.
pub proof fn mutated_xor4_matches_official_storage_v1(row: nat, col: nat)
    requires row < 16, col < 16,
    ensures
        mutated_row_major_lds_index_v1(row, col) == model::xor4_lds_index_v1(row, col),
{
}

} // verus!
