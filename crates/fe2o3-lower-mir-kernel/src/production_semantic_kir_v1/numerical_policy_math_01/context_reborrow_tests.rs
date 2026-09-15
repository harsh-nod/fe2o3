use super::*;

fn lower(
    nested: bool,
    kill: bool,
    forged: bool,
) -> Result<ProductionSemanticKirOwnerV1, ProductionSemanticKirErrorV1> {
    let mir = ProductionSemanticMirOwnerV1::try_new(
        fixture::context_reborrow_source(nested, kill, forged),
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
fn context_exclusive_transfer_and_shared_reborrow_preserve_captured_math_issuers() {
    for nested in [false, true] {
        let lowered = lower(nested, false, false).unwrap();
        lowered.verify_equivalence().unwrap();
        lowered
            .canonical_kernel_ir_v13()
            .unwrap()
            .revalidate()
            .unwrap();
        let mut getter = None;
        let mut bind = None;
        let mut consumer = None;
        let mut count = 0;
        let mut workgroups = 0;
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
            if matches!(c.operation, ExecutionCapabilityOperationV1::WorkgroupDerive { .. }) {
                workgroups += 1;
                assert!(c.source.occurrence.is_some());
            }
            let ExecutionCapabilityOperationV1::NumericalPolicyMath(operation) = &c.operation
            else {
                continue;
            };
            count += 1;
            assert!(c.source.occurrence.is_some());
            match operation {
                NumericalPolicyMathOperationV1::MathDerive { .. } => {
                    getter = Some(op.results[0].id)
                }
                NumericalPolicyMathOperationV1::Bind { .. } => {
                    bind = Some((c.operands[0], op.results[0].id))
                }
                NumericalPolicyMathOperationV1::F32 { .. } => consumer = Some(c.operands[0]),
            }
        }
        assert_eq!(count, 3);
        assert_eq!(workgroups, 1, "the mutable Context consumer must execute");
        let (math, bound) = bind.unwrap();
        assert_eq!(getter, Some(math));
        assert_eq!(consumer, Some(bound));
    }
}

#[test]
fn context_reborrow_does_not_hide_original_math_storage_death() {
    let error = lower(true, true, false).unwrap_err();
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
        if detail == "capability loan crosses a move, overwrite, deinitialization or storage death"),
        "{error:?}"
    );
}

#[test]
fn context_reborrow_does_not_authenticate_a_same_typed_forged_bind() {
    let error = lower(true, false, true).unwrap_err();
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
        if detail == "policy Math authority has no authenticated original producer"),
        "{error:?}"
    );
}
