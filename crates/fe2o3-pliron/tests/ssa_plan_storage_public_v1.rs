//! External consumer checks for data-only SSA capacity composition.
use fe2o3_kernel_ir::{
    LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error,
    LogicalStorageLimitsV1 as Limits,
};
use fe2o3_mir_model::{SsaBlockIdV1, SsaBlockInputV1, SsaConstructionInputV1, plan_ssa_v1};
use fe2o3_pliron::ProductionSemanticSsaFunctionPlanV1;

#[test]
fn both_function_plan_charge_methods_are_publicly_nameable() {
    let _: fn(&ProductionSemanticSsaFunctionPlanV1, &mut Counter) -> Result<(), Error> =
        ProductionSemanticSsaFunctionPlanV1::charge_retained_storage_v1;
    let _: fn(&ProductionSemanticSsaFunctionPlanV1, &mut Counter) -> Result<(), Error> =
        ProductionSemanticSsaFunctionPlanV1::charge_retained_heap_storage_v1;
}

#[test]
fn independent_consumer_can_bound_the_original_mir_model_plan_visitor() {
    let input = SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        0,
        vec![],
        vec![],
        vec![SsaBlockInputV1::new(vec![], vec![])],
    );
    let plan = plan_ssa_v1(&input).unwrap();
    let identity = plan.identity();
    let mut c = Counter::new(Limits {
        max_bytes: Some(1_048_576),
        max_items: 100,
    });
    plan.visit_logical_retained_heap_v1(&mut |count, width| {
        c.charge(count.checked_mul(width).ok_or(Error::Arithmetic)?, 1)
    })
    .unwrap();
    assert!(c.bytes() > 0);
    assert!(c.items() >= 12);
    assert_eq!(plan.identity(), identity);
}
