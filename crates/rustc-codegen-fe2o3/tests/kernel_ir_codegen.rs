use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, OnceLock};

use fe2o3_artifacts::DigestAlgorithm;

const PIPELINE_ENV: &str = "FE2O3_CODEGEN_PIPELINE";
const LLVM_AS_ENV: &str = "FE2O3_LLVM_AS";
const LLVM_DWARFDUMP_ENV: &str = "FE2O3_LLVM_DWARFDUMP";
const PROVIDER_SYMBOL: &str = "external_device_add_v1";
const PROVIDER_KERNEL: &str = "worker_v2_provider_kernel";
const MULTI_KERNEL_ALPHA: &str = "worker_v2_alpha";
const MULTI_KERNEL_ZETA: &str = "worker_v2_zeta";
const ALPHA_ZETA_OUTPUT_ENV: &str = "FE2O3_GFX942_ALPHA_ZETA_OUTPUT";
const S09_DEBUG_HSACO_OUTPUT_ENV: &str = "FE2O3_S09_DEBUG_HSACO_OUTPUT";

fn backend_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("backend test lock")
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn sha256_hex(bytes: &[u8]) -> String {
    DigestAlgorithm::Sha256
        .calculate(bytes)
        .bytes()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn backend(workspace: &Path, command: &str, package: &str, pipeline: Option<&str>) -> Output {
    backend_with_worker_config(workspace, command, package, pipeline, None)
}

fn backend_with_worker_config(
    workspace: &Path,
    command: &str,
    package: &str,
    pipeline: Option<&str>,
    worker_config: Option<&Path>,
) -> Output {
    let mut process = Command::new(env!("CARGO"));
    process
        .current_dir(workspace)
        .env_remove(PIPELINE_ENV)
        .args([
            "run",
            "--locked",
            "-p",
            "cargo-fe2o3",
            "--",
            command,
            "-p",
            package,
        ]);
    if let Some(pipeline) = pipeline {
        process.env(PIPELINE_ENV, pipeline);
    }
    if let Some(worker_config) = worker_config {
        process.env("FE2O3_WORKER_V2_CONFIG_V2", worker_config);
    }
    process.output().expect("run cargo-fe2o3")
}

struct WorkerV2TestConfig(PathBuf);

impl WorkerV2TestConfig {
    fn missing_envelope(workspace: &Path) -> Self {
        let worker = std::env::current_exe().expect("current test executable");
        let bytes = std::fs::read(&worker).expect("read current test executable");
        let digest = DigestAlgorithm::Sha256.calculate(&bytes).bytes();
        let hex = digest
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = workspace.join(format!(
            "target/worker-v2-missing-envelope-{}.json",
            std::process::id()
        ));
        let worker = worker.to_str().expect("UTF-8 worker path");
        let workspace = workspace.to_str().expect("UTF-8 workspace path");
        let json = format!(
            "{{\"candidate_output_max_bytes\":4194304,\"format\":\"fe2o3-worker-v2-config-v2\",\"limits\":{{\"stderr_bytes\":65536,\"stdout_bytes\":8388608,\"timeout_ms\":30000}},\"link_options\":[{{\"name\":\"code-object-version\",\"value\":\"5\"}},{{\"name\":\"opt-level\",\"value\":\"2\"}},{{\"name\":\"strip-debug\",\"value\":\"true\"}},{{\"name\":\"verify-each\",\"value\":\"true\"}}],\"providers\":[],\"units\":[{{\"crate_name\":\"fe2o3_fill\",\"source\":\"examples/fill/src/main.rs\",\"working_directory\":{workspace:?}}}],\"worker\":{{\"byte_len\":{},\"llvm_build_identity\":\"test-only-unreached-llvm\",\"path\":{worker:?},\"sha256\":\"{hex}\",\"worker_build_identity\":\"test-only-unreached-worker\"}}}}",
            bytes.len()
        );
        std::fs::write(&path, json).expect("write Worker V2 test config");
        Self(path)
    }

    fn native_source(
        directory: &Path,
        workspace: &Path,
        source: &Path,
        worker: &Path,
        worker_build_identity: &str,
        llvm_build_identity: &str,
    ) -> Self {
        Self::native_source_for_crate(
            directory,
            workspace,
            source,
            ("worker_v2_source", 5),
            worker,
            worker_build_identity,
            llvm_build_identity,
        )
    }

    fn native_source_for_crate(
        directory: &Path,
        workspace: &Path,
        source: &Path,
        unit: (&str, u16),
        worker: &Path,
        worker_build_identity: &str,
        llvm_build_identity: &str,
    ) -> Self {
        let (crate_name, code_object_version) = unit;
        let bytes = std::fs::read(worker).expect("read configured Worker V2 executable");
        let digest = DigestAlgorithm::Sha256.calculate(&bytes).bytes();
        let hex = digest
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = directory.join("worker-v2-native-source.json");
        let worker = worker.to_str().expect("UTF-8 worker path");
        let workspace = workspace.to_str().expect("UTF-8 workspace path");
        let source = source.to_str().expect("UTF-8 source path");
        let json = format!(
            "{{\"candidate_output_max_bytes\":4194304,\"format\":\"fe2o3-worker-v2-config-v2\",\"limits\":{{\"stderr_bytes\":65536,\"stdout_bytes\":8388608,\"timeout_ms\":30000}},\"link_options\":[{{\"name\":\"code-object-version\",\"value\":\"{code_object_version}\"}},{{\"name\":\"opt-level\",\"value\":\"2\"}},{{\"name\":\"strip-debug\",\"value\":\"true\"}},{{\"name\":\"verify-each\",\"value\":\"true\"}}],\"providers\":[],\"units\":[{{\"crate_name\":{crate_name:?},\"source\":{source:?},\"working_directory\":{workspace:?}}}],\"worker\":{{\"byte_len\":{},\"llvm_build_identity\":{llvm_build_identity:?},\"path\":{worker:?},\"sha256\":\"{hex}\",\"worker_build_identity\":{worker_build_identity:?}}}}}",
            bytes.len(),
        );
        std::fs::write(&path, json).expect("write native Worker V2 source config");
        Self(path)
    }

    fn native_s09_alpha_debug(
        directory: &Path,
        workspace: &Path,
        source: &Path,
        worker: &Path,
        worker_build_identity: &str,
        llvm_build_identity: &str,
    ) -> Self {
        let bytes = std::fs::read(worker).expect("read configured Worker V2 executable");
        let hex = sha256_hex(&bytes);
        let path = directory.join("worker-v2-s09-alpha-debug.json");
        let worker = worker.to_str().expect("UTF-8 worker path");
        let workspace = workspace.to_str().expect("UTF-8 workspace path");
        let source = source.to_str().expect("UTF-8 source path");
        let json = format!(
            "{{\"candidate_output_max_bytes\":4194304,\"format\":\"fe2o3-worker-v2-config-v2\",\"limits\":{{\"stderr_bytes\":65536,\"stdout_bytes\":8388608,\"timeout_ms\":30000}},\"link_options\":[{{\"name\":\"code-object-version\",\"value\":\"6\"}},{{\"name\":\"opt-level\",\"value\":\"0\"}},{{\"name\":\"strip-debug\",\"value\":\"false\"}},{{\"name\":\"verify-each\",\"value\":\"true\"}}],\"providers\":[],\"source_debug_profile\":\"s09-alpha-gfx942-o0-v1\",\"units\":[{{\"crate_name\":\"fe2o3_typed_alias_spoof\",\"source\":{source:?},\"working_directory\":{workspace:?}}}],\"worker\":{{\"byte_len\":{},\"llvm_build_identity\":{llvm_build_identity:?},\"path\":{worker:?},\"sha256\":\"{hex}\",\"worker_build_identity\":{worker_build_identity:?}}}}}",
            bytes.len(),
        );
        std::fs::write(&path, json).expect("write S09 Worker V2 debug config");
        Self(path)
    }

    fn native_source_with_bitcode_provider(
        directory: &Path,
        workspace: &Path,
        source: &Path,
        provider: &Path,
        worker: &Path,
        worker_build_identity: &str,
        llvm_build_identity: &str,
    ) -> Self {
        let worker_bytes = std::fs::read(worker).expect("read configured Worker V2 executable");
        let worker_digest = sha256_hex(&worker_bytes);
        let provider_bytes = std::fs::read(provider).expect("read LLVM bitcode provider");
        assert!(
            provider_bytes.starts_with(b"BC\xc0\xde"),
            "provider is not LLVM bitcode"
        );
        let provider_digest = sha256_hex(&provider_bytes);
        let path = directory.join("worker-v2-bitcode-provider.json");
        let worker = worker.to_str().expect("UTF-8 worker path");
        let workspace = workspace.to_str().expect("UTF-8 workspace path");
        let source = source.to_str().expect("UTF-8 source path");
        let provider = provider.to_str().expect("UTF-8 provider path");
        let json = format!(
            "{{\"candidate_output_max_bytes\":4194304,\"format\":\"fe2o3-worker-v2-config-v2\",\"limits\":{{\"stderr_bytes\":65536,\"stdout_bytes\":8388608,\"timeout_ms\":30000}},\"link_options\":[{{\"name\":\"code-object-version\",\"value\":\"5\"}},{{\"name\":\"opt-level\",\"value\":\"2\"}},{{\"name\":\"strip-debug\",\"value\":\"true\"}},{{\"name\":\"verify-each\",\"value\":\"true\"}}],\"providers\":[{{\"byte_len\":{},\"kind\":\"llvm-bitcode\",\"path\":{provider:?},\"sha256\":\"{provider_digest}\"}}],\"units\":[{{\"crate_name\":\"worker_v2_provider_source\",\"source\":{source:?},\"working_directory\":{workspace:?}}}],\"worker\":{{\"byte_len\":{},\"llvm_build_identity\":{llvm_build_identity:?},\"path\":{worker:?},\"sha256\":\"{worker_digest}\",\"worker_build_identity\":{worker_build_identity:?}}}}}",
            provider_bytes.len(),
            worker_bytes.len(),
        );
        std::fs::write(&path, json).expect("write bitcode-provider Worker V2 config");
        Self(path)
    }
}

impl Drop for WorkerV2TestConfig {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct WorkerV2SourceDirectory(PathBuf);

impl WorkerV2SourceDirectory {
    fn new(workspace: &Path) -> Self {
        let path = workspace.join(format!(
            "target/worker-v2-native-source-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create native Worker V2 source directory");
        Self(path)
    }
}

impl Drop for WorkerV2SourceDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn build_codegen_backend(workspace: &Path) -> PathBuf {
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .output()
        .expect("build rustc-codegen-fe2o3");
    assert!(
        output.status.success(),
        "backend build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    workspace.join("target/debug/librustc_codegen_fe2o3.so")
}

fn worker_v2_source() -> String {
    let fields = reserved_fe2o3_symbols::DeviceFfiContractFieldsV1 {
        direction: reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "local_device_identity_v1",
        calling_convention: "C",
        code_object_version: 5,
        target: "gfx942:xnack-",
        physical_abi: "C(u32[size=4,align=4])->u32[size=4,align=4]",
        effects: "none",
        semantic_identity: "5656565656565656565656565656565656565656565656565656565656565656",
    };
    let contract = reserved_fe2o3_symbols::derive_device_ffi_contract_id_v1(fields);
    let marker = reserved_fe2o3_symbols::device_ffi_marker_v1(contract, fields);
    format!(
        r#"
#[doc = "{marker}"]
#[unsafe(export_name = "local_device_identity_v1")]
pub unsafe extern "C" fn local_device_identity(value: u32) -> u32 {{
    value
}}

#[used]
static __fe2o3_device_ffi_registration_v1_{contract}: (
    u64, u16, u16, &'static str, &'static str, &'static str, u16,
    &'static str, &'static str, &'static str, &'static str,
    unsafe extern "C" fn(u32) -> u32,
) = (
    0x4946_4633_4f32_4546, 1, 2, "{contract}", "local_device_identity_v1",
    "C", 5, "gfx942:xnack-", "C(u32[size=4,align=4])->u32[size=4,align=4]",
    "none", "5656565656565656565656565656565656565656565656565656565656565656",
    local_device_identity,
);

#[unsafe(export_name = "fe2o3_kernel_worker_v2_kernel")]
pub fn worker_v2_kernel() {{}}

#[used]
static __fe2o3_kernel_registration_worker_v2_kernel: (
    u64, u16, u16, &'static str, &'static str, fn(),
) = (
    0x4e52_4b33_4f32_4546, 1, 1, "worker_v2_kernel",
    "worker_v2_kernel", worker_v2_kernel,
);
"#,
        contract = contract.to_hex(),
    )
}

fn worker_v2_multi_kernel_source() -> String {
    let fields = reserved_fe2o3_symbols::DeviceFfiContractFieldsV1 {
        direction: reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "local_device_identity_v1",
        calling_convention: "C",
        code_object_version: 5,
        target: "gfx942:xnack-",
        physical_abi: "C(u32[size=4,align=4])->u32[size=4,align=4]",
        effects: "none",
        semantic_identity: "5656565656565656565656565656565656565656565656565656565656565656",
    };
    let contract = reserved_fe2o3_symbols::derive_device_ffi_contract_id_v1(fields);
    let marker = reserved_fe2o3_symbols::device_ffi_marker_v1(contract, fields);
    format!(
        r#"
#[doc = "{marker}"]
#[unsafe(export_name = "local_device_identity_v1")]
pub unsafe extern "C" fn local_device_identity(value: u32) -> u32 {{
    value
}}

#[used]
static __fe2o3_device_ffi_registration_v1_{contract}: (
    u64, u16, u16, &'static str, &'static str, &'static str, u16,
    &'static str, &'static str, &'static str, &'static str,
    unsafe extern "C" fn(u32) -> u32,
) = (
    0x4946_4633_4f32_4546, 1, 2, "{contract}", "local_device_identity_v1",
    "C", 5, "gfx942:xnack-", "C(u32[size=4,align=4])->u32[size=4,align=4]",
    "none", "5656565656565656565656565656565656565656565656565656565656565656",
    local_device_identity,
);

#[inline(never)]
#[unsafe(export_name = "worker_v2_shared_helper")]
pub fn shared_helper(value: u32) -> u32 {{
    value
}}

#[unsafe(export_name = "fe2o3_kernel_{MULTI_KERNEL_ZETA}")]
pub fn zeta() {{
    let _ = shared_helper(2);
}}

#[used]
static __fe2o3_kernel_registration_worker_v2_zeta: (
    u64, u16, u16, &'static str, &'static str, fn(),
) = (
    0x4e52_4b33_4f32_4546, 1, 1, "{MULTI_KERNEL_ZETA}",
    "{MULTI_KERNEL_ZETA}", zeta,
);

#[unsafe(export_name = "fe2o3_kernel_{MULTI_KERNEL_ALPHA}")]
pub fn alpha() {{
    let _ = shared_helper(1);
}}

#[used]
static __fe2o3_kernel_registration_worker_v2_alpha: (
    u64, u16, u16, &'static str, &'static str, fn(),
) = (
    0x4e52_4b33_4f32_4546, 1, 1, "{MULTI_KERNEL_ALPHA}",
    "{MULTI_KERNEL_ALPHA}", alpha,
);
"#,
        contract = contract.to_hex(),
    )
}

fn worker_v2_provider_source() -> String {
    let fields = reserved_fe2o3_symbols::DeviceFfiContractFieldsV1 {
        direction: reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_IMPORT_V1,
        symbol: PROVIDER_SYMBOL,
        calling_convention: "C",
        code_object_version: 5,
        target: "gfx942:xnack-",
        physical_abi: "C(u32[size=4,align=4])->u32[size=4,align=4]",
        effects: "none",
        semantic_identity: "6767676767676767676767676767676767676767676767676767676767676767",
    };
    let contract = reserved_fe2o3_symbols::derive_device_ffi_contract_id_v1(fields);
    let marker = reserved_fe2o3_symbols::device_ffi_marker_v1(contract, fields);
    format!(
        r#"
unsafe extern "C" {{
    #[doc = "{marker}"]
    #[link_name = "{PROVIDER_SYMBOL}"]
    fn external_device_add(value: u32) -> u32;
}}

#[used]
static __fe2o3_device_ffi_registration_v1_{contract}: (
    u64, u16, u16, &'static str, &'static str, &'static str, u16,
    &'static str, &'static str, &'static str, &'static str,
    unsafe extern "C" fn(u32) -> u32,
) = (
    0x4946_4633_4f32_4546, 1, 1, "{contract}", "{PROVIDER_SYMBOL}",
    "C", 5, "gfx942:xnack-", "C(u32[size=4,align=4])->u32[size=4,align=4]",
    "none", "6767676767676767676767676767676767676767676767676767676767676767",
    external_device_add,
);

#[unsafe(export_name = "fe2o3_kernel_{PROVIDER_KERNEL}")]
pub fn provider_kernel() {{
    let _ = unsafe {{ external_device_add(7) }};
}}

#[used]
static __fe2o3_kernel_registration_worker_v2_provider_kernel: (
    u64, u16, u16, &'static str, &'static str, fn(),
) = (
    0x4e52_4b33_4f32_4546, 1, 1, "{PROVIDER_KERNEL}",
    "{PROVIDER_KERNEL}", provider_kernel,
);
"#,
        contract = contract.to_hex(),
    )
}

fn assemble_worker_v2_provider(directory: &Path, workspace: &Path) -> PathBuf {
    let llvm_as = PathBuf::from(
        std::env::var_os(LLVM_AS_ENV)
            .unwrap_or_else(|| panic!("required native integration pin {LLVM_AS_ENV} is absent")),
    );
    assert!(llvm_as.is_absolute(), "{LLVM_AS_ENV} must be absolute");
    let fixture = workspace.join(
        "crates/rustc-codegen-fe2o3/tests/fixtures/worker-v2-provider/external-device-add.ll",
    );
    let provider = directory.join("external-device-add.bc");
    let output = Command::new(&llvm_as)
        .arg(&fixture)
        .arg("-o")
        .arg(&provider)
        .output()
        .unwrap_or_else(|error| panic!("run {}: {error}", llvm_as.display()));
    assert!(
        output.status.success(),
        "LLVM provider assembly failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    provider
}

fn artifact_paths(workspace: &Path, kernel: &str) -> [PathBuf; 3] {
    let directory = workspace.join("target/fe2o3");
    ["ll", "o", "hsaco"].map(|extension| directory.join(format!("{kernel}.{extension}")))
}

fn assert_published_worker_v2_hsaco(artifact_dir: &Path, expected_kernel: &str) {
    let _ = assert_published_worker_v2_kernels_with_version(
        artifact_dir,
        &[expected_kernel],
        fe2o3_hsaco::CodeObjectVersion::V5,
    );
}

fn assert_published_worker_v2_kernels(artifact_dir: &Path, expected_kernels: &[&str]) {
    let _ = assert_published_worker_v2_kernels_with_version(
        artifact_dir,
        expected_kernels,
        fe2o3_hsaco::CodeObjectVersion::V5,
    );
}

fn assert_published_worker_v2_cov6_kernels(
    artifact_dir: &Path,
    expected_kernels: &[&str],
) -> Vec<u8> {
    assert_published_worker_v2_kernels_with_version(
        artifact_dir,
        expected_kernels,
        fe2o3_hsaco::CodeObjectVersion::V6,
    )
}

fn assert_published_worker_v2_kernels_with_version(
    artifact_dir: &Path,
    expected_kernels: &[&str],
    expected_version: fe2o3_hsaco::CodeObjectVersion,
) -> Vec<u8> {
    let mut artifacts = Vec::new();
    let mut records = Vec::new();
    for entry in std::fs::read_dir(artifact_dir).expect("read Worker V2 artifact directory") {
        let entry = entry.expect("read Worker V2 artifact entry");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(".fe2o3-link-artifact-v1-") && name.ends_with(".bin") {
            artifacts.push(entry.path());
        }
        if name.starts_with(".fe2o3-link-publication-v1-") && name.ends_with(".record") {
            records.push(entry.path());
        }
    }
    assert_eq!(
        artifacts.len(),
        1,
        "expected one durable Worker V2 artifact"
    );
    assert_eq!(records.len(), 1, "expected one durable publication record");

    let bytes = std::fs::read(&artifacts[0]).expect("read durable Worker V2 HSACO");
    let inspected = fe2o3_hsaco::inspect(&bytes).expect("inspect durable Worker V2 HSACO");
    assert_eq!(inspected.target().processor(), "gfx942");
    assert_eq!(inspected.code_object_version(), expected_version);
    let mut actual_names = inspected
        .kernels()
        .iter()
        .map(|kernel| kernel.name())
        .collect::<Vec<_>>();
    actual_names.sort_unstable();
    let mut expected_names = expected_kernels.to_vec();
    expected_names.sort_unstable();
    assert_eq!(actual_names, expected_names);
    for kernel in inspected.kernels() {
        assert_eq!(kernel.required_workgroup_size(), Some([256, 1, 1]));
        assert_eq!(kernel.max_flat_workgroup_size(), 256);
        assert_eq!(kernel.wavefront_size(), 64);
    }
    bytes
}

fn export_alpha_zeta_evidence(bytes: &[u8]) {
    let Some(output) = std::env::var_os(ALPHA_ZETA_OUTPUT_ENV) else {
        return;
    };
    let output = PathBuf::from(output);
    assert!(
        output.is_absolute(),
        "{ALPHA_ZETA_OUTPUT_ENV} must be absolute"
    );
    let parent = output.parent().expect("alpha/zeta output parent");
    assert_eq!(
        parent
            .canonicalize()
            .expect("canonical alpha/zeta output parent"),
        parent,
        "{ALPHA_ZETA_OUTPUT_ENV} parent must already be canonical",
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .unwrap_or_else(|error| panic!("create {}: {error}", output.display()));
    file.write_all(bytes)
        .unwrap_or_else(|error| panic!("write {}: {error}", output.display()));
    file.sync_all()
        .unwrap_or_else(|error| panic!("sync {}: {error}", output.display()));
    eprintln!(
        "exported alpha/zeta COV6 evidence {} sha256 {}",
        output.display(),
        sha256_hex(bytes),
    );
}

fn export_s09_debug_hsaco(bytes: &[u8]) -> Option<PathBuf> {
    let output = PathBuf::from(std::env::var_os(S09_DEBUG_HSACO_OUTPUT_ENV)?);
    assert!(
        output.is_absolute(),
        "{S09_DEBUG_HSACO_OUTPUT_ENV} must be absolute"
    );
    let parent = output.parent().expect("S09 output parent");
    assert_eq!(
        parent.canonicalize().expect("canonical S09 output parent"),
        parent,
        "{S09_DEBUG_HSACO_OUTPUT_ENV} parent must already be canonical",
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .unwrap_or_else(|error| panic!("create {}: {error}", output.display()));
    file.write_all(bytes)
        .unwrap_or_else(|error| panic!("write {}: {error}", output.display()));
    file.sync_all()
        .unwrap_or_else(|error| panic!("sync {}: {error}", output.display()));
    Some(output)
}

fn preseed(paths: &[PathBuf]) {
    std::fs::create_dir_all(paths[0].parent().expect("artifact parent"))
        .expect("create artifact directory");
    for path in paths {
        std::fs::write(path, b"preseeded stale artifact")
            .unwrap_or_else(|error| panic!("preseed {}: {error}", path.display()));
    }
}

fn llvm_block<'a>(llvm: &'a str, label: &str) -> &'a str {
    let marker = format!("{label}:\n");
    let start = llvm
        .find(&marker)
        .unwrap_or_else(|| panic!("missing LLVM block {label}"))
        + marker.len();
    let remainder = &llvm[start..];
    let end = remainder
        .find("\nbb")
        .or_else(|| remainder.find("\n}"))
        .unwrap_or(remainder.len());
    &remainder[..end]
}

fn assert_exact_vecadd_llvm(llvm: &str) {
    assert!(llvm.contains(
        "@vecadd(ptr addrspace(1) %arg0.data, i64 %arg0.len, ptr addrspace(1) %arg1.data, i64 %arg1.len, ptr addrspace(1) %arg2.data, i64 %arg2.len)"
    ));
    assert_eq!(llvm.matches("icmp ult i64").count(), 3);
    assert_eq!(llvm.matches("load float").count(), 2);
    assert_eq!(llvm.matches("store float").count(), 1);
    assert_eq!(llvm.matches("fadd float").count(), 1);

    let output_check = llvm_block(llvm, "bb2");
    assert!(output_check.contains("  %v19 = add i64 %arg2.len, 0\n  %v5 = icmp ult i64 %v3, %v19"));
    assert!(!output_check.contains("load float"));
    assert!(!output_check.contains("store float"));
    assert_eq!(
        llvm_block(llvm, "bb3").trim(),
        "br i1 %v5, label %bb4, label %bb7"
    );

    let first_input_check = llvm_block(llvm, "bb4");
    assert!(first_input_check.contains(
        "  %v7 = add i64 %arg0.len, 0\n  %v8 = icmp ult i64 %v4, %v7\n  br i1 %v8, label %bb5, label %bb9"
    ));
    assert!(!first_input_check.contains("load float"));
    assert!(!first_input_check.contains("store float"));

    let first_load_and_second_check = llvm_block(llvm, "bb5");
    assert!(first_load_and_second_check.contains(
        "  %v11 = load float, ptr addrspace(1) %v10, align 4\n  %v12 = add i64 %arg1.len, 0\n  %v13 = icmp ult i64 %v4, %v12\n  br i1 %v13, label %bb6, label %bb9"
    ));
    assert!(!first_load_and_second_check.contains("store float"));

    let second_load_and_store = llvm_block(llvm, "bb6");
    assert!(second_load_and_store.contains(
        "  %v16 = load float, ptr addrspace(1) %v15, align 4\n  %v17 = fadd float %v11, %v16\n  store float %v17, ptr addrspace(1) %v6, align 4\n  br label %bb7"
    ));
    assert_eq!(llvm_block(llvm, "bb7").trim(), "ret void");
    assert_eq!(llvm_block(llvm, "bb9").trim(), "unreachable");
    assert!(llvm.contains("!reqd_work_group_size !0"));
    assert!(!llvm.contains("fe2o3_device"));
}

fn assert_vecadd_publication(workspace: &Path, command: &str, expect_execution: bool) {
    let output = backend(workspace, command, "fe2o3-vecadd", Some("kernel-ir-v1"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "kernel-ir-v1 vecadd {command} failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("selected kernel-ir-v1: verified 1 kernel(s), 4 function(s)"),
        "missing selected-pipeline diagnostic:\n{stderr}"
    );
    assert!(
        stderr.contains("emitted vecadd"),
        "vecadd was not transactionally published:\n{stderr}"
    );
    if expect_execution {
        assert!(
            stdout.contains("vecadd passed for 1024 elements"),
            "vecadd did not execute successfully:\n{stdout}"
        );
    } else {
        assert!(
            !stdout.contains("vecadd passed for 1024 elements"),
            "compile-only vecadd test unexpectedly executed the binary:\n{stdout}"
        );
    }

    let llvm = std::fs::read_to_string(workspace.join("target/fe2o3/vecadd.ll"))
        .expect("published vecadd LLVM IR");
    assert_exact_vecadd_llvm(&llvm);
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain and a local AMD GPU"]
fn opt_in_fill_publishes_g1_and_executes_on_the_gpu() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let output = backend(&workspace, "run", "fe2o3-fill", Some("kernel-ir-v1"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "kernel-ir-v1 fill failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("selected kernel-ir-v1: verified 1 kernel(s), 3 function(s)"),
        "missing selected-pipeline diagnostic:\n{stderr}"
    );
    assert!(
        stderr.contains("emitted fill"),
        "fill was not transactionally published:\n{stderr}"
    );
    assert!(
        stdout.contains("fill passed for 1024 elements"),
        "fill did not execute successfully:\n{stdout}"
    );

    let llvm = std::fs::read_to_string(workspace.join("target/fe2o3/fill.ll"))
        .expect("published fill LLVM IR");
    assert!(llvm.contains("define amdgpu_kernel void @fill"));
    assert!(llvm.contains("mul i64 %v1.group, 256"));
    assert!(llvm.contains("!reqd_work_group_size !0"));
    assert!(!llvm.contains("%base = mul i32 %bid, 256"));
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn opt_in_vecadd_publishes_exact_g1_without_gpu() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    assert_vecadd_publication(&workspace, "build", false);
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain and a local AMD GPU"]
fn opt_in_vecadd_publishes_exact_g1_and_executes_on_the_gpu() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    assert_vecadd_publication(&workspace, "run", true);
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn selected_pipeline_rejects_invalid_or_unsupported_inputs_and_cleans_stale_artifacts() {
    let _lock = backend_test_lock();
    let workspace = workspace();

    let vecadd_artifacts = artifact_paths(&workspace, "vecadd");
    preseed(&vecadd_artifacts);
    let invalid = backend(&workspace, "build", "fe2o3-vecadd", Some("kernel-ir"));
    let invalid_stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(!invalid.status.success(), "invalid selector was accepted");
    assert!(
        invalid_stderr.contains(
            "FE2O3_CODEGEN_PIPELINE must be unset or exactly `legacy-v1`, `kernel-ir-v1`, or `kernel-ir-worker-v2`"
        ),
        "missing strict selector diagnostic:\n{invalid_stderr}"
    );
    assert!(!invalid_stderr.contains("emitted vecadd"));
    for artifact in vecadd_artifacts {
        assert!(
            !artifact.exists(),
            "invalid selector left stale artifact {}",
            artifact.display()
        );
    }

    let copy_artifacts = artifact_paths(&workspace, "copy");
    preseed(&copy_artifacts);
    let unsupported = backend(&workspace, "build", "fe2o3-copy", Some("kernel-ir-v1"));
    let unsupported_stderr = String::from_utf8_lossy(&unsupported.stderr);
    assert!(
        !unsupported.status.success(),
        "unsupported selected kernel unexpectedly compiled"
    );
    assert!(
        unsupported_stderr.contains("does not support kernel export \"copy\""),
        "missing exact admission diagnostic:\n{unsupported_stderr}"
    );
    assert!(
        unsupported_stderr.contains("default legacy-v1 pipeline"),
        "diagnostic did not identify the available legacy path:\n{unsupported_stderr}"
    );
    assert!(!unsupported_stderr.contains("emitted copy"));
    for artifact in copy_artifacts {
        assert!(
            !artifact.exists(),
            "unsupported selected kernel left stale artifact {}",
            artifact.display()
        );
    }
}

#[test]
#[ignore = "runs the configured rustc codegen backend through the managed wrapper"]
fn worker_v2_rejects_a_missing_envelope_without_touching_legacy_artifacts() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let fill_artifacts = artifact_paths(&workspace, "fill");
    preseed(&fill_artifacts);
    let config = WorkerV2TestConfig::missing_envelope(&workspace);

    let output = backend_with_worker_config(
        &workspace,
        "build",
        "fe2o3-fill",
        Some("kernel-ir-worker-v2"),
        Some(&config.0),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "Worker V2 accepted a collection without an FFI envelope"
    );
    assert!(
        stderr.contains(
            "selected kernel-ir-worker-v2: verified compiler-module candidate with 1 kernel(s), 3 function(s)"
        ),
        "the collection did not reach compiler-module candidate verification:\n{stderr}"
    );
    assert!(
        stderr.contains("kernel-ir-worker-v2 requires a complete compiler FFI envelope"),
        "missing fail-closed envelope diagnostic:\n{stderr}"
    );
    assert!(!stderr.contains("emitted fill"));
    for artifact in fill_artifacts {
        assert_eq!(
            std::fs::read(&artifact).unwrap(),
            b"preseeded stale artifact",
            "Worker V2 touched legacy artifact {}",
            artifact.display(),
        );
        std::fs::remove_file(&artifact).expect("remove preseeded legacy artifact");
    }
}

#[test]
#[ignore = "requires the configured native LLVM/LLD Worker V2 executable"]
fn worker_v2_real_source_publishes_inspected_gfx942_hsaco() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let directory = WorkerV2SourceDirectory::new(&workspace);
    let source = directory.0.join("worker-v2-source.rs");
    std::fs::write(&source, worker_v2_source()).expect("write Worker V2 source fixture");
    let worker =
        PathBuf::from(std::env::var_os("FE2O3_LLVM_LINK_WORKER").expect("FE2O3_LLVM_LINK_WORKER"));
    let worker_build_identity =
        std::env::var("FE2O3_LLVM_LINK_WORKER_BUILD_ID").expect("worker build identity");
    let llvm_build_identity = std::env::var("FE2O3_LLVM_BUILD_ID").expect("LLVM build identity");
    let config = WorkerV2TestConfig::native_source(
        &directory.0,
        &workspace,
        &source,
        &worker,
        &worker_build_identity,
        &llvm_build_identity,
    );
    let backend = build_codegen_backend(&workspace);
    let output = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args(["run", "--locked", "-p", "cargo-fe2o3", "--"])
        .arg("rustc")
        .arg(&source)
        .args([
            "--crate-name",
            "worker_v2_source",
            "--edition=2024",
            "--crate-type=lib",
            "--emit=obj",
            "-Cpanic=abort",
            "-Cmetadata=worker-v2-native-source",
        ])
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .arg("-Zmir-enable-passes=-JumpThreading")
        .arg("-o")
        .arg(directory.0.join("host.o"))
        .env("FE2O3_BINDING_WRAPPER_MODE_V1", "1")
        .env("FE2O3_BUILD_SESSION_V1", "77".repeat(16))
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env("FE2O3_HSACO_DIR", directory.0.join("artifacts"))
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_WORKER_V2_CONFIG_V2", &config.0)
        .output()
        .expect("run real-source Worker V2 flow");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Worker V2 publication failed:\n{stderr}"
    );
    assert!(
        stderr.contains("selected kernel-ir-worker-v2: verified compiler-module candidate"),
        "rustc did not construct the compiler module:\n{stderr}"
    );
    assert!(
        stderr.contains("published inert Worker V2 compiler-module handoff"),
        "rustc did not publish the handoff:\n{stderr}"
    );
    for rejected in [
        "Worker V2 execution failed",
        "independent Worker V2 HSACO inspection failed",
        "Worker V2 HSACO publication failed",
        "build-attempt completion failed",
    ] {
        assert!(
            !stderr.contains(rejected),
            "native Worker V2 flow reported {rejected:?}:\n{stderr}"
        );
    }
    assert_published_worker_v2_hsaco(&directory.0.join("artifacts"), "worker_v2_kernel");
}

#[test]
#[ignore = "requires the configured native LLVM/LLD Worker V2 executable"]
fn worker_v2_real_source_publishes_two_kernels_with_one_shared_helper() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let directory = WorkerV2SourceDirectory::new(&workspace);
    let project = directory.0.join("project");
    let source = project.join("src/lib.rs");
    std::fs::create_dir_all(source.parent().expect("multi-kernel source parent"))
        .expect("create multi-kernel Worker V2 project");
    std::fs::write(
        project.join("Cargo.toml"),
        "[workspace]\n\n[package]\nname = \"worker-v2-source\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n",
    )
    .expect("write multi-kernel Worker V2 manifest");
    std::fs::write(&source, worker_v2_multi_kernel_source())
        .expect("write multi-kernel Worker V2 source fixture");
    let worker =
        PathBuf::from(std::env::var_os("FE2O3_LLVM_LINK_WORKER").expect("FE2O3_LLVM_LINK_WORKER"));
    let worker_build_identity =
        std::env::var("FE2O3_LLVM_LINK_WORKER_BUILD_ID").expect("worker build identity");
    let llvm_build_identity = std::env::var("FE2O3_LLVM_BUILD_ID").expect("LLVM build identity");
    let config = WorkerV2TestConfig::native_source(
        &directory.0,
        &project,
        Path::new("src/lib.rs"),
        &worker,
        &worker_build_identity,
        &llvm_build_identity,
    );
    let backend = build_codegen_backend(&workspace);
    let target = directory.0.join("cargo-target");
    let output = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args(["run", "--locked", "-p", "cargo-fe2o3", "--", "build"])
        .arg("--manifest-path")
        .arg(project.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target)
        .arg("--offline")
        .env("FE2O3_BACKEND", &backend)
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_WORKER_V2_CONFIG_V2", &config.0)
        .output()
        .expect("run multi-kernel Worker V2 flow");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "multi-kernel Worker V2 publication failed:\n{stderr}"
    );
    assert!(
        stderr.contains(
            "selected kernel-ir-worker-v2: verified compiler-module candidate with 2 kernel(s)"
        ),
        "rustc did not preserve both kernel roots:\n{stderr}"
    );
    assert_eq!(
        stderr
            .matches("[internal-helper] worker_v2_source__shared_helper")
            .count(),
        1,
        "shared helper was not collected exactly once:\n{stderr}"
    );
    for rejected in [
        "defined-symbol set mismatch",
        "GenericLink candidate and compiler-FFI-aware Worker V2 output bytes differ",
        "Worker V2 execution failed",
        "independent Worker V2 HSACO inspection failed",
        "Worker V2 HSACO publication failed",
        "build-attempt completion failed",
    ] {
        assert!(
            !stderr.contains(rejected),
            "multi-kernel Worker V2 flow reported {rejected:?}:\n{stderr}"
        );
    }
    assert_published_worker_v2_kernels(
        &target.join("fe2o3"),
        &[MULTI_KERNEL_ALPHA, MULTI_KERNEL_ZETA],
    );
}

#[test]
#[ignore = "requires the configured native LLVM/LLD Worker V2 executable"]
fn worker_v2_general_v3_alpha_zeta_build_links_and_validate_backend_witnesses() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let directory = WorkerV2SourceDirectory::new(&workspace);
    let source =
        Path::new("crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs");
    let worker =
        PathBuf::from(std::env::var_os("FE2O3_LLVM_LINK_WORKER").expect("FE2O3_LLVM_LINK_WORKER"));
    let worker_build_identity =
        std::env::var("FE2O3_LLVM_LINK_WORKER_BUILD_ID").expect("worker build identity");
    let llvm_build_identity = std::env::var("FE2O3_LLVM_BUILD_ID").expect("LLVM build identity");
    let config = WorkerV2TestConfig::native_source_for_crate(
        &directory.0,
        &workspace,
        source,
        ("fe2o3_typed_alias_spoof", 6),
        &worker,
        &worker_build_identity,
        &llvm_build_identity,
    );
    let backend = build_codegen_backend(&workspace);
    let target = directory.0.join("cargo-target");
    let output = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args([
            "run",
            "--locked",
            "-p",
            "cargo-fe2o3",
            "--",
            "build",
            "-p",
            "fe2o3-typed-alias-spoof",
            "--features",
            "general-genuine",
            "--target-dir",
        ])
        .arg(&target)
        .env("FE2O3_BACKEND", &backend)
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_WORKER_V2_CONFIG_V2", &config.0)
        .output()
        .expect("build genuine general V3 witness fixture");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "genuine general V3 Worker V2 build failed:\n{stderr}"
    );
    assert!(
        stderr.contains("published inert Worker V2 compiler-module handoff"),
        "genuine general V3 fixture did not publish through Worker V2:\n{stderr}"
    );
    assert!(
        !stderr.contains("undefined reference to `__fe2o3_semantic_witness_v1_"),
        "semantic witness accessors remained unresolved:\n{stderr}"
    );

    let executable = target.join("debug/fe2o3-typed-alias-spoof");
    let run = Command::new(&executable)
        .output()
        .unwrap_or_else(|error| panic!("run {}: {error}", executable.display()));
    assert!(
        run.status.success(),
        "linked general V3 witness validation failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let bytes = assert_published_worker_v2_cov6_kernels(&target.join("fe2o3"), &["alpha", "zeta"]);
    export_alpha_zeta_evidence(&bytes);
}

#[test]
#[ignore = "requires the configured native LLVM/LLD Worker V2 and llvm-dwarfdump"]
fn worker_v2_s09_alpha_o0_preserves_source_dwarf_in_hsaco() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let directory = WorkerV2SourceDirectory::new(&workspace);
    let source =
        Path::new("crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs");
    let worker = PathBuf::from(std::env::var_os("FE2O3_LLVM_LINK_WORKER").expect("Worker V2 path"));
    let worker_build_identity =
        std::env::var("FE2O3_LLVM_LINK_WORKER_BUILD_ID").expect("worker build identity");
    let llvm_build_identity = std::env::var("FE2O3_LLVM_BUILD_ID").expect("LLVM build identity");
    let config = WorkerV2TestConfig::native_s09_alpha_debug(
        &directory.0,
        &workspace,
        source,
        &worker,
        &worker_build_identity,
        &llvm_build_identity,
    );
    let backend = build_codegen_backend(&workspace);
    let target = directory.0.join("cargo-target");
    let output = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args([
            "run",
            "--locked",
            "-p",
            "cargo-fe2o3",
            "--",
            "build",
            "-p",
            "fe2o3-typed-alias-spoof",
            "--features",
            "general-genuine",
            "--target-dir",
        ])
        .arg(&target)
        .env("FE2O3_BACKEND", &backend)
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_WORKER_V2_CONFIG_V2", &config.0)
        .output()
        .expect("build S09 alpha debug fixture");
    assert!(
        output.status.success(),
        "S09 alpha Worker V2 build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = assert_published_worker_v2_cov6_kernels(&target.join("fe2o3"), &["alpha", "zeta"]);
    let hsaco = export_s09_debug_hsaco(&bytes)
        .unwrap_or_else(|| panic!("{S09_DEBUG_HSACO_OUTPUT_ENV} is required"));
    let dwarfdump =
        PathBuf::from(std::env::var_os(LLVM_DWARFDUMP_ENV).expect("pinned llvm-dwarfdump path"));
    assert!(
        dwarfdump.is_absolute(),
        "{LLVM_DWARFDUMP_ENV} must be absolute"
    );
    let verify = Command::new(&dwarfdump)
        .args(["--verify"])
        .arg(&hsaco)
        .output()
        .unwrap_or_else(|error| panic!("run {}: {error}", dwarfdump.display()));
    assert!(
        verify.status.success(),
        "S09 DWARF verification failed:\n{}",
        String::from_utf8_lossy(&verify.stderr)
    );
    let dump = Command::new(&dwarfdump)
        .args(["--debug-info", "--debug-line"])
        .arg(&hsaco)
        .output()
        .unwrap_or_else(|error| panic!("run {}: {error}", dwarfdump.display()));
    assert!(dump.status.success(), "S09 DWARF dump failed");
    let dump = String::from_utf8(dump.stdout).expect("UTF-8 DWARF dump");
    for expected in [
        "DW_TAG_compile_unit",
        "DW_LANG_Rust",
        "DW_TAG_subprogram",
        "DW_AT_name\t(\"alpha\")",
        "DW_AT_decl_line\t(68)",
        "DW_AT_name\t(\"scale\")",
        "DW_AT_name\t(\"input_data\")",
        "DW_AT_name\t(\"input_len\")",
        "DW_AT_name\t(\"output_data\")",
        "DW_AT_name\t(\"output_len\")",
        "DW_AT_name\t(\"i\")",
        "DW_AT_decl_line\t(70)",
        "main.rs",
    ] {
        assert!(
            dump.contains(expected),
            "missing {expected:?} in DWARF:\n{dump}"
        );
    }
    assert_eq!(
        dump.matches("DW_AT_location\t").count(),
        6,
        "S09 requires locations for five physical arguments and local `i`"
    );
    for source_line in [68, 69, 70] {
        assert!(
            dump.lines().any(|line| {
                let mut columns = line.split_whitespace();
                columns
                    .next()
                    .is_some_and(|address| address.starts_with("0x"))
                    && columns.next().and_then(|line| line.parse::<usize>().ok())
                        == Some(source_line)
            }),
            "S09 line table is missing source line {source_line}:\n{dump}"
        );
    }
}

#[test]
#[ignore = "requires the configured native LLVM/LLD Worker V2 executable with matching llvm-as"]
fn worker_v2_real_source_links_an_external_bitcode_provider() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let directory = WorkerV2SourceDirectory::new(&workspace);
    let source = directory.0.join("worker-v2-provider-source.rs");
    std::fs::write(&source, worker_v2_provider_source())
        .expect("write Worker V2 provider source fixture");
    let provider = assemble_worker_v2_provider(&directory.0, &workspace);
    let worker =
        PathBuf::from(std::env::var_os("FE2O3_LLVM_LINK_WORKER").expect("FE2O3_LLVM_LINK_WORKER"));
    let worker_build_identity =
        std::env::var("FE2O3_LLVM_LINK_WORKER_BUILD_ID").expect("worker build identity");
    let llvm_build_identity = std::env::var("FE2O3_LLVM_BUILD_ID").expect("LLVM build identity");
    let config = WorkerV2TestConfig::native_source_with_bitcode_provider(
        &directory.0,
        &workspace,
        &source,
        &provider,
        &worker,
        &worker_build_identity,
        &llvm_build_identity,
    );
    let config_text = std::fs::read_to_string(&config.0).expect("read Worker V2 provider config");
    assert!(config_text.contains("\"format\":\"fe2o3-worker-v2-config-v2\""));
    assert!(!config_text.contains("final_symbols"));
    assert!(config_text.contains("\"kind\":\"llvm-bitcode\""));

    let backend = build_codegen_backend(&workspace);
    let output = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args(["run", "--locked", "-p", "cargo-fe2o3", "--"])
        .arg("rustc")
        .arg(&source)
        .args([
            "--crate-name",
            "worker_v2_provider_source",
            "--edition=2024",
            "--crate-type=lib",
            "--emit=obj",
            "-Cpanic=abort",
            "-Cmetadata=worker-v2-bitcode-provider",
        ])
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .arg("-Zmir-enable-passes=-JumpThreading")
        .arg("-o")
        .arg(directory.0.join("host.o"))
        .env("FE2O3_BINDING_WRAPPER_MODE_V1", "1")
        .env("FE2O3_BUILD_SESSION_V1", "88".repeat(16))
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env("FE2O3_HSACO_DIR", directory.0.join("artifacts"))
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_WORKER_V2_CONFIG_V2", &config.0)
        .output()
        .expect("run provider-backed real-source Worker V2 flow");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "provider-backed Worker V2 publication failed:\n{stderr}"
    );
    assert!(
        stderr.contains("external device import:") && stderr.contains(PROVIDER_SYMBOL),
        "collector did not retain the provider import:\n{stderr}"
    );
    assert!(
        stderr.contains("collected compiler FFI envelope")
            && stderr.contains("1 import(s), 0 export(s)"),
        "compiler envelope did not bind the provider import:\n{stderr}"
    );
    assert!(
        !stderr.contains("has no classified trusted device identity"),
        "compiler-authenticated FFI import did not lower to a kernel IR external declaration:\n{stderr}"
    );
    assert!(
        stderr.contains("published inert Worker V2 compiler-module handoff"),
        "rustc did not publish the compiler module:\n{stderr}"
    );
    for rejected in [
        "compiler-module import has no external provider",
        "linked output retains an unresolved import",
        "unresolved required import",
        "defined-symbol set mismatch",
        "GenericLink candidate and compiler-FFI-aware Worker V2 output bytes differ",
        "Worker V2 execution failed",
        "independent Worker V2 HSACO inspection failed",
        "Worker V2 HSACO publication failed",
        "build-attempt completion failed",
    ] {
        assert!(
            !stderr.contains(rejected),
            "provider-backed Worker V2 flow reported {rejected:?}:\n{stderr}"
        );
    }
    assert_published_worker_v2_hsaco(&directory.0.join("artifacts"), PROVIDER_KERNEL);
}
