use fe2o3_verifier::{
    AxiomPolicy, Configuration, ConfigurationEntry, CorrelationId, Digest, ExecutionTools,
    InvocationPaths, MAX_CONFIGURATION_ENTRIES, MAX_PROPERTIES, MAX_RESULT_BYTES, MAX_TEXT_BYTES,
    MAX_TRUSTED_ITEMS, MeasuredToolIdentity, ModelError, PlanError, ProofOutcome, ProofProperty,
    ProofRequestV1, ProofTargetIdentity, RecorderTermination, ResultError, TrustedItem,
    VerificationModelIdentity, VerifierPolicy, build_invocation_plan,
};

fn digest(seed: u8) -> Digest {
    Digest::from_bytes([seed; 32])
}

fn correlation(seed: u8) -> CorrelationId {
    CorrelationId::from_bytes([seed; 16])
}

fn target() -> ProofTargetIdentity {
    ProofTargetIdentity {
        kernel_id: digest(1),
        instance_digest: digest(2),
        source_tree_digest: digest(3),
        crate_graph_digest: digest(4),
        executable_digest: digest(5),
        environment_digest: digest(6),
        artifact_selection_digest: digest(7),
        artifact_contract_digest: digest(8),
        memory_contract_digest: digest(9),
        effects_contract_digest: digest(10),
        type_layout_digest: digest(11),
        capability_semantics_digest: digest(12),
        functional_specification_digest: digest(13),
    }
}

fn configuration() -> Configuration {
    Configuration::new(vec![
        ConfigurationEntry::new("solver", "z3").unwrap(),
        ConfigurationEntry::new("arithmetic", "integer").unwrap(),
    ])
    .unwrap()
}

fn model() -> VerificationModelIdentity {
    VerificationModelIdentity::new("gpu-model-v1", digest(20)).unwrap()
}

fn tool(name: &str, seed: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(name, "1.0.0", digest(seed), digest(seed + 1)).unwrap()
}

fn tools() -> ExecutionTools {
    ExecutionTools::new(
        tool("verus", 30),
        tool("z3", 32),
        tool("fe2o3-recorder", 34),
    )
}

fn axiom() -> TrustedItem {
    TrustedItem::new("gpu_integer_model", digest(40)).unwrap()
}

fn request_with(trusted: Vec<TrustedItem>) -> ProofRequestV1 {
    ProofRequestV1::new(
        correlation(50),
        target(),
        configuration(),
        model(),
        vec![ProofProperty::RaceFreedom, ProofProperty::Bounds],
        trusted,
    )
    .unwrap()
}

fn policy_with(axioms: Vec<TrustedItem>) -> VerifierPolicy {
    VerifierPolicy::new(
        tools(),
        configuration(),
        model(),
        AxiomPolicy::allow_list(axioms).unwrap(),
        600,
    )
    .unwrap()
}

fn paths() -> InvocationPaths {
    InvocationPaths::new(
        "/opt/verus bin/verus",
        "/opt/z3/bin/z3",
        "/opt/fe2o3/bin/recorder",
        "/tmp/request file.bin",
        "/tmp/result file.txt",
    )
    .unwrap()
}

fn plan(trusted: Vec<TrustedItem>) -> fe2o3_verifier::InvocationPlan {
    let allowed = trusted.clone();
    build_invocation_plan(
        request_with(trusted),
        tools(),
        paths(),
        90,
        &policy_with(allowed),
    )
    .unwrap()
}

fn envelope(outcome: &str, properties: &str, trusted: &str, diagnostic_hex: &str) -> Vec<u8> {
    format!(
        "FE2O3-VERIFIER-RESULT-V1\ncorrelation={}\noutcome={outcome}\nproperties={properties}\ntrusted={trusted}\ndiagnostic-hex={diagnostic_hex}\n",
        correlation(50).to_hex()
    )
    .into_bytes()
}

fn trusted_text(item: &TrustedItem) -> String {
    format!(
        "{}@{}",
        item.name().as_str(),
        item.contract_digest().to_hex()
    )
}

fn parse_recorder_result(
    bytes: &[u8],
    plan: &fe2o3_verifier::InvocationPlan,
) -> Result<fe2o3_verifier::ProofResultV1, ResultError> {
    fe2o3_verifier::parse_recorder_result(bytes, plan, RecorderTermination::Exited(0))
}

#[test]
fn configuration_and_requests_have_canonical_order() {
    let config = configuration();
    assert_eq!(config.entries()[0].key().as_str(), "arithmetic");
    assert_eq!(config.entries()[1].key().as_str(), "solver");

    let request = request_with(vec![
        TrustedItem::new("z_axiom", digest(1)).unwrap(),
        TrustedItem::new("a_axiom", digest(2)).unwrap(),
    ]);
    assert_eq!(
        request.properties(),
        &[ProofProperty::Bounds, ProofProperty::RaceFreedom]
    );
    assert_eq!(request.trusted_items()[0].name().as_str(), "a_axiom");
}

