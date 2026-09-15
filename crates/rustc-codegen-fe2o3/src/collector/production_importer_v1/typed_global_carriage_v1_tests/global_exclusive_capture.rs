//! Original shared input plus exclusive output closure, using the existing
//! actual AMD importer. No terminal or lifetime facts are injected by the test.
use super::*;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{ExclusiveReadWrite, Global, KernelContext, ReadOnly, kernel};
macro_rules! root {
    ($name:ident) => {
        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
        pub fn $name(mut context: KernelContext<'_>, input: Global<'_, f32, ReadOnly>, mut output: Global<'_, f32, ExclusiveReadWrite>) {
            context.with_workgroup(|_workgroup| {
                let Some(value) = input.load(0) else { return; };
                let _stored = output.store(0, value);
            });
        }
    };
}
root!(capture_a);
root!(capture_b);
"#;

struct ExclusiveProbe {
    completed: bool,
    cpu: &'static str,
}

impl Callbacks for ExclusiveProbe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("global_exclusive_capture_source.rs".into()),
            input: SOURCE.into(),
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
        .expect("collect original registered Global binds and mixed closure calls");
        assert!(
            closure
                .collection
                .functions
                .iter()
                .any(|function| function.closure_plan.is_some())
        );
        let typed_roots = closure.rederive_typed_descriptor_roots(tcx).unwrap();
        let observed = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
            .unwrap()
            .authenticate_import_session(tcx)
            .unwrap();
        let inventory =
            build_identity_inventory_v1(tcx, &observed, &closure.collection, &closure.roots)
                .unwrap();
        let closure_types = closure
            .collection
            .functions
            .iter()
            .filter_map(|function| function.closure_plan.as_ref())
            .flat_map(|plan| plan.authenticated_closure_type_identities())
            .map(SemanticTypeIdentityV1::from_sha256)
            .collect::<BTreeSet<_>>();
        let plan = build_production_semantic_preflight_plan_v1(
            tcx,
            canonical_target_layout_v1(observed.rustc_layout()),
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
        .expect("actual mixed capture imports without a collector exception");
        assert_eq!(imported.rustc_target.contract().cpu(), self.cpu);
        assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
        assert_eq!(
            imported.rustc_preflight_plan.canonical_transcript(),
            plan.canonical_transcript()
        );
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            imported.semantic_mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, &decoded)
            .unwrap();
        let make_owner = |mir| {
            ProductionSemanticSsaOwnerV1::try_new(
                ProductionSemanticMirOwnerV1::try_new(
                    mir,
                    ProductionSemanticMirLimitsV1::default(),
                )
                .unwrap(),
                ProductionSemanticSsaLimitsV1::default(),
            )
            .expect("actual mixed capture reaches its original checked execution SSA view")
        };
        let foreign = make_owner(decoded);
        let owner = make_owner(imported.semantic_mir);
        owner.verify_replay().unwrap();
        foreign.verify_replay().unwrap();
        let roots = owner.source_semantic().roots();
        assert_eq!(roots.len(), 2);
        for (index, root) in roots.iter().copied().enumerate() {
            let source = &owner.source_semantic().functions()[root.index() as usize];
            let binding = source.kernel_entry().unwrap().kernel_binding_identity();
            let typed = typed_roots
                .iter()
                .find(|typed| typed.kernel_binding_bytes() == *binding.as_bytes())
                .unwrap();
            let entry = imported
                .kernel_contexts
                .checked_ranked_entry(
                    &owner,
                    root,
                    typed.source_launch().unwrap(),
                    fe2o3_kernel_analysis::MAX_RANKED_BOUNDS_OPERATIONS * 16,
                )
                .unwrap()
                .expect("actual source Context entry relation is retained");
            crate::production_ranked_projection_v1::global_exclusive_source_checks::check(
                &owner,
                &foreign,
                root,
                roots[1 - index],
                &entry.0,
            );
        }
        self.completed = true;
        Compilation::Stop
    }
}

fn run(cpu: &'static str, name: &str) {
    let mut probe = ExclusiveProbe {
        cpu,
        completed: false,
    };
    if harness::run_probe(cpu, name, &mut probe) {
        assert!(probe.completed);
    }
}

#[test]
#[ignore = "requires cached actual AMD metadata; no Cargo or dependency rebuild"]
fn global_exclusive_capture_actual_amdgpu_gfx942() {
    run(
        "gfx942",
        "global_exclusive_capture::global_exclusive_capture_actual_amdgpu_gfx942",
    );
}

#[test]
#[ignore = "requires cached actual AMD metadata; no Cargo or dependency rebuild"]
fn global_exclusive_capture_actual_amdgpu_gfx950() {
    run(
        "gfx950",
        "global_exclusive_capture::global_exclusive_capture_actual_amdgpu_gfx950",
    );
}
