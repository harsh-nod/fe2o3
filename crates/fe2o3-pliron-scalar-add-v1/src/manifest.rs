//! Byte-canonical checkout-embedded approval profile parsing.

use core::fmt;

use fe2o3_hsaco_finalize::{
    ContentIdentityV1, PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES,
    PLIRON_SCALAR_ADD_V1_IMPLICIT_KERNARG_BYTES, PLIRON_SCALAR_ADD_V1_KERNARG_ALIGNMENT,
    PLIRON_SCALAR_ADD_V1_KERNARG_BYTES, PLIRON_SCALAR_ADD_V1_LLVM_BUILD_IDENTITY,
    PLIRON_SCALAR_ADD_V1_TARGET, PlironScalarAddV1AmdhsaDescriptorIdentity,
    PlironScalarAddV1MachineIdentity,
};
use sha2::{Digest as _, Sha256};

use crate::{
    authority::{RepositoryApprovalIdentityV1, RepositoryScalarAddProfileV1},
    source::{CanonicalSourceObservationV1, canonical_source_observation_v1},
};

const MANIFEST_BYTES: &[u8] = include_bytes!("../repository-approval-v1.manifest");
const MANIFEST_HEADER: &str = "FE2O3_PLIRON_SCALAR_ADD_REPOSITORY_APPROVAL_V1";
const EXPECTED_KEYS: &[&str] = &[
    "qualification",
    "serial",
    "pin_scope",
    "trust_assumption",
    "source_observation_authority",
    "source_observation_hsa_touched",
    "code_target",
    "runtime_implementation",
    "runtime_version",
    "runtime_image_sha256",
    "source_sha256",
    "source_length",
    "source_manifest_sha256",
    "source_manifest_length",
    "origin_identity_sha256",
    "semantic_identity_sha256",
    "schedule_identity_sha256",
    "target_plan_identity_sha256",
    "v2_handoff_identity_sha256",
    "assembly_sha256",
    "assembly_length",
    "compiler_handoff_sha256",
    "compiler_handoff_length",
    "symbol_manifest_sha256",
    "symbol_manifest_length",
    "worker_executable_sha256",
    "worker_executable_length",
    "worker_embedded_build_identity",
    "worker_embedded_llvm_identity",
    "hsaco_sha256",
    "hsaco_length",
    "descriptor_sha256",
    "machine_sha256",
    "kernarg_explicit",
    "kernarg_implicit",
    "kernarg_total",
    "kernarg_alignment",
    "runtime_kernarg_alignment",
    "grid",
    "workgroup",
    "dynamic_lds",
    "static_group_segment",
    "private_segment",
];

/// One malformed or unqualified field in the checkout approval manifest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RepositoryManifestFieldV1 {
    /// The exact framing, fixed line order/count, key set, or serial changed.
    Structure,
    /// This checkout has not qualified the embedded profile.
    Qualification,
    /// Compile-time pin scope or its explicit provenance assumption changed.
    TrustScope,
    /// Embedded source bytes or deterministic source lineage changed.
    SourceLineage,
    /// The code-object target differs from the exact scalar profile.
    CodeTarget,
    /// The measured ROCr/HIP runtime implementation, version, or image changed.
    RuntimeStack,
    /// The exact worker executable identity is missing or malformed.
    WorkerExecutable,
    /// The worker build identity reported for that executable is malformed.
    WorkerBuildIdentity,
    /// The pinned upstream LLVM identity is missing or changed.
    LlvmBuildIdentity,
    /// The complete HSACO identity is missing or malformed.
    OutputIdentity,
    /// The exact descriptor identity is missing or malformed.
    DescriptorIdentity,
    /// The exact machine-code identity is missing or malformed.
    MachineIdentity,
    /// The fixed ABI or singleton resource contract changed.
    RuntimeContract,
}

/// Failure to obtain the sole checkout-embedded scalar-add policy profile.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RepositoryProfileErrorV1 {
    /// A qualification field or any decorated placeholder remains.
    NotQualified,
    /// A qualified manifest is malformed or differs from the closed profile.
    Malformed(RepositoryManifestFieldV1),
}

