//! Bounded SSA construction algorithms.

use std::collections::VecDeque;

use super::support::*;
use super::*;

pub(super) struct Planner<'a> {
    input: &'a SsaConstructionInputV1,
    limits: SsaPlannerLimitsV1,
    promotable_variables: Vec<SsaVariableIdV1>,
    promotable_indices: Vec<Option<usize>>,
    variable_words: usize,
    reachable: Vec<bool>,
    reverse_postorder: Vec<usize>,
    predecessors: Vec<Vec<usize>>,
    incoming_edges: Vec<usize>,
    definition_blocks_by_variable: Vec<Vec<usize>>,
    edge_definition_targets: Vec<Vec<usize>>,
    block_definitions: BitMatrix,
    block_uses: BitMatrix,
    live_in: BitMatrix,
    work: WorkBudget,
    storage_words: usize,
    input_edges: usize,
    input_events: usize,
    input_edge_definitions: usize,
}

impl<'a> Planner<'a> {
    pub(super) fn new(
        input: &'a SsaConstructionInputV1,
        limits: SsaPlannerLimitsV1,
    ) -> Result<Self, SsaPlannerErrorV1> {
        let mut work = WorkBudget::new(limits.max_work_units);
        let variable_count = input.variable_count as usize;
        require_resource(
            SsaPlannerResourceV1::Variables,
            variable_count,
            limits.max_variables,
        )?;
        if input.promotable.len() != variable_count {
            return Err(SsaPlannerErrorV1::PromotableLengthMismatch {
                variable_count,
                bitmap_len: input.promotable.len(),
            });
        }
        work.charge(input.promotable.len())?;
        let block_count = input.blocks.len();
        if block_count == 0 {
            return Err(SsaPlannerErrorV1::EmptyControlFlow);
        }
        require_resource(SsaPlannerResourceV1::Blocks, block_count, limits.max_blocks)?;
        if input.entry.get() as usize >= block_count {
            return Err(SsaPlannerErrorV1::InvalidEntry {
                entry: input.entry,
                block_count,
            });
        }
        work.charge(input.entry_definitions.len())?;
        validate_definition_order(&input.entry_definitions, None)?;
        work.charge(input.entry_definitions.len())?;
        for (index, variable) in input.entry_definitions.iter().copied().enumerate() {
            validate_variable(
                variable,
                variable_count,
                SsaInputSiteV1::EntryDefinition(index as u32),
            )?;
        }

        let mut input_edges = 0_usize;
        let mut input_events = 0_usize;
        let mut input_edge_definitions = 0_usize;
        let mut promotable_definition_events = 0_usize;
        for (block_index, block) in input.blocks.iter().enumerate() {
            work.charge(1)?;
            input_events = checked_add_resource(
                SsaPlannerResourceV1::Events,
                input_events,
                block.events.len(),
                limits.max_events,
            )?;
            work.charge(block.events.len())?;
            for (event_index, event) in block.events.iter().copied().enumerate() {
                validate_variable(
                    event.variable(),
                    variable_count,
                    SsaInputSiteV1::Event {
                        block: SsaBlockIdV1::new(block_index as u32),
                        event: event_index as u32,
                    },
                )?;
                if matches!(event, SsaEventV1::Define(_) | SsaEventV1::Kill(_))
                    && input.promotable[event.variable().get() as usize]
                {
                    promotable_definition_events = promotable_definition_events
                        .checked_add(1)
                        .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
                }
            }
            input_edges = checked_add_resource(
                SsaPlannerResourceV1::Edges,
                input_edges,
                block.edges.len(),
                limits.max_edges,
            )?;
            work.charge(block.edges.len())?;
            for (edge_index, edge) in block.edges.iter().enumerate() {
                let edge_id =
                    SsaEdgeIdV1::new(SsaBlockIdV1::new(block_index as u32), edge_index as u32);
                if edge.role.get() == 0 {
                    return Err(SsaPlannerErrorV1::InvalidEdgeRole { edge: edge_id });
                }
                if edge.target.get() as usize >= block_count {
                    return Err(SsaPlannerErrorV1::UnknownTarget {
                        edge: edge_id,
                        target: edge.target,
                        block_count,
                    });
                }
                work.charge(edge.definitions.len())?;
                validate_definition_order(&edge.definitions, Some(edge_id))?;
                input_edge_definitions = checked_add_resource(
                    SsaPlannerResourceV1::EdgeDefinitions,
                    input_edge_definitions,
                    edge.definitions.len(),
                    limits.max_edge_definitions,
                )?;
                work.charge(edge.definitions.len())?;
                for (definition_index, variable) in edge.definitions.iter().copied().enumerate() {
                    validate_variable(
                        variable,
                        variable_count,
                        SsaInputSiteV1::EdgeDefinition {
                            edge: edge_id,
                            definition: definition_index as u32,
                        },
                    )?;
                }
            }
        }

        let promotable_count = input
            .promotable
            .iter()
            .filter(|promotable| **promotable)
            .count();
        let variable_words = promotable_count.div_ceil(u64::BITS as usize);
        let base_matrix_words = block_count
            .checked_mul(variable_words)
            .and_then(|words| words.checked_mul(3))
            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
        let mut storage_words = base_matrix_words;

        // Charge fixed-capacity planner scratch and plan row tables before any
        // of them are allocated. Dynamic payloads are charged at their sites.
        let twice_blocks = checked_scale(block_count, 2)?;
        let twice_edges = checked_scale(input_edges, 2)?;
        let twice_edge_definitions = checked_scale(input_edge_definitions, 2)?;
        let blocks_and_edges = block_count
            .checked_add(input_edges)
            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
        let twice_blocks_and_edges = checked_scale(blocks_and_edges, 2)?;
        charge_storage_items::<bool>(&mut storage_words, block_count, limits.max_storage_words)?;
        charge_storage_items::<Vec<usize>>(
            &mut storage_words,
            block_count,
            limits.max_storage_words,
        )?;
        charge_storage_items::<usize>(&mut storage_words, twice_edges, limits.max_storage_words)?;
        charge_storage_items::<usize>(&mut storage_words, block_count, limits.max_storage_words)?;
        charge_storage_items::<Vec<usize>>(
            &mut storage_words,
            checked_scale(promotable_count, 2)?,
            limits.max_storage_words,
        )?;
        let promotable_entry_definitions = input
            .entry_definitions
            .iter()
            .filter(|variable| input.promotable[variable.get() as usize])
            .count();
        let definitions_and_entry = promotable_definition_events
            .checked_add(promotable_entry_definitions)
            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
        charge_storage_items::<usize>(
            &mut storage_words,
            definitions_and_entry,
            limits.max_storage_words,
        )?;
        charge_storage_items::<usize>(
            &mut storage_words,
            twice_edge_definitions,
            limits.max_storage_words,
        )?;
        charge_storage_items::<(usize, bool)>(
            &mut storage_words,
            twice_blocks_and_edges,
            limits.max_storage_words,
        )?;
        charge_storage_items::<SsaBlockIdV1>(
            &mut storage_words,
            twice_blocks,
            limits.max_storage_words,
        )?;
        charge_storage_items::<Option<usize>>(
            &mut storage_words,
            variable_count,
            limits.max_storage_words,
        )?;
        charge_storage_items::<SsaVariableIdV1>(
            &mut storage_words,
            checked_scale(promotable_count, 2)?,
            limits.max_storage_words,
        )?;

        // Persistent per-block plan rows.
        charge_storage_items::<Vec<SsaVariableIdV1>>(
            &mut storage_words,
            checked_scale(block_count, 3)?,
            limits.max_storage_words,
        )?;
        charge_storage_items::<Vec<(u32, SsaResolvedEventV1)>>(
            &mut storage_words,
            block_count,
            limits.max_storage_words,
        )?;
        charge_storage_items::<Vec<Vec<SsaArgumentV1>>>(
            &mut storage_words,
            block_count,
            limits.max_storage_words,
        )?;
        charge_storage_items::<Vec<SsaArgumentV1>>(
            &mut storage_words,
            input_edges,
            limits.max_storage_words,
        )?;

        // Phase scratch. Charging all phases cumulatively is conservative but
        // makes the public storage report a sound upper bound on residency.
        charge_storage_items::<bool>(
            &mut storage_words,
            checked_scale(block_count, 4)?,
            limits.max_storage_words,
        )?;
        charge_storage_items::<usize>(
            &mut storage_words,
            checked_scale(block_count, 8)?,
            limits.max_storage_words,
        )?;
        charge_storage_items::<Option<usize>>(
            &mut storage_words,
            block_count,
            limits.max_storage_words,
        )?;
        charge_storage_items::<Vec<usize>>(
            &mut storage_words,
            checked_scale(block_count, 2)?,
            limits.max_storage_words,
        )?;
        charge_storage_items::<u64>(
            &mut storage_words,
            checked_scale(variable_words, 3)?,
            limits.max_storage_words,
        )?;
        charge_storage_items::<Option<SsaValueV1>>(
            &mut storage_words,
            checked_scale(promotable_count, 2)?,
            limits.max_storage_words,
        )?;
        charge_storage_items::<u32>(
            &mut storage_words,
            promotable_count,
            limits.max_storage_words,
        )?;

        work.charge(input.promotable.len())?;
        let promotable_variables = input
            .promotable
            .iter()
            .enumerate()
            .filter_map(|(variable, promotable)| {
                promotable.then_some(SsaVariableIdV1::new(variable as u32))
            })
            .collect::<Vec<_>>();
        let mut promotable_indices = vec![None; variable_count];
        for (index, variable) in promotable_variables.iter().copied().enumerate() {
            promotable_indices[variable.get() as usize] = Some(index);
        }
        let mut definition_blocks_by_variable = vec![Vec::new(); promotable_count];
        work.charge(input.entry_definitions.len())?;
        for variable in input.entry_definitions.iter().copied() {
            if let Some(index) = promotable_indices[variable.get() as usize] {
                definition_blocks_by_variable[index].push(input.entry.get() as usize);
            }
        }

        Ok(Self {
            input,
            limits,
            promotable_variables,
            promotable_indices,
            variable_words,
            reachable: vec![false; block_count],
            reverse_postorder: Vec::new(),
            predecessors: vec![Vec::new(); block_count],
            incoming_edges: vec![0; block_count],
            definition_blocks_by_variable,
            edge_definition_targets: vec![Vec::new(); promotable_count],
            block_definitions: BitMatrix::try_new(block_count, variable_words)?,
            block_uses: BitMatrix::try_new(block_count, variable_words)?,
            live_in: BitMatrix::try_new(block_count, variable_words)?,
            work,
            storage_words,
            input_edges,
            input_events,
            input_edge_definitions,
        })
    }

