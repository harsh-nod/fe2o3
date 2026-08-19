//! Canonical checkout-embedded backend fixture and deterministic Pliron lineage.
//!
//! This closed fixture is not a user-facing source language and does not stand
//! in for the authenticated Rust/MIR frontend required by issue #134. Its exact
//! fields are parsed into the typed scalar module so the backend qualification
//! lane cannot hash one operation sequence while lowering another.

use core::fmt;

use fe2o3_amdgcn_pliron_llvm::{ScalarKernelModuleV1, ScalarOperationV1, lower_scalar_kernel_v2};
use fe2o3_hsaco_finalize::ContentIdentityV1;
use fe2o3_llvm_handoff::{IdentityV1, StageIdentitiesV1};
use fe2o3_pliron_worker_v2::{
    PreparedScalarAddWorkerV2, SCALAR_ADD_KERNEL_SYMBOL_V1, prepare_scalar_add_worker_v2,
};
use sha2::{Digest as _, Sha256};

pub(crate) const CANONICAL_SOURCE_BYTES_V1: &[u8] =
    include_bytes!("canonical-scalar-add-v1.source");
pub(crate) const CANONICAL_SOURCE_MANIFEST_BYTES_V1: &[u8] =
    include_bytes!("canonical-source-v1.manifest");

const ORIGIN_DOMAIN_V1: &[u8] = b"FE2O3/PLIRON-SCALAR-ADD-V1/CANONICAL-ORIGIN/V1\0";
const SEMANTIC_DOMAIN_V1: &[u8] = b"FE2O3/PLIRON-SCALAR-ADD-V1/CANONICAL-SEMANTIC/V1\0";
const SCHEDULE_DOMAIN_V1: &[u8] = b"FE2O3/PLIRON-SCALAR-ADD-V1/CANONICAL-SCHEDULE/V1\0";
const TARGET_PLAN_DOMAIN_V1: &[u8] = b"FE2O3/PLIRON-SCALAR-ADD-V1/CANONICAL-TARGET-PLAN/V1\0";
const SOURCE_HEADER_V1: &str = "FE2O3_PLIRON_SCALAR_ADD_SOURCE_V1";
const SOURCE_KEYS_V1: &[&str] = &[
    "module",
    "kernel",
    "target",
    "code_object_version",
    "calling_convention",
    "parameter.0",
    "parameter.1",
    "parameter.2",
    "operation.0",
    "operation.1",
    "operation.2",
    "operation.3",
    "workgroup_size_range",
    "floating_point",
    "device_libraries",
];

struct ParsedCanonicalSourceV1<'a> {
    module: &'a str,
    kernel: &'a str,
    input: &'a str,
    output: &'a str,
    addend: &'a str,
    operations: [ScalarOperationV1; 4],
}

/// Deterministic identities observed from the checkout-embedded scalar source.
///
/// This is a non-authoritative observation. It does not sign, approve, load, or
/// execute anything and touches no HSA API. The checked-in approval manifest
/// separately pins these values and assumes repository and build provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalSourceObservationV1 {
    source: ContentIdentityV1,
    source_manifest: ContentIdentityV1,
    origin: [u8; 32],
    semantic: [u8; 32],
    schedule: [u8; 32],
    target_plan: [u8; 32],
    v2_handoff: [u8; 32],
    assembly: ContentIdentityV1,
    compiler_handoff: ContentIdentityV1,
    symbol_manifest: ContentIdentityV1,
}

impl CanonicalSourceObservationV1 {
    /// Returns the exact embedded source-file identity.
    pub const fn source_identity(&self) -> ContentIdentityV1 {
        self.source
    }

    /// Returns the exact embedded source-manifest identity.
    pub const fn source_manifest_identity(&self) -> ContentIdentityV1 {
        self.source_manifest
    }

    /// Returns the caller-origin identity deterministically derived from source bytes.
    pub const fn origin_identity(&self) -> &[u8; 32] {
        &self.origin
    }

    /// Returns the deterministic semantic-stage identity.
    pub const fn semantic_identity(&self) -> &[u8; 32] {
        &self.semantic
    }

    /// Returns the deterministic schedule-stage identity.
    pub const fn schedule_identity(&self) -> &[u8; 32] {
        &self.schedule
    }

