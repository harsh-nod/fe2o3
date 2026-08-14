use std::{error::Error, fmt, str};

use sha2::{Digest, Sha256};

use super::{
    CodeObjectVersion, CompilerFfiContractV1, CompilerFfiEffectAbiIdentityV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiEnvelopeError, CompilerFfiEnvelopeV1,
    CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerIdentityV1, CompilerFfiSourceOwnerV1,
    DeviceFfiContractIdV1, DeviceFfiDirectionV1, DeviceTargetV1, ENCODED_CONTRACT_FIXED_BYTES_V1,
    ENVELOPE_DOMAIN_V1, MAX_COMPILER_FFI_CONTRACTS_V1, MAX_COMPILER_FFI_CRATE_LABEL_BYTES_V1,
    MAX_COMPILER_FFI_ENVELOPE_BYTES_V1, MAX_COMPILER_FFI_INSTANCE_SYMBOL_BYTES_V1,
    MAX_COMPILER_FFI_ITEM_PATH_BYTES_V1, MAX_DEVICE_FFI_EFFECT_BYTES_V1,
    MAX_DEVICE_FFI_PHYSICAL_ABI_BYTES_V1, MAX_DEVICE_FFI_SYMBOL_BYTES_V1,
    MAX_DEVICE_FFI_TARGET_BYTES_V1, code_object_version_tag,
};

/// Maximum exact LLVM module bytes carried by one handoff value.
pub const MAX_COMPILER_MODULE_BYTES_V1: usize = 64 * 1024 * 1024;

const HANDOFF_DOMAIN_V1: &[u8] = b"FE2O3/COMPILER-MODULE-HANDOFF/V1\0";
const MODULE_IDENTITY_BYTES_V1: usize = 32 + 8;
const HANDOFF_FIXED_BYTES_V1: usize =
    HANDOFF_DOMAIN_V1.len() + 4 + 1 + 1 + MODULE_IDENTITY_BYTES_V1 + 4;

/// Maximum canonical bytes in one module handoff, including its bounded envelope.
pub const MAX_COMPILER_MODULE_HANDOFF_BYTES_V1: usize = HANDOFF_FIXED_BYTES_V1
    + MAX_DEVICE_FFI_TARGET_BYTES_V1
    + MAX_COMPILER_FFI_ENVELOPE_BYTES_V1
    + MAX_COMPILER_MODULE_BYTES_V1;

/// Exact representation of the neutral LLVM module bytes in a handoff.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompilerModuleKindV1 {
    LlvmTextIr = 1,
    LlvmBitcode = 2,
}

/// SHA-256 and byte length of the exact module payload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerModuleIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl CompilerModuleIdentityV1 {
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn matches(self, bytes: &[u8]) -> bool {
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        self.byte_len == bytes.len() as u64 && self.sha256 == actual
    }

    pub(super) fn calculate(bytes: &[u8]) -> Self {
        Self {
            sha256: Sha256::digest(bytes).into(),
            byte_len: bytes.len() as u64,
        }
    }
}

/// Bounded canonical data joining exact LLVM module bytes to one FFI envelope.
///
/// Public construction is intentional: this value records byte-level consistency only. It does
/// not authenticate who produced the bytes and grants no compiler, link, load, or launch authority.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerModuleHandoffV1 {
    kind: CompilerModuleKindV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    module_identity: CompilerModuleIdentityV1,
    envelope: CompilerFfiEnvelopeV1,
    canonical_bytes: Vec<u8>,
    module_offset: usize,
}

impl fmt::Debug for CompilerModuleHandoffV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerModuleHandoffV1")
            .field("kind", &self.kind)
            .field("target", &self.target)
            .field("code_object_version", &self.code_object_version)
            .field("module_identity", &self.module_identity)
            .field("envelope_identity", &self.envelope.identity())
            .finish_non_exhaustive()
    }
}

/// Owned exact module payload extracted from one structurally validated handoff.
///
/// The payload preserves its typed representation and declared identity with the exact retained
/// bytes. It is neutral data: neither extraction nor ownership authenticates its producer or grants
/// compiler, worker, link, load, or launch authority.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerModulePayloadV1 {
    kind: CompilerModuleKindV1,
    identity: CompilerModuleIdentityV1,
    bytes: Vec<u8>,
}

impl fmt::Debug for CompilerModulePayloadV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerModulePayloadV1")
            .field("kind", &self.kind)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl CompilerModulePayloadV1 {
    pub(super) fn from_validated(
        kind: CompilerModuleKindV1,
        identity: CompilerModuleIdentityV1,
        bytes: Vec<u8>,
    ) -> Self {
        debug_assert!(identity.matches(&bytes));
        Self {
            kind,
            identity,
            bytes,
        }
    }

    pub const fn kind(&self) -> CompilerModuleKindV1 {
        self.kind
    }

    pub const fn identity(&self) -> CompilerModuleIdentityV1 {
        self.identity
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Moves out the exact retained module bytes without another payload allocation.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_worker_authority(&self) -> bool {
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

/// Owned envelope and module components retained from one coherent handoff.
///
/// There is no public constructor. [`CompilerModuleHandoffV1::into_parts`] is the only way to
/// obtain this decomposition, so callers do not need to parse offsets or reconstruct envelope
/// fields when passing the data to the finalizer. This structural relationship carries no origin
/// or execution authority.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerModuleHandoffPartsV1 {
    envelope: CompilerFfiEnvelopeV1,
    module: CompilerModulePayloadV1,
}

impl fmt::Debug for CompilerModuleHandoffPartsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerModuleHandoffPartsV1")
            .field("target", &self.target())
            .field("code_object_version", &self.code_object_version())
            .field("envelope_identity", &self.envelope.identity())
            .field("module", &self.module)
            .finish_non_exhaustive()
    }
}

