#[test]
fn scoped_layout_producer_fixed_frames_have_an_independent_equation() {
    type E = ProductionSemanticKirErrorV1;
    type C = [u8; 17];
    type F = [usize; 3];
    type R = (Module, Vec<ScopedModuleRootV29>);
    type T = (Module, Vec<ScopedModuleRootV29>, usize);
    type L<'s> = source_storage_v29::SourceStorageLayoutsV29<'s>;
    type D<'s> = source_storage_demands_v29::SourceStorageDemandsV29<'s>;
    type Args<'a, 's, 'w> = (
        &'a ExecutionLifecycleSourceV29<'s>,
        ProductionSemanticKirLimitsV1,
        &'a ScopedSourceCleanupV29,
        &'a mut ArgumentBudgetV1<'w>,
    );
    type Read<'a, 's, 'w> = (C, &'a D<'s>, &'a mut L<'s>, &'a mut ArgumentBudgetV1<'w>);
    type Finish<'a, 's, 'w> = (F, R, L<'s>, D<'s>, &'a mut ArgumentBudgetV1<'w>);
    let expected = size_of::<C>()
        + std::mem::align_of::<C>()
        + size_of::<F>()
        + std::mem::align_of::<F>()
        + size_of::<Args<'_, '_, '_>>()
        + size_of::<(C, F, Args<'_, '_, '_>, usize)>()
        + size_of::<std::panic::AssertUnwindSafe<(C, F, Args<'_, '_, '_>, usize)>>()
        + size_of::<Read<'_, '_, '_>>()
        + size_of::<std::panic::AssertUnwindSafe<(Read<'_, '_, '_>, &ScopedSourceCleanupV29, usize)>>(
        )
        + size_of::<Finish<'_, '_, '_>>()
        + size_of::<L<'_>>()
        + size_of::<Result<L<'_>, E>>()
        + size_of::<D<'_>>()
        + size_of::<Result<D<'_>, E>>()
        + size_of::<&[SemanticTypeIdV1]>()
        + size_of::<Result<&[SemanticTypeIdV1], E>>()
        + size_of::<R>()
        + size_of::<T>()
        + size_of::<Result<R, E>>()
        + size_of::<Result<T, E>>()
        + size_of::<std::thread::Result<Result<R, E>>>()
        + size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<Box<dyn std::any::Any + Send>>()
        + 4 * size_of::<usize>()
        + size_of::<bool>();
    assert_eq!(
        scoped_source_layout_headers_v29::<R, T, C, F>().unwrap(),
        expected
    );
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let limit = MODULE_FLOOR + expected - usize::from(short);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let result =
            budget.reserve_storage(scoped_source_layout_headers_v29::<R, T, C, F>().unwrap());
        if short {
            assert!(
                matches!(result, Err(ArgumentResourceV1::Storage(error)) if error.actual() == MODULE_FLOOR + expected && error.limit() == limit)
            );
        } else {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        }
        assert_eq!((budget.work(), budget.storage()), (0, MODULE_FLOOR));
    }
}

#[test]
fn scoped_layout_cleanup_callback_and_attempt_frames_are_independent() {
    type T = [usize; 2];
    type E = ScopedModuleErrorV29;
    type F = [u8; 19];
    type RootCapture<'a, 'w> = (
        F,
        &'a ScopedSourceCleanupBoundaryV29,
        &'a mut ArgumentBudgetV1<'w>,
        Result<(), ArgumentResourceV1>,
    );
    let callback = size_of::<F>()
        + std::mem::align_of::<F>()
        + size_of::<RootCapture<'_, '_>>()
        + size_of::<std::panic::AssertUnwindSafe<RootCapture<'_, '_>>>()
        + size_of::<(F, &ScopedSourceCleanupV29, &mut ArgumentBudgetV1<'_>)>()
        + size_of::<T>()
        + size_of::<E>()
        + size_of::<Result<T, E>>()
        + size_of::<Result<(), ArgumentResourceV1>>()
        + size_of::<Result<usize, ArgumentResourceV1>>()
        + size_of::<Box<dyn std::any::Any + Send>>()
        + 2 * size_of::<usize>();
    assert_eq!(
        scoped_source_callback_headers_v29::<T, E, F>().unwrap(),
        callback
    );
    type Args<'a, 'w> = (
        &'a ScopedSourceCleanupV29,
        &'a mut ArgumentBudgetV1<'w>,
        usize,
        F,
    );
    type Capture<'a, 'w> = (
        F,
        &'a mut ArgumentBudgetV1<'w>,
        &'a mut usize,
        &'a mut usize,
    );
    let attempt = size_of::<F>()
        + std::mem::align_of::<F>()
        + size_of::<Args<'_, '_>>()
        + size_of::<Capture<'_, '_>>()
        + size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_>>>()
        + size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<Result<T, E>>()
        + size_of::<T>()
        + size_of::<E>()
        + size_of::<Box<dyn std::any::Any + Send>>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 7 * size_of::<usize>()
        + size_of::<bool>()
        + size_of::<Result<usize, ArgumentResourceV1>>()
        + size_of::<Result<(), ArgumentResourceV1>>();
    assert_eq!(
        scoped_source_attempt_headers_v29::<T, E, F>().unwrap(),
        attempt
    );
}

#[test]
fn scoped_layout_callback_first_header_denial_never_enters_consumer() {
    fn size<F>(_: &F) -> usize {
        scoped_source_callback_headers_v29::<(), ScopedModuleErrorV29, F>().unwrap()
    }
    for short in [false, true] {
        let entered = std::cell::Cell::new(false);
        let run = |_: &ScopedSourceCleanupV29, _: &mut ArgumentBudgetV1<'_>| {
            entered.set(true);
            Ok::<(), ScopedModuleErrorV29>(())
        };
        let callback = size(&run);
        let boundary = size_of::<ScopedSourceCleanupBoundaryV29>()
            + size_of::<std::thread::Result<Result<(), ScopedModuleErrorV29>>>();
        let limit = MODULE_FLOOR + boundary + callback - usize::from(short);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let result = with_scoped_source_cleanup_v29(&mut budget, MODULE_FLOOR, run);
        if short {
            assert!(
                matches!(result, Err(ScopedModuleErrorV29::Source(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error))))
                if error.actual() == limit + 1 && error.limit() == limit)
            );
        } else {
            result.unwrap();
        }
        assert_eq!(entered.get(), !short);
        assert_eq!((budget.work(), budget.storage()), (0, MODULE_FLOOR));
    }
}

