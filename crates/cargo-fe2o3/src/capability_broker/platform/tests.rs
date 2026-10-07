use super::*;
include!("../../capability_broker_v4_tests.rs");
use fe2o3_artifact_transaction::{BuildAttempt, BuildInvocation};
use fe2o3_source_isa_observation::wire_v1::{
    MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1,
    MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1, SOURCE_ISA_COLLECTION_HEADER_BYTES_V1,
    SOURCE_ISA_COLLECTION_IDENTITY_BYTES_V1, SOURCE_ISA_COLLECTION_IDENTITY_DOMAIN_V1,
    SOURCE_ISA_COLLECTION_MAGIC_V1, SourceIsaObservationContextV1, SourceIsaObservationErrorCodeV1,
    SourceIsaObservationFrameV1, SourceIsaObservationOutcomeV1,
    SourceIsaObservationUnavailableReasonV1, source_isa_collection_encoded_length,
};

fn liveness() -> InvocationLiveness {
    InvocationLiveness {
        client: ProcessIdentityV1::observe(std::process::id()).unwrap(),
        started_at: Instant::now(),
        frame_timeout: Duration::from_millis(100),
        lifetime: Duration::from_secs(1),
    }
}

fn read_request(encoded: &[u8]) -> io::Result<BrokerInvocationRequest> {
    let (mut writer, reader) = UnixStream::pair().unwrap();
    writer.write_all(encoded).unwrap();
    writer.shutdown(Shutdown::Write).unwrap();
    read_invocation_request(liveness(), &reader)
}

fn attempt(generation: u64, session: [u8; 16], invocation: [u8; 32]) -> BuildAttempt {
    BuildAttempt::from_env_value(&format!(
        "{generation}:{}:{}",
        BuildSession::from_bytes(session),
        BuildInvocation::from_bytes(invocation)
    ))
    .unwrap()
}

fn frame_with_context(
    config: [u8; 32],
    unit: [u8; 32],
    attempt: BuildAttempt,
    outcome: SourceIsaObservationOutcomeV1,
) -> SourceIsaObservationFrameV1 {
    SourceIsaObservationFrameV1::new(
        SourceIsaObservationContextV1::new(
            config,
            unit,
            crate::source_isa_observation::inert_source_isa_attempt_v1(attempt).unwrap(),
            [0x33; 32],
        )
        .unwrap(),
        outcome,
    )
}

fn frame(unit: [u8; 32], outcome: SourceIsaObservationOutcomeV1) -> SourceIsaObservationFrameV1 {
    frame_with_context(
        [0x30; 32],
        unit,
        attempt(3, [0x31; 16], [0x32; 32]),
        outcome,
    )
}

fn collector(units: &[[u8; 32]]) -> SourceIsaObservationCollectorStateV1 {
    SourceIsaObservationCollectorStateV1::with_expected_context(
        [0x30; 32],
        BuildSession::from_bytes([0x31; 16]),
        units,
        ProductionSourceIsaObservationKindV1::Summary,
    )
    .unwrap()
}

fn characteristic_collector(unit: [u8; 32]) -> SourceIsaObservationCollectorStateV1 {
    SourceIsaObservationCollectorStateV1::with_expected_context(
        [0x30; 32],
        BuildSession::from_bytes([0x31; 16]),
        &[unit],
        ProductionSourceIsaObservationKindV1::Characteristic,
    )
    .unwrap()
}

