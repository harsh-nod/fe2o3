use std::fmt;

use sha2::{Digest, Sha256};

use crate::{CapabilityBindingV3, PublicationRightsV1};

/// Distinct Broker V4 frame magic.
pub const BROKER_V4_MAGIC: [u8; 8] = *b"F2AUBR4\0";
/// Broker V4 wire version.
pub const BROKER_V4_VERSION: u16 = 4;
/// Exact Broker V4 frame-header length.
pub const BROKER_V4_HEADER_LEN: usize = 24;
/// Exact encoded process-identity length.
pub const PROCESS_IDENTITY_V4_WIRE_LEN: usize = 16;
/// Exact encoded CapabilityBinding V4 length.
pub const BROKER_V4_BINDING_WIRE_LEN: usize = 104;
/// Domain for the canonical CapabilityBinding V4 identity.
pub const BROKER_V4_BINDING_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/PROTECTED-AUTHORITY-BROKER-V4-BINDING\0";
/// Canonical mode declared for a receiver-owned host-link output.
pub const HOST_LINK_OUTPUT_MODE_V4: u32 = 0o555;
/// Broker V4's semantic authority classification: `AUTHORITY=none`.
pub const BROKER_V4_AUTHORITY: BrokerAuthorityV4 = BrokerAuthorityV4::None;

/// Payload-relative process identity offset shared by every V4 frame.
pub const BROKER_V4_PROCESS_OFFSET: usize = 0;
/// Payload-relative capability-binding identity offset shared by every V4 frame.
pub const BROKER_V4_BINDING_OFFSET: usize = 16;
/// Payload-relative host-link request identity offset.
pub const HOST_LINK_REQUEST_OFFSET_V4: usize = 48;
/// Payload-relative prepared host-link plan identity offset.
pub const HOST_LINK_PLAN_OFFSET_V4: usize = 80;
/// Payload-relative prepared host-link closure identity offset.
pub const HOST_LINK_CLOSURE_OFFSET_V4: usize = 112;
/// Payload-relative host-link grant identity offset.
pub const HOST_LINK_GRANT_OFFSET_V4: usize = 144;
/// Payload-relative declared output SHA-256 offset in HostLinkCommit V4.
pub const HOST_LINK_COMMIT_OUTPUT_SHA256_OFFSET_V4: usize = 176;
/// Payload-relative declared output length offset in HostLinkCommit V4.
pub const HOST_LINK_COMMIT_OUTPUT_LENGTH_OFFSET_V4: usize = 208;
/// Payload-relative admitted output mode offset in HostLinkCommit V4.
pub const HOST_LINK_COMMIT_OUTPUT_MODE_OFFSET_V4: usize = 216;
/// Payload-relative reserved-field offset in HostLinkCommit V4.
pub const HOST_LINK_COMMIT_RESERVED_OFFSET_V4: usize = 220;
/// Payload-relative durable publication-plan identity offset in HostLinkCommit V4.
pub const HOST_LINK_COMMIT_DURABLE_PLAN_OFFSET_V4: usize = 224;

/// Exact HostLinkPrepare V4 payload length.
pub const HOST_LINK_PREPARE_V4_PAYLOAD_LEN: usize = 144;
/// Exact HostLinkGrant V4 payload length.
pub const HOST_LINK_GRANT_V4_PAYLOAD_LEN: usize = 176;
/// Exact HostLinkCommit V4 payload length.
pub const HOST_LINK_COMMIT_V4_PAYLOAD_LEN: usize = 256;

const IDENTITY_LEN: usize = 32;
const BINDING_V3_IDENTITY_OFFSET: usize = 0;
const RELEASE_CONTRACT_IDENTITY_OFFSET: usize = 32;
const STATIC_HOST_LLD_IDENTITY_OFFSET: usize = 64;
const TARGET_OFFSET: usize = 96;
const BINDING_RESERVED_OFFSET: usize = 98;
const PUBLICATION_RIGHTS_OFFSET: usize = 100;

/// One assigned Broker V4 extension frame type and sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum BrokerFrameKindV4 {
    /// Protected wrapper submits one prepared host-link closure.
    HostLinkPrepare = 1,
    /// Carries one declared host-link grant.
    HostLinkGrant = 2,
    /// Carries one declared host-link output commitment.
    HostLinkCommit = 3,
}

impl BrokerFrameKindV4 {
    const fn sequence(self) -> u32 {
        match self {
            Self::HostLinkPrepare => 0,
            Self::HostLinkGrant => 1,
            Self::HostLinkCommit => 2,
        }
    }

    const fn payload_len(self) -> usize {
        match self {
            Self::HostLinkPrepare => HOST_LINK_PREPARE_V4_PAYLOAD_LEN,
            Self::HostLinkGrant => HOST_LINK_GRANT_V4_PAYLOAD_LEN,
            Self::HostLinkCommit => HOST_LINK_COMMIT_V4_PAYLOAD_LEN,
        }
    }

    fn from_wire(value: u16) -> Result<Self, BrokerProtocolErrorV4> {
        match value {
            1 => Ok(Self::HostLinkPrepare),
            2 => Ok(Self::HostLinkGrant),
            3 => Ok(Self::HostLinkCommit),
            actual => Err(BrokerProtocolErrorV4::UnknownFrameType { actual }),
        }
    }
}

/// The only target admitted by the Broker V4 extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum BrokerTargetV4 {
    /// AMD gfx942 with XNACK disabled.
    Gfx942XnackMinus = 1,
}

impl BrokerTargetV4 {
    fn from_wire(value: u16) -> Result<Self, BrokerProtocolErrorV4> {
        match value {
            1 => Ok(Self::Gfx942XnackMinus),
            actual => Err(BrokerProtocolErrorV4::UnknownTarget { actual }),
        }
    }
}

/// Semantic authority carried by the inert Broker V4 transcript layer.
///
/// V4 validates canonical encoding and per-instance transcript continuity. It
/// does not establish freshness, global replay exclusion, execution authority,
/// or publication authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerAuthorityV4 {
    /// No authority is carried by a V4 binding, frame, or validated transcript.
    None,
}

/// A required nonzero identity in Broker V4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BrokerIdentityFieldV4 {
    /// Completed canonical Broker V3 binding identity.
    BrokerV3Binding,
    /// Canonical authority-release contract identity.
    ReleaseContract,
    /// Authenticated static upstream-LLVM host LLD identity.
    StaticHostLld,
    /// Canonical Broker V4 capability-binding identity.
    CapabilityBinding,
    /// Host-link request identity.
    HostLinkRequest,
    /// Prepared HostLinkClosure plan identity.
    HostLinkPlan,
    /// Prepared HostLinkClosure admitted-input identity.
    HostLinkClosure,
    /// Host-link grant identity.
    HostLinkGrant,
    /// Declared host-link output content identity.
    HostLinkOutput,
    /// Durable host-link publication-plan identity.
    DurableHostLinkPlan,
}

impl fmt::Display for BrokerIdentityFieldV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::BrokerV3Binding => "Broker V3 binding",
            Self::ReleaseContract => "release contract",
            Self::StaticHostLld => "static host LLD",
            Self::CapabilityBinding => "Broker V4 capability binding",
            Self::HostLinkRequest => "host-link request",
            Self::HostLinkPlan => "host-link plan",
            Self::HostLinkClosure => "host-link closure",
            Self::HostLinkGrant => "host-link grant",
            Self::HostLinkOutput => "host-link output",
            Self::DurableHostLinkPlan => "durable host-link plan",
        };
        formatter.write_str(name)
    }
}

