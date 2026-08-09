//! Canonical, non-authoritative static-view proof obligations and evidence.
//!
//! Every identity in this module is caller-supplied. In particular, the
//! current authenticated-execution API permits caller-selected programs and
//! policy, persistent freshness ledgers have caller-selected namespaces, and
//! allocation epochs and exclusive leases have no allocator-authenticated
//! representation. None of those values can establish static-view authority.
//!
//! The authority bridge is deliberately absent. A future bridge must require
//! separate opaque trust-rooted facts for a pinned verifier/recorder/solver
//! policy, the canonical global ledger namespace, and a live allocator epoch
//! or exclusive lease. This module defines none of those facts.

use std::fmt;

use fe2o3_artifacts::DigestAlgorithm;
use fe2o3_contracts::{ByteRegionV1, StaticViewAccessDescriptionV1, StaticViewDescriptionV1};

use crate::{
    ControlFlowPayloadIdentityV1, ControlFlowProofRequestBindingV1, ControlFlowSourceBindingV1,
    Digest, ProofProperty, ProofRequestV1,
};

/// Version of the static-view obligation and evidence encodings.
pub const STATIC_VIEW_PROOF_VERSION_V1: u16 = 1;
/// Domain separator for a canonical static-view proof obligation.
pub const STATIC_VIEW_PROOF_OBLIGATION_DOMAIN_V1: [u8; 8] = *b"FE2SVPO\0";
/// Domain separator for canonical static-view request evidence.
pub const STATIC_VIEW_PROOF_EVIDENCE_DOMAIN_V1: [u8; 8] = *b"FE2SVPE\0";

/// Properties requested by a complete static-view proof obligation.
///
/// Presence in a request or result does not establish that a trusted verifier
/// proved the property.
pub const STATIC_VIEW_PROOF_REQUIRED_PROPERTIES_V1: [ProofProperty; 7] = [
    ProofProperty::Bounds,
    ProofProperty::AddressOverflowFreedom,
    ProofProperty::MemorySafety,
    ProofProperty::Initialization,
    ProofProperty::RaceFreedom,
    ProofProperty::LaunchValidity,
    ProofProperty::FunctionalCorrectness,
];

/// Caller-authored allocation lifetime and launch-epoch claim.
///
/// Construction checks arithmetic ordering only. It does not establish that
/// the allocation is live at any epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticViewLifetimeEpochClaimV1 {
    claimed_allocation_epoch_identity: Digest,
    claimed_valid_from_epoch: u64,
    claimed_valid_through_epoch: u64,
    claimed_launch_epoch: u64,
}

impl StaticViewLifetimeEpochClaimV1 {
    pub fn new(
        claimed_allocation_epoch_identity: Digest,
        claimed_valid_from_epoch: u64,
        claimed_valid_through_epoch: u64,
        claimed_launch_epoch: u64,
    ) -> Result<Self, StaticViewProofErrorV1> {
        require_nonzero(
            claimed_allocation_epoch_identity,
            "claimed allocation epoch",
        )?;
        if claimed_valid_from_epoch > claimed_valid_through_epoch {
            return Err(StaticViewProofErrorV1::InvalidClaimedLifetimeEpochRange {
                valid_from: claimed_valid_from_epoch,
                valid_through: claimed_valid_through_epoch,
            });
        }
        if claimed_launch_epoch < claimed_valid_from_epoch
            || claimed_launch_epoch > claimed_valid_through_epoch
        {
            return Err(StaticViewProofErrorV1::ClaimedLaunchEpochOutsideLifetime {
                launch_epoch: claimed_launch_epoch,
                valid_from: claimed_valid_from_epoch,
                valid_through: claimed_valid_through_epoch,
            });
        }
        Ok(Self {
            claimed_allocation_epoch_identity,
            claimed_valid_from_epoch,
            claimed_valid_through_epoch,
            claimed_launch_epoch,
        })
    }

    pub const fn claimed_allocation_epoch_identity(self) -> Digest {
        self.claimed_allocation_epoch_identity
    }

    pub const fn claimed_valid_from_epoch(self) -> u64 {
        self.claimed_valid_from_epoch
    }

    pub const fn claimed_valid_through_epoch(self) -> u64 {
        self.claimed_valid_through_epoch
    }

    pub const fn claimed_launch_epoch(self) -> u64 {
        self.claimed_launch_epoch
    }

    pub const fn authenticates_live_allocation(self) -> bool {
        false
    }
}

