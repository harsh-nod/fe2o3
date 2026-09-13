struct DynamicScheduleInputV1<'a> {
    schedule: &'a [EventSiteV1],
    accesses: &'a [AccessSiteV1],
    summary: &'a CanonicalEpochLoopV1,
}

fn verify_dynamic_schedule(
    context: &Context,
    pipeline: PlironOperationSiteV1,
    buffers: u32,
    distance: u32,
    input: DynamicScheduleInputV1<'_>,
    equivalence_resources: &mut EquivalenceResourceMeterV1,
) -> Result<(usize, usize, bool), PlironPipelineProtocolFindingV1> {
    let DynamicScheduleInputV1 {
        schedule,
        accesses,
        summary,
    } = input;
    let mut prologue = Vec::new();
    let mut body = Vec::new();
    let mut drain = Vec::new();
    let prologue_start = summary
        .prologue
        .iter()
        .position(|block| *block == pipeline.block())
        .unwrap_or(0);
    let prologue_positions = summary.prologue[prologue_start..]
        .iter()
        .enumerate()
        .map(|(position, block)| (*block, position))
        .collect::<HashMap<_, _>>();
    let body_positions = summary
        .body
        .iter()
        .enumerate()
        .map(|(position, block)| (*block, position))
        .collect::<HashMap<_, _>>();
    let drain_positions = summary
        .drain
        .iter()
        .enumerate()
        .map(|(position, block)| (*block, position))
        .collect::<HashMap<_, _>>();
    for event in schedule {
        if let Some(position) = prologue_positions.get(&event.site.block()).copied() {
            prologue.push((position, *event));
        } else if let Some(position) = body_positions.get(&event.site.block()).copied() {
            body.push((position, *event));
        } else if let Some(position) = drain_positions.get(&event.site.block()).copied() {
            drain.push((position, *event));
        } else {
            return Err(invalid(
                pipeline,
                Some(event.site),
                "event is outside the canonical prologue, loop body, or drain block",
            ));
        }
    }
    prologue.sort_by_key(|(position, event)| (*position, event.site.operation()));
    body.sort_by_key(|(position, event)| (*position, event.site.operation()));
    drain.sort_by_key(|(position, event)| (*position, event.site.operation()));
    let prologue = prologue
        .into_iter()
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let drain = drain
        .into_iter()
        .map(|(_, event)| event)
        .collect::<Vec<_>>();

    let expected_prologue = usize::try_from(distance).unwrap_or(usize::MAX) * 2;
    if prologue.len() != expected_prologue {
        return Err(invalid(
            pipeline,
            prologue.first().map(|event| event.site),
            &format!(
                "dynamic pipeline requires {expected_prologue} prologue events for prefetch distance {distance}, found {}",
                prologue.len()
            ),
        ));
    }
    for epoch in 0..u64::from(distance) {
        require_event(
            context,
            pipeline,
            &prologue[(epoch as usize) * 2],
            PipelineEventKindAttr::Stage,
            EpochExpectationV1::Constant(epoch),
            buffers,
            equivalence_resources,
        )?;
        require_event(
            context,
            pipeline,
            &prologue[(epoch as usize) * 2 + 1],
            PipelineEventKindAttr::Commit,
            EpochExpectationV1::Constant(epoch),
            buffers,
            equivalence_resources,
        )?;
    }

    let body = body.into_iter().map(|(_, event)| event).collect::<Vec<_>>();
    if body.len() != 5 {
        return Err(invalid(
            pipeline,
            body.first().map(|event| event.site),
            &format!(
                "dynamic loop body requires exactly five lifecycle events, found {}",
                body.len()
            ),
        ));
    }
    let body_induction = |event: &EventSiteV1| {
        summary
            .inductions
            .get(&event.site.block())
            .copied()
            .expect("body events are classified only in summarized blocks")
    };
    let expected = [
        (PipelineEventKindAttr::Stage, u64::from(distance)),
        (PipelineEventKindAttr::Commit, u64::from(distance)),
        (PipelineEventKindAttr::Wait, 0),
        (PipelineEventKindAttr::Consume, 0),
        (PipelineEventKindAttr::Release, 0),
    ];
    for (event, (kind, offset)) in body.iter().zip(expected) {
        require_event(
            context,
            pipeline,
            event,
            kind,
            EpochExpectationV1::Offset(body_induction(event), offset),
            buffers,
            equivalence_resources,
        )?;
    }

    let expected_drain = usize::try_from(distance).unwrap_or(usize::MAX) * 3;
    if drain.len() != expected_drain {
        return Err(invalid(
            pipeline,
            drain.first().map(|event| event.site),
            &format!(
                "dynamic pipeline requires {expected_drain} drain events for prefetch distance {distance}, found {}",
                drain.len()
            ),
        ));
    }
    for offset in 0..u64::from(distance) {
        for (inner, kind) in [
            PipelineEventKindAttr::Wait,
            PipelineEventKindAttr::Discard,
            PipelineEventKindAttr::Release,
        ]
        .into_iter()
        .enumerate()
        {
            require_event(
                context,
                pipeline,
                &drain[(offset as usize) * 3 + inner],
                kind,
                EpochExpectationV1::Offset(summary.bound, offset),
                buffers,
                equivalence_resources,
            )?;
        }
    }
    verify_dynamic_accesses(
        context,
        pipeline,
        buffers,
        schedule,
        accesses,
        summary,
        equivalence_resources,
    )
}

