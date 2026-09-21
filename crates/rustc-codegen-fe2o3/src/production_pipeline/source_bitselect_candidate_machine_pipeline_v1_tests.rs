//! Exact live V17 simulation and LLVM observation; no production continuation.
use super::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV17;

#[path = "source_bitselect_candidate_machine_simulation_v1_tests.rs"]
mod simulation;

pub(super) fn whole_kernel_oracle(
    owner: &VerifiedCanonicalKernelIrModuleV17,
) -> Result<Value, String> {
    simulation::observe(owner)
}

fn edited_registers() -> Gfx942OrderedProgramRegistersV1 {
    Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36])
        .expect("fixed distinct edited registers")
}

fn inspect(owner: &VerifiedCanonicalKernelIrModuleV17) -> Result<(Value, String), String> {
    let simulation = simulation::observe(owner)?;
    // Existing bounded emitter, directly borrowing the same immutable owner.
    // Its 16 MiB generation envelope is separate from the simulation ledger.
    let llvm =
        fe2o3_amdgcn_model::lower_canonical_v17_compiler_module_to_gfx942_xnack_minus_llvm_ir(
            owner,
        )
        .map_err(|e| format!("source-candidate exact-owner LLVM: {e}"))?;
    if llvm.is_empty() || llvm.len() > 64 * 1024 {
        return Err("source-candidate diagnostic LLVM publication cap".into());
    }
    Ok((simulation, llvm))
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_source_bitselect_candidate_machine(
        self,
        input: RetainedInput,
        bindings: Gfx942OrderedProgramRegistersV1,
    ) -> Result<(Value, String), String> {
        // All original source/identity/SSA/program checks remain in one helper.
        // The callback borrows the actual retained owner. Source currentness is
        // checked afterwards, before any inert result can leave the method.
        if bindings != registers() && bindings != edited_registers() {
            return Err("source-candidate machine requires a closed register plan".into());
        }
        let (fresh, (simulation, llvm)) =
            self.observe_fresh_source_bitselect_candidate_with(input, bindings, inspect)?;
        let [a, b, mask] = bindings.inputs();
        let report = json!({
            "stage":"fresh_source_candidate_machine_diagnostic",
            "fresh":fresh,
            "register_plan":[bindings.scratch(),bindings.output(),a,b,mask],
            "whole_kernel_simulation":simulation,
            "llvm_sha256":<[u8;32]>::from(Sha256::digest(llvm.as_bytes())),
            "llvm_bytes":llvm.len(),
            "llvm_emitter":"existing_exact_owner_v17_gfx942",
            "llvm_generation_limit":16 * 1024 * 1024,
            "llvm_publication_limit":64 * 1024,
            "same_live_owner_borrowed":true,
            "old_evidence_reused":false,
            "ranked_checks":false,"functional_proof":false,"production_resume":false,
            "native_emitted":false,"hardware_observed":false,
            "grants_artifact_or_launch_authority":false,
        });
        // Inert exact-owner text; no owner, proof or native artifact travels with it.
        Ok((report, llvm))
    }
}

#[test]
fn source_candidate_machine_plans_are_distinct_checked_register_roles() {
    assert_eq!(registers().inputs(), [0, 1, 2]);
    let edited = edited_registers();
    assert_eq!(edited.inputs(), [34, 35, 36]);
    assert_eq!(edited.scratch(), 32);
    assert_eq!(edited.output(), 33);
    assert_eq!(edited.vgpr_high_water(), 37);
    assert_ne!(edited, registers());
}
