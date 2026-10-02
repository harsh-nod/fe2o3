// Component-level provenance and extent contracts, not source/GPU admission.
use super::super::bf16_nominal_source_algorithms_v1::local_provenance_with_resources_v1;

fn shared_slice_reborrow_fixture() -> SemanticFunctionDeclV1 {
    let base = fixture(false);
    let mut blocks = base.blocks().to_vec();
    let mut statements = blocks[0].statements().to_vec();
    statements[0] = typed_assignment(
        4,
        REFERENCE,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: slice_place(1, None),
        },
    );
    blocks[0] = block(246, statements, blocks[0].terminator().kind().clone());
    rebuild(&base, base.locals().to_vec(), blocks)
}

#[test]
fn shared_slice_reborrow_preserves_both_provenance_paths_and_extent_slots() {
    let types = types();
    let function = shared_slice_reborrow_fixture();
    let original = fixture(false);
    let mut seed = scratch(&types, &function);
    let original_seed = scratch(&types, &original);
    assert_eq!(seed.origins, original_seed.origins);
    assert_eq!(seed.origins[4], Some(0));
    assert_eq!(seed.origins[5], Some(0));
    assert_eq!(seed.origins[2], Some(1));
    let old_allocation = local_provenance_with_resources_v1(
        &[],
        &types,
        &original,
        &original_seed.definitions,
        &original_seed.escaped,
        &mut PreparationResourcesV1::unmetered(),
    )
    .unwrap();
    let allocation = local_provenance_with_resources_v1(
        &[],
        &types,
        &function,
        &seed.definitions,
        &seed.escaped,
        &mut PreparationResourcesV1::unmetered(),
    )
    .unwrap();
    assert_eq!(
        allocation.allocation_origins,
        old_allocation.allocation_origins
    );
    assert_eq!(
        allocation.allocation_provenance,
        old_allocation.allocation_provenance
    );
    {
        use super::super::bf16_nominal_source_algorithms_v1::RetainedLocalProvenanceV1;
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut retained = RetainedLocalProvenanceV1::new();
        let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
        retained
            .prepare_into(
                &[],
                &types,
                &function,
                &seed.definitions,
                &seed.escaped,
                &mut resources,
            )
            .unwrap();
        let actual = retained
            .completed_for(
                &[],
                &types,
                &function,
                &seed.definitions,
                &seed.escaped,
                &resources,
            )
            .unwrap();
        assert_eq!(
            actual.stable_argument_origins,
            allocation.stable_argument_origins
        );
        assert_eq!(actual.allocation_origins, allocation.allocation_origins);
        assert_eq!(
            actual.allocation_provenance,
            allocation.allocation_provenance
        );
        drop(resources);
        drop(retained);
        assert_eq!(budget.storage(), owned);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    let mut facts = TraceFacts::default();
    let mut next = 3;
    for (length, argument) in [(6, 3), (7, 3), (6, 3), (8, 4)] {
        assert_eq!(
            Context {
                scratch: &mut seed,
                facts: &mut facts
            }
            .extent(
                &types,
                &function,
                SemanticLocalIdV1::from_index(length),
                &definitions(&function),
                &mut next,
            )
            .unwrap(),
            Some(ProductionRankedValueV1::Argument(argument))
        );
    }
    assert_eq!(next, 5);
    assert_eq!(seed.arguments[..2], [Some(3), Some(4)]);
    in_paid!(types, function, rich, budget, owned, storage, view, {
        assert_eq!(rich.stable_argument_origins(), seed.origins);
        assert_eq!(rich.allocation_origins(), allocation.allocation_origins);
        assert_eq!(
            rich.allocation_provenance(),
            allocation.allocation_provenance
        );
        let mut state = BoundsExtentArgumentsV1::new();
        state
            .initialize(
                &types,
                &function,
                rich,
                &view,
                &vec![None; function.locals().len()],
                3,
                &mut PreparationResourcesV1::new(budget, &mut owned),
            )
            .unwrap();
        for (length, argument) in [(6, 3), (7, 3), (8, 4)] {
            assert_eq!(
                state
                    .extent(
                        &types,
                        &function,
                        rich,
                        &view,
                        SemanticLocalIdV1::from_index(length),
                        &mut PreparationResourcesV1::new(budget, &mut owned)
                    )
                    .unwrap(),
                Some(ProductionRankedValueV1::Argument(argument))
            );
        }
        assert_eq!(extent::test_access::copy_rows(&state), seed.arguments);
        assert_eq!(extent::test_access::state(&state).4, next);
        drop(state);
    })
    .unwrap();
}

#[test]
fn shared_slice_reborrow_requires_exact_type_and_one_shared_dereference() {
    use super::super::bf16_nominal_source_algorithms_v1::exact_shared_slice_reborrow_source_v1;
    for case in 0..13 {
        let mut types = types();
        let mut function = shared_slice_reborrow_fixture();
        let mut place = slice_place(1, None);
        let mut result = REFERENCE;
        let mut kind = SemanticBorrowKindV1::Shared;
        match case {
            0 => {}
            1 => kind = SemanticBorrowKindV1::Mutable,
            2 => place = typed_place(1, REFERENCE),
            3 => place = slice_place(1, Some(3)),
            4 => result = USIZE,
            5 => {
                place = SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(1),
                    vec![
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Dereference,
                            SCALAR_TYPE,
                        )
                        .unwrap(),
                    ],
                    SCALAR_TYPE,
                )
                .unwrap()
            }
            6 | 7 | 8 | 9 => {
                types[2] = SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256(bytes(118)),
                    SemanticLayoutIdentityV1::from_sha256(bytes(118)),
                    SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
                    SemanticTypeShapeV1::Pointer(
                        SemanticPointerTypeV1::new_with_kind(
                            if case == 9 { SCALAR_TYPE } else { SLICE },
                            if case == 6 {
                                SemanticPointerKindV1::Raw
                            } else {
                                SemanticPointerKindV1::Reference
                            },
                            if case == 7 {
                                SemanticMutabilityV1::Mutable
                            } else {
                                SemanticMutabilityV1::Immutable
                            },
                            0,
                            64,
                            if case == 8 {
                                SemanticPointerMetadataV1::None
                            } else {
                                SemanticPointerMetadataV1::SliceLength
                            },
                        )
                        .unwrap(),
                    ),
                );
            }
            10 => {
                types.push(types[2].clone());
                result = SemanticTypeIdV1::from_index(5);
            }
            11 => {
                let mut locals = function.locals().to_vec();
                locals[1] = local(235, USIZE, SemanticLocalRoleV1::Argument(0));
                function = rebuild(&function, locals, function.blocks().to_vec());
            }
            12 => place = slice_place(99, None),
            _ => unreachable!(),
        }
        let value = SemanticRvalueV1::new(result, SemanticRvalueKindV1::Borrow { kind, place });
        assert_eq!(
            exact_shared_slice_reborrow_source_v1(&types, &function, &value),
            if case == 0 {
                Some(SemanticLocalIdV1::from_index(1))
            } else {
                None
            },
            "case {case}"
        );
    }
}

