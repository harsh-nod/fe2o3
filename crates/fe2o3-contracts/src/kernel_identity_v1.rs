//! Canonical Rust kernel-definition and monomorphization identities.
//!
//! These records contain SHA-256 commitments produced by an authenticated
//! Rust frontend. This crate deliberately does not authenticate those
//! commitments: construction and canonical decoding establish only field
//! separation, exact framing, and deterministic bytes. In particular, these
//! values grant no execution, artifact, cache, launch, or proof authority.
//!
//! The records are independent of Pliron contexts, arena allocation, printer
//! output, filesystem paths, and traversal order. Producers must derive each
//! component from a separately versioned canonical Rust descriptor.

use core::fmt;

/// Version shared by the V1 item and instance identity envelopes.
pub const KERNEL_IDENTITY_VERSION_V1: u16 = 1;
/// Domain-separating magic for a canonical [`KernelItemId`].
pub const KERNEL_ITEM_ID_MAGIC_V1: [u8; 8] = *b"F2KITEM1";
/// Domain-separating magic for a canonical [`KernelInstId`].
pub const KERNEL_INST_ID_MAGIC_V1: [u8; 8] = *b"F2KINST1";
/// Width of every independently derived identity commitment.
pub const KERNEL_IDENTITY_COMPONENT_BYTES_V1: usize = 32;
/// Exact width of a canonical [`KernelItemId`].
pub const KERNEL_ITEM_ID_CANONICAL_BYTES_V1: usize = 112;
/// Exact width of a canonical [`KernelInstId`].
pub const KERNEL_INST_ID_CANONICAL_BYTES_V1: usize = 224;

const HEADER_BYTES_V1: usize = 16;
const ITEM_PAYLOAD_BYTES_V1: usize = 3 * KERNEL_IDENTITY_COMPONENT_BYTES_V1;
const INST_PAYLOAD_BYTES_V1: usize =
    KERNEL_ITEM_ID_CANONICAL_BYTES_V1 + 3 * KERNEL_IDENTITY_COMPONENT_BYTES_V1;
const FLAGS_V1: u16 = 0;

const _: () = assert!(KERNEL_ITEM_ID_CANONICAL_BYTES_V1 == HEADER_BYTES_V1 + ITEM_PAYLOAD_BYTES_V1);
const _: () = assert!(KERNEL_INST_ID_CANONICAL_BYTES_V1 == HEADER_BYTES_V1 + INST_PAYLOAD_BYTES_V1);

macro_rules! identity_component {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name([u8; KERNEL_IDENTITY_COMPONENT_BYTES_V1]);

        impl $name {
            /// Wraps a caller-supplied SHA-256 commitment without authenticating it.
            pub const fn from_untrusted_bytes(
                bytes: [u8; KERNEL_IDENTITY_COMPONENT_BYTES_V1],
            ) -> Self {
                Self(bytes)
            }

            /// Returns the opaque commitment bytes without granting authority.
            pub const fn as_bytes(&self) -> &[u8; KERNEL_IDENTITY_COMPONENT_BYTES_V1] {
                &self.0
            }
        }
    };
}

identity_component!(
    /// Commitment to the authenticated crate identity containing the item.
    KernelCrateIdentityV1
);
identity_component!(
    /// Commitment to the authenticated Rust item identity within its crate.
    KernelRustItemIdentityV1
);
identity_component!(
    /// Commitment to the exact generic definition, before monomorphization.
    KernelGenericDefinitionIdentityV1
);
identity_component!(
    /// Commitment to the ordered, concrete, normalized type arguments.
    KernelTypeArgumentsIdentityV1
);
identity_component!(
    /// Commitment to the ordered concrete const arguments and their types.
    KernelConstArgumentsIdentityV1
);
identity_component!(
    /// Commitment to the canonical Rust `cfg` inputs affecting the instance.
    KernelCfgIdentityV1
);

/// Identity of one authenticated generic Rust kernel definition.
///
/// The crate, item, and generic-definition commitments remain separate so a
/// consumer cannot silently substitute one property for another. Creating or
/// decoding this record does not itself establish that authentication occurred.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KernelItemId {
    crate_identity: KernelCrateIdentityV1,
    rust_item_identity: KernelRustItemIdentityV1,
    generic_definition_identity: KernelGenericDefinitionIdentityV1,
}

impl KernelItemId {
    pub const fn new(
        crate_identity: KernelCrateIdentityV1,
        rust_item_identity: KernelRustItemIdentityV1,
        generic_definition_identity: KernelGenericDefinitionIdentityV1,
    ) -> Self {
        Self {
            crate_identity,
            rust_item_identity,
            generic_definition_identity,
        }
    }

