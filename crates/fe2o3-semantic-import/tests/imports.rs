use std::io::Write;
use std::process::{Command, Stdio};

use fe2o3_semantic_import::*;
use fe2o3_semantic_trace::*;

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn kir_claim() -> KernelIrIdentityClaimV1 {
    KernelIrIdentityClaimV1::canonical_v7_claim(identity(1), 97).unwrap()
}

fn artifact() -> ArtifactClaimV1 {
    ArtifactClaimV1 {
        identity: identity(0xaa),
        canonical_len: 4_096,
        format_version: 1,
    }
}

fn sparse_binding() -> SparseImportBindingV1 {
    SparseImportBindingV1 {
        kernel_ir_claim: kir_claim(),
        artifact: Some(artifact()),
        launch: LaunchGeometryV1::new_exact([65, 2, 1], [2, 1, 1], [64, 2, 1], WaveWidthV1::Wave64)
            .unwrap(),
    }
}

fn rocprof_source(start: u64, end: u64, raw_handle: u64) -> Vec<u8> {
    format!(
        r#"{{
          "rocprofiler-sdk-tool": [
            {{"metadata": {{"pid": 9988}},
              "buffer_records": {{"kernel_dispatch": [
                {{"size": 128, "kind": 1, "operation": 2,
                  "thread_id": 7788,
                  "start_timestamp": {start}, "end_timestamp": {end},
                  "dispatch_info": {{
                    "agent_id": {{"handle": {raw_handle}}},
                    "queue_id": {{"handle": 18446744073709551600}},
                    "kernel_id": 41, "dispatch_id": 99,
                    "workgroup_size": {{"x": 64, "y": 2, "z": 1}},
                    "grid_size": {{"x": 65, "y": 2, "z": 1}}
                  }}
                }}
              ]}}
            }}
          ]
        }}"#
    )
    .into_bytes()
}

fn rocprof_binding() -> RocprofBindingV1 {
    RocprofBindingV1 {
        kernel_ir_claim: kir_claim(),
        artifact: Some(artifact()),
        wave_width: WaveWidthV1::Wave64,
        selection: RocprofDispatchSelectionV1 {
            process_index: 0,
            dispatch_index: 0,
        },
    }
}

#[test]
fn rocprof_import_is_observed_bounded_and_deterministic() {
    let source = rocprof_source(100, 350, 0xfeed_beef);
    let first =
        import_rocprofv3_json_v1(&source, rocprof_binding(), ImportLimitsV1::default()).unwrap();
    let second =
        import_rocprofv3_json_v1(&source, rocprof_binding(), ImportLimitsV1::default()).unwrap();
    let first_bytes = encode_trace_v1(first.trace()).unwrap();
    let second_bytes = encode_trace_v1(second.trace()).unwrap();

    assert_eq!(first_bytes, second_bytes);
    assert!(first_bytes.len() as u64 <= MAX_IMPORT_OUTPUT_BYTES_V1);
    assert_eq!(&decode_trace_v1(&first_bytes).unwrap(), first.trace());
    assert_eq!(first.source_kind(), ImportSourceKindV1::Rocprofv3Json);
    assert_eq!(
        first.source_identity().scheme(),
        ContentIdentitySchemeV1::DomainSeparatedSha256
    );
    assert_eq!(first.selected_record_ordinal(), Some(0));
    assert_eq!(first.imported_facts(), &[ImportedFactV1::DispatchEnvelope]);
    assert!(
        first
            .unavailable_facts()
            .contains(&UnavailableImportFactV1::LaneHistory)
    );

    let trace = first.trace();
    assert_eq!(trace.events().len(), 2);
    assert_eq!(trace.header().launch().logical_grid(), [65, 2, 1]);
    assert_eq!(trace.header().launch().grid_workgroups(), [2, 1, 1]);
    assert_eq!(
        trace.header().completeness(),
        TraceCompletenessV1::Truncated {
            reason: TruncationReasonV1::CollectorLoss,
            emitted_events: 2,
            dropped_events: DroppedEventCountV1::Unknown,
        }
    );
    assert_eq!(
        trace.header().boundaries(),
        CaptureBoundariesV1::FULL_DISPATCH
    );
    for event in trace.events() {
        assert_eq!(event.provenance(), FactProvenanceV1::Observed);
        assert_eq!(
            event.scope(),
            ExecutionScopeV1::dispatch(trace.header().dispatch())
        );
        assert!(
            event
                .evidence_refs()
                .iter()
                .any(|evidence| evidence.kind() == EvidenceKindV1::RuntimeObservation)
        );
        assert!(
            event
                .evidence_refs()
                .iter()
                .any(|evidence| evidence.kind() == EvidenceKindV1::Artifact)
        );
    }
    assert!(matches!(
        trace.events()[0].kind(),
        TraceEventKindV1::Dispatch(DispatchEventV1::Begin)
    ));
    assert!(matches!(
        trace.events()[1].kind(),
        TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed))
    ));
}

