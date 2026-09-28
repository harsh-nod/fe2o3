//! Nominal storage-aware header over the same observed event relation.
use super::*;
use fe2o3_kernel_ir::{
    VerifiedCanonicalKernelIrIdentityV18 as Identity18,
    VerifiedCanonicalKernelIrModuleV18 as Owner18,
};

const DOMAIN: &[u8] = b"FE2O3/KIR-OPTIMIZATION-MAP/V18/POLICY-3/OBSERVED-V1\0";

/// Immutable observations from actual fixed execution. This map is not a
/// semantic, source, final-safety or executable certificate.
///
/// ```compile_fail
/// use fe2o3_pliron::{KirOptimizationMapPolicy3V18, KirOptimizationMapPolicy3V12};
/// fn erase(map: KirOptimizationMapPolicy3V18) -> KirOptimizationMapPolicy3V12 { map }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct KirOptimizationMapPolicy3V18 {
    data: MapData<Identity18>,
}

impl KirOptimizationMapPolicy3V18 {
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
            FixedPolicy::Checked3,
            digest,
            true,
        )
    }
    pub(crate) const fn neutral_data_v18(&self) -> &MapData<Identity18> {
        &self.data
    }
}

fn digest(data: &MapData<Identity18>, _policy: FixedPolicy) -> [u8; 32] {
    data.compute_digest_with_header(
        DOMAIN,
        [
            (data.input.digest(), data.input.canonical_length()),
            (data.output.digest(), data.output.canonical_length()),
        ],
    )
}

impl CaptureV12 {
    pub(crate) fn finish_policy3_v18(
        &self,
        input: &Owner18,
        output: &Owner18,
        roster: &LiveRosterV12,
        budget: &mut Budget<'_>,
    ) -> Result<(KirOptimizationMapPolicy3V18, usize)> {
        self.finish_header_data_admitted(
            input.module(),
            output.module(),
            [*input.identity(), *output.identity()],
            roster,
            budget,
            FixedPolicy::Checked3,
            digest,
            true,
        )
        .map(|(data, storage)| (KirOptimizationMapPolicy3V18 { data }, storage))
    }
}

#[cfg(test)]
#[path = "kir_optimization_map_v18_tests.rs"]
mod tests;
