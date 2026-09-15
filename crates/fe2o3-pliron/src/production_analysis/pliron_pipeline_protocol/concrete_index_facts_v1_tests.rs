use super::*;
use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceContractV1;
use dialect_kernel::{DIALECT_NAME, register_dialect};
use pliron::{dialect::DialectName, op::Op, operation::verify_operation, parsable::parse_from_str};

const COMPUTED: &str = include_str!("../tests/lit/pipeline_computed_constant.pliron");
const AMBIGUOUS: &str = include_str!("../tests/lit/pipeline_computed_ambiguous_slot.pliron");

fn parse(source: &str) -> (Context, FuncOp) {
    let ir = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    let operation = parse_from_str(Operation::top_level_parser(), &mut context, &ir).unwrap();
    verify_operation(operation, &context).unwrap();
    assert!(Operation::is_op::<FuncOp>(operation, &context));
    (context, FuncOp::from_operation(operation))
}

fn census(context: &Context, function: &FuncOp) -> ProductionAnalysisInputCensusV1 {
    let inventory = BoundedPlironFunctionInventoryV1::collect(context, function).unwrap();
    pipeline_protocol_inventory_census_v1(context, &inventory).unwrap()
}

fn assert_clean(report: &PlironPipelineProtocolReportV1) {
    assert!(report.is_clean(), "{:?}", report.findings());
    assert_eq!(report.certificates().len(), 1);
    assert_eq!(report.certificates()[0].concrete_epochs(), 1);
    assert!(report.certificates()[0].dynamic_loop().is_none());
    assert!(!report.grants_compiler_refinement_authority());
    assert!(!report.grants_artifact_or_launch_authority());
}

#[test]
fn concrete_cross_block_remainder_and_epoch_use_one_reused_sparse_cache() {
    let (context, function) = parse(COMPUTED);
    let census = census(&context, &function);
    let hard = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
    let pipeline = preflight_pipeline_protocol_resource_upper_bound_v1(census, hard).unwrap();
    let sparse =
        preflight_sparse_index_resource_upper_bound_v1(&context, &function, census, hard).unwrap();
    let mut analyses = PlironAnalysisManagerV1::new(&function);
    analyses.prepare_function_inventory(&context, &function);
    assert!(!analyses.sparse_indices_prepared());
    let before = analyses
        .remaining_resource_limits(ProductionAnalysisResourcePhaseV1::SparseIndex)
        .unwrap();
    let report =
        run_pliron_pipeline_protocol_check_with_analyses_v1(&context, &function, &mut analyses);
    assert_clean(&report);
    assert_eq!(report.certificates()[0].staged_writes(), 1);
    assert_eq!(report.certificates()[0].consuming_reads(), 1);
    let pointer = std::ptr::from_ref(analyses.sparse_indices().unwrap());
    let entries = analyses.cached_entries();
    let after = analyses
        .remaining_resource_limits(ProductionAnalysisResourcePhaseV1::SparseIndex)
        .unwrap();
    assert_eq!(
        before.max_work() - after.max_work(),
        pipeline.work_upper_bound() + sparse.work_upper_bound()
    );
    assert_eq!(
        before.max_peak_storage() - after.max_peak_storage(),
        pipeline.retained_storage_upper_bound() + sparse.retained_storage_upper_bound()
    );
    assert_clean(&run_pliron_pipeline_protocol_check_with_analyses_v1(
        &context,
        &function,
        &mut analyses,
    ));
    assert_eq!(
        std::ptr::from_ref(analyses.sparse_indices().unwrap()),
        pointer
    );
    assert_eq!(analyses.cached_entries(), entries);
    let repeated = analyses
        .remaining_resource_limits(ProductionAnalysisResourcePhaseV1::SparseIndex)
        .unwrap();
    assert_eq!(
        after.max_work() - repeated.max_work(),
        pipeline.work_upper_bound()
    );
    assert_eq!(
        after.max_peak_storage() - repeated.max_peak_storage(),
        pipeline.retained_storage_upper_bound()
    );
}

