//! Compose independently generated prefix and relocation models through an
//! exact operator interpretation bridge, never by equating receipt digests.
use super::*;
use fe2o3_kernel_analysis::{
    CheckedCanonicalKirLicmV18 as Pair, CheckedCanonicalKirTransitionV18 as Prefix,
};

fn keys(
    prefix: &Inventory<'_>,
    original: &Plan,
    tail: &Plan,
    ordinal: usize,
) -> Result<Option<(usize, usize)>> {
    let row = &prefix.operations()[ordinal];
    if matches!(
        row.operation.kind,
        OperationKind::Constant(_) | OperationKind::Select { .. }
    ) {
        return Ok(None);
    }
    let before = original.output_operations[ordinal];
    if before == NONE {
        return Err(Error::Statement("composed nonconstant original occurrence"));
    }
    let pure = total(&row.operation.kind);
    Ok(Some((
        total_interpretation(tail, ordinal, !pure)?,
        total_interpretation(original, before, !pure)?,
    )))
}

fn bridge(
    prefix: &Inventory<'_>,
    original: &Plan,
    tail: &Plan,
    out: &mut Writer<'_, '_>,
) -> Result<Vec<usize>> {
    out.budget
        .reserve_storage(size_of::<Vec<usize>>() + align_of::<Vec<usize>>())?;
    let count = prefix
        .operations()
        .len()
        .checked_mul(2)
        .ok_or(Resource::Arithmetic)?;
    let mut bridge = allocate(count, out)?;
    for ordinal in 0..prefix.operations().len() {
        out.budget.charge_work(5)?;
        let Some((key, source)) = keys(prefix, original, tail, ordinal)? else {
            continue;
        };
        let cell = bridge
            .get_mut(key)
            .ok_or(Error::Statement("bounded prefix interpretation key"))?;
        if *cell != NONE && *cell != source {
            return Err(Error::Statement(
                "prefix congruence changed original interpretation",
            ));
        }
        *cell = source;
    }
    replay_bridge(prefix, original, tail, &bridge, out)?;
    Ok(bridge)
}

