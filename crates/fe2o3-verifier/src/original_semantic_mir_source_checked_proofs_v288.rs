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
                            if function
                                .body
                                .event_at(block, statement, out)?
                                .emit_checked_local_add_proofs_v288(
                                    root, instance, block, statement, out,
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
                + 12 * size_of::<usize>()
                + 8 * size_of::<&()>()
        );
    }
}
