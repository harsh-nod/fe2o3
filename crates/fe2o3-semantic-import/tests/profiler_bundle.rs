use fe2o3_semantic_import::*;
use fe2o3_semantic_trace::*;
use std::io::Write;
use std::process::{Command, Stdio};

fn opaque(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn content(byte: u8, len: u64) -> ContentIdentityRecordV1 {
    ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: 1,
        digest: CaptureIdentityV1::new([byte; 32]).unwrap(),
        canonical_len: len,
    }
}

fn environment(devices: &[u8]) -> ProfilerEnvironmentBindingV4 {
    let source_agents = [17_u64, 19_u64];
    ProfilerEnvironmentBindingV4 {
        environment: content(10, 200),
        collector_tool: content(11, 50),
        collector_configuration: content(12, 80),
        stable_device_bindings: devices
            .iter()
            .enumerate()
            .map(|(index, byte)| ProfilerDeviceBindingV4 {
                source_agent_id: source_agents[index],
                stable_identity: content(*byte, 64),
            })
            .collect(),
    }
}

fn dispatch_binding(devices: &[u8]) -> ProfilerDispatchBindingV4 {
    ProfilerDispatchBindingV4 {
        environment: environment(devices),
        kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(opaque(1), 97).unwrap(),
        artifact: Some(ArtifactClaimV1 {
            identity: opaque(2),
            canonical_len: 4_096,
            format_version: 1,
        }),
        source_map: Some(
            ContentIdentityV1::new(
                ContentIdentitySchemeV1::RawCanonicalSha256,
                1,
                opaque(3),
                777,
            )
            .unwrap(),
        ),
        wave_width: WaveWidthV1::Wave64,
    }
}

fn json_source() -> &'static [u8] {
    br#"{"rocprofiler-sdk-tool":[{"buffer_records":{"kernel_dispatch":[{"start_timestamp":100,"end_timestamp":180,"dispatch_info":{"agent_id":{"handle":17},"workgroup_size":{"x":64,"y":1,"z":1},"grid_size":{"x":256,"y":1,"z":1}}}]}},{"buffer_records":{"kernel_dispatch":[{"start_timestamp":200,"end_timestamp":260,"dispatch_info":{"agent_id":{"handle":19},"workgroup_size":{"x":32,"y":2,"z":1},"grid_size":{"x":128,"y":2,"z":1}}}]}}]}"#
}

fn csv_source() -> &'static [u8] {
    include_bytes!("fixtures/rocprofv3-1.1-kernel-dispatch.csv")
}

#[test]
fn json_and_csv_bundles_are_canonical_bounded_and_identity_bound() {
    let json = import_rocprofv3_json_profiler_bundle_v4(json_source(), dispatch_binding(&[20, 21]))
        .unwrap();
    let csv =
        import_rocprofv3_csv_profiler_bundle_v4(csv_source(), dispatch_binding(&[20, 21])).unwrap();

    for bundle in [&json, &csv] {
        let bytes = encode_profiler_bundle_v4(bundle).unwrap();
        assert_eq!(decode_profiler_bundle_v4(&bytes).unwrap(), *bundle);
        assert_eq!(bundle.run_identity_origin, TruthOriginV1::Inferred);
        assert_eq!(bundle.environment.origin, TruthOriginV1::Declared);
        assert_eq!(bundle.source.origin, TruthOriginV1::Observed);
        assert_eq!(bundle.coverage.imported_dispatches, 2);
        assert_eq!(bundle.coverage.loss.state, LossStateV1::Unknown);
        assert_eq!(
            bundle.devices[0].stable_identity.value,
            Some(content(20, 64))
        );
        assert!(
            bundle
                .unavailable
                .contains(&ProfilerUnavailableFactV4::WaitEvents)
        );
        assert!(
            bundle
                .unavailable
                .contains(&ProfilerUnavailableFactV4::DecodedAttEvents)
        );
    }
    assert_ne!(csv.source.value, csv.normalized_projection.value);
    assert_eq!(
        csv.normalized_projection.value,
        Some(csv.dispatch_capture.as_ref().unwrap().runs[0].source)
    );
}

