use ProductionSourceOutputPrivateArrayInitializerV1 as InitializerOutcome;

fn initialized_output(components: u64) -> InitializerOutcome {
    InitializerOutcome::Checked {
        components,
        retained_executable: components,
        retained_nonexecutable: 0,
        omitted_unreachable: 0,
    }
}

#[test]
fn initializer_output_values_join_every_component_after_real_seven_passes() {
    for (values, float) in [
        ([11, 29, 31, 37, 41, 43, 47, u32::MAX], false),
        ([11; 8], false),
        (
            [
                0,
                0x8000_0000,
                0x3f80_0000,
                0x7f80_0000,
                0xff80_0000,
                0x7fc0_0001,
                0x7fc0_0002,
                1,
            ],
            true,
        ),
    ] {
        for repetitions in [1, 2] {
            with_output(
                array_owner(ArrayCase::Initializer {
                    values,
                    repetitions,
                    float,
                }),
                |view, budget| {
                    let floor = budget.storage();
                    assert_eq!(view.private_arrays.len(), repetitions * 8 + 1);
                    for statement in 1..=repetitions {
                        let result = view
                            .private_array_initializer(
                                ARRAY_ROOT,
                                ARRAY_ROOT,
                                site(0, statement as u32),
                                budget,
                            )
                            .unwrap();
                        assert_eq!(result, initialized_output(8));
                        assert!(!result.grants_authority());
                        let mut observed = 0;
                        for row in view
                            .private_arrays
                            .iter()
                            .filter(|row| row.key[3] == statement as u32)
                        {
                            let SourceOutputArrayPlacementV1::Retained(anchors) = row.placement
                            else {
                                panic!("actual retained initializer Store required");
                            };
                            let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                                operation,
                                result: 0,
                            } = anchors.uses[4].definition
                            else {
                                panic!("this actual literal fixture retains a constant result");
                            };
                            let definition =
                                source_output_operation_v1(view.output(), operation, budget)
                                    .unwrap();
                            let expected = if float {
                                Constant::F32Bits(values[observed])
                            } else {
                                Constant::U32(values[observed])
                            };
                            assert!(
                                matches!(&definition.kind, OperationKind::Constant(actual) if *actual == expected)
                            );
                            assert_eq!(row.key[6] as usize, observed);
                            let memory =
                                source_output_operation_v1(view.output(), anchors.memory, budget)
                                    .unwrap();
                            assert!(
                                matches!(memory.kind, OperationKind::Store { value, .. } if value == definition.results[0].id)
                            );
                            observed += 1;
                        }
                        assert_eq!(observed, 8);
                        assert!(matches!(
                            view.private_array_write(
                                ARRAY_ROOT,
                                ARRAY_ROOT,
                                site(0, statement as u32),
                                Role::Destination,
                                budget,
                            ),
                            Err(ProductionSourceOutputErrorV1::PrivateArray(
                                SemanticKirPrivateArrayQueryErrorV1::Incomplete(
                                    "private array requires one exact index projection"
                                )
                            ))
                        ));
                    }
                    assert!(matches!(
                        view.private_array_write(
                            ARRAY_ROOT,
                            ARRAY_ROOT,
                            site(0, repetitions as u32 + 1),
                            Role::Destination,
                            budget,
                        ),
                        Ok(ProductionSourceOutputPrivateArrayAccessV1::Retained {
                            index: 0,
                            executable: true,
                            ..
                        })
                    ));
                    assert_eq!(budget.storage(), floor);
                },
            );
        }
    }
}

