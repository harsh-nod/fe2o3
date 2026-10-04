//! Pure grammar/claim controls; never calls observe or opens a device.
use super::*;

fn valid() -> Vec<OsString> {
    [
        "--allow-vm-mapping",
        "--retain-until-process-exit",
        "/artifact.hsaco",
        "5536",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "kernel",
        "2",
        "16366993098680759275",
        "39903",
        "1111111111111111111111111111111111111111111111111111111111111111",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}
#[test]
fn exact_explicit_grammar_preserves_lossless_device_identity() {
    let value = parse(valid().into_iter()).unwrap();
    assert_eq!(value.unique_id, 16366993098680759275);
    assert_eq!(value.node, 2);
    assert_eq!(value.gpu_id, 39903);
}
#[test]
fn both_effect_acknowledgments_are_required_in_exact_order() {
    for index in 0..2 {
        let mut values = valid();
        values[index] = OsString::from("--other");
        assert!(parse(values.into_iter()).is_err());
    }
    let mut values = valid();
    values.swap(0, 1);
    assert!(parse(values.into_iter()).is_err());
}
#[test]
fn missing_extra_duplicate_flags_and_oversize_arguments_refuse() {
    let mut values = valid();
    values.pop();
    assert!(parse(values.into_iter()).is_err());
    let mut values = valid();
    values.push("--allow-vm-mapping".into());
    assert!(parse(values.into_iter()).is_err());
    let mut values = valid();
    values[5] = "x".repeat(4097).into();
    assert!(parse(values.into_iter()).is_err());
}
#[test]
fn exact_lowercase_digest_profiles_only() {
    for value in [
        String::new(),
        "A".repeat(64),
        "0".repeat(63),
        "g".repeat(64),
    ] {
        assert!(hash(&value).is_none());
    }
    assert_eq!(hash(&"ab".repeat(32)), Some([0xab; 32]));
}
#[test]
fn size_path_symbol_and_identity_bounds_are_enforced() {
    for (index, replacement) in [
        (2, "relative"),
        (3, "0"),
        (3, "67108865"),
        (3, "01"),
        (5, ""),
        (6, "4294967296"),
        (7, "0"),
        (7, "18446744073709551616"),
        (8, "0"),
        (8, "4294967296"),
    ] {
        let mut values = valid();
        values[index] = replacement.into();
        assert!(parse(values.into_iter()).is_err());
    }
}
#[test]
fn failures_do_not_imply_clean_native_teardown() {
    assert_eq!(
        Failure::new("arguments", "bad").native_preparation_effects,
        "not_attempted"
    );
    assert_eq!(
        Failure::new("cold_preparation", "bad")
            .possible()
            .native_preparation_effects,
        "possible_retained_until_process_exit"
    );
}
