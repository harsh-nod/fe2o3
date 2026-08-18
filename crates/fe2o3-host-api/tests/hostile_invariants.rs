use fe2o3_host_api::*;

fn digest(value: u64) -> HostDigestV1 {
    let mut bytes = [0u8; IDENTITY_BYTES_V1];
    bytes[..8].copy_from_slice(&value.to_be_bytes());
    bytes[8..16].copy_from_slice(&value.wrapping_mul(17).to_be_bytes());
    HostDigestV1::from_untrusted_bytes(bytes)
}

macro_rules! identity {
    ($type:ident, $value:expr) => {
        $type::from_untrusted_digest(digest($value))
    };
}

fn scope(value: u64) -> FlowScopeIdV1 {
    identity!(FlowScopeIdV1, value)
}

fn context(scope_id: FlowScopeIdV1, value: u64) -> OperationContextV1 {
    OperationContextV1::new(scope_id, identity!(OperationIdV1, value), 1, None, vec![]).unwrap()
}

fn payload(value: u64, byte_len: u64) -> PayloadDescriptorV1 {
    PayloadDescriptorV1::new(
        identity!(PayloadIdV1, value),
        identity!(PayloadFormatIdV1, value + 1),
        byte_len,
    )
    .unwrap()
}

fn error_diagnostic(value: u64) -> HostDiagnosticV1 {
    HostDiagnosticV1::new(
        value as u32 + 1,
        DiagnosticSeverityV1::Error,
        Some(digest(value)),
        DiagnosticMessageV1::new(format!("failure-{value}")).unwrap(),
    )
    .unwrap()
}

fn compile_success(
    scope_id: FlowScopeIdV1,
    seed: u64,
) -> (CompileRequestV1, CompileResultV1, PayloadDescriptorV1) {
    let candidate = payload(seed + 20, 4_096);
    let request = CompileRequestV1::new(
        identity!(CompileRequestIdV1, seed),
        context(scope_id, seed + 1),
        payload(seed + 10, 2_048),
        identity!(CompilerProfileIdV1, seed + 2),
        identity!(TargetProfileIdV1, seed + 3),
        identity!(CompileConfigurationIdV1, seed + 4),
        8_192,
    )
    .unwrap();
    let result = CompileResultV1::new(
        identity!(CompileResultIdV1, seed + 5),
        &request,
        CompileOutcomeV1::Candidate(candidate),
        vec![],
    )
    .unwrap();
    (request, result, candidate)
}

fn accepted_load(scope_id: FlowScopeIdV1, seed: u64) -> LoadResultV1 {
    let (_, compile_result, candidate) = compile_success(scope_id, seed);
    let admit_request = AdmitRequestV1::new(
        identity!(AdmitRequestIdV1, seed + 30),
        context(scope_id, seed + 31),
        &compile_result,
        candidate,
        identity!(AdmissionPolicyIdV1, seed + 32),
        vec![identity!(ClaimIdV1, seed + 33)],
    )
    .unwrap();
    let admit_result = AdmitResultV1::new(
        identity!(AdmitResultIdV1, seed + 34),
        &admit_request,
        AdmitOutcomeV1::Accepted {
            assessment_identity: identity!(AdmissionAssessmentIdV1, seed + 35),
        },
        vec![],
    )
    .unwrap();
    let load_request = LoadRequestV1::new(
        identity!(LoadRequestIdV1, seed + 40),
        context(scope_id, seed + 41),
        &admit_result,
        candidate.identity(),
        identity!(LoaderProfileIdV1, seed + 42),
        identity!(RuntimeContextIdV1, seed + 43),
    )
    .unwrap();
    LoadResultV1::new(
        identity!(LoadResultIdV1, seed + 44),
        &load_request,
        LoadOutcomeV1::Loaded {
            loaded_object_identity: identity!(LoadedObjectIdV1, seed + 45),
            load_generation: seed + 1,
        },
        vec![],
    )
    .unwrap()
}

