//! Process-isolated cached-metadata driver, matching the policy-math importer probe.

use std::path::PathBuf;

const CRATE_NAME: &str = "output_slice_reference_source";
const METADATA: &str = "fe2o3-output-slice-reference-v1";
const CHILD_ENV: &str = "FE2O3_OUTPUT_SLICE_REFERENCE_CHILD";

// Relocates test source on the shared runner; it supplies no compiler authority.
pub(super) fn source_root() -> PathBuf {
    std::env::var_os("FE2O3_SOURCE_PROBE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .canonicalize()
        .expect("source probe repository")
}

fn registration_binding_v1() -> reserved_fe2o3_symbols::CrateBindingIdV1 {
    reserved_fe2o3_symbols::derive_crate_binding_id_v1(CRATE_NAME, [METADATA])
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-output-slice-reference-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("exclusively owned fixture directory");
        let scratch = Self(path);
        let device = source_root().join("crates/fe2o3-device");
        // Only proc_macro_crate reads this manifest. Nothing runs Cargo.
        std::fs::write(scratch.0.join("Cargo.toml"), format!(
            "[package]\nname = \"output-slice-reference-source\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nfe2o3-device = {{ path = {device:?} }}\n"
        )).unwrap();
        scratch
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

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

// Returns true only in the child that executes the callback.
pub(super) fn run_probe(
    cpu: &'static str,
    name: &str,
    probe: &mut (impl rustc_driver::Callbacks + Send),
) -> bool {
    run_probe_named(
        cpu,
        "reference_effect_v1::output_slice_compiler_tests",
        name,
        probe,
    )
}

pub(super) fn run_probe_named(
    cpu: &'static str,
    module: &str,
    name: &str,
    probe: &mut (impl rustc_driver::Callbacks + Send),
) -> bool {
    run_probe_configured(cpu, module, name, "kernel-combine-expert-ranks", probe)
}

pub(super) fn run_probe_configured(
    cpu: &'static str,
    module: &str,
    name: &str,
    feature: &str,
    probe: &mut (impl rustc_driver::Callbacks + Send),
) -> bool {
    use fe2o3_rustc_invocation::{
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, derive_cargo_metadata_build_observation_v2,
    };

    let device = configured("FE2O3_CORE_TRY_DEVICE_RMETA", false);
    let host_deps = configured("FE2O3_CORE_TRY_HOST_DEPS", true);
    let core = configured("FE2O3_CORE_TRY_AMDGPU_CORE", false);
    let builtins = configured("FE2O3_CORE_TRY_AMDGPU_BUILTINS", false);
    let observation = derive_cargo_metadata_build_observation_v2(&[METADATA]).to_hex();
    let child_key = format!("{cpu}:{observation}");
    if std::env::var(CHILD_ENV).ok().as_deref() != Some(child_key.as_str()) {
        let root = source_root();
        let scratch = Scratch::new();
        let test = format!("{module}::{name}");
        let output = crate::process_execution::capture_output(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &test,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .current_dir(&root)
                .env("FE2O3_SOURCE_PROBE_ROOT", &root)
                .env(CHILD_ENV, child_key)
                .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation)
                .env("FE2O3_CORE_TRY_DEVICE_RMETA", &device)
                .env("FE2O3_CORE_TRY_HOST_DEPS", &host_deps)
                .env("FE2O3_CORE_TRY_AMDGPU_CORE", &core)
                .env("FE2O3_CORE_TRY_AMDGPU_BUILTINS", &builtins)
                .env("CARGO_MANIFEST_DIR", &scratch.0)
                .env("CARGO_PKG_NAME", "output-slice-reference-source")
                .env(
                    reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1,
                    registration_binding_v1().to_hex(),
                )
                .env("FE2O3_SIMULATION_MODE_V1", "1")
                .env(
                    "FE2O3_SIMULATION_ATTEMPT_V1",
                    "92929292929292929292929292929292",
                ),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "output-slice source child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        return false;
    }
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        observation
    );
    assert_eq!(
        std::env::var(reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1).unwrap(),
        registration_binding_v1().to_hex(),
        "child macro environment must match its exact crate name and metadata"
    );
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        format!("--crate-name={CRATE_NAME}"),
        format!("--out-dir={}", std::env::var("CARGO_MANIFEST_DIR").unwrap()),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        format!("--cfg=feature={feature:?}"),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "--target=amdgcn-amd-amdhsa".into(),
        format!("-Ctarget-cpu={cpu}"),
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
    rustc_driver::run_compiler(&args, probe);
    true
}

#[test]
fn output_slice_registration_uses_session_derived_binding() {
    use reserved_fe2o3_symbols::derive_crate_binding_id_v1;

    assert_eq!(
        registration_binding_v1(),
        derive_crate_binding_id_v1(CRATE_NAME, [METADATA])
    );
    assert_ne!(
        registration_binding_v1(),
        derive_crate_binding_id_v1("different_output_slice_crate", [METADATA])
    );
    assert_ne!(
        registration_binding_v1(),
        derive_crate_binding_id_v1(CRATE_NAME, ["different_output_slice_metadata"])
    );
}
