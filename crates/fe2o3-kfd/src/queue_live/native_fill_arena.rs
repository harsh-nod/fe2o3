//! Private original common allocation and queue, with separately returned slots.

use super::*;
use crate::queue::dispatch_binding::{ArenaOrderV1, Gfx942IndependentFillArenaInputsV1};
use crate::queue::dispatch_binding::{
    GFX942_NATIVE_FILL_ARENA_SLOTS_V1, Gfx942NativeFillArenaInputsV1,
    Gfx942NativeFillArenaPacketsV1, Gfx942NativeFillArenaStorageV1,
};
use fixed_dispatch::recipe::RecipeV1;
const SLOTS: usize = GFX942_NATIVE_FILL_ARENA_SLOTS_V1;

#[path = "native_fill_arena/independent.rs"]
mod independent;
pub use independent::Gfx942IndependentFillArenaSessionV1;

#[derive(Debug)]
#[must_use = "retain the original registry publication through completion"]
pub struct Gfx942NativeFillArenaBatchV1 {
    registry: u64,
    recipe: usize,
    batch: Gfx942DispatchBatchV1<1>,
}

/// Actual completion of exactly one original registry recipe before recycle.
#[derive(Debug)]
#[must_use = "the original registry completion must be recycled"]
pub struct Gfx942NativeFillArenaCompletedV1 {
    registry: u64,
    recipe: usize,
    completed: Gfx942CompletedDispatchBatchV1<1>,
}

#[derive(Debug)]
pub enum Gfx942NativeFillArenaPollV1 {
    Pending(Gfx942NativeFillArenaBatchV1),
    Ready(Gfx942NativeFillArenaCompletedV1),
}

/// Pre-entry refusal returns the exact original publication. A terminal native
/// failure does not grant disposal or replacement authority.
#[derive(Debug)]
pub struct Gfx942NativeFillArenaPollFailureV1 {
    pub error: ComputeAqlQueueSessionErrorV1,
    pub refused: Option<Gfx942NativeFillArenaBatchV1>,
}

#[derive(Debug)]
pub struct Gfx942NativeFillArenaRecycleFailureV1 {
    pub error: ComputeAqlQueueSessionErrorV1,
    pub retryable: Option<Gfx942NativeFillArenaCompletedV1>,
}

/** One private queue and one common DATA allocation with 1024 disjoint,
single-use fill slots. Per-slot copies require original completion and recycle;
no suballocation authority, common retirement or independent scheduling follows.
WaitForPrior remains mandatory. No runtime integration, hardware overlap or
out-of-order qualification is claimed. Incomplete Drop aborts before owner loss.

```compile_fail
use fe2o3_kfd::{Gfx942NativeFillArenaBatchV1, Gfx942NativeFillRegistrySessionV1};
fn reject(session: &mut Gfx942NativeFillRegistrySessionV1, batch: Gfx942NativeFillArenaBatchV1) {
    let _ = session.poll(batch);
}
```
*/
pub struct Gfx942NativeFillArenaSessionV1 {
    queue: Option<ComputeAqlQueueSessionV1>,
    storage: Gfx942NativeFillArenaStorageV1,
    destroyed: bool,
}

impl SharedGttMemorySessionV1 {
    /// Consumes the original VM, common DATA and all slots into the existing
    /// primary construction root before native preparation. No ordinary queue
    /// handle or independently disposable range token is exposed.
    pub fn create_compute_aql_queue_with_native_fill_arena_v1(
        self,
        ring_bytes: u32,
        inputs: Gfx942NativeFillArenaInputsV1<'_>,
        storage: Gfx942NativeFillArenaStorageV1,
    ) -> Result<Gfx942NativeFillArenaSessionV1, ComputeAqlQueueSessionErrorV1> {
        self.create_native_fill_arena_with_order_v1(
            ring_bytes,
            inputs,
            storage,
            ArenaOrderV1::Ordered,
        )
    }

