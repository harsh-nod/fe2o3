#![cfg(target_os = "linux")]

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_artifact_transaction::{
    KernelSetIdentityV1, LinkPublicationPhaseV1, LinkPublicationRecordV1, LinkPublicationScopeV1,
    LinkPublicationStateV1, PackageIdentityV1, TargetIdentityV1,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const CAPTURE_PREFIX: &str =
    "[cargo-fe2o3] inert prepared RustcInvocationDescriptorV2 observation sha256=";
const CAPTURE_SUFFIX: &str = "; no execution or authority claim";
const SOURCE: &str = "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs";
const PRODUCTION_V1_SOURCE: &str =
    "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/lib.rs";
const RETAIN_BASENAME_PREFIX: &str = "cargo-fe2o3-s09-retain-";
const RETAIN_SENTINEL: &str = ".fe2o3-s09-retain-v1";
const RETAIN_SENTINEL_BYTES: &[u8] = b"cargo-fe2o3 production S09 retained directory v1\n";
const DURABLE_ENVELOPE_MAGIC: &[u8] = b"FE2O3-DURABLE-LINK-V1\0";
const DURABLE_ENVELOPE_CHECKSUM_DOMAIN: &[u8] = b"fe2o3.durable-link.envelope-checksum.v1\0";
const DURABLE_SCOPE_IDENTITY_DOMAIN: &[u8] = b"fe2o3.durable-link.scope.v1\0";
const MAX_DURABLE_ENVELOPE_BYTES: u64 = 1_280;
static NEXT_TEST: AtomicU64 = AtomicU64::new(1);

struct TestDirectory {
    path: PathBuf,
    retain: bool,
}

impl TestDirectory {
    fn new() -> Self {
        Self::try_new().unwrap_or_else(|error| panic!("prepare production S09 directory: {error}"))
    }

    fn try_new() -> Result<Self, String> {
        match std::env::var_os("FE2O3_TEST_S09_RETAIN_DIR") {
            Some(requested) => Ok(Self {
                path: prepare_retain_directory(Path::new(&requested))?,
                retain: true,
            }),
            None => Ok(Self {
                path: create_ephemeral_test_directory()?,
                retain: false,
            }),
        }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if !self.retain {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn create_ephemeral_test_directory() -> Result<PathBuf, String> {
    for _ in 0..128 {
        let path = std::env::temp_dir().join(format!(
            "cargo-fe2o3-production-s09-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                fs::create_dir(path.join("home")).map_err(|error| {
                    format!("create ephemeral S09 home in {}: {error}", path.display())
                })?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "create ephemeral S09 directory {}: {error}",
                    path.display()
                ));
            }
        }
    }
    Err("cannot allocate a unique ephemeral S09 directory".to_owned())
}

fn prepare_retain_directory(requested: &Path) -> Result<PathBuf, String> {
    if !requested.is_absolute() {
        return Err("FE2O3_TEST_S09_RETAIN_DIR must be absolute".to_owned());
    }
    let basename = requested
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "retained S09 directory must have a UTF-8 basename".to_owned())?;
    let suffix = basename
        .strip_prefix(RETAIN_BASENAME_PREFIX)
        .ok_or_else(|| {
            format!("retained S09 directory basename must start with {RETAIN_BASENAME_PREFIX}")
        })?;
    if suffix.len() != 32
        || suffix
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
        || suffix.bytes().all(|byte| byte == b'0')
    {
        return Err("retained S09 directory basename must end in 32 nonzero lowercase hexadecimal characters".to_owned());
    }

    let metadata = fs::symlink_metadata(requested).map_err(|error| {
        format!(
            "inspect retained S09 directory {}: {error}",
            requested.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("retained S09 path must be a real pre-created directory".to_owned());
    }
    let canonical = fs::canonicalize(requested).map_err(|error| {
        format!(
            "canonicalize retained S09 directory {}: {error}",
            requested.display()
        )
    })?;
    if canonical != requested {
        return Err(
            "retained S09 directory must already be a canonical path without symlink aliases"
                .to_owned(),
        );
    }
    if fs::read_dir(&canonical)
        .map_err(|error| format!("read retained S09 directory: {error}"))?
        .next()
        .is_some()
    {
        return Err("retained S09 directory must be empty".to_owned());
    }

    let sentinel_path = canonical.join(RETAIN_SENTINEL);
    let mut sentinel = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&sentinel_path)
        .map_err(|error| format!("create retained S09 sentinel: {error}"))?;
    sentinel
        .write_all(RETAIN_SENTINEL_BYTES)
        .and_then(|()| sentinel.sync_all())
        .map_err(|error| format!("persist retained S09 sentinel: {error}"))?;
    fs::create_dir(canonical.join("home"))
        .map_err(|error| format!("create retained S09 home: {error}"))?;
    Ok(canonical)
}

