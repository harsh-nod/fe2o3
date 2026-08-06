#[allow(dead_code)]
mod common;

use common::{digest, kernel_with_object_digest, object_identity, target, text};
use fe2o3_artifact_transaction::{
    BuildAttempt, KernelSetIdentityV1, LinkPublicationCatalogV1, LinkPublicationScopeV1,
    PackageIdentityV1, PublicationOutcomeV1, TargetIdentityV1,
};
use fe2o3_artifacts::{
    ArtifactContainerV1, ArtifactDerivedLinkPublicationScopeV1, BundleIndexV1, Capability,
    CodeObjectFormat, CodeObjectIdentity, CodeObjectPayload, CompilerIdentity, DigestAlgorithm,
    DirectLinkBindingExpectationV1, DirectLinkBindingSourceV1, DirectLinkBridgeError,
    DirectLinkBridgeIdentityKindV1, DirectLinkBundleEvidenceV1, DirectLinkDerivedScopeFieldV1,
    DirectLinkFfiClosureIdentityV1, DirectLinkFinalizationIdentityV1,
    DirectLinkFinalizedPayloadIdentityV1, DirectLinkLinkedOutputIdentityV1,
    DirectLinkPublicationBridgeV1, DirectLinkPublicationScopeProvenanceV1,
    DirectLinkRequestIdentityV1, DirectLinkResponseIdentityV1,
    DirectLinkToolchainConfigurationIdentityV1, DirectLinkToolchainExecutableIdentityV1,
    DirectLinkToolchainIdentityV1, DirectLinkTransformationIdentityV1,
    DirectLinkWorkerConfigurationIdentityV1, DirectLinkWorkerExecutableIdentityV1,
    DirectLinkWorkerIdentityV1, Endianness, PayloadDigest, PointerWidth, TargetIdentity,
    ToolIdentity,
};

fn tagged(seed: u8) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, digest(seed))
}

fn attempt(generation: u64, seed: u8) -> BuildAttempt {
    let session = format!("{seed:02x}").repeat(16);
    let invocation = format!("{:02x}", seed.wrapping_add(64)).repeat(32);
    BuildAttempt::from_env_value(&format!("{generation}:{session}:{invocation}")).unwrap()
}

fn scope(seed: u8) -> LinkPublicationScopeV1 {
    LinkPublicationScopeV1::new(
        PackageIdentityV1::from_bytes([seed; 32]),
        KernelSetIdentityV1::from_bytes([seed.wrapping_add(1); 32]),
        TargetIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
    )
}

fn expectation(payload: PayloadDigest, ffi_seed: u8) -> DirectLinkBindingExpectationV1 {
    DirectLinkBindingExpectationV1::new(
        DirectLinkRequestIdentityV1::new(tagged(0x10)),
        DirectLinkWorkerIdentityV1::new(
            text("fe2o3-llvm-link-worker"),
            text("1.0.0"),
            DirectLinkWorkerExecutableIdentityV1::new(tagged(0x11)),
            DirectLinkWorkerConfigurationIdentityV1::new(tagged(0x12)),
        ),
        DirectLinkToolchainIdentityV1::new(
            text("rocm-llvm-lld"),
            text("22.0.0-build.17"),
            DirectLinkToolchainExecutableIdentityV1::new(tagged(0x13)),
            DirectLinkToolchainConfigurationIdentityV1::new(tagged(0x14)),
        ),
        DirectLinkResponseIdentityV1::new(tagged(0x15)),
        DirectLinkTransformationIdentityV1::new(
            DirectLinkLinkedOutputIdentityV1::new(tagged(0x16)),
            DirectLinkFinalizationIdentityV1::new(tagged(0x17)),
            DirectLinkFinalizedPayloadIdentityV1::new(payload),
        ),
        DirectLinkFfiClosureIdentityV1::new(tagged(ffi_seed)),
    )
}

struct EvidenceFixture {
    container: ArtifactContainerV1,
    bundle: BundleIndexV1,
    expectation: DirectLinkBindingExpectationV1,
    evidence: DirectLinkBundleEvidenceV1,
}

