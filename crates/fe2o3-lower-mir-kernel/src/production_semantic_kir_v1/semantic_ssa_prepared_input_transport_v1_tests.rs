const PREPARED_TEST_LIMIT: usize = 1_000_000;

fn with_original_prepared_input(
    body: impl FnOnce(
        &mut ExecutionAvailabilityV29<'_>,
        &ExecutionAvailabilityV29<'_>,
        &[ValueId],
        &[Type],
        &[Option<SemanticValueBindingV1>],
        &mut ArgumentBudgetV1<'_>,
    ),
) -> Option<usize> {
    with_prepared_owner(entry_owner(EntryShape::ScalarCycle), body)
}

fn with_prepared_owner(
    mut owner: ProductionSemanticSsaOwnerV1,
    body: impl FnOnce(
        &mut ExecutionAvailabilityV29<'_>,
        &ExecutionAvailabilityV29<'_>,
        &[ValueId],
        &[Type],
        &[Option<SemanticValueBindingV1>],
        &mut ArgumentBudgetV1<'_>,
    ),
) -> Option<usize> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(PREPARED_TEST_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, PREPARED_TEST_LIMIT);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        SemanticFunctionIdV1::from_index(0),
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            with_execution_availability_v29(
                instances,
                instances.root(),
                budget,
                |mut cursor, budget| {
                    let values = [ValueId(101)];
                    let types = [Type::Scalar(ScalarType::U32)];
                    let declarations = [(0, 1, SemanticTypeIdV1::from_index(1))];
                    let installed = [PlannedParameterLocalBindingV1::Direct {
                        local: 1,
                        value: values[0],
                        ty: types[0].clone(),
                    }];
                    let parameters = SemanticParameterBindingsV1 {
                        declarations: &declarations,
                        values: &values,
                        types: &types,
                        local_bindings: Some(&installed),
                    };
                    cursor.invocation_inputs = Some(invocation_root_inputs_v1(
                        cursor.function,
                        &parameters,
                        budget,
                    )?);
                    let locals = [
                        None,
                        Some(SemanticValueBindingV1::Value {
                            id: values[0],
                            ty: types[0].clone(),
                        }),
                    ];
                    with_execution_availability_v29(
                        instances,
                        instances.root(),
                        budget,
                        |other, budget| {
                            body(&mut cursor, &other, &values, &types, &locals, budget);
                            Ok(())
                        },
                    )?;
                    Ok(())
                },
            )
            .unwrap();
            // The cursor and its test installation map are both gone here.
            budget.release_storage(budget.storage() - floor).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), capture.retained_storage());
    drop(budget);
    work.failed_work()
}

