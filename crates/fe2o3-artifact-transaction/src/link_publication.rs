use crate::BuildAttempt;
use std::collections::BTreeMap;
use std::fmt;

const RECORD_MAGIC: &[u8] = b"FE2O3-LINK-PUBLICATION\0";
const RECORD_VERSION: u16 = 1;
const IDENTITY_BYTES: usize = 32;

const PACKAGE_TAG: u8 = 0x10;
const KERNEL_SET_TAG: u8 = 0x11;
const TARGET_TAG: u8 = 0x12;
const REQUEST_TAG: u8 = 0x20;
const WORKER_TAG: u8 = 0x21;
const RESPONSE_TAG: u8 = 0x22;
const LINKED_OUTPUT_TAG: u8 = 0x23;
const FINALIZATION_TAG: u8 = 0x24;
const FINALIZED_OUTPUT_TAG: u8 = 0x25;
const PUBLICATION_TAG: u8 = 0x26;

/// Maximum canonical size accepted for one V1 link-publication record.
pub const MAX_LINK_PUBLICATION_RECORD_BYTES: usize = 512;

/// Maximum number of publication scopes retained by one inert catalog.
pub const MAX_LINK_PUBLICATION_SCOPES: usize = 4096;

macro_rules! identity_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; IDENTITY_BYTES]);

        impl $name {
            /// Constructs an identity from its exact 256-bit representation.
            pub const fn from_bytes(bytes: [u8; IDENTITY_BYTES]) -> Self {
                Self(bytes)
            }

            /// Returns the exact 256-bit identity representation.
            pub const fn as_bytes(&self) -> &[u8; IDENTITY_BYTES] {
                &self.0
            }
        }
    };
}

identity_type!(
    /// Canonical identity of the package and output namespace owning a link.
    PackageIdentityV1
);
identity_type!(
    /// Canonical identity of the complete kernel set published as one unit.
    KernelSetIdentityV1
);
identity_type!(
    /// Canonical identity of the exact AMDGPU target and code-object domain.
    TargetIdentityV1
);
identity_type!(
    /// Canonical identity of the closed direct-link request.
    CanonicalLinkRequestIdentityV1
);
identity_type!(
    /// Content identity of the descriptor-pinned worker executable and toolchain closure.
    PinnedWorkerIdentityV1
);
identity_type!(
    /// Canonical identity of a response validated against its exact request and worker.
    ValidatedResponseIdentityV1
);
identity_type!(
    /// Content identity of the inert linked bytes returned by the worker.
    LinkedOutputIdentityV1
);
identity_type!(
    /// Canonical identity of finalization and inspection evidence.
    FinalizationIdentityV1
);
identity_type!(
    /// Content identity of finalized, inspected code-object bytes.
    FinalizedOutputIdentityV1
);
identity_type!(
    /// Canonical identity of one atomic publication commit.
    AtomicPublicationIdentityV1
);

/// Exact package, kernel-set, and target domain updated by one publication.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LinkPublicationScopeV1 {
    package: PackageIdentityV1,
    kernel_set: KernelSetIdentityV1,
    target: TargetIdentityV1,
}

impl LinkPublicationScopeV1 {
    /// Constructs an exact publication scope.
    pub const fn new(
        package: PackageIdentityV1,
        kernel_set: KernelSetIdentityV1,
        target: TargetIdentityV1,
    ) -> Self {
        Self {
            package,
            kernel_set,
            target,
        }
    }

    /// Returns the package identity.
    pub const fn package(self) -> PackageIdentityV1 {
        self.package
    }

    /// Returns the complete kernel-set identity.
    pub const fn kernel_set(self) -> KernelSetIdentityV1 {
        self.kernel_set
    }

    /// Returns the exact target identity.
    pub const fn target(self) -> TargetIdentityV1 {
        self.target
    }
}

/// Ordered durable milestone reached by a link-publication attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkPublicationPhaseV1 {
    /// A canonical request is bound to an exact build attempt and scope.
    RequestBound,
    /// The exact worker identity is pinned for that request.
    WorkerPinned,
    /// A response and linked output are validated against the request and worker.
    ResponseValidated,
    /// The linked output is finalized and inspected.
    Finalized,
    /// The finalized output is atomically published.
    Published,
}

impl LinkPublicationPhaseV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::RequestBound => 1,
            Self::WorkerPinned => 2,
            Self::ResponseValidated => 3,
            Self::Finalized => 4,
            Self::Published => 5,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, LinkPublicationCodecError> {
        match tag {
            1 => Ok(Self::RequestBound),
            2 => Ok(Self::WorkerPinned),
            3 => Ok(Self::ResponseValidated),
            4 => Ok(Self::Finalized),
            5 => Ok(Self::Published),
            actual => Err(LinkPublicationCodecError::InvalidPhase { actual }),
        }
    }
}

/// Fail-closed reason terminating a link-publication record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidationReasonV1 {
    /// A caller reported a link, validation, finalization, or publication failure.
    ExplicitFailure,
    /// Restart recovery found an incomplete attempt.
    CrashRecovery,
    /// The record no longer owns the active scope generation.
    StaleAttempt,
    /// Published state and the atomic publication catalog disagreed.
    CorruptPublication,
}