#[test]
fn duplicate_configuration_properties_and_axioms_are_rejected() {
    let duplicate_config = vec![
        ConfigurationEntry::new("solver", "z3").unwrap(),
        ConfigurationEntry::new("solver", "cvc5").unwrap(),
    ];
    assert!(matches!(
        Configuration::new(duplicate_config),
        Err(ModelError::DuplicateItem {
            field: "configuration key"
        })
    ));
    assert!(matches!(
        ProofRequestV1::new(
            correlation(1),
            target(),
            configuration(),
            model(),
            vec![ProofProperty::Bounds, ProofProperty::Bounds],
            vec![],
        ),
        Err(ModelError::DuplicateItem {
            field: "proof property"
        })
    ));
    assert!(matches!(
        AxiomPolicy::allow_list(vec![axiom(), axiom()]),
        Err(ModelError::DuplicateItem {
            field: "trusted item name"
        })
    ));
}

#[test]
fn model_bounds_are_enforced_before_planning() {
    assert!(matches!(
        MeasuredToolIdentity::new("x".repeat(MAX_TEXT_BYTES + 1), "1", digest(1), digest(2)),
        Err(ModelError::LengthOutOfRange {
            field: "tool name",
            ..
        })
    ));

    let entries = (0..=MAX_CONFIGURATION_ENTRIES)
        .map(|index| ConfigurationEntry::new(format!("k{index}"), "v").unwrap())
        .collect();
    assert!(matches!(
        Configuration::new(entries),
        Err(ModelError::TooManyItems {
            field: "configuration",
            ..
        })
    ));

    let axioms = (0..=MAX_TRUSTED_ITEMS)
        .map(|index| TrustedItem::new(format!("axiom_{index}"), digest(index as u8)).unwrap())
        .collect();
    assert!(matches!(
        AxiomPolicy::allow_list(axioms),
        Err(ModelError::TooManyItems {
            field: "trusted items",
            ..
        })
    ));
}

#[test]
fn identifiers_and_hex_are_canonical() {
    assert!(matches!(
        ConfigurationEntry::new("bad key", "value"),
        Err(ModelError::InvalidIdentifier { .. })
    ));
    assert!(matches!(
        TrustedItem::new("line\nbreak", digest(1)),
        Err(ModelError::NonCanonicalText { .. })
    ));
    assert!(matches!(
        MeasuredToolIdentity::new("v\u{e9}rus", "1", digest(1), digest(2)),
        Err(ModelError::NonCanonicalText { field: "tool name" })
    ));
    assert!(Digest::from_hex(&"AA".repeat(32)).is_err());
    assert_eq!(Digest::from_hex(&digest(7).to_hex()).unwrap(), digest(7));
}

#[test]
fn invocation_plan_is_deterministic_and_never_builds_a_shell_command() {
    let left = plan(vec![axiom()]);
    let right = plan(vec![axiom()]);
    assert_eq!(left, right);
    assert_eq!(left.request_bytes(), right.request_bytes());
    assert_eq!(
        left.canonical_invocation_bytes(),
        right.canonical_invocation_bytes()
    );
    assert_eq!(left.command().program(), "/opt/fe2o3/bin/recorder");
    assert_eq!(left.command().arguments()[1], "/tmp/request file.bin");
    assert_eq!(left.command().arguments()[5], "/opt/verus bin/verus");
    assert!(!left.command().arguments().iter().any(|arg| arg == "sh"));
}

#[test]
fn canonical_plan_changes_when_an_identity_changes() {
    let base = plan(vec![]);
    let changed_tools = ExecutionTools::new(
        tool("verus", 80),
        tool("z3", 32),
        tool("fe2o3-recorder", 34),
    );
    let changed_policy = VerifierPolicy::new(
        changed_tools.clone(),
        configuration(),
        model(),
        AxiomPolicy::deny_all(),
        600,
    )
    .unwrap();
    let changed = build_invocation_plan(
        request_with(vec![]),
        changed_tools,
        paths(),
        90,
        &changed_policy,
    )
    .unwrap();
    assert_ne!(
        base.canonical_invocation_bytes(),
        changed.canonical_invocation_bytes()
    );
}

