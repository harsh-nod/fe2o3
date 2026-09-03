use std::{fmt, ops::Range, sync::Arc};

use sha2::{Digest, Sha256};

use crate::{LineageDecodeErrorV3, LineageErrorV3};

#[derive(Clone)]
pub(crate) enum SharedBackingV3 {
    Slice(Arc<[u8]>),
    Vector(Arc<Vec<u8>>),
}

impl SharedBackingV3 {
    pub(crate) fn as_slice(&self) -> &[u8] {
        match self {
            Self::Slice(bytes) => bytes,
            Self::Vector(bytes) => bytes,
        }
    }
}

pub(crate) enum ImmutableBytesV3 {
    Owned(Box<[u8]>),
    Shared {
        backing: SharedBackingV3,
        range: Range<usize>,
    },
}

impl ImmutableBytesV3 {
    pub(crate) fn from_owned(bytes: Box<[u8]>) -> Self {
        Self::Owned(bytes)
    }

    pub(crate) fn from_shared(backing: SharedBackingV3, range: Range<usize>) -> Option<Self> {
        backing.as_slice().get(range.clone())?;
        Some(Self::Shared { backing, range })
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Shared { backing, range } => backing
                .as_slice()
                .get(range.clone())
                .expect("shared byte range was validated at construction"),
        }
    }
}