impl CompilerModuleHandoffPartsV1 {
    pub const fn target(&self) -> DeviceTargetV1 {
        self.envelope.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.envelope.code_object_version()
    }

    pub const fn envelope(&self) -> &CompilerFfiEnvelopeV1 {
        &self.envelope
    }

    pub const fn module(&self) -> &CompilerModulePayloadV1 {
        &self.module
    }

    /// Moves out both coherent components without exposing their private representation.
    pub fn into_envelope_and_module(self) -> (CompilerFfiEnvelopeV1, CompilerModulePayloadV1) {
        (self.envelope, self.module)
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_worker_authority(&self) -> bool {
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

impl CompilerModuleHandoffV1 {
    pub fn new(
        kind: CompilerModuleKindV1,
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        envelope: CompilerFfiEnvelopeV1,
        module_bytes: &[u8],
    ) -> Result<Self, CompilerModuleHandoffErrorV1> {
        validate_module_bytes(kind, module_bytes)?;
        if envelope.target() != target {
            return Err(CompilerModuleHandoffErrorV1::TargetMismatch);
        }
        if envelope.code_object_version() != code_object_version {
            return Err(CompilerModuleHandoffErrorV1::CodeObjectVersionMismatch);
        }

        let target_text = target.to_string();
        if target_text.is_empty() || target_text.len() > MAX_DEVICE_FFI_TARGET_BYTES_V1 {
            return Err(CompilerModuleHandoffErrorV1::InvalidTarget);
        }
        let envelope_bytes = envelope.canonical_bytes();
        if envelope_bytes.len() > MAX_COMPILER_FFI_ENVELOPE_BYTES_V1 {
            return Err(CompilerModuleHandoffErrorV1::EnvelopeByteBoundExceeded);
        }
        let exact_size = HANDOFF_FIXED_BYTES_V1
            .checked_add(target_text.len())
            .and_then(|size| size.checked_add(envelope_bytes.len()))
            .and_then(|size| size.checked_add(module_bytes.len()))
            .ok_or(CompilerModuleHandoffErrorV1::HandoffByteBoundExceeded)?;
        if exact_size > MAX_COMPILER_MODULE_HANDOFF_BYTES_V1 {
            return Err(CompilerModuleHandoffErrorV1::HandoffByteBoundExceeded);
        }

        let module_identity = CompilerModuleIdentityV1::calculate(module_bytes);
        let mut canonical_bytes = Vec::with_capacity(exact_size);
        canonical_bytes.extend_from_slice(HANDOFF_DOMAIN_V1);
        push_u32(&mut canonical_bytes, target_text.len())?;
        canonical_bytes.extend_from_slice(target_text.as_bytes());
        canonical_bytes.push(code_object_version_tag(code_object_version) as u8);
        canonical_bytes.push(kind as u8);
        canonical_bytes.extend_from_slice(module_identity.sha256());
        canonical_bytes.extend_from_slice(&module_identity.byte_len().to_le_bytes());
        push_u32(&mut canonical_bytes, envelope_bytes.len())?;
        canonical_bytes.extend_from_slice(envelope_bytes);
        let module_offset = canonical_bytes.len();
        canonical_bytes.extend_from_slice(module_bytes);
        debug_assert_eq!(canonical_bytes.len(), exact_size);

        Ok(Self {
            kind,
            target,
            code_object_version,
            module_identity,
            envelope,
            canonical_bytes,
            module_offset,
        })
    }

    /// Strictly decodes one complete canonical handoff.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerModuleHandoffErrorV1> {
        if bytes.len() > MAX_COMPILER_MODULE_HANDOFF_BYTES_V1 {
            return Err(CompilerModuleHandoffErrorV1::HandoffByteBoundExceeded);
        }
        let mut cursor = Cursor::new(bytes);
        if cursor.take(HANDOFF_DOMAIN_V1.len())? != HANDOFF_DOMAIN_V1 {
            return Err(CompilerModuleHandoffErrorV1::InvalidMagic);
        }
        let target_text = cursor.text(MAX_DEVICE_FFI_TARGET_BYTES_V1)?;
        let target = DeviceTargetV1::parse(target_text)
            .map_err(|_| CompilerModuleHandoffErrorV1::InvalidTarget)?;
        let code_object_version = decode_code_object_version(cursor.byte()?)?;
        let kind = decode_module_kind(cursor.byte()?)?;
        let declared_digest = cursor.fixed::<32>()?;
        let declared_len_u64 = cursor.u64()?;
        let declared_len = usize::try_from(declared_len_u64)
            .map_err(|_| CompilerModuleHandoffErrorV1::ModuleByteBoundExceeded)?;
        if declared_len == 0 || declared_len > MAX_COMPILER_MODULE_BYTES_V1 {
            return Err(CompilerModuleHandoffErrorV1::ModuleByteBoundExceeded);
        }
        let envelope_len = cursor.u32_as_usize()?;
        if envelope_len == 0 || envelope_len > MAX_COMPILER_FFI_ENVELOPE_BYTES_V1 {
            return Err(CompilerModuleHandoffErrorV1::EnvelopeByteBoundExceeded);
        }
        let envelope_bytes = cursor.take(envelope_len)?;
        let module_bytes = cursor.take(declared_len)?;
        cursor.finish()?;

        let actual_digest: [u8; 32] = Sha256::digest(module_bytes).into();
        if actual_digest != declared_digest {
            return Err(CompilerModuleHandoffErrorV1::ModuleIdentityMismatch);
        }
        validate_module_bytes(kind, module_bytes)?;
        let envelope = decode_envelope(envelope_bytes)?;
        if envelope.target() != target {
            return Err(CompilerModuleHandoffErrorV1::TargetMismatch);
        }
        if envelope.code_object_version() != code_object_version {
            return Err(CompilerModuleHandoffErrorV1::CodeObjectVersionMismatch);
        }

        let decoded = Self::new(kind, target, code_object_version, envelope, module_bytes)?;
        if decoded.module_identity.byte_len() != declared_len_u64
            || decoded.module_identity.sha256() != &declared_digest
        {
            return Err(CompilerModuleHandoffErrorV1::ModuleIdentityMismatch);
        }
        if decoded.canonical_bytes() != bytes {
            return Err(CompilerModuleHandoffErrorV1::NonCanonicalEncoding);
        }
        Ok(decoded)
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

    pub const fn module_identity(&self) -> CompilerModuleIdentityV1 {
        self.module_identity
    }

    pub const fn envelope(&self) -> &CompilerFfiEnvelopeV1 {
        &self.envelope
    }

    pub fn module_bytes(&self) -> &[u8] {
        &self.canonical_bytes[self.module_offset..]
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Moves the retained envelope and exact module payload into opaque owned components.
    ///
    /// The module reuses the canonical buffer allocation. Removing the bounded wire prefix may
    /// move bytes within that allocation, but does not parse the wire format or allocate another
    /// module payload.
    pub fn into_parts(self) -> CompilerModuleHandoffPartsV1 {
        let Self {
            kind,
            module_identity,
            envelope,
            mut canonical_bytes,
            module_offset,
            ..
        } = self;
        canonical_bytes.drain(..module_offset);
        debug_assert!(module_identity.matches(&canonical_bytes));
        CompilerModuleHandoffPartsV1 {
            envelope,
            module: CompilerModulePayloadV1 {
                kind,
                identity: module_identity,
                bytes: canonical_bytes,
            },
        }
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_worker_authority(&self) -> bool {
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

impl<'a> TryFrom<&'a [u8]> for CompilerModuleHandoffV1 {
    type Error = CompilerModuleHandoffErrorV1;

    /// Uses the same bounded, strict, exact-reencoding decoder as [`Self::decode`].
    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        Self::decode(bytes)
    }
}

/// Failure to construct or strictly decode neutral module handoff data.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerModuleHandoffErrorV1 {
    EmptyModule,
    ModuleByteBoundExceeded,
    HandoffByteBoundExceeded,
    EnvelopeByteBoundExceeded,
    InvalidMagic,
    Truncated,
    TrailingBytes,
    TextByteBoundExceeded,
    InvalidUtf8,
    InvalidTarget,
    InvalidCodeObjectVersion,
    InvalidModuleKind,
    ModuleIdentityMismatch,
    TargetMismatch,
    CodeObjectVersionMismatch,
    NonCanonicalEncoding,
    Envelope(CompilerFfiEnvelopeError),
}

impl fmt::Display for CompilerModuleHandoffErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyModule => formatter.write_str("compiler module handoff has no module bytes"),
            Self::ModuleByteBoundExceeded => {
                formatter.write_str("compiler module byte bound exceeded")
            }
            Self::HandoffByteBoundExceeded => {
                formatter.write_str("compiler module handoff byte bound exceeded")
            }
            Self::EnvelopeByteBoundExceeded => {
                formatter.write_str("compiler FFI envelope byte bound exceeded")
            }
            Self::InvalidMagic => formatter.write_str("invalid compiler module handoff magic"),
            Self::Truncated => formatter.write_str("truncated compiler module handoff"),
            Self::TrailingBytes => formatter.write_str("trailing compiler module handoff bytes"),
            Self::TextByteBoundExceeded => {
                formatter.write_str("compiler module handoff text byte bound exceeded")
            }
            Self::InvalidUtf8 => formatter.write_str("invalid UTF-8 in compiler module handoff"),
            Self::InvalidTarget => formatter.write_str("invalid compiler module handoff target"),
            Self::InvalidCodeObjectVersion => {
                formatter.write_str("invalid compiler module handoff code-object version")
            }
            Self::InvalidModuleKind => {
                formatter.write_str("invalid compiler module handoff module kind")
            }
            Self::ModuleIdentityMismatch => {
                formatter.write_str("compiler module handoff identity mismatch")
            }
            Self::TargetMismatch => {
                formatter.write_str("compiler module handoff target disagrees with its envelope")
            }
            Self::CodeObjectVersionMismatch => formatter.write_str(
                "compiler module handoff code-object version disagrees with its envelope",
            ),
            Self::NonCanonicalEncoding => {
                formatter.write_str("noncanonical compiler module handoff encoding")
            }
            Self::Envelope(error) => write!(formatter, "invalid compiler FFI envelope: {error}"),
        }
    }
}

impl Error for CompilerModuleHandoffErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Envelope(error) => Some(error),
            _ => None,
        }
    }
}

