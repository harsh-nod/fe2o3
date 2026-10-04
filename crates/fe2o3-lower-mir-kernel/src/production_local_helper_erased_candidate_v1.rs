// A deterministic producer, never the semantic checker. Its masks are inert.

fn produce_unit_erased_candidate_v1(
    original: &ProductionPreRankedKirOwnerV1,
    roots: &[ProductionRankedSemanticProjectionRootV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    (
        Module,
        fe2o3_kernel_ir::CanonicalKernelIrCandidateStorageV12,
    ),
    ProductionPreRankedKirErrorV1,
> {
    with_unit_erased_owner_scratch_v1(budget, |budget| {
        budget.charge_work(3)?;
        let (mut candidate, storage) = original
            .executable()
            .copy_module_for_transformation_v12(budget)
            .map_err(ProductionPreRankedKirErrorV1::Canonical)?;
        budget.reserve_storage(storage.retained_storage())?;
        budget.reserve_storage(argument_product_v1(2, std::mem::size_of::<Vec<bool>>())?)?;
        let counts = unit_erased_counts_v1(original.executable().module(), budget)?;
        let mut functions = unit_local_vec_v1(counts.0, budget)?;
        let mut operations = unit_local_vec_v1(counts.1, budget)?;
        budget.charge_work(argument_sum_v1(&[counts.0, counts.1])?)?;
        functions.resize(counts.0, false);
        operations.resize(counts.1, false);
        original.with_checked_unit_local_ranked_stage_v1(roots, budget, |stage, budget| {
            for body in &stage.source.rows.bodies {
                budget.charge_work(3)?;
                let mark = functions
                    .get_mut(body.physical)
                    .ok_or_else(unit_local_mismatch_v1)?;
                if std::mem::replace(mark, true) {
                    return Err(unit_local_mismatch_v1());
                }
            }
            for call in stage.calls {
                budget.charge_work(3)?;
                let ordinal = unit_deletion_operation_index_v1(
                    stage.source.inventory,
                    call.native_call(),
                    budget,
                )?
                .ok_or_else(unit_local_mismatch_v1)?;
                if !std::ptr::eq(
                    stage.source.inventory.operations()[ordinal].operation,
                    call.operation(),
                ) || std::mem::replace(&mut operations[ordinal], true)
                {
                    return Err(unit_local_mismatch_v1());
                }
            }
            // Prepay the full no-allocation filtering traversal, including
            // operations in removed helpers. Keep the copy receipt conservative
            // while the candidate's original vector capacities remain alive.
            budget.charge_work(argument_sum_v1(&[
                counts.0,
                counts.1,
                3,
                original.executable().canonical().canonical_bytes().len(),
            ])?)?;
            let mut function = 0usize;
            let mut operation = 0usize;
            for body in candidate
                .functions
                .iter_mut()
                .filter_map(|function| function.body.as_mut())
            {
                budget.charge_work(body.blocks.len())?;
                for block in &mut body.blocks {
                    block.operations.retain(|_| {
                        let retain = !operations[operation];
                        operation += 1;
                        retain
                    });
                }
            }
            if operation != operations.len() {
                return Err(unit_local_mismatch_v1());
            }
            candidate.functions.retain(|_| {
                let retain = !functions[function];
                function += 1;
                retain
            });
            if function != functions.len() {
                return Err(unit_local_mismatch_v1());
            }
            Ok(())
        })?;
        Ok((candidate, storage))
    })
}
