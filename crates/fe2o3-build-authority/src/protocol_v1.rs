use std::fmt;

use sha2::{Digest, Sha256};

use crate::{
    AuthorityProfileV1, CompilerClosureErrorV1, CompilerClosureV1, PipelineV1, PolicyErrorV1,
    PolicyV1, PublicationRightsV1,
};

/// Protocol V1 frame magic.
pub const PROTOCOL_V1_MAGIC: [u8; 8] = *b"F2AUPR1\0";
/// Protocol V1 wire version.
pub const PROTOCOL_V1_VERSION: u16 = 1;
/// Exact Protocol V1 frame-header length.
pub const PROTOCOL_V1_HEADER_LEN: usize = 8 + 2 + 2 + 4 + 4 + 4;
/// Length of every SHA-256 identity in Protocol V1.
pub const IDENTITY_V1_LEN: usize = 32;
/// Length of every Protocol V1 nonce.
pub const NONCE_V1_LEN: usize = 32;

/// Fixed `argv[0]` covered by the protected authority argument identity.
pub const PROTECTED_AUTHORITY_ARGV0_V1: &[u8] = b"/usr/libexec/fe2o3/cargo-fe2o3";
/// Maximum complete child argument count, including fixed `argv[0]`.
pub const PROTOCOL_V1_MAX_ARGUMENTS: usize = 257;
/// Maximum byte length of one argument.
pub const PROTOCOL_V1_MAX_ARGUMENT_BYTES: usize = 4096;
/// Maximum encoded bytes of forwarded arguments, including one terminator per argument.
pub const PROTOCOL_V1_MAX_TOTAL_ARGUMENT_BYTES: usize = 65_536;

/// Domain for the exact protected authority argument-vector identity.
pub const ARGUMENT_VECTOR_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-AUTHORITY-ARGV/V1\0";
/// Domain for the canonical Attest V1 commitment.
pub const ATTESTATION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-AUTHORITY-ATTEST/V1\0";
/// Domain for the canonical Grant V1 admission identity.
pub const ADMISSION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-AUTHORITY-ADMISSION/V1\0";

const U16_LEN: usize = 2;
const U32_LEN: usize = 4;
const U64_LEN: usize = 8;
const PROFILE_LEN: usize = 1;
const CHALLENGE_RESERVED_LEN: usize = 7;
const ATTEST_RESERVED_LEN: usize = 3;
const GRANT_RESERVED_LEN: usize = 3;
const DENY_RESERVED_LEN: usize = 2;
const COMPILER_PIN_COUNT: usize = 4;
const COMPILER_CLOSURE_WIRE_LEN: usize = (COMPILER_PIN_COUNT + 1) * IDENTITY_V1_LEN;

/// Exact Challenge V1 payload length, derived from its fixed fields.
pub const CHALLENGE_V1_PAYLOAD_LEN: usize =
    NONCE_V1_LEN + (4 * IDENTITY_V1_LEN) + U64_LEN + PROFILE_LEN + CHALLENGE_RESERVED_LEN;
/// Exact Attest V1 payload length, derived from its fixed fields.
pub const ATTEST_V1_PAYLOAD_LEN: usize = NONCE_V1_LEN
    + (4 * IDENTITY_V1_LEN)
    + COMPILER_CLOSURE_WIRE_LEN
    + U16_LEN
    + U16_LEN
    + PROFILE_LEN
    + U32_LEN
    + ATTEST_RESERVED_LEN
    + IDENTITY_V1_LEN;
/// Exact Grant V1 payload length, derived from its fixed fields.
pub const GRANT_V1_PAYLOAD_LEN: usize = NONCE_V1_LEN
    + (3 * IDENTITY_V1_LEN)
    + U16_LEN
    + U16_LEN
    + PROFILE_LEN
    + U32_LEN
    + GRANT_RESERVED_LEN
    + IDENTITY_V1_LEN;
/// Exact Deny V1 payload length, derived from its fixed fields.
pub const DENY_V1_PAYLOAD_LEN: usize =
    NONCE_V1_LEN + (2 * IDENTITY_V1_LEN) + U16_LEN + DENY_RESERVED_LEN;
/// Exact Accept V1 payload length, derived from its fixed field.
pub const ACCEPT_V1_PAYLOAD_LEN: usize = IDENTITY_V1_LEN;

const CHALLENGE_SEQUENCE: u32 = 0;
const ATTEST_SEQUENCE: u32 = 1;
const DECISION_SEQUENCE: u32 = 2;
const ACCEPT_SEQUENCE: u32 = 3;
const ATTEST_COMMITMENT_OFFSET: usize = ATTEST_V1_PAYLOAD_LEN - IDENTITY_V1_LEN;
const GRANT_ADMISSION_OFFSET: usize = GRANT_V1_PAYLOAD_LEN - IDENTITY_V1_LEN;

/// One assigned Protocol V1 message type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum FrameKindV1 {
    /// Launcher challenge, sequence zero.
    Challenge = 1,
    /// Rust-side attestation, sequence one.
    Attest = 2,
    /// Foundation admission without publication rights, sequence two.
    Grant = 3,
    /// Typed denial, sequence two.
    Deny = 4,
    /// Rust-side admission acceptance, sequence three.
    Accept = 5,
}

impl FrameKindV1 {
    const fn sequence(self) -> u32 {
        match self {
            Self::Challenge => CHALLENGE_SEQUENCE,
            Self::Attest => ATTEST_SEQUENCE,
            Self::Grant | Self::Deny => DECISION_SEQUENCE,
            Self::Accept => ACCEPT_SEQUENCE,
        }
    }

    const fn payload_len(self) -> usize {
        match self {
            Self::Challenge => CHALLENGE_V1_PAYLOAD_LEN,
            Self::Attest => ATTEST_V1_PAYLOAD_LEN,
            Self::Grant => GRANT_V1_PAYLOAD_LEN,
            Self::Deny => DENY_V1_PAYLOAD_LEN,
            Self::Accept => ACCEPT_V1_PAYLOAD_LEN,
        }
    }

    fn from_wire(value: u16) -> Result<Self, ProtocolErrorV1> {
        match value {
            1 => Ok(Self::Challenge),
            2 => Ok(Self::Attest),
            3 => Ok(Self::Grant),
            4 => Ok(Self::Deny),
            5 => Ok(Self::Accept),
            _ => Err(ProtocolErrorV1::UnknownFrameType { actual: value }),
        }
    }
}

/// The only target assigned by Protocol V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum ProtocolTargetV1 {
    /// AMD gfx942 with XNACK disabled.
    Gfx942XnackMinus = 1,
}

impl ProtocolTargetV1 {
    fn from_wire(value: u16) -> Result<Self, ProtocolErrorV1> {
        match value {
            1 => Ok(Self::Gfx942XnackMinus),
            _ => Err(ProtocolErrorV1::UnknownTarget { actual: value }),
        }
    }
}

/// A bounded reason carried by Deny V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum DenyReasonV1 {
    /// The policy document was rejected.
    PolicyRejected = 1,
    /// A protected executable identity did not match.
    ExecutableIdentityMismatch = 2,
    /// The complete argument-vector identity did not match.
    ArgumentVectorMismatch = 3,
    /// Compiler pins or their closure identity did not match.
    CompilerClosureMismatch = 4,
    /// The target was not admitted.
    TargetNotPermitted = 5,
    /// The selected pipeline was not admitted.
    PipelineNotPermitted = 6,
    /// Requested rights were not admitted.
    RightsNotPermitted = 7,
    /// The peer violated Protocol V1.
    ProtocolViolation = 8,
    /// A bounded internal operation failed closed.
    InternalFailure = 9,
}

