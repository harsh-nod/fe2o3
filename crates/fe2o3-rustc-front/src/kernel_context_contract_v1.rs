//! Canonical frontend metadata for compiler-issued logical kernel context.
//!
//! This record is an inert source declaration. Its item identities commit to
//! macro-generated names, but only the rustc collector can authenticate those
//! names against exact compiler items and the generated registration graph.

use std::collections::BTreeSet;
use std::fmt;
use std::str;

use sha2::{Digest as _, Sha256};

pub const KERNEL_CONTEXT_FRONTEND_CONTRACT_MAGIC_V1: [u8; 8] = *b"FE2O3KC\0";
pub const KERNEL_CONTEXT_FRONTEND_CONTRACT_VERSION_V1: u16 = 1;
pub const KERNEL_CONTEXT_FRONTEND_REGISTRATION_PREFIX_V1: &str =
    "__fe2o3_kernel_context_contract_v1_";
pub const KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1: u64 = u64::from_le_bytes(*b"FE2O3KCA");
pub const KERNEL_CONTEXT_FRONTEND_REGISTRATION_VERSION_V1: u16 = 1;
pub const KERNEL_CONTEXT_FRONTEND_REGISTRATION_KIND_V1: u16 = 1;

pub const KERNEL_CONTEXT_SOURCE_ORDINAL_V1: u16 = 0;
pub const KERNEL_CONTEXT_REQUIRED_ISSUANCE_COUNT_V1: u16 = 1;
pub const KERNEL_CONTEXT_ISSUANCE_DIAGNOSTIC_IDENTITY_V1: &str =
    "fe2o3_device_kernel_context_issue_v1";
pub const KERNEL_CONTEXT_ISSUANCE_DIAGNOSTIC_VERSION_V1: u16 = 1;
pub const MAX_KERNEL_CONTEXT_GENERATED_ITEM_NAME_BYTES_V1: usize = 512;

const HEADER_BYTES_V1: usize = 32;
const ITEM_BYTES_EXCLUDING_NAME_V1: usize = 40;
const ITEM_COUNT_V1: usize = 3;
const ITEM_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3/kernel-context-generated-item/v1";

