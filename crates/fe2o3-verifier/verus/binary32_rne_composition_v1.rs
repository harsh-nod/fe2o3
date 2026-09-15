// Proof-only companion: concatenate after the unchanged binary32_rne_v1.rs.
// A=a/k and R=rho/k are exact rationals, not machine floating-point operations.
verus! {

pub open spec fn fe2o3_ar_lower_v1(target: int, reference_error: int) -> int {
    if fe2o3_rne_abs_v1(target) > reference_error {
        fe2o3_rne_abs_v1(target) - reference_error
    } else { 0 }
}

pub open spec fn fe2o3_ar_sufficient_v1(
    target: int, denominator: int, gpu_error: int, reference_error: int,
    a: int, rho: int, k: int,
) -> bool {
    0 < denominator && 0 < k && 0 <= a && 0 <= rho
        && 0 <= gpu_error && 0 <= reference_error
        && (gpu_error + reference_error) * k
            <= a * denominator + fe2o3_ar_lower_v1(target, reference_error) * rho
}

pub open spec fn fe2o3_ar_goal_v1(
    gpu: int, reference: int, denominator: int, a: int, rho: int, k: int,
) -> bool {
    fe2o3_rne_abs_v1(gpu - reference) * k
        <= a * denominator + fe2o3_rne_abs_v1(reference) * rho
}

proof fn fe2o3_ar_triangle_v1(x: int, y: int)
    ensures fe2o3_rne_abs_v1(x + y) <= fe2o3_rne_abs_v1(x) + fe2o3_rne_abs_v1(y),
{
    if x < 0 { } else { }
    if y < 0 { } else { }
    if x + y < 0 { } else { }
}

// G/D and Ref/D may be independent results. The premises are their separate
// error theorems around T/D and a result-independent sufficient A/R condition.
pub proof fn fe2o3_two_result_ar_composition_v1(
    gpu: int, reference: int, target: int, denominator: int,
    gpu_error: int, reference_error: int, a: int, rho: int, k: int,
)
    requires
        fe2o3_rne_abs_v1(gpu - target) <= gpu_error,
        fe2o3_rne_abs_v1(reference - target) <= reference_error,
        fe2o3_ar_sufficient_v1(target, denominator, gpu_error, reference_error, a, rho, k),
    ensures fe2o3_ar_goal_v1(gpu, reference, denominator, a, rho, k),
{
    fe2o3_ar_triangle_v1(gpu - target, target - reference);
    fe2o3_ar_triangle_v1(reference, target - reference);
    assert(fe2o3_rne_abs_v1(target - reference)
        == fe2o3_rne_abs_v1(reference - target));
    assert(fe2o3_rne_abs_v1(gpu - reference) <= gpu_error + reference_error);
    assert(fe2o3_ar_lower_v1(target, reference_error) <= fe2o3_rne_abs_v1(reference));
    lemma_mul_inequality(fe2o3_rne_abs_v1(gpu - reference), gpu_error + reference_error, k);
    lemma_mul_inequality(
        fe2o3_ar_lower_v1(target, reference_error), fe2o3_rne_abs_v1(reference), rho,
    );
}

// A shared integer denominator carries exact powers of two, including negative
// exponents. Multiplication here is unbounded spec arithmetic, not emitted code.
pub open spec fn fe2o3_rne_composed_den_v1(d: int, e: int) -> int {
    (2 * d) * fe2o3_rne_scale_den_v1(e)
}

pub open spec fn fe2o3_rne_composed_num_v1(n: int, d: int, e: int) -> int {
    (2 * (fe2o3_rne_units_v1(n, d) * d)) * fe2o3_rne_scale_num_v1(e)
}

pub open spec fn fe2o3_rne_composed_target_v1(t: int, e: int) -> int {
    (2 * t) * fe2o3_rne_scale_num_v1(e)
}

pub open spec fn fe2o3_rne_composed_error_v1(d: int, e: int, input_error: int) -> int {
    (d + 2 * input_error) * fe2o3_rne_scale_num_v1(e)
}

// input_error bounds |n-t|, not |rounded-t| and not the requested output bound.
// The half-ULP contribution is proved by the existing positive-normal model.
proof fn fe2o3_rne_error_to_common_target_v1(n: int, d: int, e: int, t: int, input_error: int)
    requires
        fe2o3_f32_normal_domain_v1(n, d, e),
        0 <= input_error,
        fe2o3_rne_abs_v1(n - t) <= input_error,
    ensures
        0 < fe2o3_rne_composed_den_v1(d, e),
        0 <= fe2o3_rne_composed_error_v1(d, e, input_error),
        fe2o3_rne_abs_v1(fe2o3_rne_composed_num_v1(n, d, e)
            - fe2o3_rne_composed_target_v1(t, e))
            <= fe2o3_rne_composed_error_v1(d, e, input_error),
        8388608 <= fe2o3_f32_rne_bits_v1(n, d, e) <= 2139095039,
        fe2o3_f32_rne_significand_v1(n, d)
            * fe2o3_rne_scale_num_v1(fe2o3_f32_rne_exponent_v1(n, d, e))
            * fe2o3_rne_scale_den_v1(e)
            == fe2o3_rne_units_v1(n, d) * fe2o3_rne_scale_num_v1(e)
                * fe2o3_rne_scale_den_v1(fe2o3_f32_rne_exponent_v1(n, d, e)),
{
    fe2o3_f32_rne_local_error_v1(n, d, e);
    fe2o3_rne_integer_bound_v1(n, d);
    fe2o3_f32_rne_encoding_v1(n, d, e);
    fe2o3_rne_scale_step_v1(e);
    let rounded = fe2o3_rne_units_v1(n, d);
    let x = rounded * d;
    let sn = fe2o3_rne_scale_num_v1(e);
    let sd = fe2o3_rne_scale_den_v1(e);
    fe2o3_ar_triangle_v1(x - n, n - t);
    assert(2 * fe2o3_rne_abs_v1(x - t) <= d + 2 * input_error);
    lemma_mul_strictly_positive(2 * d, sd);
    lemma_mul_nonnegative(d + 2 * input_error, sn);
    lemma_mul_is_distributive_sub_other_way(sn, 2 * x, 2 * t);
    assert((2 * x) * sn - (2 * t) * sn == (2 * (x - t)) * sn);
    fe2o3_rne_abs_scale_v1(2 * (x - t), sn);
    assert(fe2o3_rne_abs_v1(2 * (x - t)) == 2 * fe2o3_rne_abs_v1(x - t));
    lemma_mul_inequality(2 * fe2o3_rne_abs_v1(x - t), d + 2 * input_error, sn);
}

// First binary32 adapter: both exact pre-round inputs share d and e but need
// not equal the common target t. All original normal/overflow domains remain.
pub proof fn fe2o3_f32_two_rounded_ar_v1(
    ng: int, nr: int, d: int, e: int, target: int,
    bg: int, br: int, a: int, rho: int, k: int,
)
    requires
        fe2o3_f32_normal_domain_v1(ng, d, e),
        fe2o3_f32_normal_domain_v1(nr, d, e),
        0 <= bg, 0 <= br,
        fe2o3_rne_abs_v1(ng - target) <= bg,
        fe2o3_rne_abs_v1(nr - target) <= br,
        fe2o3_ar_sufficient_v1(
            fe2o3_rne_composed_target_v1(target, e), fe2o3_rne_composed_den_v1(d, e),
            fe2o3_rne_composed_error_v1(d, e, bg), fe2o3_rne_composed_error_v1(d, e, br),
            a, rho, k,
        ),
    ensures
        fe2o3_ar_goal_v1(
            fe2o3_rne_composed_num_v1(ng, d, e), fe2o3_rne_composed_num_v1(nr, d, e),
            fe2o3_rne_composed_den_v1(d, e), a, rho, k,
        ),
        0 < fe2o3_rne_composed_den_v1(d, e),
        8388608 <= fe2o3_f32_rne_bits_v1(ng, d, e) <= 2139095039,
        8388608 <= fe2o3_f32_rne_bits_v1(nr, d, e) <= 2139095039,
        fe2o3_f32_rne_significand_v1(ng, d)
            * fe2o3_rne_scale_num_v1(fe2o3_f32_rne_exponent_v1(ng, d, e))
            * fe2o3_rne_scale_den_v1(e)
            == fe2o3_rne_units_v1(ng, d) * fe2o3_rne_scale_num_v1(e)
                * fe2o3_rne_scale_den_v1(fe2o3_f32_rne_exponent_v1(ng, d, e)),
        fe2o3_f32_rne_significand_v1(nr, d)
            * fe2o3_rne_scale_num_v1(fe2o3_f32_rne_exponent_v1(nr, d, e))
            * fe2o3_rne_scale_den_v1(e)
            == fe2o3_rne_units_v1(nr, d) * fe2o3_rne_scale_num_v1(e)
                * fe2o3_rne_scale_den_v1(fe2o3_f32_rne_exponent_v1(nr, d, e)),
{
    fe2o3_rne_error_to_common_target_v1(ng, d, e, target, bg);
    fe2o3_rne_error_to_common_target_v1(nr, d, e, target, br);
    fe2o3_two_result_ar_composition_v1(
        fe2o3_rne_composed_num_v1(ng, d, e), fe2o3_rne_composed_num_v1(nr, d, e),
        fe2o3_rne_composed_target_v1(target, e), fe2o3_rne_composed_den_v1(d, e),
        fe2o3_rne_composed_error_v1(d, e, bg), fe2o3_rne_composed_error_v1(d, e, br),
        a, rho, k,
    );
}

// G=1, Ref=1+2^-22, common target=1+2^-23. Each pre-round
// input differs from the common target by 2^-24, proved as an exact integer fact.
proof fn fe2o3_two_rounded_nonzero_example_v1()
    ensures
        fe2o3_f32_rne_bits_v1(16777217, 2, -23) == 1065353216,
        fe2o3_f32_rne_bits_v1(16777219, 2, -23) == 1065353218,
        fe2o3_ar_goal_v1(33554432, 33554440, 33554432, 1, 1, 8388608),
        !fe2o3_ar_goal_v1(33554432, 33554440, 33554432, 1, 1, 16777216),
{
    // Fixed-depth SMT unfolding avoids the separate compute interpreter.
    assert(fe2o3_rne_pow2_v1(23) == 8388608) by {
        reveal_with_fuel(fe2o3_rne_pow2_v1, 24);
    }
    assert(fe2o3_rne_scale_den_v1(-23) == 8388608);
    assert(fe2o3_rne_scale_num_v1(-23) == 1);
    assert(fe2o3_rne_composed_den_v1(2, -23) == 33554432);
    assert(fe2o3_rne_composed_target_v1(16777218, -23) == 33554436);
    assert(fe2o3_rne_composed_error_v1(2, -23, 1) == 4);
    fe2o3_f32_two_rounded_ar_v1(16777217, 16777219, 2, -23, 16777218, 1, 1, 1, 1, 8388608);
    assert(fe2o3_rne_composed_num_v1(16777217, 2, -23) == 33554432);
    assert(fe2o3_rne_composed_num_v1(16777219, 2, -23) == 33554440);
    assert(fe2o3_rne_composed_den_v1(2, -23) == 33554432);
}

}
