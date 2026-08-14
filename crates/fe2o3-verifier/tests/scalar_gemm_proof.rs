use fe2o3_verifier::{
    AxiomPolicy, CorrelationId, Digest, MeasuredToolIdentity, ProofCapsuleDependencyV1,
    ProofCapsuleExecutionV1, ProofCapsuleFreshnessIdentityV1, ProofCapsulePayloadIdentityV1,
    ProofCapsulePolicyV1, ProofCapsuleResultV1, ProofCapsuleTargetV1, ProofCapsuleV1, ProofOutcome,
    ProofProperty, ProofTargetIdentity, SCALAR_GEMM_PROOF_MODEL_VERSION_V1,
    SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1, SCALAR_GEMM_PROOF_SOURCE_PATH_V1,
    SCALAR_GEMM_PROOF_TARGET_V1, ScalarGemmProofErrorV1, ScalarGemmProofProfileV1,
    ScalarGemmProofReviewLedgerV1, ScalarGemmProofReviewV1, ScalarGemmProofSourceV1, Text,
    VerificationModelIdentity, review_scalar_gemm_proof_v1,
};

const SOURCE: &[u8] = include_bytes!("../verus/scalar_gemm_v1.rs");

fn digest(seed: u8) -> Digest {
    Digest::from_bytes([seed; 32])
}

fn source() -> ScalarGemmProofSourceV1 {
    ScalarGemmProofSourceV1::measure(SOURCE).unwrap()
}

