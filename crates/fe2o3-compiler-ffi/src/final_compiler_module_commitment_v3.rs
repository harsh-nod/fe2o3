use std::{error::Error, fmt, str};

use fe2o3_compiler_lineage::MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3;
use sha2::{Digest, Sha256};

use super::{
    CodeObjectVersion, CompilerModuleHandoffV2, CompilerModuleKindV1, DeviceTargetV1,
    MAX_COMPILER_FFI_ENVELOPE_BYTES_V1, MAX_COMPILER_MODULE_BYTES_V1,
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V2, MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1,
    MAX_DEVICE_FFI_TARGET_BYTES_V1,
};

/// Canonical wire domain for a compact final compiler-module commitment.
pub const FINAL_COMPILER_MODULE_COMMITMENT_DOMAIN_V3: &[u8] =
    b"FE2O3/FINAL-COMPILER-MODULE-COMMITMENT/V3\0";
/// Exact wire version accepted by the V3 decoder.
pub const FINAL_COMPILER_MODULE_COMMITMENT_VERSION_V3: u16 = 3;
/// Exact-content policy accepted by the V3 decoder.
pub const FINAL_COMPILER_MODULE_COMMITMENT_POLICY_V3: u16 = 1;

const COMMITMENT_IDENTITY_DOMAIN_V3: &[u8] =
    b"FE2O3/FINAL-COMPILER-MODULE-COMMITMENT-IDENTITY/V3\0";
const FLAGS_V3: u16 = 0;
const RESERVED_V3: u16 = 0;
const SHA256_BYTES: usize = 32;
const CONTENT_IDENTITY_BYTES: usize = SHA256_BYTES + 8;
const ENVELOPE_COMMITMENT_BYTES: usize = SHA256_BYTES + SHA256_BYTES + 8;
const TERMINAL_IDENTITY_BYTES: usize = SHA256_BYTES;
const HEADER_BYTES_V3: usize = FINAL_COMPILER_MODULE_COMMITMENT_DOMAIN_V3.len()
    + 2 // version
    + 2 // policy
    + 2 // flags
    + 2 // reserved
    + 4 // complete canonical length
    + 4 // target length
    + 1 // module kind
    + 1 // code-object version
    + 2; // field reserved
const FIXED_BYTES_V3: usize = HEADER_BYTES_V3
    + CONTENT_IDENTITY_BYTES // module
    + ENVELOPE_COMMITMENT_BYTES
    + CONTENT_IDENTITY_BYTES // symbol manifest
    + CONTENT_IDENTITY_BYTES // V2 handoff
    + TERMINAL_IDENTITY_BYTES;

/// Maximum canonical V3 commitment size. Raw LLVM and nested canonical payloads are never stored.
pub const MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3: usize =
    FIXED_BYTES_V3 + MAX_DEVICE_FFI_TARGET_BYTES_V1;

const _: () =
    assert!(MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3 <= MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3);

/// The structural policy represented by a V3 final compiler-module commitment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum FinalCompilerModuleCommitmentPolicyV3 {
    /// Bind exact V2 handoff content identities without granting authority or proving derivation.
    ExactCompilerModuleHandoffV2ContentOnly = FINAL_COMPILER_MODULE_COMMITMENT_POLICY_V3,
}

/// SHA-256 and byte length of one exact content component referenced by the commitment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalCompilerModuleContentIdentityV3 {
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
}

impl FinalCompilerModuleContentIdentityV3 {
    pub const fn sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn matches(self, bytes: &[u8]) -> bool {
        self.byte_len == bytes.len() as u64
            && self.sha256 == <[u8; SHA256_BYTES]>::from(Sha256::digest(bytes))
    }

    fn validate(self, field: &'static str) -> Result<Self, FinalCompilerModuleCommitmentErrorV3> {
        validate_identity(field, self.sha256, self.byte_len)?;
        Ok(self)
    }
}

/// Domain-separated identity of one complete canonical V3 commitment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalCompilerModuleCommitmentIdentityV3 {
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
}

impl FinalCompilerModuleCommitmentIdentityV3 {
    pub const fn sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        let Some(preimage_len) = bytes.len().checked_sub(TERMINAL_IDENTITY_BYTES) else {
            return false;
        };
        let Ok(byte_len) = u64::try_from(bytes.len()) else {
            return false;
        };
        let expected = calculate_commitment_identity(&bytes[..preimage_len], byte_len);
        self == expected && bytes[preimage_len..] == self.sha256
    }
}

/// Compact, strict canonical content commitment derived from one exact V2 compiler handoff.
///
/// This value records byte identities only. Public construction and decoding do not authenticate
/// a producer, prove semantic refinement, establish freshness, or grant compiler, publication,
/// link, load, or launch authority. A private producer-owned join must compare it with the exact
/// live handoff and consume the relevant move-only authorities.
#[derive(Clone, Eq, PartialEq)]
pub struct InertFinalCompilerModuleCommitmentV3 {
    kind: CompilerModuleKindV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    module: FinalCompilerModuleContentIdentityV3,
    envelope_identity: [u8; SHA256_BYTES],
    envelope_canonical_identity: [u8; SHA256_BYTES],
    envelope_byte_len: u64,
    symbol_manifest: FinalCompilerModuleContentIdentityV3,
    handoff: FinalCompilerModuleContentIdentityV3,
    identity: FinalCompilerModuleCommitmentIdentityV3,
    canonical_bytes: Box<[u8]>,
}

impl fmt::Debug for InertFinalCompilerModuleCommitmentV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertFinalCompilerModuleCommitmentV3")
            .field("policy", &self.policy())
            .field("kind", &self.kind)
            .field("target", &self.target)
            .field("code_object_version", &self.code_object_version)
            .field("module", &self.module)
            .field("envelope_byte_len", &self.envelope_byte_len)
            .field("symbol_manifest", &self.symbol_manifest)
            .field("handoff", &self.handoff)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy)]
struct CommitmentPartsV3 {
    kind: CompilerModuleKindV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    module: FinalCompilerModuleContentIdentityV3,
    envelope_identity: [u8; SHA256_BYTES],
    envelope_canonical_identity: [u8; SHA256_BYTES],
    envelope_byte_len: u64,
    symbol_manifest: FinalCompilerModuleContentIdentityV3,
    handoff: FinalCompilerModuleContentIdentityV3,
}