pub(super) fn validate_module_bytes(
    kind: CompilerModuleKindV1,
    bytes: &[u8],
) -> Result<(), CompilerModuleHandoffErrorV1> {
    if bytes.is_empty() {
        return Err(CompilerModuleHandoffErrorV1::EmptyModule);
    }
    if bytes.len() > MAX_COMPILER_MODULE_BYTES_V1 {
        return Err(CompilerModuleHandoffErrorV1::ModuleByteBoundExceeded);
    }
    if kind == CompilerModuleKindV1::LlvmTextIr {
        str::from_utf8(bytes).map_err(|_| CompilerModuleHandoffErrorV1::InvalidUtf8)?;
    }
    Ok(())
}

pub(super) fn decode_module_kind(
    value: u8,
) -> Result<CompilerModuleKindV1, CompilerModuleHandoffErrorV1> {
    match value {
        1 => Ok(CompilerModuleKindV1::LlvmTextIr),
        2 => Ok(CompilerModuleKindV1::LlvmBitcode),
        _ => Err(CompilerModuleHandoffErrorV1::InvalidModuleKind),
    }
}

pub(super) fn decode_code_object_version(
    value: u8,
) -> Result<CodeObjectVersion, CompilerModuleHandoffErrorV1> {
    match value {
        4 => Ok(CodeObjectVersion::V4),
        5 => Ok(CodeObjectVersion::V5),
        6 => Ok(CodeObjectVersion::V6),
        _ => Err(CompilerModuleHandoffErrorV1::InvalidCodeObjectVersion),
    }
}

