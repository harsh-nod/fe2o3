//! Inert fixed record for `/etc/fe2o3/build-authority/policy-v1`.
//! This codec never reads that path or establishes its ownership or provenance.
use crate::CompilerClosureErrorV2;
use crate::CompilerClosureV2;
use sha2::{Digest, Sha256};
use std::{convert::Infallible, fmt, mem::size_of};

/// Closed compiler approval policy discriminator.
pub const COMPILER_APPROVAL_POLICY_MAGIC_V1: [u8; 8] = *b"F2CAP1\0\0";
/// Sole supported record schema.
pub const COMPILER_APPROVAL_POLICY_VERSION_V1: u16 = 1;
/// Fixed header preceding seven closure digests and two policy identities.
pub const COMPILER_APPROVAL_POLICY_HEADER_LEN_V1: usize = 32;
/// Exact complete record length, including its terminal SHA256 digest.
pub const COMPILER_APPROVAL_POLICY_BYTES_V1: usize = 352;
/// Domain preceding the little-endian u64 preimage length and exact preimage.
pub const COMPILER_APPROVAL_POLICY_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3-compiler-approval-policy-v1\0";
/// Requested complete immutable executable-closure enforcement, including the
/// loader, proc-macro execution and descendants, with no unapproved executable
/// memory. This is a closed rule-set version, not observed or supported runtime
/// enforcement. The codec neither implements that guard nor attests it ran.
pub const COMPILER_APPROVAL_POLICY_RUNTIME_ENFORCEMENT_VERSION_V1: u16 = 1;
/// Conservative fixed work prepayment for either construction or decoding,
/// covering all fixed copies, walks, comparisons and both possible SHA256 hashes
/// (the record and inherited compiler-closure validation). No work is refunded.
pub const COMPILER_APPROVAL_POLICY_WORK_V1: usize = 4096;
/// Complete logical result and fixed scratch allowance, excluding the caller's
/// input backing and callback captures. Reserve before construction or decoding;
/// this codec allocates no heap storage and does not own a resource ledger.
pub const COMPILER_APPROVAL_POLICY_STORAGE_V1: usize = size_of::<CompilerApprovalPolicyV1>()
    + 4 * COMPILER_APPROVAL_POLICY_BYTES_V1
    + 4 * size_of::<CompilerClosureV2>()
    + 2 * size_of::<Sha256>()
    + 512;
const HEADER: usize = COMPILER_APPROVAL_POLICY_HEADER_LEN_V1;
const LENGTH: usize = COMPILER_APPROVAL_POLICY_BYTES_V1;
const CLOSURE_END: usize = HEADER + 7 * 32;
const PROFILE_END: usize = CLOSURE_END + 32;
const PAYLOAD_END: usize = PROFILE_END + 32;

/// Strict framing, inherited compiler-closure validation, or caller work refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerApprovalPolicyErrorV1<E = Infallible> {
    /// Original caller work refusal, before any byte walk or hash.
    Charge(E),
    /// Actual or declared length is not the exact fixed record length.
    Length,
    /// Magic, schema or header extent is unsupported.
    Header,
    /// Reserved header bytes are nonzero.
    Reserved,
    /// Existing compiler-closure validation failed.
    CompilerClosure(CompilerClosureErrorV2),
    /// Exact V3 client-profile identity is all zero.
    ZeroClientProfileIdentity,
    /// Runtime manifest identity is all zero.
    ZeroRuntimeManifestIdentity,
    /// Requested enforcement rule-set version is not supported by this codec.
    UnsupportedRuntimeEnforcementVersion {
        /// Unrecognized requested rule-set version, not an observed runtime value.
        version: u16,
    },
    /// Terminal domain-separated digest differs from the record content.
    Identity,
}
type Error<E> = CompilerApprovalPolicyErrorV1<E>;
impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Charge(error) => {
                write!(formatter, "compiler approval policy work refused: {error}")
            }
            Self::Length => formatter.write_str("compiler approval policy length is not canonical"),
            Self::Header => formatter.write_str("unsupported compiler approval policy header"),
            Self::Reserved => {
                formatter.write_str("compiler approval policy reserved bytes are nonzero")
            }
            Self::CompilerClosure(error) => {
                write!(formatter, "compiler approval policy closure: {error}")
            }
            Self::ZeroClientProfileIdentity => {
                formatter.write_str("compiler approval policy client-profile identity is zero")
            }
            Self::ZeroRuntimeManifestIdentity => {
                formatter.write_str("compiler approval policy runtime manifest identity is zero")
            }
            Self::UnsupportedRuntimeEnforcementVersion { version } => write!(
                formatter,
                "unsupported compiler approval requested enforcement version {version}"
            ),
            Self::Identity => formatter.write_str("compiler approval policy identity mismatch"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Charge(error) => Some(error),
            Self::CompilerClosure(error) => Some(error),
            _ => None,
        }
    }
}

