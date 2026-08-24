use vstd::prelude::*;

verus! {

/// One abstract GPU write after target-neutral index reconstruction.
///
/// The payload is mathematical `int`, so this theorem is independent of the
/// source scalar type and of any particular kernel algorithm.
pub struct HierarchyWriteV1 {
    pub invocation: nat,
    pub subgroup: nat,
    pub workgroup: nat,
    pub lane: nat,
    pub coordinate: int,
    pub value: int,
}

/// Exact facts supplied jointly by the semantic-refinement and hierarchical-
/// ownership compiler passes.
pub open spec fn exact_reference_contract_v1(
    writes: Seq<HierarchyWriteV1>,
    reference: Seq<int>,
    subgroup_size: nat,
    workgroup_size: nat,
) -> bool {
    subgroup_size > 0
        && workgroup_size > 0
        && workgroup_size % subgroup_size == 0
        && writes.len() == reference.len()
        && forall|i: int| 0 <= i < writes.len() ==> {
            &&& #[trigger] writes[i].coordinate == i
            &&& writes[i].invocation == i as nat
            &&& writes[i].lane == writes[i].invocation % subgroup_size
            &&& writes[i].subgroup == writes[i].invocation / subgroup_size
            &&& writes[i].workgroup == writes[i].invocation / workgroup_size
            &&& writes[i].value == reference[i]
        }
}

/// Workload-neutral composition theorem used by every CPU reference.
///
/// It proves that exact, hierarchy-consistent, injective ownership plus the
/// per-coordinate semantic equality yields the complete reference output.
pub proof fn exact_hierarchy_writes_refine_safe_cpu_reference_v1(
    writes: Seq<HierarchyWriteV1>,
    reference: Seq<int>,
    subgroup_size: nat,
    workgroup_size: nat,
)
    requires
        exact_reference_contract_v1(
            writes,
            reference,
            subgroup_size,
            workgroup_size,
        ),
    ensures
        forall|coordinate: int| 0 <= coordinate < reference.len() ==> {
            &&& #[trigger] writes[coordinate].coordinate == coordinate
            &&& writes[coordinate].value == reference[coordinate]
            &&& writes[coordinate].lane == writes[coordinate].invocation % subgroup_size
            &&& writes[coordinate].subgroup == writes[coordinate].invocation / subgroup_size
            &&& writes[coordinate].workgroup == writes[coordinate].invocation / workgroup_size
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
        &&& writes[coordinate].lane == writes[coordinate].invocation % subgroup_size
        &&& writes[coordinate].subgroup == writes[coordinate].invocation / subgroup_size
        &&& writes[coordinate].workgroup == writes[coordinate].invocation / workgroup_size
    } by {
        assert(0 <= coordinate < writes.len());
        assert(writes[coordinate].coordinate == coordinate);
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

} // verus!
