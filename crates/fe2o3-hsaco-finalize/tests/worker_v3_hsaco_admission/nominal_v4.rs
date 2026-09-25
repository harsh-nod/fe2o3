//! Fixture-worker transactions with synthetic ELF/contract bytes, never proof receipts.
use super::nominal_v3::with_descriptor_section_version;
use super::*;
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V4 as SCRATCH, NominalFinalizationErrorV4,
    NominalWorkerFinalizationErrorV4, PreparedFinalizedNominalWorkerHsacoV4,
    derive_unfinalized_nominal_hsaco_v4, finalize_protected_worker_nominal_hsaco_v4,
    inspect_finalized_nominal_hsaco_v4,
};
use fe2o3_kernel_descriptor::*;

#[allow(dead_code)]
#[path = "../../../fe2o3-kernel-descriptor/tests/support/conditional_v4.rs"]
mod descriptor_fixture;
#[path = "nominal_publication_v4.rs"]
mod publication;

fn free(_: usize) -> Result<(), &'static str> {
    Ok(())
}

// Reuse the inert contract transport fixture; no verifier receipt is constructed.
// An output-only conditional contract fits the existing one-slice ELF fixture.
fn wires(
    release: &str,
    customize: impl FnMut(&mut descriptor_fixture::Fixture),
) -> (Vec<u8>, Vec<u8>) {
    descriptor_fixture::with_custom_contracts(TARGET, 1, 0, customize, |input| {
        let compiler = CompilerIdentityV1::new(
            Text::new("rustc").unwrap(),
            Text::new(release).unwrap(),
            [7; 20],
        );
        let launch = LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
            DimensionsV1::new(1024, 1, 1).unwrap(),
            256,
            0,
            0,
        )
        .unwrap();
        let kernel = &input.nominal.kernels[0];
        let kernels = [KernelDescriptorInputV3 {
            kernel_id: kernel.kernel_id,
            logical_name: "vecadd",
            entry_name: "vecadd",
            descriptor_symbol: "vecadd.kd",
            source_evidence: kernel.source_evidence,
            executable_ir_evidence: kernel.executable_ir_evidence,
            capabilities: kernel.capabilities,
            abi_layout: KernelAbiLayoutV1::new(16, 272, 8).unwrap(),
            launch: &launch,
            arguments: kernel.arguments,
        }];
        let input = DeviceDescriptorTableInputV4 {
            nominal: DeviceDescriptorTableInputV3 {
                compiler: &compiler,
                kernels: &kernels,
                ..input.nominal
            },
            contracts: input.contracts,
        };
        let mut v4 = vec![0; encoded_device_descriptor_table_v4_len(&input, &mut free).unwrap()];
        encode_device_descriptor_table_v4(&input, &mut v4, &mut free).unwrap();
        let mut v3 =
            vec![0; encoded_device_descriptor_table_v3_len(&input.nominal, &mut free).unwrap()];
        encode_device_descriptor_table_v3(&input.nominal, &mut v3, &mut free).unwrap();
        (v4, v3)
    })
}

fn artifact(wire: &[u8], version: u8) -> Vec<u8> {
    with_descriptor_section_version(
        slice_fixture_with_descriptor_table_and_workgroup(wire, 256).bytes,
        version,
    )
}

fn source(
    directory: &TestDirectory,
    bytes: Vec<u8>,
    receipt: &[u8],
    config: EvidenceConfig,
) -> (
    fe2o3_artifact_transaction::BuildAttempt,
    InertProtectedFirstBuildWorkerV3EvidenceV1,
) {
    evidence_with_descriptor_source(
        directory,
        bytes,
        config,
        &[("vecadd", "vecadd.kd")],
        Vec::new(),
        Some(receipt),
    )
}

