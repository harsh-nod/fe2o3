//! Source-owned final candidate handoff, NOT ranked/normal admission.
//! All preparation, facts, and dense postflights finish before the borrowed
//! candidate is exposed. Source coordinates are NOT ranked recipe coordinates.
use super::bf16_nominal_capabilities_v1::AuthenticatedNominalCallerV1;
use super::bf16_nominal_dense_v1::{
    NominalCapabilityPassV1, NominalCapabilityRunV1, run_nominal_capability_dataflow_v1,
};
use super::bf16_nominal_layout_return_v1::{
    NominalReturnComponentV1, with_nominal_layout_return_v1,
};
use super::bf16_nominal_ranked_proxy_v1::{
    NominalRankedTensorProxyV1, with_nominal_ranked_tensor_proxy_v1,
};
use super::bf16_nominal_source_preparation_v1::with_nominal_source_preparation_v1;
use super::canonical_assertion_facts_v1::{
    with_nominal_canonical_facts_observation_v1, with_nominal_capability_consumer_v1,
};
use super::{
    AuthenticatedTensorInstructionV1, DigestV1, ProductionCooperativeTensorBindingV1,
    ProductionRankedOperationV1, SemanticBlockIdV1, SemanticDirectCallV1, SemanticFunctionIdV1,
    TensorConvergenceAttr, tensor_capability_root_v1, tensor_operand_root_v1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirBlockRefV1, CanonicalKirInventoryV1, CanonicalKirOperationRefV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1, CanonicalKirBlockCoordinateV1 as BlockCoordinate,
    CanonicalKirDefinitionCoordinateV1 as DefinitionCoordinate,
    CanonicalKirOperationCoordinateV1 as OperationCoordinate,
    CanonicalKirUseCoordinateV1 as UseCoordinate, OperationKind, TensorLayoutContractV1,
    Terminator, Type,
};
use fe2o3_lower_mir_kernel::{
    Bf16CallInstanceRoleV1, Bf16NominalCallQueryErrorV1 as Error, ProductionPreRankedKirOwnerV1,
};
use sha2::Sha256;
use std::mem::{size_of, size_of_val};
use std::panic::{AssertUnwindSafe, catch_unwind};

type Result<T> = std::result::Result<T, Error>;
const SCOPE_WORK: usize = 32;
const CAPTURE_WORK: usize = 256;
const REJOIN_WORK: usize = 1024;
const REHASH_WORK: usize = 2048;

// Private, unvalidated payload. It never escapes, has no public constructor,
// is never accepted as input, and cannot substitute for the live source run.
struct PendingCandidate {
    operation: ProductionRankedOperationV1,
    call: OperationCoordinate,
    matrix: OperationCoordinate,
    returned: BlockCoordinate,
    rows: [NominalReturnComponentV1; 4],
    permutation: [u8; 4],
    tensor: AuthenticatedTensorInstructionV1,
    binding: DigestV1,
}

