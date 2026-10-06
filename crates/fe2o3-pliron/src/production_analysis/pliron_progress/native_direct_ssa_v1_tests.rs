use super::native_loop_v1_tests::{blocks, parse};
use super::*;

const DIRECT: &str = include_str!("../tests/lit/native_progress_direct_header_ssa.pliron");
const NESTED: &str = include_str!("../tests/lit/native_progress_direct_nested_ssa.pliron");
const HEADER_BRANCH: &str = "gpu.cond_branch (test, i) [^body, ^exit] [operand_segment_sizes: builtin.operand_segment_sizes [1, 0, 1]]: <(builtin.integer i1, builtin.integer ui8) -> ()>";
const BACKEDGE: &str = "gpu.branch (next) [^header] []: <(builtin.integer ui8) -> ()>";

fn replace_once(source: &str, from: &str, to: &str) -> String {
    assert_eq!(source.matches(from).count(), 1, "{from}");
    source.replacen(from, to, 1)
}

fn report(source: &str) -> PlironProgressReportV1 {
    let (context, function) = parse(source);
    verify_operation(function.get_operation(), &context).unwrap();
    run_pliron_progress_check_v1(&context, &function)
}

fn with_choose(source: &str) -> String {
    let source = replace_once(
        source,
        "builtin.function <(builtin.integer ui8)",
        "builtin.function <(builtin.integer ui8, builtin.integer i1)",
    );
    replace_once(
        &source,
        "^entry(n: builtin.integer ui8):",
        "^entry(n: builtin.integer ui8, choose: builtin.integer i1):",
    )
}

fn literal_bound(source: &str, value: u8) -> String {
    let source = replace_once(
        source,
        "builtin.function <(builtin.integer ui8)",
        "builtin.function <()",
    );
    replace_once(
        &source,
        "^entry(n: builtin.integer ui8):",
        &format!(
            "^entry():\n    n = gpu.constant <builtin.integer <{value}: ui8>> : builtin.integer ui8;"
        ),
    )
}

fn assert_incomplete(source: &str) {
    let progress = report(source);
    assert_eq!(
        progress.status(),
        KernelCheckStatusV1::Incomplete,
        "{:?}",
        progress.findings()
    );
    assert!(progress.certificates().is_empty());
}

#[test]
fn native_direct_ssa_zero_argument_bodies_reach_actual_nine_without_graph_edits() {
    for (source, arguments, certificate_headers) in [
        (DIRECT, vec![1, 1, 0, 1], vec![1]),
        (NESTED, vec![1, 1, 1, 0, 0, 1], vec![1, 2]),
    ] {
        let (context, function) = parse(source);
        verify_operation(function.get_operation(), &context).unwrap();
        let before = function.disp(&context).to_string();
        let pointer = function.get_operation();
        let original_blocks = blocks(&context, &function);
        assert_eq!(
            original_blocks
                .iter()
                .map(|b| b.deref(&context).get_num_arguments())
                .collect::<Vec<_>>(),
            arguments
        );
        let outcome =
            crate::require_production_pliron_checks_before_lowering_v2(&context, &function)
                .unwrap();
        assert_eq!(
            outcome.pass_order(),
            &crate::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
        );
        let progress = outcome.semantics().progress();
        assert_eq!(progress.status(), KernelCheckStatusV1::Clean);
        assert_eq!(
            progress
                .certificates()
                .iter()
                .map(|c| c.header())
                .collect::<Vec<_>>(),
            certificate_headers
        );
        assert!(progress.certificates().iter().all(|c| c.step() == 1));
        assert!(!progress.grants_launch_or_liveness_authority());
        assert_eq!(function.get_operation(), pointer);
        assert_eq!(blocks(&context, &function), original_blocks);
        assert_eq!(function.disp(&context).to_string(), before);
    }
}

