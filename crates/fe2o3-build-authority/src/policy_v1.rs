use std::fmt;

use sha2::{Digest, Sha256};

use crate::{CompilerClosureErrorV1, CompilerClosureV1};

/// Exact encoded byte length of every Policy V1 document.
pub const POLICY_V1_ENCODED_LEN: usize = 432;
/// Exact Policy V1 header byte length.
pub const POLICY_V1_HEADER_LEN: u16 = 32;
/// Exact number of Policy V1 TLV fields.
pub const POLICY_V1_FIELD_COUNT: u16 = 14;
/// Policy V1 wire-format version.
pub const POLICY_V1_VERSION: u16 = 1;
/// Policy V1 header magic.
pub const POLICY_V1_MAGIC: [u8; 8] = *b"F2AUPOL1";
/// The only target accepted by Policy V1.
pub const POLICY_V1_TARGET: &str = "gfx942:xnack-";
/// Domain for the canonical Policy V1 content identity.
pub const POLICY_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-AUTHORITY-POLICY/V1\0";

const PROFILE_RESERVED_TRUSTED_SERVICE: u8 = 1;
const KNOWN_PIPELINE_BITS: u32 = 0b11;
const FINAL_GENERATION_COMMIT_RIGHT: u32 = 1;
const KNOWN_PUBLICATION_RIGHTS: u32 = FINAL_GENERATION_COMMIT_RIGHT;

const TAG_SERIAL: u16 = 0x0001;
const TAG_LAUNCHER_SHA256: u16 = 0x0002;
const TAG_CARGO_FE2O3_SHA256: u16 = 0x0003;
const TAG_PROFILE: u16 = 0x0004;
const TAG_CARGO_SHA256: u16 = 0x0010;
const TAG_RUSTC_SHA256: u16 = 0x0011;
const TAG_RUNTIME_TREE_SHA256: u16 = 0x0012;
const TAG_BACKEND_SHA256: u16 = 0x0013;
const TAG_COMPILER_CLOSURE_SHA256: u16 = 0x0014;
const TAG_TARGET: u16 = 0x0020;
const TAG_PIPELINE_ALLOWLIST: u16 = 0x0021;
const TAG_SELECTED_PIPELINE: u16 = 0x0022;
const TAG_ARGV_SHA256: u16 = 0x0023;
const TAG_PUBLICATION_RIGHTS: u16 = 0x0024;

const FIELD_SPECS: [(u16, u32); POLICY_V1_FIELD_COUNT as usize] = [
    (TAG_SERIAL, 8),
    (TAG_LAUNCHER_SHA256, 32),
    (TAG_CARGO_FE2O3_SHA256, 32),
    (TAG_PROFILE, 1),
    (TAG_CARGO_SHA256, 32),
    (TAG_RUSTC_SHA256, 32),
    (TAG_RUNTIME_TREE_SHA256, 32),
    (TAG_BACKEND_SHA256, 32),
    (TAG_COMPILER_CLOSURE_SHA256, 32),
    (TAG_TARGET, 13),
    (TAG_PIPELINE_ALLOWLIST, 4),
    (TAG_SELECTED_PIPELINE, 2),
    (TAG_ARGV_SHA256, 32),
    (TAG_PUBLICATION_RIGHTS, 4),
];

/// The execution profile accepted by the standalone Policy V1 codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AuthorityProfileV1 {
    /// A staging-only profile with no publication authority.
    StandaloneFoundation = 0,
}

/// A bounded pipeline selectable by Policy V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum PipelineV1 {
    /// The collected row-softmax V1 pipeline.
    CollectedRowSoftmax = 1,
    /// The collected tiled-GEMM V1 pipeline.
    CollectedTiledGemm = 2,
}

impl PipelineV1 {
    const fn allowlist_bit(self) -> u32 {
        match self {
            Self::CollectedRowSoftmax => 1 << 0,
            Self::CollectedTiledGemm => 1 << 1,
        }
    }

    pub(crate) fn from_wire(value: u16) -> Result<Self, PolicyErrorV1> {
        match value {
            1 => Ok(Self::CollectedRowSoftmax),
            2 => Ok(Self::CollectedTiledGemm),
            _ => Err(PolicyErrorV1::UnknownPipeline { value }),
        }
    }
}

/// A validated allowlist of Policy V1 pipelines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PipelineAllowlistV1(u32);

