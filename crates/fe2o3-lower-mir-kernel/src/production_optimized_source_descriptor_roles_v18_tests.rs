#[derive(Clone, Copy, Debug)]
enum DescriptorRoleSourceV18 {
    Constant,
    ReadValue,
    Arithmetic,
    Volatile,
    VolatileOnly,
    VolatileRead,
}

include!("production_optimized_source_descriptor_read_fixture_v18_tests.rs");
include!("production_optimized_source_global_access_v18_tests.rs");
include!("production_optimized_source_global_native_fixtures_v18_tests.rs");
include!("production_optimized_source_global_read_conditions_v18_tests.rs");
include!("production_optimized_source_shared_entry_v18_tests.rs");
include!("production_optimized_source_issued_role_fixture_v18_tests.rs");

#[derive(Clone, Copy, Debug)]
enum DescriptorRoleEntranceV18 {
    OriginalUniqueBorrow,
    IssuedDisjointSlice,
}

thread_local! {
    static DESCRIPTOR_ROLE_RETAINED_FLOOR_V18: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

fn descriptor_role_owner_v18(mode: DescriptorRoleSourceV18) -> ProductionSemanticSsaOwnerV1 {
    let base = descriptor_source_owner(DescriptorCase {
        write: true,
        ..DescriptorCase::READ
    });
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let SemanticStatementKindV1::Assign(store) = original.blocks()[1].statements()[0].kind() else {
        panic!("original descriptor write");
    };
    let indexed = store.destination().clone();
    let mut locals = original.locals().to_vec();
    assert_eq!(locals.len(), 6);
    locals.push(local(230, U32, SemanticLocalRoleV1::Temporary));
    let mut statements = vec![assign(
        place(6, U32),
        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
            indexed.clone(),
            if matches!(mode, DescriptorRoleSourceV18::VolatileRead) {
                SemanticVolatilityV1::Volatile
            } else {
                SemanticVolatilityV1::NonVolatile
            },
            None,
        )),
    )];
    if matches!(mode, DescriptorRoleSourceV18::VolatileOnly) {
        statements.clear();
    }
    let value = match mode {
        DescriptorRoleSourceV18::Constant
        | DescriptorRoleSourceV18::Volatile
        | DescriptorRoleSourceV18::VolatileOnly
        | DescriptorRoleSourceV18::VolatileRead => literal(17),
        DescriptorRoleSourceV18::ReadValue => SemanticOperandV1::Copy(place(6, U32)),
        DescriptorRoleSourceV18::Arithmetic => {
            locals.push(local(231, U32, SemanticLocalRoleV1::Temporary));
            statements.push(assign(
                place(7, U32),
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: SemanticOperandV1::Copy(place(6, U32)),
                    right: literal(1),
                },
            ));
            SemanticOperandV1::Copy(place(7, U32))
        }
    };
    statements.push(SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            indexed,
            value,
            if matches!(
                mode,
                DescriptorRoleSourceV18::Volatile | DescriptorRoleSourceV18::VolatileOnly
            ) {
                SemanticVolatilityV1::Volatile
            } else {
                SemanticVolatilityV1::NonVolatile
            },
            None,
        )),
    ));
    let mut blocks = original.blocks().to_vec();
    blocks[1] = block(232, statements, SemanticTerminatorKindV1::Return);
    let root = function(200, original.role(), original.abi().clone(), locals, blocks)
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root],
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn run_descriptor_roles_v18(
    entrance: DescriptorRoleEntranceV18,
    mode: DescriptorRoleSourceV18,
    work_limit: usize,
    storage_limit: usize,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    match entrance {
        DescriptorRoleEntranceV18::OriginalUniqueBorrow => run_descriptor_role_owner_v18(
            descriptor_role_owner_v18(mode),
            work_limit,
            storage_limit,
            consume,
        ),
        DescriptorRoleEntranceV18::IssuedDisjointSlice => {
            let owner = issued_descriptor_role_owner_v18(mode);
            let abi = issued_descriptor_role_abi_v18(&owner);
            run_descriptor_role_owner_with_abi_v18(owner, abi, work_limit, storage_limit, consume)
        }
    }
}