#[test]
fn shared_slice_reborrow_redefinition_escape_and_changed_origin_refuse_extent() {
    let types = types();
    let function = shared_slice_reborrow_fixture();
    for case in 0..7 {
        let mut seed = scratch(&types, &function);
        let mut rows = definitions(&function);
        match case {
            0 => seed.definitions[4] = 2,
            1 => seed.definitions[1] = 1,
            2 => seed.escaped[4] = true,
            3 => seed.escaped[1] = true,
            4 => seed.origins[4] = Some(1),
            5 => rows[4].count = 2,
            6 => rows[1].count = 1,
            _ => unreachable!(),
        }
        let mut facts = TraceFacts::default();
        let mut next = 3;
        assert_eq!(
            Context {
                scratch: &mut seed,
                facts: &mut facts
            }
            .extent(
                &types,
                &function,
                SemanticLocalIdV1::from_index(6),
                &rows,
                &mut next
            )
            .unwrap(),
            None,
            "case {case}"
        );
        assert_eq!(next, 3);
        assert!(seed.arguments.iter().all(Option::is_none));
        if case < 4 {
            let actual = local_provenance_with_resources_v1(
                &[],
                &types,
                &function,
                &seed.definitions,
                &seed.escaped,
                &mut PreparationResourcesV1::unmetered(),
            )
            .unwrap();
            assert_eq!(actual.stable_argument_origins[4], None, "case {case}");
        }
    }
}

