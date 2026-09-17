fn allocation_origin_name_v1(origin: u64) -> String {
    use std::fmt::Write as _;

    let mut name = String::with_capacity(NUMERIC_RACE_NAME_BYTES_V1);
    write!(name, "allocation origin {origin}").expect("writing a numeric diagnostic cannot fail");
    name
}

#[derive(Clone)]
struct EffectV1 {
    identity: EffectIdentityV1,
    view_name: String,
    kind: AccessKindAttr,
    location: RankedRaceLocationV1,
    indices: Vec<Value>,
    checked_success: Option<Value>,
    atomic_scope: Option<AtomicScopeAttr>,
    atomic_ordering: Option<AtomicOrderingAttr>,
    noalias_class: u64,
    conservative: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum EffectIdentityV1 {
    View(Value),
    Allocation(u64),
    AllocationSite(RankedRaceLocationV1),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct AddressKeyV1 {
    allocation_class: u64,
    indices: Vec<u64>,
}

#[derive(Clone, Debug, Default)]
struct WitnessPairV1 {
    first: Option<RankedRaceWitnessV1>,
    second: Option<RankedRaceWitnessV1>,
}

impl WitnessPairV1 {
    fn different_from(&self, invocation: &[u64]) -> Option<&RankedRaceWitnessV1> {
        self.first
            .as_ref()
            .filter(|witness| witness.invocation != invocation)
            .or_else(|| {
                self.second
                    .as_ref()
                    .filter(|witness| witness.invocation != invocation)
            })
    }

    fn insert(&mut self, witness: RankedRaceWitnessV1) {
        if self.first.is_none() {
            self.first = Some(witness);
        } else if self
            .first
            .as_ref()
            .is_some_and(|first| first.invocation != witness.invocation)
            && self.second.is_none()
        {
            self.second = Some(witness);
        }
    }
}

#[derive(Clone, Debug, Default)]
struct AddressStateV1 {
    reads: WitnessPairV1,
    writes: WitnessPairV1,
    atomic_reads: WitnessPairV1,
    atomic_writes: WitnessPairV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ConflictClassV1 {
    identity: EffectIdentityV1,
    first: RankedRaceLocationV1,
    second: RankedRaceLocationV1,
    first_kind: AccessKindAttr,
    second_kind: AccessKindAttr,
}

#[cfg(test)]
pub(crate) fn run_pliron_ranked_race_check_v1(
    context: &Context,
    function: &FuncOp,
) -> RankedRaceReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    if !run_pliron_ranked_bounds_check_with_analyses_v1(context, function, &mut analyses).is_clean()
    {
        return one(RankedRaceFindingV1::BoundsPrerequisiteRejected);
    }
    run_pliron_ranked_race_check_with_analyses_v1(context, function, &mut analyses)
}

pub(crate) fn run_pliron_ranked_race_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> RankedRaceReportV1 {
    analyses.prepare_sparse_indices(context, function);
    analyses.prepare_presburger(context, function);
    analyses.prepare_provenance_alias(context, function);
    if let Err(failure) = analyses.sparse_indices() {
        return one(RankedRaceFindingV1::SparseIndexAnalysisFailed {
            detail: sparse_failure(failure),
        });
    }
    analyses.prepare_execution_layout(context, function);
    let sparse = match analyses.sparse_indices() {
        Ok(sparse) => sparse,
        Err(failure) => {
            return one(RankedRaceFindingV1::SparseIndexAnalysisFailed {
                detail: sparse_failure(failure),
            });
        }
    };
    let presburger = match analyses.presburger() {
        Ok(presburger) => presburger,
        Err(failure) => {
            return one(RankedRaceFindingV1::SparseIndexAnalysisFailed {
                detail: sparse_failure(failure),
            });
        }
    };
    let provenance = match analyses.provenance_alias() {
        Ok(provenance) => provenance,
        Err(failure) => {
            return one(RankedRaceFindingV1::AllocationContractUnavailable {
                detail: failure.bounded_description_v1(),
            });
        }
    };
    let inventory = analyses
        .function_inventory_handle()
        .expect("race prerequisites prepare the function inventory");
    let mut effects = Vec::new();
    let mut has_global_fence = false;
    for site in inventory.operations() {
        let block_index = site.block();
        let operation_index = site.operation();
        let operation = Operation::get_op_dyn(site.pointer(), context);
        if operation
            .downcast_ref::<FenceOp>()
            .is_some_and(|fence| fence.address_space(context) == Some(AddressSpaceAttr::Global))
        {
            has_global_fence = true;
        }
        if let Some(effect) = operation.downcast_ref::<AllocationEffectOp>() {
            let Some(kind) = effect.kind(context) else {
                return one(RankedRaceFindingV1::AllocationContractUnavailable {
                    detail: "a whole-allocation effect has no access kind".to_owned(),
                });
            };
            let Some(memory_space) = effect.memory_space(context) else {
                return one(RankedRaceFindingV1::AllocationContractUnavailable {
                    detail: "a whole-allocation effect has no memory space".to_owned(),
                });
            };
            let allocation_origin = effect.allocation_origin(context).unwrap_or(0);
            let noalias_class = effect.noalias_class(context).unwrap_or(0);
            match memory_space {
                MemorySpaceAttr::Global => {}
                MemorySpaceAttr::Workgroup
                    if is_supported_allocation_effect_contract_v1(
                        kind,
                        memory_space,
                        allocation_origin,
                        noalias_class,
                    ) =>
                {
                    // The production source join authenticates the typed
                    // transpose lifecycle; no global race is represented.
                    continue;
                }
                MemorySpaceAttr::Private | MemorySpaceAttr::Workgroup => {
                    return one(RankedRaceFindingV1::AllocationContractUnavailable {
                        detail:
                            "a whole-allocation effect uses an unsupported non-global memory space"
                                .to_owned(),
                    });
                }
            }
            let location = RankedRaceLocationV1 {
                block: block_index,
                operation: operation_index,
            };
            effects.push(EffectV1 {
                identity: if allocation_origin == 0 {
                    EffectIdentityV1::AllocationSite(location)
                } else {
                    EffectIdentityV1::Allocation(allocation_origin)
                },
                view_name: allocation_origin_name_v1(allocation_origin),
                kind,
                location,
                indices: vec![],
                checked_success: None,
                atomic_scope: None,
                atomic_ordering: None,
                noalias_class,
                conservative: true,
            });
            continue;
        }
        let Some(access) = operation.downcast_ref::<RankedAccessOp>() else {
            continue;
        };
        let view = access.view(context);
        let Some(definition) = view.defining_op() else {
            return one(RankedRaceFindingV1::UnresolvedIndex {
                block: block_index,
                operation: operation_index,
                dimension: 0,
                value: "view-without-definition".to_owned(),
            });
        };
        let definition = Operation::get_op_dyn(definition, context);
        let Some(view_op) = definition.downcast_ref::<RankedViewOp>() else {
            return one(RankedRaceFindingV1::UnresolvedIndex {
                block: block_index,
                operation: operation_index,
                dimension: 0,
                value: "foreign-view-definition".to_owned(),
            });
        };
        match view_op.memory_space(context) {
            Some(MemorySpaceAttr::Private) => continue,
            // Workgroup effects are checked with barrier epochs by the
            // mandatory workgroup-memory pass that follows this pass.
            Some(MemorySpaceAttr::Workgroup) => continue,
            Some(MemorySpaceAttr::Global) => {}
            None => {
                return one(RankedRaceFindingV1::UnresolvedIndex {
                    block: block_index,
                    operation: operation_index,
                    dimension: 0,
                    value: "view-without-memory-space".to_owned(),
                });
            }
        }
        let Some(kind) = access.kind(context) else {
            return one(RankedRaceFindingV1::UnresolvedIndex {
                block: block_index,
                operation: operation_index,
                dimension: 0,
                value: "access-without-kind".to_owned(),
            });
        };
        effects.push(EffectV1 {
            identity: EffectIdentityV1::View(view),
            view_name: view.id(context).into(),
            kind,
            location: RankedRaceLocationV1 {
                block: block_index,
                operation: operation_index,
            },
            indices: access.indices(context),
            checked_success: access.checked_success(context),
            atomic_scope: access.atomic_scope(context),
            atomic_ordering: access.atomic_ordering(context),
            noalias_class: view_op.noalias_class(context).unwrap_or(0),
            conservative: false,
        });
    }

    for effect in &mut effects {
        effect.noalias_class =
            provenance.canonical_class(MemorySpaceAttr::Global, effect.noalias_class);
    }
    let classes_with_writes = effects
        .iter()
        .filter_map(|effect| effect.kind.writes_memory().then_some(effect.noalias_class))
        .collect::<HashSet<_>>();
    let layout = match analyses.execution_layout() {
        Ok(layout) => layout,
        Err(failure) => {
            return one(RankedRaceFindingV1::ExecutionLayoutUnavailable {
                detail: match failure {
                    PlironTraceFailureV1::InvalidExecutionLayout => {
                        "gpu.execution_layout is malformed or duplicated".to_owned()
                    }
                    _ => format!("execution layout extraction failed: {failure:?}"),
                },
            });
        }
    };
    let launch_extents = if let Some(layout) = layout {
        for dimension in 0..sparse.launch_extents().len().max(3) {
            if let Some(declared) = sparse.declared_launch_extent(dimension) {
                let Some(layout_extent) = layout.global_extents.get(dimension).copied() else {
                    return one(RankedRaceFindingV1::ExecutionLayoutUnavailable {
                        detail: format!(
                            "invocation coordinate axis {dimension} is outside the three-dimensional gpu.execution_layout"
                        ),
                    });
                };
                if declared != 0 && layout_extent != declared {
                    return one(RankedRaceFindingV1::ExecutionLayoutUnavailable {
                        detail: format!(
                            "invocation coordinate axis {dimension} declares extent {declared}, inconsistent with gpu.execution_layout"
                        ),
                    });
                }
            }
        }
        layout.global_extents.to_vec()
    } else if sparse.has_declared_launch_extent() {
        sparse.launch_extents().to_vec()
    } else if effects.is_empty() || classes_with_writes.is_empty() {
        vec![1]
    } else {
        return one(RankedRaceFindingV1::ExecutionLayoutUnavailable {
            detail: "concurrent memory effects require a declared execution domain even when the kernel does not read an invocation coordinate".to_owned(),
        });
    };

    let static_publication = match derive_static_publication_v1(
        context, &inventory, sparse, layout, &effects,
    ) {
        Ok(proof) => proof,
        Err(finding) => return one(*finding),
    };
    if let Some(proof) = static_publication {
        // Preserve the exact cross-invocation discharge roster in the report.
        // Only its closed five-effect protocol leaves generic pair analysis;
        // every other allocation and access remains subject to ordinary checks.
        effects.retain(|effect| {
            ![0, 1, 2, 3, 5].iter().any(|ordinal| proof.sites[*ordinal] == effect.location)
        });
    }
    // The publication proof inspected the complete effect inventory first,
    // including unknown-alias reads which cannot safely leave its roster.
    // Unrelated input-only classes need no generic address reconstruction.
    effects.retain(|effect| classes_with_writes.contains(&effect.noalias_class));
    let invocation_bounds = invocation_upper_bounds_by_block(context, function, &inventory);
    if symbolically_proves_disjoint(
        context,
        function,
        &effects,
        sparse,
        &launch_extents,
        invocation_bounds.as_deref(),
    ) || presburger_proves_no_conflicts(&effects, sparse, presburger, &launch_extents)
    {
        return RankedRaceReportV1 { findings: Vec::new(), static_publication: static_publication.into_iter().collect() };
    }
    let release_signal_views = effects
        .iter()
        .filter_map(|effect| {
            (effect.kind.is_atomic()
                && effect.kind.writes_memory()
                && matches!(
                    effect.atomic_ordering,
                    Some(
                        AtomicOrderingAttr::Release
                            | AtomicOrderingAttr::AcquireRelease
                            | AtomicOrderingAttr::SequentiallyConsistent
                    )
                )
                && effect
                    .atomic_scope
                    .is_some_and(|scope| scope.rank() >= AtomicScopeAttr::Agent.rank()))
            .then_some(effect.noalias_class)
        })
        .collect::<HashSet<_>>();
    let acquire_signal_views = effects
        .iter()
        .filter_map(|effect| {
            (effect.kind.is_atomic()
                && effect.kind.reads_memory()
                && matches!(
                    effect.atomic_ordering,
                    Some(
                        AtomicOrderingAttr::Acquire
                            | AtomicOrderingAttr::AcquireRelease
                            | AtomicOrderingAttr::SequentiallyConsistent
                    )
                )
                && effect
                    .atomic_scope
                    .is_some_and(|scope| scope.rank() >= AtomicScopeAttr::Agent.rank()))
            .then_some(effect.noalias_class)
        })
        .collect::<HashSet<_>>();
    let atomic_signal_views = release_signal_views
        .intersection(&acquire_signal_views)
        .copied()
        .collect::<HashSet<_>>();
    if let Some(dimension) = launch_extents.iter().position(|extent| *extent == 0) {
        return one(RankedRaceFindingV1::DynamicLaunchExtent { dimension });
    }
    let Some(invocation_count) = launch_extents
        .iter()
        .try_fold(1_u64, |total, extent| total.checked_mul(*extent))
    else {
        return one(RankedRaceFindingV1::LaunchDomainTooLarge {
            invocations: u64::MAX,
            limit: MAX_PLIRON_RACE_INVOCATIONS_V1,
        });
    };
    if invocation_count > MAX_PLIRON_RACE_INVOCATIONS_V1 {
        return one(RankedRaceFindingV1::LaunchDomainTooLarge {
            invocations: invocation_count,
            limit: MAX_PLIRON_RACE_INVOCATIONS_V1,
        });
    }
    if invocation_count <= 1 {
        return clean();
    }
    if let Some(effect) = effects
        .iter()
        .find(|effect| effect.conservative && classes_with_writes.contains(&effect.noalias_class))
    {
        return one(RankedRaceFindingV1::AllocationContractUnavailable {
            detail: format!(
                "whole-allocation read on {} may overlap a writable effect in alias class {} across concurrent invocations",
                effect.view_name, effect.noalias_class
            ),
        });
    }

    let zero_invocation = vec![0; launch_extents.len()];
    let mut raw_evaluation_steps = 0;
    for effect in &effects {
        for (dimension, index) in effect.indices.iter().copied().enumerate() {
            if sparse.fact(index).evaluate(&zero_invocation).is_none()
                && evaluate_raw_index_at_invocation_v1(
                    context,
                    index,
                    &zero_invocation,
                    &mut raw_evaluation_steps,
                )
                .is_none()
            {
                return one(RankedRaceFindingV1::UnresolvedIndex {
                    block: effect.location.block,
                    operation: effect.location.operation,
                    dimension,
                    value: index.id(context).into(),
                });
            }
        }
    }

    let mut addresses: HashMap<AddressKeyV1, AddressStateV1> = HashMap::new();
    let mut findings = Vec::new();
    let mut conflict_classes = HashSet::new();
    let mut effect_instances = 0_usize;
    for linear_invocation in 0..invocation_count {
        let invocation = decode_invocation(linear_invocation, &launch_extents);
        for effect in &effects {
            effect_instances = effect_instances.saturating_add(1);
            if effect_instances > MAX_PLIRON_RACE_EFFECT_INSTANCES_V1 {
                return one(RankedRaceFindingV1::EffectInstanceLimitExceeded {
                    actual: effect_instances,
                    limit: MAX_PLIRON_RACE_EFFECT_INSTANCES_V1,
                });
            }
            let Some(indices) = effect
                .indices
                .iter()
                .map(|index| {
                    sparse.fact(*index).evaluate(&invocation).or_else(|| {
                        evaluate_raw_index_at_invocation_v1(
                            context,
                            *index,
                            &invocation,
                            &mut raw_evaluation_steps,
                        )
                    })
                })
                .collect::<Option<Vec<_>>>()
            else {
                let (dimension, value) = effect
                    .indices
                    .iter()
                    .copied()
                    .enumerate()
                    .find(|(_, index)| sparse.fact(*index).evaluate(&invocation).is_none())
                    .expect("failed index evaluation identifies an unresolved index");
                return one(RankedRaceFindingV1::UnresolvedIndex {
                    block: effect.location.block,
                    operation: effect.location.operation,
                    dimension,
                    value: value.id(context).into(),
                });
            };
            let key = AddressKeyV1 {
                allocation_class: effect.noalias_class,
                indices,
            };
            let scoped_identity = layout.and_then(|layout| layout.scoped_identity(&invocation));
            let witness = RankedRaceWitnessV1 {
                location: effect.location,
                access: effect.kind,
                invocation: invocation.clone(),
                grid: layout.map_or(0, |layout| layout.grid),
                workgroup: scoped_identity.map(|identity| identity.0),
                subgroup: scoped_identity.map(|identity| identity.1),
                lane: scoped_identity.map(|identity| identity.2),
                atomic_scope: effect.atomic_scope,
            };
            let state = addresses.entry(key.clone()).or_default();
            let conflict = conflicting_witness(state, effect.kind, &witness).cloned();
            if let Some(first) = conflict {
                let class = ConflictClassV1 {
                    identity: effect.identity,
                    first: first.location,
                    second: effect.location,
                    first_kind: first.access,
                    second_kind: effect.kind,
                };
                if conflict_classes.insert(class) {
                    let finding = if first.access.is_atomic() && witness.access.is_atomic() {
                        if layout.is_none() {
                            RankedRaceFindingV1::ExecutionLayoutUnavailable {
                                detail: format!(
                                    "overlapping narrow-scope atomics on {} require retained workgroup identity",
                                    effect.view_name
                                ),
                            }
                        } else {
                            RankedRaceFindingV1::InsufficientAtomicScope {
                                view: effect.view_name.clone(),
                                indices: key.indices,
                                first,
                                second: witness.clone(),
                            }
                        }
                    } else if has_global_fence
                        || atomic_signal_views
                            .iter()
                            .any(|class| *class != effect.noalias_class)
                    {
                        RankedRaceFindingV1::HappensBeforeIncomplete {
                            view: effect.view_name.clone(),
                            detail: if atomic_signal_views
                                .iter()
                                .any(|class| *class != effect.noalias_class)
                            {
                                "release/acquire atomics require an authenticated read-from relation before they can publish ordinary memory across invocations".to_owned()
                            } else {
                                "a non-collective fence alone does not establish a cross-invocation synchronizes-with edge".to_owned()
                            },
                        }
                    } else {
                        RankedRaceFindingV1::ConflictingEffects {
                            view: effect.view_name.clone(),
                            indices: key.indices,
                            first,
                            second: witness.clone(),
                        }
                    };
                    findings.push(finding);
                    if findings.len() > MAX_PLIRON_RACE_FINDINGS_V1 {
                        return one(RankedRaceFindingV1::FindingLimitExceeded {
                            actual: findings.len(),
                            limit: MAX_PLIRON_RACE_FINDINGS_V1,
                        });
                    }
                }
            }
            insert_witness(state, witness);
        }
    }
    RankedRaceReportV1 { findings, static_publication: static_publication.into_iter().collect() }
}

pub(crate) fn require_pliron_ranked_race_freedom_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<RankedRaceReportV1, RankedRaceCheckErrorV1> {
    let report = run_pliron_ranked_race_check_with_analyses_v1(context, function, analyses);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(RankedRaceCheckErrorV1 { report })
    }
}

#[cfg(test)]
pub(crate) fn require_pliron_ranked_race_freedom_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<RankedRaceReportV1, RankedRaceCheckErrorV1> {
    let report = run_pliron_ranked_race_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(RankedRaceCheckErrorV1 { report })
    }
}