impl InertFinalCompilerModuleCommitmentV3 {
    /// Constructs a compact commitment from all exact canonical components of `handoff`.
    pub fn from_handoff(
        handoff: &CompilerModuleHandoffV2,
    ) -> Result<Self, FinalCompilerModuleCommitmentErrorV3> {
        let module = FinalCompilerModuleContentIdentityV3 {
            sha256: *handoff.module_identity().sha256(),
            byte_len: handoff.module_identity().byte_len(),
        }
        .validate("module")?;
        if !module.matches(handoff.module_bytes()) {
            return Err(
                FinalCompilerModuleCommitmentErrorV3::SourceHandoffMismatch { field: "module" },
            );
        }

        let envelope_bytes = handoff.envelope().canonical_bytes();
        let envelope_byte_len = u64::try_from(envelope_bytes.len())
            .map_err(|_| FinalCompilerModuleCommitmentErrorV3::LengthOverflow)?;
        let envelope_identity = handoff.envelope().identity().as_bytes();
        let envelope_canonical_identity = Sha256::digest(envelope_bytes).into();
        validate_identity("envelope", envelope_identity, envelope_byte_len)?;
        validate_identity(
            "envelope canonical bytes",
            envelope_canonical_identity,
            envelope_byte_len,
        )?;
        if envelope_identity != envelope_canonical_identity {
            return Err(
                FinalCompilerModuleCommitmentErrorV3::SourceHandoffMismatch {
                    field: "envelope identity",
                },
            );
        }

        let manifest_identity = handoff.symbol_manifest().identity();
        let symbol_manifest = FinalCompilerModuleContentIdentityV3 {
            sha256: *manifest_identity.sha256(),
            byte_len: manifest_identity.byte_len(),
        }
        .validate("symbol manifest")?;
        if !symbol_manifest.matches(handoff.symbol_manifest().canonical_bytes()) {
            return Err(
                FinalCompilerModuleCommitmentErrorV3::SourceHandoffMismatch {
                    field: "symbol manifest",
                },
            );
        }

        let handoff_identity = handoff.identity();
        let handoff_content = FinalCompilerModuleContentIdentityV3 {
            sha256: *handoff_identity.sha256(),
            byte_len: handoff_identity.byte_len(),
        }
        .validate("V2 handoff")?;
        if !handoff_content.matches(handoff.canonical_bytes()) {
            return Err(
                FinalCompilerModuleCommitmentErrorV3::SourceHandoffMismatch {
                    field: "V2 handoff",
                },
            );
        }

        Self::build(CommitmentPartsV3 {
            kind: handoff.kind(),
            target: handoff.target(),
            code_object_version: handoff.code_object_version(),
            module,
            envelope_identity,
            envelope_canonical_identity,
            envelope_byte_len,
            symbol_manifest,
            handoff: handoff_content,
        })
    }

    /// Strictly decodes one complete canonical V3 commitment with no earlier-version fallback.
    pub fn decode(bytes: &[u8]) -> Result<Self, FinalCompilerModuleCommitmentErrorV3> {
        let parsed = ParsedCommitmentV3::parse(bytes)?;
        let decoded = Self::build(parsed.parts)?;
        if decoded.identity != parsed.identity {
            return Err(FinalCompilerModuleCommitmentErrorV3::CommitmentIdentityMismatch);
        }
        if decoded.canonical_bytes() != bytes {
            return Err(FinalCompilerModuleCommitmentErrorV3::NonCanonicalEncoding);
        }
        Ok(decoded)
    }

    fn build(parts: CommitmentPartsV3) -> Result<Self, FinalCompilerModuleCommitmentErrorV3> {
        validate_parts(&parts)?;
        let target_text = parts.target.to_string();
        let exact_size = exact_size(target_text.len())?;
        let exact_size_u32 = u32::try_from(exact_size)
            .map_err(|_| FinalCompilerModuleCommitmentErrorV3::LengthOverflow)?;
        let target_len_u32 = u32::try_from(target_text.len())
            .map_err(|_| FinalCompilerModuleCommitmentErrorV3::LengthOverflow)?;

        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(exact_size)
            .map_err(|_| FinalCompilerModuleCommitmentErrorV3::AllocationFailed)?;
        canonical.extend_from_slice(FINAL_COMPILER_MODULE_COMMITMENT_DOMAIN_V3);
        canonical.extend_from_slice(&FINAL_COMPILER_MODULE_COMMITMENT_VERSION_V3.to_le_bytes());
        canonical.extend_from_slice(&FINAL_COMPILER_MODULE_COMMITMENT_POLICY_V3.to_le_bytes());
        canonical.extend_from_slice(&FLAGS_V3.to_le_bytes());
        canonical.extend_from_slice(&RESERVED_V3.to_le_bytes());
        canonical.extend_from_slice(&exact_size_u32.to_le_bytes());
        canonical.extend_from_slice(&target_len_u32.to_le_bytes());
        canonical.push(module_kind_tag(parts.kind));
        canonical.push(code_object_version_tag(parts.code_object_version));
        canonical.extend_from_slice(&RESERVED_V3.to_le_bytes());
        canonical.extend_from_slice(target_text.as_bytes());
        push_content_identity(&mut canonical, parts.module);
        canonical.extend_from_slice(&parts.envelope_identity);
        canonical.extend_from_slice(&parts.envelope_canonical_identity);
        canonical.extend_from_slice(&parts.envelope_byte_len.to_le_bytes());
        push_content_identity(&mut canonical, parts.symbol_manifest);
        push_content_identity(&mut canonical, parts.handoff);

        let byte_len = u64::try_from(exact_size)
            .map_err(|_| FinalCompilerModuleCommitmentErrorV3::LengthOverflow)?;
        let identity = calculate_commitment_identity(&canonical, byte_len);
        validate_identity("commitment", identity.sha256, identity.byte_len)?;
        canonical.extend_from_slice(&identity.sha256);
        debug_assert_eq!(canonical.len(), exact_size);

        Ok(Self {
            kind: parts.kind,
            target: parts.target,
            code_object_version: parts.code_object_version,
            module: parts.module,
            envelope_identity: parts.envelope_identity,
            envelope_canonical_identity: parts.envelope_canonical_identity,
            envelope_byte_len: parts.envelope_byte_len,
            symbol_manifest: parts.symbol_manifest,
            handoff: parts.handoff,
            identity,
            canonical_bytes: canonical.into_boxed_slice(),
        })
    }

    pub const fn policy(&self) -> FinalCompilerModuleCommitmentPolicyV3 {
        FinalCompilerModuleCommitmentPolicyV3::ExactCompilerModuleHandoffV2ContentOnly
    }

    pub const fn kind(&self) -> CompilerModuleKindV1 {
        self.kind
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub const fn module_identity(&self) -> FinalCompilerModuleContentIdentityV3 {
        self.module
    }

    pub const fn envelope_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.envelope_identity
    }