#[test]
fn shared_slice_reborrow_extent_paid_exact_work_and_one_short() {
    let types = types();
    let function = shared_slice_reborrow_fixture();
    for remaining in [79, 78] {
        let outcome = in_paid!(types, function, rich, budget, owned, storage, view, {
            let initial = vec![None; function.locals().len()];
            let mut state = BoundsExtentArgumentsV1::new();
            state
                .initialize(
                    &types,
                    &function,
                    rich,
                    &view,
                    &initial,
                    3,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            budget
                .charge_work(LIMIT - budget.work() - remaining)
                .unwrap();
            let before = budget.work();
            let credits = owned;
            let result = state.extent(
                &types,
                &function,
                rich,
                &view,
                SemanticLocalIdV1::from_index(6),
                &mut PreparationResourcesV1::new(budget, &mut owned),
            );
            if remaining == 79 {
                assert_eq!(result.unwrap(), Some(ProductionRankedValueV1::Argument(3)));
                assert_eq!(budget.work() - before, 79);
                assert_eq!(extent::test_access::copy_rows(&state)[0], Some(3));
            } else {
                assert!(result.is_err());
                assert!(budget.failed_work().is_some());
                assert_eq!(extent::test_access::copy_rows(&state), initial);
                assert_eq!(extent::test_access::state(&state).4, 3);
            }
            assert_eq!(owned, credits);
            drop(state);
        });
        assert_eq!(outcome.is_ok(), remaining == 79);
    }
}

#[test]
fn shared_slice_reborrow_volatile_read_eligibility_matches_copy_and_preserves_abi_checks() {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticAbiPointeeInfoV1, SemanticTypeAbiPropertiesV1};
    for case in 0..7 {
        let mut types = types();
        let pointee = if case == 2 {
            SemanticAbiPointeeKindV1::Raw
        } else {
            SemanticAbiPointeeKindV1::SharedReference { frozen: false }
        };
        types[2] = types[2].clone().with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                if case == 3 {
                    None
                } else {
                    Some(SemanticAbiPointeeInfoV1::new(pointee, 0, 4).unwrap())
                },
                None,
            ),
        );
        if case == 4 {
            types[0] = SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(116)),
                SemanticLayoutIdentityV1::from_sha256(bytes(116)),
                SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Char),
            );
        }
        if case == 5 || case == 6 {
            types[2] = volatile_load_source_types_v1(
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
                if case == 5 {
                    SemanticPointerKindV1::Raw
                } else {
                    SemanticPointerKindV1::Reference
                },
                if case == 6 {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
            )[2]
            .clone()
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(SemanticAbiPointeeInfoV1::new(pointee, 0, 4).unwrap()),
                    None,
                ),
            );
        }
        for reborrow in [false, true] {
            let base = if reborrow {
                shared_slice_reborrow_fixture()
            } else {
                fixture(false)
            };
            let abi = base
                .abi()
                .clone()
                .with_source_argument_ownership(vec![
                    if case == 1 {
                        SemanticSourceArgumentOwnershipV1::RawPointer
                    } else {
                        SemanticSourceArgumentOwnershipV1::SharedBorrow
                    },
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ])
                .unwrap();
            let function = SemanticFunctionDeclV1::new(
                base.identity(),
                base.role(),
                base.item_definition_identity(),
                base.monomorphization_identity(),
                base.generic_type_arguments_identity(),
                base.const_generic_arguments_identity(),
                base.source(),
                abi,
                base.locals().to_vec(),
                base.entry(),
                base.blocks().to_vec(),
            )
            .unwrap();
            let origins = local_stable_argument_origins(&types, &function).unwrap();
            assert_eq!(
                volatile_load_frozen_shared_scalar_source_v1(
                    &types,
                    &function,
                    &origins,
                    4,
                    SCALAR_TYPE
                ),
                case == 0,
                "case {case}, reborrow {reborrow}"
            );
        }
    }
}
