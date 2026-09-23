//! Test-only genuine source-launch roster duplication for freshness refusal.
use super::*;

impl OrderedProgramObservationOwnerV32 {
    pub(crate) fn launch_roster_for_freshness_negative_v17(
        &self,
    ) -> Result<fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1, String> {
        let [typed_root] = self.bindings.typed_descriptor_roots.as_slice() else {
            return Err("source-launch duplicate typed root roster".into());
        };
        let semantic = self.materialized.semantic_ssa();
        if semantic.source_semantic().roots().len() != 1
            || self.materialized.source_launch().roots().len() != 1
        {
            return Err("source-launch duplicate source root bound".into());
        }
        // Preflight the closed, singleton retained-payload envelope before the
        // duplicate constructor. Existing source-import limits separately bound
        // its temporary singleton collections; this is not total allocator/RSS.
        let retained = std::mem::size_of_val(self.materialized.source_launch())
            .checked_add(std::mem::size_of_val(
                self.materialized.source_launch().roots(),
            ))
            .ok_or("source-launch duplicate retained payload overflow")?;
        if retained > 16 * 1024 {
            return Err("source-launch duplicate retained payload cap".into());
        }
        let launch = typed_root
            .source_launch()
            .ok_or("source-launch duplicate exact source contract absent")?;
        let inputs = [
            crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                typed_root.logical_name(),
                typed_root.kernel_binding_bytes(),
                launch,
            ),
        ];
        crate::production_ranked_projection_v1::source_launch_roster_for_ranked_inputs_v1(
            semantic, &inputs,
        )
        .map_err(|error| format!("source-launch genuine duplicate agreement: {error}"))
    }
}
