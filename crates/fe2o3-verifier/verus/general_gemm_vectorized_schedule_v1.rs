use vstd::prelude::*;

#[path = "general_gemm_schedule_model_v1.rs"]
mod model;

verus! {

pub proof fn vectorized_a_only_modeled_global_accesses_are_bounded_v1(
    a: Seq<real>,
    b: Seq<real>,
    c: Seq<real>,
    group_x: nat,
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    k: nat,
    lda: nat,
    ldb: nat,
    ldc: nat,
)
    requires
        model::checked_problem_v1(a, b, c, m, n, k, lda, ldb, ldc),
        lane < 64,
        component < 4,
    ensures
        model::a_load_enabled_v1(group_y, phase, lane, component, m, k) ==>
            model::row_major_index_v1(
                model::a_row_v1(group_y, lane),
                model::phase_depth_v1(phase, lane, component),
                lda,
            ) < a.len(),
        model::b_load_enabled_v1(group_x, phase, lane, component, n, k) ==>
            model::row_major_index_v1(
                model::phase_depth_v1(phase, lane, component),
                model::b_column_v1(group_x, lane),
                ldb,
            ) < b.len(),
        model::output_row_v1(group_y, lane, component) < m
            && model::output_column_v1(group_x, lane) < n ==>
            model::row_major_index_v1(
                model::output_row_v1(group_y, lane, component),
                model::output_column_v1(group_x, lane),
                ldc,
            ) < c.len(),
{
    model::schedule_modeled_global_accesses_are_bounded_v1(
        model::GeneralGemmScheduleModelV1::VectorizedAOnlyBf16GlobalTransfer,
        a, b, c, group_x, group_y, phase, lane, component,
        m, n, k, lda, ldb, ldc,
    );
}

pub proof fn vectorized_a_only_output_region_is_injective_v1(
    left_group_x: nat,
    left_group_y: nat,
    left_lane: nat,
    left_component: nat,
    right_group_x: nat,
    right_group_y: nat,
    right_lane: nat,
    right_component: nat,
)
    requires
        left_lane < 64,
        right_lane < 64,
        left_component < 4,
        right_component < 4,
        model::output_row_v1(left_group_y, left_lane, left_component)
            == model::output_row_v1(right_group_y, right_lane, right_component),
        model::output_column_v1(left_group_x, left_lane)
            == model::output_column_v1(right_group_x, right_lane),
    ensures
        left_group_x == right_group_x,
        left_group_y == right_group_y,
        left_lane == right_lane,
        left_component == right_component,
{
    model::schedule_output_region_injective_v1(
        left_group_x, left_group_y, left_lane, left_component,
        right_group_x, right_group_y, right_lane, right_component,
    );
}

pub proof fn vectorized_accumulator_refines_contract_v1(
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    lda: nat,
    ldb: nat,
    k: nat,
)
    ensures
        model::scheduled_prefix_v1(
            model::GeneralGemmScheduleModelV1::VectorizedAOnlyBf16GlobalTransfer,
            a, b, row, column, lda, ldb, k,
        ) == model::dot_prefix_v1(a, b, row, column, lda, ldb, k),
{
    model::schedule_accumulator_phase_refinement_v1(
        model::GeneralGemmScheduleModelV1::VectorizedAOnlyBf16GlobalTransfer,
        a, b, row, column, lda, ldb, k,
    );
}

fn main() {}

pub proof fn vectorized_full_transfer_and_scalar_tail_refine_v1(
    a: Seq<real>,
    b: Seq<real>,
    group_x: nat,
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    k: nat,
    lda: nat,
    ldb: nat,
)
    requires lane < 64, component < 4,
    ensures
        !model::a_load_enabled_v1(group_y, phase, lane, component, m, k)
            ==> model::a_staged_value_v1(
                a, group_y, phase, lane, component, m, k, lda,
            ) == 0real,
        !model::b_load_enabled_v1(group_x, phase, lane, component, n, k)
            ==> model::b_staged_value_v1(
                b, group_x, phase, lane, component, n, k, ldb,
            ) == 0real,
{
    model::schedule_tail_refinement_v1(
        model::GeneralGemmScheduleModelV1::VectorizedAOnlyBf16GlobalTransfer,
        a, b, group_x, group_y, phase, lane, component, m, n, k, lda, ldb,
    );
}

pub proof fn vectorized_numerical_contract_v1(
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    lda: nat,
    ldb: nat,
    k: nat,
    alpha: real,
    beta: real,
    c: real,
)
    ensures
        model::epilogue_v1(
            alpha,
            model::scheduled_prefix_v1(
                model::GeneralGemmScheduleModelV1::VectorizedAOnlyBf16GlobalTransfer,
                a, b, row, column, lda, ldb, k,
            ),
            beta,
            c,
        ) == alpha * model::dot_prefix_v1(a, b, row, column, lda, ldb, k) + beta * c,
{
    model::schedule_numerical_contract_v1(
        model::GeneralGemmScheduleModelV1::VectorizedAOnlyBf16GlobalTransfer,
        a, b, row, column, lda, ldb, k, alpha, beta, c,
    );
}

pub proof fn vectorized_a_only_epilogue_refines_exact_real_contract_v1(
    alpha: real,
    accumulator: real,
    beta: real,
    c: real,
)
    ensures model::epilogue_v1(alpha, accumulator, beta, c)
        == alpha * accumulator + beta * c,
{
    model::schedule_epilogue_refinement_v1(alpha, accumulator, beta, c);
}

}