#[test]
fn production_census_collector_requires_all_units_and_exact_payload_duplicates() {
    let row = crate::production_census_v91::test_census();
    let make = |units: &[[u8; 32]]| {
        SourceIsaObservationCollectorStateV1::with_expected_context(
            row.config,
            BuildSession::from_bytes(row.session),
            units,
            ProductionSourceIsaObservationKindV1::ProductionCensusV91,
        )
        .unwrap()
    };
    let observed = frame(
        row.unit,
        SourceIsaObservationOutcomeV1::Unavailable(
            SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV18,
        ),
    );
    let bytes = row.encode().unwrap();
    let mut collector = make(&[row.unit]);
    collector
        .insert(observed.clone(), Some(bytes.clone()))
        .unwrap();
    collector
        .insert(observed.clone(), Some(bytes.clone()))
        .unwrap();
    let completed = collector.finish();
    assert!(completed.summary.failure().is_none());
    assert_eq!(completed.census, vec![(row.unit, bytes.clone())]);
    assert!(completed.characteristic.is_none());
    let mut collector = make(&[row.unit, [0x41; 32]]);
    collector
        .insert(observed.clone(), Some(bytes.clone()))
        .unwrap();
    assert_eq!(
        collector.finish().summary.failure(),
        Some(SourceIsaObservationTransportFailureV1::MissingSelectedUnits)
    );
    let mut collector = make(&[row.unit]);
    collector.insert(observed.clone(), Some(bytes)).unwrap();
    let mut changed = row;
    changed.artifact[0] ^= 1;
    assert_eq!(
        collector.insert(observed, Some(changed.encode().unwrap())),
        Err(SourceIsaObservationTransportFailureV1::ConflictingDuplicate)
    );
    assert_eq!(
        collector.finish().summary.failure(),
        Some(SourceIsaObservationTransportFailureV1::ConflictingDuplicate)
    );
}

#[test]
fn production_census_transport_does_not_accept_summary_or_characteristic_payloads() {
    let row = crate::production_census_v91::test_census();
    let observed = frame(
        row.unit,
        SourceIsaObservationOutcomeV1::Unavailable(
            SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV18,
        ),
    );
    for body in [
        None,
        Some(b"{}".to_vec()),
        Some(vec![b' '; crate::production_census_v91::MAX_BYTES + 1]),
    ] {
        let mut collector = SourceIsaObservationCollectorStateV1::with_expected_context(
            row.config,
            BuildSession::from_bytes(row.session),
            &[row.unit],
            ProductionSourceIsaObservationKindV1::ProductionCensusV91,
        )
        .unwrap();
        assert_eq!(
            collector.insert(observed.clone(), body),
            Err(SourceIsaObservationTransportFailureV1::RejectedFrame)
        );
    }
    let mut legacy = collector(&[row.unit]);
    assert!(
        legacy
            .insert(observed, Some(row.encode().unwrap()))
            .is_err()
    );
}

fn observer(units: &[[u8; 32]]) -> BrokerSourceIsaObserverV1 {
    BrokerSourceIsaObserverV1 {
        config_identity: [0x30; 32],
        session: BuildSession::from_bytes([0x31; 16]),
        selected_units: units.to_vec(),
        kind: ProductionSourceIsaObservationKindV1::Summary,
        collector: Arc::new(Mutex::new(collector(units))),
    }
}

#[test]
fn configuration_presence_and_identity_bind_request_and_response_authentication() {
    let session = BuildSession::from_bytes([0x31; 16]);
    let challenge = [0x32; CHALLENGE_BYTES];
    let secret = [0x33; SECRET_BYTES];
    let binding = CapabilityBindingV3::new(
        CapabilityProfileV1::Ordinary,
        Some([0x34; CONFIG_ID_BYTES]),
        [0x35; 32],
        [0x36; 32],
        [0x37; 32],
    )
    .unwrap();
    let request = request_bytes(session, binding, challenge, &secret);
    let auth = |request: &[u8]| -> [u8; REQUEST_AUTH_BYTES] {
        request[REQUEST_BYTES - REQUEST_AUTH_BYTES..]
            .try_into()
            .unwrap()
    };
    let response = response_bytes(&secret, challenge, auth(&request));
    for identity in [
        None,
        Some([0; CONFIG_ID_BYTES]),
        Some([0x38; CONFIG_ID_BYTES]),
    ] {
        let changed = CapabilityBindingV3 {
            config_identity: identity,
            ..binding
        };
        assert_eq!(changed.config_identity(), identity);
        let changed_request = request_bytes(session, changed, challenge, &secret);
        assert_ne!(changed_request, request);
        assert_ne!(auth(&changed_request), auth(&request));
        assert_ne!(
            response_bytes(&secret, challenge, auth(&changed_request)),
            response
        );
    }
}

