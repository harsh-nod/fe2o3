use std::{fmt, path::Path};

use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BuildAttempt, CanonicalLinkRequestIdentityV1,
    DurableCurrentLinkPublicationLeaseV1, DurableCurrentLinkPublicationTokenV1,
    DurableLinkPublicationError, DurableLinkPublicationOutcomeV1, DurableLinkPublicationPlanV1,
    DurableLinkPublicationSnapshotV1, DurableLinkPublicationTransactionV1, FinalizationIdentityV1,
    FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    PackageIdentityV1, PinnedWorkerIdentityV1, PublishedLinkArtifactV1, TargetIdentityV1,
    ValidatedResponseIdentityV1, publish_durable_link_v1, recover_durable_link_publication_v1,
};
use sha2::{Digest as _, Sha256};

use crate::{
    AbiKind, AliasClass, ArgumentOwnership, ArtifactContainerV1, BlockSize, CONTAINER_MAGIC,
    CONTAINER_VERSION, CodeObjectFormat, DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM, DigestAlgorithm,
    DigestBytes, Dimensions, DirectLinkBindingV1, DirectLinkBundleEvidenceV1,
    DirectLinkBundleIndexIdentityV1, DirectLinkContainerIdentityV1,
    DirectLinkFinalizedPayloadIdentityV1, DirectLinkToolchainIdentityV1,
    DirectLinkWorkerIdentityV1, KernelEntry, MAX_MANIFEST_BYTES, PayloadDigest,
    ValidatedDirectLinkBundleEvidenceV1,
};

const WORKER_CLOSURE_DOMAIN: &[u8] = b"fe2o3.direct-link.worker-closure.v1\0";
const PUBLICATION_DOMAIN: &[u8] = b"fe2o3.direct-link.publication-bridge.v1\0";
const MANIFEST_CLAIM_SCOPE_PUBLICATION_DOMAIN: &[u8] =
    b"fe2o3.direct-link.publication-bridge.manifest-claim-scope.v1\0";
const MANIFEST_CLAIM_TARGET_DOMAIN: &[u8] =
    b"fe2o3.direct-link.publication-scope.manifest-claim-target.v1\0";
const MANIFEST_CLAIM_KERNEL_SET_DOMAIN: &[u8] =
    b"fe2o3.direct-link.publication-scope.manifest-claim-logical-kernel-set.v1\0";
const PUBLICATION_OCCURRENCE_DOMAIN: &[u8] = b"fe2o3.direct-link.publication-occurrence.v1\0";

/// Identity domain involved in a rejected G5/G6 bridge operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectLinkBridgeIdentityKindV1 {
    Attempt,
    Scope,
    Request,
    WorkerClosure,
    Response,
    LinkedOutput,
    Finalization,
    FinalizedOutput,
    Publication,
}

/// Provenance of the G5 publication scope committed by a bridge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectLinkPublicationScopeProvenanceV1 {
    /// Unsafe compatibility path: all three identities are unauthenticated external claims.
    UnsafeLegacyExternalClaims,
    /// Package is a caller claim; target and logical kernel set are manifest claims.
    ManifestClaimDerivedV1,
}

/// Field that did not match an opaque manifest-claim-derived scope witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectLinkManifestClaimScopeFieldV1 {
    BindingIndex,
    Binding,
    BundleEvidence,
    ContainerIdentity,
    FinalizedPayloadOccurrence,
}

/// Failure to construct or validate a typed G5/G6 publication bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DirectLinkBridgeError {
    BindingIndexOutOfRange {
        index: usize,
        binding_count: usize,
    },
    UnsupportedDigestAlgorithm {
        field: &'static str,
    },
    IdentityMismatch {
        kind: DirectLinkBridgeIdentityKindV1,
    },
    ManifestClaimScopeMismatch {
        field: DirectLinkManifestClaimScopeFieldV1,
    },
    CanonicalManifestEncodingTooLarge {
        actual: usize,
        max: usize,
    },
}

impl fmt::Display for DirectLinkBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindingIndexOutOfRange {
                index,
                binding_count,
            } => write!(
                formatter,
                "direct-link binding index {index} is outside validated count {binding_count}"
            ),
            Self::UnsupportedDigestAlgorithm { field } => {
                write!(
                    formatter,
                    "{field} does not use the G5 SHA-256 identity domain"
                )
            }
            Self::IdentityMismatch { kind } => {
                write!(formatter, "G5/G6 {kind:?} identity mismatch")
            }
            Self::ManifestClaimScopeMismatch { field } => {
                write!(
                    formatter,
                    "manifest-claim-derived G5 scope {field:?} mismatch"
                )
            }
            Self::CanonicalManifestEncodingTooLarge { actual, max } => {
                write!(
                    formatter,
                    "canonical manifest encoding has {actual} bytes, exceeding {max}"
                )
            }
        }
    }
}

impl std::error::Error for DirectLinkBridgeError {}

/// Explicitly unauthenticated package-identity claim supplied by a caller.
///
/// This wrapper does not establish package ownership, namespace control, a
/// lease, or current publication authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallerClaimedPackageIdentityV1(PackageIdentityV1);

impl CallerClaimedPackageIdentityV1 {
    pub const fn new(claim: PackageIdentityV1) -> Self {
        Self(claim)
    }

    /// Returns the descriptive package claim for the inert G5 model.
    pub const fn descriptive_claim(self) -> PackageIdentityV1 {
        self.0
    }

    pub const fn grants_package_ownership_authority(self) -> bool {
        false
    }
}

/// Canonical identity of one container/finalized-payload occurrence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DirectLinkPublicationOccurrenceIdentityV1([u8; 32]);

impl DirectLinkPublicationOccurrenceIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Opaque target identity derived from every canonical manifest target field.
///
/// Construction is restricted to derive_manifest_claim_target_identity_v1.
/// The value records manifest provenance but does not authenticate the
/// container, target, compiler, publication, or current generation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ManifestClaimDerivedTargetIdentityV1(TargetIdentityV1);