impl fmt::Display for RepositoryProfileErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotQualified => formatter.write_str(
                "checkout scalar-add profile is not qualified; an approval pin remains a placeholder",
            ),
            Self::Malformed(field) => {
                write!(formatter, "checkout scalar-add manifest is invalid at {field:?}")
            }
        }
    }
}

impl std::error::Error for RepositoryProfileErrorV1 {}

/// Returns the only scalar-add qualification policy embedded in this checkout.
///
/// The exact bytes of `repository-approval-v1.manifest` are compiled into this
/// crate. The manifest is not a signature and does not establish external
/// repository or build authenticity; trust assumes the provenance of this
/// checkout and its measured build inputs. There is no public constructor from
/// raw pins or worker observations.
pub fn repository_profile_v1() -> Result<RepositoryScalarAddProfileV1, RepositoryProfileErrorV1> {
    parse_repository_profile(MANIFEST_BYTES)
}

struct CanonicalFieldsV1<'a> {
    values: Vec<&'a str>,
}

impl<'a> CanonicalFieldsV1<'a> {
    fn get(&self, key: &str) -> Result<&'a str, RepositoryProfileErrorV1> {
        EXPECTED_KEYS
            .iter()
            .position(|expected| *expected == key)
            .and_then(|index| self.values.get(index).copied())
            .ok_or_else(|| malformed(RepositoryManifestFieldV1::Structure))
    }
}

fn parse_repository_profile(
    bytes: &[u8],
) -> Result<RepositoryScalarAddProfileV1, RepositoryProfileErrorV1> {
    let fields = parse_canonical_fields(bytes)?;
    match fields.get("qualification")? {
        "NOT_QUALIFIED" => return Err(RepositoryProfileErrorV1::NotQualified),
        "QUALIFIED" => {}
        _ => return Err(malformed(RepositoryManifestFieldV1::Qualification)),
    }
    if fields.get("serial")? != "1" {
        return Err(malformed(RepositoryManifestFieldV1::Structure));
    }
    if fields.get("pin_scope")? != "compile_time_embedded_checkout"
        || fields.get("trust_assumption")? != "repository_and_build_provenance"
        || fields.get("source_observation_authority")? != "none"
        || fields.get("source_observation_hsa_touched")? != "false"
    {
        return Err(malformed(RepositoryManifestFieldV1::TrustScope));
    }
    if fields.get("code_target")? != PLIRON_SCALAR_ADD_V1_TARGET {
        return Err(malformed(RepositoryManifestFieldV1::CodeTarget));
    }
    let runtime_implementation = identity_text(
        fields.get("runtime_implementation")?,
        RepositoryManifestFieldV1::RuntimeStack,
    )?;
    let runtime_version = identity_text(
        fields.get("runtime_version")?,
        RepositoryManifestFieldV1::RuntimeStack,
    )?;
    if runtime_implementation != "ROCr_HSA" || runtime_version != "1.18" {
        return Err(malformed(RepositoryManifestFieldV1::RuntimeStack));
    }
    let runtime_image_sha256 = nonzero_digest(
        fields.get("runtime_image_sha256")?,
        RepositoryManifestFieldV1::RuntimeStack,
    )?;
    validate_runtime_contract(&fields)?;

    let source = canonical_source_observation_v1()
        .map_err(|_| malformed(RepositoryManifestFieldV1::SourceLineage))?;
    validate_source_pins(&fields, source)?;

    let worker_executable = content_identity(
        &fields,
        "worker_executable_sha256",
        "worker_executable_length",
        RepositoryManifestFieldV1::WorkerExecutable,
    )?;
    let worker_build_identity = identity_text(
        fields.get("worker_embedded_build_identity")?,
        RepositoryManifestFieldV1::WorkerBuildIdentity,
    )?;
    let llvm_build_identity = identity_text(
        fields.get("worker_embedded_llvm_identity")?,
        RepositoryManifestFieldV1::LlvmBuildIdentity,
    )?;
    if llvm_build_identity != PLIRON_SCALAR_ADD_V1_LLVM_BUILD_IDENTITY {
        return Err(malformed(RepositoryManifestFieldV1::LlvmBuildIdentity));
    }
    let output_identity = content_identity(
        &fields,
        "hsaco_sha256",
        "hsaco_length",
        RepositoryManifestFieldV1::OutputIdentity,
    )?;
    let descriptor = nonzero_digest(
        fields.get("descriptor_sha256")?,
        RepositoryManifestFieldV1::DescriptorIdentity,
    )?;
    let machine = nonzero_digest(
        fields.get("machine_sha256")?,
        RepositoryManifestFieldV1::MachineIdentity,
    )?;
    let manifest_identity =
        RepositoryApprovalIdentityV1::from_manifest_digest(Sha256::digest(bytes).into());

    Ok(RepositoryScalarAddProfileV1::from_repository_manifest(
        manifest_identity,
        source,
        worker_executable,
        worker_build_identity.to_owned(),
        llvm_build_identity.to_owned(),
        runtime_implementation.replace('_', " "),
        runtime_version.to_owned(),
        runtime_image_sha256,
        output_identity,
        PlironScalarAddV1AmdhsaDescriptorIdentity::from_bytes(descriptor),
        PlironScalarAddV1MachineIdentity::from_bytes(machine),
    ))
}

