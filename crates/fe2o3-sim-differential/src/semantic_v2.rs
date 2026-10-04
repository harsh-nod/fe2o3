use core::fmt;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CastKind, CheckedBinaryOperator,
    ComparePredicate, Constant, Function, IntegerSwitchCase, IntrinsicOperation, Kernel,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType,
    Signature, Terminator, Type, UnaryOp, ValueDef, ValueId, VerifiedCanonicalKernelIrV7,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    IndexWidthV1, ScalarBitsV1, SharedBufferV1, SimulationArgumentV1, SimulationErrorV1,
    SimulationExecutionErrorKindV1, SimulationLimitsV1, SimulationPreflightErrorV1,
    SimulationRequestV1, SimulationTargetV1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const SEMANTIC_DIFFERENTIAL_SCHEMA_V2: &str = "fe2o3-sim-semantic-differential-v2";
pub const SEMANTIC_DIFFERENTIAL_FAILURE_SCHEMA_V2: &str =
    "fe2o3-sim-semantic-differential-failure-v2";
pub const SEMANTIC_DIFFERENTIAL_CAPABILITIES_SCHEMA_V2: &str =
    "fe2o3-sim-semantic-differential-capabilities-v2";
pub const SEMANTIC_DIFFERENTIAL_REPLAY_SCHEMA_V2: &str =
    "fe2o3-sim-semantic-differential-replay-v2";

const WORK_ITEMS: usize = 8;
const WORKGROUP: [u32; 3] = [4, 1, 1];
const AMDGPU64_TARGET_NAME: &str = "amdgpu64-target-neutral";
const INDEX32_TARGET_NAME: &str = "little-endian-index32";

#[derive(Clone, Copy)]
struct ScalarProfile {
    name: &'static str,
    ty: ScalarType,
    bits: u16,
    signed: bool,
    target: SimulationTargetV1,
    target_name: &'static str,
}

