//! Canonical, non-authoritative host wall-time evidence for a direct-KFD target with and without
//! a rocprofv3 wrapper.

use crate::profile_live_qualification_v1::{
    CollectorArtifactV1, CollectorReleaseV1, RawContentIdentityV1,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

pub(crate) const WRAPPER_OVERHEAD_FILE_V1: &str =
    "fe2o3-rocprof-wrapper-host-wall-comparison-v1.json";
pub(crate) const WRAPPER_OVERHEAD_REDO_FILE_V1: &str =
    ".fe2o3-rocprof-wrapper-host-wall-comparison-v1.redo";
pub(crate) const MAX_WRAPPER_OVERHEAD_BYTES_V1: usize = 4 * 1024 * 1024;
const SCHEMA_V1: &str = "fe2o3-rocprof-wrapper-host-wall-comparison-v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TargetRuntimeBasisV1 {
    CallerDeclaredDirectKfdNotRuntimeVerifiedByHarness,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RequestedCollectorModeV1 {
    KernelTraceJson,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OrderPolicyV1 {
    AlternatingPairsEvenRawFirstOddWrappedFirst,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HostClockV1 {
    LinuxClockMonotonicRaw,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TimingBoundaryV1 {
    ImmediatelyBeforeSpawnThroughSupervisionAndBoundedOutputDrain,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WrappedTargetEnvironmentV1 {
    UnavailableCollectorDerived,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PairOrderV1 {
    RawThenWrapped,
    WrappedThenRaw,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TrialPhaseV1 {
    Warmup,
    Measured,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InvocationKindV1 {
    RawTarget,
    RocprofWrappedTarget,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InvocationOutcomeV1 {
    ExitedSuccess,
    ExitedFailure,
    TimedOut,
    StdoutLimitExceeded,
    StderrLimitExceeded,
    WaitFailed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ArtifactStateV1 {
    NotApplicableRawTarget,
    InventoryCompleteNoArtifacts,
    InventoryCompleteArtifactsPresentUnadmitted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CaptureLossStateV1 {
    NotApplicableRawTarget,
    UnavailableNoAdmittedCapture,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum CaptureOverheadStateV1 {
    #[serde(rename = "unavailable_not_measured")]
    NotMeasured,
    #[serde(rename = "unavailable_no_admitted_capture")]
    NoAdmittedCapture,
    #[serde(rename = "unavailable_artifacts_present_unadmitted")]
    ArtifactsPresentUnadmitted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WrappedInventoryAggregateV1 {
    AllEmpty,
    AllPresentUnadmitted,
    MixedUnadmitted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CandidateBudgetResultV1 {
    WithinCandidateBudget,
    ExceedsCandidateBudget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OverheadPolicyV1 {
    pub(crate) warmup_pairs: u16,
    pub(crate) measured_pairs: u16,
    pub(crate) order_policy: OrderPolicyV1,
    pub(crate) clock: HostClockV1,
    pub(crate) timing_boundary: TimingBoundaryV1,
    pub(crate) per_process_timeout_ms: u64,
    pub(crate) total_processes: u16,
    pub(crate) total_harness_timeout_ms: u64,
    pub(crate) candidate_wrapper_overhead_budget_bps: u64,
    pub(crate) excludes_outliers: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InvocationEvidenceV1 {
    pub(crate) phase: TrialPhaseV1,
    pub(crate) pair_index: u16,
    pub(crate) pair_order: PairOrderV1,
    pub(crate) invocation_index_in_pair: u8,
    pub(crate) kind: InvocationKindV1,
    pub(crate) argv: RawContentIdentityV1,
    pub(crate) host_wall_time_ns: u64,
    pub(crate) outcome: InvocationOutcomeV1,
    pub(crate) exit_code: Option<i32>,
    pub(crate) exit_signal: Option<i32>,
    pub(crate) wait_error: Option<RawContentIdentityV1>,
    pub(crate) stdout: RawContentIdentityV1,
    pub(crate) stdout_truncated: bool,
    pub(crate) stderr: RawContentIdentityV1,
    pub(crate) stderr_truncated: bool,
    pub(crate) artifact_state: ArtifactStateV1,
    pub(crate) capture_loss_state: CaptureLossStateV1,
    pub(crate) wrapped_output_inventory: Vec<CollectorArtifactV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RobustSummaryV1 {
    pub(crate) raw_median_ns: u64,
    pub(crate) wrapped_median_ns: u64,
    pub(crate) raw_p95_ns: u64,
    pub(crate) wrapped_p95_ns: u64,
    pub(crate) paired_overhead_median_bps: i64,
    pub(crate) candidate_budget_result: CandidateBudgetResultV1,
    pub(crate) production_qualified: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectKfdWrapperOverheadV1 {
    pub(crate) schema: String,
    pub(crate) schema_version: u16,
    pub(crate) plan_sha256: [u8; 32],
    pub(crate) measurement_harness_executable: RawContentIdentityV1,
    pub(crate) target_executable: RawContentIdentityV1,
    pub(crate) target_argv: RawContentIdentityV1,
    pub(crate) target_runtime_basis: TargetRuntimeBasisV1,
    pub(crate) requested_collector_mode: RequestedCollectorModeV1,
    pub(crate) collector_executable: RawContentIdentityV1,
    pub(crate) collector_release: CollectorReleaseV1,
    pub(crate) collector_closure: RawContentIdentityV1,
    pub(crate) collector_configuration: RawContentIdentityV1,
    pub(crate) environment: RawContentIdentityV1,
    pub(crate) wrapped_target_environment: WrappedTargetEnvironmentV1,
    pub(crate) working_directory: RawContentIdentityV1,
    pub(crate) working_directory_residual: String,
    pub(crate) device_topology: RawContentIdentityV1,
    pub(crate) policy: OverheadPolicyV1,
    pub(crate) invocations: Vec<InvocationEvidenceV1>,
    pub(crate) summary: Option<RobustSummaryV1>,
    pub(crate) warmup_wrapped_output_inventory: WrappedInventoryAggregateV1,
    pub(crate) measured_wrapped_output_inventory: WrappedInventoryAggregateV1,
    pub(crate) kernel_trace_capture_overhead: CaptureOverheadStateV1,
    pub(crate) counter_capture_overhead: CaptureOverheadStateV1,
    pub(crate) pc_sampling_capture_overhead: CaptureOverheadStateV1,
    pub(crate) att_capture_overhead: CaptureOverheadStateV1,
    pub(crate) debugger_capture_overhead: CaptureOverheadStateV1,
    pub(crate) grants_collection_authority: bool,
    pub(crate) grants_production_qualification: bool,
}

pub(crate) struct BuildInputsV1 {
    pub(crate) plan_sha256: [u8; 32],
    pub(crate) measurement_harness_executable: RawContentIdentityV1,
    pub(crate) target_executable: RawContentIdentityV1,
    pub(crate) target_argv: RawContentIdentityV1,
    pub(crate) collector_executable: RawContentIdentityV1,
    pub(crate) collector_release: CollectorReleaseV1,
    pub(crate) collector_closure: RawContentIdentityV1,
    pub(crate) collector_configuration: RawContentIdentityV1,
    pub(crate) environment: RawContentIdentityV1,
    pub(crate) working_directory: RawContentIdentityV1,
    pub(crate) device_topology: RawContentIdentityV1,
    pub(crate) policy: OverheadPolicyV1,
    pub(crate) invocations: Vec<InvocationEvidenceV1>,
}

pub(crate) fn build_wrapper_overhead_v1(
    inputs: BuildInputsV1,
) -> Result<DirectKfdWrapperOverheadV1, WrapperOverheadErrorV1> {
    let warmup_inventory = inventory_aggregate(&inputs.invocations, TrialPhaseV1::Warmup)?;
    let measured_inventory = inventory_aggregate(&inputs.invocations, TrialPhaseV1::Measured)?;
    let kernel_trace_capture_overhead =
        if measured_inventory == WrappedInventoryAggregateV1::AllEmpty {
            CaptureOverheadStateV1::NoAdmittedCapture
        } else {
            CaptureOverheadStateV1::ArtifactsPresentUnadmitted
        };
    let summary = summarize(&inputs.policy, &inputs.invocations)?;
    let record = DirectKfdWrapperOverheadV1 {
        schema: SCHEMA_V1.to_owned(),
        schema_version: 1,
        plan_sha256: inputs.plan_sha256,
        measurement_harness_executable: inputs.measurement_harness_executable,
        target_executable: inputs.target_executable,
        target_argv: inputs.target_argv,
        target_runtime_basis:
            TargetRuntimeBasisV1::CallerDeclaredDirectKfdNotRuntimeVerifiedByHarness,
        requested_collector_mode: RequestedCollectorModeV1::KernelTraceJson,
        collector_executable: inputs.collector_executable,
        collector_release: inputs.collector_release,
        collector_closure: inputs.collector_closure,
        collector_configuration: inputs.collector_configuration,
        environment: inputs.environment,
        wrapped_target_environment: WrappedTargetEnvironmentV1::UnavailableCollectorDerived,
        working_directory: inputs.working_directory,
        working_directory_residual:
            "path-revalidated-before-and-after-each-leg-non_atomic-change-after_pre_spawn_remains"
                .to_owned(),
        device_topology: inputs.device_topology,
        policy: inputs.policy,
        invocations: inputs.invocations,
        summary,
        warmup_wrapped_output_inventory: warmup_inventory,
        measured_wrapped_output_inventory: measured_inventory,
        kernel_trace_capture_overhead,
        counter_capture_overhead: CaptureOverheadStateV1::NotMeasured,
        pc_sampling_capture_overhead: CaptureOverheadStateV1::NotMeasured,
        att_capture_overhead: CaptureOverheadStateV1::NotMeasured,
        debugger_capture_overhead: CaptureOverheadStateV1::NotMeasured,
        grants_collection_authority: false,
        grants_production_qualification: false,
    };
    record.validate()?;
    Ok(record)
}

fn inventory_aggregate(
    invocations: &[InvocationEvidenceV1],
    phase: TrialPhaseV1,
) -> Result<WrappedInventoryAggregateV1, WrapperOverheadErrorV1> {
    let wrapped = invocations
        .iter()
        .filter(|invocation| {
            invocation.phase == phase && invocation.kind == InvocationKindV1::RocprofWrappedTarget
        })
        .collect::<Vec<_>>();
    if wrapped.is_empty() {
        return Err(WrapperOverheadErrorV1::InvalidRecord);
    }
    let present = wrapped
        .iter()
        .filter(|invocation| !invocation.wrapped_output_inventory.is_empty())
        .count();
    Ok(if present == 0 {
        WrappedInventoryAggregateV1::AllEmpty
    } else if present == wrapped.len() {
        WrappedInventoryAggregateV1::AllPresentUnadmitted
    } else {
        WrappedInventoryAggregateV1::MixedUnadmitted
    })
}

fn summarize(
    policy: &OverheadPolicyV1,
    invocations: &[InvocationEvidenceV1],
) -> Result<Option<RobustSummaryV1>, WrapperOverheadErrorV1> {
    if invocations.iter().any(|invocation| {
        invocation.outcome != InvocationOutcomeV1::ExitedSuccess
            || invocation.stdout_truncated
            || invocation.stderr_truncated
    }) {
        return Ok(None);
    }
    let measured: Vec<_> = invocations
        .iter()
        .filter(|invocation| invocation.phase == TrialPhaseV1::Measured)
        .collect();
    let mut raw = Vec::with_capacity(usize::from(policy.measured_pairs));
    let mut wrapped = Vec::with_capacity(usize::from(policy.measured_pairs));
    let mut paired_bps = Vec::with_capacity(usize::from(policy.measured_pairs));
    for pair in 0..policy.measured_pairs {
        let raw_time = measured
            .iter()
            .find(|trial| trial.pair_index == pair && trial.kind == InvocationKindV1::RawTarget)
            .ok_or(WrapperOverheadErrorV1::InvalidRecord)?
            .host_wall_time_ns;
        let wrapped_time = measured
            .iter()
            .find(|trial| {
                trial.pair_index == pair && trial.kind == InvocationKindV1::RocprofWrappedTarget
            })
            .ok_or(WrapperOverheadErrorV1::InvalidRecord)?
            .host_wall_time_ns;
        if raw_time == 0 || wrapped_time == 0 {
            return Err(WrapperOverheadErrorV1::InvalidRecord);
        }
        raw.push(raw_time);
        wrapped.push(wrapped_time);
        let difference = i128::from(wrapped_time) - i128::from(raw_time);
        let bps = difference
            .checked_mul(10_000)
            .and_then(|value| value.checked_div(i128::from(raw_time)))
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(WrapperOverheadErrorV1::SizeOverflow)?;
        paired_bps.push(bps);
    }
    raw.sort_unstable();
    wrapped.sort_unstable();
    paired_bps.sort_unstable();
    let overhead = median_i64(&paired_bps);
    let budget = i64::try_from(policy.candidate_wrapper_overhead_budget_bps)
        .map_err(|_| WrapperOverheadErrorV1::SizeOverflow)?;
    Ok(Some(RobustSummaryV1 {
        raw_median_ns: median_u64(&raw),
        wrapped_median_ns: median_u64(&wrapped),
        raw_p95_ns: nearest_rank_p95(&raw),
        wrapped_p95_ns: nearest_rank_p95(&wrapped),
        paired_overhead_median_bps: overhead,
        candidate_budget_result: if overhead <= budget {
            CandidateBudgetResultV1::WithinCandidateBudget
        } else {
            CandidateBudgetResultV1::ExceedsCandidateBudget
        },
        production_qualified: false,
    }))
}

fn median_u64(values: &[u64]) -> u64 {
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        values[middle - 1] / 2
            + values[middle] / 2
            + (values[middle - 1] % 2 + values[middle] % 2) / 2
    } else {
        values[middle]
    }
}

fn median_i64(values: &[i64]) -> i64 {
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        let sum = i128::from(values[middle - 1]) + i128::from(values[middle]);
        i64::try_from(sum / 2).expect("the average of two i64 values fits i64")
    } else {
        values[middle]
    }
}

fn nearest_rank_p95(values: &[u64]) -> u64 {
    let rank = values.len().saturating_mul(95).div_ceil(100).max(1);
    values[rank - 1]
}

impl DirectKfdWrapperOverheadV1 {
    pub(crate) fn validate(&self) -> Result<(), WrapperOverheadErrorV1> {
        if self.schema != SCHEMA_V1
            || self.schema_version != 1
            || self.plan_sha256 == [0; 32]
            || self.target_runtime_basis
                != TargetRuntimeBasisV1::CallerDeclaredDirectKfdNotRuntimeVerifiedByHarness
            || self.requested_collector_mode != RequestedCollectorModeV1::KernelTraceJson
            || self.policy.warmup_pairs == 0
            || self.policy.warmup_pairs > 20
            || self.policy.measured_pairs == 0
            || self.policy.measured_pairs > 100
            || self.policy.order_policy
                != OrderPolicyV1::AlternatingPairsEvenRawFirstOddWrappedFirst
            || self.policy.clock != HostClockV1::LinuxClockMonotonicRaw
            || self.policy.timing_boundary
                != TimingBoundaryV1::ImmediatelyBeforeSpawnThroughSupervisionAndBoundedOutputDrain
            || self.policy.per_process_timeout_ms == 0
            || self.policy.per_process_timeout_ms > 900_000
            || self.policy.total_harness_timeout_ms != 3_600_000
            || self.policy.candidate_wrapper_overhead_budget_bps > 1_000_000
            || self.policy.excludes_outliers
            || self.grants_collection_authority
            || self.grants_production_qualification
            || self.wrapped_target_environment
                != WrappedTargetEnvironmentV1::UnavailableCollectorDerived
            || self.working_directory_residual
                != "path-revalidated-before-and-after-each-leg-non_atomic-change-after_pre_spawn_remains"
            || self.counter_capture_overhead != CaptureOverheadStateV1::NotMeasured
            || self.pc_sampling_capture_overhead != CaptureOverheadStateV1::NotMeasured
            || self.att_capture_overhead != CaptureOverheadStateV1::NotMeasured
            || self.debugger_capture_overhead != CaptureOverheadStateV1::NotMeasured
        {
            return Err(WrapperOverheadErrorV1::InvalidRecord);
        }
        let expected_processes = self
            .policy
            .warmup_pairs
            .checked_add(self.policy.measured_pairs)
            .and_then(|pairs| pairs.checked_mul(2))
            .ok_or(WrapperOverheadErrorV1::SizeOverflow)?;
        if self.policy.total_processes != expected_processes
            || self.invocations.len() != usize::from(expected_processes)
        {
            return Err(WrapperOverheadErrorV1::InvalidRecord);
        }
        for identity in [
            self.target_executable,
            self.measurement_harness_executable,
            self.target_argv,
            self.collector_executable,
            self.collector_closure,
            self.collector_configuration,
            self.environment,
            self.device_topology,
            self.working_directory,
        ] {
            if identity.sha256 == [0; 32] || identity.byte_len == 0 {
                return Err(WrapperOverheadErrorV1::InvalidRecord);
            }
        }
        let mut offset = 0_usize;
        for (phase, pairs) in [
            (TrialPhaseV1::Warmup, self.policy.warmup_pairs),
            (TrialPhaseV1::Measured, self.policy.measured_pairs),
        ] {
            for pair in 0..pairs {
                let order = if pair.is_multiple_of(2) {
                    PairOrderV1::RawThenWrapped
                } else {
                    PairOrderV1::WrappedThenRaw
                };
                let expected = match order {
                    PairOrderV1::RawThenWrapped => [
                        InvocationKindV1::RawTarget,
                        InvocationKindV1::RocprofWrappedTarget,
                    ],
                    PairOrderV1::WrappedThenRaw => [
                        InvocationKindV1::RocprofWrappedTarget,
                        InvocationKindV1::RawTarget,
                    ],
                };
                for (leg, kind) in expected.into_iter().enumerate() {
                    let invocation = self
                        .invocations
                        .get(offset)
                        .ok_or(WrapperOverheadErrorV1::InvalidRecord)?;
                    if invocation.phase != phase
                        || invocation.pair_index != pair
                        || invocation.pair_order != order
                        || invocation.invocation_index_in_pair != leg as u8
                        || invocation.kind != kind
                        || invocation.host_wall_time_ns == 0
                        || invocation.argv.sha256 == [0; 32]
                        || invocation.argv.byte_len == 0
                        || invocation.stdout.sha256 == [0; 32]
                        || invocation.stderr.sha256 == [0; 32]
                    {
                        return Err(WrapperOverheadErrorV1::InvalidRecord);
                    }
                    match invocation.outcome {
                        InvocationOutcomeV1::ExitedSuccess
                            if invocation.exit_code != Some(0)
                                || invocation.exit_signal.is_some()
                                || invocation.stdout_truncated
                                || invocation.stderr_truncated =>
                        {
                            return Err(WrapperOverheadErrorV1::InvalidRecord);
                        }
                        InvocationOutcomeV1::ExitedFailure
                            if (invocation.exit_code.is_none()
                                == invocation.exit_signal.is_none())
                                || invocation.exit_code == Some(0)
                                || invocation.stdout_truncated
                                || invocation.stderr_truncated =>
                        {
                            return Err(WrapperOverheadErrorV1::InvalidRecord);
                        }
                        InvocationOutcomeV1::StdoutLimitExceeded
                            if !invocation.stdout_truncated =>
                        {
                            return Err(WrapperOverheadErrorV1::InvalidRecord);
                        }
                        InvocationOutcomeV1::StderrLimitExceeded
                            if !invocation.stderr_truncated =>
                        {
                            return Err(WrapperOverheadErrorV1::InvalidRecord);
                        }
                        _ => {}
                    }
                    if (invocation.outcome == InvocationOutcomeV1::WaitFailed)
                        != invocation.wait_error.is_some()
                    {
                        return Err(WrapperOverheadErrorV1::InvalidRecord);
                    }
                    if invocation.wait_error.is_some_and(|identity| {
                        identity.sha256 == [0; 32] || identity.byte_len == 0
                    }) {
                        return Err(WrapperOverheadErrorV1::InvalidRecord);
                    }
                    let mut previous = None;
                    if invocation.wrapped_output_inventory.len() > 4096 {
                        return Err(WrapperOverheadErrorV1::InvalidRecord);
                    }
                    for artifact in &invocation.wrapped_output_inventory {
                        if artifact.relative_path.is_empty()
                            || artifact.relative_path.len() > 4096
                            || artifact.relative_path.starts_with('/')
                            || artifact.relative_path.contains('\\')
                            || artifact.relative_path.split('/').any(|component| {
                                component.is_empty() || component == "." || component == ".."
                            })
                            || artifact.content.sha256 == [0; 32]
                            || artifact.content.byte_len == 0
                            || previous
                                .as_ref()
                                .is_some_and(|path| path >= &artifact.relative_path)
                        {
                            return Err(WrapperOverheadErrorV1::InvalidRecord);
                        }
                        previous = Some(artifact.relative_path.clone());
                    }
                    match invocation.kind {
                        InvocationKindV1::RawTarget
                            if invocation.artifact_state
                                != ArtifactStateV1::NotApplicableRawTarget
                                || invocation.capture_loss_state
                                    != CaptureLossStateV1::NotApplicableRawTarget
                                || !invocation.wrapped_output_inventory.is_empty() =>
                        {
                            return Err(WrapperOverheadErrorV1::InvalidRecord);
                        }
                        InvocationKindV1::RocprofWrappedTarget
                            if invocation.capture_loss_state
                                != CaptureLossStateV1::UnavailableNoAdmittedCapture
                                || (invocation.wrapped_output_inventory.is_empty()
                                && invocation.artifact_state
                                    != ArtifactStateV1::InventoryCompleteNoArtifacts)
                                || (!invocation.wrapped_output_inventory.is_empty()
                                    && invocation.artifact_state
                                        != ArtifactStateV1::InventoryCompleteArtifactsPresentUnadmitted) =>
                        {
                            return Err(WrapperOverheadErrorV1::InvalidRecord);
                        }
                        _ => {}
                    }
                    offset += 1;
                }
            }
        }
        let warmup_inventory = inventory_aggregate(&self.invocations, TrialPhaseV1::Warmup)?;
        let measured_inventory = inventory_aggregate(&self.invocations, TrialPhaseV1::Measured)?;
        let expected_capture = if measured_inventory == WrappedInventoryAggregateV1::AllEmpty {
            CaptureOverheadStateV1::NoAdmittedCapture
        } else {
            CaptureOverheadStateV1::ArtifactsPresentUnadmitted
        };
        if self.warmup_wrapped_output_inventory != warmup_inventory
            || self.measured_wrapped_output_inventory != measured_inventory
            || self.kernel_trace_capture_overhead != expected_capture
            || self.summary != summarize(&self.policy, &self.invocations)?
            || self
                .summary
                .as_ref()
                .is_some_and(|summary| summary.production_qualified)
        {
            return Err(WrapperOverheadErrorV1::InvalidRecord);
        }
        Ok(())
    }
}

pub(crate) fn encode_wrapper_overhead_v1(
    record: &DirectKfdWrapperOverheadV1,
) -> Result<Vec<u8>, WrapperOverheadErrorV1> {
    record.validate()?;
    let bytes = serde_json::to_vec(record).map_err(|_| WrapperOverheadErrorV1::JsonEncode)?;
    if bytes.is_empty() || bytes.len() > MAX_WRAPPER_OVERHEAD_BYTES_V1 {
        return Err(WrapperOverheadErrorV1::SizeOverflow);
    }
    Ok(bytes)
}

pub(crate) fn decode_wrapper_overhead_v1(
    bytes: &[u8],
) -> Result<DirectKfdWrapperOverheadV1, WrapperOverheadErrorV1> {
    if bytes.is_empty() || bytes.len() > MAX_WRAPPER_OVERHEAD_BYTES_V1 {
        return Err(WrapperOverheadErrorV1::SizeOverflow);
    }
    let record: DirectKfdWrapperOverheadV1 =
        serde_json::from_slice(bytes).map_err(|_| WrapperOverheadErrorV1::JsonDecode)?;
    record.validate()?;
    if serde_json::to_vec(&record).map_err(|_| WrapperOverheadErrorV1::JsonEncode)? != bytes {
        return Err(WrapperOverheadErrorV1::NonCanonicalEncoding);
    }
    Ok(record)
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum WrapperOverheadErrorV1 {
    InvalidRecord,
    SizeOverflow,
    JsonEncode,
    JsonDecode,
    NonCanonicalEncoding,
}

impl fmt::Display for WrapperOverheadErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "direct-KFD wrapper overhead evidence rejected: {self:?}"
        )
    }
}

impl Error for WrapperOverheadErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(value: u8) -> RawContentIdentityV1 {
        RawContentIdentityV1 {
            sha256: [value; 32],
            byte_len: 1,
        }
    }

    fn invocation(pair: u16, leg: u8, phase: TrialPhaseV1) -> InvocationEvidenceV1 {
        let order = if pair.is_multiple_of(2) {
            PairOrderV1::RawThenWrapped
        } else {
            PairOrderV1::WrappedThenRaw
        };
        let kind = match (order, leg) {
            (PairOrderV1::RawThenWrapped, 0) | (PairOrderV1::WrappedThenRaw, 1) => {
                InvocationKindV1::RawTarget
            }
            _ => InvocationKindV1::RocprofWrappedTarget,
        };
        InvocationEvidenceV1 {
            phase,
            pair_index: pair,
            pair_order: order,
            invocation_index_in_pair: leg,
            kind,
            argv: identity(8),
            host_wall_time_ns: if kind == InvocationKindV1::RawTarget {
                100
            } else {
                125
            },
            outcome: InvocationOutcomeV1::ExitedSuccess,
            exit_code: Some(0),
            exit_signal: None,
            wait_error: None,
            stdout: identity(9),
            stdout_truncated: false,
            stderr: identity(10),
            stderr_truncated: false,
            artifact_state: if kind == InvocationKindV1::RawTarget {
                ArtifactStateV1::NotApplicableRawTarget
            } else {
                ArtifactStateV1::InventoryCompleteNoArtifacts
            },
            capture_loss_state: if kind == InvocationKindV1::RawTarget {
                CaptureLossStateV1::NotApplicableRawTarget
            } else {
                CaptureLossStateV1::UnavailableNoAdmittedCapture
            },
            wrapped_output_inventory: Vec::new(),
        }
    }

    fn record() -> DirectKfdWrapperOverheadV1 {
        let policy = OverheadPolicyV1 {
            warmup_pairs: 1,
            measured_pairs: 3,
            order_policy: OrderPolicyV1::AlternatingPairsEvenRawFirstOddWrappedFirst,
            clock: HostClockV1::LinuxClockMonotonicRaw,
            timing_boundary:
                TimingBoundaryV1::ImmediatelyBeforeSpawnThroughSupervisionAndBoundedOutputDrain,
            per_process_timeout_ms: 1_000,
            total_processes: 8,
            total_harness_timeout_ms: 3_600_000,
            candidate_wrapper_overhead_budget_bps: 3_000,
            excludes_outliers: false,
        };
        let mut invocations = Vec::new();
        for leg in 0..2 {
            invocations.push(invocation(0, leg, TrialPhaseV1::Warmup));
        }
        for pair in 0..3 {
            for leg in 0..2 {
                invocations.push(invocation(pair, leg, TrialPhaseV1::Measured));
            }
        }
        build_wrapper_overhead_v1(BuildInputsV1 {
            plan_sha256: [1; 32],
            measurement_harness_executable: identity(15),
            target_executable: identity(2),
            target_argv: identity(3),
            collector_executable: identity(4),
            collector_release: CollectorReleaseV1::UnavailableUnrecognizedExactTool,
            collector_closure: identity(5),
            collector_configuration: identity(6),
            environment: identity(7),
            working_directory: identity(13),
            device_topology: identity(11),
            policy,
            invocations,
        })
        .unwrap()
    }

    #[test]
    fn canonical_round_trip_and_summary_are_deterministic() {
        let record = record();
        let summary = record.summary.as_ref().unwrap();
        assert_eq!(summary.paired_overhead_median_bps, 2_500);
        assert_eq!(
            summary.candidate_budget_result,
            CandidateBudgetResultV1::WithinCandidateBudget
        );
        assert_eq!(
            record.kernel_trace_capture_overhead,
            CaptureOverheadStateV1::NoAdmittedCapture
        );
        let bytes = encode_wrapper_overhead_v1(&record).unwrap();
        assert_eq!(decode_wrapper_overhead_v1(&bytes).unwrap(), record);
    }

    #[test]
    fn swapped_or_stale_trials_and_self_approval_are_rejected() {
        let mut swapped = record();
        swapped.invocations.swap(2, 3);
        assert_eq!(
            swapped.validate(),
            Err(WrapperOverheadErrorV1::InvalidRecord)
        );
        let mut stale = record();
        stale.invocations.pop();
        assert_eq!(stale.validate(), Err(WrapperOverheadErrorV1::InvalidRecord));
        let mut approved = record();
        approved.grants_production_qualification = true;
        assert_eq!(
            approved.validate(),
            Err(WrapperOverheadErrorV1::InvalidRecord)
        );
    }

    #[test]
    fn truncation_makes_summary_unavailable() {
        let mut inputs = record();
        inputs.invocations[2].stdout_truncated = true;
        inputs.invocations[2].outcome = InvocationOutcomeV1::StdoutLimitExceeded;
        inputs.summary = None;
        inputs.validate().unwrap();
    }

    #[test]
    fn unadmitted_artifacts_do_not_become_capture_overhead() {
        let mut inputs = record();
        let wrapped = inputs
            .invocations
            .iter_mut()
            .find(|trial| {
                trial.phase == TrialPhaseV1::Measured
                    && trial.kind == InvocationKindV1::RocprofWrappedTarget
            })
            .unwrap();
        wrapped.wrapped_output_inventory.push(CollectorArtifactV1 {
            relative_path: "capture.csv".to_owned(),
            content: identity(12),
        });
        wrapped.artifact_state = ArtifactStateV1::InventoryCompleteArtifactsPresentUnadmitted;
        inputs.kernel_trace_capture_overhead = CaptureOverheadStateV1::ArtifactsPresentUnadmitted;
        inputs.measured_wrapped_output_inventory = WrappedInventoryAggregateV1::MixedUnadmitted;
        inputs.validate().unwrap();
    }

    #[test]
    fn warmup_failure_suppresses_the_summary() {
        let mut inputs = record();
        inputs.invocations[0].outcome = InvocationOutcomeV1::ExitedFailure;
        inputs.invocations[0].exit_code = Some(9);
        inputs.summary = None;
        inputs.validate().unwrap();
    }

    #[test]
    fn inconsistent_exit_and_duplicate_artifact_records_are_rejected() {
        let mut exit = record();
        exit.invocations[0].exit_code = Some(1);
        assert_eq!(exit.validate(), Err(WrapperOverheadErrorV1::InvalidRecord));

        let mut artifact = record();
        let wrapped = artifact
            .invocations
            .iter_mut()
            .find(|trial| {
                trial.phase == TrialPhaseV1::Measured
                    && trial.kind == InvocationKindV1::RocprofWrappedTarget
            })
            .unwrap();
        let duplicate = CollectorArtifactV1 {
            relative_path: "same.json".to_owned(),
            content: identity(14),
        };
        wrapped.wrapped_output_inventory = vec![duplicate.clone(), duplicate];
        wrapped.artifact_state = ArtifactStateV1::InventoryCompleteArtifactsPresentUnadmitted;
        artifact.measured_wrapped_output_inventory = WrappedInventoryAggregateV1::MixedUnadmitted;
        artifact.kernel_trace_capture_overhead = CaptureOverheadStateV1::ArtifactsPresentUnadmitted;
        assert_eq!(
            artifact.validate(),
            Err(WrapperOverheadErrorV1::InvalidRecord)
        );
    }

    #[test]
    fn unknown_fields_and_noncanonical_json_are_rejected() {
        let bytes = encode_wrapper_overhead_v1(&record()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["unexpected"] = serde_json::json!(true);
        let hostile = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            decode_wrapper_overhead_v1(&hostile),
            Err(WrapperOverheadErrorV1::JsonDecode)
        );

        let mut noncanonical = bytes;
        noncanonical.push(b'\n');
        assert_eq!(
            decode_wrapper_overhead_v1(&noncanonical),
            Err(WrapperOverheadErrorV1::NonCanonicalEncoding)
        );
    }

    #[test]
    fn checked_in_mi300x_record_is_canonical_and_non_authoritative() {
        let bytes = include_bytes!(
            "../../../docs/evidence/mi300x-rocprof-wrapper-host-wall-2026-09-03.json"
        );
        let record = decode_wrapper_overhead_v1(bytes).unwrap();
        assert_eq!(record.policy.warmup_pairs, 5);
        assert_eq!(record.policy.measured_pairs, 30);
        assert_eq!(record.invocations.len(), 70);
        assert_eq!(
            record.measured_wrapped_output_inventory,
            WrappedInventoryAggregateV1::AllEmpty
        );
        assert_eq!(
            record.kernel_trace_capture_overhead,
            CaptureOverheadStateV1::NoAdmittedCapture
        );
        assert!(!record.grants_collection_authority);
        assert!(!record.grants_production_qualification);
    }
}