impl PipelineAllowlistV1 {
    /// An allowlist containing only collected row softmax.
    pub const ROW_SOFTMAX: Self = Self(1 << 0);
    /// An allowlist containing only collected tiled GEMM.
    pub const TILED_GEMM: Self = Self(1 << 1);
    /// An allowlist containing both current Policy V1 pipelines.
    pub const ALL: Self = Self(KNOWN_PIPELINE_BITS);

    /// Validates raw Policy V1 allowlist bits.
    pub fn from_bits(bits: u32) -> Result<Self, PolicyErrorV1> {
        if bits & !KNOWN_PIPELINE_BITS != 0 {
            return Err(PolicyErrorV1::UnknownPipelineAllowlistBits { bits });
        }
        Ok(Self(bits))
    }

    /// Returns the canonical wire bits.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Reports whether the pipeline is present in this allowlist.
    pub const fn allows(self, pipeline: PipelineV1) -> bool {
        self.0 & pipeline.allowlist_bit() != 0
    }
}

/// Publication rights carried by an accepted standalone policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationRightsV1(u32);

impl PublicationRightsV1 {
    /// No publication rights. This is the only value Policy V1 accepts.
    pub const NONE: Self = Self(0);

    /// Returns the canonical wire bits.
    pub const fn bits(self) -> u32 {
        self.0
    }
}

/// A top-level SHA-256 field in Policy V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PolicyDigestFieldV1 {
    /// The protected launcher executable image.
    LauncherExecutable,
    /// The `cargo-fe2o3` executable image.
    CargoFe2o3Executable,
    /// The complete child argument-vector commitment.
    ChildArgv,
}

impl fmt::Display for PolicyDigestFieldV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::LauncherExecutable => "launcher executable",
            Self::CargoFe2o3Executable => "cargo-fe2o3 executable",
            Self::ChildArgv => "child argv",
        };
        formatter.write_str(name)
    }
}

/// A strict canonical Policy V1 document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyV1 {
    serial: u64,
    launcher_executable_sha256: [u8; 32],
    cargo_fe2o3_executable_sha256: [u8; 32],
    compiler_closure: CompilerClosureV1,
    pipeline_allowlist: PipelineAllowlistV1,
    selected_pipeline: PipelineV1,
    child_argv_sha256: [u8; 32],
}

impl PolicyV1 {
    /// Constructs a standalone, staging-only Policy V1 document.
    pub fn new(
        serial: u64,
        launcher_executable_sha256: [u8; 32],
        cargo_fe2o3_executable_sha256: [u8; 32],
        compiler_closure: CompilerClosureV1,
        pipeline_allowlist: PipelineAllowlistV1,
        selected_pipeline: PipelineV1,
        child_argv_sha256: [u8; 32],
    ) -> Result<Self, PolicyErrorV1> {
        if serial == 0 {
            return Err(PolicyErrorV1::ZeroSerial);
        }
        for (field, digest) in [
            (
                PolicyDigestFieldV1::LauncherExecutable,
                launcher_executable_sha256,
            ),
            (
                PolicyDigestFieldV1::CargoFe2o3Executable,
                cargo_fe2o3_executable_sha256,
            ),
            (PolicyDigestFieldV1::ChildArgv, child_argv_sha256),
        ] {
            if digest == [0; 32] {
                return Err(PolicyErrorV1::ZeroDigest { field });
            }
        }
        if !pipeline_allowlist.allows(selected_pipeline) {
            return Err(PolicyErrorV1::SelectedPipelineNotAllowed {
                selected: selected_pipeline,
                allowlist_bits: pipeline_allowlist.bits(),
            });
        }
        Ok(Self {
            serial,
            launcher_executable_sha256,
            cargo_fe2o3_executable_sha256,
            compiler_closure,
            pipeline_allowlist,
            selected_pipeline,
            child_argv_sha256,
        })
    }

    /// Returns the nonzero anti-confusion serial carried by this policy.
    pub const fn serial(self) -> u64 {
        self.serial
    }

    /// Returns the launcher executable digest.
    pub const fn launcher_executable_sha256(self) -> [u8; 32] {
        self.launcher_executable_sha256
    }

    /// Returns the `cargo-fe2o3` executable digest.
    pub const fn cargo_fe2o3_executable_sha256(self) -> [u8; 32] {
        self.cargo_fe2o3_executable_sha256
    }

