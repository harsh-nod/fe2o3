fn descriptor_length_only_owner_v30(
    metadata: bool,
    same_parameter: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let base = descriptor_source_owner(DescriptorCase::READ);
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let pointer = original.locals()[1].ty();
    let word = original.locals()[4].ty();
    let boolean = original.locals()[5].ty();
    let SemanticTypeShapeV1::Pointer(pointer_type) =
        source.types()[pointer.index() as usize].shape()
    else {
        panic!("genuine shared slice parameter");
    };
    let slice = pointer_type.pointee();
    let mut locals = original.locals().to_vec();
    let second = u32::try_from(locals.len()).unwrap();
    locals.push(local(240, word, SemanticLocalRoleV1::Temporary));
    let length = |holder| {
        if metadata {
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand: SemanticOperandV1::Copy(place(holder, pointer)),
            }
        } else {
            SemanticRvalueKindV1::Length(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(holder),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, slice)
                            .unwrap(),
                    ],
                    slice,
                )
                .unwrap(),
            )
        }
    };
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let blocks = vec![
        block(
            241,
            vec![
                assign(place(4, word), length(1)),
                assign(
                    place(second, word),
                    length(if same_parameter { 1 } else { 2 }),
                ),
                assign(
                    place(5, boolean),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: SemanticOperandV1::Copy(place(4, word)),
                        right: SemanticOperandV1::Copy(place(second, word)),
                    },
                ),
            ],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(5, boolean)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(242, vec![], SemanticTerminatorKindV1::Return),
        block(243, vec![], SemanticTerminatorKindV1::Return),
    ];
    let root = function(
        244,
        SemanticFunctionRoleV1::KernelRoot,
        original.abi().clone(),
        locals,
        blocks,
    )
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

fn descriptor_length_native_run_v30(
    metadata: bool,
    same_parameter: bool,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    let owner = descriptor_length_only_owner_v30(metadata, same_parameter);
    let abi = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
    let reached = std::cell::Cell::new(false);
    let (result, work, peak) = run_descriptor_role_owner_with_abi_v18(
        owner,
        abi,
        work,
        storage,
        |original, optimized, budget| {
            let floor = budget.storage();
            let output = optimized.output_inventory(budget)?;
            let lengths = output
                .operations()
                .iter()
                .filter(|row| matches!(row.operation.kind, OperationKind::SliceLength { .. }))
                .count();
            assert!(
                lengths > 0,
                "dynamic metadata must survive the real optimizer"
            );
            let launches = mixed_native_launches_v26(original, budget)?;
            let completed = with_mixed_source_completion_v26(
                original,
                optimized,
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
                &mut |native, budget| {
                    assert_eq!(native.source_census(budget)?[3], lengths);
                    assert!(native.runtime_occurrences(budget)?.is_empty());
                    let premises = native.runtime_premises(budget)?;
                    assert_eq!(premises.len(), 2);
                    for premise in premises {
                        assert_eq!(premise.access_counts(), [0, 0]);
                        assert!(!premise.requires_initialized_extent());
                        assert!(!premise.requires_exclusive_nonoverlapping_runtime_binding());
                    }
                    let globals = native.completed_globals_v30(original, optimized, budget)?;
                    for operation in output.operations() {
                        if matches!(operation.operation.kind, OperationKind::SliceLength { .. }) {
                            assert_eq!(
                                globals.exact_operation(operation.coordinate, budget)?,
                                Some(slice_view_v1::CompletedGlobalOperationV26::Length)
                            );
                        }
                    }
                    assert!(native.source_roles_are_complete());
                    assert!(!native.ranked_verification_is_complete());
                    assert!(!native.runtime_requirements_are_discharged());
                    assert!(!native.grants_artifact_or_launch_authority());
                    reached.set(true);
                    Ok(())
                },
            );
            if let Err(error) = completed {
                let error = ProductionAggregateSourceErrorV30::InitialCompletion(error);
                return match aggregate_source_resource_v30(&error) {
                    Some(resource) => Err(resource.into()),
                    None => panic!("genuine metadata native completion: {error:?}"),
                };
            }
            assert_eq!(budget.storage(), floor);
            Ok(())
        },
    );
    (result, work, peak, reached.get())
}

#[test]
fn descriptor_length_source_completion_covers_both_original_metadata_forms_without_memory_accesses()
{
    for metadata in [false, true] {
        for same_parameter in [false, true] {
            let (result, _, _, reached) = descriptor_length_native_run_v30(
                metadata,
                same_parameter,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
            );
            result.unwrap();
            assert!(reached);
        }
    }
}

#[test]
fn descriptor_length_source_completion_has_exact_and_one_short_whole_resources() {
    let (result, work, storage, reached) = descriptor_length_native_run_v30(
        false,
        false,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
    );
    result.unwrap();
    assert!(reached);
    let exact = descriptor_length_native_run_v30(false, false, work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (work, storage, true));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, accepted_work, accepted_storage, _) =
            descriptor_length_native_run_v30(false, false, work_limit, storage_limit);
        match (
            is_work,
            source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err()),
        ) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work_limit);
                assert_eq!(error.actual(), work);
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage_limit);
                assert_eq!(error.actual(), storage);
            }
            other => panic!("exact descriptor length resource refusal: {other:?}"),
        }
        assert!(accepted_work <= work_limit && accepted_storage <= storage_limit);
    }
}

#[test]
fn descriptor_length_source_completion_rejects_copied_locator_and_wrong_exact_definition_or_type() {
    for fault in 0..4 {
        let owner = descriptor_length_only_owner_v30(false, false);
        let abi = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_descriptor_length_refusal_v30(
                    original, optimized, fault, &reached, budget,
                )
            },
        );
        assert!(reached.get());
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
        ));
    }
    let restored = descriptor_length_native_run_v30(
        false,
        false,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
    );
    restored.0.unwrap();
    assert!(restored.3);
}

#[test]
fn descriptor_length_source_completion_rejects_another_same_typed_source_receiver() {
    for metadata in [false, true] {
        let owner = descriptor_length_only_owner_v30(metadata, false);
        let abi = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_descriptor_length_refusal_v30(
                    original, optimized, 4, &reached, budget,
                )
            },
        );
        assert!(reached.get());
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "descriptor operand archived receiver differs"
            ))
        ));
        let restored = descriptor_length_native_run_v30(
            metadata,
            false,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        restored.0.unwrap();
        assert!(restored.3);
    }
}