    pub(super) fn build(mut self) -> Result<SsaConstructionPlanV1, SsaPlannerErrorV1> {
        self.compute_reachability_and_order()?;
        self.collect_facts()?;
        self.solve_liveness()?;
        let immediate_dominators = self.compute_immediate_dominators()?;
        let frontiers = self.compute_dominance_frontiers(&immediate_dominators)?;
        let merge_variables = self.compute_merge_variables(&frontiers)?;
        let transport_variables = self.compute_transport_variables(&merge_variables)?;
        let (
            entry_definitions,
            entry_arguments,
            resolved_events,
            edge_definitions,
            edge_arguments,
            generated_definitions,
            output_items,
        ) = self.resolve_values(&immediate_dominators, &transport_variables)?;

        let live_in = self.live_in_variables()?;
        let promoted_variables = self.promotable_variables.clone();
        let reverse_postorder = self
            .reverse_postorder
            .iter()
            .map(|block| SsaBlockIdV1::new(*block as u32))
            .collect::<Vec<_>>();
        let identity = compute_identity(
            self.input,
            &self.reachable,
            &live_in,
            &merge_variables,
            &transport_variables,
            &entry_definitions,
            &entry_arguments,
            &resolved_events,
            &edge_definitions,
            &edge_arguments,
            &mut self.work,
        )?;
        let reachable_blocks = self
            .reachable
            .iter()
            .filter(|reachable| **reachable)
            .count();
        Ok(SsaConstructionPlanV1 {
            identity,
            resources: SsaPlannerResourceReportV1 {
                input_blocks: self.input.blocks.len(),
                reachable_blocks,
                pruned_blocks: self.input.blocks.len() - reachable_blocks,
                input_edges: self.input_edges,
                input_events: self.input_events,
                input_edge_definitions: self.input_edge_definitions,
                generated_definitions,
                output_items,
                storage_words: self.storage_words,
                work_units: self.work.consumed,
            },
            reachable: self.reachable,
            reverse_postorder,
            promoted_variables,
            live_in,
            merge_variables,
            transport_variables,
            entry_definitions,
            entry_arguments,
            resolved_events,
            edge_definitions,
            edge_arguments,
        })
    }