/// Stable process identity carried by the Broker V4 extension transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessIdentityV4 {
    pid: u32,
    start_time_ticks: u64,
}

impl ProcessIdentityV4 {
    /// Constructs a nonzero process identity.
    pub fn new(pid: u32, start_time_ticks: u64) -> Result<Self, BrokerProtocolErrorV4> {
        if pid == 0 {
            return Err(BrokerProtocolErrorV4::ZeroProcessId);
        }
        if start_time_ticks == 0 {
            return Err(BrokerProtocolErrorV4::ZeroProcessStartTime);
        }
        Ok(Self {
            pid,
            start_time_ticks,
        })
    }

    /// Returns the numeric process identifier.
    pub const fn pid(self) -> u32 {
        self.pid
    }

    /// Returns the `/proc` start-time tick count.
    pub const fn start_time_ticks(self) -> u64 {
        self.start_time_ticks
    }

    fn encode(self, output: &mut [u8]) {
        output[0..4].copy_from_slice(&self.pid.to_le_bytes());
        output[8..16].copy_from_slice(&self.start_time_ticks.to_le_bytes());
    }

    fn decode(encoded: &[u8]) -> Result<Self, BrokerProtocolErrorV4> {
        if read_u32(encoded, 4) != 0 {
            return Err(BrokerProtocolErrorV4::NonzeroProcessReserved);
        }
        Self::new(read_u32(encoded, 0), read_u64(encoded, 8))
    }
}

/// Canonical zero-publication-rights binding for the Broker V4 extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityBindingV4 {
    broker_v3_binding_identity: [u8; 32],
    release_contract_identity: [u8; 32],
    static_host_lld_identity: [u8; 32],
}

impl CapabilityBindingV4 {
    /// Constructs one V4 extension binding from exact nonzero identities.
    pub fn new(
        broker_v3_binding_identity: [u8; 32],
        release_contract_identity: [u8; 32],
        static_host_lld_identity: [u8; 32],
    ) -> Result<Self, BrokerProtocolErrorV4> {
        for (field, identity) in [
            (
                BrokerIdentityFieldV4::BrokerV3Binding,
                broker_v3_binding_identity,
            ),
            (
                BrokerIdentityFieldV4::ReleaseContract,
                release_contract_identity,
            ),
            (
                BrokerIdentityFieldV4::StaticHostLld,
                static_host_lld_identity,
            ),
        ] {
            validate_identity(identity, field)?;
        }
        Ok(Self {
            broker_v3_binding_identity,
            release_contract_identity,
            static_host_lld_identity,
        })
    }

    /// Constructs a V4 extension binding from a complete canonical V3 binding.
    pub fn for_v3(
        broker_v3_binding: CapabilityBindingV3,
        release_contract_identity: [u8; 32],
        static_host_lld_identity: [u8; 32],
    ) -> Result<Self, BrokerProtocolErrorV4> {
        Self::new(
            broker_v3_binding.identity_sha256(),
            release_contract_identity,
            static_host_lld_identity,
        )
    }

    /// Returns the completed Broker V3 binding identity.
    pub const fn broker_v3_binding_identity(self) -> [u8; 32] {
        self.broker_v3_binding_identity
    }

    /// Returns the authority-release contract identity.
    pub const fn release_contract_identity(self) -> [u8; 32] {
        self.release_contract_identity
    }

    /// Returns the authenticated static host-LLD identity.
    pub const fn static_host_lld_identity(self) -> [u8; 32] {
        self.static_host_lld_identity
    }

    /// Returns the fixed gfx942 XNACK-minus target inherited from Broker V3.
    pub const fn target(self) -> BrokerTargetV4 {
        BrokerTargetV4::Gfx942XnackMinus
    }

    /// Returns the unconditionally empty publication-rights set.
    pub const fn publication_rights(self) -> PublicationRightsV1 {
        PublicationRightsV1::NONE
    }

    /// Returns the fixed `AUTHORITY=none` semantic classification.
    pub const fn authority(self) -> BrokerAuthorityV4 {
        BROKER_V4_AUTHORITY
    }

    /// Returns the exact fixed-width canonical encoding.
    pub fn encode(self) -> [u8; BROKER_V4_BINDING_WIRE_LEN] {
        let mut encoded = [0_u8; BROKER_V4_BINDING_WIRE_LEN];
        encoded[BINDING_V3_IDENTITY_OFFSET..RELEASE_CONTRACT_IDENTITY_OFFSET]
            .copy_from_slice(&self.broker_v3_binding_identity);
        encoded[RELEASE_CONTRACT_IDENTITY_OFFSET..STATIC_HOST_LLD_IDENTITY_OFFSET]
            .copy_from_slice(&self.release_contract_identity);
        encoded[STATIC_HOST_LLD_IDENTITY_OFFSET..TARGET_OFFSET]
            .copy_from_slice(&self.static_host_lld_identity);
        encoded[TARGET_OFFSET..BINDING_RESERVED_OFFSET]
            .copy_from_slice(&(BrokerTargetV4::Gfx942XnackMinus as u16).to_le_bytes());
        encoded[PUBLICATION_RIGHTS_OFFSET..BROKER_V4_BINDING_WIRE_LEN]
            .copy_from_slice(&PublicationRightsV1::NONE.bits().to_le_bytes());
        encoded
    }

    /// Returns the domain-separated canonical binding identity.
    pub fn identity_sha256(self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(BROKER_V4_BINDING_IDENTITY_DOMAIN);
        digest.update((BROKER_V4_BINDING_WIRE_LEN as u64).to_le_bytes());
        digest.update(self.encode());
        digest.finalize().into()
    }
}

/// Decodes one exact canonical CapabilityBinding V4.
pub fn decode_capability_binding_v4(
    encoded: &[u8],
) -> Result<CapabilityBindingV4, BrokerProtocolErrorV4> {
    if encoded.len() != BROKER_V4_BINDING_WIRE_LEN {
        return Err(BrokerProtocolErrorV4::InvalidBindingLength {
            actual: encoded.len(),
        });
    }
    BrokerTargetV4::from_wire(read_u16(encoded, TARGET_OFFSET))?;
    if read_u16(encoded, BINDING_RESERVED_OFFSET) != 0 {
        return Err(BrokerProtocolErrorV4::NonzeroBindingReserved);
    }
    let rights = read_u32(encoded, PUBLICATION_RIGHTS_OFFSET);
    if rights != PublicationRightsV1::NONE.bits() {
        return Err(BrokerProtocolErrorV4::PublicationAuthorityForbidden { actual: rights });
    }
    CapabilityBindingV4::new(
        digest_at(encoded, BINDING_V3_IDENTITY_OFFSET),
        digest_at(encoded, RELEASE_CONTRACT_IDENTITY_OFFSET),
        digest_at(encoded, STATIC_HOST_LLD_IDENTITY_OFFSET),
    )
}

/// Protected-wrapper request for one exact prepared host-link closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostLinkPrepareV4 {
    process: ProcessIdentityV4,
    binding_identity: [u8; 32],
    request_identity: [u8; 32],
    plan_identity: [u8; 32],
    closure_identity: [u8; 32],
}

