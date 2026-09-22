use super::*;
use pliron::{builtin::op_interfaces::OneRegionInterface, parsable::parse_from_str};

pub(super) fn source(name: &str) -> &'static str {
    match name {
        "dynamic" => include_str!("../tests/lit/native_progress_dynamic_unsigned.pliron"),
        "signed" => include_str!("../tests/lit/native_progress_signed_initial.pliron"),
        "narrow" => include_str!("../tests/lit/native_progress_narrow_nowrap.pliron"),
        "wrap" => include_str!("../tests/lit/native_progress_wrap_refused.pliron"),
        "nested" => include_str!("../tests/lit/native_progress_nested_payload.pliron"),
        "duplicate" => include_str!("../tests/lit/native_progress_duplicate_entries.pliron"),
        "conflict" => include_str!("../tests/lit/native_progress_conflicting_edge.pliron"),
        "zero" => include_str!("../tests/lit/native_progress_zero_step.pliron"),
        "no_exit" => include_str!("../tests/lit/native_progress_no_exit.pliron"),
        "malformed" => include_str!("../tests/lit/native_progress_malformed_compare.pliron"),
        _ => panic!("unknown native progress fixture"),
    }
}

pub(super) fn parse(source: &str) -> (Context, FuncOp) {
    let mut context = Context::new();
    fe2o3_pliron_owner_core::ensure_context_identity(&mut context).unwrap();
    dialect_kernel::register_dialect(
        &mut context,
        &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    let ir = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let operation = parse_from_str(Operation::top_level_parser(), &mut context, &ir).unwrap();
    assert!(Operation::is_op::<FuncOp>(operation, &context));
    (context, FuncOp::from_operation(operation))
}

pub(super) fn blocks(context: &Context, function: &FuncOp) -> Vec<Ptr<BasicBlock>> {
    function
        .get_region(context)
        .deref(context)
        .iter(context)
        .collect()
}

fn report_source(source: &str) -> PlironProgressReportV1 {
    let (context, function) = parse(source);
    verify_operation(function.get_operation(), &context).unwrap();
    run_pliron_progress_check_v1(&context, &function)
}

#[test]
fn native_progress_actual_fixed_nine_retain_nonvacuous_certificates_and_identity() {
    for (name, count) in [
        ("dynamic", 1),
        ("signed", 1),
        ("narrow", 1),
        ("nested", 2),
        ("duplicate", 1),
    ] {
        let (context, function) = parse(source(name));
        let before = function.disp(&context).to_string();
        let pointer = function.get_operation();
        let original_blocks = blocks(&context, &function);
        let complete =
            crate::require_production_pliron_checks_before_lowering_v2(&context, &function)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            complete.pass_order(),
            &crate::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
        );
        let progress = complete.semantics().progress();
        assert_eq!(progress.status(), KernelCheckStatusV1::Clean, "{name}");
        assert_eq!(progress.certificates().len(), count, "{name}");
        assert!(
            progress
                .certificates()
                .iter()
                .all(|certificate| certificate.step() > 0)
        );
        assert!(!progress.grants_launch_or_liveness_authority());
        assert_eq!(function.get_operation(), pointer);
        assert_eq!(blocks(&context, &function), original_blocks);
        assert_eq!(function.disp(&context).to_string(), before);
    }
}

#[test]
fn native_progress_fixed_nine_refusals_reach_progress_with_exact_status() {
    for (name, status, code) in [
        (
            "wrap",
            KernelCheckStatusV1::Incomplete,
            "FE2O3-PROGRESS-002",
        ),
        (
            "conflict",
            KernelCheckStatusV1::Incomplete,
            "FE2O3-PROGRESS-002",
        ),
        ("zero", KernelCheckStatusV1::Rejected, "FE2O3-PROGRESS-001"),
        (
            "no_exit",
            KernelCheckStatusV1::Rejected,
            "FE2O3-PROGRESS-001",
        ),
    ] {
        let (context, function) = parse(source(name));
        let progress = run_pliron_progress_check_v1(&context, &function);
        assert_eq!(progress.status(), status, "{name}");
        assert!(progress.certificates().is_empty());
        let failure =
            crate::require_production_pliron_checks_before_lowering_v2(&context, &function)
                .expect_err("native cycle must reach the real Progress refusal");
        assert!(failure.to_string().contains(code), "{name}: {failure}");
    }
    let (context, function) = parse(source("malformed"));
    assert!(verify_operation(function.get_operation(), &context).is_err());
    assert_eq!(
        run_pliron_progress_check_v1(&context, &function).status(),
        KernelCheckStatusV1::Rejected
    );
}