    fn compute_reachability_and_order(&mut self) -> Result<(), SsaPlannerErrorV1> {
        let entry = self.input.entry.get() as usize;
        let mut stack = vec![(entry, false)];
        while let Some((block, finish)) = stack.pop() {
            self.work.charge(1)?;
            if finish {
                self.reverse_postorder.push(block);
                continue;
            }
            if self.reachable[block] {
                continue;
            }
            self.reachable[block] = true;
            stack.push((block, true));
            for edge in self.input.blocks[block].edges.iter().rev() {
                self.work.charge(1)?;
                let target = edge.target.get() as usize;
                if !self.reachable[target] {
                    stack.push((target, false));
                }
            }
        }
        self.reverse_postorder.reverse();
        self.work.charge(self.reverse_postorder.len())?;

        for (source, block) in self.input.blocks.iter().enumerate() {
            if !self.reachable[source] {
                continue;
            }
            for edge in &block.edges {
                self.work.charge(1)?;
                let target = edge.target.get() as usize;
                if !self.reachable[target] {
                    continue;
                }
                self.incoming_edges[target] = self.incoming_edges[target]
                    .checked_add(1)
                    .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
                self.predecessors[target].push(source);
            }
        }
        for predecessors in &mut self.predecessors {
            self.work.charge(predecessors.len())?;
            // Sources are visited in ascending order, so duplicates are
            // already adjacent and no comparison sort is necessary.
            predecessors.dedup();
        }
        Ok(())
    }