    /// Returns the only accepted execution profile.
    pub const fn profile(self) -> AuthorityProfileV1 {
        AuthorityProfileV1::StandaloneFoundation
    }

    /// Returns the validated compiler closure.
    pub const fn compiler_closure(self) -> CompilerClosureV1 {
        self.compiler_closure
    }

    /// Returns the pipeline allowlist.
    pub const fn pipeline_allowlist(self) -> PipelineAllowlistV1 {
        self.pipeline_allowlist
    }

    /// Returns the selected pipeline.
    pub const fn selected_pipeline(self) -> PipelineV1 {
        self.selected_pipeline
    }

    /// Returns the complete child argument-vector digest.
    pub const fn child_argv_sha256(self) -> [u8; 32] {
        self.child_argv_sha256
    }

    /// Returns the only accepted publication-rights value.
    pub const fn publication_rights(self) -> PublicationRightsV1 {
        PublicationRightsV1::NONE
    }

    /// Encodes this value as the exact canonical 432-byte Policy V1 format.
    pub fn encode(self) -> [u8; POLICY_V1_ENCODED_LEN] {
        encode_policy_v1(&self)
    }

    /// Computes the canonical identity of this policy's encoded bytes.
    pub fn identity_sha256(self) -> [u8; 32] {
        hash_canonical_policy_bytes(&self.encode())
    }
}

/// Why Policy V1 construction or canonical decoding failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PolicyErrorV1 {
    /// The supplied byte slice was not exactly 432 bytes.
    InvalidEncodedLength {
        /// The observed byte length.
        actual: usize,
    },
    /// The fixed header magic did not match Policy V1.
    InvalidMagic,
    /// The header version was not Policy V1.
    UnsupportedVersion {
        /// The observed version.
        actual: u16,
    },
    /// The fixed header length was not 32.
    InvalidHeaderLength {
        /// The observed header length.
        actual: u16,
    },
    /// The fixed field count was not 14.
    InvalidFieldCount {
        /// The observed field count.
        actual: u16,
    },
    /// A reserved header byte was nonzero.
    NonzeroHeaderReserved,
    /// Header flags were nonzero.
    UnsupportedHeaderFlags {
        /// The observed flags.
        actual: u32,
    },
    /// The declared total length was not 432.
    InvalidTotalLength {
        /// The observed declared length.
        actual: u32,
    },
    /// A TLV was not the exact expected tag at its canonical position.
    UnexpectedFieldTag {
        /// Zero-based field position.
        index: usize,
        /// The required tag.
        expected: u16,
        /// The observed tag.
        actual: u16,
    },
    /// A TLV carried nonzero flags.
    UnsupportedFieldFlags {
        /// The affected TLV tag.
        tag: u16,
        /// The observed flags.
        actual: u16,
    },
    /// A TLV value length was not the one fixed by the schema.
    InvalidFieldLength {
        /// The affected TLV tag.
        tag: u16,
        /// The required length.
        expected: u32,
        /// The observed length.
        actual: u32,
    },
    /// The required policy serial was zero.
    ZeroSerial,
    /// A required top-level SHA-256 digest was all zero.
    ZeroDigest {
        /// The rejected digest.
        field: PolicyDigestFieldV1,
    },
    /// The profile value is not assigned by this version.
    UnknownProfile {
        /// The observed profile value.
        value: u8,
    },
    /// A known future profile is not accepted by this standalone codec.
    ProfileNotPermitted {
        /// The observed reserved profile value.
        value: u8,
    },
    /// The exact target was not `gfx942:xnack-`.
    InvalidTarget,
    /// The allowlist contained unknown pipeline bits.
    UnknownPipelineAllowlistBits {
        /// The observed raw allowlist.
        bits: u32,
    },
    /// The selected pipeline ID was unknown.
    UnknownPipeline {
        /// The observed pipeline ID.
        value: u16,
    },
    /// The selected pipeline was absent from the allowlist.
    SelectedPipelineNotAllowed {
        /// The selected pipeline.
        selected: PipelineV1,
        /// The observed allowlist bits.
        allowlist_bits: u32,
    },
    /// The publication-rights value contained unknown bits.
    UnknownPublicationRightsBits {
        /// The observed raw rights.
        bits: u32,
    },
    /// A known publication right was requested by a foundation profile.
    PublicationRightsNotPermitted {
        /// The observed raw rights.
        bits: u32,
    },
    /// The compiler pins or declared aggregate were invalid.
    InvalidCompilerClosure(CompilerClosureErrorV1),
}

