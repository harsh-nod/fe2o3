// Abort has no source successor. Its actual edge must end at the exact retained
// terminal trap, including after checked optimized block placement.
impl SourceBoundaryCheckV31<'_> {
    fn abort_v50(
        &self,
        input: &fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>,
        actual: &fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let relation = self.leaves.relation;
        budget.charge_work(10)?;
        if !matches!(input.terminator, Terminator::Branch { arguments, .. } if arguments.is_empty())
            || !matches!(actual.terminator, Terminator::Branch { arguments, .. } if arguments.is_empty())
            || input.edges.len() != 1
            || actual.edges.len() != 1
            || self.placement(input.coordinate, true, budget)? != actual.coordinate
        {
            return relation
                .source
                .missing("source Abort exit is not its exact failure edge");
        }
        let original_edge = relation.inventory.edges().get(input.edges.start).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source Abort original edge absent"),
        )?;
        let actual_edge = self.inventory.edges().get(actual.edges.start).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source Abort actual edge absent"),
        )?;
        if original_edge.coordinate.source != input.coordinate
            || original_edge.coordinate.successor != 0
            || actual_edge.coordinate.source != actual.coordinate
            || actual_edge.coordinate.successor != 0
            || !original_edge.arguments.is_empty()
            || !actual_edge.arguments.is_empty()
            || self.placement(original_edge.target, false, budget)? != actual_edge.target
        {
            return relation
                .source
                .missing("source Abort failure edge placement differs");
        }
        let function = self
            .inventory
            .functions()
            .get(actual_edge.target.function.0 as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source Abort failure function absent",
            ))?;
        let index = function
            .blocks
            .start
            .checked_add(actual_edge.target.block as usize)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let failure = self
            .inventory
            .blocks()
            .get(index)
            .filter(|row| row.coordinate == actual_edge.target)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source Abort failure block absent",
            ))?;
        if !failure.parameters.is_empty()
            || failure.operations.len() != 1
            || !matches!(failure.terminator, Terminator::Unreachable)
            || !terminal_failure_is_trap_v18(
                self.inventory.operations()[failure.operations.start].operation,
                budget,
            )
            .map_err(source_emission_error_v18)?
        {
            return relation
                .source
                .missing("source Abort target is not an exact terminal trap");
        }
        Ok(())
    }
}

fn source_boundary_abort_headers_v50() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        [&'a fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'a>; 3],
        [&'a fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'a>; 2],
        &'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>,
        [SourceOwnedResultV18<()>; 3],
        [SourceOwnedResultV18<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>; 2],
        [usize; 4],
    );
    argument_sum_v1(&[size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>()])
}
