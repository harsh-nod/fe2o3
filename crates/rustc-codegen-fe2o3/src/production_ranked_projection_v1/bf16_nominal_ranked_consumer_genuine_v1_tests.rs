//! Genuine same-owner consuming calls reached through the existing frontend
//! observer. No source is reconstructed and no new process is spawned here.
//! The expected positive is a real qualification requirement, not an assumption:
//! any mandatory verifier refusal fails the genuine frontend unchanged.
use super::super::consumer;
use super::*;
use fe2o3_pliron::{
    ProductionRankedAnalysisAllowanceV1 as Analysis,
    ProductionRankedSnapshotAllowanceV1 as Snapshot, ProductionSessionErrorV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    Positive,
    ZeroAnalysis,
}
#[derive(Clone, Copy, Debug)]
struct Checked {
    blocks: usize,
    accesses: usize,
    emitted_sources: usize,
    unmapped_private_reads: usize,
    tensors: usize,
    verifier_clean: bool,
    expected_analysis_refusal: bool,
}
// Fixed labels only: never format an unbounded verifier report or graph.
// This diagnostic cannot turn an unexpected failure into accepted refusal.
fn failure_kind(error: &Error) -> &'static str {
    match error {
        Error::Compile { error, .. } => match &**error {
            ProductionRankedCompileErrorV1::Registration(_) => "compile_registration",
            ProductionRankedCompileErrorV1::AtomicTarget(_) => "compile_atomic_target",
            ProductionRankedCompileErrorV1::Context(_) => "compile_context",
            ProductionRankedCompileErrorV1::Session(error) => match error {
                ProductionSessionErrorV1::AnalysisResourceLimit { .. } => "analysis_resource",
                ProductionSessionErrorV1::RankedTensorLayout(_) => "tensor_layout",
                ProductionSessionErrorV1::RankedBounds(_) => "ranked_bounds",
                ProductionSessionErrorV1::RankedAtomic(_) => "ranked_atomic",
                ProductionSessionErrorV1::RankedRace(_) => "ranked_race",
                ProductionSessionErrorV1::RankedOwnership(_) => "ranked_ownership",
                ProductionSessionErrorV1::ConditionalOwnership(_) => "conditional_ownership",
                ProductionSessionErrorV1::RankedBarrier(_) => "ranked_barrier",
                ProductionSessionErrorV1::RankedPipeline(_) => "ranked_pipeline",
                ProductionSessionErrorV1::RankedWorkgroup(_) => "ranked_workgroup",
                ProductionSessionErrorV1::RankedSemantic(_) => "ranked_semantic",
                ProductionSessionErrorV1::RankedPassPreservation(_) => "pass_preservation",
                ProductionSessionErrorV1::RankedReportValidation(_) => "report_validation",
                ProductionSessionErrorV1::RankedGraphChanged => "graph_changed",
                ProductionSessionErrorV1::RankedRecipe(_) => "session_recipe",
                _ => "session_other",
            },
        },
        Error::Recipe(_) => "recipe",
        Error::Construction(_) => "construction",
        Error::CanonicalAssertions(_) => "canonical_assertions",
        Error::Incomplete(_) => "incomplete",
        Error::Unsupported(_) => "unsupported",
        _ => "projection_other",
    }
}
fn report_failure(case: Case, error: &Error) {
    // The fixed strings and two-case Debug enum keep this line below 160 bytes.
    eprintln!(
        "fe2o3-nominal-ranked-consumer-failure-v1 case={:?} kind={} accepted=false",
        case,
        failure_kind(error)
    );
    tensor_diagnostic::report(error);
}

// Independent closed-fixture map oracle: do not call the production row helper.
// The existing source oracle still checks all four actual emitted operations.
fn check_correspondence(verified: &consumer::Verified<'_>) -> Result<(usize, usize)> {
    let sources = verified.emitted_sources();
    let retained = verified.access_sources();
    if sources.len() != 4 || retained.len() != 3 || verified.unmapped_private_reads() != 1 {
        return Err(Error::Incomplete("genuine filtered source counts differ"));
    }
    let mut cursor = 0usize;
    let mut unmapped = 0usize;
    for source in sources {
        let site = source
            .semantic_site
            .ok_or(Error::Incomplete("genuine source site absent"))?;
        let operation = verified
            .lowering()
            .kernel()
            .blocks()
            .get(source.block)
            .and_then(|block| block.operations().get(source.operation))
            .ok_or(Error::Incomplete("genuine source operation absent"))?;
        if source.memory_space == MemorySpaceAttr::Private {
            if source.access != AccessKindAttr::Read
                || !matches!(
                    operation,
                    ProductionRankedOperationV1::Access {
                        kind: AccessKindAttr::Read,
                        ..
                    }
                )
            {
                return Err(Error::Incomplete(
                    "genuine unmapped operation is not the private read",
                ));
            }
            unmapped += 1;
            continue;
        }
        let row = retained
            .get(cursor)
            .ok_or(Error::Incomplete("genuine retained row absent"))?;
        // At most three prior retained rows: skipped private reads do not consume
        // a semantic ordinal and never reorder the remaining correspondence.
        let ordinal = retained[..cursor]
            .iter()
            .filter(|previous| {
                previous.semantic_block() as usize == site.block
                    && previous.semantic_statement().map(|n| n as usize) == site.statement
            })
            .count();
        if row.semantic_block() as usize != site.block
            || row.semantic_statement().map(|n| n as usize) != site.statement
            || row.semantic_access_ordinal() as usize != ordinal
            || row.ranked_block() as usize != source.block
            || row.ranked_operation() as usize != source.operation
            || row.output_extent() != source.output_extent
        {
            return Err(Error::Incomplete(
                "genuine retained row differs from filtered source order",
            ));
        }
        cursor += 1;
    }
    if cursor != retained.len() || unmapped != 1 {
        return Err(Error::Incomplete(
            "genuine filtered source partition differs",
        ));
    }
    Ok((sources.len(), unmapped))
}

