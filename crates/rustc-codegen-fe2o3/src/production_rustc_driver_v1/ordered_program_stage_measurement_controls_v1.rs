//! Component controls only. None constructs a substitute live source owner.
use super::*;

#[test]
fn exact_fixture_profiles_have_only_one_three_or_sixteen_steps() {
    for profile in [Profile::One, Profile::Three, Profile::Sixteen] {
        assert!(profile.matches(profile.steps(), &profile.descriptors()));
        assert_eq!(Profile::from_steps(profile.steps()), Ok(profile));
    }
    for steps in [0, 2, 4, 15, 17, 255] {
        assert_eq!(Profile::from_steps(steps), Err(Failure::Request));
    }
}
#[test]
fn descriptor_drift_and_padding_do_not_match_a_source_profile() {
    for profile in [Profile::One, Profile::Three, Profile::Sixteen] {
        let mut words = profile.descriptors();
        for index in 0..16 {
            words[index] ^= 1;
            assert!(!profile.matches(profile.steps(), &words));
            words[index] ^= 1;
        }
        assert!(!profile.matches(profile.steps().saturating_add(1), &words));
    }
}
#[test]
fn fixed_three_and_sixteen_descriptor_order_is_explicit() {
    assert_eq!(Profile::Three.descriptors()[..3], [0x85, 0x133, 0x19d]);
    assert_eq!(
        Profile::Sixteen.descriptors(),
        [
            0, 0x8d, 0x143, 0xbc, 0x141, 0x3a, 0x40, 0xb5, 0x3c, 0x14b, 0x49, 0xc2, 0x38, 0x14d,
            0x10, 0x48
        ]
    );
}
#[test]
fn callback_completion_never_accepts_missing_reentry_or_later_fatal() {
    assert_eq!(finish(Some(Ok(7_u8)), 1, false), Ok(7));
    assert_eq!(
        finish(Some(Ok(7_u8)), 0, false),
        Err(Failure::MissingCallback)
    );
    assert_eq!(
        finish(Some(Ok(7_u8)), 2, false),
        Err(Failure::RepeatedCallback)
    );
    assert_eq!(finish(Some(Ok(7_u8)), 1, true), Err(Failure::CompilerFatal));
    assert_eq!(finish::<u8>(None, 1, false), Err(Failure::MissingCallback));
    assert_eq!(
        finish(Some(Err::<u8, _>(Failure::Validation)), 1, false),
        Err(Failure::Validation)
    );
}
#[test]
fn timer_calls_the_selected_closure_exactly_once_and_preserves_result() {
    let calls = std::cell::Cell::new(0);
    let (value, _elapsed) = timed(|| {
        calls.set(calls.get() + 1);
        Err::<(), _>(Failure::Validation)
    });
    assert_eq!(calls.get(), 1);
    assert_eq!(value, Err(Failure::Validation));
}
#[test]
fn request_limits_do_not_call_a_compiler_or_infer_source() {
    let args = vec!["rustc".to_owned(), "-Coverflow-checks=on".to_owned()];
    assert!(request_ok(&args, b"component-only-not-a-canonical-module"));
    assert!(!request_ok(&args, b""));
    assert!(!request_ok(&[], b"x"));
    let mut repeated = args.clone();
    repeated.push("-Coverflow-checks=on".into());
    assert!(!request_ok(&repeated, b"x"));
    let mut nul = args.clone();
    nul.push("bad\0argument".into());
    assert!(!request_ok(&nul, b"x"));
    let mut too_long = args.clone();
    too_long.push("x".repeat(4097));
    assert!(!request_ok(&too_long, b"x"));
    let many = vec!["x".to_owned(); MAX_ARGS + 1];
    assert!(!request_ok(&many, b"x"));
}
#[test]
fn rejected_request_has_no_invented_zero_duration_or_generation_receipt() {
    let attempt = run(&[], Profile::One, b"");
    assert_eq!(attempt.failure, Some(Failure::Request));
    assert_eq!(attempt.compiler_invocations, 0);
    assert_eq!(attempt.after_analysis_calls, 0);
    assert!(attempt.compiler_call_ns.is_none());
    assert!(attempt.source_collection_ns.is_none());
    assert!(attempt.original_validation_transition_ns.is_none());
    assert!(attempt.generation_ns.is_none());
    assert!(!attempt.warm_stage);
    assert!(!attempt.generation_or_whole_storage_target_qualified);
    assert!(attempt.observation.is_none());
}
fn scratch() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "fe2o3-ordered-stage-component-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ));
    fs::create_dir(&path).unwrap();
    fs::canonicalize(path).unwrap()
}
#[test]
fn retained_baseline_is_exact_and_refuses_mutation_symlink_or_oversize() {
    let root = scratch();
    let file = root.join("baseline");
    fs::write(&file, b"component-only").unwrap();
    let mut retained = RetainedBytes::open(&file, 64).unwrap();
    assert_eq!(retained.bytes, b"component-only");
    retained.recheck().unwrap();
    assert!(RetainedBytes::open(&file, 1).is_err());
    let link = root.join("link");
    std::os::unix::fs::symlink(&file, &link).unwrap();
    assert!(RetainedBytes::open(&link, 64).is_err());
    fs::write(&file, b"changed").unwrap();
    assert!(retained.recheck().is_err());
    drop(retained);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn probe_config_rejects_unknown_and_duplicate_fields() {
    let source = r#"{"schema":"fe2o3-ordered-stage-measurement-input-v1","steps":1,
        "rustc_args":["rustc","-Coverflow-checks=on"],"baseline_path":"/inert/baseline",
        "baseline_bytes":1,"baseline_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#;
    assert!(serde_json::from_str::<Input>(source).is_ok());
    let unknown = source.replacen("{", "{\"unexpected\":true,", 1);
    assert!(serde_json::from_str::<Input>(&unknown).is_err());
    let duplicate = source.replacen("{", "{\"steps\":3,", 1);
    assert!(serde_json::from_str::<Input>(&duplicate).is_err());
}
#[test]
fn source_wrapper_calls_original_transition_without_generation_or_query_substitution() {
    let source = include_str!("ordered_program_stage_measurement_v1_tests.rs");
    assert_eq!(
        source
            .matches("transaction.observe_ordered_program_v32()")
            .count(),
        1
    );
    assert!(
        source.contains("Ok(owner) => observe_live_owner(&owner, self.profile, self.baseline)")
    );
    for forbidden in [
        "override_queries",
        "from_packed(",
        "__checked_ordered_program_words_v1(",
        "ProductionSemanticMirOwnerV1::try_new(",
        "try_materialize_with_budget(",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
    assert!(source.contains("complete_retained_owner_bytes: None"));
    assert!(source.contains("generation_ns: None"));
}
