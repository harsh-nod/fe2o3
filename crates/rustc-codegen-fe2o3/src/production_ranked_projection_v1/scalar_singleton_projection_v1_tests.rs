// Genuine source baseline; malformed mutations below are eligibility tests,
// never substitutes for admitted source/N or executable owners.
struct ScalarSingletonMeterV1<'a, 'w> {
    budget: &'a mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'w>,
}

impl ProjectedAssertionFactsV1 for ScalarSingletonMeterV1<'_, '_> {
    fn charge_private_array_work(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.budget
            .charge_work(amount)
            .map_err(ranked_projection_source_v1::resource)
    }
    fn scalar_private_storage_v1(&self) -> Result<usize, ProductionRankedProjectionErrorV1> {
        Ok(self.budget.storage())
    }
    fn reserve_scalar_private_storage_v1(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.budget
            .reserve_storage(amount)
            .map_err(ranked_projection_source_v1::resource)
    }
    fn release_scalar_private_storage_v1(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.budget
            .release_storage(amount)
            .map_err(ranked_projection_source_v1::resource)
    }
    fn private_array_initializer_count(
        &mut self,
        _: usize,
        _: usize,
    ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
        panic!("eligibility census does not prove array initialization")
    }
    fn is_materialized_block(
        &mut self,
        _: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        panic!("eligibility census does not interpret CFG")
    }
    fn condition(
        &mut self,
        _: usize,
        _: bool,
        _: SemanticBlockIdV1,
    ) -> Result<
        canonical_assertion_facts_v1::ProjectedAssertionConditionV1,
        ProductionRankedProjectionErrorV1,
    > {
        panic!("eligibility census does not prove assertions")
    }
}

fn scalar_singleton_census_fixture_v1() -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let (source, _) = erased_backend_materialized_fixture_v1(true, 1);
    let semantic = source.semantic_ssa().source_semantic();
    (semantic.types().to_vec(), semantic.functions()[0].clone())
}

fn scalar_singleton_append_v1(
    function: &SemanticFunctionDeclV1,
    statement: SemanticStatementV1,
) -> SemanticFunctionDeclV1 {
    let mut blocks = function.blocks().to_vec();
    let block = &blocks[1];
    let mut statements = block.statements().to_vec();
    statements.push(statement);
    blocks[1] = SemanticBasicBlockV1::new(
        block.identity(),
        block.source(),
        statements,
        block.terminator().clone(),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        function.locals().to_vec(),
        function.entry(),
        blocks,
    )
    .unwrap()
}

fn scalar_singleton_run_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<Vec<bool>, ProductionRankedProjectionErrorV1>,
    usize,
    usize,
) {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as B, CanonicalKernelIrWorkBudgetV1 as W,
    };
    let mut work = W::new(work_limit);
    let mut budget = B::new(&mut work, storage_limit);
    budget.reserve_storage(19).unwrap();
    let result = scalar_singleton_projection_v1::with_scalar_private_singletons_v1(
        types,
        function,
        &mut ScalarSingletonMeterV1 {
            budget: &mut budget,
        },
        |census, _| {
            Ok((0..function.locals().len())
                .map(|i| {
                    scalar_singleton_projection_v1::eligible(
                        census,
                        SemanticLocalIdV1::from_index(i as u32),
                    )
                })
                .collect())
        },
    );
    assert_eq!(budget.storage(), 19);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn scalar_private_singleton_real_projector_preserves_one_and_two_root_effects() {
    for roots in [1, 2] {
        for expected in [false, true] {
            let (source, inputs) = erased_backend_materialized_fixture_v1(expected, roots);
            let program = project_and_verify_ranked_materialized_semantic_mir_v1(
                source,
                &inputs,
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
            )
            .unwrap();
            assert!(program.all_kernel_checks_are_clean());
            assert_eq!(program.root_count(), roots);
            for root in &program.roots {
                let mut views = std::collections::BTreeSet::new();
                for operation in root
                    .verification
                    .ordinary()
                    .expect("ordinary test root")
                    .kernel()
                    .blocks()
                    .iter()
                    .flat_map(|b| b.operations())
                {
                    if let ProductionRankedOperationV1::ViewInSpace {
                        result,
                        memory_space: MemorySpaceAttr::Private,
                        shape,
                        dynamic_extents,
                        allocation_origin,
                        noalias_class,
                        ..
                    } = operation
                    {
                        assert_eq!(shape.as_slice(), &[1]);
                        assert!(dynamic_extents.is_empty());
                        assert!(*allocation_origin > PRIVATE_ALLOCATION_ORIGIN_TAG_V1);
                        assert_eq!(allocation_origin, noalias_class);
                        views.insert(*result);
                    }
                }
                assert_eq!(views.len(), 2);
                let mut private_reads = 0;
                let mut private_writes = 0;
                for (block, recipe) in root
                    .verification
                    .ordinary()
                    .expect("ordinary test root")
                    .kernel()
                    .blocks()
                    .iter()
                    .enumerate()
                {
                    for (operation, item) in recipe.operations().iter().enumerate() {
                        if let ProductionRankedOperationV1::Access {
                            kind,
                            view: ProductionRankedValueV1::Local(view),
                            indices,
                        } = item
                            && views.contains(view)
                        {
                            assert_eq!(indices.len(), 1);
                            private_reads += usize::from(*kind == AccessKindAttr::Read);
                            private_writes += usize::from(*kind == AccessKindAttr::Write);
                            assert!(
                                !root
                                    .access_sources
                                    .iter()
                                    .any(|source| source.ranked_block() as usize == block
                                        && source.ranked_operation() as usize == operation)
                            );
                        }
                    }
                }
                assert_eq!((private_reads, private_writes), (1, 2));
            }
        }
    }
}

