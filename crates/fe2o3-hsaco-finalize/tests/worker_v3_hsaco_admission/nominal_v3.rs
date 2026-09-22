//! Real fixture-worker custody, synthetic ELF and receipts; no compiler/proof credit.
use super::*;
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3 as SCRATCH, NominalFinalizationErrorV3,
    NominalWorkerFinalizationErrorV3, derive_unfinalized_nominal_hsaco_v3,
    finalize_protected_worker_nominal_hsaco_v3,
};
use fe2o3_kernel_descriptor::*;

fn free(_: usize) -> Result<(), &'static str> {
    Ok(())
}

fn nominal_slice_source(release: &str) -> Vec<u8> {
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new(release).unwrap(),
        [7; 20],
    );
    let producer = ProducerIdentityV1::new(
        Text::new("nominal-worker-fixture").unwrap(),
        Text::new("test").unwrap(),
    );
    let source = SourceTypeRecordV3::new(
        SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::F32),
        &mut free,
    )
    .unwrap();
    let layout = device_layout_record_v3(
        DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32),
        &mut free,
    )
    .unwrap();
    let components = [
        PhysicalComponentV3 {
            kind: PhysicalAbiComponentKind::GlobalPointer,
            offset: 0,
            size: 8,
            alignment: 8,
            access: AccessMode::ReadOnly,
            alias: AliasSemantics::SharedReadOnly,
        },
        PhysicalComponentV3 {
            kind: PhysicalAbiComponentKind::SliceLengthU64,
            offset: 8,
            size: 8,
            alignment: 8,
            access: AccessMode::ByValue,
            alias: AliasSemantics::Value,
        },
    ];
    let args = [LogicalArgumentInputV3 {
        source_index: 0,
        name: "input",
        source_type: source.identity(),
        device_layout: layout.identity(),
        ownership: OwnershipSemantics::SharedBorrow,
        access: AccessMode::ReadOnly,
        alias: AliasSemantics::SharedReadOnly,
        components: &components,
    }];
    let id = KernelId::from_bytes([0xa1; 32]);
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
        DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
        256,
        0,
        0,
    )
    .unwrap();
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([11; 32]),
        EvidenceDigest::from_sha256_bytes([12; 32]),
    );
    let kernels = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: "vecadd",
        entry_name: "vecadd",
        descriptor_symbol: "vecadd.kd",
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities: &[CapabilityV1::AmdWave],
        abi_layout: KernelAbiLayoutV1::new(16, 272, 8).unwrap(),
        launch: &launch,
        arguments: &args,
    }];
    let requirements = [KernelTargetRequirementsV2::new(
        id,
        LdsRequirementsV2::new(0, 0).unwrap(),
        RequiredWavefrontWidthV2::Wave64,
        false,
        SynchronizationRequirementsV2::empty(),
        AtomicRequirementsV2::empty(),
    )];
    let input = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: DescriptorCodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: target(),
        type_records: &[source],
        layout_records: &[layout],
        kernels: &kernels,
        requirements: &requirements,
    };
    let mut wire = vec![0; encoded_device_descriptor_table_v3_len(&input, &mut free).unwrap()];
    encode_device_descriptor_table_v3(&input, &mut wire, &mut free).unwrap();
    wire
}

