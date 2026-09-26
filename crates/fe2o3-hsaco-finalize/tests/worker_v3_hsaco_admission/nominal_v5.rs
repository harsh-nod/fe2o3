//! Synthetic, inert strict-worker transactions. No native or proof authority.
use super::nominal_v3::with_descriptor_section_version;
use super::*;
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V5 as SCRATCH, NominalFinalizationErrorV5,
    NominalWorkerFinalizationErrorV5, PreparedFinalizedNominalWorkerHsacoV5,
    derive_unfinalized_nominal_hsaco_v5, finalize_protected_worker_nominal_hsaco_v5,
    inspect_finalized_nominal_hsaco_v5,
};
use fe2o3_kernel_descriptor::{
    ConditionalInvocationWireErrorV1, DescriptorWireErrorV3, DescriptorWireErrorV5,
    decode_device_descriptor_table_v5,
};
#[path = "nominal_publication_v5.rs"]
mod publication;
#[path = "../fixtures/nominal_v5_descriptor.rs"]
mod support;
use support::{free, substitute_cpu};

fn wires(target: &str, release: &str) -> (Vec<u8>, Vec<u8>) {
    support::wires(target, 1, 0, Some(256), release)
}
fn artifact(wire: &[u8], version: u8, target: &str) -> Vec<u8> {
    with_descriptor_section_version(
        hsaco_fixture::slice_fixture_with_descriptor_table_workgroup_target(wire, 256, target)
            .bytes,
        version,
    )
}
fn source(
    directory: &TestDirectory,
    bytes: Vec<u8>,
    receipt: &[u8],
    config: EvidenceConfig,
    target: &str,
) -> (
    fe2o3_artifact_transaction::BuildAttempt,
    InertProtectedFirstBuildWorkerV3EvidenceV1,
) {
    if target == TARGET {
        return evidence_with_descriptor_source(
            directory,
            bytes,
            config,
            &[("vecadd", "vecadd.kd")],
            Vec::new(),
            Some(receipt),
        );
    }
    assert_eq!(target, "gfx950:xnack-");
    evidence_with_handoff(directory, config, Vec::new(), || {
        let handoff = module_handoff_for_kernels_target(
            config.module_seed,
            &bytes,
            &[("vecadd", "vecadd.kd")],
            DeviceTargetV1::parse(target).unwrap(),
        );
        let capsule = capsule_bytes_with_semantic_to_llvm(
            config.invocation_seed,
            &handoff,
            config.lineage_mutation,
            None,
            Some(ProductionAmdTargetProfileV1::Gfx950),
            Some(receipt),
        );
        InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
            &capsule,
            handoff.canonical_bytes(),
        ))
        .unwrap()
    })
}
fn finalized(
    directory: &TestDirectory,
    wire: &[u8],
    seed: u8,
) -> (
    fe2o3_artifact_transaction::BuildAttempt,
    PreparedFinalizedNominalWorkerHsacoV5,
) {
    let (attempt, evidence) = source(
        directory,
        artifact(wire, 5, TARGET),
        wire,
        EvidenceConfig {
            attempt_seed: seed,
            ..EvidenceConfig::BASE
        },
        TARGET,
    );
    (
        attempt,
        finalize_protected_worker_nominal_hsaco_v5(evidence, SCRATCH, &mut free).unwrap(),
    )
}

#[test]
fn both_targets_keep_exact_v2_receipt_strict_custody_and_no_authority() {
    for target in [TARGET, "gfx950:xnack-"] {
        let directory = TestDirectory::new();
        let (wire, _) = wires(target, "v5-worker");
        let bytes = artifact(&wire, 5, target);
        let (attempt, evidence) = source(
            &directory,
            bytes.clone(),
            &wire,
            EvidenceConfig::BASE,
            target,
        );
        let identity = evidence.identity();
        let binding = evidence.binding();
        assert_eq!(
            evidence
                .handoff()
                .capsule()
                .receipts()
                .abi()
                .canonical_preimage(),
            wire
        );
        let value =
            finalize_protected_worker_nominal_hsaco_v5(evidence, SCRATCH, &mut free).unwrap();
        assert_eq!(value.raw().attempt(), attempt);
        assert_eq!(value.raw().source_evidence_identity(), identity);
        assert_eq!(value.raw().source_evidence().binding(), binding);
        assert_eq!(
            value.raw().policy().launch().required_workgroup_size(),
            [256, 1, 1]
        );
        assert!(
            value
                .output_identity()
                .matches(value.finalized().as_bytes())
        );
        assert!(
            value
                .descriptor_identity()
                .matches(value.finalized().descriptor_bytes())
        );
        assert_eq!(
            derive_unfinalized_nominal_hsaco_v5(value.finalized().as_bytes(), SCRATCH, &mut free)
                .unwrap(),
            bytes
        );
        let inspected =
            inspect_finalized_nominal_hsaco_v5(value.finalized().as_bytes(), SCRATCH, &mut free)
                .unwrap();
        let table = inspected.descriptor_table();
        assert_eq!(
            table.device_target(),
            DeviceTargetV1::parse(target).unwrap()
        );
        let kernel = table.kernel(0, &mut free).unwrap();
        let contract = kernel.conditional_contract(&mut free).unwrap();
        assert_eq!(contract.theorem().cpu_input_commitment, [17; 32]);
        let source_table = decode_device_descriptor_table_v5(&wire, &mut free).unwrap();
        let source_kernel = source_table.kernel(0, &mut free).unwrap();
        assert_eq!(
            contract.canonical_bytes(),
            source_kernel
                .conditional_contract(&mut free)
                .unwrap()
                .canonical_bytes()
        );
        assert!(value.is_structural_only());
        assert!(!value.authenticates_compiler_origin());
        assert!(!value.grants_compiler_authority() && !value.grants_proof_authority());
        assert!(
            !value.grants_publication_authority()
                && !value.grants_load_authority()
                && !value.grants_launch_authority()
        );
    }
}