fn submitted_dispatch(
    scope_id: FlowScopeIdV1,
    seed: u64,
    signal_value: u64,
    kind: DispatchKindV1,
) -> (DispatchRequestV1, DispatchResultV1) {
    let load = accepted_load(scope_id, seed);
    let request = DispatchRequestV1::new(
        identity!(DispatchRequestIdV1, seed + 50),
        context(scope_id, seed + 51),
        &load,
        identity!(EntryPointIdV1, seed + 52),
        identity!(DispatchContractIdV1, seed + 53),
        identity!(ArgumentSetIdV1, seed + 54),
        kind,
        vec![
            ResourceBindingV1::new(
                0,
                identity!(ResourceIdV1, seed + 55),
                AccessModeV1::Read,
                0,
                64,
            )
            .unwrap(),
            ResourceBindingV1::new(
                1,
                identity!(ResourceIdV1, seed + 56),
                AccessModeV1::Write,
                64,
                64,
            )
            .unwrap(),
        ],
        vec![],
    )
    .unwrap();
    let result = DispatchResultV1::new(
        identity!(DispatchResultIdV1, seed + 57),
        &request,
        DispatchOutcomeV1::Submitted {
            submission_identity: identity!(DispatchSubmissionIdV1, seed + 58),
            completion_signal_identity: identity!(CompletionSignalIdV1, signal_value),
        },
        vec![],
    )
    .unwrap();
    (request, result)
}

#[test]
fn identity_domains_are_unique_and_preimages_exclude_declared_identity() {
    let domains: [&[u8]; 20] = [
        CompileRequestIdV1::DOMAIN_V1,
        CompileResultIdV1::DOMAIN_V1,
        CompileStateIdV1::DOMAIN_V1,
        CompileEventIdV1::DOMAIN_V1,
        AdmitRequestIdV1::DOMAIN_V1,
        AdmitResultIdV1::DOMAIN_V1,
        AdmitStateIdV1::DOMAIN_V1,
        AdmitEventIdV1::DOMAIN_V1,
        LoadRequestIdV1::DOMAIN_V1,
        LoadResultIdV1::DOMAIN_V1,
        LoadStateIdV1::DOMAIN_V1,
        LoadEventIdV1::DOMAIN_V1,
        DispatchRequestIdV1::DOMAIN_V1,
        DispatchResultIdV1::DOMAIN_V1,
        DispatchStateIdV1::DOMAIN_V1,
        DispatchEventIdV1::DOMAIN_V1,
        WaitRequestIdV1::DOMAIN_V1,
        WaitResultIdV1::DOMAIN_V1,
        WaitStateIdV1::DOMAIN_V1,
        WaitEventIdV1::DOMAIN_V1,
    ];
    for (index, domain) in domains.iter().enumerate() {
        assert!(domain.starts_with(b"fe2o3.host."));
        assert!(domains[..index].iter().all(|earlier| earlier != domain));
    }

    let scope_id = scope(1);
    let first = CompileRequestV1::new(
        identity!(CompileRequestIdV1, 2),
        context(scope_id, 3),
        payload(4, 16),
        identity!(CompilerProfileIdV1, 5),
        identity!(TargetProfileIdV1, 6),
        identity!(CompileConfigurationIdV1, 7),
        32,
    )
    .unwrap();
    let restamped = CompileRequestV1::new(
        identity!(CompileRequestIdV1, 99),
        first.context().clone(),
        first.input(),
        first.compiler_profile_identity(),
        first.target_profile_identity(),
        first.configuration_identity(),
        first.maximum_output_bytes(),
    )
    .unwrap();
    assert_eq!(
        first.encode_identity_preimage(),
        restamped.encode_identity_preimage()
    );

    let mutated = CompileRequestV1::new(
        identity!(CompileRequestIdV1, 2),
        first.context().clone(),
        first.input(),
        first.compiler_profile_identity(),
        first.target_profile_identity(),
        first.configuration_identity(),
        31,
    )
    .unwrap();
    assert_ne!(
        first.encode_identity_preimage(),
        mutated.encode_identity_preimage()
    );
    assert!(first.encode_identity_preimage().starts_with(b"F2HOSTP1"));

    let same = digest(200);
    assert_ne!(
        HostRequestIdentityV1::Compile(CompileRequestIdV1::from_untrusted_digest(same)),
        HostRequestIdentityV1::Admit(AdmitRequestIdV1::from_untrusted_digest(same))
    );
}