#[test]
fn concrete_duplicate_edge_join_requires_all_incoming_slot_facts_to_agree() {
    let (context, function) = parse(AMBIGUOUS);
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert!(
        matches!(report.findings(), [PlironPipelineProtocolFindingV1::InvalidSchedule { detail, .. }]
        if detail == "constant epoch has a non-constant ring slot")
    );
    let agreed = AMBIGUOUS.replace(
        "kernel_index_value: kernel.index_value 1]",
        "kernel_index_value: kernel.index_value 2]",
    );
    let (context, function) = parse(&agreed);
    assert_clean(&run_pliron_pipeline_protocol_check_v1(&context, &function));
    let wrong = AMBIGUOUS.replace(
        "kernel_index_value: kernel.index_value 2]",
        "kernel_index_value: kernel.index_value 1]",
    );
    let (context, function) = parse(&wrong);
    assert!(!run_pliron_pipeline_protocol_check_v1(&context, &function).is_clean());
}

#[test]
fn concrete_zero_unknown_and_overflow_facts_remain_rejected() {
    let zero = COMPUTED.replace(
        "kernel_index_value: kernel.index_value 3]",
        "kernel_index_value: kernel.index_value 0]",
    );
    let dependent = COMPUTED.replace("(epoch_v6, modulus_v3)", "(lane_v4, modulus_v3)");
    let overflow = COMPUTED
        .replace(
            "kernel_index_value: kernel.index_value 8]",
            "kernel_index_value: kernel.index_value 18446744073709551615]",
        )
        .replace(
            "kernel_index_value: kernel.index_value 0]",
            "kernel_index_value: kernel.index_value 1]",
        );
    let unknown = COMPUTED
        .lines()
        .filter(|line| !line.contains("slot_v7 = kernel.index_binary"))
        .collect::<Vec<_>>()
        .join("\n")
        .replace(
            "builtin.function <() -> ()>",
            "builtin.function <(kernel.index ) -> ()>",
        )
        .replace("^entry():", "^entry(slot_v7: kernel.index ):");
    for source in [zero, dependent, overflow, unknown] {
        let (context, function) = parse(&source);
        let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
        assert!(!report.is_clean(), "{source}");
        assert!(report.findings().iter().any(|finding| matches!(
            finding,
            PlironPipelineProtocolFindingV1::InvalidSchedule { .. }
        )));
        assert!(report.certificates().is_empty());
    }
}

#[test]
fn literal_only_concrete_schedule_does_not_construct_sparse_cache() {
    let source = include_str!("../tests/lit/pipeline_static_cfg_linear_chain.pliron");
    let (context, function) = parse(source);
    let mut analyses = PlironAnalysisManagerV1::new(&function);
    assert_clean(&run_pliron_pipeline_protocol_check_with_analyses_v1(
        &context,
        &function,
        &mut analyses,
    ));
    assert!(!analyses.sparse_indices_prepared());
}

