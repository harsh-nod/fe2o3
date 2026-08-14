use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use super::{
    CodeObjectVersion, CompilerFfiEnvelopeV1, CompilerModuleHandoffErrorV1,
    CompilerModuleIdentityV1, CompilerModuleKindV1, CompilerModulePayloadV1,
    CompilerModuleSymbolManifestErrorV1, CompilerModuleSymbolManifestIdentityV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1, DeviceTargetV1,
    MAX_COMPILER_FFI_ENVELOPE_BYTES_V1, MAX_COMPILER_MODULE_BYTES_V1,
    MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1, MAX_DEVICE_FFI_TARGET_BYTES_V1,
    code_object_version_tag,
    module_handoff::{
        Cursor, decode_code_object_version, decode_envelope, decode_module_kind,
        validate_module_bytes,
    },
};

const HANDOFF_DOMAIN_V2: &[u8] = b"FE2O3/COMPILER-MODULE-HANDOFF/V2\0";
const CONTENT_IDENTITY_BYTES: usize = 32 + 8;
const HANDOFF_FIXED_BYTES_V2: usize =
    HANDOFF_DOMAIN_V2.len() + 4 + 1 + 1 + CONTENT_IDENTITY_BYTES + 4 + CONTENT_IDENTITY_BYTES;

/// Maximum canonical bytes in one V2 module handoff, including its symbol manifest.
pub const MAX_COMPILER_MODULE_HANDOFF_BYTES_V2: usize = HANDOFF_FIXED_BYTES_V2
    + MAX_DEVICE_FFI_TARGET_BYTES_V1
    + MAX_COMPILER_FFI_ENVELOPE_BYTES_V1
    + MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1
    + MAX_COMPILER_MODULE_BYTES_V1;

/// SHA-256 and byte length of one exact canonical V2 module handoff.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerModuleHandoffIdentityV2 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl CompilerModuleHandoffIdentityV2 {
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

    fn calculate(bytes: &[u8]) -> Self {
        Self {
            sha256: Sha256::digest(bytes).into(),
            byte_len: bytes.len() as u64,
        }
    }
}

/// Bounded canonical data joining exact LLVM module bytes, FFI contracts, and compiler symbol
/// roles.
///
/// V2 is a separate wire domain and leaves [`super::CompilerModuleHandoffV1`] byte-compatible.
/// Its identity commits to the complete encoding, including the exact manifest identity and bytes.
/// Public construction remains structural and grants no compiler, worker, link, load, or launch
/// authority.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerModuleHandoffV2 {
    kind: CompilerModuleKindV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    module_identity: CompilerModuleIdentityV1,
    envelope: CompilerFfiEnvelopeV1,
    symbol_manifest: CompilerModuleSymbolManifestV1,
    identity: CompilerModuleHandoffIdentityV2,
    canonical_bytes: Vec<u8>,
    module_offset: usize,
}

impl fmt::Debug for CompilerModuleHandoffV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerModuleHandoffV2")
            .field("kind", &self.kind)
            .field("target", &self.target)
            .field("code_object_version", &self.code_object_version)
            .field("module_identity", &self.module_identity)
            .field("envelope_identity", &self.envelope.identity())
            .field("symbol_manifest_identity", &self.symbol_manifest.identity())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Owned components retained from one coherent V2 handoff.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerModuleHandoffPartsV2 {
    envelope: CompilerFfiEnvelopeV1,
    symbol_manifest: CompilerModuleSymbolManifestV1,
    module: CompilerModulePayloadV1,
}

impl fmt::Debug for CompilerModuleHandoffPartsV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerModuleHandoffPartsV2")
            .field("target", &self.target())
            .field("code_object_version", &self.code_object_version())
            .field("envelope_identity", &self.envelope.identity())
            .field("symbol_manifest_identity", &self.symbol_manifest.identity())
            .field("module", &self.module)
            .finish_non_exhaustive()
    }
}

impl CompilerModuleHandoffPartsV2 {
    pub const fn target(&self) -> DeviceTargetV1 {
        self.envelope.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.envelope.code_object_version()
    }

    pub const fn envelope(&self) -> &CompilerFfiEnvelopeV1 {
        &self.envelope
    }

    pub const fn symbol_manifest(&self) -> &CompilerModuleSymbolManifestV1 {
        &self.symbol_manifest
    }

