use super::*;
use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};

fn layout(context: &mut Context, extents: [u64; 3]) -> ExecutionLayoutOp {
    dialect_gpu::register_dialect(context).unwrap();
    ExecutionLayoutOp::new(context, 19, extents, [64, 1, 1], 64)
}

fn quotient(
    context: &mut Context,
    extents: Option<[u64; 3]>,
    declared: u64,
    divisor: u64,
) -> (FuncOp, Value) {
    let function = function(context, "layout_quotient");
    let entry = function.get_entry_block(context);
    if let Some(extents) = extents {
        let execution = layout(context, extents);
        append(context, entry, &execution);
    }
    let invocation = InvocationIndexOp::new(context, 0, declared);
    let divisor = IndexConstantOp::new(context, divisor);
    let quotient = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Divide,
        invocation.result(context),
        divisor.result(context),
    );
    let ret = ReturnOp::new(context);
    for operation in [
        invocation.get_operation(),
        divisor.get_operation(),
        quotient.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    (function, quotient.result(context))
}

#[test]
fn effective_layout_bounds_preserve_original_declarations() {
    for actual in [128, 1024, u64::MAX] {
        for declared in [0, actual] {
            let context = &mut setup();
            let (function, value) = quotient(context, Some([actual, 1, 1]), declared, 64);
            pliron::operation::verify_operation(function.get_operation(), context).unwrap();
            let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
            assert_eq!(analysis.declared_launch_extent(0), Some(declared));
            assert_eq!(analysis.declared_launch_extent(1), None);
            assert_eq!(analysis.launch_extents(), &[actual, 1, 1]);
            let fact = analysis.fact(value);
            assert!(matches!(fact, SparseIndexFactV1::Quotient { .. }));
            assert_eq!(
                fact.maximum(analysis.launch_extents()),
                Some((actual - 1) / 64)
            );
            for point in [0, 63, 64, actual - 1] {
                assert_eq!(fact.evaluate(&[point, 0, 0]), Some(point / 64));
            }
        }
    }
}

#[test]
fn layout_does_not_invent_dynamic_bounds_or_nonzero_divisors() {
    for (extents, divisor) in [(None, 64), (Some([0, 1, 1]), 64), (Some([1024, 1, 1]), 0)] {
        let context = &mut setup();
        let (function, value) = quotient(context, extents, 0, divisor);
        let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
        assert_eq!(analysis.fact(value), SparseIndexFactV1::Unknown);
        assert_eq!(analysis.declared_launch_extent(0), Some(0));
        if divisor != 0 {
            assert_eq!(analysis.launch_extents()[0], 0);
        }
    }
}

#[test]
fn layout_conflicts_and_out_of_domain_axes_are_rejected() {
    for (actual, declared) in [(128, 64), (0, 64)] {
        let context = &mut setup();
        let (function, _) = quotient(context, Some([actual, 1, 1]), declared, 64);
        assert_eq!(
            analyze_pliron_sparse_indices_v1(context, &function).unwrap_err(),
            SparseIndexFailureV1::InvalidExecutionLayout
        );
    }
    for declared in [0, 1] {
        let context = &mut setup();
        let function = function(context, "out_of_layout_axis");
        let entry = function.get_entry_block(context);
        let execution = layout(context, [128, 1, 1]);
        let invocation = InvocationIndexOp::new(context, 3, declared);
        let ret = ReturnOp::new(context);
        append(context, entry, &execution);
        append(context, entry, &invocation);
        append(context, entry, &ret);
        assert_eq!(
            analyze_pliron_sparse_indices_v1(context, &function).unwrap_err(),
            SparseIndexFailureV1::InvalidExecutionLayout
        );
    }
}

#[test]
fn concrete_layout_never_erases_conflicting_invocation_declarations() {
    let context = &mut setup();
    let function = function(context, "conflicting_declarations");
    let entry = function.get_entry_block(context);
    let execution = layout(context, [128, 1, 1]);
    append(context, entry, &execution);
    for extent in [0, 128] {
        let invocation = InvocationIndexOp::new(context, 0, extent);
        append(context, entry, &invocation);
    }
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);
    assert_eq!(
        analyze_pliron_sparse_indices_v1(context, &function).unwrap_err(),
        SparseIndexFailureV1::InconsistentLaunchExtent {
            dimension: 0,
            first: 0,
            second: 128
        }
    );
}

#[test]
fn only_verified_entry_layouts_supply_bounds() {
    for fault in [
        "missing_domain",
        "duplicate",
        "nonentry",
        "zero_workgroup",
        "partial_full_domain",
    ] {
        let context = &mut setup();
        let function = function(context, "malformed_layout");
        let entry = function.get_entry_block(context);
        let execution = layout(context, [128, 1, 1]);
        let body = block(context, &function, "body");
        match fault {
            "missing_domain" => {
                let operation = execution.get_operation();
                let mut raw = operation.deref_mut(context);
                let key = raw
                    .attributes
                    .0
                    .iter()
                    .find_map(|(key, value)| {
                        value
                            .downcast_ref::<ExecutionDomainAttr>()
                            .map(|_| key.clone())
                    })
                    .unwrap();
                assert!(raw.attributes.0.remove(&key).is_some());
            }
            "zero_workgroup" => {
                let invalid = ExecutionLayoutOp::new(context, 19, [128, 1, 1], [0, 1, 1], 64);
                append(context, entry, &invalid);
            }
            "partial_full_domain" => {
                let invalid = ExecutionLayoutOp::new_with_domain(
                    context,
                    19,
                    [127, 1, 1],
                    [64, 1, 1],
                    64,
                    ExecutionDomainAttr::FullPhysicalWorkgroups,
                );
                append(context, entry, &invalid);
            }
            "duplicate" => {
                let duplicate = layout(context, [128, 1, 1]);
                append(context, entry, &duplicate);
            }
            _ => {}
        }
        if !matches!(fault, "zero_workgroup" | "partial_full_domain") {
            append(
                context,
                if fault == "nonentry" { body } else { entry },
                &execution,
            );
        }
        let enter = BranchOp::new(context, body);
        let ret = ReturnOp::new(context);
        append(context, entry, &enter);
        append(context, body, &ret);
        assert_eq!(
            analyze_pliron_sparse_indices_v1(context, &function).unwrap_err(),
            SparseIndexFailureV1::InvalidExecutionLayout,
            "{fault}"
        );
    }
}
