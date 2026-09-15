use super::*;
use fe2o3_pliron::{ProductionSemanticMirLimitsV1, ProductionSemanticSsaLimitsV1};
#[path = "ssa_fixture.rs"]
mod fixture;

#[path = "capture_tests.rs"]
mod capture_tests;

#[path = "context_reborrow_tests.rs"]
mod context_reborrow_tests;
#[path = "transpose_endpoint_tests.rs"]
mod transpose_endpoint_tests;
#[path = "transpose_context_tests.rs"]
mod transpose_context_tests;

fn inputs() -> Vec<ProductionKernelContextLoweringInputV1> {
    // Explicit inert component inputs, not authenticated AMD/launch evidence.
    vec![ProductionKernelContextLoweringInputV1::new(
        SemanticFunctionIdV1::from_index(0),
        [2; 32],
        [3; 32],
        [4; 32],
        [5; 32],
        [6; 32],
    )]
}

fn mir(reborrow: bool, kill: bool) -> ProductionSemanticMirOwnerV1 {
    ProductionSemanticMirOwnerV1::try_new(
        fixture::full_source(reborrow, kill),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn component_actual_ssa_bridge_getter_bind_and_f32_lower_with_reference_moves_and_reborrows() {
    for reborrow in [false, true] {
        let lowered = ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            mir(reborrow, false),
            ProductionSemanticKirLimitsV1::default(),
            inputs(),
        )
        .unwrap();
        lowered.verify_equivalence().unwrap();
        lowered
            .canonical_kernel_ir_v13()
            .unwrap()
            .revalidate()
            .unwrap();
        let operations = lowered
            .module()
            .functions
            .iter()
            .filter_map(|f| f.body.as_ref())
            .flat_map(|b| &b.blocks)
            .flat_map(|b| &b.operations)
            .filter_map(|op| match &op.kind {
                OperationKind::ExecutionCapability(c)
                    if matches!(
                        c.operation,
                        ExecutionCapabilityOperationV1::NumericalPolicyMath(_)
                    ) =>
                {
                    Some((op, c))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(operations.len(), 3);
        let getter = operations
            .iter()
            .find(|(_, c)| {
                matches!(
                    c.operation,
                    ExecutionCapabilityOperationV1::NumericalPolicyMath(
                        NumericalPolicyMathOperationV1::MathDerive { .. }
                    )
                )
            })
            .unwrap();
        let bind = operations
            .iter()
            .find(|(_, c)| {
                matches!(
                    c.operation,
                    ExecutionCapabilityOperationV1::NumericalPolicyMath(
                        NumericalPolicyMathOperationV1::Bind { .. }
                    )
                )
            })
            .unwrap();
        let consumer = operations
            .iter()
            .find(|(_, c)| {
                matches!(
                    c.operation,
                    ExecutionCapabilityOperationV1::NumericalPolicyMath(
                        NumericalPolicyMathOperationV1::F32 { .. }
                    )
                )
            })
            .unwrap();
        assert_eq!(bind.1.operands[0], getter.0.results[0].id);
        assert_eq!(consumer.1.operands[0], bind.0.results[0].id);
        assert!(
            operations
                .iter()
                .all(|(_, c)| c.source.occurrence.is_some())
        );
    }
}

#[test]
fn component_math_owner_storage_death_is_not_hidden_by_bound_reference() {
    let error = ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
        mir(true, true),
        ProductionSemanticKirLimitsV1::default(),
        inputs(),
    )
    .unwrap_err();
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. } if detail.contains("loan") || detail.contains("storage")),
        "{error:?}"
    );
}

#[test]
fn component_policy_owner_death_after_bind_rejects_the_later_math_consumer() {
    for reborrow in [false, true] {
        let live = ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            mir(reborrow, false), ProductionSemanticKirLimitsV1::default(), inputs(),
        ).unwrap();
        live.verify_equivalence().unwrap();
        live.canonical_kernel_ir_v13().unwrap().revalidate().unwrap();

        let dead = ProductionSemanticMirOwnerV1::try_new(fixture::policy_dead_source(reborrow),
            ProductionSemanticMirLimitsV1::default()).unwrap();
        let error = ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            dead, ProductionSemanticKirLimitsV1::default(), inputs(),
        ).unwrap_err();
        assert!(matches!(&error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
            if *detail == "capability loan crosses a move, overwrite, deinitialization or storage death"),
            "{error:?}");
    }
}

#[test]
fn component_math_bridge_never_accepts_different_root_issuance() {
    let mut inputs = inputs();
    inputs[0] = ProductionKernelContextLoweringInputV1::new(
        SemanticFunctionIdV1::from_index(0),
        [2; 32],
        [3; 32],
        [4; 32],
        [5; 32],
        [99; 32],
    );
    assert!(
        ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            mir(false, false),
            ProductionSemanticKirLimitsV1::default(),
            inputs
        )
        .is_err()
    );
}

#[test]
fn bridge_receipt_requires_exact_replayed_use_define_move_kill_order() {
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        mir(false, false),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    let plan = owner
        .execution_plan_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let [result] = plan.defined_math_results() else {
        panic!("one exact bridge result")
    };
    let events = plan
        .plan()
        .resolved_events(SsaBlockIdV1::new(result.block().index()))
        .unwrap();
    let values = math_bridge_events_v1(*result, events).unwrap();
    assert_ne!(values.0, values.1);
    assert_ne!(values.2, values.3);
    let prefix = events.windows(2).position(|w| matches!(w,
        [(_, SsaResolvedEventV1::Use { variable: a, .. }), (_, SsaResolvedEventV1::Use { variable: b, .. })]
        if a.get() == result.receiver().index() && b.get() == result.current().index())).unwrap();
    for index in prefix..prefix + 6 {
        let mut missing = events.to_vec();
        missing.remove(index);
        assert!(math_bridge_events_v1(*result, &missing).is_err());
    }
    let mut wrong = events.to_vec();
    wrong.swap(prefix, prefix + 1);
    assert!(math_bridge_events_v1(*result, &wrong).is_err());
    let mut duplicated = events.to_vec();
    duplicated.extend_from_slice(events);
    assert!(math_bridge_events_v1(*result, &duplicated).is_err());
}
