type CheckedDomainEnvironmentV1 = [IndexExpr; 6];
const CHECKED_DOMAIN_STATE_ITEMS_V1: usize = 64;
const CHECKED_DOMAIN_MEMO_ROWS_V1: usize = 32;
// Full owner-aware Ptr plus Option tag; never compress to an arena index.
const CHECKED_DOMAIN_TERMINATOR_ROW_ITEMS_V1: usize = 3;
const CHECKED_DOMAIN_FOLD_CACHE_ITEMS_V1: usize = 6 * 2 + CHECKED_DOMAIN_NODES_V1 * 2 + 4;
// A fold never invokes another fold or solver query. Its projected key and
// borrowed bookkeeping need one 16-cell scratch owner, not one per depth.
const CHECKED_DOMAIN_FOLD_SCRATCH_ITEMS_V1: usize = 16;
// Formula -> equality keeps the level; every further recursive dependency
// strictly decreases the expected node number. CFG traversal itself is FIFO.
// Six state-shaped argument/local slots, two checked-fold buffers, and 144
// scalar/borrowed bookkeeping items. This is a logical payload bound, not
// a promise about compiler-selected stack padding or allocator bytes.
const CHECKED_DOMAIN_FRAME_ITEMS_V1: usize = 768;
const CHECKED_DOMAIN_WORKLISTS_V1: usize = CHECKED_DOMAIN_NODES_V1 * 2;
const CHECKED_DOMAIN_QUERY_ITEMS_V1: usize = CHECKED_DOMAIN_MEMO_ROWS_V1
    * CHECKED_DOMAIN_STATE_ITEMS_V1
    + CHECKED_DOMAIN_WORKLISTS_V1 * 3
    + CHECKED_DOMAIN_FOLD_CACHE_ITEMS_V1
    + CHECKED_DOMAIN_FOLD_SCRATCH_ITEMS_V1
    + 3
    + 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedDomainResidualV1 {
    formula: u8,
    positive: [u8; 2],
    negative: [u8; 2],
    clauses: u8,
}

impl CheckedDomainResidualV1 {
    fn new(formula: u8, dag: &CheckedDomainDagV1) -> Self {
        let value = dag.formulas[usize::from(formula)];
        Self {
            formula,
            positive: value.positive,
            negative: value.negative,
            clauses: value.clauses,
        }
    }

    fn observe(&mut self, atom: usize, value: bool) {
        let bit = 1 << atom;
        for clause in 0..2 {
            if self.clauses & (1 << clause) == 0 {
                continue;
            }
            let false_literal = if value {
                self.negative[clause]
            } else {
                self.positive[clause]
            };
            if false_literal & bit != 0 {
                self.clauses &= !(1 << clause);
            } else {
                self.positive[clause] &= !bit;
                self.negative[clause] &= !bit;
            }
        }
    }

    fn proven(self) -> bool {
        (0..2).any(|clause| {
            self.clauses & (1 << clause) != 0
                && (self.positive[clause] | self.negative[clause]) == 0
        })
    }

