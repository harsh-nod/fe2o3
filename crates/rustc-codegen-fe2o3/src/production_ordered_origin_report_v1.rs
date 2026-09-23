//! Closed, bounded inert origin report. It cannot reconstruct any compiler owner.
use crate::collector::ordered_origin_v1::OrderedOriginCaptureV1;
use crate::production_pipeline::ordered_program_diagnostic_v32::OrderedProgramObservationOwnerV32;
use fe2o3_lower_mir_kernel::ProductionOrderedProgramInspectionV1;
use fe2o3_mir_model::semantic_mir_v1::SemanticSourceOriginV1;
use serde::Serialize;
use std::io::{self, Write};

pub(crate) const MAX_ORIGIN_REPORT_BYTES_V1: usize = 16 * 1024;
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct OrderedOriginReportV1 {
    schema: &'static str,
    diagnostic_only: bool,
    authenticates_source: bool,
    authenticates_compiler_execution: bool,
    grants_proof_resume_artifact_launch_authority: bool,
    stage: &'static str,
    target: &'static str,
    wave_width: u8,
    canonical_version: u16,
    canonical_sha256: String,
    canonical_bytes: usize,
    semantic_version: u16,
    semantic_sha256: String,
    source_inventory_sha256: String,
    source_preflight_sha256: String,
    root_function_sha256: String,
    root_monomorphization_sha256: String,
    rustc_mir_body_sha256: String,
    rustc_mir_block: u32,
    semantic_block_identity: String,
    semantic_function: u32,
    semantic_block: u32,
    kir_roster_coordinate: [u32; 3],
    kir_raw_block: u32,
    declared_source_ids: DeclaredSourceIds,
    origin_association: &'static str,
    origin_scope: &'static str,
    expansion: Origin,
    call_site: Origin,
    expansion_chain_sha256: String,
    expansion_depth: usize,
    macro_expansion_frames: &'static str,
    fine_step_origins: &'static str,
    declared_instructions: Vec<Instruction>,
    declared_register_roles: DeclaredRegisterRoles,
    compiler_policy_identity: &'static str,
    source_map_identity: &'static str,
    edit_epoch: &'static str,
    schedule_identity: &'static str,
    final_artifact: &'static str,
    physical_register_values: &'static str,
    physical_register_lifetimes: &'static str,
    limits: Limits,
}
#[derive(Serialize)]
struct DeclaredSourceIds {
    frontend_unit: String,
    function: String,
    contract: String,
    statement: String,
}
#[derive(Serialize)]
struct DeclaredRegisterRoles {
    scratch: u8,
    output: u8,
    inputs: [u8; 3],
}
#[derive(Serialize)]
struct Origin {
    file_identity: String,
    byte_start: u64,
    byte_end: u64,
    line_start: u32,
    column_start: u32,
    line_end: u32,
    column_end: u32,
}
#[derive(Serialize)]
struct Instruction {
    ordinal: u8,
    descriptor: u16,
    source_association: &'static str,
}
#[derive(Serialize)]
struct Limits {
    source_reobservation_work: usize,
    source_reobservation_work_used: usize,
    maximum_expansion_depth: usize,
    report_bytes: usize,
    rustc_internal_allocations_accounted: bool,
}