impl DenyReasonV1 {
    fn from_wire(value: u16) -> Result<Self, ProtocolErrorV1> {
        match value {
            1 => Ok(Self::PolicyRejected),
            2 => Ok(Self::ExecutableIdentityMismatch),
            3 => Ok(Self::ArgumentVectorMismatch),
            4 => Ok(Self::CompilerClosureMismatch),
            5 => Ok(Self::TargetNotPermitted),
            6 => Ok(Self::PipelineNotPermitted),
            7 => Ok(Self::RightsNotPermitted),
            8 => Ok(Self::ProtocolViolation),
            9 => Ok(Self::InternalFailure),
            _ => Err(ProtocolErrorV1::UnknownDenyReason { actual: value }),
        }
    }
}

/// A required nonzero Protocol V1 identity field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtocolIdentityFieldV1 {
    /// Fresh challenge nonce.
    Nonce,
    /// Canonical Policy V1 identity.
    Policy,
    /// Protected launcher executable identity.
    LauncherExecutable,
    /// Protected `cargo-fe2o3` executable identity.
    CargoFe2o3Executable,
    /// Complete protected child argument-vector identity.
    ChildArgumentVector,
    /// Attestation commitment.
    Attestation,
    /// Fresh grant identifier.
    Grant,
    /// Admission identity.
    Admission,
}

impl fmt::Display for ProtocolIdentityFieldV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Nonce => "nonce",
            Self::Policy => "policy",
            Self::LauncherExecutable => "launcher executable",
            Self::CargoFe2o3Executable => "cargo-fe2o3 executable",
            Self::ChildArgumentVector => "child argument vector",
            Self::Attestation => "attestation",
            Self::Grant => "grant",
            Self::Admission => "admission",
        };
        formatter.write_str(name)
    }
}

/// Why an argument vector could not receive a canonical Protocol V1 identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ArgvIdentityErrorV1 {
    /// The complete vector did not contain fixed `argv[0]` and a command argument.
    InvalidArgumentCount {
        /// Observed complete argument count.
        actual: usize,
    },
    /// `argv[0]` was not the fixed protected executable path.
    InvalidArgv0,
    /// An argument was empty.
    EmptyArgument {
        /// Zero-based argument index.
        index: usize,
    },
    /// An argument contained an interior NUL byte.
    InteriorNul {
        /// Zero-based argument index.
        index: usize,
    },
    /// An argument exceeded the launcher limit.
    ArgumentTooLong {
        /// Zero-based argument index.
        index: usize,
        /// Observed byte length.
        actual: usize,
    },
    /// Forwarded arguments exceeded the launcher aggregate limit.
    ArgumentsTooLarge {
        /// Observed aggregate forwarded size.
        actual: usize,
    },
}

impl fmt::Display for ArgvIdentityErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgumentCount { actual } => {
                write!(
                    formatter,
                    "protected argv has invalid argument count {actual}"
                )
            }
            Self::InvalidArgv0 => formatter.write_str("protected argv[0] is not canonical"),
            Self::EmptyArgument { index } => write!(formatter, "argv[{index}] is empty"),
            Self::InteriorNul { index } => {
                write!(formatter, "argv[{index}] contains an interior NUL")
            }
            Self::ArgumentTooLong { index, actual } => {
                write!(formatter, "argv[{index}] is too long: {actual} bytes")
            }
            Self::ArgumentsTooLarge { actual } => {
                write!(formatter, "forwarded argv is too large: {actual} bytes")
            }
        }
    }
}

impl std::error::Error for ArgvIdentityErrorV1 {}

/// Why a Protocol V1 frame was not canonical.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtocolErrorV1 {
    /// Fewer than 24 header bytes were supplied.
    TruncatedHeader {
        /// Observed byte length.
        actual: usize,
    },
    /// Frame magic did not match Protocol V1.
    InvalidMagic,
    /// Frame version was not Protocol V1.
    UnsupportedVersion {
        /// Observed version.
        actual: u16,
    },
    /// The message type was not assigned.
    UnknownFrameType {
        /// Observed message type.
        actual: u16,
    },
    /// Header flags were nonzero.
    UnsupportedFlags {
        /// Observed flags.
        actual: u32,
    },
    /// The sequence was not canonical for the message type.
    InvalidSequence {
        /// Decoded message type.
        kind: FrameKindV1,
        /// Canonical sequence for the message type.
        expected: u32,
        /// Observed sequence.
        actual: u32,
    },
    /// The declared payload length was not canonical for the message type.
    InvalidPayloadLength {
        /// Decoded message type.
        kind: FrameKindV1,
        /// Canonical payload length for the message type.
        expected: usize,
        /// Declared payload length.
        actual: u32,
    },
    /// Bytes were missing from or appended to the declared frame.
    InvalidEncodedLength {
        /// Exact encoded length required by the header.
        expected: usize,
        /// Observed encoded length.
        actual: usize,
    },
    /// A required identity or nonce was all zero.
    ZeroIdentity {
        /// Rejected identity field.
        field: ProtocolIdentityFieldV1,
    },
    /// A required policy serial was zero.
    ZeroPolicySerial,
    /// Reserved payload bytes were nonzero.
    NonzeroReserved {
        /// Message carrying nonzero reserved bytes.
        kind: FrameKindV1,
    },
    /// The target ID was not assigned by Protocol V1.
    UnknownTarget {
        /// Observed target ID.
        actual: u16,
    },
    /// The profile was not the standalone foundation profile.
    ProfileNotPermitted {
        /// Observed profile ID.
        actual: u8,
    },
    /// The selected pipeline was not assigned by Policy V1.
    UnknownPipeline {
        /// Observed pipeline ID.
        actual: u16,
    },
    /// A foundation message requested publication rights.
    PublicationRightsNotPermitted {
        /// Observed publication-rights bits.
        actual: u32,
    },
    /// Compiler pins or their declared closure identity were invalid.
    InvalidCompilerClosure(CompilerClosureErrorV1),
    /// The Attest payload did not carry its canonical commitment.
    InvalidAttestationIdentity,
    /// The Grant payload did not carry its canonical admission identity.
    InvalidAdmissionIdentity,
    /// The denial reason was not assigned by Protocol V1.
    UnknownDenyReason {
        /// Observed denial-reason ID.
        actual: u16,
    },
}