    pub const fn envelope_canonical_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.envelope_canonical_identity
    }

    pub const fn envelope_byte_len(&self) -> u64 {
        self.envelope_byte_len
    }

    pub const fn symbol_manifest_identity(&self) -> FinalCompilerModuleContentIdentityV3 {
        self.symbol_manifest
    }

    pub const fn handoff_identity(&self) -> FinalCompilerModuleContentIdentityV3 {
        self.handoff
    }

    pub const fn identity(&self) -> FinalCompilerModuleCommitmentIdentityV3 {
        self.identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Compares every committed axis with one exact V2 handoff.
    ///
    /// This is a structural equality check, not authentication or authority admission.
    pub fn matches_handoff(&self, handoff: &CompilerModuleHandoffV2) -> bool {
        let module = handoff.module_identity();
        let envelope_bytes = handoff.envelope().canonical_bytes();
        let envelope_canonical_identity: [u8; SHA256_BYTES] = Sha256::digest(envelope_bytes).into();
        let manifest = handoff.symbol_manifest().identity();
        let handoff_identity = handoff.identity();

        self.kind == handoff.kind()
            && self.target == handoff.target()
            && self.code_object_version == handoff.code_object_version()
            && self.module.sha256 == *module.sha256()
            && self.module.byte_len == module.byte_len()
            && self.envelope_identity == handoff.envelope().identity().as_bytes()
            && self.envelope_canonical_identity == envelope_canonical_identity
            && self.envelope_byte_len == envelope_bytes.len() as u64
            && self.symbol_manifest.sha256 == *manifest.sha256()
            && self.symbol_manifest.byte_len == manifest.byte_len()
            && self.handoff.sha256 == *handoff_identity.sha256()
            && self.handoff.byte_len == handoff_identity.byte_len()
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    pub const fn establishes_semantic_refinement(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl<'a> TryFrom<&'a [u8]> for InertFinalCompilerModuleCommitmentV3 {
    type Error = FinalCompilerModuleCommitmentErrorV3;

    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        Self::decode(bytes)
    }
}

struct ParsedCommitmentV3 {
    parts: CommitmentPartsV3,
    identity: FinalCompilerModuleCommitmentIdentityV3,
}

impl ParsedCommitmentV3 {
    fn parse(bytes: &[u8]) -> Result<Self, FinalCompilerModuleCommitmentErrorV3> {
        if bytes.len() > MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3 {
            return Err(
                FinalCompilerModuleCommitmentErrorV3::CommitmentByteBoundExceeded {
                    actual: bytes.len(),
                    max: MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3,
                },
            );
        }
        if bytes.len() < FIXED_BYTES_V3 {
            return Err(FinalCompilerModuleCommitmentErrorV3::Truncated);
        }

        let mut cursor = CommitmentCursorV3::new(bytes);
        if cursor.take(FINAL_COMPILER_MODULE_COMMITMENT_DOMAIN_V3.len())?
            != FINAL_COMPILER_MODULE_COMMITMENT_DOMAIN_V3
        {
            return Err(FinalCompilerModuleCommitmentErrorV3::InvalidDomain);
        }
        let version = cursor.u16()?;
        if version != FINAL_COMPILER_MODULE_COMMITMENT_VERSION_V3 {
            return Err(FinalCompilerModuleCommitmentErrorV3::UnsupportedVersion {
                observed: version,
            });
        }
        let policy = cursor.u16()?;
        if policy != FINAL_COMPILER_MODULE_COMMITMENT_POLICY_V3 {
            return Err(FinalCompilerModuleCommitmentErrorV3::UnsupportedPolicy {
                observed: policy,
            });
        }
        if cursor.u16()? != FLAGS_V3 {
            return Err(FinalCompilerModuleCommitmentErrorV3::NonZeroFlags);
        }
        if cursor.u16()? != RESERVED_V3 {
            return Err(FinalCompilerModuleCommitmentErrorV3::NonZeroReserved);
        }

        let declared_len = cursor.u32_as_usize()?;
        match declared_len.cmp(&bytes.len()) {
            std::cmp::Ordering::Less => {
                return Err(FinalCompilerModuleCommitmentErrorV3::TrailingBytes {
                    trailing: bytes.len() - declared_len,
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(FinalCompilerModuleCommitmentErrorV3::Truncated);
            }
            std::cmp::Ordering::Equal => {}
        }

        let target_len = cursor.u32_as_usize()?;
        if target_len == 0 || target_len > MAX_DEVICE_FFI_TARGET_BYTES_V1 {
            return Err(
                FinalCompilerModuleCommitmentErrorV3::InvalidTargetByteLength {
                    observed: target_len,
                    max: MAX_DEVICE_FFI_TARGET_BYTES_V1,
                },
            );
        }
        let expected_len = exact_size(target_len)?;
        match expected_len.cmp(&bytes.len()) {
            std::cmp::Ordering::Less => {
                return Err(FinalCompilerModuleCommitmentErrorV3::TrailingBytes {
                    trailing: bytes.len() - expected_len,
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(FinalCompilerModuleCommitmentErrorV3::Truncated);
            }
            std::cmp::Ordering::Equal => {}
        }

        let kind = decode_module_kind(cursor.byte()?)?;
        let code_object_version = decode_code_object_version(cursor.byte()?)?;
        if cursor.u16()? != RESERVED_V3 {
            return Err(FinalCompilerModuleCommitmentErrorV3::NonZeroReserved);
        }
        let target_bytes = cursor.take(target_len)?;
        let target_text = str::from_utf8(target_bytes)
            .map_err(|_| FinalCompilerModuleCommitmentErrorV3::InvalidTarget)?;
        let target = DeviceTargetV1::parse(target_text)
            .map_err(|_| FinalCompilerModuleCommitmentErrorV3::InvalidTarget)?;

        let module = cursor.content_identity("module")?;
        let envelope_identity = cursor.fixed::<SHA256_BYTES>()?;
        let envelope_canonical_identity = cursor.fixed::<SHA256_BYTES>()?;
        let envelope_byte_len = cursor.u64()?;
        validate_identity("envelope", envelope_identity, envelope_byte_len)?;
        validate_identity(
            "envelope canonical bytes",
            envelope_canonical_identity,
            envelope_byte_len,
        )?;
        if envelope_identity != envelope_canonical_identity {
            return Err(FinalCompilerModuleCommitmentErrorV3::EnvelopeIdentityMismatch);
        }
        let symbol_manifest = cursor.content_identity("symbol manifest")?;
        let handoff = cursor.content_identity("V2 handoff")?;
        let terminal_sha256 = cursor.fixed::<SHA256_BYTES>()?;
        cursor.finish()?;
        validate_identity(
            "commitment",
            terminal_sha256,
            u64::try_from(bytes.len())
                .map_err(|_| FinalCompilerModuleCommitmentErrorV3::LengthOverflow)?,
        )?;

        let preimage_len = bytes
            .len()
            .checked_sub(TERMINAL_IDENTITY_BYTES)
            .ok_or(FinalCompilerModuleCommitmentErrorV3::Truncated)?;
        let identity = FinalCompilerModuleCommitmentIdentityV3 {
            sha256: terminal_sha256,
            byte_len: bytes.len() as u64,
        };
        let expected_identity =
            calculate_commitment_identity(&bytes[..preimage_len], bytes.len() as u64);
        if identity != expected_identity {
            return Err(FinalCompilerModuleCommitmentErrorV3::CommitmentIdentityMismatch);
        }

        let parts = CommitmentPartsV3 {
            kind,
            target,
            code_object_version,
            module,
            envelope_identity,
            envelope_canonical_identity,
            envelope_byte_len,
            symbol_manifest,
            handoff,
        };
        validate_parts(&parts)?;
        Ok(Self { parts, identity })
    }
}

struct CommitmentCursorV3<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> CommitmentCursorV3<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], FinalCompilerModuleCommitmentErrorV3> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(FinalCompilerModuleCommitmentErrorV3::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(FinalCompilerModuleCommitmentErrorV3::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], FinalCompilerModuleCommitmentErrorV3> {
        self.take(N)?
            .try_into()
            .map_err(|_| FinalCompilerModuleCommitmentErrorV3::Truncated)
    }

    fn byte(&mut self) -> Result<u8, FinalCompilerModuleCommitmentErrorV3> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, FinalCompilerModuleCommitmentErrorV3> {
        Ok(u16::from_le_bytes(self.fixed::<2>()?))
    }

    fn u32_as_usize(&mut self) -> Result<usize, FinalCompilerModuleCommitmentErrorV3> {
        usize::try_from(u32::from_le_bytes(self.fixed::<4>()?))
            .map_err(|_| FinalCompilerModuleCommitmentErrorV3::LengthOverflow)
    }

    fn u64(&mut self) -> Result<u64, FinalCompilerModuleCommitmentErrorV3> {
        Ok(u64::from_le_bytes(self.fixed::<8>()?))
    }

    fn content_identity(
        &mut self,
        field: &'static str,
    ) -> Result<FinalCompilerModuleContentIdentityV3, FinalCompilerModuleCommitmentErrorV3> {
        FinalCompilerModuleContentIdentityV3 {
            sha256: self.fixed::<SHA256_BYTES>()?,
            byte_len: self.u64()?,
        }
        .validate(field)
    }

    fn finish(self) -> Result<(), FinalCompilerModuleCommitmentErrorV3> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(FinalCompilerModuleCommitmentErrorV3::TrailingBytes {
                trailing: self.bytes.len() - self.offset,
            })
        }
    }
}

fn validate_parts(parts: &CommitmentPartsV3) -> Result<(), FinalCompilerModuleCommitmentErrorV3> {
    parts.module.validate("module")?;
    validate_content_bound(
        "module",
        parts.module.byte_len,
        MAX_COMPILER_MODULE_BYTES_V1,
    )?;
    validate_identity("envelope", parts.envelope_identity, parts.envelope_byte_len)?;
    validate_identity(
        "envelope canonical bytes",
        parts.envelope_canonical_identity,
        parts.envelope_byte_len,
    )?;
    validate_content_bound(
        "envelope",
        parts.envelope_byte_len,
        MAX_COMPILER_FFI_ENVELOPE_BYTES_V1,
    )?;
    if parts.envelope_identity != parts.envelope_canonical_identity {
        return Err(FinalCompilerModuleCommitmentErrorV3::EnvelopeIdentityMismatch);
    }
    parts.symbol_manifest.validate("symbol manifest")?;
    validate_content_bound(
        "symbol manifest",
        parts.symbol_manifest.byte_len,
        MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1,
    )?;
    parts.handoff.validate("V2 handoff")?;
    validate_content_bound(
        "V2 handoff",
        parts.handoff.byte_len,
        MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
    )?;
    let target_text = parts.target.to_string();
    if target_text.is_empty() || target_text.len() > MAX_DEVICE_FFI_TARGET_BYTES_V1 {
        return Err(
            FinalCompilerModuleCommitmentErrorV3::InvalidTargetByteLength {
                observed: target_text.len(),
                max: MAX_DEVICE_FFI_TARGET_BYTES_V1,
            },
        );
    }
    Ok(())
}

fn exact_size(target_len: usize) -> Result<usize, FinalCompilerModuleCommitmentErrorV3> {
    let exact = FIXED_BYTES_V3
        .checked_add(target_len)
        .ok_or(FinalCompilerModuleCommitmentErrorV3::LengthOverflow)?;
    if exact > MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3 {
        return Err(
            FinalCompilerModuleCommitmentErrorV3::CommitmentByteBoundExceeded {
                actual: exact,
                max: MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3,
            },
        );
    }
    Ok(exact)
}

fn validate_identity(
    field: &'static str,
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
) -> Result<(), FinalCompilerModuleCommitmentErrorV3> {
    if sha256 == [0; SHA256_BYTES] {
        return Err(FinalCompilerModuleCommitmentErrorV3::ZeroIdentity { field });
    }
    if byte_len == 0 {
        return Err(FinalCompilerModuleCommitmentErrorV3::ZeroByteLength { field });
    }
    Ok(())
}

fn validate_content_bound(
    field: &'static str,
    observed: u64,
    max: usize,
) -> Result<(), FinalCompilerModuleCommitmentErrorV3> {
    if observed > max as u64 {
        return Err(
            FinalCompilerModuleCommitmentErrorV3::ContentByteBoundExceeded {
                field,
                observed,
                max: max as u64,
            },
        );
    }
    Ok(())
}

fn push_content_identity(bytes: &mut Vec<u8>, identity: FinalCompilerModuleContentIdentityV3) {
    bytes.extend_from_slice(&identity.sha256);
    bytes.extend_from_slice(&identity.byte_len.to_le_bytes());
}

fn calculate_commitment_identity(
    preimage: &[u8],
    canonical_byte_len: u64,
) -> FinalCompilerModuleCommitmentIdentityV3 {
    let mut hasher = Sha256::new();
    hasher.update(COMMITMENT_IDENTITY_DOMAIN_V3);
    hasher.update((preimage.len() as u64).to_le_bytes());
    hasher.update(preimage);
    FinalCompilerModuleCommitmentIdentityV3 {
        sha256: hasher.finalize().into(),
        byte_len: canonical_byte_len,
    }
}

const fn module_kind_tag(kind: CompilerModuleKindV1) -> u8 {
    match kind {
        CompilerModuleKindV1::LlvmTextIr => 1,
        CompilerModuleKindV1::LlvmBitcode => 2,
    }
}

fn decode_module_kind(
    observed: u8,
) -> Result<CompilerModuleKindV1, FinalCompilerModuleCommitmentErrorV3> {
    match observed {
        1 => Ok(CompilerModuleKindV1::LlvmTextIr),
        2 => Ok(CompilerModuleKindV1::LlvmBitcode),
        _ => Err(FinalCompilerModuleCommitmentErrorV3::InvalidModuleKind { observed }),
    }
}

const fn code_object_version_tag(version: CodeObjectVersion) -> u8 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}

fn decode_code_object_version(
    observed: u8,
) -> Result<CodeObjectVersion, FinalCompilerModuleCommitmentErrorV3> {
    match observed {
        4 => Ok(CodeObjectVersion::V4),
        5 => Ok(CodeObjectVersion::V5),
        6 => Ok(CodeObjectVersion::V6),
        _ => Err(FinalCompilerModuleCommitmentErrorV3::InvalidCodeObjectVersion { observed }),
    }
}

/// Failure to construct or strictly decode a compact V3 final compiler-module commitment.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FinalCompilerModuleCommitmentErrorV3 {
    CommitmentByteBoundExceeded {
        actual: usize,
        max: usize,
    },
    LengthOverflow,
    AllocationFailed,
    Truncated,
    TrailingBytes {
        trailing: usize,
    },
    InvalidDomain,
    UnsupportedVersion {
        observed: u16,
    },
    UnsupportedPolicy {
        observed: u16,
    },
    NonZeroFlags,
    NonZeroReserved,
    InvalidTargetByteLength {
        observed: usize,
        max: usize,
    },
    InvalidTarget,
    InvalidModuleKind {
        observed: u8,
    },
    InvalidCodeObjectVersion {
        observed: u8,
    },
    ZeroIdentity {
        field: &'static str,
    },
    ZeroByteLength {
        field: &'static str,
    },
    ContentByteBoundExceeded {
        field: &'static str,
        observed: u64,
        max: u64,
    },
    EnvelopeIdentityMismatch,
    CommitmentIdentityMismatch,
    SourceHandoffMismatch {
        field: &'static str,
    },
    NonCanonicalEncoding,
}

impl fmt::Display for FinalCompilerModuleCommitmentErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommitmentByteBoundExceeded { actual, max } => write!(
                formatter,
                "final compiler-module commitment has {actual} bytes; maximum is {max}"
            ),
            Self::LengthOverflow => {
                formatter.write_str("final compiler-module commitment length overflow")
            }
            Self::AllocationFailed => {
                formatter.write_str("could not allocate final compiler-module commitment")
            }
            Self::Truncated => formatter.write_str("truncated final compiler-module commitment"),
            Self::TrailingBytes { trailing } => write!(
                formatter,
                "final compiler-module commitment has {trailing} trailing bytes"
            ),
            Self::InvalidDomain => {
                formatter.write_str("invalid final compiler-module commitment domain")
            }
            Self::UnsupportedVersion { observed } => write!(
                formatter,
                "unsupported final compiler-module commitment version {observed}"
            ),
            Self::UnsupportedPolicy { observed } => write!(
                formatter,
                "unsupported final compiler-module commitment policy {observed}"
            ),
            Self::NonZeroFlags => {
                formatter.write_str("final compiler-module commitment flags must be zero")
            }
            Self::NonZeroReserved => {
                formatter.write_str("final compiler-module commitment reserved bits must be zero")
            }
            Self::InvalidTargetByteLength { observed, max } => write!(
                formatter,
                "final compiler-module target has {observed} bytes; expected 1..={max}"
            ),
            Self::InvalidTarget => {
                formatter.write_str("invalid canonical final compiler-module target")
            }
            Self::InvalidModuleKind { observed } => write!(
                formatter,
                "invalid final compiler-module kind tag {observed}"
            ),
            Self::InvalidCodeObjectVersion { observed } => write!(
                formatter,
                "invalid final compiler-module code-object version {observed}"
            ),
            Self::ZeroIdentity { field } => write!(formatter, "{field} identity is all zero"),
            Self::ZeroByteLength { field } => write!(formatter, "{field} byte length is zero"),
            Self::ContentByteBoundExceeded {
                field,
                observed,
                max,
            } => write!(
                formatter,
                "{field} declares {observed} bytes; maximum is {max}"
            ),
            Self::EnvelopeIdentityMismatch => formatter.write_str(
                "declared envelope identity disagrees with its canonical-bytes identity",
            ),
            Self::CommitmentIdentityMismatch => {
                formatter.write_str("final compiler-module commitment terminal identity mismatch")
            }
            Self::SourceHandoffMismatch { field } => write!(
                formatter,
                "source V2 compiler-module handoff has inconsistent {field} content"
            ),
            Self::NonCanonicalEncoding => {
                formatter.write_str("noncanonical final compiler-module commitment encoding")
            }
        }
    }
}