fn finalized(
    directory: &TestDirectory,
    wire: &[u8],
    seed: u8,
) -> (
    fe2o3_artifact_transaction::BuildAttempt,
    PreparedFinalizedNominalWorkerHsacoV4,
) {
    let (attempt, evidence) = source(
        directory,
        artifact(wire, 4),
        wire,
        EvidenceConfig {
            attempt_seed: seed,
            ..EvidenceConfig::BASE
        },
    );
    (
        attempt,
        finalize_protected_worker_nominal_hsaco_v4(evidence, SCRATCH, &mut free).unwrap(),
    )
}

#[test]
fn exact_v4_receipt_retains_contract_and_original_strict_transaction_without_authority() {
    let directory = TestDirectory::new();
    let (wire, _) = wires("exact", |_| {});
    let bytes = artifact(&wire, 4);
    let (attempt, source) = source(&directory, bytes.clone(), &wire, EvidenceConfig::BASE);
    let identity = source.identity();
    let binding = source.binding();
    assert_eq!(
        source
            .handoff()
            .capsule()
            .receipts()
            .abi()
            .canonical_preimage(),
        wire
    );
    let prepared = finalize_protected_worker_nominal_hsaco_v4(source, SCRATCH, &mut free).unwrap();
    assert_eq!(prepared.raw().source_evidence_identity(), identity);
    assert_eq!(prepared.raw().source_evidence().binding(), binding);
    assert_eq!(prepared.raw().attempt(), attempt);
    assert_eq!(
        prepared.raw().policy().launch().required_workgroup_size(),
        [256, 1, 1]
    );
    assert!(
        prepared
            .output_identity()
            .matches(prepared.finalized().as_bytes())
    );
    assert!(
        prepared
            .descriptor_identity()
            .matches(prepared.finalized().descriptor_bytes())
    );
    assert_eq!(
        derive_unfinalized_nominal_hsaco_v4(prepared.finalized().as_bytes(), SCRATCH, &mut free)
            .unwrap(),
        bytes
    );
    let table = decode_device_descriptor_table_v4(&wire, &mut free).unwrap();
    let expected = table
        .kernel(0, &mut free)
        .unwrap()
        .conditional_contract(&mut free)
        .unwrap();
    let inspection =
        inspect_finalized_nominal_hsaco_v4(prepared.finalized().as_bytes(), SCRATCH, &mut free)
            .unwrap();
    let actual = inspection
        .descriptor_table()
        .kernel(0, &mut free)
        .unwrap()
        .conditional_contract(&mut free)
        .unwrap();
    assert_eq!(actual.canonical_bytes(), expected.canonical_bytes());
    assert!(prepared.is_structural_only());
    assert!(!prepared.authenticates_compiler_origin());
    assert!(!prepared.grants_compiler_authority() && !prepared.grants_proof_authority());
    assert!(!prepared.grants_publication_authority());
    assert!(!prepared.grants_load_authority() && !prepared.grants_launch_authority());
}

#[test]
fn valid_source_and_contract_substitutions_fail_exact_v4_receipt_comparison() {
    let (wire, _) = wires("exact", |_| {});
    let (foreign_source, _) = wires("foreign", |_| {});
    let (foreign_contract, _) = wires("exact", |contract| {
        contract.arguments[0].adjusted_argument = 8
    });
    for receipt in [&foreign_source, &foreign_contract] {
        // Each alternative is independently valid and has the same physical ABI.
        decode_device_descriptor_table_v4(receipt, &mut free).unwrap();
        let directory = TestDirectory::new();
        let (_, evidence) = source(
            &directory,
            artifact(&wire, 4),
            receipt,
            EvidenceConfig::BASE,
        );
        assert!(matches!(
            finalize_protected_worker_nominal_hsaco_v4(evidence, SCRATCH, &mut free),
            Err(NominalWorkerFinalizationErrorV4::Finalization(
                NominalFinalizationErrorV4::DescriptorSourceMismatch
            ))
        ));
    }
}