fn run_descriptor_role_owner_v18(
    owner: ProductionSemanticSsaOwnerV1,
    work_limit: usize,
    storage_limit: usize,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    let abi = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
    run_descriptor_role_owner_with_abi_v18(owner, abi, work_limit, storage_limit, consume)
}

fn run_descriptor_role_owner_with_abi_v18(
    owner: ProductionSemanticSsaOwnerV1,
    abi: kernel_argument_abi_v18::tests::FixtureKernelAbiV18,
    work_limit: usize,
    storage_limit: usize,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    let (result, work, peak) = run_descriptor_role_owner_result_with_abi_v18(
        owner,
        abi,
        work_limit,
        storage_limit,
        consume,
    );
    let result = result.map_err(|error| match error {
        ProductionSourceOptimizationErrorV18::Source(error) => error,
        other => panic!("actual source-owned optimizer and consumer: {other:?}"),
    });
    (result, work, peak)
}

// Custody negatives need the selected callback's actual optimizer variant;
// ordinary positive helpers must still reject unexpected adoption failures.
fn run_descriptor_role_owner_result_with_abi_v18(
    owner: ProductionSemanticSsaOwnerV1,
    abi: kernel_argument_abi_v18::tests::FixtureKernelAbiV18,
    work_limit: usize,
    storage_limit: usize,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (ProductionOptimizerTestResultV18, usize, usize) {
    let semantic = owner.source_semantic();
    let launch_roots: Vec<_> = semantic
        .roots()
        .iter()
        .map(|root| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch = ProductionSourceLaunchRosterV1::try_new(semantic, &launch_roots).unwrap();
    let sha = *owner.source_semantic_sha256();
    let classes = vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let roots = abi.roots();
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: &sha,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let result = (|| -> ProductionOptimizerTestResultV18 {
        let prepared =
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                owner,
                launch,
                input,
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )
            .map_err(ProductionSourceOptimizationErrorV18::Source)?;
        with_production_optimizer_result_v18(prepared, &mut budget, consume)
    })();
    assert_eq!(
        budget.storage(),
        DESCRIPTOR_ROLE_RETAINED_FLOOR_V18
            .get()
            .unwrap_or(MODULE_FLOOR),
        "{result:?}"
    );
    (result, budget.work(), budget.peak_storage())
}

fn check_descriptor_roles_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    expected_write: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let root = original.source.root(0, budget)?.1;
    // Inert symbol namespace only. Source value and output expression checking
    // remain mandatory; this is not a ranked-memory or reference certificate.
    let recipe = scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
    original.with_descriptor_source_roles_v18(optimized, 0, &recipe, budget, |roles, budget| {
        let output = optimized.output_inventory(budget)?;
        let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
        let (mut reads, mut writes, mut addresses, mut lengths, mut data) = (0, 0, 0, 0, 0);
        for operation in &output.operations()[function.operations.clone()] {
            let role = roles.role(operation.coordinate, budget)?;
            match operation.operation.kind {
                OperationKind::Load { .. } => {
                    assert_eq!(role, Some(DescriptorSourceRoleV18::Read));
                    reads += 1;
                }
                OperationKind::Store { .. } => {
                    assert_eq!(
                        role,
                        expected_write.then_some(DescriptorSourceRoleV18::Write)
                    );
                    writes += 1;
                }
                OperationKind::GetElementPointer { .. } => {
                    if role.is_some() {
                        assert_eq!(role, Some(DescriptorSourceRoleV18::Address));
                        addresses += 1;
                    }
                }
                OperationKind::SliceData { .. } => {
                    if role.is_some() {
                        assert_eq!(role, Some(DescriptorSourceRoleV18::Data));
                        data += 1;
                    }
                }
                OperationKind::SliceLength { .. } => {
                    assert_eq!(role, Some(DescriptorSourceRoleV18::Length));
                    lengths += 1;
                }
                _ => assert!(role.is_none(), "non-descriptor roles remain unresolved"),
            }
        }
        assert!(reads > 0 && writes > 0 && addresses > 0 && lengths > 0 && data > 0);
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    })
}

