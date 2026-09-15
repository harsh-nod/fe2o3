use vstd::prelude::*;
use vstd::arithmetic::mul::*;
use vstd::arithmetic::div_mod::lemma_fundamental_div_mod;

verus! {
// Arithmetic model only: no emitted-operation or library equivalence is asserted.
// Positive normal exact input x = (n / d) * 2^e. Division in x is rational;
// quotient/remainder below are mathematical integer operations.

pub open spec fn fe2o3_rne_abs_v1(x: int) -> int {
    if x < 0 { -x } else { x }
}

pub open spec fn fe2o3_rne_pow2_v1(k: nat) -> int
    decreases k,
{
    if k == 0 { 1 } else { 2 * fe2o3_rne_pow2_v1((k - 1) as nat) }
}

pub open spec fn fe2o3_rne_scale_num_v1(e: int) -> int {
    if e >= 0 { fe2o3_rne_pow2_v1(e as nat) } else { 1 }
}

pub open spec fn fe2o3_rne_scale_den_v1(e: int) -> int {
    if e >= 0 { 1 } else { fe2o3_rne_pow2_v1((-e) as nat) }
}

pub open spec fn fe2o3_rne_units_v1(n: int, d: int) -> int {
    let q = n / d;
    let r = n % d;
    if 2 * r > d || (2 * r == d && q % 2 != 0) { q + 1 } else { q }
}

pub open spec fn fe2o3_f32_normal_domain_v1(n: int, d: int, e: int) -> bool {
    0 < d
        && 8388608 * d <= n < 16777216 * d
        && -149 <= e <= 104
        && (e == 104 ==> n <= 16777215 * d)
}

pub open spec fn fe2o3_f32_rne_significand_v1(n: int, d: int) -> int {
    let r = fe2o3_rne_units_v1(n, d);
    if r == 16777216 { 8388608 } else { r }
}

pub open spec fn fe2o3_f32_rne_exponent_v1(n: int, d: int, e: int) -> int {
    e + if fe2o3_rne_units_v1(n, d) == 16777216 { 1int } else { 0int }
}

pub open spec fn fe2o3_f32_rne_bits_v1(n: int, d: int, e: int) -> int {
    (fe2o3_f32_rne_exponent_v1(n, d, e) + 150) * 8388608
        + fe2o3_f32_rne_significand_v1(n, d) - 8388608
}

proof fn fe2o3_rne_integer_bound_v1(n: int, d: int)
    requires 0 <= n, 0 < d,
    ensures
        n / d <= fe2o3_rne_units_v1(n, d) <= n / d + 1,
        2 * fe2o3_rne_abs_v1(fe2o3_rne_units_v1(n, d) * d - n) <= d,
        (2 * (n % d) == d ==> fe2o3_rne_units_v1(n, d) % 2 == 0),
        (n % d == 0 ==> fe2o3_rne_units_v1(n, d) * d == n),
{
    lemma_fundamental_div_mod(n, d);
    let q = n / d;
    let r = n % d;
    assert(0 <= q);
    assert(0 <= r < d);
    assert(n == q * d + r) by {
        lemma_mul_is_commutative(d, q);
    }
    if 2 * r > d || (2 * r == d && q % 2 != 0) {
        assert(fe2o3_rne_units_v1(n, d) == q + 1);
        assert((q + 1) * d - n == d - r) by {
        lemma_mul_is_distributive_add_other_way(d, q, 1);
    }
        assert(0 < d - r);
        assert(2 * (d - r) <= d);
        if 2 * r == d {
            assert(q % 2 == 1);
            assert((q + 1) % 2 == 0);
        }
    } else {
        assert(fe2o3_rne_units_v1(n, d) == q);
        assert(q * d - n == -r) by {}
        assert(2 * r <= d);
    }
}

proof fn fe2o3_rne_nearest_integer_v1(n: int, d: int, k: int)
    requires 0 <= n, 0 < d,
    ensures
        fe2o3_rne_abs_v1(fe2o3_rne_units_v1(n, d) * d - n)
            <= fe2o3_rne_abs_v1(k * d - n),
{
    fe2o3_rne_integer_bound_v1(n, d);
    lemma_fundamental_div_mod(n, d);
    let q = n / d;
    let r = n % d;
    let rounded = fe2o3_rne_units_v1(n, d);
    assert(0 <= r < d);
    assert(n == q * d + r) by {
        lemma_mul_is_commutative(d, q);
    }
    if k <= q {
        assert(k * d <= n) by {
        lemma_mul_inequality(k, q, d);
    }
        if rounded == q {
            assert(0 <= n - rounded * d <= n - k * d) by {
        lemma_mul_inequality(k, q, d);
    }
        } else {
            assert(rounded == q + 1);
            assert(2 * r >= d);
            assert(0 <= rounded * d - n <= n - k * d) by {
        lemma_mul_inequality(k, q, d);
        lemma_mul_is_distributive_add_other_way(d, q, 1);
    }
        }
    } else {
        assert(k >= q + 1);
        assert(k * d >= n) by {
        lemma_mul_inequality(q + 1, k, d);
        lemma_mul_is_distributive_add_other_way(d, q, 1);
    }
        if rounded == q {
            assert(2 * r <= d);
            assert(0 <= n - rounded * d <= k * d - n) by {
        lemma_mul_inequality(q + 1, k, d);
        lemma_mul_is_distributive_add_other_way(d, q, 1);
    }
        } else {
            assert(rounded == q + 1);
            assert(0 <= rounded * d - n <= k * d - n) by {
        lemma_mul_inequality(q + 1, k, d);
        lemma_mul_is_distributive_add_other_way(d, q, 1);
    }
        }
    }
}

proof fn fe2o3_rne_pow2_step_v1(k: nat)
    ensures
        0 < fe2o3_rne_pow2_v1(k),
        fe2o3_rne_pow2_v1(k + 1) == 2 * fe2o3_rne_pow2_v1(k),
    decreases k,
{
    if k > 0 {
        fe2o3_rne_pow2_step_v1((k - 1) as nat);
    }
}

proof fn fe2o3_rne_scale_step_v1(e: int)
    ensures
        0 < fe2o3_rne_scale_num_v1(e),
        0 < fe2o3_rne_scale_den_v1(e),
        0 < fe2o3_rne_scale_num_v1(e + 1),
        0 < fe2o3_rne_scale_den_v1(e + 1),
        fe2o3_rne_scale_num_v1(e + 1) * fe2o3_rne_scale_den_v1(e)
            == 2 * fe2o3_rne_scale_num_v1(e) * fe2o3_rne_scale_den_v1(e + 1),
{
    let n = fe2o3_rne_scale_num_v1(e);
    let d = fe2o3_rne_scale_den_v1(e);
    let nn = fe2o3_rne_scale_num_v1(e + 1);
    let dn = fe2o3_rne_scale_den_v1(e + 1);
    if e >= 0 {
        fe2o3_rne_pow2_step_v1(e as nat);
        fe2o3_rne_pow2_step_v1((e + 1) as nat);
        assert(d == 1 && dn == 1 && nn == 2 * n);
    } else if e == -1 {
        fe2o3_rne_pow2_step_v1(0);
        assert(n == 1 && d == 2 && nn == 1 && dn == 1);
    } else {
        fe2o3_rne_pow2_step_v1((-e - 1) as nat);
        fe2o3_rne_pow2_step_v1((-e) as nat);
        assert(n == 1 && nn == 1 && d == 2 * dn);
    }
    assert(nn * d == 2 * n * dn) by {
        lemma_mul_basics(nn);
        lemma_mul_basics(2 * n);
        lemma_mul_basics(d);
        lemma_mul_basics(dn);
        lemma_mul_basics(n);
    }
}

proof fn fe2o3_rne_abs_scale_v1(x: int, s: int)
    requires 0 <= s,
    ensures fe2o3_rne_abs_v1(x * s) == fe2o3_rne_abs_v1(x) * s,
{
    if x >= 0 {
        assert(0 <= x * s) by {
        lemma_mul_nonnegative(x, s);
    }
    } else {
        assert(x * s <= 0) by {
        lemma_mul_inequality(x, 0, s);
    }
        assert(-(x * s) == (-x) * s) by {
        lemma_mul_unary_negation(x, s);
    }
    }
}

// Encodes a positive finite normal word, including a rounded significand carry.
// The significand exponent e corresponds to unbiased binary32 exponent e + 23.
proof fn fe2o3_f32_rne_encoding_v1(n: int, d: int, e: int)
    requires fe2o3_f32_normal_domain_v1(n, d, e),
    ensures
        8388608 <= fe2o3_f32_rne_significand_v1(n, d) < 16777216,
        -149 <= fe2o3_f32_rne_exponent_v1(n, d, e) <= 104,
        8388608 <= fe2o3_f32_rne_bits_v1(n, d, e) <= 2139095039,
        fe2o3_f32_rne_bits_v1(n, d, e) / 8388608
            == fe2o3_f32_rne_exponent_v1(n, d, e) + 150,
        fe2o3_f32_rne_bits_v1(n, d, e) % 8388608
            == fe2o3_f32_rne_significand_v1(n, d) - 8388608,
        fe2o3_f32_rne_bits_v1(n, d, e) % 2 == fe2o3_rne_units_v1(n, d) % 2,
        fe2o3_f32_rne_significand_v1(n, d)
            * fe2o3_rne_scale_num_v1(fe2o3_f32_rne_exponent_v1(n, d, e))
            * fe2o3_rne_scale_den_v1(e)
            == fe2o3_rne_units_v1(n, d) * fe2o3_rne_scale_num_v1(e)
                * fe2o3_rne_scale_den_v1(fe2o3_f32_rne_exponent_v1(n, d, e)),
{
    fe2o3_rne_integer_bound_v1(n, d);
    fe2o3_rne_scale_step_v1(e);
    lemma_fundamental_div_mod(n, d);
    let q = n / d;
    let r = n % d;
    let rounded = fe2o3_rne_units_v1(n, d);
    assert(n == q * d + r) by {
        lemma_mul_is_commutative(d, q);
    }
    assert(0 <= r < d);
    assert(8388608 <= q < 16777216) by {
        if q < 8388608 {
            lemma_mul_inequality(q + 1, 8388608, d);
            lemma_mul_is_distributive_add_other_way(d, q, 1);
            assert(false);
        }
        if q >= 16777216 {
            lemma_mul_inequality(16777216, q, d);
            assert(false);
        }
    }
    assert(8388608 <= rounded <= 16777216);
    if e == 104 {
        if q == 16777215 {
            assert(r == 0) by {}
            assert(rounded == q);
        } else {
            assert(q < 16777215);
            assert(rounded <= 16777215);
        }
    }
    if rounded == 16777216 {
        assert(e < 104);
        assert(fe2o3_f32_rne_significand_v1(n, d) == 8388608);
        assert(fe2o3_f32_rne_exponent_v1(n, d, e) == e + 1);
        assert(
            8388608 * fe2o3_rne_scale_num_v1(e + 1) * fe2o3_rne_scale_den_v1(e)
            == 16777216 * fe2o3_rne_scale_num_v1(e) * fe2o3_rne_scale_den_v1(e + 1)
        ) by {
        broadcast use lemma_mul_is_associative;
    }
    } else {
        assert(fe2o3_f32_rne_exponent_v1(n, d, e) == e);
    }
    let m = fe2o3_f32_rne_significand_v1(n, d);
    let exponent_field = fe2o3_f32_rne_exponent_v1(n, d, e) + 150;
    let bits = fe2o3_f32_rne_bits_v1(n, d, e);
    assert(1 <= exponent_field <= 254);
    assert(0 <= m - 8388608 < 8388608);
    assert(bits == exponent_field * 8388608 + m - 8388608);
    assert(bits / 8388608 == exponent_field);
    assert(bits % 8388608 == m - 8388608);
    assert(bits % 2 == m % 2);
    assert(m % 2 == rounded % 2);
}

// Exact rational result: decoded m*2^out_e differs from (n/d)*2^e by at most
// 2^e/2. The inequality is cross-multiplied only by proved-positive denominators.
pub proof fn fe2o3_f32_rne_local_error_v1(n: int, d: int, e: int)
    requires fe2o3_f32_normal_domain_v1(n, d, e),
    ensures
        8388608 <= fe2o3_f32_rne_bits_v1(n, d, e) <= 2139095039,
        fe2o3_f32_rne_bits_v1(n, d, e) / 8388608
            == fe2o3_f32_rne_exponent_v1(n, d, e) + 150,
        fe2o3_f32_rne_bits_v1(n, d, e) % 8388608
            == fe2o3_f32_rne_significand_v1(n, d) - 8388608,
        0 < fe2o3_rne_scale_den_v1(e),
        0 < fe2o3_rne_scale_den_v1(fe2o3_f32_rne_exponent_v1(n, d, e)),
        2 * fe2o3_rne_abs_v1(
            fe2o3_f32_rne_significand_v1(n, d)
                * fe2o3_rne_scale_num_v1(fe2o3_f32_rne_exponent_v1(n, d, e))
                * d * fe2o3_rne_scale_den_v1(e)
            - n * fe2o3_rne_scale_num_v1(e)
                * fe2o3_rne_scale_den_v1(fe2o3_f32_rne_exponent_v1(n, d, e)))
            <= d * fe2o3_rne_scale_num_v1(e)
                * fe2o3_rne_scale_den_v1(fe2o3_f32_rne_exponent_v1(n, d, e)),
        (2 * (n % d) == d ==> fe2o3_rne_units_v1(n, d) % 2 == 0),
        (2 * (n % d) == d ==> fe2o3_f32_rne_bits_v1(n, d, e) % 2 == 0),
        forall|k: int| fe2o3_rne_abs_v1(fe2o3_rne_units_v1(n, d) * d - n)
            <= #[trigger] fe2o3_rne_abs_v1(k * d - n),
{
    fe2o3_rne_integer_bound_v1(n, d);
    fe2o3_f32_rne_encoding_v1(n, d, e);
    fe2o3_rne_scale_step_v1(e);
    let rounded = fe2o3_rne_units_v1(n, d);
    let m = fe2o3_f32_rne_significand_v1(n, d);
    let out_e = fe2o3_f32_rne_exponent_v1(n, d, e);
    fe2o3_rne_scale_step_v1(out_e);
    let sn = fe2o3_rne_scale_num_v1(e);
    let sd = fe2o3_rne_scale_den_v1(e);
    let on = fe2o3_rne_scale_num_v1(out_e);
    let od = fe2o3_rne_scale_den_v1(out_e);
    assert(0 < sn * od) by {
        lemma_mul_strictly_positive(sn, od);
    }
    assert(m * on * d * sd == (m * on * sd) * d) by {
        lemma_mul_is_associative(m * on, d, sd);
        lemma_mul_is_commutative(d, sd);
        lemma_mul_is_associative(m * on, sd, d);
    }
    assert((m * on * sd) * d == (rounded * sn * od) * d);
    assert((rounded * sn * od) * d - n * sn * od
        == (rounded * d - n) * (sn * od)) by {
        lemma_mul_is_associative(rounded, sn, od);
        lemma_mul_is_associative(rounded, sn * od, d);
        lemma_mul_is_commutative(sn * od, d);
        lemma_mul_is_associative(rounded, d, sn * od);
        lemma_mul_is_distributive_sub_other_way(sn * od, rounded * d, n);
        lemma_mul_is_associative(n, sn, od);
    }
    assert(m * on * d * sd - n * sn * od == (rounded * d - n) * (sn * od));
    fe2o3_rne_abs_scale_v1(rounded * d - n, sn * od);
    assert(2 * fe2o3_rne_abs_v1(rounded * d - n) * (sn * od) <= d * sn * od)
        by {
        lemma_mul_inequality(2 * fe2o3_rne_abs_v1(rounded * d - n), d, sn * od);
        lemma_mul_is_associative(d, sn, od);
    }
    let scaled_error = fe2o3_rne_abs_v1(m * on * d * sd - n * sn * od);
    let unit_error = fe2o3_rne_abs_v1(rounded * d - n);
    assert(scaled_error == unit_error * (sn * od));
    assert(2 * scaled_error == 2 * unit_error * (sn * od)) by {
        lemma_mul_is_associative(2, unit_error, sn * od);
    }
    assert(2 * scaled_error <= d * sn * od);
    assert forall|k: int| fe2o3_rne_abs_v1(fe2o3_rne_units_v1(n, d) * d - n)
        <= #[trigger] fe2o3_rne_abs_v1(k * d - n) by {
        fe2o3_rne_nearest_integer_v1(n, d, k);
    }
}

