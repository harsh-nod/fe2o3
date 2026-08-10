use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_verifier::{
    ALPHA_ZETA_PERMISSION_MODEL_PATH_V1, ALPHA_ZETA_PROOF_HARNESS_PATH_V1,
    ALPHA_ZETA_RUST_MODEL_PATH_V1, ALPHA_ZETA_SHARED_BODY_PATH_V1, AlphaZetaExecutionReviewV1,
    AlphaZetaProofErrorV1, AlphaZetaProofSourcesV1, AlphaZetaReviewLedgerV1,
    AlphaZetaSourceFileIdentityV1, AlphaZetaSourceRoleV1, AlphaZetaTrustedConstructKindV1,
    AxiomPolicy, CorrelationId, Digest, GFX942_ALPHA_ZETA_MODEL_VERSION_V1,
    GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1, GFX942_XNACK_MINUS_TARGET_V1,
    Gfx942AlphaZetaKernelV1, Gfx942AlphaZetaProofInputV1, Gfx942XnackMinusTargetIdentityV1,
    MeasuredToolIdentity, ProofCapsuleDependencyV1, ProofCapsuleExecutionV1,
    ProofCapsuleFreshnessIdentityV1, ProofCapsulePayloadIdentityV1, ProofCapsulePolicyV1,
    ProofCapsuleResultV1, ProofCapsuleTargetV1, ProofCapsuleV1, ProofOutcome, ProofProperty,
    ProofTargetIdentity, ReviewedAlphaZetaProofSetV1, Text, TrustedItem, VerificationModelIdentity,
    record_descriptive_alpha_zeta_execution_v1,
};

const SHARED_BODY: &[u8] =
    include_bytes!("../../../examples/verus_vecadd/src/two_kernel_bodies.rs");
const RUST_MODEL: &[u8] = include_bytes!("../../../examples/verus_vecadd/src/lib.rs");
const PERMISSION_MODEL: &[u8] =
    include_bytes!("../../../examples/verus_vecadd/verus/permission_core.rs");
const PROOF_HARNESS: &[u8] = include_bytes!("../../../examples/verus_vecadd/verus/two_kernel.rs");