#[test]
fn raw_collector_identifiers_only_affect_opaque_source_binding() {
    let a = import_rocprofv3_json_v1(
        &rocprof_source(100, 350, 17),
        rocprof_binding(),
        ImportLimitsV1::default(),
    )
    .unwrap();
    let b = import_rocprofv3_json_v1(
        &rocprof_source(100, 350, 18),
        rocprof_binding(),
        ImportLimitsV1::default(),
    )
    .unwrap();

    assert_ne!(a.source_identity(), b.source_identity());
    assert_ne!(a.trace().header().dispatch(), b.trace().header().dispatch());
    assert_eq!(a.trace().header().launch(), b.trace().header().launch());
    assert_eq!(
        a.trace().events()[0].sequence(),
        b.trace().events()[0].sequence()
    );
}

#[test]
fn rocprof_rejects_hostile_or_ambiguous_semantic_fields() {
    let duplicate = br#"{"rocprofiler-sdk-tool":[{"buffer_records":{"kernel_dispatch":[{"start_timestamp":1,"start_timestamp":2,"end_timestamp":3,"dispatch_info":{"workgroup_size":{"x":1,"y":1,"z":1},"grid_size":{"x":1,"y":1,"z":1}}}]}}]}"#;
    assert!(matches!(
        import_rocprofv3_json_v1(duplicate, rocprof_binding(), ImportLimitsV1::default()),
        Err(ImportErrorV1::InvalidRocprofJson)
    ));
    assert!(matches!(
        import_rocprofv3_json_v1(
            &rocprof_source(9, 8, 17),
            rocprof_binding(),
            ImportLimitsV1::default()
        ),
        Err(ImportErrorV1::TimestampOrder)
    ));
    let zero_geometry = br#"{"rocprofiler-sdk-tool":[{"buffer_records":{"kernel_dispatch":[{"start_timestamp":1,"end_timestamp":2,"dispatch_info":{"workgroup_size":{"x":0,"y":1,"z":1},"grid_size":{"x":1,"y":1,"z":1}}}]}}]}"#;
    assert!(matches!(
        import_rocprofv3_json_v1(zero_geometry, rocprof_binding(), ImportLimitsV1::default()),
        Err(ImportErrorV1::InvalidLaunchGeometry)
    ));
}

