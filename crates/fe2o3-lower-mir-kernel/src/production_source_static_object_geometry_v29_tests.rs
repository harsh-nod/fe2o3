include!("production_source_static_pointer_cells_v29_tests.rs");

fn scalar_field_layouts_v29() -> Vec<fe2o3_kernel_ir::StorageLayoutV1> {
    use fe2o3_kernel_ir::{
        StorageFieldV1, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind, StorageLayoutV1,
    };
    vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: Kind::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: Kind::Record(
                vec![
                    StorageFieldV1 {
                        offset: 0,
                        layout: Id(0),
                    },
                    StorageFieldV1 {
                        offset: 4,
                        layout: Id(0),
                    },
                ]
                .into(),
            ),
        },
    ]
}

fn scalar_field_equations_v29(
    sibling: bool,
) -> (
    Function,
    Vec<ScopedSourceSlotV29>,
    Vec<SourceAddressAccessV29>,
) {
    use fe2o3_kernel_ir::StorageLayoutIdV1 as Id;
    let (mut function, mut slots, _) = typed_currentness_fixture();
    slots.truncate(1);
    slots[0].origin.source = ScopedAllocationSourceV29::OriginalObject {
        cell: 0,
        schema: Id(1),
    };
    slots[0].representation = ScopedSlotRepresentationV29::Object {
        schema: Id(1),
        bytes: 8,
        alignment: 4,
    };
    let mut root = allocation(A, Type::StorageObject(Id(1)));
    let OperationKind::Alloca { alignment, .. } = &mut root.kind else {
        unreachable!()
    };
    *alignment = 4;
    let project = |result, field| {
        Operation::effect_free(
            ValueDef::new(
                result,
                Type::pointer(
                    Type::StorageObject(Id(0)),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Storage(ScopedObjectOperationV29::Project {
                base: A,
                step: ScopedObjectProjectionV29::Field(field),
            }),
        )
    };
    function.body.as_mut().unwrap().blocks[0].operations = vec![
        root,
        project(EXPOSED_A, 0),
        project(EXPOSED_B, 1),
        Operation::new(
            vec![],
            OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                address: EXPOSED_A,
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(LOADED, Type::Scalar(ScalarType::U32)),
            OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
                address: if sibling { EXPOSED_B } else { EXPOSED_A },
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
    ];
    (
        function,
        slots,
        vec![
            SourceAddressAccessV29 {
                footprint: 0,
                block: BlockId(77),
                operation: 3,
                slot: 0,
            },
            SourceAddressAccessV29 {
                footprint: 0,
                block: BlockId(77),
                operation: 4,
                slot: 0,
            },
        ],
    )
}

fn run_scalar_field_history_v29(
    function: &Function,
    slots: &[ScopedSourceSlotV29],
    accesses: &[SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize, bool) {
    let layouts = scalar_field_layouts_v29();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut completed = false;
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_with_layouts(
            function, slots, None, accesses, &layouts, budget,
        )?
        .solve(slots, accesses, kills, budget)?;
        assert_eq!(
            graph.object_location(EXPOSED_A, budget)?,
            SourceStaticObjectLocationV29 {
                slot: 0,
                offset: 0,
                schema: Some(fe2o3_kernel_ir::StorageLayoutIdV1(0))
            }
        );
        assert_eq!(
            graph.object_location(EXPOSED_B, budget)?,
            SourceStaticObjectLocationV29 {
                slot: 0,
                offset: 4,
                schema: Some(fe2o3_kernel_ir::StorageLayoutIdV1(0))
            }
        );
        scoped_slot_uses_v29::check_expanded_scalar_addresses_v29(
            function, &graph, slots, accesses, kills, budget,
        )?;
        completed = true;
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn static_field_equations_keep_sibling_initialization_and_slot_kills_distinct() {
    let (function, slots, accesses) = scalar_field_equations_v29(false);
    let (positive, _, _, completed) =
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], LIMIT, LIMIT);
    positive.unwrap();
    assert!(completed);
    let (sibling, slots, accesses) = scalar_field_equations_v29(true);
    unsupported(run_scalar_field_history_v29(&sibling, &slots, &accesses, &[], LIMIT, LIMIT).0);
    let killed = [SourceAddressKillV29 {
        source_order: [0; 5],
        block: BlockId(77),
        gap: 4,
        slot: 0,
    }];
    unsupported(
        run_scalar_field_history_v29(&function, &slots, &accesses, &killed, LIMIT, LIMIT).0,
    );
    let mut early_read = function.clone();
    early_read.body.as_mut().unwrap().blocks[0]
        .operations
        .swap(3, 4);
    unsupported(run_scalar_field_history_v29(&early_read, &slots, &accesses, &[], LIMIT, LIMIT).0);
}

#[test]
fn optimized_project_equations_reject_displaced_duplicate_and_missing_subobjects() {
    use scoped_raw_admission_v29::{
        test_finish_optimized_project_equations_v33 as finish_optimized_source_project_equations_v33,
        test_optimized_project_equation_v33 as check_optimized_source_project_equation_v33,
        test_optimized_project_headers_v33 as optimized_source_project_headers_v33,
    };
    let (function, slots, accesses) = scalar_field_equations_v29(false);
    let layouts = scalar_field_layouts_v29();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_with_layouts(
            &function, &slots, None, &accesses, &layouts, budget,
        )?
        .solve(&slots, &accesses, &[], budget)?;
        budget.reserve_storage(optimized_source_project_headers_v33().unwrap())?;
        let mut seen = emission_vec_v1(graph.projections.len(), budget)?;
        budget.charge_work(graph.projections.len())?;
        seen.resize(graph.projections.len(), false);
        let source = graph.object_location(A, budget)?;
        let left = graph.object_location(EXPOSED_A, budget)?;
        let right = graph.object_location(EXPOSED_B, budget)?;
        assert_eq!(seen.len(), 2);
        for changed in [
            SourceStaticObjectLocationV29 {
                slot: source.slot + 1,
                ..source
            },
            SourceStaticObjectLocationV29 {
                offset: source.offset + 4,
                ..source
            },
            SourceStaticObjectLocationV29 {
                schema: None,
                ..source
            },
        ] {
            assert!(matches!(
                check_optimized_source_project_equation_v33(
                    changed, left, &graph, A, EXPOSED_A, &mut seen, budget,
                ),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized Project changed source subobject geometry"
                ))
            ));
        }
        assert!(matches!(
            check_optimized_source_project_equation_v33(
                source, left, &graph, A, EXPOSED_B, &mut seen, budget,
            ),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized Project changed source subobject geometry"
            ))
        ));
        check_optimized_source_project_equation_v33(
            source, left, &graph, A, EXPOSED_A, &mut seen, budget,
        )
        .unwrap();
        assert!(matches!(
            finish_optimized_source_project_equations_v33(&seen, budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized Project lacks checked original source role"
            ))
        ));
        assert!(matches!(
            check_optimized_source_project_equation_v33(
                source, left, &graph, A, EXPOSED_A, &mut seen, budget,
            ),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized Project repeats actual pointer equation"
            ))
        ));
        check_optimized_source_project_equation_v33(
            source, right, &graph, A, EXPOSED_B, &mut seen, budget,
        )
        .unwrap();
        finish_optimized_source_project_equations_v33(&seen, budget).unwrap();
        // This checks actual solved geometry, not source custody or admission.
        // The existing two-root aggregate chain test exercises that consumer.
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn typed_project_checks_stale_unused_base_at_its_execution_point() {
    let (mut function, slots, _) = scalar_field_equations_v29(false);
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    let root = operations[0].clone();
    let mut project = operations[1].clone();
    let alias = ValueId(999);
    let OperationKind::Storage(ScopedObjectOperationV29::Project { base, .. }) = &mut project.kind
    else {
        unreachable!()
    };
    *base = alias;
    *operations = vec![
        root.clone(),
        Operation::effect_free(
            ValueDef::new(ValueId(40), Type::BOOL),
            OperationKind::Constant(Constant::Bool(true)),
        ),
        Operation::effect_free(
            ValueDef::new(alias, root.results[0].ty.clone()),
            OperationKind::Select {
                condition: ValueId(40),
                true_value: A,
                false_value: A,
            },
        ),
        project,
    ];
    let layouts = scalar_field_layouts_v29();
    for state in [None, Some(false), Some(true)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let lifetimes: Vec<_> = state
            .into_iter()
            .map(|live| SourceAddressLifetimeV29 {
                block: BlockId(77),
                gap: 3,
                sequence: 0,
                slot: 0,
                live,
            })
            .collect();
        let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
            let graph = SourceAddressMemoryV29::prepare_with_layouts(
                &function,
                &slots,
                None,
                &[],
                &layouts,
                budget,
            )?
            .solve(&slots, &[], &[], budget)?;
            check_source_address_currentness_v29(
                &function,
                &graph,
                &slots,
                &[],
                &[],
                &[true],
                &lifetimes,
                &[],
                budget,
            )
        });
        match state {
            None => result.unwrap(),
            Some(_) => assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "physical raw access crosses a storage activation or unresolved alias",
                    ..
                })
            )),
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn typed_project_check_census_has_independent_capacity_and_overflow_boundaries() {
    // Three memory accesses and two zero-footprint Projects each require a
    // base-alias and object check; four failure points require one each.
    assert_eq!(source_address_currentness_check_count_v34(3, 2, 4), Ok(14));
    assert_eq!(source_address_currentness_check_count_v34(0, 2, 0), Ok(4));
    assert_eq!(source_address_currentness_check_count_v34(0, 0, 0), Ok(0));
    let largest_pairs = usize::MAX / 2;
    assert_eq!(
        source_address_currentness_check_count_v34(0, largest_pairs, 1),
        Ok(usize::MAX)
    );
    for (accesses, projects, failures) in [
        (usize::MAX, 0, 0),
        (0, usize::MAX, 0),
        (0, largest_pairs, 2),
        (largest_pairs, 1, 0),
    ] {
        assert_eq!(
            source_address_currentness_check_count_v34(accesses, projects, failures),
            Err(ArgumentResourceV1::Arithmetic)
        );
    }
    // Only four paid usize rows are needed for the two-Project census, with
    // no hidden sentinel. The bounded allocator's exact/one-short boundary is
    // independent of a successful run's observed peak.
    let bytes = 4 * std::mem::size_of::<usize>();
    for storage in [bytes, bytes - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage);
        let result = emission_vec_v1::<usize>(
            source_address_currentness_check_count_v34(0, 2, 0).unwrap(),
            &mut budget,
        );
        if storage == bytes {
            assert_eq!(result.unwrap().capacity(), 4);
            assert_eq!(budget.storage(), bytes);
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            ));
            assert_eq!(budget.storage(), 0);
        }
    }
}

