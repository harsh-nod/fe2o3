//! Private selection of the original generation owner, never native token substitution.

use super::*;
use crate::queue::dispatch_binding::RegistryRecipeV1;

pub(in crate::queue::live) enum RecipeV1<'a> {
    Ordinary,
    Registry(&'a mut RegistryRecipeV1),
}

impl RecipeV1<'_> {
    pub(super) fn bind<const N: usize>(
        &mut self,
        queue: &mut ComputeAqlQueueSessionV1,
    ) -> Result<
        (
            Box<[CompletionPacketTemplateV1; N]>,
            DispatchEpochIdentityV1,
        ),
        Gfx942DispatchBindingErrorV1,
    > {
        let common = queue
            .dispatch
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        match self {
            Self::Ordinary => common.bind_templates::<N>(queue.key),
            Self::Registry(recipe) => recipe.bind::<N>(common, queue.key),
        }
    }

    pub(super) fn mark_published<const N: usize>(
        &mut self,
        queue: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletionBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        match self {
            Self::Ordinary => queue
                .dispatch
                .as_mut()
                .expect("dispatch owner retained")
                .mark_published(identity, completion),
            Self::Registry(recipe) => recipe.mark_published(identity, completion),
        }
    }

    pub(super) fn cancel(
        &mut self,
        queue: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        match self {
            Self::Ordinary => queue
                .dispatch
                .as_mut()
                .expect("dispatch owner retained")
                .cancel_binding(identity),
            Self::Registry(recipe) => recipe.cancel(identity),
        }
    }

    pub(super) fn validate_published<const N: usize>(
        &self,
        queue: &ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletionBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        match self {
            Self::Ordinary => queue
                .dispatch
                .as_ref()
                .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
                .validate_published(identity, completion),
            Self::Registry(recipe) => recipe.validate_published(identity, completion),
        }
    }

    pub(super) fn mark_completed<const N: usize>(
        &mut self,
        queue: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletedBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        match self {
            Self::Ordinary => queue
                .dispatch
                .as_mut()
                .expect("dispatch owner retained")
                .mark_completed(identity, completion),
            Self::Registry(recipe) => recipe.mark_completed(identity, completion),
        }
    }

    pub(super) fn validate_completed<const N: usize>(
        &self,
        queue: &ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletedBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        match self {
            Self::Ordinary => queue
                .dispatch
                .as_ref()
                .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
                .validate_completed(identity, completion),
            Self::Registry(recipe) => recipe.validate_completed(identity, completion),
        }
    }

    pub(super) fn recycle(
        &mut self,
        queue: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
        completion: crate::queue::completion::CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        match self {
            Self::Ordinary => queue
                .dispatch
                .as_mut()
                .expect("dispatch owner retained")
                .mark_recycled_occurrence(identity, completion),
            Self::Registry(recipe) => recipe.recycle(identity, completion),
        }
    }

    pub(super) fn poison(&mut self, queue: &mut ComputeAqlQueueSessionV1) {
        match self {
            Self::Ordinary => {
                if let Some(common) = queue.dispatch.as_mut() {
                    common.poison();
                }
            }
            Self::Registry(recipe) => recipe.poison(),
        }
    }
}