    /// Returns the deterministic target-plan-stage identity.
    pub const fn target_plan_identity(&self) -> &[u8; 32] {
        &self.target_plan
    }

    /// Returns the exact canonical V2 handoff identity.
    pub const fn v2_handoff_identity(&self) -> &[u8; 32] {
        &self.v2_handoff
    }

    /// Returns the exact deterministic LLVM assembly identity.
    pub const fn assembly_identity(&self) -> ContentIdentityV1 {
        self.assembly
    }

    /// Returns the exact canonical compiler-handoff identity.
    pub const fn compiler_handoff_identity(&self) -> ContentIdentityV1 {
        self.compiler_handoff
    }

    /// Returns the exact two-symbol compiler-manifest identity.
    pub const fn symbol_manifest_identity(&self) -> ContentIdentityV1 {
        self.symbol_manifest
    }

    /// This observation grants no approval or execution authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Source observation does not initialize or call HSA.
    pub const fn hsa_touched(&self) -> bool {
        false
    }
}

/// Canonical prepared scalar-add lineage constructed only from embedded files.
///
/// The prepared value is inert. Moving it into the Worker V2 request builder
/// does not grant worker, publication, load, or launch authority.
pub struct CanonicalPreparedScalarAddV1 {
    observation: CanonicalSourceObservationV1,
    prepared: PreparedScalarAddWorkerV2,
}

impl fmt::Debug for CanonicalPreparedScalarAddV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalPreparedScalarAddV1")
            .field("observation", &self.observation)
            .finish_non_exhaustive()
    }
}

impl CanonicalPreparedScalarAddV1 {
    /// Returns the complete non-authoritative deterministic observation.
    pub const fn observation(&self) -> CanonicalSourceObservationV1 {
        self.observation
    }

    /// Consumes this wrapper and returns the inert prepared Worker V2 lineage.
    pub fn into_prepared(self) -> PreparedScalarAddWorkerV2 {
        self.prepared
    }
}

/// Constructs canonical scalar-add lineage from this checkout's embedded files.
pub fn canonical_prepared_scalar_add_v1()
-> Result<CanonicalPreparedScalarAddV1, CanonicalSourceErrorV1> {
    let source = canonical_scalar_module_v1()?;
    let origin = *source.origin_source_identity.as_bytes();
    let semantic = *source.stage_identities.semantic().as_bytes();
    let schedule = *source.stage_identities.schedule().as_bytes();
    let target_plan = *source.stage_identities.target_plan().as_bytes();
    let handoff = lower_scalar_kernel_v2(&source).map_err(|_| CanonicalSourceErrorV1::Lowering)?;
    let v2_handoff = *handoff.identity().as_bytes();
    let prepared =
        prepare_scalar_add_worker_v2(handoff).map_err(|_| CanonicalSourceErrorV1::Preparation)?;
    let compiler = prepared.compiler_handoff_identity();
    let symbols = prepared.manifest_identity();
    let observation = CanonicalSourceObservationV1 {
        source: ContentIdentityV1::calculate(CANONICAL_SOURCE_BYTES_V1),
        source_manifest: ContentIdentityV1::calculate(CANONICAL_SOURCE_MANIFEST_BYTES_V1),
        origin,
        semantic,
        schedule,
        target_plan,
        v2_handoff,
        assembly: prepared.assembly_content_identity(),
        compiler_handoff: ContentIdentityV1::from_parts(*compiler.sha256(), compiler.byte_len()),
        symbol_manifest: ContentIdentityV1::from_parts(*symbols.sha256(), symbols.byte_len()),
    };
    Ok(CanonicalPreparedScalarAddV1 {
        observation,
        prepared,
    })
}

/// Recomputes only the non-authoritative canonical source observation.
pub fn canonical_source_observation_v1()
-> Result<CanonicalSourceObservationV1, CanonicalSourceErrorV1> {
    canonical_prepared_scalar_add_v1().map(|prepared| prepared.observation())
}

