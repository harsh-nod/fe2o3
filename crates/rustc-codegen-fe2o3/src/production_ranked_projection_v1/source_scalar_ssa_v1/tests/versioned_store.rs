use super::*;
use deferred_fixture as f;
use crate::production_reference_effect_join_v2::scalar_guard_v1::source_scalar_test_guard_v1;

#[derive(Clone, Copy)]
enum Mutation {
    None,
    Conflict,
    Escape,
    Cycle,
}

fn run(
    overwrite: bool,
    mutation: Mutation,
) -> (Result<ProductionSemanticExpressionV2, &'static str>, usize) {
    let owner = store_fixture::owner(overwrite);
    let function = owner
        .execution_view_for_root(fixtures::ROOT)
        .unwrap()
        .body();
    let (store, block, statement, load_place) = store_fixture::sites(function);
    let intrinsic = f::empty_intrinsic(function, owner.source_semantic().types());
    let sources = [ProjectedAccessSourceV1 {
        block: 1,
        operation: 0,
        access: AccessKindAttr::Write,
        memory_space: MemorySpaceAttr::Global,
        source: function.blocks()[block].statements()[statement].source(),
        semantic_site: Some(ProjectedSemanticAccessSiteV1 {
            block,
            statement: Some(statement),
        }),
    }];
    let mut resolver =
        GpuSemanticExpressionResolverV2::new(owner.source_semantic().types(), function);
    let load = ProductionSemanticLoadV2 {
        block: 7,
        operation: 0,
        scalar: ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        },
        read_mode: fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedVolatile,
        allocation_origin: 3,
        view: f::ranked(3),
        indices: vec![f::ranked(0)].into_boxed_slice(),
    };
    // Isolate the existing consumer after the private source-bound memory
    // producer. The original typed call and every SSA use still come from owner.
    resolver
        .place_loads
        .insert(load_place as *const _, load.clone());
    resolver.memory_versions.insert(
        (7, 0),
        TypedGlobalMemoryVersionV1::Store {
            semantic_block: store,
        },
    );
    assert_eq!(
        resolver.resolve_store_v2(function.blocks()[block].statements()[statement].kind()),
        Err(AMBIGUOUS)
    );
    match mutation {
        Mutation::None => {}
        Mutation::Conflict => {
            resolver
                .memory_versions
                .insert((7, 0), TypedGlobalMemoryVersionV1::Conflict);
        }
        Mutation::Escape => {
            resolver.address_escaped.insert(2);
        }
        Mutation::Cycle => {
            let SemanticTerminatorKindV1::Call(call) = function.blocks()[store].terminator().kind()
            else {
                panic!("original store")
            };
            let SemanticOperandV1::Copy(place) = &call.arguments()[2] else {
                panic!("original store RHS")
            };
            resolver.place_loads.insert(place as *const _, load);
        }
    }
    let mut deferred = DeferredSourceValuesV1::new(
        resolver,
        owner
            .source_query_for_root(fixtures::ROOT, function)
            .unwrap(),
        &intrinsic,
        owner.source_semantic().callables(),
        &sources,
    );
    let kernel = f::kernel(false);
    let cpu = f::cpu_ir();
    let write = f::write(1);
    let mut guard = source_scalar_test_guard_v1(&kernel, &cpu, &write).unwrap();
    let result = deferred.resolve(&write, &mut guard);
    let state = deferred.resolver.source_ssa.as_ref().unwrap();
    assert!(
        state.active_site.is_none(),
        "outer site restored on success/error"
    );
    assert!(state.visiting.is_empty());
    assert!(
        deferred.resolver.visiting_stores.is_empty(),
        "no failed store traversal leaks"
    );
    (result, deferred.resolver.work)
}

#[test]
fn deferred_scalar_reaching_store_uses_original_rhs_site_then_restores_later_write() {
    let ProductionSemanticExpressionV2::Binary { lhs, rhs, .. } =
        run(false, Mutation::None).0.unwrap()
    else {
        panic!("later write addition")
    };
    let ProductionSemanticExpressionV2::Select {
        when_true,
        when_false,
        ..
    } = *lhs
    else {
        panic!("store captured original merge")
    };
    assert!(matches!(
        *when_true,
        ProductionSemanticExpressionV2::Constant { bits: 9, .. }
    ));
    assert!(matches!(
        *when_false,
        ProductionSemanticExpressionV2::Constant { bits: 7, .. }
    ));
    // This sibling is at the later write, after the same local was overwritten.
    assert!(matches!(
        *rhs,
        ProductionSemanticExpressionV2::Constant { bits: 29, .. }
    ));
}

#[test]
fn deferred_scalar_reaching_store_rejects_stale_pre_overwrite_merge() {
    let ProductionSemanticExpressionV2::Binary { lhs, rhs, .. } =
        run(true, Mutation::None).0.unwrap()
    else {
        panic!("later write addition")
    };
    assert!(matches!(
        *lhs,
        ProductionSemanticExpressionV2::Constant { bits: 13, .. }
    ));
    assert!(matches!(
        *rhs,
        ProductionSemanticExpressionV2::Constant { bits: 29, .. }
    ));
}

#[test]
fn deferred_scalar_reaching_store_conflict_escape_and_cycle_remain_errors() {
    assert_eq!(
        run(false, Mutation::Conflict).0,
        Err("mutable global load has conflicting reaching writes")
    );
    assert_eq!(run(false, Mutation::Escape).0, Err(CUSTODY));
    assert_eq!(
        run(false, Mutation::Cycle).0,
        Err("mutable global store value has a cyclic memory dependency")
    );
}
