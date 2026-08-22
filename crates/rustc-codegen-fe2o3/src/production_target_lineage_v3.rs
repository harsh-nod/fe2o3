//! Canonical target-side lineage transcripts for the production V3 pipeline.
//!
//! These records retain exact, bounded inputs and associate identities produced
//! by adjacent compiler stages. They are deliberately inert. In particular,
//! they do not authenticate a producer, prove that AMDGPU lowering refines
//! semantic MIR or Kernel IR, or grant publication, link, load, or launch
//! authority. Those properties require a private join over live, move-only
//! compiler owners.

use std::{error::Error, fmt, str};

use fe2o3_compiler_ffi::{CompilerDescriptorSourceV1, CompilerModuleSymbolManifestV1};

const TRANSCRIPT_MAGIC_V3: [u8; 8] = *b"F2O3TLV3";
const TRANSCRIPT_VERSION_V3: u16 = 3;
const TRANSCRIPT_HEADER_BYTES_V3: usize = 24;
const FIELD_HEADER_BYTES_V3: usize = 8;
const IDENTITY_BYTES_V3: usize = 32 + 8;

/// Maximum canonical size accepted for any one target-lineage receipt preimage.
pub(crate) const MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3: usize = 4 * 1024 * 1024;
/// Maximum exact LLVM text retained before descriptor publication.
pub(crate) const MAX_PRE_DESCRIPTOR_LLVM_BYTES_V3: usize =
    MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3 - 1024;

const MAX_TARGET_TEXT_BYTES_V3: usize = 256;
const MAX_TARGET_FEATURES_BYTES_V3: usize = 4 * 1024;
const MAX_DATA_LAYOUT_BYTES_V3: usize = 16 * 1024;
const MAX_COMPILER_FFI_ENVELOPE_BYTES_V3: usize = 512 * 1024;
const MAX_COMPILER_DESCRIPTOR_SOURCE_BYTES_V3: usize = 256 * 1024;

const TARGET_BINDING_KIND_V3: u16 = 1;
const DATA_LAYOUT_KIND_V3: u16 = 2;
const ABI_KIND_V3: u16 = 3;
const AMDGPU_LOWERING_KIND_V3: u16 = 4;
const SEMANTIC_TO_LLVM_KIND_V3: u16 = 5;
const PROOF_BINDING_KIND_V3: u16 = 6;
const EXPORT_MANIFEST_KIND_V3: u16 = 7;

/// Exact-input association policy. It intentionally makes no refinement claim.
pub(crate) const ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3: u16 = 1;

const ASSOCIATION_ONLY_CLAIM_V3: &[u8] = b"association-only/no-refinement-proof";
const TARGET_BINDING_DOMAIN_V3: &[u8] = b"FE2O3/PRODUCTION-TARGET-BINDING-TRANSCRIPT/V3\0";
const DATA_LAYOUT_DOMAIN_V3: &[u8] = b"FE2O3/PRODUCTION-DATA-LAYOUT-TRANSCRIPT/V3\0";
const ABI_DOMAIN_V3: &[u8] = b"FE2O3/PRODUCTION-ABI-TRANSCRIPT/V3\0";
const AMDGPU_LOWERING_DOMAIN_V3: &[u8] = b"FE2O3/PRODUCTION-AMDGPU-LOWERING-TRANSCRIPT/V3\0";
const SEMANTIC_TO_LLVM_DOMAIN_V3: &[u8] = b"FE2O3/PRODUCTION-SEMANTIC-TO-LLVM-ASSOCIATION/V3\0";
const PROOF_BINDING_DOMAIN_V3: &[u8] = b"FE2O3/PRODUCTION-PROOF-BINDING-ASSOCIATION/V3\0";
const EXPORT_MANIFEST_DOMAIN_V3: &[u8] = b"FE2O3/PRODUCTION-EXPORT-MANIFEST-ASSOCIATION/V3\0";

const EXACT_GFX942_TARGET_V3: &str = "gfx942:xnack-";
const EXACT_RUSTC_LLVM_TARGET_V3: &str = "amdgcn-amd-amdhsa";
const EXACT_GFX942_CPU_V3: &str = "gfx942";
const EXACT_GFX942_FEATURES_V3: &str = "-wavefrontsize32,+wavefrontsize64,-xnack";
const EXACT_CODE_OBJECT_VERSION_V3: u16 = 6;
const EXACT_WAVE_WIDTH_BITS_V3: u16 = 64;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct TargetLineageIdentityV3 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl TargetLineageIdentityV3 {
    pub(crate) fn new(
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        validate_identity("lineage identity", sha256, byte_len)?;
        Ok(Self { sha256, byte_len })
    }

    pub(crate) const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub(crate) const fn byte_len(self) -> u64 {
        self.byte_len
    }

    fn encode(self) -> [u8; IDENTITY_BYTES_V3] {
        let mut encoded = [0_u8; IDENTITY_BYTES_V3];
        encoded[..32].copy_from_slice(&self.sha256);
        encoded[32..].copy_from_slice(&self.byte_len.to_le_bytes());
        encoded
    }

    fn decode(field: &'static str, encoded: &[u8]) -> Result<Self, ProductionTargetLineageErrorV3> {
        if encoded.len() != IDENTITY_BYTES_V3 {
            return Err(ProductionTargetLineageErrorV3::InvalidFieldLength {
                field,
                actual: encoded.len(),
                expected: IDENTITY_BYTES_V3,
            });
        }
        let mut sha256 = [0_u8; 32];
        sha256.copy_from_slice(&encoded[..32]);
        let mut byte_len = [0_u8; 8];
        byte_len.copy_from_slice(&encoded[32..]);
        let byte_len = u64::from_le_bytes(byte_len);
        validate_identity(field, sha256, byte_len)?;
        Ok(Self { sha256, byte_len })
    }
}

/// The strongest semantic statement made by records in this module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TargetLineageClaimV3 {
    AssociationOnlyNoRefinementProof,
}

#[derive(Clone, Copy)]
struct FieldSchemaV3 {
    name: &'static str,
    max_bytes: usize,
    exact_bytes: Option<usize>,
}

impl FieldSchemaV3 {
    const fn bounded(name: &'static str, max_bytes: usize) -> Self {
        Self {
            name,
            max_bytes,
            exact_bytes: None,
        }
    }

    const fn exact(name: &'static str, exact_bytes: usize) -> Self {
        Self {
            name,
            max_bytes: exact_bytes,
            exact_bytes: Some(exact_bytes),
        }
    }
}

struct RecordSchemaV3 {
    name: &'static str,
    kind: u16,
    policy: u16,
    domain: &'static [u8],
    fields: &'static [FieldSchemaV3],
}