fn canonical_scalar_module_v1() -> Result<ScalarKernelModuleV1, CanonicalSourceErrorV1> {
    let parsed = parse_canonical_source_v1(CANONICAL_SOURCE_BYTES_V1)?;
    let origin = identity(ORIGIN_DOMAIN_V1, &[CANONICAL_SOURCE_BYTES_V1])?;
    let semantic = identity(
        SEMANTIC_DOMAIN_V1,
        &[
            CANONICAL_SOURCE_MANIFEST_BYTES_V1,
            CANONICAL_SOURCE_BYTES_V1,
        ],
    )?;
    let schedule = identity(SCHEDULE_DOMAIN_V1, &[CANONICAL_SOURCE_MANIFEST_BYTES_V1])?;
    let target_plan = identity(
        TARGET_PLAN_DOMAIN_V1,
        &[CANONICAL_SOURCE_MANIFEST_BYTES_V1, b"gfx942:xnack-"],
    )?;
    let stages = StageIdentitiesV1::new(
        *semantic.as_bytes(),
        *schedule.as_bytes(),
        *target_plan.as_bytes(),
    )
    .map_err(|_| CanonicalSourceErrorV1::Identity)?;
    if parsed.kernel != SCALAR_ADD_KERNEL_SYMBOL_V1 {
        return Err(CanonicalSourceErrorV1::Syntax);
    }
    let mut module = ScalarKernelModuleV1::canonical(parsed.module, parsed.kernel, origin, stages);
    module.input_parameter = parsed.input.to_owned();
    module.output_parameter = parsed.output.to_owned();
    module.addend_parameter = parsed.addend.to_owned();
    module.operations = parsed.operations.to_vec();
    Ok(module)
}

fn parse_canonical_source_v1(
    bytes: &[u8],
) -> Result<ParsedCanonicalSourceV1<'_>, CanonicalSourceErrorV1> {
    let text = core::str::from_utf8(bytes).map_err(|_| CanonicalSourceErrorV1::Syntax)?;
    if !text.ends_with('\n') || text.as_bytes().contains(&b'\r') || text.as_bytes().contains(&0) {
        return Err(CanonicalSourceErrorV1::Syntax);
    }
    let lines = text
        .strip_suffix('\n')
        .ok_or(CanonicalSourceErrorV1::Syntax)?
        .split('\n')
        .collect::<Vec<_>>();
    if lines.len() != SOURCE_KEYS_V1.len() + 1 || lines[0] != SOURCE_HEADER_V1 {
        return Err(CanonicalSourceErrorV1::Syntax);
    }

    let mut values = Vec::with_capacity(SOURCE_KEYS_V1.len());
    for (line, expected_key) in lines[1..].iter().zip(SOURCE_KEYS_V1) {
        let (key, value) = line.split_once('=').ok_or(CanonicalSourceErrorV1::Syntax)?;
        if key != *expected_key
            || value.is_empty()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_graphic() || byte == b' ')
        {
            return Err(CanonicalSourceErrorV1::Syntax);
        }
        values.push(value);
    }

    let value = |key: &str| {
        SOURCE_KEYS_V1
            .iter()
            .position(|candidate| *candidate == key)
            .and_then(|index| values.get(index).copied())
            .ok_or(CanonicalSourceErrorV1::Syntax)
    };
    if value("target")? != "gfx942:xnack-"
        || value("code_object_version")? != "6"
        || value("calling_convention")? != "amdgpu_kernel"
        || value("workgroup_size_range")? != "1..64"
        || value("floating_point")? != "ieee_strict"
        || value("device_libraries")? != "none"
    {
        return Err(CanonicalSourceErrorV1::Syntax);
    }

    let input = parse_parameter(value("parameter.0")?, "global_ptr_f32")?;
    let output = parse_parameter(value("parameter.1")?, "global_ptr_f32")?;
    let addend = parse_parameter(value("parameter.2")?, "f32")?;
    let expected_operations = [
        format!("load_f32 {input} align=4"),
        format!("fadd_f32 operation.0 {addend}"),
        format!("store_f32 operation.1 {output} align=4"),
        "return_void".to_owned(),
    ];
    for (index, expected) in expected_operations.iter().enumerate() {
        if values[8 + index] != expected {
            return Err(CanonicalSourceErrorV1::Syntax);
        }
    }

    Ok(ParsedCanonicalSourceV1 {
        module: value("module")?,
        kernel: value("kernel")?,
        input,
        output,
        addend,
        operations: [
            ScalarOperationV1::LoadInputF32,
            ScalarOperationV1::AddAddendF32,
            ScalarOperationV1::StoreOutputF32,
            ScalarOperationV1::ReturnVoid,
        ],
    })
}