#[test]
fn native_progress_all_fixed_widths_commuted_checked_updates_and_guard_polarities() {
    for signed in [false, true] {
        for width in [8, 16, 32, 64, 128] {
            let ty = format!("{}i{width}", if signed { "s" } else { "u" });
            for (predicate, swapped, reversed) in [
                ("LessThan", false, false),
                ("GreaterThanOrEqual", false, true),
                ("GreaterThan", true, false),
                ("LessThanOrEqual", true, true),
            ] {
                let mut ir = source("dynamic")
                    .replace("ui8", &ty)
                    .replace(
                        "gpu.compare_predicate LessThan",
                        &format!("gpu.compare_predicate {predicate}"),
                    )
                    .replace("gpu.binary (j, step)", "gpu.binary (step, j)");
                if swapped {
                    ir = ir.replace("gpu.compare (i, n)", "gpu.compare (n, i)");
                }
                if reversed {
                    ir = ir.replace("[^body, ^exit]", "[^exit, ^body]");
                }
                assert_eq!(
                    report_source(&ir).status(),
                    KernelCheckStatusV1::Clean,
                    "{ty} {predicate}"
                );
                let checked = ir
                    .replace("next = gpu.binary", "next, overflow = gpu.binary")
                    .replace("gpu.binary_kind Add", "gpu.binary_kind CheckedAdd")
                    .replace(
                        &format!("-> (builtin.integer {ty})>;"),
                        &format!("-> (builtin.integer {ty}, builtin.integer i1)>;"),
                    );
                assert_eq!(report_source(&checked).status(), KernelCheckStatusV1::Clean);
            }
        }
    }
}

#[test]
fn native_progress_signed_overflow_and_dynamic_nonunit_refuse() {
    let signed = source("narrow")
        .replace("ui8", "si8")
        .replace("<254: si8>", "<126: si8>");
    assert_eq!(report_source(&signed).status(), KernelCheckStatusV1::Clean);
    assert_eq!(
        report_source(&signed.replace("<126: si8>", "<127: si8>")).status(),
        KernelCheckStatusV1::Incomplete
    );
    assert_eq!(
        report_source(&source("dynamic").replace("<1: ui8>", "<2: ui8>")).status(),
        KernelCheckStatusV1::Incomplete
    );
}

#[test]
fn native_progress_zero_step_requires_definite_entry_and_recurrence() {
    assert_eq!(
        report_source(source("zero")).status(),
        KernelCheckStatusV1::Rejected
    );
    let inactive = source("zero").replace(
        "initial = gpu.constant <builtin.integer <0:",
        "initial = gpu.constant <builtin.integer <10:",
    );
    let report = report_source(&inactive);
    assert_eq!(report.status(), KernelCheckStatusV1::Clean);
    assert!(report.certificates().is_empty());
    let conditional = source("duplicate").replace("<1: ui8>", "<0: ui8>");
    assert_eq!(
        report_source(&conditional).status(),
        KernelCheckStatusV1::Incomplete
    );
    let mut early = source("zero")
        .replace(
            "builtin.function <()",
            "builtin.function <(builtin.integer i1)",
        )
        .replace("^entry():", "^entry(stop: builtin.integer i1):");
    early = early.replace(
        "gpu.branch (next) [^header] []: <(builtin.integer ui8) -> ()>",
        "gpu.cond_branch (stop, next, next) [^header, ^exit] [operand_segment_sizes: builtin.operand_segment_sizes [1, 1, 1]]: <(builtin.integer i1, builtin.integer ui8, builtin.integer ui8) -> ()>",
    );
    assert_eq!(
        report_source(&early).status(),
        KernelCheckStatusV1::Incomplete
    );
    let positive = early.replace(
        "step = gpu.constant <builtin.integer <0:",
        "step = gpu.constant <builtin.integer <1:",
    );
    assert_eq!(
        report_source(&positive).status(),
        KernelCheckStatusV1::Clean
    );
}

