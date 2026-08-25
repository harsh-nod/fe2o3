include!("mir_pliron_per_compilation_template_v1.rs");

verus! {

pub open spec fn fe2o3_output_0_bound_v1() -> int { 16 }

pub proof fn fe2o3_output_0_refines_v1(actual: Seq<int>, reference: Seq<int>)
    requires
        fe2o3_pointwise_equal_v1(actual, reference),
        actual.len() > 0,
        actual.len() == fe2o3_output_0_bound_v1(),
    ensures actual == reference,
{
    fe2o3_exact_total_output_v1(actual, reference);
}

pub open spec fn fe2o3_output_1_bound_v1() -> int { 8 }

pub proof fn fe2o3_output_1_refines_v1(actual: Seq<int>, reference: Seq<int>)
    requires
        fe2o3_pointwise_equal_v1(actual, reference),
        actual.len() > 0,
        actual.len() == fe2o3_output_1_bound_v1(),
    ensures actual == reference,
{
    fe2o3_exact_total_output_v1(actual, reference);
}

pub open spec fn fe2o3_output_product_arity_v1() -> int { 2 }

pub proof fn fe2o3_output_product_refines_v1(
    actual: Seq<Seq<int>>,
    reference: Seq<Seq<int>>,
)
    requires
        actual.len() == fe2o3_output_product_arity_v1(),
        reference.len() == fe2o3_output_product_arity_v1(),
        fe2o3_pointwise_equal_v1(actual[0], reference[0]),
        actual[0].len() > 0,
        actual[0].len() == fe2o3_output_0_bound_v1(),
        fe2o3_pointwise_equal_v1(actual[1], reference[1]),
        actual[1].len() > 0,
        actual[1].len() == fe2o3_output_1_bound_v1(),
    ensures actual == reference,
{
    fe2o3_output_0_refines_v1(actual[0], reference[0]);
    fe2o3_output_1_refines_v1(actual[1], reference[1]);
    assert forall|output: int| 0 <= output < actual.len() implies
        #[trigger] actual[output] == #[trigger] reference[output] by {
        assert(output == 0 || output == 1);
        if output == 0 {
            assert(actual[output] == reference[output]);
        } else if output == 1 {
            assert(actual[output] == reference[output]);
        } else {
            assert(false);
        }
    }
    assert(actual =~= reference);
}

} // verus!

fn fe2o3_contract_instantiations_v1() {}
