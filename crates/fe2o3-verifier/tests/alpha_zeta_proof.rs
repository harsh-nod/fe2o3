use fe2o3_verifier::{
    ALPHA_ZETA_PERMISSION_MODEL_PATH_V1, ALPHA_ZETA_PROOF_HARNESS_PATH_V1,
    ALPHA_ZETA_SHARED_BODY_PATH_V1, AlphaZetaExecutionReviewV1, AlphaZetaProofErrorV1,
    AlphaZetaProofSourcesV1, AlphaZetaReviewLedgerV1, AlphaZetaSourceFileIdentityV1, AxiomPolicy,
    CorrelationId, Digest, GFX942_ALPHA_ZETA_MODEL_VERSION_V1,
    GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1, Gfx942AlphaZetaKernelV1, Gfx942AlphaZetaProofInputV1,
    MeasuredToolIdentity, ProofCapsuleDependencyV1, ProofCapsuleExecutionV1,
    ProofCapsuleFreshnessIdentityV1, ProofCapsulePayloadIdentityV1, ProofCapsulePolicyV1,
    ProofCapsuleResultV1, ProofCapsuleTargetV1, ProofCapsuleV1, ProofOutcome, ProofProperty,
    ProofTargetIdentity, ReviewedAlphaZetaProofSetV1, Text, VerificationModelIdentity,
    record_reviewed_alpha_zeta_execution_v1,
};

const SHARED_BODY: &[u8] =
    include_bytes!("../../../examples/verus_vecadd/src/two_kernel_bodies.rs");
const PERMISSION_MODEL: &[u8] = include_bytes!("../../../examples/verus_vecadd/verus/vecadd.rs");
const PROOF_HARNESS: &[u8] = include_bytes!("../../../examples/verus_vecadd/verus/two_kernel.rs");

fn digest(seed: u8) -> Digest {
    Digest::from_bytes([seed; 32])
}

fn sources_with(shared_body: &[u8]) -> AlphaZetaProofSourcesV1 {
    AlphaZetaProofSourcesV1::new(vec![
        AlphaZetaSourceFileIdentityV1::measure(ALPHA_ZETA_SHARED_BODY_PATH_V1, shared_body)
            .unwrap(),
        AlphaZetaSourceFileIdentityV1::measure(
            ALPHA_ZETA_PERMISSION_MODEL_PATH_V1,
            PERMISSION_MODEL,
        )
        .unwrap(),
        AlphaZetaSourceFileIdentityV1::measure(ALPHA_ZETA_PROOF_HARNESS_PATH_V1, PROOF_HARNESS)
            .unwrap(),
    ])
    .unwrap()
}

fn target(sources: &AlphaZetaProofSourcesV1, kernel_seed: u8) -> ProofTargetIdentity {
    ProofTargetIdentity {
        kernel_id: digest(kernel_seed),
        instance_digest: digest(kernel_seed + 1),
        source_tree_digest: sources.source_tree_identity(),
        crate_graph_digest: sources.dependency_tree_identity(),
        executable_digest: digest(20),
        environment_digest: digest(21),
        artifact_selection_digest: digest(22),
        artifact_contract_digest: digest(23),
        memory_contract_digest: digest(24),
        effects_contract_digest: digest(25),
        type_layout_digest: digest(26),
        capability_semantics_digest: digest(27),
        functional_specification_digest: digest(28),
    }
}

fn tool(name: &str, version: &str, seed: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(name, version, digest(seed), digest(seed + 1)).unwrap()
}

fn model() -> VerificationModelIdentity {
    VerificationModelIdentity::new(GFX942_ALPHA_ZETA_MODEL_VERSION_V1, digest(34)).unwrap()
}

fn input_with(
    kernel: Gfx942AlphaZetaKernelV1,
    sources: AlphaZetaProofSourcesV1,
    proof_set_nonce: Digest,
    proof_nonce: Digest,
    verus: MeasuredToolIdentity,
) -> Gfx942AlphaZetaProofInputV1 {
    let kernel_seed = match kernel {
        Gfx942AlphaZetaKernelV1::Alpha => 1,
        Gfx942AlphaZetaKernelV1::Zeta => 3,
    };
    let proof_target = target(&sources, kernel_seed);
    Gfx942AlphaZetaProofInputV1::seal(
        kernel,
        sources,
        proof_target,
        digest(30 + kernel_seed),
        digest(25),
        digest(40),
        verus,
        tool("z3", "4.12.5", 37),
        model(),
        proof_set_nonce,
        proof_nonce,
    )
    .unwrap()
}