#[test]
fn typed_project_backing_reuse_is_current_after_explicit_reactivation() {
    let (mut function, slots, _) = scalar_field_equations_v29(false);
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .truncate(3);
    let layouts = scalar_field_layouts_v29();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_with_layouts(
            &function,
            &slots,
            None,
            &[],
            &layouts,
            budget,
        )?
        .solve(&slots, &[], &[], budget)?;
        assert_eq!(graph.projections.len(), 2);
        check_source_address_currentness_v29(
            &function,
            &graph,
            &slots,
            &[],
            &[],
            &[true],
            &[SourceAddressLifetimeV29 {
                block: BlockId(77),
                gap: 1,
                sequence: 0,
                slot: 0,
                live: true,
            }],
            &[],
            budget,
        )
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn static_field_equations_join_all_reachable_predecessor_histories() {
    for second_write in [false, true] {
        let (mut function, slots, _) = scalar_field_equations_v29(false);
        let entry = &mut function.body.as_mut().unwrap().blocks[0];
        let read = entry.operations.pop().unwrap();
        let write = entry.operations.pop().unwrap();
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(78),
            then_arguments: vec![],
            else_target: BlockId(79),
            else_arguments: vec![],
        });
        let mut left = block(78);
        left.operations.push(write.clone());
        left.terminator = Some(Terminator::Branch {
            target: BlockId(80),
            arguments: vec![],
        });
        let mut right = block(79);
        if second_write {
            right.operations.push(write);
        }
        right.terminator = Some(Terminator::Branch {
            target: BlockId(80),
            arguments: vec![],
        });
        let mut join = block(80);
        join.operations.push(read);
        function
            .body
            .as_mut()
            .unwrap()
            .blocks
            .extend([left, right, join]);
        let mut accesses = vec![SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(78),
            operation: 0,
            slot: 0,
        }];
        if second_write {
            accesses.push(SourceAddressAccessV29 {
                footprint: 0,
                block: BlockId(79),
                operation: 0,
                slot: 0,
            });
        }
        accesses.push(SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(80),
            operation: 0,
            slot: 0,
        });
        let (result, _, _, completed) =
            run_scalar_field_history_v29(&function, &slots, &accesses, &[], LIMIT, LIMIT);
        if second_write {
            result.unwrap();
            assert!(completed);
        } else {
            unsupported(result);
            assert!(!completed);
        }
    }
}

