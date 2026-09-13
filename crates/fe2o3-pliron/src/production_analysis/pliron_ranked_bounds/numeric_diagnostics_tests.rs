use super::*;
use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
use crate::production_analysis::pliron_pass_contract::PlironStructuralIdentityProviderV1;
use dialect_kernel::{DIALECT_NAME, IndexType, register_dialect};
use pliron::{
    basic_block::BasicBlock, builtin::types::FunctionType, context::Ptr, dialect::DialectName,
    r#type::TypeHandle,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    context
}

fn function(context: &mut Context, name: &str, arguments: usize) -> (FuncOp, Vec<Value>) {
    let index: TypeHandle = IndexType::get(context).into();
    let ty = FunctionType::get(context, vec![index; arguments], vec![]);
    let function = FuncOp::new(context, name.try_into().unwrap(), ty);
    let values = function
        .get_entry_block(context)
        .deref(context)
        .arguments()
        .collect();
    (function, values)
}

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn unlimited() -> ProductionAnalysisResourceLimitsV1 {
    ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX)
}

fn dynamic_access(context: &mut Context) -> (FuncOp, Value, Value, Value) {
    let (function, arguments) = function(context, "numeric_diagnostics", 2);
    let [index, extent]: [Value; 2] = arguments.try_into().unwrap();
    let entry = function.get_entry_block(context);
    let ty = RankedViewType::new(context, 32, false, vec![0]).unwrap();
    let view = RankedViewOp::new(context, ty, vec![extent]).unwrap();
    append(context, entry, &view);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![index],
    )
    .unwrap();
    append(context, entry, &access);
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);
    (function, view.result(context), index, extent)
}

#[test]
fn bounds_diagnostic_aliases_do_not_change_identity_findings_or_admission() {
    let context = &mut setup();
    let (function, view, index, extent) = dynamic_access(context);
    verify_operation(function.get_operation(), context).unwrap();
    let expected = run_pliron_ranked_bounds_check_v1(context, &function);
    // Retain the same number of debug-attribute owners in both censuses.
    for value in [view, index, extent] {
        value.set_name(context, Some("initial_alias".try_into().unwrap()));
    }
    let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
    let before = provider
        .capture_with_resource_limits_v1(unlimited())
        .ok()
        .unwrap();
    assert!(matches!(
        expected.findings(),
        [RankedBoundsFindingV1::UnprovedBound { .. }]
    ));
    let bound =
        preflight_ranked_bounds_resource_upper_bound_v1(before.input_census, unlimited()).unwrap();
    for alias in ["short_alias".to_owned(), "d".repeat(262_144)] {
        for value in [view, index, extent] {
            value.set_name(context, Some(alias.clone().try_into().unwrap()));
        }
        verify_operation(function.get_operation(), context).unwrap();
        let observed = provider
            .capture_with_resource_limits_v1(unlimited())
            .ok()
            .unwrap();
        assert!(
            provider
                .require_exact_identity(&before.snapshot, &observed.snapshot)
                .is_ok()
        );
        assert_eq!(observed.input_census, before.input_census);
        assert_eq!(
            preflight_ranked_bounds_resource_upper_bound_v1(observed.input_census, unlimited()),
            Ok(bound)
        );
        let report = run_pliron_ranked_bounds_check_v1(context, &function);
        assert_eq!(report, expected);
        let [
            RankedBoundsFindingV1::UnprovedBound {
                view: name,
                index: lhs,
                extent: rhs,
                ..
            },
        ] = report.findings()
        else {
            panic!("the same unproved relation must remain");
        };
        for text in [name, lhs, rhs] {
            assert!(text.starts_with('v'));
            assert!(text[1..].bytes().all(|byte| byte.is_ascii_digit()));
            assert!(text.capacity() <= 64);
        }
        assert!(!report.findings()[0].to_string().contains(&alias));
    }
}

#[test]
fn numeric_expression_variants_fit_the_requested_buffer() {
    let context = &mut setup();
    let (function, view, index, _) = dynamic_access(context);
    verify_operation(function.get_operation(), context).unwrap();
    assert_eq!(IndexExpr::Constant(0).describe(context), "0");
    let largest = IndexExpr::Constant(u64::MAX).describe(context);
    assert_eq!(largest, "18446744073709551615");
    assert_eq!(largest.len(), 20);
    assert!(largest.capacity() <= 64);
    let id: String = index.id(context).into();
    assert_eq!(IndexExpr::Value(index).describe(context), id);
    for dimension in [0, 7, usize::MAX] {
        let text = IndexExpr::Dimension { view, dimension }.describe(context);
        assert_eq!(text, format!("{}.dim<{dimension}>()", view.id(context)));
        assert!(text.len() <= 49);
        assert_eq!(text.capacity(), 64);
    }
    // Independent pinned-Value renderer maximum, without forging a Value UID.
    let largest_id = format!("v{}", u64::MAX);
    assert_eq!(largest_id.len(), 21);
    assert!(largest_id.capacity() <= 64);
    let mut largest_dimension = String::with_capacity(64);
    use std::fmt::Write as _;
    write!(largest_dimension, "{largest_id}.dim<{}>()", usize::MAX).unwrap();
    assert_eq!(largest_dimension.len(), 49);
    assert_eq!(largest_dimension.capacity(), 64);
}

