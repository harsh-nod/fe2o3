use fe2o3_semantic_import::*;
use fe2o3_semantic_query::*;
use fe2o3_semantic_trace::*;
use rmpv::{Value, encode::write_value};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};

const COUNTERS: &[u8] = include_bytes!(
    "../../fe2o3-semantic-import/tests/fixtures/rocprofv3-1.1-counter-collection.json"
);
const PC_SAMPLES: &[u8] = include_bytes!(
    "../../fe2o3-semantic-import/tests/fixtures/rocprofv3-1.1-stochastic-pc-sampling.json"
);
const STRICT_ROCPROF_DISPATCH_JSON: &[u8] = include_bytes!(
    "../../fe2o3-semantic-import/tests/fixtures/rocprofv3-installed-97f5574-kernel-dispatch-schema.json"
);
const TEST_OPAQUE_AGENT_HANDLE: u64 = 7_001;
const TEST_SECOND_OPAQUE_AGENT_HANDLE: u64 = 7_002;
const TEST_ABSOLUTE_KFD_NODE: u64 = 17;

const ELF_HEADER_BYTES: usize = 64;
const SECTION_HEADER_BYTES: usize = 64;

fn opaque(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn sha_identity(bytes: &[u8]) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new(Sha256::digest(bytes).into()).unwrap()
}

fn derived_capture_device(source: CaptureIdentityV1, ordinal: u64) -> CaptureIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3.semantic-capture.device.v1\0");
    hasher.update(source.as_bytes());
    hasher.update(ordinal.to_le_bytes());
    CaptureIdentityV1::new(hasher.finalize().into()).unwrap()
}

fn content(byte: u8) -> ContentIdentityRecordV1 {
    ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: 1,
        digest: CaptureIdentityV1::new([byte; 32]).unwrap(),
        canonical_len: 64,
    }
}

fn environment(absolute_node: u64) -> ProfilerEnvironmentBindingV4 {
    environment_with_claims(
        content(10),
        content(11),
        content(12),
        vec![(absolute_node, content(13))],
    )
}

fn environment_with_claims(
    environment: ContentIdentityRecordV1,
    tool: ContentIdentityRecordV1,
    configuration: ContentIdentityRecordV1,
    devices: Vec<(u64, ContentIdentityRecordV1)>,
) -> ProfilerEnvironmentBindingV4 {
    ProfilerEnvironmentBindingV4 {
        environment,
        collector_tool: tool,
        collector_configuration: configuration,
        stable_device_bindings: devices
            .into_iter()
            .map(
                |(source_agent_id, stable_identity)| ProfilerDeviceBindingV4 {
                    source_agent_id,
                    stable_identity,
                },
            )
            .collect(),
    }
}

fn binding(
    artifact: &[u8],
    kernel_ir: u8,
    absolute_node: u64,
) -> (ProfilerDispatchBindingV4, RocprofCaptureBindingV1) {
    let capture = RocprofCaptureBindingV1 {
        kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(opaque(kernel_ir), 97)
            .unwrap(),
        artifact: Some(ArtifactClaimV1 {
            identity: sha_identity(artifact),
            canonical_len: artifact.len() as u64,
            format_version: 1,
        }),
        source_map: None,
        wave_width: WaveWidthV1::Wave64,
    };
    (
        ProfilerDispatchBindingV4 {
            environment: environment(absolute_node),
            kernel_ir_claim: capture.kernel_ir_claim,
            artifact: capture.artifact,
            source_map: capture.source_map,
            wave_width: capture.wave_width,
        },
        capture,
    )
}

fn strict_dispatch_document(
    first_end: u64,
    second_end: u64,
    opaque_handle: u64,
    absolute_node: u64,
) -> JsonValue {
    assert_ne!(opaque_handle, absolute_node);
    let mut document: JsonValue = serde_json::from_slice(STRICT_ROCPROF_DISPATCH_JSON).unwrap();
    let process = &mut document["rocprofiler-sdk-tool"][0];
    process["metadata"]["pid"] = 7.into();
    process["agents"][0]["id"]["handle"] = opaque_handle.into();
    process["agents"][0]["node_id"] = absolute_node.into();
    let mut first = process["buffer_records"]["kernel_dispatch"][0].clone();
    first["start_timestamp"] = 100.into();
    first["end_timestamp"] = first_end.into();
    first["dispatch_info"]["agent_id"]["handle"] = opaque_handle.into();
    first["dispatch_info"]["dispatch_id"] = 1.into();
    first["dispatch_info"]["workgroup_size"] = serde_json::json!({"x": 64, "y": 1, "z": 1});
    first["dispatch_info"]["grid_size"] = serde_json::json!({"x": 256, "y": 1, "z": 1});
    let mut second = first.clone();
    second["start_timestamp"] = 200.into();
    second["end_timestamp"] = second_end.into();
    second["dispatch_info"]["dispatch_id"] = 2.into();
    second["dispatch_info"]["workgroup_size"] = serde_json::json!({"x": 32, "y": 1, "z": 1});
    second["dispatch_info"]["grid_size"] = serde_json::json!({"x": 128, "y": 1, "z": 1});
    process["buffer_records"]["kernel_dispatch"] = JsonValue::Array(vec![first, second]);
    document
}

fn dispatch_source(first_end: u64, second_end: u64) -> Vec<u8> {
    serde_json::to_vec(&strict_dispatch_document(
        first_end,
        second_end,
        TEST_OPAQUE_AGENT_HANDLE,
        TEST_ABSOLUTE_KFD_NODE,
    ))
    .unwrap()
}

fn dispatch_source_agents(first_node: u64, second_node: u64) -> Vec<u8> {
    let mut source = strict_dispatch_document(140, 260, TEST_OPAQUE_AGENT_HANDLE, first_node);
    let process = &mut source["rocprofiler-sdk-tool"][0];
    let mut second_agent = process["agents"][0].clone();
    second_agent["id"]["handle"] = TEST_SECOND_OPAQUE_AGENT_HANDLE.into();
    second_agent["node_id"] = second_node.into();
    second_agent["gpu_id"] = 43.into();
    process["agents"] = JsonValue::Array(vec![process["agents"][0].clone(), second_agent]);
    let dispatches = process["buffer_records"]["kernel_dispatch"]
        .as_array_mut()
        .unwrap();
    dispatches[0]["dispatch_info"]["agent_id"]["handle"] = TEST_OPAQUE_AGENT_HANDLE.into();
    dispatches[1]["dispatch_info"]["agent_id"]["handle"] = TEST_SECOND_OPAQUE_AGENT_HANDLE.into();
    serde_json::to_vec(&source).unwrap()
}

fn selector_mismatch_source() -> Vec<u8> {
    let mut source: JsonValue = serde_json::from_slice(&dispatch_source(140, 260)).unwrap();
    let mut second_process = source["rocprofiler-sdk-tool"][0].clone();
    let second = second_process["buffer_records"]["kernel_dispatch"]
        .as_array_mut()
        .unwrap()
        .pop()
        .unwrap();
    source["rocprofiler-sdk-tool"][0]["buffer_records"]["kernel_dispatch"]
        .as_array_mut()
        .unwrap()
        .pop();
    second_process["metadata"]["pid"] = 8.into();
    second_process["buffer_records"]["kernel_dispatch"] = JsonValue::Array(vec![second]);
    source["rocprofiler-sdk-tool"]
        .as_array_mut()
        .unwrap()
        .push(second_process);
    serde_json::to_vec(&source).unwrap()
}

fn combined_counter_source(first_end: u64, second_end: u64, last_value: f64) -> Vec<u8> {
    combined_counter_source_for_agent(
        first_end,
        second_end,
        last_value,
        TEST_OPAQUE_AGENT_HANDLE,
        TEST_ABSOLUTE_KFD_NODE,
    )
}

