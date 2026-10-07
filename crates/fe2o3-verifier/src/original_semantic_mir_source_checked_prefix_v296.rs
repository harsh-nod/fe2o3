//! General retained-event scan; no fixture or kernel-name selection.
use super::super::source_frame_plan::{FramePlan, prefix::Prefix};
use super::*;

fn headers() -> usize {
    size_of::<FramePlan<'_, '_, '_, '_>>()
        + size_of::<Result<FramePlan<'_, '_, '_, '_>>>()
        + 2 * size_of::<Prefix<'_, '_, '_, '_, '_>>()
        + 2 * size_of::<Result<Prefix<'_, '_, '_, '_, '_>>>()
        + 2 * size_of::<Event>()
        + 2 * size_of::<Result<Event>>()
        + 2 * size_of::<Option<usize>>()
        + size_of::<Option<(usize, usize, usize)>>()
        + 4 * size_of::<Range<usize>>()
        + 20 * size_of::<usize>()
        + 14 * size_of::<&()>()
        + 4 * size_of::<Result<()>>()
}

impl SourceByteProgram<'_, '_, '_> {
    pub(in super::super) fn emit_checked_prefix_projections_v296(
        &self,
        plan: &InvocationPlan<'_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        let emit = |out: &mut Writer<'_, '_>| {
            self.source_slots(out)?;
            out.budget.reserve_storage(headers())?;
            let frames = FramePlan::derive(plan, self.slots, out)?;
            let mut count = 0usize;
            for root in 0..self.roots.len() {
                out.budget.charge_work(1)?;
                let range = &self.roots[root].0;
                for instance in 0..range.len() {
                    out.budget.charge_work(4)?;
                    let Some(function) = &self.functions[range.start + instance] else {
                        continue;
                    };
                    let original = plan.instance(root, instance, out)?;
                    if !original.active
                        || function.root != root
                        || function.instance != instance
                        || function.blocks != original.blocks
                    {
                        return Err(mismatch());
                    }
                    function
                        .body
                        .check_prefix_owner_v296(plan, root, instance, out)?;
                    for block in 0..function.control.len() {
                        out.budget.charge_work(1)?;
                        for statement in 0..function.control[block].statements {
                            out.budget.charge_work(5)?;
                            let Some(pc) = function
                                .checked_micro_pc_v293(root, instance, block, statement, out)?
                            else {
                                continue;
                            };
                            let event = function.body.event_at(block, statement, out)?;
                            if event.checked_prefix_site_v296().is_none() {
                                continue;
                            }
                            let before = frames
                                .partial_prefix_v296(root, instance, block, statement, out)?;
                            let after = frames.partial_prefix_v296(
                                root,
                                instance,
                                block,
                                statement.checked_add(1).ok_or(Resource::Arithmetic)?,
                                out,
                            )?;
                            if before.pc != pc || after.pc != pc {
                                return Err(mismatch());
                            }
                            event.emit_checked_prefix_v296(&frames, &before, &after, out)?;
                            after.discard(out)?;
                            before.discard(out)?;
                            count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
                        }
                    }
                }
            }
            frames.check(plan, self.slots, out)?;
            // FramePlan retains conservative scratch; the enclosing source scope
            // owns its refund. Prefix results themselves drop before their refund.
            self.source_slots(out)?;
            Ok(count)
        };
        out.budget
            .reserve_storage(2 * std::mem::size_of_val(&emit) + std::mem::align_of_val(&emit))?;
        self.slots.with_source_query_v42(out, emit)
    }
}

#[cfg(test)]
#[path = "original_semantic_mir_source_checked_prefix_v296_tests.rs"]
mod tests;