fn digest(seed: u8) -> Digest {
    Digest::from_bytes([seed; 32])
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sources() -> AlphaZetaProofSourcesV1 {
    AlphaZetaProofSourcesV1::discover_workspace(workspace_root()).unwrap()
}

fn target(sources: &AlphaZetaProofSourcesV1, kernel_seed: u8) -> ProofTargetIdentity {
    ProofTargetIdentity {
        kernel_id: digest(kernel_seed),
        instance_digest: digest(kernel_seed + 1),
        source_tree_digest: sources.source_tree_identity(),
        crate_graph_digest: sources.dependency_tree_identity(),
        executable_digest: digest(20),
        environment_digest: Digest::from_bytes(
            *Gfx942XnackMinusTargetIdentityV1::canonical()
                .publication()
                .as_bytes(),
        ),
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
        sources(),
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
    dependency_mutation: DependencyMutation,
    target_profile: &str,
) -> ProofCapsuleV1 {
    proof_with_trusted_items(
        input,
        freshness,
        properties,
        dependency_mutation,
        target_profile,
        vec![],
    )
}

fn proof_with_trusted_items(
    input: &Gfx942AlphaZetaProofInputV1,
    freshness: ProofCapsuleFreshnessIdentityV1,
    properties: Vec<ProofProperty>,
    dependency_mutation: DependencyMutation,
    target_profile: &str,
    trusted_items: Vec<TrustedItem>,
) -> ProofCapsuleV1 {
    let mut dependencies = input
        .sources()
        .dependency_bindings()
        .into_iter()
        .map(|(name, digest)| ProofCapsuleDependencyV1::new(name, digest).unwrap())
        .collect::<Vec<_>>();
    match dependency_mutation {
        DependencyMutation::None => {}
        DependencyMutation::Changed => {
            let name = dependencies[0].name().as_str().to_owned();
            dependencies[0] = ProofCapsuleDependencyV1::new(name, digest(99)).unwrap();
        }
        DependencyMutation::Swapped => {
            let first_name = dependencies[0].name().as_str().to_owned();
            let second_name = dependencies[1].name().as_str().to_owned();
            let first_identity = dependencies[0].identity();
            let second_identity = dependencies[1].identity();
            dependencies[0] = ProofCapsuleDependencyV1::new(first_name, second_identity).unwrap();
            dependencies[1] = ProofCapsuleDependencyV1::new(second_name, first_identity).unwrap();
        }
    }
    let target = ProofCapsuleTargetV1::new(
        input.target(),
        dependencies,
        vec![Text::identifier("feature", target_profile).unwrap()],
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
        AxiomPolicy::allow_list(trusted_items.clone()).unwrap(),
        trusted_items,
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
        DependencyMutation::None,
        GFX942_XNACK_MINUS_TARGET_V1,
    )
}

#[derive(Clone, Copy)]
enum DependencyMutation {
    None,
    Changed,
    Swapped,
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
    assert!(!input.has_complete_source_closure());
    assert!(!input.has_complete_verifier_runtime_closure());
    assert!(input.sources().trusted_inventory().constructs().is_empty());
    assert!(
        !input
            .sources()
            .trusted_inventory()
            .unmeasured_imports()
            .is_empty()
    );

    input
        .sources()
        .validate_file(ALPHA_ZETA_SHARED_BODY_PATH_V1, SHARED_BODY)
        .unwrap();
    input
        .sources()
        .validate_file(ALPHA_ZETA_RUST_MODEL_PATH_V1, RUST_MODEL)
        .unwrap();
    input
        .sources()
        .validate_file(ALPHA_ZETA_PERMISSION_MODEL_PATH_V1, PERMISSION_MODEL)
        .unwrap();
    input
        .sources()
        .validate_file(ALPHA_ZETA_PROOF_HARNESS_PATH_V1, PROOF_HARNESS)
        .unwrap();
    assert!(!String::from_utf8_lossy(PERMISSION_MODEL).contains("external_body"));
    assert!(!String::from_utf8_lossy(PROOF_HARNESS).contains("external_body"));
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
    let sources = sources();
    sources.validate_workspace(workspace_root()).unwrap();
    assert!(sources.files().len() > 4);
    assert!(
        sources
            .files()
            .iter()
            .any(|file| file.path().as_str() == "Cargo.lock")
    );
    assert!(sources.files().iter().any(|file| {
        file.path().as_str() == "crates/fe2o3-contracts/src/memory_v1.rs"
            && file.role() == AlphaZetaSourceRoleV1::ContractSource
    }));
    for kind in [
        fe2o3_verifier::AlphaZetaDependencyKindV1::CargoDependency,
        fe2o3_verifier::AlphaZetaDependencyKindV1::CargoTarget,
        fe2o3_verifier::AlphaZetaDependencyKindV1::RustInclude,
        fe2o3_verifier::AlphaZetaDependencyKindV1::RustModule,
        fe2o3_verifier::AlphaZetaDependencyKindV1::RustPathModule,
    ] {
        assert!(sources.edges().iter().any(|edge| edge.kind() == kind));
    }

    let mut missing = sources.files().to_vec();
    missing.pop();
    assert_eq!(
        sources.validate_declared_files(&missing),
        Err(AlphaZetaProofErrorV1::IncompleteSourceSet)
    );

    let mut extra = sources.files().to_vec();
    extra.push(
        AlphaZetaSourceFileIdentityV1::measure(
            AlphaZetaSourceRoleV1::SharedRustSource,
            "examples/verus_vecadd/src/extra.rs",
            b"fn extra() {}",
        )
        .unwrap(),
    );
    assert_eq!(
        sources.validate_declared_files(&extra),
        Err(AlphaZetaProofErrorV1::IncompleteSourceSet)
    );

    let mut role_swapped = sources.files().to_vec();
    let shared = role_swapped
        .iter_mut()
        .find(|file| file.path().as_str() == ALPHA_ZETA_SHARED_BODY_PATH_V1)
        .unwrap();
    *shared = AlphaZetaSourceFileIdentityV1::measure(
        AlphaZetaSourceRoleV1::PermissionModel,
        ALPHA_ZETA_SHARED_BODY_PATH_V1,
        SHARED_BODY,
    )
    .unwrap();
    assert_eq!(
        sources.validate_declared_files(&role_swapped),
        Err(AlphaZetaProofErrorV1::SourceRoleSubstitution)
    );

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

struct SourceWorkspaceFixture {
    path: PathBuf,
}

impl SourceWorkspaceFixture {
    fn copy_from(sources: &AlphaZetaProofSourcesV1) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-alpha-zeta-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        for file in sources.files() {
            let relative = Path::new(file.path().as_str());
            let destination = path.join(relative);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(workspace_root().join(relative), destination).unwrap();
        }
        Self { path }
    }
}

impl Drop for SourceWorkspaceFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn structural_discovery_rejects_missing_and_unexpected_transitive_sources() {
    let expected = sources();
    let missing = SourceWorkspaceFixture::copy_from(&expected);
    fs::remove_file(missing.path.join("crates/fe2o3-contracts/src/memory_v1.rs")).unwrap();
    assert!(matches!(
        AlphaZetaProofSourcesV1::discover_workspace(&missing.path),
        Err(AlphaZetaProofErrorV1::SourceManifestStructure { .. })
    ));

    let unexpected = SourceWorkspaceFixture::copy_from(&expected);
    let harness = unexpected.path.join(ALPHA_ZETA_PROOF_HARNESS_PATH_V1);
    let mut source = fs::read_to_string(&harness).unwrap();
    source.push_str("\ninclude!(\"unexpected.rs\");\n");
    fs::write(&harness, source).unwrap();
    fs::write(
        unexpected
            .path
            .join("examples/verus_vecadd/verus/unexpected.rs"),
        "pub proof fn unexpected() {}\n",
    )
    .unwrap();
    assert_eq!(
        AlphaZetaProofSourcesV1::discover_workspace(&unexpected.path),
        Err(AlphaZetaProofErrorV1::UnexpectedSourcePath)
    );
}

#[test]
fn discovery_retains_the_measured_snapshot_after_disk_mutation() {
    let expected = sources();
    let fixture = SourceWorkspaceFixture::copy_from(&expected);
    let snapshot = AlphaZetaProofSourcesV1::discover_workspace(&fixture.path).unwrap();
    let measured = snapshot
        .files()
        .iter()
        .find(|file| file.path().as_str() == ALPHA_ZETA_SHARED_BODY_PATH_V1)
        .unwrap();
    let original = measured.snapshot_bytes().to_vec();
    fs::write(
        fixture.path.join(ALPHA_ZETA_SHARED_BODY_PATH_V1),
        b"macro_rules! mutated_after_snapshot { () => {} }\n",
    )
    .unwrap();

    assert_eq!(measured.snapshot_bytes(), original);
    assert_eq!(
        snapshot.validate_workspace(&fixture.path),
        Err(AlphaZetaProofErrorV1::SourceManifestMutation)
    );
    assert!(!snapshot.has_complete_source_closure());
    assert!(!snapshot.has_complete_verifier_runtime_closure());
}

#[cfg(unix)]
#[test]
fn discovery_rejects_workspace_and_source_parent_symlinks() {
    use std::os::unix::fs::symlink;

    let expected = sources();
    let root_fixture = SourceWorkspaceFixture::copy_from(&expected);
    let root_link = root_fixture.path.with_extension("symlink");
    symlink(&root_fixture.path, &root_link).unwrap();
    assert!(matches!(
        AlphaZetaProofSourcesV1::discover_workspace(&root_link),
        Err(AlphaZetaProofErrorV1::SourceManifestStructure { .. })
    ));
    fs::remove_file(root_link).unwrap();

    let parent_fixture = SourceWorkspaceFixture::copy_from(&expected);
    let source_parent = parent_fixture.path.join("crates/fe2o3-contracts/src");
    let real_source_parent = parent_fixture.path.join("crates/fe2o3-contracts/src-real");
    fs::rename(&source_parent, &real_source_parent).unwrap();
    symlink("src-real", &source_parent).unwrap();
    assert!(matches!(
        AlphaZetaProofSourcesV1::discover_workspace(&parent_fixture.path),
        Err(AlphaZetaProofErrorV1::SourceManifestStructure { .. })
    ));
}

#[test]
fn trusted_inventory_is_derived_from_proof_tokens_and_empty_reports_are_rejected() {
    let mutations = [
        (
            "verus! { #[verifier::external_body] proof fn trust_probe() {} }",
            AlphaZetaTrustedConstructKindV1::ExternalBody,
        ),
        (
            "verus! { proof fn trust_probe() { assume(false); } }",
            AlphaZetaTrustedConstructKindV1::Assume,
        ),
        (
            "verus! { proof fn trust_probe() { admit(); } }",
            AlphaZetaTrustedConstructKindV1::Admit,
        ),
        (
            "verus! { #[verifier::external_fn_specification] fn trust_probe() {} }",
            AlphaZetaTrustedConstructKindV1::TrustedAttribute,
        ),
        (
            "use vstd::prelude::assume;",
            AlphaZetaTrustedConstructKindV1::TrustedImport,
        ),
    ];

    for (index, (mutation, expected_kind)) in mutations.into_iter().enumerate() {
        let expected = sources();
        let fixture = SourceWorkspaceFixture::copy_from(&expected);
        let harness = fixture.path.join(ALPHA_ZETA_PROOF_HARNESS_PATH_V1);
        let mut source = fs::read_to_string(&harness).unwrap();
        source.push('\n');
        source.push_str(mutation);
        source.push('\n');
        fs::write(harness, source).unwrap();

        let mutated = AlphaZetaProofSourcesV1::discover_workspace(&fixture.path).unwrap();
        assert!(mutated.trusted_inventory().constructs().iter().any(|item| {
            item.kind() == expected_kind
                && item.source_path().as_str() == ALPHA_ZETA_PROOF_HARNESS_PATH_V1
        }));
        assert!(!mutated.trusted_inventory().trusted_items().is_empty());

        let input = input_with(
            Gfx942AlphaZetaKernelV1::Alpha,
            mutated,
            digest(120 + index as u8),
            digest(110 + index as u8),
            tool("verus", "0.2026.08.02", 35),
        );
        let fresh = freshness(1, digest(80), digest(81), 10 + index as u8);
        let empty_report = proof_with_trusted_items(
            &input,
            fresh,
            GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1.to_vec(),
            DependencyMutation::None,
            GFX942_XNACK_MINUS_TARGET_V1,
            vec![],
        );
        assert_eq!(
            record_descriptive_alpha_zeta_execution_v1(
                &input,
                &empty_report,
                review(&input, &empty_report, fresh, 90 + index as u8),
                &mut AlphaZetaReviewLedgerV1::new(),
            ),
            Err(AlphaZetaProofErrorV1::TrustedInventorySubstitution)
        );
    }
}

#[test]
fn reviewed_result_binds_properties_freshness_and_rejects_replay() {
    let input = input(Gfx942AlphaZetaKernelV1::Alpha, 60, 61);
    let fresh = freshness(1, digest(80), digest(81), 10);
    let proof = proof(&input, fresh);
    let exact_review = review(&input, &proof, fresh, 71);
    let mut ledger = AlphaZetaReviewLedgerV1::new();
    let recorded =
        record_descriptive_alpha_zeta_execution_v1(&input, &proof, exact_review, &mut ledger)
            .unwrap();
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
        record_descriptive_alpha_zeta_execution_v1(&input, &proof, exact_review, &mut ledger),
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
        DependencyMutation::Changed,
        GFX942_XNACK_MINUS_TARGET_V1,
    );
    assert_eq!(
        record_descriptive_alpha_zeta_execution_v1(
            &input,
            &wrong_dependencies,
            review(&input, &wrong_dependencies, fresh, 71),
            &mut AlphaZetaReviewLedgerV1::new(),
        ),
        Err(AlphaZetaProofErrorV1::DependencySubstitution)
    );

    let swapped_dependencies = proof_with(
        &input,
        fresh,
        GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1.to_vec(),
        DependencyMutation::Swapped,
        GFX942_XNACK_MINUS_TARGET_V1,
    );
    assert_eq!(
        record_descriptive_alpha_zeta_execution_v1(
            &input,
            &swapped_dependencies,
            review(&input, &swapped_dependencies, fresh, 72),
            &mut AlphaZetaReviewLedgerV1::new(),
        ),
        Err(AlphaZetaProofErrorV1::DependencySubstitution)
    );

    let wrong_target_profile = proof_with(
        &input,
        fresh,
        GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1.to_vec(),
        DependencyMutation::None,
        "gfx941",
    );
    assert_eq!(
        record_descriptive_alpha_zeta_execution_v1(
            &input,
            &wrong_target_profile,
            review(&input, &wrong_target_profile, fresh, 73),
            &mut AlphaZetaReviewLedgerV1::new(),
        ),
        Err(AlphaZetaProofErrorV1::TargetProfileSubstitution)
    );

    let subset = vec![ProofProperty::Bounds, ProofProperty::RaceFreedom];
    let wrong_properties = proof_with(
        &input,
        fresh,
        subset,
        DependencyMutation::None,
        GFX942_XNACK_MINUS_TARGET_V1,
    );
    assert_eq!(
        record_descriptive_alpha_zeta_execution_v1(
            &input,
            &wrong_properties,
            review(&input, &wrong_properties, fresh, 72),
            &mut AlphaZetaReviewLedgerV1::new(),
        ),
        Err(AlphaZetaProofErrorV1::PropertySubstitution)
    );

    let trusted_item = TrustedItem::new("external_body", digest(97)).unwrap();
    let wrong_trust_inventory = proof_with_trusted_items(
        &input,
        fresh,
        GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1.to_vec(),
        DependencyMutation::None,
        GFX942_XNACK_MINUS_TARGET_V1,
        vec![trusted_item],
    );
    assert_eq!(
        record_descriptive_alpha_zeta_execution_v1(
            &input,
            &wrong_trust_inventory,
            review(&input, &wrong_trust_inventory, fresh, 74),
            &mut AlphaZetaReviewLedgerV1::new(),
        ),
        Err(AlphaZetaProofErrorV1::TrustedInventorySubstitution)
    );

    let exact = proof(&input, fresh);
    let substituted = freshness(2, digest(81), digest(82), 20);
    assert_eq!(
        record_descriptive_alpha_zeta_execution_v1(
            &input,
            &exact,
            review(&input, &exact, substituted, 73),
            &mut AlphaZetaReviewLedgerV1::new(),
        ),
        Err(AlphaZetaProofErrorV1::FreshnessSubstitution)
    );
}

#[test]
fn reviewed_result_rejects_tool_and_review_expectation_substitution() {
    let expected = input(Gfx942AlphaZetaKernelV1::Alpha, 60, 61);
    let substituted_input = input_with(
        Gfx942AlphaZetaKernelV1::Alpha,
        sources(),
        digest(60),
        digest(61),
        tool("verus", "0.2026.08.11", 45),
    );
    let fresh = freshness(1, digest(80), digest(81), 10);
    let substituted_proof = proof(&substituted_input, fresh);
    let substituted_review = AlphaZetaExecutionReviewV1::new(
        expected.identity(),
        substituted_proof.identity(),
        fresh,
        digest(70),
        digest(71),
    )
    .unwrap();
    assert_eq!(
        record_descriptive_alpha_zeta_execution_v1(
            &expected,
            &substituted_proof,
            substituted_review,
            &mut AlphaZetaReviewLedgerV1::new(),
        ),
        Err(AlphaZetaProofErrorV1::ToolSubstitution)
    );

    let exact_proof = proof(&expected, fresh);
    let wrong_input_review = AlphaZetaExecutionReviewV1::new(
        digest(99),
        exact_proof.identity(),
        fresh,
        digest(70),
        digest(72),
    )
    .unwrap();
    assert_eq!(
        record_descriptive_alpha_zeta_execution_v1(
            &expected,
            &exact_proof,
            wrong_input_review,
            &mut AlphaZetaReviewLedgerV1::new(),
        ),
        Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "reviewed input"
        })
    );
}

