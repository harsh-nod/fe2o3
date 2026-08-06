//! Adapter from private rustc collection state to the neutral FFI envelope.

use std::{error::Error, fmt};

use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiContractV1, CompilerFfiEnvelopeBuilderV1,
    CompilerFfiEnvelopeError, CompilerFfiEnvelopeV1, CompilerFfiLinkRoleV1,
    CompilerFfiSourceOwnerV1, DeviceTargetV1,
};
use reserved_fe2o3_symbols::DeviceFfiDirectionV1;

use crate::{
    collector::CollectionResult,
    device_ffi::{ClosedDeviceFfiContract, DeviceFfiClosure, DeviceFfiLinkRole},
};

pub(crate) fn adapt_collection_v1(
    collection: &CollectionResult<'_>,
) -> Result<Option<CompilerFfiEnvelopeV1>, CompilerFfiAdapterError> {
    adapt_closure_v1(&collection.device_ffi, |symbol| {
        collection
            .functions
            .iter()
            .any(|function| function.export_name == symbol)
    })
}

pub(crate) fn adapt_closure_v1(
    closure: &DeviceFfiClosure,
    export_is_collected: impl Fn(&str) -> bool,
) -> Result<Option<CompilerFfiEnvelopeV1>, CompilerFfiAdapterError> {
    let contract_count = closure
        .imports
        .len()
        .checked_add(closure.exports.len())
        .ok_or(CompilerFfiAdapterError::ContractCountOverflow)?;
    if contract_count == 0 {
        if closure.target.is_some() || closure.code_object_version_assertion.is_some() {
            return Err(CompilerFfiAdapterError::InconsistentEmptyClosure);
        }
        return Ok(None);
    }

    let target_text = closure
        .target
        .as_deref()
        .ok_or(CompilerFfiAdapterError::MissingTarget)?;
    let target = DeviceTargetV1::parse(target_text)
        .map_err(|_| CompilerFfiAdapterError::InvalidTarget(target_text.to_owned()))?;
    let asserted_code_object_version = *closure
        .code_object_version_assertion
        .as_ref()
        .ok_or(CompilerFfiAdapterError::MissingCodeObjectVersion)?
        .asserted_for_consistency_check();
    let code_object_version = match asserted_code_object_version {
        4 => CodeObjectVersion::V4,
        5 => CodeObjectVersion::V5,
        6 => CodeObjectVersion::V6,
        value => return Err(CompilerFfiAdapterError::InvalidCodeObjectVersion(value)),
    };

    let mut builder =
        CompilerFfiEnvelopeBuilderV1::new(target, code_object_version, contract_count)
            .map_err(CompilerFfiAdapterError::Envelope)?;
    for entry in &closure.imports {
        builder
            .push(adapt_contract_v1(
                entry,
                target,
                code_object_version,
                DeviceFfiDirectionV1::Import,
                CompilerFfiLinkRoleV1::RequiresExternalDefinition,
            )?)
            .map_err(CompilerFfiAdapterError::Envelope)?;
    }
    for entry in &closure.exports {
        if !export_is_collected(&entry.contract.symbol) {
            return Err(CompilerFfiAdapterError::ExportMissingFromCollection(
                entry.contract.symbol.clone(),
            ));
        }
        builder
            .push(adapt_contract_v1(
                entry,
                target,
                code_object_version,
                DeviceFfiDirectionV1::Export,
                CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
            )?)
            .map_err(CompilerFfiAdapterError::Envelope)?;
    }
    builder
        .finish()
        .map(Some)
        .map_err(CompilerFfiAdapterError::Envelope)
}

