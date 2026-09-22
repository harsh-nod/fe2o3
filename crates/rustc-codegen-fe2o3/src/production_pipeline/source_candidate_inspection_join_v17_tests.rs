//! Fresh-source inspection regression on the current borrowed V17 owner.
//! Only an inert, genuine prior identity crosses sessions. No view or owner does.

use super::OrderedProgramObservationOwnerV32;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
    Gfx942OrderedProgramRegistersV1, OperationKind, VerifiedCanonicalKernelIrIdentityV17,
    WaveWidth,
};
use fe2o3_kir_sim::SimulationDebugSiteV1;
use fe2o3_lower_mir_kernel::{
    MAX_PRODUCTION_ORDERED_PROGRAM_INSPECTION_WORK_V1,
    ProductionOrderedProgramInspectionAvailabilityV1 as Availability,
    ProductionOrderedProgramInspectionErrorV1 as InspectionError,
    ProductionOrderedProgramInspectionLimitsV1,
};
use serde_json::{Value, json};

const SNAPSHOT_LIMIT: usize = 64 * 1024;
const STORAGE_LIMIT: usize = 32 * 1024 * 1024;

// Copying this genuine observation does not retain or reconstruct its old owner.
// Deliberately no serde, raw constructor, compiler session, or borrowed view.
pub(super) struct InspectionEvidenceV17 {
    identity: VerifiedCanonicalKernelIrIdentityV17,
}

