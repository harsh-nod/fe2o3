//! Inert helper-role policy for the exclusive production `policy-v2` path.
//! Decoding does not authenticate the policy, helper process or its custodian.
use crate::{
    CompilerApprovalPolicyErrorV1 as Framing, CompilerClosureV2,
    compiler_approval_policy_codec as codec,
};
use sha2::Sha256;
use std::{convert::Infallible, fmt, mem::size_of};

/// Closed V2 policy discriminator.
pub const COMPILER_APPROVAL_POLICY_MAGIC_V2: [u8; 8] = *b"F2CAP2\0\0";
/// Helper-role policy schema.
pub const COMPILER_APPROVAL_POLICY_VERSION_V2: u16 = 2;
/// Header with helper UID/GID at bytes 20..28 and zero reserved bytes 28..32.
pub const COMPILER_APPROVAL_POLICY_HEADER_LEN_V2: usize = 32;
/// Exact record length; closure and policy identity offsets remain unchanged.
pub const COMPILER_APPROVAL_POLICY_BYTES_V2: usize = codec::LENGTH;
/// Domain preceding the little-endian u64 preimage length and exact preimage.
pub const COMPILER_APPROVAL_POLICY_IDENTITY_DOMAIN_V2: &[u8] =
    b"fe2o3-compiler-approval-policy-v2\0";
/// Unchanged requested executable-closure rule set, not observed enforcement.
pub const COMPILER_APPROVAL_POLICY_RUNTIME_ENFORCEMENT_VERSION_V2: u16 =
    crate::COMPILER_APPROVAL_POLICY_RUNTIME_ENFORCEMENT_VERSION_V1;
/// Fixed prepayment covering credential checks, framing and both hashes.
pub const COMPILER_APPROVAL_POLICY_WORK_V2: usize = 4096;
/// Full logical result and scratch allowance, excluding caller-owned inputs.
pub const COMPILER_APPROVAL_POLICY_STORAGE_V2: usize = size_of::<CompilerApprovalPolicyV2>()
    + 4 * COMPILER_APPROVAL_POLICY_BYTES_V2
    + 4 * size_of::<CompilerClosureV2>()
    + 2 * size_of::<Sha256>()
    + 512;

/// Shared framing/work refusal or an invalid helper-role credential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerApprovalPolicyErrorV2<E = Infallible> {
    /// Existing strict compiler-policy framing, closure or work validation failed.
    Framing(Framing<E>),
    /// Helper UID is root or the unmapped/sentinel value.
    InvalidProofHelperUid,
    /// Helper GID is root or the unmapped/sentinel value.
    InvalidProofHelperGid,
}
type Error<E> = CompilerApprovalPolicyErrorV2<E>;
impl<E> From<Framing<E>> for Error<E> {
    fn from(error: Framing<E>) -> Self {
        Self::Framing(error)
    }
}
impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(error) => error.fmt(f),
            Self::InvalidProofHelperUid => {
                f.write_str("compiler policy proof helper UID is invalid")
            }
            Self::InvalidProofHelperGid => {
                f.write_str("compiler policy proof helper GID is invalid")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Framing(error) => Some(error),
            _ => None,
        }
    }
}

/// Canonical inert V2 policy, including separately configured helper credentials.
///
/// These values do not prove a process role or host-root/custodian provenance.
/// The production approval owner must authenticate the exact policy and V3
/// profile, then reject helper identities shared with either profile service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerApprovalPolicyV2 {
    bytes: [u8; codec::LENGTH],
    closure: CompilerClosureV2,
}
impl CompilerApprovalPolicyV2 {
    /// Constructs only inert bytes. Prepay STORAGE_V2 outside this call.
    /// Fixed work and callback destruction precede credential or payload inspection.
    pub fn new<E>(
        closure: CompilerClosureV2,
        client_profile_identity: [u8; 32],
        runtime_manifest_identity: [u8; 32],
        required_runtime_enforcement_version: u16,
        helper_uid: u32,
        helper_gid: u32,
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, Error<E>> {
        charge(COMPILER_APPROVAL_POLICY_WORK_V2).map_err(Framing::Charge)?;
        drop(charge);
        validate_credentials(helper_uid, helper_gid)?;
        let mut extension = [0; 12];
        extension[..4].copy_from_slice(&helper_uid.to_le_bytes());
        extension[4..8].copy_from_slice(&helper_gid.to_le_bytes());
        let record = codec::encode(
            closure,
            client_profile_identity,
            runtime_manifest_identity,
            required_runtime_enforcement_version,
            extension,
            SCHEMA,
        )?;
        Ok(Self {
            bytes: record.bytes,
            closure: record.closure,
        })
    }

    /// Decodes exactly V2, without fallback or V1 conversion. Length refusal
    /// precedes payload work; all other reads follow fixed work prepayment.
    pub fn decode<E>(
        bytes: &[u8],
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, Error<E>> {
        codec::require_length(bytes)?;
        charge(COMPILER_APPROVAL_POLICY_WORK_V2).map_err(Framing::Charge)?;
        drop(charge);
        codec::require_header(bytes, SCHEMA)?;
        if bytes[28..32] != [0; 4] {
            return Err(Framing::Reserved.into());
        }
        validate_credentials(u32_at(bytes, 20), u32_at(bytes, 24))?;
        let record = codec::decode_payload(bytes, SCHEMA)?;
        Ok(Self {
            bytes: record.bytes,
            closure: record.closure,
        })
    }

    /// Configured helper UID, not a credential observation or approval.
    pub fn proof_helper_uid(&self) -> u32 {
        u32_at(&self.bytes, 20)
    }
    /// Configured helper GID, not a credential observation or approval.
    pub fn proof_helper_gid(&self) -> u32 {
        u32_at(&self.bytes, 24)
    }

    codec::accessors!();
}
const SCHEMA: codec::Schema = codec::Schema {
    magic: COMPILER_APPROVAL_POLICY_MAGIC_V2,
    version: COMPILER_APPROVAL_POLICY_VERSION_V2,
    domain: COMPILER_APPROVAL_POLICY_IDENTITY_DOMAIN_V2,
};
fn validate_credentials<E>(uid: u32, gid: u32) -> Result<(), Error<E>> {
    if uid == 0 || uid == u32::MAX {
        return Err(Error::InvalidProofHelperUid);
    }
    if gid == 0 || gid == u32::MAX {
        return Err(Error::InvalidProofHelperGid);
    }
    Ok(())
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed helper credential"),
    )
}
