//! Typed retained property-to-theorem manifest for general-GEMM numerical claims.

use core::fmt;

use sha2::{Digest as _, Sha256};

use crate::GeneralGemmEvidenceIdentityV1;
use crate::general_gemm_numerical_correspondence_v1::{
    GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1, GeneralGemmNumericalCorrespondenceBasisV1,
    GeneralGemmNumericalCorrespondenceStatusV1, GeneralGemmNumericalPropertyFactV1,
    GeneralGemmNumericalPropertyV1,
};

/// Exact retained property-to-theorem manifest bytes.
pub const GENERAL_GEMM_NUMERICAL_PROPERTY_THEOREM_MANIFEST_V1: &[u8] =
    include_bytes!("../verus/pins/GENERAL_GEMM_NUMERICAL_PROPERTIES_V1.manifest");

const NUMERICAL_SOURCE: &[u8] = include_bytes!("../verus/general_gemm_numerical_contract_v1.rs");
const SCHEDULE_MODEL_SOURCE: &[u8] = include_bytes!("../verus/general_gemm_schedule_model_v1.rs");
const MANIFEST_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-numerical-property-manifest-v1\0";
const SOURCE_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-numerical-theorem-source-v1\0";
const STATEMENT_SOURCE_DOMAIN_V1: &[u8] =
    b"fe2o3-general-gemm-numerical-theorem-statement-source-v1\0";
const THEOREM_SET_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-numerical-theorem-set-v1\0";
const REVIEWED_MANIFEST_SHA256_V1: [u8; 32] = [
    0x41, 0xd1, 0xbc, 0x41, 0x48, 0x3d, 0x6f, 0x20, 0xd5, 0xe6, 0x8b, 0x55, 0x20, 0x50, 0x49, 0x22,
    0x15, 0xc3, 0x05, 0xea, 0x01, 0xca, 0xf5, 0x22, 0x8d, 0xa0, 0xe5, 0xf2, 0x24, 0xb1, 0xba, 0x5a,
];
const MAX_MANIFEST_BYTES_V1: usize = 32 * 1024;
const MAX_STATEMENT_BYTES_V1: usize = 256;
const MAX_THEOREM_NAME_BYTES_V1: usize = 128;

/// Retained source role selected by one theorem binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmNumericalTheoremSourceV1 {
    NumericalContract = 1,
    ScheduleModel = 2,
}

impl GeneralGemmNumericalTheoremSourceV1 {
    const fn name(self) -> &'static str {
        match self {
            Self::NumericalContract => "numerical-contract",
            Self::ScheduleModel => "schedule-model",
        }
    }

    const fn path(self) -> &'static str {
        match self {
            Self::NumericalContract => "proof/general_gemm_numerical_contract_v1.rs",
            Self::ScheduleModel => "proof/general_gemm_schedule_model_v1.rs",
        }
    }

    const fn bytes(self) -> &'static [u8] {
        match self {
            Self::NumericalContract => NUMERICAL_SOURCE,
            Self::ScheduleModel => SCHEDULE_MODEL_SOURCE,
        }
    }
}

/// Exact retained theorem or open-boundary binding for one property.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmNumericalPropertyTheoremBindingV1 {
    property: GeneralGemmNumericalPropertyV1,
    status: GeneralGemmNumericalCorrespondenceStatusV1,
    basis: GeneralGemmNumericalCorrespondenceBasisV1,
    source: GeneralGemmNumericalTheoremSourceV1,
    theorem_name: &'static str,
    statement: &'static str,
    statement_identity: GeneralGemmEvidenceIdentityV1,
    source_identity: GeneralGemmEvidenceIdentityV1,
    statement_source_identity: GeneralGemmEvidenceIdentityV1,
    record_identity: GeneralGemmEvidenceIdentityV1,
}

impl GeneralGemmNumericalPropertyTheoremBindingV1 {
    pub const fn property(self) -> GeneralGemmNumericalPropertyV1 {
        self.property
    }

    pub const fn status(self) -> GeneralGemmNumericalCorrespondenceStatusV1 {
        self.status
    }

