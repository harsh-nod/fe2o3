use super::*;

#[cfg(test)]
mod exhausted_boundaries {
    include!("dynamic_coordinate_exhaustion_v1_tests.rs");
}
use dialect_kernel::{DIALECT_NAME, register_dialect};
use pliron::{dialect::DialectName, op::Op, operation::verify_operation, parsable::parse_from_str};

const DYNAMIC: &str = include_str!("../tests/lit/pipeline_dynamic_double_buffer.pliron");

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

fn constants() -> (Context, [Value; 4]) {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    let values =
        [1, 0, 0, 1].map(|value| IndexConstantOp::new(&mut context, value).result(&context));
    (context, values)
}

#[test]
fn occurrence_order_has_literal_exact_and_one_under_query_boundaries() {
    // Distinct SSA constants: L=[one_a,zero_a,one_a], R=[zero_b,one_b].
    // L->R costs2+1+2 queries, R->L costs2+1. Six distinct ordered
    // pairs are expanded; duplicate rows/memo hits still count as queries.
    for _ in 0..8 {
        let (context, [one_a, zero_a, zero_b, one_b]) = constants();
        let left = vec![vec![one_a], vec![zero_a], vec![one_a]];
        let right = vec![vec![zero_b], vec![one_b]];
        let mut exact = EquivalenceResourceMeterV1::new(8, 6).unwrap();
        assert!(coordinate_sets_equivalent_v1(
            &context, &left, &right, &mut exact
        ));
        assert_eq!(
            (exact.queries, exact.expanded_pairs, exact.exhausted()),
            (8, 6, false)
        );
        let mut under = EquivalenceResourceMeterV1::new(7, 6).unwrap();
        assert!(!coordinate_sets_equivalent_v1(
            &context, &left, &right, &mut under
        ));
        assert_eq!(
            (under.queries, under.expanded_pairs, under.exhausted()),
            (8, 5, true)
        );
    }
}

#[test]
fn shared_memo_reuse_does_not_skip_ordered_query_charges() {
    let (context, [one_a, zero_a, zero_b, one_b]) = constants();
    let left = vec![vec![one_a], vec![zero_a], vec![one_a]];
    let right = vec![vec![zero_b], vec![one_b]];
    for (limit, accepted) in [(16, true), (15, false)] {
        let mut meter = EquivalenceResourceMeterV1::new(limit, 6).unwrap();
        assert!(coordinate_sets_equivalent_v1(
            &context, &left, &right, &mut meter
        ));
        assert_eq!((meter.queries, meter.expanded_pairs), (8, 6));
        assert_eq!(
            coordinate_sets_equivalent_v1(&context, &left, &right, &mut meter),
            accepted
        );
        assert_eq!(
            (meter.queries, meter.expanded_pairs, meter.exhausted()),
            (16, 6, !accepted)
        );
    }
}

#[test]
fn bidirectional_membership_ignores_duplicate_counts_but_not_missing_coordinates() {
    let (context, [one_a, zero_a, zero_b, one_b]) = constants();
    for (left, right, expected) in [
        (
            vec![vec![one_a], vec![zero_a], vec![one_a]],
            vec![vec![zero_b], vec![one_b]],
            true,
        ),
        (
            vec![vec![zero_a], vec![one_a]],
            vec![vec![one_b], vec![zero_b], vec![zero_b]],
            true,
        ),
        (vec![vec![zero_a], vec![one_a]], vec![vec![zero_b]], false),
        (vec![vec![zero_a]], vec![vec![zero_b], vec![one_b]], false),
        (vec![vec![zero_a, one_a]], vec![vec![zero_b]], false),
    ] {
        let mut meter = EquivalenceResourceMeterV1::new(32, 16).unwrap();
        assert_eq!(
            coordinate_sets_equivalent_v1(&context, &left, &right, &mut meter),
            expected
        );
        assert!(!meter.exhausted());
    }
}

#[test]
fn dynamic_windows_keep_actual_access_counts_without_multiset_semantics() {
    for (prologue, steady, reads) in [(3, 2, 4), (1, 4, 1)] {
        let source = DYNAMIC
            .lines()
            .flat_map(|line| {
                let count = if line.contains("kernel.access (v0, zero_v2,") {
                    prologue
                } else if line.contains("kernel.access (v0, future_slot_v9,") {
                    steady
                } else if line.contains("kernel.access (v0, current_slot_v10,") {
                    reads
                } else {
                    1
                };
                std::iter::repeat_n(line, count)
            })
            .collect::<Vec<_>>()
            .join("\n");
        let (context, function) = parse(&source);
        let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
        assert!(report.is_clean(), "{report:?}");
        let [certificate] = report.certificates() else {
            panic!("one exact pipeline");
        };
        assert_eq!(certificate.staged_writes(), prologue + steady);
        assert_eq!(certificate.consuming_reads(), reads);
        assert!(certificate.dynamic_loop().is_some());
        assert!(certificate.access_refinement_proven());
        assert!(!report.grants_compiler_refinement_authority());
        assert!(!report.grants_artifact_or_launch_authority());
    }
}

#[test]
fn canonical_window_and_later_windows_must_contain_the_same_coordinate_set() {
    for slot in ["zero_v2", "future_slot_v9", "current_slot_v10"] {
        let access = DYNAMIC
            .lines()
            .find(|line| line.contains(&format!("kernel.access (v0, {slot},")))
            .unwrap();
        let extra = access.replace(
            &format!("(v0, {slot}, zero_v2)"),
            &format!("(v0, {slot}, one_v3)"),
        );
        let source = DYNAMIC.replace(access, &format!("{access}\n{extra}"));
        let (context, function) = parse(&source);
        let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
        assert!(
            matches!(report.findings(), [PlironPipelineProtocolFindingV1::InvalidSchedule { detail, .. }]
            if detail.ends_with("epoch coordinates do not match the staged/consumed symbolic tile")),
            "{report:?}"
        );
        assert!(report.certificates().is_empty());
    }
    let empty_prologue = DYNAMIC
        .lines()
        .filter(|line| !line.contains("kernel.access (v0, zero_v2,"))
        .collect::<Vec<_>>()
        .join("\n");
    let (context, function) = parse(&empty_prologue);
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert!(
        matches!(report.findings(), [PlironPipelineProtocolFindingV1::InvalidSchedule { detail, .. }]
        if detail == "a consumed symbolic tile is not initialized in every prologue and steady-state epoch")
    );
}

#[test]
fn occurrence_pair_bound_does_not_assume_deduplication() {
    // Canonical3, remaining windows2+4: 2*3*6=36 candidate pairs,
    // bounded by2*A^2=162 for A9. Arity3 contributes486 queries.
    let census = ProductionAnalysisInputCensusV1 {
        pipeline_creates: 1,
        ranked_accesses: 9,
        max_operation_arity: 3,
        pipeline_events: 10,
        ..ProductionAnalysisInputCensusV1::default()
    };
    assert_eq!(
        pipeline_equivalence_query_upper_bound_v1(census).unwrap(),
        486 + 40 + 63
    );
    // Only collection ordering changes: owned coordinate rows remain Vec<Value>,
    // and the outer vector header is no larger than the former hash-set header.
    assert!(std::mem::size_of::<Vec<Vec<Value>>>() <= std::mem::size_of::<HashSet<Vec<Value>>>());
}
