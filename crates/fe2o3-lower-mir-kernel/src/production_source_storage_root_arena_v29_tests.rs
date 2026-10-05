use super::layout_tests::*;
use super::root_custody_tests::instances;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

#[test]
#[allow(forgetting_copy_types)]
fn source_storage_root_forgotten_handles_and_view_do_not_own_backing() {
    instances(|instances, budget| {
        let before = budget.storage();
        let mut layouts =
            SourceStorageLayoutsV29::new(instances.owner(), &[PAIR, WORD, REFERENCE], budget)
                .unwrap();
        let table = budget.storage();
        with_source_storage_root_v29(&mut layouts, instances, budget, |plan, root, budget| {
            let state = root.new_state(instances.root(), local_for(PAIR), budget)?;
            let path = root.root_path(PAIR, budget)?;
            root.mutate(
                state,
                path,
                SourceStorageRootMutationV29::Initialize,
                budget,
            )?;
            let copy = root.copy_state(state, budget)?;
            assert!(root.is_initialized(copy, path, budget)?);
            let origin = root.capture_origin(plan, 0, budget)?;
            assert_eq!(root.origin_count(origin, plan, budget)?, 1);
            assert_eq!(root.arena.states.borrow().len(), 2);
            assert_eq!(root.arena.paths.borrow().len(), 1);
            assert_eq!(root.arena.origins.borrow().len(), 1);
            std::mem::forget(state);
            std::mem::forget(copy);
            std::mem::forget(path);
            std::mem::forget(origin);
            std::mem::forget(root);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), table);
        layouts.release(budget).unwrap();
        assert_eq!(budget.storage(), before);
    });
}

#[test]
fn source_storage_root_partial_mutation_uses_the_existing_typed_domain() {
    instances(|instances, budget| {
        let mut layouts = SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], budget).unwrap();
        with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
            let state = root.new_state(instances.root(), local_for(PAIR), budget)?;
            let whole = root.root_path(PAIR, budget)?;
            let first = root.project(
                PAIR,
                &[SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), WORD).unwrap()],
                budget,
            )?;
            let second = root.project(
                PAIR,
                &[SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), BYTE).unwrap()],
                budget,
            )?;
            root.mutate(
                state,
                whole,
                SourceStorageRootMutationV29::Initialize,
                budget,
            )?;
            let copy = root.copy_state(state, budget)?;
            root.mutate(
                state,
                first,
                SourceStorageRootMutationV29::Deinitialize,
                budget,
            )?;
            assert!(!root.is_initialized(state, whole, budget)?);
            assert!(!root.is_initialized(state, first, budget)?);
            assert!(root.is_initialized(state, second, budget)?);
            assert!(root.is_initialized(copy, whole, budget)?);
            root.mutate(
                state,
                first,
                SourceStorageRootMutationV29::Initialize,
                budget,
            )?;
            assert!(root.is_initialized(state, whole, budget)?);
            Ok(())
        })
        .unwrap();
        layouts.release(budget).unwrap();
    });
}

#[test]
fn source_storage_root_recorded_question_mark_returns_the_original_error() {
    instances(|instances, budget| {
        let mut layouts = SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], budget).unwrap();
        let table = budget.storage();
        let result: Result<(), Error> =
            with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                root.new_state(
                    instances.root(),
                    SemanticLocalIdV1::from_index(u32::MAX),
                    budget,
                )?;
                panic!("recorded failure did not propagate")
            });
        assert!(matches!(result, Err(Error::Unsupported { .. })));
        assert_eq!(budget.storage(), table);
        assert!(layouts.release(budget).is_err());
    });
}

#[test]
fn source_storage_root_foreign_or_missing_handle_rejects_before_mutation() {
    instances(|instances, budget| {
        for foreign in [false, true] {
            let mut layouts =
                SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], budget).unwrap();
            let result =
                with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                    let state = root.new_state(instances.root(), local_for(PAIR), budget)?;
                    let path = root.root_path(PAIR, budget)?;
                    let invalid = SourceStorageStateHandleV29 {
                        arena: if foreign {
                            state.arena.wrapping_add(1)
                        } else {
                            state.arena
                        },
                        index: if foreign { state.index } else { usize::MAX },
                        view: PhantomData,
                    };
                    assert!(
                        root.mutate(
                            invalid,
                            path,
                            SourceStorageRootMutationV29::Initialize,
                            budget
                        )
                        .is_err()
                    );
                    assert!(root.arena.states.borrow()[state.index].facts.is_empty());
                    Ok(())
                })
                .unwrap_err();
            assert!(matches!(result, Error::Unsupported { .. }));
            assert!(layouts.release(budget).is_err());
        }
    });
}