fn parse_canonical_fields(bytes: &[u8]) -> Result<CanonicalFieldsV1<'_>, RepositoryProfileErrorV1> {
    let text =
        core::str::from_utf8(bytes).map_err(|_| malformed(RepositoryManifestFieldV1::Structure))?;
    if !text.ends_with('\n') || text.as_bytes().contains(&b'\r') || text.as_bytes().contains(&0) {
        return Err(malformed(RepositoryManifestFieldV1::Structure));
    }
    let body = text
        .strip_suffix('\n')
        .ok_or_else(|| malformed(RepositoryManifestFieldV1::Structure))?;
    let lines = body.split('\n').collect::<Vec<_>>();
    if lines.len() != EXPECTED_KEYS.len() + 1 || lines.first().copied() != Some(MANIFEST_HEADER) {
        return Err(malformed(RepositoryManifestFieldV1::Structure));
    }

    let mut values = Vec::with_capacity(EXPECTED_KEYS.len());
    for (line, expected_key) in lines[1..].iter().zip(EXPECTED_KEYS) {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| malformed(RepositoryManifestFieldV1::Structure))?;
        if key != *expected_key
            || value.is_empty()
            || value.contains('=')
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(malformed(RepositoryManifestFieldV1::Structure));
        }
        if value.to_ascii_uppercase().contains("PLACEHOLDER") {
            return Err(RepositoryProfileErrorV1::NotQualified);
        }
        values.push(value);
    }
    Ok(CanonicalFieldsV1 { values })
}

fn validate_source_pins(
    fields: &CanonicalFieldsV1<'_>,
    observed: CanonicalSourceObservationV1,
) -> Result<(), RepositoryProfileErrorV1> {
    let expected = [
        (
            content_identity(
                fields,
                "source_sha256",
                "source_length",
                RepositoryManifestFieldV1::SourceLineage,
            )?,
            observed.source_identity(),
        ),
        (
            content_identity(
                fields,
                "source_manifest_sha256",
                "source_manifest_length",
                RepositoryManifestFieldV1::SourceLineage,
            )?,
            observed.source_manifest_identity(),
        ),
        (
            content_identity(
                fields,
                "assembly_sha256",
                "assembly_length",
                RepositoryManifestFieldV1::SourceLineage,
            )?,
            observed.assembly_identity(),
        ),
        (
            content_identity(
                fields,
                "compiler_handoff_sha256",
                "compiler_handoff_length",
                RepositoryManifestFieldV1::SourceLineage,
            )?,
            observed.compiler_handoff_identity(),
        ),
        (
            content_identity(
                fields,
                "symbol_manifest_sha256",
                "symbol_manifest_length",
                RepositoryManifestFieldV1::SourceLineage,
            )?,
            observed.symbol_manifest_identity(),
        ),
    ];
    if expected.iter().any(|(pinned, actual)| pinned != actual) {
        return Err(malformed(RepositoryManifestFieldV1::SourceLineage));
    }
    for (key, actual) in [
        ("origin_identity_sha256", observed.origin_identity()),
        ("semantic_identity_sha256", observed.semantic_identity()),
        ("schedule_identity_sha256", observed.schedule_identity()),
        (
            "target_plan_identity_sha256",
            observed.target_plan_identity(),
        ),
        ("v2_handoff_identity_sha256", observed.v2_handoff_identity()),
    ] {
        if nonzero_digest(fields.get(key)?, RepositoryManifestFieldV1::SourceLineage)? != *actual {
            return Err(malformed(RepositoryManifestFieldV1::SourceLineage));
        }
    }
    Ok(())
}

