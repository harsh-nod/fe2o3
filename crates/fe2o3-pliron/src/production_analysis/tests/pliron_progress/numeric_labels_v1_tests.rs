use super::*;
use pliron::{common_traits::Named, linked_list::ContainsLinkedList, operation::Operation};

const LONG_ALIAS_BYTES: usize = 8_192;

fn function_values(context: &Context, function: &FuncOp) -> Vec<Value> {
    let mut values = Vec::new();
    for block in function.get_region(context).deref(context).iter(context) {
        values.extend(block.deref(context).arguments());
        for operation in block.deref(context).iter(context) {
            values.extend(operation.deref(context).results());
        }
    }
    values
}

fn name_values(context: &Context, values: &[Value], alias: &str) {
    for value in values {
        value.set_name(context, Some(alias.try_into().unwrap()));
    }
}

fn assert_exact_numeric_labels(
    context: &Context,
    function: &FuncOp,
    report: &PlironProgressReportV1,
) {
    let blocks = function
        .get_region(context)
        .deref(context)
        .iter(context)
        .collect::<Vec<_>>();
    for certificate in report.certificates() {
        let header = blocks[certificate.header()];
        let terminator = header.deref(context).get_terminator(context).unwrap();
        let operation = Operation::get_op_dyn(terminator, context);
        let branch = operation
            .downcast_ref::<IndexLessThanBranchArgsOp>()
            .unwrap();
        for (label, value) in [
            (certificate.induction(), branch.lhs(context)),
            (certificate.bound(), branch.rhs(context)),
        ] {
            let expected: String = value.id(context).into();
            assert_eq!(label, expected);
            assert!((2..=21).contains(&label.len()), "{label}");
            let digits = label.strip_prefix('v').expect("numeric value label");
            assert!(digits.bytes().all(|byte| byte.is_ascii_digit()));
            assert!(digits.parse::<u64>().is_ok());
        }
    }
}

fn assert_certificate_alias_invariance(nested: bool) {
    let context = &mut setup();
    let function = if nested {
        nested_loop(context, NestedLoopCase::Canonical)
    } else {
        constant_loop(context, 0, 7, 1)
    };
    let values = function_values(context, &function);
    assert!(!values.is_empty());
    name_values(context, &values, "initial_alias");
    verify_operation(function.get_operation(), context).unwrap();
    let expected = run_pliron_progress_check_v1(context, &function);
    assert!(expected.is_clean(), "{expected:?}");
    assert_eq!(expected.certificates().len(), if nested { 2 } else { 1 });

    for alias in ["renamed_alias".to_owned(), "d".repeat(LONG_ALIAS_BYTES)] {
        name_values(context, &values, &alias);
        verify_operation(function.get_operation(), context).unwrap();
        let observed = run_pliron_progress_check_v1(context, &function);
        assert_eq!(
            observed,
            expected,
            "nested={nested}, alias bytes={}",
            alias.len()
        );
        assert_exact_numeric_labels(context, &function, &observed);
    }
}

#[test]
fn canonical_progress_certificate_ignores_short_and_long_debug_aliases() {
    assert_certificate_alias_invariance(false);
}

#[test]
fn nested_progress_certificates_ignore_short_and_long_debug_aliases() {
    assert_certificate_alias_invariance(true);
}

#[test]
fn numeric_certificate_labels_match_values_and_fit_the_u64_decimal_maximum() {
    // Value IDs are u64 in the pinned dependency; this is a length oracle,
    // not a claim that this fixture allocates the maximum possible value ID.
    let maximum_label = format!("v{}", u64::MAX);
    assert_eq!(maximum_label, "v18446744073709551615");
    assert_eq!(maximum_label.len(), 21);

    let context = &mut setup();
    let function = constant_loop(context, 0, 7, 1);
    let values = function_values(context, &function);
    name_values(context, &values, &"n".repeat(LONG_ALIAS_BYTES));
    verify_operation(function.get_operation(), context).unwrap();
    let report = run_pliron_progress_check_v1(context, &function);
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(report.certificates().len(), 1);
    assert_exact_numeric_labels(context, &function, &report);
}

#[test]
fn unsuccessful_progress_proofs_keep_exact_findings_with_long_debug_aliases() {
    for case in 0..5 {
        let context = &mut setup();
        let function = match case {
            0 => constant_loop(context, 0, 7, 0),
            1 => constant_loop(context, 0, u64::MAX, 16),
            2 => nested_loop(context, NestedLoopCase::ZeroInnerStep),
            3 => nested_loop(context, NestedLoopCase::NonzeroInnerStart),
            4 => nested_loop(context, NestedLoopCase::LoopLocalInnerBound),
            _ => unreachable!(),
        };
        let values = function_values(context, &function);
        name_values(context, &values, "initial_alias");
        verify_operation(function.get_operation(), context).unwrap();
        let expected = run_pliron_progress_check_v1(context, &function);
        assert!(!expected.is_clean(), "case {case}: {expected:?}");
        assert!(!expected.findings().is_empty());
        assert!(expected.certificates().is_empty());

        name_values(context, &values, &"r".repeat(LONG_ALIAS_BYTES));
        verify_operation(function.get_operation(), context).unwrap();
        let observed = run_pliron_progress_check_v1(context, &function);
        assert_eq!(observed, expected, "case {case}");
    }
}