impl InvalidationReasonV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::ExplicitFailure => 1,
            Self::CrashRecovery => 2,
            Self::StaleAttempt => 3,
            Self::CorruptPublication => 4,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, LinkPublicationCodecError> {
        match tag {
            1 => Ok(Self::ExplicitFailure),
            2 => Ok(Self::CrashRecovery),
            3 => Ok(Self::StaleAttempt),
            4 => Ok(Self::CorruptPublication),
            actual => Err(LinkPublicationCodecError::InvalidInvalidationReason { actual }),
        }
    }
}

/// Durable state of one V1 link-publication record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkPublicationStateV1 {
    /// The record is active at the named ordered milestone.
    Active(LinkPublicationPhaseV1),
    /// The record is terminal and grants no publication authority.
    Invalidated {
        /// Last structurally valid milestone retained for diagnostics.
        prior_phase: LinkPublicationPhaseV1,
        /// Reason the attempt was invalidated.
        reason: InvalidationReasonV1,
    },
}

impl LinkPublicationStateV1 {
    const fn evidence_phase(self) -> LinkPublicationPhaseV1 {
        match self {
            Self::Active(phase)
            | Self::Invalidated {
                prior_phase: phase, ..
            } => phase,
        }
    }
}

/// Identity field involved in a rejected transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityKindV1 {
    /// Canonical request identity.
    Request,
    /// Pinned worker identity.
    Worker,
    /// Validated response identity.
    Response,
    /// Linked output identity.
    LinkedOutput,
    /// Finalization identity.
    Finalization,
    /// Finalized output identity.
    FinalizedOutput,
    /// Atomic publication identity.
    Publication,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LinkEvidenceV1 {
    request: CanonicalLinkRequestIdentityV1,
    worker: Option<PinnedWorkerIdentityV1>,
    response: Option<ValidatedResponseIdentityV1>,
    linked_output: Option<LinkedOutputIdentityV1>,
    finalization: Option<FinalizationIdentityV1>,
    finalized_output: Option<FinalizedOutputIdentityV1>,
    publication: Option<AtomicPublicationIdentityV1>,
}

/// Versioned inert record for one ordered direct-link publication attempt.
///
/// This record authenticates no bytes by itself and grants no loading or launch authority. Its
/// typed identities are populated only by callers that already validated the corresponding G1-G3
/// evidence. Private fields and transition methods prevent construction of a reordered state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkPublicationRecordV1 {
    attempt: BuildAttempt,
    scope: LinkPublicationScopeV1,
    state: LinkPublicationStateV1,
    evidence: LinkEvidenceV1,
}

impl LinkPublicationRecordV1 {
    /// Returns the record schema version.
    pub const fn version(&self) -> u16 {
        RECORD_VERSION
    }

    /// Returns the exact build attempt owning this record.
    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    /// Returns the exact package, kernel-set, and target publication scope.
    pub const fn scope(&self) -> LinkPublicationScopeV1 {
        self.scope
    }

    /// Returns the current durable state.
    pub const fn state(&self) -> LinkPublicationStateV1 {
        self.state
    }

    /// Returns the request identity, present in every valid record.
    pub const fn request(&self) -> CanonicalLinkRequestIdentityV1 {
        self.evidence.request
    }

    /// Returns the pinned worker identity once recorded.
    pub const fn worker(&self) -> Option<PinnedWorkerIdentityV1> {
        self.evidence.worker
    }

    /// Returns the validated response identity once recorded.
    pub const fn response(&self) -> Option<ValidatedResponseIdentityV1> {
        self.evidence.response
    }

    /// Returns the linked output identity once recorded.
    pub const fn linked_output(&self) -> Option<LinkedOutputIdentityV1> {
        self.evidence.linked_output
    }

    /// Returns the finalization identity once recorded.
    pub const fn finalization(&self) -> Option<FinalizationIdentityV1> {
        self.evidence.finalization
    }

    /// Returns the finalized output identity once recorded.
    pub const fn finalized_output(&self) -> Option<FinalizedOutputIdentityV1> {
        self.evidence.finalized_output
    }

    /// Returns the atomic publication identity once committed.
    pub const fn publication(&self) -> Option<AtomicPublicationIdentityV1> {
        self.evidence.publication
    }

    /// Records the exact worker pinned for the canonical request.
    pub fn record_pinned_worker(
        &mut self,
        catalog: &LinkPublicationCatalogV1,
        attempt: BuildAttempt,
        request: CanonicalLinkRequestIdentityV1,
        worker: PinnedWorkerIdentityV1,
    ) -> Result<(), LinkPublicationCodecError> {
        self.authorize_transition(catalog, attempt, LinkPublicationPhaseV1::RequestBound)?;
        check_identity(self.evidence.request == request, IdentityKindV1::Request)?;
        self.evidence.worker = Some(worker);
        self.state = LinkPublicationStateV1::Active(LinkPublicationPhaseV1::WorkerPinned);
        Ok(())
    }