pub(super) fn decode_envelope(
    bytes: &[u8],
) -> Result<CompilerFfiEnvelopeV1, CompilerModuleHandoffErrorV1> {
    if bytes.len() > MAX_COMPILER_FFI_ENVELOPE_BYTES_V1 {
        return Err(CompilerModuleHandoffErrorV1::EnvelopeByteBoundExceeded);
    }
    let mut cursor = Cursor::new(bytes);
    if cursor.take(ENVELOPE_DOMAIN_V1.len())? != ENVELOPE_DOMAIN_V1 {
        return Err(CompilerModuleHandoffErrorV1::Envelope(
            CompilerFfiEnvelopeError::PreflightMismatch,
        ));
    }
    let target_text = cursor.text(MAX_DEVICE_FFI_TARGET_BYTES_V1)?;
    let target = DeviceTargetV1::parse(target_text)
        .map_err(|_| CompilerModuleHandoffErrorV1::InvalidTarget)?;
    let code_object_version = decode_code_object_version(cursor.byte()?)?;
    let count = cursor.u32_as_usize()?;
    if count == 0 {
        cursor.finish()?;
        let envelope =
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, code_object_version)
                .map_err(CompilerModuleHandoffErrorV1::Envelope)?;
        if envelope.canonical_bytes() != bytes {
            return Err(CompilerModuleHandoffErrorV1::NonCanonicalEncoding);
        }
        return Ok(envelope);
    }
    if count > MAX_COMPILER_FFI_CONTRACTS_V1 {
        return Err(CompilerModuleHandoffErrorV1::Envelope(
            CompilerFfiEnvelopeError::TooManyContracts { count },
        ));
    }

    let minimum_contract_bytes = count
        .checked_mul(ENCODED_CONTRACT_FIXED_BYTES_V1 + 7 * 4)
        .ok_or(CompilerModuleHandoffErrorV1::EnvelopeByteBoundExceeded)?;
    if cursor.remaining() < minimum_contract_bytes {
        return Err(CompilerModuleHandoffErrorV1::Truncated);
    }
    let mut builder = CompilerFfiEnvelopeBuilderV1::new(target, code_object_version, count)
        .map_err(CompilerModuleHandoffErrorV1::Envelope)?;
    for _ in 0..count {
        let contract_identity = DeviceFfiContractIdV1::from_bytes(cursor.fixed::<32>()?);
        let direction = match cursor.byte()? {
            1 => DeviceFfiDirectionV1::Import,
            2 => DeviceFfiDirectionV1::Export,
            _ => {
                return Err(CompilerModuleHandoffErrorV1::Envelope(
                    CompilerFfiEnvelopeError::DirectionRoleMismatch,
                ));
            }
        };
        let role = match cursor.byte()? {
            1 => CompilerFfiLinkRoleV1::RequiresExternalDefinition,
            2 => CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
            _ => {
                return Err(CompilerModuleHandoffErrorV1::Envelope(
                    CompilerFfiEnvelopeError::DirectionRoleMismatch,
                ));
            }
        };
        let declared_owner_identity = CompilerFfiSourceOwnerIdentityV1(cursor.fixed::<32>()?);
        let crate_label = cursor.text(MAX_COMPILER_FFI_CRATE_LABEL_BYTES_V1)?;
        let item_path = cursor.text(MAX_COMPILER_FFI_ITEM_PATH_BYTES_V1)?;
        let def_path_hash = cursor.fixed::<16>()?;
        let concrete_instance_symbol = cursor.text(MAX_COMPILER_FFI_INSTANCE_SYMBOL_BYTES_V1)?;
        let symbol = cursor.text(MAX_DEVICE_FFI_SYMBOL_BYTES_V1)?;
        let contract_target_text = cursor.text(MAX_DEVICE_FFI_TARGET_BYTES_V1)?;
        let contract_target = DeviceTargetV1::parse(contract_target_text)
            .map_err(|_| CompilerModuleHandoffErrorV1::InvalidTarget)?;
        let physical_abi = cursor.text(MAX_DEVICE_FFI_PHYSICAL_ABI_BYTES_V1)?;
        let effects = cursor.text(MAX_DEVICE_FFI_EFFECT_BYTES_V1)?;
        let declared_effect_identity = CompilerFfiEffectAbiIdentityV1(cursor.fixed::<32>()?);
        let semantic_identity = cursor.fixed::<32>()?;

        let source_owner = CompilerFfiSourceOwnerV1::new(
            crate_label,
            item_path,
            def_path_hash,
            concrete_instance_symbol,
        )
        .map_err(CompilerModuleHandoffErrorV1::Envelope)?;
        if source_owner.identity() != declared_owner_identity {
            return Err(CompilerModuleHandoffErrorV1::NonCanonicalEncoding);
        }
        let contract = CompilerFfiContractV1::new(
            contract_identity,
            direction,
            role,
            contract_target,
            code_object_version,
            source_owner,
            symbol,
            physical_abi,
            effects,
            semantic_identity,
        )
        .map_err(CompilerModuleHandoffErrorV1::Envelope)?;
        if contract.effect_abi_identity() != declared_effect_identity {
            return Err(CompilerModuleHandoffErrorV1::NonCanonicalEncoding);
        }
        builder
            .push(contract)
            .map_err(CompilerModuleHandoffErrorV1::Envelope)?;
    }
    cursor.finish()?;
    let envelope = builder
        .finish()
        .map_err(CompilerModuleHandoffErrorV1::Envelope)?;
    if envelope.canonical_bytes() != bytes {
        return Err(CompilerModuleHandoffErrorV1::NonCanonicalEncoding);
    }
    Ok(envelope)
}

