use fe2o3_moe_top2_v1::{
    EXACT_PROFILE_V1, LayoutV1, MoeTop2ProfileV1, OverflowPolicyV1, ProfileMismatchV1,
    TieBreakPolicyV1, exact_launch_v1, logit_index_v1, route_id_v1, validate_profile_v1,
};

fn changed(update: impl FnOnce(&mut MoeTop2ProfileV1)) -> MoeTop2ProfileV1 {
    let mut profile = EXACT_PROFILE_V1;
    update(&mut profile);
    profile
}

#[test]
fn exact_profile_and_index_maps_are_frozen() {
    assert_eq!(validate_profile_v1(EXACT_PROFILE_V1), Ok(()));
    assert_eq!(exact_launch_v1(), ([1, 1, 1], [64, 1, 1]));
    assert_eq!(logit_index_v1(7, 3), Some(31));
    assert_eq!(logit_index_v1(8, 0), None);
    assert_eq!(route_id_v1(7, 1), Some(15));
    assert_eq!(route_id_v1(0, 2), None);
}

#[test]
fn target_launch_shape_and_layout_drift_fail_closed() {
    assert_eq!(
        validate_profile_v1(changed(|profile| profile.processor = "gfx950")),
        Err(ProfileMismatchV1::Target)
    );
    assert_eq!(
        validate_profile_v1(changed(|profile| profile.workgroup = [32, 1, 1])),
        Err(ProfileMismatchV1::Launch)
    );
    assert_eq!(
        validate_profile_v1(changed(|profile| profile.tokens = 7)),
        Err(ProfileMismatchV1::Shape)
    );
    assert_eq!(
        validate_profile_v1(changed(|profile| {
            profile.layout = LayoutV1::ExpertMajorContiguous;
        })),
        Err(ProfileMismatchV1::Layout)
    );
}

#[test]
fn tie_break_and_overflow_policy_drift_fail_closed() {
    assert_eq!(
        validate_profile_v1(changed(|profile| {
            profile.tie_break = TieBreakPolicyV1::HigherExpertIdWins;
        })),
        Err(ProfileMismatchV1::TieBreak)
    );
    assert_eq!(
        validate_profile_v1(changed(|profile| {
            profile.overflow = OverflowPolicyV1::ReplaceLowestAccepted;
        })),
        Err(ProfileMismatchV1::Overflow)
    );
}