fn parse_parameter<'a>(
    value: &'a str,
    expected_type: &str,
) -> Result<&'a str, CanonicalSourceErrorV1> {
    let (name, parameter_type) = value
        .split_once(':')
        .ok_or(CanonicalSourceErrorV1::Syntax)?;
    if name.is_empty() || parameter_type != expected_type {
        return Err(CanonicalSourceErrorV1::Syntax);
    }
    Ok(name)
}

fn identity(domain: &[u8], parts: &[&[u8]]) -> Result<IdentityV1, CanonicalSourceErrorV1> {
    let mut digest = Sha256::new();
    digest.update(domain);
    for part in parts {
        digest.update((part.len() as u64).to_le_bytes());
        digest.update(part);
    }
    IdentityV1::new(digest.finalize().into()).map_err(|_| CanonicalSourceErrorV1::Identity)
}

/// Failure while constructing deterministic lineage from embedded source files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CanonicalSourceErrorV1 {
    /// The closed backend fixture was malformed or outside its exact profile.
    Syntax,
    /// A deterministic source or stage identity was invalid.
    Identity,
    /// Typed Pliron lowering rejected the canonical source.
    Lowering,
    /// Worker V2 handoff preparation rejected the canonical handoff.
    Preparation,
}

impl fmt::Display for CanonicalSourceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax => formatter.write_str("canonical scalar-add backend fixture is invalid"),
            Self::Identity => {
                formatter.write_str("canonical scalar-add source identity is invalid")
            }
            Self::Lowering => formatter.write_str("canonical scalar-add Pliron lowering failed"),
            Self::Preparation => {
                formatter.write_str("canonical scalar-add Worker V2 preparation failed")
            }
        }
    }
}

impl std::error::Error for CanonicalSourceErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_source_is_deterministic_and_non_authoritative() {
        let first = canonical_prepared_scalar_add_v1().unwrap();
        let second = canonical_source_observation_v1().unwrap();
        assert_eq!(first.observation(), second);
        assert!(!second.grants_authority());
        assert!(!second.hsa_touched());
        assert_ne!(second.source_identity().sha256(), &[0; 32]);
        assert_ne!(second.source_manifest_identity().sha256(), &[0; 32]);
        assert_ne!(second.v2_handoff_identity(), &[0; 32]);
        assert_ne!(second.assembly_identity().sha256(), &[0; 32]);
        assert_ne!(second.compiler_handoff_identity().sha256(), &[0; 32]);
        assert_ne!(second.symbol_manifest_identity().sha256(), &[0; 32]);
    }

    #[test]
    fn every_fixture_field_is_parsed_and_operation_mutations_reject() {
        let parsed = parse_canonical_source_v1(CANONICAL_SOURCE_BYTES_V1).unwrap();
        assert_eq!(parsed.module, "scalar_module");
        assert_eq!(parsed.kernel, "scalar_add");
        assert_eq!(parsed.input, "input");
        assert_eq!(parsed.output, "output");
        assert_eq!(parsed.addend, "addend");

        let source = core::str::from_utf8(CANONICAL_SOURCE_BYTES_V1).unwrap();
        for (from, to) in [
            ("load_f32 input align=4", "load_f32 output align=4"),
            ("fadd_f32 operation.0 addend", "fadd_f32 operation.0 input"),
            (
                "store_f32 operation.1 output align=4",
                "store_f32 operation.0 output align=4",
            ),
            ("operation.3=return_void", "operation.3=return_value"),
        ] {
            let hostile = source.replacen(from, to, 1);
            assert_eq!(
                parse_canonical_source_v1(hostile.as_bytes()).err(),
                Some(CanonicalSourceErrorV1::Syntax),
            );
        }
    }

    #[test]
    fn fixture_names_drive_the_typed_module() {
        let source = core::str::from_utf8(CANONICAL_SOURCE_BYTES_V1).unwrap();
        let renamed = source
            .replace("module=scalar_module", "module=renamed_module")
            .replace("parameter.0=input:", "parameter.0=src:")
            .replace("load_f32 input align=4", "load_f32 src align=4");
        let parsed = parse_canonical_source_v1(renamed.as_bytes()).unwrap();
        assert_eq!(parsed.module, "renamed_module");
        assert_eq!(parsed.input, "src");
    }
}