impl ManifestClaimDerivedTargetIdentityV1 {
    /// Returns the inert G5 target claim used by publication records.
    pub const fn descriptive_identity(self) -> TargetIdentityV1 {
        self.0
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Derives the publication bridge's canonical target claim from one container.
///
/// This is the single implementation of the manifest-claim-target.v1 domain
/// used by direct-link publication. The returned witness is descriptive and
/// inert.
pub fn derive_manifest_claim_target_identity_v1(
    container: &ArtifactContainerV1,
) -> ManifestClaimDerivedTargetIdentityV1 {
    ManifestClaimDerivedTargetIdentityV1(TargetIdentityV1::from_bytes(
        derive_manifest_claim_target(container),
    ))
}

/// Opaque V1 witness for one manifest-claim-derived G5 publication scope.
///
/// Construction binds an explicit caller package claim to one exact G6 binding
/// occurrence and concrete container. The target identity covers every
/// canonical manifest target field. The logical kernel-set identity covers the
/// complete sorted set of manifest kernel claims referencing that occurrence:
/// stable kernel ID, logical name, symbol, source-identity claim, required
/// capabilities, launch contract, ABI, and the binding's FFI-closure claim. It
/// deliberately excludes payload, executable, code-object, compiler, producer,
/// toolchain, container, and other build-content identities.
///
/// These remain claims, not authenticated compiler or native-code facts. This
/// witness grants no package ownership, durable publication, load, or launch
/// authority. A future authoritative path must additionally require the G5
/// package lease/current-publication witness and G7 HSACO inspection witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestClaimDerivedLinkPublicationScopeV1 {
    package_claim: CallerClaimedPackageIdentityV1,
    binding_index: usize,
    bundle_index_identity: DirectLinkBundleIndexIdentityV1,
    evidence_identity: PayloadDigest,
    binding: DirectLinkBindingV1,
    scope: LinkPublicationScopeV1,
}

impl ManifestClaimDerivedLinkPublicationScopeV1 {
    /// Derives descriptive scope claims from one binding occurrence and container.
    ///
    /// Exact container identity is recomputed with bounded streaming. Only the
    /// canonical manifest encoding is allocated, and it is rejected above
    /// `MAX_MANIFEST_BYTES`; payload bytes are borrowed and streamed once.
    pub fn derive(
        package_claim: CallerClaimedPackageIdentityV1,
        validated: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        binding_index: usize,
        container: &ArtifactContainerV1,
    ) -> Result<Self, DirectLinkBridgeError> {
        let binding = validated.bindings().get(binding_index).ok_or(
            DirectLinkBridgeError::BindingIndexOutOfRange {
                index: binding_index,
                binding_count: validated.bindings().len(),
            },
        )?;
        let measured_container_identity = stream_container_identity(container)?;
        require_manifest_claim(
            measured_container_identity == binding.container_identity(),
            DirectLinkManifestClaimScopeFieldV1::ContainerIdentity,
        )?;

        let finalized = binding.expectation().finalized_payload_identity();
        container
            .manifest()
            .code_objects()
            .iter()
            .find(|object| object.digest() == finalized.digest().bytes())
            .filter(|object| object.format() == CodeObjectFormat::NativeExecutable)
            .ok_or(DirectLinkBridgeError::ManifestClaimScopeMismatch {
                field: DirectLinkManifestClaimScopeFieldV1::FinalizedPayloadOccurrence,
            })?;
        let payload = container
            .payloads()
            .iter()
            .find(|payload| payload.digest() == finalized.digest())
            .ok_or(DirectLinkBridgeError::ManifestClaimScopeMismatch {
                field: DirectLinkManifestClaimScopeFieldV1::FinalizedPayloadOccurrence,
            })?;
        payload.digest().verify(payload.bytes()).map_err(|_| {
            DirectLinkBridgeError::ManifestClaimScopeMismatch {
                field: DirectLinkManifestClaimScopeFieldV1::FinalizedPayloadOccurrence,
            }
        })?;

        let kernels = container
            .manifest()
            .kernels()
            .iter()
            .filter(|kernel| kernel.code_object_digest() == finalized.digest().bytes())
            .collect::<Vec<_>>();
        if kernels.is_empty() {
            return Err(DirectLinkBridgeError::ManifestClaimScopeMismatch {
                field: DirectLinkManifestClaimScopeFieldV1::FinalizedPayloadOccurrence,
            });
        }

        let target = derive_manifest_claim_target_identity_v1(container).descriptive_identity();
        let kernel_set = KernelSetIdentityV1::from_bytes(derive_logical_kernel_set_claim(
            binding.expectation().ffi_contract_identity().digest(),
            &kernels,
        ));
        Ok(Self {
            package_claim,
            binding_index,
            bundle_index_identity: validated.evidence().bundle_index_identity(),
            evidence_identity: validated
                .evidence()
                .digest(DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM),
            binding: binding.clone(),
            scope: LinkPublicationScopeV1::new(
                package_claim.descriptive_claim(),
                kernel_set,
                target,
            ),
        })
    }

    /// Returns the caller's descriptive package claim without adding authority.
    pub const fn caller_package_claim(&self) -> CallerClaimedPackageIdentityV1 {
        self.package_claim
    }

    /// Returns raw scope claims required by the inert G5 model.
    ///
    /// This getter erases no stored provenance from the witness itself, but the
    /// returned G5 value carries no provenance and must never authorize use.
    pub const fn descriptive_scope_claim(&self) -> LinkPublicationScopeV1 {
        self.scope
    }

    pub const fn binding_index(&self) -> usize {
        self.binding_index
    }

    pub const fn container_identity(&self) -> DirectLinkContainerIdentityV1 {
        self.binding.container_identity()
    }

    pub const fn finalized_payload_identity(&self) -> DirectLinkFinalizedPayloadIdentityV1 {
        self.binding.expectation().finalized_payload_identity()
    }

    pub fn occurrence_identity(&self) -> DirectLinkPublicationOccurrenceIdentityV1 {
        publication_occurrence_identity(&self.binding)
    }

    /// Manifest-claim scope evidence never grants module-loading authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Manifest-claim scope evidence never grants kernel-launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    fn require_matches(
        &self,
        validated: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        binding_index: usize,
        binding: &DirectLinkBindingV1,
    ) -> Result<(), DirectLinkBridgeError> {
        require_manifest_claim(
            self.binding_index == binding_index,
            DirectLinkManifestClaimScopeFieldV1::BindingIndex,
        )?;
        require_manifest_claim(
            self.binding == *binding,
            DirectLinkManifestClaimScopeFieldV1::Binding,
        )?;
        let evidence = validated.evidence();
        require_manifest_claim(
            self.bundle_index_identity == evidence.bundle_index_identity()
                && self.evidence_identity == evidence.digest(DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM),
            DirectLinkManifestClaimScopeFieldV1::BundleEvidence,
        )
    }
}

