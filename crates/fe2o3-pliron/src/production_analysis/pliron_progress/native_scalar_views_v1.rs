use crate::production_analysis::pliron_control_edges_v1::ControlViewV1;

#[derive(Clone, Copy)]
struct ProgressHeaderViewV1 {
    terminator: Ptr<Operation>,
    compare: Option<Ptr<Operation>>,
    induction: pliron::value::Value,
    bound: pliron::value::Value,
    body_ordinal: usize,
    exit_ordinal: usize,
    domain: NativeProgressDomainV1,
}

fn progress_header_view_v1(
    context: &Context,
    terminator: &OpBox,
    operation_blocks: &HashMap<Ptr<Operation>, usize>,
) -> Option<ProgressHeaderViewV1> {
    #[cfg(test)]
    native_progress_scalar_observation_v1::header();
    let pointer = terminator.get_operation();
    let header = *operation_blocks.get(&pointer)?;
    let control = ControlViewV1::observe(context, pointer).ok()?;
    if control.successor_count() != 2 {
        return None;
    }
    control.edge(0).ok()?;
    control.edge(1).ok()?;
    if let Some(branch) = terminator.downcast_ref::<IndexLessThanBranchArgsOp>() {
        return Some(ProgressHeaderViewV1 {
            terminator: pointer,
            compare: None,
            induction: branch.lhs(context),
            bound: branch.rhs(context),
            body_ordinal: 0,
            exit_ordinal: 1,
            domain: NativeProgressDomainV1::LegacyIndex,
        });
    }
    use dialect_gpu::optimization_v1::{CompareOp, ComparePredicateAttr, CondBranchOp};
    let branch = terminator.downcast_ref::<CondBranchOp>()?;
    let condition = branch.condition(context);
    let compare_pointer = condition.defining_op()?;
    // The tracked induction is a header argument, so its actual comparison
    // must be in this header. The immutable verifier established local order.
    if operation_blocks.get(&compare_pointer) != Some(&header)
        || !native_progress_scalar_shape_v1(context, compare_pointer, 2, 1)
    {
        return None;
    }
    let operation = Operation::get_op_dyn(compare_pointer, context);
    let compare = operation.downcast_ref::<CompareOp>()?;
    if compare.result(context) != condition || !native_progress_bool_v1(context, condition) {
        return None;
    }
    let lhs = compare.get_operand_lhs(context);
    let rhs = compare.get_operand_rhs(context);
    let (induction, bound, body_ordinal) = match compare.predicate(context)? {
        ComparePredicateAttr::LessThan => (lhs, rhs, 0),
        ComparePredicateAttr::GreaterThanOrEqual => (lhs, rhs, 1),
        ComparePredicateAttr::GreaterThan => (rhs, lhs, 0),
        ComparePredicateAttr::LessThanOrEqual => (rhs, lhs, 1),
        ComparePredicateAttr::Equal | ComparePredicateAttr::NotEqual => return None,
    };
    let domain = native_progress_domain_v1(context, induction)?;
    if domain == NativeProgressDomainV1::LegacyIndex
        || native_progress_domain_v1(context, bound) != Some(domain)
    {
        return None;
    }
    Some(ProgressHeaderViewV1 {
        terminator: pointer,
        compare: Some(compare_pointer),
        induction,
        bound,
        body_ordinal,
        exit_ordinal: 1 - body_ordinal,
        domain,
    })
}

fn progress_unconditional_v1(_context: &Context, terminator: &OpBox) -> bool {
    terminator.downcast_ref::<BranchOp>().is_some()
        || terminator.downcast_ref::<BranchArgsOp>().is_some()
        || terminator
            .downcast_ref::<dialect_gpu::optimization_v1::BranchOp>()
            .is_some()
}

fn progress_induction_offset_v1(
    context: &Context,
    value: pliron::value::Value,
    base: pliron::value::Value,
    domain: NativeProgressDomainV1,
) -> Option<u64> {
    #[cfg(test)]
    native_progress_scalar_observation_v1::step();
    if value == base {
        return Some(0);
    }
    if domain == NativeProgressDomainV1::LegacyIndex {
        return progress_index_offset_v1(context, value, base);
    }
    use dialect_gpu::optimization_v1::{BinaryKindAttr, BinaryOp};
    let operation = Operation::get_op_dyn(value.defining_op()?, context);
    let add = operation.downcast_ref::<BinaryOp>()?;
    let checked = match add.kind(context)? {
        BinaryKindAttr::Add => false,
        BinaryKindAttr::CheckedAdd => true,
        _ => return None,
    };
    if !native_progress_scalar_shape_v1(
        context,
        add.get_operation(),
        2,
        if checked { 2 } else { 1 },
    ) || add.result(context) != value
        || native_progress_domain_v1(context, value) != Some(domain)
        || native_progress_domain_v1(context, base) != Some(domain)
        || (checked && !native_progress_bool_v1(context, add.overflow(context)?))
    {
        return None;
    }
    let lhs = add.get_operand_lhs(context);
    let rhs = add.get_operand_rhs(context);
    let step_value = if lhs == base {
        rhs
    } else if rhs == base {
        lhs
    } else {
        return None;
    };
    let step = native_progress_literal_v1(context, step_value)?;
    (step.domain == domain)
        .then(|| domain.positive_step(step.bits))
        .flatten()
}

