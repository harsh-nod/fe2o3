//! Checkout-independent metadata for selected rustc compilations.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read as _};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt};
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use crate::RustcCompileInvocationV2;

/// Domain separator for portable selected-rustc metadata V1.
pub const PORTABLE_SELECTED_METADATA_DOMAIN_V1: &[u8] =
    b"FE2O3/PORTABLE-SELECTED-RUSTC-METADATA/V1\0";

const PORTABLE_CODEGEN_IDENTITY_KEYS_V1: &[&str] = &[
    "code-model",
    "codegen-units",
    "debuginfo",
    "debug-assertions",
    "embed-bitcode",
    "force-frame-pointers",
    "instrument-coverage",
    "lto",
    "no-redzone",
    "opt-level",
    "panic",
    "relocation-model",
    "soft-float",
    "strip",
    "target-cpu",
    "target-feature",
];
const MAX_PORTABLE_MANIFEST_BYTES_V1: u64 = 1024 * 1024;

/// Stable Cargo package inputs to portable selected-rustc metadata V1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortablePackageIdentityV1 {
    package_name: String,
    package_version: String,
    manifest_sha256: [u8; 32],
}

impl PortablePackageIdentityV1 {
    /// Constructs an identity from an authenticated package name, version, and
    /// `Cargo.toml` SHA-256 digest.
    ///
    /// This constructor supports deterministic callers that already hold the
    /// manifest digest. Environment-backed callers should use
    /// [`capture_cargo_package_identity_v1`].
    pub fn new(
        package_name: impl Into<String>,
        package_version: impl Into<String>,
        manifest_sha256: [u8; 32],
    ) -> Result<Self, PortableMetadataErrorV1> {
        let package_name = package_name.into();
        if package_name.is_empty() {
            return Err(PortableMetadataErrorV1::EmptyPackageIdentityField {
                field: "package-name",
            });
        }
        let package_version = package_version.into();
        if package_version.is_empty() {
            return Err(PortableMetadataErrorV1::EmptyPackageIdentityField {
                field: "package-version",
            });
        }
        Ok(Self {
            package_name,
            package_version,
            manifest_sha256,
        })
    }

    /// Returns the Cargo package name.
    #[must_use]
    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    /// Returns the Cargo package version.
    #[must_use]
    pub fn package_version(&self) -> &str {
        &self.package_version
    }

    /// Returns the SHA-256 digest of the retained `Cargo.toml` file.
    #[must_use]
    pub fn manifest_sha256(&self) -> &[u8; 32] {
        &self.manifest_sha256
    }
}

/// An error while capturing or deriving portable selected-rustc metadata V1.
#[derive(Debug)]
#[non_exhaustive]
pub enum PortableMetadataErrorV1 {
    /// A required Cargo environment variable was absent.
    MissingCargoEnvironment {
        /// Environment variable name.
        name: &'static str,
    },
    /// A required Cargo environment variable was not valid UTF-8.
    NonUtf8CargoEnvironment {
        /// Environment variable name.
        name: &'static str,
    },
    /// A required Cargo environment variable was empty.
    EmptyCargoEnvironment {
        /// Environment variable name.
        name: &'static str,
    },
    /// An explicit package identity field was empty.
    EmptyPackageIdentityField {
        /// Canonical portable-metadata field name.
        field: &'static str,
    },
    /// The selected package manifest could not be securely opened.
    ManifestOpen {
        /// Manifest path.
        path: PathBuf,
        /// Operating-system error.
        source: io::Error,
    },
    /// Secure manifest admission is unavailable on this platform.
    ManifestAdmissionUnsupported {
        /// Manifest path.
        path: PathBuf,
    },
    /// The retained manifest descriptor could not be inspected.
    ManifestMetadata {
        /// Manifest path.
        path: PathBuf,
        /// Whether this was the inspection after reading the file.
        after_read: bool,
        /// Operating-system error.
        source: io::Error,
    },
    /// The manifest was not a regular file within the V1 size bound.
    ManifestNotAdmitted {
        /// Manifest path.
        path: PathBuf,
    },
    /// The manifest size cannot be represented by this host.
    ManifestSizeOverflow,
    /// The retained manifest descriptor could not be read.
    ManifestRead {
        /// Manifest path.
        path: PathBuf,
        /// Operating-system error.
        source: io::Error,
    },
    /// The retained manifest changed while it was being hashed.
    ManifestChanged,
    /// A selected identity-bearing rustc option was not valid UTF-8.
    NonUtf8RustcIdentityOption {
        /// Canonical rustc option name.
        option: &'static str,
        /// Index in the original rustc argument vector, including `argv[0]`.
        argument_index: usize,
    },
    /// A selected rustc option lost the value guaranteed by classification.
    MissingRustcIdentityOptionValue {
        /// Canonical rustc option name.
        option: &'static str,
        /// Index in the original rustc argument vector, including `argv[0]`.
        argument_index: usize,
    },
}

