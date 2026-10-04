use fe2o3_semantic_import::*;

const EXPORT: &[u8] = include_bytes!("fixtures/rocprofiler-sdk-7.2.4-decoded-att-v1.json");
const MANIFEST: &[u8] = br#"{"counter_names":[],"gfxip":9,"gfxv":"vega","global_begin_time":0,"is_pcs_stochastic":false,"pc_sampling":false,"thread_trace":true,"version":"3.0.0","wave_filenames":{"0":{"0":{"0":{"0":["waves/se0.json",10,20]}}}},"se_filenames":["se0.json"]}"#;

fn identity(byte: u8, len: u64, scheme: ContentSchemeV1) -> ContentIdentityRecordV1 {
    ContentIdentityRecordV1 {
        scheme,
        format_version: 1,
        digest: CaptureIdentityV1::new([byte; 32]).unwrap(),
        canonical_len: len,
    }
}

fn environment() -> ProfilerEnvironmentBindingV4 {
    ProfilerEnvironmentBindingV4 {
        environment: identity(10, 200, ContentSchemeV1::DomainSeparatedSha256),
        collector_tool: identity(11, 50, ContentSchemeV1::DomainSeparatedSha256),
        collector_configuration: identity(12, 80, ContentSchemeV1::DomainSeparatedSha256),
        stable_device_bindings: vec![ProfilerDeviceBindingV4 {
            source_agent_id: 17,
            stable_identity: identity(20, 64, ContentSchemeV1::DomainSeparatedSha256),
        }],
    }
}

fn bundle(complete: bool) -> Vec<u8> {
    let mut referenced_artifacts = Vec::new();
    if complete {
        referenced_artifacts.push(ProfilerAttArtifactBindingV4 {
            reference: "waves/se0.json".to_owned(),
            content: identity(31, 401, ContentSchemeV1::DomainSeparatedSha256),
        });
    }
    if complete {
        referenced_artifacts.push(ProfilerAttArtifactBindingV4 {
            reference: "se0.json".to_owned(),
            content: identity(32, 402, ContentSchemeV1::DomainSeparatedSha256),
        });
    }
    let value = import_rocprofv3_att_profiler_bundle_v4(
        MANIFEST,
        ProfilerAttBindingV4 {
            environment: environment(),
            source_agent_id: 17,
            referenced_artifacts,
        },
    )
    .unwrap();
    encode_profiler_bundle_v4(&value).unwrap()
}

fn binding() -> DecodedAttImportBindingV1 {
    DecodedAttImportBindingV1 {
        trace_decoder_types_header: ContentIdentityRecordV1 {
            scheme: ContentSchemeV1::RawCanonicalSha256,
            format_version: 1,
            digest: CaptureIdentityV1::new(
                ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_SHA256_V1,
            )
            .unwrap(),
            canonical_len: ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_BYTES_V1,
        },
        trace_decoder_api_header: ContentIdentityRecordV1 {
            scheme: ContentSchemeV1::RawCanonicalSha256,
            format_version: 1,
            digest: CaptureIdentityV1::new(
                ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_SHA256_V1,
            )
            .unwrap(),
            canonical_len: ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_BYTES_V1,
        },
        decoder_library: identity(50, 50_000, ContentSchemeV1::RawCanonicalSha256),
        exporter_tool: identity(51, 25_000, ContentSchemeV1::RawCanonicalSha256),
    }
}

fn canonical_export() -> Vec<u8> {
    let mut value = EXPORT.to_vec();
    assert_eq!(value.pop(), Some(b'\n'));
    value
}

