//! Reserved names and registration values shared by fe2o3 macros and the backend.
//!
//! Kernel registration is a compiler contract, not an authenticity boundary. The
//! backend validates that records are structurally correct and internally
//! consistent, but Rust source can reproduce the reserved names and field values.
//! Code compiled with the fe2o3 backend is therefore trusted to emit honest
//! registrations.

use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

pub const RESERVED_ROOT: &str = "fe2o3_";
pub const KERNEL_PREFIX: &str = "fe2o3_kernel_";
pub const DEVICE_PREFIX: &str = "fe2o3_device_";
pub const DEVICE_EXTERN_PREFIX: &str = "fe2o3_device_extern_";

/// Final-path-segment prefix for kernel registration statics.
pub const KERNEL_REGISTRATION_PREFIX: &str = "__fe2o3_kernel_registration_";

/// ASCII `FE2O3KRN`, interpreted as a little-endian `u64`.
pub const KERNEL_REGISTRATION_MAGIC: u64 = 0x4e52_4b33_4f32_4546;
pub const KERNEL_REGISTRATION_VERSION_V1: u16 = 1;
pub const KERNEL_REGISTRATION_VERSION_V2: u16 = 2;
/// An ordinary `#[kernel]` registration without a generated typed profile.
pub const KERNEL_REGISTRATION_KIND_KERNEL: u16 = 1;
/// A `#[kernel(typed)]` registration using the exact typed vecadd V1 profile.
pub const KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1: u16 = 2;
/// A `#[kernel(typed)]` registration whose exact vecadd ABI identities are
/// derived from canonical rustc type/layout evidence.
pub const KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2: u16 = 3;

/// V1 is an immutable `#[used]` static with this exact tuple shape:
///
/// `(u64 magic, u16 version, u16 kind, &str logical_name, &str export_name, fn pointer)`.
///
/// The function pointer is the direct association to the generated kernel item.
pub const KERNEL_REGISTRATION_V1_FIELD_COUNT: usize = 6;

/// V2 extends V1 with canonical crate and kernel binding IDs before the
/// function pointer. Typed registrations must use this version.
pub const KERNEL_REGISTRATION_V2_FIELD_COUNT: usize = 8;

/// Final-path-segment prefix for device FFI registration statics.
pub const DEVICE_FFI_REGISTRATION_PREFIX_V1: &str = "__fe2o3_device_ffi_registration_v1_";
/// Prefix carried in compiler-visible documentation metadata on an FFI item.
pub const DEVICE_FFI_MARKER_PREFIX_V1: &str = "__fe2o3_device_ffi_v1|";
/// ASCII `FE2O3FFI`, interpreted as a little-endian `u64`.
pub const DEVICE_FFI_REGISTRATION_MAGIC_V1: u64 = 0x4946_4633_4f32_4546;
pub const DEVICE_FFI_REGISTRATION_VERSION_V1: u16 = 1;
pub const DEVICE_FFI_DIRECTION_IMPORT_V1: u16 = 1;
pub const DEVICE_FFI_DIRECTION_EXPORT_V1: u16 = 2;
/// `(magic, version, direction, contract, symbol, cc, code-object,
/// target, physical ABI, effects, semantic identity, function pointer)`.
pub const DEVICE_FFI_REGISTRATION_V1_FIELD_COUNT: usize = 12;
pub const MAX_DEVICE_FFI_SYMBOL_BYTES_V1: usize = 128;
pub const MAX_DEVICE_FFI_TARGET_BYTES_V1: usize = 128;
pub const MAX_DEVICE_FFI_PHYSICAL_ABI_BYTES_V1: usize = 2_048;
pub const MAX_DEVICE_FFI_EFFECT_BYTES_V1: usize = 256;
pub const MAX_DEVICE_FFI_ARGUMENTS_V1: usize = 32;

const DEVICE_FFI_CONTRACT_DOMAIN_V1: &[u8] = b"fe2o3.device-ffi-contract.v1\0";

/// Environment variable populated by the fe2o3 rustc wrapper with the exact
/// compilation unit's crate binding ID.
pub const CRATE_BINDING_ID_ENV_V1: &str = "FE2O3_CRATE_BINDING_ID_V1";

/// Stable profile tag included in typed vecadd kernel binding derivation.
pub const TYPED_VECADD_F32_PROFILE_TAG_V1: &str = "typed-vecadd-f32-v1";
/// Stable profile tag for rustc-derived typed vecadd ABI evidence.
pub const TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2: &str = "typed-vecadd-f32-rustc-layout-v2";

const CRATE_BINDING_DOMAIN_V1: &[u8] = b"fe2o3.crate-binding.v1\0";
const KERNEL_BINDING_DOMAIN_V1: &[u8] = b"fe2o3.kernel-binding.v1\0";
const ARTIFACT_ACCESSOR_PREFIX_V1: &str = "__fe2o3_artifact_v1_";
const HOST_KERNEL_PREFIX_V1: &str = "__fe2o3_host_kernel_v1_";
const BINDING_ID_BYTES: usize = 32;
const BINDING_ID_HEX_BYTES: usize = BINDING_ID_BYTES * 2;

