#[test]
fn initialized_unknown_enums_expose_only_the_exact_direct_or_niche_tag() {
    run_extended(|instances, budget| {
        let floor = budget.storage();
        for (ty, variant, tag_end) in [(EMPTY_DIRECT, 0, 1), (NICHE, 1, 8)] {
            let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[ty], budget)?;
            let root = layouts.root_subobject(instances.owner(), ty, budget)?;
            let field = payload(&layouts, &root, variant, budget)?;
            let tag = SourceStorageRangeV29 {
                start: 0,
                end: tag_end,
            };
            let mut state = SourceStorageStateV29::new(
                &layouts,
                instances,
                instances.root(),
                local_for(ty),
                budget,
            )?;
            assert!(!state.discriminant_is_initialized(&root, budget)?);
            // Initialized bits alone do not prove a valid source enum value.
            state.update_bytes(tag, true, budget)?;
            assert!(state.bytes_initialized(tag, budget)?);
            assert!(!state.discriminant_is_initialized(&root, budget)?);
            state.deinitialize(&root, budget)?;
            // This is the mutation used by checked successful construction.
            state.initialize(&root, budget)?;
            assert!(state.initialized_bytes.is_empty());
            assert!(state.enums.is_empty());
            assert!(state.discriminant_is_initialized(&root, budget)?);
            assert!(state.bytes_initialized(tag, budget)?);
            assert!(!state.variant_is_readable(&root, variant, budget)?);
            assert!(!state.is_readable(&field, budget)?);
            if ty == EMPTY_DIRECT {
                assert!(
                    !state.bytes_initialized(SourceStorageRangeV29 { start: 1, end: 8 }, budget)?
                );
                assert!(
                    !state
                        .bytes_initialized(SourceStorageRangeV29 { start: 8, end: 16 }, budget)?
                );
                assert!(
                    !state.bytes_initialized(SourceStorageRangeV29 { start: 0, end: 2 }, budget)?
                );
            }
            let before = state.copy(budget)?;
            let mut selected = state.copy(budget)?;
            assert!(selected.restrict_discriminant(&root, &[variant], budget)?);
            assert!(selected.is_readable(&field, budget)?);
            assert!(state.equivalent(&before, budget)?);
            selected.deinitialize(&field, budget)?;
            assert!(!selected.is_readable(&field, budget)?);
            // The niche overlaps the moved payload; direct-tag bytes do not.
            if ty == NICHE {
                assert!(!selected.bytes_initialized(tag, budget)?);
                assert!(!selected.discriminant_is_initialized(&root, budget)?);
            }
            let mut copied = SourceStorageStateV29::new(
                &layouts,
                instances,
                instances.root(),
                local_for(ty),
                budget,
            )?;
            copied.copy_subobject_from(&root, &state, &root, budget)?;
            assert!(copied.discriminant_is_initialized(&root, budget)?);
            assert!(!copied.is_readable(&field, budget)?);
            copied.deinitialize(&root, budget)?;
            assert!(!copied.discriminant_is_initialized(&root, budget)?);
            state.storage_dead(budget)?;
            assert!(state.discriminant_is_initialized(&root, budget).is_err());
            state.storage_live(budget)?;
            assert!(!state.discriminant_is_initialized(&root, budget)?);
            state.initialize(&root, budget)?;
            assert!(state.discriminant_is_initialized(&root, budget)?);
            copied.discard(budget)?;
            selected.discard(budget)?;
            before.discard(budget)?;
            state.discard(budget)?;
            field.discard(budget)?;
            root.discard(budget)?;
            layouts.release(budget)?;
        }
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn direct_payload_construction_without_its_tag_does_not_become_readable() {
    run_extended(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[EMPTY_DIRECT], budget)?;
        let root = layouts.root_subobject(instances.owner(), EMPTY_DIRECT, budget)?;
        let field = payload(&layouts, &root, 0, budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(EMPTY_DIRECT),
            budget,
        )?;
        state.begin_variant(&root, 0, budget)?;
        state.initialize(&field, budget)?;
        assert!(state.bytes_initialized(field.range, budget)?);
        assert!(!state.discriminant_is_initialized(&root, budget)?);
        assert!(!state.is_readable(&field, budget)?);
        state.set_discriminant(&root, 0, budget)?;
        assert!(state.discriminant_is_initialized(&root, budget)?);
        assert!(state.is_readable(&field, budget)?);
        state.discard(budget)?;
        field.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

const TAG_DIRECT_ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(26);
const TAG_NICHE_ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(27);
const TAG_ARRAY_RECORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(28);

fn tag_data_types() -> Vec<SemanticTypeDeclV1> {
    let mut declarations = extended_types();
    for (tag, element, stride) in [(206, EMPTY_DIRECT, 16), (207, NICHE, 8)] {
        declarations.push(declaration(
            tag,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                stride * 3,
                8,
                SemanticFieldsShapeV1::array(stride, 3),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array { element, length: 3 },
        ));
    }
    declarations.push(declaration(
        208,
        SemanticTypeLayoutV1::aggregate(
            Some(72),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 48], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(vec![TAG_DIRECT_ARRAY, TAG_NICHE_ARRAY]).unwrap(),
        ),
    ));
    declarations
}

#[test]
fn whole_record_and_array_facts_cover_nested_tags_without_covering_enum_padding() {
    let owner = owner_with(tag_data_types());
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let result = (|| -> Result<(), Error> {
                let floor = budget.storage();
                let layouts =
                    SourceStorageLayoutsV29::new(instances.owner(), &[TAG_ARRAY_RECORD], budget)?;
                let root = layouts.root_subobject(instances.owner(), TAG_ARRAY_RECORD, budget)?;
                let direct_array = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
                let niche_array = layouts.project_step(&root, SubobjectStep::Field(1), budget)?;
                let direct =
                    layouts.project_step(&direct_array, SubobjectStep::Element(1), budget)?;
                let niche =
                    layouts.project_step(&niche_array, SubobjectStep::Element(2), budget)?;
                let field = payload(&layouts, &niche, 1, budget)?;
                let mut state = SourceStorageStateV29::new(
                    &layouts,
                    instances,
                    instances.root(),
                    local_for(TAG_ARRAY_RECORD),
                    budget,
                )?;
                state.initialize(&root, budget)?;
                assert!(state.discriminant_is_initialized(&direct, budget)?);
                assert!(state.discriminant_is_initialized(&niche, budget)?);
                assert!(!state.is_readable(&field, budget)?);
                assert!(
                    state
                        .bytes_initialized(SourceStorageRangeV29 { start: 16, end: 17 }, budget)?
                );
                assert!(
                    state
                        .bytes_initialized(SourceStorageRangeV29 { start: 48, end: 72 }, budget)?
                );
                assert!(
                    !state
                        .bytes_initialized(SourceStorageRangeV29 { start: 16, end: 24 }, budget)?
                );
                assert!(
                    !state
                        .bytes_initialized(SourceStorageRangeV29 { start: 24, end: 32 }, budget)?
                );
                state.deinitialize(&niche, budget)?;
                assert!(!state.discriminant_is_initialized(&niche, budget)?);
                assert!(
                    !state
                        .bytes_initialized(SourceStorageRangeV29 { start: 48, end: 72 }, budget)?
                );
                assert!(
                    state
                        .bytes_initialized(SourceStorageRangeV29 { start: 48, end: 64 }, budget)?
                );
                assert!(state.discriminant_is_initialized(&direct, budget)?);
                state.initialize(&niche, budget)?;
                assert!(state.discriminant_is_initialized(&niche, budget)?);
                assert!(!state.is_readable(&field, budget)?);
                state.discard(budget)?;
                field.discard(budget)?;
                niche.discard(budget)?;
                direct.discard(budget)?;
                niche_array.discard(budget)?;
                direct_array.discard(budget)?;
                root.discard(budget)?;
                layouts.release(budget)?;
                assert_eq!(budget.storage(), floor);
                Ok(())
            })();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn niche_tag_data_mask_uses_the_original_nonzero_source_offset() {
    for slice in [false, true] {
        let fixture = selected_niche_fixture_v29(slice, true);
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        let layouts = SourceStorageLayoutsV29::new(&fixture.owner, &[], &mut budget).unwrap();
        let offset = if slice { 16 } else { 8 };
        assert!(
            layouts
                .data_covers(
                    fixture.enumeration,
                    SourceStorageRangeV29 {
                        start: offset,
                        end: offset + 8
                    },
                    &mut budget,
                )
                .unwrap()
        );
        assert!(
            !layouts
                .data_covers(
                    fixture.enumeration,
                    SourceStorageRangeV29 { start: 0, end: 8 },
                    &mut budget,
                )
                .unwrap()
        );
        assert!(
            !layouts
                .data_covers(
                    fixture.enumeration,
                    SourceStorageRangeV29 {
                        start: offset - 1,
                        end: offset + 8
                    },
                    &mut budget,
                )
                .unwrap()
        );
        if slice {
            // The neighboring descriptor length is not the niche tag.
            assert!(
                !layouts
                    .data_covers(
                        fixture.enumeration,
                        SourceStorageRangeV29 {
                            start: offset + 8,
                            end: offset + 16
                        },
                        &mut budget,
                    )
                    .unwrap()
            );
        }
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn enum_tag_data_mask_has_independent_exact_work_and_storage_limits() {
    const WORK: usize = 20_000_000;
    const LIMIT: usize = 64 * 1024 * 1024;
    // Two vector constructors (3 each), task push (2), visit (1),
    // visited push (2) and its first backing allocation (3).
    const EXACT_WORK: usize = 14;
    type Task = (SemanticTypeIdV1, SourceStorageRangeV29);
    let exact_storage = 2 * size_of::<Vec<Task>>() + 5 * size_of::<Task>();
    for (ty, range) in [
        (DIRECT, SourceStorageRangeV29 { start: 0, end: 1 }),
        (NICHE, SourceStorageRangeV29 { start: 0, end: 8 }),
    ] {
        for (short_work, short_storage) in [(false, false), (true, false), (false, true)] {
            let owner = owner();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(269).unwrap();
            let layouts = SourceStorageLayoutsV29::new(&owner, &[ty], &mut budget).unwrap();
            let resident = budget.storage();
            let persistent = layouts.lease.persistent.get();
            let pressure = LIMIT - resident - exact_storage + usize::from(short_storage);
            budget.reserve_storage(pressure).unwrap();
            budget
                .charge_work(WORK - budget.work() - EXACT_WORK + usize::from(short_work))
                .unwrap();
            let before_work = budget.work();
            let result = layouts.data_covers(ty, range, &mut budget);
            match (short_work, short_storage) {
                (false, false) => {
                    assert!(result.unwrap());
                    assert_eq!(budget.work() - before_work, EXACT_WORK);
                    assert_eq!(budget.storage(), resident + pressure);
                }
                (true, false) => {
                    assert!(matches!(
                        result,
                        Err(Error::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        ))
                    ));
                    assert_eq!(budget.work() - before_work, 11);
                }
                (false, true) => {
                    assert!(matches!(
                        result,
                        Err(Error::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_)
                        ))
                    ));
                    assert_eq!(budget.work() - before_work, EXACT_WORK);
                }
                _ => unreachable!(),
            }
            assert_eq!(layouts.lease.persistent.get(), persistent);
            assert_eq!(
                layouts.release(&mut budget).is_err(),
                short_work || short_storage
            );
            assert_eq!(budget.storage(), 269 + pressure);
            budget.release_storage(pressure).unwrap();
            assert_eq!(budget.storage(), 269);
        }
    }
}
