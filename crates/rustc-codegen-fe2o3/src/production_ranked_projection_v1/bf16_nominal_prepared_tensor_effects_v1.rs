//! Prepared source-indexed tensor effects for the closed nominal BF16 helper.
//! This composes an actual postflight candidate with the actual dense Final
//! table. It does NOT certify CFG participation, access readiness, ranked
//! coordinates, normal admission, or a retained output accumulator capability.
use super::*;

const COMPOSE_JOIN_WORK: usize = 128;
const COMPOSE_SCOPE_WORK: usize = 32;
const COMPOSE_ROW_WORK: usize = size_of::<ProjectedCapabilityTerminatorEffectsV1>() + 64;

/// Immutable prepared DATA, retaining its original source/candidate loan.
/// The TensorLayout describes an obligation for the later complete CFG
/// verifier; UniformSubgroup here is not a completed full-wave proof.
pub(in crate::production_ranked_projection_v1) struct NominalPreparedTensorEffectsV1<'a, 'g> {
    original: &'a NominalFinalRetainedEffectsV1<'a, 'g>,
    effects: &'a [ProjectedCapabilityTerminatorEffectsV1],
    source_block: SemanticBlockIdV1,
}
impl<'a, 'g> NominalPreparedTensorEffectsV1<'a, 'g> {
    pub(in crate::production_ranked_projection_v1) const fn original(
        &self,
    ) -> &'a NominalFinalRetainedEffectsV1<'a, 'g> {
        self.original
    }
    pub(in crate::production_ranked_projection_v1) const fn effects(
        &self,
    ) -> &'a [ProjectedCapabilityTerminatorEffectsV1] {
        self.effects
    }
    pub(in crate::production_ranked_projection_v1) const fn source_block(
        &self,
    ) -> SemanticBlockIdV1 {
        self.source_block
    }
}

fn compose_storage<R>(blocks: usize, callback_bytes: usize) -> Result<usize> {
    require(
        (1..=MAX_RETAINED_BLOCKS).contains(&blocks),
        "composed tensor effects leave the closed dense block profile",
    )?;
    let mut bytes = 4096usize;
    for amount in [
        times(size_of::<ProjectedCapabilityTerminatorEffectsV1>(), blocks)?,
        times(size_of::<ProjectedCapabilityTerminatorEffectsV1>(), 4)?,
        times(size_of::<Vec<ProjectedCapabilityTerminatorEffectsV1>>(), 4)?,
        times(
            size_of::<NominalPreparedTensorEffectsV1<'static, 'static>>(),
            2,
        )?,
        times(size_of::<ProductionRankedOperationV1>(), 4)?,
        times(size_of::<Checkpoint>(), 4)?,
        times(callback_bytes, 4)?,
        times(size_of::<Result<R>>(), 4)?,
    ] {
        bytes = add(bytes, amount)?;
    }
    Ok(bytes)
}

fn with_composed_scope<'w, R: Copy + 'static, F>(
    blocks: usize,
    budget: &mut Budget<'w>,
    body: F,
) -> Result<R>
where
    F: FnOnce(&mut Budget<'w>) -> Result<R>,
{
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    let reserved = compose_storage::<R>(blocks, size_of_val(&body))?;
    budget.reserve_storage(reserved)?;
    let protected = Checkpoint::take(budget);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(COMPOSE_SCOPE_WORK)?;
        body(budget)
    }));
    let result = match outcome {
        Ok(Ok(_)) if budget.failed_work().is_some() || budget.failed_storage().is_some() => {
            Err(Resource::Accounting.into())
        }
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::CallbackPanicked)
        }
    };
    // Every partial row/vector, callback capture and panic payload has dropped.
    // Do not refund callback surplus or repair a replaced/undercut ledger.
    protected.require_custody(budget)?;
    budget.release_storage(reserved)?;
    result
}

// Fixed-arity data mapping only. This helper cannot create a source-bound view.
// Production supplies only the operation borrowed from the completed candidate.
fn copy_nominal_layout(
    operation: &ProductionRankedOperationV1,
) -> Result<ProductionRankedOperationV1> {
    let ProductionRankedOperationV1::TensorLayout {
        contract,
        convergence,
        active_lanes,
        binding: Some(binding),
    } = operation
    else {
        return Err(Error::Unavailable(
            "composed nominal effect is not a bound tensor layout",
        ));
    };
    require(
        *contract
            == TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                .with_zero_filled_predicate_inputs()
            && *convergence == TensorConvergenceAttr::UniformSubgroup
            && *active_lanes == 64
            && binding.argument_count() == 4,
        "composed nominal tensor contract differs",
    )?;
    Ok(ProductionRankedOperationV1::TensorLayout {
        contract: *contract,
        convergence: *convergence,
        active_lanes: *active_lanes,
        binding: Some(*binding),
    })
}

