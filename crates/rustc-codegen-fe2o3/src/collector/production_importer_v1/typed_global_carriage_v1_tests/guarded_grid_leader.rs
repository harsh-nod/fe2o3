//! Original registered Grid getter/issuer, canonical replay, and consumed SSA.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedCapabilityContractV1, SemanticFunctionDeclV1, SemanticGuardedGridLeaderV1,
    canonical_semantic_source_body_sha256_v25,
};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

#[path = "guarded_grid_leader/consumed.rs"]
mod consumed;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{kernel,KernelContext,Global,DisjointWrite,GridExclusive};
macro_rules! root {
    ($name:ident)=>{
        #[kernel(typed,launch(required=[64,1,1],max=[64,1,1],max_grid=[1,1,1]))]
        pub fn $name(context:KernelContext<'_>,mut output:Global<'_,u32,DisjointWrite<GridExclusive>>) {
            let Some(grid)=context.grid() else{return};
            let Some(leader)=grid.leader() else{return};
            let _stored=output.store(leader.index(0),7);
        }
    };
}
root!(guarded_leader_a);
root!(guarded_leader_b);
"#;

struct Probe {
    cpu: &'static str,
    completed: bool,
}
impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("guarded_grid_leader_source.rs".into()),
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
        .expect("retain original Grid and guarded issuer calls");
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
        .expect("guarded original source imports without terminalization");
        assert_eq!(imported.rustc_target.contract().cpu(), self.cpu);
        assert_eq!(
            imported.rustc_preflight_plan.canonical_transcript(),
            plan.canonical_transcript()
        );
        let mir = &imported.semantic_mir;
        assert_eq!(mir.wire_version(), SemanticMirWireVersionV1::V26);
        let records = mir
            .functions()
            .iter()
            .filter_map(|f| match f.defined_capability_contract() {
                Some(SemanticDefinedCapabilityContractV1::GuardedGridLeader(r)) => Some(*r),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            records.len(),
            2,
            "each branded root retains its own issuer relationship"
        );
        assert_ne!(records[0].provenance(), records[1].provenance());
        assert_ne!(records[0].brand(), records[1].brand());
        for record in &records {
            let getter = &mir.functions()[record.function().index() as usize];
            assert_eq!(getter.blocks().len(), 5);
            let issuer = &mir.functions()[record.issuer().function.index() as usize];
            assert_eq!(issuer.blocks().len(), 1);
            assert!(issuer.blocks()[0].statements().is_empty());
            assert!(matches!(
                issuer.blocks()[0].terminator().kind(),
                SemanticTerminatorKindV1::Return
            ));
            let SemanticTerminatorKindV1::Call(c) = getter.blocks()
                [record.roles().issuer.index() as usize]
                .terminator()
                .kind()
            else {
                panic!("original issuer call retained")
            };
            assert_eq!(
                mir.callables()[c.callee().index() as usize],
                SemanticCallableDeclV1::defined(record.issuer().function)
            );
            assert_eq!(
                canonical_semantic_source_body_sha256_v25(getter, 262_144)
                    .unwrap()
                    .0,
                *record.body_identity()
            );
            for body in [
                record.issuer(),
                record.caller(),
                record.grid_getter(),
                record.grid_current(),
            ] {
                let f = &mir.functions()[body.function.index() as usize];
                assert_eq!((f.identity(), f.abi().identity()), (body.source, body.abi));
                assert_eq!(
                    canonical_semantic_source_body_sha256_v25(f, 262_144)
                        .unwrap()
                        .0,
                    body.body
                );
            }
            let mut source = record.source();
            source.call_block = record.source().grid_call_block;
            assert_eq!(
                SemanticGuardedGridLeaderV1::for_defined_function(
                    record.function(),
                    mir.functions(),
                    mir.callables(),
                    mir.types(),
                    record.types(),
                    source,
                    record.provenance(),
                    record.brand(),
                    &mut 1_000_000
                ),
                Err(SemanticMirErrorV1::InvalidFunctionAbi)
            );
        }
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
        validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, &decoded)
            .unwrap();
        for (i, record) in records.iter().enumerate() {
            check_substituted_carriage(
                tcx,
                &plan,
                &imported.kernel_contexts,
                mir,
                *record,
                records[1 - i],
            );
        }
        let owner = ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                decoded,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .expect("original issuer return and erased Some have consumed same-owner SSA custody");
        owner.verify_replay().unwrap();
        for root in owner.source_semantic().roots() {
            let view = owner.execution_view_for_root(*root).unwrap();
            let plan = owner.execution_plan_for_root(*root).unwrap();
            let [receipt] = plan.guarded_grid_leader_results() else {
                panic!("one root-scoped guarded source receipt")
            };
            assert_eq!(receipt.contract().provenance().root(), *root);
            assert_eq!(
                view.instances()[receipt.getter().index() as usize].function(),
                receipt.contract().function()
            );
            assert_eq!(
                view.instances()[receipt.issuer().index() as usize].parent(),
                Some(receipt.getter())
            );
            assert_eq!(
                view.body().locals()[receipt.receiver().index() as usize].ty(),
                receipt.contract().types().grid_reference
            );
            assert_eq!(
                view.body().locals()[receipt.option().index() as usize].ty(),
                receipt.contract().types().leader_option
            );
        }
        // This callback stops at source/SSA. It claims no KIR leader correspondence.
        self.completed = true;
        Compilation::Stop
    }
}

