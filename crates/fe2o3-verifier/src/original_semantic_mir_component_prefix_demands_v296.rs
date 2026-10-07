//! Reuse the retained component fixed point at a before-terminator statement cut.
use super::*;

impl ComponentDemandsV42<'_, '_, '_> {
    pub(in super::super) fn prefix_bits_v296(
        &self,
        block: usize,
        statement: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Vec<u64>> {
        self.check_owner_v281(self.slots, self.function, out)?;
        let source = self.slots.correspondence(out)?.source(out.budget)?;
        let function = source
            .source_semantic(out.budget)?
            .functions()
            .get(self.function.index() as usize)
            .ok_or_else(mismatch)?;
        let body = function.blocks().get(block).ok_or_else(mismatch)?;
        if statement > body.statements().len()
            || !source
                .source_ssa(out.budget)?
                .plan_for_function(self.function)
                .ok_or_else(mismatch)?
                .plan()
                .is_reachable(SsaBlockIdV1::new(
                    u32::try_from(block).map_err(|_| Resource::Arithmetic)?,
                ))
        {
            return Err(mismatch());
        }
        out.budget.reserve_storage(size_of::<Vec<u64>>())?;
        let mut result = zeros::<u64>(self.words, out)?;
        let retained = out.budget.storage();
        out.budget.reserve_storage(prefix_headers())?;
        let mut generated = zeros::<u64>(self.words, out)?;
        let mut killed = zeros::<u64>(self.words, out)?;
        let mut failure_killed = zeros::<u64>(self.words, out)?;
        let mut facts = Facts {
            slots: self.slots,
            function,
            locals: &self.locals,
            generated: &mut generated,
            killed: &mut killed,
        };
        for row in &body.statements()[statement..] {
            out.budget.charge_work(1)?;
            facts.statement(row.kind(), out)?;
        }
        // k == N leaves the terminator entirely unconsumed, including its moves.
        facts.terminator(body.terminator().kind(), &mut failure_killed, out)?;
        let mut edge = |edge: fe2o3_mir_model::semantic_mir_v1::SemanticControlFlowEdgeV1,
                        out: &mut Writer<'_, '_>| {
            out.budget.charge_work(3)?;
            let target = edge.target().index() as usize;
            if target >= self.blocks {
                return Err(mismatch());
            }
            let overwritten = match body.terminator().kind() {
                Terminator::Call(call) if edge.role() == EdgeRole::CallReturn => {
                    facts.range(call.destination().ok_or_else(mismatch)?.place(), false, out)?
                }
                _ => 0..0,
            };
            for (word, value) in result.iter_mut().enumerate() {
                out.budget.charge_work(3)?;
                *value |=
                    self.live[product(target, self.words)? + word] & !word_mask(&overwritten, word);
            }
            Ok::<_, Error>(())
        };
        out.budget.reserve_storage(
            2 * std::mem::size_of_val(&edge)
                + std::mem::align_of_val(&edge)
                + 2 * size_of::<&mut Writer<'_, '_>>(),
        )?;
        body.terminator()
            .kind()
            .try_for_each_edge(|row| edge(row, out))?;
        drop(edge);
        for word in 0..self.words {
            out.budget.charge_work(3)?;
            result[word] = generated[word] | (result[word] & !killed[word]);
        }
        drop((generated, killed, failure_killed));
        let released = out
            .budget
            .storage()
            .checked_sub(retained)
            .ok_or(Resource::Accounting)?;
        out.budget.release_storage(released)?;
        Ok(result)
    }

    pub(in super::super) fn prefix_leaf_v296(
        &self,
        bits: &[u64],
        local: usize,
        leaf: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        self.check_owner_v281(self.slots, self.function, out)?;
        out.budget.charge_work(3)?;
        let row = self.locals.get(local).ok_or_else(mismatch)?;
        if bits.len() != self.words || leaf >= row.range.len() {
            return Err(mismatch());
        }
        let bit = add(row.range.start, leaf)?;
        Ok(bits[bit / 64] & (1u64 << (bit % 64)) != 0)
    }
}

fn prefix_headers() -> usize {
    4 * size_of::<Vec<u64>>()
        + size_of::<Facts<'_, '_, '_, '_>>()
        + 2 * size_of::<Range<usize>>()
        + 12 * size_of::<usize>()
        + 12 * size_of::<&()>()
        + 3 * size_of::<Result<()>>()
        + size_of::<Result<Vec<u64>>>()
        + size_of::<std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1>>()
        + size_of::<std::iter::Enumerate<std::slice::IterMut<'_, u64>>>()
}

#[cfg(test)]
#[test]
fn component_prefix_headers_cover_suffix_facts_and_live_union() {
    assert_eq!(
        prefix_headers(),
        4 * size_of::<Vec<u64>>()
            + size_of::<Facts<'_, '_, '_, '_>>()
            + 2 * size_of::<Range<usize>>()
            + 12 * size_of::<usize>()
            + 12 * size_of::<&()>()
            + 3 * size_of::<Result<()>>()
            + size_of::<Result<Vec<u64>>>()
            + size_of::<std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1>>(
            )
            + size_of::<std::iter::Enumerate<std::slice::IterMut<'_, u64>>>()
    );
}
