#![cfg(target_os = "linux")]

use std::env;
use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CARGO_ENV: &str = "FE2O3_G3_ROW_SOFTMAX_V1_CARGO";
const RUSTC_ENV: &str = "FE2O3_G3_ROW_SOFTMAX_V1_RUSTC";
const BACKEND_ENV: &str = "FE2O3_G3_ROW_SOFTMAX_V1_BACKEND";
const WORKER_ENV: &str = "FE2O3_G3_ROW_SOFTMAX_V1_WORKER";
const WORKER_BUILD_ID_ENV: &str = "FE2O3_G3_ROW_SOFTMAX_V1_WORKER_BUILD_ID";
const LLVM_BUILD_ID_ENV: &str = "FE2O3_G3_ROW_SOFTMAX_V1_LLVM_BUILD_ID";
const OCML_DIRECTORY_ENV: &str = "FE2O3_G3_ROW_SOFTMAX_V1_OCML_DIRECTORY";
const SUCCESS: &str = "FE2O3_PROTECTED_ROW_SOFTMAX_V1_OK";
const PROVIDER_OBSERVATION: &str = "FE2O3_ROW_SOFTMAX_V1_PROVIDER_OBSERVATION=";
const FIXTURE: &str = "../rustc-codegen-fe2o3/tests/fixtures/collected-row-softmax-v1/Cargo.toml";
const OCML_BASENAMES: [&str; 4] = [
    "ocml.bc",
    "oclc_isa_version_942.bc",
    "oclc_unsafe_math_off.bc",
    "oclc_finite_only_off.bc",
];

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);
static RUSTC_RUNTIME_SHA256: OnceLock<String> = OnceLock::new();

struct TestDirectory {
    path: PathBuf,
    base_config: Value,
}

impl TestDirectory {
    fn new() -> Self {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        let path = env::temp_dir().join(format!(
            "cargo-fe2o3-row-softmax-release-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create private row-softmax release test directory");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .expect("make row-softmax release test directory private");
        let provider = provision_provider(&path.join("provision-target"));
        let base_config = exact_worker_config(provider);
        Self { path, base_config }
    }

    fn config(&self, name: &str, mutate: impl FnOnce(&mut Value)) -> PathBuf {
        let mut value = self.base_config.clone();
        mutate(&mut value);
        let path = self.path.join(format!("{name}.json"));
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
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn required_path(name: &str) -> PathBuf {
    let value =
        env::var_os(name).unwrap_or_else(|| panic!("required no-skip input {name} is absent"));
    let path = PathBuf::from(value);
    assert!(path.is_absolute(), "{name} must be absolute");
    fs::canonicalize(&path).unwrap_or_else(|error| panic!("cannot resolve {name}: {error}"))
}

fn required_text(name: &str) -> String {
    let value =
        env::var(name).unwrap_or_else(|_| panic!("required no-skip input {name} is absent"));
    assert!(
        !value.is_empty()
            && value.len() <= 160
            && value.is_ascii()
            && !value.bytes().any(|byte| byte.is_ascii_control()),
        "{name} must be bounded printable ASCII"
    );
    value
}

fn file_sha256(path: &Path) -> String {
    let bytes =
        fs::read(path).unwrap_or_else(|error| panic!("cannot hash {}: {error}", path.display()));
    hex(&Sha256::digest(bytes))
}

fn rustc_runtime_sha256(rustc: &Path) -> String {
    fn hash_field(hash: &mut Sha256, value: &[u8]) {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value);
    }

    fn hash_directory(hash: &mut Sha256, directory: &Path) {
        let mut entries = fs::read_dir(directory)
            .unwrap_or_else(|error| {
                panic!(
                    "cannot read rustc runtime directory {}: {error}",
                    directory.display()
                )
            })
            .map(|entry| entry.expect("read rustc runtime entry"))
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.file_name()
                .as_bytes()
                .cmp(right.file_name().as_bytes())
        });
        hash.update(b"directory\0");
        for entry in entries {
            hash_field(hash, entry.file_name().as_bytes());
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).unwrap_or_else(|error| {
                panic!(
                    "cannot inspect rustc runtime entry {}: {error}",
                    path.display()
                )
            });
            if metadata.is_file() {
                let bytes = fs::read(&path).unwrap_or_else(|error| {
                    panic!(
                        "cannot read rustc runtime entry {}: {error}",
                        path.display()
                    )
                });
                hash.update(b"file\0");
                hash.update((metadata.mode() & 0o7777).to_le_bytes());
                hash.update((bytes.len() as u64).to_le_bytes());
                hash.update(bytes);
            } else if metadata.is_dir() {
                hash.update(b"subdirectory\0");
                hash.update((metadata.mode() & 0o7777).to_le_bytes());
                hash_directory(hash, &path);
            } else {
                panic!("unsupported rustc runtime entry {}", path.display());
            }
        }
        hash.update(b"end-directory\0");
    }

    let lib = rustc
        .parent()
        .and_then(Path::parent)
        .expect("rustc path is beneath the toolchain root")
        .join("lib");
    assert!(
        lib.is_dir(),
        "rustc runtime directory is absent: {}",
        lib.display()
    );
    let mut hash = Sha256::new();
    hash.update(b"fe2o3-rustc-runtime-tree-v1\0");
    hash_directory(&mut hash, &lib);
    hex(&hash.finalize())
}

