use std::fmt;

use sha2::{Digest, Sha256};

use crate::pipeline_v2::{PipelineAllowlistV2, PipelineErrorV2, PipelineV2};
use crate::{
    CompilerClosureErrorV1, CompilerClosureErrorV2, CompilerClosureV1, CompilerClosureV2,
    PolicyErrorV1, PolicyV1, PublicationRightsV1,
};

/// Exact encoded byte length of every Policy V2 document.
pub const POLICY_V2_ENCODED_LEN: usize = 522;
/// Exact Policy V2 header byte length.
pub const POLICY_V2_HEADER_LEN: u16 = 32;
/// Exact number of Policy V2 TLV fields.
pub const POLICY_V2_FIELD_COUNT: u16 = 17;
/// Policy V2 wire-format version.
pub const POLICY_V2_VERSION: u16 = 2;
/// Policy V2 header magic.
pub const POLICY_V2_MAGIC: [u8; 8] = *b"F2AUPOL2";
/// The only target accepted by Policy V2.
pub const POLICY_V2_TARGET: &str = "gfx942:xnack-";
/// Domain for the canonical Policy V2 content identity.
pub const POLICY_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/PROTECTED-AUTHORITY-POLICY/V2\0";

const PROFILE_RESERVED_TRUSTED_SERVICE: u8 = 1;
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
const TAG_CARGO_BINDING_TRAMPOLINE_SHA256: u16 = 0x0015;
const TAG_CARGO_FE2O3_BINDING_WRAPPER_SHA256: u16 = 0x0016;
const TAG_CARGO_BINDING_TRANSITION_PROTOCOL_VERSION: u16 = 0x0017;
const TAG_TARGET: u16 = 0x0020;
const TAG_PIPELINE_ALLOWLIST: u16 = 0x0021;
const TAG_SELECTED_PIPELINE: u16 = 0x0022;
const TAG_ARGV_SHA256: u16 = 0x0023;
const TAG_PUBLICATION_RIGHTS: u16 = 0x0024;

const FIELD_SPECS: [(u16, u32); POLICY_V2_FIELD_COUNT as usize] = [
    (TAG_SERIAL, 8),
    (TAG_LAUNCHER_SHA256, 32),
    (TAG_CARGO_FE2O3_SHA256, 32),
    (TAG_PROFILE, 1),
    (TAG_CARGO_SHA256, 32),
    (TAG_RUSTC_SHA256, 32),
    (TAG_RUNTIME_TREE_SHA256, 32),
    (TAG_BACKEND_SHA256, 32),
    (TAG_COMPILER_CLOSURE_SHA256, 32),
    (TAG_CARGO_BINDING_TRAMPOLINE_SHA256, 32),
    (TAG_CARGO_FE2O3_BINDING_WRAPPER_SHA256, 32),
    (TAG_CARGO_BINDING_TRANSITION_PROTOCOL_VERSION, 2),
    (TAG_TARGET, 13),
    (TAG_PIPELINE_ALLOWLIST, 4),
    (TAG_SELECTED_PIPELINE, 2),
    (TAG_ARGV_SHA256, 32),
    (TAG_PUBLICATION_RIGHTS, 4),
];

/// The execution profile accepted by the standalone Policy V2 codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AuthorityProfileV2 {
    /// A staging-only profile with no publication authority.
    StandaloneFoundation = 0,
}

/// Publication rights carried by an accepted standalone Policy V2 document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationRightsV2(u32);

impl PublicationRightsV2 {
    /// No publication rights. This is the only value Policy V2 accepts.
    pub const NONE: Self = Self(0);

    /// Returns the canonical wire bits.
    pub const fn bits(self) -> u32 {
        self.0
    }
}

/// A top-level SHA-256 field in Policy V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PolicyDigestFieldV2 {
    /// The protected launcher executable image.
    LauncherExecutable,
    /// The `cargo-fe2o3` executable image.
    CargoFe2o3Executable,
    /// The complete child argument-vector commitment.
    ChildArgv,
}