fn validate_runtime_contract(
    fields: &CanonicalFieldsV1<'_>,
) -> Result<(), RepositoryProfileErrorV1> {
    let expected = [
        (
            "kernarg_explicit",
            PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES.to_string(),
        ),
        (
            "kernarg_implicit",
            PLIRON_SCALAR_ADD_V1_IMPLICIT_KERNARG_BYTES.to_string(),
        ),
        (
            "kernarg_total",
            PLIRON_SCALAR_ADD_V1_KERNARG_BYTES.to_string(),
        ),
        (
            "kernarg_alignment",
            PLIRON_SCALAR_ADD_V1_KERNARG_ALIGNMENT.to_string(),
        ),
        ("runtime_kernarg_alignment", "16".to_owned()),
        ("grid", "1,1,1".to_owned()),
        ("workgroup", "1,1,1".to_owned()),
        ("dynamic_lds", "0".to_owned()),
        ("static_group_segment", "0".to_owned()),
        ("private_segment", "0".to_owned()),
    ];
    if expected
        .iter()
        .any(|(key, expected)| fields.get(key).ok() != Some(expected.as_str()))
    {
        return Err(malformed(RepositoryManifestFieldV1::RuntimeContract));
    }
    Ok(())
}

fn content_identity(
    fields: &CanonicalFieldsV1<'_>,
    digest_key: &str,
    length_key: &str,
    error_field: RepositoryManifestFieldV1,
) -> Result<ContentIdentityV1, RepositoryProfileErrorV1> {
    let digest = nonzero_digest(fields.get(digest_key)?, error_field)?;
    let length = fields
        .get(length_key)?
        .parse::<u64>()
        .ok()
        .filter(|length| *length != 0)
        .ok_or_else(|| malformed(error_field))?;
    if fields.get(length_key)? != length.to_string() {
        return Err(malformed(error_field));
    }
    Ok(ContentIdentityV1::from_parts(digest, length))
}

fn nonzero_digest(
    value: &str,
    error_field: RepositoryManifestFieldV1,
) -> Result<[u8; 32], RepositoryProfileErrorV1> {
    let digest = parse_digest(value).ok_or_else(|| malformed(error_field))?;
    if digest == [0; 32] {
        return Err(malformed(error_field));
    }
    Ok(digest)
}

