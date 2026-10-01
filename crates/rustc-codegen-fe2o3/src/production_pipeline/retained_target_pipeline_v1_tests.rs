//! Real-source observer plus focused custody/oracle controls.
//! The driver creates the actual transaction; no authenticated owner is fabricated.
use super::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const ORACLE_BYTES: usize = 256 * 1024;

fn bytes_hex(bytes: &[u8], limit: usize) -> std::result::Result<String, String> {
    if bytes.len() > limit {
        return Err("target oracle copy limit".into());
    }
    let capacity = bytes
        .len()
        .checked_mul(2)
        .ok_or("target oracle hex overflow")?;
    let mut output = String::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| "target oracle allocation")?;
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 15) as usize] as char);
    }
    Ok(output)
}

fn canonical_record(bytes: &[u8], identity: &[u8; 32]) -> std::result::Result<Value, String> {
    Ok(json!({
        "bytes": bytes.len().to_string(),
        "hex": bytes_hex(bytes, ORACLE_BYTES)?,
        "serialized_sha256": bytes_hex(&Sha256::digest(bytes), 32)?,
        "canonical_identity": bytes_hex(identity, 32)?,
    }))
}

// This oracle re-encodes the actual borrowed optimized module through its
// existing versioned owner, not a hand-authored model. That constructor/clone
// and report formatting are outside the copied-oracle byte cap and source meter.
fn target_record(
    target: &TargetLoweredProductionCompilation,
) -> std::result::Result<Value, String> {
    if target.llvm_ir().len() > ORACLE_BYTES {
        return Err("target LLVM oracle copy limit".into());
    }
    let canonical = match target.canonical_kernel_ir_version() {
        8 => {
            let owner =
                fe2o3_kernel_ir::VerifiedCanonicalKernelIrV8::from_module(target.module().clone())
                    .map_err(|e| e.to_string())?;
            canonical_record(owner.canonical_bytes(), owner.identity().digest())?
        }
        9 => {
            let owner =
                fe2o3_kernel_ir::VerifiedCanonicalKernelIrV9::from_module(target.module().clone())
                    .map_err(|e| e.to_string())?;
            canonical_record(owner.canonical_bytes(), owner.identity().digest())?
        }
        11 => {
            let owner =
                fe2o3_kernel_ir::VerifiedCanonicalKernelIrV11::from_module(target.module().clone())
                    .map_err(|e| e.to_string())?;
            canonical_record(owner.canonical_bytes(), owner.identity().digest())?
        }
        _ => return Err("unknown actual target canonical version".into()),
    };
    let original = target
        .admitted
        .semantic_kir()
        .pre_ranked_executable()
        .ok_or("target lost original pre-ranked owner")?;
    if target.grants_artifact_or_launch_authority() {
        return Err("inert target observation acquired authority".into());
    }
    Ok(json!({
        "target": target.target_name(),
        "kir_version": target.canonical_kernel_ir_version().to_string(),
        "canonical": canonical,
        "source_canonical_identity": bytes_hex(original.canonical().identity().digest(), 32)?,
        "llvm_bytes": target.llvm_ir().len().to_string(),
        "llvm_sha256": bytes_hex(&Sha256::digest(target.llvm_ir().as_bytes()), 32)?,
        "llvm": target.llvm_ir(),
        "semantic_functions": target.semantic_function_count().to_string(),
        "correspondence_blocks": target.correspondence_block_count().to_string(),
        "formal_allocations": target.formal_allocation_count().to_string(),
        "formal_accesses": target.formal_access_count().to_string(),
        "ranked_index_discharges": target.ranked_dynamic_index_discharge_count().to_string(),
        "target_passes": target.target_optimization_pass_count().to_string(),
        "target_mutating_passes": target.target_optimization_mutating_pass_count().to_string(),
        "retained_bindings": target.retained_identity_and_transaction_binding_count().to_string(),
        "artifact_or_launch_authority": false,
    }))
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Real existing production stages; the returned object is inert copied
    /// evidence, never a detachable owner or permission. The two cases must run
    /// in independent fresh rustc processes with the same source/invocation.
    pub(crate) fn observe_ordinary_target_account_for_test_v1(
        self,
        retained: bool,
        target_budget: &mut Budget<'_>,
    ) -> std::result::Result<Value, String> {
        if !retained {
            let target = self
                .lower_production_target(target_budget)
                .map_err(|e| e.to_string())?;
            let observation = target_record(&target)?;
            drop(target);
            return Ok(json!({ "target": observation, "source_account": null }));
        }
        super::super::tests::with_terminal_account_audit_v1(|| -> std::result::Result<_, String> {
            let target = self
                .lower_target_with_retained_source_account_v1(target_budget)
                .map_err(|e| e.to_string())?;
            let ledger = &target.account.ledger;
            let address = ledger.as_ref() as *const OwnedBudget as usize;
            let work = ledger.work();
            let storage = ledger.storage();
            let observation = target_record(&target.payload)?;
            let report = json!({
                "target": observation,
                "source_account": {
                    "work": work.to_string(), "storage": storage.to_string(),
                    "peak": ledger.peak_storage().to_string(),
                    "work_limit": ledger.work_limit().to_string(),
                    "storage_limit": ledger.storage_limit().to_string(),
                    "failed_work": ledger.failed_work().map(|v| v.to_string()),
                    "failed_storage": ledger.failed_storage().map(|v| v.to_string()),
                    "later_phase_work_charged_to_source": false,
                    "whole_memory_envelope": false,
                },
            });
            // The actual formal, ranked, optimized, LLVM and binding owners
            // are destroyed before OriginalMaterializationAccountV1::drop.
            drop(target);
            Ok((report, (address, work, storage)))
        })
    }
}