fn fixture_manifest() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    fs::canonicalize(manifest).expect("resolve attributed row-softmax fixture")
}

fn fixture_directory() -> PathBuf {
    fixture_manifest()
        .parent()
        .expect("fixture manifest has a parent")
        .to_owned()
}

fn protected_command(action: &str, target_dir: &Path, target: &str) -> Command {
    let cargo = required_path(CARGO_ENV);
    let rustc = required_path(RUSTC_ENV);
    let rustc_runtime_sha256 = RUSTC_RUNTIME_SHA256
        .get_or_init(|| rustc_runtime_sha256(&rustc))
        .clone();
    let backend = required_path(BACKEND_ENV);
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .env_clear()
        .args([
            OsString::from("authority"),
            OsString::from("release"),
            OsString::from(action),
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
            rustc_runtime_sha256,
        )
        .env("FE2O3_BACKEND", &backend)
        .env("FE2O3_AUTHORITY_BACKEND_SHA256_V1", file_sha256(&backend))
        .env("FE2O3_CODEGEN_PIPELINE", "collected-row-softmax-v1")
        .env("FE2O3_TARGET", target)
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("TZ", "UTC");
    command
}

fn provision_provider(target_dir: &Path) -> Value {
    let output = protected_command("build", target_dir, "gfx942:xnack-")
        .output()
        .expect("execute protected row-softmax provider provisioning");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "provider provisioning: {stderr}");
    assert!(
        stderr.contains("no worker, artifact, runtime, or GPU authority was minted"),
        "provider provisioning overclaimed or omitted its boundary: {stderr}"
    );
    assert!(
        !stderr.contains(SUCCESS),
        "provider provisioning reached the GPU terminal marker: {stderr}"
    );
    let encoded = stderr
        .lines()
        .find_map(|line| line.strip_prefix(PROVIDER_OBSERVATION))
        .unwrap_or_else(|| panic!("provider provisioning omitted its observation: {stderr}"));
    serde_json::from_str(encoded).expect("decode canonical provider observation")
}

fn exact_worker_config(provider: Value) -> Value {
    let worker = required_path(WORKER_ENV);
    let worker_bytes = fs::metadata(&worker)
        .expect("inspect upstream-LLVM worker")
        .len();
    let worker_sha256 = file_sha256(&worker);
    let ocml_directory = required_path(OCML_DIRECTORY_ENV);
    assert!(ocml_directory.is_dir(), "OCML provider must be a directory");
    let ocml_file_sha256 = OCML_BASENAMES.map(|basename| {
        let path = ocml_directory.join(basename);
        assert!(
            path.is_file(),
            "missing OCML provider file {}",
            path.display()
        );
        file_sha256(&path)
    });
    let manifest = ocml_manifest_sha256(&ocml_file_sha256);
    json!({
        "candidate_output_max_bytes": fe2o3_hsaco::MAX_HSACO_BYTES,
        "format": "fe2o3-worker-v2-config-v2",
        "limits": {
            "stderr_bytes": fe2o3_hsaco_finalize::DEFAULT_WORKER_STDERR_BYTES,
            "stdout_bytes": fe2o3_hsaco_finalize::MAX_WORKER_RESPONSE_BYTES,
            "timeout_ms": fe2o3_hsaco_finalize::DEFAULT_WORKER_TIMEOUT.as_millis() as u64,
        },
        "link_options": [
            {"name": "code-object-version", "value": "6"},
            {"name": "opt-level", "value": "0"},
            {"name": "strip-debug", "value": "true"},
            {"name": "verify-each", "value": "true"},
        ],
        "providers": [],
        "row_softmax_v1": {
            "case": "normal",
            "comparison_policy": "gfx942-ocml-unmasked-64-v1",
            "mask": "unmasked",
            "ocml_file_sha256": ocml_file_sha256,
            "ocml_manifest_sha256": manifest,
            "provider_crate_hash": provider["provider_crate_hash"].clone(),
            "provider_definition_identities": provider["provider_definition_identities"].clone(),
            "provider_source_identities": provider["provider_source_identities"].clone(),
            "provider_stable_crate_id": provider["provider_stable_crate_id"].clone(),
            "row_elements": 64,
        },
        "units": [{
            "crate_name": "fe2o3_collected_row_softmax_v1_fixture",
            "source": "src/lib.rs",
            "working_directory": fixture_directory(),
        }],
        "worker": {
            "byte_len": worker_bytes,
            "llvm_build_identity": required_text(LLVM_BUILD_ID_ENV),
            "path": worker,
            "sha256": worker_sha256,
            "worker_build_identity": required_text(WORKER_BUILD_ID_ENV),
        },
    })
}

