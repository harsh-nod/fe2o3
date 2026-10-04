fn conflicting_witness<'a>(
    state: &'a AddressStateV1,
    access: AccessKindAttr,
    witness: &RankedRaceWitnessV1,
) -> Option<&'a RankedRaceWitnessV1> {
    match access {
        AccessKindAttr::Read => state
            .writes
            .different_from(&witness.invocation)
            .or_else(|| state.atomic_writes.different_from(&witness.invocation)),
        AccessKindAttr::Write => state
            .writes
            .different_from(&witness.invocation)
            .or_else(|| state.reads.different_from(&witness.invocation))
            .or_else(|| state.atomic_reads.different_from(&witness.invocation))
            .or_else(|| state.atomic_writes.different_from(&witness.invocation)),
        AccessKindAttr::AtomicRead => state
            .writes
            .different_from(&witness.invocation)
            .or_else(|| incompatible_atomic(&state.atomic_writes, witness)),
        AccessKindAttr::AtomicWrite | AccessKindAttr::AtomicReadModifyWrite => state
            .writes
            .different_from(&witness.invocation)
            .or_else(|| state.reads.different_from(&witness.invocation))
            .or_else(|| incompatible_atomic(&state.atomic_reads, witness))
            .or_else(|| incompatible_atomic(&state.atomic_writes, witness)),
    }
}

fn incompatible_atomic<'a>(
    state: &'a WitnessPairV1,
    witness: &RankedRaceWitnessV1,
) -> Option<&'a RankedRaceWitnessV1> {
    [state.first.as_ref(), state.second.as_ref()]
        .into_iter()
        .flatten()
        .find(|other| {
            other.invocation != witness.invocation
                && (!atomic_scope_covers_pair(other.atomic_scope, other, witness)
                    || !atomic_scope_covers_pair(witness.atomic_scope, witness, other))
        })
}

fn atomic_scope_covers_pair(
    scope: Option<AtomicScopeAttr>,
    first: &RankedRaceWitnessV1,
    second: &RankedRaceWitnessV1,
) -> bool {
    if first.invocation == second.invocation {
        return true;
    }
    match (first.workgroup, second.workgroup) {
        (Some(first), Some(second)) if first == second => matches!(
            scope,
            Some(
                AtomicScopeAttr::Workgroup
                    | AtomicScopeAttr::Agent
                    | AtomicScopeAttr::Device
                    | AtomicScopeAttr::System
            )
        ),
        _ => matches!(
            scope,
            Some(AtomicScopeAttr::Agent | AtomicScopeAttr::Device | AtomicScopeAttr::System)
        ),
    }
}

fn insert_witness(state: &mut AddressStateV1, witness: RankedRaceWitnessV1) {
    match witness.access {
        AccessKindAttr::Read => state.reads.insert(witness),
        AccessKindAttr::Write => state.writes.insert(witness),
        AccessKindAttr::AtomicRead => state.atomic_reads.insert(witness),
        AccessKindAttr::AtomicWrite | AccessKindAttr::AtomicReadModifyWrite => {
            state.atomic_writes.insert(witness);
        }
    }
}

fn symbolically_proves_disjoint(
    context: &Context,
    function: &FuncOp,
    effects: &[EffectV1],
    sparse: &SparseIndexAnalysisV1,
    launch_extents: &[u64],
    invocation_bounds: Option<&[[Option<u64>; MAX_RANKED_MEMORY_RANK]]>,
    observer: RaceObserverV1<'_, '_, '_>,
) -> bool {
    let run = || {
        let mut by_view: HashMap<u64, Vec<&EffectV1>> = HashMap::new();
        for effect in effects {
            by_view
                .entry(effect.noalias_class)
                .or_default()
                .push(effect);
        }
        for effects in by_view.values() {
            if !effect_pair_inventory_fits_budget(effects.len()) {
                observe_race_quota_v1(observer, "race effect-pair inventory work limit");
                return false;
            }
            for first_index in 0..effects.len() {
                for second_index in first_index..effects.len() {
                    let first = effects[first_index];
                    let second = effects[second_index];
                    if !access_kinds_need_disjoint_coordinates(first.kind, second.kind)
                        || atomics_are_device_compatible(first, second)
                    {
                        continue;
                    }
                    if !effect_pair_symbolically_disjoint(
                        context,
                        function,
                        first,
                        second,
                        sparse,
                        launch_extents,
                        invocation_bounds,
                    ) {
                        return false;
                    }
                }
            }
        }
        true
    };
    match observer {
        None => run(),
        Some(observer) => observer.with_projection(&Ok, |_| run()),
    }
}

