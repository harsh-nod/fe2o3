//! Original SSA argument forwarding, not erased-value or transition admission.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1 as EdgeArgument;
use fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1 as Function;

pub(super) fn headers() -> usize {
    // The caller's original endpoint remains live throughout this query.
    12 * size_of::<&()>()
        + 5 * size_of::<usize>()
        + size_of::<Option<usize>>()
        + size_of::<Function>()
        + size_of::<Definition>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<std::slice::Iter<'_, EdgeArgument>>()
        + 2 * size_of::<Result<usize>>()
}

impl ExpandedScalarBindingsV196<'_, '_, '_, '_> {
    /// Locate only exact, same-typed incoming SSA forwarding. Every source cut
    /// still requires its original value to relate to this actual target value.
    /// Computations, absent edges, ambiguous predecessors and cycles refuse.
    pub(in super::super::super) fn source_transport_definition(
        &self,
        original: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            let input = self.slots.correspondence(out)?.inventory(out.budget)?;
            let tile = self.slots.tile_owner_v176(out)?;
            let neutral = tile.neutral_source_v162(out.budget)?;
            out.budget.charge_work(2)?;
            let source = input.definitions().get(original).ok_or_else(mismatch)?;
            let mut current = original;
            // With one checked predecessor per step, revisiting a definition
            // cannot discover a new terminal. This bound needs no visited heap.
            for _ in 0..input.definitions().len() {
                out.budget.charge_work(2)?;
                let row = input.definitions().get(current).ok_or_else(mismatch)?;
                let descendants = neutral.definition_descendants(row.coordinate, out.budget)?;
                out.budget.charge_work(1)?;
                if !descendants.is_empty() {
                    return self.source_definition(current, out);
                }
                out.budget.charge_work(2)?;
                if !matches!(source.ty, Type::Scalar(_)) {
                    return Err(mismatch());
                }
                let Definition::BlockArgument { block, .. } = row.coordinate else {
                    return Err(mismatch());
                };
                out.budget.charge_work(4)?;
                let function = input
                    .functions()
                    .get(block.function.0 as usize)
                    .filter(|function| {
                        function.coordinate == block.function
                            && function.definitions.contains(&current)
                    })
                    .ok_or_else(mismatch)?;
                let edges = input
                    .edge_arguments()
                    .get(function.edge_arguments.clone())
                    .ok_or_else(mismatch)?;
                let mut incoming = None;
                for edge in edges {
                    out.budget.charge_work(1)?;
                    if edge.target_definition != current {
                        continue;
                    }
                    out.budget.charge_work(5)?;
                    if edge.coordinate.edge.source.function != block.function
                        || !function.definitions.contains(&edge.incoming_definition)
                        || incoming.is_some_and(|prior| prior != edge.incoming_definition)
                    {
                        return Err(mismatch());
                    }
                    let predecessor = input
                        .definitions()
                        .get(edge.incoming_definition)
                        .ok_or_else(mismatch)?;
                    out.budget.charge_work(2)?;
                    if predecessor.value != Some(edge.value)
                        || compare_type(source.ty, predecessor.ty, out)? != Ordering::Equal
                    {
                        return Err(mismatch());
                    }
                    incoming = Some(edge.incoming_definition);
                }
                out.budget.charge_work(1)?;
                current = incoming.ok_or_else(mismatch)?;
            }
            Err(mismatch())
        })
    }
}
