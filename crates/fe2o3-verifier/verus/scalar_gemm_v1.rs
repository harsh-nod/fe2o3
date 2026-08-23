use vstd::arithmetic::div_mod::{lemma_fundamental_div_mod, lemma_mod_bound};
use vstd::prelude::*;

verus! {

pub open spec fn output_count(m: nat, n: nat) -> nat {
    m * n
}

pub open spec fn rust_checked_shape_64(m: nat, n: nat, k: nat) -> bool {
    &&& m <= 0xffff_ffff
    &&& n <= 0xffff_ffff
    &&& k <= 0xffff_ffff
    &&& m * k <= 0x3fff_ffff_ffff_ffff
    &&& k * n <= 0x3fff_ffff_ffff_ffff
    &&& m * n <= 0x3fff_ffff_ffff_ffff
}

pub open spec fn output_row(p: nat, n: nat) -> nat
    recommends n > 0,
{
    p / n
}

pub open spec fn output_col(p: nat, n: nat) -> nat
    recommends n > 0,
{
    p % n
}

pub open spec fn flatten(row: nat, col: nat, n: nat) -> nat {
    row * n + col
}

pub open spec fn a_index(row: nat, t: nat, k: nat) -> nat {
    row * k + t
}

pub open spec fn b_index(t: nat, col: nat, n: nat) -> nat {
    t * n + col
}

/// Exact abstract recurrence over mathematical integers. This is not an
/// IEEE-754 or Rust `f32` arithmetic model.
pub open spec fn exact_dot_prefix(
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
        exact_dot_prefix(a, b, row, col, m, n, k, (t - 1) as nat)
            + a[a_index(row, (t - 1) as nat, k) as int]
                * b[b_index((t - 1) as nat, col, n) as int]
    }
}

pub open spec fn output_initialized_by(p: nat, output: nat, m: nat, n: nat) -> bool {
    p < output_count(m, n) && output == p
}

pub open spec fn all_inputs_initialized(
    a_initialized: Seq<bool>,
    b_initialized: Seq<bool>,
    m: nat,
    n: nat,
    k: nat,
) -> bool {
    a_initialized.len() == m * k
        && b_initialized.len() == k * n
        && forall |index: nat| index < a_initialized.len()
            ==> a_initialized[index as int]
        && forall |index: nat| index < b_initialized.len()
            ==> b_initialized[index as int]
}

pub proof fn active_invocation_has_unique_coordinates(p: nat, m: nat, n: nat)
    requires
        rust_checked_shape_64(m, n, 0),
        p < output_count(m, n),
    ensures
        n > 0,
        output_row(p, n) < m,
        output_col(p, n) < n,
        flatten(output_row(p, n), output_col(p, n), n) == p,
{
    if n == 0 {
        assert(output_count(m, n) == 0);
        assert(false);
    }
    lemma_mod_bound(p as int, n as int);
    lemma_fundamental_div_mod(p as int, n as int);
    assert(p == (p / n) * n + p % n);
    let row = p / n;
    let col = p % n;
    assert(row < m) by {
        if row >= m {
            assert(row * n >= m * n) by (nonlinear_arith)
                requires
                    row >= m,
                    n > 0,
            ;
            assert(p >= m * n);
            assert(false);
        }
    }
}

pub proof fn active_accesses_are_in_bounds(p: nat, t: nat, m: nat, n: nat, k: nat)
    requires
        rust_checked_shape_64(m, n, k),
        p < output_count(m, n),
        t < k,
    ensures
        a_index(output_row(p, n), t, k) < m * k,
        b_index(t, output_col(p, n), n) < k * n,
        p < m * n,
{
    assert(rust_checked_shape_64(m, n, 0));
    active_invocation_has_unique_coordinates(p, m, n);
    assert(k > 0);
    assert(output_row(p, n) + 1 <= m);
    assert((output_row(p, n) + 1) * k <= m * k) by (nonlinear_arith)
        requires
            output_row(p, n) + 1 <= m,
            k > 0,
    ;
    assert(output_row(p, n) * k + t < (output_row(p, n) + 1) * k)
        by (nonlinear_arith)
        requires
            t < k,
            k > 0,
    ;
    assert(t + 1 <= k);
    assert((t + 1) * n <= k * n) by (nonlinear_arith)
        requires
            t + 1 <= k,
            n > 0,
    ;
    assert(t * n + output_col(p, n) < (t + 1) * n) by (nonlinear_arith)
        requires
            output_col(p, n) < n,
            n > 0,
    ;
}

pub proof fn active_input_reads_are_initialized(
    a_initialized: Seq<bool>,
    b_initialized: Seq<bool>,
    p: nat,
    t: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        rust_checked_shape_64(m, n, k),
        p < output_count(m, n),
        t < k,
        all_inputs_initialized(a_initialized, b_initialized, m, n, k),
    ensures
        a_initialized[a_index(output_row(p, n), t, k) as int],
        b_initialized[b_index(t, output_col(p, n), n) as int],
{
    active_accesses_are_in_bounds(p, t, m, n, k);
    assert(a_index(output_row(p, n), t, k) < a_initialized.len());
    assert(b_index(t, output_col(p, n), n) < b_initialized.len());
    assert(a_initialized[a_index(output_row(p, n), t, k) as int]);
    assert(b_initialized[b_index(t, output_col(p, n), n) as int]);
}

/// Canonical output-index mapping only; this does not attest physical stores.
pub proof fn distinct_active_invocations_have_distinct_output_indices(
    left: nat,
    right: nat,
    m: nat,
    n: nat,
)
    requires
        rust_checked_shape_64(m, n, 0),
        left < output_count(m, n),
        right < output_count(m, n),
        left != right,
    ensures
        left != right,
        !output_initialized_by(left, right, m, n),
        !output_initialized_by(right, left, m, n),
{
}

/// Canonical invocation-domain property only; this does not attest a launch.
pub proof fn every_output_has_unique_canonical_invocation(
    output: nat,
    m: nat,
    n: nat,
)
    requires
        rust_checked_shape_64(m, n, 0),
        output < output_count(m, n),
    ensures
        output_initialized_by(output, output, m, n),
        forall |other: nat| other < output_count(m, n) && other != output
            ==> !output_initialized_by(other, output, m, n),
{
}

pub proof fn exact_dot_has_fixed_sequential_recurrence(
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
        exact_dot_prefix(a, b, row, col, m, n, k, t + 1)
            == exact_dot_prefix(a, b, row, col, m, n, k, t)
                + a[a_index(row, t, k) as int] * b[b_index(t, col, n) as int],
{
    assert(a_index(row, t, k) < a.len()) by (nonlinear_arith)
        requires
            row < m,
            t < k,
            a.len() == m * k,
    ;
    assert(b_index(t, col, n) < b.len()) by (nonlinear_arith)
        requires
            t < k,
            col < n,
            b.len() == k * n,
    ;
}

pub proof fn abstract_dot_starts_at_zero(
    a: Seq<int>,
    b: Seq<int>,
    row: nat,
    col: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        rust_checked_shape_64(m, n, k),
        row < m,
        col < n,
        a.len() == m * k,
        b.len() == k * n,
    ensures
        exact_dot_prefix(a, b, row, col, m, n, k, 0) == 0,
{
}

} // verus!
