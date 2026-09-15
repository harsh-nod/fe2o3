use super::*;
use deferred_fixture as f;
use crate::production_reference_effect_join_v2::scalar_guard_v1::source_scalar_test_guard_v1;

const UNPROVED: &str =
    "scalar SSA selector lacks an exact Boolean or proved typed-global condition";

fn cleared(deferred: &DeferredSourceValuesV1<'_>) {
    let state = deferred.resolver.source_ssa.as_ref().unwrap();
    assert!(state.active_site.is_none());
    assert!(state.visiting.is_empty());
}

#[test]
fn deferred_scalar_requires_gpu_comparison_even_with_matching_cpu_assertion() {
    for gpu_comparison in [true, false] {
        let owner = f::discriminant_owner();
        let function = owner
            .execution_view_for_root(fixtures::ROOT)
            .unwrap()
            .body();
        let intrinsic = f::fact_intrinsic(&owner, false);
        let sources = [f::source(&owner, 1)];
        let mut resolver =
            GpuSemanticExpressionResolverV2::new(owner.source_semantic().types(), function);
        let source = sources[0].semantic_site.unwrap();
        assert_eq!(
            resolver.resolve_store_v2(
                function.blocks()[source.block].statements()[source.statement.unwrap()].kind()
            ),
            Err(AMBIGUOUS)
        );
        let mut deferred = DeferredSourceValuesV1::new(
            resolver,
            owner
                .source_query_for_root(fixtures::ROOT, function)
                .unwrap(),
            &intrinsic,
            owner.source_semantic().callables(),
            &sources,
        );
        let kernel = f::kernel(gpu_comparison);
        let cpu = f::cpu_ir();
        assert!(matches!(
            cpu.blocks[0].terminator,
            crate::reference_effect_v1::ReferenceTerminatorV1::Assert { expected: true, .. }
        ));
        let write = f::write(1);
        let mut guard = source_scalar_test_guard_v1(&kernel, &cpu, &write).unwrap();
        let result = deferred.resolve(&write, &mut guard);
        if gpu_comparison {
            let ProductionSemanticExpressionV2::Select {
                condition,
                when_true,
                when_false,
                ..
            } = result.unwrap()
            else {
                panic!("retained two-way selection")
            };
            assert!(matches!(
                *condition,
                ProductionSemanticExpressionV2::Constant {
                    scalar: ProductionSemanticScalarTypeV2::Bool,
                    bits: 1
                }
            ));
            assert!(matches!(
                *when_true,
                ProductionSemanticExpressionV2::Constant { bits: 9, .. }
            ));
            assert!(matches!(
                *when_false,
                ProductionSemanticExpressionV2::Constant { bits: 7, .. }
            ));
        } else {
            assert_eq!(result, Err(UNPROVED));
            assert!(
                deferred
                    .resolver
                    .source_ssa
                    .as_ref()
                    .unwrap()
                    .proved_some
                    .is_empty()
            );
        }
        cleared(&deferred);
    }
}

#[test]
fn deferred_scalar_rejects_changed_extent_despite_cpu_and_other_gpu_bound() {
    let owner = f::discriminant_owner();
    let function = owner
        .execution_view_for_root(fixtures::ROOT)
        .unwrap()
        .body();
    let intrinsic = f::fact_intrinsic(&owner, true);
    let sources = [f::source(&owner, 1)];
    let resolver = GpuSemanticExpressionResolverV2::new(owner.source_semantic().types(), function);
    let mut deferred = DeferredSourceValuesV1::new(
        resolver,
        owner
            .source_query_for_root(fixtures::ROOT, function)
            .unwrap(),
        &intrinsic,
        owner.source_semantic().callables(),
        &sources,
    );
    let kernel = f::kernel(true);
    let cpu = f::cpu_ir();
    let write = f::write(1);
    let mut guard = source_scalar_test_guard_v1(&kernel, &cpu, &write).unwrap();
    assert_eq!(deferred.resolve(&write, &mut guard), Err(UNPROVED));
    assert!(
        deferred
            .resolver
            .source_ssa
            .as_ref()
            .unwrap()
            .proved_some
            .is_empty()
    );
    cleared(&deferred);
}

#[test]
fn deferred_scalar_gpu_facts_do_not_leak_between_writes_or_after_failure() {
    let owner = f::discriminant_owner();
    let function = owner
        .execution_view_for_root(fixtures::ROOT)
        .unwrap()
        .body();
    let intrinsic = f::fact_intrinsic(&owner, false);
    let sources = [f::source(&owner, 1), f::source(&owner, 2)];
    let resolver = GpuSemanticExpressionResolverV2::new(owner.source_semantic().types(), function);
    let mut deferred = DeferredSourceValuesV1::new(
        resolver,
        owner
            .source_query_for_root(fixtures::ROOT, function)
            .unwrap(),
        &intrinsic,
        owner.source_semantic().callables(),
        &sources,
    );
    let kernel = f::kernel(true);
    let cpu = f::cpu_ir();
    let proven = f::write(1);
    let unproven = f::write(2);
    let mut first_guard = source_scalar_test_guard_v1(&kernel, &cpu, &proven).unwrap();
    let expected = deferred.resolve(&proven, &mut first_guard).unwrap();
    assert_eq!(
        deferred
            .resolver
            .source_ssa
            .as_ref()
            .unwrap()
            .proved_some
            .len(),
        1
    );
    let spent = deferred.resolver.work;
    let mut second_guard = source_scalar_test_guard_v1(&kernel, &cpu, &unproven).unwrap();
    assert_eq!(deferred.resolve(&unproven, &mut first_guard), Err(CUSTODY));
    assert_eq!(
        deferred.resolve(&unproven, &mut second_guard),
        Err(UNPROVED)
    );
    assert!(
        deferred
            .resolver
            .source_ssa
            .as_ref()
            .unwrap()
            .proved_some
            .is_empty()
    );
    assert!(deferred.resolver.work > spent);
    cleared(&deferred);
    let mut repeated_guard = source_scalar_test_guard_v1(&kernel, &cpu, &proven).unwrap();
    assert_eq!(deferred.resolve(&proven, &mut repeated_guard), Ok(expected));
    cleared(&deferred);
}
