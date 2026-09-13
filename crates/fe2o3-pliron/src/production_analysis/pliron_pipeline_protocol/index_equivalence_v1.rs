fn index_offset(
    context: &Context,
    value: Value,
    base: Value,
    equivalence_resources: &mut EquivalenceResourceMeterV1,
) -> Option<u64> {
    if index_values_equivalent(context, value, base, equivalence_resources) {
        return Some(0);
    }
    let definition = value.defining_op()?;
    let operation = Operation::get_op_dyn(definition, context);
    let add = operation.downcast_ref::<IndexBinaryOp>()?;
    if add.kind(context) != Some(IndexBinaryKindAttr::Add) {
        return None;
    }
    if index_values_equivalent(context, add.lhs(context), base, equivalence_resources) {
        index_constant(context, add.rhs(context))
    } else if index_values_equivalent(context, add.rhs(context), base, equivalence_resources) {
        index_constant(context, add.lhs(context))
    } else {
        None
    }
}

type EquivalencePairV1 = (Value, Value);

#[derive(Clone, Copy)]
struct EquivalenceBinaryPlanV1 {
    pair: EquivalencePairV1,
    direct_lhs: EquivalencePairV1,
    direct_rhs: EquivalencePairV1,
    commutative: Option<(EquivalencePairV1, EquivalencePairV1)>,
    same_kind: bool,
}

