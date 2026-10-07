// The actual borrowed Record predicate, not the enclosing account observation.
// Record is genuinely Copy in production; no token or owner is copied here.
// Exact pinned vector declarations cover all nineteen coordinates. The concrete
// Phase/Record schemas and counts projection below are matched to production by
// the source checker. Rust derive lowering and pinned vstd equality/array specs
// are trusted compiler boundaries, not proofs of compiler or ISA correctness.
// No mutex/Arc identity, freshness, domain ancestry, conservation, disposal,
// retained-token custody, Context admission or per-input validation is proved.

use vstd::prelude::verus as resource_vector_declarations_v1;
use vstd::prelude::*;
include!("../src/resource_vector_declarations.rs");
include!("../../fe2o3-resource-accounting/src/retained_charge_body.rs");

verus! {

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Reserved,
    Retained,
    Quarantined,
    Vacant,
}

impl vstd::std_specs::cmp::PartialEqSpecImpl for Phase {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

impl vstd::std_specs::cmp::PartialEqSpecImpl for R67ResourceVectorV1 {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

impl R67ResourceVectorV1 {
    // Same immutable projection as production; also used by vector mutants.
    fn counts(&self) -> (out: &[u64; R67_RESOURCE_DIMENSIONS_V1])
        ensures *out == self.counts,
    {
        &self.counts
    }
}

proof fn vector_equality_matches_view(actual: R67ResourceVectorV1, expected: R67ResourceVectorV1)
    ensures (actual == expected) == (actual.counts@ == expected.counts@),
{
    if actual.counts@ == expected.counts@ {
        assert forall|i: int| 0 <= i < R67_RESOURCE_DIMENSIONS_V1
            implies #[trigger] actual.counts[i] == expected.counts[i] by {
            vstd::array::lemma_array_index(actual.counts, i);
            vstd::array::lemma_array_index(expected.counts, i);
        }
        vstd::array::axiom_array_ext_equal(actual.counts, expected.counts);
        assert(actual.counts =~= expected.counts);
    }
}

#[derive(Clone, Copy)]
struct Record {
    owner: u64,
    charge: R67ResourceVectorV1,
    phase: Phase,
}

impl Record {
    fn matches_retained_charge(&self, owner: u64, expected: R67ResourceVectorV1)
        -> (out: bool)
        ensures out == (owner != 0 && self.owner == owner
            && self.phase == Phase::Retained && self.charge.counts@ == expected.counts@),
    {
        proof { vector_equality_matches_view(self.charge, expected); }
        retained_credit_record_matches_body!(self, owner, expected)
    }
}

}