    /// Records a response and output validated against the exact request and worker.
    pub fn record_validated_response(
        &mut self,
        catalog: &LinkPublicationCatalogV1,
        attempt: BuildAttempt,
        request: CanonicalLinkRequestIdentityV1,
        worker: PinnedWorkerIdentityV1,
        response: ValidatedResponseIdentityV1,
        linked_output: LinkedOutputIdentityV1,
    ) -> Result<(), LinkPublicationCodecError> {
        self.authorize_transition(catalog, attempt, LinkPublicationPhaseV1::WorkerPinned)?;
        check_identity(self.evidence.request == request, IdentityKindV1::Request)?;
        check_identity(self.evidence.worker == Some(worker), IdentityKindV1::Worker)?;
        self.evidence.response = Some(response);
        self.evidence.linked_output = Some(linked_output);
        self.state = LinkPublicationStateV1::Active(LinkPublicationPhaseV1::ResponseValidated);
        Ok(())
    }

    /// Records finalization and inspection of the exact validated linked output.
    pub fn record_finalization(
        &mut self,
        catalog: &LinkPublicationCatalogV1,
        attempt: BuildAttempt,
        response: ValidatedResponseIdentityV1,
        linked_output: LinkedOutputIdentityV1,
        finalization: FinalizationIdentityV1,
        finalized_output: FinalizedOutputIdentityV1,
    ) -> Result<(), LinkPublicationCodecError> {
        self.authorize_transition(catalog, attempt, LinkPublicationPhaseV1::ResponseValidated)?;
        check_identity(
            self.evidence.response == Some(response),
            IdentityKindV1::Response,
        )?;
        check_identity(
            self.evidence.linked_output == Some(linked_output),
            IdentityKindV1::LinkedOutput,
        )?;
        self.evidence.finalization = Some(finalization);
        self.evidence.finalized_output = Some(finalized_output);
        self.state = LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Finalized);
        Ok(())
    }

    /// Atomically updates this record and the inert publication catalog.
    ///
    /// Exact replay after a successful commit is idempotent. Any changed identity, stale attempt,
    /// or catalog disagreement fails without changing either value.
    pub fn publish(
        &mut self,
        catalog: &mut LinkPublicationCatalogV1,
        attempt: BuildAttempt,
        finalization: FinalizationIdentityV1,
        finalized_output: FinalizedOutputIdentityV1,
        publication: AtomicPublicationIdentityV1,
    ) -> Result<PublicationOutcomeV1, LinkPublicationCodecError> {
        if self.attempt != attempt {
            return Err(LinkPublicationCodecError::AttemptMismatch);
        }
        check_identity(
            self.evidence.finalization == Some(finalization),
            IdentityKindV1::Finalization,
        )?;
        check_identity(
            self.evidence.finalized_output == Some(finalized_output),
            IdentityKindV1::FinalizedOutput,
        )?;

        if self.state == LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published) {
            check_identity(
                self.evidence.publication == Some(publication),
                IdentityKindV1::Publication,
            )?;
            let expected = self.published_artifact()?;
            if catalog.published(&self.scope) != Some(&expected) {
                return Err(LinkPublicationCodecError::CatalogMismatch);
            }
            return Ok(PublicationOutcomeV1::AlreadyPublished);
        }

        catalog.authorize(self, attempt)?;
        self.expect_active(LinkPublicationPhaseV1::Finalized)?;
        let mut next_record = self.clone();
        next_record.evidence.publication = Some(publication);
        next_record.state = LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published);
        next_record.validate()?;
        let artifact = next_record.published_artifact()?;

        let mut next_catalog = catalog.clone();
        next_catalog.commit(artifact)?;
        *catalog = next_catalog;
        *self = next_record;
        Ok(PublicationOutcomeV1::Published)
    }

    /// Invalidates an active, unpublished record and removes only catalog state owned by it.
    pub fn invalidate(
        &mut self,
        catalog: &mut LinkPublicationCatalogV1,
        attempt: BuildAttempt,
        reason: InvalidationReasonV1,
    ) -> Result<(), LinkPublicationCodecError> {
        if self.attempt != attempt {
            return Err(LinkPublicationCodecError::AttemptMismatch);
        }
        if let LinkPublicationStateV1::Invalidated {
            reason: existing, ..
        } = self.state
        {
            return if existing == reason {
                Ok(())
            } else {
                Err(LinkPublicationCodecError::InvalidTransition {
                    expected: self.state.evidence_phase(),
                    actual: self.state,
                })
            };
        }
        catalog.authorize(self, attempt)?;
        let prior_phase = match self.state {
            LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published) => {
                return Err(LinkPublicationCodecError::InvalidTransition {
                    expected: LinkPublicationPhaseV1::Finalized,
                    actual: self.state,
                });
            }
            LinkPublicationStateV1::Active(phase) => phase,
            LinkPublicationStateV1::Invalidated { .. } => unreachable!("handled above"),
        };

        let mut next_catalog = catalog.clone();
        next_catalog.remove_owned(self);
        next_catalog.deactivate_owned(self);
        self.state = LinkPublicationStateV1::Invalidated {
            prior_phase,
            reason,
        };
        *catalog = next_catalog;
        Ok(())
    }

    /// Reconciles a decoded record after restart.
    ///
    /// Incomplete or stale records are invalidated and only publication state owned by their exact
    /// scope, attempt, request, and complete publication chain is removed. A published record
    /// survives only when the catalog contains the exact same identity chain.
    pub fn recover(
        &mut self,
        catalog: &mut LinkPublicationCatalogV1,
    ) -> Result<RecoveryOutcomeV1, LinkPublicationCodecError> {
        self.validate()?;
        if matches!(self.state, LinkPublicationStateV1::Invalidated { .. }) {
            catalog.remove_owned(self);
            catalog.deactivate_owned(self);
            return Ok(RecoveryOutcomeV1::AlreadyInvalidated);
        }

        if self.state == LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published) {
            let expected = self.published_artifact()?;
            if catalog.published(&self.scope) == Some(&expected) {
                catalog.deactivate_owned(self);
                return Ok(RecoveryOutcomeV1::PublicationConfirmed);
            }
            if catalog.is_current(self.scope, self.attempt, self.evidence.request) {
                catalog.remove_owned(self);
                catalog.deactivate_owned(self);
                self.state = LinkPublicationStateV1::Invalidated {
                    prior_phase: LinkPublicationPhaseV1::Published,
                    reason: InvalidationReasonV1::CorruptPublication,
                };
                return Ok(RecoveryOutcomeV1::InvalidatedCorruptPublication);
            }
        }

        if !catalog.is_current(self.scope, self.attempt, self.evidence.request) {
            let prior_phase = self.state.evidence_phase();
            catalog.remove_owned(self);
            self.state = LinkPublicationStateV1::Invalidated {
                prior_phase,
                reason: InvalidationReasonV1::StaleAttempt,
            };
            return Ok(RecoveryOutcomeV1::InvalidatedStaleAttempt);
        }

        match self.state {
            LinkPublicationStateV1::Active(prior_phase) => {
                catalog.remove_owned(self);
                catalog.deactivate_owned(self);
                self.state = LinkPublicationStateV1::Invalidated {
                    prior_phase,
                    reason: InvalidationReasonV1::CrashRecovery,
                };
                Ok(RecoveryOutcomeV1::InvalidatedIncomplete)
            }
            LinkPublicationStateV1::Invalidated { .. } => unreachable!("handled above"),
        }
    }

    /// Encodes a canonical, bounded V1 record for durable restart recovery.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, LinkPublicationCodecError> {
        self.validate()?;
        let mut bytes = Vec::with_capacity(MAX_LINK_PUBLICATION_RECORD_BYTES);
        bytes.extend_from_slice(RECORD_MAGIC);
        bytes.extend_from_slice(&RECORD_VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.attempt.generation().to_le_bytes());
        bytes.extend_from_slice(self.attempt.session().as_bytes());
        bytes.extend_from_slice(self.attempt.invocation().as_bytes());
        push_identity(&mut bytes, PACKAGE_TAG, self.scope.package.as_bytes());
        push_identity(&mut bytes, KERNEL_SET_TAG, self.scope.kernel_set.as_bytes());
        push_identity(&mut bytes, TARGET_TAG, self.scope.target.as_bytes());
        match self.state {
            LinkPublicationStateV1::Active(phase) => {
                bytes.push(phase.tag());
            }
            LinkPublicationStateV1::Invalidated {
                prior_phase,
                reason,
            } => {
                bytes.push(0xff);
                bytes.push(prior_phase.tag());
                bytes.push(reason.tag());
            }
        }
        encode_evidence(&mut bytes, &self.evidence, self.state.evidence_phase())?;
        if bytes.len() > MAX_LINK_PUBLICATION_RECORD_BYTES {
            return Err(LinkPublicationCodecError::RecordTooLarge);
        }
        Ok(bytes)
    }

    /// Decodes a canonical, bounded V1 record.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, LinkPublicationCodecError> {
        if bytes.len() > MAX_LINK_PUBLICATION_RECORD_BYTES {
            return Err(LinkPublicationCodecError::RecordTooLarge);
        }
        let mut decoder = Decoder::new(bytes);
        if decoder.take(RECORD_MAGIC.len())? != RECORD_MAGIC {
            return Err(LinkPublicationCodecError::BadMagic);
        }
        let version = decoder.u16()?;
        if version != RECORD_VERSION {
            return Err(LinkPublicationCodecError::UnsupportedVersion { actual: version });
        }
        let generation = decoder.u64()?;
        let session: [u8; 16] = copy_array(decoder.take(16)?);
        let invocation: [u8; 32] = copy_array(decoder.take(32)?);
        let attempt_text = format!(
            "{generation}:{}:{}",
            encode_hex(&session),
            encode_hex(&invocation)
        );
        let attempt = BuildAttempt::from_env_value(&attempt_text)
            .map_err(|_| LinkPublicationCodecError::InvalidAttempt)?;

        let package = PackageIdentityV1::from_bytes(decoder.identity(PACKAGE_TAG)?);
        let kernel_set = KernelSetIdentityV1::from_bytes(decoder.identity(KERNEL_SET_TAG)?);
        let target = TargetIdentityV1::from_bytes(decoder.identity(TARGET_TAG)?);
        let scope = LinkPublicationScopeV1::new(package, kernel_set, target);
        let state_tag = decoder.byte()?;
        let state = if state_tag == 0xff {
            LinkPublicationStateV1::Invalidated {
                prior_phase: LinkPublicationPhaseV1::from_tag(decoder.byte()?)?,
                reason: InvalidationReasonV1::from_tag(decoder.byte()?)?,
            }
        } else {
            LinkPublicationStateV1::Active(LinkPublicationPhaseV1::from_tag(state_tag)?)
        };
        let evidence = decode_evidence(&mut decoder, state.evidence_phase())?;
        if !decoder.is_finished() {
            return Err(LinkPublicationCodecError::TrailingBytes);
        }
        let record = Self {
            attempt,
            scope,
            state,
            evidence,
        };
        record.validate()?;
        if record.encode_canonical()? != bytes {
            return Err(LinkPublicationCodecError::NonCanonical);
        }
        Ok(record)
    }

    fn authorize_transition(
        &self,
        catalog: &LinkPublicationCatalogV1,
        attempt: BuildAttempt,
        expected: LinkPublicationPhaseV1,
    ) -> Result<(), LinkPublicationCodecError> {
        catalog.authorize(self, attempt)?;
        self.expect_active(expected)
    }

    fn expect_active(
        &self,
        expected: LinkPublicationPhaseV1,
    ) -> Result<(), LinkPublicationCodecError> {
        if self.state != LinkPublicationStateV1::Active(expected) {
            return Err(LinkPublicationCodecError::InvalidTransition {
                expected,
                actual: self.state,
            });
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), LinkPublicationCodecError> {
        let phase = self.state.evidence_phase();
        let worker = phase_at_least(phase, LinkPublicationPhaseV1::WorkerPinned);
        let response = phase_at_least(phase, LinkPublicationPhaseV1::ResponseValidated);
        let finalized = phase_at_least(phase, LinkPublicationPhaseV1::Finalized);
        let published = phase_at_least(phase, LinkPublicationPhaseV1::Published);
        if self.evidence.worker.is_some() != worker
            || self.evidence.response.is_some() != response
            || self.evidence.linked_output.is_some() != response
            || self.evidence.finalization.is_some() != finalized
            || self.evidence.finalized_output.is_some() != finalized
            || self.evidence.publication.is_some() != published
        {
            return Err(LinkPublicationCodecError::InvalidEvidenceShape);
        }
        Ok(())
    }

    fn published_artifact(&self) -> Result<PublishedLinkArtifactV1, LinkPublicationCodecError> {
        self.validate()?;
        if self.state.evidence_phase() != LinkPublicationPhaseV1::Published {
            return Err(LinkPublicationCodecError::InvalidTransition {
                expected: LinkPublicationPhaseV1::Published,
                actual: self.state,
            });
        }
        Ok(PublishedLinkArtifactV1 {
            attempt: self.attempt,
            scope: self.scope,
            request: self.evidence.request,
            worker: self.evidence.worker.expect("validated published worker"),
            response: self
                .evidence
                .response
                .expect("validated published response"),
            linked_output: self
                .evidence
                .linked_output
                .expect("validated published linked output"),
            finalization: self
                .evidence
                .finalization
                .expect("validated published finalization"),
            finalized_output: self
                .evidence
                .finalized_output
                .expect("validated published finalized output"),
            publication: self
                .evidence
                .publication
                .expect("validated published commit"),
        })
    }
}

