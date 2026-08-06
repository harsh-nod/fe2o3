#[allow(dead_code)]
mod common;

use common::{digest, kernel_with_object_digest, object_identity, target, text};
use fe2o3_artifact_transaction::{
    BuildAttempt, KernelSetIdentityV1, LinkPublicationCatalogV1, LinkPublicationScopeV1,
    PackageIdentityV1, PublicationOutcomeV1, TargetIdentityV1,
};
use fe2o3_artifacts::{
    ArtifactContainerV1, BundleIndexV1, CodeObjectPayload, CompilerIdentity, DigestAlgorithm,
    DirectLinkBindingExpectationV1, DirectLinkBindingSourceV1, DirectLinkBridgeError,
    DirectLinkBridgeIdentityKindV1, DirectLinkBundleEvidenceV1, DirectLinkFfiClosureIdentityV1,
    DirectLinkFinalizationIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
    DirectLinkLinkedOutputIdentityV1, DirectLinkPublicationBridgeV1, DirectLinkRequestIdentityV1,
    DirectLinkResponseIdentityV1, DirectLinkToolchainConfigurationIdentityV1,
    DirectLinkToolchainExecutableIdentityV1, DirectLinkToolchainIdentityV1,
    DirectLinkTransformationIdentityV1, DirectLinkWorkerConfigurationIdentityV1,
    DirectLinkWorkerExecutableIdentityV1, DirectLinkWorkerIdentityV1, PayloadDigest, PointerWidth,
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
    let payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, b"bridge native payload".to_vec())
            .unwrap();
    let payload_identity = payload.digest();
    let manifest = fe2o3_artifacts::ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![]),
        vec![object_identity(
            payload_identity.bytes(),
            payload.bytes().len() as u64,
        )],
        vec![kernel_with_object_digest(
            0x20,
            "bridge_kernel",
            "bridge_kernel.kd",
            payload_identity.bytes(),
            vec![],
        )],
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