/// Canonical caller-authored static-view proof obligation.
///
/// The optional exclusive-lease identity is required when the underlying
/// description claims exclusive write access. This is a completeness check on
/// symbolic input, not evidence that a lease exists or is live.
///
/// No branded proof type or public authority constructor exists.
///
/// ```compile_fail
/// # fn claims_cannot_become_authority(
/// #     obligation: fe2o3_verifier::StaticViewProofObligationV1,
/// # ) {
/// let _: fe2o3_verifier::ProvenStaticViewRegionV1<'_> = obligation.into();
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticViewProofObligationV1 {
    description: StaticViewDescriptionV1,
    source_contract: ControlFlowPayloadIdentityV1,
    cfg_identity: ControlFlowPayloadIdentityV1,
    source_binding_identity: Digest,
    source_tree_identity: Digest,
    memory_contract_identity: Digest,
    type_layout_identity: Digest,
    capability_semantics_identity: Digest,
    launch_identity: Digest,
    claimed_lifetime: StaticViewLifetimeEpochClaimV1,
    claimed_exclusive_lease_identity: Option<Digest>,
    obligation_identity: Digest,
}

impl StaticViewProofObligationV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        description: StaticViewDescriptionV1,
        source: &ControlFlowSourceBindingV1,
        source_tree_identity: Digest,
        memory_contract_identity: Digest,
        type_layout_identity: Digest,
        capability_semantics_identity: Digest,
        launch_identity: Digest,
        claimed_lifetime: StaticViewLifetimeEpochClaimV1,
        claimed_exclusive_lease_identity: Option<Digest>,
    ) -> Result<Self, StaticViewProofErrorV1> {
        for (field, identity) in [
            ("source binding", source.binding_identity()),
            ("source tree", source_tree_identity),
            ("memory contract", memory_contract_identity),
            ("type layout", type_layout_identity),
            ("capability semantics", capability_semantics_identity),
            ("launch", launch_identity),
        ] {
            require_nonzero(identity, field)?;
        }
        require_nonzero(source.source_contract().digest(), "source contract")?;
        require_nonzero(source.cfg_identity().digest(), "CFG")?;

        match (
            description.access_description(),
            claimed_exclusive_lease_identity,
        ) {
            (StaticViewAccessDescriptionV1::SharedRead, Some(_)) => {
                return Err(StaticViewProofErrorV1::UnexpectedClaimedExclusiveLease);
            }
            (StaticViewAccessDescriptionV1::ExclusiveWrite, None) => {
                return Err(StaticViewProofErrorV1::MissingClaimedExclusiveLease);
            }
            (_, Some(identity)) => require_nonzero(identity, "claimed exclusive lease")?,
            (_, None) => {}
        }

        let mut obligation = Self {
            description,
            source_contract: source.source_contract(),
            cfg_identity: source.cfg_identity(),
            source_binding_identity: source.binding_identity(),
            source_tree_identity,
            memory_contract_identity,
            type_layout_identity,
            capability_semantics_identity,
            launch_identity,
            claimed_lifetime,
            claimed_exclusive_lease_identity,
            obligation_identity: Digest::from_bytes([0; 32]),
        };
        obligation.obligation_identity = sha256(&obligation.to_canonical_bytes());
        Ok(obligation)
    }

    pub const fn version(&self) -> u16 {
        STATIC_VIEW_PROOF_VERSION_V1
    }

    pub const fn description(&self) -> StaticViewDescriptionV1 {
        self.description
    }

    pub const fn source_contract(&self) -> ControlFlowPayloadIdentityV1 {
        self.source_contract
    }

    pub const fn cfg_identity(&self) -> ControlFlowPayloadIdentityV1 {
        self.cfg_identity
    }

    pub const fn source_binding_identity(&self) -> Digest {
        self.source_binding_identity
    }

    pub const fn source_tree_identity(&self) -> Digest {
        self.source_tree_identity
    }

    pub const fn memory_contract_identity(&self) -> Digest {
        self.memory_contract_identity
    }

    pub const fn type_layout_identity(&self) -> Digest {
        self.type_layout_identity
    }

    pub const fn capability_semantics_identity(&self) -> Digest {
        self.capability_semantics_identity
    }

    pub const fn launch_identity(&self) -> Digest {
        self.launch_identity
    }

    pub const fn claimed_lifetime(&self) -> StaticViewLifetimeEpochClaimV1 {
        self.claimed_lifetime
    }

    pub const fn claimed_exclusive_lease_identity(&self) -> Option<Digest> {
        self.claimed_exclusive_lease_identity
    }

    pub const fn obligation_identity(&self) -> Digest {
        self.obligation_identity
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }

    pub const fn authenticates_live_allocation(&self) -> bool {
        false
    }

    pub const fn authenticates_exclusive_lease(&self) -> bool {
        false
    }

    /// Canonical fixed-width encoding of the caller-authored obligation.
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let description = self.description;
        let allocation = description.described_allocation();
        let parent = description.described_parent_region();
        let region = description.described_region();
        let mut writer = IdentityWriter::with_domain(STATIC_VIEW_PROOF_OBLIGATION_DOMAIN_V1);
        writer.u32(self.source_contract.byte_len());
        writer.digest(self.source_contract.digest());
        writer.u32(self.cfg_identity.byte_len());
        writer.digest(self.cfg_identity.digest());
        writer.digest(self.source_binding_identity);
        writer.digest(self.source_tree_identity);
        writer.digest(self.memory_contract_identity);
        writer.digest(self.type_layout_identity);
        writer.digest(self.capability_semantics_identity);
        writer.digest(self.launch_identity);
        writer.digest(self.claimed_lifetime.claimed_allocation_epoch_identity);
        writer.u64(self.claimed_lifetime.claimed_valid_from_epoch);
        writer.u64(self.claimed_lifetime.claimed_valid_through_epoch);
        writer.u64(self.claimed_lifetime.claimed_launch_epoch);
        writer.u32(allocation.provenance().get());
        writer.u16(allocation.address_space().get());
        writer.u16(0);
        writer.u64(allocation.base_address());
        writer.u64(allocation.byte_length());
        writer.u64(allocation.address_space_size());
        write_region(&mut writer, parent);
        write_region(&mut writer, region);
        writer.u64(description.parent_element_count());
        writer.u64(description.start_element());
        writer.u64(description.element_count());
        writer.u64(description.element_size());
        writer.u64(description.element_alignment());
        writer.u8(match description.access_description() {
            StaticViewAccessDescriptionV1::SharedRead => 1,
            StaticViewAccessDescriptionV1::ExclusiveWrite => 2,
        });
        match self.claimed_exclusive_lease_identity {
            Some(identity) => {
                writer.u8(1);
                writer.digest(identity);
            }
            None => {
                writer.u8(0);
                writer.digest(Digest::from_bytes([0; 32]));
            }
        }
        writer.u16(0);
        writer.finish()
    }
}

