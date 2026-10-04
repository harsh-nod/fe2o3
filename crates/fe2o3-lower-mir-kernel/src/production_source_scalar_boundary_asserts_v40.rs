// This is a temporary borrowed index over the existing immutable assertion
// archive, not a second assertion capture or an assumption that asserts succeed.
fn source_boundary_assertions_v40<'a>(
    relation: &'a ProductionSourceCorrespondenceV18<'_>,
    function: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<&'a ReplayedInstanceAssertV1>> {
    relation.query(budget)?;
    let rows = &relation.source.owner.inner.assertions;
    // collect_scoped_module_assertions_v29 appends roots in physical function
    // order, and replay authenticates each binding's function before append.
    let boundary =
        |after: bool, budget: &mut ArgumentBudgetV1<'_>| -> SourceOwnedResultV18<usize> {
            let (mut lo, mut hi) = (0, rows.len());
            while lo < hi {
                budget.charge_work(2)?;
                let at = lo + (hi - lo) / 2;
                let key = rows[at].binding.block().function.0 as usize;
                if key < function || (after && key == function) {
                    lo = at + 1;
                } else {
                    hi = at;
                }
            }
            Ok(lo)
        };
    let first = boundary(false, budget)?;
    let last = boundary(true, budget)?;
    let mut index = emission_vec_v1(last - first, budget).map_err(source_emission_error_v18)?;
    for row in &rows[first..last] {
        budget.charge_work(2)?;
        if row.binding.block().function.0 as usize != function {
            return relation
                .source
                .missing("source assertion index root differs");
        }
        index.push(row);
    }
    assert_origin_sort_v1(&mut index, budget, |a, b, budget| {
        budget.charge_work(2)?;
        Ok((a.instance.index(), a.site).cmp(&(b.instance.index(), b.site)))
    })
    .map_err(|error| source_emission_error_v18(error.into()))?;
    for pair in index.windows(2) {
        budget.charge_work(2)?;
        if (pair[0].instance.index(), pair[0].site) >= (pair[1].instance.index(), pair[1].site) {
            return relation
                .source
                .missing("source assertion index is not unique");
        }
    }
    Ok(index)
}