impl EvidenceFixture {
    fn validated(&self) -> fe2o3_artifacts::ValidatedDirectLinkBundleEvidenceV1<'_> {
        let sources = [DirectLinkBindingSourceV1::new(
            &self.container,
            self.expectation.clone(),
        )];
        self.evidence
            .validate_against(&self.bundle, &[&self.container], &sources)
            .unwrap()
    }
}

fn evidence(ffi_seed: u8) -> EvidenceFixture {
    evidence_variant(
        b"bridge native payload",
        "1.94.0",
        target(PointerWidth::Bits64, vec![]),
        &[(0x20, "bridge_kernel", "bridge_kernel.kd")],
        ffi_seed,
    )
}

fn evidence_variant(
    payload_bytes: &[u8],
    compiler_version: &str,
    target: TargetIdentity,
    kernels: &[(u8, &str, &str)],
    ffi_seed: u8,
) -> EvidenceFixture {
    let payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, payload_bytes.to_vec()).unwrap();
    let payload_identity = payload.digest();
    let kernels = kernels
        .iter()
        .map(|(id, name, symbol)| {
            kernel_with_object_digest(*id, name, symbol, payload_identity.bytes(), vec![])
        })
        .collect();
    let manifest = fe2o3_artifacts::ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text(compiler_version)),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target,
        vec![object_identity(
            payload_identity.bytes(),
            payload.bytes().len() as u64,
        )],
        kernels,
    )
    .unwrap();
    let container =
        ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload]).unwrap();
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
    let expectation = expectation(payload_identity, ffi_seed);
    let source = DirectLinkBindingSourceV1::new(&container, expectation.clone());
    let evidence = DirectLinkBundleEvidenceV1::bind(&bundle, &[&container], &[source]).unwrap();
    EvidenceFixture {
        container,
        bundle,
        expectation,
        evidence,
    }
}

fn package(seed: u8) -> PackageIdentityV1 {
    PackageIdentityV1::from_bytes([seed; 32])
}

fn target_architecture(architecture: &str) -> TargetIdentity {
    TargetIdentity::new(
        text("amdgcn-amd-amdhsa"),
        text(architecture),
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::AmdWave],
    )
    .unwrap()
}

fn publish_bridge(
    bridge: &DirectLinkPublicationBridgeV1,
) -> fe2o3_artifact_transaction::PublishedLinkArtifactV1 {
    let mut catalog = LinkPublicationCatalogV1::default();
    let mut record = catalog
        .begin(
            bridge.attempt(),
            bridge.trusted_scope(),
            bridge.request_identity(),
        )
        .unwrap();
    record
        .record_pinned_worker(
            &catalog,
            bridge.attempt(),
            bridge.request_identity(),
            bridge.worker_identity(),
        )
        .unwrap();
    record
        .record_validated_response(
            &catalog,
            bridge.attempt(),
            bridge.request_identity(),
            bridge.worker_identity(),
            bridge.response_identity(),
            bridge.linked_output_identity(),
        )
        .unwrap();
    record
        .record_finalization(
            &catalog,
            bridge.attempt(),
            bridge.response_identity(),
            bridge.linked_output_identity(),
            bridge.finalization_identity(),
            bridge.finalized_output_identity(),
        )
        .unwrap();
    assert_eq!(
        record.publish(
            &mut catalog,
            bridge.attempt(),
            bridge.finalization_identity(),
            bridge.finalized_output_identity(),
            bridge.publication_identity(),
        ),
        Ok(PublicationOutcomeV1::Published)
    );
    *catalog.published(&bridge.trusted_scope()).unwrap()
}