const INTEGER_PROFILES: [ScalarProfile; 12] = [
    ScalarProfile {
        name: "i8",
        ty: ScalarType::I8,
        bits: 8,
        signed: true,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "i16",
        ty: ScalarType::I16,
        bits: 16,
        signed: true,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "i32",
        ty: ScalarType::I32,
        bits: 32,
        signed: true,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "i64",
        ty: ScalarType::I64,
        bits: 64,
        signed: true,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "i128",
        ty: ScalarType::I128,
        bits: 128,
        signed: true,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "u8",
        ty: ScalarType::U8,
        bits: 8,
        signed: false,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "u16",
        ty: ScalarType::U16,
        bits: 16,
        signed: false,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "u32",
        ty: ScalarType::U32,
        bits: 32,
        signed: false,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "u64",
        ty: ScalarType::U64,
        bits: 64,
        signed: false,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "u128",
        ty: ScalarType::U128,
        bits: 128,
        signed: false,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "index32",
        ty: ScalarType::Index,
        bits: 32,
        signed: false,
        target: SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
        target_name: INDEX32_TARGET_NAME,
    },
    ScalarProfile {
        name: "index64",
        ty: ScalarType::Index,
        bits: 64,
        signed: false,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
];

const FLOAT_PROFILES: [ScalarProfile; 4] = [
    ScalarProfile {
        name: "f16",
        ty: ScalarType::F16,
        bits: 16,
        signed: true,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "bf16",
        ty: ScalarType::Bf16,
        bits: 16,
        signed: true,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "f32",
        ty: ScalarType::F32,
        bits: 32,
        signed: true,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
    ScalarProfile {
        name: "f64",
        ty: ScalarType::F64,
        bits: 64,
        signed: true,
        target: SimulationTargetV1::amdgpu_64(),
        target_name: AMDGPU64_TARGET_NAME,
    },
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticDifferentialCapabilitiesV2 {
    pub schema: &'static str,
    pub authority: &'static str,
    pub simulated: bool,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
    pub target_profiles: Vec<&'static str>,
    pub integer_types: Vec<&'static str>,
    pub boolean_types: Vec<&'static str>,
    pub exact_float_types: Vec<&'static str>,
    pub covered_families: Vec<&'static str>,
    pub expected_rejection_families: Vec<&'static str>,
    pub exclusions: Vec<SemanticDifferentialExclusionV2>,
    pub case_limit: usize,
    pub lanes_per_positive_case: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticDifferentialExclusionV2 {
    pub code: &'static str,
    pub disposition: &'static str,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticDifferentialSuccessV2 {
    pub schema: &'static str,
    pub status: &'static str,
    pub evidence_origin: &'static str,
    pub authority: &'static str,
    pub simulated: bool,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
    pub kir_version: u8,
    pub target_profiles: Vec<&'static str>,
    pub seed: u64,
    pub cases: Vec<SemanticCaseEvidenceV2>,
    pub agreement_cases: usize,
    pub expected_rejections: usize,
    pub lanes_compared: usize,
    pub capability_sha256: String,
    pub suite_sha256: String,
    pub replay_contract: &'static str,
    pub reducer: SemanticReducerMetadataV2,
    pub exclusions: Vec<SemanticDifferentialExclusionV2>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticCaseEvidenceV2 {
    pub case_id: String,
    pub disposition: &'static str,
    pub scalar_type: Option<&'static str>,
    pub target_profile: &'static str,
    pub features: Vec<&'static str>,
    pub lanes: usize,
    pub kir_sha256: String,
    pub expected_sha256: String,
    pub observed_sha256: String,
    pub rejection_code: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticReducerMetadataV2 {
    pub strategy: &'static str,
    pub maximum_lane_candidates: usize,
    pub preserves_case_identity: bool,
    pub replay_requires_kir_identity: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticDifferentialFailureV2 {
    pub schema: &'static str,
    pub status: &'static str,
    pub authority: &'static str,
    pub seed: u64,
    pub case_id: String,
    pub failure_class: &'static str,
    pub message: String,
    pub kir_sha256: String,
    pub canonical_kir_v7_hex: String,
    pub expected_hex: String,
    pub observed_hex: String,
    pub reduction: SemanticReductionV2,
    pub replay: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticReductionV2 {
    pub strategy: &'static str,
    pub original_lanes: usize,
    pub retained_lane: Option<usize>,
    pub retained_byte_offset: Option<usize>,
    pub retained_byte_len: Option<usize>,
    pub predicate_evaluations: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticDifferentialReplayV2 {
    pub schema: &'static str,
    pub status: &'static str,
    pub authority: &'static str,
    pub seed: u64,
    pub case: SemanticCaseEvidenceV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticDifferentialErrorV2 {
    Kir(String),
    Admission(String),
    Simulation(String),
    UnknownCase(String),
    InvalidKirIdentity,
    KirIdentityMismatch { expected: String, actual: String },
    EvidenceEncoding(String),
    CorpusInvariant(String),
}

impl fmt::Display for SemanticDifferentialErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kir(message) => write!(formatter, "KIR construction failed: {message}"),
            Self::Admission(message) => write!(formatter, "simulator admission failed: {message}"),
            Self::Simulation(message) => write!(formatter, "simulator execution failed: {message}"),
            Self::UnknownCase(case) => {
                write!(formatter, "unknown semantic differential case {case:?}")
            }
            Self::InvalidKirIdentity => {
                formatter.write_str("KIR identity must be exactly 64 lowercase hexadecimal digits")
            }
            Self::KirIdentityMismatch { expected, actual } => write!(
                formatter,
                "semantic replay KIR identity mismatch: expected {expected}, observed {actual}"
            ),
            Self::EvidenceEncoding(message) => {
                write!(formatter, "semantic evidence encoding failed: {message}")
            }
            Self::CorpusInvariant(message) => {
                write!(formatter, "semantic corpus invariant failed: {message}")
            }
        }
    }
}

impl std::error::Error for SemanticDifferentialErrorV2 {}

struct CaseObservation {
    id: String,
    scalar_type: Option<&'static str>,
    target_profile: &'static str,
    features: Vec<&'static str>,
    lanes: usize,
    element_bytes: usize,
    canonical_kir: Vec<u8>,
    expected: Vec<u8>,
    observed: Vec<u8>,
    expected_rejection: Option<&'static str>,
    observed_rejection: Option<&'static str>,
}

pub fn semantic_differential_capabilities_v2() -> SemanticDifferentialCapabilitiesV2 {
    SemanticDifferentialCapabilitiesV2 {
        schema: SEMANTIC_DIFFERENTIAL_CAPABILITIES_SCHEMA_V2,
        authority: "none",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        target_profiles: vec![INDEX32_TARGET_NAME, AMDGPU64_TARGET_NAME],
        integer_types: INTEGER_PROFILES
            .iter()
            .map(|profile| profile.name)
            .collect(),
        boolean_types: vec!["bool"],
        exact_float_types: FLOAT_PROFILES.iter().map(|profile| profile.name).collect(),
        covered_families: vec![
            "checked_integer_add_and_overflow_select",
            "integer_bitwise",
            "boolean_logic_compare_and_select",
            "exact_finite_float_add",
            "global_load_gep_store",
            "conditional_branch_and_block_arguments",
            "typed_integer_switch",
            "internal_calls",
            "overlapping_shared_buffer_views",
        ],
        expected_rejection_families: vec![
            "shared_view_bounds",
            "dynamic_global_bounds",
            "uninitialized_global_read",
            "undefined_integer_division",
        ],
        exclusions: exclusions(),
        case_limit: 23,
        lanes_per_positive_case: WORK_ITEMS,
    }
}

pub fn run_semantic_differential_v2(
    seed: u64,
) -> Result<
    Result<SemanticDifferentialSuccessV2, SemanticDifferentialFailureV2>,
    SemanticDifferentialErrorV2,
> {
    let capabilities = semantic_differential_capabilities_v2();
    let capability_bytes = serde_json::to_vec(&capabilities)
        .map_err(|error| SemanticDifferentialErrorV2::EvidenceEncoding(error.to_string()))?;
    let observations = observations(seed)?;
    let mut evidence = Vec::with_capacity(observations.len());
    let mut suite = Sha256::new();
    suite.update(b"FE2O3/SIM-SEMANTIC-DIFFERENTIAL/V2\0");
    suite.update(seed.to_le_bytes());
    hash_field(&mut suite, &capability_bytes);
    let mut agreement_cases = 0;
    let mut expected_rejections = 0;
    let mut lanes_compared = 0;
    for observation in observations {
        if let Some(failure) = mismatch(seed, &observation) {
            return Ok(Err(failure));
        }
        let item = evidence_for(&observation);
        hash_field(&mut suite, item.case_id.as_bytes());
        hash_field(&mut suite, &observation.canonical_kir);
        hash_field(&mut suite, &observation.expected);
        hash_field(&mut suite, &observation.observed);
        hash_field(
            &mut suite,
            item.rejection_code.unwrap_or("agreement").as_bytes(),
        );
        if item.rejection_code.is_some() {
            expected_rejections += 1;
        } else {
            agreement_cases += 1;
            lanes_compared += item.lanes;
        }
        evidence.push(item);
    }
    Ok(Ok(SemanticDifferentialSuccessV2 {
        schema: SEMANTIC_DIFFERENTIAL_SCHEMA_V2,
        status: "agreement",
        evidence_origin: "independent_bounded_cpu_model_agreement",
        authority: "none",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        kir_version: 7,
        target_profiles: capabilities.target_profiles.clone(),
        seed,
        cases: evidence,
        agreement_cases,
        expected_rejections,
        lanes_compared,
        capability_sha256: hex_plain(&Sha256::digest(&capability_bytes)),
        suite_sha256: hex(&suite.finalize()),
        replay_contract: "semantic-replay-v2 requires exact seed, case ID, and canonical KIR SHA-256",
        reducer: reducer_metadata(),
        exclusions: capabilities.exclusions,
    }))
}

pub fn replay_semantic_differential_case_v2(
    seed: u64,
    case_id: &str,
    expected_kir_sha256: &str,
) -> Result<SemanticDifferentialReplayV2, SemanticDifferentialErrorV2> {
    if expected_kir_sha256.len() != 64
        || !expected_kir_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(SemanticDifferentialErrorV2::InvalidKirIdentity);
    }
    let observation = observations(seed)?
        .into_iter()
        .find(|observation| observation.id == case_id)
        .ok_or_else(|| SemanticDifferentialErrorV2::UnknownCase(case_id.to_owned()))?;
    let actual = hex_plain(&Sha256::digest(&observation.canonical_kir));
    if actual != expected_kir_sha256 {
        return Err(SemanticDifferentialErrorV2::KirIdentityMismatch {
            expected: expected_kir_sha256.to_owned(),
            actual,
        });
    }
    if let Some(failure) = mismatch(seed, &observation) {
        return Err(SemanticDifferentialErrorV2::Simulation(failure.message));
    }
    Ok(SemanticDifferentialReplayV2 {
        schema: SEMANTIC_DIFFERENTIAL_REPLAY_SCHEMA_V2,
        status: "reproduced",
        authority: "none",
        seed,
        case: evidence_for(&observation),
    })
}

fn observations(seed: u64) -> Result<Vec<CaseObservation>, SemanticDifferentialErrorV2> {
    let mut cases = Vec::with_capacity(23);
    for profile in INTEGER_PROFILES {
        cases.push(integer_case(seed, profile)?);
    }
    cases.push(boolean_case(seed)?);
    for profile in FLOAT_PROFILES {
        cases.push(float_case(seed, profile)?);
    }
    cases.push(cfg_call_case(seed)?);
    cases.push(alias_case(seed)?);
    cases.push(view_bounds_case(seed)?);
    cases.push(dynamic_bounds_case(seed)?);
    cases.push(uninitialized_read_case(seed)?);
    cases.push(undefined_division_case(seed)?);
    let capabilities = semantic_differential_capabilities_v2();
    if cases.len() != capabilities.case_limit {
        return Err(SemanticDifferentialErrorV2::CorpusInvariant(format!(
            "capability declares {} cases but corpus constructed {}",
            capabilities.case_limit,
            cases.len()
        )));
    }
    for (index, case) in cases.iter().enumerate() {
        if cases[..index].iter().any(|prior| prior.id == case.id) {
            return Err(SemanticDifferentialErrorV2::CorpusInvariant(format!(
                "duplicate case ID {:?}",
                case.id
            )));
        }
        if !capabilities.target_profiles.contains(&case.target_profile) {
            return Err(SemanticDifferentialErrorV2::CorpusInvariant(format!(
                "case {:?} has undeclared target profile {:?}",
                case.id, case.target_profile
            )));
        }
    }
    Ok(cases)
}

fn integer_case(
    seed: u64,
    profile: ScalarProfile,
) -> Result<CaseObservation, SemanticDifferentialErrorV2> {
    let (admitted, canonical_kir) = admit(integer_module(
        profile.ty,
        alignment(profile.ty, profile.target),
        &format!("integer-{}", profile.name),
    ))?;
    let mask = bit_mask(profile.bits);
    let mut random = SplitMix64::new(seed ^ type_seed(profile.name));
    let mut left = Vec::with_capacity(WORK_ITEMS);
    let mut right = Vec::with_capacity(WORK_ITEMS);
    let mut expected_bits = Vec::with_capacity(WORK_ITEMS);
    for lane in 0..WORK_ITEMS {
        let (a, b) = match lane {
            0 if profile.signed => ((1_u128 << (profile.bits - 1)) - 1, 1),
            0 => (mask, 1),
            1 => (1, 2),
            _ => (random.next_u128() & mask, random.next_u128() & mask),
        };
        let sum = a.wrapping_add(b) & mask;
        let overflow = add_overflow(a, b, profile.bits, profile.signed);
        let expected = if overflow { a ^ b } else { sum };
        left.push(scalar_for(profile.ty, a, profile.target)?);
        right.push(scalar_for(profile.ty, b, profile.target)?);
        expected_bits.push(scalar_for(profile.ty, expected, profile.target)?);
    }
    let target = profile.target;
    let request = SimulationRequestV1::new(
        "semantic_integer",
        [WORK_ITEMS as u64, 1, 1],
        WORKGROUP,
        vec![
            buffer(AccessMode::ReadOnly, &left, target)?,
            buffer(AccessMode::ReadOnly, &right, target)?,
            buffer(
                AccessMode::ReadWrite,
                &vec![scalar_for(profile.ty, 0, target)?; WORK_ITEMS],
                target,
            )?,
        ],
    );
    let execution = admitted
        .simulate(&request, target, SimulationLimitsV1::default())
        .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))?;
    let observed = execution
        .buffer(2)
        .expect("output argument")
        .bytes()
        .to_vec();
    Ok(CaseObservation {
        id: format!("integer-{}", profile.name),
        scalar_type: Some(profile.name),
        target_profile: profile.target_name,
        features: vec![
            "integer",
            "checked_add",
            "overflow",
            "bitwise",
            "select",
            "global_memory",
        ],
        lanes: WORK_ITEMS,
        element_bytes: bytes(profile.bits),
        canonical_kir,
        expected: encode_scalars(&expected_bits, profile.bits),
        observed,
        expected_rejection: None,
        observed_rejection: None,
    })
}

fn boolean_case(seed: u64) -> Result<CaseObservation, SemanticDifferentialErrorV2> {
    let (admitted, canonical_kir) = admit(boolean_module())?;
    let left = (0..WORK_ITEMS)
        .map(|lane| ScalarBitsV1::boolean(((seed >> (lane % 64)) ^ lane as u64) & 1 != 0))
        .collect::<Vec<_>>();
    let right = (0..WORK_ITEMS)
        .map(|lane| {
            ScalarBitsV1::boolean(
                ((seed.rotate_left(17) >> (lane % 64)) ^ (lane as u64 >> 1)) & 1 != 0,
            )
        })
        .collect::<Vec<_>>();
    let expected = left
        .iter()
        .zip(&right)
        .map(|(left, right)| {
            let xor = left.bits() ^ right.bits();
            let not_left = u128::from(left.bits() == 0);
            ScalarBitsV1::boolean(if xor == not_left {
                xor != 0
            } else {
                not_left != 0
            })
        })
        .collect::<Vec<_>>();
    let target = SimulationTargetV1::amdgpu_64();
    let request = SimulationRequestV1::new(
        "semantic_bool",
        [WORK_ITEMS as u64, 1, 1],
        WORKGROUP,
        vec![
            buffer(AccessMode::ReadOnly, &left, target)?,
            buffer(AccessMode::ReadOnly, &right, target)?,
            buffer(
                AccessMode::ReadWrite,
                &vec![ScalarBitsV1::boolean(false); WORK_ITEMS],
                target,
            )?,
        ],
    );
    let execution = admitted
        .simulate(&request, target, SimulationLimitsV1::default())
        .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))?;
    Ok(CaseObservation {
        id: "boolean-logic".to_owned(),
        scalar_type: Some("bool"),
        target_profile: AMDGPU64_TARGET_NAME,
        features: vec![
            "boolean",
            "bitwise",
            "unary_not",
            "compare",
            "select",
            "global_memory",
        ],
        lanes: WORK_ITEMS,
        element_bytes: 1,
        canonical_kir,
        expected: encode_scalars(&expected, 1),
        observed: execution
            .buffer(2)
            .expect("output argument")
            .bytes()
            .to_vec(),
        expected_rejection: None,
        observed_rejection: None,
    })
}

fn float_case(
    seed: u64,
    profile: ScalarProfile,
) -> Result<CaseObservation, SemanticDifferentialErrorV2> {
    let (pairs, sums) = exact_float_vectors(profile.ty);
    let rotation = (seed as usize) % pairs.len();
    let mut left = Vec::with_capacity(WORK_ITEMS);
    let mut right = Vec::with_capacity(WORK_ITEMS);
    let mut expected = Vec::with_capacity(WORK_ITEMS);
    for lane in 0..WORK_ITEMS {
        let index = (lane + rotation) % pairs.len();
        left.push(scalar(profile.ty, pairs[index].0)?);
        right.push(scalar(profile.ty, pairs[index].1)?);
        expected.push(scalar(profile.ty, sums[index])?);
    }
    let (admitted, canonical_kir) =
        admit(float_module(profile.ty, &format!("float-{}", profile.name)))?;
    let target = SimulationTargetV1::amdgpu_64();
    let request = SimulationRequestV1::new(
        "semantic_float",
        [WORK_ITEMS as u64, 1, 1],
        WORKGROUP,
        vec![
            buffer(AccessMode::ReadOnly, &left, target)?,
            buffer(AccessMode::ReadOnly, &right, target)?,
            buffer(
                AccessMode::ReadWrite,
                &vec![scalar(profile.ty, 0)?; WORK_ITEMS],
                target,
            )?,
        ],
    );
    let execution = admitted
        .simulate(&request, target, SimulationLimitsV1::default())
        .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))?;
    Ok(CaseObservation {
        id: format!("float-exact-add-{}", profile.name),
        scalar_type: Some(profile.name),
        target_profile: profile.target_name,
        features: vec!["float", "exact_finite_add", "global_memory"],
        lanes: WORK_ITEMS,
        element_bytes: bytes(profile.bits),
        canonical_kir,
        expected: encode_scalars(&expected, profile.bits),
        observed: execution
            .buffer(2)
            .expect("output argument")
            .bytes()
            .to_vec(),
        expected_rejection: None,
        observed_rejection: None,
    })
}