#[test]
fn scalar_and_collection_bounds_fail_closed() {
    assert!(matches!(
        PayloadDescriptorV1::new(
            identity!(PayloadIdV1, 1),
            identity!(PayloadFormatIdV1, 2),
            0,
        ),
        Err(HostContractErrorV1::Empty {
            field: ContractFieldV1::PayloadBytes
        })
    ));
    assert!(matches!(
        PayloadDescriptorV1::new(
            identity!(PayloadIdV1, 1),
            identity!(PayloadFormatIdV1, 2),
            MAX_PAYLOAD_BYTES_V1 + 1,
        ),
        Err(HostContractErrorV1::LimitExceeded {
            field: ContractFieldV1::PayloadBytes,
            ..
        })
    ));
    assert!(
        OperationContextV1::new(scope(3), identity!(OperationIdV1, 4), 0, None, vec![],).is_err()
    );
    let operation = identity!(OperationIdV1, 5);
    assert!(OperationContextV1::new(scope(3), operation, 1, Some(operation), vec![]).is_err());

    let event = HostEventIdentityV1::Compile(identity!(CompileEventIdV1, 10));
    assert!(matches!(
        OperationContextV1::new(scope(3), operation, 1, None, vec![event, event]),
        Err(HostContractErrorV1::Duplicate {
            field: ContractFieldV1::CausalEvents
        })
    ));
    let causal_events = (0..=MAX_CAUSAL_EVENTS_V1)
        .map(|index| {
            HostEventIdentityV1::Compile(identity!(CompileEventIdV1, 1_000 + index as u64))
        })
        .collect();
    assert!(matches!(
        OperationContextV1::new(scope(3), operation, 1, None, causal_events),
        Err(HostContractErrorV1::TooManyItems {
            field: ContractFieldV1::CausalEvents,
            ..
        })
    ));

    assert!(DiagnosticMessageV1::new(String::new()).is_err());
    assert!(DiagnosticMessageV1::new("bad\0message".into()).is_err());
    assert!(DiagnosticMessageV1::new("x".repeat(MAX_DIAGNOSTIC_MESSAGE_BYTES_V1 + 1)).is_err());
    assert!(
        HostDiagnosticV1::new(
            0,
            DiagnosticSeverityV1::Error,
            None,
            DiagnosticMessageV1::new("error".into()).unwrap(),
        )
        .is_err()
    );

    let (request, _, _) = compile_success(scope(9), 100);
    assert!(
        CompileResultV1::new(
            identity!(CompileResultIdV1, 200),
            &request,
            CompileOutcomeV1::Failed,
            vec![],
        )
        .is_err()
    );
    assert!(
        CompileResultV1::new(
            identity!(CompileResultIdV1, 201),
            &request,
            CompileOutcomeV1::Candidate(payload(202, 32)),
            vec![error_diagnostic(1)],
        )
        .is_err()
    );
}

#[test]
fn upstream_substitution_and_noncanonical_admission_claims_reject() {
    let scope_id = scope(20);
    let (_, compile_result, candidate) = compile_success(scope_id, 300);
    assert!(matches!(
        AdmitRequestV1::new(
            identity!(AdmitRequestIdV1, 301),
            context(scope_id, 302),
            &compile_result,
            payload(999, candidate.byte_len()),
            identity!(AdmissionPolicyIdV1, 303),
            vec![],
        ),
        Err(HostContractErrorV1::Mismatch {
            field: ContractFieldV1::UpstreamObject
        })
    ));
    let claim = identity!(ClaimIdV1, 10);
    assert!(matches!(
        AdmitRequestV1::new(
            identity!(AdmitRequestIdV1, 304),
            context(scope_id, 305),
            &compile_result,
            candidate,
            identity!(AdmissionPolicyIdV1, 306),
            vec![claim, claim],
        ),
        Err(HostContractErrorV1::Duplicate {
            field: ContractFieldV1::AdmissionClaims
        })
    ));
    let claims = (0..=MAX_ADMISSION_CLAIMS_V1)
        .map(|index| identity!(ClaimIdV1, 2_000 + index as u64))
        .collect();
    assert!(matches!(
        AdmitRequestV1::new(
            identity!(AdmitRequestIdV1, 307),
            context(scope_id, 308),
            &compile_result,
            candidate,
            identity!(AdmissionPolicyIdV1, 309),
            claims,
        ),
        Err(HostContractErrorV1::TooManyItems {
            field: ContractFieldV1::AdmissionClaims,
            ..
        })
    ));
}

