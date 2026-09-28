// Original safe loans can cross call instances. Their referent identity comes
// from exact Borrow occurrences, never from an equal physical slot or pointer.
fn source_safe_object_invocation_headers_v29() -> Result<usize, ArgumentResourceV1> {
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
        h::<SourceSafeObjectOriginV29>()?,
        h::<Option<SourceSafeObjectOriginV29>>()?,
        h::<SourceDirectObjectOriginV29>()?,
        h::<SourceReferenceAccessKeyV29>()?,
        h::<ScopedMemoryFrameV29>()?,
        h::<ScopedMemoryRoleV29>()?,
        h::<Option<ScopedMemoryRoleV29>>()?,
        h::<(u32, Option<u32>)>()?,
        h::<SourceReferenceSiteV29>()?,
        h::<u32>()?,
        h::<Option<u32>>()?,
        h::<ExecutionOperandV29>()?,
        h::<bool>()?,
        h::<Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>>()?,
        h::<&production_call_instances_v1::ProductionCallInstanceV1<'_>>()?,
        h::<&SemanticFunctionDeclV1>()?,
        h::<Option<&SemanticPlaceV1>>()?,
        h::<&SemanticPlaceV1>()?,
        h::<Option<&SemanticOperandV1>>()?,
        h::<&SemanticOperandV1>()?,
        h::<SourceObjectLoanV29>()?,
        h::<Option<SourceObjectLoanV29>>()?,
        h::<Option<&SourceReferenceLoanV29>>()?,
        h::<&SourceReferenceLoanV29>()?,
        h::<Option<&SourceReferenceOriginV29>>()?,
        h::<&SourceReferenceOriginV29>()?,
        h::<&SourceReferenceAccessRecordV29>()?,
        h::<production_call_instances_v1::ProductionInstanceBorrowV1<'_>>()?,
        h::<
            Result<
                production_call_instances_v1::ProductionInstanceBorrowV1<'_>,
                production_call_instances_v1::ProductionCallInstanceErrorV1,
            >,
        >()?,
        h::<SemanticBorrowKindV1>()?,
        h::<SourceReferenceAccessV29>()?,
        h::<Option<usize>>()?,
        h::<usize>()?,
        h::<usize>()?,
        h::<usize>()?,
        h::<PendingSourceMemoryAlternativeV29>()?,
        h::<Option<PendingSourceMemoryAlternativeV29>>()?,
        h::<()>()?,
        h::<(
            &ExecutionInstancesV29<'_>,
            &SourceReferencePlanV29<'_, '_>,
            &OwnedScopedSourceSlotsV29,
            &SourceAddressAccessSourceV29,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        // Function arguments coexist with the immediate closure captures.
        h::<(
            &ExecutionInstancesV29<'_>,
            &SourceReferencePlanV29<'_, '_>,
            &OwnedScopedSourceSlotsV29,
            &SourceAddressAccessSourceV29,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
    ])
}

