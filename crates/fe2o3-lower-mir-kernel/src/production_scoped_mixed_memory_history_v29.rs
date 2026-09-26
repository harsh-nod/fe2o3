// Slot-disjoint factorization of the existing physical history equations. The
// complete graph/access roster still owns all provenance and memory versions.
struct MixedHistoryDomainV29 {
    accesses: Vec<SourceAddressAccessV29>,
    kills: Vec<SourceAddressKillV29>,
    failures: Vec<SourceIndexFailureV29>,
}

fn mixed_history_object_slot_v29(
    slots: &[ScopedSourceSlotV29],
    slot: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<bool> {
    budget.charge_work(1)?;
    Ok(matches!(slots.get(slot).ok_or_else(scoped_slot_error_v29)?.representation,
        ScopedSlotRepresentationV29::Object { .. }))
}

fn mixed_history_domain_v29(
    counts: [usize; 3],
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<MixedHistoryDomainV29> {
    Ok(MixedHistoryDomainV29 {
        accesses: emission_vec_v1(counts[0], budget)?,
        kills: emission_vec_v1(counts[1], budget)?,
        failures: emission_vec_v1(counts[2], budget)?,
    })
}

fn mixed_history_headers_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<[MixedHistoryDomainV29; 2]>(),
        std::mem::size_of::<[UseResult<MixedHistoryDomainV29>; 2]>(),
        std::mem::size_of::<[UseResult<Vec<SourceAddressAccessV29>>; 2]>(),
        std::mem::size_of::<[UseResult<Vec<SourceAddressKillV29>>; 2]>(),
        std::mem::size_of::<[UseResult<Vec<SourceIndexFailureV29>>; 2]>(),
        std::mem::size_of::<[[usize; 3]; 2]>(),
        std::mem::size_of::<UseResult<bool>>(),
        std::mem::size_of::<UseResult<()>>(),
    ])
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check_expanded_indices_v29<'view, 'inventory, 'graph>(
    function: &'graph Function,
    graph: &'view SourceAddressMemoryV29<'graph>,
    slots: &'view [ScopedSourceSlotV29],
    accesses: &'view [SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    lifetimes: &[SourceAddressLifetimeV29],
    failures: &[SourceIndexFailureV29],
    selected: &[SourceIndexLocationV29],
    guards: &[SourceIndexGuardLocationV29],
    inventory: &'inventory fe2o3_kernel_analysis::CanonicalKirInventoryV18<'graph>,
    versions: Option<&'view fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'inventory, 'graph>>,
    function_coordinate: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    budget.charge_work(slots.len())?;
    if !slots.iter().any(|slot| matches!(slot.representation, ScopedSlotRepresentationV29::Object { .. })) {
        return check_expanded_array_history_v29(function, graph, slots, accesses, accesses,
            kills, lifetimes, failures, selected, guards, inventory, versions,
            function_coordinate, budget);
    }
    with_canonical_call_scratch_v1(budget, |budget| {
        budget.reserve_storage(mixed_history_headers_v29()?)?;
        let mut counts = [[0usize; 3]; 2];
        for (kind, slot) in accesses.iter().map(|row| (0, row.slot))
            .chain(kills.iter().map(|row| (1, row.slot)))
            .chain(failures.iter().map(|row| (2, row.slot)))
        {
            let domain = usize::from(mixed_history_object_slot_v29(slots, slot, budget)?);
            counts[domain][kind] = argument_sum_v1(&[counts[domain][kind], 1])?;
        }
        let mut arrays = mixed_history_domain_v29(counts[0], budget)?;
        let mut objects = mixed_history_domain_v29(counts[1], budget)?;
        for &row in accesses {
            if mixed_history_object_slot_v29(slots, row.slot, budget)? {
                objects.accesses.push(row);
            } else {
                arrays.accesses.push(row);
            }
        }
        for &row in kills {
            if mixed_history_object_slot_v29(slots, row.slot, budget)? {
                objects.kills.push(row);
            } else {
                arrays.kills.push(row);
            }
        }
        for &row in failures {
            if mixed_history_object_slot_v29(slots, row.slot, budget)? {
                objects.failures.push(row);
            } else {
                arrays.failures.push(row);
            }
        }
        // Each immutable input row occurs in exactly one ordered partition;
        // original slot ordinals and each block's actual operation order remain.
        check_expanded_static_object_history_v29(function, graph, slots, &objects.accesses,
            &objects.kills, &objects.failures, budget)?;
        check_expanded_array_history_v29(function, graph, slots, &arrays.accesses, accesses,
            &arrays.kills, lifetimes, &arrays.failures, selected, guards, inventory,
            versions, function_coordinate, budget)
    })
}