fn combined_counter_source_for_agent(
    first_end: u64,
    second_end: u64,
    last_value: f64,
    opaque_handle: u64,
    absolute_node: u64,
) -> Vec<u8> {
    let counters: JsonValue = serde_json::from_slice(COUNTERS).unwrap();
    let mut source = strict_dispatch_document(first_end, second_end, opaque_handle, absolute_node);
    source["rocprofiler-sdk-tool"][0]["callback_records"]["counter_collection"] =
        counters["rocprofiler-sdk-tool"][0]["callback_records"]["counter_collection"].clone();
    source["rocprofiler-sdk-tool"][0]["counters"] =
        counters["rocprofiler-sdk-tool"][0]["counters"].clone();
    for counter in source["rocprofiler-sdk-tool"][0]["counters"]
        .as_array_mut()
        .unwrap()
    {
        counter["agent_id"]["handle"] = opaque_handle.into();
    }
    let collections = source["rocprofiler-sdk-tool"][0]["callback_records"]["counter_collection"]
        .as_array_mut()
        .unwrap();
    for (collection, end_timestamp) in collections.iter_mut().zip([first_end, second_end]) {
        collection["dispatch_data"]["end_timestamp"] = end_timestamp.into();
        collection["dispatch_data"]["dispatch_info"]["agent_id"]["handle"] = opaque_handle.into();
    }
    collections[1]["records"][0]["value"] = JsonValue::from(last_value);
    serde_json::to_vec(&source).unwrap()
}

fn combined_counter_source_with_count(count: usize, value: f64) -> Vec<u8> {
    assert!(count >= 2);
    let mut source: JsonValue =
        serde_json::from_slice(&combined_counter_source(140, 260, value)).unwrap();
    let collections = source["rocprofiler-sdk-tool"][0]["callback_records"]["counter_collection"]
        .as_array_mut()
        .unwrap();
    collections[0]["records"] = JsonValue::Array(
        (0..count - 1)
            .map(|_| {
                serde_json::json!({
                    "counter_id": {"handle": 101},
                    "value": value
                })
            })
            .collect(),
    );
    collections[1]["records"] = JsonValue::Array(vec![serde_json::json!({
        "counter_id": {"handle": 101},
        "value": value
    })]);
    serde_json::to_vec(&source).unwrap()
}

fn with_counter_name(source: Vec<u8>, name: &str) -> Vec<u8> {
    let mut source: JsonValue = serde_json::from_slice(&source).unwrap();
    source["rocprofiler-sdk-tool"][0]["counters"][0]["name"] = name.into();
    serde_json::to_vec(&source).unwrap()
}

fn pc_source(absolute_node: u64) -> Vec<u8> {
    let opaque_handle = absolute_node.checked_add(1_000_000).unwrap();
    let samples: JsonValue = serde_json::from_slice(PC_SAMPLES).unwrap();
    let sample_process = &samples["rocprofiler-sdk-tool"][0];
    let sample_dispatches = sample_process["buffer_records"]["kernel_dispatch"]
        .as_array()
        .unwrap();
    assert_eq!(sample_dispatches.len(), 2);
    let mut source = strict_dispatch_document(1, 2, opaque_handle, absolute_node);
    let process = &mut source["rocprofiler-sdk-tool"][0];
    let dispatches = process["buffer_records"]["kernel_dispatch"]
        .as_array_mut()
        .unwrap();
    for (dispatch, sample) in dispatches.iter_mut().zip(sample_dispatches) {
        dispatch["start_timestamp"] = sample["start_timestamp"].clone();
        dispatch["end_timestamp"] = sample["end_timestamp"].clone();
        dispatch["dispatch_info"]["agent_id"]["handle"] = opaque_handle.into();
        for field in ["dispatch_id", "workgroup_size", "grid_size"] {
            dispatch["dispatch_info"][field] = sample["dispatch_info"][field].clone();
        }
    }
    for field in ["pc_sample_host_trap", "pc_sample_stochastic"] {
        process["buffer_records"][field] = sample_process["buffer_records"][field].clone();
    }
    serde_json::to_vec(&source).unwrap()
}

fn duplicate_envelope_counter_source(reverse_collections: bool) -> Vec<u8> {
    let mut source: JsonValue =
        serde_json::from_slice(&combined_counter_source(140, 260, 9.0)).unwrap();
    let dispatches = source["rocprofiler-sdk-tool"][0]["buffer_records"]["kernel_dispatch"]
        .as_array_mut()
        .unwrap();
    let second_dispatch_id = dispatches[1]["dispatch_info"]["dispatch_id"].clone();
    dispatches[1] = dispatches[0].clone();
    dispatches[1]["dispatch_info"]["dispatch_id"] = second_dispatch_id;

    let collections = source["rocprofiler-sdk-tool"][0]["callback_records"]["counter_collection"]
        .as_array_mut()
        .unwrap();
    let second_dispatch_id =
        collections[1]["dispatch_data"]["dispatch_info"]["dispatch_id"].clone();
    let second_records = collections[1]["records"].clone();
    collections[1]["dispatch_data"] = collections[0]["dispatch_data"].clone();
    collections[1]["dispatch_data"]["dispatch_info"]["dispatch_id"] = second_dispatch_id;
    collections[1]["records"] = second_records;
    if reverse_collections {
        collections.reverse();
    }
    serde_json::to_vec(&source).unwrap()
}

fn counter_dispatch_id_source(collection_ids: [u64; 2], kernel_ids: [u64; 2]) -> Vec<u8> {
    let mut source: JsonValue =
        serde_json::from_slice(&combined_counter_source(140, 260, 9.0)).unwrap();
    let dispatches = source["rocprofiler-sdk-tool"][0]["buffer_records"]["kernel_dispatch"]
        .as_array_mut()
        .unwrap();
    for (dispatch, id) in dispatches.iter_mut().zip(kernel_ids) {
        dispatch["dispatch_info"]["dispatch_id"] = id.into();
    }
    let collections = source["rocprofiler-sdk-tool"][0]["callback_records"]["counter_collection"]
        .as_array_mut()
        .unwrap();
    for (collection, id) in collections.iter_mut().zip(collection_ids) {
        collection["dispatch_data"]["dispatch_info"]["dispatch_id"] = id.into();
    }
    serde_json::to_vec(&source).unwrap()
}

fn bundle(source: &[u8], artifact: &[u8], kernel_ir: u8, absolute_node: u64) -> Vec<u8> {
    let (binding, _) = binding(artifact, kernel_ir, absolute_node);
    bundle_with_binding(source, binding)
}

fn bundle_with_environment(
    source: &[u8],
    artifact: &[u8],
    kernel_ir: u8,
    environment: ProfilerEnvironmentBindingV4,
) -> Vec<u8> {
    let (mut binding, _) = binding(artifact, kernel_ir, TEST_ABSOLUTE_KFD_NODE);
    binding.environment = environment;
    bundle_with_binding(source, binding)
}

fn bundle_with_binding(source: &[u8], binding: ProfilerDispatchBindingV4) -> Vec<u8> {
    encode_profiler_bundle_v4(&import_rocprofv3_json_profiler_bundle_v4(source, binding).unwrap())
        .unwrap()
}

fn counters(source: &[u8], artifact: &[u8], kernel_ir: u8) -> Vec<u8> {
    let (_, binding) = binding(artifact, kernel_ir, TEST_ABSOLUTE_KFD_NODE);
    encode_counter_capture_v2(
        &import_rocprofv3_counter_capture_v2(source, binding, ImportLimitsV1::default()).unwrap(),
    )
    .unwrap()
}

