mod ordinary_source_census_tests {
    use super::{ScratchTarget, materialize_source_safety_fixture, semantic_mir_sha256};
    use std::process::Command;

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target"]
    fn actual_ordinary_source_census_imports_scalar_shared_helper_and_mutable_aggregate() {
        for (name, body) in [
            (
                "scalar",
                r#"
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn scalar(_input: u32) {}
"#,
            ),
            (
                "shared_helper",
                r#"
#[inline(never)]
fn shared(value: u32) -> u32 { value ^ 17 }
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn first(input: u32) { assert!(shared(input) != 0); }
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn second(input: u32) { assert!(shared(input) != 1); }
"#,
            ),
            (
                "mutable_aggregate",
                r#"
#[inline(never)]
fn change(pair: &mut (u32, u32)) { pair.0 ^= pair.1; }
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn mutable_aggregate(input: u32) {
    let mut pair = (input, 17);
    change(&mut pair);
    assert!(pair.0 != pair.1);
}
"#,
            ),
        ] {
            let target = ScratchTarget::new();
            let source = format!("#![no_std]\nuse fe2o3_device::kernel;\n{body}");
            let fixture = materialize_source_safety_fixture(&target, &source);
            let mut command = Command::new(env!("CARGO"));
            for (key, _) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("FE2O3_EXTRACT_") {
                    command.env_remove(key);
                }
            }
            for key in [
                "RUSTFLAGS",
                "CARGO_ENCODED_RUSTFLAGS",
                "RUSTC_WRAPPER",
                "RUSTC_WORKSPACE_WRAPPER",
                "CARGO_BUILD_RUSTC_WRAPPER",
                "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
                "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
                "FE2O3_CRATE_BINDING_ID_V1",
                "FE2O3_DIAGNOSTIC_SOURCE_CENSUS_PATH_V1",
                "FE2O3_DIAGNOSTIC_SOURCE_CENSUS_RUN_ID_V1",
            ] {
                command.env_remove(key);
            }
            let output = command.current_dir(&fixture)
                .env("RUSTC_WORKSPACE_WRAPPER", env!("CARGO_BIN_EXE_fe2o3-rustc-extract"))
                .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_production_source_safety_fixture")
                .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                    "-Zalways-encode-mir -Copt-level=0 -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32")
                .args(["check", "--locked", "-Zbuild-std=core", "--target", "amdgcn-amd-amdhsa", "--target-dir"])
                .arg(target.path().join("build"))
                .output().unwrap_or_else(|error| panic!("{name}: run actual importer: {error}"));
            let stderr = String::from_utf8(output.stderr).expect("UTF-8 compiler diagnostic");
            // This existing extraction boundary deliberately stops after actual
            // source import/SSA, before parent-owned default pipeline activation.
            assert!(
                !output.status.success(),
                "{name}: expected extraction boundary:\n{stderr}"
            );
            assert_eq!(semantic_mir_sha256(&stderr).len(), 64);
            for expected in [
                "semantic importer authenticated rustc identity inventory",
                "then admitted one complete semantic MIR request",
                "canonical identity",
                "target-neutral lowering remains pending",
                "no fallback or artifact emission was entered",
            ] {
                assert!(
                    stderr.contains(expected),
                    "{name}: missing {expected}:\n{stderr}"
                );
            }
            for forbidden in [
                "semantic importer rejected semantic body construction",
                "function commitment seal completeness",
                "function commitment changed body",
                "authenticated workgroup scope custody",
                "scope custody initialization",
            ] {
                assert!(
                    !stderr.contains(forbidden),
                    "{name}: rejected real census: {forbidden}:\n{stderr}"
                );
            }
        }
    }
}
