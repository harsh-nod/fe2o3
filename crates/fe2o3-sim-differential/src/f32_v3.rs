use core::fmt;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, ComparePredicate, F32MathFunction,
    FloatOperation, Function, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MemoryAccess,
    Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type, UnaryOp, ValueDef,
    ValueId, VerifiedCanonicalKernelIrV7,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const F32_DIFFERENTIAL_CAPABILITIES_SCHEMA_V3: &str =
    "fe2o3-sim-f32-differential-capabilities-v3";
pub const F32_DIFFERENTIAL_SCHEMA_V3: &str = "fe2o3-sim-f32-differential-v3";
pub const F32_DIFFERENTIAL_FAILURE_SCHEMA_V3: &str = "fe2o3-sim-f32-differential-failure-v3";
pub const F32_DIFFERENTIAL_REPLAY_SCHEMA_V3: &str = "fe2o3-sim-f32-differential-replay-v3";

const TARGET_NAME: &str = "amdgpu64-target-neutral";
const WORKGROUP: [u32; 3] = [4, 1, 1];
const CASE_LIMIT: usize = 17;
const MAX_ROWS_PER_CASE: usize = 10;

const PZERO: u32 = 0x0000_0000;
const NZERO: u32 = 0x8000_0000;
const MIN_SUB: u32 = 0x0000_0001;
const NEG_MIN_SUB: u32 = 0x8000_0001;
const ONE: u32 = 0x3f80_0000;
const NEG_ONE: u32 = 0xbf80_0000;
const TWO: u32 = 0x4000_0000;
const NEG_TWO: u32 = 0xc000_0000;
const HALF: u32 = 0x3f00_0000;
const NEG_HALF: u32 = 0xbf00_0000;
const ONE_HALF: u32 = 0x3fc0_0000;
const TWO_HALF: u32 = 0x4020_0000;
const THREE: u32 = 0x4040_0000;
const FOUR: u32 = 0x4080_0000;
const MAX_FINITE: u32 = 0x7f7f_ffff;
const NEG_MAX_FINITE: u32 = 0xff7f_ffff;
const PINF: u32 = 0x7f80_0000;
const NINF: u32 = 0xff80_0000;
const QNAN: u32 = 0x7fc0_0042;
const NEG_QNAN: u32 = 0xffc0_0042;
const CANONICAL_QNAN: u32 = 0x7fc0_0000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct F32DifferentialCapabilitiesV3 {
    pub schema: &'static str,
    pub authority: &'static str,
    pub simulated: bool,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
    pub kir_version: u8,
    pub target_profile: &'static str,
    pub scalar_type: &'static str,
    pub admitted_operations: Vec<&'static str>,
    pub edge_classes: Vec<&'static str>,
    pub oracle_contract: &'static str,
    pub case_limit: usize,
    pub maximum_rows_per_case: usize,
    pub exclusions: Vec<F32DifferentialExclusionV3>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct F32DifferentialExclusionV3 {
    pub code: &'static str,
    pub disposition: &'static str,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct F32DifferentialSuccessV3 {
    pub schema: &'static str,
    pub status: &'static str,
    pub evidence_origin: &'static str,
    pub authority: &'static str,
    pub simulated: bool,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
    pub kir_version: u8,
    pub target_profile: &'static str,
    pub scalar_type: &'static str,
    pub operation_cases: usize,
    pub rows_compared: usize,
    pub cases: Vec<F32CaseEvidenceV3>,
    pub capability_sha256: String,
    pub suite_sha256: String,
    pub replay_contract: &'static str,
    pub reducer: F32ReducerMetadataV3,
    pub exclusions: Vec<F32DifferentialExclusionV3>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct F32CaseEvidenceV3 {
    pub case_id: &'static str,
    pub operation: &'static str,
    pub family: &'static str,
    pub edge_classes: Vec<&'static str>,
    pub row_ids: Vec<&'static str>,
    pub rows: usize,
    pub result_type: &'static str,
    pub kir_sha256: String,
    pub oracle_sha256: String,
    pub observed_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct F32ReducerMetadataV3 {
    pub strategy: &'static str,
    pub maximum_row_candidates: usize,
    pub preserves_case_identity: bool,
    pub replay_requires_kir_identity: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct F32DifferentialFailureV3 {
    pub schema: &'static str,
    pub status: &'static str,
    pub evidence_origin: &'static str,
    pub authority: &'static str,
    pub simulated: bool,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
    pub case_id: &'static str,
    pub operation: &'static str,
    pub failure_class: &'static str,
    pub message: String,
    pub kir_sha256: String,
    pub canonical_kir_v7_hex: String,
    pub oracle_hex: String,
    pub observed_hex: String,
    pub reduction: F32ReductionV3,
    pub replay: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct F32ReductionV3 {
    pub strategy: &'static str,
    pub original_rows: usize,
    pub retained_row: Option<usize>,
    pub retained_row_id: Option<&'static str>,
    pub retained_input_bits: Vec<String>,
    pub retained_expected_bits: Option<String>,
    pub retained_observed_bits: Option<String>,
    pub predicate_evaluations: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct F32DifferentialReplayV3 {
    pub schema: &'static str,
    pub status: &'static str,
    pub authority: &'static str,
    pub simulated: bool,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
    pub case: F32CaseEvidenceV3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum F32DifferentialErrorV3 {
    Kir(String),
    Admission(String),
    Simulation(String),
    UnknownCase(String),
    InvalidKirIdentity,
    KirIdentityMismatch { expected: String, actual: String },
    EvidenceEncoding(String),
    CorpusInvariant(String),
}

impl fmt::Display for F32DifferentialErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kir(message) => write!(formatter, "KIR construction failed: {message}"),
            Self::Admission(message) => write!(formatter, "simulator admission failed: {message}"),
            Self::Simulation(message) => write!(formatter, "simulator execution failed: {message}"),
            Self::UnknownCase(case) => write!(formatter, "unknown f32 differential case {case:?}"),
            Self::InvalidKirIdentity => {
                formatter.write_str("KIR identity must be exactly 64 lowercase hexadecimal digits")
            }
            Self::KirIdentityMismatch { expected, actual } => write!(
                formatter,
                "f32 replay KIR identity mismatch: expected {expected}, observed {actual}"
            ),
            Self::EvidenceEncoding(message) => {
                write!(formatter, "f32 evidence encoding failed: {message}")
            }
            Self::CorpusInvariant(message) => {
                write!(formatter, "f32 corpus invariant failed: {message}")
            }
        }
    }
}

impl std::error::Error for F32DifferentialErrorV3 {}

#[derive(Clone, Copy)]
enum CaseOperation {
    Unary(UnaryOp),
    Binary(BinaryOp),
    Compare(ComparePredicate),
    Math(F32MathFunction),
}

#[derive(Clone, Copy)]
struct OracleRow {
    id: &'static str,
    inputs: [u32; 3],
    expected: u32,
}

#[derive(Clone, Copy)]
struct CaseSpec {
    id: &'static str,
    operation_name: &'static str,
    family: &'static str,
    operation: CaseOperation,
    edge_classes: &'static [&'static str],
    rows: &'static [OracleRow],
}

struct CaseObservation {
    spec: CaseSpec,
    canonical_kir: Vec<u8>,
    expected: Vec<u8>,
    observed: Vec<u8>,
}

pub fn f32_differential_capabilities_v3() -> F32DifferentialCapabilitiesV3 {
    F32DifferentialCapabilitiesV3 {
        schema: F32_DIFFERENTIAL_CAPABILITIES_SCHEMA_V3,
        authority: "none",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        kir_version: 7,
        target_profile: TARGET_NAME,
        scalar_type: "f32",
        admitted_operations: case_specs()
            .iter()
            .map(|case| case.operation_name)
            .collect(),
        edge_classes: vec![
            "signed_zero",
            "subnormal",
            "infinity",
            "quiet_nan",
            "round_ties_even",
            "overflow",
            "unordered_compare",
            "fused_single_rounding",
            "exact_integral_rounding",
            "division_by_zero",
            "invalid_operation",
        ],
        oracle_contract: "compile-time exact IEEE-754 binary32 input/result bit tables; no host floating-point evaluation",
        case_limit: CASE_LIMIT,
        maximum_rows_per_case: MAX_ROWS_PER_CASE,
        exclusions: exclusions(),
    }
}

pub fn run_f32_differential_v3()
-> Result<Result<F32DifferentialSuccessV3, F32DifferentialFailureV3>, F32DifferentialErrorV3> {
    let capabilities = f32_differential_capabilities_v3();
    let capability_bytes = serde_json::to_vec(&capabilities)
        .map_err(|error| F32DifferentialErrorV3::EvidenceEncoding(error.to_string()))?;
    let observations = observations()?;
    let mut suite = Sha256::new();
    suite.update(b"FE2O3/SIM-F32-DIFFERENTIAL/V3\0");
    hash_field(&mut suite, &capability_bytes);
    let mut rows_compared = 0;
    let mut cases = Vec::with_capacity(observations.len());
    for observation in observations {
        if let Some(failure) = mismatch(&observation) {
            return Ok(Err(failure));
        }
        let evidence = evidence_for(&observation);
        hash_field(&mut suite, evidence.case_id.as_bytes());
        hash_field(&mut suite, &observation.canonical_kir);
        hash_field(&mut suite, &observation.expected);
        hash_field(&mut suite, &observation.observed);
        rows_compared += evidence.rows;
        cases.push(evidence);
    }
    Ok(Ok(F32DifferentialSuccessV3 {
        schema: F32_DIFFERENTIAL_SCHEMA_V3,
        status: "agreement",
        evidence_origin: "independent_exact_bit_table_agreement",
        authority: "none",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        kir_version: 7,
        target_profile: TARGET_NAME,
        scalar_type: "f32",
        operation_cases: cases.len(),
        rows_compared,
        cases,
        capability_sha256: hex_plain(&Sha256::digest(&capability_bytes)),
        suite_sha256: hex(&suite.finalize()),
        replay_contract: "f32-replay-v3 requires exact case ID and canonical KIR SHA-256",
        reducer: reducer_metadata(),
        exclusions: capabilities.exclusions,
    }))
}

pub fn replay_f32_differential_case_v3(
    case_id: &str,
    expected_kir_sha256: &str,
) -> Result<F32DifferentialReplayV3, F32DifferentialErrorV3> {
    if expected_kir_sha256.len() != 64
        || !expected_kir_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(F32DifferentialErrorV3::InvalidKirIdentity);
    }
    let observation = observations()?
        .into_iter()
        .find(|observation| observation.spec.id == case_id)
        .ok_or_else(|| F32DifferentialErrorV3::UnknownCase(case_id.to_owned()))?;
    let actual = hex_plain(&Sha256::digest(&observation.canonical_kir));
    if actual != expected_kir_sha256 {
        return Err(F32DifferentialErrorV3::KirIdentityMismatch {
            expected: expected_kir_sha256.to_owned(),
            actual,
        });
    }
    if let Some(failure) = mismatch(&observation) {
        return Err(F32DifferentialErrorV3::Simulation(failure.message));
    }
    Ok(F32DifferentialReplayV3 {
        schema: F32_DIFFERENTIAL_REPLAY_SCHEMA_V3,
        status: "reproduced",
        authority: "none",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        case: evidence_for(&observation),
    })
}

fn observations() -> Result<Vec<CaseObservation>, F32DifferentialErrorV3> {
    let specs = case_specs();
    if specs.len() != CASE_LIMIT {
        return Err(F32DifferentialErrorV3::CorpusInvariant(format!(
            "capability declares {CASE_LIMIT} cases but corpus constructed {}",
            specs.len()
        )));
    }
    let mut observations = Vec::with_capacity(specs.len());
    for (index, spec) in specs.iter().copied().enumerate() {
        if specs[..index].iter().any(|prior| prior.id == spec.id) {
            return Err(F32DifferentialErrorV3::CorpusInvariant(format!(
                "duplicate case ID {:?}",
                spec.id
            )));
        }
        if spec.rows.is_empty() || spec.rows.len() > MAX_ROWS_PER_CASE {
            return Err(F32DifferentialErrorV3::CorpusInvariant(format!(
                "case {:?} has {} rows outside 1..={MAX_ROWS_PER_CASE}",
                spec.id,
                spec.rows.len()
            )));
        }
        for (row_index, row) in spec.rows.iter().enumerate() {
            if spec.rows[..row_index]
                .iter()
                .any(|prior| prior.id == row.id)
            {
                return Err(F32DifferentialErrorV3::CorpusInvariant(format!(
                    "case {:?} has duplicate row ID {:?}",
                    spec.id, row.id
                )));
            }
        }
        observations.push(observe(spec)?);
    }
    Ok(observations)
}

fn observe(spec: CaseSpec) -> Result<CaseObservation, F32DifferentialErrorV3> {
    let (admitted, canonical_kir) = admit(case_module(spec))?;
    let target = SimulationTargetV1::amdgpu_64();
    let arity = operation_arity(spec.operation);
    let mut arguments = Vec::with_capacity(arity + 1);
    for input_index in 0..arity {
        let values = spec
            .rows
            .iter()
            .map(|row| f32_bits(row.inputs[input_index]))
            .collect::<Result<Vec<_>, _>>()?;
        arguments.push(buffer(AccessMode::ReadOnly, &values)?);
    }
    let output_type = result_type(spec.operation);
    let zero = if output_type == ScalarType::Bool {
        ScalarBitsV1::boolean(false)
    } else {
        f32_bits(0)?
    };
    arguments.push(buffer(AccessMode::ReadWrite, &vec![zero; spec.rows.len()])?);
    let request = SimulationRequestV1::new(
        spec.id,
        [spec.rows.len() as u64, 1, 1],
        WORKGROUP,
        arguments,
    );
    let execution = admitted
        .simulate(&request, target, SimulationLimitsV1::default())
        .map_err(|error| F32DifferentialErrorV3::Simulation(error.to_string()))?;
    let observed = execution
        .buffer(arity)
        .ok_or_else(|| F32DifferentialErrorV3::Simulation("missing output buffer".to_owned()))?
        .bytes()
        .to_vec();
    let expected = encode_expected(spec);
    Ok(CaseObservation {
        spec,
        canonical_kir,
        expected,
        observed,
    })
}

fn case_module(spec: CaseSpec) -> Module {
    let arity = operation_arity(spec.operation);
    let result = result_type(spec.operation);
    let read = f32_pointer(AccessMode::ReadOnly);
    let write = pointer(result, AccessMode::ReadWrite);
    let mut parameters = vec![read.clone(); arity];
    parameters.push(write.clone());
    let parameter_values = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect::<Vec<_>>();
    let mut next = parameters.len() as u32;
    let global_id = ValueId(next);
    next += 1;
    let mut operations = vec![one(
        global_id,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
    )];
    let mut inputs = Vec::with_capacity(arity);
    for input in 0..arity {
        let element = ValueId(next);
        next += 1;
        operations.push(one(
            element,
            read.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(input as u32),
                offset: global_id,
            },
        ));
        let value = ValueId(next);
        next += 1;
        operations.push(one(
            value,
            Type::F32,
            OperationKind::Load {
                pointer: element,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        inputs.push(value);
    }
    let result_id = ValueId(next);
    next += 1;
    let mut float_declaration = None;
    let result_operation = match spec.operation {
        CaseOperation::Unary(op) => one(
            result_id,
            Type::F32,
            OperationKind::Unary {
                op,
                operand: inputs[0],
            },
        ),
        CaseOperation::Binary(op) => one(
            result_id,
            Type::F32,
            OperationKind::Binary {
                op,
                lhs: inputs[0],
                rhs: inputs[1],
            },
        ),
        CaseOperation::Compare(predicate) => one(
            result_id,
            Type::BOOL,
            OperationKind::Compare {
                predicate,
                lhs: inputs[0],
                rhs: inputs[1],
            },
        ),
        CaseOperation::Math(function) => {
            let float = FloatOperation::F32Math {
                function,
                implementation: function.required_implementation(),
                arguments: inputs,
            };
            let operation = float.operation(result_id);
            float_declaration = Some(float.declaration());
            operation
        }
    };
    operations.push(result_operation);
    let output_pointer = ValueId(next);
    operations.push(one(
        output_pointer,
        write,
        OperationKind::GetElementPointer {
            base: ValueId(arity as u32),
            offset: global_id,
        },
    ));
    operations.push(Operation::new(
        Vec::new(),
        OperationKind::Store {
            pointer: output_pointer,
            value: result_id,
            access: MemoryAccess::new(
                AddressSpace::Global,
                if result == ScalarType::Bool { 1 } else { 4 },
            ),
        },
    ));
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry_id = format!("{}_impl", spec.id);
    let entry = Function::kernel_entry(
        entry_id.clone(),
        Signature::new(parameters, vec![]),
        parameter_values,
        vec![block],
    );
    let mut module = Module::new(format!("f32-differential::{}", spec.id));
    module.functions.push(entry);
    if let Some(declaration) = float_declaration {
        module.functions.push(declaration);
    }
    module.kernels.push(Kernel::new(
        spec.id,
        entry_id,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn admit(module: Module) -> Result<(AdmittedSimulationModuleV1, Vec<u8>), F32DifferentialErrorV3> {
    let verified = VerifiedCanonicalKernelIrV7::from_module(module)
        .map_err(|error| F32DifferentialErrorV3::Kir(error.to_string()))?;
    let canonical = verified.canonical_bytes().to_vec();
    let admitted = AdmittedSimulationModuleV1::admit(verified, SimulationLimitsV1::default())
        .map_err(|error| F32DifferentialErrorV3::Admission(error.to_string()))?;
    Ok((admitted, canonical))
}

fn one(id: ValueId, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(id, ty), kind)
}

fn f32_pointer(access: AccessMode) -> Type {
    pointer(ScalarType::F32, access)
}

fn pointer(ty: ScalarType, access: AccessMode) -> Type {
    Type::pointer(Type::Scalar(ty), AddressSpace::Global, access)
}

fn buffer(
    access: AccessMode,
    values: &[ScalarBitsV1],
) -> Result<SimulationArgumentV1, F32DifferentialErrorV3> {
    let alignment = if values
        .first()
        .is_some_and(|value| value.ty() == ScalarType::Bool)
    {
        1
    } else {
        4
    };
    BufferArgumentV1::from_scalars(access, alignment, values, SimulationTargetV1::amdgpu_64())
        .map(SimulationArgumentV1::Buffer)
        .map_err(|error| F32DifferentialErrorV3::Simulation(error.to_string()))
}

fn f32_bits(bits: u32) -> Result<ScalarBitsV1, F32DifferentialErrorV3> {
    ScalarBitsV1::new(
        ScalarType::F32,
        u128::from(bits),
        SimulationTargetV1::amdgpu_64(),
    )
    .map_err(|error| F32DifferentialErrorV3::Simulation(error.to_string()))
}

fn operation_arity(operation: CaseOperation) -> usize {
    match operation {
        CaseOperation::Unary(_) => 1,
        CaseOperation::Binary(_) | CaseOperation::Compare(_) => 2,
        CaseOperation::Math(function) => function.arity(),
    }
}

fn result_type(operation: CaseOperation) -> ScalarType {
    match operation {
        CaseOperation::Compare(_) => ScalarType::Bool,
        _ => ScalarType::F32,
    }
}

fn encode_expected(spec: CaseSpec) -> Vec<u8> {
    if result_type(spec.operation) == ScalarType::Bool {
        spec.rows
            .iter()
            .map(|row| u8::from(row.expected != 0))
            .collect()
    } else {
        spec.rows
            .iter()
            .flat_map(|row| row.expected.to_le_bytes())
            .collect()
    }
}

fn evidence_for(observation: &CaseObservation) -> F32CaseEvidenceV3 {
    F32CaseEvidenceV3 {
        case_id: observation.spec.id,
        operation: observation.spec.operation_name,
        family: observation.spec.family,
        edge_classes: observation.spec.edge_classes.to_vec(),
        row_ids: observation.spec.rows.iter().map(|row| row.id).collect(),
        rows: observation.spec.rows.len(),
        result_type: if result_type(observation.spec.operation) == ScalarType::Bool {
            "bool"
        } else {
            "f32"
        },
        kir_sha256: hex_plain(&Sha256::digest(&observation.canonical_kir)),
        oracle_sha256: hex_plain(&Sha256::digest(&observation.expected)),
        observed_sha256: hex_plain(&Sha256::digest(&observation.observed)),
    }
}

fn mismatch(observation: &CaseObservation) -> Option<F32DifferentialFailureV3> {
    if observation.expected == observation.observed {
        return None;
    }
    let width = if result_type(observation.spec.operation) == ScalarType::Bool {
        1
    } else {
        4
    };
    let retained = first_mismatch(&observation.expected, &observation.observed, width);
    let row = retained.and_then(|index| observation.spec.rows.get(index));
    let observed_bits = retained.and_then(|index| {
        let start = index.checked_mul(width)?;
        let bytes = observation.observed.get(start..start + width)?;
        Some(if width == 1 {
            format!("0x{:02x}", bytes[0])
        } else {
            format!(
                "0x{:08x}",
                u32::from_le_bytes(bytes.try_into().expect("four-byte f32 result"))
            )
        })
    });
    let kir_sha256 = hex_plain(&Sha256::digest(&observation.canonical_kir));
    Some(F32DifferentialFailureV3 {
        schema: F32_DIFFERENTIAL_FAILURE_SCHEMA_V3,
        status: "failure",
        evidence_origin: "independent_exact_bit_table_mismatch",
        authority: "none",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        case_id: observation.spec.id,
        operation: observation.spec.operation_name,
        failure_class: "output_mismatch",
        message: "independent exact-bit oracle differs from simulator output".to_owned(),
        kir_sha256: kir_sha256.clone(),
        canonical_kir_v7_hex: hex(&observation.canonical_kir),
        oracle_hex: hex(&observation.expected),
        observed_hex: hex(&observation.observed),
        reduction: F32ReductionV3 {
            strategy: "first_mismatching_oracle_row_v1",
            original_rows: observation.spec.rows.len(),
            retained_row: retained,
            retained_row_id: row.map(|row| row.id),
            retained_input_bits: row
                .map(|row| {
                    row.inputs[..operation_arity(observation.spec.operation)]
                        .iter()
                        .map(|bits| format!("0x{bits:08x}"))
                        .collect()
                })
                .unwrap_or_default(),
            retained_expected_bits: row.map(|row| {
                if width == 1 {
                    format!("0x{:02x}", u8::from(row.expected != 0))
                } else {
                    format!("0x{:08x}", row.expected)
                }
            }),
            retained_observed_bits: observed_bits,
            predicate_evaluations: retained.map_or(observation.spec.rows.len(), |row| row + 1),
        },
        replay: format!(
            "fe2o3-sim-differential f32-replay-v3 --case {} --kir-sha256 {kir_sha256}",
            observation.spec.id
        ),
    })
}

fn first_mismatch(expected: &[u8], observed: &[u8], width: usize) -> Option<usize> {
    let compared = expected.len().min(observed.len());
    for offset in (0..compared).step_by(width) {
        let end = (offset + width).min(compared);
        if expected[offset..end] != observed[offset..end] {
            return Some(offset / width);
        }
    }
    (expected.len() != observed.len()).then_some(compared / width)
}

fn reducer_metadata() -> F32ReducerMetadataV3 {
    F32ReducerMetadataV3 {
        strategy: "first_mismatching_oracle_row_v1",
        maximum_row_candidates: MAX_ROWS_PER_CASE,
        preserves_case_identity: true,
        replay_requires_kir_identity: true,
    }
}

fn exclusions() -> Vec<F32DifferentialExclusionV3> {
    vec![
        F32DifferentialExclusionV3 {
            code: "float_conversions_and_casts",
            disposition: "not_covered",
            reason: "cross-width and integer/float conversions require a separate typed source/result matrix",
        },
        F32DifferentialExclusionV3 {
            code: "f16_bf16_f64_edge_matrices",
            disposition: "not_covered",
            reason: "this bounded V3 matrix qualifies only binary32 core scalar operations",
        },
        F32DifferentialExclusionV3 {
            code: "sqrt_and_transcendentals",
            disposition: "typed_unsupported",
            reason: "simulator preflight rejects F32 functions without an admitted executable numerical contract",
        },
        F32DifferentialExclusionV3 {
            code: "fast_math_and_contraction",
            disposition: "not_admitted",
            reason: "the matrix exercises strict operations and explicit FMA only; it does not infer target fast-math behavior",
        },
        F32DifferentialExclusionV3 {
            code: "physical_gpu_parity",
            disposition: "not_observed",
            reason: "the command opens no GPU device and grants no compiler, ISA, runtime, or hardware authority",
        },
        F32DifferentialExclusionV3 {
            code: "performance_prediction",
            disposition: "out_of_scope",
            reason: "CPU execution time is not measured and provides no GPU performance evidence",
        },
    ]
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

const NEGATE_ROWS: [OracleRow; 8] = [
    row1("positive-zero", PZERO, NZERO),
    row1("negative-zero", NZERO, PZERO),
    row1("minimum-subnormal", MIN_SUB, NEG_MIN_SUB),
    row1("negative-minimum-subnormal", NEG_MIN_SUB, MIN_SUB),
    row1("positive-infinity", PINF, NINF),
    row1("negative-infinity", NINF, PINF),
    row1("quiet-nan-payload", QNAN, NEG_QNAN),
    row1("finite", ONE, NEG_ONE),
];

const ADD_ROWS: [OracleRow; 9] = [
    row2("finite-exact", ONE, ONE, TWO),
    row2("minimum-subnormal", MIN_SUB, MIN_SUB, 0x0000_0002),
    row2("half-ulp-tie-to-lower-even", ONE, 0x3380_0000, ONE),
    row2(
        "half-ulp-tie-to-upper-even",
        0x3f80_0001,
        0x3380_0000,
        0x3f80_0002,
    ),
    row2("opposite-infinities", PINF, NINF, CANONICAL_QNAN),
    row2("quiet-nan-payload", QNAN, ONE, QNAN),
    row2("negative-zero", NZERO, NZERO, NZERO),
    row2("mixed-signed-zero", PZERO, NZERO, PZERO),
    row2("finite-overflow", MAX_FINITE, MAX_FINITE, PINF),
];

const SUBTRACT_ROWS: [OracleRow; 9] = [
    row2("finite-exact", TWO, ONE, ONE),
    row2("positive-zero", PZERO, PZERO, PZERO),
    row2("negative-zero", NZERO, PZERO, NZERO),
    row2("subnormal-cancellation", MIN_SUB, MIN_SUB, PZERO),
    row2("lower-neighbor", ONE, 0x3380_0000, 0x3f7f_ffff),
    row2("same-infinities", PINF, PINF, CANONICAL_QNAN),
    row2("quiet-nan-payload", QNAN, ONE, QNAN),
    row2("positive-overflow", MAX_FINITE, NEG_MAX_FINITE, PINF),
    row2("negative-overflow", NEG_MAX_FINITE, MAX_FINITE, NINF),
];

const MULTIPLY_ROWS: [OracleRow; 9] = [
    row2("finite-exact", TWO, TWO, FOUR),
    row2("minimum-subnormal", MIN_SUB, TWO, 0x0000_0002),
    row2("positive-overflow", MAX_FINITE, TWO, PINF),
    row2("underflow-half-ulp-tie-even", MIN_SUB, HALF, PZERO),
    row2("infinity-times-zero", PINF, PZERO, CANONICAL_QNAN),
    row2("quiet-nan-payload", QNAN, ONE, QNAN),
    row2("negative-zero", NZERO, TWO, NZERO),
    row2("positive-zero-from-signs", NZERO, NEG_TWO, PZERO),
    row2("finite-sign", ONE_HALF, NEG_TWO, 0xc040_0000),
];

const DIVIDE_ROWS: [OracleRow; 9] = [
    row2("finite-exact", TWO, TWO, ONE),
    row2("underflow-half-ulp-tie-even", MIN_SUB, TWO, PZERO),
    row2("finite-half", ONE, TWO, HALF),
    row2("positive-division-by-zero", ONE, PZERO, PINF),
    row2("negative-division-by-zero", NEG_ONE, PZERO, NINF),
    row2("zero-by-zero", PZERO, PZERO, CANONICAL_QNAN),
    row2("infinity-by-infinity", PINF, PINF, CANONICAL_QNAN),
    row2("quiet-nan-payload", QNAN, ONE, QNAN),
    row2("negative-zero", NZERO, TWO, NZERO),
];

const REMAINDER_ROWS: [OracleRow; 9] = [
    row2("finite-positive", TWO_HALF, TWO, HALF),
    row2("finite-negative", 0xc020_0000, TWO, NEG_HALF),
    row2("positive-zero-result", TWO, TWO, PZERO),
    row2("negative-zero-result", NEG_TWO, TWO, NZERO),
    row2("minimum-subnormal", MIN_SUB, TWO, MIN_SUB),
    row2("infinity-dividend", PINF, ONE, CANONICAL_QNAN),
    row2("zero-divisor", ONE, PZERO, CANONICAL_QNAN),
    row2("quiet-nan-payload", QNAN, ONE, QNAN),
    row2("finite-exact", THREE, TWO, ONE),
];

const COMPARE_INPUTS: [[u32; 2]; 8] = [
    [PZERO, NZERO],
    [NEG_ONE, PZERO],
    [PINF, MAX_FINITE],
    [NINF, NEG_MAX_FINITE],
    [QNAN, ONE],
    [MIN_SUB, PZERO],
    [ONE, ONE],
    [NEG_MIN_SUB, PZERO],
];

const EQUAL_RESULTS: [u32; 8] = [1, 0, 0, 0, 0, 0, 1, 0];
const NOT_EQUAL_RESULTS: [u32; 8] = [0, 1, 1, 1, 1, 1, 0, 1];
const LESS_RESULTS: [u32; 8] = [0, 1, 0, 1, 0, 0, 0, 1];
const LESS_EQUAL_RESULTS: [u32; 8] = [1, 1, 0, 1, 0, 0, 1, 1];
const GREATER_RESULTS: [u32; 8] = [0, 0, 1, 0, 0, 1, 0, 0];
const GREATER_EQUAL_RESULTS: [u32; 8] = [1, 0, 1, 0, 0, 1, 1, 0];

const EQUAL_ROWS: [OracleRow; 8] = compare_rows(EQUAL_RESULTS);
const NOT_EQUAL_ROWS: [OracleRow; 8] = compare_rows(NOT_EQUAL_RESULTS);
const LESS_ROWS: [OracleRow; 8] = compare_rows(LESS_RESULTS);
const LESS_EQUAL_ROWS: [OracleRow; 8] = compare_rows(LESS_EQUAL_RESULTS);
const GREATER_ROWS: [OracleRow; 8] = compare_rows(GREATER_RESULTS);
const GREATER_EQUAL_ROWS: [OracleRow; 8] = compare_rows(GREATER_EQUAL_RESULTS);

const FMA_ROWS: [OracleRow; 8] = [
    row3("finite-exact", ONE, TWO, ONE, THREE),
    row3(
        "fused-single-rounding",
        0x3f80_0001,
        0x3f7f_fffe,
        NEG_ONE,
        0xa880_0000,
    ),
    row3("minimum-subnormal", MIN_SUB, ONE, MIN_SUB, 0x0000_0002),
    row3("finite-overflow", MAX_FINITE, TWO, PZERO, PINF),
    row3("infinity-times-zero", PINF, PZERO, ONE, CANONICAL_QNAN),
    row3("quiet-nan-payload", QNAN, ONE, PZERO, QNAN),
    row3("negative-zero", NZERO, TWO, NZERO, NZERO),
    row3("underflow-half-ulp-tie-even", MIN_SUB, HALF, PZERO, PZERO),
];

const INTEGRAL_INPUTS: [u32; 10] = [
    PZERO,
    NZERO,
    MIN_SUB,
    NEG_MIN_SUB,
    HALF,
    NEG_HALF,
    ONE_HALF,
    TWO_HALF,
    PINF,
    QNAN,
];

const FLOOR_RESULTS: [u32; 10] = [
    PZERO, NZERO, PZERO, NEG_ONE, PZERO, NEG_ONE, ONE, TWO, PINF, QNAN,
];
const CEIL_RESULTS: [u32; 10] = [PZERO, NZERO, ONE, NZERO, ONE, NZERO, TWO, THREE, PINF, QNAN];
const TRUNCATE_RESULTS: [u32; 10] = [
    PZERO, NZERO, PZERO, NZERO, PZERO, NZERO, ONE, TWO, PINF, QNAN,
];
const ROUND_EVEN_RESULTS: [u32; 10] = [
    PZERO, NZERO, PZERO, NZERO, PZERO, NZERO, TWO, TWO, PINF, QNAN,
];

const FLOOR_ROWS: [OracleRow; 10] = integral_rows(FLOOR_RESULTS);
const CEIL_ROWS: [OracleRow; 10] = integral_rows(CEIL_RESULTS);
const TRUNCATE_ROWS: [OracleRow; 10] = integral_rows(TRUNCATE_RESULTS);
const ROUND_EVEN_ROWS: [OracleRow; 10] = integral_rows(ROUND_EVEN_RESULTS);

const fn row1(id: &'static str, input: u32, expected: u32) -> OracleRow {
    OracleRow {
        id,
        inputs: [input, 0, 0],
        expected,
    }
}

const fn row2(id: &'static str, left: u32, right: u32, expected: u32) -> OracleRow {
    OracleRow {
        id,
        inputs: [left, right, 0],
        expected,
    }
}

const fn row3(id: &'static str, first: u32, second: u32, third: u32, expected: u32) -> OracleRow {
    OracleRow {
        id,
        inputs: [first, second, third],
        expected,
    }
}

const fn compare_rows(results: [u32; 8]) -> [OracleRow; 8] {
    let ids = [
        "signed-zero",
        "negative-finite",
        "positive-infinity",
        "negative-infinity",
        "unordered-quiet-nan",
        "positive-subnormal",
        "equal-finite",
        "negative-subnormal",
    ];
    let mut rows = [row2("", 0, 0, 0); 8];
    let mut index = 0;
    while index < rows.len() {
        rows[index] = row2(
            ids[index],
            COMPARE_INPUTS[index][0],
            COMPARE_INPUTS[index][1],
            results[index],
        );
        index += 1;
    }
    rows
}

const fn integral_rows(results: [u32; 10]) -> [OracleRow; 10] {
    let ids = [
        "positive-zero",
        "negative-zero",
        "positive-subnormal",
        "negative-subnormal",
        "positive-half",
        "negative-half",
        "one-and-half",
        "two-and-half",
        "positive-infinity",
        "quiet-nan-payload",
    ];
    let mut rows = [row1("", 0, 0); 10];
    let mut index = 0;
    while index < rows.len() {
        rows[index] = row1(ids[index], INTEGRAL_INPUTS[index], results[index]);
        index += 1;
    }
    rows
}

fn case_specs() -> [CaseSpec; CASE_LIMIT] {
    const COMPARE_EDGES: &[&str] = &[
        "signed_zero",
        "subnormal",
        "infinity",
        "quiet_nan",
        "unordered_compare",
    ];
    const INTEGRAL_EDGES: &[&str] = &[
        "signed_zero",
        "subnormal",
        "infinity",
        "quiet_nan",
        "exact_integral_rounding",
    ];
    [
        CaseSpec {
            id: "f32-negate",
            operation_name: "negate",
            family: "unary",
            operation: CaseOperation::Unary(UnaryOp::Negate),
            edge_classes: &["signed_zero", "subnormal", "infinity", "quiet_nan"],
            rows: &NEGATE_ROWS,
        },
        CaseSpec {
            id: "f32-add",
            operation_name: "add",
            family: "binary",
            operation: CaseOperation::Binary(BinaryOp::Add),
            edge_classes: &[
                "signed_zero",
                "subnormal",
                "infinity",
                "quiet_nan",
                "round_ties_even",
                "overflow",
            ],
            rows: &ADD_ROWS,
        },
        CaseSpec {
            id: "f32-subtract",
            operation_name: "subtract",
            family: "binary",
            operation: CaseOperation::Binary(BinaryOp::Subtract),
            edge_classes: &[
                "signed_zero",
                "subnormal",
                "infinity",
                "quiet_nan",
                "overflow",
            ],
            rows: &SUBTRACT_ROWS,
        },
        CaseSpec {
            id: "f32-multiply",
            operation_name: "multiply",
            family: "binary",
            operation: CaseOperation::Binary(BinaryOp::Multiply),
            edge_classes: &[
                "signed_zero",
                "subnormal",
                "infinity",
                "quiet_nan",
                "round_ties_even",
                "overflow",
            ],
            rows: &MULTIPLY_ROWS,
        },
        CaseSpec {
            id: "f32-divide",
            operation_name: "divide",
            family: "binary",
            operation: CaseOperation::Binary(BinaryOp::Divide),
            edge_classes: &[
                "signed_zero",
                "subnormal",
                "infinity",
                "quiet_nan",
                "round_ties_even",
                "division_by_zero",
            ],
            rows: &DIVIDE_ROWS,
        },
        CaseSpec {
            id: "f32-remainder",
            operation_name: "remainder",
            family: "binary",
            operation: CaseOperation::Binary(BinaryOp::Remainder),
            edge_classes: &[
                "signed_zero",
                "subnormal",
                "infinity",
                "quiet_nan",
                "invalid_operation",
            ],
            rows: &REMAINDER_ROWS,
        },
        CaseSpec {
            id: "f32-compare-equal",
            operation_name: "compare_equal",
            family: "compare",
            operation: CaseOperation::Compare(ComparePredicate::Equal),
            edge_classes: COMPARE_EDGES,
            rows: &EQUAL_ROWS,
        },
        CaseSpec {
            id: "f32-compare-not-equal",
            operation_name: "compare_not_equal",
            family: "compare",
            operation: CaseOperation::Compare(ComparePredicate::NotEqual),
            edge_classes: COMPARE_EDGES,
            rows: &NOT_EQUAL_ROWS,
        },
        CaseSpec {
            id: "f32-compare-less-than",
            operation_name: "compare_less_than",
            family: "compare",
            operation: CaseOperation::Compare(ComparePredicate::LessThan),
            edge_classes: COMPARE_EDGES,
            rows: &LESS_ROWS,
        },
        CaseSpec {
            id: "f32-compare-less-than-or-equal",
            operation_name: "compare_less_than_or_equal",
            family: "compare",
            operation: CaseOperation::Compare(ComparePredicate::LessThanOrEqual),
            edge_classes: COMPARE_EDGES,
            rows: &LESS_EQUAL_ROWS,
        },
        CaseSpec {
            id: "f32-compare-greater-than",
            operation_name: "compare_greater_than",
            family: "compare",
            operation: CaseOperation::Compare(ComparePredicate::GreaterThan),
            edge_classes: COMPARE_EDGES,
            rows: &GREATER_ROWS,
        },
        CaseSpec {
            id: "f32-compare-greater-than-or-equal",
            operation_name: "compare_greater_than_or_equal",
            family: "compare",
            operation: CaseOperation::Compare(ComparePredicate::GreaterThanOrEqual),
            edge_classes: COMPARE_EDGES,
            rows: &GREATER_EQUAL_ROWS,
        },
        CaseSpec {
            id: "f32-fused-multiply-add",
            operation_name: "fused_multiply_add",
            family: "f32_math",
            operation: CaseOperation::Math(F32MathFunction::FusedMultiplyAdd),
            edge_classes: &[
                "signed_zero",
                "subnormal",
                "infinity",
                "quiet_nan",
                "round_ties_even",
                "overflow",
                "fused_single_rounding",
            ],
            rows: &FMA_ROWS,
        },
        CaseSpec {
            id: "f32-floor",
            operation_name: "floor",
            family: "f32_math",
            operation: CaseOperation::Math(F32MathFunction::Floor),
            edge_classes: INTEGRAL_EDGES,
            rows: &FLOOR_ROWS,
        },
        CaseSpec {
            id: "f32-ceil",
            operation_name: "ceil",
            family: "f32_math",
            operation: CaseOperation::Math(F32MathFunction::Ceil),
            edge_classes: INTEGRAL_EDGES,
            rows: &CEIL_ROWS,
        },
        CaseSpec {
            id: "f32-truncate",
            operation_name: "truncate",
            family: "f32_math",
            operation: CaseOperation::Math(F32MathFunction::Truncate),
            edge_classes: INTEGRAL_EDGES,
            rows: &TRUNCATE_ROWS,
        },
        CaseSpec {
            id: "f32-round-ties-even",
            operation_name: "round_ties_even",
            family: "f32_math",
            operation: CaseOperation::Math(F32MathFunction::RoundTiesEven),
            edge_classes: &[
                "signed_zero",
                "subnormal",
                "infinity",
                "quiet_nan",
                "exact_integral_rounding",
                "round_ties_even",
            ],
            rows: &ROUND_EVEN_ROWS,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_f32_matrix_is_deterministic_and_covers_every_declared_operation() {
        let first = run_f32_differential_v3().unwrap().unwrap();
        let second = run_f32_differential_v3().unwrap().unwrap();
        assert_eq!(first, second);
        assert_eq!(first.operation_cases, CASE_LIMIT);
        assert_eq!(first.rows_compared, 149);
        assert_eq!(
            first.capability_sha256,
            "7c1f98bb7582b01bf920b99b4c6f5db80eb63594abc0f4488b596a3b6a84b196"
        );
        assert_eq!(
            first.suite_sha256,
            "0x4af03e4ceb188fddddb8bc48a288f30e8a6a9e5a52e0b2e220e53cf45c4e6bef"
        );
        assert_eq!(
            first
                .cases
                .iter()
                .map(|case| case.operation)
                .collect::<Vec<_>>(),
            f32_differential_capabilities_v3().admitted_operations
        );
        assert!(first.cases.iter().all(|case| {
            case.oracle_sha256 == case.observed_sha256 && !case.row_ids.is_empty()
        }));
        assert_eq!(first.authority, "none");
        assert!(!first.hardware_observed);
        assert!(!first.performance_prediction);
    }

    #[test]
    fn exact_replay_rejects_case_and_kir_substitution() {
        let report = run_f32_differential_v3().unwrap().unwrap();
        let case = &report.cases[12];
        let replay = replay_f32_differential_case_v3(case.case_id, &case.kir_sha256).unwrap();
        assert_eq!(replay.case, *case);
        assert!(matches!(
            replay_f32_differential_case_v3("F32-FUSED-MULTIPLY-ADD", &case.kir_sha256),
            Err(F32DifferentialErrorV3::UnknownCase(_))
        ));
        let mut substituted = case.kir_sha256.clone();
        substituted.replace_range(0..1, if &substituted[0..1] == "0" { "1" } else { "0" });
        assert!(matches!(
            replay_f32_differential_case_v3(case.case_id, &substituted),
            Err(F32DifferentialErrorV3::KirIdentityMismatch { .. })
        ));
        assert!(matches!(
            replay_f32_differential_case_v3(case.case_id, &case.kir_sha256.to_uppercase()),
            Err(F32DifferentialErrorV3::InvalidKirIdentity)
        ));
    }

    #[test]
    fn mismatch_reduction_retains_the_first_exact_oracle_row() {
        let mut observation = observe(case_specs()[1]).unwrap();
        observation.observed[5] ^= 1;
        observation.observed[13] ^= 1;
        let failure = mismatch(&observation).unwrap();
        assert_eq!(failure.reduction.retained_row, Some(1));
        assert_eq!(failure.reduction.retained_row_id, Some("minimum-subnormal"));
        assert_eq!(
            failure.reduction.retained_input_bits,
            ["0x00000001", "0x00000001"]
        );
        assert_eq!(
            failure.reduction.retained_expected_bits.as_deref(),
            Some("0x00000002")
        );
        assert_eq!(failure.reduction.predicate_evaluations, 2);
        assert!(failure.replay.contains(observation.spec.id));
        assert!(failure.replay.contains(&failure.kir_sha256));
    }

    #[test]
    fn capability_contract_names_scope_and_nonclaims() {
        let capabilities = f32_differential_capabilities_v3();
        assert_eq!(capabilities.case_limit, CASE_LIMIT);
        assert_eq!(capabilities.maximum_rows_per_case, MAX_ROWS_PER_CASE);
        for edge in [
            "signed_zero",
            "subnormal",
            "infinity",
            "quiet_nan",
            "round_ties_even",
        ] {
            assert!(capabilities.edge_classes.contains(&edge), "missing {edge}");
        }
        for exclusion in [
            "float_conversions_and_casts",
            "f16_bf16_f64_edge_matrices",
            "sqrt_and_transcendentals",
            "physical_gpu_parity",
            "performance_prediction",
        ] {
            assert!(
                capabilities
                    .exclusions
                    .iter()
                    .any(|item| item.code == exclusion),
                "missing {exclusion}"
            );
        }
    }
}
