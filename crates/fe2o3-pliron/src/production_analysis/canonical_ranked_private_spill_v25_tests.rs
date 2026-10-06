use super::*;

fn scalar_spill_module(scalar: ScalarType, mixed: bool) -> Module {
    let bytes = u64::from(scalar.bit_width().unwrap().div_ceil(8));
    let mut result = module(scalar, bytes, bytes as u32, true);
    let body = result.functions[0].body.as_mut().unwrap();
    let scalar_operations = [
        Operation::new(
            vec![ValueDef::new(
                ValueId(60),
                Type::pointer(
                    Type::Scalar(scalar),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            )],
            OperationKind::Alloca {
                element: Type::Scalar(scalar),
                count: None,
                address_space: AddressSpace::Private,
                alignment: bytes as u32,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(60),
                value: ValueId(20),
                access: MemoryAccess::new(AddressSpace::Private, bytes as u32),
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(61), Type::Scalar(scalar))],
            OperationKind::Load {
                pointer: ValueId(60),
                access: MemoryAccess::new(AddressSpace::Private, bytes as u32),
            },
        ),
    ];
    if mixed {
        body.blocks[0].operations.splice(3..3, scalar_operations);
    } else {
        body.blocks[0].operations.splice(0..3, scalar_operations);
    }
    result
}

#[test]
fn private_native_scalar_spills_and_typed_objects_complete_nine_real_stages() {
    for scalar in [
        ScalarType::Bool,
        ScalarType::I8,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::I128,
        ScalarType::U128,
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        for mixed in [false, true] {
            let finished = Cell::new(false);
            with_physical(
                &scalar_spill_module(scalar, mixed),
                |checked, physical, budget| {
                    let owner = physical.inventory().owner();
                    let floor = budget.storage();
                    with_pending_canonical_ranked_source_roles_v18(
                        checked,
                        LAYOUTS,
                        budget,
                        |pending, budget| {
                            pending
                                .with_private_memory_observations_v18(
                                    physical,
                                    budget,
                                    |view, budget| {
                                        assert!(std::ptr::eq(view.owner(budget)?, owner));
                                        assert_eq!(view.function_count(budget)?, 1);
                                        assert_eq!(view.paired_stage_count(0, budget)?, Some(9));
                                        assert!(view.report(0, budget)?.unwrap().is_clean());
                                        assert!(view.history(0, budget)?.is_some());
                                        assert_eq!(
                                            view.obligations(budget)?
                                                .iter()
                                                .filter(|row| row.requirement()
                                                    == CanonicalRankedSourceRequirementV18::Memory)
                                                .count(),
                                            if mixed { 6 } else { 3 }
                                        );
                                        assert!(!view.source_roles_are_complete());
                                        assert!(!view.ranked_verification_is_complete());
                                        assert!(!view.grants_artifact_or_launch_authority());
                                        finished.set(true);
                                        Ok(())
                                    },
                                )
                                .unwrap();
                            Ok(())
                        },
                    )
                    .unwrap();
                    assert_eq!(budget.storage(), floor);
                },
            );
            assert!(finished.get());
        }
    }
}

#[test]
fn private_native_scalar_spill_live_pointer_access_root_and_ordinal_mutations_refuse() {
    for fault in 0..9 {
        let finished = Cell::new(false);
        with_physical(
            &scalar_spill_module(ScalarType::U32, true),
            |checked, physical, budget| {
                let result = with_pending_canonical_ranked_source_roles_v18(
                    checked,
                    LAYOUTS,
                    budget,
                    |pending, budget| {
                        pending
                            .with_private_memory_observations_v18(
                                physical,
                                budget,
                                |view, budget| {
                                    assert_eq!(view.paired_stage_count(0, budget)?, Some(9));
                                    Ok(())
                                },
                            )
                            .unwrap();
                        let error = pending
                            .graph
                            .test_private_spill_fault_v25(physical, pending.epoch, fault, budget)
                            .unwrap_err();
                        if fault < 4 {
                            assert!(
                                matches!(error, Failure::ExactGraph | Failure::NativeSchema),
                                "fault {fault}: {error:?}"
                            );
                        } else {
                            assert!(
                                matches!(error, Failure::Mutation),
                                "fault {fault}: {error:?}"
                            );
                        }
                        finished.set(true);
                        Ok(())
                    },
                );
                if fault < 4 {
                    result.unwrap();
                } else {
                    assert!(matches!(result, Err(Failure::Mutation)));
                }
            },
        );
        assert!(finished.get());
    }
}

#[test]
fn private_native_scalar_spills_keep_foreign_physical_owner_closed() {
    let module = scalar_spill_module(ScalarType::U32, true);
    let finished = Cell::new(false);
    with_physical(&module, |checked, physical, budget| {
        with_physical(&module, |_, foreign, _| {
            assert_eq!(
                physical.inventory().owner().identity(),
                foreign.inventory().owner().identity()
            );
            assert!(!std::ptr::eq(
                physical.inventory().owner(),
                foreign.inventory().owner()
            ));
            with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    let error = pending
                        .with_private_memory_observations_v18(
                            foreign,
                            budget,
                            |_, _| -> Result<(), Failure> {
                                panic!("foreign spill proof reached consumer")
                            },
                        )
                        .unwrap_err();
                    assert!(matches!(error.failure(), Failure::ExactGraph));
                    finished.set(true);
                    Ok(())
                },
            )
            .unwrap();
        });
    });
    assert!(finished.get());
}
