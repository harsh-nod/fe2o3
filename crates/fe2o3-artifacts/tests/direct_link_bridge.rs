#[allow(dead_code)]
mod common;

use common::{digest, kernel_with_object_digest, object_identity, target, text};
use fe2o3_artifact_transaction::{
    BuildAttempt, DurableLinkPublicationOutcomeV1, KernelSetIdentityV1, LinkPublicationCatalogV1,
    LinkPublicationPhaseV1, LinkPublicationScopeV1, LinkPublicationStateV1, PackageIdentityV1,
    PublicationOutcomeV1, TargetIdentityV1,
};
use fe2o3_artifacts::{
    AbiLayout, ArtifactContainerV1, BundleIndexV1, CallerClaimedPackageIdentityV1, Capability,
    CodeObjectFormat, CodeObjectIdentity, CodeObjectPayload, CompilerIdentity, DigestAlgorithm,
    DirectLinkBindingExpectationV1, DirectLinkBindingSourceV1, DirectLinkBridgeError,
    DirectLinkBridgeIdentityKindV1, DirectLinkBundleEvidenceV1, DirectLinkFfiClosureIdentityV1,
    DirectLinkFinalizationIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
    DirectLinkLinkedOutputIdentityV1, DirectLinkManifestClaimScopeFieldV1,
    DirectLinkPublicationBridgeV1, DirectLinkPublicationScopeProvenanceV1,
    DirectLinkRequestIdentityV1, DirectLinkResponseIdentityV1,
    DirectLinkToolchainConfigurationIdentityV1, DirectLinkToolchainExecutableIdentityV1,
    DirectLinkToolchainIdentityV1, DirectLinkTransformationIdentityV1,
    DirectLinkWorkerConfigurationIdentityV1, DirectLinkWorkerExecutableIdentityV1,
    DirectLinkWorkerIdentityV1, Endianness, KernelEntry,
    ManifestClaimDerivedLinkPublicationScopeV1, ManifestClaimDirectLinkDurablePlanHandoffV1,
    ManifestClaimDirectLinkPublicationBridgeV1, PayloadDigest, PointerWidth, TargetIdentity,
    ToolIdentity, derive_manifest_claim_target_identity_v1,
    publish_manifest_claim_direct_link_durable_v1, recover_manifest_claim_direct_link_durable_v1,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-direct-link-bridge-{}-{}",
            std::process::id(),
            NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

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
    expectation_with_toolchain(payload, ffi_seed, 0x13, 0x14)
}

fn expectation_with_toolchain(
    payload: PayloadDigest,
    ffi_seed: u8,
    toolchain_executable_seed: u8,
    toolchain_configuration_seed: u8,
) -> DirectLinkBindingExpectationV1 {
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
            DirectLinkToolchainExecutableIdentityV1::new(tagged(toolchain_executable_seed)),
            DirectLinkToolchainConfigurationIdentityV1::new(tagged(toolchain_configuration_seed)),
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
    evidence_with_kernels(
        payload_bytes,
        compiler_version,
        target,
        |payload_identity| {
            kernels
                .iter()
                .map(|(id, name, symbol)| {
                    kernel_with_object_digest(*id, name, symbol, payload_identity, vec![])
                })
                .collect()
        },
        ffi_seed,
    )
}

fn evidence_with_kernels(
    payload_bytes: &[u8],
    compiler_version: &str,
    target: TargetIdentity,
    kernels: impl FnOnce(fe2o3_artifacts::DigestBytes) -> Vec<KernelEntry>,
    ffi_seed: u8,
) -> EvidenceFixture {
    evidence_with_kernels_and_producer(
        payload_bytes,
        compiler_version,
        "0.1.0",
        target,
        kernels,
        ffi_seed,
    )
}

fn evidence_with_kernels_and_producer(
    payload_bytes: &[u8],
    compiler_version: &str,
    producer_version: &str,
    target: TargetIdentity,
    kernels: impl FnOnce(fe2o3_artifacts::DigestBytes) -> Vec<KernelEntry>,
    ffi_seed: u8,
) -> EvidenceFixture {
    let payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, payload_bytes.to_vec()).unwrap();
    let payload_identity = payload.digest();
    let kernels = kernels(payload_identity.bytes());
    let manifest = fe2o3_artifacts::ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text(compiler_version)),
        ToolIdentity::new(text("fe2o3"), text(producer_version)),
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