#[derive(Clone, Copy)]
enum EquivalenceCursorV1 {
    Enter(EquivalencePairV1),
    FinishSingle {
        pair: EquivalencePairV1,
        dependency: EquivalencePairV1,
    },
    AfterDirectLhs(EquivalenceBinaryPlanV1),
    AfterDirectRhs(EquivalenceBinaryPlanV1),
    AfterCommutativeLhs {
        plan: EquivalenceBinaryPlanV1,
        direct: bool,
    },
    AfterCommutativeRhs {
        plan: EquivalenceBinaryPlanV1,
        direct: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EquivalenceVisitLimitV1;

struct EquivalenceResourceMeterV1 {
    query_limit: usize,
    cursor_step_limit: usize,
    unique_pair_limit: usize,
    queries: usize,
    cursor_steps: usize,
    expanded_pairs: usize,
    memo: HashMap<EquivalencePairV1, bool>,
    exhausted: bool,
}

impl EquivalenceResourceMeterV1 {
    fn new(query_limit: usize, unique_pair_limit: usize) -> Result<Self, EquivalenceVisitLimitV1> {
        let cursor_step_limit = query_limit
            .checked_add(
                unique_pair_limit
                    .checked_mul(8)
                    .ok_or(EquivalenceVisitLimitV1)?,
            )
            .ok_or(EquivalenceVisitLimitV1)?;
        Ok(Self {
            query_limit,
            cursor_step_limit,
            unique_pair_limit,
            queries: 0,
            cursor_steps: 0,
            expanded_pairs: 0,
            memo: HashMap::new(),
            exhausted: false,
        })
    }

    fn begin_query(&mut self) -> Result<(), EquivalenceVisitLimitV1> {
        self.queries = self.queries.checked_add(1).ok_or(EquivalenceVisitLimitV1)?;
        if self.queries > self.query_limit {
            self.exhausted = true;
            return Err(EquivalenceVisitLimitV1);
        }
        Ok(())
    }

    fn charge_expansion(&mut self) -> Result<(), EquivalenceVisitLimitV1> {
        self.expanded_pairs = self
            .expanded_pairs
            .checked_add(1)
            .ok_or(EquivalenceVisitLimitV1)?;
        if self.expanded_pairs > self.unique_pair_limit {
            self.exhausted = true;
            return Err(EquivalenceVisitLimitV1);
        }
        Ok(())
    }

    fn charge_cursor(
        &mut self,
        query_cursor_steps: &mut usize,
        query_cursor_limit: usize,
    ) -> Result<(), EquivalenceVisitLimitV1> {
        *query_cursor_steps = query_cursor_steps
            .checked_add(1)
            .ok_or(EquivalenceVisitLimitV1)?;
        self.cursor_steps = self
            .cursor_steps
            .checked_add(1)
            .ok_or(EquivalenceVisitLimitV1)?;
        if *query_cursor_steps > query_cursor_limit || self.cursor_steps > self.cursor_step_limit {
            self.exhausted = true;
            return Err(EquivalenceVisitLimitV1);
        }
        Ok(())
    }

    const fn exhausted(&self) -> bool {
        self.exhausted
    }

    fn mark_exhausted(&mut self) {
        self.exhausted = true;
    }
}

fn push_equivalence_cursor_v1(
    pending: &mut Vec<EquivalenceCursorV1>,
    cursor: EquivalenceCursorV1,
    cursor_limit: usize,
) -> Result<(), EquivalenceVisitLimitV1> {
    if pending.len() >= cursor_limit {
        return Err(EquivalenceVisitLimitV1);
    }
    pending
        .try_reserve(1)
        .map_err(|_| EquivalenceVisitLimitV1)?;
    pending.push(cursor);
    Ok(())
}

fn cache_equivalence_result_v1(
    memo: &mut HashMap<EquivalencePairV1, bool>,
    pair: EquivalencePairV1,
    result: bool,
) -> Result<(), EquivalenceVisitLimitV1> {
    if !memo.contains_key(&pair) {
        memo.try_reserve(1).map_err(|_| EquivalenceVisitLimitV1)?;
    }
    memo.insert(pair, result);
    Ok(())
}

fn equivalence_pair_result_v1(
    memo: &HashMap<EquivalencePairV1, bool>,
    pair: EquivalencePairV1,
) -> Option<bool> {
    (pair.0 == pair.1)
        .then_some(true)
        .or_else(|| memo.get(&pair).copied())
}

fn finish_equivalence_pair_v1(
    memo: &mut HashMap<EquivalencePairV1, bool>,
    active: &mut HashSet<EquivalencePairV1>,
    pair: EquivalencePairV1,
    result: bool,
) -> Result<(), EquivalenceVisitLimitV1> {
    active.remove(&pair);
    cache_equivalence_result_v1(memo, pair, result)
}

fn evaluate_index_equivalence_v1(
    context: &Context,
    left: Value,
    right: Value,
    visit_limit: usize,
    resources: &mut EquivalenceResourceMeterV1,
) -> Result<bool, EquivalenceVisitLimitV1> {
    resources.begin_query()?;
    let cursor_limit = visit_limit.checked_add(1).ok_or(EquivalenceVisitLimitV1)?;
    let query_cursor_limit = visit_limit
        .checked_mul(9)
        .and_then(|steps| steps.checked_add(1))
        .ok_or(EquivalenceVisitLimitV1)?;
    let root = (left, right);
    let mut visits = 0_usize;
    let mut query_cursor_steps = 0_usize;
    let mut active = HashSet::<EquivalencePairV1>::new();
    let mut pending = Vec::new();
    push_equivalence_cursor_v1(&mut pending, EquivalenceCursorV1::Enter(root), cursor_limit)?;

    while let Some(cursor) = pending.pop() {
        resources.charge_cursor(&mut query_cursor_steps, query_cursor_limit)?;
        match cursor {
            EquivalenceCursorV1::Enter(pair) => {
                if pair.0 == pair.1 || resources.memo.contains_key(&pair) {
                    continue;
                }
                if active.contains(&pair) {
                    cache_equivalence_result_v1(&mut resources.memo, pair, false)?;
                    continue;
                }
                visits = visits.checked_add(1).ok_or(EquivalenceVisitLimitV1)?;
                if visits > visit_limit {
                    resources.mark_exhausted();
                    return Err(EquivalenceVisitLimitV1);
                }
                resources.charge_expansion()?;
                active.try_reserve(1).map_err(|_| EquivalenceVisitLimitV1)?;
                active.insert(pair);

                let transparent_source = |value: Value| {
                    let definition = value.defining_op()?;
                    let operation = Operation::get_op_dyn(definition, context);
                    let join = operation.downcast_ref::<DeterministicJoinOp>()?;
                    let dependencies = join.dependencies(context);
                    (dependencies.len() == 1).then(|| dependencies[0])
                };
                if let Some(source) = transparent_source(pair.0) {
                    let dependency = (source, pair.1);
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::FinishSingle { pair, dependency },
                        cursor_limit,
                    )?;
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::Enter(dependency),
                        cursor_limit,
                    )?;
                    continue;
                }
                if let Some(source) = transparent_source(pair.1) {
                    let dependency = (pair.0, source);
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::FinishSingle { pair, dependency },
                        cursor_limit,
                    )?;
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::Enter(dependency),
                        cursor_limit,
                    )?;
                    continue;
                }

                let (Some(left_definition), Some(right_definition)) =
                    (pair.0.defining_op(), pair.1.defining_op())
                else {
                    finish_equivalence_pair_v1(&mut resources.memo, &mut active, pair, false)?;
                    continue;
                };
                let left_operation = Operation::get_op_dyn(left_definition, context);
                let right_operation = Operation::get_op_dyn(right_definition, context);
                if let (Some(left), Some(right)) = (
                    left_operation.downcast_ref::<IndexConstantOp>(),
                    right_operation.downcast_ref::<IndexConstantOp>(),
                ) {
                    finish_equivalence_pair_v1(
                        &mut resources.memo,
                        &mut active,
                        pair,
                        left.value(context) == right.value(context),
                    )?;
                } else if let (Some(left), Some(right)) = (
                    left_operation.downcast_ref::<IndexUnsignedCastOp>(),
                    right_operation.downcast_ref::<IndexUnsignedCastOp>(),
                ) {
                    if left.bit_width(context) != right.bit_width(context) {
                        finish_equivalence_pair_v1(&mut resources.memo, &mut active, pair, false)?;
                        continue;
                    }
                    let dependency = (left.source(context), right.source(context));
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::FinishSingle { pair, dependency },
                        cursor_limit,
                    )?;
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::Enter(dependency),
                        cursor_limit,
                    )?;
                } else if let (Some(left), Some(right)) = (
                    left_operation.downcast_ref::<IndexBinaryOp>(),
                    right_operation.downcast_ref::<IndexBinaryOp>(),
                ) {
                    let plan = EquivalenceBinaryPlanV1 {
                        pair,
                        direct_lhs: (left.lhs(context), right.lhs(context)),
                        direct_rhs: (left.rhs(context), right.rhs(context)),
                        commutative: matches!(
                            left.kind(context),
                            Some(IndexBinaryKindAttr::Add | IndexBinaryKindAttr::Multiply)
                        )
                        .then_some((
                            (left.lhs(context), right.rhs(context)),
                            (left.rhs(context), right.lhs(context)),
                        )),
                        same_kind: left.kind(context) == right.kind(context),
                    };
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::AfterDirectLhs(plan),
                        cursor_limit,
                    )?;
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::Enter(plan.direct_lhs),
                        cursor_limit,
                    )?;
                } else {
                    finish_equivalence_pair_v1(&mut resources.memo, &mut active, pair, false)?;
                }
            }
            EquivalenceCursorV1::FinishSingle { pair, dependency } => {
                let result =
                    equivalence_pair_result_v1(&resources.memo, dependency).unwrap_or(false);
                finish_equivalence_pair_v1(&mut resources.memo, &mut active, pair, result)?;
            }
            EquivalenceCursorV1::AfterDirectLhs(plan) => {
                if equivalence_pair_result_v1(&resources.memo, plan.direct_lhs) == Some(true) {
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::AfterDirectRhs(plan),
                        cursor_limit,
                    )?;
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::Enter(plan.direct_rhs),
                        cursor_limit,
                    )?;
                } else if let Some((commutative_lhs, _)) = plan.commutative {
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::AfterCommutativeLhs {
                            plan,
                            direct: false,
                        },
                        cursor_limit,
                    )?;
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::Enter(commutative_lhs),
                        cursor_limit,
                    )?;
                } else {
                    finish_equivalence_pair_v1(&mut resources.memo, &mut active, plan.pair, false)?;
                }
            }
            EquivalenceCursorV1::AfterDirectRhs(plan) => {
                let direct =
                    equivalence_pair_result_v1(&resources.memo, plan.direct_rhs) == Some(true);
                if let Some((commutative_lhs, _)) = plan.commutative {
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::AfterCommutativeLhs { plan, direct },
                        cursor_limit,
                    )?;
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::Enter(commutative_lhs),
                        cursor_limit,
                    )?;
                } else {
                    finish_equivalence_pair_v1(
                        &mut resources.memo,
                        &mut active,
                        plan.pair,
                        plan.same_kind && direct,
                    )?;
                }
            }
            EquivalenceCursorV1::AfterCommutativeLhs { plan, direct } => {
                let Some((commutative_lhs, commutative_rhs)) = plan.commutative else {
                    unreachable!("only a commutative plan schedules this cursor")
                };
                if equivalence_pair_result_v1(&resources.memo, commutative_lhs) == Some(true) {
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::AfterCommutativeRhs { plan, direct },
                        cursor_limit,
                    )?;
                    push_equivalence_cursor_v1(
                        &mut pending,
                        EquivalenceCursorV1::Enter(commutative_rhs),
                        cursor_limit,
                    )?;
                } else {
                    finish_equivalence_pair_v1(
                        &mut resources.memo,
                        &mut active,
                        plan.pair,
                        plan.same_kind && direct,
                    )?;
                }
            }
            EquivalenceCursorV1::AfterCommutativeRhs { plan, direct } => {
                let Some((_, commutative_rhs)) = plan.commutative else {
                    unreachable!("only a commutative plan schedules this cursor")
                };
                let commutative =
                    equivalence_pair_result_v1(&resources.memo, commutative_rhs) == Some(true);
                finish_equivalence_pair_v1(
                    &mut resources.memo,
                    &mut active,
                    plan.pair,
                    plan.same_kind && (direct || commutative),
                )?;
            }
        }
    }
    Ok(equivalence_pair_result_v1(&resources.memo, root).unwrap_or(false))
}

fn index_values_equivalent(
    context: &Context,
    left: Value,
    right: Value,
    resources: &mut EquivalenceResourceMeterV1,
) -> bool {
    match evaluate_index_equivalence_v1(context, left, right, MAX_EQUIVALENCE_WORK_V1, resources) {
        Ok(result) => result,
        Err(_) => {
            resources.mark_exhausted();
            false
        }
    }
}

fn index_constant(context: &Context, value: Value) -> Option<u64> {
    let definition = value.defining_op()?;
    Operation::get_op_dyn(definition, context)
        .downcast_ref::<IndexConstantOp>()?
        .value(context)
}
