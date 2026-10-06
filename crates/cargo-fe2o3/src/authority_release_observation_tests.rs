use std::os::unix::ffi::OsStringExt as _;

fn observation_environment() -> Vec<(OsString, OsString)> {
    [("LANG", "C"), ("LC_ALL", "C"), ("TZ", "UTC")]
        .map(|(name, value)| (name.into(), value.into()))
        .to_vec()
}

fn observation_selectors() -> [(String, String); 3] {
    [
        (
            crate::build_config::PRODUCTION_BUILD_EXPECTED_ID_ENV.into(),
            "12".repeat(32),
        ),
        (
            crate::build_config::PRODUCTION_BUILD_EXPECTED_ID_V2_ENV.into(),
            "34".repeat(32),
        ),
        (
            crate::production_graph_capture_v92::DIRECTORY_ENV.into(),
            "/private/graphs".into(),
        ),
    ]
}

#[test]
fn release_admits_existing_config_pins_and_inert_graph_observation() {
    let base = observation_environment();
    assert!(validate_release_environment_values(&base).is_ok());
    for (name, value) in observation_selectors() {
        let mut values = base.clone();
        values.push((name.into(), value.into()));
        assert!(validate_release_environment_values(&values).is_ok());
    }
    // Admission names a supported channel; config-family/value checks still run
    // in the existing build parser, and graph capture retains its bounded writer.
    let mut all = base;
    all.extend(observation_selectors().map(|(name, value)| (name.into(), value.into())));
    assert!(validate_release_environment_values(&all).is_ok());
}

#[test]
fn release_observation_does_not_admit_loader_toolchain_or_unknown_channels() {
    for name in [
        "LD_LIBRARY_PATH",
        "LD_PRELOAD",
        "PATH",
        "HOME",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTFLAGS",
        "CC",
        "CARGO_NET_OFFLINE",
        "FE2O3_TUTORIAL_GRAPH_CAPTURE_V93",
        "FE2O3_PRODUCTION_BUILD_EXPECTED_ID_V3",
    ] {
        let mut values = observation_environment();
        values.push((name.into(), "hostile".into()));
        let error = validate_release_environment_values(&values).unwrap_err();
        assert!(
            error.contains("unexpected inherited environment") && error.contains(name),
            "{error}"
        );
    }
    let mut values = observation_environment();
    values.push((OsString::from_vec(vec![0xff]), "hostile".into()));
    assert!(
        validate_release_environment_values(&values)
            .unwrap_err()
            .contains("non-UTF-8")
    );
}

#[test]
fn release_environment_refusal_does_not_disclose_unknown_values() {
    for secret in [
        OsString::from("example-secret-token"),
        OsString::from("secret\nwith\"escaped content"),
        OsString::from_vec(vec![0xff, 0xfe, b's', b'e', b'c', b'r', b'e', b't']),
        OsString::new(),
    ] {
        let mut values = observation_environment();
        values.push(("FE2O3_TEST_SECRET".into(), secret));
        assert_eq!(
            validate_release_environment_values(&values).unwrap_err(),
            "authority release rejects unexpected inherited environment \"FE2O3_TEST_SECRET\""
        );
        values.last_mut().unwrap().0 = OsString::from_vec(vec![0xff]);
        assert_eq!(
            validate_release_environment_values(&values).unwrap_err(),
            "authority release rejects a non-UTF-8 environment name"
        );
    }
}

#[test]
fn release_locale_refusal_does_not_disclose_observed_values() {
    for (index, expected) in ["LANG=C", "LC_ALL=C", "TZ=UTC"].into_iter().enumerate() {
        let mut values = observation_environment();
        values[index].1 = "example-secret-token".into();
        assert_eq!(
            validate_release_environment_values(&values).unwrap_err(),
            format!("authority release requires exact environment {expected}")
        );
    }
}

#[test]
fn release_observation_preserves_exact_locale_requirements() {
    for index in 0..3 {
        let mut values = observation_environment();
        let name = values[index].0.to_string_lossy().into_owned();
        values.remove(index);
        assert!(
            validate_release_environment_values(&values)
                .unwrap_err()
                .contains(&name)
        );
        values.push((name.clone().into(), "different".into()));
        assert!(
            validate_release_environment_values(&values)
                .unwrap_err()
                .contains(&name)
        );
    }
}

#[test]
fn release_observation_selectors_are_sealed_and_rechecked_in_the_child() {
    for (name, value) in observation_selectors() {
        let mut record = contract();
        record
            .environment
            .push((name.into_bytes(), value.into_bytes()));
        record.environment.sort();
        let encoded = record.encode().unwrap();
        let (decoded, identity) = ReleaseContract::decode(&encoded).unwrap();
        assert_eq!(decoded, record);
        assert!(validate_child_observation(&record, &observation(&record)).is_ok());
        let mut changed = observation(&record);
        changed.environment[0].1.push(b'x');
        assert!(
            validate_child_observation(&record, &changed)
                .unwrap_err()
                .contains("environment")
        );
        let mut replacement = record;
        replacement.environment = changed.environment;
        let (_, replacement_identity) =
            ReleaseContract::decode(&replacement.encode().unwrap()).unwrap();
        assert_ne!(identity, replacement_identity);
    }
}