impl fmt::Display for PolicyDigestFieldV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::LauncherExecutable => "launcher executable",
            Self::CargoFe2o3Executable => "cargo-fe2o3 executable",
            Self::ChildArgv => "child argv",
        };
        formatter.write_str(name)
    }
}

/// A strict canonical Policy V2 document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyV2 {
    serial: u64,
    launcher_executable_sha256: [u8; 32],
    cargo_fe2o3_executable_sha256: [u8; 32],
    compiler_closure: CompilerClosureV2,
    pipeline_allowlist: PipelineAllowlistV2,
    selected_pipeline: PipelineV2,
    child_argv_sha256: [u8; 32],
}

impl PolicyV2 {
    /// Constructs a standalone, staging-only Policy V2 document.
    pub fn new(
        serial: u64,
        launcher_executable_sha256: [u8; 32],
        cargo_fe2o3_executable_sha256: [u8; 32],
        compiler_closure: CompilerClosureV2,
        pipeline_allowlist: PipelineAllowlistV2,
        selected_pipeline: PipelineV2,
        child_argv_sha256: [u8; 32],
    ) -> Result<Self, PolicyErrorV2> {
        if serial == 0 {
            return Err(PolicyErrorV2::ZeroSerial);
        }
        for (field, digest) in [
            (
                PolicyDigestFieldV2::LauncherExecutable,
                launcher_executable_sha256,
            ),
            (
                PolicyDigestFieldV2::CargoFe2o3Executable,
                cargo_fe2o3_executable_sha256,
            ),
            (PolicyDigestFieldV2::ChildArgv, child_argv_sha256),
        ] {
            if digest == [0; 32] {
                return Err(PolicyErrorV2::ZeroDigest { field });
            }
        }
        if !pipeline_allowlist.allows(selected_pipeline) {
            return Err(PolicyErrorV2::SelectedPipelineNotAllowed {
                selected: selected_pipeline,
                allowlist_bits: pipeline_allowlist.bits(),
            });
        }
        if cargo_fe2o3_executable_sha256 != compiler_closure.cargo_fe2o3_binding_wrapper_sha256() {
            return Err(PolicyErrorV2::CargoFe2o3BindingWrapperMismatch);
        }
        let cargo = compiler_closure.cargo_executable_sha256();
        let trampoline = compiler_closure.cargo_binding_trampoline_sha256();
        let wrapper = compiler_closure.cargo_fe2o3_binding_wrapper_sha256();
        if cargo == trampoline || cargo == wrapper || trampoline == wrapper {
            return Err(PolicyErrorV2::CargoTransitionImageDigestsNotDistinct);
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

    /// Upgrades Policy V1 with the additional trampoline pin required by the V2 transition.
    pub fn from_policy_v1(
        policy: PolicyV1,
        cargo_binding_trampoline_sha256: [u8; 32],
    ) -> Result<Self, PolicyErrorV2> {
        let compiler_v1 = policy.compiler_closure();
        let compiler_v2 = CompilerClosureV2::new(
            compiler_v1.cargo_executable_sha256(),
            cargo_binding_trampoline_sha256,
            policy.cargo_fe2o3_executable_sha256(),
            compiler_v1.rustc_executable_sha256(),
            compiler_v1.rustc_runtime_tree_sha256(),
            compiler_v1.codegen_backend_sha256(),
        )?;
        Self::new(
            policy.serial(),
            policy.launcher_executable_sha256(),
            policy.cargo_fe2o3_executable_sha256(),
            compiler_v2,
            PipelineAllowlistV2::from(policy.pipeline_allowlist()),
            PipelineV2::from(policy.selected_pipeline()),
            policy.child_argv_sha256(),
        )
    }

    /// Downgrades a legacy-lane value, dropping V2's trampoline and transition-protocol pins.
    ///
    /// Production V1 cannot be downgraded. The caller must separately decide whether Policy V1's
    /// weaker four-pin compiler-closure model is acceptable for the destination.
    pub fn try_into_policy_v1(self) -> Result<PolicyV1, PolicyCompatibilityErrorV2> {
        let allowlist = self.pipeline_allowlist.try_into()?;
        let selected = self.selected_pipeline.try_into()?;
        let compiler = CompilerClosureV1::new(
            self.compiler_closure.cargo_executable_sha256(),
            self.compiler_closure.rustc_executable_sha256(),
            self.compiler_closure.rustc_runtime_tree_sha256(),
            self.compiler_closure.codegen_backend_sha256(),
        )
        .map_err(PolicyCompatibilityErrorV2::InvalidCompilerClosureV1)?;
        PolicyV1::new(
            self.serial,
            self.launcher_executable_sha256,
            self.cargo_fe2o3_executable_sha256,
            compiler,
            allowlist,
            selected,
            self.child_argv_sha256,
        )
        .map_err(PolicyCompatibilityErrorV2::InvalidPolicyV1)
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
    pub const fn profile(self) -> AuthorityProfileV2 {
        AuthorityProfileV2::StandaloneFoundation
    }

    /// Returns the validated compiler closure.
    pub const fn compiler_closure(self) -> CompilerClosureV2 {
        self.compiler_closure
    }

    /// Returns the pipeline allowlist.
    pub const fn pipeline_allowlist(self) -> PipelineAllowlistV2 {
        self.pipeline_allowlist
    }

    /// Returns the selected pipeline.
    pub const fn selected_pipeline(self) -> PipelineV2 {
        self.selected_pipeline
    }

    /// Returns the complete child argument-vector digest.
    pub const fn child_argv_sha256(self) -> [u8; 32] {
        self.child_argv_sha256
    }

    /// Returns the only accepted publication-rights value.
    pub const fn publication_rights(self) -> PublicationRightsV2 {
        PublicationRightsV2::NONE
    }

    /// Encodes this value as the exact canonical 522-byte Policy V2 format.
    pub fn encode(self) -> [u8; POLICY_V2_ENCODED_LEN] {
        encode_policy_v2(&self)
    }

    /// Computes the canonical identity of this policy's encoded bytes.
    pub fn identity_sha256(self) -> [u8; 32] {
        hash_canonical_policy_bytes(&self.encode())
    }
}

impl TryFrom<PolicyV2> for PolicyV1 {
    type Error = PolicyCompatibilityErrorV2;

    fn try_from(value: PolicyV2) -> Result<Self, Self::Error> {
        value.try_into_policy_v1()
    }
}

/// Why a Policy V2 value could not be represented by Policy V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PolicyCompatibilityErrorV2 {
    /// A V2 pipeline or allowlist has no V1 representation.
    InvalidPipeline(PipelineErrorV2),
    /// Reconstructing the four-pin V1 compiler closure failed validation.
    InvalidCompilerClosureV1(CompilerClosureErrorV1),
    /// Reconstructing the V1 value failed validation.
    InvalidPolicyV1(PolicyErrorV1),
}

impl fmt::Display for PolicyCompatibilityErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPipeline(error) => write!(formatter, "{error}"),
            Self::InvalidCompilerClosureV1(error) => write!(formatter, "{error}"),
            Self::InvalidPolicyV1(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for PolicyCompatibilityErrorV2 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPipeline(error) => Some(error),
            Self::InvalidCompilerClosureV1(error) => Some(error),
            Self::InvalidPolicyV1(error) => Some(error),
        }
    }
}

