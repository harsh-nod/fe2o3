//! Child of the existing candidate eligibility module; reuses its exact helpers.
//! No JSON/report re-admission and no adversarial callback in this interface.
use super::*;
use crate::source_bitselect_promotion_v1::{
    BitselectPromotionFailureV1 as Failure, BitselectPromotionRequestV1 as Request,
    FailurePhaseV1 as Phase, PublishedBitselectCandidateV1 as Published,
};

fn eligibility(error: String) -> Failure {
    Failure::before(Phase::Eligibility, error)
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn materialize_bitselect_source_v1(
        self,
        mut input: RetainedInput,
        request: &Request,
    ) -> Result<Published, Failure> {
        let tcx = self.stage.tcx;
        let mut captured = capture(tcx, &self.stage.closure).map_err(eligibility)?;
        bind_capture(tcx, &mut captured, &input).map_err(eligibility)?;
        if &captured.original_sha256 != request.expected_original_sha256() {
            return Err(eligibility(
                "source promotion expected source revision differs".into(),
            ));
        }
        require_baseline_profile(&self.stage.closure, &mut captured.meter).map_err(eligibility)?;
        // Consume the genuine existing transaction; never return an older stage
        // on refusal and never substitute a diagnostic owner for normal checks.
        let imported = self
            .import_semantic_mir()
            .map_err(|e| eligibility(format!("source-candidate ordinary import: {e}")))?;
        let middle = imported
            .construct_semantic_middle_end()
            .map_err(|e| eligibility(format!("source-candidate ordinary middle: {e}")))?;
        let ssa = middle
            .construct_semantic_ssa()
            .map_err(|e| eligibility(format!("source-candidate ordinary SSA: {e}")))?;
        let materialized = ssa
            .materialize_target_neutral()
            .map_err(|e| eligibility(format!("source-candidate ordinary materialization: {e}")))?;
        let ranked = materialized
            .verify_general_kernel_checks()
            .map_err(|e| eligibility(format!("source-candidate ordinary ranked: {e}")))?;
        let neutral = ranked
            .attach_target_neutral_checks()
            .map_err(|e| eligibility(format!("source-candidate ordinary attachment: {e}")))?;
        captured.recheck_original(tcx).map_err(eligibility)?;
        bind_capture(tcx, &mut captured, &input).map_err(eligibility)?;
        let joined = exact_join(&mut captured, &neutral.lowered).map_err(eligibility)?;
        if joined.owner.canonical_kernel_ir_identity().version()
            != ProductionCanonicalKernelIrVersionV1::V8
        {
            return Err(eligibility(
                "source-candidate baseline requires the actual V8 owner".into(),
            ));
        }
        let bounds = joined
            .function
            .kernel_entry()
            .and_then(|entry| entry.source_contract().launch());
        require_profile(
            neutral.bindings.rustc_target.profile() == ProductionAmdTargetProfileV1::Gfx942,
            bounds.and_then(|b| b.required()).map(|v| v.as_array()),
            bounds.and_then(|b| b.maximum()).map(|v| v.as_array()),
        )
        .map_err(eligibility)?;
        if joined.owner.semantic().semantic().functions().len() != 1
            || joined.owner.module().functions.len() != 1
            || captured.parameters.iter().map(|p| p.ordinal).ne([1, 2, 3])
        {
            return Err(eligibility(
                "source-candidate requires one body and three ordered scalar parameters".into(),
            ));
        }
        joined
            .require_attribution(&mut captured.meter)
            .map_err(eligibility)?;
        require_escape(
            joined
                .kir
                .body
                .as_ref()
                .ok_or_else(|| eligibility("source-candidate body absent".into()))?,
            joined.kir_block.id,
            joined.operations,
            joined.inputs,
            joined.results,
            &mut captured.meter,
        )
        .map_err(eligibility)?;
        captured.meter.storage(2 * 64 * 1024).map_err(eligibility)?;
        captured.meter.scan(2 * 64 * 1024).map_err(eligibility)?;
        let (_, expression) = render_bitselect_expression_v1(
            captured.parameters.each_ref().map(|p| p.name.as_str()),
            request.registers(),
        )
        .map_err(|e| eligibility(format!("source-candidate render: {e}")))?;
        let bytes = replace_initializer(
            input.original(),
            captured.initializer.original_start,
            captured.initializer.original_end,
            &expression,
            &mut captured.meter,
        )
        .map_err(eligibility)?;
        // Prepay the final bounded byte hash and fixed return payload before I/O.
        captured.meter.scan(bytes.len()).map_err(eligibility)?;
        captured
            .meter
            .storage(std::mem::size_of::<Published>())
            .map_err(eligibility)?;
        let candidate_sha256 = <[u8; 32]>::from(Sha256::digest(&bytes));
        let original_sha256 = captured.original_sha256;
        input
            .publish(request.candidate_path(), &bytes)
            .map_err(|e| Failure::after_attempt(Phase::Publication, e))?;
        #[cfg(test)]
        crate::source_bitselect_promotion_v1::live_test_support::after_publication(request);
        input
            .recheck()
            .map_err(|e| Failure::after_attempt(Phase::PostPublication, e))?;
        // Owner and witness stay live through publication and final recheck.
        drop(joined);
        drop(neutral);
        Ok(Published::observed(
            original_sha256,
            candidate_sha256,
            bytes.len(),
            request.registers(),
        ))
    }
}