#[test]
fn abi_effects_and_launch_each_change_the_sealed_input_identity() {
    let baseline = input(Gfx942AlphaZetaKernelV1::Alpha, 60, 61);
    let make = |abi: Digest, effects: Digest, launch: Digest| {
        let sources = sources();
        let mut proof_target = target(&sources, 1);
        proof_target.effects_contract_digest = effects;
        Gfx942AlphaZetaProofInputV1::seal(
            Gfx942AlphaZetaKernelV1::Alpha,
            sources,
            proof_target,
            abi,
            effects,
            launch,
            tool("verus", "0.2026.08.10", 35),
            tool("z3", "4.12.5", 37),
            model(),
            digest(60),
            digest(61),
        )
        .unwrap()
    };
    let changed_abi = make(digest(99), digest(25), digest(40));
    let changed_effects = make(digest(31), digest(98), digest(40));
    let changed_launch = make(digest(31), digest(25), digest(97));
    assert_ne!(baseline.identity(), changed_abi.identity());
    assert_ne!(baseline.identity(), changed_effects.identity());
    assert_ne!(baseline.identity(), changed_launch.identity());
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
    let alpha_record = record_descriptive_alpha_zeta_execution_v1(
        alpha,
        &alpha_proof,
        review(alpha, &alpha_proof, first_fresh, 71),
        &mut ledger,
    )
    .unwrap();
    let zeta_record = record_descriptive_alpha_zeta_execution_v1(
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
fn two_kernel_set_rejects_reused_proof_binding_identity_across_ledgers() {
    let alpha = input(Gfx942AlphaZetaKernelV1::Alpha, 60, 61);
    let zeta = input(Gfx942AlphaZetaKernelV1::Zeta, 60, 62);
    let alpha_fresh = freshness(1, digest(80), digest(81), 10);
    let zeta_fresh = ProofCapsuleFreshnessIdentityV1::new_inert(
        alpha_fresh.proof_binding_identity(),
        digest(21),
        digest(22),
        digest(23),
        digest(90),
        digest(81),
        2,
        digest(82),
        digest(24),
    )
    .unwrap();
    let alpha_proof = proof(&alpha, alpha_fresh);
    let zeta_proof = proof(&zeta, zeta_fresh);
    let alpha_record = record_descriptive_alpha_zeta_execution_v1(
        &alpha,
        &alpha_proof,
        review(&alpha, &alpha_proof, alpha_fresh, 71),
        &mut AlphaZetaReviewLedgerV1::new(),
    )
    .unwrap();
    let zeta_record = record_descriptive_alpha_zeta_execution_v1(
        &zeta,
        &zeta_proof,
        review(&zeta, &zeta_proof, zeta_fresh, 72),
        &mut AlphaZetaReviewLedgerV1::new(),
    )
    .unwrap();

    assert_eq!(
        ReviewedAlphaZetaProofSetV1::new(alpha_record, zeta_record),
        Err(AlphaZetaProofErrorV1::MixedProofSet)
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
    record_descriptive_alpha_zeta_execution_v1(
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
        record_descriptive_alpha_zeta_execution_v1(
            &second,
            &second_proof,
            review(&second, &second_proof, second_fresh, 72),
            &mut ledger,
        ),
        Err(AlphaZetaProofErrorV1::ReviewCapacityReached)
    );
}
