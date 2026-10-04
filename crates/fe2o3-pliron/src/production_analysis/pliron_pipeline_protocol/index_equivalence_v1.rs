use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::InvocationObserverV1;

type ProtocolObserverV1<'o, 'p, 'r> = Option<&'o InvocationObserverV1<'p, 'r>>;

fn observe_protocol_quota_v1(observer: ProtocolObserverV1<'_, '_, '_>, resource: &'static str) {
    if let Some(observer) = observer {
        observer.deny(ProductionAnalysisResourceLimitV1 {
            phase: ProductionAnalysisResourcePhaseV1::PipelineProtocol,
            resource,
        });
    }
}

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

struct EquivalenceResourceMeterV1<'o, 'p, 'r> {
    query_limit: usize,
    cursor_step_limit: usize,
    unique_pair_limit: usize,
    queries: usize,
    cursor_steps: usize,
    expanded_pairs: usize,
    memo: HashMap<EquivalencePairV1, bool>,
    exhausted: bool,
    observer: ProtocolObserverV1<'o, 'p, 'r>,
}

impl<'o, 'p, 'r> EquivalenceResourceMeterV1<'o, 'p, 'r> {
    #[cfg(test)]
    fn new(query_limit: usize, unique_pair_limit: usize) -> Result<Self, EquivalenceVisitLimitV1> {
        Self::new_with_observation_v1(query_limit, unique_pair_limit, None)
    }