pub(super) fn observe(
    owner: &OrderedProgramObservationOwnerV32,
    expected_registers: Gfx942OrderedProgramRegistersV1,
    selected: SimulationDebugSiteV1,
    old: Option<&InspectionEvidenceV17>,
) -> Result<(InspectionEvidenceV17, Value), String> {
    let materialized = owner.materialized();
    let executable = materialized.executable();
    let identity = *executable.canonical().identity();
    let bytes = executable.canonical().canonical_bytes();
    if bytes.len() > SNAPSHOT_LIMIT || identity.canonical_length() != bytes.len() as u64 {
        return Err("source-candidate inspection canonical snapshot bound".into());
    }
    let local_limit = MAX_PRODUCTION_ORDERED_PROGRAM_INSPECTION_WORK_V1;
    // Exactly one current query, plus one stale query only in the edited session.
    // This ledger is not reset between the stale refusal and current recovery.
    let work_limit = local_limit
        .checked_mul(if old.is_some() { 2 } else { 1 })
        .ok_or("source-candidate inspection work limit overflow")?;
    let input_storage = materialized
        .executable_storage()
        .retained_storage()
        .checked_add(materialized.call_correspondence_storage())
        .ok_or("source-candidate inspection input storage overflow")?;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE_LIMIT);
    budget
        .reserve_storage(input_storage)
        .map_err(|error| error.to_string())?;
    // Prepay the bounded exact-byte snapshot before allocation; this is logical
    // payload accounting, not allocator slack, source-engine allocations or RSS.
    budget
        .reserve_storage(bytes.len())
        .map_err(|error| error.to_string())?;
    let mut before = Vec::new();
    before
        .try_reserve_exact(bytes.len())
        .map_err(|error| error.to_string())?;
    before.extend_from_slice(bytes);
    let query_floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut stale_work = 0;
    if let Some(old) = old {
        if old.identity == identity {
            return Err("source-candidate inspection edited identity did not change".into());
        }
        let work_before = budget.work();
        match materialized.inspect_ordered_program_v1(
            &old.identity,
            None,
            ProductionOrderedProgramInspectionLimitsV1::default(),
            &mut budget,
        ) {
            Err(InspectionError::StaleIdentity) => {}
            _ => return Err("source-candidate inspection exact stale refusal differs".into()),
        }
        stale_work = budget
            .work()
            .checked_sub(work_before)
            .ok_or("source-candidate inspection stale work regressed")?;
        if stale_work != 37
            || budget.storage() != query_floor
            || budget.work_ledger_identity_v1() != ledger
            || budget.failed_storage().is_some()
            || executable.canonical().canonical_bytes() != before.as_slice()
            || executable.canonical().identity() != &identity
        {
            return Err(
                "source-candidate inspection stale refusal changed custody or ledger".into(),
            );
        }
    }
    let (coordinate, view_storage) = {
        let (view, storage) = materialized
            .inspect_ordered_program_v1(
                &identity,
                None,
                ProductionOrderedProgramInspectionLimitsV1::default(),
                &mut budget,
            )
            .map_err(|error| format!("source-candidate current inspection: {error}"))?;
        if budget.storage() != query_floor || budget.work_ledger_identity_v1() != ledger {
            return Err("source-candidate inspection success changed incoming ledger".into());
        }
        let view_storage = storage.retained_storage();
        if view_storage != std::mem::size_of_val(&view) || view_storage > 1024 {
            return Err("source-candidate inspection view payload differs".into());
        }
        budget
            .reserve_storage(view_storage)
            .map_err(|error| error.to_string())?;
        let coordinate = view.coordinate();
        let function = executable
            .module()
            .functions
            .get(coordinate.block.function.0 as usize)
            .ok_or("source-candidate inspection function coordinate absent")?;
        let body = function
            .body
            .as_ref()
            .ok_or("source-candidate inspection function body absent")?;
        let block = body
            .blocks
            .get(coordinate.block.block as usize)
            .ok_or("source-candidate inspection block coordinate absent")?;
        let operation = block
            .operations
            .get(coordinate.operation as usize)
            .ok_or("source-candidate inspection operation coordinate absent")?;
        let OperationKind::Gfx942OrderedProgram(payload) = &operation.kind else {
            return Err("source-candidate inspection coordinate is not ordered program".into());
        };
        let [result] = operation.results.as_slice() else {
            return Err("source-candidate inspection result roster differs".into());
        };
        let [launch] = materialized.source_launch().roots() else {
            return Err("source-candidate inspection source launch roster differs".into());
        };
        let span = view.terminator_span();
        let end = span
            .first_operation_ordinal()
            .checked_add(span.operation_count())
            .ok_or("source-candidate inspection terminator span overflow")?;
        if view.canonical_identity() != &identity
            || view.semantic_sha256() != materialized.correspondence().semantic_sha256()
            || coordinate.block.function.0 as usize != selected.function_ordinal
            || block.id != selected.block
            || coordinate.operation != selected.operation
            || view.kernel_ir_block() != block.id
            || span.first_operation_ordinal() > coordinate.operation
            || coordinate.operation >= end
            || end as usize > block.operations.len()
            || view.ordered_program().registers() != expected_registers
            || view.ordered_program().registers() != payload.registers()
            || view.ordered_program().inputs() != payload.inputs()
            || view.ordered_program().result() != result.id
            || view.ordered_program().source() != payload.source()
            || view.declared_program() != payload.program()
            || view.declared_target() != "gfx942:xnack-"
            || view.declared_wave_width() != WaveWidth::Wave64
            || view.source_launch() != launch
            || view.semantic_function() != launch.selected_root()
            || view.source_association() != Availability::RetainedSemanticCorrespondence
            || view.physical_values() != Availability::Unavailable
            || view.final_artifact() != Availability::Unavailable
            || view.source_insertion() != Availability::Unavailable
            || view.can_materialize_helper()
            || view.authenticates_source()
            || view.grants_proof_or_resume_authority()
            || view.grants_artifact_or_launch_authority()
        {
            return Err("source-candidate current inspection exact owner/plan/site differs".into());
        }
        (coordinate, view_storage)
    };
    budget
        .release_storage(view_storage)
        .map_err(|error| error.to_string())?;
    if budget.storage() != query_floor
        || budget.work_ledger_identity_v1() != ledger
        || budget.failed_storage().is_some()
        || budget.work() <= stale_work
        || budget.work() > work_limit
        || executable.canonical().canonical_bytes() != before.as_slice()
        || executable.canonical().identity() != &identity
    {
        return Err("source-candidate current inspection changed custody or ledger".into());
    }
    drop(before);
    budget
        .release_storage(bytes.len())
        .map_err(|error| error.to_string())?;
    if budget.storage() != input_storage {
        return Err("source-candidate inspection snapshot storage not released".into());
    }
    let report = json!({
        "kind":"actual_source_ordered_program_inspection_join_v1",
        "consumer":"ProductionOrderedProgramPreRankedKirOwnerV17::inspect_ordered_program_v1",
        "canonical_sha256":identity.digest(),"canonical_bytes":identity.canonical_length(),
        "semantic_sha256":materialized.correspondence().semantic_sha256(),
        "coordinate":{"function":coordinate.block.function.0,"block":coordinate.block.block,
            "operation":coordinate.operation},
        "current_owner_inspections":1,"exact_stale_identity_refusals":usize::from(old.is_some()),
        "current_identity_rebind_after_stale":old.is_some(),
        "old_identity_used_only_as_negative_input":old.is_some(),
        "same_live_v17_owner":true,"exact_selected_site_checked":true,
        "canonical_bytes_unchanged":true,"incoming_storage_restored":true,
        "same_work_ledger":true,"accepted_work_units":budget.work(),"work_limit":work_limit,
        "local_query_work_limit":local_limit,"stale_refusal_work_units":stale_work,
        "input_storage_bytes":input_storage,"query_storage_floor_bytes":query_floor,
        "peak_storage_bytes":budget.peak_storage(),"storage_limit":STORAGE_LIMIT,
        "snapshot_byte_limit":SNAPSHOT_LIMIT,"view_payload_bytes":view_storage,
        "resource_accounting_is_rss":false,
        "ranked_checks":false,"functional_proof":false,"proof_invalidation_qualified":false,
        "source_authentication_claim":false,"production_resume":false,
        "physical_register_observations":false,"grants_artifact_or_launch_authority":false,
    });
    Ok((InspectionEvidenceV17 { identity }, report))
}
