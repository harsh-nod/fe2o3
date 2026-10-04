//! Actual source-derived launch-analysis freshness at the existing V17 consumer.
//! Test-only. An old move-only source-only roster is used exclusively as a
//! negative input; no compiler owner or serialized graph crosses rustc sessions.

use super::{CollectedRustStage, ProductionCompilation, SsaSemanticMirStage};
use crate::collector::source_census_v1::bitselect_feasibility::{
    ScanMeter,
    retained::{RetainedInput, fresh_header},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
    Gfx942OrderedProgramRegistersV1,
};
use fe2o3_lower_mir_kernel::{
    ProductionOrderedProgramPreRankedErrorV17, ProductionOrderedProgramPreRankedKirOwnerV17,
    ProductionSemanticKirErrorV1, ProductionSemanticKirLimitsV1, ProductionSourceLaunchRosterV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
    SemanticFunctionRoleV1, SemanticMirWireVersionV1, SemanticTargetArchitectureV1,
    SemanticTerminatorKindV1,
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const STALE_DETAIL: &str = "ordered program requires one exact V32 gfx942 source and launch root";
const PREFIX_WORK: usize = 7;
const REFUSAL_WORK: usize = 8;
const ROSTER_PAYLOAD_CAP: usize = 16 * 1024;

// Neither Clone nor serde. Its roster has no artifact/proof authority, and is
// never used for successful continuation of a later source session.
pub(crate) struct SourceLaunchAnalysisEvidenceV17 {
    pub(crate) report: Value,
    launch: ProductionSourceLaunchRosterV1,
    source_sha256: [u8; 32],
    canonical_sha256: [u8; 32],
    registers: [u8; 5],
}

fn roles(value: Gfx942OrderedProgramRegistersV1) -> [u8; 5] {
    let [a, b, mask] = value.inputs();
    [value.scratch(), value.output(), a, b, mask]
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_source_candidate_launch_analysis_v17(
        self,
        input: RetainedInput,
        registers: Gfx942OrderedProgramRegistersV1,
    ) -> Result<SourceLaunchAnalysisEvidenceV17, String> {
        let declared = roles(registers);
        if ![[4, 5, 0, 1, 2], [32, 33, 34, 35, 36]].contains(&declared) {
            return Err("source-launch analysis closed register profile".into());
        }
        let source_sha256 = <[u8; 32]>::from(Sha256::digest(input.original()));
        // Reuse the actual retained-source/HIR/semantic/SSA/KIR join and its
        // existing independent 30-run whole-kernel oracle, without a new importer.
        let (fresh, (oracle, (launch, canonical_sha256))) = self
            .observe_fresh_source_bitselect_candidate_debug_with(input, registers, |owner| {
                let launch = owner.launch_roster_for_freshness_negative_v17()?;
                if launch.grants_artifact_or_launch_authority()
                    || &launch != owner.materialized().source_launch()
                    || launch.roots().len() != 1
                {
                    return Err("source-launch current roster differs from actual owner".into());
                }
                Ok((
                    launch,
                    *owner.materialized().executable().identity().digest(),
                ))
            })?;
        if oracle["runs"] != 30
            || oracle["output_and_canaries_checked"] != true
            || oracle["immutable_inputs"] != true
        {
            return Err("source-launch existing independent oracle differs".into());
        }
        let retained_payload = std::mem::size_of_val(&launch)
            .checked_add(std::mem::size_of_val(launch.roots()))
            .ok_or("source-launch current retained payload overflow")?;
        if retained_payload > ROSTER_PAYLOAD_CAP {
            return Err("source-launch current retained payload cap".into());
        }
        let report = json!({
            "kind":"actual_source_launch_analysis_current_v17",
            "fresh":fresh,"whole_kernel_simulation":oracle,
            "source_sha256":source_sha256,"semantic_sha256":launch.semantic_sha256(),
            "canonical_sha256":canonical_sha256,"register_plan":declared,
            "actual_current_owner_materialized":true,"source_currentness_rechecked":true,
            "analysis_roots":1,"retained_extra_roster_payload_cap":ROSTER_PAYLOAD_CAP,
            "retained_extra_roster_payload_bytes":retained_payload,
            "resource_accounting_is_rss":false,
            "old_analysis_used_for_success":false,"ranked_checks":false,
            "functional_proof":false,"proof_invalidation_qualified":false,
            "production_resume":false,"hardware_observed":false,
            "source_authentication_claim":false,"grants_artifact_or_launch_authority":false,
        });
        Ok(SourceLaunchAnalysisEvidenceV17 {
            report,
            launch,
            source_sha256,
            canonical_sha256,
            registers: declared,
        })
    }

    pub(crate) fn refuse_stale_source_candidate_launch_analysis_v17(
        self,
        mut input: RetainedInput,
        old: SourceLaunchAnalysisEvidenceV17,
    ) -> Result<Value, String> {
        let mut meter = ScanMeter::default();
        let header = fresh_header(self.stage.tcx, &self.stage.closure, &input, &mut meter)?;
        let source_sha256 = <[u8; 32]>::from(Sha256::digest(input.original()));
        if old.registers != [4, 5, 0, 1, 2] || old.source_sha256 == source_sha256 {
            return Err("source-launch negative requires the genuine source register edit".into());
        }
        let ssa = self
            .import_semantic_mir()
            .and_then(ProductionCompilation::construct_semantic_middle_end)
            .and_then(ProductionCompilation::construct_semantic_ssa)
            .map_err(|error| format!("source-launch current source stages: {error}"))?;
        let SsaSemanticMirStage {
            semantic_ssa,
            bindings,
        } = ssa.stage;
        let semantic = semantic_ssa.source_semantic();
        if semantic.wire_version() != SemanticMirWireVersionV1::V32
            || semantic.target().architecture() != SemanticTargetArchitectureV1::AmdGpuGfx942
            || semantic.functions().len() != 1
            || semantic.roots().len() != 1
        {
            return Err("source-launch current V32 gfx942 singleton profile differs".into());
        }
        let root = semantic.roots()[0];
        let function = semantic
            .functions()
            .get(root.index() as usize)
            .ok_or("source-launch current root absent")?;
        let ids = header.identities;
        if root.index() != 0
            || function.identity() != ids.function()
            || function.item_definition_identity() != ids.item_definition()
            || function.monomorphization_identity() != ids.monomorphization()
            || function.generic_type_arguments_identity() != ids.generic_type_arguments()
            || function.const_generic_arguments_identity() != ids.const_generic_arguments()
            || function.role() != SemanticFunctionRoleV1::KernelRoot
        {
            return Err("source-launch current HIR/semantic root identity differs".into());
        }
        // Check the real semantic call, not copied register fields in a report.
        meter.rows(function.blocks().len())?;
        let mut selected = 0usize;
        for (index, block) in function.blocks().iter().enumerate() {
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && matches!(
                    semantic.callables().get(call.callee().index() as usize),
                    Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(_),
                        ..
                    })
                )
            {
                let index = u32::try_from(index).map_err(|_| "source-launch block overflow")?;
                let call = semantic
                    .checked_gfx942_ordered_program_call_v32(
                        root,
                        SemanticBlockIdV1::from_index(index),
                    )
                    .map_err(|error| error.to_string())?;
                let plan = call.registers();
                if [
                    plan.scratch(),
                    plan.output(),
                    plan.inputs()[0],
                    plan.inputs()[1],
                    plan.inputs()[2],
                ] != [32, 33, 34, 35, 36]
                    || call.program().active_descriptors() != [0x0085, 0x0133, 0x019d]
                    || call.source().function() != ids.function()
                {
                    return Err("source-launch current real register edit differs".into());
                }
                selected += 1;
            }
        }
        if selected != 1 {
            return Err("source-launch current ordered call roster differs".into());
        }
        crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
            &bindings.typed_descriptor_roots,
            semantic,
        )
        .map_err(|error| format!("source-launch descriptor ownership: {error}"))?;
        let [typed_root] = bindings.typed_descriptor_roots.as_slice() else {
            return Err("source-launch typed root roster differs".into());
        };
        let launch_input = typed_root
            .source_launch()
            .ok_or("source-launch exact source contract absent")?;
        let inputs = [
            crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                typed_root.logical_name(),
                typed_root.kernel_binding_bytes(),
                launch_input,
            ),
        ];
        // This is source-only agreement, not ranked verification. Both retained
        // contracts come from their respective actual frontend bindings.
        meter.storage(ROSTER_PAYLOAD_CAP)?;
        let current =
            crate::production_ranked_projection_v1::source_launch_roster_for_ranked_inputs_v1(
                &semantic_ssa,
                &inputs,
            )
            .map_err(|error| format!("source-launch current roster: {error}"))?;
        let [old_root] = old.launch.roots() else {
            return Err("source-launch old real roster is not singleton".into());
        };
        let [current_root] = current.roots() else {
            return Err("source-launch current real roster is not singleton".into());
        };
        let current_semantic = *semantic.semantic_sha256().as_bytes();
        let old_semantic = *old.launch.semantic_sha256();
        if current.semantic_sha256() != &current_semantic
            || old_semantic == current_semantic
            || old_root.selected_root() != current_root.selected_root()
            || old_root.source_launch() != current_root.source_launch()
            || old_root.layout().global_extents() != current_root.layout().global_extents()
            || old_root.layout().workgroup_extents() != current_root.layout().workgroup_extents()
            || old_root.layout().subgroup_size() != current_root.layout().subgroup_size()
            || old_root.layout().full_physical_workgroups()
                != current_root.layout().full_physical_workgroups()
            || old.launch.grants_artifact_or_launch_authority()
            || current.grants_artifact_or_launch_authority()
        {
            return Err(
                "source-launch edit must change semantic identity with identical geometry".into(),
            );
        }
        let workgroup = current_root.layout().workgroup_extents();
        let grid = current_root.layout().global_extents();
        let subgroup = current_root.layout().subgroup_size();
        let full = current_root.layout().full_physical_workgroups();
        let floor = std::mem::size_of_val(&old.launch)
            .checked_add(std::mem::size_of_val(old.launch.roots()))
            .and_then(|n| n.checked_add(std::mem::size_of_val(&current)))
            .and_then(|n| n.checked_add(std::mem::size_of_val(current.roots())))
            .ok_or("source-launch retained roster accounting overflow")?;
        if floor == 0 || floor > ROSTER_PAYLOAD_CAP {
            return Err("source-launch retained roster payload bound".into());
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(PREFIX_WORK + REFUSAL_WORK);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, ROSTER_PAYLOAD_CAP);
        budget
            .charge_work(PREFIX_WORK)
            .map_err(|error| error.to_string())?;
        budget
            .reserve_storage(floor)
            .map_err(|error| error.to_string())?;
        let ledger = budget.work_ledger_identity_v1();
        // No clone, deserialization, digest rewrite or old executable enters here.
        // Failure consumes the stale roster and current source SSA without ever
        // returning a new V17 executable owner.
        let result = ProductionOrderedProgramPreRankedKirOwnerV17::try_materialize_with_budget(
            semantic_ssa,
            old.launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        );
        if !matches!(
            result,
            Err(ProductionOrderedProgramPreRankedErrorV17::Lowering(
                ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: STALE_DETAIL,
                }
            ))
        ) {
            return Err("source-launch exact stale-analysis consumer refusal differs".into());
        }
        if budget.work() != PREFIX_WORK + REFUSAL_WORK
            || budget.storage() != floor
            || budget.peak_storage() != floor
            || budget.work_ledger_identity_v1() != ledger
            || budget.failed_storage().is_some()
        {
            return Err("source-launch stale refusal changed phase ledger".into());
        }
        input.recheck()?;
        budget
            .release_storage(floor)
            .map_err(|error| error.to_string())?;
        if budget.storage() != 0 || budget.work_ledger_identity_v1() != ledger {
            return Err("source-launch caller storage release differs".into());
        }
        let mut report = json!({
            "kind":"actual_source_launch_analysis_stale_refusal_v17",
            "consumer":"ProductionOrderedProgramPreRankedKirOwnerV17::try_materialize_with_budget",
            "old_source_sha256":old.source_sha256,"current_source_sha256":source_sha256,
            "old_semantic_sha256":old_semantic,"current_semantic_sha256":current_semantic,
            "old_canonical_sha256":old.canonical_sha256,
            "old_register_plan":old.registers,"current_register_plan":[32,33,34,35,36],
            "same_geometry":true,"workgroup":workgroup,"grid":grid,"subgroup":subgroup,
            "full_physical_workgroups":full,"exact_stale_analysis_refusals":1,
            "error":"Lowering::Unsupported","function":0,"block":null,"statement":null,
            "detail":STALE_DETAIL,"negative_work_units":REFUSAL_WORK,
        });
        // Keep macro expansion bounded without changing the flat report schema.
        let Value::Object(accounting) = json!({
            "prefix_work_units":PREFIX_WORK,"accepted_work_units":budget.work(),
            "work_limit":PREFIX_WORK + REFUSAL_WORK,"incoming_storage_bytes":floor,
            "peak_storage_bytes":budget.peak_storage(),"returned_storage_bytes":budget.storage(),
            "storage_limit":ROSTER_PAYLOAD_CAP,"incoming_storage_restored":true,
            "caller_storage_released":true,"same_work_ledger":true,
            "source_currentness_rechecked":true,"scan_accounting":meter,"io_accounting":input.io,
            "new_executable_owner_returned":false,"old_analysis_used_only_as_negative_input":true,
            "ranked_checks":false,"functional_proof":false,"proof_invalidation_qualified":false,
            "production_resume":false,"hardware_observed":false,"resource_accounting_is_rss":false,
            "source_authentication_claim":false,"grants_artifact_or_launch_authority":false,
        }) else {
            unreachable!("literal JSON object");
        };
        report
            .as_object_mut()
            .expect("literal JSON object")
            .extend(accounting);
        // Actual current authenticated source/target/descriptor bindings stay
        // alive through the refusal and source recheck, then are abandoned.
        drop(bindings);
        Ok(report)
    }
}