#[test]
fn original_descriptor_roles_check_actual_read_write_and_their_address_dependencies() {
    let entrance = DescriptorRoleEntranceV18::IssuedDisjointSlice;
    for mode in [
        DescriptorRoleSourceV18::Constant,
        DescriptorRoleSourceV18::ReadValue,
        DescriptorRoleSourceV18::Arithmetic,
        DescriptorRoleSourceV18::Volatile,
    ] {
        let completed = std::cell::Cell::new(false);
        let result = run_descriptor_roles_v18(
            entrance,
            mode,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                check_descriptor_roles_v18(
                    original,
                    optimized,
                    matches!(
                        mode,
                        DescriptorRoleSourceV18::Constant | DescriptorRoleSourceV18::ReadValue
                    ),
                    budget,
                )?;
                completed.set(true);
                Ok(())
            },
        )
        .0;
        assert!(result.is_ok() && completed.get(), "{mode:?}: {result:?}");
    }
}

#[test]
fn original_descriptor_role_scope_repeats_at_fixed_peak_and_preserves_owned_callback_backing() {
    let entrance = DescriptorRoleEntranceV18::IssuedDisjointSlice;
    let completed = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(
        entrance,
        DescriptorRoleSourceV18::ReadValue,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            let root = original.source.root(0, budget)?.1;
            let recipe =
                scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
            let floor = budget.storage();
            let mut peak = None;
            for _ in 0..3 {
                check_descriptor_roles_v18(original, optimized, true, budget)?;
                assert_eq!(budget.storage(), floor);
                if let Some(peak) = peak {
                    assert_eq!(budget.peak_storage(), peak);
                }
                peak = Some(budget.peak_storage());
            }
            for mode in 0..3 {
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    original.with_descriptor_source_roles_v18(
                        optimized,
                        0,
                        &recipe,
                        budget,
                        |_, budget| {
                            let payload = owned_callback_payload_v18(budget);
                            match mode {
                                0 => Ok(payload),
                                1 => Err(OwnedCallbackErrorV18::Payload(payload)),
                                _ => {
                                    budget.reserve_storage(size_of::<Vec<u64>>()).unwrap();
                                    std::panic::resume_unwind(Box::new(payload))
                                }
                            }
                        },
                    )
                }));
                let bytes =
                    64 * size_of::<u64>() + if mode == 2 { size_of::<Vec<u64>>() } else { 0 };
                assert_eq!(budget.storage(), floor + bytes);
                match (mode, caught) {
                    (0, Ok(Ok(payload)))
                    | (1, Ok(Err(OwnedCallbackErrorV18::Payload(payload)))) => {
                        assert_eq!(payload, vec![0x271; 64]);
                        drop(payload);
                    }
                    (2, Err(payload)) => {
                        let payload = payload.downcast::<Vec<u64>>().unwrap();
                        assert_eq!(*payload, vec![0x271; 64]);
                        drop(payload);
                    }
                    _ => panic!("descriptor callback disposition changed"),
                }
                budget.release_storage(bytes)?;
                assert_eq!(budget.storage(), floor);
            }
            completed.set(true);
            Ok(())
        },
    )
    .0;
    assert!(result.is_ok() && completed.get(), "{result:?}");
}