#[test]
fn invocation_request_dispatch_preserves_v1_and_accepts_exact_v2_width() {
    let v1 = BrokeredInvocationCapabilityRequestV1::Release.encode();
    assert!(matches!(
        read_request(&v1).unwrap(),
        BrokerInvocationRequest::V1(BrokeredInvocationCapabilityRequestV1::Release)
    ));

    let expected = BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
        [0x30; 32],
        [0x40; 32],
        attempt(3, [0x31; 16], [0x32; 32]),
    )
    .unwrap();
    assert!(matches!(
        read_request(&expected.encode()).unwrap(),
        BrokerInvocationRequest::V2(actual) if actual == expected
    ));
    let proof = proof_request();
    assert!(matches!(
        read_request(&proof.encode(&[0x51; 32])).unwrap(),
        BrokerInvocationRequest::PrepareProof(actual) if actual.attempt == proof.attempt
    ));
}

fn proof_request() -> ProofPreparationRequest {
    ProofPreparationRequest {
        attempt: attempt(3, [0x31; 16], [0x32; 32]),
        challenge: [0x50; 32],
        authentication: [0; 32],
    }
}

#[test]
fn ordinary_invocation_cannot_prepare_or_retry_compiler_proof() {
    let (client, mut server) = UnixStream::pair().unwrap();
    server.set_nonblocking(true).unwrap();
    let mut authority = BrokeredInvocationAuthorityV1 {
        stream: client,
        profile_account: None,
        proof: None,
    };
    for _ in 0..2 {
        assert!(
            authority
                .prepare_compiler_proof(proof_request().attempt)
                .is_err()
        );
        assert_eq!(
            server.read(&mut [0_u8; 1]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
}

#[test]
fn proof_preparation_authenticates_exact_attempt_session_and_challenge() {
    let secret = [0x51; 32];
    let session = BuildSession::from_bytes([0x31; 16]);
    let encoded = proof_request().encode(&secret);
    let decoded = ProofPreparationRequest::decode(&encoded).unwrap();
    decoded.require_authenticated(&secret, session).unwrap();
    assert!(decoded.require_authenticated(&[0x52; 32], session).is_err());
    assert!(
        decoded
            .require_authenticated(&secret, BuildSession::DIRECT)
            .is_err()
    );
    for offset in 8..PROOF_PREPARE_BYTES {
        let mut changed = encoded;
        changed[offset] ^= 1;
        assert!(
            ProofPreparationRequest::decode(&changed)
                .and_then(|request| request.require_authenticated(&secret, session))
                .is_err(),
            "altered proof preparation accepted at byte {offset}"
        );
    }
    let mut direct = proof_request();
    direct.attempt = attempt(3, [0; 16], [0; 32]);
    let decoded = ProofPreparationRequest::decode(&direct.encode(&secret)).unwrap();
    assert!(
        decoded
            .require_authenticated(&secret, BuildSession::DIRECT)
            .is_err()
    );
}

#[test]
fn proof_preparation_accepts_maximum_generation_without_overflow() {
    let mut request = proof_request();
    request.attempt = attempt(u64::MAX, [0x31; 16], [0x32; 32]);
    let decoded = ProofPreparationRequest::decode(&request.encode(&[0x51; 32])).unwrap();
    assert_eq!(decoded.attempt, request.attempt);
}

#[test]
fn proof_preparation_rejects_zero_challenge_and_noncanonical_padding() {
    let mut request = proof_request();
    request.challenge = [0; 32];
    assert!(ProofPreparationRequest::decode(&request.encode(&[0x51; 32])).is_err());
    let mut encoded = proof_request().encode(&[0x51; 32]);
    encoded[159] = 1;
    assert!(ProofPreparationRequest::decode(&encoded).is_err());
}

#[test]
fn proof_response_binds_original_request_and_new_session() {
    let secret = [0x51; 32];
    let mut request = proof_request();
    let expected = request.response(&secret, [0x53; 32]);
    assert_ne!(expected, request.response(&secret, [0x54; 32]));
    assert_ne!(expected, request.response(&[0x52; 32], [0x53; 32]));
    request.challenge[0] ^= 1;
    assert_ne!(expected, request.response(&secret, [0x53; 32]));
    request.challenge[0] ^= 1;
    request.attempt = attempt(4, [0x31; 16], [0x32; 32]);
    assert_ne!(expected, request.response(&secret, [0x53; 32]));
}

#[test]
fn invocation_request_dispatch_rejects_unknown_and_every_truncated_width() {
    assert!(read_request(b"UNKNOWN!").is_err());
    let requests = [
        proof_request().encode(&[0x51; 32]).to_vec(),
        BrokeredInvocationCapabilityRequestV1::Release
            .encode()
            .to_vec(),
        BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
            [0x30; 32],
            [0x40; 32],
            attempt(3, [0x31; 16], [0x32; 32]),
        )
        .unwrap()
        .encode()
        .to_vec(),
    ];
    for request in requests {
        for length in 0..request.len() {
            assert!(
                read_request(&request[..length]).is_err(),
                "truncated request length {length} was accepted"
            );
        }
    }
}

#[test]
fn v1_release_waits_for_the_frozen_prepared_ack_and_rejects_substitution() {
    let (client, mut server) = UnixStream::pair().unwrap();
    let (result_sender, result_receiver) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        result_sender
            .send(
                BrokeredInvocationAuthorityV1 {
                    stream: client,
                    profile_account: None,
                    proof: None,
                }
                .release(),
            )
            .unwrap();
    });
    let mut request = [0; BROKERED_INVOCATION_REQUEST_BYTES_V1];
    server.read_exact(&mut request).unwrap();
    assert_eq!(
        BrokeredInvocationCapabilityRequestV1::decode(&request),
        Ok(BrokeredInvocationCapabilityRequestV1::Release)
    );
    assert!(matches!(
        result_receiver.recv_timeout(Duration::from_millis(25)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    ));
    server.write_all(BROKERED_INVOCATION_PREPARED_V1).unwrap();
    assert!(
        result_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .is_ok()
    );
    worker.join().unwrap();

    for response in [Some([0xa5; 16]), None] {
        let (client, mut server) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            let mut request = [0; BROKERED_INVOCATION_REQUEST_BYTES_V1];
            server.read_exact(&mut request).unwrap();
            if let Some(response) = response {
                server.write_all(&response).unwrap();
            }
        });
        assert!(
            BrokeredInvocationAuthorityV1 {
                stream: client,
                profile_account: None,
                proof: None,
            }
            .release()
            .is_err()
        );
        worker.join().unwrap();
    }
}