#[test]
fn exact_entire_receipt_rejects_coherent_foreign_cpu_and_source() {
    let (wire, _) = wires(TARGET, "source");
    let (foreign, _) = wires(TARGET, "foreign");
    for receipt in [substitute_cpu(&wire, true), foreign] {
        decode_device_descriptor_table_v5(&receipt, &mut free).unwrap();
        let directory = TestDirectory::new();
        let (_, evidence) = source(
            &directory,
            artifact(&wire, 5, TARGET),
            &receipt,
            EvidenceConfig::BASE,
            TARGET,
        );
        assert!(matches!(
            finalize_protected_worker_nominal_hsaco_v5(evidence, SCRATCH, &mut free),
            Err(NominalWorkerFinalizationErrorV5::Finalization(
                NominalFinalizationErrorV5::DescriptorSourceMismatch
            ))
        ));
    }
}

#[test]
fn strict_export_and_symbol_closure_remain_mandatory() {
    let (wire, _) = wires(TARGET, "exports");
    let directory = TestDirectory::new();
    let (_, evidence) = source(
        &directory,
        artifact(&wire, 5, TARGET),
        &wire,
        EvidenceConfig {
            lineage_mutation: DescriptorLineageMutation::DifferentExportManifest,
            ..EvidenceConfig::BASE
        },
        TARGET,
    );
    assert!(matches!(
        finalize_protected_worker_nominal_hsaco_v5(evidence, SCRATCH, &mut free),
        Err(NominalWorkerFinalizationErrorV5::ExportManifestMismatch)
    ));
    for symbols in [[("foreign", "vecadd.kd")], [("vecadd", "foreign.kd")]] {
        let directory = TestDirectory::new();
        let (_, evidence) = evidence_with_descriptor_source(
            &directory,
            artifact(&wire, 5, TARGET),
            EvidenceConfig::BASE,
            &symbols,
            Vec::new(),
            Some(&wire),
        );
        let result = finalize_protected_worker_nominal_hsaco_v5(evidence, SCRATCH, &mut free);
        assert!(matches!(
            result,
            Err(NominalWorkerFinalizationErrorV5::Inspection(
                WorkerV3HsacoInspectionError::KernelEntryRoleMismatch
                    | WorkerV3HsacoInspectionError::KernelDescriptorRoleMismatch
            ))
        ));
    }
}

#[test]
fn worker_original_work_exact_one_short_scratch_and_unwind_fail_closed() {
    let (wire, _) = wires(TARGET, "work");
    let directory = TestDirectory::new();
    let (_, evidence) = source(
        &directory,
        artifact(&wire, 5, TARGET),
        &wire,
        EvidenceConfig::BASE,
        TARGET,
    );
    let mut trace = Vec::new();
    finalize_protected_worker_nominal_hsaco_v5(evidence, SCRATCH, &mut |n| {
        trace.push(n);
        free(0)
    })
    .unwrap();
    let total = trace.iter().sum::<usize>();
    for (scratch, allowance, panic) in [
        (SCRATCH, total, false),
        (SCRATCH - 1, total, false),
        (SCRATCH, 0, false),
        (SCRATCH, total - 1, false),
        (SCRATCH, total - 1, true),
    ] {
        let directory = TestDirectory::new();
        let (_, evidence) = source(
            &directory,
            artifact(&wire, 5, TARGET),
            &wire,
            EvidenceConfig::BASE,
            TARGET,
        );
        let mut remaining = allowance;
        let mut seen = Vec::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            finalize_protected_worker_nominal_hsaco_v5(evidence, scratch, &mut |n| {
                seen.push(n);
                if let Some(rest) = remaining.checked_sub(n) {
                    remaining = rest;
                    return Ok(());
                }
                assert!(!panic, "inert V5 callback unwind");
                Err("denied")
            })
        }));
        assert_eq!(seen, trace[..seen.len()]);
        if panic {
            assert!(result.is_err());
        } else if scratch < SCRATCH {
            assert!(seen.is_empty());
            assert!(matches!(
                result.unwrap(),
                Err(NominalWorkerFinalizationErrorV5::Finalization(
                    NominalFinalizationErrorV5::Scratch { .. }
                ))
            ));
        } else if allowance == total {
            result.unwrap().unwrap();
            assert_eq!(remaining, 0);
        } else {
            let error = result.unwrap().unwrap_err();
            assert!(
                matches!(
                    error,
                    NominalWorkerFinalizationErrorV5::Finalization(
                        NominalFinalizationErrorV5::Work("denied")
                            | NominalFinalizationErrorV5::Wire(DescriptorWireErrorV5::Nominal(
                                DescriptorWireErrorV3::Work("denied")
                            ))
                            | NominalFinalizationErrorV5::Wire(DescriptorWireErrorV5::Contract(
                                ConditionalInvocationWireErrorV1::Work("denied")
                            ))
                    )
                ),
                "{error:?}"
            );
        }
    }
}