/// Only this module constructs this view, after its complete source-owned
/// invocation returned successfully and exact immutable rows were rejoined.
/// Not Clone/Copy and no owned receipt, access-ready conversion or recipe insert.
pub(super) struct NominalFinalRankedCandidateV1<'scope, 'graph> {
    owner: &'scope ProductionPreRankedKirOwnerV1,
    inventory: &'scope CanonicalKirInventoryV1<'graph>,
    source_call: &'scope SemanticDirectCallV1,
    call: &'scope CanonicalKirOperationRefV1<'graph>,
    matrix: &'scope CanonicalKirOperationRefV1<'graph>,
    returned: &'scope CanonicalKirBlockRefV1<'graph>,
    pending: &'scope PendingCandidate,
}
impl<'scope, 'graph> NominalFinalRankedCandidateV1<'scope, 'graph> {
    pub(super) const fn owner(&self) -> &'scope ProductionPreRankedKirOwnerV1 {
        self.owner
    }
    pub(super) const fn inventory(&self) -> &'scope CanonicalKirInventoryV1<'graph> {
        self.inventory
    }
    pub(super) const fn source_call(&self) -> &'scope SemanticDirectCallV1 {
        self.source_call
    }
    pub(super) const fn call(&self) -> &'scope CanonicalKirOperationRefV1<'graph> {
        self.call
    }
    pub(super) const fn matrix(&self) -> &'scope CanonicalKirOperationRefV1<'graph> {
        self.matrix
    }
    pub(super) const fn returned(&self) -> &'scope CanonicalKirBlockRefV1<'graph> {
        self.returned
    }
    pub(super) const fn operation(&self) -> &ProductionRankedOperationV1 {
        &self.pending.operation
    }
    pub(super) const fn rows(&self) -> &[NominalReturnComponentV1; 4] {
        &self.pending.rows
    }
    pub(super) const fn permutation(&self) -> [u8; 4] {
        self.pending.permutation
    }
}

fn require(condition: bool, reason: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Unavailable(reason))
    }
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(Resource::Arithmetic.into())
}
fn times(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b).ok_or(Resource::Arithmetic.into())
}
fn scope_storage<R>(callback_bytes: usize) -> Result<usize> {
    let mut bytes = 8192usize; // bounded internal closure, traversal and panic-match frames
    for amount in [
        times(size_of::<PendingCandidate>(), 4)?,
        times(size_of::<ProductionRankedOperationV1>(), 2)?,
        times(
            size_of::<NominalFinalRankedCandidateV1<'static, 'static>>(),
            2,
        )?,
        times(size_of::<Checkpoint>(), 4)?,
        times(size_of::<NominalCapabilityRunV1>(), 3)?,
        times(size_of::<ProductionCooperativeTensorBindingV1>(), 2)?,
        times(size_of::<AuthenticatedTensorInstructionV1>(), 2)?,
        size_of::<Sha256>(),
        times(size_of::<[DigestV1; 6]>(), 2)?,
        times(callback_bytes, 4)?,
        times(size_of::<Result<R>>(), 4)?,
    ] {
        bytes = add(bytes, amount)?;
    }
    Ok(bytes)
}