#[test]
fn dispatch_rejects_binding_gaps_dependency_confusion_and_failed_loads() {
    let scope_id = scope(30);
    let load = accepted_load(scope_id, 400);
    let binding =
        ResourceBindingV1::new(1, identity!(ResourceIdV1, 401), AccessModeV1::Read, 0, 16).unwrap();
    assert!(matches!(
        DispatchRequestV1::new(
            identity!(DispatchRequestIdV1, 402),
            context(scope_id, 403),
            &load,
            identity!(EntryPointIdV1, 404),
            identity!(DispatchContractIdV1, 405),
            identity!(ArgumentSetIdV1, 406),
            DispatchKindV1::Finite,
            vec![binding],
            vec![],
        ),
        Err(HostContractErrorV1::NonCanonicalOrder {
            field: ContractFieldV1::DispatchBindings
        })
    ));

    let aliased_dependencies = vec![
        DispatchDependencyV1::new(
            identity!(CompletionSignalIdV1, 4_500),
            identity!(DispatchSubmissionIdV1, 4_501),
        ),
        DispatchDependencyV1::new(
            identity!(CompletionSignalIdV1, 4_500),
            identity!(DispatchSubmissionIdV1, 4_502),
        ),
    ];
    assert!(matches!(
        DispatchRequestV1::new(
            identity!(DispatchRequestIdV1, 4_503),
            context(scope_id, 4_504),
            &load,
            identity!(EntryPointIdV1, 4_505),
            identity!(DispatchContractIdV1, 4_506),
            identity!(ArgumentSetIdV1, 4_507),
            DispatchKindV1::Finite,
            vec![],
            aliased_dependencies,
        ),
        Err(HostContractErrorV1::Duplicate {
            field: ContractFieldV1::DispatchDependencies
        })
    ));

    assert!(matches!(
        DispatchRequestV1::new(
            identity!(DispatchRequestIdV1, 4_508),
            context(scope_id, 4_509),
            &load,
            identity!(EntryPointIdV1, 4_510),
            identity!(DispatchContractIdV1, 4_511),
            identity!(ArgumentSetIdV1, 4_512),
            DispatchKindV1::PersistentTask {
                service_instance_identity: identity!(ServiceInstanceIdV1, 4_513),
                task_schema_identity: identity!(TaskSchemaIdV1, 4_514),
                task_tag: 0,
                service_epoch: 0,
            },
            vec![],
            vec![],
        ),
        Err(HostContractErrorV1::Empty {
            field: ContractFieldV1::ServiceEpoch
        })
    ));

    let dependency = DispatchDependencyV1::new(
        identity!(CompletionSignalIdV1, 410),
        identity!(DispatchSubmissionIdV1, 411),
    );
    assert!(matches!(
        DispatchRequestV1::new(
            identity!(DispatchRequestIdV1, 412),
            context(scope_id, 413),
            &load,
            identity!(EntryPointIdV1, 414),
            identity!(DispatchContractIdV1, 415),
            identity!(ArgumentSetIdV1, 416),
            DispatchKindV1::Finite,
            vec![],
            vec![dependency, dependency],
        ),
        Err(HostContractErrorV1::Duplicate {
            field: ContractFieldV1::DispatchDependencies
        })
    ));

    let dependencies = (0..=MAX_DISPATCH_DEPENDENCIES_V1)
        .map(|index| {
            DispatchDependencyV1::new(
                identity!(CompletionSignalIdV1, 3_000 + index as u64),
                identity!(DispatchSubmissionIdV1, 4_000 + index as u64),
            )
        })
        .collect();
    assert!(matches!(
        DispatchRequestV1::new(
            identity!(DispatchRequestIdV1, 417),
            context(scope_id, 418),
            &load,
            identity!(EntryPointIdV1, 419),
            identity!(DispatchContractIdV1, 420),
            identity!(ArgumentSetIdV1, 421),
            DispatchKindV1::Finite,
            vec![],
            dependencies,
        ),
        Err(HostContractErrorV1::TooManyItems {
            field: ContractFieldV1::DispatchDependencies,
            ..
        })
    ));

    let (_, compile_result, candidate) = compile_success(scope_id, 500);
    let admit_request = AdmitRequestV1::new(
        identity!(AdmitRequestIdV1, 501),
        context(scope_id, 502),
        &compile_result,
        candidate,
        identity!(AdmissionPolicyIdV1, 503),
        vec![],
    )
    .unwrap();
    let admit_result = AdmitResultV1::new(
        identity!(AdmitResultIdV1, 504),
        &admit_request,
        AdmitOutcomeV1::Accepted {
            assessment_identity: identity!(AdmissionAssessmentIdV1, 505),
        },
        vec![],
    )
    .unwrap();
    let load_request = LoadRequestV1::new(
        identity!(LoadRequestIdV1, 506),
        context(scope_id, 507),
        &admit_result,
        candidate.identity(),
        identity!(LoaderProfileIdV1, 508),
        identity!(RuntimeContextIdV1, 509),
    )
    .unwrap();
    let failed_load = LoadResultV1::new(
        identity!(LoadResultIdV1, 510),
        &load_request,
        LoadOutcomeV1::Failed,
        vec![error_diagnostic(510)],
    )
    .unwrap();
    assert!(matches!(
        DispatchRequestV1::new(
            identity!(DispatchRequestIdV1, 511),
            context(scope_id, 512),
            &failed_load,
            identity!(EntryPointIdV1, 513),
            identity!(DispatchContractIdV1, 514),
            identity!(ArgumentSetIdV1, 515),
            DispatchKindV1::Finite,
            vec![],
            vec![],
        ),
        Err(HostContractErrorV1::InvalidOutcome)
    ));
}