impl fmt::Display for PortableMetadataErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCargoEnvironment { name } => {
                write!(
                    formatter,
                    "portable metadata requires Cargo environment {name}"
                )
            }
            Self::NonUtf8CargoEnvironment { name } => write!(
                formatter,
                "portable metadata requires UTF-8 Cargo environment {name}"
            ),
            Self::EmptyCargoEnvironment { name } => write!(
                formatter,
                "portable metadata requires nonempty Cargo environment {name}"
            ),
            Self::EmptyPackageIdentityField { field } => {
                write!(formatter, "portable metadata requires nonempty {field}")
            }
            Self::ManifestOpen { path, source } => write!(
                formatter,
                "cannot securely open selected package manifest `{}`: {source}",
                path.display()
            ),
            Self::ManifestAdmissionUnsupported { path } => write!(
                formatter,
                "selected package manifest `{}` requires Unix O_NOFOLLOW admission",
                path.display()
            ),
            Self::ManifestMetadata {
                path,
                after_read,
                source,
            } => {
                let action = if *after_read { "re-inspect" } else { "inspect" };
                write!(
                    formatter,
                    "cannot {action} opened selected package manifest `{}`: {source}",
                    path.display()
                )
            }
            Self::ManifestNotAdmitted { path } => write!(
                formatter,
                "selected package manifest `{}` must be a regular file of at most {MAX_PORTABLE_MANIFEST_BYTES_V1} bytes",
                path.display()
            ),
            Self::ManifestSizeOverflow => {
                formatter.write_str("selected package manifest size does not fit this host")
            }
            Self::ManifestRead { path, source } => write!(
                formatter,
                "cannot read opened selected package manifest `{}`: {source}",
                path.display()
            ),
            Self::ManifestChanged => formatter
                .write_str("selected package manifest changed while deriving portable metadata"),
            Self::NonUtf8RustcIdentityOption {
                option,
                argument_index,
            } => write!(
                formatter,
                "portable metadata requires UTF-8 value for rustc option `{option}` at argument {argument_index}"
            ),
            Self::MissingRustcIdentityOptionValue {
                option,
                argument_index,
            } => write!(
                formatter,
                "rustc option `{option}` at argument {argument_index} lost its validated value"
            ),
        }
    }
}

