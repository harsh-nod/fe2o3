use fe2o3_artifacts::{
    DigestAlgorithm, ProofOutcome as ArtifactOutcome, ProofProperty as ArtifactProperty,
};
use fe2o3_verifier::{
    ArtifactRecordConversionError, AxiomPolicy, Configuration, ConfigurationEntry, CorrelationId,
    Digest, ExecutionTools, InvocationPaths, InvocationPlan, MeasuredToolIdentity, ProofOutcome,
    ProofProperty, ProofRequestV1, ProofTargetIdentity, RecorderTermination,
    ReviewedInvocationIdentityV1, TrustedItem, VerificationModelIdentity, VerifierPolicy,
    build_invocation_plan, canonical_invocation_digest, convert_to_artifact_proof_record,
    parse_recorder_result,
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

fn trusted(name: &str, seed: u8) -> TrustedItem {
    TrustedItem::new(name, digest(seed)).unwrap()
}

fn paths() -> InvocationPaths {
    InvocationPaths::new(
        "/opt/verus/bin/verus",
        "/opt/z3/bin/z3",
        "/opt/fe2o3/bin/recorder",
        "/tmp/request.bin",
        "/tmp/result.txt",
    )
    .unwrap()
}

fn plan_with(
    correlation_id: CorrelationId,
    target: ProofTargetIdentity,
    configuration: Configuration,
    model: VerificationModelIdentity,
    properties: Vec<ProofProperty>,
    trusted_items: Vec<TrustedItem>,
    tools: ExecutionTools,
) -> InvocationPlan {
    let request = ProofRequestV1::new(
        correlation_id,
        target,
        configuration.clone(),
        model.clone(),
        properties,
        trusted_items.clone(),
    )
    .unwrap();
    let policy = VerifierPolicy::new(
        tools.clone(),
        configuration,
        model,
        AxiomPolicy::allow_list(trusted_items).unwrap(),
        600,
    )
    .unwrap();
    build_invocation_plan(request, tools, paths(), 90, &policy).unwrap()
}

fn plan() -> InvocationPlan {
    plan_with(
        correlation(50),
        target(),
        configuration(),
        model(),
        vec![ProofProperty::RaceFreedom, ProofProperty::Bounds],
        vec![trusted("gpu_integer_model", 40)],
        tools(),
    )
}

fn envelope(plan: &InvocationPlan, outcome: &str, properties: &[ProofProperty]) -> Vec<u8> {
    let properties = properties
        .iter()
        .map(|property| property.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let trusted_items = plan
        .request()
        .trusted_items()
        .iter()
        .map(|item| {
            format!(
                "{}@{}",
                item.name().as_str(),
                item.contract_digest().to_hex()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "FE2O3-VERIFIER-RESULT-V1\ncorrelation={}\noutcome={outcome}\nproperties={properties}\ntrusted={trusted_items}\ndiagnostic-hex=\n",
        plan.request().correlation_id().to_hex()
    )
    .into_bytes()
}

fn result(plan: &InvocationPlan, outcome: &str) -> fe2o3_verifier::ProofResultV1 {
    let properties = if outcome == "proved" {
        plan.request().properties()
    } else {
        &[]
    };
    parse_recorder_result(
        &envelope(plan, outcome, properties),
        plan,
        RecorderTermination::Exited(0),
    )
    .unwrap()
}

fn review(plan: &InvocationPlan) -> ReviewedInvocationIdentityV1 {
    ReviewedInvocationIdentityV1::from_hex(
        plan.request().correlation_id(),
        &canonical_invocation_digest(plan).to_hex(),
    )
    .unwrap()
}

#[test]
fn proved_conversion_binds_every_identity_exactly() {
    let plan = plan();
    let evidence =
        convert_to_artifact_proof_record(&plan, &result(&plan, "proved"), review(&plan)).unwrap();
    let record = evidence.record();

    assert_eq!(evidence.correlation_id(), correlation(50));
    assert_eq!(
        evidence.canonical_invocation_digest(),
        canonical_invocation_digest(&plan)
    );
    assert_eq!(record.outcome(), ArtifactOutcome::Proved);
    assert_eq!(
        record.proved_properties(),
        &[ArtifactProperty::Bounds, ArtifactProperty::RaceFreedom]
    );
    assert_eq!(record.configuration()[0].key().as_str(), "arithmetic");
    assert_eq!(record.configuration()[1].value().as_str(), "z3");
    assert_eq!(
        record.target().artifact().kernel_id().bytes().as_bytes(),
        target().kernel_id.as_bytes()
    );
    assert_eq!(
        record
            .target()
            .source_contracts()
            .functional_specification_digest()
            .bytes()
            .as_bytes(),
        target().functional_specification_digest.as_bytes()
    );
    assert_eq!(
        record.execution().model().version().as_str(),
        "gpu-model-v1"
    );
    assert_eq!(
        record
            .execution()
            .model()
            .axioms_digest()
            .bytes()
            .as_bytes(),
        model().axioms_digest().as_bytes()
    );
    assert_eq!(record.execution().verifier().name().as_str(), "verus");
    assert_eq!(record.execution().solver().name().as_str(), "z3");
    assert_eq!(
        record.execution().evidence_recorder().name().as_str(),
        "fe2o3-recorder"
    );
    assert_eq!(
        record.execution().invocation_digest().algorithm(),
        DigestAlgorithm::Sha256
    );
    assert_eq!(
        record.execution().invocation_digest().bytes().as_bytes(),
        canonical_invocation_digest(&plan).as_bytes()
    );
    assert_eq!(
        record.trusted_items()[0].name().as_str(),
        "gpu_integer_model"
    );
    assert_eq!(
        record.trusted_items()[0]
            .contract_digest()
            .bytes()
            .as_bytes(),
        digest(40).as_bytes()
    );
}

#[test]
fn failed_and_timeout_results_remain_non_proof_evidence() {
    let plan = plan();
    for (wire, expected) in [
        ("failed", ArtifactOutcome::Failed),
        ("timed-out", ArtifactOutcome::TimedOut),
    ] {
        let evidence =
            convert_to_artifact_proof_record(&plan, &result(&plan, wire), review(&plan)).unwrap();
        assert_eq!(evidence.record().outcome(), expected);
        assert!(evidence.record().proved_properties().is_empty());
        assert_eq!(evidence.record().trusted_items().len(), 1);
    }
}

#[test]
fn conversion_is_deterministic_down_to_artifact_wire_bytes() {
    let left_plan = plan();
    let right_plan = plan();
    let left = convert_to_artifact_proof_record(
        &left_plan,
        &result(&left_plan, "proved"),
        review(&left_plan),
    )
    .unwrap();
    let right = convert_to_artifact_proof_record(
        &right_plan,
        &result(&right_plan, "proved"),
        review(&right_plan),
    )
    .unwrap();

    assert_eq!(left, right);
    assert_eq!(left.record().to_bytes(), right.record().to_bytes());
}

#[test]
fn correlation_and_reviewed_invocation_digest_reject_stale_evidence() {
    let original = plan();
    let result = result(&original, "proved");
    let changed_correlation = plan_with(
        correlation(51),
        target(),
        configuration(),
        model(),
        original.request().properties().to_vec(),
        original.request().trusted_items().to_vec(),
        tools(),
    );
    assert_eq!(
        convert_to_artifact_proof_record(
            &changed_correlation,
            &result,
            review(&changed_correlation)
        ),
        Err(ArtifactRecordConversionError::CorrelationMismatch)
    );

    let wrong_review = ReviewedInvocationIdentityV1::new(correlation(50), digest(99));
    assert_eq!(
        convert_to_artifact_proof_record(&original, &result, wrong_review),
        Err(ArtifactRecordConversionError::InvocationDigestMismatch)
    );
}

#[test]
fn malformed_reviewed_digest_is_rejected_before_conversion() {
    for malformed in ["", "12", &"AA".repeat(32), &"gg".repeat(32)] {
        assert_eq!(
            ReviewedInvocationIdentityV1::from_hex(correlation(50), malformed),
            Err(ArtifactRecordConversionError::MalformedInvocationDigest)
        );
    }
}

#[test]
fn stale_target_configuration_model_tools_and_trusted_items_are_rejected() {
    let original = plan();
    let result = result(&original, "proved");

    let mut stale_target = target();
    stale_target.memory_contract_digest = digest(70);
    let target_plan = plan_with(
        correlation(50),
        stale_target,
        configuration(),
        model(),
        original.request().properties().to_vec(),
        original.request().trusted_items().to_vec(),
        tools(),
    );
    assert_eq!(
        convert_to_artifact_proof_record(&target_plan, &result, review(&target_plan)),
        Err(ArtifactRecordConversionError::IdentityMismatch {
            field: "proof target"
        })
    );

    let stale_configuration =
        Configuration::new(vec![ConfigurationEntry::new("solver", "cvc5").unwrap()]).unwrap();
    let configuration_plan = plan_with(
        correlation(50),
        target(),
        stale_configuration,
        model(),
        original.request().properties().to_vec(),
        original.request().trusted_items().to_vec(),
        tools(),
    );
    assert_eq!(
        convert_to_artifact_proof_record(&configuration_plan, &result, review(&configuration_plan)),
        Err(ArtifactRecordConversionError::IdentityMismatch {
            field: "proof configuration"
        })
    );

    let model_plan = plan_with(
        correlation(50),
        target(),
        configuration(),
        VerificationModelIdentity::new("gpu-model-v2", digest(20)).unwrap(),
        original.request().properties().to_vec(),
        original.request().trusted_items().to_vec(),
        tools(),
    );
    assert_eq!(
        convert_to_artifact_proof_record(&model_plan, &result, review(&model_plan)),
        Err(ArtifactRecordConversionError::IdentityMismatch {
            field: "verification model"
        })
    );

    let changed_tools = ExecutionTools::new(
        tool("verus", 60),
        tool("z3", 32),
        tool("fe2o3-recorder", 34),
    );
    let tools_plan = plan_with(
        correlation(50),
        target(),
        configuration(),
        model(),
        original.request().properties().to_vec(),
        original.request().trusted_items().to_vec(),
        changed_tools,
    );
    assert_eq!(
        convert_to_artifact_proof_record(&tools_plan, &result, review(&tools_plan)),
        Err(ArtifactRecordConversionError::IdentityMismatch {
            field: "measured tools"
        })
    );

    let trusted_plan = plan_with(
        correlation(50),
        target(),
        configuration(),
        model(),
        original.request().properties().to_vec(),
        vec![trusted("gpu_integer_model", 41)],
        tools(),
    );
    assert_eq!(
        convert_to_artifact_proof_record(&trusted_plan, &result, review(&trusted_plan)),
        Err(ArtifactRecordConversionError::IdentityMismatch {
            field: "trusted items"
        })
    );
}

#[test]
fn property_subsets_and_supersets_are_rejected() {
    let full = plan();
    let full_result = result(&full, "proved");
    let subset = plan_with(
        correlation(50),
        target(),
        configuration(),
        model(),
        vec![ProofProperty::Bounds],
        full.request().trusted_items().to_vec(),
        tools(),
    );
    assert_eq!(
        convert_to_artifact_proof_record(&subset, &full_result, review(&subset)),
        Err(ArtifactRecordConversionError::PropertyMismatch)
    );

    let subset_result = result(&subset, "proved");
    assert_eq!(
        convert_to_artifact_proof_record(&full, &subset_result, review(&full)),
        Err(ArtifactRecordConversionError::PropertyMismatch)
    );
}

#[test]
fn zero_sentinel_target_model_tool_and_trusted_digests_are_rejected() {
    let zero = Digest::from_bytes([0; 32]);

    let mut missing_target = target();
    missing_target.type_layout_digest = zero;
    let target_plan = plan_with(
        correlation(50),
        missing_target,
        configuration(),
        model(),
        vec![ProofProperty::Bounds],
        vec![],
        tools(),
    );
    assert_eq!(
        convert_to_artifact_proof_record(
            &target_plan,
            &result(&target_plan, "failed"),
            review(&target_plan)
        ),
        Err(ArtifactRecordConversionError::UnmeasuredIdentity(
            "type-layout identity"
        ))
    );

    let model_plan = plan_with(
        correlation(50),
        target(),
        configuration(),
        VerificationModelIdentity::new("gpu-model-v1", zero).unwrap(),
        vec![ProofProperty::Bounds],
        vec![],
        tools(),
    );
    assert_eq!(
        convert_to_artifact_proof_record(
            &model_plan,
            &result(&model_plan, "failed"),
            review(&model_plan)
        ),
        Err(ArtifactRecordConversionError::UnmeasuredIdentity(
            "verification-model axioms"
        ))
    );

    let unmeasured_tools = ExecutionTools::new(
        MeasuredToolIdentity::new("verus", "1.0.0", zero, digest(31)).unwrap(),
        tool("z3", 32),
        tool("fe2o3-recorder", 34),
    );
    let tool_plan = plan_with(
        correlation(50),
        target(),
        configuration(),
        model(),
        vec![ProofProperty::Bounds],
        vec![],
        unmeasured_tools,
    );
    assert_eq!(
        convert_to_artifact_proof_record(
            &tool_plan,
            &result(&tool_plan, "failed"),
            review(&tool_plan)
        ),
        Err(ArtifactRecordConversionError::UnmeasuredIdentity(
            "verifier"
        ))
    );

    let trusted_plan = plan_with(
        correlation(50),
        target(),
        configuration(),
        model(),
        vec![ProofProperty::Bounds],
        vec![TrustedItem::new("unmeasured_axiom", zero).unwrap()],
        tools(),
    );
    assert_eq!(
        convert_to_artifact_proof_record(
            &trusted_plan,
            &result(&trusted_plan, "failed"),
            review(&trusted_plan)
        ),
        Err(ArtifactRecordConversionError::UnmeasuredIdentity(
            "trusted-item contract"
        ))
    );
}

#[test]
fn result_correlation_is_retained_by_the_strict_parser() {
    let plan = plan();
    let result = result(&plan, "failed");
    assert_eq!(result.correlation_id(), plan.request().correlation_id());
    assert_eq!(result.outcome(), ProofOutcome::Failed);
}