    fn collect_facts(&mut self) -> Result<(), SsaPlannerErrorV1> {
        for (block_index, block) in self.input.blocks.iter().enumerate() {
            if !self.reachable[block_index] {
                continue;
            }
            let mut defined = vec![0_u64; self.variable_words];
            for event in block.events.iter().copied() {
                self.work.charge(1)?;
                let Some(variable) = self.promotable_index(event.variable()) else {
                    continue;
                };
                match event {
                    SsaEventV1::Use(_) if !bit_contains(&defined, variable) => {
                        self.block_uses.insert(block_index, variable);
                    }
                    SsaEventV1::Define(_) | SsaEventV1::Kill(_) => {
                        bit_insert(&mut defined, variable);
                        self.block_definitions.insert(block_index, variable);
                        if self.definition_blocks_by_variable[variable].last().copied()
                            != Some(block_index)
                        {
                            self.definition_blocks_by_variable[variable].push(block_index);
                        }
                    }
                    SsaEventV1::Use(_) => {}
                }
            }
            for edge in &block.edges {
                let target = edge.target.get() as usize;
                for variable in edge.definitions.iter().copied() {
                    self.work.charge(1)?;
                    if let Some(variable) = self.promotable_index(variable) {
                        self.edge_definition_targets[variable].push(target);
                    }
                }
            }
        }
        Ok(())
    }

    fn solve_liveness(&mut self) -> Result<(), SsaPlannerErrorV1> {
        let mut queued = self.reachable.clone();
        let mut pending = self
            .reverse_postorder
            .iter()
            .rev()
            .copied()
            .collect::<VecDeque<_>>();
        let mut live_out = vec![0_u64; self.variable_words];
        let mut edge_live = vec![0_u64; self.variable_words];
        while let Some(block) = pending.pop_front() {
            queued[block] = false;
            self.work.charge(1 + self.variable_words)?;
            live_out.fill(0);
            for edge in &self.input.blocks[block].edges {
                self.work
                    .charge(self.variable_words + edge.definitions.len())?;
                let target = edge.target.get() as usize;
                if !self.reachable[target] {
                    continue;
                }
                for (word, edge_live_word) in edge_live.iter_mut().enumerate() {
                    *edge_live_word = self.live_in.word(target, word);
                }
                for variable in edge.definitions.iter().copied() {
                    if let Some(variable) = self.promotable_index(variable) {
                        bit_remove(&mut edge_live, variable);
                    }
                }
                for word in 0..self.variable_words {
                    live_out[word] |= edge_live[word];
                }
            }
            let mut changed = false;
            for (word, live_out_word) in live_out.iter().copied().enumerate() {
                let next = self.block_uses.word(block, word)
                    | (live_out_word & !self.block_definitions.word(block, word));
                changed |= self.live_in.set_word(block, word, next);
            }
            if changed {
                for predecessor in &self.predecessors[block] {
                    self.work.charge(1)?;
                    if !queued[*predecessor] {
                        queued[*predecessor] = true;
                        pending.push_back(*predecessor);
                    }
                }
            }
        }
        Ok(())
    }

    fn compute_immediate_dominators(&mut self) -> Result<Vec<Option<usize>>, SsaPlannerErrorV1> {
        let block_count = self.input.blocks.len();
        let entry = self.input.entry.get() as usize;
        let mut rpo_index = vec![usize::MAX; block_count];
        for (index, block) in self.reverse_postorder.iter().copied().enumerate() {
            rpo_index[block] = index;
        }
        let mut immediate = vec![None; block_count];
        immediate[entry] = Some(entry);
        loop {
            self.work.charge(1)?;
            let mut changed = false;
            for rpo_position in 1..self.reverse_postorder.len() {
                self.work.charge(1)?;
                let block = self.reverse_postorder[rpo_position];
                let mut processed = self.predecessors[block]
                    .iter()
                    .copied()
                    .filter(|predecessor| immediate[*predecessor].is_some());
                let Some(mut next) = processed.next() else {
                    continue;
                };
                for predecessor in processed {
                    self.work.charge(1)?;
                    next = intersect_dominator_paths(
                        predecessor,
                        next,
                        &immediate,
                        &rpo_index,
                        &mut self.work,
                    )?;
                }
                if immediate[block] != Some(next) {
                    immediate[block] = Some(next);
                    changed = true;
                }
            }
            if !changed {
                immediate[entry] = None;
                return Ok(immediate);
            }
        }
    }