/// Inert identity chain retained for one atomically published scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublishedLinkArtifactV1 {
    attempt: BuildAttempt,
    scope: LinkPublicationScopeV1,
    request: CanonicalLinkRequestIdentityV1,
    worker: PinnedWorkerIdentityV1,
    response: ValidatedResponseIdentityV1,
    linked_output: LinkedOutputIdentityV1,
    finalization: FinalizationIdentityV1,
    finalized_output: FinalizedOutputIdentityV1,
    publication: AtomicPublicationIdentityV1,
}

impl PublishedLinkArtifactV1 {
    /// Returns the exact owning attempt.
    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }

    /// Returns the exact publication scope.
    pub const fn scope(self) -> LinkPublicationScopeV1 {
        self.scope
    }

    /// Returns the canonical request identity.
    pub const fn request(self) -> CanonicalLinkRequestIdentityV1 {
        self.request
    }

    /// Returns the pinned worker identity.
    pub const fn worker(self) -> PinnedWorkerIdentityV1 {
        self.worker
    }

    /// Returns the validated response identity.
    pub const fn response(self) -> ValidatedResponseIdentityV1 {
        self.response
    }

    /// Returns the linked output identity.
    pub const fn linked_output(self) -> LinkedOutputIdentityV1 {
        self.linked_output
    }

    /// Returns the finalization identity.
    pub const fn finalization(self) -> FinalizationIdentityV1 {
        self.finalization
    }

    /// Returns the finalized output identity.
    pub const fn finalized_output(self) -> FinalizedOutputIdentityV1 {
        self.finalized_output
    }

    /// Returns the atomic publication identity.
    pub const fn publication(self) -> AtomicPublicationIdentityV1 {
        self.publication
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveAttemptV1 {
    attempt: BuildAttempt,
    request: CanonicalLinkRequestIdentityV1,
}

/// Bounded inert model of active link attempts and atomically published identities.
///
/// The catalog performs no I/O. It models the ownership checks that a later filesystem adapter
/// must hold under the existing artifact lock.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LinkPublicationCatalogV1 {
    active: BTreeMap<LinkPublicationScopeV1, ActiveAttemptV1>,
    published: BTreeMap<LinkPublicationScopeV1, PublishedLinkArtifactV1>,
}