#[test]
fn native_direct_ssa_uses_existing_nested_fallback_and_real_header_identity() {
    let (context, function) = parse(DIRECT);
    verify_operation(function.get_operation(), &context).unwrap();
    let inventory = bounded_structural_inventory(&context, &function).unwrap();
    let blocks = &inventory.root_blocks;
    let indices = blocks.iter().enumerate().map(|(i, b)| (*b, i)).collect();
    let graph = build_root_graph(&context, blocks, &indices).unwrap();
    let dominators = progress_dominators_v1(&graph.edges, &graph.predecessors);
    let component = vec![1, 2];
    let members = component.iter().copied().collect();
    assert!(dominators[2].contains(&1));
    assert!(matches!(
        canonical_positive_induction_loop(
            &context,
            blocks,
            &indices,
            &inventory.root_operation_blocks,
            &dominators,
            &graph.predecessors,
            &graph.edges,
            &graph.incoming,
            &component,
            &members,
        ),
        CanonicalLoopResultV1::Incomplete(_)
    ));
    let actual = blocks[1].deref(&context).get_argument(0);
    let propagated =
        propagate_loop_induction_v1(&context, blocks, &graph.edges, &members, 1, actual).unwrap();
    assert_eq!(propagated[&2], actual);
    let progress = run_pliron_progress_check_v1(&context, &function);
    assert_eq!(progress.status(), KernelCheckStatusV1::Clean);
    assert_eq!(
        progress.certificates()[0].induction(),
        actual.id(&context).to_string()
    );
}

#[test]
fn native_direct_ssa_all_finite_widths_signed_seeds_and_guard_polarities() {
    for signed in [false, true] {
        for width in [8, 16, 32, 64, 128] {
            let ty = format!("{}i{width}", if signed { "s" } else { "u" });
            let mut source = DIRECT.replace("ui8", &ty);
            if signed {
                source = source.replace(&format!("<3: {ty}>"), &format!("<-3: {ty}>"));
            }
            for reversed in [false, true] {
                let source = if reversed {
                    source
                        .replace(
                            "gpu.compare_predicate LessThan",
                            "gpu.compare_predicate GreaterThanOrEqual",
                        )
                        .replace("(test, i) [^body, ^exit]", "(test, i) [^exit, ^body]")
                        .replace(
                            "builtin.operand_segment_sizes [1, 0, 1]",
                            "builtin.operand_segment_sizes [1, 1, 0]",
                        )
                } else {
                    source.clone()
                };
                let progress = report(&source);
                assert_eq!(
                    progress.status(),
                    KernelCheckStatusV1::Clean,
                    "{ty} {reversed}"
                );
                assert_eq!(progress.certificates().len(), 1);
                assert_eq!(progress.certificates()[0].step(), 1);
            }
        }
    }
}

#[test]
fn native_direct_ssa_arbitrary_seed_and_narrow_no_wrap_remain_typed() {
    let source = replace_once(
        DIRECT,
        "builtin.function <(builtin.integer ui8)",
        "builtin.function <(builtin.integer ui8, builtin.integer ui8)",
    );
    let source = replace_once(
        &source,
        "^entry(n: builtin.integer ui8):",
        "^entry(n: builtin.integer ui8, initial: builtin.integer ui8):",
    );
    let source = replace_once(
        &source,
        "    initial = gpu.constant <builtin.integer <3: ui8>> : builtin.integer ui8;\n",
        "",
    );
    assert_eq!(report(&source).status(), KernelCheckStatusV1::Clean);
    let safe = literal_bound(DIRECT, 254).replace("<1: ui8>", "<2: ui8>");
    assert_eq!(report(&safe).status(), KernelCheckStatusV1::Clean);
    assert_incomplete(&safe.replace("<254: ui8>", "<255: ui8>"));
    assert_incomplete(&DIRECT.replace("<1: ui8>", "<2: ui8>"));
    let signed = safe
        .replace("ui8", "si8")
        .replace("<254: si8>", "<126: si8>");
    assert_eq!(report(&signed).status(), KernelCheckStatusV1::Clean);
    assert_incomplete(&signed.replace("<126: si8>", "<127: si8>"));
}

#[test]
fn native_direct_ssa_decoy_and_mutated_updates_cannot_inherit_header_progress() {
    for (from, to) in [
        ("gpu.binary (i, step)", "gpu.binary (n, step)"),
        ("gpu.binary (i, step)", "gpu.binary (initial, step)"),
        ("gpu.binary (i, step)", "gpu.binary (i, n)"),
        ("gpu.binary_kind Add", "gpu.binary_kind Subtract"),
        ("gpu.compare (i, n)", "gpu.compare (n, n)"),
    ] {
        assert_incomplete(&replace_once(DIRECT, from, to));
    }
    let nested = replace_once(NESTED, "gpu.binary (j, step)", "gpu.binary (i, step)");
    assert_incomplete(&nested);
}

