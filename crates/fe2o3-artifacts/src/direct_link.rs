use std::fmt;

use crate::{
    ArtifactContainerV1, BundleIndexV1, CodeObjectFormat, DigestAlgorithm, IdentityText,
    PayloadDigest,
};

pub const MAX_DIRECT_LINK_BINDINGS: usize = 1024;
pub const DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM: DigestAlgorithm = DigestAlgorithm::Sha256;

/// A measured worker or toolchain participating in direct device linking.
///
/// The measurements are caller-supplied evidence. This type neither measures
/// the tool nor authenticates the supplied values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectLinkToolIdentityV1 {
    name: IdentityText,
    version: IdentityText,
    executable_digest: PayloadDigest,
    configuration_digest: PayloadDigest,
}

impl DirectLinkToolIdentityV1 {
    pub const fn new(
        name: IdentityText,
        version: IdentityText,
        executable_digest: PayloadDigest,
        configuration_digest: PayloadDigest,
    ) -> Self {
        Self {
            name,
            version,
            executable_digest,
            configuration_digest,
        }
    }

    pub const fn name(&self) -> &IdentityText {
        &self.name
    }

    pub const fn version(&self) -> &IdentityText {
        &self.version
    }

    pub const fn executable_digest(&self) -> PayloadDigest {
        self.executable_digest
    }

    pub const fn configuration_digest(&self) -> PayloadDigest {
        self.configuration_digest
    }
}

/// Identity chain for descriptor finalization and independent inspection.
///
/// Linked output and finalized payload identities are intentionally distinct:
/// descriptor finalization patches the canonical code-object digest slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectLinkTransformationIdentityV1 {
    linked_output_identity: PayloadDigest,
    finalization_identity: PayloadDigest,
    finalized_payload_identity: PayloadDigest,
}

impl DirectLinkTransformationIdentityV1 {
    pub const fn new(
        linked_output_identity: PayloadDigest,
        finalization_identity: PayloadDigest,
        finalized_payload_identity: PayloadDigest,
    ) -> Self {
        Self {
            linked_output_identity,
            finalization_identity,
            finalized_payload_identity,
        }
    }

    pub const fn linked_output_identity(self) -> PayloadDigest {
        self.linked_output_identity
    }

    pub const fn finalization_identity(self) -> PayloadDigest {
        self.finalization_identity
    }

    pub const fn finalized_payload_identity(self) -> PayloadDigest {
        self.finalized_payload_identity
    }
}

/// The complete externally derived identity closure for one direct-link result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectLinkBindingExpectationV1 {
    request_identity: PayloadDigest,
    worker: DirectLinkToolIdentityV1,
    toolchain: DirectLinkToolIdentityV1,
    response_identity: PayloadDigest,
    transformation: DirectLinkTransformationIdentityV1,
    ffi_contract_identity: PayloadDigest,
}

impl DirectLinkBindingExpectationV1 {
    pub const fn new(
        request_identity: PayloadDigest,
        worker: DirectLinkToolIdentityV1,
        toolchain: DirectLinkToolIdentityV1,
        response_identity: PayloadDigest,
        transformation: DirectLinkTransformationIdentityV1,
        ffi_contract_identity: PayloadDigest,
    ) -> Self {
        Self {
            request_identity,
            worker,
            toolchain,
            response_identity,
            transformation,
            ffi_contract_identity,
        }
    }

    pub const fn request_identity(&self) -> PayloadDigest {
        self.request_identity
    }

    pub const fn worker(&self) -> &DirectLinkToolIdentityV1 {
        &self.worker
    }

    pub const fn toolchain(&self) -> &DirectLinkToolIdentityV1 {
        &self.toolchain
    }

    pub const fn response_identity(&self) -> PayloadDigest {
        self.response_identity
    }

    pub const fn linked_output_identity(&self) -> PayloadDigest {
        self.transformation.linked_output_identity()
    }

    pub const fn finalization_identity(&self) -> PayloadDigest {
        self.transformation.finalization_identity()
    }

    pub const fn finalized_payload_identity(&self) -> PayloadDigest {
        self.transformation.finalized_payload_identity()
    }

    pub const fn transformation(&self) -> DirectLinkTransformationIdentityV1 {
        self.transformation
    }