    fn compute_dominance_frontiers(
        &mut self,
        immediate: &[Option<usize>],
    ) -> Result<Vec<Vec<usize>>, SsaPlannerErrorV1> {
        let block_count = self.input.blocks.len();
        let mut frontiers = vec![Vec::new(); block_count];
        let entry = self.input.entry.get() as usize;
        let mut children = vec![Vec::new(); block_count];
        for (block, parent) in immediate.iter().copied().enumerate() {
            if let Some(parent) = parent {
                children[parent].push(block);
            }
        }
        let mut postorder = Vec::with_capacity(self.reverse_postorder.len());
        let mut pending = vec![(entry, false)];
        while let Some((block, finish)) = pending.pop() {
            self.work.charge(1)?;
            if finish {
                postorder.push(block);
            } else {
                pending.push((block, true));
                for child in children[block].iter().rev() {
                    pending.push((*child, false));
                }
            }
        }
        for block in postorder {
            let mut candidate_count = self.input.blocks[block].edges.len();
            for child in &children[block] {
                candidate_count = candidate_count
                    .checked_add(frontiers[*child].len())
                    .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
            }
            self.charge_storage_items::<usize>(candidate_count)?;
            let mut block_frontier = Vec::with_capacity(candidate_count);
            for edge in &self.input.blocks[block].edges {
                self.work.charge(1)?;
                let successor = edge.target.get() as usize;
                if self.reachable[successor] && immediate[successor] != Some(block) {
                    block_frontier.push(successor);
                }
            }
            for child in &children[block] {
                self.work.charge(frontiers[*child].len())?;
                for candidate in frontiers[*child].iter().copied() {
                    self.work.charge(1)?;
                    if immediate[candidate] != Some(block) {
                        block_frontier.push(candidate);
                    }
                }
            }
            charge_sort_work(&mut self.work, block_frontier.len())?;
            block_frontier.sort_unstable();
            block_frontier.dedup();
            frontiers[block] = block_frontier;
        }
        Ok(frontiers)
    }

    fn compute_merge_variables(
        &mut self,
        frontiers: &[Vec<usize>],
    ) -> Result<Vec<Vec<SsaVariableIdV1>>, SsaPlannerErrorV1> {
        let block_count = self.input.blocks.len();
        let mut merges = vec![Vec::new(); block_count];
        let entry = self.input.entry.get() as usize;
        let mut definition_generation = vec![0_usize; block_count];
        let mut queued_generation = vec![0_usize; block_count];
        let mut idf_generation = vec![0_usize; block_count];
        let mut pending = VecDeque::new();

        for variable in 0..self.promotable_variables.len() {
            self.work.charge(1)?;
            let semantic_variable = self.promotable_variables[variable];
            let generation = variable
                .checked_add(1)
                .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
            pending.clear();
            self.work
                .charge(self.definition_blocks_by_variable[variable].len())?;
            for block in self.definition_blocks_by_variable[variable].iter().copied() {
                self.work.charge(1)?;
                definition_generation[block] = generation;
                if self.reachable[block] && queued_generation[block] != generation {
                    queued_generation[block] = generation;
                    pending.push_back(block);
                }
            }
            self.work
                .charge(self.edge_definition_targets[variable].len())?;
            for target_index in 0..self.edge_definition_targets[variable].len() {
                let target = self.edge_definition_targets[variable][target_index];
                definition_generation[target] = generation;
                if self.reachable[target] && queued_generation[target] != generation {
                    queued_generation[target] = generation;
                    pending.push_back(target);
                }
            }

            // An edge definition is a definition in a conceptual edge block. If
            // another edge (or the external entry) also reaches its target, the
            // target itself is the first merge site.
            self.work
                .charge(self.edge_definition_targets[variable].len())?;
            for target_index in 0..self.edge_definition_targets[variable].len() {
                let target = self.edge_definition_targets[variable][target_index];
                let incoming = self.incoming_edges[target]
                    .checked_add(usize::from(target == entry))
                    .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
                if incoming > 1
                    && self.live_in.contains(target, variable)
                    && idf_generation[target] != generation
                {
                    idf_generation[target] = generation;
                    self.charge_storage(1)?;
                    merges[target].push(semantic_variable);
                    if queued_generation[target] != generation {
                        queued_generation[target] = generation;
                        pending.push_back(target);
                    }
                }
            }

            while let Some(block) = pending.pop_front() {
                self.work.charge(1 + frontiers[block].len())?;
                for frontier in frontiers[block].iter().copied() {
                    self.work.charge(1)?;
                    if !self.live_in.contains(frontier, variable)
                        || idf_generation[frontier] == generation
                    {
                        continue;
                    }
                    idf_generation[frontier] = generation;
                    self.charge_storage(1)?;
                    merges[frontier].push(semantic_variable);
                    if definition_generation[frontier] != generation
                        && queued_generation[frontier] != generation
                    {
                        queued_generation[frontier] = generation;
                        pending.push_back(frontier);
                    }
                }
            }
        }
        Ok(merges)
    }

    fn compute_transport_variables(
        &mut self,
        merge_variables: &[Vec<SsaVariableIdV1>],
    ) -> Result<Vec<Vec<SsaVariableIdV1>>, SsaPlannerErrorV1> {
        let mut transport = Vec::with_capacity(merge_variables.len());
        for variables in merge_variables {
            self.work.charge(1 + variables.len())?;
            self.charge_storage(variables.len())?;
            transport.push(variables.clone());
        }
        Ok(transport)
    }