impl fmt::Display for PolicyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEncodedLength { actual } => write!(
                formatter,
                "Policy V1 must be exactly {POLICY_V1_ENCODED_LEN} bytes, got {actual}"
            ),
            Self::InvalidMagic => formatter.write_str("invalid Policy V1 magic"),
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "unsupported Policy V1 version {actual}")
            }
            Self::InvalidHeaderLength { actual } => {
                write!(formatter, "invalid Policy V1 header length {actual}")
            }
            Self::InvalidFieldCount { actual } => {
                write!(formatter, "invalid Policy V1 field count {actual}")
            }
            Self::NonzeroHeaderReserved => {
                formatter.write_str("Policy V1 reserved header bytes must be zero")
            }
            Self::UnsupportedHeaderFlags { actual } => {
                write!(formatter, "unsupported Policy V1 header flags {actual:#x}")
            }
            Self::InvalidTotalLength { actual } => {
                write!(formatter, "invalid Policy V1 declared length {actual}")
            }
            Self::UnexpectedFieldTag {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "Policy V1 field {index} has tag {actual:#06x}, expected {expected:#06x}"
            ),
            Self::UnsupportedFieldFlags { tag, actual } => write!(
                formatter,
                "Policy V1 field {tag:#06x} has unsupported flags {actual:#x}"
            ),
            Self::InvalidFieldLength {
                tag,
                expected,
                actual,
            } => write!(
                formatter,
                "Policy V1 field {tag:#06x} has length {actual}, expected {expected}"
            ),
            Self::ZeroSerial => formatter.write_str("Policy V1 serial must be nonzero"),
            Self::ZeroDigest { field } => write!(formatter, "{field} digest must be nonzero"),
            Self::UnknownProfile { value } => {
                write!(formatter, "unknown Policy V1 profile {value}")
            }
            Self::ProfileNotPermitted { value } => write!(
                formatter,
                "Policy V1 profile {value} is not permitted by the standalone foundation"
            ),
            Self::InvalidTarget => formatter.write_str("invalid Policy V1 target"),
            Self::UnknownPipelineAllowlistBits { bits } => {
                write!(formatter, "unknown Policy V1 pipeline bits {bits:#x}")
            }
            Self::UnknownPipeline { value } => {
                write!(formatter, "unknown Policy V1 selected pipeline {value}")
            }
            Self::SelectedPipelineNotAllowed {
                selected,
                allowlist_bits,
            } => write!(
                formatter,
                "selected pipeline {selected:?} is absent from allowlist {allowlist_bits:#x}"
            ),
            Self::UnknownPublicationRightsBits { bits } => {
                write!(
                    formatter,
                    "unknown Policy V1 publication-rights bits {bits:#x}"
                )
            }
            Self::PublicationRightsNotPermitted { bits } => write!(
                formatter,
                "publication rights {bits:#x} are not permitted by the foundation profile"
            ),
            Self::InvalidCompilerClosure(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for PolicyErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidCompilerClosure(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompilerClosureErrorV1> for PolicyErrorV1 {
    fn from(error: CompilerClosureErrorV1) -> Self {
        Self::InvalidCompilerClosure(error)
    }
}

/// Encodes a validated policy in the exact canonical Policy V1 format.
pub fn encode_policy_v1(policy: &PolicyV1) -> [u8; POLICY_V1_ENCODED_LEN] {
    let mut encoded = [0_u8; POLICY_V1_ENCODED_LEN];
    encoded[..8].copy_from_slice(&POLICY_V1_MAGIC);
    encoded[8..10].copy_from_slice(&POLICY_V1_VERSION.to_le_bytes());
    encoded[10..12].copy_from_slice(&POLICY_V1_HEADER_LEN.to_le_bytes());
    encoded[12..14].copy_from_slice(&POLICY_V1_FIELD_COUNT.to_le_bytes());
    encoded[16..20].copy_from_slice(&(POLICY_V1_ENCODED_LEN as u32).to_le_bytes());

    let compiler = policy.compiler_closure;
    let mut cursor = usize::from(POLICY_V1_HEADER_LEN);
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_SERIAL,
        &policy.serial.to_le_bytes(),
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_LAUNCHER_SHA256,
        &policy.launcher_executable_sha256,
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_CARGO_FE2O3_SHA256,
        &policy.cargo_fe2o3_executable_sha256,
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_PROFILE,
        &[AuthorityProfileV1::StandaloneFoundation as u8],
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_CARGO_SHA256,
        &compiler.cargo_executable_sha256(),
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_RUSTC_SHA256,
        &compiler.rustc_executable_sha256(),
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_RUNTIME_TREE_SHA256,
        &compiler.rustc_runtime_tree_sha256(),
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_BACKEND_SHA256,
        &compiler.codegen_backend_sha256(),
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_COMPILER_CLOSURE_SHA256,
        &compiler.identity_sha256(),
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_TARGET,
        POLICY_V1_TARGET.as_bytes(),
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_PIPELINE_ALLOWLIST,
        &policy.pipeline_allowlist.bits().to_le_bytes(),
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_SELECTED_PIPELINE,
        &(policy.selected_pipeline as u16).to_le_bytes(),
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_ARGV_SHA256,
        &policy.child_argv_sha256,
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_PUBLICATION_RIGHTS,
        &PublicationRightsV1::NONE.bits().to_le_bytes(),
    );
    debug_assert_eq!(cursor, POLICY_V1_ENCODED_LEN);
    encoded
}

/// Decodes and validates one exact canonical Policy V1 document.
pub fn decode_policy_v1(encoded: &[u8]) -> Result<PolicyV1, PolicyErrorV1> {
    if encoded.len() != POLICY_V1_ENCODED_LEN {
        return Err(PolicyErrorV1::InvalidEncodedLength {
            actual: encoded.len(),
        });
    }
    if encoded[..8] != POLICY_V1_MAGIC {
        return Err(PolicyErrorV1::InvalidMagic);
    }
    let version = read_u16(encoded, 8);
    if version != POLICY_V1_VERSION {
        return Err(PolicyErrorV1::UnsupportedVersion { actual: version });
    }
    let header_len = read_u16(encoded, 10);
    if header_len != POLICY_V1_HEADER_LEN {
        return Err(PolicyErrorV1::InvalidHeaderLength { actual: header_len });
    }
    let field_count = read_u16(encoded, 12);
    if field_count != POLICY_V1_FIELD_COUNT {
        return Err(PolicyErrorV1::InvalidFieldCount {
            actual: field_count,
        });
    }
    if encoded[14..16] != [0; 2] || encoded[24..32] != [0; 8] {
        return Err(PolicyErrorV1::NonzeroHeaderReserved);
    }
    let total_len = read_u32(encoded, 16);
    if total_len != POLICY_V1_ENCODED_LEN as u32 {
        return Err(PolicyErrorV1::InvalidTotalLength { actual: total_len });
    }
    let flags = read_u32(encoded, 20);
    if flags != 0 {
        return Err(PolicyErrorV1::UnsupportedHeaderFlags { actual: flags });
    }

    let mut cursor = usize::from(POLICY_V1_HEADER_LEN);
    let mut fields: [&[u8]; POLICY_V1_FIELD_COUNT as usize] = [&[]; POLICY_V1_FIELD_COUNT as usize];
    for (index, (tag, length)) in FIELD_SPECS.into_iter().enumerate() {
        fields[index] = read_field(encoded, &mut cursor, index, tag, length)?;
    }
    debug_assert_eq!(cursor, POLICY_V1_ENCODED_LEN);

    let serial = u64::from_le_bytes(fields[0].try_into().expect("fixed serial field length"));
    let launcher = digest_from_field(fields[1]);
    let cargo_fe2o3 = digest_from_field(fields[2]);
    match fields[3][0] {
        0 => {}
        PROFILE_RESERVED_TRUSTED_SERVICE => {
            return Err(PolicyErrorV1::ProfileNotPermitted {
                value: PROFILE_RESERVED_TRUSTED_SERVICE,
            });
        }
        value => return Err(PolicyErrorV1::UnknownProfile { value }),
    }
    let compiler = CompilerClosureV1::from_pins_and_identity(
        digest_from_field(fields[4]),
        digest_from_field(fields[5]),
        digest_from_field(fields[6]),
        digest_from_field(fields[7]),
        digest_from_field(fields[8]),
    )?;
    if fields[9] != POLICY_V1_TARGET.as_bytes() {
        return Err(PolicyErrorV1::InvalidTarget);
    }
    let allowlist = PipelineAllowlistV1::from_bits(u32::from_le_bytes(
        fields[10]
            .try_into()
            .expect("fixed pipeline allowlist field length"),
    ))?;
    let selected = PipelineV1::from_wire(u16::from_le_bytes(
        fields[11]
            .try_into()
            .expect("fixed selected pipeline field length"),
    ))?;
    let argv = digest_from_field(fields[12]);
    let rights = u32::from_le_bytes(
        fields[13]
            .try_into()
            .expect("fixed publication-rights field length"),
    );
    if rights & !KNOWN_PUBLICATION_RIGHTS != 0 {
        return Err(PolicyErrorV1::UnknownPublicationRightsBits { bits: rights });
    }
    if rights != 0 {
        return Err(PolicyErrorV1::PublicationRightsNotPermitted { bits: rights });
    }

    PolicyV1::new(
        serial,
        launcher,
        cargo_fe2o3,
        compiler,
        allowlist,
        selected,
        argv,
    )
}

/// Validates canonical Policy V1 bytes and computes their domain-separated identity.
pub fn policy_identity_sha256_v1(encoded: &[u8]) -> Result<[u8; 32], PolicyErrorV1> {
    decode_policy_v1(encoded)?;
    Ok(hash_canonical_policy_bytes(encoded))
}

fn write_field(
    encoded: &mut [u8; POLICY_V1_ENCODED_LEN],
    cursor: &mut usize,
    tag: u16,
    value: &[u8],
) {
    encoded[*cursor..*cursor + 2].copy_from_slice(&tag.to_le_bytes());
    encoded[*cursor + 4..*cursor + 8].copy_from_slice(&(value.len() as u32).to_le_bytes());
    let value_start = *cursor + 8;
    let value_end = value_start + value.len();
    encoded[value_start..value_end].copy_from_slice(value);
    *cursor = value_end;
}

fn read_field<'a>(
    encoded: &'a [u8],
    cursor: &mut usize,
    index: usize,
    expected_tag: u16,
    expected_length: u32,
) -> Result<&'a [u8], PolicyErrorV1> {
    let tag = read_u16(encoded, *cursor);
    if tag != expected_tag {
        return Err(PolicyErrorV1::UnexpectedFieldTag {
            index,
            expected: expected_tag,
            actual: tag,
        });
    }
    let flags = read_u16(encoded, *cursor + 2);
    if flags != 0 {
        return Err(PolicyErrorV1::UnsupportedFieldFlags { tag, actual: flags });
    }
    let length = read_u32(encoded, *cursor + 4);
    if length != expected_length {
        return Err(PolicyErrorV1::InvalidFieldLength {
            tag,
            expected: expected_length,
            actual: length,
        });
    }
    let value_start = *cursor + 8;
    let value_end = value_start + expected_length as usize;
    *cursor = value_end;
    Ok(&encoded[value_start..value_end])
}

