mod static_publication_v1_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;

    include!("static_publication_fixture_v1_tests.rs");
    include!("static_publication_flow_v1_tests.rs");
    include!("static_publication_memory_v1_tests.rs");

    #[test]
    fn static_publication_owner_retains_exact_ordered_effect_rosters() {
        for publish in [false, true] {
            for copy in [false, true] {
                let owner = owner(publish, copy);
                owner.verify_equivalence().unwrap();
                verify_module(owner.module()).unwrap();
                let operations: Vec<_> = owner.module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .collect();
                let effects: Vec<_> = operations
                    .iter()
                    .filter(|op| {
                        matches!(
                            op.kind,
                            OperationKind::Store { .. }
                                | OperationKind::Atomic(_)
                                | OperationKind::GuardedLoad { .. }
                        )
                    })
                    .collect();
                assert_eq!(effects.len(), if publish { 2 } else { 3 });
                let OperationKind::Atomic(release) = &effects[usize::from(publish)].kind else {
                    panic!("release");
                };
                assert_eq!(release.ordering, MemoryOrdering::Release);
                assert_eq!(release.scope, SynchronizationScope::System);
                let value = release.value.unwrap();
                let definition = operations
                    .iter()
                    .find(|op| op.results.iter().any(|v| v.id == value))
                    .unwrap();
                assert_eq!(
                    definition.kind,
                    OperationKind::Constant(Constant::U32(if publish { 2 } else { 1 }))
                );
                if !publish {
                    let OperationKind::Atomic(acquire) = &effects[1].kind else {
                        panic!("acquire");
                    };
                    assert_eq!(acquire.pointer, release.pointer);
                    assert_eq!(acquire.ordering, MemoryOrdering::Acquire);
                    assert_eq!(acquire.scope, SynchronizationScope::System);
                    assert!(matches!(effects[2].kind, OperationKind::GuardedLoad { .. }));
                }
            }
        }
    }

    #[test]
    fn static_publication_copy_requires_original_exclusive_argument() {
        let admitted = request(false, true, false)
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        let source = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert!(
            ProductionSemanticKirOwnerV1::try_lower(
                source,
                ProductionSemanticKirLimitsV1::default()
            )
            .is_err()
        );
    }

    #[test]
    fn static_publication_owner_replay_rejects_well_typed_protocol_mutants() {
        for mutation in 0..10 {
            let mut owner = owner(false, true);
            let RetainedProductionKirModuleV1::Legacy(module) = &mut owner.module else {
                panic!("legacy fixture");
            };
            let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
            let read = operations
                .iter()
                .position(|op| matches!(op.kind, OperationKind::GuardedLoad { .. }))
                .unwrap();
            let OperationKind::GuardedLoad {
                pointer,
                predicate,
                fallback,
                access,
            } = operations[read].kind
            else {
                unreachable!();
            };
            let release = operations
                .iter()
                .position(|op| {
                    matches!(
                        op.kind,
                        OperationKind::Atomic(Atomic {
                            kind: AtomicKind::Store,
                            ..
                        })
                    )
                })
                .unwrap();
            let acquire = operations
                .iter()
                .position(|op| {
                    matches!(
                        op.kind,
                        OperationKind::Atomic(Atomic {
                            kind: AtomicKind::Load,
                            ..
                        })
                    )
                })
                .unwrap();
            match mutation {
                0 | 1 => {
                    operations
                        .iter_mut()
                        .find(|op| op.results.iter().any(|v| v.id == predicate))
                        .unwrap()
                        .kind = OperationKind::Constant(Constant::Bool(mutation == 0))
                }
                2 => {
                    operations
                        .iter_mut()
                        .find(|op| op.results.iter().any(|v| v.id == fallback))
                        .unwrap()
                        .kind = OperationKind::Constant(Constant::F32Bits(1.0_f32.to_bits()))
                }
                3 => {
                    operations
                        .iter_mut()
                        .find(|op| matches!(op.kind, OperationKind::SliceLength { .. }))
                        .unwrap()
                        .kind = OperationKind::Constant(Constant::Index(128))
                }
                4 => operations[read].kind = OperationKind::Load { pointer, access },
                5 => {
                    let OperationKind::Atomic(atomic) = &mut operations[acquire].kind else {
                        unreachable!();
                    };
                    atomic.ordering = MemoryOrdering::Relaxed;
                }
                6 => {
                    let OperationKind::Atomic(atomic) = &mut operations[release].kind else {
                        unreachable!();
                    };
                    atomic.scope = SynchronizationScope::Workgroup;
                }
                7 => {
                    let OperationKind::Atomic(atomic) = &operations[release].kind else {
                        unreachable!();
                    };
                    let value = atomic.value.unwrap();
                    operations
                        .iter_mut()
                        .find(|op| op.results.iter().any(|v| v.id == value))
                        .unwrap()
                        .kind = OperationKind::Constant(Constant::U32(2));
                }
                8 => operations.swap(release, acquire),
                9 => {
                    let operation = operations
                        .iter_mut()
                        .find(|op| {
                            matches!(
                                op.kind,
                                OperationKind::Compare {
                                    predicate: ComparePredicate::Equal,
                                    ..
                                }
                            )
                        })
                        .unwrap();
                    let OperationKind::Compare { predicate, .. } = &mut operation.kind else {
                        unreachable!();
                    };
                    *predicate = ComparePredicate::NotEqual;
                }
                _ => unreachable!(),
            }
            verify_module(owner.module()).expect("mutant remains structurally valid");
            assert!(
                matches!(
                    owner.verify_equivalence(),
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ),
                "mutation {mutation}"
            );
        }
    }
}
