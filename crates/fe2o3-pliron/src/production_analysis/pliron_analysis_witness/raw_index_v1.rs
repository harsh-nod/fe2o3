use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::InvocationObserverV1;

type RawIndexObserverV1<'o, 'p, 'r> = Option<&'o InvocationObserverV1<'p, 'r>>;

struct RawIndexEvaluationBudgetV1<'a, 'p, 'r> {
    steps: &'a mut usize,
    stack_frames: usize,
    quota_phase: ProductionAnalysisResourcePhaseV1,
    observer: RawIndexObserverV1<'a, 'p, 'r>,
}

fn raw_index_quota_failure_v1(
    observer: RawIndexObserverV1<'_, '_, '_>,
    phase: ProductionAnalysisResourcePhaseV1,
    reason: &'static str,
) -> RawIndexEvaluationFailureV1 {
    if let Some(observer) = observer {
        observer.deny(ProductionAnalysisResourceLimitV1 {
            phase,
            resource: reason,
        });
    }
    RawIndexEvaluationFailureV1::Incomplete(reason)
}

enum RawIndexEvaluationFailureV1 {
    Incomplete(&'static str),
    Overflow(&'static str),
}

#[cfg(test)]
pub(crate) fn evaluate_raw_index_at_invocation_v1(
    context: &Context,
    value: Value,
    invocation: &[u64],
    evaluation_steps: &mut usize,
) -> Option<u64> {
    evaluate_raw_index_at_invocation_with_observation_v1(
        context,
        value,
        invocation,
        evaluation_steps,
        None,
    )
}

pub(crate) fn evaluate_raw_index_at_invocation_with_observation_v1(
    context: &Context,
    value: Value,
    invocation: &[u64],
    evaluation_steps: &mut usize,
    observer: RawIndexObserverV1<'_, '_, '_>,
) -> Option<u64> {
    let evaluation_stack_frame_limit = MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1
        .checked_mul(RAW_INDEX_STACK_FRAMES_PER_OPERATION_V1)
        .and_then(|frames| frames.checked_add(RAW_INDEX_STACK_FIXED_FRAMES_V1))
        .ok_or_else(|| {
            raw_index_quota_failure_v1(
                observer,
                ProductionAnalysisResourcePhaseV1::RaceFreedom,
                "raw-index evaluation stack bound overflow",
            )
        })
        .ok()?;
    evaluate_raw_index_with_budget_v1(
        context,
        value,
        invocation,
        &mut HashMap::new(),
        &mut HashSet::new(),
        &mut RawIndexEvaluationBudgetV1 {
            steps: evaluation_steps,
            stack_frames: evaluation_stack_frame_limit,
            quota_phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
            observer,
        },
    )
    .ok()
}

#[derive(Clone, Copy)]
enum RawIndexEvaluationFrameV1 {
    Evaluate(Value),
    FinishBinary {
        value: Value,
        lhs: Value,
        rhs: Value,
        kind: Option<IndexBinaryKindAttr>,
    },
}

fn push_raw_index_evaluation_frame_v1(
    stack: &mut Vec<RawIndexEvaluationFrameV1>,
    frame: RawIndexEvaluationFrameV1,
    budget: &RawIndexEvaluationBudgetV1<'_, '_, '_>,
) -> Result<(), RawIndexEvaluationFailureV1> {
    if stack.len() == budget.stack_frames {
        return Err(raw_index_quota_failure_v1(
            budget.observer,
            budget.quota_phase,
            "raw-index evaluation exceeded its deterministic stack cap",
        ));
    }
    stack.try_reserve(1).map_err(|_| {
        raw_index_quota_failure_v1(
            budget.observer,
            budget.quota_phase,
            "raw-index evaluation stack allocation failed",
        )
    })?;
    stack.push(frame);
    Ok(())
}

#[cfg(test)]
fn evaluate_raw_index_iterative(
    context: &Context,
    value: Value,
    invocation: &[u64],
    cache: &mut HashMap<Value, u64>,
    active: &mut HashSet<Value>,
    evaluation_steps: &mut usize,
    evaluation_stack_frame_limit: usize,
) -> Result<u64, RawIndexEvaluationFailureV1> {
    evaluate_raw_index_with_budget_v1(
        context,
        value,
        invocation,
        cache,
        active,
        &mut RawIndexEvaluationBudgetV1 {
            steps: evaluation_steps,
            stack_frames: evaluation_stack_frame_limit,
            quota_phase: ProductionAnalysisResourcePhaseV1::ReportValidation,
            observer: None,
        },
    )
}

fn evaluate_raw_index_with_budget_v1(
    context: &Context,
    value: Value,
    invocation: &[u64],
    cache: &mut HashMap<Value, u64>,
    active: &mut HashSet<Value>,
    budget: &mut RawIndexEvaluationBudgetV1<'_, '_, '_>,
) -> Result<u64, RawIndexEvaluationFailureV1> {
    match budget.observer {
        None => evaluate_raw_index_inner_v1(context, value, invocation, cache, active, budget),
        Some(observer) => observer.with_projection(&Ok, |_| {
            evaluate_raw_index_inner_v1(context, value, invocation, cache, active, budget)
        }),
    }
}

fn evaluate_raw_index_inner_v1(
    context: &Context,
    value: Value,
    invocation: &[u64],
    cache: &mut HashMap<Value, u64>,
    active: &mut HashSet<Value>,
    budget: &mut RawIndexEvaluationBudgetV1<'_, '_, '_>,
) -> Result<u64, RawIndexEvaluationFailureV1> {
    let mut stack = Vec::new();
    push_raw_index_evaluation_frame_v1(
        &mut stack,
        RawIndexEvaluationFrameV1::Evaluate(value),
        budget,
    )?;
    while let Some(frame) = stack.pop() {
        match frame {
            RawIndexEvaluationFrameV1::Evaluate(value) => {
                if cache.contains_key(&value) {
                    continue;
                }
                *budget.steps = budget.steps.saturating_add(1);
                if *budget.steps > MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 {
                    return Err(raw_index_quota_failure_v1(
                        budget.observer,
                        budget.quota_phase,
                        "raw-index evaluation exceeded its deterministic work cap",
                    ));
                }
                if !active.insert(value) {
                    return Err(RawIndexEvaluationFailureV1::Incomplete(
                        "the raw index definition graph is cyclic",
                    ));
                }
                let Some(definition) = value.defining_op() else {
                    active.remove(&value);
                    return Err(RawIndexEvaluationFailureV1::Incomplete(
                        "block arguments are outside the V1 raw-index fragment",
                    ));
                };
                let operation = Operation::get_op_dyn(definition, context);
                if let Some(constant) = operation.downcast_ref::<IndexConstantOp>() {
                    let Some(result) = constant.value(context) else {
                        active.remove(&value);
                        return Err(RawIndexEvaluationFailureV1::Incomplete(
                            "index constant has no value",
                        ));
                    };
                    active.remove(&value);
                    cache.insert(value, result);
                    continue;
                }
                if let Some(index) = operation.downcast_ref::<InvocationIndexOp>() {
                    let Some(dimension) = index
                        .dimension(context)
                        .and_then(|dimension| usize::try_from(dimension).ok())
                    else {
                        active.remove(&value);
                        return Err(RawIndexEvaluationFailureV1::Incomplete(
                            "invocation index has no valid dimension",
                        ));
                    };
                    let Some(result) = invocation.get(dimension).copied() else {
                        active.remove(&value);
                        return Err(RawIndexEvaluationFailureV1::Incomplete(
                            "invocation dimension is absent from the launch inventory",
                        ));
                    };
                    active.remove(&value);
                    cache.insert(value, result);
                    continue;
                }
                let Some(binary) = operation.downcast_ref::<IndexBinaryOp>() else {
                    active.remove(&value);
                    return Err(RawIndexEvaluationFailureV1::Incomplete(
                        "index producer is not a supported constant, invocation, or binary operation",
                    ));
                };
                let lhs = binary.lhs(context);
                let rhs = binary.rhs(context);
                push_raw_index_evaluation_frame_v1(
                    &mut stack,
                    RawIndexEvaluationFrameV1::FinishBinary {
                        value,
                        lhs,
                        rhs,
                        kind: binary.kind(context),
                    },
                    budget,
                )?;
                push_raw_index_evaluation_frame_v1(
                    &mut stack,
                    RawIndexEvaluationFrameV1::Evaluate(rhs),
                    budget,
                )?;
                push_raw_index_evaluation_frame_v1(
                    &mut stack,
                    RawIndexEvaluationFrameV1::Evaluate(lhs),
                    budget,
                )?;
            }
            RawIndexEvaluationFrameV1::FinishBinary {
                value,
                lhs,
                rhs,
                kind,
            } => {
                let result = match (cache.get(&lhs).copied(), cache.get(&rhs).copied(), kind) {
                    (Some(lhs), Some(rhs), Some(IndexBinaryKindAttr::Add)) => lhs
                        .checked_add(rhs)
                        .ok_or(RawIndexEvaluationFailureV1::Overflow("addition")),
                    (Some(lhs), Some(rhs), Some(IndexBinaryKindAttr::Multiply)) => lhs
                        .checked_mul(rhs)
                        .ok_or(RawIndexEvaluationFailureV1::Overflow("multiplication")),
                    (Some(lhs), Some(rhs), Some(IndexBinaryKindAttr::Remainder)) if rhs != 0 => {
                        Ok(lhs % rhs)
                    }
                    (Some(lhs), Some(rhs), Some(IndexBinaryKindAttr::Divide)) if rhs != 0 => {
                        Ok(lhs / rhs)
                    }
                    (Some(_), Some(_), Some(IndexBinaryKindAttr::Remainder)) => Err(
                        RawIndexEvaluationFailureV1::Incomplete("remainder divisor is zero"),
                    ),
                    (Some(_), Some(_), Some(IndexBinaryKindAttr::Divide)) => Err(
                        RawIndexEvaluationFailureV1::Incomplete("division divisor is zero"),
                    ),
                    (Some(_), Some(_), None) => Err(RawIndexEvaluationFailureV1::Incomplete(
                        "index binary operation has no kind",
                    )),
                    _ => Err(RawIndexEvaluationFailureV1::Incomplete(
                        "raw-index evaluation stack lost an operand result",
                    )),
                };
                active.remove(&value);
                cache.insert(value, result?);
            }
        }
    }
    cache
        .get(&value)
        .copied()
        .ok_or(RawIndexEvaluationFailureV1::Incomplete(
            "raw-index evaluation stack produced no result",
        ))
}

#[cfg(test)]
mod observed_raw_index_tests {
    use super::*;
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1, InvocationReceiptV1,
    };
    use pliron::{builtin::types::FunctionType, op::Op};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn fixture() -> (Context, Value, Vec<Value>) {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let signature = FunctionType::get(
            &context,
            vec![dialect_kernel::IndexType::get(&context).into()],
            vec![],
        );
        let function = FuncOp::new(
            &mut context,
            "observed_raw_index".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        let argument = entry.deref(&context).arguments().next().unwrap();
        let zero = IndexConstantOp::new(&mut context, 0);
        let one = IndexConstantOp::new(&mut context, 1);
        let max = IndexConstantOp::new(&mut context, u64::MAX);
        let z = zero.result(&context);
        let o = one.result(&context);
        let m = max.result(&context);
        let overflow = IndexBinaryOp::new(&mut context, IndexBinaryKindAttr::Add, m, o);
        let divide = IndexBinaryOp::new(&mut context, IndexBinaryKindAttr::Divide, o, z);
        let remainder = IndexBinaryOp::new(&mut context, IndexBinaryKindAttr::Remainder, o, z);
        let missing = InvocationIndexOp::new(&mut context, 1, 8);
        let cycle = IndexBinaryOp::new(&mut context, IndexBinaryKindAttr::Add, o, z);
        let unsupported = dialect_kernel::SemanticConstantOp::new(&mut context, 0);
        for op in [
            zero.get_operation(),
            one.get_operation(),
            max.get_operation(),
            overflow.get_operation(),
            divide.get_operation(),
            remainder.get_operation(),
            missing.get_operation(),
            cycle.get_operation(),
            unsupported.get_operation(),
            dialect_kernel::ReturnOp::new(&mut context).get_operation(),
        ] {
            op.insert_at_back(entry, &context);
        }
        Operation::replace_operand(cycle.get_operation(), &context, 0, cycle.result(&context));
        let refused = vec![
            overflow.result(&context),
            divide.result(&context),
            remainder.result(&context),
            missing.result(&context),
            cycle.result(&context),
            argument,
            unsupported.result(&context),
        ];
        (context, z, refused)
    }

    #[test]
    fn real_constant_observes_exact_work_and_stack_boundaries() {
        let (context, value, _) = fixture();
        for (initial_steps, frames, denied) in [
            (0, 1, false),
            (0, 0, true),
            (MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 - 1, 1, false),
            (MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1, 1, true),
        ] {
            let mut receipt = InvocationReceiptV1::new(
                Default::default(),
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
            .unwrap();
            let phase = receipt
                .phase(ProductionAnalysisResourcePhaseV1::RaceFreedom, 0)
                .unwrap();
            let observer = phase.observer(&Ok);
            let mut steps = initial_steps;
            let result = evaluate_raw_index_with_budget_v1(
                &context,
                value,
                &[0],
                &mut HashMap::new(),
                &mut HashSet::new(),
                &mut RawIndexEvaluationBudgetV1 {
                    steps: &mut steps,
                    stack_frames: frames,
                    quota_phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                    observer: Some(&observer),
                },
            );
            assert_eq!(result.ok(), (!denied).then_some(0));
            assert_eq!(steps, initial_steps + usize::from(frames != 0));
            drop(phase);
            let state = receipt.snapshot();
            assert_eq!(
                state.first_denial,
                denied.then_some(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                    resource: if frames == 0 {
                        "raw-index evaluation exceeded its deterministic stack cap"
                    } else {
                        "raw-index evaluation exceeded its deterministic work cap"
                    },
                })
            );
            assert!(!state.caught_panic);
            // These are callback-limit controls, not accepted phase receipts.
            assert_eq!(state.committed, Default::default());
        }
    }

    #[test]
    fn nonquota_raw_index_refusals_match_the_ordinary_adapter() {
        let (context, _, refused) = fixture();
        for value in refused {
            let mut ordinary_steps = 0;
            assert_eq!(
                evaluate_raw_index_at_invocation_v1(&context, value, &[0], &mut ordinary_steps),
                None
            );
            let mut receipt = InvocationReceiptV1::new(
                Default::default(),
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
            .unwrap();
            let phase = receipt
                .phase(ProductionAnalysisResourcePhaseV1::RaceFreedom, 0)
                .unwrap();
            let mut steps = 0;
            assert_eq!(
                evaluate_raw_index_at_invocation_with_observation_v1(
                    &context,
                    value,
                    &[0],
                    &mut steps,
                    Some(&phase.observer(&Ok))
                ),
                None
            );
            assert_eq!(steps, ordinary_steps);
            drop(phase);
            let state = receipt.snapshot();
            assert_eq!(state.first_denial, None);
            assert!(!state.caught_panic);
        }
    }

    #[test]
    fn raw_index_first_quota_survives_none_and_real_borrow_panic() {
        let (context, value, _) = fixture();
        let mut receipt = InvocationReceiptV1::new(
            Default::default(),
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        let phase = receipt
            .phase(ProductionAnalysisResourcePhaseV1::RaceFreedom, 0)
            .unwrap();
        let observer = phase.observer(&Ok);
        assert!(
            evaluate_raw_index_with_budget_v1(
                &context,
                value,
                &[0],
                &mut HashMap::new(),
                &mut HashSet::new(),
                &mut RawIndexEvaluationBudgetV1 {
                    steps: &mut 0,
                    stack_frames: 0,
                    quota_phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                    observer: Some(&observer)
                }
            )
            .is_err()
        );
        let mut steps = MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1;
        assert_eq!(
            evaluate_raw_index_at_invocation_with_observation_v1(
                &context,
                value,
                &[0],
                &mut steps,
                Some(&observer)
            ),
            None
        );
        let _borrow = value.defining_op().unwrap().deref_mut(&context);
        let result = catch_unwind(AssertUnwindSafe(|| {
            evaluate_raw_index_at_invocation_with_observation_v1(
                &context,
                value,
                &[0],
                &mut 0,
                Some(&observer),
            )
        }));
        assert!(result.is_err());
        drop(phase);
        let state = receipt.snapshot();
        let denial = state.first_denial.unwrap();
        assert_eq!(
            denial.resource,
            "raw-index evaluation exceeded its deterministic stack cap"
        );
        assert!(state.caught_panic);
        assert_eq!(
            receipt.complete(),
            Err(InvocationReceiptFailureV1::Denied(denial))
        );
    }
}