#[test]
fn static_field_solver_and_history_have_exact_measured_resource_boundaries() {
    let (function, slots, accesses) = scalar_field_equations_v29(false);
    let (result, work, storage, completed) =
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], LIMIT, LIMIT);
    result.unwrap();
    assert!(completed);
    let (result, _, _, completed) =
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], work, storage);
    result.unwrap();
    assert!(completed);
    assert!(matches!(
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], work - 1, storage).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert!(matches!(
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], work, storage - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}

fn object_write_equations_v48(
    rights: AccessMode,
) -> (
    Function,
    Vec<ScopedSourceSlotV29>,
    Vec<SourceAddressAccessV29>,
) {
    let (mut function, slots, mut accesses) = scalar_field_equations_v29(false);
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    let Type::Pointer(pointer) = &mut operations[1].results[0].ty else {
        unreachable!()
    };
    pointer.access = rights;
    operations.truncate(4);
    accesses.truncate(1);
    (function, slots, accesses)
}

#[test]
fn object_write_only_solver_admits_payload_writes_without_read_authority() {
    for rights in [AccessMode::ReadWrite, AccessMode::WriteOnly] {
        let (function, slots, accesses) = object_write_equations_v48(rights);
        let (result, _, _, completed) =
            run_scalar_field_history_v29(&function, &slots, &accesses, &[], LIMIT, LIMIT);
        result.unwrap();
        assert!(completed);
    }
    let (read_only, slots, accesses) = object_write_equations_v48(AccessMode::ReadOnly);
    let (result, _, _, completed) =
        run_scalar_field_history_v29(&read_only, &slots, &accesses, &[], LIMIT, LIMIT);
    unsupported(result);
    assert!(!completed);

    let (mut unreadable, slots, accesses) = scalar_field_equations_v29(false);
    let Type::Pointer(pointer) =
        &mut unreadable.body.as_mut().unwrap().blocks[0].operations[1].results[0].ty
    else {
        unreachable!()
    };
    pointer.access = AccessMode::WriteOnly;
    let (result, _, _, completed) =
        run_scalar_field_history_v29(&unreadable, &slots, &accesses, &[], LIMIT, LIMIT);
    unsupported(result);
    assert!(!completed);
}

