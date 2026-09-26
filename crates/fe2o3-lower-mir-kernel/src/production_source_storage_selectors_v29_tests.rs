use super::layout_tests::*;

pub(super) fn run_selectors(
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
fn selected_reads_reduce_complete_concrete_coverage_with_the_exact_typed_suffix() {
    run_selectors(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[ARRAY], budget)?;
        let root = layouts.root_subobject(instances.owner(), ARRAY, budget)?;
        let selected = layouts.project_step(&root, SubobjectStep::Selection(1), budget)?;
        let field = layouts.project_step(&selected, SubobjectStep::Field(0), budget)?;
        let other = layouts.project_step(&root, SubobjectStep::Selection(2), budget)?;
        let mut state = SourceStorageStateV29::new(&layouts, instances, instances.root(), local_for(ARRAY), budget)?;
        for index in 0..4 {
            let element = layouts.project_step(&root, SubobjectStep::Element(index), budget)?;
            let first = layouts.project_step(&element, SubobjectStep::Field(0), budget)?;
            state.initialize(&first, budget)?;
            assert_eq!(state.is_initialized(&field, budget)?, index == 3);
            assert!(!state.is_initialized(&selected, budget)?);
            first.discard(budget)?;
            element.discard(budget)?;
        }
        for index in 0..4 {
            let element = layouts.project_step(&root, SubobjectStep::Element(index), budget)?;
            state.initialize(&element, budget)?;
            element.discard(budget)?;
        }
        assert!(state.is_initialized(&selected, budget)?);
        assert!(state.is_initialized(&root, budget)?);
        assert!(!state.facts.iter().any(|fact| fact.place.path.is_empty() && fact.initialized));
        let copied = state.copy(budget)?;
        let joined = state.join(&copied, budget)?;
        assert!(joined.is_initialized(&selected, budget)?);
        let fixed = layouts.project_step(&root, SubobjectStep::Element(0), budget)?;
        state.deinitialize(&fixed, budget)?;
        assert!(!state.is_initialized(&selected, budget)?);
        state.initialize(&fixed, budget)?;
        assert!(state.is_initialized(&selected, budget)?);
        state.deinitialize(&other, budget)?;
        assert!(!state.is_initialized(&selected, budget)?);
        for state in [state, copied, joined] { state.discard(budget)?; }
        for place in [fixed, field, selected, other, root] { place.discard(budget)?; }
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn sparse_selected_coverage_does_not_expand_an_uncovered_large_array() {
    run_selectors(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[HUGE], budget)?;
        let root = layouts.root_subobject(instances.owner(), HUGE, budget)?;
        let selected = layouts.project_step(&root, SubobjectStep::Selection(1), budget)?;
        let fixed = layouts.project_step(&root, SubobjectStep::Element(0), budget)?;
        let mut state = SourceStorageStateV29::new(&layouts, instances, instances.root(), local_for(HUGE), budget)?;
        state.initialize(&fixed, budget)?;
        assert!(!state.is_initialized(&selected, budget)?);
        state.discard(budget)?;
        for place in [fixed, selected, root] { place.discard(budget)?; }
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn nested_selected_reads_preserve_both_domains_and_the_field_suffix() {
    let mut types = types();
    let old = &types[HUGE.index() as usize];
    types[HUGE.index() as usize] = SemanticTypeDeclV1::new(old.identity(), old.layout_identity(),
        fe2o3_mir_model::semantic_mir_v1::SemanticTypeLayoutV1::with_exact_rustc_layout(128, 8,
            SemanticFieldsShapeV1::array(64, 2), SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true), None, false, None, 8, 0,
            SemanticTypeLayoutDetailsV1::None).unwrap(),
        SemanticTypeShapeV1::Array { element: ARRAY, length: 2 });
    let owner = owner_with(types);
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    production_call_instances_v1::with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let run = (|| -> Result<(), Error> {
            let floor = budget.storage();
            let layouts = SourceStorageLayoutsV29::new(&owner, &[HUGE], budget)?;
            let root = layouts.root_subobject(&owner, HUGE, budget)?;
            let outer = layouts.project_step(&root, SubobjectStep::Selection(1), budget)?;
            let inner = layouts.project_step(&outer, SubobjectStep::Selection(2), budget)?;
            let field = layouts.project_step(&inner, SubobjectStep::Field(0), budget)?;
            let mut state = SourceStorageStateV29::new(&layouts, instances, instances.root(), local_for(HUGE), budget)?;
            for i in 0..2 {
                let a = layouts.project_step(&root, SubobjectStep::Element(i), budget)?;
                for j in 0..4 {
                    let b = layouts.project_step(&a, SubobjectStep::Element(j), budget)?;
                    let f = layouts.project_step(&b, SubobjectStep::Field(0), budget)?;
                    state.initialize(&f, budget)?;
                    assert_eq!(state.is_initialized(&field, budget)?, i == 1 && j == 3);
                    assert!(!state.is_initialized(&inner, budget)?);
                    f.discard(budget)?;
                    b.discard(budget)?;
                }
                a.discard(budget)?;
            }
            let first = layouts.project_step(&root, SubobjectStep::Element(0), budget)?;
            state.deinitialize(&first, budget)?;
            assert!(!state.is_initialized(&field, budget)?);
            state.discard(budget)?;
            for place in [first, field, inner, outer, root] { place.discard(budget)?; }
            layouts.release(budget)?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        })();
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(run)
    }).unwrap().unwrap();
}