fn cfg_call_case(_seed: u64) -> Result<CaseObservation, SemanticDifferentialErrorV2> {
    let (admitted, canonical_kir) = admit(cfg_call_module())?;
    let target = SimulationTargetV1::amdgpu_64();
    let output = vec![ScalarBitsV1::u32(0); WORK_ITEMS];
    let request = SimulationRequestV1::new(
        "semantic_cfg_call",
        [WORK_ITEMS as u64, 1, 1],
        WORKGROUP,
        vec![buffer(AccessMode::ReadWrite, &output, target)?],
    );
    let expected = (0..WORK_ITEMS)
        .map(|lane| {
            let branch = if lane % 2 == 0 { 10_u32 } else { 20 };
            branch
                + match lane {
                    0 => 100,
                    1 => 200,
                    _ => 300,
                }
        })
        .map(ScalarBitsV1::u32)
        .collect::<Vec<_>>();
    let execution = admitted
        .simulate(&request, target, SimulationLimitsV1::default())
        .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))?;
    Ok(CaseObservation {
        id: "cfg-call-u32".to_owned(),
        scalar_type: Some("u32"),
        target_profile: AMDGPU64_TARGET_NAME,
        features: vec![
            "conditional_branch",
            "block_arguments",
            "integer_switch",
            "internal_call",
            "global_memory",
        ],
        lanes: WORK_ITEMS,
        element_bytes: 4,
        canonical_kir,
        expected: encode_scalars(&expected, 32),
        observed: execution
            .buffer(0)
            .expect("output argument")
            .bytes()
            .to_vec(),
        expected_rejection: None,
        observed_rejection: None,
    })
}