/// Inert, manifest-claim-only API boundary for the durable G5 adapter.
///
/// This type can be constructed only by
/// [`ManifestClaimDirectLinkPublicationBridgeV1`]. Its raw G5 scope is private:
/// the durable adapter consumes this opaque value through an API owned by this
/// crate instead of reconstructing authority from provenance-erasing getters.
/// It is not a package lease, current-publication witness, HSACO inspection
/// witness, or runtime authority.
///
/// A legacy bridge cannot obtain this handoff at compile time:
///
/// ```compile_fail
/// use fe2o3_artifacts::{
///     DirectLinkPublicationBridgeV1, ManifestClaimDirectLinkDurablePlanHandoffV1,
/// };
///
/// fn cannot_escalate_legacy_claims(
///     legacy: &DirectLinkPublicationBridgeV1,
/// ) -> ManifestClaimDirectLinkDurablePlanHandoffV1 {
///     legacy.durable_plan_handoff()
/// }
/// ```
///
/// The handoff also cannot erase its scope provenance through a raw getter:
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::LinkPublicationScopeV1;
/// use fe2o3_artifacts::ManifestClaimDirectLinkDurablePlanHandoffV1;
///
/// fn cannot_extract_raw_scope(
///     handoff: &ManifestClaimDirectLinkDurablePlanHandoffV1,
/// ) -> LinkPublicationScopeV1 {
///     handoff.descriptive_scope_claim()
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestClaimDirectLinkDurablePlanHandoffV1 {
    attempt: BuildAttempt,
    _scope_claim: LinkPublicationScopeV1,
    request: CanonicalLinkRequestIdentityV1,
    worker: PinnedWorkerIdentityV1,
    response: ValidatedResponseIdentityV1,
    linked_output: LinkedOutputIdentityV1,
    finalization: FinalizationIdentityV1,
    finalized_output: FinalizedOutputIdentityV1,
    publication: AtomicPublicationIdentityV1,
    occurrence: DirectLinkPublicationOccurrenceIdentityV1,
    container_identity: DirectLinkContainerIdentityV1,
    finalized_payload_identity: DirectLinkFinalizedPayloadIdentityV1,
    bundle_index_identity: DirectLinkBundleIndexIdentityV1,
    evidence_identity: PayloadDigest,
}

impl ManifestClaimDirectLinkDurablePlanHandoffV1 {
    fn durable_publication_plan(&self) -> DurableLinkPublicationPlanV1 {
        DurableLinkPublicationPlanV1::new(
            self.attempt,
            self._scope_claim,
            self.request,
            self.worker,
            self.response,
            self.linked_output,
            self.finalization,
            self.finalized_output,
            self.publication,
        )
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub const fn request_identity(&self) -> CanonicalLinkRequestIdentityV1 {
        self.request
    }

    pub const fn worker_identity(&self) -> PinnedWorkerIdentityV1 {
        self.worker
    }

    pub const fn response_identity(&self) -> ValidatedResponseIdentityV1 {
        self.response
    }

    pub const fn linked_output_identity(&self) -> LinkedOutputIdentityV1 {
        self.linked_output
    }

    pub const fn finalization_identity(&self) -> FinalizationIdentityV1 {
        self.finalization
    }

    pub const fn finalized_output_identity(&self) -> FinalizedOutputIdentityV1 {
        self.finalized_output
    }

    pub const fn publication_identity(&self) -> AtomicPublicationIdentityV1 {
        self.publication
    }

    pub const fn occurrence_identity(&self) -> DirectLinkPublicationOccurrenceIdentityV1 {
        self.occurrence
    }

    pub const fn container_identity(&self) -> DirectLinkContainerIdentityV1 {
        self.container_identity
    }

    pub const fn finalized_payload_identity(&self) -> DirectLinkFinalizedPayloadIdentityV1 {
        self.finalized_payload_identity
    }

    pub const fn bundle_index_identity(&self) -> DirectLinkBundleIndexIdentityV1 {
        self.bundle_index_identity
    }

    pub const fn evidence_identity(&self) -> PayloadDigest {
        self.evidence_identity
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Manifest-claim-derived wrapper around one exact durable current-publication lease.
///
/// The wrapper is deliberately non-clone and exposes neither its generic G5 lease nor raw scope.
/// It retains the complete opaque handoff used to publish the artifact, preventing a legacy bridge
/// or diagnostics-only scope from entering the G7 current-publication path.
///
/// ```compile_fail
/// use fe2o3_artifacts::ManifestClaimDirectLinkCurrentPublicationLeaseV1;
///
/// fn cannot_clone(
///     lease: ManifestClaimDirectLinkCurrentPublicationLeaseV1,
/// ) -> (
///     ManifestClaimDirectLinkCurrentPublicationLeaseV1,
///     ManifestClaimDirectLinkCurrentPublicationLeaseV1,
/// ) {
///     (lease.clone(), lease)
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::DurableCurrentLinkPublicationLeaseV1;
/// use fe2o3_artifacts::ManifestClaimDirectLinkCurrentPublicationLeaseV1;
///
/// fn cannot_extract_generic(
///     lease: ManifestClaimDirectLinkCurrentPublicationLeaseV1,
/// ) -> DurableCurrentLinkPublicationLeaseV1 {
///     lease.lease
/// }
/// ```
pub struct ManifestClaimDirectLinkCurrentPublicationLeaseV1 {
    handoff: ManifestClaimDirectLinkDurablePlanHandoffV1,
    lease: DurableCurrentLinkPublicationLeaseV1,
}

impl fmt::Debug for ManifestClaimDirectLinkCurrentPublicationLeaseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManifestClaimDirectLinkCurrentPublicationLeaseV1")
            .field("handoff", &self.handoff)
            .field("lease", &self.lease)
            .finish_non_exhaustive()
    }
}

impl ManifestClaimDirectLinkCurrentPublicationLeaseV1 {
    pub fn published(&self) -> PublishedLinkArtifactV1 {
        self.lease.published()
    }

    /// Returns whether this lease retains the exact opaque handoff supplied by the bridge.
    pub fn is_bound_to_handoff(
        &self,
        handoff: &ManifestClaimDirectLinkDurablePlanHandoffV1,
    ) -> bool {
        &self.handoff == handoff
    }

    /// Borrows descriptor-derived bytes without reopening a pathname.
    pub fn exact_artifact_bytes(&self) -> &[u8] {
        self.lease.exact_artifact_bytes()
    }