#[test]
fn native_direct_ssa_unrelated_parallel_payloads_are_compared_in_full() {
    let source = with_choose(DIRECT);
    let source = replace_once(
        &source,
        HEADER_BRANCH,
        "gpu.cond_branch (test, i) [^dispatch, ^exit] [operand_segment_sizes: builtin.operand_segment_sizes [1, 0, 1]]: <(builtin.integer i1, builtin.integer ui8) -> ()>",
    );
    let source = replace_once(
        &source,
        "  ^body():",
        "  ^dispatch():\n    gpu.cond_branch (choose, initial, initial) [^body, ^body] [operand_segment_sizes: builtin.operand_segment_sizes [1, 1, 1]]: <(builtin.integer i1, builtin.integer ui8, builtin.integer ui8) -> ()>\n  ^body(decoy: builtin.integer ui8):",
    );
    assert_eq!(report(&source).status(), KernelCheckStatusV1::Clean);
    let conflict = replace_once(
        &source,
        "gpu.cond_branch (choose, initial, initial)",
        "gpu.cond_branch (choose, initial, step)",
    );
    assert_incomplete(&conflict);
    let used_decoy = replace_once(&source, "gpu.binary (i, step)", "gpu.binary (decoy, step)");
    assert_incomplete(&used_decoy);
}

#[test]
fn native_direct_ssa_ambiguous_forwarding_is_not_a_direct_value_fallback() {
    let source = replace_once(
        DIRECT,
        HEADER_BRANCH,
        "gpu.cond_branch (test, i, i, i) [^body, ^exit] [operand_segment_sizes: builtin.operand_segment_sizes [1, 2, 1]]: <(builtin.integer i1, builtin.integer ui8, builtin.integer ui8, builtin.integer ui8) -> ()>",
    );
    let source = replace_once(
        &source,
        "^body():",
        "^body(first: builtin.integer ui8, second: builtin.integer ui8):",
    );
    assert_incomplete(&source);
}

#[test]
fn native_direct_ssa_bypass_and_side_entry_do_not_gain_a_certificate() {
    let source = with_choose(DIRECT);
    let bypass = replace_once(
        &source,
        BACKEDGE,
        "gpu.cond_branch (choose, next) [^header, ^body] [operand_segment_sizes: builtin.operand_segment_sizes [1, 1, 0]]: <(builtin.integer i1, builtin.integer ui8) -> ()>",
    );
    assert_incomplete(&bypass);
    let side_entry = replace_once(
        &source,
        "gpu.branch (initial) [^header] []: <(builtin.integer ui8) -> ()>",
        "gpu.cond_branch (choose, initial) [^header, ^body] [operand_segment_sizes: builtin.operand_segment_sizes [1, 1, 0]]: <(builtin.integer i1, builtin.integer ui8) -> ()>",
    );
    // This first form is invalid SSA: the header definition no longer dominates.
    let (context, function) = parse(&side_entry);
    assert!(verify_operation(function.get_operation(), &context).is_err());
    assert_eq!(
        run_pliron_progress_check_v1(&context, &function).status(),
        KernelCheckStatusV1::Rejected
    );
    // A structurally valid decoy recurrence still cannot bypass header custody.
    assert_incomplete(&replace_once(
        &side_entry,
        "gpu.binary (i, step)",
        "gpu.binary (initial, step)",
    ));
}

#[test]
fn native_direct_ssa_zero_step_needs_actual_entry_and_no_early_exit() {
    let zero = literal_bound(DIRECT, 10).replace("<1: ui8>", "<0: ui8>");
    let rejected = report(&zero);
    assert_eq!(rejected.status(), KernelCheckStatusV1::Rejected);
    assert!(rejected.certificates().is_empty());
    let inactive = zero.replace("<3: ui8>", "<10: ui8>");
    let inactive = report(&inactive);
    assert_eq!(inactive.status(), KernelCheckStatusV1::Clean);
    assert!(inactive.certificates().is_empty());
    assert_incomplete(&DIRECT.replace("<1: ui8>", "<0: ui8>"));
    let source = literal_bound(DIRECT, 10);
    let source = replace_once(
        &source,
        "builtin.function <()",
        "builtin.function <(builtin.integer i1)",
    );
    let source = replace_once(&source, "^entry():", "^entry(choose: builtin.integer i1):");
    let early = replace_once(
        &source,
        BACKEDGE,
        "gpu.cond_branch (choose, next, next) [^header, ^exit] [operand_segment_sizes: builtin.operand_segment_sizes [1, 1, 1]]: <(builtin.integer i1, builtin.integer ui8, builtin.integer ui8) -> ()>",
    );
    assert_eq!(report(&early).status(), KernelCheckStatusV1::Clean);
    assert_incomplete(&early.replace("<1: ui8>", "<0: ui8>"));
}
