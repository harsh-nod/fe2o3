use crate::{
    ArtifactBindingError, ArtifactRevalidationError, ObservedContext, ValidatedArtifactSelectionV1,
};
use fe2o3_artifact_transaction::PublishedLinkArtifactV1;
use fe2o3_artifacts::{
    ArtifactContainerV1, DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM, DirectLinkBridgeError,
    DirectLinkContainerIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
    DirectLinkPublicationBridgeV1, SelectedNativeKernel, ValidatedDirectLinkBundleEvidenceV1,
};
use std::fmt;

/// An opaque, inert host-side admission of one structurally validated G5/G6 selection.
///
/// Construction binds one validated direct-link evidence envelope and bridge to an exact G5
/// publication, canonical container identity, finalized payload occurrence, selected kernel, and
/// observed context. The token owns the existing structural [`ValidatedArtifactSelectionV1`]
/// result and can revalidate the complete input tuple against substitutions.
///
/// This value authenticates no filesystem object or compiler marker, does not establish that the
/// executable is safe, and grants no module-loading or kernel-launch authority.
pub struct ValidatedPublishedDirectLinkSelectionV1 {
    selection: ValidatedArtifactSelectionV1,
    bridge: DirectLinkPublicationBridgeV1,
    published: PublishedLinkArtifactV1,
    container_identity: DirectLinkContainerIdentityV1,
    finalized_payload_identity: DirectLinkFinalizedPayloadIdentityV1,
}

impl fmt::Debug for ValidatedPublishedDirectLinkSelectionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedPublishedDirectLinkSelectionV1")
            .field("selection", &self.selection)
            .field("published", &self.published)
            .field("container_identity", &self.container_identity)
            .field(
                "finalized_payload_identity",
                &self.finalized_payload_identity,
            )
            .finish_non_exhaustive()
    }
}

impl ValidatedPublishedDirectLinkSelectionV1 {
    /// Jointly validates one exact structural publication selection.
    pub fn validate(
        validated_bundle: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        bridge: &DirectLinkPublicationBridgeV1,
        published: PublishedLinkArtifactV1,
        container: &ArtifactContainerV1,
        selected: SelectedNativeKernel<'_>,
        observed: &ObservedContext,
    ) -> Result<Self, PublishedDirectLinkAdmissionError> {
        let (container_identity, finalized_payload_identity) =
            validate_direct_link_inputs(validated_bundle, bridge, published, container, selected)?;
        let selection = ValidatedArtifactSelectionV1::validate(selected, observed)
            .map_err(PublishedDirectLinkAdmissionError::ArtifactSelection)?;

        Ok(Self {
            selection,
            bridge: bridge.clone(),
            published,
            container_identity,
            finalized_payload_identity,
        })
    }

    /// Revalidates the complete tuple and rejects any publication, container, payload, kernel, or
    /// observed-context substitution relative to this token.
    pub fn revalidate(
        &self,
        validated_bundle: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        bridge: &DirectLinkPublicationBridgeV1,
        published: PublishedLinkArtifactV1,
        container: &ArtifactContainerV1,
        selected: SelectedNativeKernel<'_>,
        observed: &ObservedContext,
    ) -> Result<(), PublishedDirectLinkAdmissionError> {
        if bridge != &self.bridge {
            return Err(PublishedDirectLinkAdmissionError::BridgeSubstitution);
        }
        if published != self.published {
            return Err(PublishedDirectLinkAdmissionError::PublicationSubstitution);
        }

        let (container_identity, finalized_payload_identity) =
            validate_direct_link_inputs(validated_bundle, bridge, published, container, selected)?;
        if container_identity != self.container_identity {
            return Err(PublishedDirectLinkAdmissionError::ContainerIdentityMismatch);
        }
        if finalized_payload_identity != self.finalized_payload_identity {
            return Err(PublishedDirectLinkAdmissionError::FinalizedPayloadMismatch);
        }
        self.selection
            .revalidate(selected, observed)
            .map_err(PublishedDirectLinkAdmissionError::ArtifactRevalidation)
    }

    pub const fn published(&self) -> PublishedLinkArtifactV1 {
        self.published
    }

    pub const fn container_identity(&self) -> DirectLinkContainerIdentityV1 {
        self.container_identity
    }

    pub const fn finalized_payload_identity(&self) -> DirectLinkFinalizedPayloadIdentityV1 {
        self.finalized_payload_identity
    }

    /// Structural admission does not authenticate a filesystem object or pathname.
    pub const fn authenticates_filesystem_artifact(&self) -> bool {
        false
    }