    fn create_native_fill_arena_with_order_v1(
        self,
        ring_bytes: u32,
        inputs: Gfx942NativeFillArenaInputsV1<'_>,
        mut storage: Gfx942NativeFillArenaStorageV1,
        expected_order: ArenaOrderV1,
    ) -> Result<Gfx942NativeFillArenaSessionV1, ComputeAqlQueueSessionErrorV1> {
        let Gfx942NativeFillArenaInputsV1 {
            programs,
            packets,
            data,
            order,
        } = inputs;
        let mut premises = match storage.premises.take() {
            Some(premises) => premises,
            None => std::process::abort(),
        };
        premises.order = order;
        let custody =
            FixedDispatchPreparationCustodyV1::new_native_fill_arena(packets, data, premises);
        let mut root = PrimaryQueueConstructionV1::new(self, (programs, custody, storage));
        root = root.run(|root, entry| {
            if order != expected_order {
                return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
            }
            root.construct_native_fill_arena(entry, ring_bytes)
        })?;
        let Some(completed) = root.completed.take() else {
            std::process::abort();
        };
        let queue = completed.into_session();
        let (_, _, storage) = root.preparation;
        // No fallible allocation or callback separates original native owner moves.
        Ok(Gfx942NativeFillArenaSessionV1 {
            queue: Some(queue),
            storage,
            destroyed: false,
        })
    }
}

type ArenaPreparationV1<'a> = (
    Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'a>>,
    FixedDispatchPreparationCustodyV1<SLOTS, Gfx942NativeFillArenaPacketsV1>,
    Gfx942NativeFillArenaStorageV1,
);

impl<E: construction_primary::PrimaryEnvironmentV1>
    PrimaryQueueConstructionV1<ArenaPreparationV1<'_>, E>
where
    E::Memory: crate::queue::dispatch_binding::preparation::PreparationMemoryV1,
{
    pub(super) fn construct_native_fill_arena(
        &mut self,
        entry: &mut construction_primary::UserptrConstructionEntryV1<'_>,
        ring_bytes: u32,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        validate_fixed_batch_ring::<SLOTS>(ring_bytes)?;
        let memory = self
            .memory
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing arena construction memory",
            ))?;
        let geometry = memory.plan_aql_queue_resources(ring_bytes)?;
        self.preparation
            .2
            .prepare(memory, &self.preparation.0, &mut self.preparation.1)?;
        self.dispatch = Some(self.preparation.1.take_completed()?);
        self.construct(
            entry,
            geometry,
            ring_bytes,
            QueueRingBackingV1::AqlSpecial,
            None,
        )
    }
}
impl Gfx942NativeFillArenaSessionV1 {
    /// Borrows only the original device under the existing retained queue
    /// currentness scope. No queue, common DATA or recipe receipt is exposed.
    pub fn with_retained_device_v1<R>(
        &mut self,
        observe: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        self.queue()?.with_retained_device_v1(observe)
    }

