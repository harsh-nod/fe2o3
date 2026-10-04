//! Closed read-only ranked-policy access to the storage-aware emitter.

use super::*;
use crate::production_analysis::CanonicalRankedPolicyFailureV1 as Failure;

impl KirPlironGraphV18<'_> {
    #[cfg(test)]
    pub(crate) fn test_ranked_mutate_and_restore_v18(&self) {
        let context = &self.session.context;
        let root = self.session.operations[&self.root.identity];
        let attributes = root.deref(context).attributes.clone();
        root.deref_mut(context).attributes = attributes;
    }

    pub(crate) fn ranked_policy_epoch_v18(&self) -> Result<u64, Failure> {
        self.session
            .context
            .ir_mutation_attempt_epoch()
            .map(|epoch| epoch.value())
            .map_err(|_| Failure::Mutation)
    }

    pub(crate) fn check_ranked_policy_epoch_v18(&self, expected: u64) -> Result<(), Failure> {
        if self.ranked_policy_epoch_v18()? != expected {
            return Err(Failure::Mutation);
        }
        Ok(())
    }

    /// Visits only the existing emitted functions in original declaration order.
    /// No callback or Context escapes the crate's fixed policy implementation.
    /// Canonical declarations are explicit gaps, not empty successful functions.
    pub(crate) fn visit_ranked_policy_functions_v18(
        &self,
        expected_epoch: u64,
        budget: &mut Budget<'_>,
        mut consume: impl FnMut(usize, &NativeLifecycleIdentityAdmissionV18<'_>) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        self.validate_custody(budget)?;
        self.check_ranked_policy_epoch_v18(expected_epoch)?;
        budget.reserve_storage(std::mem::size_of_val(&consume))?;
        budget.reserve_storage(std::mem::size_of::<NativeLifecycleIdentityAdmissionV18<'_>>())?;
        let context = &self.session.context;
        let root = self.session.operations[&self.root.identity];
        let region = root.deref(context).get_region(0);
        let block = region
            .deref(context)
            .iter(context)
            .next()
            .ok_or(Failure::NativeSchema)?;
        let live_block = block.deref(context);
        let mut live_functions = live_block.iter(context);
        for (ordinal, source) in self.profile.owner().module().functions.iter().enumerate() {
            budget.charge_work(1)?;
            if source.body.is_none() {
                continue;
            }
            let pointer = live_functions.next().ok_or(Failure::NativeSchema)?;
            budget.charge_work(1)?;
            if self.origins.functions.get(&pointer) != Some(&ordinal) {
                return Err(Failure::ExactGraph);
            }
            let function =
                Operation::get_op::<FuncOp>(pointer, context).ok_or(Failure::NativeSchema)?;
            self.check_ranked_policy_epoch_v18(expected_epoch)?;
            let source_body = source.body.as_ref().ok_or(Failure::NativeSchema)?;
            budget.charge_work(source_body.blocks.len())?;
            let operations = source_body
                .blocks
                .iter()
                .try_fold(0usize, |count, block| {
                    count.checked_add(block.operations.len())?.checked_add(1)
                })
                .ok_or(ResourceError::Arithmetic)?;
            let identity = NativeLifecycleIdentityAdmissionV18 {
                context,
                function: &function,
                origins: &self.origins,
                ordinal,
                epoch: expected_epoch,
                operations,
            };
            consume(ordinal, &identity)?;
            self.check_ranked_policy_epoch_v18(expected_epoch)?;
        }
        budget.charge_work(1)?;
        if live_functions.next().is_some() {
            return Err(Failure::NativeSchema);
        }
        self.validate_custody(budget)?;
        self.check_ranked_policy_epoch_v18(expected_epoch)
    }
}

#[cfg(test)]
#[path = "kir_bridge_lifecycle_identity_v18_tests.rs"]
mod lifecycle_identity_tests;
