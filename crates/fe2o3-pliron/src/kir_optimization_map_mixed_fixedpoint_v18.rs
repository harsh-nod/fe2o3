//! Observed Policy11 map. Independent semantic adoption is still required.
use super::*;

#[derive(Debug, Eq, PartialEq)]
pub struct KirOptimizationMapMixedFixedpointV18 {
    data: MapData<Identity18>,
}
impl KirOptimizationMapMixedFixedpointV18 {
    pub const fn input_identity(&self) -> &Identity18 {
        &self.data.input
    }
    pub const fn output_identity(&self) -> &Identity18 {
        &self.data.output
    }
    pub const fn digest(&self) -> &[u8; 32] {
        self.data.digest()
    }
    pub fn comparison_work(&self) -> std::result::Result<usize, ResourceError> {
        self.data.comparison_work()
    }
    pub fn relations(&self) -> &[KirOptimizationRelationV12] {
        self.data.relations()
    }
    pub fn targets(&self, relation: &KirOptimizationRelationV12) -> Option<&[Endpoint]> {
        self.data.targets(relation)
    }
    pub fn synthesized_operations(&self) -> &[Coordinate] {
        self.data.synthesized_operations()
    }
    pub fn matches_execution(&self, report: &crate::PlironOptimizationReportV1) -> bool {
        self.data.matches_execution(report)
    }
    pub(crate) const fn neutral_data_v18(&self) -> &MapData<Identity18> {
        &self.data
    }
    pub fn check_against(
        &self,
        input: &Owner18,
        output: &Owner18,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        if self.data.input != *input.identity() || self.data.output != *output.identity() {
            return Err(KirOptimizationMapErrorV12::Identity);
        }
        self.data.check_modules_admitted(
            input.module(),
            output.module(),
            input.canonical_bytes().len(),
            budget,
            FixedPolicy::MixedFixedpoint11,
            mixed_fixedpoint_digest,
            true,
        )
    }
}
fn mixed_fixedpoint_digest(data: &MapData<Identity18>, _policy: FixedPolicy) -> [u8; 32] {
    data.compute_digest_with_header(
        FixedPolicy::MixedFixedpoint11.map_domain(),
        [
            (data.input.digest(), data.input.canonical_length()),
            (data.output.digest(), data.output.canonical_length()),
        ],
    )
}
impl CaptureV12 {
    pub(crate) fn finish_mixed_fixedpoint_v18(
        &self,
        input: &Owner18,
        output: &Owner18,
        roster: &LiveRosterV12,
        budget: &mut Budget<'_>,
    ) -> Result<(KirOptimizationMapMixedFixedpointV18, usize)> {
        self.finish_header_data_admitted(
            input.module(),
            output.module(),
            [*input.identity(), *output.identity()],
            roster,
            budget,
            FixedPolicy::MixedFixedpoint11,
            mixed_fixedpoint_digest,
            true,
        )
        .map(|(data, storage)| (KirOptimizationMapMixedFixedpointV18 { data }, storage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neutral_optimization_v1::storage_v18::tests::{LIMITS, SPACE, WORK, fixture, input};
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn mixed_fixedpoint_map_checks_terminal_round_order_and_epoch_independently() {
        let source = input(&fixture());
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(source.storage).unwrap();
        let value = crate::optimize_neutral_kernel_ir_mixed_fixedpoint_v18(
            &source.owner,
            LIMITS,
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(value.storage().retained_storage())
            .unwrap();
        let data = &value.map().data;
        let check = |passes: &[PassSpan]| {
            validate_lifecycle_for_policy(
                &data.nodes,
                &data.events,
                &data.terminal,
                passes,
                FixedPolicy::MixedFixedpoint11,
            )
        };
        assert!(check(&data.passes).is_ok());
        for fault in 0..5 {
            let mut passes = data.passes.clone();
            match fault {
                0 => passes.truncate(passes.len() - 5),
                1 => passes.swap(0, 1),
                2 => passes[0].output_epoch += 2,
                3 => {
                    let last = passes[passes.len() - 5..].to_vec();
                    passes.extend_from_slice(&last);
                }
                _ => passes.clear(),
            }
            assert!(
                matches!(check(&passes), Err(KirOptimizationMapErrorV12::Passes)),
                "fault {fault}"
            );
        }
        value.try_check_and_finish_v18(&mut budget).unwrap();
        assert_eq!(budget.storage(), source.storage);
    }

    #[test]
    fn mixed_fixedpoint_capture_round_admission_failure_is_sticky() {
        let limits =
            CaptureLimitsV12::for_policy_bytes(128, FixedPolicy::MixedFixedpoint11).unwrap();
        let capture = CaptureV12::new_for_policy_admitted(
            limits,
            &Vec::new(),
            FixedPolicy::MixedFixedpoint11,
            Some(1),
        )
        .unwrap();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        let mut ledger = crate::fixed_policy_v3::CseLedger::new(&mut budget);
        assert_eq!(
            capture.admit_fixedpoint_round(&mut ledger),
            Err(KirOptimizationMapErrorV12::Passes)
        );
        assert_eq!(capture.failure(), Some(KirOptimizationMapErrorV12::Passes));
        assert_eq!(
            capture.require_policy(FixedPolicy::MixedFixedpoint11),
            Err(KirOptimizationMapErrorV12::Passes)
        );
        assert_eq!(ledger.finish(), Ok(0));
    }
}
