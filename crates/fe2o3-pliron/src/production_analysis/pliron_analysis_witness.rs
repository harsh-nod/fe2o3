//! Independently replayed witnesses for sealed production analysis reports.
//!
//! The envelope in this module is issued only from the compiler-owned report
//! custody session. It binds an exact PLIRON subject and pass checkpoint to a
//! typed witness, then re-derives the supported obligations from live IR. A
//! digest, a report commitment, or a caller-authored success bit is never
//! accepted as evidence.
//!
//! V1 completes only the explicitly documented ranked-bounds fragment. Other
//! passes and unsupported bounds forms remain `Incomplete`. A complete replay
//! validates this analysis report at this checkpoint; it grants no compiler
//! refinement, lowering, artifact, publication, or launch authority.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

use dialect_gpu::ExecutionLayoutOp;
use dialect_kernel::{
    DYNAMIC_EXTENT, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp, InvocationIndexOp,
    MAX_RANKED_MEMORY_RANK, RankedAccessOp, ranked_view_type,
};
use fe2o3_pliron_owner_core::{ContextIdentity, require_context_identity};
use pliron::{
    builtin::ops::FuncOp,
    common_traits::Verify,
    context::{Context, Ptr},
    op::Op,
    operation::Operation,
    r#type::TypedHandle,
    value::Value,
};

use crate::production_analysis::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use crate::{
    CapturedProductionAnalysisReportV1, KernelCheckPassKindV1, KernelCheckStatusV1,
    PresburgerMapV1, ProductionAnalysisCheckpointV1, ProductionAnalysisConfigurationV1,
    ProductionAnalysisImplementationV1, ProductionAnalysisWitnessGapV1, RankedBoundsReportV1,
    SparseIndexFactV1, witness_gap,
};

const MAX_BOUNDS_WITNESS_INVOCATIONS_V1: u64 = 65_536;
pub(crate) const MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1: usize = 1_048_576;
pub(crate) const MAX_PRODUCTION_ANALYSIS_WITNESS_REASON_BYTES_V1: usize = 1_024;
const RAW_INDEX_STACK_FRAMES_PER_OPERATION_V1: usize = 2;
const RAW_INDEX_STACK_FIXED_FRAMES_V1: usize = 1;
const RAW_INDEX_STACK_WORK_PER_EVALUATION_ATTEMPT_V1: usize = 8;
// Four logical fields in the widest frame and at most one Vec growth copy.
const RAW_INDEX_STACK_STORAGE_ITEMS_PER_FRAME_V1: usize = 8;

fn witness_resource_error_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::ReportValidation,
        resource: "analysis witness resource upper bound",
    }
}

fn checked_witness_sum_v1(values: &[usize]) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(witness_resource_error_v1)
    })
}

fn checked_witness_product_v1(
    lhs: usize,
    rhs: usize,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs).ok_or_else(witness_resource_error_v1)
}

fn raw_index_stack_frame_upper_bound_v1(
    operations: usize,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    checked_witness_product_v1(operations, RAW_INDEX_STACK_FRAMES_PER_OPERATION_V1)?
        .checked_add(RAW_INDEX_STACK_FIXED_FRAMES_V1)
        .ok_or_else(witness_resource_error_v1)
}

pub(crate) fn preflight_production_analysis_witness_resource_upper_bound_v1(
    pass: KernelCheckPassKindV1,
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let phase = ProductionAnalysisResourcePhaseV1::ReportValidation;
    let reason_storage = MAX_PRODUCTION_ANALYSIS_WITNESS_REASON_BYTES_V1 * 2;
    let (work, retained, temporary) = if pass == KernelCheckPassKindV1::MemoryBounds {
        let obligations = checked_witness_product_v1(census.operations, MAX_RANKED_MEMORY_RANK)?;
        let obligation_items = MAX_RANKED_MEMORY_RANK * 3 + 16;
        let witness_storage = checked_witness_sum_v1(&[
            checked_witness_product_v1(obligations, obligation_items)?,
            1,
        ])?;
        let charged_evaluation_attempts = MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1
            .checked_add(1)
            .ok_or_else(witness_resource_error_v1)?;
        // Every raw-index evaluation attempt is enclosed by one invocation
        // iteration. In addition to the evaluator visit, that iteration creates
        // a cache and active-set owner and can scan the complete rank-wide
        // odometer. The rejecting (limit + 1) evaluation has both map owners
        // live even though it returns before advancing the odometer.
        let invocation_iteration_work = checked_witness_product_v1(
            charged_evaluation_attempts,
            MAX_RANKED_MEMORY_RANK
                .checked_add(2)
                .ok_or_else(witness_resource_error_v1)?,
        )?;
        // Each invocation contributes one root frame. Every evaluated binary
        // can add a finish frame and two operand frames; charging eight frame
        // operations per attempt covers all corresponding pushes and pops.
        let evaluation_stack_work = checked_witness_product_v1(
            charged_evaluation_attempts,
            RAW_INDEX_STACK_WORK_PER_EVALUATION_ATTEMPT_V1,
        )?;
        let evaluation_stack_storage = checked_witness_product_v1(
            raw_index_stack_frame_upper_bound_v1(census.operations)?,
            RAW_INDEX_STACK_STORAGE_ITEMS_PER_FRAME_V1,
        )?;
        let one_replay_work = checked_witness_sum_v1(&[
            checked_witness_product_v1(census.operations, 2)?,
            charged_evaluation_attempts,
            invocation_iteration_work,
            evaluation_stack_work,
            checked_witness_product_v1(obligations, MAX_RANKED_MEMORY_RANK * 4 + 16)?,
            MAX_RANKED_MEMORY_RANK * 8 + 32,
        ])?;
        let one_replay_temporary = checked_witness_sum_v1(&[
            checked_witness_product_v1(census.operations, 6)?,
            evaluation_stack_storage,
            MAX_RANKED_MEMORY_RANK * 8 + 32,
        ])?;
        let retained = witness_storage.max(reason_storage);
        (
            checked_witness_sum_v1(&[
                checked_witness_product_v1(one_replay_work, 2)?,
                witness_storage,
                reason_storage,
            ])?,
            retained,
            checked_witness_sum_v1(&[witness_storage, one_replay_temporary, reason_storage])?,
        )
    } else {
        (
            reason_storage + 32,
            reason_storage,
            MAX_PRODUCTION_ANALYSIS_WITNESS_REASON_BYTES_V1,
        )
    };
    let bound =
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, retained, temporary)?;
    limits.require(phase, bound)
}

