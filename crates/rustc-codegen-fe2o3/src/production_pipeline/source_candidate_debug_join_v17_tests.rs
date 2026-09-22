//! Private same-owner diagnostic catalog/capture join.
//! No compiler owner, subject, source authority or executable is reconstructed.
//! Reuses the exact checked source-map producer; no public transport is added.

use super::{
    CollectedRustStage, ExactDebugSourceOwnerV1, ExactDebugSourceProjectionV1,
    ProductionCompilation, ordered_program_diagnostic_v32::OrderedProgramObservationOwnerV32,
};
use crate::collector::source_census_v1::bitselect_feasibility::retained::RetainedInput;
use fe2o3_kernel_ir::{AccessMode, Gfx942OrderedProgramRegistersV1, OperationKind, ScalarType};
use fe2o3_kir_debugger::{
    DebugInspectionUnavailableV1, DebugInspectionV1, DebugKirIdentityV1, DebugSessionV1,
    DebugSourceCatalogV1, DebugSourceFileV1, DebugSourceSiteV1, DebugSourceSpanV1,
    DebugTranscriptCompletenessV1, DebugTranscriptV1, DebugWaveWidthV1, DebuggerErrorV1,
    DebuggerLimitsV1, capture_debugger_run_v1,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    ScalarBitsV1, SharedBufferV1, SimulationArgumentV1, SimulationDebugAllocationV1,
    SimulationDebugBindingV1, SimulationDebugCaptureLimitsV1, SimulationDebugCheckpointPhaseV1,
    SimulationDebugFrameV1, SimulationDebugRecordKindV1, SimulationDebugRecordV1,
    SimulationDebugSiteV1, SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};
use serde_json::{Value, json};

#[path = "source_candidate_inspection_join_v17_tests.rs"]
mod inspection;

const MAX_RECORDS: usize = 8192;
const MAX_VALUES: usize = 131_072;
const MAX_MEMORY: usize = 1024 * 1024;
const MAX_FILES: usize = 16;
const MAX_SITES: usize = 4096;
const MAX_SPANS: usize = 8192;
const LOGICAL_ENVELOPE: usize = 128 * 1024 * 1024;
const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();

// Fixed cardinality, not an allocator/RSS accounting assertion. Reserve before
// any projection, capture or clone. CUMULATIVE two-session construction/clone
// roster: 2 captures + 2 positive sessions + 3 negatives = 7 transcripts;
// 2 catalogs + 2 positive bindings + 3 negatives + 1 recovery = 8 catalogs.
fn prepay_fixed_envelope() -> Result<usize, String> {
    use std::mem::size_of;
    let transcript = MAX_RECORDS
        .checked_mul(
            size_of::<SimulationDebugRecordV1>()
                + size_of::<SimulationDebugFrameV1>()
                + size_of::<SimulationDebugAllocationV1>(),
        )
        .and_then(|n| n.checked_add(MAX_VALUES * size_of::<SimulationDebugBindingV1>()))
        .and_then(|n| n.checked_add(MAX_MEMORY))
        .ok_or("source-candidate debug logical envelope overflow")?;
    let catalog = MAX_FILES * (4096 + size_of::<DebugSourceFileV1>())
        + MAX_SITES * size_of::<DebugSourceSiteV1>()
        + MAX_SPANS * size_of::<DebugSourceSpanV1>();
    let required = transcript
        .checked_mul(7)
        .and_then(|n| {
            catalog
                .checked_mul(8)
                .and_then(|catalogs| n.checked_add(catalogs))
        })
        // Separate bounded shared-projection tree/Vec coexistence allowance.
        .and_then(|n| n.checked_add(16 * 1024 * 1024))
        .ok_or("source-candidate debug logical envelope overflow")?;
    if required > LOGICAL_ENVELOPE {
        return Err("source-candidate debug logical envelope exceeds fixed cap".into());
    }
    Ok(required)
}