// Concrete inhabitants of the domain, including nonzero-error ties. These are
// arithmetic proof goals, not assumed input-domain evidence for a kernel.
proof fn fe2o3_f32_rne_nonvacuous_examples_v1() {
    fe2o3_f32_rne_local_error_v1(16777217, 2, -23);
    assert(fe2o3_f32_rne_bits_v1(16777217, 2, -23) == 1065353216);
    assert(fe2o3_rne_units_v1(16777217, 2) * 2 != 16777217);
    fe2o3_f32_rne_local_error_v1(16777219, 2, -23);
    assert(fe2o3_f32_rne_bits_v1(16777219, 2, -23) == 1065353218);
    fe2o3_f32_rne_local_error_v1(33554431, 2, -23);
    assert(fe2o3_f32_rne_bits_v1(33554431, 2, -23) == 1073741824);
    fe2o3_f32_rne_local_error_v1(8388608, 1, -149);
    assert(fe2o3_f32_rne_bits_v1(8388608, 1, -149) == 8388608);
    fe2o3_f32_rne_local_error_v1(16777215, 1, 104);
    assert(fe2o3_f32_rne_bits_v1(16777215, 1, 104) == 2139095039);
}

// Compare the rounding model with every finite binary32 encoding.
// A decoded tick is exactly 2^-149. Negative words and both zeros are included
// as competitors; the exact input domain remains positive finite normals.

