//! Opt-in diagnostic handoff; ordinary production owners and wires are unchanged.
use super::ordered_program_diagnostic_v32::OrderedProgramObservationOwnerV32;
use super::*;
use crate::collector::ordered_origin_v1::OrderedOriginRootV1;
use crate::production_ordered_origin_report_v1::OrderedOriginReportV1;

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_ordered_program_origin_v1(
        self,
    ) -> Result<(OrderedProgramObservationOwnerV32, OrderedOriginReportV1), String> {
        let (owner, report, _) = self.observe_ordered_origin_inner(false)?;
        Ok((owner, report))
    }

    /// Opt-in actual same-session producer; no public exporter or wire changes.
    pub(crate) fn observe_ordered_program_macro_frames_v1(
        self,
    ) -> Result<
        (
            OrderedProgramObservationOwnerV32,
            OrderedOriginReportV1,
            crate::collector::ordered_origin_v1::macro_frames::OrderedMacroFramesV1,
        ),
        String,
    > {
        let (owner, report, frames) = self.observe_ordered_origin_inner(true)?;
        Ok((owner, report, frames.ok_or("macro frame capture missing")?))
    }

    fn observe_ordered_origin_inner(
        self,
        include_macro_frames: bool,
    ) -> Result<
        (
            OrderedProgramObservationOwnerV32,
            OrderedOriginReportV1,
            Option<crate::collector::ordered_origin_v1::macro_frames::OrderedMacroFramesV1>,
        ),
        String,
    > {
        let seed = OrderedOriginRootV1::from_collected(&self.stage.closure)?;
        let tcx = self.stage.tcx;
        let owner = self.observe_ordered_program_v32()?;
        let materialized = owner.materialized();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_048_576);
        let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        budget
            .reserve_storage(materialized.executable_storage().retained_storage())
            .map_err(|_| "origin retained canonical storage bound")?;
        budget
            .reserve_storage(materialized.call_correspondence_storage())
            .map_err(|_| "origin retained correspondence storage bound")?;
        let (view, receipt) = materialized
            .inspect_ordered_program_v1(
                materialized.executable().canonical().identity(),
                None,
                fe2o3_lower_mir_kernel::ProductionOrderedProgramInspectionLimitsV1::default(),
                &mut budget,
            )
            .map_err(|_| "origin retained semantic correspondence unavailable")?;
        budget
            .reserve_storage(receipt.retained_storage())
            .map_err(|_| "origin borrowed inspection storage bound")?;
        let semantic = materialized.semantic_ssa().source_semantic();
        let function = semantic
            .functions()
            .get(view.semantic_function().index() as usize)
            .ok_or("origin semantic function missing")?;
        let block = function
            .blocks()
            .get(view.semantic_block().index() as usize)
            .ok_or("origin semantic block missing")?;
        let (captured, frames) = if include_macro_frames {
            budget
                .reserve_storage(
                    crate::collector::ordered_origin_v1::macro_frames::MACRO_LOGICAL_PREPAY_V1,
                )
                .map_err(|_| "origin macro logical storage bound")?;
            let (captured, frames) =
                seed.capture_with_macro_frames(tcx, &view, function.identity(), block.identity())?;
            (captured, Some(frames))
        } else {
            (
                seed.capture(tcx, &view, function.identity(), block.identity())?,
                None,
            )
        };
        let report = OrderedOriginReportV1::from_live_owner(&owner, &view, captured)?;
        Ok((owner, report, frames))
    }
}
