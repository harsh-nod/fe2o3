use vstd::prelude::*;

verus! {

/// One normalized final write after the hierarchy and ownership passes.
pub struct SemanticWriteV1 {
    pub invocation: nat,
    pub subgroup: nat,
    pub workgroup: nat,
    pub lane: nat,
    pub coordinate: int,
    pub value: int,
}

/// Workload-neutral facts supplied by the live PLIRON pass pipeline.
pub open spec fn exact_total_output_contract_v1(
    writes: Seq<SemanticWriteV1>,
    reference: Seq<int>,
    subgroup_size: nat,
    workgroup_size: nat,
) -> bool {
    &&& subgroup_size > 0
    &&& workgroup_size > 0
    &&& workgroup_size % subgroup_size == 0
    &&& writes.len() == reference.len()
    &&& forall|i: int| 0 <= i < writes.len() ==> {
        &&& #[trigger] writes[i].coordinate == i
        &&& writes[i].invocation == i as nat
        &&& writes[i].lane == writes[i].invocation % subgroup_size
        &&& writes[i].subgroup == writes[i].invocation / subgroup_size
        &&& writes[i].workgroup == writes[i].invocation / workgroup_size
        &&& writes[i].value == reference[i]
    }
}

/// Shared final theorem: per-coordinate equality plus exact ownership implies
/// equality of the complete GPU output and safe CPU reference.
pub proof fn exact_total_output_refines_safe_reference_v1(
    writes: Seq<SemanticWriteV1>,
    reference: Seq<int>,
    subgroup_size: nat,
    workgroup_size: nat,
)
    requires
        exact_total_output_contract_v1(
            writes,
            reference,
            subgroup_size,
            workgroup_size,
        ),
    ensures
        forall|coordinate: int| 0 <= coordinate < reference.len() ==> {
            &&& #[trigger] writes[coordinate].coordinate == coordinate
            &&& writes[coordinate].value == reference[coordinate]
        },
        forall|left: int, right: int|
            0 <= left < writes.len()
                && 0 <= right < writes.len()
                && #[trigger] writes[left].coordinate == #[trigger] writes[right].coordinate
                ==> left == right,
{
    assert(writes.len() == reference.len());
    assert forall|coordinate: int| 0 <= coordinate < reference.len() implies {
        &&& #[trigger] writes[coordinate].coordinate == coordinate
        &&& writes[coordinate].value == reference[coordinate]
    } by {
        assert(0 <= coordinate < writes.len());
        assert(writes[coordinate].coordinate == coordinate);
        assert(writes[coordinate].value == reference[coordinate]);
    }
    assert forall|left: int, right: int|
        0 <= left < writes.len()
            && 0 <= right < writes.len()
            && #[trigger] writes[left].coordinate == #[trigger] writes[right].coordinate
            implies left == right by {
        assert(writes[left].coordinate == left);
        assert(writes[right].coordinate == right);
    }
}

/// The workload-specific proof must establish this implication for every
/// actual/reference transition pair. The shared theorem does not assume an
/// addition, maximum, matrix product, routing rule, or other workload.
pub open spec fn finite_recurrence_contract_v1(
    actual: Seq<int>,
    reference: Seq<int>,
) -> bool {
    &&& actual.len() == reference.len()
    &&& actual.len() > 0
    &&& actual[0] == reference[0]
    &&& forall|i: int| 0 <= i < actual.len() - 1
        && #[trigger] actual[i] == #[trigger] reference[i]
        ==> actual[i + 1] == reference[i + 1]
}

proof fn finite_recurrence_prefix_v1(
    actual: Seq<int>,
    reference: Seq<int>,
    end: nat,
)
    requires
        finite_recurrence_contract_v1(actual, reference),
        end < actual.len(),
    ensures
        actual[end as int] == reference[end as int],
    decreases end,
{
    if end > 0 {
        finite_recurrence_prefix_v1(actual, reference, (end - 1) as nat);
        assert(0 <= end - 1 < actual.len() - 1);
    }
}

/// Shared finite-loop theorem used after the CFG pass binds the induction,
/// transition, and decreasing variant to the exact kernel loop.
pub proof fn finite_recurrence_refines_reference_v1(
    actual: Seq<int>,
    reference: Seq<int>,
)
    requires
        finite_recurrence_contract_v1(actual, reference),
    ensures
        actual == reference,
{
    assert forall|i: int| 0 <= i < actual.len() implies
        #[trigger] actual[i] == #[trigger] reference[i] by {
        finite_recurrence_prefix_v1(actual, reference, i as nat);
    }
    assert(actual =~= reference);
}

