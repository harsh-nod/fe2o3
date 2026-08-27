use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str;

use sha2::{Digest, Sha256};

/// Authority Cargo Environment V1 wire magic.
pub const AUTHORITY_CARGO_ENVIRONMENT_MAGIC_V1: [u8; 8] = *b"F2AUENV1";
/// Authority Cargo Environment V1 wire version.
pub const AUTHORITY_CARGO_ENVIRONMENT_VERSION_V1: u16 = 1;
/// Exact Authority Cargo Environment V1 header length.
pub const AUTHORITY_CARGO_ENVIRONMENT_HEADER_LEN_V1: u16 = 64;
/// Exact number of variables in the Authority Cargo Environment V1 allowlist.
pub const AUTHORITY_CARGO_ENVIRONMENT_ENTRY_COUNT_V1: u16 = 9;
/// Maximum encoded Authority Cargo Environment V1 byte length.
pub const AUTHORITY_CARGO_ENVIRONMENT_MAX_WIRE_LEN_V1: usize = 3_072;
/// Maximum byte length of one canonical Authority Cargo Environment V1 path.
pub const AUTHORITY_CARGO_ENVIRONMENT_MAX_PATH_LEN_V1: usize = 255;
/// Maximum raw byte length of any Authority Cargo Environment V1 value.
pub const AUTHORITY_CARGO_ENVIRONMENT_MAX_RAW_VALUE_LEN_V1: usize = 1_024;
/// The only GPU target accepted by Authority Cargo Environment V1.
pub const AUTHORITY_CARGO_ENVIRONMENT_TARGET_V1: &str = "gfx942:xnack-";
/// Exact canonical Cargo mode arguments required by Authority Cargo Environment V1.
pub const AUTHORITY_CARGO_MODE_ARGV_V1: [&str; 2] = ["--offline", "--frozen"];
/// Domain for a canonical Authority Cargo Environment V1 identity.
pub const AUTHORITY_CARGO_ENVIRONMENT_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/AUTHORITY-CARGO-ENVIRONMENT/V1\0";

const MODE_OFFLINE: u32 = 1 << 0;
const MODE_FROZEN: u32 = 1 << 1;
const REQUIRED_MODE: u32 = MODE_OFFLINE | MODE_FROZEN;
const MAX_VARIABLE_NAME_LEN: usize = 64;
const ENTRY_HEADER_LEN: usize = 4;

/// One variable in the exact Authority Cargo Environment V1 allowlist.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum AuthorityCargoEnvironmentVariableV1 {
    /// Absolute lexical path to the provisioned Cargo home.
    CargoHome,
    /// Cargo's offline switch, fixed to `true`.
    CargoNetOffline,
    /// Absolute lexical path to an isolated Cargo target directory.
    CargoTargetDir,
    /// fe2o3 GPU target, fixed to `gfx942:xnack-`.
    Fe2o3Target,
    /// Absolute lexical home directory presented to Cargo.
    Home,
    /// Process language, fixed to `C.UTF-8`.
    Lang,
    /// Process locale, fixed to `C.UTF-8`.
    LcAll,
    /// Absolute lexical temporary directory presented to Cargo.
    Tmpdir,
    /// Process timezone, fixed to `UTC`.
    Tz,
}

impl AuthorityCargoEnvironmentVariableV1 {
    const ALL: [Self; AUTHORITY_CARGO_ENVIRONMENT_ENTRY_COUNT_V1 as usize] = [
        Self::CargoHome,
        Self::CargoNetOffline,
        Self::CargoTargetDir,
        Self::Fe2o3Target,
        Self::Home,
        Self::Lang,
        Self::LcAll,
        Self::Tmpdir,
        Self::Tz,
    ];

    /// Returns the canonical environment-variable name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::CargoHome => "CARGO_HOME",
            Self::CargoNetOffline => "CARGO_NET_OFFLINE",
            Self::CargoTargetDir => "CARGO_TARGET_DIR",
            Self::Fe2o3Target => "FE2O3_TARGET",
            Self::Home => "HOME",
            Self::Lang => "LANG",
            Self::LcAll => "LC_ALL",
            Self::Tmpdir => "TMPDIR",
            Self::Tz => "TZ",
        }
    }

    const fn is_path(self) -> bool {
        matches!(
            self,
            Self::CargoHome | Self::CargoTargetDir | Self::Home | Self::Tmpdir
        )
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|variable| variable.name() == name)
    }
}