    pub const fn basis(self) -> GeneralGemmNumericalCorrespondenceBasisV1 {
        self.basis
    }

    pub const fn source(self) -> GeneralGemmNumericalTheoremSourceV1 {
        self.source
    }

    pub const fn theorem_name(self) -> &'static str {
        self.theorem_name
    }

    pub const fn statement(self) -> &'static str {
        self.statement
    }

    pub const fn statement_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.statement_identity
    }

    pub const fn source_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.source_identity
    }

    pub const fn statement_source_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.statement_source_identity
    }

    pub const fn record_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.record_identity
    }

    pub const fn fact(self) -> GeneralGemmNumericalPropertyFactV1 {
        GeneralGemmNumericalPropertyFactV1 {
            property: self.property,
            status: self.status,
            basis: self.basis,
        }
    }
}

/// Parsed exact retained theorem manifest. This record grants no proof authority.
///
/// ```compile_fail
/// fn duplicate(
///     value: &fe2o3_verifier::GeneralGemmNumericalPropertyTheoremManifestV1,
/// ) {
///     let _copy = (*value).clone();
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct GeneralGemmNumericalPropertyTheoremManifestV1 {
    bindings:
        [GeneralGemmNumericalPropertyTheoremBindingV1; GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1],
    identity: GeneralGemmEvidenceIdentityV1,
    theorem_set_identity: GeneralGemmEvidenceIdentityV1,
}

impl GeneralGemmNumericalPropertyTheoremManifestV1 {
    pub const fn bindings(
        &self,
    ) -> [GeneralGemmNumericalPropertyTheoremBindingV1; GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1]
    {
        self.bindings
    }

    pub const fn identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.identity
    }

    pub const fn theorem_set_identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.theorem_set_identity
    }

    pub const fn grants_proof_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Exact retained-manifest validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmNumericalPropertyManifestErrorV1 {
    InvalidEncoding,
    InvalidManifestIdentity,
    InvalidShape,
    InvalidSource,
    InvalidProperty,
    InvalidStatusBasis,
    InvalidTheorem,
    InvalidStatement,
    InvalidRecordIdentity,
}

impl fmt::Display for GeneralGemmNumericalPropertyManifestErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "general GEMM numerical property manifest failed: {self:?}"
        )
    }
}

impl std::error::Error for GeneralGemmNumericalPropertyManifestErrorV1 {}

/// Parses the exact reviewed retained manifest into typed, non-authoritative bindings.
pub fn reviewed_general_gemm_numerical_property_theorem_manifest_v1() -> Result<
    GeneralGemmNumericalPropertyTheoremManifestV1,
    GeneralGemmNumericalPropertyManifestErrorV1,
> {
    parse_manifest(GENERAL_GEMM_NUMERICAL_PROPERTY_THEOREM_MANIFEST_V1)
}

fn parse_manifest(
    bytes: &'static [u8],
) -> Result<
    GeneralGemmNumericalPropertyTheoremManifestV1,
    GeneralGemmNumericalPropertyManifestErrorV1,