fn classify(case: Case, result: Result<consumer::Verified<'_>>) -> Result<Checked> {
    match (case, result) {
        (Case::Positive, Ok(verified)) => {
            let lowering = verified.lowering();
            if !lowering.all_mandatory_reports_are_clean()
                || lowering.grants_artifact_or_launch_authority()
                || lowering.grants_compiler_refinement_authority()
                || verified.tensor_count() == 0
            {
                return Err(Error::Incomplete(
                    "genuine nominal verifier result is not a clean source-bound tensor",
                ));
            }
            let (emitted_sources, unmapped_private_reads) = check_correspondence(&verified)?;
            Ok(Checked {
                blocks: lowering.kernel().blocks().len(),
                accesses: verified.access_sources().len(),
                emitted_sources,
                unmapped_private_reads,
                tensors: verified.tensor_count(),
                verifier_clean: true,
                expected_analysis_refusal: false,
            })
        }
        (Case::Positive, Err(error)) => {
            report_failure(case, &error);
            Err(error)
        }
        (Case::ZeroAnalysis, Err(Error::Compile { error, .. }))
            if matches!(
                *error,
                ProductionRankedCompileErrorV1::Session(
                    ProductionSessionErrorV1::AnalysisResourceLimit { .. }
                )
            ) =>
        {
            Ok(Checked {
                blocks: 0,
                accesses: 0,
                emitted_sources: 0,
                unmapped_private_reads: 0,
                tensors: 0,
                verifier_clean: false,
                expected_analysis_refusal: true,
            })
        }
        (Case::ZeroAnalysis, Err(error)) => {
            report_failure(case, &error);
            Err(error)
        }
        (Case::ZeroAnalysis, Ok(_)) => Err(Error::Incomplete(
            "zero analysis unexpectedly verified nominal source",
        )),
    }
}

