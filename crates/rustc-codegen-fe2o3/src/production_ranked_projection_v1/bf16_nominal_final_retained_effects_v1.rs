//! Original-meter retention of the complete actual dense Final table.
//! No recipe, bounds/access certificate, ranked position or normal admission.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_dense_v1::{
    NominalCapabilityConsumerV1, NominalCapabilityLedgerV1,
};
use crate::production_ranked_projection_v1::{
    ProjectedCapabilityTerminatorEffectsV1, SemanticFunctionDeclV1,
};

const MAX_RETAINED_BLOCKS: usize = 32; // same closed bound as the dense driver
const RETAIN_PREPARE_WORK: usize = 32;
const RETAIN_INITIALIZE_WORK: usize = 32;
const RETAIN_ROW_WORK: usize = size_of::<ProjectedCapabilityTerminatorEffectsV1>() + 64;
const RETAIN_CAPTURE_WORK: usize = 32;
const RETAIN_REJOIN_WORK: usize = 64;

/// Immutable lexical observation after ALL original source/dense postflights.
/// Block indices belong to function()'s semantic source, NOT ranked/KIR blocks.
/// There is no owned/Clone/Copy receipt or detached-table input constructor.
pub(in crate::production_ranked_projection_v1) struct NominalFinalRetainedEffectsV1<'scope, 'graph>
{
    candidate: &'scope NominalFinalRankedCandidateV1<'scope, 'graph>,
    function: &'scope SemanticFunctionDeclV1,
    effects: &'scope [ProjectedCapabilityTerminatorEffectsV1],
    run: NominalCapabilityRunV1,
}
impl<'scope, 'graph> NominalFinalRetainedEffectsV1<'scope, 'graph> {
    pub(in crate::production_ranked_projection_v1) const fn candidate(
        &self,
    ) -> &'scope NominalFinalRankedCandidateV1<'scope, 'graph> {
        self.candidate
    }
    pub(in crate::production_ranked_projection_v1) const fn function(
        &self,
    ) -> &'scope SemanticFunctionDeclV1 {
        self.function
    }
    pub(in crate::production_ranked_projection_v1) const fn effects(
        &self,
    ) -> &'scope [ProjectedCapabilityTerminatorEffectsV1] {
        self.effects
    }
    pub(in crate::production_ranked_projection_v1) const fn run(&self) -> NominalCapabilityRunV1 {
        self.run
    }
}

