/// Replay-derived, fixed-size publication proof over the entire maximum domain.
/// This is compiler analysis, not evidence about a concrete dispatch or allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankedStaticPublicationProofV1 {
    payload_origin: u64,
    flags_origin: u64,
    sites: [RankedRaceLocationV1; 6],
}

impl RankedStaticPublicationProofV1 {
    pub const fn payload_origin(self) -> u64 { self.payload_origin }
    pub const fn flags_origin(self) -> u64 { self.flags_origin }
    /// Payload store, READY release, REQUEST release, acquire, guard, payload read.
    pub const fn sites(&self) -> &[RankedRaceLocationV1; 6] { &self.sites }
    pub const fn maximum_invocations(self) -> u32 { 256 }
    pub const fn potentially_conflicting_cell_pairs(self) -> u32 { 128 }
    pub const fn discharged_cell_pairs(self) -> u32 { 128 }
    pub const fn unresolved_cell_pairs(self) -> u32 { 0 }
}

fn publication_failure_v1() -> Box<RankedRaceFindingV1> {
    Box::new(RankedRaceFindingV1::HappensBeforeIncomplete {
        view: "static publication".to_owned(),
        detail: "the exact once-only five-effect handshake, distinct allocation roots, same-cell read-from guard, or full 256-invocation role proof could not be rederived".to_owned(),
    })
}

// Initial atomics may contain READY. The consumer's REQUEST is sequenced
// before its acquire, so write-read coherence excludes every modification
// before REQUEST. The only remaining READY writer is this cell's publisher.
// Reading READY therefore synchronizes with its release and orders W before R.
// This proves safety only: clearing a completed publication may lose progress.
fn derive_static_publication_v1(
    context: &Context,
    inventory: &crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1,
    sparse: &SparseIndexAnalysisV1,
    layout: Option<PlironExecutionLayoutV1>,
    effects: &[EffectV1],
) -> Result<Option<RankedStaticPublicationProofV1>, Box<RankedRaceFindingV1>> {
    use dialect_kernel::{PublicationAtomicAccessAttr as Atomic, PublicationReadGuardOp};
    let mut markers = Vec::new();
    let mut guards = Vec::new();
    for site in inventory.operations() {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        if let Some(access) = operation.downcast_ref::<RankedAccessOp>()
            && let Some(kind) = access.publication_atomic_kind(context)
        {
            if markers.len() == 3 { return Err(publication_failure_v1()); }
            markers.push((kind, *site));
        }
        if operation.downcast_ref::<PublicationReadGuardOp>().is_some() {
            if !guards.is_empty() { return Err(publication_failure_v1()); }
            guards.push(*site);
        }
    }
    if markers.is_empty() && guards.is_empty() { return Ok(None); }
    let Some(layout) = layout else { return Err(publication_failure_v1()); };
    if layout.global_extents != [256, 1, 1]
        || layout.workgroup_extents != [128, 1, 1]
        || layout.subgroup_size != 64
        || layout.execution_domain != dialect_gpu::ExecutionDomainAttr::FullPhysicalWorkgroups
        || effects.iter().any(|effect| effect.noalias_class == 0)
        || markers.len() != 3 || guards.len() != 1
    { return Err(publication_failure_v1()); }
    let marker = |kind| {
        let mut selected = markers.iter().filter(|(candidate, _)| *candidate == kind);
        let site = selected.next().map(|(_, site)| *site)?;
        selected.next().is_none().then_some(site)
    };
    let ready = marker(Atomic::ReleaseReadyU32).ok_or_else(publication_failure_v1)?;
    let request = marker(Atomic::ReleaseRequestU32).ok_or_else(publication_failure_v1)?;
    let acquire = marker(Atomic::AcquireU32).ok_or_else(publication_failure_v1)?;
    let guard_site = guards[0];
    let ready_op = Operation::get_op::<RankedAccessOp>(ready.pointer(), context)
        .ok_or_else(publication_failure_v1)?;
    let request_op = Operation::get_op::<RankedAccessOp>(request.pointer(), context)
        .ok_or_else(publication_failure_v1)?;
    let acquire_op = Operation::get_op::<RankedAccessOp>(acquire.pointer(), context)
        .ok_or_else(publication_failure_v1)?;
    let guard = Operation::get_op::<PublicationReadGuardOp>(guard_site.pointer(), context)
        .ok_or_else(publication_failure_v1)?;
    let flags = ready_op.view(context);
    if request_op.view(context) != flags || acquire_op.view(context) != flags
        || request_op.indices(context) != acquire_op.indices(context)
        || acquire_op.indices(context) != [guard.index(context)]
        || acquire_op.publication_read_result(context) != Some(guard.acquired(context))
        || request.block() != acquire.block() || request.block() != guard_site.block()
        || request.operation() >= acquire.operation() || acquire.operation() >= guard_site.operation()
    { return Err(publication_failure_v1()); }
    let location = |site: crate::production_analysis::pliron_function_inventory::PlironOperationSiteV1| {
        RankedRaceLocationV1 { block: site.block(), operation: site.operation() }
    };
    let mut reads = effects.iter().filter(|effect| effect.checked_success == Some(guard.success(context)));
    let read = reads.next().ok_or_else(publication_failure_v1)?;
    if reads.next().is_some() || read.kind != AccessKindAttr::Read
        || read.indices != [guard.result(context)]
        || read.location.block != guard_site.block() || read.location.operation <= guard_site.operation()
    { return Err(publication_failure_v1()); }
    let EffectIdentityV1::View(payload) = read.identity else { return Err(publication_failure_v1()); };
    let mut payload_effects = effects.iter().filter(|effect| effect.noalias_class == read.noalias_class);
    let mut write = None;
    let mut payload_count = 0;
    for effect in &mut payload_effects {
        payload_count += 1;
        if effect.kind == AccessKindAttr::Write && effect.identity == read.identity {
            if write.replace(effect).is_some() { return Err(publication_failure_v1()); }
        } else if effect.location != read.location { return Err(publication_failure_v1()); }
    }
    let write = write.ok_or_else(publication_failure_v1)?;
    if payload_count != 2 || write.location.block != ready.block()
        || write.location.operation >= ready.operation() || write.indices != ready_op.indices(context)
        || write.checked_success.is_some() || write.location.block == read.location.block
    { return Err(publication_failure_v1()); }
    let flag_effect = effects.iter().find(|effect| effect.location == location(ready))
        .ok_or_else(publication_failure_v1)?;
    if flag_effect.noalias_class == read.noalias_class || read.noalias_class == 0
        || flag_effect.noalias_class == 0
    { return Err(publication_failure_v1()); }
    let flag_sites = [location(ready), location(request), location(acquire)];
    let mut flag_count = 0;
    for effect in effects.iter().filter(|effect| effect.noalias_class == flag_effect.noalias_class) {
        flag_count += 1;
        if effect.identity != EffectIdentityV1::View(flags) || !flag_sites.contains(&effect.location)
            || effect.atomic_scope != Some(AtomicScopeAttr::System)
        { return Err(publication_failure_v1()); }
    }
    if flag_count != 3 { return Err(publication_failure_v1()); }
    let origin = |view: Value| -> Option<u64> {
        let operation = Operation::get_op::<RankedViewOp>(view.defining_op()?, context)?;
        let origin = operation.allocation_origin(context)?;
        (origin != 0 && operation.memory_space(context) == Some(MemorySpaceAttr::Global)).then_some(origin)
    };
    let payload_origin = origin(payload).ok_or_else(publication_failure_v1)?;
    let flags_origin = origin(flags).ok_or_else(publication_failure_v1)?;
    if payload_origin == flags_origin { return Err(publication_failure_v1()); }
    prove_publication_roles_v1(context, inventory, sparse,
        write.location.block, read.location.block, write.indices[0], guard.index(context))?;
    Ok(Some(RankedStaticPublicationProofV1 {
        payload_origin, flags_origin,
        sites: [write.location, location(ready), location(request), location(acquire), location(guard_site), read.location],
    }))
}