fn target(source: ScalarGemmProofSourceV1) -> ProofTargetIdentity {
    ProofTargetIdentity {
        kernel_id: digest(1),
        instance_digest: digest(2),
        source_tree_digest: source.identity(),
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

fn dependencies() -> Vec<ProofCapsuleDependencyV1> {
    vec![ProofCapsuleDependencyV1::new("vstd", digest(14)).unwrap()]
}

fn verus() -> MeasuredToolIdentity {
    MeasuredToolIdentity::new("verus", "0.2026.08.10", digest(15), digest(16)).unwrap()
}

fn solver() -> MeasuredToolIdentity {
    MeasuredToolIdentity::new("z3", "4.12.5", digest(17), digest(18)).unwrap()
}

fn model() -> VerificationModelIdentity {
    VerificationModelIdentity::new(SCALAR_GEMM_PROOF_MODEL_VERSION_V1, digest(19)).unwrap()
}

fn freshness_with(transcript: Digest, result: Digest) -> ProofCapsuleFreshnessIdentityV1 {
    ProofCapsuleFreshnessIdentityV1::new_inert(
        digest(20),
        digest(21),
        transcript,
        result,
        digest(22),
        digest(23),
        1,
        digest(24),
        digest(25),
    )
    .unwrap()
}

fn freshness() -> ProofCapsuleFreshnessIdentityV1 {
    freshness_with(digest(26), digest(27))
}

fn profile_with(
    source: ScalarGemmProofSourceV1,
    proof_target: ProofTargetIdentity,
    transcript: Digest,
    result: Digest,
) -> Result<ScalarGemmProofProfileV1, ScalarGemmProofErrorV1> {
    ScalarGemmProofProfileV1::seal(
        source,
        proof_target,
        dependencies(),
        digest(28),
        digest(10),
        digest(29),
        digest(30),
        digest(31),
        digest(32),
        verus(),
        solver(),
        model(),
        vec![],
        transcript,
        result,
    )
}

fn profile() -> ScalarGemmProofProfileV1 {
    let source = source();
    profile_with(source, target(source), digest(26), digest(27)).unwrap()
}

#[derive(Clone, Copy)]
enum Mutation {
    None,
    Target,
    Abi,
    Effects,
    Launch,
    MachineEffects,
    FinalizedArtifact,
    Artifact,
    Transcript,
    Result,
    Feature,
    Dependencies,
    Tool,
    Model,
}

fn capsule(
    profile: &ScalarGemmProofProfileV1,
    properties: Vec<ProofProperty>,
    outcome: ProofOutcome,
    mutation: Mutation,
) -> ProofCapsuleV1 {
    let mut proof_target = profile.proof_target();
    if matches!(mutation, Mutation::Target) {
        proof_target.kernel_id = digest(90);
    }
    if matches!(mutation, Mutation::Effects) {
        proof_target.effects_contract_digest = digest(91);
    }
    let dependencies = if matches!(mutation, Mutation::Dependencies) {
        vec![ProofCapsuleDependencyV1::new("vstd", digest(92)).unwrap()]
    } else {
        dependencies()
    };
    let feature = if matches!(mutation, Mutation::Feature) {
        "gfx941:xnack-"
    } else {
        SCALAR_GEMM_PROOF_TARGET_V1
    };
    let target = ProofCapsuleTargetV1::new(
        proof_target,
        dependencies,
        vec![Text::identifier("feature", feature).unwrap()],
        if matches!(mutation, Mutation::Abi) {
            digest(93)
        } else {
            digest(28)
        },
        if matches!(mutation, Mutation::Launch) {
            digest(94)
        } else {
            digest(29)
        },
        if matches!(mutation, Mutation::MachineEffects) {
            digest(95)
        } else {
            digest(30)
        },
        if matches!(mutation, Mutation::FinalizedArtifact) {
            digest(96)
        } else {
            digest(31)
        },
        if matches!(mutation, Mutation::Artifact) {
            digest(97)
        } else {
            digest(32)
        },
    )
    .unwrap();
    let policy = ProofCapsulePolicyV1::new(
        if matches!(mutation, Mutation::Model) {
            VerificationModelIdentity::new(SCALAR_GEMM_PROOF_MODEL_VERSION_V1, digest(98)).unwrap()
        } else {
            model()
        },
        if matches!(mutation, Mutation::Tool) {
            MeasuredToolIdentity::new("verus", "mutated", digest(15), digest(16)).unwrap()
        } else {
            verus()
        },
        solver(),
        AxiomPolicy::deny_all(),
        vec![],
        properties.clone(),
    )
    .unwrap();
    let transcript = if matches!(mutation, Mutation::Transcript) {
        digest(99)
    } else {
        digest(26)
    };
    let result = if matches!(mutation, Mutation::Result) {
        digest(100)
    } else {
        digest(27)
    };
    let freshness = freshness_with(transcript, result);
    let execution = ProofCapsuleExecutionV1::new_inert(
        CorrelationId::from_bytes([1; 16]),
        digest(33),
        digest(34),
        digest(35),
        freshness.challenge(),
        transcript,
        ProofCapsulePayloadIdentityV1::sealed_result(128, result).unwrap(),
        if outcome != ProofOutcome::Proved {
            None
        } else {
            Some(freshness)
        },
    )
    .unwrap();
    let reported_properties = if outcome == ProofOutcome::Proved {
        properties
    } else {
        vec![]
    };
    ProofCapsuleV1::new_inert(
        target,
        policy,
        execution,
        ProofCapsuleResultV1::new(outcome, reported_properties).unwrap(),
    )
    .unwrap()
}

fn exact_capsule(profile: &ScalarGemmProofProfileV1) -> ProofCapsuleV1 {
    capsule(
        profile,
        SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1.to_vec(),
        ProofOutcome::Proved,
        Mutation::None,
    )
}

fn review(
    profile: &ScalarGemmProofProfileV1,
    capsule: &ProofCapsuleV1,
    freshness: ProofCapsuleFreshnessIdentityV1,
    nonce: Digest,
) -> ScalarGemmProofReviewV1 {
    ScalarGemmProofReviewV1::new(
        profile.identity(),
        capsule.identity(),
        freshness,
        digest(36),
        nonce,
    )
    .unwrap()
}

#[test]
fn exact_scalar_profile_binds_source_transcript_result_and_artifact() {
    let source = source();
    let profile = profile();
    let capsule = exact_capsule(&profile);
    let mut ledger = ScalarGemmProofReviewLedgerV1::new();
    let reviewed = review_scalar_gemm_proof_v1(
        &profile,
        &capsule,
        review(&profile, &capsule, freshness(), digest(37)),
        &mut ledger,
    )
    .unwrap();

    assert_eq!(source.path(), SCALAR_GEMM_PROOF_SOURCE_PATH_V1);
    assert_eq!(source.byte_len(), SOURCE.len() as u64);
    assert_eq!(
        source.content_identity(),
        Digest::from_bytes([
            0x98, 0x80, 0x3a, 0x62, 0x48, 0x8e, 0x1a, 0xf2, 0xfb, 0xc8, 0x86, 0xb1, 0xda, 0x5d,
            0xdc, 0x68, 0x0b, 0x16, 0xd3, 0x5a, 0x8a, 0x8a, 0x5c, 0x22, 0xd4, 0x95, 0x91, 0x28,
            0xdd, 0x2d, 0xa5, 0xfe,
        ])
    );
    assert_eq!(reviewed.source_identity(), source.identity());
    assert_eq!(reviewed.proof_target(), profile.proof_target());
    assert_eq!(reviewed.transcript_identity(), digest(26));
    assert_eq!(reviewed.result_identity(), digest(27));
    assert_eq!(reviewed.finalized_artifact_identity(), digest(31));
    assert_eq!(reviewed.artifact_identity(), digest(32));
    assert_eq!(
        reviewed.reported_properties(),
        &SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1
    );
    assert_eq!(ledger.recorded_count(), 1);
    assert!(!profile.authenticates_verus_execution());
    assert!(!profile.grants_proof_authority());
    assert!(!profile.has_complete_source_closure());
    assert!(!profile.has_complete_verifier_runtime_closure());
    assert!(!profile.proves_compiler_refinement());
    assert!(!profile.grants_load_authority());
    assert!(!profile.grants_launch_authority());
    assert!(!reviewed.authenticates_verus_execution());
    assert!(!reviewed.grants_proof_authority());
    assert!(!reviewed.has_complete_source_closure());
    assert!(!reviewed.has_complete_verifier_runtime_closure());
    assert!(!reviewed.proves_compiler_refinement());
    assert!(!reviewed.grants_load_authority());
    assert!(!reviewed.grants_launch_authority());
}

#[test]
fn coherent_source_reprofiling_is_rejected_at_measurement() {
    let mut mutated = SOURCE.to_vec();
    mutated[0] ^= 1;
    assert_eq!(
        ScalarGemmProofSourceV1::measure(&mutated),
        Err(ScalarGemmProofErrorV1::PinnedSourceMismatch {
            expected_byte_len: SOURCE.len() as u64,
            actual_byte_len: SOURCE.len() as u64,
        })
    );
}

#[test]
fn every_target_and_artifact_axis_rejects_substitution() {
    let profile = profile();
    for (mutation, expected_field) in [
        (Mutation::Target, "proof target"),
        (Mutation::Abi, "ABI"),
        (Mutation::Effects, "proof target"),
        (Mutation::Launch, "launch"),
        (Mutation::MachineEffects, "machine-effect evidence"),
        (Mutation::FinalizedArtifact, "finalized artifact"),
        (Mutation::Artifact, "artifact"),
    ] {
        let capsule = capsule(
            &profile,
            SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1.to_vec(),
            ProofOutcome::Proved,
            mutation,
        );
        let mut ledger = ScalarGemmProofReviewLedgerV1::new();
        assert_eq!(
            review_scalar_gemm_proof_v1(
                &profile,
                &capsule,
                review(&profile, &capsule, freshness(), digest(37)),
                &mut ledger,
            ),
            Err(ScalarGemmProofErrorV1::IdentityMismatch {
                field: expected_field
            })
        );
        assert_eq!(ledger.recorded_count(), 0);
    }
}

#[test]
fn target_dependencies_tools_and_model_reject_substitution() {
    let profile = profile();
    for (mutation, expected) in [
        (
            Mutation::Feature,
            ScalarGemmProofErrorV1::TargetProfileSubstitution,
        ),
        (
            Mutation::Dependencies,
            ScalarGemmProofErrorV1::DependencySubstitution,
        ),
        (Mutation::Tool, ScalarGemmProofErrorV1::ToolSubstitution),
        (Mutation::Model, ScalarGemmProofErrorV1::ModelSubstitution),
    ] {
        let capsule = capsule(
            &profile,
            SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1.to_vec(),
            ProofOutcome::Proved,
            mutation,
        );
        let mut ledger = ScalarGemmProofReviewLedgerV1::new();
        assert_eq!(
            review_scalar_gemm_proof_v1(
                &profile,
                &capsule,
                review(&profile, &capsule, freshness(), digest(37)),
                &mut ledger,
            ),
            Err(expected)
        );
    }
}

#[test]
fn transcript_and_result_mutations_are_rejected() {
    let profile = profile();
    for (mutation, field) in [
        (Mutation::Transcript, "Verus transcript"),
        (Mutation::Result, "proof result"),
    ] {
        let capsule = capsule(
            &profile,
            SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1.to_vec(),
            ProofOutcome::Proved,
            mutation,
        );
        let mut ledger = ScalarGemmProofReviewLedgerV1::new();
        assert_eq!(
            review_scalar_gemm_proof_v1(
                &profile,
                &capsule,
                review(
                    &profile,
                    &capsule,
                    capsule.execution().freshness().unwrap(),
                    digest(37)
                ),
                &mut ledger,
            ),
            Err(ScalarGemmProofErrorV1::IdentityMismatch { field })
        );
    }
}

#[test]
fn every_missing_required_property_is_rejected() {
    let profile = profile();
    for missing in SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1 {
        let properties = SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1
            .into_iter()
            .filter(|property| *property != missing)
            .collect();
        let capsule = capsule(&profile, properties, ProofOutcome::Proved, Mutation::None);
        let mut ledger = ScalarGemmProofReviewLedgerV1::new();
        assert_eq!(
            review_scalar_gemm_proof_v1(
                &profile,
                &capsule,
                review(&profile, &capsule, freshness(), digest(37)),
                &mut ledger,
            ),
            Err(ScalarGemmProofErrorV1::PropertySubstitution),
            "missing {}",
            missing.as_str()
        );
    }
}

#[test]
fn non_proved_result_and_freshness_substitution_are_rejected() {
    let profile = profile();
    let failed = capsule(
        &profile,
        SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1.to_vec(),
        ProofOutcome::Failed,
        Mutation::None,
    );
    let exact = exact_capsule(&profile);
    let substituted_freshness = ProofCapsuleFreshnessIdentityV1::new_inert(
        digest(40),
        digest(41),
        digest(42),
        digest(43),
        digest(44),
        digest(45),
        2,
        digest(46),
        digest(47),
    )
    .unwrap();

    let mut ledger = ScalarGemmProofReviewLedgerV1::new();
    assert_eq!(
        review_scalar_gemm_proof_v1(
            &profile,
            &failed,
            review(&profile, &failed, freshness(), digest(37)),
            &mut ledger,
        ),
        Err(ScalarGemmProofErrorV1::ProofOutcomeSubstitution)
    );
    assert_eq!(
        review_scalar_gemm_proof_v1(
            &profile,
            &exact,
            review(&profile, &exact, substituted_freshness, digest(38)),
            &mut ledger,
        ),
        Err(ScalarGemmProofErrorV1::FreshnessSubstitution)
    );
}

#[test]
fn duplicate_is_rejected_within_one_process_local_ledger() {
    let profile = profile();
    let capsule = exact_capsule(&profile);
    let mut ledger = ScalarGemmProofReviewLedgerV1::new();

    review_scalar_gemm_proof_v1(
        &profile,
        &capsule,
        review(&profile, &capsule, freshness(), digest(37)),
        &mut ledger,
    )
    .unwrap();
    assert_eq!(
        review_scalar_gemm_proof_v1(
            &profile,
            &capsule,
            review(&profile, &capsule, freshness(), digest(38)),
            &mut ledger,
        ),
        Err(ScalarGemmProofErrorV1::Replay)
    );
    assert_eq!(ledger.recorded_count(), 1);
}

#[test]
fn replay_is_explicitly_permitted_after_ledger_recreation() {
    let profile = profile();
    let capsule = exact_capsule(&profile);
    let review = review(&profile, &capsule, freshness(), digest(37));

    let first = review_scalar_gemm_proof_v1(
        &profile,
        &capsule,
        review,
        &mut ScalarGemmProofReviewLedgerV1::new(),
    )
    .unwrap();
    let replayed = review_scalar_gemm_proof_v1(
        &profile,
        &capsule,
        review,
        &mut ScalarGemmProofReviewLedgerV1::new(),
    )
    .unwrap();

    assert_eq!(first, replayed);
    assert!(!replayed.grants_proof_authority());
    assert!(!replayed.grants_load_authority());
    assert!(!replayed.grants_launch_authority());
}
