#[test]
fn selected_immutable_replay_frames_match_an_independent_type_oracle() {
    fn h<T>() -> usize {
        std::mem::size_of::<T>() + 2 * std::mem::size_of::<SourceOwnedResultV18<T>>()
    }
    let expected = h::<SourcePhysicalAccessV18<'_>>()
        + h::<SourcePhysicalPayloadV18<'_>>()
        + h::<Option<SourcePhysicalPayloadV18<'_>>>()
        + h::<&PendingSourceSelectedAccessV30>()
        + h::<&PendingSourceSelectedLeafV30>()
        + h::<&PendingSourceSelectedGuardV30>()
        + h::<&[SourceIssuedGuardV29]>()
        + h::<&SemanticPlaceV1>()
        + h::<&SemanticTypeDeclV1>()
        + h::<&ScopedMemoryAnchorsV29>()
        + h::<&ScopedMemoryAnchorV29>()
        + h::<&SourceReferenceSelectionActualNodeV30>()
        + h::<SourceReferenceSelectionSubjectV29>()
        + h::<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>()
        + h::<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>()
        + h::<(usize, usize)>()
        + h::<Option<(usize, usize)>>()
        + h::<usize>()
        + h::<()>();
    assert_eq!(
        scoped_raw_admission_v29::source_selected_replay_frame_headers_v30().unwrap(),
        expected
    );
    for short in [false, true] {
        let limit = 17 + expected - usize::from(short);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(17).unwrap();
        let result = budget.reserve_storage(
            scoped_raw_admission_v29::source_selected_replay_frame_headers_v30().unwrap(),
        );
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == 17 + expected && error.limit() == limit));
            assert_eq!(budget.failed_storage(), Some(17 + expected));
        } else {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        }
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn selected_common_immutable_replay_checks_the_full_receipt_before_singleton_final_refusal() {
    for fault in 0..8 {
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_selected_memory_owner(
            selection_owner(true, true),
            SELECTED_MEMORY_LIMIT,
            SELECTED_MEMORY_LIMIT,
            |original, budget| {
                check_selected_memory_rows(original, true, true, budget)?;
                let floor = budget.storage();
                let mut copy =
                    scoped_raw_admission_v29::issued_role_tests_v29::copied_issued_rows_v18(
                        retained_selected_memory(original),
                        budget,
                    )?;
                let owned = budget.storage() - floor;
                // An exact copied receipt is replayed independently against the
                // immutable source/actual graph, not accepted by pointer equality.
                scoped_raw_admission_v29::test_issued_copied_rows_replay_v26(
                    original, 0, &copy, budget,
                )?;
                match fault {
                    0 => copy.selected.clear(),
                    1 => copy.selected[0].selection.edges.reverse(),
                    2 => copy.selected[0].selection.nodes[0].pointer = ValueId(u32::MAX),
                    3 => copy.selected[0].guards[0].edge ^= 1,
                    4 => {
                        copy.selected[0].obligations[0].at_access =
                            !copy.selected[0].obligations[0].at_access
                    }
                    5 => copy.selected[0].subject.instance = ProductionCallInstanceIdV1(usize::MAX),
                    6 => copy.selected[0].memory.alignment *= 2,
                    7 => assert!(
                        copy.selected[0].selection.edges[0]
                            .argument
                            .take()
                            .is_some()
                    ),
                    _ => unreachable!(),
                }
                let error = scoped_raw_admission_v29::test_issued_copied_rows_replay_v26(
                    original, 0, &copy, budget,
                )
                .unwrap_err();
                assert!(
                    matches!(
                        error,
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "issued immutable receipt differs from its original owner"
                        )
                    ),
                    "fault {fault}: {error:?}"
                );
                drop(copy);
                budget.release_storage(owned)?;
                assert_eq!(budget.storage(), floor);
                let work = budget.work();
                assert!(matches!(
                    original.query(budget),
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "issued immutable receipt differs from its original owner"
                    ))
                ));
                assert_eq!(budget.work(), work);
                reached.set(true);
                Ok(())
            },
        );
        assert!(reached.get(), "fault {fault}: {result:?}");
        assert!(
            matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "issued immutable receipt differs from its original owner"
                ))
            ),
            "{result:?}"
        );
    }
    check_distinct_selection_memory();
}

