use std::fmt;

use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BuildAttempt, CanonicalLinkRequestIdentityV1,
    FinalizationIdentityV1, FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1,
    LinkedOutputIdentityV1, PackageIdentityV1, PinnedWorkerIdentityV1, PublishedLinkArtifactV1,
    TargetIdentityV1, ValidatedResponseIdentityV1,
};

use crate::{
    ArtifactContainerV1, CodeObjectFormat, DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM, DigestAlgorithm,
    DirectLinkBindingV1, DirectLinkBundleEvidenceV1, DirectLinkBundleIndexIdentityV1,
    DirectLinkContainerIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
    DirectLinkToolchainIdentityV1, DirectLinkWorkerIdentityV1, ManifestV1, PayloadDigest,
    ValidatedDirectLinkBundleEvidenceV1, ValidationError,
};

const WORKER_CLOSURE_DOMAIN: &[u8] = b"fe2o3.direct-link.worker-closure.v1\0";
const PUBLICATION_DOMAIN: &[u8] = b"fe2o3.direct-link.publication-bridge.v1\0";
const DERIVED_SCOPE_PUBLICATION_DOMAIN: &[u8] =
    b"fe2o3.direct-link.publication-bridge.derived-scope.v1\0";
const DERIVED_TARGET_DOMAIN: &[u8] = b"fe2o3.direct-link.publication-scope.target.v1\0";
const DERIVED_KERNEL_SET_DOMAIN: &[u8] = b"fe2o3.direct-link.publication-scope.kernel-set.v1\0";

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
    /// Weaker compatibility path: all three identities came from external policy.
    TrustedExternalPolicy,
    /// Package policy is external; target and kernel-set identities were derived from artifacts.
    ArtifactDerivedV1,
}

/// Field that did not match an opaque artifact-derived scope witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectLinkDerivedScopeFieldV1 {
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
    DerivedScopeMismatch {
        field: DirectLinkDerivedScopeFieldV1,
    },
    DerivedScopeProjectionInvalid(ValidationError),
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
            Self::DerivedScopeMismatch { field } => {
                write!(formatter, "artifact-derived G5 scope {field:?} mismatch")
            }
            Self::DerivedScopeProjectionInvalid(error) => {
                write!(
                    formatter,
                    "artifact-derived G5 scope projection is invalid: {error}"
                )
            }
        }
    }
}

impl std::error::Error for DirectLinkBridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DerivedScopeProjectionInvalid(error) => Some(error),
            _ => None,
        }
    }
}

/// Opaque V1 witness for one artifact-derived G5 publication scope.
///
/// Construction binds an externally trusted package identity to one exact G6
/// binding occurrence and concrete container. The target identity covers every
/// canonical target field. The kernel-set identity covers a canonical manifest
/// projection containing the producer identities, complete target, finalized
/// native code-object record, and the complete sorted set of kernel records
/// that reference that payload.
///
/// This witness authenticates none of its inputs and grants no load or launch
/// authority. In particular, `package` remains an external policy assertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactDerivedLinkPublicationScopeV1 {
    binding_index: usize,
    bundle_index_identity: DirectLinkBundleIndexIdentityV1,
    evidence_identity: PayloadDigest,
    binding: DirectLinkBindingV1,
    scope: LinkPublicationScopeV1,
}