/// Full SHA-256 identity of one rustc compilation unit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CrateBindingIdV1([u8; BINDING_ID_BYTES]);

impl CrateBindingIdV1 {
    /// Constructs an identity from exact digest bytes.
    pub const fn from_bytes(bytes: [u8; BINDING_ID_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes.
    pub const fn as_bytes(self) -> [u8; BINDING_ID_BYTES] {
        self.0
    }

    /// Parses the canonical lowercase hexadecimal representation.
    pub fn from_hex(value: &str) -> Result<Self, BindingIdError> {
        parse_binding_hex(value).map(Self)
    }

    /// Returns the canonical lowercase hexadecimal representation.
    pub fn to_hex(self) -> String {
        encode_binding_hex(self.0)
    }
}

/// Full SHA-256 identity of one kernel in one rustc compilation unit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KernelBindingIdV1([u8; BINDING_ID_BYTES]);

impl KernelBindingIdV1 {
    /// Constructs an identity from exact digest bytes.
    pub const fn from_bytes(bytes: [u8; BINDING_ID_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes.
    pub const fn as_bytes(self) -> [u8; BINDING_ID_BYTES] {
        self.0
    }

    /// Parses the canonical lowercase hexadecimal representation.
    pub fn from_hex(value: &str) -> Result<Self, BindingIdError> {
        parse_binding_hex(value).map(Self)
    }

    /// Returns the canonical lowercase hexadecimal representation.
    pub fn to_hex(self) -> String {
        encode_binding_hex(self.0)
    }
}

/// Full SHA-256 identity of one exact device FFI declaration.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeviceFfiContractIdV1([u8; BINDING_ID_BYTES]);

impl DeviceFfiContractIdV1 {
    pub const fn from_bytes(bytes: [u8; BINDING_ID_BYTES]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; BINDING_ID_BYTES] {
        self.0
    }

    pub fn from_hex(value: &str) -> Result<Self, BindingIdError> {
        parse_binding_hex(value).map(Self)
    }

    pub fn to_hex(self) -> String {
        encode_binding_hex(self.0)
    }
}

/// Canonical fields hashed into a [`DeviceFfiContractIdV1`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceFfiContractFieldsV1<'a> {
    pub direction: u16,
    pub symbol: &'a str,
    pub calling_convention: &'a str,
    pub code_object_version: u16,
    pub target: &'a str,
    pub physical_abi: &'a str,
    pub effects: &'a str,
    pub semantic_identity: &'a str,
}

/// Canonical direction carried by a V1 device FFI declaration.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceFfiDirectionV1 {
    Import,
    Export,
}

impl DeviceFfiDirectionV1 {
    pub const fn tag(self) -> u16 {
        match self {
            Self::Import => DEVICE_FFI_DIRECTION_IMPORT_V1,
            Self::Export => DEVICE_FFI_DIRECTION_EXPORT_V1,
        }
    }
}

/// Canonical scalar types allowed in a V1 device FFI physical ABI.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceFfiScalarTypeV1 {
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

/// Canonical AMDGPU address spaces allowed in a V1 device FFI pointer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceFfiAddressSpaceV1 {
    Constant,
    Global,
    Private,
    Workgroup,
}

/// Constness carried by a V1 device FFI pointer wrapper.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceFfiPointerAccessV1 {
    Const,
    Mut,
}

/// Parsed canonical V1 device pointer ABI component.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeviceFfiPointerTypeV1 {
    access: DeviceFfiPointerAccessV1,
    address_space: DeviceFfiAddressSpaceV1,
    element: DeviceFfiScalarTypeV1,
}

impl DeviceFfiPointerTypeV1 {
    pub const fn access(self) -> DeviceFfiPointerAccessV1 {
        self.access
    }

    pub const fn address_space(self) -> DeviceFfiAddressSpaceV1 {
        self.address_space
    }

    pub const fn element(self) -> DeviceFfiScalarTypeV1 {
        self.element
    }
}

/// Parsed canonical physical component in a V1 device FFI ABI.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceFfiPhysicalTypeV1 {
    Scalar(DeviceFfiScalarTypeV1),
    Pointer(DeviceFfiPointerTypeV1),
}

/// Parsed canonical return component in a V1 device FFI ABI.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceFfiPhysicalResultV1 {
    Unit,
    Value(DeviceFfiPhysicalTypeV1),
}

/// Parsed, bounded canonical V1 device FFI physical ABI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceFfiPhysicalAbiV1 {
    arguments: Vec<DeviceFfiPhysicalTypeV1>,
    result: DeviceFfiPhysicalResultV1,
}

