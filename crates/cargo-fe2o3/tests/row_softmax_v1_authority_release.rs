#![cfg(target_os = "linux")]

use std::env;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CONFIG_ENV: &str = "FE2O3_G3_ROW_SOFTMAX_V1_CONFIG";
const CARGO_ENV: &str = "FE2O3_G3_ROW_SOFTMAX_V1_CARGO";
const RUSTC_ENV: &str = "FE2O3_G3_ROW_SOFTMAX_V1_RUSTC";
const RUSTC_RUNTIME_SHA256_ENV: &str = "FE2O3_G3_ROW_SOFTMAX_V1_RUSTC_RUNTIME_SHA256";
const BACKEND_ENV: &str = "FE2O3_G3_ROW_SOFTMAX_V1_BACKEND";
const SUCCESS: &str = "FE2O3_PROTECTED_ROW_SOFTMAX_V1_OK";
const FIXTURE: &str = "../rustc-codegen-fe2o3/tests/fixtures/collected-row-softmax-v1/Cargo.toml";

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = env::temp_dir().join(format!(
            "cargo-fe2o3-row-softmax-release-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create private row-softmax release test directory");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .expect("make row-softmax release test directory private");
        Self(path)
    }

    fn config(&self, name: &str, mutate: impl FnOnce(&mut Value)) -> PathBuf {
        let source = required_path(CONFIG_ENV);
        let bytes = fs::read(source).expect("read exact row-softmax base config");
        let mut value: Value = serde_json::from_slice(&bytes).expect("decode base config");
        mutate(&mut value);
        let path = self.0.join(format!("{name}.json"));
        fs::write(
            &path,
            serde_json::to_vec(&value).expect("encode canonical config"),
        )
        .expect("write exact row-softmax config");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("make row-softmax config private");
        path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn required_path(name: &str) -> PathBuf {
    let value =
        env::var_os(name).unwrap_or_else(|| panic!("required no-skip input {name} is absent"));
    let path = PathBuf::from(value);
    assert!(path.is_absolute(), "{name} must be absolute");
    fs::canonicalize(&path).unwrap_or_else(|error| panic!("cannot resolve {name}: {error}"))
}

fn required_sha256(name: &str) -> String {
    let value =
        env::var(name).unwrap_or_else(|_| panic!("required no-skip input {name} is absent"));
    assert!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{name} must be a lowercase SHA-256 digest"
    );
    value
}

fn file_sha256(path: &Path) -> String {
    let bytes =
        fs::read(path).unwrap_or_else(|error| panic!("cannot hash {}: {error}", path.display()));
    hex(&Sha256::digest(bytes))
}

fn fixture_manifest() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    fs::canonicalize(manifest).expect("resolve attributed row-softmax fixture")
}

fn release(config: &Path, target_dir: &Path, target: &str) -> Output {
    let cargo = required_path(CARGO_ENV);
    let rustc = required_path(RUSTC_ENV);
    let backend = required_path(BACKEND_ENV);
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .env_clear()
        .args([
            OsString::from("authority"),
            OsString::from("release"),
            OsString::from("run"),
            OsString::from("--manifest-path"),
            fixture_manifest().into_os_string(),
            OsString::from("--target-dir"),
            target_dir.as_os_str().to_owned(),
        ])
        .env("CARGO", &cargo)
        .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", file_sha256(&cargo))
        .env("FE2O3_AUTHORITY_RUSTC_PATH_V1", &rustc)
        .env("FE2O3_AUTHORITY_RUSTC_SHA256_V1", file_sha256(&rustc))
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            required_sha256(RUSTC_RUNTIME_SHA256_ENV),
        )
        .env("FE2O3_BACKEND", &backend)
        .env("FE2O3_AUTHORITY_BACKEND_SHA256_V1", file_sha256(&backend))
        .env("FE2O3_CODEGEN_PIPELINE", "collected-row-softmax-v1")
        .env("FE2O3_TARGET", target)
        .env("FE2O3_WORKER_V2_CONFIG_V2", config)
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("TZ", "UTC");
    command
        .output()
        .expect("execute protected row-softmax release")
}

