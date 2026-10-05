use super::check_native_v18_text_descriptor_relation_v89 as check_current;
#[path = "native_v18_replay_fixture_v89.rs"]
mod fixture;
const SECTION_NAME: &str = ".fe2o3.kd.v89";
include!("native_v18_text_descriptor_replay_family_tests.rs");

#[test]
fn predicated_native_replay_keeps_descriptor_families_separate() {
    let (owner, retained) = fixture::owner(2, "predicated-native-family");
    let prefix = lower_942(&owner).unwrap();
    let wire = fixture::descriptor(&owner, Profile::Gfx942);
    let table =
        fe2o3_kernel_descriptor::decode_mixed_descriptor_v89(
            &wire,
            &mut |_: usize| Ok::<_, ()>(()),
        )
        .unwrap();
    assert_eq!(table.kernel_count(), 2);
    assert!(
        fe2o3_kernel_descriptor::decode_mixed_descriptor_v53(
            &wire,
            &mut |_: usize| Ok::<_, ()>(()),
        )
        .is_err()
    );
    let text = fixture::append_descriptor(&prefix, &wire);
    let legacy_section = text.replace(".fe2o3.kd.v89", ".fe2o3.kd.v53");
    assert!(
        run(
            &owner,
            retained,
            Profile::Gfx942,
            prefix.as_bytes(),
            &wire,
            &legacy_section,
            LIMIT,
            LIMIT
        )
        .0
        .is_err()
    );
    let backing = FLOOR + retained + prefix.len() + wire.len() + text.len();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(backing).unwrap();
    assert!(
        crate::check_native_v18_text_descriptor_relation_v60(
            &owner,
            Profile::Gfx942,
            prefix.as_bytes(),
            &wire,
            &text,
            &mut budget,
        )
        .is_err()
    );
    assert_eq!(budget.storage(), backing);
}
