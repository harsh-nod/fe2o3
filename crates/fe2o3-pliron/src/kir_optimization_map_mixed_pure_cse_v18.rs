//! Nominal V18 header for actual fixed mixed pure-CSE execution.
use super::*;

const MIXED_PURE_CSE_DOMAIN: &[u8] =
    b"FE2O3/KIR-OPTIMIZATION-MAP/V18/POLICY-10/MIXED-PURE-CSE-V1\0";

/// Actual execution observations, not semantic or publication authority.
#[derive(Debug, Eq, PartialEq)]
pub struct KirOptimizationMapMixedPureCseV18 {
    data: MapData<Identity18>,
}

impl KirOptimizationMapMixedPureCseV18 {
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
            FixedPolicy::MixedPureCse10,
            mixed_pure_cse_digest,
            true,
        )
    }
}

fn mixed_pure_cse_digest(data: &MapData<Identity18>, _policy: FixedPolicy) -> [u8; 32] {
    data.compute_digest_with_header(
        MIXED_PURE_CSE_DOMAIN,
        [
            (data.input.digest(), data.input.canonical_length()),
            (data.output.digest(), data.output.canonical_length()),
        ],
    )
}

impl CaptureV12 {
    pub(crate) fn finish_mixed_pure_cse_v18(
        &self,
        input: &Owner18,
        output: &Owner18,
        roster: &LiveRosterV12,
        budget: &mut Budget<'_>,
    ) -> Result<(KirOptimizationMapMixedPureCseV18, usize)> {
        self.finish_header_data_admitted(
            input.module(),
            output.module(),
            [*input.identity(), *output.identity()],
            roster,
            budget,
            FixedPolicy::MixedPureCse10,
            mixed_pure_cse_digest,
            true,
        )
        .map(|(data, storage)| (KirOptimizationMapMixedPureCseV18 { data }, storage))
    }
}
