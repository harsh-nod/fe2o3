//! Single-function ownership controls; no full source/SSA memory measurement.
use super::super::{
    ProductionSemanticSsaLimitsV1, plan_semantic_function_ssa_with_module_v1,
    tests::{test_function, test_types},
};
use super::*;
use fe2o3_kernel_ir::LogicalStorageLimitsV1 as Limits;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{SsaBlockIdV1, SsaVariableIdV1};
use std::mem::size_of_val;

fn counter(bytes: Option<usize>, items: usize) -> Counter {
    Counter::new(Limits {
        max_bytes: bytes,
        max_items: items,
    })
}
fn place(local: u32, field: Option<u32>) -> SemanticPlaceV1 {
    let ty = SemanticTypeIdV1::from_index(1);
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        field
            .map(|field| {
                vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap()]
            })
            .unwrap_or_default(),
        ty,
    )
    .unwrap()
}
fn assign(destination: u32, source: SemanticPlaceV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(destination, None),
            SemanticRvalueV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source)),
            ),
        )),
    )
}
fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(source, kind),
    )
    .unwrap()
}
fn plan() -> ProductionSemanticSsaFunctionPlanV1 {
    let function = test_function(vec![
        block(
            120,
            vec![assign(2, place(1, Some(0)))],
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(1),
            )),
        ),
        block(
            121,
            vec![assign(3, place(2, None))],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &function,
        &test_types(false),
        &[],
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}
fn observe(plan: &ProductionSemanticSsaFunctionPlanV1) -> (usize, usize) {
    let mut c = counter(None, 100_000);
    plan.charge_retained_storage_v1(&mut c).unwrap();
    (c.bytes(), c.items())
}

#[test]
fn actual_function_plan_composes_its_original_planner_and_both_boxes_once() {
    let plan = plan();
    assert!(plan.plan().is_reachable(SsaBlockIdV1::new(1)));
    let before = plan.clone();
    let mut nested = counter(None, 100_000);
    plan.plan()
        .visit_logical_retained_heap_v1(&mut |n, w| charge_extent(&mut nested, n, w))
        .unwrap();
    let boxes = (plan.implicit_entry_variables.len() + plan.retained_cross_edge_variables.len())
        * size_of::<SsaVariableIdV1>();
    assert_eq!(
        observe(&plan),
        (
            size_of::<ProductionSemanticSsaFunctionPlanV1>() + nested.bytes() + boxes,
            1 + nested.items() + 2,
        )
    );
    assert_eq!(plan, before);
}

#[test]
fn actual_box_payload_lengths_are_counted_without_extra_box_headers() {
    let mut plan = plan();
    let before = observe(&plan);
    let old = (plan.implicit_entry_variables.len() + plan.retained_cross_edge_variables.len())
        * size_of::<SsaVariableIdV1>();
    // Private storage-only fixture mutation; no claim of semantic replay/admission.
    plan.implicit_entry_variables = vec![SsaVariableIdV1::new(0); 3].into_boxed_slice();
    plan.retained_cross_edge_variables = vec![SsaVariableIdV1::new(1); 5].into_boxed_slice();
    let changed = observe(&plan);
    assert_eq!(changed.0, before.0 - old + 8 * size_of::<SsaVariableIdV1>());
    assert_eq!(changed.1, before.1);
    let immutable = plan.clone();
    assert_eq!(observe(&plan), changed);
    assert_eq!(plan, immutable);
}

#[test]
fn enclosing_header_and_heap_only_path_do_not_double_count_the_plan_wrapper() {
    struct Enclosing {
        _tag: [u8; 19],
        plan: ProductionSemanticSsaFunctionPlanV1,
    }
    let plan = plan();
    let standalone = observe(&plan);
    let owner = Enclosing {
        _tag: [0; 19],
        plan,
    };
    let expected =
        size_of::<Enclosing>() + standalone.0 - size_of::<ProductionSemanticSsaFunctionPlanV1>();
    let mut c = counter(Some(expected), standalone.1);
    c.charge(size_of::<Enclosing>(), 1).unwrap();
    owner.plan.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (expected, standalone.1));
}

#[test]
fn function_plan_exact_and_short_limits_preserve_refusal_as_incomplete() {
    let plan = plan();
    let (bytes, items) = observe(&plan);
    let mut exact = counter(Some(bytes), items);
    plan.charge_retained_storage_v1(&mut exact).unwrap();
    assert_eq!((exact.bytes(), exact.items()), (bytes, items));
    for (limit, error) in [
        (
            Limits {
                max_bytes: Some(bytes - 1),
                max_items: items,
            },
            Error::ByteLimit,
        ),
        (
            Limits {
                max_bytes: None,
                max_items: items - 1,
            },
            Error::ItemLimit,
        ),
    ] {
        let mut c = Counter::new(limit);
        assert_eq!(plan.charge_retained_storage_v1(&mut c), Err(error));
        assert_ne!((c.bytes(), c.items()), (bytes, items));
    }
    let mut zero = counter(None, 0);
    assert_eq!(
        plan.charge_retained_heap_storage_v1(&mut zero),
        Err(Error::ItemLimit)
    );
    assert_eq!((zero.bytes(), zero.items()), (0, 0));
}

#[test]
fn checked_extent_and_existing_counter_overflow_fail_without_a_success_total() {
    let mut c = counter(None, usize::MAX);
    assert_eq!(charge_extent(&mut c, usize::MAX, 2), Err(Error::Arithmetic));
    assert_eq!((c.bytes(), c.items()), (0, 0));
    c.charge(usize::MAX, 0).unwrap();
    assert_eq!(
        plan().charge_retained_storage_v1(&mut c),
        Err(Error::Arithmetic)
    );
    assert_eq!((c.bytes(), c.items()), (usize::MAX, 0));
    let mut items = counter(None, usize::MAX);
    items.charge(0, usize::MAX).unwrap();
    assert_eq!(
        plan().charge_retained_heap_storage_v1(&mut items),
        Err(Error::Arithmetic)
    );
    assert_eq!((items.bytes(), items.items()), (0, usize::MAX));
}

#[test]
fn independently_retained_equal_plans_keep_distinct_allocations() {
    let first = plan();
    let second = first.clone();
    assert_eq!(first, second);
    let first_bytes = observe(&first);
    let second_bytes = observe(&second);
    let owners = (first, second);
    let mut c = counter(None, 100_000);
    c.charge(size_of_val(&owners), 1).unwrap();
    owners.0.charge_retained_heap_storage_v1(&mut c).unwrap();
    owners.1.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!(
        c.bytes(),
        size_of_val(&owners) + first_bytes.0 + second_bytes.0
            - 2 * size_of::<ProductionSemanticSsaFunctionPlanV1>()
    );
    assert_eq!(c.items(), first_bytes.1 + second_bytes.1 - 1);
}
