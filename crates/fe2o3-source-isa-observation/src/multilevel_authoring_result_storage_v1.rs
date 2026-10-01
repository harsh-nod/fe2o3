//! Actual-owner heap accounting for the four primary authoring action results.
//! These methods exclude the root header: embedding/caller ledgers charge it
//! exactly once. Capacity, not serialized size, is observed. No constructor,
//! schema, admission limit or authoring authority changes.

use super::{
    AuthoringAssemblySourceV1, AuthoringOperationPageV1, AuthoringOperationV1,
    AuthoringRegionSelectorV1, AuthoringRegionV1, AuthoringRustCandidateV1,
    AuthoringSnapshotSummaryV1, AuthoringSourceSpanV1, AuthoringValueV1,
};
use fe2o3_kernel_ir::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};
type Result = std::result::Result<(), Error>;

impl AuthoringRegionSelectorV1 {
    /// Charges owned heap only, under the caller's explicit observation limits.
    /// On refusal the ledger is incomplete and must not be reported as complete.
    pub fn charge_retained_heap_v1(&self, c: &mut Counter) -> std::result::Result<(), Error> {
        let Self {
            bundle_identity,
            canonical_kir_digest,
            target,
            operations,
        } = self;
        c.charge(0, 1)?;
        c.string(bundle_identity)?;
        c.string(canonical_kir_digest)?;
        c.string(target)?;
        c.vector(operations)
    }
}
impl AuthoringSnapshotSummaryV1 {
    /// Charges actual retained heap, not wire size or temporary/peak memory.
    pub fn charge_retained_heap_v1(&self, c: &mut Counter) -> std::result::Result<(), Error> {
        let Self {
            schema: _,
            authority: _,
            bundle_identity,
            bundle_subject_identity,
            canonical_kir_version: _,
            canonical_kir_digest,
            canonical_kir_bytes,
            target,
            source_map_identity,
            semantic_mir_identity,
            rustc_identity_inventory_receipt_sha256,
            rustc_identity_inventory_receipt_bytes,
            rustc_preflight_plan_receipt_sha256,
            rustc_preflight_plan_receipt_bytes,
            compiler_policy_identity: _,
            final_artifact_identity: _,
            operation_count: _,
            eliminated_source_span_count: _,
            capabilities,
        } = self;
        c.charge(0, 1)?;
        for text in [
            bundle_identity,
            bundle_subject_identity,
            canonical_kir_digest,
            canonical_kir_bytes,
            target,
            source_map_identity,
            semantic_mir_identity,
            rustc_identity_inventory_receipt_sha256,
            rustc_identity_inventory_receipt_bytes,
            rustc_preflight_plan_receipt_sha256,
            rustc_preflight_plan_receipt_bytes,
        ] {
            c.string(text)?;
        }
        // AuthoringCapabilityV1 is Copy and contains only borrowed static text.
        fixed_vector(capabilities, c)
    }
}
impl AuthoringOperationPageV1 {
    /// Charges actual retained heap, excluding this result's root header.
    pub fn charge_retained_heap_v1(&self, c: &mut Counter) -> std::result::Result<(), Error> {
        let Self {
            authority: _,
            bundle_identity,
            canonical_kir_digest,
            target,
            start: _,
            next_start: _,
            total_operations: _,
            operations,
        } = self;
        c.charge(0, 1)?;
        c.string(bundle_identity)?;
        c.string(canonical_kir_digest)?;
        c.string(target)?;
        operation_vector(operations, c)
    }
}
impl AuthoringRegionV1 {
    /// Includes the independently owned selector clone and every nested view.
    pub fn charge_retained_heap_v1(&self, c: &mut Counter) -> std::result::Result<(), Error> {
        let Self {
            authority: _,
            selector,
            structural_boundary: _,
            source_insertion_boundary: _,
            live_in,
            live_out,
            operations,
            materialization: _,
        } = self;
        c.charge(0, 1)?;
        selector.charge_retained_heap_v1(c)?;
        value_vector(live_in, c)?;
        value_vector(live_out, c)?;
        operation_vector(operations, c)
    }
}
impl AuthoringRustCandidateV1 {
    /// Includes generated source and owned selector/value copies, not authority.
    pub fn charge_retained_heap_v1(&self, c: &mut Counter) -> std::result::Result<(), Error> {
        let Self {
            authority: _,
            selector,
            helper_name,
            source,
            live_in,
            live_out,
            status: _,
            frontend_readmission: _,
            source_application: _,
            semantic_equivalence: _,
            exact_machine_contract: _,
        } = self;
        c.charge(0, 1)?;
        selector.charge_retained_heap_v1(c)?;
        c.string(helper_name)?;
        c.string(source)?;
        value_vector(live_in, c)?;
        value_vector(live_out, c)
    }
}
fn fixed_vector<T: Copy>(values: &Vec<T>, c: &mut Counter) -> Result {
    c.vector(values)
}
fn value_vector(values: &Vec<AuthoringValueV1>, c: &mut Counter) -> Result {
    c.vector(values)?;
    for AuthoringValueV1 { value: _, ty } in values {
        c.charge(0, 1)?;
        c.string(ty)?;
    }
    Ok(())
}
fn operation_vector(values: &Vec<AuthoringOperationV1>, c: &mut Counter) -> Result {
    c.vector(values)?;
    for operation in values {
        operation_heap(operation, c)?;
    }
    Ok(())
}
fn operation_heap(value: &AuthoringOperationV1, c: &mut Counter) -> Result {
    let AuthoringOperationV1 {
        coordinate: _,
        function_name,
        kind: _,
        semantic_detail,
        mnemonic,
        inline_assembly_source,
        inputs,
        results,
        local_memory_effects,
        complete_local_effect_summary: _,
        convergence: _,
        traps: _,
        physical_resources: _,
        source_binding: _,
        source_spans,
        materialization: _,
    } = value;
    c.charge(0, 1)?;
    c.string(function_name)?;
    if let Some(text) = semantic_detail {
        c.string(text)?;
    }
    if let Some(text) = mnemonic {
        c.string(text)?;
    }
    if let Some(AuthoringAssemblySourceV1 {
        frontend_unit,
        function,
        contract,
        statement,
        authority: _,
    }) = inline_assembly_source
    {
        c.charge(0, 1)?;
        c.string(frontend_unit)?;
        c.string(function)?;
        c.string(contract)?;
        c.string(statement)?;
    }
    value_vector(inputs, c)?;
    value_vector(results, c)?;
    c.vector(local_memory_effects)?;
    for effect in local_memory_effects {
        c.string(effect)?;
    }
    c.vector(source_spans)?;
    for AuthoringSourceSpanV1 {
        file_identity,
        display_path,
        byte_start,
        byte_end,
        line: _,
        column: _,
    } in source_spans
    {
        c.charge(0, 1)?;
        c.string(file_identity)?;
        c.string(display_path)?;
        c.string(byte_start)?;
        c.string(byte_end)?;
    }
    Ok(())
}