impl fmt::Display for ProtocolErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader { actual } => {
                write!(
                    formatter,
                    "Protocol V1 header is truncated at {actual} bytes"
                )
            }
            Self::InvalidMagic => formatter.write_str("invalid Protocol V1 magic"),
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "unsupported Protocol V1 version {actual}")
            }
            Self::UnknownFrameType { actual } => {
                write!(formatter, "unknown Protocol V1 frame type {actual}")
            }
            Self::UnsupportedFlags { actual } => {
                write!(formatter, "unsupported Protocol V1 flags {actual:#x}")
            }
            Self::InvalidSequence {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "{kind:?} sequence is {actual}, expected {expected}"
            ),
            Self::InvalidPayloadLength {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "{kind:?} payload length is {actual}, expected {expected}"
            ),
            Self::InvalidEncodedLength { expected, actual } => write!(
                formatter,
                "Protocol V1 frame is {actual} bytes, expected {expected}"
            ),
            Self::ZeroIdentity { field } => write!(formatter, "{field} identity must be nonzero"),
            Self::ZeroPolicySerial => formatter.write_str("policy serial must be nonzero"),
            Self::NonzeroReserved { kind } => {
                write!(formatter, "{kind:?} reserved bytes must be zero")
            }
            Self::UnknownTarget { actual } => {
                write!(formatter, "unknown Protocol V1 target {actual}")
            }
            Self::ProfileNotPermitted { actual } => write!(
                formatter,
                "profile {actual} is not permitted by the Protocol V1 foundation"
            ),
            Self::UnknownPipeline { actual } => {
                write!(formatter, "unknown Protocol V1 pipeline {actual}")
            }
            Self::PublicationRightsNotPermitted { actual } => write!(
                formatter,
                "publication rights {actual:#x} are not permitted by the foundation profile"
            ),
            Self::InvalidCompilerClosure(error) => write!(formatter, "{error}"),
            Self::InvalidAttestationIdentity => {
                formatter.write_str("invalid canonical Attest V1 identity")
            }
            Self::InvalidAdmissionIdentity => {
                formatter.write_str("invalid canonical Grant V1 admission identity")
            }
            Self::UnknownDenyReason { actual } => {
                write!(formatter, "unknown Protocol V1 denial reason {actual}")
            }
        }
    }
}

impl std::error::Error for ProtocolErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidCompilerClosure(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompilerClosureErrorV1> for ProtocolErrorV1 {
    fn from(error: CompilerClosureErrorV1) -> Self {
        Self::InvalidCompilerClosure(error)
    }
}

/// Computes the canonical identity of the complete protected child argv.
///
/// The hash input is the domain, a little-endian `u32` argument count, then
/// each nonempty argument as `u32_le(length) || exact_bytes`. The complete
/// vector must begin with [`PROTECTED_AUTHORITY_ARGV0_V1`].
pub fn argv_identity_sha256_v1(argv: &[&[u8]]) -> Result<[u8; 32], ArgvIdentityErrorV1> {
    if !(2..=PROTOCOL_V1_MAX_ARGUMENTS).contains(&argv.len()) {
        return Err(ArgvIdentityErrorV1::InvalidArgumentCount { actual: argv.len() });
    }
    if argv[0] != PROTECTED_AUTHORITY_ARGV0_V1 {
        return Err(ArgvIdentityErrorV1::InvalidArgv0);
    }

    let mut forwarded_size = 0_usize;
    for (index, argument) in argv.iter().enumerate() {
        if argument.is_empty() {
            return Err(ArgvIdentityErrorV1::EmptyArgument { index });
        }
        if argument.contains(&0) {
            return Err(ArgvIdentityErrorV1::InteriorNul { index });
        }
        if argument.len() > PROTOCOL_V1_MAX_ARGUMENT_BYTES {
            return Err(ArgvIdentityErrorV1::ArgumentTooLong {
                index,
                actual: argument.len(),
            });
        }
        if index != 0 {
            forwarded_size = forwarded_size.saturating_add(argument.len() + 1);
        }
    }
    if forwarded_size > PROTOCOL_V1_MAX_TOTAL_ARGUMENT_BYTES {
        return Err(ArgvIdentityErrorV1::ArgumentsTooLarge {
            actual: forwarded_size,
        });
    }

    let mut digest = Sha256::new();
    digest.update(ARGUMENT_VECTOR_IDENTITY_DOMAIN_V1);
    digest.update((argv.len() as u32).to_le_bytes());
    for argument in argv {
        digest.update((argument.len() as u32).to_le_bytes());
        digest.update(argument);
    }
    Ok(digest.finalize().into())
}

/// Canonical launcher challenge at sequence zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChallengeV1 {
    nonce: [u8; 32],
    policy_identity: [u8; 32],
    launcher_executable_identity: [u8; 32],
    cargo_fe2o3_executable_identity: [u8; 32],
    child_argv_identity: [u8; 32],
    policy_serial: u64,
}

impl ChallengeV1 {
    /// Constructs a validated standalone challenge from explicit observations.
    pub fn new(
        nonce: [u8; 32],
        policy_identity: [u8; 32],
        launcher_executable_identity: [u8; 32],
        cargo_fe2o3_executable_identity: [u8; 32],
        child_argv_identity: [u8; 32],
        policy_serial: u64,
    ) -> Result<Self, ProtocolErrorV1> {
        validate_identity(nonce, ProtocolIdentityFieldV1::Nonce)?;
        validate_identity(policy_identity, ProtocolIdentityFieldV1::Policy)?;
        validate_identity(
            launcher_executable_identity,
            ProtocolIdentityFieldV1::LauncherExecutable,
        )?;
        validate_identity(
            cargo_fe2o3_executable_identity,
            ProtocolIdentityFieldV1::CargoFe2o3Executable,
        )?;
        validate_identity(
            child_argv_identity,
            ProtocolIdentityFieldV1::ChildArgumentVector,
        )?;
        if policy_serial == 0 {
            return Err(ProtocolErrorV1::ZeroPolicySerial);
        }
        Ok(Self {
            nonce,
            policy_identity,
            launcher_executable_identity,
            cargo_fe2o3_executable_identity,
            child_argv_identity,
            policy_serial,
        })
    }

    /// Constructs a challenge carrying the exact identities in a Policy V1 value.
    pub fn for_policy(nonce: [u8; 32], policy: PolicyV1) -> Result<Self, ProtocolErrorV1> {
        Self::new(
            nonce,
            policy.identity_sha256(),
            policy.launcher_executable_sha256(),
            policy.cargo_fe2o3_executable_sha256(),
            policy.child_argv_sha256(),
            policy.serial(),
        )
    }

    /// Returns the challenge nonce.
    pub const fn nonce(self) -> [u8; 32] {
        self.nonce
    }
    /// Returns the policy identity.
    pub const fn policy_identity(self) -> [u8; 32] {
        self.policy_identity
    }
    /// Returns the launcher executable identity.
    pub const fn launcher_executable_identity(self) -> [u8; 32] {
        self.launcher_executable_identity
    }
    /// Returns the `cargo-fe2o3` executable identity.
    pub const fn cargo_fe2o3_executable_identity(self) -> [u8; 32] {
        self.cargo_fe2o3_executable_identity
    }
    /// Returns the child argument-vector identity.
    pub const fn child_argv_identity(self) -> [u8; 32] {
        self.child_argv_identity
    }
    /// Returns the policy serial.
    pub const fn policy_serial(self) -> u64 {
        self.policy_serial
    }
    /// Returns the standalone foundation profile.
    pub const fn profile(self) -> AuthorityProfileV1 {
        AuthorityProfileV1::StandaloneFoundation
    }
}

/// Canonical Rust-side attestation at sequence one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttestV1 {
    nonce: [u8; 32],
    policy_identity: [u8; 32],
    launcher_executable_identity: [u8; 32],
    cargo_fe2o3_executable_identity: [u8; 32],
    child_argv_identity: [u8; 32],
    compiler_closure: CompilerClosureV1,
    pipeline: PipelineV1,
    attestation_identity: [u8; 32],
}