fn prove_publication_roles_v1(
    context: &Context,
    inventory: &crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1,
    sparse: &SparseIndexAnalysisV1,
    producer: usize,
    consumer: usize,
    producer_index: Value,
    consumer_index: Value,
) -> Result<(), Box<RankedRaceFindingV1>> {
    use dialect_kernel::{AnalysisSplitOp, BranchArgsOp, BranchOp, ReturnOp, TrapOp};
    let blocks = inventory.blocks();
    if blocks.is_empty() || blocks.len() > 1024 || inventory.operations().len() > 8192 {
        return Err(publication_failure_v1());
    }
    let indices = blocks.iter().copied().enumerate().map(|(index, block)| (block, index))
        .collect::<HashMap<_, _>>();
    let mut successors = Vec::with_capacity(blocks.len());
    let mut indegrees = vec![0_usize; blocks.len()];
    for block in 0..blocks.len() {
        let site = inventory.block_operations(block).last().ok_or_else(publication_failure_v1)?;
        let operation = site.pointer().deref(context);
        let mut targets = Vec::new();
        for successor in operation.successors() {
            if targets.len() == 2 { return Err(publication_failure_v1()); }
            let target = *indices.get(&successor).ok_or_else(publication_failure_v1)?;
            indegrees[target] += 1;
            targets.push(target);
        }
        successors.push(targets);
    }
    // A topological order proves once-only execution, including unknown edges.
    // Reject cycles anywhere rather than assuming they are unreachable.
    let mut queue = (0..blocks.len()).filter(|index| indegrees[*index] == 0).collect::<VecDeque<_>>();
    let mut order = Vec::with_capacity(blocks.len());
    while let Some(block) = queue.pop_front() {
        order.push(block);
        for target in &successors[block] {
            indegrees[*target] -= 1;
            if indegrees[*target] == 0 { queue.push_back(*target); }
        }
    }
    if order.len() != blocks.len() { return Err(publication_failure_v1()); }
    let mut raw_steps = 0;
    let mut reachable = vec![false; blocks.len()];
    for global in 0..256_u64 {
        let invocation = [global, 0, 0];
        reachable.fill(false);
        reachable[0] = true;
        for block in order.iter().copied() {
            if !reachable[block] { continue; }
            let site = inventory.block_operations(block).last().ok_or_else(publication_failure_v1)?;
            let operation = Operation::get_op_dyn(site.pointer(), context);
            let mut evaluate = |value| sparse.fact(value).evaluate(&invocation).or_else(|| {
                evaluate_raw_index_at_invocation_v1(context, value, &invocation, &mut raw_steps)
            });
            let decision = if let Some(branch) = operation.downcast_ref::<IndexLessThanBranchOp>() {
                evaluate(branch.lhs(context)).zip(evaluate(branch.rhs(context))).map(|(a,b)| usize::from(a >= b))
            } else if let Some(branch) = operation.downcast_ref::<IndexLessThanBranchArgsOp>() {
                evaluate(branch.lhs(context)).zip(evaluate(branch.rhs(context))).map(|(a,b)| usize::from(a >= b))
            } else if let Some(branch) = operation.downcast_ref::<IndexEqualBranchOp>() {
                evaluate(branch.lhs(context)).zip(evaluate(branch.rhs(context))).map(|(a,b)| usize::from(a != b))
            } else if let Some(branch) = operation.downcast_ref::<IndexEqualBranchArgsOp>() {
                evaluate(branch.lhs(context)).zip(evaluate(branch.rhs(context))).map(|(a,b)| usize::from(a != b))
            } else if operation.downcast_ref::<BranchOp>().is_some() || operation.downcast_ref::<BranchArgsOp>().is_some() {
                if successors[block].len() != 1 { return Err(publication_failure_v1()); }
                Some(0)
            } else if operation.downcast_ref::<AnalysisSplitOp>().is_some() {
                if successors[block].len() != 2 { return Err(publication_failure_v1()); }
                // Dependencies are not Boolean facts. Both paths remain
                // possible, including post-publication return/trap splits.
                None
            } else if operation.downcast_ref::<ReturnOp>().is_some() || operation.downcast_ref::<TrapOp>().is_some() {
                if !successors[block].is_empty() { return Err(publication_failure_v1()); }
                None
            } else {
                return Err(publication_failure_v1());
            };
            if raw_steps > MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 { return Err(publication_failure_v1()); }
            if let Some(selected) = decision {
                let target = successors[block].get(selected).ok_or_else(publication_failure_v1)?;
                reachable[*target] = true;
            } else {
                // Unknown bounds or actual launch values only add possible paths.
                for target in &successors[block] { reachable[*target] = true; }
            }
        }
        if reachable[producer] != (global < 128) || reachable[consumer] != (global >= 128) {
            return Err(publication_failure_v1());
        }
        let value = if global < 128 { producer_index } else { consumer_index };
        let cell = sparse.fact(value).evaluate(&invocation).or_else(|| {
            evaluate_raw_index_at_invocation_v1(context, value, &invocation, &mut raw_steps)
        });
        if cell != Some(global % 128) { return Err(publication_failure_v1()); }
    }
    Ok(())
}