#[test]
fn self_copy_cycle_needs_the_original_input_anchor_not_an_alias_guard_exception() {
    let original = entry_owner(EntryShape::ScalarCycle);
    let old = &original.source_semantic().functions()[0];
    let source = SemanticSourceProvenanceV1::unavailable();
    let ty = SemanticTypeIdV1::from_index(1);
    let place = || SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], ty).unwrap();
    let mut blocks = old.blocks().to_vec();
    blocks[0] = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([40; 32]),
        source,
        vec![SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(),
                SemanticRvalueV1::new(
                    ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place())),
                ),
            )),
        )],
        old.blocks()[0].terminator().clone(),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([20; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([21; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([22; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([23; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([24; 32]),
        source,
        old.abi().clone(),
        old.locals().to_vec(),
        old.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(old.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        original.source_semantic().types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    with_prepared_owner(owner, |cursor, _, values, types, locals, budget| {
        let source_types = cursor.cfg.types;
        let promoted = BTreeSet::from([1]);
        let dominance = SemanticOptionDominanceV1::analyze(cursor.function, &[]).unwrap();
        let mut origins = SemanticCapabilityOriginResolverV1::new(
            source_types,
            &[],
            cursor.function,
            &dominance,
            &promoted,
            10000,
            10000,
        )
        .unwrap();
        assert!(matches!(
            promoted_transport_descriptor_v1(
                source_types,
                cursor.function,
                1,
                &BTreeMap::new(),
                &promoted,
                &mut origins,
                &BTreeMap::new(),
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "transparent-borrow SSA transport is cyclic or too deep",
                ..
            })
        ));
        let mut origins = SemanticCapabilityOriginResolverV1::new(
            source_types,
            &[],
            cursor.function,
            &dominance,
            &promoted,
            10000,
            10000,
        )
        .unwrap();
        with_prepared_input_transport_v1(
            Some(cursor),
            cursor.function,
            cursor.ssa,
            values,
            types,
            locals,
            Some(budget),
            |prepared, budget| {
                assert_eq!(
                    promoted_transport_descriptor_with_inputs_v1(
                        source_types,
                        cursor.function,
                        1,
                        &BTreeMap::new(),
                        &promoted,
                        &mut origins,
                        &BTreeMap::new(),
                        prepared,
                        budget,
                    )?,
                    (
                        ty,
                        SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary)
                    )
                );
                Ok(())
            },
        )
        .unwrap();
    });
}

#[test]
fn prepared_input_representation_is_bound_to_the_original_unentered_cursor() {
    with_original_prepared_input(|cursor, other, values, types, locals, budget| {
        let before = (
            cursor.current.clone(),
            cursor.visited.clone(),
            cursor.claimed.clone(),
        );
        let floor = budget.storage();
        with_prepared_input_transport_v1(
            Some(cursor),
            cursor.function,
            cursor.ssa,
            values,
            types,
            locals,
            Some(budget),
            |prepared, budget| {
                let prepared = prepared.unwrap();
                let budget = budget.unwrap();
                prepared.check(cursor, cursor.function, cursor.ssa, budget)?;
                assert_eq!(cursor.instance, other.instance);
                assert!(std::ptr::eq(cursor.function, other.function));
                assert!(
                    prepared
                        .check(other, other.function, other.ssa, budget)
                        .is_err()
                );
                let mut other_work = CanonicalKernelIrWorkBudgetV1::new(100);
                let mut other_budget = ArgumentBudgetV1::new(&mut other_work, 100);
                assert!(
                    prepared
                        .check(cursor, cursor.function, cursor.ssa, &mut other_budget)
                        .is_err()
                );
                assert_eq!(other_budget.work(), 0);
                assert_eq!(
                    prepared.ordinary(1, budget)?,
                    Some(SemanticTypeIdV1::from_index(1))
                );
                assert_eq!(prepared.ordinary(0, budget)?, None);
                let foreign_owner = entry_owner(EntryShape::ScalarCycle);
                let (foreign_function, foreign_ssa) = original_entry(&foreign_owner);
                assert!(
                    prepared
                        .check(cursor, foreign_function, foreign_ssa, budget)
                        .is_err()
                );
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            (
                cursor.current.clone(),
                cursor.visited.clone(),
                cursor.claimed.clone()
            ),
            before
        );
        cursor.visited[0] = true;
        let rejected: Result<(), _> = with_prepared_input_transport_v1(
            Some(cursor),
            cursor.function,
            cursor.ssa,
            values,
            types,
            locals,
            Some(budget),
            |_, _| panic!("an entered cursor must not install a representation anchor"),
        );
        assert!(rejected.is_err());
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn prepared_input_checks_exact_original_ordinals_types_and_components() {
    for corruption in 0..10 {
        with_original_prepared_input(|cursor, _, values, types, locals, budget| {
            let mut values = values.to_vec();
            let mut types = types.to_vec();
            let mut locals = locals.to_vec();
            match corruption {
                0 => values[0] = ValueId(102),
                1 => types[0] = Type::Scalar(ScalarType::U64),
                2 => locals[1] = None,
                3 => cursor.invocation_inputs.as_mut().unwrap()[0].first_parameter = 1,
                4 => cursor.invocation_inputs.as_mut().unwrap()[0].source_argument = 1,
                5 => {
                    let row = cursor.invocation_inputs.as_ref().unwrap()[0];
                    cursor.invocation_inputs.as_mut().unwrap().push(row);
                }
                6 => cursor.invocation_inputs.as_mut().unwrap().clear(),
                7 => cursor.invocation_inputs.as_mut().unwrap()[0].local = 0,
                8 => {
                    cursor.invocation_inputs.as_mut().unwrap()[0].ty =
                        SemanticTypeIdV1::from_index(0)
                }
                9 => cursor.invocation_inputs.as_mut().unwrap()[0].parameter_count = 0,
                _ => unreachable!(),
            }
            let floor = budget.storage();
            let rejected = with_prepared_input_transport_v1(
                Some(cursor),
                cursor.function,
                cursor.ssa,
                &values,
                &types,
                &locals,
                Some(budget),
                |prepared, budget| prepared.unwrap().ordinary(1, budget.unwrap()).map(|_| ()),
            );
            assert!(rejected.is_err(), "corruption {corruption}");
            assert_eq!(budget.storage(), floor);
        });
    }
}

fn independent_prepared_header() -> usize {
    use std::mem::size_of;
    size_of::<&ExecutionAvailabilityV29<'_>>()
        + size_of::<&[ValueId]>()
        + size_of::<&[Type]>()
        + size_of::<&[Option<SemanticValueBindingV1>]>()
        + size_of::<usize>() // Original concrete ledger slot retained by the view.
        // Borrowed original-root view, captured owned count and table count.
        + size_of::<Option<(&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>, usize, usize)>>()
}

#[test]
fn prepared_input_has_independent_exact_and_one_short_resource_boundaries() {
    let header = independent_prepared_header();
    assert_eq!(
        header,
        std::mem::size_of::<PreparedInputTransportV1<'_, '_>>()
            + std::mem::size_of::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>(
            )
    );
    // One source check, two original block flags, input-map work 3 + 2L + 5R,
    // Then row + shape + binding + component, plus both scalar Type trees:
    // invocation_equal_types_v1 prepays left and right independently.
    let required_work = 1 + 2 + (3 + 2 * 2 + 5) + (4 + 2);
    let scratch = std::mem::size_of::<Vec<bool>>() + 2 * std::mem::size_of::<bool>();
    let bytes = header + scratch;
    for (short_work, short_storage) in [(false, false), (true, false), (false, true)] {
        let failed = with_original_prepared_input(|cursor, _, values, types, locals, budget| {
            assert_eq!(cursor.visited.len(), 2);
            assert_eq!(cursor.function.locals().len(), 2);
            assert_eq!(cursor.invocation_inputs.as_ref().unwrap().len(), 1);
            let floor = PREPARED_TEST_LIMIT - bytes + usize::from(short_storage);
            let setup_floor = budget.storage();
            budget.reserve_storage(floor - setup_floor).unwrap();
            let prior_work = PREPARED_TEST_LIMIT - required_work + usize::from(short_work);
            budget.charge_work(prior_work - budget.work()).unwrap();
            let result = with_prepared_input_transport_v1(
                Some(cursor),
                cursor.function,
                cursor.ssa,
                values,
                types,
                locals,
                Some(budget),
                |prepared, budget| prepared.unwrap().ordinary(1, budget.unwrap()).map(|_| ()),
            );
            assert_eq!(result.is_ok(), !short_work && !short_storage);
            assert_eq!(budget.storage(), floor);
            if short_storage {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_)
                        )
                    )
                ));
                assert_eq!(budget.failed_storage(), Some(floor + bytes));
                assert_eq!(
                    budget.peak_storage(),
                    floor + header + std::mem::size_of::<Vec<bool>>()
                );
            } else {
                assert_eq!(budget.peak_storage(), floor + bytes);
                assert_eq!(budget.failed_storage(), None);
                assert_eq!(budget.work(), PREPARED_TEST_LIMIT);
                if short_work {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ));
                }
            }
            budget.release_storage(floor - setup_floor).unwrap();
        });
        assert_eq!(failed, short_work.then_some(PREPARED_TEST_LIMIT + 1));
    }
}

#[test]
fn prepared_input_scope_preserves_prior_history_and_unwinds_its_header() {
    for behavior in 0..3 {
        let mut expected_denial = None;
        let actual_denial =
            with_original_prepared_input(|cursor, _, values, types, locals, budget| {
                let floor = budget.storage();
                assert!(budget.reserve_storage(PREPARED_TEST_LIMIT).is_err());
                assert_eq!(budget.failed_storage(), Some(floor + PREPARED_TEST_LIMIT));
                expected_denial = Some(budget.work() + PREPARED_TEST_LIMIT);
                assert!(budget.charge_work(PREPARED_TEST_LIMIT).is_err());
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_prepared_input_transport_v1(
                        Some(cursor),
                        cursor.function,
                        cursor.ssa,
                        values,
                        types,
                        locals,
                        Some(budget),
                        |prepared, budget| {
                            let budget = budget.unwrap();
                            assert_eq!(
                                prepared.unwrap().ordinary(1, budget)?,
                                Some(SemanticTypeIdV1::from_index(1))
                            );
                            budget.reserve_storage(13)?;
                            match behavior {
                                0 => Ok(()),
                                1 => Err(invocation_entry_error_v1()),
                                _ => panic!("prepared input callback unwind"),
                            }
                        },
                    )
                }));
                match behavior {
                    0 => assert!(result.unwrap().is_ok()),
                    1 => assert!(result.unwrap().is_err()),
                    _ => assert!(result.is_err()),
                }
                assert_eq!(budget.storage(), floor + if behavior == 0 { 13 } else { 0 });
                if behavior == 0 {
                    budget.release_storage(13).unwrap();
                }
                assert_eq!(budget.failed_storage(), Some(floor + PREPARED_TEST_LIMIT));
            });
        assert_eq!(actual_denial, expected_denial);
    }
}

