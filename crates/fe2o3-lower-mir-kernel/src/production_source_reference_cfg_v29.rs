// Finite source-reference dataflow. Nodes retain source identities, not scalar
// values. Different runtime referent choices need checked addressable transport.
struct SourceReferenceCfgStateV29 {
    frames: Vec<(usize, usize)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceFunctionOutcomeV29 {
    Returned(usize),
    NoNormalReturn,
}

struct SourceReferenceCallSummaryV29 {
    arguments: Option<Vec<usize>>,
    before: SourceReferenceCfgStateV29,
    after: Option<SourceReferenceCfgStateV29>,
    result: SourceReferenceFunctionOutcomeV29,
    reuse_count: usize,
    evaluations: usize,
}

struct SourceReferenceCfgIndexV29 {
    successors: Vec<SourceReferenceCfgEdgeV29>,
    successor_ranges: Vec<std::ops::Range<usize>>,
    predecessors: Vec<usize>,
    predecessor_ranges: Vec<std::ops::Range<usize>>,
    reachable: Vec<bool>,
}

#[derive(Clone, Copy)]
struct SourceReferenceCfgEdgeV29 {
    target: usize,
    ordinal: usize,
    role: SemanticEdgeRoleV1,
}

#[derive(Clone, Copy)]
enum SourceReferenceTerminatorOutcomeV29 {
    Continue,
    Returned(usize),
    Switch(usize),
}

fn source_reference_cfg_return_headers_v29<T>() -> Result<usize, ArgumentResourceV1> {
    argument_product_v1(
        2,
        std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
    )
}

fn source_reference_cfg_call_headers_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<SourceReferenceCallSummaryV29>(),
        std::mem::size_of::<Option<SourceReferenceCallSummaryV29>>(),
        argument_product_v1(2, std::mem::size_of::<Option<Vec<usize>>>())?,
    ])
}

fn source_reference_cfg_run_headers_v29() -> Result<usize, ArgumentResourceV1> {
    argument_product_v1(2, std::mem::size_of::<Option<SourceReferenceCfgStateV29>>())
}

fn source_reference_cfg_exit_headers_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<Option<usize>>(),
        argument_product_v1(4, std::mem::size_of::<usize>())?,
        source_reference_cfg_return_headers_v29::<()>()?,
    ])
}

fn source_reference_cfg_obligation_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29(
        "source reference control-flow choice requires checked addressable holder state and writeback",
    )
}

