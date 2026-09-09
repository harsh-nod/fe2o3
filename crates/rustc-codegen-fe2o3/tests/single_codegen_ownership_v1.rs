#[path = "support/issue272_harness.rs"]
mod harness;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, CompilerModuleHandoffSlotV3, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_in_slot_v3,
    publish_compiler_module_handoff_in_slot_v3,
};
use fe2o3_compiler_ffi::{
    CompilerModuleKindV1, CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
    InertSemanticCompilerModuleHandoffV3,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkOptionV1, PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1, execute_protected_reproducible_first_build_worker_v3,
};
use object::{Object as _, ObjectSymbol as _};

use harness::{
    ProtectedCompileResult, ScratchDirectory, fixture_source, forged_source, protected_compile,
    recursively_named, require_failure_with,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FinalHelperDispositionV1 {
    RetainedLocalSymbol,
}

const WORKER_SHA256: &str = "5b3c1536fcc0b6f79e68be6b2cf509ecf0aabffac02c1bfa7325a9e02074cd4a";
const WORKER_BUILD_ID: &str =
    "fe2o3-worker-v1-sha256-3e58c83a82a8643c20787707b9e0e28a520c5300f0912c556cd94c92a89992dc";
const LLVM_BUILD_ID: &str = "rocm-llvm-dev-22.0.0.26014.70200-43~24.04-2ab89c95bdea5adc";

#[derive(Debug)]
struct DefinedSymbolV1 {
    name: String,
    global: bool,
}

fn defined_symbols(path: &Path) -> Vec<DefinedSymbolV1> {
    let bytes = fs::read(path)
        .unwrap_or_else(|error| panic!("read executable artifact {}: {error}", path.display()));
    let object = object::File::parse(bytes.as_slice())
        .unwrap_or_else(|error| panic!("parse executable artifact {}: {error}", path.display()));
    object
        .symbols()
        .filter(|symbol| symbol.is_definition())
        .map(|symbol| DefinedSymbolV1 {
            name: symbol
                .name()
                .unwrap_or_else(|error| {
                    panic!("read defined symbol in {}: {error}", path.display())
                })
                .to_owned(),
            global: symbol.is_global(),
        })
        .collect()
}

fn emitted_handoff(result: &ProtectedCompileResult) -> InertSemanticCompilerModuleHandoffV3 {
    let decoded = recursively_named(&result.artifact_directory, OsStr::new("module"))
        .into_iter()
        .filter_map(|path| {
            let bytes = fs::read(path).expect("read compiler-module handoff candidate");
            InertSemanticCompilerModuleHandoffV3::decode_owned(bytes.into_boxed_slice()).ok()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        decoded.len(),
        1,
        "attributed compilation must emit one exact V3 compiler-module handoff"
    );
    decoded.into_iter().next().unwrap()
}

fn compile_exact_handoff_to_hsaco(
    handoff: &InertSemanticCompilerModuleHandoffV3,
    scratch: &ScratchDirectory,
) -> PathBuf {
    let module = handoff.module_handoff();
    assert_eq!(module.kind(), CompilerModuleKindV1::LlvmTextIr);
    let hsaco = scratch.path().join("final-v13.hsaco");
    fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
    let producer = ProducerIdentity::from_codegen(
        "issue272_w3_qualification",
        Some(Path::new("tests/single_codegen_ownership_v1.rs")),
    )
    .expect("construct qualification producer");
    let attempt = begin_build_attempt(
        scratch.path(),
        &producer,
        BuildInvocation::from_bytes(*handoff.identity().sha256()),
        BuildSession::from_bytes([0x72; 16]),
    )
    .expect("begin exact final-V13 qualification transaction");
    let receipt = publish_compiler_module_handoff_in_slot_v3(
        scratch.path(),
        &producer,
        attempt,
        CompilerModuleHandoffSlotV3::Production,
        handoff,
    )
    .expect("publish exact final-V13 handoff");
    let consumed = consume_compiler_module_handoff_in_slot_v3(
        scratch.path(),
        &producer,
        attempt,
        CompilerModuleHandoffSlotV3::Production,
        handoff.identity(),
    )
    .expect("consume exact final-V13 handoff");
    let parent_closure = *consumed.handoff().capsule().compiler_closure();

    let worker_path =
        harness::workspace().join("target/issue272-qualification/toolchain/fe2o3-llvm-link-worker");
    let worker_bytes = fs::read(&worker_path).unwrap_or_else(|error| {
        panic!(
            "read measured issue-272 worker {}: {error}",
            worker_path.display()
        )
    });
    let worker_identity = ContentIdentityV1::calculate(&worker_bytes);
    let worker_sha256 = worker_identity
        .sha256()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        worker_sha256, WORKER_SHA256,
        "qualification worker does not match the pinned repository build"
    );
    let measurement = WorkerMeasurementV1::new(worker_identity, WORKER_BUILD_ID, LLVM_BUILD_ID)
        .expect("construct exact worker measurement");
    let worker = PinnedWorkerV1::open(&worker_path, measurement)
        .expect("capture measured issue-272 worker image");
    let options = [
        ("verify-each", "true"),
        ("code-object-version", "6"),
        ("strip-debug", "true"),
        ("opt-level", "2"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).expect("valid worker option"))
    .collect();
    let limits =
        WorkerExecutionLimitsV1::new(Duration::from_secs(120), 64 * 1024 * 1024, 64 * 1024)
            .expect("valid worker execution limits");
    let evidence = execute_protected_reproducible_first_build_worker_v3(
        consumed,
        receipt,
        parent_closure,
        &worker,
        Vec::new(),
        options,
        WorkerOutputConstraintsV1::new(64 * 1024 * 1024).expect("valid worker output bound"),
        limits,
    )
    .expect("finalize exact final-V13 handoff with protected Worker V3");
    assert!(evidence.replays_exact_llvm_object_lld_derivation());
    assert_eq!(
        evidence.derivation_evidence().hsaco(),
        evidence.output_identity(),
        "worker derivation must terminate in the inspected HSACO bytes"
    );
    assert!(
        evidence.output_identity().matches(evidence.output_bytes()),
        "worker output bytes must match their exact derivation identity"
    );
    fs::write(&hsaco, evidence.output_bytes()).expect("write exact protected HSACO output");
    hsaco
}

fn occurrence_counts(symbols: &[DefinedSymbolV1]) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for symbol in symbols {
        *counts.entry(symbol.name.as_str()).or_default() += 1;
    }
    counts
}

