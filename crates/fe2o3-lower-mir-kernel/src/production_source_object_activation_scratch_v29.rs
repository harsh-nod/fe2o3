// Closed unit queries cannot export a borrow or a heap allocation. Their copied
// alternative is held in the caller's separately prepaid loop result slot.
type SourceActivationRefundFrameV29<'a, 'plan, 'source> = (
    Option<&'a SourceReferencePlanV29<'plan, 'source>>,
    fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    usize,
    usize,
    usize,
    Option<source_storage_v29::SourceStorageRootGrowthV29<'a, 'plan, 'source>>,
    Option<usize>,
    &'a mut dyn SemanticEmissionBudgetV1,
);

fn source_object_activation_scratch_headers_v29<F>() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            std::mem::size_of::<T>(),
            argument_product_v1(
                2,
                std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
            )?,
        ])
    }
    argument_sum_v1(&[
        h::<F>()?,
        std::mem::align_of::<F>(),
        h::<(
            &ExecutionInstancesV29<'_>,
            &SourceReferencePlanV29<'_, '_>,
            &mut ArgumentBudgetV1<'_>,
            F,
        )>()?,
        h::<&ExecutionInstancesV29<'_>>()?,
        h::<&SourceReferencePlanV29<'_, '_>>()?,
        h::<&mut ArgumentBudgetV1<'_>>()?,
        h::<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>()?,
        h::<Option<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()?,
        h::<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>()?,
        h::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()?,
        h::<&mut Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()?,
        h::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()?,
        argument_product_v1(4, h::<usize>()?)?,
        h::<Option<usize>>()?,
        h::<bool>()?,
        h::<Result<(), ProductionSemanticKirErrorV1>>()?,
        h::<std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>()?,
        h::<&std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>()?,
        h::<Box<dyn std::any::Any + Send>>()?,
        h::<&ProductionSemanticKirErrorV1>()?,
        h::<Option<ProductionSemanticKirErrorV1>>()?,
        h::<std::panic::AssertUnwindSafe<(&mut ArgumentBudgetV1<'_>, F)>>()?,
        h::<SourceActivationRefundFrameV29<'_, '_, '_>>()?,
        h::<Option<&SourceReferencePlanV29<'_, '_>>>()?,
        h::<&mut dyn SemanticEmissionBudgetV1>()?,
        h::<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>()?,
        h::<Option<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()?,
        h::<usize>()?,
        h::<bool>()?,
    ])
}

#[cfg(test)]
#[test]
fn activation_query_scratch_fixed_header_equation_is_explicit() {
    fn frame<T>() -> usize {
        std::mem::size_of::<T>()
            + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    type Callback = fn(&mut ArgumentBudgetV1<'_>) -> Result<(), ProductionSemanticKirErrorV1>;
    let expected = frame::<Callback>()
        + std::mem::align_of::<Callback>()
        + frame::<(
            &ExecutionInstancesV29<'_>,
            &SourceReferencePlanV29<'_, '_>,
            &mut ArgumentBudgetV1<'_>,
            Callback,
        )>()
        + frame::<&ExecutionInstancesV29<'_>>()
        + frame::<&SourceReferencePlanV29<'_, '_>>()
        + frame::<&mut ArgumentBudgetV1<'_>>()
        + frame::<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>()
        + frame::<Option<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()
        + frame::<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>()
        + frame::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
        + frame::<&mut Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
        + frame::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 4 * frame::<usize>()
        + frame::<Option<usize>>()
        + frame::<bool>()
        + frame::<Result<(), ProductionSemanticKirErrorV1>>()
        + frame::<std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>()
        + frame::<&std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>()
        + frame::<Box<dyn std::any::Any + Send>>()
        + frame::<&ProductionSemanticKirErrorV1>()
        + frame::<Option<ProductionSemanticKirErrorV1>>()
        + frame::<std::panic::AssertUnwindSafe<(&mut ArgumentBudgetV1<'_>, Callback)>>()
        + frame::<(
            Option<&SourceReferencePlanV29<'_, '_>>,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            usize,
            usize,
            usize,
            Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>,
            Option<usize>,
            &mut dyn SemanticEmissionBudgetV1,
        )>()
        + frame::<Option<&SourceReferencePlanV29<'_, '_>>>()
        + frame::<&mut dyn SemanticEmissionBudgetV1>()
        + frame::<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>()
        + frame::<Option<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
        + frame::<usize>()
        + frame::<bool>();
    assert_eq!(
        source_object_activation_scratch_headers_v29::<Callback>().unwrap(),
        expected
    );
    for remaining in [expected, expected - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 23 + remaining);
        budget.reserve_storage(23).unwrap();
        let result = budget
            .reserve_storage(source_object_activation_scratch_headers_v29::<Callback>().unwrap());
        if remaining == expected {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        } else {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == expected + 23 && error.limit() == expected + 22));
        }
        assert_eq!(budget.storage(), 23);
    }
}