#[test]
fn device_bindings_join_by_absolute_agent_id_not_position() {
    let mut binding = dispatch_binding(&[20, 21]);
    binding.environment.stable_device_bindings.reverse();
    binding
        .environment
        .stable_device_bindings
        .push(ProfilerDeviceBindingV4 {
            source_agent_id: 99,
            stable_identity: content(22, 64),
        });
    let bundle = import_rocprofv3_json_profiler_bundle_v4(json_source(), binding).unwrap();
    assert_eq!(
        bundle.devices[0].stable_identity.value,
        Some(content(20, 64))
    );
    assert_eq!(
        bundle.devices[1].stable_identity.value,
        Some(content(21, 64))
    );
    assert_eq!(bundle.devices.len(), 2);

    let missing = ProfilerDispatchBindingV4 {
        environment: environment(&[20]),
        ..dispatch_binding(&[20, 21])
    };
    assert!(matches!(
        import_rocprofv3_json_profiler_bundle_v4(json_source(), missing),
        Err(ProfilerBundleErrorV4::MissingDeviceBinding)
    ));

    let mut duplicate = dispatch_binding(&[20, 21]);
    duplicate.environment.stable_device_bindings[1].source_agent_id = 17;
    assert!(matches!(
        import_rocprofv3_json_profiler_bundle_v4(json_source(), duplicate),
        Err(ProfilerBundleErrorV4::DuplicateSourceAgentBinding)
    ));
}

#[test]
fn csv_import_is_strict_about_schema_values_and_resource_bounds() {
    let legacy_agent_ids = String::from_utf8(csv_source().to_vec())
        .unwrap()
        .replace("Agent 17", "17")
        .replace("Agent 19", "19");
    assert!(
        import_rocprofv3_csv_profiler_bundle_v4(
            legacy_agent_ids.as_bytes(),
            dispatch_binding(&[20, 21])
        )
        .is_ok()
    );

    for noncanonical in ["Agent 017", "Agent 0x11", "Agent 17 "] {
        let source =
            String::from_utf8(csv_source().to_vec())
                .unwrap()
                .replacen("Agent 17", noncanonical, 1);
        assert!(matches!(
            import_rocprofv3_csv_profiler_bundle_v4(source.as_bytes(), dispatch_binding(&[20, 21])),
            Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
        ));
    }

    let unknown = String::from_utf8(csv_source().to_vec())
        .unwrap()
        .replacen("Kind,", "Unknown,Kind,", 1)
        .replacen("KERNEL_DISPATCH,", "x,KERNEL_DISPATCH,", 2);
    assert!(matches!(
        import_rocprofv3_csv_profiler_bundle_v4(unknown.as_bytes(), dispatch_binding(&[20, 21])),
        Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
    ));

    let duplicate = String::from_utf8(csv_source().to_vec())
        .unwrap()
        .replacen("Kind,", "Kind,Kind,", 1)
        .replacen("KERNEL_DISPATCH,", "KERNEL_DISPATCH,KERNEL_DISPATCH,", 2);
    assert!(matches!(
        import_rocprofv3_csv_profiler_bundle_v4(duplicate.as_bytes(), dispatch_binding(&[20, 21])),
        Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
    ));

    let bad_number = String::from_utf8(csv_source().to_vec())
        .unwrap()
        .replacen(",100,180", ",wat,180", 1);
    assert!(matches!(
        import_rocprofv3_csv_profiler_bundle_v4(bad_number.as_bytes(), dispatch_binding(&[20, 21])),
        Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
    ));

    let oversized = vec![b' '; MAX_PROFILER_SOURCE_BYTES_V4 as usize + 1];
    assert!(matches!(
        import_rocprofv3_csv_profiler_bundle_v4(&oversized, dispatch_binding(&[20])),
        Err(ProfilerBundleErrorV4::SourceSizeOutOfRange)
    ));
}

#[test]
fn att_import_retains_only_safe_references_and_never_claims_decoding() {
    let source = br#"{"counter_names":[],"gfxip":9,"gfxv":"vega","global_begin_time":0,"is_pcs_stochastic":false,"pc_sampling":false,"thread_trace":true,"version":"3.0.0","wave_filenames":{"0":{"0":{"0":{"0":["waves/se0.json",10,20]}}}},"se_filenames":["se0.json"]}"#;
    let bundle = import_rocprofv3_att_profiler_bundle_v4(
        source,
        ProfilerAttBindingV4 {
            environment: environment(&[20]),
            source_agent_id: 17,
            referenced_artifacts: vec![ProfilerAttArtifactBindingV4 {
                reference: "se0.json".to_owned(),
                content: content(30, 400),
            }],
        },
    )
    .unwrap();
    let att = bundle.att.as_ref().unwrap();
    assert_eq!(bundle.coverage.att_references, 2);
    assert_eq!(att.decoder_output_origin, TruthOriginV1::Unavailable);
    assert_eq!(att.references[0].kind, AttReferenceKindV4::WaveTimeline);
    assert_eq!(att.references[0].content.origin, TruthOriginV1::Unavailable);
    assert_eq!(
        att.references[1].kind,
        AttReferenceKindV4::ShaderEngineMetadata
    );
    assert_eq!(att.references[1].content.value, Some(content(30, 400)));
    assert!(
        bundle
            .unavailable
            .contains(&ProfilerUnavailableFactV4::WaitEvents)
    );

    let installed = include_bytes!("fixtures/rocprofv3-1.1-att-manifest.json");
    assert!(
        import_rocprofv3_att_profiler_bundle_v4(
            installed,
            ProfilerAttBindingV4 {
                environment: environment(&[20]),
                source_agent_id: 17,
                referenced_artifacts: Vec::new(),
            }
        )
        .is_ok()
    );
}