/// Canonical inert policy bytes, not a trust owner or approval capability.
///
/// The exact V3 client-profile identity already binds its generation; this record
/// adds no independent generation. Runtime enforcement version 1 requests a
/// complete immutable executable closure, including loader, proc-macro execution
/// and descendants, and forbids unapproved executable memory. It is not evidence
/// of runtime support, observation, or enforcement; this codec implements no guard.
///
/// Construction, decoding, equality and public identities establish only content.
/// Authenticating the root-controlled policy path and approving a compiler remain
/// separate operations. Publicly constructed or coherently resealed records do not
/// grant compiler, publication, loading, or execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerApprovalPolicyV1 {
    bytes: [u8; LENGTH],
    closure: CompilerClosureV2,
}
impl CompilerApprovalPolicyV1 {
    /// Encode an already validated compiler closure and inert policy requirements.
    /// Reserve `COMPILER_APPROVAL_POLICY_STORAGE_V1` first. Fixed work prepayment
    /// and callback destruction precede all payload walks, hashes and writes.
    pub fn new<E>(
        closure: CompilerClosureV2,
        client_profile_identity: [u8; 32],
        runtime_manifest_identity: [u8; 32],
        required_runtime_enforcement_version: u16,
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, Error<E>> {
        charge(COMPILER_APPROVAL_POLICY_WORK_V1).map_err(Error::Charge)?;
        drop(charge);
        validate_requirements(
            &client_profile_identity,
            &runtime_manifest_identity,
            required_runtime_enforcement_version,
        )?;
        let mut bytes = [0; LENGTH];
        bytes[..8].copy_from_slice(&COMPILER_APPROVAL_POLICY_MAGIC_V1);
        bytes[8..10].copy_from_slice(&COMPILER_APPROVAL_POLICY_VERSION_V1.to_le_bytes());
        bytes[10..12].copy_from_slice(&(HEADER as u16).to_le_bytes());
        bytes[12..16].copy_from_slice(&(LENGTH as u32).to_le_bytes());
        bytes[16..18].copy_from_slice(
            &closure
                .cargo_binding_transition_protocol_version()
                .to_le_bytes(),
        );
        bytes[18..20].copy_from_slice(&required_runtime_enforcement_version.to_le_bytes());
        let digests = [
            closure.cargo_executable_sha256(),
            closure.cargo_binding_trampoline_sha256(),
            closure.cargo_fe2o3_binding_wrapper_sha256(),
            closure.rustc_executable_sha256(),
            closure.rustc_runtime_tree_sha256(),
            closure.codegen_backend_sha256(),
            closure.identity_sha256(),
        ];
        for (slot, digest) in bytes[HEADER..CLOSURE_END].chunks_exact_mut(32).zip(digests) {
            slot.copy_from_slice(&digest);
        }
        bytes[CLOSURE_END..PROFILE_END].copy_from_slice(&client_profile_identity);
        bytes[PROFILE_END..PAYLOAD_END].copy_from_slice(&runtime_manifest_identity);
        let identity = hash(&bytes[..PAYLOAD_END]);
        bytes[PAYLOAD_END..].copy_from_slice(&identity);
        Ok(Self { bytes, closure })
    }

    /// Decode exactly one fixed V1 record, without allocation or legacy fallback.
    /// Length rejection visits no payload bytes. Every other check follows fixed
    /// work prepayment and callback destruction. Input bytes remain unchanged.
    /// Caller prepays input backing and `COMPILER_APPROVAL_POLICY_STORAGE_V1`.
    pub fn decode<E>(
        bytes: &[u8],
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, Error<E>> {
        if bytes.len() != LENGTH {
            return Err(Error::Length);
        }
        charge(COMPILER_APPROVAL_POLICY_WORK_V1).map_err(Error::Charge)?;
        drop(charge);
        if bytes[..8] != COMPILER_APPROVAL_POLICY_MAGIC_V1
            || bytes[8..10] != COMPILER_APPROVAL_POLICY_VERSION_V1.to_le_bytes()
            || bytes[10..12] != (HEADER as u16).to_le_bytes()
        {
            return Err(Error::Header);
        }
        if bytes[12..16] != (LENGTH as u32).to_le_bytes() {
            return Err(Error::Length);
        }
        if bytes[20..HEADER] != [0; 12] {
            return Err(Error::Reserved);
        }
        let version = u16::from_le_bytes(bytes[18..20].try_into().expect("fixed header"));
        validate_requirements(
            &bytes[CLOSURE_END..PROFILE_END],
            &bytes[PROFILE_END..PAYLOAD_END],
            version,
        )?;
        if bytes[PAYLOAD_END..] != hash(&bytes[..PAYLOAD_END]) {
            return Err(Error::Identity);
        }
        let digests: [[u8; 32]; 7] = bytes[HEADER..CLOSURE_END]
            .as_chunks::<32>()
            .0
            .try_into()
            .expect("fixed closure");
        let closure = CompilerClosureV2::from_pins_and_identity(
            digests[0],
            digests[1],
            digests[2],
            digests[3],
            digests[4],
            digests[5],
            u16::from_le_bytes(bytes[16..18].try_into().expect("fixed header")),
            digests[6],
        )
        .map_err(Error::CompilerClosure)?;
        Ok(Self {
            bytes: bytes.try_into().expect("checked fixed record"),
            closure,
        })
    }

    /// Complete canonical inert record, including its terminal digest.
    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_APPROVAL_POLICY_BYTES_V1] {
        &self.bytes
    }
    /// Validated six-pin compiler closure, not an approved executable owner.
    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.closure
    }
    /// Exact V3 client-profile identity; its generation is already identity-bound.
    pub fn client_profile_identity(&self) -> &[u8; 32] {
        self.bytes[CLOSURE_END..PROFILE_END]
            .try_into()
            .expect("fixed profile identity")
    }
    /// Claimed runtime manifest identity, not authenticated runtime custody.
    pub fn runtime_manifest_identity(&self) -> &[u8; 32] {
        self.bytes[PROFILE_END..PAYLOAD_END]
            .try_into()
            .expect("fixed manifest identity")
    }
    /// Requested closed rule set, not a claim that any runtime supports or enforces it.
    pub const fn required_runtime_enforcement_version(&self) -> u16 {
        COMPILER_APPROVAL_POLICY_RUNTIME_ENFORCEMENT_VERSION_V1
    }
    /// Domain-separated exact record identity, not a signature or approval.
    pub fn identity(&self) -> &[u8; 32] {
        self.bytes[PAYLOAD_END..]
            .try_into()
            .expect("fixed record identity")
    }
    /// Public policy bytes grant no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn validate_requirements<E>(profile: &[u8], runtime: &[u8], version: u16) -> Result<(), Error<E>> {
    if version != COMPILER_APPROVAL_POLICY_RUNTIME_ENFORCEMENT_VERSION_V1 {
        return Err(Error::UnsupportedRuntimeEnforcementVersion { version });
    }
    if profile == [0; 32] {
        return Err(Error::ZeroClientProfileIdentity);
    }
    if runtime == [0; 32] {
        return Err(Error::ZeroRuntimeManifestIdentity);
    }
    Ok(())
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(COMPILER_APPROVAL_POLICY_IDENTITY_DOMAIN_V1);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