    /// Revalidates currentness and retains the cooperative lock in the returned inert token.
    pub fn acquire_current_token(
        &self,
    ) -> Result<ManifestClaimDirectLinkCurrentPublicationTokenV1, DurableLinkPublicationError> {
        Ok(ManifestClaimDirectLinkCurrentPublicationTokenV1 {
            token: self.lease.acquire_current_token()?,
        })
    }

    /// Validates an already-held token without acquiring the cooperative lock again.
    pub fn validate_current_token(
        &self,
        token: &ManifestClaimDirectLinkCurrentPublicationTokenV1,
    ) -> Result<(), DurableLinkPublicationError> {
        self.lease.validate_current_token(&token.token)
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Locked currentness token for one manifest-claim-derived exact-file lease.
pub struct ManifestClaimDirectLinkCurrentPublicationTokenV1 {
    token: DurableCurrentLinkPublicationTokenV1,
}

impl fmt::Debug for ManifestClaimDirectLinkCurrentPublicationTokenV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManifestClaimDirectLinkCurrentPublicationTokenV1")
            .field("token", &self.token)
            .finish_non_exhaustive()
    }
}

impl ManifestClaimDirectLinkCurrentPublicationTokenV1 {
    pub fn exact_artifact_bytes(&self) -> &[u8] {
        self.token.exact_artifact_bytes()
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Result of provenance-preserving durable publication and current-lease issuance.
pub struct ManifestClaimDirectLinkDurablePublicationResultV1 {
    outcome: DurableLinkPublicationOutcomeV1,
    lease: ManifestClaimDirectLinkCurrentPublicationLeaseV1,
}

impl fmt::Debug for ManifestClaimDirectLinkDurablePublicationResultV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManifestClaimDirectLinkDurablePublicationResultV1")
            .field("outcome", &self.outcome)
            .field("lease", &self.lease)
            .finish()
    }
}

impl ManifestClaimDirectLinkDurablePublicationResultV1 {
    pub const fn outcome(&self) -> DurableLinkPublicationOutcomeV1 {
        self.outcome
    }

    pub fn snapshot(&self) -> &DurableLinkPublicationSnapshotV1 {
        self.lease.lease.snapshot()
    }

    pub fn into_current_lease(self) -> ManifestClaimDirectLinkCurrentPublicationLeaseV1 {
        self.lease
    }
}

/// Publishes through the provenance-preserving G5/G6 durable adapter.
///
/// The opaque handoff is the only accepted bridge input. Its complete G5 plan
/// remains private, so neither a legacy bridge nor a provenance-erasing
/// diagnostic scope can be substituted at this boundary. The handoff and
/// returned snapshot remain inert and grant no package, load, or launch
/// authority.
///
/// A legacy bridge cannot enter this adapter:
///
/// ```compile_fail
/// use std::path::Path;
/// use fe2o3_artifacts::{
///     DirectLinkPublicationBridgeV1, publish_manifest_claim_direct_link_durable_v1,
/// };
///
/// fn cannot_publish_legacy(output: &Path, legacy: &DirectLinkPublicationBridgeV1) {
///     let _ = publish_manifest_claim_direct_link_durable_v1(output, legacy, |_| Ok(()));
/// }
/// ```
///
/// A diagnostic raw scope cannot enter this adapter either:
///
/// ```compile_fail
/// use std::path::Path;
/// use fe2o3_artifacts::{
///     NonAuthoritativeDirectLinkPublicationDiagnosticsV1,
///     publish_manifest_claim_direct_link_durable_v1,
/// };
///
/// fn cannot_publish_diagnostic_scope(
///     output: &Path,
///     diagnostics: &NonAuthoritativeDirectLinkPublicationDiagnosticsV1,
/// ) {
///     let scope = diagnostics.descriptive_scope_claim();
///     let _ = publish_manifest_claim_direct_link_durable_v1(output, &scope, |_| Ok(()));
/// }
/// ```
pub fn publish_manifest_claim_direct_link_durable_v1<F>(
    output_dir: &Path,
    handoff: &ManifestClaimDirectLinkDurablePlanHandoffV1,
    work: F,
) -> Result<ManifestClaimDirectLinkDurablePublicationResultV1, DurableLinkPublicationError>
where
    F: FnOnce(
        &mut DurableLinkPublicationTransactionV1<'_>,
    ) -> Result<(), DurableLinkPublicationError>,
{
    let result = publish_durable_link_v1(output_dir, handoff.durable_publication_plan(), work)?;
    let outcome = result.outcome();
    let lease = result.into_current_lease();
    if !published_matches_handoff(lease.published(), handoff) {
        return Err(DurableLinkPublicationError::CurrentPublication {
            reason: "durable lease differs from its manifest-claim handoff".to_string(),
        });
    }
    Ok(ManifestClaimDirectLinkDurablePublicationResultV1 {
        outcome,
        lease: ManifestClaimDirectLinkCurrentPublicationLeaseV1 {
            handoff: handoff.clone(),
            lease,
        },
    })
}

fn published_matches_handoff(
    published: PublishedLinkArtifactV1,
    handoff: &ManifestClaimDirectLinkDurablePlanHandoffV1,
) -> bool {
    published.attempt() == handoff.attempt
        && published.scope() == handoff._scope_claim
        && published.request() == handoff.request
        && published.worker() == handoff.worker
        && published.response() == handoff.response
        && published.linked_output() == handoff.linked_output
        && published.finalization() == handoff.finalization
        && published.finalized_output() == handoff.finalized_output
        && published.publication() == handoff.publication
}

/// Recovers the current inert publication for an opaque manifest-claim scope.
///
/// Recovery is scope-based and may return a newer publication than the handoff's
/// attempt. It provides immutable evidence only and grants no use authority.
pub fn recover_manifest_claim_direct_link_durable_v1(
    output_dir: &Path,
    handoff: &ManifestClaimDirectLinkDurablePlanHandoffV1,
) -> Result<Option<DurableLinkPublicationSnapshotV1>, DurableLinkPublicationError> {
    recover_durable_link_publication_v1(output_dir, handoff._scope_claim)
}

/// Legacy, explicitly non-authoritative conversion into the inert G5 model.
///
/// This compatibility type accepts raw external scope claims and deliberately
/// has no durable-handoff API. Its historical constructor and scope getters
/// remain available for descriptive model integration, but a value of this type
/// cannot enter the manifest-claim-derived G5 handoff path. Use
/// [`ManifestClaimDirectLinkPublicationBridgeV1`] for that structurally distinct
/// path.
///
/// This model performs no filesystem I/O, authenticates no caller-supplied
/// measurement, and grants no publication, load, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectLinkPublicationBridgeV1 {
    attempt: BuildAttempt,
    scope: LinkPublicationScopeV1,
    scope_provenance: DirectLinkPublicationScopeProvenanceV1,
    bundle: DirectLinkBundleEvidenceV1,
    binding: DirectLinkBindingV1,
}