impl DeviceFfiPhysicalAbiV1 {
    pub fn arguments(&self) -> &[DeviceFfiPhysicalTypeV1] {
        &self.arguments
    }

    pub const fn result(&self) -> DeviceFfiPhysicalResultV1 {
        self.result
    }
}

/// Canonical declared effects allowed in a V1 device FFI contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceFfiEffectV1 {
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

impl DeviceFfiEffectV1 {
    pub const fn as_str(self) -> &'static str {
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

/// Parsed, sorted canonical V1 device FFI effects. An empty slice represents `none`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceFfiEffectsV1 {
    effects: Vec<DeviceFfiEffectV1>,
}

impl DeviceFfiEffectsV1 {
    pub fn effects(&self) -> &[DeviceFfiEffectV1] {
        &self.effects
    }

    pub fn is_none(&self) -> bool {
        self.effects.is_empty()
    }
}

/// Shared parsed grammar retained after validating one contract's coupled fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedDeviceFfiContractGrammarV1 {
    physical_abi: DeviceFfiPhysicalAbiV1,
    effects: DeviceFfiEffectsV1,
}

impl ValidatedDeviceFfiContractGrammarV1 {
    pub const fn physical_abi(&self) -> &DeviceFfiPhysicalAbiV1 {
        &self.physical_abi
    }

    pub const fn effects(&self) -> &DeviceFfiEffectsV1 {
        &self.effects
    }
}

/// Failure to parse the shared canonical V1 device FFI grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DeviceFfiGrammarError {
    InvalidDirection,
    InvalidSymbol,
    InvalidPhysicalAbi,
    TooManyPhysicalAbiArguments,
    InvalidEffects,
    EffectAbiMismatch(DeviceFfiEffectV1),
}

impl fmt::Display for DeviceFfiGrammarError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDirection => formatter.write_str("noncanonical device FFI direction"),
            Self::InvalidSymbol => formatter.write_str("invalid external symbol"),
            Self::InvalidPhysicalAbi => {
                formatter.write_str("physical ABI is empty, oversized, or noncanonical")
            }
            Self::TooManyPhysicalAbiArguments => formatter.write_str("too many physical arguments"),
            Self::InvalidEffects => {
                formatter.write_str("effects are empty, oversized, or noncanonical")
            }
            Self::EffectAbiMismatch(effect) => write!(
                formatter,
                "effect `{}` has no compatible physical pointer argument",
                effect.as_str()
            ),
        }
    }
}

impl Error for DeviceFfiGrammarError {}

/// Parses the exact canonical decimal direction spelling used in V1 markers.
pub fn parse_device_ffi_direction_v1(
    direction: &str,
) -> Result<DeviceFfiDirectionV1, DeviceFfiGrammarError> {
    match direction {
        "1" => Ok(DeviceFfiDirectionV1::Import),
        "2" => Ok(DeviceFfiDirectionV1::Export),
        _ => Err(DeviceFfiGrammarError::InvalidDirection),
    }
}

/// Validates the exact canonical grammar for an external FFI symbol.
pub fn validate_device_ffi_symbol_v1(symbol: &str) -> Result<(), DeviceFfiGrammarError> {
    let mut bytes = symbol.bytes();
    let valid = !symbol.is_empty()
        && symbol.len() <= MAX_DEVICE_FFI_SYMBOL_BYTES_V1
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'.' | b'$'))
        && bytes.all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$' | b'@' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(DeviceFfiGrammarError::InvalidSymbol)
    }
}

/// Parses the exact canonical V1 physical ABI grammar.
pub fn parse_device_ffi_physical_abi_v1(
    abi: &str,
) -> Result<DeviceFfiPhysicalAbiV1, DeviceFfiGrammarError> {
    if abi.is_empty() || abi.len() > MAX_DEVICE_FFI_PHYSICAL_ABI_BYTES_V1 {
        return Err(DeviceFfiGrammarError::InvalidPhysicalAbi);
    }
    let mut rest = abi
        .strip_prefix("C(")
        .ok_or(DeviceFfiGrammarError::InvalidPhysicalAbi)?;
    let mut arguments = Vec::new();
    if let Some(after) = rest.strip_prefix(")->") {
        rest = after;
    } else {
        loop {
            let (argument, after) = consume_device_ffi_physical_type_v1(rest)
                .ok_or(DeviceFfiGrammarError::InvalidPhysicalAbi)?;
            arguments.push(argument);
            if arguments.len() > MAX_DEVICE_FFI_ARGUMENTS_V1 {
                return Err(DeviceFfiGrammarError::TooManyPhysicalAbiArguments);
            }
            if let Some(after) = after.strip_prefix(',') {
                rest = after;
                continue;
            }
            rest = after
                .strip_prefix(")->")
                .ok_or(DeviceFfiGrammarError::InvalidPhysicalAbi)?;
            break;
        }
    }
    let result = if rest == "unit[size=0,align=1]" {
        DeviceFfiPhysicalResultV1::Unit
    } else {
        let (result, trailing) = consume_device_ffi_physical_type_v1(rest)
            .ok_or(DeviceFfiGrammarError::InvalidPhysicalAbi)?;
        if !trailing.is_empty() {
            return Err(DeviceFfiGrammarError::InvalidPhysicalAbi);
        }
        DeviceFfiPhysicalResultV1::Value(result)
    };
    Ok(DeviceFfiPhysicalAbiV1 { arguments, result })
}