impl ArtifactDerivedLinkPublicationScopeV1 {
    /// Derives a scope from one validated binding and its exact container.
    pub fn derive(
        package: PackageIdentityV1,
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
        let measured_container_identity = DirectLinkContainerIdentityV1::new(
            DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM.calculate(&container.to_bytes()),
        );
        require_derived(
            measured_container_identity == binding.container_identity(),
            DirectLinkDerivedScopeFieldV1::ContainerIdentity,
        )?;

        let finalized = binding.expectation().finalized_payload_identity();
        let object = container
            .manifest()
            .code_objects()
            .iter()
            .find(|object| object.digest() == finalized.digest().bytes())
            .filter(|object| object.format() == CodeObjectFormat::NativeExecutable)
            .ok_or(DirectLinkBridgeError::DerivedScopeMismatch {
                field: DirectLinkDerivedScopeFieldV1::FinalizedPayloadOccurrence,
            })?;
        let payload = container
            .payloads()
            .iter()
            .find(|payload| payload.digest() == finalized.digest())
            .ok_or(DirectLinkBridgeError::DerivedScopeMismatch {
                field: DirectLinkDerivedScopeFieldV1::FinalizedPayloadOccurrence,
            })?;
        payload.digest().verify(payload.bytes()).map_err(|_| {
            DirectLinkBridgeError::DerivedScopeMismatch {
                field: DirectLinkDerivedScopeFieldV1::FinalizedPayloadOccurrence,
            }
        })?;

        let kernels = container
            .manifest()
            .kernels()
            .iter()
            .filter(|kernel| kernel.code_object_digest() == finalized.digest().bytes())
            .cloned()
            .collect::<Vec<_>>();
        if kernels.is_empty() {
            return Err(DirectLinkBridgeError::DerivedScopeMismatch {
                field: DirectLinkDerivedScopeFieldV1::FinalizedPayloadOccurrence,
            });
        }
        let kernel_projection = ManifestV1::new(
            container.manifest().compiler().clone(),
            container.manifest().producer().clone(),
            container.manifest().target().clone(),
            vec![object.clone()],
            kernels,
        )
        .map_err(DirectLinkBridgeError::DerivedScopeProjectionInvalid)?;

        let target = TargetIdentityV1::from_bytes(derive_target_identity(container));
        let kernel_set = KernelSetIdentityV1::from_bytes(derive_kernel_set_identity(
            finalized,
            &kernel_projection,
        ));
        Ok(Self {
            binding_index,
            bundle_index_identity: validated.evidence().bundle_index_identity(),
            evidence_identity: validated
                .evidence()
                .digest(DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM),
            binding: binding.clone(),
            scope: LinkPublicationScopeV1::new(package, kernel_set, target),
        })
    }

    pub const fn scope(&self) -> LinkPublicationScopeV1 {
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

    /// Derived-scope evidence never grants module-loading authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Derived-scope evidence never grants kernel-launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    fn require_matches(
        &self,
        validated: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        binding_index: usize,
        binding: &DirectLinkBindingV1,
    ) -> Result<(), DirectLinkBridgeError> {
        require_derived(
            self.binding_index == binding_index,
            DirectLinkDerivedScopeFieldV1::BindingIndex,
        )?;
        require_derived(
            self.binding == *binding,
            DirectLinkDerivedScopeFieldV1::Binding,
        )?;
        let evidence = validated.evidence();
        require_derived(
            self.bundle_index_identity == evidence.bundle_index_identity()
                && self.evidence_identity == evidence.digest(DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM),
            DirectLinkDerivedScopeFieldV1::BundleEvidence,
        )
    }
}