fn adapt_contract_v1(
    entry: &ClosedDeviceFfiContract,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    expected_direction: DeviceFfiDirectionV1,
    expected_role: CompilerFfiLinkRoleV1,
) -> Result<CompilerFfiContractV1, CompilerFfiAdapterError> {
    if entry.contract.direction != expected_direction {
        return Err(CompilerFfiAdapterError::ClosurePartitionMismatch(
            entry.contract.symbol.clone(),
        ));
    }
    let asserted_role = match entry.link_role_assertion.asserted_for_consistency_check() {
        DeviceFfiLinkRole::RequiresExternalDefinition => {
            CompilerFfiLinkRoleV1::RequiresExternalDefinition
        }
        DeviceFfiLinkRole::RequiresCompilerModuleDefinition => {
            CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition
        }
    };
    if asserted_role != expected_role {
        return Err(CompilerFfiAdapterError::LinkRoleMismatch(
            entry.contract.symbol.clone(),
        ));
    }
    if entry.contract.target != target.to_string() {
        return Err(CompilerFfiAdapterError::ContractTargetMismatch(
            entry.contract.symbol.clone(),
        ));
    }
    let asserted_version = *entry
        .contract
        .code_object_version_assertion
        .asserted_for_consistency_check();
    if asserted_version != code_object_version_tag(code_object_version) {
        return Err(CompilerFfiAdapterError::ContractCodeObjectVersionMismatch(
            entry.contract.symbol.clone(),
        ));
    }

    let semantic_identity = parse_lower_hex_32(
        entry
            .contract
            .semantic_identity_assertion
            .asserted_for_consistency_check(),
    )
    .ok_or_else(|| {
        CompilerFfiAdapterError::InvalidSemanticIdentity(entry.contract.symbol.clone())
    })?;
    let source_owner = CompilerFfiSourceOwnerV1::new(
        &entry.owner.crate_name,
        &entry.owner.item_path,
        entry.owner.def_path_hash,
        &entry.owner.concrete_instance_symbol,
    )
    .map_err(CompilerFfiAdapterError::Envelope)?;
    CompilerFfiContractV1::new(
        entry.contract.id,
        expected_direction,
        expected_role,
        target,
        code_object_version,
        source_owner,
        &entry.contract.symbol,
        &entry.contract.physical_abi,
        entry
            .contract
            .effects_assertion
            .asserted_for_consistency_check(),
        semantic_identity,
    )
    .map_err(CompilerFfiAdapterError::Envelope)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CompilerFfiAdapterError {
    ContractCountOverflow,
    InconsistentEmptyClosure,
    MissingTarget,
    InvalidTarget(String),
    MissingCodeObjectVersion,
    InvalidCodeObjectVersion(u16),
    ClosurePartitionMismatch(String),
    LinkRoleMismatch(String),
    ContractTargetMismatch(String),
    ContractCodeObjectVersionMismatch(String),
    InvalidSemanticIdentity(String),
    ExportMissingFromCollection(String),
    Envelope(CompilerFfiEnvelopeError),
}

impl fmt::Display for CompilerFfiAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContractCountOverflow => {
                formatter.write_str("device FFI contract count overflow")
            }
            Self::InconsistentEmptyClosure => {
                formatter.write_str("empty device FFI closure retains target or version state")
            }
            Self::MissingTarget => formatter.write_str("nonempty device FFI closure has no target"),
            Self::InvalidTarget(target) => {
                write!(
                    formatter,
                    "device FFI closure target `{target}` is noncanonical"
                )
            }
            Self::MissingCodeObjectVersion => {
                formatter.write_str("nonempty device FFI closure has no code-object version")
            }
            Self::InvalidCodeObjectVersion(version) => write!(
                formatter,
                "device FFI closure has unsupported code-object version {version}"
            ),
            Self::ClosurePartitionMismatch(symbol) => write!(
                formatter,
                "device FFI `{symbol}` is stored in the wrong import/export partition"
            ),
            Self::LinkRoleMismatch(symbol) => write!(
                formatter,
                "device FFI `{symbol}` has a direction-incompatible definition role"
            ),
            Self::ContractTargetMismatch(symbol) => {
                write!(
                    formatter,
                    "device FFI `{symbol}` disagrees with closure target"
                )
            }
            Self::ContractCodeObjectVersionMismatch(symbol) => write!(
                formatter,
                "device FFI `{symbol}` disagrees with closure code-object version"
            ),
            Self::InvalidSemanticIdentity(symbol) => write!(
                formatter,
                "device FFI `{symbol}` has a noncanonical semantic identity"
            ),
            Self::ExportMissingFromCollection(symbol) => write!(
                formatter,
                "device FFI export `{symbol}` is absent from the collected compiler definitions"
            ),
            Self::Envelope(error) => error.fmt(formatter),
        }
    }
}

impl Error for CompilerFfiAdapterError {}

const fn code_object_version_tag(version: CodeObjectVersion) -> u16 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}

fn parse_lower_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut decoded = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = decode_lower_hex(pair[0])?;
        let low = decode_lower_hex(pair[1])?;
        decoded[index] = (high << 4) | low;
    }
    Some(decoded)
}

const fn decode_lower_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_semantic_identity_parser_rejects_noncanonical_hex() {
        assert_eq!(parse_lower_hex_32(&"ab".repeat(32)), Some([0xab; 32]));
        for invalid in [
            String::new(),
            "a".repeat(63),
            "a".repeat(65),
            "AB".repeat(32),
            format!("{}g0", "ab".repeat(31)),
        ] {
            assert_eq!(parse_lower_hex_32(&invalid), None, "{invalid}");
        }
    }
}