fn replace_expectation(
    fixture: EvidenceFixture,
    expectation: DirectLinkBindingExpectationV1,
) -> EvidenceFixture {
    let EvidenceFixture {
        container, bundle, ..
    } = fixture;
    let source = DirectLinkBindingSourceV1::new(&container, expectation.clone());
    let evidence = DirectLinkBundleEvidenceV1::bind(&bundle, &[&container], &[source]).unwrap();
    EvidenceFixture {
        container,
        bundle,
        expectation,
        evidence,
    }
}

fn package_claim(seed: u8) -> CallerClaimedPackageIdentityV1 {
    CallerClaimedPackageIdentityV1::new(PackageIdentityV1::from_bytes([seed; 32]))
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

fn rebuilt_kernel_claim(
    payload: fe2o3_artifacts::DigestBytes,
    source_seed: u8,
    executable_seed: u8,
    empty_abi: bool,
) -> KernelEntry {
    let base = kernel_with_object_digest(0x20, "base", "base.kd", payload, vec![]);
    KernelEntry::new(
        base.kernel_id(),
        base.name().clone(),
        base.symbol().clone(),
        digest(source_seed),
        digest(executable_seed),
        payload,
        base.required_capabilities().to_vec(),
        base.launch().clone(),
        if empty_abi {
            AbiLayout::new(0, 1, PointerWidth::Bits64, vec![]).unwrap()
        } else {
            base.abi().clone()
        },
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

fn manifest_scope_claim(
    bridge: &ManifestClaimDirectLinkPublicationBridgeV1,
) -> LinkPublicationScopeV1 {
    bridge
        .non_authoritative_diagnostics()
        .descriptive_scope_claim()
}

fn publish_manifest_claim_bridge(
    bridge: &ManifestClaimDirectLinkPublicationBridgeV1,
) -> fe2o3_artifact_transaction::PublishedLinkArtifactV1 {
    let scope = manifest_scope_claim(bridge);
    let mut catalog = LinkPublicationCatalogV1::default();
    let mut record = catalog
        .begin(bridge.attempt(), scope, bridge.request_identity())
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
    *catalog.published(&scope).unwrap()
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
fn manifest_claim_handoff_publishes_and_recovers_inert_durable_bytes() {
    const PAYLOAD: &[u8] = b"bridge native payload";

    let fixture = evidence(0x18);
    let validated = fixture.validated();
    let derived = ManifestClaimDerivedLinkPublicationScopeV1::derive(
        CallerClaimedPackageIdentityV1::new(PackageIdentityV1::from_bytes([0x31; 32])),
        &validated,
        0,
        &fixture.container,
    )
    .unwrap();
    let bridge = ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
        attempt(13, 10),
        derived,
        &validated,
        0,
    )
    .unwrap();
    let expected_scope = manifest_scope_claim(&bridge);
    let handoff = bridge.durable_plan_handoff();
    let temp = TestDirectory::new();
    let output = temp.0.join("output");
    fs::create_dir(&output).unwrap();

    let result = publish_manifest_claim_direct_link_durable_v1(&output, &handoff, |transaction| {
        transaction.record_worker_pinned()?;
        transaction.record_response_validated()?;
        transaction.record_finalized(PAYLOAD)
    })
    .unwrap();
    assert_eq!(result.outcome(), DurableLinkPublicationOutcomeV1::Published);
    let snapshot = result.snapshot();
    assert_eq!(snapshot.artifact().bytes(), PAYLOAD);
    assert_eq!(snapshot.record().attempt(), bridge.attempt());
    assert_eq!(snapshot.record().scope(), expected_scope);
    assert_eq!(snapshot.record().request(), bridge.request_identity());
    assert_eq!(snapshot.record().worker(), Some(bridge.worker_identity()));
    assert_eq!(
        snapshot.record().response(),
        Some(bridge.response_identity())
    );
    assert_eq!(
        snapshot.record().linked_output(),
        Some(bridge.linked_output_identity())
    );
    assert_eq!(
        snapshot.record().finalization(),
        Some(bridge.finalization_identity())
    );
    assert_eq!(
        snapshot.record().finalized_output(),
        Some(bridge.finalized_output_identity())
    );
    assert_eq!(
        snapshot.record().publication(),
        Some(bridge.publication_identity())
    );
    assert_eq!(
        snapshot.record().state(),
        LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published)
    );
    assert!(!snapshot.grants_load_authority());
    assert!(!snapshot.grants_launch_authority());

    let recovered = recover_manifest_claim_direct_link_durable_v1(&output, &handoff)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.record(), snapshot.record());
    assert_eq!(recovered.artifact().bytes(), PAYLOAD);
    assert!(!recovered.grants_load_authority());
    assert!(!recovered.grants_launch_authority());

    let lease = result.into_current_lease();
    assert!(lease.is_bound_to_handoff(&handoff));
    assert_eq!(lease.exact_artifact_bytes(), PAYLOAD);
    assert!(!lease.grants_load_authority());
    assert!(!lease.grants_launch_authority());
    let token = lease.acquire_current_token().unwrap();
    assert_eq!(token.exact_artifact_bytes(), PAYLOAD);
    assert!(!token.grants_load_authority());
    assert!(!token.grants_launch_authority());
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
fn manifest_claim_scope_drives_the_complete_inert_g5_chain() {
    let fixture = evidence(0x18);
    let validated = fixture.validated();
    let derived = ManifestClaimDerivedLinkPublicationScopeV1::derive(
        package_claim(0x42),
        &validated,
        0,
        &fixture.container,
    )
    .unwrap();

    assert_eq!(derived.binding_index(), 0);
    assert_eq!(
        derived.descriptive_scope_claim().package(),
        package_claim(0x42).descriptive_claim()
    );
    assert!(
        !derived
            .caller_package_claim()
            .grants_package_ownership_authority()
    );
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

    let bridge = ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
        attempt(20, 0x43),
        derived,
        &validated,
        0,
    )
    .unwrap();
    let diagnostics = bridge.non_authoritative_diagnostics();
    assert_eq!(
        diagnostics.scope_provenance(),
        DirectLinkPublicationScopeProvenanceV1::ManifestClaimDerivedV1
    );
    assert!(!diagnostics.grants_publication_authority());
    assert!(!diagnostics.grants_load_authority());
    assert!(!diagnostics.grants_launch_authority());
    assert_eq!(
        bridge.validate_published(publish_manifest_claim_bridge(&bridge)),
        Ok(())
    );
    assert!(!bridge.grants_publication_authority());
    assert!(!bridge.grants_load_authority());
    assert!(!bridge.grants_launch_authority());
    let handoff: ManifestClaimDirectLinkDurablePlanHandoffV1 = bridge.durable_plan_handoff();
    assert_eq!(handoff.occurrence_identity(), bridge.occurrence_identity());
    assert_eq!(
        handoff.container_identity(),
        validated.bindings()[0].container_identity()
    );
    assert_eq!(
        handoff.finalized_payload_identity(),
        validated.bindings()[0]
            .expectation()
            .finalized_payload_identity()
    );
    assert!(!handoff.grants_publication_authority());
    assert!(!handoff.grants_load_authority());
    assert!(!handoff.grants_launch_authority());
    let descriptive_scope = diagnostics.descriptive_scope_claim();
    let weaker_same_scope = DirectLinkPublicationBridgeV1::prepare_with_trusted_scope(
        bridge.attempt(),
        descriptive_scope,
        &validated,
        0,
    )
    .unwrap();
    assert_ne!(
        bridge.publication_identity(),
        weaker_same_scope.publication_identity()
    );

    assert_eq!(
        *descriptive_scope.target().as_bytes(),
        [
            0xcc, 0x48, 0x6c, 0xff, 0xe7, 0x51, 0xc0, 0xe8, 0x39, 0x29, 0xff, 0x50, 0x65, 0x40,
            0x7b, 0xb4, 0x1c, 0x06, 0x87, 0xa8, 0x94, 0x58, 0x7a, 0xea, 0xbd, 0x6b, 0xd2, 0xa1,
            0xe3, 0xfa, 0xd3, 0x05,
        ]
    );
    assert_eq!(
        *descriptive_scope.kernel_set().as_bytes(),
        [
            0xda, 0x81, 0x04, 0xb9, 0xe2, 0x15, 0x2a, 0xbb, 0xf2, 0xb4, 0xda, 0x3b, 0x3e, 0xea,
            0x37, 0xdd, 0x12, 0xed, 0x19, 0x55, 0x72, 0x4a, 0x5e, 0x0c, 0x9a, 0x60, 0xe6, 0xb7,
            0xdd, 0x11, 0x78, 0x3e,
        ]
    );
    assert_eq!(
        *bridge.publication_identity().as_bytes(),
        [
            0x38, 0xb9, 0x1e, 0x53, 0x6e, 0xa7, 0xe5, 0xdf, 0x23, 0xbc, 0x92, 0x97, 0x66, 0xcf,
            0xe6, 0xbd, 0x52, 0x48, 0xd3, 0xdd, 0xd7, 0xd0, 0x78, 0x9a, 0x5b, 0x9e, 0xa2, 0x72,
            0xa6, 0xfc, 0x5c, 0x69,
        ]
    );
    assert_eq!(
        *bridge.occurrence_identity().as_bytes(),
        [
            0x65, 0x03, 0x8c, 0x77, 0xd7, 0x72, 0x6b, 0x3a, 0x9e, 0x6a, 0xe9, 0xe5, 0x5a, 0xe8,
            0xed, 0x24, 0x8e, 0xe1, 0x90, 0xab, 0xea, 0x01, 0xc4, 0x44, 0x88, 0xa0, 0xdf, 0x3c,
            0xab, 0x5a, 0x4e, 0x0b,
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
        DirectLinkPublicationScopeProvenanceV1::UnsafeLegacyExternalClaims
    );
    assert_eq!(bridge.publication_scope(), bridge.trusted_scope());
    assert!(!bridge.grants_publication_authority());
    assert!(!bridge.grants_load_authority());
    assert!(!bridge.grants_launch_authority());
}

#[test]
fn manifest_claim_scope_rejects_container_and_binding_substitution() {
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
        ManifestClaimDerivedLinkPublicationScopeV1::derive(
            package_claim(0x46),
            &original_validated,
            0,
            &changed_container.container,
        ),
        Err(DirectLinkBridgeError::ManifestClaimScopeMismatch {
            field: DirectLinkManifestClaimScopeFieldV1::ContainerIdentity,
        })
    );

    let witness = ManifestClaimDerivedLinkPublicationScopeV1::derive(
        package_claim(0x46),
        &original_validated,
        0,
        &original.container,
    )
    .unwrap();
    let changed_validated = changed_container.validated();
    assert_eq!(
        ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
            attempt(22, 0x47),
            witness,
            &changed_validated,
            0,
        ),
        Err(DirectLinkBridgeError::ManifestClaimScopeMismatch {
            field: DirectLinkManifestClaimScopeFieldV1::Binding,
        })
    );
}

