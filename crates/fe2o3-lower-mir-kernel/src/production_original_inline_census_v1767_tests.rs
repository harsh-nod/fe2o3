#[test]
fn private_ordinary_census_keeps_real_inline_profile_through_complete_original_replay() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let original = repeated_inline_owner_v18();
    let version = original.source_semantic().wire_version();
    let sha = *original.source_semantic_sha256();
    assert_ne!(version, SemanticMirWireVersionV1::V29);
    let prepared = scalar_payload_prepared_from_v18(repeated_inline_owner_v18, &mut budget);
    let visited = std::cell::Cell::new(false);
    prepared
        .with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            let actual = source.source_ssa(budget)?;
            assert_eq!(actual.source_semantic_sha256(), &sha);
            assert_eq!(actual.source_semantic().wire_version(), version);
            source.with_analysis_v18(budget, |scope| {
                scope.with_sparse_and_memory_ssa_v1(|_, memory, budget| {
                    source.with_ranked_correspondence_v18(
                        memory.inventory(),
                        budget,
                        |relation, budget| {
                            let mut inline = 0;
                            for root in 0..source.root_count(budget)? {
                                let cache = Gfx942InlineScalarCorrespondenceV30::build_source_v18(
                                    relation, root, budget,
                                )?;
                                let function =
                                    &memory.inventory().functions()[source.root(root, budget)?.1];
                                inline += memory.inventory().operations()
                                    [function.operations.clone()]
                                .iter()
                                .filter(|row| {
                                    matches!(row.operation.kind, OperationKind::InlineAssembly(_))
                                })
                                .count();
                                drop(cache);
                                scoped_raw_admission_v29::with_checked_source_memory_v29(
                                    relation,
                                    root,
                                    Some(memory),
                                    budget,
                                    |physical, budget| -> SourceOwnedResultV18<()> {
                                        physical.visit_effects(budget, |_, _| Ok(()))?;
                                        Ok(())
                                    },
                                )?;
                            }
                            assert_eq!(inline, 2, "both real original helper instances");
                            visited.set(true);
                            Ok(())
                        },
                    )
                })
            })
        })
        .unwrap();
    assert!(visited.get());
    assert_eq!(budget.storage(), MODULE_FLOOR);
    assert_eq!(original.source_semantic_sha256(), &sha);
}

#[test]
fn ordinary_policy_cannot_hide_genuine_projected_roots_events_or_nominal_catalog() {
    use crate::production_execution_source_input_v29::{
        check_original_ordinary_census_v18, check_source_owned_census_v18,
    };
    let owner = module_fixture_owner(ModuleFixture::Mixed);
    let semantic = owner.source_semantic();
    let first_nominal = semantic
        .types()
        .iter()
        .position(|ty| matches!(ty.rust_type_kind(), SemanticRustTypeKindV1::Execution(_)))
        .unwrap();
    assert!(semantic.callables().iter().any(|callable| matches!(
        callable,
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::Execution(_),
            ..
        }
    )));
    let original = *owner.source_semantic_sha256();
    let ordinary =
        vec![crate::ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    with_module_fixture_view(
        &owner,
        ModuleFixture::Mixed,
        &mut budget,
        |source, budget| {
            assert!(!source.input.roots.is_empty());
            assert!(!source.input.events.is_empty());
            for mode in 0..3 {
                let before = (budget.work(), budget.storage());
                let projected = crate::ProductionExecutionSourceInputV29 {
                    semantic_sha256: &original,
                    roots: if mode == 0 { source.input.roots } else { &[] },
                    classes: &ordinary,
                    events: if mode == 1 { source.input.events } else { &[] },
                };
                let result =
                    check_original_ordinary_census_v18(&owner, source.launch, projected, budget);
                assert!(
                    matches!(result, Err(crate::ProductionContextRootErrorV29::Source)),
                    "mode {mode}: {result:?}"
                );
                assert_eq!(budget.storage(), before.1);
                assert_eq!(
                    budget.work() - before.0,
                    if mode == 2 {
                        32 + first_nominal + 1
                    } else {
                        32
                    }
                );
            }
            // Refusing an ordinary interpretation must not alter the genuine V29 route.
            check_source_owned_census_v18(&owner, source.launch, source.input, budget).unwrap();
            assert_eq!(owner.source_semantic_sha256(), &original);
            assert_eq!(
                owner.source_semantic().wire_version(),
                SemanticMirWireVersionV1::V29
            );
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
