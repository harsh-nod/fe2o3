use crate::{
    ArtifactBindingError, ArtifactRevalidationError, ObservedContext, ValidatedArtifactSelectionV1,
};
use fe2o3_artifact_transaction::PublishedLinkArtifactV1;
use fe2o3_artifacts::{
    AbiLayout, ArtifactContainerV1, DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM, DigestBytes,
    DirectLinkBridgeError, DirectLinkContainerIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
    DirectLinkPublicationBridgeV1, LaunchContract, Name, SelectedNativeKernel,
    ValidatedDirectLinkBundleEvidenceV1,
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
    binding_index: usize,
    container_identity: DirectLinkContainerIdentityV1,
    finalized_payload_identity: DirectLinkFinalizedPayloadIdentityV1,
    payload_kernel_set: Box<[PublishedPayloadKernelV1]>,
}

/// Exact manifest entries that reference the admitted payload, in canonical kernel-ID order.
///
/// This is crate-private structural data, not authenticated compiler provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PublishedPayloadKernelV1 {
    kernel_id: DigestBytes,
    name: Name,
    symbol: Name,
    abi: AbiLayout,
    launch: LaunchContract,
}

impl PublishedPayloadKernelV1 {
    pub(crate) const fn symbol(&self) -> &Name {
        &self.symbol
    }

    pub(crate) const fn abi(&self) -> &AbiLayout {
        &self.abi
    }

    pub(crate) const fn launch(&self) -> &LaunchContract {
        &self.launch
    }
}