impl fmt::Display for AuthorityCargoEnvironmentVariableV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// A prohibited ambient channel recognized before the unknown-variable check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ForbiddenCargoEnvironmentChannelV1 {
    /// Dynamic-loader state such as `LD_PRELOAD` or a `DYLD_*` variable.
    DynamicLoader,
    /// Rust flags, compiler wrappers, linkers, or other tool substitution.
    ToolOverride,
    /// rustup toolchain selection or distribution state.
    RustupSelection,
    /// Network, certificate, or proxy configuration.
    Network,
    /// Cargo registry, credential helper, Git, or SSH state.
    RegistryCredentialGitSsh,
    /// A variable name that appears to carry a secret.
    SecretLike,
}

impl fmt::Display for ForbiddenCargoEnvironmentChannelV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::DynamicLoader => "dynamic-loader",
            Self::ToolOverride => "tool-override",
            Self::RustupSelection => "rustup-selection",
            Self::Network => "network/proxy",
            Self::RegistryCredentialGitSsh => "registry/credential/Git/SSH",
            Self::SecretLike => "secret-like",
        };
        formatter.write_str(name)
    }
}

/// Why an Authority Cargo Environment V1 path was not lexically canonical.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AuthorityCargoEnvironmentPathErrorV1 {
    /// The path was empty.
    Empty,
    /// The path exceeded the fixed V1 byte bound.
    TooLong {
        /// Observed UTF-8 byte length.
        actual: usize,
    },
    /// The path was not absolute.
    Relative,
    /// The path contained an empty, `.` or `..` component or a trailing slash.
    NonCanonicalComponent,
    /// The path contained non-ASCII, whitespace, control, or backslash bytes.
    NonPortableByte,
}

impl fmt::Display for AuthorityCargoEnvironmentPathErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("path is empty"),
            Self::TooLong { actual } => write!(
                formatter,
                "path is {actual} bytes; maximum is {AUTHORITY_CARGO_ENVIRONMENT_MAX_PATH_LEN_V1}"
            ),
            Self::Relative => formatter.write_str("path is relative"),
            Self::NonCanonicalComponent => {
                formatter.write_str("path has a noncanonical lexical component")
            }
            Self::NonPortableByte => formatter.write_str("path has a nonportable byte"),
        }
    }
}

/// A strict, inert snapshot of the one Cargo environment admitted by V1.
///
/// This value performs no environment mutation or filesystem access. Its paths
/// are only lexically canonical, and its cache digest is only declared data;
/// neither property authenticates a filesystem object.
///
/// Lexical validation does not resolve symlinks, mounts, hard links, or other
/// aliases. The four paths may be equal, nested, or resolve to overlapping
/// objects. An authority integration must authenticate the retained filesystem
/// objects and enforce its required ownership, permissions, and separation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityCargoEnvironmentV1 {
    cargo_home: String,
    cargo_target_dir: String,
    home: String,
    tmpdir: String,
    provisioned_cargo_cache_sha256: [u8; 32],
}

impl AuthorityCargoEnvironmentV1 {
    /// Validates an unordered environment and derives its canonical sorted map.
    ///
    /// Names and values are accepted as bytes so non-UTF-8 inputs fail at this
    /// boundary instead of being silently omitted by a string-only caller.
    pub fn new<I, N, V>(
        entries: I,
        provisioned_cargo_cache_sha256: [u8; 32],
    ) -> Result<Self, AuthorityCargoEnvironmentErrorV1>
    where
        I: IntoIterator<Item = (N, V)>,
        N: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        if provisioned_cargo_cache_sha256 == [0; 32] {
            return Err(AuthorityCargoEnvironmentErrorV1::ZeroCargoCacheIdentity);
        }

