//! One private queue root with four independently settled original recipes.

use super::*;
use crate::queue::dispatch_binding::{
    Gfx942NativeFillRegistryInputsV1, Gfx942NativeFillRegistryStorageV1,
};
use fixed_dispatch::recipe::RecipeV1;

/// Actual published single-recipe custody. No conversion to a scalar or cohort
/// receipt is exposed, and no receipt is copied from an aggregate completion.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx942NativeFillRegistryBatchV1, Gfx942DispatchBatchV1};
/// fn ordinary(_: Gfx942DispatchBatchV1<1>) {}
/// fn reject(receipt: Gfx942NativeFillRegistryBatchV1) { ordinary(receipt); }
/// ```
/// ```compile_fail
/// fn duplicate(receipt: fe2o3_kfd::Gfx942NativeFillRegistryBatchV1) {
///     let _copy = receipt.clone();
/// }
/// ```
#[derive(Debug)]
#[must_use = "retain the original registry publication through completion"]
pub struct Gfx942NativeFillRegistryBatchV1 {
    registry: u64,
    recipe: usize,
    batch: Gfx942DispatchBatchV1<1>,
}

/// Actual completion of exactly one original registry recipe before recycle.
#[derive(Debug)]
#[must_use = "the original registry completion must be recycled"]
pub struct Gfx942NativeFillRegistryCompletedV1 {
    registry: u64,
    recipe: usize,
    completed: Gfx942CompletedDispatchBatchV1<1>,
}

#[derive(Debug)]
pub enum Gfx942NativeFillRegistryPollV1 {
    Pending(Gfx942NativeFillRegistryBatchV1),
    Ready(Gfx942NativeFillRegistryCompletedV1),
}

/// Pre-entry refusal returns the exact original publication. A terminal native
/// failure does not grant disposal or replacement authority.
#[derive(Debug)]
pub struct Gfx942NativeFillRegistryPollFailureV1 {
    pub error: ComputeAqlQueueSessionErrorV1,
    pub refused: Option<Gfx942NativeFillRegistryBatchV1>,
}

#[derive(Debug)]
pub struct Gfx942NativeFillRegistryRecycleFailureV1 {
    pub error: ComputeAqlQueueSessionErrorV1,
    pub retryable: Option<Gfx942NativeFillRegistryCompletedV1>,
}

/// Closed registry of four original, disjoint fill recipes on one actual queue.
/// Each can be submitted once, polled and recycled independently. `WaitForPrior`
/// remains mandatory: this does not establish physical overlap, out-of-order
/// execution, rolling admission, a runtime bridge or thousand-operation depth.
///
/// CODE, kernarg, DATA and queue custody remain here until explicit destruction.
/// Readback of one exactly recycled recipe does not require other recipes to
/// finish, because admission retains distinct full allocations. Forgetting a
/// receipt cannot authorize teardown. Dropping an undestroyed registry aborts
/// before its original queue or metadata credits can be released.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx942NativeFillRegistrySessionV1, ComputeAqlQueueSessionV1};
/// fn ordinary(_: ComputeAqlQueueSessionV1) {}
/// fn reject(registry: Gfx942NativeFillRegistrySessionV1) { ordinary(registry); }
/// ```
pub struct Gfx942NativeFillRegistrySessionV1 {
    queue: Option<ComputeAqlQueueSessionV1>,
    storage: Gfx942NativeFillRegistryStorageV1,
    destroyed: bool,
}

impl SharedGttMemorySessionV1 {
    /// Consumes this original VM and all four original inputs through the same
    /// rooted primary construction as the checked cohort. The resulting owner
    /// exposes only the distinct registry interface, never the underlying queue.
    pub fn create_compute_aql_queue_with_native_fill_registry_v1(
        self,
        ring_bytes: u32,
        inputs: Gfx942NativeFillRegistryInputsV1<'_>,
        storage: Gfx942NativeFillRegistryStorageV1,
    ) -> Result<Gfx942NativeFillRegistrySessionV1, ComputeAqlQueueSessionErrorV1> {
        let crate::queue::dispatch_binding::Gfx942NativeFillCohortV1 {
            programs,
            packets,
            data,
        } = inputs.cohort;
        let mut root = PrimaryQueueConstructionV1::new(
            self,
            (
                programs,
                FixedDispatchPreparationCustodyV1::new_native_fill_cohort(packets, data),
                storage,
            ),
        );
        root = root.run(|root, entry| root.construct_native_fill_registry(entry, ring_bytes))?;
        let Some(completed) = root.completed.take() else {
            std::process::abort();
        };
        let queue = completed.into_session();
        let (_, _, storage) = root.preparation;
        // No allocation, validation or user callback separates these owner moves.
        Ok(Gfx942NativeFillRegistrySessionV1 {
            queue: Some(queue),
            storage,
            destroyed: false,
        })
    }
}