pub(super) fn observe(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Q<()> {
    for case in [Case::Positive, Case::ZeroAnalysis] {
        let analysis = match case {
            Case::Positive => Analysis::production_hard_ceiling(),
            Case::ZeroAnalysis => Analysis::new(0, 0).expect("closed refusing allowance"),
        };
        let snapshot = Snapshot::production_hard_ceiling();
        let observed = owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
            let slot = budget as *const Budget<'_> as usize;
            let ledger = budget.work_ledger_identity_v1();
            let before = budget.storage();
            let work = budget.work();
            let peak = budget.peak_storage();
            budget.charge_work(4 * HEADERS)?;
            budget.reserve_storage(HEADERS)?;
            let mut owned = HEADERS;
            // Exact same physical owner pattern as the original genuine stream.
            // It outlives all richer/source/facts borrows and their postflights.
            let mut pending = PendingActualRootPrefixIndicesV1::new();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                with_nominal_prepared_control_flow_v1(
                    owner,
                    inventory,
                    source.root(),
                    source.call_block(),
                    source.source_call(),
                    budget,
                    |flow, budget| {
                        owner.with_checked_bf16_nominal_call_v1(
                            inventory,
                            source.root(),
                            source.root(),
                            source.call_block(),
                            source.source_call(),
                            budget,
                            |checked, budget| {
                                assert_eq!(
                                    flow.effects().original().candidate().permutation(),
                                    source.return_permutation()
                                );
                                with_nominal_rich_source_preparation_v1(
                                    owner,
                                    inventory,
                                    source.root(),
                                    source.root(),
                                    source.call_block(),
                                    source.source_call(),
                                    budget,
                                    |rich, budget| {
                                        with_nominal_canonical_facts_observation_v1(
                                            owner,
                                            inventory,
                                            source.root(),
                                            source.root(),
                                            source.call_block(),
                                            source.source_call(),
                                            budget,
                                            |facts| {
                                                with_nominal_recipe_resources_v1(
                                                    facts,
                                                    rich,
                                                    &mut owned,
                                                    |context| {
                                                        context.with_actual_root_block_stream_v1(
                                                            checked,
                                                            rich,
                                                            actual_inputs,
                                                            flow,
                                                            &mut pending,
                                                            |view, context| {
                                                                // Real constructor + fixed mandatory nine-pass pipeline
                                                                // and replay, then copied diagnostics only. Live lowering
                                                                // remains inside Pending through every postflight below.
                                                                classify(
                                                                    case,
                                                                    view.verify_ranked(
                                                                        context, analysis, snapshot,
                                                                    ),
                                                                )
                                                            },
                                                        )
                                                    },
                                                )
                                                .map_err(projection)
                                            },
                                        )
                                    },
                                )
                            },
                        )
                    },
                )
            }));
            let result = match outcome {
                Ok(result) => result,
                Err(payload) => {
                    drop(payload);
                    Err(QueryError::CallbackPanicked)
                }
            };
            // Includes the actual successful Pliron session, access mappings,
            // tensors and all source scratch. No refund precedes these drops.
            drop(pending);
            if slot != budget as *const Budget<'_> as usize
                || ledger != budget.work_ledger_identity_v1()
                || budget.storage()
                    < before
                        .checked_add(owned)
                        .ok_or(QueryError::Resource(Resource::Arithmetic))?
                || budget.work() < work
                || budget.peak_storage() < peak
            {
                let _ = result;
                return Err(QueryError::Resource(Resource::Accounting));
            }
            budget.release_storage(owned)?;
            if result.is_ok()
                && (budget.failed_work().is_some() || budget.failed_storage().is_some())
            {
                return Err(QueryError::Resource(Resource::Accounting));
            }
            assert_eq!(budget.storage(), before);
            result
        })?;
        eprintln!(
            "fe2o3-nominal-ranked-consumer-v2 case={:?} root={} source_call_block={} permutation={:?} ranked_blocks={} emitted_sources={} retained_access_sources={} unmapped_private_reads={} tensor_sites={} verifier_clean={} expected_analysis_refusal={} prepaid_analysis_work={} prepaid_analysis_storage={} prepaid_snapshot_work={} same_ledger=true storage_restored=true normal_admission=false",
            case,
            source.root().index(),
            source.call_block().index(),
            source.return_permutation(),
            observed.blocks,
            observed.emitted_sources,
            observed.accesses,
            observed.unmapped_private_reads,
            observed.tensors,
            observed.verifier_clean,
            observed.expected_analysis_refusal,
            analysis.max_work(),
            analysis.max_peak_storage(),
            snapshot.max_work()
        );
    }
    Ok(())
}

#[test]
fn genuine_nominal_consumer_negative_requires_exact_analysis_error_not_any_error() {
    assert!(
        classify(
            Case::ZeroAnalysis,
            Err(Error::Incomplete("unrelated source refusal"))
        )
        .is_err()
    );
    assert!(classify(Case::Positive, Err(Error::Incomplete("mandatory failure"))).is_err());
    let exact = Error::Compile {
        error: Box::new(ProductionRankedCompileErrorV1::Session(
            ProductionSessionErrorV1::AnalysisResourceLimit {
                phase: fe2o3_pliron::ProductionAnalysisResourcePhaseV1::PipelineVerification,
                producing_pass: None,
                resource: "work",
            },
        )),
        ranked_ir: String::new(),
        access_sources: Vec::new(),
    };
    let result = classify(Case::ZeroAnalysis, Err(exact)).unwrap();
    assert!(result.expected_analysis_refusal);
    assert!(!result.verifier_clean);
    assert_eq!((result.blocks, result.accesses, result.tensors), (0, 0, 0));
    assert_eq!(
        (result.emitted_sources, result.unmapped_private_reads),
        (0, 0)
    );
}

#[test]
fn genuine_nominal_consumer_failure_diagnostic_is_typed_and_fixed_without_formatting() {
    assert_eq!(
        failure_kind(&Error::Incomplete("not serialized")),
        "incomplete"
    );
    assert_eq!(
        failure_kind(&Error::Unsupported("not serialized")),
        "unsupported"
    );
    let typed = Error::Compile {
        error: Box::new(ProductionRankedCompileErrorV1::Session(
            ProductionSessionErrorV1::AnalysisResourceLimit {
                phase: fe2o3_pliron::ProductionAnalysisResourcePhaseV1::PipelineVerification,
                producing_pass: None,
                resource: "arbitrary diagnostic payload is not formatted",
            },
        )),
        ranked_ir: "retained graph is not formatted".into(),
        access_sources: Vec::new(),
    };
    assert_eq!(failure_kind(&typed), "analysis_resource");
    // The typed expected-refusal matcher is unchanged; the diagnostic is not it.
    assert!(classify(Case::Positive, Err(typed)).is_err());
}

#[path = "bf16_nominal_tensor_failure_v1_tests.rs"]
mod tensor_diagnostic;
