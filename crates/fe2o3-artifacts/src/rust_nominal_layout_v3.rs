//! Fixed-size portable nominal declarations, not authenticated rustc evidence.
//!
//! These content identities do not authenticate a source type, descriptor,
//! native artifact or launch. Consumers must independently bind the actual
//! nominal source and physical ABI to the same descriptor. V1 evidence is not
//! constructed or reinterpreted here. Every payload and hash uses fixed-size
//! stack state, with no owned heap backing or dynamic resource receipt.

use std::fmt;

use sha2::{Digest, Sha256};

use crate::{
    DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestBytes, PointerWidth,
    RustScalarElementTypeV1, RustcAbiClassV1, TypeIdentity,
};

/// Domain for the length-framed four-byte nominal source payload.
pub const RUST_NOMINAL_TYPE_DOMAIN_V3: &[u8] = b"FE2O3/RUST-NOMINAL-TYPE/V3\0";
/// Domain for the fixed 56-byte nominal layout payload, without a length frame.
pub const RUST_NOMINAL_LAYOUT_DOMAIN_V3: &[u8] = b"FE2O3/RUST-NOMINAL-LAYOUT/V3\0";

/// Closed nominal source kinds; equal physical width does not imply equality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustNominalScalarKindV3 {
    Usize,
    Isize,
}

/// A validated portable declaration with no source-identity override.
///
/// Constructing this value does not prove that rustc observed the declared kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustNominalScalarEvidenceV3 {
    kind: RustNominalScalarKindV3,
    pointer_width: PointerWidth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustNominalLayoutErrorV3 {
    UnsupportedPointerWidth(PointerWidth),
}

impl fmt::Display for RustNominalLayoutErrorV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPointerWidth(width) => write!(
                f,
                "nominal scalar ABI V3 requires 64-bit pointers, got {}-bit",
                width.bytes() * 8
            ),
        }
    }
}

impl std::error::Error for RustNominalLayoutErrorV3 {}

impl RustNominalScalarEvidenceV3 {
    /// The initial portable ABI contract admits only a 64-bit target.
    pub fn new(
        kind: RustNominalScalarKindV3,
        pointer_width: PointerWidth,
    ) -> Result<Self, RustNominalLayoutErrorV3> {
        if pointer_width != PointerWidth::Bits64 {
            return Err(RustNominalLayoutErrorV3::UnsupportedPointerWidth(
                pointer_width,
            ));
        }
        Ok(Self {
            kind,
            pointer_width,
        })
    }

    pub const fn kind(self) -> RustNominalScalarKindV3 {
        self.kind
    }

    pub const fn pointer_width(self) -> PointerWidth {
        self.pointer_width
    }

    pub const fn physical_scalar(self) -> RustScalarElementTypeV1 {
        match self.kind {
            RustNominalScalarKindV3::Usize => RustScalarElementTypeV1::U64,
            RustNominalScalarKindV3::Isize => RustScalarElementTypeV1::I64,
        }
    }

    pub const fn abi_class(self) -> RustcAbiClassV1 {
        RustcAbiClassV1::Scalar
    }

    pub const fn size(self) -> u64 {
        8
    }

    pub const fn abi_alignment(self) -> u32 {
        8
    }

    /// Version u16, nominal tag and reserved byte; not a descriptor source row.
    pub const fn canonical_type_payload(self) -> [u8; 4] {
        let tag = match self.kind {
            RustNominalScalarKindV3::Usize => 5,
            RustNominalScalarKindV3::Isize => 6,
        };
        [3, 0, tag, 0]
    }

    fn nominal_type_digest(self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(RUST_NOMINAL_TYPE_DOMAIN_V3);
        hash.update(4_u64.to_le_bytes());
        hash.update(self.canonical_type_payload());
        hash.finalize().into()
    }

    /// Type digest, pointer bits, physical/ABI tags, size, alignment and offset.
    /// All integer fields are little-endian; the sole scalar begins at offset 0.
    pub fn canonical_layout_payload(self) -> [u8; 56] {
        let mut payload = [0; 56];
        payload[..32].copy_from_slice(&self.nominal_type_digest());
        payload[32..34].copy_from_slice(&64_u16.to_le_bytes());
        payload[34] = match self.kind {
            RustNominalScalarKindV3::Usize => 8,
            RustNominalScalarKindV3::Isize => 7,
        };
        payload[35] = 1;
        payload[36..44].copy_from_slice(&self.size().to_le_bytes());
        payload[44..48].copy_from_slice(&self.abi_alignment().to_le_bytes());
        payload
    }

    /// Content identities only, distinct from actual rustc and descriptor IDs.
    pub fn type_identity(self) -> TypeIdentity {
        let payload = self.canonical_layout_payload();
        let mut source_digest = [0; 32];
        source_digest.copy_from_slice(&payload[..32]);
        let mut hash = Sha256::new();
        hash.update(RUST_NOMINAL_LAYOUT_DOMAIN_V3);
        hash.update(payload);
        TypeIdentity::new(
            DeclaredRustTypeIdentity::from_untrusted_bytes(DigestBytes::from_bytes(source_digest)),
            DeclaredRustLayoutIdentity::from_untrusted_bytes(DigestBytes::from_bytes(
                hash.finalize().into(),
            )),
        )
    }
}