#[test]
fn observer_frame_requires_exact_request_config_unit_and_attempt() {
    let broker_session = BuildSession::from_bytes([0x31; 16]);
    let exact_attempt = attempt(3, [0x31; 16], [0x32; 32]);
    let request = BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
        [0x30; 32],
        [0x40; 32],
        exact_attempt,
    )
    .unwrap();
    let selected_units = vec![[0x40; 32], [0x41; 32]];
    let observer = observer(&selected_units);
    assert!(observer.accepts(request));
    assert!(
        observer.accepts(
            BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                [0x30; 32],
                [0x41; 32],
                exact_attempt,
            )
            .unwrap()
        )
    );
    let outcome = SourceIsaObservationOutcomeV1::Unavailable(
        SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV9,
    );
    let exact_frame = frame_with_context([0x30; 32], [0x40; 32], exact_attempt, outcome);
    assert!(
        validate_source_isa_observer_frame_binding(broker_session, request, &exact_frame,).is_ok()
    );

    for substituted in [
        frame_with_context([0x35; 32], [0x40; 32], exact_attempt, outcome),
        // A different configured unit must not substitute for the request's exact unit.
        frame_with_context([0x30; 32], [0x41; 32], exact_attempt, outcome),
        frame_with_context(
            [0x30; 32],
            [0x40; 32],
            attempt(4, [0x31; 16], [0x32; 32]),
            outcome,
        ),
        frame_with_context(
            [0x30; 32],
            [0x40; 32],
            attempt(3, [0x31; 16], [0x36; 32]),
            outcome,
        ),
    ] {
        assert!(
            validate_source_isa_observer_frame_binding(broker_session, request, &substituted,)
                .is_err()
        );
    }

    for substituted_attempt in [
        attempt(4, [0x31; 16], [0x32; 32]),
        attempt(3, [0x31; 16], [0x36; 32]),
    ] {
        let substituted_request =
            BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                [0x30; 32],
                [0x40; 32],
                substituted_attempt,
            )
            .unwrap();
        assert!(
            validate_source_isa_observer_frame_binding(
                broker_session,
                substituted_request,
                &exact_frame,
            )
            .is_err()
        );
    }

    let wrong_session_attempt = attempt(3, [0x37; 16], [0x32; 32]);
    let wrong_session_request =
        BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
            [0x30; 32],
            [0x40; 32],
            wrong_session_attempt,
        )
        .unwrap();
    assert!(
        validate_source_isa_observer_frame_binding(
            broker_session,
            wrong_session_request,
            &frame_with_context([0x30; 32], [0x40; 32], wrong_session_attempt, outcome,),
        )
        .is_err()
    );
}