#[test]
fn typed_bridge_drives_and_validates_the_complete_g5_chain() {
    let fixture = evidence(0x18);
    let validated = fixture.validated();
    let bridge = DirectLinkPublicationBridgeV1::prepare_with_trusted_scope(
        attempt(9, 3),
        scope(4),
        &validated,
        0,
    )
    .unwrap();
    let published = publish_bridge(&bridge);

    assert_eq!(bridge.validate_published(published), Ok(()));
    assert_eq!(
        *bridge.publication_identity().as_bytes(),
        [
            0x0c, 0xa4, 0xe2, 0xf8, 0x72, 0xba, 0xb1, 0x73, 0x8a, 0x68, 0x86, 0x2d, 0x08, 0x17,
            0x65, 0x36, 0xf3, 0x8f, 0x23, 0x56, 0x2c, 0xcc, 0xa9, 0x7d, 0x65, 0x94, 0x79, 0x0b,
            0x71, 0x2f, 0xf6, 0x89,
        ]
    );
    assert!(!bridge.grants_load_authority());
    assert!(!bridge.grants_launch_authority());
}

#[test]
fn ffi_bundle_attempt_and_scope_are_committed_by_publication_identity() {
    let first_evidence = evidence(0x18);
    let changed_ffi_evidence = evidence(0x19);
    let first_validated = first_evidence.validated();
    let changed_ffi_validated = changed_ffi_evidence.validated();
    let first = DirectLinkPublicationBridgeV1::prepare_with_trusted_scope(
        attempt(10, 5),
        scope(6),
        &first_validated,
        0,
    )
    .unwrap();
    let changed_ffi = DirectLinkPublicationBridgeV1::prepare_with_trusted_scope(
        first.attempt(),
        first.trusted_scope(),
        &changed_ffi_validated,
        0,
    )
    .unwrap();
    let changed_attempt = DirectLinkPublicationBridgeV1::prepare_with_trusted_scope(
        attempt(11, 5),
        first.trusted_scope(),
        &first_validated,
        0,
    )
    .unwrap();
    let changed_scope = DirectLinkPublicationBridgeV1::prepare_with_trusted_scope(
        first.attempt(),
        scope(7),
        &first_validated,
        0,
    )
    .unwrap();

    assert_ne!(
        first.publication_identity(),
        changed_ffi.publication_identity()
    );
    assert_ne!(
        first.publication_identity(),
        changed_attempt.publication_identity()
    );
    assert_ne!(
        first.publication_identity(),
        changed_scope.publication_identity()
    );
    assert_eq!(
        changed_ffi.validate_published(publish_bridge(&first)),
        Err(DirectLinkBridgeError::IdentityMismatch {
            kind: DirectLinkBridgeIdentityKindV1::Publication,
        })
    );
}

#[test]
fn bridge_rejects_unvalidated_inputs_and_out_of_range_binding_selection() {
    let fixture = evidence(0x18);
    let validated = fixture.validated();
    assert_eq!(
        DirectLinkPublicationBridgeV1::prepare_with_trusted_scope(
            attempt(12, 8),
            scope(9),
            &validated,
            1,
        ),
        Err(DirectLinkBridgeError::BindingIndexOutOfRange {
            index: 1,
            binding_count: 1,
        })
    );

    let changed_expectation = expectation(
        fixture.expectation.finalized_payload_identity().digest(),
        0x19,
    );
    let changed_sources = [DirectLinkBindingSourceV1::new(
        &fixture.container,
        changed_expectation,
    )];
    assert_eq!(
        fixture
            .evidence
            .validate_against(&fixture.bundle, &[&fixture.container], &changed_sources),
        Err(fe2o3_artifacts::DirectLinkEvidenceError::ExpectationMismatch)
    );
}

