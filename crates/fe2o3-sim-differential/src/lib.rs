#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::io::Write;
use std::process::ExitCode;

use fe2o3_differential::{
    BinaryOp as OracleBinaryOp, Expr, GenerateConfig, KernelCase, MismatchReport, ReductionResult,
    UnaryOp as OracleUnaryOp, compare_outputs, encode_case_v1, generate_case, reduce_case,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CastKind, CheckedBinaryOperator,
    ComparePredicate, Constant, Function, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent,
    MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type,
    UnaryOp, ValueDef, ValueId, VerifiedCanonicalKernelIrV7,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

mod physical_v1;
mod semantic_v2;

pub use physical_v1::{
    MAX_PHYSICAL_DIFFERENTIAL_BYTES_V1, PHYSICAL_DIFFERENTIAL_CAPABILITIES_SCHEMA_V1,
    PHYSICAL_DIFFERENTIAL_SCHEMA_V1, PHYSICAL_DIFFERENTIAL_SIMULATOR_CONTRACT_V1,
    PhysicalDifferentialBufferV1, PhysicalDifferentialByteMismatchV1,
    PhysicalDifferentialCapabilitiesV1, PhysicalDifferentialDispositionV1,
    PhysicalDifferentialErrorV1, PhysicalDifferentialLimitsV1, PhysicalDifferentialReportV1,
    PhysicalDifferentialUnavailableV1, PreparedPhysicalDifferentialV1,
    physical_differential_capabilities_v1, physical_differential_production_readiness_v1,
    prepare_physical_differential_v1,
};

pub use semantic_v2::{
    SEMANTIC_DIFFERENTIAL_CAPABILITIES_SCHEMA_V2, SEMANTIC_DIFFERENTIAL_FAILURE_SCHEMA_V2,
    SEMANTIC_DIFFERENTIAL_REPLAY_SCHEMA_V2, SEMANTIC_DIFFERENTIAL_SCHEMA_V2,
    SemanticDifferentialCapabilitiesV2, SemanticDifferentialErrorV2, SemanticDifferentialFailureV2,
    SemanticDifferentialReplayV2, SemanticDifferentialSuccessV2,
    replay_semantic_differential_case_v2, run_semantic_differential_v2,
    semantic_differential_capabilities_v2,
};

pub const SCALAR_DIFFERENTIAL_SCHEMA_V1: &str = "fe2o3-sim-scalar-differential-v1";
pub const SCALAR_DIFFERENTIAL_FAILURE_SCHEMA_V1: &str = "fe2o3-sim-scalar-differential-failure-v1";
pub const MAX_DIFFERENTIAL_CASES_V1: u32 = 4_096;
pub const MAX_DIFFERENTIAL_RESPONSE_BYTES_V1: usize = 1024 * 1024;

const USAGE: &str = "usage: fe2o3-sim-differential [--seed-start U64] [--cases 1..4096] [--inputs 0..4] [--work-items 1..256] [--max-nodes 1..127] [--max-depth 1..12]";
const DEFAULT_CASES: u32 = 256;
const KIR_VERSION: u8 = 7;
const SIMULATION_TARGET: &str = "amdgpu64-target-neutral";
const WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarDifferentialConfigV1 {
    pub seed_start: u64,
    pub cases: u32,
    pub generation: GenerateConfig,
}

impl ScalarDifferentialConfigV1 {
    pub fn new(
        seed_start: u64,
        cases: u32,
        generation: GenerateConfig,
    ) -> Result<Self, ScalarDifferentialErrorV1> {
        if cases == 0 || cases > MAX_DIFFERENTIAL_CASES_V1 {
            return Err(ScalarDifferentialErrorV1::InvalidCases(cases));
        }
        seed_start
            .checked_add(u64::from(cases - 1))
            .ok_or(ScalarDifferentialErrorV1::SeedRangeOverflow)?;
        Ok(Self {
            seed_start,
            cases,
            generation,
        })
    }
}

impl Default for ScalarDifferentialConfigV1 {
    fn default() -> Self {
        Self {
            seed_start: 0,
            cases: DEFAULT_CASES,
            generation: GenerateConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ScalarDifferentialSuccessV1 {
    pub schema: &'static str,
    pub status: &'static str,
    pub evidence_origin: &'static str,
    pub authority: &'static str,
    pub simulated: bool,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
    pub kir_version: u8,
    pub simulation_target: &'static str,
    pub workgroup_size: [u32; 3],
    pub seed_start: u64,
    pub cases: u32,
    pub work_items_per_case: u16,
    pub lanes_compared: u64,
    pub input_buffers: u8,
    pub max_expression_nodes: u8,
    pub max_expression_depth: u8,
    pub suite_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ScalarDifferentialFailureV1 {
    pub schema: &'static str,
    pub status: &'static str,
    pub evidence_origin: &'static str,
    pub authority: &'static str,
    pub simulated: bool,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
    pub kir_version: u8,
    pub simulation_target: &'static str,
    pub workgroup_size: [u32; 3],
    pub failure_class: ScalarDifferentialFailureClassV1,
    pub seed: u64,
    pub message: String,
    pub source_case_hex: String,
    pub reduced_case_hex: String,
    pub reduced_kir_v7_hex: Option<String>,
    pub reduced_kir_sha256: Option<String>,
    pub reduction: ScalarReductionSummaryV1,
    pub mismatch: Option<ScalarMismatchSummaryV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarDifferentialFailureClassV1 {
    OutputMismatch,
    TranslationFailure,
    SimulationFailure,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ScalarReductionSummaryV1 {
    pub initial_expression_nodes: usize,
    pub final_expression_nodes: usize,
    pub initial_work_items: usize,
    pub final_work_items: usize,
    pub predicate_evaluations: usize,
    pub accepted_reductions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ScalarMismatchSummaryV1 {
    pub expected_len: usize,
    pub observed_len: usize,
    pub total_mismatches: usize,
    pub retained: Vec<ScalarLaneMismatchV1>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ScalarLaneMismatchV1 {
    pub lane: usize,
    pub expected: Option<i32>,
    pub observed: Option<i32>,
}

#[derive(Debug)]
pub enum ScalarDifferentialErrorV1 {
    InvalidCases(u32),
    SeedRangeOverflow,
    Generation(String),
    Kir(String),
    Admission(String),
    Reduction(String),
}

impl fmt::Display for ScalarDifferentialErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCases(cases) => {
                write!(formatter, "invalid differential case count {cases}")
            }
            Self::SeedRangeOverflow => formatter.write_str("differential seed range overflows u64"),
            Self::Generation(message) => write!(formatter, "case generation failed: {message}"),
            Self::Kir(message) => write!(formatter, "KIR translation failed: {message}"),
            Self::Admission(message) => write!(formatter, "simulator admission failed: {message}"),
            Self::Reduction(message) => write!(formatter, "failure reduction failed: {message}"),
        }
    }
}

impl Error for ScalarDifferentialErrorV1 {}

struct PreparedCaseV1 {
    module: AdmittedSimulationModuleV1,
    request: SimulationRequestV1,
    canonical_kir: Vec<u8>,
    kir_sha256: [u8; 32],
    output_argument: usize,
}

enum CaseOutcomeV1 {
    Agreement {
        observed: Vec<i32>,
        kir_sha256: [u8; 32],
    },
    OutputMismatch(MismatchReport),
    TranslationFailure(String),
    SimulationFailure(String),
}

impl CaseOutcomeV1 {
    fn failure_class(&self) -> Option<ScalarDifferentialFailureClassV1> {
        match self {
            Self::Agreement { .. } => None,
            Self::OutputMismatch(_) => Some(ScalarDifferentialFailureClassV1::OutputMismatch),
            Self::TranslationFailure(_) => {
                Some(ScalarDifferentialFailureClassV1::TranslationFailure)
            }
            Self::SimulationFailure(_) => Some(ScalarDifferentialFailureClassV1::SimulationFailure),
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Agreement { .. } => "agreement".to_owned(),
            Self::OutputMismatch(mismatch) => {
                format!("{} lane output mismatch(es)", mismatch.total_mismatches)
            }
            Self::TranslationFailure(message) | Self::SimulationFailure(message) => message.clone(),
        }
    }
}

pub fn run_scalar_differential_v1(
    config: ScalarDifferentialConfigV1,
) -> Result<
    Result<ScalarDifferentialSuccessV1, ScalarDifferentialFailureV1>,
    ScalarDifferentialErrorV1,
> {
    ScalarDifferentialConfigV1::new(config.seed_start, config.cases, config.generation)?;
    run_with_observer(config, simulate_case_v1)
}

fn run_with_observer<F>(
    config: ScalarDifferentialConfigV1,
    mut observe: F,
) -> Result<
    Result<ScalarDifferentialSuccessV1, ScalarDifferentialFailureV1>,
    ScalarDifferentialErrorV1,
>
where
    F: FnMut(&KernelCase) -> CaseOutcomeV1,
{
    let mut suite = Sha256::new();
    suite.update(b"FE2O3/SIM-SCALAR-DIFFERENTIAL/V1\0");
    suite.update(config.seed_start.to_le_bytes());
    suite.update(config.cases.to_le_bytes());
    suite.update(config.generation.input_count().to_le_bytes());
    suite.update(config.generation.work_items().to_le_bytes());
    suite.update(config.generation.max_nodes().to_le_bytes());
    suite.update(config.generation.max_depth().to_le_bytes());
    for offset in 0..config.cases {
        let seed = config.seed_start + u64::from(offset);
        let case = generate_case(seed, config.generation);
        let case_bytes = encode_case_v1(&case)
            .map_err(|error| ScalarDifferentialErrorV1::Generation(error.to_string()))?;
        match observe(&case) {
            CaseOutcomeV1::Agreement {
                observed,
                kir_sha256,
            } => {
                suite.update((case_bytes.len() as u64).to_le_bytes());
                suite.update(case_bytes);
                suite.update(kir_sha256);
                for value in observed {
                    suite.update(value.to_le_bytes());
                }
            }
            failure => {
                return Ok(Err(reduce_failure_v1(&case, failure, &mut observe)?));
            }
        }
    }
    Ok(Ok(ScalarDifferentialSuccessV1 {
        schema: SCALAR_DIFFERENTIAL_SCHEMA_V1,
        status: "agreement",
        evidence_origin: "differential_model_agreement",
        authority: "none",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        kir_version: KIR_VERSION,
        simulation_target: SIMULATION_TARGET,
        workgroup_size: WORKGROUP_SIZE,
        seed_start: config.seed_start,
        cases: config.cases,
        work_items_per_case: config.generation.work_items(),
        lanes_compared: u64::from(config.cases) * u64::from(config.generation.work_items()),
        input_buffers: config.generation.input_count(),
        max_expression_nodes: config.generation.max_nodes(),
        max_expression_depth: config.generation.max_depth(),
        suite_sha256: hex(&suite.finalize()),
    }))
}

fn reduce_failure_v1<F>(
    case: &KernelCase,
    initial: CaseOutcomeV1,
    observe: &mut F,
) -> Result<ScalarDifferentialFailureV1, ScalarDifferentialErrorV1>
where
    F: FnMut(&KernelCase) -> CaseOutcomeV1,
{
    let failure_class = initial
        .failure_class()
        .expect("reduction is entered only for a failure");
    let reduction = reduce_case(case, |candidate| {
        observe(candidate).failure_class() == Some(failure_class)
    })
    .map_err(|error| ScalarDifferentialErrorV1::Reduction(error.to_string()))?;
    let reduced_outcome = observe(&reduction.case);
    if reduced_outcome.failure_class() != Some(failure_class) {
        return Err(ScalarDifferentialErrorV1::Reduction(
            "reduced case did not reproduce the initial failure class".to_owned(),
        ));
    }
    let prepared = prepare_case_v1(&reduction.case).ok();
    let source_case = encode_case_v1(case)
        .map_err(|error| ScalarDifferentialErrorV1::Generation(error.to_string()))?;
    let reduced_case = encode_case_v1(&reduction.case)
        .map_err(|error| ScalarDifferentialErrorV1::Generation(error.to_string()))?;
    let mismatch = match &reduced_outcome {
        CaseOutcomeV1::OutputMismatch(mismatch) => Some(mismatch_summary(mismatch)),
        _ => None,
    };
    Ok(ScalarDifferentialFailureV1 {
        schema: SCALAR_DIFFERENTIAL_FAILURE_SCHEMA_V1,
        status: "failure",
        evidence_origin: "differential_model_observation",
        authority: "none",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        kir_version: KIR_VERSION,
        simulation_target: SIMULATION_TARGET,
        workgroup_size: WORKGROUP_SIZE,
        failure_class,
        seed: case.seed(),
        message: reduced_outcome.message(),
        source_case_hex: hex(&source_case),
        reduced_case_hex: hex(&reduced_case),
        reduced_kir_v7_hex: prepared.as_ref().map(|value| hex(&value.canonical_kir)),
        reduced_kir_sha256: prepared.as_ref().map(|value| hex(&value.kir_sha256)),
        reduction: reduction_summary(&reduction),
        mismatch,
    })
}

fn reduction_summary(reduction: &ReductionResult) -> ScalarReductionSummaryV1 {
    ScalarReductionSummaryV1 {
        initial_expression_nodes: reduction.initial_complexity.expression_nodes,
        final_expression_nodes: reduction.final_complexity.expression_nodes,
        initial_work_items: reduction.initial_complexity.work_items,
        final_work_items: reduction.final_complexity.work_items,
        predicate_evaluations: reduction.predicate_evaluations,
        accepted_reductions: reduction.accepted_reductions,
    }
}

fn mismatch_summary(mismatch: &MismatchReport) -> ScalarMismatchSummaryV1 {
    ScalarMismatchSummaryV1 {
        expected_len: mismatch.expected_len,
        observed_len: mismatch.observed_len,
        total_mismatches: mismatch.total_mismatches,
        retained: mismatch
            .mismatches
            .iter()
            .map(|item| ScalarLaneMismatchV1 {
                lane: item.lane,
                expected: item.expected,
                observed: item.observed,
            })
            .collect(),
        truncated: mismatch.truncated,
    }
}

fn simulate_case_v1(case: &KernelCase) -> CaseOutcomeV1 {
    let prepared = match prepare_case_v1(case) {
        Ok(prepared) => prepared,
        Err(error) => return CaseOutcomeV1::TranslationFailure(error.to_string()),
    };
    let execution = match prepared.module.simulate(
        &prepared.request,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
    ) {
        Ok(execution) => execution,
        Err(error) => return CaseOutcomeV1::SimulationFailure(error.to_string()),
    };
    let Some(output) = execution.buffer(prepared.output_argument) else {
        return CaseOutcomeV1::SimulationFailure("missing output buffer".to_owned());
    };
    let expected_output_bytes = usize::from(case.program().work_items()) * 4;
    if output.bytes().len() != expected_output_bytes {
        return CaseOutcomeV1::SimulationFailure(format!(
            "output buffer has {} bytes, expected {expected_output_bytes}",
            output.bytes().len()
        ));
    }
    let observed = output
        .bytes()
        .chunks_exact(4)
        .map(|bytes| i32::from_le_bytes(bytes.try_into().expect("four-byte i32 output")))
        .collect::<Vec<_>>();
    let mismatch = compare_outputs(case, &observed);
    if mismatch.is_mismatch() {
        CaseOutcomeV1::OutputMismatch(mismatch)
    } else {
        CaseOutcomeV1::Agreement {
            observed,
            kir_sha256: prepared.kir_sha256,
        }
    }
}

fn prepare_case_v1(case: &KernelCase) -> Result<PreparedCaseV1, ScalarDifferentialErrorV1> {
    let mut lower = ScalarKirLoweringV1::new(case.program().input_count());
    let value = lower.expression(case.program().expression())?;
    lower.store_output(value);
    let (module, request, output_argument) = lower.finish(case)?;
    let canonical = VerifiedCanonicalKernelIrV7::from_module(module)
        .map_err(|error| ScalarDifferentialErrorV1::Kir(error.to_string()))?;
    let canonical_kir = canonical.canonical_bytes().to_vec();
    let kir_sha256 = Sha256::digest(&canonical_kir).into();
    let module = AdmittedSimulationModuleV1::admit(canonical, SimulationLimitsV1::default())
        .map_err(|error| ScalarDifferentialErrorV1::Admission(error.to_string()))?;
    Ok(PreparedCaseV1 {
        module,
        request,
        canonical_kir,
        kir_sha256,
        output_argument,
    })
}

struct ScalarKirLoweringV1 {
    input_count: u8,
    next_value: u32,
    operations: Vec<Operation>,
    input_pointers: Vec<ValueId>,
    output_pointer: ValueId,
}

impl ScalarKirLoweringV1 {
    fn new(input_count: u8) -> Self {
        let input_pointers = (0..u32::from(input_count)).map(ValueId).collect::<Vec<_>>();
        Self {
            input_count,
            next_value: u32::from(input_count) + 1,
            operations: Vec::new(),
            input_pointers,
            output_pointer: ValueId(u32::from(input_count)),
        }
    }

    fn next(&mut self) -> ValueId {
        let value = ValueId(self.next_value);
        self.next_value += 1;
        value
    }

    fn one(&mut self, ty: Type, kind: OperationKind) -> ValueId {
        let result = self.next();
        self.operations
            .push(Operation::effect_free(ValueDef::new(result, ty), kind));
        result
    }

    fn expression(&mut self, expression: &Expr) -> Result<ValueId, ScalarDifferentialErrorV1> {
        Ok(match expression {
            Expr::Const(value) => self.one(
                Type::Scalar(ScalarType::I32),
                OperationKind::Constant(Constant::I32(*value)),
            ),
            Expr::GlobalId => {
                let index = self.one(
                    Type::INDEX,
                    OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
                );
                self.index_to_i32(index)
            }
            Expr::Load { input } => {
                let base = self
                    .input_pointers
                    .get(usize::from(*input))
                    .copied()
                    .ok_or_else(|| {
                        ScalarDifferentialErrorV1::Kir(format!("unknown input {input}"))
                    })?;
                let index = self.one(
                    Type::INDEX,
                    OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
                );
                let pointer = self.one(
                    i32_pointer(AccessMode::ReadOnly),
                    OperationKind::GetElementPointer {
                        base,
                        offset: index,
                    },
                );
                self.one(
                    Type::Scalar(ScalarType::I32),
                    OperationKind::Load {
                        pointer,
                        access: MemoryAccess::new(AddressSpace::Global, 4),
                    },
                )
            }
            Expr::Unary { op, value } => {
                let value = self.expression(value)?;
                match op {
                    OracleUnaryOp::Not => self.one(
                        Type::Scalar(ScalarType::I32),
                        OperationKind::Unary {
                            op: UnaryOp::Not,
                            operand: value,
                        },
                    ),
                    OracleUnaryOp::Neg => {
                        let zero = self.one(
                            Type::Scalar(ScalarType::I32),
                            OperationKind::Constant(Constant::I32(0)),
                        );
                        self.checked_binary(CheckedBinaryOperator::Subtract, zero, value)
                    }
                }
            }
            Expr::Binary { op, left, right } => {
                let left = self.expression(left)?;
                let right = self.expression(right)?;
                match op {
                    OracleBinaryOp::Add => {
                        self.checked_binary(CheckedBinaryOperator::Add, left, right)
                    }
                    OracleBinaryOp::Sub => {
                        self.checked_binary(CheckedBinaryOperator::Subtract, left, right)
                    }
                    OracleBinaryOp::Mul => {
                        self.checked_binary(CheckedBinaryOperator::Multiply, left, right)
                    }
                    OracleBinaryOp::BitAnd | OracleBinaryOp::BitOr | OracleBinaryOp::BitXor => {
                        let operation = match op {
                            OracleBinaryOp::BitAnd => BinaryOp::BitAnd,
                            OracleBinaryOp::BitOr => BinaryOp::BitOr,
                            OracleBinaryOp::BitXor => BinaryOp::BitXor,
                            _ => unreachable!("guarded bitwise operation"),
                        };
                        self.one(
                            Type::Scalar(ScalarType::I32),
                            OperationKind::Binary {
                                op: operation,
                                lhs: left,
                                rhs: right,
                            },
                        )
                    }
                    OracleBinaryOp::Eq | OracleBinaryOp::Lt => {
                        let predicate = if *op == OracleBinaryOp::Eq {
                            ComparePredicate::Equal
                        } else {
                            ComparePredicate::LessThan
                        };
                        let boolean = self.one(
                            Type::BOOL,
                            OperationKind::Compare {
                                predicate,
                                lhs: left,
                                rhs: right,
                            },
                        );
                        self.one(
                            Type::Scalar(ScalarType::I32),
                            OperationKind::Cast {
                                kind: CastKind::ZeroExtend,
                                value: boolean,
                                to: Type::Scalar(ScalarType::I32),
                            },
                        )
                    }
                }
            }
            Expr::Select {
                condition,
                then_value,
                else_value,
            } => {
                let condition = self.expression(condition)?;
                let zero = self.one(
                    Type::Scalar(ScalarType::I32),
                    OperationKind::Constant(Constant::I32(0)),
                );
                let condition = self.one(
                    Type::BOOL,
                    OperationKind::Compare {
                        predicate: ComparePredicate::NotEqual,
                        lhs: condition,
                        rhs: zero,
                    },
                );
                let true_value = self.expression(then_value)?;
                let false_value = self.expression(else_value)?;
                self.one(
                    Type::Scalar(ScalarType::I32),
                    OperationKind::Select {
                        condition,
                        true_value,
                        false_value,
                    },
                )
            }
        })
    }

    fn checked_binary(
        &mut self,
        operator: CheckedBinaryOperator,
        lhs: ValueId,
        rhs: ValueId,
    ) -> ValueId {
        // Checked KIR makes wrapping explicit; the independent model consumes
        // the wrapped result while this harness deliberately ignores the flag.
        let value = self.next();
        let overflow = self.next();
        self.operations.push(Operation::checked_binary(
            ValueDef::new(value, Type::Scalar(ScalarType::I32)),
            ValueDef::new(overflow, Type::BOOL),
            operator,
            lhs,
            rhs,
        ));
        value
    }

    fn index_to_i32(&mut self, index: ValueId) -> ValueId {
        let fixed_width = self.one(
            Type::Scalar(ScalarType::U64),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: index,
                to: Type::Scalar(ScalarType::U64),
            },
        );
        self.one(
            Type::Scalar(ScalarType::I32),
            OperationKind::Cast {
                kind: CastKind::Truncate,
                value: fixed_width,
                to: Type::Scalar(ScalarType::I32),
            },
        )
    }

    fn store_output(&mut self, value: ValueId) {
        let index = self.one(
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        );
        let pointer = self.one(
            i32_pointer(AccessMode::ReadWrite),
            OperationKind::GetElementPointer {
                base: self.output_pointer,
                offset: index,
            },
        );
        self.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer,
                value,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }

    fn finish(
        self,
        case: &KernelCase,
    ) -> Result<(Module, SimulationRequestV1, usize), ScalarDifferentialErrorV1> {
        let output_argument = usize::from(self.input_count);
        let mut parameters = vec![i32_pointer(AccessMode::ReadOnly); output_argument];
        parameters.push(i32_pointer(AccessMode::ReadWrite));
        let parameter_values = (0..parameters.len())
            .map(|index| ValueId(u32::try_from(index).expect("bounded input count")))
            .collect::<Vec<_>>();
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = self.operations;
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        let entry = Function::kernel_entry(
            "generated_scalar_impl",
            Signature::new(parameters, Vec::new()),
            parameter_values,
            vec![block],
        );
        let mut module = Module::new("fe2o3-sim-differential::generated-scalar-v1");
        module.functions.push(entry);
        module.kernels.push(Kernel::new(
            "generated_scalar",
            "generated_scalar_impl",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));

        let target = SimulationTargetV1::amdgpu_64();
        let mut arguments = case
            .inputs()
            .iter()
            .map(|values| {
                let values = values
                    .iter()
                    .copied()
                    .map(ScalarBitsV1::i32)
                    .collect::<Vec<_>>();
                BufferArgumentV1::from_scalars(AccessMode::ReadOnly, 4, &values, target)
                    .map(SimulationArgumentV1::Buffer)
                    .map_err(|error| ScalarDifferentialErrorV1::Kir(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let zeroes = vec![ScalarBitsV1::i32(0); usize::from(case.program().work_items())];
        arguments.push(SimulationArgumentV1::Buffer(
            BufferArgumentV1::from_scalars(AccessMode::ReadWrite, 4, &zeroes, target)
                .map_err(|error| ScalarDifferentialErrorV1::Kir(error.to_string()))?,
        ));
        let request = SimulationRequestV1::new(
            "generated_scalar",
            [u64::from(case.program().work_items()), 1, 1],
            WORKGROUP_SIZE,
            arguments,
        );
        Ok((module, request, output_argument))
    }
}

fn i32_pointer(access: AccessMode) -> Type {
    Type::pointer(Type::Scalar(ScalarType::I32), AddressSpace::Global, access)
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(2 + bytes.len() * 2);
    output.push_str("0x");
    for byte in bytes {
        use fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[derive(Serialize)]
struct CommandErrorV1<'a> {
    schema: &'static str,
    status: &'static str,
    code: &'a str,
    message: &'a str,
    evidence_origin: &'static str,
    authority: &'static str,
    hardware_observed: bool,
    performance_prediction: bool,
}

pub fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let first = arguments.next();
    match first.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("physical-capabilities-v1") => {
            if arguments.next().is_some() {
                emit_command_error(
                    "invalid_command_line",
                    "usage: fe2o3-sim-differential physical-capabilities-v1",
                );
                return ExitCode::FAILURE;
            }
            return write_json_stdout(&physical_differential_capabilities_v1());
        }
        Some("semantic-capabilities-v2") => {
            if arguments.next().is_some() {
                emit_command_error(
                    "invalid_command_line",
                    "usage: fe2o3-sim-differential semantic-capabilities-v2",
                );
                return ExitCode::FAILURE;
            }
            return write_json_stdout(&semantic_differential_capabilities_v2());
        }
        Some("semantic-run-v2") => {
            let seed = match parse_semantic_seed(arguments) {
                Ok(seed) => seed,
                Err(error) => {
                    emit_command_error("invalid_command_line", &error);
                    return ExitCode::FAILURE;
                }
            };
            return match run_semantic_differential_v2(seed) {
                Ok(Ok(report)) => write_json_stdout(&report),
                Ok(Err(report)) => write_json_stderr(&report),
                Err(error) => {
                    emit_command_error("harness_failed", &error.to_string());
                    ExitCode::FAILURE
                }
            };
        }
        Some("semantic-replay-v2") => {
            let (seed, case, kir_sha256) = match parse_semantic_replay(arguments) {
                Ok(replay) => replay,
                Err(error) => {
                    emit_command_error("invalid_command_line", &error);
                    return ExitCode::FAILURE;
                }
            };
            return match replay_semantic_differential_case_v2(seed, &case, &kir_sha256) {
                Ok(report) => write_json_stdout(&report),
                Err(error) => {
                    emit_command_error("replay_rejected", &error.to_string());
                    ExitCode::FAILURE
                }
            };
        }
        _ => {}
    }
    let config = match parse(first.into_iter().chain(arguments)) {
        Ok(config) => config,
        Err(error) => {
            emit_command_error("invalid_command_line", &error);
            return ExitCode::FAILURE;
        }
    };
    match run_scalar_differential_v1(config) {
        Ok(Ok(report)) => write_json_stdout(&report),
        Ok(Err(report)) => write_json_stderr(&report),
        Err(error) => {
            emit_command_error("harness_failed", &error.to_string());
            ExitCode::FAILURE
        }
    }
}

fn parse_semantic_seed(mut arguments: impl Iterator<Item = OsString>) -> Result<u64, String> {
    let Some(name) = arguments.next() else {
        return Ok(0);
    };
    if name != "--seed" {
        return Err("usage: fe2o3-sim-differential semantic-run-v2 [--seed U64]".to_owned());
    }
    let value = arguments
        .next()
        .ok_or_else(|| "usage: fe2o3-sim-differential semantic-run-v2 [--seed U64]".to_owned())?;
    if arguments.next().is_some() {
        return Err("usage: fe2o3-sim-differential semantic-run-v2 [--seed U64]".to_owned());
    }
    parse_number(
        value
            .to_str()
            .ok_or_else(|| "--seed must be UTF-8".to_owned())?,
        "--seed",
    )
}

fn parse_semantic_replay(
    arguments: impl Iterator<Item = OsString>,
) -> Result<(u64, String, String), String> {
    let usage =
        "usage: fe2o3-sim-differential semantic-replay-v2 --seed U64 --case ID --kir-sha256 HEX";
    let mut seed = None;
    let mut case = None;
    let mut kir_sha256 = None;
    let mut arguments = arguments;
    while let Some(name) = arguments.next() {
        let value = arguments.next().ok_or_else(|| usage.to_owned())?;
        let name = name.to_str().ok_or_else(|| usage.to_owned())?;
        let value = value.to_str().ok_or_else(|| usage.to_owned())?;
        match name {
            "--seed" => assign(&mut seed, parse_number(value, name)?, name)?,
            "--case" => assign(&mut case, value.to_owned(), name)?,
            "--kir-sha256" => assign(&mut kir_sha256, value.to_owned(), name)?,
            _ => return Err(usage.to_owned()),
        }
    }
    Ok((
        seed.ok_or_else(|| usage.to_owned())?,
        case.ok_or_else(|| usage.to_owned())?,
        kir_sha256.ok_or_else(|| usage.to_owned())?,
    ))
}

fn write_json_stdout(value: &impl Serialize) -> ExitCode {
    let Some(encoded) = encode_json_line(value) else {
        emit_command_error(
            "response_bound_exceeded",
            "encoded differential response exceeded its compiled byte bound",
        );
        return ExitCode::FAILURE;
    };
    let mut output = std::io::stdout().lock();
    if output.write_all(&encoded).is_err() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn write_json_stderr(value: &impl Serialize) -> ExitCode {
    let mut output = std::io::stderr().lock();
    if let Some(encoded) = encode_json_line(value) {
        let _ = output.write_all(&encoded);
    } else {
        let _ = output.write_all(b"{\"schema\":\"fe2o3-sim-scalar-differential-command-error-v1\",\"status\":\"error\",\"code\":\"response_bound_exceeded\",\"message\":\"encoded differential response exceeded its compiled byte bound\",\"evidence_origin\":\"command_validation\",\"authority\":\"none\",\"hardware_observed\":false,\"performance_prediction\":false}\n");
    }
    ExitCode::FAILURE
}

fn encode_json_line(value: &impl Serialize) -> Option<Vec<u8>> {
    let mut encoded = serde_json::to_vec(value).ok()?;
    if encoded.len() >= MAX_DIFFERENTIAL_RESPONSE_BYTES_V1 {
        return None;
    }
    encoded.push(b'\n');
    Some(encoded)
}

fn emit_command_error(code: &str, message: &str) {
    let error = CommandErrorV1 {
        schema: "fe2o3-sim-scalar-differential-command-error-v1",
        status: "error",
        code,
        message,
        evidence_origin: "command_validation",
        authority: "none",
        hardware_observed: false,
        performance_prediction: false,
    };
    let _ = write_json_stderr(&error);
}

fn parse(arguments: impl Iterator<Item = OsString>) -> Result<ScalarDifferentialConfigV1, String> {
    let defaults = GenerateConfig::default();
    let mut seed_start = None;
    let mut cases = None;
    let mut inputs = None;
    let mut work_items = None;
    let mut max_nodes = None;
    let mut max_depth = None;
    let mut arguments = arguments;
    while let Some(name) = arguments.next() {
        let value = arguments.next().ok_or_else(|| USAGE.to_owned())?;
        let name = name.to_str().ok_or_else(|| USAGE.to_owned())?;
        let value = value.to_str().ok_or_else(|| USAGE.to_owned())?;
        match name {
            "--seed-start" => assign(&mut seed_start, parse_number(value, name)?, name)?,
            "--cases" => assign(&mut cases, parse_number(value, name)?, name)?,
            "--inputs" => assign(&mut inputs, parse_number(value, name)?, name)?,
            "--work-items" => assign(&mut work_items, parse_number(value, name)?, name)?,
            "--max-nodes" => assign(&mut max_nodes, parse_number(value, name)?, name)?,
            "--max-depth" => assign(&mut max_depth, parse_number(value, name)?, name)?,
            _ => return Err(USAGE.to_owned()),
        }
    }
    let generation = GenerateConfig::new(
        inputs.unwrap_or(defaults.input_count()),
        work_items.unwrap_or(defaults.work_items()),
        max_nodes.unwrap_or(defaults.max_nodes()),
        max_depth.unwrap_or(defaults.max_depth()),
    )
    .map_err(|error| error.to_string())?;
    ScalarDifferentialConfigV1::new(
        seed_start.unwrap_or(0),
        cases.unwrap_or(DEFAULT_CASES),
        generation,
    )
    .map_err(|error| error.to_string())
}

fn assign<T>(slot: &mut Option<T>, value: T, name: &str) -> Result<(), String> {
    if slot.replace(value).is_some() {
        return Err(format!("duplicate {name}; {USAGE}"));
    }
    Ok(())
}

fn parse_number<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{name} must be an unsigned decimal integer; {USAGE}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_reducer_preserves_an_injected_output_mismatch() {
        let config =
            ScalarDifferentialConfigV1::new(17, 1, GenerateConfig::new(2, 8, 31, 7).unwrap())
                .unwrap();
        let inject = |case: &KernelCase| {
            let mut observed = fe2o3_differential::evaluate_case(case);
            observed[0] ^= 1;
            CaseOutcomeV1::OutputMismatch(compare_outputs(case, &observed))
        };
        let first = run_with_observer(config, inject).unwrap().unwrap_err();
        let second = run_with_observer(config, inject).unwrap().unwrap_err();
        assert_eq!(first, second);
        assert_eq!(
            first.failure_class,
            ScalarDifferentialFailureClassV1::OutputMismatch
        );
        assert!(first.reduction.accepted_reductions > 0);
        assert!(first.reduction.final_expression_nodes <= first.reduction.initial_expression_nodes);
        assert!(
            first
                .reduced_kir_v7_hex
                .as_deref()
                .unwrap()
                .starts_with("0x")
        );
        assert_eq!(first.evidence_origin, "differential_model_observation");
        assert_eq!(first.authority, "none");
        assert!(!first.hardware_observed);
        assert!(!first.performance_prediction);
        assert!(encode_json_line(&first).is_some());
    }

    #[test]
    fn case_and_kir_translation_are_deterministic() {
        let case = generate_case(99, GenerateConfig::new(2, 8, 31, 7).unwrap());
        let first = prepare_case_v1(&case).unwrap();
        let second = prepare_case_v1(&case).unwrap();
        assert_eq!(first.canonical_kir, second.canonical_kir);
        assert_eq!(first.kir_sha256, second.kir_sha256);
    }

    #[test]
    fn generated_suite_agrees_and_has_a_reproducible_identity() {
        let config =
            ScalarDifferentialConfigV1::new(0, 32, GenerateConfig::new(2, 16, 31, 7).unwrap())
                .unwrap();
        let first = run_scalar_differential_v1(config).unwrap().unwrap();
        let second = run_scalar_differential_v1(config).unwrap().unwrap();
        assert_eq!(first, second);
        assert_eq!(first.cases, 32);
        assert_eq!(first.lanes_compared, 512);
        assert_eq!(first.kir_version, 7);
        assert_eq!(first.simulation_target, "amdgpu64-target-neutral");
        assert_eq!(first.workgroup_size, [64, 1, 1]);
        assert_eq!(first.evidence_origin, "differential_model_agreement");
        assert_eq!(first.authority, "none");
        assert!(!first.hardware_observed);
        assert!(!first.performance_prediction);
    }

    #[test]
    fn complexity_import_is_kept_exact() {
        let case = generate_case(1, GenerateConfig::new(1, 4, 7, 4).unwrap());
        let complexity = fe2o3_differential::CaseComplexity::measure(&case);
        assert_eq!(complexity.work_items, 4);
    }

    #[test]
    fn default_ci_seed_range_covers_every_translated_expression_form() {
        fn visit(expression: &Expr, coverage: &mut u16) {
            match expression {
                Expr::Const(_) => *coverage |= 1 << 0,
                Expr::GlobalId => *coverage |= 1 << 1,
                Expr::Load { .. } => *coverage |= 1 << 2,
                Expr::Unary { op, value } => {
                    *coverage |= match op {
                        OracleUnaryOp::Neg => 1 << 3,
                        OracleUnaryOp::Not => 1 << 4,
                    };
                    visit(value, coverage);
                }
                Expr::Binary { op, left, right } => {
                    *coverage |= 1
                        << match op {
                            OracleBinaryOp::Add => 5,
                            OracleBinaryOp::Sub => 6,
                            OracleBinaryOp::Mul => 7,
                            OracleBinaryOp::BitAnd => 8,
                            OracleBinaryOp::BitOr => 9,
                            OracleBinaryOp::BitXor => 10,
                            OracleBinaryOp::Eq => 11,
                            OracleBinaryOp::Lt => 12,
                        };
                    visit(left, coverage);
                    visit(right, coverage);
                }
                Expr::Select {
                    condition,
                    then_value,
                    else_value,
                } => {
                    *coverage |= 1 << 13;
                    visit(condition, coverage);
                    visit(then_value, coverage);
                    visit(else_value, coverage);
                }
            }
        }

        let mut coverage = 0;
        for seed in 0..u64::from(DEFAULT_CASES) {
            let case = generate_case(seed, GenerateConfig::default());
            visit(case.program().expression(), &mut coverage);
        }
        assert_eq!(coverage, (1 << 14) - 1);
    }

    #[test]
    fn encoded_response_limit_rejects_oversized_json() {
        #[derive(Serialize)]
        struct Oversized {
            bytes: String,
        }

        let value = Oversized {
            bytes: "x".repeat(MAX_DIFFERENTIAL_RESPONSE_BYTES_V1),
        };
        assert!(encode_json_line(&value).is_none());
    }
}