#[test]
fn observer_sink_is_returned_only_after_exact_server_preparation() {
    fn release(
        observer: BrokerSourceIsaObserverV1,
        config: [u8; 32],
        unit: [u8; 32],
        request_attempt: BuildAttempt,
    ) -> Result<SourceIsaObservationSinkV1, String> {
        let (client, server) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            let BrokerInvocationRequest::V2(request) =
                read_invocation_request(liveness(), &server).unwrap()
            else {
                panic!("expected V2 observer request");
            };
            prepare_source_isa_observer_request(
                &observer,
                BuildSession::from_bytes([0x31; 16]),
                &server,
                request,
            )
        });
        let result = BrokeredInvocationAuthorityV1 {
            stream: client,
            profile_account: None,
            proof: None,
        }
        .release_with_source_isa_observer(config, unit, request_attempt);
        let server_result = worker.join().unwrap();
        assert_eq!(result.is_ok(), server_result.is_ok());
        result
    }

    let units = [[0x40; 32], [0x41; 32]];
    let exact_attempt = attempt(3, [0x31; 16], [0x32; 32]);

    let (client, mut server) = UnixStream::pair().unwrap();
    let (result_sender, result_receiver) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = BrokeredInvocationAuthorityV1 {
            stream: client,
            profile_account: None,
            proof: None,
        }
        .release_with_source_isa_observer([0x30; 32], [0x40; 32], exact_attempt)
        .map(drop);
        result_sender.send(result).unwrap();
    });
    let mut request = [0; BROKERED_INVOCATION_REQUEST_BYTES_V2];
    server.read_exact(&mut request).unwrap();
    assert!(BrokeredInvocationCapabilityRequestV2::decode(&request).is_ok());
    assert!(matches!(
        result_receiver.recv_timeout(Duration::from_millis(25)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    ));
    server.write_all(BROKERED_SOURCE_ISA_PREPARED_V1).unwrap();
    assert!(
        result_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .is_ok()
    );
    worker.join().unwrap();

    assert!(release(observer(&units), [0x30; 32], units[0], exact_attempt).is_ok());
    assert!(release(observer(&units), [0x35; 32], units[0], exact_attempt).is_err());
    assert!(release(observer(&units), [0x30; 32], [0x45; 32], exact_attempt).is_err());
    assert!(
        release(
            observer(&units),
            [0x30; 32],
            units[0],
            attempt(3, [0x37; 16], [0x32; 32]),
        )
        .is_err()
    );

    let (client, mut server) = UnixStream::pair().unwrap();
    let worker = std::thread::spawn(move || {
        let mut request = [0; BROKERED_INVOCATION_REQUEST_BYTES_V2];
        server.read_exact(&mut request).unwrap();
        server.write_all(&[0xa5; 16]).unwrap();
    });
    assert!(
        BrokeredInvocationAuthorityV1 {
            stream: client,
            profile_account: None,
            proof: None,
        }
        .release_with_source_isa_observer([0x30; 32], units[0], exact_attempt,)
        .is_err()
    );
    worker.join().unwrap();

    let mut direct = BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
        [0x30; 32],
        units[0],
        exact_attempt,
    )
    .unwrap()
    .encode();
    direct[88..136].fill(0);
    assert!(read_request(&direct).is_err());
    assert!(
        BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
            [0x30; 32],
            units[0],
            attempt(3, [0; 16], [0; 32]),
        )
        .is_err()
    );
}