#[test]
fn object_write_only_solver_retains_geometry_and_access_census_checks() {
    let (function, slots, accesses) = object_write_equations_v48(AccessMode::WriteOnly);
    run_scalar_field_history_v29(&function, &slots, &accesses, &[], LIMIT, LIMIT)
        .0
        .unwrap();
    for wrong_space in [AddressSpace::Global, AddressSpace::Generic] {
        let mut changed = function.clone();
        let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { access, .. }) =
            &mut changed.body.as_mut().unwrap().blocks[0].operations[3].kind
        else {
            unreachable!()
        };
        access.address_space = wrong_space;
        unsupported(run_scalar_field_history_v29(&changed, &slots, &accesses, &[], LIMIT, LIMIT).0);
    }
    for alignment in [0, 3, 8] {
        let mut changed = function.clone();
        let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { access, .. }) =
            &mut changed.body.as_mut().unwrap().blocks[0].operations[3].kind
        else {
            unreachable!()
        };
        access.alignment = alignment;
        unsupported(run_scalar_field_history_v29(&changed, &slots, &accesses, &[], LIMIT, LIMIT).0);
    }
    unsupported(run_scalar_field_history_v29(&function, &slots, &[], &[], LIMIT, LIMIT).0);
    let mut wrong_value = function.clone();
    let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { value, .. }) =
        &mut wrong_value.body.as_mut().unwrap().blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    *value = ValueId(0);
    unsupported(run_scalar_field_history_v29(&wrong_value, &slots, &accesses, &[], LIMIT, LIMIT).0);
}

#[test]
fn object_write_only_solver_has_exact_and_one_short_resources() {
    let (function, slots, accesses) = object_write_equations_v48(AccessMode::WriteOnly);
    let (result, work, storage, completed) =
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], LIMIT, LIMIT);
    result.unwrap();
    assert!(completed);
    let (result, used, peak, completed) =
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], work, storage);
    result.unwrap();
    assert!(completed);
    assert_eq!((used, peak), (work, storage));
    let (result, _, _, completed) =
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], work - 1, storage);
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                limit
            ))
        ) if limit.limit() == work - 1 && limit.actual() == work
    ));
    assert!(!completed);
    let (result, _, _, completed) =
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], work, storage - 1);
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(limit)
            )
        ) if limit.limit() == storage - 1 && limit.actual() == storage
    ));
    assert!(!completed);
}