#[allow(clippy::too_many_arguments)]
fn effect_pair_symbolically_disjoint(
    context: &Context,
    function: &FuncOp,
    first: &EffectV1,
    second: &EffectV1,
    sparse: &SparseIndexAnalysisV1,
    launch_extents: &[u64],
    invocation_bounds: Option<&[[Option<u64>; MAX_RANKED_MEMORY_RANK]]>,
) -> bool {
    if effects_share_proven_singleton_invocation_domain(
        first,
        second,
        launch_extents,
        invocation_bounds,
    ) {
        return true;
    }
    if effect_affine_map_is_injective(first, sparse, launch_extents, invocation_bounds)
        && effect_affine_map_is_injective(second, sparse, launch_extents, invocation_bounds)
        && same_index_formula(&first.indices, &second.indices, sparse)
    {
        return true;
    }
    checked_tiled_pair_is_disjoint(context, function, first, second, sparse, launch_extents)
        || checked_row_striped_pair_is_disjoint(
            context,
            function,
            first,
            second,
            sparse,
            launch_extents,
        )
        || one_dimensional_affine_residues_are_disjoint(
            first,
            second,
            sparse,
            launch_extents,
            invocation_bounds,
        )
}

fn effects_share_proven_singleton_invocation_domain(
    first: &EffectV1,
    second: &EffectV1,
    launch_extents: &[u64],
    invocation_bounds: Option<&[[Option<u64>; MAX_RANKED_MEMORY_RANK]]>,
) -> bool {
    const GPU_INVOCATION_DIMENSIONS: usize = 3;

    let first = effective_launch_extents(first, launch_extents, invocation_bounds);
    let second = effective_launch_extents(second, launch_extents, invocation_bounds);
    first.len() == GPU_INVOCATION_DIMENSIONS
        && second.len() == GPU_INVOCATION_DIMENSIONS
        && first.iter().all(|extent| *extent == 1)
        && second.iter().all(|extent| *extent == 1)
}

fn checked_tiled_pair_is_disjoint(
    context: &Context,
    function: &FuncOp,
    first: &EffectV1,
    second: &EffectV1,
    sparse: &SparseIndexAnalysisV1,
    launch_extents: &[u64],
) -> bool {
    let ([first_index], [second_index]) = (first.indices.as_slice(), second.indices.as_slice())
    else {
        return false;
    };
    if first.checked_success.is_none() || second.checked_success.is_none() {
        return false;
    }
    let first_fact = sparse.fact(*first_index);
    let second_fact = sparse.fact(*second_index);
    let (Some(first), Some(second)) = (
        first_fact.checked_tiled_2d(),
        second_fact.checked_tiled_2d(),
    ) else {
        return false;
    };
    let first_component = sparse.fact(first.component()).constant_value();
    let second_component = sparse.fact(second.component()).constant_value();
    first.geometry() == second.geometry()
        && checked_runtime_layouts_are_equivalent_and_uniform(
            context,
            function,
            &first.runtime_layout(),
            &second.runtime_layout(),
            sparse,
        )
        && first.invocation() == second.invocation()
        && first_component.is_some_and(|component| component < first.geometry()[3])
        && second_component.is_some_and(|component| component < second.geometry()[3])
        && checked_invocation_is_injective(first.invocation(), launch_extents)
}