pub open spec fn fe2o3_f32_finite_word_v1(w: int) -> bool {
    0 <= w < 4294967296 && w % 2147483648 < 2139095040
}

pub open spec fn fe2o3_f32_magnitude_ticks_v1(m: int) -> int {
    if m / 8388608 == 0 { m } else {
        (8388608 + m % 8388608) * fe2o3_rne_pow2_v1((m / 8388608 - 1) as nat)
    }
}

pub open spec fn fe2o3_f32_decoded_ticks_v1(w: int) -> int {
    let magnitude = fe2o3_f32_magnitude_ticks_v1(w % 2147483648);
    if w < 2147483648 { magnitude } else { -magnitude }
}

pub open spec fn fe2o3_f32_tick_spacing_v1(e: int) -> int {
    fe2o3_rne_pow2_v1((e + 149) as nat)
}

pub open spec fn fe2o3_f32_tick_distance_v1(n: int, d: int, e: int, w: int) -> int {
    fe2o3_rne_abs_v1(fe2o3_f32_decoded_ticks_v1(w) * d
        - n * fe2o3_f32_tick_spacing_v1(e))
}

pub open spec fn fe2o3_f32_best_word_v1(n: int, d: int, e: int, b: int, w: int) -> bool {
    fe2o3_f32_tick_distance_v1(n, d, e, b) <= fe2o3_f32_tick_distance_v1(n, d, e, w)
        && ((w != b && fe2o3_f32_tick_distance_v1(n, d, e, b)
            == fe2o3_f32_tick_distance_v1(n, d, e, w)) ==> b % 2 == 0 && w % 2 == 1)
}