type RegistryPreparationV1<'a> = (
    Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'a>>,
    FixedDispatchPreparationCustodyV1<4>,
    Gfx942NativeFillRegistryStorageV1,
);

impl<E: construction_primary::PrimaryEnvironmentV1>
    PrimaryQueueConstructionV1<RegistryPreparationV1<'_>, E>
where
    E::Memory: crate::queue::dispatch_binding::preparation::PreparationMemoryV1,
{
    pub(super) fn construct_native_fill_registry(
        &mut self,
        entry: &mut construction_primary::UserptrConstructionEntryV1<'_>,
        ring_bytes: u32,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        validate_fixed_batch_ring::<4>(ring_bytes)?;
        let memory = self
            .memory
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing registry construction memory",
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

impl Gfx942NativeFillRegistrySessionV1 {
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
    ) -> Result<Gfx942NativeFillRegistryBatchV1, Gfx942FixedDispatchSubmissionFailureV1> {
        let registry = self.storage.identity();
        if recipe >= self.storage.recipes.len() {
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
        let owner = &mut self.storage.recipes[recipe];
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            queue.submit_selected_fixed_dispatch_using(
                FixedDispatchBindingModeV1::Ordinary,
                &mut RecipeV1::Registry(owner),
                |queue, packets| queue.submit_prepared_batch_classified(packets),
            )
        }));
        match operation {
            Ok(result) => result
                .map(|batch| Gfx942NativeFillRegistryBatchV1 {
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
        batch: Gfx942NativeFillRegistryBatchV1,
    ) -> Result<Gfx942NativeFillRegistryPollV1, Gfx942NativeFillRegistryPollFailureV1> {
        if batch.registry != self.storage.identity()
            || batch.recipe >= self.storage.recipes.len()
            || self.queue.is_none()
        {
            return Err(Gfx942NativeFillRegistryPollFailureV1 {
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
            return Err(Gfx942NativeFillRegistryPollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                refused: Some(batch),
            });
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = queue.poll_selected_fixed_dispatch(
                batch.batch,
                &mut RecipeV1::Registry(&mut self.storage.recipes[recipe]),
            );
            queue.terminalize_fixed_dispatch_observation_result_v1(result)
        }));
        match operation {
            Ok(Ok(Gfx942DispatchPollWithProgressV1::Pending { batch, .. })) => Ok(
                Gfx942NativeFillRegistryPollV1::Pending(Gfx942NativeFillRegistryBatchV1 {
                    registry,
                    recipe,
                    batch,
                }),
            ),
            Ok(Ok(Gfx942DispatchPollWithProgressV1::Ready { completed, .. })) => Ok(
                Gfx942NativeFillRegistryPollV1::Ready(Gfx942NativeFillRegistryCompletedV1 {
                    registry,
                    recipe,
                    completed,
                }),
            ),
            Ok(Err(error)) => Err(Gfx942NativeFillRegistryPollFailureV1 {
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
        completed: Gfx942NativeFillRegistryCompletedV1,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942NativeFillRegistryRecycleFailureV1>
    {
        if completed.registry != self.storage.identity()
            || completed.recipe >= self.storage.recipes.len()
            || self.queue.is_none()
        {
            return Err(Gfx942NativeFillRegistryRecycleFailureV1 {
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
            return Err(Gfx942NativeFillRegistryRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                retryable: Some(completed),
            });
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = queue.recycle_selected_fixed_dispatch(
                completed.completed,
                &mut RecipeV1::Registry(&mut self.storage.recipes[recipe]),
            );
            queue.terminalize_fixed_dispatch_recycle_result_v1(result)
        }));
        match operation {
            Ok(result) => result.map_err(|failure| Gfx942NativeFillRegistryRecycleFailureV1 {
                error: failure.error,
                retryable: failure.retryable_completed.map(|completed| {
                    Gfx942NativeFillRegistryCompletedV1 {
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

    /// Reads exactly the selected original full output after its actual signal
    /// recycle. Other disjoint recipes may remain published. No native owner,
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
        let owner = self
            .storage
            .recipes
            .get(recipe)
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
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
        self.queue()?;
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

impl Drop for Gfx942NativeFillRegistrySessionV1 {
    fn drop(&mut self) {
        if !self.destroyed {
            std::process::abort();
        }
    }
}