// Private payload, never caller-supplied and never exposed before postflight.
struct RetainedEffects {
    rows: Vec<ProjectedCapabilityTerminatorEffectsV1>,
    completed: Option<NominalCapabilityRunV1>,
}
fn retained_scope_storage<R>(blocks: usize, callback_bytes: usize) -> Result<usize> {
    require(
        (1..=MAX_RETAINED_BLOCKS).contains(&blocks),
        "retained Final effects leave the closed dense block profile",
    )?;
    let mut bytes = scope_storage::<R>(callback_bytes)?;
    for amount in [
        times(size_of::<ProjectedCapabilityTerminatorEffectsV1>(), blocks)?,
        times(size_of::<ProjectedCapabilityTerminatorEffectsV1>(), 4)?,
        times(size_of::<RetainedEffects>(), 4)?,
        times(
            size_of::<NominalFinalRetainedEffectsV1<'static, 'static>>(),
            2,
        )?,
        times(size_of::<NominalCapabilityLedgerV1>(), 4)?,
        4096, // bounded copy/rejoin/traversal and allocation-error frames
    ] {
        bytes = add(bytes, amount)?;
    }
    Ok(bytes)
}
fn with_retained_scope<'w, R: Copy + 'static, F>(
    budget: &mut Budget<'w>,
    blocks: usize,
    body: F,
) -> Result<R>
where
    F: FnOnce(&mut Budget<'w>, Checkpoint) -> Result<R>,
{
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    let reserved = retained_scope_storage::<R>(blocks, size_of_val(&body))?;
    budget.reserve_storage(reserved)?;
    let protected = Checkpoint::take(budget);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(SCOPE_WORK)?;
        body(budget, protected)
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
    // The body owns every partial/completed table and callback capture. They
    // have dropped (also on unwind) before only this reservation is refunded.
    protected.require_custody(budget)?;
    budget.release_storage(reserved)?;
    result
}
fn require_exact_capacity(actual: usize, requested: usize) -> Result<()> {
    if actual != requested {
        return Err(Resource::Allocation.into());
    }
    Ok(())
}
fn require_consumer_custody(
    protected: Checkpoint,
    consumer: &dyn NominalCapabilityConsumerV1,
) -> Result<()> {
    let current = consumer.ledger_v1();
    if current.slot != protected.slot
        || current.identity != protected.ledger
        || current.storage < protected.storage
        || current.work < protected.work
        || current.peak < protected.peak
        || current.denied_work
        || current.denied_storage
    {
        return Err(Resource::Accounting.into());
    }
    Ok(())
}

// The actual shared transfer currently emits None/TensorLayout only. Refuse
// any future broader variant instead of cloning a possibly heap-owned enum.
fn copy_fixed_effect(
    source: &ProjectedCapabilityTerminatorEffectsV1,
) -> Result<ProjectedCapabilityTerminatorEffectsV1> {
    let layout = match &source.layout {
        None => None,
        Some(ProductionRankedOperationV1::TensorLayout {
            contract,
            convergence,
            active_lanes,
            binding,
        }) => Some(ProductionRankedOperationV1::TensorLayout {
            contract: *contract,
            convergence: *convergence,
            active_lanes: *active_lanes,
            binding: *binding,
        }),
        Some(_) => {
            return Err(Error::Unavailable(
                "retained Final layout leaves fixed tensor form",
            ));
        }
    };
    Ok(ProjectedCapabilityTerminatorEffectsV1 {
        layout,
        global_read: source.global_read,
        transpose_workgroup: source.transpose_workgroup,
        read_view: source.read_view,
    })
}
impl RetainedEffects {
    // Called only inside with_retained_scope, after the whole table was paid.
    fn new(blocks: usize, budget: &mut Budget<'_>) -> Result<Self> {
        budget.charge_work(add(
            RETAIN_INITIALIZE_WORK,
            times(size_of::<ProjectedCapabilityTerminatorEffectsV1>(), blocks)?,
        )?)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(blocks)
            .map_err(|_| Resource::Allocation)?;
        require_exact_capacity(rows.capacity(), blocks)?;
        rows.resize_with(blocks, ProjectedCapabilityTerminatorEffectsV1::default);
        Ok(Self {
            rows,
            completed: None,
        })
    }
    fn capture(
        &mut self,
        source: &[ProjectedCapabilityTerminatorEffectsV1],
        run: NominalCapabilityRunV1,
        consumer: &mut dyn NominalCapabilityConsumerV1,
        protected: Checkpoint,
    ) -> Result<()> {
        require_consumer_custody(protected, consumer)?;
        consumer.charge_work_v1(RETAIN_CAPTURE_WORK)?;
        require(
            self.completed.is_none()
                && source.len() == self.rows.len()
                && self.rows.capacity() == self.rows.len(),
            "retained Final capture is repeated or has a different actual block shape",
        )?;
        require_completed_run(run, run.authenticated_visits, true)?;
        for (destination, actual) in self.rows.iter_mut().zip(source) {
            consumer.charge_work_v1(RETAIN_ROW_WORK)?;
            *destination = copy_fixed_effect(actual)?;
        }
        require_consumer_custody(protected, consumer)?;
        // This only records private capture completion, not source readiness.
        self.completed = Some(run);
        Ok(())
    }
}

/// Distinct source-owned entry; the old tensor-only candidate API is unchanged.
/// Retains ALL actual Final rows internally. No callback sees a provisional
/// table, and no fourth pass, detached table, alternate meter or recipe is used.
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_final_retained_effects_v1<
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
    F: for<'scope> FnOnce(&NominalFinalRetainedEffectsV1<'scope, 'g>, &mut Budget<'w>) -> Result<R>,
{
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, move |budget| {
        // The new entry scope owns the entire generic F/Result<R> envelope while
        // the original N1 runs. Its floor check uses the storage captured BEFORE
        // this envelope; neither it nor the candidate frame can mask F-1.
        // This existing N1 still finishes before any pending candidate is exposed.
        owner.with_checked_bf16_nominal_call_v1(
            inventory,
            root,
            root,
            block,
            source_call,
            budget,
            |_, _| Ok(()),
        )?;
        budget.charge_work(RETAIN_PREPARE_WORK)?;
        let function = owner
            .semantic_ssa()
            .source_semantic()
            .functions()
            .get(root.index() as usize)
            .ok_or(Error::Unavailable(
                "retained Final actual root function absent",
            ))?;
        with_retained_scope(budget, function.blocks().len(), move |budget, protected| {
            let mut retained = RetainedEffects::new(function.blocks().len(), budget)?;
            let mut pending = None;
            let mut observed = [0usize; 3];
            let run = with_nominal_source_preparation_v1(
                owner,
                inventory,
                root,
                root,
                block,
                source_call,
                budget,
                |inputs, budget| {
                    with_nominal_canonical_facts_observation_v1(
                        owner,
                        inventory,
                        root,
                        root,
                        block,
                        source_call,
                        budget,
                        |facts| {
                            with_nominal_capability_consumer_v1(facts, |site, consumer| {
                                require(
                                    std::ptr::eq(site.owner(), owner)
                                        && std::ptr::eq(site.inventory(), inventory)
                                        && std::ptr::eq(site.call(), source_call)
                                        && std::ptr::eq(site.function(), function)
                                        && site.root() == root
                                        && site.block() == block,
                                    "final candidate real consumer source differs",
                                )?;
                                let mut visit =
                                    |pass: NominalCapabilityPassV1,
                                     authenticated: &AuthenticatedNominalCallerV1<'_>,
                                     budget: &mut Budget<'_>| {
                                        budget.charge_work(8)?;
                                        let index = match pass {
                                            NominalCapabilityPassV1::Initial => 0,
                                            NominalCapabilityPassV1::Repeated => 1,
                                            NominalCapabilityPassV1::Final => 2,
                                        };
                                        observed[index] = add(observed[index], 1)?;
                                        if pass == NominalCapabilityPassV1::Final {
                                            require(
                                                pending.is_none(),
                                                "final candidate captured more than once",
                                            )?;
                                            with_nominal_layout_return_v1(
                                                authenticated,
                                                budget,
                                                |occurrence, budget| {
                                                    with_nominal_ranked_tensor_proxy_v1(
                                                        occurrence,
                                                        budget,
                                                        |proxy, budget| {
                                                            pending = Some(capture(proxy, budget)?);
                                                            Ok(())
                                                        },
                                                    )
                                                },
                                            )?;
                                        }
                                        Ok(())
                                    };
                                let (run, ()) = run_nominal_capability_dataflow_v1(
                                    site,
                                    inputs,
                                    consumer,
                                    &mut visit,
                                    |effects, run, consumer| {
                                        retained.capture(effects, run, consumer, protected)
                                    },
                                )?;
                                Ok(run)
                            })
                        },
                    )
                },
            )?;
            // No external visitor has run. ALL three nested source-owned scopes
            // returned, including postflight and owned-only cleanup.
            protected.require_completed(budget)?;
            budget.charge_work(16)?;
            require_completed_run(run, observed, pending.is_some())?;
            require(
                retained.completed == Some(run),
                "retained Final completed table differs from the actual dense run",
            )?;
            let pending = pending.ok_or(Error::Unavailable("final candidate payload absent"))?;
            let candidate = rejoin(owner, inventory, root, block, source_call, &pending, budget)?;
            protected.require_completed(budget)?;
            budget.charge_work(RETAIN_REJOIN_WORK)?;
            require(
                owner
                    .semantic_ssa()
                    .source_semantic()
                    .functions()
                    .get(root.index() as usize)
                    .is_some_and(|actual| std::ptr::eq(actual, function))
                    && retained.rows.len() == function.blocks().len()
                    && retained.rows.capacity() == retained.rows.len(),
                "retained Final source/block table changed before its immutable loan",
            )?;
            let view = NominalFinalRetainedEffectsV1 {
                candidate: &candidate,
                function,
                effects: &retained.rows,
                run,
            };
            protected.require_completed(budget)?;
            let result = inspect(&view, budget);
            drop(view);
            drop(candidate);
            drop(pending);
            drop(retained);
            result
        })
    })
}

#[cfg(test)]
#[path = "bf16_nominal_final_retained_effects_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "bf16_nominal_final_retained_effects_genuine_v1_tests.rs"]
pub(in crate::production_ranked_projection_v1) mod genuine;