    pub const fn module(&self) -> &CompilerModulePayloadV1 {
        &self.module
    }

    pub fn into_envelope_manifest_and_module(
        self,
    ) -> (
        CompilerFfiEnvelopeV1,
        CompilerModuleSymbolManifestV1,
        CompilerModulePayloadV1,
    ) {
        (self.envelope, self.symbol_manifest, self.module)
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

impl CompilerModuleHandoffV2 {
    pub fn new(
        kind: CompilerModuleKindV1,
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        envelope: CompilerFfiEnvelopeV1,
        symbol_manifest: CompilerModuleSymbolManifestV1,
        module_bytes: &[u8],
    ) -> Result<Self, CompilerModuleHandoffErrorV2> {
        validate_module_bytes(kind, module_bytes).map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        if envelope.target() != target {
            return Err(CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::TargetMismatch,
            ));
        }
        if envelope.code_object_version() != code_object_version {
            return Err(CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::CodeObjectVersionMismatch,
            ));
        }
        validate_envelope_manifest(&envelope, &symbol_manifest)?;

        let target_text = target.to_string();
        if target_text.is_empty() || target_text.len() > MAX_DEVICE_FFI_TARGET_BYTES_V1 {
            return Err(CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::InvalidTarget,
            ));
        }
        let envelope_bytes = envelope.canonical_bytes();
        if envelope_bytes.len() > MAX_COMPILER_FFI_ENVELOPE_BYTES_V1 {
            return Err(CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::EnvelopeByteBoundExceeded,
            ));
        }
        let manifest_bytes = symbol_manifest.canonical_bytes();
        if manifest_bytes.len() > MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1 {
            return Err(CompilerModuleHandoffErrorV2::Manifest(
                CompilerModuleSymbolManifestErrorV1::ManifestByteBoundExceeded,
            ));
        }
        let exact_size = HANDOFF_FIXED_BYTES_V2
            .checked_add(target_text.len())
            .and_then(|size| size.checked_add(envelope_bytes.len()))
            .and_then(|size| size.checked_add(manifest_bytes.len()))
            .and_then(|size| size.checked_add(module_bytes.len()))
            .ok_or(CompilerModuleHandoffErrorV2::HandoffByteBoundExceeded)?;
        if exact_size > MAX_COMPILER_MODULE_HANDOFF_BYTES_V2 {
            return Err(CompilerModuleHandoffErrorV2::HandoffByteBoundExceeded);
        }

        let module_identity = CompilerModuleIdentityV1::calculate(module_bytes);
        let manifest_identity = symbol_manifest.identity();
        let mut canonical_bytes = Vec::with_capacity(exact_size);
        canonical_bytes.extend_from_slice(HANDOFF_DOMAIN_V2);
        push_u32(&mut canonical_bytes, target_text.len())?;
        canonical_bytes.extend_from_slice(target_text.as_bytes());
        canonical_bytes.push(code_object_version_tag(code_object_version) as u8);
        canonical_bytes.push(kind as u8);
        canonical_bytes.extend_from_slice(module_identity.sha256());
        canonical_bytes.extend_from_slice(&module_identity.byte_len().to_le_bytes());
        push_u32(&mut canonical_bytes, envelope_bytes.len())?;
        canonical_bytes.extend_from_slice(manifest_identity.sha256());
        canonical_bytes.extend_from_slice(&manifest_identity.byte_len().to_le_bytes());
        canonical_bytes.extend_from_slice(envelope_bytes);
        canonical_bytes.extend_from_slice(manifest_bytes);
        let module_offset = canonical_bytes.len();
        canonical_bytes.extend_from_slice(module_bytes);
        debug_assert_eq!(canonical_bytes.len(), exact_size);
        let identity = CompilerModuleHandoffIdentityV2::calculate(&canonical_bytes);

        Ok(Self {
            kind,
            target,
            code_object_version,
            module_identity,
            envelope,
            symbol_manifest,
            identity,
            canonical_bytes,
            module_offset,
        })
    }

    /// Strictly decodes one complete canonical V2 handoff.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerModuleHandoffErrorV2> {
        if bytes.len() > MAX_COMPILER_MODULE_HANDOFF_BYTES_V2 {
            return Err(CompilerModuleHandoffErrorV2::HandoffByteBoundExceeded);
        }
        let mut cursor = Cursor::new(bytes);
        if cursor
            .take(HANDOFF_DOMAIN_V2.len())
            .map_err(CompilerModuleHandoffErrorV2::Handoff)?
            != HANDOFF_DOMAIN_V2
        {
            return Err(CompilerModuleHandoffErrorV2::InvalidMagic);
        }
        let target_text = cursor
            .text(MAX_DEVICE_FFI_TARGET_BYTES_V1)
            .map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        let target = DeviceTargetV1::parse(target_text).map_err(|_| {
            CompilerModuleHandoffErrorV2::Handoff(CompilerModuleHandoffErrorV1::InvalidTarget)
        })?;
        let code_object_version = decode_code_object_version(
            cursor
                .byte()
                .map_err(CompilerModuleHandoffErrorV2::Handoff)?,
        )
        .map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        let kind = decode_module_kind(
            cursor
                .byte()
                .map_err(CompilerModuleHandoffErrorV2::Handoff)?,
        )
        .map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        let declared_module_digest = cursor
            .fixed::<32>()
            .map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        let declared_module_len_u64 = cursor
            .u64()
            .map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        let declared_module_len = usize::try_from(declared_module_len_u64).map_err(|_| {
            CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::ModuleByteBoundExceeded,
            )
        })?;
        if declared_module_len == 0 || declared_module_len > MAX_COMPILER_MODULE_BYTES_V1 {
            return Err(CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::ModuleByteBoundExceeded,
            ));
        }
        let envelope_len = cursor
            .u32_as_usize()
            .map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        if envelope_len == 0 || envelope_len > MAX_COMPILER_FFI_ENVELOPE_BYTES_V1 {
            return Err(CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::EnvelopeByteBoundExceeded,
            ));
        }
        let declared_manifest_digest = cursor
            .fixed::<32>()
            .map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        let declared_manifest_len_u64 = cursor
            .u64()
            .map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        let declared_manifest_len = usize::try_from(declared_manifest_len_u64)
            .map_err(|_| CompilerModuleHandoffErrorV2::ManifestByteBoundExceeded)?;
        if declared_manifest_len == 0
            || declared_manifest_len > MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1
        {
            return Err(CompilerModuleHandoffErrorV2::ManifestByteBoundExceeded);
        }
        let envelope_bytes = cursor
            .take(envelope_len)
            .map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        let manifest_bytes = cursor
            .take(declared_manifest_len)
            .map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        let module_bytes = cursor
            .take(declared_module_len)
            .map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        cursor
            .finish()
            .map_err(CompilerModuleHandoffErrorV2::Handoff)?;

        let actual_module_digest: [u8; 32] = Sha256::digest(module_bytes).into();
        if actual_module_digest != declared_module_digest {
            return Err(CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::ModuleIdentityMismatch,
            ));
        }
        let declared_manifest_identity = CompilerModuleSymbolManifestIdentityV1::from_parts(
            declared_manifest_digest,
            declared_manifest_len_u64,
        );
        if !declared_manifest_identity.matches(manifest_bytes) {
            return Err(CompilerModuleHandoffErrorV2::ManifestIdentityMismatch);
        }
        validate_module_bytes(kind, module_bytes).map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        let envelope =
            decode_envelope(envelope_bytes).map_err(CompilerModuleHandoffErrorV2::Handoff)?;
        let symbol_manifest = CompilerModuleSymbolManifestV1::decode(manifest_bytes)
            .map_err(CompilerModuleHandoffErrorV2::Manifest)?;

        let decoded = Self::new(
            kind,
            target,
            code_object_version,
            envelope,
            symbol_manifest,
            module_bytes,
        )?;
        if decoded.module_identity.byte_len() != declared_module_len_u64
            || decoded.module_identity.sha256() != &declared_module_digest
        {
            return Err(CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::ModuleIdentityMismatch,
            ));
        }
        if decoded.symbol_manifest.identity() != declared_manifest_identity {
            return Err(CompilerModuleHandoffErrorV2::ManifestIdentityMismatch);
        }
        if decoded.canonical_bytes() != bytes {
            return Err(CompilerModuleHandoffErrorV2::NonCanonicalEncoding);
        }
        Ok(decoded)
    }

    pub const fn identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.identity
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

    pub const fn symbol_manifest(&self) -> &CompilerModuleSymbolManifestV1 {
        &self.symbol_manifest
    }

    pub fn module_bytes(&self) -> &[u8] {
        &self.canonical_bytes[self.module_offset..]
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn into_parts(self) -> CompilerModuleHandoffPartsV2 {
        let Self {
            kind,
            module_identity,
            envelope,
            symbol_manifest,
            mut canonical_bytes,
            module_offset,
            ..
        } = self;
        canonical_bytes.drain(..module_offset);
        debug_assert!(module_identity.matches(&canonical_bytes));
        CompilerModuleHandoffPartsV2 {
            envelope,
            symbol_manifest,
            module: CompilerModulePayloadV1::from_validated(kind, module_identity, canonical_bytes),
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

impl<'a> TryFrom<&'a [u8]> for CompilerModuleHandoffV2 {
    type Error = CompilerModuleHandoffErrorV2;

    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        Self::decode(bytes)
    }
}

/// Failure to construct or strictly decode a V2 module handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerModuleHandoffErrorV2 {
    HandoffByteBoundExceeded,
    ManifestByteBoundExceeded,
    InvalidMagic,
    ManifestIdentityMismatch,
    FfiImportRoleMismatch,
    FfiExportRoleMismatch,
    NonCanonicalEncoding,
    Handoff(CompilerModuleHandoffErrorV1),
    Manifest(CompilerModuleSymbolManifestErrorV1),
}

impl fmt::Display for CompilerModuleHandoffErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HandoffByteBoundExceeded => {
                formatter.write_str("V2 compiler module handoff byte bound exceeded")
            }
            Self::ManifestByteBoundExceeded => {
                formatter.write_str("V2 compiler module manifest byte bound exceeded")
            }
            Self::InvalidMagic => formatter.write_str("invalid V2 compiler module handoff magic"),
            Self::ManifestIdentityMismatch => {
                formatter.write_str("compiler module symbol manifest identity mismatch")
            }
            Self::FfiImportRoleMismatch => formatter
                .write_str("compiler FFI import is not an unresolved external module symbol"),
            Self::FfiExportRoleMismatch => {
                formatter.write_str("compiler FFI exports disagree with module symbol roles")
            }
            Self::NonCanonicalEncoding => {
                formatter.write_str("noncanonical V2 compiler module handoff encoding")
            }
            Self::Handoff(error) => write!(formatter, "invalid compiler module data: {error}"),
            Self::Manifest(error) => write!(formatter, "invalid compiler symbol manifest: {error}"),
        }
    }
}

