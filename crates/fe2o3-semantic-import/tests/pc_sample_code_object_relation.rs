use fe2o3_semantic_import::*;
use fe2o3_semantic_trace::*;
use sha2::{Digest, Sha256};

#[path = "fixtures/pc_sample_code_object_hsaco_fixture.rs"]
mod exact_hsaco_fixture;

use exact_hsaco_fixture::{
    ExactHsacoFixtureV1, exact_sparse_two_kernel_hsaco_v1,
    exact_sparse_two_kernel_hsaco_with_wavefront_v1, official_rocprof_source_v1,
};

const SOURCE: &[u8] = include_bytes!("fixtures/rocprofv3-1.1-stochastic-pc-sampling.json");

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn capture() -> (SemanticPcSampleCaptureV3, Vec<u8>) {
    let capture = import_rocprofv3_pc_sample_capture_v3(
        SOURCE,
        RocprofPcSampleCaptureBindingV3 {
            capture: RocprofCaptureBindingV1 {
                kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(identity(1), 97)
                    .unwrap(),
                artifact: Some(ArtifactClaimV1 {
                    identity: identity(2),
                    canonical_len: 4096,
                    format_version: 1,
                }),
                source_map: None,
                wave_width: WaveWidthV1::Wave64,
            },
            sampling_interval_cycles: 1_048_576,
        },
        ImportLimitsV1::default(),
    )
    .unwrap();
    let bytes = encode_pc_sample_capture_v3(&capture).unwrap();
    (capture, bytes)
}

struct ExactInputsV1 {
    source: Vec<u8>,
    capture_bytes: Vec<u8>,
    artifact: ExactHsacoFixtureV1,
}

fn exact_inputs() -> ExactInputsV1 {
    let artifact = exact_sparse_two_kernel_hsaco_v1();
    let mut source: serde_json::Value = serde_json::from_slice(SOURCE).unwrap();
    let process = &mut source["rocprofiler-sdk-tool"][0];
    let deltas = [-0x2000_i64, 0x10_0000_i64];
    process["code_objects"] = serde_json::Value::Array(
        [2_u64, 3]
            .into_iter()
            .zip(deltas)
            .map(|(code_object_id, load_delta)| {
                serde_json::json!({
                    "code_object_id": code_object_id,
                    "agent_id": {"handle": 18217},
                    "uri": format!("file:///capture/{code_object_id}.hsaco"),
                    "load_base": checked_add_signed(artifact.virtual_base, load_delta),
                    "load_size": artifact.memory_size,
                    "load_delta": load_delta,
                    "storage_type": 1,
                    "memory_base": 0,
                    "memory_size": 0
                })
            })
            .collect(),
    );
    let mut symbols = Vec::new();
    for (code_object_index, (code_object_id, load_delta)) in
        [2_u64, 3].into_iter().zip(deltas).enumerate()
    {
        for (kernel_index, kernel) in artifact.kernels.iter().enumerate() {
            let name = if kernel_index == 0 { "first" } else { "second" };
            symbols.push(serde_json::json!({
                "size": 80,
                "kernel_id": 100 + code_object_index * 2 + kernel_index,
                "code_object_id": code_object_id,
                "kernel_name": name,
                "kernel_object": checked_add_signed(kernel.descriptor_address, load_delta),
                "kernarg_segment_size": kernel.kernarg_size,
                "kernarg_segment_alignment": kernel.kernarg_alignment,
                "group_segment_size": kernel.group_segment_size,
                "private_segment_size": kernel.private_segment_size,
                "formatted_kernel_name": name,
                "demangled_kernel_name": name,
                "truncated_kernel_name": name
            }));
        }
    }
    process["kernel_symbols"] = serde_json::Value::Array(symbols);
    let first_offset = artifact.kernels[0].entry_address - artifact.virtual_base;
    let second_offset = artifact.kernels[1].entry_address - artifact.virtual_base;
    process["buffer_records"]["pc_sample_stochastic"][0]["record"]["pc"]["code_object_offset"] =
        serde_json::json!(first_offset);
    process["buffer_records"]["pc_sample_stochastic"][1]["record"]["pc"]["code_object_offset"] =
        serde_json::json!(first_offset + 4);
    process["buffer_records"]["pc_sample_stochastic"][2]["record"]["pc"]["code_object_offset"] =
        serde_json::json!(second_offset);
    process["buffer_records"]["pc_sample_stochastic"][3]["record"]["pc"]["code_object_offset"] =
        serde_json::json!(second_offset + 4);
    let source = serde_json::to_vec(&source).unwrap();
    let (_, capture_bytes) = capture_for_source(&source, &artifact.bytes);
    ExactInputsV1 {
        source,
        capture_bytes,
        artifact,
    }
}

