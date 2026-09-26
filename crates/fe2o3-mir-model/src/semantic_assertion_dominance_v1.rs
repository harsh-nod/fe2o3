// Relocated semantic rules; metering does not grant assertion admission.
impl<M: SemanticAssertionMeterV1> SemanticAssertionRecipeQueriesV1<'_, '_, M> {
    #[allow(clippy::too_many_arguments)]
    fn same_edge_stable_unsigned_value_v1(
        &mut self,
        compared: &SemanticOperandV1,
        comparison_site: ScalarAssignmentSiteV1,
        expected: &SemanticOperandV1,
        expected_use: ScalarAssignmentSiteV1,
        edge_target: usize,
        use_site: ScalarAssignmentSiteV1,
        can_reach_use: &[bool],
        visited: &mut [usize],
        pending: &mut VecDeque<usize>,
        generation: &mut usize,
    ) -> Result<bool, ErrorFor<M>> {
        if compared.ty() != expected.ty() || self.unsigned_integer_bits(compared.ty()).is_none() {
            return Ok(false);
        }
        if expected_use.block == use_site.block
            && expected_use.statement == use_site.statement
            && let (Some(compared_local), Some(expected_local)) = (
                simple_operand_local(compared),
                simple_operand_local(expected),
            )
            && compared_local == expected_local
            && self
                .core
                .address_escaped
                .get(compared_local.index() as usize)
                .copied()
                == Some(false)
        {
            let local = compared_local.index() as usize;
            if !self.comparison_edge_authenticates_each_dynamic_use_v1(
                comparison_site.block,
                edge_target,
                use_site.block,
            )? {
                return Ok(false);
            }
            return self.local_stable_after_comparison_and_edge_to_use_v1(
                local,
                comparison_site,
                edge_target,
                use_site,
                can_reach_use,
                visited,
                pending,
                generation,
            );
        }
        let Some((compared, compared_origin_use)) =
            self.exact_same_type_use_alias_origin_v1(compared, comparison_site)?
        else {
            return Ok(false);
        };
        let Some((expected, expected_origin_use)) =
            self.exact_same_type_use_alias_origin_v1(expected, expected_use)?
        else {
            return Ok(false);
        };
        match (&compared, &expected) {
            (SemanticOperandV1::Constant(left), SemanticOperandV1::Constant(right)) => {
                Ok(left.ty() == right.ty() && left.value() == right.value())
            }
            _ => {
                let (Some(compared), Some(expected)) = (
                    simple_operand_local(&compared),
                    simple_operand_local(&expected),
                ) else {
                    return Ok(false);
                };
                if compared != expected {
                    return Ok(false);
                }
                let local = compared.index() as usize;
                if !self.local_is_stable_between_sites_v1(
                    local,
                    compared_origin_use,
                    comparison_site,
                )? || !self.local_is_stable_between_sites_v1(
                    local,
                    expected_origin_use,
                    expected_use,
                )? {
                    return Ok(false);
                }
                self.local_stable_after_comparison_and_edge_to_use_v1(
                    local,
                    comparison_site,
                    edge_target,
                    use_site,
                    can_reach_use,
                    visited,
                    pending,
                    generation,
                )
            }
        }
    }