#[test]
fn external_reborrow_type_replay_only_discards_mutable_permission() {
    let owner = mixed_selected_memory_owner();
    let types = owner.source_semantic().types();
    let shared: Vec<_> = types
        .iter()
        .enumerate()
        .filter(|(_, ty)| {
            matches!(ty.shape(),
        SemanticTypeShapeV1::Pointer(pointer) if pointer.kind() == SemanticPointerKindV1::Reference
            && pointer.metadata() == SemanticPointerMetadataV1::None
            && pointer.mutability() == SemanticMutabilityV1::Immutable)
        })
        .map(|(index, _)| SemanticTypeIdV1::from_index(index as u32))
        .collect();
    let [shared] = shared.as_slice() else {
        panic!("one original shared thin reference");
    };
    for (input, output, kind, expected) in [
        (REFERENCE, *shared, SemanticBorrowKindV1::Shared, true),
        (*shared, *shared, SemanticBorrowKindV1::Shared, true),
        (REFERENCE, REFERENCE, SemanticBorrowKindV1::Mutable, true),
        (*shared, REFERENCE, SemanticBorrowKindV1::Mutable, false),
        (REFERENCE, *shared, SemanticBorrowKindV1::Mutable, false),
        (*shared, REFERENCE, SemanticBorrowKindV1::Shared, false),
        (REFERENCE, REFERENCE, SemanticBorrowKindV1::Shared, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(17).unwrap();
        assert_eq!(
            source_external_reborrow_types_v30(types, input, output, kind, &mut budget).unwrap(),
            expected
        );
        assert_eq!(budget.work(), 24);
        let headers = std::mem::size_of::<bool>()
            + 2 * std::mem::size_of::<Result<bool, ProductionSemanticKirErrorV1>>();
        assert_eq!(budget.storage(), 17 + headers);
        budget.release_storage(headers).unwrap();
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn mixed_selected_shared_reborrow_emits_the_checked_permission_restriction() {
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = run_selected_memory_owner(
        mixed_selected_memory_owner(),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            let rows = retained_selected_memory(original);
            assert_eq!(rows.selected.len(), 1);
            let row = &rows.selected[0];
            assert!(!row.writing);
            let issuer = row
                .leaves
                .iter()
                .find_map(|leaf| match leaf.origin {
                    PendingSourceSelectedLeafOriginV30::Issued(issuer) => Some(issuer),
                    _ => None,
                })
                .expect("the separate original issued leaf");
            assert_eq!(issuer.access, AccessMode::ReadWrite);
            let function = original.inventory.functions()
                [original.source.root_row(0)?.function_ordinal]
                .function;
            let restrictions: Vec<_> = function.body.as_ref().unwrap().blocks.iter()
                .flat_map(|block| &block.operations).filter(|operation| matches!(&operation.kind,
                    OperationKind::Cast { kind: CastKind::RestrictPointerAccess, value, to: Type::Pointer(pointer) }
                        if *value == issuer.pointer && pointer.access == AccessMode::ReadOnly
                            && pointer.address_space == AddressSpace::Global
                            && *pointer.pointee == Type::Scalar(issuer.element))).collect();
            assert_eq!(restrictions.len(), 1);
            assert!(matches!(restrictions[0].results.as_slice(), [result]
                if matches!(&result.ty, Type::Pointer(pointer) if pointer.access == AccessMode::ReadOnly)));
            let error =
                scoped_raw_admission_v29::checked_issued_source_rows_v18(original, 0, budget)
                    .err()
                    .unwrap();
            assert!(matches!(
                error,
                ProductionSourceOwnedViewErrorV18::Binding(
                    "selected reference access requires its conditional V30 consumer"
                )
            ));
            reached.set(true);
            Ok(())
        },
    );
    assert!(reached.get(), "{result:?}");
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "selected reference access requires its conditional V30 consumer"
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn external_reborrow_type_replay_has_exact_resource_boundaries() {
    let owner = mixed_selected_memory_owner();
    let types = owner.source_semantic().types();
    let headers = std::mem::size_of::<bool>()
        + 2 * std::mem::size_of::<Result<bool, ProductionSemanticKirErrorV1>>();
    for (work_limit, storage_limit) in [(24, 17 + headers), (23, 17 + headers), (24, 16 + headers)]
    {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(17).unwrap();
        let result = source_external_reborrow_types_v30(
            types,
            REFERENCE,
            REFERENCE,
            SemanticBorrowKindV1::Mutable,
            &mut budget,
        );
        if work_limit == 23 {
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(error))) if error.actual() == 24 && error.limit() == 23)
            );
            assert_eq!(budget.storage(), 17 + headers);
            budget.release_storage(headers).unwrap();
        } else if storage_limit == 16 + headers {
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(error))) if error.actual() == 17 + headers
                    && error.limit() == storage_limit)
            );
            assert_eq!(budget.work(), 0);
        } else {
            assert!(result.unwrap());
            assert_eq!(budget.work(), 24);
            budget.release_storage(headers).unwrap();
        }
        assert_eq!(budget.storage(), 17);
    }
}
