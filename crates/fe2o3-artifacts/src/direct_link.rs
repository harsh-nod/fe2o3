use std::fmt;

use crate::{
    ArtifactContainerV1, BundleIndexV1, CodeObjectFormat, DigestAlgorithm, IdentityText,
    PayloadDigest,
};

pub const MAX_DIRECT_LINK_BINDINGS: usize = 1024;
pub const DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM: DigestAlgorithm = DigestAlgorithm::Sha256;

macro_rules! digest_identity_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(PayloadDigest);

        impl $name {
            /// Constructs a typed identity from algorithm-tagged digest bytes.
            pub const fn new(digest: PayloadDigest) -> Self {
                Self(digest)
            }

            /// Returns the algorithm-tagged digest carried by this identity domain.
            pub const fn digest(self) -> PayloadDigest {
                self.0
            }
        }
    };
}

digest_identity_type!(
    /// Canonical identity of one closed direct-link request.
    ///
    /// Identity domains are intentionally not interchangeable:
    /// ```compile_fail
    /// use fe2o3_artifacts::{
    ///     DigestAlgorithm, DigestBytes, DirectLinkRequestIdentityV1,
    ///     DirectLinkResponseIdentityV1, PayloadDigest,
    /// };
    /// let digest = PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes([7; 32]));
    /// let response = DirectLinkResponseIdentityV1::new(digest);
    /// let _: DirectLinkRequestIdentityV1 = response;
    /// ```
    DirectLinkRequestIdentityV1
);
digest_identity_type!(
    /// Canonical identity of a worker response validated against its request.
    DirectLinkResponseIdentityV1
);
digest_identity_type!(
    /// Content identity of linked bytes before finalization.
    DirectLinkLinkedOutputIdentityV1
);
digest_identity_type!(
    /// Canonical identity of finalization and inspection evidence.
    DirectLinkFinalizationIdentityV1
);
digest_identity_type!(
    /// Content identity of a finalized native payload.
    DirectLinkFinalizedPayloadIdentityV1
);
digest_identity_type!(
    /// Canonical identity of the closed device FFI contract.
    DirectLinkFfiClosureIdentityV1
);
digest_identity_type!(
    /// Content identity of the worker executable.
    DirectLinkWorkerExecutableIdentityV1
);
digest_identity_type!(
    /// Canonical identity of the worker configuration.
    DirectLinkWorkerConfigurationIdentityV1
);
digest_identity_type!(
    /// Content identity of the LLVM/LLD toolchain executable closure.
    DirectLinkToolchainExecutableIdentityV1
);
digest_identity_type!(
    /// Canonical identity of the LLVM/LLD toolchain configuration.
    DirectLinkToolchainConfigurationIdentityV1
);
digest_identity_type!(
    /// Content identity of one canonical artifact container.
    DirectLinkContainerIdentityV1
);
digest_identity_type!(
    /// Content identity of one canonical bundle index.
    DirectLinkBundleIndexIdentityV1
);

macro_rules! tool_identity_type {
    ($name:ident, $executable:ident, $configuration:ident, $description:literal) => {
        #[doc = $description]
        ///
        /// Measurements are caller-supplied evidence. This type neither performs nor
        /// authenticates the measurement.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name {
            name: IdentityText,
            version: IdentityText,
            executable_digest: $executable,
            configuration_digest: $configuration,
        }

        impl $name {
            pub const fn new(
                name: IdentityText,
                version: IdentityText,
                executable_digest: $executable,
                configuration_digest: $configuration,
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

            pub const fn executable_digest(&self) -> $executable {
                self.executable_digest
            }

            pub const fn configuration_digest(&self) -> $configuration {
                self.configuration_digest
            }
        }
    };
}

tool_identity_type!(
    DirectLinkWorkerIdentityV1,
    DirectLinkWorkerExecutableIdentityV1,
    DirectLinkWorkerConfigurationIdentityV1,
    "A measured worker participating in direct device linking."
);
tool_identity_type!(
    DirectLinkToolchainIdentityV1,
    DirectLinkToolchainExecutableIdentityV1,
    DirectLinkToolchainConfigurationIdentityV1,
    "A measured LLVM/LLD toolchain closure participating in direct device linking."
);

/// Identity chain for descriptor finalization and independent inspection.
///
/// Linked output and finalized payload identities are intentionally distinct:
/// descriptor finalization patches the canonical code-object digest slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectLinkTransformationIdentityV1 {
    linked_output_identity: DirectLinkLinkedOutputIdentityV1,
    finalization_identity: DirectLinkFinalizationIdentityV1,
    finalized_payload_identity: DirectLinkFinalizedPayloadIdentityV1,
}

