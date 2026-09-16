#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn staged_generative_providers_reject_without_export_authority() {
    const REASON: &str = "reserved capability type has no authenticated production owner";
    let target = ScratchTarget::new();
    let build_dir = target.path().join("provider-target");
    let mut failures = Vec::new();
    for feature in [
        "provider_context",
        "provider_context_alias",
        "provider_context_nested",
        "provider_context_reference",
        "provider_context_empty_array",
        "provider_context_helper_result",
        "provider_workgroup",
        "provider_tile",
        "provider_fragment",
        "provider_tile_chain",
    ] {
        let bundle = target.path().join(format!("{feature}.fe2sim"));
        let result = output(
            simulation_export_command_for_feature("gfx942", &bundle, &build_dir, Some(5), feature),
            "reject staged nominal authority",
        );
        if result.status.success() || !result.stderr.contains(REASON) {
            failures.push(format!(
                "{feature} did not reach the nominal capability refusal:\n{}",
                result.stderr,
            ));
        }
        assert!(!bundle.exists(), "{feature} emitted a simulation bundle");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    let bundle = target.path().join("provider-gfx950.fe2sim");
    let result = output(
        simulation_export_command_for_feature(
            "gfx950",
            &bundle,
            &build_dir,
            Some(5),
            "provider_context",
        ),
        "reject staged authority independently of the AMD profile",
    );
    assert!(
        !result.status.success() && result.stderr.contains(REASON),
        "{}",
        result.stderr
    );
    assert!(!bundle.exists());

    let llvm = target.path().join("provider.ll");
    let mut command = base_command("check", &build_dir);
    command
        .env("FE2O3_EXTRACT_RANKED_MEMORY_V1", "1")
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_ranked_bounds_fixture",
        )
        .env("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1", &llvm)
        .args(["--features", "provider_context"]);
    let result = output(command, "reject staged authority before LLVM output");
    assert!(
        !result.status.success() && result.stderr.contains(REASON),
        "{}",
        result.stderr
    );
    assert!(!llvm.exists(), "rejected provider emitted LLVM");

    for feature in ["aggregate_zst", "provider_phantom"] {
        let control = target.path().join(format!("{feature}.fe2sim"));
        let result = output(
            simulation_export_command_for_feature("gfx942", &control, &build_dir, Some(5), feature),
            "preserve ordinary zero-sized source arguments",
        );
        assert!(
            result.status.success(),
            "{feature} rejected:\n{}",
            result.stderr
        );
        assert!(control.is_file());
    }
}
