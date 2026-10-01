//! Public dependency-free callback API type check.
use fe2o3_functional_proof::ParallelReferenceContractV1;

fn check(owner: &ParallelReferenceContractV1) -> Result<(), &'static str> {
    owner.visit_retained_heap_storage_v1(|count, width| {
        count.checked_mul(width).ok_or("overflow").map(|_| ())
    })
}

#[test]
fn parallel_heap_visitor_is_publicly_nameable() {
    let _: fn(&ParallelReferenceContractV1) -> Result<(), &'static str> = check;
}
