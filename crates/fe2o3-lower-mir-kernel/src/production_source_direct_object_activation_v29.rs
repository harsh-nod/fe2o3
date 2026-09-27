// A logical gen0 endpoint may begin at its authenticated invocation. This is
// only an alternative; complete lifetime/init/currentness equations still run.
fn source_direct_object_invocation_headers_v29() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            std::mem::size_of::<T>(),
            argument_product_v1(2, std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<SourceDirectObjectOriginV29>()?,
        h::<Option<SourceDirectObjectOriginV29>>()?,
        h::<&ScopedSourceSlotV29>()?,
        h::<Option<&ScopedSourceSlotV29>>()?,
        h::<usize>()?, h::<bool>()?,
        h::<PendingSourceMemoryAlternativeV29>()?,
        h::<Option<PendingSourceMemoryAlternativeV29>>()?,
        h::<(&ExecutionInstancesV29<'_>, &SourceReferencePlanV29<'_, '_>,
            &OwnedScopedSourceSlotsV29, &SourceAddressAccessSourceV29, &mut ArgumentBudgetV1<'_>)>()?,
    ])
}

fn source_direct_object_invocation_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    source: &SourceAddressAccessSourceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<PendingSourceMemoryAlternativeV29>, ProductionSemanticKirErrorV1> {
    let result = (|| {
        plan.check_owner(instances, budget)?;
        budget.reserve_storage(source_direct_object_invocation_headers_v29()?)?;
        budget.charge_work(5)?;
        let Some(origin) = source.direct_object else { return Ok(None); };
        // Nonzero logical generations and reference/aggregate-holder origins
        // need their separate source activation proof. They remain pending.
        if origin.generation != 0 { return Ok(None); }
        if source.raw.is_some() || source.instance != origin.instance {
            return Err(source_raw_physical_error_v29());
        }
        let slot = slots.slots.get(source.physical.slot)
            .ok_or_else(source_raw_physical_error_v29)?;
        if !matches!(slot.representation, ScopedSlotRepresentationV29::Object { .. }) {
            return Err(source_raw_physical_error_v29());
        }
        // This exact source query checks owner/ledger, declaration type,
        // selected schema and logical-to-physical generation correspondence.
        let actual = source_address_object_slot_v29(instances, plan, slots,
            origin.instance, origin.local, origin.generation, origin.ty, budget)?;
        if actual != source.physical.slot {
            return Err(source_raw_physical_error_v29());
        }
        Ok(Some(PendingSourceMemoryAlternativeV29 {
            instance: origin.instance, local: origin.local, slot: actual,
            activation: SourceMemoryActivationV29::Invocation, formation: None,
        }))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

#[cfg(test)]
thread_local! {
    static DIRECT_OBJECT_ACTIVATION_TEST_V29: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(super) fn with_direct_object_activation_test_v29<T>(run: impl FnOnce() -> T) -> (T, usize) {
    struct Reset(Option<usize>);
    impl Drop for Reset {
        fn drop(&mut self) { DIRECT_OBJECT_ACTIVATION_TEST_V29.set(self.0); }
    }
    let _reset = Reset(DIRECT_OBJECT_ACTIVATION_TEST_V29.replace(Some(0)));
    let result = run();
    (result, DIRECT_OBJECT_ACTIVATION_TEST_V29.get().unwrap())
}

#[cfg(test)]
fn test_direct_object_invocation_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    source: &SourceAddressAccessSourceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let Some(count) = DIRECT_OBJECT_ACTIVATION_TEST_V29.get() else { return Ok(()); };
    let origin = source.direct_object.unwrap();
    assert_eq!(origin.generation, 0);
    let checked = source_direct_object_invocation_v29(instances, plan, slots, source, budget)?.unwrap();
    assert_eq!(checked.instance, source.instance);
    assert_eq!(checked.local, origin.local);
    assert_eq!(checked.slot, source.physical.slot);
    assert_eq!(checked.activation, SourceMemoryActivationV29::Invocation);
    assert!(checked.formation.is_none());
    // These copied rows carry no authority; the real producer row remains
    // unchanged and complete immutable source admission follows each query.
    for fault in 0..3 {
        let mut copied = *source;
        match fault {
            0 => copied.direct_object.as_mut().unwrap().local = SemanticLocalIdV1::from_index(u32::MAX),
            1 => copied.direct_object.as_mut().unwrap().ty = SemanticTypeIdV1::from_index(u32::MAX),
            2 => copied.physical.slot = slots.slots.len(),
            _ => unreachable!(),
        }
        assert!(source_direct_object_invocation_v29(instances, plan, slots, &copied, budget).is_err());
    }
    if let Some(other) = (0..instances.instances().len()).filter_map(|at| instances.id_at(at))
        .find(|id| *id != source.instance) {
        let mut copied = *source;
        copied.direct_object.as_mut().unwrap().instance = other;
        assert!(source_direct_object_invocation_v29(instances, plan, slots, &copied, budget).is_err());
    }
    let mut missing = *source;
    missing.direct_object = None;
    assert!(source_direct_object_invocation_v29(instances, plan, slots, &missing, budget)?.is_none());
    let mut later = *source;
    later.direct_object.as_mut().unwrap().generation = 1;
    assert!(source_direct_object_invocation_v29(instances, plan, slots, &later, budget)?.is_none());
    let after = source_direct_object_invocation_v29(instances, plan, slots, source, budget)?.unwrap();
    assert_eq!((after.instance, after.local, after.slot), (checked.instance, checked.local, checked.slot));
    DIRECT_OBJECT_ACTIVATION_TEST_V29.set(Some(count + 1));
    Ok(())
}