fn capture_for_source(
    source: &[u8],
    artifact_bytes: &[u8],
) -> (SemanticPcSampleCaptureV3, Vec<u8>) {
    capture_for_source_with_wave_width(source, artifact_bytes, WaveWidthV1::Wave64)
}

fn capture_for_source_with_wave_width(
    source: &[u8],
    artifact_bytes: &[u8],
    wave_width: WaveWidthV1,
) -> (SemanticPcSampleCaptureV3, Vec<u8>) {
    let artifact_digest: [u8; 32] = Sha256::digest(artifact_bytes).into();
    let capture = import_rocprofv3_pc_sample_capture_v3(
        source,
        RocprofPcSampleCaptureBindingV3 {
            capture: RocprofCaptureBindingV1 {
                kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(identity(1), 97)
                    .unwrap(),
                artifact: Some(ArtifactClaimV1 {
                    identity: OpaqueIdentityV1::new(artifact_digest).unwrap(),
                    canonical_len: artifact_bytes.len() as u64,
                    format_version: 1,
                }),
                source_map: None,
                wave_width,
            },
            sampling_interval_cycles: 1_048_576,
        },
        ImportLimitsV1::default(),
    )
    .unwrap();
    let capture_bytes = encode_pc_sample_capture_v3(&capture).unwrap();
    (capture, capture_bytes)
}

fn checked_add_signed(value: u64, delta: i64) -> u64 {
    if delta >= 0 {
        value.checked_add(delta as u64).unwrap()
    } else {
        value.checked_sub(delta.unsigned_abs()).unwrap()
    }
}

fn claims() -> PcSampleCodeObjectRelationClaimsV1 {
    PcSampleCodeObjectRelationClaimsV1 {
        retains_native_addresses: false,
        grants_load_or_execution_authority: false,
        claims_runtime_loaded_bytes_equal_artifact: false,
        claims_complete_code_object_lifetime: false,
        identifies_a_live_pc: false,
        claims_complete_sample_coverage: false,
        claims_complete_instruction_history: false,
        claims_schedule_correlation: false,
        claims_source_attribution: false,
    }
}

fn unavailable_relation() -> (SemanticPcSampleCodeObjectRelationV1, Vec<u8>) {
    let (capture, capture_bytes) = capture();
    let mut records: Vec<_> = capture
        .code_objects
        .iter()
        .map(|code_object| {
            let sample = capture
                .samples
                .iter()
                .find(|sample| sample.pc.code_object_identity == Some(code_object.identity))
                .unwrap();
            let dispatch = capture
                .dispatches
                .iter()
                .find(|dispatch| dispatch.identity == sample.dispatch_identity)
                .unwrap();
            PcSampleCodeObjectRelationRecordV1 {
                code_object_identity: code_object.identity,
                source_code_object_ordinal: code_object.source_code_object_ordinal,
                process_index: dispatch.process_index,
                device_identity: dispatch.device_identity,
                status: PcSampleCodeObjectRelationStatusV1::Unavailable(
                    PcSampleCodeObjectRelationUnavailableReasonV1::MissingStructuredLoad,
                ),
                loaded_code_object_size: None,
            }
        })
        .collect();
    records.sort_by_key(|record| record.code_object_identity);
    (
        SemanticPcSampleCodeObjectRelationV1 {
            schema_version: PC_SAMPLE_CODE_OBJECT_RELATION_SCHEMA_VERSION_V1,
            source_identity: capture.runs[0].source,
            capture_identity: pc_sample_capture_content_identity_v3(&capture_bytes).unwrap(),
            artifact_identity: capture.dispatches[0].artifact.value.unwrap(),
            records,
            symbol_domains: Vec::new(),
            claims: claims(),
        },
        capture_bytes,
    )
}