impl From<PipelineErrorV2> for PolicyCompatibilityErrorV2 {
    fn from(error: PipelineErrorV2) -> Self {
        Self::InvalidPipeline(error)
    }
}

/// Why Policy V2 construction or canonical decoding failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PolicyErrorV2 {
    /// The supplied byte slice was not exactly 522 bytes.
    InvalidEncodedLength {
        /// The observed byte length.
        actual: usize,
    },
    /// The fixed header magic did not match Policy V2.
    InvalidMagic,
    /// The header version was not Policy V2.
    UnsupportedVersion {
        /// The observed version.
        actual: u16,
    },
    /// The fixed header length was not 32.
    InvalidHeaderLength {
        /// The observed header length.
        actual: u16,
    },
    /// The fixed field count was not 17.
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
    /// The declared total length was not 522.
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
        field: PolicyDigestFieldV2,
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
    /// A pipeline ID or allowlist was invalid.
    InvalidPipeline(PipelineErrorV2),
    /// The selected pipeline was absent from the allowlist.
    SelectedPipelineNotAllowed {
        /// The selected pipeline.
        selected: PipelineV2,
        /// The observed allowlist bits.
        allowlist_bits: u32,
    },
    /// The top-level cargo-fe2o3 pin did not name the V2 closure's full wrapper.
    CargoFe2o3BindingWrapperMismatch,
    /// Cargo, the static trampoline, and the full wrapper did not have distinct digests.
    CargoTransitionImageDigestsNotDistinct,
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
    InvalidCompilerClosure(CompilerClosureErrorV2),
}

