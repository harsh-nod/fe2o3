#[path = "constrained_affine_bounds_v2.rs"]
mod constrained_v2;

use constrained_v2::{
    accepted_constrained_affine_certificate_is_sound,
    affine_prefix,
    affine_value,
    constrained_checker_accepts,
    cut_edge_unreachability_implies_edge_dominance,
    reachable_without_edge,
    rows_hold,
    strict_affine_guard_implies_normalized_row,
    valid_edge_path,
};
use vstd::arithmetic::mul::{
    lemma_mul_is_distributive_add_other_way,
    lemma_mul_unary_negation,
};
use vstd::prelude::*;

verus! {

/// Public V3 vocabulary for the exact conjunction of normalized affine rows.
pub open spec fn dynamic_rows_hold_v3(
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    point: Seq<int>,
) -> bool {
    rows_hold(row_constants, rows, point)
}

/// Public V3 vocabulary for an affine value at one exact point.
pub open spec fn dynamic_affine_value_v3(
    constant: int,
    coefficients: Seq<int>,
    point: Seq<int>,
) -> int {
    affine_value(constant, coefficients, point)
}

/// A strict affine comparison establishes its exact normalized integer row.
pub proof fn dynamic_strict_affine_guard_implies_normalized_row_v3(
    lhs_constant: int,
    lhs_coefficients: Seq<int>,
    rhs_constant: int,
    rhs_coefficients: Seq<int>,
    point: Seq<int>,
)
    requires
        lhs_coefficients.len() == point.len(),
        rhs_coefficients.len() == point.len(),
        dynamic_affine_value_v3(lhs_constant, lhs_coefficients, point)
            < dynamic_affine_value_v3(rhs_constant, rhs_coefficients, point),
    ensures
        dynamic_affine_value_v3(
            lhs_constant - rhs_constant + 1,
            Seq::new(point.len(), |dimension: int|
                lhs_coefficients[dimension] - rhs_coefficients[dimension]),
            point,
        ) <= 0,
{
    strict_affine_guard_implies_normalized_row(
        lhs_constant,
        lhs_coefficients,
        rhs_constant,
        rhs_coefficients,
        point,
    );
}

pub open spec fn dynamic_checker_accepts(
    index_constant: int,
    index_coefficients: Seq<int>,
    extent_constant: int,
    extent_coefficients: Seq<int>,
    slack_constant: int,
    slack_coefficients: Seq<int>,
    component_ceiling: int,
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    domain_witness: Seq<int>,
    index_lower_multipliers: Seq<int>,
    index_upper_multipliers: Seq<int>,
    slack_lower_multipliers: Seq<int>,
    slack_upper_multipliers: Seq<int>,
) -> bool {
    rows.len() >= 2
        && (forall|left: int, right: int|
            0 <= left < right < rows.len() ==>
                row_constants[left] != row_constants[right] || rows[left] != rows[right])
        && index_coefficients.len() == extent_coefficients.len()
        && slack_coefficients.len() == index_coefficients.len()
        && slack_constant == extent_constant - index_constant - 1
        && slack_coefficients =~= Seq::new(index_coefficients.len(), |dimension: int|
            extent_coefficients[dimension] - index_coefficients[dimension])
        && constrained_checker_accepts(
            index_constant,
            index_coefficients,
            component_ceiling,
            row_constants,
            rows,
            domain_witness,
            index_lower_multipliers,
            index_upper_multipliers,
        )
        && constrained_checker_accepts(
            slack_constant,
            slack_coefficients,
            component_ceiling,
            row_constants,
            rows,
            domain_witness,
            slack_lower_multipliers,
            slack_upper_multipliers,
        )
}

proof fn affine_slack_relation_prefix(
    index_constant: int,
    index_coefficients: Seq<int>,
    extent_constant: int,
    extent_coefficients: Seq<int>,
    point: Seq<int>,
    count: nat,
)
    requires
        index_coefficients.len() == point.len(),
        extent_coefficients.len() == point.len(),
        count <= point.len(),
    ensures
        affine_prefix(
            extent_constant - index_constant - 1,
            Seq::new(point.len(), |dimension: int|
                extent_coefficients[dimension] - index_coefficients[dimension]),
            point,
            count,
        ) == affine_prefix(extent_constant, extent_coefficients, point, count)
            - affine_prefix(index_constant, index_coefficients, point, count) - 1,
    decreases count
{
    if count > 0 {
        affine_slack_relation_prefix(
            index_constant,
            index_coefficients,
            extent_constant,
            extent_coefficients,
            point,
            (count - 1) as nat,
        );
        let dimension = (count - 1) as int;
        lemma_mul_is_distributive_add_other_way(
            point[dimension],
            extent_coefficients[dimension],
            -index_coefficients[dimension],
        );
        lemma_mul_unary_negation(index_coefficients[dimension], point[dimension]);
        reveal(affine_prefix);
    }
}

/// Acceptance contains the same concrete witness for both component proofs.
pub proof fn accepted_dynamic_certificate_has_nonempty_domain(
    index_constant: int,
    index_coefficients: Seq<int>,
    extent_constant: int,
    extent_coefficients: Seq<int>,
    slack_constant: int,
    slack_coefficients: Seq<int>,
    component_ceiling: int,
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    domain_witness: Seq<int>,
    index_lower_multipliers: Seq<int>,
    index_upper_multipliers: Seq<int>,
    slack_lower_multipliers: Seq<int>,
    slack_upper_multipliers: Seq<int>,
)
    requires dynamic_checker_accepts(
        index_constant, index_coefficients, extent_constant, extent_coefficients,
        slack_constant, slack_coefficients, component_ceiling, row_constants, rows,
        domain_witness, index_lower_multipliers, index_upper_multipliers,
        slack_lower_multipliers, slack_upper_multipliers,
    ),
    ensures
        domain_witness.len() == index_coefficients.len(),
        dynamic_rows_hold_v3(row_constants, rows, domain_witness),
{
}

/// Universal relational theorem checked by V3.
pub proof fn accepted_dynamic_constrained_affine_certificate_is_sound(
    index_constant: int,
    index_coefficients: Seq<int>,
    extent_constant: int,
    extent_coefficients: Seq<int>,
    slack_constant: int,
    slack_coefficients: Seq<int>,
    component_ceiling: int,
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    domain_witness: Seq<int>,
    index_lower_multipliers: Seq<int>,
    index_upper_multipliers: Seq<int>,
    slack_lower_multipliers: Seq<int>,
    slack_upper_multipliers: Seq<int>,
    point: Seq<int>,
)
    requires
        dynamic_checker_accepts(
            index_constant, index_coefficients, extent_constant, extent_coefficients,
            slack_constant, slack_coefficients, component_ceiling, row_constants, rows,
            domain_witness, index_lower_multipliers, index_upper_multipliers,
            slack_lower_multipliers, slack_upper_multipliers,
        ),
        point.len() == index_coefficients.len(),
        dynamic_rows_hold_v3(row_constants, rows, point),
    ensures
        0 <= dynamic_affine_value_v3(index_constant, index_coefficients, point),
        dynamic_affine_value_v3(index_constant, index_coefficients, point)
            < dynamic_affine_value_v3(extent_constant, extent_coefficients, point),
{
    accepted_constrained_affine_certificate_is_sound(
        index_constant, index_coefficients, component_ceiling, row_constants, rows,
        domain_witness, index_lower_multipliers, index_upper_multipliers, point,
    );
    accepted_constrained_affine_certificate_is_sound(
        slack_constant, slack_coefficients, component_ceiling, row_constants, rows,
        domain_witness, slack_lower_multipliers, slack_upper_multipliers, point,
    );
    affine_slack_relation_prefix(
        index_constant, index_coefficients, extent_constant, extent_coefficients,
        point, point.len(),
    );
    assert(slack_coefficients =~= Seq::new(point.len(), |dimension: int|
        extent_coefficients[dimension] - index_coefficients[dimension]));
    reveal(affine_value);
}

/// Every independently cut dominating true edge establishes its exact row.
pub proof fn authenticated_dominating_guards_establish_all_rows(
    row_constants: Seq<int>,
    rows: Seq<Seq<int>>,
    lhs_constants: Seq<int>,
    lhs_coefficients: Seq<Seq<int>>,
    rhs_constants: Seq<int>,
    rhs_coefficients: Seq<Seq<int>>,
    edge_sources: Seq<int>,
    edge_targets: Seq<int>,
    entry: int,
    site: int,
    true_edges: Seq<int>,
    path: Seq<int>,
    point: Seq<int>,
)
    requires
        row_constants.len() == rows.len(),
        lhs_constants.len() == rows.len(),
        lhs_coefficients.len() == rows.len(),
        rhs_constants.len() == rows.len(),
        rhs_coefficients.len() == rows.len(),
        true_edges.len() == rows.len(),
        valid_edge_path(edge_sources, edge_targets, entry, site, path),
        forall|row: int| 0 <= row < rows.len() ==>
            #[trigger] lhs_coefficients[row].len() == point.len(),
        forall|row: int| 0 <= row < rows.len() ==>
            #[trigger] rhs_coefficients[row].len() == point.len(),
        forall|row: int| 0 <= row < rows.len() ==>
            #[trigger] row_constants[row] == lhs_constants[row] - rhs_constants[row] + 1,
        forall|row: int| 0 <= row < rows.len() ==>
            #[trigger] rows[row] =~= Seq::new(point.len(), |dimension: int|
                lhs_coefficients[row][dimension] - rhs_coefficients[row][dimension]),
        forall|row: int| 0 <= row < rows.len() ==>
            !reachable_without_edge(
                edge_sources, edge_targets, entry, site, #[trigger] true_edges[row]),
        forall|row: int| 0 <= row < rows.len() ==>
            (constrained_v2::path_uses_edge(path, #[trigger] true_edges[row]) ==>
                affine_value(lhs_constants[row], lhs_coefficients[row], point)
                    < affine_value(rhs_constants[row], rhs_coefficients[row], point)),
    ensures dynamic_rows_hold_v3(row_constants, rows, point),
{
    assert forall|row: int| 0 <= row < rows.len() implies
        #[trigger] affine_value(row_constants[row], rows[row], point) <= 0 by {
        assert(lhs_coefficients[row].len() == point.len());
        assert(rhs_coefficients[row].len() == point.len());
        assert(!reachable_without_edge(
            edge_sources, edge_targets, entry, site, true_edges[row]
        ));
        cut_edge_unreachability_implies_edge_dominance(
            edge_sources, edge_targets, entry, site, true_edges[row], path,
        );
        assert(constrained_v2::path_uses_edge(path, true_edges[row]));
        assert(affine_value(lhs_constants[row], lhs_coefficients[row], point)
            < affine_value(rhs_constants[row], rhs_coefficients[row], point));
        strict_affine_guard_implies_normalized_row(
            lhs_constants[row], lhs_coefficients[row],
            rhs_constants[row], rhs_coefficients[row], point,
        );
        assert(row_constants[row] == lhs_constants[row] - rhs_constants[row] + 1);
        assert(rows[row] =~= Seq::new(point.len(), |dimension: int|
            lhs_coefficients[row][dimension] - rhs_coefficients[row][dimension]));
    }
}

}