    #[allow(clippy::type_complexity)]
    fn resolve_values(
        &mut self,
        immediate_dominators: &[Option<usize>],
        transport_variables: &[Vec<SsaVariableIdV1>],
    ) -> Result<
        (
            Vec<SsaArgumentV1>,
            Vec<SsaArgumentV1>,
            Vec<Vec<(u32, SsaResolvedEventV1)>>,
            Vec<Vec<Vec<SsaArgumentV1>>>,
            Vec<Vec<Vec<SsaArgumentV1>>>,
            usize,
            usize,
        ),
        SsaPlannerErrorV1,
    > {
        let entry = self.input.entry.get() as usize;
        let mut next_definition = 0_u32;
        let mut entry_values = vec![None; self.promotable_variables.len()];
        self.work.charge(self.input.entry_definitions.len())?;
        for variable in self.input.entry_definitions.iter().copied() {
            let Some(index) = self.promotable_index(variable) else {
                continue;
            };
            entry_values[index] = Some(SsaValueV1::Definition(SsaDefinitionIdV1::new(
                next_definition,
            )));
            next_definition = next_definition
                .checked_add(1)
                .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
        }

        let block_count = self.input.blocks.len();
        let mut resolved_counts = vec![0_usize; block_count];
        let mut event_definition_bases = vec![0_u32; block_count];
        let mut promoted_event_definitions = 0_usize;
        let mut promoted_event_changes = 0_usize;
        let mut promoted_edge_definitions = 0_usize;
        for (block_index, block) in self.input.blocks.iter().enumerate() {
            if !self.reachable[block_index] {
                continue;
            }
            event_definition_bases[block_index] = u32::try_from(
                (next_definition as usize)
                    .checked_add(promoted_event_definitions)
                    .ok_or(SsaPlannerErrorV1::IdentityOverflow)?,
            )
            .map_err(|_| SsaPlannerErrorV1::IdentityOverflow)?;
            self.work.charge(block.events.len())?;
            for event in block.events.iter().copied() {
                if self.promotable_index(event.variable()).is_some() {
                    resolved_counts[block_index] = resolved_counts[block_index]
                        .checked_add(1)
                        .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
                    if matches!(event, SsaEventV1::Define(_)) {
                        promoted_event_definitions = promoted_event_definitions
                            .checked_add(1)
                            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
                    }
                    if matches!(event, SsaEventV1::Define(_) | SsaEventV1::Kill(_)) {
                        promoted_event_changes = promoted_event_changes
                            .checked_add(1)
                            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
                    }
                }
            }
            for edge in &block.edges {
                self.work.charge(edge.definitions.len())?;
                for variable in edge.definitions.iter().copied() {
                    if self.promotable_index(variable).is_some() {
                        promoted_edge_definitions = promoted_edge_definitions
                            .checked_add(1)
                            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
                    }
                }
            }
        }

        let entry_definition_count = next_definition as usize;
        self.charge_storage_items::<SsaArgumentV1>(entry_definition_count)?;
        let entry_definitions = self
            .input
            .entry_definitions
            .iter()
            .copied()
            .filter_map(|variable| {
                let variable_index = self.promotable_index(variable)?;
                Some(SsaArgumentV1 {
                    variable,
                    value: entry_values[variable_index]?,
                })
            })
            .collect::<Vec<_>>();
        let edge_definition_start = entry_definition_count
            .checked_add(promoted_event_definitions)
            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
        let generated_definitions = edge_definition_start
            .checked_add(promoted_edge_definitions)
            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
        require_resource(
            SsaPlannerResourceV1::OutputItems,
            generated_definitions,
            self.limits.max_output_items,
        )?;
        let mut edge_definition_bases = self
            .input
            .blocks
            .iter()
            .map(|block| vec![0_u32; block.edges.len()])
            .collect::<Vec<_>>();
        self.charge_storage_items::<u32>(self.input_edges)?;
        let mut edge_definitions = self
            .input
            .blocks
            .iter()
            .map(|block| vec![Vec::new(); block.edges.len()])
            .collect::<Vec<_>>();
        self.charge_storage_items::<Vec<Vec<SsaArgumentV1>>>(block_count)?;
        self.charge_storage_items::<Vec<SsaArgumentV1>>(self.input_edges)?;
        self.charge_storage_items::<SsaArgumentV1>(promoted_edge_definitions)?;
        let mut next_edge_definition = u32::try_from(edge_definition_start)
            .map_err(|_| SsaPlannerErrorV1::IdentityOverflow)?;
        for (block_index, block) in self.input.blocks.iter().enumerate() {
            if !self.reachable[block_index] {
                continue;
            }
            for (edge_index, edge) in block.edges.iter().enumerate() {
                edge_definition_bases[block_index][edge_index] = next_edge_definition;
                for variable in edge.definitions.iter().copied() {
                    if self.promotable_index(variable).is_some() {
                        let value = take_definition_value(&mut next_edge_definition)?;
                        edge_definitions[block_index][edge_index]
                            .push(SsaArgumentV1 { variable, value });
                    }
                }
            }
        }
        if next_edge_definition as usize != generated_definitions {
            return Err(SsaPlannerErrorV1::IdentityOverflow);
        }

        if self.incoming_edges[entry] != 0 {
            for variable in self
                .live_in
                .ones(entry, self.promotable_variables.len())
                .map(|variable| self.promotable_variables[variable])
            {
                let variable_index = self
                    .promotable_index(variable)
                    .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
                if entry_values[variable_index].is_none() {
                    return Err(SsaPlannerErrorV1::UndefinedAtEntry { variable });
                }
            }
        }

        self.charge_storage_items::<SsaArgumentV1>(transport_variables[entry].len())?;
        let mut output_items = checked_add_resource(
            SsaPlannerResourceV1::OutputItems,
            generated_definitions,
            transport_variables[entry].len(),
            self.limits.max_output_items,
        )?;
        let entry_arguments = transport_variables[entry]
            .iter()
            .copied()
            .map(|variable| {
                let variable_index = self
                    .promotable_index(variable)
                    .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
                let value = entry_values[variable_index]
                    .ok_or(SsaPlannerErrorV1::UndefinedAtEntry { variable })?;
                Ok(SsaArgumentV1 { variable, value })
            })
            .collect::<Result<Vec<_>, SsaPlannerErrorV1>>()?;

        let mut resolved_events = vec![Vec::new(); block_count];
        let mut edge_arguments = self
            .input
            .blocks
            .iter()
            .map(|block| vec![Vec::new(); block.edges.len()])
            .collect::<Vec<_>>();
        let mut dominator_children = vec![Vec::new(); block_count];
        for (block, immediate) in immediate_dominators.iter().copied().enumerate() {
            if let Some(parent) = immediate {
                dominator_children[parent].push(block);
            }
        }
        self.charge_storage_items::<Vec<usize>>(block_count)?;
        self.charge_storage_items::<usize>(block_count.saturating_sub(1))?;

        let mut unique_incoming_edges = vec![None; block_count];
        for (source, block) in self.input.blocks.iter().enumerate() {
            if !self.reachable[source] {
                continue;
            }
            for (edge_index, edge) in block.edges.iter().enumerate() {
                let target = edge.target.get() as usize;
                if self.reachable[target] && self.incoming_edges[target] == 1 {
                    unique_incoming_edges[target] = Some((source, edge_index));
                }
            }
        }
        self.charge_storage_items::<Option<(usize, usize)>>(block_count)?;

        let mut current_values = entry_values.clone();
        self.work.charge(transport_variables.len())?;
        let transport_changes =
            transport_variables
                .iter()
                .try_fold(0_usize, |count, variables| {
                    count
                        .checked_add(variables.len())
                        .ok_or(SsaPlannerErrorV1::IdentityOverflow)
                })?;
        let scoped_change_capacity = promoted_event_changes
            .checked_add(promoted_edge_definitions)
            .and_then(|count| count.checked_add(transport_changes))
            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
        self.charge_storage_items::<(usize, Option<SsaValueV1>)>(scoped_change_capacity)?;
        let mut scoped_changes = Vec::with_capacity(scoped_change_capacity);
        let mut pending = vec![(entry, None)];
        while let Some((block_index, restore_to)) = pending.pop() {
            if let Some(restore_to) = restore_to {
                while scoped_changes.len() > restore_to {
                    let (variable, previous) = scoped_changes
                        .pop()
                        .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
                    current_values[variable] = previous;
                }
                continue;
            }
            self.work.charge(1)?;
            let restore_to = scoped_changes.len();
            pending.push((block_index, Some(restore_to)));

            if block_index != entry
                && let Some((source, edge_index)) = unique_incoming_edges[block_index]
            {
                let edge = &self.input.blocks[source].edges[edge_index];
                let mut next = edge_definition_bases[source][edge_index];
                for variable in edge.definitions.iter().copied() {
                    let Some(variable_index) = self.promotable_index(variable) else {
                        continue;
                    };
                    let value = take_definition_value(&mut next)?;
                    scoped_changes.push((variable_index, current_values[variable_index]));
                    current_values[variable_index] = Some(value);
                }
            }

            for variable in transport_variables[block_index].iter().copied() {
                let variable_index = self
                    .promotable_index(variable)
                    .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
                scoped_changes.push((variable_index, current_values[variable_index]));
                current_values[variable_index] = Some(SsaValueV1::BlockArgument {
                    block: SsaBlockIdV1::new(block_index as u32),
                    variable,
                });
            }
            let block = &self.input.blocks[block_index];
            output_items = checked_add_resource(
                SsaPlannerResourceV1::OutputItems,
                output_items,
                transport_variables[block_index].len(),
                self.limits.max_output_items,
            )?;
            output_items = checked_add_resource(
                SsaPlannerResourceV1::OutputItems,
                output_items,
                resolved_counts[block_index],
                self.limits.max_output_items,
            )?;
            self.charge_storage_items::<(u32, SsaResolvedEventV1)>(resolved_counts[block_index])?;
            let mut block_resolved = Vec::with_capacity(resolved_counts[block_index]);
            let mut next_event_definition = event_definition_bases[block_index];
            for (event_index, event) in block.events.iter().copied().enumerate() {
                self.work.charge(1)?;
                let variable = event.variable();
                let Some(variable_index) = self.promotable_index(variable) else {
                    continue;
                };
                let resolved = match event {
                    SsaEventV1::Use(_) => SsaResolvedEventV1::Use {
                        variable,
                        value: current_values[variable_index].ok_or(
                            SsaPlannerErrorV1::UndefinedAtUse {
                                block: SsaBlockIdV1::new(block_index as u32),
                                event: event_index as u32,
                                variable,
                            },
                        )?,
                    },
                    SsaEventV1::Define(_) => {
                        let value = take_definition_value(&mut next_event_definition)?;
                        scoped_changes.push((variable_index, current_values[variable_index]));
                        current_values[variable_index] = Some(value);
                        SsaResolvedEventV1::Define { variable, value }
                    }
                    SsaEventV1::Kill(_) => {
                        let previous = current_values[variable_index];
                        scoped_changes.push((variable_index, previous));
                        current_values[variable_index] = None;
                        SsaResolvedEventV1::Kill { variable, previous }
                    }
                };
                block_resolved.push((event_index as u32, resolved));
            }
            resolved_events[block_index] = block_resolved;
            for (edge_index, edge) in block.edges.iter().enumerate() {
                let edge_id =
                    SsaEdgeIdV1::new(SsaBlockIdV1::new(block_index as u32), edge_index as u32);
                let target = edge.target.get() as usize;
                self.work
                    .charge(1 + edge.definitions.len() + transport_variables[target].len())?;
                output_items = checked_add_resource(
                    SsaPlannerResourceV1::OutputItems,
                    output_items,
                    transport_variables[target].len(),
                    self.limits.max_output_items,
                )?;
                self.charge_storage_items::<SsaArgumentV1>(transport_variables[target].len())?;
                let mut arguments = Vec::with_capacity(transport_variables[target].len());
                let mut definition_index = 0_usize;
                let mut next_edge_definition = edge_definition_bases[block_index][edge_index];
                for variable in transport_variables[target].iter().copied() {
                    while definition_index < edge.definitions.len()
                        && edge.definitions[definition_index] < variable
                    {
                        let definition = edge.definitions[definition_index];
                        if self.promotable_index(definition).is_some() {
                            let _ = take_definition_value(&mut next_edge_definition)?;
                        }
                        definition_index += 1;
                    }
                    let edge_definition =
                        if edge.definitions.get(definition_index) == Some(&variable) {
                            definition_index += 1;
                            Some(take_definition_value(&mut next_edge_definition)?)
                        } else {
                            None
                        };
                    let variable_index = self
                        .promotable_index(variable)
                        .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
                    let current = current_values[variable_index];
                    let value =
                        edge_definition
                            .or(current)
                            .ok_or(SsaPlannerErrorV1::UndefinedAtEdge {
                                edge: edge_id,
                                target: edge.target,
                                variable,
                            })?;
                    arguments.push(SsaArgumentV1 { variable, value });
                }
                for definition in edge.definitions[definition_index..].iter().copied() {
                    if self.promotable_index(definition).is_some() {
                        let _ = take_definition_value(&mut next_edge_definition)?;
                    }
                }
                edge_arguments[block_index][edge_index] = arguments;
            }
            for child in dominator_children[block_index].iter().rev().copied() {
                pending.push((child, None));
            }
        }
        Ok((
            entry_definitions,
            entry_arguments,
            resolved_events,
            edge_definitions,
            edge_arguments,
            generated_definitions,
            output_items,
        ))
    }