#[test]
fn att_manifest_is_sparse_and_truthful() {
    let manifest = br#"{"counter_names":[],"gfxip":9,"gfxv":"vega","global_begin_time":0,"is_pcs_stochastic":false,"pc_sampling":false,"thread_trace":true,"version":"3.0.0","wave_filenames":{"0":{"0":{"0":{"0":["se0_sm0_sl0_wv0.json",10,20]}}}}}"#;
    let imported =
        import_rocprofv3_att_manifest_v1(manifest, sparse_binding(), ImportLimitsV1::default())
            .unwrap();
    assert_eq!(
        imported.source_kind(),
        ImportSourceKindV1::Rocprofv3AttManifest
    );
    assert!(imported.trace().events().is_empty());
    assert_eq!(
        imported.trace().header().boundaries(),
        CaptureBoundariesV1::new(
            CaptureStartBoundaryV1::DispatchAlreadyActive,
            CaptureEndBoundaryV1::DispatchContinuesAfterCapture,
        )
    );
    assert_eq!(
        imported.unavailable_facts(),
        &[
            UnavailableImportFactV1::DispatchTiming,
            UnavailableImportFactV1::InvocationHistory,
            UnavailableImportFactV1::WorkgroupHistory,
            UnavailableImportFactV1::WaveHistory,
            UnavailableImportFactV1::LaneHistory,
            UnavailableImportFactV1::KirSiteHistory,
            UnavailableImportFactV1::MemoryHistory,
            UnavailableImportFactV1::RegisterAndValueState,
            UnavailableImportFactV1::DiagnosticAndFaultHistory,
        ]
    );

    let false_manifest = manifest.replace(b"\"thread_trace\":true", b"\"thread_trace\":false");
    assert!(matches!(
        import_rocprofv3_att_manifest_v1(
            &false_manifest,
            sparse_binding(),
            ImportLimitsV1::default()
        ),
        Err(ImportErrorV1::InvalidAttManifest)
    ));
}

#[test]
fn installed_rocprofiler_1_1_att_manifest_is_accepted() {
    let installed = br#"{"wave_filenames":{"0":{"0":{"0":{"0":["se0_sm0_sl0_wv0.json",10,20]}}}},"se_filenames":["se0.json"],"global_begin_time":10,"gfxv":"vega"}"#;
    let imported =
        import_rocprofv3_att_manifest_v1(installed, sparse_binding(), ImportLimitsV1::default())
            .unwrap();
    assert!(imported.trace().events().is_empty());
    assert_eq!(
        imported.imported_facts(),
        &[ImportedFactV1::AttCaptureManifest]
    );

    let ambiguous = br#"{"wave_filenames":{"0":{}},"global_begin_time":10,"gfxv":"vega"}"#;
    assert!(matches!(
        import_rocprofv3_att_manifest_v1(ambiguous, sparse_binding(), ImportLimitsV1::default()),
        Err(ImportErrorV1::InvalidAttManifest)
    ));
}

#[test]
fn source_limits_apply_before_parsing_at_exact_boundary() {
    let limits = ImportLimitsV1::new(16).unwrap();
    assert!(matches!(
        import_rocprofv3_json_v1(&[b' '; 16], rocprof_binding(), limits),
        Err(ImportErrorV1::InvalidRocprofJson)
    ));
    assert!(matches!(
        import_rocprofv3_json_v1(&[b' '; 17], rocprof_binding(), limits),
        Err(ImportErrorV1::SourceTooLarge {
            actual: 17,
            max: 16
        })
    ));
    assert!(matches!(
        ImportLimitsV1::new(MAX_IMPORT_SOURCE_BYTES_V1 + 1),
        Err(ImportErrorV1::SourceLimitOutOfRange { .. })
    ));
}

#[test]
fn json_collection_limits_apply_during_deserialization() {
    let process = r#"{"buffer_records":{"kernel_dispatch":[]}}"#;
    let source = format!(
        "{{\"rocprofiler-sdk-tool\":[{}]}}",
        vec![process; MAX_ROCPROF_PROCESSES_V1 + 1].join(",")
    );
    assert!(source.len() as u64 <= MAX_IMPORT_SOURCE_BYTES_V1);
    assert!(matches!(
        import_rocprofv3_json_v1(
            source.as_bytes(),
            rocprof_binding(),
            ImportLimitsV1::default()
        ),
        Err(ImportErrorV1::InvalidRocprofJson)
    ));

    let deeply_nested = format!(
        "{{\"version\":\"3.0.0\",\"thread_trace\":true,\"wave_filenames\":{}0{}}}",
        "[".repeat(256),
        "]".repeat(256)
    );
    assert!(matches!(
        import_rocprofv3_att_manifest_v1(
            deeply_nested.as_bytes(),
            sparse_binding(),
            ImportLimitsV1::default()
        ),
        Err(ImportErrorV1::InvalidAttManifest)
    ));
}

