use fe2o3_debug_protocol::*;

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).expect("nonzero identity")
}

#[test]
fn strict_request_codec_rejects_duplicates_unknowns_and_native_aliases() {
    for line in [
        br#"{"operation":"get_session","schema":"fe2o3-rocgdb-mi-request-v3","request_id":1,"request_id":2}
"#
        .as_slice(),
        br#"{"operation":"get_session","schema":"fe2o3-rocgdb-mi-request-v3","request_id":1,"extra":true}
"#,
        br#"{"operation":"admit_allocation","schema":"fe2o3-rocgdb-mi-request-v3","request_id":1,"allocation":{"ordinal":1,"generation":1},"base":"0x01","byte_len":4,"space":"global"}
"#,
        br#"{"operation":"admit_allocation","schema":"fe2o3-rocgdb-mi-request-v3","request_id":1,"allocation":{"ordinal":1,"generation":1},"base":"0xAB","byte_len":4,"space":"global"}
"#,
        b"{}\r\n",
    ] {
        assert!(
            decode_rocgdb_mi_cli_request_line_v3(line).is_err(),
            "accepted hostile request: {line:?}"
        );
    }
    let oversized = vec![b'x'; MAX_ROCGDB_MI_CLI_REQUEST_BYTES_V3 + 1];
    assert_eq!(
        decode_rocgdb_mi_cli_request_line_v3(&oversized),
        Err(RocgdbMiCliCodecErrorV3::LineTooLarge)
    );
}

#[test]
fn control_request_binds_outer_id_and_forbids_second_bootstrap() {
    let authorization = RocgdbMiControlAuthorizationV3 {
        authorization_identity: identity(1),
        expected_revision: 2,
    };
    let mismatched = serde_json::to_vec(&RocgdbMiCliRequestV3::Control {
        schema: RocgdbMiCliRequestSchemaV3::V3,
        request_id: 4,
        control: RocgdbMiControlRequestV3::Pause {
            request_id: 5,
            authorization,
        },
    })
    .unwrap();
    let mut line = mismatched;
    line.push(b'\n');
    assert!(decode_rocgdb_mi_cli_request_line_v3(&line).is_err());

    let relaunch = RocgdbMiCliRequestV3::Control {
        schema: RocgdbMiCliRequestSchemaV3::V3,
        request_id: 6,
        control: RocgdbMiControlRequestV3::Launch {
            request_id: 6,
            authorization,
        },
    };
    assert_eq!(
        relaunch.validate(),
        Err(RocgdbMiCliValidationErrorV3::InvalidControl)
    );
}

#[test]
fn capability_response_keeps_generic_hierarchy_explicitly_unsupported() {
    let capabilities = RocgdbMiCliCapabilitiesV3 {
        mi: RocgdbMiCapabilitiesV3 {
            capabilities: vec![RocgdbMiCapabilityV3 {
                name: RocgdbMiCapabilityNameV3::Launch,
                availability: LiveGpuCapabilityAvailabilityV3::Available,
                unavailable_reason: None,
                authorization: RocgdbMiAuthorizationRequirementV3::Required,
            }],
        },
        generic_stopped_scopes: [
            LiveGpuCapabilityNameV3::StoppedDispatch,
            LiveGpuCapabilityNameV3::StoppedWorkgroups,
            LiveGpuCapabilityNameV3::StoppedWaves,
            LiveGpuCapabilityNameV3::StoppedLanes,
        ]
        .into_iter()
        .map(|name| LiveGpuCapabilityV3 {
            backend: LiveGpuBackendV3::RocgdbMi,
            name,
            availability: LiveGpuCapabilityAvailabilityV3::Unavailable,
            unavailable_reason: Some(LiveGpuUnavailableReasonV3::Unsupported),
        })
        .collect(),
    };
    capabilities.validate().expect("explicit capability split");

    let mut missing = capabilities;
    missing.generic_stopped_scopes.pop();
    assert_eq!(
        missing.validate(),
        Err(RocgdbMiCliValidationErrorV3::InvalidResult)
    );
}

#[test]
fn valid_large_memory_observation_is_stopped_by_the_response_bound() {
    let response = RocgdbMiCliResponseV3::Ok {
        schema: RocgdbMiCliResponseSchemaV3::V3,
        request_id: 1,
        revision: 1,
        result: Box::new(RocgdbMiCliResultV3::Memory {
            memory: RocgdbMiMemoryReadResultV3 {
                request_id: 1,
                revision: 1,
                memory: LiveGpuMemoryReadV3 {
                    allocation: AllocationIdentityV1 {
                        ordinal: 1,
                        generation: 1,
                    },
                    byte_offset: 0,
                    requested_bytes: MAX_ROCGDB_MI_MEMORY_BYTES_V3,
                    returned_bytes: MAX_ROCGDB_MI_MEMORY_BYTES_V3,
                    value: LiveGpuAvailabilityV3::Available {
                        value: LiveGpuMemoryBytesV3 {
                            space: LiveGpuMemorySpaceV3::Global,
                            bytes: "00".repeat(MAX_ROCGDB_MI_MEMORY_BYTES_V3 as usize),
                        },
                        truth: LiveGpuTruthV3 {
                            origin: LiveGpuTruthOriginV3::Observed,
                            evidence: vec![LiveGpuEvidenceRefV3 {
                                kind: LiveGpuEvidenceKindV3::RuntimeObservation,
                                identity: identity(2),
                            }],
                        },
                    },
                },
            },
        }),
    };
    assert_eq!(
        encode_rocgdb_mi_cli_response_line_v3(&response),
        Err(RocgdbMiCliCodecErrorV3::ResponseTooLarge)
    );
}