#[test]
fn every_sparse_error_rendering_fits_the_singleton_allowance() {
    assert_eq!(usize::BITS, 64);
    let cases = [
        (
            SparseIndexFailureV1::ResourceLimit {
                resource: "sparse fact publications",
                limit: usize::MAX,
                actual: usize::MAX,
            },
            80,
        ),
        (
            SparseIndexFailureV1::InconsistentLaunchExtent {
                dimension: usize::MAX,
                first: u64::MAX,
                second: u64::MAX,
            },
            119,
        ),
        (
            SparseIndexFailureV1::MalformedControlFlow {
                detail: "a block argument has a predecessor without typed edge operands",
            },
            62,
        ),
    ];
    for (failure, length) in cases {
        let RankedBoundsFindingV1::SparseIndexAnalysisFailed { detail } =
            sparse_index_failure(failure)
        else {
            panic!("the sparse failure variant must be preserved");
        };
        assert_eq!(detail.len(), length);
        assert!(detail.capacity() <= 256);
    }
}

fn literal_census(identifier_bytes: usize) -> ProductionAnalysisInputCensusV1 {
    ProductionAnalysisInputCensusV1 {
        blocks: 1,
        operations: 1,
        operands: 3,
        identifier_bytes,
        ..ProductionAnalysisInputCensusV1::default()
    }
}

fn assert_literal_boundary(
    census: ProductionAnalysisInputCensusV1,
    work: usize,
    retained: usize,
    peak: usize,
) {
    let exact = preflight_ranked_bounds_resource_upper_bound_v1(
        census,
        ProductionAnalysisResourceLimitsV1::new(work, peak),
    )
    .unwrap();
    assert_eq!(exact.work_upper_bound(), work);
    assert_eq!(exact.retained_storage_upper_bound(), retained);
    assert_eq!(exact.peak_storage_upper_bound(), peak);
    assert_eq!(
        preflight_ranked_bounds_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(work - 1, peak),
        ),
        Err(ranked_bounds_resource_error_v1("work upper bound"))
    );
    assert_eq!(
        preflight_ranked_bounds_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(work, peak - 1),
        ),
        Err(ranked_bounds_resource_error_v1("peak storage upper bound"))
    );
}

#[test]
fn numeric_report_literal_exact_and_one_under() {
    // S=22, internal=15, graph+companion scan work=21; four numeric findings.
    // Work: 3S+21+3*1048577 +4*768+4*17+1024 = 3149982.
    // Retained=max(4*256, 2*17+320)=1024.
    // Temporary=15+22+48+34+128=247; peak=1271.
    assert_literal_boundary(literal_census(17), 3_149_982, 1_024, 1_271);
}

#[test]
fn singleton_report_literal_exact_and_one_under() {
    // S=1029, internal=15, graph+companion scan=21.
    // Work: 3S+21+3*1048577+4*768+4096+1024 = 3157031.
    // Retained=max(1024,2048+320)=2368; temporary=3268.
    assert_literal_boundary(literal_census(1_024), 3_157_031, 2_368, 5_636);
}

#[test]
fn diagnostic_bound_checked_arithmetic_remains_fail_closed() {
    assert_eq!(
        preflight_ranked_bounds_resource_upper_bound_v1(literal_census(usize::MAX), unlimited()),
        Err(ranked_bounds_resource_error_v1("structural input census"))
    );
    let census = ProductionAnalysisInputCensusV1 {
        identifier_bytes: usize::MAX / 4 + 1,
        ..ProductionAnalysisInputCensusV1::default()
    };
    assert!(preflight_ranked_bounds_resource_upper_bound_v1(census, unlimited()).is_err());
}

#[test]
fn authentic_many_accesses_do_not_retain_unrelated_function_text_per_finding() {
    let context = &mut setup();
    let (function, _) = function(context, &"f".repeat(32_768), 0);
    let entry = function.get_entry_block(context);
    let ty = RankedViewType::new(context, 32, false, vec![8]).unwrap();
    let view = RankedViewOp::new(context, ty, vec![]).unwrap();
    append(context, entry, &view);
    let zero = IndexConstantOp::new(context, 0);
    append(context, entry, &zero);
    for _ in 0..600 {
        let access = RankedAccessOp::new(
            context,
            AccessKindAttr::Read,
            view.result(context),
            vec![zero.result(context)],
        )
        .unwrap();
        append(context, entry, &access);
    }
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);
    verify_operation(function.get_operation(), context).unwrap();
    let capture = LivePlironStructuralIdentityProviderV1::new(context, &function)
        .capture_with_resource_limits_v1(unlimited())
        .ok()
        .unwrap();
    let census = capture.input_census;
    assert_eq!(
        (census.blocks, census.operations, census.operands),
        (1, 603, 1_200)
    );
    assert_eq!(census.ranked_accesses, 600);
    let findings = (census.blocks + census.operands).min(4_096);
    let former_retained = findings * (64 + 3 * (census.identifier_bytes + 64));
    assert!(former_retained > 93_323_264);
    let bound = preflight_ranked_bounds_resource_upper_bound_v1(
        census,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap();
    assert!(bound.peak_storage_upper_bound() < former_retained);
    assert!(run_pliron_ranked_bounds_check_v1(context, &function).is_clean());
}

#[test]
fn finding_capacity_rejection_still_precedes_numeric_rendering() {
    use std::cell::Cell;
    let mut budget = RankedBoundsBudget {
        findings: MAX_RANKED_BOUNDS_FINDINGS,
        ..RankedBoundsBudget::default()
    };
    let mut findings = Vec::new();
    let rendered = Cell::new(false);
    let error = push_finding(&mut findings, &mut budget, || {
        rendered.set(true);
        RankedBoundsFindingV1::UnreachableBlock { block: 0 }
    });
    assert!(matches!(
        error,
        Err(RankedBoundsFindingV1::ResourceLimitExceeded {
            resource: "finding",
            actual: 4_097,
            limit: 4_096
        })
    ));
    assert!(!rendered.get());
    assert!(findings.is_empty());
}