fn alias_case(seed: u64) -> Result<CaseObservation, SemanticDifferentialErrorV2> {
    let (admitted, canonical_kir) = admit(alias_module())?;
    let target = SimulationTargetV1::amdgpu_64();
    let backing_id = BufferBackingIdV1(7);
    let values = (0..WORK_ITEMS)
        .map(|lane| {
            let value = if lane == 0 {
                u32::MAX
            } else {
                (seed as u32).wrapping_add(lane as u32 * 17)
            };
            ScalarBitsV1::u32(value)
        })
        .collect::<Vec<_>>();
    let expected = values
        .iter()
        .map(|value| {
            ScalarBitsV1::u32(if value.bits() == u32::MAX as u128 {
                u32::MAX
            } else {
                value.bits() as u32 + 1
            })
        })
        .collect::<Vec<_>>();
    let shared = SharedBufferV1 {
        id: backing_id,
        buffer: BufferArgumentV1::from_scalars(AccessMode::ReadWrite, 4, &values, target)
            .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))?,
    };
    let arguments = vec![
        SimulationArgumentV1::BufferView(
            BufferViewArgumentV1::new(
                backing_id,
                ScalarType::U32,
                AccessMode::ReadOnly,
                4,
                0,
                WORK_ITEMS,
                target,
            )
            .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))?,
        ),
        SimulationArgumentV1::BufferView(
            BufferViewArgumentV1::new(
                backing_id,
                ScalarType::U32,
                AccessMode::ReadWrite,
                4,
                0,
                WORK_ITEMS,
                target,
            )
            .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))?,
        ),
    ];
    let request = SimulationRequestV1::new(
        "semantic_alias",
        [WORK_ITEMS as u64, 1, 1],
        WORKGROUP,
        arguments,
    )
    .with_shared_buffers(vec![shared]);
    let execution = admitted
        .simulate(&request, target, SimulationLimitsV1::default())
        .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))?;
    Ok(CaseObservation {
        id: "alias-overlapping-shared-u32".to_owned(),
        scalar_type: Some("u32"),
        target_profile: AMDGPU64_TARGET_NAME,
        features: vec![
            "shared_backing",
            "overlapping_views",
            "global_load",
            "global_store",
        ],
        lanes: WORK_ITEMS,
        element_bytes: 4,
        canonical_kir,
        expected: encode_scalars(&expected, 32),
        observed: execution
            .shared_buffer(backing_id)
            .expect("shared output")
            .bytes()
            .to_vec(),
        expected_rejection: None,
        observed_rejection: None,
    })
}

fn view_bounds_case(_seed: u64) -> Result<CaseObservation, SemanticDifferentialErrorV2> {
    let (admitted, canonical_kir) = admit(alias_module())?;
    let target = SimulationTargetV1::amdgpu_64();
    let backing_id = BufferBackingIdV1(11);
    let values = vec![ScalarBitsV1::u32(1); WORK_ITEMS];
    let request = SimulationRequestV1::new(
        "semantic_alias",
        [WORK_ITEMS as u64, 1, 1],
        WORKGROUP,
        vec![
            SimulationArgumentV1::BufferView(
                BufferViewArgumentV1::new(
                    backing_id,
                    ScalarType::U32,
                    AccessMode::ReadOnly,
                    4,
                    4,
                    WORK_ITEMS,
                    target,
                )
                .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))?,
            ),
            SimulationArgumentV1::BufferView(
                BufferViewArgumentV1::new(
                    backing_id,
                    ScalarType::U32,
                    AccessMode::ReadWrite,
                    4,
                    0,
                    WORK_ITEMS,
                    target,
                )
                .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))?,
            ),
        ],
    )
    .with_shared_buffers(vec![SharedBufferV1 {
        id: backing_id,
        buffer: BufferArgumentV1::from_scalars(AccessMode::ReadWrite, 4, &values, target)
            .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))?,
    }]);
    let observed = match admitted.simulate(&request, target, SimulationLimitsV1::default()) {
        Err(SimulationErrorV1::Preflight(SimulationPreflightErrorV1::BufferViewBounds {
            argument: 0,
        })) => Some("buffer_view_bounds"),
        Err(error) => {
            return Err(SemanticDifferentialErrorV2::Simulation(format!(
                "view bounds case returned {error:?}"
            )));
        }
        Ok(_) => None,
    };
    Ok(rejection_observation(
        "reject-shared-view-bounds",
        vec!["shared_backing", "bounds", "preflight_rejection"],
        canonical_kir,
        "buffer_view_bounds",
        observed,
    ))
}

fn dynamic_bounds_case(_seed: u64) -> Result<CaseObservation, SemanticDifferentialErrorV2> {
    let (admitted, canonical_kir) = admit(alias_module())?;
    let target = SimulationTargetV1::amdgpu_64();
    let one = vec![ScalarBitsV1::u32(1)];
    let zero = vec![ScalarBitsV1::u32(0)];
    let request = SimulationRequestV1::new(
        "semantic_alias",
        [2, 1, 1],
        [2, 1, 1],
        vec![
            buffer(AccessMode::ReadOnly, &one, target)?,
            buffer(AccessMode::ReadWrite, &zero, target)?,
        ],
    );
    let observed = match admitted.simulate(&request, target, SimulationLimitsV1::default()) {
        Err(SimulationErrorV1::Execution(error))
            if matches!(
                error.kind,
                SimulationExecutionErrorKindV1::OutOfBounds { .. }
            ) =>
        {
            Some("dynamic_out_of_bounds")
        }
        Err(error) => {
            return Err(SemanticDifferentialErrorV2::Simulation(format!(
                "dynamic bounds case returned {error:?}"
            )));
        }
        Ok(_) => None,
    };
    Ok(rejection_observation(
        "reject-dynamic-global-bounds",
        vec!["global_memory", "bounds", "execution_rejection"],
        canonical_kir,
        "dynamic_out_of_bounds",
        observed,
    ))
}

