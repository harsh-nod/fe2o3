//! Compiler-private replacement eligibility shared by bounded source promotion.
//! Adversarial callbacks and diagnostic observers remain test-only. No resume.

use super::*;
use crate::collector::source_census_v1::bitselect_feasibility::retained::{
    CANDIDATE_CAP, RetainedInput, bind_capture, require_baseline_profile,
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
#[cfg(test)]
use fe2o3_kernel_ir::Gfx942OrderedProgramRegistersV1;
use fe2o3_kernel_ir::{BlockId, FunctionBody};
use fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1;
use fe2o3_source_isa_observation::multilevel_authoring_v1::ordered_program_materialization_v1::render_bitselect_expression_v1;
use sha2::{Digest, Sha256};

#[cfg(test)]
#[path = "source_bitselect_candidate_fresh_v1_tests.rs"]
mod fresh;
#[path = "source_bitselect_promotion_pipeline_v1.rs"]
mod headless;
#[cfg(test)]
#[path = "source_local_order_pipeline_v1_tests.rs"]
mod local_order;
#[path = "source_local_order_join_v1.rs"]
mod local_order_join;
#[path = "source_local_order_recipe_adapter_v1.rs"]
mod local_order_recipe;
#[cfg(test)]
#[path = "source_bitselect_candidate_checks_v1_tests.rs"]
mod tests;

#[cfg(test)]
pub(super) fn registers() -> Gfx942OrderedProgramRegistersV1 {
    Gfx942OrderedProgramRegistersV1::new(4, 5, [0, 1, 2]).expect("fixed distinct fixture registers")
}

#[cfg(test)]
impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn publish_source_bitselect_candidate(
        self,
        mut input: RetainedInput,
        destination: &str,
        after_join: impl FnOnce() -> Result<(), String>,
    ) -> Result<Value, String> {
        let tcx = self.stage.tcx;
        let mut captured = capture(tcx, &self.stage.closure)?;
        bind_capture(tcx, &mut captured, &input)?;
        require_baseline_profile(&self.stage.closure, &mut captured.meter)?;
        // Exactly the existing normal baseline route, not a diagnostic inverse.
        let imported = self
            .import_semantic_mir()
            .map_err(|e| format!("source-candidate ordinary import: {e}"))?;
        let middle = imported
            .construct_semantic_middle_end()
            .map_err(|e| format!("source-candidate ordinary middle: {e}"))?;
        let ssa = middle
            .construct_semantic_ssa()
            .map_err(|e| format!("source-candidate ordinary SSA: {e}"))?;
        let materialized = ssa
            .materialize_target_neutral()
            .map_err(|e| format!("source-candidate ordinary materialization: {e}"))?;
        let ranked = materialized
            .verify_general_kernel_checks()
            .map_err(|e| format!("source-candidate ordinary ranked: {e}"))?;
        let neutral = ranked
            .attach_target_neutral_checks()
            .map_err(|e| format!("source-candidate ordinary attachment: {e}"))?;
        captured.recheck_original(tcx)?;
        bind_capture(tcx, &mut captured, &input)?;
        let joined = exact_join(&mut captured, &neutral.lowered)?;
        if joined.owner.canonical_kernel_ir_identity().version()
            != ProductionCanonicalKernelIrVersionV1::V8
        {
            return Err("source-candidate baseline requires the actual V8 owner".into());
        }
        let bounds = joined
            .function
            .kernel_entry()
            .and_then(|entry| entry.source_contract().launch());
        require_profile(
            neutral.bindings.rustc_target.profile() == ProductionAmdTargetProfileV1::Gfx942,
            bounds.and_then(|b| b.required()).map(|v| v.as_array()),
            bounds.and_then(|b| b.maximum()).map(|v| v.as_array()),
        )?;
        if joined.owner.semantic().semantic().functions().len() != 1
            || joined.owner.module().functions.len() != 1
            || captured.parameters.iter().map(|p| p.ordinal).ne([1, 2, 3])
        {
            return Err(
                "source-candidate requires one body and three ordered scalar parameters".into(),
            );
        }
        joined.require_attribution(&mut captured.meter)?;
        require_escape(
            joined
                .kir
                .body
                .as_ref()
                .ok_or("source-candidate body absent")?,
            joined.kir_block.id,
            joined.operations,
            joined.inputs,
            joined.results,
            &mut captured.meter,
        )?;
        let names = captured.parameters.each_ref().map(|p| p.name.as_str());
        // Shared renderer has its own fixed output cap; prepay its text payload
        // envelope here as well before it allocates. Diagnostic JSON is separate.
        captured.meter.storage(2 * 64 * 1024)?;
        captured.meter.scan(2 * 64 * 1024)?;
        let (program, expression) = render_bitselect_expression_v1(names, registers())
            .map_err(|e| format!("source-candidate render: {e}"))?;
        let bytes = replace_initializer(
            input.original(),
            captured.initializer.original_start,
            captured.initializer.original_end,
            &expression,
            &mut captured.meter,
        )?;
        let baseline = joined.observation(&captured)?;
        // Hook exists only for adversarial task-owned test files. It supplies no
        // identities, names, ranges, graph or acceptance assertions.
        after_join()?;
        input.publish(destination, &bytes)?;
        input.recheck()?;
        // All owners and the session witness remain live through publication.
        let result = json!({
            "stage": "same_session_source_candidate_published",
            "baseline": baseline,
            "candidate_sha256": <[u8;32]>::from(Sha256::digest(&bytes)),
            "candidate_bytes": bytes.len(),
            "descriptors": program.active_descriptors(),
            "own_scan_accounting": captured.meter,
            "io_envelope_accounting": input.io,
            "candidate_written": true, "original_unchanged": true,
            "fresh_frontend_admitted": false, "production_resume": false,
            "grants_artifact_or_launch_authority": false,
        });
        drop(joined);
        drop(neutral);
        Ok(result)
    }
}