#[test]
fn prepared_component_walk_keeps_nested_order_and_zero_width_without_parameters() {
    let scalar = |id| SemanticValueBindingV1::Value {
        id: ValueId(id),
        ty: Type::Scalar(ScalarType::U32),
    };
    let binding = SemanticValueBindingV1::Aggregate(vec![
        SemanticValueBindingV1::Unit,
        SemanticValueBindingV1::Aggregate(vec![scalar(12), SemanticValueBindingV1::Unit]),
        scalar(13),
    ]);
    // Six binding nodes visited twice; each physical value charges once, then
    // invocation_equal_types_v1 prepays its two scalar Type trees separately.
    let required = 2 * 6 + 2 * (1 + 2);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(required - usize::from(short));
        let mut budget = ArgumentBudgetV1::new(&mut work, 73);
        budget.reserve_storage(73).unwrap();
        assert!(prepared_input_ordinary_shape_v1(&binding, &mut 0, &mut budget).unwrap());
        let result = binding.visit_values_v1(&mut PreparedInputComponentsV1 {
            values: &[ValueId(12), ValueId(13)],
            types: &[Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)],
            next: 0,
            nodes: 0,
            budget: &mut budget,
        });
        assert_eq!(result.is_ok(), !short);
        assert_eq!(budget.storage(), 73);
        drop(budget);
        assert_eq!(work.failed_work(), short.then_some(required));
    }
    for values in [
        vec![ValueId(13), ValueId(12)],
        vec![ValueId(12), ValueId(12)],
        vec![ValueId(12)],
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert!(
            binding
                .visit_values_v1(&mut PreparedInputComponentsV1 {
                    values: &values,
                    types: &[Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)],
                    next: 0,
                    nodes: 0,
                    budget: &mut budget,
                })
                .is_err()
        );
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    let mut empty = PreparedInputComponentsV1 {
        values: &[],
        types: &[],
        next: 0,
        nodes: 0,
        budget: &mut budget,
    };
    SemanticValueBindingV1::Unit
        .visit_values_v1(&mut empty)
        .unwrap();
    assert_eq!(empty.next, 0);
    assert!(
        !prepared_input_ordinary_shape_v1(
            &SemanticValueBindingV1::MathContext,
            &mut 0,
            &mut budget
        )
        .unwrap()
    );
}