#[test]
fn original_descriptor_role_query_rejects_foreign_occurrence_and_preserves_first_refusal() {
    let entrance = DescriptorRoleEntranceV18::IssuedDisjointSlice;
    let completed = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(
        entrance,
        DescriptorRoleSourceV18::ReadValue,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            check_descriptor_roles_v18(original, optimized, true, budget)?;
            let root = original.source.root(0, budget)?.1;
            let recipe =
                scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
            original.with_descriptor_source_roles_v18(
                optimized,
                0,
                &recipe,
                budget,
                |roles, budget| {
                    let output = optimized.output_inventory(budget)?;
                    let function =
                        optimized_source_root_function_v18(original, optimized, 0, budget)?;
                    let exact = output.operations()[function.operations.start].coordinate;
                    let mut foreign = exact;
                    foreign.block.function =
                        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(u32::MAX);
                    let refused = roles.role(foreign, budget);
                    assert!(matches!(
                        refused,
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "descriptor role query changed actual occurrence"
                        ))
                    ));
                    let before = (budget.work(), budget.storage());
                    assert!(matches!(
                        roles.role(exact, budget),
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "descriptor role query changed actual occurrence"
                        ))
                    ));
                    assert_eq!((budget.work(), budget.storage()), before);
                    completed.set(true);
                    // Ignoring the local query error cannot publish a successful
                    // enclosing original source transaction.
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )
        },
    )
    .0;
    assert!(
        completed.get(),
        "the exact refusal and zero-debit replay must both complete"
    );
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "descriptor role query changed actual occurrence"
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn original_descriptor_role_entrance_authenticates_exact_correspondence_before_new_debits() {
    let entrance = DescriptorRoleEntranceV18::IssuedDisjointSlice;
    let completed = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(
        entrance,
        DescriptorRoleSourceV18::Constant,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            check_descriptor_roles_v18(original, optimized, true, budget)?;
            let root = original.source.root(0, budget)?.1;
            let recipe =
                scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
            original.source.with_ranked_correspondence_v18(
                original.inventory,
                budget,
                |other, budget| {
                    assert!(!std::ptr::eq(original, other));
                    let before = (budget.work(), budget.storage());
                    let refused = other.with_descriptor_source_roles_v18(
                        optimized,
                        0,
                        &recipe,
                        budget,
                        |_, _| -> SourceOwnedResultV18<()> {
                            panic!("foreign exact correspondence reached role consumer");
                        },
                    );
                    assert!(matches!(
                        refused,
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "optimized source substituted its exact correspondence"
                        ))
                    ));
                    assert_eq!((budget.work(), budget.storage()), before);
                    completed.set(true);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )
        },
    )
    .0;
    assert!(completed.get());
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized source substituted its exact correspondence"
            ))
        ),
        "{result:?}"
    );
}

fn descriptor_role_boundary_run_v18(
    entrance: DescriptorRoleEntranceV18,
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    let visited = std::cell::Cell::new(false);
    let (result, work, storage) = run_descriptor_roles_v18(
        entrance,
        DescriptorRoleSourceV18::ReadValue,
        work_limit,
        storage_limit,
        |original, optimized, budget| {
            check_descriptor_roles_v18(original, optimized, true, budget)?;
            visited.set(true);
            Ok(())
        },
    );
    (result, work, storage, visited.get())
}

#[test]
fn original_descriptor_role_complete_transaction_exact_and_one_short_limits() {
    let entrance = DescriptorRoleEntranceV18::IssuedDisjointSlice;
    let (result, work, storage, visited) =
        descriptor_role_boundary_run_v18(entrance, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(visited);
    let exact = descriptor_role_boundary_run_v18(entrance, work, storage);
    exact.0.unwrap();
    assert!(exact.3);
    assert_eq!((exact.1, exact.2), (work, storage));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let short = descriptor_role_boundary_run_v18(entrance, work_limit, storage_limit);
        // The role callback may run before a final metered postflight refuses.
        // Completion requires the whole original/optimized transaction's Ok.
        match (
            is_work,
            source_slot_tests::original_repeated_source_resource_v29(short.0.unwrap_err()),
        ) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work_limit);
                assert!(error.actual() > work_limit);
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage_limit);
                assert!(error.actual() > storage_limit);
            }
            other => panic!("descriptor role boundary {other:?}, visited={}", short.3),
        }
    }
}