impl DirectLinkPublicationBridgeV1 {
    /// Prepares a bridge for one binding in a concretely validated envelope.
    ///
    /// This is an unsafe, inert compatibility constructor. Its historical name
    /// is retained for API compatibility only; `trusted_scope` is not trusted.
    /// All three identities are unauthenticated caller claims.
    /// This model does not derive or verify its package, kernel-set, or target
    /// identities from artifact records. The value is committed into the
    /// publication identity without being elevated beyond a claim.
    pub fn prepare_with_trusted_scope(
        attempt: BuildAttempt,
        trusted_scope: LinkPublicationScopeV1,
        validated: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        binding_index: usize,
    ) -> Result<Self, DirectLinkBridgeError> {
        let binding = validated.bindings().get(binding_index).ok_or(
            DirectLinkBridgeError::BindingIndexOutOfRange {
                index: binding_index,
                binding_count: validated.bindings().len(),
            },
        )?;
        let bridge = Self {
            attempt,
            scope: trusted_scope,
            scope_provenance: DirectLinkPublicationScopeProvenanceV1::UnsafeLegacyExternalClaims,
            bundle: validated.evidence().clone(),
            binding: binding.clone(),
        };
        bridge.require_sha256_domains()?;
        Ok(bridge)
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    /// Compatibility accessor for raw descriptive publication-scope claims.
    ///
    /// The historical name is misleading and retained only for G5 model
    /// compatibility. The returned value carries no provenance or authority.
    pub const fn trusted_scope(&self) -> LinkPublicationScopeV1 {
        self.scope
    }

    /// Returns raw descriptive scope claims committed by this bridge.
    ///
    /// This raw G5 model value carries no provenance. Future authoritative code
    /// must retain this opaque bridge or its provenance-preserving durable
    /// handoff and additionally require the separate G5 and G7 witnesses.
    pub const fn publication_scope(&self) -> LinkPublicationScopeV1 {
        self.scope
    }

    /// Returns whether scope fields are legacy external or manifest-derived claims.
    pub const fn scope_provenance(&self) -> DirectLinkPublicationScopeProvenanceV1 {
        self.scope_provenance
    }

    /// Returns the descriptive G6 binding claim; it carries no authority alone.
    pub const fn binding(&self) -> &DirectLinkBindingV1 {
        &self.binding
    }

    /// Returns the descriptive G6 evidence envelope; it carries no authority alone.
    pub const fn bundle(&self) -> &DirectLinkBundleEvidenceV1 {
        &self.bundle
    }

    pub fn request_identity(&self) -> CanonicalLinkRequestIdentityV1 {
        CanonicalLinkRequestIdentityV1::from_bytes(digest_bytes(
            self.binding.expectation().request_identity().digest(),
        ))
    }

    pub fn worker_identity(&self) -> PinnedWorkerIdentityV1 {
        PinnedWorkerIdentityV1::from_bytes(calculate_identity(WORKER_CLOSURE_DOMAIN, |bytes| {
            write_worker(bytes, self.binding.expectation().worker());
            write_toolchain(bytes, self.binding.expectation().toolchain());
        }))
    }

    pub fn response_identity(&self) -> ValidatedResponseIdentityV1 {
        ValidatedResponseIdentityV1::from_bytes(digest_bytes(
            self.binding.expectation().response_identity().digest(),
        ))
    }

    pub fn linked_output_identity(&self) -> LinkedOutputIdentityV1 {
        LinkedOutputIdentityV1::from_bytes(digest_bytes(
            self.binding.expectation().linked_output_identity().digest(),
        ))
    }

    pub fn finalization_identity(&self) -> FinalizationIdentityV1 {
        FinalizationIdentityV1::from_bytes(digest_bytes(
            self.binding.expectation().finalization_identity().digest(),
        ))
    }

    pub fn finalized_output_identity(&self) -> FinalizedOutputIdentityV1 {
        FinalizedOutputIdentityV1::from_bytes(digest_bytes(
            self.binding
                .expectation()
                .finalized_payload_identity()
                .digest(),
        ))
    }

    /// Returns the exact container/finalized-payload occurrence identity.
    pub fn occurrence_identity(&self) -> DirectLinkPublicationOccurrenceIdentityV1 {
        publication_occurrence_identity(&self.binding)
    }

    /// Derives the atomic G5 publication identity over the complete G5/G6 closure.
    pub fn publication_identity(&self) -> AtomicPublicationIdentityV1 {
        let domain = match self.scope_provenance {
            DirectLinkPublicationScopeProvenanceV1::UnsafeLegacyExternalClaims => {
                PUBLICATION_DOMAIN
            }
            DirectLinkPublicationScopeProvenanceV1::ManifestClaimDerivedV1 => {
                MANIFEST_CLAIM_SCOPE_PUBLICATION_DOMAIN
            }
        };
        AtomicPublicationIdentityV1::from_bytes(calculate_identity(domain, |bytes| {
            bytes.push(0x01);
            bytes.extend_from_slice(&self.attempt.generation().to_le_bytes());
            bytes.push(0x02);
            bytes.extend_from_slice(self.attempt.session().as_bytes());
            bytes.push(0x03);
            bytes.extend_from_slice(self.attempt.invocation().as_bytes());
            write_identity(bytes, 0x10, self.scope.package().as_bytes());
            write_identity(bytes, 0x11, self.scope.kernel_set().as_bytes());
            write_identity(bytes, 0x12, self.scope.target().as_bytes());
            write_identity(bytes, 0x20, self.request_identity().as_bytes());
            write_identity(bytes, 0x21, self.worker_identity().as_bytes());
            write_identity(bytes, 0x22, self.response_identity().as_bytes());
            write_identity(bytes, 0x23, self.linked_output_identity().as_bytes());
            write_identity(bytes, 0x24, self.finalization_identity().as_bytes());
            write_identity(bytes, 0x25, self.finalized_output_identity().as_bytes());
            write_typed_digest(
                bytes,
                0x30,
                self.binding.expectation().ffi_contract_identity().digest(),
            );
            write_typed_digest(bytes, 0x31, self.binding.container_identity().digest());
            write_typed_digest(bytes, 0x32, self.bundle.bundle_index_identity().digest());
            write_typed_digest(
                bytes,
                0x33,
                self.bundle.digest(DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM),
            );
            if self.scope_provenance
                == DirectLinkPublicationScopeProvenanceV1::ManifestClaimDerivedV1
            {
                write_identity(bytes, 0x34, self.occurrence_identity().as_bytes());
            }
        }))
    }

    /// Validates a completed G5 publication against every identity prepared by this bridge.
    pub fn validate_published(
        &self,
        published: PublishedLinkArtifactV1,
    ) -> Result<(), DirectLinkBridgeError> {
        require(
            published.attempt() == self.attempt,
            DirectLinkBridgeIdentityKindV1::Attempt,
        )?;
        require(
            published.scope() == self.scope,
            DirectLinkBridgeIdentityKindV1::Scope,
        )?;
        require(
            published.request() == self.request_identity(),
            DirectLinkBridgeIdentityKindV1::Request,
        )?;
        require(
            published.worker() == self.worker_identity(),
            DirectLinkBridgeIdentityKindV1::WorkerClosure,
        )?;
        require(
            published.response() == self.response_identity(),
            DirectLinkBridgeIdentityKindV1::Response,
        )?;
        require(
            published.linked_output() == self.linked_output_identity(),
            DirectLinkBridgeIdentityKindV1::LinkedOutput,
        )?;
        require(
            published.finalization() == self.finalization_identity(),
            DirectLinkBridgeIdentityKindV1::Finalization,
        )?;
        require(
            published.finalized_output() == self.finalized_output_identity(),
            DirectLinkBridgeIdentityKindV1::FinalizedOutput,
        )?;
        require(
            published.publication() == self.publication_identity(),
            DirectLinkBridgeIdentityKindV1::Publication,
        )
    }

    /// Legacy bridge claims never grant publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Bridge evidence never grants module-loading authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Bridge evidence never grants kernel-launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    fn require_sha256_domains(&self) -> Result<(), DirectLinkBridgeError> {
        let expectation = self.binding.expectation();
        for (field, digest) in [
            ("request", expectation.request_identity().digest()),
            (
                "worker executable",
                expectation.worker().executable_digest().digest(),
            ),
            (
                "worker configuration",
                expectation.worker().configuration_digest().digest(),
            ),
            (
                "toolchain executable",
                expectation.toolchain().executable_digest().digest(),
            ),
            (
                "toolchain configuration",
                expectation.toolchain().configuration_digest().digest(),
            ),
            ("response", expectation.response_identity().digest()),
            (
                "linked output",
                expectation.linked_output_identity().digest(),
            ),
            ("finalization", expectation.finalization_identity().digest()),
            (
                "finalized payload",
                expectation.finalized_payload_identity().digest(),
            ),
            ("FFI closure", expectation.ffi_contract_identity().digest()),
            ("container", self.binding.container_identity().digest()),
            ("bundle", self.bundle.bundle_index_identity().digest()),
        ] {
            if digest.algorithm() != DigestAlgorithm::Sha256 {
                return Err(DirectLinkBridgeError::UnsupportedDigestAlgorithm { field });
            }
        }
        Ok(())
    }
}