impl LinkPublicationCatalogV1 {
    /// Starts or idempotently reopens an exact request record for a scope.
    ///
    /// A higher build generation supersedes active authority without deleting the last complete
    /// publication. Older generations cannot regain authority.
    pub fn begin(
        &mut self,
        attempt: BuildAttempt,
        scope: LinkPublicationScopeV1,
        request: CanonicalLinkRequestIdentityV1,
    ) -> Result<LinkPublicationRecordV1, LinkPublicationCodecError> {
        match self.active.get(&scope) {
            Some(active) if active.attempt == attempt && active.request == request => {}
            Some(active) if active.attempt == attempt => {
                return Err(LinkPublicationCodecError::IdentityMismatch {
                    kind: IdentityKindV1::Request,
                });
            }
            Some(active) if active.attempt.generation() >= attempt.generation() => {
                return Err(LinkPublicationCodecError::StaleAttempt);
            }
            Some(_) => {
                self.active
                    .insert(scope, ActiveAttemptV1 { attempt, request });
            }
            None => {
                if self.scope_count() == MAX_LINK_PUBLICATION_SCOPES {
                    return Err(LinkPublicationCodecError::TooManyScopes);
                }
                self.active
                    .insert(scope, ActiveAttemptV1 { attempt, request });
            }
        }

        Ok(LinkPublicationRecordV1 {
            attempt,
            scope,
            state: LinkPublicationStateV1::Active(LinkPublicationPhaseV1::RequestBound),
            evidence: LinkEvidenceV1 {
                request,
                worker: None,
                response: None,
                linked_output: None,
                finalization: None,
                finalized_output: None,
                publication: None,
            },
        })
    }