fn nominal_artifact(wire: &[u8]) -> Vec<u8> {
    let mut bytes = slice_fixture_with_descriptor_table_and_workgroup(wire, 256).bytes;
    let u64_at =
        |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;
    let sections = u64_at(40);
    let strings_index = u16::from_le_bytes(bytes[62..64].try_into().unwrap()) as usize;
    let strings = u64_at(sections + strings_index * 64 + 24);
    let name = u32::from_le_bytes(
        bytes[sections + 6 * 64..sections + 6 * 64 + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let start = strings + name;
    assert_eq!(&bytes[start..start + 13], b".fe2o3.kd.v1\0");
    bytes[start..start + 13].copy_from_slice(b".fe2o3.kd.v3\0");
    bytes
}

fn nominal_evidence(
    directory: &TestDirectory,
    artifact: Vec<u8>,
    wire: &[u8],
    mutation: DescriptorLineageMutation,
) -> InertProtectedFirstBuildWorkerV3EvidenceV1 {
    evidence_with_descriptor_source(
        directory,
        artifact,
        EvidenceConfig {
            lineage_mutation: mutation,
            ..EvidenceConfig::BASE
        },
        &[("vecadd", "vecadd.kd")],
        Vec::new(),
        Some(wire),
    )
    .1
}

#[test]
fn exact_receipt_retains_worker_transaction_and_derived_launch_without_authority() {
    let directory = TestDirectory::new();
    let wire = nominal_slice_source("exact");
    let artifact = nominal_artifact(&wire);
    let source = nominal_evidence(
        &directory,
        artifact.clone(),
        &wire,
        DescriptorLineageMutation::Exact,
    );
    assert_eq!(
        source
            .handoff()
            .capsule()
            .receipts()
            .abi()
            .canonical_preimage(),
        wire
    );
    let identity = source.identity();
    let binding = source.binding();
    let prepared = finalize_protected_worker_nominal_hsaco_v3(source, SCRATCH, &mut free).unwrap();
    assert_eq!(prepared.raw().source_evidence().identity(), identity);
    assert_eq!(prepared.raw().source_evidence().binding(), binding);
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
        derive_unfinalized_nominal_hsaco_v3(prepared.finalized().as_bytes(), SCRATCH, &mut free)
            .unwrap(),
        artifact
    );
    assert!(!prepared.authenticates_compiler_origin());
    assert!(!prepared.grants_publication_authority());
    assert!(!prepared.grants_load_authority());
    assert!(!prepared.grants_launch_authority());
}

#[test]
fn different_canonical_receipt_is_not_rescued_by_physical_abi_agreement() {
    let directory = TestDirectory::new();
    let wire = nominal_slice_source("exact");
    let foreign = nominal_slice_source("foreign");
    let source = nominal_evidence(
        &directory,
        nominal_artifact(&wire),
        &foreign,
        DescriptorLineageMutation::Exact,
    );
    assert!(matches!(
        finalize_protected_worker_nominal_hsaco_v3(source, SCRATCH, &mut free),
        Err(NominalWorkerFinalizationErrorV3::Finalization(
            NominalFinalizationErrorV3::DescriptorSourceMismatch
        ))
    ));
}

#[test]
fn different_export_receipt_is_rejected_before_finalization() {
    let directory = TestDirectory::new();
    let wire = nominal_slice_source("exact");
    let source = nominal_evidence(
        &directory,
        nominal_artifact(&wire),
        &wire,
        DescriptorLineageMutation::DifferentExportManifest,
    );
    assert!(matches!(
        finalize_protected_worker_nominal_hsaco_v3(source, SCRATCH, &mut free),
        Err(NominalWorkerFinalizationErrorV3::ExportManifestMismatch)
    ));
}

#[test]
fn consuming_worker_work_refusals_and_panics_produce_no_result() {
    let wire = nominal_slice_source("exact");
    let artifact = nominal_artifact(&wire);
    let mut total = 0usize;
    {
        let directory = TestDirectory::new();
        let source = nominal_evidence(
            &directory,
            artifact.clone(),
            &wire,
            DescriptorLineageMutation::Exact,
        );
        finalize_protected_worker_nominal_hsaco_v3(source, SCRATCH, &mut |n| {
            total += n;
            free(0)
        })
        .unwrap();
    }
    for (allowance, panic) in [(0, false), (total - 1, false), (total - 1, true)] {
        let directory = TestDirectory::new();
        let source = nominal_evidence(
            &directory,
            artifact.clone(),
            &wire,
            DescriptorLineageMutation::Exact,
        );
        let mut remaining = allowance;
        let mut refused = false;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            finalize_protected_worker_nominal_hsaco_v3(source, SCRATCH, &mut |n| {
                if let Some(rest) = remaining.checked_sub(n) {
                    remaining = rest;
                    return Ok(());
                }
                refused = true;
                assert!(!panic, "deliberate late worker callback panic");
                Err("denied")
            })
        }));
        assert!(refused);
        if panic {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(NominalWorkerFinalizationErrorV3::Finalization(
                    NominalFinalizationErrorV3::Work("denied")
                        | NominalFinalizationErrorV3::Wire(DescriptorWireErrorV3::Work("denied"))
                ))
            ));
        }
    }
}