impl fmt::Debug for ValidatedPublishedDirectLinkSelectionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedPublishedDirectLinkSelectionV1")
            .field("selection", &self.selection)
            .field("published", &self.published)
            .field("binding_index", &self.binding_index)
            .field("container_identity", &self.container_identity)
            .field(
                "finalized_payload_identity",
                &self.finalized_payload_identity,
            )
            .field("payload_kernel_count", &self.payload_kernel_set.len())
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
        let (binding_index, container_identity, finalized_payload_identity) =
            validate_direct_link_inputs(validated_bundle, bridge, published, container, selected)?;
        let selection = ValidatedArtifactSelectionV1::validate(selected, observed)
            .map_err(PublishedDirectLinkAdmissionError::ArtifactSelection)?;
        let payload_kernel_set = payload_kernel_set(selected);

        Ok(Self {
            selection,
            bridge: bridge.clone(),
            published,
            binding_index,
            container_identity,
            finalized_payload_identity,
            payload_kernel_set,
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

        let (binding_index, container_identity, finalized_payload_identity) =
            validate_direct_link_inputs(validated_bundle, bridge, published, container, selected)?;
        if binding_index != self.binding_index {
            return Err(PublishedDirectLinkAdmissionError::BindingIndexSubstitution);
        }
        if container_identity != self.container_identity {
            return Err(PublishedDirectLinkAdmissionError::ContainerIdentityMismatch);
        }
        if finalized_payload_identity != self.finalized_payload_identity {
            return Err(PublishedDirectLinkAdmissionError::FinalizedPayloadMismatch);
        }
        if payload_kernel_set(selected) != self.payload_kernel_set {
            return Err(PublishedDirectLinkAdmissionError::PayloadKernelSetSubstitution);
        }
        self.selection
            .revalidate(selected, observed)
            .map_err(PublishedDirectLinkAdmissionError::ArtifactRevalidation)
    }

    pub const fn published(&self) -> PublishedLinkArtifactV1 {
        self.published
    }

    /// Returns the unique canonical G6 binding index associated with this admission.
    pub const fn binding_index(&self) -> usize {
        self.binding_index
    }

    pub const fn container_identity(&self) -> DirectLinkContainerIdentityV1 {
        self.container_identity
    }

    pub const fn finalized_payload_identity(&self) -> DirectLinkFinalizedPayloadIdentityV1 {
        self.finalized_payload_identity
    }

    pub(crate) const fn artifact_selection(&self) -> &ValidatedArtifactSelectionV1 {
        &self.selection
    }

    pub(crate) fn payload_kernel_set(&self) -> &[PublishedPayloadKernelV1] {
        &self.payload_kernel_set
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

fn payload_kernel_set(selected: SelectedNativeKernel<'_>) -> Box<[PublishedPayloadKernelV1]> {
    selected
        .manifest()
        .kernels()
        .iter()
        .filter(|kernel| kernel.code_object_digest() == selected.code_object().digest())
        .map(|kernel| PublishedPayloadKernelV1 {
            kernel_id: kernel.kernel_id(),
            name: kernel.name().clone(),
            symbol: kernel.symbol().clone(),
            abi: kernel.abi().clone(),
            launch: kernel.launch().clone(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn validate_direct_link_inputs(
    validated_bundle: &ValidatedDirectLinkBundleEvidenceV1<'_>,
    bridge: &DirectLinkPublicationBridgeV1,
    published: PublishedLinkArtifactV1,
    container: &ArtifactContainerV1,
    selected: SelectedNativeKernel<'_>,
) -> Result<
    (
        usize,
        DirectLinkContainerIdentityV1,
        DirectLinkFinalizedPayloadIdentityV1,
    ),
    PublishedDirectLinkAdmissionError,
> {
    if validated_bundle.evidence() != bridge.bundle() {
        return Err(PublishedDirectLinkAdmissionError::EvidenceBridgeMismatch);
    }
    let binding_index = unique_binding_index(validated_bundle, bridge)
        .ok_or(PublishedDirectLinkAdmissionError::EvidenceBridgeMismatch)?;

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

    Ok((
        binding_index,
        container_identity,
        finalized_payload_identity,
    ))
}

fn unique_binding_index(
    validated_bundle: &ValidatedDirectLinkBundleEvidenceV1<'_>,
    bridge: &DirectLinkPublicationBridgeV1,
) -> Option<usize> {
    let mut matches = validated_bundle
        .bindings()
        .iter()
        .enumerate()
        .filter(|(_, binding)| *binding == bridge.binding());
    let (index, _) = matches.next()?;

    // G6 canonical validation rejects duplicate (container identity, finalized payload identity)
    // occurrences. Equal bindings necessarily have the same occurrence key, so one full-equality
    // match identifies exactly one canonical index. Keep the cardinality check fail-closed here in
    // case that upstream invariant ever changes.
    if matches.next().is_some() {
        None
    } else {
        Some(index)
    }
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
    PayloadKernelSetSubstitution,
    ArtifactSelection(ArtifactBindingError),
    BridgeSubstitution,
    PublicationSubstitution,
    BindingIndexSubstitution,
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
            Self::PayloadKernelSetSubstitution => formatter.write_str(
                "manifest kernel set for the finalized payload differs from the admitted set",
            ),
            Self::ArtifactSelection(error) => error.fmt(formatter),
            Self::BridgeSubstitution => {
                formatter.write_str("publication bridge differs from the admitted bridge")
            }
            Self::PublicationSubstitution => {
                formatter.write_str("published artifact differs from the admitted publication")
            }
            Self::BindingIndexSubstitution => formatter
                .write_str("canonical G6 binding index differs from the admitted occurrence"),
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
            | Self::PayloadKernelSetSubstitution
            | Self::BridgeSubstitution
            | Self::PublicationSubstitution
            | Self::BindingIndexSubstitution => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        InspectedPublishedDirectLinkPhysicalLayoutV1, PhysicalMetadataValueV1,
        PublishedPhysicalLayoutInspectionError,
    };
    use fe2o3_artifact_transaction::{
        BuildAttempt, KernelSetIdentityV1, LinkPublicationCatalogV1, LinkPublicationScopeV1,
        PackageIdentityV1, PublicationOutcomeV1, TargetIdentityV1,
    };
    use fe2o3_artifacts::{
        AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
        BlockSize, BundleIndexV1, CodeObjectFormat, CodeObjectIdentity, CodeObjectPayload,
        CompilerIdentity, DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestAlgorithm,
        DigestBytes, Dimensions, DirectLinkBindingExpectationV1, DirectLinkBindingSourceV1,
        DirectLinkBundleEvidenceV1, DirectLinkFfiClosureIdentityV1,
        DirectLinkFinalizationIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
        DirectLinkLinkedOutputIdentityV1, DirectLinkRequestIdentityV1,
        DirectLinkResponseIdentityV1, DirectLinkToolchainConfigurationIdentityV1,
        DirectLinkToolchainExecutableIdentityV1, DirectLinkToolchainIdentityV1,
        DirectLinkTransformationIdentityV1, DirectLinkWorkerConfigurationIdentityV1,
        DirectLinkWorkerExecutableIdentityV1, DirectLinkWorkerIdentityV1, Endianness, IdentityText,
        KernelEntry, LaunchContract, ManifestV1, Mutability, Name, PayloadDigest, PointerWidth,
        ScalarType, TargetIdentity, ToolIdentity, TypeIdentity,
    };
    use fe2o3_hsaco::{
        ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion, ExplicitValueKind,
        InspectionError, KernelBindingError,
    };
    use rmpv::{Value, encode::write_value};

    const ELF_HEADER_BYTES: usize = 64;
    const SECTION_HEADER_BYTES: usize = 64;

    struct Fixture {
        container: ArtifactContainerV1,
        bundle: BundleIndexV1,
        expectations: Vec<DirectLinkBindingExpectationV1>,
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

    fn type_identity(seed: u8) -> TypeIdentity {
        TypeIdentity::new(
            DeclaredRustTypeIdentity::from_untrusted_bytes(repeated_digest(seed)),
            DeclaredRustLayoutIdentity::from_untrusted_bytes(repeated_digest(seed.wrapping_add(1))),
        )
    }

    fn physical_test_abi(alternate_semantics: bool) -> AbiLayout {
        let scalar = AbiField::new(
            name("scalar"),
            0,
            4,
            4,
            AbiKind::Scalar(if alternate_semantics {
                ScalarType::F32
            } else {
                ScalarType::U32
            }),
            Mutability::Immutable,
            Access::ByValue,
            AddressSpace::Value,
            type_identity(if alternate_semantics { 0xa1 } else { 0x91 }),
            ArgumentOwnership::ByValue,
            AliasClass::Value,
        )
        .unwrap();
        let pointer = AbiField::new(
            name("pointer"),
            8,
            8,
            8,
            AbiKind::Pointer {
                pointee_size: if alternate_semantics { 8 } else { 4 },
                pointee_alignment: if alternate_semantics { 8 } else { 4 },
            },
            if alternate_semantics {
                Mutability::Mutable
            } else {
                Mutability::Immutable
            },
            if alternate_semantics {
                Access::ReadWrite
            } else {
                Access::ReadOnly
            },
            AddressSpace::Global,
            type_identity(if alternate_semantics { 0xa2 } else { 0x92 }),
            if alternate_semantics {
                ArgumentOwnership::UniqueBorrow
            } else {
                ArgumentOwnership::SharedBorrow
            },
            if alternate_semantics {
                AliasClass::Exclusive
            } else {
                AliasClass::SharedReadOnly
            },
        )
        .unwrap();
        let slice = AbiField::new(
            name("slice"),
            16,
            16,
            8,
            AbiKind::Slice {
                element_size: if alternate_semantics { 8 } else { 4 },
                element_alignment: if alternate_semantics { 8 } else { 4 },
            },
            if alternate_semantics {
                Mutability::Immutable
            } else {
                Mutability::Mutable
            },
            if alternate_semantics {
                Access::ReadOnly
            } else {
                Access::ReadWrite
            },
            AddressSpace::Global,
            type_identity(if alternate_semantics { 0xa3 } else { 0x93 }),
            if alternate_semantics {
                ArgumentOwnership::SharedBorrow
            } else {
                ArgumentOwnership::UniqueBorrow
            },
            if alternate_semantics {
                AliasClass::SharedReadOnly
            } else {
                AliasClass::Exclusive
            },
        )
        .unwrap();
        AbiLayout::new(32, 8, PointerWidth::Bits64, vec![scalar, pointer, slice]).unwrap()
    }

    fn physical_test_launch(max_dynamic_shared_memory_bytes: u32) -> LaunchContract {
        LaunchContract::new(
            1,
            BlockSize::Exact(Dimensions::new(64, 1, 1).unwrap()),
            Dimensions::new(65_535, 1, 1).unwrap(),
            0,
            max_dynamic_shared_memory_bytes,
        )
        .unwrap()
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
        let expectations = vec![
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

    fn hsaco_kernel(
        id: u8,
        logical_name: &str,
        symbol: &str,
        payload: PayloadDigest,
        static_shared_memory_bytes: u32,
    ) -> KernelEntry {
        KernelEntry::new(
            repeated_digest(id),
            name(logical_name),
            name(symbol),
            repeated_digest(id.wrapping_add(0x40)),
            repeated_digest(id.wrapping_add(0x50)),
            payload.bytes(),
            vec![],
            LaunchContract::new(
                1,
                BlockSize::Exact(Dimensions::new(64, 1, 1).unwrap()),
                Dimensions::new(65_535, 1, 1).unwrap(),
                static_shared_memory_bytes,
                0,
            )
            .unwrap(),
            AbiLayout::new(0, 1, PointerWidth::Bits64, vec![]).unwrap(),
        )
        .unwrap()
    }

    fn make_hsaco_fixture(
        seed: u8,
        payload_bytes: Vec<u8>,
        manifest_architecture: &str,
        manifest_symbol: &str,
        include_payload_alias: bool,
        manifest_static_shared_memory_bytes: u32,
    ) -> Fixture {
        let first_payload =
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, payload_bytes).unwrap();
        let second_payload =
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, vec![seed.wrapping_add(1); 56])
                .unwrap();
        let first_identity = first_payload.digest();
        let second_identity = second_payload.digest();
        let primary_kernel = repeated_digest(seed.wrapping_add(0x10));
        let alias_kernel = repeated_digest(seed.wrapping_add(0x11));
        let other_payload_kernel = repeated_digest(seed.wrapping_add(0x12));
        let mut kernels = vec![hsaco_kernel(
            seed.wrapping_add(0x10),
            "logical_primary",
            manifest_symbol,
            first_identity,
            manifest_static_shared_memory_bytes,
        )];
        if include_payload_alias {
            kernels.push(hsaco_kernel(
                seed.wrapping_add(0x11),
                "logical_alias",
                "alias_kernel",
                first_identity,
                manifest_static_shared_memory_bytes,
            ));
        }
        kernels.push(hsaco_kernel(
            seed.wrapping_add(0x12),
            "logical_other_payload",
            "other_payload_kernel",
            second_identity,
            0,
        ));

        let manifest = ManifestV1::new(
            CompilerIdentity::new(text("rustc"), text("1.94.0")),
            ToolIdentity::new(text("fe2o3"), text("0.1.0")),
            TargetIdentity::new(
                text("amdgcn-amd-amdhsa"),
                text(manifest_architecture),
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
            kernels,
        )
        .unwrap();
        let container = ArtifactContainerV1::new(
            manifest,
            DigestAlgorithm::Sha256,
            vec![first_payload, second_payload],
        )
        .unwrap();
        let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
        let expectations = vec![
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

    fn make_single_hsaco_fixture(
        seed: u8,
        payload_bytes: Vec<u8>,
        architecture: &str,
        abi: AbiLayout,
        launch: LaunchContract,
    ) -> Fixture {
        let payload =
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, payload_bytes).unwrap();
        let payload_identity = payload.digest();
        let primary_kernel = repeated_digest(seed.wrapping_add(0x10));
        let manifest = ManifestV1::new(
            CompilerIdentity::new(text("rustc"), text("1.94.0")),
            ToolIdentity::new(text("fe2o3"), text("0.1.0")),
            TargetIdentity::new(
                text("amdgcn-amd-amdhsa"),
                text(architecture),
                PointerWidth::Bits64,
                Endianness::Little,
                vec![],
            )
            .unwrap(),
            vec![
                CodeObjectIdentity::new(
                    payload_identity.bytes(),
                    CodeObjectFormat::NativeExecutable,
                    payload.bytes().len() as u64,
                )
                .unwrap(),
            ],
            vec![
                KernelEntry::new(
                    primary_kernel,
                    name("logical_primary"),
                    name("primary_kernel"),
                    repeated_digest(seed.wrapping_add(0x40)),
                    repeated_digest(seed.wrapping_add(0x50)),
                    payload_identity.bytes(),
                    vec![],
                    launch,
                    abi,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let container =
            ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload]).unwrap();
        let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
        let expectations = vec![expectation(seed.wrapping_add(0x20), payload_identity)];
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
            alias_kernel: repeated_digest(seed.wrapping_add(0x11)),
            other_payload_kernel: repeated_digest(seed.wrapping_add(0x12)),
        }
    }

    fn fixture_from_generated_container(seed: u8, container: ArtifactContainerV1) -> Fixture {
        assert_eq!(container.manifest().kernels().len(), 1);
        let primary_kernel = container.manifest().kernels()[0].kernel_id();
        let payload_identity = PayloadDigest::new(
            container.digest_algorithm(),
            container.manifest().kernels()[0].code_object_digest(),
        );
        let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
        let expectations = vec![expectation(seed.wrapping_add(0x20), payload_identity)];
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
            alias_kernel: repeated_digest(seed.wrapping_add(0x11)),
            other_payload_kernel: repeated_digest(seed.wrapping_add(0x12)),
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

    fn make_observed_for(identity: usize, architecture: &str) -> ObservedContext {
        ObservedContext::for_test(identity, 0, architecture, 1024, 65_536)
    }

    fn admit_hsaco(
        fixture: &Fixture,
        identity: usize,
        architecture: &str,
    ) -> ValidatedPublishedDirectLinkSelectionV1 {
        prepare_hsaco_admission(fixture, identity, architecture).admission
    }

    struct HsacoAdmission<'fixture> {
        validated: ValidatedDirectLinkBundleEvidenceV1<'fixture>,
        bridge: DirectLinkPublicationBridgeV1,
        published: PublishedLinkArtifactV1,
        selected: SelectedNativeKernel<'fixture>,
        observed: ObservedContext,
        admission: ValidatedPublishedDirectLinkSelectionV1,
    }

    fn prepare_hsaco_admission<'fixture>(
        fixture: &'fixture Fixture,
        identity: usize,
        architecture: &str,
    ) -> HsacoAdmission<'fixture> {
        let validated = fixture.validated();
        let bridge = make_bridge(fixture, &validated, 0, identity as u64, identity as u8);
        let published = publish(&bridge);
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let observed = make_observed_for(identity, architecture);
        let admission = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &bridge,
            published,
            &fixture.container,
            selected,
            &observed,
        )
        .unwrap();
        HsacoAdmission {
            validated,
            bridge,
            published,
            selected,
            observed,
            admission,
        }
    }

    struct TestHsaco {
        bytes: Vec<u8>,
        descriptor_offset: usize,
    }

    fn test_hsaco(target: &str, static_shared_memory_bytes: u32) -> TestHsaco {
        let metadata = test_metadata(
            target,
            vec![test_metadata_kernel_with(
                "primary_kernel",
                "primary_kernel.kd",
                Vec::new(),
                256,
                8,
                static_shared_memory_bytes,
                16,
                None,
                [None; 3],
                false,
            )],
        );
        binding_hsaco(metadata, target, static_shared_memory_bytes, 16, 256)
    }

    fn physical_arguments_hsaco(
        required_workgroup_size: Option<[u32; 3]>,
        max_workgroups: [Option<u32>; 3],
        dynamic_lds_size: bool,
    ) -> TestHsaco {
        physical_arguments_hsaco_with_layout(
            288,
            8,
            required_workgroup_size,
            max_workgroups,
            dynamic_lds_size,
        )
    }

    fn physical_arguments_hsaco_with_layout(
        kernarg_segment_size: u32,
        kernarg_segment_alignment: u32,
        required_workgroup_size: Option<[u32; 3]>,
        max_workgroups: [Option<u32>; 3],
        dynamic_lds_size: bool,
    ) -> TestHsaco {
        let arguments = vec![
            test_explicit_argument("scalar", 0, 4, "by_value", None, None, None, None),
            test_explicit_argument(
                "pointer",
                8,
                8,
                "global_buffer",
                Some("global"),
                Some(8),
                Some("read_only"),
                Some("read_write"),
            ),
            test_explicit_argument(
                "slice_ptr",
                16,
                8,
                "global_buffer",
                Some("global"),
                None,
                None,
                None,
            ),
            test_explicit_argument("slice_len", 24, 8, "by_value", None, None, None, None),
        ];
        let metadata = test_metadata(
            "gfx1151",
            vec![test_metadata_kernel_with(
                "primary_kernel",
                "primary_kernel.kd",
                arguments,
                kernarg_segment_size,
                kernarg_segment_alignment,
                0,
                16,
                required_workgroup_size,
                max_workgroups,
                dynamic_lds_size,
            )],
        );
        binding_hsaco(metadata, "gfx1151", 0, 16, kernarg_segment_size)
    }

    fn test_metadata(target: &str, kernels: Vec<Value>) -> Value {
        value_map(vec![
            (
                "amdhsa.version",
                Value::Array(vec![Value::from(1), Value::from(2)]),
            ),
            (
                "amdhsa.target",
                Value::from(format!("amdgcn-amd-amdhsa--{target}")),
            ),
            ("amdhsa.kernels", Value::Array(kernels)),
        ])
    }

    #[allow(clippy::too_many_arguments)]
    fn test_metadata_kernel_with(
        export_symbol: &str,
        symbol: &str,
        mut explicit_arguments: Vec<Value>,
        kernarg_segment_size: u32,
        kernarg_segment_alignment: u32,
        static_shared_memory_bytes: u32,
        private_segment_fixed_size: u32,
        required_workgroup_size: Option<[u32; 3]>,
        max_workgroups: [Option<u32>; 3],
        dynamic_lds_size: bool,
    ) -> Value {
        let explicit_size = u64::from(kernarg_segment_size) - 256;
        explicit_arguments.extend(test_hidden_arguments(explicit_size));
        if dynamic_lds_size {
            explicit_arguments.push(value_map(vec![
                (".offset", Value::from(explicit_size + 120)),
                (".size", Value::from(4)),
                (".value_kind", Value::from("hidden_dynamic_lds_size")),
            ]));
        }
        let mut fields = vec![
            (".name", Value::from(export_symbol)),
            (".symbol", Value::from(symbol)),
            (".args", Value::Array(explicit_arguments)),
            (".kernarg_segment_size", Value::from(kernarg_segment_size)),
            (
                ".kernarg_segment_align",
                Value::from(kernarg_segment_alignment),
            ),
            (
                ".group_segment_fixed_size",
                Value::from(static_shared_memory_bytes),
            ),
            (
                ".private_segment_fixed_size",
                Value::from(private_segment_fixed_size),
            ),
            (".wavefront_size", Value::from(32)),
            (".sgpr_count", Value::from(14)),
            (".vgpr_count", Value::from(7)),
            (".agpr_count", Value::from(3)),
            (".sgpr_spill_count", Value::from(2)),
            (".vgpr_spill_count", Value::from(4)),
            (".workgroup_processor_mode", Value::from(1)),
            (".max_flat_workgroup_size", Value::from(1024)),
        ];
        if let Some(required) = required_workgroup_size {
            fields.push((
                ".reqd_workgroup_size",
                Value::Array(required.into_iter().map(Value::from).collect()),
            ));
        }
        for (field, value) in [
            (".max_num_workgroups_x", max_workgroups[0]),
            (".max_num_workgroups_y", max_workgroups[1]),
            (".max_num_workgroups_z", max_workgroups[2]),
        ] {
            if let Some(value) = value {
                fields.push((field, Value::from(value)));
            }
        }
        value_map(fields)
    }

    #[allow(clippy::too_many_arguments)]
    fn test_explicit_argument(
        name: &str,
        offset: u64,
        size: u64,
        value_kind: &str,
        address_space: Option<&str>,
        alignment: Option<u64>,
        access: Option<&str>,
        actual_access: Option<&str>,
    ) -> Value {
        let mut fields = vec![
            (".name", Value::from(name)),
            (".offset", Value::from(offset)),
            (".size", Value::from(size)),
            (".value_kind", Value::from(value_kind)),
        ];
        for (field, value) in [
            (".address_space", address_space),
            (".access", access),
            (".actual_access", actual_access),
        ] {
            if let Some(value) = value {
                fields.push((field, Value::from(value)));
            }
        }
        if let Some(alignment) = alignment {
            fields.push((".align", Value::from(alignment)));
        }
        value_map(fields)
    }

    fn test_hidden_arguments(base: u64) -> Vec<Value> {
        [
            (0, 4, "hidden_block_count_x"),
            (4, 4, "hidden_block_count_y"),
            (8, 4, "hidden_block_count_z"),
            (12, 2, "hidden_group_size_x"),
            (14, 2, "hidden_group_size_y"),
            (16, 2, "hidden_group_size_z"),
            (18, 2, "hidden_remainder_x"),
            (20, 2, "hidden_remainder_y"),
            (22, 2, "hidden_remainder_z"),
            (40, 8, "hidden_global_offset_x"),
            (48, 8, "hidden_global_offset_y"),
            (56, 8, "hidden_global_offset_z"),
            (64, 2, "hidden_grid_dims"),
        ]
        .into_iter()
        .map(|(offset, size, kind)| {
            Value::Map(vec![
                (Value::from(".offset"), Value::from(base + offset)),
                (Value::from(".size"), Value::from(size)),
                (Value::from(".value_kind"), Value::from(kind)),
            ])
        })
        .collect()
    }

    fn value_map(fields: Vec<(&str, Value)>) -> Value {
        Value::Map(
            fields
                .into_iter()
                .map(|(key, value)| (Value::from(key), value))
                .collect(),
        )
    }

    fn encode_value(value: &Value) -> Vec<u8> {
        let mut bytes = Vec::new();
        write_value(&mut bytes, value).unwrap();
        bytes
    }

    fn metadata_note(metadata: &[u8]) -> Vec<u8> {
        let owner = b"AMDGPU\0";
        let mut note = Vec::new();
        note.extend_from_slice(&u32::try_from(owner.len()).unwrap().to_le_bytes());
        note.extend_from_slice(&u32::try_from(metadata.len()).unwrap().to_le_bytes());
        note.extend_from_slice(&32u32.to_le_bytes());
        note.extend_from_slice(owner);
        align_bytes(&mut note, 4);
        note.extend_from_slice(metadata);
        align_bytes(&mut note, 4);
        note
    }

    fn binding_hsaco(
        document: Value,
        target: &str,
        static_shared_memory_bytes: u32,
        private_segment_fixed_size: u32,
        kernarg_segment_size: u32,
    ) -> TestHsaco {
        const PROGRAM_HEADER_BYTES: usize = 56;
        const PROGRAM_COUNT: usize = 2;
        const SECTION_COUNT: usize = 7;
        const DESCRIPTOR_OFFSET: usize = 0x9c0;

        let note = metadata_note(&encode_value(&document));
        let first_program_header = ELF_HEADER_BYTES;
        let second_program_header = first_program_header + PROGRAM_HEADER_BYTES;
        let mut bytes = vec![0; ELF_HEADER_BYTES + PROGRAM_COUNT * PROGRAM_HEADER_BYTES];
        align_bytes(&mut bytes, 64);
        let note_offset = bytes.len();
        bytes.extend_from_slice(&note);
        align_bytes(&mut bytes, 64);
        assert!(bytes.len() <= DESCRIPTOR_OFFSET);
        bytes.resize(DESCRIPTOR_OFFSET + 64, 0);
        align_bytes(&mut bytes, 256);
        let entry_offset = bytes.len();
        bytes.resize(entry_offset + 64, 0xbf);
        let entry_address = entry_offset as u64 + 0x1000;
        let descriptor_address = DESCRIPTOR_OFFSET as u64;

        let entry_name = b"primary_kernel";
        let descriptor_name = b"primary_kernel.kd";
        let mut strings = vec![0];
        let entry_name_index = strings.len() as u32;
        strings.extend_from_slice(entry_name);
        strings.push(0);
        let descriptor_name_index = strings.len() as u32;
        strings.extend_from_slice(descriptor_name);
        strings.push(0);
        let other_name_index = strings.len() as u32;
        strings.extend_from_slice(b"other\0");
        let string_table_offset = bytes.len();
        bytes.extend_from_slice(&strings);
        align_bytes(&mut bytes, 8);

        let symbol_table_offset = bytes.len();
        bytes.resize(symbol_table_offset + 4 * 24, 0);
        let entry_symbol = symbol_table_offset + 24;
        write_test_u32(&mut bytes, entry_symbol, entry_name_index);
        bytes[entry_symbol + 4] = 0x12;
        bytes[entry_symbol + 5] = 3;
        write_test_u16(&mut bytes, entry_symbol + 6, 3);
        write_test_u64(&mut bytes, entry_symbol + 8, entry_address);
        write_test_u64(&mut bytes, entry_symbol + 16, 64);

        let descriptor_symbol = symbol_table_offset + 48;
        write_test_u32(&mut bytes, descriptor_symbol, descriptor_name_index);
        bytes[descriptor_symbol + 4] = 0x11;
        write_test_u16(&mut bytes, descriptor_symbol + 6, 2);
        write_test_u64(&mut bytes, descriptor_symbol + 8, descriptor_address);
        write_test_u64(&mut bytes, descriptor_symbol + 16, 64);

        let spare_symbol = symbol_table_offset + 72;
        write_test_u32(&mut bytes, spare_symbol, other_name_index);
        bytes[spare_symbol + 4] = 0x10;
        write_test_u16(&mut bytes, spare_symbol + 6, 0xfff1);

        let section_names = b"\0.note\0.rodata\0.text\0.strtab\0.symtab\0.shstrtab\0";
        let section_names_offset = bytes.len();
        bytes.extend_from_slice(section_names);
        align_bytes(&mut bytes, 8);
        let section_offset = bytes.len();
        bytes.resize(section_offset + SECTION_COUNT * SECTION_HEADER_BYTES, 0);

        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[6] = 1;
        bytes[7] = 64;
        bytes[8] = 4;
        write_test_u16(&mut bytes, 16, 3);
        write_test_u16(&mut bytes, 18, 224);
        write_test_u32(&mut bytes, 20, 1);
        write_test_u64(&mut bytes, 32, first_program_header as u64);
        write_test_u64(&mut bytes, 40, section_offset as u64);
        write_test_u32(&mut bytes, 48, test_elf_flags(target));
        write_test_u16(&mut bytes, 52, 64);
        write_test_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
        write_test_u16(&mut bytes, 56, PROGRAM_COUNT as u16);
        write_test_u16(&mut bytes, 58, SECTION_HEADER_BYTES as u16);
        write_test_u16(&mut bytes, 60, SECTION_COUNT as u16);
        write_test_u16(&mut bytes, 62, 6);

        write_test_u32(&mut bytes, first_program_header, 1);
        write_test_u32(&mut bytes, first_program_header + 4, 4);
        write_test_u64(&mut bytes, first_program_header + 8, 0);
        write_test_u64(&mut bytes, first_program_header + 16, 0);
        write_test_u64(
            &mut bytes,
            first_program_header + 32,
            (DESCRIPTOR_OFFSET + 64) as u64,
        );
        write_test_u64(
            &mut bytes,
            first_program_header + 40,
            (DESCRIPTOR_OFFSET + 64) as u64,
        );
        write_test_u64(&mut bytes, first_program_header + 48, 0x1000);

        write_test_u32(&mut bytes, second_program_header, 1);
        write_test_u32(&mut bytes, second_program_header + 4, 5);
        write_test_u64(&mut bytes, second_program_header + 8, entry_offset as u64);
        write_test_u64(&mut bytes, second_program_header + 16, entry_address);
        write_test_u64(&mut bytes, second_program_header + 32, 64);
        write_test_u64(&mut bytes, second_program_header + 40, 64);
        write_test_u64(&mut bytes, second_program_header + 48, 0x1000);

        let note_header = section_offset + SECTION_HEADER_BYTES;
        write_test_u32(&mut bytes, note_header, 1);
        write_test_u32(&mut bytes, note_header + 4, 7);
        write_test_u64(&mut bytes, note_header + 8, 2);
        write_test_u64(&mut bytes, note_header + 16, note_offset as u64);
        write_test_u64(&mut bytes, note_header + 24, note_offset as u64);
        write_test_u64(&mut bytes, note_header + 32, note.len() as u64);
        write_test_u64(&mut bytes, note_header + 48, 4);

        let rodata_header = section_offset + 2 * SECTION_HEADER_BYTES;
        write_test_u32(&mut bytes, rodata_header, 7);
        write_test_u32(&mut bytes, rodata_header + 4, 1);
        write_test_u64(&mut bytes, rodata_header + 8, 2);
        write_test_u64(&mut bytes, rodata_header + 16, descriptor_address);
        write_test_u64(&mut bytes, rodata_header + 24, DESCRIPTOR_OFFSET as u64);
        write_test_u64(&mut bytes, rodata_header + 32, 64);
        write_test_u64(&mut bytes, rodata_header + 48, 64);

        let text_header = section_offset + 3 * SECTION_HEADER_BYTES;
        write_test_u32(&mut bytes, text_header, 15);
        write_test_u32(&mut bytes, text_header + 4, 1);
        write_test_u64(&mut bytes, text_header + 8, 6);
        write_test_u64(&mut bytes, text_header + 16, entry_address);
        write_test_u64(&mut bytes, text_header + 24, entry_offset as u64);
        write_test_u64(&mut bytes, text_header + 32, 64);
        write_test_u64(&mut bytes, text_header + 48, 256);

        let strings_header = section_offset + 4 * SECTION_HEADER_BYTES;
        write_test_u32(&mut bytes, strings_header, 21);
        write_test_u32(&mut bytes, strings_header + 4, 3);
        write_test_u64(&mut bytes, strings_header + 24, string_table_offset as u64);
        write_test_u64(&mut bytes, strings_header + 32, strings.len() as u64);
        write_test_u64(&mut bytes, strings_header + 48, 1);

        let symbols_header = section_offset + 5 * SECTION_HEADER_BYTES;
        write_test_u32(&mut bytes, symbols_header, 29);
        write_test_u32(&mut bytes, symbols_header + 4, 2);
        write_test_u64(&mut bytes, symbols_header + 24, symbol_table_offset as u64);
        write_test_u64(&mut bytes, symbols_header + 32, 4 * 24);
        write_test_u32(&mut bytes, symbols_header + 40, 4);
        write_test_u32(&mut bytes, symbols_header + 44, 1);
        write_test_u64(&mut bytes, symbols_header + 48, 8);
        write_test_u64(&mut bytes, symbols_header + 56, 24);

        let section_strings_header = section_offset + 6 * SECTION_HEADER_BYTES;
        write_test_u32(&mut bytes, section_strings_header, 37);
        write_test_u32(&mut bytes, section_strings_header + 4, 3);
        write_test_u64(
            &mut bytes,
            section_strings_header + 24,
            section_names_offset as u64,
        );
        write_test_u64(
            &mut bytes,
            section_strings_header + 32,
            section_names.len() as u64,
        );
        write_test_u64(&mut bytes, section_strings_header + 48, 1);

        write_test_u32(&mut bytes, DESCRIPTOR_OFFSET, static_shared_memory_bytes);
        write_test_u32(
            &mut bytes,
            DESCRIPTOR_OFFSET + 4,
            private_segment_fixed_size,
        );
        write_test_u32(&mut bytes, DESCRIPTOR_OFFSET + 8, kernarg_segment_size);
        write_test_i64(
            &mut bytes,
            DESCRIPTOR_OFFSET + 16,
            i64::try_from(entry_address - descriptor_address).unwrap(),
        );
        write_test_u32(&mut bytes, DESCRIPTOR_OFFSET + 44, 0x40);
        write_test_u32(&mut bytes, DESCRIPTOR_OFFSET + 48, 0xe0af_0000);
        write_test_u32(&mut bytes, DESCRIPTOR_OFFSET + 52, 0x1391);
        write_test_u16(&mut bytes, DESCRIPTOR_OFFSET + 56, 0x041e);

        TestHsaco {
            bytes,
            descriptor_offset: DESCRIPTOR_OFFSET,
        }
    }

    fn test_elf_flags(target: &str) -> u32 {
        match target {
            "gfx1100" => 0x41,
            "gfx1151" => 0x4a,
            _ => panic!("unsupported host inspection test target {target}"),
        }
    }

    fn align_bytes(bytes: &mut Vec<u8>, alignment: usize) {
        while !bytes.len().is_multiple_of(alignment) {
            bytes.push(0);
        }
    }

    fn write_test_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_test_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_test_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn write_test_i64(bytes: &mut [u8], offset: usize, value: i64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
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
            admitted.binding_index(),
            fixture.binding_index(&validated, 0)
        );
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

    #[test]
    fn exact_published_hsaco_is_inspected_without_authority() {
        let hsaco = test_hsaco("gfx1151", 0);
        let fixture = make_hsaco_fixture(
            20,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let admission = prepare_hsaco_admission(&fixture, 20, "gfx1151");
        assert_eq!(
            fixture.container.manifest().kernels()[0].name().as_str(),
            "logical_primary"
        );
        assert_eq!(
            fixture.container.manifest().kernels()[0].symbol().as_str(),
            "primary_kernel"
        );

        let inspected = InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(
            admission.admission,
            &hsaco.bytes,
        )
        .unwrap();

        assert_eq!(inspected.code_object_version(), CodeObjectVersion::V6);
        assert_eq!(inspected.target().to_string(), "gfx1151");
        assert_eq!(inspected.kernel_count(), 1);
        assert_eq!(
            inspected.selected_kernel().export_symbol(),
            "primary_kernel"
        );
        assert_eq!(
            inspected.selected_kernel().descriptor_symbol(),
            "primary_kernel.kd"
        );
        assert!(inspected.selected_kernel().arguments().is_empty());
        assert_eq!(
            inspected
                .selected_kernel()
                .launch()
                .required_workgroup_size(),
            crate::PhysicalMetadataValueV1::Unknown
        );
        assert_eq!(
            inspected.selected_kernel().launch().max_workgroups(),
            [crate::PhysicalMetadataValueV1::Unknown; 3]
        );
        assert_eq!(
            inspected
                .selected_descriptor_binding()
                .descriptor_file_offset(),
            hsaco.descriptor_offset as u64
        );
        assert!(!inspected.authenticates_filesystem_artifact());
        assert!(!inspected.proves_compiler_provenance());
        assert!(!inspected.proves_rust_type_or_abi_agreement());
        assert!(!inspected.proves_ownership_alias_or_effects());
        assert!(!inspected.proves_complete_launch_contract());
        assert!(!inspected.grants_load_authority());
        assert!(!inspected.grants_launch_authority());
        assert_eq!(
            inspected.revalidate(
                &admission.validated,
                &admission.bridge,
                admission.published,
                &fixture.container,
                admission.selected,
                &admission.observed,
                &hsaco.bytes,
            ),
            Ok(())
        );
    }

    #[test]
    fn compiler_generated_gfx1151_container_inspects_end_to_end() {
        let fixture_bytes = include_bytes!("../tests/fixtures/gfx1151-typed-vecadd-v1.fe2o3");
        assert_eq!(fixture_bytes.len(), 6_045);
        let container = ArtifactContainerV1::from_bytes(fixture_bytes).unwrap();
        assert_eq!(container.to_bytes(), fixture_bytes);
        assert_eq!(
            container.manifest().target().architecture().as_str(),
            "gfx1151"
        );
        assert_eq!(container.manifest().kernels().len(), 1);
        assert_eq!(container.manifest().kernels()[0].name().as_str(), "vecadd");
        assert_eq!(
            container.manifest().kernels()[0].symbol().as_str(),
            "vecadd"
        );

        let fixture = fixture_from_generated_container(40, container);
        let payload = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap()
            .payload();
        let admission = admit_hsaco(&fixture, 40, "gfx1151");
        let inspected =
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admission, payload).unwrap();

        assert_eq!(inspected.code_object_version(), CodeObjectVersion::V6);
        assert_eq!(inspected.target().to_string(), "gfx1151");
        assert_eq!(inspected.selected_kernel().export_symbol(), "vecadd");
        assert_eq!(inspected.selected_kernel().descriptor_symbol(), "vecadd.kd");
        assert_eq!(
            inspected
                .selected_kernel()
                .arguments()
                .iter()
                .map(|argument| (argument.offset(), argument.size(), argument.value_kind()))
                .collect::<Vec<_>>(),
            vec![
                (0, 8, ExplicitValueKind::GlobalBuffer),
                (8, 8, ExplicitValueKind::ByValue),
                (16, 8, ExplicitValueKind::GlobalBuffer),
                (24, 8, ExplicitValueKind::ByValue),
                (32, 8, ExplicitValueKind::GlobalBuffer),
                (40, 8, ExplicitValueKind::ByValue),
            ]
        );
        let launch = inspected.selected_kernel().launch();
        assert_eq!(launch.kernarg_segment_size(), 304);
        assert_eq!(launch.kernarg_segment_alignment(), 8);
        assert_eq!(
            launch.implicit_argument_offset(),
            PhysicalMetadataValueV1::Known(48)
        );
        assert_eq!(launch.group_segment_fixed_size(), 0);
        assert_eq!(launch.private_segment_fixed_size(), 16);
        assert!(!inspected.grants_load_authority());
        assert!(!inspected.grants_launch_authority());
    }

    fn inspect_environment_generated_container(variable: &str, target: &str, seed: u8) {
        let path = std::env::var(variable).unwrap_or_else(|_| panic!("set {variable}"));
        let bytes = std::fs::read(path).unwrap();
        let container = ArtifactContainerV1::from_bytes(&bytes).unwrap();
        assert_eq!(container.to_bytes(), bytes);
        assert_eq!(
            container.manifest().target().architecture().as_str(),
            target
        );
        let fixture = fixture_from_generated_container(seed, container);
        let payload = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap()
            .payload();
        let inspected = InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(
            admit_hsaco(&fixture, usize::from(seed), target),
            payload,
        )
        .unwrap();
        assert_eq!(inspected.target().to_string(), target);
        assert!(!inspected.grants_load_authority());
        assert!(!inspected.grants_launch_authority());
    }

    #[test]
    #[ignore = "requires FE2O3_GFX942_TYPED_CONTAINER_V1 from a pinned gfx942 build"]
    fn ingests_environment_pinned_gfx942_container() {
        inspect_environment_generated_container("FE2O3_GFX942_TYPED_CONTAINER_V1", "gfx942", 42);
    }

    #[test]
    #[ignore = "requires FE2O3_GFX950_TYPED_CONTAINER_V1 from a pinned gfx950 build"]
    fn ingests_environment_pinned_gfx950_container() {
        inspect_environment_generated_container("FE2O3_GFX950_TYPED_CONTAINER_V1", "gfx950", 43);
    }

    #[test]
    fn payload_substitution_is_rejected_before_hsaco_parsing() {
        let hsaco = test_hsaco("gfx1151", 0);
        let fixture = make_hsaco_fixture(
            21,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let admitted = admit_hsaco(&fixture, 21, "gfx1151");
        let mut substitute = hsaco.bytes.clone();
        substitute[0] ^= 0xff;

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted, &substitute)
                .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::PayloadDigestMismatch
        );
    }

    #[test]
    fn malformed_payload_owned_by_the_admission_is_rejected_by_the_bounded_inspector() {
        let mut hsaco = test_hsaco("gfx1151", 0);
        hsaco.bytes[0] ^= 0xff;
        let fixture = make_hsaco_fixture(
            22,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let admitted = admit_hsaco(&fixture, 22, "gfx1151");

        assert!(matches!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted, &hsaco.bytes),
            Err(PublishedPhysicalLayoutInspectionError::Inspection(
                KernelBindingError::Inspection(InspectionError::InvalidElf("invalid ELF magic"))
            ))
        ));
    }

    #[test]
    fn unsupported_code_object_version_is_explicitly_rejected() {
        let mut hsaco = test_hsaco("gfx1151", 0);
        hsaco.bytes[8] = 7;
        let fixture = make_hsaco_fixture(
            23,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let admitted = admit_hsaco(&fixture, 23, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted, &hsaco.bytes)
                .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::Inspection(KernelBindingError::Inspection(
                InspectionError::UnsupportedCodeObjectVersion,
            ))
        );
    }

    #[test]
    fn inspected_target_must_exactly_match_the_manifest_target() {
        let hsaco = test_hsaco("gfx1100", 0);
        let fixture = make_hsaco_fixture(
            24,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let admitted = admit_hsaco(&fixture, 24, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted, &hsaco.bytes)
                .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::TargetMismatch
        );
    }

    #[test]
    fn selected_symbol_must_exactly_match_inspected_metadata() {
        let hsaco = test_hsaco("gfx1151", 0);
        let fixture = make_hsaco_fixture(
            25,
            hsaco.bytes.clone(),
            "gfx1151",
            "substituted_symbol",
            false,
            0,
        );
        let admitted = admit_hsaco(&fixture, 25, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted, &hsaco.bytes)
                .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::KernelSetMismatch
        );
    }

    #[test]
    fn alias_sharing_the_payload_cannot_bypass_exact_kernel_set_binding() {
        let hsaco = test_hsaco("gfx1151", 0);
        let fixture = make_hsaco_fixture(
            26,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            true,
            0,
        );
        let admitted = admit_hsaco(&fixture, 26, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted, &hsaco.bytes)
                .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::KernelSetMismatch
        );
    }

    #[test]
    fn inspected_metadata_must_match_the_manifest_launch_contract() {
        let hsaco = test_hsaco("gfx1151", 64);
        let fixture = make_hsaco_fixture(
            27,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let admitted = admit_hsaco(&fixture, 27, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted, &hsaco.bytes)
                .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::PhysicalLayoutMismatch {
                export_symbol: "primary_kernel".to_owned(),
                field: "static group segment size",
            }
        );
    }

    #[test]
    fn invalid_kernel_descriptor_is_rejected_after_exact_payload_binding() {
        let mut hsaco = test_hsaco("gfx1151", 0);
        hsaco.bytes[hsaco.descriptor_offset + 12] = 1;
        let fixture = make_hsaco_fixture(
            28,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let admitted = admit_hsaco(&fixture, 28, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted, &hsaco.bytes)
                .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::Inspection(
                KernelBindingError::InvalidKernelDescriptor(
                    "reserved descriptor bytes are nonzero"
                )
            )
        );
    }

    #[test]
    fn substituted_metadata_descriptor_symbol_is_rejected() {
        let metadata = test_metadata(
            "gfx1151",
            vec![test_metadata_kernel_with(
                "primary_kernel",
                "substituted_descriptor.kd",
                Vec::new(),
                256,
                8,
                0,
                16,
                None,
                [None; 3],
                false,
            )],
        );
        let hsaco = binding_hsaco(metadata, "gfx1151", 0, 16, 256);
        let fixture = make_hsaco_fixture(
            44,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(
                admit_hsaco(&fixture, 44, "gfx1151"),
                &hsaco.bytes,
            )
            .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::Inspection(
                KernelBindingError::MissingDescriptorSymbol,
            )
        );
    }

    #[test]
    fn physical_layout_evidence_does_not_overclaim_rust_argument_semantics() {
        let hsaco =
            physical_arguments_hsaco(Some([64, 1, 1]), [Some(65_535), Some(2), None], false);
        let first_fixture = make_single_hsaco_fixture(
            36,
            hsaco.bytes.clone(),
            "gfx1151",
            physical_test_abi(false),
            physical_test_launch(0),
        );
        let second_fixture = make_single_hsaco_fixture(
            37,
            hsaco.bytes.clone(),
            "gfx1151",
            physical_test_abi(true),
            physical_test_launch(0),
        );
        let first = InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(
            admit_hsaco(&first_fixture, 36, "gfx1151"),
            &hsaco.bytes,
        )
        .unwrap();
        let second = InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(
            admit_hsaco(&second_fixture, 37, "gfx1151"),
            &hsaco.bytes,
        )
        .unwrap();

        let arguments = first.selected_kernel().arguments();
        assert_eq!(arguments, second.selected_kernel().arguments());
        assert_eq!(arguments.len(), 4);
        assert_eq!(
            arguments
                .iter()
                .map(|argument| (argument.offset(), argument.size(), argument.value_kind()))
                .collect::<Vec<_>>(),
            vec![
                (0, 4, ExplicitValueKind::ByValue),
                (8, 8, ExplicitValueKind::GlobalBuffer),
                (16, 8, ExplicitValueKind::GlobalBuffer),
                (24, 8, ExplicitValueKind::ByValue),
            ]
        );
        assert_eq!(arguments[0].alignment(), PhysicalMetadataValueV1::Unknown);
        assert_eq!(arguments[1].alignment(), PhysicalMetadataValueV1::Known(8));
        assert_eq!(
            arguments[1].address_space(),
            PhysicalMetadataValueV1::Known(ArgumentAddressSpace::Global)
        );
        assert_eq!(
            arguments[1].declared_access(),
            PhysicalMetadataValueV1::Known(ArgumentAccess::ReadOnly)
        );
        assert_eq!(
            arguments[1].actual_access(),
            PhysicalMetadataValueV1::Known(ArgumentAccess::ReadWrite)
        );
        assert_eq!(
            arguments[2].declared_access(),
            PhysicalMetadataValueV1::Unknown
        );
        assert!(!first.proves_rust_type_or_abi_agreement());
        assert!(!first.proves_ownership_alias_or_effects());

        let launch = first.selected_kernel().launch();
        assert_eq!(
            launch.required_workgroup_size(),
            PhysicalMetadataValueV1::Known([64, 1, 1])
        );
        assert_eq!(
            launch.max_workgroups(),
            [
                PhysicalMetadataValueV1::Known(65_535),
                PhysicalMetadataValueV1::Known(2),
                PhysicalMetadataValueV1::Unknown,
            ]
        );
        assert_eq!(launch.kernarg_segment_size(), 288);
        assert_eq!(launch.kernarg_segment_alignment(), 8);
        assert_eq!(
            launch.implicit_argument_offset(),
            PhysicalMetadataValueV1::Known(32)
        );
        assert_eq!(launch.group_segment_fixed_size(), 0);
        assert_eq!(launch.private_segment_fixed_size(), 16);
        assert_eq!(
            launch.dynamic_shared_memory_indicator(),
            PhysicalMetadataValueV1::Unknown
        );
        assert!(!first.proves_complete_launch_contract());
    }

    #[test]
    fn represented_launch_constraints_are_directionally_checked() {
        for (hsaco, field) in [
            (
                physical_arguments_hsaco(Some([32, 1, 1]), [None; 3], false),
                "required workgroup size",
            ),
            (
                physical_arguments_hsaco(None, [Some(65_534), None, None], false),
                "maximum workgroups",
            ),
            (
                physical_arguments_hsaco(None, [None; 3], true),
                "dynamic shared-memory relation",
            ),
        ] {
            let fixture = make_single_hsaco_fixture(
                38,
                hsaco.bytes.clone(),
                "gfx1151",
                physical_test_abi(false),
                physical_test_launch(0),
            );
            assert_eq!(
                InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(
                    admit_hsaco(&fixture, 38, "gfx1151"),
                    &hsaco.bytes,
                )
                .unwrap_err(),
                PublishedPhysicalLayoutInspectionError::PhysicalLayoutMismatch {
                    export_symbol: "primary_kernel".to_owned(),
                    field,
                }
            );
        }

        let hsaco = physical_arguments_hsaco(None, [None; 3], true);
        let fixture = make_single_hsaco_fixture(
            39,
            hsaco.bytes.clone(),
            "gfx1151",
            physical_test_abi(false),
            physical_test_launch(1024),
        );
        let inspected = InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(
            admit_hsaco(&fixture, 39, "gfx1151"),
            &hsaco.bytes,
        )
        .unwrap();
        assert_eq!(
            inspected
                .selected_kernel()
                .launch()
                .dynamic_shared_memory_indicator(),
            PhysicalMetadataValueV1::Known(true)
        );
    }

    #[test]
    fn kernarg_size_and_alignment_must_cover_the_manifest_physical_layout() {
        let hsaco = physical_arguments_hsaco(None, [None; 3], false);
        let base_abi = physical_test_abi(false);
        let padded_abi =
            AbiLayout::new(40, 8, PointerWidth::Bits64, base_abi.fields().to_vec()).unwrap();
        let padded_fixture = make_single_hsaco_fixture(
            45,
            hsaco.bytes.clone(),
            "gfx1151",
            padded_abi,
            physical_test_launch(0),
        );
        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(
                admit_hsaco(&padded_fixture, 45, "gfx1151"),
                &hsaco.bytes,
            )
            .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::PhysicalLayoutMismatch {
                export_symbol: "primary_kernel".to_owned(),
                field: "explicit kernarg size",
            }
        );

        for (seed, alignment) in [(46, 4), (47, 16)] {
            let hsaco =
                physical_arguments_hsaco_with_layout(288, alignment, None, [None; 3], false);
            let alignment_fixture = make_single_hsaco_fixture(
                seed,
                hsaco.bytes.clone(),
                "gfx1151",
                physical_test_abi(false),
                physical_test_launch(0),
            );
            assert_eq!(
                InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(
                    admit_hsaco(&alignment_fixture, usize::from(seed), "gfx1151"),
                    &hsaco.bytes,
                )
                .unwrap_err(),
                PublishedPhysicalLayoutInspectionError::PhysicalLayoutMismatch {
                    export_symbol: "primary_kernel".to_owned(),
                    field: "kernarg segment alignment",
                }
            );
        }
    }

    #[test]
    fn inspection_revalidation_rejects_another_admission() {
        let hsaco = test_hsaco("gfx1151", 0);
        let first_fixture = make_hsaco_fixture(
            29,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let second_fixture = make_hsaco_fixture(
            30,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let first = prepare_hsaco_admission(&first_fixture, 29, "gfx1151");
        let second = prepare_hsaco_admission(&second_fixture, 30, "gfx1151");
        let inspected =
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(first.admission, &hsaco.bytes)
                .unwrap();

        assert_eq!(
            inspected.revalidate(
                &second.validated,
                &second.bridge,
                second.published,
                &second_fixture.container,
                second.selected,
                &second.observed,
                &hsaco.bytes,
            ),
            Err(
                PublishedPhysicalLayoutInspectionError::AdmissionRevalidation(
                    PublishedDirectLinkAdmissionError::BridgeSubstitution,
                )
            )
        );
    }

    #[test]
    fn inspection_revalidation_rejects_a_different_publication_of_identical_bytes() {
        let hsaco = test_hsaco("gfx1151", 0);
        let fixture = make_hsaco_fixture(
            31,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let first = prepare_hsaco_admission(&fixture, 31, "gfx1151");
        let second = prepare_hsaco_admission(&fixture, 32, "gfx1151");
        let inspected =
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(first.admission, &hsaco.bytes)
                .unwrap();

        assert_eq!(
            inspected.revalidate(
                &second.validated,
                &second.bridge,
                second.published,
                &fixture.container,
                second.selected,
                &second.observed,
                &hsaco.bytes,
            ),
            Err(
                PublishedPhysicalLayoutInspectionError::AdmissionRevalidation(
                    PublishedDirectLinkAdmissionError::BridgeSubstitution,
                )
            )
        );
    }

    #[test]
    fn inspection_revalidation_binds_context_identity_limits_and_capabilities() {
        let hsaco = test_hsaco("gfx1151", 0);
        let fixture = make_hsaco_fixture(
            33,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let admission = prepare_hsaco_admission(&fixture, 33, "gfx1151");
        let wrong_context = make_observed_for(34, "gfx1151");
        let changed_limits = ObservedContext::for_test(33, 0, "gfx1151", 512, 65_536);
        let changed_capabilities = admission
            .observed
            .clone()
            .with_changed_test_hip_capabilities();
        let inspected = InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(
            admission.admission,
            &hsaco.bytes,
        )
        .unwrap();

        for (observed, expected) in [
            (wrong_context, ArtifactRevalidationError::WrongContext),
            (
                changed_limits,
                ArtifactRevalidationError::DeviceLimitsChanged,
            ),
            (
                changed_capabilities,
                ArtifactRevalidationError::DeviceCapabilitiesChanged,
            ),
        ] {
            assert_eq!(
                inspected.revalidate(
                    &admission.validated,
                    &admission.bridge,
                    admission.published,
                    &fixture.container,
                    admission.selected,
                    &observed,
                    &hsaco.bytes,
                ),
                Err(
                    PublishedPhysicalLayoutInspectionError::AdmissionRevalidation(
                        PublishedDirectLinkAdmissionError::ArtifactRevalidation(expected),
                    )
                )
            );
        }
    }
}