#[test]
fn parallel_dispatches_have_independent_interleavable_state_chains() {
    let scope_id = scope(40);
    let (request_a, result_a) = submitted_dispatch(scope_id, 600, 10_000, DispatchKindV1::Finite);
    let (request_b, result_b) = submitted_dispatch(
        scope_id,
        700,
        10_001,
        DispatchKindV1::PersistentTask {
            service_instance_identity: identity!(ServiceInstanceIdV1, 701),
            task_schema_identity: identity!(TaskSchemaIdV1, 702),
            task_tag: 4,
            service_epoch: 9,
        },
    );

    let a0 = HostOperationStateV1::initial(
        HostStateIdentityV1::Dispatch(identity!(DispatchStateIdV1, 800)),
        request_a.context(),
        HostRequestIdentityV1::Dispatch(request_a.identity()),
    )
    .unwrap();
    let a1 = HostOperationStateV1::transition(
        HostStateIdentityV1::Dispatch(identity!(DispatchStateIdV1, 801)),
        &a0,
        OperationPhaseV1::Pending,
    )
    .unwrap();
    let a2 = HostOperationStateV1::transition(
        HostStateIdentityV1::Dispatch(identity!(DispatchStateIdV1, 802)),
        &a1,
        OperationPhaseV1::Succeeded(HostResultIdentityV1::Dispatch(result_a.identity())),
    )
    .unwrap();
    a2.validate_terminal_result(result_a.result_reference())
        .unwrap();

    let b0 = HostOperationStateV1::initial(
        HostStateIdentityV1::Dispatch(identity!(DispatchStateIdV1, 900)),
        request_b.context(),
        HostRequestIdentityV1::Dispatch(request_b.identity()),
    )
    .unwrap();
    let b1 = HostOperationStateV1::transition(
        HostStateIdentityV1::Dispatch(identity!(DispatchStateIdV1, 901)),
        &b0,
        OperationPhaseV1::Active,
    )
    .unwrap();
    let b2 = HostOperationStateV1::transition(
        HostStateIdentityV1::Dispatch(identity!(DispatchStateIdV1, 902)),
        &b1,
        OperationPhaseV1::Succeeded(HostResultIdentityV1::Dispatch(result_b.identity())),
    )
    .unwrap();
    b2.validate_terminal_result(result_b.result_reference())
        .unwrap();

    let events = vec![
        HostEventV1::new(
            HostEventIdentityV1::Dispatch(identity!(DispatchEventIdV1, 800)),
            &a0,
        )
        .unwrap(),
        HostEventV1::new(
            HostEventIdentityV1::Dispatch(identity!(DispatchEventIdV1, 900)),
            &b0,
        )
        .unwrap(),
        HostEventV1::new(
            HostEventIdentityV1::Dispatch(identity!(DispatchEventIdV1, 901)),
            &b1,
        )
        .unwrap(),
        HostEventV1::new(
            HostEventIdentityV1::Dispatch(identity!(DispatchEventIdV1, 801)),
            &a1,
        )
        .unwrap(),
        HostEventV1::new(
            HostEventIdentityV1::Dispatch(identity!(DispatchEventIdV1, 802)),
            &a2,
        )
        .unwrap(),
        HostEventV1::new(
            HostEventIdentityV1::Dispatch(identity!(DispatchEventIdV1, 902)),
            &b2,
        )
        .unwrap(),
    ];
    HostEventBatchV1::new(scope_id, events).unwrap();

    let targets = vec![
        identity!(CompletionSignalIdV1, 10_000),
        identity!(CompletionSignalIdV1, 10_001),
    ];
    let wait_request = WaitRequestV1::new(
        identity!(WaitRequestIdV1, 1_000),
        context(scope_id, 1_001),
        WaitModeV1::All,
        targets,
        None,
    )
    .unwrap();
    wait_request
        .validate_dispatch_results(&[result_b.clone(), result_a.clone()])
        .unwrap();
    let wait_result = WaitResultV1::new(
        identity!(WaitResultIdV1, 1_002),
        &wait_request,
        WaitOutcomeV1::Satisfied(vec![
            CompletionObservationV1::new(
                identity!(CompletionSignalIdV1, 10_000),
                identity!(CompletionRecordIdV1, 11_000),
                CompletionStatusV1::Succeeded,
            ),
            CompletionObservationV1::new(
                identity!(CompletionSignalIdV1, 10_001),
                identity!(CompletionRecordIdV1, 11_001),
                CompletionStatusV1::Failed,
            ),
        ]),
        vec![],
    )
    .unwrap();
    assert!(matches!(
        wait_result.outcome(),
        WaitOutcomeV1::Satisfied(observations) if observations.len() == 2
    ));
}