        let mut values = BTreeMap::new();
        let mut names = BTreeSet::new();
        let mut count = 0_usize;
        for (raw_name, raw_value) in entries {
            count += 1;
            if count > usize::from(AUTHORITY_CARGO_ENVIRONMENT_ENTRY_COUNT_V1) {
                return Err(AuthorityCargoEnvironmentErrorV1::TooManyVariables { actual: count });
            }
            let name_bytes = raw_name.as_ref();
            let value_bytes = raw_value.as_ref();
            if name_bytes.len() > MAX_VARIABLE_NAME_LEN {
                return Err(AuthorityCargoEnvironmentErrorV1::VariableNameTooLong {
                    actual: name_bytes.len(),
                });
            }
            let name = str::from_utf8(name_bytes)
                .map_err(|_| AuthorityCargoEnvironmentErrorV1::NonUtf8VariableName)?;
            validate_variable_name(name)?;
            if value_bytes.len() > AUTHORITY_CARGO_ENVIRONMENT_MAX_RAW_VALUE_LEN_V1 {
                return Err(AuthorityCargoEnvironmentErrorV1::VariableValueTooLong {
                    name: name.to_owned(),
                    actual: value_bytes.len(),
                });
            }
            let value = str::from_utf8(value_bytes).map_err(|_| {
                AuthorityCargoEnvironmentErrorV1::NonUtf8VariableValue {
                    name: name.to_owned(),
                }
            })?;
            if !names.insert(name.to_owned()) {
                return Err(AuthorityCargoEnvironmentErrorV1::DuplicateVariable {
                    name: name.to_owned(),
                });
            }
            let variable = match AuthorityCargoEnvironmentVariableV1::from_name(name) {
                Some(variable) => variable,
                None => {
                    if let Some(channel) = forbidden_channel(name) {
                        return Err(AuthorityCargoEnvironmentErrorV1::ForbiddenVariable {
                            name: name.to_owned(),
                            channel,
                        });
                    }
                    return Err(AuthorityCargoEnvironmentErrorV1::UnknownVariable {
                        name: name.to_owned(),
                    });
                }
            };
            validate_value(variable, value)?;
            values.insert(variable, value.to_owned());
        }

        for variable in AuthorityCargoEnvironmentVariableV1::ALL {
            if !values.contains_key(&variable) {
                return Err(AuthorityCargoEnvironmentErrorV1::MissingVariable { variable });
            }
        }

        Ok(Self {
            cargo_home: values
                .remove(&AuthorityCargoEnvironmentVariableV1::CargoHome)
                .expect("all Authority Cargo Environment V1 variables were checked as present"),
            cargo_target_dir: values
                .remove(&AuthorityCargoEnvironmentVariableV1::CargoTargetDir)
                .expect("all Authority Cargo Environment V1 variables were checked as present"),
            home: values
                .remove(&AuthorityCargoEnvironmentVariableV1::Home)
                .expect("all Authority Cargo Environment V1 variables were checked as present"),
            tmpdir: values
                .remove(&AuthorityCargoEnvironmentVariableV1::Tmpdir)
                .expect("all Authority Cargo Environment V1 variables were checked as present"),
            provisioned_cargo_cache_sha256,
        })
    }

    /// Returns the exact canonical sorted environment map.
    ///
    /// A caller can use this same array for metadata, configuration probes, and
    /// the build. This crate does not clear or install a process environment.
    pub fn environment(&self) -> [(&'static str, &str); 9] {
        [
            ("CARGO_HOME", &self.cargo_home),
            ("CARGO_NET_OFFLINE", "true"),
            ("CARGO_TARGET_DIR", &self.cargo_target_dir),
            ("FE2O3_TARGET", AUTHORITY_CARGO_ENVIRONMENT_TARGET_V1),
            ("HOME", &self.home),
            ("LANG", "C.UTF-8"),
            ("LC_ALL", "C.UTF-8"),
            ("TMPDIR", &self.tmpdir),
            ("TZ", "UTC"),
        ]
    }

    /// Reports the mandatory Cargo offline mode.
    pub const fn offline(&self) -> bool {
        true
    }

    /// Reports the mandatory Cargo frozen mode.
    pub const fn frozen(&self) -> bool {
        true
    }

    /// Returns the exact Cargo mode arguments required for this environment.
    ///
    /// An authority caller must pass these arguments to Cargo in this order and
    /// bind the complete resulting Cargo argv into its protected argv identity.
    /// The encoded mode bits describe this requirement but do not enforce the
    /// arguments in another process.
    pub const fn cargo_mode_argv(&self) -> [&'static str; 2] {
        AUTHORITY_CARGO_MODE_ARGV_V1
    }

    /// Returns the separately provisioned, declared Cargo-cache identity.
    pub const fn provisioned_cargo_cache_sha256(&self) -> [u8; 32] {
        self.provisioned_cargo_cache_sha256
    }

    /// Encodes the cache identity, mode, and exact sorted environment map.
    pub fn encode(&self) -> Vec<u8> {
        encode_authority_cargo_environment_v1(self)
    }

    /// Computes the canonical identity of this Cargo environment.
    pub fn identity_sha256(&self) -> [u8; 32] {
        hash_environment(&self.encode())
    }
}