fn selected_coverage_query_boundary(limits: Option<(usize, usize, bool, bool)>) -> (usize, usize) {
    const WORK: usize = 20_000_000;
    const STORAGE: usize = 64 * 1024 * 1024;
    let owner = owner();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    production_call_instances_v1::with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[ARRAY], budget).unwrap();
        let root = layouts.root_subobject(&owner, ARRAY, budget).unwrap();
        let selected = layouts.project_step(&root, SubobjectStep::Selection(1), budget).unwrap();
        let mut state = SourceStorageStateV29::new(&layouts, instances, instances.root(), local_for(ARRAY), budget).unwrap();
        for index in 0..4 {
            let element = layouts.project_step(&root, SubobjectStep::Element(index), budget).unwrap();
            state.initialize(&element, budget).unwrap();
            element.discard(budget).unwrap();
        }
        let pressure = if let Some((work, storage, work_short, storage_short)) = limits {
            budget.charge_work(WORK - budget.work() - work + usize::from(work_short)).unwrap();
            STORAGE - budget.storage() - storage + usize::from(storage_short)
        } else {
            // Lift the query above construction's previous high-water mark.
            STORAGE / 2
        };
        budget.reserve_storage(pressure).unwrap();
        let before = (budget.work(), budget.storage());
        let result = state.is_initialized(&selected, budget);
        let used = (budget.work() - before.0, budget.peak_storage() - before.1);
        let resource = |error| match error {
            Error::ArgumentCorrespondenceResource(error) => error,
            other => panic!("unexpected selected-coverage failure: {other:?}"),
        };
        let first = match result {
            Ok(readable) => { assert!(readable); None },
            Err(error) => Some(resource(error)),
        };
        match limits {
            None => assert!(first.is_none()),
            Some((work, storage, false, false)) => {
                assert!(first.is_none());
                assert_eq!(used, (work, storage));
            }
            Some((_, _, true, false)) => assert!(matches!(first, Some(ArgumentResourceV1::Work(_)))),
            Some((_, _, false, true)) => assert!(matches!(first, Some(ArgumentResourceV1::Storage(_)))),
            Some((_, _, true, true)) => unreachable!(),
        }
        assert_eq!(layouts.lease.failure.resource(), first);
        if let Some(first) = first {
            let counters = (budget.work(), budget.storage());
            assert_eq!(resource(layouts.lease.work(usize::MAX, budget).unwrap_err()), first);
            assert_eq!(resource(layouts.lease.reserve(usize::MAX, budget).unwrap_err()), first);
            assert_eq!(counters, (budget.work(), budget.storage()));
        }
        drop(state);
        drop(selected);
        drop(root);
        assert_eq!(layouts.release(budget).err().map(resource), first);
        assert_eq!(budget.storage(), floor + pressure);
        budget.release_storage(pressure).unwrap();
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(used)
    }).unwrap()
}

#[test]
fn selected_coverage_query_has_exact_and_one_short_limits_with_sticky_cleanup() {
    let (work, storage) = selected_coverage_query_boundary(None);
    assert!(work > 0 && storage > 0);
    for (work_short, storage_short) in [(false, false), (true, false), (false, true)] {
        selected_coverage_query_boundary(Some((work, storage, work_short, storage_short)));
    }
}