/// Parses the exact sorted canonical V1 effect grammar.
pub fn parse_device_ffi_effects_v1(
    effects: &str,
) -> Result<DeviceFfiEffectsV1, DeviceFfiGrammarError> {
    if effects.is_empty() || effects.len() > MAX_DEVICE_FFI_EFFECT_BYTES_V1 {
        return Err(DeviceFfiGrammarError::InvalidEffects);
    }
    if effects == "none" {
        return Ok(DeviceFfiEffectsV1 {
            effects: Vec::new(),
        });
    }
    let mut parsed = Vec::new();
    let mut previous = None;
    for spelling in effects.split(',') {
        let effect =
            parse_device_ffi_effect_v1(spelling).ok_or(DeviceFfiGrammarError::InvalidEffects)?;
        if previous.is_some_and(|previous: &str| previous >= spelling) {
            return Err(DeviceFfiGrammarError::InvalidEffects);
        }
        parsed.push(effect);
        previous = Some(spelling);
    }
    Ok(DeviceFfiEffectsV1 { effects: parsed })
}

/// Checks declared effects against parsed pointer arguments in the physical ABI.
pub fn validate_device_ffi_effect_abi_v1(
    effects: &DeviceFfiEffectsV1,
    abi: &DeviceFfiPhysicalAbiV1,
) -> Result<(), DeviceFfiGrammarError> {
    for effect in &effects.effects {
        let requirement = match effect {
            DeviceFfiEffectV1::AtomicGlobal | DeviceFfiEffectV1::WriteGlobal => Some((
                DeviceFfiAddressSpaceV1::Global,
                DeviceFfiPointerAccessV1::Mut,
            )),
            DeviceFfiEffectV1::AtomicWorkgroup | DeviceFfiEffectV1::WriteWorkgroup => Some((
                DeviceFfiAddressSpaceV1::Workgroup,
                DeviceFfiPointerAccessV1::Mut,
            )),
            DeviceFfiEffectV1::WritePrivate => Some((
                DeviceFfiAddressSpaceV1::Private,
                DeviceFfiPointerAccessV1::Mut,
            )),
            DeviceFfiEffectV1::ReadConstant => Some((
                DeviceFfiAddressSpaceV1::Constant,
                DeviceFfiPointerAccessV1::Const,
            )),
            DeviceFfiEffectV1::ReadGlobal => Some((
                DeviceFfiAddressSpaceV1::Global,
                DeviceFfiPointerAccessV1::Const,
            )),
            DeviceFfiEffectV1::ReadPrivate => Some((
                DeviceFfiAddressSpaceV1::Private,
                DeviceFfiPointerAccessV1::Const,
            )),
            DeviceFfiEffectV1::ReadWorkgroup => Some((
                DeviceFfiAddressSpaceV1::Workgroup,
                DeviceFfiPointerAccessV1::Const,
            )),
            DeviceFfiEffectV1::BarrierWorkgroup => None,
        };
        let Some((address_space, minimum_access)) = requirement else {
            continue;
        };
        let compatible = abi.arguments.iter().any(|argument| {
            let DeviceFfiPhysicalTypeV1::Pointer(pointer) = argument else {
                return false;
            };
            pointer.address_space == address_space
                && (minimum_access == DeviceFfiPointerAccessV1::Const
                    || pointer.access == DeviceFfiPointerAccessV1::Mut)
        });
        if !compatible {
            return Err(DeviceFfiGrammarError::EffectAbiMismatch(*effect));
        }
    }
    Ok(())
}

/// Validates and parses the coupled symbol, physical-ABI, effects, and effect-ABI grammar.
pub fn validate_device_ffi_contract_grammar_v1(
    symbol: &str,
    physical_abi: &str,
    effects: &str,
) -> Result<ValidatedDeviceFfiContractGrammarV1, DeviceFfiGrammarError> {
    validate_device_ffi_symbol_v1(symbol)?;
    let physical_abi = parse_device_ffi_physical_abi_v1(physical_abi)?;
    let effects = parse_device_ffi_effects_v1(effects)?;
    validate_device_ffi_effect_abi_v1(&effects, &physical_abi)?;
    Ok(ValidatedDeviceFfiContractGrammarV1 {
        physical_abi,
        effects,
    })
}

