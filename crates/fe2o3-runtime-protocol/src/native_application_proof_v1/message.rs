//! Canonical native-only frames; live owners separately enforce sender and phase.
use super::*;

const MAGIC: &[u8; 8] = b"F3NPMS1\0";
const MESSAGE_HEADER: usize = 64;
const EMPTY_BYTES: usize = MESSAGE_HEADER + 32;
pub const NATIVE_APPLICATION_PROOF_MAX_PACKET_BYTES_V1: usize = EMPTY_BYTES + EVIDENCE_BYTES;
const MAX_BYTES: usize = NATIVE_APPLICATION_PROOF_MAX_PACKET_BYTES_V1;

/// No Release, settlement or native-launch message exists in this protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum NativeApplicationProofKindV1 {
    Active = 1,
    Request = 2,
    Proved = 3,
    Probe = 4,
    Retained = 5,
    Rejected = 6,
}
use NativeApplicationProofKindV1 as Kind;
impl Kind {
    fn from_wire(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Active),
            2 => Ok(Self::Request),
            3 => Ok(Self::Proved),
            4 => Ok(Self::Probe),
            5 => Ok(Self::Retained),
            6 => Ok(Self::Rejected),
            _ => Err(Error::Kind),
        }
    }
    fn nested_quote(self) -> Option<Quote> {
        match self {
            Self::Request => Some(Inputs::decoding_quote()),
            Self::Proved | Self::Retained => Some(Evidence::decoding_quote()),
            _ => None,
        }
    }
}