impl Error for PortableMetadataErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ManifestOpen { source, .. }
            | Self::ManifestMetadata { source, .. }
            | Self::ManifestRead { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Captures the current Cargo package identity through a retained, securely
/// opened `Cargo.toml` descriptor.
///
/// This function requires `CARGO_PKG_NAME`, `CARGO_PKG_VERSION`, and
/// `CARGO_MANIFEST_DIR`. It deliberately does not require
/// `CARGO_PRIMARY_PACKAGE`, so wrappers can bind managed dependency units.
pub fn capture_cargo_package_identity_v1()
-> Result<PortablePackageIdentityV1, PortableMetadataErrorV1> {
    let package_name = required_utf8_cargo_environment_v1("CARGO_PKG_NAME")?;
    let package_version = required_utf8_cargo_environment_v1("CARGO_PKG_VERSION")?;
    let manifest_dir = required_utf8_cargo_environment_v1("CARGO_MANIFEST_DIR")?;
    let manifest_path = Path::new(&manifest_dir).join("Cargo.toml");
    let manifest_file = open_portable_manifest_v1(&manifest_path)?;
    let manifest_sha256 = hash_open_portable_manifest_v1(manifest_file, &manifest_path)?;
    PortablePackageIdentityV1::new(package_name, package_version, manifest_sha256)
}

/// Derives the portable synthetic `-C metadata` token for one validated rustc
/// compile invocation.
///
/// Only identity-bearing options are decoded. Non-UTF-8 source, output,
/// dependency, and other path-bearing arguments therefore do not affect or
/// prevent derivation. Configuration and crate-type values are sorted and
/// deduplicated; the remaining admitted semantic options preserve rustc order.
///
/// This token is a selected-crate binding input, not a general Cargo crate
/// disambiguator. Artifact-producing wrappers must leave Cargo's original
/// `-Cmetadata` arguments unchanged; terminal extraction may replace them only
/// because it stops before producing a linkable artifact.
pub fn portable_rustc_metadata_v1(
    compile: RustcCompileInvocationV2<'_>,
    package_identity: &PortablePackageIdentityV1,
) -> Result<String, PortableMetadataErrorV1> {
    let argv = compile.argv();
    let mut cfgs = Vec::new();
    let mut crate_types = Vec::new();
    let mut identity_fields = Vec::new();
    let mut index = 1;
    while index < argv.len() {
        let argument = &argv[index];
        let bytes = argument.as_encoded_bytes();
        if bytes == b"--" {
            break;
        }
        if let Some(key) = separate_identity_option_v1(bytes) {
            let value_index = index + 1;
            let value = argv.get(value_index).ok_or(
                PortableMetadataErrorV1::MissingRustcIdentityOptionValue {
                    option: key,
                    argument_index: index,
                },
            )?;
            let value = identity_option_utf8_v1(value, key, value_index)?;
            record_portable_rustc_option_v1(
                key,
                value,
                &mut cfgs,
                &mut crate_types,
                &mut identity_fields,
            );
            index += 2;
            continue;
        }
        if bytes == b"-C" || bytes == b"--codegen" {
            let value_index = index + 1;
            let value = argv.get(value_index).ok_or(
                PortableMetadataErrorV1::MissingRustcIdentityOptionValue {
                    option: "codegen",
                    argument_index: index,
                },
            )?;
            record_portable_codegen_os_option_v1(value, value_index, &mut identity_fields)?;
            index += 2;
            continue;
        }
        for (prefix, key) in [
            (b"--cfg=".as_slice(), "cfg"),
            (b"--crate-type=".as_slice(), "crate-type"),
            (b"--edition=".as_slice(), "edition"),
            (b"--target=".as_slice(), "target"),
        ] {
            if let Some(value) = bytes.strip_prefix(prefix) {
                let value = encoded_utf8_v1(value, key, index)?;
                record_portable_rustc_option_v1(
                    key,
                    value,
                    &mut cfgs,
                    &mut crate_types,
                    &mut identity_fields,
                );
            }
        }
        if let Some(value) = bytes.strip_prefix(b"-C") {
            record_portable_codegen_bytes_v1(value, index, &mut identity_fields)?;
        } else if let Some(value) = bytes.strip_prefix(b"--codegen=") {
            record_portable_codegen_bytes_v1(value, index, &mut identity_fields)?;
        }
        index += 1;
    }

    cfgs.sort_unstable();
    cfgs.dedup();
    crate_types.sort_unstable();
    crate_types.dedup();

    let mut digest = Sha256::new();
    digest.update(PORTABLE_SELECTED_METADATA_DOMAIN_V1);
    hash_portable_metadata_field_v1(&mut digest, "package-name", &package_identity.package_name);
    hash_portable_metadata_field_v1(
        &mut digest,
        "package-version",
        &package_identity.package_version,
    );
    hash_portable_metadata_field_v1(
        &mut digest,
        "manifest-sha256",
        &lower_hex_v1(&package_identity.manifest_sha256),
    );
    hash_portable_metadata_field_v1(&mut digest, "crate-name", compile.crate_name());
    for cfg in cfgs {
        hash_portable_metadata_field_v1(&mut digest, "cfg", cfg);
    }
    for crate_type in crate_types {
        hash_portable_metadata_field_v1(&mut digest, "crate-type", crate_type);
    }
    for (key, value) in identity_fields {
        hash_portable_metadata_field_v1(&mut digest, key, value);
    }
    Ok(lower_hex_v1(digest.finalize().as_slice()))
}

fn required_utf8_cargo_environment_v1(
    name: &'static str,
) -> Result<String, PortableMetadataErrorV1> {
    let value = env::var_os(name)
        .ok_or(PortableMetadataErrorV1::MissingCargoEnvironment { name })?
        .into_string()
        .map_err(|_| PortableMetadataErrorV1::NonUtf8CargoEnvironment { name })?;
    if value.is_empty() {
        return Err(PortableMetadataErrorV1::EmptyCargoEnvironment { name });
    }
    Ok(value)
}

#[cfg(unix)]
fn open_portable_manifest_v1(path: &Path) -> Result<File, PortableMetadataErrorV1> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    options
        .open(path)
        .map_err(|source| PortableMetadataErrorV1::ManifestOpen {
            path: path.to_owned(),
            source,
        })
}

