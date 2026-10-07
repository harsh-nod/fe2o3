//! Authentic call-record projections without constructor-state or definedness facts.

use super::*;

fn headers() -> usize {
    12 * size_of::<usize>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, (usize, bool, u32)>>>()
        + size_of::<std::iter::Rev<std::ops::Range<usize>>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<(usize, &(usize, bool, u32))>()
        + size_of::<Result<usize>>()
}

pub(super) fn emit(
    root: usize,
    pc: usize,
    hint: &SourceCutHintsV85,
    call: &SourceCallHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(9)?;
    if hint.statements != 0 || hint.operands != call.arguments.len() {
        return Err(mismatch());
    }
    emit!(
        out,
        "#[verifier::spinoff_prover]\nproof fn invocation_constructor_source_record_{root}_{pc}_v181(source: InvocationSourceByteStateV36)\n requires source.machine.pc == {pc}, invocation_source_block_runtime_{root}_v36(source).source.machine.valid,\n ensures ({{\n let copied_0 = source;\n"
    );
    for (ordinal, (local, moved, bits)) in call.arguments.iter().enumerate() {
        out.budget.charge_work(3)?;
        if *moved {
            return Err(mismatch());
        }
        let next = add(ordinal, 1)?;
        emit!(
            out,
            " let captured_{ordinal} = invocation_source_value_evaluate_v42(copied_{ordinal}, InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local: {local}int, moved: false }}, bits: {bits}int }}, {root}, {}, invocation_runtime_little_endian_v36());\n let copied_{next} = captured_{ordinal}.source;\n",
            hint.instance
        );
    }
    emit!(out, " let entered_arguments = seq![");
    for ordinal in 0..call.arguments.len() {
        out.budget.charge_work(1)?;
        emit!(out, "captured_{ordinal}.value,");
    }
    emit!(
        out,
        "];\n let result = invocation_source_block_runtime_{root}_v36(source);\n result.source == invocation_source_enter_{root}_{}_v36(copied_{}, entered_arguments, invocation_runtime_little_endian_v36())\n && result.returned.is_none() && result.observations.len() == 0 && result.operands.len() == {}\n",
        call.child,
        call.arguments.len(),
        call.arguments.len()
    );
    for (ordinal, (local, _, bits)) in call.arguments.iter().enumerate() {
        out.budget.charge_work(5)?;
        let next = add(ordinal, 1)?;
        emit!(
            out,
            " && result.operands[{ordinal}].before == copied_{ordinal}\n && result.operands[{ordinal}].after == copied_{next}\n && result.operands[{ordinal}].value == captured_{ordinal}.value\n && result.operands[{ordinal}].operand == (InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local: {local}int, moved: false }}, bits: {bits}int }})\n"
        );
    }
    emit!(
        out,
        " }}),\n{{\n hide(invocation_source_enter_{root}_{}_v36);\n hide(invocation_source_entry_refuses_{root}_{}_v167);\n hide(invocation_source_entry_body_{root}_{}_v167);\n hide(invocation_source_value_evaluate_v42);\n hide(invocation_source_byte_state_well_formed_v36);\n hide(invocation_source_byte_value_typed_v36);\n reveal_with_fuel(invocation_source_micro_run_{root}_{}_v36, 1);\n assert(invocation_source_active_{root}_{}_v36(source));\n let guarded_0 = if source.logical.execution_pending.dom().len() == 0 {{ source }} else {{ invocation_source_byte_refused_v36(source) }};\n",
        call.child,
        call.child,
        call.child,
        hint.instance,
        hint.instance
    );
    for (ordinal, (local, _, bits)) in call.arguments.iter().enumerate() {
        out.budget.charge_work(3)?;
        let next = add(ordinal, 1)?;
        emit!(
            out,
            " let evaluated_{ordinal} = invocation_source_value_evaluate_v42(guarded_{ordinal}, InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local: {local}int, moved: false }}, bits: {bits}int }}, {root}, {}, invocation_runtime_little_endian_v36());\n let guarded_{next} = evaluated_{ordinal}.source;\n",
            hint.instance
        );
    }
    emit!(out, " let actual_arguments = seq![");
    for ordinal in 0..call.arguments.len() {
        out.budget.charge_work(1)?;
        emit!(out, "evaluated_{ordinal}.value,");
    }
    let count = call.arguments.len();
    out.budget.charge_work(3)?;
    emit!(
        out,
        "];\n assert(invocation_source_block_runtime_{root}_v36(source).source == invocation_source_enter_{root}_{}_v36(guarded_{count}, actual_arguments, invocation_runtime_little_endian_v36()));\n assert(guarded_{count}.machine.valid) by {{\n  reveal(invocation_source_enter_{root}_{}_v36);\n  invocation_source_entry_select_success_v167(guarded_{count}, invocation_source_entry_refuses_{root}_{}_v167(guarded_{count}, actual_arguments, invocation_runtime_little_endian_v36()), invocation_source_entry_body_{root}_{}_v167(guarded_{count}, actual_arguments, invocation_runtime_little_endian_v36()));\n }}\n",
        call.child,
        call.child,
        call.child,
        call.child
    );
    // Each successful copied operand is its input state, so validity propagates
    // backward through the actual guarded chain without opening the evaluator.
    for ordinal in (0..count).rev() {
        out.budget.charge_work(3)?;
        let (local, _, bits) = call.arguments[ordinal];
        emit!(
            out,
            " invocation_source_scalar_copy_valid_identity_v164(guarded_{ordinal}, {local}, {bits}, {root}, {}, invocation_runtime_little_endian_v36());\n assert(guarded_{ordinal}.machine.valid);\n",
            hint.instance
        );
    }
    emit!(
        out,
        " assert(source.logical.execution_pending.dom().len() == 0);\n assert(guarded_0 == source);\n}}\n"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_record_headers_cover_forward_capture_and_reverse_validity_replay() {
        let forward = size_of::<std::iter::Enumerate<std::slice::Iter<'_, (usize, bool, u32)>>>()
            + size_of::<(usize, &(usize, bool, u32))>();
        let reverse = size_of::<std::iter::Rev<std::ops::Range<usize>>>()
            + size_of::<std::ops::Range<usize>>();
        assert_eq!(
            headers(),
            12 * size_of::<usize>() + forward + reverse + size_of::<Result<usize>>()
        );
    }
}