    /// Returns the active attempt for a scope, if any.
    pub fn active_attempt(&self, scope: &LinkPublicationScopeV1) -> Option<BuildAttempt> {
        self.active.get(scope).map(|active| active.attempt)
    }

    /// Returns the last complete publication for a scope, if any.
    pub fn published(&self, scope: &LinkPublicationScopeV1) -> Option<&PublishedLinkArtifactV1> {
        self.published.get(scope)
    }

    /// Returns the number of distinct active or published scopes.
    pub fn scope_count(&self) -> usize {
        self.active
            .keys()
            .chain(self.published.keys())
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    }

    /// Returns whether no active or published scope exists.
    pub fn is_empty(&self) -> bool {
        self.active.is_empty() && self.published.is_empty()
    }

    fn authorize(
        &self,
        record: &LinkPublicationRecordV1,
        attempt: BuildAttempt,
    ) -> Result<(), LinkPublicationCodecError> {
        if record.attempt != attempt {
            return Err(LinkPublicationCodecError::AttemptMismatch);
        }
        if !self.is_current(record.scope, attempt, record.evidence.request) {
            return Err(LinkPublicationCodecError::StaleAttempt);
        }
        Ok(())
    }

    fn is_current(
        &self,
        scope: LinkPublicationScopeV1,
        attempt: BuildAttempt,
        request: CanonicalLinkRequestIdentityV1,
    ) -> bool {
        self.active.get(&scope) == Some(&ActiveAttemptV1 { attempt, request })
    }

    fn commit(
        &mut self,
        artifact: PublishedLinkArtifactV1,
    ) -> Result<(), LinkPublicationCodecError> {
        if !self.is_current(artifact.scope, artifact.attempt, artifact.request) {
            return Err(LinkPublicationCodecError::StaleAttempt);
        }
        if self
            .published
            .get(&artifact.scope)
            .is_some_and(|existing| existing.attempt == artifact.attempt)
        {
            return Err(LinkPublicationCodecError::CatalogMismatch);
        }
        self.published.insert(artifact.scope, artifact);
        Ok(())
    }

    fn remove_owned(&mut self, record: &LinkPublicationRecordV1) {
        let Ok(expected) = record.published_artifact() else {
            return;
        };
        if self.published.get(&record.scope) == Some(&expected) {
            self.published.remove(&record.scope);
        }
    }

    fn deactivate(
        &mut self,
        scope: LinkPublicationScopeV1,
        attempt: BuildAttempt,
        request: CanonicalLinkRequestIdentityV1,
    ) {
        if self
            .active
            .get(&scope)
            .is_some_and(|active| *active == ActiveAttemptV1 { attempt, request })
        {
            self.active.remove(&scope);
        }
    }