    pub const fn crate_identity(self) -> KernelCrateIdentityV1 {
        self.crate_identity
    }

    pub const fn rust_item_identity(self) -> KernelRustItemIdentityV1 {
        self.rust_item_identity
    }

    pub const fn generic_definition_identity(self) -> KernelGenericDefinitionIdentityV1 {
        self.generic_definition_identity
    }

    /// Encodes this item identity in its exact fixed-width V1 form.
    pub fn encode_canonical(self) -> [u8; KERNEL_ITEM_ID_CANONICAL_BYTES_V1] {
        encode_kernel_item_id(self)
    }

    /// Decodes only the exact canonical V1 item representation.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, KernelItemIdDecodeErrorV1> {
        decode_kernel_item_id(bytes)
    }
}

/// Identity of one concrete Rust kernel monomorphization.
///
/// This layer binds only source-level type, const, and `cfg` specialization to
/// its generic item. Algorithm, schedule, target-plan, executable, and proof
/// identities remain separate authority domains and are intentionally absent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KernelInstId {
    item: KernelItemId,
    type_arguments_identity: KernelTypeArgumentsIdentityV1,
    const_arguments_identity: KernelConstArgumentsIdentityV1,
    cfg_identity: KernelCfgIdentityV1,
}

impl KernelInstId {
    pub const fn new(
        item: KernelItemId,
        type_arguments_identity: KernelTypeArgumentsIdentityV1,
        const_arguments_identity: KernelConstArgumentsIdentityV1,
        cfg_identity: KernelCfgIdentityV1,
    ) -> Self {
        Self {
            item,
            type_arguments_identity,
            const_arguments_identity,
            cfg_identity,
        }
    }

    pub const fn item(self) -> KernelItemId {
        self.item
    }

    pub const fn type_arguments_identity(self) -> KernelTypeArgumentsIdentityV1 {
        self.type_arguments_identity
    }

    pub const fn const_arguments_identity(self) -> KernelConstArgumentsIdentityV1 {
        self.const_arguments_identity
    }

    pub const fn cfg_identity(self) -> KernelCfgIdentityV1 {
        self.cfg_identity
    }

    /// Encodes this instance identity in its exact fixed-width V1 form.
    pub fn encode_canonical(self) -> [u8; KERNEL_INST_ID_CANONICAL_BYTES_V1] {
        encode_kernel_inst_id(self)
    }

    /// Decodes only the exact canonical V1 instance representation.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, KernelInstIdDecodeErrorV1> {
        decode_kernel_inst_id(bytes)
    }
}

/// Encodes a generic kernel item with an explicit domain, version, flags, and length.
pub fn encode_kernel_item_id(id: KernelItemId) -> [u8; KERNEL_ITEM_ID_CANONICAL_BYTES_V1] {
    let mut bytes = [0_u8; KERNEL_ITEM_ID_CANONICAL_BYTES_V1];
    encode_header(
        &mut bytes[..HEADER_BYTES_V1],
        KERNEL_ITEM_ID_MAGIC_V1,
        ITEM_PAYLOAD_BYTES_V1,
    );
    let mut offset = HEADER_BYTES_V1;
    put_component(&mut bytes, &mut offset, id.crate_identity.as_bytes());
    put_component(&mut bytes, &mut offset, id.rust_item_identity.as_bytes());
    put_component(
        &mut bytes,
        &mut offset,
        id.generic_definition_identity.as_bytes(),
    );
    debug_assert_eq!(offset, bytes.len());
    bytes
}

/// Decodes a generic kernel item and rejects malformed or noncanonical bytes.
pub fn decode_kernel_item_id(bytes: &[u8]) -> Result<KernelItemId, KernelItemIdDecodeErrorV1> {
    decode_item_envelope(bytes)?;
    let id = KernelItemId::new(
        KernelCrateIdentityV1::from_untrusted_bytes(component(bytes, HEADER_BYTES_V1)),
        KernelRustItemIdentityV1::from_untrusted_bytes(component(
            bytes,
            HEADER_BYTES_V1 + KERNEL_IDENTITY_COMPONENT_BYTES_V1,
        )),
        KernelGenericDefinitionIdentityV1::from_untrusted_bytes(component(
            bytes,
            HEADER_BYTES_V1 + 2 * KERNEL_IDENTITY_COMPONENT_BYTES_V1,
        )),
    );
    if id.encode_canonical().as_slice() != bytes {
        return Err(KernelItemIdDecodeErrorV1::NonCanonical);
    }
    Ok(id)
}