fn checked_row_striped_pair_is_disjoint(
    context: &Context,
    function: &FuncOp,
    first: &EffectV1,
    second: &EffectV1,
    sparse: &SparseIndexAnalysisV1,
    launch_extents: &[u64],
) -> bool {
    let ([first_index], [second_index]) = (first.indices.as_slice(), second.indices.as_slice())
    else {
        return false;
    };
    if !effect_has_authenticated_row_striped_contract(context, first, *first_index)
        || !effect_has_authenticated_row_striped_contract(context, second, *second_index)
    {
        return false;
    }
    let first_fact = sparse.fact(*first_index);
    let second_fact = sparse.fact(*second_index);
    let (Some(first), Some(second)) = (
        first_fact.checked_row_striped_2d(),
        second_fact.checked_row_striped_2d(),
    ) else {
        return false;
    };
    // A successful checked row-striped mapping already proves that its dynamic
    // component is below elements-per-lane, its column is below columns, and
    // columns is at most the row stride.  Different lanes therefore have
    // different residues modulo lanes-per-row, while different rows occupy
    // disjoint stride intervals.  Component constness is not required for
    // cross-invocation disjointness.
    first.geometry() == second.geometry()
        && checked_runtime_layouts_are_equivalent_and_uniform(
            context,
            function,
            &first.runtime_layout(),
            &second.runtime_layout(),
            sparse,
        )
        && first.invocation() == second.invocation()
        && checked_invocation_is_injective(first.invocation(), launch_extents)
}

fn effect_has_authenticated_row_striped_contract(
    context: &Context,
    effect: &EffectV1,
    index: Value,
) -> bool {
    let (Some(success), EffectIdentityV1::View(view)) = (effect.checked_success, effect.identity)
    else {
        return false;
    };
    let Some(producer) = index.defining_op() else {
        return false;
    };
    if success.defining_op() != Some(producer) {
        return false;
    }
    let producer = Operation::get_op_dyn(producer, context);
    let Some(checked) = producer.downcast_ref::<CheckedRowStripedIndex2DOp>() else {
        return false;
    };
    let Some(view_definition) = view.defining_op() else {
        return false;
    };
    let view_definition = Operation::get_op_dyn(view_definition, context);
    let Some(view) = view_definition.downcast_ref::<RankedViewOp>() else {
        return false;
    };
    if view
        .view_type(context)
        .is_none_or(|view_type| view_type.deref(context).shape() != [DYNAMIC_EXTENT])
    {
        return false;
    }
    checked.result(context) == index
        && checked.success(context) == Some(success)
        && checked
            .physical_extent(context)
            .is_some_and(|extent| view.dynamic_extent(context, 0) == Some(extent))
}

fn checked_runtime_layouts_are_equivalent_and_uniform(
    context: &Context,
    function: &FuncOp,
    first: &[Value; 3],
    second: &[Value; 3],
    sparse: &SparseIndexAnalysisV1,
) -> bool {
    checked_runtime_layout_is_uniform(context, function, first, sparse)
        && checked_runtime_layout_is_uniform(context, function, second, sparse)
        && first.iter().zip(second).all(|(first, second)| {
            first == second
                || sparse
                    .stable_root(context, function, *first)
                    .is_some_and(|root| {
                        sparse.stable_root(context, function, *second) == Some(root)
                    })
                || match (sparse.fact(*first).affine(), sparse.fact(*second).affine()) {
                    (Some(first), Some(second)) => first == second,
                    _ => false,
                }
        })
}

fn checked_runtime_layout_is_uniform(
    context: &Context,
    function: &FuncOp,
    layout: &[Value; 3],
    sparse: &SparseIndexAnalysisV1,
) -> bool {
    let entry = function.get_entry_block(context);
    layout.iter().all(|value| {
        (value.defining_op().is_none() && value.defining_block() == Some(entry))
            || sparse.stable_root(context, function, *value).is_some()
            || sparse.fact(*value).affine().is_some_and(|affine| {
                affine
                    .coefficients()
                    .iter()
                    .all(|coefficient| *coefficient == 0)
            })
    })
}

fn checked_invocation_is_injective(
    invocation: &SparseAffineIndexV1,
    launch_extents: &[u64],
) -> bool {
    let facts = [invocation.clone()];
    affine_facts_are_injective(&facts, launch_extents)
        || affine_facts_contain_unit_coordinate_embedding(&facts, launch_extents)
}