    pub const fn ffi_contract_identity(&self) -> PayloadDigest {
        self.ffi_contract_identity
    }
}

/// One container and its externally derived direct-link identity closure.
pub struct DirectLinkBindingSourceV1<'a> {
    container: &'a ArtifactContainerV1,
    expectation: DirectLinkBindingExpectationV1,
}

impl<'a> DirectLinkBindingSourceV1<'a> {
    pub const fn new(
        container: &'a ArtifactContainerV1,
        expectation: DirectLinkBindingExpectationV1,
    ) -> Self {
        Self {
            container,
            expectation,
        }
    }
}

/// Canonical evidence for one directly linked payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectLinkBindingV1 {
    container_identity: PayloadDigest,
    expectation: DirectLinkBindingExpectationV1,
}

impl DirectLinkBindingV1 {
    pub const fn container_identity(&self) -> PayloadDigest {
        self.container_identity
    }

    pub const fn expectation(&self) -> &DirectLinkBindingExpectationV1 {
        &self.expectation
    }

    pub(crate) const fn from_decoded(
        container_identity: PayloadDigest,
        expectation: DirectLinkBindingExpectationV1,
    ) -> Self {
        Self {
            container_identity,
            expectation,
        }
    }
}

/// A versioned companion record binding direct-link evidence to a bundle.
///
/// This record is deliberately separate from the V1 manifest, container, and
/// bundle-index wires. Construction derives container and bundle identities
/// from canonical bytes and checks the container's complete bundle-index
/// closure. Decoding only establishes canonical structure. A consumer must
/// authenticate the record separately and call `validate_against` with its own
/// expected identities.
///
/// Neither construction nor validation grants authority to load or launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectLinkBundleEvidenceV1 {
    bundle_index_identity: PayloadDigest,
    bindings: Vec<DirectLinkBindingV1>,
}

impl DirectLinkBundleEvidenceV1 {
    pub fn bind(
        bundle: &BundleIndexV1,
        sources: &[DirectLinkBindingSourceV1<'_>],
    ) -> Result<Self, DirectLinkEvidenceError> {
        require_binding_count(sources.len())?;
        let mut bindings = Vec::with_capacity(sources.len());
        for source in sources {
            validate_container_bundle_closure(bundle, source.container, &source.expectation)?;
            bindings.push(DirectLinkBindingV1 {
                container_identity: container_identity(source.container),
                expectation: source.expectation.clone(),
            });
        }
        canonicalize_bindings(&mut bindings)?;
        Ok(Self {
            bundle_index_identity: bundle_identity(bundle),
            bindings,
        })
    }

    pub const fn bundle_index_identity(&self) -> PayloadDigest {
        self.bundle_index_identity
    }

    pub fn bindings(&self) -> &[DirectLinkBindingV1] {
        &self.bindings
    }

    /// Direct-link evidence is never sufficient to authorize module loading.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Direct-link evidence is never sufficient to authorize kernel launch.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Matches decoded evidence against exact caller-derived expectations and
    /// canonical containers. Success returns no runtime-authority token.
    pub fn validate_against(
        &self,
        bundle: &BundleIndexV1,
        containers: &[ArtifactContainerV1],
        expectations: &[DirectLinkBindingExpectationV1],
    ) -> Result<(), DirectLinkEvidenceError> {
        if self.bundle_index_identity != bundle_identity(bundle) {
            return Err(DirectLinkEvidenceError::BundleIdentityMismatch);
        }
        require_binding_count(expectations.len())?;
        if expectations.len() != self.bindings.len() {
            return Err(DirectLinkEvidenceError::BindingCountMismatch {
                expected: expectations.len(),
                actual: self.bindings.len(),
            });
        }
        if containers.len() > self.bindings.len() {
            return Err(DirectLinkEvidenceError::ExtraContainer);
        }

        let mut expectations = expectations.to_vec();
        canonicalize_expectations(&mut expectations)?;
        for (binding, expected) in self.bindings.iter().zip(&expectations) {
            if binding.expectation != *expected {
                return Err(DirectLinkEvidenceError::ExpectationMismatch);
            }
        }

        let mut measured_containers = containers
            .iter()
            .map(|container| (container_identity(container), container, false))
            .collect::<Vec<_>>();
        measured_containers.sort_unstable_by_key(|(identity, _, _)| *identity);
        if measured_containers
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0)
        {
            return Err(DirectLinkEvidenceError::Duplicate {
                field: "container identity",
            });
        }

        for binding in &self.bindings {
            let index = measured_containers
                .binary_search_by_key(&binding.container_identity, |(identity, _, _)| *identity)
                .map_err(|_| DirectLinkEvidenceError::MissingContainer)?;
            let (_, container, used) = &mut measured_containers[index];
            *used = true;
            validate_container_bundle_closure(bundle, container, &binding.expectation)?;
        }
        if measured_containers.iter().any(|(_, _, used)| !used) {
            return Err(DirectLinkEvidenceError::ExtraContainer);
        }
        Ok(())
    }

