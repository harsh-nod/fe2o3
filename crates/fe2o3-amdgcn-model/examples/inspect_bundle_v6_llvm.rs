//! Authority-free LLVM-text inspection of an exact admitted simulation Bundle V6.
//!
//! This example does not construct a production handoff, invoke a worker, optimize,
//! link, publish, prove, load, or launch. Bundle custody is not source authentication.

use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_amdgcn_model::{
    bind_production_target_v1, lower_compiler_module_to_gfx942_xnack_minus_llvm_ir,
};
use fe2o3_kernel_ir::{
    MAX_SIMULATION_BUNDLE_BYTES_V6, Module, OperationKind, VerifiedCanonicalKernelIrV11,
    VerifiedSimulationBundleV6, decode_module_v11,
};
use sha2::{Digest, Sha256};

const TARGET: &str = "gfx942:xnack-";
const MAX_OBSERVATION_BYTES: usize = 64 * 1024 * 1024;
const USAGE: &str = "usage: inspect_bundle_v6_llvm BUNDLE_V6_FILE\n\
    Emits observation-only LLVM text to stdout; no machine artifact or production authority.";

fn main() {
    if let Err(error) = run() {
        eprintln!("inspect_bundle_v6_llvm: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let path = input_path(env::args_os().skip(1))?;
    let file = File::open(&path).map_err(|error| format!("open bundle: {error}"))?;
    if !file
        .metadata()
        .map_err(|error| format!("inspect bundle file: {error}"))?
        .is_file()
    {
        return Err("bundle input must be a regular file".into());
    }
    let bytes = read_bounded(file, MAX_SIMULATION_BUNDLE_BYTES_V6)?;
    let observation = observe(bytes)?;
    io::stdout()
        .lock()
        .write_all(observation.as_bytes())
        .map_err(|error| format!("write observation: {error}"))
}

fn input_path(mut args: impl Iterator<Item = OsString>) -> Result<PathBuf, String> {
    let path = args.next().ok_or_else(|| USAGE.to_owned())?;
    if args.next().is_some() || path == "--help" || path == "-h" {
        return Err(USAGE.to_owned());
    }
    Ok(path.into())
}

fn read_bounded(reader: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let read_limit = u64::try_from(limit)
        .ok()
        .and_then(|limit| limit.checked_add(1))
        .ok_or_else(|| "bundle read limit overflow".to_owned())?;
    let mut bytes = Vec::new();
    reader
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read bundle: {error}"))?;
    if bytes.len() > limit {
        return Err("bundle exceeds the bounded V6 input limit".into());
    }
    Ok(bytes)
}

fn exact_target(target: &str) -> Result<(), String> {
    if target != TARGET {
        return Err(format!(
            "unsupported bundle target {target:?}; requires {TARGET}"
        ));
    }
    Ok(())
}

fn observe(bytes: Vec<u8>) -> Result<String, String> {
    let bundle = VerifiedSimulationBundleV6::from_canonical_bytes(bytes)
        .map_err(|error| format!("V6 admission: {error}"))?;
    exact_target(bundle.target())?;
    let module = decode_module_v11(bundle.canonical_kir_v11())
        .map_err(|error| format!("canonical KIR V11 decode: {error}"))?;
    let (llvm, target_bound) = lower_observed_module(&module)?;
    let mut text = format!(
        "; schema: fe2o3-bundle-v6-llvm-text-observation-v1\n\
         ; observation_only: true\n\
         ; authenticates_compiler_execution: false\n\
         ; source_authenticated: false\n\
         ; grants_production_resume: false\n\
         ; grants_proof_authority: false\n\
         ; grants_artifact_authority: false\n\
         ; grants_load_or_launch: false\n\
         ; machine_code_generated: false\n\
         ; final_machine_inspection: not_exercised\n\
         ; target: {TARGET}\n\
         ; bundle_identity: {}\n\
         ; bundle_subject_identity: {}\n\
         ; canonical_kir_version: 11\n\
         ; canonical_kir_digest: {}\n\
         ; canonical_kir_bytes: {}\n\
         ; target_binding: existing_target_transform_only_no_refinement\n\
         ; target_bound_kir_version: 11\n\
         ; target_bound_kir_digest: {}\n\
         ; target_bound_kir_bytes: {}\n\
         ; semantic_mir_identity: {}\n\
         ; rustc_preflight_plan_receipt_sha256: {}\n\
         ; llvm_text_sha256: {}\n",
        hex(bundle.identity().as_bytes()),
        hex(bundle.subject_identity()),
        hex(bundle.canonical_kir_v11_digest()),
        bundle.canonical_kir_v11_length(),
        hex(target_bound.identity().digest()),
        target_bound.identity().canonical_length(),
        hex(&bundle.semantic_mir_identity()),
        hex(&bundle
            .source_lineage()
            .rustc_preflight_plan_receipt_sha256()),
        hex(&Sha256::digest(llvm.as_bytes())),
    );
    let mut assembly_count = 0_usize;
    let mut call_count = 0_usize;
    for (function_ordinal, function) in module.functions.iter().enumerate() {
        let Some(body) = &function.body else { continue };
        for (block_ordinal, block) in body.blocks.iter().enumerate() {
            for (operation_ordinal, operation) in block.operations.iter().enumerate() {
                match &operation.kind {
                    OperationKind::InlineAssembly(assembly) => {
                        assembly_count += 1;
                        // The lowerer has already checked the closed static mnemonic.
                        // These are inert source references, not authentication receipts
                        // or an association with any final machine-code instruction.
                        append_bounded(
                            &mut text,
                            &format!(
                                "; inline_assembly function_ordinal={function_ordinal} block_ordinal={block_ordinal} operation_ordinal={operation_ordinal} mnemonic={} frontend_unit={} source_function={} contract={} statement={}\n",
                                assembly.mnemonic,
                                hex(&assembly.source.frontend_unit),
                                hex(&assembly.source.function),
                                hex(&assembly.source.contract),
                                hex(&assembly.source.statement),
                            ),
                        )?;
                    }
                    OperationKind::Call { .. } => call_count += 1,
                    _ => {}
                }
            }
        }
    }
    append_bounded(
        &mut text,
        &format!(
            "; inline_assembly_count: {assembly_count}\n; call_operation_count: {call_count}\n; llvm_text_begin\n"
        ),
    )?;
    append_bounded(&mut text, &llvm)?;
    Ok(text)
}

fn lower_observed_module(
    module: &Module,
) -> Result<(String, VerifiedCanonicalKernelIrV11), String> {
    // Bundles carry pre-ranked neutral KIR. Use the existing sole target
    // transform, not manually inserted capabilities or a kernel-only graph.
    // Its structural result is a separate subject, not semantic refinement.
    let bound = bind_production_target_v1(module, ProductionAmdTargetProfileV1::Gfx942)
        .map_err(|error| format!("exact-target binding: {error}"))?;
    let canonical = VerifiedCanonicalKernelIrV11::from_module(bound.module().clone())
        .map_err(|error| format!("target-bound canonical KIR V11: {error}"))?;
    // This existing complete-module entry validates all bodies before returning
    // any text and retains internal definitions and call edges.
    let llvm = lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(bound.module())
        .map_err(|error| format!("exact-target LLVM text lowering: {error}"))?;
    Ok((llvm, canonical))
}

fn append_bounded(output: &mut String, text: &str) -> Result<(), String> {
    append_with_limit(output, text, MAX_OBSERVATION_BYTES)
}

fn append_with_limit(output: &mut String, text: &str, limit: usize) -> Result<(), String> {
    if output
        .len()
        .checked_add(text.len())
        .is_none_or(|len| len > limit)
    {
        return Err("LLVM observation exceeds the output limit".into());
    }
    output
        .try_reserve(text.len())
        .map_err(|_| "LLVM observation allocation failed".to_owned())?;
    output.push_str(text);
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Signature, Terminator,
        WorkgroupSize,
    };

    #[test]
    fn sole_target_binding_is_separate_from_original_and_rejects_missing_entry() {
        let mut module = Module::new("inspection_target_fixture");
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        assert!(
            lower_observed_module(&module)
                .unwrap_err()
                .contains("MissingWorkgroupSize")
        );
        module.kernels[0].workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        let original = VerifiedCanonicalKernelIrV11::from_module(module.clone()).unwrap();
        let (llvm, bound) = lower_observed_module(&module).unwrap();
        assert_ne!(original.identity(), bound.identity());
        assert!(llvm.contains("\"target-cpu\"=\"gfx942\""));
        assert!(module.required_capabilities.is_empty());
        module.functions.clear();
        assert!(
            lower_observed_module(&module)
                .unwrap_err()
                .starts_with("exact-target binding:")
        );
    }

    #[test]
    fn input_is_exactly_one_path_without_target_overrides() {
        assert!(input_path(std::iter::empty()).is_err());
        assert!(input_path(["x".into(), "y".into()].into_iter()).is_err());
        assert!(input_path(["--help".into()].into_iter()).is_err());
        assert_eq!(
            input_path(["input.fe2sim".into()].into_iter()).unwrap(),
            PathBuf::from("input.fe2sim")
        );
    }

    #[test]
    fn bounded_read_accepts_exact_limit_and_refuses_one_more_byte() {
        struct FailedRead;
        impl Read for FailedRead {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("fixture read error"))
            }
        }
        assert_eq!(read_bounded(&b"1234"[..], 4).unwrap(), b"1234");
        assert!(
            read_bounded(&b"12345"[..], 4)
                .unwrap_err()
                .contains("input limit")
        );
        assert!(read_bounded(FailedRead, 4).is_err());
    }

    #[test]
    fn only_exact_gfx942_xnack_minus_target_is_inspectable() {
        assert!(exact_target(TARGET).is_ok());
        for target in [
            "gfx942",
            "gfx942:xnack+",
            "gfx950:xnack-",
            "gfx942:xnack-\n",
        ] {
            assert!(exact_target(target).is_err(), "{target:?}");
        }
    }

    #[test]
    fn invalid_and_legacy_bundles_fail_before_any_observation() {
        for bytes in [
            Vec::new(),
            b"F2SIMB06".to_vec(),
            b"F2SIMB05".to_vec(),
            vec![0; 408],
        ] {
            assert!(observe(bytes).unwrap_err().starts_with("V6 admission:"));
        }
    }

    #[test]
    fn output_limit_failure_preserves_existing_output() {
        let mut text = String::new();
        append_bounded(&mut text, "prefix").unwrap();
        assert!(append_with_limit(&mut text, "x", 6).is_err());
        assert_eq!(text, "prefix");
        assert_eq!(hex(&[0, 15, 16, 255]), "000f10ff");
    }
}
