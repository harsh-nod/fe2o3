// Exact projection AST to reviewed operational-machine correspondence.
//
// Execution is defined only for bytes that decode to the independently
// reviewed scalar GEMM AST. Every other byte stream transitions to Fault.
// The retained active and inactive machine theorems can therefore be composed
// with exact AST decoding without granting semantics to a broader graph.

pub mod scalar_gemm_kir_projection_operational_correspondence_v1 {

use super::*;

use super::scalar_gemm_kir_operational_semantics_v1::{
    ScalarKirStateV1,
    scalar_kir_active_execution_refines_integer_model_v1,
    scalar_kir_fault_state_v1,
    scalar_kir_halted_inactive_state_v1,
    scalar_kir_halted_write_state_v1,
    scalar_kir_header_fuel_v1,
    scalar_kir_inactive_execution_performs_no_store_v1,
    scalar_kir_run_v1,
    scalar_kir_start_state_v1,
    scalar_kir_step_v1,
};
use super::scalar_gemm_kir_projection_exact_v1::{
    generated_scalar_kir_projection_decodes_to_exact_ast_v1,
    scalar_kir_projection_ast_is_exact_v1,
};
use super::scalar_gemm_kir_projection_generated_v1::FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1;

verus! {

pub open spec fn scalar_kir_projection_step_v1(
    bytes: Seq<u8>,
    a: Seq<int>,
    b: Seq<int>,
    p: nat,
    m: nat,
    n: nat,
    k: nat,
    state: ScalarKirStateV1,
) -> ScalarKirStateV1 {
    if scalar_kir_projection_ast_is_exact_v1(bytes) {
        scalar_kir_step_v1(a, b, p, m, n, k, state)
    } else {
        scalar_kir_fault_state_v1(state)
    }
}

pub open spec fn scalar_kir_projection_run_v1(
    bytes: Seq<u8>,
    a: Seq<int>,
    b: Seq<int>,
    p: nat,
    m: nat,
    n: nat,
    k: nat,
    state: ScalarKirStateV1,
    fuel: nat,
) -> ScalarKirStateV1 {
    if scalar_kir_projection_ast_is_exact_v1(bytes) {
        scalar_kir_run_v1(a, b, p, m, n, k, state, fuel)
    } else {
        scalar_kir_fault_state_v1(state)
    }
}

pub proof fn generated_scalar_kir_projection_denotes_reviewed_step_v1(
    a: Seq<int>,
    b: Seq<int>,
    p: nat,
    m: nat,
    n: nat,
    k: nat,
    state: ScalarKirStateV1,
)
    ensures
        scalar_kir_projection_step_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@,
            a,
            b,
            p,
            m,
            n,
            k,
            state,
        ) == scalar_kir_step_v1(a, b, p, m, n, k, state),
{
    generated_scalar_kir_projection_decodes_to_exact_ast_v1();
}

pub proof fn generated_scalar_kir_projection_active_refines_integer_model_v1(
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
        scalar_kir_projection_run_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@,
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
    generated_scalar_kir_projection_decodes_to_exact_ast_v1();
    scalar_kir_active_execution_refines_integer_model_v1(a, b, p, m, n, k);
}

pub proof fn generated_scalar_kir_projection_inactive_performs_no_store_v1(
    a: Seq<int>,
    b: Seq<int>,
    p: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires p >= output_count(m, n),
    ensures
        scalar_kir_projection_run_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@,
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
    generated_scalar_kir_projection_decodes_to_exact_ast_v1();
    scalar_kir_inactive_execution_performs_no_store_v1(a, b, p, m, n, k);
}

} // verus!

} // mod scalar_gemm_kir_projection_operational_correspondence_v1
