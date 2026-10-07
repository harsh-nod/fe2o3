//! Native, inert session frames. Original OS owners establish sender and freshness.
use super::*;

const MAGIC: &[u8; 8] = b"F3NASE1\0";
const SESSION_HEADER: usize = 112;
const EMPTY_BYTES: usize = SESSION_HEADER + 32;
pub const NATIVE_APPLICATION_SESSION_MAX_BYTES_V1: usize = EMPTY_BYTES + BINDING_BYTES;
const MAX_BYTES: usize = NATIVE_APPLICATION_SESSION_MAX_BYTES_V1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum NativeApplicationSessionKindV1 {
    Hello = 1,
    Challenge = 2,
    Accept = 3,
    Ready = 4,
    CustodianReady = 5,
}
type Kind = NativeApplicationSessionKindV1;
impl Kind {
    fn from_wire(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Hello),
            2 => Ok(Self::Challenge),
            3 => Ok(Self::Accept),
            4 => Ok(Self::Ready),
            5 => Ok(Self::CustodianReady),
            _ => Err(Error::Kind),
        }
    }
    const fn bytes(self) -> usize {
        EMPTY_BYTES
            + match self {
                Self::Hello => INPUT_BYTES,
                Self::Challenge => BINDING_BYTES,
                Self::CustodianReady => NATIVE_APPLICATION_PROOF_SESSION_BYTES_V1,
                _ => 0,
            }
    }
}

/// Equality data, never authenticated sender, nonce freshness or proof custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeApplicationSessionTranscriptV1 {
    app_nonce: [u8; 32],
    root_nonce: [u8; 32],
    binding: [u8; 32],
}
type Transcript = NativeApplicationSessionTranscriptV1;
impl Transcript {
    /// Inert equality data only; this does not observe nonce freshness or a sender.
    pub const fn construction_quote() -> Quote {
        fixed_quote(0, size_of::<Self>() + size_of::<Storage>())
    }
    pub const fn decoding_quote() -> Quote {
        fixed_quote(96, size_of::<Self>() + size_of::<Storage>())
    }
    pub fn from_untrusted_parts(
        app_nonce: [u8; 32],
        root_nonce: [u8; 32],
        binding: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(true, Self::construction_quote(), budget, |_| {
            Ok((
                Self::new(app_nonce, root_nonce, binding)?,
                Storage(size_of::<Self>() + size_of::<Storage>()),
            ))
        })
    }
    /// The fixed 96-byte tuple has no standalone authentication. Enclosing native
    /// capsule/session framing must bind its exact bytes and independently admit custody.
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(bytes.len() == 96, Self::decoding_quote(), budget, |_| {
            if bytes.len() != 96 {
                return Err(Error::Length);
            }
            Ok((
                Self::new(
                    bytes[..32].try_into().unwrap(),
                    bytes[32..64].try_into().unwrap(),
                    bytes[64..].try_into().unwrap(),
                )?,
                Storage(size_of::<Self>() + size_of::<Storage>()),
            ))
        })
    }
    pub fn canonical_bytes(self) -> [u8; 96] {
        let mut bytes = [0; 96];
        bytes[..32].copy_from_slice(&self.app_nonce);
        bytes[32..64].copy_from_slice(&self.root_nonce);
        bytes[64..].copy_from_slice(&self.binding);
        bytes
    }
    pub const fn authenticates_sender(self) -> bool {
        false
    }
    pub(super) fn new(
        app_nonce: [u8; 32],
        root_nonce: [u8; 32],
        binding: [u8; 32],
    ) -> Result<Self> {
        if app_nonce == [0; 32]
            || root_nonce == [0; 32]
            || binding == [0; 32]
            || app_nonce == root_nonce
        {
            return Err(Error::Transcript);
        }
        Ok(Self {
            app_nonce,
            root_nonce,
            binding,
        })
    }
    pub const fn app_nonce(self) -> [u8; 32] {
        self.app_nonce
    }
    pub const fn root_nonce(self) -> [u8; 32] {
        self.root_nonce
    }
    pub const fn binding(self) -> [u8; 32] {
        self.binding
    }
}