#[test]
fn manifest_claim_scope_rejects_binding_index_substitution() {
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
    let witness = ManifestClaimDerivedLinkPublicationScopeV1::derive(
        package_claim(0x48),
        &validated,
        first_index,
        &containers[0],
    )
    .unwrap();

    assert_eq!(
        ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
            attempt(23, 0x49),
            witness,
            &validated,
            other_index,
        ),
        Err(DirectLinkBridgeError::ManifestClaimScopeMismatch {
            field: DirectLinkManifestClaimScopeFieldV1::BindingIndex,
        })
    );
}

#[test]
fn manifest_claim_scope_rejects_a_different_bundle_around_the_same_binding() {
    let native = evidence(0x18);
    let native_validated = native.validated();
    let witness = ManifestClaimDerivedLinkPublicationScopeV1::derive(
        package_claim(0x4c),
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
        ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
            attempt(24, 0x4d),
            witness,
            &validated,
            0,
        ),
        Err(DirectLinkBridgeError::ManifestClaimScopeMismatch {
            field: DirectLinkManifestClaimScopeFieldV1::BundleEvidence,
        })
    );
}

#[test]
fn target_and_logical_contracts_change_scope_but_rebuild_content_does_not() {
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
    let changed_compiler = evidence_variant(
        b"scope payload A",
        "1.94.1",
        target_architecture("gfx942"),
        &[(0x20, "base", "base.kd")],
        0x18,
    );
    let changed_producer = evidence_with_kernels_and_producer(
        b"scope payload A",
        "1.94.0",
        "0.2.0",
        target_architecture("gfx942"),
        |payload| vec![rebuilt_kernel_claim(payload, 0x22, 0x33, false)],
        0x18,
    );
    let toolchain_base = evidence_variant(
        b"scope payload A",
        "1.94.0",
        target_architecture("gfx942"),
        &[(0x20, "base", "base.kd")],
        0x18,
    );
    let toolchain_payload = toolchain_base
        .expectation
        .finalized_payload_identity()
        .digest();
    let changed_toolchain = replace_expectation(
        toolchain_base,
        expectation_with_toolchain(toolchain_payload, 0x18, 0x1a, 0x1b),
    );
    let changed_ffi_claim = evidence_variant(
        b"scope payload A",
        "1.94.0",
        target_architecture("gfx942"),
        &[(0x20, "base", "base.kd")],
        0x19,
    );
    let changed_executable_claim = evidence_with_kernels(
        b"scope payload A",
        "1.94.0",
        target_architecture("gfx942"),
        |payload| vec![rebuilt_kernel_claim(payload, 0x22, 0x34, false)],
        0x18,
    );
    let changed_source_contract = evidence_with_kernels(
        b"scope payload A",
        "1.94.0",
        target_architecture("gfx942"),
        |payload| vec![rebuilt_kernel_claim(payload, 0x23, 0x33, false)],
        0x18,
    );
    let changed_abi = evidence_with_kernels(
        b"scope payload A",
        "1.94.0",
        target_architecture("gfx942"),
        |payload| vec![rebuilt_kernel_claim(payload, 0x22, 0x33, true)],
        0x18,
    );

    let derive = |fixture: &EvidenceFixture| {
        let validated = fixture.validated();
        let witness = ManifestClaimDerivedLinkPublicationScopeV1::derive(
            package_claim(0x4a),
            &validated,
            0,
            &fixture.container,
        )
        .unwrap();
        ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
            attempt(25, 0x4e),
            witness,
            &validated,
            0,
        )
        .unwrap()
    };
    let base = derive(&base);
    let target = derive(&changed_target);
    let payload = derive(&changed_payload);
    let alias = derive(&alias_kernel);
    let compiler = derive(&changed_compiler);
    let producer = derive(&changed_producer);
    let toolchain = derive(&changed_toolchain);
    let ffi = derive(&changed_ffi_claim);
    let executable = derive(&changed_executable_claim);
    let source = derive(&changed_source_contract);
    let abi = derive(&changed_abi);

    assert_ne!(
        manifest_scope_claim(&base).target(),
        manifest_scope_claim(&target).target()
    );
    assert_eq!(
        manifest_scope_claim(&base).kernel_set(),
        manifest_scope_claim(&target).kernel_set()
    );
    assert_eq!(manifest_scope_claim(&base), manifest_scope_claim(&payload));
    assert_eq!(manifest_scope_claim(&base), manifest_scope_claim(&compiler));
    assert_eq!(manifest_scope_claim(&base), manifest_scope_claim(&producer));
    assert_eq!(
        manifest_scope_claim(&base),
        manifest_scope_claim(&toolchain)
    );
    assert_eq!(
        manifest_scope_claim(&base),
        manifest_scope_claim(&executable)
    );
    assert_eq!(
        manifest_scope_claim(&base).target(),
        manifest_scope_claim(&alias).target()
    );
    assert_ne!(
        manifest_scope_claim(&base).kernel_set(),
        manifest_scope_claim(&alias).kernel_set()
    );
    assert_ne!(
        manifest_scope_claim(&base).kernel_set(),
        manifest_scope_claim(&ffi).kernel_set()
    );
    assert_ne!(
        manifest_scope_claim(&base).kernel_set(),
        manifest_scope_claim(&source).kernel_set()
    );
    assert_ne!(
        manifest_scope_claim(&base).kernel_set(),
        manifest_scope_claim(&abi).kernel_set()
    );
    for changed in [
        &target,
        &payload,
        &alias,
        &compiler,
        &producer,
        &toolchain,
        &ffi,
        &executable,
        &source,
        &abi,
    ] {
        assert_ne!(base.publication_identity(), changed.publication_identity());
    }
}

