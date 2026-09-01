use vstd::prelude::*;
use vstd::arithmetic::mul::{
    lemma_mul_inequality,
    lemma_mul_is_commutative,
    lemma_mul_unary_negation,
};

verus! {

/// Mathematical semantics of the first `count` terms of an affine form.
pub open spec fn affine_value_prefix(
    constant: int,
    coefficients: Seq<int>,
    point: Seq<int>,
    count: nat,
) -> int
    recommends
        count <= coefficients.len(),
        count <= point.len(),
    decreases count
{
    if count == 0 {
        constant
    } else {
        affine_value_prefix(constant, coefficients, point, (count - 1) as nat)
            + coefficients[(count - 1) as int] * point[(count - 1) as int]
    }
}

/// The full mathematical affine form represented by a V1 query.
pub open spec fn affine_value(
    constant: int,
    coefficients: Seq<int>,
    point: Seq<int>,
) -> int
    recommends coefficients.len() == point.len()
{
    affine_value_prefix(constant, coefficients, point, coefficients.len())
}

pub open spec fn certificate_shapes_match(
    coefficients: Seq<int>,
    lower: Seq<int>,
    upper_exclusive: Seq<int>,
    minimum_coordinates: Seq<int>,
    maximum_coordinates: Seq<int>,
) -> bool {
    coefficients.len() == lower.len()
        && coefficients.len() == upper_exclusive.len()
        && coefficients.len() == minimum_coordinates.len()
        && coefficients.len() == maximum_coordinates.len()
}

pub open spec fn domain_is_nonempty_box(
    lower: Seq<int>,
    upper_exclusive: Seq<int>,
) -> bool {
    forall|i: int| 0 <= i < lower.len() ==>
        lower[i] < upper_exclusive[i]
}

pub open spec fn point_is_in_box(
    point: Seq<int>,
    lower: Seq<int>,
    upper_exclusive: Seq<int>,
) -> bool {
    point.len() == lower.len()
        && forall|i: int| 0 <= i < point.len() ==>
            lower[i] <= #[trigger] point[i] < upper_exclusive[i]
}

pub open spec fn endpoints_follow_coefficient_sign(
    coefficients: Seq<int>,
    lower: Seq<int>,
    upper_exclusive: Seq<int>,
    minimum_coordinates: Seq<int>,
    maximum_coordinates: Seq<int>,
) -> bool {
    forall|i: int| 0 <= i < coefficients.len() ==>
        if coefficients[i] >= 0 {
            minimum_coordinates[i] == lower[i]
                && maximum_coordinates[i] == upper_exclusive[i] - 1
        } else {
            minimum_coordinates[i] == upper_exclusive[i] - 1
                && maximum_coordinates[i] == lower[i]
        }
}

/// Mathematical acceptance predicate implemented by the V1 Rust checker,
/// after its checked-i128 operations have succeeded.
pub open spec fn affine_bounds_checker_accepts(
    constant: int,
    coefficients: Seq<int>,
    lower: Seq<int>,
    upper_exclusive: Seq<int>,
    extent: int,
    minimum_coordinates: Seq<int>,
    maximum_coordinates: Seq<int>,
    claimed_minimum: int,
    claimed_maximum: int,
) -> bool {
    certificate_shapes_match(
        coefficients,
        lower,
        upper_exclusive,
        minimum_coordinates,
        maximum_coordinates,
    )
        && domain_is_nonempty_box(lower, upper_exclusive)
        && extent > 0
        && endpoints_follow_coefficient_sign(
            coefficients,
            lower,
            upper_exclusive,
            minimum_coordinates,
            maximum_coordinates,
        )
        && claimed_minimum == affine_value(constant, coefficients, minimum_coordinates)
        && claimed_maximum == affine_value(constant, coefficients, maximum_coordinates)
        && 0 <= claimed_minimum
        && claimed_maximum < extent
}

proof fn nonnegative_multiplication_preserves_order(coefficient: int, left: int, right: int)
    requires
        0 <= coefficient,
        left <= right,
    ensures
        coefficient * left <= coefficient * right,
{
    lemma_mul_inequality(left, right, coefficient);
    lemma_mul_is_commutative(left, coefficient);
    lemma_mul_is_commutative(right, coefficient);
}

proof fn negative_multiplication_reverses_order(coefficient: int, left: int, right: int)
    requires
        coefficient < 0,
        left <= right,
    ensures
        coefficient * right <= coefficient * left,
{
    lemma_mul_inequality(left, right, -coefficient);
    lemma_mul_unary_negation(left, coefficient);
    lemma_mul_unary_negation(right, coefficient);
    lemma_mul_is_commutative(coefficient, left);
    lemma_mul_is_commutative(coefficient, right);
}

proof fn endpoint_extrema_bound_prefix(
    constant: int,
    coefficients: Seq<int>,
    lower: Seq<int>,
    upper_exclusive: Seq<int>,
    minimum_coordinates: Seq<int>,
    maximum_coordinates: Seq<int>,
    point: Seq<int>,
    count: nat,
)
    requires
        certificate_shapes_match(
            coefficients,
            lower,
            upper_exclusive,
            minimum_coordinates,
            maximum_coordinates,
        ),
        point.len() == coefficients.len(),
        count <= coefficients.len(),
        forall|i: int| 0 <= i < count ==>
            lower[i] < upper_exclusive[i]
                && lower[i] <= point[i] < upper_exclusive[i],
        forall|i: int| 0 <= i < count ==>
            if coefficients[i] >= 0 {
                minimum_coordinates[i] == lower[i]
                    && maximum_coordinates[i] == upper_exclusive[i] - 1
            } else {
                minimum_coordinates[i] == upper_exclusive[i] - 1
                    && maximum_coordinates[i] == lower[i]
            },
    ensures
        affine_value_prefix(constant, coefficients, minimum_coordinates, count)
            <= affine_value_prefix(constant, coefficients, point, count),
        affine_value_prefix(constant, coefficients, point, count)
            <= affine_value_prefix(constant, coefficients, maximum_coordinates, count),
    decreases count
{
    if count > 0 {
        endpoint_extrema_bound_prefix(
            constant,
            coefficients,
            lower,
            upper_exclusive,
            minimum_coordinates,
            maximum_coordinates,
            point,
            (count - 1) as nat,
        );
        let i = (count - 1) as int;
        assert(lower[i] < upper_exclusive[i]);
        assert(lower[i] <= point[i]);
        assert(point[i] < upper_exclusive[i]);
        assert(point[i] <= upper_exclusive[i] - 1);
        let coefficient = coefficients[i];
        let minimum_coordinate = minimum_coordinates[i];
        let maximum_coordinate = maximum_coordinates[i];
        let coordinate = point[i];
        if coefficient >= 0 {
            assert(minimum_coordinates[i] == lower[i]);
            assert(maximum_coordinates[i] == upper_exclusive[i] - 1);
            assert(minimum_coordinate <= coordinate);
            assert(coordinate <= maximum_coordinate);
            nonnegative_multiplication_preserves_order(
                coefficient,
                minimum_coordinate,
                coordinate,
            );
            nonnegative_multiplication_preserves_order(
                coefficient,
                coordinate,
                maximum_coordinate,
            );
        } else {
            assert(minimum_coordinates[i] == upper_exclusive[i] - 1);
            assert(maximum_coordinates[i] == lower[i]);
            assert(coordinate <= minimum_coordinate);
            assert(maximum_coordinate <= coordinate);
            negative_multiplication_reverses_order(
                coefficient,
                coordinate,
                minimum_coordinate,
            );
            negative_multiplication_reverses_order(
                coefficient,
                maximum_coordinate,
                coordinate,
            );
        }
    }
}

/// Soundness theorem for one accepted V1 affine-box certificate.
///
/// For every integer point in the certificate's exact rectangular domain,
/// checker acceptance implies `0 <= f(point) < extent`.
pub proof fn accepted_affine_box_certificate_is_sound(
    constant: int,
    coefficients: Seq<int>,
    lower: Seq<int>,
    upper_exclusive: Seq<int>,
    extent: int,
    minimum_coordinates: Seq<int>,
    maximum_coordinates: Seq<int>,
    claimed_minimum: int,
    claimed_maximum: int,
    point: Seq<int>,
)
    requires
        affine_bounds_checker_accepts(
            constant,
            coefficients,
            lower,
            upper_exclusive,
            extent,
            minimum_coordinates,
            maximum_coordinates,
            claimed_minimum,
            claimed_maximum,
        ),
        point_is_in_box(point, lower, upper_exclusive),
    ensures
        0 <= affine_value(constant, coefficients, point),
        affine_value(constant, coefficients, point) < extent,
{
    endpoint_extrema_bound_prefix(
        constant,
        coefficients,
        lower,
        upper_exclusive,
        minimum_coordinates,
        maximum_coordinates,
        point,
        coefficients.len(),
    );
}

}