impl OrderedOriginReportV1 {
    pub(crate) fn from_live_owner(
        owner: &OrderedProgramObservationOwnerV32,
        view: &ProductionOrderedProgramInspectionV1<'_>,
        capture: OrderedOriginCaptureV1,
    ) -> Result<Self, &'static str> {
        let provenance = capture.source.provenance();
        if provenance != view.source_provenance()
            || capture.function != view.ordered_program().source().function
            || view.canonical_identity() != owner.materialized().executable().canonical().identity()
        {
            return Err("origin report live owner binding mismatch");
        }
        let count = view.declared_program().count();
        if !(1..=16).contains(&count) {
            return Err("origin declared program bound");
        }
        let mut instructions = Vec::new();
        instructions
            .try_reserve_exact(usize::from(count))
            .map_err(|_| "origin instruction report allocation")?;
        for ordinal in 0..count {
            instructions.push(Instruction {
                ordinal,
                descriptor: view.declared_program().descriptors()[usize::from(ordinal)],
                source_association: "whole_ordered_region_only",
            });
        }
        let source = view.ordered_program().source();
        let registers = view.ordered_program().registers();
        let coordinate = view.coordinate();
        let (inventory, preflight) = owner.authenticated_source_identities();
        Ok(Self {
            schema: "fe2o3-diagnostic-ordered-program-origin-v1",
            diagnostic_only: true,
            authenticates_source: false,
            authenticates_compiler_execution: false,
            grants_proof_resume_artifact_launch_authority: false,
            stage: "pre_ranked_diagnostic",
            target: view.declared_target(),
            wave_width: 64,
            canonical_version: 17,
            canonical_sha256: hex(view.canonical_identity().digest()),
            canonical_bytes: owner
                .materialized()
                .executable()
                .canonical()
                .canonical_bytes()
                .len(),
            semantic_version: 32,
            semantic_sha256: hex(view.semantic_sha256()),
            source_inventory_sha256: hex(&inventory),
            source_preflight_sha256: hex(&preflight),
            root_function_sha256: hex(&capture.function),
            root_monomorphization_sha256: hex(&capture.monomorphization),
            rustc_mir_body_sha256: hex(&capture.mir_body),
            rustc_mir_block: capture.mir_block,
            semantic_block_identity: hex(&capture.block_identity),
            semantic_function: view.semantic_function().index(),
            semantic_block: view.semantic_block().index(),
            kir_roster_coordinate: [
                coordinate.block.function.0,
                coordinate.block.block,
                coordinate.operation,
            ],
            kir_raw_block: view.kernel_ir_block().0,
            declared_source_ids: DeclaredSourceIds {
                frontend_unit: hex(&source.frontend_unit),
                function: hex(&source.function),
                contract: hex(&source.contract),
                statement: hex(&source.statement),
            },
            origin_association: "retained_semantic_correspondence_and_live_rustc_block_identity",
            origin_scope: "whole_ordered_region",
            expansion: Origin::new(
                provenance
                    .expansion()
                    .ok_or("origin expansion unavailable")?,
            ),
            call_site: Origin::new(
                provenance
                    .call_site()
                    .ok_or("origin callsite unavailable")?,
            ),
            expansion_chain_sha256: hex(&capture.source.expansion_chain_sha256()),
            expansion_depth: capture.source.expansion_depth(),
            macro_expansion_frames: "unavailable_only_digest_and_depth_retained",
            fine_step_origins: "unavailable_flat_descriptor_program",
            declared_instructions: instructions,
            declared_register_roles: DeclaredRegisterRoles {
                scratch: registers.scratch(),
                output: registers.output(),
                inputs: registers.inputs(),
            },
            compiler_policy_identity: "unavailable_in_this_diagnostic",
            source_map_identity: "unavailable_no_debug_map_exported",
            edit_epoch: "unavailable",
            schedule_identity: "unavailable",
            final_artifact: "unavailable_no_native_compilation",
            physical_register_values: "unavailable",
            physical_register_lifetimes: "unavailable",
            limits: Limits {
                source_reobservation_work: crate::collector::ordered_origin_v1::MAX_ORIGIN_WORK_V1,
                source_reobservation_work_used: capture.work_used,
                maximum_expansion_depth:
                    crate::collector::ordered_origin_v1::MAX_ORIGIN_EXPANSION_DEPTH_V1,
                report_bytes: MAX_ORIGIN_REPORT_BYTES_V1,
                rustc_internal_allocations_accounted: false,
            },
        })
    }

    pub(crate) fn bytes(&self) -> Result<Vec<u8>, &'static str> {
        encode_bounded(self)
    }
}
impl Origin {
    fn new(source: SemanticSourceOriginV1) -> Self {
        let (byte_start, byte_end) = source.byte_range();
        let (line_start, column_start) = source.start_coordinate();
        let (line_end, column_end) = source.end_coordinate();
        Self {
            file_identity: hex(source.file().as_bytes()),
            byte_start,
            byte_end,
            line_start,
            column_start,
            line_end,
            column_end,
        }
    }
}
fn hex(bytes: &[u8; 32]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for &byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    result
}
struct LimitedBytes(Vec<u8>);
impl Write for LimitedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_ORIGIN_REPORT_BYTES_V1.saturating_sub(self.0.len()) {
            return Err(io::Error::other("origin report byte bound exceeded"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn encode_bounded(value: &impl Serialize) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_ORIGIN_REPORT_BYTES_V1)
        .map_err(|_| "origin report allocation failed")?;
    let mut output = LimitedBytes(bytes);
    serde_json::to_writer(&mut output, value)
        .map_err(|_| "origin report encoding or byte bound")?;
    Ok(output.0)
}

#[cfg(test)]
#[path = "production_ordered_origin_report_v1_tests.rs"]
mod tests;