#[test]
fn kernel_input_permutation_has_one_canonical_manifest_claim_scope() {
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
    let forward_scope = ManifestClaimDerivedLinkPublicationScopeV1::derive(
        package_claim(0x4b),
        &forward_validated,
        0,
        &forward.container,
    )
    .unwrap();
    let reversed_scope = ManifestClaimDerivedLinkPublicationScopeV1::derive(
        package_claim(0x4b),
        &reversed_validated,
        0,
        &reversed.container,
    )
    .unwrap();
    assert_eq!(
        forward_scope.descriptive_scope_claim(),
        reversed_scope.descriptive_scope_claim()
    );
    assert_eq!(
        forward_scope.container_identity(),
        reversed_scope.container_identity()
    );
}

#[test]
fn exported_manifest_target_derivation_is_the_publication_scope_identity() {
    let fixture = evidence(0x4c);
    let validated = fixture.validated();
    let scope = ManifestClaimDerivedLinkPublicationScopeV1::derive(
        package_claim(0x4d),
        &validated,
        0,
        &fixture.container,
    )
    .unwrap();
    let target = derive_manifest_claim_target_identity_v1(&fixture.container);

    assert_eq!(
        target.descriptive_identity(),
        scope.descriptive_scope_claim().target()
    );
    assert!(!target.grants_publication_authority());
    assert!(!target.grants_load_authority());
    assert!(!target.grants_launch_authority());
}