// These power facts connect the decoded integer coordinate to the dyadic
// numerator/denominator definition; they do not assert floating-point accuracy.
proof fn fe2o3_bridge_pow2_add_v1(a: nat, b: nat)
    ensures fe2o3_rne_pow2_v1(a + b) == fe2o3_rne_pow2_v1(a) * fe2o3_rne_pow2_v1(b),
    decreases b,
{
    if b > 0 {
        fe2o3_bridge_pow2_add_v1(a, (b - 1) as nat);
        fe2o3_rne_pow2_step_v1(a + (b - 1) as nat);
        fe2o3_rne_pow2_step_v1((b - 1) as nat);
        assert(fe2o3_rne_pow2_v1(a + b)
            == fe2o3_rne_pow2_v1(a) * fe2o3_rne_pow2_v1(b)) by {
        lemma_mul_is_associative(fe2o3_rne_pow2_v1(a), 2, fe2o3_rne_pow2_v1((b - 1) as nat));
        lemma_mul_is_commutative(fe2o3_rne_pow2_v1(a), 2);
        lemma_mul_is_associative(2, fe2o3_rne_pow2_v1(a), fe2o3_rne_pow2_v1((b - 1) as nat));
    }
    }
}

proof fn fe2o3_bridge_pow2_order_v1(a: nat, b: nat)
    requires a <= b,
    ensures 0 < fe2o3_rne_pow2_v1(a) <= fe2o3_rne_pow2_v1(b),
    decreases b,
{
    fe2o3_rne_pow2_step_v1(a);
    if a < b {
        fe2o3_bridge_pow2_order_v1(a, (b - 1) as nat);
        fe2o3_rne_pow2_step_v1((b - 1) as nat);
    }
}

