use vstd::prelude::*;

verus! {

pub open spec fn source_inactive_publication_v2() -> int { 0 }
pub open spec fn mutated_cpu_inactive_publication_v2() -> int { 1 }

pub proof fn mutated_cpu_inactive_publication_is_positive_zero_v2()
    ensures source_inactive_publication_v2() == mutated_cpu_inactive_publication_v2(),
{
}

}
