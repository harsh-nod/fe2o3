fn run_static_publication_handoff_v1(source: &str) -> (std::process::Output, Option<Vec<u8>>) {
    let target = ScratchTarget::new();
    let fixture = materialize_source_safety_fixture(&target, source);
    let path = target.path().join("publication-semantic.handoff");
    let output = Command::new(env!("CARGO"))
        .current_dir(fixture)
        .env("RUSTC_WORKSPACE_WRAPPER", env!("CARGO_BIN_EXE_fe2o3-rustc-extract"))
        .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_production_source_safety_fixture")
        .env("FE2O3_EXTRACT_AMDGPU_COMPILER_HANDOFF_PATH_V1", &path)
        .env(
            "FE2O3_EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX",
            inert_invocation_v3::canonical_inert_gfx950_invocation_hex(),
        )
        .env("FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2", "55".repeat(32))
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env("CARGO_BUILD_JOBS", "2")
        .env_remove("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1")
        .env_remove("FE2O3_EXTRACT_GFX942_LLVM_PATH_V1")
        .env_remove("FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Zinline-mir=yes -Zmir-enable-passes=-JumpThreading -Copt-level=3 -Ctarget-cpu=gfx950 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32")
        .args(["check", "--offline", "-Zbuild-std=core", "--target", "amdgcn-amd-amdhsa", "--target-dir"])
        .arg(target.path().join("cargo"))
        .output()
        .expect("run actual publication source through semantic handoff extraction");
    let handoff = match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("read publication handoff output: {error}"),
    };
    (output, handoff)
}