fn bounded_witness_reason_v1(mut reason: String) -> String {
    if reason.len() <= MAX_PRODUCTION_ANALYSIS_WITNESS_REASON_BYTES_V1 {
        return reason;
    }
    let mut end = MAX_PRODUCTION_ANALYSIS_WITNESS_REASON_BYTES_V1;
    while !reason.is_char_boundary(end) {
        end -= 1;
    }
    reason.truncate(end);
    reason
}

/// Checker implementation recorded in a witness envelope.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionAnalysisWitnessCheckerV1 {
    BoundsExhaustiveRawIrReplayV1,
    UnsupportedV1,
}

/// Exact supported-fragment result of one independent replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionAnalysisWitnessCoverageV1 {
    Complete {
        obligation_count: usize,
    },
    Incomplete {
        gap: ProductionAnalysisWitnessGapV1,
        reason: String,
    },
}

impl ProductionAnalysisWitnessCoverageV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::Complete { .. } => KernelCheckStatusV1::Clean,
            Self::Incomplete { .. } => KernelCheckStatusV1::Incomplete,
        }
    }

    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete { .. })
    }

    pub const fn obligation_count(&self) -> usize {
        match self {
            Self::Complete { obligation_count } => *obligation_count,
            Self::Incomplete { .. } => 0,
        }
    }

    pub const fn gap(&self) -> Option<ProductionAnalysisWitnessGapV1> {
        match self {
            Self::Complete { .. } => None,
            Self::Incomplete { gap, .. } => Some(*gap),
        }
    }

    pub fn incomplete_reason(&self) -> Option<&str> {
        match self {
            Self::Complete { .. } => None,
            Self::Incomplete { reason, .. } => Some(reason),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BoundsPresburgerObligationV1 {
    block: usize,
    operation: usize,
    dimension: usize,
    extent: u64,
    checked_invocations: u64,
    normalized_map: PresburgerMapV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BoundsPresburgerWitnessV1 {
    obligations: Vec<BoundsPresburgerObligationV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProductionAnalysisWitnessPayloadV1 {
    Bounds(BoundsPresburgerWitnessV1),
    Incomplete {
        gap: ProductionAnalysisWitnessGapV1,
        reason: String,
    },
}

/// Compiler-issued, subject-bound witness envelope for one sealed report.
///
/// All authority-bearing identity fields and the payload are private. Public
/// consumers can inspect replay coverage, but cannot construct or modify an
/// envelope or convert it into another compiler capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionAnalysisWitnessEnvelopeV1 {
    context_identity: Option<ContextIdentity>,
    function: Ptr<Operation>,
    checkpoint: ProductionAnalysisCheckpointV1,
    implementation: ProductionAnalysisImplementationV1,
    configuration: ProductionAnalysisConfigurationV1,
    report: CapturedProductionAnalysisReportV1,
    checker: ProductionAnalysisWitnessCheckerV1,
    payload: ProductionAnalysisWitnessPayloadV1,
    coverage: ProductionAnalysisWitnessCoverageV1,
}

impl ProductionAnalysisWitnessEnvelopeV1 {
    pub const fn checkpoint(&self) -> ProductionAnalysisCheckpointV1 {
        self.checkpoint
    }

    pub const fn implementation(&self) -> ProductionAnalysisImplementationV1 {
        self.implementation
    }

    pub const fn configuration(&self) -> &ProductionAnalysisConfigurationV1 {
        &self.configuration
    }

    pub const fn checker(&self) -> ProductionAnalysisWitnessCheckerV1 {
        self.checker
    }

    pub const fn coverage(&self) -> &ProductionAnalysisWitnessCoverageV1 {
        &self.coverage
    }

    pub const fn status(&self) -> KernelCheckStatusV1 {
        self.coverage.status()
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_lowering_or_launch_authority(&self) -> bool {
        false
    }
}

/// Integrity failures from replaying an issued witness envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionAnalysisWitnessValidationErrorV1 {
    SubjectMismatch,
    BindingMismatch {
        component: &'static str,
    },
    MutationEpochUnavailable,
    MutationEpochMismatch {
        expected: u64,
        observed: u64,
    },
    ReportMismatch,
    PayloadMismatch,
    BoundsCounterexample {
        block: usize,
        operation: usize,
        dimension: usize,
        invocation: Vec<u64>,
        index: u64,
        extent: u64,
    },
    BoundsMachineOverflow {
        block: usize,
        operation: usize,
        dimension: usize,
        invocation: Vec<u64>,
        operator: &'static str,
    },
}

impl ProductionAnalysisWitnessValidationErrorV1 {
    pub const fn code(&self) -> &'static str {
        "FE2O3-PRESERVE-045"
    }
}

impl fmt::Display for ProductionAnalysisWitnessValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "error[{}]: ", self.code())?;
        match self {
            Self::SubjectMismatch => formatter.write_str(
                "analysis witness belongs to a different PLIRON context or function",
            ),
            Self::BindingMismatch { component } => write!(
                formatter,
                "analysis witness {component} differs from its sealed report checkpoint",
            ),
            Self::MutationEpochUnavailable => formatter.write_str(
                "the PLIRON mutation-attempt epoch is unavailable while replaying an analysis witness",
            ),
            Self::MutationEpochMismatch { expected, observed } => write!(
                formatter,
                "analysis witness checkpoint mutation epoch changed from {expected} to {observed}",
            ),
            Self::ReportMismatch => formatter.write_str(
                "analysis witness report payload or status differs from the sealed report",
            ),
            Self::PayloadMismatch => formatter.write_str(
                "analysis witness omitted, substituted, reordered, or forged a live-IR obligation",
            ),
            Self::BoundsCounterexample {
                block,
                operation,
                dimension,
                invocation,
                index,
                extent,
            } => write!(
                formatter,
                "bounds witness replay found a counterexample at block {block} op {operation} dimension {dimension}: invocation {invocation:?} evaluates to {index}, outside extent {extent}",
            ),
            Self::BoundsMachineOverflow {
                block,
                operation,
                dimension,
                invocation,
                operator,
            } => write!(
                formatter,
                "bounds witness replay found checked unsigned {operator} overflow at block {block} op {operation} dimension {dimension} for invocation {invocation:?}",
            ),
        }
    }
}