    fn deactivate_owned(&mut self, record: &LinkPublicationRecordV1) {
        if self.published.get(&record.scope).is_some_and(|published| {
            published.attempt == record.attempt
                && published.request == record.evidence.request
                && record.published_artifact().ok().as_ref() != Some(published)
        }) {
            return;
        }
        self.deactivate(record.scope, record.attempt, record.evidence.request);
    }
}

/// Result of an atomic publication request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationOutcomeV1 {
    /// The identity chain was committed once.
    Published,
    /// An exact replay observed the already committed identity chain.
    AlreadyPublished,
}

/// Deterministic result of restart recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryOutcomeV1 {
    /// An incomplete active record was invalidated.
    InvalidatedIncomplete,
    /// A superseded record was invalidated without touching newer publication state.
    InvalidatedStaleAttempt,
    /// Published state and catalog identity agreed exactly.
    PublicationConfirmed,
    /// Published state disagreed with the catalog and the conflicting record was invalidated.
    /// Catalog publication is retained unless the record identifies its complete chain exactly.
    InvalidatedCorruptPublication,
    /// The record was already terminal and cleanup remained idempotent.
    AlreadyInvalidated,
}

/// Canonical codec, ordering, ownership, or publication-model failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkPublicationCodecError {
    /// Record magic did not identify this schema.
    BadMagic,
    /// Record version is not supported.
    UnsupportedVersion { actual: u16 },
    /// Record exceeded its canonical byte bound.
    RecordTooLarge,
    /// Input ended before a complete record was decoded.
    Truncated,
    /// Input contained bytes after one complete record.
    TrailingBytes,
    /// The build-attempt fields were not canonical.
    InvalidAttempt,
    /// A phase tag was unknown.
    InvalidPhase { actual: u8 },
    /// An invalidation-reason tag was unknown.
    InvalidInvalidationReason { actual: u8 },
    /// A domain-separated identity appeared out of order.
    InvalidIdentityTag { expected: u8, actual: u8 },
    /// Optional evidence did not form a contiguous identity chain.
    InvalidEvidenceShape,
    /// Decoding did not reproduce the same canonical bytes.
    NonCanonical,
    /// The supplied build-attempt token did not match the record.
    AttemptMismatch,
    /// A newer attempt owns the scope.
    StaleAttempt,
    /// A transition was requested from the wrong durable state.
    InvalidTransition {
        expected: LinkPublicationPhaseV1,
        actual: LinkPublicationStateV1,
    },
    /// A supplied identity did not match the previously recorded parent evidence.
    IdentityMismatch { kind: IdentityKindV1 },
    /// The bounded catalog has no room for another distinct scope.
    TooManyScopes,
    /// Record state and the atomic publication catalog disagreed.
    CatalogMismatch,
}

impl fmt::Display for LinkPublicationCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => formatter.write_str("bad link-publication record magic"),
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "unsupported link-publication version {actual}")
            }
            Self::RecordTooLarge => formatter.write_str("link-publication record is too large"),
            Self::Truncated => formatter.write_str("truncated link-publication record"),
            Self::TrailingBytes => formatter.write_str("trailing link-publication record bytes"),
            Self::InvalidAttempt => formatter.write_str("invalid build-attempt identity"),
            Self::InvalidPhase { actual } => {
                write!(formatter, "invalid link-publication phase {actual}")
            }
            Self::InvalidInvalidationReason { actual } => {
                write!(
                    formatter,
                    "invalid link-publication invalidation reason {actual}"
                )
            }
            Self::InvalidIdentityTag { expected, actual } => write!(
                formatter,
                "invalid link-publication identity tag {actual:#x}; expected {expected:#x}"
            ),
            Self::InvalidEvidenceShape => {
                formatter.write_str("non-contiguous link-publication identity chain")
            }
            Self::NonCanonical => formatter.write_str("noncanonical link-publication record"),
            Self::AttemptMismatch => formatter.write_str("build attempt does not match record"),
            Self::StaleAttempt => formatter.write_str("link-publication attempt is stale"),
            Self::InvalidTransition { expected, actual } => write!(
                formatter,
                "invalid link-publication transition from {actual:?}; expected {expected:?}"
            ),
            Self::IdentityMismatch { kind } => {
                write!(formatter, "link-publication {kind:?} identity mismatch")
            }
            Self::TooManyScopes => formatter.write_str("too many link-publication scopes"),
            Self::CatalogMismatch => {
                formatter.write_str("link-publication record and catalog disagree")
            }
        }
    }
}

impl std::error::Error for LinkPublicationCodecError {}

fn phase_at_least(actual: LinkPublicationPhaseV1, expected: LinkPublicationPhaseV1) -> bool {
    actual.tag() >= expected.tag()
}

fn check_identity(matches: bool, kind: IdentityKindV1) -> Result<(), LinkPublicationCodecError> {
    if matches {
        Ok(())
    } else {
        Err(LinkPublicationCodecError::IdentityMismatch { kind })
    }
}