proof fn fe2o3_bridge_scale_v1(e: int)
    requires -149 <= e <= 104,
    ensures
        0 < fe2o3_f32_tick_spacing_v1(e),
        0 < fe2o3_rne_pow2_v1(149),
        fe2o3_f32_tick_spacing_v1(e) * fe2o3_rne_scale_den_v1(e)
            == fe2o3_rne_scale_num_v1(e) * fe2o3_rne_pow2_v1(149),
{
    fe2o3_rne_pow2_step_v1((e + 149) as nat);
    fe2o3_rne_pow2_step_v1(149);
    if e >= 0 {
        fe2o3_bridge_pow2_add_v1(149, e as nat);
        assert(fe2o3_f32_tick_spacing_v1(e) * fe2o3_rne_scale_den_v1(e)
            == fe2o3_rne_scale_num_v1(e) * fe2o3_rne_pow2_v1(149)) by {
        lemma_mul_is_commutative(fe2o3_rne_scale_num_v1(e), fe2o3_rne_pow2_v1(149));
    }
    } else {
        fe2o3_bridge_pow2_add_v1((e + 149) as nat, (-e) as nat);
        assert(fe2o3_f32_tick_spacing_v1(e) * fe2o3_rne_scale_den_v1(e)
            == fe2o3_rne_scale_num_v1(e) * fe2o3_rne_pow2_v1(149)) by {}
    }
}

