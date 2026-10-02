use fe2o3_mir_model::semantic_mir_v1::{
    SemanticLocalIdV1, SemanticMirErrorV1, SemanticProjectionKindV1, SemanticProjectionV1,
};

fn geometry_projection(
    kind: SemanticProjectionKindV1,
    ty: SemanticTypeIdV1,
) -> SemanticProjectionV1 {
    SemanticProjectionV1::new(kind, ty).unwrap()
}

#[test]
fn table_geometry_fixed_array_paths_keep_exact_original_offsets() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(113).unwrap();
    let layouts = SourceStorageLayoutsV29::new(&owner, &[ARRAY, HUGE], &mut budget).unwrap();
    for (offset, from_end) in [(3, false), (1, true)] {
        let path = [
            geometry_projection(
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length: 4,
                    from_end,
                },
                PAIR,
            ),
            geometry_projection(SemanticProjectionKindV1::Field(1), BYTE),
        ];
        let place = layouts
            .projected_subobject(&owner, ARRAY, &path, &mut budget)
            .unwrap();
        assert_eq!((place.range.start, place.range.length()), (56, 1));
        assert_eq!(
            place.path,
            [SubobjectStep::Element(3), SubobjectStep::Field(1)]
        );
        assert!(!place.has_selection(&mut budget).unwrap());
        place.discard(&mut budget).unwrap();
    }
    let root = layouts.root_subobject(&owner, HUGE, &mut budget).unwrap();
    let last = layouts
        .project_step(
            &root,
            SubobjectStep::Element(u64::from(u32::MAX) - 1),
            &mut budget,
        )
        .unwrap();
    assert_eq!(last.path.len(), 1);
    assert_eq!(last.range.start, 8 * (u64::from(u32::MAX) - 1));
    assert_eq!(last.range.length(), 8);
    last.discard(&mut budget).unwrap();
    root.discard(&mut budget).unwrap();
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 113);
}

#[test]
fn table_geometry_fields_unions_and_zero_sized_paths_keep_logical_identity() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(127).unwrap();
    let layouts =
        SourceStorageLayoutsV29::new(&owner, &[REORDERED, UNION, TWO_ZST], &mut budget).unwrap();
    for (ty, expected) in [
        (REORDERED, [(8, 1), (0, 8)]),
        (UNION, [(0, 8), (0, 1)]),
        (TWO_ZST, [(0, 0), (0, 0)]),
    ] {
        let root = layouts.root_subobject(&owner, ty, &mut budget).unwrap();
        let left = layouts
            .project_step(&root, SubobjectStep::Field(0), &mut budget)
            .unwrap();
        let right = layouts
            .project_step(&root, SubobjectStep::Field(1), &mut budget)
            .unwrap();
        assert_eq!((left.range.start, left.range.length()), expected[0]);
        assert_eq!((right.range.start, right.range.length()), expected[1]);
        assert_ne!(left.path, right.path);
        assert!(!left.selected && !right.selected);
        assert_eq!(left.initialization_floor, usize::from(ty == UNION));
        assert_eq!(right.initialization_floor, usize::from(ty == UNION));
        right.discard(&mut budget).unwrap();
        left.discard(&mut budget).unwrap();
        root.discard(&mut budget).unwrap();
    }
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 127);
}