#[allow(clippy::too_many_arguments)]
fn progress_external_bound_v1(
    context: &Context,
    value: pliron::value::Value,
    domain: NativeProgressDomainV1,
    block_indices: &HashMap<Ptr<BasicBlock>, usize>,
    operation_blocks: &HashMap<Ptr<Operation>, usize>,
    dominators: &[HashSet<usize>],
    members: &HashSet<usize>,
    header: usize,
) -> bool {
    #[cfg(test)]
    native_progress_scalar_observation_v1::external();
    if domain == NativeProgressDomainV1::LegacyIndex {
        return !value
            .defining_op()
            .and_then(|op| operation_blocks.get(&op))
            .is_some_and(|block| members.contains(block));
    }
    let definition = if let Some(operation) = value.defining_op() {
        operation_blocks.get(&operation).copied()
    } else {
        value
            .defining_block()
            .and_then(|block| block_indices.get(&block).copied())
    };
    definition.is_some_and(|block| {
        !members.contains(&block)
            && dominators
                .get(header)
                .is_some_and(|set| set.contains(&block))
            && native_progress_domain_v1(context, value) == Some(domain)
    })
}

fn progress_step_has_no_wrap_v1(
    context: &Context,
    header: ProgressHeaderViewV1,
    step: u64,
) -> bool {
    // Retain the exact branch/compare identity in the borrowed view. No view
    // survives the enclosing immutable invocation or authenticates another IR.
    debug_assert!(
        header
            .compare
            .is_none_or(|compare| compare != header.terminator)
    );
    if header.domain != NativeProgressDomainV1::LegacyIndex {
        return native_progress_no_wrap_v1(context, header.bound, header.domain, step);
    }
    if step <= 1 {
        return step == 1;
    }
    index_constant(context, header.bound)
        .or_else(|| unsigned_cast_upper_bound(context, header.bound))
        .is_some_and(|bound| bound == 0 || (bound - 1).checked_add(step).is_some())
}

#[allow(clippy::too_many_arguments)]
fn native_zero_step_result_v1(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    block_indices: &HashMap<Ptr<BasicBlock>, usize>,
    members: &HashSet<usize>,
    incoming: &[IncomingEdgeV1],
    header_index: usize,
    latch_index: usize,
    induction_argument: usize,
    header: ProgressHeaderViewV1,
    reason: &'static str,
) -> CanonicalLoopResultV1 {
    let Some(bound) = native_progress_literal_v1(context, header.bound)
        .filter(|bound| bound.domain == header.domain)
        .and_then(NativeProgressLiteralV1::ordered)
    else {
        return CanonicalLoopResultV1::Incomplete(
            "the native zero-step guard has no exact typed bound witness",
        );
    };
    let mut saw_entry = false;
    let mut saw_unknown = false;
    let mut saw_active = false;
    let mut definite_active = false;
    for edge in incoming {
        if members.contains(&edge.source) {
            continue;
        }
        saw_entry = true;
        let Some(pointer) = blocks[edge.source].deref(context).get_terminator(context) else {
            saw_unknown = true;
            continue;
        };
        let terminator = Operation::get_op_dyn(pointer, context);
        let initial = ControlViewV1::observe(context, pointer)
            .ok()
            .and_then(|control| {
                control
                    .edge(edge.ordinal)
                    .ok()
                    .and_then(|occurrence| occurrence.argument_at(induction_argument).ok())
                    .map(|pair| pair.0)
            })
            .and_then(|value| native_progress_literal_v1(context, value))
            .filter(|value| value.domain == header.domain)
            .and_then(NativeProgressLiteralV1::ordered);
        match initial {
            Some(initial) if initial < bound => {
                saw_active = true;
                definite_active |= native_progress_definitely_reaches_v1(
                    context,
                    blocks,
                    block_indices,
                    edge.source,
                ) && progress_unconditional_v1(context, &terminator);
            }
            Some(_) => {}
            None => saw_unknown = true,
        }
    }
    if saw_entry && !saw_unknown && !saw_active {
        return CanonicalLoopResultV1::Inactive;
    }
    let definite_recurrence = native_progress_definite_recurrence_v1(
        context,
        blocks,
        block_indices,
        members,
        header_index,
        latch_index,
        header.body_ordinal,
    );
    if definite_active && definite_recurrence {
        CanonicalLoopResultV1::Rejected {
            reason,
            counterexample: "a definitely reached typed true guard retains an unchanged induction value along an unconditional recurrence".to_owned(),
        }
    } else {
        CanonicalLoopResultV1::Incomplete(
            "the native zero-step cycle has conditional entry, an unresolved seed, or an early-exit path",
        )
    }
}