#[test]
fn official_catalog_admits_sparse_two_kernel_hsaco_and_reopens_exactly() {
    let inputs = exact_inputs();
    let inspected =
        fe2o3_hsaco::inspect_and_bind_kernel_descriptors(&inputs.artifact.bytes).unwrap();
    let layout = inspected.load_layout().unwrap();
    assert_eq!(layout.virtual_base(), inputs.artifact.virtual_base);
    assert_eq!(layout.memory_size(), inputs.artifact.memory_size);
    assert_eq!(inspected.bindings().len(), 2);

    let admitted = admit_rocprofv3_pc_sample_code_object_relation_v1(
        &inputs.source,
        &inputs.capture_bytes,
        &inputs.artifact.bytes,
        ImportLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(admitted.relation().records.len(), 2);
    assert_eq!(admitted.relation().symbol_domains.len(), 4);
    assert!(admitted.relation().records.iter().all(|record| {
        record.status == PcSampleCodeObjectRelationStatusV1::ExactDeclaredArtifactStructure
            && record.loaded_code_object_size == Some(inputs.artifact.memory_size)
    }));
    assert_eq!(admitted.relation().claims, claims());
    let bytes = encode_pc_sample_code_object_relation_v1(&admitted, &inputs.capture_bytes).unwrap();
    assert_eq!(
        decode_pc_sample_code_object_relation_v1(&bytes, &inputs.capture_bytes).unwrap(),
        *admitted.relation()
    );
    assert_eq!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            &inputs.source,
            &inputs.capture_bytes,
            &inputs.artifact.bytes,
            ImportLimitsV1::default(),
        )
        .unwrap()
        .relation(),
        admitted.relation()
    );

    let source: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
    let symbols = source["rocprofiler-sdk-tool"][0]["kernel_symbols"]
        .as_array()
        .unwrap();
    assert!(symbols.iter().all(|symbol| {
        symbol.get("kernel_object").is_some()
            && symbol.get("sgpr_count").is_none()
            && symbol.get("arch_vgpr_count").is_none()
            && symbol.get("accum_vgpr_count").is_none()
            && symbol.get("kernel_code_entry_byte_offset").is_none()
            && symbol.get("kernel_address").is_none()
    }));
}

#[test]
fn capture_and_artifact_wave_widths_must_match_in_both_directions() {
    let wave64_artifact = exact_sparse_two_kernel_hsaco_v1();
    let wave64_source = official_rocprof_source_v1(&wave64_artifact);
    let (_, wave32_capture) = capture_for_source_with_wave_width(
        &wave64_source,
        &wave64_artifact.bytes,
        WaveWidthV1::Wave32,
    );
    assert!(matches!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            &wave64_source,
            &wave32_capture,
            &wave64_artifact.bytes,
            ImportLimitsV1::default()
        ),
        Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution)
    ));

    let wave32_artifact = exact_sparse_two_kernel_hsaco_with_wavefront_v1(32);
    let wave32_source = official_rocprof_source_v1(&wave32_artifact);
    let (_, wave64_capture) = capture_for_source_with_wave_width(
        &wave32_source,
        &wave32_artifact.bytes,
        WaveWidthV1::Wave64,
    );
    assert!(matches!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            &wave32_source,
            &wave64_capture,
            &wave32_artifact.bytes,
            ImportLimitsV1::default()
        ),
        Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution)
    ));
}