fn uninitialized_read_case(_seed: u64) -> Result<CaseObservation, SemanticDifferentialErrorV2> {
    let (admitted, canonical_kir) = admit(alias_module())?;
    let target = SimulationTargetV1::amdgpu_64();
    let input = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadOnly,
        4,
        vec![0; 4],
        vec![false; 4],
        target,
    )
    .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))?;
    let output = vec![ScalarBitsV1::u32(0)];
    let request = SimulationRequestV1::new(
        "semantic_alias",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(input),
            buffer(AccessMode::ReadWrite, &output, target)?,
        ],
    );
    let observed = match admitted.simulate(&request, target, SimulationLimitsV1::default()) {
        Err(SimulationErrorV1::Execution(error))
            if matches!(
                error.kind,
                SimulationExecutionErrorKindV1::UninitializedRead { .. }
            ) =>
        {
            Some("uninitialized_read")
        }
        Err(error) => {
            return Err(SemanticDifferentialErrorV2::Simulation(format!(
                "uninitialized case returned {error:?}"
            )));
        }
        Ok(_) => None,
    };
    Ok(rejection_observation(
        "reject-uninitialized-global-read",
        vec!["global_memory", "initialization", "execution_rejection"],
        canonical_kir,
        "uninitialized_read",
        observed,
    ))
}

fn undefined_division_case(_seed: u64) -> Result<CaseObservation, SemanticDifferentialErrorV2> {
    let (admitted, canonical_kir) = admit(undefined_division_module())?;
    let target = SimulationTargetV1::amdgpu_64();
    let output = vec![ScalarBitsV1::u32(0)];
    let request = SimulationRequestV1::new(
        "semantic_divide",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(7)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(0)),
            buffer(AccessMode::ReadWrite, &output, target)?,
        ],
    );
    let observed = match admitted.simulate(&request, target, SimulationLimitsV1::default()) {
        Err(SimulationErrorV1::Execution(error))
            if matches!(
                error.kind,
                SimulationExecutionErrorKindV1::UndefinedIntegerOperation("division by zero")
            ) =>
        {
            Some("undefined_integer_division_by_zero")
        }
        Err(error) => {
            return Err(SemanticDifferentialErrorV2::Simulation(format!(
                "division case returned {error:?}"
            )));
        }
        Ok(_) => None,
    };
    Ok(rejection_observation(
        "reject-u32-division-by-zero",
        vec!["integer", "undefined_operation", "execution_rejection"],
        canonical_kir,
        "undefined_integer_division_by_zero",
        observed,
    ))
}

fn integer_module(ty: ScalarType, scalar_alignment: u32, module_id: &str) -> Module {
    let pointer_read = pointer(ty, AccessMode::ReadOnly);
    let pointer_write = pointer(ty, AccessMode::ReadWrite);
    let operations = vec![
        one(
            3,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        one(
            4,
            pointer_read.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(3),
            },
        ),
        one(
            5,
            pointer_read.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(3),
            },
        ),
        one(
            6,
            Type::Scalar(ty),
            OperationKind::Load {
                pointer: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, scalar_alignment),
            },
        ),
        one(
            7,
            Type::Scalar(ty),
            OperationKind::Load {
                pointer: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Global, scalar_alignment),
            },
        ),
        Operation::checked_binary(
            ValueDef::new(ValueId(8), Type::Scalar(ty)),
            ValueDef::new(ValueId(9), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(6),
            ValueId(7),
        ),
        one(
            10,
            Type::Scalar(ty),
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(6),
                rhs: ValueId(7),
            },
        ),
        one(
            11,
            Type::Scalar(ty),
            OperationKind::Select {
                condition: ValueId(9),
                true_value: ValueId(10),
                false_value: ValueId(8),
            },
        ),
        one(
            12,
            pointer_write.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(2),
                offset: ValueId(3),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(12),
                value: ValueId(11),
                access: MemoryAccess::new(AddressSpace::Global, scalar_alignment),
            },
        ),
    ];
    single_kernel_module(
        module_id,
        "semantic_integer",
        vec![pointer_read.clone(), pointer_read, pointer_write],
        vec![ValueId(0), ValueId(1), ValueId(2)],
        operations,
    )
}

fn boolean_module() -> Module {
    let read = pointer(ScalarType::Bool, AccessMode::ReadOnly);
    let write = pointer(ScalarType::Bool, AccessMode::ReadWrite);
    let operations = vec![
        one(
            3,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        one(
            4,
            read.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(3),
            },
        ),
        one(
            5,
            read.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(3),
            },
        ),
        one(
            6,
            Type::BOOL,
            OperationKind::Load {
                pointer: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, 1),
            },
        ),
        one(
            7,
            Type::BOOL,
            OperationKind::Load {
                pointer: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Global, 1),
            },
        ),
        one(
            8,
            Type::BOOL,
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(6),
                rhs: ValueId(7),
            },
        ),
        one(
            9,
            Type::BOOL,
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(6),
            },
        ),
        one(
            10,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(8),
                rhs: ValueId(9),
            },
        ),
        one(
            11,
            Type::BOOL,
            OperationKind::Select {
                condition: ValueId(10),
                true_value: ValueId(8),
                false_value: ValueId(9),
            },
        ),
        one(
            12,
            write.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(2),
                offset: ValueId(3),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(12),
                value: ValueId(11),
                access: MemoryAccess::new(AddressSpace::Global, 1),
            },
        ),
    ];
    single_kernel_module(
        "boolean-logic",
        "semantic_bool",
        vec![read.clone(), read, write],
        vec![ValueId(0), ValueId(1), ValueId(2)],
        operations,
    )
}

fn float_module(ty: ScalarType, module_id: &str) -> Module {
    let read = pointer(ty, AccessMode::ReadOnly);
    let write = pointer(ty, AccessMode::ReadWrite);
    let access = MemoryAccess::new(
        AddressSpace::Global,
        alignment(ty, SimulationTargetV1::amdgpu_64()),
    );
    let operations = vec![
        one(
            3,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        one(
            4,
            read.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(3),
            },
        ),
        one(
            5,
            read.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(3),
            },
        ),
        one(
            6,
            Type::Scalar(ty),
            OperationKind::Load {
                pointer: ValueId(4),
                access,
            },
        ),
        one(
            7,
            Type::Scalar(ty),
            OperationKind::Load {
                pointer: ValueId(5),
                access,
            },
        ),
        one(
            8,
            Type::Scalar(ty),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(6),
                rhs: ValueId(7),
            },
        ),
        one(
            9,
            write.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(2),
                offset: ValueId(3),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(9),
                value: ValueId(8),
                access,
            },
        ),
    ];
    single_kernel_module(
        module_id,
        "semantic_float",
        vec![read.clone(), read, write],
        vec![ValueId(0), ValueId(1), ValueId(2)],
        operations,
    )
}

