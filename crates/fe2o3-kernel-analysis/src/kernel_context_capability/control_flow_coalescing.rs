//! Closed correspondence for deleting unconditional, argument-free CFG edges.
//! No instruction, value, branch decision, effect, or capability is rewritten.

use super::*;
use fe2o3_kernel_ir::{FunctionBody, Terminator};

const MAX_COALESCING_ITEMS: usize = 1_000_000;

impl KernelCapabilityPreservationAnalysisV1 {
    /// Accepts coordinate changes only after replaying the complete block merge
    /// relation against the exact source graph. This is not a general CFG,
    /// block-argument, dead-code, or instruction-motion equivalence checker.
    pub fn replay_control_flow_coalescing(
        &self,
        source: &Module,
        observed_input_epoch: u64,
        candidate_output_epoch: u64,
        committed_mutations: u64,
        candidate: &Module,
    ) -> Result<KernelCapabilityPreservationReplayV1, KernelCapabilityPreservationErrorV1> {
        self.check_input_epoch(observed_input_epoch)?;
        if !within_budget(source) || !within_budget(candidate) {
            return Err(KernelCapabilityPreservationErrorV1::ControlFlowCoalescingMismatch);
        }
        let canonical = VerifiedCanonicalKernelIrV13::from_module(source.clone())
            .map_err(|source| KernelCapabilityPreservationErrorV1::CandidateRejected { source })?;
        if canonical.identity() != &self.canonical_identity {
            return Err(KernelCapabilityPreservationErrorV1::SourceAnalysisMismatch);
        }
        // Validate once, then reuse this exact identity for the epoch receipt.
        let canonical_candidate = VerifiedCanonicalKernelIrV13::from_module(candidate.clone())
            .map_err(|source| KernelCapabilityPreservationErrorV1::CandidateRejected { source })?;
        if !exact_coalescing(source, candidate) {
            return Err(KernelCapabilityPreservationErrorV1::ControlFlowCoalescingMismatch);
        }
        // Exact correspondence retains every call, type, operation, and scoped
        // declaration. Only coordinates change; the captured closure is preserved
        // without recomputing either source or candidate capability facts.
        self.replay_identity(
            observed_input_epoch,
            candidate_output_epoch,
            committed_mutations,
            canonical_candidate.identity(),
        )
    }
}

fn within_budget(module: &Module) -> bool {
    let mut items = module.functions.len();
    for function in &module.functions {
        if let Some(body) = &function.body {
            items = items.saturating_add(body.blocks.len());
            if items > MAX_COALESCING_ITEMS {
                return false;
            }
            for block in &body.blocks {
                items = items.saturating_add(block.operations.len());
                if items > MAX_COALESCING_ITEMS {
                    return false;
                }
            }
        }
        if items > MAX_COALESCING_ITEMS {
            return false;
        }
    }
    true
}

fn exact_coalescing(source: &Module, candidate: &Module) -> bool {
    source.id == candidate.id
        && source.kernels == candidate.kernels
        && source.required_capabilities == candidate.required_capabilities
        && source.functions.len() == candidate.functions.len()
        && source
            .functions
            .iter()
            .zip(&candidate.functions)
            .all(|(before, after)| {
                before.id == after.id
                    && before.signature == after.signature
                    && before.role == after.role
                    && before.required_capabilities == after.required_capabilities
                    && match (&before.body, &after.body) {
                        (None, None) => true,
                        (Some(before), Some(after)) => exact_body_coalescing(before, after),
                        _ => false,
                    }
            })
}

fn exact_body_coalescing(before: &FunctionBody, after: &FunctionBody) -> bool {
    if before.parameters != after.parameters
        || before.blocks.first().map(|block| block.id) != after.blocks.first().map(|block| block.id)
    {
        return false;
    }
    let blocks = before
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let kept = after
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    if !before
        .blocks
        .iter()
        .filter(|block| kept.contains(&block.id))
        .map(|block| block.id)
        .eq(after.blocks.iter().map(|block| block.id))
    {
        return false;
    }
    let mut predecessors = BTreeMap::<BlockId, usize>::new();
    for block in &before.blocks {
        let Some(terminator) = &block.terminator else {
            return false;
        };
        for successor in terminator.successors() {
            *predecessors.entry(successor).or_default() += 1;
        }
    }
    let mut visited = BTreeSet::new();
    for candidate in &after.blocks {
        let Some(mut current) = blocks.get(&candidate.id).copied() else {
            return false;
        };
        if current.parameters != candidate.parameters {
            return false;
        }
        let mut cursor = 0_usize;
        loop {
            if !visited.insert(current.id) {
                return false;
            }
            let Some(end) = cursor.checked_add(current.operations.len()) else {
                return false;
            };
            if candidate.operations.get(cursor..end) != Some(current.operations.as_slice()) {
                return false;
            }
            cursor = end;
            match &current.terminator {
                Some(Terminator::Branch { target, arguments }) if !kept.contains(target) => {
                    let Some(next) = blocks.get(target).copied() else {
                        return false;
                    };
                    if !arguments.is_empty()
                        || !next.parameters.is_empty()
                        || predecessors.get(target) != Some(&1)
                    {
                        return false;
                    }
                    current = next;
                }
                terminator => {
                    if terminator != &candidate.terminator || cursor != candidate.operations.len() {
                        return false;
                    }
                    break;
                }
            }
        }
    }
    visited.len() == before.blocks.len()
}