> {
    if bytes.is_empty()
        || bytes.len() > MAX_MANIFEST_BYTES_V1
        || !bytes.ends_with(b"\n")
        || bytes.contains(&b'\r')
    {
        return Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidEncoding);
    }
    if <[u8; 32]>::from(Sha256::digest(bytes)) != REVIEWED_MANIFEST_SHA256_V1 {
        return Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidManifestIdentity);
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| GeneralGemmNumericalPropertyManifestErrorV1::InvalidEncoding)?;
    let lines: Vec<_> = text[..text.len() - 1].split('\n').collect();
    if lines.len() != 4 + GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1
        || lines[0] != "format|fe2o3-general-gemm-numerical-property-theorems-v1"
        || lines[1] != "property-count|11"
    {
        return Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidShape);
    }

    let numerical_source = parse_source_line(
        lines[2],
        GeneralGemmNumericalTheoremSourceV1::NumericalContract,
    )?;
    let schedule_source =
        parse_source_line(lines[3], GeneralGemmNumericalTheoremSourceV1::ScheduleModel)?;
    let sources = [numerical_source, schedule_source];

    let mut parsed = Vec::with_capacity(GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1);
    for (index, line) in lines[4..].iter().copied().enumerate() {
        parsed.push(parse_property_line(index + 1, line, &sources)?);
    }
    let bindings: [GeneralGemmNumericalPropertyTheoremBindingV1;
        GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1] = parsed
        .try_into()
        .map_err(|_| GeneralGemmNumericalPropertyManifestErrorV1::InvalidShape)?;

    let mut theorem_set_hasher = Sha256::new();
    theorem_set_hasher.update(THEOREM_SET_DOMAIN_V1);
    for binding in bindings {
        theorem_set_hasher.update(binding.statement_source_identity.as_bytes());
        theorem_set_hasher.update(binding.record_identity.as_bytes());
    }
    let theorem_set_identity = identity(theorem_set_hasher.finalize().into());

    let mut hasher = Sha256::new();
    hasher.update(MANIFEST_DOMAIN_V1);
    put_blob(&mut hasher, bytes);
    for source in sources {
        hasher.update(source.identity.as_bytes());
    }
    for binding in bindings {
        hasher.update(binding.statement_source_identity.as_bytes());
        hasher.update(binding.record_identity.as_bytes());
    }
    Ok(GeneralGemmNumericalPropertyTheoremManifestV1 {
        bindings,
        identity: identity(hasher.finalize().into()),
        theorem_set_identity,
    })
}

#[derive(Clone, Copy)]
struct SourceRecordV1 {
    role: GeneralGemmNumericalTheoremSourceV1,
    identity: GeneralGemmEvidenceIdentityV1,
}

fn parse_source_line(
    line: &str,
    expected: GeneralGemmNumericalTheoremSourceV1,
) -> Result<SourceRecordV1, GeneralGemmNumericalPropertyManifestErrorV1> {
    let fields: Vec<_> = line.split('|').collect();
    if fields.len() != 5
        || fields[0] != "source"
        || fields[1] != expected.name()
        || fields[2] != expected.path()
        || fields[3].parse::<usize>().ok() != Some(expected.bytes().len())
    {
        return Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidSource);
    }
    let declared = parse_sha256(fields[4])?;
    if Sha256::digest(expected.bytes()).as_slice() != declared {
        return Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidSource);
    }
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_DOMAIN_V1);
    hasher.update([expected as u8]);
    put_blob(&mut hasher, expected.path().as_bytes());
    put_blob(&mut hasher, expected.bytes());
    Ok(SourceRecordV1 {
        role: expected,
        identity: identity(hasher.finalize().into()),
    })
}