fn cfg_call_module() -> Module {
    let output = pointer(ScalarType::U32, AccessMode::ReadWrite);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        one(
            1,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        one(
            2,
            Type::Scalar(ScalarType::U64),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(1),
                to: Type::Scalar(ScalarType::U64),
            },
        ),
        one(
            3,
            Type::Scalar(ScalarType::U32),
            OperationKind::Cast {
                kind: CastKind::Truncate,
                value: ValueId(2),
                to: Type::Scalar(ScalarType::U32),
            },
        ),
        constant(4, Constant::U32(1)),
        one(
            5,
            Type::Scalar(ScalarType::U32),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(3),
                rhs: ValueId(4),
            },
        ),
        constant(6, Constant::U32(0)),
        one(
            7,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::NotEqual,
                lhs: ValueId(5),
                rhs: ValueId(6),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(1),
        else_arguments: vec![],
    });

    let mut even = BasicBlock::new(BlockId(1));
    even.operations.push(constant(8, Constant::U32(10)));
    even.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(8)],
    });
    let mut odd = BasicBlock::new(BlockId(2));
    odd.operations.push(constant(9, Constant::U32(20)));
    odd.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(9)],
    });

    let mut select = BasicBlock::new(BlockId(3));
    select
        .parameters
        .push(ValueDef::new(ValueId(10), Type::Scalar(ScalarType::U32)));
    select.terminator = Some(Terminator::IntegerSwitch {
        selector: ValueId(3),
        cases: vec![
            IntegerSwitchCase {
                value: Constant::U32(0),
                target: BlockId(4),
                arguments: vec![ValueId(10)],
            },
            IntegerSwitchCase {
                value: Constant::U32(1),
                target: BlockId(5),
                arguments: vec![ValueId(10)],
            },
        ],
        default_target: BlockId(6),
        default_arguments: vec![ValueId(10)],
    });
    let mut zero = route_block(4, 11, 12, 100);
    zero.terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![ValueId(11), ValueId(12)],
    });
    let mut one_block = route_block(5, 13, 14, 200);
    one_block.terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![ValueId(13), ValueId(14)],
    });
    let mut default = route_block(6, 15, 16, 300);
    default.terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![ValueId(15), ValueId(16)],
    });

    let mut finish = BasicBlock::new(BlockId(7));
    finish.parameters = vec![
        ValueDef::new(ValueId(17), Type::Scalar(ScalarType::U32)),
        ValueDef::new(ValueId(18), Type::Scalar(ScalarType::U32)),
    ];
    finish.operations = vec![
        one(
            19,
            Type::Scalar(ScalarType::U32),
            OperationKind::Call {
                callee: "semantic_add_helper".into(),
                arguments: vec![ValueId(17), ValueId(18)],
            },
        ),
        one(
            20,
            output.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(1),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(20),
                value: ValueId(19),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    finish.terminator = Some(Terminator::Return { values: vec![] });
    let kernel = Function::kernel_entry(
        "semantic_cfg_call_impl",
        Signature::new(vec![output], vec![]),
        vec![ValueId(0)],
        vec![entry, even, odd, select, zero, one_block, default, finish],
    );

    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.operations.push(Operation::checked_binary(
        ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        ValueDef::new(ValueId(3), Type::BOOL),
        CheckedBinaryOperator::Add,
        ValueId(0),
        ValueId(1),
    ));
    helper_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let helper = Function::internal_helper(
        "semantic_add_helper",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32); 2],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![helper_block],
    );
    let mut module = Module::new("cfg-call-u32");
    module.functions = vec![kernel, helper];
    module.kernels.push(Kernel::new(
        "semantic_cfg_call",
        "semantic_cfg_call_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn route_block(block: u32, parameter: u32, constant_id: u32, value: u32) -> BasicBlock {
    let mut result = BasicBlock::new(BlockId(block));
    result.parameters.push(ValueDef::new(
        ValueId(parameter),
        Type::Scalar(ScalarType::U32),
    ));
    result
        .operations
        .push(constant(constant_id, Constant::U32(value)));
    result
}

fn alias_module() -> Module {
    let read = pointer(ScalarType::U32, AccessMode::ReadOnly);
    let write = pointer(ScalarType::U32, AccessMode::ReadWrite);
    let operations = vec![
        one(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        one(
            3,
            read.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(2),
            },
        ),
        one(
            4,
            Type::Scalar(ScalarType::U32),
            OperationKind::Load {
                pointer: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        constant(5, Constant::U32(1)),
        Operation::checked_binary(
            ValueDef::new(ValueId(6), Type::Scalar(ScalarType::U32)),
            ValueDef::new(ValueId(7), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(4),
            ValueId(5),
        ),
        one(
            8,
            Type::Scalar(ScalarType::U32),
            OperationKind::Select {
                condition: ValueId(7),
                true_value: ValueId(4),
                false_value: ValueId(6),
            },
        ),
        one(
            9,
            write.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(2),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(9),
                value: ValueId(8),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    single_kernel_module(
        "alias-shared-u32",
        "semantic_alias",
        vec![read, write],
        vec![ValueId(0), ValueId(1)],
        operations,
    )
}

fn undefined_division_module() -> Module {
    let output = pointer(ScalarType::U32, AccessMode::ReadWrite);
    let operations = vec![
        one(
            3,
            Type::Scalar(ScalarType::U32),
            OperationKind::Binary {
                op: BinaryOp::Divide,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    single_kernel_module(
        "undefined-division",
        "semantic_divide",
        vec![
            Type::Scalar(ScalarType::U32),
            Type::Scalar(ScalarType::U32),
            output,
        ],
        vec![ValueId(0), ValueId(1), ValueId(2)],
        operations,
    )
}

fn single_kernel_module(
    module_id: &str,
    kernel_id: &str,
    parameters: Vec<Type>,
    parameter_values: Vec<ValueId>,
    operations: Vec<Operation>,
) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry_id = format!("{kernel_id}_impl");
    let entry = Function::kernel_entry(
        entry_id.clone(),
        Signature::new(parameters, vec![]),
        parameter_values,
        vec![block],
    );
    let mut module = Module::new(module_id);
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        kernel_id,
        entry_id,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn admit(
    module: Module,
) -> Result<(AdmittedSimulationModuleV1, Vec<u8>), SemanticDifferentialErrorV2> {
    let verified = VerifiedCanonicalKernelIrV7::from_module(module)
        .map_err(|error| SemanticDifferentialErrorV2::Kir(error.to_string()))?;
    let canonical = verified.canonical_bytes().to_vec();
    let admitted = AdmittedSimulationModuleV1::admit(verified, SimulationLimitsV1::default())
        .map_err(|error| SemanticDifferentialErrorV2::Admission(error.to_string()))?;
    Ok((admitted, canonical))
}

fn one(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn constant(id: u32, value: Constant) -> Operation {
    one(id, value.ty(), OperationKind::Constant(value))
}

fn pointer(ty: ScalarType, access: AccessMode) -> Type {
    Type::pointer(Type::Scalar(ty), AddressSpace::Global, access)
}

fn buffer(
    access: AccessMode,
    values: &[ScalarBitsV1],
    target: SimulationTargetV1,
) -> Result<SimulationArgumentV1, SemanticDifferentialErrorV2> {
    let alignment = values
        .first()
        .map(|value| alignment(value.ty(), target))
        .unwrap_or(1);
    BufferArgumentV1::from_scalars(access, alignment, values, target)
        .map(SimulationArgumentV1::Buffer)
        .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))
}

fn scalar(ty: ScalarType, bits: u128) -> Result<ScalarBitsV1, SemanticDifferentialErrorV2> {
    scalar_for(ty, bits, SimulationTargetV1::amdgpu_64())
}

fn scalar_for(
    ty: ScalarType,
    bits: u128,
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SemanticDifferentialErrorV2> {
    ScalarBitsV1::new(ty, bits, target)
        .map_err(|error| SemanticDifferentialErrorV2::Simulation(error.to_string()))
}

fn alignment(ty: ScalarType, target: SimulationTargetV1) -> u32 {
    match ty {
        ScalarType::Bool | ScalarType::I8 | ScalarType::U8 => 1,
        ScalarType::I16 | ScalarType::U16 | ScalarType::F16 | ScalarType::Bf16 => 2,
        ScalarType::I32 | ScalarType::U32 | ScalarType::F32 => 4,
        ScalarType::I64 | ScalarType::U64 | ScalarType::F64 => 8,
        ScalarType::I128 | ScalarType::U128 => 16,
        ScalarType::Index => match target.index_width() {
            IndexWidthV1::Bits32 => 4,
            IndexWidthV1::Bits64 => 8,
        },
    }
}

fn exact_float_vectors(ty: ScalarType) -> (&'static [(u128, u128)], &'static [u128]) {
    const F16_PAIRS: [(u128, u128); 4] = [
        (0x3c00, 0x4000),
        (0x3800, 0x3400),
        (0xbc00, 0x3c00),
        (0x4400, 0xc000),
    ];
    const F16_SUMS: [u128; 4] = [0x4200, 0x3a00, 0, 0x4000];
    const BF16_PAIRS: [(u128, u128); 4] = [
        (0x3f80, 0x4000),
        (0x3f00, 0x3e80),
        (0xbf80, 0x3f80),
        (0x4080, 0xc000),
    ];
    const BF16_SUMS: [u128; 4] = [0x4040, 0x3f40, 0, 0x4000];
    const F32_PAIRS: [(u128, u128); 4] = [
        (0x3f800000, 0x40000000),
        (0x3f000000, 0x3e800000),
        (0xbf800000, 0x3f800000),
        (0x40800000, 0xc0000000),
    ];
    const F32_SUMS: [u128; 4] = [0x40400000, 0x3f400000, 0, 0x40000000];
    const F64_PAIRS: [(u128, u128); 4] = [
        (0x3ff0000000000000, 0x4000000000000000),
        (0x3fe0000000000000, 0x3fd0000000000000),
        (0xbff0000000000000, 0x3ff0000000000000),
        (0x4010000000000000, 0xc000000000000000),
    ];
    const F64_SUMS: [u128; 4] = [
        0x4008000000000000,
        0x3fe8000000000000,
        0,
        0x4000000000000000,
    ];
    match ty {
        ScalarType::F16 => (&F16_PAIRS, &F16_SUMS),
        ScalarType::Bf16 => (&BF16_PAIRS, &BF16_SUMS),
        ScalarType::F32 => (&F32_PAIRS, &F32_SUMS),
        ScalarType::F64 => (&F64_PAIRS, &F64_SUMS),
        _ => unreachable!("float profile"),
    }
}

fn evidence_for(observation: &CaseObservation) -> SemanticCaseEvidenceV2 {
    SemanticCaseEvidenceV2 {
        case_id: observation.id.clone(),
        disposition: if observation.expected_rejection.is_some() {
            "expected_rejection"
        } else {
            "agreement"
        },
        scalar_type: observation.scalar_type,
        target_profile: observation.target_profile,
        features: observation.features.clone(),
        lanes: observation.lanes,
        kir_sha256: hex_plain(&Sha256::digest(&observation.canonical_kir)),
        expected_sha256: hex_plain(&Sha256::digest(&observation.expected)),
        observed_sha256: hex_plain(&Sha256::digest(&observation.observed)),
        rejection_code: observation.expected_rejection,
    }
}

fn mismatch(seed: u64, observation: &CaseObservation) -> Option<SemanticDifferentialFailureV2> {
    let rejection_matches = observation.expected_rejection == observation.observed_rejection;
    if observation.expected == observation.observed && rejection_matches {
        return None;
    }
    let retained = first_mismatch(
        &observation.expected,
        &observation.observed,
        observation.element_bytes,
    );
    let kir_sha256 = hex_plain(&Sha256::digest(&observation.canonical_kir));
    Some(SemanticDifferentialFailureV2 {
        schema: SEMANTIC_DIFFERENTIAL_FAILURE_SCHEMA_V2,
        status: "failure",
        authority: "none",
        seed,
        case_id: observation.id.clone(),
        failure_class: if rejection_matches {
            "output_mismatch"
        } else {
            "rejection_mismatch"
        },
        message: match (
            observation.expected_rejection,
            observation.observed_rejection,
        ) {
            (Some(expected), observed) if !rejection_matches => {
                format!("expected rejection {expected}, observed {observed:?}")
            }
            _ => "independent expected bytes differ from simulator output".to_owned(),
        },
        kir_sha256: kir_sha256.clone(),
        canonical_kir_v7_hex: hex(&observation.canonical_kir),
        expected_hex: hex(&observation.expected),
        observed_hex: hex(&observation.observed),
        reduction: SemanticReductionV2 {
            strategy: "first_mismatching_scalar_v1",
            original_lanes: observation.lanes,
            retained_lane: retained.map(|(lane, _)| lane),
            retained_byte_offset: retained.map(|(_, offset)| offset),
            retained_byte_len: retained.map(|_| observation.element_bytes),
            predicate_evaluations: observation.lanes.min(WORK_ITEMS),
        },
        replay: format!(
            "fe2o3-sim-differential semantic-replay-v2 --seed {seed} --case {} --kir-sha256 {kir_sha256}",
            observation.id
        ),
    })
}

fn first_mismatch(expected: &[u8], observed: &[u8], width: usize) -> Option<(usize, usize)> {
    let compared = expected.len().min(observed.len());
    for offset in (0..compared).step_by(width.max(1)) {
        let end = (offset + width).min(compared);
        if expected[offset..end] != observed[offset..end] {
            return Some((offset / width.max(1), offset));
        }
    }
    (expected.len() != observed.len()).then_some((compared / width.max(1), compared))
}

fn rejection_observation(
    id: &str,
    features: Vec<&'static str>,
    canonical_kir: Vec<u8>,
    expected: &'static str,
    observed: Option<&'static str>,
) -> CaseObservation {
    CaseObservation {
        id: id.to_owned(),
        scalar_type: Some("u32"),
        target_profile: AMDGPU64_TARGET_NAME,
        features,
        lanes: 0,
        element_bytes: 1,
        canonical_kir,
        expected: expected.as_bytes().to_vec(),
        observed: observed.unwrap_or("success").as_bytes().to_vec(),
        expected_rejection: Some(expected),
        observed_rejection: observed,
    }
}

fn reducer_metadata() -> SemanticReducerMetadataV2 {
    SemanticReducerMetadataV2 {
        strategy: "first_mismatching_scalar_v1",
        maximum_lane_candidates: WORK_ITEMS,
        preserves_case_identity: true,
        replay_requires_kir_identity: true,
    }
}

fn exclusions() -> Vec<SemanticDifferentialExclusionV2> {
    vec![
        SemanticDifferentialExclusionV2 {
            code: "float_rounding_edges",
            disposition: "not_covered",
            reason: "V2 admits only finite operations whose exact result bits are independently enumerated",
        },
        SemanticDifferentialExclusionV2 {
            code: "float_nonfinite",
            disposition: "not_covered",
            reason: "NaN payload, infinity, signed-zero, and subnormal edge matrices remain separate work",
        },
        SemanticDifferentialExclusionV2 {
            code: "float_transcendentals",
            disposition: "typed_unsupported",
            reason: "the simulator preflight rejects transcendental F32 imports",
        },
        SemanticDifferentialExclusionV2 {
            code: "memory_intrinsics",
            disposition: "typed_unsupported",
            reason: "the simulator preflight rejects generic memory intrinsics",
        },
        SemanticDifferentialExclusionV2 {
            code: "matrix_and_inline_assembly",
            disposition: "typed_unsupported",
            reason: "matrix operations, gfx950 LDS transpose, and inline assembly remain outside CPU simulation",
        },
        SemanticDifferentialExclusionV2 {
            code: "concurrent_wave_workgroup",
            disposition: "not_covered",
            reason: "wave, workgroup, barrier, atomic, fence, and schedule exploration have simulator tests but are outside this scalar/CFG differential corpus",
        },
        SemanticDifferentialExclusionV2 {
            code: "physical_gpu_parity",
            disposition: "not_observed",
            reason: "this command opens no GPU device and makes no hardware-equivalence claim",
        },
    ]
}

fn add_overflow(left: u128, right: u128, width: u16, signed: bool) -> bool {
    if !signed {
        return left
            .checked_add(right)
            .is_none_or(|value| value > bit_mask(width));
    }
    let left = signed_value(left, width);
    let right = signed_value(right, width);
    let Some(value) = left.checked_add(right) else {
        return true;
    };
    if width == 128 {
        false
    } else {
        let limit = 1_i128 << (width - 1);
        value < -limit || value >= limit
    }
}

fn signed_value(value: u128, width: u16) -> i128 {
    if width == 128 {
        value as i128
    } else {
        let shift = 128 - width;
        ((value << shift) as i128) >> shift
    }
}

fn bit_mask(width: u16) -> u128 {
    if width == 128 {
        u128::MAX
    } else {
        (1_u128 << width) - 1
    }
}

fn bytes(bits: u16) -> usize {
    usize::from(bits.max(8) / 8)
}

fn encode_scalars(values: &[ScalarBitsV1], bits: u16) -> Vec<u8> {
    let width = bytes(bits);
    let mut output = Vec::with_capacity(values.len() * width);
    for value in values {
        output.extend_from_slice(&value.bits().to_le_bytes()[..width]);
    }
    output
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn hex(bytes: &[u8]) -> String {
    format!("0x{}", hex_plain(bytes))
}

fn hex_plain(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        write!(&mut result, "{byte:02x}").expect("String writes are infallible");
    }
    result
}

fn type_seed(name: &str) -> u64 {
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/SIM-SEMANTIC-TYPE/V2\0");
    digest.update(name.as_bytes());
    u64::from_le_bytes(
        digest.finalize()[..8]
            .try_into()
            .expect("eight digest bytes"),
    )
}

struct SplitMix64(u64);

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn next_u128(&mut self) -> u128 {
        u128::from(self.next_u64()) | (u128::from(self.next_u64()) << 64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_semantic_suite_is_deterministic_and_covers_declared_types() {
        let first = run_semantic_differential_v2(0).unwrap().unwrap();
        let second = run_semantic_differential_v2(0).unwrap().unwrap();
        assert_eq!(first, second);
        assert_eq!(first.cases.len(), 23);
        assert_eq!(first.agreement_cases, 19);
        assert_eq!(first.expected_rejections, 4);
        assert_eq!(first.lanes_compared, 19 * WORK_ITEMS);
        assert_eq!(
            first.capability_sha256,
            "588cfc9745061b71849bd358526a62c2906ab23facfe3742c5420a1ff53b067f"
        );
        assert_eq!(
            first.suite_sha256,
            "0x36750b483a5056ca5f07f5443a7f1671150edcd6bf76b074651271d237de0d46"
        );
        for name in [
            "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "index64",
            "index32", "bool", "f16", "bf16", "f32", "f64",
        ] {
            assert!(
                first
                    .cases
                    .iter()
                    .any(|case| case.scalar_type == Some(name)),
                "missing {name}"
            );
        }
        assert_eq!(first.authority, "none");
        assert!(!first.hardware_observed);
        assert!(!first.performance_prediction);
        let capabilities = semantic_differential_capabilities_v2();
        assert_eq!(
            capabilities.target_profiles,
            [INDEX32_TARGET_NAME, AMDGPU64_TARGET_NAME]
        );
        let capabilities = serde_json::to_vec(&capabilities).unwrap();
        assert_eq!(
            first.capability_sha256,
            hex_plain(&Sha256::digest(capabilities))
        );
    }

    #[test]
    fn exact_replay_rejects_case_and_kir_substitution() {
        let report = run_semantic_differential_v2(7).unwrap().unwrap();
        let case = &report.cases[4];
        let replay =
            replay_semantic_differential_case_v2(7, &case.case_id, &case.kir_sha256).unwrap();
        assert_eq!(replay.case, *case);
        assert!(matches!(
            replay_semantic_differential_case_v2(7, "INTEGER-I128", &case.kir_sha256),
            Err(SemanticDifferentialErrorV2::UnknownCase(_))
        ));
        let mut substituted = case.kir_sha256.clone();
        substituted.replace_range(0..1, if &substituted[0..1] == "0" { "1" } else { "0" });
        assert!(matches!(
            replay_semantic_differential_case_v2(7, &case.case_id, &substituted),
            Err(SemanticDifferentialErrorV2::KirIdentityMismatch { .. })
        ));
        assert!(matches!(
            replay_semantic_differential_case_v2(7, &case.case_id, &case.kir_sha256.to_uppercase()),
            Err(SemanticDifferentialErrorV2::InvalidKirIdentity)
        ));
    }

    #[test]
    fn every_integer_profile_forces_overflow_and_non_overflow_vectors() {
        for profile in INTEGER_PROFILES {
            let overflow_left = if profile.signed {
                (1_u128 << (profile.bits - 1)) - 1
            } else {
                bit_mask(profile.bits)
            };
            assert!(
                add_overflow(overflow_left, 1, profile.bits, profile.signed),
                "{} overflow vector did not overflow",
                profile.name
            );
            assert!(
                !add_overflow(1, 2, profile.bits, profile.signed),
                "{} ordinary vector overflowed",
                profile.name
            );
        }
    }

    #[test]
    fn mismatch_reduction_selects_the_first_complete_scalar() {
        let mut observation = integer_case(3, INTEGER_PROFILES[2]).unwrap();
        observation.observed[5] ^= 1;
        observation.observed[9] ^= 1;
        let failure = mismatch(3, &observation).unwrap();
        assert_eq!(failure.failure_class, "output_mismatch");
        assert_eq!(failure.reduction.retained_lane, Some(1));
        assert_eq!(failure.reduction.retained_byte_offset, Some(4));
        assert_eq!(failure.reduction.retained_byte_len, Some(4));
        assert!(failure.replay.contains(&observation.id));
        assert!(failure.replay.contains(&failure.kir_sha256));
    }

    #[test]
    fn capability_contract_names_every_intentional_exclusion() {
        let capabilities = semantic_differential_capabilities_v2();
        assert_eq!(capabilities.case_limit, 23);
        assert_eq!(capabilities.integer_types.len(), 12);
        assert_eq!(
            capabilities.exact_float_types,
            ["f16", "bf16", "f32", "f64"]
        );
        assert!(
            capabilities
                .exclusions
                .iter()
                .any(|item| item.code == "physical_gpu_parity")
        );
        assert!(
            capabilities
                .exclusions
                .iter()
                .any(|item| item.code == "concurrent_wave_workgroup")
        );
    }
}
