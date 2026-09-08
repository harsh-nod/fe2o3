use vstd::prelude::*;

verus! {

pub enum AtomicMemoryDomainV1 {
    Coherent,
    NonCoherentFallback,
}

pub open spec fn mutated_fetch_add_memory_domain_v1() -> AtomicMemoryDomainV1 {
    AtomicMemoryDomainV1::NonCoherentFallback
}

pub proof fn mutated_fetch_add_retains_coherence_v1()
    ensures mutated_fetch_add_memory_domain_v1() == AtomicMemoryDomainV1::Coherent,
{
}

} // verus!
