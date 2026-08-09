use super::normalized::CaptureLimitsV2;
use rustc_middle::mir::{
    Body, NonDivergingIntrinsic, Operand, Place, Rvalue, StatementKind, TerminatorKind,
};
use std::fmt;

pub(super) fn preflight_body_v2(
    body: &Body<'_>,
    limits: CaptureLimitsV2,
) -> Result<usize, PreflightErrorV2> {
    bounded("locals", body.local_decls.len(), limits.max_locals)?;
    bounded("blocks", body.basic_blocks.len(), limits.max_blocks)?;

    let mut work = WorkCounter::new(limits.max_total_work_items);
    work.add("body", 1)?;
    work.add("locals", body.local_decls.len())?;
    work.add("blocks", body.basic_blocks.len())?;

    let mut total_statements = 0usize;
    for (block_index, block) in body.basic_blocks.iter_enumerated() {
        bounded(
            "statements per block",
            block.statements.len(),
            limits.max_statements_per_block,
        )?;
        total_statements = total_statements
            .checked_add(block.statements.len())
            .ok_or_else(|| PreflightErrorV2::new("statement count overflowed"))?;
        bounded(
            "total statements",
            total_statements,
            limits.max_total_statements,
        )?;
        for statement in &block.statements {
            work.add("statement", 1)?;
            preflight_statement(&statement.kind, limits, &mut work)?;
        }

        let terminator = block.terminator.as_ref().ok_or_else(|| {
            PreflightErrorV2::new(format!("bb{} has no terminator", block_index.as_usize()))
        })?;
        work.add("terminator", 1)?;
        let successors = terminator.successors().count();
        bounded("terminator successors", successors, limits.max_successors)?;
        work.add("terminator successors", successors)?;
        preflight_terminator(&terminator.kind, limits, &mut work)?;
    }
    Ok(work.total)
}

fn preflight_statement(
    kind: &StatementKind<'_>,
    limits: CaptureLimitsV2,
    work: &mut WorkCounter,
) -> Result<(), PreflightErrorV2> {
    match kind {
        StatementKind::Assign(assignment) => {
            let (place, value) = &**assignment;
            preflight_place(*place, limits, work)?;
            preflight_rvalue(value, limits, work)
        }
        StatementKind::FakeRead(contents) => preflight_place(contents.1, limits, work),
        StatementKind::SetDiscriminant { place, .. }
        | StatementKind::Retag(_, place)
        | StatementKind::PlaceMention(place) => preflight_place(**place, limits, work),
        StatementKind::AscribeUserType(contents, _) => preflight_place(contents.0, limits, work),
        StatementKind::Intrinsic(intrinsic) => match intrinsic.as_ref() {
            NonDivergingIntrinsic::Assume(operand) => preflight_operand(operand, limits, work),
            NonDivergingIntrinsic::CopyNonOverlapping(copy) => {
                preflight_operand(&copy.src, limits, work)?;
                preflight_operand(&copy.dst, limits, work)?;
                preflight_operand(&copy.count, limits, work)
            }
        },
        StatementKind::BackwardIncompatibleDropHint { place, .. } => {
            preflight_place(**place, limits, work)
        }
        StatementKind::StorageLive(_)
        | StatementKind::StorageDead(_)
        | StatementKind::Coverage(_)
        | StatementKind::ConstEvalCounter
        | StatementKind::Nop => Ok(()),
    }
}

fn preflight_rvalue(
    value: &Rvalue<'_>,
    limits: CaptureLimitsV2,
    work: &mut WorkCounter,
) -> Result<(), PreflightErrorV2> {
    work.add("rvalue", 1)?;
    match value {
        Rvalue::Use(operand)
        | Rvalue::Repeat(operand, _)
        | Rvalue::UnaryOp(_, operand)
        | Rvalue::Cast(_, operand, _)
        | Rvalue::WrapUnsafeBinder(operand, _) => preflight_operand(operand, limits, work),
        Rvalue::Ref(_, _, place)
        | Rvalue::RawPtr(_, place)
        | Rvalue::Discriminant(place)
        | Rvalue::CopyForDeref(place) => preflight_place(*place, limits, work),
        Rvalue::BinaryOp(_, operands) => {
            preflight_operand(&operands.0, limits, work)?;
            preflight_operand(&operands.1, limits, work)
        }
        Rvalue::Aggregate(_, operands) => {
            bounded("aggregate operands", operands.len(), limits.max_operands)?;
            work.add("aggregate operands", operands.len())?;
            for operand in operands {
                preflight_operand(operand, limits, work)?;
            }
            Ok(())
        }
        Rvalue::ThreadLocalRef(_) => Ok(()),
    }
}