fn parse_property_line(
    expected_index: usize,
    line: &'static str,
    sources: &[SourceRecordV1; 2],
) -> Result<GeneralGemmNumericalPropertyTheoremBindingV1, GeneralGemmNumericalPropertyManifestErrorV1>
{
    let (record, declared_record_identity) = line
        .rsplit_once('|')
        .ok_or(GeneralGemmNumericalPropertyManifestErrorV1::InvalidShape)?;
    if Sha256::digest(record.as_bytes()).as_slice() != parse_sha256(declared_record_identity)? {
        return Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidRecordIdentity);
    }
    let fields: Vec<_> = record.split('|').collect();
    if fields.len() != 9 || fields[0] != "property" {
        return Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidShape);
    }
    let index = fields[1]
        .parse::<usize>()
        .map_err(|_| GeneralGemmNumericalPropertyManifestErrorV1::InvalidProperty)?;
    let property = parse_property(fields[2])?;
    if index != expected_index || property as usize != expected_index {
        return Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidProperty);
    }
    let status = parse_status(fields[3])?;
    let basis = parse_basis(fields[4])?;
    let source = parse_source(fields[5])?;
    let source_record = sources
        .iter()
        .copied()
        .find(|record| record.role == source)
        .ok_or(GeneralGemmNumericalPropertyManifestErrorV1::InvalidSource)?;
    let theorem_name = fields[6];
    if theorem_name.is_empty()
        || theorem_name.len() > MAX_THEOREM_NAME_BYTES_V1
        || !theorem_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        || !contains_token(source.bytes(), theorem_name)
    {
        return Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidTheorem);
    }
    if status == GeneralGemmNumericalCorrespondenceStatusV1::ModelOnly
        && source != GeneralGemmNumericalTheoremSourceV1::ScheduleModel
        || status != GeneralGemmNumericalCorrespondenceStatusV1::ModelOnly
            && source != GeneralGemmNumericalTheoremSourceV1::NumericalContract
    {
        return Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidStatusBasis);
    }
    let statement = fields[7];
    if statement.is_empty()
        || statement.len() > MAX_STATEMENT_BYTES_V1
        || !statement.is_ascii()
        || Sha256::digest(statement.as_bytes()).as_slice() != parse_sha256(fields[8])?
    {
        return Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidStatement);
    }
    let statement_identity = identity(parse_sha256(fields[8])?);
    let record_identity = identity(parse_sha256(declared_record_identity)?);
    let mut hasher = Sha256::new();
    hasher.update(STATEMENT_SOURCE_DOMAIN_V1);
    hasher.update([property as u8, status as u8, basis as u8, source as u8]);
    put_blob(&mut hasher, theorem_name.as_bytes());
    hasher.update(statement_identity.as_bytes());
    hasher.update(source_record.identity.as_bytes());
    hasher.update(record_identity.as_bytes());
    Ok(GeneralGemmNumericalPropertyTheoremBindingV1 {
        property,
        status,
        basis,
        source,
        theorem_name,
        statement,
        statement_identity,
        source_identity: source_record.identity,
        statement_source_identity: identity(hasher.finalize().into()),
        record_identity,
    })
}

fn parse_property(
    value: &str,
) -> Result<GeneralGemmNumericalPropertyV1, GeneralGemmNumericalPropertyManifestErrorV1> {
    use GeneralGemmNumericalPropertyV1 as Property;
    match value {
        "exact-bf16-to-f32-encoding-widening" => Ok(Property::ExactBf16ToF32EncodingWidening),
        "bf16-rust-kir-refinement" => Ok(Property::Bf16RustKirRefinement),
        "bf16-ieee-value-interpretation" => Ok(Property::Bf16IeeeValueInterpretation),
        "fp32-multiply-rne" => Ok(Property::Fp32MultiplyRoundToNearestTiesEven),
        "fp32-add-rne" => Ok(Property::Fp32AddRoundToNearestTiesEven),
        "increasing-k-separate-mul-add-order" => Ok(Property::IncreasingKSeparateMulAddOrder),
        "separate-alpha-beta-epilogue-order" => Ok(Property::SeparateAlphaBetaEpilogueOrder),
        "gfx942-mfma-shape-controls" => Ok(Property::Gfx942MfmaShapeAndControls),
        "gfx942-mfma-fp32-accumulation" => Ok(Property::Gfx942MfmaFp32Accumulation),
        "exceptional-and-subnormal-values" => Ok(Property::ExceptionalAndSubnormalValues),
        "emitted-machine-numerical-refinement" => Ok(Property::EmittedMachineNumericalRefinement),
        _ => Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidProperty),
    }
}

fn parse_status(
    value: &str,
) -> Result<GeneralGemmNumericalCorrespondenceStatusV1, GeneralGemmNumericalPropertyManifestErrorV1>
{
    use GeneralGemmNumericalCorrespondenceStatusV1 as Status;
    match value {
        "proved" => Ok(Status::Proved),
        "model-only" => Ok(Status::ModelOnly),
        "contracted" => Ok(Status::Contracted),
        "unsupported" => Ok(Status::Unsupported),
        _ => Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidStatusBasis),
    }
}

