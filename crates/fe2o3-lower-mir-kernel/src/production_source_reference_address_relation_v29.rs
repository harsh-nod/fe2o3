// Original source recipes are re-read here. Archives and operation captures are
// locators only; neither can replace the immutable recipe or the actual graph.
include!("production_source_address_emitted_index_v29.rs");
include!("production_source_issued_pointer_v29.rs");
include!("production_source_issued_pointer_actual_v29.rs");
include!("production_source_issued_pointer_accesses_v29.rs");
include!("production_source_scalar_loan_access_v29.rs");
struct SourceAddressStatementV29<'source> {
    instance: usize,
    block: u32,
    statement: u32,
    span: &'source SemanticKirStatementOperationSpanV1,
}

impl SourceAddressStatementV29<'_> {
    fn key(&self) -> (usize, u32, u32) {
        (self.instance, self.block, self.statement)
    }
}

struct SourceAddressTerminatorV29<'source> {
    instance: usize,
    block: u32,
    span: &'source SemanticKirTerminatorOperationSpanV1,
}

impl SourceAddressTerminatorV29<'_> {
    fn key(&self) -> (usize, u32) {
        (self.instance, self.block)
    }
}

// Sidecars can be compact, but original call-instance IDs never change. This
// metadata index is not a graph or an independent source-admission authority.
struct SourceAddressSourceIndexV29<'source> {
    owner: &'source ProductionSemanticSsaOwnerV1,
    pending: &'source PendingScopedRootEmissionV29,
    statements: Vec<SourceAddressStatementV29<'source>>,
    terminators: Vec<SourceAddressTerminatorV29<'source>>,
    emitted: SourceAddressEmittedIndexV29<'source>,
    storage: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    required: usize,
}

impl<'source> SourceAddressSourceIndexV29<'source> {
    fn new<'owner: 'source>(
        instances: &ExecutionInstancesV29<'owner>,
        pending: &'source PendingScopedRootEmissionV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let root = instances
            .instance(instances.root())
            .ok_or_else(source_raw_physical_error_v29)?
            .function();
        budget.charge_work(5)?;
        if pending.coordinates.root != root
            || pending.coordinates.semantic_sha256 != *instances.owner().source_semantic_sha256()
            || pending.coordinates.ssa != instances.owner().identity()
            || pending.coordinates.sources.rows.len() != instances.instances().len()
        {
            return Err(source_raw_physical_error_v29());
        }
        let count = instances.instances().len();
        let mut expected = 0;
        let mut expected_blocks = 0;
        for index in 0..count {
            budget.charge_work(2)?;
            let instance = instances
                .id_at(index)
                .ok_or_else(source_raw_physical_error_v29)?;
            let retained = pending.active_instances.sidecar_ordinal(
                index,
                count,
                &pending.sidecars.rows,
                budget,
            )?;
            if instances.instance_reachable(instance) != Some(retained.is_some()) {
                return Err(source_raw_physical_error_v29());
            }
            match instances.instance_reachable(instance) {
                Some(false) => continue,
                Some(true) => {}
                None => return Err(source_raw_physical_error_v29()),
            }
            let original = instances
                .instance(instance)
                .ok_or_else(source_raw_physical_error_v29)?;
            for (block, declaration) in original.declaration().blocks().iter().enumerate() {
                budget.charge_work(2)?;
                let block = SemanticBlockIdV1::from_index(
                    u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                );
                match instances.block_reachable(instance, block) {
                    Some(false) => {}
                    Some(true) => {
                        expected = argument_sum_v1(&[expected, declaration.statements().len()])?;
                        expected_blocks = argument_sum_v1(&[expected_blocks, 1])?;
                    }
                    None => return Err(source_raw_physical_error_v29()),
                }
            }
        }
        budget.reserve_storage(std::mem::size_of::<Self>())?;
        let mut statements = emission_vec_v1(expected, budget)?;
        let mut terminators = emission_vec_v1(expected_blocks, budget)?;
        for (ordinal, sidecar) in pending.sidecars.rows.iter().enumerate() {
            budget.charge_work(4)?;
            let instance = sidecar
                .source_call_instance
                .ok_or_else(source_raw_physical_error_v29)?;
            if instances.id_at(instance.index()) != Some(instance)
                || instances.instance_reachable(instance) != Some(true)
            {
                return Err(source_raw_physical_error_v29());
            }
            if pending.active_instances.sidecar_ordinal(
                instance.index(),
                count,
                &pending.sidecars.rows,
                budget,
            )? != Some(ordinal)
            {
                return Err(source_raw_physical_error_v29());
            }
            let original = instances
                .instance(instance)
                .ok_or_else(source_raw_physical_error_v29)?;
            let anchors = sidecar
                .scoped_memory_anchors
                .as_ref()
                .ok_or_else(source_raw_physical_error_v29)?;
            if anchors.subject.ledger != budget.work_ledger_identity_v1() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            budget.charge_work(5)?;
            if anchors.subject.instance != instance
                || anchors.subject.function != original.function()
                || anchors.subject.source
                    != ExecutionCallSourceV29::from_instances(instances, budget)?
            {
                return Err(source_raw_physical_error_v29());
            }
            for span in &sidecar.statement_operation_spans {
                budget.charge_work(7)?;
                if span.correspondence_owner != root
                    || span.semantic_function != original.function()
                    || span.kernel_ir_block
                        != anchors.placement.block(span.semantic_block.index())?
                    || instances.block_reachable(instance, span.semantic_block) != Some(true)
                    || original
                        .declaration()
                        .blocks()
                        .get(span.semantic_block.index() as usize)
                        .and_then(|block| block.statements().get(span.statement_ordinal as usize))
                        .is_none()
                    || statements.len() >= expected
                    || statements.len() == statements.capacity()
                {
                    return Err(source_raw_physical_error_v29());
                }
                statements.push(SourceAddressStatementV29 {
                    instance: instance.index(),
                    block: span.semantic_block.index(),
                    statement: span.statement_ordinal,
                    span,
                });
            }
            for span in &sidecar.terminator_operation_spans {
                budget.charge_work(6)?;
                if span.correspondence_owner != root
                    || span.semantic_function != original.function()
                    || span.kernel_ir_block
                        != anchors.placement.block(span.semantic_block.index())?
                    || instances.block_reachable(instance, span.semantic_block) != Some(true)
                    || terminators.len() >= expected_blocks
                    || terminators.len() == terminators.capacity()
                {
                    return Err(source_raw_physical_error_v29());
                }
                terminators.push(SourceAddressTerminatorV29 {
                    instance: instance.index(),
                    block: span.semantic_block.index(),
                    span,
                });
            }
        }
        if statements.len() != expected || terminators.len() != expected_blocks {
            return Err(source_raw_physical_error_v29());
        }
        call_splice_sort_work_v1(argument_product_v1(statements.len(), 3)?, budget)
            .map_err(source_address_call_error_v29)?;
        statements.sort_unstable_by_key(SourceAddressStatementV29::key);
        for pair in statements.windows(2) {
            budget.charge_work(6)?;
            if pair[0].key() >= pair[1].key() {
                return Err(source_raw_physical_error_v29());
            }
        }
        call_splice_sort_work_v1(argument_product_v1(terminators.len(), 2)?, budget)
            .map_err(source_address_call_error_v29)?;
        terminators.sort_unstable_by_key(SourceAddressTerminatorV29::key);
        for pair in terminators.windows(2) {
            budget.charge_work(4)?;
            if pair[0].key() >= pair[1].key() {
                return Err(source_raw_physical_error_v29());
            }
        }
        let emitted = SourceAddressEmittedIndexV29::new(pending, budget)?;
        // Unique, valid original keys plus the independently counted complete
        // reachable domain establish coverage without an original-ID rewrite.
        let storage = argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            argument_product_v1(
                statements.capacity(),
                std::mem::size_of::<SourceAddressStatementV29<'_>>(),
            )?,
            argument_product_v1(
                terminators.capacity(),
                std::mem::size_of::<SourceAddressTerminatorV29<'_>>(),
            )?,
            emitted.storage,
        ])?;
        Ok(Self {
            owner: instances.owner(),
            pending,
            statements,
            terminators,
            emitted,
            storage,
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            required: budget.storage(),
        })
    }

    fn sidecar(
        &self,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'source PendingInstanceSidecarsV29, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let ordinal = self
            .pending
            .active_instances
            .sidecar_ordinal(
                instance.index(),
                self.pending.coordinates.sources.rows.len(),
                &self.pending.sidecars.rows,
                budget,
            )?
            .ok_or_else(source_raw_physical_error_v29)?;
        let row = self
            .pending
            .sidecars
            .rows
            .get(ordinal)
            .ok_or_else(source_raw_physical_error_v29)?;
        if row.source_call_instance != Some(instance) {
            return Err(source_raw_physical_error_v29());
        }
        Ok(row)
    }

    fn statement(
        &self,
        site: SourceReferenceSiteV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'source SemanticKirStatementOperationSpanV1, ProductionSemanticKirErrorV1> {
        let statement = u32::try_from(site.statement.ok_or_else(source_raw_physical_error_v29)?)
            .map_err(|_| ArgumentResourceV1::Arithmetic)?;
        budget.charge_work(argument_product_v1(
            call_splice_search_work_v1(self.statements.len()),
            3,
        )?)?;
        let ordinal = self
            .statements
            .binary_search_by_key(
                &(site.instance.index(), site.block.index(), statement),
                SourceAddressStatementV29::key,
            )
            .map_err(|_| source_raw_physical_error_v29())?;
        Ok(self.statements[ordinal].span)
    }

    fn discard(
        self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
            || self.required < self.storage
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let storage = self.storage;
        drop(self);
        budget.release_storage(storage)?;
        Ok(())
    }

    fn frame_gap(
        &self,
        instance: ProductionCallInstanceIdV1,
        frame: ScopedMemoryFrameV29,
        block: BlockId,
        gap: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let (original_block, statement) = scoped_memory_site_key_v29(frame.site);
        let (expected_block, first, count) = match statement {
            Some(statement) => {
                let span = self.statement(
                    SourceReferenceSiteV29 {
                        instance,
                        block: SemanticBlockIdV1::from_index(original_block),
                        statement: Some(statement as usize),
                    },
                    budget,
                )?;
                (
                    span.kernel_ir_block,
                    span.first_operation_ordinal,
                    span.operation_count,
                )
            }
            None => {
                budget.charge_work(argument_product_v1(
                    call_splice_search_work_v1(self.terminators.len()),
                    2,
                )?)?;
                let index = self
                    .terminators
                    .binary_search_by_key(
                        &(instance.index(), original_block),
                        SourceAddressTerminatorV29::key,
                    )
                    .map_err(|_| source_raw_physical_error_v29())?;
                let span = self.terminators[index].span;
                (
                    span.kernel_ir_block,
                    span.first_operation_ordinal,
                    span.operation_count,
                )
            }
        };
        budget.charge_work(3)?;
        if expected_block != block
            || gap < first as usize
            || gap > argument_sum_v1(&[first as usize, count as usize])?
        {
            return Err(source_raw_physical_error_v29());
        }
        Ok(())
    }
}