fn preflight_operand(
    operand: &Operand<'_>,
    limits: CaptureLimitsV2,
    work: &mut WorkCounter,
) -> Result<(), PreflightErrorV2> {
    work.add("operand", 1)?;
    match operand {
        Operand::Copy(place) | Operand::Move(place) => preflight_place(*place, limits, work),
        Operand::Constant(_) | Operand::RuntimeChecks(_) => Ok(()),
    }
}

fn preflight_place(
    place: Place<'_>,
    limits: CaptureLimitsV2,
    work: &mut WorkCounter,
) -> Result<(), PreflightErrorV2> {
    bounded(
        "place projection",
        place.projection.len(),
        limits.max_projection_depth,
    )?;
    work.add("place", 1)?;
    work.add("place projection", place.projection.len())
}

fn preflight_terminator(
    kind: &TerminatorKind<'_>,
    limits: CaptureLimitsV2,
    work: &mut WorkCounter,
) -> Result<(), PreflightErrorV2> {
    match kind {
        TerminatorKind::SwitchInt { discr, targets } => {
            let target_count = targets.iter().count();
            bounded("switch targets", target_count, limits.max_switch_targets)?;
            work.add("switch targets", target_count)?;
            preflight_operand(discr, limits, work)
        }
        TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } => {
            bounded("call arguments", args.len(), limits.max_operands)?;
            work.add("call arguments", args.len())?;
            preflight_operand(func, limits, work)?;
            for argument in args {
                preflight_operand(&argument.node, limits, work)?;
            }
            preflight_place(*destination, limits, work)
        }
        TerminatorKind::TailCall { func, args, .. } => {
            bounded("tail-call arguments", args.len(), limits.max_operands)?;
            work.add("tail-call arguments", args.len())?;
            preflight_operand(func, limits, work)?;
            for argument in args {
                preflight_operand(&argument.node, limits, work)?;
            }
            Ok(())
        }
        TerminatorKind::Drop { place, .. } => preflight_place(*place, limits, work),
        TerminatorKind::Assert { cond, .. } => preflight_operand(cond, limits, work),
        TerminatorKind::Yield {
            value, resume_arg, ..
        } => {
            preflight_operand(value, limits, work)?;
            preflight_place(*resume_arg, limits, work)
        }
        TerminatorKind::InlineAsm {
            template,
            operands,
            line_spans,
            targets,
            ..
        } => {
            bounded(
                "inline assembly operands",
                operands.len(),
                limits.max_operands,
            )?;
            bounded(
                "inline assembly targets",
                targets.len(),
                limits.max_successors,
            )?;
            work.add("inline assembly template", template.len())?;
            work.add("inline assembly operands", operands.len())?;
            work.add("inline assembly line spans", line_spans.len())?;
            work.add("inline assembly targets", targets.len())
        }
        TerminatorKind::Goto { .. }
        | TerminatorKind::UnwindResume
        | TerminatorKind::UnwindTerminate(_)
        | TerminatorKind::Return
        | TerminatorKind::Unreachable
        | TerminatorKind::CoroutineDrop
        | TerminatorKind::FalseEdge { .. }
        | TerminatorKind::FalseUnwind { .. } => Ok(()),
    }
}

struct WorkCounter {
    total: usize,
    limit: usize,
}

impl WorkCounter {
    fn new(limit: usize) -> Self {
        Self { total: 0, limit }
    }

    fn add(&mut self, label: &str, count: usize) -> Result<(), PreflightErrorV2> {
        self.total = self
            .total
            .checked_add(count)
            .ok_or_else(|| PreflightErrorV2::new(format!("{label} work count overflowed")))?;
        bounded("total capture work", self.total, self.limit)
    }
}

fn bounded(label: &str, actual: usize, limit: usize) -> Result<(), PreflightErrorV2> {
    if actual > limit {
        return Err(PreflightErrorV2::new(format!(
            "{label} bound exceeded: {actual} > {limit}"
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PreflightErrorV2 {
    reason: String,
}

impl PreflightErrorV2 {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl fmt::Display for PreflightErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}