fn encode_evidence(
    bytes: &mut Vec<u8>,
    evidence: &LinkEvidenceV1,
    phase: LinkPublicationPhaseV1,
) -> Result<(), LinkPublicationCodecError> {
    push_identity(bytes, REQUEST_TAG, evidence.request.as_bytes());
    if phase_at_least(phase, LinkPublicationPhaseV1::WorkerPinned) {
        push_identity(
            bytes,
            WORKER_TAG,
            evidence
                .worker
                .ok_or(LinkPublicationCodecError::InvalidEvidenceShape)?
                .as_bytes(),
        );
    }
    if phase_at_least(phase, LinkPublicationPhaseV1::ResponseValidated) {
        push_identity(
            bytes,
            RESPONSE_TAG,
            evidence
                .response
                .ok_or(LinkPublicationCodecError::InvalidEvidenceShape)?
                .as_bytes(),
        );
        push_identity(
            bytes,
            LINKED_OUTPUT_TAG,
            evidence
                .linked_output
                .ok_or(LinkPublicationCodecError::InvalidEvidenceShape)?
                .as_bytes(),
        );
    }
    if phase_at_least(phase, LinkPublicationPhaseV1::Finalized) {
        push_identity(
            bytes,
            FINALIZATION_TAG,
            evidence
                .finalization
                .ok_or(LinkPublicationCodecError::InvalidEvidenceShape)?
                .as_bytes(),
        );
        push_identity(
            bytes,
            FINALIZED_OUTPUT_TAG,
            evidence
                .finalized_output
                .ok_or(LinkPublicationCodecError::InvalidEvidenceShape)?
                .as_bytes(),
        );
    }
    if phase_at_least(phase, LinkPublicationPhaseV1::Published) {
        push_identity(
            bytes,
            PUBLICATION_TAG,
            evidence
                .publication
                .ok_or(LinkPublicationCodecError::InvalidEvidenceShape)?
                .as_bytes(),
        );
    }
    Ok(())
}

fn decode_evidence(
    decoder: &mut Decoder<'_>,
    phase: LinkPublicationPhaseV1,
) -> Result<LinkEvidenceV1, LinkPublicationCodecError> {
    let request = CanonicalLinkRequestIdentityV1::from_bytes(decoder.identity(REQUEST_TAG)?);
    let worker = phase_at_least(phase, LinkPublicationPhaseV1::WorkerPinned)
        .then(|| {
            decoder
                .identity(WORKER_TAG)
                .map(PinnedWorkerIdentityV1::from_bytes)
        })
        .transpose()?;
    let (response, linked_output) =
        if phase_at_least(phase, LinkPublicationPhaseV1::ResponseValidated) {
            (
                Some(ValidatedResponseIdentityV1::from_bytes(
                    decoder.identity(RESPONSE_TAG)?,
                )),
                Some(LinkedOutputIdentityV1::from_bytes(
                    decoder.identity(LINKED_OUTPUT_TAG)?,
                )),
            )
        } else {
            (None, None)
        };
    let (finalization, finalized_output) =
        if phase_at_least(phase, LinkPublicationPhaseV1::Finalized) {
            (
                Some(FinalizationIdentityV1::from_bytes(
                    decoder.identity(FINALIZATION_TAG)?,
                )),
                Some(FinalizedOutputIdentityV1::from_bytes(
                    decoder.identity(FINALIZED_OUTPUT_TAG)?,
                )),
            )
        } else {
            (None, None)
        };
    let publication = phase_at_least(phase, LinkPublicationPhaseV1::Published)
        .then(|| {
            decoder
                .identity(PUBLICATION_TAG)
                .map(AtomicPublicationIdentityV1::from_bytes)
        })
        .transpose()?;
    Ok(LinkEvidenceV1 {
        request,
        worker,
        response,
        linked_output,
        finalization,
        finalized_output,
        publication,
    })
}

fn push_identity(bytes: &mut Vec<u8>, tag: u8, identity: &[u8; IDENTITY_BYTES]) {
    bytes.push(tag);
    bytes.extend_from_slice(identity);
}

fn copy_array<const N: usize>(bytes: &[u8]) -> [u8; N] {
    let mut result = [0; N];
    result.copy_from_slice(bytes);
    result
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], LinkPublicationCodecError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(LinkPublicationCodecError::Truncated)?;
        let result = self
            .bytes
            .get(self.offset..end)
            .ok_or(LinkPublicationCodecError::Truncated)?;
        self.offset = end;
        Ok(result)
    }

    fn byte(&mut self) -> Result<u8, LinkPublicationCodecError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, LinkPublicationCodecError> {
        Ok(u16::from_le_bytes(copy_array(self.take(2)?)))
    }

    fn u64(&mut self) -> Result<u64, LinkPublicationCodecError> {
        Ok(u64::from_le_bytes(copy_array(self.take(8)?)))
    }

    fn identity(&mut self, expected: u8) -> Result<[u8; 32], LinkPublicationCodecError> {
        let actual = self.byte()?;
        if actual != expected {
            return Err(LinkPublicationCodecError::InvalidIdentityTag { expected, actual });
        }
        Ok(copy_array(self.take(IDENTITY_BYTES)?))
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