#[test]
fn state_and_event_chains_reject_cross_flow_stale_skipped_and_terminal_use() {
    let scope_id = scope(50);
    let (request, result) = submitted_dispatch(scope_id, 1_100, 12_000, DispatchKindV1::Finite);
    assert!(matches!(
        HostOperationStateV1::initial(
            HostStateIdentityV1::Load(identity!(LoadStateIdV1, 1_101)),
            request.context(),
            HostRequestIdentityV1::Dispatch(request.identity()),
        ),
        Err(HostContractErrorV1::Mismatch {
            field: ContractFieldV1::Flow
        })
    ));

    let s0 = HostOperationStateV1::initial(
        HostStateIdentityV1::Dispatch(identity!(DispatchStateIdV1, 1_102)),
        request.context(),
        HostRequestIdentityV1::Dispatch(request.identity()),
    )
    .unwrap();
    assert!(
        HostOperationStateV1::transition(
            HostStateIdentityV1::Dispatch(identity!(DispatchStateIdV1, 1_103)),
            &s0,
            OperationPhaseV1::Succeeded(HostResultIdentityV1::Load(identity!(
                LoadResultIdV1,
                1_104
            ))),
        )
        .is_err()
    );
    let s1 = HostOperationStateV1::transition(
        HostStateIdentityV1::Dispatch(identity!(DispatchStateIdV1, 1_105)),
        &s0,
        OperationPhaseV1::Pending,
    )
    .unwrap();
    assert!(matches!(
        HostOperationStateV1::transition(s1.identity(), &s1, OperationPhaseV1::Active,),
        Err(HostContractErrorV1::Duplicate {
            field: ContractFieldV1::StateIdentity
        })
    ));
    let s2 = HostOperationStateV1::transition(
        HostStateIdentityV1::Dispatch(identity!(DispatchStateIdV1, 1_106)),
        &s1,
        OperationPhaseV1::Succeeded(HostResultIdentityV1::Dispatch(result.identity())),
    )
    .unwrap();
    assert!(matches!(
        HostOperationStateV1::transition(
            HostStateIdentityV1::Dispatch(identity!(DispatchStateIdV1, 1_107)),
            &s2,
            OperationPhaseV1::Cancelled,
        ),
        Err(HostContractErrorV1::TerminalStateTransition)
    ));

    let e0 = HostEventV1::new(
        HostEventIdentityV1::Dispatch(identity!(DispatchEventIdV1, 1_102)),
        &s0,
    )
    .unwrap();
    let e1 = HostEventV1::new(
        HostEventIdentityV1::Dispatch(identity!(DispatchEventIdV1, 1_105)),
        &s1,
    )
    .unwrap();
    let e2 = HostEventV1::new(
        HostEventIdentityV1::Dispatch(identity!(DispatchEventIdV1, 1_106)),
        &s2,
    )
    .unwrap();
    assert!(e0.validate_state(&s1).is_err());
    assert!(matches!(
        HostEventBatchV1::new(scope_id, vec![e0, e2]),
        Err(HostContractErrorV1::Mismatch {
            field: ContractFieldV1::StateRevision
        })
    ));
    assert!(matches!(
        HostEventBatchV1::new(scope_id, vec![e0, e0]),
        Err(HostContractErrorV1::Duplicate {
            field: ContractFieldV1::EventBatch
        })
    ));
    let duplicate_state_event = HostEventV1::new(
        HostEventIdentityV1::Dispatch(identity!(DispatchEventIdV1, 1_108)),
        &s0,
    )
    .unwrap();
    assert!(matches!(
        HostEventBatchV1::new(scope_id, vec![e0, duplicate_state_event]),
        Err(HostContractErrorV1::Duplicate {
            field: ContractFieldV1::StateIdentity
        })
    ));
    HostEventBatchV1::new(scope_id, vec![e0, e1, e2]).unwrap();
}