impl HostLinkPrepareV4 {
    /// Constructs one canonical host-link preparation request.
    pub fn new(
        process: ProcessIdentityV4,
        binding_identity: [u8; 32],
        request_identity: [u8; 32],
        plan_identity: [u8; 32],
        closure_identity: [u8; 32],
    ) -> Result<Self, BrokerProtocolErrorV4> {
        validate_host_link_identities(
            binding_identity,
            request_identity,
            plan_identity,
            closure_identity,
        )?;
        Ok(Self {
            process,
            binding_identity,
            request_identity,
            plan_identity,
            closure_identity,
        })
    }

    /// Returns the stable protected-process identity.
    pub const fn process(self) -> ProcessIdentityV4 {
        self.process
    }

    /// Returns the canonical Broker V4 capability-binding identity.
    pub const fn binding_identity(self) -> [u8; 32] {
        self.binding_identity
    }

    /// Returns the request identity.
    pub const fn request_identity(self) -> [u8; 32] {
        self.request_identity
    }

    /// Returns the prepared host-link plan identity.
    pub const fn plan_identity(self) -> [u8; 32] {
        self.plan_identity
    }

    /// Returns the prepared host-link closure identity.
    pub const fn closure_identity(self) -> [u8; 32] {
        self.closure_identity
    }
}

/// Canonical grant frame for one exact prepared host-link closure.
///
/// This wire value carries no consume authority and may be copied for encoding
/// and transport. Validation creates only a move-only, per-instance transcript
/// state; it does not establish global uniqueness or grant authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostLinkGrantV4 {
    process: ProcessIdentityV4,
    binding_identity: [u8; 32],
    request_identity: [u8; 32],
    plan_identity: [u8; 32],
    closure_identity: [u8; 32],
    grant_identity: [u8; 32],
}

impl HostLinkGrantV4 {
    /// Constructs one canonical host-link grant frame.
    pub fn new(
        process: ProcessIdentityV4,
        binding_identity: [u8; 32],
        request_identity: [u8; 32],
        plan_identity: [u8; 32],
        closure_identity: [u8; 32],
        grant_identity: [u8; 32],
    ) -> Result<Self, BrokerProtocolErrorV4> {
        validate_host_link_identities(
            binding_identity,
            request_identity,
            plan_identity,
            closure_identity,
        )?;
        validate_identity(grant_identity, BrokerIdentityFieldV4::HostLinkGrant)?;
        Ok(Self {
            process,
            binding_identity,
            request_identity,
            plan_identity,
            closure_identity,
            grant_identity,
        })
    }

    /// Returns the stable protected-process identity.
    pub const fn process(self) -> ProcessIdentityV4 {
        self.process
    }

    /// Returns the canonical Broker V4 capability-binding identity.
    pub const fn binding_identity(self) -> [u8; 32] {
        self.binding_identity
    }

    /// Returns the granted request identity.
    pub const fn request_identity(self) -> [u8; 32] {
        self.request_identity
    }

    /// Returns the granted host-link plan identity.
    pub const fn plan_identity(self) -> [u8; 32] {
        self.plan_identity
    }

    /// Returns the granted host-link closure identity.
    pub const fn closure_identity(self) -> [u8; 32] {
        self.closure_identity
    }

    /// Returns the declared grant identity.
    pub const fn grant_identity(self) -> [u8; 32] {
        self.grant_identity
    }

    /// Returns the fixed `AUTHORITY=none` semantic classification.
    pub const fn authority(self) -> BrokerAuthorityV4 {
        BROKER_V4_AUTHORITY
    }
}

/// Protected-wrapper declaration of one host-link output commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostLinkCommitV4 {
    process: ProcessIdentityV4,
    binding_identity: [u8; 32],
    request_identity: [u8; 32],
    plan_identity: [u8; 32],
    closure_identity: [u8; 32],
    grant_identity: [u8; 32],
    output_sha256: [u8; 32],
    output_length: u64,
    output_mode: u32,
    durable_plan_identity: [u8; 32],
}

impl HostLinkCommitV4 {
    /// Constructs one canonical declared-output commit frame.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        process: ProcessIdentityV4,
        binding_identity: [u8; 32],
        request_identity: [u8; 32],
        plan_identity: [u8; 32],
        closure_identity: [u8; 32],
        grant_identity: [u8; 32],
        output_sha256: [u8; 32],
        output_length: u64,
        output_mode: u32,
        durable_plan_identity: [u8; 32],
    ) -> Result<Self, BrokerProtocolErrorV4> {
        validate_host_link_identities(
            binding_identity,
            request_identity,
            plan_identity,
            closure_identity,
        )?;
        for (field, identity) in [
            (BrokerIdentityFieldV4::HostLinkGrant, grant_identity),
            (BrokerIdentityFieldV4::HostLinkOutput, output_sha256),
            (
                BrokerIdentityFieldV4::DurableHostLinkPlan,
                durable_plan_identity,
            ),
        ] {
            validate_identity(identity, field)?;
        }
        if output_length == 0 {
            return Err(BrokerProtocolErrorV4::ZeroHostLinkOutputLength);
        }
        if output_mode != HOST_LINK_OUTPUT_MODE_V4 {
            return Err(BrokerProtocolErrorV4::InvalidHostLinkOutputMode {
                actual: output_mode,
            });
        }
        Ok(Self {
            process,
            binding_identity,
            request_identity,
            plan_identity,
            closure_identity,
            grant_identity,
            output_sha256,
            output_length,
            output_mode,
            durable_plan_identity,
        })
    }

    /// Returns the stable protected-process identity.
    pub const fn process(self) -> ProcessIdentityV4 {
        self.process
    }

    /// Returns the canonical Broker V4 capability-binding identity.
    pub const fn binding_identity(self) -> [u8; 32] {
        self.binding_identity
    }

    /// Returns the declared request identity.
    pub const fn request_identity(self) -> [u8; 32] {
        self.request_identity
    }

    /// Returns the committed host-link plan identity.
    pub const fn plan_identity(self) -> [u8; 32] {
        self.plan_identity
    }

    /// Returns the committed host-link closure identity.
    pub const fn closure_identity(self) -> [u8; 32] {
        self.closure_identity
    }

    /// Returns the declared grant identity.
    pub const fn grant_identity(self) -> [u8; 32] {
        self.grant_identity
    }

    /// Returns the declared output SHA-256 identity.
    pub const fn output_sha256(self) -> [u8; 32] {
        self.output_sha256
    }

    /// Returns the declared output length.
    pub const fn output_length(self) -> u64 {
        self.output_length
    }

    /// Returns the declared output mode.
    pub const fn output_mode(self) -> u32 {
        self.output_mode
    }

    /// Returns the durable publication-plan identity.
    pub const fn durable_plan_identity(self) -> [u8; 32] {
        self.durable_plan_identity
    }
}

/// One typed Broker V4 extension frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerFrameV4 {
    /// Prepared host-link closure request.
    HostLinkPrepare(HostLinkPrepareV4),
    /// Declared host-link grant.
    HostLinkGrant(HostLinkGrantV4),
    /// Declared host-link output commitment.
    HostLinkCommit(HostLinkCommitV4),
}

