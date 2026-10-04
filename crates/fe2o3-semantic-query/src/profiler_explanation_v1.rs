//! Bounded, evidence-linked explanations over exact profiler comparisons.
//!
//! This layer composes the existing V1/V2/V3 and complete-catalog contracts.
//! Its rules rank co-observations only; they never upgrade compiler records,
//! samples, counters, or structural differences into causal claims.

use std::{error::Error, fmt};

use fe2o3_hsaco_finalize::{
    AdmittedProductionProfilerKirArchiveV1, PRODUCTION_PROFILER_KIR_ARCHIVE_VERSION_V1,
    ProductionProfilerOptimizationEvidenceV1,
};
use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1, TruthOriginV1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    ProfilerCompleteStructuralComparisonRequestV1, ProfilerCompleteStructuralComparisonV1,
    ProfilerCompleteStructuralErrorV1, ProfilerCompleteStructuralTreatmentInputV1,
    ProfilerVariantTreatmentSideV3, compare_profiler_complete_structural_v1,
};

pub const PROFILER_REGRESSION_EXPLANATION_SCHEMA_VERSION_V1: u16 = 1;
pub const PROFILER_REGRESSION_EXPLANATION_SCHEMA_V1: &str =
    "fe2o3-profiler-regression-explanation-v1";
pub const MAX_PROFILER_REGRESSION_HYPOTHESES_V1: usize = 8;
pub const MAX_PROFILER_NEXT_MEASUREMENTS_V1: usize = 8;
pub const MAX_PROFILER_REGRESSION_EXPLANATION_BYTES_V1: u64 = 20 * 1024 * 1024;