/// Why Authority Cargo Environment V1 construction or decoding failed.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AuthorityCargoEnvironmentErrorV1 {
    /// A variable name was not UTF-8.
    NonUtf8VariableName,
    /// A variable value was not UTF-8.
    NonUtf8VariableValue {
        /// Variable whose value was rejected.
        name: String,
    },
    /// A variable name exceeded the fixed input bound.
    VariableNameTooLong {
        /// Observed byte length.
        actual: usize,
    },
    /// A raw variable value exceeded the fixed input bound.
    VariableValueTooLong {
        /// Variable whose value was rejected.
        name: String,
        /// Observed byte length.
        actual: usize,
    },
    /// A variable name was not canonical uppercase ASCII environment syntax.
    NonCanonicalVariableName {
        /// Rejected variable name.
        name: String,
    },
    /// A variable occurred more than once.
    DuplicateVariable {
        /// Duplicated variable name.
        name: String,
    },
    /// More entries than the exact allowlist size were supplied.
    TooManyVariables {
        /// Number observed before rejection.
        actual: usize,
    },
    /// A recognized ambient attack channel was supplied.
    ForbiddenVariable {
        /// Rejected variable name.
        name: String,
        /// Class of prohibited ambient channel.
        channel: ForbiddenCargoEnvironmentChannelV1,
    },
    /// A canonical name was not in the exact allowlist.
    UnknownVariable {
        /// Rejected variable name.
        name: String,
    },
    /// One required allowlisted variable was absent.
    MissingVariable {
        /// Missing variable.
        variable: AuthorityCargoEnvironmentVariableV1,
    },
    /// A fixed-value variable did not have its one canonical value.
    InvalidFixedValue {
        /// Rejected variable.
        variable: AuthorityCargoEnvironmentVariableV1,
    },
    /// A path variable was not in the V1 lexical path grammar.
    InvalidPath {
        /// Rejected path variable.
        variable: AuthorityCargoEnvironmentVariableV1,
        /// Path validation failure.
        reason: AuthorityCargoEnvironmentPathErrorV1,
    },
    /// The separately provisioned Cargo-cache digest was all zero.
    ZeroCargoCacheIdentity,
    /// Encoded bytes were shorter than the fixed header or exceeded the bound.
    InvalidWireLength {
        /// Observed byte length.
        actual: usize,
    },
    /// Wire magic did not match V1.
    InvalidMagic,
    /// Wire version was not V1.
    UnsupportedVersion {
        /// Observed version.
        actual: u16,
    },
    /// Header length was not the V1 fixed length.
    InvalidHeaderLength {
        /// Observed header length.
        actual: u16,
    },
    /// Entry count was not the exact V1 count.
    InvalidEntryCount {
        /// Observed entry count.
        actual: u16,
    },
    /// A reserved header byte was nonzero.
    NonzeroHeaderReserved,
    /// Declared total length did not equal the received length.
    InvalidDeclaredLength {
        /// Declared byte length.
        actual: u32,
    },
    /// Offline and frozen mode bits were not exactly the V1 value.
    InvalidMode {
        /// Observed mode bits.
        actual: u32,
    },
    /// One entry header or value extended beyond the document.
    TruncatedEntry {
        /// Zero-based entry index.
        index: usize,
    },
    /// One wire entry had an invalid name length.
    InvalidWireNameLength {
        /// Zero-based entry index.
        index: usize,
        /// Observed byte length.
        actual: u16,
    },
    /// One wire entry value exceeded its fixed bound.
    InvalidWireValueLength {
        /// Zero-based entry index.
        index: usize,
        /// Observed byte length.
        actual: u16,
    },
    /// Wire entries were not in exact canonical name order.
    NonCanonicalEntryOrder {
        /// Zero-based entry index.
        index: usize,
    },
}

