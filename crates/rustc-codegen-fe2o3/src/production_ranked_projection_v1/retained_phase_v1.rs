//! One original projection ledger, retained through conditional target admission.
use super::*;
use crate::production_reference_effect_join_v2::conditional::ReferenceRootV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as OwnedBudget,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};

type Error = ProductionRankedVerificationErrorV1;

#[cfg(test)]
#[path = "retained_proof_observation_v1.rs"]
pub(crate) mod observation;

// Private and non-Clone. No restored counters, replacement budgets, or persisted
// work-identity tokens. All subsequent views borrow this same original account.
pub(super) struct RetainedProjectionPhaseV1 {
    ledger: Box<OwnedBudget>,
    floor: usize,
    poisoned: bool,
}
impl RetainedProjectionPhaseV1 {
    pub(super) fn new(ledger: Box<OwnedBudget>) -> Self {
        #[cfg(test)]
        observation::phase_retained(&ledger);
        Self {
            floor: ledger.storage(),
            ledger,
            poisoned: false,
        }
    }

    fn with_budget<R>(
        &mut self,
        consume: impl FnOnce(&mut Budget<'_>) -> Result<R, Error>,
    ) -> Result<R, Error> {
        use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
        #[cfg(test)]
        let before = observation::Resources::from_owned(&self.ledger);
        if self.poisoned || self.ledger.storage() < self.floor {
            #[cfg(test)]
            observation::phase_finished(before, &self.ledger, observation::Outcome::Accounting);
            return Err(Error::ConditionalResource(Resource::Accounting));
        }
        let floor = self.floor;
        let mut poisoned = false;
        let result = self.ledger.with_budget(|budget| {
            let account = budget.work_ledger_identity_v1();
            let result = catch_unwind(AssertUnwindSafe(|| {
                budget.charge_work(1).map_err(Error::ConditionalResource)?;
                consume(budget)
            }));
            poisoned = budget.work_ledger_identity_v1() != account || budget.storage() < floor;
            result
        });
        self.poisoned |= poisoned;
        #[cfg(test)]
        observation::phase_finished(
            before,
            &self.ledger,
            match &result {
                Err(_) => observation::Outcome::Unwind,
                Ok(_) if self.poisoned => observation::Outcome::Accounting,
                Ok(Err(_)) => observation::Outcome::Rejected,
                Ok(Ok(_)) => observation::Outcome::Accepted,
            },
        );
        match result {
            Ok(result) if !self.poisoned => result,
            Ok(result) => {
                drop(result);
                Err(Error::ConditionalResource(Resource::Accounting))
            }
            Err(payload) => resume_unwind(payload),
        }
    }
}

impl Drop for RetainedProjectionPhaseV1 {
    fn drop(&mut self) {
        // The program declares this field after its roots, and consuming paths
        // keep it until after destroying/transferring all phase-owned evidence.
        // The account dies here; work and denial history are never reset.
        #[cfg(test)]
        let before = observation::Resources::from_owned(&self.ledger);
        if !self.poisoned {
            self.ledger.with_budget(|budget| {
                let _ = budget.release_storage(budget.storage());
            });
        }
        #[cfg(test)]
        observation::phase_dropped(before, &self.ledger, self.poisoned);
    }
}

impl ProductionRankedSemanticProgramV1 {
    pub(crate) fn has_conditional_roots_v1(&self) -> bool {
        self.roots
            .iter()
            .any(|root| matches!(root.verification, ReferenceRootV1::Conditional(_)))
    }

    /// Replays retained conditional roots on the original account and returns
    /// the same owning phase. Failure destroys the consumed roster. This neither
    /// admits formal memory nor changes the existing typed target refusal.
    pub(crate) fn replay_conditional_roots_v1(
        mut self,
        references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    ) -> Result<Self, Error> {
        let roots = std::mem::take(&mut self.roots);
        let source = &self.materialized;
        let (roots, first) = self.phase.with_budget(|budget| {
            // Prepay new root/roster traversal and simultaneous result storage.
            let count = roots.len();
            budget
                .charge_work(
                    count
                        .checked_mul(8)
                        .ok_or(Error::ConditionalResource(Resource::Arithmetic))?,
                )
                .map_err(Error::ConditionalResource)?;
            let scratch = count
                .checked_mul(
                    std::mem::size_of::<ProductionRankedRootProgramV1>()
                        + std::mem::size_of::<RankedRootSemanticBindingRecordV1<'_>>(),
                )
                .ok_or(Error::ConditionalResource(Resource::Arithmetic))?;
            budget
                .reserve_storage(scratch)
                .map_err(Error::ConditionalResource)?;
            source
                .semantic_ssa()
                .verify_replay()
                .map_err(Error::SemanticSsa)?;
            let bindings = roots
                .iter()
                .map(ranked_root_program_semantic_binding_v1)
                .collect::<Vec<_>>();
            validate_ranked_roster_semantic_bindings_v1(
                source.semantic_ssa().source_owner(),
                &bindings,
            )?;
            drop(bindings);
            let mut first = None;
            let mut replayed = Vec::new();
            replayed
                .try_reserve_exact(count)
                .map_err(|_| Error::ConditionalResource(Resource::Allocation))?;
            if replayed.capacity() != count {
                return Err(Error::ConditionalResource(Resource::Accounting));
            }
            for mut root in roots.into_vec() {
                root.verification = match root.verification {
                    ReferenceRootV1::Conditional(conditional) => {
                        first.get_or_insert(root.semantic_root.index());
                        let mut selected = None;
                        for reference in references.as_slice() {
                            budget
                                .charge_work(
                                    reference
                                        .logical_kernel_name
                                        .len()
                                        .checked_add(root.logical_name.len())
                                        .and_then(|n| n.checked_add(1))
                                        .ok_or(Error::ConditionalResource(Resource::Arithmetic))?,
                                )
                                .map_err(Error::ConditionalResource)?;
                            if reference.logical_kernel_name == root.logical_name {
                                if selected.replace(reference).is_some() {
                                    return Err(Error::RosterMetadata(
                                        "ambiguous conditional CPU binding",
                                    ));
                                }
                            }
                        }
                        let reference = selected
                            .ok_or(Error::RosterMetadata("missing conditional CPU binding"))?;
                        ReferenceRootV1::Conditional(
                            conditional
                                .replay_for_target_v1(source, reference, budget)
                                .map_err(Error::ConditionalReplay)?,
                        )
                    }
                    ReferenceRootV1::Pending(_) => {
                        return Err(Error::RosterMetadata(
                            "conditional target contains pending proof request",
                        ));
                    }
                    ordinary @ ReferenceRootV1::Ordinary { .. } => ordinary,
                };
                replayed.push(root);
            }
            let replayed = replayed.into_boxed_slice();
            // The input array has been consumed; the retained array replaces it.
            budget
                .release_storage(scratch)
                .map_err(Error::ConditionalResource)?;
            Ok((replayed, first))
        })?;
        self.roots = roots;
        first.ok_or(Error::RosterMetadata(
            "conditional replay has no conditional roots",
        ))?;
        Ok(self)
    }
}

#[cfg(test)]
#[path = "retained_phase_v1_tests.rs"]
mod tests;