#[test]
fn tool_configuration_model_and_axiom_policy_mismatches_fail_closed() {
    let request = request_with(vec![axiom()]);
    let deny = VerifierPolicy::new(
        tools(),
        configuration(),
        model(),
        AxiomPolicy::deny_all(),
        600,
    )
    .unwrap();
    assert!(matches!(
        build_invocation_plan(request.clone(), tools(), paths(), 90, &deny),
        Err(PlanError::Model(ModelError::AxiomRejected(_)))
    ));

    let wrong_tools = ExecutionTools::new(
        tool("other-verus", 60),
        tool("z3", 32),
        tool("fe2o3-recorder", 34),
    );
    assert_eq!(
        build_invocation_plan(
            request.clone(),
            wrong_tools,
            paths(),
            90,
            &policy_with(vec![axiom()])
        ),
        Err(PlanError::ToolPolicyMismatch)
    );

    let wrong_config =
        Configuration::new(vec![ConfigurationEntry::new("solver", "cvc5").unwrap()]).unwrap();
    let wrong_config_policy = VerifierPolicy::new(
        tools(),
        wrong_config,
        model(),
        AxiomPolicy::allow_list(vec![axiom()]).unwrap(),
        600,
    )
    .unwrap();
    assert_eq!(
        build_invocation_plan(request.clone(), tools(), paths(), 90, &wrong_config_policy),
        Err(PlanError::ConfigurationPolicyMismatch)
    );

    let wrong_model_policy = VerifierPolicy::new(
        tools(),
        configuration(),
        VerificationModelIdentity::new("other-model", digest(20)).unwrap(),
        AxiomPolicy::allow_list(vec![axiom()]).unwrap(),
        600,
    )
    .unwrap();
    assert_eq!(
        build_invocation_plan(request, tools(), paths(), 90, &wrong_model_policy),
        Err(PlanError::ModelPolicyMismatch)
    );
}

#[test]
fn paths_and_timeouts_are_bounded() {
    assert!(InvocationPaths::new("", "z3", "recorder", "request", "result").is_err());
    assert!(InvocationPaths::new("verus\n", "z3", "recorder", "request", "result").is_err());
    let policy = policy_with(vec![]);
    assert!(matches!(
        build_invocation_plan(request_with(vec![]), tools(), paths(), 0, &policy),
        Err(PlanError::TimeoutOutOfRange { .. })
    ));
    assert!(matches!(
        build_invocation_plan(request_with(vec![]), tools(), paths(), 601, &policy),
        Err(PlanError::TimeoutOutOfRange { .. })
    ));
}

#[test]
fn proved_result_binds_all_requested_evidence() {
    let axiom = axiom();
    let plan = plan(vec![axiom.clone()]);
    let output = envelope(
        "proved",
        "bounds,race-freedom",
        &trusted_text(&axiom),
        "7665726966696564",
    );
    let result = parse_recorder_result(&output, &plan).unwrap();
    assert_eq!(result.outcome(), ProofOutcome::Proved);
    assert_eq!(result.target(), target());
    assert_eq!(result.configuration(), &configuration());
    assert_eq!(result.tools(), &tools());
    assert_eq!(
        result.recorder_reported_properties(),
        plan.request().properties()
    );
    assert_eq!(result.trusted_items(), &[axiom]);
    assert_eq!(result.diagnostic().unwrap().as_str(), "verified");
}

#[test]
fn failed_and_timed_out_results_are_evidence_without_claims() {
    let plan = plan(vec![]);
    for outcome in ["failed", "timed-out"] {
        let result = parse_recorder_result(&envelope(outcome, "", "", ""), &plan).unwrap();
        assert!(matches!(
            result.outcome(),
            ProofOutcome::Failed | ProofOutcome::TimedOut
        ));
        assert!(result.recorder_reported_properties().is_empty());
        assert!(result.diagnostic().is_none());
    }
}

#[test]
fn malformed_and_oversized_envelopes_are_rejected() {
    let plan = plan(vec![]);
    assert_eq!(
        parse_recorder_result(b"not utf8: \xff", &plan),
        Err(ResultError::InvalidUtf8)
    );
    assert_eq!(
        parse_recorder_result(b"FE2O3-VERIFIER-RESULT-V1", &plan),
        Err(ResultError::MalformedEnvelope)
    );
    let mut extra = envelope("failed", "", "", "");
    extra.extend_from_slice(b"extra=true\n");
    assert_eq!(
        parse_recorder_result(&extra, &plan),
        Err(ResultError::MalformedEnvelope)
    );
    assert_eq!(
        parse_recorder_result(&vec![b'x'; MAX_RESULT_BYTES + 1], &plan),
        Err(ResultError::TooLarge {
            max: MAX_RESULT_BYTES
        })
    );
}