impl Error for CompilerModuleHandoffErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(error) => Some(error),
            Self::Manifest(error) => Some(error),
            _ => None,
        }
    }
}

fn validate_envelope_manifest(
    envelope: &CompilerFfiEnvelopeV1,
    manifest: &CompilerModuleSymbolManifestV1,
) -> Result<(), CompilerModuleHandoffErrorV2> {
    let directional = envelope.directional_symbols();
    let imports = manifest
        .symbols(CompilerModuleSymbolRoleV1::UnresolvedExternalImport)
        .collect::<Vec<_>>();
    if directional.import_count() != imports.len()
        || directional
            .imports()
            .any(|symbol| imports.binary_search(&symbol).is_err())
    {
        return Err(CompilerModuleHandoffErrorV2::FfiImportRoleMismatch);
    }

    let exports = manifest
        .symbols(CompilerModuleSymbolRoleV1::DeviceFfiExport)
        .collect::<Vec<_>>();
    if directional.export_count() != exports.len()
        || directional
            .exports()
            .any(|symbol| exports.binary_search(&symbol).is_err())
    {
        return Err(CompilerModuleHandoffErrorV2::FfiExportRoleMismatch);
    }
    Ok(())
}

fn push_u32(bytes: &mut Vec<u8>, value: usize) -> Result<(), CompilerModuleHandoffErrorV2> {
    let value =
        u32::try_from(value).map_err(|_| CompilerModuleHandoffErrorV2::HandoffByteBoundExceeded)?;
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CompilerFfiContractV1, CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1,
        CompilerFfiSourceOwnerV1, DeviceFfiDirectionV1,
    };
    use reserved_fe2o3_symbols::{
        DEVICE_FFI_DIRECTION_EXPORT_V1, DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1,
        derive_device_ffi_contract_id_v1,
    };

    const IMPORT_ABI: &str =
        "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]";
    const EXPORT_ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    const LLVM_IR: &[u8] =
        b"; ModuleID = 'handoff-v2'\ndefine amdgpu_kernel void @kernel() { ret void }\n";

    fn target() -> DeviceTargetV1 {
        DeviceTargetV1::parse("gfx942:xnack-").unwrap()
    }

    fn contract(
        direction: DeviceFfiDirectionV1,
        symbol: &str,
        abi: &str,
        effects: &str,
        byte: u8,
    ) -> CompilerFfiContractV1 {
        let semantic_identity = [byte; 32];
        let semantic_text = semantic_identity
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
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
            CompilerFfiSourceOwnerV1::new(
                "ffi_crate",
                &format!("ffi_crate::{symbol}"),
                [byte; 16],
                &format!("_RINvNtCs1234_ffi_crate{symbol}"),
            )
            .unwrap(),
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
            ))
            .unwrap();
        builder
            .push(contract(
                DeviceFfiDirectionV1::Export,
                "rust_helper",
                EXPORT_ABI,
                "none",
                0x22,
            ))
            .unwrap();
        builder.finish().unwrap()
    }

    fn manifest(helper: &str) -> CompilerModuleSymbolManifestV1 {
        use CompilerModuleSymbolRoleV1 as Role;
        CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, "kernel"),
            (Role::KernelDescriptor, "kernel.kd"),
            (Role::DeviceFfiExport, "rust_helper"),
            (Role::InternalHelper, helper),
            (Role::UnresolvedExternalImport, "external_add"),
        ])
        .unwrap()
    }

    fn handoff(kind: CompilerModuleKindV1, module: &[u8]) -> CompilerModuleHandoffV2 {
        CompilerModuleHandoffV2::new(
            kind,
            target(),
            CodeObjectVersion::V5,
            envelope(),
            manifest("internal_helper"),
            module,
        )
        .unwrap()
    }

    #[test]
    fn ffi_free_compiler_module_handoff_round_trips_canonically() {
        use CompilerModuleSymbolRoleV1 as Role;

        let envelope =
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target(), CodeObjectVersion::V5)
                .unwrap();
        let manifest = CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, "kernel"),
            (Role::KernelDescriptor, "kernel.kd"),
        ])
        .unwrap();
        let handoff = CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmTextIr,
            target(),
            CodeObjectVersion::V5,
            envelope,
            manifest,
            LLVM_IR,
        )
        .unwrap();

        let decoded = CompilerModuleHandoffV2::decode(handoff.canonical_bytes()).unwrap();
        assert_eq!(decoded, handoff);
        assert_eq!(decoded.envelope().inspection().import_count(), 0);
        assert_eq!(decoded.envelope().inspection().export_count(), 0);
        assert!(!decoded.envelope().authenticates_compiler_origin());
        assert!(!decoded.envelope().grants_link_authority());
    }

    #[derive(Clone, Copy)]
    struct Offsets {
        module_digest: usize,
        module_len: usize,
        envelope_len: usize,
        manifest_digest: usize,
        manifest_len: usize,
        module_start: usize,
    }

    fn read_u32(bytes: &[u8], offset: usize) -> usize {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
    }

    fn read_u64(bytes: &[u8], offset: usize) -> usize {
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize
    }

    fn offsets(bytes: &[u8]) -> Offsets {
        let target_len = read_u32(bytes, HANDOFF_DOMAIN_V2.len());
        let cov = HANDOFF_DOMAIN_V2.len() + 4 + target_len;
        let module_digest = cov + 2;
        let module_len = module_digest + 32;
        let envelope_len = module_len + 8;
        let manifest_digest = envelope_len + 4;
        let manifest_len = manifest_digest + 32;
        let envelope_start = manifest_len + 8;
        let manifest_start = envelope_start + read_u32(bytes, envelope_len);
        let module_start = manifest_start + read_u64(bytes, manifest_len);
        Offsets {
            module_digest,
            module_len,
            envelope_len,
            manifest_digest,
            manifest_len,
            module_start,
        }
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
            let second = CompilerModuleHandoffV2::decode(first.canonical_bytes()).unwrap();
            let via_try_from = CompilerModuleHandoffV2::try_from(first.canonical_bytes()).unwrap();
            assert_eq!(second, first);
            assert_eq!(via_try_from, first);
            assert_eq!(second.kind(), kind);
            assert_eq!(second.target(), target());
            assert_eq!(second.code_object_version(), CodeObjectVersion::V5);
            assert_eq!(second.module_bytes(), module);
            assert!(second.module_identity().matches(module));
            assert_eq!(second.symbol_manifest(), first.symbol_manifest());
            assert!(second.identity().matches(second.canonical_bytes()));
            assert!(!second.authenticates_compiler_origin());
            assert!(!second.grants_compiler_authority());
            assert!(!second.grants_worker_authority());
            assert!(!second.grants_link_authority());
            assert!(!second.grants_load_authority());
            assert!(!second.grants_launch_authority());
        }
    }

    #[test]
    fn handoff_identity_binds_exact_manifest_identity_and_bytes() {
        let first = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let second = CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmTextIr,
            target(),
            CodeObjectVersion::V5,
            envelope(),
            manifest("internal_helper_2"),
            LLVM_IR,
        )
        .unwrap();

        assert_eq!(first.module_identity(), second.module_identity());
        assert_ne!(
            first.symbol_manifest().identity(),
            second.symbol_manifest().identity()
        );
        assert_ne!(first.identity(), second.identity());
        assert_ne!(first.canonical_bytes(), second.canonical_bytes());
    }

    #[test]
    fn ffi_envelope_and_manifest_roles_must_agree() {
        use CompilerModuleSymbolRoleV1 as Role;
        let missing_import = CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, "kernel"),
            (Role::KernelDescriptor, "kernel.kd"),
            (Role::DeviceFfiExport, "rust_helper"),
        ])
        .unwrap();
        assert_eq!(
            CompilerModuleHandoffV2::new(
                CompilerModuleKindV1::LlvmTextIr,
                target(),
                CodeObjectVersion::V5,
                envelope(),
                missing_import,
                LLVM_IR,
            ),
            Err(CompilerModuleHandoffErrorV2::FfiImportRoleMismatch)
        );

        let extra_import = CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, "kernel"),
            (Role::KernelDescriptor, "kernel.kd"),
            (Role::DeviceFfiExport, "rust_helper"),
            (Role::UnresolvedExternalImport, "external_add"),
            (Role::UnresolvedExternalImport, "uncontracted_import"),
        ])
        .unwrap();
        assert_eq!(
            CompilerModuleHandoffV2::new(
                CompilerModuleKindV1::LlvmTextIr,
                target(),
                CodeObjectVersion::V5,
                envelope(),
                extra_import,
                LLVM_IR,
            ),
            Err(CompilerModuleHandoffErrorV2::FfiImportRoleMismatch)
        );

        let missing_export = CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, "kernel"),
            (Role::KernelDescriptor, "kernel.kd"),
            (Role::UnresolvedExternalImport, "external_add"),
        ])
        .unwrap();
        assert_eq!(
            CompilerModuleHandoffV2::new(
                CompilerModuleKindV1::LlvmTextIr,
                target(),
                CodeObjectVersion::V5,
                envelope(),
                missing_export,
                LLVM_IR,
            ),
            Err(CompilerModuleHandoffErrorV2::FfiExportRoleMismatch)
        );

        let extra_export = CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, "kernel"),
            (Role::KernelDescriptor, "kernel.kd"),
            (Role::DeviceFfiExport, "extra_export"),
            (Role::DeviceFfiExport, "rust_helper"),
            (Role::UnresolvedExternalImport, "external_add"),
        ])
        .unwrap();
        assert_eq!(
            CompilerModuleHandoffV2::new(
                CompilerModuleKindV1::LlvmTextIr,
                target(),
                CodeObjectVersion::V5,
                envelope(),
                extra_export,
                LLVM_IR,
            ),
            Err(CompilerModuleHandoffErrorV2::FfiExportRoleMismatch)
        );
    }

    #[test]
    fn every_truncation_and_trailing_byte_is_rejected() {
        let encoded = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR)
            .canonical_bytes()
            .to_vec();
        for length in 0..encoded.len() {
            assert!(
                CompilerModuleHandoffV2::decode(&encoded[..length]).is_err(),
                "accepted prefix of length {length}"
            );
        }
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            CompilerModuleHandoffV2::decode(&trailing),
            Err(CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::TrailingBytes
            ))
        );
    }

    #[test]
    fn forged_manifest_or_module_identity_is_rejected() {
        let original = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let location = offsets(original.canonical_bytes());
        let mut encoded = original.canonical_bytes().to_vec();
        encoded[location.manifest_digest] ^= 1;
        assert_eq!(
            CompilerModuleHandoffV2::decode(&encoded),
            Err(CompilerModuleHandoffErrorV2::ManifestIdentityMismatch)
        );

        encoded = original.canonical_bytes().to_vec();
        encoded[location.module_digest] ^= 1;
        assert_eq!(
            CompilerModuleHandoffV2::decode(&encoded),
            Err(CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::ModuleIdentityMismatch
            ))
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
                    CompilerModuleHandoffV2::decode(&mutated).is_err(),
                    "accepted bit {bit} mutation at byte {index}"
                );
            }
        }
    }

    #[test]
    fn declared_bounds_fail_before_nested_payload_decoding() {
        let original = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let location = offsets(original.canonical_bytes());
        let mut encoded = original.canonical_bytes().to_vec();
        encoded[location.module_len..location.module_len + 8]
            .copy_from_slice(&((MAX_COMPILER_MODULE_BYTES_V1 as u64) + 1).to_le_bytes());
        assert_eq!(
            CompilerModuleHandoffV2::decode(&encoded),
            Err(CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::ModuleByteBoundExceeded
            ))
        );

        encoded = original.canonical_bytes().to_vec();
        encoded[location.envelope_len..location.envelope_len + 4]
            .copy_from_slice(&((MAX_COMPILER_FFI_ENVELOPE_BYTES_V1 as u32) + 1).to_le_bytes());
        assert_eq!(
            CompilerModuleHandoffV2::decode(&encoded),
            Err(CompilerModuleHandoffErrorV2::Handoff(
                CompilerModuleHandoffErrorV1::EnvelopeByteBoundExceeded
            ))
        );

        encoded = original.canonical_bytes().to_vec();
        encoded[location.manifest_len..location.manifest_len + 8].copy_from_slice(
            &((MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1 as u64) + 1).to_le_bytes(),
        );
        assert_eq!(
            CompilerModuleHandoffV2::decode(&encoded),
            Err(CompilerModuleHandoffErrorV2::ManifestByteBoundExceeded)
        );
    }

    #[test]
    fn owned_parts_preserve_manifest_and_reuse_module_allocation() {
        let handoff = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let expected_manifest = handoff.symbol_manifest().clone();
        let canonical_allocation = handoff.canonical_bytes.as_ptr();
        let location = offsets(handoff.canonical_bytes());
        assert_eq!(
            handoff.module_bytes().as_ptr(),
            handoff.canonical_bytes()[location.module_start..].as_ptr()
        );

        let parts = handoff.into_parts();
        assert_eq!(parts.symbol_manifest(), &expected_manifest);
        assert_eq!(parts.module().bytes(), LLVM_IR);
        assert_eq!(parts.module.bytes().as_ptr(), canonical_allocation);
        assert!(!parts.authenticates_compiler_origin());
        assert!(!parts.grants_link_authority());
        let (_, manifest, module) = parts.into_envelope_manifest_and_module();
        assert_eq!(manifest, expected_manifest);
        assert_eq!(module.into_bytes(), LLVM_IR);
    }

    #[test]
    fn stable_handoff_identity_golden() {
        let value = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        assert_eq!(
            value.identity().sha256(),
            &[
                0x8d, 0xcd, 0x4f, 0x0a, 0x31, 0xa0, 0x78, 0x08, 0x13, 0x87, 0xde, 0x29, 0xed, 0xac,
                0xe3, 0x7b, 0xcd, 0xc5, 0xdf, 0xf1, 0xa6, 0x6c, 0x67, 0xbb, 0x67, 0x8f, 0xc0, 0xe2,
                0x18, 0x58, 0xd0, 0xe5,
            ]
        );
    }

    #[test]
    fn debug_output_does_not_expose_module_contract_or_symbol_text() {
        let value = handoff(CompilerModuleKindV1::LlvmTextIr, LLVM_IR);
        let debug = format!("{value:?}");
        for secret in [
            "handoff-v2",
            "kernel",
            "internal_helper",
            "external_add",
            "rust_helper",
        ] {
            assert!(
                !debug.contains(secret),
                "debug output leaked `{secret}`: {debug}"
            );
        }
    }
}