    fn live_in_variables(&mut self) -> Result<Vec<Vec<SsaVariableIdV1>>, SsaPlannerErrorV1> {
        let mut live_in = vec![Vec::new(); self.input.blocks.len()];
        for (block, variables) in live_in.iter_mut().enumerate() {
            self.work.charge(1)?;
            if !self.reachable[block] {
                continue;
            }
            self.work.charge(self.variable_words)?;
            let variable_count = self
                .live_in
                .ones(block, self.promotable_variables.len())
                .count();
            self.work.charge(self.variable_words + variable_count)?;
            self.charge_storage(variable_count)?;
            variables.reserve_exact(variable_count);
            variables.extend(
                self.live_in
                    .ones(block, self.promotable_variables.len())
                    .map(|variable| self.promotable_variables[variable]),
            );
        }
        Ok(live_in)
    }

    fn charge_storage(&mut self, words: usize) -> Result<(), SsaPlannerErrorV1> {
        self.storage_words = checked_add_resource(
            SsaPlannerResourceV1::StorageWords,
            self.storage_words,
            words,
            self.limits.max_storage_words,
        )?;
        Ok(())
    }

    fn promotable_index(&self, variable: SsaVariableIdV1) -> Option<usize> {
        self.promotable_indices[variable.get() as usize]
    }

    fn charge_storage_items<T>(&mut self, count: usize) -> Result<(), SsaPlannerErrorV1> {
        charge_storage_items::<T>(
            &mut self.storage_words,
            count,
            self.limits.max_storage_words,
        )
    }
}