#[test]
fn collector_deduplicates_exact_recovery_and_preserves_partial_failure() {
    let units = [[0x40; 32], [0x41; 32]];
    let mut collector = collector(&units);
    let accepted = frame(
        units[0],
        SourceIsaObservationOutcomeV1::Unavailable(
            SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV9,
        ),
    );
    assert!(collector.insert(accepted.clone(), None).is_ok());
    assert!(collector.insert(accepted, None).is_ok());
    collector.fail(SourceIsaObservationTransportFailureV1::RejectedFrame);
    let conflicting = frame(
        units[0],
        SourceIsaObservationOutcomeV1::Error(SourceIsaObservationErrorCodeV1::ResourceLimit),
    );
    assert_eq!(
        collector.insert(conflicting, None),
        Err(SourceIsaObservationTransportFailureV1::ConflictingDuplicate)
    );
    collector
        .insert(
            frame(
                units[1],
                SourceIsaObservationOutcomeV1::Unavailable(
                    SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
                ),
            ),
            None,
        )
        .unwrap();
    collector.fail(SourceIsaObservationTransportFailureV1::AggregateByteBound);
    let collection = collector.finish().summary;
    assert_eq!(collection.frames().len(), 2);
    assert!(collection.missing_units().is_empty());
    assert_eq!(
        collection.failure(),
        Some(SourceIsaObservationTransportFailureV1::RejectedFrame)
    );
    let encoded = collection.encode_canonical().unwrap();
    assert_eq!(&encoded[..8], SOURCE_ISA_COLLECTION_MAGIC_V1);
    assert_eq!(&encoded[32..64], &[0x30; 32]);
    assert_eq!(&encoded[64..80], &[0x31; 16]);
    assert_eq!(u32::from_le_bytes(encoded[16..20].try_into().unwrap()), 2);
    assert_eq!(
        u16::from_le_bytes(encoded[24..26].try_into().unwrap()),
        SourceIsaObservationTransportFailureV1::RejectedFrame.code()
    );
    assert!(!collection.grants_compiler_authority());
    assert!(!collection.grants_publication_authority());
    assert!(!collection.grants_runtime_authority());
    assert_eq!(
        SourceIsaObservationCollectionV1::decode_canonical(&encoded),
        Ok(collection)
    );
}