#[test]
fn deprecated_agent_alias_is_isolated_and_conflicts_fail_closed() {
    let inputs = exact_inputs();
    let mut deprecated: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
    let load = &mut deprecated["rocprofiler-sdk-tool"][0]["code_objects"][0];
    let agent = load["agent_id"].take();
    load["rocp_agent"] = agent;
    let deprecated = serde_json::to_vec(&deprecated).unwrap();
    let (_, capture_bytes) = capture_for_source(&deprecated, &inputs.artifact.bytes);
    assert!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            &deprecated,
            &capture_bytes,
            &inputs.artifact.bytes,
            ImportLimitsV1::default()
        )
        .is_ok()
    );

    let mut conflicting: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
    conflicting["rocprofiler-sdk-tool"][0]["code_objects"][0]["rocp_agent"] =
        serde_json::json!({"handle": 999});
    let conflicting = serde_json::to_vec(&conflicting).unwrap();
    let (_, capture_bytes) = capture_for_source(&conflicting, &inputs.artifact.bytes);
    assert!(matches!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            &conflicting,
            &capture_bytes,
            &inputs.artifact.bytes,
            ImportLimitsV1::default()
        ),
        Err(PcSampleCodeObjectRelationErrorV1::InvalidStructuredLoad)
    ));
}

#[test]
fn exact_catalog_rejects_load_symbol_resource_and_ownership_substitutions() {
    let inputs = exact_inputs();
    let mut source: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
    source["rocprofiler-sdk-tool"][0]["kernel_symbols"][0]["group_segment_size"] =
        serde_json::json!(1);
    let source = serde_json::to_vec(&source).unwrap();
    let (_, capture_bytes) = capture_for_source(&source, &inputs.artifact.bytes);
    assert!(matches!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            &source,
            &capture_bytes,
            &inputs.artifact.bytes,
            ImportLimitsV1::default()
        ),
        Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution)
    ));

    let mut source: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
    let mut duplicate = source["rocprofiler-sdk-tool"][0]["code_objects"][0].clone();
    duplicate["agent_id"]["handle"] = serde_json::json!(999);
    duplicate["load_delta"] = serde_json::json!(0x20_0000_i64);
    duplicate["load_base"] = serde_json::json!(inputs.artifact.virtual_base + 0x20_0000_u64);
    source["rocprofiler-sdk-tool"][0]["code_objects"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    let source = serde_json::to_vec(&source).unwrap();
    let (_, capture_bytes) = capture_for_source(&source, &inputs.artifact.bytes);
    assert!(matches!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            &source,
            &capture_bytes,
            &inputs.artifact.bytes,
            ImportLimitsV1::default()
        ),
        Err(PcSampleCodeObjectRelationErrorV1::DeviceMismatch)
    ));

    let mut source: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
    source["rocprofiler-sdk-tool"][0]["buffer_records"]["kernel_dispatch"][1]["dispatch_info"]["agent_id"]
        ["handle"] = serde_json::json!(999);
    source["rocprofiler-sdk-tool"][0]["buffer_records"]["pc_sample_stochastic"][2]["record"]["pc"]
        ["code_object_id"] = serde_json::json!(2);
    let source = serde_json::to_vec(&source).unwrap();
    let (_, capture_bytes) = capture_for_source(&source, &inputs.artifact.bytes);
    assert!(matches!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            &source,
            &capture_bytes,
            &inputs.artifact.bytes,
            ImportLimitsV1::default()
        ),
        Err(PcSampleCodeObjectRelationErrorV1::DeviceMismatch)
    ));
}