#[test]
fn identical_logical_claims_across_rebuild_containers_share_scope_not_occurrence() {
    let first = evidence_variant(
        b"identical rebuilt payload",
        "1.94.0",
        target_architecture("gfx942"),
        &[(0x20, "stable", "stable.kd")],
        0x18,
    );
    let second = evidence_variant(
        b"identical rebuilt payload",
        "1.95.0",
        target_architecture("gfx942"),
        &[(0x20, "stable", "stable.kd")],
        0x18,
    );
    let first_validated = first.validated();
    let second_validated = second.validated();
    let first_witness = ManifestClaimDerivedLinkPublicationScopeV1::derive(
        package_claim(0x50),
        &first_validated,
        0,
        &first.container,
    )
    .unwrap();
    let second_witness = ManifestClaimDerivedLinkPublicationScopeV1::derive(
        package_claim(0x50),
        &second_validated,
        0,
        &second.container,
    )
    .unwrap();
    let first_bridge =
        ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
            attempt(26, 0x51),
            first_witness,
            &first_validated,
            0,
        )
        .unwrap();
    let second_bridge =
        ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
            attempt(26, 0x51),
            second_witness,
            &second_validated,
            0,
        )
        .unwrap();

    assert_eq!(
        manifest_scope_claim(&first_bridge),
        manifest_scope_claim(&second_bridge)
    );
    assert_eq!(
        first_bridge.finalized_output_identity(),
        second_bridge.finalized_output_identity()
    );
    assert_ne!(
        first_bridge.occurrence_identity(),
        second_bridge.occurrence_identity()
    );
    assert_ne!(
        first_bridge.publication_identity(),
        second_bridge.publication_identity()
    );
    assert_eq!(
        first_bridge.validate_published(publish_manifest_claim_bridge(&second_bridge)),
        Err(DirectLinkBridgeError::IdentityMismatch {
            kind: DirectLinkBridgeIdentityKindV1::Publication,
        })
    );
}

