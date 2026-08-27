use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{CompilerModuleHandoffV2, CompilerModuleSymbolRoleV1};
use fe2o3_hsaco::{
    CodeObjectVersion as InspectedCodeObjectVersion, MAX_HSACO_BYTES,
    inspect_and_bind_kernel_descriptors,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, InertFirstBuildWorkerV2EvidenceV1, LinkOptionV1, PinnedWorkerV1,
    WorkerExecutionLimitsV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    execute_reproducible_first_build_worker_v2,
};

const WORKER_ENV: &str = "FE2O3_PRODUCTION_GFX942_WORKER";
const WORKER_BUILD_ID_ENV: &str = "FE2O3_PRODUCTION_GFX942_WORKER_BUILD_ID";
const LLVM_BUILD_ID_ENV: &str = "FE2O3_PRODUCTION_GFX942_LLVM_BUILD_ID";
const PRODUCTION_DEVICE_RUSTFLAGS: &str = "-Zalways-encode-mir -Zinline-mir=yes -Zinline-mir-hint-threshold=1000 -Zmir-enable-passes=-JumpThreading --cfg fe2o3_codegen_generation=\"0123456789abcdef0123456789abcdef\" -Copt-level=2 -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32";
const SCOPED_ATOMIC_HSACO_OUTPUT_ENV: &str = "FE2O3_PRODUCTION_SCOPED_ATOMIC_HSACO_OUTPUT";
const LDS_REDUCTION_HSACO_OUTPUT_ENV: &str = "FE2O3_PRODUCTION_LDS_REDUCTION_HSACO_OUTPUT";
const LDS_REDUCTION_EXPORT: &str = "lds_publish_read_reduce_i32_v1";
const SCOPED_ATOMIC_EXPLICIT_KERNARG_BYTES: usize = 40;
const LDS_REDUCTION_EXPLICIT_KERNARG_BYTES: usize = 32;
const IMPLICIT_KERNARG_BYTES: usize = 256;
const SCOPED_ATOMIC_COMPLETE_KERNARG_BYTES: usize =
    SCOPED_ATOMIC_EXPLICIT_KERNARG_BYTES + IMPLICIT_KERNARG_BYTES;
const LDS_REDUCTION_COMPLETE_KERNARG_BYTES: usize =
    LDS_REDUCTION_EXPLICIT_KERNARG_BYTES + IMPLICIT_KERNARG_BYTES;
const WORKGROUP_X: u32 = 64;
const ELEMENTS: usize = WORKGROUP_X as usize;

struct ScratchDirectory {
    path: PathBuf,
}