#[test]
fn scoped_layout_adopted_input_first_attempt_header_denial_refunds_only_owned_credit() {
    let mut measured_header: Option<usize> = None;
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let source = owning_source_fixture(ModuleFixture::Ordinary, true, &mut budget).unwrap();
        let inherited = source.input.retained_storage;
        let rollback_floor = budget.storage() - inherited;
        let mut donor = Some(source);
        with_scoped_source_cleanup_v29(&mut budget, rollback_floor, |cleanup, budget| {
            let entry_parent_floor = budget.storage();
            let available = if short {
                measured_header.unwrap() - 1
            } else {
                0
            };
            let padding = MODULE_LIMIT - entry_parent_floor - available;
            budget.reserve_storage(padding)?;
            let before = (budget.work(), budget.storage());
            let result = SourceOwnedScopedModuleV29::try_new_with_cleanup(
                &mut donor,
                ProductionSemanticKirLimitsV1::default(),
                cleanup,
                budget,
            );
            let Err(ScopedModuleErrorV29::Source(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error),
                ),
            )) = result
            else {
                panic!("first adopted-input attempt header did not refuse storage");
            };
            assert!(donor.is_none(), "the genuine input was not adopted");
            assert_eq!(budget.work(), before.0, "constructor body was entered");
            assert_eq!(error.limit(), MODULE_LIMIT);
            assert_eq!(budget.failed_storage(), Some(error.actual()));
            let header = error.actual() - before.1;
            if short {
                assert_eq!(Some(header), measured_header);
                assert_eq!(error.actual(), MODULE_LIMIT + 1);
            } else {
                assert!(header > 1);
                measured_header = Some(header);
            }
            assert_eq!(budget.storage(), before.1 - inherited);
            assert!(!cleanup.is_denied());
            budget.release_storage(padding)?;
            assert_eq!(budget.storage(), entry_parent_floor - inherited);
            Ok::<(), ScopedModuleErrorV29>(())
        })
        .unwrap();
        assert_eq!(budget.storage(), rollback_floor);
    }
}

#[test]
fn scoped_layout_owned_entry_callback_header_denial_drops_the_original_donor() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let source = owning_source_fixture(ModuleFixture::Ordinary, true, &mut budget).unwrap();
    let inherited = source.input.retained_storage;
    let rollback_floor = budget.storage() - inherited;
    let boundary = size_of::<ScopedSourceCleanupBoundaryV29>()
        + size_of::<std::thread::Result<Result<SourceOwnedScopedModuleV29, ScopedModuleErrorV29>>>(
        );
    let padding = MODULE_LIMIT - budget.storage() - boundary;
    budget.reserve_storage(padding).unwrap();
    let before = (budget.work(), budget.storage());
    let mut donor = Some(source);
    let result = SourceOwnedScopedModuleV29::try_new(
        &mut donor,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    );
    let Err(ScopedModuleErrorV29::Source(
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(
            error,
        )),
    )) = result
    else {
        panic!("owned-entry callback header did not refuse storage");
    };
    assert!(donor.is_none());
    assert_eq!(budget.work(), before.0);
    assert_eq!(error.limit(), MODULE_LIMIT);
    assert!(error.actual() > MODULE_LIMIT);
    assert_eq!(budget.failed_storage(), Some(error.actual()));
    assert_eq!(budget.storage(), before.1 - inherited);
    budget.release_storage(padding).unwrap();
    assert_eq!(budget.storage(), rollback_floor);
}

#[test]
fn scoped_layout_foreign_source_preflight_precedes_new_wrapper_storage() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
        let entry_parent_floor = budget.storage();
        let owned = OwnedExecutionInputV29::capture(source, budget).unwrap();
        let launch = alternate_module_launch(source);
        let padding = MODULE_LIMIT - budget.storage();
        budget.reserve_storage(padding).unwrap();
        let before = budget.work();
        let result = owned.with_source::<()>(source.owner, &launch, budget, |_, _| {
            panic!("foreign launch entered source callback");
        });
        assert!(matches!(
            result,
            Err(ScopedModuleErrorV29::Source(
                ProductionSemanticKirErrorV1::Unsupported {
                    detail: "execution lifecycle differs from its retained source instance",
                    ..
                }
            ))
        ));
        assert_eq!(
            budget.work() - before,
            98 + owned.launch.len() * size_of::<crate::ProductionSourceLaunchRootV1>()
        );
        assert_eq!(budget.storage(), MODULE_LIMIT);
        assert_eq!(budget.failed_storage(), None);
        budget.release_storage(padding).unwrap();
        let retained_output = owned.retained_storage;
        drop(owned);
        budget.release_storage(retained_output).unwrap();
        assert_eq!(budget.storage(), entry_parent_floor);
    })
    .unwrap();
    assert_eq!(budget.storage(), 0);
}