fn ocml_manifest_sha256(file_sha256: &[String; 4]) -> String {
    fn push_u32(output: &mut Vec<u8>, value: usize) {
        output.extend_from_slice(&u32::try_from(value).unwrap().to_le_bytes());
    }
    fn push_text(output: &mut Vec<u8>, value: &str) {
        push_u32(output, value.len());
        output.extend_from_slice(value.as_bytes());
    }
    let mut preimage = Vec::new();
    push_text(&mut preimage, "gfx942-ocml-v1");
    push_text(&mut preimage, "gfx942:xnack-");
    preimage.push(6);
    push_u32(&mut preimage, 1);
    push_text(&mut preimage, "__ocml_exp_f32");
    push_u32(&mut preimage, OCML_BASENAMES.len());
    for (basename, digest) in OCML_BASENAMES.iter().zip(file_sha256) {
        push_text(&mut preimage, basename);
        preimage.extend_from_slice(&decode_sha256(digest));
    }
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/DEVICE-LIBRARY-PROVIDER-MANIFEST/V1\0");
    digest.update((preimage.len() as u64).to_le_bytes());
    digest.update(preimage);
    hex(&digest.finalize())
}

fn decode_sha256(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    std::array::from_fn(|index| {
        u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).expect("decode SHA-256")
    })
}

fn release(config: &Path, target_dir: &Path, target: &str) -> Output {
    let mut command = protected_command("run", target_dir, target);
    command.env("FE2O3_WORKER_V2_CONFIG_V2", config);
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
#[ignore = "blocked until the static production binding wrapper is integrated"]
fn exact_gfx942_authority_release_run_covers_positive_and_hostile_profiles() {
    let directory = TestDirectory::new();

    for case in ["normal", "equal", "dominant"] {
        let config = directory.config(case, |value| {
            value["row_softmax_v1"]["case"] = json!(case);
        });
        assert_success(
            release(&config, &directory.path.join(case), "gfx942:xnack-"),
            case,
        );
    }

    let masked = directory.config("masked", |value| {
        value["row_softmax_v1"]["mask"] = json!("alternating");
    });
    assert_rejected(
        release(
            &masked,
            &directory.path.join("masked-target"),
            "gfx942:xnack-",
        ),
        "stage=workload-mask",
    );

    let exceptional = directory.config("exceptional", |value| {
        value["row_softmax_v1"]["case"] = json!("exceptional");
    });
    assert_rejected(
        release(
            &exceptional,
            &directory.path.join("exceptional-target"),
            "gfx942:xnack-",
        ),
        "stage=cpu-oracle",
    );

    let shape = directory.config("shape", |value| {
        value["row_softmax_v1"]["row_elements"] = json!(63);
    });
    assert_rejected(
        release(
            &shape,
            &directory.path.join("shape-target"),
            "gfx942:xnack-",
        ),
        "stage=workload-shape",
    );

    let policy = directory.config("policy", |value| {
        value["row_softmax_v1"]["comparison_policy"] = json!("wrong-policy");
    });
    assert_rejected(
        release(
            &policy,
            &directory.path.join("policy-target"),
            "gfx942:xnack-",
        ),
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
            &directory.path.join("provider-target"),
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
            &directory.path.join("artifact-target"),
            "gfx942:xnack-",
        ),
        "stage=worker-artifact",
    );

    let target = directory.config("target", |_| {});
    assert_rejected(
        release(&target, &directory.path.join("target-target"), "gfx1100"),
        "requires FE2O3_TARGET=gfx942:xnack-",
    );
}

#[test]
#[ignore = "requires exact production compiler pins on Linux"]
fn exact_gfx942_authority_release_stops_at_static_binding_wrapper_boundary() {
    let target = env::temp_dir().join(format!(
        "cargo-fe2o3-row-softmax-binding-boundary-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    let output = protected_command("build", &target, "gfx942:xnack-")
        .output()
        .expect("execute protected row-softmax binding boundary");
    let _ = fs::remove_dir_all(&target);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "unexpected success: {stderr}");
    assert!(stderr.contains("stage=binding-wrapper"), "{stderr}");
    assert!(
        stderr.contains("integrated static binding wrapper"),
        "{stderr}"
    );
    assert!(!stderr.contains(PROVIDER_OBSERVATION), "{stderr}");
    assert!(!stderr.contains(SUCCESS), "{stderr}");
}