/// Digest used to identify this obligation in an external functional spec.
///
/// This is content identity only. It does not authenticate the obligation.
pub fn derive_static_view_functional_specification_digest_v1(
    obligation: &StaticViewProofObligationV1,
) -> Digest {
    obligation.obligation_identity
}

/// Canonical descriptive evidence that an obligation was joined to one exact
/// control-flow proof request.
///
/// The request, property list, tool policy, and all identities remain
/// caller-selected. This value does not indicate that any trusted verifier ran
/// or that a live allocation or lease exists.
///
/// ```compile_fail
/// # fn caller_execution_cannot_mint(
/// #     evidence: fe2o3_verifier::StaticViewProofEvidenceV1,
/// #     execution: fe2o3_verifier::PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1,
/// # ) {
/// let _ = fe2o3_verifier::with_authenticated_static_view_proof_v1(
///     evidence,
///     execution,
///     |_| (),
/// );
/// # }
/// ```
///
/// ```compile_fail
/// # fn evidence_cannot_create_a_static_index(
/// #     evidence: fe2o3_verifier::StaticViewProofEvidenceV1,
/// # ) {
/// let _ = evidence.into_static_index::<0>();
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticViewProofEvidenceV1 {
    obligation: StaticViewProofObligationV1,
    control_flow_request: ControlFlowProofRequestBindingV1,
    evidence_identity: Digest,
}

impl StaticViewProofEvidenceV1 {
    pub const fn obligation(&self) -> &StaticViewProofObligationV1 {
        &self.obligation
    }

    pub const fn control_flow_request(&self) -> &ControlFlowProofRequestBindingV1 {
        &self.control_flow_request
    }

    pub const fn evidence_identity(&self) -> Digest {
        self.evidence_identity
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }

    pub const fn authenticates_verifier_execution(&self) -> bool {
        false
    }

    pub const fn authenticates_global_ledger_namespace(&self) -> bool {
        false
    }

    pub const fn authenticates_live_allocation(&self) -> bool {
        false
    }

    pub const fn authenticates_exclusive_lease(&self) -> bool {
        false
    }

    /// Canonical encoding of the exact obligation/request join.
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut writer = IdentityWriter::with_domain(STATIC_VIEW_PROOF_EVIDENCE_DOMAIN_V1);
        writer.digest(self.obligation.obligation_identity);
        writer.digest(self.control_flow_request.source().binding_identity());
        writer.digest(self.control_flow_request.binding_identity());
        writer.digest(self.control_flow_request.request_digest());
        writer.digest(self.control_flow_request.functional_specification_digest());
        for digest in self.control_flow_request.target().digests() {
            writer.digest(digest);
        }
        writer.finish()
    }
}