fn required_canonical_file(name: &str) -> PathBuf {
    let value = std::env::var_os(name).unwrap_or_else(|| panic!("required test input {name}"));
    let path = fs::canonicalize(&value)
        .unwrap_or_else(|error| panic!("canonicalize {name}={value:?}: {error}"));
    assert!(path.is_file(), "{name}={} is not a file", path.display());
    path
}

fn required_canonical_directory(name: &str) -> PathBuf {
    let value = std::env::var_os(name).unwrap_or_else(|| panic!("required test input {name}"));
    let path = fs::canonicalize(&value)
        .unwrap_or_else(|error| panic!("canonicalize {name}={value:?}: {error}"));
    assert!(
        path.is_dir(),
        "{name}={} is not a directory",
        path.display()
    );
    path
}

fn sha256(path: &Path) -> String {
    hex(&Sha256::digest(fs::read(path).unwrap_or_else(|error| {
        panic!("read {}: {error}", path.display())
    })))
}

fn required_sha256(name: &str) -> String {
    let value = std::env::var(name).unwrap_or_else(|_| panic!("required test input {name}"));
    assert!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            && !value.bytes().all(|byte| byte == b'0'),
        "{name} is not a nonzero canonical SHA-256 digest"
    );
    value
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_s09_config(
    root: &Path,
    workspace: &Path,
    worker: &Path,
    worker_build_identity: &str,
    llvm_build_identity: &str,
) -> PathBuf {
    let worker_bytes = fs::read(worker).expect("read configured Worker V2 executable");
    let config = json!({
        "candidate_output_max_bytes": 4_194_304,
        "format": "fe2o3-worker-v2-config-v2",
        "limits": {
            "stderr_bytes": 65_536,
            "stdout_bytes": 8_388_608,
            "timeout_ms": 30_000
        },
        "link_options": [
            {"name": "code-object-version", "value": "6"},
            {"name": "opt-level", "value": "0"},
            {"name": "strip-debug", "value": "false"},
            {"name": "verify-each", "value": "true"}
        ],
        "providers": [],
        "source_debug_profile": "s09-alpha-gfx942-o0-v1",
        "units": [{
            "crate_name": "fe2o3_typed_alias_spoof",
            "source": SOURCE,
            "working_directory": workspace
        }],
        "worker": {
            "byte_len": worker_bytes.len(),
            "llvm_build_identity": llvm_build_identity,
            "path": worker,
            "sha256": hex(&Sha256::digest(&worker_bytes)),
            "worker_build_identity": worker_build_identity
        }
    });
    let path = root.join("worker-v2-s09.json");
    fs::write(&path, serde_json::to_vec(&config).unwrap()).expect("write S09 Worker config");
    path
}