#[test]
fn initializer_output_keeps_retained_read_promotion_and_helper_boundaries() {
    with_output(array_owner(ArrayCase::RetainedValueRead), |view, budget| {
        assert_eq!(view.private_arrays.len(), 10);
        assert_eq!(
            view.private_array_initializer(ARRAY_ROOT, ARRAY_ROOT, site(0, 1), budget)
                .unwrap(),
            initialized_output(8)
        );
        assert!(matches!(
            view.private_array_write(
                ARRAY_ROOT,
                ARRAY_ROOT,
                site(0, 2),
                Role::Destination,
                budget
            ),
            Ok(ProductionSourceOutputPrivateArrayAccessV1::Retained { index: 0, .. })
        ));
        assert!(matches!(
            view.private_array_write(
                ARRAY_ROOT,
                ARRAY_ROOT,
                site(0, 3),
                Role::RvalueOperand(0),
                budget
            ),
            Err(ProductionSourceOutputErrorV1::PrivateArray(
                SemanticKirPrivateArrayQueryErrorV1::Incomplete(
                    "checked output currently requires an ordinary private-array write"
                )
            ))
        ));
        assert!(matches!(
            view.private_array_initializer(ARRAY_ROOT, ARRAY_ROOT, site(0, 3), budget),
            Err(ProductionSourceOutputErrorV1::PrivateArray(
                SemanticKirPrivateArrayQueryErrorV1::Incomplete(
                    "private initializer requires an Array Aggregate"
                )
            ))
        ));
    });
    with_output(
        array_owner(ArrayCase::ValueRead { local_index: false }),
        |view, budget| {
            assert!(view.private_arrays.is_empty());
            let result = view
                .private_array_initializer(ARRAY_ROOT, ARRAY_ROOT, site(0, 1), budget)
                .unwrap();
            assert_eq!(result, InitializerOutcome::ProvenUnretained);
            assert!(!result.grants_authority());
        },
    );
    assert!(matches!(
        array_owner_at_body(
            ArrayCase::Initializer {
                values: [11; 8],
                repetitions: 1,
                float: false
            },
            true
        ),
        Err(ProductionPreRankedKirErrorV1::Lowering(
            ProductionSemanticKirErrorV1::HelperEffectsUnavailable { function: 1, .. }
        ))
    ));
}

#[test]
fn initializer_output_rejects_private_row_and_actual_operand_substitutions() {
    with_output(
        array_owner(ArrayCase::Initializer {
            values: [11, 29, 31, 37, 41, 43, 47, 53],
            repetitions: 1,
            float: false,
        }),
        |view, budget| {
            // Mutate derived rows only, never admitted sources or checked owners.
            let saved: [SourceOutputArrayRowV1; 8] = view.private_arrays[..8].try_into().unwrap();
            for mutation in 0..9 {
                let expected = match mutation {
                    0 => {
                        view.private_arrays[7] = view.private_arrays[8];
                        "initializer output component census changed"
                    }
                    1 => {
                        view.private_arrays.swap(0, 1);
                        "initializer original occurrence identity changed"
                    }
                    2 => {
                        view.private_arrays[1] = view.private_arrays[0];
                        "initializer original occurrence identity changed"
                    }
                    3 => {
                        view.private_arrays[0].original_effect =
                            view.private_arrays[8].original_effect;
                        "initializer original component tag changed"
                    }
                    4 => {
                        view.private_arrays[0].original_slot = usize::MAX;
                        "initializer original slot is absent"
                    }
                    5 => {
                        view.private_arrays[0].placement =
                            SourceOutputArrayPlacementV1::Unsupported;
                        "initializer output placement is unsupported"
                    }
                    6..=8 => {
                        let SourceOutputArrayPlacementV1::Retained(other) = saved[1].placement
                        else {
                            panic!("retained fixture");
                        };
                        let SourceOutputArrayPlacementV1::Retained(mut anchors) =
                            saved[0].placement
                        else {
                            panic!("retained fixture");
                        };
                        let detail = if mutation == 6 {
                            anchors.uses[2].definition = other.uses[2].definition;
                            "array output index type or literal changed"
                        } else if mutation == 7 {
                            anchors.uses[4].definition = other.uses[4].definition;
                            "array output stored-value ancestry or type changed"
                        } else {
                            let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                                mut operation,
                                result,
                            } = anchors.uses[4].definition
                            else {
                                panic!("constant result");
                            };
                            operation.operation = u32::MAX;
                            anchors.uses[4].definition =
                                fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                                    operation,
                                    result,
                                };
                            "array output operation coordinate is absent"
                        };
                        view.private_arrays[0].placement =
                            SourceOutputArrayPlacementV1::Retained(anchors);
                        detail
                    }
                    _ => unreachable!(),
                };
                let floor = budget.storage();
                assert!(
                    matches!(view.private_array_initializer(ARRAY_ROOT, ARRAY_ROOT, site(0, 1), budget),
                    Err(ProductionSourceOutputErrorV1::Invalid(actual)) if actual == expected)
                );
                assert_eq!(budget.storage(), floor);
                view.private_arrays[..8].copy_from_slice(&saved);
                assert_eq!(
                    view.private_array_initializer(ARRAY_ROOT, ARRAY_ROOT, site(0, 1), budget)
                        .unwrap(),
                    initialized_output(8)
                );
            }
            for (root, body, expected) in [
                (1, 0, "selected root is absent from this owner"),
                (0, 1, "initializer body is absent from this owner"),
            ] {
                assert!(matches!(view.private_array_initializer(
                    SemanticFunctionIdV1::from_index(root), SemanticFunctionIdV1::from_index(body), site(0, 1), budget,
                ), Err(ProductionSourceOutputErrorV1::PrivateArray(SemanticKirPrivateArrayQueryErrorV1::InvalidSource(actual))) if actual == expected));
            }
        },
    );
}

