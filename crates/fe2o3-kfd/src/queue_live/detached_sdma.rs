//! Exact completed DATA returns through its original SDMA promotion bridge.

#![forbid(unsafe_code)]

use super::*;
use crate::queue::dispatch_binding::DispatchDataStorageRefV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

/// Refusal of an exact detached-DATA to SDMA owner transfer.
///
/// Borrowed admission failures return both originals. After original native
/// validation begins, refusal retains them in the terminal queue instead.
#[must_use = "recovered originals or the terminal queue must remain owned"]
pub struct Gfx942DetachedSdmaFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    recovered: Option<(
        Gfx942DetachedFixedDispatchV1,
        Gfx942SdmaDispatchDataBridgeV1,
    )>,
}

impl Gfx942DetachedSdmaFailureV1 {
    /// Returns the transition error without exposing native identities.
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    /// Consumes this refusal and returns the untouched pre-entry originals, if
    /// admission failed before they were installed in terminal queue custody.
    pub fn into_recovered(
        self,
    ) -> Option<(
        Gfx942DetachedFixedDispatchV1,
        Gfx942SdmaDispatchDataBridgeV1,
    )> {
        self.recovered
    }
}

impl std::fmt::Debug for Gfx942DetachedSdmaFailureV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Gfx942DetachedSdmaFailureV1")
            .field("error", &self.error)
            .field("recovered", &self.recovered.is_some())
            .finish_non_exhaustive()
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "keep exact native input/output custody inline without allocating during transfer"
)]
pub(super) enum Custody {
    Input(
        Gfx942DetachedFixedDispatchV1,
        Gfx942SdmaDispatchDataBridgeV1,
    ),
    Output(Gfx942SdmaBufferV1),
}

pub(crate) struct CompletedDetachedSdmaV1 {
    data: Gfx942FixedDispatchDataV1,
    owner: QueueKeyV1,
    generation: u64,
    logical_bytes: u64,
}

impl CompletedDetachedSdmaV1 {
    pub(crate) fn into_parts(self) -> (Gfx942FixedDispatchDataV1, QueueKeyV1, u64, u64) {
        (self.data, self.owner, self.generation, self.logical_bytes)
    }
}

#[derive(Clone, Copy)]
struct Commit {
    pool_generation: u64,
    outstanding: usize,
}

struct Ledger<'a> {
    generation: Option<u64>,
    count: &'a mut usize,
    identities: &'a mut Vec<Gfx942FixedDispatchStorageIdentityV1>,
    next_insertion: &'a mut Option<usize>,
    outstanding: &'a mut usize,
}

