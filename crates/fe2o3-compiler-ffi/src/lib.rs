#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::{error::Error, fmt};

pub use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1,
    DeviceFfiContractIdV1, DeviceFfiDirectionV1, DeviceFfiGrammarError,
    MAX_DEVICE_FFI_EFFECT_BYTES_V1, MAX_DEVICE_FFI_PHYSICAL_ABI_BYTES_V1,
    MAX_DEVICE_FFI_SYMBOL_BYTES_V1, MAX_DEVICE_FFI_TARGET_BYTES_V1,
    derive_device_ffi_contract_id_v1, validate_device_ffi_contract_grammar_v1,
};
use sha2::{Digest, Sha256};

/// Maximum complete import/export contracts in one compiler observation.
pub const MAX_COMPILER_FFI_CONTRACTS_V1: usize = 128;
/// Maximum bytes in a source crate label.
pub const MAX_COMPILER_FFI_CRATE_LABEL_BYTES_V1: usize = 128;
/// Maximum bytes in a source item path.
pub const MAX_COMPILER_FFI_ITEM_PATH_BYTES_V1: usize = 1_024;
/// Maximum bytes in a concrete rustc instance symbol.
pub const MAX_COMPILER_FFI_INSTANCE_SYMBOL_BYTES_V1: usize = 512;
/// Maximum aggregate bytes in all variable-length envelope fields.
pub const MAX_COMPILER_FFI_AGGREGATE_TEXT_BYTES_V1: usize = 384 * 1024;
/// Maximum exact canonical envelope size.
pub const MAX_COMPILER_FFI_ENVELOPE_BYTES_V1: usize = 512 * 1024;

const SOURCE_OWNER_DOMAIN_V1: &[u8] = b"FE2O3/COMPILER-FFI-SOURCE-OWNER/V1\0";
const EFFECT_ABI_DOMAIN_V1: &[u8] = b"FE2O3/COMPILER-FFI-EFFECT-ABI/V1\0";
const ENVELOPE_DOMAIN_V1: &[u8] = b"FE2O3/COMPILER-FFI-ENVELOPE/V1\0";

/// Identity of one exact source-owner record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerFfiSourceOwnerIdentityV1([u8; 32]);

impl CompilerFfiSourceOwnerIdentityV1 {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact rustc source ownership copied into a neutral compiler observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerFfiSourceOwnerV1 {
    crate_label: String,
    item_path: String,
    def_path_hash: [u8; 16],
    concrete_instance_symbol: String,
    identity: CompilerFfiSourceOwnerIdentityV1,
}

impl CompilerFfiSourceOwnerV1 {
    pub fn new(
        crate_label: &str,
        item_path: &str,
        def_path_hash: [u8; 16],
        concrete_instance_symbol: &str,
    ) -> Result<Self, CompilerFfiEnvelopeError> {
        validate_text(
            crate_label,
            MAX_COMPILER_FFI_CRATE_LABEL_BYTES_V1,
            CompilerFfiTextFieldV1::CrateLabel,
            false,
        )?;
        validate_text(
            item_path,
            MAX_COMPILER_FFI_ITEM_PATH_BYTES_V1,
            CompilerFfiTextFieldV1::ItemPath,
            false,
        )?;
        validate_text(
            concrete_instance_symbol,
            MAX_COMPILER_FFI_INSTANCE_SYMBOL_BYTES_V1,
            CompilerFfiTextFieldV1::ConcreteInstanceSymbol,
            true,
        )?;

        let mut digest = Sha256::new();
        digest.update(SOURCE_OWNER_DOMAIN_V1);
        update_field(&mut digest, crate_label.as_bytes());
        update_field(&mut digest, item_path.as_bytes());
        digest.update(def_path_hash);
        update_field(&mut digest, concrete_instance_symbol.as_bytes());
        let identity = CompilerFfiSourceOwnerIdentityV1(digest.finalize().into());
        Ok(Self {
            crate_label: crate_label.to_owned(),
            item_path: item_path.to_owned(),
            def_path_hash,
            concrete_instance_symbol: concrete_instance_symbol.to_owned(),
            identity,
        })
    }