#[test]
fn initializer_output_distinct_bound_owner_preserves_exact_source_and_history_custody() {
    let case = ArrayCase::Initializer {
        values: [11; 8],
        repetitions: 1,
        float: false,
    };
    let source = array_owner(case);
    let foreign = array_owner(case);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let source_storage = retained(&source);
    let foreign_storage = retained(&foreign);
    budget.reserve_storage(source_storage).unwrap();
    budget.reserve_storage(foreign_storage).unwrap();
    let mut module = source.executable().module().clone();
    assert!(
        module
            .required_capabilities
            .insert(fe2o3_kernel_ir::TargetCapability::WaveWidth(
                fe2o3_kernel_ir::WaveWidth::Wave64
            ))
    );
    let (bound, bound_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(bound_storage.retained_storage())
        .unwrap();
    let checked = optimize(&bound, &mut budget);
    let wrong_history = optimize(source.executable(), &mut budget);
    let (coordinates, coordinate_storage) =
        check_canonical_kir_coordinate_preservation_v1(source.executable(), &bound, &mut budget)
            .unwrap();
    budget
        .reserve_storage(coordinate_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    assert!(matches!(
        derive_source_output_occurrences_v1(&foreign, &coordinates, &checked, &mut budget),
        Err(ProductionSourceOutputErrorV1::InputCustody)
    ));
    assert_eq!(budget.storage(), floor);
    assert!(matches!(
        derive_source_output_occurrences_v1(&source, &coordinates, &wrong_history, &mut budget),
        Err(ProductionSourceOutputErrorV1::InputCustody)
    ));
    assert_eq!(budget.storage(), floor);
    let (view, storage) =
        derive_source_output_occurrences_v1(&source, &coordinates, &checked, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert!(!std::ptr::eq(view.source().executable(), view.bound()));
    assert_ne!(
        view.source().executable().canonical().canonical_bytes(),
        view.bound().canonical().canonical_bytes()
    );
    assert_eq!(
        view.private_array_initializer(ARRAY_ROOT, ARRAY_ROOT, site(0, 1), &mut budget)
            .unwrap(),
        initialized_output(8)
    );
    drop(view);
    budget.release_storage(storage.retained_storage()).unwrap();
    #[allow(
        clippy::drop_non_drop,
        reason = "End the borrowed coordinate relation before owner release"
    )]
    drop(coordinates);
    budget
        .release_storage(coordinate_storage.retained_storage())
        .unwrap();
    let wrong_storage = wrong_history.storage().retained_storage();
    drop(wrong_history);
    budget.release_storage(wrong_storage).unwrap();
    let checked_storage = checked.storage().retained_storage();
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(bound);
    budget
        .release_storage(bound_storage.retained_storage())
        .unwrap();
    drop(foreign);
    budget.release_storage(foreign_storage).unwrap();
    drop(source);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn initializer_output_wrong_value_and_cross_site_use_fail_at_checked_transition() {
    use fe2o3_kernel_analysis::{
        CanonicalKirInventoryV1, CanonicalKirTransitionErrorV1, check_canonical_kir_transition_v1,
    };
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirUseCoordinateV1 as Use,
    };
    with_output(
        array_owner(ArrayCase::Initializer {
            values: [11, 29, 31, 37, 41, 43, 47, 53],
            repetitions: 2,
            float: false,
        }),
        |view, live_budget| {
            let SourceOutputArrayPlacementV1::Retained(first) = view.private_arrays[0].placement
            else {
                panic!("retained fixture");
            };
            let SourceOutputArrayPlacementV1::Retained(second) = view.private_arrays[1].placement
            else {
                panic!("retained fixture");
            };
            let SourceOutputArrayPlacementV1::Retained(later) = view.private_arrays[8].placement
            else {
                panic!("retained fixture");
            };
            with_control(view, live_budget, |_, transition, floor| {
                for mutation in 0..2 {
                    // Freshly verify the changed O. Do not manufacture a checked
                    // neutral owner: this is an actual-IR transition negative.
                    let mut module = view.output().module().clone();
                    if mutation == 0 {
                        let Definition::Result {
                            operation,
                            result: 0,
                        } = first.uses[4].definition
                        else {
                            panic!("literal result");
                        };
                        let operation = &mut module.functions[operation.block.function.0 as usize]
                            .body
                            .as_mut()
                            .unwrap()
                            .blocks[operation.block.block as usize]
                            .operations[operation.operation as usize];
                        let OperationKind::Constant(Constant::U32(bits)) = &mut operation.kind
                        else {
                            panic!("U32 literal");
                        };
                        *bits ^= 1;
                    } else {
                        let mut values = [ValueId(0); 2];
                        for (index, coordinate) in
                            [first.memory, second.memory].into_iter().enumerate()
                        {
                            let operation = &module.functions[coordinate.block.function.0 as usize]
                                .body
                                .as_ref()
                                .unwrap()
                                .blocks[coordinate.block.block as usize]
                                .operations[coordinate.operation as usize];
                            let OperationKind::Store { value, .. } = operation.kind else {
                                panic!("Store");
                            };
                            values[index] = value;
                        }
                        assert_ne!(values[0], values[1]);
                        for (index, coordinate) in
                            [first.memory, second.memory].into_iter().enumerate()
                        {
                            let operation = &mut module.functions
                                [coordinate.block.function.0 as usize]
                                .body
                                .as_mut()
                                .unwrap()
                                .blocks[coordinate.block.block as usize]
                                .operations[coordinate.operation as usize];
                            let OperationKind::Store { value, .. } = &mut operation.kind else {
                                panic!("Store");
                            };
                            *value = values[1 - index];
                        }
                    }
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                    budget.reserve_storage(floor).unwrap();
                    let (owner, owner_storage) = VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
                    budget
                        .reserve_storage(owner_storage.retained_storage())
                        .unwrap();
                    let (inventory, inventory_storage) =
                        CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
                    budget
                        .reserve_storage(inventory_storage.retained_storage())
                        .unwrap();
                    let live = budget.storage();
                    assert!(matches!(
                        check_canonical_kir_transition_v1(
                            transition.input(),
                            &inventory,
                            transition.rows(),
                            &mut budget
                        ),
                        Err(CanonicalKirTransitionErrorV1::Rule(_))
                    ));
                    assert_eq!(budget.storage(), live);
                    drop(inventory);
                    budget
                        .release_storage(inventory_storage.retained_storage())
                        .unwrap();
                    drop(owner);
                    budget
                        .release_storage(owner_storage.retained_storage())
                        .unwrap();
                    assert_eq!(budget.storage(), floor);
                }
                let candidate = transition.rows();
                let mut uses = candidate.uses.to_vec();
                let first_use = uses
                    .iter()
                    .position(|row| {
                        row.output
                            == Use::OperationOperand {
                                operation: first.memory,
                                operand: 1,
                            }
                    })
                    .unwrap();
                let later_use = uses
                    .iter()
                    .position(|row| {
                        row.output
                            == Use::OperationOperand {
                                operation: later.memory,
                                operand: 1,
                            }
                    })
                    .unwrap();
                assert_ne!(uses[first_use].input, uses[later_use].input);
                // Equal literals at two statements still have distinct use origins.
                let first_input = uses[first_use].input;
                uses[first_use].input = uses[later_use].input;
                uses[later_use].input = first_input;
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                assert!(matches!(
                    check_canonical_kir_transition_v1(
                        transition.input(),
                        transition.output(),
                        fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1 {
                            uses: &uses,
                            ..candidate
                        },
                        &mut budget
                    ),
                    Err(CanonicalKirTransitionErrorV1::Rule(
                        "final operation operand origin" | "final operand has no exact descendant"
                    ))
                ));
                assert_eq!(budget.storage(), floor);
                let mut operations = candidate.operations.to_vec();
                let first_operation = operations
                    .iter()
                    .position(|row| row.output == first.memory)
                    .unwrap();
                let second_operation = operations
                    .iter()
                    .position(|row| row.output == second.memory)
                    .unwrap();
                operations[second_operation].origin = operations[first_operation].origin;
                assert!(matches!(
                    check_canonical_kir_transition_v1(
                        transition.input(),
                        transition.output(),
                        fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1 {
                            operations: &operations,
                            ..candidate
                        },
                        &mut budget,
                    ),
                    Err(CanonicalKirTransitionErrorV1::Rule(
                        "retained operation owner or duplication"
                    ))
                ));
                assert_eq!(budget.storage(), floor);
            });
        },
    );
}

#[test]
fn initializer_output_prefix_and_retained_tail_have_exact_work_boundaries() {
    with_output(
        array_owner(ArrayCase::Initializer {
            values: [11; 8],
            repetitions: 1,
            float: false,
        }),
        |view, live_budget| {
            let floor = live_budget.storage();
            assert_eq!(view.private_arrays.len(), 9);
            let prefix = |row: &SourceOutputArrayRowV1| {
                [
                    row.key[0] as usize,
                    row.key[1] as usize,
                    row.key[2] as usize,
                    row.key[3] as usize,
                    row.key[4] as usize,
                    row.key[5] as usize,
                ]
            };
            // Lower partition: four equal iterations (12 each), terminal1=49.
            // Upper: equal13 + equal13 + statement mismatch10 + terminal1=37.
            for limit in [85, 86] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                let start = private_array_partition_v1(
                    &view.private_arrays,
                    prefix,
                    [0, 0, 0, 1, 2, 0],
                    false,
                    &mut PrivateArrayQueryWorkV1 {
                        budget: &mut budget,
                    },
                )
                .unwrap();
                assert_eq!((start, budget.work()), (0, 49));
                let end = private_array_partition_v1(
                    &view.private_arrays,
                    prefix,
                    [0, 0, 0, 1, 2, 0],
                    true,
                    &mut PrivateArrayQueryWorkV1 {
                        budget: &mut budget,
                    },
                );
                if limit == 86 {
                    assert_eq!(end.unwrap(), 8);
                } else {
                    assert!(
                        matches!(end, Err(SemanticKirPrivateArrayQueryErrorV1::Resource(AssertOriginResourceV1::Work(error))) if error.actual() == 86 && error.limit() == 85)
                    );
                }
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (limit, floor, floor)
                );
            }
            let row = &view.private_arrays[0];
            let slot = &view.source.correspondence.private_arrays.slots[row.original_slot];
            let SourceOutputArrayPlacementV1::Retained(anchors) = row.placement else {
                panic!("retained fixture");
            };
            for limit in [96, 97] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                let result = view.private_array_retained_write_v1(slot, anchors, 0, &mut budget);
                if limit == 97 {
                    assert!(matches!(
                        result,
                        Ok(ProductionSourceOutputPrivateArrayAccessV1::Retained {
                            index: 0,
                            executable: true,
                            ..
                        })
                    ));
                    assert_eq!(budget.work(), 97);
                } else {
                    assert!(
                        matches!(result, Err(ProductionSourceOutputErrorV1::PrivateArray(SemanticKirPrivateArrayQueryErrorV1::Resource(AssertOriginResourceV1::Work(error)))) if error.actual() == 97 && error.limit() == 96)
                    );
                    assert_eq!(budget.work(), 96);
                }
                assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
            }
        },
    );
}

