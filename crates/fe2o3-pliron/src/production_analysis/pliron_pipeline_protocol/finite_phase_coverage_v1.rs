// This private input is constructed only after the finite-loop verifier has
// proved full-workgroup participation and classified every phase access.
struct FinitePhaseAccessInputV1<'a> {
    pipeline: PlironOperationSiteV1,
    buffers: u32,
    phase: u64,
    summary: &'a CanonicalEpochLoopV1,
    stage: EventSiteV1,
    commit: EventSiteV1,
    consume: EventSiteV1,
    release: EventSiteV1,
    accesses: &'a [AccessSiteV1],
    dominators: &'a [HashSet<usize>],
}

fn finite_site_precedes_v1(
    first: PlironOperationSiteV1,
    second: PlironOperationSiteV1,
    dominators: &[HashSet<usize>],
) -> bool {
    if first.block() == second.block() {
        first.operation() < second.operation()
    } else {
        dominators
            .get(second.block())
            .is_some_and(|set| set.contains(&first.block()))
    }
}

fn finite_strip_lossless_casts_v1(
    context: &Context,
    mut value: Value,
    maximum: u64,
    resources: &mut EquivalenceResourceMeterV1,
) -> Option<Value> {
    if resources.exhausted() {
        return None;
    }
    resources.begin_query().ok()?;
    let mut steps = 0;
    loop {
        resources
            .charge_cursor(&mut steps, MAX_EQUIVALENCE_WORK_V1)
            .ok()?;
        let Some(definition) = value.defining_op() else {
            return Some(value);
        };
        let operation = Operation::get_op_dyn(definition, context);
        let Some(cast) = operation.downcast_ref::<IndexUnsignedCastOp>() else {
            return Some(value);
        };
        if cast.inclusive_upper_bound(context)? < maximum {
            return None;
        }
        value = cast.source(context);
    }
}

fn finite_full_lane_coordinate_v1(
    context: &Context,
    value: Value,
    global_extent: u64,
    resources: &mut EquivalenceResourceMeterV1,
) -> bool {
    let Some(global_maximum) = global_extent.checked_sub(1) else {
        return false;
    };
    let Some(value) = finite_strip_lossless_casts_v1(context, value, 127, resources) else {
        return false;
    };
    let Some(definition) = value.defining_op() else {
        return false;
    };
    let operation = Operation::get_op_dyn(definition, context);
    let Some(remainder) = operation.downcast_ref::<IndexBinaryOp>() else {
        return false;
    };
    if remainder.kind(context) != Some(IndexBinaryKindAttr::Remainder)
        || index_constant(context, remainder.rhs(context)) != Some(128)
    {
        return false;
    }
    let Some(global) =
        finite_strip_lossless_casts_v1(context, remainder.lhs(context), global_maximum, resources)
    else {
        return false;
    };
    global.defining_op().is_some_and(|definition| {
        Operation::get_op_dyn(definition, context)
            .downcast_ref::<dialect_kernel::InvocationIndexOp>()
            .is_some_and(|invocation| {
                invocation.dimension(context) == Some(0)
                    && invocation
                        .launch_extent(context)
                        .is_some_and(|extent| extent == 0 || extent == global_extent)
            })
    })
}

fn finite_phase_slot_matches_v1(
    context: &Context,
    slot: Value,
    site: PlironOperationSiteV1,
    input: &FinitePhaseAccessInputV1<'_>,
    resources: &mut EquivalenceResourceMeterV1,
) -> bool {
    let Some(slot) = finite_strip_lossless_casts_v1(context, slot, 1, resources) else {
        return false;
    };
    if index_constant(context, slot) == Some(input.phase) {
        return true;
    }
    let Some(definition) = slot.defining_op() else {
        return false;
    };
    let operation = Operation::get_op_dyn(definition, context);
    let Some(remainder) = operation.downcast_ref::<IndexBinaryOp>() else {
        return false;
    };
    remainder.kind(context) == Some(IndexBinaryKindAttr::Remainder)
        && index_constant(context, remainder.rhs(context)) == Some(u64::from(input.buffers))
        && input
            .summary
            .inductions
            .get(&site.block())
            .is_some_and(|induction| {
                finite_epoch_offset_v1(context, remainder.lhs(context), *induction, resources)
                    == Some(input.phase)
            })
}