#[test]
fn v4_preserves_strict_export_manifest_rejection() {
    let directory = TestDirectory::new();
    let (wire, _) = wires("exports", |_| {});
    let (_, evidence) = source(
        &directory,
        artifact(&wire, 4),
        &wire,
        EvidenceConfig {
            lineage_mutation: DescriptorLineageMutation::DifferentExportManifest,
            ..EvidenceConfig::BASE
        },
    );
    assert!(matches!(
        finalize_protected_worker_nominal_hsaco_v4(evidence, SCRATCH, &mut free),
        Err(NominalWorkerFinalizationErrorV4::ExportManifestMismatch)
    ));
}

#[test]
fn v4_preserves_strict_handoff_kernel_and_descriptor_symbol_closure() {
    let (wire, _) = wires("symbols", |_| {});
    for symbols in [[("foreign", "vecadd.kd")], [("vecadd", "foreign.kd")]] {
        let directory = TestDirectory::new();
        let (_, evidence) = evidence_with_descriptor_source(
            &directory,
            artifact(&wire, 4),
            EvidenceConfig::BASE,
            &symbols,
            Vec::new(),
            Some(&wire),
        );
        let result = finalize_protected_worker_nominal_hsaco_v4(evidence, SCRATCH, &mut free);
        if symbols[0].0 == "foreign" {
            assert!(matches!(
                result,
                Err(NominalWorkerFinalizationErrorV4::Inspection(
                    WorkerV3HsacoInspectionError::KernelEntryRoleMismatch
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(NominalWorkerFinalizationErrorV4::Inspection(
                    WorkerV3HsacoInspectionError::KernelDescriptorRoleMismatch
                ))
            ));
        }
    }
}

#[test]
fn v4_worker_scratch_work_refusal_and_panic_never_return_an_owner() {
    let (wire, _) = wires("budget", |_| {});
    let mut total = 0;
    let directory = TestDirectory::new();
    let (_, evidence) = source(&directory, artifact(&wire, 4), &wire, EvidenceConfig::BASE);
    finalize_protected_worker_nominal_hsaco_v4(evidence, SCRATCH, &mut |n| {
        total += n;
        free(0)
    })
    .unwrap();
    for (scratch, allowance, panic) in [
        (SCRATCH - 1, total, false),
        (SCRATCH, 0, false),
        (SCRATCH, total - 1, false),
        (SCRATCH, total - 1, true),
    ] {
        let directory = TestDirectory::new();
        let (_, evidence) = source(&directory, artifact(&wire, 4), &wire, EvidenceConfig::BASE);
        let mut remaining = allowance;
        let mut calls = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            finalize_protected_worker_nominal_hsaco_v4(evidence, scratch, &mut |n| {
                calls += 1;
                if let Some(rest) = remaining.checked_sub(n) {
                    remaining = rest;
                    return Ok(());
                }
                assert!(!panic, "deliberate V4 worker callback panic");
                Err("denied")
            })
        }));
        if panic {
            assert!(result.is_err());
        } else if scratch < SCRATCH {
            assert_eq!(calls, 0);
            assert!(matches!(
                result.unwrap(),
                Err(NominalWorkerFinalizationErrorV4::Finalization(
                    NominalFinalizationErrorV4::Scratch { .. }
                ))
            ));
        } else {
            let error = result.unwrap().unwrap_err();
            assert!(
                matches!(
                    error,
                    NominalWorkerFinalizationErrorV4::Finalization(
                        NominalFinalizationErrorV4::Work("denied")
                            | NominalFinalizationErrorV4::Wire(DescriptorWireErrorV4::Nominal(
                                DescriptorWireErrorV3::Work("denied")
                            ))
                            | NominalFinalizationErrorV4::Wire(DescriptorWireErrorV4::Contract(
                                ConditionalInvocationWireErrorV1::Work("denied")
                            ))
                    )
                ),
                "{error:?}"
            );
        }
    }
}