#[test]
fn original_descriptor_role_scope_observes_local_floor_on_success_error_and_unwind() {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(self.0);
        }
    }
    let _restore = Restore(DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.replace(None));
    for mode in 0..3 {
        DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(None);
        let completed = std::cell::Cell::new(false);
        let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::ReadValue);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let result = run_descriptor_role_owner_result_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                check_descriptor_roles_v18(original, optimized, true, budget)?;
                let root = original.source.root(0, budget)?.1;
                let recipe =
                    scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
                let parent_floor = budget.storage();
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    original.with_descriptor_source_roles_v18(
                        optimized,
                        0,
                        &recipe,
                        budget,
                        |roles, budget| -> SourceOwnedResultV18<()> {
                            let required = roles.test_required_storage_v18();
                            assert_eq!(required, budget.storage());
                            assert!(required > parent_floor);
                            if mode == 2 {
                                budget.reserve_storage(size_of::<&'static str>())?;
                            }
                            // Include the newly paid panic payload in the deliberate
                            // undercut. No allocation is hidden by this hostile case.
                            budget.release_storage(budget.storage() - required + 1)?;
                            DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(Some(budget.storage()));
                            match mode {
                                0 => Ok(()),
                                1 => Err(ProductionSourceOwnedViewErrorV18::Binding(
                                    "descriptor callback sentinel",
                                )),
                                _ => {
                                    std::panic::resume_unwind(Box::new("descriptor callback panic"))
                                }
                            }
                        },
                    )
                }));
                assert_eq!(
                    Some(budget.storage()),
                    DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.get()
                );
                assert!(
                    original.source.cleanup.is_denied(),
                    "local ownership loss forbids outer refund"
                );
                let error = match (mode, caught) {
                    (
                        0,
                        Ok(Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Accounting,
                        ))),
                    ) => {
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                    }
                    (
                        1,
                        Ok(Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "descriptor callback sentinel",
                        ))),
                    ) => ProductionSourceOwnedViewErrorV18::Binding("descriptor callback sentinel"),
                    (2, Err(payload)) => {
                        assert_eq!(
                            *payload.downcast::<&'static str>().unwrap(),
                            "descriptor callback panic"
                        );
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "caught descriptor callback panic",
                        )
                    }
                    _ => panic!("descriptor local custody disposition changed"),
                };
                completed.set(true);
                Err(error)
            },
        )
        .0;
        assert!(
            completed.get(),
            "all custody/disposition assertions must run: {result:?}"
        );
        assert!(DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.get().unwrap() > MODULE_FLOOR);
        match (mode, result) {
            (
                0,
                Err(ProductionSourceOptimizationErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting),
                )),
            ) => {}
            (
                1,
                Err(ProductionSourceOptimizationErrorV18::Adoption(
                    fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                        ProductionSourceOwnedViewErrorV18::Binding("descriptor callback sentinel"),
                    ),
                )),
            ) => {}
            (
                2,
                Err(ProductionSourceOptimizationErrorV18::Adoption(
                    fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "caught descriptor callback panic",
                        ),
                    ),
                )),
            ) => {}
            (_, error) => panic!("descriptor outer custody disposition: {error:?}"),
        }
    }
}

#[test]
fn original_descriptor_roles_keep_unclaimed_address_dependencies_and_ordered_access_pending() {
    let entrance = DescriptorRoleEntranceV18::IssuedDisjointSlice;
    let completed = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(
        entrance,
        DescriptorRoleSourceV18::VolatileOnly,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            let root = original.source.root(0, budget)?.1;
            let recipe =
                scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
            original.with_descriptor_source_roles_v18(
                optimized,
                0,
                &recipe,
                budget,
                |roles, budget| {
                    let output = optimized.output_inventory(budget)?;
                    let function =
                        optimized_source_root_function_v18(original, optimized, 0, budget)?;
                    assert_eq!(roles.test_operation_count_v18(), function.operations.len());
                    let (mut addresses, mut data, mut lengths, mut stores) = (0, 0, 0, 0);
                    for operation in &output.operations()[function.operations.clone()] {
                        assert_eq!(roles.role(operation.coordinate, budget)?, None);
                        match operation.operation.kind {
                            OperationKind::GetElementPointer { .. } => addresses += 1,
                            OperationKind::SliceData { .. } => data += 1,
                            OperationKind::SliceLength { .. } => lengths += 1,
                            OperationKind::Store { access, .. } => {
                                assert!(access.volatile);
                                stores += 1;
                            }
                            _ => {}
                        }
                    }
                    // These are real dependencies used by a pending ordered Store,
                    // not a fabricated dead operation that the optimizer may erase.
                    assert!(addresses > 0 && data > 0 && lengths > 0 && stores > 0);
                    completed.set(true);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )
        },
    )
    .0;
    assert!(result.is_ok() && completed.get(), "{result:?}");
}