#[test]
fn initializer_output_full_schedule_is_once_source_plus_exact_new_work() {
    with_output(
        array_owner(ArrayCase::Initializer {
            values: [11; 8],
            repetitions: 1,
            float: false,
        }),
        |view, live_budget| {
            // This is a relative schedule against the unchanged paid source query,
            // not a calibrated literal total for all source/SSA validation.
            let before = live_budget.work();
            assert_eq!(
                view.source()
                    .materialized_private_array_initializer_count(
                        ARRAY_ROOT,
                        ARRAY_ROOT,
                        site(0, 1),
                        live_budget
                    )
                    .unwrap(),
                Some(8)
            );
            let source_work = live_budget.work() - before;
            const PRIOR: usize = 11;
            // Fixed22 + the exact prefix86 + eight (identity33 + scalar O tail97).
            let exact = PRIOR + source_work + 22 + 86 + 8 * 130;
            let floor = live_budget.storage();
            for (limit, seeded) in [(exact, false), (exact - 1, false), (exact - 1, true)] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                if seeded {
                    assert!(work.charge_work(limit + 7).is_err());
                }
                let mut budget = AssertOriginBudgetV1::new(&mut work, floor + 3);
                budget.charge_work(PRIOR).unwrap();
                budget.reserve_storage(floor + 3).unwrap();
                budget.release_storage(3).unwrap();
                let result =
                    view.private_array_initializer(ARRAY_ROOT, ARRAY_ROOT, site(0, 1), &mut budget);
                if limit == exact {
                    assert_eq!(result.unwrap(), initialized_output(8));
                    assert_eq!(budget.work(), exact);
                } else {
                    assert!(
                        matches!(result, Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Work(error))) if error.actual() == exact && error.limit() == limit)
                    );
                    assert_eq!(budget.work(), exact - 4);
                }
                assert_eq!(
                    (budget.storage(), budget.peak_storage()),
                    (floor, floor + 3)
                );
                assert_eq!(
                    work.failed_work(),
                    if seeded {
                        Some(limit + 7)
                    } else if limit < exact {
                        Some(exact)
                    } else {
                        None
                    }
                );
            }
            assert_eq!(
                view.private_array_initializer(ARRAY_ROOT, ARRAY_ROOT, site(0, 1), live_budget)
                    .unwrap(),
                initialized_output(8)
            );
        },
    );
}