pub(super) fn preflight_projection(
    owner: ExactDebugSourceOwnerV1<'_>,
    files: &[fe2o3_kernel_ir::DebugSourceMapFileV1],
) -> Result<(), String> {
    prepay_fixed_envelope()?;
    let module = owner.module();
    let [function] = module.functions.as_slice() else {
        return Err("source-candidate debug requires one exact executable function".into());
    };
    let body = function
        .body
        .as_ref()
        .ok_or("source-candidate debug body absent")?;
    if body.blocks.len() > 64
        || body
            .blocks
            .iter()
            .try_fold(0usize, |n, b| n.checked_add(b.operations.len()))
            .is_none_or(|n| n > MAX_SITES)
    {
        return Err("source-candidate debug executable projection bound".into());
    }
    let [semantic] = owner.semantic().semantic().functions() else {
        return Err("source-candidate debug semantic function roster".into());
    };
    if semantic.blocks().len() > 64
        || semantic.locals().len() > 4096
        || semantic
            .blocks()
            .iter()
            .try_fold(0usize, |n, b| n.checked_add(b.statements().len()))
            .is_none_or(|n| n > 4096)
    {
        return Err("source-candidate debug semantic projection bound".into());
    }
    let correspondence = owner.correspondence();
    for count in [
        correspondence.lowered_functions().len(),
        correspondence.statement_operation_spans().len(),
        correspondence.terminator_operation_spans().len(),
        correspondence.synthetic_operation_spans().len(),
    ] {
        if count > MAX_SPANS {
            return Err("source-candidate debug correspondence projection bound".into());
        }
    }
    let mut range_work = 0usize;
    for count in correspondence
        .statement_operation_spans()
        .iter()
        .map(|span| span.operation_count())
        .chain(
            correspondence
                .terminator_operation_spans()
                .iter()
                .map(|span| span.operation_count()),
        )
        .chain(
            correspondence
                .synthetic_operation_spans()
                .iter()
                .map(|span| span.operation_count()),
        )
    {
        range_work = range_work
            .checked_add(count as usize)
            .ok_or("source-candidate debug correspondence range-work overflow")?;
        if range_work > MAX_SPANS {
            return Err("source-candidate debug correspondence range-work bound".into());
        }
    }
    if files.is_empty()
        || files.len() > MAX_FILES
        || files.iter().any(|file| file.display_path().len() > 4096)
    {
        return Err("source-candidate debug captured file profile bound".into());
    }
    // The unchanged shared producer still checks every referenced file and
    // every operation, including exact synthetic/semantic non-overlap.
    Ok(())
}

fn span(value: &fe2o3_kernel_ir::DebugSourceMapSpanV1) -> DebugSourceSpanV1 {
    DebugSourceSpanV1 {
        file: value.file_identity(),
        byte_start: value.byte_start(),
        byte_end: value.byte_end(),
        line: value.line(),
        column: value.column(),
    }
}

fn catalog(
    owner: &OrderedProgramObservationOwnerV32,
    projection: ExactDebugSourceProjectionV1,
) -> Result<DebugSourceCatalogV1, String> {
    let executable = owner.materialized().executable();
    let module = executable.module();
    let span_count = projection
        .sites
        .iter()
        .try_fold(projection.eliminated.len(), |n, site| {
            n.checked_add(site.spans().len())
        })
        .ok_or("source-candidate debug projection span overflow")?;
    if projection.files.is_empty()
        || projection.files.len() > MAX_FILES
        || projection.sites.is_empty()
        || projection.sites.len() > MAX_SITES
        || span_count > MAX_SPANS
    {
        return Err("source-candidate debug projected catalog bound".into());
    }
    let files = projection
        .files
        .iter()
        .map(|file| DebugSourceFileV1 {
            identity: file.identity(),
            byte_len: file.byte_len(),
            display_path: file.display_path().to_owned(),
        })
        .collect();
    let mut sites = Vec::with_capacity(projection.sites.len());
    for mapped in &projection.sites {
        let wire = mapped.site();
        let function_ordinal = usize::try_from(wire.function_ordinal())
            .map_err(|_| "source-candidate debug function ordinal conversion")?;
        let function = module
            .functions
            .get(function_ordinal)
            .ok_or("source-candidate debug mapped function absent")?;
        let body = function
            .body
            .as_ref()
            .ok_or("source-candidate debug mapped body absent")?;
        let block = body
            .blocks
            .get(
                usize::try_from(wire.block_ordinal())
                    .map_err(|_| "source-candidate debug block ordinal conversion")?,
            )
            .ok_or("source-candidate debug mapped block absent")?;
        let operation = u32::try_from(wire.operation_ordinal())
            .map_err(|_| "source-candidate debug operation ordinal conversion")?;
        if block.operations.get(operation as usize).is_none() {
            return Err("source-candidate debug mapped operation absent".into());
        }
        sites.push(DebugSourceSiteV1 {
            site: SimulationDebugSiteV1 {
                function_ordinal,
                block: block.id,
                operation,
            },
            spans: mapped.spans().iter().map(span).collect(),
        });
    }
    let identity = executable.identity();
    DebugSourceCatalogV1::new_with_eliminated(
        DebugKirIdentityV1 {
            digest: *identity.digest(),
            canonical_len: executable.canonical().canonical_bytes().len() as u64,
        },
        files,
        sites,
        projection.eliminated.iter().map(span).collect(),
    )
    .map_err(|error| format!("source-candidate debug catalog: {error}"))
}

fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: 64 * 1024,
        max_reachable_functions: 1,
        max_reachable_operations: 4096,
        max_invocations: 64,
        max_workgroups: 1,
        max_scheduled_slots: 256,
        max_steps: 250_000,
        max_call_depth: 1,
        max_ssa_values: 4096,
        max_allocations: 1024,
        max_allocation_bytes: 4096,
        max_total_bytes: 1024 * 1024,
        max_resident_bytes: 32 * 1024 * 1024,
        max_events: 1,
        max_memory_access_records: 4096,
    }
}

fn backing(value: u32, initialized: bool) -> Result<SharedBufferV1, String> {
    let bytes = [0xdead_beefu32, value, 0x1357_2468u32]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let mut bits = vec![true; 12];
    bits[4..8].fill(initialized);
    Ok(SharedBufferV1 {
        id: BufferBackingIdV1(0),
        buffer: BufferArgumentV1::new(
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            bytes,
            bits,
            TARGET,
        )
        .map_err(|error| error.to_string())?,
    })
}

fn actual_capture(module: &AdmittedSimulationModuleV1) -> Result<(DebugTranscriptV1, u64), String> {
    let [kernel] = module.module().kernels.as_slice() else {
        return Err("source-candidate debug capture kernel roster".into());
    };
    let [a, b, mask] = [0xaaaa_aaaau32, 0x5555_5555, 0x0f0f_0f0f];
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(0),
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        4,
        1,
        TARGET,
    )
    .map_err(|error| error.to_string())?;
    let request = SimulationRequestV1::new(
        kernel.id.clone(),
        [64, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::BufferView(view),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(a)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(b)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(mask)),
        ],
    )
    .with_shared_buffers(vec![backing(0x3141_5926, false)?]);
    let before = request.clone();
    let expected = backing((a & mask) | (b & !mask), true)?;
    let run = capture_debugger_run_v1(
        module,
        &request,
        TARGET,
        limits(),
        SimulationDebugCaptureLimitsV1::new(1, 128, 1, 4096).map_err(|error| error.to_string())?,
        DebuggerLimitsV1::new(MAX_RECORDS, MAX_VALUES, MAX_MEMORY)
            .map_err(|error| error.to_string())?,
        DebugWaveWidthV1::Wave64,
    );
    let execution = run
        .execution
        .map_err(|error| format!("source-candidate debug execution: {error}"))?;
    if request != before
        || execution.identity() != module.identity()
        || execution.arguments() != request.arguments
        || execution.invocations_executed() != 64
        || execution.shared_buffers() != std::slice::from_ref(&expected)
        || run.transcript.completeness() != DebugTranscriptCompletenessV1::Complete
        || run.transcript.terminal_fault().is_some()
        || run.transcript.records().is_empty()
    {
        return Err(
            "source-candidate debug output/init/canary/identity/completeness mismatch".into(),
        );
    }
    Ok((run.transcript, execution.steps_executed()))
}

fn ordered_site(
    owner: &OrderedProgramObservationOwnerV32,
) -> Result<SimulationDebugSiteV1, String> {
    let mut selected = None;
    for (function_ordinal, function) in owner
        .materialized()
        .executable()
        .module()
        .functions
        .iter()
        .enumerate()
    {
        let body = function
            .body
            .as_ref()
            .ok_or("source-candidate debug ordered body absent")?;
        for block in &body.blocks {
            for (operation, op) in block.operations.iter().enumerate() {
                if matches!(&op.kind, OperationKind::Gfx942OrderedProgram(_)) {
                    let site = SimulationDebugSiteV1 {
                        function_ordinal,
                        block: block.id,
                        operation: u32::try_from(operation)
                            .map_err(|_| "source-candidate debug site overflow")?,
                    };
                    if selected.replace(site).is_some() {
                        return Err("source-candidate debug multiple ordered operations".into());
                    }
                }
            }
        }
    }
    selected.ok_or("source-candidate debug ordered operation absent".into())
}

