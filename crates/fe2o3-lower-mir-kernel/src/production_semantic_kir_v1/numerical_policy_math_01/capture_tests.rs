use super::*;

fn lower_capture(
    reborrow: bool,
    kill: bool,
    forged: bool,
    moved: bool,
) -> Result<ProductionSemanticKirOwnerV1, ProductionSemanticKirErrorV1> {
    // Synthetic component receipt only; actual AMD callback remains a separate gate.
    let mir = ProductionSemanticMirOwnerV1::try_new(
        fixture::captured_source(reborrow, kill, forged, moved),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
        mir,
        ProductionSemanticKirLimitsV1::default(),
        inputs(),
    )
}

#[test]
fn captured_math_reference_crosses_ssa_carrier_move_without_reissuing_authority() {
    for reborrow in [false, true] {
        let lowered = lower_capture(reborrow, false, false, false).unwrap();
        lowered.verify_equivalence().unwrap();
        lowered
            .canonical_kernel_ir_v13()
            .unwrap()
            .revalidate()
            .unwrap();
        let mut bound = None;
        let mut used = None;
        let mut count = 0;
        for op in lowered
            .module()
            .functions
            .iter()
            .filter_map(|f| f.body.as_ref())
            .flat_map(|b| &b.blocks)
            .flat_map(|b| &b.operations)
        {
            let OperationKind::ExecutionCapability(c) = &op.kind else {
                continue;
            };
            if let ExecutionCapabilityOperationV1::NumericalPolicyMath(operation) = &c.operation {
                count += 1;
                assert!(c.source.occurrence.is_some());
                match operation {
                    NumericalPolicyMathOperationV1::Bind { .. } => bound = Some(op.results[0].id),
                    NumericalPolicyMathOperationV1::F32 { .. } => used = Some(c.operands[0]),
                    _ => {}
                }
            }
        }
        assert_eq!(count, 3);
        assert!(bound.is_some());
        assert_eq!(bound, used);
    }
}

#[test]
fn captured_math_reference_cannot_hide_original_math_storage_death() {
    let error = lower_capture(false, true, false, false).unwrap_err();
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
        if detail.contains("loan") || detail.contains("storage")),
        "{error:?}"
    );
}

#[test]
fn captured_math_reference_requires_original_checked_bind_not_same_typed_aggregate() {
    let error = lower_capture(false, false, true, false).unwrap_err();
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
        if detail == "policy Math authority has no authenticated original producer"),
        "{error:?}"
    );
}

#[test]
fn captured_math_projected_move_is_not_silently_lowered_as_a_copy() {
    let error = lower_capture(false, false, false, true).unwrap_err();
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
        if detail == "Math capture projected moves require exact partial-move transport"),
        "{error:?}"
    );
}