#[test]
fn table_geometry_enum_paths_require_original_variant_before_field() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let layouts = SourceStorageLayoutsV29::new(&owner, &[DIRECT, NICHE], &mut budget).unwrap();
    for (ty, variant, field_ty, offset) in [
        (DIRECT, 0, WORD, 8),
        (DIRECT, 1, WORD, 8),
        (NICHE, 1, REFERENCE, 0),
    ] {
        let path = [
            geometry_projection(SemanticProjectionKindV1::Downcast(variant), ty),
            geometry_projection(SemanticProjectionKindV1::Field(0), field_ty),
        ];
        let place = layouts
            .projected_subobject(&owner, ty, &path, &mut budget)
            .unwrap();
        assert_eq!((place.range.start, place.range.length()), (offset, 8));
        assert_eq!(
            place.path,
            [SubobjectStep::Variant(variant), SubobjectStep::Field(0)]
        );
        place.discard(&mut budget).unwrap();
    }
    for path in [
        vec![geometry_projection(
            SemanticProjectionKindV1::Field(0),
            WORD,
        )],
        vec![geometry_projection(
            SemanticProjectionKindV1::Downcast(2),
            DIRECT,
        )],
    ] {
        assert!(
            layouts
                .projected_subobject(&owner, DIRECT, &path, &mut budget)
                .is_err()
        );
    }
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn table_geometry_rejects_foreign_owner_dynamic_and_invalid_fixed_paths() {
    assert_eq!(
        SemanticProjectionV1::new(
            SemanticProjectionKindV1::ConstantIndex {
                offset: 0,
                minimum_length: 4,
                from_end: true,
            },
            PAIR,
        ),
        Err(SemanticMirErrorV1::InvalidProjectionShape),
    );
    let owner = owner();
    let foreign = owner_with(types());
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let layouts = SourceStorageLayoutsV29::new(&owner, &[ARRAY], &mut budget).unwrap();
    let other = SourceStorageLayoutsV29::new(&owner, &[ARRAY], &mut budget).unwrap();
    assert!(
        layouts
            .root_subobject(&foreign, ARRAY, &mut budget)
            .is_err()
    );
    let foreign_root = other.root_subobject(&owner, ARRAY, &mut budget).unwrap();
    assert!(
        layouts
            .project_step(&foreign_root, SubobjectStep::Element(0), &mut budget)
            .is_err()
    );
    foreign_root.discard(&mut budget).unwrap();
    other.release(&mut budget).unwrap();
    for kind in [
        SemanticProjectionKindV1::ConstantIndex {
            offset: 4,
            minimum_length: 5,
            from_end: false,
        },
        SemanticProjectionKindV1::ConstantIndex {
            offset: 1,
            minimum_length: 5,
            from_end: true,
        },
        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(0)),
        SemanticProjectionKindV1::Dereference,
        SemanticProjectionKindV1::Field(0),
    ] {
        let path = [geometry_projection(kind, PAIR)];
        assert!(
            layouts
                .projected_subobject(&owner, ARRAY, &path, &mut budget)
                .is_err()
        );
    }
    let wrong_type = [geometry_projection(
        SemanticProjectionKindV1::ConstantIndex {
            offset: 0,
            minimum_length: 4,
            from_end: false,
        },
        WORD,
    )];
    assert!(
        layouts
            .projected_subobject(&owner, ARRAY, &wrong_type, &mut budget)
            .is_err()
    );
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn table_geometry_source_only_paths_do_not_create_a_physical_row() {
    let owner = owner();
    let foreign = owner_with(types());
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
    assert!(
        layouts
            .source_root_subobject(&foreign, TWO_ZST, &mut budget)
            .is_err()
    );
    assert!(
        layouts
            .source_root_subobject(&owner, SLICE, &mut budget)
            .is_err()
    );
    assert!(
        layouts
            .root_subobject(&owner, TWO_ZST, &mut budget)
            .is_err()
    );
    let path = [geometry_projection(
        SemanticProjectionKindV1::Field(1),
        UNIT,
    )];
    let place = layouts
        .source_projected_subobject(&owner, TWO_ZST, &path, &mut budget)
        .unwrap();
    assert_eq!((place.range.start, place.range.length()), (0, 0));
    assert_eq!(place.path, [SubobjectStep::Field(1)]);
    assert!(!place.has_selection(&mut budget).unwrap());
    place.discard(&mut budget).unwrap();
    assert!(layouts.rows(&owner, &mut budget).unwrap().is_empty());
    assert!(
        layouts
            .root_subobject(&owner, TWO_ZST, &mut budget)
            .is_err()
    );
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn table_geometry_root_and_field_have_independent_exact_and_one_short_budgets() {
    const WORK: usize = 2_000_000;
    const STORAGE: usize = 64 * 1024 * 1024;
    const FLOOR: usize = 139;
    for field in [false, true] {
        for boundary in 0..3 {
            let owner = owner();
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(FLOOR).unwrap();
            let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
            // Owner check; projection and step; empty path vector; first push/grow.
            let needed_work = if field { 1 + 1 + 1 + 3 + 2 + 3 } else { 1 };
            let place = size_of::<SourceStorageSubobjectV29<'_, '_>>();
            let needed_storage = if field {
                2 * place + 4 * size_of::<SubobjectStep>()
            } else {
                place
            };
            let available_work = needed_work - usize::from(boundary == 1);
            let available_storage = needed_storage - usize::from(boundary == 2);
            budget
                .charge_work(WORK - budget.work() - available_work)
                .unwrap();
            let pressure = STORAGE - budget.storage() - available_storage;
            budget.reserve_storage(pressure).unwrap();
            let result = if field {
                layouts.source_projected_subobject(
                    &owner,
                    TWO_ZST,
                    &[geometry_projection(
                        SemanticProjectionKindV1::Field(0),
                        UNIT,
                    )],
                    &mut budget,
                )
            } else {
                layouts.source_root_subobject(&owner, TWO_ZST, &mut budget)
            };
            if boundary == 0 {
                let place = result.unwrap();
                assert_eq!((place.range.start, place.range.length()), (0, 0));
                assert!(!place.selected);
                assert_eq!(budget.work(), WORK);
                assert_eq!(budget.peak_storage(), STORAGE);
                place.discard(&mut budget).unwrap();
                layouts.release(&mut budget).unwrap();
            } else {
                let error = match result {
                    Err(Error::ArgumentCorrespondenceResource(error)) => error,
                    Err(other) => panic!("unexpected geometry refusal: {other:?}"),
                    Ok(_) => panic!("short geometry budget accepted"),
                };
                match (boundary, error) {
                    (1, ArgumentResourceV1::Work(error)) => {
                        assert_eq!(error.actual(), WORK + 1);
                        assert_eq!(error.limit(), WORK);
                    }
                    (2, ArgumentResourceV1::Storage(error)) => {
                        assert_eq!(error.actual(), STORAGE + 1);
                        assert_eq!(error.limit(), STORAGE);
                    }
                    _ => panic!("wrong geometry resource refusal: {error:?}"),
                }
                let counters = (budget.work(), budget.storage(), budget.peak_storage());
                assert!(
                    matches!(layouts.lease.work(0, &mut budget), Err(Error::ArgumentCorrespondenceResource(found)) if found == error)
                );
                assert!(
                    matches!(layouts.lease.reserve(usize::MAX, &mut budget), Err(Error::ArgumentCorrespondenceResource(found)) if found == error)
                );
                assert_eq!(
                    counters,
                    (budget.work(), budget.storage(), budget.peak_storage())
                );
                assert!(
                    matches!(layouts.release(&mut budget), Err(Error::ArgumentCorrespondenceResource(found)) if found == error)
                );
            }
            assert_eq!(budget.storage(), FLOOR + pressure);
            budget.release_storage(pressure).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}