#[test]
fn selected_write_proves_only_the_same_ssa_element_and_field() {
    run_selectors(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[ARRAY], budget)?;
        let root = layouts.root_subobject(instances.owner(), ARRAY, budget)?;
        let i = layouts.project_step(&root, SubobjectStep::Selection(1), budget)?;
        let j = layouts.project_step(&root, SubobjectStep::Selection(2), budget)?;
        let i0 = layouts.project_step(&i, SubobjectStep::Field(0), budget)?;
        let i1 = layouts.project_step(&i, SubobjectStep::Field(1), budget)?;
        let j0 = layouts.project_step(&j, SubobjectStep::Field(0), budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(ARRAY),
            budget,
        )?;
        state.initialize(&i0, budget)?;
        assert!(state.is_initialized(&i0, budget)?);
        assert!(!state.is_initialized(&i1, budget)?);
        assert!(!state.is_initialized(&j0, budget)?);
        assert!(!state.is_initialized(&root, budget)?);
        assert!(!state.bytes_initialized(root.range, budget)?);
        state.initialize(&i1, budget)?;
        assert!(state.is_initialized(&i, budget)?);
        assert!(!state.is_initialized(&root, budget)?);
        state.discard(budget)?;
        for place in [i0, i1, j0, i, j, root] {
            place.discard(budget)?;
        }
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn selected_holes_preserve_logical_siblings_and_exact_reinitialization() {
    run_selectors(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[ARRAY], budget)?;
        let root = layouts.root_subobject(instances.owner(), ARRAY, budget)?;
        let i = layouts.project_step(&root, SubobjectStep::Selection(1), budget)?;
        let j = layouts.project_step(&root, SubobjectStep::Selection(2), budget)?;
        let fixed = layouts.project_step(&root, SubobjectStep::Element(0), budget)?;
        let i0 = layouts.project_step(&i, SubobjectStep::Field(0), budget)?;
        let j0 = layouts.project_step(&j, SubobjectStep::Field(0), budget)?;
        let j1 = layouts.project_step(&j, SubobjectStep::Field(1), budget)?;
        let fixed1 = layouts.project_step(&fixed, SubobjectStep::Field(1), budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(ARRAY),
            budget,
        )?;
        state.initialize(&root, budget)?;
        state.deinitialize(&i0, budget)?;
        assert!(!state.is_initialized(&i0, budget)?);
        assert!(!state.is_initialized(&j0, budget)?);
        assert!(state.is_initialized(&j1, budget)?);
        assert!(state.is_initialized(&fixed1, budget)?);
        assert!(!state.is_initialized(&root, budget)?);
        state.initialize(&i0, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        assert_eq!(state.facts.len(), 1);
        state.discard(budget)?;
        for place in [i0, j0, j1, fixed1, i, j, fixed, root] {
            place.discard(budget)?;
        }
        layouts.release(budget)
    });
}

#[test]
fn selected_reinitialization_dominates_old_unknown_holes_but_not_future_moves() {
    run_selectors(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[HUGE], budget)?;
        let root = layouts.root_subobject(instances.owner(), HUGE, budget)?;
        let i = layouts.project_step(&root, SubobjectStep::Selection(1), budget)?;
        let j = layouts.project_step(&root, SubobjectStep::Selection(2), budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(HUGE),
            budget,
        )?;
        state.initialize(&root, budget)?;
        state.deinitialize(&i, budget)?;
        state.forget_selectors(&[1], budget)?;
        assert!(!state.is_initialized(&root, budget)?);
        assert!(!state.is_initialized(&i, budget)?);
        state.initialize(&i, budget)?;
        assert!(state.is_initialized(&i, budget)?);
        assert!(!state.is_initialized(&j, budget)?);
        assert!(!state.is_initialized(&root, budget)?);
        state.deinitialize(&j, budget)?;
        assert!(!state.is_initialized(&i, budget)?);
        assert!(state.facts.len() <= 3);
        state.discard(budget)?;
        for place in [i, j, root] {
            place.discard(budget)?;
        }
        layouts.release(budget)
    });
}

#[test]
fn selected_copy_join_and_repeated_widening_are_finite() {
    run_selectors(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[HUGE], budget)?;
        let root = layouts.root_subobject(instances.owner(), HUGE, budget)?;
        let i = layouts.project_step(&root, SubobjectStep::Selection(1), budget)?;
        let j = layouts.project_step(&root, SubobjectStep::Selection(2), budget)?;
        let mut a = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(HUGE),
            budget,
        )?;
        a.initialize(&i, budget)?;
        let b = a.copy(budget)?;
        let same = a.join(&b, budget)?;
        assert!(same.is_initialized(&i, budget)?);
        assert!(same.equivalent(&a, budget)?);
        a.copy_subobject_from(&j, &b, &i, budget)?;
        assert!(a.is_initialized(&j, budget)?);
        assert!(!a.is_initialized(&root, budget)?);
        a.forget_selectors(&[1, 2], budget)?;
        assert!(!a.is_initialized(&i, budget)?);
        assert!(!a.is_initialized(&j, budget)?);
        let empty = a.copy(budget)?;
        let before = budget.storage();
        for _ in 0..16 {
            a.forget_selectors(&[1, 2], budget)?;
        }
        assert_eq!(budget.storage(), before);
        assert!(a.equivalent(&empty, budget)?);
        for state in [a, b, same, empty] {
            state.discard(budget)?;
        }
        for place in [i, j, root] {
            place.discard(budget)?;
        }
        layouts.release(budget)
    });
}

#[test]
fn raw_index_projection_still_requires_an_original_source_selector() {
    run_selectors(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[HUGE], budget)?;
        let projection = SemanticProjectionV1::new(
            SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(1)),
            WORD,
        )
        .unwrap();
        assert!(
            layouts
                .projected_subobject(instances.owner(), HUGE, &[projection], budget)
                .is_err()
        );
        layouts.release(budget)
    });
}