    /// Structural admission does not prove an association with a compiler-generated marker.
    pub const fn proves_compiler_marker_binding(&self) -> bool {
        false
    }

    /// Structural admission does not establish executable memory safety or race freedom.
    pub const fn establishes_executable_safety(&self) -> bool {
        false
    }

    /// Structural admission never grants module-loading authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Structural admission never grants kernel-launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn validate_direct_link_inputs(
    validated_bundle: &ValidatedDirectLinkBundleEvidenceV1<'_>,
    bridge: &DirectLinkPublicationBridgeV1,
    published: PublishedLinkArtifactV1,
    container: &ArtifactContainerV1,
    selected: SelectedNativeKernel<'_>,
) -> Result<
    (
        DirectLinkContainerIdentityV1,
        DirectLinkFinalizedPayloadIdentityV1,
    ),
    PublishedDirectLinkAdmissionError,
> {
    if validated_bundle.evidence() != bridge.bundle()
        || !validated_bundle.bindings().contains(bridge.binding())
    {
        return Err(PublishedDirectLinkAdmissionError::EvidenceBridgeMismatch);
    }

    bridge
        .validate_published(published)
        .map_err(PublishedDirectLinkAdmissionError::PublicationBridge)?;

    let container_identity = DirectLinkContainerIdentityV1::new(
        DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM.calculate(&container.to_bytes()),
    );
    if bridge.binding().container_identity() != container_identity
        || !validated_bundle
            .container_identities()
            .contains(&container_identity)
    {
        return Err(PublishedDirectLinkAdmissionError::ContainerIdentityMismatch);
    }

    let concrete_selection = container
        .select_native_kernel(selected.kernel().kernel_id())
        .map_err(|_| PublishedDirectLinkAdmissionError::SelectedKernelContainerMismatch)?;
    if concrete_selection != selected {
        return Err(PublishedDirectLinkAdmissionError::SelectedKernelContainerMismatch);
    }

    let finalized_payload_identity = bridge.binding().expectation().finalized_payload_identity();
    let selected_payload_identity = fe2o3_artifacts::PayloadDigest::new(
        selected.digest_algorithm(),
        selected.code_object().digest(),
    );
    if finalized_payload_identity.digest() != selected_payload_identity
        || selected_payload_identity
            .verify(selected.payload())
            .is_err()
    {
        return Err(PublishedDirectLinkAdmissionError::FinalizedPayloadMismatch);
    }

    Ok((container_identity, finalized_payload_identity))
}

/// Failure to establish or revalidate inert published direct-link admission.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PublishedDirectLinkAdmissionError {
    EvidenceBridgeMismatch,
    PublicationBridge(DirectLinkBridgeError),
    ContainerIdentityMismatch,
    SelectedKernelContainerMismatch,
    FinalizedPayloadMismatch,
    ArtifactSelection(ArtifactBindingError),
    BridgeSubstitution,
    PublicationSubstitution,
    ArtifactRevalidation(ArtifactRevalidationError),
}

