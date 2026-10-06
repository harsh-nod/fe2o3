//! Actual target execution between exact source-cut candidates. This does not
//! establish value, heap, effect or execution-lifetime correspondence.
use super::{Result, TileMicroCutsV180, Writer};
use std::fmt::Write as _;

const SHARED: &str = include_str!("original_semantic_mir_tile_microexecution_v181.vrs");

#[cfg(test)]
#[path = "original_semantic_mir_tile_microexecution_v181_tests.rs"]
mod tests;

pub(super) fn emit(
    model: &TileMicroCutsV180<'_, '_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    model.check(out)?;
    write!(out, "{SHARED}").map_err(|_| out.error())?;
    for root in 0..model.roots.len() {
        out.budget.charge_work(1)?;
        write!(
            out,
            r#"
spec fn invocation_tile_micro_advance_{root}_v181(
    before: MemoryMicroStateV30, little_endian: bool,
) -> InvocationTileMicroStepV181 {{
    if before.next_operation != -1 {{
        invocation_tile_micro_operation_v181(byte_micro_step_{root}_v30(before, little_endian))
    }} else {{
        let finished = byte_micro_finish_{root}_v30(before);
        // Control consumes the full retained prefix; it must not emit that
        // already-accounted prefix a second time at an interior source cut.
        let state = MemoryStateV30 {{
            valid: finished.state.valid && finished.observations == before.observations,
            ..finished.state
        }};
        let terminal = state.pc < 0;
        let next = if terminal || !state.valid {{
            MemoryMicroStateV30 {{ state, ..before }}
        }} else {{ byte_micro_begin_{root}_v30(state) }};
        InvocationTileMicroStepV181 {{
            next, observations: seq![], returned: finished.returned, terminal,
        }}
    }}
}}

spec fn invocation_tile_micro_seek_{root}_v181(
    source_pc: int, start: MemoryMicroStateV30, little_endian: bool, fuel: nat,
) -> InvocationTileMicroSegmentV181
    decreases fuel
{{
    if invocation_tile_cursor_{root}_v180(source_pc, start) {{
        invocation_tile_micro_zero_v181(start)
    }} else if !start.state.valid || fuel == 0 {{
        invocation_tile_micro_exhausted_v181(start)
    }} else {{
        let head = invocation_tile_micro_advance_{root}_v181(start, little_endian);
        if head.terminal || !head.next.state.valid {{
            invocation_tile_micro_last_v181(start, head)
        }} else {{
            let tail = invocation_tile_micro_seek_{root}_v181(
                source_pc, head.next, little_endian, (fuel - 1) as nat,
            );
            invocation_tile_micro_join_v181(start, head, tail)
        }}
    }}
}}

proof fn invocation_tile_micro_zero_is_exact_candidate_{root}_v181(
    source_pc: int, start: MemoryMicroStateV30, little_endian: bool, fuel: nat,
)
    requires invocation_tile_cursor_{root}_v180(source_pc, start),
    ensures invocation_tile_micro_seek_{root}_v181(source_pc, start, little_endian, fuel)
        == invocation_tile_micro_zero_v181(start),
{{ }}
"#
        )
        .map_err(|_| out.error())?;
    }
    model.check(out)
}
