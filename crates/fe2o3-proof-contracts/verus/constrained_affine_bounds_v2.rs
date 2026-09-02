use vstd::arithmetic::mul::{
    lemma_mul_inequality,
    lemma_mul_is_associative,
    lemma_mul_is_distributive_add,
    lemma_mul_is_distributive_add_other_way,
    lemma_mul_unary_negation,
};
use vstd::prelude::*;

verus! {

pub open spec fn affine_prefix(
    constant: int,
    coefficients: Seq<int>,
    point: Seq<int>,
    count: nat,
) -> int
    recommends count <= coefficients.len(), count <= point.len()
    decreases count
{
    if count == 0 {
        constant
    } else {
        affine_prefix(constant, coefficients, point, (count - 1) as nat)
            + coefficients[(count - 1) as int] * point[(count - 1) as int]
    }
}

pub open spec fn affine_value(
    constant: int,
    coefficients: Seq<int>,
    point: Seq<int>,
) -> int
    recommends coefficients.len() == point.len()
{
    affine_prefix(constant, coefficients, point, coefficients.len())
}

pub open spec fn rows_have_rank(rows: Seq<Seq<int>>, rank: nat) -> bool {
    forall|row: int| 0 <= row < rows.len() ==> #[trigger] rows[row].len() == rank
}

pub open spec fn rows_hold(
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    point: Seq<int>,
) -> bool {
    row_constants.len() == rows.len()
        && forall|row: int| 0 <= row < rows.len() ==>
            #[trigger] affine_value(row_constants[row], rows[row], point) <= 0
}

pub open spec fn nonnegative_multipliers(multipliers: Seq<int>) -> bool {
    forall|row: int| 0 <= row < multipliers.len() ==> #[trigger] multipliers[row] >= 0
}

pub open spec fn weighted_constant_prefix(
    row_constants: Seq<int>,
    multipliers: Seq<int>,
    count: nat,
) -> int
    recommends count <= row_constants.len(), count <= multipliers.len()
    decreases count
{
    if count == 0 {
        0
    } else {
        weighted_constant_prefix(row_constants, multipliers, (count - 1) as nat)
            + multipliers[(count - 1) as int] * row_constants[(count - 1) as int]
    }
}

pub open spec fn weighted_coefficient_prefix(
    rows: Seq<Seq<int>>,
    multipliers: Seq<int>,
    dimension: int,
    count: nat,
) -> int
    recommends count <= rows.len(), count <= multipliers.len()
    decreases count
{
    if count == 0 {
        0
    } else {
        weighted_coefficient_prefix(rows, multipliers, dimension, (count - 1) as nat)
            + multipliers[(count - 1) as int] * rows[(count - 1) as int][dimension]
    }
}

pub open spec fn weighted_row_values_prefix(
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    multipliers: Seq<int>,
    point: Seq<int>,
    count: nat,
) -> int
    recommends
        count <= row_constants.len(),
        count <= rows.len(),
        count <= multipliers.len(),
    decreases count
{
    if count == 0 {
        0
    } else {
        weighted_row_values_prefix(
            row_constants,
            rows,
            multipliers,
            point,
            (count - 1) as nat,
        ) + multipliers[(count - 1) as int]
            * affine_value(
                row_constants[(count - 1) as int],
                rows[(count - 1) as int],
                point,
            )
    }
}

pub open spec fn combination_matches_target(
    target_constant: int,
    target_coefficients: Seq<int>,
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    multipliers: Seq<int>,
) -> bool {
    target_constant <= weighted_constant_prefix(row_constants, multipliers, rows.len())
        && forall|dimension: int| 0 <= dimension < target_coefficients.len() ==>
            #[trigger] target_coefficients[dimension]
                == weighted_coefficient_prefix(rows, multipliers, dimension, rows.len())
}

pub open spec fn constrained_checker_accepts(
    constant: int,
    coefficients: Seq<int>,
    extent: int,
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    domain_witness: Seq<int>,
    lower_multipliers: Seq<int>,
    upper_multipliers: Seq<int>,
) -> bool {
    coefficients.len() > 0
        && extent > 0
        && row_constants.len() == rows.len()
        && rows_have_rank(rows, coefficients.len())
        && domain_witness.len() == coefficients.len()
        && rows_hold(row_constants, rows, domain_witness)
        && lower_multipliers.len() == rows.len()
        && upper_multipliers.len() == rows.len()
        && nonnegative_multipliers(lower_multipliers)
        && nonnegative_multipliers(upper_multipliers)
        && combination_matches_target(
            -constant,
            coefficients.map_values(|coefficient: int| -coefficient),
            row_constants,
            rows,
            lower_multipliers,
        )
        && combination_matches_target(
            constant - (extent - 1),
            coefficients,
            row_constants,
            rows,
            upper_multipliers,
        )
}

/// A path is represented by exact edge identities. This keeps two CFG edges
/// with the same source and target distinct.
pub open spec fn valid_edge_path(
    edge_sources: Seq<int>,
    edge_targets: Seq<int>,
    entry: int,
    site: int,
    path: Seq<int>,
) -> bool {
    edge_sources.len() == edge_targets.len()
        && (path.len() == 0 ==> entry == site)
        && (path.len() > 0 ==> {
            &&& 0 <= path[0] < edge_sources.len()
            &&& edge_sources[path[0]] == entry
            &&& 0 <= path[(path.len() - 1) as int] < edge_targets.len()
            &&& edge_targets[path[(path.len() - 1) as int]] == site
            &&& forall|step: int| 0 <= step < path.len() ==> {
                &&& 0 <= #[trigger] path[step] < edge_sources.len()
                &&& (step + 1 < path.len() ==>
                    edge_targets[path[step]] == edge_sources[path[step + 1]])
            }
        })
}

pub open spec fn path_uses_edge(path: Seq<int>, edge: int) -> bool {
    exists|step: int| 0 <= step < path.len() && #[trigger] path[step] == edge
}

pub open spec fn reachable_without_edge(
    edge_sources: Seq<int>,
    edge_targets: Seq<int>,
    entry: int,
    site: int,
    removed_edge: int,
) -> bool {
    exists|path: Seq<int>| valid_edge_path(edge_sources, edge_targets, entry, site, path)
        && !path_uses_edge(path, removed_edge)
}

/// Generic CFG closure used by the V2 production replay. If deleting one exact
/// edge makes a reachable site unreachable, every path to the site crossed
/// that edge. The Rust graph-to-Verus correspondence remains in the TCB.
pub proof fn cut_edge_unreachability_implies_edge_dominance(
    edge_sources: Seq<int>,
    edge_targets: Seq<int>,
    entry: int,
    site: int,
    removed_edge: int,
    path: Seq<int>,
)
    requires
        valid_edge_path(edge_sources, edge_targets, entry, site, path),
        !reachable_without_edge(edge_sources, edge_targets, entry, site, removed_edge),
    ensures path_uses_edge(path, removed_edge),
{
    if !path_uses_edge(path, removed_edge) {
        assert(reachable_without_edge(
            edge_sources,
            edge_targets,
            entry,
            site,
            removed_edge,
        ));
    }
}

proof fn affine_difference_plus_one_prefix(
    lhs_constant: int,
    lhs_coefficients: Seq<int>,
    rhs_constant: int,
    rhs_coefficients: Seq<int>,
    point: Seq<int>,
    count: nat,
)
    requires
        lhs_coefficients.len() == point.len(),
        rhs_coefficients.len() == point.len(),
        count <= point.len(),
    ensures
        affine_prefix(
            lhs_constant - rhs_constant + 1,
            Seq::new(point.len(), |dimension: int|
                lhs_coefficients[dimension] - rhs_coefficients[dimension]),
            point,
            count,
        ) == affine_prefix(lhs_constant, lhs_coefficients, point, count)
            - affine_prefix(rhs_constant, rhs_coefficients, point, count) + 1,
    decreases count
{
    if count > 0 {
        affine_difference_plus_one_prefix(
            lhs_constant,
            lhs_coefficients,
            rhs_constant,
            rhs_coefficients,
            point,
            (count - 1) as nat,
        );
        let dimension = (count - 1) as int;
        lemma_mul_is_distributive_add_other_way(
            point[dimension],
            lhs_coefficients[dimension],
            -rhs_coefficients[dimension],
        );
        lemma_mul_unary_negation(rhs_coefficients[dimension], point[dimension]);
        reveal(affine_prefix);
    }
}

/// The integer semantics of an exact true `lhs < rhs` edge establishes its
/// canonical Presburger row at the same immutable affine valuation.
pub proof fn strict_affine_guard_implies_normalized_row(
    lhs_constant: int,
    lhs_coefficients: Seq<int>,
    rhs_constant: int,
    rhs_coefficients: Seq<int>,
    point: Seq<int>,
)
    requires
        lhs_coefficients.len() == point.len(),
        rhs_coefficients.len() == point.len(),
        affine_value(lhs_constant, lhs_coefficients, point)
            < affine_value(rhs_constant, rhs_coefficients, point),
    ensures
        affine_value(
            lhs_constant - rhs_constant + 1,
            Seq::new(point.len(), |dimension: int|
                lhs_coefficients[dimension] - rhs_coefficients[dimension]),
            point,
        ) <= 0,
{
    affine_difference_plus_one_prefix(
        lhs_constant,
        lhs_coefficients,
        rhs_constant,
        rhs_coefficients,
        point,
        point.len(),
    );
}

proof fn weighted_rows_are_nonpositive(
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    multipliers: Seq<int>,
    point: Seq<int>,
    count: nat,
)
    requires
        row_constants.len() == rows.len(),
        multipliers.len() == rows.len(),
        count <= rows.len(),
        rows_hold(row_constants, rows, point),
        nonnegative_multipliers(multipliers),
    ensures
        weighted_row_values_prefix(row_constants, rows, multipliers, point, count) <= 0,
    decreases count
{
    if count > 0 {
        weighted_rows_are_nonpositive(
            row_constants,
            rows,
            multipliers,
            point,
            (count - 1) as nat,
        );
        let row = (count - 1) as int;
        assert(affine_value(row_constants[row], rows[row], point) <= 0);
        assert(multipliers[row] >= 0);
        lemma_mul_inequality(
            affine_value(row_constants[row], rows[row], point),
            0,
            multipliers[row],
        );
    }
}

proof fn weighted_affine_expansion_prefix(
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    multipliers: Seq<int>,
    point: Seq<int>,
    row_count: nat,
)
    requires
        row_constants.len() == rows.len(),
        multipliers.len() == rows.len(),
        rows_have_rank(rows, point.len()),
        row_count <= rows.len(),
    ensures
        weighted_row_values_prefix(row_constants, rows, multipliers, point, row_count)
            == weighted_constant_prefix(row_constants, multipliers, row_count)
                + affine_value(0,
                    Seq::new(point.len(), |dimension: int|
                        weighted_coefficient_prefix(rows, multipliers, dimension, row_count)),
                    point,
                ),
    decreases row_count
{
    if row_count == 0 {
        let current = Seq::new(point.len(), |dimension: int|
            weighted_coefficient_prefix(rows, multipliers, dimension, row_count));
        assert forall|dimension: int| 0 <= dimension < point.len() implies
            current[dimension] == 0
        by {
            reveal(weighted_coefficient_prefix);
        }
        assert(current =~= Seq::new(point.len(), |_dimension: int| 0));
        zero_affine(point, point.len());
        reveal(weighted_row_values_prefix);
        reveal(weighted_constant_prefix);
        reveal(affine_value);
    } else {
        let row = (row_count - 1) as int;
        assert(rows[row].len() == point.len());
        weighted_affine_expansion_prefix(
            row_constants,
            rows,
            multipliers,
            point,
            (row_count - 1) as nat,
        );
        // Distribute this row multiplier across the affine form. The following
        // helper induction exposes one coordinate at a time to integer algebra.
        distribute_one_row(
            row_constants[row],
            rows[row],
            multipliers[row],
            point,
            point.len(),
        );
        let previous = Seq::new(point.len(), |dimension: int|
            weighted_coefficient_prefix(
                rows,
                multipliers,
                dimension,
                (row_count - 1) as nat,
            ));
        let scaled = Seq::new(point.len(), |dimension: int|
            multipliers[row] * rows[row][dimension]);
        let current = Seq::new(point.len(), |dimension: int|
            weighted_coefficient_prefix(rows, multipliers, dimension, row_count));
        assert forall|dimension: int| 0 <= dimension < point.len() implies
            current[dimension] == previous[dimension] + scaled[dimension]
        by {
            reveal(weighted_coefficient_prefix);
        }
        assert(current =~= Seq::new(point.len(), |dimension: int|
            previous[dimension] + scaled[dimension]));
        affine_coefficient_sum_distributes(previous, scaled, point, point.len());
        reveal(weighted_row_values_prefix);
        reveal(weighted_constant_prefix);
        reveal(affine_value);
        assert(affine_value(0, current, point)
            == affine_value(0, previous, point) + affine_value(0, scaled, point));
        assert(multipliers[row] * affine_value(row_constants[row], rows[row], point)
            == multipliers[row] * row_constants[row] + affine_value(0, scaled, point));
        assert(
            weighted_row_values_prefix(row_constants, rows, multipliers, point, row_count)
                == weighted_constant_prefix(row_constants, multipliers, row_count)
                    + affine_value(0, current, point)
        );
        assert(
            weighted_row_values_prefix(row_constants, rows, multipliers, point, row_count)
                == weighted_constant_prefix(row_constants, multipliers, row_count)
                    + affine_value(
                        0,
                        Seq::new(point.len(), |dimension: int|
                            weighted_coefficient_prefix(
                                rows,
                                multipliers,
                                dimension,
                                row_count,
                            )),
                        point,
                    )
        );
    }
}

proof fn zero_affine(point: Seq<int>, count: nat)
    requires count <= point.len(),
    ensures affine_prefix(0, Seq::new(point.len(), |_dimension: int| 0), point, count) == 0,
    decreases count
{
    if count > 0 {
        zero_affine(point, (count - 1) as nat);
        reveal(affine_prefix);
    }
}

proof fn affine_coefficient_sum_distributes(
    left: Seq<int>,
    right: Seq<int>,
    point: Seq<int>,
    count: nat,
)
    requires left.len() == point.len(), right.len() == point.len(), count <= point.len(),
    ensures
        affine_prefix(
            0,
            Seq::new(point.len(), |dimension: int| left[dimension] + right[dimension]),
            point,
            count,
        ) == affine_prefix(0, left, point, count) + affine_prefix(0, right, point, count),
    decreases count
{
    if count > 0 {
        affine_coefficient_sum_distributes(left, right, point, (count - 1) as nat);
        let dimension = (count - 1) as int;
        lemma_mul_is_distributive_add_other_way(
            point[dimension],
            left[dimension],
            right[dimension],
        );
        reveal(affine_prefix);
    }
}

proof fn distribute_one_row(
    constant: int,
    coefficients: Seq<int>,
    multiplier: int,
    point: Seq<int>,
    count: nat,
)
    requires coefficients.len() == point.len(), count <= point.len(),
    ensures
        multiplier * affine_prefix(constant, coefficients, point, count)
            == multiplier * constant
                + affine_prefix(
                    0,
                    Seq::new(point.len(), |dimension: int|
                        multiplier * coefficients[dimension]),
                    point,
                    count,
                ),
    decreases count
{
    if count > 0 {
        distribute_one_row(constant, coefficients, multiplier, point, (count - 1) as nat);
        let dimension = (count - 1) as int;
        let previous = affine_prefix(constant, coefficients, point, (count - 1) as nat);
        let term = coefficients[dimension] * point[dimension];
        let scaled = Seq::new(point.len(), |i: int| multiplier * coefficients[i]);
        lemma_mul_is_distributive_add(
            multiplier,
            previous,
            term,
        );
        lemma_mul_is_associative(multiplier, coefficients[dimension], point[dimension]);
        assert(scaled[dimension] == multiplier * coefficients[dimension]);
        reveal(affine_prefix);
        assert(multiplier * previous
            == multiplier * constant
                + affine_prefix(0, scaled, point, (count - 1) as nat));
        assert(multiplier * (previous + term) == multiplier * previous + multiplier * term);
        assert(multiplier * affine_prefix(constant, coefficients, point, count)
            == multiplier * previous + multiplier * term);
        assert(multiplier * term == scaled[dimension] * point[dimension]);
        assert(affine_prefix(0, scaled, point, count)
            == affine_prefix(0, scaled, point, (count - 1) as nat)
                + scaled[dimension] * point[dimension]);
        assert(
            multiplier * previous + multiplier * term
                == multiplier * constant + affine_prefix(0, scaled, point, count)
        );
        assert(multiplier * affine_prefix(constant, coefficients, point, count)
            == multiplier * constant + affine_prefix(0, scaled, point, count));
    }
}

proof fn matched_combination_bounds_target(
    target_constant: int,
    target_coefficients: Seq<int>,
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    multipliers: Seq<int>,
    point: Seq<int>,
)
    requires
        target_coefficients.len() == point.len(),
        row_constants.len() == rows.len(),
        multipliers.len() == rows.len(),
        rows_have_rank(rows, point.len()),
        combination_matches_target(
            target_constant,
            target_coefficients,
            row_constants,
            rows,
            multipliers,
        ),
    ensures
        affine_value(target_constant, target_coefficients, point)
            <= weighted_row_values_prefix(
                row_constants,
                rows,
                multipliers,
                point,
                rows.len(),
            ),
{
    weighted_affine_expansion_prefix(
        row_constants,
        rows,
        multipliers,
        point,
        rows.len(),
    );
    assert forall|dimension: int| 0 <= dimension < point.len() implies
        #[trigger] target_coefficients[dimension]
            == Seq::new(point.len(), |d: int|
                weighted_coefficient_prefix(rows, multipliers, d, rows.len()))[dimension]
    by {
    }
    assert(target_coefficients =~= Seq::new(point.len(), |dimension: int|
        weighted_coefficient_prefix(rows, multipliers, dimension, rows.len())));
    let combined = Seq::new(point.len(), |dimension: int|
        weighted_coefficient_prefix(rows, multipliers, dimension, rows.len()));
    let combined_constant = weighted_constant_prefix(row_constants, multipliers, rows.len());
    affine_constant_order(
        target_constant,
        combined_constant,
        target_coefficients,
        point,
        point.len(),
    );
    assert(affine_value(target_constant, target_coefficients, point)
        <= affine_value(combined_constant, combined, point));
    affine_constant_shift(
        combined_constant,
        combined_constant,
        combined,
        point,
        point.len(),
    );
    reveal(affine_value);
    assert(affine_value(combined_constant, combined, point)
        == combined_constant + affine_value(0, combined, point));
}

proof fn affine_constant_order(
    lower: int,
    upper: int,
    coefficients: Seq<int>,
    point: Seq<int>,
    count: nat,
)
    requires lower <= upper, coefficients.len() == point.len(), count <= point.len(),
    ensures affine_prefix(lower, coefficients, point, count)
        <= affine_prefix(upper, coefficients, point, count),
    decreases count
{
    if count > 0 {
        affine_constant_order(lower, upper, coefficients, point, (count - 1) as nat);
    }
}

/// Accepted evidence contains a concrete satisfying point, so the constrained
/// domain is not empty.
pub proof fn accepted_certificate_has_nonempty_domain(
    constant: int,
    coefficients: Seq<int>,
    extent: int,
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    domain_witness: Seq<int>,
    lower_multipliers: Seq<int>,
    upper_multipliers: Seq<int>,
)
    requires constrained_checker_accepts(
        constant,
        coefficients,
        extent,
        row_constants,
        rows,
        domain_witness,
        lower_multipliers,
        upper_multipliers,
    ),
    ensures
        domain_witness.len() == coefficients.len(),
        rows_hold(row_constants, rows, domain_witness),
{
}

/// Universal soundness theorem for the exact constrained row system checked by V2.
pub proof fn accepted_constrained_affine_certificate_is_sound(
    constant: int,
    coefficients: Seq<int>,
    extent: int,
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    domain_witness: Seq<int>,
    lower_multipliers: Seq<int>,
    upper_multipliers: Seq<int>,
    point: Seq<int>,
)
    requires
        constrained_checker_accepts(
            constant,
            coefficients,
            extent,
            row_constants,
            rows,
            domain_witness,
            lower_multipliers,
            upper_multipliers,
        ),
        point.len() == coefficients.len(),
        rows_hold(row_constants, rows, point),
    ensures 0 <= affine_value(constant, coefficients, point) < extent,
{
    let lower_coefficients = coefficients.map_values(|coefficient: int| -coefficient);
    matched_combination_bounds_target(
        -constant,
        lower_coefficients,
        row_constants,
        rows,
        lower_multipliers,
        point,
    );
    weighted_rows_are_nonpositive(
        row_constants,
        rows,
        lower_multipliers,
        point,
        rows.len(),
    );
    matched_combination_bounds_target(
        constant - (extent - 1),
        coefficients,
        row_constants,
        rows,
        upper_multipliers,
        point,
    );
    weighted_rows_are_nonpositive(
        row_constants,
        rows,
        upper_multipliers,
        point,
        rows.len(),
    );
    assert(lower_coefficients =~= Seq::new(coefficients.len(), |i: int| -coefficients[i]));
    affine_negation(constant, coefficients, point, coefficients.len());
    affine_constant_shift(
        constant,
        extent - 1,
        coefficients,
        point,
        coefficients.len(),
    );
}

/// End-to-end mathematical composition for the production V2 subset. The
/// `path_uses_edge ==> lhs < rhs` premise is the exact immutable-SSA true-edge
/// operational semantics supplied by the ranked IR correspondence in the TCB.
pub proof fn accepted_edge_dominated_affine_guard_is_sound(
    constant: int,
    coefficients: Seq<int>,
    extent: int,
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    domain_witness: Seq<int>,
    lower_multipliers: Seq<int>,
    upper_multipliers: Seq<int>,
    point: Seq<int>,
    lhs_constant: int,
    lhs_coefficients: Seq<int>,
    rhs_constant: int,
    rhs_coefficients: Seq<int>,
    edge_sources: Seq<int>,
    edge_targets: Seq<int>,
    entry: int,
    site: int,
    true_edge: int,
    path: Seq<int>,
)
    requires
        constrained_checker_accepts(
            constant,
            coefficients,
            extent,
            row_constants,
            rows,
            domain_witness,
            lower_multipliers,
            upper_multipliers,
        ),
        point.len() == coefficients.len(),
        lhs_coefficients.len() == point.len(),
        rhs_coefficients.len() == point.len(),
        rows.len() > 0,
        row_constants[0] == lhs_constant - rhs_constant + 1,
        rows[0] =~= Seq::new(point.len(), |dimension: int|
            lhs_coefficients[dimension] - rhs_coefficients[dimension]),
        valid_edge_path(edge_sources, edge_targets, entry, site, path),
        !reachable_without_edge(edge_sources, edge_targets, entry, site, true_edge),
        path_uses_edge(path, true_edge) ==> affine_value(
            lhs_constant,
            lhs_coefficients,
            point,
        ) < affine_value(rhs_constant, rhs_coefficients, point),
        forall|row: int| 1 <= row < rows.len() ==>
            #[trigger] affine_value(row_constants[row], rows[row], point) <= 0,
    ensures 0 <= affine_value(constant, coefficients, point) < extent,
{
    cut_edge_unreachability_implies_edge_dominance(
        edge_sources,
        edge_targets,
        entry,
        site,
        true_edge,
        path,
    );
    strict_affine_guard_implies_normalized_row(
        lhs_constant,
        lhs_coefficients,
        rhs_constant,
        rhs_coefficients,
        point,
    );
    assert(affine_value(row_constants[0], rows[0], point) <= 0);
    assert forall|row: int| 0 <= row < rows.len() implies
        #[trigger] affine_value(row_constants[row], rows[row], point) <= 0 by {
        if row > 0 {
            assert(affine_value(row_constants[row], rows[row], point) <= 0);
        }
    }
    assert(rows_hold(row_constants, rows, point));
    accepted_constrained_affine_certificate_is_sound(
        constant,
        coefficients,
        extent,
        row_constants,
        rows,
        domain_witness,
        lower_multipliers,
        upper_multipliers,
        point,
    );
}

proof fn affine_negation(
    constant: int,
    coefficients: Seq<int>,
    point: Seq<int>,
    count: nat,
)
    requires coefficients.len() == point.len(), count <= point.len(),
    ensures
        affine_prefix(-constant, coefficients.map_values(|coefficient: int| -coefficient), point, count)
            == -affine_prefix(constant, coefficients, point, count),
    decreases count
{
    if count > 0 {
        affine_negation(constant, coefficients, point, (count - 1) as nat);
        let dimension = (count - 1) as int;
        lemma_mul_unary_negation(coefficients[dimension], point[dimension]);
        reveal(affine_prefix);
    }
}

proof fn affine_constant_shift(
    constant: int,
    shift: int,
    coefficients: Seq<int>,
    point: Seq<int>,
    count: nat,
)
    requires coefficients.len() == point.len(), count <= point.len(),
    ensures
        affine_prefix(constant - shift, coefficients, point, count)
            == affine_prefix(constant, coefficients, point, count) - shift,
    decreases count
{
    if count > 0 {
        affine_constant_shift(constant, shift, coefficients, point, (count - 1) as nat);
        reveal(affine_prefix);
    }
}

}