fn source_address_point_error_v29(error: ScopedTileFailureKindV29) -> ProductionSemanticKirErrorV1 {
    match error {
        ScopedTileFailureKindV29::Resource(error) => error.into(),
        _ => source_raw_physical_error_v29(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAddressBoundaryV29 {
    instance: ProductionCallInstanceIdV1,
    anchor: usize,
    event: usize,
    frame: ScopedMemoryFrameV29,
    slot: usize,
    block: BlockId,
    gap: usize,
    cause: ScopedMemoryKillV29,
}

impl SourceAddressBoundaryV29 {
    // Value moves/deinitialization do not end the allocation's lifetime. In
    // contrast, each dynamic StorageLive restarts it even at a repeated site.
    fn activation(self) -> Option<bool> {
        match self.cause {
            ScopedMemoryKillV29::StorageLive => Some(true),
            ScopedMemoryKillV29::StorageDead => Some(false),
            ScopedMemoryKillV29::Deinitialize
            | ScopedMemoryKillV29::Move
            | ScopedMemoryKillV29::ProjectedArrayMove => None,
        }
    }
}

// Original occurrences, not the number of emitted claims, bound the scratch.
// The all-claim join below rejects duplicate, missing and control-inactive rows.
fn source_address_boundaries_v29(
    instances: &ExecutionInstancesV29<'_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    graph: &SourceAddressMemoryV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<SourceAddressBoundaryV29>, ProductionSemanticKirErrorV1> {
    let source = ExecutionCallSourceV29::from_instances(instances, budget)?;
    if slots.ledger != budget.work_ledger_identity_v1() || slots.source != source {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let mut capacity = 0;
    for index in 0..instances.instances().len() {
        budget.charge_work(2)?;
        let instance = instances
            .id_at(index)
            .ok_or_else(source_raw_physical_error_v29)?;
        match instances.instance_reachable(instance) {
            Some(false) => continue,
            Some(true) => {}
            None => return Err(source_raw_physical_error_v29()),
        }
        budget.charge_work(call_splice_search_work_v1(slots.instances.len()))?;
        let owner = slots
            .instances
            .binary_search_by_key(&instance.index(), |row| row.instance.index())
            .ok()
            .and_then(|index| slots.instances.get(index))
            .ok_or_else(source_raw_physical_error_v29)?;
        let local_slots = slots
            .slots
            .get(owner.slots.clone())
            .ok_or_else(source_raw_physical_error_v29)?;
        let anchors = source_index
            .sidecar(instance, budget)?
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?;
        // Count only recorded boundaries and their generation-specific rows.
        // The second pass authenticates each boundary against original events.
        for row in &anchors.rows {
            budget.charge_work(1)?;
            if let ScopedMemoryAnchorKindV29::Kill { local, .. } = row.kind {
                let (legacy, objects) =
                    source_address_local_slot_ranges_v29(local_slots, local, budget)?;
                capacity =
                    argument_sum_v1(&[capacity, usize::from(legacy.is_some()), objects.len()])?;
            }
        }
    }
    let mut boundaries = emission_vec_v1(capacity, budget)?;
    for index in 0..instances.instances().len() {
        budget.charge_work(2)?;
        let instance = instances
            .id_at(index)
            .ok_or_else(source_raw_physical_error_v29)?;
        if instances.instance_reachable(instance) == Some(false) {
            continue;
        }
        let original = instances
            .instance(instance)
            .ok_or_else(source_raw_physical_error_v29)?;
        let sidecar = source_index.sidecar(instance, budget)?;
        let anchors = sidecar
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?;
        if anchors.subject.source != source
            || anchors.subject.instance != instance
            || anchors.subject.function != original.function()
        {
            return Err(source_raw_physical_error_v29());
        }
        if anchors.subject.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(call_splice_search_work_v1(slots.instances.len()))?;
        let slot_owner = slots
            .instances
            .binary_search_by_key(&instance.index(), |row| row.instance.index())
            .ok()
            .and_then(|ordinal| slots.instances.get(ordinal))
            .ok_or_else(source_raw_physical_error_v29)?;
        if slot_owner.function != original.function() || slot_owner.placement != anchors.placement {
            return Err(source_raw_physical_error_v29());
        }
        let local_slots = slots
            .slots
            .get(slot_owner.slots.clone())
            .ok_or_else(source_raw_physical_error_v29)?;
        let occurrences = instances
            .occurrences(instance)
            .ok_or_else(source_raw_physical_error_v29)?;
        let mut expected = emission_vec_v1(occurrences.events().len(), budget)?;
        for occurrence in occurrences.events() {
            budget.charge_work(3)?;
            let block =
                SemanticBlockIdV1::from_index(scoped_memory_site_key_v29(occurrence.site()).0);
            if !occurrence.is_reachable()
                || instances.block_reachable(instance, block) == Some(false)
            {
                expected.push(None);
                continue;
            }
            if instances.block_reachable(instance, block) != Some(true) {
                return Err(source_raw_physical_error_v29());
            }
            let local = occurrence.event().variable().get();
            let (legacy, objects) =
                source_address_local_slot_ranges_v29(local_slots, local, budget)?;
            let slot = legacy.or_else(|| (!objects.is_empty()).then_some(objects.start));
            let needed = if let Some(slot) = slot {
                let declaration = instances
                    .owner()
                    .source_semantic()
                    .types()
                    .get(local_slots[slot].origin.semantic_type.index() as usize)
                    .ok_or_else(source_raw_physical_error_v29)?;
                scoped_expected_kill_v29(
                    original.declaration(),
                    occurrence,
                    legacy.is_some()
                        && matches!(declaration.shape(), SemanticTypeShapeV1::Array { .. }),
                )
            } else {
                None
            };
            expected.push(needed);
        }
        for (anchor, row) in anchors.rows.iter().enumerate() {
            budget.charge_work(2)?;
            let ScopedMemoryAnchorKindV29::Kill {
                event,
                local,
                cause,
            } = row.kind
            else {
                continue;
            };
            let frame = row.source.ok_or_else(source_raw_physical_error_v29)?;
            let occurrence = occurrences
                .events()
                .get(event)
                .ok_or_else(source_raw_physical_error_v29)?;
            let (original_local, original_cause) = expected
                .get_mut(event)
                .and_then(Option::take)
                .ok_or_else(source_raw_physical_error_v29)?;
            if original_local != local
                || original_cause != cause
                || occurrence.site() != frame.site
                || frame.role != Some(ScopedMemoryRoleV29::Operand(occurrence.operand()))
                || occurrence.role() != cause.event_role()
            {
                return Err(source_raw_physical_error_v29());
            }
            source_index.frame_gap(instance, frame, row.block, row.position, budget)?;
            let mut mapping = ScopedEmittedPointsV29 {
                coordinates: &source_index.pending.coordinates,
                relocation: &source_index.pending.slot_relocation,
                budget,
            };
            let (block, gap) = mapping
                .emitted_point(
                    instance,
                    row.block,
                    u32::try_from(row.position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    true,
                )
                .map_err(source_address_point_error_v29)?
                .ok_or_else(source_raw_physical_error_v29)?;
            if gap as usize > graph.blocks[graph.block(block, budget)?].1.operations.len()
                || boundaries.len() == boundaries.capacity()
            {
                return Err(source_raw_physical_error_v29());
            }
            let (legacy, objects) =
                source_address_local_slot_ranges_v29(local_slots, local, budget)?;
            for slot in legacy.into_iter().chain(objects) {
                budget.charge_work(2)?;
                if boundaries.len() == boundaries.capacity() {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                boundaries.push(SourceAddressBoundaryV29 {
                    instance,
                    anchor,
                    event,
                    frame,
                    slot: argument_sum_v1(&[slot_owner.slots.start, slot])?,
                    block,
                    gap: gap as usize,
                    cause,
                });
            }
        }
        budget.charge_work(expected.len())?;
        if expected.iter().any(Option::is_some) {
            return Err(source_raw_physical_error_v29());
        }
        let scratch = argument_product_v1(
            expected.capacity(),
            std::mem::size_of::<Option<(u32, ScopedMemoryKillV29)>>(),
        )?;
        drop(expected);
        budget.release_storage(scratch)?;
    }
    call_splice_sort_work_v1(argument_product_v1(boundaries.len(), 5)?, budget)
        .map_err(source_address_call_error_v29)?;
    boundaries.sort_unstable_by_key(|row| {
        (
            row.block,
            row.gap,
            row.instance.index(),
            row.anchor,
            row.slot,
        )
    });
    Ok(boundaries)
}

fn source_address_original_operation_v29<'a>(
    pending: &'a PendingScopedRootEmissionV29,
    graph: &SourceAddressMemoryV29<'a>,
    instance: ProductionCallInstanceIdV1,
    block: BlockId,
    position: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<&'a Operation, ProductionSemanticKirErrorV1> {
    let mut mapping = ScopedEmittedPointsV29 {
        coordinates: &pending.coordinates,
        relocation: &pending.slot_relocation,
        budget,
    };
    let (block, position) = mapping
        .emitted_point(
            instance,
            block,
            u32::try_from(position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            false,
        )
        .map_err(source_address_point_error_v29)?
        .ok_or_else(source_raw_physical_error_v29)?;
    graph.blocks[graph.block(block, budget)?]
        .1
        .operations
        .get(position as usize)
        .ok_or_else(source_raw_physical_error_v29)
}

struct SourceAddressLifetimesV29 {
    initial: Vec<bool>,
    lifetimes: Vec<SourceAddressLifetimeV29>,
    kills: Vec<SourceAddressKillV29>,
}

fn source_address_invocation_entry_v29(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    sidecar: &PendingInstanceSidecarsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<BlockId, ProductionSemanticKirErrorV1> {
    let original = instances
        .instance(instance)
        .ok_or_else(source_raw_physical_error_v29)?;
    let anchors = sidecar
        .scoped_memory_anchors
        .as_ref()
        .ok_or_else(source_raw_physical_error_v29)?;
    budget.charge_work(sidecar.synthetic_operation_spans.len())?;
    let has_trap = sidecar
        .synthetic_operation_spans
        .iter()
        .any(|span| span.rule == SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap);
    with_invocation_entry_plan_v1(
        original.declaration(),
        original.ssa(),
        anchors.placement,
        has_trap,
        budget,
        |plan, budget| {
            budget.charge_work(8)?;
            match (plan.layout.preheader, &sidecar.invocation_entry) {
                (None, None) => Ok(plan.layout.source_entry),
                (Some(preheader), Some(relation))
                    if relation.layout == plan.layout
                        && relation.subject == Some(anchors.subject)
                        && relation.span.version == INVOCATION_ENTRY_RELATION_VERSION_V1
                        && relation.span.correspondence_owner == anchors.subject.source.root
                        && relation.span.semantic_function == original.function()
                        && relation.span.kernel_ir_block == preheader =>
                {
                    Ok(preheader)
                }
                _ => Err(source_raw_physical_error_v29()),
            }
        },
    )
}

fn source_address_lifetimes_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    graph: &SourceAddressMemoryV29<'_>,
    boundaries: &[SourceAddressBoundaryV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceAddressLifetimesV29, ProductionSemanticKirErrorV1> {
    plan.check_owner(instances, budget)?;
    let pending = source_index.pending;
    let count = instances.instances().len();
    let mut initial = emission_vec_v1(slots.slots.len(), budget)?;
    budget.charge_work(slots.slots.len())?;
    initial.extend(slots.slots.iter().map(|slot| {
        slot.instance == instances.root()
            && match slot.origin.identity {
                ScopedAllocationIdentityV29::OriginalObject { generation, .. } => generation == 0,
                _ => true,
            }
    }));
    let mut ranges = emission_vec_v1(count, budget)?;
    let mut entries = emission_vec_v1(count, budget)?;
    let mut preheaders = emission_vec_v1(count, budget)?;
    let mut incoming = emission_vec_v1(graph.blocks.len(), budget)?;
    budget.charge_work(argument_sum_v1(&[
        argument_product_v1(count, 3)?,
        graph.blocks.len(),
    ])?)?;
    ranges.resize(count, None);
    entries.resize(count, None);
    preheaders.resize(count, None);
    incoming.resize(graph.blocks.len(), 0usize);
    let mut capacity = boundaries.len();
    for row in &slots.instances {
        budget.charge_work(3)?;
        let target = ranges
            .get_mut(row.instance.index())
            .ok_or_else(source_raw_physical_error_v29)?;
        if target.replace(row.slots.clone()).is_some()
            || instances.instance_reachable(row.instance) != Some(true)
            || row.slots.end > slots.slots.len()
            || row.slots.start > row.slots.end
        {
            return Err(source_raw_physical_error_v29());
        }
        let function = instances
            .instance(row.instance)
            .ok_or_else(source_raw_physical_error_v29)?
            .declaration();
        let mut exits = 0;
        for (block, source) in function.blocks().iter().enumerate() {
            budget.charge_work(2)?;
            let block = SemanticBlockIdV1::from_index(
                u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            if instances.block_reachable(row.instance, block) == Some(true)
                && matches!(source.terminator().kind(), SemanticTerminatorKindV1::Return)
            {
                exits = argument_sum_v1(&[exits, 1])?;
            }
        }
        capacity = argument_sum_v1(&[
            capacity,
            argument_product_v1(
                row.slots.len(),
                argument_sum_v1(&[exits, usize::from(row.instance != instances.root())])?,
            )?,
        ])?;
    }
    let mut lifetimes = emission_vec_v1(capacity, budget)?;
    let mut kills = emission_vec_v1(capacity, budget)?;
    let return_sequence = argument_product_v1(capacity, 2)?;
    let mut expected_returns = emission_vec_v1(source_index.terminators.len(), budget)?;
    for row in &source_index.terminators {
        budget.charge_work(2)?;
        let instance = instances
            .id_at(row.instance)
            .ok_or_else(source_raw_physical_error_v29)?;
        let source = instances
            .instance(instance)
            .ok_or_else(source_raw_physical_error_v29)?;
        if matches!(
            source.declaration().blocks()[row.block as usize]
                .terminator()
                .kind(),
            SemanticTerminatorKindV1::Return
        ) {
            expected_returns.push((instance, SemanticBlockIdV1::from_index(row.block), false));
        }
    }
    // Count edges once; a synthetic call preheader must have only its real
    // call edge, never a source backedge or a forged bypass predecessor.
    for (_, block) in &graph.blocks {
        budget.charge_work(1)?;
        block
            .terminator
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?
            .try_visit_edges_v1(|target, arguments| {
                budget.charge_work(argument_sum_v1(&[1, arguments.len()])?)?;
                let index = graph.block(target, budget)?;
                incoming[index] = argument_sum_v1(&[incoming[index], 1])?;
                Ok::<_, ProductionSemanticKirErrorV1>(())
            })?;
    }
    for control in &pending.coordinates.controls.rows {
        budget.charge_work(4)?;
        if instances.instance_reachable(control.instance) != Some(true) {
            return Err(source_raw_physical_error_v29());
        }
        let block = graph.blocks[graph.block(control.physical_block, budget)?].1;
        let terminator = block
            .terminator
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?;
        if let Some((target, arguments)) = &control.expected_branch {
            let expected = pending
                .coordinates
                .values
                .rows
                .get(arguments.clone())
                .ok_or_else(source_raw_physical_error_v29)?;
            let Terminator::Branch {
                target: actual_target,
                arguments: actual,
            } = terminator
            else {
                return Err(source_raw_physical_error_v29());
            };
            budget.charge_work(argument_sum_v1(&[1, expected.len()])?)?;
            if actual_target != target || actual.as_slice() != expected {
                return Err(source_raw_physical_error_v29());
            }
        }
        match control.origin {
            InstanceControlOriginV1::CallEntry { call, child } => {
                if instances
                    .incoming(child)
                    .map(|incoming| incoming.occurrence())
                    != Some(call)
                    || control.instance != call.caller
                    || control.semantic_block != Some(call.block)
                    || instances.block_reachable(call.caller, call.block) != Some(true)
                    || control.return_values.is_some()
                    || control.expected_branch.is_none()
                    || entries
                        .get_mut(child.index())
                        .ok_or_else(source_raw_physical_error_v29)?
                        .replace(control)
                        .is_some()
                {
                    return Err(source_raw_physical_error_v29());
                }
            }
            InstanceControlOriginV1::ParameterPreheader { call, child } => {
                if instances
                    .incoming(child)
                    .map(|incoming| incoming.occurrence())
                    != Some(call)
                    || control.instance != child
                    || control.semantic_block.is_some()
                    || control.return_values.is_some()
                    || control.expected_branch.is_none()
                    || preheaders
                        .get_mut(child.index())
                        .ok_or_else(source_raw_physical_error_v29)?
                        .replace(control)
                        .is_some()
                {
                    return Err(source_raw_physical_error_v29());
                }
            }
            InstanceControlOriginV1::Retained | InstanceControlOriginV1::ExpandedReturn { .. } => {}
        }
        if let Some(values) = &control.return_values {
            let source_block = control
                .semantic_block
                .ok_or_else(source_raw_physical_error_v29)?;
            budget.charge_work(call_splice_search_work_v1(expected_returns.len()))?;
            let index = expected_returns
                .binary_search_by_key(&(control.instance.index(), source_block.index()), |row| {
                    (row.0.index(), row.1.index())
                })
                .map_err(|_| source_raw_physical_error_v29())?;
            if std::mem::replace(&mut expected_returns[index].2, true) {
                return Err(source_raw_physical_error_v29());
            }
            let expected = pending
                .coordinates
                .values
                .rows
                .get(values.clone())
                .ok_or_else(source_raw_physical_error_v29)?;
            budget.charge_work(expected.len())?;
            match instances.incoming(control.instance) {
                Some(call) => {
                    if control.origin
                        != (InstanceControlOriginV1::ExpandedReturn {
                            call: call.occurrence(),
                        })
                        || !matches!(terminator, Terminator::Branch { arguments, .. } if arguments.as_slice() == expected)
                    {
                        return Err(source_raw_physical_error_v29());
                    }
                }
                None => {
                    if control.instance != instances.root()
                        || control.origin != InstanceControlOriginV1::Retained
                        || !matches!(terminator, Terminator::Return { values } if values.as_slice() == expected)
                    {
                        return Err(source_raw_physical_error_v29());
                    }
                }
            }
            budget.charge_work(call_splice_search_work_v1(source_index.terminators.len()))?;
            let span = &source_index.terminators[source_index
                .terminators
                .binary_search_by_key(
                    &(control.instance.index(), source_block.index()),
                    SourceAddressTerminatorV29::key,
                )
                .map_err(|_| source_raw_physical_error_v29())?]
            .span;
            if span.kernel_ir_block != control.original_block {
                return Err(source_raw_physical_error_v29());
            }
            let gap = span
                .first_operation_ordinal
                .checked_add(span.operation_count)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let mut mapping = ScopedEmittedPointsV29 {
                coordinates: &pending.coordinates,
                relocation: &pending.slot_relocation,
                budget,
            };
            if mapping
                .emitted_point(control.instance, control.original_block, gap, true)
                .map_err(source_address_point_error_v29)?
                != Some((
                    block.id,
                    u32::try_from(block.operations.len())
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                ))
            {
                return Err(source_raw_physical_error_v29());
            }
            for slot in ranges[control.instance.index()]
                .clone()
                .ok_or_else(source_raw_physical_error_v29)?
            {
                budget.charge_work(6)?;
                if lifetimes.len() == capacity {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                lifetimes.push(SourceAddressLifetimeV29 {
                    block: block.id,
                    gap: block.operations.len(),
                    sequence: argument_sum_v1(&[return_sequence, lifetimes.len()])?,
                    source_order: [0, 0, 2, control.instance.index(), lifetimes.len()],
                    slot,
                    live: false,
                });
            }
        }
    }
    budget.charge_work(expected_returns.len())?;
    if expected_returns.iter().any(|row| !row.2) {
        return Err(source_raw_physical_error_v29());
    }
    for index in 0..count {
        budget.charge_work(3)?;
        let instance = instances
            .id_at(index)
            .ok_or_else(source_raw_physical_error_v29)?;
        if instances.instance_reachable(instance) == Some(false) {
            if ranges[index].is_some() || entries[index].is_some() || preheaders[index].is_some() {
                return Err(source_raw_physical_error_v29());
            }
            continue;
        }
        let range = ranges[index]
            .clone()
            .ok_or_else(source_raw_physical_error_v29)?;
        if instance == instances.root() {
            if entries[index].is_some() || preheaders[index].is_some() {
                return Err(source_raw_physical_error_v29());
            }
            continue;
        }
        let entry = entries[index].ok_or_else(source_raw_physical_error_v29)?;
        let preheader = preheaders[index].ok_or_else(source_raw_physical_error_v29)?;
        let preheader_index = graph.block(preheader.physical_block, budget)?;
        if incoming[preheader_index] != 1
            || entry.expected_branch.as_ref().map(|row| row.0) != Some(preheader.physical_block)
        {
            return Err(source_raw_physical_error_v29());
        }
        let sidecar = source_index.sidecar(instance, budget)?;
        let original_entry =
            source_address_invocation_entry_v29(instances, instance, sidecar, budget)?;
        if preheader.original_block != original_entry
            || preheader.expected_branch.as_ref().map(|row| row.0) != Some(original_entry)
        {
            return Err(source_raw_physical_error_v29());
        }
        for slot in range {
            budget.charge_work(6)?;
            if lifetimes.len() == capacity {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            lifetimes.push(SourceAddressLifetimeV29 {
                block: preheader.physical_block,
                gap: 0,
                sequence: lifetimes.len(),
                source_order: [0, 0, 0, instance.index(), 0],
                slot,
                live: match slots.slots[slot].origin.identity {
                    ScopedAllocationIdentityV29::OriginalObject { generation, .. } => {
                        generation == 0
                    }
                    _ => true,
                },
            });
            if kills.len() == capacity {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            budget.charge_work(1)?;
            kills.push(SourceAddressKillV29 {
                block: preheader.physical_block,
                gap: 0,
                slot,
                source_order: [0; 5],
            });
        }
    }
    // Invocation reset precedes source operations; explicit source boundaries
    // retain their original event order even when no physical op separates them.
    for row in boundaries {
        budget.charge_work(2)?;
        // The boundary census still authenticates every original Move. A
        // diagnostic Move only invalidates the ordered failure shadow below.
        if source_boundary_is_failure_move_v29(row) {
            continue;
        }
        if let Some(mut live) = row.activation() {
            if live
                && let ScopedAllocationIdentityV29::OriginalObject { local, generation } =
                    slots.slots[row.slot].origin.identity
            {
                let function = instances
                    .instance(row.instance)
                    .ok_or_else(source_raw_physical_error_v29)?
                    .declaration();
                let activation =
                    source_address_live_generation_v29(function, row.frame.site, budget)?;
                let ScopedAllocationSourceV29::OriginalObject { cell, schema } =
                    slots.slots[row.slot].origin.source
                else {
                    return Err(source_raw_physical_error_v29());
                };
                let (representative, physical, joined) = plan.physical_object_cell(cell, budget)?;
                if representative != cell
                    || physical.instance != row.instance
                    || physical.local.index() != local
                    || physical.generation != generation
                    || physical.kind != SourceBackingKindV29::Object(schema)
                {
                    return Err(source_raw_physical_error_v29());
                }
                if joined {
                    let (_, original_representative, original_physical) = plan
                        .physical_object_generation(
                            row.instance,
                            physical.local,
                            activation,
                            budget,
                        )?
                        .ok_or_else(source_raw_physical_error_v29)?;
                    if original_representative != cell || original_physical != physical {
                        return Err(source_raw_physical_error_v29());
                    }
                } else {
                    live = generation == activation;
                }
            }
            if lifetimes.len() == capacity {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            budget.charge_work(5)?;
            lifetimes.push(SourceAddressLifetimeV29 {
                block: row.block,
                gap: row.gap,
                sequence: argument_sum_v1(&[capacity, lifetimes.len()])?,
                source_order: [0, 0, 1, row.instance.index(), row.anchor],
                slot: row.slot,
                live,
            });
        }
        budget.charge_work(1)?;
        if kills.len() == capacity {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        kills.push(SourceAddressKillV29 {
            block: row.block,
            gap: row.gap,
            slot: row.slot,
            source_order: [0, 0, 1, row.instance.index(), row.anchor],
        });
    }
    call_splice_sort_work_v1(argument_product_v1(lifetimes.len(), 3)?, budget)
        .map_err(source_address_call_error_v29)?;
    lifetimes.sort_unstable_by_key(|row| (row.block, row.gap, row.sequence));
    call_splice_sort_work_v1(argument_product_v1(kills.len(), 8)?, budget)
        .map_err(source_address_call_error_v29)?;
    kills.sort_unstable_by_key(|row| (row.block, row.gap, row.source_order, row.slot));
    budget.charge_work(kills.len())?;
    kills.dedup();
    let scratch = argument_sum_v1(&[
        argument_product_v1(
            ranges.capacity(),
            std::mem::size_of::<Option<std::ops::Range<usize>>>(),
        )?,
        argument_product_v1(
            entries.capacity(),
            std::mem::size_of::<Option<&InstanceControlV1>>(),
        )?,
        argument_product_v1(
            preheaders.capacity(),
            std::mem::size_of::<Option<&InstanceControlV1>>(),
        )?,
        argument_product_v1(incoming.capacity(), std::mem::size_of::<usize>())?,
        argument_product_v1(
            expected_returns.capacity(),
            std::mem::size_of::<(ProductionCallInstanceIdV1, SemanticBlockIdV1, bool)>(),
        )?,
    ])?;
    drop((ranges, entries, preheaders, incoming, expected_returns));
    budget.release_storage(scratch)?;
    Ok(SourceAddressLifetimesV29 {
        initial,
        lifetimes,
        kills,
    })
}

fn source_address_original_slot_v29(
    instances: &ExecutionInstancesV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    if slots.ledger != budget.work_ledger_identity_v1()
        || slots.source != ExecutionCallSourceV29::from_instances(instances, budget)?
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let original = instances
        .instance(instance)
        .ok_or_else(source_raw_physical_error_v29)?;
    budget.charge_work(4)?;
    if original
        .declaration()
        .locals()
        .get(local.index() as usize)
        .map(|local| local.ty())
        != Some(ty)
    {
        return Err(source_raw_physical_error_v29());
    }
    budget.charge_work(call_splice_search_work_v1(slots.instances.len()))?;
    let owner = slots
        .instances
        .binary_search_by_key(&instance.index(), |row| row.instance.index())
        .ok()
        .and_then(|index| slots.instances.get(index))
        .ok_or_else(source_raw_physical_error_v29)?;
    if owner.function != original.function() {
        return Err(source_raw_physical_error_v29());
    }
    let rows = slots
        .slots
        .get(owner.slots.clone())
        .ok_or_else(source_raw_physical_error_v29)?;
    budget.charge_work(call_splice_search_work_v1(rows.len()))?;
    let offset = rows
        .binary_search_by_key(
            &ScopedAllocationIdentityV29::LegacyLocal(local.index()),
            |row| row.origin.identity,
        )
        .map_err(|_| source_raw_physical_error_v29())?;
    let row = &rows[offset];
    let scalar = row.scalar_array()?;
    budget.charge_work(5)?;
    let (element, length) = match instances
        .owner()
        .source_semantic()
        .types()
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    {
        Some(SemanticTypeShapeV1::Array { element, length }) => (*element, *length),
        Some(_) => (ty, 1),
        None => return Err(source_raw_physical_error_v29()),
    };
    if row.instance != instance
        || row.origin.semantic_type != ty
        || scalar.element_type != element
        || scalar.length != length
        || scalar.bytes
            != length
                .checked_mul(scalar.element.size)
                .ok_or(ArgumentResourceV1::Arithmetic)?
    {
        return Err(source_raw_physical_error_v29());
    }
    argument_sum_v1(&[owner.slots.start, offset]).map_err(Into::into)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceDirectObjectOriginV29 {
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    ty: SemanticTypeIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceSafeObjectOriginV29 {
    frame: ScopedMemoryFrameV29,
    key: SourceReferenceAccessKeyV29,
    loan: usize,
    referent: SourceDirectObjectOriginV29,
    formation: SourceReferenceSiteV29,
}

#[derive(Clone, Copy)]
struct SourceAddressAccessSourceV29 {
    instance: ProductionCallInstanceIdV1,
    anchor: usize,
    physical: SourceAddressAccessV29,
    raw: Option<SourceReferenceRawAccessV29>,
    direct_object: Option<SourceDirectObjectOriginV29>,
    safe_object: Option<SourceSafeObjectOriginV29>,
}

include!("production_source_tag_census_v43.rs");

// A completed descriptor producer may be classified outside the private access
// roster only after this exact source/actual join. The physical solver separately
// requires its actual pointer to have no private-slot origin; this is not a seed.
fn source_address_external_descriptor_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    instance: ProductionCallInstanceIdV1,
    row: &ScopedMemoryAnchorV29,
    place: &SemanticPlaceV1,
    prefix: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    if prefix < 2 || prefix != place.projections().len() {
        return Ok(false);
    }
    let frame = row.source.ok_or_else(source_raw_physical_error_v29)?;
    let Some((index, _)) =
        references
            .plan
            .descriptor_at(instance, frame.site, place, prefix - 1, budget)?
    else {
        return Ok(false);
    };
    references.check(budget)?;
    budget.charge_work(3)?;
    let claim = references
        .descriptors
        .get(index)
        .and_then(|claim| claim.get())
        .ok_or_else(source_descriptor_error_v29)?;
    let SourceReferenceSelectorProducerV29::Address {
        base,
        offset,
        pointer,
        block,
        operation,
    } = claim.producer
    else {
        return Err(source_descriptor_error_v29());
    };
    if !matches!(row.kind, ScopedMemoryAnchorKindV29::Access { pointer: actual, .. } if actual == pointer)
    {
        return Err(source_descriptor_error_v29());
    }
    let producer = source_index
        .emitted
        .operation(instance, block, operation, budget)?;
    let (
        OperationKind::GetElementPointer {
            base: actual_base,
            offset: actual_offset,
        },
        [result],
    ) = (&producer.kind, producer.results.as_slice())
    else {
        return Err(source_descriptor_error_v29());
    };
    if *actual_base != base
        || *actual_offset != offset
        || result.id != pointer
        || !matches!(&result.ty, Type::Pointer(_))
    {
        return Err(source_descriptor_error_v29());
    }
    if source_address_value_access_v29(source_index.emitted.operation(
        instance,
        row.block,
        row.position,
        budget,
    )?)?
    .is_none_or(|access| access.pointer != pointer || access.object)
    {
        return Err(source_descriptor_error_v29());
    }
    Ok(true)
}

// Source targets are hypotheses only. The physical solver must independently
// derive the same object from actual Allocas, pointer Stores/Loads and edges.
#[cfg(test)]
type SourceAddressAccessCapacityObserverV29 =
    fn(&SourceAddressSourceIndexV29<'_>, &[SourceAddressAccessSourceV29], usize, usize, usize);

#[cfg(test)]
thread_local! {
    static SOURCE_ADDRESS_ACCESS_CAPACITY_OBSERVER_V29: std::cell::Cell<Option<SourceAddressAccessCapacityObserverV29>> = const { std::cell::Cell::new(None) };
    static SOURCE_ADDRESS_ACCESS_FIRST_CENSUS_V29: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static SOURCE_ADDRESS_ACCESS_ALLOCATIONS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn source_address_accesses_v29(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    (
        Vec<SourceAddressAccessSourceV29>,
        PendingSourceIssuedRolesV29,
    ),
    ProductionSemanticKirErrorV1,
> {
    references.plan.check_owner(instances, budget)?;
    budget.reserve_storage(source_issued_census_query_headers_v29()?)?;
    budget.reserve_storage(source_address_tag_query_headers_v43()?)?;
    source_reference_emission_prepay_v29::<SourceDirectObjectOriginV29>(budget)?;
    source_reference_emission_prepay_v29::<Option<SourceDirectObjectOriginV29>>(budget)?;
    source_reference_emission_prepay_v29::<SourceSafeObjectOriginV29>(budget)?;
    source_reference_emission_prepay_v29::<Option<SourceSafeObjectOriginV29>>(budget)?;
    source_reference_emission_prepay_v29::<Option<&SourceReferenceLoanV29>>(budget)?;
    source_reference_emission_prepay_v29::<&SourceReferenceLoanV29>(budget)?;
    source_reference_emission_prepay_v29::<(
        ScopedObjectEndpointV29,
        ScopedObjectIdentityV29,
        ProductionCallInstanceIdV1,
        SemanticLocalIdV1,
        u32,
        SemanticTypeIdV1,
    )>(budget)?;
    let issued_header = std::mem::size_of::<Option<SourceIssuedAccessesV29<'_, '_, '_>>>();
    budget.reserve_storage(issued_header)?;
    let mut issued = None;
    let mut count = 0;
    for sidecar in &source_index.pending.sidecars.rows {
        budget.charge_work(1)?;
        let anchors = sidecar
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?;
        for row in &anchors.rows {
            #[cfg(test)]
            if SOURCE_ADDRESS_ACCESS_FIRST_CENSUS_V29.get().is_none() {
                SOURCE_ADDRESS_ACCESS_FIRST_CENSUS_V29.set(Some(budget.work()));
            }
            budget.charge_work(1)?;
            // Only value- or tag-access anchors can enter this vector. The full fill
            // below still authenticates every payload and may exclude issued
            // or descriptor accesses, so this remains a conservative bound.
            let candidate = match row.kind {
                ScopedMemoryAnchorKindV29::Access {
                    payload: Some(_), ..
                } => true,
                ScopedMemoryAnchorKindV29::Object(_) => {
                    let payload = anchors.object_payload(row, budget)?;
                    budget.charge_work(2)?;
                    matches!(
                        payload.operation,
                        ScopedObjectOperationV29::ReadValue { .. }
                            | ScopedObjectOperationV29::WriteValue { .. }
                            | ScopedObjectOperationV29::ReadDiscriminant { .. }
                            | ScopedObjectOperationV29::SetDiscriminant { .. }
                    )
                }
                _ => false,
            };
            if candidate {
                count = argument_sum_v1(&[count, 1])?;
            }
        }
    }
    #[cfg(test)]
    let capacity_floor = budget.storage();
    let mut rows = emission_vec_v1(count, budget)?;
    #[cfg(test)]
    let capacity_bytes = budget.storage() - capacity_floor;
    #[cfg(test)]
    SOURCE_ADDRESS_ACCESS_ALLOCATIONS_V29.set(SOURCE_ADDRESS_ACCESS_ALLOCATIONS_V29.get() + 1);
    let mut raw_seen = emission_vec_v1(references.plan.raw_accesses.len(), budget)?;
    budget.charge_work(references.plan.raw_accesses.len())?;
    raw_seen.resize(references.plan.raw_accesses.len(), false);
    for sidecar in &source_index.pending.sidecars.rows {
        let instance = sidecar
            .source_call_instance
            .ok_or_else(source_raw_physical_error_v29)?;
        let original = instances
            .instance(instance)
            .ok_or_else(source_raw_physical_error_v29)?
            .declaration();
        for (anchor, row) in sidecar
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?
            .rows
            .iter()
            .enumerate()
        {
            budget.charge_work(4)?;
            let recorded = sidecar
                .scoped_memory_anchors
                .as_ref()
                .ok_or_else(source_raw_physical_error_v29)?;
            if let Some(tag) = source_address_tag_source_v43(
                instances,
                references.plan,
                source_index,
                slots,
                instance,
                anchor,
                recorded,
                row,
                budget,
            )? {
                if rows.len() == count {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                rows.push(tag);
                continue;
            }
            let object = source_address_object_payload_v29(
                sidecar
                    .scoped_memory_anchors
                    .as_ref()
                    .ok_or_else(source_raw_physical_error_v29)?,
                row,
                budget,
            )?;
            let payload = match (row.kind, object) {
                (ScopedMemoryAnchorKindV29::Access { payload, .. }, None) => payload,
                (ScopedMemoryAnchorKindV29::Object(_), Some((_, payload))) => Some(payload),
                (_, None) => continue,
                _ => return Err(source_raw_physical_error_v29()),
            };
            let mut safe_object = None;
            let (target, raw) = match payload {
                Some(ScopedMemoryPayloadV29::IndexLoad { read, .. }) => {
                    let frame = row.source.ok_or_else(source_raw_physical_error_v29)?;
                    let occurrences = instances
                        .occurrences(instance)
                        .ok_or_else(source_raw_physical_error_v29)?;
                    check_scoped_index_read_v29(original, &occurrences, read, budget)?;
                    if frame != ScopedMemoryFrameV29::operand(read.site, Some(read.role)) {
                        return Err(source_raw_physical_error_v29());
                    }
                    source_index.frame_gap(instance, frame, row.block, row.position, budget)?;
                    let slot = if let Some((endpoint, _)) = object {
                        source_address_object_index_v29(
                            instances,
                            references.plan,
                            slots,
                            instance,
                            read,
                            endpoint,
                            budget,
                        )?
                    } else {
                        source_address_original_slot_v29(
                            instances, slots, instance, read.local, read.ty, budget,
                        )?
                    };
                    (slot, None)
                }
                Some(ScopedMemoryPayloadV29::Store {
                    source: ScopedMemoryStoreSourceV29::EntryArgument { local, ty },
                    ..
                }) => {
                    if row.source.is_some()
                        || !original.locals().get(local.index() as usize).is_some_and(
                            |declaration| {
                                declaration.ty() == ty && declaration.role().is_entry_argument()
                            },
                        )
                    {
                        return Err(source_raw_physical_error_v29());
                    }
                    let slot = if let Some((endpoint, _)) = object {
                        source_address_object_entry_v29(
                            instances,
                            references.plan,
                            slots,
                            instance,
                            endpoint,
                            budget,
                        )?
                    } else {
                        source_address_original_slot_v29(
                            instances, slots, instance, local, ty, budget,
                        )?
                    };
                    (slot, None)
                }
                Some(payload) => {
                    let frame = row.source.ok_or_else(source_raw_physical_error_v29)?;
                    source_index.frame_gap(instance, frame, row.block, row.position, budget)?;
                    let (place, prefix, access) = match payload {
                        ScopedMemoryPayloadV29::IndexLoad { .. } => {
                            return Err(source_raw_physical_error_v29());
                        }
                        ScopedMemoryPayloadV29::Load { read, .. } => {
                            if frame != ScopedMemoryFrameV29::operand(read.site, Some(read.role)) {
                                return Err(source_raw_physical_error_v29());
                            }
                            (
                                scoped_payload_place_v29(original, read.site, read.role)
                                    .ok_or_else(source_raw_physical_error_v29)?,
                                read.prefix as usize,
                                SourceReferenceAccessV29::Read,
                            )
                        }
                        ScopedMemoryPayloadV29::Store { .. } => {
                            let place = match frame.role {
                                Some(ScopedMemoryRoleV29::Operand(role)) => {
                                    scoped_source_place_v29(original, frame.site, role)
                                }
                                Some(ScopedMemoryRoleV29::CallResult) => {
                                    scoped_source_call_destination_v29(original, frame.site)
                                }
                                None => None,
                            }
                            .ok_or_else(source_raw_physical_error_v29)?;
                            (
                                place,
                                place.projections().len(),
                                SourceReferenceAccessV29::Write,
                            )
                        }
                    };
                    let (block, statement) = scoped_memory_site_key_v29(frame.site);
                    let site = SourceReferenceSiteV29 {
                        instance,
                        block: SemanticBlockIdV1::from_index(block),
                        statement: statement.map(|value| value as usize),
                    };
                    if prefix > place.projections().len() {
                        return Err(source_raw_physical_error_v29());
                    }
                    if object.is_none()
                        && source_address_external_descriptor_v29(
                            references,
                            source_index,
                            instance,
                            row,
                            place,
                            prefix,
                            budget,
                        )?
                    {
                        continue;
                    }
                    let access = if object.is_some() && prefix < place.projections().len() {
                        source_reference_raw_original_access_v29(original, site, place, budget)?
                            .ok_or_else(source_raw_physical_error_v29)?
                    } else {
                        access
                    };
                    if prefix == 0
                        || object.is_some_and(|(endpoint, _)| {
                            matches!(endpoint.object, ScopedObjectIdentityV29::Local { .. })
                        })
                    {
                        let ty = original
                            .locals()
                            .get(place.local().index() as usize)
                            .ok_or_else(source_raw_physical_error_v29)?
                            .ty();
                        let slot = if let Some((endpoint, _)) = object {
                            source_address_object_direct_v29(
                                instances,
                                references.plan,
                                slots,
                                site,
                                place,
                                access,
                                endpoint,
                                budget,
                            )?
                        } else {
                            source_address_original_slot_v29(
                                instances,
                                slots,
                                instance,
                                place.local(),
                                ty,
                                budget,
                            )?
                        };
                        (slot, None)
                    } else if let Some((endpoint, _)) = object
                        && matches!(frame.role, Some(ScopedMemoryRoleV29::Operand(_)))
                        && let Some(checked) = source_aggregate_object_endpoint_access_v29(
                            references.plan,
                            site,
                            place,
                            access,
                            endpoint,
                            budget,
                        )?
                    {
                        let loan = checked.loan.original;
                        (
                            source_address_object_slot_v29(
                                instances,
                                references.plan,
                                slots,
                                loan.instance,
                                loan.local,
                                loan.generation,
                                loan.ty,
                                budget,
                            )?,
                            None,
                        )
                    } else if let Some((endpoint, _)) = object
                        && prefix != 0
                        && prefix == place.projections().len()
                        && place.projections()[prefix - 1].kind()
                            == SemanticProjectionKindV1::Dereference
                        && matches!(instances.owner().source_semantic().types()[if prefix == 1 {
                            original.locals()[place.local().index() as usize].ty().index() as usize
                        } else { place.projections()[prefix - 2].result_type().index() as usize }].shape(),
                            SemanticTypeShapeV1::Pointer(pointer) if pointer.kind() == SemanticPointerKindV1::Reference
                                && pointer.metadata() == SemanticPointerMetadataV1::None)
                        && let Some(loan) = source_object_loan_access_v29(
                            references.plan,
                            site,
                            place,
                            access,
                            budget,
                        )?
                    {
                        budget.charge_work(8)?;
                        if prefix != place.projections().len()
                            || endpoint.root_type != loan.original.ty
                            || endpoint.projected_type != loan.original.ty
                            || loan.original.kind
                                != SourceBackingKindV29::Object(endpoint.root_schema)
                            || endpoint.root_schema != endpoint.projected_schema
                            || endpoint.path.count != 0
                            || !matches!(endpoint.object, ScopedObjectIdentityV29::Reference {
                                instance: actual, site: original, role, dereference_prefix,
                            } if actual == instance && original == frame.site && dereference_prefix as usize == prefix
                                && frame.role == Some(ScopedMemoryRoleV29::Operand(role)))
                        {
                            return Err(scoped_object_error_v29());
                        }
                        budget.charge_work(8)?;
                        let record = references
                            .plan
                            .loans
                            .get(loan.loan)
                            .ok_or_else(source_raw_physical_error_v29)?;
                        safe_object = Some(SourceSafeObjectOriginV29 {
                            frame,
                            key: SourceReferenceAccessKeyV29 {
                                site,
                                source: place as *const SemanticPlaceV1 as usize,
                                access,
                            },
                            loan: loan.loan,
                            referent: SourceDirectObjectOriginV29 {
                                instance: loan.original.instance,
                                local: loan.original.local,
                                generation: loan.original.generation,
                                ty: loan.original.ty,
                            },
                            formation: record.site,
                        });
                        (
                            source_address_object_slot_v29(
                                instances,
                                references.plan,
                                slots,
                                loan.original.instance,
                                loan.original.local,
                                loan.original.generation,
                                loan.original.ty,
                                budget,
                            )?,
                            None,
                        )
                    } else if object.is_none()
                        && prefix > 1
                        && prefix == place.projections().len()
                        && matches!(frame.role, Some(ScopedMemoryRoleV29::Operand(_)))
                        && let Some(loan) = source_scalar_loan_access_v29(
                            references.plan,
                            site,
                            place,
                            access,
                            budget,
                        )?
                    {
                        (
                            source_address_original_slot_v29(
                                instances,
                                slots,
                                loan.cell.instance,
                                loan.cell.local,
                                loan.cell.ty,
                                budget,
                            )?,
                            None,
                        )
                    } else if prefix == 1
                        && matches!(
                            place.projections()[0].kind(),
                            SemanticProjectionKindV1::Index(_)
                                | SemanticProjectionKindV1::ConstantIndex { .. }
                        )
                    {
                        budget.charge_work(5)?;
                        let ty = original
                            .locals()
                            .get(place.local().index() as usize)
                            .ok_or_else(source_raw_physical_error_v29)?
                            .ty();
                        let Some(SemanticTypeShapeV1::Array { element, .. }) = instances
                            .owner()
                            .source_semantic()
                            .types()
                            .get(ty.index() as usize)
                            .map(SemanticTypeDeclV1::shape)
                        else {
                            return Err(source_raw_physical_error_v29());
                        };
                        if place.projections()[0].result_type() != *element
                            || place.ty() != *element
                        {
                            return Err(source_raw_physical_error_v29());
                        }
                        if let SemanticProjectionKindV1::Index(_) = place.projections()[0].kind() {
                            let (_, selector) = references
                                .plan
                                .selector_at(instance, frame.site, place, 0, budget)?
                                .ok_or_else(source_raw_physical_error_v29)?;
                            selector.check(instances, budget)?;
                        }
                        (
                            source_address_original_slot_v29(
                                instances,
                                slots,
                                instance,
                                place.local(),
                                ty,
                                budget,
                            )?,
                            None,
                        )
                    } else {
                        // A nested holder must retain its exact original raw
                        // crossing. The pointee still needs its full cell proof.
                        let crossing = prefix
                            .checked_sub(1)
                            .ok_or_else(source_raw_physical_error_v29)?;
                        if (prefix != 1
                            && (object.is_none() || prefix != place.projections().len()))
                            || !matches!(
                                place.projections()[crossing].kind(),
                                SemanticProjectionKindV1::Dereference
                            )
                        {
                            return Err(source_reference_error_v29(
                                "raw source memory needs its exact physical subobject path",
                            ));
                        }
                        let key = source_reference_access_key_v29(site, place, access);
                        charge_execution_cfg_lookup_v29(
                            references.plan.raw_accesses.len(),
                            budget,
                        )?;
                        let raw = if crossing == 0 {
                            references
                                .plan
                                .raw_accesses
                                .get(&(key, crossing))
                                .map(|row| **row)
                        } else {
                            Some(source_static_raw_holder_v42(
                                references.plan,
                                site,
                                place,
                                access,
                                crossing,
                                budget,
                            )?)
                        };
                        if let Some(source) = raw {
                            budget.charge_work(10)?;
                            if source.site != site
                                || source.source != place as *const SemanticPlaceV1 as usize
                                || source.access != access
                                || source.crossing != access
                                || source.projection != crossing
                                || source.pointee != place.ty()
                                || source.ty != place.ty()
                            {
                                return Err(source_raw_physical_error_v29());
                            }
                            let set = references
                                .plan
                                .raw_sets
                                .get(source.set)
                                .ok_or_else(source_raw_physical_error_v29)?;
                            let mut target = None;
                            for offset in 0..set.count {
                                budget.charge_work(4)?;
                                let choice = references
                                    .plan
                                    .raw_choices
                                    .get(argument_sum_v1(&[set.first, offset])?)
                                    .ok_or_else(source_raw_physical_error_v29)?;
                                let origin = references
                                    .plan
                                    .raw_origins
                                    .get(choice.origin)
                                    .ok_or_else(source_raw_physical_error_v29)?;
                                if choice.expired
                                    || origin.count != 0
                                    || origin.parent.is_some()
                                    || origin.ty != place.ty()
                                {
                                    return Err(source_raw_physical_error_v29());
                                }
                                let candidate = if let Some((endpoint, _)) = object {
                                    let candidate = source_address_object_slot_v29(
                                        instances,
                                        references.plan,
                                        slots,
                                        origin.instance,
                                        origin.local,
                                        origin.generation,
                                        origin.ty,
                                        budget,
                                    )?;
                                    if endpoint.projected_type != origin.ty
                                        || !matches!(slots.slots[candidate].representation, ScopedSlotRepresentationV29::Object { schema, .. }
                                            if schema == endpoint.projected_schema)
                                    {
                                        return Err(source_raw_physical_error_v29());
                                    }
                                    candidate
                                } else {
                                    source_address_original_slot_v29(
                                        instances,
                                        slots,
                                        origin.instance,
                                        origin.local,
                                        origin.ty,
                                        budget,
                                    )?
                                };
                                if target
                                    .replace(candidate)
                                    .is_some_and(|old| old != candidate)
                                {
                                    return Err(source_reference_error_v29(
                                        "raw physical access needs correlated multi-object alternatives",
                                    ));
                                }
                            }
                            *raw_seen
                                .get_mut(source.ordinal)
                                .ok_or_else(source_raw_physical_error_v29)? = true;
                            (
                                target.ok_or_else(source_raw_physical_error_v29)?,
                                Some(source),
                            )
                        } else {
                            charge_execution_cfg_lookup_v29(
                                references.plan.access_sites.len(),
                                budget,
                            )?;
                            let source = references
                                .plan
                                .access_sites
                                .get(&key)
                                .and_then(|&index| references.plan.accesses.get(index));
                            if source.is_none() && object.is_none() {
                                if issued.is_none() {
                                    issued = Some(SourceIssuedAccessesV29::new(
                                        references.plan,
                                        instances,
                                        source_index,
                                        budget,
                                    )?);
                                }
                                if issued
                                    .as_mut()
                                    .ok_or(ArgumentResourceV1::Accounting)?
                                    .access(references, instance, anchor, row, place, budget)?
                                {
                                    continue;
                                }
                            }
                            let source = source.ok_or_else(source_raw_physical_error_v29)?;
                            budget.charge_work(7)?;
                            if source.key.site != site
                                || source.key.source != place as *const SemanticPlaceV1 as usize
                                || source.key.access != access
                                || !source.projections.is_empty()
                                || source.loan.is_none()
                                || source.ty != place.ty()
                            {
                                return Err(source_raw_physical_error_v29());
                            }
                            (
                                source_address_original_slot_v29(
                                    instances,
                                    slots,
                                    source.instance,
                                    source.local,
                                    source.ty,
                                    budget,
                                )?,
                                None,
                            )
                        }
                    }
                }
                None => {
                    if scoped_raw_admission_v29::source_address_compiler_enum_access_v55(
                        instances,
                        references,
                        slots,
                        source_index,
                        instance,
                        anchor,
                        budget,
                    )?
                    .is_some()
                    {
                        continue;
                    }
                    return Err(source_reference_error_v29(
                        "raw-root access lacks its exact source payload role",
                    ));
                }
            };
            let mut mapping = ScopedEmittedPointsV29 {
                coordinates: &source_index.pending.coordinates,
                relocation: &source_index.pending.slot_relocation,
                budget,
            };
            let (block, operation) = mapping
                .emitted_point(
                    instance,
                    row.block,
                    u32::try_from(row.position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    false,
                )
                .map_err(source_address_point_error_v29)?
                .ok_or_else(source_raw_physical_error_v29)?;
            // Retain the authenticated logical endpoint, never infer its
            // generation from a physical slot shared by several source epochs.
            budget.charge_work(2)?;
            let direct_object = match object {
                Some((endpoint, _)) if raw.is_none() => match endpoint.object {
                    ScopedObjectIdentityV29::Local {
                        instance: source,
                        local,
                        generation,
                    } if source == instance => Some(SourceDirectObjectOriginV29 {
                        instance: source,
                        local,
                        generation,
                        ty: endpoint.root_type,
                    }),
                    ScopedObjectIdentityV29::Local { .. } => {
                        return Err(source_raw_physical_error_v29());
                    }
                    _ => None,
                },
                _ => None,
            };
            budget.charge_work(2)?;
            if rows.len() == count {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            rows.push(SourceAddressAccessSourceV29 {
                instance,
                anchor,
                physical: SourceAddressAccessV29 {
                    footprint: 0,
                    block,
                    operation: operation as usize,
                    slot: target,
                },
                raw,
                direct_object,
                safe_object,
            });
        }
    }
    #[cfg(test)]
    if let Some(observe) = SOURCE_ADDRESS_ACCESS_CAPACITY_OBSERVER_V29.get() {
        observe(source_index, &rows, count, rows.capacity(), capacity_bytes);
    }
    let issuer_count = source_issued_source_count_v29(instances, budget)?;
    if issuer_count != 0 {
        if issued.is_none() {
            issued = Some(SourceIssuedAccessesV29::new(
                references.plan,
                instances,
                source_index,
                budget,
            )?);
        }
        issued
            .as_mut()
            .ok_or(ArgumentResourceV1::Accounting)?
            .census(references, issuer_count, budget)?;
    } else if issued.as_ref().is_some_and(|rows| !rows.issuers.is_empty()) {
        return Err(source_issued_error_v29());
    }
    let retained_issued = match issued {
        Some(issued) => issued.finish(budget)?,
        None => PendingSourceIssuedRolesV29::empty(),
    };
    budget.release_storage(issued_header)?;
    budget.charge_work(raw_seen.len())?;
    if raw_seen.iter().any(|seen| !seen) {
        return Err(source_raw_physical_error_v29());
    }
    call_splice_sort_work_v1(argument_product_v1(rows.len(), 3)?, budget)
        .map_err(source_address_call_error_v29)?;
    rows.sort_unstable_by_key(|row| {
        (
            row.physical.block,
            row.physical.operation,
            row.physical.footprint,
        )
    });
    for pair in rows.windows(2) {
        budget.charge_work(3)?;
        if (
            pair[0].physical.block,
            pair[0].physical.operation,
            pair[0].physical.footprint,
        ) == (
            pair[1].physical.block,
            pair[1].physical.operation,
            pair[1].physical.footprint,
        ) {
            return Err(source_raw_physical_error_v29());
        }
    }
    let bytes = argument_product_v1(raw_seen.capacity(), std::mem::size_of::<bool>())?;
    drop(raw_seen);
    budget.release_storage(bytes)?;
    Ok((rows, retained_issued))
}

fn check_source_address_formations_v29(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    pending: &PendingScopedRootEmissionV29,
    slots: &OwnedScopedSourceSlotsV29,
    graph: &SourceAddressMemoryV29<'_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<SourceAddressBirthV29>, ProductionSemanticKirErrorV1> {
    references.plan.check_owner(instances, budget)?;
    references.check(budget)?;
    if !std::ptr::eq(source_index.pending, pending) {
        return Err(source_raw_physical_error_v29());
    }
    budget.charge_work(2)?;
    if references.raw_formations.len() != references.plan.raw_origins.len() {
        return Err(source_raw_physical_error_v29());
    }
    let mut births = emission_vec_v1(references.raw_formations.len(), budget)?;
    for (index, origin) in references.plan.raw_origins.iter().enumerate() {
        budget.charge_work(12)?;
        let receipt = references.raw_formations[index]
            .get()
            .ok_or_else(source_raw_physical_error_v29)?;
        let source = instances
            .instance(origin.site.instance)
            .ok_or_else(source_raw_physical_error_v29)?;
        let statement = origin
            .site
            .statement
            .ok_or_else(source_raw_physical_error_v29)?;
        let SemanticStatementKindV1::Assign(assign) = source
            .declaration()
            .blocks()
            .get(origin.site.block.index() as usize)
            .and_then(|block| block.statements().get(statement))
            .ok_or_else(source_raw_physical_error_v29)?
            .kind()
        else {
            return Err(source_raw_physical_error_v29());
        };
        let SemanticRvalueKindV1::AddressOf { place, mutability } = assign.value().kind() else {
            return Err(source_raw_physical_error_v29());
        };
        references.check_raw_formation_source_v29(
            origin.site,
            place,
            assign.value().result_type(),
            *mutability,
            budget,
        )?;
        if !place.projections().is_empty()
            || receipt.site != origin.site
            || receipt.source != origin.source
            || origin.instance != origin.site.instance
            || origin.local != place.local()
            || origin.ty != place.ty()
            || origin.count != 0
            || origin.parent.is_some()
        {
            return Err(source_raw_physical_error_v29());
        }
        let selected =
            source_reference_assignment_pointer_v29(references.plan, origin.site, assign, budget)?;
        let object = selected.is_some();
        let slot = if object {
            source_address_object_slot_v29(
                instances,
                references.plan,
                slots,
                origin.instance,
                origin.local,
                origin.generation,
                origin.ty,
                budget,
            )?
        } else {
            source_address_original_slot_v29(
                instances,
                slots,
                origin.instance,
                origin.local,
                origin.ty,
                budget,
            )?
        };
        let backing = slots
            .slots
            .get(slot)
            .ok_or_else(source_raw_physical_error_v29)?;
        if receipt.base != backing.origin.pointer
            || graph.exact(receipt.base, budget)? != Some(slot)
            || graph.exact(receipt.result, budget)? != Some(slot)
        {
            return Err(source_raw_physical_error_v29());
        }
        let span = source_index.statement(origin.site, budget)?;
        if span.correspondence_owner
            != instances
                .instance(instances.root())
                .ok_or_else(source_raw_physical_error_v29)?
                .function()
            || span.kernel_ir_block != receipt.block
            || receipt.first > receipt.end
            || receipt.first < span.first_operation_ordinal as usize
            || receipt.end
                > argument_sum_v1(&[
                    span.first_operation_ordinal as usize,
                    span.operation_count as usize,
                ])?
        {
            return Err(source_raw_physical_error_v29());
        }
        let types = instances.owner().source_semantic().types();
        let SemanticTypeShapeV1::Pointer(source_pointer) = types
            .get(origin.pointer_type.index() as usize)
            .ok_or_else(source_raw_physical_error_v29)?
            .shape()
        else {
            return Err(source_raw_physical_error_v29());
        };
        let Type::Pointer(result_type) = graph.ty(receipt.result, budget)? else {
            return Err(source_raw_physical_error_v29());
        };
        let expected_access = if *mutability == SemanticMutabilityV1::Mutable {
            AccessMode::ReadWrite
        } else {
            AccessMode::ReadOnly
        };
        if result_type.address_space
            != if object {
                AddressSpace::Private
            } else {
                source_address_space_v18(source_pointer.address_space())?
            }
            || result_type.access != expected_access
        {
            return Err(source_raw_physical_error_v29());
        }
        if let Some((expected, _)) = selected {
            SourceAddressMemoryV29::same_type(
                &expected,
                graph.ty(receipt.result, budget)?,
                budget,
            )?;
            let ScopedSlotRepresentationV29::Object { schema, .. } = backing.representation else {
                return Err(source_raw_physical_error_v29());
            };
            if result_type.pointee.as_ref() != &Type::StorageObject(schema) {
                return Err(source_raw_physical_error_v29());
            }
        } else if !backing
            .scalar_array()?
            .element
            .element
            .matches_borrowed(&result_type.pointee, budget)?
        {
            return Err(source_raw_physical_error_v29());
        }
        let mut current = receipt.base;
        let mut operation = receipt.first;
        for expected in [
            (expected_access == AccessMode::ReadOnly).then_some(CastKind::RestrictPointerAccess),
            (result_type.address_space == AddressSpace::Generic)
                .then_some(CastKind::PointerToGeneric),
        ]
        .into_iter()
        .flatten()
        {
            budget.charge_work(4)?;
            if operation >= receipt.end {
                return Err(source_raw_physical_error_v29());
            }
            let actual = source_address_original_operation_v29(
                pending,
                graph,
                origin.site.instance,
                receipt.block,
                operation,
                budget,
            )?;
            let (OperationKind::Cast { kind, value, .. }, [result]) =
                (&actual.kind, actual.results.as_slice())
            else {
                return Err(source_raw_physical_error_v29());
            };
            if *kind != expected || *value != current {
                return Err(source_raw_physical_error_v29());
            }
            graph.check_cast(actual, budget)?;
            current = result.id;
            operation = argument_sum_v1(&[operation, 1])?;
        }
        if expected_access == AccessMode::ReadWrite
            && result_type.address_space == AddressSpace::Private
        {
            if object {
                let condition = source_address_original_operation_v29(
                    pending,
                    graph,
                    origin.site.instance,
                    receipt.block,
                    operation,
                    budget,
                )?;
                let (OperationKind::Constant(Constant::Bool(true)), [condition]) =
                    (&condition.kind, condition.results.as_slice())
                else {
                    return Err(source_raw_physical_error_v29());
                };
                SourceAddressMemoryV29::same_type(&condition.ty, &Type::BOOL, budget)?;
                operation = argument_sum_v1(&[operation, 1])?;
                let alias = source_address_original_operation_v29(
                    pending,
                    graph,
                    origin.site.instance,
                    receipt.block,
                    operation,
                    budget,
                )?;
                let (
                    OperationKind::Select {
                        condition: actual,
                        true_value,
                        false_value,
                    },
                    [result],
                ) = (&alias.kind, alias.results.as_slice())
                else {
                    return Err(source_raw_physical_error_v29());
                };
                budget.charge_work(4)?;
                if *actual != condition.id || *true_value != current || *false_value != current {
                    return Err(source_raw_physical_error_v29());
                }
                SourceAddressMemoryV29::same_type(graph.ty(current, budget)?, &result.ty, budget)?;
                current = result.id;
                operation = argument_sum_v1(&[operation, 1])?;
            } else {
                let zero = source_address_original_operation_v29(
                    pending,
                    graph,
                    origin.site.instance,
                    receipt.block,
                    operation,
                    budget,
                )?;
                let (OperationKind::Constant(Constant::Index(0)), [offset]) =
                    (&zero.kind, zero.results.as_slice())
                else {
                    return Err(source_raw_physical_error_v29());
                };
                SourceAddressMemoryV29::same_type(&offset.ty, &Type::INDEX, budget)?;
                operation = argument_sum_v1(&[operation, 1])?;
                let gep = source_address_original_operation_v29(
                    pending,
                    graph,
                    origin.site.instance,
                    receipt.block,
                    operation,
                    budget,
                )?;
                let (
                    OperationKind::GetElementPointer {
                        base,
                        offset: actual,
                    },
                    [result],
                ) = (&gep.kind, gep.results.as_slice())
                else {
                    return Err(source_raw_physical_error_v29());
                };
                budget.charge_work(3)?;
                if *base != current || *actual != offset.id {
                    return Err(source_raw_physical_error_v29());
                }
                graph.check_zero_gep(gep, budget)?;
                current = result.id;
                operation = argument_sum_v1(&[operation, 1])?;
            }
        }
        if operation != receipt.end || current != receipt.result || current == receipt.base {
            return Err(source_raw_physical_error_v29());
        }
        check_source_address_destination_v29(
            instances,
            pending,
            source_index,
            graph,
            receipt,
            assign.destination(),
            origin.pointer_type,
            budget,
        )?;
        let position = receipt
            .end
            .checked_sub(1)
            .ok_or_else(source_raw_physical_error_v29)?;
        let mut mapping = ScopedEmittedPointsV29 {
            coordinates: &pending.coordinates,
            relocation: &pending.slot_relocation,
            budget,
        };
        let (block, operation) = mapping
            .emitted_point(
                origin.site.instance,
                receipt.block,
                u32::try_from(position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                false,
            )
            .map_err(source_address_point_error_v29)?
            .ok_or_else(source_raw_physical_error_v29)?;
        budget.charge_work(1)?;
        births.push(SourceAddressBirthV29 {
            block,
            operation: operation as usize,
            result: current,
        });
    }
    call_splice_sort_work_v1(argument_product_v1(births.len(), 3)?, budget)
        .map_err(source_address_call_error_v29)?;
    births.sort_unstable_by_key(|row| (row.block, row.operation, row.result));
    budget.charge_work(births.len())?;
    if births.windows(2).any(|pair| {
        (pair[0].block, pair[0].operation) == (pair[1].block, pair[1].operation)
            && pair[0].result != pair[1].result
    }) {
        return Err(source_raw_physical_error_v29());
    }
    budget.charge_work(births.len())?;
    births.dedup();
    Ok(births)
}

// Safe-reference SSA bindings retain a source loan, not a raw Value binding.
// Join that exact loan to the original read before accepting its physical ID.
fn check_source_cell_dereference_payload_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    binding: &SemanticSourceReferenceBindingV29,
    pointer: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    references.check(budget)?;
    let result = (|| {
        let plan = references.plan;
        source_reference_validate_binding_v29(plan, binding, budget)?;
        let access = source_reference_access_at_v29(
            plan,
            site,
            place,
            SourceReferenceAccessV29::Read,
            budget,
        )?;
        let loan = binding.origin.single_loan()?;
        let (_, cell) = plan
            .scalar_cell(loan, budget)?
            .ok_or_else(source_raw_physical_error_v29)?;
        budget.source_reference_charge_v29(plan, 10)?;
        let declaration = plan
            .instances
            .instance(site.instance)
            .and_then(|instance| {
                instance
                    .declaration()
                    .locals()
                    .get(place.local().index() as usize)
            })
            .ok_or_else(source_raw_physical_error_v29)?;
        if access.loan != Some(loan)
            || access.instance != cell.instance
            || access.local != cell.local
            || access.generation != cell.generation
            || access.ty != cell.ty
            || !access.projections.is_empty()
            || declaration.ty() != binding.source_type
            || !matches!(binding.values.as_slice(), [value]
                if value.id == pointer && matches!(value.ty, Type::Pointer(_)))
        {
            return Err(source_raw_physical_error_v29());
        }
        Ok(())
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(references.plan, error))
}

#[cfg(test)]
fn source_address_payload_row_scratch_v43(budget: &ArgumentBudgetV1<'_>, query_floor: usize) {
    let reclaimed = budget
        .storage()
        .checked_sub(query_floor)
        .expect("payload row undercut its query frame");
    let (calls, bytes, largest) =
        scoped_raw_admission_v29::SOURCE_OBJECT_PAYLOAD_ROW_SCRATCH_V29.get();
    scoped_raw_admission_v29::SOURCE_OBJECT_PAYLOAD_ROW_SCRATCH_V29.set((
        calls.checked_add(1).unwrap(),
        bytes.checked_add(reclaimed).unwrap(),
        largest.max(reclaimed),
    ));
}

fn check_source_address_payloads_v29(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    source_index: &SourceAddressSourceIndexV29<'_>,
    graph: &SourceAddressMemoryV29<'_>,
    rows: &[SourceAddressAccessSourceV29],
    payload_index: &SourceObjectPayloadIndexV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<Option<ScopedMemoryFrameV29>>(budget)?;
    source_reference_emission_prepay_v29::<Option<&SemanticPlaceV1>>(budget)?;
    budget.reserve_storage(source_address_tag_query_headers_v43()?)?;
    for source in rows {
        // Each row is a closed validation query. Its temporary paid envelopes
        // must die here, not accumulate until the complete access census ends.
        #[cfg(test)]
        let row_floor = budget.storage();
        scoped_raw_admission_v29::source_object_activation_scratch_v29(
            instances,
            references.plan,
            budget,
            |budget| {
                #[cfg(test)]
                let query_floor = budget.storage();
                budget.charge_work(5)?;
                let sidecar = source_index.sidecar(source.instance, budget)?;
                let original = instances
                    .instance(source.instance)
                    .ok_or_else(source_raw_physical_error_v29)?
                    .declaration();
                let occurrences = instances
                    .occurrences(source.instance)
                    .ok_or_else(source_raw_physical_error_v29)?;
                let archive = sidecar
                    .execution_observation
                    .as_ref()
                    .ok_or_else(execution_archive_error_v29)?;
                archive.check_original_v29(instances, source.instance, budget)?;
                let recorded = sidecar
                    .scoped_memory_anchors
                    .as_ref()
                    .ok_or_else(source_raw_physical_error_v29)?;
                let anchors = &recorded.rows;
                let row = anchors
                    .get(source.anchor)
                    .ok_or_else(source_raw_physical_error_v29)?;
                if check_source_address_tag_payload_v43(
                    instances,
                    references,
                    slots,
                    source_index,
                    graph,
                    source,
                    recorded,
                    row,
                    budget,
                )? {
                    #[cfg(test)]
                    source_address_payload_row_scratch_v43(budget, query_floor);
                    return Ok(());
                }
                let object = source_address_object_payload_v29(recorded, row, budget)?;
                let (pointer, payload) = match (row.kind, object) {
                    (
                        ScopedMemoryAnchorKindV29::Access {
                            pointer,
                            payload: Some(payload),
                        },
                        None,
                    ) => (pointer, payload),
                    (ScopedMemoryAnchorKindV29::Object(_), Some((_, payload))) => {
                        let object = recorded.object_payload(row, budget)?;
                        let pointer = object.operands()[0].ok_or_else(scoped_object_error_v29)?;
                        recorded.check_object_source(
                            original,
                            &occurrences,
                            source.anchor,
                            row,
                            object,
                            budget,
                        )?;
                        (pointer, payload)
                    }
                    _ => return Err(source_raw_physical_error_v29()),
                };
                let operation = graph.blocks[graph.block(source.physical.block, budget)?]
                    .1
                    .operations
                    .get(source.physical.operation)
                    .ok_or_else(source_raw_physical_error_v29)?;
                if source_address_value_access_v29(operation)?.map(|access| access.pointer)
                    != Some(pointer)
                    || graph.exact(pointer, budget)? != Some(source.physical.slot)
                {
                    return Err(source_raw_physical_error_v29());
                }
                if object.is_some() {
                    recorded
                        .object_payload(row, budget)?
                        .check_operation(operation, budget)?;
                    if let Some(expected) = source_static_object_expected_location_v29(
                        instances,
                        references.plan,
                        source_index,
                        slots,
                        object.unwrap().0,
                        budget,
                    )? && graph.object_location(pointer, budget)? != expected
                    {
                        return Err(scoped_object_error_v29());
                    }
                    check_source_object_holder_value_v29(
                        instances,
                        references.plan,
                        source_index,
                        source,
                        graph,
                        recorded,
                        payload_index,
                        object.unwrap().0,
                        pointer,
                        budget,
                    )?;
                } else {
                    check_scoped_payload_v29(original, &occurrences, row, operation, budget)?;
                    check_scoped_array_initializer_recipe_v29(
                        original,
                        row,
                        &sidecar.private_arrays.effects,
                        budget,
                    )?;
                    budget.charge_work(4)?;
                    if matches!(payload, ScopedMemoryPayloadV29::Load { read, .. } if read.prefix > 1)
                        || matches!(payload, ScopedMemoryPayloadV29::Store { .. })
                            && row
                                .source
                                .and_then(|frame| match frame.role {
                                    Some(ScopedMemoryRoleV29::Operand(role)) => {
                                        scoped_source_place_v29(original, frame.site, role)
                                    }
                                    _ => None,
                                })
                                .is_some_and(|place| place.projections().len() > 1)
                    {
                        check_source_scalar_loan_holder_v29(
                            instances,
                            references.plan,
                            source_index,
                            source,
                            row,
                            payload,
                            payload_index,
                            pointer,
                            budget,
                        )?;
                    }
                }
                match payload {
                    ScopedMemoryPayloadV29::IndexLoad { .. } => {
                        // The retained occurrence is authenticated above. Its actual
                        // incoming memory version is still a final-census obligation.
                    }
                    ScopedMemoryPayloadV29::Load { read, .. } => {
                        // A promoted holder's actual dereference must use that exact
                        // original definition, not another same-typed physical value.
                        budget.charge_work(4)?;
                        // Typed endpoints were independently rejoined for both reads
                        // and writes by the exact original holder query above.
                        if object.is_none()
                            && read.prefix == 1
                            && scoped_payload_place_v29(original, read.site, read.role)
                                .and_then(|place| place.projections().first())
                                .is_some_and(|projection| {
                                    matches!(
                                        projection.kind(),
                                        SemanticProjectionKindV1::Dereference
                                    )
                                })
                            && let ScopedMemoryOccurrenceV29::Promoted { definition, .. } =
                                read.occurrence
                        {
                            let binding = archive.lookup_original_v29(
                                instances,
                                source.instance,
                                definition,
                                budget,
                            )?;
                            match binding {
                                SemanticValueBindingV1::Value { id, ty }
                                    if *id == pointer && matches!(ty, Type::Pointer(_)) => {}
                                SemanticValueBindingV1::SourceReference(binding) => {
                                    let place =
                                        scoped_payload_place_v29(original, read.site, read.role)
                                            .ok_or_else(source_raw_physical_error_v29)?;
                                    let (block, statement) = scoped_memory_site_key_v29(read.site);
                                    check_source_cell_dereference_payload_v29(
                                        references,
                                        SourceReferenceSiteV29 {
                                            instance: source.instance,
                                            block: SemanticBlockIdV1::from_index(block),
                                            statement: statement.map(|value| value as usize),
                                        },
                                        place,
                                        binding,
                                        pointer,
                                        budget,
                                    )?;
                                }
                                _ => return Err(source_raw_physical_error_v29()),
                            }
                        }
                    }
                    ScopedMemoryPayloadV29::Store {
                        value,
                        source:
                            ScopedMemoryStoreSourceV29::Operand {
                                site,
                                role,
                                ty,
                                source: operand,
                            },
                    } => {
                        match operand {
                            ScopedMemoryOperandSourceV29::Place(occurrence) => {
                                let place = scoped_payload_place_v29(original, site, role)
                                    .ok_or_else(source_raw_physical_error_v29)?;
                                if instances
                                    .owner()
                                    .source_semantic()
                                    .types()
                                    .get(ty.index() as usize)
                                    .is_some_and(source_object_reference_field_v44)
                                {
                                    let (endpoint, _) = object
                                        .ok_or_else(source_object_reference_payload_error_v44)?;
                                    if endpoint.projected_type != ty || place.ty() != ty {
                                        return Err(source_object_reference_payload_error_v44());
                                    }
                                    let binding = source_object_archived_reference_v44(
                                        references.plan,
                                        archive,
                                        source.instance,
                                        site,
                                        role,
                                        place,
                                        occurrence,
                                        budget,
                                    )?;
                                    let expected = source_object_reference_value_v44(
                                        references.plan,
                                        binding,
                                        endpoint.projected_schema,
                                        budget,
                                    )?;
                                    if expected.id != value
                                        || !invocation_equal_types_v1(
                                            &expected.ty,
                                            graph.ty(value, budget)?,
                                            budget,
                                        )?
                                    {
                                        return Err(source_object_reference_payload_error_v44());
                                    }
                                } else if object.is_some()
                                    && matches!(
                                        occurrence,
                                        ScopedMemoryOccurrenceV29::Retained { .. }
                                    )
                                {
                                    check_source_object_stored_read_v29(
                                        source_index,
                                        source,
                                        graph,
                                        recorded,
                                        payload_index,
                                        place,
                                        site,
                                        role,
                                        occurrence,
                                        value,
                                        budget,
                                    )?;
                                } else {
                                    check_scoped_payload_archive_v29(
                                        &archive.bindings,
                                        place,
                                        occurrence,
                                        value,
                                        budget,
                                    )?;
                                }
                            }
                            ScopedMemoryOperandSourceV29::Memory { occurrence, access } => {
                                check_scoped_payload_memory_v29(
                                    original,
                                    recorded,
                                    source.anchor,
                                    row,
                                    value,
                                    site,
                                    role,
                                    ty,
                                    occurrence,
                                    access,
                                    budget,
                                )?;
                                let prior = anchors
                                    .get(access)
                                    .ok_or_else(source_raw_physical_error_v29)?;
                                let previous = source_address_original_operation_v29(
                                    source_index.pending,
                                    graph,
                                    source.instance,
                                    prior.block,
                                    prior.position,
                                    budget,
                                )?;
                                if !matches!(
                                    previous.kind,
                                    OperationKind::Load { .. }
                                        | OperationKind::Storage(
                                            ScopedObjectOperationV29::ReadValue { .. }
                                        )
                                ) || !matches!(previous.results.as_slice(), [result] if result.id == value)
                                {
                                    return Err(source_raw_physical_error_v29());
                                }
                            }
                            ScopedMemoryOperandSourceV29::Constant => {
                                // Exact constant/expression equivalence is retained as
                                // a source consumer obligation, not pointer provenance.
                                if matches!(graph.ty(value, budget)?, Type::Pointer(_)) {
                                    return Err(source_reference_error_v29(
                                        "raw pointer constants require admitted source provenance",
                                    ));
                                }
                            }
                        }
                    }
                    ScopedMemoryPayloadV29::Store {
                        source:
                            ScopedMemoryStoreSourceV29::Assignment { .. }
                            | ScopedMemoryStoreSourceV29::AssignmentComponent { .. }
                            | ScopedMemoryStoreSourceV29::CallResult { .. }
                            | ScopedMemoryStoreSourceV29::EntryArgument { .. },
                        ..
                    } => {
                        // Original formation/call and parameter censuses provide the
                        // producer relation; this row alone grants no source equation.
                    }
                }
                #[cfg(test)]
                source_address_payload_row_scratch_v43(budget, query_floor);
                Ok(())
            },
        )?;
        #[cfg(test)]
        assert_eq!(
            budget.storage(),
            row_floor,
            "payload row retained query scratch"
        );
    }
    Ok(())
}

fn check_source_address_destination_v29(
    instances: &ExecutionInstancesV29<'_>,
    pending: &PendingScopedRootEmissionV29,
    source_index: &SourceAddressSourceIndexV29<'_>,
    graph: &SourceAddressMemoryV29<'_>,
    receipt: SourceRawFormationReceiptV29,
    destination: &SemanticPlaceV1,
    result_type: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if !destination.projections().is_empty() || destination.ty() != result_type {
        return Err(source_raw_physical_error_v29());
    }
    if !std::ptr::eq(source_index.pending, pending) {
        return Err(source_raw_physical_error_v29());
    }
    let sidecar = source_index.sidecar(receipt.site.instance, budget)?;
    let archive = sidecar
        .execution_observation
        .as_ref()
        .ok_or_else(execution_archive_error_v29)?;
    archive.check_original_v29(instances, receipt.site.instance, budget)?;
    let occurrences = instances
        .occurrences(receipt.site.instance)
        .ok_or_else(execution_archive_error_v29)?;
    let site = execution_site_v29(
        receipt.site.block,
        Some(
            u32::try_from(
                receipt
                    .site
                    .statement
                    .ok_or_else(source_raw_physical_error_v29)?,
            )
            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
        ),
    );
    let mut definition = None;
    for row in occurrences.events() {
        budget.charge_work(3)?;
        if row.site() == site
            && row.role() == ExecutionEventV29::DestinationDefine
            && row.event().variable().get() == destination.local().index()
        {
            if definition.replace(row).is_some() || !row.is_reachable() {
                return Err(source_raw_physical_error_v29());
            }
        }
    }
    let definition = definition.ok_or_else(source_raw_physical_error_v29)?;
    if let Some(SsaResolvedEventV1::Define { variable, value }) = definition.resolved() {
        if !definition.is_promoted() || variable.get() != destination.local().index() {
            return Err(source_raw_physical_error_v29());
        }
        let SemanticValueBindingV1::Value { id, ty } =
            archive.lookup_original_v29(instances, receipt.site.instance, value, budget)?
        else {
            return Err(source_raw_physical_error_v29());
        };
        if *id != receipt.result {
            return Err(source_raw_physical_error_v29());
        }
        SourceAddressMemoryV29::same_type(ty, graph.ty(*id, budget)?, budget)?;
    } else {
        if definition.is_promoted() || definition.resolved().is_some() {
            return Err(source_raw_physical_error_v29());
        }
        let anchors = sidecar
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(scoped_memory_error_v29)?;
        let mut found = 0;
        for row in &anchors.rows {
            budget.charge_work(3)?;
            let payload = match row.kind {
                ScopedMemoryAnchorKindV29::Access { payload, .. } => payload,
                ScopedMemoryAnchorKindV29::Object(_) => {
                    source_address_object_payload_v29(anchors, row, budget)?
                        .map(|(_, payload)| payload)
                }
                _ => None,
            };
            if let Some(ScopedMemoryPayloadV29::Store {
                value,
                source: ScopedMemoryStoreSourceV29::Assignment { site: actual, ty },
            }) = payload
                && actual == site
                && ty == result_type
            {
                if value != receipt.result {
                    return Err(source_raw_physical_error_v29());
                }
                let operation = source_address_original_operation_v29(
                    pending,
                    graph,
                    receipt.site.instance,
                    row.block,
                    row.position,
                    budget,
                )?;
                if !matches!(operation.kind, OperationKind::Store { value: actual, .. }
                    | OperationKind::Storage(ScopedObjectOperationV29::WriteValue { value: actual, .. }) if actual == value)
                {
                    return Err(source_raw_physical_error_v29());
                }
                found = argument_sum_v1(&[found, 1])?;
            }
        }
        if found != 1 {
            return Err(source_raw_physical_error_v29());
        }
    }
    Ok(())
}