#[test]
fn load_and_symbol_absence_ambiguity_and_coordinates_are_independently_typed() {
    let inputs = exact_inputs();
    let mut source: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
    let mut duplicate = source["rocprofiler-sdk-tool"][0]["code_objects"][0].clone();
    duplicate["load_delta"] = serde_json::json!(0x20_0000_i64);
    duplicate["load_base"] = serde_json::json!(inputs.artifact.virtual_base + 0x20_0000_u64);
    source["rocprofiler-sdk-tool"][0]["code_objects"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    let source = serde_json::to_vec(&source).unwrap();
    let (_, capture_bytes) = capture_for_source(&source, &inputs.artifact.bytes);
    let admitted = admit_rocprofv3_pc_sample_code_object_relation_v1(
        &source,
        &capture_bytes,
        &inputs.artifact.bytes,
        ImportLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        admitted
            .relation()
            .records
            .iter()
            .find(|record| record.source_code_object_ordinal == 0)
            .unwrap()
            .status,
        PcSampleCodeObjectRelationStatusV1::Unavailable(
            PcSampleCodeObjectRelationUnavailableReasonV1::AmbiguousStructuredLoad
        )
    );

    let mut source: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
    source["rocprofiler-sdk-tool"][0]["code_objects"]
        .as_array_mut()
        .unwrap()
        .retain(|load| load["code_object_id"] != 2);
    let source = serde_json::to_vec(&source).unwrap();
    let (_, capture_bytes) = capture_for_source(&source, &inputs.artifact.bytes);
    let admitted = admit_rocprofv3_pc_sample_code_object_relation_v1(
        &source,
        &capture_bytes,
        &inputs.artifact.bytes,
        ImportLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        admitted
            .relation()
            .records
            .iter()
            .find(|record| record.source_code_object_ordinal == 0)
            .unwrap()
            .status,
        PcSampleCodeObjectRelationStatusV1::Unavailable(
            PcSampleCodeObjectRelationUnavailableReasonV1::MissingStructuredLoad
        )
    );

    let mut source: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
    source["rocprofiler-sdk-tool"][0]["kernel_symbols"]
        .as_array_mut()
        .unwrap()
        .retain(|symbol| symbol["code_object_id"] != 2);
    let source = serde_json::to_vec(&source).unwrap();
    let (_, capture_bytes) = capture_for_source(&source, &inputs.artifact.bytes);
    let admitted = admit_rocprofv3_pc_sample_code_object_relation_v1(
        &source,
        &capture_bytes,
        &inputs.artifact.bytes,
        ImportLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        admitted
            .relation()
            .records
            .iter()
            .find(|record| record.source_code_object_ordinal == 0)
            .unwrap()
            .status,
        PcSampleCodeObjectRelationStatusV1::Unavailable(
            PcSampleCodeObjectRelationUnavailableReasonV1::MissingStructuredKernelSymbols
        )
    );

    for field in ["load_base", "load_size", "load_delta"] {
        let mut source: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
        let value = source["rocprofiler-sdk-tool"][0]["code_objects"][0][field]
            .as_i64()
            .unwrap();
        source["rocprofiler-sdk-tool"][0]["code_objects"][0][field] = serde_json::json!(value + 4);
        let source = serde_json::to_vec(&source).unwrap();
        let (_, capture_bytes) = capture_for_source(&source, &inputs.artifact.bytes);
        assert!(matches!(
            admit_rocprofv3_pc_sample_code_object_relation_v1(
                &source,
                &capture_bytes,
                &inputs.artifact.bytes,
                ImportLimitsV1::default()
            ),
            Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution)
        ));
    }
}