    pub(crate) fn from_decoded(
        bundle_index_identity: PayloadDigest,
        mut bindings: Vec<DirectLinkBindingV1>,
    ) -> Result<Self, DirectLinkEvidenceError> {
        require_binding_count(bindings.len())?;
        ensure_canonical_bindings(&bindings)?;
        // Keep decoder behavior fail-closed if model validation grows stricter.
        canonicalize_bindings(&mut bindings)?;
        Ok(Self {
            bundle_index_identity,
            bindings,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DirectLinkEvidenceError {
    EmptyBindings,
    TooManyBindings { max: usize },
    Duplicate { field: &'static str },
    NonCanonicalBindingOrder,
    BundleIdentityMismatch,
    BindingCountMismatch { expected: usize, actual: usize },
    ExpectationMismatch,
    MissingContainer,
    ExtraContainer,
    MissingFinalizedPayload,
    UnreferencedFinalizedPayload,
    FinalizedPayloadNotNative,
    ContainerBundleMismatch { field: &'static str },
}

impl fmt::Display for DirectLinkEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBindings => write!(formatter, "direct-link evidence must not be empty"),
            Self::TooManyBindings { max } => {
                write!(formatter, "direct-link evidence exceeds {max} bindings")
            }
            Self::Duplicate { field } => write!(formatter, "duplicate direct-link {field}"),
            Self::NonCanonicalBindingOrder => {
                write!(formatter, "direct-link bindings are not in canonical order")
            }
            Self::BundleIdentityMismatch => {
                write!(formatter, "direct-link bundle identity does not match")
            }
            Self::BindingCountMismatch { expected, actual } => write!(
                formatter,
                "expected {expected} direct-link bindings, but evidence has {actual}"
            ),
            Self::ExpectationMismatch => {
                write!(formatter, "direct-link identity closure does not match")
            }
            Self::MissingContainer => write!(formatter, "direct-link container is missing"),
            Self::ExtraContainer => write!(formatter, "unbound direct-link container was supplied"),
            Self::MissingFinalizedPayload => {
                write!(formatter, "finalized payload is absent from its container")
            }
            Self::UnreferencedFinalizedPayload => {
                write!(formatter, "no kernel references the finalized payload")
            }
            Self::FinalizedPayloadNotNative => {
                write!(formatter, "finalized payload is not a native executable")
            }
            Self::ContainerBundleMismatch { field } => {
                write!(
                    formatter,
                    "container {field} is absent or differs in the bundle"
                )
            }
        }
    }
}

impl std::error::Error for DirectLinkEvidenceError {}

fn require_binding_count(count: usize) -> Result<(), DirectLinkEvidenceError> {
    if count == 0 {
        Err(DirectLinkEvidenceError::EmptyBindings)
    } else if count > MAX_DIRECT_LINK_BINDINGS {
        Err(DirectLinkEvidenceError::TooManyBindings {
            max: MAX_DIRECT_LINK_BINDINGS,
        })
    } else {
        Ok(())
    }
}

fn canonicalize_bindings(
    bindings: &mut [DirectLinkBindingV1],
) -> Result<(), DirectLinkEvidenceError> {
    bindings.sort_unstable_by_key(|binding| binding.expectation.finalized_payload_identity());
    reject_binding_duplicates(bindings)
}

fn ensure_canonical_bindings(
    bindings: &[DirectLinkBindingV1],
) -> Result<(), DirectLinkEvidenceError> {
    for pair in bindings.windows(2) {
        match pair[0]
            .expectation
            .finalized_payload_identity()
            .cmp(&pair[1].expectation.finalized_payload_identity())
        {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(DirectLinkEvidenceError::Duplicate {
                    field: "finalized payload identity",
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(DirectLinkEvidenceError::NonCanonicalBindingOrder);
            }
        }
    }
    reject_binding_duplicates(bindings)
}

fn reject_binding_duplicates(
    bindings: &[DirectLinkBindingV1],
) -> Result<(), DirectLinkEvidenceError> {
    reject_duplicate_digest(
        bindings
            .iter()
            .map(|binding| binding.expectation.request_identity()),
        "request identity",
    )?;
    reject_duplicate_digest(
        bindings
            .iter()
            .map(|binding| binding.expectation.response_identity()),
        "response identity",
    )?;
    reject_duplicate_digest(
        bindings
            .iter()
            .map(|binding| binding.expectation.finalized_payload_identity()),
        "finalized payload identity",
    )
}

fn canonicalize_expectations(
    expectations: &mut [DirectLinkBindingExpectationV1],
) -> Result<(), DirectLinkEvidenceError> {
    expectations.sort_unstable_by_key(|expectation| expectation.finalized_payload_identity());
    reject_duplicate_digest(
        expectations
            .iter()
            .map(DirectLinkBindingExpectationV1::request_identity),
        "request identity",
    )?;
    reject_duplicate_digest(
        expectations
            .iter()
            .map(DirectLinkBindingExpectationV1::response_identity),
        "response identity",
    )?;
    reject_duplicate_digest(
        expectations
            .iter()
            .map(DirectLinkBindingExpectationV1::finalized_payload_identity),
        "finalized payload identity",
    )
}

fn reject_duplicate_digest(
    values: impl IntoIterator<Item = PayloadDigest>,
    field: &'static str,
) -> Result<(), DirectLinkEvidenceError> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_unstable();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        Err(DirectLinkEvidenceError::Duplicate { field })
    } else {
        Ok(())
    }
}

fn container_identity(container: &ArtifactContainerV1) -> PayloadDigest {
    DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM.calculate(&container.to_bytes())
}

fn bundle_identity(bundle: &BundleIndexV1) -> PayloadDigest {
    DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM.calculate(&bundle.to_bytes())
}

fn validate_container_bundle_closure(
    bundle: &BundleIndexV1,
    container: &ArtifactContainerV1,
    expectation: &DirectLinkBindingExpectationV1,
) -> Result<(), DirectLinkEvidenceError> {
    let finalized = expectation.finalized_payload_identity();
    let payload = container
        .payloads()
        .iter()
        .find(|payload| payload.digest() == finalized)
        .ok_or(DirectLinkEvidenceError::MissingFinalizedPayload)?;
    payload
        .digest()
        .verify(payload.bytes())
        .map_err(|_| DirectLinkEvidenceError::MissingFinalizedPayload)?;

    let object = container
        .manifest()
        .code_objects()
        .iter()
        .find(|object| object.digest() == finalized.bytes())
        .ok_or(DirectLinkEvidenceError::MissingFinalizedPayload)?;
    if object.format() != CodeObjectFormat::NativeExecutable {
        return Err(DirectLinkEvidenceError::FinalizedPayloadNotNative);
    }
    if !container
        .manifest()
        .kernels()
        .iter()
        .any(|kernel| kernel.code_object_digest() == finalized.bytes())
    {
        return Err(DirectLinkEvidenceError::UnreferencedFinalizedPayload);
    }

    let expected = BundleIndexV1::from_containers(std::slice::from_ref(container))
        .map_err(|_| DirectLinkEvidenceError::ContainerBundleMismatch { field: "closure" })?;
    require_subset(
        expected.target_associations(),
        bundle.target_associations(),
        "target association",
    )?;
    require_subset(expected.payloads(), bundle.payloads(), "payload reference")?;
    require_subset(expected.kernels(), bundle.kernels(), "kernel entry")
}

fn require_subset<T: PartialEq>(
    expected: &[T],
    actual: &[T],
    field: &'static str,
) -> Result<(), DirectLinkEvidenceError> {
    if expected.iter().all(|value| actual.contains(value)) {
        Ok(())
    } else {
        Err(DirectLinkEvidenceError::ContainerBundleMismatch { field })
    }
}