impl ScratchDirectory {
    fn new(case: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-production-workgroup-{case}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create production workgroup scratch directory");
        Self { path }
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn required_env(name: &str) -> String {
    std::env::var(name)
        .unwrap_or_else(|_| panic!("required production worker pin {name} is absent"))
}

fn link_options() -> Vec<LinkOptionV1> {
    [
        ("code-object-version", "6"),
        ("opt-level", "2"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).expect("fixed production link option"))
    .collect()
}

fn extract_workgroup_handoff(
    scratch: &ScratchDirectory,
    case: &str,
    feature: &str,
) -> CompilerModuleHandoffV2 {
    let handoff_path = scratch.path.join("compiler-handoff-v2");
    let binding_path = scratch.path.join("crate-binding-v1");
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace().join("examples/workgroup_sync_v1"))
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_workgroup_sync_v1")
        .env("FE2O3_EXTRACT_CRATE_BINDING_PATH_V1", &binding_path)
        .env(
            "FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1",
            &handoff_path,
        )
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            PRODUCTION_DEVICE_RUSTFLAGS,
        )
        .args([
            "check",
            "--release",
            "--locked",
            "--no-default-features",
            "--features",
            feature,
            "-Zbuild-std=core",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(scratch.path.join("cargo-handoff"))
        .arg("--lib")
        .output()
        .expect("run production workgroup handoff extraction");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
    assert!(
        output.status.success(),
        "{case} handoff extraction failed:\n{stderr}"
    );
    assert!(
        stderr.contains("gfx942 LLVM -> compiler-bound inert handoff")
            && stderr.contains("artifact/launch authority false"),
        "{case} handoff omitted mandatory evidence:\n{stderr}",
    );
    let bytes = std::fs::read(handoff_path).expect("read production compiler handoff");
    let handoff = CompilerModuleHandoffV2::decode(&bytes).expect("decode production handoff");
    assert_eq!(handoff.canonical_bytes(), bytes);
    handoff
}

fn consumed_handoff(
    scratch: &ScratchDirectory,
    handoff: &CompilerModuleHandoffV2,
    producer_name: &str,
) -> fe2o3_artifact_transaction::ConsumedCompilerModuleHandoffV1 {
    fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
    let transaction = scratch.path.join("worker-transaction");
    std::fs::create_dir(&transaction).expect("create worker transaction directory");
    let producer = ProducerIdentity::from_codegen(
        producer_name,
        Some(Path::new("tests/production_workgroup_sync_driver_v1.rs")),
    )
    .expect("production workgroup test producer");
    let attempt = begin_build_attempt(
        &transaction,
        &producer,
        BuildInvocation::from_bytes(*handoff.identity().sha256()),
        BuildSession::from_bytes([0xa7; 16]),
    )
    .expect("begin production workgroup handoff attempt");
    publish_compiler_module_handoff_v1(&transaction, &producer, attempt, handoff.canonical_bytes())
        .expect("publish production workgroup handoff");
    consume_compiler_module_handoff_v1(&transaction, &producer, attempt)
        .expect("consume production workgroup handoff")
}

fn execute_workgroup_handoff(
    scratch: &ScratchDirectory,
    handoff: &CompilerModuleHandoffV2,
    producer_name: &str,
) -> InertFirstBuildWorkerV2EvidenceV1 {
    let worker_path = PathBuf::from(required_env(WORKER_ENV));
    let worker_identity = ContentIdentityV1::calculate(
        &std::fs::read(&worker_path).expect("read production LLVM worker"),
    );
    let measurement = WorkerMeasurementV1::new(
        worker_identity,
        required_env(WORKER_BUILD_ID_ENV),
        required_env(LLVM_BUILD_ID_ENV),
    )
    .expect("construct production worker measurement");
    let worker = PinnedWorkerV1::open(&worker_path, measurement)
        .expect("open measured production LLVM worker");
    execute_reproducible_first_build_worker_v2(
        consumed_handoff(scratch, handoff, producer_name),
        &worker,
        Vec::new(),
        link_options(),
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64)
            .expect("bounded production HSACO output"),
        WorkerExecutionLimitsV1::default(),
    )
    .expect("source-authentic generic upstream LLVM/LLD production")
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn scoped_atomic_explicit_kernarg(
    values_address: u64,
    eligible_address: u64,
    target_address: u64,
) -> [u8; SCOPED_ATOMIC_EXPLICIT_KERNARG_BYTES] {
    let mut bytes = [0; SCOPED_ATOMIC_EXPLICIT_KERNARG_BYTES];
    put_u64(&mut bytes, 0, values_address);
    put_u64(&mut bytes, 8, ELEMENTS as u64);
    put_u64(&mut bytes, 16, eligible_address);
    put_u64(&mut bytes, 24, ELEMENTS as u64);
    put_u64(&mut bytes, 32, target_address);
    bytes
}

fn lds_reduction_explicit_kernarg(
    values_address: u64,
    output_address: u64,
) -> [u8; LDS_REDUCTION_EXPLICIT_KERNARG_BYTES] {
    let mut bytes = [0; LDS_REDUCTION_EXPLICIT_KERNARG_BYTES];
    put_u64(&mut bytes, 0, values_address);
    put_u64(&mut bytes, 8, ELEMENTS as u64);
    put_u64(&mut bytes, 16, output_address);
    put_u64(&mut bytes, 24, 1);
    bytes
}
fn run_case(case: &str, feature: &str) {
    let scratch = ScratchDirectory::new(case);
    let llvm_path = scratch.path.join("kernel.ll");
    let binding_path = scratch.path.join("crate-binding-v1");
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace().join("examples/workgroup_sync_v1"))
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_workgroup_sync_v1")
        .env("FE2O3_EXTRACT_CRATE_BINDING_PATH_V1", &binding_path)
        .env("FE2O3_EXTRACT_GFX942_LLVM_PATH_V1", &llvm_path)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            PRODUCTION_DEVICE_RUSTFLAGS,
        )
        .args([
            "check",
            "--release",
            "--locked",
            "--no-default-features",
            "--features",
            feature,
            "-Zbuild-std=core",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(scratch.path.join("cargo"))
        .arg("--lib")
        .output()
        .expect("run production workgroup extraction");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
    assert!(
        output.status.success(),
        "{case} failed production extraction:\n{stderr}"
    );
    assert!(
        stderr.contains(
            "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> Kernel IR -> composed formal/ranked memory -> gfx942:xnack- LLVM;"
        ) && stderr.contains("artifact/launch authority false"),
        "{case} omitted mandatory production evidence:\n{stderr}",
    );
    let llvm = std::fs::read_to_string(&llvm_path).expect("read extracted gfx942 LLVM");
    assert!(
        llvm.contains("target triple = \"amdgcn-amd-amdhsa\"")
            && llvm.matches("define amdgpu_kernel").count() == 1,
        "{case} omitted the exact AMDGPU kernel ABI",
    );
    if case == "scoped-atomic" {
        assert!(
            llvm.contains("atomicrmw add") && llvm.contains("llvm.trap"),
            "{case} omitted its atomic or terminating trap operation:\n{llvm}",
        );
    } else if case == "lds-reduction" {
        assert!(
            llvm.contains("= internal addrspace(3) global [64 x i32]")
                && llvm.contains("call void asm sideeffect \"s_barrier\", \"\"()")
                && llvm.contains("@llvm.amdgcn.dispatch.ptr")
                && llvm.contains("ptr addrspace(4)")
                && llvm.contains("i64 12")
                && llvm.contains("udiv i64")
                && llvm.contains("!reqd_work_group_size")
                && llvm.contains("!{i32 64, i32 1, i32 1}"),
            "{case} omitted exact LDS, barrier, dispatch, or workgroup evidence:\n{llvm}",
        );
    }
    let binding = std::fs::read_to_string(&binding_path).expect("read crate binding handoff");
    assert!(
        binding.trim().len() == 64 && binding.trim().bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{case} emitted a malformed crate binding handoff",
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn lds_reduction_uses_the_single_production_pipeline() {
    run_case("lds-reduction", "lds-kernel");
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn scoped_atomic_uses_the_single_production_pipeline() {
    run_case("scoped-atomic", "scoped-atomic-kernel");
}

#[test]
#[ignore = "requires the pinned nightly AMD target and measured upstream LLVM/LLD worker"]
fn scoped_atomic_source_handoff_finalizes_and_inspects_reproducible_generic_gfx942_hsaco() {
    let scratch = ScratchDirectory::new("scoped-atomic-worker");
    let handoff = extract_workgroup_handoff(&scratch, "scoped-atomic", "scoped-atomic-kernel");
    assert_eq!(handoff.target().to_string(), "gfx942:xnack-");
    assert_eq!(
        handoff
            .symbol_manifest()
            .role_count(CompilerModuleSymbolRoleV1::KernelEntry),
        1
    );
    assert_eq!(
        handoff
            .symbol_manifest()
            .role_count(CompilerModuleSymbolRoleV1::KernelDescriptor),
        1
    );
    assert_eq!(
        handoff
            .symbol_manifest()
            .role_count(CompilerModuleSymbolRoleV1::UnresolvedExternalImport),
        0
    );
    let llvm = std::str::from_utf8(handoff.module_bytes()).expect("production LLVM is UTF-8");
    assert!(llvm.contains("atomicrmw add"));
    assert!(llvm.contains(".section .fe2o3.kd.v1"));
    assert!(!llvm.contains(".fe2o3.wg-atomic"));
    assert!(!llvm.contains(".fe2o3.wg-lds"));

    let evidence =
        execute_workgroup_handoff(&scratch, &handoff, "production_scoped_atomic_source_v1");
    assert!(!evidence.grants_publication_authority());
    assert!(!evidence.grants_load_authority());
    assert!(!evidence.grants_launch_authority());
    let diagnostics = evidence.exact_replay().response().diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|line| line.contains("post_link.check=target status=ok"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|line| line.contains("post_link.check=metadata status=ok kernels=1"))
    );
    assert!(
        diagnostics
            .iter()
            .all(|line| !line.contains("scoped_atomic_v1_profile")),
        "production source was routed through the legacy exact profile: {diagnostics:?}"
    );

    let bindings = inspect_and_bind_kernel_descriptors(evidence.output_bytes())
        .expect("inspect and bind source-authentic production HSACO");
    let inspection = bindings.inspection();
    assert_eq!(
        inspection.code_object_version(),
        InspectedCodeObjectVersion::V6
    );
    assert_eq!(inspection.target().to_string(), "gfx942:xnack-");
    let [kernel] = inspection.kernels() else {
        panic!("production HSACO must contain exactly one kernel");
    };
    assert_eq!(kernel.name(), "scoped_atomic_add_u32_v1");
    assert_eq!(kernel.symbol(), "scoped_atomic_add_u32_v1.kd");
    assert_eq!(kernel.kernarg_segment_size(), 296);
    assert_eq!(kernel.kernarg_segment_alignment(), 8);
    assert_eq!(kernel.group_segment_fixed_size(), 0);
    assert_eq!(kernel.wavefront_size(), 64);
    assert_eq!(kernel.max_flat_workgroup_size(), 64);
    assert_eq!(kernel.required_workgroup_size(), Some([64, 1, 1]));
    assert_eq!(kernel.explicit_arguments().len(), 5);
    assert_eq!(
        kernel
            .explicit_arguments()
            .iter()
            .map(|argument| (argument.offset(), argument.size()))
            .collect::<Vec<_>>(),
        [(0, 8), (8, 8), (16, 8), (24, 8), (32, 8)]
    );
    let [binding] = bindings.bindings() else {
        panic!("production HSACO must bind exactly one descriptor");
    };
    assert_eq!(binding.descriptor().kernarg_size(), 296);
    assert_eq!(binding.descriptor().wavefront_size(), 64);

    if let Some(path) = std::env::var_os(SCOPED_ATOMIC_HSACO_OUTPUT_ENV) {
        std::fs::write(path, evidence.output_bytes()).expect("write observed production HSACO");
    }
}

#[test]
#[ignore = "requires the pinned nightly AMD target and measured upstream LLVM/LLD worker"]
fn lds_reduction_source_handoff_finalizes_and_inspects_reproducible_generic_gfx942_hsaco() {
    let scratch = ScratchDirectory::new("lds-reduction-worker");
    let handoff = extract_workgroup_handoff(&scratch, "lds-reduction", "lds-kernel");
    assert_eq!(handoff.target().to_string(), "gfx942:xnack-");
    assert_eq!(
        handoff
            .symbol_manifest()
            .role_count(CompilerModuleSymbolRoleV1::KernelEntry),
        1
    );
    assert_eq!(
        handoff
            .symbol_manifest()
            .role_count(CompilerModuleSymbolRoleV1::KernelDescriptor),
        1
    );
    assert_eq!(
        handoff
            .symbol_manifest()
            .role_count(CompilerModuleSymbolRoleV1::UnresolvedExternalImport),
        0
    );
    let llvm = std::str::from_utf8(handoff.module_bytes()).expect("production LLVM is UTF-8");
    assert!(llvm.contains("= internal addrspace(3) global [64 x i32]"));
    assert!(llvm.contains("call void asm sideeffect \"s_barrier\", \"\"()"));
    assert!(llvm.contains("@llvm.amdgcn.dispatch.ptr"));
    assert!(llvm.contains("ptr addrspace(4)"));
    assert!(llvm.contains("i64 12"));
    assert!(llvm.contains(".section .fe2o3.kd.v1"));
    assert!(!llvm.contains(".fe2o3.wg-atomic"));
    assert!(!llvm.contains(".fe2o3.wg-lds"));

    let evidence =
        execute_workgroup_handoff(&scratch, &handoff, "production_lds_reduction_source_v1");
    assert!(!evidence.grants_publication_authority());
    assert!(!evidence.grants_load_authority());
    assert!(!evidence.grants_launch_authority());
    let diagnostics = evidence.exact_replay().response().diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|line| line.contains("post_link.check=target status=ok"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|line| line.contains("post_link.check=metadata status=ok kernels=1"))
    );
    assert!(
        diagnostics.iter().all(|line| {
            !line.contains("workgroup_lds_v1_profile")
                && !line.contains("tiled_gemm_lds_v1_profile")
        }),
        "production source was routed through a legacy exact profile: {diagnostics:?}"
    );
    let bindings = inspect_and_bind_kernel_descriptors(evidence.output_bytes())
        .expect("inspect and bind source-authentic LDS-reduction HSACO");
    let inspection = bindings.inspection();
    assert_eq!(
        inspection.code_object_version(),
        InspectedCodeObjectVersion::V6
    );
    assert_eq!(inspection.target().to_string(), "gfx942:xnack-");
    let [kernel] = inspection.kernels() else {
        panic!("production HSACO must contain exactly one kernel");
    };
    assert_eq!(kernel.name(), LDS_REDUCTION_EXPORT);
    assert_eq!(kernel.symbol(), "lds_publish_read_reduce_i32_v1.kd");
    assert_eq!(kernel.kernarg_segment_size(), 288);
    assert_eq!(kernel.kernarg_segment_alignment(), 8);
    assert_eq!(kernel.group_segment_fixed_size(), 256);
    assert_eq!(kernel.wavefront_size(), 64);
    assert_eq!(kernel.max_flat_workgroup_size(), 64);
    assert_eq!(kernel.required_workgroup_size(), Some([64, 1, 1]));
    assert_eq!(kernel.explicit_arguments().len(), 4);
    assert_eq!(
        kernel
            .explicit_arguments()
            .iter()
            .map(|argument| (argument.offset(), argument.size()))
            .collect::<Vec<_>>(),
        [(0, 8), (8, 8), (16, 8), (24, 8)]
    );
    let [binding] = bindings.bindings() else {
        panic!("production HSACO must bind exactly one descriptor");
    };
    assert_eq!(binding.descriptor().kernarg_size(), 288);
    assert_eq!(binding.descriptor().wavefront_size(), 64);

    if let Some(path) = std::env::var_os(LDS_REDUCTION_HSACO_OUTPUT_ENV) {
        std::fs::write(path, evidence.output_bytes()).expect("write observed production HSACO");
    }
}

#[test]
fn scoped_atomic_explicit_kernarg_matches_the_compiler_abi() {
    let packed = scoped_atomic_explicit_kernarg(0x1111, 0x2222, 0x3333);
    assert_eq!(&packed[0..8], &0x1111_u64.to_le_bytes());
    assert_eq!(&packed[8..16], &(ELEMENTS as u64).to_le_bytes());
    assert_eq!(&packed[16..24], &0x2222_u64.to_le_bytes());
    assert_eq!(&packed[24..32], &(ELEMENTS as u64).to_le_bytes());
    assert_eq!(&packed[32..40], &0x3333_u64.to_le_bytes());
    assert_eq!(SCOPED_ATOMIC_COMPLETE_KERNARG_BYTES, 296);
}

#[test]
fn lds_reduction_explicit_kernarg_matches_the_compiler_abi() {
    let packed = lds_reduction_explicit_kernarg(0x1111, 0x2222);
    assert_eq!(&packed[0..8], &0x1111_u64.to_le_bytes());
    assert_eq!(&packed[8..16], &(ELEMENTS as u64).to_le_bytes());
    assert_eq!(&packed[16..24], &0x2222_u64.to_le_bytes());
    assert_eq!(&packed[24..32], &1_u64.to_le_bytes());
    assert_eq!(LDS_REDUCTION_COMPLETE_KERNARG_BYTES, 288);
}