fn read_u16(encoded: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        encoded[offset..offset + 2]
            .try_into()
            .expect("fixed Policy V1 bounds"),
    )
}

fn read_u32(encoded: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        encoded[offset..offset + 4]
            .try_into()
            .expect("fixed Policy V1 bounds"),
    )
}

fn digest_from_field(field: &[u8]) -> [u8; 32] {
    field.try_into().expect("fixed SHA-256 field length")
}

fn hash_canonical_policy_bytes(encoded: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(POLICY_IDENTITY_DOMAIN_V1);
    digest.update((POLICY_V1_ENCODED_LEN as u64).to_le_bytes());
    digest.update(encoded);
    digest.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> PolicyV1 {
        PolicyV1::new(
            7,
            [1; 32],
            [2; 32],
            CompilerClosureV1::new([5; 32], [6; 32], [7; 32], [8; 32]).unwrap(),
            PipelineAllowlistV1::ALL,
            PipelineV1::CollectedTiledGemm,
            [9; 32],
        )
        .unwrap()
    }

    #[test]
    fn policy_roundtrips_at_the_exact_fixed_length() {
        let policy = policy();
        let encoded = policy.encode();
        assert_eq!(encoded.len(), POLICY_V1_ENCODED_LEN);
        assert_eq!(decode_policy_v1(&encoded), Ok(policy));
        assert_eq!(
            policy_identity_sha256_v1(&encoded),
            Ok(policy.identity_sha256())
        );
    }
}