    pub const fn identity(&self) -> CompilerFfiSourceOwnerIdentityV1 {
        self.identity
    }
}

/// Domain-separated identity of one validated effects/physical-ABI pair.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerFfiEffectAbiIdentityV1([u8; 32]);

impl CompilerFfiEffectAbiIdentityV1 {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Required definition location retained from the compiler's closed graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompilerFfiLinkRoleV1 {
    RequiresExternalDefinition = 1,
    RequiresCompilerModuleDefinition = 2,
}

/// One complete canonical import or export before it is sealed into an envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerFfiContractV1 {
    contract_identity: DeviceFfiContractIdV1,
    direction: DeviceFfiDirectionV1,
    link_role: CompilerFfiLinkRoleV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    source_owner: CompilerFfiSourceOwnerV1,
    symbol: String,
    physical_abi: String,
    effects: String,
    effect_abi_identity: CompilerFfiEffectAbiIdentityV1,
    semantic_identity: [u8; 32],
}

impl CompilerFfiContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contract_identity: DeviceFfiContractIdV1,
        direction: DeviceFfiDirectionV1,
        link_role: CompilerFfiLinkRoleV1,
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        source_owner: CompilerFfiSourceOwnerV1,
        symbol: &str,
        physical_abi: &str,
        effects: &str,
        semantic_identity: [u8; 32],
    ) -> Result<Self, CompilerFfiEnvelopeError> {
        let expected_role = match direction {
            DeviceFfiDirectionV1::Import => CompilerFfiLinkRoleV1::RequiresExternalDefinition,
            DeviceFfiDirectionV1::Export => CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
        };
        if link_role != expected_role {
            return Err(CompilerFfiEnvelopeError::DirectionRoleMismatch);
        }
        validate_text(
            symbol,
            MAX_DEVICE_FFI_SYMBOL_BYTES_V1,
            CompilerFfiTextFieldV1::Symbol,
            true,
        )?;
        validate_text(
            physical_abi,
            MAX_DEVICE_FFI_PHYSICAL_ABI_BYTES_V1,
            CompilerFfiTextFieldV1::PhysicalAbi,
            true,
        )?;
        validate_text(
            effects,
            MAX_DEVICE_FFI_EFFECT_BYTES_V1,
            CompilerFfiTextFieldV1::Effects,
            true,
        )?;
        validate_device_ffi_contract_grammar_v1(symbol, physical_abi, effects)
            .map_err(CompilerFfiEnvelopeError::Grammar)?;

        let target_text = target.to_string();
        validate_text(
            &target_text,
            MAX_DEVICE_FFI_TARGET_BYTES_V1,
            CompilerFfiTextFieldV1::Target,
            true,
        )?;
        let semantic_text = lower_hex(&semantic_identity);
        let derived = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: direction_tag(direction),
            symbol,
            calling_convention: "C",
            code_object_version: code_object_version_tag(code_object_version),
            target: &target_text,
            physical_abi,
            effects,
            semantic_identity: &semantic_text,
        });
        if derived != contract_identity {
            return Err(CompilerFfiEnvelopeError::ContractIdentityMismatch {
                claimed: contract_identity,
                derived,
            });
        }

        let mut effect_abi = Sha256::new();
        effect_abi.update(EFFECT_ABI_DOMAIN_V1);
        update_field(&mut effect_abi, physical_abi.as_bytes());
        update_field(&mut effect_abi, effects.as_bytes());
        let effect_abi_identity = CompilerFfiEffectAbiIdentityV1(effect_abi.finalize().into());
        Ok(Self {
            contract_identity,
            direction,
            link_role,
            target,
            code_object_version,
            source_owner,
            symbol: symbol.to_owned(),
            physical_abi: physical_abi.to_owned(),
            effects: effects.to_owned(),
            effect_abi_identity,
            semantic_identity,
        })
    }

    pub const fn contract_identity(&self) -> DeviceFfiContractIdV1 {
        self.contract_identity
    }

    pub const fn effect_abi_identity(&self) -> CompilerFfiEffectAbiIdentityV1 {
        self.effect_abi_identity
    }

    fn encoded_text_bytes(&self) -> Result<usize, CompilerFfiEnvelopeError> {
        [
            self.source_owner.crate_label.as_str(),
            self.source_owner.item_path.as_str(),
            self.source_owner.concrete_instance_symbol.as_str(),
            self.symbol.as_str(),
            self.target.to_string().as_str(),
            self.physical_abi.as_str(),
            self.effects.as_str(),
        ]
        .into_iter()
        .try_fold(0_usize, checked_text_sum)
    }
}

