use super::*;
use std::sync::Mutex;

/// A single lowering invocation attached to the receipt's retained SSA owner.
/// This scope is not source authority; production retains its private source
/// seal in the callback that consumes it. There is no public constructor.
pub struct ProductionSemanticPhaseEmissionScopeV1<'a> {
    pub(in super::super) owner: &'a ProductionSemanticSsaOwnerV1,
    pub(in super::super) limits: ProductionSemanticKirLimitsV1,
    pub(in super::super) launch: &'a [RetainedRankedLaunchRootV1],
    pub(in super::super) contexts: &'a [ProductionKernelContextLoweringInputV1],
}

/// Lowered module and correspondence tied to the scope's retained SSA owner.
/// Phase results retain the original request and its unspent replay budget;
/// this result alone grants no source-proof, load or launch authority.
#[must_use]
pub struct ProductionSemanticPhaseEmissionResultV1<'a> {
    pub(in super::super) owner: &'a ProductionSemanticSsaOwnerV1,
    pub(in super::super) module: Module,
    pub(in super::super) correspondence: SemanticKirCorrespondenceV1,
    pub(in super::super) replay: Option<PhaseReplayV1>,
}

#[derive(Debug)]
pub(in super::super) struct PhaseReplayV1 {
    input: PhaseEmissionInputV1,
    /// Unspent source work, moved here once; no replay refunds its debits.
    remaining: Mutex<usize>,
}

impl PhaseReplayV1 {
    fn retain_after_emission(input: PhaseEmissionInputV1, work: &mut usize) -> PhaseResult<Self> {
        // The input's actual retained capacities were charged by CheckedRows.
        // This is the only additional inline replay state; the input is moved.
        spend(work, std::mem::size_of::<Mutex<usize>>())?;
        Ok(Self {
            input,
            remaining: Mutex::new(std::mem::take(work)),
        })
    }
}

impl<'a> ProductionSemanticPhaseEmissionScopeV1<'a> {
    /// Borrow the exact SSA owner captured by this lowering invocation.
    pub const fn semantic_ssa(&self) -> &'a ProductionSemanticSsaOwnerV1 {
        self.owner
    }

    /// Consume this scope through the ordinary lowerer, without a phase request.
    pub fn emit_without_phases(self) -> PhaseResult<ProductionSemanticPhaseEmissionResultV1<'a>> {
        let (module, correspondence) =
            lower_module(self.owner, self.limits, Some(self.launch), self.contexts)?;
        Ok(ProductionSemanticPhaseEmissionResultV1 {
            owner: self.owner,
            module,
            correspondence,
            replay: None,
        })
    }

    /// Check and consume the phase request against this scope's source/SSA owner.
    /// Success transfers the remaining work into the result's replay ledger,
    /// leaving `work` zero; failures retain every debit already incurred.
    pub fn emit(
        self,
        input: PhaseEmissionInputV1,
        work: &mut usize,
    ) -> PhaseResult<ProductionSemanticPhaseEmissionResultV1<'a>> {
        let (module, correspondence) = lower_module_with_phase(
            self.owner,
            self.limits,
            Some(self.launch),
            self.contexts,
            Some(&input),
            work,
        )?;
        Ok(ProductionSemanticPhaseEmissionResultV1 {
            owner: self.owner,
            module,
            correspondence,
            replay: Some(PhaseReplayV1::retain_after_emission(input, work)?),
        })
    }
}

/// Keep the same ledger exclusively borrowed until all replay temporaries and
/// comparisons are finished. Reentrant/concurrent and poisoned checks fail
/// closed. The callback returns no module or other retained replay allocation.
pub(in super::super) fn with_replay_work(
    phase: Option<&PhaseReplayV1>,
    verify: impl FnOnce(Option<(&PhaseEmissionInputV1, &mut usize)>) -> PhaseResult<()>,
) -> PhaseResult<()> {
    match phase {
        None => verify(None),
        Some(phase) => {
            let mut work = phase
                .remaining
                .try_lock()
                .map_err(|_| rejected("phase replay work is already borrowed or poisoned"))?;
            // Reject exhaustion before the unmodified outer validation work.
            spend(&mut work, 1)?;
            verify(Some((&phase.input, &mut work)))
        }
    }
}

pub(in super::super) fn replay(
    owner: &ProductionSemanticSsaOwnerV1,
    limits: ProductionSemanticKirLimitsV1,
    launch: Option<&[RetainedRankedLaunchRootV1]>,
    contexts: &[ProductionKernelContextLoweringInputV1],
    phase: Option<(&PhaseEmissionInputV1, &mut usize)>,
) -> PhaseResult<(Module, SemanticKirCorrespondenceV1)> {
    match phase {
        None => lower_module(owner, limits, launch, contexts),
        Some((input, work)) => {
            lower_module_with_phase(owner, limits, launch, contexts, Some(input), work)
        }
    }
}

#[cfg(test)]
#[path = "replay_budget_tests.rs"]
mod tests;