impl DirectLinkTransformationIdentityV1 {
    pub const fn new(
        linked_output_identity: DirectLinkLinkedOutputIdentityV1,
        finalization_identity: DirectLinkFinalizationIdentityV1,
        finalized_payload_identity: DirectLinkFinalizedPayloadIdentityV1,
    ) -> Self {
        Self {
            linked_output_identity,
            finalization_identity,
            finalized_payload_identity,
        }
    }

    pub const fn linked_output_identity(self) -> DirectLinkLinkedOutputIdentityV1 {
        self.linked_output_identity
    }

    pub const fn finalization_identity(self) -> DirectLinkFinalizationIdentityV1 {
        self.finalization_identity
    }

    pub const fn finalized_payload_identity(self) -> DirectLinkFinalizedPayloadIdentityV1 {
        self.finalized_payload_identity
    }
}

/// The complete externally derived identity closure for one direct-link result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectLinkBindingExpectationV1 {
    request_identity: DirectLinkRequestIdentityV1,
    worker: DirectLinkWorkerIdentityV1,
    toolchain: DirectLinkToolchainIdentityV1,
    response_identity: DirectLinkResponseIdentityV1,
    transformation: DirectLinkTransformationIdentityV1,
    ffi_contract_identity: DirectLinkFfiClosureIdentityV1,
}

impl DirectLinkBindingExpectationV1 {
    pub const fn new(
        request_identity: DirectLinkRequestIdentityV1,
        worker: DirectLinkWorkerIdentityV1,
        toolchain: DirectLinkToolchainIdentityV1,
        response_identity: DirectLinkResponseIdentityV1,
        transformation: DirectLinkTransformationIdentityV1,
        ffi_contract_identity: DirectLinkFfiClosureIdentityV1,
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

    pub const fn request_identity(&self) -> DirectLinkRequestIdentityV1 {
        self.request_identity
    }

    pub const fn worker(&self) -> &DirectLinkWorkerIdentityV1 {
        &self.worker
    }

    pub const fn toolchain(&self) -> &DirectLinkToolchainIdentityV1 {
        &self.toolchain
    }

    pub const fn response_identity(&self) -> DirectLinkResponseIdentityV1 {
        self.response_identity
    }

    pub const fn linked_output_identity(&self) -> DirectLinkLinkedOutputIdentityV1 {
        self.transformation.linked_output_identity()
    }

    pub const fn finalization_identity(&self) -> DirectLinkFinalizationIdentityV1 {
        self.transformation.finalization_identity()
    }

    pub const fn finalized_payload_identity(&self) -> DirectLinkFinalizedPayloadIdentityV1 {
        self.transformation.finalized_payload_identity()
    }

    pub const fn transformation(&self) -> DirectLinkTransformationIdentityV1 {
        self.transformation
    }

    pub const fn ffi_contract_identity(&self) -> DirectLinkFfiClosureIdentityV1 {
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
    container_identity: DirectLinkContainerIdentityV1,
    expectation: DirectLinkBindingExpectationV1,
}

impl DirectLinkBindingV1 {
    pub const fn container_identity(&self) -> DirectLinkContainerIdentityV1 {
        self.container_identity
    }

    pub const fn expectation(&self) -> &DirectLinkBindingExpectationV1 {
        &self.expectation
    }

    pub(crate) const fn from_decoded(
        container_identity: DirectLinkContainerIdentityV1,
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
    bundle_index_identity: DirectLinkBundleIndexIdentityV1,
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
            validate_binding_source(source.container, &source.expectation)?;
            bindings.push(DirectLinkBindingV1 {
                container_identity: container_identity(source.container),
                expectation: source.expectation.clone(),
            });
        }
        canonicalize_bindings(&mut bindings)?;
        let mut containers = sources
            .iter()
            .map(|source| (container_identity(source.container), source.container))
            .collect::<Vec<_>>();
        containers.sort_unstable_by_key(|(identity, _)| *identity);
        containers.dedup_by_key(|(identity, _)| *identity);
        let containers = containers
            .into_iter()
            .map(|(_, container)| container)
            .collect::<Vec<_>>();
        validate_complete_bundle_closure(bundle, &containers, &bindings)?;
        Ok(Self {
            bundle_index_identity: bundle_identity(bundle),
            bindings,
        })
    }

    pub const fn bundle_index_identity(&self) -> DirectLinkBundleIndexIdentityV1 {
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
            validate_binding_source(container, &binding.expectation)?;
        }
        if measured_containers.iter().any(|(_, _, used)| !used) {
            return Err(DirectLinkEvidenceError::ExtraContainer);
        }
        let containers = measured_containers
            .iter()
            .map(|(_, container, _)| *container)
            .collect::<Vec<_>>();
        validate_complete_bundle_closure(bundle, &containers, &self.bindings)?;
        Ok(())
    }