fn consume_device_ffi_physical_type_v1(input: &str) -> Option<(DeviceFfiPhysicalTypeV1, &str)> {
    if let Some((scalar, rest)) = consume_device_ffi_scalar_layout_v1(input) {
        return Some((DeviceFfiPhysicalTypeV1::Scalar(scalar), rest));
    }

    let (access, rest) = if let Some(rest) = input.strip_prefix("const_ptr<") {
        (DeviceFfiPointerAccessV1::Const, rest)
    } else if let Some(rest) = input.strip_prefix("mut_ptr<") {
        (DeviceFfiPointerAccessV1::Mut, rest)
    } else {
        return None;
    };
    let (address_space, rest, suffix) = if let Some(rest) = rest.strip_prefix("constant,") {
        (
            DeviceFfiAddressSpaceV1::Constant,
            rest,
            ">[size=8,align=8,as=constant]",
        )
    } else if let Some(rest) = rest.strip_prefix("global,") {
        (
            DeviceFfiAddressSpaceV1::Global,
            rest,
            ">[size=8,align=8,as=global]",
        )
    } else if let Some(rest) = rest.strip_prefix("private,") {
        (
            DeviceFfiAddressSpaceV1::Private,
            rest,
            ">[size=8,align=8,as=private]",
        )
    } else if let Some(rest) = rest.strip_prefix("workgroup,") {
        (
            DeviceFfiAddressSpaceV1::Workgroup,
            rest,
            ">[size=8,align=8,as=workgroup]",
        )
    } else {
        return None;
    };
    if access == DeviceFfiPointerAccessV1::Mut && address_space == DeviceFfiAddressSpaceV1::Constant
    {
        return None;
    }
    let (element, rest) = consume_device_ffi_scalar_name_v1(rest)?;
    let rest = rest.strip_prefix(suffix)?;
    Some((
        DeviceFfiPhysicalTypeV1::Pointer(DeviceFfiPointerTypeV1 {
            access,
            address_space,
            element,
        }),
        rest,
    ))
}

fn consume_device_ffi_scalar_layout_v1(input: &str) -> Option<(DeviceFfiScalarTypeV1, &str)> {
    for (spelling, scalar) in [
        ("i8[size=1,align=1]", DeviceFfiScalarTypeV1::I8),
        ("u8[size=1,align=1]", DeviceFfiScalarTypeV1::U8),
        ("i16[size=2,align=2]", DeviceFfiScalarTypeV1::I16),
        ("u16[size=2,align=2]", DeviceFfiScalarTypeV1::U16),
        ("i32[size=4,align=4]", DeviceFfiScalarTypeV1::I32),
        ("u32[size=4,align=4]", DeviceFfiScalarTypeV1::U32),
        ("i64[size=8,align=8]", DeviceFfiScalarTypeV1::I64),
        ("u64[size=8,align=8]", DeviceFfiScalarTypeV1::U64),
        ("f32[size=4,align=4]", DeviceFfiScalarTypeV1::F32),
        ("f64[size=8,align=8]", DeviceFfiScalarTypeV1::F64),
    ] {
        if let Some(rest) = input.strip_prefix(spelling) {
            return Some((scalar, rest));
        }
    }
    None
}

fn consume_device_ffi_scalar_name_v1(input: &str) -> Option<(DeviceFfiScalarTypeV1, &str)> {
    for (spelling, scalar) in [
        ("i8", DeviceFfiScalarTypeV1::I8),
        ("u8", DeviceFfiScalarTypeV1::U8),
        ("i16", DeviceFfiScalarTypeV1::I16),
        ("u16", DeviceFfiScalarTypeV1::U16),
        ("i32", DeviceFfiScalarTypeV1::I32),
        ("u32", DeviceFfiScalarTypeV1::U32),
        ("i64", DeviceFfiScalarTypeV1::I64),
        ("u64", DeviceFfiScalarTypeV1::U64),
        ("f32", DeviceFfiScalarTypeV1::F32),
        ("f64", DeviceFfiScalarTypeV1::F64),
    ] {
        if let Some(rest) = input.strip_prefix(spelling) {
            return Some((scalar, rest));
        }
    }
    None
}

fn parse_device_ffi_effect_v1(spelling: &str) -> Option<DeviceFfiEffectV1> {
    match spelling {
        "atomic_global" => Some(DeviceFfiEffectV1::AtomicGlobal),
        "atomic_workgroup" => Some(DeviceFfiEffectV1::AtomicWorkgroup),
        "barrier_workgroup" => Some(DeviceFfiEffectV1::BarrierWorkgroup),
        "read_constant" => Some(DeviceFfiEffectV1::ReadConstant),
        "read_global" => Some(DeviceFfiEffectV1::ReadGlobal),
        "read_private" => Some(DeviceFfiEffectV1::ReadPrivate),
        "read_workgroup" => Some(DeviceFfiEffectV1::ReadWorkgroup),
        "write_global" => Some(DeviceFfiEffectV1::WriteGlobal),
        "write_private" => Some(DeviceFfiEffectV1::WritePrivate),
        "write_workgroup" => Some(DeviceFfiEffectV1::WriteWorkgroup),
        _ => None,
    }
}