fn parse_digest(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.is_ascii() {
        return None;
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        digest[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Some(digest)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn identity_text(
    value: &str,
    error_field: RepositoryManifestFieldV1,
) -> Result<&str, RepositoryProfileErrorV1> {
    if value.is_empty()
        || value.len() > 256
        || !value.is_ascii()
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(malformed(error_field));
    }
    Ok(value)
}

const fn malformed(field: RepositoryManifestFieldV1) -> RepositoryProfileErrorV1 {
    RepositoryProfileErrorV1::Malformed(field)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replace_line(bytes: &[u8], index: usize, replacement: &str) -> Vec<u8> {
        let text = core::str::from_utf8(bytes).unwrap();
        let mut lines = text
            .strip_suffix('\n')
            .unwrap()
            .split('\n')
            .collect::<Vec<_>>();
        lines[index] = replacement;
        format!("{}\n", lines.join("\n")).into_bytes()
    }

    #[test]
    fn checked_in_manifest_is_checkout_qualified_and_source_bound() {
        let profile = repository_profile_v1().unwrap();
        let source = canonical_source_observation_v1().unwrap();
        assert_eq!(profile.canonical_source(), source);
        assert!(profile.matches_embedded_worker_pins(
            profile.worker_executable(),
            profile.worker_build_identity(),
            profile.llvm_build_identity(),
        ));
        assert_eq!(profile.worker_executable().byte_len(), 86_057_672);
        assert_eq!(profile.output_identity().byte_len(), 4_984);
    }

    #[test]
    fn trust_scope_is_honest_and_not_signature_claiming() {
        let text = core::str::from_utf8(MANIFEST_BYTES).unwrap();
        assert!(text.contains("pin_scope=compile_time_embedded_checkout"));
        assert!(text.contains("trust_assumption=repository_and_build_provenance"));
        assert!(text.contains("source_observation_authority=none"));
        assert!(text.contains("source_observation_hsa_touched=false"));
        assert!(!text.to_ascii_lowercase().contains("signed"));
        assert!(!text.contains("approval_owner="));
    }

    #[test]
    fn manifest_requires_exact_line_order_count_and_trailing_newline() {
        assert!(parse_canonical_fields(MANIFEST_BYTES).is_ok());
        let mut missing_newline = MANIFEST_BYTES.to_vec();
        missing_newline.pop();
        let mut trailing_blank = MANIFEST_BYTES.to_vec();
        trailing_blank.push(b'\n');
        let text = core::str::from_utf8(MANIFEST_BYTES).unwrap();
        let lines = text
            .strip_suffix('\n')
            .unwrap()
            .split('\n')
            .collect::<Vec<_>>();
        let duplicate = format!("{}{}\n", text, lines[1]);
        let unknown = replace_line(MANIFEST_BYTES, 1, "unknown=value");
        for hostile in [
            missing_newline,
            trailing_blank,
            duplicate.into_bytes(),
            unknown,
        ] {
            assert!(matches!(
                parse_canonical_fields(&hostile),
                Err(RepositoryProfileErrorV1::Malformed(
                    RepositoryManifestFieldV1::Structure
                ))
            ));
        }
        for index in 1..lines.len() - 1 {
            let mut reordered = lines.clone();
            reordered.swap(index, index + 1);
            let hostile = format!("{}\n", reordered.join("\n"));
            assert!(matches!(
                parse_canonical_fields(hostile.as_bytes()),
                Err(RepositoryProfileErrorV1::Malformed(
                    RepositoryManifestFieldV1::Structure
                ))
            ));
        }
    }

    #[test]
    fn every_field_rejects_plain_or_decorated_placeholder_substitution() {
        for (offset, key) in EXPECTED_KEYS.iter().enumerate() {
            for value in ["PLACEHOLDER", "xPLACEHOLDER", "placeholder_pending"] {
                let hostile = replace_line(MANIFEST_BYTES, offset + 1, &format!("{key}={value}"));
                assert!(
                    matches!(
                        parse_repository_profile(&hostile),
                        Err(RepositoryProfileErrorV1::NotQualified)
                    ),
                    "key {key} accepted {value}",
                );
            }
        }
    }

    #[test]
    fn exact_manifest_bytes_have_one_stable_identity() {
        let first = parse_repository_profile(MANIFEST_BYTES).unwrap();
        let second = parse_repository_profile(MANIFEST_BYTES).unwrap();
        assert_eq!(first.identity(), second.identity());
        assert_eq!(
            first.identity().as_bytes(),
            &<[u8; 32]>::from(Sha256::digest(MANIFEST_BYTES))
        );
    }

    #[test]
    fn decimal_lengths_require_one_canonical_encoding() {
        let hostile = replace_line(MANIFEST_BYTES, 12, "source_length=000463");
        assert!(matches!(
            parse_repository_profile(&hostile),
            Err(RepositoryProfileErrorV1::Malformed(
                RepositoryManifestFieldV1::SourceLineage
            ))
        ));
    }
}