/// Move-only fixed-capacity native frame. It describes exactly two ordered sealed
/// input FDs for Request and zero FDs otherwise; OS custody is not established here.
/// Decoders bind a supplied native session but do not authenticate its provenance.
/// Live transport owners must additionally enforce request-once and strict monotonic
/// probe sequence, exact sender credentials, original controller pidfd and phase.
///
/// ```compile_fail
/// use fe2o3_runtime_protocol::{NativeApplicationProofMessageV1 as Native,
///     WorkerV3ApplicationProofMessageV1 as Legacy};
/// fn downgrade(value: Native) -> Legacy { value.into() }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationProofMessageV1 {
    kind: Kind,
    length: usize,
    bytes: [u8; MAX_BYTES],
}
type Message = NativeApplicationProofMessageV1;
impl Message {
    const RETAINED: usize = size_of::<Self>() + size_of::<Storage>();
    pub fn construction_quote(kind: Kind, session: &Session) -> Quote {
        let payload = match kind {
            Kind::Request => Inputs::RETAINED,
            Kind::Proved | Kind::Retained => Evidence::RETAINED,
            _ => 0,
        };
        quote(session.retained_storage() + payload, Self::RETAINED)
    }
    pub fn decoding_quote(kind: Kind, byte_len: usize, session: &Session) -> Result<Quote> {
        if !(EMPTY_BYTES..=MAX_BYTES).contains(&byte_len) {
            return Err(Error::Length);
        }
        let mut q = quote(byte_len + session.retained_storage(), Self::RETAINED);
        if let Some(inner) = kind.nested_quote() {
            q.work += inner.work;
            q.scratch += inner.scratch;
        }
        Ok(q)
    }
    pub fn active(session: &Session, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Self::construct(Kind::Active, session, 1, &[], budget)
    }
    pub fn request(
        session: &Session,
        inputs: &Inputs,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(
            true,
            Self::construction_quote(Kind::Request, session),
            budget,
            |_| {
                inputs.matches_session(session)?;
                Ok((
                    Self::encode(Kind::Request, session, 1, inputs.canonical_bytes())?,
                    Storage(Self::RETAINED),
                ))
            },
        )
    }
    pub fn proved(
        session: &Session,
        evidence: &Evidence,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::evidence(Kind::Proved, session, 1, evidence, budget)
    }
    pub fn probe(
        session: &Session,
        sequence: u64,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::construct(Kind::Probe, session, sequence, &[], budget)
    }
    pub fn retained(
        session: &Session,
        sequence: u64,
        evidence: &Evidence,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::evidence(Kind::Retained, session, sequence, evidence, budget)
    }
    pub fn rejected(
        session: &Session,
        sequence: u64,
        reason: &str,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::construct(Kind::Rejected, session, sequence, reason.as_bytes(), budget)
    }
    fn evidence(
        kind: Kind,
        session: &Session,
        sequence: u64,
        evidence: &Evidence,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(
            true,
            Self::construction_quote(kind, session),
            budget,
            |_| {
                evidence.matches_session(session)?;
                Ok((
                    Self::encode(kind, session, sequence, evidence.canonical_bytes())?,
                    Storage(Self::RETAINED),
                ))
            },
        )
    }
    fn construct(
        kind: Kind,
        session: &Session,
        sequence: u64,
        payload: &[u8],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(
            true,
            Self::construction_quote(kind, session),
            budget,
            |_| {
                Ok((
                    Self::encode(kind, session, sequence, payload)?,
                    Storage(Self::RETAINED),
                ))
            },
        )
    }
    fn encode(kind: Kind, session: &Session, sequence: u64, payload: &[u8]) -> Result<Self> {
        validate_shape(kind, sequence, payload)?;
        let length = EMPTY_BYTES + payload.len();
        let mut bytes = [0; MAX_BYTES];
        write_header(&mut bytes[..length], MAGIC);
        bytes[10] = kind as u8;
        bytes[11] = if kind == Kind::Request { 2 } else { 0 };
        bytes[16..48].copy_from_slice(&session.identity());
        bytes[48..56].copy_from_slice(&sequence.to_le_bytes());
        bytes[MESSAGE_HEADER..length - 32].copy_from_slice(payload);
        seal(&mut bytes[..length]);
        Ok(Self {
            kind,
            length,
            bytes,
        })
    }
    pub fn decode(
        bytes: &[u8],
        session: &Session,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        // Even malformed kind/length attempts consume work on the original account.
        budget.charge_work(ENTRY)?;
        if !(EMPTY_BYTES..=MAX_BYTES).contains(&bytes.len()) {
            return Err(Error::Length);
        }
        let kind = Kind::from_wire(bytes[10])?;
        let q = Self::decoding_quote(kind, bytes.len(), session)?;
        budget.with_prepaid_scope(q.input_floor, 0, WORK - ENTRY, SCRATCH, |b| {
            if &bytes[..8] != MAGIC
                || bytes[8..10] != 1u16.to_le_bytes()
                || bytes[11] != if kind == Kind::Request { 2 } else { 0 }
                || bytes[12..16] != (bytes.len() as u32).to_le_bytes()
                || bytes[56..MESSAGE_HEADER] != [0; 8]
            {
                return Err(Error::Header);
            }
            if identity(bytes, 16) != session.identity()
                || bytes[bytes.len() - 32..] != checksum(&bytes[..bytes.len() - 32])
            {
                return Err(Error::Identity);
            }
            let sequence = u64::from_le_bytes(bytes[48..56].try_into().unwrap());
            let payload = &bytes[MESSAGE_HEADER..bytes.len() - 32];
            validate_shape(kind, sequence, payload)?;
            match kind {
                Kind::Request => {
                    let (inputs, storage) = Inputs::decode(payload, b)?;
                    b.reserve_storage(storage.retained_storage())?;
                    inputs.matches_session(session)?;
                }
                Kind::Proved | Kind::Retained => {
                    let (evidence, storage) = Evidence::decode(payload, b)?;
                    b.reserve_storage(storage.retained_storage())?;
                    evidence.matches_session(session)?;
                }
                _ => {}
            }
            let value = Self::encode(kind, session, sequence, payload)?;
            if value.canonical_bytes() != bytes {
                return Err(Error::Identity);
            }
            Ok((value, Storage(Self::RETAINED)))
        })
    }
    /// Recovering a payload is a separate metered decode on the same account.
    pub fn decode_inputs(&self, budget: &mut Budget<'_>) -> Result<(Inputs, Storage)> {
        budget.charge_work(ENTRY)?;
        if self.kind != Kind::Request {
            return Err(Error::Kind);
        }
        Inputs::decode(self.body(), budget)
    }
    pub fn decode_evidence(&self, budget: &mut Budget<'_>) -> Result<(Evidence, Storage)> {
        budget.charge_work(ENTRY)?;
        if !matches!(self.kind, Kind::Proved | Kind::Retained) {
            return Err(Error::Kind);
        }
        Evidence::decode(self.body(), budget)
    }
    pub const fn kind(&self) -> Kind {
        self.kind
    }
    pub fn session_identity(&self) -> [u8; 32] {
        identity(&self.bytes, 16)
    }
    pub fn sequence(&self) -> u64 {
        u64::from_le_bytes(self.bytes[48..56].try_into().unwrap())
    }
    pub const fn required_rights(&self) -> usize {
        self.bytes[11] as usize
    }
    pub fn body(&self) -> &[u8] {
        &self.bytes[MESSAGE_HEADER..self.length - 32]
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }
    pub const fn authenticates_sender(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}
fn validate_shape(kind: Kind, sequence: u64, payload: &[u8]) -> Result<()> {
    if sequence == 0 || sequence == u64::MAX {
        return Err(Error::Sequence);
    }
    let valid = match kind {
        Kind::Active => sequence == 1 && payload.is_empty(),
        Kind::Request => sequence == 1 && payload.len() == INPUT_BYTES,
        Kind::Proved => sequence == 1 && payload.len() == EVIDENCE_BYTES,
        Kind::Probe => sequence >= 2 && payload.is_empty(),
        Kind::Retained => sequence >= 2 && payload.len() == EVIDENCE_BYTES,
        Kind::Rejected => {
            !payload.is_empty() && payload.len() <= 512 && std::str::from_utf8(payload).is_ok()
        }
    };
    if valid { Ok(()) } else { Err(Error::Sequence) }
}