#[cfg(test)]
#[test]
fn safe_object_activation_fixed_headers_match_independent_equation_and_cuts() {
    use std::mem::size_of;
    type Error = ProductionSemanticKirErrorV1;
    macro_rules! frames {
        ($($ty:ty),* $(,)?) => { 0usize $(+ size_of::<$ty>() + 2 * size_of::<Result<$ty, Error>>())* };
    }
    type Call<'a, 's, 'w> = (
        &'a ExecutionInstancesV29<'s>,
        &'a SourceReferencePlanV29<'s, 'a>,
        &'a OwnedScopedSourceSlotsV29,
        &'a SourceAddressAccessSourceV29,
        &'a mut ArgumentBudgetV1<'w>,
    );
    let expected = frames![
        SourceSafeObjectOriginV29,
        Option<SourceSafeObjectOriginV29>,
        SourceDirectObjectOriginV29,
        SourceReferenceAccessKeyV29,
        ScopedMemoryFrameV29,
        ScopedMemoryRoleV29,
        Option<ScopedMemoryRoleV29>,
        (u32, Option<u32>),
        SourceReferenceSiteV29,
        u32,
        Option<u32>,
        ExecutionOperandV29,
        bool,
        Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>,
        &production_call_instances_v1::ProductionCallInstanceV1<'_>,
        &SemanticFunctionDeclV1,
        Option<&SemanticPlaceV1>,
        &SemanticPlaceV1,
        Option<&SemanticOperandV1>,
        &SemanticOperandV1,
        SourceObjectLoanV29,
        Option<SourceObjectLoanV29>,
        Option<&SourceReferenceLoanV29>,
        &SourceReferenceLoanV29,
        Option<&SourceReferenceOriginV29>,
        &SourceReferenceOriginV29,
        &SourceReferenceAccessRecordV29,
        production_call_instances_v1::ProductionInstanceBorrowV1<'_>,
        Result<
            production_call_instances_v1::ProductionInstanceBorrowV1<'_>,
            production_call_instances_v1::ProductionCallInstanceErrorV1,
        >,
        SemanticBorrowKindV1,
        SourceReferenceAccessV29,
        Option<usize>,
        usize,
        usize,
        usize,
        PendingSourceMemoryAlternativeV29,
        Option<PendingSourceMemoryAlternativeV29>,
        (),
        Call<'_, '_, '_>,
        Call<'_, '_, '_>,
    ];
    assert_eq!(
        source_safe_object_invocation_headers_v29().unwrap(),
        expected
    );
    for short in [false, true] {
        let limit = expected - usize::from(short);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        let result = budget.reserve_storage(source_safe_object_invocation_headers_v29().unwrap());
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == expected && error.limit() == limit));
            assert_eq!(budget.storage(), 0);
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), expected);
        }
        assert_eq!(budget.work(), 0);
    }
}

