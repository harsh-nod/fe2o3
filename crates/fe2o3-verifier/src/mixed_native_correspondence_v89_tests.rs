use super::check_mixed_native_correspondence_v89 as check_current;
#[path = "../../fe2o3-amdgcn-model/src/native_v18_replay_fixture_v89.rs"]
mod fixture;
include!("mixed_native_correspondence_family_tests.rs");

#[test]
fn predicated_native_correspondence_refuses_legacy_section_with_rebound_receipts() {
    let mut fixture = Fixture::new(Profile::Gfx942, 2);
    fixture.text = fixture.text.replace(".fe2o3.kd.v89", ".fe2o3.kd.v53");
    let outer = fixture.outer(|_| {});
    assert!(matches!(
        fixture.run(&outer, LIMIT, LIMIT).0,
        Err(Error::Native(_))
    ));
}