impl fmt::Display for AuthorityCargoEnvironmentErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUtf8VariableName => {
                formatter.write_str("environment variable name is not UTF-8")
            }
            Self::NonUtf8VariableValue { name } => {
                write!(formatter, "environment variable {name} is not UTF-8")
            }
            Self::VariableNameTooLong { actual } => {
                write!(formatter, "environment variable name is {actual} bytes")
            }
            Self::VariableValueTooLong { name, actual } => write!(
                formatter,
                "environment variable {name} value is {actual} bytes; maximum is {AUTHORITY_CARGO_ENVIRONMENT_MAX_RAW_VALUE_LEN_V1}"
            ),
            Self::NonCanonicalVariableName { name } => {
                write!(
                    formatter,
                    "environment variable name {name:?} is not canonical"
                )
            }
            Self::DuplicateVariable { name } => {
                write!(formatter, "duplicate environment variable {name}")
            }
            Self::TooManyVariables { actual } => {
                write!(formatter, "too many environment variables: {actual}")
            }
            Self::ForbiddenVariable { name, channel } => {
                write!(formatter, "forbidden {channel} environment variable {name}")
            }
            Self::UnknownVariable { name } => {
                write!(formatter, "unknown environment variable {name}")
            }
            Self::MissingVariable { variable } => {
                write!(formatter, "missing environment variable {variable}")
            }
            Self::InvalidFixedValue { variable } => {
                write!(formatter, "invalid fixed value for {variable}")
            }
            Self::InvalidPath { variable, reason } => {
                write!(formatter, "invalid {variable}: {reason}")
            }
            Self::ZeroCargoCacheIdentity => {
                formatter.write_str("provisioned Cargo-cache identity must be nonzero")
            }
            Self::InvalidWireLength { actual } => write!(
                formatter,
                "invalid Authority Cargo Environment V1 wire length {actual}"
            ),
            Self::InvalidMagic => {
                formatter.write_str("invalid Authority Cargo Environment V1 magic")
            }
            Self::UnsupportedVersion { actual } => write!(
                formatter,
                "unsupported Authority Cargo Environment version {actual}"
            ),
            Self::InvalidHeaderLength { actual } => write!(
                formatter,
                "invalid Authority Cargo Environment header length {actual}"
            ),
            Self::InvalidEntryCount { actual } => write!(
                formatter,
                "invalid Authority Cargo Environment entry count {actual}"
            ),
            Self::NonzeroHeaderReserved => formatter
                .write_str("Authority Cargo Environment reserved header bytes must be zero"),
            Self::InvalidDeclaredLength { actual } => write!(
                formatter,
                "invalid declared Authority Cargo Environment length {actual}"
            ),
            Self::InvalidMode { actual } => write!(
                formatter,
                "invalid Authority Cargo Environment mode {actual:#x}"
            ),
            Self::TruncatedEntry { index } => write!(
                formatter,
                "truncated Authority Cargo Environment entry {index}"
            ),
            Self::InvalidWireNameLength { index, actual } => {
                write!(formatter, "entry {index} has invalid name length {actual}")
            }
            Self::InvalidWireValueLength { index, actual } => {
                write!(formatter, "entry {index} has invalid value length {actual}")
            }
            Self::NonCanonicalEntryOrder { index } => {
                write!(formatter, "entry {index} is not in canonical name order")
            }
        }
    }
}

impl std::error::Error for AuthorityCargoEnvironmentErrorV1 {}