pub open spec fn additive_trace_v1(trace: Seq<int>, terms: Seq<int>) -> bool {
    &&& trace.len() == terms.len() + 1
    &&& trace[0] == 0
    &&& forall|i: int| 0 <= i < terms.len() ==>
        #[trigger] trace[i + 1] == trace[i] + terms[i]
}

proof fn identical_additive_traces_refine_v1(
    actual: Seq<int>,
    reference: Seq<int>,
    terms: Seq<int>,
)
    requires
        additive_trace_v1(actual, terms),
        additive_trace_v1(reference, terms),
    ensures
        actual == reference,
{
    assert(finite_recurrence_contract_v1(actual, reference)) by {
        assert forall|i: int| 0 <= i < actual.len() - 1
            && #[trigger] actual[i] == #[trigger] reference[i]
            implies actual[i + 1] == reference[i + 1] by {
            assert(i < terms.len());
        }
    }
    finite_recurrence_refines_reference_v1(actual, reference);
}

/// GEMM instantiation. `products[k]` is the typed MIR expression for one
/// `a[m,k] * b[k,n]` contribution under the selected numerical policy.
pub proof fn gemm_k_fold_refines_cpu_v1(
    gpu_accumulators: Seq<int>,
    cpu_accumulators: Seq<int>,
    products: Seq<int>,
)
    requires
        additive_trace_v1(gpu_accumulators, products),
        additive_trace_v1(cpu_accumulators, products),
    ensures
        gpu_accumulators == cpu_accumulators,
{
    identical_additive_traces_refine_v1(gpu_accumulators, cpu_accumulators, products);
}

pub open spec fn maximum_trace_v1(trace: Seq<int>, values: Seq<int>) -> bool {
    &&& values.len() > 0
    &&& trace.len() == values.len()
    &&& trace[0] == values[0]
    &&& forall|i: int| 0 <= i < values.len() - 1 ==> #[trigger] trace[i + 1]
        == if trace[i] < values[i + 1] { values[i + 1] } else { trace[i] }
}

/// Softmax instantiation for the maximum fold. The denominator and numerator
/// folds use the same additive-trace theorem over their typed MIR terms.
pub proof fn softmax_maximum_refines_cpu_v1(
    gpu_trace: Seq<int>,
    cpu_trace: Seq<int>,
    values: Seq<int>,
)
    requires
        maximum_trace_v1(gpu_trace, values),
        maximum_trace_v1(cpu_trace, values),
    ensures
        gpu_trace == cpu_trace,
{
    assert(finite_recurrence_contract_v1(gpu_trace, cpu_trace)) by {
        assert forall|i: int| 0 <= i < gpu_trace.len() - 1
            && #[trigger] gpu_trace[i] == #[trigger] cpu_trace[i]
            implies gpu_trace[i + 1] == cpu_trace[i + 1] by {
            assert(i + 1 < values.len());
        }
    }
    finite_recurrence_refines_reference_v1(gpu_trace, cpu_trace);
}

/// Flash-attention instantiation. Each term is the exact typed contribution
/// produced after the separately proved online-max/rescaling recurrence.
pub proof fn attention_value_recurrence_refines_cpu_v1(
    gpu_numerators: Seq<int>,
    cpu_numerators: Seq<int>,
    rescaled_value_terms: Seq<int>,
)
    requires
        additive_trace_v1(gpu_numerators, rescaled_value_terms),
        additive_trace_v1(cpu_numerators, rescaled_value_terms),
    ensures
        gpu_numerators == cpu_numerators,
{
    identical_additive_traces_refine_v1(
        gpu_numerators,
        cpu_numerators,
        rescaled_value_terms,
    );
}

pub open spec fn inverse_permutation_v1(
    mapping: Seq<nat>,
    inverse: Seq<nat>,
) -> bool {
    &&& mapping.len() == inverse.len()
    &&& forall|i: int| 0 <= i < mapping.len() ==> {
        &&& #[trigger] mapping[i] < mapping.len()
        &&& inverse[mapping[i] as int] == i
    }
}

/// MoE instantiation: an authenticated inverse map proves collision-free,
/// total token routing. Expert arithmetic is then a finite recurrence.
pub proof fn moe_routing_is_injective_v1(
    mapping: Seq<nat>,
    inverse: Seq<nat>,
    left: nat,
    right: nat,
)
    requires
        inverse_permutation_v1(mapping, inverse),
        left < mapping.len(),
        right < mapping.len(),
        mapping[left as int] == mapping[right as int],
    ensures
        left == right,
{
    assert(inverse[mapping[left as int] as int] == left);
    assert(inverse[mapping[right as int] as int] == right);
}

} // verus!

fn main() {}