#[test]
fn original_descriptor_role_volatile_read_keeps_the_existing_scalar_namespace_closed() {
    let entrance = DescriptorRoleEntranceV18::IssuedDisjointSlice;
    let positive =
        descriptor_role_boundary_run_v18(entrance, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
    let entered = std::cell::Cell::new(false);
    let published = std::cell::Cell::new(false);
    let settled = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(
        entrance,
        DescriptorRoleSourceV18::VolatileRead,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            entered.set(true);
            let root = original.source.root(0, budget)?.1;
            let recipe =
                scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
            let capture = [17u128; 256];
            let published = &published;
            let callback = move |_: &CheckedDescriptorSourceRolesV18<'_>,
                                 _: &mut ArgumentBudgetV1<'_>|
                  -> SourceOwnedResultV18<()> {
                std::hint::black_box(capture);
                published.set(true);
                Ok(())
            };
            let headers =
                descriptor_role_outer_headers_v18::<(), ProductionSourceOwnedViewErrorV18>(
                    std::mem::size_of_val(&callback),
                    std::mem::align_of_val(&callback),
                )
                .unwrap();
            let floor = budget.storage();
            let result =
                original.with_descriptor_source_roles_v18(optimized, 0, &recipe, budget, callback);
            assert!(
                matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "scalar leaf physical type or Load changed"
                    ))
                ),
                "{result:?}"
            );
            assert_eq!(budget.storage(), floor);
            assert!(budget.peak_storage() >= floor + headers);
            settled.set(true);
            result
        },
    )
    .0;
    assert!(
        entered.get(),
        "the genuine source/optimizer entrance must succeed: {result:?}"
    );
    assert!(
        !published.get(),
        "an unsupported scalar namespace grants no partial role view"
    );
    assert!(
        settled.get(),
        "the early-refusal envelope must settle before outer cleanup"
    );
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "scalar leaf physical type or Load changed"
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn original_descriptor_role_outer_header_refuses_before_unsupported_namespace() {
    let entrance = DescriptorRoleEntranceV18::IssuedDisjointSlice;
    let positive =
        descriptor_role_boundary_run_v18(entrance, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
    for cut in 0..3 {
        let completed = std::cell::Cell::new(false);
        let published = std::cell::Cell::new(false);
        let result = run_descriptor_roles_v18(entrance,
        DescriptorRoleSourceV18::VolatileRead,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            let root = original.source.root(0, budget)?.1;
            let recipe =
                scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
            optimized.check_exact_original_v18(original, budget)?;
            let capture = [29u128; 256];
            let published = &published;
            let callback = move |_: &CheckedDescriptorSourceRolesV18<'_>,
                                 _: &mut ArgumentBudgetV1<'_>|
                  -> SourceOwnedResultV18<()> {
                std::hint::black_box(capture);
                published.set(true);
                Ok(())
            };
            let headers =
                descriptor_role_outer_headers_v18::<(), ProductionSourceOwnedViewErrorV18>(
                    std::mem::size_of_val(&callback),
                    std::mem::align_of_val(&callback),
                )
                .unwrap();
            // The outer constructor borrows the source and callback while its
            // attempt envelope remains live across the explicit capture debit.
            let attempt = scoped_source_attempt_header_oracle_v29::<
                usize,
                ProductionSourceOwnedViewErrorV18,
                (&ProductionSourceOwnedViewV18<'_>, &()),
            >();
            let floor = budget.storage();
            let limit = budget.storage_limit();
            let remaining = match cut {
                0 => attempt - 1,
                1 => attempt,
                _ => attempt + headers - 1,
            };
            let expected_excess = if cut == 1 { headers } else { 1 };
            let padding = limit.checked_sub(floor + remaining).unwrap();
            budget.reserve_storage(padding).unwrap();
            let padded_floor = budget.storage();
            let refused =
                original.with_descriptor_source_roles_v18(optimized, 0, &recipe, budget, callback);
            let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                error,
            ))) = refused
            else {
                panic!(
                    "outer capture must refuse before the volatile scalar namespace: {refused:?}"
                );
            };
            assert_eq!((error.actual(), error.limit()), (limit + expected_excess, limit));
            assert_eq!(budget.storage(), padded_floor);
            budget.release_storage(padding).unwrap();
            assert_eq!(budget.storage(), floor);
            let before = (budget.work(), budget.storage());
            let repeated = original.check(budget);
            assert!(matches!(repeated, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(repeated))) if repeated.actual() == error.actual() && repeated.limit() == error.limit()));
            assert_eq!((budget.work(), budget.storage()), before);
            let replay: SourceOwnedResultV18<()> = original.with_descriptor_source_roles_v18(
                optimized, 0, &recipe, budget, |_, _| panic!("the first constructor failure forbids retry"),
            );
            assert!(matches!(replay, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(repeated))) if repeated.actual() == error.actual() && repeated.limit() == error.limit()));
            assert_eq!((budget.work(), budget.storage()), before);
            completed.set(true);
            Ok(())
        },
    )
    .0;
        assert!(completed.get() && !published.get(), "{result:?}");
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error))) if error.limit() == MODULE_LIMIT && error.actual() > MODULE_LIMIT),
            "{result:?}"
        );
    }
}