/// Explicitly non-authoritative diagnostics for a direct-link publication bridge.
///
/// This is the only API by which the manifest-claim bridge reveals the raw G5
/// scope claim. The returned scope is suitable for diagnostics and tests of the
/// inert G5 model only. This value is not accepted by the durable-handoff path
/// and grants no package, publication, load, or launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonAuthoritativeDirectLinkPublicationDiagnosticsV1 {
    scope_claim: LinkPublicationScopeV1,
    scope_provenance: DirectLinkPublicationScopeProvenanceV1,
}

impl NonAuthoritativeDirectLinkPublicationDiagnosticsV1 {
    pub const fn descriptive_scope_claim(&self) -> LinkPublicationScopeV1 {
        self.scope_claim
    }

    pub const fn scope_provenance(&self) -> DirectLinkPublicationScopeProvenanceV1 {
        self.scope_provenance
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Opaque, inert manifest-claim bridge for one G6 binding and G5 model chain.
///
/// Unlike [`DirectLinkPublicationBridgeV1`], this type can be constructed only
/// by consuming a [`ManifestClaimDerivedLinkPublicationScopeV1`] that matches
/// the selected binding and validated evidence envelope. It alone can produce
/// [`ManifestClaimDirectLinkDurablePlanHandoffV1`], making legacy external
/// scope claims structurally unable to enter that future adapter boundary.
///
/// Manifest fields and the caller package identity remain unauthenticated
/// claims. This bridge grants no publication, load, or launch authority. A
/// future authoritative adapter must additionally require the separately owned
/// G5 package lease/current-publication witness and G7 HSACO inspection witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestClaimDirectLinkPublicationBridgeV1 {
    bridge: DirectLinkPublicationBridgeV1,
}

impl ManifestClaimDirectLinkPublicationBridgeV1 {
    /// Prepares a bridge with target and logical-kernel-set manifest claims.
    ///
    /// The witness is consumed and must match the selected index, exact binding,
    /// and complete validated G6 evidence envelope. Package identity remains an
    /// explicit unauthenticated caller claim captured by the witness.
    pub fn prepare_with_manifest_claim_scope(
        attempt: BuildAttempt,
        manifest_claim_scope: ManifestClaimDerivedLinkPublicationScopeV1,
        validated: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        binding_index: usize,
    ) -> Result<Self, DirectLinkBridgeError> {
        let binding = validated.bindings().get(binding_index).ok_or(
            DirectLinkBridgeError::BindingIndexOutOfRange {
                index: binding_index,
                binding_count: validated.bindings().len(),
            },
        )?;
        manifest_claim_scope.require_matches(validated, binding_index, binding)?;
        let bridge = DirectLinkPublicationBridgeV1 {
            attempt,
            scope: manifest_claim_scope.scope,
            scope_provenance: DirectLinkPublicationScopeProvenanceV1::ManifestClaimDerivedV1,
            bundle: validated.evidence().clone(),
            binding: binding.clone(),
        };
        bridge.require_sha256_domains()?;
        Ok(Self { bridge })
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.bridge.attempt()
    }