#[test]
fn object_write_only_solver_does_not_widen_ordinary_pointer_access() {
    for rights in [
        AccessMode::ReadOnly,
        AccessMode::ReadWrite,
        AccessMode::WriteOnly,
    ] {
        for writing in [false, true] {
            let mut entry = block(77);
            entry.operations.push(if writing {
                store(ValueId(0), ValueId(1), AddressSpace::Generic)
            } else {
                Operation::effect_free(
                    ValueDef::new(LOADED, Type::Scalar(ScalarType::U32)),
                    OperationKind::Load {
                        pointer: ValueId(0),
                        access: MemoryAccess::new(AddressSpace::Generic, 4),
                    },
                )
            });
            let function = Function::internal_helper(
                "external-scalar-access-equations-only",
                Signature::new(
                    vec![
                        Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Generic, rights),
                        Type::Scalar(ScalarType::U32),
                    ],
                    vec![],
                ),
                vec![ValueId(0), ValueId(1)],
                vec![entry],
            );
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
                SourceAddressMemoryV29::prepare(&function, &[], None, &[], budget)?.solve(
                    &[],
                    &[],
                    &[],
                    budget,
                )?;
                Ok(())
            });
            assert_eq!(budget.storage(), FLOOR);
            let allowed = if writing {
                rights == AccessMode::ReadWrite
            } else {
                rights != AccessMode::WriteOnly
            };
            if allowed {
                result.unwrap();
            } else {
                unsupported(result);
            }
        }
    }
}

#[test]
fn static_field_transfer_has_independent_constant_work_and_no_storage() {
    use fe2o3_kernel_ir::StorageLayoutIdV1 as Id;
    let layouts = scalar_field_layouts_v29();
    for limit in [7, 6] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = source_static_object_transfer_v29(
            &layouts,
            Id(1),
            ScopedObjectProjectionV29::Field(1),
            &mut budget,
        );
        if limit == 7 {
            assert_eq!(
                result.unwrap(),
                SourceStaticObjectTransferV29::Project {
                    parent: Id(1),
                    child: Id(0),
                    offset: 4,
                    parent_bytes: 8,
                    child_bytes: 4
                }
            );
            assert_eq!(budget.work(), 7);
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        }
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), 0);
    }
    let (_, slots, _) = scalar_field_equations_v29(false);
    let transfer = SourceStaticObjectTransferV29::Project {
        parent: Id(1),
        child: Id(0),
        offset: 4,
        parent_bytes: 8,
        child_bytes: 4,
    };
    for limit in [6, 5] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = source_static_object_apply_v29(
            transfer,
            Some(SourceStaticObjectLocationV29 {
                slot: 0,
                offset: 0,
                schema: Some(Id(1)),
            }),
            &slots,
            &mut budget,
        );
        if limit == 6 {
            assert_eq!(
                result.unwrap(),
                Some(SourceStaticObjectLocationV29 {
                    slot: 0,
                    offset: 4,
                    schema: Some(Id(0))
                })
            );
            assert_eq!(budget.work(), 6);
        } else {
            assert!(matches!(
                result,
                Err(origin_worklist_v1::OriginWorkErrorV1::Resource(
                    ArgumentResourceV1::Work(_)
                ))
            ));
        }
        assert_eq!(budget.peak_storage(), 0);
    }
}

