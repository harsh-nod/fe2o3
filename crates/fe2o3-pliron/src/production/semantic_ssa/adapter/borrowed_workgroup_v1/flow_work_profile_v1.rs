//! Fixed-size observations of existing successful charges; no new graph walks.
use super::ProductionSemanticSsaErrorV1;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(usize)]
pub(super) enum Stage {
    #[default]
    Facts,
    Candidates,
    Uses,
    Components,
    OwnerRoots,
    Cfg,
    OwnerChanges,
    Loans,
    Paths,
    Descendants,
    Transfers,
}

impl Stage {
    fn label(self) -> &'static str {
        match self {
            Self::Facts => "facts",
            Self::Candidates => "candidates",
            Self::Uses => "uses",
            Self::Components => "components",
            Self::OwnerRoots => "owner-roots",
            Self::Cfg => "cfg",
            Self::OwnerChanges => "owner-changes",
            Self::Loans => "loans",
            Self::Paths => "paths",
            Self::Descendants => "descendants",
            Self::Transfers => "transfers",
        }
    }
}

#[derive(Default)]
pub(super) struct Profile {
    pub(super) stage: Stage,
    pub(super) uses: super::uses_observation_v1::Observation,
    units: [usize; 11],
    pub(super) proof_calls: usize,
}

impl Profile {
    pub(super) fn finish_uses(&mut self, remaining: usize) {
        self.uses.complete(self.units[Stage::Uses as usize], remaining);
    }

    pub(super) fn charged(&mut self, work: usize) {
        // Successful charges across all stages cannot exceed the fixed budget.
        self.units[self.stage as usize] += work;
        if self.stage == Stage::Uses { self.uses.charged(work); }
    }

    pub(super) fn failure(
        &self,
        remaining: usize,
        requested: usize,
        error: ProductionSemanticSsaErrorV1,
    ) -> ProductionSemanticSsaErrorV1 {
        if self.stage == Stage::Uses {
            self.uses.emit(self.units[Stage::Uses as usize], remaining, requested);
        }
        ProductionSemanticSsaErrorV1::BorrowFlowWork {
            stage: self.stage.label(),
            phase_work_units: self.units,
            ordered_proof_calls: self.proof_calls,
            remaining_work_units: remaining,
            requested_work_units: requested,
            error: Box::new(error),
        }
    }
}

impl Drop for Profile {
    fn drop(&mut self) {
        self.uses.aborted(self.units[Stage::Uses as usize]);
    }
}

#[cfg(test)]
pub(super) fn original_error_for_test(
    error: ProductionSemanticSsaErrorV1,
) -> ProductionSemanticSsaErrorV1 {
    match error {
        ProductionSemanticSsaErrorV1::BorrowFlowWork { error, .. } => *error,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::super::Budget;
    use super::*;

    fn budget(limit: usize) -> Budget {
        Budget {
            remaining: limit,
            limit,
            profile: Profile::default(),
        }
    }

    #[test]
    fn exact_original_work_gate_and_remaining_are_retained() {
        for limit in 0..20 {
            let mut observed = budget(limit);
            let mut remaining = limit;
            for work in [0, 1, 3, 8, 0, 5] {
                let expected = remaining.checked_sub(work);
                match (observed.charge(work), expected) {
                    (Ok(()), Some(next)) => remaining = next,
                    (
                        Err(ProductionSemanticSsaErrorV1::BorrowFlowWork {
                            phase_work_units,
                            remaining_work_units,
                            requested_work_units,
                            error,
                            ..
                        }),
                        None,
                    ) => {
                        assert_eq!(remaining_work_units, remaining);
                        assert_eq!(requested_work_units, work);
                        assert_eq!(phase_work_units.iter().sum::<usize>(), limit - remaining);
                        assert_eq!(
                            *error,
                            ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                                resource: super::super::SsaPlannerResourceV1::WorkUnits,
                                required: limit + 1,
                                limit,
                            }
                        );
                        break;
                    }
                    other => panic!("changed work gate: {other:?}"),
                }
                assert_eq!(observed.remaining, remaining);
            }
        }
    }

    #[test]
    fn nested_stage_error_records_failure_before_restoring_caller_stage() {
        let mut observed = budget(10);
        observed.charge(2).unwrap();
        let failure = observed
            .scoped(Stage::Loans, |budget| {
                budget.charge(3)?;
                budget.scoped(Stage::Paths, |budget| budget.charge(6))
            })
            .unwrap_err();
        assert_eq!(observed.profile.stage, Stage::Facts);
        let ProductionSemanticSsaErrorV1::BorrowFlowWork {
            stage,
            phase_work_units,
            ..
        } = failure
        else {
            panic!("missing phase observation")
        };
        assert_eq!(stage, "paths");
        assert_eq!(phase_work_units, [2, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0]);
    }
}
