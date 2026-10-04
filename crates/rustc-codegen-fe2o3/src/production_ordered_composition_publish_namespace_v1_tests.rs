//! Pure namespace-policy regressions; no rustc/HIR owner is manufactured here.
use super::check_enclosing_names;

const NAME: &str = "__fe2o3_region_0123456789abcdef";

#[test]
fn empty_and_different_enclosing_names_are_not_collisions() {
    check_enclosing_names(std::iter::empty(), NAME).unwrap();
    check_enclosing_names(
        ["other", "__fe2o3_region_fedcba9876543210"].into_iter(),
        NAME,
    )
    .unwrap();
}

#[test]
fn exact_enclosing_name_collision_is_refused_without_a_publication_attempt() {
    let error = check_enclosing_names(["before", NAME, "after"].into_iter(), NAME).unwrap_err();
    assert_eq!(
        error.to_string(),
        "NotAttempted: publisher helper name collides in enclosing namespace"
    );
}

#[test]
fn final_name_at_exact_namespace_bound_is_still_checked() {
    let names = (0..4096).map(|index| if index == 4095 { NAME } else { "other" });
    let error = check_enclosing_names(names, NAME).unwrap_err();
    assert_eq!(
        error.to_string(),
        "NotAttempted: publisher helper name collides in enclosing namespace"
    );
}

#[test]
fn namespace_bound_is_checked_before_reading_names() {
    check_enclosing_names((0..4096).map(|_| "other"), NAME).unwrap();
    let names =
        (0..4097).map(|_| -> &str { panic!("an over-bound namespace must not inspect any name") });
    let error = check_enclosing_names(names, NAME).unwrap_err();
    assert_eq!(
        error.to_string(),
        "NotAttempted: publisher enclosing namespace bound exceeded"
    );
}

#[test]
fn namespace_matching_is_exact_not_a_prefix_or_substring() {
    check_enclosing_names(
        [
            "__fe2o3_region_0123456789abcdef_extra",
            "prefix__fe2o3_region_0123456789abcdef",
            "__fe2o3_region_0123456789abcde",
        ]
        .into_iter(),
        NAME,
    )
    .unwrap();
}