#[test]
fn recorder_crash_timeout_or_signal_rejects_even_a_proved_envelope() {
    let plan = plan(vec![]);
    let proved = envelope("proved", "bounds,race-freedom", "", "");
    for termination in [
        RecorderTermination::Exited(1),
        RecorderTermination::TimedOut,
        RecorderTermination::Signaled(9),
    ] {
        assert_eq!(
            fe2o3_verifier::parse_recorder_result(&proved, &plan, termination),
            Err(ResultError::RecorderDidNotSucceed(termination))
        );
    }
}

#[test]
fn unknown_reordered_and_duplicate_fields_fail_closed() {
    let plan = plan(vec![]);
    let unknown_field = envelope("failed", "", "", "")
        .split(|byte| *byte == b'\n')
        .enumerate()
        .flat_map(|(index, line)| {
            let mut line = if index == 2 {
                b"status=failed".to_vec()
            } else {
                line.to_vec()
            };
            line.push(b'\n');
            line
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        parse_recorder_result(&unknown_field, &plan),
        Err(ResultError::UnexpectedField {
            expected: "outcome"
        }) | Err(ResultError::MalformedEnvelope)
    ));
    assert!(matches!(
        parse_recorder_result(&envelope("proved", "race-freedom,bounds", "", ""), &plan),
        Err(ResultError::NonCanonicalOrder {
            field: "properties"
        })
    ));
    assert!(matches!(
        parse_recorder_result(&envelope("proved", "bounds,bounds", "", ""), &plan),
        Err(ResultError::NonCanonicalOrder {
            field: "properties"
        })
    ));
}

#[test]
fn result_correlation_outcome_and_property_vocabulary_are_strict() {
    let plan = plan(vec![]);
    let wrong_correlation = String::from_utf8(envelope("failed", "", "", ""))
        .unwrap()
        .replace(&correlation(50).to_hex(), &correlation(51).to_hex());
    assert_eq!(
        parse_recorder_result(wrong_correlation.as_bytes(), &plan),
        Err(ResultError::CorrelationMismatch)
    );
    assert!(matches!(
        parse_recorder_result(&envelope("unknown", "", "", ""), &plan),
        Err(ResultError::Model(ModelError::UnknownValue {
            field: "proof outcome"
        }))
    ));
    assert!(matches!(
        parse_recorder_result(&envelope("proved", "bounds,new-property", "", ""), &plan),
        Err(ResultError::Model(ModelError::UnknownValue {
            field: "proof property"
        }))
    ));
}

#[test]
fn false_success_and_failure_claims_are_rejected() {
    let plan = plan(vec![]);
    assert_eq!(
        parse_recorder_result(&envelope("proved", "bounds", "", ""), &plan),
        Err(ResultError::IncompleteProof)
    );
    assert_eq!(
        parse_recorder_result(&envelope("failed", "bounds", "", ""), &plan),
        Err(ResultError::ClaimsOnIncompleteProof)
    );
}

#[test]
fn trusted_inventory_must_be_canonical_and_exact() {
    let axiom = axiom();
    let plan = plan(vec![axiom.clone()]);
    assert_eq!(
        parse_recorder_result(&envelope("failed", "", "", ""), &plan),
        Err(ResultError::TrustedItemsMismatch)
    );
    let changed = TrustedItem::new("gpu_integer_model", digest(41)).unwrap();
    assert_eq!(
        parse_recorder_result(&envelope("failed", "", &trusted_text(&changed), ""), &plan),
        Err(ResultError::TrustedItemsMismatch)
    );
    assert_eq!(
        parse_recorder_result(&envelope("failed", "", "missing-at-sign", ""), &plan),
        Err(ResultError::MalformedTrustedItem)
    );
}

#[test]
fn parser_enforces_result_collection_bounds() {
    let plan = plan(vec![]);
    let too_many_properties = std::iter::repeat_n("bounds", MAX_PROPERTIES + 1)
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        parse_recorder_result(&envelope("proved", &too_many_properties, "", ""), &plan),
        Err(ResultError::CountOutOfRange {
            field: "properties",
            max: MAX_PROPERTIES
        })
    );
}

#[test]
fn diagnostic_must_be_lowercase_hex_bounded_canonical_text() {
    let plan = plan(vec![]);
    assert_eq!(
        parse_recorder_result(&envelope("failed", "", "", "GG"), &plan),
        Err(ResultError::InvalidDiagnostic)
    );
    assert_eq!(
        parse_recorder_result(&envelope("failed", "", "", "0A"), &plan),
        Err(ResultError::InvalidDiagnostic)
    );
    assert!(matches!(
        parse_recorder_result(&envelope("failed", "", "", "0a"), &plan),
        Err(ResultError::Model(ModelError::NonCanonicalText {
            field: "diagnostic"
        }))
    ));
}
