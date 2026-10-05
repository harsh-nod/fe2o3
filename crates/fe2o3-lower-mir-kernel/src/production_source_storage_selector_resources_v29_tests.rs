use super::layout_tests::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectorBoundary {
    Exact,
    WorkShort,
    StorageShort,
}

fn selected_projection_boundary(boundary: SelectorBoundary, ty: SemanticTypeIdV1) {
    const WORK: usize = 2_000_000;
    const STORAGE: usize = 64 * 1024 * 1024;
    let owner = owner();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner, ROOT, &mut budget, |_, budget| {
            let floor = budget.storage();
            let layouts = SourceStorageLayoutsV29::new(&owner, &[ty], budget).unwrap();
            let root = layouts.root_subobject(&owner, ty, budget).unwrap();
            // Typed step1; copy empty path vector3; path push2; its first
            // capacity-four vector3. Extent and runtime index never add rows.
            let exact_work = 1 + 3 + 2 + 3;
            let exact_storage = size_of::<SourceStorageSubobjectV29<'_, '_>>()
                + 4 * size_of::<SubobjectStep>();
            let allowed_work = exact_work - usize::from(boundary == SelectorBoundary::WorkShort);
            let allowed_storage = exact_storage - usize::from(boundary == SelectorBoundary::StorageShort);
            budget.charge_work(WORK - budget.work() - allowed_work).unwrap();
            let pressure = STORAGE - budget.storage() - allowed_storage;
            budget.reserve_storage(pressure).unwrap();
            let before = (budget.work(), budget.storage());
            let result = layouts.project_step(&root, SubobjectStep::Selection(7), budget).map(drop);
            let resource = |error| match error {
                Error::ArgumentCorrespondenceResource(error) => error,
                other => panic!("unexpected error: {other:?}"),
            };
            let first = result.err().map(resource);
            match boundary {
                SelectorBoundary::Exact => {
                    assert!(first.is_none());
                    assert_eq!(budget.work(), before.0 + exact_work);
                    assert_eq!(budget.peak_storage(), before.1 + exact_storage);
                }
                SelectorBoundary::WorkShort => assert!(matches!(first,
                    Some(ArgumentResourceV1::Work(error)) if error.actual() == before.0 + exact_work && error.limit() == WORK)),
                SelectorBoundary::StorageShort => assert!(matches!(first,
                    Some(ArgumentResourceV1::Storage(error)) if error.actual() == before.1 + exact_storage && error.limit() == STORAGE)),
            }
            assert_eq!(layouts.lease.failure.resource(), first);
            if let Some(first) = first {
                let counters = (budget.work(), budget.storage());
                assert_eq!(resource(layouts.lease.work(usize::MAX, budget).unwrap_err()), first);
                assert_eq!(resource(layouts.lease.reserve(usize::MAX, budget).unwrap_err()), first);
                assert_eq!(counters, (budget.work(), budget.storage()));
            }
            drop(root);
            assert_eq!(layouts.release(budget).err().map(resource), first);
            assert_eq!(budget.storage(), floor + pressure);
            budget.release_storage(pressure).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    ).unwrap();
}

#[test]
fn checked_symbolic_step_has_independent_exact_and_one_short_limits() {
    for ty in [BYTE_OCTET, HUGE] {
        for boundary in [
            SelectorBoundary::Exact,
            SelectorBoundary::WorkShort,
            SelectorBoundary::StorageShort,
        ] {
            selected_projection_boundary(boundary, ty);
        }
    }
}

#[test]
fn symbolic_overlap_work_depends_on_path_depth_not_array_extent() {
    super::selector_tests::run_selectors(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[], budget)?;
        let left = [SubobjectStep::Selection(1), SubobjectStep::Field(0)];
        let right = [SubobjectStep::Selection(2), SubobjectStep::Field(1)];
        let before = (budget.work(), budget.storage());
        assert!(!selected_paths_overlap(
            &left,
            &right,
            &layouts.lease,
            budget
        )?);
        assert_eq!(budget.work() - before.0, 1 + left.len().min(right.len()));
        assert_eq!(budget.storage(), before.1);
        layouts.release(budget)
    });
}