#[test]
fn wait_predicates_reject_unknown_duplicate_and_contradictory_observations() {
    let scope_id = scope(60);
    assert!(
        WaitRequestV1::new(
            identity!(WaitRequestIdV1, 1_200),
            context(scope_id, 1_201),
            WaitModeV1::Any,
            vec![],
            None,
        )
        .is_err()
    );
    let signal_a = identity!(CompletionSignalIdV1, 1_210);
    let signal_b = identity!(CompletionSignalIdV1, 1_211);
    assert!(
        WaitRequestV1::new(
            identity!(WaitRequestIdV1, 1_202),
            context(scope_id, 1_203),
            WaitModeV1::All,
            vec![signal_a, signal_a],
            None,
        )
        .is_err()
    );
    let targets = (0..=MAX_WAIT_TARGETS_V1)
        .map(|index| identity!(CompletionSignalIdV1, 20_000 + index as u64))
        .collect();
    assert!(matches!(
        WaitRequestV1::new(
            identity!(WaitRequestIdV1, 1_204),
            context(scope_id, 1_205),
            WaitModeV1::All,
            targets,
            None,
        ),
        Err(HostContractErrorV1::TooManyItems {
            field: ContractFieldV1::WaitTargets,
            ..
        })
    ));

    let any = WaitRequestV1::new(
        identity!(WaitRequestIdV1, 1_206),
        context(scope_id, 1_207),
        WaitModeV1::Any,
        vec![signal_a, signal_b],
        None,
    )
    .unwrap();
    let observed_a = CompletionObservationV1::new(
        signal_a,
        identity!(CompletionRecordIdV1, 1_212),
        CompletionStatusV1::Succeeded,
    );
    assert!(
        WaitResultV1::new(
            identity!(WaitResultIdV1, 1_208),
            &any,
            WaitOutcomeV1::Satisfied(vec![]),
            vec![],
        )
        .is_err()
    );
    assert!(
        WaitResultV1::new(
            identity!(WaitResultIdV1, 1_209),
            &any,
            WaitOutcomeV1::Pending(vec![observed_a]),
            vec![],
        )
        .is_err()
    );
    assert!(
        WaitResultV1::new(
            identity!(WaitResultIdV1, 1_213),
            &any,
            WaitOutcomeV1::Satisfied(vec![CompletionObservationV1::new(
                identity!(CompletionSignalIdV1, 9_999),
                identity!(CompletionRecordIdV1, 1_214),
                CompletionStatusV1::Succeeded,
            )]),
            vec![],
        )
        .is_err()
    );
    assert!(
        WaitResultV1::new(
            identity!(WaitResultIdV1, 1_215),
            &any,
            WaitOutcomeV1::Satisfied(vec![observed_a, observed_a]),
            vec![],
        )
        .is_err()
    );

    let all = WaitRequestV1::new(
        identity!(WaitRequestIdV1, 1_216),
        context(scope_id, 1_217),
        WaitModeV1::All,
        vec![signal_a, signal_b],
        None,
    )
    .unwrap();
    assert!(
        WaitResultV1::new(
            identity!(WaitResultIdV1, 1_218),
            &all,
            WaitOutcomeV1::Satisfied(vec![observed_a]),
            vec![],
        )
        .is_err()
    );
}