/// Encodes a concrete kernel instance, including its complete canonical item record.
pub fn encode_kernel_inst_id(id: KernelInstId) -> [u8; KERNEL_INST_ID_CANONICAL_BYTES_V1] {
    let mut bytes = [0_u8; KERNEL_INST_ID_CANONICAL_BYTES_V1];
    encode_header(
        &mut bytes[..HEADER_BYTES_V1],
        KERNEL_INST_ID_MAGIC_V1,
        INST_PAYLOAD_BYTES_V1,
    );
    let item = id.item.encode_canonical();
    let item_end = HEADER_BYTES_V1 + item.len();
    bytes[HEADER_BYTES_V1..item_end].copy_from_slice(&item);
    let mut offset = item_end;
    put_component(
        &mut bytes,
        &mut offset,
        id.type_arguments_identity.as_bytes(),
    );
    put_component(
        &mut bytes,
        &mut offset,
        id.const_arguments_identity.as_bytes(),
    );
    put_component(&mut bytes, &mut offset, id.cfg_identity.as_bytes());
    debug_assert_eq!(offset, bytes.len());
    bytes
}

/// Decodes a concrete kernel instance and rejects malformed or noncanonical bytes.
pub fn decode_kernel_inst_id(bytes: &[u8]) -> Result<KernelInstId, KernelInstIdDecodeErrorV1> {
    decode_inst_envelope(bytes)?;
    let item_end = HEADER_BYTES_V1 + KERNEL_ITEM_ID_CANONICAL_BYTES_V1;
    let item = decode_kernel_item_id(&bytes[HEADER_BYTES_V1..item_end])
        .map_err(KernelInstIdDecodeErrorV1::InvalidKernelItem)?;
    let id = KernelInstId::new(
        item,
        KernelTypeArgumentsIdentityV1::from_untrusted_bytes(component(bytes, item_end)),
        KernelConstArgumentsIdentityV1::from_untrusted_bytes(component(
            bytes,
            item_end + KERNEL_IDENTITY_COMPONENT_BYTES_V1,
        )),
        KernelCfgIdentityV1::from_untrusted_bytes(component(
            bytes,
            item_end + 2 * KERNEL_IDENTITY_COMPONENT_BYTES_V1,
        )),
    );
    if id.encode_canonical().as_slice() != bytes {
        return Err(KernelInstIdDecodeErrorV1::NonCanonical);
    }
    Ok(id)
}

/// Why a canonical generic kernel-item record was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelItemIdDecodeErrorV1 {
    Truncated { actual: usize, expected: usize },
    TrailingBytes { actual: usize, expected: usize },
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    InvalidPayloadLength { actual: u32, expected: u32 },
    NonCanonical,
}

impl fmt::Display for KernelItemIdDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { actual, expected } | Self::TrailingBytes { actual, expected } => {
                write!(
                    formatter,
                    "kernel item identity has {actual} bytes; expected {expected}"
                )
            }
            Self::InvalidMagic => formatter.write_str("invalid kernel item identity magic"),
            Self::UnknownVersion(version) => {
                write!(formatter, "unknown kernel item identity version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(
                    formatter,
                    "unsupported kernel item identity flags {flags:#x}"
                )
            }
            Self::InvalidPayloadLength { actual, expected } => write!(
                formatter,
                "kernel item identity payload has {actual} bytes; expected {expected}"
            ),
            Self::NonCanonical => formatter.write_str("kernel item identity is not canonical"),
        }
    }
}

/// Why a canonical concrete kernel-instance record was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelInstIdDecodeErrorV1 {
    Truncated { actual: usize, expected: usize },
    TrailingBytes { actual: usize, expected: usize },
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    InvalidPayloadLength { actual: u32, expected: u32 },
    InvalidKernelItem(KernelItemIdDecodeErrorV1),
    NonCanonical,
}

impl fmt::Display for KernelInstIdDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { actual, expected } | Self::TrailingBytes { actual, expected } => {
                write!(
                    formatter,
                    "kernel instance identity has {actual} bytes; expected {expected}"
                )
            }
            Self::InvalidMagic => formatter.write_str("invalid kernel instance identity magic"),
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "unknown kernel instance identity version {version}"
                )
            }
            Self::UnsupportedFlags(flags) => {
                write!(
                    formatter,
                    "unsupported kernel instance identity flags {flags:#x}"
                )
            }
            Self::InvalidPayloadLength { actual, expected } => write!(
                formatter,
                "kernel instance identity payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidKernelItem(error) => {
                write!(formatter, "invalid nested kernel item identity: {error}")
            }
            Self::NonCanonical => formatter.write_str("kernel instance identity is not canonical"),
        }
    }
}

