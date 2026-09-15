use super::*;

#[test]
fn amd_cache_nests_under_absolute_or_relative_outer_target() {
    let manifest = Path::new("/checkout/example");
    let cwd = Path::new("/caller");
    let suffix = "tutorial-ui/fe2o3-tiled-gemm-general-v1/amdgpu";
    for (outer, resolved) in [
        (Path::new("/shared/target"), Path::new("/shared/target")),
        (
            Path::new("relative-target"),
            Path::new("/caller/relative-target"),
        ),
    ] {
        let target = device_target_dir(manifest, Some(outer), cwd);
        assert_eq!(target, resolved.join(suffix));
        assert_ne!(target, resolved);
    }
}

#[test]
fn amd_cache_has_a_conventional_default_without_private_environment() {
    let manifest = Path::new("/checkout/example");
    assert_eq!(
        device_target_dir(manifest, None, Path::new("/caller")),
        manifest.join("target/tutorial-ui/fe2o3-tiled-gemm-general-v1/amdgpu")
    );
}

#[test]
fn cargo_plan_provisions_actual_amd_and_preserves_pinned_rustc_handoff() {
    let source = Path::new("/scratch/source");
    let target = Path::new("/shared/nested-target");
    let command = device_command(source, target);
    assert_eq!(command.get_current_dir(), Some(source));
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            "check",
            "--offline",
            "--lib",
            "-Zbuild-std=core",
            "--target",
            "amdgcn-amd-amdhsa",
            "--message-format=json",
        ]
        .map(std::ffi::OsStr::new)
    );
    let environment = command
        .get_envs()
        .collect::<std::collections::BTreeMap<_, _>>();
    assert!(!environment.contains_key(std::ffi::OsStr::new("RUSTC")));
    assert!(!environment.contains_key(std::ffi::OsStr::new("CARGO_BUILD_RUSTC")));
    assert_eq!(
        environment.get(std::ffi::OsStr::new("CARGO_TARGET_DIR")),
        Some(&Some(target.as_os_str()))
    );
    assert_eq!(
        environment.get(std::ffi::OsStr::new(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS"
        )),
        Some(&Some(std::ffi::OsStr::new(
            "-Ctarget-cpu=gfx950 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Cpanic=abort"
        )))
    );
    for wrapper in [
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    ] {
        assert_eq!(environment.get(std::ffi::OsStr::new(wrapper)), Some(&None));
    }
    let private_values = environment
        .iter()
        .filter_map(|(name, value)| {
            (name.to_str().unwrap().starts_with("FE2O3_") && value.is_some()).then_some(*name)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        private_values,
        [std::ffi::OsStr::new("FE2O3_CRATE_BINDING_ID_V1")]
    );
}

#[test]
fn fixture_messages_require_both_exact_target_name_and_source_path() {
    let source = Path::new("/scratch/source/src/lib.rs");
    let mut record = serde_json::json!({
        "target": {"name": "fixture", "src_path": source}
    });
    assert!(is_fixture_target(&record, "fixture", source));
    record["target"]["src_path"] = "/dependency/src/lib.rs".into();
    assert!(!is_fixture_target(&record, "fixture", source));
    record["target"]["src_path"] = source.to_str().unwrap().into();
    record["target"]["name"] = "dependency".into();
    assert!(!is_fixture_target(&record, "fixture", source));
}

#[test]
fn primary_substitution_span_accepts_cargo_relative_path_not_foreign_source() {
    let source = Path::new("/scratch/source/src/lib.rs");
    let line = "matrix.with_numerical_policy(policy)";
    let mut span = serde_json::json!({
        "is_primary": true, "file_name": "src/lib.rs",
        "text": [{"text": "    let _ = matrix.with_numerical_policy(policy);"}]
    });
    assert!(is_fixture_span(&span, source, line));
    span["file_name"] = source.to_str().unwrap().into();
    assert!(is_fixture_span(&span, source, line));
    span["file_name"] = "/dependency/src/lib.rs".into();
    assert!(!is_fixture_span(&span, source, line));
    span["file_name"] = "src/lib.rs".into();
    span["is_primary"] = false.into();
    assert!(!is_fixture_span(&span, source, line));
    span["is_primary"] = true.into();
    assert!(!is_fixture_span(&span, source, "unrelated_call()"));
}