#[test]
fn exact_typed_successors_cannot_return_an_unwrapped_compiler_owner() {
    type Held<T> = RetainedMaterializationPhaseV1<T>;
    let _: fn(
        Held<MaterializedNeutralProductionCompilation>,
    ) -> Result<Held<RankedVerifiedProductionCompilation>> =
        Held::<MaterializedNeutralProductionCompilation>::verify_general_kernel_checks_retained_v1;
    let _: fn(
        Held<RankedVerifiedProductionCompilation>,
    ) -> Result<Held<TargetNeutralProductionCompilation>> =
        Held::<RankedVerifiedProductionCompilation>::attach_target_neutral_checks_retained_v1;
    let _: fn(
        Held<TargetNeutralProductionCompilation>,
    ) -> Result<Held<FormalMemoryAdmittedProductionCompilation>> =
        Held::<TargetNeutralProductionCompilation>::admit_formal_memory_retained_v1;
    let _: fn(
        Held<FormalMemoryAdmittedProductionCompilation>,
    ) -> Result<Held<TargetLoweredProductionCompilation>> =
        Held::<FormalMemoryAdmittedProductionCompilation>::lower_production_target_retained_v1;
}

#[test]
fn ordinary_and_retained_entries_share_one_original_materializer() {
    let parent = include_str!("../production_pipeline.rs");
    let core = include_str!("retained_target_pipeline_v1.rs");
    let ordinary = parent
        .split("fn materialize_with_context_observer_v29(")
        .nth(1)
        .unwrap()
        .split("// Both owning constructors")
        .next()
        .unwrap();
    assert!(
        ordinary.contains("retained_target_pipeline_v1::materialize_ordinary_owner_with_budget_v1")
    );
    assert!(!ordinary.contains("try_materialize_with_budget("));
    assert_eq!(
        core.matches("ProductionPreRankedKirOwnerV1::try_materialize_with_budget(")
            .count(),
        1
    );
    assert!(core.contains("materialize_prepared_with_budget_v29("));
}