struct UnadmittedPreparedBudget<'a, 'work>(&'a mut ArgumentBudgetV1<'work>);

impl SemanticEmissionBudgetV1 for UnadmittedPreparedBudget<'_, '_> {
    fn work_ledger_identity_v1(&self) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 {
        self.0.work_ledger_identity_v1()
    }
    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.0.charge_work(amount).map_err(Into::into)
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.0.reserve_storage(amount).map_err(Into::into)
    }
    fn release_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.0.release_storage(amount).map_err(Into::into)
    }
    fn storage(&self) -> usize {
        self.0.storage()
    }
}

#[test]
fn prepared_view_refuses_an_unadmitted_meter_without_work_or_storage_effects() {
    with_original_prepared_input(|cursor, _, values, types, locals, budget| {
        let before = (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage(),
        );
        let slot = budget.prepared_input_slot_v1().unwrap();
        let mut alternative = UnadmittedPreparedBudget(budget);
        assert!(!alternative.permits_prepared_input_refund_v1(
            None,
            slot,
            cursor.ledger,
            before.1,
            0,
        ));
        let result: Result<(), _> = with_prepared_input_transport_v1(
            Some(cursor),
            cursor.function,
            cursor.ssa,
            values,
            types,
            locals,
            Some(&mut alternative),
            |_, _| panic!("unadmitted meter reached consumer"),
        );
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        drop(alternative);
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            before
        );
    });
}