fn input(
    kernel: Gfx942AlphaZetaKernelV1,
    proof_set_seed: u8,
    proof_seed: u8,
) -> Gfx942AlphaZetaProofInputV1 {
    input_with(
        kernel,
        sources_with(SHARED_BODY),
        digest(proof_set_seed),
        digest(proof_seed),
        tool("verus", "0.2026.08.10", 35),
    )
}

fn freshness(
    generation: u64,
    previous_state: Digest,
    state: Digest,
    seed: u8,
) -> ProofCapsuleFreshnessIdentityV1 {
    ProofCapsuleFreshnessIdentityV1::new_inert(
        digest(seed),
        digest(seed + 1),
        digest(seed + 2),
        digest(seed + 3),
        digest(90),
        previous_state,
        generation,
        state,
        digest(seed + 4),
    )
    .unwrap()
}

fn proof_with(
    input: &Gfx942AlphaZetaProofInputV1,
    freshness: ProofCapsuleFreshnessIdentityV1,
    properties: Vec<ProofProperty>,
    dependency_mutation: bool,
) -> ProofCapsuleV1 {
    let mut dependencies = input
        .sources()
        .files()
        .iter()
        .map(|file| {
            let name = match file.path().as_str() {
                ALPHA_ZETA_SHARED_BODY_PATH_V1 => "shared-body",
                ALPHA_ZETA_PERMISSION_MODEL_PATH_V1 => "permission-model",
                ALPHA_ZETA_PROOF_HARNESS_PATH_V1 => "proof-harness",
                _ => unreachable!(),
            };
            ProofCapsuleDependencyV1::new(name, file.digest()).unwrap()
        })
        .collect::<Vec<_>>();
    if dependency_mutation {
        let name = dependencies[0].name().as_str().to_owned();
        dependencies[0] = ProofCapsuleDependencyV1::new(name, digest(99)).unwrap();
    }
    let target = ProofCapsuleTargetV1::new(
        input.target(),
        dependencies,
        vec![Text::identifier("feature", "gfx942").unwrap()],
        input.abi_identity(),
        input.launch_identity(),
        digest(50),
        digest(51),
        digest(52),
    )
    .unwrap();
    let policy = ProofCapsulePolicyV1::new(
        input.model().clone(),
        input.verus().clone(),
        input.solver().clone(),
        AxiomPolicy::deny_all(),
        vec![],
        properties.clone(),
    )
    .unwrap();
    let execution = ProofCapsuleExecutionV1::new_inert(
        CorrelationId::from_bytes([freshness.ledger_generation() as u8; 16]),
        digest(53),
        digest(54),
        digest(55),
        freshness.challenge(),
        freshness.transcript(),
        ProofCapsulePayloadIdentityV1::sealed_result(128, freshness.result()).unwrap(),
        Some(freshness),
    )
    .unwrap();
    ProofCapsuleV1::new_inert(
        target,
        policy,
        execution,
        ProofCapsuleResultV1::new(ProofOutcome::Proved, properties).unwrap(),
    )
    .unwrap()
}

fn proof(
    input: &Gfx942AlphaZetaProofInputV1,
    freshness: ProofCapsuleFreshnessIdentityV1,
) -> ProofCapsuleV1 {
    proof_with(
        input,
        freshness,
        GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1.to_vec(),
        false,
    )
}

fn review(
    input: &Gfx942AlphaZetaProofInputV1,
    proof: &ProofCapsuleV1,
    freshness: ProofCapsuleFreshnessIdentityV1,
    review_seed: u8,
) -> AlphaZetaExecutionReviewV1 {
    AlphaZetaExecutionReviewV1::new(
        input.identity(),
        proof.identity(),
        freshness,
        digest(70),
        digest(review_seed),
    )
    .unwrap()
}