#[test]
fn artifact_derived_scope_drives_the_complete_g5_chain() {
    let fixture = evidence(0x18);
    let validated = fixture.validated();
    let derived = ArtifactDerivedLinkPublicationScopeV1::derive(
        package(0x42),
        &validated,
        0,
        &fixture.container,
    )
    .unwrap();

    assert_eq!(derived.binding_index(), 0);
    assert_eq!(derived.scope().package(), package(0x42));
    assert_eq!(
        derived.container_identity(),
        validated.bindings()[0].container_identity()
    );
    assert_eq!(
        derived.finalized_payload_identity(),
        fixture.expectation.finalized_payload_identity()
    );
    assert!(!derived.grants_load_authority());
    assert!(!derived.grants_launch_authority());

    let bridge = DirectLinkPublicationBridgeV1::prepare_with_derived_scope(
        attempt(20, 0x43),
        derived,
        &validated,
        0,
    )
    .unwrap();
    assert_eq!(
        bridge.scope_provenance(),
        DirectLinkPublicationScopeProvenanceV1::ArtifactDerivedV1
    );
    assert_eq!(bridge.validate_published(publish_bridge(&bridge)), Ok(()));
    assert!(!bridge.grants_load_authority());
    assert!(!bridge.grants_launch_authority());
    let weaker_same_scope = DirectLinkPublicationBridgeV1::prepare_with_trusted_scope(
        bridge.attempt(),
        bridge.publication_scope(),
        &validated,
        0,
    )
    .unwrap();
    assert_ne!(
        bridge.publication_identity(),
        weaker_same_scope.publication_identity()
    );

    assert_eq!(
        *bridge.publication_scope().target().as_bytes(),
        [
            0x13, 0x12, 0x2c, 0x01, 0xe3, 0x28, 0x20, 0x06, 0x75, 0xdf, 0x7a, 0xfb, 0x80, 0x28,
            0x72, 0x66, 0x4a, 0x45, 0xf8, 0xbc, 0x69, 0xfa, 0x8d, 0x02, 0xfb, 0xf1, 0xf5, 0x13,
            0x09, 0xfc, 0x15, 0x13,
        ]
    );
    assert_eq!(
        *bridge.publication_scope().kernel_set().as_bytes(),
        [
            0x91, 0x5a, 0x54, 0x7b, 0xe1, 0xc8, 0xd5, 0xc8, 0x74, 0x9b, 0x21, 0xf9, 0x34, 0xef,
            0x02, 0x89, 0x3a, 0xf8, 0x7b, 0x19, 0x49, 0x9a, 0x20, 0x55, 0x64, 0x1a, 0x01, 0xe1,
            0x74, 0x54, 0x99, 0xa4,
        ]
    );
    assert_eq!(
        *bridge.publication_identity().as_bytes(),
        [
            0xed, 0xdd, 0xdc, 0xa9, 0x3c, 0xd4, 0x9d, 0x61, 0x92, 0x71, 0x74, 0x95, 0xf9, 0x78,
            0x1d, 0xa5, 0xab, 0x96, 0xb8, 0xe0, 0xe4, 0xd5, 0x8c, 0x6b, 0x9e, 0x07, 0xd8, 0x53,
            0x87, 0xcc, 0x98, 0x49,
        ]
    );
}

#[test]
fn trusted_scope_constructor_is_explicitly_the_weaker_compatibility_path() {
    let fixture = evidence(0x18);
    let validated = fixture.validated();
    let bridge = DirectLinkPublicationBridgeV1::prepare_with_trusted_scope(
        attempt(21, 0x44),
        scope(0x45),
        &validated,
        0,
    )
    .unwrap();

    assert_eq!(
        bridge.scope_provenance(),
        DirectLinkPublicationScopeProvenanceV1::TrustedExternalPolicy
    );
    assert_eq!(bridge.publication_scope(), bridge.trusted_scope());
}

#[test]
fn derived_scope_rejects_container_and_binding_substitution() {
    let original = evidence(0x18);
    let changed_container = evidence_variant(
        b"bridge native payload",
        "1.94.1",
        target(PointerWidth::Bits64, vec![]),
        &[(0x20, "bridge_kernel", "bridge_kernel.kd")],
        0x18,
    );
    let original_validated = original.validated();
    assert_eq!(
        ArtifactDerivedLinkPublicationScopeV1::derive(
            package(0x46),
            &original_validated,
            0,
            &changed_container.container,
        ),
        Err(DirectLinkBridgeError::DerivedScopeMismatch {
            field: DirectLinkDerivedScopeFieldV1::ContainerIdentity,
        })
    );

    let witness = ArtifactDerivedLinkPublicationScopeV1::derive(
        package(0x46),
        &original_validated,
        0,
        &original.container,
    )
    .unwrap();
    let changed_validated = changed_container.validated();
    assert_eq!(
        DirectLinkPublicationBridgeV1::prepare_with_derived_scope(
            attempt(22, 0x47),
            witness,
            &changed_validated,
            0,
        ),
        Err(DirectLinkBridgeError::DerivedScopeMismatch {
            field: DirectLinkDerivedScopeFieldV1::Binding,
        })
    );
}