impl BrokerFrameV4 {
    /// Returns the assigned frame type.
    pub const fn kind(self) -> BrokerFrameKindV4 {
        match self {
            Self::HostLinkPrepare(_) => BrokerFrameKindV4::HostLinkPrepare,
            Self::HostLinkGrant(_) => BrokerFrameKindV4::HostLinkGrant,
            Self::HostLinkCommit(_) => BrokerFrameKindV4::HostLinkCommit,
        }
    }

    /// Returns the exact encoded frame length.
    pub const fn encoded_len(self) -> usize {
        BROKER_V4_HEADER_LEN + self.kind().payload_len()
    }

    /// Returns the fixed `AUTHORITY=none` semantic classification.
    pub const fn authority(self) -> BrokerAuthorityV4 {
        BROKER_V4_AUTHORITY
    }

    /// Encodes this frame canonically.
    pub fn encode(self) -> Vec<u8> {
        encode_broker_frame_v4(&self)
    }
}

/// Encodes one typed Broker V4 frame with its exact 24-byte header.
pub fn encode_broker_frame_v4(frame: &BrokerFrameV4) -> Vec<u8> {
    let kind = frame.kind();
    let mut encoded = vec![0_u8; BROKER_V4_HEADER_LEN + kind.payload_len()];
    encoded[0..8].copy_from_slice(&BROKER_V4_MAGIC);
    encoded[8..10].copy_from_slice(&BROKER_V4_VERSION.to_le_bytes());
    encoded[10..12].copy_from_slice(&(kind as u16).to_le_bytes());
    encoded[12..16].copy_from_slice(&(kind.payload_len() as u32).to_le_bytes());
    encoded[16..20].copy_from_slice(&kind.sequence().to_le_bytes());
    let payload = &mut encoded[BROKER_V4_HEADER_LEN..];
    match frame {
        BrokerFrameV4::HostLinkPrepare(value) => encode_host_link_prepare(*value, payload),
        BrokerFrameV4::HostLinkGrant(value) => encode_host_link_grant(*value, payload),
        BrokerFrameV4::HostLinkCommit(value) => encode_host_link_commit(*value, payload),
    }
    encoded
}

/// Decodes one exact canonical Broker V4 extension frame.
pub fn decode_broker_frame_v4(encoded: &[u8]) -> Result<BrokerFrameV4, BrokerProtocolErrorV4> {
    if encoded.len() < BROKER_V4_HEADER_LEN {
        return Err(BrokerProtocolErrorV4::TruncatedHeader {
            actual: encoded.len(),
        });
    }
    if encoded[0..8] != BROKER_V4_MAGIC {
        return Err(BrokerProtocolErrorV4::InvalidMagic);
    }
    let version = read_u16(encoded, 8);
    if version != BROKER_V4_VERSION {
        return Err(BrokerProtocolErrorV4::UnsupportedVersion { actual: version });
    }
    let kind = BrokerFrameKindV4::from_wire(read_u16(encoded, 10))?;
    let payload_len = read_u32(encoded, 12);
    if payload_len != kind.payload_len() as u32 {
        return Err(BrokerProtocolErrorV4::InvalidPayloadLength {
            kind,
            expected: kind.payload_len(),
            actual: payload_len,
        });
    }
    let sequence = read_u32(encoded, 16);
    if sequence != kind.sequence() {
        return Err(BrokerProtocolErrorV4::InvalidSequence {
            kind,
            expected: kind.sequence(),
            actual: sequence,
        });
    }
    let flags = read_u32(encoded, 20);
    if flags != 0 {
        return Err(BrokerProtocolErrorV4::UnsupportedFlags { actual: flags });
    }
    let expected_len = BROKER_V4_HEADER_LEN + kind.payload_len();
    if encoded.len() != expected_len {
        return Err(BrokerProtocolErrorV4::InvalidEncodedLength {
            expected: expected_len,
            actual: encoded.len(),
        });
    }
    let payload = &encoded[BROKER_V4_HEADER_LEN..];
    match kind {
        BrokerFrameKindV4::HostLinkPrepare => {
            decode_host_link_prepare(payload).map(BrokerFrameV4::HostLinkPrepare)
        }
        BrokerFrameKindV4::HostLinkGrant => {
            decode_host_link_grant(payload).map(BrokerFrameV4::HostLinkGrant)
        }
        BrokerFrameKindV4::HostLinkCommit => {
            decode_host_link_commit(payload).map(BrokerFrameV4::HostLinkCommit)
        }
    }
}

fn encode_host_link_common(
    process: ProcessIdentityV4,
    binding_identity: [u8; 32],
    request_identity: [u8; 32],
    plan_identity: [u8; 32],
    closure_identity: [u8; 32],
    payload: &mut [u8],
) {
    process.encode(
        &mut payload
            [BROKER_V4_PROCESS_OFFSET..BROKER_V4_PROCESS_OFFSET + PROCESS_IDENTITY_V4_WIRE_LEN],
    );
    payload[BROKER_V4_BINDING_OFFSET..BROKER_V4_BINDING_OFFSET + IDENTITY_LEN]
        .copy_from_slice(&binding_identity);
    payload[HOST_LINK_REQUEST_OFFSET_V4..HOST_LINK_REQUEST_OFFSET_V4 + IDENTITY_LEN]
        .copy_from_slice(&request_identity);
    payload[HOST_LINK_PLAN_OFFSET_V4..HOST_LINK_PLAN_OFFSET_V4 + IDENTITY_LEN]
        .copy_from_slice(&plan_identity);
    payload[HOST_LINK_CLOSURE_OFFSET_V4..HOST_LINK_CLOSURE_OFFSET_V4 + IDENTITY_LEN]
        .copy_from_slice(&closure_identity);
}

fn encode_host_link_prepare(value: HostLinkPrepareV4, payload: &mut [u8]) {
    encode_host_link_common(
        value.process,
        value.binding_identity,
        value.request_identity,
        value.plan_identity,
        value.closure_identity,
        payload,
    );
}

fn decode_host_link_prepare(payload: &[u8]) -> Result<HostLinkPrepareV4, BrokerProtocolErrorV4> {
    HostLinkPrepareV4::new(
        decode_process(payload)?,
        digest_at(payload, BROKER_V4_BINDING_OFFSET),
        digest_at(payload, HOST_LINK_REQUEST_OFFSET_V4),
        digest_at(payload, HOST_LINK_PLAN_OFFSET_V4),
        digest_at(payload, HOST_LINK_CLOSURE_OFFSET_V4),
    )
}

fn encode_host_link_grant(value: HostLinkGrantV4, payload: &mut [u8]) {
    encode_host_link_common(
        value.process,
        value.binding_identity,
        value.request_identity,
        value.plan_identity,
        value.closure_identity,
        payload,
    );
    payload[HOST_LINK_GRANT_OFFSET_V4..HOST_LINK_GRANT_OFFSET_V4 + IDENTITY_LEN]
        .copy_from_slice(&value.grant_identity);
}

fn decode_host_link_grant(payload: &[u8]) -> Result<HostLinkGrantV4, BrokerProtocolErrorV4> {
    HostLinkGrantV4::new(
        decode_process(payload)?,
        digest_at(payload, BROKER_V4_BINDING_OFFSET),
        digest_at(payload, HOST_LINK_REQUEST_OFFSET_V4),
        digest_at(payload, HOST_LINK_PLAN_OFFSET_V4),
        digest_at(payload, HOST_LINK_CLOSURE_OFFSET_V4),
        digest_at(payload, HOST_LINK_GRANT_OFFSET_V4),
    )
}