fn source_safe_object_invocation_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    source: &SourceAddressAccessSourceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<PendingSourceMemoryAlternativeV29>, ProductionSemanticKirErrorV1> {
    let result = (|| {
        plan.check_owner(instances, budget)?;
        budget.reserve_storage(source_safe_object_invocation_headers_v29()?)?;
        budget.charge_work(14)?;
        let Some(safe) = source.safe_object else {
            return Ok(None);
        };
        // This prerequisite only supplies the original invocation alternative.
        // Restart generations retain their independent activation obligation.
        if safe.referent.generation != 0 {
            return Ok(None);
        }
        if source.raw.is_some()
            || source.direct_object.is_some()
            || safe.key.site.instance != source.instance
            || !matches!(
                safe.key.access,
                SourceReferenceAccessV29::Read | SourceReferenceAccessV29::Write
            )
        {
            return Err(source_raw_physical_error_v29());
        }
        let (block, statement) = scoped_memory_site_key_v29(safe.frame.site);
        if safe.key.site.block.index() != block
            || safe.key.site.statement != statement.map(|value| value as usize)
        {
            return Err(source_raw_physical_error_v29());
        }
        let Some(ScopedMemoryRoleV29::Operand(role)) = safe.frame.role else {
            return Err(source_raw_physical_error_v29());
        };
        let function = instances
            .instance(source.instance)
            .ok_or_else(source_raw_physical_error_v29)?
            .declaration();
        let place = match safe.key.access {
            SourceReferenceAccessV29::Read => {
                scoped_payload_place_v29(function, safe.frame.site, role)
            }
            SourceReferenceAccessV29::Write => {
                scoped_source_place_v29(function, safe.frame.site, role)
            }
            _ => None,
        }
        .ok_or_else(source_raw_physical_error_v29)?;
        if safe.key.source != place as *const SemanticPlaceV1 as usize {
            return Err(source_raw_physical_error_v29());
        }
        let checked =
            source_object_loan_access_v29(plan, safe.key.site, place, safe.key.access, budget)?
                .ok_or_else(source_raw_physical_error_v29)?;
        if checked.loan != safe.loan
            || checked.original.instance != safe.referent.instance
            || checked.original.local != safe.referent.local
            || checked.original.generation != safe.referent.generation
            || checked.original.ty != safe.referent.ty
        {
            return Err(source_raw_physical_error_v29());
        }

        let mut next = Some(safe.loan);
        while let Some(index) = next {
            budget.charge_work(18)?;
            let loan = plan
                .loans
                .get(index)
                .ok_or_else(source_raw_physical_error_v29)?;
            let origin = plan
                .origins
                .get(loan.origin)
                .ok_or_else(source_raw_physical_error_v29)?;
            if origin.instance != safe.referent.instance || origin.local != safe.referent.local
                || origin.generation != 0 || origin.ty != safe.referent.ty
                || !origin.projections.is_empty()
                || (index == safe.loan && loan.site != safe.formation)
                // A parent existed before its child was first constructed.
                // This also bounds replay without trusting a copied chain.
                || loan.parent.is_some_and(|parent| parent >= index)
            {
                return Err(source_raw_physical_error_v29());
            }
            let exact = instances
                .borrow_at(
                    loan.site.instance,
                    loan.site.block,
                    loan.site
                        .statement
                        .ok_or_else(source_raw_physical_error_v29)?,
                    budget,
                )
                .map_err(|error| match error {
                    production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(
                        error,
                    ) => error.into(),
                    _ => source_raw_physical_error_v29(),
                })?;
            if exact.instance != loan.site.instance
                || exact.block != loan.site.block
                || Some(exact.statement) != loan.site.statement
                || exact.kind != loan.kind
                || exact.destination.ty() != loan.source_type
                || exact.source.ty() != origin.ty
            {
                return Err(source_raw_physical_error_v29());
            }
            // Borrow and Read/Write are different original keys. In particular,
            // a reborrow must identify its exact parent, not alias a Read key.
            let access = source_reference_access_at_v29(
                plan,
                loan.site,
                exact.source,
                SourceReferenceAccessV29::Borrow(loan.kind),
                budget,
            )?;
            if access.loan != loan.parent
                || access.instance != origin.instance
                || access.local != origin.local
                || access.generation != origin.generation
                || access.ty != origin.ty
                || !access.projections.is_empty()
            {
                return Err(source_raw_physical_error_v29());
            }
            next = loan.parent;
        }
        let actual = source_address_object_slot_v29(
            instances,
            plan,
            slots,
            safe.referent.instance,
            safe.referent.local,
            safe.referent.generation,
            safe.referent.ty,
            budget,
        )?;
        if actual != source.physical.slot {
            return Err(source_raw_physical_error_v29());
        }
        Ok(Some(PendingSourceMemoryAlternativeV29 {
            instance: safe.referent.instance,
            local: safe.referent.local,
            slot: actual,
            activation: SourceMemoryActivationV29::Invocation,
            formation: Some(safe.formation),
        }))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

#[cfg(test)]
thread_local! {
    static SAFE_OBJECT_ACTIVATION_TEST_V29: std::cell::Cell<Option<(u8, [usize; 4])>> = const {
        std::cell::Cell::new(None)
    };
}

#[cfg(test)]
pub(super) fn with_safe_object_activation_test_v29<T>(
    mode: u8,
    run: impl FnOnce() -> T,
) -> (T, [usize; 4]) {
    struct Reset(Option<(u8, [usize; 4])>);
    impl Drop for Reset {
        fn drop(&mut self) {
            SAFE_OBJECT_ACTIVATION_TEST_V29.set(self.0);
        }
    }
    let _reset = Reset(SAFE_OBJECT_ACTIVATION_TEST_V29.replace(Some((mode, [0; 4]))));
    let result = run();
    (result, SAFE_OBJECT_ACTIVATION_TEST_V29.get().unwrap().1)
}

#[cfg(test)]
fn test_safe_object_invocation_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    source: &SourceAddressAccessSourceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let Some((mode, mut counts)) = SAFE_OBJECT_ACTIVATION_TEST_V29.get() else {
        return Ok(());
    };
    let safe = source.safe_object.unwrap();
    let original =
        source_safe_object_invocation_v29(instances, plan, slots, source, budget)?.unwrap();
    assert_eq!(original.instance, safe.referent.instance);
    assert_eq!(original.local, safe.referent.local);
    assert_eq!(original.slot, source.physical.slot);
    assert_eq!(original.activation, SourceMemoryActivationV29::Invocation);
    assert_eq!(original.formation, Some(safe.formation));
    counts[0] += 1;
    counts[1] += usize::from(source.instance != safe.referent.instance);
    counts[2] += usize::from(plan.loans[safe.loan].parent.is_some());
    if mode == 2 || mode == 3 {
        SAFE_OBJECT_ACTIVATION_TEST_V29.set(Some((mode, counts)));
        let held = budget.storage();
        let error = if mode == 2 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let error =
                source_safe_object_invocation_v29(instances, plan, slots, source, &mut foreign)
                    .unwrap_err();
            assert!(matches!(
                error,
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            ));
            assert_eq!((foreign.work(), foreign.storage()), (0, 0));
            error
        } else {
            let headers = source_safe_object_invocation_headers_v29()?;
            let limit = budget.storage_limit();
            let padding = limit
                .checked_sub(held)
                .and_then(|left| left.checked_sub(headers - 1))
                .unwrap();
            budget.reserve_storage(padding)?;
            let error = source_safe_object_invocation_v29(instances, plan, slots, source, budget)
                .unwrap_err();
            assert!(
                matches!(&error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(error)) if error.limit() == limit && error.actual() == limit + 1)
            );
            assert_eq!(budget.storage(), held + padding);
            budget.release_storage(padding)?;
            error
        };
        let stopped = (budget.work(), budget.storage());
        let replay =
            source_safe_object_invocation_v29(instances, plan, slots, source, budget).unwrap_err();
        match (&error, &replay) {
            (
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first),
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(second),
            ) => assert_eq!(first, second),
            _ => panic!("exact first resource failure must replay"),
        }
        assert_eq!((budget.work(), budget.storage()), stopped);
        assert_eq!(budget.storage(), held);
        return Err(error);
    }
    if mode == 1 {
        // Copied metadata is queried inside the authentic source owner. These
        // semantic refusals are not resource-latch recovery tests.
        for fault in 0..9 {
            let mut copied = *source;
            let copied_safe = copied.safe_object.as_mut().unwrap();
            match fault {
                0 => copied_safe.key.source = usize::MAX,
                1 => {
                    copied_safe.key.access =
                        SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Shared)
                }
                2 => copied_safe.loan = plan.loans.len(),
                3 => copied_safe.referent.local = SemanticLocalIdV1::from_index(u32::MAX),
                4 => copied_safe.referent.ty = SemanticTypeIdV1::from_index(u32::MAX),
                5 => copied_safe.formation.statement = None,
                6 => copied.physical.slot = slots.slots.len(),
                7 => copied_safe.frame.role = None,
                8 => copied_safe.key.site.statement = None,
                _ => unreachable!(),
            }
            assert!(
                source_safe_object_invocation_v29(instances, plan, slots, &copied, budget).is_err(),
                "copied safe-origin fault {fault}"
            );
            counts[3] += 1;
        }
        if source.instance != safe.referent.instance {
            let mut copied = *source;
            copied.safe_object.as_mut().unwrap().referent.instance = source.instance;
            assert!(
                source_safe_object_invocation_v29(instances, plan, slots, &copied, budget).is_err()
            );
            counts[3] += 1;
        }
        if let Some(parent) = plan.loans[safe.loan].parent {
            for replace_loan in [false, true] {
                let mut copied = *source;
                if replace_loan {
                    copied.safe_object.as_mut().unwrap().loan = parent;
                } else {
                    copied.safe_object.as_mut().unwrap().formation = plan.loans[parent].site;
                }
                assert!(
                    source_safe_object_invocation_v29(instances, plan, slots, &copied, budget)
                        .is_err(),
                    "authentic parent cannot replace terminal loan or Borrow formation"
                );
                counts[3] += 1;
            }
        }
        let mut later = *source;
        later.safe_object.as_mut().unwrap().referent.generation = 1;
        assert!(
            source_safe_object_invocation_v29(instances, plan, slots, &later, budget)?.is_none()
        );
        let after =
            source_safe_object_invocation_v29(instances, plan, slots, source, budget)?.unwrap();
        assert_eq!(
            (after.instance, after.local, after.slot, after.formation),
            (
                original.instance,
                original.local,
                original.slot,
                original.formation
            )
        );
    }
    SAFE_OBJECT_ACTIVATION_TEST_V29.set(Some((mode, counts)));
    Ok(())
}
