// Allocation bounds only. The original population pass still authenticates
// every activation, generation, source slot, and currentness obligation.
fn pending_alternative_capacity_headers_v29() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            std::mem::size_of::<T>(),
            std::mem::size_of::<Result<T, ArgumentResourceV1>>(),
            std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
        ])
    }
    type Count<'a, 'b> = (
        &'a SourceReferencePlanV29<'a, 'b>,
        &'a [SourceAddressAccessSourceV29],
        &'a OwnedScopedSourceSlotsV29,
        &'a [u32],
        &'a [(usize, SourceMemoryActivationV29)],
        bool,
        &'a mut ArgumentBudgetV1<'b>,
    );
    type Push<'a, 'b> = (
        &'a mut Vec<PendingSourceMemoryAlternativeV29>,
        PendingSourceMemoryAlternativeV29,
        usize,
        &'a mut ArgumentBudgetV1<'b>,
    );
    argument_sum_v1(&[
        h::<Count<'_, '_>>()?,
        h::<Push<'_, '_>>()?,
        h::<std::slice::Iter<'_, SourceAddressAccessSourceV29>>()?,
        h::<&SourceAddressAccessSourceV29>()?,
        h::<SourceReferenceRawAccessV29>()?,
        h::<Option<SourceReferenceRawAccessV29>>()?,
        h::<&SourceReferenceRawSetV29>()?,
        h::<Option<&SourceReferenceRawSetV29>>()?,
        h::<std::ops::Range<usize>>()?,
        h::<&[SourceReferenceRawChoiceV29]>()?,
        h::<Option<&[SourceReferenceRawChoiceV29]>>()?,
        h::<std::slice::Iter<'_, SourceReferenceRawChoiceV29>>()?,
        h::<&SourceReferenceRawChoiceV29>()?,
        h::<&SourceReferenceRawOriginV29>()?,
        h::<Option<&SourceReferenceRawOriginV29>>()?,
        h::<&u32>()?,
        h::<Option<&u32>>()?,
        h::<&SourceReferenceEpochSetV29>()?,
        h::<Option<&SourceReferenceEpochSetV29>>()?,
        h::<&[SourceReferenceRawSetV29]>()?,
        h::<&[SourceReferenceRawOriginV29]>()?,
        h::<&[SourceReferenceEpochSetV29]>()?,
        h::<&[ScopedSourceSlotV29]>()?,
        h::<&[u32]>()?,
        h::<&[usize]>()?,
        h::<[usize; 2]>()?,
        h::<Option<&ScopedSourceSlotV29>>()?,
        h::<&ScopedSourceSlotV29>()?,
        h::<ScopedSlotRepresentationV29>()?,
        h::<(
            &(usize, SourceMemoryActivationV29),
            &SourceAddressAccessSourceV29,
        )>()?,
        h::<usize>()?,
        h::<usize>()?,
        h::<usize>()?,
        h::<usize>()?,
        h::<u32>()?,
        h::<u32>()?,
        h::<bool>()?,
        h::<()>()?,
    ])
}

fn pending_alternative_capacity_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    sources: &[SourceAddressAccessSourceV29],
    slots: &OwnedScopedSourceSlotsV29,
    limits: &[u32],
    ordinary: &[(usize, SourceMemoryActivationV29)],
    ordinary_indices: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    let mut count = 0;
    for source in sources {
        budget.charge_work(4)?;
        if let Some(access) = source.raw {
            let set = plan
                .raw_sets
                .get(access.set)
                .ok_or_else(source_raw_physical_error_v29)?;
            let choices = plan
                .raw_choices
                .get(set.first..argument_sum_v1(&[set.first, set.count])?)
                .ok_or_else(source_raw_physical_error_v29)?;
            for choice in choices {
                budget.charge_work(3)?;
                let origin = plan
                    .raw_origins
                    .get(choice.origin)
                    .ok_or_else(source_raw_physical_error_v29)?;
                let limit = *limits
                    .get(origin.instance.index())
                    .ok_or_else(source_raw_physical_error_v29)?;
                let atoms = if origin.generation < limit {
                    1
                } else {
                    plan.epoch_sets
                        .get((origin.generation - limit) as usize)
                        .ok_or_else(source_raw_physical_error_v29)?
                        .count
                };
                count = argument_sum_v1(&[count, atoms])?;
            }
        } else {
            let mut alternatives =
                usize::from(source.direct_object.is_some() || source.safe_object.is_some());
            if ordinary_indices
                && matches!(
                    slots
                        .slots
                        .get(source.physical.slot)
                        .ok_or_else(source_raw_physical_error_v29)?
                        .representation,
                    ScopedSlotRepresentationV29::ScalarArray(_)
                )
            {
                budget.charge_work(argument_product_v1(
                    2,
                    call_splice_search_work_v1(ordinary.len()),
                )?)?;
                let first = ordinary.partition_point(|row| row.0 < source.physical.slot);
                let end = ordinary.partition_point(|row| row.0 <= source.physical.slot);
                // The checked object and ordinary-array branches are exclusive.
                alternatives = alternatives.max(end - first);
            }
            count = argument_sum_v1(&[count, alternatives])?;
        }
    }
    Ok(count)
}

fn push_pending_alternative_v29(
    rows: &mut Vec<PendingSourceMemoryAlternativeV29>,
    row: PendingSourceMemoryAlternativeV29,
    limit: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    if rows.len() >= limit || rows.len() >= rows.capacity() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    rows.push(row);
    Ok(())
}

#[cfg(test)]
std::thread_local! {
    pub(super) static PENDING_ALTERNATIVE_CAPACITY_V29: std::cell::Cell<(usize, usize, usize)> = const {
        std::cell::Cell::new((0, 0, 0))
    };
}