impl std::error::Error for ProductionAnalysisWitnessValidationErrorV1 {}

enum SupportedWitnessBuildV1<T> {
    Complete(T),
    Incomplete(String),
}

fn current_mutation_epoch(
    context: &Context,
) -> Result<u64, ProductionAnalysisWitnessValidationErrorV1> {
    context
        .ir_mutation_attempt_epoch()
        .map(|epoch| epoch.value())
        .map_err(|_| ProductionAnalysisWitnessValidationErrorV1::MutationEpochUnavailable)
}

pub(crate) fn issue_and_validate_production_analysis_witness_v1(
    context: &Context,
    function: &FuncOp,
    checkpoint: ProductionAnalysisCheckpointV1,
    implementation: ProductionAnalysisImplementationV1,
    configuration: ProductionAnalysisConfigurationV1,
    report: CapturedProductionAnalysisReportV1,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<ProductionAnalysisWitnessEnvelopeV1, ProductionAnalysisWitnessValidationErrorV1> {
    let observed_epoch = current_mutation_epoch(context)?;
    if observed_epoch != checkpoint.mutation_epoch() {
        return Err(
            ProductionAnalysisWitnessValidationErrorV1::MutationEpochMismatch {
                expected: checkpoint.mutation_epoch(),
                observed: observed_epoch,
            },
        );
    }

    let context_identity = require_context_identity(context).ok();
    let pass = report.pass();
    let (checker, payload, coverage) = match (&report, context_identity) {
        (CapturedProductionAnalysisReportV1::Bounds(_), None) => {
            let gap = witness_gap(pass);
            let reason = "bounds replay cannot complete because this PLIRON context has no compiler-owned ContextIdentity".to_owned();
            (
                ProductionAnalysisWitnessCheckerV1::BoundsExhaustiveRawIrReplayV1,
                ProductionAnalysisWitnessPayloadV1::Incomplete {
                    gap,
                    reason: reason.clone(),
                },
                ProductionAnalysisWitnessCoverageV1::Incomplete { gap, reason },
            )
        }
        (CapturedProductionAnalysisReportV1::Bounds(bounds), Some(_)) => {
            match build_bounds_presburger_witness(context, function, bounds, analyses)? {
                SupportedWitnessBuildV1::Complete(witness) => {
                    let obligation_count = witness.obligations.len();
                    (
                        ProductionAnalysisWitnessCheckerV1::BoundsExhaustiveRawIrReplayV1,
                        ProductionAnalysisWitnessPayloadV1::Bounds(witness),
                        ProductionAnalysisWitnessCoverageV1::Complete { obligation_count },
                    )
                }
                SupportedWitnessBuildV1::Incomplete(reason) => {
                    let gap = witness_gap(pass);
                    let reason = bounded_witness_reason_v1(reason);
                    (
                        ProductionAnalysisWitnessCheckerV1::BoundsExhaustiveRawIrReplayV1,
                        ProductionAnalysisWitnessPayloadV1::Incomplete {
                            gap,
                            reason: reason.clone(),
                        },
                        ProductionAnalysisWitnessCoverageV1::Incomplete { gap, reason },
                    )
                }
            }
        }
        _ => {
            let gap = witness_gap(pass);
            let reason = bounded_witness_reason_v1(format!(
                "{} witness replay is not implemented in V1; required evidence: {}",
                pass.name(),
                gap.required_evidence()
            ));
            (
                ProductionAnalysisWitnessCheckerV1::UnsupportedV1,
                ProductionAnalysisWitnessPayloadV1::Incomplete {
                    gap,
                    reason: reason.clone(),
                },
                ProductionAnalysisWitnessCoverageV1::Incomplete { gap, reason },
            )
        }
    };

    let envelope = ProductionAnalysisWitnessEnvelopeV1 {
        context_identity,
        function: function.get_operation(),
        checkpoint,
        implementation,
        configuration,
        report,
        checker,
        payload,
        coverage,
    };
    validate_production_analysis_witness_v1(
        context,
        function,
        ExpectedProductionAnalysisWitnessV1 {
            checkpoint,
            implementation,
            configuration: envelope.configuration(),
            report: &envelope.report,
        },
        &envelope,
        analyses,
    )?;
    Ok(envelope)
}

struct ExpectedProductionAnalysisWitnessV1<'a> {
    checkpoint: ProductionAnalysisCheckpointV1,
    implementation: ProductionAnalysisImplementationV1,
    configuration: &'a ProductionAnalysisConfigurationV1,
    report: &'a CapturedProductionAnalysisReportV1,
}