/// Identity of all canonical bytes in one compiler FFI envelope.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerFfiEnvelopeIdentityV1([u8; 32]);

impl CompilerFfiEnvelopeIdentityV1 {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn to_hex(self) -> String {
        lower_hex(&self.0)
    }
}

/// Non-authoritative summary that cannot be reduced to a symbol closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerFfiEnvelopeInspectionV1 {
    import_count: usize,
    export_count: usize,
    requires_compiler_module_definition_count: usize,
}

impl CompilerFfiEnvelopeInspectionV1 {
    pub const fn import_count(self) -> usize {
        self.import_count
    }

    pub const fn export_count(self) -> usize {
        self.export_count
    }

    pub const fn requires_compiler_module_definition_count(self) -> usize {
        self.requires_compiler_module_definition_count
    }
}

/// Opaque, canonical compiler observation with no executable authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerFfiEnvelopeV1 {
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    contracts: Vec<CompilerFfiContractV1>,
    canonical_bytes: Vec<u8>,
    identity: CompilerFfiEnvelopeIdentityV1,
    inspection: CompilerFfiEnvelopeInspectionV1,
}

impl CompilerFfiEnvelopeV1 {
    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.identity
    }

    pub const fn inspection(&self) -> CompilerFfiEnvelopeInspectionV1 {
        self.inspection
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }
}

/// Count-first builder for one exact canonical envelope.
pub struct CompilerFfiEnvelopeBuilderV1 {
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    expected_contract_count: usize,
    contracts: Vec<CompilerFfiContractV1>,
    aggregate_text_bytes: usize,
}

impl CompilerFfiEnvelopeBuilderV1 {
    pub fn new(
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        contract_count: usize,
    ) -> Result<Self, CompilerFfiEnvelopeError> {
        if contract_count == 0 {
            return Err(CompilerFfiEnvelopeError::EmptyEnvelope);
        }
        if contract_count > MAX_COMPILER_FFI_CONTRACTS_V1 {
            return Err(CompilerFfiEnvelopeError::TooManyContracts {
                count: contract_count,
            });
        }
        let target_text = target.to_string();
        validate_text(
            &target_text,
            MAX_DEVICE_FFI_TARGET_BYTES_V1,
            CompilerFfiTextFieldV1::Target,
            true,
        )?;
        Ok(Self {
            target,
            code_object_version,
            expected_contract_count: contract_count,
            contracts: Vec::with_capacity(contract_count),
            aggregate_text_bytes: target_text.len(),
        })
    }

    pub fn push(
        &mut self,
        contract: CompilerFfiContractV1,
    ) -> Result<(), CompilerFfiEnvelopeError> {
        if self.contracts.len() == self.expected_contract_count {
            return Err(CompilerFfiEnvelopeError::ContractCountMismatch {
                expected: self.expected_contract_count,
                actual: self.contracts.len() + 1,
            });
        }
        if contract.target != self.target {
            return Err(CompilerFfiEnvelopeError::TargetMismatch);
        }
        if contract.code_object_version != self.code_object_version {
            return Err(CompilerFfiEnvelopeError::CodeObjectVersionMismatch);
        }
        if let Some(previous) = self.contracts.last()
            && contract_sort_key(previous) >= contract_sort_key(&contract)
        {
            return Err(CompilerFfiEnvelopeError::NonCanonicalContractOrder);
        }
        self.aggregate_text_bytes = self
            .aggregate_text_bytes
            .checked_add(contract.encoded_text_bytes()?)
            .ok_or(CompilerFfiEnvelopeError::AggregateTextBoundExceeded)?;
        if self.aggregate_text_bytes > MAX_COMPILER_FFI_AGGREGATE_TEXT_BYTES_V1 {
            return Err(CompilerFfiEnvelopeError::AggregateTextBoundExceeded);
        }
        self.contracts.push(contract);
        Ok(())
    }