fn replay_bridge(
    prefix: &Inventory<'_>,
    original: &Plan,
    tail: &Plan,
    bridge: &[usize],
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let expected = prefix
        .operations()
        .len()
        .checked_mul(2)
        .ok_or(Resource::Arithmetic)?;
    if bridge.len() != expected {
        return Err(Error::Statement("complete composed operator key census"));
    }
    let floor = out.budget.storage();
    let checked = (|| {
        out.budget
            .reserve_storage(size_of::<Vec<usize>>() + align_of::<Vec<usize>>())?;
        let mut seen = allocate(expected, out)?;
        for (ordinal, row) in prefix.operations().iter().enumerate() {
            out.budget.charge_work(6)?;
            if matches!(
                row.operation.kind,
                OperationKind::Constant(_) | OperationKind::Select { .. }
            ) {
                continue;
            }
            let original_operation = original.output_operations[ordinal];
            let (key, source) = if total(&row.operation.kind) {
                let tail_classes = tail
                    .total_classes
                    .as_ref()
                    .ok_or(Error::Statement("tail total interpretation census"))?;
                let original_classes = original
                    .total_classes
                    .as_ref()
                    .ok_or(Error::Statement("original total interpretation census"))?;
                (
                    *tail_classes
                        .get(ordinal)
                        .ok_or(Error::Statement("prefix total key"))?,
                    *original_classes
                        .get(original_operation)
                        .ok_or(Error::Statement("original total key"))?,
                )
            } else {
                (ordinal, original_operation)
            };
            if source == NONE || bridge.get(key) != Some(&source) {
                return Err(Error::Statement("exact composed operator interpretation"));
            }
            *seen
                .get_mut(key)
                .ok_or(Error::Statement("composed operator key range"))? = 1;
        }
        for (key, source) in bridge.iter().enumerate() {
            out.budget.charge_work(2)?;
            if (seen[key] != NONE) != (*source != NONE) {
                return Err(Error::Statement("invented composed operator key"));
            }
        }
        Ok(())
    })();
    out.budget.release_storage(
        out.budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    checked
}

#[cfg(test)]
pub(in super::super) fn bridge_negative_controls(
    checked: &Prefix<'_, '_, '_, '_>,
    output: &Inventory<'_>,
    pair: &Pair<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let floor = out.budget.storage();
    let result = (|| {
        let original = checked.input();
        let prefix = checked.output();
        let mut first = Plan::build(original, prefix, checked.rows(), out)?;
        first.total_classes = Some(congruence_v27::build(
            original,
            prefix,
            &first.output_operations,
            out,
        )?);
        let second = relocation_v28::plan(prefix, output, pair, out)?;
        let mut map = bridge(prefix, &first, &second, out)?;
        let live = map.iter().position(|key| *key != NONE).unwrap();
        let vacant = map.iter().position(|key| *key == NONE).unwrap();
        let correct = map[live];
        map[live] = NONE;
        assert!(replay_bridge(prefix, &first, &second, &map, out).is_err());
        map[live] = correct + 1;
        assert!(replay_bridge(prefix, &first, &second, &map, out).is_err());
        map[live] = correct;
        map[vacant] = correct;
        assert!(replay_bridge(prefix, &first, &second, &map, out).is_err());
        map[vacant] = NONE;
        assert!(replay_bridge(prefix, &first, &second, &map[..map.len() - 1], out).is_err());
        let count = prefix.operations().len();
        let ordered = (0..count).find(|key| map[*key] != NONE).unwrap();
        let total = (count..map.len()).find(|key| map[*key] != NONE).unwrap();
        map.swap(ordered, total);
        assert!(replay_bridge(prefix, &first, &second, &map, out).is_err());
        map.swap(ordered, total);
        replay_bridge(prefix, &first, &second, &map, out)
    })();
    out.budget.release_storage(
        out.budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    result
}

fn emit_bridge(bridge: &[usize], out: &mut Writer<'_, '_>) -> Result<()> {
    emit!(
        out,
        "verus! {{\nopen spec fn original_interpretation_v28(op: spec_fn(int, int, Seq<int>, int) -> int, key: int, result: int, arguments: Seq<int>, memory: int) -> int {{\n"
    );
    for (key, &source) in bridge.iter().enumerate() {
        out.budget.charge_work(1)?;
        if source != NONE {
            emit!(
                out,
                " if key == {key} {{ op({source}, result, arguments, memory) }} else "
            );
        }
    }
    emit!(
        out,
        "{{ 0int }}\n}}\nopen spec fn prefix_operator_v28(op: spec_fn(int, int, Seq<int>, int) -> int) -> spec_fn(int, int, Seq<int>, int) -> int {{\n |key: int, result: int, arguments: Seq<int>, memory: int| original_interpretation_v28(op, key, result, arguments, memory)\n}}\n"
    );
    emit!(
        out,
        "open spec fn parent_state_v28(s: CfgStateV26) -> super::CfgStateV26 {{ super::CfgStateV26 {{ pc: s.pc, values: s.values, memory: s.memory }} }}\nopen spec fn child_state_v28(s: super::CfgStateV26) -> CfgStateV26 {{ CfgStateV26 {{ pc: s.pc, values: s.values, memory: s.memory }} }}\n"
    );
    Ok(())
}

fn emit_step_bridge(
    original: &Inventory<'_>,
    prefix: &Inventory<'_>,
    final_output: &Inventory<'_>,
    first: &Plan,
    second: &Plan,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let definitions = prefix.definitions().len();
    emit!(
        out,
        "proof fn prefix_step_interpretations_agree_v28(s: CfgStateV26, source_op: spec_fn(int, int, Seq<int>, int) -> int)\n requires s.values.len() == {definitions},\n ensures super::cfg_step_o_v26(parent_state_v28(s), source_op).events == cfg_step_n_v26(s, prefix_operator_v28(source_op)).events,\n super::cfg_step_o_v26(parent_state_v28(s), source_op).halted == cfg_step_n_v26(s, prefix_operator_v28(source_op)).halted,\n child_state_v28(super::cfg_step_o_v26(parent_state_v28(s), source_op).state) == cfg_step_n_v26(s, prefix_operator_v28(source_op)).state,\n super::cfg_step_o_v26(parent_state_v28(s), source_op).state.values.len() == {definitions},\n{{\n"
    );
    for block in 0..prefix.blocks().len() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " if s.pc == {block} {{\n let base = s.values; let initial = s.memory; let op = source_op;\n"
        );
        // Both bodies execute actual prefix definitions. Only their independently
        // derived semantic keys differ, and the checked bridge joins each key.
        body_in(
            original,
            prefix,
            first,
            first.blocks[block],
            block,
            Side::Output,
            true,
            Environment::ActualDefinitions,
            out,
        )?;
        emit!(out, " let op = prefix_operator_v28(source_op);\n");
        body_in(
            prefix,
            final_output,
            second,
            block,
            block,
            Side::Input,
            true,
            Environment::ActualDefinitions,
            out,
        )?;
        emit!(out, " }} else ");
    }
    emit!(out, "{{ }}\n}}\n");
    emit!(
        out,
        "proof fn prefix_trace_interpretations_agree_v28(s: super::CfgStateV26, op: spec_fn(int, int, Seq<int>, int) -> int, fuel: nat)\n requires s.values.len() == {definitions},\n ensures super::cfg_trace_v26(|p: super::CfgStateV26| super::cfg_step_o_v26(p, op), s, fuel).events == cfg_trace_v26(|p: CfgStateV26| cfg_step_n_v26(p, prefix_operator_v28(op)), child_state_v28(s), fuel).events,\n super::cfg_trace_v26(|p: super::CfgStateV26| super::cfg_step_o_v26(p, op), s, fuel).halted == cfg_trace_v26(|p: CfgStateV26| cfg_step_n_v26(p, prefix_operator_v28(op)), child_state_v28(s), fuel).halted,\n super::cfg_trace_v26(|p: super::CfgStateV26| super::cfg_step_o_v26(p, op), s, fuel).steps == cfg_trace_v26(|p: CfgStateV26| cfg_step_n_v26(p, prefix_operator_v28(op)), child_state_v28(s), fuel).steps,\n decreases fuel,\n{{\n if fuel > 0 {{\n prefix_step_interpretations_agree_v28(child_state_v28(s), op);\n let first = super::cfg_step_o_v26(s, op);\n if !first.halted {{ prefix_trace_interpretations_agree_v28(first.state, op, (fuel - 1) as nat); }}\n }}\n}}\n"
    );
    Ok(())
}