#[test]
fn identical_payload_aliases_in_one_bundle_remain_occurrence_local() {
    let first = evidence_variant(
        b"shared alias payload",
        "1.94.0",
        target_architecture("gfx942"),
        &[(0x20, "first", "shared.kd")],
        0x18,
    );
    let alias = evidence_variant(
        b"shared alias payload",
        "1.95.0",
        target_architecture("gfx942"),
        &[(0x21, "alias", "alias.kd")],
        0x18,
    );
    let first_expectation = first.expectation;
    let alias_expectation = alias.expectation;
    let containers = vec![first.container, alias.container];
    let bundle = BundleIndexV1::from_containers(&containers).unwrap();
    let container_refs = [&containers[0], &containers[1]];
    let sources = [
        DirectLinkBindingSourceV1::new(&containers[0], first_expectation),
        DirectLinkBindingSourceV1::new(&containers[1], alias_expectation),
    ];
    let evidence = DirectLinkBundleEvidenceV1::bind(&bundle, &container_refs, &sources).unwrap();
    let validated = evidence
        .validate_against(&bundle, &container_refs, &sources)
        .unwrap();

    let locate = |container: &ArtifactContainerV1| {
        (0..validated.bindings().len())
            .find_map(|index| {
                ManifestClaimDerivedLinkPublicationScopeV1::derive(
                    package_claim(0x52),
                    &validated,
                    index,
                    container,
                )
                .ok()
                .map(|witness| (index, witness))
            })
            .unwrap()
    };
    let (first_index, first_witness) = locate(&containers[0]);
    let (alias_index, alias_witness) = locate(&containers[1]);
    assert_ne!(first_index, alias_index);
    assert_eq!(
        first_witness.finalized_payload_identity(),
        alias_witness.finalized_payload_identity()
    );
    assert_ne!(
        first_witness.occurrence_identity(),
        alias_witness.occurrence_identity()
    );
    assert_ne!(
        first_witness.descriptive_scope_claim().kernel_set(),
        alias_witness.descriptive_scope_claim().kernel_set()
    );

    let first_bridge =
        ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
            attempt(27, 0x53),
            first_witness,
            &validated,
            first_index,
        )
        .unwrap();
    let alias_bridge =
        ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
            attempt(27, 0x53),
            alias_witness,
            &validated,
            alias_index,
        )
        .unwrap();
    assert_ne!(
        first_bridge.occurrence_identity(),
        alias_bridge.occurrence_identity()
    );
    assert_ne!(
        first_bridge.publication_identity(),
        alias_bridge.publication_identity()
    );
    assert_eq!(
        first_bridge.validate_published(publish_manifest_claim_bridge(&alias_bridge)),
        Err(DirectLinkBridgeError::IdentityMismatch {
            kind: DirectLinkBridgeIdentityKindV1::Scope,
        })
    );
}

