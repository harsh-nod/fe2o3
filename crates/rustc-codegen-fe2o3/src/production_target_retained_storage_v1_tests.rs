//! Compile/type coverage without fabricating authenticated rustc-session facts.
use super::super::AuthenticatedProductionTargetV1;

fn observe(owner: &AuthenticatedProductionTargetV1) -> Result<(), &'static str> {
    owner.visit_retained_heap_storage_v1(|count, width| {
        count.checked_mul(width).ok_or("arithmetic").map(|_| ())
    })
}

#[test]
fn authenticated_target_heap_api_is_nameable_without_constructing_authority() {
    let _: fn(&AuthenticatedProductionTargetV1) -> Result<(), &'static str> = observe;
}