fn encode_host_link_commit(value: HostLinkCommitV4, payload: &mut [u8]) {
    encode_host_link_common(
        value.process,
        value.binding_identity,
        value.request_identity,
        value.plan_identity,
        value.closure_identity,
        payload,
    );
    payload[HOST_LINK_GRANT_OFFSET_V4..HOST_LINK_GRANT_OFFSET_V4 + IDENTITY_LEN]
        .copy_from_slice(&value.grant_identity);
    payload[HOST_LINK_COMMIT_OUTPUT_SHA256_OFFSET_V4
        ..HOST_LINK_COMMIT_OUTPUT_SHA256_OFFSET_V4 + IDENTITY_LEN]
        .copy_from_slice(&value.output_sha256);
    payload[HOST_LINK_COMMIT_OUTPUT_LENGTH_OFFSET_V4..HOST_LINK_COMMIT_OUTPUT_MODE_OFFSET_V4]
        .copy_from_slice(&value.output_length.to_le_bytes());
    payload[HOST_LINK_COMMIT_OUTPUT_MODE_OFFSET_V4..HOST_LINK_COMMIT_RESERVED_OFFSET_V4]
        .copy_from_slice(&value.output_mode.to_le_bytes());
    payload[HOST_LINK_COMMIT_DURABLE_PLAN_OFFSET_V4
        ..HOST_LINK_COMMIT_DURABLE_PLAN_OFFSET_V4 + IDENTITY_LEN]
        .copy_from_slice(&value.durable_plan_identity);
}

fn decode_host_link_commit(payload: &[u8]) -> Result<HostLinkCommitV4, BrokerProtocolErrorV4> {
    if read_u32(payload, HOST_LINK_COMMIT_RESERVED_OFFSET_V4) != 0 {
        return Err(BrokerProtocolErrorV4::NonzeroHostLinkCommitReserved);
    }
    HostLinkCommitV4::new(
        decode_process(payload)?,
        digest_at(payload, BROKER_V4_BINDING_OFFSET),
        digest_at(payload, HOST_LINK_REQUEST_OFFSET_V4),
        digest_at(payload, HOST_LINK_PLAN_OFFSET_V4),
        digest_at(payload, HOST_LINK_CLOSURE_OFFSET_V4),
        digest_at(payload, HOST_LINK_GRANT_OFFSET_V4),
        digest_at(payload, HOST_LINK_COMMIT_OUTPUT_SHA256_OFFSET_V4),
        read_u64(payload, HOST_LINK_COMMIT_OUTPUT_LENGTH_OFFSET_V4),
        read_u32(payload, HOST_LINK_COMMIT_OUTPUT_MODE_OFFSET_V4),
        digest_at(payload, HOST_LINK_COMMIT_DURABLE_PLAN_OFFSET_V4),
    )
}

fn decode_process(payload: &[u8]) -> Result<ProcessIdentityV4, BrokerProtocolErrorV4> {
    ProcessIdentityV4::decode(
        &payload[BROKER_V4_PROCESS_OFFSET..BROKER_V4_PROCESS_OFFSET + PROCESS_IDENTITY_V4_WIRE_LEN],
    )
}

/// A field that did not remain continuous across the Broker V4 extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BrokerTranscriptFieldV4 {
    /// Stable PID and process start time.
    ProcessIdentity,
    /// Canonical CapabilityBinding V4 identity.
    CapabilityBindingIdentity,
    /// Fresh host-link request identity.
    HostLinkRequestIdentity,
    /// Prepared host-link plan identity.
    HostLinkPlanIdentity,
    /// Prepared host-link closure identity.
    HostLinkClosureIdentity,
    /// Host-link grant identity.
    HostLinkGrantIdentity,
}

impl fmt::Display for BrokerTranscriptFieldV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::ProcessIdentity => "process identity",
            Self::CapabilityBindingIdentity => "capability-binding identity",
            Self::HostLinkRequestIdentity => "host-link request identity",
            Self::HostLinkPlanIdentity => "host-link plan identity",
            Self::HostLinkClosureIdentity => "host-link closure identity",
            Self::HostLinkGrantIdentity => "host-link grant identity",
        };
        formatter.write_str(name)
    }
}

/// Why an ownership-consuming Broker V4 transition was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BrokerStateErrorV4 {
    /// A frame field did not match the binding or preceding validated frame.
    TranscriptMismatch {
        /// Field whose continuity check failed.
        field: BrokerTranscriptFieldV4,
    },
}

impl fmt::Display for BrokerStateErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TranscriptMismatch { field } => {
                write!(formatter, "Broker V4 transcript {field} mismatch")
            }
        }
    }
}

impl std::error::Error for BrokerStateErrorV4 {}

/// A rejected validation step together with the unconsumed prior validator.
#[derive(Debug, Eq, PartialEq)]
pub struct BrokerValidationRejectedV4<S> {
    state: S,
    error: BrokerStateErrorV4,
}

impl<S> BrokerValidationRejectedV4<S> {
    fn boxed(state: S, error: BrokerStateErrorV4) -> Box<Self> {
        Box::new(Self { state, error })
    }

    /// Returns the validation error without consuming the rejection.
    pub const fn error(&self) -> BrokerStateErrorV4 {
        self.error
    }

    /// Recovers the unmodified prior validator and validation error.
    pub fn into_parts(self) -> (S, BrokerStateErrorV4) {
        (self.state, self.error)
    }
}

/// Initial inert Broker V4 transcript validator awaiting HostLinkPrepare.
///
/// This validator does not implement `Clone` or `Copy`, which enforces ordering
/// for one Rust value. Its inputs are copyable and [`Self::new`] is public, so a
/// caller can construct equivalent validators and validate the same transcript
/// more than once. Consequently this type carries `AUTHORITY=none` and does not
/// provide global or durable replay protection. Production authority requires a
/// broker-owned [`BrokerReplayRegistryV4`] implementation and session capability.
///
/// ```compile_fail
/// use fe2o3_build_authority::BrokerTranscriptValidatorV4;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<BrokerTranscriptValidatorV4>();
/// ```
///
/// ```compile_fail
/// use fe2o3_build_authority::BrokerTranscriptValidatorV4;
/// fn requires_copy<T: Copy>() {}
/// requires_copy::<BrokerTranscriptValidatorV4>();
/// ```
///
/// ```compile_fail
/// use fe2o3_build_authority::{BrokerTranscriptValidatorV4, HostLinkGrantV4};
/// fn grant_before_prepare(state: BrokerTranscriptValidatorV4, grant: HostLinkGrantV4) {
///     let _ = state.validate_grant(grant);
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct BrokerTranscriptValidatorV4 {
    expected_binding: CapabilityBindingV4,
    expected_process: ProcessIdentityV4,
}

impl BrokerTranscriptValidatorV4 {
    /// Creates an inert V4 validator from an expected binding and process.
    ///
    /// Repeating this call with equal inputs is explicitly supported and creates
    /// an equivalent validator. Construction does not reserve a unique session.
    pub const fn new(
        expected_binding: CapabilityBindingV4,
        expected_process: ProcessIdentityV4,
    ) -> Self {
        Self {
            expected_binding,
            expected_process,
        }
    }