fn source_object_activation_scratch_v29<'work, F>(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'work>,
    run: F,
) -> Result<(), ProductionSemanticKirErrorV1>
where
    F: FnOnce(&mut ArgumentBudgetV1<'work>) -> Result<(), ProductionSemanticKirErrorV1>,
{
    let result = (|| {
        plan.check_owner(instances, budget)?;
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let slot = budget
            .prepared_input_slot_v1()
            .ok_or(ArgumentResourceV1::Accounting)?;
        let root = plan.storage_root.as_ref();
        // This query never retains arena growth. Still use the original root's
        // growth-aware refund contract, including its sticky deny-refund bit.
        budget.charge_work(argument_sum_v1(&[
            12,
            if root.is_some() {
                source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29
            } else {
                0
            },
        ])?)?;
        let mut growth = None;
        let attempt = |budget: &mut ArgumentBudgetV1<'work>| {
            if let Some(root) = root {
                growth = Some(
                    root.capture_retained_growth()
                        .ok_or(ArgumentResourceV1::Accounting)?,
                );
            }
            run(budget)
        };
        let header = argument_sum_v1(&[
            source_object_activation_scratch_headers_v29::<F>()?,
            // The owned attempt and the catch closure containing it coexist
            // with their borrowed budget and AssertUnwindSafe representations.
            argument_product_v1(3, std::mem::size_of_val(&attempt))?,
            std::mem::align_of_val(&attempt),
            argument_product_v1(2, std::mem::size_of::<&mut ArgumentBudgetV1<'_>>())?,
        ])?;
        let required = argument_sum_v1(&[floor, header])?;
        budget.reserve_storage(header)?;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| attempt(budget)));
        if let Ok(Err(error)) = &caught {
            source_reference_record_failure_v29(plan, error);
        }
        let first = plan.failure.first_error();
        let refund = budget.storage().checked_sub(floor);
        let settled = scoped_emission_refund_v29(
            Some(plan),
            ledger,
            slot,
            floor,
            required,
            growth,
            refund,
            budget,
        );
        if !settled {
            plan.failure.record_resource(ArgumentResourceV1::Accounting);
        }
        match caught {
            Ok(Err(error)) => Err(first.unwrap_or(error)),
            Ok(Ok(())) => match plan.failure.first_error() {
                Some(error) => Err(error),
                None => Ok(()),
            },
            Err(payload) => std::panic::resume_unwind(payload),
        }
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

#[cfg(test)]
pub(super) fn check_object_activation_scratch_scope_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
    mode: u8,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // Scope controls use a genuine original plan, but their scalar output is
    // deliberately not an activation proof. Full source growth/replay tests
    // independently exercise the real direct/safe queries and retained rows.
    source_reference_emission_prepay_v29::<u32>(budget)?;
    source_reference_emission_prepay_v29::<Result<(), ProductionSemanticKirErrorV1>>(budget)?;
    source_reference_emission_prepay_v29::<
        std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>,
    >(budget)?;
    source_reference_emission_prepay_v29::<Box<dyn std::any::Any + Send>>(budget)?;
    source_reference_emission_prepay_v29::<&'static str>(budget)?;
    let floor = budget.storage();
    let mut value = 0u32;
    if mode == 0 {
        let mut peak = 0;
        for index in 0..128 {
            source_object_activation_scratch_v29(plan.instances, plan, budget, |budget| {
                plan.check_owner(plan.instances, budget)?;
                budget.reserve_storage(37)?;
                value = 17;
                Ok(())
            })?;
            assert_eq!((budget.storage(), value), (floor, 17));
            if index == 0 {
                peak = budget.peak_storage();
            } else {
                assert_eq!(budget.peak_storage(), peak);
            }
        }
        return Ok(());
    }
    if mode == 5 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(20_000_000);
        let mut foreign = ArgumentBudgetV1::new(&mut work, 20_000_000);
        let error =
            source_object_activation_scratch_v29(plan.instances, plan, &mut foreign, |_| {
                panic!("foreign scope callback entered")
            })
            .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        ));
        assert_eq!((foreign.work(), foreign.storage()), (0, 0));
        assert_eq!(budget.storage(), floor);
    } else {
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            source_object_activation_scratch_v29(plan.instances, plan, budget, |budget| {
                plan.check_owner(plan.instances, budget)?;
                budget.reserve_storage(37)?;
                value = 17;
                match mode {
                    1 => Err(source_reference_error_v29(
                        "activation scratch selected error",
                    )),
                    2 => std::panic::panic_any("activation scratch selected panic"),
                    3 => {
                        let more = budget.storage_limit() - budget.storage() + 1;
                        budget.reserve_storage(more)?;
                        unreachable!("one-short storage accepted")
                    }
                    4 => {
                        budget.charge_work(20_000_001 - budget.work())?;
                        unreachable!("one-short work accepted")
                    }
                    6 | 7 => {
                        // Undercut the new fixed frame, but remain strictly
                        // above the older caller floor. No refund is permitted.
                        budget.release_storage(budget.storage() - (floor + 1))?;
                        if mode == 7 {
                            std::panic::panic_any("activation scratch selected panic");
                        }
                        Err(source_reference_error_v29(
                            "activation scratch selected error",
                        ))
                    }
                    _ => unreachable!(),
                }
            })
        }));
        assert_eq!(value, 17);
        assert_eq!(budget.storage(), if mode >= 6 { floor + 1 } else { floor });
        match (mode, caught) {
            (1 | 6, Ok(Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. }))) => {
                assert_eq!(detail, "activation scratch selected error")
            }
            (2 | 7, Err(payload)) => {
                assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"activation scratch selected panic")
                );
                drop(payload);
            }
            (
                3,
                Ok(Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error),
                ))),
            ) => {
                assert_eq!(error.limit(), 20_000_000);
                assert_eq!(error.actual(), 20_000_001);
            }
            (
                4,
                Ok(Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error),
                ))),
            ) => {
                assert_eq!(error.limit(), 20_000_000);
                assert_eq!(error.actual(), 20_000_001);
            }
            (_, outcome) => panic!("activation scratch mode {mode}: {outcome:?}"),
        }
        if mode <= 2 {
            assert!(plan.failure.first_error().is_none());
            source_object_activation_scratch_v29(plan.instances, plan, budget, |budget| {
                plan.check_owner(plan.instances, budget)
            })?;
            assert_eq!(budget.storage(), floor);
            return Ok(());
        }
    }
    let first = plan.failure.get().expect("first resource retained");
    let before = (budget.work(), budget.storage());
    let replay = source_object_activation_scratch_v29(plan.instances, plan, budget, |_| {
        panic!("retained resource entered callback")
    })
    .unwrap_err();
    assert!(
        matches!(&replay, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)
        if *error == first)
    );
    assert_eq!((budget.work(), budget.storage()), before);
    Err(replay)
}