fn push_u32(bytes: &mut Vec<u8>, value: usize) -> Result<(), CompilerModuleHandoffErrorV1> {
    let value =
        u32::try_from(value).map_err(|_| CompilerModuleHandoffErrorV1::HandoffByteBoundExceeded)?;
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }

    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], CompilerModuleHandoffErrorV1> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(CompilerModuleHandoffErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(CompilerModuleHandoffErrorV1::Truncated)?;
        self.position = end;
        Ok(value)
    }

    pub(super) fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], CompilerModuleHandoffErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| CompilerModuleHandoffErrorV1::Truncated)
    }

    pub(super) fn byte(&mut self) -> Result<u8, CompilerModuleHandoffErrorV1> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn u32_as_usize(&mut self) -> Result<usize, CompilerModuleHandoffErrorV1> {
        Ok(u32::from_le_bytes(self.fixed::<4>()?) as usize)
    }

    pub(super) fn u64(&mut self) -> Result<u64, CompilerModuleHandoffErrorV1> {
        Ok(u64::from_le_bytes(self.fixed::<8>()?))
    }

    pub(super) fn text(&mut self, max: usize) -> Result<&'a str, CompilerModuleHandoffErrorV1> {
        let len = self.u32_as_usize()?;
        if len == 0 || len > max {
            return Err(CompilerModuleHandoffErrorV1::TextByteBoundExceeded);
        }
        str::from_utf8(self.take(len)?).map_err(|_| CompilerModuleHandoffErrorV1::InvalidUtf8)
    }

    pub(super) fn finish(self) -> Result<(), CompilerModuleHandoffErrorV1> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(CompilerModuleHandoffErrorV1::TrailingBytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reserved_fe2o3_symbols::{
        DEVICE_FFI_DIRECTION_EXPORT_V1, DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1,
        derive_device_ffi_contract_id_v1,
    };

    const IMPORT_ABI: &str =
        "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]";
    const EXPORT_ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    const LLVM_IR: &[u8] =
        b"; ModuleID = 'handoff'\ndefine amdgpu_kernel void @kernel() { ret void }\n";

    fn target() -> DeviceTargetV1 {
        DeviceTargetV1::parse("gfx942:xnack-").unwrap()
    }

    fn owner(byte: u8, item: &str) -> CompilerFfiSourceOwnerV1 {
        CompilerFfiSourceOwnerV1::new(
            "ffi_crate",
            &format!("ffi_crate::{item}"),
            [byte; 16],
            &format!("_RINvNtCs1234_ffi_crate{item}"),
        )
        .unwrap()
    }

    fn contract(
        direction: DeviceFfiDirectionV1,
        symbol: &str,
        abi: &str,
        effects: &str,
        semantic_byte: u8,
        source_owner: CompilerFfiSourceOwnerV1,
    ) -> CompilerFfiContractV1 {
        let semantic_identity = [semantic_byte; 32];
        let semantic_text = super::super::lower_hex(&semantic_identity);
        let direction_tag = match direction {
            DeviceFfiDirectionV1::Import => DEVICE_FFI_DIRECTION_IMPORT_V1,
            DeviceFfiDirectionV1::Export => DEVICE_FFI_DIRECTION_EXPORT_V1,
        };
        let id = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: direction_tag,
            symbol,
            calling_convention: "C",
            code_object_version: 5,
            target: "gfx942:xnack-",
            physical_abi: abi,
            effects,
            semantic_identity: &semantic_text,
        });
        CompilerFfiContractV1::new(
            id,
            direction,
            match direction {
                DeviceFfiDirectionV1::Import => CompilerFfiLinkRoleV1::RequiresExternalDefinition,
                DeviceFfiDirectionV1::Export => {
                    CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition
                }
            },
            target(),
            CodeObjectVersion::V5,
            source_owner,
            symbol,
            abi,
            effects,
            semantic_identity,
        )
        .unwrap()
    }

    fn envelope() -> CompilerFfiEnvelopeV1 {
        let mut builder =
            CompilerFfiEnvelopeBuilderV1::new(target(), CodeObjectVersion::V5, 2).unwrap();
        builder
            .push(contract(
                DeviceFfiDirectionV1::Import,
                "external_add",
                IMPORT_ABI,
                "read_global",
                0x11,
                owner(1, "external_add"),
            ))
            .unwrap();
        builder
            .push(contract(
                DeviceFfiDirectionV1::Export,
                "rust_helper",
                EXPORT_ABI,
                "none",
                0x22,
                owner(2, "rust_helper"),
            ))
            .unwrap();
        builder.finish().unwrap()
    }

    fn handoff(kind: CompilerModuleKindV1, module: &[u8]) -> CompilerModuleHandoffV1 {
        CompilerModuleHandoffV1::new(kind, target(), CodeObjectVersion::V5, envelope(), module)
            .unwrap()
    }

    #[derive(Clone, Copy)]
    struct HandoffOffsets {
        target_start: usize,
        code_object_version: usize,
        kind: usize,
        digest: usize,
        module_len: usize,
        envelope_len: usize,
        envelope_start: usize,
        module_start: usize,
    }

    fn read_u32(bytes: &[u8], offset: usize) -> usize {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
    }

    fn offsets(bytes: &[u8]) -> HandoffOffsets {
        let target_len_offset = HANDOFF_DOMAIN_V1.len();
        let target_start = target_len_offset + 4;
        let code_object_version = target_start + read_u32(bytes, target_len_offset);
        let kind = code_object_version + 1;
        let digest = kind + 1;
        let module_len = digest + 32;
        let envelope_len = module_len + 8;
        let envelope_start = envelope_len + 4;
        let module_start = envelope_start + read_u32(bytes, envelope_len);
        HandoffOffsets {
            target_start,
            code_object_version,
            kind,
            digest,
            module_len,
            envelope_len,
            envelope_start,
            module_start,
        }
    }

    fn envelope_contract_ranges(envelope: &[u8]) -> Vec<std::ops::Range<usize>> {
        let mut position = ENVELOPE_DOMAIN_V1.len();
        position += 4 + read_u32(envelope, position);
        position += 1;
        let count = read_u32(envelope, position);
        position += 4;
        let mut ranges = Vec::with_capacity(count);
        for _ in 0..count {
            let start = position;
            position += 32 + 1 + 1 + 32;
            position += 4 + read_u32(envelope, position);
            position += 4 + read_u32(envelope, position);
            position += 16;
            for _ in 0..5 {
                position += 4 + read_u32(envelope, position);
            }
            position += 32 + 32;
            ranges.push(start..position);
        }
        assert_eq!(position, envelope.len());
        ranges
    }

    fn envelope_header_offsets(envelope: &[u8]) -> (usize, usize) {
        let target_len_offset = ENVELOPE_DOMAIN_V1.len();
        let code_object_version = target_len_offset + 4 + read_u32(envelope, target_len_offset);
        (code_object_version, code_object_version + 1)
    }

    #[test]
    fn text_and_bitcode_round_trip_exact_canonical_bytes() {
        for (kind, module) in [
            (CompilerModuleKindV1::LlvmTextIr, LLVM_IR),
            (
                CompilerModuleKindV1::LlvmBitcode,
                &[0x42, 0x43, 0xc0, 0xde, 0xff][..],
            ),
        ] {
            let first = handoff(kind, module);
            let second = CompilerModuleHandoffV1::decode(first.canonical_bytes()).unwrap();
            let via_try_from = CompilerModuleHandoffV1::try_from(first.canonical_bytes()).unwrap();

            assert_eq!(second, first);
            assert_eq!(via_try_from, first);
            assert_eq!(second.kind(), kind);
            assert_eq!(second.target(), target());
            assert_eq!(second.code_object_version(), CodeObjectVersion::V5);
            assert_eq!(second.module_bytes(), module);
            assert!(second.module_identity().matches(module));
            assert_eq!(second.envelope().identity(), first.envelope().identity());
            assert!(second.canonical_bytes().starts_with(HANDOFF_DOMAIN_V1));
            assert!(!second.authenticates_compiler_origin());
            assert!(!second.grants_link_authority());
            assert!(!second.grants_load_authority());
            assert!(!second.grants_launch_authority());
        }
    }

    #[test]
    fn module_identity_and_encoding_bind_every_payload_byte() {
        let first = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let mut changed_module = LLVM_IR.to_vec();
        *changed_module.last_mut().unwrap() = b' ';
        let second = handoff(CompilerModuleKindV1::LlvmTextIr, &changed_module);

        assert_ne!(first.module_identity(), second.module_identity());
        assert_ne!(first.canonical_bytes(), second.canonical_bytes());
        assert_eq!(first.module_identity().byte_len(), LLVM_IR.len() as u64);
        assert_eq!(
            first.module_bytes().as_ptr(),
            first.canonical_bytes()[offsets(first.canonical_bytes()).module_start..].as_ptr()
        );
    }

    #[test]
    fn owned_parts_reuse_retained_data_without_exposing_authority() {
        let handoff = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let expected_envelope_identity = handoff.envelope().identity();
        let expected_module_identity = handoff.module_identity();
        let canonical_allocation = handoff.canonical_bytes.as_ptr();
        let canonical_capacity = handoff.canonical_bytes.capacity();

        let parts = handoff.into_parts();
        assert_eq!(parts.target(), target());
        assert_eq!(parts.code_object_version(), CodeObjectVersion::V5);
        assert_eq!(parts.envelope().identity(), expected_envelope_identity);
        assert_eq!(parts.module().kind(), CompilerModuleKindV1::LlvmTextIr);
        assert_eq!(parts.module().identity(), expected_module_identity);
        assert_eq!(parts.module().bytes(), LLVM_IR);
        assert_eq!(parts.module.bytes.as_ptr(), canonical_allocation);
        assert_eq!(parts.module.bytes.capacity(), canonical_capacity);
        assert!(!parts.authenticates_compiler_origin());
        assert!(!parts.grants_compiler_authority());
        assert!(!parts.grants_worker_authority());
        assert!(!parts.grants_link_authority());
        assert!(!parts.grants_load_authority());
        assert!(!parts.grants_launch_authority());
        assert!(!parts.module().authenticates_compiler_origin());
        assert!(!parts.module().grants_compiler_authority());
        assert!(!parts.module().grants_worker_authority());
        assert!(!parts.module().grants_link_authority());
        assert!(!parts.module().grants_load_authority());
        assert!(!parts.module().grants_launch_authority());

        let (envelope, module) = parts.into_envelope_and_module();
        assert_eq!(envelope.identity(), expected_envelope_identity);
        let module_allocation = module.bytes.as_ptr();
        let module_bytes = module.into_bytes();
        assert_eq!(module_bytes, LLVM_IR);
        assert_eq!(module_bytes.as_ptr(), module_allocation);
    }

    #[test]
    fn coordinated_payload_and_digest_rewrite_is_new_authority_free_data() {
        let original = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let mut encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        encoded[location.module_start + 2] ^= 1;
        let digest: [u8; 32] = Sha256::digest(&encoded[location.module_start..]).into();
        encoded[location.digest..location.digest + 32].copy_from_slice(&digest);

        let changed = CompilerModuleHandoffV1::decode(&encoded).unwrap();
        assert_ne!(changed.module_identity(), original.module_identity());
        assert_ne!(changed.module_bytes(), original.module_bytes());
        assert!(!changed.authenticates_compiler_origin());
        assert!(!changed.grants_compiler_authority());
        assert!(!changed.grants_worker_authority());
        assert!(!changed.grants_link_authority());
        assert!(!changed.grants_load_authority());
        assert!(!changed.grants_launch_authority());
    }

    #[test]
    fn constructor_rejects_empty_oversized_and_mismatched_data() {
        assert_eq!(
            CompilerModuleHandoffV1::new(
                CompilerModuleKindV1::LlvmTextIr,
                target(),
                CodeObjectVersion::V5,
                envelope(),
                b"",
            ),
            Err(CompilerModuleHandoffErrorV1::EmptyModule)
        );
        assert_eq!(
            CompilerModuleHandoffV1::new(
                CompilerModuleKindV1::LlvmTextIr,
                DeviceTargetV1::parse("gfx950:xnack-").unwrap(),
                CodeObjectVersion::V5,
                envelope(),
                LLVM_IR,
            ),
            Err(CompilerModuleHandoffErrorV1::TargetMismatch)
        );
        assert_eq!(
            CompilerModuleHandoffV1::new(
                CompilerModuleKindV1::LlvmTextIr,
                target(),
                CodeObjectVersion::V6,
                envelope(),
                LLVM_IR,
            ),
            Err(CompilerModuleHandoffErrorV1::CodeObjectVersionMismatch)
        );
        let oversized = vec![0; MAX_COMPILER_MODULE_BYTES_V1 + 1];
        assert_eq!(
            CompilerModuleHandoffV1::new(
                CompilerModuleKindV1::LlvmBitcode,
                target(),
                CodeObjectVersion::V5,
                envelope(),
                &oversized,
            ),
            Err(CompilerModuleHandoffErrorV1::ModuleByteBoundExceeded)
        );
    }

    #[test]
    fn every_truncation_and_trailing_byte_is_rejected() {
        let encoded = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR)
            .canonical_bytes()
            .to_vec();
        for length in 0..encoded.len() {
            assert!(
                CompilerModuleHandoffV1::decode(&encoded[..length]).is_err(),
                "accepted prefix of length {length}"
            );
        }
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            CompilerModuleHandoffV1::decode(&trailing),
            Err(CompilerModuleHandoffErrorV1::TrailingBytes)
        );
    }

    #[test]
    fn every_single_bit_mutation_is_rejected() {
        let encoded = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR)
            .canonical_bytes()
            .to_vec();
        for index in 0..encoded.len() {
            for bit in 0..8 {
                let mut mutated = encoded.clone();
                mutated[index] ^= 1 << bit;
                assert!(
                    CompilerModuleHandoffV1::decode(&mutated).is_err(),
                    "accepted bit {bit} mutation at byte {index}"
                );
            }
        }
    }

    #[test]
    fn declared_bounds_fail_before_payload_or_contract_allocation() {
        let original = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let mut encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);

        encoded[location.module_len..location.module_len + 8]
            .copy_from_slice(&((MAX_COMPILER_MODULE_BYTES_V1 as u64) + 1).to_le_bytes());
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::ModuleByteBoundExceeded)
        );

        encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        encoded[location.envelope_len..location.envelope_len + 4]
            .copy_from_slice(&((MAX_COMPILER_FFI_ENVELOPE_BYTES_V1 as u32) + 1).to_le_bytes());
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::EnvelopeByteBoundExceeded)
        );

        encoded = original.canonical_bytes().to_vec();
        encoded[HANDOFF_DOMAIN_V1.len()..HANDOFF_DOMAIN_V1.len() + 4]
            .copy_from_slice(&((MAX_DEVICE_FFI_TARGET_BYTES_V1 as u32) + 1).to_le_bytes());
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::TextByteBoundExceeded)
        );

        encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        let envelope = &mut encoded[location.envelope_start..location.module_start];
        let (_, count_offset) = envelope_header_offsets(envelope);
        envelope[count_offset..count_offset + 4]
            .copy_from_slice(&((MAX_COMPILER_FFI_CONTRACTS_V1 as u32) + 1).to_le_bytes());
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::Envelope(
                CompilerFfiEnvelopeError::TooManyContracts {
                    count: MAX_COMPILER_FFI_CONTRACTS_V1 + 1,
                }
            ))
        );
    }

    #[test]
    fn malformed_utf8_and_forged_digest_are_rejected() {
        let original = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let mut encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        encoded[location.target_start] = 0xff;
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::InvalidUtf8)
        );

        encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        encoded[location.module_start] = 0xff;
        let digest: [u8; 32] = Sha256::digest(&encoded[location.module_start..]).into();
        encoded[location.digest..location.digest + 32].copy_from_slice(&digest);
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::InvalidUtf8)
        );

        encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        encoded[location.module_start] ^= 1;
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::ModuleIdentityMismatch)
        );
    }

    #[test]
    fn header_target_cov_kind_and_identity_substitution_fail_closed() {
        let original = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let location = offsets(original.canonical_bytes());
        let noncanonical_target = b"gfx942:xnack-:sramecc+";
        let mut noncanonical = Vec::new();
        noncanonical.extend_from_slice(&original.canonical_bytes()[..HANDOFF_DOMAIN_V1.len()]);
        noncanonical.extend_from_slice(&(noncanonical_target.len() as u32).to_le_bytes());
        noncanonical.extend_from_slice(noncanonical_target);
        noncanonical.extend_from_slice(&original.canonical_bytes()[location.code_object_version..]);
        assert_eq!(
            CompilerModuleHandoffV1::decode(&noncanonical),
            Err(CompilerModuleHandoffErrorV1::InvalidTarget)
        );

        let mut encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        encoded[location.target_start..location.target_start + "gfx950:xnack-".len()]
            .copy_from_slice(b"gfx950:xnack-");
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::TargetMismatch)
        );

        encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        encoded[location.code_object_version] = 6;
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::CodeObjectVersionMismatch)
        );

        encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        encoded[location.kind] = 0xff;
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::InvalidModuleKind)
        );

        encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        encoded[location.digest] ^= 1;
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::ModuleIdentityMismatch)
        );
    }

    #[test]
    fn envelope_role_and_contract_order_must_remain_canonical() {
        let original = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let mut encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        let envelope = &mut encoded[location.envelope_start..location.module_start];
        let ranges = envelope_contract_ranges(envelope);
        envelope[ranges[0].start + 33] =
            CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition as u8;
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::Envelope(
                CompilerFfiEnvelopeError::DirectionRoleMismatch
            ))
        );

        encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        let envelope = &mut encoded[location.envelope_start..location.module_start];
        let ranges = envelope_contract_ranges(envelope);
        let mut reversed = Vec::with_capacity(ranges[0].len() + ranges[1].len());
        reversed.extend_from_slice(&envelope[ranges[1].clone()]);
        reversed.extend_from_slice(&envelope[ranges[0].clone()]);
        envelope[ranges[0].start..ranges[1].end].copy_from_slice(&reversed);
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::Envelope(
                CompilerFfiEnvelopeError::NonCanonicalContractOrder
            ))
        );
    }

    #[test]
    fn envelope_identity_utf8_and_target_mutations_are_rejected() {
        let original = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let mut encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        let envelope = &mut encoded[location.envelope_start..location.module_start];
        let first_contract = envelope_contract_ranges(envelope)[0].clone();
        envelope[first_contract.start + 34] ^= 1;
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::NonCanonicalEncoding)
        );

        encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        let envelope = &mut encoded[location.envelope_start..location.module_start];
        let first_contract = envelope_contract_ranges(envelope)[0].clone();
        let crate_label_start = first_contract.start + 32 + 1 + 1 + 32 + 4;
        envelope[crate_label_start] = 0xff;
        assert_eq!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::InvalidUtf8)
        );

        encoded = original.canonical_bytes().to_vec();
        let location = offsets(&encoded);
        let envelope = &mut encoded[location.envelope_start..location.module_start];
        let first_contract = envelope_contract_ranges(envelope)[0].clone();
        let mut position = first_contract.start + 32 + 1 + 1 + 32;
        position += 4 + read_u32(envelope, position);
        position += 4 + read_u32(envelope, position);
        position += 16;
        position += 4 + read_u32(envelope, position);
        position += 4 + read_u32(envelope, position);
        let contract_target_start = position + 4;
        envelope[contract_target_start..contract_target_start + "gfx950:xnack-".len()]
            .copy_from_slice(b"gfx950:xnack-");
        assert!(matches!(
            CompilerModuleHandoffV1::decode(&encoded),
            Err(CompilerModuleHandoffErrorV1::Envelope(
                CompilerFfiEnvelopeError::ContractIdentityMismatch { .. }
            ))
        ));
    }

    #[test]
    fn debug_output_does_not_expose_module_or_contract_text() {
        let value = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let debug = format!("{value:?}");
        for secret in ["ModuleID", "external_add", "rust_helper", "ffi_crate"] {
            assert!(
                !debug.contains(secret),
                "debug output leaked `{secret}`: {debug}"
            );
        }
    }
}