impl PartialEq for ImmutableBytesV3 {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for ImmutableBytesV3 {}

/// Maximum bytes retained for one non-MIR lineage receipt preimage.
pub const MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3: usize = 4 * 1024 * 1024;

/// Maximum bytes retained for the canonical rustc preflight-plan transcript.
///
/// The plan records the complete admitted rustc producer closure and can be
/// larger than the other non-MIR receipts for aggregate device crates.
pub const MAX_RUSTC_PREFLIGHT_PLAN_RECEIPT_PREIMAGE_BYTES_V3: usize = 8 * 1024 * 1024;

/// Maximum bytes retained for exact canonical semantic MIR.
pub const MAX_CANONICAL_SEMANTIC_MIR_BYTES_V3: usize = 128 * 1024 * 1024;

pub(crate) fn derive_identity(
    domain: &[u8],
    bytes: &[u8],
    field: &'static str,
) -> Result<[u8; 32], LineageErrorV3> {
    let byte_len = u64::try_from(bytes.len()).map_err(|_| LineageErrorV3::LengthOverflow)?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(byte_len.to_le_bytes());
    digest.update(bytes);
    let sha256: [u8; 32] = digest.finalize().into();
    if sha256 == [0; 32] {
        return Err(LineageErrorV3::ZeroIdentity { field });
    }
    Ok(sha256)
}

macro_rules! define_receipt {
    (
        $(#[$identity_meta:meta])*
        $identity:ident,
        $(#[$receipt_meta:meta])*
        $receipt:ident,
        $field:literal,
        $domain:literal,
        $max:expr
    ) => {
        $(#[$identity_meta])*
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $identity {
            sha256: [u8; 32],
            byte_len: u64,
        }

        impl $identity {
            /// Returns the domain-separated SHA-256 identity bytes.
            pub const fn sha256(&self) -> &[u8; 32] {
                &self.sha256
            }

            /// Returns the exact canonical preimage length.
            pub const fn byte_len(self) -> u64 {
                self.byte_len
            }
        }

        impl fmt::Debug for $identity {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($identity))
                    .field("sha256", &self.sha256)
                    .field("byte_len", &self.byte_len)
                    .finish()
            }
        }

        $(#[$receipt_meta])*
        pub struct $receipt {
            canonical_preimage: ImmutableBytesV3,
            identity: $identity,
        }

        impl PartialEq for $receipt {
            fn eq(&self, other: &Self) -> bool {
                self.identity == other.identity
                    && self.canonical_preimage == other.canonical_preimage
            }
        }

        impl Eq for $receipt {}

        impl $receipt {
            const DOMAIN: &'static [u8] = $domain;
            pub(crate) const FIELD: &'static str = $field;
            pub(crate) const MAX_BYTES: usize = $max;

            /// Retains caller-supplied opaque stage content and derives its inert identity.
            ///
            /// This dependency-light layer does not parse the stage-specific content,
            /// authenticate its producer, establish derivation, or grant authority. A later
            /// typed producer integration must supply and authenticate canonical bytes.
            pub fn from_canonical_preimage(
                canonical_preimage: impl Into<Vec<u8>>,
            ) -> Result<Self, LineageErrorV3> {
                let canonical_preimage = canonical_preimage.into();
                if canonical_preimage.is_empty() {
                    return Err(LineageErrorV3::EmptyPreimage { field: Self::FIELD });
                }
                if canonical_preimage.len() > Self::MAX_BYTES {
                    return Err(LineageErrorV3::PreimageTooLarge {
                        field: Self::FIELD,
                        max: Self::MAX_BYTES,
                    });
                }
                let sha256 = derive_identity(Self::DOMAIN, &canonical_preimage, Self::FIELD)?;
                let byte_len = u64::try_from(canonical_preimage.len())
                    .map_err(|_| LineageErrorV3::LengthOverflow)?;
                Ok(Self {
                    canonical_preimage: ImmutableBytesV3::from_owned(
                        canonical_preimage.into_boxed_slice(),
                    ),
                    identity: $identity { sha256, byte_len },
                })
            }

            /// Returns the exact retained caller-supplied stage content.
            pub fn canonical_preimage(&self) -> &[u8] {
                self.canonical_preimage.as_slice()
            }

            /// Returns the inert typed identity derived from the retained content.
            pub const fn identity(&self) -> $identity {
                self.identity
            }

            pub(crate) fn decode_shared(
                backing: SharedBackingV3,
                range: Range<usize>,
                declared_identity: [u8; 32],
            ) -> Result<Self, LineageDecodeErrorV3> {
                let canonical_preimage = ImmutableBytesV3::from_shared(backing, range)
                    .ok_or(LineageDecodeErrorV3::Truncated)?;
                let bytes = canonical_preimage.as_slice();
                if bytes.is_empty() {
                    return Err(LineageDecodeErrorV3::EmptyPreimage { field: Self::FIELD });
                }
                if bytes.len() > Self::MAX_BYTES {
                    return Err(LineageDecodeErrorV3::PreimageTooLarge {
                        field: Self::FIELD,
                        max: Self::MAX_BYTES,
                    });
                }
                if declared_identity == [0; 32] {
                    return Err(LineageDecodeErrorV3::ZeroIdentity { field: Self::FIELD });
                }
                let sha256 = derive_identity(Self::DOMAIN, bytes, Self::FIELD).map_err(|error| {
                    match error {
                        LineageErrorV3::ZeroIdentity { field } => {
                            LineageDecodeErrorV3::ZeroIdentity { field }
                        }
                        _ => LineageDecodeErrorV3::NonCanonical,
                    }
                })?;
                if sha256 != declared_identity {
                    return Err(LineageDecodeErrorV3::ReceiptIdentityMismatch {
                        field: Self::FIELD,
                    });
                }
                let byte_len =
                    u64::try_from(bytes.len()).map_err(|_| LineageDecodeErrorV3::NonCanonical)?;
                Ok(Self {
                    canonical_preimage,
                    identity: $identity { sha256, byte_len },
                })
            }
        }

        impl fmt::Debug for $receipt {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($receipt))
                    .field("identity", &self.identity)
                    .finish_non_exhaustive()
            }
        }
    };
}

define_receipt!(
    /// Inert content identity of one canonical rustc identity-inventory transcript.
    InertRustcIdentityInventoryReceiptIdentityV3,
    /// Inert canonical rustc identity-inventory content receipt.
    InertRustcIdentityInventoryReceiptV3,
    "rustc identity inventory",
    b"FE2O3/INERT-LINEAGE-CONTENT/RUSTC-IDENTITY-INVENTORY/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one canonical rustc preflight-plan transcript.
    InertRustcPreflightPlanReceiptIdentityV3,
    /// Inert canonical rustc preflight-plan content receipt.
    InertRustcPreflightPlanReceiptV3,
    "rustc preflight plan",
    b"FE2O3/INERT-LINEAGE-CONTENT/RUSTC-PREFLIGHT-PLAN/V3\0",
    MAX_RUSTC_PREFLIGHT_PLAN_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one canonical semantic-MIR transcript.
    InertCanonicalSemanticMirIdentityV3,
    /// Inert canonical semantic-MIR content receipt.
    InertCanonicalSemanticMirReceiptV3,
    "canonical semantic MIR",
    b"FE2O3/INERT-LINEAGE-CONTENT/CANONICAL-SEMANTIC-MIR/V3\0",
    MAX_CANONICAL_SEMANTIC_MIR_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one ordered middle-end pass-chain transcript.
    InertMiddleEndReceiptIdentityV3,
    /// Inert ordered middle-end pass-chain content receipt.
    InertMiddleEndReceiptV3,
    "middle-end pass chain",
    b"FE2O3/INERT-LINEAGE-CONTENT/MIDDLE-END-PASS-CHAIN/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one canonical Kernel IR transcript.
    InertKernelIrReceiptIdentityV3,
    /// Inert canonical Kernel IR content receipt.
    InertKernelIrReceiptV3,
    "canonical Kernel IR",
    b"FE2O3/INERT-LINEAGE-CONTENT/CANONICAL-KERNEL-IR/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one MIR-to-KIR correspondence transcript.
    InertMirToKirCorrespondenceReceiptIdentityV3,
    /// Inert MIR-to-KIR correspondence content receipt.
    InertMirToKirCorrespondenceReceiptV3,
    "MIR-to-KIR correspondence",
    b"FE2O3/INERT-LINEAGE-CONTENT/MIR-TO-KIR-CORRESPONDENCE/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one formal-memory obligation transcript.
    InertFormalMemoryReceiptIdentityV3,
    /// Inert formal-memory obligation content receipt.
    InertFormalMemoryReceiptV3,
    "formal memory obligations",
    b"FE2O3/INERT-LINEAGE-CONTENT/FORMAL-MEMORY-OBLIGATIONS/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one proof-binding transcript.
    InertProofBindingReceiptIdentityV3,
    /// Inert proof-binding content receipt.
    InertProofBindingReceiptV3,
    "proof binding set",
    b"FE2O3/INERT-LINEAGE-CONTENT/PROOF-BINDING-SET/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one target-binding transcript.
    InertTargetBindingReceiptIdentityV3,
    /// Inert target-binding content receipt.
    InertTargetBindingReceiptV3,
    "target binding",
    b"FE2O3/INERT-LINEAGE-CONTENT/TARGET-BINDING/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one target data-layout transcript.
    InertDataLayoutReceiptIdentityV3,
    /// Inert target data-layout content receipt.
    InertDataLayoutReceiptV3,
    "target data layout",
    b"FE2O3/INERT-LINEAGE-CONTENT/TARGET-DATA-LAYOUT/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one ABI transcript.
    InertAbiReceiptIdentityV3,
    /// Inert ABI content receipt.
    InertAbiReceiptV3,
    "ABI",
    b"FE2O3/INERT-LINEAGE-CONTENT/ABI/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one export-manifest transcript.
    InertExportManifestReceiptIdentityV3,
    /// Inert export-manifest content receipt.
    InertExportManifestReceiptV3,
    "export manifest",
    b"FE2O3/INERT-LINEAGE-CONTENT/EXPORT-MANIFEST/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one AMDGPU lowering transcript.
    InertAmdgpuLoweringReceiptIdentityV3,
    /// Inert AMDGPU lowering content receipt.
    InertAmdgpuLoweringReceiptV3,
    "AMDGPU lowering",
    b"FE2O3/INERT-LINEAGE-CONTENT/AMDGPU-LOWERING/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one semantic-to-LLVM derivation transcript.
    InertSemanticToLlvmReceiptIdentityV3,
    /// Inert semantic-to-LLVM derivation content receipt.
    InertSemanticToLlvmReceiptV3,
    "semantic-to-LLVM derivation",
    b"FE2O3/INERT-LINEAGE-CONTENT/SEMANTIC-TO-LLVM/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
define_receipt!(
    /// Inert content identity of one compact final compiler-module commitment.
    InertFinalCompilerModuleCommitmentIdentityV3,
    /// Inert compact final compiler-module commitment content receipt.
    InertFinalCompilerModuleCommitmentReceiptV3,
    "final compiler module commitment",
    b"FE2O3/INERT-LINEAGE-CONTENT/FINAL-COMPILER-MODULE-COMMITMENT/V3\0",
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
);
