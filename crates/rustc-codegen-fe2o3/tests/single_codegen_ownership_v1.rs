#[path = "support/issue272_harness.rs"]
mod harness;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fe2o3_compiler_ffi::{
    CompilerModuleKindV1, CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
    InertSemanticCompilerModuleHandoffV3,
};
use object::{Object as _, ObjectSymbol as _};

use harness::{
    ProtectedCompileResult, ScratchDirectory, fixture_source, forged_source, protected_compile,
    recursively_named, require_failure_with,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FinalHelperDispositionV1 {
    RetainedLocalSymbol,
    InlinedByLlvm,
}

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
) -> (PathBuf, String) {
    let module = handoff.module_handoff();
    assert_eq!(module.kind(), CompilerModuleKindV1::LlvmTextIr);
    let llvm = scratch.path().join("final-v13.ll");
    let relocatable = scratch.path().join("final-v13.o");
    let hsaco = scratch.path().join("final-v13.hsaco");
    let optimization_record = scratch.path().join("final-v13.opt.yaml");
    fs::write(&llvm, module.module_bytes()).expect("write exact final V13 LLVM module");

    let compile = Command::new("/opt/rocm/llvm/bin/clang")
        .args([
            "-x",
            "ir",
            "--target=amdgcn-amd-amdhsa",
            "-mcpu=gfx942",
            "-mcode-object-version=6",
            "-nogpulib",
            "-O2",
            "-c",
            "-fsave-optimization-record",
        ])
        .arg(format!(
            "-foptimization-record-file={}",
            optimization_record.display()
        ))
        .arg(&llvm)
        .arg("-o")
        .arg(&relocatable)
        .output()
        .expect("run production AMD clang over exact V13 LLVM");
    assert!(
        compile.status.success(),
        "AMDGPU compilation failed:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let link = Command::new("/opt/rocm/llvm/bin/ld.lld")
        .args(["-shared"])
        .arg(&relocatable)
        .arg("-o")
        .arg(&hsaco)
        .output()
        .expect("link exact V13 AMDGPU object");
    assert!(
        link.status.success(),
        "AMDGPU link failed:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    let optimization_record = fs::read_to_string(&optimization_record).unwrap_or_default();
    (hsaco, optimization_record)
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
    optimization_record: &str,
) -> BTreeMap<String, FinalHelperDispositionV1> {
    let symbols = defined_symbols(hsaco);
    let counts = occurrence_counts(&symbols);
    let kernels = manifest
        .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
        .collect::<Vec<_>>();
    assert_eq!(kernels.len(), 1, "qualification fixture has one kernel");
    assert_eq!(
        counts.get(kernels[0]).copied(),
        Some(1),
        "the exact kernel export must occur once in the HSACO"
    );
    assert!(
        symbols
            .iter()
            .find(|symbol| symbol.name == kernels[0])
            .is_some_and(|symbol| symbol.global),
        "the kernel entry must remain a global HSACO export"
    );

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
        let disposition = match counts.get(helper).copied().unwrap_or(0) {
            1 => {
                let retained = symbols
                    .iter()
                    .find(|symbol| symbol.name == helper)
                    .expect("counted retained helper");
                assert!(
                    !retained.global,
                    "internal helper escaped as a global export"
                );
                FinalHelperDispositionV1::RetainedLocalSymbol
            }
            0 => {
                assert!(
                    optimization_record.contains(helper) && optimization_record.contains("Inlined"),
                    "removed helper {helper:?} has no LLVM inline transformation record"
                );
                FinalHelperDispositionV1::InlinedByLlvm
            }
            count => panic!("helper {helper:?} occurs {count} times in HSACO"),
        };
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
    let (hsaco, optimization_record) = compile_exact_handoff_to_hsaco(&handoff, &device_scratch);
    let dispositions = verify_device_artifact(&hsaco, manifest, &optimization_record);
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
}