/// Derives the collision-resistant identity for one canonical FFI declaration.
///
/// The fixed V1 safety policy (`nounwind` and `nopanic`) is part of the hash
/// domain. Callers must validate field grammar and bounds before derivation.
pub fn derive_device_ffi_contract_id_v1(
    fields: DeviceFfiContractFieldsV1<'_>,
) -> DeviceFfiContractIdV1 {
    let mut digest = Sha256::new();
    digest.update(DEVICE_FFI_CONTRACT_DOMAIN_V1);
    digest.update(fields.direction.to_le_bytes());
    update_field(&mut digest, fields.symbol.as_bytes());
    update_field(&mut digest, fields.calling_convention.as_bytes());
    digest.update(fields.code_object_version.to_le_bytes());
    update_field(&mut digest, fields.target.as_bytes());
    update_field(&mut digest, fields.physical_abi.as_bytes());
    update_field(&mut digest, fields.effects.as_bytes());
    update_field(&mut digest, fields.semantic_identity.as_bytes());
    update_field(&mut digest, b"nounwind;nopanic");
    DeviceFfiContractIdV1(digest.finalize().into())
}

/// Encodes a bounded compiler marker. Fields use a grammar that excludes `|`.
pub fn device_ffi_marker_v1(
    id: DeviceFfiContractIdV1,
    fields: DeviceFfiContractFieldsV1<'_>,
) -> String {
    format!(
        "{DEVICE_FFI_MARKER_PREFIX_V1}{}|{}|{}|{}|{}|{}|{}|{}|{}",
        fields.direction,
        id.to_hex(),
        fields.symbol,
        fields.calling_convention,
        fields.code_object_version,
        fields.target,
        fields.physical_abi,
        fields.effects,
        fields.semantic_identity,
    )
}

/// Error returned for a noncanonical binding ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingIdError {
    reason: &'static str,
}

impl fmt::Display for BindingIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.reason)
    }
}

impl Error for BindingIdError {}

/// Derives one crate binding from rustc's crate name and ordered `-C metadata`
/// values. Field lengths and the metadata count make the encoding unambiguous.
pub fn derive_crate_binding_id_v1<'a>(
    crate_name: &str,
    metadata: impl IntoIterator<Item = &'a str>,
) -> CrateBindingIdV1 {
    let metadata = metadata.into_iter().collect::<Vec<_>>();
    let mut digest = Sha256::new();
    digest.update(CRATE_BINDING_DOMAIN_V1);
    update_field(&mut digest, crate_name.as_bytes());
    digest.update((metadata.len() as u64).to_le_bytes());
    for value in metadata {
        update_field(&mut digest, value.as_bytes());
    }
    CrateBindingIdV1(digest.finalize().into())
}

/// Derives one kernel binding from its compilation unit, typed profile, and
/// source/export names.
pub fn derive_kernel_binding_id_v1(
    crate_binding: CrateBindingIdV1,
    profile_tag: &str,
    logical_name: &str,
    export_name: &str,
) -> KernelBindingIdV1 {
    let mut digest = Sha256::new();
    digest.update(KERNEL_BINDING_DOMAIN_V1);
    update_field(&mut digest, &crate_binding.as_bytes());
    update_field(&mut digest, profile_tag.as_bytes());
    update_field(&mut digest, logical_name.as_bytes());
    update_field(&mut digest, export_name.as_bytes());
    KernelBindingIdV1(digest.finalize().into())
}

/// Returns the private artifact pointer accessor for one exact kernel binding.
pub fn artifact_pointer_symbol_v1(binding: KernelBindingIdV1) -> String {
    format!("{ARTIFACT_ACCESSOR_PREFIX_V1}{}_ptr", binding.to_hex())
}

/// Returns the private artifact length accessor for one exact kernel binding.
pub fn artifact_length_symbol_v1(binding: KernelBindingIdV1) -> String {
    format!("{ARTIFACT_ACCESSOR_PREFIX_V1}{}_len", binding.to_hex())
}

/// Returns the reserved host symbol for one exact registered kernel function.
pub fn host_kernel_symbol_v1(binding: KernelBindingIdV1) -> String {
    format!("{HOST_KERNEL_PREFIX_V1}{}", binding.to_hex())
}

fn update_field(digest: &mut Sha256, field: &[u8]) {
    digest.update((field.len() as u64).to_le_bytes());
    digest.update(field);
}