const REPORT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.profiler-regression-explanation.v1\0";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerOptimizationPassEvidenceV1 {
    pub ordinal: u16,
    pub pass: &'static str,
    pub changed: bool,
    pub input_epoch: u64,
    pub output_epoch: u64,
    pub input_graph_work: u64,
    pub output_graph_work: u64,
    pub work_units: u64,
    pub origin: TruthOriginV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerOptimizationLimitsV1 {
    pub max_input_canonical_bytes: u64,
    pub max_output_canonical_bytes: u64,
    pub max_dialects: u64,
    pub max_passes: u64,
    pub max_diagnostic_bytes: u64,
    pub max_optimization_passes: u64,
    pub max_graph_work: u64,
    pub max_work_units: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerOptimizationAuditV1 {
    pub side: ProfilerVariantTreatmentSideV3,
    pub origin: TruthOriginV1,
    pub archive: ContentIdentityRecordV1,
    pub replay: ContentIdentityRecordV1,
    pub pre_optimization_target_kir: ContentIdentityRecordV1,
    pub final_target_kir: ContentIdentityRecordV1,
    pub optimizer_policy_version: u16,
    pub input_bridge: ContentIdentityRecordV1,
    pub output_bridge: ContentIdentityRecordV1,
    pub correspondence_identity: CaptureIdentityV1,
    pub correspondence_count: u64,
    pub changed: bool,
    pub initial_epoch: u64,
    pub final_epoch: u64,
    pub initial_graph_work: u64,
    pub final_graph_work: u64,
    pub invalidated_handle_count: u64,
    pub work_units: u64,
    pub limits: ProfilerOptimizationLimitsV1,
    pub passes: Vec<ProfilerOptimizationPassEvidenceV1>,
    pub semantics: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerOptimizationUnavailableV1 {
    pub side: ProfilerVariantTreatmentSideV3,
    pub origin: TruthOriginV1,
    pub archive: Option<ContentIdentityRecordV1>,
    pub reason_code: &'static str,
    pub semantics: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProfilerOptimizationEvidenceStatusV1 {
    Available {
        audit: Box<ProfilerOptimizationAuditV1>,
    },
    Unavailable {
        unavailable: ProfilerOptimizationUnavailableV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerRegressionHypothesisKindV1 {
    CompilerTransformationCoObservation,
    StaticResourceCoObservation,
    CounterCoObservation,
    SemanticOccurrenceCoObservation,
    StructuralMultiplicityCoObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerRegressionConfidenceV1 {
    Low,
    Moderate,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerRegressionMissingFactV1 {
    ExactComparableCapture,
    ObservedScheduleExecution,
    CounterDimensionsAndCompleteness,
    PositivePcOrAttSemanticOccurrences,
    CompleteStructuralCoverage,
    ControlledCausalExperiment,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerRankedRegressionHypothesisV1 {
    pub rank: u16,
    pub rule_id: &'static str,
    pub kind: ProfilerRegressionHypothesisKindV1,
    pub origin: TruthOriginV1,
    pub statement: &'static str,
    pub confidence: ProfilerRegressionConfidenceV1,
    pub supporting_evidence_ids: Vec<CaptureIdentityV1>,
    pub contradicting_evidence_ids: Vec<CaptureIdentityV1>,
    pub missing_facts: Vec<ProfilerRegressionMissingFactV1>,
    pub limitations: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerNextMeasurementKindV1 {
    ExactComparableRecapture,
    CounterCollection,
    PcSampling,
    DecodedAtt,
    ScheduleExecutionObservation,
    ControlledVariantReplicates,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerMeasurementCostV1 {
    Low,
    Moderate,
    High,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerNextMeasurementV1 {
    pub rank: u16,
    pub kind: ProfilerNextMeasurementKindV1,
    pub reason: &'static str,
    pub required_scope: &'static str,
    pub expected_cost: ProfilerMeasurementCostV1,
    pub requires_explicit_collection_authorization: bool,
    pub semantics: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerRegressionExplanationV1 {
    pub schema_version: u16,
    pub report_identity: ContentIdentityRecordV1,
    pub comparison: Box<ProfilerCompleteStructuralComparisonV1>,
    pub compiler_optimization: Vec<ProfilerOptimizationEvidenceStatusV1>,
    pub ranked_hypotheses: Vec<ProfilerRankedRegressionHypothesisV1>,
    pub next_measurements: Vec<ProfilerNextMeasurementV1>,
    pub ranking_policy: &'static str,
    pub causal_attribution: TruthOriginV1,
    pub authority: &'static str,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProfilerRegressionExplanationErrorV1 {
    Comparison(ProfilerCompleteStructuralErrorV1),
    Identity,
    EvidenceTooLarge,
    ResultTooLarge,
}

impl fmt::Display for ProfilerRegressionExplanationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "profiler regression explanation rejected: {self:?}"
        )
    }
}

impl Error for ProfilerRegressionExplanationErrorV1 {}

pub fn explain_profiler_regression_v1(
    request: ProfilerCompleteStructuralComparisonRequestV1,
    baseline: ProfilerCompleteStructuralTreatmentInputV1<'_>,
    candidate: ProfilerCompleteStructuralTreatmentInputV1<'_>,
) -> Result<ProfilerRegressionExplanationV1, ProfilerRegressionExplanationErrorV1> {
    let comparison = compare_profiler_complete_structural_v1(request, baseline, candidate)
        .map_err(ProfilerRegressionExplanationErrorV1::Comparison)?;
    let compiler_optimization = vec![
        optimization_status(ProfilerVariantTreatmentSideV3::Baseline, baseline.archive)?,
        optimization_status(ProfilerVariantTreatmentSideV3::Candidate, candidate.archive)?,
    ];
    let ranked_hypotheses = hypotheses(&comparison, baseline.archive, candidate.archive)?;
    let next_measurements = next_measurements(&comparison);
    let report_identity = report_identity(
        comparison.request_identity,
        baseline.archive,
        candidate.archive,
        &ranked_hypotheses,
        &next_measurements,
    )?;
    let result = ProfilerRegressionExplanationV1 {
        schema_version: PROFILER_REGRESSION_EXPLANATION_SCHEMA_VERSION_V1,
        report_identity,
        comparison: Box::new(comparison),
        compiler_optimization,
        ranked_hypotheses,
        next_measurements,
        ranking_policy: "fixed_rule_order_over_exact_co_observations_not_probability_superiority_or_causality",
        causal_attribution: TruthOriginV1::Unavailable,
        authority: "read_only_no_execution_attach_scheduling_collection_decoder_publication_load_launch_dispatch_or_runtime_authority",
    };
    let encoded = serde_json::to_vec(&result)
        .map_err(|_| ProfilerRegressionExplanationErrorV1::ResultTooLarge)?;
    if encoded.len() as u64 > MAX_PROFILER_REGRESSION_EXPLANATION_BYTES_V1 {
        return Err(ProfilerRegressionExplanationErrorV1::ResultTooLarge);
    }
    Ok(result)
}

fn optimization_status(
    side: ProfilerVariantTreatmentSideV3,
    archive: Option<&AdmittedProductionProfilerKirArchiveV1>,
) -> Result<ProfilerOptimizationEvidenceStatusV1, ProfilerRegressionExplanationErrorV1> {
    let Some(archive) = archive else {
        return Ok(ProfilerOptimizationEvidenceStatusV1::Unavailable {
            unavailable: ProfilerOptimizationUnavailableV1 {
                side,
                origin: TruthOriginV1::Unavailable,
                archive: None,
                reason_code: "production_profiler_archive_not_supplied",
                semantics: "no exact finalizer-replayed production archive was supplied for this treatment",
            },
        });
    };
    let archive_identity = archive_identity(archive)?;
    let Some(evidence) = archive.optimization_v1() else {
        return Ok(ProfilerOptimizationEvidenceStatusV1::Unavailable {
            unavailable: ProfilerOptimizationUnavailableV1 {
                side,
                origin: TruthOriginV1::Unavailable,
                archive: Some(archive_identity),
                reason_code: "legacy_replay_has_no_optimizer_audit",
                semantics: "the exact production replay predates optimizer audit V4; no pass decision is inferred",
            },
        });
    };
    Ok(ProfilerOptimizationEvidenceStatusV1::Available {
        audit: Box::new(project_optimization(side, archive_identity, evidence)?),
    })
}

fn project_optimization(
    side: ProfilerVariantTreatmentSideV3,
    archive: ContentIdentityRecordV1,
    evidence: &ProductionProfilerOptimizationEvidenceV1,
) -> Result<ProfilerOptimizationAuditV1, ProfilerRegressionExplanationErrorV1> {
    let audit = evidence.audit();
    let report = audit.report();
    let passes = report
        .passes()
        .iter()
        .copied()
        .enumerate()
        .map(|(ordinal, pass)| {
            Ok(ProfilerOptimizationPassEvidenceV1 {
                ordinal: u16::try_from(ordinal)
                    .map_err(|_| ProfilerRegressionExplanationErrorV1::EvidenceTooLarge)?,
                pass: pass.pass().name(),
                changed: pass.changed(),
                input_epoch: pass.input_epoch(),
                output_epoch: pass.output_epoch(),
                input_graph_work: pass.input_graph_work(),
                output_graph_work: pass.output_graph_work(),
                work_units: pass.work_units(),
                origin: TruthOriginV1::Declared,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let replay = evidence.replay_identity();
    let pre = audit.pre_optimization_target_bound_kernel_ir_identity();
    let final_kir = evidence.final_target_bound_kernel_ir_identity();
    let limits = audit.limits();
    Ok(ProfilerOptimizationAuditV1 {
        side,
        origin: TruthOriginV1::Declared,
        archive,
        replay: identity(
            ContentSchemeV1::DomainSeparatedSha256,
            4,
            replay.sha256(),
            replay.byte_len(),
        )?,
        pre_optimization_target_kir: identity(
            ContentSchemeV1::RawCanonicalSha256,
            pre.version().wire_version(),
            pre.sha256(),
            pre.byte_len(),
        )?,
        final_target_kir: identity(
            ContentSchemeV1::RawCanonicalSha256,
            final_kir.version().wire_version(),
            final_kir.sha256(),
            final_kir.byte_len(),
        )?,
        optimizer_policy_version: audit.optimizer_policy_version(),
        input_bridge: identity(
            ContentSchemeV1::RawCanonicalSha256,
            audit.optimizer_policy_version(),
            audit.input_bridge_digest(),
            audit.input_bridge_bytes(),
        )?,
        output_bridge: identity(
            ContentSchemeV1::RawCanonicalSha256,
            audit.optimizer_policy_version(),
            audit.output_bridge_digest(),
            audit.output_bridge_bytes(),
        )?,
        correspondence_identity: capture(audit.correspondence_digest())?,
        correspondence_count: audit.correspondence_count(),
        changed: audit.changed(),
        initial_epoch: report.initial_epoch(),
        final_epoch: report.final_epoch(),
        initial_graph_work: report.initial_graph_work(),
        final_graph_work: report.final_graph_work(),
        invalidated_handle_count: report.invalidated_handle_count(),
        work_units: report.work_units(),
        limits: ProfilerOptimizationLimitsV1 {
            max_input_canonical_bytes: limits.max_input_canonical_bytes(),
            max_output_canonical_bytes: limits.max_output_canonical_bytes(),
            max_dialects: limits.max_dialects(),
            max_passes: limits.max_passes(),
            max_diagnostic_bytes: limits.max_diagnostic_bytes(),
            max_optimization_passes: limits.max_optimization_passes(),
            max_graph_work: limits.max_graph_work(),
            max_work_units: limits.max_work_units(),
        },
        passes,
        semantics: "exact compiler-declared optimizer transcript independently reconstructed by finalizer archive replay; not a semantic-preservation proof or execution observation",
    })
}

fn hypotheses(
    comparison: &ProfilerCompleteStructuralComparisonV1,
    baseline_archive: Option<&AdmittedProductionProfilerKirArchiveV1>,
    candidate_archive: Option<&AdmittedProductionProfilerKirArchiveV1>,
) -> Result<Vec<ProfilerRankedRegressionHypothesisV1>, ProfilerRegressionExplanationErrorV1> {
    let v1 = &comparison.comparison_v3.comparison_v2.comparison_v1;
    if !v1.comparable {
        return Ok(Vec::new());
    }
    let slower = v1
        .duration_deltas
        .iter()
        .filter(|delta| delta.signed_delta_ticks > 0)
        .collect::<Vec<_>>();
    if slower.is_empty() {
        return Ok(Vec::new());
    }
    let mut duration_support = slower
        .iter()
        .flat_map(|delta| delta.evidence.iter().map(|evidence| evidence.identity))
        .collect::<Vec<_>>();
    sort_dedup(&mut duration_support);
    let mut duration_contradiction = v1
        .duration_deltas
        .iter()
        .filter(|delta| delta.signed_delta_ticks <= 0)
        .flat_map(|delta| delta.evidence.iter().map(|evidence| evidence.identity))
        .collect::<Vec<_>>();
    sort_dedup(&mut duration_contradiction);

    let optimizer_changed = match (baseline_archive, candidate_archive) {
        (Some(left), Some(right)) => left.optimization_v1() != right.optimization_v1(),
        _ => false,
    };
    let mut optimizer_evidence = Vec::new();
    if optimizer_changed {
        for archive in [baseline_archive, candidate_archive].into_iter().flatten() {
            optimizer_evidence.push(archive_identity(archive)?.digest);
            if let Some(evidence) = archive.optimization_v1() {
                optimizer_evidence.push(capture(evidence.replay_identity().sha256())?);
            }
        }
    }

    let mut output = Vec::new();
    if optimizer_changed || !comparison.comparison_v3.structural_changes.is_empty() {
        let mut support = duration_support.clone();
        support.extend(optimizer_evidence);
        support.extend(
            comparison
                .comparison_v3
                .structural_changes
                .iter()
                .flat_map(|change| change.evidence_ids.iter().copied()),
        );
        push_hypothesis(
            &mut output,
            "compiler-transformation-co-observation-v1",
            ProfilerRegressionHypothesisKindV1::CompilerTransformationCoObservation,
            "A compiler transformation or exact structural mapping changed in a treatment with longer captured dispatch duration.",
            support,
            duration_contradiction.clone(),
            missing_facts(comparison),
        )?;
    }
    let changed_resources = v1
        .resource_deltas
        .iter()
        .filter(|delta| delta.signed_delta.is_some_and(|value| value != 0))
        .collect::<Vec<_>>();
    if !changed_resources.is_empty() {
        let mut support = duration_support.clone();
        support.extend(
            changed_resources
                .iter()
                .flat_map(|delta| delta.evidence.iter().map(|evidence| evidence.identity)),
        );
        push_hypothesis(
            &mut output,
            "static-resource-co-observation-v1",
            ProfilerRegressionHypothesisKindV1::StaticResourceCoObservation,
            "Final HSACO static resources changed in a treatment with longer captured dispatch duration.",
            support,
            duration_contradiction.clone(),
            missing_facts(comparison),
        )?;
    }
    if !v1.counter_deltas.is_empty() {
        let mut support = duration_support.clone();
        support.extend(
            v1.counter_deltas
                .iter()
                .flat_map(|delta| delta.evidence.iter().map(|evidence| evidence.identity)),
        );
        push_hypothesis(
            &mut output,
            "counter-co-observation-v1",
            ProfilerRegressionHypothesisKindV1::CounterCoObservation,
            "One or more exactly bound counter values changed in a treatment with longer captured dispatch duration.",
            support,
            duration_contradiction.clone(),
            missing_facts(comparison),
        )?;
    }
    if !comparison
        .comparison_v3
        .comparison_v2
        .changed_occurrences
        .is_empty()
    {
        let mut support = duration_support.clone();
        support.extend(
            comparison
                .comparison_v3
                .comparison_v2
                .changed_occurrences
                .iter()
                .flat_map(|change| change.evidence_ids.iter().copied()),
        );
        push_hypothesis(
            &mut output,
            "semantic-occurrence-co-observation-v1",
            ProfilerRegressionHypothesisKindV1::SemanticOccurrenceCoObservation,
            "A positively observed source/MIR/KIR/LLVM/ISA occurrence changed in a treatment with longer captured dispatch duration.",
            support,
            duration_contradiction.clone(),
            missing_facts(comparison),
        )?;
    }
    if !comparison.added.is_empty() || !comparison.removed.is_empty() {
        let mut support = duration_support;
        support.extend(
            comparison
                .added
                .iter()
                .chain(&comparison.removed)
                .flat_map(|delta| delta.evidence_ids.iter().copied()),
        );
        push_hypothesis(
            &mut output,
            "structural-multiplicity-co-observation-v1",
            ProfilerRegressionHypothesisKindV1::StructuralMultiplicityCoObservation,
            "A complete same-domain structural operation multiplicity changed in a treatment with longer captured dispatch duration.",
            support,
            duration_contradiction,
            missing_facts(comparison),
        )?;
    }
    Ok(output)
}

fn push_hypothesis(
    output: &mut Vec<ProfilerRankedRegressionHypothesisV1>,
    rule_id: &'static str,
    kind: ProfilerRegressionHypothesisKindV1,
    statement: &'static str,
    mut supporting_evidence_ids: Vec<CaptureIdentityV1>,
    mut contradicting_evidence_ids: Vec<CaptureIdentityV1>,
    missing_facts: Vec<ProfilerRegressionMissingFactV1>,
) -> Result<(), ProfilerRegressionExplanationErrorV1> {
    if output.len() >= MAX_PROFILER_REGRESSION_HYPOTHESES_V1 {
        return Err(ProfilerRegressionExplanationErrorV1::EvidenceTooLarge);
    }
    sort_dedup(&mut supporting_evidence_ids);
    sort_dedup(&mut contradicting_evidence_ids);
    let independent_classes =
        usize::from(!supporting_evidence_ids.is_empty()) + usize::from(missing_facts.len() <= 3);
    output.push(ProfilerRankedRegressionHypothesisV1 {
        rank: u16::try_from(output.len() + 1)
            .map_err(|_| ProfilerRegressionExplanationErrorV1::EvidenceTooLarge)?,
        rule_id,
        kind,
        origin: TruthOriginV1::Inferred,
        statement,
        confidence: if independent_classes > 1 && contradicting_evidence_ids.is_empty() {
            ProfilerRegressionConfidenceV1::Moderate
        } else {
            ProfilerRegressionConfidenceV1::Low
        },
        supporting_evidence_ids,
        contradicting_evidence_ids,
        missing_facts,
        limitations: "rank is deterministic rule order, not probability; co-observation does not establish that the cited compiler, resource, counter, or structural change caused the duration delta",
    });
    Ok(())
}

fn missing_facts(
    comparison: &ProfilerCompleteStructuralComparisonV1,
) -> Vec<ProfilerRegressionMissingFactV1> {
    let v1 = &comparison.comparison_v3.comparison_v2.comparison_v1;
    let mut facts = Vec::new();
    if !v1.comparable {
        facts.push(ProfilerRegressionMissingFactV1::ExactComparableCapture);
    }
    facts.push(ProfilerRegressionMissingFactV1::ObservedScheduleExecution);
    if v1.counter_deltas.is_empty() {
        facts.push(ProfilerRegressionMissingFactV1::CounterDimensionsAndCompleteness);
    }
    if comparison
        .comparison_v3
        .comparison_v2
        .changed_occurrences
        .is_empty()
    {
        facts.push(ProfilerRegressionMissingFactV1::PositivePcOrAttSemanticOccurrences);
    }
    if comparison.comparison_domain.is_none() {
        facts.push(ProfilerRegressionMissingFactV1::CompleteStructuralCoverage);
    }
    facts.push(ProfilerRegressionMissingFactV1::ControlledCausalExperiment);
    facts.sort_unstable();
    facts.dedup();
    facts
}

fn next_measurements(
    comparison: &ProfilerCompleteStructuralComparisonV1,
) -> Vec<ProfilerNextMeasurementV1> {
    let v1 = &comparison.comparison_v3.comparison_v2.comparison_v1;
    let mut kinds = Vec::new();
    if !v1.comparable {
        kinds.push(ProfilerNextMeasurementKindV1::ExactComparableRecapture);
    } else {
        if v1.counter_deltas.is_empty() {
            kinds.push(ProfilerNextMeasurementKindV1::CounterCollection);
        }
        if comparison
            .comparison_v3
            .comparison_v2
            .changed_occurrences
            .is_empty()
        {
            kinds.push(ProfilerNextMeasurementKindV1::PcSampling);
        } else if comparison.comparison_domain.is_none() {
            kinds.push(ProfilerNextMeasurementKindV1::DecodedAtt);
        }
        kinds.push(ProfilerNextMeasurementKindV1::ScheduleExecutionObservation);
        kinds.push(ProfilerNextMeasurementKindV1::ControlledVariantReplicates);
    }
    kinds.truncate(MAX_PROFILER_NEXT_MEASUREMENTS_V1);
    kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| measurement(index, kind))
        .collect()
}

fn measurement(index: usize, kind: ProfilerNextMeasurementKindV1) -> ProfilerNextMeasurementV1 {
    let (reason, required_scope, expected_cost, authorized, semantics) = match kind {
        ProfilerNextMeasurementKindV1::ExactComparableRecapture => (
            "comparison axes differ; collect the smallest pair with identical environment, collector configuration, device, workload, arguments, and launch",
            "the mismatched treatment pair only",
            ProfilerMeasurementCostV1::Moderate,
            true,
            "a comparable recapture enables deltas but still does not prove causation",
        ),
        ProfilerNextMeasurementKindV1::CounterCollection => (
            "no exact comparable counter delta is available",
            "the affected dispatches and a minimal named counter set on both treatments",
            ProfilerMeasurementCostV1::Moderate,
            true,
            "counter values require exact dimensions, dispatch binding, and completeness disclosure",
        ),
        ProfilerNextMeasurementKindV1::PcSampling => (
            "no positive source/IR/ISA occurrence distinguishes the treatments",
            "the affected kernel symbol and dispatches on both treatments",
            ProfilerMeasurementCostV1::Moderate,
            true,
            "PC sampling is sampled positive evidence; missing samples never establish absence",
        ),
        ProfilerNextMeasurementKindV1::DecodedAtt => (
            "positive samples exist but complete same-domain structural coverage is unavailable",
            "the smallest target CU/kernel/dispatch window that distinguishes the remaining sites",
            ProfilerMeasurementCostV1::High,
            true,
            "decoded ATT remains target-scoped and loss-aware, not a full-grid execution history",
        ),
        ProfilerNextMeasurementKindV1::ScheduleExecutionObservation => (
            "the treatment schedule is content-bound but not authenticated as executed",
            "the compared dispatches only",
            ProfilerMeasurementCostV1::Low,
            true,
            "the producer must bind an observed runtime dispatch to the exact schedule identity",
        ),
        ProfilerNextMeasurementKindV1::ControlledVariantReplicates => (
            "single paired observations cannot separate treatment effect from run variance",
            "bounded interleaved repetitions of the exact comparable treatments",
            ProfilerMeasurementCostV1::High,
            true,
            "replicates estimate variance; they do not provide deterministic replay of a GPU schedule",
        ),
    };
    ProfilerNextMeasurementV1 {
        rank: u16::try_from(index + 1).expect("bounded measurement rank fits u16"),
        kind,
        reason,
        required_scope,
        expected_cost,
        requires_explicit_collection_authorization: authorized,
        semantics,
    }
}

fn report_identity(
    comparison: ContentIdentityRecordV1,
    baseline: Option<&AdmittedProductionProfilerKirArchiveV1>,
    candidate: Option<&AdmittedProductionProfilerKirArchiveV1>,
    hypotheses: &[ProfilerRankedRegressionHypothesisV1],
    measurements: &[ProfilerNextMeasurementV1],
) -> Result<ContentIdentityRecordV1, ProfilerRegressionExplanationErrorV1> {
    #[derive(Serialize)]
    struct Preimage<'a> {
        schema_version: u16,
        comparison: ContentIdentityRecordV1,
        baseline_archive: Option<ContentIdentityRecordV1>,
        candidate_archive: Option<ContentIdentityRecordV1>,
        hypotheses: &'a [ProfilerRankedRegressionHypothesisV1],
        measurements: &'a [ProfilerNextMeasurementV1],
    }
    let preimage = Preimage {
        schema_version: PROFILER_REGRESSION_EXPLANATION_SCHEMA_VERSION_V1,
        comparison,
        baseline_archive: baseline.map(archive_identity).transpose()?,
        candidate_archive: candidate.map(archive_identity).transpose()?,
        hypotheses,
        measurements,
    };
    let bytes = serde_json::to_vec(&preimage)
        .map_err(|_| ProfilerRegressionExplanationErrorV1::Identity)?;
    let mut digest = Sha256::new();
    digest.update(REPORT_IDENTITY_DOMAIN_V1);
    digest.update(&bytes);
    identity(
        ContentSchemeV1::DomainSeparatedSha256,
        PROFILER_REGRESSION_EXPLANATION_SCHEMA_VERSION_V1,
        digest.finalize().into(),
        u64::try_from(bytes.len())
            .map_err(|_| ProfilerRegressionExplanationErrorV1::EvidenceTooLarge)?,
    )
}

fn archive_identity(
    archive: &AdmittedProductionProfilerKirArchiveV1,
) -> Result<ContentIdentityRecordV1, ProfilerRegressionExplanationErrorV1> {
    identity(
        ContentSchemeV1::DomainSeparatedSha256,
        PRODUCTION_PROFILER_KIR_ARCHIVE_VERSION_V1,
        *archive.identity().as_bytes(),
        archive.canonical_len(),
    )
}

fn identity(
    scheme: ContentSchemeV1,
    format_version: u16,
    digest: [u8; 32],
    canonical_len: u64,
) -> Result<ContentIdentityRecordV1, ProfilerRegressionExplanationErrorV1> {
    Ok(ContentIdentityRecordV1 {
        scheme,
        format_version,
        digest: capture(digest)?,
        canonical_len,
    })
}

fn capture(bytes: [u8; 32]) -> Result<CaptureIdentityV1, ProfilerRegressionExplanationErrorV1> {
    CaptureIdentityV1::new(bytes).map_err(|_| ProfilerRegressionExplanationErrorV1::Identity)
}

fn sort_dedup(values: &mut Vec<CaptureIdentityV1>) {
    values.sort_unstable();
    values.dedup();
}

const _: () = assert!(MAX_PROFILER_REGRESSION_HYPOTHESES_V1 <= u16::MAX as usize);
const _: () = assert!(MAX_PROFILER_NEXT_MEASUREMENTS_V1 <= u16::MAX as usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurement_roster_is_bounded_and_explicitly_authorized() {
        for (index, kind) in [
            ProfilerNextMeasurementKindV1::ExactComparableRecapture,
            ProfilerNextMeasurementKindV1::CounterCollection,
            ProfilerNextMeasurementKindV1::PcSampling,
            ProfilerNextMeasurementKindV1::DecodedAtt,
            ProfilerNextMeasurementKindV1::ScheduleExecutionObservation,
            ProfilerNextMeasurementKindV1::ControlledVariantReplicates,
        ]
        .into_iter()
        .enumerate()
        {
            let value = measurement(index, kind);
            assert_eq!(value.rank, u16::try_from(index + 1).unwrap());
            assert!(value.requires_explicit_collection_authorization);
            assert!(!value.reason.is_empty());
            assert!(!value.semantics.is_empty());
        }
    }

    #[test]
    fn result_bounds_are_additive_to_complete_comparison() {
        const {
            assert!(
                MAX_PROFILER_REGRESSION_EXPLANATION_BYTES_V1
                    > crate::MAX_PROFILER_COMPLETE_STRUCTURAL_RESULT_BYTES_V1
            );
        }
        assert_eq!(PROFILER_REGRESSION_EXPLANATION_SCHEMA_VERSION_V1, 1);
    }

    #[test]
    fn resource_kinds_remain_general_not_kernel_named() {
        let kinds = [
            crate::ProfilerStaticResourceKindV1::KernargSegmentSize,
            crate::ProfilerStaticResourceKindV1::GroupSegmentFixedSize,
            crate::ProfilerStaticResourceKindV1::PrivateSegmentFixedSize,
            crate::ProfilerStaticResourceKindV1::SgprCount,
            crate::ProfilerStaticResourceKindV1::VgprCount,
            crate::ProfilerStaticResourceKindV1::AgprCount,
            crate::ProfilerStaticResourceKindV1::SgprSpillCount,
            crate::ProfilerStaticResourceKindV1::VgprSpillCount,
        ];
        assert_eq!(kinds.len(), 8);
    }
}