#[test]
fn att_import_rejects_unsafe_duplicate_and_unrecognized_evidence() {
    for reference in [
        "/absolute.json",
        "../parent.json",
        "a/./b.json",
        "C:drive.json",
    ] {
        let source = serde_json::to_vec(&serde_json::json!({
            "thread_trace": true,
            "version": "3.0.0",
            "wave_filenames": {"0":{"0":{"0":{"0":[reference, 1, 2]}}}}
        }))
        .unwrap();
        assert!(matches!(
            import_rocprofv3_att_profiler_bundle_v4(
                &source,
                ProfilerAttBindingV4 {
                    environment: environment(&[20]),
                    source_agent_id: 17,
                    referenced_artifacts: Vec::new(),
                }
            ),
            Err(ProfilerBundleErrorV4::InvalidAttReference)
        ));
    }

    let unknown = br#"{"thread_trace":true,"version":"3.0.0","unknown":1,"wave_filenames":{"0":{"0":{"0":{"0":["wave.json",1,2]}}}}}"#;
    assert!(matches!(
        import_rocprofv3_att_profiler_bundle_v4(
            unknown,
            ProfilerAttBindingV4 {
                environment: environment(&[20]),
                source_agent_id: 17,
                referenced_artifacts: Vec::new(),
            }
        ),
        Err(ProfilerBundleErrorV4::InvalidAttManifest)
    ));
}

#[test]
fn decoder_rejects_noncanonical_and_stale_bundle_claims() {
    let bundle =
        import_rocprofv3_json_profiler_bundle_v4(json_source(), dispatch_binding(&[20, 21]))
            .unwrap();
    let mut bytes = encode_profiler_bundle_v4(&bundle).unwrap();
    bytes.push(b'\n');
    assert!(matches!(
        decode_profiler_bundle_v4(&bytes),
        Err(ProfilerBundleErrorV4::NonCanonicalEncoding)
    ));

    let mut stale = bundle;
    stale.run_identity = CaptureIdentityV1::new([99; 32]).unwrap();
    assert!(matches!(
        decode_profiler_bundle_v4(&serde_json::to_vec(&stale).unwrap()),
        Err(ProfilerBundleErrorV4::StaleRunIdentity)
    ));
}

#[test]
fn profiler_import_cli_emits_canonical_v4_without_paths_or_native_handles() {
    let id = |byte: u8, len: u64| format!("domain:1:{}:{len}", format!("{byte:02x}").repeat(32));
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-import"))
        .args([
            "dispatch-csv-v4",
            "--environment",
            &id(10, 200),
            "--tool",
            &id(11, 50),
            "--config",
            &id(12, 80),
            "--device-binding",
            &format!("17={}", id(20, 64)),
            "--device-binding",
            &format!("19={}", id(21, 64)),
            "--kir-sha256",
            &"01".repeat(32),
            "--kir-len",
            "97",
            "--wave-width",
            "64",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(csv_source()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let bundle = decode_profiler_bundle_v4(&output.stdout).unwrap();
    assert_eq!(bundle.schema_version, 4);
    let encoded = String::from_utf8(output.stdout).unwrap();
    assert!(!encoded.contains("generic,kernel"));
    assert!(!encoded.contains("Queue_Id"));
}

#[test]
fn att_cli_argument_bound_covers_more_than_the_legacy_128_arguments() {
    let id = |byte: u8, len: u64| format!("domain:1:{}:{len}", format!("{byte:02x}").repeat(32));
    let references = (0..64)
        .map(|index| format!("se-{index}.json"))
        .collect::<Vec<_>>();
    let source = serde_json::to_vec(&serde_json::json!({
        "thread_trace": true,
        "version": "3.0.0",
        "wave_filenames": {"0":{"0":{"0":{"0":["wave.json", 1, 2]}}}},
        "se_filenames": references,
    }))
    .unwrap();
    let mut arguments = vec![
        "att-v4".to_owned(),
        "--environment".to_owned(),
        id(10, 200),
        "--tool".to_owned(),
        id(11, 50),
        "--config".to_owned(),
        id(12, 80),
        "--device-binding".to_owned(),
        format!("17={}", id(20, 64)),
        "--att-agent-id".to_owned(),
        "17".to_owned(),
    ];
    for reference in &references {
        arguments.push("--att-artifact".to_owned());
        arguments.push(format!("{reference}={}", id(30, 400)));
    }
    assert!(arguments.len() > 128);
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-import"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&source).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bundle = decode_profiler_bundle_v4(&output.stdout).unwrap();
    assert_eq!(bundle.coverage.att_references, 65);
}