    pub fn finish(self) -> Result<CompilerFfiEnvelopeV1, CompilerFfiEnvelopeError> {
        if self.contracts.len() != self.expected_contract_count {
            return Err(CompilerFfiEnvelopeError::ContractCountMismatch {
                expected: self.expected_contract_count,
                actual: self.contracts.len(),
            });
        }
        let exact_size = exact_envelope_size(self.target, &self.contracts)?;
        if exact_size > MAX_COMPILER_FFI_ENVELOPE_BYTES_V1 {
            return Err(CompilerFfiEnvelopeError::EnvelopeByteBoundExceeded);
        }
        let mut canonical_bytes = Vec::with_capacity(exact_size);
        canonical_bytes.extend_from_slice(ENVELOPE_DOMAIN_V1);
        push_text(&mut canonical_bytes, &self.target.to_string());
        canonical_bytes.push(code_object_version_tag(self.code_object_version) as u8);
        push_u32(&mut canonical_bytes, self.contracts.len());
        for contract in &self.contracts {
            encode_contract(&mut canonical_bytes, contract);
        }
        debug_assert_eq!(canonical_bytes.len(), exact_size);
        let identity = CompilerFfiEnvelopeIdentityV1(Sha256::digest(&canonical_bytes).into());
        let import_count = self
            .contracts
            .iter()
            .filter(|contract| contract.direction == DeviceFfiDirectionV1::Import)
            .count();
        let export_count = self.contracts.len() - import_count;
        Ok(CompilerFfiEnvelopeV1 {
            target: self.target,
            code_object_version: self.code_object_version,
            contracts: self.contracts,
            canonical_bytes,
            identity,
            inspection: CompilerFfiEnvelopeInspectionV1 {
                import_count,
                export_count,
                requires_compiler_module_definition_count: export_count,
            },
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerFfiTextFieldV1 {
    CrateLabel,
    ItemPath,
    ConcreteInstanceSymbol,
    Symbol,
    Target,
    PhysicalAbi,
    Effects,
}

/// Failure to construct one bounded canonical compiler FFI envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerFfiEnvelopeError {
    EmptyEnvelope,
    TooManyContracts {
        count: usize,
    },
    ContractCountMismatch {
        expected: usize,
        actual: usize,
    },
    InvalidText(CompilerFfiTextFieldV1),
    Grammar(DeviceFfiGrammarError),
    DirectionRoleMismatch,
    ContractIdentityMismatch {
        claimed: DeviceFfiContractIdV1,
        derived: DeviceFfiContractIdV1,
    },
    TargetMismatch,
    CodeObjectVersionMismatch,
    NonCanonicalContractOrder,
    AggregateTextBoundExceeded,
    EnvelopeByteBoundExceeded,
}

impl fmt::Display for CompilerFfiEnvelopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyEnvelope => formatter.write_str("compiler FFI envelope is empty"),
            Self::TooManyContracts { count } => write!(
                formatter,
                "compiler FFI envelope contains {count} contracts; maximum is {MAX_COMPILER_FFI_CONTRACTS_V1}"
            ),
            Self::ContractCountMismatch { expected, actual } => write!(
                formatter,
                "compiler FFI envelope declared {expected} contracts but received {actual}"
            ),
            Self::InvalidText(field) => write!(formatter, "invalid compiler FFI {field:?}"),
            Self::Grammar(error) => write!(formatter, "invalid compiler FFI grammar: {error}"),
            Self::DirectionRoleMismatch => {
                formatter.write_str("device FFI direction disagrees with required definition role")
            }
            Self::ContractIdentityMismatch { claimed, derived } => write!(
                formatter,
                "device FFI contract identity {} disagrees with canonical identity {}",
                claimed.to_hex(),
                derived.to_hex()
            ),
            Self::TargetMismatch => formatter.write_str("device FFI contract target mismatch"),
            Self::CodeObjectVersionMismatch => {
                formatter.write_str("device FFI contract code-object version mismatch")
            }
            Self::NonCanonicalContractOrder => {
                formatter.write_str("device FFI contracts are not in strict canonical order")
            }
            Self::AggregateTextBoundExceeded => {
                formatter.write_str("compiler FFI aggregate text bound exceeded")
            }
            Self::EnvelopeByteBoundExceeded => {
                formatter.write_str("compiler FFI canonical byte bound exceeded")
            }
        }
    }
}