#[cfg(not(unix))]
fn open_portable_manifest_v1(path: &Path) -> Result<File, PortableMetadataErrorV1> {
    Err(PortableMetadataErrorV1::ManifestAdmissionUnsupported {
        path: path.to_owned(),
    })
}

fn hash_open_portable_manifest_v1(
    mut file: File,
    path: &Path,
) -> Result<[u8; 32], PortableMetadataErrorV1> {
    let initial = file
        .metadata()
        .map_err(|source| PortableMetadataErrorV1::ManifestMetadata {
            path: path.to_owned(),
            after_read: false,
            source,
        })?;
    if !initial.is_file() || initial.len() > MAX_PORTABLE_MANIFEST_BYTES_V1 {
        return Err(PortableMetadataErrorV1::ManifestNotAdmitted {
            path: path.to_owned(),
        });
    }
    let capacity = usize::try_from(initial.len())
        .map_err(|_| PortableMetadataErrorV1::ManifestSizeOverflow)?;
    let mut manifest = Vec::with_capacity(capacity);
    (&mut file)
        .take(MAX_PORTABLE_MANIFEST_BYTES_V1 + 1)
        .read_to_end(&mut manifest)
        .map_err(|source| PortableMetadataErrorV1::ManifestRead {
            path: path.to_owned(),
            source,
        })?;
    let final_metadata =
        file.metadata()
            .map_err(|source| PortableMetadataErrorV1::ManifestMetadata {
                path: path.to_owned(),
                after_read: true,
                source,
            })?;
    if manifest.len() as u64 != initial.len()
        || portable_manifest_metadata_identity_v1(&initial)
            != portable_manifest_metadata_identity_v1(&final_metadata)
    {
        return Err(PortableMetadataErrorV1::ManifestChanged);
    }
    Ok(Sha256::digest(manifest).into())
}

#[cfg(unix)]
fn portable_manifest_metadata_identity_v1(
    metadata: &std::fs::Metadata,
) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

#[cfg(not(unix))]
fn portable_manifest_metadata_identity_v1(
    metadata: &std::fs::Metadata,
) -> (u64, u64, u64, i64, i64, i64, i64) {
    (0, 0, metadata.len(), 0, 0, 0, 0)
}

fn separate_identity_option_v1(argument: &[u8]) -> Option<&'static str> {
    match argument {
        b"--cfg" => Some("cfg"),
        b"--crate-type" => Some("crate-type"),
        b"--edition" => Some("edition"),
        b"--target" => Some("target"),
        _ => None,
    }
}

fn identity_option_utf8_v1<'a>(
    value: &'a OsStr,
    option: &'static str,
    argument_index: usize,
) -> Result<&'a str, PortableMetadataErrorV1> {
    value
        .to_str()
        .ok_or(PortableMetadataErrorV1::NonUtf8RustcIdentityOption {
            option,
            argument_index,
        })
}