    /// Returns raw claims only through an explicitly non-authoritative view.
    pub const fn non_authoritative_diagnostics(
        &self,
    ) -> NonAuthoritativeDirectLinkPublicationDiagnosticsV1 {
        NonAuthoritativeDirectLinkPublicationDiagnosticsV1 {
            scope_claim: self.bridge.scope,
            scope_provenance: DirectLinkPublicationScopeProvenanceV1::ManifestClaimDerivedV1,
        }
    }

    pub fn request_identity(&self) -> CanonicalLinkRequestIdentityV1 {
        self.bridge.request_identity()
    }

    pub fn worker_identity(&self) -> PinnedWorkerIdentityV1 {
        self.bridge.worker_identity()
    }

    pub fn response_identity(&self) -> ValidatedResponseIdentityV1 {
        self.bridge.response_identity()
    }

    pub fn linked_output_identity(&self) -> LinkedOutputIdentityV1 {
        self.bridge.linked_output_identity()
    }

    pub fn finalization_identity(&self) -> FinalizationIdentityV1 {
        self.bridge.finalization_identity()
    }

    pub fn finalized_output_identity(&self) -> FinalizedOutputIdentityV1 {
        self.bridge.finalized_output_identity()
    }

    pub fn occurrence_identity(&self) -> DirectLinkPublicationOccurrenceIdentityV1 {
        self.bridge.occurrence_identity()
    }

    pub fn publication_identity(&self) -> AtomicPublicationIdentityV1 {
        self.bridge.publication_identity()
    }

    /// Produces the derived-only opaque handoff for the durable adapter.
    ///
    /// No raw publication scope can be extracted from the returned value. The
    /// G5 adapter consumes it through a dedicated API that preserves its
    /// manifest-claim provenance and exact occurrence binding.
    pub fn durable_plan_handoff(&self) -> ManifestClaimDirectLinkDurablePlanHandoffV1 {
        ManifestClaimDirectLinkDurablePlanHandoffV1 {
            attempt: self.bridge.attempt,
            _scope_claim: self.bridge.scope,
            request: self.request_identity(),
            worker: self.worker_identity(),
            response: self.response_identity(),
            linked_output: self.linked_output_identity(),
            finalization: self.finalization_identity(),
            finalized_output: self.finalized_output_identity(),
            publication: self.publication_identity(),
            occurrence: self.occurrence_identity(),
            container_identity: self.bridge.binding.container_identity(),
            finalized_payload_identity: self
                .bridge
                .binding
                .expectation()
                .finalized_payload_identity(),
            bundle_index_identity: self.bridge.bundle.bundle_index_identity(),
            evidence_identity: self
                .bridge
                .bundle
                .digest(DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM),
        }
    }