fn current_source_positive(
    module: &AdmittedSimulationModuleV1,
    transcript: &DebugTranscriptV1,
    catalog: &DebugSourceCatalogV1,
    site: SimulationDebugSiteV1,
) -> Result<(), String> {
    let entry = catalog
        .sites()
        .iter()
        .find(|entry| entry.site == site)
        .ok_or("source-candidate debug ordered operation missing attribution")?;
    if entry.spans.is_empty() {
        return Err("source-candidate debug ordered operation has empty attribution".into());
    }
    let mut session = DebugSessionV1::new(transcript.clone());
    session
        .bind_source_catalog(module, catalog.clone())
        .map_err(|error| error.to_string())?;
    for phase in [
        SimulationDebugCheckpointPhaseV1::BeforeOperation,
        SimulationDebugCheckpointPhaseV1::AfterOperation,
    ] {
        let index = transcript.records().iter().position(|record| {
            record.site == site && record.invocation.global == [0, 0, 0]
                && matches!(&record.kind, SimulationDebugRecordKindV1::Checkpoint { phase: actual, .. } if *actual == phase)
        }).ok_or("source-candidate debug actual before/after checkpoint absent")?;
        session.seek_record_index(index);
        match session.source_spans() {
            DebugInspectionV1::Available(actual) if actual == entry.spans.as_slice() => {}
            _ => return Err("source-candidate debug actual source span query mismatch".into()),
        }
    }
    Ok(())
}

fn require_unbound(session: &mut DebugSessionV1) -> Result<(), String> {
    session.seek_record_index(0);
    if session.source_spans()
        != DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::SourceNotBound)
    {
        return Err("source-candidate debug failed binding changed source availability".into());
    }
    Ok(())
}

fn old_evidence_negatives(
    current: &AdmittedSimulationModuleV1,
    transcript: &DebugTranscriptV1,
    catalog: &DebugSourceCatalogV1,
    old: &CandidateDebugEvidenceV17,
) -> Result<(), String> {
    if old.catalog.identity() == catalog.identity() {
        return Err(
            "source-candidate debug register edit did not change actual canonical identity".into(),
        );
    }
    // First check: old catalog versus current actual transcript.
    {
        let mut session = DebugSessionV1::new(transcript.clone());
        if session.bind_source_catalog(current, old.catalog.clone())
            != Err(DebuggerErrorV1::SourceIdentityMismatch)
        {
            return Err("source-candidate debug old catalog refusal differs".into());
        }
        require_unbound(&mut session)?;
        session
            .bind_source_catalog(current, catalog.clone())
            .map_err(|error| error.to_string())?;
    }
    // Second check reaches validate_sites: old catalog and old capture agree,
    // but this is genuinely the CURRENT actual owner, not a rewritten digest.
    {
        let mut session = DebugSessionV1::new(old.transcript.clone());
        if session.bind_source_catalog(current, old.catalog.clone())
            != Err(DebuggerErrorV1::SourceIdentityMismatch)
        {
            return Err("source-candidate debug old pair/current owner refusal differs".into());
        }
        require_unbound(&mut session)?;
    }
    // Third check: old actual capture cannot be paired with current catalog.
    {
        let mut session = DebugSessionV1::new(old.transcript.clone());
        if session.bind_source_catalog(current, catalog.clone())
            != Err(DebuggerErrorV1::SourceIdentityMismatch)
        {
            return Err("source-candidate debug old capture refusal differs".into());
        }
        require_unbound(&mut session)?;
    }
    Ok(())
}

// Ordinary diagnostic values only. No serde, owner, module, source edit handle,
// borrowed TyCtxt, source/bundle subject or replay authority crosses sessions.
pub(crate) struct CandidateDebugEvidenceV17 {
    pub(crate) report: Value,
    catalog: DebugSourceCatalogV1,
    transcript: DebugTranscriptV1,
    inspection: inspection::InspectionEvidenceV17,
}