/// Encodes the exact sorted V1 environment map and mandatory invocation mode.
pub fn encode_authority_cargo_environment_v1(environment: &AuthorityCargoEnvironmentV1) -> Vec<u8> {
    let entries = environment.environment();
    let total_len = usize::from(AUTHORITY_CARGO_ENVIRONMENT_HEADER_LEN_V1)
        + entries
            .iter()
            .map(|(name, value)| ENTRY_HEADER_LEN + name.len() + value.len())
            .sum::<usize>();
    debug_assert!(total_len <= AUTHORITY_CARGO_ENVIRONMENT_MAX_WIRE_LEN_V1);

    let mut encoded = Vec::with_capacity(total_len);
    encoded.extend_from_slice(&AUTHORITY_CARGO_ENVIRONMENT_MAGIC_V1);
    encoded.extend_from_slice(&AUTHORITY_CARGO_ENVIRONMENT_VERSION_V1.to_le_bytes());
    encoded.extend_from_slice(&AUTHORITY_CARGO_ENVIRONMENT_HEADER_LEN_V1.to_le_bytes());
    encoded.extend_from_slice(&AUTHORITY_CARGO_ENVIRONMENT_ENTRY_COUNT_V1.to_le_bytes());
    encoded.extend_from_slice(&0_u16.to_le_bytes());
    encoded.extend_from_slice(&(total_len as u32).to_le_bytes());
    encoded.extend_from_slice(&REQUIRED_MODE.to_le_bytes());
    encoded.extend_from_slice(&[0; 8]);
    encoded.extend_from_slice(&environment.provisioned_cargo_cache_sha256);
    for (name, value) in entries {
        encoded.extend_from_slice(&(name.len() as u16).to_le_bytes());
        encoded.extend_from_slice(&(value.len() as u16).to_le_bytes());
        encoded.extend_from_slice(name.as_bytes());
        encoded.extend_from_slice(value.as_bytes());
    }
    debug_assert_eq!(encoded.len(), total_len);
    encoded
}

/// Decodes exact sorted wire bytes, including the provisioned cache identity.
pub fn decode_authority_cargo_environment_v1(
    encoded: &[u8],
) -> Result<AuthorityCargoEnvironmentV1, AuthorityCargoEnvironmentErrorV1> {
    let header_len = usize::from(AUTHORITY_CARGO_ENVIRONMENT_HEADER_LEN_V1);
    if !(header_len..=AUTHORITY_CARGO_ENVIRONMENT_MAX_WIRE_LEN_V1).contains(&encoded.len()) {
        return Err(AuthorityCargoEnvironmentErrorV1::InvalidWireLength {
            actual: encoded.len(),
        });
    }
    if encoded[..8] != AUTHORITY_CARGO_ENVIRONMENT_MAGIC_V1 {
        return Err(AuthorityCargoEnvironmentErrorV1::InvalidMagic);
    }
    let version = read_u16(encoded, 8);
    if version != AUTHORITY_CARGO_ENVIRONMENT_VERSION_V1 {
        return Err(AuthorityCargoEnvironmentErrorV1::UnsupportedVersion { actual: version });
    }
    let observed_header_len = read_u16(encoded, 10);
    if observed_header_len != AUTHORITY_CARGO_ENVIRONMENT_HEADER_LEN_V1 {
        return Err(AuthorityCargoEnvironmentErrorV1::InvalidHeaderLength {
            actual: observed_header_len,
        });
    }
    let entry_count = read_u16(encoded, 12);
    if entry_count != AUTHORITY_CARGO_ENVIRONMENT_ENTRY_COUNT_V1 {
        return Err(AuthorityCargoEnvironmentErrorV1::InvalidEntryCount {
            actual: entry_count,
        });
    }
    if encoded[14..16] != [0; 2] || encoded[24..32] != [0; 8] {
        return Err(AuthorityCargoEnvironmentErrorV1::NonzeroHeaderReserved);
    }
    let declared_len = read_u32(encoded, 16);
    if declared_len != encoded.len() as u32 {
        return Err(AuthorityCargoEnvironmentErrorV1::InvalidDeclaredLength {
            actual: declared_len,
        });
    }
    let mode = read_u32(encoded, 20);
    if mode != REQUIRED_MODE {
        return Err(AuthorityCargoEnvironmentErrorV1::InvalidMode { actual: mode });
    }
    let provisioned_cargo_cache_sha256 = encoded[32..64]
        .try_into()
        .expect("fixed Authority Cargo Environment V1 header bounds");

    let mut cursor = header_len;
    let mut entries = Vec::with_capacity(usize::from(entry_count));
    for (index, expected) in AuthorityCargoEnvironmentVariableV1::ALL
        .into_iter()
        .enumerate()
    {
        if encoded.len().saturating_sub(cursor) < ENTRY_HEADER_LEN {
            return Err(AuthorityCargoEnvironmentErrorV1::TruncatedEntry { index });
        }
        let name_len = read_u16(encoded, cursor);
        let value_len = read_u16(encoded, cursor + 2);
        if name_len == 0 || usize::from(name_len) > MAX_VARIABLE_NAME_LEN {
            return Err(AuthorityCargoEnvironmentErrorV1::InvalidWireNameLength {
                index,
                actual: name_len,
            });
        }
        if value_len == 0 || usize::from(value_len) > AUTHORITY_CARGO_ENVIRONMENT_MAX_PATH_LEN_V1 {
            return Err(AuthorityCargoEnvironmentErrorV1::InvalidWireValueLength {
                index,
                actual: value_len,
            });
        }
        cursor += ENTRY_HEADER_LEN;
        let entry_len = usize::from(name_len) + usize::from(value_len);
        if encoded.len().saturating_sub(cursor) < entry_len {
            return Err(AuthorityCargoEnvironmentErrorV1::TruncatedEntry { index });
        }
        let name_end = cursor + usize::from(name_len);
        let value_end = name_end + usize::from(value_len);
        let name = &encoded[cursor..name_end];
        let value = &encoded[name_end..value_end];
        if name != expected.name().as_bytes() {
            return Err(AuthorityCargoEnvironmentErrorV1::NonCanonicalEntryOrder { index });
        }
        entries.push((name, value));
        cursor = value_end;
    }
    if cursor != encoded.len() {
        return Err(AuthorityCargoEnvironmentErrorV1::InvalidDeclaredLength {
            actual: declared_len,
        });
    }
    AuthorityCargoEnvironmentV1::new(entries, provisioned_cargo_cache_sha256)
}