#[derive(Clone, Copy)]
enum EpochExpectationV1 {
    Constant(u64),
    Offset(Value, u64),
}

fn require_event(
    context: &Context,
    pipeline: PlironOperationSiteV1,
    event: &EventSiteV1,
    kind: PipelineEventKindAttr,
    epoch: EpochExpectationV1,
    buffers: u32,
    equivalence_resources: &mut EquivalenceResourceMeterV1,
) -> Result<(), PlironPipelineProtocolFindingV1> {
    if event.kind != Some(kind) {
        return Err(invalid(
            pipeline,
            Some(event.site),
            &format!("expected {kind:?}, found {:?}", event.kind),
        ));
    }
    let epoch_matches = match epoch {
        EpochExpectationV1::Constant(expected) => {
            index_constant(context, event.epoch) == Some(expected)
        }
        EpochExpectationV1::Offset(base, offset) => {
            index_offset(context, event.epoch, base, equivalence_resources) == Some(offset)
        }
    };
    if !epoch_matches {
        return Err(invalid(
            pipeline,
            Some(event.site),
            "event uses the wrong epoch for its pipeline phase",
        ));
    }
    if !slot_is_epoch_modulo(
        context,
        event.slot,
        event.epoch,
        buffers,
        equivalence_resources,
    ) {
        return Err(invalid(
            pipeline,
            Some(event.site),
            &format!("slot is not epoch % {buffers}"),
        ));
    }
    Ok(())
}

#[derive(Clone)]
enum DynamicPipelineActionV1 {
    Event(EventSiteV1),
    Access(AccessSiteV1),
}

impl DynamicPipelineActionV1 {
    const fn site(&self) -> PlironOperationSiteV1 {
        match self {
            Self::Event(event) => event.site,
            Self::Access(access) => access.site,
        }
    }
}