    /// Returns the expected capability binding.
    pub const fn expected_binding(&self) -> CapabilityBindingV4 {
        self.expected_binding
    }

    /// Returns the expected protected-process identity.
    pub const fn expected_process(&self) -> ProcessIdentityV4 {
        self.expected_process
    }

    /// Returns the fixed `AUTHORITY=none` semantic classification.
    pub const fn authority(&self) -> BrokerAuthorityV4 {
        BROKER_V4_AUTHORITY
    }

    /// Consumes this validator to validate one canonical HostLinkPrepare frame.
    pub fn validate_prepare(
        self,
        value: HostLinkPrepareV4,
    ) -> Result<PreparedHostLinkTranscriptV4, Box<BrokerValidationRejectedV4<Self>>> {
        if let Err(error) = validate_process_and_binding(
            self.expected_process,
            self.expected_binding,
            value.process(),
            value.binding_identity(),
        ) {
            return Err(BrokerValidationRejectedV4::boxed(self, error));
        }
        Ok(PreparedHostLinkTranscriptV4 {
            expected_binding: self.expected_binding,
            expected_process: self.expected_process,
            request_identity: value.request_identity(),
            plan_identity: value.plan_identity(),
            closure_identity: value.closure_identity(),
        })
    }
}

/// Move-only per-instance validator retaining one prepared closure.
///
/// This value records transcript continuity only. An equivalent validator can
/// be reconstructed from the same public inputs, so this value is not authority.
#[derive(Debug, Eq, PartialEq)]
pub struct PreparedHostLinkTranscriptV4 {
    expected_binding: CapabilityBindingV4,
    expected_process: ProcessIdentityV4,
    request_identity: [u8; 32],
    plan_identity: [u8; 32],
    closure_identity: [u8; 32],
}

impl PreparedHostLinkTranscriptV4 {
    /// Returns the validated request identity.
    pub const fn request_identity(&self) -> [u8; 32] {
        self.request_identity
    }

    /// Returns the validated plan identity.
    pub const fn plan_identity(&self) -> [u8; 32] {
        self.plan_identity
    }

    /// Returns the validated closure identity.
    pub const fn closure_identity(&self) -> [u8; 32] {
        self.closure_identity
    }

    /// Returns the replay-registry claim represented by this transcript prefix.
    pub fn session_claim(&self) -> BrokerSessionClaimV4 {
        BrokerSessionClaimV4::new(
            self.expected_binding.identity_sha256(),
            self.expected_process,
            self.request_identity,
            self.plan_identity,
            self.closure_identity,
        )
    }

    /// Returns the fixed `AUTHORITY=none` semantic classification.
    pub const fn authority(&self) -> BrokerAuthorityV4 {
        BROKER_V4_AUTHORITY
    }

    /// Consumes this validator to validate one exact grant frame.
    pub fn validate_grant(
        self,
        value: HostLinkGrantV4,
    ) -> Result<GrantedHostLinkTranscriptV4, Box<BrokerValidationRejectedV4<Self>>> {
        if let Err(error) = self.validate_continuity(
            value.process(),
            value.binding_identity(),
            value.request_identity(),
            value.plan_identity(),
            value.closure_identity(),
        ) {
            return Err(BrokerValidationRejectedV4::boxed(self, error));
        }
        Ok(GrantedHostLinkTranscriptV4 {
            expected_binding: self.expected_binding,
            expected_process: self.expected_process,
            request_identity: self.request_identity,
            plan_identity: self.plan_identity,
            closure_identity: self.closure_identity,
            grant_identity: value.grant_identity(),
        })
    }

    fn validate_continuity(
        &self,
        process: ProcessIdentityV4,
        binding_identity: [u8; 32],
        request_identity: [u8; 32],
        plan_identity: [u8; 32],
        closure_identity: [u8; 32],
    ) -> Result<(), BrokerStateErrorV4> {
        validate_process_and_binding(
            self.expected_process,
            self.expected_binding,
            process,
            binding_identity,
        )?;
        ensure_transcript(
            request_identity == self.request_identity,
            BrokerTranscriptFieldV4::HostLinkRequestIdentity,
        )?;
        ensure_transcript(
            plan_identity == self.plan_identity,
            BrokerTranscriptFieldV4::HostLinkPlanIdentity,
        )?;
        ensure_transcript(
            closure_identity == self.closure_identity,
            BrokerTranscriptFieldV4::HostLinkClosureIdentity,
        )
    }
}

/// Move-only per-instance validator retaining one matching grant frame.
///
/// This value does not implement `Clone` or `Copy`, so one instance cannot
/// validate two commits. Equivalent validator instances can still be recreated
/// from the same public inputs and can validate the same commit. This is inert
/// transcript state with `AUTHORITY=none`, not a globally one-shot grant.
///
/// ```compile_fail
/// use fe2o3_build_authority::GrantedHostLinkTranscriptV4;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<GrantedHostLinkTranscriptV4>();
/// ```
///
/// ```compile_fail
/// use fe2o3_build_authority::GrantedHostLinkTranscriptV4;
/// fn requires_copy<T: Copy>() {}
/// requires_copy::<GrantedHostLinkTranscriptV4>();
/// ```
///
/// ```compile_fail
/// use fe2o3_build_authority::{GrantedHostLinkTranscriptV4, HostLinkCommitV4};
/// fn consume_twice(
///     validated_grant: GrantedHostLinkTranscriptV4,
///     first: HostLinkCommitV4,
///     second: HostLinkCommitV4,
/// ) {
///     let _ = validated_grant.validate_commit(first);
///     let _ = validated_grant.validate_commit(second);
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct GrantedHostLinkTranscriptV4 {
    expected_binding: CapabilityBindingV4,
    expected_process: ProcessIdentityV4,
    request_identity: [u8; 32],
    plan_identity: [u8; 32],
    closure_identity: [u8; 32],
    grant_identity: [u8; 32],
}

impl GrantedHostLinkTranscriptV4 {
    /// Returns the validated grant identity.
    pub const fn grant_identity(&self) -> [u8; 32] {
        self.grant_identity
    }

    /// Returns the replay-registry claim represented by this transcript prefix.
    pub fn session_claim(&self) -> BrokerSessionClaimV4 {
        BrokerSessionClaimV4::new(
            self.expected_binding.identity_sha256(),
            self.expected_process,
            self.request_identity,
            self.plan_identity,
            self.closure_identity,
        )
    }

    /// Returns the fixed `AUTHORITY=none` semantic classification.
    pub const fn authority(&self) -> BrokerAuthorityV4 {
        BROKER_V4_AUTHORITY
    }

    /// Consumes this validator to validate one matching commit frame.
    pub fn validate_commit(
        self,
        value: HostLinkCommitV4,
    ) -> Result<CompletedBrokerTranscriptV4, Box<BrokerValidationRejectedV4<Self>>> {
        if let Err(error) = self.validate_continuity(value) {
            return Err(BrokerValidationRejectedV4::boxed(self, error));
        }
        Ok(CompletedBrokerTranscriptV4 {
            binding_identity: self.expected_binding.identity_sha256(),
            process: self.expected_process,
            request_identity: self.request_identity,
            plan_identity: self.plan_identity,
            closure_identity: self.closure_identity,
            grant_identity: self.grant_identity,
            output_sha256: value.output_sha256(),
            output_length: value.output_length(),
            output_mode: value.output_mode(),
            durable_plan_identity: value.durable_plan_identity(),
        })
    }

