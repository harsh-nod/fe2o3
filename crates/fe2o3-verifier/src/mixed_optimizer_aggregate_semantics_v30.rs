//! Concrete selected private-memory refinement over exact original/final CFGs.
//! Generated obligations are not executed proof or source allocation authority.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirAggregateSsaActionV18 as Action, CanonicalKirAggregateSsaMemoryEventV18 as Memory,
    CanonicalKirAggregateSsaWitnessV18 as Witness, CheckedCanonicalKirAggregateSsaV18 as Pair,
};
#[path = "mixed_optimizer_aggregate_memory_plan_v30.rs"]
mod memory_plan;
use memory_plan::MemoryPlan;
#[path = "mixed_optimizer_aggregate_memory_graph_v30.rs"]
mod graph;
#[path = "mixed_optimizer_aggregate_memory_relation_v30.rs"]
mod relation;
#[path = "mixed_optimizer_aggregate_memory_trace_v30.rs"]
mod trace;

pub(in crate::mixed_optimizer_refinement_v26) fn generate(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    pair: &Pair<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    if !input.belongs_to(pair.input()) || !output.belongs_to(pair.output()) {
        return Err(Error::Statement(
            "aggregate generator exact checked endpoint owners",
        ));
    }
    let floor = out.budget.storage();
    let generated = (|| {
        out.budget.reserve_storage(
            size_of::<Plan>()
                + size_of::<Result<usize>>()
                + size_of::<(
                    &Inventory<'_>,
                    &Inventory<'_>,
                    &Pair<'_>,
                    &mut Writer<'_, '_>,
                )>(),
        )?;
        let mut memory = MemoryPlan::build(input, output, pair.witness(), out)?;
        // The shared emitter only needs the exact unchanged-operation map.
        // Move that paid map instead of constructing another serialized graph.
        let scalar = Plan {
            anchors: Vec::new(),
            input_operations: Vec::new(),
            output_operations: std::mem::take(&mut memory.output_origins),
            blocks: Vec::new(),
            seen_definitions: Vec::new(),
            seen_operations: Vec::new(),
            pending_definitions: Vec::new(),
            total_classes: None,
        };
        emit!(
            out,
            "use vstd::prelude::*;\nuse vstd::seq_lib::*;\nverus! {{\n{}\n// V30 concrete original private storage and exact actual secondary SSA graph.\nopen spec fn signed(x: int, m: int) -> int {{ if x < m / 2 {{ x }} else {{ x - m }} }}\n",
            cfg_trace::PRELUDE
        );
        select_prelude(input, output, out)?;
        for width in [8, 16, 32, 64] {
            let max = (1u128 << width) - 1;
            emit!(
                out,
                "proof fn bit_identity_{width}(x: u{width})\n ensures (x & {max}u{width}) == x, (x | 0u{width}) == x, (x ^ 0u{width}) == x,\n{{\n assert((x & {max}u{width}) == x) by(bit_vector);\n assert((x | 0u{width}) == x) by(bit_vector);\n assert((x ^ 0u{width}) == x) by(bit_vector);\n}}\n"
            );
        }
        graph::generate(input, output, &scalar, &memory, pair.witness(), out)?;
        relation::generate(input, output, &scalar, &memory, pair.witness(), out)?;
        trace::generate(input, output, &memory, out)?;
        emit!(out, "}}\nfn main() {{}}\n");
        Ok(output.blocks().len())
    })();
    let release = out
        .budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    let settled = out.budget.release_storage(release).map_err(Error::from);
    match generated {
        Err(error) => Err(error),
        Ok(blocks) => settled.map(|()| blocks),
    }
}

#[cfg(test)]
pub(in crate::mixed_optimizer_refinement_v26) fn check_omissions(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    pair: &Pair<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    let mut memory = MemoryPlan::build(input, output, pair.witness(), out)?;
    let mut count = 0;
    for cell in 0..memory.requirements.len() {
        let original = memory.requirements[cell];
        if original == NONE {
            continue;
        }
        memory.requirements[cell] = NONE;
        assert!(matches!(
            memory.validate(input, output, pair.witness(), out),
            Err(Error::Statement(
                "aggregate closure omits actual phi memory"
                    | "aggregate closure omits original read memory"
                    | "aggregate complete exact edge memory closure"
            ))
        ));
        memory.requirements[cell] = original;
        memory.validate(input, output, pair.witness(), out)?;
        count += 1;
    }
    Ok(count)
}