fn require_profile(
    gfx942: bool,
    required: Option<[u32; 3]>,
    maximum: Option<[u32; 3]>,
) -> Result<(), String> {
    if !gfx942 {
        return Err("source-candidate requires authenticated gfx942 xnack-off wave64".into());
    }
    if required != Some([64, 1, 1]) || maximum != Some([64, 1, 1]) {
        return Err("source-candidate requires required and maximum 64x1x1 bounds".into());
    }
    Ok(())
}

struct Attribution {
    operations: [u32; 3],
    counts: [u8; 3],
}

impl Attribution {
    fn visit(
        &mut self,
        first: u32,
        count: u32,
        selected_statement: Option<usize>,
    ) -> Result<(), String> {
        let end = first
            .checked_add(count)
            .ok_or("source-candidate attribution interval overflow")?;
        for (index, operation) in self.operations.iter().enumerate() {
            if (first..end).contains(operation) {
                if selected_statement != Some(index) || count != 1 || self.counts[index] != 0 {
                    return Err(
                        "source-candidate ambiguous or foreign operation attribution".into(),
                    );
                }
                self.counts[index] = 1;
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<(), String> {
        if self.counts != [1; 3] {
            return Err("source-candidate selected attribution missing".into());
        }
        Ok(())
    }
}

impl JoinedBitselect<'_> {
    fn require_attribution(&self, meter: &mut ScanMeter) -> Result<(), String> {
        let correspondence = self.owner.correspondence();
        let mut census = Attribution {
            operations: self.operations,
            counts: [0; 3],
        };
        meter.rows(correspondence.statement_operation_spans().len())?;
        for row in correspondence.statement_operation_spans() {
            meter.scan(8)?;
            if row.kernel_ir_block() != self.kir_block.id {
                continue;
            }
            let selected = if row.correspondence_owner() == self.root
                && row.semantic_function() == self.root
                && row.semantic_block() == self.function.entry()
            {
                self.statements
                    .iter()
                    .position(|ordinal| *ordinal == row.statement_ordinal())
            } else {
                None
            };
            census.visit(
                row.first_operation_ordinal(),
                row.operation_count(),
                selected,
            )?;
        }
        meter.rows(correspondence.terminator_operation_spans().len())?;
        for row in correspondence.terminator_operation_spans() {
            meter.scan(8)?;
            if row.kernel_ir_block() == self.kir_block.id {
                census.visit(row.first_operation_ordinal(), row.operation_count(), None)?;
            }
        }
        meter.rows(correspondence.synthetic_operation_spans().len())?;
        for row in correspondence.synthetic_operation_spans() {
            meter.scan(8)?;
            if row.kernel_ir_block() == self.kir_block.id {
                census.visit(row.first_operation_ordinal(), row.operation_count(), None)?;
            }
        }
        census.finish()
    }
}

fn require_escape(
    body: &FunctionBody,
    block_id: BlockId,
    operations: [u32; 3],
    inputs: [ValueId; 3],
    results: [ValueId; 3],
    meter: &mut ScanMeter,
) -> Result<(), String> {
    // Indices come from the live source/semantic/KIR join, never a recipe or
    // caller coordinate. A supported pure source prefix may precede them.
    meter.scan(12)?;
    let boundary_error = "source-candidate boundary is not three contiguous entry operations";
    let entry = body.blocks.first().ok_or(boundary_error)?;
    let first = usize::try_from(operations[0]).map_err(|_| boundary_error)?;
    let end = operations[2].checked_add(1).ok_or(boundary_error)?;
    let end = usize::try_from(end).map_err(|_| boundary_error)?;
    if entry.id != block_id
        || operations[0].checked_add(1) != Some(operations[1])
        || operations[1].checked_add(1) != Some(operations[2])
        || end > entry.operations.len()
    {
        return Err(boundary_error.into());
    }
    let mut live_inputs = [false; 3];
    let mut live_output = false;
    meter.rows(body.blocks.len())?;
    let external =
        |value: ValueId, meter: &mut ScanMeter, output: &mut bool| -> Result<(), String> {
            meter.scan(1)?;
            if results[..2].contains(&value) {
                return Err("source-candidate intermediate escapes the selected graph".into());
            }
            *output |= value == results[2];
            Ok(())
        };
    for block in &body.blocks {
        meter.rows(block.operations.len())?;
        for (ordinal, operation) in block.operations.iter().enumerate() {
            if block.id == block_id && (first..end).contains(&ordinal) {
                let relative = ordinal - first;
                if !operation.has_complete_effect_summary() {
                    return Err("source-candidate selected effects are incomplete".into());
                }
                operation.try_visit_local_memory_effects_v1(|_| {
                    meter.scan(1)?;
                    Err::<(), String>("source-candidate selected graph has memory effects".into())
                })?;
                operation.kind.try_visit_operands(|value| {
                    meter.scan(1)?;
                    if let Some(index) = inputs.iter().position(|input| *input == value) {
                        live_inputs[index] = true;
                    } else if !results[..relative].contains(&value) {
                        return Err("source-candidate selected graph has an unbound live-in".into());
                    }
                    Ok::<(), String>(())
                })?;
            } else {
                operation.kind.try_visit_operands(|value| {
                    // A pre-boundary use cannot establish the selected live-out.
                    // The admitted owner also rejects this SSA use-before-definition.
                    if block.id == block_id && ordinal < first && value == results[2] {
                        meter.scan(1)?;
                        return Err("source-candidate output used before selected boundary".into());
                    }
                    external(value, meter, &mut live_output)
                })?;
            }
        }
        block
            .terminator
            .as_ref()
            .ok_or("source-candidate block terminator absent")?
            .try_visit_operands(|value| external(value, meter, &mut live_output))?;
    }
    if live_inputs != [true; 3] || !live_output {
        return Err("source-candidate graph is not three live-ins and one live-out".into());
    }
    Ok(())
}

fn replace_initializer(
    original: &[u8],
    start: u32,
    end: u32,
    expression: &str,
    meter: &mut ScanMeter,
) -> Result<Vec<u8>, String> {
    let text =
        std::str::from_utf8(original).map_err(|_| "source-candidate replacement requires UTF-8")?;
    let (start, end) = (start as usize, end as usize);
    if start >= end
        || end > original.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        return Err("source-candidate invalid original initializer range".into());
    }
    let length = original
        .len()
        .checked_sub(end - start)
        .and_then(|n| n.checked_add(expression.len()))
        .ok_or("source-candidate replacement length overflow")?;
    if length > CANDIDATE_CAP {
        return Err("source-candidate replacement byte limit".into());
    }
    meter.scan(length)?;
    meter.storage(length)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| "source-candidate replacement allocation")?;
    output.extend_from_slice(&original[..start]);
    output.extend_from_slice(expression.as_bytes());
    output.extend_from_slice(&original[end..]);
    Ok(output)
}