fn write_production_v1_config(
    root: &Path,
    workspace: &Path,
    worker: &Path,
    worker_build_identity: &str,
    llvm_build_identity: &str,
) -> PathBuf {
    let worker_bytes = fs::read(worker).expect("read configured Worker V2 executable");
    let config = json!({
        "candidate_output_max_bytes": 67_108_864,
        "format": "fe2o3-worker-v2-config-v2",
        "limits": {
            "stderr_bytes": 16_384,
            "stdout_bytes": 16_384,
            "timeout_ms": 60_000
        },
        "link_options": [
            {"name": "code-object-version", "value": "6"},
            {"name": "opt-level", "value": "2"},
            {"name": "strip-debug", "value": "true"},
            {"name": "verify-each", "value": "true"}
        ],
        "providers": [],
        "units": [{
            "crate_name": "fe2o3_production_extraction_fixture",
            "source": PRODUCTION_V1_SOURCE,
            "working_directory": workspace
        }],
        "worker": {
            "byte_len": worker_bytes.len(),
            "llvm_build_identity": llvm_build_identity,
            "path": worker,
            "sha256": hex(&Sha256::digest(&worker_bytes)),
            "worker_build_identity": worker_build_identity
        }
    });
    let path = root.join("worker-v2-production-v1.json");
    fs::write(&path, serde_json::to_vec(&config).unwrap())
        .expect("write production-v1 Worker config");
    path
}

fn assert_success(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "production S09 compile failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    stderr.into_owned()
}

fn capture_digest(stderr: &str) -> String {
    let digests = stderr
        .lines()
        .filter_map(|line| {
            line.strip_prefix(CAPTURE_PREFIX)
                .and_then(|value| value.strip_suffix(CAPTURE_SUFFIX))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        digests.len(),
        1,
        "expected one production capture observation:\n{stderr}"
    );
    let digest = digests[0];
    assert!(
        digest.len() == 64
            && digest.bytes().any(|byte| byte != b'0')
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "noncanonical capture digest {digest:?}"
    );
    digest.to_owned()
}

struct PublishedS09Observation {
    hsaco: PathBuf,
    hsaco_sha256: String,
    record_sha256: String,
    target: String,
    kernel_set_identity: String,
    target_identity: String,
    request_identity: String,
    publication_worker_identity: String,
    publication_identity: String,
}

fn published_hsaco(artifact_dir: &Path, expected_kernel: &str) -> PublishedS09Observation {
    let mut hsaco = Vec::new();
    let mut records = Vec::new();
    for entry in fs::read_dir(artifact_dir).expect("read production S09 artifact directory") {
        let entry = entry.expect("read production S09 artifact entry");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(".fe2o3-link-artifact-v1-") && name.ends_with(".bin") {
            hsaco.push(entry.path());
        }
        if name.starts_with(".fe2o3-link-publication-v1-") && name.ends_with(".record") {
            records.push(entry.path());
        }
    }
    assert_eq!(hsaco.len(), 1, "expected one durable S09 HSACO");
    assert_eq!(
        records.len(),
        1,
        "expected one durable S09 publication record"
    );

    let record_metadata = fs::metadata(&records[0]).expect("inspect production S09 record");
    assert!(
        record_metadata.len() <= MAX_DURABLE_ENVELOPE_BYTES,
        "production S09 record exceeds its canonical bound"
    );
    let record_bytes = fs::read(&records[0]).expect("read production S09 publication record");
    let record = decode_durable_publication_record(&record_bytes, &records[0]);
    assert_eq!(
        record.state(),
        LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published)
    );
    assert!(record.worker().is_some());
    assert!(record.response().is_some());
    assert!(record.linked_output().is_some());
    assert!(record.finalization().is_some());
    assert!(record.publication().is_some());

    let hsaco = hsaco.pop().unwrap();
    let bytes = fs::read(&hsaco).expect("read production S09 HSACO");
    let hsaco_digest: [u8; 32] = Sha256::digest(&bytes).into();
    let finalized = record
        .finalized_output()
        .expect("published record has finalized output");
    assert_eq!(finalized.as_bytes(), &hsaco_digest);
    let expected_artifact_name = format!(".fe2o3-link-artifact-v1-{}.bin", hex(&hsaco_digest));
    assert_eq!(
        hsaco.file_name().and_then(|name| name.to_str()),
        Some(expected_artifact_name.as_str())
    );

    let inspection = fe2o3_hsaco::inspect(&bytes).expect("inspect production Worker V2 HSACO");
    let target = inspection.target().to_string();
    assert_eq!(target, "gfx942:xnack-");
    assert_eq!(
        inspection.code_object_version(),
        fe2o3_hsaco::CodeObjectVersion::V6
    );
    assert_eq!(
        inspection
            .kernels()
            .iter()
            .map(|kernel| kernel.name())
            .collect::<Vec<_>>(),
        [expected_kernel]
    );
    let finalized = fe2o3_hsaco_finalize::verify_finalized(&bytes)
        .expect("verify the canonical embedded descriptor digest");
    assert_eq!(finalized.descriptor_table().kernels().len(), 1);
    assert_eq!(
        finalized.descriptor_table().kernels()[0]
            .entry_name()
            .as_str(),
        expected_kernel
    );
    PublishedS09Observation {
        hsaco,
        hsaco_sha256: hex(&hsaco_digest),
        record_sha256: hex(&Sha256::digest(&record_bytes)),
        target,
        kernel_set_identity: hex(record.scope().kernel_set().as_bytes()),
        target_identity: hex(record.scope().target().as_bytes()),
        request_identity: hex(record.request().as_bytes()),
        publication_worker_identity: hex(record
            .worker()
            .expect("published record has worker identity")
            .as_bytes()),
        publication_identity: hex(record
            .publication()
            .expect("published record has publication identity")
            .as_bytes()),
    }
}

