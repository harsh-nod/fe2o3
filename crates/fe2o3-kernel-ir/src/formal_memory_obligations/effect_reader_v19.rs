//! Exact-owner effect-query integration. The rest of the shared formal engine
//! below remains legacy-unmetered; this is not a complete paid report service.
use super::*;
use crate::{
    CanonicalEffectErrorV19, CanonicalEffectScopeV19,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Kernel,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::{convert::Infallible, mem::size_of};

#[derive(Debug)]
pub(super) enum FormalEffectEngineErrorV19<E> {
    Formal(FormalMemoryObligationError),
    Reader(E),
}
impl<E> From<FormalMemoryObligationError> for FormalEffectEngineErrorV19<E> {
    fn from(error: FormalMemoryObligationError) -> Self {
        Self::Formal(error)
    }
}
impl<E> From<GuardedResourceErrorV1> for FormalEffectEngineErrorV19<E> {
    fn from(error: GuardedResourceErrorV1) -> Self {
        Self::Formal(error.into())
    }
}
impl From<Resource> for FormalEffectEngineErrorV19<CanonicalEffectErrorV19> {
    fn from(error: Resource) -> Self {
        Self::Reader(error.into())
    }
}
impl From<CanonicalEffectErrorV19> for FormalEffectEngineErrorV19<CanonicalEffectErrorV19> {
    fn from(error: CanonicalEffectErrorV19) -> Self {
        Self::Reader(error)
    }
}

pub(super) trait EffectReaderV19 {
    type Error;
    fn is_complete_and_pure(&mut self, operation: &Operation) -> Result<bool, Self::Error>;
}

pub(super) struct LegacyEffectReaderV19<'a>(pub &'a crate::InterproceduralEffectAnalysisV1);
impl EffectReaderV19 for LegacyEffectReaderV19<'_> {
    type Error = Infallible;
    fn is_complete_and_pure(&mut self, operation: &Operation) -> Result<bool, Self::Error> {
        let OperationKind::Call { callee, .. } = &operation.kind else {
            return Ok(false);
        };
        Ok(operation.has_complete_effect_summary()
            || self
                .0
                .function(callee)
                .is_some_and(|summary| summary.is_complete_and_pure()))
    }
}

struct CanonicalEffectReaderV19<'scope, 'owner, 'budget, 'work> {
    owner: &'owner Owner,
    scope: &'scope CanonicalEffectScopeV19<'owner>,
    budget: &'budget mut Budget<'work>,
}
impl EffectReaderV19 for CanonicalEffectReaderV19<'_, '_, '_, '_> {
    type Error = CanonicalEffectErrorV19;
    fn is_complete_and_pure(&mut self, operation: &Operation) -> Result<bool, Self::Error> {
        let OperationKind::Call { callee, .. } = &operation.kind else {
            return Err(CanonicalEffectErrorV19::ForeignOwner);
        };
        self.budget.check_prior_denials_v1()?;
        if operation.has_complete_effect_summary_with_budget_v1(self.budget)? {
            return Ok(true);
        }
        // The shared engine supplies this actual Call from owner.module(); no
        // externally supplied name, cloned operation or graph is accepted here.
        Ok(self
            .scope
            .function_named(self.owner, callee, self.budget)?
            .is_complete_and_pure())
    }
}

// Private, explicitly partial integration: only root/effect lookup and this
// adapter's fixed frame are paid. The shared engine's other legacy allocations
// and report payload remain unmetered, not a retained result or final authority.
pub(super) fn derive_with_canonical_effects_legacy_remainder_v19<'owner>(
    owner: &'owner Owner,
    scope: &CanonicalEffectScopeV19<'owner>,
    kernel_id: &KernelId,
    launch_extent: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
    budget: &mut Budget<'_>,
) -> Result<FormalMemoryObligationAnalysis, FormalEffectEngineErrorV19<CanonicalEffectErrorV19>> {
    let first = owner
        .module()
        .functions
        .first()
        .ok_or(CanonicalEffectErrorV19::ForeignOwner)?;
    scope.function(owner, first, budget)?;
    let floor = budget.storage();
    let consume = |budget: &mut Budget<'_>| {
        let mut root = None;
        for kernel in &owner.module().kernels {
            budget.charge_work(
                kernel
                    .id
                    .as_str()
                    .len()
                    .min(kernel_id.as_str().len())
                    .checked_add(2)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if &kernel.id == kernel_id {
                root = Some(kernel);
                break;
            }
        }
        let root = root.ok_or_else(|| FormalMemoryObligationError::MissingKernel {
            kernel: kernel_id.clone(),
        })?;
        // Authenticate the original scope/owner/ledger even for a call-free
        // root. Its summary need not be pure for deriving a partial report.
        scope.function_named(owner, &root.entry, budget)?;
        let mut reader = CanonicalEffectReaderV19 {
            owner,
            scope,
            budget,
        };
        derive_kernel_memory_obligations_with_effect_reader_v19(
            owner.module(),
            kernel_id,
            launch_extent,
            index_width,
            None,
            None,
            &mut reader,
        )
    };
    let scratch = size_of::<CanonicalEffectReaderV19<'_, '_, '_, '_>>()
        .checked_add(size_of::<Option<&Kernel>>())
        .and_then(|n| {
            n.checked_add(size_of::<
                std::thread::Result<
                    Result<
                        FormalMemoryObligationAnalysis,
                        FormalEffectEngineErrorV19<CanonicalEffectErrorV19>,
                    >,
                >,
            >())
        })
        .and_then(|n| n.checked_add(std::mem::size_of_val(&consume)))
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(floor, 1, 1, scratch, consume)
}

#[cfg(test)]
#[path = "effect_reader_v19_tests.rs"]
mod tests;