fn check_substituted_carriage<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    mir: &AdmittedInertSemanticMirV1,
    record: SemanticGuardedGridLeaderV1,
    other: SemanticGuardedGridLeaderV1,
) {
    for (provenance, brand) in [
        (other.provenance(), record.brand()),
        (record.provenance(), other.brand()),
    ] {
        let replacement = SemanticGuardedGridLeaderV1::for_defined_function(
            record.function(),
            mir.functions(),
            mir.callables(),
            mir.types(),
            record.types(),
            record.source(),
            provenance,
            brand,
            &mut 1_000_000,
        )
        .unwrap();
        let mut functions = mir.functions().to_vec();
        let f = &functions[record.function().index() as usize];
        functions[record.function().index() as usize] = SemanticFunctionDeclV1::new(
            f.identity(),
            f.role(),
            f.item_definition_identity(),
            f.monomorphization_identity(),
            f.generic_type_arguments_identity(),
            f.const_generic_arguments_identity(),
            f.source(),
            f.abi().clone(),
            f.locals().to_vec(),
            f.entry(),
            f.blocks().to_vec(),
        )
        .unwrap()
        .with_defined_capability_contract(SemanticDefinedCapabilityContractV1::GuardedGridLeader(
            replacement,
        ))
        .unwrap();
        let altered = InertSemanticMirRequestV1::new_with_callables(
            mir.target(),
            mir.types().to_vec(),
            mir.allocations().to_vec(),
            mir.statics().to_vec(),
            mir.vtables().to_vec(),
            functions,
            mir.callables().to_vec(),
            mir.roots().to_vec(),
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        assert!(matches!(
            validate_execution_terminal_carriage_v1(tcx, plan, contexts, &altered),
            Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "guarded Grid leader attachment mismatch"
            ))
        ));
    }
}

fn run(cpu: &'static str, name: &str) {
    let mut probe = Probe {
        cpu,
        completed: false,
    };
    if harness::run_probe(cpu, name, &mut probe) {
        assert!(probe.completed);
    }
}
#[test]
#[ignore = "requires cached actual AMD metadata; parent runs without dependency rebuild"]
fn guarded_grid_leader_actual_amdgpu_gfx942() {
    run(
        "gfx942",
        "guarded_grid_leader::guarded_grid_leader_actual_amdgpu_gfx942",
    );
}
#[test]
#[ignore = "requires cached actual AMD metadata; parent runs without dependency rebuild"]
fn guarded_grid_leader_actual_amdgpu_gfx950() {
    run(
        "gfx950",
        "guarded_grid_leader::guarded_grid_leader_actual_amdgpu_gfx950",
    );
}