/// Uses exact bounded relation images to discharge affine/remainder effect
/// families that the matrix-rank fast path cannot prove. This query only
/// returns true when every potentially conflicting pair has an empty
/// cross-invocation intersection. Unsupported facts and exhausted budgets fall
/// through to the existing exact trace path.
fn presburger_proves_no_conflicts(
    effects: &[EffectV1],
    sparse: &SparseIndexAnalysisV1,
    presburger: &PlironPresburgerAnalysisV1,
    launch_extents: &[u64],
    observer: RaceObserverV1<'_, '_, '_>,
) -> bool {
    let run = || {
        let Some(invocations) = launch_extents.iter().try_fold(1_u128, |count, extent| {
            count.checked_mul(u128::from(*extent))
        }) else {
            observe_race_quota_v1(observer, "race Presburger invocation-count overflow");
            return false;
        };
        // The existing address-indexed trace is O(invocations * effects) and is
        // preferable inside its admitted domain. Presburger map intersection is
        // reserved for domains that trace intentionally refuses.
        if invocations <= u128::from(MAX_PLIRON_RACE_INVOCATIONS_V1) {
            return false;
        }
        if !effect_pair_inventory_fits_budget(effects.len()) {
            observe_race_quota_v1(observer, "race effect-pair inventory work limit");
            return false;
        }
        let relevant_pairs = (0..effects.len())
            .flat_map(|first| (first..effects.len()).map(move |second| (first, second)))
            .filter(|(first, second)| {
                let first = &effects[*first];
                let second = &effects[*second];
                first.noalias_class == second.noalias_class
                    && access_kinds_need_disjoint_coordinates(first.kind, second.kind)
                    && !atomics_are_device_compatible(first, second)
            })
            .count();
        let estimated_work = invocations
            .checked_mul((launch_extents.len() as u128).saturating_add(1))
            .and_then(|work| work.checked_mul(2))
            .and_then(|work| work.checked_mul(relevant_pairs as u128));
        if estimated_work.is_none_or(|work| work > MAX_PRESBURGER_WORK_UNITS_V1 as u128) {
            observe_race_quota_v1(observer, "race Presburger estimated work limit");
            return false;
        }
        for first_index in 0..effects.len() {
            for second_index in first_index..effects.len() {
                let first = &effects[first_index];
                let second = &effects[second_index];
                if first.noalias_class != second.noalias_class
                    || !access_kinds_need_disjoint_coordinates(first.kind, second.kind)
                    || atomics_are_device_compatible(first, second)
                {
                    continue;
                }
                let first_facts = first
                    .indices
                    .iter()
                    .map(|index| sparse.fact(*index).clone())
                    .collect::<Vec<_>>();
                let second_facts = second
                    .indices
                    .iter()
                    .map(|index| sparse.fact(*index).clone())
                    .collect::<Vec<_>>();
                let (Ok(first_map), Ok(second_map)) = (
                    presburger
                        .map_for_facts_over_extents(&first_facts, launch_extents)
                        .inspect_err(|failure| {
                            observe_race_presburger_failure_v1(failure, observer)
                        }),
                    presburger
                        .map_for_facts_over_extents(&second_facts, launch_extents)
                        .inspect_err(|failure| {
                            observe_race_presburger_failure_v1(failure, observer)
                        }),
                ) else {
                    return false;
                };
                for map in [&first_map, &second_map] {
                    match map.find_machine_overflow(PresburgerMachineIntSemanticsV1::unsigned_64())
                    {
                        PresburgerMachineRangeDecisionV1::Proved => {}
                        PresburgerMachineRangeDecisionV1::Incomplete(failure) => {
                            observe_race_presburger_failure_v1(&failure, observer);
                            return false;
                        }
                        _ => return false,
                    }
                }
                match first_map.find_cross_collision(&second_map, true) {
                    PresburgerCollisionDecisionV1::Proved => {}
                    PresburgerCollisionDecisionV1::Incomplete(failure) => {
                        observe_race_presburger_failure_v1(&failure, observer);
                        return false;
                    }
                    _ => return false,
                }
            }
        }
        true
    };
    match observer {
        None => run(),
        Some(observer) => observer.with_projection(&Ok, |_| run()),
    }
}

fn effect_pair_inventory_fits_budget(effect_count: usize) -> bool {
    let effect_count = effect_count as u128;
    effect_count
        .checked_add(1)
        .and_then(|next| effect_count.checked_mul(next))
        .map(|ordered| ordered / 2)
        .is_some_and(|pairs| pairs <= MAX_PRESBURGER_WORK_UNITS_V1 as u128)
}

