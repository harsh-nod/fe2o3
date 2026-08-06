use std::fmt;

use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BuildAttempt, CanonicalLinkRequestIdentityV1,
    FinalizationIdentityV1, FinalizedOutputIdentityV1, LinkPublicationScopeV1,
    LinkedOutputIdentityV1, PinnedWorkerIdentityV1, PublishedLinkArtifactV1,
    ValidatedResponseIdentityV1,
};

use crate::{
    DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM, DigestAlgorithm, DirectLinkBindingV1,
    DirectLinkBundleEvidenceV1, DirectLinkToolchainIdentityV1, DirectLinkWorkerIdentityV1,
    PayloadDigest,
};

const WORKER_CLOSURE_DOMAIN: &[u8] = b"fe2o3.direct-link.worker-closure.v1\0";
const PUBLICATION_DOMAIN: &[u8] = b"fe2o3.direct-link.publication-bridge.v1\0";

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

/// Failure to construct or validate a typed G5/G6 publication bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DirectLinkBridgeError {
    BindingOutsideEvidence,
    UnsupportedDigestAlgorithm {
        field: &'static str,
    },
    IdentityMismatch {
        kind: DirectLinkBridgeIdentityKindV1,
    },
}

impl fmt::Display for DirectLinkBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindingOutsideEvidence => {
                formatter.write_str("direct-link binding is absent from its evidence record")
            }
            Self::UnsupportedDigestAlgorithm { field } => {
                write!(
                    formatter,
                    "{field} does not use the G5 SHA-256 identity domain"
                )
            }
            Self::IdentityMismatch { kind } => {
                write!(formatter, "G5/G6 {kind:?} identity mismatch")
            }
        }
    }
}

impl std::error::Error for DirectLinkBridgeError {}

/// Typed, inert conversion between one G6 binding and one G5 publication chain.
///
/// `prepare` verifies that the binding belongs to the named evidence envelope. The returned
/// values are the only normative conversions into G5 identity domains: direct SHA-256 identities
/// are converted field by field, while worker/toolchain and publication identities are derived
/// from domain-separated canonical preimages. The publication preimage commits to the attempt,
/// scope, request, worker and toolchain measurements, response, transformation, FFI closure,
/// container, bundle, and complete direct-link evidence envelope.
///
/// This model performs no filesystem I/O, does not authenticate caller-supplied measurements, and
/// grants no authority to load or launch code. Filesystem publication and durable recovery remain
/// the responsibility of a future adapter under the artifact transaction lock.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectLinkPublicationBridgeV1 {
    attempt: BuildAttempt,
    scope: LinkPublicationScopeV1,
    bundle: DirectLinkBundleEvidenceV1,
    binding: DirectLinkBindingV1,
}

impl DirectLinkPublicationBridgeV1 {
    /// Prepares an exact bridge for a binding present in `bundle`.
    pub fn prepare(
        attempt: BuildAttempt,
        scope: LinkPublicationScopeV1,
        bundle: &DirectLinkBundleEvidenceV1,
        binding: &DirectLinkBindingV1,
    ) -> Result<Self, DirectLinkBridgeError> {
        if !bundle.bindings().contains(binding) {
            return Err(DirectLinkBridgeError::BindingOutsideEvidence);
        }
        let bridge = Self {
            attempt,
            scope,
            bundle: bundle.clone(),
            binding: binding.clone(),
        };
        bridge.require_sha256_domains()?;
        Ok(bridge)
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub const fn scope(&self) -> LinkPublicationScopeV1 {
        self.scope
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
        AtomicPublicationIdentityV1::from_bytes(calculate_identity(PUBLICATION_DOMAIN, |bytes| {
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