trait Context {
    fn owner(&self) -> QueueKeyV1;
    fn preflight(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn root(&mut self) -> &mut Option<Custody>;
    fn ledger(&mut self) -> Ledger<'_>;
    fn currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn validate_original(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn poison(&mut self);
}

fn contract(message: &'static str) -> ComputeAqlQueueSessionErrorV1 {
    ComputeAqlQueueSessionErrorV1::Contract(message)
}

fn admit(
    owner: QueueKeyV1,
    ledger: &Ledger<'_>,
    detached: &Gfx942DetachedFixedDispatchV1,
    bridge: &Gfx942SdmaDispatchDataBridgeV1,
) -> Result<Commit, ComputeAqlQueueSessionErrorV1> {
    if bridge.owner != owner
        || detached.data.len() != 1
        || detached.generation == 0
        || bridge.pool_generation == 0
    {
        return Err(contract("detached SDMA input owner or cardinality"));
    }
    let data = &detached.data[0];
    if ledger.generation != Some(detached.generation)
        || *ledger.count != 1
        || ledger.identities.as_slice() != [data.storage_identity()]
        || ledger.next_insertion.is_some()
    {
        return Err(contract(
            "detached SDMA original generation or storage ledger",
        ));
    }
    if !matches!(data.storage_ref(), DispatchDataStorageRefV1::HostVisible(_))
        || !data.is_fully_initialized()
        || data.sdma_storage_identity() != bridge.storage_identity
        || bridge.logical_bytes == 0
        || bridge.logical_bytes != bridge.physical_bytes
        || data.layout().requested_bytes() != bridge.physical_bytes
    {
        return Err(contract(
            "detached SDMA original bridge or initialized extent",
        ));
    }
    let pool_generation = bridge
        .pool_generation
        .checked_add(1)
        .filter(|generation| *generation != 0)
        .ok_or_else(|| contract("detached SDMA pool generation exhausted"))?;
    let outstanding = ledger
        .outstanding
        .checked_add(1)
        .ok_or_else(|| contract("detached SDMA buffer ledger exhausted"))?;
    Ok(Commit {
        pool_generation,
        outstanding,
    })
}

fn poison<C: Context>(context: &mut C) {
    core::mem::forget(catch_unwind(AssertUnwindSafe(|| context.poison())));
}

#[allow(clippy::result_large_err)]
fn transfer<C: Context>(
    context: &mut C,
    detached: Gfx942DetachedFixedDispatchV1,
    bridge: Gfx942SdmaDispatchDataBridgeV1,
) -> Result<Gfx942SdmaBufferV1, Gfx942DetachedSdmaFailureV1> {
    let admission = catch_unwind(AssertUnwindSafe(|| {
        context.preflight()?;
        if context.root().is_some() {
            return Err(contract("unfinished detached SDMA transfer"));
        }
        admit(context.owner(), &context.ledger(), &detached, &bridge)
    }));
    let commit = match admission {
        Ok(Ok(commit)) => commit,
        Ok(Err(error)) => {
            return Err(Gfx942DetachedSdmaFailureV1 {
                error,
                recovered: Some((detached, bridge)),
            });
        }
        Err(payload) => {
            if context.root().is_some() {
                std::process::abort();
            }
            *context.root() = Some(Custody::Input(detached, bridge));
            poison(context);
            resume_unwind(payload)
        }
    };
    *context.root() = Some(Custody::Input(detached, bridge));
    let result = catch_unwind(AssertUnwindSafe(|| {
        context.currentness()?;
        context.validate_original()?;
        context.currentness()?;
        // Validation never receives the input owner by value. Once it returns,
        // conversion and ledger commit contain no native operation or callback.
        let Some(Custody::Input(mut detached, bridge)) = context.root().take() else {
            std::process::abort();
        };
        let Some(data) = detached.data.pop() else {
            std::process::abort();
        };
        let buffer =
            Gfx942SdmaBufferV1::from_completed_detached_dispatch_v1(CompletedDetachedSdmaV1 {
                data,
                owner: bridge.owner,
                generation: commit.pool_generation,
                logical_bytes: bridge.logical_bytes,
            });
        *context.root() = Some(Custody::Output(buffer));
        let ledger = context.ledger();
        ledger.identities.clear();
        *ledger.count = 0;
        *ledger.next_insertion = Some(0);
        *ledger.outstanding = commit.outstanding;
        let Some(Custody::Output(buffer)) = context.root().take() else {
            std::process::abort();
        };
        Ok(buffer)
    }));
    match result {
        Ok(Ok(buffer)) => Ok(buffer),
        Ok(Err(error)) => {
            poison(context);
            Err(Gfx942DetachedSdmaFailureV1 {
                error,
                recovered: None,
            })
        }
        Err(payload) => {
            poison(context);
            resume_unwind(payload)
        }
    }
}

impl ComputeAqlQueueSessionV1 {
    /// Consumes one exact completed detached coherent DATA extent and its
    /// original SDMA promotion bridge as an initialized SDMA read owner.
    ///
    /// This requires the bridge retained from the original SDMA-to-DATA
    /// promotion; ordinary DATA without it is not admitted. The returned pool
    /// generation advances once and no pre-compute content digest is preserved.
    /// This transfers storage custody, not a kernel/value proof or peer route.
    #[allow(clippy::result_large_err)]
    pub fn transfer_detached_fixed_dispatch_to_sdma_v1(
        &mut self,
        detached: Gfx942DetachedFixedDispatchV1,
        bridge: Gfx942SdmaDispatchDataBridgeV1,
    ) -> Result<Gfx942SdmaBufferV1, Gfx942DetachedSdmaFailureV1> {
        transfer(self, detached, bridge)
    }
}

impl Context for ComputeAqlQueueSessionV1 {
    fn owner(&self) -> QueueKeyV1 {
        self.key
    }
    fn preflight(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_sdma_enabled()?;
        self.require_unbound_fixed_dispatch()?;
        if !self.unpublished_dispatch.is_clear() || self.has_any_persistent_compute_attachment_v1()
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        Ok(())
    }
    fn root(&mut self) -> &mut Option<Custody> {
        &mut self.detached_sdma
    }
    fn ledger(&mut self) -> Ledger<'_> {
        Ledger {
            generation: self.detached_dispatch_generation,
            count: &mut self.detached_data_count,
            identities: &mut self.detached_data_identities,
            next_insertion: &mut self.detached_next_insertion_index,
            outstanding: &mut self.sdma_outstanding_buffers,
        }
    }
    fn currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.check_currentness()
    }
    fn validate_original(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let (validation, retake) = execute_live_model_custody_v1(
            self,
            Self::restore_model_ownership_for_live_mutation,
            |session| {
                let engine = session
                    .engine
                    .as_ref()
                    .ok_or_else(|| contract("missing queue engine"))?;
                let Some(Custody::Input(detached, _)) = session.detached_sdma.as_ref() else {
                    std::process::abort();
                };
                let DispatchDataStorageRefV1::HostVisible(token) = detached.data[0].storage_ref()
                else {
                    std::process::abort();
                };
                engine
                    .backend
                    .session
                    .preflight_mapped_queue_token_v1(token)
                    .map_err(Into::into)
            },
            Self::retake_model_ownership_after_live_mutation,
            poison,
        )?;
        retake?;
        validation
    }
    fn poison(&mut self) {
        self.poison_terminal();
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
}

#[cfg(test)]
#[path = "detached_sdma/tests.rs"]
mod tests;
