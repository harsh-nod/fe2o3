//! The read-only API is public even without internal-proof-staging constructors.
use fe2o3_functional_proof::ImportedFunctionalRefinementProofV2;

fn check(owner: &ImportedFunctionalRefinementProofV2) -> Result<(), &'static str> {
    owner.visit_retained_heap_storage_v1(|count, width| {
        count.checked_mul(width).ok_or("arithmetic").map(|_| ())
    })
}

#[test]
fn imported_receipt_heap_api_is_publicly_nameable() {
    let _: fn(&ImportedFunctionalRefinementProofV2) -> Result<(), &'static str> = check;
}