#[test]
fn descriptor_rows_inner_attempt_header_exact_and_short_keep_first_refusal() {
    for short in [false, true] {
        let completed = std::cell::Cell::new(false);
        let result = run_descriptor_roles_v18(
            DescriptorRoleEntranceV18::IssuedDisjointSlice,
            DescriptorRoleSourceV18::Constant,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                let root = original.source.root(0, budget)?.1;
                let recipe =
                    scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
                slice_view_v1::test_descriptor_rows_attempt_boundary_v18(
                    original, optimized, &recipe, short, &completed, budget,
                )
            },
        )
        .0;
        assert!(completed.get(), "{result:?}");
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Storage(_)
            ))
        ));
    }
}

#[test]
fn original_descriptor_roles_require_same_candidate_descriptor_extent_and_guard() {
    let entrance = DescriptorRoleEntranceV18::IssuedDisjointSlice;
    for fault in [
        DescriptorFault::Data,
        DescriptorFault::Extent,
        DescriptorFault::Guard,
    ] {
        let positive = descriptor_role_boundary_run_v18(
            entrance,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(
            positive.0.is_ok() && positive.3,
            "{fault:?}: {:?}",
            positive.0
        );
        let visited = std::cell::Cell::new(false);
        let issued_fault = match fault {
            DescriptorFault::Data => 2,
            DescriptorFault::Extent => 6,
            DescriptorFault::Guard => 5,
            _ => unreachable!(),
        };
        let run = || {
            run_descriptor_roles_v18(
                entrance,
                DescriptorRoleSourceV18::ReadValue,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                |original, optimized, budget| {
                    visited.set(true);
                    check_descriptor_roles_v18(original, optimized, true, budget)
                },
            )
            .0
        };
        let (result, mutated) = match entrance {
            DescriptorRoleEntranceV18::OriginalUniqueBorrow => {
                let _restore = DescriptorObservers::install(fault);
                let result = run();
                assert_eq!(DESCRIPTOR_TAMPERED.get(), 1);
                (result, true)
            }
            DescriptorRoleEntranceV18::IssuedDisjointSlice => {
                source_issued_pointer_source_tests_v29::with_issued_source_fault_v29(
                    issued_fault,
                    run,
                )
            }
        };
        assert!(
            !visited.get(),
            "the changed actual source candidate must refuse before role construction"
        );
        assert!(mutated);
        let expected = match entrance {
            DescriptorRoleEntranceV18::OriginalUniqueBorrow if fault == DescriptorFault::Guard => {
                "execution call parameters differ from their source instance"
            }
            DescriptorRoleEntranceV18::OriginalUniqueBorrow => {
                "source runtime slice descriptor/index/extent correspondence differs"
            }
            DescriptorRoleEntranceV18::IssuedDisjointSlice => {
                "source issued pointer differs from its original issuer or actual guard"
            }
        };
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })))
            if detail == expected),
            "{fault:?}: {result:?}"
        );
    }
}