// Strict positive-word order includes zero/subnormals and every binade boundary.
proof fn fe2o3_f32_magnitude_order_v1(a: int, b: int)
    requires 0 <= a < b < 2139095040,
    ensures fe2o3_f32_magnitude_ticks_v1(a) < fe2o3_f32_magnitude_ticks_v1(b),
{
    let ae = a / 8388608;
    let be = b / 8388608;
    let af = a % 8388608;
    let bf = b % 8388608;
    assert(a == ae * 8388608 + af && b == be * 8388608 + bf);
    assert(0 <= af < 8388608 && 0 <= bf < 8388608);
    assert(0 <= ae <= be <= 254);
    let av = fe2o3_f32_magnitude_ticks_v1(a);
    let bv = fe2o3_f32_magnitude_ticks_v1(b);
    let ap = fe2o3_rne_pow2_v1((ae - 1) as nat);
    let bp = fe2o3_rne_pow2_v1((be - 1) as nat);
    if ae == be {
        assert(af < bf);
        if ae > 0 {
            fe2o3_rne_pow2_step_v1((ae - 1) as nat);
            assert(av < bv) by {
        lemma_mul_strict_inequality(8388608 + af, 8388608 + bf, ap);
    }
        } else {
            assert(av == a && bv == b);
        }
    } else {
        assert(ae < be);
        fe2o3_rne_pow2_step_v1((be - 1) as nat);
        if ae > 0 {
            fe2o3_rne_pow2_step_v1((ae - 1) as nat);
            fe2o3_bridge_pow2_order_v1(ae as nat, (be - 1) as nat);
            assert(av < bv) by {
        lemma_mul_strict_inequality(8388608 + af, 16777216, ap);
        lemma_mul_inequality(2 * ap, bp, 8388608);
        lemma_mul_inequality(8388608, 8388608 + bf, bp);
    }
        } else {
            assert(av < bv) by {
        lemma_mul_inequality(1, bp, 8388608);
        lemma_mul_inequality(8388608, 8388608 + bf, bp);
    }
        }
    }
}

// The upper endpoint is included only when finite. Word arithmetic is linear
// across this one carry: lo + (2^24 - 2^23) is the next binade's zero fraction.
proof fn fe2o3_f32_binade_lattice_v1(e: int, w: int)
    requires
        -149 <= e <= 104,
        fe2o3_f32_finite_word_v1(w),
        (e + 150) * 8388608 <= w <= (e + 151) * 8388608,
    ensures
        fe2o3_f32_decoded_ticks_v1(w)
            == (8388608 + w - (e + 150) * 8388608) * fe2o3_f32_tick_spacing_v1(e),
        w % 2 == (8388608 + w - (e + 150) * 8388608) % 2,
{
    let lo = (e + 150) * 8388608;
    assert(0 <= w < 2147483648);
    assert(w % 2147483648 == w);
    if w < lo + 8388608 {
        assert(w / 8388608 == e + 150);
        assert(w % 8388608 == w - lo);
    } else {
        assert(w == lo + 8388608);
        assert(e < 104);
        assert(w / 8388608 == e + 151 && w % 8388608 == 0);
        fe2o3_rne_pow2_step_v1((e + 149) as nat);
        assert(fe2o3_f32_decoded_ticks_v1(w) == 16777216 * fe2o3_f32_tick_spacing_v1(e))
            by {}
    }
    assert(w % 2 == (8388608 + w - lo) % 2);
}