    fn validate_continuity(&self, value: HostLinkCommitV4) -> Result<(), BrokerStateErrorV4> {
        validate_process_and_binding(
            self.expected_process,
            self.expected_binding,
            value.process(),
            value.binding_identity(),
        )?;
        ensure_transcript(
            value.request_identity() == self.request_identity,
            BrokerTranscriptFieldV4::HostLinkRequestIdentity,
        )?;
        ensure_transcript(
            value.plan_identity() == self.plan_identity,
            BrokerTranscriptFieldV4::HostLinkPlanIdentity,
        )?;
        ensure_transcript(
            value.closure_identity() == self.closure_identity,
            BrokerTranscriptFieldV4::HostLinkClosureIdentity,
        )?;
        ensure_transcript(
            value.grant_identity() == self.grant_identity,
            BrokerTranscriptFieldV4::HostLinkGrantIdentity,
        )
    }
}

/// Inert terminal transcript produced after a matching HostLinkCommit is validated.
///
/// This value records only that one validator instance observed internally
/// consistent Prepare, Grant, and Commit frames in order. Equivalent validators
/// may produce equal terminal transcripts. It provides no freshness, durable
/// replay exclusion, execution authority, or publication authority. A production
/// broker must combine it with a capability from [`BrokerReplayRegistryV4`].
#[derive(Debug, Eq, PartialEq)]
pub struct CompletedBrokerTranscriptV4 {
    binding_identity: [u8; 32],
    process: ProcessIdentityV4,
    request_identity: [u8; 32],
    plan_identity: [u8; 32],
    closure_identity: [u8; 32],
    grant_identity: [u8; 32],
    output_sha256: [u8; 32],
    output_length: u64,
    output_mode: u32,
    durable_plan_identity: [u8; 32],
}

impl CompletedBrokerTranscriptV4 {
    /// Returns the validated V4 capability-binding identity.
    pub const fn binding_identity(&self) -> [u8; 32] {
        self.binding_identity
    }

    /// Returns the protected-process identity.
    pub const fn process(&self) -> ProcessIdentityV4 {
        self.process
    }

    /// Returns the validated request identity.
    pub const fn request_identity(&self) -> [u8; 32] {
        self.request_identity
    }

    /// Returns the validated host-link plan identity.
    pub const fn plan_identity(&self) -> [u8; 32] {
        self.plan_identity
    }

    /// Returns the validated host-link closure identity.
    pub const fn closure_identity(&self) -> [u8; 32] {
        self.closure_identity
    }

    /// Returns the validated grant identity.
    pub const fn grant_identity(&self) -> [u8; 32] {
        self.grant_identity
    }

    /// Returns the declared output SHA-256 identity.
    pub const fn output_sha256(&self) -> [u8; 32] {
        self.output_sha256
    }

    /// Returns the declared output length.
    pub const fn output_length(&self) -> u64 {
        self.output_length
    }

    /// Returns the declared output mode.
    pub const fn output_mode(&self) -> u32 {
        self.output_mode
    }

    /// Returns the durable publication-plan identity.
    pub const fn durable_plan_identity(&self) -> [u8; 32] {
        self.durable_plan_identity
    }

    /// Returns the replay-registry claim represented by this transcript.
    pub const fn session_claim(&self) -> BrokerSessionClaimV4 {
        BrokerSessionClaimV4::new(
            self.binding_identity,
            self.process,
            self.request_identity,
            self.plan_identity,
            self.closure_identity,
        )
    }

    /// Returns the unconditionally empty publication-rights set.
    pub const fn publication_rights(&self) -> PublicationRightsV1 {
        PublicationRightsV1::NONE
    }

    /// Returns the fixed `AUTHORITY=none` semantic classification.
    pub const fn authority(&self) -> BrokerAuthorityV4 {
        BROKER_V4_AUTHORITY
    }
}

/// Copyable replay key for production broker session reservation.
///
/// This value is inert and intentionally reproducible. Possessing it does not
/// prove that a session was reserved and grants no execution or publication
/// authority. Only a production [`BrokerReplayRegistryV4`] implementation can
/// exchange it for a runtime-owned session capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrokerSessionClaimV4 {
    binding_identity: [u8; 32],
    process: ProcessIdentityV4,
    request_identity: [u8; 32],
    plan_identity: [u8; 32],
    closure_identity: [u8; 32],
}

impl BrokerSessionClaimV4 {
    const fn new(
        binding_identity: [u8; 32],
        process: ProcessIdentityV4,
        request_identity: [u8; 32],
        plan_identity: [u8; 32],
        closure_identity: [u8; 32],
    ) -> Self {
        Self {
            binding_identity,
            process,
            request_identity,
            plan_identity,
            closure_identity,
        }
    }

    /// Returns the V4 capability-binding identity.
    pub const fn binding_identity(self) -> [u8; 32] {
        self.binding_identity
    }

    /// Returns the protected-process identity.
    pub const fn process(self) -> ProcessIdentityV4 {
        self.process
    }

    /// Returns the host-link request identity.
    pub const fn request_identity(self) -> [u8; 32] {
        self.request_identity
    }

    /// Returns the host-link plan identity.
    pub const fn plan_identity(self) -> [u8; 32] {
        self.plan_identity
    }

    /// Returns the host-link closure identity.
    pub const fn closure_identity(self) -> [u8; 32] {
        self.closure_identity
    }

    /// Returns the fixed `AUTHORITY=none` semantic classification.
    pub const fn authority(self) -> BrokerAuthorityV4 {
        BROKER_V4_AUTHORITY
    }
}

/// Integration seam for broker-owned durable replay exclusion.
///
/// This crate deliberately provides no implementation, default registry, or
/// always-allow fallback. A production implementation is a trusted runtime
/// boundary: it must atomically reserve each claim exactly once, persist replay
/// state across process restarts, issue an unforgeable move-only session
/// capability, bind completion to that reservation, and consume the capability
/// exactly once when recording the terminal transcript.
pub trait BrokerReplayRegistryV4 {
    /// Runtime-owned proof that one session claim was durably reserved.
    ///
    /// Production implementations must use an unforgeable type that does not
    /// implement `Clone` or `Copy`.
    type SessionCapability;

    /// Registry-specific reservation or durable-commit error.
    type Error;

    /// Atomically reserves a fresh session and returns its runtime capability.
    ///
    /// The production broker must complete this reservation before treating any
    /// grant frame as permission to invoke the host linker.
    fn reserve_session(
        &mut self,
        claim: BrokerSessionClaimV4,
    ) -> Result<Self::SessionCapability, Self::Error>;

    /// Atomically records completion and consumes the reserved session capability.
    ///
    /// The implementation must reject a transcript that does not match the
    /// reservation represented by `capability` and must reject every replay.
    fn commit_session(
        &mut self,
        capability: Self::SessionCapability,
        transcript: &CompletedBrokerTranscriptV4,
    ) -> Result<(), Self::Error>;
}

