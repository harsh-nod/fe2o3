use super::layout_tests::*;

mod enum_transfer_tests {
    use super::*;
    include!("production_source_storage_enum_transfer_v29_tests.rs");
}

fn run(
    consume: impl FnOnce(&ProductionCallInstancePlanV1<'_>, &mut Budget<'_>) -> Result<(), Error>,
) {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(consume(
                instances, budget,
            ))
        },
    )
    .unwrap()
    .unwrap();
}

#[test]
fn partial_initialization_move_and_reinitialization_preserve_siblings() {
    run(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], budget)?;
        let root = layouts.root_subobject(instances.owner(), PAIR, budget)?;
        let first = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
        let second = layouts.project_step(&root, SubobjectStep::Field(1), budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(PAIR),
            budget,
        )?;
        assert!(!state.is_initialized(&root, budget)?);
        state.initialize(&first, budget)?;
        assert!(state.is_initialized(&first, budget)?);
        assert!(!state.is_initialized(&second, budget)?);
        assert!(!state.is_initialized(&root, budget)?);
        state.initialize(&second, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        state.deinitialize(&first, budget)?;
        assert!(!state.is_initialized(&first, budget)?);
        assert!(state.is_initialized(&second, budget)?);
        assert!(!state.is_initialized(&root, budget)?);
        state.initialize(&first, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        state.discard(budget)?;
        first.discard(budget)?;
        second.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn aggregate_data_masks_exclude_padding_and_holes_without_expanding_arrays() {
    run(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[PAIR, HUGE], budget)?;
        let root = layouts.root_subobject(instances.owner(), PAIR, budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(PAIR),
            budget,
        )?;
        state.initialize(&root, budget)?;
        assert!(state.bytes_initialized(SourceStorageRangeV29::new(0, 9, 16)?, budget)?);
        assert!(!state.bytes_initialized(SourceStorageRangeV29::new(9, 7, 16)?, budget)?);
        assert!(!state.bytes_initialized(SourceStorageRangeV29::new(0, 16, 16)?, budget)?);
        let huge = layouts.root_subobject(instances.owner(), HUGE, budget)?;
        let last = layouts.project_step(
            &huge,
            SubobjectStep::Element(u64::from(u32::MAX) - 1),
            budget,
        )?;
        let mut large = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(HUGE),
            budget,
        )?;
        large.initialize(&huge, budget)?;
        assert!(large.is_initialized(&huge, budget)?);
        assert!(large.bytes_initialized(huge.range, budget)?);
        large.deinitialize(&last, budget)?;
        assert!(!large.is_initialized(&huge, budget)?);
        assert!(large.bytes_initialized(
            SourceStorageRangeV29::new(0, huge.range.length() - 8, huge.range.length())?,
            budget
        )?);
        assert!(!large.bytes_initialized(last.range, budget)?);
        large.initialize(&last, budget)?;
        assert!(large.is_initialized(&huge, budget)?);
        assert!(large.bytes_initialized(huge.range, budget)?);
        assert_eq!(large.facts.len(), 1);
        large.discard(budget)?;
        state.discard(budget)?;
        last.discard(budget)?;
        huge.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn union_writes_invalidate_overlapping_logical_values_but_preserve_initialized_bytes() {
    run(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[UNION], budget)?;
        let root = layouts.root_subobject(instances.owner(), UNION, budget)?;
        let word = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
        let byte = layouts.project_step(&root, SubobjectStep::Field(1), budget)?;
        assert_eq!((word.range.start, byte.range.start), (0, 0));
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(UNION),
            budget,
        )?;
        state.initialize(&word, budget)?;
        assert!(state.is_initialized(&word, budget)?);
        assert!(state.is_initialized(&root, budget)?);
        assert!(!state.is_initialized(&byte, budget)?);
        state.initialize(&byte, budget)?;
        assert!(!state.is_initialized(&word, budget)?);
        assert!(state.is_initialized(&byte, budget)?);
        assert!(state.is_initialized(&root, budget)?);
        assert!(state.bytes_initialized(word.range, budget)?);
        state.deinitialize(&byte, budget)?;
        assert!(!state.bytes_initialized(word.range, budget)?);
        state.discard(budget)?;
        word.discard(budget)?;
        byte.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn zero_sized_fields_have_distinct_logical_initialization() {
    run(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[TWO_ZST], budget)?;
        let root = layouts.root_subobject(instances.owner(), TWO_ZST, budget)?;
        let left = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
        let right = layouts.project_step(&root, SubobjectStep::Field(1), budget)?;
        assert_eq!(left.range, right.range);
        assert_ne!(left.path, right.path);
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(TWO_ZST),
            budget,
        )?;
        state.initialize(&left, budget)?;
        assert!(state.is_initialized(&left, budget)?);
        assert!(!state.is_initialized(&right, budget)?);
        assert!(!state.is_initialized(&root, budget)?);
        state.initialize(&right, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        state.deinitialize(&left, budget)?;
        assert!(state.is_initialized(&right, budget)?);
        state.discard(budget)?;
        left.discard(budget)?;
        right.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn storage_death_rebirth_and_join_never_restore_incoming_loans_or_old_contents() {
    run(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[WORD], budget)?;
        let root = layouts.root_subobject(instances.owner(), WORD, budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            SemanticLocalIdV1::from_index(1),
            budget,
        )?;
        state.initialize(&root, budget)?;
        let entry = state.copy(budget)?;
        state.storage_dead(budget)?;
        assert!(state.is_initialized(&root, budget).is_err());
        state.storage_live(budget)?;
        assert!(state.kills_incoming_loans);
        assert!(!state.is_initialized(&root, budget)?);
        state.initialize(&root, budget)?;
        let joined = state.join(&entry, budget)?;
        assert!(joined.kills_incoming_loans);
        let unrelated = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            SemanticLocalIdV1::from_index(2),
            budget,
        )?;
        assert!(state.join(&unrelated, budget).is_err());
        unrelated.discard(budget)?;
        joined.discard(budget)?;
        entry.discard(budget)?;
        state.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn repeated_sparse_joins_converge_and_release_scratch_credit() {
    run(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], budget)?;
        let root = layouts.root_subobject(instances.owner(), PAIR, budget)?;
        let first = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
        let mut left = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(PAIR),
            budget,
        )?;
        left.initialize(&root, budget)?;
        let mut right = left.copy(budget)?;
        right.deinitialize(&first, budget)?;
        let mut fixed = left.join(&right, budget)?;
        let stable_storage = budget.storage();
        for _ in 0..32 {
            let next = fixed.join(&right, budget)?;
            assert!(fixed.equivalent(&next, budget)?);
            fixed.discard(budget)?;
            fixed = next;
            assert_eq!(budget.storage(), stable_storage);
        }
        fixed.discard(budget)?;
        left.discard(budget)?;
        right.discard(budget)?;
        first.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn nested_sparse_join_is_idempotent_in_both_predecessor_orders_without_retained_growth() {
    run(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[ARRAY], budget)?;
        let root = layouts.root_subobject(instances.owner(), ARRAY, budget)?;
        let first = layouts.project_step(&root, SubobjectStep::Element(0), budget)?;
        let second = layouts.project_step(&root, SubobjectStep::Element(1), budget)?;
        let first_word = layouts.project_step(&first, SubobjectStep::Field(0), budget)?;
        let second_word = layouts.project_step(&second, SubobjectStep::Field(0), budget)?;
        let sibling = layouts.project_step(&second, SubobjectStep::Field(1), budget)?;
        let mut complete = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(ARRAY),
            budget,
        )?;
        complete.initialize(&root, budget)?;
        let mut partial = complete.copy(budget)?;
        partial.deinitialize(&first_word, budget)?;
        partial.deinitialize(&second_word, budget)?;
        for reversed in [false, true] {
            let mut fixed = if reversed {
                partial.join(&complete, budget)?
            } else {
                complete.join(&partial, budget)?
            };
            assert_eq!(fixed.facts.len(), 3);
            let retained = budget.storage();
            for iteration in 0..32 {
                let next = match iteration % 3 {
                    0 => fixed.join(&partial, budget)?,
                    1 => partial.join(&fixed, budget)?,
                    _ => fixed.join(&fixed, budget)?,
                };
                assert!(fixed.equivalent(&next, budget)?);
                assert!(!next.is_initialized(&first_word, budget)?);
                assert!(!next.is_initialized(&second_word, budget)?);
                assert!(next.is_initialized(&sibling, budget)?);
                fixed.discard(budget)?;
                fixed = next;
                assert_eq!(budget.storage(), retained);
            }
            fixed.discard(budget)?;
        }
        partial.discard(budget)?;
        complete.discard(budget)?;
        sibling.discard(budget)?;
        second_word.discard(budget)?;
        first_word.discard(budget)?;
        second.discard(budget)?;
        first.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn typed_overlapping_copy_snapshots_partial_masks_and_independent_siblings() {
    run(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[ARRAY], budget)?;
        let root = layouts.root_subobject(instances.owner(), ARRAY, budget)?;
        let first = layouts.project_step(&root, SubobjectStep::Element(0), budget)?;
        let second = layouts.project_step(&root, SubobjectStep::Element(1), budget)?;
        let source_word = layouts.project_step(&first, SubobjectStep::Field(0), budget)?;
        let destination_word = layouts.project_step(&second, SubobjectStep::Field(0), budget)?;
        let destination_byte = layouts.project_step(&second, SubobjectStep::Field(1), budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(ARRAY),
            budget,
        )?;
        state.initialize(&source_word, budget)?;
        state.copy_subobject_within(&second, &first, budget)?;
        assert!(state.is_initialized(&source_word, budget)?);
        assert!(state.is_initialized(&destination_word, budget)?);
        assert!(!state.is_initialized(&destination_byte, budget)?);
        assert!(state.bytes_initialized(destination_word.range, budget)?);
        assert!(!state.bytes_initialized(
            SourceStorageRangeV29::new(second.range.start + 9, 7, root.range.length())?,
            budget
        )?);
        state.deinitialize(&source_word, budget)?;
        assert!(state.is_initialized(&destination_word, budget)?);
        state.discard(budget)?;
        source_word.discard(budget)?;
        destination_word.discard(budget)?;
        destination_byte.discard(budget)?;
        first.discard(budget)?;
        second.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn physical_range_overflow_and_out_of_bounds_are_rejected() {
    assert!(SourceStorageRangeV29::new(u64::MAX, 1, u64::MAX).is_err());
    assert!(SourceStorageRangeV29::new(7, 2, 8).is_err());
    assert!(SourceStorageRangeV29::new(9, 0, 8).is_err());
    let empty = SourceStorageRangeV29::new(8, 0, 8).unwrap();
    assert!(!empty.overlaps(SourceStorageRangeV29::new(0, 8, 8).unwrap()));
    let interior = SourceStorageRangeV29::new(4, 0, 8).unwrap();
    let whole = SourceStorageRangeV29::new(0, 8, 8).unwrap();
    assert!(!interior.overlaps(whole));
    assert!(!whole.overlaps(interior));
}

#[test]
fn equivalent_constant_index_syntax_normalizes_to_the_same_array_subobject() {
    run(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[ARRAY], budget)?;
        // The admitted model permits both forms. This is not a claim that
        // rustc's Array producer emits reverse projections.
        let forward = SemanticProjectionV1::new(
            SemanticProjectionKindV1::ConstantIndex {
                offset: 3,
                minimum_length: 4,
                from_end: false,
            },
            PAIR,
        )
        .unwrap();
        let reverse = SemanticProjectionV1::new(
            SemanticProjectionKindV1::ConstantIndex {
                offset: 1,
                minimum_length: 4,
                from_end: true,
            },
            PAIR,
        )
        .unwrap();
        let a = layouts.projected_subobject(instances.owner(), ARRAY, &[forward], budget)?;
        let b = layouts.projected_subobject(instances.owner(), ARRAY, &[reverse], budget)?;
        assert_eq!(a.path, b.path);
        assert_eq!(a.range, b.range);
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(ARRAY),
            budget,
        )?;
        state.initialize(&a, budget)?;
        assert!(state.is_initialized(&b, budget)?);
        state.deinitialize(&b, budget)?;
        assert!(!state.is_initialized(&a, budget)?);
        state.discard(budget)?;
        a.discard(budget)?;
        b.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn enum_construction_tag_state_and_payload_initialization_are_independent() {
    run(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[DIRECT], budget)?;
        let root = layouts.root_subobject(instances.owner(), DIRECT, budget)?;
        let variant = layouts.project_step(&root, SubobjectStep::Variant(0), budget)?;
        let field = layouts.project_step(&variant, SubobjectStep::Field(0), budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(DIRECT),
            budget,
        )?;
        state.begin_variant(&root, 0, budget)?;
        state.initialize(&field, budget)?;
        assert!(!state.variant_is_readable(&root, 0, budget)?);
        assert!(!state.is_initialized(&root, budget)?);
        state.set_discriminant(&root, 0, budget)?;
        assert!(state.variant_is_readable(&root, 0, budget)?);
        assert!(state.is_initialized(&root, budget)?);
        state.deinitialize(&field, budget)?;
        assert!(state.variant_is_readable(&root, 0, budget)?);
        assert!(!state.is_initialized(&root, budget)?);
        state.initialize(&field, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        assert!(state.bytes_initialized(SourceStorageRangeV29::new(0, 1, 16)?, budget)?);
        assert!(!state.bytes_initialized(SourceStorageRangeV29::new(1, 7, 16)?, budget)?);
        state.discard(budget)?;
        field.discard(budget)?;
        variant.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn enum_join_preserves_variant_payload_correlation_without_choosing_a_predecessor() {
    run(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[DIRECT], budget)?;
        let root = layouts.root_subobject(instances.owner(), DIRECT, budget)?;
        let zero = layouts.project_step(&root, SubobjectStep::Variant(0), budget)?;
        let one = layouts.project_step(&root, SubobjectStep::Variant(1), budget)?;
        let zero_field = layouts.project_step(&zero, SubobjectStep::Field(0), budget)?;
        let one_field = layouts.project_step(&one, SubobjectStep::Field(0), budget)?;
        let mut left = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(DIRECT),
            budget,
        )?;
        let mut right = left.copy(budget)?;
        left.begin_variant(&root, 0, budget)?;
        left.initialize(&zero_field, budget)?;
        left.set_discriminant(&root, 0, budget)?;
        right.begin_variant(&root, 1, budget)?;
        right.initialize(&one_field, budget)?;
        right.set_discriminant(&root, 1, budget)?;
        let joined = left.join(&right, budget)?;
        assert!(joined.is_initialized(&root, budget)?);
        assert!(!joined.variant_is_readable(&root, 0, budget)?);
        assert!(!joined.variant_is_readable(&root, 1, budget)?);
        let unknown = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(DIRECT),
            budget,
        )?;
        let uninitialized = joined.join(&unknown, budget)?;
        assert!(!uninitialized.is_initialized(&root, budget)?);
        uninitialized.discard(budget)?;
        unknown.discard(budget)?;
        joined.discard(budget)?;
        left.discard(budget)?;
        right.discard(budget)?;
        zero_field.discard(budget)?;
        one_field.discard(budget)?;
        zero.discard(budget)?;
        one.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn typed_niche_payload_replacement_preserves_active_untagged_state_but_move_does_not() {
    run(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[NICHE], budget)?;
        let root = layouts.root_subobject(instances.owner(), NICHE, budget)?;
        let some = layouts.project_step(&root, SubobjectStep::Variant(1), budget)?;
        let field = layouts.project_step(&some, SubobjectStep::Field(0), budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(NICHE),
            budget,
        )?;
        state.begin_variant(&root, 1, budget)?;
        state.initialize(&field, budget)?;
        assert!(!state.variant_is_readable(&root, 1, budget)?);
        state.set_discriminant(&root, 1, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        state.initialize(&field, budget)?;
        assert!(state.variant_is_readable(&root, 1, budget)?);
        state.deinitialize(&field, budget)?;
        assert!(!state.variant_is_readable(&root, 1, budget)?);
        assert!(!state.is_initialized(&root, budget)?);
        state.discard(budget)?;
        field.discard(budget)?;
        some.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}