#[test]
fn missing_incomplete_and_process_local_catalogs_remain_typed() {
    let inputs = exact_inputs();
    let mut source: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
    source["rocprofiler-sdk-tool"][0]["kernel_symbols"]
        .as_array_mut()
        .unwrap()
        .retain(|symbol| symbol["code_object_id"] != 2 || symbol["kernel_id"] == 100);
    let source = serde_json::to_vec(&source).unwrap();
    let (_, capture_bytes) = capture_for_source(&source, &inputs.artifact.bytes);
    let admitted = admit_rocprofv3_pc_sample_code_object_relation_v1(
        &source,
        &capture_bytes,
        &inputs.artifact.bytes,
        ImportLimitsV1::default(),
    )
    .unwrap();
    assert!(admitted.relation().records.iter().any(|record| {
        record.status
            == PcSampleCodeObjectRelationStatusV1::Unavailable(
                PcSampleCodeObjectRelationUnavailableReasonV1::IncompleteStructuredKernelSymbols,
            )
    }));

    let mut source: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
    let mut duplicate = source["rocprofiler-sdk-tool"][0]["kernel_symbols"][0].clone();
    duplicate["kernel_id"] = serde_json::json!(999);
    source["rocprofiler-sdk-tool"][0]["kernel_symbols"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    let source = serde_json::to_vec(&source).unwrap();
    let (_, capture_bytes) = capture_for_source(&source, &inputs.artifact.bytes);
    assert!(matches!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            &source,
            &capture_bytes,
            &inputs.artifact.bytes,
            ImportLimitsV1::default(),
        ),
        Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution)
    ));

    let mut source: serde_json::Value = serde_json::from_slice(&inputs.source).unwrap();
    let mut second_process = source["rocprofiler-sdk-tool"][0].clone();
    second_process["metadata"]["pid"] = serde_json::json!(41_053);
    source["rocprofiler-sdk-tool"]
        .as_array_mut()
        .unwrap()
        .push(second_process);
    let source = serde_json::to_vec(&source).unwrap();
    let (capture, capture_bytes) = capture_for_source(&source, &inputs.artifact.bytes);
    assert_eq!(capture.code_objects.len(), 4);
    let admitted = admit_rocprofv3_pc_sample_code_object_relation_v1(
        &source,
        &capture_bytes,
        &inputs.artifact.bytes,
        ImportLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(admitted.relation().records.len(), 4);
    assert!(admitted.relation().records.iter().all(|record| {
        record.status == PcSampleCodeObjectRelationStatusV1::ExactDeclaredArtifactStructure
    }));
}

#[test]
fn sidecar_is_canonical_content_bound_and_preserves_frozen_v3_bytes() {
    let (_, original_capture_bytes) = capture();
    let (relation, capture_bytes) = unavailable_relation();
    assert_eq!(capture_bytes, original_capture_bytes);
    let bytes = serde_json::to_vec(&relation).unwrap();
    assert_eq!(
        decode_pc_sample_code_object_relation_v1(&bytes, &capture_bytes).unwrap(),
        relation
    );
    assert_eq!(
        pc_sample_code_object_relation_content_identity_v1(&bytes, &capture_bytes).unwrap(),
        pc_sample_code_object_relation_content_identity_v1(&bytes, &capture_bytes).unwrap()
    );
    let mut noncanonical = bytes;
    noncanonical.push(b'\n');
    assert!(matches!(
        decode_pc_sample_code_object_relation_v1(&noncanonical, &capture_bytes),
        Err(PcSampleCodeObjectRelationErrorV1::NonCanonicalEncoding)
    ));
}