#[test]
fn derived_scope_rejects_binding_index_substitution() {
    let first = evidence_variant(
        b"first native payload",
        "1.94.0",
        target(PointerWidth::Bits64, vec![]),
        &[(0x20, "first_kernel", "first_kernel.kd")],
        0x18,
    );
    let second = evidence_variant(
        b"second native payload",
        "1.94.0",
        target(PointerWidth::Bits64, vec![]),
        &[(0x21, "second_kernel", "second_kernel.kd")],
        0x19,
    );
    let first_expectation = first.expectation;
    let second_expectation = second.expectation;
    let containers = vec![first.container, second.container];
    let bundle = BundleIndexV1::from_containers(&containers).unwrap();
    let container_refs = [&containers[0], &containers[1]];
    let sources = [
        DirectLinkBindingSourceV1::new(&containers[0], first_expectation.clone()),
        DirectLinkBindingSourceV1::new(&containers[1], second_expectation.clone()),
    ];
    let evidence = DirectLinkBundleEvidenceV1::bind(&bundle, &container_refs, &sources).unwrap();
    let validated = evidence
        .validate_against(&bundle, &container_refs, &sources)
        .unwrap();
    let first_payload = first_expectation.finalized_payload_identity();
    let first_index = validated
        .bindings()
        .iter()
        .position(|binding| binding.expectation().finalized_payload_identity() == first_payload)
        .unwrap();
    let other_index = 1 - first_index;
    let witness = ArtifactDerivedLinkPublicationScopeV1::derive(
        package(0x48),
        &validated,
        first_index,
        &containers[0],
    )
    .unwrap();

    assert_eq!(
        DirectLinkPublicationBridgeV1::prepare_with_derived_scope(
            attempt(23, 0x49),
            witness,
            &validated,
            other_index,
        ),
        Err(DirectLinkBridgeError::DerivedScopeMismatch {
            field: DirectLinkDerivedScopeFieldV1::BindingIndex,
        })
    );
}

#[test]
fn derived_scope_rejects_a_different_bundle_around_the_same_binding() {
    let native = evidence(0x18);
    let native_validated = native.validated();
    let witness = ArtifactDerivedLinkPublicationScopeV1::derive(
        package(0x4c),
        &native_validated,
        0,
        &native.container,
    )
    .unwrap();
    let native_binding = native_validated.bindings()[0].clone();
    drop(native_validated);

    let relocatable_payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, b"relocatable payload".to_vec())
            .unwrap();
    let relocatable_digest = relocatable_payload.digest();
    let relocatable_manifest = fe2o3_artifacts::ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![]),
        vec![
            CodeObjectIdentity::new(
                relocatable_digest.bytes(),
                CodeObjectFormat::RelocatableObject,
                relocatable_payload.bytes().len() as u64,
            )
            .unwrap(),
        ],
        vec![kernel_with_object_digest(
            0x30,
            "relocatable_kernel",
            "relocatable_kernel.kd",
            relocatable_digest.bytes(),
            vec![],
        )],
    )
    .unwrap();
    let relocatable = ArtifactContainerV1::new(
        relocatable_manifest,
        DigestAlgorithm::Sha256,
        vec![relocatable_payload],
    )
    .unwrap();
    let native_expectation = native.expectation;
    let containers = vec![native.container, relocatable];
    let bundle = BundleIndexV1::from_containers(&containers).unwrap();
    let container_refs = [&containers[0], &containers[1]];
    let sources = [DirectLinkBindingSourceV1::new(
        &containers[0],
        native_expectation,
    )];
    let evidence = DirectLinkBundleEvidenceV1::bind(&bundle, &container_refs, &sources).unwrap();
    let validated = evidence
        .validate_against(&bundle, &container_refs, &sources)
        .unwrap();

    assert_eq!(validated.bindings()[0], native_binding);
    assert_eq!(
        DirectLinkPublicationBridgeV1::prepare_with_derived_scope(
            attempt(24, 0x4d),
            witness,
            &validated,
            0,
        ),
        Err(DirectLinkBridgeError::DerivedScopeMismatch {
            field: DirectLinkDerivedScopeFieldV1::BundleEvidence,
        })
    );
}

