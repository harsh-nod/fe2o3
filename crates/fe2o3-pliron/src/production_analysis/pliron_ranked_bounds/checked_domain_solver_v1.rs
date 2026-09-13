impl CheckedDomainProofV1<'_, '_> {
    fn equality(
        &mut self,
        state: CheckedDomainStateV1,
        budget: &mut RankedBoundsBudget,
    ) -> Result<bool, RankedBoundsFindingV1> {
        if let Some(result) = self.memoized(state, budget)? {
            return Ok(result);
        }
        let result = self.equality_uncached(state, budget)?;
        self.remember(state, result, budget)
    }

    fn equality_uncached(
        &mut self,
        initial: CheckedDomainStateV1,
        budget: &mut RankedBoundsBudget,
    ) -> Result<bool, RankedBoundsFindingV1> {
        // Literal equations and authenticated entry arguments need no CFG
        // closure. Admit the fixed inspection before creating any worklist.
        budget.work(16)?;
        if let CheckedDomainGoalV1::Equal { actual, expected } = initial.goal {
            let node = self
                .dag
                .nodes
                .get(usize::from(expected))
                .filter(|_| usize::from(expected) < self.dag.node_count)
                .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
            let constant = match node.kind {
                CheckedDomainNodeKindV1::Constant(value) => Some(value),
                CheckedDomainNodeKindV1::Leaf(role) => {
                    match initial.environment[usize::from(role)] {
                        IndexExpr::Constant(value) => Some(value),
                        _ => None,
                    }
                }
                CheckedDomainNodeKindV1::Binary { .. } => None,
            };
            if let (IndexExpr::Constant(actual), Some(expected)) = (actual, constant) {
                return Ok(actual == expected);
            }
            if let CheckedDomainNodeKindV1::Leaf(role) = node.kind {
                budget.work(8)?;
                if actual == initial.environment[usize::from(role)]
                    && let IndexExpr::Value(value) = actual
                    && value.defining_block().is_some()
                {
                    let parent = self.owned_value(value, budget)?;
                    if self.graph.blocks.first().copied() == Some(parent) {
                        return Ok(true);
                    }
                }
            }
        }
        self.with_worklist(budget, |proof, visited, budget| {
            proof.equality_worklist(initial, visited, budget)
        })
    }

    fn equality_worklist(
        &mut self,
        initial: CheckedDomainStateV1,
        visited: &mut Vec<CheckedDomainStateV1>,
        budget: &mut RankedBoundsBudget,
    ) -> Result<bool, RankedBoundsFindingV1> {
        checked_domain_enqueue_v1(visited, initial, budget)?;
        let mut cursor = 0;
        while cursor < visited.len() {
            budget.work(CHECKED_DOMAIN_STATE_ITEMS_V1 + 3)?;
            let state = visited[cursor];
            cursor += 1;
            if let CheckedDomainGoalV1::Defined(actual) = state.goal {
                if !self.defined_leaf(state, actual, visited, budget)? {
                    return Ok(false);
                }
                continue;
            }
            let CheckedDomainGoalV1::Equal { actual, expected } = state.goal else {
                return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
            };
            let node = *self
                .dag
                .nodes
                .get(usize::from(expected))
                .filter(|_| usize::from(expected) < self.dag.node_count)
                .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
            let folded = self.fold(state.environment, budget)?;
            if let (IndexExpr::Constant(actual), Some(expected)) =
                (actual, folded[usize::from(expected)])
            {
                if actual != expected {
                    return Ok(false);
                }
                continue;
            }
            let value = match actual {
                IndexExpr::Constant(_) => return Ok(false),
                IndexExpr::Value(value) | IndexExpr::Dimension { view: value, .. } => value,
            };
            let parent = self.owned_value(value, budget)?;
            let local_definition = value
                .defining_op()
                .filter(|_| parent == self.graph.blocks[state.block]);
            if let CheckedDomainNodeKindV1::Leaf(role) = node.kind {
                budget.work(8)?;
                if actual == state.environment[usize::from(role)] {
                    checked_domain_enqueue_v1(
                        visited,
                        CheckedDomainStateV1 {
                            environment: [IndexExpr::Constant(0); 6],
                            goal: CheckedDomainGoalV1::Defined(actual),
                            ..state
                        },
                        budget,
                    )?;
                    continue;
                }
            }
            if let Some(definition) = local_definition {
                let CheckedDomainNodeKindV1::Binary { kind, lhs, rhs } = node.kind else {
                    return Ok(false);
                };
                let operands = {
                    budget.work(12)?;
                    let Some(binary) =
                        Operation::get_op::<IndexBinaryOp>(definition, self.graph.context)
                    else {
                        return Ok(false);
                    };
                    if binary.kind(self.graph.context) != Some(kind) {
                        return Ok(false);
                    }
                    [
                        binary.lhs(self.graph.context),
                        binary.rhs(self.graph.context),
                    ]
                };
                let operands = [
                    self.canonical(operands[0], budget)?,
                    self.canonical(operands[1], budget)?,
                ];
                let mut matched = false;
                for reverse in [false, true] {
                    if reverse
                        && !matches!(
                            kind,
                            IndexBinaryKindAttr::Add | IndexBinaryKindAttr::Multiply
                        )
                    {
                        break;
                    }
                    let [actual_lhs, actual_rhs] = if reverse {
                        [operands[1], operands[0]]
                    } else {
                        operands
                    };
                    if self.equality(
                        CheckedDomainStateV1 {
                            goal: CheckedDomainGoalV1::Equal {
                                actual: actual_lhs,
                                expected: lhs,
                            },
                            ..state
                        },
                        budget,
                    )? && self.equality(
                        CheckedDomainStateV1 {
                            goal: CheckedDomainGoalV1::Equal {
                                actual: actual_rhs,
                                expected: rhs,
                            },
                            ..state
                        },
                        budget,
                    )? {
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    return Ok(false);
                }
                // These predicates must hold on entry to the actual defining
                // block, not merely later where its result guards an access.
                for formula in 0..self.dag.formula_count {
                    budget.work(2)?;
                    if node.requirements & (1 << formula) != 0 {
                        budget.work(32)?;
                        if self.dag.formulas[formula].level() >= expected {
                            return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
                        }
                        let goal = CheckedDomainGoalV1::Predicate(CheckedDomainResidualV1::new(
                            formula as u8,
                            &self.dag,
                        ));
                        if !self.predicate(CheckedDomainStateV1 { goal, ..state }, budget)? {
                            return Ok(false);
                        }
                    }
                }
                continue;
            }
            if state.block == 0
                || self.graph.predecessors[state.block].is_empty()
                || self.killed(state.environment, state.block, budget)?
            {
                return Ok(false);
            }
            for edge in &self.graph.predecessors[state.block] {
                budget.work(1)?;
                let (environment, actual) = self.pull_environment(state, edge, budget)?;
                let actual = actual.ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
                checked_domain_enqueue_v1(
                    visited,
                    CheckedDomainStateV1 {
                        block: edge.block,
                        environment,
                        goal: CheckedDomainGoalV1::Equal { actual, expected },
                    },
                    budget,
                )?;
            }
        }
        Ok(true)
    }

    fn defined_leaf(
        &mut self,
        state: CheckedDomainStateV1,
        actual: IndexExpr,
        visited: &mut Vec<CheckedDomainStateV1>,
        budget: &mut RankedBoundsBudget,
    ) -> Result<bool, RankedBoundsFindingV1> {
        budget.work(12)?;
        let value = match actual {
            IndexExpr::Constant(_) => return Ok(true),
            IndexExpr::Value(value) | IndexExpr::Dimension { view: value, .. } => value,
        };
        let parent = self.owned_value(value, budget)?;
        if value.defining_block() == Some(self.graph.blocks[0]) {
            return Ok(true);
        }
        if let Some(definition) = value
            .defining_op()
            .filter(|_| parent == self.graph.blocks[state.block])
        {
            if matches!(actual, IndexExpr::Value(_))
                && (Operation::is_op::<IndexUnknownOp>(definition, self.graph.context)
                    || Operation::is_op::<InvocationIndexOp>(definition, self.graph.context)
                    || Operation::is_op::<IndexConstantOp>(definition, self.graph.context))
            {
                return Ok(true);
            }
            let source = if let Some(cast) =
                Operation::get_op::<IndexUnsignedCastOp>(definition, self.graph.context)
            {
                budget.work(8)?;
                if cast.inclusive_upper_bound(self.graph.context).is_none() {
                    return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
                }
                self.canonical(cast.source(self.graph.context), budget)?
            } else if let IndexExpr::Dimension { view, dimension } = actual {
                self.validate_view_extent(view, dimension, budget)?;
                budget.work(bounds_transport_sum_v1(
                    self.graph.roster_len(view)?,
                    BOUNDS_TRANSPORT_CANONICAL_WORK_V1,
                )?)?;
                let ty = ranked_view_type(view, self.graph.context)
                    .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
                let ty = ty.deref(self.graph.context);
                if dimension >= ty.shape().len() {
                    return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
                }
                let source = extent_expr(view, &ty, dimension, self.graph.context);
                if source == actual {
                    return Ok(false);
                }
                source
            } else {
                // A partial binary/Checked result is not a defined free leaf.
                return Ok(false);
            };
            checked_domain_enqueue_v1(
                visited,
                CheckedDomainStateV1 {
                    goal: CheckedDomainGoalV1::Defined(source),
                    ..state
                },
                budget,
            )?;
            return Ok(true);
        }
        if state.block == 0 || self.graph.predecessors[state.block].is_empty() {
            return Ok(false);
        }
        for edge in &self.graph.predecessors[state.block] {
            budget.work(1)?;
            let (environment, actual) = self.pull_environment(state, edge, budget)?;
            checked_domain_enqueue_v1(
                visited,
                CheckedDomainStateV1 {
                    block: edge.block,
                    environment,
                    goal: CheckedDomainGoalV1::Defined(
                        actual.ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?,
                    ),
                },
                budget,
            )?;
        }
        Ok(true)
    }

    fn predicate(
        &mut self,
        state: CheckedDomainStateV1,
        budget: &mut RankedBoundsBudget,
    ) -> Result<bool, RankedBoundsFindingV1> {
        if let Some(result) = self.memoized(state, budget)? {
            return Ok(result);
        }
        let result = self.predicate_uncached(state, budget)?;
        self.remember(state, result, budget)
    }

    fn predicate_uncached(
        &mut self,
        initial: CheckedDomainStateV1,
        budget: &mut RankedBoundsBudget,
    ) -> Result<bool, RankedBoundsFindingV1> {
        self.with_worklist(budget, |proof, visited, budget| {
            proof.predicate_worklist(initial, visited, budget)
        })
    }

    fn predicate_worklist(
        &mut self,
        initial: CheckedDomainStateV1,
        visited: &mut Vec<CheckedDomainStateV1>,
        budget: &mut RankedBoundsBudget,
    ) -> Result<bool, RankedBoundsFindingV1> {
        checked_domain_enqueue_v1(visited, initial, budget)?;
        let mut cursor = 0;
        while cursor < visited.len() {
            budget.work(CHECKED_DOMAIN_STATE_ITEMS_V1 + 3)?;
            let state = visited[cursor];
            cursor += 1;
            let CheckedDomainGoalV1::Predicate(mut residual) = state.goal else {
                return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
            };
            let formula = self.dag.formulas[usize::from(residual.formula)];
            let folded = self.fold(state.environment, budget)?;
            for (i, atom) in formula.atoms.iter().enumerate() {
                budget.work(8)?;
                if residual.used() & (1 << i) != 0
                    && let Some((lhs, rhs)) =
                        folded[usize::from(atom.lhs)].zip(folded[usize::from(atom.rhs)])
                {
                    residual.observe(i, lhs < rhs);
                }
            }
            if residual.proven() {
                continue;
            }
            if residual.clauses == 0
                || state.block == 0
                || self.graph.predecessors[state.block].is_empty()
                || self.killed(state.environment, state.block, budget)?
            {
                return Ok(false);
            }
            for edge in &self.graph.predecessors[state.block] {
                budget.work(1)?;
                let (environment, _) = self.pull_environment(state, edge, budget)?;
                let mut incoming = residual;
                let guard = self.edge_guard(edge, budget)?;
                if let Some((lhs, rhs, equal, truth)) = guard {
                    let lhs = self.canonical(lhs, budget)?;
                    let rhs = self.canonical(rhs, budget)?;
                    for (i, atom) in formula.atoms.iter().enumerate() {
                        budget.work(8)?;
                        if incoming.used() & (1 << i) == 0 {
                            continue;
                        }
                        let base = CheckedDomainStateV1 {
                            block: edge.block,
                            environment,
                            goal: CheckedDomainGoalV1::Equal {
                                actual: lhs,
                                expected: atom.lhs,
                            },
                        };
                        let direct = self.equality(base, budget)?
                            && self.equality(
                                CheckedDomainStateV1 {
                                    goal: CheckedDomainGoalV1::Equal {
                                        actual: rhs,
                                        expected: atom.rhs,
                                    },
                                    ..base
                                },
                                budget,
                            )?;
                        if direct && (!equal || truth) {
                            incoming.observe(i, if equal { false } else { truth });
                        } else if truth {
                            let reverse = self.equality(
                                CheckedDomainStateV1 {
                                    goal: CheckedDomainGoalV1::Equal {
                                        actual: lhs,
                                        expected: atom.rhs,
                                    },
                                    ..base
                                },
                                budget,
                            )? && self.equality(
                                CheckedDomainStateV1 {
                                    goal: CheckedDomainGoalV1::Equal {
                                        actual: rhs,
                                        expected: atom.lhs,
                                    },
                                    ..base
                                },
                                budget,
                            )?;
                            if reverse {
                                incoming.observe(i, false);
                            }
                        }
                    }
                }
                if incoming.proven() {
                    continue;
                }
                if incoming.clauses == 0 {
                    return Ok(false);
                }
                checked_domain_enqueue_v1(
                    visited,
                    CheckedDomainStateV1 {
                        block: edge.block,
                        environment,
                        goal: CheckedDomainGoalV1::Predicate(incoming),
                    },
                    budget,
                )?;
            }
        }
        // Revisits close only a universal safety cycle. Entry and every fresh
        // definition are rejecting boundaries unless an exact edge discharged
        // the residual first. No visited bit is exported as equation authority.
        Ok(true)
    }

    fn edge_guard(
        &self,
        edge: &PredecessorEdge,
        budget: &mut RankedBoundsBudget,
    ) -> Result<Option<(Value, Value, bool, bool)>, RankedBoundsFindingV1> {
        budget.work(16)?;
        let terminator = self.graph.blocks[edge.block]
            .deref(self.graph.context)
            .get_terminator(self.graph.context)
            .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
        let less = Operation::is_op::<IndexLessThanBranchOp>(terminator, self.graph.context)
            || Operation::is_op::<IndexLessThanBranchArgsOp>(terminator, self.graph.context);
        let equal = Operation::is_op::<IndexEqualBranchOp>(terminator, self.graph.context)
            || Operation::is_op::<IndexEqualBranchArgsOp>(terminator, self.graph.context);
        if !less && !equal {
            return Ok(None);
        }
        let raw = terminator.deref(self.graph.context);
        if raw.get_num_operands() < 2 || edge.successor >= 2 {
            return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
        }
        Ok(Some((
            raw.get_operand(0),
            raw.get_operand(1),
            equal,
            edge.successor == 0,
        )))
    }
}