fn native_progress_definite_recurrence_v1(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    indices: &HashMap<Ptr<BasicBlock>, usize>,
    members: &HashSet<usize>,
    header: usize,
    latch: usize,
    body_ordinal: usize,
) -> bool {
    let Some(pointer) = blocks[header].deref(context).get_terminator(context) else {
        return false;
    };
    let Ok(control) = ControlViewV1::observe(context, pointer) else {
        return false;
    };
    let Some(mut current) = control
        .edge(body_ordinal)
        .ok()
        .and_then(|edge| indices.get(&edge.target()).copied())
    else {
        return false;
    };
    if current == header {
        return latch == header;
    }
    for _ in 0..members.len() {
        if !members.contains(&current) {
            return false;
        }
        let Some(pointer) = blocks[current].deref(context).get_terminator(context) else {
            return false;
        };
        let operation = Operation::get_op_dyn(pointer, context);
        if !progress_unconditional_v1(context, &operation) {
            return false;
        }
        let Ok(control) = ControlViewV1::observe(context, pointer) else {
            return false;
        };
        if control.successor_count() != 1 {
            return false;
        }
        let Some(next) = control
            .edge(0)
            .ok()
            .and_then(|edge| indices.get(&edge.target()).copied())
        else {
            return false;
        };
        if next == header {
            return current == latch;
        }
        current = next;
    }
    false
}

fn native_progress_definitely_reaches_v1(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    indices: &HashMap<Ptr<BasicBlock>, usize>,
    target: usize,
) -> bool {
    let mut current = 0;
    for _ in 0..blocks.len() {
        if current == target {
            return true;
        }
        let Some(pointer) = blocks[current].deref(context).get_terminator(context) else {
            return false;
        };
        let operation = Operation::get_op_dyn(pointer, context);
        if !progress_unconditional_v1(context, &operation) {
            return false;
        }
        let Ok(control) = ControlViewV1::observe(context, pointer) else {
            return false;
        };
        if control.successor_count() != 1 {
            return false;
        }
        let Some(next) = control
            .edge(0)
            .ok()
            .and_then(|edge| indices.get(&edge.target()))
        else {
            return false;
        };
        current = *next;
    }
    false
}

#[cfg(test)]
mod native_progress_scalar_observation_v1 {
    use std::cell::Cell;
    thread_local! {
        static DOMAINS: Cell<usize> = const { Cell::new(0) };
        static LITERALS: Cell<usize> = const { Cell::new(0) };
        static COPY_ATTEMPTS: Cell<usize> = const { Cell::new(0) };
        static PROBES: Cell<usize> = const { Cell::new(0) };
        static PANIC_AFTER_LITERAL: Cell<bool> = const { Cell::new(false) };
    }
    pub(super) fn domain() {
        DOMAINS.set(DOMAINS.get() + 1);
    }
    pub(super) fn literal() {
        LITERALS.set(LITERALS.get() + 1);
    }
    pub(super) fn header() {
        PROBES.set(PROBES.get() + 1);
    }
    pub(super) fn step() {
        PROBES.set(PROBES.get() + 1);
    }
    pub(super) fn external() {
        PROBES.set(PROBES.get() + 1);
    }
    pub(super) fn bound() {
        PROBES.set(PROBES.get() + 1);
    }
    pub(super) fn copied_literal() {
        assert!(
            !PANIC_AFTER_LITERAL.replace(false),
            "native progress paid literal panic"
        );
    }
    pub(super) fn copy_attempt() {
        COPY_ATTEMPTS.set(COPY_ATTEMPTS.get() + 1);
    }
    pub(super) fn copy_attempts() -> usize {
        COPY_ATTEMPTS.get()
    }
    pub(super) fn panic_after_literal() {
        PANIC_AFTER_LITERAL.set(true);
    }
    pub(super) fn reset() {
        DOMAINS.set(0);
        LITERALS.set(0);
        COPY_ATTEMPTS.set(0);
        PROBES.set(0);
        PANIC_AFTER_LITERAL.set(false);
    }
    pub(super) fn counts() -> (usize, usize) {
        (DOMAINS.get(), LITERALS.get())
    }
    pub(super) fn probes() -> usize {
        PROBES.get()
    }
}