/// Fixed-capacity inert frame. Encoding borrows native owners without cloning them.
/// Decoding validates native payloads on the same caller account; it creates no
/// admitted application session. Recovering a payload is a separate metered decode.
///
/// ```compile_fail
/// use fe2o3_runtime_protocol::{NativeApplicationSessionMessageV1 as Native,
///     WorkerV3ApplicationSessionMessageV1 as Legacy};
/// fn downgrade(value: Native) -> Legacy { value.into() }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationSessionMessageV1 {
    kind: Kind,
    transcript: Transcript,
    bytes: [u8; MAX_BYTES],
}
type Message = NativeApplicationSessionMessageV1;
impl Message {
    const RETAINED: usize = size_of::<Self>() + size_of::<Storage>();
    pub const fn hello_quote() -> Quote {
        fixed_quote(Inputs::RETAINED, Self::RETAINED)
    }
    pub const fn challenge_quote() -> Quote {
        fixed_quote(Binding::RETAINED, Self::RETAINED)
    }
    pub const fn empty_quote() -> Quote {
        fixed_quote(0, Self::RETAINED)
    }
    pub const fn custodian_ready_quote() -> Quote {
        fixed_quote(NativeApplicationProofSessionV1::RETAINED, Self::RETAINED)
    }
    /// Describes exactly one original controller pidfd transfer. Admission of
    /// that pidfd and the pinned native deployment remains a live-owner duty.
    pub fn custodian_ready(
        session: &NativeApplicationProofSessionV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(true, Self::custodian_ready_quote(), budget, |_| {
            Ok((
                Self::encode(
                    Kind::CustodianReady,
                    session.transcript(),
                    session.canonical_bytes(),
                ),
                Storage(Self::RETAINED),
            ))
        })
    }
    pub fn hello(
        inputs: &Inputs,
        app_nonce: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(true, Self::hello_quote(), budget, |_| {
            if app_nonce == [0; 32] {
                return Err(Error::Transcript);
            }
            Ok((
                Self::encode(
                    Kind::Hello,
                    Transcript {
                        app_nonce,
                        root_nonce: [0; 32],
                        binding: [0; 32],
                    },
                    inputs.canonical_bytes(),
                ),
                Storage(Self::RETAINED),
            ))
        })
    }
    pub fn challenge(
        binding: &Binding,
        app_nonce: [u8; 32],
        root_nonce: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(true, Self::challenge_quote(), budget, |_| {
            let transcript =
                Transcript::new(app_nonce, root_nonce, *binding.identity().as_bytes())?;
            Ok((
                Self::encode(Kind::Challenge, transcript, binding.canonical_bytes()),
                Storage(Self::RETAINED),
            ))
        })
    }
    pub fn accept(transcript: Transcript, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Self::empty(Kind::Accept, transcript, budget)
    }
    pub fn ready(transcript: Transcript, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Self::empty(Kind::Ready, transcript, budget)
    }
    fn empty(
        kind: Kind,
        transcript: Transcript,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(true, Self::empty_quote(), budget, |_| {
            let transcript = Transcript::new(
                transcript.app_nonce,
                transcript.root_nonce,
                transcript.binding,
            )?;
            Ok((Self::encode(kind, transcript, &[]), Storage(Self::RETAINED)))
        })
    }
    fn encode(kind: Kind, transcript: Transcript, payload: &[u8]) -> Self {
        let mut bytes = [0; MAX_BYTES];
        let length = kind.bytes();
        write_header(&mut bytes[..length], MAGIC);
        bytes[10] = kind as u8;
        bytes[16..48].copy_from_slice(&transcript.app_nonce);
        bytes[48..80].copy_from_slice(&transcript.root_nonce);
        bytes[80..SESSION_HEADER].copy_from_slice(&transcript.binding);
        bytes[SESSION_HEADER..length - 32].copy_from_slice(payload);
        seal(&mut bytes[..length]);
        Self {
            kind,
            transcript,
            bytes,
        }
    }
    pub fn decoding_quote(kind: Kind) -> Quote {
        let nested = match kind {
            Kind::Hello => Some(Inputs::decoding_quote()),
            Kind::Challenge => Some(Binding::decoding_quote()),
            Kind::CustodianReady => Some(NativeApplicationProofSessionV1::decoding_quote()),
            _ => None,
        };
        Quote {
            input_floor: kind.bytes(),
            work: OUTER_WORK + nested.map_or(0, |q| q.work),
            outer_work: OUTER_WORK,
            scratch: OUTER_SCRATCH + nested.map_or(0, |q| q.scratch),
            retained: Self::RETAINED,
            additional: Self::RETAINED,
        }
    }
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        budget.charge_work(ENTRY)?;
        let kind = Kind::from_wire(*bytes.get(10).ok_or(Error::Length)?)?;
        let q = Self::decoding_quote(kind);
        budget.with_prepaid_scope(
            if bytes.len() == kind.bytes() {
                q.input_floor
            } else {
                0
            },
            0,
            q.outer_work - ENTRY,
            OUTER_SCRATCH,
            |budget| {
                let length = kind.bytes();
                if bytes.len() != length {
                    return Err(Error::Length);
                }
                if &bytes[..8] != MAGIC
                    || bytes[8..10] != 1u16.to_le_bytes()
                    || bytes[11] != 0
                    || bytes[12..16] != (length as u32).to_le_bytes()
                {
                    return Err(Error::Header);
                }
                if bytes[length - 32..] != checksum(&bytes[..length - 32]) {
                    return Err(Error::Identity);
                }
                let app_nonce = bytes[16..48].try_into().unwrap();
                let root_nonce = bytes[48..80].try_into().unwrap();
                let identity = bytes[80..SESSION_HEADER].try_into().unwrap();
                let transcript = if kind == Kind::Hello {
                    if app_nonce == [0; 32] || root_nonce != [0; 32] || identity != [0; 32] {
                        return Err(Error::Transcript);
                    }
                    let _ = Inputs::decode(&bytes[SESSION_HEADER..length - 32], budget)?;
                    Transcript {
                        app_nonce,
                        root_nonce,
                        binding: identity,
                    }
                } else {
                    let transcript = Transcript::new(app_nonce, root_nonce, identity)?;
                    if kind == Kind::Challenge {
                        let (binding, _) =
                            Binding::decode(&bytes[SESSION_HEADER..length - 32], budget)?;
                        if binding.identity().as_bytes() != &identity {
                            return Err(Error::Transcript);
                        }
                    }
                    if kind == Kind::CustodianReady {
                        let (session, _) = NativeApplicationProofSessionV1::decode(
                            &bytes[SESSION_HEADER..length - 32],
                            budget,
                        )?;
                        if session.transcript() != transcript {
                            return Err(Error::Transcript);
                        }
                    }
                    transcript
                };
                let value = Self::encode(kind, transcript, &bytes[SESSION_HEADER..length - 32]);
                if value.canonical_bytes() != bytes {
                    return Err(Error::Identity);
                }
                Ok((value, Storage(Self::RETAINED)))
            },
        )
    }
    /// The frame remains prepaid; the returned owned native payload is unreserved.
    pub fn decode_registration(&self, budget: &mut Budget<'_>) -> Result<(Binding, Storage)> {
        budget.with_prepaid_scope(Self::RETAINED, ENTRY, ENTRY, 0, |budget| {
            if self.kind != Kind::Challenge {
                return Err(Error::Kind);
            }
            Binding::decode(&self.bytes[SESSION_HEADER..self.kind.bytes() - 32], budget)
        })
    }
    pub fn decode_inputs(&self, budget: &mut Budget<'_>) -> Result<(Inputs, Storage)> {
        budget.with_prepaid_scope(Self::RETAINED, ENTRY, ENTRY, 0, |budget| {
            if self.kind != Kind::Hello {
                return Err(Error::Kind);
            }
            Inputs::decode(&self.bytes[SESSION_HEADER..self.kind.bytes() - 32], budget)
        })
    }
    pub fn decode_proof_session(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(NativeApplicationProofSessionV1, Storage)> {
        budget.with_prepaid_scope(Self::RETAINED, ENTRY, ENTRY, 0, |budget| {
            if self.kind != Kind::CustodianReady {
                return Err(Error::Kind);
            }
            NativeApplicationProofSessionV1::decode(
                &self.bytes[SESSION_HEADER..self.kind.bytes() - 32],
                budget,
            )
        })
    }
    pub const fn kind(&self) -> Kind {
        self.kind
    }
    pub const fn app_nonce(&self) -> [u8; 32] {
        self.transcript.app_nonce
    }
    pub fn transcript(&self) -> Option<Transcript> {
        (self.kind != Kind::Hello).then_some(self.transcript)
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes[..self.kind.bytes()]
    }
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }
    pub const fn rights(&self) -> usize {
        if matches!(self.kind, Kind::Challenge | Kind::CustodianReady) {
            1
        } else {
            0
        }
    }
    pub const fn authenticates_application_occurrence(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}