#[test]
fn maximum_valid_records_have_bounded_identity_preimages() {
    let scope_id = scope(70);
    let (request, _, _) = compile_success(scope_id, 1_300);
    let diagnostics = (0..MAX_DIAGNOSTICS_V1)
        .map(|index| {
            HostDiagnosticV1::new(
                index as u32 + 1,
                DiagnosticSeverityV1::Error,
                Some(digest(30_000 + index as u64)),
                DiagnosticMessageV1::new("x".repeat(MAX_DIAGNOSTIC_MESSAGE_BYTES_V1)).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let result = CompileResultV1::new(
        identity!(CompileResultIdV1, 1_301),
        &request,
        CompileOutcomeV1::Failed,
        diagnostics,
    )
    .unwrap();
    assert!(result.encode_identity_preimage().len() <= MAX_IDENTITY_PREIMAGE_BYTES_V1);

    let targets: Vec<_> = (0..MAX_WAIT_TARGETS_V1)
        .map(|index| identity!(CompletionSignalIdV1, 40_000 + index as u64))
        .collect();
    let observations = targets
        .iter()
        .enumerate()
        .map(|(index, signal)| {
            CompletionObservationV1::new(
                *signal,
                identity!(CompletionRecordIdV1, 50_000 + index as u64),
                CompletionStatusV1::Succeeded,
            )
        })
        .collect();
    let wait_request = WaitRequestV1::new(
        identity!(WaitRequestIdV1, 1_302),
        context(scope_id, 1_303),
        WaitModeV1::All,
        targets,
        Some(identity!(DeadlineIdV1, 1_304)),
    )
    .unwrap();
    let wait_result = WaitResultV1::new(
        identity!(WaitResultIdV1, 1_305),
        &wait_request,
        WaitOutcomeV1::Satisfied(observations),
        vec![],
    )
    .unwrap();
    assert!(wait_request.encode_identity_preimage().len() <= MAX_IDENTITY_PREIMAGE_BYTES_V1);
    assert!(wait_result.encode_identity_preimage().len() <= MAX_IDENTITY_PREIMAGE_BYTES_V1);
}
