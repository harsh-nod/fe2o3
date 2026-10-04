//! Private consuming continuation for the one-root nominal profile.
//! The caller owns the original persistent projection account. No ordinary
//! dispatch, receipt replay, target check or executable authority is added here.
use super::*;
use crate::production_ranked_projection_v1::{
    bf16_nominal_call_routing_v1::query_error,
    bf16_nominal_source_preparation_v1::with_nominal_rich_source_preparation_v1,
    canonical_assertion_facts_v1::{
        with_nominal_canonical_facts_observation_v1, with_nominal_prepared_control_flow_v1,
    },
};
use fe2o3_kernel_analysis::{CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1};
use fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1 as QueryError;
use fe2o3_pliron::{
    ProductionRankedAnalysisAllowanceV1 as Analysis,
    ProductionRankedSnapshotAllowanceV1 as Snapshot,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

type Parts = (
    ProductionRankedKernelLoweringInputV1,
    Vec<ProductionRankedAccessSourceV1>,
);
const SAVED_ERROR: &str = "nominal owning driver retained exact projection error";

fn frame_bytes() -> Result<usize> {
    // Original outer Pending, by-value root transfer, stream transfer, and
    // returned root/result headers. Inner consumer prepays its own move frame.
    let mut bytes = 4096usize;
    for (size, count) in [
        (size_of::<PendingActualRootPrefixIndicesV1>(), 2usize),
        (size_of::<block_stream::BlockStream>(), 2),
        (size_of::<Result<Parts>>(), 2),
        (size_of::<ProductionRankedRootProgramV1>(), 2),
        (size_of::<Result<ProductionRankedRootProgramV1>>(), 2),
        (size_of::<Option<Error>>(), 2),
        (size_of::<ReservationState>(), 2),
        (size_of::<CanonicalKirInventoryV1<'static>>(), 2),
    ] {
        bytes = bytes
            .checked_add(
                size.checked_mul(count)
                    .ok_or_else(|| resource(Resource::Arithmetic))?,
            )
            .ok_or_else(|| resource(Resource::Arithmetic))?;
    }
    Ok(bytes)
}

fn prepay_frame(budget: &mut Budget<'_>, owned: &mut usize) -> Result<()> {
    budget.check_prior_denials_v1().map_err(resource)?;
    let mut resources = PreparationResourcesV1::new(budget, owned);
    let frame = frame_bytes()?;
    resources.work(
        frame
            .checked_mul(4)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    )?;
    resources.reserve_storage(frame)
}

// Unlike ReservationState::check, this shape check also runs on failure. The
// exact original denial is propagated separately, not hidden as success.
fn account_shape(state: ReservationState, budget: &Budget<'_>, owned: usize) -> Result<()> {
    let growth = owned
        .checked_sub(state.owned)
        .ok_or_else(|| resource(Resource::Accounting))?;
    let floor = state
        .storage
        .checked_add(growth)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    if state.slot != budget as *const Budget<'_> as usize
        || state.ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
        || budget.work() < state.work
        || budget.peak_storage() < state.peak
    {
        return Err(resource(Resource::Accounting));
    }
    Ok(())
}

fn inventory_error(error: CanonicalKirInventoryErrorV1) -> Error {
    match error {
        CanonicalKirInventoryErrorV1::Resource(error) => resource(error),
        other => Error::CanonicalAssertions(CanonicalAssertionErrorV1::Inventory(other)),
    }
}

fn restore_query_error(
    result: std::result::Result<(), QueryError>,
    saved: Option<Error>,
) -> Result<()> {
    match (result, saved) {
        (Err(QueryError::Unavailable(SAVED_ERROR)), Some(error)) => Err(error),
        (Err(error), _) => Err(query_error(error)),
        (Ok(()), None) => Ok(()),
        _ => Err(resource(Resource::Accounting)),
    }
}

/// Only the owning projection entry calls this shipping continuation. The input
/// factory lends the caller's actual owner/roster/bindings; the SAME caller's Box
/// ledger remains alive when the actual returned root is installed in Program.
pub(in crate::production_ranked_projection_v1) fn consume_actual_nominal_root_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    actual: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
    owned: &mut usize,
) -> Result<ProductionRankedRootProgramV1> {
    let state = ReservationState::new(budget, *owned)?;
    prepay_frame(budget, owned)?;
    if !actual.belongs_to(owner)
        || actual.inputs().len() != 1
        || !actual.bindings().as_slice().is_empty()
        || owner.helper_source_policy_v1()
            != fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::Bf16Nominal
    {
        return Err(Error::Incomplete(
            "private nominal owning profile requires actual one-root/no-reference inputs",
        ));
    }
    let emission = owner
        .bf16_call_instance_emission_v1()
        .ok_or(Error::Incomplete(
            "private nominal owning source relation absent",
        ))?;
    let root = emission.root();
    let call_block = emission.source_call_block();
    let semantic = owner.semantic_ssa().source_semantic();
    let function = semantic
        .functions()
        .get(root.index() as usize)
        .ok_or(Error::Incomplete("private nominal owning root absent"))?;
    let call = match function
        .blocks()
        .get(call_block.index() as usize)
        .map(|block| block.terminator().kind())
    {
        Some(SemanticTerminatorKindV1::Call(call)) => call,
        _ => {
            return Err(Error::Incomplete(
                "private nominal owning source call absent",
            ));
        }
    };

    // Derive prepays every inventory allocation and normally restores its own
    // floor. Catch only its construction here: no Pending callback can yet have
    // allocated anything, so panic cleanup cannot refund callback-owned surplus.
    let construction_floor = budget.storage();
    let derived = catch_unwind(AssertUnwindSafe(|| {
        CanonicalKirInventoryV1::derive(owner.executable(), budget)
    }));
    account_shape(state, budget, *owned)?;
    let (inventory, receipt) = match derived {
        Ok(result) => {
            if budget.storage() != construction_floor {
                drop(result);
                return Err(resource(Resource::Accounting));
            }
            result.map_err(inventory_error)?
        }
        Err(payload) => {
            drop(payload);
            let construction_only = budget
                .storage()
                .checked_sub(construction_floor)
                .ok_or_else(|| resource(Resource::Accounting))?;
            budget
                .release_storage(construction_only)
                .map_err(resource)?;
            return Err(query_error(QueryError::CallbackPanicked));
        }
    };
    // Transfer the real, already-prepaid construction receipt immediately; no
    // next ledger-controlled allocation occurs in the gap.
    let inventory_storage = receipt.retained_storage();
    budget
        .reserve_storage(inventory_storage)
        .map_err(resource)?;

    // These actual payloads outlive all nested source/facts/report postflights.
    // Unit is the only value crossing Copy-only query callbacks.
    let mut pending = PendingActualRootPrefixIndicesV1::new();
    let mut saved = None;
    let queried = catch_unwind(AssertUnwindSafe(|| {
        owner.with_bf16_nominal_entry_resources_v1(&inventory, budget, |budget| {
            with_nominal_prepared_control_flow_v1(
                owner,
                &inventory,
                root,
                call_block,
                call,
                budget,
                |flow, budget| {
                    owner.with_checked_bf16_nominal_call_v1(
                        &inventory,
                        root,
                        root,
                        call_block,
                        call,
                        budget,
                        |checked, budget| {
                            with_nominal_rich_source_preparation_v1(
                                owner,
                                &inventory,
                                root,
                                root,
                                call_block,
                                call,
                                budget,
                                |rich, budget| {
                                    with_nominal_canonical_facts_observation_v1(
                                        owner,
                                        &inventory,
                                        root,
                                        root,
                                        call_block,
                                        call,
                                        budget,
                                        |facts| {
                                            let result = with_nominal_recipe_resources_v1(
                                                facts,
                                                rich,
                                                owned,
                                                |context| {
                                                    context.with_actual_root_block_stream_v1(
                                                        checked,
                                                        rich,
                                                        actual,
                                                        flow,
                                                        &mut pending,
                                                        |view, context| {
                                                            view.verify_ranked(
                                                                context,
                                                                Analysis::production_hard_ceiling(),
                                                                Snapshot::production_hard_ceiling(),
                                                            )?;
                                                            Ok(())
                                                        },
                                                    )
                                                },
                                            );
                                            match result {
                                                Ok(()) => Ok(()),
                                                Err(error) => {
                                                    saved = Some(error);
                                                    Err(QueryError::Unavailable(SAVED_ERROR))
                                                }
                                            }
                                        },
                                    )
                                },
                            )
                        },
                    )
                },
            )
        })
    }));
    let queried = match queried {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(QueryError::CallbackPanicked)
        }
    };
    // Every borrowed report/flow/rich table has now dropped. Shape checks do not
    // release storage and take precedence over saved callback errors.
    let shape = account_shape(state, budget, *owned);
    drop(inventory);
    // This is the ONLY refund after callbacks begin. Pending credits remain
    // accepted even when the query failed or panicked.
    shape?;
    budget
        .release_storage(inventory_storage)
        .map_err(resource)?;
    account_shape(state, budget, *owned)?;
    restore_query_error(queried, saved)?;
    budget.check_prior_denials_v1().map_err(resource)?;
    let (lowering, access_sources) = {
        let mut resources = PreparationResourcesV1::new(budget, owned);
        pending.into_verified_parts(&mut resources)?
    };
    account_shape(state, budget, *owned)?;
    budget.check_prior_denials_v1().map_err(resource)?;

    // The original complete input selection ran inside the real source loan.
    // One actual root plus an actually empty binding roster (checked above)
    // makes an empty reference receipt roster truthful, not reconstructed proof.
    let input = &actual.inputs()[0];
    let entry = function.kernel_entry().ok_or(Error::Incomplete(
        "private nominal owning kernel entry absent",
    ))?;
    let export = entry.export_symbol().as_bytes();
    {
        let mut resources = PreparationResourcesV1::new(budget, owned);
        let bytes = input
            .logical_name
            .len()
            .checked_add(export.len())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        resources.work(bytes)?;
        resources.reserve_storage(bytes)?;
    }
    // Preserve the ordinary pipeline's existing bounded induction analysis and
    // ranked-text formatting scopes. Neither is claimed to be newly globally
    // metered by the selected canonical/source-proof account above.
    let semantic_u32_induction =
        fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(semantic, root)
            .map_err(Error::SemanticU32Induction)?;
    let verification =
        crate::production_reference_effect_join_v2::conditional::ReferenceRootV1::Ordinary {
            lowering,
            receipts: Vec::new(),
        };
    let ranked_ir = format_ranked_cfg(function_name(function)?, verification.kernel().blocks())?;
    let root = ProductionRankedRootProgramV1 {
        logical_name: input.logical_name.clone(),
        export_symbol: export.to_vec().into_boxed_slice(),
        semantic_root: root,
        semantic_root_identity: function.identity(),
        kernel_binding: input.kernel_binding,
        source_rank: input.source_launch.rank(),
        semantic_u32_induction,
        verification,
        ranked_ir,
        access_sources,
        // Closed emission rejects all Pipeline/GeneratedFromSemanticTerminator
        // rows. Actual AllocationEffect/access mappings remain in access_sources.
        executable_effect_sources: Vec::new(),
        #[cfg(test)]
        observed_reference_writes: Vec::new(),
    };
    account_shape(state, budget, *owned)?;
    budget.check_prior_denials_v1().map_err(resource)?;
    Ok(root)
}

#[cfg(test)]
#[path = "bf16_nominal_owned_root_driver_v1_tests.rs"]
mod tests;