thread_local! {
    static DESCRIPTOR_ROLE_CHANGED_STORE_V18: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn descriptor_role_changed_store_v18(
    _: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let lowered = emitted.iter_mut().flatten().next().unwrap();
    let body = lowered.function.body.as_mut().unwrap();
    let value = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::Store { value, access, .. }
                if access.address_space == AddressSpace::Global =>
            {
                Some(value)
            }
            _ => None,
        })
        .expect("genuine descriptor Store");
    let producer = body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.results.iter().any(|result| result.id == value))
        .unwrap();
    assert!(matches!(
        producer.kind,
        OperationKind::Constant(Constant::U32(17))
    ));
    producer.kind = OperationKind::Constant(Constant::U32(18));
    DESCRIPTOR_ROLE_CHANGED_STORE_V18.set(DESCRIPTOR_ROLE_CHANGED_STORE_V18.get() + 1);
    Ok(())
}

#[test]
fn original_descriptor_role_write_recipe_rechecks_actual_value_against_original_constant() {
    let entrance = DescriptorRoleEntranceV18::IssuedDisjointSlice;
    let positive = run_descriptor_roles_v18(
        entrance,
        DescriptorRoleSourceV18::Constant,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| check_descriptor_roles_v18(original, optimized, true, budget),
    )
    .0;
    positive.unwrap();
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let _restore =
        Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(descriptor_role_changed_store_v18)));
    DESCRIPTOR_ROLE_CHANGED_STORE_V18.set(0);
    let entered = std::cell::Cell::new(false);
    let completed = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(
        entrance,
        DescriptorRoleSourceV18::Constant,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            entered.set(true);
            check_descriptor_roles_v18(original, optimized, true, budget)?;
            completed.set(true);
            Ok(())
        },
    )
    .0;
    assert!(DESCRIPTOR_ROLE_CHANGED_STORE_V18.get() > 0);
    assert!(!completed.get());
    match entrance {
        DescriptorRoleEntranceV18::OriginalUniqueBorrow => {
            assert!(
                entered.get(),
                "the original source-owned scalar recipe gate must be reached: {result:?}"
            );
            assert!(
                matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "actual scalar expression differs from its original source value"
                    ))
                ),
                "{result:?}"
            );
        }
        DescriptorRoleEntranceV18::IssuedDisjointSlice => {
            assert!(
                !entered.get(),
                "the original issued Store constant is checked before optimized role construction: {result:?}"
            );
            assert!(
                matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Source(
                        ProductionPendingScopedSourceErrorV29::Source(
                            ProductionSemanticKirErrorV1::Unsupported {
                                detail: "source issued pointer differs from its original issuer or actual guard",
                                ..
                            }
                        )
                    ))
                ),
                "{result:?}"
            );
        }
    }
}

#[test]
fn original_descriptor_role_foreign_ledger_refuses_without_debit_or_refund() {
    let entrance = DescriptorRoleEntranceV18::IssuedDisjointSlice;
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(self.0);
        }
    }
    let _restore = Restore(DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.replace(None));
    let completed = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(
        entrance,
        DescriptorRoleSourceV18::ReadValue,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            check_descriptor_roles_v18(original, optimized, true, budget)?;
            let root = original.source.root(0, budget)?.1;
            let recipe =
                scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
            original.with_descriptor_source_roles_v18(
                optimized,
                0,
                &recipe,
                budget,
                |roles, budget| {
                    let output = optimized.output_inventory(budget)?;
                    let function =
                        optimized_source_root_function_v18(original, optimized, 0, budget)?;
                    let coordinate = output.operations()[function.operations.start].coordinate;
                    roles.role(coordinate, budget)?;
                    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
                    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                    let before = (budget.work(), budget.storage());
                    let foreign_before = (foreign.work(), foreign.storage());
                    assert!(matches!(
                        roles.role(coordinate, &mut foreign),
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Accounting
                        ))
                    ));
                    assert_eq!((foreign.work(), foreign.storage()), foreign_before);
                    assert_eq!((budget.work(), budget.storage()), before);
                    assert!(matches!(
                        roles.role(coordinate, budget),
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Accounting
                        ))
                    ));
                    assert_eq!((budget.work(), budget.storage()), before);
                    assert!(original.source.cleanup.is_denied());
                    DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(Some(budget.storage()));
                    completed.set(true);
                    // The authenticated first failure must survive a callback
                    // which ignores the returned foreign-ledger query error.
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )
        },
    )
    .0;
    assert!(completed.get(), "{result:?}");
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert!(DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.get().unwrap() > MODULE_FLOOR);
}