#[test]
fn streamed_container_identity_matches_canonical_multi_payload_occurrences() {
    let first_payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, b"first payload".to_vec()).unwrap();
    let second_payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, b"second payload".to_vec()).unwrap();
    let first_digest = first_payload.digest();
    let second_digest = second_payload.digest();
    let manifest = fe2o3_artifacts::ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target_architecture("gfx942"),
        vec![
            object_identity(first_digest.bytes(), first_payload.bytes().len() as u64),
            object_identity(second_digest.bytes(), second_payload.bytes().len() as u64),
        ],
        vec![
            kernel_with_object_digest(0x20, "first", "first.kd", first_digest.bytes(), vec![]),
            kernel_with_object_digest(0x21, "second", "second.kd", second_digest.bytes(), vec![]),
        ],
    )
    .unwrap();
    let container = ArtifactContainerV1::new(
        manifest,
        DigestAlgorithm::Sha256,
        vec![second_payload, first_payload],
    )
    .unwrap();
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
    let first_expectation = expectation(first_digest, 0x18);
    let second_expectation = expectation(second_digest, 0x19);
    let sources = [
        DirectLinkBindingSourceV1::new(&container, first_expectation),
        DirectLinkBindingSourceV1::new(&container, second_expectation),
    ];
    let evidence = DirectLinkBundleEvidenceV1::bind(&bundle, &[&container], &sources).unwrap();
    let validated = evidence
        .validate_against(&bundle, &[&container], &sources)
        .unwrap();

    let witnesses = (0..validated.bindings().len())
        .map(|index| {
            ManifestClaimDerivedLinkPublicationScopeV1::derive(
                package_claim(0x54),
                &validated,
                index,
                &container,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(witnesses.len(), 2);
    assert_eq!(
        witnesses[0].container_identity(),
        witnesses[1].container_identity()
    );
    assert_ne!(
        witnesses[0].occurrence_identity(),
        witnesses[1].occurrence_identity()
    );
    assert_ne!(
        witnesses[0].descriptive_scope_claim().kernel_set(),
        witnesses[1].descriptive_scope_claim().kernel_set()
    );
}