#[test]
fn target_payload_alias_kernel_and_container_facts_change_derived_scope() {
    let base = evidence_variant(
        b"scope payload A",
        "1.94.0",
        target_architecture("gfx942"),
        &[(0x20, "base", "base.kd")],
        0x18,
    );
    let changed_target = evidence_variant(
        b"scope payload A",
        "1.94.0",
        target_architecture("gfx950"),
        &[(0x20, "base", "base.kd")],
        0x18,
    );
    let changed_payload = evidence_variant(
        b"scope payload B",
        "1.94.0",
        target_architecture("gfx942"),
        &[(0x20, "base", "base.kd")],
        0x18,
    );
    let alias_kernel = evidence_variant(
        b"scope payload A",
        "1.94.0",
        target_architecture("gfx942"),
        &[(0x20, "base", "base.kd"), (0x21, "alias", "alias.kd")],
        0x18,
    );
    let changed_producer_fact = evidence_variant(
        b"scope payload A",
        "1.94.1",
        target_architecture("gfx942"),
        &[(0x20, "base", "base.kd")],
        0x18,
    );

    let derive = |fixture: &EvidenceFixture| {
        let validated = fixture.validated();
        ArtifactDerivedLinkPublicationScopeV1::derive(
            package(0x4a),
            &validated,
            0,
            &fixture.container,
        )
        .unwrap()
        .scope()
    };
    let base_scope = derive(&base);
    let target_scope = derive(&changed_target);
    let payload_scope = derive(&changed_payload);
    let alias_scope = derive(&alias_kernel);
    let producer_scope = derive(&changed_producer_fact);

    assert_ne!(base_scope.target(), target_scope.target());
    assert_ne!(base_scope.kernel_set(), target_scope.kernel_set());
    assert_eq!(base_scope.target(), payload_scope.target());
    assert_ne!(base_scope.kernel_set(), payload_scope.kernel_set());
    assert_eq!(base_scope.target(), alias_scope.target());
    assert_ne!(base_scope.kernel_set(), alias_scope.kernel_set());
    assert_eq!(base_scope.target(), producer_scope.target());
    assert_ne!(base_scope.kernel_set(), producer_scope.kernel_set());
}

#[test]
fn kernel_input_permutation_has_one_canonical_derived_scope() {
    let forward = evidence_variant(
        b"permutation payload",
        "1.94.0",
        target_architecture("gfx942"),
        &[(0x20, "alpha", "alpha.kd"), (0x21, "beta", "beta.kd")],
        0x18,
    );
    let reversed = evidence_variant(
        b"permutation payload",
        "1.94.0",
        target_architecture("gfx942"),
        &[(0x21, "beta", "beta.kd"), (0x20, "alpha", "alpha.kd")],
        0x18,
    );
    assert_eq!(forward.container.to_bytes(), reversed.container.to_bytes());

    let forward_validated = forward.validated();
    let reversed_validated = reversed.validated();
    let forward_scope = ArtifactDerivedLinkPublicationScopeV1::derive(
        package(0x4b),
        &forward_validated,
        0,
        &forward.container,
    )
    .unwrap();
    let reversed_scope = ArtifactDerivedLinkPublicationScopeV1::derive(
        package(0x4b),
        &reversed_validated,
        0,
        &reversed.container,
    )
    .unwrap();
    assert_eq!(forward_scope.scope(), reversed_scope.scope());
    assert_eq!(
        forward_scope.container_identity(),
        reversed_scope.container_identity()
    );
}
