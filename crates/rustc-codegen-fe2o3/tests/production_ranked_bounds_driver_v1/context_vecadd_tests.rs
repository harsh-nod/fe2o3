#[test]
#[ignore = "requires pinned nightly rust-src and AMD source compilation; no execution qualification"]
fn context_vecadd_normal_exports_require_checked_materialization() {
    let scratch = ScratchTarget::new();
    let source = workspace().join(
        "crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device/context_vecadd.rs",
    );
    for target in ["gfx942", "gfx950"] {
        let build_dir = scratch.path().join(target);
        let bundle = scratch.path().join(format!("{target}.fe2sim"));
        let mut command = simulation_export_command_for_feature(
            target,
            &bundle,
            &build_dir,
            Some(5),
            "provider_context_protocol",
        );
        command.env("FE2O3_CONTEXT_PROTOCOL_SOURCE", &source);
        let result = output(
            command,
            "context vecadd through the normal simulation export",
        );
        assert!(
            !result.status.success()
                && result
                    .stderr
                    .contains(EXECUTION_ROLE_MATERIALIZATION_PENDING),
            "{target} vecadd did not reach checked-materialization refusal:\n{}",
            result.stderr
        );
        assert!(!bundle.exists(), "rejected context vecadd emitted a bundle");

        let llvm = scratch.path().join(format!("{target}.ll"));
        let mut command = base_command("check", &build_dir);
        command
            .env_remove("FE2O3_EXTRACT_RANKED_MEMORY_V1")
            .env(
                "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                format!("-Zalways-encode-mir -Ctarget-cpu={target} -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32"),
            )
            .env("RUSTC_WORKSPACE_WRAPPER", env!("CARGO_BIN_EXE_fe2o3-rustc-extract"))
            .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_production_ranked_bounds_fixture")
            .env("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1", &llvm)
            .env("FE2O3_CONTEXT_PROTOCOL_SOURCE", &source)
            .args(["--features", "provider_context_protocol"]);
        let result = output(command, "context vecadd through normal target lowering");
        assert!(
            !result.status.success()
                && result
                    .stderr
                    .contains(EXECUTION_ROLE_MATERIALIZATION_PENDING),
            "{target} vecadd stopped at an unexpected LLVM boundary:\n{}",
            result.stderr
        );
        assert!(!llvm.exists(), "rejected context vecadd emitted LLVM");
    }
}