    fn new_with_observation_v1(
        query_limit: usize,
        unique_pair_limit: usize,
        observer: ProtocolObserverV1<'o, 'p, 'r>,
    ) -> Result<Self, EquivalenceVisitLimitV1> {
        let overflow = || {
            observe_protocol_quota_v1(observer, "index equivalence cursor bound overflow");
            EquivalenceVisitLimitV1
        };
        let cursor_step_limit = query_limit
            .checked_add(unique_pair_limit.checked_mul(8).ok_or_else(overflow)?)
            .ok_or_else(overflow)?;
        Ok(Self {
            query_limit,
            cursor_step_limit,
            unique_pair_limit,
            queries: 0,
            cursor_steps: 0,
            expanded_pairs: 0,
            memo: HashMap::new(),
            exhausted: false,
            observer,
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
    let observer = resources.observer;
    let result = match observer {
        None => evaluate_index_equivalence_inner_v1(context, left, right, visit_limit, resources),
        Some(observer) => observer.with_projection(&Ok, |_| {
            evaluate_index_equivalence_inner_v1(context, left, right, visit_limit, resources)
        }),
    };
    if result.is_err() {
        observe_protocol_quota_v1(observer, "index equivalence evaluation quota");
    }
    result
}

fn evaluate_index_equivalence_inner_v1(
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

                // Dependency-only joins are opaque here, even with one input.
                // Uniformity dependence does not establish numeric equality.
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

#[cfg(test)]
mod observed_protocol_quota_tests {
    use super::*;
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1, InvocationReceiptV1,
    };
    use pliron::{builtin::types::FunctionType, op::Op};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn fixture() -> (Context, [Value; 3]) {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "observed_protocol".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        let left = IndexConstantOp::new(&mut context, 7);
        let right = IndexConstantOp::new(&mut context, 9);
        let lane = dialect_kernel::InvocationIndexOp::new(&mut context, 0, 8);
        for op in [
            left.get_operation(),
            right.get_operation(),
            lane.get_operation(),
            dialect_kernel::ReturnOp::new(&mut context).get_operation(),
        ] {
            op.insert_at_back(entry, &context);
        }
        let values = [
            left.result(&context),
            right.result(&context),
            lane.result(&context),
        ];
        (context, values)
    }

    #[test]
    fn standalone_preflight_observes_global_floor_before_manager_admission() {
        type Bound = ProductionAnalysisResourceUpperBoundV1;
        type Limits = ProductionAnalysisResourceLimitsV1;
        let owner = ProductionAnalysisResourcePhaseV1::PipelineProtocol;
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "observed_preflight".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        let ty = dialect_kernel::RankedViewType::new(&context, 16, true, vec![2, 64]).unwrap();
        let view = RankedViewOp::new_in_space(
            &mut context,
            ty,
            vec![],
            dialect_kernel::MemorySpaceAttr::Workgroup,
        )
        .unwrap();
        let value = view.result(&context);
        let create = PipelineCreateOp::new(&mut context, value, 2, 1).unwrap();
        let ret = dialect_kernel::ReturnOp::new(&mut context);
        for op in [
            view.get_operation(),
            create.get_operation(),
            ret.get_operation(),
        ] {
            op.insert_at_back(entry, &context);
        }
        let mut baseline = PlironAnalysisManagerV1::new(&function);
        assert!(baseline.input_census().is_none());
        let ordinary =
            run_pliron_pipeline_protocol_check_with_analyses_v1(&context, &function, &mut baseline);
        assert!(matches!(ordinary.findings(),
            [PlironPipelineProtocolFindingV1::InvalidSchedule { detail, .. }]
            if detail == "pipeline has no lifecycle events"));
        let h = baseline.resource_upper_bound();
        assert!(h.work_upper_bound() > 0 && h.retained_storage_upper_bound() > 0);
        let total = h.checked_then_retain(h, owner).unwrap();
        let held =
            Bound::checked_phase(owner, h.work_upper_bound(), h.peak_storage_upper_bound(), 0)
                .unwrap();
        for (dw, dp, resource) in [
            (0, 0, None),
            (1, 0, Some("work upper bound")),
            (0, 1, Some("peak storage upper bound")),
        ] {
            let limits = Limits::new(
                total.work_upper_bound() - dw,
                total.peak_storage_upper_bound() - dp,
            );
            let mut receipt = InvocationReceiptV1::new(h, limits).unwrap();
            let phase = receipt.phase(owner, 0).unwrap();
            let mut manager = PlironAnalysisManagerV1::new(&function);
            assert!(manager.input_census().is_none());
            let actual = run_pliron_pipeline_protocol_with_observation_v1(
                &context,
                &function,
                &mut manager,
                Some(&phase.observer(&Ok)),
            );
            // Retain the full peak: this boundary test transfers no cache owner.
            drop(phase);
            let state = receipt.snapshot();
            let denial = resource.map(|resource| ProductionAnalysisResourceLimitV1 {
                phase: owner,
                resource,
            });
            assert_eq!(state.first_denial, denial);
            assert!(!state.caught_panic);
            assert_eq!(state.current, state.committed);
            let expected = if denial.is_none() {
                held
            } else {
                Bound::default()
            };
            assert_eq!(state.committed, expected);
            assert_eq!(
                manager.resource_upper_bound(),
                if denial.is_none() {
                    h
                } else {
                    Bound::default()
                }
            );
            if let Some(error) = denial {
                assert_eq!(actual.status(), KernelCheckStatusV1::Incomplete);
                assert_eq!(
                    receipt.complete(),
                    Err(InvocationReceiptFailureV1::Denied(error))
                );
            } else {
                assert_eq!(actual, ordinary);
                assert_eq!(receipt.complete(), Ok(held));
            }
        }
        drop((ordinary, baseline));
    }

    #[test]
    fn actual_inventory_refusal_is_latched_before_incomplete_report() {
        use crate::production_analysis::pliron_function_inventory::{
            BoundedPlironFunctionInventoryFailureV1, MAX_PLIRON_FUNCTION_INVENTORY_OPERATIONS_V1,
        };
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "inventory_refusal".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        let limit = MAX_PLIRON_FUNCTION_INVENTORY_OPERATIONS_V1;
        for _ in 0..limit {
            IndexConstantOp::new(&mut context, 0)
                .get_operation()
                .insert_at_back(entry, &context);
        }
        dialect_kernel::ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(entry, &context);
        let mut receipt = InvocationReceiptV1::new(
            Default::default(),
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        let owner = ProductionAnalysisResourcePhaseV1::PipelineProtocol;
        let phase = receipt.phase(owner, 0).unwrap();
        let mut manager = PlironAnalysisManagerV1::new(&function);
        let actual = run_pliron_pipeline_protocol_with_observation_v1(
            &context,
            &function,
            &mut manager,
            Some(&phase.observer(&Ok)),
        );
        assert!(matches!(
            manager.function_inventory(),
            Err(BoundedPlironFunctionInventoryFailureV1::OperationLimit { actual, limit: cap })
                if actual == limit + 1 && cap == limit
        ));
        assert_eq!(actual.status(), KernelCheckStatusV1::Incomplete);
        drop(phase);
        let denial = ProductionAnalysisResourceLimitV1 {
            phase: owner,
            resource: "operation",
        };
        assert_eq!(receipt.snapshot().first_denial, Some(denial));
        assert!(!receipt.snapshot().caught_panic);
        // This quota control is not pre-census traversal admission evidence.
        assert_eq!(receipt.snapshot().committed, Default::default());
        assert_eq!(manager.resource_upper_bound(), Default::default());
        assert_eq!(
            receipt.complete(),
            Err(InvocationReceiptFailureV1::Denied(denial))
        );
    }

    #[test]
    fn actual_inequality_is_not_query_or_pair_exhaustion() {
        let (context, [left, right, _]) = fixture();
        for (queries, pairs, denied, counters) in [
            (1, 1, false, (1, 1, 1)),
            (0, 1, true, (1, 0, 0)),
            (1, 0, true, (1, 1, 1)),
        ] {
            let mut receipt = InvocationReceiptV1::new(
                Default::default(),
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
            .unwrap();
            let phase = receipt
                .phase(ProductionAnalysisResourcePhaseV1::PipelineProtocol, 0)
                .unwrap();
            let observer = phase.observer(&Ok);
            let mut meter = EquivalenceResourceMeterV1::new_with_observation_v1(
                queries,
                pairs,
                Some(&observer),
            )
            .unwrap();
            assert!(!index_values_equivalent(&context, left, right, &mut meter));
            assert_eq!(meter.exhausted(), denied);
            assert_eq!(
                (meter.queries, meter.cursor_steps, meter.expanded_pairs),
                counters
            );
            drop(meter);
            drop(phase);
            let state = receipt.snapshot();
            assert_eq!(
                state.first_denial,
                denied.then_some(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::PipelineProtocol,
                    resource: "index equivalence evaluation quota",
                })
            );
            assert!(!state.caught_panic);
            // Callback-quota controls do not claim a successful admission.
            assert_eq!(state.committed, Default::default());
        }
    }

    #[test]
    fn actual_uniformity_false_is_distinct_from_visit_exhaustion() {
        let (context, [constant, _, lane]) = fixture();
        for (value, limit, expected) in [
            (constant, 1, Ok(true)),
            (lane, 1, Ok(false)),
            (constant, 0, Err(UniformityVisitLimitV1)),
        ] {
            let mut receipt = InvocationReceiptV1::new(
                Default::default(),
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
            .unwrap();
            let phase = receipt
                .phase(ProductionAnalysisResourcePhaseV1::PipelineProtocol, 0)
                .unwrap();
            let result = is_uniform_value_with_observation_v1(
                &context,
                value,
                &HashSet::new(),
                limit,
                Some(&phase.observer(&Ok)),
            );
            assert_eq!(result, expected);
            drop(phase);
            let state = receipt.snapshot();
            assert_eq!(
                state.first_denial,
                expected
                    .is_err()
                    .then_some(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::PipelineProtocol,
                        resource: "uniformity operation visit quota",
                    })
            );
            assert!(!state.caught_panic);
            assert_eq!(state.committed, Default::default());
        }
    }

    #[test]
    fn quota_survives_conversion_followed_by_real_borrow_panic() {
        let (context, [left, right, _]) = fixture();
        let mut receipt = InvocationReceiptV1::new(
            Default::default(),
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        let phase = receipt
            .phase(ProductionAnalysisResourcePhaseV1::PipelineProtocol, 0)
            .unwrap();
        let observer = phase.observer(&Ok);
        assert!(
            EquivalenceResourceMeterV1::new_with_observation_v1(0, usize::MAX, Some(&observer))
                .is_err()
        );
        let mut denied =
            EquivalenceResourceMeterV1::new_with_observation_v1(0, 1, Some(&observer)).unwrap();
        assert!(!index_values_equivalent(&context, left, right, &mut denied));
        let mut meter =
            EquivalenceResourceMeterV1::new_with_observation_v1(1, 1, Some(&observer)).unwrap();
        let _borrow = left.defining_op().unwrap().deref_mut(&context);
        let result = catch_unwind(AssertUnwindSafe(|| {
            index_values_equivalent(&context, left, right, &mut meter)
        }));
        assert!(result.is_err());
        drop(meter);
        drop(denied);
        drop(phase);
        let state = receipt.snapshot();
        let denial = state.first_denial.unwrap();
        assert_eq!(denial.resource, "index equivalence cursor bound overflow");
        assert!(state.caught_panic);
        assert_eq!(
            receipt.complete(),
            Err(InvocationReceiptFailureV1::Denied(denial))
        );
    }
}
