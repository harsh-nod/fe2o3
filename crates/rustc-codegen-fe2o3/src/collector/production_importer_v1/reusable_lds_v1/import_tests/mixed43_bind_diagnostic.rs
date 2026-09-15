//! Diagnostic only: mixed43 nominal source joins, never an issuer or KIR proof.
use super::*;
use fe2o3_mir_model::{
    SemanticCallExpansionLimitsV1, SemanticCallExpansionV1, SemanticExpandedStatementOriginV1,
};
use std::fmt::Write;

#[path = "mixed43_bind_diagnostic/bounded.rs"]
mod bounded;
#[path = "mixed43_bind_diagnostic/harness.rs"]
mod harness;
#[path = "mixed43_bind_diagnostic/join.rs"]
mod join;

const BIND: &str = "fe2o3_device::execution::WorkgroupCapability::bind_reusable_lds";

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    cargo_metadata: &'static str,
    function: &'static str,
    old_coordinates: &'static str,
    features: &'static [&'static str],
}
const MUON: Case = Case {
    name: "mixed43_muon_bind_nominal_join",
    cargo_metadata: "967e5e7933dbcf3a",
    function: "38edd6c97b483e0a2a11d731cb9754def7d4bf80dd8493570ed6164d66dededf",
    old_coordinates: "root15/instance121/function9/return0/caller60:function8:bb29",
    features: &["kernel-muon-update"],
};
const BROADCAST: Case = Case {
    name: "mixed43_muon_broadcast16_bind_nominal_join",
    cargo_metadata: "0b4f47c17bdc6cff",
    function: "2a85f13aed9141f8b5eef41f4691c93cbf3d717928289b0a08b43d6a82eca205",
    old_coordinates: "root34/instance243/function15/return2/caller90:function49:bb33",
    features: &["ablation-muon-broadcast16", "kernel-muon-update"],
};

struct BindProbe {
    case: Case,
    completed: bool,
}
impl Callbacks for BindProbe {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_target_v1::RetainedProductionTargetV1;
        let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        )
        .expect("actual registered mixed43 source and provider authentication");
        let session = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
            .unwrap()
            .authenticate_import_session(tcx)
            .unwrap();
        let inventory =
            build_identity_inventory_v1(tcx, &session, &closure.collection, &closure.roots)
                .unwrap();
        let closure_types = closure
            .collection
            .functions
            .iter()
            .filter_map(|f| f.closure_plan.as_ref())
            .flat_map(|p| p.authenticated_closure_type_identities())
            .map(SemanticTypeIdentityV1::from_sha256)
            .collect::<BTreeSet<_>>();
        let plan = build_production_semantic_preflight_plan_v1(
            tcx,
            canonical_target_layout_v1(session.rustc_layout()),
            inventory.functions,
            inventory.roots,
            inventory.sha256,
            &closure_types,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("retain actual original source roster");
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("actual source must import before nominal diagnostic");
        assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
        assert_eq!(
            imported.rustc_preflight_plan.canonical_transcript(),
            plan.canonical_transcript()
        );
        let mir = &imported.semantic_mir;
        mir.require_complete_external_entries().unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
        validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, &decoded)
            .unwrap();
        validate_carriage(tcx, &plan, &imported.kernel_contexts, &decoded).unwrap();
        let expansion =
            SemanticCallExpansionV1::try_new(mir, SemanticCallExpansionLimitsV1::default())
                .expect("bounded checked expansion of actual source");
        expansion.verify_replay(mir).unwrap();
        assert!(!expansion.grants_proof_or_artifact_authority());
        let mut output = bounded::Output::new();
        writeln!(
            output,
            "DIAGNOSTIC_ONLY case={} old_report={} expected={} wire={:?}",
            self.case.name,
            self.case.old_coordinates,
            self.case.function,
            mir.wire_version()
        )
        .unwrap();
        let function = join::inspect(tcx, &plan, mir, &expansion, self.case, &mut output);
        // Emit the complete checked join even if a later, reordered SSA frontier differs.
        eprintln!("{}", output.as_str());
        drop(expansion);
        let source_owner = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
            decoded,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let result = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
            source_owner,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        );
        let mut status = bounded::Output::new();
        match result {
            Err(error) => {
                let exact = join::same_return_error(&error, function);
                writeln!(
                    status,
                    "CURRENT_SSA same_target_return={exact} error={error}"
                )
                .unwrap();
            }
            Ok(owner) => {
                owner.verify_replay().unwrap();
                writeln!(status, "CURRENT_SSA planned=true; no KIR/proof claim").unwrap();
            }
        }
        eprintln!("{}", status.as_str());
        eprintln!("DIAGNOSTIC_ONLY exact_nominal_join=complete production_authority=none");
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
#[ignore = "requires exact cached mixed43 AMD inputs; diagnostic only; no Cargo"]
fn mixed43_muon_bind_nominal_join() {
    harness::run(MUON);
}

#[test]
#[ignore = "requires exact cached mixed43 AMD inputs; diagnostic only; no Cargo"]
fn mixed43_muon_broadcast16_bind_nominal_join() {
    harness::run(BROADCAST);
}