    fn queue(&mut self) -> Result<&mut ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        self.queue
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase.into())
    }

    fn poison(&mut self) {
        if let Some(queue) = self.queue.as_mut() {
            queue.poison_terminal();
        }
        poison_process_global_after_dispatch_terminal_v1();
    }

    /// Publishes this original recipe once. Ring/signal capacity refusal retains
    /// the same unpublished recipe and burned generations for an exact retry.
    pub fn submit(
        &mut self,
        recipe: usize,
    ) -> Result<Gfx942NativeFillArenaBatchV1, Gfx942FixedDispatchSubmissionFailureV1> {
        let registry = self.storage.identity();
        if recipe >= SLOTS {
            return Err(
                Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                    Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                ),
            );
        }
        let queue = self.queue.as_mut().ok_or_else(|| {
            Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
            )
        })?;
        let owner = self.storage.recipe(recipe).map_err(|error| {
            Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error.into())
        })?;
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            queue.submit_selected_fixed_dispatch_using(
                FixedDispatchBindingModeV1::Ordinary,
                &mut RecipeV1::Arena(owner),
                |queue, packets| queue.submit_prepared_batch_classified(packets),
            )
        }));
        match operation {
            Ok(result) => result
                .map(|batch| Gfx942NativeFillArenaBatchV1 {
                    registry,
                    recipe,
                    batch,
                })
                .map_err(FixedDispatchSubmissionFailureV1::into_public),
            Err(payload) => {
                self.poison();
                std::panic::resume_unwind(payload)
            }
        }
    }

    // Refusal returns the original linear receipt without a post-effect allocation.
    #[allow(clippy::result_large_err)]
    pub fn poll(
        &mut self,
        batch: Gfx942NativeFillArenaBatchV1,
    ) -> Result<Gfx942NativeFillArenaPollV1, Gfx942NativeFillArenaPollFailureV1> {
        if batch.registry != self.storage.identity()
            || batch.recipe >= SLOTS
            || self.queue.is_none()
            || self
                .storage
                .recipe(batch.recipe)
                .and_then(|owner| owner.validate_original_batch(&batch.batch))
                .is_err()
        {
            return Err(Gfx942NativeFillArenaPollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                refused: Some(batch),
            });
        }
        let registry = batch.registry;
        let recipe = batch.recipe;
        let queue = match self.queue.as_mut() {
            Some(queue) => queue,
            None => std::process::abort(),
        };
        if queue.terminal_poisoned {
            return Err(Gfx942NativeFillArenaPollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                refused: Some(batch),
            });
        }
        let owner = match self.storage.recipe(recipe) {
            Ok(owner) => owner,
            Err(_) => std::process::abort(),
        };
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result =
                queue.poll_selected_fixed_dispatch(batch.batch, &mut RecipeV1::Arena(owner));
            queue.terminalize_fixed_dispatch_observation_result_v1(result)
        }));
        match operation {
            Ok(Ok(Gfx942DispatchPollWithProgressV1::Pending { batch, .. })) => Ok(
                Gfx942NativeFillArenaPollV1::Pending(Gfx942NativeFillArenaBatchV1 {
                    registry,
                    recipe,
                    batch,
                }),
            ),
            Ok(Ok(Gfx942DispatchPollWithProgressV1::Ready { completed, .. })) => Ok(
                Gfx942NativeFillArenaPollV1::Ready(Gfx942NativeFillArenaCompletedV1 {
                    registry,
                    recipe,
                    completed,
                }),
            ),
            Ok(Err(error)) => Err(Gfx942NativeFillArenaPollFailureV1 {
                error,
                refused: None,
            }),
            Err(payload) => {
                self.poison();
                std::panic::resume_unwind(payload)
            }
        }
    }

    // A pinned completion keeps the original inline owner through an exact retry.
    #[allow(clippy::result_large_err)]
    pub fn recycle(
        &mut self,
        completed: Gfx942NativeFillArenaCompletedV1,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942NativeFillArenaRecycleFailureV1> {
        if completed.registry != self.storage.identity()
            || completed.recipe >= SLOTS
            || self.queue.is_none()
            || self
                .storage
                .recipe(completed.recipe)
                .and_then(|owner| owner.validate_original_completed(&completed.completed))
                .is_err()
        {
            return Err(Gfx942NativeFillArenaRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                retryable: Some(completed),
            });
        }
        let registry = completed.registry;
        let recipe = completed.recipe;
        let queue = match self.queue.as_mut() {
            Some(queue) => queue,
            None => std::process::abort(),
        };
        if queue.terminal_poisoned {
            return Err(Gfx942NativeFillArenaRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                retryable: Some(completed),
            });
        }
        let owner = match self.storage.recipe(recipe) {
            Ok(owner) => owner,
            Err(_) => std::process::abort(),
        };
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = queue
                .recycle_selected_fixed_dispatch(completed.completed, &mut RecipeV1::Arena(owner));
            queue.terminalize_fixed_dispatch_recycle_result_v1(result)
        }));
        match operation {
            Ok(result) => result.map_err(|failure| Gfx942NativeFillArenaRecycleFailureV1 {
                error: failure.error,
                retryable: failure.retryable_completed.map(|completed| {
                    Gfx942NativeFillArenaCompletedV1 {
                        registry,
                        recipe,
                        completed,
                    }
                }),
            }),
            Err(payload) => {
                self.poison();
                std::panic::resume_unwind(payload)
            }
        }
    }

    /// Reads exactly the selected original subrange after its actual signal
    /// recycle. Other disjoint subranges may remain published. No native owner,
    /// initialization promise or mapped reference is transferred.
    pub fn read_into(
        &mut self,
        recipe: usize,
        destination: &mut [u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let queue = self
            .queue
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        if queue.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let owner = self.storage.recipe(recipe)?;
        let dispatch = queue
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut queue
            .engine
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .backend
            .session;
        let result = owner.read_into(dispatch, memory, destination);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison();
        }
        result.map_err(Into::into)
    }

    /// Releases all common native backing only after every accepted recipe has
    /// exact completion and signal recycle. Unsubmitted recipes need no invented
    /// receipt. On native teardown failure this owner remains terminal.
    pub fn destroy(&mut self) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
        if !self.storage.settled() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        self.queue()?
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .require_arena_backing_v1()?;
        let queue = match self.queue.take() {
            Some(queue) => queue,
            None => std::process::abort(),
        };
        let result = queue.destroy();
        if result.is_ok() {
            self.destroyed = true;
        } else {
            self.poison();
        }
        result
    }
}

impl Drop for Gfx942NativeFillArenaSessionV1 {
    fn drop(&mut self) {
        if !self.destroyed {
            std::process::abort();
        }
    }
}