fn decode_durable_publication_record(bytes: &[u8], record_path: &Path) -> LinkPublicationRecordV1 {
    let checksum_offset = bytes
        .len()
        .checked_sub(32)
        .expect("durable publication record has checksum");
    let (body, checksum) = bytes.split_at(checksum_offset);
    let mut digest = Sha256::new();
    digest.update(DURABLE_ENVELOPE_CHECKSUM_DOMAIN);
    digest.update(body);
    assert_eq!(digest.finalize().as_slice(), checksum);

    let mut cursor = ByteCursor::new(body);
    assert_eq!(
        cursor.take(DURABLE_ENVELOPE_MAGIC.len()),
        DURABLE_ENVELOPE_MAGIC
    );
    assert_eq!(cursor.u16(), 1);
    assert_eq!(cursor.byte(), 0, "durable publication is poisoned");
    let generation_floor = cursor.u64();
    let scope = LinkPublicationScopeV1::new(
        PackageIdentityV1::from_bytes(cursor.identity(0x10)),
        KernelSetIdentityV1::from_bytes(cursor.identity(0x11)),
        TargetIdentityV1::from_bytes(cursor.identity(0x12)),
    );
    let published_length = usize::from(cursor.u16());
    assert!(published_length > 0);
    let published_bytes = cursor.take(published_length);
    let record = LinkPublicationRecordV1::decode_canonical(published_bytes)
        .expect("decode canonical nested publication record");
    assert_eq!(record.encode_canonical().unwrap(), published_bytes);
    assert_eq!(record.scope(), scope);
    assert_eq!(record.attempt().generation(), generation_floor);
    assert_eq!(
        cursor.byte(),
        0,
        "completed publication retained an active plan"
    );
    assert_eq!(
        cursor.u16(),
        0,
        "completed publication retained an active record"
    );
    assert!(cursor.finished());

    let mut scope_digest = Sha256::new();
    scope_digest.update(DURABLE_SCOPE_IDENTITY_DOMAIN);
    scope_digest.update(scope.package().as_bytes());
    scope_digest.update(scope.kernel_set().as_bytes());
    scope_digest.update(scope.target().as_bytes());
    let expected_name = format!(
        ".fe2o3-link-publication-v1-{}.record",
        hex(&scope_digest.finalize())
    );
    assert_eq!(
        record_path.file_name().and_then(|name| name.to_str()),
        Some(expected_name.as_str())
    );
    record
}

struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> &'a [u8] {
        let end = self
            .offset
            .checked_add(count)
            .expect("record offset overflow");
        let value = self
            .bytes
            .get(self.offset..end)
            .expect("truncated durable publication record");
        self.offset = end;
        value
    }

    fn byte(&mut self) -> u8 {
        self.take(1)[0]
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take(2).try_into().unwrap())
    }

    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }

    fn identity(&mut self, expected_tag: u8) -> [u8; 32] {
        assert_eq!(self.byte(), expected_tag);
        self.take(32).try_into().unwrap()
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[test]
#[ignore = "requires explicit upstream Cargo/rustc/backend, Cargo cache, and native LLVM Worker pins; see crates/cargo-fe2o3/README.md"]
fn production_s09_compile_captures_and_publishes_worker_output() {
    let workspace = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("canonical workspace");
    let rustc = required_canonical_file("FE2O3_TEST_UPSTREAM_RUSTC");
    let cargo = required_canonical_file("FE2O3_TEST_UPSTREAM_CARGO");
    let backend = required_canonical_file("FE2O3_TEST_CODEGEN_BACKEND");
    let cargo_home = required_canonical_directory("FE2O3_TEST_CARGO_HOME");
    let worker = required_canonical_file("FE2O3_LLVM_LINK_WORKER");
    let worker_build_identity = std::env::var("FE2O3_LLVM_LINK_WORKER_BUILD_ID")
        .expect("required test input FE2O3_LLVM_LINK_WORKER_BUILD_ID");
    let llvm_build_identity =
        std::env::var("FE2O3_LLVM_BUILD_ID").expect("required test input FE2O3_LLVM_BUILD_ID");
    let directory = TestDirectory::new();
    let config = write_s09_config(
        &directory.path,
        &workspace,
        &worker,
        &worker_build_identity,
        &llvm_build_identity,
    );
    let target = directory.path.join("cargo-target");
    let rustc_bin = directory.path.join("pinned-rustc-bin");
    fs::create_dir(&rustc_bin).expect("create pinned rustc bin directory");
    symlink(&rustc, rustc_bin.join("rustc")).expect("install pinned rustc path entry");
    let rustc_sha256 = sha256(&rustc);
    let rustc_runtime_sha256 = required_sha256("FE2O3_TEST_RUSTC_RUNTIME_SHA256_V1");
    let cargo_sha256 = sha256(&cargo);
    let backend_sha256 = sha256(&backend);

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .current_dir(&workspace)
        .args([
            "build",
            "-p",
            "fe2o3-typed-alias-spoof",
            "--features",
            "s09-alpha-only",
            "--locked",
            "--offline",
            "--target-dir",
        ])
        .arg(&target)
        .env("CARGO", &cargo)
        .env("CARGO_HOME", &cargo_home)
        .env("HOME", directory.path.join("home"))
        .env("LANG", "C.UTF-8")
        .env("PATH", format!("{}:/usr/bin:/bin", rustc_bin.display()))
        .env("FE2O3_BACKEND", &backend)
        .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", &cargo_sha256)
        .env("FE2O3_AUTHORITY_RUSTC_SHA256_V1", &rustc_sha256)
        .env("FE2O3_AUTHORITY_RUSTC_PATH_V1", &rustc)
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            &rustc_runtime_sha256,
        )
        .env("FE2O3_AUTHORITY_BACKEND_SHA256_V1", &backend_sha256)
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env(
            "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1",
            "1",
        )
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_WORKER_V2_CONFIG_V2", &config)
        .env(
            "RUSTFLAGS",
            format!("--remap-path-prefix={}/=", workspace.display()),
        )
        .output()
        .expect("run production cargo-fe2o3 S09 compile");
    let stderr = assert_success(&output);
    let capture = capture_digest(&stderr);
    assert!(
        stderr.contains("selected kernel-ir-worker-v2: verified compiler-module candidate"),
        "production backend did not select verified Worker output:\n{stderr}"
    );
    assert!(
        stderr.contains("published inert Worker V2 compiler-module handoff"),
        "production backend did not publish its inert Worker handoff:\n{stderr}"
    );
    assert!(
        stderr.contains(&format!("pinned_cargo_image_sha256={cargo_sha256}")),
        "S09 build claim did not bind the brokered pinned Cargo image:\n{stderr}"
    );

    let published = published_hsaco(&target.join("fe2o3"), "alpha");
    println!(
        "FE2O3_S09_PRODUCTION_OBSERVATION_V1 capture_sha256={capture} rustc_sha256={} backend_sha256={} cargo_sha256={cargo_sha256} worker_sha256={} hsaco_sha256={} publication_record_sha256={} publication_kernel_set_identity={} publication_target_identity={} publication_request_identity={} publication_worker_identity={} publication_identity={} target={}; observation only, no execution or authority claim",
        rustc_sha256,
        backend_sha256,
        sha256(&worker),
        published.hsaco_sha256,
        published.record_sha256,
        published.kernel_set_identity,
        published.target_identity,
        published.request_identity,
        published.publication_worker_identity,
        published.publication_identity,
        published.target,
    );
    assert_eq!(sha256(&published.hsaco), published.hsaco_sha256);
}