#[test]
fn admits_all_official_callback_families_without_authenticating_the_decoder() {
    let export = canonical_export();
    let bundle = bundle(true);
    let value = import_rocprofiler_sdk_decoded_att_v1(
        &export,
        &bundle,
        binding(),
        DecodedAttImportLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(value.gfxip_major, 9);
    assert_eq!(value.coverage.callback_count, 8);
    assert_eq!(value.coverage.occupancy_count, 2);
    assert_eq!(value.coverage.wave_count, 1);
    assert_eq!(value.coverage.wave_state_count, 5);
    assert_eq!(value.coverage.instruction_count, 13);
    assert_eq!(value.coverage.data_lost_info_count, 1);
    assert_eq!(
        value.coverage.completeness,
        DecodedAttCompletenessV1::IncompleteInfoReported
    );
    assert_eq!(
        value.coverage.loss,
        DecodedAttLossStateV1::ExternalDecoderReportedDataLost
    );
    assert_eq!(
        value.decoder.authenticity,
        DecodedAttAuthenticityV1::UnavailableSelfClaimedExternalDecoder
    );
    assert_eq!(
        value.raw_decode_relation.state,
        DecodedAttRawRelationV1::ExternalDecoderDeclaredCompleteExactWaveInputs
    );
    assert_eq!(
        value.occupancy[1].pc.availability,
        DecodedAttPcAvailabilityV1::UnavailableNativeVirtualAddressRedacted
    );
    let encoded = encode_decoded_att_v1(&value).unwrap();
    assert_eq!(decode_decoded_att_v1(&encoded).unwrap(), value);
    assert_eq!(
        readmit_rocprofiler_sdk_decoded_att_v1(
            &export,
            &bundle,
            &encoded,
            binding(),
            DecodedAttImportLimitsV1::default(),
        )
        .unwrap(),
        value
    );
    let text = std::str::from_utf8(&encoded).unwrap();
    assert!(!text.contains("code_object_id"));
    assert!(!text.contains("load_id"));
    assert!(!text.contains("3735928559"));
}

#[test]
fn missing_exact_raw_identity_keeps_the_decode_relation_unavailable() {
    let complete = canonical_export();
    let wave_identity = r#"{"scheme":"domain_separated_sha256","format_version":1,"digest":"1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f","canonical_len":401}"#;
    let source = std::str::from_utf8(&complete)
        .unwrap()
        .replacen(wave_identity, "null", 1)
        .into_bytes();
    let incomplete_bundle = import_rocprofv3_att_profiler_bundle_v4(
        MANIFEST,
        ProfilerAttBindingV4 {
            environment: environment(),
            source_agent_id: 17,
            referenced_artifacts: vec![ProfilerAttArtifactBindingV4 {
                reference: "se0.json".to_owned(),
                content: identity(32, 402, ContentSchemeV1::DomainSeparatedSha256),
            }],
        },
    )
    .unwrap();
    let incomplete_bundle = encode_profiler_bundle_v4(&incomplete_bundle).unwrap();
    let value = import_rocprofiler_sdk_decoded_att_v1(
        &source,
        &incomplete_bundle,
        binding(),
        DecodedAttImportLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        value.raw_decode_relation,
        DecodedAttRawDecodeRelationV1 {
            origin: TruthOriginV1::Unavailable,
            state: DecodedAttRawRelationV1::UnavailableMissingExactRawReferenceIdentity,
        }
    );
}

#[test]
fn rejects_mutated_abi_records_bindings_and_noncanonical_inputs() {
    let source = canonical_export();
    let bundle = bundle(true);
    for mutated in [
        String::from_utf8(source.clone())
            .unwrap()
            .replacen("\"reserved\":0", "\"reserved\":1", 1),
        String::from_utf8(source.clone())
            .unwrap()
            .replacen("\"simd\":2", "\"simd\":4", 1),
        String::from_utf8(source.clone())
            .unwrap()
            .replacen("\"bank\":1", "\"bank\":2", 1),
        String::from_utf8(source.clone()).unwrap().replacen(
            "\"code_object_id\":77",
            "\"code_object_id\":78",
            1,
        ),
        String::from_utf8(source.clone()).unwrap().replacen(
            "\"stall\":1,\"duration\":2",
            "\"stall\":3,\"duration\":2",
            1,
        ),
        String::from_utf8(source.clone()).unwrap().replacen(
            "\"category\":\"smem\"",
            "\"category\":\"unknown\"",
            1,
        ),
        String::from_utf8(source.clone())
            .unwrap()
            .replacen("waves/se0.json", "../se0.json", 1),
        format!(" {}", String::from_utf8(source.clone()).unwrap()),
    ] {
        assert!(
            import_rocprofiler_sdk_decoded_att_v1(
                mutated.as_bytes(),
                &bundle,
                binding(),
                DecodedAttImportLimitsV1::default(),
            )
            .is_err()
        );
    }

    let mut wrong_binding = binding();
    wrong_binding.trace_decoder_types_header.digest = CaptureIdentityV1::new([99; 32]).unwrap();
    assert!(
        import_rocprofiler_sdk_decoded_att_v1(
            &source,
            &bundle,
            wrong_binding,
            DecodedAttImportLimitsV1::default(),
        )
        .is_err()
    );

    let other_bundle = import_rocprofv3_att_profiler_bundle_v4(
        br#"{"wave_filenames":{"0":{"0":{"0":{"0":["different.json",1,2]}}}},"se_filenames":["se0.json"],"global_begin_time":10,"gfxv":"vega"}"#,
        ProfilerAttBindingV4 {
            environment: environment(),
            source_agent_id: 17,
            referenced_artifacts: Vec::new(),
        },
    )
    .unwrap();
    let other_bundle = encode_profiler_bundle_v4(&other_bundle).unwrap();
    assert!(
        import_rocprofiler_sdk_decoded_att_v1(
            &source,
            &other_bundle,
            binding(),
            DecodedAttImportLimitsV1::default(),
        )
        .is_err()
    );
}

#[test]
fn bounds_inputs_and_rejects_interchange_mutation() {
    let source = canonical_export();
    let bundle = bundle(true);
    let limits = DecodedAttImportLimitsV1::new((source.len() - 1) as u64, 1024).unwrap();
    assert!(matches!(
        import_rocprofiler_sdk_decoded_att_v1(&source, &bundle, binding(), limits),
        Err(DecodedAttErrorV1::InputTooLarge)
    ));
    let value = import_rocprofiler_sdk_decoded_att_v1(
        &source,
        &bundle,
        binding(),
        DecodedAttImportLimitsV1::default(),
    )
    .unwrap();
    let encoded = encode_decoded_att_v1(&value).unwrap();
    let changed = std::str::from_utf8(&encoded).unwrap().replacen(
        "\"callback_count\":8",
        "\"callback_count\":999",
        1,
    );
    assert!(matches!(
        decode_decoded_att_v1(changed.as_bytes()),
        Err(DecodedAttErrorV1::InvalidCoverage)
    ));
    let decoder_substitution =
        std::str::from_utf8(&encoded)
            .unwrap()
            .replacen(&"32".repeat(32), &"63".repeat(32), 1);
    assert!(matches!(
        readmit_rocprofiler_sdk_decoded_att_v1(
            &source,
            &bundle,
            decoder_substitution.as_bytes(),
            binding(),
            DecodedAttImportLimitsV1::default(),
        ),
        Err(DecodedAttErrorV1::StaleInterchange)
    ));
    let manifest_substitution = std::str::from_utf8(&encoded).unwrap().replacen(
        &format!(
            "\"att_manifest\":{}",
            serde_json::to_string(&value.att_manifest).unwrap()
        ),
        &format!(
            "\"att_manifest\":{}",
            serde_json::to_string(&identity(
                99,
                value.att_manifest.canonical_len,
                value.att_manifest.scheme,
            ))
            .unwrap()
        ),
        1,
    );
    assert!(matches!(
        readmit_rocprofiler_sdk_decoded_att_v1(
            &source,
            &bundle,
            manifest_substitution.as_bytes(),
            binding(),
            DecodedAttImportLimitsV1::default(),
        ),
        Err(DecodedAttErrorV1::StaleInterchange)
    ));
    let mut trailing = encoded;
    trailing.push(b'\n');
    assert!(matches!(
        decode_decoded_att_v1(&trailing),
        Err(DecodedAttErrorV1::NonCanonicalInterchange)
    ));
}

#[test]
fn aggregates_multiple_decode_invocations_and_types_partial_wave_coverage() {
    let manifest = br#"{"counter_names":[],"gfxip":9,"gfxv":"vega","global_begin_time":0,"is_pcs_stochastic":false,"pc_sampling":false,"thread_trace":true,"version":"3.0.0","wave_filenames":{"0":{"0":{"0":{"0":["waves/se0.json",10,20],"1":["waves/se1.json",20,30]}}}},"se_filenames":["se0.json"]}"#;
    let bundle = import_rocprofv3_att_profiler_bundle_v4(
        manifest,
        ProfilerAttBindingV4 {
            environment: environment(),
            source_agent_id: 17,
            referenced_artifacts: vec![
                ProfilerAttArtifactBindingV4 {
                    reference: "waves/se0.json".to_owned(),
                    content: identity(31, 401, ContentSchemeV1::DomainSeparatedSha256),
                },
                ProfilerAttArtifactBindingV4 {
                    reference: "waves/se1.json".to_owned(),
                    content: identity(33, 403, ContentSchemeV1::DomainSeparatedSha256),
                },
                ProfilerAttArtifactBindingV4 {
                    reference: "se0.json".to_owned(),
                    content: identity(32, 402, ContentSchemeV1::DomainSeparatedSha256),
                },
            ],
        },
    )
    .unwrap();
    let bundle = encode_profiler_bundle_v4(&bundle).unwrap();
    let second = r#"{"ordinal":1,"kind":"wave_timeline","reference":"waves/se1.json","content":{"scheme":"domain_separated_sha256","format_version":1,"digest":"2121212121212121212121212121212121212121212121212121212121212121","canonical_len":403}},"#;
    let source = String::from_utf8(canonical_export()).unwrap().replacen(
        "{\"ordinal\":1,\"kind\":\"shader_engine_metadata\"",
        &format!("{second}{{\"ordinal\":2,\"kind\":\"shader_engine_metadata\""),
        1,
    );
    let partial = import_rocprofiler_sdk_decoded_att_v1(
        source.as_bytes(),
        &bundle,
        binding(),
        DecodedAttImportLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(partial.decoded_wave_references, vec![0]);
    assert_eq!(
        partial.raw_decode_relation.state,
        DecodedAttRawRelationV1::UnavailableIncompleteWaveReferenceCoverage
    );

    let prefix = source.strip_suffix("]}]}").unwrap();
    let complete_source = format!(
        "{prefix}]}},{{\"record_type\":\"gfxip\",\"source_reference_ordinal\":1,\"records\":[9]}}]}}"
    );
    let complete = import_rocprofiler_sdk_decoded_att_v1(
        complete_source.as_bytes(),
        &bundle,
        binding(),
        DecodedAttImportLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(complete.decoded_wave_references, vec![0, 1]);
    assert_eq!(complete.coverage.callback_count, 9);
    assert_eq!(
        complete.raw_decode_relation.state,
        DecodedAttRawRelationV1::ExternalDecoderDeclaredCompleteExactWaveInputs
    );
}