fn validate_production_analysis_witness_v1(
    context: &Context,
    function: &FuncOp,
    expected: ExpectedProductionAnalysisWitnessV1<'_>,
    envelope: &ProductionAnalysisWitnessEnvelopeV1,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<(), ProductionAnalysisWitnessValidationErrorV1> {
    let ExpectedProductionAnalysisWitnessV1 {
        checkpoint,
        implementation,
        configuration,
        report,
    } = expected;
    if envelope.context_identity != require_context_identity(context).ok()
        || envelope.function != function.get_operation()
    {
        return Err(ProductionAnalysisWitnessValidationErrorV1::SubjectMismatch);
    }
    if envelope.checkpoint != checkpoint {
        return Err(
            ProductionAnalysisWitnessValidationErrorV1::BindingMismatch {
                component: "checkpoint",
            },
        );
    }
    if envelope.implementation != implementation {
        return Err(
            ProductionAnalysisWitnessValidationErrorV1::BindingMismatch {
                component: "implementation",
            },
        );
    }
    if envelope.configuration != *configuration {
        return Err(
            ProductionAnalysisWitnessValidationErrorV1::BindingMismatch {
                component: "configuration",
            },
        );
    }
    if envelope.report != *report
        || envelope.report.pass() != checkpoint.pass()
        || envelope.report.status() != report.status()
    {
        return Err(ProductionAnalysisWitnessValidationErrorV1::ReportMismatch);
    }
    let before = current_mutation_epoch(context)?;
    if before != checkpoint.mutation_epoch() {
        return Err(
            ProductionAnalysisWitnessValidationErrorV1::MutationEpochMismatch {
                expected: checkpoint.mutation_epoch(),
                observed: before,
            },
        );
    }

    match (&envelope.report, &envelope.payload, &envelope.coverage) {
        (
            CapturedProductionAnalysisReportV1::Bounds(bounds),
            ProductionAnalysisWitnessPayloadV1::Bounds(witness),
            ProductionAnalysisWitnessCoverageV1::Complete { obligation_count },
        ) if envelope.checker
            == ProductionAnalysisWitnessCheckerV1::BoundsExhaustiveRawIrReplayV1 =>
        {
            if envelope.context_identity.is_none() {
                return Err(ProductionAnalysisWitnessValidationErrorV1::PayloadMismatch);
            }
            if *obligation_count != witness.obligations.len() {
                return Err(ProductionAnalysisWitnessValidationErrorV1::PayloadMismatch);
            }
            match build_bounds_presburger_witness(context, function, bounds, analyses)? {
                SupportedWitnessBuildV1::Complete(replayed) if replayed == *witness => {}
                SupportedWitnessBuildV1::Complete(_) | SupportedWitnessBuildV1::Incomplete(_) => {
                    return Err(ProductionAnalysisWitnessValidationErrorV1::PayloadMismatch);
                }
            }
        }
        (
            _,
            ProductionAnalysisWitnessPayloadV1::Incomplete {
                gap: payload_gap,
                reason: payload_reason,
            },
            ProductionAnalysisWitnessCoverageV1::Incomplete { gap, reason },
        ) if envelope.checker == ProductionAnalysisWitnessCheckerV1::UnsupportedV1
            || envelope.checker
                == ProductionAnalysisWitnessCheckerV1::BoundsExhaustiveRawIrReplayV1 =>
        {
            if payload_gap != gap
                || payload_reason != reason
                || gap.pass() != checkpoint.pass()
                || reason.is_empty()
            {
                return Err(ProductionAnalysisWitnessValidationErrorV1::PayloadMismatch);
            }
        }
        _ => return Err(ProductionAnalysisWitnessValidationErrorV1::PayloadMismatch),
    }

    let after = current_mutation_epoch(context)?;
    if after != before {
        return Err(
            ProductionAnalysisWitnessValidationErrorV1::MutationEpochMismatch {
                expected: before,
                observed: after,
            },
        );
    }
    Ok(())
}

fn build_bounds_presburger_witness(
    context: &Context,
    function: &FuncOp,
    report: &RankedBoundsReportV1,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<
    SupportedWitnessBuildV1<BoundsPresburgerWitnessV1>,
    ProductionAnalysisWitnessValidationErrorV1,
> {
    analyses.prepare_function_inventory(context, function);
    let inventory = match analyses.function_inventory_handle() {
        Ok(inventory) => inventory,
        Err(failure) => {
            return Ok(SupportedWitnessBuildV1::Incomplete(format!(
                "bounds witness function inventory {} count {} exceeds limit {}",
                failure.resource(),
                failure.actual(),
                failure.limit(),
            )));
        }
    };
    let blocks = inventory.blocks();
    if blocks.len() != 1 {
        return Ok(SupportedWitnessBuildV1::Incomplete(
            "bounds witness V1 cannot yet enumerate exhaustive CFG path domains or dominating guard facts"
                .to_owned(),
        ));
    }

    let block_operations = inventory.block_operations(0);
    let launch_extents = match raw_launch_extents(context, block_operations) {
        Ok(extents) => extents,
        Err(reason) => return Ok(SupportedWitnessBuildV1::Incomplete(reason)),
    };
    let invocation_count = match exhaustive_invocation_count(&launch_extents) {
        Ok(count) => count,
        Err(reason) => return Ok(SupportedWitnessBuildV1::Incomplete(reason)),
    };
    if report.status() != KernelCheckStatusV1::Clean {
        return Ok(SupportedWitnessBuildV1::Incomplete(
            "the raw launch domain is supported, but only a Clean bounds report can be replayed as a positive witness"
                .to_owned(),
        ));
    }

    // This transcript is useful for auditing the production analysis, but it
    // is deliberately not the authority for `Complete`. The separate raw-IR
    // evaluator below interprets the defining operation DAG directly and
    // enumerates every invocation in the finite launch box.
    analyses.prepare_sparse_indices(context, function);
    analyses.prepare_presburger(context, function);
    let sparse = match analyses.sparse_indices() {
        Ok(sparse) => sparse,
        Err(failure) => {
            return Ok(SupportedWitnessBuildV1::Incomplete(format!(
                "sparse-index witness construction is incomplete: {failure:?}"
            )));
        }
    };
    let presburger = match analyses.presburger() {
        Ok(presburger) => presburger,
        Err(failure) => {
            return Ok(SupportedWitnessBuildV1::Incomplete(format!(
                "Presburger witness construction is incomplete: {failure:?}"
            )));
        }
    };
    let evaluation_stack_frame_limit =
        raw_index_stack_frame_upper_bound_v1(inventory.operations().len())
            .map_err(|_| ProductionAnalysisWitnessValidationErrorV1::PayloadMismatch)?;

    let mut obligations = Vec::new();
    let mut evaluation_steps = 0usize;
    for site in block_operations {
        let operation_index = site.operation();
        let operation = Operation::get_op_dyn(site.pointer(), context);
        let Some(access) = operation.downcast_ref::<RankedAccessOp>() else {
            continue;
        };
        let view = access.view(context);
        let Some(view_type) = ranked_view_type(view, context) else {
            return Err(ProductionAnalysisWitnessValidationErrorV1::PayloadMismatch);
        };
        let view_type: TypedHandle<dialect_kernel::RankedViewType> = view_type;
        let view_type = view_type.deref(context);
        for (dimension, index) in access.indices(context).into_iter().enumerate() {
            let Some(extent) = view_type.shape().get(dimension).copied() else {
                return Err(ProductionAnalysisWitnessValidationErrorV1::PayloadMismatch);
            };
            if extent == DYNAMIC_EXTENT {
                return Ok(SupportedWitnessBuildV1::Incomplete(format!(
                    "bounds witness V1 cannot enumerate dynamic extent at block 0 op {operation_index} dimension {dimension}"
                )));
            }

            if let Err(failure) = exhaustively_check_raw_index(
                context,
                index,
                &launch_extents,
                extent,
                &mut evaluation_steps,
                evaluation_stack_frame_limit,
                RawBoundsReplaySiteV1 {
                    operation: operation_index,
                    dimension,
                },
            ) {
                match failure {
                    RawBoundsReplayFailureV1::Incomplete(reason) => {
                        return Ok(SupportedWitnessBuildV1::Incomplete(reason));
                    }
                    RawBoundsReplayFailureV1::Counterexample(error) => return Err(error),
                }
            }

            let fact = sparse.fact(index);
            if matches!(fact, SparseIndexFactV1::MachineOverflow(_)) {
                return Err(ProductionAnalysisWitnessValidationErrorV1::PayloadMismatch);
            }
            let normalized_map = match presburger.map_for_facts(&[fact]) {
                Ok(map) => map,
                Err(failure) => {
                    return Ok(SupportedWitnessBuildV1::Incomplete(format!(
                        "bounds witness V1 cannot capture the Presburger transcript at block 0 op {operation_index} dimension {dimension}: {failure}"
                    )));
                }
            };
            obligations.push(BoundsPresburgerObligationV1 {
                block: 0,
                operation: operation_index,
                dimension,
                extent,
                checked_invocations: invocation_count,
                normalized_map,
            });
        }
    }
    Ok(SupportedWitnessBuildV1::Complete(
        BoundsPresburgerWitnessV1 { obligations },
    ))
}

fn raw_launch_extents(
    context: &Context,
    operations: &[crate::production_analysis::pliron_function_inventory::PlironOperationSiteV1],
) -> Result<Vec<u64>, String> {
    let mut by_dimension = BTreeMap::<usize, u64>::new();
    let mut execution_layout = None;
    for site in operations {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        if let Some(layout) = operation.downcast_ref::<ExecutionLayoutOp>() {
            if execution_layout.is_some() {
                return Err("bounds witness V1 found more than one gpu.execution_layout".to_owned());
            }
            if layout.verify(context).is_err() {
                return Err("bounds witness V1 found a malformed gpu.execution_layout".to_owned());
            }
            let Some(global_extents) = layout.global_extents(context) else {
                return Err("bounds witness V1 found a malformed gpu.execution_layout".to_owned());
            };
            execution_layout = Some(global_extents);
            continue;
        }

        let Some(invocation) = operation.downcast_ref::<InvocationIndexOp>() else {
            continue;
        };
        let Some(dimension) = invocation
            .dimension(context)
            .and_then(|dimension| usize::try_from(dimension).ok())
        else {
            return Err("bounds witness V1 found a malformed invocation dimension".to_owned());
        };
        let Some(extent) = invocation.launch_extent(context) else {
            return Err("bounds witness V1 found a missing launch extent".to_owned());
        };
        if extent == DYNAMIC_EXTENT {
            return Err(format!(
                "bounds witness V1 cannot enumerate dynamic launch dimension {dimension}"
            ));
        }
        if by_dimension.insert(dimension, extent).is_some() {
            return Err(format!(
                "bounds witness V1 found duplicate invocation dimension {dimension}"
            ));
        }
    }

    if let Some(layout_extents) = execution_layout {
        for (dimension, layout_extent) in layout_extents.into_iter().enumerate() {
            if layout_extent == DYNAMIC_EXTENT {
                return Err(format!(
                    "bounds witness V1 cannot enumerate dynamic gpu.execution_layout axis {dimension}"
                ));
            }
            match by_dimension.get(&dimension) {
                Some(invocation_extent) if *invocation_extent == layout_extent => {}
                Some(invocation_extent) => {
                    return Err(format!(
                        "bounds witness V1 invocation dimension {dimension} extent {invocation_extent} is inconsistent with gpu.execution_layout extent {layout_extent}"
                    ));
                }
                None if layout_extent > 1 => {
                    return Err(format!(
                        "bounds witness V1 gpu.execution_layout has active axis {dimension} extent {layout_extent} without an invocation dimension"
                    ));
                }
                None => {}
            }
        }
        if let Some((dimension, _)) = by_dimension.range(3..).next() {
            return Err(format!(
                "bounds witness V1 invocation dimension {dimension} is outside the three-dimensional gpu.execution_layout"
            ));
        }
        return Ok(layout_extents.to_vec());
    }

    let dimension_count = by_dimension
        .last_key_value()
        .map_or(0, |(dimension, _)| dimension + 1);
    let mut extents = vec![1; dimension_count];
    for (dimension, extent) in by_dimension {
        extents[dimension] = extent;
    }
    Ok(extents)
}

fn exhaustive_invocation_count(extents: &[u64]) -> Result<u64, String> {
    let mut count = 1u64;
    for extent in extents {
        count = count.checked_mul(*extent).ok_or_else(|| {
            "bounds witness V1 launch-domain cardinality overflows u64".to_owned()
        })?;
        if count > MAX_BOUNDS_WITNESS_INVOCATIONS_V1 {
            return Err(format!(
                "bounds witness V1 needs {count} invocations, exceeding its exhaustive replay cap of {MAX_BOUNDS_WITNESS_INVOCATIONS_V1}"
            ));
        }
    }
    Ok(count)
}

enum RawBoundsReplayFailureV1 {
    Incomplete(String),
    Counterexample(ProductionAnalysisWitnessValidationErrorV1),
}

struct RawBoundsReplaySiteV1 {
    operation: usize,
    dimension: usize,
}

fn exhaustively_check_raw_index(
    context: &Context,
    index: Value,
    extents: &[u64],
    extent: u64,
    evaluation_steps: &mut usize,
    evaluation_stack_frame_limit: usize,
    site: RawBoundsReplaySiteV1,
) -> Result<(), RawBoundsReplayFailureV1> {
    let RawBoundsReplaySiteV1 {
        operation,
        dimension,
    } = site;
    if extents.contains(&0) {
        return Ok(());
    }
    let mut invocation = vec![0u64; extents.len()];
    loop {
        let mut cache = HashMap::new();
        let mut active = HashSet::new();
        let evaluated = match evaluate_raw_index_iterative(
            context,
            index,
            &invocation,
            &mut cache,
            &mut active,
            evaluation_steps,
            evaluation_stack_frame_limit,
        ) {
            Ok(value) => value,
            Err(RawIndexEvaluationFailureV1::Incomplete(reason)) => {
                return Err(RawBoundsReplayFailureV1::Incomplete(format!(
                    "bounds witness V1 cannot interpret block 0 op {operation} dimension {dimension}: {reason}"
                )));
            }
            Err(RawIndexEvaluationFailureV1::Overflow(operator)) => {
                return Err(RawBoundsReplayFailureV1::Counterexample(
                    ProductionAnalysisWitnessValidationErrorV1::BoundsMachineOverflow {
                        block: 0,
                        operation,
                        dimension,
                        invocation,
                        operator,
                    },
                ));
            }
        };
        if evaluated >= extent {
            return Err(RawBoundsReplayFailureV1::Counterexample(
                ProductionAnalysisWitnessValidationErrorV1::BoundsCounterexample {
                    block: 0,
                    operation,
                    dimension,
                    invocation,
                    index: evaluated,
                    extent,
                },
            ));
        }
        if !increment_invocation(&mut invocation, extents) {
            return Ok(());
        }
    }
}

fn increment_invocation(invocation: &mut [u64], extents: &[u64]) -> bool {
    for dimension in (0..invocation.len()).rev() {
        invocation[dimension] += 1;
        if invocation[dimension] < extents[dimension] {
            return true;
        }
        invocation[dimension] = 0;
    }
    false
}

enum RawIndexEvaluationFailureV1 {
    Incomplete(&'static str),
    Overflow(&'static str),
}

pub(crate) fn evaluate_raw_index_at_invocation_v1(
    context: &Context,
    value: Value,
    invocation: &[u64],
    evaluation_steps: &mut usize,
) -> Option<u64> {
    let evaluation_stack_frame_limit = MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1
        .checked_mul(RAW_INDEX_STACK_FRAMES_PER_OPERATION_V1)?
        .checked_add(RAW_INDEX_STACK_FIXED_FRAMES_V1)?;
    evaluate_raw_index_iterative(
        context,
        value,
        invocation,
        &mut HashMap::new(),
        &mut HashSet::new(),
        evaluation_steps,
        evaluation_stack_frame_limit,
    )
    .ok()
}

#[derive(Clone, Copy)]
enum RawIndexEvaluationFrameV1 {
    Evaluate(Value),
    FinishBinary {
        value: Value,
        lhs: Value,
        rhs: Value,
        kind: Option<IndexBinaryKindAttr>,
    },
}

fn push_raw_index_evaluation_frame_v1(
    stack: &mut Vec<RawIndexEvaluationFrameV1>,
    frame: RawIndexEvaluationFrameV1,
    frame_limit: usize,
) -> Result<(), RawIndexEvaluationFailureV1> {
    if stack.len() == frame_limit {
        return Err(RawIndexEvaluationFailureV1::Incomplete(
            "raw-index evaluation exceeded its deterministic stack cap",
        ));
    }
    stack.try_reserve(1).map_err(|_| {
        RawIndexEvaluationFailureV1::Incomplete("raw-index evaluation stack allocation failed")
    })?;
    stack.push(frame);
    Ok(())
}

fn evaluate_raw_index_iterative(
    context: &Context,
    value: Value,
    invocation: &[u64],
    cache: &mut HashMap<Value, u64>,
    active: &mut HashSet<Value>,
    evaluation_steps: &mut usize,
    evaluation_stack_frame_limit: usize,
) -> Result<u64, RawIndexEvaluationFailureV1> {
    let mut stack = Vec::new();
    push_raw_index_evaluation_frame_v1(
        &mut stack,
        RawIndexEvaluationFrameV1::Evaluate(value),
        evaluation_stack_frame_limit,
    )?;
    while let Some(frame) = stack.pop() {
        match frame {
            RawIndexEvaluationFrameV1::Evaluate(value) => {
                if cache.contains_key(&value) {
                    continue;
                }
                *evaluation_steps = evaluation_steps.saturating_add(1);
                if *evaluation_steps > MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 {
                    return Err(RawIndexEvaluationFailureV1::Incomplete(
                        "raw-index evaluation exceeded its deterministic work cap",
                    ));
                }
                if !active.insert(value) {
                    return Err(RawIndexEvaluationFailureV1::Incomplete(
                        "the raw index definition graph is cyclic",
                    ));
                }
                let Some(definition) = value.defining_op() else {
                    active.remove(&value);
                    return Err(RawIndexEvaluationFailureV1::Incomplete(
                        "block arguments are outside the V1 raw-index fragment",
                    ));
                };
                let operation = Operation::get_op_dyn(definition, context);
                if let Some(constant) = operation.downcast_ref::<IndexConstantOp>() {
                    let Some(result) = constant.value(context) else {
                        active.remove(&value);
                        return Err(RawIndexEvaluationFailureV1::Incomplete(
                            "index constant has no value",
                        ));
                    };
                    active.remove(&value);
                    cache.insert(value, result);
                    continue;
                }
                if let Some(index) = operation.downcast_ref::<InvocationIndexOp>() {
                    let Some(dimension) = index
                        .dimension(context)
                        .and_then(|dimension| usize::try_from(dimension).ok())
                    else {
                        active.remove(&value);
                        return Err(RawIndexEvaluationFailureV1::Incomplete(
                            "invocation index has no valid dimension",
                        ));
                    };
                    let Some(result) = invocation.get(dimension).copied() else {
                        active.remove(&value);
                        return Err(RawIndexEvaluationFailureV1::Incomplete(
                            "invocation dimension is absent from the launch inventory",
                        ));
                    };
                    active.remove(&value);
                    cache.insert(value, result);
                    continue;
                }
                let Some(binary) = operation.downcast_ref::<IndexBinaryOp>() else {
                    active.remove(&value);
                    return Err(RawIndexEvaluationFailureV1::Incomplete(
                        "index producer is not a supported constant, invocation, or binary operation",
                    ));
                };
                let lhs = binary.lhs(context);
                let rhs = binary.rhs(context);
                push_raw_index_evaluation_frame_v1(
                    &mut stack,
                    RawIndexEvaluationFrameV1::FinishBinary {
                        value,
                        lhs,
                        rhs,
                        kind: binary.kind(context),
                    },
                    evaluation_stack_frame_limit,
                )?;
                push_raw_index_evaluation_frame_v1(
                    &mut stack,
                    RawIndexEvaluationFrameV1::Evaluate(rhs),
                    evaluation_stack_frame_limit,
                )?;
                push_raw_index_evaluation_frame_v1(
                    &mut stack,
                    RawIndexEvaluationFrameV1::Evaluate(lhs),
                    evaluation_stack_frame_limit,
                )?;
            }
            RawIndexEvaluationFrameV1::FinishBinary {
                value,
                lhs,
                rhs,
                kind,
            } => {
                let result = match (cache.get(&lhs).copied(), cache.get(&rhs).copied(), kind) {
                    (Some(lhs), Some(rhs), Some(IndexBinaryKindAttr::Add)) => lhs
                        .checked_add(rhs)
                        .ok_or(RawIndexEvaluationFailureV1::Overflow("addition")),
                    (Some(lhs), Some(rhs), Some(IndexBinaryKindAttr::Multiply)) => lhs
                        .checked_mul(rhs)
                        .ok_or(RawIndexEvaluationFailureV1::Overflow("multiplication")),
                    (Some(lhs), Some(rhs), Some(IndexBinaryKindAttr::Remainder)) if rhs != 0 => {
                        Ok(lhs % rhs)
                    }
                    (Some(lhs), Some(rhs), Some(IndexBinaryKindAttr::Divide)) if rhs != 0 => {
                        Ok(lhs / rhs)
                    }
                    (Some(_), Some(_), Some(IndexBinaryKindAttr::Remainder)) => Err(
                        RawIndexEvaluationFailureV1::Incomplete("remainder divisor is zero"),
                    ),
                    (Some(_), Some(_), Some(IndexBinaryKindAttr::Divide)) => Err(
                        RawIndexEvaluationFailureV1::Incomplete("division divisor is zero"),
                    ),
                    (Some(_), Some(_), None) => Err(RawIndexEvaluationFailureV1::Incomplete(
                        "index binary operation has no kind",
                    )),
                    _ => Err(RawIndexEvaluationFailureV1::Incomplete(
                        "raw-index evaluation stack lost an operand result",
                    )),
                };
                active.remove(&value);
                cache.insert(value, result?);
            }
        }
    }
    cache
        .get(&value)
        .copied()
        .ok_or(RawIndexEvaluationFailureV1::Incomplete(
            "raw-index evaluation stack produced no result",
        ))
}

include!("pliron_analysis_witness/resource_tests.rs");
