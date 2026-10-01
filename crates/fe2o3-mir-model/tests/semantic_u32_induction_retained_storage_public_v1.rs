//! Public dependency-free callback API type check.
use fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1;

fn check(owner: &SemanticU32InductionNoOverflowReportV1) -> Result<(), &'static str> {
    owner.visit_retained_heap_storage_v1(|count, width| {
        count.checked_mul(width).ok_or("overflow").map(|_| ())
    })
}

#[test]
fn induction_heap_visitor_is_publicly_nameable() {
    let _: fn(&SemanticU32InductionNoOverflowReportV1) -> Result<(), &'static str> = check;
}