impl AttestV1 {
    /// Constructs a validated foundation attestation from explicit observations.
    pub fn new(
        nonce: [u8; 32],
        policy_identity: [u8; 32],
        launcher_executable_identity: [u8; 32],
        cargo_fe2o3_executable_identity: [u8; 32],
        child_argv_identity: [u8; 32],
        compiler_closure: CompilerClosureV1,
        pipeline: PipelineV1,
    ) -> Result<Self, ProtocolErrorV1> {
        validate_identity(nonce, ProtocolIdentityFieldV1::Nonce)?;
        validate_identity(policy_identity, ProtocolIdentityFieldV1::Policy)?;
        validate_identity(
            launcher_executable_identity,
            ProtocolIdentityFieldV1::LauncherExecutable,
        )?;
        validate_identity(
            cargo_fe2o3_executable_identity,
            ProtocolIdentityFieldV1::CargoFe2o3Executable,
        )?;
        validate_identity(
            child_argv_identity,
            ProtocolIdentityFieldV1::ChildArgumentVector,
        )?;
        let mut value = Self {
            nonce,
            policy_identity,
            launcher_executable_identity,
            cargo_fe2o3_executable_identity,
            child_argv_identity,
            compiler_closure,
            pipeline,
            attestation_identity: [0; 32],
        };
        let payload = value.encode_payload();
        value.attestation_identity = hash_payload_prefix(
            ATTESTATION_IDENTITY_DOMAIN_V1,
            &payload[..ATTEST_COMMITMENT_OFFSET],
        );
        Ok(value)
    }

    /// Constructs an attestation carrying the exact identities in a policy.
    pub fn for_policy(nonce: [u8; 32], policy: PolicyV1) -> Result<Self, ProtocolErrorV1> {
        Self::new(
            nonce,
            policy.identity_sha256(),
            policy.launcher_executable_sha256(),
            policy.cargo_fe2o3_executable_sha256(),
            policy.child_argv_sha256(),
            policy.compiler_closure(),
            policy.selected_pipeline(),
        )
    }

    /// Returns the challenge nonce.
    pub const fn nonce(self) -> [u8; 32] {
        self.nonce
    }
    /// Returns the policy identity.
    pub const fn policy_identity(self) -> [u8; 32] {
        self.policy_identity
    }
    /// Returns the independently observed launcher identity.
    pub const fn launcher_executable_identity(self) -> [u8; 32] {
        self.launcher_executable_identity
    }
    /// Returns the independently observed `cargo-fe2o3` identity.
    pub const fn cargo_fe2o3_executable_identity(self) -> [u8; 32] {
        self.cargo_fe2o3_executable_identity
    }
    /// Returns the independently observed argv identity.
    pub const fn child_argv_identity(self) -> [u8; 32] {
        self.child_argv_identity
    }
    /// Returns the compiler pins and aggregate identity.
    pub const fn compiler_closure(self) -> CompilerClosureV1 {
        self.compiler_closure
    }
    /// Returns the fixed target.
    pub const fn target(self) -> ProtocolTargetV1 {
        ProtocolTargetV1::Gfx942XnackMinus
    }
    /// Returns the selected pipeline.
    pub const fn pipeline(self) -> PipelineV1 {
        self.pipeline
    }
    /// Returns the standalone foundation profile.
    pub const fn profile(self) -> AuthorityProfileV1 {
        AuthorityProfileV1::StandaloneFoundation
    }
    /// Returns zero publication rights.
    pub const fn publication_rights(self) -> PublicationRightsV1 {
        PublicationRightsV1::NONE
    }
    /// Returns the canonical attestation commitment.
    pub const fn attestation_identity(self) -> [u8; 32] {
        self.attestation_identity
    }

    fn encode_payload(self) -> [u8; ATTEST_V1_PAYLOAD_LEN] {
        let mut payload = [0; ATTEST_V1_PAYLOAD_LEN];
        let mut cursor = 0;
        write_bytes(&mut payload, &mut cursor, &self.nonce);
        write_bytes(&mut payload, &mut cursor, &self.policy_identity);
        write_bytes(
            &mut payload,
            &mut cursor,
            &self.launcher_executable_identity,
        );
        write_bytes(
            &mut payload,
            &mut cursor,
            &self.cargo_fe2o3_executable_identity,
        );
        write_bytes(&mut payload, &mut cursor, &self.child_argv_identity);
        write_compiler_closure(&mut payload, &mut cursor, self.compiler_closure);
        write_bytes(
            &mut payload,
            &mut cursor,
            &(ProtocolTargetV1::Gfx942XnackMinus as u16).to_le_bytes(),
        );
        write_bytes(
            &mut payload,
            &mut cursor,
            &(self.pipeline as u16).to_le_bytes(),
        );
        payload[cursor] = AuthorityProfileV1::StandaloneFoundation as u8;
        cursor += PROFILE_LEN;
        write_bytes(
            &mut payload,
            &mut cursor,
            &PublicationRightsV1::NONE.bits().to_le_bytes(),
        );
        cursor += ATTEST_RESERVED_LEN;
        debug_assert_eq!(cursor, ATTEST_COMMITMENT_OFFSET);
        write_bytes(&mut payload, &mut cursor, &self.attestation_identity);
        debug_assert_eq!(cursor, ATTEST_V1_PAYLOAD_LEN);
        payload
    }
}

/// Canonical foundation admission at sequence two.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrantV1 {
    nonce: [u8; 32],
    policy_identity: [u8; 32],
    attestation_identity: [u8; 32],
    grant_id: [u8; 32],
    pipeline: PipelineV1,
    admission_identity: [u8; 32],
}

impl GrantV1 {
    /// Constructs a zero-rights foundation grant for one attestation.
    pub fn for_attestation(
        attestation: AttestV1,
        grant_id: [u8; 32],
    ) -> Result<Self, ProtocolErrorV1> {
        validate_identity(grant_id, ProtocolIdentityFieldV1::Grant)?;
        let mut value = Self {
            nonce: attestation.nonce,
            policy_identity: attestation.policy_identity,
            attestation_identity: attestation.attestation_identity,
            grant_id,
            pipeline: attestation.pipeline,
            admission_identity: [0; 32],
        };
        let payload = value.encode_payload();
        value.admission_identity = hash_payload_prefix(
            ADMISSION_IDENTITY_DOMAIN_V1,
            &payload[..GRANT_ADMISSION_OFFSET],
        );
        Ok(value)
    }

    /// Returns the challenge nonce.
    pub const fn nonce(self) -> [u8; 32] {
        self.nonce
    }
    /// Returns the policy identity.
    pub const fn policy_identity(self) -> [u8; 32] {
        self.policy_identity
    }
    /// Returns the attestation commitment.
    pub const fn attestation_identity(self) -> [u8; 32] {
        self.attestation_identity
    }
    /// Returns the fresh grant identifier.
    pub const fn grant_id(self) -> [u8; 32] {
        self.grant_id
    }
    /// Returns the fixed target.
    pub const fn target(self) -> ProtocolTargetV1 {
        ProtocolTargetV1::Gfx942XnackMinus
    }
    /// Returns the selected pipeline.
    pub const fn pipeline(self) -> PipelineV1 {
        self.pipeline
    }
    /// Returns the standalone foundation profile.
    pub const fn profile(self) -> AuthorityProfileV1 {
        AuthorityProfileV1::StandaloneFoundation
    }
    /// Returns zero publication rights.
    pub const fn publication_rights(self) -> PublicationRightsV1 {
        PublicationRightsV1::NONE
    }
    /// Returns the canonical admission identity.
    pub const fn admission_identity(self) -> [u8; 32] {
        self.admission_identity
    }

