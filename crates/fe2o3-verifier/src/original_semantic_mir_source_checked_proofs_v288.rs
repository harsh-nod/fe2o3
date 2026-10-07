//! Emit proved source-step specializations from retained source events.
//! This adds no admission rule and does not prove a source/target simulation.
use super::*;

fn headers() -> usize {
    2 * size_of::<Event>()
        + 2 * size_of::<Result<Event>>()
        + 2 * size_of::<Result<bool>>()
        + 2 * size_of::<Result<usize>>()
        + 4 * size_of::<Range<usize>>()
        + 2 * size_of::<Option<usize>>()
        + size_of::<Result<Option<usize>>>()
        + size_of::<Option<usize>>()
        + 6 * size_of::<usize>()
        + 2 * size_of::<&()>()
        + 12 * size_of::<usize>()
        + 8 * size_of::<&()>()
}

impl SourceByteProgram<'_, '_, '_> {
    pub(in super::super) fn emit_checked_local_add_proofs_v288(
        &self,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        self.slots.with_source_query_v42(out, |out| {
            self.source_slots(out)?;
            let headers = headers();
            out.budget.reserve_storage(headers)?;
            let mut count = 0usize;
            for root in 0..self.roots.len() {
                out.budget.charge_work(1)?;
                let range = &self.roots[root].0;
                for instance in 0..range.len() {
                    out.budget.charge_work(2)?;
                    let Some(function) = &self.functions[range.start + instance] else {
                        continue;
                    };
                    if function.root != root || function.instance != instance {
                        return Err(mismatch());
                    }
                    for block in 0..function.control.len() {
                        out.budget.charge_work(1)?;
                        for statement in 0..function.control[block].statements {
                            out.budget.charge_work(1)?;
                            let pc = function
                                .checked_micro_pc_v293(root, instance, block, statement, out)?;
                            if function
                                .body
                                .event_at(block, statement, out)?
                                .emit_checked_local_add_proofs_v288(
                                    root, instance, block, statement, pc, out,
                                )?
                            {
                                count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
                            }
                        }
                    }
                }
            }
            self.source_slots(out)?;
            out.budget.release_storage(headers)?;
            Ok(count)
        })
    }
}

impl SourceByteFunction<'_, '_, '_> {
    fn checked_micro_pc_v293(
        &self,
        root: usize,
        instance: usize,
        block: usize,
        statement: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<usize>> {
        out.budget.charge_work(5)?;
        if self.root != root
            || self.instance != instance
            || self.blocks.len() != self.control.len()
            || block >= self.control.len()
            || statement >= self.control[block].statements
        {
            return Err(mismatch());
        }
        if matches!(self.control[block].end, End::Unreachable) {
            return Ok(None);
        }
        let pc = self
            .blocks
            .start
            .checked_add(block)
            .ok_or(Resource::Arithmetic)?;
        if pc >= self.blocks.end {
            return Err(mismatch());
        }
        Ok(Some(pc))
    }
}

#[cfg(test)]
#[path = "original_semantic_mir_source_checked_micro_v293_tests.rs"]
mod micro_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_transition_scanner_headers_cover_nested_iteration_and_results() {
        type Loop = std::ops::Range<usize>;
        assert_eq!(
            headers(),
            2 * size_of::<Event>()
                + 2 * size_of::<Result<Event>>()
                + 2 * size_of::<Result<bool>>()
                + 2 * size_of::<Result<usize>>()
                + 4 * size_of::<Loop>()
                + 2 * size_of::<Option<usize>>()
                + size_of::<Result<Option<usize>>>()
                + size_of::<Option<usize>>()
                + 6 * size_of::<usize>()
                + 2 * size_of::<&()>()
                + 12 * size_of::<usize>()
                + 8 * size_of::<&()>()
        );
    }
}
