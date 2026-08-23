// Operational semantics for the exact scalar GEMM KIR V1 control-flow graph.
//
// This state machine models the six reviewed blocks, the loop-carried SSA
// values, guarded input reads, and the final output store over mathematical
// integers. A separately retained correspondence module admits these semantics
// only after projection bytes decode to the independently reviewed exact AST.

pub mod scalar_gemm_kir_operational_semantics_v1 {

use super::*;

verus! {

pub enum ScalarKirPcV1 {
    Entry,
    Coordinates,
    Header,
    Body,
    Store,
    Inactive,
    Halted,
    Fault,
}

pub struct ScalarKirStateV1 {
    pub pc: ScalarKirPcV1,
    pub row: nat,
    pub col: nat,
    pub t: nat,
    pub acc: int,
    pub wrote: bool,
    pub output_index: nat,
    pub output_value: int,
}

pub open spec fn scalar_kir_start_state_v1() -> ScalarKirStateV1 {
    ScalarKirStateV1 {
        pc: ScalarKirPcV1::Entry,
        row: 0,
        col: 0,
        t: 0,
        acc: 0,
        wrote: false,
        output_index: 0,
        output_value: 0,
    }
}

pub open spec fn scalar_kir_header_state_v1(
    p: nat,
    n: nat,
    t: nat,
    acc: int,
) -> ScalarKirStateV1
    recommends n > 0,
{
    ScalarKirStateV1 {
        pc: ScalarKirPcV1::Header,
        row: output_row(p, n),
        col: output_col(p, n),
        t,
        acc,
        wrote: false,
        output_index: 0,
        output_value: 0,
    }
}

pub open spec fn scalar_kir_halted_write_state_v1(
    p: nat,
    row: nat,
    col: nat,
    t: nat,
    value: int,
) -> ScalarKirStateV1 {
    ScalarKirStateV1 {
        pc: ScalarKirPcV1::Halted,
        row,
        col,
        t,
        acc: value,
        wrote: true,
        output_index: p,
        output_value: value,
    }
}

pub open spec fn scalar_kir_halted_inactive_state_v1() -> ScalarKirStateV1 {
    ScalarKirStateV1 {
        pc: ScalarKirPcV1::Halted,
        row: 0,
        col: 0,
        t: 0,
        acc: 0,
        wrote: false,
        output_index: 0,
        output_value: 0,
    }
}

pub open spec fn scalar_kir_fault_state_v1(state: ScalarKirStateV1) -> ScalarKirStateV1 {
    ScalarKirStateV1 {
        pc: ScalarKirPcV1::Fault,
        row: state.row,
        col: state.col,
        t: state.t,
        acc: state.acc,
        wrote: false,
        output_index: 0,
        output_value: 0,
    }
}

/// One whole-basic-block transition of the reviewed scalar KIR graph.
pub open spec fn scalar_kir_step_v1(
    a: Seq<int>,
    b: Seq<int>,
    p: nat,
    m: nat,
    n: nat,
    k: nat,
    state: ScalarKirStateV1,
) -> ScalarKirStateV1 {
    match state.pc {
        ScalarKirPcV1::Entry => {
            if p < m * n {
                ScalarKirStateV1 {
                    pc: ScalarKirPcV1::Coordinates,
                    row: 0,
                    col: 0,
                    t: 0,
                    acc: 0,
                    wrote: false,
                    output_index: 0,
                    output_value: 0,
                }
            } else {
                ScalarKirStateV1 {
                    pc: ScalarKirPcV1::Inactive,
                    row: 0,
                    col: 0,
                    t: 0,
                    acc: 0,
                    wrote: false,
                    output_index: 0,
                    output_value: 0,
                }
            }
        },
        ScalarKirPcV1::Coordinates => {
            if n > 0 {
                scalar_kir_header_state_v1(p, n, 0, 0)
            } else {
                scalar_kir_fault_state_v1(state)
            }
        },
        ScalarKirPcV1::Header => {
            if state.t < k {
                ScalarKirStateV1 {
                    pc: ScalarKirPcV1::Body,
                    row: state.row,
                    col: state.col,
                    t: state.t,
                    acc: state.acc,
                    wrote: false,
                    output_index: 0,
                    output_value: 0,
                }
            } else {
                ScalarKirStateV1 {
                    pc: ScalarKirPcV1::Store,
                    row: state.row,
                    col: state.col,
                    t: state.t,
                    acc: state.acc,
                    wrote: false,
                    output_index: 0,
                    output_value: 0,
                }
            }
        },
        ScalarKirPcV1::Body => {
            if state.row < m
                && state.col < n
                && state.t < k
                && a_index(state.row, state.t, k) < a.len()
                && b_index(state.t, state.col, n) < b.len()
            {
                ScalarKirStateV1 {
                    pc: ScalarKirPcV1::Header,
                    row: state.row,
                    col: state.col,
                    t: state.t + 1,
                    acc: state.acc
                        + a[a_index(state.row, state.t, k) as int]
                            * b[b_index(state.t, state.col, n) as int],
                    wrote: false,
                    output_index: 0,
                    output_value: 0,
                }
            } else {
                scalar_kir_fault_state_v1(state)
            }
        },
        ScalarKirPcV1::Store => {
            scalar_kir_halted_write_state_v1(p, state.row, state.col, state.t, state.acc)
        },
        ScalarKirPcV1::Inactive => scalar_kir_halted_inactive_state_v1(),
        ScalarKirPcV1::Halted | ScalarKirPcV1::Fault => state,
    }
}

pub open spec fn scalar_kir_run_v1(
    a: Seq<int>,
    b: Seq<int>,
    p: nat,
    m: nat,
    n: nat,
    k: nat,
    state: ScalarKirStateV1,
    fuel: nat,
) -> ScalarKirStateV1
    decreases fuel,
{
    if fuel == 0
        || state.pc == ScalarKirPcV1::Halted
        || state.pc == ScalarKirPcV1::Fault
    {
        state
    } else {
        scalar_kir_run_v1(
            a,
            b,
            p,
            m,
            n,
            k,
            scalar_kir_step_v1(a, b, p, m, n, k, state),
            (fuel - 1) as nat,
        )
    }
}

pub open spec fn scalar_kir_header_fuel_v1(k: nat, t: nat) -> nat
    recommends t <= k,
    decreases k - t,
{
    if t < k {
        2 + scalar_kir_header_fuel_v1(k, t + 1)
    } else {
        2
    }
}

pub proof fn scalar_kir_run_two_blocks_v1(
    a: Seq<int>,
    b: Seq<int>,
    p: nat,
    m: nat,
    n: nat,
    k: nat,
    state: ScalarKirStateV1,
    remaining_fuel: nat,
)
    requires
        state.pc != ScalarKirPcV1::Halted,
        state.pc != ScalarKirPcV1::Fault,
        scalar_kir_step_v1(a, b, p, m, n, k, state).pc != ScalarKirPcV1::Halted,
        scalar_kir_step_v1(a, b, p, m, n, k, state).pc != ScalarKirPcV1::Fault,
    ensures
        scalar_kir_run_v1(a, b, p, m, n, k, state, remaining_fuel + 2)
            == scalar_kir_run_v1(
                a,
                b,
                p,
                m,
                n,
                k,
                scalar_kir_step_v1(
                    a,
                    b,
                    p,
                    m,
                    n,
                    k,
                    scalar_kir_step_v1(a, b, p, m, n, k, state),
                ),
                remaining_fuel,
            ),
{
    let first = scalar_kir_step_v1(a, b, p, m, n, k, state);
    let second = scalar_kir_step_v1(a, b, p, m, n, k, first);
    assert((remaining_fuel + 2 - 1) as nat == remaining_fuel + 1);
    assert((remaining_fuel + 1 - 1) as nat == remaining_fuel);
    assert(
        scalar_kir_run_v1(a, b, p, m, n, k, state, remaining_fuel + 2)
            == scalar_kir_run_v1(a, b, p, m, n, k, first, remaining_fuel + 1)
    );
    assert(
        scalar_kir_run_v1(a, b, p, m, n, k, first, remaining_fuel + 1)
            == scalar_kir_run_v1(a, b, p, m, n, k, second, remaining_fuel)
    );
}

pub proof fn scalar_kir_body_cycle_refines_dot_v1(
    a: Seq<int>,
    b: Seq<int>,
    p: nat,
    m: nat,
    n: nat,
    k: nat,
    t: nat,
)
    requires
        rust_checked_shape_64(m, n, k),
        p < output_count(m, n),
        a.len() == m * k,
        b.len() == k * n,
        t < k,
    ensures
        scalar_kir_step_v1(
            a,
            b,
            p,
            m,
            n,
            k,
            scalar_kir_step_v1(
                a,
                b,
                p,
                m,
                n,
                k,
                scalar_kir_header_state_v1(
                    p,
                    n,
                    t,
                    exact_dot_prefix(
                        a,
                        b,
                        output_row(p, n),
                        output_col(p, n),
                        m,
                        n,
                        k,
                        t,
                    ),
                ),
            ),
        ) == scalar_kir_header_state_v1(
            p,
            n,
            t + 1,
            exact_dot_prefix(
                a,
                b,
                output_row(p, n),
                output_col(p, n),
                m,
                n,
                k,
                t + 1,
            ),
        ),
{
    active_invocation_has_unique_coordinates(p, m, n);
    active_accesses_are_in_bounds(p, t, m, n, k);
    exact_dot_has_fixed_sequential_recurrence(
        a,
        b,
        output_row(p, n),
        output_col(p, n),
        m,
        n,
        k,
        t,
    );
}

pub proof fn scalar_kir_header_run_refines_dot_v1(
    a: Seq<int>,
    b: Seq<int>,
    p: nat,
    m: nat,
    n: nat,
    k: nat,
    t: nat,
)
    requires
        rust_checked_shape_64(m, n, k),
        p < output_count(m, n),
        a.len() == m * k,
        b.len() == k * n,
        t <= k,
    ensures
        scalar_kir_run_v1(
            a,
            b,
            p,
            m,
            n,
            k,
            scalar_kir_header_state_v1(
                p,
                n,
                t,
                exact_dot_prefix(
                    a,
                    b,
                    output_row(p, n),
                    output_col(p, n),
                    m,
                    n,
                    k,
                    t,
                ),
            ),
            scalar_kir_header_fuel_v1(k, t),
        ) == scalar_kir_halted_write_state_v1(
            p,
            output_row(p, n),
            output_col(p, n),
            k,
            exact_dot_prefix(
                a,
                b,
                output_row(p, n),
                output_col(p, n),
                m,
                n,
                k,
                k,
            ),
        ),
    decreases k - t,
{
    active_invocation_has_unique_coordinates(p, m, n);
    if t < k {
        let state = scalar_kir_header_state_v1(
            p,
            n,
            t,
            exact_dot_prefix(
                a,
                b,
                output_row(p, n),
                output_col(p, n),
                m,
                n,
                k,
                t,
            ),
        );
        let remaining_fuel = scalar_kir_header_fuel_v1(k, t + 1);
        scalar_kir_body_cycle_refines_dot_v1(a, b, p, m, n, k, t);
        assert(state.pc == ScalarKirPcV1::Header);
        assert(
            scalar_kir_step_v1(a, b, p, m, n, k, state).pc
                == ScalarKirPcV1::Body
        );
        scalar_kir_run_two_blocks_v1(a, b, p, m, n, k, state, remaining_fuel);
        scalar_kir_header_run_refines_dot_v1(a, b, p, m, n, k, t + 1);
        assert(scalar_kir_header_fuel_v1(k, t) == remaining_fuel + 2);
    } else {
        let state = scalar_kir_header_state_v1(
            p,
            n,
            t,
            exact_dot_prefix(
                a,
                b,
                output_row(p, n),
                output_col(p, n),
                m,
                n,
                k,
                t,
            ),
        );
        assert(t == k);
        assert(state.pc == ScalarKirPcV1::Header);
        assert(
            scalar_kir_step_v1(a, b, p, m, n, k, state).pc
                == ScalarKirPcV1::Store
        );
        scalar_kir_run_two_blocks_v1(a, b, p, m, n, k, state, 0);
    }
}

/// Complete active-invocation theorem for the reviewed scalar KIR machine.
pub proof fn scalar_kir_active_execution_refines_integer_model_v1(
    a: Seq<int>,
    b: Seq<int>,
    p: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        rust_checked_shape_64(m, n, k),
        p < output_count(m, n),
        a.len() == m * k,
        b.len() == k * n,
    ensures
        scalar_kir_run_v1(
            a,
            b,
            p,
            m,
            n,
            k,
            scalar_kir_start_state_v1(),
            2 + scalar_kir_header_fuel_v1(k, 0),
        ) == scalar_kir_halted_write_state_v1(
            p,
            output_row(p, n),
            output_col(p, n),
            k,
            exact_dot_prefix(
                a,
                b,
                output_row(p, n),
                output_col(p, n),
                m,
                n,
                k,
                k,
            ),
        ),
{
    active_invocation_has_unique_coordinates(p, m, n);
    let start = scalar_kir_start_state_v1();
    assert(start.pc == ScalarKirPcV1::Entry);
    assert(
        scalar_kir_step_v1(a, b, p, m, n, k, start).pc
            == ScalarKirPcV1::Coordinates
    );
    scalar_kir_run_two_blocks_v1(
        a,
        b,
        p,
        m,
        n,
        k,
        start,
        scalar_kir_header_fuel_v1(k, 0),
    );
    scalar_kir_header_run_refines_dot_v1(a, b, p, m, n, k, 0);
}

pub proof fn scalar_kir_inactive_execution_performs_no_store_v1(
    a: Seq<int>,
    b: Seq<int>,
    p: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires p >= output_count(m, n),
    ensures
        scalar_kir_run_v1(
            a,
            b,
            p,
            m,
            n,
            k,
            scalar_kir_start_state_v1(),
            2,
        ) == scalar_kir_halted_inactive_state_v1(),
{
    let start = scalar_kir_start_state_v1();
    assert(start.pc == ScalarKirPcV1::Entry);
    assert(
        scalar_kir_step_v1(a, b, p, m, n, k, start).pc
            == ScalarKirPcV1::Inactive
    );
    scalar_kir_run_two_blocks_v1(a, b, p, m, n, k, start, 0);
}

} // verus!

} // mod scalar_gemm_kir_operational_semantics_v1
