// Profile-level equations transcribed from the reviewed scalar GEMM KIR V5 graph.
//
// The host admits this source only after byte-exact validation against `scalar_gemm_v1_module`.
// These theorems model that graph's entry guard, coordinate block, loop-carried accumulator,
// address arithmetic, and final store. This is not a decoder or whole-KIR operational semantics,
// and it does not model Rust MIR or IEEE-754 `f32` operations.

pub mod scalar_gemm_kir_integer_profile_v1 {

use super::*;

verus! {

/// Entry block 0, values 7 through 11: `p < zext(m) * zext(n)`.
pub open spec fn scalar_kir_v5_invocation_is_active_v1(p: nat, m: nat, n: nat) -> bool {
    p < m * n
}

/// Coordinate block 1, value 15.
pub open spec fn scalar_kir_v5_row_v1(p: nat, n: nat) -> nat
    recommends n > 0,
{
    p / n
}

/// Coordinate block 1, value 16.
pub open spec fn scalar_kir_v5_col_v1(p: nat, n: nat) -> nat
    recommends n > 0,
{
    p % n
}

/// Body block 3, values 22 through 24.
pub open spec fn scalar_kir_v5_a_offset_v1(row: nat, t: nat, k: nat) -> nat {
    row * k + t
}

/// Body block 3, values 22, 25, and 26.
pub open spec fn scalar_kir_v5_b_offset_v1(t: nat, col: nat, n: nat) -> nat {
    t * n + col
}

/// Header/body cyclic SSA values 19, 20, 28, 30, 31, 32, and 34.
pub open spec fn scalar_kir_v5_accumulator_v1(
    a: Seq<int>,
    b: Seq<int>,
    row: nat,
    col: nat,
    m: nat,
    n: nat,
    k: nat,
    t: nat,
) -> int
    recommends
        rust_checked_shape_64(m, n, k),
        row < m,
        col < n,
        a.len() == m * k,
        b.len() == k * n,
        t <= k,
    decreases t,
{
    if t == 0 {
        0
    } else {
        scalar_kir_v5_accumulator_v1(a, b, row, col, m, n, k, (t - 1) as nat)
            + a[scalar_kir_v5_a_offset_v1(row, (t - 1) as nat, k) as int]
                * b[scalar_kir_v5_b_offset_v1((t - 1) as nat, col, n) as int]
    }
}

pub proof fn scalar_kir_v5_entry_guard_refines_model_v1(p: nat, m: nat, n: nat)
    ensures
        scalar_kir_v5_invocation_is_active_v1(p, m, n)
            == (p < output_count(m, n)),
{
}

pub proof fn scalar_kir_v5_coordinates_refine_model_v1(p: nat, m: nat, n: nat)
    requires
        rust_checked_shape_64(m, n, 0),
        scalar_kir_v5_invocation_is_active_v1(p, m, n),
    ensures
        n > 0,
        scalar_kir_v5_row_v1(p, n) == output_row(p, n),
        scalar_kir_v5_col_v1(p, n) == output_col(p, n),
        scalar_kir_v5_row_v1(p, n) < m,
        scalar_kir_v5_col_v1(p, n) < n,
        flatten(scalar_kir_v5_row_v1(p, n), scalar_kir_v5_col_v1(p, n), n) == p,
{
    active_invocation_has_unique_coordinates(p, m, n);
}

pub proof fn scalar_kir_v5_addresses_refine_model_v1(
    p: nat,
    t: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        rust_checked_shape_64(m, n, k),
        scalar_kir_v5_invocation_is_active_v1(p, m, n),
        t < k,
    ensures
        scalar_kir_v5_a_offset_v1(scalar_kir_v5_row_v1(p, n), t, k)
            == a_index(output_row(p, n), t, k),
        scalar_kir_v5_b_offset_v1(t, scalar_kir_v5_col_v1(p, n), n)
            == b_index(t, output_col(p, n), n),
        scalar_kir_v5_a_offset_v1(scalar_kir_v5_row_v1(p, n), t, k) < m * k,
        scalar_kir_v5_b_offset_v1(t, scalar_kir_v5_col_v1(p, n), n) < k * n,
    {
    scalar_kir_v5_coordinates_refine_model_v1(p, m, n);
    active_accesses_are_in_bounds(p, t, m, n, k);
}

pub proof fn scalar_kir_v5_accumulator_step_refines_model_v1(
    a: Seq<int>,
    b: Seq<int>,
    row: nat,
    col: nat,
    m: nat,
    n: nat,
    k: nat,
    t: nat,
)
    requires
        rust_checked_shape_64(m, n, k),
        row < m,
        col < n,
        a.len() == m * k,
        b.len() == k * n,
        t < k,
    ensures
        scalar_kir_v5_accumulator_v1(a, b, row, col, m, n, k, t + 1)
            == scalar_kir_v5_accumulator_v1(a, b, row, col, m, n, k, t)
                + a[a_index(row, t, k) as int] * b[b_index(t, col, n) as int],
{
}

pub proof fn scalar_kir_v5_accumulator_refines_model_v1(
    a: Seq<int>,
    b: Seq<int>,
    row: nat,
    col: nat,
    m: nat,
    n: nat,
    k: nat,
    t: nat,
)
    requires
        rust_checked_shape_64(m, n, k),
        row < m,
        col < n,
        a.len() == m * k,
        b.len() == k * n,
        t <= k,
    ensures
        scalar_kir_v5_accumulator_v1(a, b, row, col, m, n, k, t)
            == exact_dot_prefix(a, b, row, col, m, n, k, t),
    decreases t,
{
    if t > 0 {
        scalar_kir_v5_accumulator_refines_model_v1(
            a,
            b,
            row,
            col,
            m,
            n,
            k,
            (t - 1) as nat,
        );
    }
}

pub proof fn scalar_kir_v5_store_refines_model_v1(p: nat, m: nat, n: nat)
    requires
        rust_checked_shape_64(m, n, 0),
        scalar_kir_v5_invocation_is_active_v1(p, m, n),
    ensures
        flatten(scalar_kir_v5_row_v1(p, n), scalar_kir_v5_col_v1(p, n), n) == p,
        output_initialized_by(p, p, m, n),
{
    scalar_kir_v5_coordinates_refine_model_v1(p, m, n);
}

/// Complete profile theorem for one active invocation under mathematical-integer arithmetic.
pub proof fn scalar_kir_v5_active_invocation_refines_integer_model_v1(
    a: Seq<int>,
    b: Seq<int>,
    p: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        rust_checked_shape_64(m, n, k),
        scalar_kir_v5_invocation_is_active_v1(p, m, n),
        a.len() == m * k,
        b.len() == k * n,
    ensures
        n > 0,
        scalar_kir_v5_a_offset_v1(scalar_kir_v5_row_v1(p, n), 0, k)
            == a_index(output_row(p, n), 0, k),
        scalar_kir_v5_b_offset_v1(0, scalar_kir_v5_col_v1(p, n), n)
            == b_index(0, output_col(p, n), n),
        scalar_kir_v5_accumulator_v1(
            a,
            b,
            scalar_kir_v5_row_v1(p, n),
            scalar_kir_v5_col_v1(p, n),
            m,
            n,
            k,
            k,
        ) == exact_dot_prefix(a, b, output_row(p, n), output_col(p, n), m, n, k, k),
        output_initialized_by(p, p, m, n),
{
    scalar_kir_v5_coordinates_refine_model_v1(p, m, n);
    scalar_kir_v5_accumulator_refines_model_v1(
        a,
        b,
        scalar_kir_v5_row_v1(p, n),
        scalar_kir_v5_col_v1(p, n),
        m,
        n,
        k,
        k,
    );
    scalar_kir_v5_store_refines_model_v1(p, m, n);
}

} // verus!

} // mod scalar_gemm_kir_integer_profile_v1