#[test]
fn native_progress_no_exit_conditional_entry_is_not_a_nontermination_witness() {
    let source = r#"
      builtin.func @conditional_cycle: builtin.function <(builtin.integer i1) -> ()> {
        ^entry(choose: builtin.integer i1):
          gpu.cond_branch (choose) [^cycle, ^exit] [operand_segment_sizes: builtin.operand_segment_sizes [1, 0, 0]]: <(builtin.integer i1) -> ()>
        ^cycle():
          gpu.branch () [^cycle] []: <() -> ()>
        ^exit():
          gpu.return () [] []: <() -> ()>
      }
    "#;
    assert_eq!(
        report_source(source).status(),
        KernelCheckStatusV1::Incomplete
    );
}

#[test]
fn native_progress_parallel_edges_preserve_every_occurrence_and_payload() {
    for (name, expected) in [
        ("duplicate", KernelCheckStatusV1::Clean),
        ("conflict", KernelCheckStatusV1::Incomplete),
    ] {
        let (context, function) = parse(source(name));
        verify_operation(function.get_operation(), &context).unwrap();
        let blocks = blocks(&context, &function);
        let indices = blocks.iter().enumerate().map(|(i, b)| (*b, i)).collect();
        let graph = build_root_graph(&context, &blocks, &indices).unwrap();
        assert_eq!(graph.edges[0], vec![1, 1]);
        assert_eq!(
            graph.incoming[1]
                .iter()
                .map(|e| (e.source, e.ordinal))
                .collect::<Vec<_>>(),
            vec![(0, 0), (0, 1), (2, 0)]
        );
        assert_eq!(
            progress_edge_arguments_v1(&context, blocks[0], blocks[1]).is_ok(),
            name == "duplicate"
        );
        assert_eq!(
            run_pliron_progress_check_v1(&context, &function).status(),
            expected
        );
    }
}

#[test]
fn native_progress_reordered_blocks_and_loop_carried_bound() {
    let text = source("dynamic");
    let body = text.find("  ^body").unwrap();
    let exit = text.find("  ^exit").unwrap();
    let end = text.rfind('}').unwrap();
    let reordered = format!(
        "{}{}{}{}",
        &text[..body],
        &text[exit..end],
        &text[body..exit],
        &text[end..]
    );
    let report = report_source(&reordered);
    assert_eq!(report.status(), KernelCheckStatusV1::Clean);
    assert_eq!(report.certificates()[0].body(), 3);
    assert_eq!(report.certificates()[0].exit(), 2);
    let carried = source("nested").replace("gpu.compare (j, n)", "gpu.compare (j, outer_i)");
    assert_eq!(
        report_source(&carried).status(),
        KernelCheckStatusV1::Incomplete
    );
}

#[test]
fn native_progress_actual_condition_donor_and_bypass_update_refuse() {
    let donor = source("dynamic").replace("gpu.compare (i, n)", "gpu.compare (n, n)");
    assert_eq!(
        report_source(&donor).status(),
        KernelCheckStatusV1::Incomplete
    );
    let bypass = source("dynamic").replace("builtin.function <(builtin.integer ui8)", "builtin.function <(builtin.integer ui8, builtin.integer i1)")
        .replace("^entry(n: builtin.integer ui8):", "^entry(n: builtin.integer ui8, choose: builtin.integer i1):")
        .replace("gpu.branch (next) [^header] []: <(builtin.integer ui8) -> ()>",
            "gpu.cond_branch (choose, next, j) [^header, ^body] [operand_segment_sizes: builtin.operand_segment_sizes [1, 1, 1]]: <(builtin.integer i1, builtin.integer ui8, builtin.integer ui8) -> ()>");
    assert_eq!(
        report_source(&bypass).status(),
        KernelCheckStatusV1::Incomplete
    );
}