#[test]
fn source_storage_root_later_scratch_cannot_be_spent_by_an_emission_abort_refund() {
    instances(|instances, budget| {
        let mut layouts = SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], budget).unwrap();
        let result: Result<(), Error> =
            with_source_storage_root_v29(&mut layouts, instances, budget, |plan, root, budget| {
                let emission = SourceReferenceEmissionV29::new(plan, budget)?;
                assert!(emission.owned > 0);
                let _ = root.new_state(instances.root(), local_for(PAIR), budget)?;
                let retained = root.arena.layouts.lease.root.get().unwrap();
                let required = retained
                    .required(root.arena.layouts.lease.owned.get())
                    .unwrap();
                // Both independent floors still hold, but returning emission-owned
                // credit would now undercut the composite root floor.
                let target = required.max(emission.floor);
                assert!(budget.storage() > target);
                budget.release_storage(budget.storage() - target)?;
                assert!(plan.retains_custody(instances, budget));
                let held = budget.storage();
                assert!(emission.abort_scope(instances, budget).is_err());
                assert_eq!(budget.storage(), held);
                Err(error("callback after invalid emission refund").into())
            });
        assert!(matches!(
            result,
            Err(Error::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert!(layouts.release(budget).is_err());
    });
}

#[test]
fn source_storage_root_state_vector_exact_and_short_storage_admit_before_mutation() {
    instances(|instances, budget| {
        for short in [false, true] {
            let mut layouts =
                SourceStorageLayoutsV29::new(instances.owner(), &[WORD], budget).unwrap();
            assert_eq!(layouts.keys, [RowKey::ty(WORD)]);
            let table = budget.storage();
            let mut padding = 0;
            let result =
                with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                    // Empty state constructor plus the first four-element arena
                    // allocation; no facts, enum rows or byte intervals exist yet.
                    let required = size_of::<SourceStorageStateV29<'_, '_>>()
                        + size_of::<Result<SourceStorageStateV29<'_, '_>, Error>>()
                        + 4 * size_of::<SourceStorageStateV29<'_, '_>>();
                    padding =
                        budget.storage_limit() - budget.storage() - required + usize::from(short);
                    budget.reserve_storage(padding)?;
                    let before = budget.storage();
                    let state = root.new_state(instances.root(), local_for(WORD), budget);
                    assert_eq!(state.is_err(), short);
                    assert_eq!(root.arena.states.borrow().len(), usize::from(!short));
                    if short {
                        assert_eq!(budget.failed_storage(), Some(before + required));
                        let work = budget.work();
                        assert!(root.root_path(WORD, budget).is_err());
                        assert_eq!(budget.work(), work);
                    } else {
                        assert_eq!(budget.storage(), before + required);
                    }
                    Ok(())
                });
            assert_eq!(result.is_err(), short);
            assert_eq!(budget.storage(), table + padding);
            assert_eq!(layouts.release(budget).is_err(), short);
            budget.release_storage(padding).unwrap();
        }
    });
}

#[test]
fn source_storage_root_state_vector_exact_and_short_work_preserve_denial_history() {
    instances(|instances, _| {
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
            let mut layouts =
                SourceStorageLayoutsV29::new(instances.owner(), &[WORD], &mut budget).unwrap();
            let table = budget.storage();
            let result = with_source_storage_root_v29(
                &mut layouts,
                instances,
                &mut budget,
                |_, root, budget| {
                    // Arena owner1 + state owner1 + singleton row lookup1 + push2
                    // + vector allocation3; its empty prior content costs zero.
                    let exact = 1 + 1 + 1 + 2 + 3;
                    budget.charge_work(usize::MAX - budget.work() - exact + usize::from(short))?;
                    let before = budget.work();
                    let state = root.new_state(instances.root(), local_for(WORD), budget);
                    assert_eq!(state.is_err(), short);
                    assert_eq!(root.arena.states.borrow().len(), usize::from(!short));
                    assert_eq!(
                        budget.work(),
                        before + if short { exact - 3 } else { exact }
                    );
                    Ok(())
                },
            );
            assert_eq!(result.is_err(), short);
            assert_eq!(budget.storage(), table);
            assert_eq!(layouts.release(&mut budget).is_err(), short);
            assert_eq!(budget.storage(), 0);
        }
    });
}
