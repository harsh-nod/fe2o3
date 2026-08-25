include!("mir_pliron_per_compilation_template_v1.rs");

verus! {

pub open spec fn fe2o3_output_0_bound_v1() -> int { 64 }

pub proof fn fe2o3_output_0_refines_v1(actual: Seq<int>, reference: Seq<int>)
    requires
        fe2o3_pointwise_equal_v1(actual, reference),
        actual.len() > 0,
        actual.len() == fe2o3_output_0_bound_v1(),
    ensures actual == reference,
{
    fe2o3_exact_total_output_v1(actual, reference);
}

pub open spec fn fe2o3_loop_0_maximum_steps_v1() -> int { 64 }

pub proof fn fe2o3_loop_0_refines_v1(actual: Seq<int>, reference: Seq<int>)
    requires
        fe2o3_finite_recurrence_v1(actual, reference),
        actual.len() - 1 <= fe2o3_loop_0_maximum_steps_v1(),
    ensures actual == reference,
{
    fe2o3_finite_recurrence_refinement_v1(actual, reference);
}

pub open spec fn fe2o3_collective_0_kind_v1() -> int { 1 }
pub open spec fn fe2o3_collective_0_order_v1() -> int { 1 }
pub open spec fn fe2o3_collective_0_domain_bound_v1() -> int { 64 }
pub open spec fn fe2o3_collective_0_step_bound_v1() -> int { 64 }

pub proof fn fe2o3_collective_0_refines_v1(actual: Seq<int>, reference: Seq<int>)
    requires
        fe2o3_finite_recurrence_v1(actual, reference),
        actual.len() - 1 <= fe2o3_collective_0_step_bound_v1(),
        actual.len() - 1 <= fe2o3_collective_0_domain_bound_v1(),
    ensures actual == reference,
{
    fe2o3_finite_recurrence_refinement_v1(actual, reference);
}

pub open spec fn fe2o3_collective_1_kind_v1() -> int { 3 }
pub open spec fn fe2o3_collective_1_order_v1() -> int { 3 }
pub open spec fn fe2o3_collective_1_domain_bound_v1() -> int { 64 }
pub open spec fn fe2o3_collective_1_step_bound_v1() -> int { 64 }

pub proof fn fe2o3_collective_1_is_injective_v1(
    mapping: Seq<nat>,
    inverse: Seq<nat>,
    left: nat,
    right: nat,
)
    requires
        fe2o3_inverse_permutation_v1(mapping, inverse),
        mapping.len() == fe2o3_collective_1_domain_bound_v1(),
        left < mapping.len(),
        right < mapping.len(),
        mapping[left as int] == mapping[right as int],
    ensures left == right,
{
    fe2o3_permutation_injective_v1(mapping, inverse, left, right);
}

} // verus!

fn fe2o3_contract_instantiations_v1() {}
