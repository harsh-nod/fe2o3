//! Constructed paired prestates, not root-initializer or reachable-prefix proofs.
use super::*;
use std::fmt::Write as _;

fn emit_witness(
    segment: &CheckedTargetSegment<'_, '_, '_, '_>,
    plan: &InvocationPlan<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    segment.check(segment.program, segment.target, out)?;
    let row = plan.instance(segment.root, segment.instance, out)?;
    let n = row.locals.end;
    let mut owners = Vec::new();
    let mut instance = segment.instance;
    loop {
        let row = plan.instance(segment.root, instance, out)?;
        assert!(row.active);
        owners.push(row.function.index());
        match row.incoming {
            Some((parent, _)) => {
                assert!(parent < instance);
                instance = parent;
            }
            None => {
                assert_eq!(instance, 0);
                break;
            }
        }
    }
    owners.reverse();
    let inventory = segment.target.inventory(out)?;
    let target_owner = segment.target.root_function(segment.root, out)?.0;
    let definitions = inventory.definitions().len();
    // This fixture has no variant contract rows. We nevertheless use the actual
    // classified registry (namespace 1), not an unrelated empty registry.
    assert!(
        inventory
            .owner()
            .module()
            .storage_layouts
            .iter()
            .all(|row| !matches!(
                row.kind,
                fe2o3_kernel_ir::StorageLayoutKindV1::Variants { .. }
            ))
    );
    for index in [
        segment.checked.operands[0],
        segment.checked.operands[1],
        segment.checked.results[0],
        segment.checked.results[1],
    ] {
        assert!(index < definitions);
    }
    for local in [
        segment.destination,
        segment.source_operands[0],
        segment.source_operands[1],
    ] {
        assert!(local < n);
    }
    let (r, i, b, k) = (
        segment.root,
        segment.instance,
        segment.block,
        segment.statement,
    );
    let source_pc = segment.source_pc;
    let (target_pc, op, prefix) = (
        segment.target_block,
        segment.target_operation,
        segment.target_prefix,
    );
    let target_local_block = inventory.operations()[op].coordinate.block.block;
    let [value_result, overflow_result] = segment.checked.results;
    let namespace = super::super::super::super::TARGET_TAG_NAMESPACE_V40;
    write!(out, r#"// Synthetic old histories satisfy only the documented prefix-length guards.
// No source/target root trajectory or cross-machine heap/frame relation is claimed.
spec fn checked_pair_source_{r}_{i}_{b}_{k}_v298(overflowing: bool) -> InvocationSourceMicroStateV36 {{
 let execution = MemoryExecutionContextV37 {{
 rank: 1, extent: seq![1int, 1int, 1int], workgroup: seq![1int, 1int, 1int],
 group: seq![0int, 0int, 0int], local: seq![0int, 0int, 0int],
 ieee_operators: |operation: int, bits: int, a: int, b: int, c: int, mode: int| 0int,
 }};
 let frames = byte_root_frame_with_execution_v37({}, execution);
"#, owners[0]).map_err(|_| out.error())?;
    for owner in &owners[1..] {
        writeln!(out, " let frames = byte_enter_frame_v30(frames, {owner});")
            .map_err(|_| out.error())?;
    }
    write!(out, r#" let source = InvocationSourceByteStateV36 {{
 machine: MemoryStateV30 {{ pc: {source_pc}, values: Seq::new({n}, |local: int| MemoryValueV30::Scalar(if overflowing {{ 2147483648int }} else {{ 0int }})),
 memory: ByteMemoryV30 {{ live: Map::empty(), view_contracts: byte_empty_view_contracts_v38() }},
 generations: Map::empty(), frames, valid: true }},
 slots: Map::empty(), objects: Map::empty(), logical: invocation_source_logical_initial_v38({n}),
 }};
 InvocationSourceMicroStateV36 {{ source, next_statement: {k},
 observations: Seq::new({k}, |old: int| InvocationSourceStatementObservationV36 {{
 root: {r}, instance: {i}, block: {b}, statement: old, event: None, before: source, after: source,
 }}) }}
}}
spec fn checked_pair_target_{r}_{i}_{b}_{k}_v298(overflowing: bool) -> MemoryMicroStateV30 {{
 let state = MemoryStateV30 {{ pc: {target_pc}, values: Seq::new({definitions}, |value: int| MemoryValueV30::Scalar(if overflowing {{ 2147483648int }} else {{ 0int }})),
 memory: ByteMemoryV30 {{ live: Map::empty(), view_contracts: byte_target_view_contracts_{namespace}_v38(true) }},
 generations: Map::empty(), frames: byte_root_frame_v30({target_owner}), valid: true }};
 MemoryMicroStateV30 {{ state, next_operation: {op},
 observations: Seq::new({prefix}, |old: int| MemoryOperationObservationV30 {{
 operation: MemorySourceOperationV30 {{ function: {target_owner}, block: {target_local_block}, operation: old }},
 before: state, after: state, valid_before: true, valid_after: true, effect: MemoryOperationEffectV30::Pure,
 }}) }}
}}
proof fn checked_pair_constructed_step_{r}_{i}_{b}_{k}_v298(overflowing: bool)
 ensures ({{ let s = checked_pair_source_{r}_{i}_{b}_{k}_v298(overflowing);
 let t = checked_pair_target_{r}_{i}_{b}_{k}_v298(overflowing);
 let operand = if overflowing {{ 2147483648int }} else {{ 0int }};
 checked_actual_segment_inputs_{r}_{i}_{b}_{k}_v298(s, t, operand, operand, true)
 && checked_actual_segment_results_{r}_{i}_{b}_{k}_v298(s, t,
 invocation_source_micro_step_{r}_{i}_v36(s, true), byte_micro_step_{r}_v30(t, true), operand, operand)
 && byte_micro_step_{r}_v30(t, true).next.state.values[{value_result}] == MemoryValueV30::Scalar(0)
 && byte_micro_step_{r}_v30(t, true).next.state.values[{overflow_result}] == MemoryValueV30::Scalar(if overflowing {{ 1int }} else {{ 0int }}) }}),
{{
 let s = checked_pair_source_{r}_{i}_{b}_{k}_v298(overflowing);
 let t = checked_pair_target_{r}_{i}_{b}_{k}_v298(overflowing);
 let operand = if overflowing {{ 2147483648int }} else {{ 0int }};
 reveal(checked_pair_source_{r}_{i}_{b}_{k}_v298);
 reveal(checked_pair_target_{r}_{i}_{b}_{k}_v298);
 assert(s.source.machine.frames.active.len() == {});
"#, owners.len()).map_err(|_| out.error())?;
    for (depth, owner) in owners.iter().enumerate() {
        writeln!(out, " assert(s.source.machine.frames.active[{depth}].owner == {owner});\n assert(s.source.machine.frames.active[{depth}].invocation == {depth});").map_err(|_| out.error())?;
    }
    write!(out, r#" assert(match s.source.machine.frames.execution {{ Some(execution) => byte_execution_well_formed_v37(execution), None => false }});
 assert(byte_frame_runtime_well_formed_v30(s.source.machine.frames));
 assert(s.source.machine.memory.live =~= Map::empty());
 assert(s.source.slots =~= Map::empty());
 assert(s.source.objects =~= Map::empty());
 assert(s.source.logical.execution_pending =~= Map::empty());
 assert(invocation_source_byte_state_well_formed_v36(s.source));
 reveal(invocation_source_active_{r}_{i}_v36);
 reveal(byte_target_view_contracts_{namespace}_v38);
 assert(t.state.memory.view_contracts.rows =~= Map::empty());
 assert(byte_state_memory_well_formed_v30(t.state));
 reveal(byte_inputs_{r}_v55);
 assert(byte_inputs_{r}_v55(t.state, true));
 reveal(checked_actual_segment_inputs_{r}_{i}_{b}_{k}_v298);
 assert(checked_actual_segment_inputs_{r}_{i}_{b}_{k}_v298(s, t, operand, operand, true));
 checked_actual_source_target_step_{r}_{i}_{b}_{k}_v298(s, t, operand, operand, true);
 reveal(checked_actual_segment_results_{r}_{i}_{b}_{k}_v298);
}}
"#).map_err(|_| out.error())?;
    segment.check(segment.program, segment.target, out)
}

fn model(layout: Layout, examine: impl FnOnce(&str)) {
    fixture(layout, SourceOp::Add, LIMIT, LIMIT, |plan, slots, out| {
        let model = ExpandedGenerationV221::derive(
            plan,
            slots,
            FormalIndexWidth::Bits64,
            EndiannessV2::Little,
            out,
        )?;
        model.emit_support(out)?;
        let count = inspect_segments(plan, slots, out, |segment, out| {
            emit_witness(segment, plan, out)
        })?;
        assert!(count > 0);
        model.finish(out)?;
        assert_eq!(
            out.text
                .matches("proof fn checked_pair_constructed_step_")
                .count(),
            count
        );
        assert_eq!(
            out.text
                .matches("proof fn checked_actual_source_target_step_")
                .count(),
            count
        );
        assert!(
            out.text
                .starts_with("use vstd::prelude::*;\nuse vstd::seq_lib::*;\nverus! {\n")
        );
        assert!(out.text.ends_with("}\n"));
        examine(&out.text);
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn actual_checked_pair_witness_derives_prestates_without_reachability_premises() {
    for layout in [Layout::Blocked, Layout::Striped] {
        model(layout, |text| {
            for tail in text
                .split("proof fn checked_pair_constructed_step_")
                .skip(1)
            {
                let (contract, body) = tail.split_once("\n{\n").unwrap();
                assert!(!contract.contains("requires"));
                assert!(contract.contains("checked_actual_segment_inputs_"));
                assert!(contract.contains("checked_actual_segment_results_"));
                assert!(body.contains("checked_actual_source_target_step_"));
            }
            for forbidden in ["assume(", "admit(", "external_body", "assume_specification"] {
                assert!(!text.contains(forbidden));
            }
            assert!(text.contains("view_contracts: byte_target_view_contracts_1_v38(true)"));
            assert!(text.contains("Synthetic old histories"));
            assert!(text.contains("No source/target root trajectory"));
        });
    }
}

#[test]
#[ignore = "complete conditional Checked segment models and constructed prestates; no root reachability or production authority"]
fn diagnostic_complete_checked_source_target_segment_models_export_v298() {
    use sha2::{Digest, Sha256};
    use std::io::{BufWriter, Write as _};
    for (layout, label) in [(Layout::Blocked, "blocked"), (Layout::Striped, "striped")] {
        model(layout, |text| {
            assert!(text.len() <= 16 * 1024 * 1024);
            let mut output = BufWriter::new(std::io::stdout().lock());
            write!(output, "{{\"kind\":\"fe2o3-checked-source-target-segment-model-v298\",\"authority\":false,\"root_reachability\":false,\"layout\":\"{label}\",\"bytes\":{},\"sha256\":\"", text.len()).unwrap();
            for byte in Sha256::digest(text.as_bytes()) {
                write!(output, "{byte:02x}").unwrap();
            }
            write!(output, "\",\"model_hex\":\"").unwrap();
            for byte in text.as_bytes() {
                write!(output, "{byte:02x}").unwrap();
            }
            writeln!(output, "\"}}").unwrap();
            output.flush().unwrap();
        });
    }
}