fn verify_dynamic_accesses(
    context: &Context,
    pipeline: PlironOperationSiteV1,
    buffers: u32,
    schedule: &[EventSiteV1],
    accesses: &[AccessSiteV1],
    summary: &CanonicalEpochLoopV1,
    equivalence_resources: &mut EquivalenceResourceMeterV1,
) -> Result<(usize, usize, bool), PlironPipelineProtocolFindingV1> {
    if accesses.is_empty() {
        return Ok((0, 0, true));
    }
    let prologue_start = summary
        .prologue
        .iter()
        .position(|block| *block == pipeline.block())
        .unwrap_or(0);
    let mut ordered_blocks = summary.prologue[prologue_start..].to_vec();
    ordered_blocks.extend(summary.body.iter().copied());
    ordered_blocks.extend(summary.drain.iter().copied());
    let positions = ordered_blocks
        .iter()
        .enumerate()
        .map(|(position, block)| (*block, position))
        .collect::<HashMap<_, _>>();
    let order_key = |site: PlironOperationSiteV1| {
        positions
            .get(&site.block())
            .copied()
            .map(|position| (position, site.operation()))
    };
    let mut actions = Vec::with_capacity(accesses.len() + schedule.len());
    for access in accesses {
        if order_key(access.site).is_none() {
            return Err(invalid(
                pipeline,
                Some(access.site),
                "pipeline-owned storage is accessed outside the canonical prologue, loop body, or drain block",
            ));
        }
        actions.push(DynamicPipelineActionV1::Access(access.clone()));
    }
    for event in schedule {
        actions.push(DynamicPipelineActionV1::Event(*event));
    }
    actions.sort_by_key(|action| {
        order_key(action.site()).unwrap_or((usize::MAX, action.site().operation()))
    });

    let mut staging = None::<(Value, usize)>;
    let mut consuming = None::<(Value, usize)>;
    let mut staging_coordinates = HashSet::<Vec<Value>>::new();
    let mut consuming_coordinates = HashSet::<Vec<Value>>::new();
    let mut canonical_coordinates = None::<HashSet<Vec<Value>>>;
    let mut empty_staging_windows = 0_usize;
    let mut empty_consuming_windows = 0_usize;
    let mut staged_writes = 0;
    let mut consuming_reads = 0;
    for action in actions {
        match action {
            DynamicPipelineActionV1::Event(event) => match event.kind {
                Some(PipelineEventKindAttr::Stage) => {
                    staging = Some((event.epoch, event.site.block()));
                    staging_coordinates.clear();
                }
                Some(PipelineEventKindAttr::Commit) => {
                    if staging_coordinates.is_empty() {
                        empty_staging_windows += 1;
                    } else {
                        require_matching_pipeline_coordinates_v1(
                            context,
                            pipeline,
                            event.site,
                            "staging",
                            &staging_coordinates,
                            &mut canonical_coordinates,
                            equivalence_resources,
                        )?;
                    }
                    staging = None;
                }
                Some(PipelineEventKindAttr::Consume) => {
                    consuming = Some((event.epoch, event.site.block()));
                    consuming_coordinates.clear();
                }
                Some(PipelineEventKindAttr::Release) => {
                    if consuming.is_some() {
                        if consuming_coordinates.is_empty() {
                            empty_consuming_windows += 1;
                        } else {
                            require_matching_pipeline_coordinates_v1(
                                context,
                                pipeline,
                                event.site,
                                "consuming",
                                &consuming_coordinates,
                                &mut canonical_coordinates,
                                equivalence_resources,
                            )?;
                        }
                    }
                    consuming_coordinates.clear();
                    consuming = None;
                }
                Some(PipelineEventKindAttr::Wait | PipelineEventKindAttr::Discard) | None => {}
            },
            DynamicPipelineActionV1::Access(access) => {
                let expected = match access.kind {
                    AccessKindAttr::Write => staging.map(|(epoch, block)| (epoch, block, true)),
                    AccessKindAttr::Read => consuming.map(|(epoch, block)| (epoch, block, false)),
                    _ => None,
                };
                let Some((epoch, epoch_block, is_write)) = expected else {
                    return Err(invalid(
                        pipeline,
                        Some(access.site),
                        &format!(
                            "{:?} access is outside its legal stage-to-commit or consume-to-release window",
                            access.kind
                        ),
                    ));
                };
                if !slot_is_epoch_modulo_across_loop_blocks(
                    context,
                    PipelineBlockValueV1 {
                        value: access.slot,
                        block: access.site.block(),
                    },
                    PipelineBlockValueV1 {
                        value: epoch,
                        block: epoch_block,
                    },
                    buffers,
                    summary,
                    equivalence_resources,
                ) {
                    return Err(invalid(
                        pipeline,
                        Some(access.site),
                        &format!(
                            "{:?} access does not use the live epoch modulo {buffers} as its leading ring index",
                            access.kind
                        ),
                    ));
                }
                if is_write {
                    staging_coordinates.insert(access.indices[1..].to_vec());
                    staged_writes += 1;
                } else {
                    consuming_coordinates.insert(access.indices[1..].to_vec());
                    consuming_reads += 1;
                }
            }
        }
    }
    if consuming_reads != 0 && (empty_staging_windows != 0 || empty_consuming_windows != 0) {
        return Err(invalid(
            pipeline,
            None,
            "a consumed symbolic tile is not initialized in every prologue and steady-state epoch",
        ));
    }
    Ok((staged_writes, consuming_reads, true))
}

fn require_matching_pipeline_coordinates_v1(
    context: &Context,
    pipeline: PlironOperationSiteV1,
    event: PlironOperationSiteV1,
    phase: &str,
    coordinates: &HashSet<Vec<Value>>,
    canonical: &mut Option<HashSet<Vec<Value>>>,
    equivalence_resources: &mut EquivalenceResourceMeterV1,
) -> Result<(), PlironPipelineProtocolFindingV1> {
    debug_assert!(!coordinates.is_empty());
    match canonical {
        None => *canonical = Some(coordinates.clone()),
        Some(expected)
            if coordinate_sets_equivalent_v1(
                context,
                expected,
                coordinates,
                equivalence_resources,
            ) => {}
        Some(_) => {
            return Err(invalid(
                pipeline,
                Some(event),
                &format!(
                    "{phase} epoch coordinates do not match the staged/consumed symbolic tile"
                ),
            ));
        }
    }
    Ok(())
}

fn coordinate_sets_equivalent_v1(
    context: &Context,
    left: &HashSet<Vec<Value>>,
    right: &HashSet<Vec<Value>>,
    equivalence_resources: &mut EquivalenceResourceMeterV1,
) -> bool {
    let mut contains_equivalent = |haystack: &HashSet<Vec<Value>>, needle: &[Value]| {
        haystack.iter().any(|candidate| {
            candidate.len() == needle.len()
                && candidate
                    .iter()
                    .copied()
                    .zip(needle.iter().copied())
                    .all(|(left, right)| {
                        index_values_equivalent(context, left, right, equivalence_resources)
                    })
        })
    };
    left.iter()
        .all(|coordinate| contains_equivalent(right, coordinate))
        && right
            .iter()
            .all(|coordinate| contains_equivalent(left, coordinate))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SlotStateV1 {
    Free,
    Staged(u64),
    Committed(u64),
    Ready(u64),
    Consuming(u64),
    Discarding(u64),
}