/// Joins caller-authored obligation and request records as inert evidence.
pub fn bind_static_view_proof_evidence_v1(
    request: &ProofRequestV1,
    control_flow_request: ControlFlowProofRequestBindingV1,
    obligation: StaticViewProofObligationV1,
) -> Result<StaticViewProofEvidenceV1, StaticViewProofErrorV1> {
    require_requested_properties(request.properties())?;
    if sha256(&request.to_canonical_bytes()) != control_flow_request.request_digest() {
        return Err(StaticViewProofErrorV1::ProofRequestMismatch);
    }
    if request.target() != control_flow_request.target() {
        return Err(StaticViewProofErrorV1::ProofTargetMismatch);
    }
    if derive_static_view_functional_specification_digest_v1(&obligation)
        != control_flow_request.base_functional_specification_digest()
    {
        return Err(StaticViewProofErrorV1::FunctionalSpecificationMismatch);
    }
    validate_source(&obligation, control_flow_request.source())?;
    validate_target(&obligation, request.target())?;

    let mut evidence = StaticViewProofEvidenceV1 {
        obligation,
        control_flow_request,
        evidence_identity: Digest::from_bytes([0; 32]),
    };
    evidence.evidence_identity = sha256(&evidence.to_canonical_bytes());
    Ok(evidence)
}

fn validate_source(
    obligation: &StaticViewProofObligationV1,
    source: &ControlFlowSourceBindingV1,
) -> Result<(), StaticViewProofErrorV1> {
    for (field, expected, actual) in [
        (
            "source contract",
            obligation.source_contract,
            source.source_contract(),
        ),
        ("CFG", obligation.cfg_identity, source.cfg_identity()),
    ] {
        if expected != actual {
            return Err(StaticViewProofErrorV1::SourceIdentityMismatch { field });
        }
    }
    if obligation.source_binding_identity != source.binding_identity() {
        return Err(StaticViewProofErrorV1::SourceIdentityMismatch {
            field: "source binding",
        });
    }
    Ok(())
}

fn validate_target(
    obligation: &StaticViewProofObligationV1,
    target: crate::ProofTargetIdentity,
) -> Result<(), StaticViewProofErrorV1> {
    for (field, expected, actual) in [
        (
            "source tree",
            obligation.source_tree_identity,
            target.source_tree_digest,
        ),
        (
            "memory contract",
            obligation.memory_contract_identity,
            target.memory_contract_digest,
        ),
        (
            "type layout",
            obligation.type_layout_identity,
            target.type_layout_digest,
        ),
        (
            "capability semantics",
            obligation.capability_semantics_identity,
            target.capability_semantics_digest,
        ),
    ] {
        if expected != actual {
            return Err(StaticViewProofErrorV1::TargetIdentityMismatch { field });
        }
    }
    Ok(())
}

fn require_requested_properties(
    properties: &[ProofProperty],
) -> Result<(), StaticViewProofErrorV1> {
    for property in STATIC_VIEW_PROOF_REQUIRED_PROPERTIES_V1 {
        if properties.binary_search(&property).is_err() {
            return Err(StaticViewProofErrorV1::MissingRequestedProperty { property });
        }
    }
    Ok(())
}

fn require_nonzero(identity: Digest, field: &'static str) -> Result<(), StaticViewProofErrorV1> {
    if identity.as_bytes().iter().all(|byte| *byte == 0) {
        Err(StaticViewProofErrorV1::ZeroIdentity { field })
    } else {
        Ok(())
    }
}

fn write_region(writer: &mut IdentityWriter, region: ByteRegionV1) {
    writer.u32(region.provenance().get());
    writer.u16(region.address_space().get());
    writer.u16(0);
    writer.u64(region.byte_offset());
    writer.u64(region.byte_length());
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

struct IdentityWriter {
    bytes: Vec<u8>,
}

impl IdentityWriter {
    fn with_domain(domain: [u8; 8]) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&domain);
        bytes.extend_from_slice(&STATIC_VIEW_PROOF_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        Self { bytes }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn digest(&mut self, value: Digest) {
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// Why a static-view obligation or request-evidence join was rejected.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StaticViewProofErrorV1 {
    ZeroIdentity {
        field: &'static str,
    },
    InvalidClaimedLifetimeEpochRange {
        valid_from: u64,
        valid_through: u64,
    },
    ClaimedLaunchEpochOutsideLifetime {
        launch_epoch: u64,
        valid_from: u64,
        valid_through: u64,
    },
    MissingClaimedExclusiveLease,
    UnexpectedClaimedExclusiveLease,
    MissingRequestedProperty {
        property: ProofProperty,
    },
    ProofRequestMismatch,
    ProofTargetMismatch,
    FunctionalSpecificationMismatch,
    SourceIdentityMismatch {
        field: &'static str,
    },
    TargetIdentityMismatch {
        field: &'static str,
    },
}

impl fmt::Display for StaticViewProofErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid static-view proof obligation: {self:?}")
    }
}

impl std::error::Error for StaticViewProofErrorV1 {}