fn run_cli(arguments: &[&str], input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-trace-import"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let write_result = child.stdin.take().unwrap().write_all(input);
    let output = child.wait_with_output().unwrap();
    match write_result {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {
            assert!(
                !output.status.success(),
                "stdin closed early even though the importer succeeded"
            );
        }
        Err(error) => panic!("could not write importer stdin: {error}"),
    }
    output
}

fn rocprof_cli_arguments() -> Vec<String> {
    vec![
        "rocprofv3-json".into(),
        "--kir-sha256".into(),
        "01".repeat(32),
        "--kir-len".into(),
        "97".into(),
        "--wave-width".into(),
        "64".into(),
        "--process-index".into(),
        "0".into(),
        "--dispatch-index".into(),
        "0".into(),
    ]
}

#[test]
fn cli_is_stdin_only_deterministic_and_rejects_duplicate_flags() {
    let arguments = rocprof_cli_arguments();
    let borrowed: Vec<_> = arguments.iter().map(String::as_str).collect();
    let first = run_cli(&borrowed, &rocprof_source(1, 2, 17));
    let second = run_cli(&borrowed, &rocprof_source(1, 2, 17));
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(first.stdout, second.stdout);
    assert!(first.stdout.len() as u64 <= MAX_IMPORT_OUTPUT_BYTES_V1);
    decode_trace_v1(&first.stdout).unwrap();

    let duplicate_arguments = ["rocprofv3-json", "--kir-len", "1", "--kir-len", "2"];
    let duplicate = run_cli(&duplicate_arguments, b"ignored");
    let repeated_duplicate = run_cli(&duplicate_arguments, b"ignored");
    assert_eq!(duplicate.status.code(), Some(1));
    assert_eq!(duplicate.status.code(), repeated_duplicate.status.code());
    assert!(duplicate.stdout.is_empty());
    assert_eq!(duplicate.stdout, repeated_duplicate.stdout);
    assert_eq!(
        duplicate.stderr,
        b"{\"error\":\"arguments\",\"message\":\"each import flag may appear at most once\"}\n"
    );
    assert_eq!(duplicate.stderr, repeated_duplicate.stderr);

    let path_like = run_cli(&["rocprofv3-json", "/dev/stdin"], b"ignored");
    assert_eq!(path_like.status.code(), Some(1));
    assert!(path_like.stdout.is_empty());
    assert_eq!(
        path_like.stderr,
        b"{\"error\":\"arguments\",\"message\":\"every flag requires one value\"}\n"
    );

    let retired = run_cli(&["rocgdb-s09"], b"ignored");
    assert_eq!(retired.status.code(), Some(1));
    assert!(retired.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&retired.stderr).contains("rocgdb-s09"));
}

#[test]
fn cli_rejects_over_limit_stream_before_json_parse() {
    let arguments = rocprof_cli_arguments();
    let borrowed: Vec<_> = arguments.iter().map(String::as_str).collect();
    let input = vec![b' '; usize::try_from(MAX_IMPORT_SOURCE_BYTES_V1).unwrap() + 1];
    let output = run_cli(&borrowed, &input);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("input_too_large"));
}

trait ReplaceBytes {
    fn replace(&self, from: &[u8], to: &[u8]) -> Vec<u8>;
}

impl ReplaceBytes for [u8] {
    fn replace(&self, from: &[u8], to: &[u8]) -> Vec<u8> {
        let position = self
            .windows(from.len())
            .position(|window| window == from)
            .unwrap();
        let mut output = Vec::with_capacity(self.len() - from.len() + to.len());
        output.extend_from_slice(&self[..position]);
        output.extend_from_slice(to);
        output.extend_from_slice(&self[position + from.len()..]);
        output
    }
}