    fn used(self) -> u8 {
        (0..2)
            .filter(|clause| self.clauses & (1 << clause) != 0)
            .fold(0, |bits, clause| {
                bits | self.positive[clause] | self.negative[clause]
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckedDomainGoalV1 {
    Equal { actual: IndexExpr, expected: u8 },
    Defined(IndexExpr),
    Predicate(CheckedDomainResidualV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedDomainStateV1 {
    block: usize,
    environment: CheckedDomainEnvironmentV1,
    goal: CheckedDomainGoalV1,
}

#[derive(Clone, Copy)]
struct CheckedDomainMemoV1 {
    state: CheckedDomainStateV1,
    result: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedDomainFoldCacheV1 {
    key: [Option<u64>; 6],
    values: [Option<u64>; CHECKED_DOMAIN_NODES_V1],
}

fn checked_domain_memo_key_equal_v1(
    lhs: &CheckedDomainStateV1,
    rhs: &CheckedDomainStateV1,
    budget: &mut RankedBoundsBudget,
) -> Result<bool, RankedBoundsFindingV1> {
    // Compare the same complete key, but stop before unrelated payloads.
    // These bounded field costs partition the existing 66-unit comparison:
    // block/control2 + goal16 + six IndexExpr slots8 each. Charge each
    // field before inspecting it; no graph dereference or new fact occurs.
    budget.work(2)?;
    if lhs.block != rhs.block {
        return Ok(false);
    }
    budget.work(16)?;
    if lhs.goal != rhs.goal {
        return Ok(false);
    }
    for (lhs, rhs) in lhs.environment.iter().zip(&rhs.environment) {
        budget.work(8)?;
        if lhs != rhs {
            return Ok(false);
        }
    }
    Ok(true)
}

struct CheckedDomainProofV1<'a, 'graph> {
    graph: &'a BoundsEdgeTransportV1<'graph>,
    dag: CheckedDomainDagV1,
    memo: [Option<CheckedDomainMemoV1>; CHECKED_DOMAIN_MEMO_ROWS_V1],
    fold_cache: Option<CheckedDomainFoldCacheV1>,
    authenticated_terminators: Vec<Option<pliron::context::Ptr<Operation>>>,
    next_memo: usize,
    worklists: [Vec<CheckedDomainStateV1>; CHECKED_DOMAIN_WORKLISTS_V1],
    active_queries: usize,
    depth_limit: usize,
}

fn checked_domain_enqueue_v1(
    visited: &mut Vec<CheckedDomainStateV1>,
    state: CheckedDomainStateV1,
    budget: &mut RankedBoundsBudget,
) -> Result<(), RankedBoundsFindingV1> {
    budget.work(bounds_transport_sum_v1(
        bounds_transport_product_v1(visited.len(), CHECKED_DOMAIN_STATE_ITEMS_V1 + 1)?,
        1,
    )?)?;
    if visited.contains(&state) {
        return Ok(());
    }
    if visited.len() == MAX_RANKED_BOUNDS_FACTS {
        return Err(bounds_transport_failure_v1(
            "checked domain obligation",
            MAX_RANKED_BOUNDS_FACTS,
            MAX_RANKED_BOUNDS_FACTS + 1,
        ));
    }
    if visited.len() == visited.capacity() {
        let capacity =
            bounds_transport_product_v1(visited.capacity(), 2)?.clamp(4, MAX_RANKED_BOUNDS_FACTS);
        budget.storage(bounds_transport_product_v1(
            capacity,
            CHECKED_DOMAIN_STATE_ITEMS_V1,
        )?)?;
        budget.work(bounds_transport_sum_v1(
            bounds_transport_product_v1(visited.len(), CHECKED_DOMAIN_STATE_ITEMS_V1)?,
            bounds_transport_sum_v1(capacity, 1)?,
        )?)?;
        visited
            .try_reserve_exact(capacity - visited.len())
            .map_err(|_| {
                bounds_transport_failure_v1(
                    "analysis storage item",
                    MAX_RANKED_BOUNDS_STORAGE_ITEMS,
                    usize::MAX,
                )
            })?;
    }
    // Publication and eventual drop traversal are both prepaid. No mutation
    // follows a failed work/storage admission; the existing meter owns errors.
    budget.work(CHECKED_DOMAIN_STATE_ITEMS_V1 * 2 + 1)?;
    visited.push(state);
    Ok(())
}

impl<'a, 'graph> CheckedDomainProofV1<'a, 'graph> {
    fn new(
        graph: &'a BoundsEdgeTransportV1<'graph>,
        dag: CheckedDomainDagV1,
        budget: &mut RankedBoundsBudget,
    ) -> Result<Self, RankedBoundsFindingV1> {
        budget.work(CHECKED_DOMAIN_FORMULAS_V1)?;
        let depth = dag.formulas[..dag.formula_count]
            .iter()
            .map(|formula| usize::from(formula.depth))
            .max()
            .unwrap_or(0)
            .max(1);
        if depth > CHECKED_DOMAIN_WORKLISTS_V1 {
            return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
        }
        budget.work(4)?;
        let blocks = graph.blocks.len();
        if blocks > MAX_RANKED_BOUNDS_BLOCKS {
            return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
        }
        let scratch = bounds_transport_sum_v1(
            CHECKED_DOMAIN_QUERY_ITEMS_V1,
            bounds_transport_sum_v1(
                bounds_transport_product_v1(depth, CHECKED_DOMAIN_FRAME_ITEMS_V1)?,
                bounds_transport_product_v1(blocks, CHECKED_DOMAIN_TERMINATOR_ROW_ITEMS_V1)?,
            )?,
        )?;
        budget.storage(scratch)?;
        budget.work(bounds_transport_sum_v1(
            bounds_transport_product_v1(scratch, 2)?,
            1,
        )?)?;
        let mut authenticated_terminators = Vec::new();
        authenticated_terminators
            .try_reserve_exact(blocks)
            .map_err(|_| {
                bounds_transport_failure_v1(
                    "analysis storage item",
                    MAX_RANKED_BOUNDS_STORAGE_ITEMS,
                    usize::MAX,
                )
            })?;
        authenticated_terminators.resize(blocks, None);
        Ok(Self {
            graph,
            dag,
            memo: [None; CHECKED_DOMAIN_MEMO_ROWS_V1],
            fold_cache: None,
            authenticated_terminators,
            next_memo: 0,
            worklists: core::array::from_fn(|_| Vec::new()),
            active_queries: 0,
            depth_limit: depth,
        })
    }

    fn with_worklist<T>(
        &mut self,
        budget: &mut RankedBoundsBudget,
        query: impl FnOnce(
            &mut Self,
            &mut Vec<CheckedDomainStateV1>,
            &mut RankedBoundsBudget,
        ) -> Result<T, RankedBoundsFindingV1>,
    ) -> Result<T, RankedBoundsFindingV1> {
        // Take/return metadata is prepaid. Published rows already prepay
        // their clear/drop work; restoration must not itself fail admission.
        budget.work(32)?;
        let depth = self.active_queries;
        if depth >= self.depth_limit {
            return Err(bounds_transport_failure_v1(
                "checked domain worklist depth",
                self.depth_limit,
                depth + 1,
            ));
        }
        let mut visited = core::mem::take(&mut self.worklists[depth]);
        self.active_queries += 1;
        let result = query(self, &mut visited, budget);
        visited.clear();
        self.active_queries -= 1;
        self.worklists[depth] = visited;
        result
    }

    fn memoized(
        &self,
        state: CheckedDomainStateV1,
        budget: &mut RankedBoundsBudget,
    ) -> Result<Option<bool>, RankedBoundsFindingV1> {
        budget.work(CHECKED_DOMAIN_MEMO_ROWS_V1)?;
        for entry in self.memo.iter().flatten() {
            if checked_domain_memo_key_equal_v1(&entry.state, &state, budget)? {
                return Ok(Some(entry.result));
            }
        }
        Ok(None)
    }

    fn remember(
        &mut self,
        state: CheckedDomainStateV1,
        result: bool,
        budget: &mut RankedBoundsBudget,
    ) -> Result<bool, RankedBoundsFindingV1> {
        budget.work(CHECKED_DOMAIN_STATE_ITEMS_V1 + 3)?;
        self.memo[self.next_memo] = Some(CheckedDomainMemoV1 { state, result });
        self.next_memo = (self.next_memo + 1) % CHECKED_DOMAIN_MEMO_ROWS_V1;
        Ok(result)
    }

    fn owned_value(
        &self,
        value: Value,
        budget: &mut RankedBoundsBudget,
    ) -> Result<pliron::context::Ptr<pliron::basic_block::BasicBlock>, RankedBoundsFindingV1> {
        budget.work(12)?;
        let parent = if let Some(operation) = value.defining_op() {
            operation
                .try_deref(self.graph.context)
                .map_err(|_| RankedBoundsFindingV1::StructuralVerificationFailed)?
                .get_parent_block()
                .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?
        } else {
            value
                .defining_block()
                .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?
        };
        let block = parent
            .try_deref(self.graph.context)
            .map_err(|_| RankedBoundsFindingV1::StructuralVerificationFailed)?;
        let region = self
            .graph
            .blocks
            .first()
            .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?
            .try_deref(self.graph.context)
            .map_err(|_| RankedBoundsFindingV1::StructuralVerificationFailed)?
            .get_parent_region()
            .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
        if block.get_parent_region() != Some(region) {
            return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
        }
        let roster = self.graph.roster_len(value)?;
        budget.work(bounds_transport_sum_v1(roster, 1)?)?;
        value
            .try_find_index(self.graph.context)
            .map_err(|_| RankedBoundsFindingV1::StructuralVerificationFailed)?;
        Ok(parent)
    }

    fn canonical(
        &self,
        value: Value,
        budget: &mut RankedBoundsBudget,
    ) -> Result<IndexExpr, RankedBoundsFindingV1> {
        self.owned_value(value, budget)?;
        budget.work(bounds_transport_sum_v1(self.graph.roster_len(value)?, 2)?)?;
        if !dialect_kernel::is_index_type(value, self.graph.context) {
            return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
        }
        if let Some(definition) = value.defining_op() {
            budget.work(8)?;
            if let Some(dimension) =
                Operation::get_op::<DimensionOp>(definition, self.graph.context)
            {
                let ordinal = dimension
                    .dimension(self.graph.context)
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
                self.validate_view_extent(dimension.view(self.graph.context), ordinal, budget)?;
            }
        }
        let result = self.graph.canonical(value, budget)?;
        if let IndexExpr::Value(value) | IndexExpr::Dimension { view: value, .. } = result {
            self.owned_value(value, budget)?;
        }
        Ok(result)
    }

    fn validate_view_extent(
        &self,
        view: Value,
        dimension: usize,
        budget: &mut RankedBoundsBudget,
    ) -> Result<(), RankedBoundsFindingV1> {
        self.owned_value(view, budget)?;
        budget.work(bounds_transport_sum_v1(
            self.graph.roster_len(view)?,
            MAX_RANKED_MEMORY_RANK + 12,
        )?)?;
        let ty = ranked_view_type(view, self.graph.context)
            .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
        let ty = ty.deref(self.graph.context);
        let extent = ty
            .shape()
            .get(dimension)
            .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
        if *extent == dialect_kernel::DYNAMIC_EXTENT
            && let Some(definition) = view.defining_op()
            && let Some(view) = Operation::get_op::<RankedViewOp>(definition, self.graph.context)
        {
            let extent = view
                .dynamic_extent(self.graph.context, dimension)
                .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
            self.owned_value(extent, budget)?;
        }
        Ok(())
    }

    fn fold(
        &mut self,
        environment: CheckedDomainEnvironmentV1,
        budget: &mut RankedBoundsBudget,
    ) -> Result<[Option<u64>; CHECKED_DOMAIN_NODES_V1], RankedBoundsFindingV1> {
        // Only literal projections affect this pure immutable-DAG evaluation.
        // None carries no SSA equality, definedness, guard, or owner authority.
        budget.work(6 * 8 + 1)?;
        let key = core::array::from_fn(|role| match &environment[role] {
            IndexExpr::Constant(value) => Some(*value),
            _ => None,
        });
        budget.work(1)?;
        if let Some(cache) = &self.fold_cache {
            let mut equal = true;
            for (previous, current) in cache.key.iter().zip(&key) {
                budget.work(4)?;
                if previous != current {
                    equal = false;
                    break;
                }
            }
            if equal {
                budget.work(CHECKED_DOMAIN_NODES_V1 * 2)?;
                return Ok(cache.values);
            }
        }
        budget.work(CHECKED_DOMAIN_NODES_V1 * 8)?;
        let mut values = [None; CHECKED_DOMAIN_NODES_V1];
        for (index, node) in self.dag.nodes[..self.dag.node_count].iter().enumerate() {
            values[index] = match node.kind {
                CheckedDomainNodeKindV1::Leaf(role) => match environment[usize::from(role)] {
                    IndexExpr::Constant(value) => Some(value),
                    _ => None,
                },
                CheckedDomainNodeKindV1::Constant(value) => Some(value),
                CheckedDomainNodeKindV1::Binary { kind, lhs, rhs } => values[usize::from(lhs)]
                    .zip(values[usize::from(rhs)])
                    .and_then(|(lhs, rhs)| match kind {
                        IndexBinaryKindAttr::Add => lhs.checked_add(rhs),
                        IndexBinaryKindAttr::Multiply => lhs.checked_mul(rhs),
                        IndexBinaryKindAttr::Divide => lhs.checked_div(rhs),
                        IndexBinaryKindAttr::Remainder => lhs.checked_rem(rhs),
                    }),
            };
        }
        // Copy into the one-entry cache only after the entire write is paid.
        // Initialization/drop and key scratch were admitted by the proof owner.
        budget.work(6 * 2 + CHECKED_DOMAIN_NODES_V1 * 2 + 2)?;
        self.fold_cache = Some(CheckedDomainFoldCacheV1 { key, values });
        Ok(values)
    }

    fn killed(
        &self,
        environment: CheckedDomainEnvironmentV1,
        block: usize,
        budget: &mut RankedBoundsBudget,
    ) -> Result<bool, RankedBoundsFindingV1> {
        for expression in environment {
            budget.work(1)?;
            if self.graph.defined_in_block(expression, block, budget)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn authenticate_terminator(
        &mut self,
        block: usize,
        budget: &mut RankedBoundsBudget,
    ) -> Result<(), RankedBoundsFindingV1> {
        // Only completed ownership/metadata checks survive within this
        // immutable graph owner. No edge, guard or substitution is cached.
        budget.work(12)?;
        let parent = self
            .graph
            .blocks
            .get(block)
            .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
        let terminator = parent
            .try_deref(self.graph.context)
            .map_err(|_| RankedBoundsFindingV1::StructuralVerificationFailed)?
            .get_terminator(self.graph.context)
            .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
        let cached = self
            .authenticated_terminators
            .get(block)
            .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
        if let Some(previous) = cached {
            return if *previous == terminator {
                Ok(())
            } else {
                Err(RankedBoundsFindingV1::StructuralVerificationFailed)
            };
        }
        self.authenticate_terminator_uncached(terminator, budget)?;
        budget.work(CHECKED_DOMAIN_TERMINATOR_ROW_ITEMS_V1 + 2)?;
        self.authenticated_terminators[block] = Some(terminator);
        Ok(())
    }

    fn authenticate_terminator_uncached(
        &self,
        terminator: pliron::context::Ptr<Operation>,
        budget: &mut RankedBoundsBudget,
    ) -> Result<(), RankedBoundsFindingV1> {
        // Canonicalization can turn an incoming constant into an ownerless
        // atom. Authenticate the exact edge's values before that conversion.
        budget.work(8)?;
        let raw = terminator
            .try_deref(self.graph.context)
            .map_err(|_| RankedBoundsFindingV1::StructuralVerificationFailed)?;
        budget.work(bounds_transport_sum_v1(raw.get_num_operands(), 1)?)?;
        for value in raw.operands() {
            self.owned_value(value, budget)?;
            // Exact-edge substitution uses the shared canonicalizer. Check
            // Dimension metadata before it can fold away its owner identity.
            budget.work(8)?;
            if let Some(definition) = value.defining_op()
                && let Some(dimension) =
                    Operation::get_op::<DimensionOp>(definition, self.graph.context)
            {
                let ordinal = dimension
                    .dimension(self.graph.context)
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
                self.validate_view_extent(dimension.view(self.graph.context), ordinal, budget)?;
            }
            budget.work(8)?;
            if let Some(definition) = value.defining_op()
                && Operation::is_op::<RankedViewOp>(definition, self.graph.context)
            {
                budget.work(bounds_transport_sum_v1(
                    self.graph.roster_len(value)?,
                    MAX_RANKED_MEMORY_RANK + 4,
                )?)?;
                let ty = ranked_view_type(value, self.graph.context)
                    .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
                for dimension in 0..ty.deref(self.graph.context).shape().len() {
                    self.validate_view_extent(value, dimension, budget)?;
                }
            }
        }
        Ok(())
    }

    fn pull_environment(
        &mut self,
        state: CheckedDomainStateV1,
        edge: &PredecessorEdge,
        budget: &mut RankedBoundsBudget,
    ) -> Result<(CheckedDomainEnvironmentV1, Option<IndexExpr>), RankedBoundsFindingV1> {
        self.authenticate_terminator(edge.block, budget)?;
        let mut environment = state.environment;
        for pair in 0..3 {
            let (fact, _) = self.graph.pull_back(
                BoundsTransportObligationV1 {
                    block: state.block,
                    fact: LessThanFact {
                        lhs: environment[pair * 2],
                        rhs: environment[pair * 2 + 1],
                    },
                },
                edge,
                budget,
            )?;
            environment[pair * 2] = fact.lhs;
            environment[pair * 2 + 1] = fact.rhs;
        }
        let actual = if let CheckedDomainGoalV1::Equal { actual, .. }
        | CheckedDomainGoalV1::Defined(actual) = state.goal
        {
            let (fact, _) = self.graph.pull_back(
                BoundsTransportObligationV1 {
                    block: state.block,
                    fact: LessThanFact {
                        lhs: actual,
                        rhs: IndexExpr::Constant(0),
                    },
                },
                edge,
                budget,
            )?;
            Some(fact.lhs)
        } else {
            None
        };
        budget.work(CHECKED_DOMAIN_STATE_ITEMS_V1)?;
        for expression in environment.into_iter().chain(actual) {
            if let IndexExpr::Value(value) | IndexExpr::Dimension { view: value, .. } = expression {
                self.owned_value(value, budget)?;
            }
        }
        Ok((environment, actual))
    }
}