fn access_kinds_need_disjoint_coordinates(first: AccessKindAttr, second: AccessKindAttr) -> bool {
    first.writes_memory() || second.writes_memory()
}

fn atomics_are_device_compatible(first: &EffectV1, second: &EffectV1) -> bool {
    first.kind.is_atomic()
        && second.kind.is_atomic()
        && [first.atomic_scope, second.atomic_scope]
            .into_iter()
            .all(|scope| {
                matches!(
                    scope,
                    Some(
                        AtomicScopeAttr::Agent | AtomicScopeAttr::Device | AtomicScopeAttr::System
                    )
                )
            })
}

fn affine_facts_contain_unit_coordinate_embedding(
    facts: &[SparseAffineIndexV1],
    launch_extents: &[u64],
) -> bool {
    let active_dimensions = launch_extents
        .iter()
        .enumerate()
        .filter_map(|(dimension, extent)| (*extent != 1).then_some(dimension))
        .collect::<Vec<_>>();
    active_dimensions.iter().all(|embedded_dimension| {
        facts.iter().any(|affine| {
            affine.constant_term() == 0
                && affine.coefficients().iter().copied().enumerate().all(
                    |(dimension, coefficient)| match launch_extents.get(dimension) {
                        None => coefficient == 0,
                        Some(1) => true,
                        Some(_) => coefficient == u64::from(dimension == *embedded_dimension),
                    },
                )
                && active_dimensions.iter().all(|dimension| {
                    affine.coefficients().get(*dimension).copied()
                        == Some(u64::from(dimension == embedded_dimension))
                })
        })
    })
}

fn same_index_formula(first: &[Value], second: &[Value], sparse: &SparseIndexAnalysisV1) -> bool {
    first.len() == second.len()
        && first
            .iter()
            .zip(second)
            .all(|(first, second)| sparse.fact(*first) == sparse.fact(*second))
}

fn one_dimensional_affine_residues_are_disjoint(
    first: &EffectV1,
    second: &EffectV1,
    sparse: &SparseIndexAnalysisV1,
    launch_extents: &[u64],
    invocation_bounds: Option<&[[Option<u64>; MAX_RANKED_MEMORY_RANK]]>,
) -> bool {
    let ([first_index], [second_index]) = (first.indices.as_slice(), second.indices.as_slice())
    else {
        return false;
    };
    let first_fact = sparse.fact(*first_index);
    let second_fact = sparse.fact(*second_index);
    let (Some(first_affine), Some(second_affine)) = (first_fact.affine(), second_fact.affine())
    else {
        return false;
    };
    if first_affine.coefficients() != second_affine.coefficients() {
        return false;
    }
    let mut active = launch_extents
        .iter()
        .enumerate()
        .filter_map(|(dimension, extent)| (*extent != 1).then_some(dimension));
    let Some(dimension) = active.next() else {
        return false;
    };
    if active.next().is_some() {
        return false;
    }
    let Some(stride) = first_affine.coefficients().get(dimension).copied() else {
        return false;
    };
    if stride == 0
        || first_affine
            .coefficients()
            .iter()
            .enumerate()
            .any(|(candidate, coefficient)| candidate != dimension && *coefficient != 0)
    {
        return false;
    }
    let first_extents = effective_launch_extents(first, launch_extents, invocation_bounds);
    let second_extents = effective_launch_extents(second, launch_extents, invocation_bounds);
    affine_is_total_over_launch(first_affine, &first_extents)
        && affine_is_total_over_launch(second_affine, &second_extents)
        && first_affine.constant_term() % stride != second_affine.constant_term() % stride
}

fn affine_map_is_injective(
    indices: &[Value],
    sparse: &SparseIndexAnalysisV1,
    launch_extents: &[u64],
) -> bool {
    let facts = indices
        .iter()
        .map(|index| sparse.fact(*index).affine().cloned())
        .collect::<Option<Vec<_>>>();
    facts.is_some_and(|facts| {
        affine_facts_are_injective(&facts, launch_extents)
            || affine_facts_contain_unit_coordinate_embedding(&facts, launch_extents)
    })
}