#[test]
fn exact_source_capsule_binds_real_files_and_all_identity_axes() {
    let input = input(Gfx942AlphaZetaKernelV1::Alpha, 60, 61);
    assert_eq!(input.kernel().as_str(), "alpha");
    assert_eq!(
        input.requested_properties(),
        &GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1
    );
    assert!(input.to_canonical_bytes().len() > 32);
    assert!(!input.grants_launch_authority());
    assert!(!input.proves_ieee_f32_refinement());
    assert!(!input.proves_compiler_to_machine_refinement());

    input
        .sources()
        .validate_file(ALPHA_ZETA_SHARED_BODY_PATH_V1, SHARED_BODY)
        .unwrap();
    let mut mutated = SHARED_BODY.to_vec();
    mutated[0] ^= 1;
    assert_eq!(
        input
            .sources()
            .validate_file(ALPHA_ZETA_SHARED_BODY_PATH_V1, &mutated),
        Err(AlphaZetaProofErrorV1::SourceMutation)
    );
}

#[test]
fn source_set_and_capsule_construction_reject_substitution() {
    let shared =
        AlphaZetaSourceFileIdentityV1::measure(ALPHA_ZETA_SHARED_BODY_PATH_V1, SHARED_BODY)
            .unwrap();
    assert_eq!(
        AlphaZetaProofSourcesV1::new(vec![shared.clone(), shared.clone(), shared]),
        Err(AlphaZetaProofErrorV1::DuplicateSourcePath)
    );
    assert_eq!(
        AlphaZetaSourceFileIdentityV1::measure("examples/other.rs", b"x"),
        Err(AlphaZetaProofErrorV1::UnexpectedSourcePath)
    );

    let sources = sources_with(SHARED_BODY);
    let mut wrong_target = target(&sources, 1);
    wrong_target.source_tree_digest = digest(99);
    assert_eq!(
        Gfx942AlphaZetaProofInputV1::seal(
            Gfx942AlphaZetaKernelV1::Alpha,
            sources,
            wrong_target,
            digest(31),
            digest(25),
            digest(40),
            tool("verus", "0.2026.08.10", 35),
            tool("z3", "4.12.5", 37),
            model(),
            digest(60),
            digest(61),
        ),
        Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "source tree"
        })
    );
}

#[test]
fn reviewed_result_binds_properties_freshness_and_rejects_replay() {
    let input = input(Gfx942AlphaZetaKernelV1::Alpha, 60, 61);
    let fresh = freshness(1, digest(80), digest(81), 10);
    let proof = proof(&input, fresh);
    let exact_review = review(&input, &proof, fresh, 71);
    let mut ledger = AlphaZetaReviewLedgerV1::new();
    let recorded =
        record_reviewed_alpha_zeta_execution_v1(&input, &proof, exact_review, &mut ledger).unwrap();
    assert_eq!(ledger.recorded_count(), 1);
    assert_eq!(recorded.kernel(), Gfx942AlphaZetaKernelV1::Alpha);
    assert_eq!(
        recorded.reported_properties(),
        &GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1
    );
    assert!(!recorded.grants_proof_authority());
    assert!(!recorded.grants_launch_authority());
    assert!(!recorded.proves_ieee_f32_refinement());
    assert!(!recorded.proves_compiler_to_machine_refinement());

    assert_eq!(
        record_reviewed_alpha_zeta_execution_v1(&input, &proof, exact_review, &mut ledger),
        Err(AlphaZetaProofErrorV1::Replay)
    );
}

#[test]
fn reviewed_result_rejects_dependency_property_and_freshness_substitution() {
    let input = input(Gfx942AlphaZetaKernelV1::Alpha, 60, 61);
    let fresh = freshness(1, digest(80), digest(81), 10);

    let wrong_dependencies = proof_with(
        &input,
        fresh,
        GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1.to_vec(),
        true,
    );
    assert_eq!(
        record_reviewed_alpha_zeta_execution_v1(
            &input,
            &wrong_dependencies,
            review(&input, &wrong_dependencies, fresh, 71),
            &mut AlphaZetaReviewLedgerV1::new(),
        ),
        Err(AlphaZetaProofErrorV1::DependencySubstitution)
    );

    let subset = vec![ProofProperty::Bounds, ProofProperty::RaceFreedom];
    let wrong_properties = proof_with(&input, fresh, subset, false);
    assert_eq!(
        record_reviewed_alpha_zeta_execution_v1(
            &input,
            &wrong_properties,
            review(&input, &wrong_properties, fresh, 72),
            &mut AlphaZetaReviewLedgerV1::new(),
        ),
        Err(AlphaZetaProofErrorV1::PropertySubstitution)
    );

    let exact = proof(&input, fresh);
    let substituted = freshness(2, digest(81), digest(82), 20);
    assert_eq!(
        record_reviewed_alpha_zeta_execution_v1(
            &input,
            &exact,
            review(&input, &exact, substituted, 73),
            &mut AlphaZetaReviewLedgerV1::new(),
        ),
        Err(AlphaZetaProofErrorV1::FreshnessSubstitution)
    );
}