#[test]
fn scalar_private_singleton_census_exact_work_storage_and_scope_cleanup() {
    let (types, function) = scalar_singleton_census_fixture_v1();
    let (result, work, peak) = scalar_singleton_run_v1(&types, &function, usize::MAX, usize::MAX);
    let flags = result.unwrap();
    assert!(flags[3] && flags[7]);
    assert!(!flags[0] && !flags[1] && !flags[2] && !flags[4] && !flags[5] && !flags[6]);
    assert!(
        scalar_singleton_run_v1(&types, &function, work, peak)
            .0
            .is_ok()
    );
    assert!(
        scalar_singleton_run_v1(&types, &function, work - 1, peak)
            .0
            .is_err()
    );
    assert!(
        scalar_singleton_run_v1(&types, &function, work, peak - 1)
            .0
            .is_err()
    );
}

#[test]
fn scalar_private_singleton_rejects_all_borrows_addresses_and_projected_places() {
    let (types, function) = scalar_singleton_census_fixture_v1();
    let pointer = function.locals()[2].ty();
    let mut hostile = Vec::new();
    for kind in [
        SemanticBorrowKindV1::Mutable,
        SemanticBorrowKindV1::Shared,
        SemanticBorrowKindV1::Fake,
    ] {
        hostile.push(typed_assignment(
            2,
            pointer,
            SemanticRvalueKindV1::Borrow {
                kind,
                place: typed_place(3, A_U32),
            },
        ));
    }
    hostile.push(typed_assignment(
        2,
        pointer,
        SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Mutable,
            place: typed_place(3, A_U32),
        },
    ));
    hostile.push(typed_assignment(
        4,
        A_U32,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(3),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, A_U32).unwrap(),
                ],
                A_U32,
            )
            .unwrap(),
        )),
    ));
    for statement in hostile {
        let changed = scalar_singleton_append_v1(&function, statement);
        let flags = scalar_singleton_run_v1(&types, &changed, usize::MAX, usize::MAX)
            .0
            .unwrap();
        assert!(!flags[3]);
        assert!(flags[7], "unrelated private cell remains eligible");
    }
}