fn verify_finite_phase_accesses_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    input: FinitePhaseAccessInputV1<'_>,
    resources: &mut EquivalenceResourceMeterV1,
) -> Result<(usize, usize, bool), PlironPipelineProtocolFindingV1> {
    use crate::production_analysis::pliron_invocation_trace::pliron_execution_layout_with_inventory_v1;
    let reject = |site, detail| invalid(input.pipeline, site, detail);
    let layout = pliron_execution_layout_with_inventory_v1(context, inventory)
        .ok()
        .flatten()
        .ok_or_else(|| reject(None, "finite pipeline lacks an exact execution layout"))?;
    if input.buffers != 2
        || input.phase >= 2
        || layout.workgroup_extents != [128, 1, 1]
        || layout.subgroup_size != 64
        || layout.execution_domain != dialect_gpu::ExecutionDomainAttr::FullPhysicalWorkgroups
        || layout.global_extents[0] == 0
        || !layout.global_extents[0].is_multiple_of(128)
        || layout.global_extents[1..] != [1, 1]
    {
        return Err(reject(
            None,
            "finite pipeline requires complete physical 128-lane workgroups",
        ));
    }
    let operation = Operation::get_op_dyn(input.pipeline.pointer(), context);
    let create = operation
        .downcast_ref::<PipelineCreateOp>()
        .ok_or_else(|| reject(None, "finite pipeline owner is not its creation operation"))?;
    let view = create.view(context);
    let view_definition = view
        .defining_op()
        .ok_or_else(|| reject(None, "finite pipeline view lacks a defining owner"))?;
    let view_operation = Operation::get_op_dyn(view_definition, context);
    let ranked = view_operation
        .downcast_ref::<RankedViewOp>()
        .ok_or_else(|| reject(None, "finite pipeline view is not compiler-ranked storage"))?;
    let view_type = ranked
        .view_type(context)
        .ok_or_else(|| reject(None, "finite pipeline view type is absent"))?;
    let ty = view_type.deref(context);
    if ty.shape() != [2, 128]
        || ty.element_width() != 32
        || !ty.writable()
        || ranked.memory_space(context) != Some(dialect_kernel::MemorySpaceAttr::Workgroup)
        || create.pipeline_type(context).is_none_or(|ty| {
            let ty = ty.deref(context);
            ty.buffers() != 2 || ty.prefetch_distance() != 1
        })
    {
        return Err(reject(
            None,
            "finite pipeline owner or physical element layout changed",
        ));
    }
    // Initialization is tied to this exact view, not an equal-width scalar or
    // another allocation. Opaque workgroup effects cannot prove non-escape.
    let owner = pipeline_storage_contract(context, view);
    let mut declared_extents = [None; 3];
    for site in inventory.operations() {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        if let Some(invocation) = operation.downcast_ref::<dialect_kernel::InvocationIndexOp>() {
            let dimension = invocation
                .dimension(context)
                .and_then(|dimension| usize::try_from(dimension).ok())
                .filter(|dimension| *dimension < layout.global_extents.len())
                .ok_or_else(|| {
                    reject(
                        Some(*site),
                        "finite pipeline invocation dimension is not in its layout",
                    )
                })?;
            let extent = invocation.launch_extent(context).ok_or_else(|| {
                reject(Some(*site), "finite pipeline invocation extent is absent")
            })?;
            // Zero is the dialect's dynamic declaration, bound by this exact
            // validated layout, never by a nominal workgroup-size assumption.
            if (extent != 0 && extent != layout.global_extents[dimension])
                || declared_extents[dimension].is_some_and(|first| first != extent)
            {
                return Err(reject(
                    Some(*site),
                    "finite pipeline invocation declarations conflict with its layout",
                ));
            }
            declared_extents[dimension] = Some(extent);
        }
        if operation
            .downcast_ref::<AllocationEffectOp>()
            .is_some_and(|effect| {
                effect.memory_space(context) == Some(dialect_kernel::MemorySpaceAttr::Workgroup)
            })
        {
            return Err(reject(
                Some(*site),
                "opaque workgroup effects invalidate finite initialization coverage",
            ));
        }
        if let Some(other) = operation.downcast_ref::<RankedViewOp>()
            && other.result(context) != view
            && other.memory_space(context) == Some(dialect_kernel::MemorySpaceAttr::Workgroup)
        {
            let other = pipeline_storage_contract(context, other.result(context));
            if owner
                .1
                .zip(other.1)
                .is_none_or(|(left, right)| left == right)
                || owner
                    .0
                    .zip(other.0)
                    .is_some_and(|(left, right)| left == right)
            {
                return Err(reject(
                    Some(*site),
                    "finite pipeline view may alias another workgroup view",
                ));
            }
        }
    }
    for (event, kind) in [
        (input.stage, PipelineEventKindAttr::Stage),
        (input.commit, PipelineEventKindAttr::Commit),
        (input.consume, PipelineEventKindAttr::Consume),
        (input.release, PipelineEventKindAttr::Release),
    ] {
        let operation = Operation::get_op_dyn(event.site.pointer(), context);
        let valid = operation
            .downcast_ref::<PipelineEventOp>()
            .is_some_and(|actual| {
                actual.pipeline(context) == create.pipeline(context)
                    && actual.kind(context) == Some(kind)
                    && actual.epoch(context) == event.epoch
                    && actual.slot(context) == event.slot
                    && input
                        .summary
                        .inductions
                        .get(&event.site.block())
                        .is_some_and(|induction| {
                            finite_epoch_offset_v1(context, event.epoch, *induction, resources)
                                == Some(input.phase)
                        })
                    && finite_phase_slot_matches_v1(
                        context, event.slot, event.site, &input, resources,
                    )
            });
        if !valid {
            return Err(reject(
                Some(event.site),
                "finite pipeline event owner, epoch or slot changed",
            ));
        }
    }
    let mut writes = 0;
    let mut reads = 0;
    for access in input.accesses {
        let operation = Operation::get_op_dyn(access.site.pointer(), context);
        let actual = operation.downcast_ref::<RankedAccessOp>().ok_or_else(|| {
            reject(
                Some(access.site),
                "finite phase access is not a ranked operation",
            )
        })?;
        if actual.view(context) != view
            || actual.kind(context) != Some(access.kind)
            || actual.indices(context) != access.indices
            || actual.checked_success(context).is_some()
            || access.indices.len() != 2
            || access.indices.first() != Some(&access.slot)
            || !finite_phase_slot_matches_v1(context, access.slot, access.site, &input, resources)
        {
            return Err(reject(
                Some(access.site),
                "finite phase access owner, rank, mask or slot changed",
            ));
        }
        let coordinate = access.indices[1];
        if access.kind == AccessKindAttr::Write {
            if !finite_site_precedes_v1(input.stage.site, access.site, input.dominators)
                || !finite_site_precedes_v1(access.site, input.commit.site, input.dominators)
                || !finite_full_lane_coordinate_v1(
                    context,
                    coordinate,
                    layout.global_extents[0],
                    resources,
                )
            {
                return Err(reject(
                    Some(access.site),
                    "finite phase does not initialize every lane before commit",
                ));
            }
            writes += 1;
        } else if access.kind == AccessKindAttr::Read {
            let bounded = index_constant(context, coordinate).is_some_and(|value| value < 128)
                || finite_full_lane_coordinate_v1(
                    context,
                    coordinate,
                    layout.global_extents[0],
                    resources,
                )
                || input.summary.finite_inner_loops.iter().any(|inner| {
                    inner.bound > 0
                        && inner.bound <= 128
                        && inner.body_members.contains(&access.site.block())
                        && inner
                            .inductions
                            .get(&access.site.block())
                            .is_some_and(|induction| {
                                index_values_equivalent(context, coordinate, *induction, resources)
                            })
                });
            if !bounded
                || !finite_site_precedes_v1(input.consume.site, access.site, input.dominators)
                || finite_site_precedes_v1(input.release.site, access.site, input.dominators)
            {
                return Err(reject(
                    Some(access.site),
                    "finite phase read lacks bounded initialized coordinates in its consume window",
                ));
            }
            reads += 1;
        } else {
            return Err(reject(
                Some(access.site),
                "finite phase access is not a plain read or write",
            ));
        }
    }
    if writes != 1 || resources.exhausted() {
        return Err(reject(
            None,
            "finite phase lacks one complete initialized lane image",
        ));
    }
    Ok((writes, reads, true))
}
