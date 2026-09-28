type ReferenceValidationArgumentsV29<'a, 'plan, 'source> = (
    &'a SourceReferencePlanV29<'plan, 'source>,
    &'a SemanticSourceReferenceBindingV29,
    &'a mut dyn SemanticEmissionBudgetV1,
);

type ReferenceValidationCaptureV29<'a, 'plan, 'source> = (
    &'a SourceReferencePlanV29<'plan, 'source>,
    &'a SemanticSourceReferenceBindingV29,
    &'a mut dyn SemanticEmissionBudgetV1,
    Option<&'a source_storage_v29::SourceStorageRootCustodyViewV29<'plan, 'source>>,
    &'a mut Option<source_storage_v29::SourceStorageRootGrowthV29<'a, 'plan, 'source>>,
);

fn source_reference_validation_headers_v29() -> Result<usize, ArgumentResourceV1> {
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
        // Entrance, scoped call, outer borrowed closure, and comparison body.
        argument_product_v1(4, h::<ReferenceValidationArgumentsV29<'_, '_, '_>>()?)?,
        h::<ReferenceValidationCaptureV29<'_, '_, '_>>()?,
        std::mem::align_of::<ReferenceValidationCaptureV29<'_, '_, '_>>(),
        h::<std::panic::AssertUnwindSafe<ReferenceValidationCaptureV29<'_, '_, '_>>>()?,
        h::<&SourceReferencePlanV29<'_, '_>>()?,
        h::<&SemanticSourceReferenceBindingV29>()?,
        h::<&mut dyn SemanticEmissionBudgetV1>()?,
        h::<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>()?,
        h::<Option<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()?,
        h::<&Option<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()?,
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
        h::<(
            Option<&SourceReferencePlanV29<'_, '_>>,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            usize,
            usize,
            usize,
            Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>,
            Option<usize>,
            &mut dyn SemanticEmissionBudgetV1,
        )>()?,
        h::<Option<&SourceReferencePlanV29<'_, '_>>>()?,
        h::<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>()?,
        h::<Option<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()?,
        h::<usize>()?,
        h::<bool>()?,
    ])
}

// The entrance has already checked the source owner and exact binding identity.
// Only the closed unit comparison runs here: no binding, Type, or arena row escapes.
fn source_reference_validate_payload_scoped_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    binding: &SemanticSourceReferenceBindingV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let result = (|| {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let slot = budget
            .prepared_input_slot_v1()
            .ok_or(ArgumentResourceV1::Accounting)?;
        let root = plan.storage_root.as_ref();
        budget.source_reference_charge_v29(
            plan,
            argument_sum_v1(&[
                12,
                if root.is_some() {
                    source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29
                } else {
                    0
                },
            ])?,
        )?;
        let header = source_reference_validation_headers_v29()?;
        let required = argument_sum_v1(&[floor, header])?;
        budget.source_reference_reserve_v29(plan, header)?;
        let mut growth = None;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(root) = root {
                growth = Some(
                    root.capture_retained_growth()
                        .ok_or(ArgumentResourceV1::Accounting)?,
                );
            }
            source_reference_validate_payload_inner_v29(plan, binding, budget)
        }));
        // The inner function has dropped every expected Type and candidate Vec.
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