struct Treatment {
    manifest: Vec<u8>,
    workload: Vec<u8>,
    raw_source: Vec<u8>,
    bundle: Vec<u8>,
    schedule: Vec<u8>,
    artifact: Vec<u8>,
    isa: Vec<u8>,
    counters: Option<Vec<u8>>,
    pc: Option<Vec<u8>>,
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

fn treatment_json(treatment: &Treatment) -> JsonValue {
    serde_json::json!({
        "manifest_hex": lower_hex(&treatment.manifest),
        "semantic_workload_hex": lower_hex(&treatment.workload),
        "raw_profiler_source_hex": lower_hex(&treatment.raw_source),
        "bundle_hex": lower_hex(&treatment.bundle),
        "schedule_hex": lower_hex(&treatment.schedule),
        "artifact_hex": lower_hex(&treatment.artifact),
        "isa_projection_hex": lower_hex(&treatment.isa),
        "counters_hex": treatment.counters.as_deref().map(lower_hex),
        "pc_samples_hex": treatment.pc.as_deref().map(lower_hex),
    })
}

fn run_variant_service(requests: &[JsonValue]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-service"))
        .arg("variant-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for request in requests {
        serde_json::to_writer(&mut input, request).unwrap();
        input.write_all(b"\n").unwrap();
    }
    drop(input);
    child.wait_with_output().unwrap()
}

fn output_json_lines(output: &[u8]) -> Vec<JsonValue> {
    output
        .split_inclusive(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect()
}

impl Treatment {
    fn input(&self) -> ProfilerVariantTreatmentInputV1<'_> {
        ProfilerVariantTreatmentInputV1 {
            manifest: &self.manifest,
            semantic_workload: &self.workload,
            raw_profiler_source: &self.raw_source,
            bundle: &self.bundle,
            schedule: &self.schedule,
            artifact: &self.artifact,
            isa_projection: Some(&self.isa),
            counters: self.counters.as_deref(),
            pc_samples: self.pc.as_deref(),
        }
    }
}

fn treatment(
    workload: &[u8],
    source: &[u8],
    artifact: Vec<u8>,
    kernel_ir: u8,
    schedule: &[u8],
    isa: &[u8],
    counter_source: Option<&[u8]>,
) -> Treatment {
    let bundle = bundle(source, &artifact, kernel_ir, 17);
    let counters = counter_source.map(|source| counters(source, &artifact, kernel_ir));
    assembled_treatment(
        workload,
        source.to_vec(),
        bundle,
        artifact,
        schedule,
        isa,
        (counters, None),
    )
}

fn assembled_treatment(
    workload: &[u8],
    raw_source: Vec<u8>,
    bundle: Vec<u8>,
    artifact: Vec<u8>,
    schedule: &[u8],
    isa: &[u8],
    side_evidence: (Option<Vec<u8>>, Option<Vec<u8>>),
) -> Treatment {
    let (counters, pc) = side_evidence;
    let manifest = build_profiler_variant_manifest_v1(ProfilerVariantManifestInputV1 {
        semantic_workload: workload,
        raw_profiler_source: &raw_source,
        bundle: &bundle,
        schedule,
        artifact: &artifact,
        kernel_ordinal: 0,
        isa_projection: Some(isa),
        counters: counters.as_deref(),
        pc_samples: pc.as_deref(),
    })
    .unwrap();
    Treatment {
        manifest,
        workload: workload.to_vec(),
        raw_source,
        bundle,
        schedule: schedule.to_vec(),
        artifact,
        isa: isa.to_vec(),
        counters,
        pc,
    }
}

fn assert_incomparable_without_deltas(
    baseline: &Treatment,
    candidate: &Treatment,
    expected_axis: ProfilerVariantCompatibilityAxisV1,
) {
    let request = build_profiler_variant_request_v1(
        &baseline.workload,
        &baseline.manifest,
        &candidate.manifest,
    )
    .unwrap();
    let comparison =
        compare_profiler_variants_v1(request, baseline.input(), candidate.input()).unwrap();
    assert!(!comparison.comparable);
    assert!(comparison.resource_deltas.is_empty());
    assert!(comparison.duration_deltas.is_empty());
    assert!(comparison.counter_deltas.is_empty());
    assert!(comparison.ranked_explanations.is_empty());
    assert!(comparison.compatibility.iter().any(|fact| {
        fact.axis == expected_axis && fact.status == ProfilerVariantCompatibilityStatusV1::Mismatch
    }));
}

#[test]
fn variant_path_allows_compiled_evidence_to_differ_and_cites_co_observation() {
    let workload = br#"{"kernel":"generic","shape":[256,2,1]}"#;
    let baseline_source = combined_counter_source(140, 260, 9.0);
    let candidate_source = combined_counter_source(170, 310, 11.0);
    let baseline = treatment(
        workload,
        &baseline_source,
        hsaco(7, 0),
        1,
        b"schedule-v1",
        b"isa-v1",
        Some(&baseline_source),
    );
    let candidate = treatment(
        workload,
        &candidate_source,
        hsaco(11, 2),
        2,
        b"schedule-v2",
        b"isa-v2",
        Some(&candidate_source),
    );
    let request =
        build_profiler_variant_request_v1(workload, &baseline.manifest, &candidate.manifest)
            .unwrap();
    let comparison =
        compare_profiler_variants_v1(request, baseline.input(), candidate.input()).unwrap();

    assert!(comparison.comparable);
    assert!(comparison.compatibility.iter().all(|fact| {
        fact.status == ProfilerVariantCompatibilityStatusV1::Exact && !fact.evidence.is_empty()
    }));
    assert_ne!(
        comparison.baseline_treatment.kernel_ir,
        comparison.candidate_treatment.kernel_ir
    );
    assert_ne!(
        comparison.baseline_treatment.artifact,
        comparison.candidate_treatment.artifact
    );
    assert_ne!(
        comparison.baseline_treatment.isa_projection,
        comparison.candidate_treatment.isa_projection
    );
    assert_eq!(comparison.baseline_resources.vgpr_count, 7);
    assert_eq!(comparison.candidate_resources.vgpr_count, 11);
    assert_eq!(comparison.candidate_resources.sgpr_spill_count, Some(2));
    assert!(
        comparison.duration_deltas.iter().all(|delta| {
            delta.signed_delta_ticks > 0 && delta.origin == TruthOriginV1::Inferred
        })
    );
    assert_eq!(comparison.counter_deltas.len(), 4);
    assert_eq!(comparison.ranked_explanations.len(), 1);
    assert_eq!(
        comparison.ranked_explanations[0].origin,
        TruthOriginV1::Inferred
    );
    assert!(comparison.ranked_explanations[0].evidence.len() >= 9);
    assert!(comparison.unavailable.iter().any(|fact| {
        fact.kind == ProfilerVariantUnavailableKindV1::CausalRegressionAttribution
            && fact.origin == TruthOriginV1::Unavailable
    }));
    assert!(comparison.unavailable.iter().any(|fact| {
        fact.kind == ProfilerVariantUnavailableKindV1::DecodedAttEvents
            && fact.origin == TruthOriginV1::Unavailable
    }));
    assert!(comparison.unavailable.iter().any(|fact| {
        fact.kind == ProfilerVariantUnavailableKindV1::CompleteWorkloadAndArguments
            && fact.origin == TruthOriginV1::Unavailable
    }));
    assert!(comparison.unavailable.iter().any(|fact| {
        fact.kind == ProfilerVariantUnavailableKindV1::ClockDomainAndNormalization
            && fact.origin == TruthOriginV1::Unavailable
    }));

    let encoded = encode_profiler_variant_comparison_v1(
        request,
        baseline.input(),
        candidate.input(),
        &comparison,
    )
    .unwrap();
    assert!(encoded.len() as u64 <= MAX_PROFILER_VARIANT_RESULT_BYTES_V1);
    assert_eq!(
        decode_profiler_variant_comparison_v1(
            &encoded,
            request,
            baseline.input(),
            candidate.input(),
        )
        .unwrap(),
        comparison
    );
    let mut noncanonical = encoded.clone();
    noncanonical.push(b'\n');
    assert_eq!(
        decode_profiler_variant_comparison_v1(
            &noncanonical,
            request,
            baseline.input(),
            candidate.input(),
        )
        .unwrap_err(),
        ProfilerVariantErrorV1::InvalidResult
    );

    let old_comparator = compare_profiler_bundles_v4(&baseline.bundle, &candidate.bundle).unwrap();
    assert!(!old_comparator.comparable);
    assert!(old_comparator.deltas.is_empty());
}

#[test]
fn raw_dispatch_ids_bind_reordered_collections_with_duplicate_envelopes() {
    let workload = b"dispatch-id-workload";
    let artifact = hsaco(7, 0);
    let baseline_source = duplicate_envelope_counter_source(false);
    let candidate_source = duplicate_envelope_counter_source(true);
    let baseline = treatment(
        workload,
        &baseline_source,
        artifact.clone(),
        1,
        b"schedule",
        b"isa",
        Some(&baseline_source),
    );
    let candidate = treatment(
        workload,
        &candidate_source,
        artifact,
        1,
        b"schedule",
        b"isa",
        Some(&candidate_source),
    );
    let request =
        build_profiler_variant_request_v1(workload, &baseline.manifest, &candidate.manifest)
            .unwrap();
    let comparison =
        compare_profiler_variants_v1(request, baseline.input(), candidate.input()).unwrap();
    assert!(comparison.comparable);
    assert_eq!(comparison.counter_deltas.len(), 4);
    assert!(
        comparison.counter_deltas[..3]
            .iter()
            .all(|delta| delta.dispatch_ordinal == 0 && f64::from_bits(delta.delta_f64_bits) == 0.0)
    );
    assert_eq!(comparison.counter_deltas[3].dispatch_ordinal, 1);
    assert_eq!(
        f64::from_bits(comparison.counter_deltas[3].delta_f64_bits),
        0.0
    );
}

#[test]
fn wrong_unknown_and_duplicate_raw_dispatch_ids_are_typed_unavailable() {
    let workload = b"hostile-dispatch-id-workload";
    let artifact = hsaco(7, 0);
    for source in [
        counter_dispatch_id_source([2, 2], [1, 2]),
        counter_dispatch_id_source([99, 2], [1, 2]),
        counter_dispatch_id_source([1, 2], [1, 1]),
    ] {
        let treatment = treatment(
            workload,
            &source,
            artifact.clone(),
            1,
            b"schedule",
            b"isa",
            Some(&source),
        );
        let request =
            build_profiler_variant_request_v1(workload, &treatment.manifest, &treatment.manifest)
                .unwrap();
        let comparison =
            compare_profiler_variants_v1(request, treatment.input(), treatment.input()).unwrap();
        assert!(comparison.counter_deltas.is_empty());
        assert!(comparison.unavailable.iter().any(|fact| {
            fact.kind == ProfilerVariantUnavailableKindV1::CounterComparison
                && fact.reason.contains("dispatch-id relation")
        }));
    }
}

#[test]
fn resealed_normalized_or_raw_source_substitution_fails_admission() {
    let workload = b"raw-source-substitution-workload";
    let source = dispatch_source(140, 260);
    let mut treatment = treatment(workload, &source, hsaco(7, 0), 1, b"schedule", b"isa", None);

    let mut changed_bundle = decode_profiler_bundle_v4(&treatment.bundle).unwrap();
    let dispatch = &mut changed_bundle.dispatch_capture.as_mut().unwrap().dispatches[0];
    dispatch.end_timestamp += 1;
    dispatch.duration_ticks += 1;
    assert!(matches!(
        validate_rocprofv3_bundle_raw_source_relation_v1(
            &source,
            &changed_bundle,
            ImportLimitsV1::default(),
        ),
        Err(RocprofRawSourceRelationErrorV1::RelationMismatch)
    ));
    assert!(matches!(
        validate_rocprofv3_bundle_raw_source_relation_v1(
            b"{",
            &changed_bundle,
            ImportLimitsV1::default(),
        ),
        Err(RocprofRawSourceRelationErrorV1::ProfilerBundle(
            ProfilerBundleErrorV4::InvalidRocprofJson
        ))
    ));
    treatment.bundle = encode_profiler_bundle_v4(&changed_bundle).unwrap();
    let mut manifest: ProfilerVariantTreatmentManifestV1 =
        serde_json::from_slice(&treatment.manifest).unwrap();
    manifest.bundle = profiler_bundle_content_identity_v4(&treatment.bundle).unwrap();
    treatment.manifest = serde_json::to_vec(&manifest).unwrap();
    let request =
        build_profiler_variant_request_v1(workload, &treatment.manifest, &treatment.manifest)
            .unwrap();
    assert_eq!(
        compare_profiler_variants_v1(request, treatment.input(), treatment.input()).unwrap_err(),
        ProfilerVariantErrorV1::RawSourceAdmission
    );

    let mut raw_substitution = treatment;
    raw_substitution.bundle = bundle(&source, &raw_substitution.artifact, 1, 17);
    raw_substitution.raw_source.push(b' ');
    let mut manifest: ProfilerVariantTreatmentManifestV1 =
        serde_json::from_slice(&raw_substitution.manifest).unwrap();
    manifest.bundle = profiler_bundle_content_identity_v4(&raw_substitution.bundle).unwrap();
    manifest.raw_profiler_source = rocprofv3_json_source_content_identity_v1(
        &raw_substitution.raw_source,
        ImportLimitsV1::default(),
    )
    .unwrap();
    raw_substitution.manifest = serde_json::to_vec(&manifest).unwrap();
    let request = build_profiler_variant_request_v1(
        workload,
        &raw_substitution.manifest,
        &raw_substitution.manifest,
    )
    .unwrap();
    assert_eq!(
        compare_profiler_variants_v1(request, raw_substitution.input(), raw_substitution.input(),)
            .unwrap_err(),
        ProfilerVariantErrorV1::RawSourceAdmission
    );
}

#[test]
fn raw_relation_token_cannot_be_reused_for_different_bundle_claims() {
    let source = combined_counter_source(140, 260, 9.0);
    let artifact = hsaco(7, 0);
    let baseline_bundle = decode_profiler_bundle_v4(&bundle(&source, &artifact, 1, 17)).unwrap();
    let changed_bundle = decode_profiler_bundle_v4(&bundle_with_environment(
        &source,
        &artifact,
        1,
        environment_with_claims(
            content(10),
            content(11),
            content(12),
            vec![(17, content(99))],
        ),
    ))
    .unwrap();
    let relation = validate_rocprofv3_bundle_raw_source_relation_v1(
        &source,
        &baseline_bundle,
        ImportLimitsV1::default(),
    )
    .unwrap();
    let counter_capture = decode_counter_capture_v2(&counters(&source, &artifact, 1)).unwrap();

    assert!(matches!(
        validate_rocprofv3_counter_bundle_relation_v1(
            &source,
            &changed_bundle,
            relation,
            &counter_capture,
            ImportLimitsV1::default(),
        ),
        Err(RocprofRawSourceRelationErrorV1::RelationMismatch)
    ));
}

#[test]
fn stale_substitution_reseal_and_hostile_result_are_rejected() {
    let workload = b"same-semantic-workload";
    let source = dispatch_source(140, 260);
    let baseline = treatment(
        workload,
        &source,
        hsaco(7, 0),
        1,
        b"schedule-a",
        b"isa-a",
        None,
    );
    let mut candidate = treatment(
        workload,
        &source,
        hsaco(8, 1),
        2,
        b"schedule-b",
        b"isa-b",
        None,
    );
    let request =
        build_profiler_variant_request_v1(workload, &baseline.manifest, &candidate.manifest)
            .unwrap();

    candidate.schedule[0] ^= 1;
    assert_eq!(
        compare_profiler_variants_v1(request, baseline.input(), candidate.input()).unwrap_err(),
        ProfilerVariantErrorV1::StaleIdentity
    );

    candidate.schedule[0] ^= 1;
    candidate.schedule.extend_from_slice(b"-resealed");
    candidate.manifest = build_profiler_variant_manifest_v1(ProfilerVariantManifestInputV1 {
        semantic_workload: &candidate.workload,
        raw_profiler_source: &candidate.raw_source,
        bundle: &candidate.bundle,
        schedule: &candidate.schedule,
        artifact: &candidate.artifact,
        kernel_ordinal: 0,
        isa_projection: Some(&candidate.isa),
        counters: None,
        pc_samples: None,
    })
    .unwrap();
    assert_eq!(
        compare_profiler_variants_v1(request, baseline.input(), candidate.input()).unwrap_err(),
        ProfilerVariantErrorV1::RequestMismatch
    );

    let request =
        build_profiler_variant_request_v1(workload, &baseline.manifest, &candidate.manifest)
            .unwrap();
    let accepted =
        compare_profiler_variants_v1(request, baseline.input(), candidate.input()).unwrap();
    let accepted_bytes = encode_profiler_variant_comparison_v1(
        request,
        baseline.input(),
        candidate.input(),
        &accepted,
    )
    .unwrap();
    let stale_candidate = treatment(
        workload,
        &source,
        hsaco(9, 2),
        3,
        b"schedule-c",
        b"isa-c",
        None,
    );
    let stale_request =
        build_profiler_variant_request_v1(workload, &baseline.manifest, &stale_candidate.manifest)
            .unwrap();
    let stale =
        compare_profiler_variants_v1(stale_request, baseline.input(), stale_candidate.input())
            .unwrap();
    let stale_bytes = encode_profiler_variant_comparison_v1(
        stale_request,
        baseline.input(),
        stale_candidate.input(),
        &stale,
    )
    .unwrap();
    assert_eq!(
        decode_profiler_variant_comparison_v1(
            &accepted_bytes,
            request,
            baseline.input(),
            candidate.input(),
        )
        .unwrap(),
        accepted
    );
    assert_eq!(
        decode_profiler_variant_comparison_v1(
            &stale_bytes,
            request,
            baseline.input(),
            candidate.input(),
        )
        .unwrap_err(),
        ProfilerVariantErrorV1::InvalidResult
    );
    let mut result =
        compare_profiler_variants_v1(request, baseline.input(), candidate.input()).unwrap();
    result.comparable = false;
    assert_eq!(
        encode_profiler_variant_comparison_v1(
            request,
            baseline.input(),
            candidate.input(),
            &result,
        )
        .unwrap_err(),
        ProfilerVariantErrorV1::InvalidResult
    );
}

#[test]
fn resealed_axis_substitutions_reach_compatibility_and_suppress_every_delta() {
    let workload = b"axis-bound-workload";
    let source = dispatch_source(140, 260);
    let baseline = treatment(
        workload,
        &source,
        hsaco(7, 0),
        1,
        b"schedule-a",
        b"isa-a",
        None,
    );

    let claim_cases = [
        (
            ProfilerVariantCompatibilityAxisV1::Environment,
            environment_with_claims(
                content(20),
                content(11),
                content(12),
                vec![(17, content(13))],
            ),
        ),
        (
            ProfilerVariantCompatibilityAxisV1::CollectorTool,
            environment_with_claims(
                content(10),
                content(21),
                content(12),
                vec![(17, content(13))],
            ),
        ),
        (
            ProfilerVariantCompatibilityAxisV1::CollectorConfiguration,
            environment_with_claims(
                content(10),
                content(11),
                content(22),
                vec![(17, content(13))],
            ),
        ),
    ];
    for (axis, environment) in claim_cases {
        let artifact = hsaco(11, 2);
        let candidate_bundle = bundle_with_environment(&source, &artifact, 2, environment);
        let candidate = assembled_treatment(
            workload,
            source.clone(),
            candidate_bundle,
            artifact,
            b"schedule-b",
            b"isa-b",
            (None, None),
        );
        assert_incomparable_without_deltas(&baseline, &candidate, axis);
    }

    let artifact = hsaco(11, 2);
    let selector_source = selector_mismatch_source();
    let selector_bundle = bundle(&selector_source, &artifact, 2, 17);
    let selector = assembled_treatment(
        workload,
        selector_source,
        selector_bundle,
        artifact,
        b"schedule-b",
        b"isa-b",
        (None, None),
    );
    assert_incomparable_without_deltas(
        &baseline,
        &selector,
        ProfilerVariantCompatibilityAxisV1::DispatchWorkloadAndLaunch,
    );

    let devices = vec![(17, content(13)), (19, content(14))];
    let baseline_artifact = hsaco(7, 0);
    let baseline_device_source = dispatch_source_agents(17, 19);
    let baseline_devices = assembled_treatment(
        workload,
        baseline_device_source.clone(),
        bundle_with_environment(
            &baseline_device_source,
            &baseline_artifact,
            1,
            environment_with_claims(content(10), content(11), content(12), devices.clone()),
        ),
        baseline_artifact,
        b"schedule-a",
        b"isa-a",
        (None, None),
    );
    let candidate_artifact = hsaco(11, 2);
    let candidate_device_source = dispatch_source_agents(19, 17);
    let candidate_devices = assembled_treatment(
        workload,
        candidate_device_source.clone(),
        bundle_with_environment(
            &candidate_device_source,
            &candidate_artifact,
            2,
            environment_with_claims(content(10), content(11), content(12), devices),
        ),
        candidate_artifact,
        b"schedule-b",
        b"isa-b",
        (None, None),
    );
    assert_incomparable_without_deltas(
        &baseline_devices,
        &candidate_devices,
        ProfilerVariantCompatibilityAxisV1::StableDevices,
    );
}

#[test]
fn unused_device_catalog_entries_do_not_control_dispatch_comparability() {
    let workload = b"unused-device-workload";
    let source = dispatch_source(140, 260);
    let artifact = hsaco(7, 0);
    let baseline = treatment(
        workload,
        &source,
        artifact.clone(),
        1,
        b"schedule",
        b"isa",
        None,
    );
    let mut expanded = decode_profiler_bundle_v4(&baseline.bundle).unwrap();
    let source_digest = expanded.dispatch_capture.as_ref().unwrap().runs[0]
        .source
        .digest;
    let extra_device = derived_capture_device(source_digest, 1);
    expanded
        .dispatch_capture
        .as_mut()
        .unwrap()
        .devices
        .push(CaptureDeviceV1 {
            identity: extra_device,
            identity_origin: TruthOriginV1::Observed,
            source_device_ordinal: 1,
        });
    expanded.devices.push(ProfilerDeviceV4 {
        ordinal: 1,
        stable_identity: ProfilerIdentityFactV4::declared(
            ProfilerIdentityRoleV4::StableDevice,
            content(99),
        ),
        source_bound_identity: Some(extra_device),
        source_bound_origin: TruthOriginV1::Observed,
    });
    let candidate = assembled_treatment(
        workload,
        source,
        encode_profiler_bundle_v4(&expanded).unwrap(),
        artifact,
        b"schedule",
        b"isa",
        (None, None),
    );
    let request =
        build_profiler_variant_request_v1(workload, &baseline.manifest, &candidate.manifest)
            .unwrap();
    let comparison =
        compare_profiler_variants_v1(request, baseline.input(), candidate.input()).unwrap();
    assert!(comparison.comparable);
    assert_eq!(comparison.duration_deltas.len(), 2);
    assert_eq!(comparison.resource_deltas.len(), 11);
}

#[test]
fn missing_stable_device_identity_is_rejected_before_compatibility_classification() {
    let source = dispatch_source(140, 260);
    let artifact = hsaco(7, 0);
    let valid = bundle(&source, &artifact, 1, 17);
    let mut hostile = decode_profiler_bundle_v4(&valid).unwrap();
    hostile.devices[0].stable_identity = ProfilerIdentityFactV4::unavailable(
        ProfilerIdentityRoleV4::StableDevice,
        CaptureUnavailableReasonV1::NotProvided,
    );
    let hostile = serde_json::to_vec(&hostile).unwrap();
    assert_eq!(
        build_profiler_variant_manifest_v1(ProfilerVariantManifestInputV1 {
            semantic_workload: b"missing-stable-device",
            raw_profiler_source: &source,
            bundle: &hostile,
            schedule: b"schedule",
            artifact: &artifact,
            kernel_ordinal: 0,
            isa_projection: None,
            counters: None,
            pc_samples: None,
        })
        .unwrap_err(),
        ProfilerVariantErrorV1::BundleAdmission
    );
}

#[test]
fn pc_evidence_is_rebound_but_cross_artifact_localization_stays_unavailable() {
    let artifact = hsaco(9, 0);
    let (profiler_binding, capture_binding) = binding(&artifact, 4, 18_217);
    let source = pc_source(18_217);
    let bundle = encode_profiler_bundle_v4(
        &import_rocprofv3_json_profiler_bundle_v4(&source, profiler_binding.clone()).unwrap(),
    )
    .unwrap();
    let pc = encode_pc_sample_capture_v3(
        &import_rocprofv3_pc_sample_capture_v3(
            &source,
            RocprofPcSampleCaptureBindingV3 {
                capture: capture_binding,
                sampling_interval_cycles: 1_048_576,
            },
            ImportLimitsV1::default(),
        )
        .unwrap(),
    )
    .unwrap();
    let manifest = build_profiler_variant_manifest_v1(ProfilerVariantManifestInputV1 {
        semantic_workload: b"pc-workload",
        raw_profiler_source: &source,
        bundle: &bundle,
        schedule: b"schedule",
        artifact: &artifact,
        kernel_ordinal: 0,
        isa_projection: None,
        counters: None,
        pc_samples: Some(&pc),
    })
    .unwrap();
    let treatment = ProfilerVariantTreatmentInputV1 {
        manifest: &manifest,
        semantic_workload: b"pc-workload",
        raw_profiler_source: &source,
        bundle: &bundle,
        schedule: b"schedule",
        artifact: &artifact,
        isa_projection: None,
        counters: None,
        pc_samples: Some(&pc),
    };
    let request = build_profiler_variant_request_v1(b"pc-workload", &manifest, &manifest).unwrap();
    let comparison = compare_profiler_variants_v1(request, treatment, treatment).unwrap();
    let unavailable = comparison
        .unavailable
        .iter()
        .find(|fact| fact.kind == ProfilerVariantUnavailableKindV1::PcToSemanticOrIsaCorrelation)
        .unwrap();
    assert_eq!(unavailable.origin, TruthOriginV1::Unavailable);
    assert_eq!(unavailable.evidence.len(), 2);

    let mut changed_source: JsonValue = serde_json::from_slice(&source).unwrap();
    changed_source["rocprofiler-sdk-tool"][0]["buffer_records"]["kernel_dispatch"][0]["dispatch_info"]
        ["workgroup_size"]["x"] = 128.into();
    let changed_source = serde_json::to_vec(&changed_source).unwrap();
    let changed_bundle = encode_profiler_bundle_v4(
        &import_rocprofv3_json_profiler_bundle_v4(&changed_source, profiler_binding).unwrap(),
    )
    .unwrap();
    let changed_manifest = build_profiler_variant_manifest_v1(ProfilerVariantManifestInputV1 {
        semantic_workload: b"pc-workload",
        raw_profiler_source: &changed_source,
        bundle: &changed_bundle,
        schedule: b"schedule",
        artifact: &artifact,
        kernel_ordinal: 0,
        isa_projection: None,
        counters: None,
        pc_samples: Some(&pc),
    })
    .unwrap();
    let hostile = ProfilerVariantTreatmentInputV1 {
        manifest: &changed_manifest,
        raw_profiler_source: &changed_source,
        bundle: &changed_bundle,
        ..treatment
    };
    let hostile_request =
        build_profiler_variant_request_v1(b"pc-workload", &manifest, &changed_manifest).unwrap();
    let hostile = compare_profiler_variants_v1(hostile_request, treatment, hostile).unwrap();
    assert!(hostile.unavailable.iter().any(|fact| {
        fact.kind == ProfilerVariantUnavailableKindV1::PcCaptureBinding
            && fact.origin == TruthOriginV1::Unavailable
    }));
}

#[test]
fn same_device_ordinal_with_changed_agent_cannot_bind_counter_or_pc_evidence() {
    let artifact = hsaco(9, 0);
    let counter_source = combined_counter_source(140, 260, 9.0);
    let counter_capture = counters(&counter_source, &artifact, 4);
    let changed_counter_source =
        combined_counter_source_for_agent(140, 260, 9.0, TEST_SECOND_OPAQUE_AGENT_HANDLE, 19);
    let counter_bundle = bundle(&changed_counter_source, &artifact, 4, 19);
    let counter_manifest = build_profiler_variant_manifest_v1(ProfilerVariantManifestInputV1 {
        semantic_workload: b"changed-counter-agent",
        raw_profiler_source: &changed_counter_source,
        bundle: &counter_bundle,
        schedule: b"schedule",
        artifact: &artifact,
        kernel_ordinal: 0,
        isa_projection: None,
        counters: Some(&counter_capture),
        pc_samples: None,
    })
    .unwrap();
    let counter_input = ProfilerVariantTreatmentInputV1 {
        manifest: &counter_manifest,
        semantic_workload: b"changed-counter-agent",
        raw_profiler_source: &changed_counter_source,
        bundle: &counter_bundle,
        schedule: b"schedule",
        artifact: &artifact,
        isa_projection: None,
        counters: Some(&counter_capture),
        pc_samples: None,
    };
    let counter_request = build_profiler_variant_request_v1(
        b"changed-counter-agent",
        &counter_manifest,
        &counter_manifest,
    )
    .unwrap();
    let counter_comparison =
        compare_profiler_variants_v1(counter_request, counter_input, counter_input).unwrap();
    assert!(counter_comparison.counter_deltas.is_empty());
    assert!(counter_comparison.unavailable.iter().any(|fact| {
        fact.kind == ProfilerVariantUnavailableKindV1::CounterComparison
            && fact.reason.contains("dispatch-id relation")
    }));

    let (_, capture_binding) = binding(&artifact, 4, 18_217);
    let original_pc_source = pc_source(18_217);
    let pc_capture = encode_pc_sample_capture_v3(
        &import_rocprofv3_pc_sample_capture_v3(
            &original_pc_source,
            RocprofPcSampleCaptureBindingV3 {
                capture: capture_binding,
                sampling_interval_cycles: 1_048_576,
            },
            ImportLimitsV1::default(),
        )
        .unwrap(),
    )
    .unwrap();
    let changed_pc_source = pc_source(18_219);
    let pc_bundle = bundle(&changed_pc_source, &artifact, 4, 18_219);
    let pc_manifest = build_profiler_variant_manifest_v1(ProfilerVariantManifestInputV1 {
        semantic_workload: b"changed-pc-agent",
        raw_profiler_source: &changed_pc_source,
        bundle: &pc_bundle,
        schedule: b"schedule",
        artifact: &artifact,
        kernel_ordinal: 0,
        isa_projection: None,
        counters: None,
        pc_samples: Some(&pc_capture),
    })
    .unwrap();
    let pc_input = ProfilerVariantTreatmentInputV1 {
        manifest: &pc_manifest,
        semantic_workload: b"changed-pc-agent",
        raw_profiler_source: &changed_pc_source,
        bundle: &pc_bundle,
        schedule: b"schedule",
        artifact: &artifact,
        isa_projection: None,
        counters: None,
        pc_samples: Some(&pc_capture),
    };
    let pc_request =
        build_profiler_variant_request_v1(b"changed-pc-agent", &pc_manifest, &pc_manifest).unwrap();
    let pc_comparison = compare_profiler_variants_v1(pc_request, pc_input, pc_input).unwrap();
    assert!(pc_comparison.unavailable.iter().any(|fact| {
        fact.kind == ProfilerVariantUnavailableKindV1::PcCaptureBinding
            && fact.reason.contains("dispatch-id relation")
    }));
}

#[test]
fn opaque_and_manifest_bounds_fail_closed_before_allocation_heavy_admission() {
    let oversized = vec![0_u8; MAX_PROFILER_VARIANT_OPAQUE_EVIDENCE_BYTES_V1 as usize + 1];
    assert_eq!(
        build_profiler_variant_request_v1(&oversized, b"{}", b"{}").unwrap_err(),
        ProfilerVariantErrorV1::EvidenceTooLarge
    );
    assert_eq!(
        build_profiler_variant_request_v1(
            b"workload",
            &vec![b' '; MAX_PROFILER_VARIANT_MANIFEST_BYTES_V1 as usize + 1],
            b"{}",
        )
        .unwrap_err(),
        ProfilerVariantErrorV1::EvidenceTooLarge
    );
}

#[test]
fn counter_overflow_cardinality_and_result_bounds_fail_closed() {
    let workload = b"counter-edge-workload";
    let artifact = hsaco(7, 0);
    let negative = combined_counter_source_with_count(2, -f64::MAX);
    let positive = combined_counter_source_with_count(2, f64::MAX);
    let baseline = treatment(
        workload,
        &negative,
        artifact.clone(),
        1,
        b"schedule",
        b"isa",
        Some(&negative),
    );
    let candidate = treatment(
        workload,
        &positive,
        artifact.clone(),
        1,
        b"schedule",
        b"isa",
        Some(&positive),
    );
    let request =
        build_profiler_variant_request_v1(workload, &baseline.manifest, &candidate.manifest)
            .unwrap();
    let comparison =
        compare_profiler_variants_v1(request, baseline.input(), candidate.input()).unwrap();
    assert!(comparison.comparable);
    assert!(comparison.counter_deltas.is_empty());
    assert!(comparison.unavailable.iter().any(|fact| {
        fact.kind == ProfilerVariantUnavailableKindV1::CounterComparison
            && fact.reason.contains("overflowed")
    }));

    let maximum_name = "x".repeat(MAX_COUNTER_NAME_BYTES_V2);
    let max_left_source = with_counter_name(
        combined_counter_source_with_count(MAX_PROFILER_VARIANT_COUNTER_VALUES_V1, 1.0),
        &maximum_name,
    );
    let max_right_source = with_counter_name(
        combined_counter_source_with_count(MAX_PROFILER_VARIANT_COUNTER_VALUES_V1, 2.0),
        &maximum_name,
    );
    let max_left = treatment(
        workload,
        &max_left_source,
        artifact.clone(),
        1,
        b"schedule",
        b"isa",
        Some(&max_left_source),
    );
    let max_right = treatment(
        workload,
        &max_right_source,
        artifact.clone(),
        1,
        b"schedule",
        b"isa",
        Some(&max_right_source),
    );
    let max_request =
        build_profiler_variant_request_v1(workload, &max_left.manifest, &max_right.manifest)
            .unwrap();
    let max_comparison =
        compare_profiler_variants_v1(max_request, max_left.input(), max_right.input()).unwrap();
    assert_eq!(
        max_comparison.counter_deltas.len(),
        MAX_PROFILER_VARIANT_COUNTER_VALUES_V1
    );
    let encoded = encode_profiler_variant_comparison_v1(
        max_request,
        max_left.input(),
        max_right.input(),
        &max_comparison,
    )
    .unwrap();
    assert!(encoded.len() as u64 <= MAX_PROFILER_VARIANT_RESULT_BYTES_V1);

    let excessive_source =
        combined_counter_source_with_count(MAX_PROFILER_VARIANT_COUNTER_VALUES_V1 + 1, 1.0);
    let excessive = treatment(
        workload,
        &excessive_source,
        artifact,
        1,
        b"schedule",
        b"isa",
        Some(&excessive_source),
    );
    let excessive_request =
        build_profiler_variant_request_v1(workload, &excessive.manifest, &excessive.manifest)
            .unwrap();
    assert_eq!(
        compare_profiler_variants_v1(excessive_request, excessive.input(), excessive.input())
            .unwrap_err(),
        ProfilerVariantErrorV1::TooManyCounterValues
    );
}

#[test]
fn multi_kernel_hsaco_is_not_bound_by_an_unauthenticated_ordinal() {
    let artifact = hsaco_with_kernels(vec![kernel("first", 7, 0), kernel("second", 11, 2)]);
    let source = dispatch_source(140, 260);
    let bundle = bundle(&source, &artifact, 1, 17);
    let manifest = build_profiler_variant_manifest_v1(ProfilerVariantManifestInputV1 {
        semantic_workload: b"multi-kernel-workload",
        raw_profiler_source: &source,
        bundle: &bundle,
        schedule: b"schedule",
        artifact: &artifact,
        kernel_ordinal: 0,
        isa_projection: None,
        counters: None,
        pc_samples: None,
    })
    .unwrap();
    let input = ProfilerVariantTreatmentInputV1 {
        manifest: &manifest,
        semantic_workload: b"multi-kernel-workload",
        raw_profiler_source: &source,
        bundle: &bundle,
        schedule: b"schedule",
        artifact: &artifact,
        isa_projection: None,
        counters: None,
        pc_samples: None,
    };
    let request =
        build_profiler_variant_request_v1(b"multi-kernel-workload", &manifest, &manifest).unwrap();
    assert_eq!(
        compare_profiler_variants_v1(request, input, input).unwrap_err(),
        ProfilerVariantErrorV1::AmbiguousKernelBinding
    );
}

#[test]
fn additive_variant_jsonl_is_discoverable_bounded_and_deterministic() {
    let workload = br#"{"kernel":"generic","shape":[256,2,1]}"#;
    let baseline_source = combined_counter_source(140, 260, 9.0);
    let candidate_source = combined_counter_source(170, 310, 11.0);
    let baseline = treatment(
        workload,
        &baseline_source,
        hsaco(7, 0),
        1,
        b"schedule-v1",
        b"isa-v1",
        Some(&baseline_source),
    );
    let candidate = treatment(
        workload,
        &candidate_source,
        hsaco(11, 2),
        2,
        b"schedule-v2",
        b"isa-v2",
        Some(&candidate_source),
    );
    let requests = vec![
        serde_json::json!({
            "operation": "discover_capabilities",
            "schema": AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
            "request_id": 1,
            "expected_revision": 0,
        }),
        serde_json::json!({
            "operation": "compare_variants",
            "schema": AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
            "request_id": 2,
            "expected_revision": 1,
            "baseline": treatment_json(&baseline),
            "candidate": treatment_json(&candidate),
        }),
    ];
    let first = run_variant_service(&requests);
    let second = run_variant_service(&requests);
    assert!(first.status.success());
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    let responses = output_json_lines(&first.stdout);
    assert_eq!(responses.len(), 2);
    for line in first
        .stdout
        .split_inclusive(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        validate_agent_profiler_variant_response_line_v1(line).unwrap();
    }
    assert!(responses.iter().all(|response| {
        serde_json::to_vec(response).unwrap().len() as u64
            <= MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V1
    }));
    let capabilities = &responses[0]["value"]["capabilities"];
    assert_eq!(
        capabilities["authority"],
        "read_only_no_execution_attach_scheduling_or_collection_authority"
    );
    assert_eq!(
        capabilities["exact_input_encoding"],
        "canonical_lowercase_hex_of_exact_bytes"
    );
    assert_eq!(capabilities["operations"].as_array().unwrap().len(), 2);
    let comparison = &responses[1]["value"]["comparison"];
    assert_eq!(comparison["comparable"], true);
    assert!(
        !comparison["ranked_explanations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for kind in [
        "decoded_att_events",
        "runtime_api_events",
        "copy_events",
        "pc_to_semantic_or_isa_correlation",
        "semantic_ir_isa_change_localization",
        "causal_regression_attribution",
    ] {
        assert!(
            comparison["unavailable"]
                .as_array()
                .unwrap()
                .iter()
                .any(|fact| fact["kind"] == kind)
        );
    }
    let encoded = String::from_utf8(first.stdout).unwrap();
    for forbidden in ["/dev/kfd", "launch_kernel", "collect_profile"] {
        assert!(!encoded.contains(forbidden));
    }
    let mut substituted = responses[1].clone();
    substituted["value"]["comparison"]["ranking_policy"] = "forged".into();
    let mut substituted = serde_json::to_vec(&substituted).unwrap();
    substituted.push(b'\n');
    assert!(validate_agent_profiler_variant_response_line_v1(&substituted).is_err());
}

#[test]
fn additive_variant_jsonl_rejects_stale_duplicate_and_substituted_evidence() {
    assert!(decode_agent_profiler_variant_request_line_v1(
        br#"{"operation":"compare_variants","schema":"fe2o3-agent-profiler-variant-request-v1","request_id":1,"expected_revision":0,"baseline":{"manifest_path":"mutable.json"},"candidate":{"manifest_path":"mutable.json"}}
"#,
    )
    .is_err());
    let workload = b"service-hostile-workload";
    let source = combined_counter_source(140, 260, 9.0);
    let treatment = treatment(
        workload,
        &source,
        hsaco(7, 0),
        1,
        b"schedule",
        b"isa",
        Some(&source),
    );
    let mut substituted = treatment_json(&treatment);
    substituted["raw_profiler_source_hex"] = lower_hex(b"{}").into();
    let output = run_variant_service(&[
        serde_json::json!({
            "operation": "discover_capabilities",
            "schema": AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
            "request_id": 1,
            "expected_revision": 0,
        }),
        serde_json::json!({
            "operation": "compare_variants",
            "schema": AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
            "request_id": 2,
            "expected_revision": 0,
            "baseline": treatment_json(&treatment),
            "candidate": treatment_json(&treatment),
        }),
        serde_json::json!({
            "operation": "compare_variants",
            "schema": AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
            "request_id": 2,
            "expected_revision": 2,
            "baseline": treatment_json(&treatment),
            "candidate": treatment_json(&treatment),
        }),
        serde_json::json!({
            "operation": "compare_variants",
            "schema": AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
            "request_id": 3,
            "expected_revision": 3,
            "baseline": treatment_json(&treatment),
            "candidate": substituted,
        }),
    ]);
    assert!(output.status.success());
    let responses = output_json_lines(&output.stdout);
    assert_eq!(responses[1]["code"], "stale_revision");
    assert_eq!(responses[2]["code"], "duplicate_request_id");
    assert_eq!(responses[3]["code"], "evidence_admission_failed");
    assert!(
        responses
            .iter()
            .all(|response| !response["response_identity"].is_null())
    );

    let malformed = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-service"))
        .arg("variant-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.take().unwrap().write_all(b"{}\n")?;
            child.wait_with_output()
        })
        .unwrap();
    assert_eq!(malformed.status.code(), Some(1));
    let response = &output_json_lines(&malformed.stdout)[0];
    assert_eq!(response["code"], "invalid_request");
    assert_eq!(response["terminal"], true);
}

fn hsaco(vgpr_count: u64, spill_count: u64) -> Vec<u8> {
    hsaco_with_kernels(vec![kernel("generic", vgpr_count, spill_count)])
}

fn kernel(name: &str, vgpr_count: u64, spill_count: u64) -> Value {
    map(vec![
        (".name", Value::from(name)),
        (".symbol", Value::from(format!("{name}.kd"))),
        (".kernarg_segment_size", Value::from(0)),
        (".kernarg_segment_align", Value::from(8)),
        (".group_segment_fixed_size", Value::from(0)),
        (".private_segment_fixed_size", Value::from(16)),
        (".wavefront_size", Value::from(64)),
        (".sgpr_count", Value::from(14)),
        (".vgpr_count", Value::from(vgpr_count)),
        (".agpr_count", Value::from(3)),
        (".sgpr_spill_count", Value::from(spill_count)),
        (".vgpr_spill_count", Value::from(4)),
        (".workgroup_processor_mode", Value::from(1)),
        (".max_flat_workgroup_size", Value::from(1024)),
    ])
}

fn hsaco_with_kernels(kernels: Vec<Value>) -> Vec<u8> {
    let metadata = map(vec![
        (
            "amdhsa.version",
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        ("amdhsa.target", Value::from("amdgcn-amd-amdhsa--gfx1151")),
        ("amdhsa.kernels", Value::Array(kernels)),
    ]);
    let mut encoded = Vec::new();
    write_value(&mut encoded, &metadata).unwrap();
    elf_with_metadata(&encoded)
}

fn map(fields: Vec<(&str, Value)>) -> Value {
    Value::Map(
        fields
            .into_iter()
            .map(|(key, value)| (Value::from(key), value))
            .collect(),
    )
}

fn elf_with_metadata(metadata: &[u8]) -> Vec<u8> {
    let owner = b"AMDGPU\0";
    let mut note = Vec::new();
    note.extend_from_slice(&(owner.len() as u32).to_le_bytes());
    note.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    note.extend_from_slice(&32_u32.to_le_bytes());
    note.extend_from_slice(owner);
    align(&mut note, 4);
    note.extend_from_slice(metadata);
    align(&mut note, 4);

    let mut bytes = vec![0; ELF_HEADER_BYTES];
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);
    let string_table = b"\0.note\0.shstrtab\0";
    let string_table_offset = bytes.len();
    bytes.extend_from_slice(string_table);
    align(&mut bytes, 8);
    let section_offset = bytes.len();
    bytes.resize(section_offset + 3 * SECTION_HEADER_BYTES, 0);

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = 4;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u32(&mut bytes, 48, 0x4a);
    write_u64(&mut bytes, 40, section_offset as u64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, 3);
    write_u16(&mut bytes, 62, 2);

    let note_header = section_offset + SECTION_HEADER_BYTES;
    write_u32(&mut bytes, note_header, 1);
    write_u32(&mut bytes, note_header + 4, 7);
    write_u64(&mut bytes, note_header + 8, 2);
    write_u64(&mut bytes, note_header + 24, note_offset as u64);
    write_u64(&mut bytes, note_header + 32, note.len() as u64);
    write_u64(&mut bytes, note_header + 48, 4);

    let strings_header = section_offset + 2 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, strings_header, 7);
    write_u32(&mut bytes, strings_header + 4, 3);
    write_u64(&mut bytes, strings_header + 24, string_table_offset as u64);
    write_u64(&mut bytes, strings_header + 32, string_table.len() as u64);
    write_u64(&mut bytes, strings_header + 48, 1);
    bytes
}

fn align(bytes: &mut Vec<u8>, alignment: usize) {
    let remainder = bytes.len() % alignment;
    if remainder != 0 {
        bytes.resize(bytes.len() + alignment - remainder, 0);
    }
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
use std::io::Write;
use std::process::{Command, Stdio};