impl Error for FinalCompilerModuleCommitmentErrorV3 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CompilerFfiEnvelopeV1, CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
    };

    const TARGET: &str = "gfx942:xnack-";
    const OTHER_TARGET: &str = "gfx942:xnack+";
    const FEATURED_TARGET: &str = "gfx942:sramecc+:xnack-";
    const NONCANONICAL_FEATURED_TARGET: &[u8] = b"gfx942:xnack-:sramecc+";
    const LLVM_IR: &[u8] =
        b"; ModuleID = 'final-commitment-v3'\ndefine amdgpu_kernel void @kernel() { ret void }\n";

    fn target(text: &str) -> DeviceTargetV1 {
        DeviceTargetV1::parse(text).expect("canonical test target")
    }

    fn manifest(helper: &str) -> CompilerModuleSymbolManifestV1 {
        use CompilerModuleSymbolRoleV1 as Role;
        CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, "kernel"),
            (Role::KernelDescriptor, "kernel.kd"),
            (Role::InternalHelper, helper),
        ])
        .expect("canonical test manifest")
    }

    fn handoff(
        kind: CompilerModuleKindV1,
        target_text: &str,
        code_object_version: CodeObjectVersion,
        module: &[u8],
        helper: &str,
    ) -> CompilerModuleHandoffV2 {
        let target = target(target_text);
        CompilerModuleHandoffV2::new(
            kind,
            target,
            code_object_version,
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, code_object_version)
                .expect("valid FFI-free envelope"),
            manifest(helper),
            module,
        )
        .expect("valid test handoff")
    }

    fn baseline_handoff() -> CompilerModuleHandoffV2 {
        handoff(
            CompilerModuleKindV1::LlvmTextIr,
            TARGET,
            CodeObjectVersion::V5,
            LLVM_IR,
            "helper_a",
        )
    }

    #[derive(Clone, Copy)]
    struct WireOffsets {
        version: usize,
        policy: usize,
        flags: usize,
        header_reserved: usize,
        total_len: usize,
        target_len: usize,
        kind: usize,
        code_object_version: usize,
        field_reserved: usize,
        target: usize,
        module_sha256: usize,
        module_len: usize,
        envelope_identity: usize,
        envelope_canonical_identity: usize,
        envelope_len: usize,
        manifest_sha256: usize,
        manifest_len: usize,
        handoff_sha256: usize,
        handoff_len: usize,
        terminal_identity: usize,
    }

    fn read_u32(bytes: &[u8], offset: usize) -> usize {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
    }

    fn offsets(bytes: &[u8]) -> WireOffsets {
        let version = FINAL_COMPILER_MODULE_COMMITMENT_DOMAIN_V3.len();
        let policy = version + 2;
        let flags = policy + 2;
        let header_reserved = flags + 2;
        let total_len = header_reserved + 2;
        let target_len = total_len + 4;
        let kind = target_len + 4;
        let code_object_version = kind + 1;
        let field_reserved = code_object_version + 1;
        let target = field_reserved + 2;
        let module_sha256 = target + read_u32(bytes, target_len);
        let module_len = module_sha256 + SHA256_BYTES;
        let envelope_identity = module_len + 8;
        let envelope_canonical_identity = envelope_identity + SHA256_BYTES;
        let envelope_len = envelope_canonical_identity + SHA256_BYTES;
        let manifest_sha256 = envelope_len + 8;
        let manifest_len = manifest_sha256 + SHA256_BYTES;
        let handoff_sha256 = manifest_len + 8;
        let handoff_len = handoff_sha256 + SHA256_BYTES;
        let terminal_identity = handoff_len + 8;
        assert_eq!(terminal_identity + TERMINAL_IDENTITY_BYTES, bytes.len());
        WireOffsets {
            version,
            policy,
            flags,
            header_reserved,
            total_len,
            target_len,
            kind,
            code_object_version,
            field_reserved,
            target,
            module_sha256,
            module_len,
            envelope_identity,
            envelope_canonical_identity,
            envelope_len,
            manifest_sha256,
            manifest_len,
            handoff_sha256,
            handoff_len,
            terminal_identity,
        }
    }

    fn rehash(bytes: &mut [u8]) {
        let terminal = bytes.len() - TERMINAL_IDENTITY_BYTES;
        let identity = calculate_commitment_identity(&bytes[..terminal], bytes.len() as u64);
        bytes[terminal..].copy_from_slice(identity.sha256());
    }

    fn assert_rehashed_substitution_is_inert(
        original: &InertFinalCompilerModuleCommitmentV3,
        handoff: &CompilerModuleHandoffV2,
        mutate: impl FnOnce(&mut [u8], WireOffsets),
    ) {
        let mut substituted = original.canonical_bytes().to_vec();
        let wire = offsets(&substituted);
        mutate(&mut substituted, wire);
        rehash(&mut substituted);
        let decoded = InertFinalCompilerModuleCommitmentV3::decode(&substituted)
            .expect("structurally valid fully rehashed substitution");
        assert_ne!(decoded, *original);
        assert!(!decoded.matches_handoff(handoff));
        assert!(!decoded.authenticates_compiler_origin());
        assert!(!decoded.grants_publication_authority());
    }

    #[test]
    fn exact_handoff_round_trips_without_embedding_nested_payloads() {
        for (kind, module) in [
            (CompilerModuleKindV1::LlvmTextIr, LLVM_IR),
            (
                CompilerModuleKindV1::LlvmBitcode,
                &[0x42, 0x43, 0xc0, 0xde, 0x01][..],
            ),
        ] {
            let handoff = handoff(kind, TARGET, CodeObjectVersion::V5, module, "helper_a");
            let first = InertFinalCompilerModuleCommitmentV3::from_handoff(&handoff).unwrap();
            let repeated = InertFinalCompilerModuleCommitmentV3::from_handoff(&handoff).unwrap();
            let decoded = InertFinalCompilerModuleCommitmentV3::decode(first.canonical_bytes())
                .expect("strict canonical round trip");
            let via_try_from =
                InertFinalCompilerModuleCommitmentV3::try_from(first.canonical_bytes()).unwrap();

            assert_eq!(first, repeated);
            assert_eq!(decoded, first);
            assert_eq!(via_try_from, first);
            assert_eq!(first.kind(), kind);
            assert_eq!(first.target(), target(TARGET));
            assert_eq!(first.code_object_version(), CodeObjectVersion::V5);
            assert_eq!(
                first.module_identity().sha256(),
                handoff.module_identity().sha256()
            );
            assert_eq!(
                first.module_identity().byte_len(),
                handoff.module_identity().byte_len()
            );
            assert_eq!(
                first.envelope_identity(),
                &handoff.envelope().identity().as_bytes()
            );
            assert_eq!(
                first.envelope_identity(),
                first.envelope_canonical_identity()
            );
            assert_eq!(
                first.envelope_byte_len(),
                handoff.envelope().canonical_bytes().len() as u64
            );
            assert_eq!(
                first.symbol_manifest_identity().sha256(),
                handoff.symbol_manifest().identity().sha256()
            );
            assert_eq!(
                first.handoff_identity().sha256(),
                handoff.identity().sha256()
            );
            assert!(
                first
                    .identity()
                    .matches_canonical_bytes(first.canonical_bytes())
            );
            assert!(first.matches_handoff(&handoff));
            assert_eq!(first.canonical_bytes().len(), FIXED_BYTES_V3 + TARGET.len());
            assert!(
                !first
                    .canonical_bytes()
                    .windows(module.len())
                    .any(|window| window == module)
            );
            assert!(first.canonical_bytes().len() <= MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3);
        }
    }

    #[test]
    fn policy_and_authority_classification_are_explicit() {
        let value =
            InertFinalCompilerModuleCommitmentV3::from_handoff(&baseline_handoff()).unwrap();
        assert_eq!(
            value.policy(),
            FinalCompilerModuleCommitmentPolicyV3::ExactCompilerModuleHandoffV2ContentOnly
        );
        assert_eq!(
            value.policy() as u16,
            FINAL_COMPILER_MODULE_COMMITMENT_POLICY_V3
        );
        assert!(!value.authenticates_compiler_origin());
        assert!(!value.authenticates_producer());
        assert!(!value.establishes_semantic_refinement());
        assert!(!value.grants_publication_authority());
        assert!(!value.grants_link_authority());
        assert!(!value.grants_load_authority());
        assert!(!value.grants_launch_authority());
    }

    #[test]
    fn every_committed_axis_rejects_substitution_against_the_exact_handoff() {
        let handoff = baseline_handoff();
        let original = InertFinalCompilerModuleCommitmentV3::from_handoff(&handoff).unwrap();

        assert_rehashed_substitution_is_inert(&original, &handoff, |bytes, wire| {
            bytes[wire.kind] = module_kind_tag(CompilerModuleKindV1::LlvmBitcode);
        });
        assert_rehashed_substitution_is_inert(&original, &handoff, |bytes, wire| {
            assert_eq!(TARGET.len(), OTHER_TARGET.len());
            bytes[wire.target..wire.target + TARGET.len()].copy_from_slice(OTHER_TARGET.as_bytes());
        });
        assert_rehashed_substitution_is_inert(&original, &handoff, |bytes, wire| {
            bytes[wire.code_object_version] = code_object_version_tag(CodeObjectVersion::V6);
        });
        assert_rehashed_substitution_is_inert(&original, &handoff, |bytes, wire| {
            bytes[wire.module_sha256] ^= 1;
        });
        assert_rehashed_substitution_is_inert(&original, &handoff, |bytes, wire| {
            bytes[wire.module_len..wire.module_len + 8]
                .copy_from_slice(&(handoff.module_identity().byte_len() + 1).to_le_bytes());
        });
        assert_rehashed_substitution_is_inert(&original, &handoff, |bytes, wire| {
            bytes[wire.envelope_identity] ^= 1;
            bytes[wire.envelope_canonical_identity] ^= 1;
        });
        assert_rehashed_substitution_is_inert(&original, &handoff, |bytes, wire| {
            bytes[wire.envelope_len..wire.envelope_len + 8].copy_from_slice(
                &(handoff.envelope().canonical_bytes().len() as u64 + 1).to_le_bytes(),
            );
        });
        assert_rehashed_substitution_is_inert(&original, &handoff, |bytes, wire| {
            bytes[wire.manifest_sha256] ^= 1;
        });
        assert_rehashed_substitution_is_inert(&original, &handoff, |bytes, wire| {
            bytes[wire.manifest_len..wire.manifest_len + 8].copy_from_slice(
                &(handoff.symbol_manifest().identity().byte_len() + 1).to_le_bytes(),
            );
        });
        assert_rehashed_substitution_is_inert(&original, &handoff, |bytes, wire| {
            bytes[wire.handoff_sha256] ^= 1;
        });
        assert_rehashed_substitution_is_inert(&original, &handoff, |bytes, wire| {
            bytes[wire.handoff_len..wire.handoff_len + 8]
                .copy_from_slice(&(handoff.identity().byte_len() + 1).to_le_bytes());
        });
    }

    #[test]
    fn exact_handoff_changes_always_change_the_commitment() {
        let baseline = baseline_handoff();
        let expected = InertFinalCompilerModuleCommitmentV3::from_handoff(&baseline).unwrap();
        let mut changed_module = LLVM_IR.to_vec();
        *changed_module.last_mut().unwrap() = b' ';
        let mut longer_module = LLVM_IR.to_vec();
        longer_module.push(b'\n');
        let variants = [
            handoff(
                CompilerModuleKindV1::LlvmBitcode,
                TARGET,
                CodeObjectVersion::V5,
                LLVM_IR,
                "helper_a",
            ),
            handoff(
                CompilerModuleKindV1::LlvmTextIr,
                OTHER_TARGET,
                CodeObjectVersion::V5,
                LLVM_IR,
                "helper_a",
            ),
            handoff(
                CompilerModuleKindV1::LlvmTextIr,
                TARGET,
                CodeObjectVersion::V6,
                LLVM_IR,
                "helper_a",
            ),
            handoff(
                CompilerModuleKindV1::LlvmTextIr,
                TARGET,
                CodeObjectVersion::V5,
                &changed_module,
                "helper_a",
            ),
            handoff(
                CompilerModuleKindV1::LlvmTextIr,
                TARGET,
                CodeObjectVersion::V5,
                &longer_module,
                "helper_a",
            ),
            handoff(
                CompilerModuleKindV1::LlvmTextIr,
                TARGET,
                CodeObjectVersion::V5,
                LLVM_IR,
                "helper_b",
            ),
            handoff(
                CompilerModuleKindV1::LlvmTextIr,
                TARGET,
                CodeObjectVersion::V5,
                LLVM_IR,
                "longer_helper",
            ),
        ];

        for variant in variants {
            let observed = InertFinalCompilerModuleCommitmentV3::from_handoff(&variant).unwrap();
            assert_ne!(observed.identity(), expected.identity());
            assert_ne!(observed.canonical_bytes(), expected.canonical_bytes());
            assert!(!expected.matches_handoff(&variant));
            assert!(!observed.matches_handoff(&baseline));
        }
    }

    #[test]
    fn envelope_declared_and_canonical_identities_must_agree() {
        let original =
            InertFinalCompilerModuleCommitmentV3::from_handoff(&baseline_handoff()).unwrap();
        for identity_axis in [
            offsets(original.canonical_bytes()).envelope_identity,
            offsets(original.canonical_bytes()).envelope_canonical_identity,
        ] {
            let mut bytes = original.canonical_bytes().to_vec();
            bytes[identity_axis] ^= 1;
            rehash(&mut bytes);
            assert_eq!(
                InertFinalCompilerModuleCommitmentV3::decode(&bytes),
                Err(FinalCompilerModuleCommitmentErrorV3::EnvelopeIdentityMismatch)
            );
        }
    }

    #[test]
    fn every_truncation_and_trailing_byte_is_rejected() {
        let encoded = InertFinalCompilerModuleCommitmentV3::from_handoff(&baseline_handoff())
            .unwrap()
            .canonical_bytes()
            .to_vec();
        for length in 0..encoded.len() {
            assert!(
                InertFinalCompilerModuleCommitmentV3::decode(&encoded[..length]).is_err(),
                "accepted prefix of length {length}"
            );
        }

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&trailing),
            Err(FinalCompilerModuleCommitmentErrorV3::TrailingBytes { trailing: 1 })
        );

        let mut declared_trailing = trailing;
        let wire = offsets(&encoded);
        let declared_trailing_len = declared_trailing.len() as u32;
        declared_trailing[wire.total_len..wire.total_len + 4]
            .copy_from_slice(&declared_trailing_len.to_le_bytes());
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&declared_trailing),
            Err(FinalCompilerModuleCommitmentErrorV3::TrailingBytes { trailing: 1 })
        );
    }

    #[test]
    fn physical_and_declared_bounds_are_rejected_before_retention() {
        let oversized = vec![0_u8; MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3 + 1];
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&oversized),
            Err(
                FinalCompilerModuleCommitmentErrorV3::CommitmentByteBoundExceeded {
                    actual: MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3 + 1,
                    max: MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3,
                }
            )
        );

        let original =
            InertFinalCompilerModuleCommitmentV3::from_handoff(&baseline_handoff()).unwrap();
        let wire = offsets(original.canonical_bytes());
        let mut target_too_large = original.canonical_bytes().to_vec();
        target_too_large[wire.target_len..wire.target_len + 4]
            .copy_from_slice(&((MAX_DEVICE_FFI_TARGET_BYTES_V1 as u32) + 1).to_le_bytes());
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&target_too_large),
            Err(
                FinalCompilerModuleCommitmentErrorV3::InvalidTargetByteLength {
                    observed: MAX_DEVICE_FFI_TARGET_BYTES_V1 + 1,
                    max: MAX_DEVICE_FFI_TARGET_BYTES_V1,
                }
            )
        );

        let mut declared_too_long = original.canonical_bytes().to_vec();
        declared_too_long[wire.total_len..wire.total_len + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&declared_too_long),
            Err(FinalCompilerModuleCommitmentErrorV3::Truncated)
        );

        for (field, offset, max) in [
            ("module", wire.module_len, MAX_COMPILER_MODULE_BYTES_V1),
            (
                "envelope",
                wire.envelope_len,
                MAX_COMPILER_FFI_ENVELOPE_BYTES_V1,
            ),
            (
                "symbol manifest",
                wire.manifest_len,
                MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1,
            ),
            (
                "V2 handoff",
                wire.handoff_len,
                MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
            ),
        ] {
            let mut declared_content_too_large = original.canonical_bytes().to_vec();
            declared_content_too_large[offset..offset + 8]
                .copy_from_slice(&(max as u64 + 1).to_le_bytes());
            rehash(&mut declared_content_too_large);
            assert_eq!(
                InertFinalCompilerModuleCommitmentV3::decode(&declared_content_too_large),
                Err(
                    FinalCompilerModuleCommitmentErrorV3::ContentByteBoundExceeded {
                        field,
                        observed: max as u64 + 1,
                        max: max as u64,
                    }
                )
            );
        }
    }

    #[test]
    fn strict_header_and_canonical_target_rules_are_enforced() {
        let original =
            InertFinalCompilerModuleCommitmentV3::from_handoff(&baseline_handoff()).unwrap();
        let wire = offsets(original.canonical_bytes());

        let mut bad_domain = original.canonical_bytes().to_vec();
        bad_domain[0] ^= 1;
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&bad_domain),
            Err(FinalCompilerModuleCommitmentErrorV3::InvalidDomain)
        );

        let mut bad_version = original.canonical_bytes().to_vec();
        bad_version[wire.version..wire.version + 2].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&bad_version),
            Err(FinalCompilerModuleCommitmentErrorV3::UnsupportedVersion { observed: 2 })
        );

        let mut bad_policy = original.canonical_bytes().to_vec();
        bad_policy[wire.policy..wire.policy + 2].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&bad_policy),
            Err(FinalCompilerModuleCommitmentErrorV3::UnsupportedPolicy { observed: 2 })
        );

        for reserved in [wire.flags, wire.header_reserved, wire.field_reserved] {
            let mut bad_reserved = original.canonical_bytes().to_vec();
            bad_reserved[reserved] = 1;
            assert!(InertFinalCompilerModuleCommitmentV3::decode(&bad_reserved).is_err());
        }

        let mut bad_kind = original.canonical_bytes().to_vec();
        bad_kind[wire.kind] = 0;
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&bad_kind),
            Err(FinalCompilerModuleCommitmentErrorV3::InvalidModuleKind { observed: 0 })
        );

        let mut bad_cov = original.canonical_bytes().to_vec();
        bad_cov[wire.code_object_version] = 0;
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&bad_cov),
            Err(FinalCompilerModuleCommitmentErrorV3::InvalidCodeObjectVersion { observed: 0 })
        );

        let featured_handoff = handoff(
            CompilerModuleKindV1::LlvmTextIr,
            FEATURED_TARGET,
            CodeObjectVersion::V5,
            LLVM_IR,
            "helper_a",
        );
        let featured =
            InertFinalCompilerModuleCommitmentV3::from_handoff(&featured_handoff).unwrap();
        let featured_wire = offsets(featured.canonical_bytes());
        assert_eq!(FEATURED_TARGET.len(), NONCANONICAL_FEATURED_TARGET.len());
        let mut noncanonical_target = featured.canonical_bytes().to_vec();
        noncanonical_target[featured_wire.target..featured_wire.target + FEATURED_TARGET.len()]
            .copy_from_slice(NONCANONICAL_FEATURED_TARGET);
        rehash(&mut noncanonical_target);
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&noncanonical_target),
            Err(FinalCompilerModuleCommitmentErrorV3::InvalidTarget)
        );

        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(baseline_handoff().canonical_bytes()),
            Err(FinalCompilerModuleCommitmentErrorV3::InvalidDomain)
        );
    }

    #[test]
    fn zero_identities_and_lengths_are_rejected() {
        let original =
            InertFinalCompilerModuleCommitmentV3::from_handoff(&baseline_handoff()).unwrap();
        let wire = offsets(original.canonical_bytes());

        for (field, offset) in [
            ("module", wire.module_sha256),
            ("envelope", wire.envelope_identity),
            ("envelope canonical bytes", wire.envelope_canonical_identity),
            ("symbol manifest", wire.manifest_sha256),
            ("V2 handoff", wire.handoff_sha256),
        ] {
            let mut bytes = original.canonical_bytes().to_vec();
            bytes[offset..offset + SHA256_BYTES].fill(0);
            rehash(&mut bytes);
            assert_eq!(
                InertFinalCompilerModuleCommitmentV3::decode(&bytes),
                Err(FinalCompilerModuleCommitmentErrorV3::ZeroIdentity { field })
            );
        }

        for (field, offset) in [
            ("module", wire.module_len),
            ("envelope", wire.envelope_len),
            ("symbol manifest", wire.manifest_len),
            ("V2 handoff", wire.handoff_len),
        ] {
            let mut bytes = original.canonical_bytes().to_vec();
            bytes[offset..offset + 8].fill(0);
            rehash(&mut bytes);
            assert_eq!(
                InertFinalCompilerModuleCommitmentV3::decode(&bytes),
                Err(FinalCompilerModuleCommitmentErrorV3::ZeroByteLength { field })
            );
        }

        let mut zero_terminal = original.canonical_bytes().to_vec();
        zero_terminal[wire.terminal_identity..].fill(0);
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&zero_terminal),
            Err(FinalCompilerModuleCommitmentErrorV3::ZeroIdentity {
                field: "commitment"
            })
        );
    }

    #[test]
    fn terminal_identity_and_true_reencoding_are_required() {
        let original =
            InertFinalCompilerModuleCommitmentV3::from_handoff(&baseline_handoff()).unwrap();
        let wire = offsets(original.canonical_bytes());
        let mut bad_terminal = original.canonical_bytes().to_vec();
        bad_terminal[wire.terminal_identity] ^= 1;
        assert_eq!(
            InertFinalCompilerModuleCommitmentV3::decode(&bad_terminal),
            Err(FinalCompilerModuleCommitmentErrorV3::CommitmentIdentityMismatch)
        );

        let decoded =
            InertFinalCompilerModuleCommitmentV3::decode(original.canonical_bytes()).unwrap();
        assert_eq!(decoded.canonical_bytes(), original.canonical_bytes());
        assert!(
            decoded
                .identity()
                .matches_canonical_bytes(decoded.canonical_bytes())
        );
    }
}
