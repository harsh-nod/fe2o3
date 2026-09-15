//! Isolated rustc callback against complete metadata; never builds dependencies.

use super::{Case, Probe};
use crate::test_temp_dir::TestTempDir;
use fe2o3_rustc_invocation::{
    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, derive_cargo_metadata_build_observation_v2,
};
use std::path::PathBuf;
use std::process::Command;

const METADATA: &str = "fe2o3-workgroup-index-mapping-v1";
const CRATE_NAME: &str = "workgroup_index_mapping_fixture";
const CHILD_ENV: &str = "FE2O3_WORKGROUP_INDEX_MAPPING_CHILD";
const OUTPUT_ENV: &str = "FE2O3_WORKGROUP_INDEX_MAPPING_OUTPUT";

fn configured(name: &str, directory: bool) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| {
        panic!("set {name} to complete cached metadata; no dependency build is performed")
    }));
    assert!(
        if directory {
            path.is_dir()
        } else {
            path.is_file()
        },
        "{name}: {}",
        path.display()
    );
    path.canonicalize().unwrap()
}

pub(super) fn run(case: Case, name: &str) {
    let device = configured("FE2O3_CORE_TRY_DEVICE_RMETA", false);
    let host_deps = configured("FE2O3_CORE_TRY_HOST_DEPS", true);
    let core = configured("FE2O3_CORE_TRY_AMDGPU_CORE", false);
    let builtins = configured("FE2O3_CORE_TRY_AMDGPU_BUILTINS", false);
    let observation = derive_cargo_metadata_build_observation_v2(&[METADATA]).to_hex();
    let key = format!("{name}:{observation}");
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    if std::env::var(CHILD_ENV).ok().as_deref() != Some(key.as_str()) {
        let scratch = TestTempDir::create("fe2o3-workgroup-index-mapping");
        let device_source = repository.join("crates/fe2o3-device");
        std::fs::write(scratch.path().join("Cargo.toml"), format!(
            "[package]\nname = \"workgroup-index-mapping-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nfe2o3-device = {{ path = {device_source:?} }}\n"
        )).unwrap();
        let test = format!(
            "collector::production_importer_v1::index_witness_mapping_v1::compiler_tests::{name}"
        );
        let output = crate::process_execution::capture_output(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &test,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .current_dir(&repository)
                .env(CHILD_ENV, key)
                .env(OUTPUT_ENV, scratch.path())
                .env("CARGO_MANIFEST_DIR", scratch.path())
                .env("CARGO_PKG_NAME", "workgroup-index-mapping-fixture")
                .env(
                    reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1,
                    registration_binding().to_hex(),
                )
                .env("FE2O3_SIMULATION_MODE_V1", "1")
                .env(
                    "FE2O3_SIMULATION_ATTEMPT_V1",
                    "93939393939393939393939393939393",
                )
                .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation)
                .env("FE2O3_CORE_TRY_DEVICE_RMETA", &device)
                .env("FE2O3_CORE_TRY_HOST_DEPS", &host_deps)
                .env("FE2O3_CORE_TRY_AMDGPU_CORE", &core)
                .env("FE2O3_CORE_TRY_AMDGPU_BUILTINS", &builtins),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "workgroup mapping child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        return;
    }
    assert_eq!(std::env::current_dir().unwrap(), repository);
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        observation
    );
    let scratch = configured(OUTPUT_ENV, true);
    assert_eq!(
        std::env::var(reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1).unwrap(),
        registration_binding().to_hex()
    );
    let sysroot = crate::process_execution::capture_output(
        Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        format!("--crate-name={CRATE_NAME}"),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        format!("--out-dir={}", scratch.display()),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "--target=amdgcn-amd-amdhsa".into(),
        "-Ctarget-cpu=gfx942".into(),
        "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
        "-Zno-codegen".into(),
        "-Zalways-encode-mir".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Zunstable-options".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-Cdebuginfo=2".into(),
        format!("-Cmetadata={METADATA}"),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-L".into(),
        format!("dependency={}", host_deps.display()),
        "--extern".into(),
        format!("noprelude,nounused:core={}", core.display()),
        "--extern".into(),
        format!(
            "noprelude,nounused:compiler_builtins={}",
            builtins.display()
        ),
        "-".into(),
    ];
    let mut probe = Probe {
        case,
        completed: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.completed, "actual compiler callback must finish");
}

pub(super) fn registration_binding() -> reserved_fe2o3_symbols::CrateBindingIdV1 {
    reserved_fe2o3_symbols::derive_crate_binding_id_v1(CRATE_NAME, [METADATA])
}
