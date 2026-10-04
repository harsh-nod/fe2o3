use super::layout_tests::*;

fn run(
    consume: impl FnOnce(&SourceReferencePlanV29<'_, '_>, &mut Budget<'_>) -> Result<(), Error>,
) {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result = with_source_reference_plan_v29(instances, budget, consume);
            budget.release_storage(budget.storage() - floor).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap()
    .unwrap();
}

#[test]
fn original_loan_sets_form_a_finite_sorted_idempotent_join() {
    run(|plan, budget| {
        assert_eq!(plan.loans.len(), 2);
        let layouts =
            SourceStorageLayoutsV29::new(plan.instances.owner(), &[WORD, REFERENCE], budget)?;
        let first = SourceStorageOriginsV29::capture(&layouts, plan, 0, budget)?;
        let second = SourceStorageOriginsV29::capture(&layouts, plan, 1, budget)?;
        let mut joined = second.copy(plan, budget)?;
        assert!(joined.merge(&first, plan, budget)?);
        assert_eq!(joined.keys(plan, budget)?.len(), 2);
        assert!(joined.keys(plan, budget)?[0] < joined.keys(plan, budget)?[1]);
        let stable = budget.storage();
        for _ in 0..32 {
            assert!(!joined.merge(&first, plan, budget)?);
            assert!(!joined.merge(&second, plan, budget)?);
            assert_eq!(joined.keys(plan, budget)?.len(), 2);
        }
        // Capacity may grow once to the fixed input-sum bound, never per visit.
        let after = budget.storage();
        assert!(after <= stable + 2 * size_of::<SourceStorageOriginKeyV29>());
        joined.discard(budget)?;
        second.discard(budget)?;
        first.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn joined_descriptors_preserve_correlated_data_and_length_alternatives() {
    run(|plan, budget| {
        let layouts =
            SourceStorageLayoutsV29::new(plan.instances.owner(), &[TWO_DESCRIPTORS], budget)?;
        let holder = SourceStorageStateV29::new(
            &layouts,
            plan.instances,
            plan.instances.root(),
            local_for(TWO_DESCRIPTORS),
            budget,
        )?;
        let first = SourceStorageOriginsV29::capture(&layouts, plan, 0, budget)?;
        let second = SourceStorageOriginsV29::capture(&layouts, plan, 1, budget)?;
        let representation = layouts.row_for(plan.instances.owner(), DESCRIPTOR, budget)?;
        let mut left = SourceStorageRelocationsV29::new(&holder, budget)?;
        let mut right = SourceStorageRelocationsV29::new(&holder, budget)?;
        left.replace(
            0,
            representation,
            &first,
            Some(SourceStorageDescriptorLengthV29::Constant(0)),
            plan,
            budget,
        )?;
        right.replace(
            0,
            representation,
            &second,
            Some(SourceStorageDescriptorLengthV29::Constant(1)),
            plan,
            budget,
        )?;
        let mut joined = left.join(&right, plan, budget)?;
        let alternatives = joined.read(0, representation, plan, budget)?;
        assert_eq!(alternatives.len(), 2);
        let (a, al) = alternatives[0].parts(plan, budget)?;
        let (b, bl) = alternatives[1].parts(plan, budget)?;
        assert!(a.same_keys(&first, budget)?);
        assert!(b.same_keys(&second, budget)?);
        assert_eq!(al, Some(SourceStorageDescriptorLengthV29::Constant(0)));
        assert_eq!(bl, Some(SourceStorageDescriptorLengthV29::Constant(1)));
        for _ in 0..16 {
            let next = joined.join(&left, plan, budget)?;
            assert_eq!(next.read(0, representation, plan, budget)?.len(), 2);
            joined.discard(budget)?;
            joined = next;
        }
        let reverse = right.join(&left, plan, budget)?;
        for (a, b) in joined
            .read(0, representation, plan, budget)?
            .iter()
            .zip(reverse.read(0, representation, plan, budget)?)
        {
            assert_eq!(a.compare(b, budget)?, std::cmp::Ordering::Equal);
        }
        reverse.discard(budget)?;
        joined.discard(budget)?;
        right.discard(budget)?;
        left.discard(budget)?;
        second.discard(budget)?;
        first.discard(budget)?;
        holder.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn copied_holder_is_independent_and_any_partial_overwrite_kills_its_complete_relocation() {
    run(|plan, budget| {
        let layouts =
            SourceStorageLayoutsV29::new(plan.instances.owner(), &[TWO_DESCRIPTORS], budget)?;
        let holder = SourceStorageStateV29::new(
            &layouts,
            plan.instances,
            plan.instances.root(),
            local_for(TWO_DESCRIPTORS),
            budget,
        )?;
        let first = SourceStorageOriginsV29::capture(&layouts, plan, 0, budget)?;
        let second = SourceStorageOriginsV29::capture(&layouts, plan, 1, budget)?;
        let representation = layouts.row_for(plan.instances.owner(), DESCRIPTOR, budget)?;
        let mut values = SourceStorageRelocationsV29::new(&holder, budget)?;
        values.replace(
            0,
            representation,
            &first,
            Some(SourceStorageDescriptorLengthV29::Constant(1)),
            plan,
            budget,
        )?;
        values.copy_within(16, SourceStorageRangeV29::new(0, 16, 32)?, plan, budget)?;
        values.replace(
            0,
            representation,
            &second,
            Some(SourceStorageDescriptorLengthV29::Constant(0)),
            plan,
            budget,
        )?;
        let copied = values.read(16, representation, plan, budget)?;
        assert!(copied[0].parts(plan, budget)?.0.same_keys(&first, budget)?);
        assert_eq!(
            copied[0].parts(plan, budget)?.1,
            Some(SourceStorageDescriptorLengthV29::Constant(1))
        );
        values.invalidate(SourceStorageRangeV29::new(24, 1, 32)?, budget)?;
        assert!(values.read(16, representation, plan, budget).is_err());
        assert!(values.read(0, representation, plan, budget).is_ok());
        values.copy_within(16, SourceStorageRangeV29::new(0, 8, 32)?, plan, budget)?;
        assert!(values.read(16, representation, plan, budget).is_err());
        values.discard(budget)?;
        second.discard(budget)?;
        first.discard(budget)?;
        holder.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn absent_predecessor_relocation_is_not_recovered_by_selecting_the_other_predecessor() {
    run(|plan, budget| {
        let layouts =
            SourceStorageLayoutsV29::new(plan.instances.owner(), &[TWO_DESCRIPTORS], budget)?;
        let holder = SourceStorageStateV29::new(
            &layouts,
            plan.instances,
            plan.instances.root(),
            local_for(TWO_DESCRIPTORS),
            budget,
        )?;
        let origins = SourceStorageOriginsV29::capture(&layouts, plan, 0, budget)?;
        let representation = layouts.row_for(plan.instances.owner(), DESCRIPTOR, budget)?;
        let mut initialized = SourceStorageRelocationsV29::new(&holder, budget)?;
        let absent = SourceStorageRelocationsV29::new(&holder, budget)?;
        initialized.replace(
            0,
            representation,
            &origins,
            Some(SourceStorageDescriptorLengthV29::Constant(1)),
            plan,
            budget,
        )?;
        let joined = initialized.join(&absent, plan, budget)?;
        assert!(joined.read(0, representation, plan, budget).is_err());
        assert!(
            initialized
                .replace(0, representation, &origins, None, plan, budget)
                .is_err()
        );
        assert!(SourceStorageOriginsV29::capture(&layouts, plan, usize::MAX, budget).is_err());
        joined.discard(budget)?;
        absent.discard(budget)?;
        initialized.discard(budget)?;
        origins.discard(budget)?;
        holder.discard(budget)?;
        layouts.release(budget)
    });
}