/// Validates wire bytes and computes the canonical Cargo-environment identity.
pub fn authority_cargo_environment_identity_sha256_v1(
    encoded: &[u8],
) -> Result<[u8; 32], AuthorityCargoEnvironmentErrorV1> {
    decode_authority_cargo_environment_v1(encoded)?;
    Ok(hash_environment(encoded))
}

fn validate_variable_name(name: &str) -> Result<(), AuthorityCargoEnvironmentErrorV1> {
    let canonical = !name.is_empty()
        && name.as_bytes()[0].is_ascii_uppercase()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_');
    if canonical {
        Ok(())
    } else {
        Err(AuthorityCargoEnvironmentErrorV1::NonCanonicalVariableName {
            name: name.to_owned(),
        })
    }
}

fn validate_value(
    variable: AuthorityCargoEnvironmentVariableV1,
    value: &str,
) -> Result<(), AuthorityCargoEnvironmentErrorV1> {
    if variable.is_path() {
        return validate_path(value)
            .map_err(|reason| AuthorityCargoEnvironmentErrorV1::InvalidPath { variable, reason });
    }
    let expected = match variable {
        AuthorityCargoEnvironmentVariableV1::CargoNetOffline => "true",
        AuthorityCargoEnvironmentVariableV1::Fe2o3Target => AUTHORITY_CARGO_ENVIRONMENT_TARGET_V1,
        AuthorityCargoEnvironmentVariableV1::Lang | AuthorityCargoEnvironmentVariableV1::LcAll => {
            "C.UTF-8"
        }
        AuthorityCargoEnvironmentVariableV1::Tz => "UTC",
        _ => unreachable!("path variables returned before fixed-value matching"),
    };
    if value == expected {
        Ok(())
    } else {
        Err(AuthorityCargoEnvironmentErrorV1::InvalidFixedValue { variable })
    }
}

fn validate_path(path: &str) -> Result<(), AuthorityCargoEnvironmentPathErrorV1> {
    if path.is_empty() {
        return Err(AuthorityCargoEnvironmentPathErrorV1::Empty);
    }
    if path.len() > AUTHORITY_CARGO_ENVIRONMENT_MAX_PATH_LEN_V1 {
        return Err(AuthorityCargoEnvironmentPathErrorV1::TooLong { actual: path.len() });
    }
    if !path.starts_with('/') {
        return Err(AuthorityCargoEnvironmentPathErrorV1::Relative);
    }
    if path
        .bytes()
        .any(|byte| !(b'!'..=b'~').contains(&byte) || byte == b'\\')
    {
        return Err(AuthorityCargoEnvironmentPathErrorV1::NonPortableByte);
    }
    if path != "/"
        && (path.ends_with('/')
            || path[1..]
                .split('/')
                .any(|component| component.is_empty() || component == "." || component == ".."))
    {
        return Err(AuthorityCargoEnvironmentPathErrorV1::NonCanonicalComponent);
    }
    Ok(())
}