#[test]
fn characteristic_collector_requires_byte_identical_duplicate_recovery() {
    let unit = [0x40; 32];
    let accepted = frame(
        unit,
        SourceIsaObservationOutcomeV1::Unavailable(
            SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV9,
        ),
    );
    let mut collector = characteristic_collector(unit);
    assert!(
        collector
            .insert(accepted.clone(), Some(vec![0x51, 0x52]))
            .is_ok()
    );
    assert!(
        collector
            .insert(accepted.clone(), Some(vec![0x51, 0x52]))
            .is_ok()
    );
    assert_eq!(
        collector.insert(accepted, Some(vec![0x51, 0x53])),
        Err(SourceIsaObservationTransportFailureV1::ConflictingDuplicate)
    );
    let completed = collector.finish();
    assert_eq!(completed.summary.frames().len(), 1);
    assert_eq!(completed.characteristic, Some((unit, vec![0x51, 0x52])));
    assert_eq!(
        completed.summary.failure(),
        Some(SourceIsaObservationTransportFailureV1::ConflictingDuplicate)
    );
}

#[test]
fn collector_deduplicates_exact_recovery_at_the_unit_bound() {
    let units = (0..MAX_SOURCE_ISA_OBSERVATION_UNITS_V1)
        .map(|index| {
            let mut unit = [0x40; 32];
            unit[..8].copy_from_slice(&(index as u64).to_le_bytes());
            unit
        })
        .collect::<Vec<_>>();
    let mut collector = collector(&units);
    for &unit in &units {
        collector
            .insert(
                frame(
                    unit,
                    SourceIsaObservationOutcomeV1::Unavailable(
                        SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
                    ),
                ),
                None,
            )
            .unwrap();
    }

    assert!(
        collector
            .insert(
                frame(
                    units[MAX_SOURCE_ISA_OBSERVATION_UNITS_V1 - 1],
                    SourceIsaObservationOutcomeV1::Unavailable(
                        SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
                    ),
                ),
                None
            )
            .is_ok()
    );
    let collection = collector.finish().summary;
    assert_eq!(
        collection.frames().len(),
        MAX_SOURCE_ISA_OBSERVATION_UNITS_V1
    );
    assert!(collection.missing_units().is_empty());
    assert_eq!(collection.failure(), None);
}

#[test]
fn collector_reports_missing_selected_units_without_discarding_frames() {
    let units = [[0x40; 32], [0x41; 32]];
    let mut collector = collector(&units);
    collector
        .insert(
            frame(
                units[1],
                SourceIsaObservationOutcomeV1::Unavailable(
                    SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
                ),
            ),
            None,
        )
        .unwrap();
    let collection = collector.finish().summary;
    assert_eq!(collection.frames().len(), 1);
    assert_eq!(collection.missing_units(), &[units[0]]);
    assert_eq!(
        collection.failure(),
        Some(SourceIsaObservationTransportFailureV1::MissingSelectedUnits)
    );
}

#[test]
fn all_missing_collection_retains_exact_config_and_session() {
    let collection = collector(&[[0x40; 32], [0x41; 32]]).finish().summary;
    assert_eq!(collection.config_identity(), [0x30; 32]);
    assert_eq!(
        collection.session(),
        crate::source_isa_observation::inert_source_isa_session_v1(BuildSession::from_bytes(
            [0x31; 16]
        ))
    );
    assert_eq!(collection.frames().len(), 0);
    assert_eq!(collection.missing_units(), &[[0x40; 32], [0x41; 32]]);
    assert_eq!(
        SourceIsaObservationCollectionV1::decode_canonical(&collection.encode_canonical().unwrap()),
        Ok(collection)
    );
}

#[test]
fn canonical_collection_length_is_bounded_and_fallible() {
    assert_eq!(
        source_isa_collection_encoded_length(MAX_SOURCE_ISA_OBSERVATION_UNITS_V1, 0,),
        Ok(MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1)
    );
    assert_eq!(MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1, 696_432);
    assert!(source_isa_collection_encoded_length(MAX_SOURCE_ISA_OBSERVATION_UNITS_V1, 1,).is_err());
    assert!(
        source_isa_collection_encoded_length(MAX_SOURCE_ISA_OBSERVATION_UNITS_V1 + 1, 0,).is_err()
    );
    assert!(source_isa_collection_encoded_length(usize::MAX, usize::MAX).is_err());
    assert_eq!(
        MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1,
        MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1 * 2
    );
    assert_eq!(
        MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1,
        1_392_864
    );
}

