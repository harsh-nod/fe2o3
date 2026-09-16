#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn forged_capability_markers_cannot_gain_nominal_authority() {
    const PACKAGE: &str = "fe2o3-trusted-item-local-marker";
    const SPOOF: &str = "reserved-capability-spoof";
    const CONTROL: &str = "reserved-capability-control";
    const REFUSAL: &str = "unauthenticated reserved device type provider";
    const PENDING: &str =
        "target-neutral lowering remains pending; no fallback or artifact emission was entered";

    let target = ScratchTarget::new();
    for feature in [CONTROL, SPOOF] {
        let result = Command::new(env!("CARGO"))
            .current_dir(workspace())
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove("CARGO_BUILD_RUSTC_WRAPPER")
            .env_remove("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .args([
                "check",
                "--locked",
                "--offline",
                "--lib",
                "-p",
                PACKAGE,
                "--features",
                feature,
                "--target-dir",
            ])
            .arg(target.path().join("host"))
            .output()
            .expect("check authority marker frontend control");
        assert!(
            result.status.success(),
            "{feature} failed before the production importer:\n{}",
            String::from_utf8_lossy(&result.stderr),
        );

        let result = Command::new(env!("CARGO"))
            .current_dir(workspace())
            .env("RUSTC_WORKSPACE_WRAPPER", env!("CARGO_BIN_EXE_fe2o3-rustc-extract"))
            .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_trusted_item_local_marker")
            .env("FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2", "55".repeat(32))
            .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32")
            .args(["check", "--locked", "--offline", "--lib", "-p", PACKAGE, "--features", feature,
                "-Zbuild-std=core", "--target", "amdgcn-amd-amdhsa", "--target-dir"])
            .arg(target.path().join("device"))
            .output()
            .expect("import actual marked authority type");
        let stderr = String::from_utf8(result.stderr).expect("UTF-8 importer diagnostics");
        assert!(
            !result.status.success(),
            "{feature} unexpectedly acquired production authority"
        );
        let expected = if feature == CONTROL { PENDING } else { REFUSAL };
        assert!(
            stderr.contains(expected),
            "{feature} omitted {expected:?}:\n{stderr}"
        );
        if feature == SPOOF {
            for forbidden in [
                PENDING,
                "duplicate diagnostic item",
                "unknown kernel registration",
                "containing a user-provided unsafe block",
                "reaches unsafe function instance",
            ] {
                assert!(
                    !stderr.contains(forbidden),
                    "{feature} failed at {forbidden:?}:\n{stderr}"
                );
            }
        } else {
            assert!(
                !stderr.contains(REFUSAL),
                "ordinary local ZST was treated as authority:\n{stderr}"
            );
        }
    }
}