#[test]
fn stale_capture_claims_catalog_and_symbol_substitutions_are_rejected() {
    let (relation, capture_bytes) = unavailable_relation();

    let mut hostile = relation.clone();
    hostile.capture_identity.digest = CaptureIdentityV1::new([9; 32]).unwrap();
    assert!(matches!(
        decode_pc_sample_code_object_relation_v1(
            &serde_json::to_vec(&hostile).unwrap(),
            &capture_bytes
        ),
        Err(PcSampleCodeObjectRelationErrorV1::StaleCapture)
    ));

    let mut hostile = relation.clone();
    hostile.claims.grants_load_or_execution_authority = true;
    assert!(matches!(
        decode_pc_sample_code_object_relation_v1(
            &serde_json::to_vec(&hostile).unwrap(),
            &capture_bytes
        ),
        Err(PcSampleCodeObjectRelationErrorV1::InvalidClaims)
    ));

    let mut hostile = relation.clone();
    hostile.records[0].process_index = hostile.records[0].process_index.saturating_add(1);
    assert!(matches!(
        decode_pc_sample_code_object_relation_v1(
            &serde_json::to_vec(&hostile).unwrap(),
            &capture_bytes
        ),
        Err(PcSampleCodeObjectRelationErrorV1::ProcessMismatch)
    ));

    let mut hostile = relation.clone();
    hostile.records[0].device_identity = CaptureIdentityV1::new([7; 32]).unwrap();
    assert!(matches!(
        decode_pc_sample_code_object_relation_v1(
            &serde_json::to_vec(&hostile).unwrap(),
            &capture_bytes
        ),
        Err(PcSampleCodeObjectRelationErrorV1::DeviceMismatch)
    ));

    let mut hostile = relation.clone();
    hostile.artifact_identity.digest = CaptureIdentityV1::new([8; 32]).unwrap();
    assert!(matches!(
        decode_pc_sample_code_object_relation_v1(
            &serde_json::to_vec(&hostile).unwrap(),
            &capture_bytes
        ),
        Err(PcSampleCodeObjectRelationErrorV1::ArtifactSubstitution)
    ));

    let mut hostile = relation.clone();
    hostile.records.swap(0, 1);
    assert!(matches!(
        decode_pc_sample_code_object_relation_v1(
            &serde_json::to_vec(&hostile).unwrap(),
            &capture_bytes
        ),
        Err(PcSampleCodeObjectRelationErrorV1::NonCanonicalOrder)
    ));

    let mut hostile = relation;
    hostile.records[0].status = PcSampleCodeObjectRelationStatusV1::ExactDeclaredArtifactStructure;
    hostile.records[0].loaded_code_object_size = Some(128);
    hostile.symbol_domains.push(PcSampleKernelSymbolDomainV1 {
        code_object_identity: hostile.records[0].code_object_identity,
        metadata_kernel_ordinal: 0,
        code_object_offset: 124,
        byte_len: 8,
    });
    assert!(matches!(
        decode_pc_sample_code_object_relation_v1(
            &serde_json::to_vec(&hostile).unwrap(),
            &capture_bytes
        ),
        Err(PcSampleCodeObjectRelationErrorV1::InvalidSymbolDomain)
    ));
}

#[test]
fn duplicate_kernel_ordinal_and_unknown_fields_are_rejected() {
    let (mut relation, capture_bytes) = unavailable_relation();
    relation.records[0].status = PcSampleCodeObjectRelationStatusV1::ExactDeclaredArtifactStructure;
    relation.records[0].loaded_code_object_size = Some(4096);
    let identity = relation.records[0].code_object_identity;
    relation.symbol_domains = vec![
        PcSampleKernelSymbolDomainV1 {
            code_object_identity: identity,
            metadata_kernel_ordinal: 0,
            code_object_offset: 256,
            byte_len: 64,
        },
        PcSampleKernelSymbolDomainV1 {
            code_object_identity: identity,
            metadata_kernel_ordinal: 0,
            code_object_offset: 512,
            byte_len: 64,
        },
    ];
    assert!(matches!(
        decode_pc_sample_code_object_relation_v1(
            &serde_json::to_vec(&relation).unwrap(),
            &capture_bytes
        ),
        Err(PcSampleCodeObjectRelationErrorV1::InvalidSymbolDomain)
    ));

    let (relation, capture_bytes) = unavailable_relation();
    let mut value = serde_json::to_value(relation).unwrap();
    value["native_address"] = serde_json::json!(1234);
    assert!(matches!(
        decode_pc_sample_code_object_relation_v1(
            &serde_json::to_vec(&value).unwrap(),
            &capture_bytes
        ),
        Err(PcSampleCodeObjectRelationErrorV1::JsonDecode)
    ));
}

