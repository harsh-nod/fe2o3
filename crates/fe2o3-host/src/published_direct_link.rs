use crate::{
    ArtifactBindingError, ArtifactRevalidationError, ObservedContext, ValidatedArtifactSelectionV1,
};
use fe2o3_artifact_transaction::{DurableLinkPublicationError, PublishedLinkArtifactV1};
use fe2o3_artifacts::{
    AbiLayout, ArtifactContainerV1, DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM, DigestBytes,
    DirectLinkBridgeError, DirectLinkContainerIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
    LaunchContract, ManifestClaimDirectLinkCurrentPublicationLeaseV1,
    ManifestClaimDirectLinkCurrentPublicationTokenV1, ManifestClaimDirectLinkDurablePlanHandoffV1,
    ManifestClaimDirectLinkPublicationBridgeV1, Name, SelectedNativeKernel,
    ValidatedDirectLinkBundleEvidenceV1,
};
use std::fmt;

/// An opaque, inert host-side admission of one structurally validated G5/G6 selection.
///
/// Construction consumes a manifest-claim-derived exact-file-handle lease and binds it with one
/// validated direct-link evidence envelope, bridge, canonical container identity, finalized
/// payload occurrence, selected kernel, complete manifest launch claims, and observed context. The
/// token owns the existing structural [`ValidatedArtifactSelectionV1`] result and can revalidate
/// the complete input tuple and current durable generation against substitutions.
///
/// The legacy bridge is structurally excluded:
///
/// ```compile_fail
/// use fe2o3_artifacts::{
///     DirectLinkPublicationBridgeV1, ManifestClaimDirectLinkPublicationBridgeV1,
/// };
///
/// fn g7_requires_manifest_claim(_: &ManifestClaimDirectLinkPublicationBridgeV1) {}
///
/// fn legacy_cannot_enter_g7(legacy: &DirectLinkPublicationBridgeV1) {
///     g7_requires_manifest_claim(legacy);
/// }
/// ```
///
/// This value retains locally authenticated record and artifact descriptors. It authenticates no
/// compiler marker, does not establish that the executable is safe, and grants no module-loading
/// or kernel-launch authority.
pub struct ValidatedPublishedDirectLinkSelectionV1 {
    selection: ValidatedArtifactSelectionV1,
    bridge: ManifestClaimDirectLinkPublicationBridgeV1,
    durable_handoff: ManifestClaimDirectLinkDurablePlanHandoffV1,
    current_lease: ManifestClaimDirectLinkCurrentPublicationLeaseV1,
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
            .field("durable_handoff", &self.durable_handoff)
            .field("current_lease", &self.current_lease)
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
        bridge: &ManifestClaimDirectLinkPublicationBridgeV1,
        current_lease: ManifestClaimDirectLinkCurrentPublicationLeaseV1,
        container: &ArtifactContainerV1,
        selected: SelectedNativeKernel<'_>,
        observed: &ObservedContext,
    ) -> Result<Self, PublishedDirectLinkAdmissionError> {
        let durable_handoff = bridge.durable_plan_handoff();
        if !current_lease.is_bound_to_handoff(&durable_handoff) {
            return Err(PublishedDirectLinkAdmissionError::CurrentLeaseSubstitution);
        }
        let published = current_lease.published();
        let _current = current_lease
            .acquire_current_token()
            .map_err(PublishedDirectLinkAdmissionError::current_publication)?;
        let (binding_index, container_identity, finalized_payload_identity) =
            validate_direct_link_inputs(
                validated_bundle,
                bridge,
                &durable_handoff,
                published,
                container,
                selected,
            )?;
        let selection = ValidatedArtifactSelectionV1::validate(selected, observed)
            .map_err(PublishedDirectLinkAdmissionError::ArtifactSelection)?;
        let payload_kernel_set = payload_kernel_set(selected);
        drop(_current);

        Ok(Self {
            selection,
            bridge: bridge.clone(),
            durable_handoff,
            current_lease,
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
        bridge: &ManifestClaimDirectLinkPublicationBridgeV1,
        container: &ArtifactContainerV1,
        selected: SelectedNativeKernel<'_>,
        observed: &ObservedContext,
    ) -> Result<(), PublishedDirectLinkAdmissionError> {
        let current = self
            .acquire_current_token()
            .map_err(PublishedDirectLinkAdmissionError::current_publication)?;
        self.revalidate_with_current_token(
            &current,
            validated_bundle,
            bridge,
            container,
            selected,
            observed,
        )
    }

    /// Revalidates with an already-held currentness token without reacquiring its lock.
    #[allow(clippy::too_many_arguments)]
    pub fn revalidate_with_current_token(
        &self,
        current: &ManifestClaimDirectLinkCurrentPublicationTokenV1,
        validated_bundle: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        bridge: &ManifestClaimDirectLinkPublicationBridgeV1,
        container: &ArtifactContainerV1,
        selected: SelectedNativeKernel<'_>,
        observed: &ObservedContext,
    ) -> Result<(), PublishedDirectLinkAdmissionError> {
        self.current_lease
            .validate_current_token(current)
            .map_err(PublishedDirectLinkAdmissionError::current_publication)?;
        if bridge != &self.bridge {
            return Err(PublishedDirectLinkAdmissionError::BridgeSubstitution);
        }
        let durable_handoff = bridge.durable_plan_handoff();
        if durable_handoff != self.durable_handoff
            || !self.current_lease.is_bound_to_handoff(&durable_handoff)
        {
            return Err(PublishedDirectLinkAdmissionError::BridgeSubstitution);
        }
        let published = self.current_lease.published();

        let (binding_index, container_identity, finalized_payload_identity) =
            validate_direct_link_inputs(
                validated_bundle,
                bridge,
                &durable_handoff,
                published,
                container,
                selected,
            )?;
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

    pub fn published(&self) -> PublishedLinkArtifactV1 {
        self.current_lease.published()
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

    pub(crate) fn exact_artifact_bytes(&self) -> &[u8] {
        self.current_lease.exact_artifact_bytes()
    }

    pub(crate) fn acquire_current_token(
        &self,
    ) -> Result<ManifestClaimDirectLinkCurrentPublicationTokenV1, DurableLinkPublicationError> {
        self.current_lease.acquire_current_token()
    }

    pub(crate) fn validate_current_token(
        &self,
        current: &ManifestClaimDirectLinkCurrentPublicationTokenV1,
    ) -> Result<(), DurableLinkPublicationError> {
        self.current_lease.validate_current_token(current)
    }

    /// Admission retains a locally validated exact-file-handle lease.
    pub const fn authenticates_filesystem_artifact(&self) -> bool {
        true
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

pub(crate) fn payload_kernel_set(
    selected: SelectedNativeKernel<'_>,
) -> Box<[PublishedPayloadKernelV1]> {
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
    bridge: &ManifestClaimDirectLinkPublicationBridgeV1,
    durable_handoff: &ManifestClaimDirectLinkDurablePlanHandoffV1,
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
    if durable_handoff.bundle_index_identity()
        != validated_bundle.evidence().bundle_index_identity()
        || durable_handoff.evidence_identity()
            != validated_bundle
                .evidence()
                .digest(DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM)
    {
        return Err(PublishedDirectLinkAdmissionError::EvidenceBridgeMismatch);
    }
    let binding_index = unique_binding_index(validated_bundle, durable_handoff)
        .ok_or(PublishedDirectLinkAdmissionError::EvidenceBridgeMismatch)?;

    bridge
        .validate_published(published)
        .map_err(PublishedDirectLinkAdmissionError::PublicationBridge)?;

    let container_identity = DirectLinkContainerIdentityV1::new(
        DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM.calculate(&container.to_bytes()),
    );
    if durable_handoff.container_identity() != container_identity
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

    let finalized_payload_identity = durable_handoff.finalized_payload_identity();
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
    durable_handoff: &ManifestClaimDirectLinkDurablePlanHandoffV1,
) -> Option<usize> {
    let mut matches = validated_bundle
        .bindings()
        .iter()
        .enumerate()
        .filter(|(_, binding)| {
            binding.container_identity() == durable_handoff.container_identity()
                && binding.expectation().finalized_payload_identity()
                    == durable_handoff.finalized_payload_identity()
        });
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
    Busy,
    EvidenceBridgeMismatch,
    PublicationBridge(DirectLinkBridgeError),
    ContainerIdentityMismatch,
    SelectedKernelContainerMismatch,
    FinalizedPayloadMismatch,
    PayloadKernelSetSubstitution,
    ArtifactSelection(ArtifactBindingError),
    BridgeSubstitution,
    PublicationSubstitution,
    CurrentLeaseSubstitution,
    CurrentPublication { reason: String },
    BindingIndexSubstitution,
    ArtifactRevalidation(ArtifactRevalidationError),
}

impl fmt::Display for PublishedDirectLinkAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => formatter.write_str("durable publication lock is busy"),
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
            Self::CurrentLeaseSubstitution => formatter
                .write_str("current publication lease differs from the manifest-claim handoff"),
            Self::CurrentPublication { reason } => {
                write!(
                    formatter,
                    "current publication lease failed revalidation: {reason}"
                )
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
            | Self::Busy
            | Self::ContainerIdentityMismatch
            | Self::SelectedKernelContainerMismatch
            | Self::FinalizedPayloadMismatch
            | Self::PayloadKernelSetSubstitution
            | Self::BridgeSubstitution
            | Self::PublicationSubstitution
            | Self::CurrentLeaseSubstitution
            | Self::CurrentPublication { .. }
            | Self::BindingIndexSubstitution => None,
        }
    }
}

impl PublishedDirectLinkAdmissionError {
    fn current_publication(error: DurableLinkPublicationError) -> Self {
        match error {
            DurableLinkPublicationError::Busy => Self::Busy,
            error => Self::CurrentPublication {
                reason: error.to_string(),
            },
        }
    }
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
pub(crate) mod tests {
    #![cfg_attr(not(test), allow(dead_code, unused_imports))]

    use super::*;
    use crate::{
        InspectedPublishedDirectLinkPhysicalLayoutV1, MissingPublishedDirectLinkLoadPrerequisiteV1,
        PhysicalMetadataValueV1, PublishedLoadAdmissionError,
        PublishedPhysicalLayoutInspectionError,
    };
    use fe2o3_artifact_transaction::{BuildAttempt, PackageIdentityV1};
    use fe2o3_artifacts::{
        AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
        BlockSize, BundleIndexV1, CallerClaimedPackageIdentityV1, CodeObjectFormat,
        CodeObjectIdentity, CodeObjectPayload, CompilerIdentity, DeclaredRustLayoutIdentity,
        DeclaredRustTypeIdentity, DigestAlgorithm, DigestBytes, Dimensions,
        DirectLinkBindingExpectationV1, DirectLinkBindingSourceV1, DirectLinkBundleEvidenceV1,
        DirectLinkFfiClosureIdentityV1, DirectLinkFinalizationIdentityV1,
        DirectLinkFinalizedPayloadIdentityV1, DirectLinkLinkedOutputIdentityV1,
        DirectLinkRequestIdentityV1, DirectLinkResponseIdentityV1,
        DirectLinkToolchainConfigurationIdentityV1, DirectLinkToolchainExecutableIdentityV1,
        DirectLinkToolchainIdentityV1, DirectLinkTransformationIdentityV1,
        DirectLinkWorkerConfigurationIdentityV1, DirectLinkWorkerExecutableIdentityV1,
        DirectLinkWorkerIdentityV1, Endianness, IdentityText, KernelEntry, LaunchContract,
        ManifestClaimDerivedLinkPublicationScopeV1,
        ManifestClaimDirectLinkCurrentPublicationLeaseV1, ManifestV1, Mutability, Name,
        PayloadDigest, PointerWidth, ScalarType, TargetIdentity, ToolIdentity, TypeIdentity,
        publish_manifest_claim_direct_link_durable_v1,
    };
    use fe2o3_hsaco::{
        ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion, ExplicitValueKind,
        InspectionError, KernelBindingError,
    };
    use rmpv::{Value, encode::write_value};
    use std::fs;
    use std::path::PathBuf;
    use std::process::{Child, Command, ExitStatus, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    const ELF_HEADER_BYTES: usize = 64;
    const SECTION_HEADER_BYTES: usize = 64;

    static NEXT_PUBLICATION_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    fn wait_for_child(mut child: Child, timeout: Duration) -> ExitStatus {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                return status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                let _ = child.wait();
                panic!("token-aware host child exceeded {timeout:?}");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    struct TestPublicationDirectory {
        path: PathBuf,
    }

    impl TestPublicationDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fe2o3-host-current-publication-{}-{}",
                std::process::id(),
                NEXT_PUBLICATION_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TestPublicationDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    pub(crate) struct Fixture {
        pub(crate) container: ArtifactContainerV1,
        pub(crate) bundle: BundleIndexV1,
        pub(crate) expectations: Vec<DirectLinkBindingExpectationV1>,
        pub(crate) evidence: DirectLinkBundleEvidenceV1,
        pub(crate) primary_kernel: DigestBytes,
        alias_kernel: DigestBytes,
        other_payload_kernel: DigestBytes,
    }

    impl Fixture {
        pub(crate) fn validated(&self) -> ValidatedDirectLinkBundleEvidenceV1<'_> {
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

    pub(crate) fn physical_test_abi(alternate_semantics: bool) -> AbiLayout {
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
        physical_test_launch_with_rank(1, max_dynamic_shared_memory_bytes)
    }

    fn physical_test_launch_with_rank(
        rank: u8,
        max_dynamic_shared_memory_bytes: u32,
    ) -> LaunchContract {
        let max_grid = match rank {
            1 => Dimensions::new(65_535, 1, 1),
            2 => Dimensions::new(65_535, 65_535, 1),
            3 => Dimensions::new(65_535, 65_535, 65_535),
            _ => panic!("unsupported test rank {rank}"),
        }
        .unwrap();
        LaunchContract::new(
            rank,
            BlockSize::Exact(Dimensions::new(64, 1, 1).unwrap()),
            max_grid,
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

    pub(crate) fn make_single_hsaco_fixture(
        seed: u8,
        payload_bytes: Vec<u8>,
        architecture: &str,
        abi: AbiLayout,
        launch: LaunchContract,
    ) -> Fixture {
        make_single_hsaco_fixture_with_kernel_id(
            seed,
            payload_bytes,
            architecture,
            abi,
            launch,
            repeated_digest(seed.wrapping_add(0x10)),
        )
    }

    pub(crate) fn make_single_hsaco_fixture_with_kernel_id(
        seed: u8,
        payload_bytes: Vec<u8>,
        architecture: &str,
        abi: AbiLayout,
        launch: LaunchContract,
        primary_kernel: DigestBytes,
    ) -> Fixture {
        make_single_hsaco_fixture_with_names_and_kernel_id(
            seed,
            payload_bytes,
            architecture,
            "logical_primary",
            "primary_kernel",
            abi,
            launch,
            primary_kernel,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn make_single_hsaco_fixture_with_names_and_kernel_id(
        seed: u8,
        payload_bytes: Vec<u8>,
        architecture: &str,
        logical_name: &str,
        export_symbol: &str,
        abi: AbiLayout,
        launch: LaunchContract,
        primary_kernel: DigestBytes,
    ) -> Fixture {
        let payload =
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, payload_bytes).unwrap();
        let payload_identity = payload.digest();
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
                    name(logical_name),
                    name(export_symbol),
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn make_two_hsaco_fixture_with_kernel_ids(
        seed: u8,
        payload_bytes: Vec<u8>,
        architecture: &str,
        first_logical_name: &str,
        first_export_symbol: &str,
        first_kernel: DigestBytes,
        second_logical_name: &str,
        second_export_symbol: &str,
        second_kernel: DigestBytes,
        abi: AbiLayout,
        launch: LaunchContract,
    ) -> Fixture {
        make_two_hsaco_fixture_with_kernel_ids_and_abis(
            seed,
            payload_bytes,
            architecture,
            first_logical_name,
            first_export_symbol,
            first_kernel,
            abi.clone(),
            second_logical_name,
            second_export_symbol,
            second_kernel,
            abi,
            launch,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn make_two_hsaco_fixture_with_kernel_ids_and_abis(
        seed: u8,
        payload_bytes: Vec<u8>,
        architecture: &str,
        first_logical_name: &str,
        first_export_symbol: &str,
        first_kernel: DigestBytes,
        first_abi: AbiLayout,
        second_logical_name: &str,
        second_export_symbol: &str,
        second_kernel: DigestBytes,
        second_abi: AbiLayout,
        launch: LaunchContract,
    ) -> Fixture {
        let payload =
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, payload_bytes).unwrap();
        let payload_identity = payload.digest();
        let kernel =
            |kernel_id, logical_name: &str, export_symbol: &str, identity_seed, abi: AbiLayout| {
                KernelEntry::new(
                    kernel_id,
                    name(logical_name),
                    name(export_symbol),
                    repeated_digest(identity_seed),
                    repeated_digest(identity_seed.wrapping_add(0x10)),
                    payload_identity.bytes(),
                    vec![],
                    launch.clone(),
                    abi,
                )
                .unwrap()
            };
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
                kernel(
                    first_kernel,
                    first_logical_name,
                    first_export_symbol,
                    seed.wrapping_add(0x40),
                    first_abi,
                ),
                kernel(
                    second_kernel,
                    second_logical_name,
                    second_export_symbol,
                    seed.wrapping_add(0x41),
                    second_abi,
                ),
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
            primary_kernel: first_kernel,
            alias_kernel: second_kernel,
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

    fn make_bridge(
        fixture: &Fixture,
        validated: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        expectation_index: usize,
        generation: u64,
        seed: u8,
    ) -> ManifestClaimDirectLinkPublicationBridgeV1 {
        let binding_index = fixture.binding_index(validated, expectation_index);
        let manifest_claim_scope = ManifestClaimDerivedLinkPublicationScopeV1::derive(
            CallerClaimedPackageIdentityV1::new(PackageIdentityV1::from_bytes([seed; 32])),
            validated,
            binding_index,
            &fixture.container,
        )
        .unwrap();
        ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
            attempt(generation, seed),
            manifest_claim_scope,
            validated,
            binding_index,
        )
        .unwrap()
    }

    fn publish(
        directory: &TestPublicationDirectory,
        bridge: &ManifestClaimDirectLinkPublicationBridgeV1,
        bytes: &[u8],
    ) -> ManifestClaimDirectLinkCurrentPublicationLeaseV1 {
        publish_manifest_claim_direct_link_durable_v1(
            &directory.path,
            &bridge.durable_plan_handoff(),
            |transaction| {
                transaction.record_worker_pinned()?;
                transaction.record_response_validated()?;
                transaction.record_finalized(bytes)
            },
        )
        .unwrap()
        .into_current_lease()
    }

    fn make_observed(identity: usize) -> ObservedContext {
        ObservedContext::for_test(identity, 0, "gfx1100", 1024, 65_536)
    }

    pub(crate) fn make_observed_for(identity: usize, architecture: &str) -> ObservedContext {
        ObservedContext::for_test(identity, 0, architecture, 1024, 65_536)
    }

    struct HsacoAdmission<'fixture> {
        _publication_directory: TestPublicationDirectory,
        validated: ValidatedDirectLinkBundleEvidenceV1<'fixture>,
        bridge: ManifestClaimDirectLinkPublicationBridgeV1,
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
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let publication_directory = TestPublicationDirectory::new();
        let current_lease = publish(&publication_directory, &bridge, selected.payload());
        let observed = make_observed_for(identity, architecture);
        let admission = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &bridge,
            current_lease,
            &fixture.container,
            selected,
            &observed,
        )
        .unwrap();
        HsacoAdmission {
            _publication_directory: publication_directory,
            validated,
            bridge,
            selected,
            observed,
            admission,
        }
    }

    pub(crate) struct TestHsaco {
        pub(crate) bytes: Vec<u8>,
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
        physical_arguments_hsaco_for_target(
            "gfx1151",
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
        physical_arguments_hsaco_for_target(
            "gfx1151",
            kernarg_segment_size,
            kernarg_segment_alignment,
            required_workgroup_size,
            max_workgroups,
            dynamic_lds_size,
        )
    }

    pub(crate) fn physical_arguments_hsaco_for_target(
        target: &str,
        kernarg_segment_size: u32,
        kernarg_segment_alignment: u32,
        required_workgroup_size: Option<[u32; 3]>,
        max_workgroups: [Option<u32>; 3],
        dynamic_lds_size: bool,
    ) -> TestHsaco {
        let private_segment_fixed_size: u32 = if target.starts_with("gfx94") { 0 } else { 16 };
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
            target,
            vec![test_metadata_kernel_with_wavefront(
                "primary_kernel",
                "primary_kernel.kd",
                arguments,
                kernarg_segment_size,
                kernarg_segment_alignment,
                0,
                private_segment_fixed_size,
                required_workgroup_size,
                max_workgroups,
                dynamic_lds_size,
                if target.starts_with("gfx94") { 64 } else { 32 },
            )],
        );
        binding_hsaco(
            metadata,
            target,
            0,
            private_segment_fixed_size,
            kernarg_segment_size,
        )
    }

    pub(crate) fn typed_vecadd_hsaco_for_target(target: &str) -> TestHsaco {
        let private_segment_fixed_size: u32 = if target.starts_with("gfx94") { 0 } else { 16 };
        let mut arguments = Vec::new();
        for (index, access) in [(0, "read_only"), (1, "read_only"), (2, "write_only")] {
            let offset = index * 16;
            arguments.push(test_explicit_argument(
                &format!("arg{index}_ptr"),
                offset,
                8,
                "global_buffer",
                Some("global"),
                Some(8),
                Some(access),
                Some(access),
            ));
            arguments.push(test_explicit_argument(
                &format!("arg{index}_len"),
                offset + 8,
                8,
                "by_value",
                None,
                None,
                None,
                None,
            ));
        }
        let metadata = test_metadata(
            target,
            vec![test_metadata_kernel_with_wavefront(
                "primary_kernel",
                "primary_kernel.kd",
                arguments,
                304,
                8,
                0,
                private_segment_fixed_size,
                Some([256, 1, 1]),
                [None; 3],
                false,
                if target.starts_with("gfx94") { 64 } else { 32 },
            )],
        );
        binding_hsaco(metadata, target, 0, private_segment_fixed_size, 304)
    }

    pub(crate) fn alpha_cov6_hsaco_for_target(target: &str) -> TestHsaco {
        let private_segment_fixed_size: u32 = if target.starts_with("gfx94") { 0 } else { 16 };
        let arguments = vec![
            test_explicit_argument("scale", 0, 4, "by_value", None, None, None, None),
            test_explicit_argument(
                "input_ptr",
                8,
                8,
                "global_buffer",
                Some("global"),
                Some(8),
                Some("read_only"),
                Some("read_only"),
            ),
            test_explicit_argument("input_len", 16, 8, "by_value", None, None, None, None),
            test_explicit_argument(
                "output_ptr",
                24,
                8,
                "global_buffer",
                Some("global"),
                Some(8),
                Some("read_write"),
                Some("read_write"),
            ),
            test_explicit_argument("output_len", 32, 8, "by_value", None, None, None, None),
        ];
        let metadata = test_metadata(
            target,
            vec![test_metadata_kernel_with_wavefront(
                "alpha",
                "alpha.kd",
                arguments,
                296,
                8,
                0,
                private_segment_fixed_size,
                Some([256, 1, 1]),
                [None; 3],
                false,
                if target.starts_with("gfx94") { 64 } else { 32 },
            )],
        );
        binding_hsaco_with_kernel_names(
            metadata,
            target,
            0,
            private_segment_fixed_size,
            296,
            ("alpha", "alpha.kd"),
            None,
        )
    }

    pub(crate) fn scalar_gemm_v1_hsaco_for_target(target: &str) -> TestHsaco {
        let private_segment_fixed_size: u32 = if target.starts_with("gfx94") { 0 } else { 16 };
        let slice_arguments = |name: &str, offset: u64, access: &'static str| {
            vec![
                test_explicit_argument(
                    &format!("{name}_ptr"),
                    offset,
                    8,
                    "global_buffer",
                    Some("global"),
                    Some(8),
                    Some(access),
                    Some(access),
                ),
                test_explicit_argument(
                    &format!("{name}_len"),
                    offset + 8,
                    8,
                    "by_value",
                    None,
                    None,
                    None,
                    None,
                ),
            ]
        };
        let mut arguments = slice_arguments("a", 0, "read_only");
        arguments.extend(slice_arguments("b", 16, "read_only"));
        arguments.extend(slice_arguments("c", 32, "read_write"));
        for (name, offset) in [("m", 48), ("n", 52), ("k", 56)] {
            arguments.push(test_explicit_argument(
                name, offset, 4, "by_value", None, None, None, None,
            ));
        }
        let metadata = test_metadata(
            target,
            vec![test_metadata_kernel_with_wavefront(
                "scalar_gemm_v1",
                "scalar_gemm_v1.kd",
                arguments,
                320,
                8,
                0,
                private_segment_fixed_size,
                Some([256, 1, 1]),
                [None; 3],
                false,
                if target.starts_with("gfx94") { 64 } else { 32 },
            )],
        );
        binding_hsaco_with_kernel_names(
            metadata,
            target,
            0,
            private_segment_fixed_size,
            320,
            ("scalar_gemm_v1", "scalar_gemm_v1.kd"),
            None,
        )
    }

    pub(crate) fn alpha_zeta_cov6_hsaco_for_target(target: &str) -> TestHsaco {
        let private_segment_fixed_size: u32 = if target.starts_with("gfx94") { 0 } else { 16 };
        let slice_arguments = |name: &str, offset: u64, access: &'static str| {
            vec![
                test_explicit_argument(
                    &format!("{name}_ptr"),
                    offset,
                    8,
                    "global_buffer",
                    Some("global"),
                    Some(8),
                    Some(access),
                    Some(access),
                ),
                test_explicit_argument(
                    &format!("{name}_len"),
                    offset + 8,
                    8,
                    "by_value",
                    None,
                    None,
                    None,
                    None,
                ),
            ]
        };
        let mut alpha_arguments = vec![test_explicit_argument(
            "scale", 0, 4, "by_value", None, None, None, None,
        )];
        alpha_arguments.extend(slice_arguments("input", 8, "read_only"));
        alpha_arguments.extend(slice_arguments("output", 24, "read_write"));

        let mut zeta_arguments = slice_arguments("a", 0, "read_only");
        zeta_arguments.extend(slice_arguments("b", 16, "read_only"));
        zeta_arguments.push(test_explicit_argument(
            "bias", 32, 4, "by_value", None, None, None, None,
        ));
        zeta_arguments.extend(slice_arguments("output", 40, "read_write"));

        let metadata = test_metadata(
            target,
            vec![
                test_metadata_kernel_with_wavefront(
                    "alpha",
                    "alpha.kd",
                    alpha_arguments,
                    296,
                    8,
                    0,
                    private_segment_fixed_size,
                    Some([256, 1, 1]),
                    [None; 3],
                    false,
                    if target.starts_with("gfx94") { 64 } else { 32 },
                ),
                test_metadata_kernel_with_wavefront(
                    "zeta",
                    "zeta.kd",
                    zeta_arguments,
                    312,
                    8,
                    0,
                    private_segment_fixed_size,
                    Some([256, 1, 1]),
                    [None; 3],
                    false,
                    if target.starts_with("gfx94") { 64 } else { 32 },
                ),
            ],
        );
        binding_hsaco_with_kernel_layouts(
            metadata,
            target,
            0,
            private_segment_fixed_size,
            ("alpha", "alpha.kd", 296),
            Some(("zeta", "zeta.kd", 312)),
        )
    }

    pub(crate) fn typed_vecadd_two_kernel_hsaco_for_target(target: &str) -> TestHsaco {
        let private_segment_fixed_size: u32 = if target.starts_with("gfx94") { 0 } else { 16 };
        let mut arguments = Vec::new();
        for (index, access) in [(0, "read_only"), (1, "read_only"), (2, "write_only")] {
            let offset = index * 16;
            arguments.push(test_explicit_argument(
                &format!("arg{index}_ptr"),
                offset,
                8,
                "global_buffer",
                Some("global"),
                Some(8),
                Some(access),
                Some(access),
            ));
            arguments.push(test_explicit_argument(
                &format!("arg{index}_len"),
                offset + 8,
                8,
                "by_value",
                None,
                None,
                None,
                None,
            ));
        }
        let kernel = |export: &str, descriptor: &str| {
            test_metadata_kernel_with_wavefront(
                export,
                descriptor,
                arguments.clone(),
                304,
                8,
                0,
                private_segment_fixed_size,
                Some([256, 1, 1]),
                [None; 3],
                false,
                if target.starts_with("gfx94") { 64 } else { 32 },
            )
        };
        let metadata = test_metadata(
            target,
            vec![
                kernel("primary_kernel", "primary_kernel.kd"),
                kernel("second_kernel", "second_kernel.kd"),
            ],
        );
        binding_hsaco_with_optional_second_kernel(
            metadata,
            target,
            0,
            private_segment_fixed_size,
            304,
            Some(("second_kernel", "second_kernel.kd")),
        )
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
        explicit_arguments: Vec<Value>,
        kernarg_segment_size: u32,
        kernarg_segment_alignment: u32,
        static_shared_memory_bytes: u32,
        private_segment_fixed_size: u32,
        required_workgroup_size: Option<[u32; 3]>,
        max_workgroups: [Option<u32>; 3],
        dynamic_lds_size: bool,
    ) -> Value {
        test_metadata_kernel_with_wavefront(
            export_symbol,
            symbol,
            explicit_arguments,
            kernarg_segment_size,
            kernarg_segment_alignment,
            static_shared_memory_bytes,
            private_segment_fixed_size,
            required_workgroup_size,
            max_workgroups,
            dynamic_lds_size,
            32,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn test_metadata_kernel_with_wavefront(
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
        wavefront_size: u32,
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
            (".wavefront_size", Value::from(wavefront_size)),
            (".sgpr_count", Value::from(14)),
            (
                ".vgpr_count",
                Value::from(if wavefront_size == 64 { 11 } else { 7 }),
            ),
            (".agpr_count", Value::from(3)),
            (".max_flat_workgroup_size", Value::from(1024)),
        ];
        if wavefront_size == 32 {
            fields.push((".sgpr_spill_count", Value::from(2)));
            fields.push((".vgpr_spill_count", Value::from(4)));
            fields.push((".workgroup_processor_mode", Value::from(1)));
        }
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
        binding_hsaco_with_optional_second_kernel(
            document,
            target,
            static_shared_memory_bytes,
            private_segment_fixed_size,
            kernarg_segment_size,
            None,
        )
    }

    fn binding_hsaco_with_optional_second_kernel(
        document: Value,
        target: &str,
        static_shared_memory_bytes: u32,
        private_segment_fixed_size: u32,
        kernarg_segment_size: u32,
        second_kernel: Option<(&str, &str)>,
    ) -> TestHsaco {
        binding_hsaco_with_kernel_names(
            document,
            target,
            static_shared_memory_bytes,
            private_segment_fixed_size,
            kernarg_segment_size,
            ("primary_kernel", "primary_kernel.kd"),
            second_kernel,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn binding_hsaco_with_kernel_names(
        document: Value,
        target: &str,
        static_shared_memory_bytes: u32,
        private_segment_fixed_size: u32,
        kernarg_segment_size: u32,
        first_kernel: (&str, &str),
        second_kernel: Option<(&str, &str)>,
    ) -> TestHsaco {
        binding_hsaco_with_kernel_layouts(
            document,
            target,
            static_shared_memory_bytes,
            private_segment_fixed_size,
            (first_kernel.0, first_kernel.1, kernarg_segment_size),
            second_kernel.map(|(entry, descriptor)| (entry, descriptor, kernarg_segment_size)),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn binding_hsaco_with_kernel_layouts(
        document: Value,
        target: &str,
        static_shared_memory_bytes: u32,
        private_segment_fixed_size: u32,
        first_kernel: (&str, &str, u32),
        second_kernel: Option<(&str, &str, u32)>,
    ) -> TestHsaco {
        const PROGRAM_HEADER_BYTES: usize = 56;
        const PROGRAM_COUNT: usize = 2;
        const SECTION_COUNT: usize = 7;
        const SINGLE_DESCRIPTOR_OFFSET: usize = 0x9c0;
        let kernel_count = if second_kernel.is_some() { 2 } else { 1 };
        let descriptor_bytes = 64 * kernel_count;
        let entry_stride = if second_kernel.is_some() { 256 } else { 64 };
        let entry_bytes = entry_stride * kernel_count;
        let descriptor_file_offset = if second_kernel.is_some() {
            0x1800
        } else {
            SINGLE_DESCRIPTOR_OFFSET
        };

        let note = metadata_note(&encode_value(&document));
        let first_program_header = ELF_HEADER_BYTES;
        let second_program_header = first_program_header + PROGRAM_HEADER_BYTES;
        let mut bytes = vec![0; ELF_HEADER_BYTES + PROGRAM_COUNT * PROGRAM_HEADER_BYTES];
        align_bytes(&mut bytes, 64);
        let note_offset = bytes.len();
        bytes.extend_from_slice(&note);
        align_bytes(&mut bytes, 64);
        assert!(bytes.len() <= descriptor_file_offset);
        bytes.resize(descriptor_file_offset + descriptor_bytes, 0);
        align_bytes(&mut bytes, 256);
        let entry_offset = bytes.len();
        bytes.resize(entry_offset + entry_bytes, 0xbf);
        let entry_address = entry_offset as u64 + 0x1000;
        let descriptor_address = descriptor_file_offset as u64;

        let entry_name = first_kernel.0.as_bytes();
        let descriptor_name = first_kernel.1.as_bytes();
        let mut strings = vec![0];
        let entry_name_index = strings.len() as u32;
        strings.extend_from_slice(entry_name);
        strings.push(0);
        let descriptor_name_index = strings.len() as u32;
        strings.extend_from_slice(descriptor_name);
        strings.push(0);
        let other_name_index = strings.len() as u32;
        strings.extend_from_slice(b"other\0");
        let second_name_indices = second_kernel.map(|(entry, descriptor, _)| {
            let entry_index = strings.len() as u32;
            strings.extend_from_slice(entry.as_bytes());
            strings.push(0);
            let descriptor_index = strings.len() as u32;
            strings.extend_from_slice(descriptor.as_bytes());
            strings.push(0);
            (entry_index, descriptor_index)
        });
        let string_table_offset = bytes.len();
        bytes.extend_from_slice(&strings);
        align_bytes(&mut bytes, 8);

        let symbol_table_offset = bytes.len();
        let symbol_count = 4 + usize::from(second_kernel.is_some()) * 2;
        bytes.resize(symbol_table_offset + symbol_count * 24, 0);
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

        if let Some((second_entry_name, second_descriptor_name)) = second_name_indices {
            let second_entry_symbol = symbol_table_offset + 96;
            write_test_u32(&mut bytes, second_entry_symbol, second_entry_name);
            bytes[second_entry_symbol + 4] = 0x12;
            bytes[second_entry_symbol + 5] = 3;
            write_test_u16(&mut bytes, second_entry_symbol + 6, 3);
            write_test_u64(
                &mut bytes,
                second_entry_symbol + 8,
                entry_address + entry_stride as u64,
            );
            write_test_u64(&mut bytes, second_entry_symbol + 16, 64);

            let second_descriptor_symbol = symbol_table_offset + 120;
            write_test_u32(&mut bytes, second_descriptor_symbol, second_descriptor_name);
            bytes[second_descriptor_symbol + 4] = 0x11;
            write_test_u16(&mut bytes, second_descriptor_symbol + 6, 2);
            write_test_u64(
                &mut bytes,
                second_descriptor_symbol + 8,
                descriptor_address + 64,
            );
            write_test_u64(&mut bytes, second_descriptor_symbol + 16, 64);
        }

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
            (descriptor_file_offset + descriptor_bytes) as u64,
        );
        write_test_u64(
            &mut bytes,
            first_program_header + 40,
            (descriptor_file_offset + descriptor_bytes) as u64,
        );
        write_test_u64(&mut bytes, first_program_header + 48, 0x1000);

        write_test_u32(&mut bytes, second_program_header, 1);
        write_test_u32(&mut bytes, second_program_header + 4, 5);
        write_test_u64(&mut bytes, second_program_header + 8, entry_offset as u64);
        write_test_u64(&mut bytes, second_program_header + 16, entry_address);
        write_test_u64(&mut bytes, second_program_header + 32, entry_bytes as u64);
        write_test_u64(&mut bytes, second_program_header + 40, entry_bytes as u64);
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
        write_test_u64(
            &mut bytes,
            rodata_header + 24,
            descriptor_file_offset as u64,
        );
        write_test_u64(&mut bytes, rodata_header + 32, descriptor_bytes as u64);
        write_test_u64(&mut bytes, rodata_header + 48, 64);

        let text_header = section_offset + 3 * SECTION_HEADER_BYTES;
        write_test_u32(&mut bytes, text_header, 15);
        write_test_u32(&mut bytes, text_header + 4, 1);
        write_test_u64(&mut bytes, text_header + 8, 6);
        write_test_u64(&mut bytes, text_header + 16, entry_address);
        write_test_u64(&mut bytes, text_header + 24, entry_offset as u64);
        write_test_u64(&mut bytes, text_header + 32, entry_bytes as u64);
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
        write_test_u64(&mut bytes, symbols_header + 32, (symbol_count * 24) as u64);
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

        let (rsrc1, rsrc2, rsrc3) = if target.starts_with("gfx94") {
            (1, 0x00af_0081, 0x1390)
        } else {
            (0x40, 0xe0af_0000, 0x1391)
        };
        for index in 0..kernel_count {
            let descriptor_offset = descriptor_file_offset + index * 64;
            let descriptor_address = descriptor_address + (index * 64) as u64;
            let entry_address = entry_address + (index * entry_stride) as u64;
            write_test_u32(&mut bytes, descriptor_offset, static_shared_memory_bytes);
            write_test_u32(
                &mut bytes,
                descriptor_offset + 4,
                private_segment_fixed_size,
            );
            let kernarg_segment_size = if index == 0 {
                first_kernel.2
            } else {
                second_kernel.unwrap().2
            };
            write_test_u32(&mut bytes, descriptor_offset + 8, kernarg_segment_size);
            write_test_i64(
                &mut bytes,
                descriptor_offset + 16,
                i64::try_from(entry_address - descriptor_address).unwrap(),
            );
            write_test_u32(&mut bytes, descriptor_offset + 44, rsrc1);
            write_test_u32(&mut bytes, descriptor_offset + 48, rsrc2);
            write_test_u32(&mut bytes, descriptor_offset + 52, rsrc3);
            write_test_u16(
                &mut bytes,
                descriptor_offset + 56,
                if target.starts_with("gfx94") {
                    0x001e
                } else {
                    0x041e
                },
            );
        }

        TestHsaco {
            bytes,
            descriptor_offset: descriptor_file_offset,
        }
    }

    fn test_elf_flags(target: &str) -> u32 {
        match target {
            "gfx1100" => 0x41,
            "gfx1151" => 0x4a,
            "gfx942" => 0x54c,
            "gfx942:xnack-" => 0x64c,
            "gfx950" => 0x54f,
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
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let publication_directory = TestPublicationDirectory::new();
        let current_lease = publish(&publication_directory, &bridge, selected.payload());
        let published = current_lease.published();
        let observed = make_observed(1);
        let durable_handoff = bridge.durable_plan_handoff();

        let admitted = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &bridge,
            current_lease,
            &fixture.container,
            selected,
            &observed,
        )
        .unwrap();

        assert_eq!(admitted.published(), published);
        assert_eq!(admitted.bridge, bridge);
        assert_eq!(admitted.durable_handoff, durable_handoff);
        assert!(!admitted.durable_handoff.grants_publication_authority());
        assert!(!admitted.durable_handoff.grants_load_authority());
        assert!(!admitted.durable_handoff.grants_launch_authority());
        assert_eq!(
            admitted.binding_index(),
            fixture.binding_index(&validated, 0)
        );
        assert_eq!(
            admitted.finalized_payload_identity(),
            fixture.expectations[0].finalized_payload_identity()
        );
        assert!(admitted.authenticates_filesystem_artifact());
        assert!(!admitted.proves_compiler_marker_binding());
        assert!(!admitted.establishes_executable_safety());
        assert!(!admitted.grants_load_authority());
        assert!(!admitted.grants_launch_authority());
        assert_eq!(
            admitted.revalidate(&validated, &bridge, &fixture.container, selected, &observed,),
            Ok(())
        );
    }

    #[test]
    fn evidence_and_publication_substitutions_are_rejected() {
        let fixture = make_fixture(2);
        let validated = fixture.validated();
        let bridge = make_bridge(&fixture, &validated, 0, 2, 2);
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let publication_directory = TestPublicationDirectory::new();
        let current_lease = publish(&publication_directory, &bridge, selected.payload());
        let observed = make_observed(2);
        let admitted = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &bridge,
            current_lease,
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
                &fixture.container,
                selected,
                &observed,
            ),
            Err(PublishedDirectLinkAdmissionError::EvidenceBridgeMismatch)
        );

        let other_bridge = make_bridge(&fixture, &validated, 0, 3, 4);
        let other_directory = TestPublicationDirectory::new();
        let other_lease = publish(&other_directory, &other_bridge, selected.payload());
        assert_eq!(
            ValidatedPublishedDirectLinkSelectionV1::validate(
                &validated,
                &bridge,
                other_lease,
                &fixture.container,
                selected,
                &observed,
            )
            .unwrap_err(),
            PublishedDirectLinkAdmissionError::CurrentLeaseSubstitution
        );
        assert_eq!(
            admitted.revalidate(
                &validated,
                &other_bridge,
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
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let publication_directory = TestPublicationDirectory::new();
        let current_lease = publish(&publication_directory, &bridge, selected.payload());
        let observed = make_observed(4);
        let admitted = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &bridge,
            current_lease,
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
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let publication_directory = TestPublicationDirectory::new();
        let current_lease = publish(&publication_directory, &bridge, selected.payload());
        let substituted = fixture
            .container
            .select_native_kernel(fixture.other_payload_kernel)
            .unwrap();

        assert_eq!(
            ValidatedPublishedDirectLinkSelectionV1::validate(
                &validated,
                &bridge,
                current_lease,
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
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let substituted = fixture
            .container
            .select_native_kernel(fixture.alias_kernel)
            .unwrap();
        let publication_directory = TestPublicationDirectory::new();
        let current_lease = publish(&publication_directory, &bridge, selected.payload());
        let observed = make_observed(7);
        let admitted = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &bridge,
            current_lease,
            &fixture.container,
            selected,
            &observed,
        )
        .unwrap();

        assert_eq!(
            admitted.revalidate(
                &validated,
                &bridge,
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
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let publication_directory = TestPublicationDirectory::new();
        let current_lease = publish(&publication_directory, &bridge, selected.payload());
        let observed = make_observed(8);
        let admitted = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &bridge,
            current_lease,
            &fixture.container,
            selected,
            &observed,
        )
        .unwrap();

        assert_eq!(
            admitted.revalidate(
                &validated,
                &bridge,
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

        let inspected =
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admission.admission).unwrap();

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
        assert!(inspected.authenticates_filesystem_artifact());
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
                &fixture.container,
                admission.selected,
                &admission.observed,
            ),
            Ok(())
        );
    }

    #[test]
    fn exact_inspection_pins_pending_load_identity_without_granting_authority() {
        let hsaco = test_hsaco("gfx1151", 0);
        let fixture = make_hsaco_fixture(52, hsaco.bytes, "gfx1151", "primary_kernel", false, 0);
        let prepared = prepare_hsaco_admission(&fixture, 52, "gfx1151");
        let expected_identity = prepared.admission.artifact_selection().identity().clone();
        let expected_container = prepared.admission.container_identity();
        let expected_payload = prepared.admission.finalized_payload_identity();
        let inspected =
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(prepared.admission).unwrap();
        let pending = inspected.into_pending_load_admission().unwrap();

        assert_eq!(pending.generation(), 52);
        assert_eq!(pending.published().attempt().generation(), 52);
        assert_eq!(pending.container_identity(), expected_container);
        assert_eq!(pending.finalized_payload_identity(), expected_payload);
        assert_eq!(pending.artifact_identity(), &expected_identity);
        assert_eq!(pending.target().to_string(), "gfx1151");
        assert_eq!(pending.code_object_version(), CodeObjectVersion::V6);
        assert_eq!(pending.kernel_symbol(), "primary_kernel");
        assert_eq!(pending.abi(), expected_identity.abi());
        assert_eq!(
            pending.missing_prerequisites(),
            &[
                MissingPublishedDirectLinkLoadPrerequisiteV1::AuthenticatedCompilerProducerChain,
                MissingPublishedDirectLinkLoadPrerequisiteV1::
                    AuthenticatedRustMarkerAbiAndEffectsBinding,
                MissingPublishedDirectLinkLoadPrerequisiteV1::
                    AuthenticatedExecutableLoadUnloadContract,
            ]
        );
        assert!(!pending.grants_load_authority());
        assert!(!pending.grants_launch_authority());

        let current = pending.acquire_currentness().unwrap();
        assert_eq!(current.admission().artifact_identity(), &expected_identity);
        assert!(!current.grants_load_authority());
        assert!(!current.grants_launch_authority());
    }

    #[test]
    fn pending_load_admission_rejects_a_stale_publication() {
        let hsaco = test_hsaco("gfx1151", 0);
        let fixture = make_hsaco_fixture(53, hsaco.bytes, "gfx1151", "primary_kernel", false, 0);
        let validated = fixture.validated();
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let observed = make_observed_for(53, "gfx1151");
        let directory = TestPublicationDirectory::new();
        let first_bridge = make_bridge(&fixture, &validated, 0, 1, 53);
        let first_lease = publish(&directory, &first_bridge, selected.payload());
        let admission = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &first_bridge,
            first_lease,
            &fixture.container,
            selected,
            &observed,
        )
        .unwrap();
        let pending = InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admission)
            .unwrap()
            .into_pending_load_admission()
            .unwrap();

        let second_bridge = make_bridge(&fixture, &validated, 0, 2, 53);
        let second_lease = publish(&directory, &second_bridge, selected.payload());
        assert!(second_lease.acquire_current_token().is_ok());
        assert!(matches!(
            pending.acquire_currentness(),
            Err(PublishedLoadAdmissionError::Inspection(
                PublishedPhysicalLayoutInspectionError::CurrentPublication { .. }
            ))
        ));
    }

    #[test]
    fn pending_load_admission_rejects_post_inspection_artifact_mutation() {
        let hsaco = test_hsaco("gfx1151", 0);
        let fixture = make_hsaco_fixture(
            54,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let prepared = prepare_hsaco_admission(&fixture, 54, "gfx1151");
        let directory = prepared._publication_directory.path.clone();
        let pending = InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(prepared.admission)
            .unwrap()
            .into_pending_load_admission()
            .unwrap();

        let mut substitute = hsaco.bytes;
        substitute[0] ^= 0xff;
        let artifact = fs::read_dir(directory)
            .unwrap()
            .map(Result::unwrap)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".fe2o3-link-artifact-v1-")
            })
            .unwrap()
            .path();
        fs::write(artifact, substitute).unwrap();

        assert!(matches!(
            pending.acquire_currentness(),
            Err(PublishedLoadAdmissionError::Inspection(
                PublishedPhysicalLayoutInspectionError::CurrentPublication { .. }
            ))
        ));
    }

    #[test]
    fn target_cov_symbol_and_physical_abi_mismatches_never_reach_pending_load_state() {
        let target_hsaco = test_hsaco("gfx1100", 0);
        let target_fixture = make_hsaco_fixture(
            55,
            target_hsaco.bytes,
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let target_admission = prepare_hsaco_admission(&target_fixture, 55, "gfx1151");
        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(target_admission.admission)
                .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::TargetMismatch
        );

        let mut cov_hsaco = test_hsaco("gfx1151", 0);
        cov_hsaco.bytes[8] = 7;
        let cov_fixture =
            make_hsaco_fixture(56, cov_hsaco.bytes, "gfx1151", "primary_kernel", false, 0);
        let cov_admission = prepare_hsaco_admission(&cov_fixture, 56, "gfx1151");
        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(cov_admission.admission)
                .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::Inspection(KernelBindingError::Inspection(
                InspectionError::UnsupportedCodeObjectVersion,
            ))
        );

        let symbol_hsaco = test_hsaco("gfx1151", 0);
        let symbol_fixture = make_hsaco_fixture(
            57,
            symbol_hsaco.bytes,
            "gfx1151",
            "substituted_symbol",
            false,
            0,
        );
        let symbol_admission = prepare_hsaco_admission(&symbol_fixture, 57, "gfx1151");
        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(symbol_admission.admission)
                .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::KernelSetMismatch
        );

        let abi_hsaco = physical_arguments_hsaco(None, [None; 3], false);
        let base_abi = physical_test_abi(false);
        let mismatched_abi =
            AbiLayout::new(40, 8, PointerWidth::Bits64, base_abi.fields().to_vec()).unwrap();
        let abi_fixture = make_single_hsaco_fixture(
            58,
            abi_hsaco.bytes,
            "gfx1151",
            mismatched_abi,
            physical_test_launch(0),
        );
        let abi_admission = prepare_hsaco_admission(&abi_fixture, 58, "gfx1151");
        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(abi_admission.admission)
                .unwrap_err(),
            PublishedPhysicalLayoutInspectionError::PhysicalLayoutMismatch {
                export_symbol: "primary_kernel".to_owned(),
                field: "explicit kernarg size",
            }
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
        let admission = prepare_hsaco_admission(&fixture, 40, "gfx1151");
        let inspected =
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admission.admission).unwrap();

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

    #[test]
    fn inspected_snapshot_survives_but_cannot_revalidate_after_newer_publication() {
        let hsaco = test_hsaco("gfx1151", 0);
        let fixture = make_hsaco_fixture(
            50,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let validated = fixture.validated();
        let selected = fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap();
        let observed = make_observed_for(50, "gfx1151");
        let directory = TestPublicationDirectory::new();
        let first_bridge = make_bridge(&fixture, &validated, 0, 1, 50);
        let first_lease = publish(&directory, &first_bridge, selected.payload());
        let admission = ValidatedPublishedDirectLinkSelectionV1::validate(
            &validated,
            &first_bridge,
            first_lease,
            &fixture.container,
            selected,
            &observed,
        )
        .unwrap();
        let inspected = InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admission).unwrap();

        let second_bridge = make_bridge(&fixture, &validated, 0, 2, 50);
        let second_lease = publish(&directory, &second_bridge, selected.payload());
        assert!(second_lease.acquire_current_token().is_ok());
        assert!(matches!(
            inspected.acquire_current_publication_token(),
            Err(PublishedPhysicalLayoutInspectionError::CurrentPublication { .. })
        ));
        assert_eq!(
            inspected.selected_kernel().export_symbol(),
            "primary_kernel"
        );
        assert!(!inspected.grants_load_authority());
    }

    #[test]
    fn token_aware_revalidation_child() {
        if std::env::var_os("FE2O3_TOKEN_AWARE_REVALIDATION_CHILD").is_none() {
            return;
        }
        let hsaco = test_hsaco("gfx1151", 0);
        let fixture = make_hsaco_fixture(51, hsaco.bytes, "gfx1151", "primary_kernel", false, 0);
        let prepared = prepare_hsaco_admission(&fixture, 51, "gfx1151");
        let current = prepared.admission.acquire_current_token().unwrap();
        assert_eq!(
            prepared.admission.revalidate(
                &prepared.validated,
                &prepared.bridge,
                &fixture.container,
                prepared.selected,
                &prepared.observed,
            ),
            Err(PublishedDirectLinkAdmissionError::Busy)
        );
        assert_eq!(
            prepared.admission.revalidate_with_current_token(
                &current,
                &prepared.validated,
                &prepared.bridge,
                &fixture.container,
                prepared.selected,
                &prepared.observed,
            ),
            Ok(())
        );
        let HsacoAdmission {
            _publication_directory,
            validated,
            bridge,
            selected,
            observed,
            admission,
        } = prepared;
        let inspected = InspectedPublishedDirectLinkPhysicalLayoutV1::inspect_with_current_token(
            admission, &current,
        )
        .unwrap();
        assert_eq!(
            inspected.revalidate_with_current_token(
                &current,
                &validated,
                &bridge,
                &fixture.container,
                selected,
                &observed,
            ),
            Ok(())
        );
        assert_eq!(
            inspected.revalidate(&validated, &bridge, &fixture.container, selected, &observed,),
            Err(PublishedPhysicalLayoutInspectionError::Busy)
        );
        drop(current);
        assert_eq!(
            inspected.revalidate(&validated, &bridge, &fixture.container, selected, &observed,),
            Ok(())
        );
    }

    #[test]
    fn token_aware_revalidation_never_self_deadlocks() {
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "published_direct_link::tests::token_aware_revalidation_child",
            ])
            .env("FE2O3_TOKEN_AWARE_REVALIDATION_CHILD", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        assert!(wait_for_child(child, Duration::from_secs(5)).success());
    }

    #[test]
    fn parser_fixture_pins_are_exact_and_canonical() {
        let fixture_bytes = include_bytes!("../tests/fixtures/gfx1151-typed-vecadd-v1.fe2o3");
        let container = ArtifactContainerV1::from_bytes(fixture_bytes).unwrap();
        assert_eq!(
            parse_sha256_pin("c74ee3f593b0bc302f67312e415a867f541fe3e3e79973ee596bc7bbf98a22d1"),
            Some(DigestAlgorithm::Sha256.calculate(fixture_bytes))
        );
        assert_eq!(
            parse_sha256_pin("053551d6a21604bec295acf1aedb4e3b2dedefa7f904a5fa160660b889b480fa"),
            Some(container.payloads()[0].digest())
        );
        assert_eq!(parse_sha256_pin("00"), None);
        assert_eq!(
            parse_sha256_pin("C74ee3f593b0bc302f67312e415a867f541fe3e3e79973ee596bc7bbf98a22d1"),
            None
        );

        assert!(canonical_source_commit(
            "30f6d75cf1c10c5dc18e2f9de6eb33015f6aab80"
        ));
        assert!(!canonical_source_commit(
            "0000000000000000000000000000000000000000"
        ));
        assert!(!canonical_source_commit(
            "30F6d75cf1c10c5dc18e2f9de6eb33015f6aab80"
        ));
        assert!(!canonical_source_commit("30f6d75"));
    }

    fn required_environment_pin(variable: &str) -> String {
        std::env::var(variable).unwrap_or_else(|_| panic!("set {variable}"))
    }

    fn pinned_sha256(variable: &str) -> PayloadDigest {
        let value = required_environment_pin(variable);
        parse_sha256_pin(&value)
            .unwrap_or_else(|| panic!("{variable} must be 64 lowercase hex digits"))
    }

    fn parse_sha256_pin(value: &str) -> Option<PayloadDigest> {
        if value.len() != 64 {
            return None;
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            let high = canonical_hex_nibble(pair[0])?;
            let low = canonical_hex_nibble(pair[1])?;
            bytes[index] = high << 4 | low;
        }
        Some(PayloadDigest::new(
            DigestAlgorithm::Sha256,
            DigestBytes::from_bytes(bytes),
        ))
    }

    fn canonical_hex_nibble(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            _ => None,
        }
    }

    fn pinned_source_commit(variable: &str) -> String {
        let value = required_environment_pin(variable);
        assert!(
            canonical_source_commit(&value),
            "{variable} must be a full, nonzero 40-digit lowercase Git commit ID"
        );
        value
    }

    fn canonical_source_commit(value: &str) -> bool {
        value.len() == 40
            && value
                .bytes()
                .all(|byte| canonical_hex_nibble(byte).is_some())
            && value.bytes().any(|byte| byte != b'0')
    }

    fn assert_manifest_commit_pin_where_available(container: &ArtifactContainerV1, expected: &str) {
        for version in [
            container.manifest().compiler().version().as_str(),
            container.manifest().producer().version().as_str(),
        ] {
            let declared = version
                .strip_prefix("git:")
                .or_else(|| version.rsplit_once("+git.").map(|(_, commit)| commit));
            if let Some(declared) = declared {
                assert_eq!(
                    declared.len(),
                    40,
                    "embedded Git commit must be full length"
                );
                assert!(
                    declared
                        .bytes()
                        .all(|byte| canonical_hex_nibble(byte).is_some()),
                    "embedded Git commit must be lowercase hexadecimal"
                );
                assert_eq!(
                    declared, expected,
                    "environment parser fixture source-commit pin mismatch"
                );
            }
        }
    }

    fn inspect_environment_parser_fixture(prefix: &str, target: &str) {
        let path_variable = format!("{prefix}_CONTAINER_V1");
        let container_digest_variable = format!("{prefix}_CONTAINER_SHA256");
        let payload_digest_variable = format!("{prefix}_PAYLOAD_SHA256");
        let source_commit_variable = format!("{prefix}_SOURCE_COMMIT");
        let path = required_environment_pin(&path_variable);
        let expected_container_digest = pinned_sha256(&container_digest_variable);
        let expected_payload_digest = pinned_sha256(&payload_digest_variable);
        let source_commit = pinned_source_commit(&source_commit_variable);
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(
            DigestAlgorithm::Sha256.calculate(&bytes),
            expected_container_digest,
            "environment parser fixture container digest mismatch"
        );
        let container = ArtifactContainerV1::from_bytes(&bytes).unwrap();
        assert_eq!(container.to_bytes(), bytes);
        assert_manifest_commit_pin_where_available(&container, &source_commit);
        assert_eq!(
            container.manifest().target().architecture().as_str(),
            target
        );
        let [payload] = container.payloads() else {
            panic!("environment parser fixture must contain exactly one payload");
        };
        assert_eq!(
            payload.digest(),
            expected_payload_digest,
            "environment parser fixture payload digest mismatch"
        );
        expected_payload_digest.verify(payload.bytes()).unwrap();
        let inspected = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(payload.bytes()).unwrap();
        assert_eq!(inspected.inspection().target().to_string(), target);
        assert_eq!(
            inspected.bindings().len(),
            inspected.inspection().kernels().len()
        );
    }

    #[test]
    #[ignore = "parser-only: requires all FE2O3_GFX942_PARSER_* pins"]
    fn parses_environment_pinned_gfx942_container_without_g7_or_g8_evidence() {
        inspect_environment_parser_fixture("FE2O3_GFX942_PARSER", "gfx942");
    }

    #[test]
    #[ignore = "parser-only: requires all FE2O3_GFX950_PARSER_* pins"]
    fn parses_environment_pinned_gfx950_container_without_g7_or_g8_evidence() {
        inspect_environment_parser_fixture("FE2O3_GFX950_PARSER", "gfx950");
    }

    #[test]
    fn in_place_payload_mutation_is_rejected_before_hsaco_parsing() {
        let hsaco = test_hsaco("gfx1151", 0);
        let fixture = make_hsaco_fixture(
            21,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let admitted = prepare_hsaco_admission(&fixture, 21, "gfx1151");
        let mut substitute = hsaco.bytes.clone();
        substitute[0] ^= 0xff;
        let artifact = fs::read_dir(&admitted._publication_directory.path)
            .unwrap()
            .map(Result::unwrap)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".fe2o3-link-artifact-v1-")
            })
            .unwrap()
            .path();
        fs::write(artifact, substitute).unwrap();

        assert!(matches!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted.admission),
            Err(PublishedPhysicalLayoutInspectionError::CurrentPublication { .. })
        ));
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
        let admitted = prepare_hsaco_admission(&fixture, 22, "gfx1151");

        assert!(matches!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted.admission),
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
        let admitted = prepare_hsaco_admission(&fixture, 23, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted.admission).unwrap_err(),
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
        let admitted = prepare_hsaco_admission(&fixture, 24, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted.admission).unwrap_err(),
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
        let admitted = prepare_hsaco_admission(&fixture, 25, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted.admission).unwrap_err(),
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
        let admitted = prepare_hsaco_admission(&fixture, 26, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted.admission).unwrap_err(),
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
        let admitted = prepare_hsaco_admission(&fixture, 27, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted.admission).unwrap_err(),
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
        let admitted = prepare_hsaco_admission(&fixture, 28, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted.admission).unwrap_err(),
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
        let admitted = prepare_hsaco_admission(&fixture, 44, "gfx1151");

        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admitted.admission).unwrap_err(),
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
        let first_admission = prepare_hsaco_admission(&first_fixture, 36, "gfx1151");
        let second_admission = prepare_hsaco_admission(&second_fixture, 37, "gfx1151");
        let first =
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(first_admission.admission)
                .unwrap();
        let second =
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(second_admission.admission)
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
        assert_eq!(launch.rank(), PhysicalMetadataValueV1::Unknown);
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
        assert_eq!(
            launch.cluster_dimensions(),
            PhysicalMetadataValueV1::Unknown
        );
        assert_eq!(launch.kernarg_segment_size(), 288);
        assert_eq!(launch.kernarg_segment_alignment(), 8);
        assert_eq!(
            launch.implicit_argument_offset(),
            PhysicalMetadataValueV1::Known(32)
        );
        assert_eq!(launch.implicit_argument_size(), 256);
        assert_eq!(launch.group_segment_fixed_size(), 0);
        assert_eq!(launch.private_segment_fixed_size(), 16);
        assert_eq!(launch.wavefront_size(), 32);
        assert_eq!(launch.scalar_register_count(), 14);
        assert_eq!(launch.vector_register_count(), 7);
        assert_eq!(
            launch.accumulator_register_count(),
            PhysicalMetadataValueV1::Known(3)
        );
        assert_eq!(
            launch.scalar_register_spill_count(),
            PhysicalMetadataValueV1::Known(2)
        );
        assert_eq!(
            launch.vector_register_spill_count(),
            PhysicalMetadataValueV1::Known(4)
        );
        assert_eq!(
            launch.workgroup_processor_mode(),
            PhysicalMetadataValueV1::Known(true)
        );
        assert_eq!(launch.gfx1250_revision(), PhysicalMetadataValueV1::Unknown);
        assert_eq!(
            launch.uniform_workgroup_size_indicator(),
            PhysicalMetadataValueV1::Unknown
        );
        assert_eq!(
            launch.dynamic_shared_memory_indicator(),
            PhysicalMetadataValueV1::Unknown
        );
        assert!(!first.proves_complete_launch_contract());
    }

    #[test]
    fn hidden_grid_dimensions_never_upgrade_manifest_rank_to_physical_evidence() {
        let hsaco = physical_arguments_hsaco(None, [None; 3], false);
        let fixture = make_single_hsaco_fixture(
            48,
            hsaco.bytes.clone(),
            "gfx1151",
            physical_test_abi(false),
            physical_test_launch_with_rank(3, 0),
        );
        let admission = prepare_hsaco_admission(&fixture, 48, "gfx1151");

        assert_eq!(admission.admission.payload_kernel_set().len(), 1);
        assert_eq!(
            admission.admission.payload_kernel_set()[0].launch().rank(),
            3
        );
        let inspected =
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admission.admission).unwrap();
        assert_eq!(
            inspected.selected_kernel().launch().rank(),
            PhysicalMetadataValueV1::Unknown
        );
    }

    #[test]
    fn optional_launch_resources_preserve_unknown_and_known_metadata() {
        let mut kernel = test_metadata_kernel_with(
            "primary_kernel",
            "primary_kernel.kd",
            Vec::new(),
            256,
            8,
            0,
            16,
            None,
            [None; 3],
            false,
        );
        let Value::Map(fields) = &mut kernel else {
            panic!("test kernel metadata must be a map");
        };
        fields.retain(|(key, _)| {
            !matches!(
                key.as_str(),
                Some(
                    ".agpr_count"
                        | ".sgpr_spill_count"
                        | ".vgpr_spill_count"
                        | ".workgroup_processor_mode"
                )
            )
        });
        fields.extend([
            (
                Value::from(".cluster_dims"),
                Value::Array(vec![Value::from(2), Value::from(1), Value::from(1)]),
            ),
            (Value::from(".uniform_work_group_size"), Value::from(1)),
        ]);
        let metadata = test_metadata("gfx1151", vec![kernel]);
        let hsaco = binding_hsaco(metadata, "gfx1151", 0, 16, 256);
        let fixture = make_hsaco_fixture(
            49,
            hsaco.bytes.clone(),
            "gfx1151",
            "primary_kernel",
            false,
            0,
        );
        let admission = prepare_hsaco_admission(&fixture, 49, "gfx1151");
        let inspected =
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admission.admission).unwrap();
        let launch = inspected.selected_kernel().launch();

        assert_eq!(
            launch.cluster_dimensions(),
            PhysicalMetadataValueV1::Known([2, 1, 1])
        );
        assert_eq!(
            launch.accumulator_register_count(),
            PhysicalMetadataValueV1::Unknown
        );
        assert_eq!(
            launch.scalar_register_spill_count(),
            PhysicalMetadataValueV1::Unknown
        );
        assert_eq!(
            launch.vector_register_spill_count(),
            PhysicalMetadataValueV1::Unknown
        );
        assert_eq!(
            launch.workgroup_processor_mode(),
            PhysicalMetadataValueV1::Unknown
        );
        assert_eq!(
            launch.uniform_workgroup_size_indicator(),
            PhysicalMetadataValueV1::Known(true)
        );
        assert_eq!(launch.rank(), PhysicalMetadataValueV1::Unknown);
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
            let admission = prepare_hsaco_admission(&fixture, 38, "gfx1151");
            assert_eq!(
                InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admission.admission)
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
        let admission = prepare_hsaco_admission(&fixture, 39, "gfx1151");
        let inspected =
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admission.admission).unwrap();
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
        let padded_admission = prepare_hsaco_admission(&padded_fixture, 45, "gfx1151");
        assert_eq!(
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(padded_admission.admission)
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
            let alignment_admission =
                prepare_hsaco_admission(&alignment_fixture, usize::from(seed), "gfx1151");
            assert_eq!(
                InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(
                    alignment_admission.admission,
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
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(first.admission).unwrap();

        assert_eq!(
            inspected.revalidate(
                &second.validated,
                &second.bridge,
                &second_fixture.container,
                second.selected,
                &second.observed,
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
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(first.admission).unwrap();

        assert_eq!(
            inspected.revalidate(
                &second.validated,
                &second.bridge,
                &fixture.container,
                second.selected,
                &second.observed,
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
        let inspected =
            InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admission.admission).unwrap();

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
                    &fixture.container,
                    admission.selected,
                    &observed,
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