const TARGET_BINDING_FIELDS_V3: &[FieldSchemaV3] = &[
    FieldSchemaV3::exact("domain", TARGET_BINDING_DOMAIN_V3.len()),
    FieldSchemaV3::exact("claim", ASSOCIATION_ONLY_CLAIM_V3.len()),
    FieldSchemaV3::exact("protected rustc invocation identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("semantic MIR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("target-neutral Kernel IR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("target-bound Kernel IR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::bounded("configured target", MAX_TARGET_TEXT_BYTES_V3),
    FieldSchemaV3::bounded("rustc LLVM target", MAX_TARGET_TEXT_BYTES_V3),
    FieldSchemaV3::bounded("target CPU", MAX_TARGET_TEXT_BYTES_V3),
    FieldSchemaV3::bounded("target features", MAX_TARGET_FEATURES_BYTES_V3),
    FieldSchemaV3::exact("code object version", 2),
    FieldSchemaV3::exact("wave width", 2),
    FieldSchemaV3::exact("default workgroup", 12),
];

const DATA_LAYOUT_FIELDS_V3: &[FieldSchemaV3] = &[
    FieldSchemaV3::exact("domain", DATA_LAYOUT_DOMAIN_V3.len()),
    FieldSchemaV3::exact("claim", ASSOCIATION_ONLY_CLAIM_V3.len()),
    FieldSchemaV3::exact("semantic MIR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("target binding identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("semantic layout identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::bounded("rustc LLVM target", MAX_TARGET_TEXT_BYTES_V3),
    FieldSchemaV3::bounded("live rustc data layout", MAX_DATA_LAYOUT_BYTES_V3),
    FieldSchemaV3::bounded("final LLVM target", MAX_TARGET_TEXT_BYTES_V3),
    FieldSchemaV3::bounded("final LLVM data layout", MAX_DATA_LAYOUT_BYTES_V3),
    FieldSchemaV3::exact("default pointer width", 2),
];

const ABI_FIELDS_V3: &[FieldSchemaV3] = &[
    FieldSchemaV3::exact("domain", ABI_DOMAIN_V3.len()),
    FieldSchemaV3::exact("claim", ASSOCIATION_ONLY_CLAIM_V3.len()),
    FieldSchemaV3::exact("semantic MIR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("target-bound Kernel IR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("formal memory identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::bounded("compiler FFI envelope", MAX_COMPILER_FFI_ENVELOPE_BYTES_V3),
    FieldSchemaV3::bounded(
        "compiler descriptor source",
        MAX_COMPILER_DESCRIPTOR_SOURCE_BYTES_V3,
    ),
];

const AMDGPU_LOWERING_FIELDS_V3: &[FieldSchemaV3] = &[
    FieldSchemaV3::exact("domain", AMDGPU_LOWERING_DOMAIN_V3.len()),
    FieldSchemaV3::exact("claim", ASSOCIATION_ONLY_CLAIM_V3.len()),
    FieldSchemaV3::exact("target binding identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("data layout identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("target-bound Kernel IR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::bounded("configured target", MAX_TARGET_TEXT_BYTES_V3),
    FieldSchemaV3::bounded(
        "exact pre-descriptor LLVM",
        MAX_PRE_DESCRIPTOR_LLVM_BYTES_V3,
    ),
];

const SEMANTIC_TO_LLVM_FIELDS_V3: &[FieldSchemaV3] = &[
    FieldSchemaV3::exact("domain", SEMANTIC_TO_LLVM_DOMAIN_V3.len()),
    FieldSchemaV3::exact("claim", ASSOCIATION_ONLY_CLAIM_V3.len()),
    FieldSchemaV3::exact("semantic MIR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("middle-end identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("Kernel IR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("MIR-to-KIR correspondence identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("formal memory identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("proof binding identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("target binding identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("data layout identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("ABI identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("export manifest identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("AMDGPU lowering identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("final LLVM identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact(
        "final compiler module commitment identity",
        IDENTITY_BYTES_V3,
    ),
];

const PROOF_BINDING_FIELDS_V3: &[FieldSchemaV3] = &[
    FieldSchemaV3::exact("domain", PROOF_BINDING_DOMAIN_V3.len()),
    FieldSchemaV3::exact("claim", ASSOCIATION_ONLY_CLAIM_V3.len()),
    FieldSchemaV3::exact("semantic MIR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("middle-end identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("Kernel IR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("MIR-to-KIR correspondence identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("formal memory identity", IDENTITY_BYTES_V3),
];

const EXPORT_MANIFEST_FIELDS_V3: &[FieldSchemaV3] = &[
    FieldSchemaV3::exact("domain", EXPORT_MANIFEST_DOMAIN_V3.len()),
    FieldSchemaV3::exact("claim", ASSOCIATION_ONLY_CLAIM_V3.len()),
    FieldSchemaV3::exact("semantic MIR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("target-bound Kernel IR identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::exact("ABI identity", IDENTITY_BYTES_V3),
    FieldSchemaV3::bounded(
        "compiler descriptor source",
        MAX_COMPILER_DESCRIPTOR_SOURCE_BYTES_V3,
    ),
    FieldSchemaV3::bounded(
        "final compiler symbol manifest",
        MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3,
    ),
];

const TARGET_BINDING_SCHEMA_V3: RecordSchemaV3 = RecordSchemaV3 {
    name: "target binding transcript",
    kind: TARGET_BINDING_KIND_V3,
    policy: ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3,
    domain: TARGET_BINDING_DOMAIN_V3,
    fields: TARGET_BINDING_FIELDS_V3,
};
const DATA_LAYOUT_SCHEMA_V3: RecordSchemaV3 = RecordSchemaV3 {
    name: "data layout transcript",
    kind: DATA_LAYOUT_KIND_V3,
    policy: ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3,
    domain: DATA_LAYOUT_DOMAIN_V3,
    fields: DATA_LAYOUT_FIELDS_V3,
};
const ABI_SCHEMA_V3: RecordSchemaV3 = RecordSchemaV3 {
    name: "ABI transcript",
    kind: ABI_KIND_V3,
    policy: ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3,
    domain: ABI_DOMAIN_V3,
    fields: ABI_FIELDS_V3,
};
const AMDGPU_LOWERING_SCHEMA_V3: RecordSchemaV3 = RecordSchemaV3 {
    name: "AMDGPU lowering transcript",
    kind: AMDGPU_LOWERING_KIND_V3,
    policy: ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3,
    domain: AMDGPU_LOWERING_DOMAIN_V3,
    fields: AMDGPU_LOWERING_FIELDS_V3,
};
const SEMANTIC_TO_LLVM_SCHEMA_V3: RecordSchemaV3 = RecordSchemaV3 {
    name: "semantic-to-LLVM association",
    kind: SEMANTIC_TO_LLVM_KIND_V3,
    policy: ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3,
    domain: SEMANTIC_TO_LLVM_DOMAIN_V3,
    fields: SEMANTIC_TO_LLVM_FIELDS_V3,
};
const PROOF_BINDING_SCHEMA_V3: RecordSchemaV3 = RecordSchemaV3 {
    name: "proof binding association",
    kind: PROOF_BINDING_KIND_V3,
    policy: ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3,
    domain: PROOF_BINDING_DOMAIN_V3,
    fields: PROOF_BINDING_FIELDS_V3,
};
const EXPORT_MANIFEST_SCHEMA_V3: RecordSchemaV3 = RecordSchemaV3 {
    name: "export manifest association",
    kind: EXPORT_MANIFEST_KIND_V3,
    policy: ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3,
    domain: EXPORT_MANIFEST_DOMAIN_V3,
    fields: EXPORT_MANIFEST_FIELDS_V3,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FieldRangeV3 {
    start: u32,
    end: u32,
}

#[derive(Eq, PartialEq)]
struct CanonicalRecordV3 {
    canonical_bytes: Box<[u8]>,
    fields: Box<[FieldRangeV3]>,
}

impl fmt::Debug for CanonicalRecordV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalRecordV3")
            .field("byte_len", &self.canonical_bytes.len())
            .field("field_count", &self.fields.len())
            .finish()
    }
}

impl CanonicalRecordV3 {
    fn build(
        schema: &'static RecordSchemaV3,
        fields: &[&[u8]],
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        validate_field_count(schema, fields.len())?;
        validate_fields(schema, fields)?;
        let total_len = preflight_encoded_len(schema, fields)?;
        let total_len_u32 =
            u32::try_from(total_len).map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?;
        let field_count = u16::try_from(fields.len())
            .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?;

        let mut canonical_bytes = Vec::new();
        canonical_bytes
            .try_reserve_exact(total_len)
            .map_err(|_| ProductionTargetLineageErrorV3::AllocationFailed)?;
        canonical_bytes.extend_from_slice(&TRANSCRIPT_MAGIC_V3);
        canonical_bytes.extend_from_slice(&TRANSCRIPT_VERSION_V3.to_le_bytes());
        canonical_bytes.extend_from_slice(&schema.kind.to_le_bytes());
        canonical_bytes.extend_from_slice(&schema.policy.to_le_bytes());
        canonical_bytes.extend_from_slice(&field_count.to_le_bytes());
        canonical_bytes.extend_from_slice(&total_len_u32.to_le_bytes());
        canonical_bytes.extend_from_slice(&0_u32.to_le_bytes());

        let mut ranges = Vec::new();
        ranges
            .try_reserve_exact(fields.len())
            .map_err(|_| ProductionTargetLineageErrorV3::AllocationFailed)?;
        for (index, field) in fields.iter().enumerate() {
            let tag = u16::try_from(index + 1)
                .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?;
            let field_len = u32::try_from(field.len())
                .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?;
            canonical_bytes.extend_from_slice(&tag.to_le_bytes());
            canonical_bytes.extend_from_slice(&0_u16.to_le_bytes());
            canonical_bytes.extend_from_slice(&field_len.to_le_bytes());
            let start = u32::try_from(canonical_bytes.len())
                .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?;
            canonical_bytes.extend_from_slice(field);
            let end = u32::try_from(canonical_bytes.len())
                .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?;
            ranges.push(FieldRangeV3 { start, end });
        }
        debug_assert_eq!(canonical_bytes.len(), total_len);
        Ok(Self {
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            fields: ranges.into_boxed_slice(),
        })
    }

    fn decode(
        schema: &'static RecordSchemaV3,
        bytes: &[u8],
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        let parsed = ParsedRecordV3::parse(schema, bytes)?;
        validate_fields(schema, &parsed.field_slices())?;
        parsed.into_owned()
    }

    fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    fn field(&self, index: usize) -> &[u8] {
        let range = self.fields[index];
        &self.canonical_bytes[range.start as usize..range.end as usize]
    }
}

struct ParsedRecordV3<'a> {
    bytes: &'a [u8],
    ranges: Vec<FieldRangeV3>,
}

impl<'a> ParsedRecordV3<'a> {
    fn parse(
        schema: &'static RecordSchemaV3,
        bytes: &'a [u8],
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        if bytes.len() > MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3 {
            return Err(ProductionTargetLineageErrorV3::TranscriptTooLarge {
                actual: bytes.len(),
                max: MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3,
            });
        }
        if bytes.len() < TRANSCRIPT_HEADER_BYTES_V3 {
            return Err(ProductionTargetLineageErrorV3::Truncated);
        }
        if bytes[..8] != TRANSCRIPT_MAGIC_V3 {
            return Err(ProductionTargetLineageErrorV3::InvalidMagic);
        }
        let version = read_u16(bytes, 8)?;
        if version != TRANSCRIPT_VERSION_V3 {
            return Err(ProductionTargetLineageErrorV3::UnsupportedVersion { observed: version });
        }
        let kind = read_u16(bytes, 10)?;
        if kind != schema.kind {
            return Err(ProductionTargetLineageErrorV3::WrongRecordKind {
                expected: schema.kind,
                observed: kind,
            });
        }
        let policy = read_u16(bytes, 12)?;
        if policy != schema.policy {
            return Err(ProductionTargetLineageErrorV3::WrongPolicy {
                expected: schema.policy,
                observed: policy,
            });
        }
        let field_count = usize::from(read_u16(bytes, 14)?);
        validate_field_count(schema, field_count)?;
        let declared_len = usize::try_from(read_u32(bytes, 16)?)
            .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?;
        if declared_len != bytes.len() {
            return Err(ProductionTargetLineageErrorV3::DeclaredLengthMismatch {
                declared: declared_len,
                actual: bytes.len(),
            });
        }
        if read_u32(bytes, 20)? != 0 {
            return Err(ProductionTargetLineageErrorV3::NonZeroReserved);
        }

        let minimum_len = TRANSCRIPT_HEADER_BYTES_V3
            .checked_add(
                field_count
                    .checked_mul(FIELD_HEADER_BYTES_V3)
                    .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?,
            )
            .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
        if declared_len < minimum_len {
            return Err(ProductionTargetLineageErrorV3::Truncated);
        }

        let mut offset = TRANSCRIPT_HEADER_BYTES_V3;
        let mut ranges = Vec::new();
        ranges
            .try_reserve_exact(field_count)
            .map_err(|_| ProductionTargetLineageErrorV3::AllocationFailed)?;
        for (index, field_schema) in schema.fields.iter().enumerate() {
            let expected_tag = u16::try_from(index + 1)
                .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?;
            let tag = read_u16(bytes, offset)?;
            if tag != expected_tag {
                return Err(ProductionTargetLineageErrorV3::WrongFieldTag {
                    field: field_schema.name,
                    expected: expected_tag,
                    observed: tag,
                });
            }
            if read_u16(bytes, offset + 2)? != 0 {
                return Err(ProductionTargetLineageErrorV3::NonZeroFieldFlags {
                    field: field_schema.name,
                });
            }
            let field_len = usize::try_from(read_u32(bytes, offset + 4)?)
                .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?;
            validate_field_len(field_schema, field_len)?;
            let start = offset
                .checked_add(FIELD_HEADER_BYTES_V3)
                .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
            let end = start
                .checked_add(field_len)
                .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
            if end > bytes.len() {
                return Err(ProductionTargetLineageErrorV3::Truncated);
            }
            ranges.push(FieldRangeV3 {
                start: u32::try_from(start)
                    .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?,
                end: u32::try_from(end)
                    .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?,
            });
            offset = end;
        }
        if offset != bytes.len() {
            return Err(ProductionTargetLineageErrorV3::TrailingBytes {
                trailing: bytes.len() - offset,
            });
        }
        Ok(Self { bytes, ranges })
    }

    fn field_slices(&self) -> Vec<&[u8]> {
        self.ranges
            .iter()
            .map(|range| &self.bytes[range.start as usize..range.end as usize])
            .collect()
    }

    fn into_owned(self) -> Result<CanonicalRecordV3, ProductionTargetLineageErrorV3> {
        let mut canonical_bytes = Vec::new();
        canonical_bytes
            .try_reserve_exact(self.bytes.len())
            .map_err(|_| ProductionTargetLineageErrorV3::AllocationFailed)?;
        canonical_bytes.extend_from_slice(self.bytes);
        Ok(CanonicalRecordV3 {
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            fields: self.ranges.into_boxed_slice(),
        })
    }
}

fn validate_field_count(
    schema: &RecordSchemaV3,
    actual: usize,
) -> Result<(), ProductionTargetLineageErrorV3> {
    if actual == schema.fields.len() {
        Ok(())
    } else {
        Err(ProductionTargetLineageErrorV3::WrongFieldCount {
            record: schema.name,
            expected: schema.fields.len(),
            observed: actual,
        })
    }
}

fn validate_fields(
    schema: &RecordSchemaV3,
    fields: &[&[u8]],
) -> Result<(), ProductionTargetLineageErrorV3> {
    validate_field_count(schema, fields.len())?;
    for (field_schema, field) in schema.fields.iter().zip(fields) {
        validate_field_len(field_schema, field.len())?;
    }
    if fields[0] != schema.domain {
        return Err(ProductionTargetLineageErrorV3::WrongDomain {
            record: schema.name,
        });
    }
    if fields[1] != ASSOCIATION_ONLY_CLAIM_V3 {
        return Err(ProductionTargetLineageErrorV3::WrongClaim {
            record: schema.name,
        });
    }
    Ok(())
}

fn validate_field_len(
    schema: &FieldSchemaV3,
    actual: usize,
) -> Result<(), ProductionTargetLineageErrorV3> {
    if actual == 0 {
        return Err(ProductionTargetLineageErrorV3::EmptyField { field: schema.name });
    }
    if actual > schema.max_bytes {
        return Err(ProductionTargetLineageErrorV3::FieldTooLarge {
            field: schema.name,
            actual,
            max: schema.max_bytes,
        });
    }
    if let Some(expected) = schema.exact_bytes
        && actual != expected
    {
        return Err(ProductionTargetLineageErrorV3::InvalidFieldLength {
            field: schema.name,
            actual,
            expected,
        });
    }
    Ok(())
}

fn preflight_encoded_len(
    schema: &RecordSchemaV3,
    fields: &[&[u8]],
) -> Result<usize, ProductionTargetLineageErrorV3> {
    let field_headers = FIELD_HEADER_BYTES_V3
        .checked_mul(schema.fields.len())
        .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
    let mut total = TRANSCRIPT_HEADER_BYTES_V3
        .checked_add(field_headers)
        .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
    for field in fields {
        total = total
            .checked_add(field.len())
            .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
    }
    if total > MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3 {
        return Err(ProductionTargetLineageErrorV3::TranscriptTooLarge {
            actual: total,
            max: MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3,
        });
    }
    Ok(total)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ProductionTargetLineageErrorV3> {
    let end = offset
        .checked_add(2)
        .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
    let encoded = bytes
        .get(offset..end)
        .ok_or(ProductionTargetLineageErrorV3::Truncated)?;
    Ok(u16::from_le_bytes([encoded[0], encoded[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ProductionTargetLineageErrorV3> {
    let end = offset
        .checked_add(4)
        .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
    let encoded = bytes
        .get(offset..end)
        .ok_or(ProductionTargetLineageErrorV3::Truncated)?;
    Ok(u32::from_le_bytes([
        encoded[0], encoded[1], encoded[2], encoded[3],
    ]))
}

fn decode_u16(field: &'static str, bytes: &[u8]) -> Result<u16, ProductionTargetLineageErrorV3> {
    if bytes.len() != 2 {
        return Err(ProductionTargetLineageErrorV3::InvalidFieldLength {
            field,
            actual: bytes.len(),
            expected: 2,
        });
    }
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn validate_identity(
    field: &'static str,
    sha256: [u8; 32],
    byte_len: u64,
) -> Result<(), ProductionTargetLineageErrorV3> {
    if sha256 == [0; 32] {
        return Err(ProductionTargetLineageErrorV3::ZeroIdentity { field });
    }
    if byte_len == 0 {
        return Err(ProductionTargetLineageErrorV3::ZeroIdentityLength { field });
    }
    Ok(())
}

fn validate_ascii_token<'a>(
    field: &'static str,
    bytes: &'a [u8],
) -> Result<&'a str, ProductionTargetLineageErrorV3> {
    let text =
        str::from_utf8(bytes).map_err(|_| ProductionTargetLineageErrorV3::InvalidText { field })?;
    if text.is_empty()
        || !text.is_ascii()
        || text
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err(ProductionTargetLineageErrorV3::InvalidText { field });
    }
    Ok(text)
}

fn validate_llvm_text(bytes: &[u8]) -> Result<&str, ProductionTargetLineageErrorV3> {
    let text = str::from_utf8(bytes).map_err(|_| ProductionTargetLineageErrorV3::InvalidText {
        field: "exact pre-descriptor LLVM",
    })?;
    if text.is_empty() || text.as_bytes().contains(&0) || text.as_bytes().contains(&b'\r') {
        return Err(ProductionTargetLineageErrorV3::InvalidText {
            field: "exact pre-descriptor LLVM",
        });
    }
    Ok(text)
}

fn require_exact_text(
    field: &'static str,
    observed: &str,
    expected: &'static str,
) -> Result<(), ProductionTargetLineageErrorV3> {
    if observed == expected {
        Ok(())
    } else {
        Err(ProductionTargetLineageErrorV3::ExactValueMismatch { field })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TargetBindingTranscriptInputsV3<'a> {
    pub(crate) protected_rustc_invocation: TargetLineageIdentityV3,
    pub(crate) semantic_mir: TargetLineageIdentityV3,
    pub(crate) target_neutral_kir: TargetLineageIdentityV3,
    pub(crate) target_bound_kir: TargetLineageIdentityV3,
    pub(crate) configured_target: &'a str,
    pub(crate) rustc_llvm_target: &'a str,
    pub(crate) target_cpu: &'a str,
    pub(crate) target_features: &'a str,
    pub(crate) code_object_version: u16,
    pub(crate) wave_width_bits: u16,
    pub(crate) default_workgroup: [u32; 3],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct TargetBindingTranscriptV3 {
    record: CanonicalRecordV3,
}

impl TargetBindingTranscriptV3 {
    pub(crate) fn new(
        inputs: TargetBindingTranscriptInputsV3<'_>,
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        validate_target_binding_inputs_v3(&inputs)?;

        let invocation = inputs.protected_rustc_invocation.encode();
        let semantic_mir = inputs.semantic_mir.encode();
        let neutral_kir = inputs.target_neutral_kir.encode();
        let bound_kir = inputs.target_bound_kir.encode();
        let code_object_version = inputs.code_object_version.to_le_bytes();
        let wave_width_bits = inputs.wave_width_bits.to_le_bytes();
        let mut default_workgroup = [0_u8; 12];
        for (index, dimension) in inputs.default_workgroup.iter().enumerate() {
            let start = index * 4;
            default_workgroup[start..start + 4].copy_from_slice(&dimension.to_le_bytes());
        }
        let fields: [&[u8]; 13] = [
            TARGET_BINDING_DOMAIN_V3,
            ASSOCIATION_ONLY_CLAIM_V3,
            &invocation,
            &semantic_mir,
            &neutral_kir,
            &bound_kir,
            inputs.configured_target.as_bytes(),
            inputs.rustc_llvm_target.as_bytes(),
            inputs.target_cpu.as_bytes(),
            inputs.target_features.as_bytes(),
            &code_object_version,
            &wave_width_bits,
            &default_workgroup,
        ];
        Ok(Self {
            record: CanonicalRecordV3::build(&TARGET_BINDING_SCHEMA_V3, &fields)?,
        })
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ProductionTargetLineageErrorV3> {
        let record = CanonicalRecordV3::decode(&TARGET_BINDING_SCHEMA_V3, bytes)?;
        let value = Self { record };
        validate_target_binding_inputs_v3(&value.inputs()?)?;
        Ok(value)
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        self.record.canonical_bytes()
    }

    pub(crate) const fn claim(&self) -> TargetLineageClaimV3 {
        TargetLineageClaimV3::AssociationOnlyNoRefinementProof
    }

    pub(crate) const fn establishes_refinement_proof(&self) -> bool {
        false
    }

    pub(crate) fn inputs(
        &self,
    ) -> Result<TargetBindingTranscriptInputsV3<'_>, ProductionTargetLineageErrorV3> {
        let mut default_workgroup = [0_u32; 3];
        for (index, dimension) in default_workgroup.iter_mut().enumerate() {
            let start = index * 4;
            let bytes = &self.record.field(12)[start..start + 4];
            *dimension = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        Ok(TargetBindingTranscriptInputsV3 {
            protected_rustc_invocation: TargetLineageIdentityV3::decode(
                "protected rustc invocation identity",
                self.record.field(2),
            )?,
            semantic_mir: TargetLineageIdentityV3::decode(
                "semantic MIR identity",
                self.record.field(3),
            )?,
            target_neutral_kir: TargetLineageIdentityV3::decode(
                "target-neutral Kernel IR identity",
                self.record.field(4),
            )?,
            target_bound_kir: TargetLineageIdentityV3::decode(
                "target-bound Kernel IR identity",
                self.record.field(5),
            )?,
            configured_target: validate_ascii_token("configured target", self.record.field(6))?,
            rustc_llvm_target: validate_ascii_token("rustc LLVM target", self.record.field(7))?,
            target_cpu: validate_ascii_token("target CPU", self.record.field(8))?,
            target_features: validate_ascii_token("target features", self.record.field(9))?,
            code_object_version: decode_u16("code object version", self.record.field(10))?,
            wave_width_bits: decode_u16("wave width", self.record.field(11))?,
            default_workgroup,
        })
    }
}

fn validate_target_binding_inputs_v3(
    inputs: &TargetBindingTranscriptInputsV3<'_>,
) -> Result<(), ProductionTargetLineageErrorV3> {
    require_exact_text(
        "configured target",
        validate_ascii_token("configured target", inputs.configured_target.as_bytes())?,
        EXACT_GFX942_TARGET_V3,
    )?;
    require_exact_text(
        "rustc LLVM target",
        validate_ascii_token("rustc LLVM target", inputs.rustc_llvm_target.as_bytes())?,
        EXACT_RUSTC_LLVM_TARGET_V3,
    )?;
    require_exact_text(
        "target CPU",
        validate_ascii_token("target CPU", inputs.target_cpu.as_bytes())?,
        EXACT_GFX942_CPU_V3,
    )?;
    require_exact_text(
        "target features",
        validate_ascii_token("target features", inputs.target_features.as_bytes())?,
        EXACT_GFX942_FEATURES_V3,
    )?;
    if inputs.code_object_version != EXACT_CODE_OBJECT_VERSION_V3 {
        return Err(ProductionTargetLineageErrorV3::InvalidInteger {
            field: "code object version",
            observed: u64::from(inputs.code_object_version),
        });
    }
    if inputs.wave_width_bits != EXACT_WAVE_WIDTH_BITS_V3 {
        return Err(ProductionTargetLineageErrorV3::InvalidInteger {
            field: "wave width",
            observed: u64::from(inputs.wave_width_bits),
        });
    }
    if inputs.default_workgroup.contains(&0) {
        return Err(ProductionTargetLineageErrorV3::InvalidInteger {
            field: "default workgroup",
            observed: 0,
        });
    }
    if inputs.target_neutral_kir == inputs.target_bound_kir {
        return Err(ProductionTargetLineageErrorV3::AssociationInvariant {
            detail: "target-neutral and target-bound Kernel IR identities must differ",
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DataLayoutTranscriptInputsV3<'a> {
    pub(crate) semantic_mir: TargetLineageIdentityV3,
    pub(crate) target_binding: TargetLineageIdentityV3,
    pub(crate) semantic_layout: TargetLineageIdentityV3,
    pub(crate) rustc_llvm_target: &'a str,
    pub(crate) live_rustc_data_layout: &'a str,
    pub(crate) final_llvm_target: &'a str,
    pub(crate) final_llvm_data_layout: &'a str,
    pub(crate) default_pointer_width_bits: u16,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct DataLayoutTranscriptV3 {
    record: CanonicalRecordV3,
}

impl DataLayoutTranscriptV3 {
    pub(crate) fn new(
        inputs: DataLayoutTranscriptInputsV3<'_>,
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        validate_data_layout_inputs_v3(&inputs)?;

        let semantic_mir = inputs.semantic_mir.encode();
        let target_binding = inputs.target_binding.encode();
        let semantic_layout = inputs.semantic_layout.encode();
        let pointer_width = inputs.default_pointer_width_bits.to_le_bytes();
        let fields: [&[u8]; 10] = [
            DATA_LAYOUT_DOMAIN_V3,
            ASSOCIATION_ONLY_CLAIM_V3,
            &semantic_mir,
            &target_binding,
            &semantic_layout,
            inputs.rustc_llvm_target.as_bytes(),
            inputs.live_rustc_data_layout.as_bytes(),
            inputs.final_llvm_target.as_bytes(),
            inputs.final_llvm_data_layout.as_bytes(),
            &pointer_width,
        ];
        Ok(Self {
            record: CanonicalRecordV3::build(&DATA_LAYOUT_SCHEMA_V3, &fields)?,
        })
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ProductionTargetLineageErrorV3> {
        let record = CanonicalRecordV3::decode(&DATA_LAYOUT_SCHEMA_V3, bytes)?;
        let value = Self { record };
        validate_data_layout_inputs_v3(&value.inputs()?)?;
        Ok(value)
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        self.record.canonical_bytes()
    }

    pub(crate) const fn claim(&self) -> TargetLineageClaimV3 {
        TargetLineageClaimV3::AssociationOnlyNoRefinementProof
    }

    pub(crate) const fn establishes_refinement_proof(&self) -> bool {
        false
    }

    pub(crate) fn inputs(
        &self,
    ) -> Result<DataLayoutTranscriptInputsV3<'_>, ProductionTargetLineageErrorV3> {
        Ok(DataLayoutTranscriptInputsV3 {
            semantic_mir: TargetLineageIdentityV3::decode(
                "semantic MIR identity",
                self.record.field(2),
            )?,
            target_binding: TargetLineageIdentityV3::decode(
                "target binding identity",
                self.record.field(3),
            )?,
            semantic_layout: TargetLineageIdentityV3::decode(
                "semantic layout identity",
                self.record.field(4),
            )?,
            rustc_llvm_target: validate_ascii_token("rustc LLVM target", self.record.field(5))?,
            live_rustc_data_layout: validate_ascii_token(
                "live rustc data layout",
                self.record.field(6),
            )?,
            final_llvm_target: validate_ascii_token("final LLVM target", self.record.field(7))?,
            final_llvm_data_layout: validate_ascii_token(
                "final LLVM data layout",
                self.record.field(8),
            )?,
            default_pointer_width_bits: decode_u16("default pointer width", self.record.field(9))?,
        })
    }
}

fn validate_data_layout_inputs_v3(
    inputs: &DataLayoutTranscriptInputsV3<'_>,
) -> Result<(), ProductionTargetLineageErrorV3> {
    let rustc_target =
        validate_ascii_token("rustc LLVM target", inputs.rustc_llvm_target.as_bytes())?;
    let final_target =
        validate_ascii_token("final LLVM target", inputs.final_llvm_target.as_bytes())?;
    require_exact_text(
        "rustc LLVM target",
        rustc_target,
        EXACT_RUSTC_LLVM_TARGET_V3,
    )?;
    if rustc_target != final_target {
        return Err(ProductionTargetLineageErrorV3::AssociationInvariant {
            detail: "live rustc and final LLVM target strings must be byte-identical",
        });
    }
    let live_layout = validate_ascii_token(
        "live rustc data layout",
        inputs.live_rustc_data_layout.as_bytes(),
    )?;
    let final_layout = validate_ascii_token(
        "final LLVM data layout",
        inputs.final_llvm_data_layout.as_bytes(),
    )?;
    if live_layout != final_layout {
        return Err(ProductionTargetLineageErrorV3::AssociationInvariant {
            detail: "live rustc and final LLVM data layouts must be byte-identical",
        });
    }
    if inputs.default_pointer_width_bits != 64 {
        return Err(ProductionTargetLineageErrorV3::InvalidInteger {
            field: "default pointer width",
            observed: u64::from(inputs.default_pointer_width_bits),
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AbiTranscriptInputsV3<'a> {
    pub(crate) semantic_mir: TargetLineageIdentityV3,
    pub(crate) target_bound_kir: TargetLineageIdentityV3,
    pub(crate) formal_memory: TargetLineageIdentityV3,
    /// Exact bytes from `CompilerFfiEnvelopeV1::canonical_bytes()`.
    pub(crate) compiler_ffi_envelope: &'a [u8],
    /// Exact bytes from `CompilerDescriptorSourceV1::canonical_bytes()`.
    pub(crate) compiler_descriptor_source: &'a [u8],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AbiTranscriptV3 {
    record: CanonicalRecordV3,
}

impl AbiTranscriptV3 {
    pub(crate) fn new(
        inputs: AbiTranscriptInputsV3<'_>,
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        let semantic_mir = inputs.semantic_mir.encode();
        let target_bound_kir = inputs.target_bound_kir.encode();
        let formal_memory = inputs.formal_memory.encode();
        let fields: [&[u8]; 7] = [
            ABI_DOMAIN_V3,
            ASSOCIATION_ONLY_CLAIM_V3,
            &semantic_mir,
            &target_bound_kir,
            &formal_memory,
            inputs.compiler_ffi_envelope,
            inputs.compiler_descriptor_source,
        ];
        Ok(Self {
            record: CanonicalRecordV3::build(&ABI_SCHEMA_V3, &fields)?,
        })
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ProductionTargetLineageErrorV3> {
        let record = CanonicalRecordV3::decode(&ABI_SCHEMA_V3, bytes)?;
        let value = Self { record };
        let _ = value.inputs()?;
        Ok(value)
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        self.record.canonical_bytes()
    }

    pub(crate) const fn claim(&self) -> TargetLineageClaimV3 {
        TargetLineageClaimV3::AssociationOnlyNoRefinementProof
    }

    pub(crate) const fn establishes_refinement_proof(&self) -> bool {
        false
    }

    pub(crate) fn inputs(
        &self,
    ) -> Result<AbiTranscriptInputsV3<'_>, ProductionTargetLineageErrorV3> {
        Ok(AbiTranscriptInputsV3 {
            semantic_mir: TargetLineageIdentityV3::decode(
                "semantic MIR identity",
                self.record.field(2),
            )?,
            target_bound_kir: TargetLineageIdentityV3::decode(
                "target-bound Kernel IR identity",
                self.record.field(3),
            )?,
            formal_memory: TargetLineageIdentityV3::decode(
                "formal memory identity",
                self.record.field(4),
            )?,
            compiler_ffi_envelope: self.record.field(5),
            compiler_descriptor_source: self.record.field(6),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AmdgpuLoweringTranscriptInputsV3<'a> {
    pub(crate) target_binding: TargetLineageIdentityV3,
    pub(crate) data_layout: TargetLineageIdentityV3,
    pub(crate) target_bound_kir: TargetLineageIdentityV3,
    pub(crate) configured_target: &'a str,
    /// Exact LLVM text before compiler descriptor publication or linking.
    pub(crate) pre_descriptor_llvm: &'a [u8],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AmdgpuLoweringTranscriptV3 {
    record: CanonicalRecordV3,
}

impl AmdgpuLoweringTranscriptV3 {
    pub(crate) fn new(
        inputs: AmdgpuLoweringTranscriptInputsV3<'_>,
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        validate_amdgpu_lowering_inputs_v3(&inputs)?;
        let target_binding = inputs.target_binding.encode();
        let data_layout = inputs.data_layout.encode();
        let target_bound_kir = inputs.target_bound_kir.encode();
        let fields: [&[u8]; 7] = [
            AMDGPU_LOWERING_DOMAIN_V3,
            ASSOCIATION_ONLY_CLAIM_V3,
            &target_binding,
            &data_layout,
            &target_bound_kir,
            inputs.configured_target.as_bytes(),
            inputs.pre_descriptor_llvm,
        ];
        Ok(Self {
            record: CanonicalRecordV3::build(&AMDGPU_LOWERING_SCHEMA_V3, &fields)?,
        })
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ProductionTargetLineageErrorV3> {
        let record = CanonicalRecordV3::decode(&AMDGPU_LOWERING_SCHEMA_V3, bytes)?;
        let value = Self { record };
        validate_amdgpu_lowering_inputs_v3(&value.inputs()?)?;
        Ok(value)
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        self.record.canonical_bytes()
    }

    pub(crate) const fn claim(&self) -> TargetLineageClaimV3 {
        TargetLineageClaimV3::AssociationOnlyNoRefinementProof
    }

    pub(crate) const fn establishes_refinement_proof(&self) -> bool {
        false
    }

    pub(crate) fn inputs(
        &self,
    ) -> Result<AmdgpuLoweringTranscriptInputsV3<'_>, ProductionTargetLineageErrorV3> {
        Ok(AmdgpuLoweringTranscriptInputsV3 {
            target_binding: TargetLineageIdentityV3::decode(
                "target binding identity",
                self.record.field(2),
            )?,
            data_layout: TargetLineageIdentityV3::decode(
                "data layout identity",
                self.record.field(3),
            )?,
            target_bound_kir: TargetLineageIdentityV3::decode(
                "target-bound Kernel IR identity",
                self.record.field(4),
            )?,
            configured_target: validate_ascii_token("configured target", self.record.field(5))?,
            pre_descriptor_llvm: validate_llvm_text(self.record.field(6))?.as_bytes(),
        })
    }
}

fn validate_amdgpu_lowering_inputs_v3(
    inputs: &AmdgpuLoweringTranscriptInputsV3<'_>,
) -> Result<(), ProductionTargetLineageErrorV3> {
    require_exact_text(
        "configured target",
        validate_ascii_token("configured target", inputs.configured_target.as_bytes())?,
        EXACT_GFX942_TARGET_V3,
    )?;
    validate_llvm_text(inputs.pre_descriptor_llvm)?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProofBindingTranscriptInputsV3 {
    pub(crate) semantic_mir: TargetLineageIdentityV3,
    pub(crate) middle_end: TargetLineageIdentityV3,
    pub(crate) kernel_ir: TargetLineageIdentityV3,
    pub(crate) mir_to_kir_correspondence: TargetLineageIdentityV3,
    pub(crate) formal_memory: TargetLineageIdentityV3,
}

/// Exact association of the five verified semantic-stage records consumed by
/// production lowering.
///
/// This transcript does not claim Verus verification or a refinement proof.
/// Its only claim is that the private production join observed these exact
/// content identities together before the live stage owners were consumed.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProofBindingTranscriptV3 {
    record: CanonicalRecordV3,
}

impl ProofBindingTranscriptV3 {
    pub(crate) fn new(
        inputs: ProofBindingTranscriptInputsV3,
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        let identities = [
            inputs.semantic_mir.encode(),
            inputs.middle_end.encode(),
            inputs.kernel_ir.encode(),
            inputs.mir_to_kir_correspondence.encode(),
            inputs.formal_memory.encode(),
        ];
        let fields: [&[u8]; 7] = [
            PROOF_BINDING_DOMAIN_V3,
            ASSOCIATION_ONLY_CLAIM_V3,
            &identities[0],
            &identities[1],
            &identities[2],
            &identities[3],
            &identities[4],
        ];
        Ok(Self {
            record: CanonicalRecordV3::build(&PROOF_BINDING_SCHEMA_V3, &fields)?,
        })
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ProductionTargetLineageErrorV3> {
        let record = CanonicalRecordV3::decode(&PROOF_BINDING_SCHEMA_V3, bytes)?;
        let value = Self { record };
        let _ = value.inputs()?;
        Ok(value)
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        self.record.canonical_bytes()
    }

    pub(crate) const fn claim(&self) -> TargetLineageClaimV3 {
        TargetLineageClaimV3::AssociationOnlyNoRefinementProof
    }

    pub(crate) const fn establishes_refinement_proof(&self) -> bool {
        false
    }

    pub(crate) const fn claims_verus_verification(&self) -> bool {
        false
    }

    pub(crate) fn inputs(
        &self,
    ) -> Result<ProofBindingTranscriptInputsV3, ProductionTargetLineageErrorV3> {
        Ok(ProofBindingTranscriptInputsV3 {
            semantic_mir: self.identity_field(2, "semantic MIR identity")?,
            middle_end: self.identity_field(3, "middle-end identity")?,
            kernel_ir: self.identity_field(4, "Kernel IR identity")?,
            mir_to_kir_correspondence: self
                .identity_field(5, "MIR-to-KIR correspondence identity")?,
            formal_memory: self.identity_field(6, "formal memory identity")?,
        })
    }

    fn identity_field(
        &self,
        index: usize,
        field: &'static str,
    ) -> Result<TargetLineageIdentityV3, ProductionTargetLineageErrorV3> {
        TargetLineageIdentityV3::decode(field, self.record.field(index))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ExportManifestTranscriptInputsV3<'a> {
    pub(crate) semantic_mir: TargetLineageIdentityV3,
    pub(crate) target_bound_kir: TargetLineageIdentityV3,
    pub(crate) abi: TargetLineageIdentityV3,
    /// Exact bytes from `CompilerDescriptorSourceV1::canonical_bytes()`.
    pub(crate) compiler_descriptor_source: &'a [u8],
    /// Exact bytes from `CompilerModuleSymbolManifestV1::canonical_bytes()`.
    pub(crate) final_symbol_manifest: &'a [u8],
}

/// Exact association of source semantics and target-bound ABI with the
/// descriptor and symbol-role manifest handed to the Worker.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ExportManifestTranscriptV3 {
    record: CanonicalRecordV3,
}

impl ExportManifestTranscriptV3 {
    pub(crate) fn new(
        inputs: ExportManifestTranscriptInputsV3<'_>,
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        validate_export_manifest_inputs_v3(&inputs)?;
        let semantic_mir = inputs.semantic_mir.encode();
        let target_bound_kir = inputs.target_bound_kir.encode();
        let abi = inputs.abi.encode();
        let fields: [&[u8]; 7] = [
            EXPORT_MANIFEST_DOMAIN_V3,
            ASSOCIATION_ONLY_CLAIM_V3,
            &semantic_mir,
            &target_bound_kir,
            &abi,
            inputs.compiler_descriptor_source,
            inputs.final_symbol_manifest,
        ];
        Ok(Self {
            record: CanonicalRecordV3::build(&EXPORT_MANIFEST_SCHEMA_V3, &fields)?,
        })
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ProductionTargetLineageErrorV3> {
        let record = CanonicalRecordV3::decode(&EXPORT_MANIFEST_SCHEMA_V3, bytes)?;
        let value = Self { record };
        validate_export_manifest_inputs_v3(&value.inputs()?)?;
        Ok(value)
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        self.record.canonical_bytes()
    }

    pub(crate) const fn claim(&self) -> TargetLineageClaimV3 {
        TargetLineageClaimV3::AssociationOnlyNoRefinementProof
    }

    pub(crate) const fn establishes_refinement_proof(&self) -> bool {
        false
    }

    pub(crate) fn inputs(
        &self,
    ) -> Result<ExportManifestTranscriptInputsV3<'_>, ProductionTargetLineageErrorV3> {
        Ok(ExportManifestTranscriptInputsV3 {
            semantic_mir: self.identity_field(2, "semantic MIR identity")?,
            target_bound_kir: self.identity_field(3, "target-bound Kernel IR identity")?,
            abi: self.identity_field(4, "ABI identity")?,
            compiler_descriptor_source: self.record.field(5),
            final_symbol_manifest: self.record.field(6),
        })
    }

    fn identity_field(
        &self,
        index: usize,
        field: &'static str,
    ) -> Result<TargetLineageIdentityV3, ProductionTargetLineageErrorV3> {
        TargetLineageIdentityV3::decode(field, self.record.field(index))
    }
}

fn validate_export_manifest_inputs_v3(
    inputs: &ExportManifestTranscriptInputsV3<'_>,
) -> Result<(), ProductionTargetLineageErrorV3> {
    CompilerDescriptorSourceV1::decode(inputs.compiler_descriptor_source).map_err(|_| {
        ProductionTargetLineageErrorV3::InvalidNestedEncoding {
            field: "compiler descriptor source",
        }
    })?;
    CompilerModuleSymbolManifestV1::decode(inputs.final_symbol_manifest).map_err(|_| {
        ProductionTargetLineageErrorV3::InvalidNestedEncoding {
            field: "final compiler symbol manifest",
        }
    })?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SemanticToLlvmAssociationInputsV3 {
    pub(crate) semantic_mir: TargetLineageIdentityV3,
    pub(crate) middle_end: TargetLineageIdentityV3,
    pub(crate) kernel_ir: TargetLineageIdentityV3,
    pub(crate) mir_to_kir_correspondence: TargetLineageIdentityV3,
    pub(crate) formal_memory: TargetLineageIdentityV3,
    pub(crate) proof_binding: TargetLineageIdentityV3,
    pub(crate) target_binding: TargetLineageIdentityV3,
    pub(crate) data_layout: TargetLineageIdentityV3,
    pub(crate) abi: TargetLineageIdentityV3,
    pub(crate) export_manifest: TargetLineageIdentityV3,
    pub(crate) amdgpu_lowering: TargetLineageIdentityV3,
    pub(crate) final_llvm: TargetLineageIdentityV3,
    pub(crate) final_compiler_module_commitment: TargetLineageIdentityV3,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SemanticToLlvmAssociationTranscriptV3 {
    record: CanonicalRecordV3,
}

impl SemanticToLlvmAssociationTranscriptV3 {
    pub(crate) fn new(
        inputs: SemanticToLlvmAssociationInputsV3,
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        let identities = [
            inputs.semantic_mir.encode(),
            inputs.middle_end.encode(),
            inputs.kernel_ir.encode(),
            inputs.mir_to_kir_correspondence.encode(),
            inputs.formal_memory.encode(),
            inputs.proof_binding.encode(),
            inputs.target_binding.encode(),
            inputs.data_layout.encode(),
            inputs.abi.encode(),
            inputs.export_manifest.encode(),
            inputs.amdgpu_lowering.encode(),
            inputs.final_llvm.encode(),
            inputs.final_compiler_module_commitment.encode(),
        ];
        let fields: [&[u8]; 15] = [
            SEMANTIC_TO_LLVM_DOMAIN_V3,
            ASSOCIATION_ONLY_CLAIM_V3,
            &identities[0],
            &identities[1],
            &identities[2],
            &identities[3],
            &identities[4],
            &identities[5],
            &identities[6],
            &identities[7],
            &identities[8],
            &identities[9],
            &identities[10],
            &identities[11],
            &identities[12],
        ];
        Ok(Self {
            record: CanonicalRecordV3::build(&SEMANTIC_TO_LLVM_SCHEMA_V3, &fields)?,
        })
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ProductionTargetLineageErrorV3> {
        let record = CanonicalRecordV3::decode(&SEMANTIC_TO_LLVM_SCHEMA_V3, bytes)?;
        let value = Self { record };
        let _ = value.inputs()?;
        Ok(value)
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        self.record.canonical_bytes()
    }

    pub(crate) const fn claim(&self) -> TargetLineageClaimV3 {
        TargetLineageClaimV3::AssociationOnlyNoRefinementProof
    }

    pub(crate) const fn establishes_refinement_proof(&self) -> bool {
        false
    }

    pub(crate) const fn authenticates_producer(&self) -> bool {
        false
    }

    pub(crate) const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub(crate) fn inputs(
        &self,
    ) -> Result<SemanticToLlvmAssociationInputsV3, ProductionTargetLineageErrorV3> {
        Ok(SemanticToLlvmAssociationInputsV3 {
            semantic_mir: self.identity_field(2, "semantic MIR identity")?,
            middle_end: self.identity_field(3, "middle-end identity")?,
            kernel_ir: self.identity_field(4, "Kernel IR identity")?,
            mir_to_kir_correspondence: self
                .identity_field(5, "MIR-to-KIR correspondence identity")?,
            formal_memory: self.identity_field(6, "formal memory identity")?,
            proof_binding: self.identity_field(7, "proof binding identity")?,
            target_binding: self.identity_field(8, "target binding identity")?,
            data_layout: self.identity_field(9, "data layout identity")?,
            abi: self.identity_field(10, "ABI identity")?,
            export_manifest: self.identity_field(11, "export manifest identity")?,
            amdgpu_lowering: self.identity_field(12, "AMDGPU lowering identity")?,
            final_llvm: self.identity_field(13, "final LLVM identity")?,
            final_compiler_module_commitment: self
                .identity_field(14, "final compiler module commitment identity")?,
        })
    }

    fn identity_field(
        &self,
        index: usize,
        field: &'static str,
    ) -> Result<TargetLineageIdentityV3, ProductionTargetLineageErrorV3> {
        TargetLineageIdentityV3::decode(field, self.record.field(index))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub(crate) enum ProductionTargetLineageErrorV3 {
    AllocationFailed,
    AssociationInvariant {
        detail: &'static str,
    },
    DeclaredLengthMismatch {
        declared: usize,
        actual: usize,
    },
    EmptyField {
        field: &'static str,
    },
    ExactValueMismatch {
        field: &'static str,
    },
    FieldTooLarge {
        field: &'static str,
        actual: usize,
        max: usize,
    },
    InvalidFieldLength {
        field: &'static str,
        actual: usize,
        expected: usize,
    },
    InvalidInteger {
        field: &'static str,
        observed: u64,
    },
    InvalidNestedEncoding {
        field: &'static str,
    },
    InvalidMagic,
    InvalidText {
        field: &'static str,
    },
    LengthOverflow,
    NonZeroFieldFlags {
        field: &'static str,
    },
    NonZeroReserved,
    TrailingBytes {
        trailing: usize,
    },
    TranscriptTooLarge {
        actual: usize,
        max: usize,
    },
    Truncated,
    UnsupportedVersion {
        observed: u16,
    },
    WrongClaim {
        record: &'static str,
    },
    WrongDomain {
        record: &'static str,
    },
    WrongFieldCount {
        record: &'static str,
        expected: usize,
        observed: usize,
    },
    WrongFieldTag {
        field: &'static str,
        expected: u16,
        observed: u16,
    },
    WrongPolicy {
        expected: u16,
        observed: u16,
    },
    WrongRecordKind {
        expected: u16,
        observed: u16,
    },
    ZeroIdentity {
        field: &'static str,
    },
    ZeroIdentityLength {
        field: &'static str,
    },
}

impl fmt::Display for ProductionTargetLineageErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed => formatter.write_str("target lineage allocation failed"),
            Self::AssociationInvariant { detail } => {
                write!(formatter, "target lineage association failed: {detail}")
            }
            Self::DeclaredLengthMismatch { declared, actual } => write!(
                formatter,
                "target lineage declared {declared} bytes but received {actual}"
            ),
            Self::EmptyField { field } => write!(formatter, "target lineage {field} is empty"),
            Self::ExactValueMismatch { field } => {
                write!(
                    formatter,
                    "target lineage {field} is not the production V3 value"
                )
            }
            Self::FieldTooLarge { field, actual, max } => write!(
                formatter,
                "target lineage {field} has {actual} bytes; maximum is {max}"
            ),
            Self::InvalidFieldLength {
                field,
                actual,
                expected,
            } => write!(
                formatter,
                "target lineage {field} has {actual} bytes; expected exactly {expected}"
            ),
            Self::InvalidInteger { field, observed } => {
                write!(
                    formatter,
                    "target lineage {field} has invalid value {observed}"
                )
            }
            Self::InvalidNestedEncoding { field } => {
                write!(formatter, "target lineage {field} is not canonical")
            }
            Self::InvalidMagic => formatter.write_str("invalid target lineage magic"),
            Self::InvalidText { field } => {
                write!(formatter, "target lineage {field} is not canonical text")
            }
            Self::LengthOverflow => formatter.write_str("target lineage length overflow"),
            Self::NonZeroFieldFlags { field } => {
                write!(formatter, "target lineage {field} has nonzero field flags")
            }
            Self::NonZeroReserved => {
                formatter.write_str("target lineage header has nonzero reserved bytes")
            }
            Self::TrailingBytes { trailing } => {
                write!(formatter, "target lineage has {trailing} trailing bytes")
            }
            Self::TranscriptTooLarge { actual, max } => write!(
                formatter,
                "target lineage transcript has {actual} bytes; maximum is {max}"
            ),
            Self::Truncated => formatter.write_str("truncated target lineage transcript"),
            Self::UnsupportedVersion { observed } => {
                write!(formatter, "unsupported target lineage version {observed}")
            }
            Self::WrongClaim { record } => {
                write!(
                    formatter,
                    "{record} does not carry the association-only claim"
                )
            }
            Self::WrongDomain { record } => write!(formatter, "wrong domain for {record}"),
            Self::WrongFieldCount {
                record,
                expected,
                observed,
            } => write!(
                formatter,
                "{record} has {observed} fields; expected {expected}"
            ),
            Self::WrongFieldTag {
                field,
                expected,
                observed,
            } => write!(
                formatter,
                "target lineage {field} has tag {observed}; expected {expected}"
            ),
            Self::WrongPolicy { expected, observed } => write!(
                formatter,
                "target lineage policy {observed} is unsupported; expected {expected}"
            ),
            Self::WrongRecordKind { expected, observed } => write!(
                formatter,
                "target lineage kind {observed} does not match expected kind {expected}"
            ),
            Self::ZeroIdentity { field } => {
                write!(formatter, "target lineage {field} has a zero digest")
            }
            Self::ZeroIdentityLength { field } => {
                write!(formatter, "target lineage {field} has a zero byte length")
            }
        }
    }
}

impl Error for ProductionTargetLineageErrorV3 {}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_compiler_ffi::CompilerModuleSymbolRoleV1;
    use fe2o3_kernel_descriptor::{
        BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CodeObjectVersion,
        CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
        DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
        KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1,
        ProducerIdentityV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, Text,
        ValidName,
    };

    type DecoderV3 = fn(&[u8]) -> bool;

    const DATA_LAYOUT: &str = "e-m:e-p:64:64-p1:64:64-n32:64-S32-A5-G1";
    const LLVM: &[u8] = b"target triple = \"amdgcn-amd-amdhsa\"\n\
target datalayout = \"e-m:e-p:64:64-p1:64:64-n32:64-S32-A5-G1\"\n\n\
define amdgpu_kernel void @kernel() {\nentry:\n  ret void\n}\n";

    fn identity(seed: u8) -> TargetLineageIdentityV3 {
        TargetLineageIdentityV3::new([seed; 32], u64::from(seed) + 1).unwrap()
    }

    fn descriptor_source() -> CompilerDescriptorSourceV1 {
        let source_type =
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
        let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
        let argument = LogicalArgumentV1::scalar(
            0,
            ValidName::new("value").unwrap(),
            &source_type,
            &layout,
            0,
        )
        .unwrap();
        let evidence = |byte| {
            BuildEvidenceV1::new(
                EvidenceIdentity::from_opaque_bytes([byte; 32]),
                EvidenceDigest::from_sha256_bytes([byte.wrapping_add(1); 32]),
            )
        };
        let kernel = KernelDescriptorV1::new(
            KernelId::from_bytes([0x11; 32]),
            ValidName::new("scale").unwrap(),
            ValidName::new("scale").unwrap(),
            ValidName::new("scale.kd").unwrap(),
            evidence(0x21),
            evidence(0x31),
            vec![],
            KernelAbiLayoutV1::new(4, 4, 4).unwrap(),
            LaunchConstraintsV1::new(
                1,
                BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
                DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
                256,
                0,
                0,
            )
            .unwrap(),
            vec![argument],
        )
        .unwrap();
        CompilerDescriptorSourceV1::new(
            DeviceDescriptorTableV1::new(
                CanonicalCodeObjectDigest::from_bytes([0; 32]),
                CodeObjectVersion::V6,
                CompilerIdentityV1::new(
                    Text::new("rustc-codegen-fe2o3").unwrap(),
                    Text::new("test").unwrap(),
                    [0x41; 20],
                ),
                ProducerIdentityV1::new(
                    Text::new("rustc-codegen-fe2o3").unwrap(),
                    Text::new("test").unwrap(),
                ),
                DeviceTargetV1::parse(EXACT_GFX942_TARGET_V3).unwrap(),
                vec![source_type],
                vec![layout],
                vec![kernel],
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn symbol_manifest() -> CompilerModuleSymbolManifestV1 {
        CompilerModuleSymbolManifestV1::new([
            (CompilerModuleSymbolRoleV1::KernelEntry, "scale"),
            (CompilerModuleSymbolRoleV1::KernelDescriptor, "scale.kd"),
        ])
        .unwrap()
    }

    fn target_binding() -> TargetBindingTranscriptV3 {
        TargetBindingTranscriptV3::new(TargetBindingTranscriptInputsV3 {
            protected_rustc_invocation: identity(1),
            semantic_mir: identity(2),
            target_neutral_kir: identity(3),
            target_bound_kir: identity(4),
            configured_target: EXACT_GFX942_TARGET_V3,
            rustc_llvm_target: EXACT_RUSTC_LLVM_TARGET_V3,
            target_cpu: EXACT_GFX942_CPU_V3,
            target_features: EXACT_GFX942_FEATURES_V3,
            code_object_version: EXACT_CODE_OBJECT_VERSION_V3,
            wave_width_bits: EXACT_WAVE_WIDTH_BITS_V3,
            default_workgroup: [256, 1, 1],
        })
        .unwrap()
    }

    fn data_layout() -> DataLayoutTranscriptV3 {
        DataLayoutTranscriptV3::new(DataLayoutTranscriptInputsV3 {
            semantic_mir: identity(2),
            target_binding: identity(5),
            semantic_layout: identity(6),
            rustc_llvm_target: EXACT_RUSTC_LLVM_TARGET_V3,
            live_rustc_data_layout: DATA_LAYOUT,
            final_llvm_target: EXACT_RUSTC_LLVM_TARGET_V3,
            final_llvm_data_layout: DATA_LAYOUT,
            default_pointer_width_bits: 64,
        })
        .unwrap()
    }

    fn abi() -> AbiTranscriptV3 {
        AbiTranscriptV3::new(AbiTranscriptInputsV3 {
            semantic_mir: identity(2),
            target_bound_kir: identity(4),
            formal_memory: identity(7),
            compiler_ffi_envelope: b"\0ffi\xffcanonical",
            compiler_descriptor_source: b"\0descriptor\xffcanonical",
        })
        .unwrap()
    }

    fn lowering() -> AmdgpuLoweringTranscriptV3 {
        AmdgpuLoweringTranscriptV3::new(AmdgpuLoweringTranscriptInputsV3 {
            target_binding: identity(5),
            data_layout: identity(8),
            target_bound_kir: identity(4),
            configured_target: EXACT_GFX942_TARGET_V3,
            pre_descriptor_llvm: LLVM,
        })
        .unwrap()
    }

    fn proof_binding() -> ProofBindingTranscriptV3 {
        ProofBindingTranscriptV3::new(ProofBindingTranscriptInputsV3 {
            semantic_mir: identity(2),
            middle_end: identity(3),
            kernel_ir: identity(4),
            mir_to_kir_correspondence: identity(5),
            formal_memory: identity(6),
        })
        .unwrap()
    }

    fn export_manifest() -> ExportManifestTranscriptV3 {
        let descriptor = descriptor_source();
        let manifest = symbol_manifest();
        ExportManifestTranscriptV3::new(ExportManifestTranscriptInputsV3 {
            semantic_mir: identity(2),
            target_bound_kir: identity(4),
            abi: identity(10),
            compiler_descriptor_source: descriptor.canonical_bytes(),
            final_symbol_manifest: manifest.canonical_bytes(),
        })
        .unwrap()
    }

    fn association() -> SemanticToLlvmAssociationTranscriptV3 {
        SemanticToLlvmAssociationTranscriptV3::new(SemanticToLlvmAssociationInputsV3 {
            semantic_mir: identity(2),
            middle_end: identity(3),
            kernel_ir: identity(4),
            mir_to_kir_correspondence: identity(5),
            formal_memory: identity(6),
            proof_binding: identity(7),
            target_binding: identity(8),
            data_layout: identity(9),
            abi: identity(10),
            export_manifest: identity(11),
            amdgpu_lowering: identity(12),
            final_llvm: identity(13),
            final_compiler_module_commitment: identity(14),
        })
        .unwrap()
    }

    #[test]
    fn every_transcript_strictly_round_trips_and_reencodes() {
        let target = target_binding();
        let decoded = TargetBindingTranscriptV3::decode(target.canonical_bytes()).unwrap();
        assert_eq!(decoded.canonical_bytes(), target.canonical_bytes());
        assert_eq!(decoded.inputs().unwrap().default_workgroup, [256, 1, 1]);
        assert_eq!(
            TargetBindingTranscriptV3::new(decoded.inputs().unwrap())
                .unwrap()
                .canonical_bytes(),
            target.canonical_bytes()
        );

        let layout = data_layout();
        let decoded = DataLayoutTranscriptV3::decode(layout.canonical_bytes()).unwrap();
        assert_eq!(decoded.canonical_bytes(), layout.canonical_bytes());
        assert_eq!(
            decoded.inputs().unwrap().final_llvm_data_layout,
            DATA_LAYOUT
        );
        assert_eq!(
            DataLayoutTranscriptV3::new(decoded.inputs().unwrap())
                .unwrap()
                .canonical_bytes(),
            layout.canonical_bytes()
        );

        let abi = abi();
        let decoded = AbiTranscriptV3::decode(abi.canonical_bytes()).unwrap();
        assert_eq!(decoded.canonical_bytes(), abi.canonical_bytes());
        assert_eq!(
            decoded.inputs().unwrap().compiler_ffi_envelope,
            b"\0ffi\xffcanonical"
        );
        assert_eq!(
            AbiTranscriptV3::new(decoded.inputs().unwrap())
                .unwrap()
                .canonical_bytes(),
            abi.canonical_bytes()
        );

        let lowering = lowering();
        let decoded = AmdgpuLoweringTranscriptV3::decode(lowering.canonical_bytes()).unwrap();
        assert_eq!(decoded.canonical_bytes(), lowering.canonical_bytes());
        assert_eq!(decoded.inputs().unwrap().pre_descriptor_llvm, LLVM);
        assert_eq!(
            AmdgpuLoweringTranscriptV3::new(decoded.inputs().unwrap())
                .unwrap()
                .canonical_bytes(),
            lowering.canonical_bytes()
        );

        let proof_binding = proof_binding();
        let decoded = ProofBindingTranscriptV3::decode(proof_binding.canonical_bytes()).unwrap();
        assert_eq!(decoded.canonical_bytes(), proof_binding.canonical_bytes());
        assert_eq!(decoded.inputs().unwrap().formal_memory, identity(6));
        assert_eq!(
            ProofBindingTranscriptV3::new(decoded.inputs().unwrap())
                .unwrap()
                .canonical_bytes(),
            proof_binding.canonical_bytes()
        );

        let export_manifest = export_manifest();
        let decoded =
            ExportManifestTranscriptV3::decode(export_manifest.canonical_bytes()).unwrap();
        assert_eq!(decoded.canonical_bytes(), export_manifest.canonical_bytes());
        assert_eq!(decoded.inputs().unwrap().abi, identity(10));
        assert_eq!(
            ExportManifestTranscriptV3::new(decoded.inputs().unwrap())
                .unwrap()
                .canonical_bytes(),
            export_manifest.canonical_bytes()
        );

        let association = association();
        let decoded =
            SemanticToLlvmAssociationTranscriptV3::decode(association.canonical_bytes()).unwrap();
        assert_eq!(decoded.canonical_bytes(), association.canonical_bytes());
        assert_eq!(decoded.inputs().unwrap().final_llvm, identity(13));
        assert_eq!(
            decoded.inputs().unwrap().final_compiler_module_commitment,
            identity(14)
        );
        assert_eq!(
            SemanticToLlvmAssociationTranscriptV3::new(decoded.inputs().unwrap())
                .unwrap()
                .canonical_bytes(),
            association.canonical_bytes()
        );
    }

    #[test]
    fn all_records_are_explicitly_association_only() {
        let identity = identity(42);
        assert_eq!(identity.sha256(), [42; 32]);
        assert_eq!(identity.byte_len(), 43);
        assert_eq!(
            target_binding().claim(),
            TargetLineageClaimV3::AssociationOnlyNoRefinementProof
        );
        assert!(!target_binding().establishes_refinement_proof());
        let layout = data_layout();
        assert_eq!(
            layout.claim(),
            TargetLineageClaimV3::AssociationOnlyNoRefinementProof
        );
        assert!(!layout.establishes_refinement_proof());
        let abi = abi();
        assert_eq!(
            abi.claim(),
            TargetLineageClaimV3::AssociationOnlyNoRefinementProof
        );
        assert!(!abi.establishes_refinement_proof());
        let lowering = lowering();
        assert_eq!(
            lowering.claim(),
            TargetLineageClaimV3::AssociationOnlyNoRefinementProof
        );
        assert!(!lowering.establishes_refinement_proof());
        let proof_binding = proof_binding();
        assert_eq!(
            proof_binding.claim(),
            TargetLineageClaimV3::AssociationOnlyNoRefinementProof
        );
        assert!(!proof_binding.establishes_refinement_proof());
        assert!(!proof_binding.claims_verus_verification());
        let export_manifest = export_manifest();
        assert_eq!(
            export_manifest.claim(),
            TargetLineageClaimV3::AssociationOnlyNoRefinementProof
        );
        assert!(!export_manifest.establishes_refinement_proof());
        let association = association();
        assert_eq!(
            association.claim(),
            TargetLineageClaimV3::AssociationOnlyNoRefinementProof
        );
        assert!(!association.establishes_refinement_proof());
        assert!(!association.authenticates_producer());
        assert!(!association.grants_publication_authority());
    }

    #[test]
    fn every_prefix_and_trailing_byte_is_rejected() {
        let target = target_binding();
        let layout = data_layout();
        let abi = abi();
        let lowering = lowering();
        let proof_binding = proof_binding();
        let export_manifest = export_manifest();
        let association = association();
        let records: Vec<(&[u8], DecoderV3)> = vec![
            (target.canonical_bytes(), |bytes| {
                TargetBindingTranscriptV3::decode(bytes).is_ok()
            }),
            (layout.canonical_bytes(), |bytes| {
                DataLayoutTranscriptV3::decode(bytes).is_ok()
            }),
            (abi.canonical_bytes(), |bytes| {
                AbiTranscriptV3::decode(bytes).is_ok()
            }),
            (lowering.canonical_bytes(), |bytes| {
                AmdgpuLoweringTranscriptV3::decode(bytes).is_ok()
            }),
            (proof_binding.canonical_bytes(), |bytes| {
                ProofBindingTranscriptV3::decode(bytes).is_ok()
            }),
            (export_manifest.canonical_bytes(), |bytes| {
                ExportManifestTranscriptV3::decode(bytes).is_ok()
            }),
            (association.canonical_bytes(), |bytes| {
                SemanticToLlvmAssociationTranscriptV3::decode(bytes).is_ok()
            }),
        ];
        for (bytes, decodes) in records {
            for prefix_len in 0..bytes.len() {
                assert!(
                    !decodes(&bytes[..prefix_len]),
                    "accepted prefix {prefix_len}"
                );
            }
            let mut trailing = bytes.to_vec();
            trailing.push(0);
            assert!(!decodes(&trailing));
        }
    }

    #[test]
    fn header_and_field_noncanonical_axes_are_rejected() {
        let canonical = target_binding().canonical_bytes().to_vec();
        for (offset, replacement) in [
            (0, 0_u8),
            (8, 2_u8),
            (10, DATA_LAYOUT_KIND_V3 as u8),
            (12, 2_u8),
            (14, 12_u8),
            (20, 1_u8),
            (TRANSCRIPT_HEADER_BYTES_V3, 2_u8),
            (TRANSCRIPT_HEADER_BYTES_V3 + 2, 1_u8),
        ] {
            let mut hostile = canonical.clone();
            hostile[offset] = replacement;
            assert!(TargetBindingTranscriptV3::decode(&hostile).is_err());
        }
        let mut wrong_domain = canonical.clone();
        wrong_domain[TRANSCRIPT_HEADER_BYTES_V3 + FIELD_HEADER_BYTES_V3] ^= 1;
        assert!(TargetBindingTranscriptV3::decode(&wrong_domain).is_err());
    }

    #[test]
    fn exact_production_target_policy_rejects_substitutions() {
        let base = TargetBindingTranscriptInputsV3 {
            protected_rustc_invocation: identity(1),
            semantic_mir: identity(2),
            target_neutral_kir: identity(3),
            target_bound_kir: identity(4),
            configured_target: EXACT_GFX942_TARGET_V3,
            rustc_llvm_target: EXACT_RUSTC_LLVM_TARGET_V3,
            target_cpu: EXACT_GFX942_CPU_V3,
            target_features: EXACT_GFX942_FEATURES_V3,
            code_object_version: 6,
            wave_width_bits: 64,
            default_workgroup: [256, 1, 1],
        };
        assert!(
            TargetBindingTranscriptV3::new(TargetBindingTranscriptInputsV3 {
                configured_target: "gfx942:xnack+",
                ..base
            })
            .is_err()
        );
        assert!(
            TargetBindingTranscriptV3::new(TargetBindingTranscriptInputsV3 {
                code_object_version: 5,
                ..base
            })
            .is_err()
        );
        assert!(
            TargetBindingTranscriptV3::new(TargetBindingTranscriptInputsV3 {
                target_bound_kir: base.target_neutral_kir,
                ..base
            })
            .is_err()
        );
    }

    #[test]
    fn layout_policy_requires_exact_live_to_final_agreement() {
        let base = DataLayoutTranscriptInputsV3 {
            semantic_mir: identity(2),
            target_binding: identity(5),
            semantic_layout: identity(6),
            rustc_llvm_target: EXACT_RUSTC_LLVM_TARGET_V3,
            live_rustc_data_layout: DATA_LAYOUT,
            final_llvm_target: EXACT_RUSTC_LLVM_TARGET_V3,
            final_llvm_data_layout: DATA_LAYOUT,
            default_pointer_width_bits: 64,
        };
        assert!(
            DataLayoutTranscriptV3::new(DataLayoutTranscriptInputsV3 {
                final_llvm_target: "spirv64-amd-amdhsa",
                ..base
            })
            .is_err()
        );
        assert!(
            DataLayoutTranscriptV3::new(DataLayoutTranscriptInputsV3 {
                final_llvm_data_layout: "e-p:32:32",
                ..base
            })
            .is_err()
        );
    }

    #[test]
    fn bounds_are_checked_before_record_allocation() {
        let oversized_envelope = vec![0_u8; MAX_COMPILER_FFI_ENVELOPE_BYTES_V3 + 1];
        assert!(matches!(
            AbiTranscriptV3::new(AbiTranscriptInputsV3 {
                semantic_mir: identity(2),
                target_bound_kir: identity(4),
                formal_memory: identity(7),
                compiler_ffi_envelope: &oversized_envelope,
                compiler_descriptor_source: b"descriptor",
            }),
            Err(ProductionTargetLineageErrorV3::FieldTooLarge {
                field: "compiler FFI envelope",
                ..
            })
        ));

        let oversized_llvm = vec![b'x'; MAX_PRE_DESCRIPTOR_LLVM_BYTES_V3 + 1];
        assert!(matches!(
            AmdgpuLoweringTranscriptV3::new(AmdgpuLoweringTranscriptInputsV3 {
                target_binding: identity(5),
                data_layout: identity(8),
                target_bound_kir: identity(4),
                configured_target: EXACT_GFX942_TARGET_V3,
                pre_descriptor_llvm: &oversized_llvm,
            }),
            Err(ProductionTargetLineageErrorV3::FieldTooLarge {
                field: "exact pre-descriptor LLVM",
                ..
            })
        ));

        let descriptor = descriptor_source();
        let manifest = symbol_manifest();
        assert!(matches!(
            ExportManifestTranscriptV3::new(ExportManifestTranscriptInputsV3 {
                semantic_mir: identity(2),
                target_bound_kir: identity(4),
                abi: identity(10),
                compiler_descriptor_source: &descriptor.canonical_bytes()[..1],
                final_symbol_manifest: manifest.canonical_bytes(),
            }),
            Err(ProductionTargetLineageErrorV3::InvalidNestedEncoding {
                field: "compiler descriptor source"
            })
        ));
        assert!(matches!(
            ExportManifestTranscriptV3::new(ExportManifestTranscriptInputsV3 {
                semantic_mir: identity(2),
                target_bound_kir: identity(4),
                abi: identity(10),
                compiler_descriptor_source: descriptor.canonical_bytes(),
                final_symbol_manifest: &manifest.canonical_bytes()[..1],
            }),
            Err(ProductionTargetLineageErrorV3::InvalidNestedEncoding {
                field: "final compiler symbol manifest"
            })
        ));
    }

    #[test]
    fn identities_reject_zero_digest_and_zero_length() {
        assert!(matches!(
            TargetLineageIdentityV3::new([0; 32], 1),
            Err(ProductionTargetLineageErrorV3::ZeroIdentity { .. })
        ));
        assert!(matches!(
            TargetLineageIdentityV3::new([1; 32], 0),
            Err(ProductionTargetLineageErrorV3::ZeroIdentityLength { .. })
        ));
    }
}