impl fmt::Display for PolicyErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEncodedLength { actual } => write!(
                formatter,
                "Policy V2 must be exactly {POLICY_V2_ENCODED_LEN} bytes, got {actual}"
            ),
            Self::InvalidMagic => formatter.write_str("invalid Policy V2 magic"),
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "unsupported Policy V2 version {actual}")
            }
            Self::InvalidHeaderLength { actual } => {
                write!(formatter, "invalid Policy V2 header length {actual}")
            }
            Self::InvalidFieldCount { actual } => {
                write!(formatter, "invalid Policy V2 field count {actual}")
            }
            Self::NonzeroHeaderReserved => {
                formatter.write_str("Policy V2 reserved header bytes must be zero")
            }
            Self::UnsupportedHeaderFlags { actual } => {
                write!(formatter, "unsupported Policy V2 header flags {actual:#x}")
            }
            Self::InvalidTotalLength { actual } => {
                write!(formatter, "invalid Policy V2 declared length {actual}")
            }
            Self::UnexpectedFieldTag {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "Policy V2 field {index} has tag {actual:#06x}, expected {expected:#06x}"
            ),
            Self::UnsupportedFieldFlags { tag, actual } => write!(
                formatter,
                "Policy V2 field {tag:#06x} has unsupported flags {actual:#x}"
            ),
            Self::InvalidFieldLength {
                tag,
                expected,
                actual,
            } => write!(
                formatter,
                "Policy V2 field {tag:#06x} has length {actual}, expected {expected}"
            ),
            Self::ZeroSerial => formatter.write_str("Policy V2 serial must be nonzero"),
            Self::ZeroDigest { field } => write!(formatter, "{field} digest must be nonzero"),
            Self::UnknownProfile { value } => {
                write!(formatter, "unknown Policy V2 profile {value}")
            }
            Self::ProfileNotPermitted { value } => write!(
                formatter,
                "Policy V2 profile {value} is not permitted by the standalone foundation"
            ),
            Self::InvalidTarget => formatter.write_str("invalid Policy V2 target"),
            Self::InvalidPipeline(error) => write!(formatter, "{error}"),
            Self::SelectedPipelineNotAllowed {
                selected,
                allowlist_bits,
            } => write!(
                formatter,
                "selected pipeline {selected:?} is absent from allowlist {allowlist_bits:#x}"
            ),
            Self::CargoFe2o3BindingWrapperMismatch => formatter.write_str(
                "Policy V2 cargo-fe2o3 executable does not match its compiler-closure wrapper",
            ),
            Self::CargoTransitionImageDigestsNotDistinct => formatter.write_str(
                "Policy V2 Cargo transition executable digests must be pairwise distinct",
            ),
            Self::UnknownPublicationRightsBits { bits } => {
                write!(
                    formatter,
                    "unknown Policy V2 publication-rights bits {bits:#x}"
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

impl std::error::Error for PolicyErrorV2 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPipeline(error) => Some(error),
            Self::InvalidCompilerClosure(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompilerClosureErrorV2> for PolicyErrorV2 {
    fn from(error: CompilerClosureErrorV2) -> Self {
        Self::InvalidCompilerClosure(error)
    }
}

impl From<PipelineErrorV2> for PolicyErrorV2 {
    fn from(error: PipelineErrorV2) -> Self {
        Self::InvalidPipeline(error)
    }
}

/// Encodes a validated policy in the exact canonical Policy V2 format.
pub fn encode_policy_v2(policy: &PolicyV2) -> [u8; POLICY_V2_ENCODED_LEN] {
    let mut encoded = [0_u8; POLICY_V2_ENCODED_LEN];
    encoded[..8].copy_from_slice(&POLICY_V2_MAGIC);
    encoded[8..10].copy_from_slice(&POLICY_V2_VERSION.to_le_bytes());
    encoded[10..12].copy_from_slice(&POLICY_V2_HEADER_LEN.to_le_bytes());
    encoded[12..14].copy_from_slice(&POLICY_V2_FIELD_COUNT.to_le_bytes());
    encoded[16..20].copy_from_slice(&(POLICY_V2_ENCODED_LEN as u32).to_le_bytes());

    let compiler = policy.compiler_closure;
    let mut cursor = usize::from(POLICY_V2_HEADER_LEN);
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
        &[AuthorityProfileV2::StandaloneFoundation as u8],
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
        TAG_CARGO_BINDING_TRAMPOLINE_SHA256,
        &compiler.cargo_binding_trampoline_sha256(),
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_CARGO_FE2O3_BINDING_WRAPPER_SHA256,
        &compiler.cargo_fe2o3_binding_wrapper_sha256(),
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_CARGO_BINDING_TRANSITION_PROTOCOL_VERSION,
        &compiler
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    write_field(
        &mut encoded,
        &mut cursor,
        TAG_TARGET,
        POLICY_V2_TARGET.as_bytes(),
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
        &policy.selected_pipeline.wire_value().to_le_bytes(),
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
        &PublicationRightsV2::NONE.bits().to_le_bytes(),
    );
    debug_assert_eq!(cursor, POLICY_V2_ENCODED_LEN);
    encoded
}

/// Decodes and validates one exact canonical Policy V2 document.
pub fn decode_policy_v2(encoded: &[u8]) -> Result<PolicyV2, PolicyErrorV2> {
    if encoded.len() != POLICY_V2_ENCODED_LEN {
        return Err(PolicyErrorV2::InvalidEncodedLength {
            actual: encoded.len(),
        });
    }
    if encoded[..8] != POLICY_V2_MAGIC {
        return Err(PolicyErrorV2::InvalidMagic);
    }
    let version = read_u16(encoded, 8);
    if version != POLICY_V2_VERSION {
        return Err(PolicyErrorV2::UnsupportedVersion { actual: version });
    }
    let header_len = read_u16(encoded, 10);
    if header_len != POLICY_V2_HEADER_LEN {
        return Err(PolicyErrorV2::InvalidHeaderLength { actual: header_len });
    }
    let field_count = read_u16(encoded, 12);
    if field_count != POLICY_V2_FIELD_COUNT {
        return Err(PolicyErrorV2::InvalidFieldCount {
            actual: field_count,
        });
    }
    if encoded[14..16] != [0; 2] || encoded[24..32] != [0; 8] {
        return Err(PolicyErrorV2::NonzeroHeaderReserved);
    }
    let total_len = read_u32(encoded, 16);
    if total_len != POLICY_V2_ENCODED_LEN as u32 {
        return Err(PolicyErrorV2::InvalidTotalLength { actual: total_len });
    }
    let flags = read_u32(encoded, 20);
    if flags != 0 {
        return Err(PolicyErrorV2::UnsupportedHeaderFlags { actual: flags });
    }

    let mut cursor = usize::from(POLICY_V2_HEADER_LEN);
    let mut fields: [&[u8]; POLICY_V2_FIELD_COUNT as usize] = [&[]; POLICY_V2_FIELD_COUNT as usize];
    for (index, (tag, length)) in FIELD_SPECS.into_iter().enumerate() {
        fields[index] = read_field(encoded, &mut cursor, index, tag, length)?;
    }
    debug_assert_eq!(cursor, POLICY_V2_ENCODED_LEN);

    let serial = u64::from_le_bytes(fields[0].try_into().expect("fixed serial field length"));
    let launcher = digest_from_field(fields[1]);
    let cargo_fe2o3 = digest_from_field(fields[2]);
    match fields[3][0] {
        0 => {}
        PROFILE_RESERVED_TRUSTED_SERVICE => {
            return Err(PolicyErrorV2::ProfileNotPermitted {
                value: PROFILE_RESERVED_TRUSTED_SERVICE,
            });
        }
        value => return Err(PolicyErrorV2::UnknownProfile { value }),
    }
    let compiler = CompilerClosureV2::from_pins_and_identity(
        digest_from_field(fields[4]),
        digest_from_field(fields[9]),
        digest_from_field(fields[10]),
        digest_from_field(fields[5]),
        digest_from_field(fields[6]),
        digest_from_field(fields[7]),
        u16::from_le_bytes(
            fields[11]
                .try_into()
                .expect("fixed transition protocol field length"),
        ),
        digest_from_field(fields[8]),
    )?;
    if fields[12] != POLICY_V2_TARGET.as_bytes() {
        return Err(PolicyErrorV2::InvalidTarget);
    }
    let allowlist = PipelineAllowlistV2::from_bits(u32::from_le_bytes(
        fields[13]
            .try_into()
            .expect("fixed pipeline allowlist field length"),
    ))?;
    let selected = PipelineV2::try_from(u16::from_le_bytes(
        fields[14]
            .try_into()
            .expect("fixed selected pipeline field length"),
    ))?;
    let argv = digest_from_field(fields[15]);
    let rights = u32::from_le_bytes(
        fields[16]
            .try_into()
            .expect("fixed publication-rights field length"),
    );
    if rights & !KNOWN_PUBLICATION_RIGHTS != 0 {
        return Err(PolicyErrorV2::UnknownPublicationRightsBits { bits: rights });
    }
    if rights != 0 {
        return Err(PolicyErrorV2::PublicationRightsNotPermitted { bits: rights });
    }

    PolicyV2::new(
        serial,
        launcher,
        cargo_fe2o3,
        compiler,
        allowlist,
        selected,
        argv,
    )
}

/// Validates canonical Policy V2 bytes and computes their domain-separated identity.
pub fn policy_identity_sha256_v2(encoded: &[u8]) -> Result<[u8; 32], PolicyErrorV2> {
    decode_policy_v2(encoded)?;
    Ok(hash_canonical_policy_bytes(encoded))
}

fn write_field(
    encoded: &mut [u8; POLICY_V2_ENCODED_LEN],
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
) -> Result<&'a [u8], PolicyErrorV2> {
    let tag = read_u16(encoded, *cursor);
    if tag != expected_tag {
        return Err(PolicyErrorV2::UnexpectedFieldTag {
            index,
            expected: expected_tag,
            actual: tag,
        });
    }
    let flags = read_u16(encoded, *cursor + 2);
    if flags != 0 {
        return Err(PolicyErrorV2::UnsupportedFieldFlags { tag, actual: flags });
    }
    let length = read_u32(encoded, *cursor + 4);
    if length != expected_length {
        return Err(PolicyErrorV2::InvalidFieldLength {
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
            .expect("fixed Policy V2 bounds"),
    )
}

fn read_u32(encoded: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        encoded[offset..offset + 4]
            .try_into()
            .expect("fixed Policy V2 bounds"),
    )
}

fn digest_from_field(field: &[u8]) -> [u8; 32] {
    field.try_into().expect("fixed SHA-256 field length")
}

fn hash_canonical_policy_bytes(encoded: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(POLICY_IDENTITY_DOMAIN_V2);
    digest.update((POLICY_V2_ENCODED_LEN as u64).to_le_bytes());
    digest.update(encoded);
    digest.finalize().into()
}

const _: () = {
    assert!(PublicationRightsV1::NONE.bits() == PublicationRightsV2::NONE.bits());
};