fn forbidden_channel(name: &str) -> Option<ForbiddenCargoEnvironmentChannelV1> {
    if name.starts_with("LD_") || name.starts_with("DYLD_") {
        return Some(ForbiddenCargoEnvironmentChannelV1::DynamicLoader);
    }
    if matches!(
        name,
        "RUSTFLAGS"
            | "RUSTDOCFLAGS"
            | "RUSTC"
            | "RUSTDOC"
            | "RUSTC_WRAPPER"
            | "RUSTC_WORKSPACE_WRAPPER"
            | "RUSTC_BOOTSTRAP"
            | "CARGO_ENCODED_RUSTFLAGS"
            | "CARGO_INCREMENTAL"
            | "CARGO_MAKEFLAGS"
            | "CARGO"
            | "CC"
            | "CXX"
            | "AR"
            | "LINKER"
            | "LLVM_CONFIG"
            | "LIBRARY_PATH"
            | "CPATH"
            | "CPLUS_INCLUDE_PATH"
            | "OBJC_INCLUDE_PATH"
            | "COMPILER_PATH"
            | "GCC_EXEC_PREFIX"
            | "BINDGEN_EXTRA_CLANG_ARGS"
            | "CFLAGS"
            | "CXXFLAGS"
            | "CPPFLAGS"
            | "LDFLAGS"
            | "MAKEFLAGS"
            | "MFLAGS"
            | "PATH"
    ) || name.starts_with("CARGO_BUILD_")
        || name.starts_with("CARGO_TARGET_")
        || name.starts_with("CARGO_PROFILE_")
        || name == "PKG_CONFIG"
        || name.starts_with("PKG_CONFIG_")
        || name.starts_with("NIX_CFLAGS_")
        || name.starts_with("NIX_LDFLAGS")
        || name.starts_with("CMAKE_")
        || name.starts_with("FE2O3_")
    {
        return Some(ForbiddenCargoEnvironmentChannelV1::ToolOverride);
    }
    if name.starts_with("RUSTUP_") {
        return Some(ForbiddenCargoEnvironmentChannelV1::RustupSelection);
    }
    if name.contains("PROXY")
        || name.starts_with("CARGO_HTTP_")
        || name.starts_with("CARGO_NET_")
        || name.starts_with("CURL_")
        || name.starts_with("SSL_CERT_")
    {
        return Some(ForbiddenCargoEnvironmentChannelV1::Network);
    }
    if name.starts_with("CARGO_REGISTR")
        || name.starts_with("CARGO_CREDENTIAL")
        || name.starts_with("GIT_")
        || name.starts_with("SSH_")
        || matches!(
            name,
            "GIT" | "SSH" | "GIT_ASKPASS" | "BROWSER" | "PAGER" | "EDITOR" | "VISUAL"
        )
    {
        return Some(ForbiddenCargoEnvironmentChannelV1::RegistryCredentialGitSsh);
    }
    if [
        "SECRET",
        "TOKEN",
        "PASSWORD",
        "PASSWD",
        "CREDENTIAL",
        "API_KEY",
        "PRIVATE_KEY",
    ]
    .into_iter()
    .any(|marker| name.contains(marker))
    {
        return Some(ForbiddenCargoEnvironmentChannelV1::SecretLike);
    }
    None
}

fn hash_environment(encoded: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(AUTHORITY_CARGO_ENVIRONMENT_IDENTITY_DOMAIN_V1);
    digest.update((encoded.len() as u64).to_le_bytes());
    digest.update(encoded);
    digest.finalize().into()
}

fn read_u16(encoded: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        encoded[offset..offset + 2]
            .try_into()
            .expect("fixed Authority Cargo Environment V1 bounds"),
    )
}

fn read_u32(encoded: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        encoded[offset..offset + 4]
            .try_into()
            .expect("fixed Authority Cargo Environment V1 bounds"),
    )
}