#[cfg(test)]
mod pending_alternative_capacity_tests {
    use super::*;

    fn expected_headers() -> usize {
        fn h<T>() -> usize {
            std::mem::size_of::<T>()
                + std::mem::size_of::<Result<T, ArgumentResourceV1>>()
                + std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
        }
        h::<(
            &SourceReferencePlanV29<'_, '_>,
            &[SourceAddressAccessSourceV29],
            &OwnedScopedSourceSlotsV29,
            &[u32],
            &[(usize, SourceMemoryActivationV29)],
            bool,
            &mut ArgumentBudgetV1<'_>,
        )>() + h::<(
            &mut Vec<PendingSourceMemoryAlternativeV29>,
            PendingSourceMemoryAlternativeV29,
            usize,
            &mut ArgumentBudgetV1<'_>,
        )>() + h::<std::slice::Iter<'_, SourceAddressAccessSourceV29>>()
            + h::<&SourceAddressAccessSourceV29>()
            + h::<SourceReferenceRawAccessV29>()
            + h::<Option<SourceReferenceRawAccessV29>>()
            + h::<&SourceReferenceRawSetV29>()
            + h::<Option<&SourceReferenceRawSetV29>>()
            + h::<std::ops::Range<usize>>()
            + h::<&[SourceReferenceRawChoiceV29]>()
            + h::<Option<&[SourceReferenceRawChoiceV29]>>()
            + h::<std::slice::Iter<'_, SourceReferenceRawChoiceV29>>()
            + h::<&SourceReferenceRawChoiceV29>()
            + h::<&SourceReferenceRawOriginV29>()
            + h::<Option<&SourceReferenceRawOriginV29>>()
            + h::<&u32>()
            + h::<Option<&u32>>()
            + h::<&SourceReferenceEpochSetV29>()
            + h::<Option<&SourceReferenceEpochSetV29>>()
            + h::<&[SourceReferenceRawSetV29]>()
            + h::<&[SourceReferenceRawOriginV29]>()
            + h::<&[SourceReferenceEpochSetV29]>()
            + h::<&[ScopedSourceSlotV29]>()
            + h::<&[u32]>()
            + h::<&[usize]>()
            + h::<[usize; 2]>()
            + h::<Option<&ScopedSourceSlotV29>>()
            + h::<&ScopedSourceSlotV29>()
            + h::<ScopedSlotRepresentationV29>()
            + h::<(
                &(usize, SourceMemoryActivationV29),
                &SourceAddressAccessSourceV29,
            )>()
            + 4 * h::<usize>()
            + h::<u32>()
            + h::<u32>()
            + h::<bool>()
            + h::<()>()
    }

    fn inert_row() -> PendingSourceMemoryAlternativeV29 {
        PendingSourceMemoryAlternativeV29 {
            instance: ProductionCallInstanceIdV1(0),
            local: SemanticLocalIdV1::from_index(0),
            slot: 0,
            activation: SourceMemoryActivationV29::Invocation,
            formation: None,
        }
    }

    #[test]
    fn pending_alternative_capacity_has_independent_fixed_and_exact_short_allocation_cuts() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
        let headers = expected_headers();
        assert_eq!(pending_alternative_capacity_headers_v29().unwrap(), headers);
        for count in [0, 1, 17, 769] {
            let bytes = count * std::mem::size_of::<PendingSourceMemoryAlternativeV29>();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
            let mut budget = ArgumentBudgetV1::new(&mut work, headers + bytes);
            budget.reserve_storage(headers).unwrap();
            let rows =
                emission_vec_v1::<PendingSourceMemoryAlternativeV29>(count, &mut budget).unwrap();
            assert_eq!(
                budget.storage(),
                headers
                    + rows.capacity() * std::mem::size_of::<PendingSourceMemoryAlternativeV29>()
            );
            drop(rows);
            if bytes != 0 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
                let mut short = ArgumentBudgetV1::new(&mut work, headers + bytes - 1);
                short.reserve_storage(headers).unwrap();
                let error = emission_vec_v1::<PendingSourceMemoryAlternativeV29>(count, &mut short)
                    .unwrap_err();
                assert!(
                    matches!(error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error)) if error.actual() == headers + bytes && error.limit() == headers + bytes - 1)
                );
                assert_eq!(short.storage(), headers);
            }
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut short = ArgumentBudgetV1::new(&mut work, headers - 1);
        assert!(
            matches!(short.reserve_storage(headers), Err(ArgumentResourceV1::Storage(error))
            if error.actual() == headers && error.limit() == headers - 1)
        );
        assert_eq!(short.storage(), 0);
    }

    #[test]
    fn pending_alternative_population_cannot_grow_or_exceed_its_preflight_bound() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
        budget.reserve_storage(expected_headers()).unwrap();
        let mut rows = emission_vec_v1(4, &mut budget).unwrap();
        let capacity = rows.capacity();
        let floor = budget.storage();
        for _ in 0..3 {
            push_pending_alternative_v29(&mut rows, inert_row(), 3, &mut budget).unwrap();
        }
        assert_eq!(rows, vec![inert_row(); 3]);
        assert!(matches!(
            push_pending_alternative_v29(&mut rows, inert_row(), 3, &mut budget),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(
            (rows.len(), rows.capacity(), budget.storage()),
            (3, capacity, floor)
        );
        let mut empty = Vec::new();
        assert!(push_pending_alternative_v29(&mut empty, inert_row(), 1, &mut budget).is_err());
        assert!(empty.is_empty());
        assert_eq!(budget.storage(), floor);
    }
}