/// Typed, inert conversion between one G6 binding and one G5 publication chain.
///
/// Construction requires an opaque witness produced by exact validation
/// against a concrete bundle, complete container set, and binding sources. The
/// returned values are the only normative conversions into G5 identity domains:
/// direct SHA-256 identities are converted field by field, while worker/toolchain
/// and publication identities are derived from domain-separated canonical
/// preimages. Scope is either externally asserted by the weaker compatibility
/// path or target/kernel-set-derived from an exact container. The publication
/// preimage commits to the attempt, scope,
/// request, worker and toolchain measurements, response, transformation, FFI
/// closure, container, bundle, and complete direct-link evidence envelope.
///
/// This model performs no filesystem I/O, does not authenticate caller-supplied measurements, and
/// grants no authority to load or launch code. Filesystem publication and durable recovery remain
/// the responsibility of a future adapter under the artifact transaction lock.
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
    /// This is the weaker compatibility constructor. `trusted_scope` is
    /// supplied entirely by an external trusted policy boundary.
    /// This model does not derive or verify its package, kernel-set, or target
    /// identities from artifact records. The value is committed into the
    /// publication identity without being elevated to an artifact-derived fact.
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
            scope_provenance: DirectLinkPublicationScopeProvenanceV1::TrustedExternalPolicy,
            bundle: validated.evidence().clone(),
            binding: binding.clone(),
        };
        bridge.require_sha256_domains()?;
        Ok(bridge)
    }

    /// Prepares a bridge with target and kernel-set scope derived from artifacts.
    ///
    /// The witness is consumed and must match the selected index, exact binding,
    /// and complete validated G6 evidence envelope. Package identity remains an
    /// explicitly trusted external policy input captured by the witness.
    pub fn prepare_with_derived_scope(
        attempt: BuildAttempt,
        derived_scope: ArtifactDerivedLinkPublicationScopeV1,
        validated: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        binding_index: usize,
    ) -> Result<Self, DirectLinkBridgeError> {
        let binding = validated.bindings().get(binding_index).ok_or(
            DirectLinkBridgeError::BindingIndexOutOfRange {
                index: binding_index,
                binding_count: validated.bindings().len(),
            },
        )?;
        derived_scope.require_matches(validated, binding_index, binding)?;
        let bridge = Self {
            attempt,
            scope: derived_scope.scope,
            scope_provenance: DirectLinkPublicationScopeProvenanceV1::ArtifactDerivedV1,
            bundle: validated.evidence().clone(),
            binding: binding.clone(),
        };
        bridge.require_sha256_domains()?;
        Ok(bridge)
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    /// Compatibility accessor for the publication scope.
    ///
    /// Callers must inspect `scope_provenance` before treating target or
    /// kernel-set fields as artifact-derived.
    pub const fn trusted_scope(&self) -> LinkPublicationScopeV1 {
        self.scope
    }

    /// Returns the publication scope committed by this bridge.
    pub const fn publication_scope(&self) -> LinkPublicationScopeV1 {
        self.scope
    }

    /// Returns whether scope fields were externally asserted or artifact-derived.
    pub const fn scope_provenance(&self) -> DirectLinkPublicationScopeProvenanceV1 {
        self.scope_provenance
    }

    pub const fn binding(&self) -> &DirectLinkBindingV1 {
        &self.binding
    }

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

    /// Derives the atomic G5 publication identity over the complete G5/G6 closure.
    pub fn publication_identity(&self) -> AtomicPublicationIdentityV1 {
        let domain = match self.scope_provenance {
            DirectLinkPublicationScopeProvenanceV1::TrustedExternalPolicy => PUBLICATION_DOMAIN,
            DirectLinkPublicationScopeProvenanceV1::ArtifactDerivedV1 => {
                DERIVED_SCOPE_PUBLICATION_DOMAIN
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

fn require_derived(
    matches: bool,
    field: DirectLinkDerivedScopeFieldV1,
) -> Result<(), DirectLinkBridgeError> {
    if matches {
        Ok(())
    } else {
        Err(DirectLinkBridgeError::DerivedScopeMismatch { field })
    }
}

fn derive_target_identity(container: &ArtifactContainerV1) -> [u8; 32] {
    let target = container.manifest().target();
    calculate_identity(DERIVED_TARGET_DOMAIN, |bytes| {
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

fn derive_kernel_set_identity(
    finalized: DirectLinkFinalizedPayloadIdentityV1,
    projection: &ManifestV1,
) -> [u8; 32] {
    calculate_identity(DERIVED_KERNEL_SET_DOMAIN, |bytes| {
        bytes.push(1);
        write_digest(bytes, finalized.digest());
        let projection = projection.to_bytes();
        bytes.extend_from_slice(&(projection.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&projection);
    })
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