#[test]
fn static_field_geometry_refuses_unknown_schema_nested_pointer_and_array_steps() {
    use fe2o3_kernel_ir::{StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind};
    let mut layouts = scalar_field_layouts_v29();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    for (schema, step) in [
        (Id(99), ScopedObjectProjectionV29::Field(0)),
        (Id(1), ScopedObjectProjectionV29::Field(2)),
        (Id(1), ScopedObjectProjectionV29::ArrayIndex(ValueId(0))),
    ] {
        unsupported(
            source_static_object_transfer_v29(&layouts, schema, step, &mut budget).map(|_| ()),
        );
    }
    let Kind::Record(fields) = &mut layouts[1].kind else {
        unreachable!()
    };
    fields[0].layout = Id(1);
    unsupported(
        source_static_object_transfer_v29(
            &layouts,
            Id(1),
            ScopedObjectProjectionV29::Field(0),
            &mut budget,
        )
        .map(|_| ()),
    );
    let (_, _, pointer_layouts) = typed_currentness_fixture();
    layouts.push(pointer_layouts[1].clone());
    let Kind::Record(fields) = &mut layouts[1].kind else {
        unreachable!()
    };
    fields[0].layout = Id(2);
    fields[0].offset = 4;
    unsupported(
        source_static_object_transfer_v29(
            &layouts,
            Id(1),
            ScopedObjectProjectionV29::Field(0),
            &mut budget,
        )
        .map(|_| ()),
    );
    let (_, slots, _) = scalar_field_equations_v29(false);
    let transfer = SourceStaticObjectTransferV29::Project {
        parent: Id(1),
        child: Id(0),
        offset: 4,
        parent_bytes: 8,
        child_bytes: 4,
    };
    for location in [
        None,
        Some(SourceStaticObjectLocationV29 {
            slot: 0,
            offset: 0,
            schema: Some(Id(0)),
        }),
        Some(SourceStaticObjectLocationV29 {
            slot: 0,
            offset: 4,
            schema: Some(Id(1)),
        }),
        Some(SourceStaticObjectLocationV29 {
            slot: 1,
            offset: 0,
            schema: Some(Id(1)),
        }),
    ] {
        assert!(matches!(
            source_static_object_apply_v29(transfer, location, &slots, &mut budget),
            Err(origin_worklist_v1::OriginWorkErrorV1::Shape)
        ));
    }
    assert_eq!(budget.peak_storage(), 0);
}

#[test]
fn static_pointer_field_geometry_preserves_exact_child_and_parent_bounds() {
    use fe2o3_kernel_ir::{
        StorageFieldV1, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind, StorageLayoutV1,
        StoragePointerV1,
    };
    for bits in [32u16, 64] {
        for access in [AccessMode::ReadOnly, AccessMode::ReadWrite] {
            let width = u64::from(bits / 8);
            let mut layouts = vec![
                StorageLayoutV1 {
                    size: 4,
                    alignment: 4,
                    kind: Kind::Scalar(ScalarType::U32),
                },
                StorageLayoutV1 {
                    size: width,
                    alignment: u32::from(bits / 8),
                    kind: Kind::Pointer(StoragePointerV1 {
                        pointee: Id(0),
                        value_space: AddressSpace::Private,
                        encoded_space: AddressSpace::Generic,
                        access,
                        stored_bits: bits,
                    }),
                },
                StorageLayoutV1 {
                    size: 2 * width,
                    alignment: u32::from(bits / 8),
                    kind: Kind::Record(
                        vec![StorageFieldV1 {
                            offset: width,
                            layout: Id(1),
                        }]
                        .into(),
                    ),
                },
            ];
            for limit in [7, 6] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = ArgumentBudgetV1::new(&mut work, 0);
                let result = source_static_object_transfer_v29(
                    &layouts,
                    Id(2),
                    ScopedObjectProjectionV29::Field(0),
                    &mut budget,
                );
                if limit == 7 {
                    assert_eq!(
                        result.unwrap(),
                        SourceStaticObjectTransferV29::Project {
                            parent: Id(2),
                            child: Id(1),
                            offset: width,
                            parent_bytes: 2 * width,
                            child_bytes: width,
                        }
                    );
                    assert_eq!(budget.work(), 7);
                } else {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ));
                }
                assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let Kind::Record(fields) = &mut layouts[2].kind else {
                unreachable!()
            };
            fields[0].offset += 1;
            unsupported(
                source_static_object_transfer_v29(
                    &layouts,
                    Id(2),
                    ScopedObjectProjectionV29::Field(0),
                    &mut budget,
                )
                .map(|_| ()),
            );
            unsupported(
                source_static_object_transfer_v29(
                    &layouts,
                    Id(2),
                    ScopedObjectProjectionV29::Field(1),
                    &mut budget,
                )
                .map(|_| ()),
            );
            unsupported(
                source_static_object_transfer_v29(
                    &layouts,
                    Id(1),
                    ScopedObjectProjectionV29::Field(0),
                    &mut budget,
                )
                .map(|_| ()),
            );
            unsupported(
                source_static_object_transfer_v29(
                    &layouts,
                    Id(2),
                    ScopedObjectProjectionV29::Variant {
                        index: 0,
                        access: MemoryAccess::new(AddressSpace::Private, 1),
                    },
                    &mut budget,
                )
                .map(|_| ()),
            );
        }
    }
}