fn encoded_utf8_v1<'a>(
    value: &'a [u8],
    option: &'static str,
    argument_index: usize,
) -> Result<&'a str, PortableMetadataErrorV1> {
    std::str::from_utf8(value).map_err(|_| PortableMetadataErrorV1::NonUtf8RustcIdentityOption {
        option,
        argument_index,
    })
}

fn record_portable_rustc_option_v1<'a>(
    key: &'static str,
    value: &'a str,
    cfgs: &mut Vec<&'a str>,
    crate_types: &mut Vec<&'a str>,
    identity_fields: &mut Vec<(&'static str, &'a str)>,
) {
    match key {
        "cfg" => cfgs.push(value),
        "crate-type" => crate_types.push(value),
        "edition" | "target" => identity_fields.push((key, value)),
        _ => unreachable!("portable rustc option table is closed"),
    }
}

fn record_portable_codegen_os_option_v1<'a>(
    value: &'a OsStr,
    argument_index: usize,
    identity_fields: &mut Vec<(&'static str, &'a str)>,
) -> Result<(), PortableMetadataErrorV1> {
    record_portable_codegen_bytes_v1(value.as_encoded_bytes(), argument_index, identity_fields)
}

fn record_portable_codegen_bytes_v1<'a>(
    value: &'a [u8],
    argument_index: usize,
    identity_fields: &mut Vec<(&'static str, &'a str)>,
) -> Result<(), PortableMetadataErrorV1> {
    let Some(separator) = value.iter().position(|byte| *byte == b'=') else {
        return Ok(());
    };
    let key = &value[..separator];
    let Some(canonical_key) = PORTABLE_CODEGEN_IDENTITY_KEYS_V1
        .iter()
        .copied()
        .find(|candidate| candidate.as_bytes() == key)
    else {
        return Ok(());
    };
    let option_value = encoded_utf8_v1(&value[separator + 1..], canonical_key, argument_index)?;
    identity_fields.push((canonical_key, option_value));
    Ok(())
}

fn hash_portable_metadata_field_v1(digest: &mut Sha256, key: &str, value: &str) {
    digest.update((key.len() as u64).to_le_bytes());
    digest.update(key.as_bytes());
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value.as_bytes());
}