#[test]
fn initializer_output_entry_floor_and_private_outcome_components_are_separate() {
    with_output(
        array_owner(ArrayCase::Initializer {
            values: [11; 8],
            repetitions: 1,
            float: false,
        }),
        |view, live_budget| {
            let minimum = retained(view.source())
                + view.checked_output.storage().retained_storage()
                + view.storage.retained_storage();
            for (limit, floor) in [(3, minimum), (4, minimum - 1)] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                let result =
                    view.private_array_initializer(ARRAY_ROOT, ARRAY_ROOT, site(0, 1), &mut budget);
                if limit == 3 {
                    assert!(
                        matches!(result, Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Work(error))) if error.actual() == 4 && error.limit() == 3)
                    );
                    assert_eq!(budget.work(), 0);
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionSourceOutputErrorV1::Resource(
                            AssertOriginResourceV1::Accounting
                        ))
                    ));
                    assert_eq!(budget.work(), 4);
                }
                assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
            }
            // Branch accounting components only. These private mutations do NOT
            // establish that the real CFG proves these Stores unreachable.
            let saved: [SourceOutputArrayRowV1; 8] = view.private_arrays[..8].try_into().unwrap();
            for (index, row) in view.private_arrays[..8].iter_mut().enumerate() {
                if index < 3 {
                    row.placement = SourceOutputArrayPlacementV1::OmittedUnreachable;
                } else if index < 5 {
                    let SourceOutputArrayPlacementV1::Retained(ref mut anchors) = row.placement
                    else {
                        panic!("retained fixture");
                    };
                    anchors.executable = false;
                }
            }
            assert_eq!(
                view.private_array_initializer(ARRAY_ROOT, ARRAY_ROOT, site(0, 1), live_budget)
                    .unwrap(),
                InitializerOutcome::Checked {
                    components: 8,
                    retained_executable: 3,
                    retained_nonexecutable: 2,
                    omitted_unreachable: 3
                }
            );
            view.private_arrays[..8].copy_from_slice(&saved);
            assert_eq!(
                view.private_array_initializer(ARRAY_ROOT, ARRAY_ROOT, site(0, 1), live_budget)
                    .unwrap(),
                initialized_output(8)
            );
        },
    );
}