fn parse_basis(
    value: &str,
) -> Result<GeneralGemmNumericalCorrespondenceBasisV1, GeneralGemmNumericalPropertyManifestErrorV1>
{
    use GeneralGemmNumericalCorrespondenceBasisV1 as Basis;
    match value {
        "verus-bf16-encoding-theorem" => Ok(Basis::VerusBf16EncodingTheorem),
        "open-rust-kir-refinement" => Ok(Basis::OpenRustKirRefinement),
        "ieee754-binary32-contract" => Ok(Basis::Ieee754Binary32Contract),
        "exact-real-schedule-model" => Ok(Basis::ExactRealScheduleModel),
        "gfx942-mfma-instruction-contract" => Ok(Basis::Gfx942MfmaInstructionContract),
        "finite-normal-or-zero-policy" => Ok(Basis::FiniteNormalOrZeroPolicy),
        "future-graph-worker-finalizer-join" => Ok(Basis::FutureGraphWorkerFinalizerJoin),
        _ => Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidStatusBasis),
    }
}

fn parse_source(
    value: &str,
) -> Result<GeneralGemmNumericalTheoremSourceV1, GeneralGemmNumericalPropertyManifestErrorV1> {
    match value {
        "numerical-contract" => Ok(GeneralGemmNumericalTheoremSourceV1::NumericalContract),
        "schedule-model" => Ok(GeneralGemmNumericalTheoremSourceV1::ScheduleModel),
        _ => Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidSource),
    }
}

fn contains_token(source: &[u8], expected: &str) -> bool {
    std::str::from_utf8(source).is_ok_and(|source| {
        source
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .any(|token| token == expected)
    })
}

fn parse_sha256(value: &str) -> Result<[u8; 32], GeneralGemmNumericalPropertyManifestErrorV1> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidEncoding);
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> Result<u8, GeneralGemmNumericalPropertyManifestErrorV1> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(GeneralGemmNumericalPropertyManifestErrorV1::InvalidEncoding),
    }
}

const fn identity(bytes: [u8; 32]) -> GeneralGemmEvidenceIdentityV1 {
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(bytes)
}

fn put_blob(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_manifest_has_one_proved_property_and_exact_open_boundaries() {
        let manifest = reviewed_general_gemm_numerical_property_theorem_manifest_v1().unwrap();
        let bindings = manifest.bindings();
        let proved: Vec<_> = bindings
            .iter()
            .filter(|binding| {
                binding.status() == GeneralGemmNumericalCorrespondenceStatusV1::Proved
            })
            .map(|binding| binding.property())
            .collect();
        assert_eq!(
            proved,
            [GeneralGemmNumericalPropertyV1::ExactBf16ToF32EncodingWidening]
        );
        assert!(!manifest.grants_proof_or_runtime_authority());
        assert_ne!(manifest.identity().as_bytes(), &[0; 32]);
        assert_ne!(manifest.theorem_set_identity().as_bytes(), &[0; 32]);
    }

    #[test]
    fn status_weakening_statement_and_record_mutations_fail_closed() {
        let original =
            std::str::from_utf8(GENERAL_GEMM_NUMERICAL_PROPERTY_THEOREM_MANIFEST_V1).unwrap();
        for mutated in [
            recompute_property_record_sha(&original.replacen("|proved|", "|model-only|", 1), 0),
            recompute_property_record_sha(&original.replacen("|model-only|", "|proved|", 1), 5),
            recompute_property_record_sha(&original.replacen("|contracted|", "|proved|", 1), 2),
            recompute_property_record_sha(
                &original.replacen("|unsupported|", "|contracted|", 1),
                1,
            ),
            original.replacen("widen-div-65536", "widen-div-65535", 1),
            original.replacen("9bd705c42f480d82", "8bd705c42f480d82", 1),
        ] {
            let leaked: &'static [u8] = Box::leak(mutated.into_bytes().into_boxed_slice());
            assert!(parse_manifest(leaked).is_err());
        }
    }

    fn recompute_property_record_sha(manifest: &str, property_index: usize) -> String {
        use std::fmt::Write as _;

        let mut lines: Vec<_> = manifest.lines().map(str::to_owned).collect();
        let line_index = 4 + property_index;
        let (record, _) = lines[line_index].rsplit_once('|').unwrap();
        let mut digest = String::with_capacity(64);
        for byte in Sha256::digest(record.as_bytes()) {
            write!(&mut digest, "{byte:02x}").unwrap();
        }
        lines[line_index] = format!("{record}|{digest}");
        format!("{}\n", lines.join("\n"))
    }
}