// The caller has reserved the entire vector backing and fixed frames before
// entry. All validation and copy work is paid before allocation or mutation.
fn compose_rows(
    original: &[ProjectedCapabilityTerminatorEffectsV1],
    call_block: usize,
    operation: &ProductionRankedOperationV1,
    budget: &mut Budget<'_>,
) -> Result<Vec<ProjectedCapabilityTerminatorEffectsV1>> {
    require(
        (1..=MAX_RETAINED_BLOCKS).contains(&original.len()) && call_block < original.len(),
        "composed nominal source block or table shape differs",
    )?;
    budget.charge_work(add(64, times(original.len(), COMPOSE_ROW_WORK)?)?)?;
    require(
        original[call_block] == ProjectedCapabilityTerminatorEffectsV1::default(),
        "composed nominal source slot is not the empty actual Defined-call effect",
    )?;
    let layout = copy_nominal_layout(operation)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(original.len())
        .map_err(|_| Resource::Allocation)?;
    require_exact_capacity(rows.capacity(), original.len())?;
    for (index, original_row) in original.iter().enumerate() {
        // Exhaustive field construction in the unchanged copier preserves
        // read/layout/transpose/view rows and refuses future owned variants.
        let mut row = copy_fixed_effect(original_row)?;
        if index == call_block {
            // The full row was required empty; this never overwrites a read or
            // replaces a direct source Matrix effect with the helper proxy.
            row.layout = Some(copy_nominal_layout(&layout)?);
        }
        rows.push(row);
    }
    Ok(rows)
}

fn source_block<'a, 'g>(
    original: &NominalFinalRetainedEffectsV1<'a, 'g>,
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'g>,
    root: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    source_call: &SemanticDirectCallV1,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    budget.charge_work(COMPOSE_JOIN_WORK)?;
    let candidate = original.candidate();
    require(
        std::ptr::eq(candidate.owner(), owner)
            && std::ptr::eq(candidate.inventory(), inventory)
            && std::ptr::eq(candidate.source_call(), source_call)
            && inventory.belongs_to(owner.executable()),
        "composed nominal candidate owner/inventory/call identity differs",
    )?;
    let semantic = owner.semantic_ssa().source_semantic();
    let function = semantic
        .functions()
        .get(root.index() as usize)
        .ok_or(Error::Unavailable("composed nominal source root absent"))?;
    require(
        std::ptr::eq(function, original.function())
            && original.effects().len() == function.blocks().len(),
        "composed nominal effect table is not the actual source function",
    )?;
    let emission = owner
        .bf16_call_instance_emission_v1()
        .ok_or(Error::Unavailable(
            "composed nominal source emission absent",
        ))?;
    require(
        emission.root() == root
            && emission.source_call_block() == block
            && emission.return_permutation() == candidate.permutation(),
        "composed nominal source root/block/Return binding differs",
    )?;
    let index = block.index() as usize;
    let actual = function.blocks().get(index).ok_or(Error::Unavailable(
        "composed nominal source call block absent",
    ))?;
    let crate::production_ranked_projection_v1::SemanticTerminatorKindV1::Call(call) =
        actual.terminator().kind()
    else {
        return Err(Error::Unavailable(
            "composed nominal source terminator is not Call",
        ));
    };
    require(
        std::ptr::eq(call, source_call),
        "composed nominal source call is not the actual borrowed terminator",
    )?;
    require_completed_run(original.run(), original.run().authenticated_visits, true)?;
    Ok(index)
}

/// Prepares the complete actual dense effect table plus its one nominal tensor
/// obligation. It accepts original owners, not detached rows, counts, hashes,
/// a "Final" flag or a consumer-created capability.
///
/// The unchanged original factory finishes every source/facts/dense/N1
/// postflight before this closure. Its outer custody guard also encloses the
/// new consumer. The new table, original table and original candidate coexist
/// with separate paid storage until the consumer returns; no source replay or
/// producer is skipped and no global NominalPending routing changes.
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_prepared_tensor_effects_v1<
    'g,
    'w,
    R,
    F,
>(
    owner: &'g ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'g>,
    root: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    source_call: &SemanticDirectCallV1,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'a> FnOnce(&NominalPreparedTensorEffectsV1<'a, 'g>, &mut Budget<'w>) -> Result<R>,
{
    with_nominal_final_retained_effects_v1(
        owner,
        inventory,
        root,
        block,
        source_call,
        budget,
        move |original, budget| {
            with_composed_scope(original.effects().len(), budget, move |budget| {
                let index =
                    source_block(original, owner, inventory, root, block, source_call, budget)?;
                let effects = compose_rows(
                    original.effects(),
                    index,
                    original.candidate().operation(),
                    budget,
                )?;
                let view = NominalPreparedTensorEffectsV1 {
                    original,
                    effects: &effects,
                    source_block: block,
                };
                let result = inspect(&view, budget);
                drop(view);
                drop(effects);
                result
            })
        },
    )
}

#[cfg(test)]
#[path = "bf16_nominal_prepared_tensor_effects_v1_tests.rs"]
mod tests;