fn observe(
    owner: &OrderedProgramObservationOwnerV32,
    registers: Gfx942OrderedProgramRegistersV1,
    old: Option<&CandidateDebugEvidenceV17>,
) -> Result<CandidateDebugEvidenceV17, String> {
    let prepaid = prepay_fixed_envelope()?;
    let materialized = owner.materialized();
    if materialized.grants_artifact_or_launch_authority() {
        return Err("source-candidate debug unexpected owner authority".into());
    }
    if materialized
        .executable()
        .canonical()
        .canonical_bytes()
        .len()
        > 64 * 1024
    {
        return Err("source-candidate debug canonical byte profile bound".into());
    }
    let catalog = catalog(owner, owner.diagnostic_source_projection_v17()?)?;
    let module = AdmittedSimulationModuleV1::admit_v17(materialized.executable(), limits())
        .map_err(|error| error.to_string())?;
    if module.grants_execution_authority() {
        return Err("source-candidate debug unexpected simulation authority".into());
    }
    let (transcript, steps) = actual_capture(&module)?;
    let selected = ordered_site(owner)?;
    let (inspection, inspection_report) = inspection::observe(
        owner,
        registers,
        selected,
        old.map(|evidence| &evidence.inspection),
    )?;
    current_source_positive(&module, &transcript, &catalog, selected)?;
    if let Some(old) = old {
        old_evidence_negatives(&module, &transcript, &catalog, old)?;
    }
    let report = json!({
        "stage":"private_actual_source_candidate_catalog_join",
        "ordered_program_inspection":inspection_report,
        "catalog_identity":catalog.identity().digest,
        "catalog_canonical_bytes":catalog.identity().canonical_len,
        "source_files":catalog.files().len(),"mapped_sites":catalog.sites().len(),
        "eliminated_spans":catalog.eliminated().len(),
        "actual_captures":1,"capture_records":transcript.records().len(),
        "capture_steps":steps,"capture_step_limit":250_000,
        "actual_lane_zero_before_after":true,"complete_transcript":true,
        "capture_output_init_and_canaries_checked":true,"capture_input_immutable":true,
        "same_live_v17_owner":true,"in_memory_compiler_catalog_available":true,
        "old_evidence_used_only_as_negative_input":old.is_some(),
        "exact_source_identity_mismatch_refusals":if old.is_some() {3} else {0},
        "consumer":"DebugSessionV1::bind_source_catalog",
        "prepaid_combined_logical_payload":prepaid,
        "combined_logical_payload_limit":LOGICAL_ENVELOPE,"logical_payload_is_rss":false,
        "portable_map_transport":false,"portable_capture_import":false,
        "source_authentication_from_catalog":false,"production_resume":false,
        "physical_register_observations":false,"instruction_microsteps":false,
        "resource_lifetime_observations":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false,
    });
    Ok(CandidateDebugEvidenceV17 {
        report,
        catalog,
        transcript,
        inspection,
    })
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_source_candidate_debug_join(
        self,
        input: RetainedInput,
        registers: Gfx942OrderedProgramRegistersV1,
        previous: Option<&CandidateDebugEvidenceV17>,
    ) -> Result<CandidateDebugEvidenceV17, String> {
        let expected = if previous.is_some() {
            Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36])
        } else {
            Gfx942OrderedProgramRegistersV1::new(4, 5, [0, 1, 2])
        }
        .map_err(|error| error.to_string())?;
        if registers != expected {
            return Err("source-candidate debug closed register scenario differs".into());
        }
        let (fresh, (oracle, mut evidence)) = self
            .observe_fresh_source_bitselect_candidate_debug_with(input, registers, |owner| {
                observe(owner, registers, previous)
            })?;
        // Existing fresh helper rechecks actual source custody after ALL work.
        if oracle["runs"] != 30
            || oracle["output_and_canaries_checked"] != true
            || oracle["immutable_inputs"] != true
        {
            return Err("source-candidate debug existing full-output oracle differs".into());
        }
        evidence.report["fresh"] = fresh;
        evidence.report["whole_kernel_simulation"] = oracle;
        Ok(evidence)
    }
}

#[test]
fn private_debug_join_payload_profile_is_bounded() {
    // Arithmetic/control only; this does not create a compiler owner or capture.
    let prepaid = prepay_fixed_envelope().unwrap();
    assert!(prepaid > MAX_MEMORY * 4 && prepaid <= LOGICAL_ENVELOPE);
    assert_eq!(MAX_FILES, 16);
    assert_eq!(MAX_SITES, 4096);
    assert_eq!(MAX_SPANS, 8192);
}