pub const MAX_KERNEL_CONTEXT_FRONTEND_CONTRACT_BYTES_V1: usize = HEADER_BYTES_V1
    + KERNEL_CONTEXT_ISSUANCE_DIAGNOSTIC_IDENTITY_V1.len()
    + ITEM_COUNT_V1
        * (ITEM_BYTES_EXCLUDING_NAME_V1 + MAX_KERNEL_CONTEXT_GENERATED_ITEM_NAME_BYTES_V1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum KernelContextGeneratedItemRoleV1 {
    PhysicalKernelRoot = 1,
    LogicalHelper = 2,
    NominalKernelMarker = 3,
}

impl KernelContextGeneratedItemRoleV1 {
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::PhysicalKernelRoot => "physical-kernel-root",
            Self::LogicalHelper => "logical-helper",
            Self::NominalKernelMarker => "nominal-kernel-marker",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct KernelContextGeneratedItemIdentityV1([u8; 32]);

impl KernelContextGeneratedItemIdentityV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, KernelContextFrontendContractValidationErrorV1> {
        if bytes == [0; 32] {
            return Err(KernelContextFrontendContractValidationErrorV1::ZeroItemIdentity);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelContextGeneratedItemV1 {
    role: KernelContextGeneratedItemRoleV1,
    identity: KernelContextGeneratedItemIdentityV1,
    name: String,
}

impl KernelContextGeneratedItemV1 {
    pub fn new(
        role: KernelContextGeneratedItemRoleV1,
        identity: KernelContextGeneratedItemIdentityV1,
        name: impl Into<String>,
    ) -> Result<Self, KernelContextFrontendContractValidationErrorV1> {
        let name = name.into();
        validate_generated_item_name(&name)?;
        Ok(Self {
            role,
            identity,
            name,
        })
    }

    pub fn for_generated_name(
        role: KernelContextGeneratedItemRoleV1,
        name: impl Into<String>,
    ) -> Result<Self, KernelContextFrontendContractValidationErrorV1> {
        let name = name.into();
        validate_generated_item_name(&name)?;
        let identity = derive_kernel_context_generated_item_identity_v1(role, &name);
        Ok(Self {
            role,
            identity,
            name,
        })
    }

    pub const fn role(&self) -> KernelContextGeneratedItemRoleV1 {
        self.role
    }

    pub const fn identity(&self) -> KernelContextGeneratedItemIdentityV1 {
        self.identity
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelContextIssuanceDiagnosticV1 {
    identity: String,
    version: u16,
}

impl KernelContextIssuanceDiagnosticV1 {
    pub fn new(
        identity: impl Into<String>,
        version: u16,
    ) -> Result<Self, KernelContextFrontendContractValidationErrorV1> {
        let identity = identity.into();
        if identity != KERNEL_CONTEXT_ISSUANCE_DIAGNOSTIC_IDENTITY_V1 {
            return Err(KernelContextFrontendContractValidationErrorV1::IssuanceDiagnosticIdentity);
        }
        if version != KERNEL_CONTEXT_ISSUANCE_DIAGNOSTIC_VERSION_V1 {
            return Err(
                KernelContextFrontendContractValidationErrorV1::IssuanceDiagnosticVersion(version),
            );
        }
        Ok(Self { identity, version })
    }

    pub fn exact() -> Self {
        Self {
            identity: KERNEL_CONTEXT_ISSUANCE_DIAGNOSTIC_IDENTITY_V1.to_owned(),
            version: KERNEL_CONTEXT_ISSUANCE_DIAGNOSTIC_VERSION_V1,
        }
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub const fn version(&self) -> u16 {
        self.version
    }
}

/// Inert macro-to-compiler declaration for one logical `KernelContext` input.
///
/// This value does not authenticate a rustc item and grants no compilation,
/// lowering, artifact, load, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelContextFrontendContractV1 {
    items: [KernelContextGeneratedItemV1; ITEM_COUNT_V1],
    context_source_ordinal: u16,
    issuance_diagnostic: KernelContextIssuanceDiagnosticV1,
    required_issuance_count: u16,
}

impl KernelContextFrontendContractV1 {
    pub fn new(
        mut items: [KernelContextGeneratedItemV1; ITEM_COUNT_V1],
        context_source_ordinal: u16,
        issuance_diagnostic: KernelContextIssuanceDiagnosticV1,
        required_issuance_count: u16,
    ) -> Result<Self, KernelContextFrontendContractValidationErrorV1> {
        if context_source_ordinal != KERNEL_CONTEXT_SOURCE_ORDINAL_V1 {
            return Err(
                KernelContextFrontendContractValidationErrorV1::ContextSourceOrdinal(
                    context_source_ordinal,
                ),
            );
        }
        if required_issuance_count != KERNEL_CONTEXT_REQUIRED_ISSUANCE_COUNT_V1 {
            return Err(
                KernelContextFrontendContractValidationErrorV1::RequiredIssuanceCount(
                    required_issuance_count,
                ),
            );
        }

        items.sort_unstable_by_key(KernelContextGeneratedItemV1::role);
        if let Some(pair) = items.windows(2).find(|pair| pair[0].role == pair[1].role) {
            return Err(
                KernelContextFrontendContractValidationErrorV1::DuplicateItemRole(pair[0].role),
            );
        }
        for (actual, expected) in items.iter().map(|item| item.role).zip([
            KernelContextGeneratedItemRoleV1::PhysicalKernelRoot,
            KernelContextGeneratedItemRoleV1::LogicalHelper,
            KernelContextGeneratedItemRoleV1::NominalKernelMarker,
        ]) {
            if actual != expected {
                return Err(
                    KernelContextFrontendContractValidationErrorV1::MissingItemRole(expected),
                );
            }
        }

        let mut names = BTreeSet::new();
        for item in &items {
            if !names.insert(item.name.as_str()) {
                return Err(KernelContextFrontendContractValidationErrorV1::DuplicateItemName);
            }
            let expected = derive_kernel_context_generated_item_identity_v1(item.role, &item.name);
            if item.identity != expected {
                return Err(
                    KernelContextFrontendContractValidationErrorV1::ItemIdentityNameMismatch(
                        item.role,
                    ),
                );
            }
        }

        Ok(Self {
            items,
            context_source_ordinal,
            issuance_diagnostic,
            required_issuance_count,
        })
    }

    pub fn for_generated_kernel(
        physical_kernel_root_name: impl Into<String>,
        logical_helper_name: impl Into<String>,
        nominal_kernel_marker_name: impl Into<String>,
    ) -> Result<Self, KernelContextFrontendContractValidationErrorV1> {
        Self::new(
            [
                KernelContextGeneratedItemV1::for_generated_name(
                    KernelContextGeneratedItemRoleV1::PhysicalKernelRoot,
                    physical_kernel_root_name,
                )?,
                KernelContextGeneratedItemV1::for_generated_name(
                    KernelContextGeneratedItemRoleV1::LogicalHelper,
                    logical_helper_name,
                )?,
                KernelContextGeneratedItemV1::for_generated_name(
                    KernelContextGeneratedItemRoleV1::NominalKernelMarker,
                    nominal_kernel_marker_name,
                )?,
            ],
            KERNEL_CONTEXT_SOURCE_ORDINAL_V1,
            KernelContextIssuanceDiagnosticV1::exact(),
            KERNEL_CONTEXT_REQUIRED_ISSUANCE_COUNT_V1,
        )
    }

    pub fn physical_kernel_root(&self) -> &KernelContextGeneratedItemV1 {
        &self.items[0]
    }

    pub fn logical_helper(&self) -> &KernelContextGeneratedItemV1 {
        &self.items[1]
    }

    pub fn nominal_kernel_marker(&self) -> &KernelContextGeneratedItemV1 {
        &self.items[2]
    }

    pub const fn context_source_ordinal(&self) -> u16 {
        self.context_source_ordinal
    }

    pub const fn issuance_diagnostic(&self) -> &KernelContextIssuanceDiagnosticV1 {
        &self.issuance_diagnostic
    }

    pub const fn required_issuance_count(&self) -> u16 {
        self.required_issuance_count
    }

    fn items(&self) -> &[KernelContextGeneratedItemV1; ITEM_COUNT_V1] {
        &self.items
    }
}

pub fn derive_kernel_context_generated_item_identity_v1(
    role: KernelContextGeneratedItemRoleV1,
    name: &str,
) -> KernelContextGeneratedItemIdentityV1 {
    let mut digest = Sha256::new();
    append_digest_field(&mut digest, ITEM_IDENTITY_DOMAIN_V1);
    append_digest_field(&mut digest, &(role as u16).to_le_bytes());
    append_digest_field(&mut digest, name.as_bytes());
    KernelContextGeneratedItemIdentityV1(digest.finalize().into())
}

/// Convenience encoder intended for the `#[kernel]` macro.
pub fn encode_generated_kernel_context_frontend_contract_v1(
    physical_kernel_root_name: &str,
    logical_helper_name: &str,
    nominal_kernel_marker_name: &str,
) -> Result<Vec<u8>, KernelContextFrontendContractValidationErrorV1> {
    KernelContextFrontendContractV1::for_generated_kernel(
        physical_kernel_root_name,
        logical_helper_name,
        nominal_kernel_marker_name,
    )
    .map(|contract| encode_kernel_context_frontend_contract_v1(&contract))
}

pub fn encode_kernel_context_frontend_contract_v1(
    contract: &KernelContextFrontendContractV1,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(MAX_KERNEL_CONTEXT_FRONTEND_CONTRACT_BYTES_V1);
    bytes.extend_from_slice(&KERNEL_CONTEXT_FRONTEND_CONTRACT_MAGIC_V1);
    push_u16(&mut bytes, KERNEL_CONTEXT_FRONTEND_CONTRACT_VERSION_V1);
    push_u16(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u16(&mut bytes, ITEM_COUNT_V1 as u16);
    push_u16(&mut bytes, contract.context_source_ordinal);
    push_u16(&mut bytes, contract.required_issuance_count);
    push_u16(&mut bytes, contract.issuance_diagnostic.version);
    push_u16(
        &mut bytes,
        contract.issuance_diagnostic.identity.len() as u16,
    );
    push_u16(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    bytes.extend_from_slice(contract.issuance_diagnostic.identity.as_bytes());

    for item in contract.items() {
        push_u16(&mut bytes, item.role as u16);
        push_u16(&mut bytes, 0);
        bytes.extend_from_slice(item.identity.as_bytes());
        push_u16(&mut bytes, item.name.len() as u16);
        push_u16(&mut bytes, 0);
        bytes.extend_from_slice(item.name.as_bytes());
    }

    let length = u32::try_from(bytes.len()).expect("V1 context contract is bounded below u32");
    bytes[12..16].copy_from_slice(&length.to_le_bytes());
    bytes
}

pub fn decode_kernel_context_frontend_contract_v1(
    bytes: &[u8],
) -> Result<KernelContextFrontendContractV1, KernelContextFrontendContractDecodeErrorV1> {
    if bytes.len() > MAX_KERNEL_CONTEXT_FRONTEND_CONTRACT_BYTES_V1 {
        return Err(KernelContextFrontendContractDecodeErrorV1::TooLarge);
    }
    let mut reader = Reader::new(bytes);
    if reader.fixed::<8>()? != KERNEL_CONTEXT_FRONTEND_CONTRACT_MAGIC_V1 {
        return Err(KernelContextFrontendContractDecodeErrorV1::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != KERNEL_CONTEXT_FRONTEND_CONTRACT_VERSION_V1 {
        return Err(KernelContextFrontendContractDecodeErrorV1::UnknownVersion(
            version,
        ));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(KernelContextFrontendContractDecodeErrorV1::UnsupportedFlags(flags));
    }
    let declared = reader.u32()?;
    if declared < HEADER_BYTES_V1 as u32 {
        return Err(KernelContextFrontendContractDecodeErrorV1::InvalidLength(
            declared,
        ));
    }
    let declared = usize::try_from(declared)
        .map_err(|_| KernelContextFrontendContractDecodeErrorV1::InvalidLength(declared))?;
    if declared > bytes.len() {
        return Err(KernelContextFrontendContractDecodeErrorV1::Truncated);
    }
    if declared < bytes.len() {
        return Err(KernelContextFrontendContractDecodeErrorV1::TrailingBytes);
    }

    let item_count = usize::from(reader.u16()?);
    if item_count != ITEM_COUNT_V1 {
        return Err(KernelContextFrontendContractDecodeErrorV1::Validation(
            KernelContextFrontendContractValidationErrorV1::ItemCount(item_count),
        ));
    }
    let context_source_ordinal = reader.u16()?;
    let required_issuance_count = reader.u16()?;
    let diagnostic_version = reader.u16()?;
    let diagnostic_length = usize::from(reader.u16()?);
    if diagnostic_length > KERNEL_CONTEXT_ISSUANCE_DIAGNOSTIC_IDENTITY_V1.len() {
        return Err(KernelContextFrontendContractDecodeErrorV1::TextTooLong {
            field: "kernel context issuance diagnostic identity",
            max: KERNEL_CONTEXT_ISSUANCE_DIAGNOSTIC_IDENTITY_V1.len(),
        });
    }
    reader.reserved_u16("kernel context contract header")?;
    reader.reserved_u32("kernel context contract header")?;
    let diagnostic_identity = reader.text(
        diagnostic_length,
        "kernel context issuance diagnostic identity",
    )?;
    let issuance_diagnostic =
        KernelContextIssuanceDiagnosticV1::new(diagnostic_identity, diagnostic_version)?;

    let mut items = Vec::with_capacity(ITEM_COUNT_V1);
    for _ in 0..item_count {
        let role = match reader.u16()? {
            1 => KernelContextGeneratedItemRoleV1::PhysicalKernelRoot,
            2 => KernelContextGeneratedItemRoleV1::LogicalHelper,
            3 => KernelContextGeneratedItemRoleV1::NominalKernelMarker,
            tag => {
                return Err(KernelContextFrontendContractDecodeErrorV1::UnknownItemRole(
                    tag,
                ));
            }
        };
        reader.reserved_u16("kernel context generated item")?;
        let identity = KernelContextGeneratedItemIdentityV1::new(reader.fixed::<32>()?)?;
        let name_length = usize::from(reader.u16()?);
        if name_length > MAX_KERNEL_CONTEXT_GENERATED_ITEM_NAME_BYTES_V1 {
            return Err(KernelContextFrontendContractDecodeErrorV1::TextTooLong {
                field: "kernel context generated item name",
                max: MAX_KERNEL_CONTEXT_GENERATED_ITEM_NAME_BYTES_V1,
            });
        }
        reader.reserved_u16("kernel context generated item")?;
        let name = reader.text(name_length, "kernel context generated item name")?;
        items.push(KernelContextGeneratedItemV1::new(role, identity, name)?);
    }
    if !reader.finished() {
        return Err(KernelContextFrontendContractDecodeErrorV1::TrailingBytes);
    }
    let items: [KernelContextGeneratedItemV1; ITEM_COUNT_V1] = items
        .try_into()
        .map_err(|_| KernelContextFrontendContractDecodeErrorV1::Truncated)?;
    let contract = KernelContextFrontendContractV1::new(
        items,
        context_source_ordinal,
        issuance_diagnostic,
        required_issuance_count,
    )?;
    if encode_kernel_context_frontend_contract_v1(&contract) != bytes {
        return Err(KernelContextFrontendContractDecodeErrorV1::NonCanonical);
    }
    Ok(contract)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelContextFrontendContractValidationErrorV1 {
    ItemCount(usize),
    EmptyItemName,
    ItemNameTooLong,
    NonAsciiItemName,
    InvalidItemName,
    ZeroItemIdentity,
    DuplicateItemRole(KernelContextGeneratedItemRoleV1),
    MissingItemRole(KernelContextGeneratedItemRoleV1),
    DuplicateItemName,
    ItemIdentityNameMismatch(KernelContextGeneratedItemRoleV1),
    ContextSourceOrdinal(u16),
    IssuanceDiagnosticIdentity,
    IssuanceDiagnosticVersion(u16),
    RequiredIssuanceCount(u16),
}

impl fmt::Display for KernelContextFrontendContractValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ItemCount(actual) => write!(
                formatter,
                "kernel context contract requires exactly three item bindings; found {actual}"
            ),
            Self::EmptyItemName => formatter.write_str("generated item name is empty"),
            Self::ItemNameTooLong => write!(
                formatter,
                "generated item name exceeds {MAX_KERNEL_CONTEXT_GENERATED_ITEM_NAME_BYTES_V1} bytes"
            ),
            Self::NonAsciiItemName => {
                formatter.write_str("generated item name must contain only ASCII bytes")
            }
            Self::InvalidItemName => {
                formatter.write_str("generated item name is not a canonical ASCII Rust identifier")
            }
            Self::ZeroItemIdentity => formatter.write_str("generated item identity is zero"),
            Self::DuplicateItemRole(role) => write!(
                formatter,
                "kernel context contract duplicates the {} binding",
                role.canonical_name()
            ),
            Self::MissingItemRole(role) => write!(
                formatter,
                "kernel context contract omits the {} binding",
                role.canonical_name()
            ),
            Self::DuplicateItemName => {
                formatter.write_str("kernel context generated item names must be distinct")
            }
            Self::ItemIdentityNameMismatch(role) => write!(
                formatter,
                "{} identity does not commit to its exact generated name",
                role.canonical_name()
            ),
            Self::ContextSourceOrdinal(actual) => write!(
                formatter,
                "logical KernelContext source ordinal must be 0; found {actual}"
            ),
            Self::IssuanceDiagnosticIdentity => formatter.write_str(
                "logical KernelContext issuance diagnostic identity is not canonical V1",
            ),
            Self::IssuanceDiagnosticVersion(actual) => write!(
                formatter,
                "logical KernelContext issuance diagnostic version must be 1; found {actual}"
            ),
            Self::RequiredIssuanceCount(actual) => write!(
                formatter,
                "logical KernelContext issuance count must be exactly 1; found {actual}"
            ),
        }
    }
}

impl std::error::Error for KernelContextFrontendContractValidationErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelContextFrontendContractDecodeErrorV1 {
    TooLarge,
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    InvalidLength(u32),
    TrailingBytes,
    NonzeroReserved(&'static str),
    UnknownItemRole(u16),
    InvalidUtf8(&'static str),
    TextTooLong { field: &'static str, max: usize },
    NonCanonical,
    Validation(KernelContextFrontendContractValidationErrorV1),
}

impl fmt::Display for KernelContextFrontendContractDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(
                formatter,
                "kernel context frontend contract exceeds {MAX_KERNEL_CONTEXT_FRONTEND_CONTRACT_BYTES_V1} bytes"
            ),
            Self::Truncated => formatter.write_str("kernel context frontend contract is truncated"),
            Self::InvalidMagic => {
                formatter.write_str("kernel context frontend contract magic is invalid")
            }
            Self::UnknownVersion(version) => write!(
                formatter,
                "unsupported kernel context frontend contract version {version}"
            ),
            Self::UnsupportedFlags(flags) => write!(
                formatter,
                "unsupported kernel context frontend contract flags {flags:#x}"
            ),
            Self::InvalidLength(length) => write!(
                formatter,
                "invalid kernel context frontend contract length {length}"
            ),
            Self::TrailingBytes => {
                formatter.write_str("kernel context frontend contract contains trailing bytes")
            }
            Self::NonzeroReserved(field) => write!(formatter, "{field} reserved field is nonzero"),
            Self::UnknownItemRole(tag) => {
                write!(
                    formatter,
                    "unknown kernel context generated item role {tag}"
                )
            }
            Self::InvalidUtf8(field) => write!(formatter, "{field} is not valid UTF-8"),
            Self::TextTooLong { field, max } => {
                write!(formatter, "{field} exceeds {max} bytes")
            }
            Self::NonCanonical => {
                formatter.write_str("kernel context frontend contract is not canonical")
            }
            Self::Validation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for KernelContextFrontendContractDecodeErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<KernelContextFrontendContractValidationErrorV1>
    for KernelContextFrontendContractDecodeErrorV1
{
    fn from(value: KernelContextFrontendContractValidationErrorV1) -> Self {
        Self::Validation(value)
    }
}

fn validate_generated_item_name(
    name: &str,
) -> Result<(), KernelContextFrontendContractValidationErrorV1> {
    if name.is_empty() {
        return Err(KernelContextFrontendContractValidationErrorV1::EmptyItemName);
    }
    if name.len() > MAX_KERNEL_CONTEXT_GENERATED_ITEM_NAME_BYTES_V1 {
        return Err(KernelContextFrontendContractValidationErrorV1::ItemNameTooLong);
    }
    if !name.is_ascii() {
        return Err(KernelContextFrontendContractValidationErrorV1::NonAsciiItemName);
    }
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return Err(KernelContextFrontendContractValidationErrorV1::EmptyItemName);
    };
    if !(first == b'_' || first.is_ascii_alphabetic())
        || !bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
        || name == "_"
    {
        return Err(KernelContextFrontendContractValidationErrorV1::InvalidItemName);
    }
    Ok(())
}

fn append_digest_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], KernelContextFrontendContractDecodeErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(KernelContextFrontendContractDecodeErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(KernelContextFrontendContractDecodeErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], KernelContextFrontendContractDecodeErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| KernelContextFrontendContractDecodeErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, KernelContextFrontendContractDecodeErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, KernelContextFrontendContractDecodeErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn reserved_u16(
        &mut self,
        field: &'static str,
    ) -> Result<(), KernelContextFrontendContractDecodeErrorV1> {
        if self.u16()? != 0 {
            return Err(KernelContextFrontendContractDecodeErrorV1::NonzeroReserved(
                field,
            ));
        }
        Ok(())
    }

    fn reserved_u32(
        &mut self,
        field: &'static str,
    ) -> Result<(), KernelContextFrontendContractDecodeErrorV1> {
        if self.u32()? != 0 {
            return Err(KernelContextFrontendContractDecodeErrorV1::NonzeroReserved(
                field,
            ));
        }
        Ok(())
    }

    fn text(
        &mut self,
        length: usize,
        field: &'static str,
    ) -> Result<String, KernelContextFrontendContractDecodeErrorV1> {
        str::from_utf8(self.take(length)?)
            .map(str::to_owned)
            .map_err(|_| KernelContextFrontendContractDecodeErrorV1::InvalidUtf8(field))
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