#[test]
fn complete_retained_route_keeps_every_existing_gate_in_order() {
    let source = include_str!("retained_target_pipeline_v1.rs");
    let route = source
        .split(
            "pub(in crate::production_pipeline) fn lower_target_with_retained_source_account_v1(",
        )
        .nth(1)
        .unwrap()
        .split("\n#[cfg(test)]")
        .next()
        .unwrap();
    let mut prior = 0;
    for name in [
        ".import_semantic_mir()?",
        ".construct_semantic_middle_end()?",
        ".construct_semantic_ssa()?",
        ".materialize_target_neutral_retained_v1()?",
        ".verify_general_kernel_checks_retained_v1()?",
        ".require_ordinary_target_route_retained_v1(target_budget)?",
        ".attach_target_neutral_checks_retained_v1()?",
        ".admit_formal_memory_retained_v1()?",
        ".lower_production_target_retained_v1()",
    ] {
        assert_eq!(route.matches(name).count(), 1);
        let at = route.find(name).unwrap();
        assert!(at >= prior);
        prior = at + name.len();
    }
    assert!(source.contains("owner.has_direct_conditional_roots_v2()"));
    assert!(source.contains("owner.conditional_production_finalizer_refusal_v5(target_budget)"));
}

#[test]
fn copied_oracle_bytes_have_an_exact_pre_copy_limit() {
    assert_eq!(bytes_hex(&[0, 255, 16], 3).unwrap(), "00ff10");
    assert!(bytes_hex(&[0, 255, 16], 2).is_err());
    assert_eq!(bytes_hex(&[], 0).unwrap(), "");
    assert_eq!(bytes_hex("λ".as_bytes(), 2).unwrap(), "cebb");
    assert!(bytes_hex("λ".as_bytes(), 1).is_err());
}

#[test]
fn canonical_identity_and_serialized_digest_are_distinct_oracle_domains() {
    let record = canonical_record(b"synthetic control only", &[7; 32]).unwrap();
    assert_ne!(record["canonical_identity"], record["serialized_sha256"]);
    assert_eq!(record["canonical_identity"], "07".repeat(32));
    assert_eq!(record["bytes"], "22");
}

#[test]
fn terminal_audit_observes_one_actual_account_after_payload_destruction() {
    let result = super::super::tests::with_terminal_account_audit_v1(|| -> Result<_> {
        let held = RetainedMaterializationPhaseV1::start(19, 23, |budget| {
            budget
                .charge_work(3)
                .map_err(materialization_resource_error_v29)?;
            budget
                .reserve_storage(5)
                .map_err(materialization_resource_error_v29)?;
            Ok(41u32)
        })?;
        let address = held.account.ledger.as_ref() as *const OwnedBudget as usize;
        let next = held.try_map(|value, _| Ok(value + 1))?;
        assert_eq!(
            next.account.ledger.as_ref() as *const OwnedBudget as usize,
            address
        );
        let result = next.finish_copy();
        Ok((result, (address, 3, 5)))
    })
    .unwrap();
    assert_eq!(result, 42);
}

#[test]
fn complete_paid_route_keeps_every_existing_gate_and_changes_only_verifier_selection() {
    let source = include_str!("retained_target_pipeline_v1.rs");
    let route = source.split(
        "pub(in crate::production_pipeline) fn lower_target_with_paid_ranked_source_account_v1("
    ).nth(1).unwrap().split("\n}\n").next().unwrap();
    let mut prior = 0;
    for name in [
        ".import_semantic_mir()?",
        ".construct_semantic_middle_end()?",
        ".construct_semantic_ssa()?",
        ".materialize_target_neutral_retained_v1()?",
        ".verify_with_paid_ranked_allowances_retained_v1(analysis, snapshot)?",
        ".require_ordinary_target_route_retained_v1(target_budget)?",
        ".attach_target_neutral_checks_retained_v1()?",
        ".admit_formal_memory_retained_v1()?",
        ".lower_production_target_retained_v1()",
    ] {
        assert_eq!(route.matches(name).count(), 1);
        let at = route.find(name).unwrap();
        assert!(at >= prior);
        prior = at + name.len();
    }
    assert!(!route.contains(".verify_general_kernel_checks_retained_v1()"));
}