fn record_pair(
    alpha: &Gfx942AlphaZetaProofInputV1,
    zeta: &Gfx942AlphaZetaProofInputV1,
    second_previous: Digest,
) -> (
    fe2o3_verifier::ReviewedAlphaZetaExecutionV1,
    fe2o3_verifier::ReviewedAlphaZetaExecutionV1,
) {
    let first_fresh = freshness(1, digest(80), digest(81), 10);
    let second_fresh = freshness(2, second_previous, digest(82), 20);
    let alpha_proof = proof(alpha, first_fresh);
    let zeta_proof = proof(zeta, second_fresh);
    let mut ledger = AlphaZetaReviewLedgerV1::new();
    let alpha_record = record_reviewed_alpha_zeta_execution_v1(
        alpha,
        &alpha_proof,
        review(alpha, &alpha_proof, first_fresh, 71),
        &mut ledger,
    )
    .unwrap();
    let zeta_record = record_reviewed_alpha_zeta_execution_v1(
        zeta,
        &zeta_proof,
        review(zeta, &zeta_proof, second_fresh, 72),
        &mut ledger,
    )
    .unwrap();
    (alpha_record, zeta_record)
}

#[test]
fn exact_two_kernel_set_accepts_one_contiguous_history() {
    let alpha = input(Gfx942AlphaZetaKernelV1::Alpha, 60, 61);
    let zeta = input(Gfx942AlphaZetaKernelV1::Zeta, 60, 62);
    let (alpha_record, zeta_record) = record_pair(&alpha, &zeta, digest(81));
    let set = ReviewedAlphaZetaProofSetV1::new(zeta_record, alpha_record).unwrap();
    assert_eq!(set.alpha().kernel(), Gfx942AlphaZetaKernelV1::Alpha);
    assert_eq!(set.zeta().kernel(), Gfx942AlphaZetaKernelV1::Zeta);
    assert!(!set.grants_proof_authority());
    assert!(!set.grants_launch_authority());
}

#[test]
fn two_kernel_set_rejects_mixed_sets_and_forked_history() {
    let alpha = input(Gfx942AlphaZetaKernelV1::Alpha, 60, 61);
    let zeta_other_set = input(Gfx942AlphaZetaKernelV1::Zeta, 63, 62);
    let (alpha_record, zeta_record) = record_pair(&alpha, &zeta_other_set, digest(81));
    assert_eq!(
        ReviewedAlphaZetaProofSetV1::new(alpha_record, zeta_record),
        Err(AlphaZetaProofErrorV1::MixedProofSet)
    );

    let zeta = input(Gfx942AlphaZetaKernelV1::Zeta, 60, 62);
    let (alpha_record, zeta_record) = record_pair(&alpha, &zeta, digest(99));
    assert_eq!(
        ReviewedAlphaZetaProofSetV1::new(alpha_record, zeta_record),
        Err(AlphaZetaProofErrorV1::MixedFreshnessHistory)
    );
}

#[test]
fn review_ledger_is_bounded_and_fail_closed() {
    assert_eq!(
        AlphaZetaReviewLedgerV1::with_max_records(0).unwrap_err(),
        AlphaZetaProofErrorV1::ReviewCapacityOutOfRange
    );
    let first = input(Gfx942AlphaZetaKernelV1::Alpha, 60, 61);
    let fresh = freshness(1, digest(80), digest(81), 10);
    let first_proof = proof(&first, fresh);
    let mut ledger = AlphaZetaReviewLedgerV1::with_max_records(1).unwrap();
    record_reviewed_alpha_zeta_execution_v1(
        &first,
        &first_proof,
        review(&first, &first_proof, fresh, 71),
        &mut ledger,
    )
    .unwrap();

    let second = input(Gfx942AlphaZetaKernelV1::Zeta, 60, 62);
    let second_fresh = freshness(2, digest(81), digest(82), 20);
    let second_proof = proof(&second, second_fresh);
    assert_eq!(
        record_reviewed_alpha_zeta_execution_v1(
            &second,
            &second_proof,
            review(&second, &second_proof, second_fresh, 72),
            &mut ledger,
        ),
        Err(AlphaZetaProofErrorV1::ReviewCapacityReached)
    );
}