fn emit_composition(
    original: &Inventory<'_>,
    prefix: &Inventory<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    for function in original.functions() {
        out.budget.charge_work(4)?;
        if function.blocks.is_empty() {
            continue;
        }
        let index = function.coordinate.0;
        let next = prefix
            .functions()
            .get(index as usize)
            .ok_or(Error::Statement("composed function census"))?;
        if next.coordinate != function.coordinate || next.blocks.is_empty() {
            return Err(Error::Statement("composed actual prefix function"));
        }
        emit!(
            out,
            "proof fn composed_original_to_final_trace_{index}_v28(n: super::CfgStateV26, p: super::CfgStateV26, o: CfgStateV26, arguments: Seq<int>, op: spec_fn(int, int, Seq<int>, int) -> int, fuel: nat)\n requires super::cfg_entry_n_{index}_v26(n, arguments), super::cfg_entry_o_{index}_v26(p, arguments), cfg_entry_o_{index}_v26(o, arguments), n.memory == p.memory, p.memory == o.memory,\n ensures super::cfg_trace_v26(|s: super::CfgStateV26| super::cfg_step_n_v26(s, op), n, fuel).events == cfg_trace_v26(|s: CfgStateV26| cfg_step_o_v26(s, prefix_operator_v28(op)), o, fuel).events,\n super::cfg_trace_v26(|s: super::CfgStateV26| super::cfg_step_n_v26(s, op), n, fuel).halted == cfg_trace_v26(|s: CfgStateV26| cfg_step_o_v26(s, prefix_operator_v28(op)), o, fuel).halted,\n super::cfg_trace_v26(|s: super::CfgStateV26| super::cfg_step_n_v26(s, op), n, fuel).steps == cfg_trace_v26(|s: CfgStateV26| cfg_step_o_v26(s, prefix_operator_v28(op)), o, fuel).steps,\n{{\n super::cfg_function_trace_refinement_{index}_v26(n, p, arguments, op, fuel);\n prefix_trace_interpretations_agree_v28(p, op, fuel);\n assert(cfg_entry_n_{index}_v26(child_state_v28(p), arguments));\n relocation_function_trace_{index}_v28(child_state_v28(p), o, arguments, prefix_operator_v28(op), fuel);\n}}\n"
        );
    }
    Ok(())
}

pub(in super::super) fn generate(
    checked: &Prefix<'_, '_, '_, '_>,
    final_output: &Inventory<'_>,
    pair: &Pair<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    let original = checked.input();
    let prefix = checked.output();
    if !std::ptr::eq(prefix.owner(), pair.input())
        || !std::ptr::eq(final_output.owner(), pair.output())
    {
        return Err(Error::Statement(
            "exact composed intermediate and final owners",
        ));
    }
    let floor = out.budget.storage();
    let result = (|| {
        let mut first = Plan::build(original, prefix, checked.rows(), out)?;
        first.total_classes = Some(congruence_v27::build(
            original,
            prefix,
            &first.output_operations,
            out,
        )?);
        let second = relocation_v28::plan(prefix, final_output, pair, out)?;
        let bridge = bridge(prefix, &first, &second, out)?;
        // Rust lexical modules keep each independently emitted interpreter's
        // definitions distinct. No generated source is rewritten or substituted.
        emit!(out, "mod original_prefix_v28 {{\n");
        generate_cfg_v27(original, prefix, checked.rows(), out)?;
        emit!(out, "mod relocation_v28 {{\n");
        relocation_v28::generate(prefix, final_output, pair, out)?;
        emit_bridge(&bridge, out)?;
        emit_step_bridge(original, prefix, final_output, &first, &second, out)?;
        emit_composition(original, prefix, out)?;
        emit!(out, "}}\n}}\n}}\nfn main() {{}}\n");
        Ok(final_output.blocks().len())
    })();
    let released = out
        .budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    out.budget.release_storage(released)?;
    result
}