fn parse_binding_hex(value: &str) -> Result<[u8; BINDING_ID_BYTES], BindingIdError> {
    if value.len() != BINDING_ID_HEX_BYTES {
        return Err(BindingIdError {
            reason: "binding ID must contain exactly 64 lowercase hexadecimal bytes",
        });
    }
    let mut decoded = [0_u8; BINDING_ID_BYTES];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = decode_hex_nibble(pair[0]).ok_or(BindingIdError {
            reason: "binding ID must contain exactly 64 lowercase hexadecimal bytes",
        })?;
        let low = decode_hex_nibble(pair[1]).ok_or(BindingIdError {
            reason: "binding ID must contain exactly 64 lowercase hexadecimal bytes",
        })?;
        decoded[index] = (high << 4) | low;
    }
    Ok(decoded)
}

fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn encode_binding_hex(bytes: [u8; BINDING_ID_BYTES]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(BINDING_ID_HEX_BYTES);
    for byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_registration_v1_values_are_stable() {
        assert_eq!(KERNEL_REGISTRATION_MAGIC.to_le_bytes(), *b"FE2O3KRN");
        assert_eq!(KERNEL_REGISTRATION_VERSION_V1, 1);
        assert_eq!(KERNEL_REGISTRATION_VERSION_V2, 2);
        assert_eq!(KERNEL_REGISTRATION_KIND_KERNEL, 1);
        assert_eq!(KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1, 2);
        assert_eq!(KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2, 3);
        assert_ne!(
            KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1,
            KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2
        );
        assert_eq!(KERNEL_REGISTRATION_V1_FIELD_COUNT, 6);
        assert_eq!(KERNEL_REGISTRATION_V2_FIELD_COUNT, 8);
    }

    #[test]
    fn binding_derivation_is_ordered_domain_separated_and_round_trips() {
        let first = derive_crate_binding_id_v1("same", ["alpha", "beta"]);
        let reordered = derive_crate_binding_id_v1("same", ["beta", "alpha"]);
        let different_crate = derive_crate_binding_id_v1("other", ["alpha", "beta"]);
        assert_ne!(first, reordered);
        assert_ne!(first, different_crate);
        assert_eq!(CrateBindingIdV1::from_hex(&first.to_hex()).unwrap(), first);

        let opaque_kernel =
            derive_kernel_binding_id_v1(first, TYPED_VECADD_F32_PROFILE_TAG_V1, "vecadd", "vecadd");
        let kernel = derive_kernel_binding_id_v1(
            first,
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            "vecadd",
            "vecadd",
        );
        assert_ne!(opaque_kernel, kernel);
        assert_ne!(kernel.as_bytes(), first.as_bytes());
        assert_eq!(
            KernelBindingIdV1::from_hex(&kernel.to_hex()).unwrap(),
            kernel
        );
    }

    #[test]
    fn generated_symbols_include_the_full_kernel_binding() {
        let crate_id = derive_crate_binding_id_v1("crate", ["metadata"]);
        let kernel = derive_kernel_binding_id_v1(
            crate_id,
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            "vecadd",
            "vecadd",
        );
        let hex = kernel.to_hex();

        assert_eq!(
            artifact_pointer_symbol_v1(kernel),
            format!("__fe2o3_artifact_v1_{hex}_ptr")
        );
        assert_eq!(
            artifact_length_symbol_v1(kernel),
            format!("__fe2o3_artifact_v1_{hex}_len")
        );
        assert_eq!(
            host_kernel_symbol_v1(kernel),
            format!("__fe2o3_host_kernel_v1_{hex}")
        );
    }

    #[test]
    fn parser_rejects_noncanonical_binding_ids() {
        for invalid in [
            "",
            "00",
            "000000000000000000000000000000000000000000000000000000000000000",
            "00000000000000000000000000000000000000000000000000000000000000000",
            "000000000000000000000000000000000000000000000000000000000000000G",
            "000000000000000000000000000000000000000000000000000000000000000A",
        ] {
            assert!(CrateBindingIdV1::from_hex(invalid).is_err(), "{invalid}");
            assert!(KernelBindingIdV1::from_hex(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn device_ffi_registration_and_identity_are_stable_and_bound() {
        assert_eq!(DEVICE_FFI_REGISTRATION_MAGIC_V1.to_le_bytes(), *b"FE2O3FFI");
        assert_eq!(DEVICE_FFI_REGISTRATION_VERSION_V1, 1);
        assert_eq!(DEVICE_FFI_REGISTRATION_V1_FIELD_COUNT, 12);
        let fields = DeviceFfiContractFieldsV1 {
            direction: DEVICE_FFI_DIRECTION_EXPORT_V1,
            symbol: "helper",
            calling_convention: "C",
            code_object_version: 5,
            target: "gfx942",
            physical_abi: "C(u32[size=4,align=4])->unit[size=0,align=1]",
            effects: "none",
            semantic_identity: "1111111111111111111111111111111111111111111111111111111111111111",
        };
        let id = derive_device_ffi_contract_id_v1(fields);
        assert_eq!(
            id.to_hex(),
            "7e5c3173ef0a8ba24dba7e993872bfd8053cbf1db1ff221c57c48b49e7824da4"
        );
        let marker = device_ffi_marker_v1(id, fields);
        assert!(marker.starts_with("__fe2o3_device_ffi_v1|2|"));
        assert!(marker.contains(&id.to_hex()));

        let changed = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
            ..fields
        });
        assert_ne!(id, changed);
        assert_eq!(DeviceFfiContractIdV1::from_hex(&id.to_hex()).unwrap(), id);
    }

    #[test]
    fn shared_device_ffi_grammar_parses_real_g4_compatible_fields() {
        let abi = concat!(
            "C(const_ptr<constant,u32>[size=8,align=8,as=constant],",
            "mut_ptr<global,f32>[size=8,align=8,as=global])->",
            "u64[size=8,align=8]"
        );
        let parsed = validate_device_ffi_contract_grammar_v1(
            "external_mix.v1",
            abi,
            "atomic_global,read_constant,read_global,write_global",
        )
        .unwrap();

        assert_eq!(parsed.physical_abi().arguments().len(), 2);
        assert_eq!(
            parsed.physical_abi().result(),
            DeviceFfiPhysicalResultV1::Value(DeviceFfiPhysicalTypeV1::Scalar(
                DeviceFfiScalarTypeV1::U64
            ))
        );
        assert_eq!(parsed.effects().effects().len(), 4);
        assert!(!parsed.effects().is_none());
    }

    #[test]
    fn shared_device_ffi_grammar_rejects_noncanonical_spellings() {
        assert_eq!(
            parse_device_ffi_direction_v1("1"),
            Ok(DeviceFfiDirectionV1::Import)
        );
        assert_eq!(
            parse_device_ffi_direction_v1("2"),
            Ok(DeviceFfiDirectionV1::Export)
        );
        for direction in ["", "0", "01", "02", "+1", " 1", "1 ", "3"] {
            assert_eq!(
                parse_device_ffi_direction_v1(direction),
                Err(DeviceFfiGrammarError::InvalidDirection),
                "{direction:?}"
            );
        }

        for symbol in ["", "9bad", "bad symbol", "x\n", &"x".repeat(129)] {
            assert_eq!(
                validate_device_ffi_symbol_v1(symbol),
                Err(DeviceFfiGrammarError::InvalidSymbol),
                "{symbol:?}"
            );
        }

        for abi in [
            "",
            "C( )->unit[size=0,align=1]",
            "C(u32[size=4,align=8])->unit[size=0,align=1]",
            "C(mut_ptr<constant,u32>[size=8,align=8,as=constant])->unit[size=0,align=1]",
            "C(ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]",
            "C()->void",
            "C()->unit[size=0,align=1]trailing",
        ] {
            assert_eq!(
                parse_device_ffi_physical_abi_v1(abi),
                Err(DeviceFfiGrammarError::InvalidPhysicalAbi),
                "{abi}"
            );
        }

        for effects in [
            "",
            "None",
            "read_global,atomic_global",
            "read_global,read_global",
            "read-global",
            "none,read_global",
        ] {
            assert_eq!(
                parse_device_ffi_effects_v1(effects),
                Err(DeviceFfiGrammarError::InvalidEffects),
                "{effects}"
            );
        }
    }

    #[test]
    fn effect_abi_requires_compatible_pointer_arguments_not_return_values() {
        let global_const = concat!(
            "C(const_ptr<global,u32>[size=8,align=8,as=global])->",
            "unit[size=0,align=1]"
        );
        assert!(
            validate_device_ffi_contract_grammar_v1("reader", global_const, "read_global").is_ok()
        );
        assert_eq!(
            validate_device_ffi_contract_grammar_v1("writer", global_const, "write_global"),
            Err(DeviceFfiGrammarError::EffectAbiMismatch(
                DeviceFfiEffectV1::WriteGlobal
            ))
        );

        let pointer_result_only = "C()->const_ptr<global,u32>[size=8,align=8,as=global]";
        assert_eq!(
            validate_device_ffi_contract_grammar_v1(
                "return_only",
                pointer_result_only,
                "read_global"
            ),
            Err(DeviceFfiGrammarError::EffectAbiMismatch(
                DeviceFfiEffectV1::ReadGlobal
            ))
        );
    }

    #[test]
    fn physical_abi_argument_bound_is_shared_and_exact() {
        let argument = "u32[size=4,align=4]";
        let at_bound = format!(
            "C({})->unit[size=0,align=1]",
            vec![argument; MAX_DEVICE_FFI_ARGUMENTS_V1].join(",")
        );
        assert_eq!(
            parse_device_ffi_physical_abi_v1(&at_bound)
                .unwrap()
                .arguments()
                .len(),
            MAX_DEVICE_FFI_ARGUMENTS_V1
        );
        let over_bound = format!("C({argument},{}", &at_bound[2..]);
        assert_eq!(
            parse_device_ffi_physical_abi_v1(&over_bound),
            Err(DeviceFfiGrammarError::TooManyPhysicalAbiArguments)
        );
    }
}