    fn encode_payload(self) -> [u8; GRANT_V1_PAYLOAD_LEN] {
        let mut payload = [0; GRANT_V1_PAYLOAD_LEN];
        let mut cursor = 0;
        write_bytes(&mut payload, &mut cursor, &self.nonce);
        write_bytes(&mut payload, &mut cursor, &self.policy_identity);
        write_bytes(&mut payload, &mut cursor, &self.attestation_identity);
        write_bytes(&mut payload, &mut cursor, &self.grant_id);
        write_bytes(
            &mut payload,
            &mut cursor,
            &(ProtocolTargetV1::Gfx942XnackMinus as u16).to_le_bytes(),
        );
        write_bytes(
            &mut payload,
            &mut cursor,
            &(self.pipeline as u16).to_le_bytes(),
        );
        payload[cursor] = AuthorityProfileV1::StandaloneFoundation as u8;
        cursor += PROFILE_LEN;
        write_bytes(
            &mut payload,
            &mut cursor,
            &PublicationRightsV1::NONE.bits().to_le_bytes(),
        );
        cursor += GRANT_RESERVED_LEN;
        debug_assert_eq!(cursor, GRANT_ADMISSION_OFFSET);
        write_bytes(&mut payload, &mut cursor, &self.admission_identity);
        debug_assert_eq!(cursor, GRANT_V1_PAYLOAD_LEN);
        payload
    }
}

/// Canonical typed denial at sequence two.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DenyV1 {
    nonce: [u8; 32],
    policy_identity: [u8; 32],
    attestation_identity: [u8; 32],
    reason: DenyReasonV1,
}

impl DenyV1 {
    /// Constructs a denial for one attestation.
    pub fn for_attestation(attestation: AttestV1, reason: DenyReasonV1) -> Self {
        Self {
            nonce: attestation.nonce,
            policy_identity: attestation.policy_identity,
            attestation_identity: attestation.attestation_identity,
            reason,
        }
    }
    /// Returns the challenge nonce.
    pub const fn nonce(self) -> [u8; 32] {
        self.nonce
    }
    /// Returns the policy identity.
    pub const fn policy_identity(self) -> [u8; 32] {
        self.policy_identity
    }
    /// Returns the attestation commitment.
    pub const fn attestation_identity(self) -> [u8; 32] {
        self.attestation_identity
    }
    /// Returns the bounded denial reason.
    pub const fn reason(self) -> DenyReasonV1 {
        self.reason
    }
}

/// Canonical Rust-side acceptance at sequence three.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptV1 {
    admission_identity: [u8; 32],
}

impl AcceptV1 {
    /// Constructs an acceptance for a validated grant.
    pub fn for_grant(grant: GrantV1) -> Self {
        Self {
            admission_identity: grant.admission_identity,
        }
    }
    /// Constructs an acceptance from an explicit nonzero admission identity.
    pub fn new(admission_identity: [u8; 32]) -> Result<Self, ProtocolErrorV1> {
        validate_identity(admission_identity, ProtocolIdentityFieldV1::Admission)?;
        Ok(Self { admission_identity })
    }
    /// Returns the accepted admission identity.
    pub const fn admission_identity(self) -> [u8; 32] {
        self.admission_identity
    }
}

/// One decoded, typed Protocol V1 frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolFrameV1 {
    /// Challenge at sequence zero.
    Challenge(ChallengeV1),
    /// Attestation at sequence one.
    Attest(AttestV1),
    /// Foundation grant at sequence two.
    Grant(GrantV1),
    /// Denial at sequence two.
    Deny(DenyV1),
    /// Acceptance at sequence three.
    Accept(AcceptV1),
}

/// A phase in the pure Protocol V1 transcript validator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolPhaseV1 {
    /// No challenge has been accepted.
    AwaitChallenge,
    /// A matching challenge was accepted; Attest is required next.
    AwaitAttest,
    /// A matching attestation was accepted; Grant or Deny is required next.
    AwaitDecision,
    /// A matching zero-rights foundation grant was accepted; Accept is required next.
    AwaitAccept,
    /// The matching admission identity was accepted.
    Complete,
    /// A matching typed denial terminated the transcript.
    Denied,
}

/// A field whose value did not remain continuous across a Protocol V1 transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TranscriptFieldV1 {
    /// Challenge nonce.
    Nonce,
    /// Policy content identity.
    PolicyIdentity,
    /// Launcher executable identity.
    LauncherExecutableIdentity,
    /// `cargo-fe2o3` executable identity.
    CargoFe2o3ExecutableIdentity,
    /// Complete child argument-vector identity.
    ChildArgumentVectorIdentity,
    /// Nonzero policy serial.
    PolicySerial,
    /// Compiler pins and canonical closure identity.
    CompilerClosure,
    /// Fixed target.
    Target,
    /// Selected pipeline.
    Pipeline,
    /// Standalone foundation profile.
    Profile,
    /// Zero foundation publication rights.
    PublicationRights,
    /// Attestation commitment.
    AttestationIdentity,
    /// Admission identity.
    AdmissionIdentity,
}

impl fmt::Display for TranscriptFieldV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Nonce => "nonce",
            Self::PolicyIdentity => "policy identity",
            Self::LauncherExecutableIdentity => "launcher executable identity",
            Self::CargoFe2o3ExecutableIdentity => "cargo-fe2o3 executable identity",
            Self::ChildArgumentVectorIdentity => "child argument-vector identity",
            Self::PolicySerial => "policy serial",
            Self::CompilerClosure => "compiler closure",
            Self::Target => "target",
            Self::Pipeline => "pipeline",
            Self::Profile => "profile",
            Self::PublicationRights => "publication rights",
            Self::AttestationIdentity => "attestation identity",
            Self::AdmissionIdentity => "admission identity",
        };
        formatter.write_str(name)
    }
}

/// Why the pure Protocol V1 transcript validator rejected a typed frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtocolStateErrorV1 {
    /// The frame type was not valid in the current phase.
    UnexpectedFrame {
        /// Phase before the rejected transition.
        phase: ProtocolPhaseV1,
        /// Rejected frame type.
        actual: FrameKindV1,
    },
    /// A frame was supplied after a terminal state.
    TerminalState {
        /// Terminal phase that rejected the frame.
        phase: ProtocolPhaseV1,
    },
    /// A transcript field did not match the policy or preceding frame.
    TranscriptMismatch {
        /// Field whose continuity check failed.
        field: TranscriptFieldV1,
    },
}

impl fmt::Display for ProtocolStateErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedFrame { phase, actual } => {
                write!(formatter, "unexpected {actual:?} frame in {phase:?}")
            }
            Self::TerminalState { phase } => {
                write!(formatter, "Protocol V1 transcript is already {phase:?}")
            }
            Self::TranscriptMismatch { field } => {
                write!(formatter, "Protocol V1 transcript {field} mismatch")
            }
        }
    }
}

impl std::error::Error for ProtocolStateErrorV1 {}

/// Pure, inert validation state for one Policy V1 transcript.
///
/// This state machine checks canonical message order and continuity only. It
/// does not authenticate a process, channel, peer, service, or fresh random
/// source, and completion does not confer publication authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolStateV1 {
    policy: PolicyV1,
    phase: ProtocolPhaseV1,
    challenge: Option<ChallengeV1>,
    attestation: Option<AttestV1>,
    grant: Option<GrantV1>,
    denial_reason: Option<DenyReasonV1>,
}

