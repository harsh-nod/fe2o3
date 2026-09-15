//! Immutable proof of the tuple-forwarding MIR retained in the pinned AMD core.

use super::*;
use rustc_middle::mir::AggregateKind;

pub(super) fn reviewed_ne<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    let entry = &body.basic_blocks[BasicBlock::from_usize(0)];
    let done = &body.basic_blocks[BasicBlock::from_usize(1)];
    entry.statements.is_empty()
        && call_target_with_operands(
            tcx,
            instance,
            &entry.terminator().kind,
            contract,
            3,
            1,
            2,
            copy_local,
        ) == Some(BasicBlock::from_usize(1))
        && matches!(done.statements.as_slice(), [negate]
            if matches!(assignment(&negate.kind, 0), Some(Rvalue::UnaryOp(UnOp::Not, operand))
                if move_local(operand, 3)))
        && matches!(done.terminator().kind, TerminatorKind::Return)
}

pub(super) fn eq_local_types<'tcx>(tcx: TyCtxt<'tcx>, contract: &Contract<'tcx>) -> Vec<Ty<'tcx>> {
    let reference = shared(tcx, contract.option);
    vec![
        tcx.types.bool,
        reference,
        reference,
        Ty::new_tup(tcx, &[reference, reference]),
        tcx.types.isize,
        tcx.types.isize,
        tcx.types.isize,
        shared(tcx, contract.payload),
        shared(tcx, contract.payload),
        reference,
        reference,
        reference,
        reference,
        reference,
    ]
}

fn tuple_reference<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    statement: &StatementKind<'tcx>,
    contract: &Contract<'tcx>,
    destination: usize,
    field_index: usize,
) -> bool {
    matches!(assignment(statement, destination), Some(Rvalue::Use(Operand::Copy(place)))
        if place.local.as_usize() == 3
            && matches!(place.projection.as_ref(), [ProjectionElem::Field(field, ty)]
                if field.as_usize() == field_index
                    && normalized_ty(tcx, instance, *ty) == Some(shared(tcx, contract.option))))
}

fn switch<'a>(
    terminator: &'a TerminatorKind<'_>,
    local: usize,
    contract: &Contract<'_>,
) -> Option<&'a SwitchTargets> {
    let TerminatorKind::SwitchInt { discr, targets } = terminator else {
        return None;
    };
    (move_local(discr, local)
        && targets.all_values() == [contract.none_discriminant, contract.some_discriminant]
        && targets.all_targets().len() == 3
        && targets
            .all_targets()
            .iter()
            .all(|block| block.as_usize() < 9))
    .then_some(targets)
}

fn result<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    block: BasicBlock,
    done: BasicBlock,
    value: bool,
) -> bool {
    let block = &body.basic_blocks[block];
    matches!(block.statements.as_slice(), [statement]
        if matches!(assignment(&statement.kind, 0), Some(Rvalue::Use(operand))
            if constant(tcx, operand, tcx.types.bool, u128::from(value))))
        && matches!(block.terminator().kind, TerminatorKind::Goto { target } if target == done)
}

pub(super) fn reviewed_eq<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    let entry = BasicBlock::from_usize(0);
    let start = &body.basic_blocks[entry];
    if !matches!(start.statements.as_slice(), [tuple, left, tag]
        if matches!(assignment(&tuple.kind, 3), Some(Rvalue::Aggregate(kind, operands))
            if matches!(kind.as_ref(), AggregateKind::Tuple)
                && operands.len() == 2
                && operands.iter().zip([1, 2]).all(|(operand, local)| copy_local(operand, local)))
            && tuple_reference(tcx, instance, &left.kind, contract, 9, 0)
            && discriminant(&tag.kind, 6, 9))
    {
        return false;
    }
    let Some(first) = switch(&start.terminator().kind, 6, contract) else {
        return false;
    };
    let left_none = first.target_for_value(contract.none_discriminant);
    let left_some = first.target_for_value(contract.some_discriminant);
    let invalid = first.otherwise();
    let some_block = &body.basic_blocks[left_some];
    let none_block = &body.basic_blocks[left_none];
    if !matches!(some_block.statements.as_slice(), [right, tag]
        if tuple_reference(tcx, instance, &right.kind, contract, 10, 1)
            && discriminant(&tag.kind, 4, 10))
        || !matches!(none_block.statements.as_slice(), [right, tag]
            if tuple_reference(tcx, instance, &right.kind, contract, 11, 1)
                && discriminant(&tag.kind, 5, 11))
    {
        return false;
    }
    let Some(second) = switch(&some_block.terminator().kind, 4, contract) else {
        return false;
    };
    let Some(third) = switch(&none_block.terminator().kind, 5, contract) else {
        return false;
    };
    if second.otherwise() != invalid || third.otherwise() != invalid {
        return false;
    }
    let both_some = second.target_for_value(contract.some_discriminant);
    let some_none = second.target_for_value(contract.none_discriminant);
    let none_some = third.target_for_value(contract.some_discriminant);
    let both_none = third.target_for_value(contract.none_discriminant);
    let both_block = &body.basic_blocks[both_some];
    let Some(done) = call_target_with_operands(
        tcx,
        instance,
        &both_block.terminator().kind,
        contract,
        0,
        7,
        8,
        copy_local,
    ) else {
        return false;
    };
    let mut roles = [
        entry, invalid, left_some, left_none, both_none, none_some, some_none, both_some, done,
    ]
    .map(|block| block.as_usize());
    roles.sort_unstable();
    if roles != [0, 1, 2, 3, 4, 5, 6, 7, 8] {
        return false;
    }
    let invalid_block = &body.basic_blocks[invalid];
    let done_block = &body.basic_blocks[done];
    invalid_block.statements.is_empty()
        && matches!(invalid_block.terminator().kind, TerminatorKind::Unreachable)
        && result(tcx, body, both_none, done, true)
        && result(tcx, body, none_some, done, false)
        && result(tcx, body, some_none, done, false)
        && matches!(both_block.statements.as_slice(), [left, left_payload, right, right_payload]
            if tuple_reference(tcx, instance, &left.kind, contract, 12, 0)
                && borrow_payload(tcx, instance, &left_payload.kind, contract, 7, 12)
                && tuple_reference(tcx, instance, &right.kind, contract, 13, 1)
                && borrow_payload(tcx, instance, &right_payload.kind, contract, 8, 13))
        && done_block.statements.is_empty()
        && matches!(done_block.terminator().kind, TerminatorKind::Return)
}