fn verify_device_artifact(
    hsaco: &Path,
    manifest: &CompilerModuleSymbolManifestV1,
    expected_kernel_count: usize,
) -> BTreeMap<String, FinalHelperDispositionV1> {
    let symbols = defined_symbols(hsaco);
    let counts = occurrence_counts(&symbols);
    let kernels = manifest
        .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
        .collect::<Vec<_>>();
    assert_eq!(kernels.len(), expected_kernel_count);
    for kernel in kernels {
        assert_eq!(
            counts.get(kernel).copied(),
            Some(1),
            "the exact kernel export must occur once in the HSACO"
        );
        assert!(
            symbols
                .iter()
                .find(|symbol| symbol.name == kernel)
                .is_some_and(|symbol| symbol.global),
            "the kernel entry must remain a global HSACO export"
        );
    }

    for role in [
        CompilerModuleSymbolRoleV1::KernelEntry,
        CompilerModuleSymbolRoleV1::KernelDescriptor,
        CompilerModuleSymbolRoleV1::DeviceFfiExport,
        CompilerModuleSymbolRoleV1::InternalHelper,
    ] {
        for expected in manifest.symbols(role) {
            assert!(
                counts.get(expected).copied().unwrap_or(0) <= 1,
                "device symbol {expected:?} has duplicate executable definitions"
            );
        }
    }

    let mut dispositions = BTreeMap::new();
    for helper in manifest.symbols(CompilerModuleSymbolRoleV1::InternalHelper) {
        assert_eq!(
            counts.get(helper).copied(),
            Some(1),
            "the fixture pins every device helper as non-inlined; each must remain in the HSACO"
        );
        let retained = symbols
            .iter()
            .find(|symbol| symbol.name == helper)
            .expect("counted retained helper");
        assert!(
            !retained.global,
            "internal helper escaped as a global export"
        );
        let disposition = FinalHelperDispositionV1::RetainedLocalSymbol;
        assert!(
            dispositions
                .insert(helper.to_owned(), disposition)
                .is_none()
        );
    }
    assert_eq!(
        dispositions.len(),
        manifest
            .symbols(CompilerModuleSymbolRoleV1::InternalHelper)
            .count(),
        "every typed final-V13 helper must have one final disposition"
    );
    dispositions
}