fn assert_success(output: Output, case: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{case}: {stderr}");
    assert!(stderr.contains(SUCCESS), "{case}: {stderr}");
    assert!(
        stderr.contains(&format!("case={}", title_case(case))),
        "{case}: {stderr}"
    );
    assert!(stderr.contains("pins=25"), "{case}: {stderr}");
    assert!(stderr.contains("source_tested=true"), "{case}: {stderr}");
    assert!(
        stderr.contains("verus_refinement=false"),
        "{case}: {stderr}"
    );
}

fn assert_rejected(output: Output, expected_stage: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "unexpected success: {stderr}");
    assert!(
        stderr.contains(expected_stage),
        "missing {expected_stage:?}: {stderr}"
    );
    assert!(
        !stderr.contains(SUCCESS),
        "rejection reached terminal launch: {stderr}"
    );
}

fn title_case(value: &str) -> String {
    let mut bytes = value.as_bytes().to_vec();
    bytes[0].make_ascii_uppercase();
    String::from_utf8(bytes).expect("ASCII case name")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
#[ignore = "requires exact upstream-LLVM worker/compiler pins and a local gfx942:xnack- MI300X"]
fn exact_gfx942_authority_release_run_covers_positive_and_hostile_profiles() {
    let directory = TestDirectory::new();

    for case in ["normal", "equal", "dominant"] {
        let config = directory.config(case, |value| {
            value["row_softmax_v1"]["case"] = json!(case);
        });
        assert_success(
            release(&config, &directory.0.join(case), "gfx942:xnack-"),
            case,
        );
    }

    let masked = directory.config("masked", |value| {
        value["row_softmax_v1"]["mask"] = json!("alternating");
    });
    assert_rejected(
        release(&masked, &directory.0.join("masked-target"), "gfx942:xnack-"),
        "stage=workload-mask",
    );

    let exceptional = directory.config("exceptional", |value| {
        value["row_softmax_v1"]["case"] = json!("exceptional");
    });
    assert_rejected(
        release(
            &exceptional,
            &directory.0.join("exceptional-target"),
            "gfx942:xnack-",
        ),
        "stage=cpu-oracle",
    );

    let shape = directory.config("shape", |value| {
        value["row_softmax_v1"]["row_elements"] = json!(63);
    });
    assert_rejected(
        release(&shape, &directory.0.join("shape-target"), "gfx942:xnack-"),
        "stage=workload-shape",
    );

    let policy = directory.config("policy", |value| {
        value["row_softmax_v1"]["comparison_policy"] = json!("wrong-policy");
    });
    assert_rejected(
        release(&policy, &directory.0.join("policy-target"), "gfx942:xnack-"),
        "stage=workload-policy",
    );

    let provider = directory.config("provider", |value| {
        let stable = value["row_softmax_v1"]["provider_stable_crate_id"]
            .as_u64()
            .expect("provider stable crate id");
        value["row_softmax_v1"]["provider_stable_crate_id"] = json!(stable + 1);
    });
    assert_rejected(
        release(
            &provider,
            &directory.0.join("provider-target"),
            "gfx942:xnack-",
        ),
        "stage=authority-policy",
    );

    let artifact = directory.config("artifact", |value| {
        value["worker"]["sha256"] = json!("01".repeat(32));
    });
    assert_rejected(
        release(
            &artifact,
            &directory.0.join("artifact-target"),
            "gfx942:xnack-",
        ),
        "stage=worker-artifact",
    );

    let target = directory.config("target", |_| {});
    assert_rejected(
        release(&target, &directory.0.join("target-target"), "gfx1100"),
        "requires FE2O3_TARGET=gfx942:xnack-",
    );
}
