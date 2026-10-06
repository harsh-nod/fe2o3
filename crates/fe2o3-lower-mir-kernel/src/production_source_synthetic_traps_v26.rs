//! Authenticate compiler-generated assertion failure sinks from retained
//! instance spans, including sinks that have no explicit terminal-failure row.

use super::*;

#[cfg(test)]
std::thread_local! {
    static SYNTHETIC_TRAP_OBSERVER_V26:
        std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
impl ProductionMixedMemoryCheckedNativePoliciesV26<'_, '_> {
    pub(crate) fn synthetic_trap_test_mode_v26(
        mode: Option<(usize, usize)>,
    ) -> Option<(usize, usize)> {
        SYNTHETIC_TRAP_OBSERVER_V26.with(|state| state.replace(mode))
    }
}

pub(super) fn complete(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    completed: &mut [bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    original.source.check_query_v18(budget)?;
    let output = optimized.output_inventory(budget)?;
    budget.charge_work(1)?;
    if completed.len() != output.operations().len() {
        return original
            .source
            .missing("mixed synthetic trap output census differs");
    }
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let allocated = (|| -> SourceOwnedResultV18<Vec<bool>> {
        budget.reserve_storage(std::mem::size_of::<Vec<bool>>())?;
        let mut seen = resources::vector(output.operations().len(), budget)?;
        budget.charge_work(output.operations().len())?;
        seen.resize(output.operations().len(), false);
        Ok(seen)
    })();
    let retained = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let result = match allocated {
        Err(error) => Err(error),
        Ok(mut seen) => {
            let result = visit(original, optimized, output, completed, &mut seen, budget);
            drop(seen);
            result
        }
    };
    let cleanup = if ledger != budget.work_ledger_identity_v1()
        || floor.checked_add(retained) != Some(budget.storage())
    {
        Err(ArgumentResourceV1::Accounting)
    } else {
        budget.release_storage(retained)
    };
    result?;
    cleanup?;
    Ok(())
}

fn visit(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    output: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    completed: &mut [bool],
    seen: &mut [bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    for root in 0..original.source.root_count(budget)? {
        let root_row = original.source.root_row(root)?;
        for (row, span) in root_row.coordinates.spans.rows.iter().enumerate() {
            budget.charge_work(3)?;
            let InstanceSpanSourceV1::Synthetic(source) = span.source else {
                continue;
            };
            if source.rule != SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap {
                continue;
            }
            let instance = span.instance.index();
            #[cfg(test)]
            let fault = SYNTHETIC_TRAP_OBSERVER_V26.with(|state| state.get().map_or(0, |x| x.0));
            #[cfg(test)]
            if fault == 1 {
                continue;
            }
            #[cfg(test)]
            let source = if fault == 2 {
                let mut source = source;
                source.semantic_function = SemanticFunctionIdV1::from_index(u32::MAX);
                source
            } else {
                source
            };
            if source.correspondence_owner != root_row.coordinates.root
                || source.operation_count != 1
                || !original.source.instance_active(root, instance, budget)?
                || original.source.instance(root, instance, budget)?.0 != source.semantic_function
            {
                return original
                    .source
                    .missing("mixed synthetic trap source instance or span differs");
            }
            let key = TileAttachmentKeyV29 {
                root,
                family: TileAttachmentFamilyV29::InstanceSpans,
                instance,
                row,
                field: TileAttachmentFieldV29::Span,
                component: 0,
                part: 0,
            };
            let [attachment] = original.attachment_range(key, budget)? else {
                return original
                    .source
                    .missing("mixed synthetic trap singleton attachment differs");
            };
            let ProductionSourceOperationV18::Operation(input) =
                original.mapped_source_operation(attachment.location, budget)?
            else {
                return original
                    .source
                    .missing("mixed synthetic trap has no original operation");
            };
            let before = source_operation_row_v18(original.inventory, input, budget)?;
            let block = source_block_row_v18(original.inventory, input.block, budget)?;
            budget.charge_work(4)?;
            if input.block.function.0 as usize != root_row.function_ordinal
                || !before
                    .operation
                    .has_registered_trap_contract_with_budget_v26(budget)?
                || input.operation as usize + 1 != block.block.operations.len()
                || !matches!(block.block.terminator, Some(Terminator::Unreachable))
            {
                return original
                    .source
                    .missing("mixed synthetic original trap or terminal differs");
            }
            let at = match optimized.operation(input, budget)? {
                ProductionOptimizedSourceOperationV18::Retained {
                    input: exact,
                    output,
                } if exact == input => output,
                ProductionOptimizedSourceOperationV18::RemovedUnreachable { input: exact }
                    if exact == input =>
                {
                    continue;
                }
                _ => {
                    return original
                        .source
                        .missing("mixed synthetic trap occurrence was rewritten");
                }
            };
            let after = source_operation_row_v18(output, at, budget)?;
            let block = source_block_row_v18(output, at.block, budget)?;
            budget.charge_work(4)?;
            if !after
                .operation
                .has_registered_trap_contract_with_budget_v26(budget)?
                || at.operation as usize + 1 != block.block.operations.len()
                || !matches!(block.block.terminator, Some(Terminator::Unreachable))
            {
                return original
                    .source
                    .missing("mixed synthetic retained trap or terminal differs");
            }
            let index = resources::operation_index(output, at, budget)?;
            #[cfg(test)]
            if fault == 3 {
                seen[index] = true;
            }
            if seen[index] {
                return original
                    .source
                    .missing("mixed synthetic trap has duplicate source spans");
            }
            seen[index] = true;
            // Explicit source assertions and the synthetic shared sink may name
            // one operation. Both origins are checked; the final call is counted once.
            completed[index] = true;
            #[cfg(test)]
            SYNTHETIC_TRAP_OBSERVER_V26.with(|state| {
                if let Some((fault, count)) = state.get() {
                    state.set(Some((fault, count + 1)));
                }
            });
        }
    }
    Ok(())
}