impl ProtocolStateV1 {
    /// Creates an inert transcript validator bound to one canonical policy.
    pub const fn new(policy: PolicyV1) -> Self {
        Self {
            policy,
            phase: ProtocolPhaseV1::AwaitChallenge,
            challenge: None,
            attestation: None,
            grant: None,
            denial_reason: None,
        }
    }

    /// Returns the current transcript phase.
    pub const fn phase(self) -> ProtocolPhaseV1 {
        self.phase
    }

    /// Returns the denial reason only after a matching Deny terminated the transcript.
    pub const fn denial_reason(self) -> Option<DenyReasonV1> {
        self.denial_reason
    }

    /// Returns the accepted admission identity only after a complete transcript.
    pub const fn accepted_admission_identity(self) -> Option<[u8; 32]> {
        if matches!(self.phase, ProtocolPhaseV1::Complete) {
            match self.grant {
                Some(grant) => Some(grant.admission_identity),
                None => None,
            }
        } else {
            None
        }
    }

    /// Validates and applies one typed frame without changing state on failure.
    pub fn advance(&mut self, frame: ProtocolFrameV1) -> Result<(), ProtocolStateErrorV1> {
        match self.phase {
            ProtocolPhaseV1::AwaitChallenge => {
                let ProtocolFrameV1::Challenge(challenge) = frame else {
                    return Err(self.unexpected(frame));
                };
                self.validate_challenge(challenge)?;
                self.challenge = Some(challenge);
                self.phase = ProtocolPhaseV1::AwaitAttest;
            }
            ProtocolPhaseV1::AwaitAttest => {
                let ProtocolFrameV1::Attest(attestation) = frame else {
                    return Err(self.unexpected(frame));
                };
                self.validate_attestation(attestation)?;
                self.attestation = Some(attestation);
                self.phase = ProtocolPhaseV1::AwaitDecision;
            }
            ProtocolPhaseV1::AwaitDecision => match frame {
                ProtocolFrameV1::Grant(grant) => {
                    self.validate_grant(grant)?;
                    self.grant = Some(grant);
                    self.phase = ProtocolPhaseV1::AwaitAccept;
                }
                ProtocolFrameV1::Deny(denial) => {
                    self.validate_denial(denial)?;
                    self.denial_reason = Some(denial.reason);
                    self.phase = ProtocolPhaseV1::Denied;
                }
                _ => return Err(self.unexpected(frame)),
            },
            ProtocolPhaseV1::AwaitAccept => {
                let ProtocolFrameV1::Accept(acceptance) = frame else {
                    return Err(self.unexpected(frame));
                };
                self.validate_acceptance(acceptance)?;
                self.phase = ProtocolPhaseV1::Complete;
            }
            ProtocolPhaseV1::Complete | ProtocolPhaseV1::Denied => {
                return Err(ProtocolStateErrorV1::TerminalState { phase: self.phase });
            }
        }
        Ok(())
    }

    fn unexpected(self, frame: ProtocolFrameV1) -> ProtocolStateErrorV1 {
        ProtocolStateErrorV1::UnexpectedFrame {
            phase: self.phase,
            actual: frame.kind(),
        }
    }

    fn validate_challenge(self, value: ChallengeV1) -> Result<(), ProtocolStateErrorV1> {
        ensure_transcript(
            value.policy_identity == self.policy.identity_sha256(),
            TranscriptFieldV1::PolicyIdentity,
        )?;
        ensure_transcript(
            value.launcher_executable_identity == self.policy.launcher_executable_sha256(),
            TranscriptFieldV1::LauncherExecutableIdentity,
        )?;
        ensure_transcript(
            value.cargo_fe2o3_executable_identity == self.policy.cargo_fe2o3_executable_sha256(),
            TranscriptFieldV1::CargoFe2o3ExecutableIdentity,
        )?;
        ensure_transcript(
            value.child_argv_identity == self.policy.child_argv_sha256(),
            TranscriptFieldV1::ChildArgumentVectorIdentity,
        )?;
        ensure_transcript(
            value.policy_serial == self.policy.serial(),
            TranscriptFieldV1::PolicySerial,
        )?;
        ensure_transcript(
            value.profile() == self.policy.profile(),
            TranscriptFieldV1::Profile,
        )
    }

    fn validate_attestation(self, value: AttestV1) -> Result<(), ProtocolStateErrorV1> {
        let challenge = self
            .challenge
            .expect("AwaitAttest state always retains its challenge");
        ensure_transcript(value.nonce == challenge.nonce, TranscriptFieldV1::Nonce)?;
        ensure_transcript(
            value.policy_identity == challenge.policy_identity,
            TranscriptFieldV1::PolicyIdentity,
        )?;
        ensure_transcript(
            value.launcher_executable_identity == challenge.launcher_executable_identity,
            TranscriptFieldV1::LauncherExecutableIdentity,
        )?;
        ensure_transcript(
            value.cargo_fe2o3_executable_identity == challenge.cargo_fe2o3_executable_identity,
            TranscriptFieldV1::CargoFe2o3ExecutableIdentity,
        )?;
        ensure_transcript(
            value.child_argv_identity == challenge.child_argv_identity,
            TranscriptFieldV1::ChildArgumentVectorIdentity,
        )?;
        ensure_transcript(
            value.compiler_closure == self.policy.compiler_closure(),
            TranscriptFieldV1::CompilerClosure,
        )?;
        ensure_transcript(
            value.target() == ProtocolTargetV1::Gfx942XnackMinus,
            TranscriptFieldV1::Target,
        )?;
        ensure_transcript(
            value.pipeline == self.policy.selected_pipeline(),
            TranscriptFieldV1::Pipeline,
        )?;
        ensure_transcript(
            value.profile() == self.policy.profile(),
            TranscriptFieldV1::Profile,
        )?;
        ensure_transcript(
            value.publication_rights() == self.policy.publication_rights(),
            TranscriptFieldV1::PublicationRights,
        )
    }

    fn validate_grant(self, value: GrantV1) -> Result<(), ProtocolStateErrorV1> {
        let attestation = self
            .attestation
            .expect("AwaitDecision state always retains its attestation");
        ensure_transcript(value.nonce == attestation.nonce, TranscriptFieldV1::Nonce)?;
        ensure_transcript(
            value.policy_identity == attestation.policy_identity,
            TranscriptFieldV1::PolicyIdentity,
        )?;
        ensure_transcript(
            value.attestation_identity == attestation.attestation_identity,
            TranscriptFieldV1::AttestationIdentity,
        )?;
        ensure_transcript(
            value.target() == attestation.target(),
            TranscriptFieldV1::Target,
        )?;
        ensure_transcript(
            value.pipeline == attestation.pipeline,
            TranscriptFieldV1::Pipeline,
        )?;
        ensure_transcript(
            value.profile() == attestation.profile(),
            TranscriptFieldV1::Profile,
        )?;
        ensure_transcript(
            value.publication_rights() == attestation.publication_rights(),
            TranscriptFieldV1::PublicationRights,
        )
    }

    fn validate_denial(self, value: DenyV1) -> Result<(), ProtocolStateErrorV1> {
        let attestation = self
            .attestation
            .expect("AwaitDecision state always retains its attestation");
        ensure_transcript(value.nonce == attestation.nonce, TranscriptFieldV1::Nonce)?;
        ensure_transcript(
            value.policy_identity == attestation.policy_identity,
            TranscriptFieldV1::PolicyIdentity,
        )?;
        ensure_transcript(
            value.attestation_identity == attestation.attestation_identity,
            TranscriptFieldV1::AttestationIdentity,
        )
    }