    /// Validates a completed inert G5 model publication against this bridge.
    pub fn validate_published(
        &self,
        published: PublishedLinkArtifactV1,
    ) -> Result<(), DirectLinkBridgeError> {
        self.bridge.validate_published(published)
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn require(
    matches: bool,
    kind: DirectLinkBridgeIdentityKindV1,
) -> Result<(), DirectLinkBridgeError> {
    if matches {
        Ok(())
    } else {
        Err(DirectLinkBridgeError::IdentityMismatch { kind })
    }
}

fn require_manifest_claim(
    matches: bool,
    field: DirectLinkManifestClaimScopeFieldV1,
) -> Result<(), DirectLinkBridgeError> {
    if matches {
        Ok(())
    } else {
        Err(DirectLinkBridgeError::ManifestClaimScopeMismatch { field })
    }
}

fn stream_container_identity(
    container: &ArtifactContainerV1,
) -> Result<DirectLinkContainerIdentityV1, DirectLinkBridgeError> {
    let manifest = container.manifest().to_bytes();
    if manifest.len() > MAX_MANIFEST_BYTES {
        return Err(DirectLinkBridgeError::CanonicalManifestEncodingTooLarge {
            actual: manifest.len(),
            max: MAX_MANIFEST_BYTES,
        });
    }

    let mut hasher = Sha256::new();
    hasher.update(CONTAINER_MAGIC);
    hasher.update(CONTAINER_VERSION.to_le_bytes());
    hasher.update(0_u16.to_le_bytes());
    hasher.update(
        match container.digest_algorithm() {
            DigestAlgorithm::Sha256 => 1_u16,
        }
        .to_le_bytes(),
    );
    hasher.update(0_u16.to_le_bytes());
    hasher.update((manifest.len() as u32).to_le_bytes());
    hasher.update((container.payloads().len() as u32).to_le_bytes());
    hasher.update(&manifest);
    for payload in container.payloads() {
        hasher.update(payload.digest().bytes().as_bytes());
        hasher.update((payload.bytes().len() as u64).to_le_bytes());
    }
    for payload in container.payloads() {
        hasher.update(payload.bytes());
    }
    let digest = PayloadDigest::new(
        DigestAlgorithm::Sha256,
        DigestBytes::from_bytes(hasher.finalize().into()),
    );
    Ok(DirectLinkContainerIdentityV1::new(digest))
}

fn derive_manifest_claim_target(container: &ArtifactContainerV1) -> [u8; 32] {
    let target = container.manifest().target();
    calculate_identity(MANIFEST_CLAIM_TARGET_DOMAIN, |bytes| {
        bytes.push(1);
        write_text(bytes, target.triple().as_str());
        write_text(bytes, target.architecture().as_str());
        bytes.push(crate::encode::pointer_width_tag(target.pointer_width()));
        bytes.push(crate::encode::endianness_tag(target.endianness()));
        bytes.extend_from_slice(&(target.capabilities().len() as u16).to_le_bytes());
        for capability in target.capabilities() {
            bytes.extend_from_slice(&crate::encode::capability_tag(*capability).to_le_bytes());
        }
    })
}

fn derive_logical_kernel_set_claim(
    ffi_closure_claim: PayloadDigest,
    kernels: &[&KernelEntry],
) -> [u8; 32] {
    calculate_identity(MANIFEST_CLAIM_KERNEL_SET_DOMAIN, |bytes| {
        bytes.push(1);
        write_typed_digest(bytes, 0x10, ffi_closure_claim);
        bytes.extend_from_slice(&(kernels.len() as u16).to_le_bytes());
        for kernel in kernels {
            write_logical_kernel_claim(bytes, kernel);
        }
    })
}

fn write_logical_kernel_claim(bytes: &mut Vec<u8>, kernel: &KernelEntry) {
    bytes.push(0x20);
    write_digest_bytes(bytes, kernel.kernel_id());
    write_text(bytes, kernel.name().as_str());
    write_text(bytes, kernel.symbol().as_str());
    write_digest_bytes(bytes, kernel.source_digest());
    bytes.extend_from_slice(&(kernel.required_capabilities().len() as u16).to_le_bytes());
    for capability in kernel.required_capabilities() {
        bytes.extend_from_slice(&crate::encode::capability_tag(*capability).to_le_bytes());
    }
    write_launch_claim(bytes, kernel.launch());
    write_abi_claim(bytes, kernel.abi());
}

fn write_launch_claim(bytes: &mut Vec<u8>, launch: &crate::LaunchContract) {
    bytes.push(launch.rank());
    match launch.block_size() {
        BlockSize::Any => bytes.push(0),
        BlockSize::Exact(dimensions) => {
            bytes.push(1);
            write_dimensions(bytes, dimensions);
        }
        BlockSize::AtMost(dimensions) => {
            bytes.push(2);
            write_dimensions(bytes, dimensions);
        }
    }
    write_dimensions(bytes, launch.max_grid());
    bytes.extend_from_slice(&launch.static_shared_memory_bytes().to_le_bytes());
    bytes.extend_from_slice(&launch.max_dynamic_shared_memory_bytes().to_le_bytes());
}

fn write_dimensions(bytes: &mut Vec<u8>, dimensions: Dimensions) {
    bytes.extend_from_slice(&dimensions.x().to_le_bytes());
    bytes.extend_from_slice(&dimensions.y().to_le_bytes());
    bytes.extend_from_slice(&dimensions.z().to_le_bytes());
}

fn write_abi_claim(bytes: &mut Vec<u8>, abi: &crate::AbiLayout) {
    bytes.extend_from_slice(&abi.size().to_le_bytes());
    bytes.extend_from_slice(&abi.alignment().to_le_bytes());
    bytes.push(crate::encode::pointer_width_tag(abi.pointer_width()));
    bytes.extend_from_slice(&(abi.fields().len() as u16).to_le_bytes());
    for field in abi.fields() {
        write_text(bytes, field.name().as_str());
        bytes.extend_from_slice(&field.offset().to_le_bytes());
        bytes.extend_from_slice(&field.size().to_le_bytes());
        bytes.extend_from_slice(&field.alignment().to_le_bytes());
        match field.kind() {
            AbiKind::Scalar(scalar) => {
                bytes.push(0);
                bytes.push(crate::encode::scalar_tag(scalar));
            }
            AbiKind::Pointer {
                pointee_size,
                pointee_alignment,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&pointee_size.to_le_bytes());
                bytes.extend_from_slice(&pointee_alignment.to_le_bytes());
            }
            AbiKind::Slice {
                element_size,
                element_alignment,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&element_size.to_le_bytes());
                bytes.extend_from_slice(&element_alignment.to_le_bytes());
            }
        }
        bytes.push(crate::encode::mutability_tag(field.mutability()));
        bytes.push(crate::encode::access_tag(field.access()));
        bytes.push(crate::encode::address_space_tag(field.address_space()));
        write_digest_bytes(bytes, field.type_identity().rust_type().bytes());
        write_digest_bytes(bytes, field.type_identity().layout().bytes());
        bytes.push(ownership_tag(field.ownership()));
        bytes.push(alias_class_tag(field.alias_class()));
    }
}

const fn ownership_tag(value: ArgumentOwnership) -> u8 {
    match value {
        ArgumentOwnership::ByValue => 0,
        ArgumentOwnership::SharedBorrow => 1,
        ArgumentOwnership::UniqueBorrow => 2,
        ArgumentOwnership::RawPointer => 3,
    }
}

const fn alias_class_tag(value: AliasClass) -> u8 {
    match value {
        AliasClass::Value => 0,
        AliasClass::SharedReadOnly => 1,
        AliasClass::Exclusive => 2,
        AliasClass::Unrestricted => 3,
        AliasClass::SharedAtomic => 4,
    }
}

fn publication_occurrence_identity(
    binding: &DirectLinkBindingV1,
) -> DirectLinkPublicationOccurrenceIdentityV1 {
    DirectLinkPublicationOccurrenceIdentityV1(calculate_identity(
        PUBLICATION_OCCURRENCE_DOMAIN,
        |bytes| {
            bytes.push(1);
            write_typed_digest(bytes, 0x10, binding.container_identity().digest());
            write_typed_digest(
                bytes,
                0x11,
                binding.expectation().finalized_payload_identity().digest(),
            );
        },
    ))
}

fn calculate_identity(domain: &[u8], write: impl FnOnce(&mut Vec<u8>)) -> [u8; 32] {
    let mut preimage = Vec::with_capacity(1024);
    preimage.extend_from_slice(domain);
    write(&mut preimage);
    digest_bytes(DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM.calculate(&preimage))
}

fn digest_bytes(digest: PayloadDigest) -> [u8; 32] {
    *digest.bytes().as_bytes()
}

fn write_digest(bytes: &mut Vec<u8>, digest: PayloadDigest) {
    bytes.push(match digest.algorithm() {
        DigestAlgorithm::Sha256 => 0,
    });
    bytes.extend_from_slice(digest.bytes().as_bytes());
}

fn write_digest_bytes(bytes: &mut Vec<u8>, digest: DigestBytes) {
    bytes.extend_from_slice(digest.as_bytes());
}

fn write_identity(bytes: &mut Vec<u8>, tag: u8, identity: &[u8; 32]) {
    bytes.push(tag);
    bytes.extend_from_slice(identity);
}

fn write_typed_digest(bytes: &mut Vec<u8>, tag: u8, digest: PayloadDigest) {
    bytes.push(tag);
    write_digest(bytes, digest);
}

fn write_text(bytes: &mut Vec<u8>, text: &str) {
    bytes.extend_from_slice(&(text.len() as u16).to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
}

fn write_worker(bytes: &mut Vec<u8>, worker: &DirectLinkWorkerIdentityV1) {
    bytes.push(0x10);
    write_text(bytes, worker.name().as_str());
    write_text(bytes, worker.version().as_str());
    write_digest(bytes, worker.executable_digest().digest());
    write_digest(bytes, worker.configuration_digest().digest());
}

fn write_toolchain(bytes: &mut Vec<u8>, toolchain: &DirectLinkToolchainIdentityV1) {
    bytes.push(0x11);
    write_text(bytes, toolchain.name().as_str());
    write_text(bytes, toolchain.version().as_str());
    write_digest(bytes, toolchain.executable_digest().digest());
    write_digest(bytes, toolchain.configuration_digest().digest());
}