impl Error for CompilerFfiEnvelopeError {}

fn direction_tag(direction: DeviceFfiDirectionV1) -> u16 {
    match direction {
        DeviceFfiDirectionV1::Import => DEVICE_FFI_DIRECTION_IMPORT_V1,
        DeviceFfiDirectionV1::Export => DEVICE_FFI_DIRECTION_EXPORT_V1,
    }
}

const fn code_object_version_tag(version: CodeObjectVersion) -> u16 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}

fn validate_text(
    text: &str,
    max_bytes: usize,
    field: CompilerFfiTextFieldV1,
    ascii_token: bool,
) -> Result<(), CompilerFfiEnvelopeError> {
    if text.is_empty()
        || text.len() > max_bytes
        || text.chars().any(char::is_control)
        || (ascii_token
            && (!text.is_ascii() || text.bytes().any(|byte| byte.is_ascii_whitespace())))
    {
        return Err(CompilerFfiEnvelopeError::InvalidText(field));
    }
    Ok(())
}

fn contract_sort_key(
    contract: &CompilerFfiContractV1,
) -> (
    u16,
    &str,
    DeviceFfiContractIdV1,
    CompilerFfiSourceOwnerIdentityV1,
) {
    (
        direction_tag(contract.direction),
        &contract.symbol,
        contract.contract_identity,
        contract.source_owner.identity,
    )
}

fn checked_text_sum(total: usize, text: &str) -> Result<usize, CompilerFfiEnvelopeError> {
    total
        .checked_add(text.len())
        .ok_or(CompilerFfiEnvelopeError::AggregateTextBoundExceeded)
}

fn exact_envelope_size(
    target: DeviceTargetV1,
    contracts: &[CompilerFfiContractV1],
) -> Result<usize, CompilerFfiEnvelopeError> {
    let mut size = ENVELOPE_DOMAIN_V1
        .len()
        .checked_add(encoded_text_size(&target.to_string())?)
        .and_then(|size| size.checked_add(1 + 4))
        .ok_or(CompilerFfiEnvelopeError::EnvelopeByteBoundExceeded)?;
    for contract in contracts {
        let fixed = 32 + 1 + 1 + 32 + 16 + 32 + 32;
        size = size
            .checked_add(fixed)
            .ok_or(CompilerFfiEnvelopeError::EnvelopeByteBoundExceeded)?;
        for text in [
            contract.source_owner.crate_label.as_str(),
            contract.source_owner.item_path.as_str(),
            contract.source_owner.concrete_instance_symbol.as_str(),
            contract.symbol.as_str(),
            contract.target.to_string().as_str(),
            contract.physical_abi.as_str(),
            contract.effects.as_str(),
        ] {
            size = size
                .checked_add(encoded_text_size(text)?)
                .ok_or(CompilerFfiEnvelopeError::EnvelopeByteBoundExceeded)?;
        }
    }
    Ok(size)
}

fn encoded_text_size(text: &str) -> Result<usize, CompilerFfiEnvelopeError> {
    u32::try_from(text.len()).map_err(|_| CompilerFfiEnvelopeError::EnvelopeByteBoundExceeded)?;
    4_usize
        .checked_add(text.len())
        .ok_or(CompilerFfiEnvelopeError::EnvelopeByteBoundExceeded)
}

