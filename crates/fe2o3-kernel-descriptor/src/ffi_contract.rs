//! Bounded, authority-free contracts for direct device symbol linking.

use std::collections::BTreeSet;
use std::fmt;

use fe2o3_amd_target::AmdTargetId;
use sha2::{Digest, Sha256};

pub const MAX_DEVICE_FFI_CONTRACTS_V1: usize = 128;
pub const MAX_DEVICE_FFI_ARGUMENTS_V1: usize = 32;
pub const MAX_DEVICE_FFI_SYMBOL_BYTES_V1: usize = 128;
pub const MAX_DEVICE_FFI_EFFECTS_V1: usize = 16;
pub const MAX_DEVICE_FFI_CANONICAL_BYTES_V1: usize = 4 * 1024;

const CONTRACT_DOMAIN_V1: &[u8] = b"fe2o3.device-ffi-contract.v1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceFfiDirectionV1 {
    Import,
    Export,
}

impl DeviceFfiDirectionV1 {
    const fn tag(self) -> u16 {
        match self {
            Self::Import => 1,
            Self::Export => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceFfiScalarV1 {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl DeviceFfiScalarV1 {
    pub const fn size_bytes(self) -> u8 {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
        }
    }

    fn spelling(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::U8 => "u8",
            Self::I16 => "i16",
            Self::U16 => "u16",
            Self::I32 => "i32",
            Self::U32 => "u32",
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceAddressSpaceV1 {
    Global,
    Constant,
    Workgroup,
    Private,
}

impl DeviceAddressSpaceV1 {
    fn spelling(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Constant => "constant",
            Self::Workgroup => "workgroup",
            Self::Private => "private",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceFfiAbiTypeV1 {
    Scalar(DeviceFfiScalarV1),
    Pointer {
        address_space: DeviceAddressSpaceV1,
        mutable: bool,
        element: DeviceFfiScalarV1,
    },
}

impl DeviceFfiAbiTypeV1 {
    fn encode(self, output: &mut String) {
        match self {
            Self::Scalar(scalar) => {
                let size = scalar.size_bytes();
                output.push_str(scalar.spelling());
                output.push_str("[size=");
                output.push_str(&size.to_string());
                output.push_str(",align=");
                output.push_str(&size.to_string());
                output.push(']');
            }
            Self::Pointer {
                address_space,
                mutable,
                element,
            } => {
                output.push_str(if mutable { "mut_ptr<" } else { "const_ptr<" });
                output.push_str(address_space.spelling());
                output.push(',');
                output.push_str(element.spelling());
                output.push_str(">[size=8,align=8,as=");
                output.push_str(address_space.spelling());
                output.push(']');
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevicePhysicalAbiV1 {
    arguments: Vec<DeviceFfiAbiTypeV1>,
    result: Option<DeviceFfiAbiTypeV1>,
}

impl DevicePhysicalAbiV1 {
    pub fn new(
        arguments: Vec<DeviceFfiAbiTypeV1>,
        result: Option<DeviceFfiAbiTypeV1>,
    ) -> Result<Self, DeviceFfiContractError> {
        if arguments.len() > MAX_DEVICE_FFI_ARGUMENTS_V1 {
            return Err(DeviceFfiContractError::TooManyArguments);
        }
        Ok(Self { arguments, result })
    }

    pub fn arguments(&self) -> &[DeviceFfiAbiTypeV1] {
        &self.arguments
    }

    pub const fn result(&self) -> Option<DeviceFfiAbiTypeV1> {
        self.result
    }

    pub fn canonical_spelling(&self) -> String {
        let mut output = String::from("C(");
        for (index, argument) in self.arguments.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            argument.encode(&mut output);
        }
        output.push_str(")->");
        match self.result {
            Some(result) => result.encode(&mut output),
            None => output.push_str("unit[size=0,align=1]"),
        }
        output
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceMemoryEffectV1 {
    AtomicGlobal,
    AtomicWorkgroup,
    BarrierWorkgroup,
    ReadConstant,
    ReadGlobal,
    ReadPrivate,
    ReadWorkgroup,
    WriteGlobal,
    WritePrivate,
    WriteWorkgroup,
}

impl DeviceMemoryEffectV1 {
    fn spelling(self) -> &'static str {
        match self {
            Self::AtomicGlobal => "atomic_global",
            Self::AtomicWorkgroup => "atomic_workgroup",
            Self::BarrierWorkgroup => "barrier_workgroup",
            Self::ReadConstant => "read_constant",
            Self::ReadGlobal => "read_global",
            Self::ReadPrivate => "read_private",
            Self::ReadWorkgroup => "read_workgroup",
            Self::WriteGlobal => "write_global",
            Self::WritePrivate => "write_private",
            Self::WriteWorkgroup => "write_workgroup",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct DeviceFfiContractIdV1([u8; 32]);

impl DeviceFfiContractIdV1 {
    pub const fn from_sha256_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct DeviceFfiSemanticIdentityV1([u8; 32]);

impl DeviceFfiSemanticIdentityV1 {
    pub const fn from_opaque_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceFfiContractV1 {
    id: DeviceFfiContractIdV1,
    direction: DeviceFfiDirectionV1,
    symbol: String,
    target: AmdTargetId,
    code_object_version: u16,
    abi: DevicePhysicalAbiV1,
    effects: Vec<DeviceMemoryEffectV1>,
    semantic_identity: DeviceFfiSemanticIdentityV1,
}

impl DeviceFfiContractV1 {
    pub fn new(
        direction: DeviceFfiDirectionV1,
        symbol: impl Into<String>,
        target: AmdTargetId,
        code_object_version: u16,
        abi: DevicePhysicalAbiV1,
        effects: Vec<DeviceMemoryEffectV1>,
        semantic_identity: DeviceFfiSemanticIdentityV1,
    ) -> Result<Self, DeviceFfiContractError> {
        let symbol = symbol.into();
        validate_symbol(&symbol)?;
        if !matches!(code_object_version, 4..=6) {
            return Err(DeviceFfiContractError::UnsupportedCodeObjectVersion);
        }
        if effects.len() > MAX_DEVICE_FFI_EFFECTS_V1 {
            return Err(DeviceFfiContractError::TooManyEffects);
        }
        let mut seen = BTreeSet::new();
        let mut previous = None;
        for effect in &effects {
            if !seen.insert(*effect) {
                return Err(DeviceFfiContractError::DuplicateEffect);
            }
            if previous.is_some_and(|previous| previous >= *effect) {
                return Err(DeviceFfiContractError::NonCanonicalEffects);
            }
            previous = Some(*effect);
        }
        if semantic_identity.0 == [0; 32] {
            return Err(DeviceFfiContractError::MissingSemanticIdentity);
        }
        validate_effect_abi_compatibility(&effects, &abi)?;

        let mut contract = Self {
            id: DeviceFfiContractIdV1([0; 32]),
            direction,
            symbol,
            target,
            code_object_version,
            abi,
            effects,
            semantic_identity,
        };
        let bytes = contract.canonical_preimage();
        if bytes.len() > MAX_DEVICE_FFI_CANONICAL_BYTES_V1 {
            return Err(DeviceFfiContractError::ContractTooLarge);
        }
        contract.id = DeviceFfiContractIdV1(Sha256::digest(bytes).into());
        Ok(contract)
    }

    pub fn verify_claimed_id(
        claimed: DeviceFfiContractIdV1,
        contract: Self,
    ) -> Result<Self, DeviceFfiContractError> {
        if contract.id != claimed {
            return Err(DeviceFfiContractError::IdentityMismatch);
        }
        Ok(contract)
    }

    pub const fn id(&self) -> DeviceFfiContractIdV1 {
        self.id
    }

    pub const fn direction(&self) -> DeviceFfiDirectionV1 {
        self.direction
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub const fn target(&self) -> AmdTargetId {
        self.target
    }

    pub const fn code_object_version(&self) -> u16 {
        self.code_object_version
    }

    pub const fn abi(&self) -> &DevicePhysicalAbiV1 {
        &self.abi
    }

    pub fn effects(&self) -> &[DeviceMemoryEffectV1] {
        &self.effects
    }

    pub const fn semantic_identity(&self) -> DeviceFfiSemanticIdentityV1 {
        self.semantic_identity
    }

    /// Returns deterministic, data-only bytes for the direct LLVM worker.
    /// Possessing these bytes grants no symbol resolution, code loading, call,
    /// or launch authority.
    pub fn canonical_link_record(&self) -> Vec<u8> {
        let mut bytes = self.canonical_preimage();
        bytes.extend_from_slice(&self.id.0);
        bytes
    }

    fn canonical_preimage(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(CONTRACT_DOMAIN_V1);
        bytes.extend_from_slice(&self.direction.tag().to_le_bytes());
        append_field(&mut bytes, self.symbol.as_bytes());
        append_field(&mut bytes, b"C");
        bytes.extend_from_slice(&self.code_object_version.to_le_bytes());
        append_field(&mut bytes, self.target.to_string().as_bytes());
        append_field(&mut bytes, self.abi.canonical_spelling().as_bytes());
        let effects = if self.effects.is_empty() {
            "none".to_owned()
        } else {
            self.effects
                .iter()
                .map(|effect| effect.spelling())
                .collect::<Vec<_>>()
                .join(",")
        };
        append_field(&mut bytes, effects.as_bytes());
        append_field(&mut bytes, &hex(self.semantic_identity.0).into_bytes());
        append_field(&mut bytes, b"nounwind;nopanic");
        bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceFfiContractError {
    InvalidSymbol,
    TooManyArguments,
    TooManyEffects,
    DuplicateEffect,
    NonCanonicalEffects,
    UnsupportedCodeObjectVersion,
    MissingSemanticIdentity,
    EffectAbiMismatch,
    ContractTooLarge,
    IdentityMismatch,
}

impl fmt::Display for DeviceFfiContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSymbol => "invalid device FFI symbol",
            Self::TooManyArguments => "too many device FFI arguments",
            Self::TooManyEffects => "too many device FFI memory effects",
            Self::DuplicateEffect => "duplicate device FFI memory effect",
            Self::NonCanonicalEffects => "device FFI effects are not in canonical order",
            Self::UnsupportedCodeObjectVersion => "unsupported device code-object version",
            Self::MissingSemanticIdentity => "device FFI semantic identity is missing",
            Self::EffectAbiMismatch => "device FFI effect has no compatible pointer argument",
            Self::ContractTooLarge => "device FFI contract exceeds its byte bound",
            Self::IdentityMismatch => "device FFI contract identity mismatch",
        })
    }
}

impl std::error::Error for DeviceFfiContractError {}

fn validate_symbol(symbol: &str) -> Result<(), DeviceFfiContractError> {
    let mut bytes = symbol.bytes();
    let valid_first = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'.' | b'$'));
    let valid_rest = bytes.all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$' | b'@' | b'-')
    });
    if symbol.len() <= MAX_DEVICE_FFI_SYMBOL_BYTES_V1 && valid_first && valid_rest {
        Ok(())
    } else {
        Err(DeviceFfiContractError::InvalidSymbol)
    }
}

fn append_field(output: &mut Vec<u8>, field: &[u8]) {
    output.extend_from_slice(&(field.len() as u64).to_le_bytes());
    output.extend_from_slice(field);
}

fn validate_effect_abi_compatibility(
    effects: &[DeviceMemoryEffectV1],
    abi: &DevicePhysicalAbiV1,
) -> Result<(), DeviceFfiContractError> {
    for effect in effects {
        let matches = abi.arguments.iter().any(|argument| {
            matches!(
                (effect, argument),
                (
                    DeviceMemoryEffectV1::ReadConstant,
                    DeviceFfiAbiTypeV1::Pointer {
                        address_space: DeviceAddressSpaceV1::Constant,
                        mutable: false,
                        ..
                    },
                ) | (
                    DeviceMemoryEffectV1::ReadGlobal,
                    DeviceFfiAbiTypeV1::Pointer {
                        address_space: DeviceAddressSpaceV1::Global,
                        ..
                    },
                ) | (
                    DeviceMemoryEffectV1::ReadPrivate,
                    DeviceFfiAbiTypeV1::Pointer {
                        address_space: DeviceAddressSpaceV1::Private,
                        ..
                    },
                ) | (
                    DeviceMemoryEffectV1::ReadWorkgroup,
                    DeviceFfiAbiTypeV1::Pointer {
                        address_space: DeviceAddressSpaceV1::Workgroup,
                        ..
                    },
                ) | (
                    DeviceMemoryEffectV1::WriteGlobal | DeviceMemoryEffectV1::AtomicGlobal,
                    DeviceFfiAbiTypeV1::Pointer {
                        address_space: DeviceAddressSpaceV1::Global,
                        mutable: true,
                        ..
                    },
                ) | (
                    DeviceMemoryEffectV1::WritePrivate,
                    DeviceFfiAbiTypeV1::Pointer {
                        address_space: DeviceAddressSpaceV1::Private,
                        mutable: true,
                        ..
                    },
                ) | (
                    DeviceMemoryEffectV1::WriteWorkgroup | DeviceMemoryEffectV1::AtomicWorkgroup,
                    DeviceFfiAbiTypeV1::Pointer {
                        address_space: DeviceAddressSpaceV1::Workgroup,
                        mutable: true,
                        ..
                    },
                ) | (DeviceMemoryEffectV1::BarrierWorkgroup, _)
            )
        });
        if !matches && *effect != DeviceMemoryEffectV1::BarrierWorkgroup {
            return Err(DeviceFfiContractError::EffectAbiMismatch);
        }
    }
    Ok(())
}

fn hex(bytes: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(64);
    for byte in bytes {
        value.push(HEX[usize::from(byte >> 4)] as char);
        value.push(HEX[usize::from(byte & 0xf)] as char);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(direction: DeviceFfiDirectionV1) -> DeviceFfiContractV1 {
        DeviceFfiContractV1::new(
            direction,
            "saxpy_impl",
            AmdTargetId::parse("gfx942:sramecc+:xnack-").unwrap(),
            5,
            DevicePhysicalAbiV1::new(
                vec![
                    DeviceFfiAbiTypeV1::Pointer {
                        address_space: DeviceAddressSpaceV1::Global,
                        mutable: true,
                        element: DeviceFfiScalarV1::F32,
                    },
                    DeviceFfiAbiTypeV1::Scalar(DeviceFfiScalarV1::U64),
                ],
                None,
            )
            .unwrap(),
            vec![
                DeviceMemoryEffectV1::ReadGlobal,
                DeviceMemoryEffectV1::WriteGlobal,
            ],
            DeviceFfiSemanticIdentityV1::from_opaque_bytes([7; 32]),
        )
        .unwrap()
    }

    #[test]
    fn record_binds_every_link_relevant_field() {
        let base = contract(DeviceFfiDirectionV1::Export);
        let import = contract(DeviceFfiDirectionV1::Import);
        assert_ne!(base.id(), import.id());
        assert_ne!(base.canonical_link_record(), import.canonical_link_record());

        let mut mutated = base.canonical_link_record();
        let last = mutated.len() - 1;
        mutated[last] ^= 1;
        assert_ne!(mutated, base.canonical_link_record());
    }

    #[test]
    fn claimed_identity_fails_closed() {
        let base = contract(DeviceFfiDirectionV1::Export);
        let error = DeviceFfiContractV1::verify_claimed_id(
            DeviceFfiContractIdV1::from_sha256_bytes([0; 32]),
            base,
        )
        .unwrap_err();
        assert_eq!(error, DeviceFfiContractError::IdentityMismatch);
    }

    #[test]
    fn malformed_and_noncanonical_inputs_are_rejected() {
        let abi = DevicePhysicalAbiV1::new(Vec::new(), None).unwrap();
        let target = AmdTargetId::parse("gfx942").unwrap();
        let semantic = DeviceFfiSemanticIdentityV1::from_opaque_bytes([1; 32]);
        assert!(matches!(
            DeviceFfiContractV1::new(
                DeviceFfiDirectionV1::Import,
                "bad|symbol",
                target,
                5,
                abi.clone(),
                Vec::new(),
                semantic,
            ),
            Err(DeviceFfiContractError::InvalidSymbol)
        ));
        assert!(matches!(
            DeviceFfiContractV1::new(
                DeviceFfiDirectionV1::Import,
                "valid",
                target,
                3,
                abi.clone(),
                Vec::new(),
                semantic,
            ),
            Err(DeviceFfiContractError::UnsupportedCodeObjectVersion)
        ));
        assert!(matches!(
            DeviceFfiContractV1::new(
                DeviceFfiDirectionV1::Import,
                "valid",
                target,
                5,
                abi,
                vec![
                    DeviceMemoryEffectV1::WriteGlobal,
                    DeviceMemoryEffectV1::ReadGlobal,
                ],
                semantic,
            ),
            Err(DeviceFfiContractError::NonCanonicalEffects)
        ));
        assert!(matches!(
            DeviceFfiContractV1::new(
                DeviceFfiDirectionV1::Import,
                "valid",
                target,
                5,
                DevicePhysicalAbiV1::new(Vec::new(), None).unwrap(),
                Vec::new(),
                DeviceFfiSemanticIdentityV1::from_opaque_bytes([0; 32]),
            ),
            Err(DeviceFfiContractError::MissingSemanticIdentity)
        ));
        assert!(matches!(
            DeviceFfiContractV1::new(
                DeviceFfiDirectionV1::Import,
                "valid",
                target,
                5,
                DevicePhysicalAbiV1::new(Vec::new(), None).unwrap(),
                vec![DeviceMemoryEffectV1::ReadGlobal],
                semantic,
            ),
            Err(DeviceFfiContractError::EffectAbiMismatch)
        ));
    }

    #[test]
    fn exact_pointer_layout_and_address_space_are_canonical() {
        let abi = contract(DeviceFfiDirectionV1::Export)
            .abi()
            .canonical_spelling();
        assert_eq!(
            abi,
            "C(mut_ptr<global,f32>[size=8,align=8,as=global],u64[size=8,align=8])->unit[size=0,align=1]"
        );
    }
}