impl SourceBoundaryCheckV31<'_> {
    fn assertion_v40(
        &self,
        assertions: &[&ReplayedInstanceAssertV1],
        instance: usize,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        original: &SemanticTerminatorKindV1,
        input: &fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>,
        actual: &fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let relation = self.leaves.relation;
        let source = relation.source.root_row(self.leaves.root)?;
        let SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            target,
            unwind,
            ..
        } = original
        else {
            return relation
                .source
                .missing("source assertion control kind differs");
        };
        budget.charge_work(8)?;
        if *unwind != SemanticUnwindActionV1::Unreachable
            || target.role() != SemanticEdgeRoleV1::AssertSuccess
            || lower_scalar_type(
                relation.source.source_semantic(budget)?.types(),
                condition.ty(),
            )
            .map_err(source_emission_error_v18)?
                != Type::BOOL
        {
            return relation
                .source
                .missing("source assertion unwind or condition is not modeled");
        }
        let key = (
            instance,
            SemanticKirAssertSiteV1::new(source.coordinates.root, function, block),
        );
        let at = assert_origin_find_v1(assertions, budget, |row, budget| {
            budget.charge_work(2)?;
            Ok((row.instance.index(), row.site).cmp(&key))
        })
        .map_err(|error| source_emission_error_v18(error.into()))?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source assertion attachment is absent",
        ))?;
        let binding = assertions[at].binding;
        if binding.expected() != *expected
            || binding.semantic_success() != target.target()
            || binding.block() != input.coordinate
            || self.placement(input.coordinate, true, budget)? != actual.coordinate
        {
            return relation
                .source
                .missing("source assertion coordinate or polarity differs");
        }
        match (binding.outcome(), actual.terminator) {
            (
                SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge },
                Terminator::Branch { .. },
            ) => {
                if success_edge.source != input.coordinate
                    || success_edge.successor != 0
                    || actual.edges.len() != 1
                {
                    return relation
                        .source
                        .missing("source assertion elision edge differs");
                }
                // No success assumption is introduced here. The mandatory
                // original byte interpreter still evaluates the assertion and
                // requires refinement of either success or its observable trap.
            }
            (
                SemanticKirAssertConditionOutcomeV1::Emitted {
                    condition_use,
                    definition,
                    success_edge,
                    failure_edge,
                },
                Terminator::ConditionalBranch {
                    condition: actual_condition,
                    ..
                },
            ) => {
                if input.edges.len() != 2
                    || actual.edges.len() != 2
                    || condition_use
                        != (fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::TerminatorOperand {
                            block: input.coordinate,
                            operand: 0,
                        })
                    || success_edge.source != input.coordinate
                    || failure_edge.source != input.coordinate
                    || success_edge.successor != u32::from(!*expected)
                    || failure_edge.successor != u32::from(*expected)
                {
                    return relation
                        .source
                        .missing("source assertion ordered edges differ");
                }
                let uses = relation
                    .inventory
                    .uses()
                    .get(input.terminator_uses.clone())
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source assertion condition use absent",
                    ))?;
                let [condition_row] = uses else {
                    return relation
                        .source
                        .missing("source assertion condition census differs");
                };
                if condition_row.coordinate != condition_use
                    || relation.inventory.definitions()[condition_row.definition].coordinate
                        != definition
                {
                    return relation
                        .source
                        .missing("source assertion condition definition differs");
                }
                self.selector_with_role(
                    instance,
                    block,
                    condition,
                    *actual_condition,
                    EntryOperandV20::AssertCondition,
                    budget,
                )?;
                let original_failure =
                    &relation.inventory.edges()[input.edges.start + usize::from(*expected)];
                let actual_failure =
                    &self.inventory.edges()[actual.edges.start + usize::from(*expected)];
                if original_failure.coordinate != failure_edge
                    || !actual_failure.arguments.is_empty()
                    || self.placement(original_failure.target, false, budget)?
                        != actual_failure.target
                {
                    return relation
                        .source
                        .missing("source assertion failure target differs");
                }
                let function = self
                    .inventory
                    .functions()
                    .get(actual_failure.target.function.0 as usize)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source assertion failure function absent",
                    ))?;
                let failure_index = function
                    .blocks
                    .start
                    .checked_add(actual_failure.target.block as usize)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let failure = self
                    .inventory
                    .blocks()
                    .get(failure_index)
                    .filter(|row| row.coordinate == actual_failure.target)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source assertion failure block absent",
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
                        .missing("source assertion failure is not an exact terminal trap");
                }
            }
            _ => {
                return relation
                    .source
                    .missing("source assertion control transformation is not modeled");
            }
        }
        Ok(())
    }
}

fn source_boundary_assert_headers_v40() -> Result<usize, ArgumentResourceV1> {
    type Frames<'a> = (
        Vec<&'a ReplayedInstanceAssertV1>,
        SourceOwnedResultV18<Vec<&'a ReplayedInstanceAssertV1>>,
        [&'a ReplayedInstanceAssertV1; 3],
        &'a [&'a ReplayedInstanceAssertV1],
        SemanticKirAssertConditionBindingV1,
        SemanticKirAssertConditionOutcomeV1,
        SemanticKirAssertSiteV1,
        (usize, SemanticKirAssertSiteV1),
        std::slice::Iter<'a, ReplayedInstanceAssertV1>,
        std::slice::Windows<'a, &'a ReplayedInstanceAssertV1>,
        &'a fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'a>,
        [SourceOwnedResultV18<usize>; 3],
        [SourceOwnedResultV18<()>; 2],
        [usize; 12],
        [&'a (); 16],
    );
    argument_sum_v1(&[size_of::<Frames<'_>>(), std::mem::align_of::<Frames<'_>>()])
}