#[test]
fn canonical_collection_decoder_rejects_hostile_framing_and_payloads() {
    fn rehash(encoded: &mut [u8]) {
        let identity_start = encoded.len() - SOURCE_ISA_COLLECTION_IDENTITY_BYTES_V1;
        let mut digest = Sha256::new();
        digest.update(SOURCE_ISA_COLLECTION_IDENTITY_DOMAIN_V1);
        digest.update(&encoded[..identity_start]);
        encoded[identity_start..].copy_from_slice(&digest.finalize());
    }

    let units = [[0x40; 32], [0x41; 32]];
    let mut collector = collector(&units);
    collector
        .insert(
            frame(
                units[0],
                SourceIsaObservationOutcomeV1::Unavailable(
                    SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
                ),
            ),
            None,
        )
        .unwrap();
    let encoded = collector.finish().summary.encode_canonical().unwrap();

    let mut truncated = encoded.clone();
    truncated.pop();
    assert!(SourceIsaObservationCollectionV1::decode_canonical(&truncated).is_err());
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(SourceIsaObservationCollectionV1::decode_canonical(&trailing).is_err());
    let oversized = vec![0; MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1 + 1];
    assert!(SourceIsaObservationCollectionV1::decode_canonical(&oversized).is_err());

    let mut over_combined_count = encoded.clone();
    over_combined_count[16..20]
        .copy_from_slice(&(MAX_SOURCE_ISA_OBSERVATION_UNITS_V1 as u32).to_le_bytes());
    over_combined_count[20..24].copy_from_slice(&1_u32.to_le_bytes());
    rehash(&mut over_combined_count);
    assert!(SourceIsaObservationCollectionV1::decode_canonical(&over_combined_count).is_err());

    for substituted in [
        frame_with_context(
            [0x35; 32],
            units[0],
            attempt(3, [0x31; 16], [0x32; 32]),
            SourceIsaObservationOutcomeV1::Unavailable(
                SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
            ),
        ),
        frame_with_context(
            [0x30; 32],
            units[0],
            attempt(3, [0x36; 16], [0x32; 32]),
            SourceIsaObservationOutcomeV1::Unavailable(
                SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
            ),
        ),
    ] {
        let mut mixed_context = encoded.clone();
        mixed_context[SOURCE_ISA_COLLECTION_HEADER_BYTES_V1
            ..SOURCE_ISA_COLLECTION_HEADER_BYTES_V1 + SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1]
            .copy_from_slice(&substituted.encode());
        rehash(&mut mixed_context);
        assert!(SourceIsaObservationCollectionV1::decode_canonical(&mixed_context).is_err());
    }

    for offset in [0, 8, 12, 16, 20, 24, 28, 32, 64, 80, encoded.len() - 1] {
        let mut changed = encoded.clone();
        changed[offset] ^= 1;
        assert!(
            SourceIsaObservationCollectionV1::decode_canonical(&changed).is_err(),
            "hostile byte {offset} was accepted"
        );
    }

    let mut unknown_failure = encoded.clone();
    unknown_failure[24..26].copy_from_slice(&99_u16.to_le_bytes());
    rehash(&mut unknown_failure);
    assert!(SourceIsaObservationCollectionV1::decode_canonical(&unknown_failure).is_err());

    let mut nonzero_truth = encoded;
    nonzero_truth[28] = 1;
    rehash(&mut nonzero_truth);
    assert!(SourceIsaObservationCollectionV1::decode_canonical(&nonzero_truth).is_err());
}