    pub(crate) fn from_decoded(
        bundle_index_identity: DirectLinkBundleIndexIdentityV1,
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
    MissingExecutableBinding,
    ExtraExecutableBinding,
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
            Self::MissingExecutableBinding => {
                write!(
                    formatter,
                    "bundle native payload has no direct-link binding"
                )
            }
            Self::ExtraExecutableBinding => {
                write!(
                    formatter,
                    "direct-link binding is outside the bundle native closure"
                )
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

fn reject_duplicate_digest<T: Ord>(
    values: impl IntoIterator<Item = T>,
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

fn container_identity(container: &ArtifactContainerV1) -> DirectLinkContainerIdentityV1 {
    DirectLinkContainerIdentityV1::new(
        DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM.calculate(&container.to_bytes()),
    )
}

fn bundle_identity(bundle: &BundleIndexV1) -> DirectLinkBundleIndexIdentityV1 {
    DirectLinkBundleIndexIdentityV1::new(
        DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM.calculate(&bundle.to_bytes()),
    )
}

fn validate_binding_source(
    container: &ArtifactContainerV1,
    expectation: &DirectLinkBindingExpectationV1,
) -> Result<(), DirectLinkEvidenceError> {
    let finalized = expectation.finalized_payload_identity().digest();
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

    Ok(())
}

fn validate_complete_bundle_closure(
    bundle: &BundleIndexV1,
    containers: &[&ArtifactContainerV1],
    bindings: &[DirectLinkBindingV1],
) -> Result<(), DirectLinkEvidenceError> {
    let indexes = containers
        .iter()
        .map(|container| BundleIndexV1::from_containers(std::slice::from_ref(*container)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| DirectLinkEvidenceError::ContainerBundleMismatch { field: "closure" })?;
    let mut payloads = indexes
        .iter()
        .flat_map(|index| index.payloads().iter().cloned())
        .collect::<Vec<_>>();
    payloads.sort_unstable_by_key(|payload| payload.digest());
    for pair in payloads.windows(2) {
        if pair[0].digest() == pair[1].digest() && pair[0] != pair[1] {
            return Err(DirectLinkEvidenceError::ContainerBundleMismatch {
                field: "payload reference",
            });
        }
    }
    payloads.dedup();
    let derived = BundleIndexV1::new(
        indexes
            .iter()
            .flat_map(|index| index.target_associations().iter().cloned())
            .collect(),
        payloads,
        indexes
            .iter()
            .flat_map(|index| index.kernels().iter().cloned())
            .collect(),
    )
    .map_err(|_| DirectLinkEvidenceError::ContainerBundleMismatch { field: "closure" })?;
    if derived != *bundle {
        return if derived.target_associations().len() < bundle.target_associations().len() {
            Err(DirectLinkEvidenceError::MissingContainer)
        } else if derived.target_associations().len() > bundle.target_associations().len() {
            Err(DirectLinkEvidenceError::ExtraContainer)
        } else {
            Err(DirectLinkEvidenceError::ContainerBundleMismatch {
                field: "complete closure",
            })
        };
    }

    let mut expected_payloads = containers
        .iter()
        .flat_map(|container| {
            container
                .manifest()
                .code_objects()
                .iter()
                .filter(|object| object.format() == CodeObjectFormat::NativeExecutable)
                .map(|object| {
                    DirectLinkFinalizedPayloadIdentityV1::new(PayloadDigest::new(
                        container.digest_algorithm(),
                        object.digest(),
                    ))
                })
        })
        .collect::<Vec<_>>();
    expected_payloads.sort_unstable();
    expected_payloads.dedup();

    let mut actual_payloads = bindings
        .iter()
        .map(|binding| binding.expectation.finalized_payload_identity())
        .collect::<Vec<_>>();
    actual_payloads.sort_unstable();
    if expected_payloads
        .iter()
        .any(|identity| actual_payloads.binary_search(identity).is_err())
    {
        return Err(DirectLinkEvidenceError::MissingExecutableBinding);
    }
    if actual_payloads
        .iter()
        .any(|identity| expected_payloads.binary_search(identity).is_err())
    {
        return Err(DirectLinkEvidenceError::ExtraExecutableBinding);
    }
    Ok(())
}