fn validate_process_and_binding(
    expected_process: ProcessIdentityV4,
    expected_binding: CapabilityBindingV4,
    process: ProcessIdentityV4,
    binding_identity: [u8; 32],
) -> Result<(), BrokerStateErrorV4> {
    ensure_transcript(
        process == expected_process,
        BrokerTranscriptFieldV4::ProcessIdentity,
    )?;
    ensure_transcript(
        binding_identity == expected_binding.identity_sha256(),
        BrokerTranscriptFieldV4::CapabilityBindingIdentity,
    )
}

fn ensure_transcript(
    condition: bool,
    field: BrokerTranscriptFieldV4,
) -> Result<(), BrokerStateErrorV4> {
    if condition {
        Ok(())
    } else {
        Err(BrokerStateErrorV4::TranscriptMismatch { field })
    }
}

/// Why a Broker V4 value or canonical frame was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BrokerProtocolErrorV4 {
    /// A frame ended before its complete header.
    TruncatedHeader {
        /// Observed byte length.
        actual: usize,
    },
    /// The frame magic was not Broker V4.
    InvalidMagic,
    /// The frame version was not Broker V4.
    UnsupportedVersion {
        /// Observed version.
        actual: u16,
    },
    /// The frame type was not assigned.
    UnknownFrameType {
        /// Observed type number.
        actual: u16,
    },
    /// The declared payload length was not canonical for the frame type.
    InvalidPayloadLength {
        /// Parsed frame type.
        kind: BrokerFrameKindV4,
        /// Required payload length.
        expected: usize,
        /// Declared payload length.
        actual: u32,
    },
    /// The sequence was not canonical for the frame type.
    InvalidSequence {
        /// Parsed frame type.
        kind: BrokerFrameKindV4,
        /// Required sequence.
        expected: u32,
        /// Observed sequence.
        actual: u32,
    },
    /// Header flags were nonzero.
    UnsupportedFlags {
        /// Observed flags.
        actual: u32,
    },
    /// The frame had missing or trailing bytes.
    InvalidEncodedLength {
        /// Required complete frame length.
        expected: usize,
        /// Observed complete frame length.
        actual: usize,
    },
    /// A standalone binding had missing or trailing bytes.
    InvalidBindingLength {
        /// Observed binding length.
        actual: usize,
    },
    /// A required identity was all zero.
    ZeroIdentity {
        /// Zero-valued field.
        field: BrokerIdentityFieldV4,
    },
    /// A process identifier was zero.
    ZeroProcessId,
    /// A process start time was zero.
    ZeroProcessStartTime,
    /// Process-identity reserved bytes were nonzero.
    NonzeroProcessReserved,
    /// The target was not assigned to Broker V4.
    UnknownTarget {
        /// Observed target number.
        actual: u16,
    },
    /// Capability-binding reserved bytes were nonzero.
    NonzeroBindingReserved,
    /// Any nonzero publication right is forbidden in Broker V4.
    PublicationAuthorityForbidden {
        /// Observed rights bits.
        actual: u32,
    },
    /// The admitted host-link output length was zero.
    ZeroHostLinkOutputLength,
    /// The admitted host-link output mode was not canonical.
    InvalidHostLinkOutputMode {
        /// Observed mode.
        actual: u32,
    },
    /// HostLinkCommit reserved bytes were nonzero.
    NonzeroHostLinkCommitReserved,
}

impl fmt::Display for BrokerProtocolErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader { actual } => {
                write!(formatter, "truncated Broker V4 header ({actual} bytes)")
            }
            Self::InvalidMagic => formatter.write_str("invalid Broker V4 magic"),
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "unsupported Broker V4 version {actual}")
            }
            Self::UnknownFrameType { actual } => {
                write!(formatter, "unknown Broker V4 frame type {actual}")
            }
            Self::InvalidPayloadLength {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "invalid {kind:?} payload length {actual}; expected {expected}"
            ),
            Self::InvalidSequence {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "invalid {kind:?} sequence {actual}; expected {expected}"
            ),
            Self::UnsupportedFlags { actual } => {
                write!(formatter, "unsupported Broker V4 flags {actual:#x}")
            }
            Self::InvalidEncodedLength { expected, actual } => write!(
                formatter,
                "invalid Broker V4 frame length {actual}; expected {expected}"
            ),
            Self::InvalidBindingLength { actual } => write!(
                formatter,
                "invalid CapabilityBinding V4 length {actual}; expected {BROKER_V4_BINDING_WIRE_LEN}"
            ),
            Self::ZeroIdentity { field } => write!(formatter, "zero {field} identity"),
            Self::ZeroProcessId => formatter.write_str("zero Broker V4 process identifier"),
            Self::ZeroProcessStartTime => formatter.write_str("zero Broker V4 process start time"),
            Self::NonzeroProcessReserved => {
                formatter.write_str("nonzero Broker V4 process reserved bytes")
            }
            Self::UnknownTarget { actual } => write!(formatter, "unknown target {actual}"),
            Self::NonzeroBindingReserved => {
                formatter.write_str("nonzero CapabilityBinding V4 reserved bytes")
            }
            Self::PublicationAuthorityForbidden { actual } => write!(
                formatter,
                "publication authority {actual:#x} is forbidden in Broker V4"
            ),
            Self::ZeroHostLinkOutputLength => formatter.write_str("zero host-link output length"),
            Self::InvalidHostLinkOutputMode { actual } => write!(
                formatter,
                "invalid host-link output mode {actual:#o}; expected {HOST_LINK_OUTPUT_MODE_V4:#o}"
            ),
            Self::NonzeroHostLinkCommitReserved => {
                formatter.write_str("nonzero HostLinkCommit V4 reserved bytes")
            }
        }
    }
}

impl std::error::Error for BrokerProtocolErrorV4 {}

fn validate_host_link_identities(
    binding_identity: [u8; 32],
    request_identity: [u8; 32],
    plan_identity: [u8; 32],
    closure_identity: [u8; 32],
) -> Result<(), BrokerProtocolErrorV4> {
    for (field, identity) in [
        (BrokerIdentityFieldV4::CapabilityBinding, binding_identity),
        (BrokerIdentityFieldV4::HostLinkRequest, request_identity),
        (BrokerIdentityFieldV4::HostLinkPlan, plan_identity),
        (BrokerIdentityFieldV4::HostLinkClosure, closure_identity),
    ] {
        validate_identity(identity, field)?;
    }
    Ok(())
}

fn validate_identity(
    value: [u8; 32],
    field: BrokerIdentityFieldV4,
) -> Result<(), BrokerProtocolErrorV4> {
    if value == [0; 32] {
        Err(BrokerProtocolErrorV4::ZeroIdentity { field })
    } else {
        Ok(())
    }
}

fn digest_at(input: &[u8], offset: usize) -> [u8; 32] {
    input[offset..offset + IDENTITY_LEN]
        .try_into()
        .expect("validated Broker V4 wire bounds")
}

fn read_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        input[offset..offset + 2]
            .try_into()
            .expect("validated Broker V4 wire bounds"),
    )
}

fn read_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        input[offset..offset + 4]
            .try_into()
            .expect("validated Broker V4 wire bounds"),
    )
}

fn read_u64(input: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        input[offset..offset + 8]
            .try_into()
            .expect("validated Broker V4 wire bounds"),
    )
}