#[test]
fn sidecar_catalog_bounds_precede_hostile_public_vec_allocation() {
    let (relation, capture_bytes) = unavailable_relation();
    let mut value = serde_json::to_value(&relation).unwrap();
    let record = serde_json::to_value(relation.records[0]).unwrap();
    value["records"] =
        serde_json::Value::Array(vec![record; MAX_PC_SAMPLE_CODE_OBJECT_LOADS_V1 + 1]);
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(bytes.len() < MAX_PC_SAMPLE_CODE_OBJECT_RELATION_BYTES_V1 as usize);
    assert!(matches!(
        decode_pc_sample_code_object_relation_v1(&bytes, &capture_bytes),
        Err(PcSampleCodeObjectRelationErrorV1::JsonDecode)
    ));

    let mut value = serde_json::to_value(&relation).unwrap();
    let domain = serde_json::to_value(PcSampleKernelSymbolDomainV1 {
        code_object_identity: relation.records[0].code_object_identity,
        metadata_kernel_ordinal: 0,
        code_object_offset: 0,
        byte_len: 4,
    })
    .unwrap();
    value["symbol_domains"] =
        serde_json::Value::Array(vec![domain; MAX_PC_SAMPLE_KERNEL_SYMBOLS_V1 + 1]);
    assert!(
        serde_json::from_value::<SemanticPcSampleCodeObjectRelationV1>(value).is_err(),
        "the bounded wire rejects before constructing the public Vec"
    );
}

#[test]
fn admission_rejects_stale_source_artifact_substitution_and_missing_claim() {
    let (_, capture_bytes) = capture();
    assert!(matches!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            SOURCE,
            &capture_bytes,
            &[],
            ImportLimitsV1::default()
        ),
        Err(PcSampleCodeObjectRelationErrorV1::ArtifactSizeOutOfRange)
    ));
    assert!(matches!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            SOURCE,
            &capture_bytes,
            b"substituted artifact",
            ImportLimitsV1::default()
        ),
        Err(PcSampleCodeObjectRelationErrorV1::ArtifactSubstitution)
    ));

    let mut stale_source = SOURCE.to_vec();
    stale_source.push(b' ');
    assert!(matches!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            &stale_source,
            &capture_bytes,
            b"substituted artifact",
            ImportLimitsV1::default()
        ),
        Err(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch)
    ));

    let capture_without_artifact = import_rocprofv3_pc_sample_capture_v3(
        SOURCE,
        RocprofPcSampleCaptureBindingV3 {
            capture: RocprofCaptureBindingV1 {
                kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(identity(1), 97)
                    .unwrap(),
                artifact: None,
                source_map: None,
                wave_width: WaveWidthV1::Wave64,
            },
            sampling_interval_cycles: 1_048_576,
        },
        ImportLimitsV1::default(),
    )
    .unwrap();
    let capture_without_artifact = encode_pc_sample_capture_v3(&capture_without_artifact).unwrap();
    assert!(matches!(
        admit_rocprofv3_pc_sample_code_object_relation_v1(
            SOURCE,
            &capture_without_artifact,
            b"artifact",
            ImportLimitsV1::default()
        ),
        Err(PcSampleCodeObjectRelationErrorV1::ArtifactClaimUnavailable)
    ));
}

#[test]
fn frozen_v3_import_still_ignores_sidecar_only_load_fields() {
    let mut source: serde_json::Value = serde_json::from_slice(SOURCE).unwrap();
    source["rocprofiler-sdk-tool"][0]["code_objects"] = serde_json::json!("not a load array");
    source["rocprofiler-sdk-tool"][0]["kernel_symbols"] = serde_json::json!({"not":"symbols"});
    let source = serde_json::to_vec(&source).unwrap();
    assert!(
        import_rocprofv3_pc_sample_capture_v3(
            &source,
            binding_for_test(),
            ImportLimitsV1::default()
        )
        .is_ok()
    );
}

fn binding_for_test() -> RocprofPcSampleCaptureBindingV3 {
    RocprofPcSampleCaptureBindingV3 {
        capture: RocprofCaptureBindingV1 {
            kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(identity(1), 97).unwrap(),
            artifact: Some(ArtifactClaimV1 {
                identity: identity(2),
                canonical_len: 4096,
                format_version: 1,
            }),
            source_map: None,
            wave_width: WaveWidthV1::Wave64,
        },
        sampling_interval_cycles: 1_048_576,
    }
}
