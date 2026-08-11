use sha2::{Digest, Sha256};
use std::ffi::OsStr;

pub(crate) const MODE_ENV: &str = "FE2O3_NON_PRODUCTION_COMPILER_REPRODUCTION_V1";
pub(crate) const MODE_VALUE: &str = "gfx942-alpha-zeta-cov6-v1";
const OBSERVATION_ENV: &str = "FE2O3_NON_PRODUCTION_COMPILER_TRANSITION_OBSERVATION_V1";
const OBSERVATION_VALUE: &str = "observe-without-golden-acceptance";
const DOMAIN: &[u8] = b"FE2O3/NON-PRODUCTION-COMPILER-REPRODUCTION/V1\0";
const CANONICAL_METADATA: &str = "fe2o3-gfx942-alpha-zeta-cov6-reproduction-v1";

pub(crate) fn enabled() -> bool {
    enabled_from(
        std::env::var(MODE_ENV).ok().as_deref(),
        std::env::var(OBSERVATION_ENV).ok().as_deref(),
    )
}

fn enabled_from(mode: Option<&str>, observation: Option<&str>) -> bool {
    mode == Some(MODE_VALUE) && observation == Some(OBSERVATION_VALUE)
}

pub(crate) fn deterministic_16(label: &[u8]) -> [u8; 16] {
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update((label.len() as u64).to_le_bytes());
    digest.update(label);
    digest.finalize()[..16]
        .try_into()
        .expect("fixed deterministic identity length")
}

pub(crate) const fn canonical_metadata() -> &'static str {
    CANONICAL_METADATA
}

pub(crate) fn canonicalize_argument(value: &OsStr) -> Vec<u8> {
    let mut bytes = os_bytes(value).to_vec();
    for (name, replacement) in [
        ("CARGO_TARGET_DIR", b"<cargo-target>".as_slice()),
        ("CARGO_HOME", b"<cargo-home>".as_slice()),
        ("HOME", b"<home>".as_slice()),
        ("TMPDIR", b"<tmp>".as_slice()),
    ] {
        if let Some(path) = std::env::var_os(name) {
            replace_all(&mut bytes, os_bytes(&path), replacement);
            if name == "CARGO_TARGET_DIR"
                && let Some(run_root) = std::path::Path::new(&path).parent()
            {
                replace_all(
                    &mut bytes,
                    os_bytes(run_root.as_os_str()),
                    b"<evidence-run>",
                );
            }
        }
    }
    canonicalize_codegen_identity(&bytes)
}

fn canonicalize_codegen_identity(value: &[u8]) -> Vec<u8> {
    for prefix in [b"metadata=".as_slice(), b"-Cmetadata=".as_slice()] {
        if value.starts_with(prefix) {
            let mut result = prefix.to_vec();
            result.extend_from_slice(CANONICAL_METADATA.as_bytes());
            return result;
        }
    }
    for prefix in [
        b"extra-filename=".as_slice(),
        b"-Cextra-filename=".as_slice(),
    ] {
        if value.starts_with(prefix) {
            let mut result = prefix.to_vec();
            result.extend_from_slice(b"-<cargo-extra-filename>");
            return result;
        }
    }
    value.to_vec()
}

fn replace_all(value: &mut Vec<u8>, needle: &[u8], replacement: &[u8]) {
    if needle.is_empty() {
        return;
    }
    let mut start = 0;
    while let Some(relative) = value[start..]
        .windows(needle.len())
        .position(|window| window == needle)
    {
        let index = start + relative;
        value.splice(index..index + needle.len(), replacement.iter().copied());
        start = index + replacement.len();
    }
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> &[u8] {
    value.to_str().unwrap_or_default().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_labels_are_stable_and_distinct() {
        assert_eq!(
            deterministic_16(b"generation"),
            deterministic_16(b"generation")
        );
        assert_ne!(
            deterministic_16(b"generation"),
            deterministic_16(b"session")
        );
        assert_ne!(deterministic_16(b"generation"), [0; 16]);
    }

    #[test]
    fn reproduction_requires_both_exact_non_production_gates() {
        assert!(enabled_from(Some(MODE_VALUE), Some(OBSERVATION_VALUE)));
        for (mode, observation) in [
            (None, None),
            (Some(MODE_VALUE), None),
            (None, Some(OBSERVATION_VALUE)),
            (Some("caller-selected"), Some(OBSERVATION_VALUE)),
            (Some(MODE_VALUE), Some("accept-golden")),
        ] {
            assert!(!enabled_from(mode, observation));
        }
    }

    #[test]
    fn canonicalizes_only_cargo_codegen_identity_fields() {
        assert_eq!(
            canonicalize_codegen_identity(b"metadata=random"),
            b"metadata=fe2o3-gfx942-alpha-zeta-cov6-reproduction-v1"
        );
        assert_eq!(
            canonicalize_codegen_identity(b"-Cextra-filename=-1234"),
            b"-Cextra-filename=-<cargo-extra-filename>"
        );
        assert_eq!(
            canonicalize_codegen_identity(b"--crate-name"),
            b"--crate-name"
        );
    }
}