    fn local_is_stable_between_sites_v1(
        &mut self,
        local: usize,
        capture_site: ScalarAssignmentSiteV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<bool, ErrorFor<M>> {
        let block_count = self.core.function.blocks().len();
        let Some(capture_block) = self.core.function.blocks().get(capture_site.block) else {
            return Ok(false);
        };
        let Some(use_block) = self.core.function.blocks().get(use_site.block) else {
            return Ok(false);
        };
        if local >= self.core.function.locals().len()
            || capture_site.statement > capture_block.statements().len()
            || use_site.statement > use_block.statements().len()
            || self.core.address_escaped.get(local).copied() != Some(false)
        {
            return Ok(false);
        }
        if capture_site.block == use_site.block && capture_site.statement == use_site.statement {
            return Ok(true);
        }
        if capture_site.block == use_site.block {
            if capture_site.statement >= use_site.statement {
                return Ok(false);
            }
            return Ok(!self.block_defines_local_in_statement_range_v1(
                capture_site.block,
                local,
                capture_site
                    .statement
                    .saturating_add(1)
                    .min(capture_block.statements().len()),
                use_site.statement,
            )?);
        }
        if !self.assignment_dominates_use(capture_site, use_site.block, use_site.statement)?
            || self.block_defines_local_in_statement_range_v1(
                capture_site.block,
                local,
                capture_site
                    .statement
                    .saturating_add(1)
                    .min(capture_block.statements().len()),
                capture_block.statements().len(),
            )?
            || self.block_terminator_defines_local_v1(capture_site.block, local)
        {
            return Ok(false);
        }

        let can_reach_use = self.blocks_reaching(use_site.block)?;
        let mut visited = Vec::new();
        self.paid().grow(&mut visited, block_count)?;
        self.paid().charge(block_count)?;
        visited.resize(block_count, false);
        let mut pending = self.paid().deque(block_count)?;
        self.charge(self.core.graph.successors[capture_site.block].len())?;
        for successor in &self.core.graph.successors[capture_site.block] {
            if can_reach_use[*successor] && !visited[*successor] {
                visited[*successor] = true;
                pending.push_back(*successor);
            }
        }
        while let Some(block) = pending.pop_front() {
            self.charge(1)?;
            let defines_local = if block == use_site.block {
                self.block_defines_local_in_statement_range_v1(block, local, 0, use_site.statement)?
            } else {
                self.block_defines_local(block, local)?
            };
            if defines_local {
                return Ok(false);
            }
            if block == use_site.block {
                continue;
            }
            self.charge(self.core.graph.successors[block].len())?;
            for successor in &self.core.graph.successors[block] {
                if can_reach_use[*successor] && !visited[*successor] {
                    visited[*successor] = true;
                    pending.push_back(*successor);
                }
            }
        }
        Ok(true)
    }

    fn block_terminator_defines_local_v1(&self, block: usize, local: usize) -> bool {
        matches!(
            self.core.function.blocks()[block].terminator().kind(),
            SemanticTerminatorKindV1::Call(call)
                if call.destination().and_then(|destination| {
                    local_definition_index(destination.place())
                }) == Some(local)
        )
    }

    fn exact_same_type_use_alias_origin_v1(
        &mut self,
        operand: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<Option<(SemanticOperandV1, ScalarAssignmentSiteV1)>, ErrorFor<M>> {
        let expected_type = operand.ty();
        if self.unsigned_integer_bits(expected_type).is_none() {
            return Ok(None);
        }
        let mut operand = self.paid().clone_operand(operand)?;
        let mut use_site = use_site;
        let mut visited = HashSet::new();
        loop {
            self.charge(1)?;
            match &operand {
                SemanticOperandV1::Constant(constant) if constant.ty() == expected_type => {
                    return Ok(Some((operand, use_site)));
                }
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                    if place.projections().is_empty() && place.ty() == expected_type =>
                {
                    let local = place.local().index() as usize;
                    if !self.paid().set_insert(&mut visited, local)?
                        || self.core.address_escaped.get(local).copied() != Some(false)
                    {
                        return Ok(None);
                    }
                    let Some(definition) = self.exact_reaching_assignment_v1(local, use_site)?
                    else {
                        return Ok(Some((operand, use_site)));
                    };
                    let SemanticStatementKindV1::Assign(assignment) = self.core.function.blocks()
                        [definition.block]
                        .statements()[definition.statement]
                        .kind()
                    else {
                        return Ok(None);
                    };
                    let SemanticRvalueKindV1::Use(next) = assignment.value().kind() else {
                        return Ok(Some((operand, use_site)));
                    };
                    if assignment.value().result_type() != expected_type
                        || next.ty() != expected_type
                    {
                        return Ok(None);
                    }
                    operand = self.paid().clone_operand(next)?;
                    use_site = definition;
                }
                _ => return Ok(None),
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn unsigned_operand_stable_from_edge_to_use_v1(
        &mut self,
        operand: &SemanticOperandV1,
        comparison_site: ScalarAssignmentSiteV1,
        edge_target: usize,
        use_site: ScalarAssignmentSiteV1,
        can_reach_use: &[bool],
        visited: &mut [usize],
        pending: &mut VecDeque<usize>,
        generation: &mut usize,
    ) -> Result<bool, ErrorFor<M>> {
        if self.unsigned_integer_bits(operand.ty()).is_none() {
            return Ok(false);
        }
        let Some(local) = simple_operand_local(operand) else {
            return Ok(matches!(operand, SemanticOperandV1::Constant(_)));
        };
        self.local_stable_after_comparison_and_edge_to_use_v1(
            local.index() as usize,
            comparison_site,
            edge_target,
            use_site,
            can_reach_use,
            visited,
            pending,
            generation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn local_stable_after_comparison_and_edge_to_use_v1(
        &mut self,
        local: usize,
        comparison_site: ScalarAssignmentSiteV1,
        edge_target: usize,
        use_site: ScalarAssignmentSiteV1,
        can_reach_use: &[bool],
        visited: &mut [usize],
        pending: &mut VecDeque<usize>,
        generation: &mut usize,
    ) -> Result<bool, ErrorFor<M>> {
        if self.core.address_escaped.get(local).copied() != Some(false)
            || self.block_defines_local_in_statement_range_v1(
                comparison_site.block,
                local,
                comparison_site.statement.saturating_add(1),
                self.core.function.blocks()[comparison_site.block]
                    .statements()
                    .len(),
            )?
        {
            return Ok(false);
        }
        *generation = generation.checked_add(1).ok_or(analysis_error::<M>(
            SemanticAssertionErrorV1::Unsupported(
                "strict flat-index bound stability generation overflowed",
            ),
        ))?;
        self.local_is_stable_between_edge_and_site_v1(
            local,
            edge_target,
            use_site,
            can_reach_use,
            visited,
            pending,
            *generation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn local_is_stable_between_edge_and_site_v1(
        &mut self,
        local: usize,
        edge_target: usize,
        use_site: ScalarAssignmentSiteV1,
        can_reach_use: &[bool],
        visited: &mut [usize],
        pending: &mut VecDeque<usize>,
        generation: usize,
    ) -> Result<bool, ErrorFor<M>> {
        let block_count = self.core.function.blocks().len();
        if local >= self.core.function.locals().len()
            || edge_target >= block_count
            || use_site.block >= block_count
            || use_site.statement
                > self.core.function.blocks()[use_site.block]
                    .statements()
                    .len()
            || can_reach_use.len() != block_count
            || visited.len() != block_count
            || !can_reach_use[edge_target]
        {
            return Ok(false);
        }

        pending.clear();
        pending.push_back(edge_target);
        visited[edge_target] = generation;
        while let Some(block) = pending.pop_front() {
            self.charge(1)?;
            let defines_local = if block == use_site.block {
                self.block_defines_local_in_statement_range_v1(block, local, 0, use_site.statement)?
            } else {
                self.block_defines_local(block, local)?
            };
            if defines_local {
                return Ok(false);
            }
            if block == use_site.block {
                continue;
            }
            self.charge(self.core.graph.successors[block].len())?;
            for successor in &self.core.graph.successors[block] {
                if can_reach_use[*successor] && visited[*successor] != generation {
                    visited[*successor] = generation;
                    pending.push_back(*successor);
                }
            }
        }
        Ok(true)
    }

    fn block_defines_local_in_statement_range_v1(
        &mut self,
        block: usize,
        local: usize,
        start: usize,
        end: usize,
    ) -> MR<bool, M> {
        self.paid().charge(
            end.checked_sub(start)
                .and_then(|n| n.checked_mul(3))
                .ok_or(SemanticAssertionErrorV1::Arithmetic)?,
        )?;
        Ok(self.core.function.blocks()[block].statements()[start..end]
            .iter()
            .any(|statement| {
                let mut defines_local = false;
                visit_statement_definition_places(statement.kind(), &mut |place| {
                    defines_local |= local_definition_index(place) == Some(local);
                });
                defines_local
            }))
    }

    fn exact_checked_overflow_flag_source_local_v1(
        &mut self,
        operand: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<Option<usize>, ErrorFor<M>> {
        let mut operand = self.paid().clone_operand(operand)?;
        let mut use_site = use_site;
        let mut visited = HashSet::new();
        loop {
            self.charge(1)?;
            let Some(place) = raw_operand_place(&operand) else {
                return Ok(None);
            };
            let local = place.local().index() as usize;
            if !self.paid().set_insert(&mut visited, local)? {
                return Ok(None);
            }
            if let [field] = place.projections()
                && matches!(field.kind(), SemanticProjectionKindV1::Field(1))
                && self.scalar_unsigned_maximum(place.ty()) == Some(1)
            {
                return Ok(Some(local));
            }
            if !place.projections().is_empty() {
                return Ok(None);
            }
            let Some(site) = self.exact_reaching_assignment_v1(local, use_site)? else {
                return Ok(None);
            };
            let SemanticStatementKindV1::Assign(assignment) =
                self.core.function.blocks()[site.block].statements()[site.statement].kind()
            else {
                return Ok(None);
            };
            let SemanticRvalueKindV1::Use(next) = assignment.value().kind() else {
                return Ok(None);
            };
            operand = self.paid().clone_operand(next)?;
            use_site = site;
        }
    }

    fn operand_has_globally_stable_value_at_v1(
        &mut self,
        operand: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<bool, ErrorFor<M>> {
        match operand {
            SemanticOperandV1::Constant(_) => Ok(true),
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                if place.projections().is_empty() =>
            {
                self.local_has_globally_stable_value_at(place.local().index() as usize, use_site)
            }
            _ => Ok(false),
        }
    }

    pub fn assignment_dominates_use(
        &mut self,
        definition: ScalarAssignmentSiteV1,
        use_block: usize,
        use_statement: usize,
    ) -> Result<bool, ErrorFor<M>> {
        if definition.block == use_block {
            return Ok(definition.statement < use_statement);
        }
        self.block_dominates(definition.block, use_block)
    }

    pub fn exact_reaching_assignment_v1(
        &mut self,
        local: usize,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<Option<ScalarAssignmentSiteV1>, ErrorFor<M>> {
        if self.core.address_escaped.get(local).copied() != Some(false) {
            return Ok(None);
        }
        let Some(block) = self.core.function.blocks().get(use_site.block) else {
            return Ok(None);
        };
        if use_site.statement > block.statements().len() {
            return Ok(None);
        }
        let defining_statement = if self.use_statement_index
            && let Some(index) = &self.core.statement_definitions
        {
            self.core
                .resources
                .metered(self.meter)
                .charge(3 * (1 + (usize::BITS - index.rows.len().leading_zeros()) as usize))?;
            let statement = index.before(local, use_site.block, use_site.statement);
            let visits = statement.map_or(use_site.statement, |index| use_site.statement - index);
            self.scan_equivalent(visits)?;
            statement
        } else {
            let mut found = None;
            for statement in (0..use_site.statement).rev() {
                self.charge(1)?;
                let kind = block.statements()[statement].kind();
                let mut defines_local = false;
                visit_statement_definition_places(kind, &mut |place| {
                    defines_local |= local_definition_index(place) == Some(local);
                });
                if defines_local {
                    found = Some(statement);
                    break;
                }
            }
            found
        };
        if let Some(statement) = defining_statement {
            let kind = block.statements()[statement].kind();
            return Ok(matches!(kind, SemanticStatementKindV1::Assign(assignment)
                    if assignment.destination().projections().is_empty()
                        && assignment.destination().local().index() as usize == local)
            .then_some(ScalarAssignmentSiteV1 {
                block: use_site.block,
                statement,
            }));
        }
        if self.core.definition_counts.get(local).copied() != Some(1) {
            return Ok(None);
        }
        let Some(site) = self.core.assignments.get(local).copied().flatten() else {
            return Ok(None);
        };
        self.assignment_dominates_use(site, use_site.block, use_site.statement)
            .map(|dominates| dominates.then_some(site))
    }

    fn zero_excluding_edge_dominates(
        &mut self,
        local: usize,
        use_block: usize,
    ) -> Result<bool, ErrorFor<M>> {
        self.core
            .resources
            .metered(self.meter)
            .charge(sum(self.core.zero_exclusion.len(), 1)?)?;
        if let Some(result) = self.core.zero_exclusion.get(&(local, use_block)).copied() {
            return Ok(result);
        }
        let block_count = self.core.function.blocks().len();
        let can_reach_use = self.blocks_reaching(use_block)?;
        let mut excluding_edges = HashSet::new();
        self.paid().set_reserve(&mut excluding_edges, block_count)?;
        let mut stability_visited = Vec::new();
        self.paid().grow(&mut stability_visited, block_count)?;
        self.paid().charge(block_count)?;
        stability_visited.resize(block_count, 0_usize);
        let mut stability_pending = self.paid().deque(block_count)?;
        let mut stability_generation = 0_usize;
        for (switch_block, block) in self.core.function.blocks().iter().enumerate() {
            self.charge(1)?;
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = block.terminator().kind()
            else {
                continue;
            };
            let Some(discriminant_local) =
                simple_operand_local(discriminant).map(|value| value.index() as usize)
            else {
                continue;
            };
            if targets.values().len() != 1 || targets.values()[0].value() != 0 {
                continue;
            }
            let zero_target = targets.values()[0].edge().target().index() as usize;
            let nonzero_target = targets.otherwise().target().index() as usize;
            let discriminant_is_tested = self.local_is_value_preserving_alias_of(
                discriminant_local,
                local,
                switch_block,
                block.statements().len(),
                None,
            )?;
            let discriminant_is_tested = discriminant_is_tested
                && (discriminant_local == local
                    || !self.block_defines_local(switch_block, local)?);
            let excluding_target = if discriminant_is_tested {
                Some(nonzero_target)
            } else {
                self.comparison_zero_excluding_target(
                    discriminant_local,
                    local,
                    switch_block,
                    zero_target,
                    nonzero_target,
                )?
            };
            let Some(excluding_target) = excluding_target else {
                continue;
            };
            if zero_target == nonzero_target {
                continue;
            }
            stability_generation =
                stability_generation
                    .checked_add(1)
                    .ok_or(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                        "assertion proof stability generation overflowed",
                    )))?;
            let stable = self.local_is_stable_between_edge_and_use(
                local,
                excluding_target,
                use_block,
                &can_reach_use,
                &mut stability_visited,
                &mut stability_pending,
                stability_generation,
            )?;
            if !stable {
                continue;
            }
            self.paid()
                .set_insert(&mut excluding_edges, (switch_block, excluding_target))?;
        }
        let result = self.edge_set_dominates(&excluding_edges, use_block)?;
        insert_assertion_proof_cache(
            self.core.resources.metered(self.meter),
            &mut self.core.zero_exclusion,
            (local, use_block),
            result,
        )?;
        Ok(result)
    }

    fn blocks_reaching(&mut self, use_block: usize) -> Result<Vec<bool>, ErrorFor<M>> {
        let block_count = self.core.function.blocks().len();
        if use_block >= block_count {
            return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                "an assertion proof reachability query is outside the semantic CFG",
            )));
        }
        let mut can_reach_use = Vec::new();
        self.paid().grow(&mut can_reach_use, block_count)?;
        self.paid().charge(block_count)?;
        can_reach_use.resize(block_count, false);
        let mut reverse = self.paid().deque(block_count)?;
        reverse.push_back(use_block);
        can_reach_use[use_block] = true;
        while let Some(block) = reverse.pop_front() {
            self.charge(1)?;
            self.charge(self.core.graph.predecessors[block].len())?;
            for predecessor in &self.core.graph.predecessors[block] {
                if !can_reach_use[*predecessor] {
                    can_reach_use[*predecessor] = true;
                    reverse.push_back(*predecessor);
                }
            }
        }
        Ok(can_reach_use)
    }

    #[allow(clippy::too_many_arguments)]
    fn local_is_stable_between_edge_and_use(
        &mut self,
        local: usize,
        edge_target: usize,
        use_block: usize,
        can_reach_use: &[bool],
        visited: &mut [usize],
        pending: &mut VecDeque<usize>,
        generation: usize,
    ) -> Result<bool, ErrorFor<M>> {
        let block_count = self.core.function.blocks().len();
        if local >= self.core.function.locals().len()
            || edge_target >= block_count
            || use_block >= block_count
            || can_reach_use.len() != block_count
            || visited.len() != block_count
        {
            return Ok(false);
        }
        if !can_reach_use[edge_target] {
            return Ok(false);
        }

        pending.clear();
        pending.push_back(edge_target);
        visited[edge_target] = generation;
        while let Some(block) = pending.pop_front() {
            self.charge(1)?;
            if self.block_defines_local(block, local)? {
                return Ok(false);
            }
            if block == use_block {
                continue;
            }
            self.charge(self.core.graph.successors[block].len())?;
            for successor in &self.core.graph.successors[block] {
                if can_reach_use[*successor] && visited[*successor] != generation {
                    visited[*successor] = generation;
                    pending.push_back(*successor);
                }
            }
        }
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    fn local_is_stable_from_revalidating_edge_to_use(
        &mut self,
        local: usize,
        guard_source: usize,
        guard_target: usize,
        use_block: usize,
        can_reach_use: &[bool],
        visited: &mut [usize],
        pending: &mut VecDeque<usize>,
        generation: usize,
    ) -> Result<bool, ErrorFor<M>> {
        let block_count = self.core.function.blocks().len();
        if local >= self.core.function.locals().len()
            || guard_source >= block_count
            || guard_target >= block_count
            || use_block >= block_count
            || guard_source == guard_target
            || can_reach_use.len() != block_count
            || visited.len() != block_count
        {
            return Ok(false);
        }

        let reverse_generation = generation.checked_mul(2).ok_or(analysis_error::<M>(
            SemanticAssertionErrorV1::Unsupported(
                "assertion proof revalidating-edge generation overflowed",
            ),
        ))?;
        let forward_generation = reverse_generation
            .checked_add(1)
            .ok_or(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                "assertion proof revalidating-edge generation overflowed",
            )))?;

        // Mark exactly the blocks that can reach the use without first
        // returning to the guard. This excludes latch-side definitions whose
        // only route back to the use crosses a fresh successful guard edge.
        pending.clear();
        pending.push_back(use_block);
        visited[use_block] = reverse_generation;
        while let Some(block) = pending.pop_front() {
            self.charge(1)?;
            self.charge(self.core.graph.predecessors[block].len())?;
            for predecessor in &self.core.graph.predecessors[block] {
                if *predecessor == guard_source
                    || !can_reach_use[*predecessor]
                    || visited[*predecessor] == reverse_generation
                    || visited[*predecessor] == forward_generation
                {
                    continue;
                }
                visited[*predecessor] = reverse_generation;
                pending.push_back(*predecessor);
            }
        }
        if visited[guard_target] != reverse_generation {
            return Ok(true);
        }

        pending.clear();
        pending.push_back(guard_target);
        visited[guard_target] = forward_generation;
        while let Some(block) = pending.pop_front() {
            self.charge(1)?;
            if self.block_defines_local(block, local)? {
                return Ok(false);
            }
            if block == use_block {
                continue;
            }
            self.charge(self.core.graph.successors[block].len())?;
            for successor in &self.core.graph.successors[block] {
                if visited[*successor] == reverse_generation {
                    visited[*successor] = forward_generation;
                    pending.push_back(*successor);
                }
            }
        }
        Ok(true)
    }

    fn comparison_zero_excluding_target(
        &mut self,
        condition_local: usize,
        tested_local: usize,
        switch_block: usize,
        false_target: usize,
        true_target: usize,
    ) -> Result<Option<usize>, ErrorFor<M>> {
        let switch_use = ScalarAssignmentSiteV1 {
            block: switch_block,
            statement: self.core.function.blocks()[switch_block].statements().len(),
        };
        let Some(site) = self.exact_reaching_assignment_v1(condition_local, switch_use)? else {
            return Ok(None);
        };
        if site.block != switch_block
            && !self.local_has_globally_stable_value_at(
                tested_local,
                ScalarAssignmentSiteV1 {
                    block: site.block,
                    statement: site.statement,
                },
            )?
        {
            return Ok(None);
        }
        let SemanticStatementKindV1::Assign(assignment) =
            self.core.function.blocks()[site.block].statements()[site.statement].kind()
        else {
            return Ok(None);
        };
        let SemanticRvalueKindV1::Binary {
            operation,
            left,
            right,
        } = assignment.value().kind()
        else {
            return Ok(None);
        };
        let left_local = simple_operand_local(left).map(|value| value.index() as usize);
        let right_local = simple_operand_local(right).map(|value| value.index() as usize);
        let left_is_tested = if let Some(left_local) = left_local {
            self.local_is_value_preserving_alias_of(
                left_local,
                tested_local,
                site.block,
                site.statement,
                None,
            )?
        } else {
            false
        };
        let right_is_tested = if let Some(right_local) = right_local {
            self.local_is_value_preserving_alias_of(
                right_local,
                tested_local,
                site.block,
                site.statement,
                None,
            )?
        } else {
            false
        };
        let compares_tested_local_to_zero = left_is_tested
            && self.operand_is_exact_unsigned_zero(right)
            || right_is_tested && self.operand_is_exact_unsigned_zero(left);
        if !compares_tested_local_to_zero {
            return Ok(None);
        }
        if site.block == switch_block && self.block_defines_local(switch_block, tested_local)? {
            return Ok(None);
        }
        Ok(match operation {
            SemanticBinaryOpV1::Equal => Some(false_target),
            SemanticBinaryOpV1::NotEqual => Some(true_target),
            _ => None,
        })
    }

    fn operand_is_exact_unsigned_zero(&self, operand: &SemanticOperandV1) -> bool {
        let SemanticOperandV1::Constant(constant) = operand else {
            return false;
        };
        self.scalar_unsigned_maximum(constant.ty()).is_some()
            && matches!(
                constant.value(),
                SemanticConstantValueV1::Scalar(value) if value.bits() == 0
            )
    }

    fn local_has_globally_stable_value_at(
        &mut self,
        local: usize,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<bool, ErrorFor<M>> {
        if self.core.address_escaped.get(local).copied() != Some(false) {
            return Ok(false);
        }
        match self.core.definition_counts.get(local).copied() {
            Some(0) => Ok(true),
            Some(1) => {
                let Some(definition) = self.core.assignments.get(local).copied().flatten() else {
                    return Ok(false);
                };
                self.assignment_dominates_use(definition, use_site.block, use_site.statement)
            }
            _ => Ok(false),
        }
    }

    fn block_defines_local(&mut self, block: usize, local: usize) -> MR<bool, M> {
        let count = self.core.block_definitions.get(block).map_or(0, Vec::len);
        self.paid()
            .charge(1 + (usize::BITS - count.leading_zeros()) as usize)?;
        Ok(self
            .core
            .block_definitions
            .get(block)
            .is_some_and(|definitions| definitions.binary_search(&local).is_ok()))
    }

    fn local_is_value_preserving_alias_of(
        &mut self,
        mut candidate: usize,
        source: usize,
        mut use_block: usize,
        mut use_statement: usize,
        capture: Option<&mut ScalarAssignmentSiteV1>,
    ) -> Result<bool, ErrorFor<M>> {
        // Each followed definition must strictly precede its use in the same block, so
        // the statement index is both the cycle guard and the stack-independent bound.
        loop {
            self.charge(1)?;
            if self.core.address_escaped.get(candidate).copied() != Some(false) {
                return Ok(false);
            }
            if candidate == source {
                if let Some(capture) = capture {
                    *capture = ScalarAssignmentSiteV1 {
                        block: use_block,
                        statement: use_statement,
                    };
                }
                return Ok(true);
            }
            if self.core.definition_counts.get(candidate).copied() != Some(1) {
                return Ok(false);
            }
            let Some(site) = self.core.assignments.get(candidate).copied().flatten() else {
                return Ok(false);
            };
            if site.block != use_block
                || !self.assignment_dominates_use(site, use_block, use_statement)?
            {
                return Ok(false);
            }
            let SemanticStatementKindV1::Assign(assignment) =
                self.core.function.blocks()[site.block].statements()[site.statement].kind()
            else {
                return Ok(false);
            };
            let operand = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => Some(operand),
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand,
                } if self
                    .unsigned_integer_bits(assignment.value().result_type())
                    .zip(self.unsigned_integer_bits(operand.ty()))
                    .is_some_and(|(destination, source)| destination >= source) =>
                {
                    Some(operand)
                }
                _ => None,
            };
            let Some(next) = operand.and_then(simple_operand_local) else {
                return Ok(false);
            };
            candidate = next.index() as usize;
            use_block = site.block;
            use_statement = site.statement;
        }
    }

    pub fn block_dominates(&mut self, dominator: usize, block: usize) -> Result<bool, ErrorFor<M>> {
        if dominator >= self.core.graph.successors.len()
            || block >= self.core.graph.successors.len()
        {
            return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                "an assertion proof dominance query is outside the semantic CFG",
            )));
        }
        self.core
            .resources
            .metered(self.meter)
            .charge(sum(self.core.dominance.len(), 1)?)?;
        if let Some(result) = self.core.dominance.get(&(dominator, block)).copied() {
            return Ok(result);
        }
        if dominator == block {
            let result = self.core.graph.reachable[block];
            insert_assertion_proof_cache(
                self.core.resources.metered(self.meter),
                &mut self.core.dominance,
                (dominator, block),
                result,
            )?;
            return Ok(result);
        }
        if !self.core.graph.reachable[dominator] || !self.core.graph.reachable[block] {
            insert_assertion_proof_cache(
                self.core.resources.metered(self.meter),
                &mut self.core.dominance,
                (dominator, block),
                false,
            )?;
            return Ok(false);
        }
        let node_count = self.core.graph.successors.len();
        let mut visited = Vec::new();
        self.paid().grow(&mut visited, node_count)?;
        self.paid().charge(node_count)?;
        visited.resize(node_count, false);
        let mut pending = Vec::new();
        self.paid().grow(&mut pending, node_count)?;
        if self.core.graph.entry != dominator {
            visited[self.core.graph.entry] = true;
            pending.push(self.core.graph.entry);
        }
        while let Some(current) = pending.pop() {
            self.charge(1)?;
            if current == block {
                insert_assertion_proof_cache(
                    self.core.resources.metered(self.meter),
                    &mut self.core.dominance,
                    (dominator, block),
                    false,
                )?;
                return Ok(false);
            }
            self.charge(self.core.graph.successors[current].len())?;
            for successor in &self.core.graph.successors[current] {
                if *successor != dominator && !visited[*successor] {
                    visited[*successor] = true;
                    pending.push(*successor);
                }
            }
        }
        insert_assertion_proof_cache(
            self.core.resources.metered(self.meter),
            &mut self.core.dominance,
            (dominator, block),
            true,
        )?;
        Ok(true)
    }

    pub fn edge_set_dominates(
        &mut self,
        excluding_edges: &HashSet<(usize, usize)>,
        block: usize,
    ) -> Result<bool, ErrorFor<M>> {
        if block >= self.core.graph.successors.len() {
            return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                "an assertion proof edge-set dominance query is outside the semantic CFG",
            )));
        }
        if excluding_edges.is_empty() || !self.core.graph.reachable[block] {
            return Ok(false);
        }
        let node_count = self.core.graph.successors.len();
        let mut visited = Vec::new();
        self.paid().grow(&mut visited, node_count)?;
        self.paid().charge(node_count)?;
        visited.resize(node_count, false);
        let mut pending = Vec::new();
        self.paid().grow(&mut pending, node_count)?;
        visited[self.core.graph.entry] = true;
        pending.push(self.core.graph.entry);
        while let Some(current) = pending.pop() {
            self.charge(1)?;
            if current == block {
                return Ok(false);
            }
            self.charge(self.core.graph.successors[current].len())?;
            for successor in &self.core.graph.successors[current] {
                if !self
                    .core
                    .resources
                    .metered(self.meter)
                    .set_contains(excluding_edges, &(current, *successor))?
                    && !visited[*successor]
                {
                    visited[*successor] = true;
                    pending.push(*successor);
                }
            }
        }
        Ok(true)
    }
}