#[test]
fn rustc_rejects_a_forged_reserved_root_without_a_registration() {
    let result = protected_compile(&forged_source(), &[], "forged-prefix");
    require_failure_with(
        &result.output,
        "reserved device-root symbol `__fe2o3_host_kernel_v1_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa` has no authenticated collector root",
    );
    assert!(!result.host_object.exists());
}

#[test]
fn protected_device_global_assembly_is_rejected_before_object_output() {
    let result = protected_compile(
        &fixture_source(),
        &["device-global-asm"],
        "device-global-asm",
    );
    require_failure_with(&result.output, "[FE2O3-OWN-GASM001]");
    assert!(!result.host_object.exists());
    assert!(
        recursively_named(&result.artifact_directory, OsStr::new("module")).is_empty(),
        "rejected global assembly must not publish a compiler module"
    );
}

#[test]
fn protected_host_device_symbol_collision_is_rejected_before_object_output() {
    let result = protected_compile(
        &fixture_source(),
        &["device-symbol-collision"],
        "device-symbol-collision",
    );
    require_failure_with(&result.output, "defines device-owned symbol `owned_kernel`");
    assert!(!result.host_object.exists());
}

#[test]
fn issue272_qualification_release_gate_real_backend_has_single_codegen_ownership() {
    let result = protected_compile(&fixture_source(), &[], "release-gate");
    let stderr = String::from_utf8_lossy(&result.output.stderr);
    assert!(
        result.output.status.success(),
        "attributed rustc failed:\n{stderr}"
    );
    assert!(stderr.contains("production compilation published"));
    assert!(stderr.contains("target gfx942:xnack-"));

    let handoff = emitted_handoff(&result);
    let manifest = handoff.module_handoff().symbol_manifest();
    let host_symbols = defined_symbols(&result.host_object);
    let host_names = host_symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect::<BTreeSet<_>>();
    assert!(host_names.contains("retained_host_symbol"));
    assert!(
        host_names
            .iter()
            .any(|symbol| symbol.contains("owned_kernel")),
        "same-named ordinary host helper must remain in the host object"
    );
    assert!(
        host_names
            .iter()
            .all(|symbol| !symbol.starts_with("__fe2o3_host_kernel_v1_"))
    );
    assert!(
        host_names
            .iter()
            .all(|symbol| !symbol.contains("device_helper"))
    );
    assert!(
        host_names
            .iter()
            .all(|symbol| !symbol.contains("generic_device_helper"))
    );
    assert!(
        host_names
            .iter()
            .all(|symbol| !symbol.contains("DEVICE_BIAS"))
    );
    for role in [
        CompilerModuleSymbolRoleV1::KernelEntry,
        CompilerModuleSymbolRoleV1::DeviceFfiExport,
        CompilerModuleSymbolRoleV1::InternalHelper,
    ] {
        for symbol in manifest.symbols(role) {
            assert!(
                !host_names.contains(symbol),
                "device-owned symbol {symbol:?} leaked into the host object"
            );
        }
    }

    let device_scratch = ScratchDirectory::new("device-artifact");
    let hsaco = compile_exact_handoff_to_hsaco(&handoff, &device_scratch);
    let dispositions = verify_device_artifact(&hsaco, manifest, 1);
    assert!(
        !dispositions.is_empty(),
        "cross-crate and local device helpers must be represented in typed final-V13 custody"
    );
}

#[test]
fn multiple_authenticated_kernels_remain_in_one_final_v13_handoff() {
    let result = protected_compile(&fixture_source(), &["multiple-kernels"], "multiple-kernels");
    let stderr = String::from_utf8_lossy(&result.output.stderr);
    assert!(
        result.output.status.success(),
        "attributed rustc failed:\n{stderr}"
    );
    let handoff = emitted_handoff(&result);
    assert_eq!(
        handoff
            .module_handoff()
            .symbol_manifest()
            .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
            .count(),
        2,
        "both exact authenticated roots must be owned by the single final-V13 module"
    );
    let host = defined_symbols(&result.host_object);
    assert!(
        host.iter()
            .all(|symbol| !symbol.name.starts_with("__fe2o3_host_kernel_v1_"))
    );
    let device_scratch = ScratchDirectory::new("multiple-device-artifact");
    let hsaco = compile_exact_handoff_to_hsaco(&handoff, &device_scratch);
    verify_device_artifact(&hsaco, handoff.module_handoff().symbol_manifest(), 2);
}