fn source_reference_call_returns_v29(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    block: SemanticBlockIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    use production_call_instances_v1::ProductionCallControlV1;
    budget.charge_work(2)?;
    match instances.call_control(production_call_instances_v1::ProductionCallOccurrenceV1 {
        caller: instance,
        block,
    }) {
        Some(ProductionCallControlV1::MayReturn) => Ok(true),
        Some(ProductionCallControlV1::NoNormalReturn) => Ok(false),
        _ => Err(source_reference_cfg_obligation_v29()),
    }
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn next_effect_ordinal(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        let ordinal = self.effect_ordinal;
        self.effect_ordinal = argument_sum_v1(&[ordinal, 1])?;
        Ok(ordinal)
    }

    fn record_effect(
        &mut self,
        loan: usize,
        effect: SourceReferenceEffectV29,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let site = self
            .effect_site
            .ok_or_else(source_reference_cfg_obligation_v29)?;
        let tag = match effect {
            SourceReferenceEffectV29::ReadReferent => 0,
            SourceReferenceEffectV29::WriteReferent => 1,
            SourceReferenceEffectV29::ReadPayload => 2,
            SourceReferenceEffectV29::WritePayload => 3,
            SourceReferenceEffectV29::ObserveAddress => 4,
        };
        let key = (
            site.instance.index(),
            site.block.index(),
            site.statement,
            ordinal,
            loan,
            tag,
        );
        charge_execution_cfg_lookup_v29(self.effect_sites.len(), budget)?;
        if self.effect_sites.contains(&key) {
            return Ok(());
        }
        reserve_execution_cfg_map_entry_v29::<(usize, u32, Option<usize>, usize, usize, u8), ()>(
            self.effect_sites.len(),
            budget,
        )?;
        self.effect_sites.insert(key);
        let effects = &mut self.plan.loans[loan].effects;
        let counter = match effect {
            SourceReferenceEffectV29::ReadReferent => &mut effects.referent_reads,
            SourceReferenceEffectV29::WriteReferent => &mut effects.referent_writes,
            SourceReferenceEffectV29::ReadPayload => &mut effects.payload_reads,
            SourceReferenceEffectV29::WritePayload => &mut effects.payload_writes,
            SourceReferenceEffectV29::ObserveAddress => {
                self.plan.address_observed = true;
                &mut effects.address_observations
            }
        };
        *counter = argument_sum_v1(&[*counter, 1])?;
        Ok(())
    }

    fn cfg_index(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceCfgIndexV29, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<SourceReferenceCfgIndexV29>(),
            source_reference_cfg_return_headers_v29::<SourceReferenceCfgIndexV29>()?,
        ])?)?;
        let row = self
            .plan
            .instances
            .instance(instance)
            .ok_or_else(source_reference_cfg_obligation_v29)?;
        let blocks = row.declaration().blocks();
        let count = blocks.len();
        let mut successor_ranges = source_reference_scratch_v29(count, budget)?;
        let mut predecessor_counts = source_reference_scratch_v29(count, budget)?;
        let mut reachable = source_reference_scratch_v29(count, budget)?;
        let mut epochs = source_reference_scratch_v29(count, budget)?;
        budget.charge_work(argument_product_v1(count, 2)?)?;
        predecessor_counts.resize(count, 0usize);
        for block in 0..count {
            reachable.push(
                self.plan
                    .instances
                    .block_reachable(instance, SemanticBlockIdV1::from_index(block as u32))
                    .ok_or_else(source_reference_cfg_obligation_v29)?,
            );
        }
        let mut successors = source_reference_scratch_v29(0, budget)?;
        let mut epoch = 1usize;
        for (block, declaration) in blocks.iter().enumerate() {
            budget.charge_work(2)?;
            epochs.push(epoch);
            epoch = argument_sum_v1(&[epoch, declaration.statements().len()])?;
            if epoch >= u32::MAX as usize {
                return Err(ArgumentResourceV1::Arithmetic.into());
            }
            let first = successors.len();
            if reachable[block] {
                let mut ordinal = 0;
                declaration.terminator().kind().try_for_each_edge(|edge| {
                    budget.charge_work(3)?;
                    let occurrence = ordinal;
                    ordinal = argument_sum_v1(&[ordinal, 1])?;
                    if edge.role() == SemanticEdgeRoleV1::CallReturn
                        && !source_reference_call_returns_v29(
                            self.plan.instances,
                            instance,
                            SemanticBlockIdV1::from_index(block as u32),
                            budget,
                        )?
                    {
                        return Ok(());
                    }
                    let target = edge.target().index() as usize;
                    if reachable.get(target) != Some(&true) {
                        return Err(source_reference_cfg_obligation_v29());
                    }
                    emission_push_v1(&mut successors, SourceReferenceCfgEdgeV29 {
                        target, ordinal: occurrence, role: edge.role(),
                    }, budget)?;
                    predecessor_counts[target] = argument_sum_v1(&[predecessor_counts[target], 1])?;
                    Ok::<_, ProductionSemanticKirErrorV1>(())
                })?;
            }
            successor_ranges.push(first..successors.len());
        }
        let mut predecessors = source_reference_scratch_v29(successors.len(), budget)?;
        let mut predecessor_ranges = source_reference_scratch_v29(count, budget)?;
        let mut positions = source_reference_scratch_v29(count, budget)?;
        budget.charge_work(successors.len())?;
        predecessors.resize(successors.len(), 0);
        let mut next = 0;
        for count in &predecessor_counts {
            budget.charge_work(2)?;
            let end = argument_sum_v1(&[next, *count])?;
            predecessor_ranges.push(next..end);
            positions.push(next);
            next = end;
        }
        for (block, range) in successor_ranges.iter().enumerate() {
            for edge in &successors[range.clone()] {
                let target = edge.target;
                budget.charge_work(3)?;
                let slot = positions[target];
                *predecessors
                    .get_mut(slot)
                    .ok_or_else(source_reference_cfg_obligation_v29)? = block;
                positions[target] = argument_sum_v1(&[slot, 1])?;
            }
        }
        self.epochs[instance.index()] = epochs;
        Ok(SourceReferenceCfgIndexV29 {
            successors,
            successor_ranges,
            predecessors,
            predecessor_ranges,
            reachable,
        })
    }

    fn cfg_capture(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceCfgStateV29, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<SourceReferenceCfgStateV29>(),
            source_reference_cfg_return_headers_v29::<SourceReferenceCfgStateV29>()?,
        ])?)?;
        let mut frames = source_reference_scratch_v29(self.frames.len(), budget)?;
        for (instance, state) in self.frames.iter().enumerate() {
            budget.charge_work(1)?;
            if let Some(state) = state {
                frames.push((instance, *state));
            }
        }
        Ok(SourceReferenceCfgStateV29 { frames })
    }

    fn cfg_clone(
        &mut self,
        source: &SourceReferenceCfgStateV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceCfgStateV29, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<SourceReferenceCfgStateV29>(),
            source_reference_cfg_return_headers_v29::<SourceReferenceCfgStateV29>()?,
        ])?)?;
        let mut frames = source_reference_scratch_v29(source.frames.len(), budget)?;
        for &(instance, state) in &source.frames {
            budget.charge_work(1)?;
            frames.push((instance, self.clone_state(state, budget)?));
        }
        Ok(SourceReferenceCfgStateV29 { frames })
    }

    fn cfg_project_return_continuation(
        &self,
        instance: ProductionCallInstanceIdV1,
        captured: &mut SourceReferenceCfgStateV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.reserve_storage(source_reference_cfg_exit_headers_v29()?)?;
        let slots = self.frames.len();
        let count = captured.frames.len();
        // All original instance slots are checked, not just the supplied rows.
        // Prepay the full scan and worst-case removal shift before publication.
        budget.charge_work(argument_sum_v1(&[
            6,
            argument_product_v1(7, slots)?,
            count,
        ])?)?;
        if slots != self.plan.instances.instances().len()
            || self.plan.instances.instance_reachable(instance) != Some(true)
            || self.frames.get(instance.index()).is_none_or(Option::is_none)
        {
            return Err(source_reference_cfg_obligation_v29());
        }
        let mut cursor = 0usize;
        let mut exiting = None;
        for (ordinal, state) in self.frames.iter().enumerate() {
            let Some(state) = state else { continue };
            let original = self.plan.instances.id_at(ordinal)
                .ok_or_else(source_reference_cfg_obligation_v29)?;
            if self.plan.instances.instance_reachable(original) != Some(true)
                || captured.frames.get(cursor) != Some(&(ordinal, *state))
                || self.plan.states.get(*state).is_none()
            {
                return Err(source_reference_cfg_obligation_v29());
            }
            if original == instance {
                exiting = Some(cursor);
            }
            cursor += 1;
        }
        if cursor != count {
            return Err(source_reference_cfg_obligation_v29());
        }
        let exiting = exiting.ok_or_else(source_reference_cfg_obligation_v29)?;
        captured.frames.remove(exiting);
        Ok(())
    }

    fn cfg_snapshot(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceCfgStateV29, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(source_reference_cfg_return_headers_v29::<
            SourceReferenceCfgStateV29,
        >()?)?;
        let current = self.cfg_capture(budget)?;
        self.cfg_clone(&current, budget)
    }

    fn cfg_install(
        &mut self,
        source: &SourceReferenceCfgStateV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(self.frames.len())?;
        self.frames.fill(None);
        for &(instance, state) in &source.frames {
            budget.charge_work(1)?;
            let target = self
                .frames
                .get_mut(instance)
                .ok_or_else(source_reference_cfg_obligation_v29)?;
            if target.replace(state).is_some() {
                return Err(source_reference_cfg_obligation_v29());
            }
        }
        Ok(())
    }

    fn nodes_equal(
        &self,
        left: usize,
        right: usize,
        depth: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        if left == right {
            return Ok(true);
        }
        if depth >= 256 {
            return Err(source_reference_cfg_obligation_v29());
        }
        let left = self
            .plan
            .nodes
            .get(left)
            .ok_or_else(source_reference_cfg_obligation_v29)?;
        let right = self
            .plan
            .nodes
            .get(right)
            .ok_or_else(source_reference_cfg_obligation_v29)?;
        if left.ty != right.ty || left.descriptor != right.descriptor {
            return Ok(false);
        }
        if !self.storage_values_equal(left.storage, right.storage, budget)? {
            return Ok(false);
        }
        match (left.kind, right.kind) {
            (SourceReferenceNodeKindV29::Absent, SourceReferenceNodeKindV29::Absent) => {
                self.inactive_sets_equal(left.inactive, right.inactive, budget)
            }
            (SourceReferenceNodeKindV29::Plain(a), SourceReferenceNodeKindV29::Plain(b)) => {
                Ok(a == b)
            }
            (SourceReferenceNodeKindV29::Loan(a), SourceReferenceNodeKindV29::Loan(b)) => {
                Ok(a == b)
            }
            (SourceReferenceNodeKindV29::Address(a), SourceReferenceNodeKindV29::Address(b)) => {
                Ok(a == b)
            }
            (SourceReferenceNodeKindV29::Discriminant(a), SourceReferenceNodeKindV29::Discriminant(b)) => {
                Ok(a == b)
            }
            (
                SourceReferenceNodeKindV29::Enum { first: a, count },
                SourceReferenceNodeKindV29::Enum { first: b, count: other },
            ) if count == other => {
                for offset in 0..count {
                    if self.plan.enum_member(a, count, offset, budget)?
                        != self.plan.enum_member(b, count, offset, budget)?
                    {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (SourceReferenceNodeKindV29::EnumView(a), SourceReferenceNodeKindV29::EnumView(b)) => {
                Ok(a == b)
            }
            (
                SourceReferenceNodeKindV29::Aggregate { first: a, count },
                SourceReferenceNodeKindV29::Aggregate {
                    first: b,
                    count: other,
                },
            ) if count == other => {
                for field in 0..count {
                    budget.charge_work(1)?;
                    if !self.nodes_equal(
                        self.plan.children[argument_sum_v1(&[a, field])?],
                        self.plan.children[argument_sum_v1(&[b, field])?],
                        depth + 1,
                        budget,
                    )? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn states_equal(
        &self,
        left: usize,
        right: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let left = &self.plan.states[left];
        let right = &self.plan.states[right];
        if left.len() != right.len() {
            return Ok(false);
        }
        for (left, right) in left.iter().zip(right) {
            budget.charge_work(2)?;
            if !self.storage_states_equal(left.storage, right.storage, budget)? {
                return Ok(false);
            }
            if left.generation != right.generation {
                return Ok(false);
            }
            match (left.node, right.node) {
                (Some(left), Some(right)) if self.nodes_equal(left, right, 0, budget)? => {}
                (None, None) => {}
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    fn cfg_merge(
        &mut self,
        into: &SourceReferenceCfgStateV29,
        other: &SourceReferenceCfgStateV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        if into.frames.len() != other.frames.len() {
            return Err(source_reference_cfg_obligation_v29());
        }
        let mut changed = false;
        for (&(left_instance, left), &(right_instance, right)) in
            into.frames.iter().zip(&other.frames)
        {
            budget.charge_work(2)?;
            if left_instance != right_instance {
                return Err(source_reference_cfg_obligation_v29());
            }
            if !self.states_equal(left, right, budget)? {
                let before = self.clone_state(left, budget)?;
                let instance = self
                    .plan
                    .instances
                    .id_at(left_instance)
                    .ok_or_else(source_reference_cfg_obligation_v29)?;
                self.merge_cfg_state(instance, left, right, budget)?;
                changed |= !self.states_equal(before, left, budget)?;
            }
        }
        Ok(changed)
    }

    fn merge_cfg_state(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        into: usize,
        other: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let count = self.plan.states[into].len();
        if count != self.plan.states[other].len() {
            return Err(source_reference_cfg_obligation_v29());
        }
        for local in 0..count {
            budget.charge_work(2)?;
            let left = self.plan.states[into][local];
            let right = self.plan.states[other][local];
            self.plan.states[into][local].storage =
                self.join_storage_states(left.storage, right.storage, budget)?;
            // Only the legacy no-C2 path lacks an independent lifetime query.
            // An absent value in a live C2 object does not erase its activation.
            if left.node.is_none()
                && right.node.is_none()
                && (left.storage.is_none() || right.storage.is_none())
            {
                self.plan.states[into][local].generation = if left.generation == right.generation {
                    left.generation
                } else {
                    u32::MAX
                };
                continue;
            }
            if left.generation != right.generation {
                let left_loan = if let Some(node) = left.node {
                    self.contains_loan(node, budget)?
                } else {
                    false
                };
                let right_loan = if let Some(node) = right.node {
                    self.contains_loan(node, budget)?
                } else {
                    false
                };
                if left.storage.is_none() || right.storage.is_none() || left_loan || right_loan {
                    return Err(source_reference_error_v29(
                        "source reference CFG storage generations differ",
                    ));
                }
                let local = SemanticLocalIdV1::from_index(
                    u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                );
                self.plan.states[into][local.index() as usize].generation = self
                    .join_storage_epochs(
                        instance,
                        local,
                        left.generation,
                        right.generation,
                        budget,
                    )?;
            }
            let node = match (left.node, right.node) {
                (Some(left), Some(right)) => Some(self.merge_node(left, right, budget)?),
                (Some(node), None) | (None, Some(node)) => {
                    if left.storage.is_some() && right.storage.is_some() {
                        // C2 denies a read unless both predecessors initialize
                        // the holder. Its possible loan remains live here.
                        self.plan.states[into][local].node = Some(node);
                        continue;
                    }
                    if self.contains_loan(node, budget)? {
                        return Err(source_reference_error_v29(
                            "source reference CFG holder liveness differs",
                        ));
                    }
                    None
                }
                (None, None) => None,
            };
            self.plan.states[into][local].node = node;
        }
        Ok(())
    }

    fn run_cfg(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        entry: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceFunctionOutcomeV29, ProductionSemanticKirErrorV1> {
        // One retained return option and one non-reentrant Some construction
        // slot. Input option cells themselves are paid in their vector backing.
        budget.reserve_storage(source_reference_cfg_run_headers_v29()?)?;
        let index = self.cfg_index(instance, budget)?;
        let function = self
            .plan
            .instances
            .instance(instance)
            .ok_or_else(source_reference_cfg_obligation_v29)?
            .declaration();
        let count = function.blocks().len();
        let mut inputs = source_reference_scratch_v29(count, budget)?;
        let mut queued = source_reference_scratch_v29(count, budget)?;
        let mut queue = source_reference_scratch_v29(count, budget)?;
        budget.charge_work(argument_product_v1(count, 3)?)?;
        inputs.resize_with(count, || None);
        queued.resize(count, false);
        queue.resize(count, 0usize);
        self.frames[instance.index()] = Some(entry);
        let first = function.entry().index() as usize;
        if index.reachable.get(first) != Some(&true) {
            return Err(source_reference_cfg_obligation_v29());
        }
        inputs[first] = Some(self.cfg_snapshot(budget)?);
        queue[0] = first;
        queued[first] = true;
        let mut head = 0usize;
        let mut length = 1usize;
        let mut result = None;
        let mut returns: Option<SourceReferenceCfgStateV29> = None;
        while length != 0 {
            budget.charge_work(5)?;
            let block = queue[head];
            head = (head + 1) % count;
            length -= 1;
            queued[block] = false;
            let input = inputs[block]
                .as_ref()
                .ok_or_else(source_reference_cfg_obligation_v29)?;
            let working = self.cfg_clone(input, budget)?;
            self.cfg_install(&working, budget)?;
            let block_id = SemanticBlockIdV1::from_index(block as u32);
            self.enter_selector_block(instance, block as u32, budget)?;
            let declaration = &function.blocks()[block];
            for (ordinal, statement) in declaration.statements().iter().enumerate() {
                budget.charge_work(1)?;
                let site = SourceReferenceSiteV29 {
                    instance,
                    block: block_id,
                    statement: Some(ordinal),
                };
                self.effect_site = Some(site);
                self.effect_ordinal = 0;
                self.statement(site, statement.kind(), budget)?;
            }
            let site = SourceReferenceSiteV29 {
                instance,
                block: block_id,
                statement: None,
            };
            self.effect_site = Some(site);
            self.effect_ordinal = 0;
            let outcome = self.terminator(site, declaration.terminator().kind(), budget)?;
            let mut output = self.cfg_capture(budget)?;
            if let SourceReferenceTerminatorOutcomeV29::Returned(value) = outcome {
                // Return already checked escapes and recorded its source value.
                // Only ancestor frames survive into the caller continuation.
                self.cfg_project_return_continuation(instance, &mut output, budget)?;
                result = Some(match result {
                    Some(previous) => self.merge_node(previous, value, budget)?,
                    None => value,
                });
                if let Some(previous) = &returns {
                    self.cfg_merge(previous, &output, budget)?;
                } else {
                    returns = Some(self.cfg_clone(&output, budget)?);
                }
            }
            for &edge in &index.successors[index.successor_ranges[block].clone()] {
                budget.charge_work(3)?;
                let target = edge.target;
                let refined = if let SourceReferenceTerminatorOutcomeV29::Switch(discriminant) = outcome {
                    self.refine_discriminant_edge(site, edge, discriminant, &output, budget)?
                } else { None };
                let edge_output = refined.as_ref().unwrap_or(&output);
                let changed = if let Some(previous) = &inputs[target] {
                    self.cfg_merge(previous, edge_output, budget)?
                } else {
                    inputs[target] = Some(self.cfg_clone(edge_output, budget)?);
                    true
                };
                if changed && !queued[target] {
                    if length == count {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    queue[(head + length) % count] = target;
                    length += 1;
                    queued[target] = true;
                }
            }
        }
        for (block, input) in inputs.iter().enumerate() {
            budget.charge_work(2)?;
            if !index.reachable[block] {
                continue;
            }
            let input = input
                .as_ref()
                .ok_or_else(source_reference_cfg_obligation_v29)?;
            budget.charge_work(input.frames.len())?;
            let state = input
                .frames
                .iter()
                .find(|(id, _)| *id == instance.index())
                .ok_or_else(source_reference_cfg_obligation_v29)?
                .1;
            self.retain_call_block_entry(
                instance,
                SemanticBlockIdV1::from_index(block as u32),
                state,
                budget,
            )?;
        }
        match (
            self.plan.instances.instance_may_return(instance),
            result,
            returns,
        ) {
            (Some(true), Some(value), Some(returns)) => {
                self.cfg_install(&returns, budget)?;
                self.frames[instance.index()] = None;
                Ok(SourceReferenceFunctionOutcomeV29::Returned(value))
            }
            (Some(false), None, None) => {
                self.frames[instance.index()] = None;
                Ok(SourceReferenceFunctionOutcomeV29::NoNormalReturn)
            }
            _ => Err(source_reference_cfg_obligation_v29()),
        }
    }

    fn cfg_clone_summary_after(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceCfgStateV29, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<SourceReferenceCfgStateV29>(),
            source_reference_cfg_return_headers_v29::<SourceReferenceCfgStateV29>()?,
        ])?)?;
        let count = self.summaries[instance.index()]
            .as_ref()
            .ok_or_else(source_reference_cfg_obligation_v29)?
            .after
            .as_ref()
            .ok_or_else(source_reference_cfg_obligation_v29)?
            .frames
            .len();
        let mut frames = source_reference_scratch_v29(count, budget)?;
        for index in 0..count {
            budget.charge_work(1)?;
            let (id, state) = self.summaries[instance.index()]
                .as_ref()
                .unwrap()
                .after
                .as_ref()
                .ok_or_else(source_reference_cfg_obligation_v29)?
                .frames[index];
            frames.push((id, self.clone_state(state, budget)?));
        }
        Ok(SourceReferenceCfgStateV29 { frames })
    }
}

include!("production_source_reference_call_transfer_v29.rs");