#[test]
fn native_progress_arbitrary_seed_and_inside_bound_observe_actual_dominance() {
    let arbitrary = source("dynamic")
        .replace(
            "builtin.function <(builtin.integer ui8)",
            "builtin.function <(builtin.integer ui8, builtin.integer ui8)",
        )
        .replace(
            "^entry(n: builtin.integer ui8):",
            "^entry(n: builtin.integer ui8, initial: builtin.integer ui8):",
        )
        .replace(
            "    initial = gpu.constant <builtin.integer <0: ui8>> : builtin.integer ui8;\n",
            "",
        );
    assert_eq!(
        report_source(&arbitrary).status(),
        KernelCheckStatusV1::Clean
    );
    let bound = "    n = gpu.constant <builtin.integer <254: ui8>> : builtin.integer ui8;\n";
    let inside = source("narrow").replace(bound, "").replace(
        "  ^header(i: builtin.integer ui8):\n",
        &format!("  ^header(i: builtin.integer ui8):\n{bound}"),
    );
    assert_eq!(
        report_source(&inside).status(),
        KernelCheckStatusV1::Incomplete
    );
    let side_entry = arbitrary
        .replace("builtin.function <(builtin.integer ui8, builtin.integer ui8)", "builtin.function <(builtin.integer ui8, builtin.integer ui8, builtin.integer i1)")
        .replace("initial: builtin.integer ui8):", "initial: builtin.integer ui8, choose: builtin.integer i1):")
        .replace("gpu.branch (initial) [^header] []: <(builtin.integer ui8) -> ()>",
            "gpu.cond_branch (choose, initial, initial) [^header, ^body] [operand_segment_sizes: builtin.operand_segment_sizes [1, 1, 1]]: <(builtin.integer i1, builtin.integer ui8, builtin.integer ui8) -> ()>");
    assert_eq!(
        report_source(&side_entry).status(),
        KernelCheckStatusV1::Incomplete
    );
}

#[test]
fn native_progress_unknown_index_width_and_wide_steps_have_explicit_refusals() {
    let index = source("dynamic")
        .replace("builtin.integer <0: ui8>", "gpu.index_value 0")
        .replace("builtin.integer <1: ui8>", "gpu.index_value 1")
        .replace("builtin.integer ui8", "gpu.index");
    assert_eq!(report_source(&index).status(), KernelCheckStatusV1::Clean);
    let nonunit = report_source(&index.replace("gpu.index_value 1", "gpu.index_value 2"));
    assert!(
        matches!(nonunit.findings(), [PlironProgressFindingV1::ProgressIncomplete { reason, .. }]
        if reason.contains("width custody"))
    );
    let wide = source("dynamic")
        .replace("ui8", "ui128")
        .replace("<1: ui128>", "<18446744073709551616: ui128>");
    let report = report_source(&wide);
    assert!(
        matches!(report.findings(), [PlironProgressFindingV1::ProgressIncomplete { reason, .. }]
        if reason.contains("u64 certificate"))
    );
}

#[test]
fn native_progress_verified_narrow_bound_extension_allows_nonunit_loop() {
    let ir = source("dynamic")
        .replace("builtin.integer ui8", "builtin.integer ui16")
        .replace("<0: ui8>", "<0: ui16>").replace("<1: ui8>", "<2: ui16>")
        .replace("builtin.function <(builtin.integer ui16)", "builtin.function <(builtin.integer ui8)")
        .replace("^entry(n: builtin.integer ui16):", "^entry(narrow: builtin.integer ui8):\n    n = gpu.cast (narrow) [] [gpu_cast_kind: gpu.cast_kind ZeroExtend]: <(builtin.integer ui8) -> (builtin.integer ui16)>;");
    assert_eq!(report_source(&ir).status(), KernelCheckStatusV1::Clean);
    let (context, function) =
        parse(&ir.replace("gpu.cast_kind ZeroExtend", "gpu.cast_kind SignExtend"));
    assert!(verify_operation(function.get_operation(), &context).is_err());
}

#[test]
fn native_progress_two_distinct_latches_remain_outside_existing_proof_shape() {
    let ir = source("dynamic")
        .replace("builtin.function <(builtin.integer ui8)", "builtin.function <(builtin.integer ui8, builtin.integer i1)")
        .replace("^entry(n: builtin.integer ui8):", "^entry(n: builtin.integer ui8, choose: builtin.integer i1):")
        .replace("gpu.branch (next) [^header] []: <(builtin.integer ui8) -> ()>",
            "gpu.cond_branch (choose, next, next) [^left, ^right] [operand_segment_sizes: builtin.operand_segment_sizes [1, 1, 1]]: <(builtin.integer i1, builtin.integer ui8, builtin.integer ui8) -> ()>\n  ^left(left_i: builtin.integer ui8):\n    gpu.branch (left_i) [^header] []: <(builtin.integer ui8) -> ()>\n  ^right(right_i: builtin.integer ui8):\n    gpu.branch (right_i) [^header] []: <(builtin.integer ui8) -> ()>");
    assert_eq!(report_source(&ir).status(), KernelCheckStatusV1::Incomplete);
}