#[test]
fn scalar_private_singleton_rejects_volatile_and_atomic_accesses() {
    let (types, function) = scalar_singleton_census_fixture_v1();
    for (volatility, atomic) in [
        (SemanticVolatilityV1::Volatile, None),
        (
            SemanticVolatilityV1::NonVolatile,
            Some(SemanticAtomicAccessV1::new(
                SemanticAtomicOrderingV1::Relaxed,
                SemanticAtomicScopeV1::SingleThread,
            )),
        ),
    ] {
        for read in [false, true] {
            let operation = if read {
                typed_assignment(
                    4,
                    A_U32,
                    SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                        typed_place(3, A_U32),
                        volatility,
                        atomic,
                    )),
                )
            } else {
                statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    typed_place(3, A_U32),
                    typed_constant(A_U32, 7, 4),
                    volatility,
                    atomic,
                )))
            };
            let changed = scalar_singleton_append_v1(&function, operation);
            assert!(
                !scalar_singleton_run_v1(&types, &changed, usize::MAX, usize::MAX)
                    .0
                    .unwrap()[3]
            );
        }
    }
}

#[test]
fn scalar_private_singleton_rejects_entry_nonscalar_and_malformed_places() {
    let (types, function) = scalar_singleton_census_fixture_v1();
    let by_value = assertion_root_with_access(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (A_U32, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![A_U32],
        vec![block(
            188,
            vec![statement(SemanticStatementKindV1::Store(
                SemanticMemoryStoreV1::new(
                    typed_place(1, A_U32),
                    typed_constant(A_U32, 7, 4),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                ),
            ))],
            SemanticTerminatorKindV1::Return,
        )],
        false,
    );
    assert!(
        !scalar_singleton_run_v1(&types, &by_value, usize::MAX, usize::MAX)
            .0
            .unwrap()[1]
    );
    let nonscalar = scalar_singleton_append_v1(
        &function,
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            typed_place(0, A_UNIT),
            typed_constant(A_UNIT, 0, 1),
            SemanticVolatilityV1::NonVolatile,
            None,
        ))),
    );
    assert!(
        !scalar_singleton_run_v1(&types, &nonscalar, usize::MAX, usize::MAX)
            .0
            .unwrap()[0]
    );
    for place in [typed_place(999, A_U32), typed_place(3, A_U64)] {
        let changed = scalar_singleton_append_v1(
            &function,
            statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place,
                typed_constant(A_U32, 7, 4),
                SemanticVolatilityV1::NonVolatile,
                None,
            ))),
        );
        assert!(
            scalar_singleton_run_v1(&types, &changed, usize::MAX, usize::MAX)
                .0
                .is_err()
        );
    }
}

#[test]
fn scalar_private_singleton_callback_error_and_panic_restore_exact_floor() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as B, CanonicalKernelIrWorkBudgetV1 as W,
    };
    let (types, function) = scalar_singleton_census_fixture_v1();
    let mut work = W::new(usize::MAX);
    let mut budget = B::new(&mut work, usize::MAX);
    budget.reserve_storage(23).unwrap();
    let error = scalar_singleton_projection_v1::with_scalar_private_singletons_v1::<(), _>(
        &types,
        &function,
        &mut ScalarSingletonMeterV1 {
            budget: &mut budget,
        },
        |_, _| {
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "singleton callback sentinel",
            ))
        },
    );
    assert!(matches!(
        error,
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "singleton callback sentinel"
        ))
    ));
    assert_eq!(budget.storage(), 23);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scalar_singleton_projection_v1::with_scalar_private_singletons_v1::<(), _>(
            &types,
            &function,
            &mut ScalarSingletonMeterV1 {
                budget: &mut budget,
            },
            |_, _| std::panic::panic_any(719_u32),
        )
    }))
    .unwrap_err();
    assert_eq!(*panic.downcast::<u32>().unwrap(), 719);
    assert_eq!(budget.storage(), 23);
}

#[test]
fn scalar_private_singleton_no_candidate_needs_no_storage_adapter() {
    let owner = materialized_aggregate_helper_v1();
    let semantic = owner.semantic_ssa().source_semantic();
    let function = &semantic.functions()[0];
    let mut synthetic = canonical_assertion_facts_v1::ProjectedAssertionConditionV1::Dynamic;
    scalar_singleton_projection_v1::with_scalar_private_singletons_v1(
        semantic.types(),
        function,
        &mut synthetic,
        |census, _| {
            assert!(census.is_empty());
            Ok(())
        },
    )
    .unwrap();
}
