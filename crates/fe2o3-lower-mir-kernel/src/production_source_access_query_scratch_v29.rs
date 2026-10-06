// The full access query returns owned/copy rows, not borrows into query scratch.
// Only actual returned vector capacities survive its completed local frames.
type SourceAccessQueryOutputV29 = (
    Vec<SourceAddressAccessSourceV29>,
    PendingSourceIssuedRolesV29,
);

fn source_access_query_scratch_headers_v29() -> Result<usize, ArgumentResourceV1> {
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
        h::<(
            &ExecutionInstancesV29<'_>,
            &SourceReferenceEmissionV29<'_, '_>,
            &SourceAddressSourceIndexV29<'_>,
            &OwnedScopedSourceSlotsV29,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<&SourceReferencePlanV29<'_, '_>>()?,
        h::<Option<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()?,
        h::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()?,
        argument_product_v1(5, h::<usize>()?)?,
        h::<Option<usize>>()?,
        h::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()?,
        h::<SourceAccessQueryOutputV29>()?,
        h::<Option<ProductionSemanticKirErrorV1>>()?,
        h::<&ProductionSemanticKirErrorV1>()?,
        h::<&SourceReferencePlanV29<'_, '_>>()?,
        h::<SourceActivationRefundFrameV29<'_, '_, '_>>()?,
        h::<Option<&SourceReferencePlanV29<'_, '_>>>()?,
        h::<&mut dyn SemanticEmissionBudgetV1>()?,
        h::<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>()?,
        h::<Option<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()?,
        h::<usize>()?,
        h::<bool>()?,
        h::<bool>()?,
        h::<Result<(), ArgumentResourceV1>>()?,
    ])
}

fn source_address_accesses_retained_v29(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceAccessQueryOutputV29, ProductionSemanticKirErrorV1> {
    let plan = references.plan;
    plan.check_owner(instances, budget)?;
    // Keep the wrapper's live result/refund frames paid in the enclosing owner.
    // The floor is taken after these headers, so the refund cannot erase them.
    budget
        .reserve_storage(source_access_query_scratch_headers_v29()?)
        .inspect_err(|error| plan.failure.record_resource(*error))?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget
        .prepared_input_slot_v1()
        .ok_or(ArgumentResourceV1::Accounting)?;
    let root = plan.storage_root.as_ref();
    plan.charge(
        argument_sum_v1(&[
            12,
            if root.is_some() {
                source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29
            } else {
                0
            },
        ])?,
        budget,
    )?;
    let growth = match root {
        Some(root) => Some(
            root.capture_retained_growth()
                .ok_or(ArgumentResourceV1::Accounting)?,
        ),
        None => None,
    };
    let output = source_address_accesses_v29(instances, references, source_index, slots, budget)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    if let Some(error) = plan.failure.first_error() {
        drop(output);
        return Err(error);
    }
    let retained = argument_sum_v1(&[
        argument_product_v1(
            output.0.capacity(),
            std::mem::size_of::<SourceAddressAccessSourceV29>(),
        )?,
        output.1.retained_storage(budget)?,
    ])?;
    let required = argument_sum_v1(&[floor, retained])?;
    let refund = budget.storage().checked_sub(required);
    if !scoped_emission_refund_v29(
        Some(plan),
        ledger,
        slot,
        floor,
        required,
        growth,
        refund,
        budget,
    ) {
        drop(output);
        plan.failure.record_resource(ArgumentResourceV1::Accounting);
        return Err(ArgumentResourceV1::Accounting.into());
    }
    #[cfg(test)]
    {
        let (calls, bytes, rows, retained_bytes) = SOURCE_ADDRESS_QUERY_SCRATCH_V29.get();
        SOURCE_ADDRESS_QUERY_SCRATCH_V29.set((
            calls.checked_add(1).unwrap(),
            bytes.checked_add(refund.unwrap()).unwrap(),
            rows.checked_add(output.0.len()).unwrap(),
            retained_bytes.checked_add(retained).unwrap(),
        ));
        assert_eq!(budget.storage(), required);
    }
    Ok(output)
}

#[cfg(test)]
std::thread_local! {
    pub(super) static SOURCE_ADDRESS_QUERY_SCRATCH_V29: std::cell::Cell<(usize, usize, usize, usize)> = const {
        std::cell::Cell::new((0, 0, 0, 0))
    };
}

#[cfg(test)]
#[test]
fn source_access_query_result_and_refund_headers_have_an_independent_equation() {
    fn h<T>() -> usize {
        std::mem::size_of::<T>()
            + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    let expected = h::<(
        &ExecutionInstancesV29<'_>,
        &SourceReferenceEmissionV29<'_, '_>,
        &SourceAddressSourceIndexV29<'_>,
        &OwnedScopedSourceSlotsV29,
        &mut ArgumentBudgetV1<'_>,
    )>() + h::<&SourceReferencePlanV29<'_, '_>>()
        + h::<Option<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()
        + h::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 5 * h::<usize>()
        + h::<Option<usize>>()
        + h::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
        + h::<(
            Vec<SourceAddressAccessSourceV29>,
            PendingSourceIssuedRolesV29,
        )>()
        + h::<Option<ProductionSemanticKirErrorV1>>()
        + h::<&ProductionSemanticKirErrorV1>()
        + h::<&SourceReferencePlanV29<'_, '_>>()
        + h::<(
            Option<&SourceReferencePlanV29<'_, '_>>,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            usize,
            usize,
            usize,
            Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>,
            Option<usize>,
            &mut dyn SemanticEmissionBudgetV1,
        )>()
        + h::<Option<&SourceReferencePlanV29<'_, '_>>>()
        + h::<&mut dyn SemanticEmissionBudgetV1>()
        + h::<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>()
        + h::<Option<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
        + h::<usize>()
        + h::<bool>()
        + h::<bool>()
        + h::<Result<(), ArgumentResourceV1>>();
    assert_eq!(source_access_query_scratch_headers_v29().unwrap(), expected);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 37 + expected - usize::from(short));
        budget.reserve_storage(37).unwrap();
        let result = budget.reserve_storage(source_access_query_scratch_headers_v29().unwrap());
        if short {
            assert!(
                matches!(result, Err(ArgumentResourceV1::Storage(error)) if error.actual() == 37 + expected && error.limit() == 36 + expected)
            );
            assert_eq!(budget.storage(), 37);
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), 37 + expected);
            budget.release_storage(expected).unwrap();
            assert_eq!(budget.storage(), 37);
        }
    }
}