impl RankedRaceReportV1 {
    /// Exact proof retained separately from unresolved findings, never hidden.
    pub fn static_publication(&self) -> Option<&RankedStaticPublicationProofV1> {
        self.static_publication.first()
    }

    pub(super) fn validation_payload_receipt_v1(
        &self,
        limits: ProductionAnalysisResourceLimitsV1,
    ) -> Result<super::pliron_report_payload_receipt::ProductionAnalysisReportPayloadReceiptV1, ProductionAnalysisResourceLimitV1> {
        use super::pliron_report_payload_receipt::{ProductionAnalysisReportPayloadReceiptV1 as Receipt, require_payload_census_v1};
        require_payload_census_v1(2, limits)?;
        Ok(if self.findings.is_empty() && self.findings.capacity() == 0
            && self.static_publication.capacity() <= 1 {
            let items = self.static_publication.capacity() * 32;
            Receipt::exact(self.pass(), 2, items, items, 3 + items, 1 + items, 4)
        } else { Receipt::fallback(self.pass(), 2) })
    }

    pub(super) fn try_clone_validation_payload_v1(&self) -> Result<Self, ProductionAnalysisResourceLimitV1> {
        if !self.findings.is_empty() || self.findings.capacity() != 0 || self.static_publication.len() > 1 {
            return Err(super::pliron_report_payload_receipt::payload_limit_v1("report payload shape changed"));
        }
        let mut static_publication = Vec::new();
        if let Some(proof) = self.static_publication.first() {
            super::pliron_report_payload_receipt::try_reserve_payload_v1(&mut static_publication, 1)?;
            static_publication.push(*proof);
        }
        Ok(Self { findings: Vec::new(), static_publication })
    }
}