#[derive(Clone, Copy)]
struct Checkpoint {
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    work: usize,
    storage: usize,
    peak: usize,
}
impl Checkpoint {
    fn take(budget: &Budget<'_>) -> Self {
        Self {
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            work: budget.work(),
            storage: budget.storage(),
            peak: budget.peak_storage(),
        }
    }
    fn require_custody(self, budget: &Budget<'_>) -> Result<()> {
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.storage
            || budget.work() < self.work
            || budget.peak_storage() < self.peak
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
    fn require_completed(self, budget: &Budget<'_>) -> Result<()> {
        self.require_custody(budget)?;
        // This source-owned body grants no external visitor before completion.
        // Every nested scope owns/refunds its own storage. Only our outer slot
        // remains. A silent inner leak is a refusal, never candidate readiness.
        if budget.storage() != self.storage
            || budget.failed_work().is_some()
            || budget.failed_storage().is_some()
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}

// Separate accounting core permits synthetic tests without forging source data.
fn with_owned_scope<'w, R: Copy + 'static, F>(budget: &mut Budget<'w>, body: F) -> Result<R>
where
    F: FnOnce(&mut Budget<'w>, Checkpoint) -> Result<R>,
{
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    let reserved = scope_storage::<R>(size_of_val(&body))?;
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
    // All private candidate/view values and callback captures have dropped.
    // On invalid custody do not repair the replacement or undercut account.
    protected.require_custody(budget)?;
    budget.release_storage(reserved)?;
    result
}

fn capture(
    proxy: &NominalRankedTensorProxyV1<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<PendingCandidate> {
    budget.charge_work(CAPTURE_WORK)?;
    let occurrence = proxy.occurrence();
    let ProductionRankedOperationV1::TensorLayout {
        contract,
        convergence,
        active_lanes,
        binding: Some(binding),
    } = proxy.operation()
    else {
        return Err(Error::Unavailable(
            "final candidate proxy is not a bound tensor",
        ));
    };
    // Copy ONLY the fixed TensorLayout variant; no generic enum clone or heap
    // data crosses the query. The outer scope has prepaid this slot already.
    Ok(PendingCandidate {
        operation: ProductionRankedOperationV1::TensorLayout {
            contract: *contract,
            convergence: *convergence,
            active_lanes: *active_lanes,
            binding: Some(*binding),
        },
        call: occurrence.call_coordinate(),
        matrix: occurrence.matrix_coordinate(),
        returned: occurrence.return_coordinate(),
        rows: *occurrence.return_rows(),
        permutation: occurrence.permutation(),
        tensor: occurrence.input_tensor(),
        binding: occurrence.binding_digest(),
    })
}

fn require_completed_run(
    run: NominalCapabilityRunV1,
    visits: [usize; 3],
    captured: bool,
) -> Result<()> {
    require(
        run.query_visits == [1, 1, 1]
            && run.authenticated_visits == visits
            && visits[2] == 1
            && captured
            && !run.array_destination_has_origin,
        "final candidate lacks a completed genuine three-pass invocation",
    )
}

fn block_row<'a, 'g>(
    inventory: &'a CanonicalKirInventoryV1<'g>,
    coordinate: BlockCoordinate,
) -> Result<&'a CanonicalKirBlockRefV1<'g>> {
    let function = inventory
        .functions()
        .get(coordinate.function.0 as usize)
        .ok_or(Error::Unavailable("final candidate function row absent"))?;
    require(
        function.coordinate == coordinate.function,
        "final candidate function coordinate differs",
    )?;
    let index = add(function.blocks.start, coordinate.block as usize)?;
    require(
        index < function.blocks.end,
        "final candidate block leaves its function",
    )?;
    let block = inventory
        .blocks()
        .get(index)
        .ok_or(Error::Unavailable("final candidate block row absent"))?;
    let body = function
        .function
        .body
        .as_ref()
        .ok_or(Error::Unavailable("final candidate function body absent"))?;
    require(
        block.coordinate == coordinate
            && body
                .blocks
                .get(coordinate.block as usize)
                .is_some_and(|b| std::ptr::eq(b, block.block))
            && block
                .block
                .terminator
                .as_ref()
                .is_some_and(|term| std::ptr::eq(block.terminator, term)),
        "final candidate actual block row differs",
    )?;
    Ok(block)
}
fn operation_row<'a, 'g>(
    inventory: &'a CanonicalKirInventoryV1<'g>,
    coordinate: OperationCoordinate,
) -> Result<&'a CanonicalKirOperationRefV1<'g>> {
    let block = block_row(inventory, coordinate.block)?;
    let index = add(block.operations.start, coordinate.operation as usize)?;
    require(
        index < block.operations.end,
        "final candidate operation leaves its block",
    )?;
    let operation = inventory
        .operations()
        .get(index)
        .ok_or(Error::Unavailable("final candidate operation row absent"))?;
    require(
        operation.coordinate == coordinate
            && block
                .block
                .operations
                .get(coordinate.operation as usize)
                .is_some_and(|op| std::ptr::eq(op, operation.operation)),
        "final candidate actual operation row differs",
    )?;
    Ok(operation)
}

// This rejoin cannot be called by an external consumer with a detached DTO.
// It receives only this invocation's PRIVATE payload AFTER every existing
// query/dense/facts/source-preparation postflight succeeded on the original
// account. It lends the actual immutable source/inventory rows, not their hash.
#[allow(clippy::too_many_arguments)]
fn rejoin<'a, 'g>(
    owner: &'a ProductionPreRankedKirOwnerV1,
    inventory: &'a CanonicalKirInventoryV1<'g>,
    root: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    source_call: &'a SemanticDirectCallV1,
    pending: &'a PendingCandidate,
    budget: &mut Budget<'_>,
) -> Result<NominalFinalRankedCandidateV1<'a, 'g>> {
    budget.charge_work(REJOIN_WORK)?;
    let emission = owner
        .bf16_call_instance_emission_v1()
        .ok_or(Error::Unavailable("final candidate source emission absent"))?;
    require(
        inventory.belongs_to(owner.executable())
            && emission.root() == root
            && emission.source_call_block() == block
            && emission.return_permutation() == pending.permutation,
        "final candidate original owner/inventory/source association differs",
    )?;
    let source = owner.semantic_ssa().source_semantic();
    let caller = source
        .functions()
        .get(root.index() as usize)
        .ok_or(Error::Unavailable(
            "final candidate actual source caller absent",
        ))?;
    let source_block = caller
        .blocks()
        .get(block.index() as usize)
        .ok_or(Error::Unavailable(
            "final candidate actual source block absent",
        ))?;
    let super::SemanticTerminatorKindV1::Call(actual_source_call) =
        source_block.terminator().kind()
    else {
        return Err(Error::Unavailable(
            "final candidate source terminal differs",
        ));
    };
    require(
        std::ptr::eq(actual_source_call, source_call),
        "final candidate source call pointer differs",
    )?;
    require(
        pending.call.block.function != pending.matrix.block.function
            && pending.returned.function == pending.matrix.block.function,
        "final candidate qualified namespaces differ",
    )?;
    let call = operation_row(inventory, pending.call)?;
    let matrix = operation_row(inventory, pending.matrix)?;
    let returned = block_row(inventory, pending.returned)?;
    let call_block = block_row(inventory, pending.call.block)?;
    let matrix_block = block_row(inventory, pending.matrix.block)?;
    require(
        emission.call_site() == (call_block.block.id, pending.call.operation)
            && emission.matrix_site() == (matrix_block.block.id, pending.matrix.operation),
        "final candidate source/canonical occurrence differs",
    )?;
    let OperationKind::Call { arguments, .. } = &call.operation.kind else {
        return Err(Error::Unavailable("final candidate actual Call absent"));
    };
    let OperationKind::Matrix(actual_matrix) = &matrix.operation.kind else {
        return Err(Error::Unavailable("final candidate actual Matrix absent"));
    };
    let Terminator::Return { values } = returned.terminator else {
        return Err(Error::Unavailable("final candidate actual Return absent"));
    };
    let required = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
        .with_zero_filled_predicate_inputs();
    let ProductionRankedOperationV1::TensorLayout {
        contract,
        convergence,
        active_lanes,
        binding: Some(binding),
    } = &pending.operation
    else {
        return Err(Error::Unavailable("final candidate tensor row differs"));
    };
    require(
        *contract == required
            && pending.tensor.contract == required
            && *convergence == TensorConvergenceAttr::UniformSubgroup
            && *active_lanes == 64
            && actual_matrix.tensor_layout == Some(required)
            && actual_matrix.active_lanes == 64
            && arguments.len() == 12
            && call.operation.results.len() == 4
            && matrix.operation.results.len() == 4
            && values.len() == 4
            && binding.argument_count() == 4
            && binding.result_root() == pending.binding,
        "final candidate fixed tensor/component contract differs",
    )?;
    let emitted_matrix = emission.producer_components(Bf16CallInstanceRoleV1::Result);
    require(
        emitted_matrix.len() == 4,
        "final candidate emitted Matrix width differs",
    )?;
    for index in 0..4 {
        let producer = usize::from(pending.permutation[index]);
        require(
            producer < 4,
            "final candidate permutation outside four components",
        )?;
        let row = pending.rows[index];
        require(
            row.caller_definition()
                == (DefinitionCoordinate::Result {
                    operation: pending.call,
                    result: index as u32,
                })
                && row.helper_return()
                    == (UseCoordinate::TerminatorOperand {
                        block: pending.returned,
                        operand: index as u32,
                    })
                && row.matrix_definition()
                    == (DefinitionCoordinate::Result {
                        operation: pending.matrix,
                        result: producer as u32,
                    })
                && row.caller_value() == call.operation.results[index].id
                && row.caller_value() == emission.call_results()[index]
                && row.return_value() == values[index]
                && row.return_value() == emission.helper_return()[index]
                && row.matrix_value() == matrix.operation.results[producer].id
                && row.matrix_value() == emitted_matrix[producer]
                && call.operation.results[index].ty == Type::F32
                && matrix.operation.results[producer].ty == Type::F32,
            "final candidate actual qualified component row differs",
        )?;
    }
    // Do not require Return ValueIds to equal Matrix ValueIds: real edge
    // transport was authenticated in this invocation's N1/C3, before postflight.
    budget.charge_work(REHASH_WORK)?;
    let tensor = pending.tensor;
    require(
        binding.context_root() == tensor_capability_root_v1(1, &[tensor.context_root])
            && binding.lane_root() == tensor_capability_root_v1(2, &[tensor.accumulator.lane_root])
            && binding.lhs_root() == tensor_operand_root_v1(tensor.lhs)
            && binding.rhs_root() == tensor_operand_root_v1(tensor.rhs)
            && binding.accumulator_root()
                == tensor_capability_root_v1(6, &[tensor.accumulator.flow_root]),
        "final candidate actual input roots differ",
    )?;
    // Rejoin exact checked N1 inventory rows again, then let THAT scope finish
    // its postflight before lending the final candidate. Its observer returns
    // only unit and cannot expose provisional output. This is not a fourth
    // capability propagation pass and does not consume caller operands twice.
    owner.with_checked_bf16_nominal_call_v1(
        inventory,
        root,
        root,
        block,
        source_call,
        budget,
        |checked, budget| {
            budget.charge_work(16)?;
            require(
                checked.belongs_to(inventory)
                    && std::ptr::eq(checked.call(), call)
                    && std::ptr::eq(checked.matrix(), matrix)
                    && checked.helper().coordinate == returned.coordinate.function
                    && std::ptr::eq(checked.source_call(), source_call),
                "final candidate exact checked Call/Matrix/Return source differs",
            )
        },
    )?;
    Ok(NominalFinalRankedCandidateV1 {
        owner,
        inventory,
        source_call,
        call,
        matrix,
        returned,
        pending,
    })
}

/// Source-owned, lexical candidate only. No caller-supplied dense inputs,
/// generic consumer, pass tag, payload, proof Boolean, or replacement meter.
/// Copying operation() yields an UNVALIDATED recipe row; no complete root
/// memory/CFG/output recipe, ranked-position relation or normal admission exists.
#[allow(clippy::too_many_arguments)]
pub(super) fn with_nominal_final_ranked_candidate_v1<'g, 'w, R, F>(
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
    F: for<'scope> FnOnce(&NominalFinalRankedCandidateV1<'scope, 'g>, &mut Budget<'w>) -> Result<R>,
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
        with_owned_scope(budget, move |budget, protected| {
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
                                        && std::ptr::eq(site.call(), source_call),
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
                                    |_, _, _| Ok(()),
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
            let pending = pending.ok_or(Error::Unavailable("final candidate payload absent"))?;
            let view = rejoin(owner, inventory, root, block, source_call, &pending, budget)?;
            protected.require_completed(budget)?;
            let result = inspect(&view, budget);
            drop(view);
            drop(pending);
            result
        })
    })
}

#[cfg(test)]
#[path = "bf16_nominal_final_candidate_v1_tests.rs"]
mod tests;