    fn validate_acceptance(self, value: AcceptV1) -> Result<(), ProtocolStateErrorV1> {
        let grant = self
            .grant
            .expect("AwaitAccept state always retains its grant");
        ensure_transcript(
            value.admission_identity == grant.admission_identity,
            TranscriptFieldV1::AdmissionIdentity,
        )
    }
}

fn ensure_transcript(
    condition: bool,
    field: TranscriptFieldV1,
) -> Result<(), ProtocolStateErrorV1> {
    if condition {
        Ok(())
    } else {
        Err(ProtocolStateErrorV1::TranscriptMismatch { field })
    }
}

impl ProtocolFrameV1 {
    /// Returns the assigned message type.
    pub const fn kind(self) -> FrameKindV1 {
        match self {
            Self::Challenge(_) => FrameKindV1::Challenge,
            Self::Attest(_) => FrameKindV1::Attest,
            Self::Grant(_) => FrameKindV1::Grant,
            Self::Deny(_) => FrameKindV1::Deny,
            Self::Accept(_) => FrameKindV1::Accept,
        }
    }

    /// Returns the exact encoded frame length.
    pub const fn encoded_len(self) -> usize {
        PROTOCOL_V1_HEADER_LEN + self.kind().payload_len()
    }

    /// Encodes this frame canonically.
    pub fn encode(self) -> Vec<u8> {
        encode_protocol_frame_v1(&self)
    }
}

/// Encodes one typed Protocol V1 frame with its exact 24-byte header.
pub fn encode_protocol_frame_v1(frame: &ProtocolFrameV1) -> Vec<u8> {
    let kind = frame.kind();
    let mut encoded = vec![0; PROTOCOL_V1_HEADER_LEN + kind.payload_len()];
    encoded[..8].copy_from_slice(&PROTOCOL_V1_MAGIC);
    encoded[8..10].copy_from_slice(&PROTOCOL_V1_VERSION.to_le_bytes());
    encoded[10..12].copy_from_slice(&(kind as u16).to_le_bytes());
    encoded[12..16].copy_from_slice(&(kind.payload_len() as u32).to_le_bytes());
    encoded[16..20].copy_from_slice(&kind.sequence().to_le_bytes());

    let payload = &mut encoded[PROTOCOL_V1_HEADER_LEN..];
    match frame {
        ProtocolFrameV1::Challenge(value) => encode_challenge_payload(*value, payload),
        ProtocolFrameV1::Attest(value) => payload.copy_from_slice(&value.encode_payload()),
        ProtocolFrameV1::Grant(value) => payload.copy_from_slice(&value.encode_payload()),
        ProtocolFrameV1::Deny(value) => encode_deny_payload(*value, payload),
        ProtocolFrameV1::Accept(value) => payload.copy_from_slice(&value.admission_identity),
    }
    encoded
}

/// Decodes one exact canonical Protocol V1 frame.
pub fn decode_protocol_frame_v1(encoded: &[u8]) -> Result<ProtocolFrameV1, ProtocolErrorV1> {
    if encoded.len() < PROTOCOL_V1_HEADER_LEN {
        return Err(ProtocolErrorV1::TruncatedHeader {
            actual: encoded.len(),
        });
    }
    if encoded[..8] != PROTOCOL_V1_MAGIC {
        return Err(ProtocolErrorV1::InvalidMagic);
    }
    let version = read_u16(encoded, 8);
    if version != PROTOCOL_V1_VERSION {
        return Err(ProtocolErrorV1::UnsupportedVersion { actual: version });
    }
    let kind = FrameKindV1::from_wire(read_u16(encoded, 10))?;
    let declared_payload_len = read_u32(encoded, 12);
    if declared_payload_len != kind.payload_len() as u32 {
        return Err(ProtocolErrorV1::InvalidPayloadLength {
            kind,
            expected: kind.payload_len(),
            actual: declared_payload_len,
        });
    }
    let sequence = read_u32(encoded, 16);
    if sequence != kind.sequence() {
        return Err(ProtocolErrorV1::InvalidSequence {
            kind,
            expected: kind.sequence(),
            actual: sequence,
        });
    }
    let flags = read_u32(encoded, 20);
    if flags != 0 {
        return Err(ProtocolErrorV1::UnsupportedFlags { actual: flags });
    }
    let expected_len = PROTOCOL_V1_HEADER_LEN + kind.payload_len();
    if encoded.len() != expected_len {
        return Err(ProtocolErrorV1::InvalidEncodedLength {
            expected: expected_len,
            actual: encoded.len(),
        });
    }
    let payload = &encoded[PROTOCOL_V1_HEADER_LEN..];
    match kind {
        FrameKindV1::Challenge => decode_challenge_payload(payload).map(ProtocolFrameV1::Challenge),
        FrameKindV1::Attest => decode_attest_payload(payload).map(ProtocolFrameV1::Attest),
        FrameKindV1::Grant => decode_grant_payload(payload).map(ProtocolFrameV1::Grant),
        FrameKindV1::Deny => decode_deny_payload(payload).map(ProtocolFrameV1::Deny),
        FrameKindV1::Accept => decode_accept_payload(payload).map(ProtocolFrameV1::Accept),
    }
}

fn encode_challenge_payload(value: ChallengeV1, payload: &mut [u8]) {
    let mut cursor = 0;
    write_bytes(payload, &mut cursor, &value.nonce);
    write_bytes(payload, &mut cursor, &value.policy_identity);
    write_bytes(payload, &mut cursor, &value.launcher_executable_identity);
    write_bytes(payload, &mut cursor, &value.cargo_fe2o3_executable_identity);
    write_bytes(payload, &mut cursor, &value.child_argv_identity);
    write_bytes(payload, &mut cursor, &value.policy_serial.to_le_bytes());
    payload[cursor] = AuthorityProfileV1::StandaloneFoundation as u8;
    cursor += PROFILE_LEN + CHALLENGE_RESERVED_LEN;
    debug_assert_eq!(cursor, CHALLENGE_V1_PAYLOAD_LEN);
}

fn decode_challenge_payload(payload: &[u8]) -> Result<ChallengeV1, ProtocolErrorV1> {
    let nonce = digest_at(payload, 0);
    let policy = digest_at(payload, 32);
    let launcher = digest_at(payload, 64);
    let cargo_fe2o3 = digest_at(payload, 96);
    let argv = digest_at(payload, 128);
    let serial = read_u64(payload, 160);
    validate_foundation_profile(payload[168])?;
    if payload[169..176] != [0; CHALLENGE_RESERVED_LEN] {
        return Err(ProtocolErrorV1::NonzeroReserved {
            kind: FrameKindV1::Challenge,
        });
    }
    ChallengeV1::new(nonce, policy, launcher, cargo_fe2o3, argv, serial)
}