// Equality of distinct lattice distances forces a true half-way case, not just
// the one-way midpoint implication supplied by the local-error lemma.
proof fn fe2o3_rne_distinct_equal_distance_v1(n: int, d: int, k: int)
    requires
        0 <= n, 0 < d, k != fe2o3_rne_units_v1(n, d),
        fe2o3_rne_abs_v1(k * d - n)
            == fe2o3_rne_abs_v1(fe2o3_rne_units_v1(n, d) * d - n),
    ensures
        2 * (n % d) == d,
        fe2o3_rne_units_v1(n, d) % 2 == 0,
        k % 2 == 1,
{
    fe2o3_rne_integer_bound_v1(n, d);
    lemma_fundamental_div_mod(n, d);
    let rounded = fe2o3_rne_units_v1(n, d);
    let a = rounded * d - n;
    let b = k * d - n;
    assert(a != b) by {
        if rounded < k { lemma_mul_strict_inequality(rounded, k, d); }
        else { lemma_mul_strict_inequality(k, rounded, d); }
    }
    if a >= 0 {
        if b >= 0 { assert(a == b); assert(false); }
    } else {
        if b < 0 { assert(a == b); assert(false); }
    }
    assert(a == -b);
    assert((rounded + k) * d == 2 * n) by {
        lemma_mul_is_distributive_add_other_way(d, rounded, k);
    }
    let q = n / d;
    let r = n % d;
    assert(n == q * d + r) by {
        lemma_mul_is_commutative(d, q);
    }
    assert(0 <= r < d);
    assert((rounded + k - 2 * q) * d == 2 * r) by {
        lemma_mul_is_distributive_sub_other_way(d, rounded + k, 2 * q);
        lemma_mul_is_associative(2, q, d);
    }
    assert(0 <= rounded + k - 2 * q < 2) by {
        let t = rounded + k - 2 * q;
        if t < 0 { lemma_mul_strict_inequality(t, 0, d); assert(false); }
        if t >= 2 { lemma_mul_inequality(2, t, d); assert(false); }
    }
    if rounded + k - 2 * q == 0 {
        assert(r == 0) by {}
        assert(rounded == q);
        assert(k == rounded);
        assert(false);
    }
    assert(rounded + k == 2 * q + 1);
    assert(2 * r == d) by {}
    fe2o3_rne_integer_bound_v1(n, d);
    assert(k % 2 == 1);
}

proof fn fe2o3_rne_scaled_distance_v1(n: int, d: int, k: int, s: int, decoded: int)
    requires decoded == k * s, 0 <= s,
    ensures fe2o3_rne_abs_v1(decoded * d - n * s) == fe2o3_rne_abs_v1(k * d - n) * s,
{
    assert(decoded * d == (k * s) * d);
    assert((k * s) * d - n * s == (k * d - n) * s) by {
        lemma_mul_is_associative(k, s, d);
        lemma_mul_is_commutative(s, d);
        lemma_mul_is_associative(k, d, s);
        lemma_mul_is_distributive_sub_other_way(s, k * d, n);
    }
    fe2o3_rne_abs_scale_v1(k * d - n, s);
}

