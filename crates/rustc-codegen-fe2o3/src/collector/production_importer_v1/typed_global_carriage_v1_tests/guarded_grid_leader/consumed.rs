//! Actual original source, ranked store consumer, and success-required KIR.
use super::*;
#[path = "consumed/retained_borrows.rs"]
mod retained_borrows;
#[path = "consumed/source_census.rs"]
mod source_census;
use fe2o3_lower_mir_kernel::{
    ProductionKernelContextLoweringInputV1, ProductionSemanticKirLimitsV1,
    ProductionSemanticKirOwnerV1,
};
use fe2o3_pliron::{ProductionGuardedGridActionKindV1, ProductionSemanticSsaSourceQueryErrorV1};

struct ConsumedProbe {
    cpu: &'static str,
    completed: bool,
}
impl Callbacks for ConsumedProbe {
    fn config(&mut self, config: &mut Config) {
        assert_eq!(SOURCE.matches("root!(guarded_leader_b);").count(), 1);
        config.input = Input::Str {
            name: FileName::Custom("guarded_grid_consumed_source.rs".into()),
            input: SOURCE.replace("root!(guarded_leader_b);", ""),
        };
    }
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
        .expect("retain the complete original Grid/index call closure");
        let typed = closure.rederive_typed_descriptor_roots(tcx).unwrap();
        let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
            .unwrap()
            .authenticate_import_session(tcx)
            .unwrap();
        let inventory =
            build_identity_inventory_v1(tcx, &target, &closure.collection, &closure.roots).unwrap();
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
            canonical_target_layout_v1(target.rustc_layout()),
            inventory.functions,
            inventory.roots,
            inventory.sha256,
            &closure_types,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .unwrap();
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("exact branded source imports before consumed correspondence");
        assert_eq!(imported.rustc_target.contract().cpu(), self.cpu);
        assert_eq!(
            imported.rustc_preflight_plan.canonical_transcript(),
            plan.canonical_transcript()
        );
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            imported.semantic_mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(
            decoded.canonical_encoding(),
            imported.semantic_mir.canonical_encoding()
        );
        validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, &decoded)
            .unwrap();
        assert_eq!(decoded.wire_version(), SemanticMirWireVersionV1::V26);
        assert!(!decoded.callables().iter().any(|c| matches!(c,
            SemanticCallableDeclV1::CompilerIntrinsic {operation:fe2o3_mir_model::semantic_mir_v1::SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent {..},..}
        )),"branded from_grid must not become GridLeaderCurrent");
        let typed = crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(
            typed, &decoded,
        )
        .unwrap();
        let [typed] = typed.as_slice() else {
            panic!("one exact physical root")
        };
        let [root] = decoded.roots() else {
            panic!("one original semantic root")
        };
        let root = *root;
        let contexts = &imported.kernel_contexts;
        contexts
            .validate_carriage(
                kernel_context_frontend_unit_identity_v1(&imported.rustc_identity_inventory),
                kernel_context_target_brand_identity_v1(&imported.rustc_target),
                &[ProductionKernelContextRootObservationV1 {
                    selected_root: root,
                    root_function_identity: *decoded.functions()[root.index() as usize]
                        .identity()
                        .as_bytes(),
                    kernel_binding: typed.kernel_binding_bytes(),
                    launch_brand_identity: kernel_context_launch_brand_identity_v1(
                        typed.source_launch().unwrap(),
                    ),
                }],
            )
            .unwrap();
        let [context] = contexts.roots.as_ref() else {
            panic!("one source-issued context")
        };
        let mut input = ProductionKernelContextLoweringInputV1::new(
            root,
            contexts.frontend_unit_identity,
            context.kernel_marker_identity,
            contexts.target_brand_identity,
            context.launch_brand_identity,
            context.issuance_identity,
        );
        if let Some(transfer) = context.entry_transfer {
            input = input.with_entry_transfer(transfer);
        }

        // Separate observation owner: these query results are not carried into
        // KIR. Its production constructor below builds and consumes its own plan.
        let observation_mir = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            imported.semantic_mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        let observed = ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                observation_mir,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        observed.verify_replay().unwrap();
        let view = observed.execution_view_for_root(root).unwrap();
        let source = observed
            .guarded_grid_source_for_root(root, view.body())
            .unwrap();
        assert!(matches!(
            observed.guarded_grid_source_for_root(root, &view.body().clone()),
            Err(ProductionSemanticSsaSourceQueryErrorV1::WrongOwner)
        ));
        let mut work = 1_000_000_usize;
        let mut charge = |n| match work.checked_sub(n) {
            Some(next) => {
                work = next;
                true
            }
            None => false,
        };
        let mut kinds = [0; 3];
        for i in 0..source.event_count() {
            let site = source.event_site(i).unwrap();
            assert!(matches!(
                source.action_at(site, &mut |_| false),
                Err(ProductionSemanticSsaSourceQueryErrorV1::WorkLimit)
            ));
            let action = source.action_at(site, &mut charge).unwrap().unwrap();
            assert!(action.belongs_to(&source));
            assert_eq!(action.receipt().contract().provenance().root(), root);
            kinds[match action.kind() {
                ProductionGuardedGridActionKindV1::Issuer => 0,
                ProductionGuardedGridActionKindV1::Some => 1,
                ProductionGuardedGridActionKindV1::Borrow => 2,
            }] += 1;
        }
        assert_eq!(
            kinds,
            [1, 1, 1],
            "complete issuer, erased Some, and one original shared borrow"
        );
        source_census::assert_source_census(&observed, root);
        crate::production_ranked_projection_v1::assert_actual_guarded_grid_store_projection(
            &observed, root, typed.source_launch().expect("authenticated source launch"),
        );
        let kir = ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            ProductionSemanticMirOwnerV1::try_new(
                decoded,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticKirLimitsV1::default(),
            vec![input],
        );
        if let Err(error) = &kir {
            retained_borrows::report(&observed, root, error);
        }
        drop(observed);
        let kir = kir.expect("original guarded Grid source must complete consumed KIR correspondence");
        kir.verify_equivalence().unwrap();
        assert!(!kir.canonical_kernel_ir_bytes().is_empty());
        // KIR construction is not final effect refinement, an export, or a GPU run.
        self.completed = true;
        Compilation::Stop
    }
}
fn run_consumed(cpu: &'static str, name: &str) {
    let mut probe = ConsumedProbe {
        cpu,
        completed: false,
    };
    if harness::run_probe(cpu, name, &mut probe) {
        assert!(probe.completed);
    }
}
#[test]
#[ignore = "requires cached actual AMD metadata; parent runs without dependency rebuild"]
fn guarded_grid_consumed_actual_amdgpu_gfx942() {
    run_consumed(
        "gfx942",
        "guarded_grid_leader::consumed::guarded_grid_consumed_actual_amdgpu_gfx942",
    );
}
#[test]
#[ignore = "requires cached actual AMD metadata; parent runs without dependency rebuild"]
fn guarded_grid_consumed_actual_amdgpu_gfx950() {
    run_consumed(
        "gfx950",
        "guarded_grid_leader::consumed::guarded_grid_consumed_actual_amdgpu_gfx950",
    );
}