impl fmt::Display for PublishedDirectLinkAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EvidenceBridgeMismatch => {
                formatter.write_str("validated G6 evidence does not own the publication bridge")
            }
            Self::PublicationBridge(error) => error.fmt(formatter),
            Self::ContainerIdentityMismatch => formatter.write_str(
                "concrete container identity does not match the validated direct-link binding",
            ),
            Self::SelectedKernelContainerMismatch => {
                formatter.write_str("selected kernel does not belong to the concrete container")
            }
            Self::FinalizedPayloadMismatch => formatter.write_str(
                "selected kernel payload is not the binding's finalized payload occurrence",
            ),
            Self::ArtifactSelection(error) => error.fmt(formatter),
            Self::BridgeSubstitution => {
                formatter.write_str("publication bridge differs from the admitted bridge")
            }
            Self::PublicationSubstitution => {
                formatter.write_str("published artifact differs from the admitted publication")
            }
            Self::ArtifactRevalidation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PublishedDirectLinkAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PublicationBridge(error) => Some(error),
            Self::ArtifactSelection(error) => Some(error),
            Self::ArtifactRevalidation(error) => Some(error),
            Self::EvidenceBridgeMismatch
            | Self::ContainerIdentityMismatch
            | Self::SelectedKernelContainerMismatch
            | Self::FinalizedPayloadMismatch
            | Self::BridgeSubstitution
            | Self::PublicationSubstitution => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifact_transaction::{
        BuildAttempt, KernelSetIdentityV1, LinkPublicationCatalogV1, LinkPublicationScopeV1,
        PackageIdentityV1, PublicationOutcomeV1, TargetIdentityV1,
    };
    use fe2o3_artifacts::{
        AbiLayout, BlockSize, BundleIndexV1, CodeObjectFormat, CodeObjectIdentity,
        CodeObjectPayload, CompilerIdentity, DigestAlgorithm, DigestBytes, Dimensions,
        DirectLinkBindingExpectationV1, DirectLinkBindingSourceV1, DirectLinkBundleEvidenceV1,
        DirectLinkFfiClosureIdentityV1, DirectLinkFinalizationIdentityV1,
        DirectLinkFinalizedPayloadIdentityV1, DirectLinkLinkedOutputIdentityV1,
        DirectLinkRequestIdentityV1, DirectLinkResponseIdentityV1,
        DirectLinkToolchainConfigurationIdentityV1, DirectLinkToolchainExecutableIdentityV1,
        DirectLinkToolchainIdentityV1, DirectLinkTransformationIdentityV1,
        DirectLinkWorkerConfigurationIdentityV1, DirectLinkWorkerExecutableIdentityV1,
        DirectLinkWorkerIdentityV1, Endianness, IdentityText, KernelEntry, LaunchContract,
        ManifestV1, Name, PayloadDigest, PointerWidth, TargetIdentity, ToolIdentity,
    };

    struct Fixture {
        container: ArtifactContainerV1,
        bundle: BundleIndexV1,
        expectations: [DirectLinkBindingExpectationV1; 2],
        evidence: DirectLinkBundleEvidenceV1,
        primary_kernel: DigestBytes,
        alias_kernel: DigestBytes,
        other_payload_kernel: DigestBytes,
    }

    impl Fixture {
        fn validated(&self) -> ValidatedDirectLinkBundleEvidenceV1<'_> {
            let sources = self
                .expectations
                .iter()
                .cloned()
                .map(|expectation| DirectLinkBindingSourceV1::new(&self.container, expectation))
                .collect::<Vec<_>>();
            self.evidence
                .validate_against(&self.bundle, &[&self.container], &sources)
                .unwrap()
        }

        fn binding_index(
            &self,
            validated: &ValidatedDirectLinkBundleEvidenceV1<'_>,
            expectation_index: usize,
        ) -> usize {
            let finalized = self.expectations[expectation_index].finalized_payload_identity();
            validated
                .bindings()
                .iter()
                .position(|binding| binding.expectation().finalized_payload_identity() == finalized)
                .unwrap()
        }
    }

    fn text(value: &str) -> IdentityText {
        IdentityText::new(value).unwrap()
    }

    fn name(value: &str) -> Name {
        Name::new(value).unwrap()
    }

    fn repeated_digest(seed: u8) -> DigestBytes {
        DigestBytes::from_bytes([seed; 32])
    }

    fn tagged(seed: u8) -> PayloadDigest {
        PayloadDigest::new(DigestAlgorithm::Sha256, repeated_digest(seed))
    }

    fn expectation(seed: u8, payload: PayloadDigest) -> DirectLinkBindingExpectationV1 {
        DirectLinkBindingExpectationV1::new(
            DirectLinkRequestIdentityV1::new(tagged(seed)),
            DirectLinkWorkerIdentityV1::new(
                text("fe2o3-llvm-link-worker"),
                text("1.0.0"),
                DirectLinkWorkerExecutableIdentityV1::new(tagged(seed.wrapping_add(1))),
                DirectLinkWorkerConfigurationIdentityV1::new(tagged(seed.wrapping_add(2))),
            ),
            DirectLinkToolchainIdentityV1::new(
                text("rocm-llvm-lld"),
                text("22.0.0-build.17"),
                DirectLinkToolchainExecutableIdentityV1::new(tagged(seed.wrapping_add(3))),
                DirectLinkToolchainConfigurationIdentityV1::new(tagged(seed.wrapping_add(4))),
            ),
            DirectLinkResponseIdentityV1::new(tagged(seed.wrapping_add(5))),
            DirectLinkTransformationIdentityV1::new(
                DirectLinkLinkedOutputIdentityV1::new(tagged(seed.wrapping_add(6))),
                DirectLinkFinalizationIdentityV1::new(tagged(seed.wrapping_add(7))),
                DirectLinkFinalizedPayloadIdentityV1::new(payload),
            ),
            DirectLinkFfiClosureIdentityV1::new(tagged(seed.wrapping_add(8))),
        )
    }

    fn kernel(id: u8, symbol: &str, payload: PayloadDigest) -> KernelEntry {
        KernelEntry::new(
            repeated_digest(id),
            name(symbol),
            name(&format!("{symbol}.kd")),
            repeated_digest(id.wrapping_add(0x40)),
            repeated_digest(id.wrapping_add(0x50)),
            payload.bytes(),
            vec![],
            LaunchContract::new(
                1,
                BlockSize::Exact(Dimensions::new(64, 1, 1).unwrap()),
                Dimensions::new(65_535, 1, 1).unwrap(),
                0,
                0,
            )
            .unwrap(),
            AbiLayout::new(0, 1, PointerWidth::Bits64, vec![]).unwrap(),
        )
        .unwrap()
    }

    fn make_fixture(seed: u8) -> Fixture {
        let first_payload =
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, vec![seed; 48]).unwrap();
        let second_payload =
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, vec![seed.wrapping_add(1); 56])
                .unwrap();
        let first_identity = first_payload.digest();
        let second_identity = second_payload.digest();
        let primary_kernel = repeated_digest(seed.wrapping_add(0x10));
        let alias_kernel = repeated_digest(seed.wrapping_add(0x11));
        let other_payload_kernel = repeated_digest(seed.wrapping_add(0x12));
        let manifest = ManifestV1::new(
            CompilerIdentity::new(text("rustc"), text("1.94.0")),
            ToolIdentity::new(text("fe2o3"), text("0.1.0")),
            TargetIdentity::new(
                text("amdgcn-amd-amdhsa"),
                text("gfx1100"),
                PointerWidth::Bits64,
                Endianness::Little,
                vec![],
            )
            .unwrap(),
            vec![
                CodeObjectIdentity::new(
                    first_identity.bytes(),
                    CodeObjectFormat::NativeExecutable,
                    first_payload.bytes().len() as u64,
                )
                .unwrap(),
                CodeObjectIdentity::new(
                    second_identity.bytes(),
                    CodeObjectFormat::NativeExecutable,
                    second_payload.bytes().len() as u64,
                )
                .unwrap(),
            ],
            vec![
                kernel(seed.wrapping_add(0x10), "primary_kernel", first_identity),
                kernel(seed.wrapping_add(0x11), "alias_kernel", first_identity),
                kernel(
                    seed.wrapping_add(0x12),
                    "other_payload_kernel",
                    second_identity,
                ),
            ],
        )
        .unwrap();
        let container = ArtifactContainerV1::new(
            manifest,
            DigestAlgorithm::Sha256,
            vec![first_payload, second_payload],
        )
        .unwrap();
        let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
        let expectations = [
            expectation(seed.wrapping_add(0x20), first_identity),
            expectation(seed.wrapping_add(0x30), second_identity),
        ];
        let sources = expectations
            .iter()
            .cloned()
            .map(|expectation| DirectLinkBindingSourceV1::new(&container, expectation))
            .collect::<Vec<_>>();
        let evidence = DirectLinkBundleEvidenceV1::bind(&bundle, &[&container], &sources).unwrap();
        Fixture {
            container,
            bundle,
            expectations,
            evidence,
            primary_kernel,
            alias_kernel,
            other_payload_kernel,
        }
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

    fn make_bridge(
        fixture: &Fixture,
        validated: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        expectation_index: usize,
        generation: u64,
        seed: u8,
    ) -> DirectLinkPublicationBridgeV1 {
        DirectLinkPublicationBridgeV1::prepare_with_trusted_scope(
            attempt(generation, seed),
            scope(seed),
            validated,
            fixture.binding_index(validated, expectation_index),
        )
        .unwrap()
    }

    fn publish(bridge: &DirectLinkPublicationBridgeV1) -> PublishedLinkArtifactV1 {
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

    fn make_observed(identity: usize) -> ObservedContext {
        ObservedContext::for_test(identity, 0, "gfx1100", 1024, 65_536)
    }

    #[test]
    fn exact_published_selection_is_admitted_without_authority() {
        let fixture = make_fixture(1);
        let validated = fixture.validated();
        let bridge = make_bridge(&fixture, &validated, 0, 1, 1);
        let published = publish(&bridge);
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let observed = make_observed(1);

        let admitted = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &bridge,
            published,
            &fixture.container,
            selected,
            &observed,
        )
        .unwrap();

        assert_eq!(admitted.published(), published);
        assert_eq!(
            admitted.finalized_payload_identity(),
            fixture.expectations[0].finalized_payload_identity()
        );
        assert!(!admitted.authenticates_filesystem_artifact());
        assert!(!admitted.proves_compiler_marker_binding());
        assert!(!admitted.establishes_executable_safety());
        assert!(!admitted.grants_load_authority());
        assert!(!admitted.grants_launch_authority());
        assert_eq!(
            admitted.revalidate(
                &validated,
                &bridge,
                published,
                &fixture.container,
                selected,
                &observed,
            ),
            Ok(())
        );
    }

    #[test]
    fn evidence_and_publication_substitutions_are_rejected() {
        let fixture = make_fixture(2);
        let validated = fixture.validated();
        let bridge = make_bridge(&fixture, &validated, 0, 2, 2);
        let published = publish(&bridge);
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let observed = make_observed(2);
        let admitted = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &bridge,
            published,
            &fixture.container,
            selected,
            &observed,
        )
        .unwrap();

        let other_fixture = make_fixture(3);
        let other_validated = other_fixture.validated();
        assert_eq!(
            admitted.revalidate(
                &other_validated,
                &bridge,
                published,
                &fixture.container,
                selected,
                &observed,
            ),
            Err(PublishedDirectLinkAdmissionError::EvidenceBridgeMismatch)
        );

        let other_bridge = make_bridge(&fixture, &validated, 0, 3, 4);
        let other_publication = publish(&other_bridge);
        assert_eq!(
            admitted.revalidate(
                &validated,
                &bridge,
                other_publication,
                &fixture.container,
                selected,
                &observed,
            ),
            Err(PublishedDirectLinkAdmissionError::PublicationSubstitution)
        );
        assert_eq!(
            admitted.revalidate(
                &validated,
                &other_bridge,
                other_publication,
                &fixture.container,
                selected,
                &observed,
            ),
            Err(PublishedDirectLinkAdmissionError::BridgeSubstitution)
        );
    }

    #[test]
    fn concrete_container_substitution_is_rejected() {
        let fixture = make_fixture(4);
        let validated = fixture.validated();
        let bridge = make_bridge(&fixture, &validated, 0, 4, 4);
        let published = publish(&bridge);
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let observed = make_observed(4);
        let admitted = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &bridge,
            published,
            &fixture.container,
            selected,
            &observed,
        )
        .unwrap();

        let substitute = make_fixture(5);
        let substitute_selected = substitute
            .container
            .select_native_kernel(substitute.primary_kernel)
            .unwrap();
        assert_eq!(
            admitted.revalidate(
                &validated,
                &bridge,
                published,
                &substitute.container,
                substitute_selected,
                &observed,
            ),
            Err(PublishedDirectLinkAdmissionError::ContainerIdentityMismatch)
        );
    }

    #[test]
    fn finalized_payload_substitution_is_rejected() {
        let fixture = make_fixture(6);
        let validated = fixture.validated();
        let bridge = make_bridge(&fixture, &validated, 0, 6, 6);
        let published = publish(&bridge);
        let substituted = fixture
            .container
            .select_native_kernel(fixture.other_payload_kernel)
            .unwrap();

        assert_eq!(
            ValidatedPublishedDirectLinkSelectionV1::validate(
                &validated,
                &bridge,
                published,
                &fixture.container,
                substituted,
                &make_observed(6),
            )
            .unwrap_err(),
            PublishedDirectLinkAdmissionError::FinalizedPayloadMismatch
        );
    }

    #[test]
    fn selected_kernel_substitution_is_rejected_even_for_the_same_payload() {
        let fixture = make_fixture(7);
        let validated = fixture.validated();
        let bridge = make_bridge(&fixture, &validated, 0, 7, 7);
        let published = publish(&bridge);
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let substituted = fixture
            .container
            .select_native_kernel(fixture.alias_kernel)
            .unwrap();
        let observed = make_observed(7);
        let admitted = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &bridge,
            published,
            &fixture.container,
            selected,
            &observed,
        )
        .unwrap();

        assert_eq!(
            admitted.revalidate(
                &validated,
                &bridge,
                published,
                &fixture.container,
                substituted,
                &observed,
            ),
            Err(PublishedDirectLinkAdmissionError::ArtifactRevalidation(
                ArtifactRevalidationError::WrongArtifactIdentity,
            ))
        );
    }

    #[test]
    fn observed_context_substitution_is_rejected() {
        let fixture = make_fixture(8);
        let validated = fixture.validated();
        let bridge = make_bridge(&fixture, &validated, 0, 8, 8);
        let published = publish(&bridge);
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let observed = make_observed(8);
        let admitted = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &bridge,
            published,
            &fixture.container,
            selected,
            &observed,
        )
        .unwrap();

        assert_eq!(
            admitted.revalidate(
                &validated,
                &bridge,
                published,
                &fixture.container,
                selected,
                &make_observed(9),
            ),
            Err(PublishedDirectLinkAdmissionError::ArtifactRevalidation(
                ArtifactRevalidationError::WrongContext,
            ))
        );
    }
}