#[test]
fn cached_sparse_failure_is_incomplete_and_is_not_rebuilt_or_recharged() {
    let original = COMPUTED
        .lines()
        .find(|line| line.contains("lane_v4 ="))
        .unwrap();
    let conflicting = original.replace("lane_v4", "other_lane_v9").replace(
        "kernel_launch_extent: kernel.launch_extent 64",
        "kernel_launch_extent: kernel.launch_extent 32",
    );
    let source = COMPUTED.replace(original, &format!("{original}\n{conflicting}"));
    let (context, function) = parse(&source);
    let census = census(&context, &function);
    let pipeline = preflight_pipeline_protocol_resource_upper_bound_v1(
        census,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap();
    let mut analyses = PlironAnalysisManagerV1::new(&function);
    let report =
        run_pliron_pipeline_protocol_check_with_analyses_v1(&context, &function, &mut analyses);
    assert!(matches!(
        report.findings(),
        [PlironPipelineProtocolFindingV1::AnalysisIncomplete { .. }]
    ));
    assert!(report.certificates().is_empty());
    assert!(analyses.sparse_indices_prepared());
    assert!(analyses.sparse_indices().is_err());
    let entries = analyses.cached_entries();
    let before = analyses
        .remaining_resource_limits(ProductionAnalysisResourcePhaseV1::SparseIndex)
        .unwrap();
    let report =
        run_pliron_pipeline_protocol_check_with_analyses_v1(&context, &function, &mut analyses);
    assert!(matches!(
        report.findings(),
        [PlironPipelineProtocolFindingV1::AnalysisIncomplete { .. }]
    ));
    assert_eq!(analyses.cached_entries(), entries);
    let after = analyses
        .remaining_resource_limits(ProductionAnalysisResourcePhaseV1::SparseIndex)
        .unwrap();
    assert_eq!(
        before.max_work() - after.max_work(),
        pipeline.work_upper_bound()
    );
    assert_eq!(
        before.max_peak_storage() - after.max_peak_storage(),
        pipeline.retained_storage_upper_bound()
    );
}

#[test]
fn sparse_failure_after_a_literal_creation_discards_every_partial_certificate() {
    let view = COMPUTED
        .lines()
        .find(|line| line.contains("v0 = kernel.ranked_view"))
        .unwrap();
    let first_view = view
        .replace("v0 =", "first_view_v10 =")
        .replace(
            "kernel.allocation_origin 301",
            "kernel.allocation_origin 302",
        )
        .replace("kernel.noalias_class 41", "kernel.noalias_class 42");
    let create = COMPUTED
        .lines()
        .find(|line| line.contains("p1 = kernel.pipeline_create"))
        .unwrap();
    let first_create = create
        .replace("p1 =", "first_pipeline_v11 =")
        .replace("(v0)", "(first_view_v10)");
    let event = COMPUTED
        .lines()
        .find(|line| line.contains("kernel.pipeline_event") && line.contains(" Stage]"))
        .unwrap()
        .replace(
            "(p1, epoch_v6, slot_v7)",
            "(first_pipeline_v11, zero_v2, zero_v2)",
        );
    let first_schedule = ["Stage", "Commit", "Wait", "Consume", "Release"]
        .map(|kind| event.replace("kind Stage]", &format!("kind {kind}]")))
        .join("\n");
    let source = COMPUTED
        .replace(view, &format!("{first_view}\n{first_create}\n{view}"))
        .replacen(
            "    kernel.br () [^stage]",
            &format!("{first_schedule}\n    kernel.br () [^stage]"),
            1,
        );
    let (context, function) = parse(&source);
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(report.certificates().len(), 2);
    assert_eq!(report.certificates()[0].pipeline_block(), 0);
    assert_eq!(report.certificates()[0].pipeline_operation(), 2);
    assert_eq!(report.certificates()[0].concrete_epochs(), 1);
    assert_eq!(report.certificates()[0].staged_writes(), 0);
    assert_eq!(report.certificates()[1].staged_writes(), 1);
    assert_eq!(report.certificates()[1].consuming_reads(), 1);

    // Both exact schedules and disjoint storage owners remain unchanged. The
    // first creation uses only literals, so only the second can touch sparse.
    let original = source
        .lines()
        .find(|line| line.contains("lane_v4 ="))
        .unwrap();
    let conflicting = original.replace("lane_v4", "other_lane_v9").replace(
        "kernel_launch_extent: kernel.launch_extent 64",
        "kernel_launch_extent: kernel.launch_extent 32",
    );
    let source = source.replace(original, &format!("{original}\n{conflicting}"));
    let (context, function) = parse(&source);
    let mut analyses = PlironAnalysisManagerV1::new(&function);
    let report =
        run_pliron_pipeline_protocol_check_with_analyses_v1(&context, &function, &mut analyses);
    assert!(matches!(
        report.findings(),
        [PlironPipelineProtocolFindingV1::AnalysisIncomplete { detail }]
            if detail == "concrete pipeline index facts could not be admitted"
    ));
    assert!(report.certificates().is_empty());
    assert!(analyses.sparse_indices_prepared());
    assert!(matches!(
        analyses.sparse_indices(),
        Err(crate::production_analysis::pliron_sparse_index::SparseIndexFailureV1::InconsistentLaunchExtent {
            dimension: 0, first: 64, second: 32,
        })
    ));
}

#[test]
fn denied_sparse_initialization_stops_the_protocol_with_no_partial_certificate() {
    let (context, function) = parse(COMPUTED);
    let census = census(&context, &function);
    let pipeline = preflight_pipeline_protocol_resource_upper_bound_v1(
        census,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap();
    let setup = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::StructuralIdentity,
        7,
        11,
        13,
    )
    .unwrap();
    let limit = 7 + census.blocks + census.operations + 1 + pipeline.work_upper_bound();
    let mut analyses = PlironAnalysisManagerV1::new_with_resource_contract(
        &function,
        census,
        setup,
        0,
        ProductionAnalysisResourceLimitsV1::new(limit, 1_000_000_000),
    )
    .unwrap();
    analyses
        .admit_retained_resource_upper_bound(
            ProductionAnalysisResourcePhaseV1::PipelineProtocol,
            pipeline,
        )
        .unwrap();
    let report =
        run_pliron_pipeline_protocol_check_with_analyses_v1(&context, &function, &mut analyses);
    assert!(
        matches!(report.findings(), [PlironPipelineProtocolFindingV1::AnalysisIncomplete { detail }]
        if detail == "concrete pipeline index facts could not be admitted")
    );
    assert!(report.certificates().is_empty());
    assert!(!analyses.sparse_indices_prepared());
    assert_eq!(
        analyses
            .remaining_resource_limits(ProductionAnalysisResourcePhaseV1::SparseIndex)
            .unwrap()
            .max_work(),
        0
    );
}

#[test]
fn concrete_query_work_is_derived_from_fixed_rank_and_all_possible_queries() {
    assert_eq!(dialect_kernel::MAX_RANKED_MEMORY_RANK, 8);
    for (creates, events, accesses, expected) in [
        (0, usize::MAX, usize::MAX, 0),
        (1, 0, 0, 1),
        (1, 5, 2, 145),
        (2, 4, 2, 242),
        (2, 10, 4, 578),
        (3, 11, 7, 1047),
    ] {
        assert_eq!(
            pipeline_concrete_fact_query_work_v1(ProductionAnalysisInputCensusV1 {
                pipeline_creates: creates,
                pipeline_events: events,
                ranked_accesses: accesses,
                ..ProductionAnalysisInputCensusV1::default()
            })
            .unwrap(),
            expected
        );
    }
    assert!(
        pipeline_concrete_fact_query_work_v1(ProductionAnalysisInputCensusV1 {
            pipeline_creates: 1,
            pipeline_events: usize::MAX,
            ..ProductionAnalysisInputCensusV1::default()
        })
        .is_err()
    );
}

#[test]
fn sparse_peak_and_live_pipeline_scratch_have_independent_exact_boundaries() {
    let census = ProductionAnalysisInputCensusV1 {
        blocks: 3,
        successors: 4,
        operations: 12,
        operands: 14,
        results: 3,
        max_operation_arity: 4,
        pipeline_creates: 2,
        pipeline_events: 4,
        ranked_accesses: 2,
        ..ProductionAnalysisInputCensusV1::default()
    };
    let phase = ProductionAnalysisResourcePhaseV1::SparseIndex;
    let pipeline = preflight_pipeline_protocol_resource_upper_bound_v1(
        census,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap();
    // Existing census: W=2840+242 query work, R=14150, peak=19103.
    // A symbolic sparse phase W17/R19/T23 overlaps Ptemp=4953.
    // Caller W7/R11/T13 yields W3106 and peak11+14150+4953+42=19156.
    assert_eq!(
        (
            pipeline.work_upper_bound(),
            pipeline.retained_storage_upper_bound(),
            pipeline.peak_storage_upper_bound()
        ),
        (3082, 14150, 19103)
    );
    let sparse = ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 17, 19, 23).unwrap();
    let nested = concrete_index_nested_bound_v1(census, sparse).unwrap();
    assert_eq!(
        (
            nested.work_upper_bound(),
            nested.retained_storage_upper_bound(),
            nested.peak_storage_upper_bound()
        ),
        (17, 19, 4995)
    );
    for (work, peak, expected) in [
        (3106, 19156, None),
        (3105, 19156, Some("work upper bound")),
        (3106, 19155, Some("peak storage upper bound")),
    ] {
        let mut ledger = ProductionAnalysisResourceContractV1::new(
            ProductionAnalysisResourceLimitsV1::new(work, peak),
        );
        ledger
            .admit_retained(
                phase,
                ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 7, 11, 13).unwrap(),
            )
            .unwrap();
        ledger
            .admit_retained(
                ProductionAnalysisResourcePhaseV1::PipelineProtocol,
                pipeline,
            )
            .unwrap();
        let result = ledger.admit_retained(phase, nested);
        match expected {
            None => assert!(result.is_ok()),
            Some(resource) => assert_eq!(
                result,
                Err(ProductionAnalysisResourceLimitV1 { phase, resource })
            ),
        }
    }
}