fn lower_hex_v1(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;
    use crate::{RustcInvocationV2, classify_rustc_invocation_v2};

    fn package_identity(version: &str, manifest_byte: u8) -> PortablePackageIdentityV1 {
        PortablePackageIdentityV1::new("package", version, [manifest_byte; 32]).unwrap()
    }

    fn portable(argv: Vec<OsString>, identity: &PortablePackageIdentityV1) -> String {
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap()
        else {
            panic!("fixture must classify as a compile invocation");
        };
        portable_rustc_metadata_v1(compile, identity).unwrap()
    }

    fn portable_args(args: &[&str]) -> String {
        let argv = ["rustc", "--crate-name", "unit", "unit.rs"]
            .into_iter()
            .chain(args.iter().copied())
            .map(OsString::from)
            .collect();
        portable(argv, &package_identity("1.0.0", 1))
    }

    fn semantic_argv(root: &str, features: &[&str], target_cpu: &str) -> Vec<OsString> {
        let mut argv = vec![
            OsString::from(format!("{root}/rustc")),
            OsString::from("--crate-name"),
            OsString::from("unit"),
            OsString::from(format!("{root}/src/lib.rs")),
            OsString::from("--target=amdgcn-amd-amdhsa"),
            OsString::from("--crate-type=lib"),
            OsString::from("--edition=2024"),
        ];
        for feature in features {
            argv.push(OsString::from("--cfg"));
            argv.push(OsString::from(format!("feature=\"{feature}\"")));
        }
        argv.extend([
            OsString::from(format!("-Ctarget-cpu={target_cpu}")),
            OsString::from("-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack"),
            OsString::from("-Copt-level=3"),
            OsString::from(format!("-Cmetadata={root}-cargo-salt")),
            OsString::from(format!("-Cextra-filename=-{root}-cargo-salt")),
            OsString::from("--out-dir"),
            OsString::from(format!("{root}/target/out")),
            OsString::from("--extern"),
            OsString::from(format!("dep={root}/target/libdep.rmeta")),
        ]);
        argv
    }

    #[test]
    fn explicit_identity_rejects_empty_name_and_version() {
        assert!(PortablePackageIdentityV1::new("", "1.0.0", [0; 32]).is_err());
        assert!(PortablePackageIdentityV1::new("package", "", [0; 32]).is_err());
    }

    #[test]
    fn portable_metadata_ignores_checkout_paths_and_cargo_salts() {
        let identity = package_identity("1.0.0", 1);
        let first = portable(
            semantic_argv(
                "/checkout/one",
                &["kernel", "auxiliary", "kernel"],
                "gfx942",
            ),
            &identity,
        );
        let relocated = portable(
            semantic_argv("/different/root", &["auxiliary", "kernel"], "gfx942"),
            &identity,
        );
        assert_eq!(first, relocated);
        assert_ne!(
            first,
            portable(
                semantic_argv("/checkout/one", &["kernel", "auxiliary"], "gfx950"),
                &identity,
            )
        );
    }

    #[test]
    fn split_and_joined_identity_options_are_equivalent() {
        let joined = portable_args(&[
            "--cfg=feature=\"kernel\"",
            "--crate-type=lib",
            "--edition=2024",
            "--target=amdgcn-amd-amdhsa",
            "-Copt-level=3",
            "--codegen=target-cpu=gfx942",
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64",
        ]);
        let split = portable_args(&[
            "--cfg",
            "feature=\"kernel\"",
            "--crate-type",
            "lib",
            "--edition",
            "2024",
            "--target",
            "amdgcn-amd-amdhsa",
            "-C",
            "opt-level=3",
            "--codegen",
            "target-cpu=gfx942",
            "-C",
            "target-feature=-wavefrontsize32,+wavefrontsize64",
        ]);
        assert_eq!(joined, split);
    }

    #[test]
    fn cfgs_and_crate_types_are_sorted_and_deduplicated() {
        let repeated = portable_args(&[
            "--cfg=feature=\"zeta\"",
            "--crate-type=rlib",
            "--cfg=feature=\"alpha\"",
            "--crate-type=lib",
            "--cfg=feature=\"zeta\"",
            "--crate-type=rlib",
        ]);
        let canonical = portable_args(&[
            "--cfg=feature=\"alpha\"",
            "--cfg=feature=\"zeta\"",
            "--crate-type=lib",
            "--crate-type=rlib",
        ]);
        assert_eq!(repeated, canonical);
    }

    #[test]
    fn portable_metadata_v1_golden_vector_is_frozen() {
        let metadata = portable_args(&[
            "--target=amdgcn-amd-amdhsa",
            "--crate-type=lib",
            "--edition=2024",
            "--cfg=feature=\"kernel\"",
            "-Ctarget-cpu=gfx942",
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack",
            "-Copt-level=3",
            "-Cmetadata=cargo-salt",
            "-Cextra-filename=-cargo-salt",
        ]);
        assert_eq!(
            metadata,
            "dd3e0082f3ac34a728c000c43c98bc8321362f6712224fb06fbefdd5d670324c"
        );
    }

    #[test]
    fn target_and_codegen_dimensions_separate_portable_metadata() {
        let baseline = portable_args(&[
            "--target=amdgcn-amd-amdhsa",
            "-Copt-level=3",
            "-Ctarget-cpu=gfx942",
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64",
        ]);
        assert_ne!(
            baseline,
            portable_args(&[
                "--target=x86_64-unknown-linux-gnu",
                "-Copt-level=3",
                "-Ctarget-cpu=gfx942",
                "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64",
            ])
        );
        assert_ne!(
            baseline,
            portable_args(&[
                "--target=amdgcn-amd-amdhsa",
                "-Copt-level=2",
                "-Ctarget-cpu=gfx942",
                "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64",
            ])
        );
        assert_ne!(
            baseline,
            portable_args(&[
                "--target=amdgcn-amd-amdhsa",
                "-Copt-level=3",
                "-Ctarget-cpu=gfx942",
                "-Ctarget-feature=+wavefrontsize32,-wavefrontsize64",
            ])
        );
    }

    #[test]
    fn nonsorted_identity_fields_preserve_rustc_option_order() {
        let target_then_edition = portable_args(&[
            "--target=amdgcn-amd-amdhsa",
            "--edition=2024",
            "-Ctarget-cpu=gfx942",
            "-Copt-level=3",
        ]);
        let edition_then_target = portable_args(&[
            "--edition=2024",
            "--target=amdgcn-amd-amdhsa",
            "-Ctarget-cpu=gfx942",
            "-Copt-level=3",
        ]);
        let reordered_codegen = portable_args(&[
            "--target=amdgcn-amd-amdhsa",
            "--edition=2024",
            "-Copt-level=3",
            "-Ctarget-cpu=gfx942",
        ]);
        assert_ne!(target_then_edition, edition_then_target);
        assert_ne!(target_then_edition, reordered_codegen);
    }

    #[test]
    fn portable_metadata_separates_package_and_manifest_identity() {
        let argv = semantic_argv("/checkout", &["kernel"], "gfx942");
        let baseline = portable(argv.clone(), &package_identity("1.0.0", 1));
        assert_ne!(
            baseline,
            portable(argv.clone(), &package_identity("1.0.1", 1))
        );
        assert_ne!(baseline, portable(argv, &package_identity("1.0.0", 2)));
    }

    #[cfg(unix)]
    #[test]
    fn unrelated_non_utf8_arguments_are_tolerated() {
        use std::os::unix::ffi::OsStringExt as _;

        let identity = package_identity("1.0.0", 1);
        let baseline = portable(semantic_argv("/checkout", &["kernel"], "gfx942"), &identity);
        let mut argv = semantic_argv("/checkout", &["kernel"], "gfx942");
        argv.extend([
            OsString::from("--extern"),
            OsString::from_vec(b"dep=/tmp/non-utf8-\xff.rmeta".to_vec()),
        ]);
        assert_eq!(baseline, portable(argv, &identity));
    }

    #[cfg(unix)]
    #[test]
    fn identity_bearing_non_utf8_argument_is_rejected() {
        use std::os::unix::ffi::OsStringExt as _;

        let mut argv = semantic_argv("/checkout", &["kernel"], "gfx942");
        argv.push(OsString::from_vec(b"-Ctarget-cpu=gfx\xff".to_vec()));
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap()
        else {
            panic!("fixture must classify as a compile invocation");
        };
        assert!(matches!(
            portable_rustc_metadata_v1(compile, &package_identity("1.0.0", 1)),
            Err(PortableMetadataErrorV1::NonUtf8RustcIdentityOption {
                option: "target-cpu",
                ..
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn manifest_capture_rejects_symlinks_and_hashes_the_retained_descriptor() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "fe2o3-portable-manifest-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let manifest = root.join("Cargo.toml");
        let replacement = root.join("replacement.toml");
        let link = root.join("linked.toml");
        let original_bytes = b"[package]\nname='original'\n";
        let replacement_bytes = b"[package]\nname='replaced'\n";
        assert_eq!(original_bytes.len(), replacement_bytes.len());
        std::fs::write(&manifest, original_bytes).unwrap();
        symlink(&manifest, &link).unwrap();
        assert!(matches!(
            open_portable_manifest_v1(&link),
            Err(PortableMetadataErrorV1::ManifestOpen { .. })
        ));

        let retained = open_portable_manifest_v1(&manifest).unwrap();
        std::fs::write(&replacement, replacement_bytes).unwrap();
        std::fs::rename(&replacement, &manifest).unwrap();
        assert_eq!(
            hash_open_portable_manifest_v1(retained, &manifest).unwrap(),
            Sha256::digest(original_bytes).as_slice(),
            "an atomic same-length path replacement must not replace the retained file",
        );
        assert_ne!(
            Sha256::digest(original_bytes).as_slice(),
            Sha256::digest(replacement_bytes).as_slice(),
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