fn encode_header(bytes: &mut [u8], magic: [u8; 8], payload_bytes: usize) {
    bytes[..8].copy_from_slice(&magic);
    bytes[8..10].copy_from_slice(&KERNEL_IDENTITY_VERSION_V1.to_le_bytes());
    bytes[10..12].copy_from_slice(&FLAGS_V1.to_le_bytes());
    bytes[12..16].copy_from_slice(&(payload_bytes as u32).to_le_bytes());
}

fn put_component(bytes: &mut [u8], offset: &mut usize, component: &[u8; 32]) {
    let end = *offset + component.len();
    bytes[*offset..end].copy_from_slice(component);
    *offset = end;
}

fn component(bytes: &[u8], offset: usize) -> [u8; KERNEL_IDENTITY_COMPONENT_BYTES_V1] {
    let mut component = [0_u8; KERNEL_IDENTITY_COMPONENT_BYTES_V1];
    component.copy_from_slice(&bytes[offset..offset + KERNEL_IDENTITY_COMPONENT_BYTES_V1]);
    component
}

fn decode_item_envelope(bytes: &[u8]) -> Result<(), KernelItemIdDecodeErrorV1> {
    if bytes.len() < HEADER_BYTES_V1 {
        return Err(KernelItemIdDecodeErrorV1::Truncated {
            actual: bytes.len(),
            expected: HEADER_BYTES_V1,
        });
    }
    if bytes[..8] != KERNEL_ITEM_ID_MAGIC_V1 {
        return Err(KernelItemIdDecodeErrorV1::InvalidMagic);
    }
    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != KERNEL_IDENTITY_VERSION_V1 {
        return Err(KernelItemIdDecodeErrorV1::UnknownVersion(version));
    }
    let flags = u16::from_le_bytes([bytes[10], bytes[11]]);
    if flags != FLAGS_V1 {
        return Err(KernelItemIdDecodeErrorV1::UnsupportedFlags(flags));
    }
    let payload_bytes = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    if payload_bytes != ITEM_PAYLOAD_BYTES_V1 as u32 {
        return Err(KernelItemIdDecodeErrorV1::InvalidPayloadLength {
            actual: payload_bytes,
            expected: ITEM_PAYLOAD_BYTES_V1 as u32,
        });
    }
    if bytes.len() < KERNEL_ITEM_ID_CANONICAL_BYTES_V1 {
        return Err(KernelItemIdDecodeErrorV1::Truncated {
            actual: bytes.len(),
            expected: KERNEL_ITEM_ID_CANONICAL_BYTES_V1,
        });
    }
    if bytes.len() > KERNEL_ITEM_ID_CANONICAL_BYTES_V1 {
        return Err(KernelItemIdDecodeErrorV1::TrailingBytes {
            actual: bytes.len(),
            expected: KERNEL_ITEM_ID_CANONICAL_BYTES_V1,
        });
    }
    Ok(())
}

fn decode_inst_envelope(bytes: &[u8]) -> Result<(), KernelInstIdDecodeErrorV1> {
    if bytes.len() < HEADER_BYTES_V1 {
        return Err(KernelInstIdDecodeErrorV1::Truncated {
            actual: bytes.len(),
            expected: HEADER_BYTES_V1,
        });
    }
    if bytes[..8] != KERNEL_INST_ID_MAGIC_V1 {
        return Err(KernelInstIdDecodeErrorV1::InvalidMagic);
    }
    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != KERNEL_IDENTITY_VERSION_V1 {
        return Err(KernelInstIdDecodeErrorV1::UnknownVersion(version));
    }
    let flags = u16::from_le_bytes([bytes[10], bytes[11]]);
    if flags != FLAGS_V1 {
        return Err(KernelInstIdDecodeErrorV1::UnsupportedFlags(flags));
    }
    let payload_bytes = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    if payload_bytes != INST_PAYLOAD_BYTES_V1 as u32 {
        return Err(KernelInstIdDecodeErrorV1::InvalidPayloadLength {
            actual: payload_bytes,
            expected: INST_PAYLOAD_BYTES_V1 as u32,
        });
    }
    if bytes.len() < KERNEL_INST_ID_CANONICAL_BYTES_V1 {
        return Err(KernelInstIdDecodeErrorV1::Truncated {
            actual: bytes.len(),
            expected: KERNEL_INST_ID_CANONICAL_BYTES_V1,
        });
    }
    if bytes.len() > KERNEL_INST_ID_CANONICAL_BYTES_V1 {
        return Err(KernelInstIdDecodeErrorV1::TrailingBytes {
            actual: bytes.len(),
            expected: KERNEL_INST_ID_CANONICAL_BYTES_V1,
        });
    }
    Ok(())
}