#[test]
#[ignore = "requires explicit upstream Cargo/rustc/backend, Cargo cache, and native LLVM Worker pins; see crates/cargo-fe2o3/README.md"]
fn production_v1_fill_compiles_and_publishes_finalized_worker_output() {
    let workspace = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("canonical workspace");
    let rustc = required_canonical_file("FE2O3_TEST_UPSTREAM_RUSTC");
    let cargo = required_canonical_file("FE2O3_TEST_UPSTREAM_CARGO");
    let backend = required_canonical_file("FE2O3_TEST_CODEGEN_BACKEND");
    let cargo_home = required_canonical_directory("FE2O3_TEST_CARGO_HOME");
    let worker = required_canonical_file("FE2O3_LLVM_LINK_WORKER");
    let worker_build_identity = std::env::var("FE2O3_LLVM_LINK_WORKER_BUILD_ID")
        .expect("required test input FE2O3_LLVM_LINK_WORKER_BUILD_ID");
    let llvm_build_identity =
        std::env::var("FE2O3_LLVM_BUILD_ID").expect("required test input FE2O3_LLVM_BUILD_ID");
    let directory = TestDirectory::new();
    let config = write_production_v1_config(
        &directory.path,
        &workspace,
        &worker,
        &worker_build_identity,
        &llvm_build_identity,
    );
    let target = directory.path.join("cargo-target");
    let rustc_bin = directory.path.join("pinned-rustc-bin");
    fs::create_dir(&rustc_bin).expect("create pinned rustc bin directory");
    symlink(&rustc, rustc_bin.join("rustc")).expect("install pinned rustc path entry");

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .current_dir(&workspace)
        .args([
            "build",
            "-p",
            "fe2o3-production-extraction-fixture",
            "--locked",
            "--offline",
            "-Zbuild-std=core",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(&target)
        .env("CARGO", &cargo)
        .env("CARGO_HOME", &cargo_home)
        .env("HOME", directory.path.join("home"))
        .env("LANG", "C.UTF-8")
        .env("PATH", format!("{}:/usr/bin:/bin", rustc_bin.display()))
        .env("FE2O3_BACKEND", &backend)
        .env("FE2O3_CODEGEN_PIPELINE", "production-v1")
        .env(
            "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1",
            "1",
        )
        .env("FE2O3_TARGET", "gfx942")
        .env("FE2O3_WORKER_V2_CONFIG_V2", &config)
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            format!(
                "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 --remap-path-prefix={}/=",
                workspace.display()
            ),
        )
        .output()
        .expect("run production-v1 cargo-fe2o3 compile");
    let stderr = assert_success(&output);
    assert!(
        stderr.contains("production-v1 published")
            && stderr.contains("managed Worker V2 transaction"),
        "production backend did not publish its managed handoff:\n{stderr}"
    );

    let published = published_hsaco(&target.join("fe2o3"), "fill");
    let bytes = fs::read(&published.hsaco).expect("read production-v1 fill HSACO");
    let finalized = fe2o3_hsaco_finalize::verify_finalized(&bytes)
        .expect("verify production-v1 fill descriptor");
    let kernel = &finalized.descriptor_table().kernels()[0];
    assert_eq!(kernel.descriptor_symbol().as_str(), "fill.kd");
    assert_eq!(kernel.abi_layout().explicit_argument_size(), 16);
    assert_eq!(kernel.abi_layout().kernarg_segment_size(), 272);
    assert_eq!(kernel.abi_layout().kernarg_segment_alignment(), 8);
    assert_eq!(kernel.arguments().len(), 1);
    assert_eq!(kernel.launch().max_flat_workgroup_size(), 64);

    println!(
        "FE2O3_PRODUCTION_V1_FILL_OBSERVATION_V1 backend_sha256={} cargo_sha256={} worker_sha256={} hsaco_sha256={} publication_record_sha256={} target={}; non-authoritative integration observation",
        sha256(&backend),
        sha256(&cargo),
        sha256(&worker),
        published.hsaco_sha256,
        published.record_sha256,
        published.target,
    );
}

#[test]
fn retained_directory_policy_refuses_home_repo_nonempty_and_symlink_paths() {
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        if home.exists() {
            assert!(prepare_retain_directory(&home).is_err());
            assert!(!home.join(RETAIN_SENTINEL).exists());
        }
    }
    let repository = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("canonical repository");
    assert!(prepare_retain_directory(&repository).is_err());
    assert!(!repository.join(RETAIN_SENTINEL).exists());

    let scratch = create_policy_test_root("refusal");
    let nonempty = scratch.join(valid_retain_basename(1));
    fs::create_dir(&nonempty).unwrap();
    fs::write(nonempty.join("caller-data"), b"keep").unwrap();
    assert!(prepare_retain_directory(&nonempty).is_err());
    assert_eq!(fs::read(nonempty.join("caller-data")).unwrap(), b"keep");
    assert!(!nonempty.join(RETAIN_SENTINEL).exists());

    let target = scratch.join("real-empty-target");
    fs::create_dir(&target).unwrap();
    let alias = scratch.join(valid_retain_basename(2));
    symlink(&target, &alias).unwrap();
    assert!(prepare_retain_directory(&alias).is_err());
    assert!(!target.join(RETAIN_SENTINEL).exists());

    let real_parent = scratch.join("real-parent");
    fs::create_dir(&real_parent).unwrap();
    let nested = real_parent.join(valid_retain_basename(4));
    fs::create_dir(&nested).unwrap();
    let parent_alias = scratch.join("parent-alias");
    symlink(&real_parent, &parent_alias).unwrap();
    assert!(prepare_retain_directory(&parent_alias.join(valid_retain_basename(4))).is_err());
    assert!(!nested.join(RETAIN_SENTINEL).exists());
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn retained_directory_policy_marks_but_never_removes_approved_leaf() {
    let scratch = create_policy_test_root("approved");
    let retained = scratch.join(valid_retain_basename(3));
    fs::create_dir(&retained).unwrap();

    let prepared = prepare_retain_directory(&retained).unwrap();
    assert_eq!(prepared, retained);
    assert_eq!(
        fs::read(retained.join(RETAIN_SENTINEL)).unwrap(),
        RETAIN_SENTINEL_BYTES
    );
    assert!(retained.join("home").is_dir());
    assert!(prepare_retain_directory(&retained).is_err());
    assert!(retained.exists());
    fs::remove_dir_all(scratch).unwrap();
}

fn create_policy_test_root(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "cargo-fe2o3-s09-retain-policy-{label}-{}-{}",
        std::process::id(),
        NEXT_TEST.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}

fn valid_retain_basename(seed: u128) -> String {
    format!("{RETAIN_BASENAME_PREFIX}{seed:032x}")
}