proof fn fe2o3_f32_compare_finite_v1(n: int, d: int, e: int, w: int)
    requires fe2o3_f32_normal_domain_v1(n, d, e), fe2o3_f32_finite_word_v1(w),
    ensures fe2o3_f32_best_word_v1(n, d, e, fe2o3_f32_rne_bits_v1(n, d, e), w),
{
    fe2o3_f32_rne_encoding_v1(n, d, e);
    fe2o3_bridge_scale_v1(e);
    let r = fe2o3_rne_units_v1(n, d);
    let b = fe2o3_f32_rne_bits_v1(n, d, e);
    let lo = (e + 150) * 8388608;
    let hi = lo + 8388608;
    let s = fe2o3_f32_tick_spacing_v1(e);
    assert(8388608 <= r <= 16777216);
    assert(b == lo + r - 8388608);
    assert(fe2o3_f32_finite_word_v1(b));
    fe2o3_f32_binade_lattice_v1(e, b);
    fe2o3_rne_scaled_distance_v1(n, d, r, s, fe2o3_f32_decoded_ticks_v1(b));
    assert(fe2o3_f32_tick_distance_v1(n, d, e, b)
        == fe2o3_rne_abs_v1(r * d - n) * s);
    let db = fe2o3_f32_tick_distance_v1(n, d, e, b);
    let dw = fe2o3_f32_tick_distance_v1(n, d, e, w);
    let ar = fe2o3_rne_abs_v1(r * d - n);
    let y = fe2o3_f32_decoded_ticks_v1(w);
    if w >= 2147483648 || w < lo {
        fe2o3_f32_binade_lattice_v1(e, lo);
        if w >= 2147483648 {
            let m = w % 2147483648;
            if m > 0 { fe2o3_f32_magnitude_order_v1(0, m); }
            assert(y <= 0);
        } else {
            fe2o3_f32_magnitude_order_v1(w, lo);
        }
        assert(y < 8388608 * s);
        fe2o3_rne_nearest_integer_v1(n, d, 8388608);
        fe2o3_rne_abs_scale_v1(8388608 * d - n, s);
        assert(fe2o3_rne_abs_v1(8388608 * d - n) == n - 8388608 * d);
        assert(y * d - n * s < 0) by {
        lemma_mul_strict_inequality(y, 8388608 * s, d);
        lemma_mul_inequality(8388608 * d, n, s);
        lemma_mul_is_associative(8388608, s, d);
        lemma_mul_is_commutative(s, d);
        lemma_mul_is_associative(8388608, d, s);
    }
        assert(dw == n * s - y * d);
        assert(db < dw) by {
        lemma_mul_inequality(ar, n - 8388608 * d, s);
        lemma_mul_is_distributive_sub_other_way(s, n, 8388608 * d);
        lemma_mul_strict_inequality(y, 8388608 * s, d);
        lemma_mul_is_associative(8388608, s, d);
        lemma_mul_is_commutative(s, d);
        lemma_mul_is_associative(8388608, d, s);
    }
    } else if w > hi {
        assert(e < 104 && fe2o3_f32_finite_word_v1(hi));
        fe2o3_f32_binade_lattice_v1(e, hi);
        fe2o3_f32_magnitude_order_v1(hi, w);
        assert(y > 16777216 * s);
        fe2o3_rne_nearest_integer_v1(n, d, 16777216);
        fe2o3_rne_abs_scale_v1(16777216 * d - n, s);
        assert(fe2o3_rne_abs_v1(16777216 * d - n) == 16777216 * d - n);
        assert(y * d - n * s > 0) by {
        lemma_mul_strict_inequality(16777216 * s, y, d);
        lemma_mul_strict_inequality(n, 16777216 * d, s);
        lemma_mul_is_associative(16777216, s, d);
        lemma_mul_is_commutative(s, d);
        lemma_mul_is_associative(16777216, d, s);
    }
        assert(dw == y * d - n * s);
        assert(db < dw) by {
        lemma_mul_inequality(ar, 16777216 * d - n, s);
        lemma_mul_is_distributive_sub_other_way(s, 16777216 * d, n);
        lemma_mul_strict_inequality(16777216 * s, y, d);
        lemma_mul_is_associative(16777216, s, d);
        lemma_mul_is_commutative(s, d);
        lemma_mul_is_associative(16777216, d, s);
    }
    } else {
        let k = 8388608 + w - lo;
        fe2o3_f32_binade_lattice_v1(e, w);
        fe2o3_rne_nearest_integer_v1(n, d, k);
        fe2o3_rne_scaled_distance_v1(n, d, k, s, y);
        let ak = fe2o3_rne_abs_v1(k * d - n);
        assert(dw == ak * s);
        assert(db <= dw) by {
        lemma_mul_inequality(ar, ak, s);
    }
        if w != b && fe2o3_f32_tick_distance_v1(n, d, e, b)
            == fe2o3_f32_tick_distance_v1(n, d, e, w)
        {
            assert(k != r);
            assert(ak == ar) by {
        lemma_mul_is_commutative(s, ak);
        lemma_mul_is_commutative(s, ar);
        lemma_mul_equality_converse(s, ak, ar);
    }
            assert(fe2o3_rne_abs_v1(k * d - n) == fe2o3_rne_abs_v1(r * d - n));
            fe2o3_rne_distinct_equal_distance_v1(n, d, k);
            assert(b % 2 == 0 && w % 2 == 1);
        }
    }
}

pub proof fn fe2o3_f32_rne_all_finite_v1(n: int, d: int, e: int)
    requires fe2o3_f32_normal_domain_v1(n, d, e),
    ensures
        fe2o3_f32_finite_word_v1(fe2o3_f32_rne_bits_v1(n, d, e)),
        fe2o3_f32_tick_spacing_v1(e) * fe2o3_rne_scale_den_v1(e)
            == fe2o3_rne_scale_num_v1(e) * fe2o3_rne_pow2_v1(149),
        forall|w: int| fe2o3_f32_finite_word_v1(w) ==>
            fe2o3_f32_best_word_v1(n, d, e, fe2o3_f32_rne_bits_v1(n, d, e), w),
{
    fe2o3_f32_rne_encoding_v1(n, d, e);
    fe2o3_bridge_scale_v1(e);
    assert forall|w: int| fe2o3_f32_finite_word_v1(w) implies
        fe2o3_f32_best_word_v1(n, d, e, fe2o3_f32_rne_bits_v1(n, d, e), w) by {
        fe2o3_f32_compare_finite_v1(n, d, e, w);
    }
}

proof fn fe2o3_f32_bridge_inhabitants_v1() {
    fe2o3_f32_rne_all_finite_v1(8388608, 1, -149);
    fe2o3_f32_rne_all_finite_v1(16777215, 1, 104);
    fe2o3_f32_rne_all_finite_v1(16777217, 2, -23);
    fe2o3_f32_rne_all_finite_v1(16777219, 2, -23);
    fe2o3_f32_rne_all_finite_v1(33554431, 2, -23);
}

}

fn main() {}