fn encode_contract(bytes: &mut Vec<u8>, contract: &CompilerFfiContractV1) {
    bytes.extend_from_slice(&contract.contract_identity.as_bytes());
    bytes.push(direction_tag(contract.direction) as u8);
    bytes.push(contract.link_role as u8);
    bytes.extend_from_slice(&contract.source_owner.identity.as_bytes());
    push_text(bytes, &contract.source_owner.crate_label);
    push_text(bytes, &contract.source_owner.item_path);
    bytes.extend_from_slice(&contract.source_owner.def_path_hash);
    push_text(bytes, &contract.source_owner.concrete_instance_symbol);
    push_text(bytes, &contract.symbol);
    push_text(bytes, &contract.target.to_string());
    push_text(bytes, &contract.physical_abi);
    push_text(bytes, &contract.effects);
    bytes.extend_from_slice(&contract.effect_abi_identity.as_bytes());
    bytes.extend_from_slice(&contract.semantic_identity);
}

fn push_text(bytes: &mut Vec<u8>, text: &str) {
    push_u32(bytes, text.len());
    bytes.extend_from_slice(text.as_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(&(value as u32).to_le_bytes());
}

fn update_field(digest: &mut Sha256, field: &[u8]) {
    digest.update((field.len() as u64).to_le_bytes());
    digest.update(field);
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMPORT_ABI: &str =
        "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]";
    const EXPORT_ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";

    fn target() -> DeviceTargetV1 {
        DeviceTargetV1::parse("gfx942:xnack-").unwrap()
    }

    fn owner(byte: u8, crate_label: &str, item: &str) -> CompilerFfiSourceOwnerV1 {
        CompilerFfiSourceOwnerV1::new(
            crate_label,
            &format!("{crate_label}::{item}"),
            [byte; 16],
            &format!("_RINvNtCs1234_{crate_label}{item}"),
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
        let semantic_text = lower_hex(&semantic_identity);
        let fields = DeviceFfiContractFieldsV1 {
            direction: direction_tag(direction),
            symbol,
            calling_convention: "C",
            code_object_version: 5,
            target: "gfx942:xnack-",
            physical_abi: abi,
            effects,
            semantic_identity: &semantic_text,
        };
        CompilerFfiContractV1::new(
            derive_device_ffi_contract_id_v1(fields),
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

    fn envelope(crate_label: &str) -> CompilerFfiEnvelopeV1 {
        let mut builder =
            CompilerFfiEnvelopeBuilderV1::new(target(), CodeObjectVersion::V5, 2).unwrap();
        builder
            .push(contract(
                DeviceFfiDirectionV1::Import,
                "external_add",
                IMPORT_ABI,
                "read_global",
                0x11,
                owner(1, crate_label, "external_add"),
            ))
            .unwrap();
        builder
            .push(contract(
                DeviceFfiDirectionV1::Export,
                "rust_helper",
                EXPORT_ABI,
                "none",
                0x22,
                owner(2, crate_label, "rust_helper"),
            ))
            .unwrap();
        builder.finish().unwrap()
    }

    #[test]
    fn canonical_envelope_identity_is_stable_and_domain_separated() {
        let first = envelope("ffi_crate");
        let second = envelope("ffi_crate");
        assert_eq!(first.canonical_bytes(), second.canonical_bytes());
        assert_eq!(first.identity(), second.identity());
        assert_eq!(
            first.identity().to_hex(),
            "c3b5923e7c133b43020cceb5541d8f228b71659c0b5a9ae594afea7b2b89ff63"
        );
        assert!(first.canonical_bytes().starts_with(ENVELOPE_DOMAIN_V1));
        assert!(!first.grants_link_authority());
        assert_eq!(first.inspection().import_count(), 1);
        assert_eq!(first.inspection().export_count(), 1);
        assert_eq!(
            first
                .inspection()
                .requires_compiler_module_definition_count(),
            1
        );
        assert_ne!(first.identity(), envelope("other_crate").identity());
    }

    #[test]
    fn count_and_text_bounds_are_checked_before_builder_storage() {
        assert!(matches!(
            CompilerFfiEnvelopeBuilderV1::new(
                target(),
                CodeObjectVersion::V5,
                MAX_COMPILER_FFI_CONTRACTS_V1 + 1,
            ),
            Err(CompilerFfiEnvelopeError::TooManyContracts { .. })
        ));
        assert!(
            CompilerFfiSourceOwnerV1::new(
                &"a".repeat(MAX_COMPILER_FFI_CRATE_LABEL_BYTES_V1),
                "crate::item",
                [1; 16],
                "_Ritem",
            )
            .is_ok()
        );
        assert!(matches!(
            CompilerFfiSourceOwnerV1::new(
                &"a".repeat(MAX_COMPILER_FFI_CRATE_LABEL_BYTES_V1 + 1),
                "crate::item",
                [1; 16],
                "_Ritem",
            ),
            Err(CompilerFfiEnvelopeError::InvalidText(
                CompilerFfiTextFieldV1::CrateLabel
            ))
        ));

        let builder =
            CompilerFfiEnvelopeBuilderV1::new(target(), CodeObjectVersion::V5, 1).unwrap();
        assert!(matches!(
            builder.finish(),
            Err(CompilerFfiEnvelopeError::ContractCountMismatch {
                expected: 1,
                actual: 0
            })
        ));
    }

    #[test]
    fn direction_role_identity_and_order_mismatches_fail_closed() {
        let source_owner = owner(1, "ffi_crate", "external_add");
        let semantic = [0x11; 32];
        let semantic_text = lower_hex(&semantic);
        let id = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
            symbol: "external_add",
            calling_convention: "C",
            code_object_version: 5,
            target: "gfx942:xnack-",
            physical_abi: IMPORT_ABI,
            effects: "read_global",
            semantic_identity: &semantic_text,
        });
        assert!(matches!(
            CompilerFfiContractV1::new(
                id,
                DeviceFfiDirectionV1::Import,
                CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
                target(),
                CodeObjectVersion::V5,
                source_owner.clone(),
                "external_add",
                IMPORT_ABI,
                "read_global",
                semantic,
            ),
            Err(CompilerFfiEnvelopeError::DirectionRoleMismatch)
        ));
        assert!(matches!(
            CompilerFfiContractV1::new(
                DeviceFfiContractIdV1::from_bytes([0x55; 32]),
                DeviceFfiDirectionV1::Import,
                CompilerFfiLinkRoleV1::RequiresExternalDefinition,
                target(),
                CodeObjectVersion::V5,
                source_owner,
                "external_add",
                IMPORT_ABI,
                "read_global",
                semantic,
            ),
            Err(CompilerFfiEnvelopeError::ContractIdentityMismatch { .. })
        ));

        let mut builder =
            CompilerFfiEnvelopeBuilderV1::new(target(), CodeObjectVersion::V5, 2).unwrap();
        builder
            .push(contract(
                DeviceFfiDirectionV1::Export,
                "rust_helper",
                EXPORT_ABI,
                "none",
                0x22,
                owner(2, "ffi_crate", "rust_helper"),
            ))
            .unwrap();
        assert!(matches!(
            builder.push(contract(
                DeviceFfiDirectionV1::Import,
                "external_add",
                IMPORT_ABI,
                "read_global",
                0x11,
                owner(1, "ffi_crate", "external_add"),
            )),
            Err(CompilerFfiEnvelopeError::NonCanonicalContractOrder)
        ));
    }

    #[test]
    fn effect_abi_identity_binds_both_validated_fields() {
        let read = contract(
            DeviceFfiDirectionV1::Import,
            "external_add",
            IMPORT_ABI,
            "read_global",
            0x11,
            owner(1, "ffi_crate", "external_add"),
        );
        let write = contract(
            DeviceFfiDirectionV1::Import,
            "external_write",
            IMPORT_ABI,
            "write_global",
            0x12,
            owner(2, "ffi_crate", "external_write"),
        );
        assert_ne!(read.effect_abi_identity(), write.effect_abi_identity());
    }
}