fn decode_attest_payload(payload: &[u8]) -> Result<AttestV1, ProtocolErrorV1> {
    let nonce = digest_at(payload, 0);
    let policy = digest_at(payload, 32);
    let launcher = digest_at(payload, 64);
    let cargo_fe2o3 = digest_at(payload, 96);
    let argv = digest_at(payload, 128);
    let compiler = read_compiler_closure(payload, 160)?;
    ProtocolTargetV1::from_wire(read_u16(payload, 320))?;
    let pipeline_wire = read_u16(payload, 322);
    let pipeline = PipelineV1::from_wire(pipeline_wire).map_err(|error| match error {
        PolicyErrorV1::UnknownPipeline { value } => {
            ProtocolErrorV1::UnknownPipeline { actual: value }
        }
        _ => unreachable!("pipeline parser returns only UnknownPipeline"),
    })?;
    validate_foundation_profile(payload[324])?;
    validate_foundation_rights(read_u32(payload, 325))?;
    if payload[329..332] != [0; ATTEST_RESERVED_LEN] {
        return Err(ProtocolErrorV1::NonzeroReserved {
            kind: FrameKindV1::Attest,
        });
    }
    let declared_attestation = digest_at(payload, ATTEST_COMMITMENT_OFFSET);
    validate_identity(declared_attestation, ProtocolIdentityFieldV1::Attestation)?;
    let value = AttestV1::new(
        nonce,
        policy,
        launcher,
        cargo_fe2o3,
        argv,
        compiler,
        pipeline,
    )?;
    if value.attestation_identity != declared_attestation {
        return Err(ProtocolErrorV1::InvalidAttestationIdentity);
    }
    Ok(value)
}

fn decode_grant_payload(payload: &[u8]) -> Result<GrantV1, ProtocolErrorV1> {
    let nonce = digest_at(payload, 0);
    let policy = digest_at(payload, 32);
    let attestation = digest_at(payload, 64);
    let grant_id = digest_at(payload, 96);
    validate_identity(nonce, ProtocolIdentityFieldV1::Nonce)?;
    validate_identity(policy, ProtocolIdentityFieldV1::Policy)?;
    validate_identity(attestation, ProtocolIdentityFieldV1::Attestation)?;
    validate_identity(grant_id, ProtocolIdentityFieldV1::Grant)?;
    ProtocolTargetV1::from_wire(read_u16(payload, 128))?;
    let pipeline_wire = read_u16(payload, 130);
    let pipeline = PipelineV1::from_wire(pipeline_wire).map_err(|error| match error {
        PolicyErrorV1::UnknownPipeline { value } => {
            ProtocolErrorV1::UnknownPipeline { actual: value }
        }
        _ => unreachable!("pipeline parser returns only UnknownPipeline"),
    })?;
    validate_foundation_profile(payload[132])?;
    validate_foundation_rights(read_u32(payload, 133))?;
    if payload[137..140] != [0; GRANT_RESERVED_LEN] {
        return Err(ProtocolErrorV1::NonzeroReserved {
            kind: FrameKindV1::Grant,
        });
    }
    let declared_admission = digest_at(payload, GRANT_ADMISSION_OFFSET);
    validate_identity(declared_admission, ProtocolIdentityFieldV1::Admission)?;
    let expected = hash_payload_prefix(
        ADMISSION_IDENTITY_DOMAIN_V1,
        &payload[..GRANT_ADMISSION_OFFSET],
    );
    if expected != declared_admission {
        return Err(ProtocolErrorV1::InvalidAdmissionIdentity);
    }
    Ok(GrantV1 {
        nonce,
        policy_identity: policy,
        attestation_identity: attestation,
        grant_id,
        pipeline,
        admission_identity: declared_admission,
    })
}

fn encode_deny_payload(value: DenyV1, payload: &mut [u8]) {
    let mut cursor = 0;
    write_bytes(payload, &mut cursor, &value.nonce);
    write_bytes(payload, &mut cursor, &value.policy_identity);
    write_bytes(payload, &mut cursor, &value.attestation_identity);
    write_bytes(payload, &mut cursor, &(value.reason as u16).to_le_bytes());
    cursor += DENY_RESERVED_LEN;
    debug_assert_eq!(cursor, DENY_V1_PAYLOAD_LEN);
}

fn decode_deny_payload(payload: &[u8]) -> Result<DenyV1, ProtocolErrorV1> {
    let nonce = digest_at(payload, 0);
    let policy = digest_at(payload, 32);
    let attestation = digest_at(payload, 64);
    validate_identity(nonce, ProtocolIdentityFieldV1::Nonce)?;
    validate_identity(policy, ProtocolIdentityFieldV1::Policy)?;
    validate_identity(attestation, ProtocolIdentityFieldV1::Attestation)?;
    let reason = DenyReasonV1::from_wire(read_u16(payload, 96))?;
    if payload[98..100] != [0; DENY_RESERVED_LEN] {
        return Err(ProtocolErrorV1::NonzeroReserved {
            kind: FrameKindV1::Deny,
        });
    }
    Ok(DenyV1 {
        nonce,
        policy_identity: policy,
        attestation_identity: attestation,
        reason,
    })
}

fn decode_accept_payload(payload: &[u8]) -> Result<AcceptV1, ProtocolErrorV1> {
    AcceptV1::new(digest_at(payload, 0))
}

fn validate_identity(
    value: [u8; 32],
    field: ProtocolIdentityFieldV1,
) -> Result<(), ProtocolErrorV1> {
    if value == [0; 32] {
        return Err(ProtocolErrorV1::ZeroIdentity { field });
    }
    Ok(())
}

fn validate_foundation_profile(value: u8) -> Result<(), ProtocolErrorV1> {
    if value != AuthorityProfileV1::StandaloneFoundation as u8 {
        return Err(ProtocolErrorV1::ProfileNotPermitted { actual: value });
    }
    Ok(())
}

fn validate_foundation_rights(value: u32) -> Result<(), ProtocolErrorV1> {
    if value != PublicationRightsV1::NONE.bits() {
        return Err(ProtocolErrorV1::PublicationRightsNotPermitted { actual: value });
    }
    Ok(())
}

fn write_compiler_closure(payload: &mut [u8], cursor: &mut usize, value: CompilerClosureV1) {
    write_bytes(payload, cursor, &value.cargo_executable_sha256());
    write_bytes(payload, cursor, &value.rustc_executable_sha256());
    write_bytes(payload, cursor, &value.rustc_runtime_tree_sha256());
    write_bytes(payload, cursor, &value.codegen_backend_sha256());
    write_bytes(payload, cursor, &value.identity_sha256());
}

fn read_compiler_closure(
    payload: &[u8],
    offset: usize,
) -> Result<CompilerClosureV1, ProtocolErrorV1> {
    Ok(CompilerClosureV1::from_pins_and_identity(
        digest_at(payload, offset),
        digest_at(payload, offset + 32),
        digest_at(payload, offset + 64),
        digest_at(payload, offset + 96),
        digest_at(payload, offset + 128),
    )?)
}

fn hash_payload_prefix(domain: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((payload.len() as u32).to_le_bytes());
    digest.update(payload);
    digest.finalize().into()
}

fn write_bytes(output: &mut [u8], cursor: &mut usize, value: &[u8]) {
    let end = *cursor + value.len();
    output[*cursor..end].copy_from_slice(value);
    *cursor = end;
}

fn digest_at(input: &[u8], offset: usize) -> [u8; 32] {
    input[offset..offset + 32]
        .try_into()
        .expect("validated Protocol V1 payload bounds")
}

fn read_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        input[offset..offset + 2]
            .try_into()
            .expect("validated Protocol V1 payload bounds"),
    )
}

fn read_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        input[offset..offset + 4]
            .try_into()
            .expect("validated Protocol V1 payload bounds"),
    )
}

fn read_u64(input: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        input[offset..offset + 8]
            .try_into()
            .expect("validated Protocol V1 payload bounds"),
    )
}
