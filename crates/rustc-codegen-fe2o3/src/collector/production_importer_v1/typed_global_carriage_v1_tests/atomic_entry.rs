//! Cached AMD source -> authenticated entry ABI -> canonical MIR. No lowering qualification.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{AtomicReadWrite, Global, KernelContext, SystemScope, kernel};

macro_rules! root {
    ($name:ident) => {
        #[kernel(typed, launch(required = [1, 1, 1], max = [1, 1, 1], max_grid = [1, 1, 1]))]
        pub fn $name(context: KernelContext<'_>, counters: Global<'_, u32, AtomicReadWrite<SystemScope>>) {
            let _ = (context, counters);
        }
    };
}
root!(atomic_primary);
root!(atomic_other);
"#;

struct AtomicProbe {
    completed: bool,
}

impl Callbacks for AtomicProbe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("atomic_entry_custody_source.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_target_v1::RetainedProductionTargetV1;
        use rustc_hir::Mutability;

        let partitions = tcx.collect_and_partition_mono_items(());
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap(),
        )
        .expect("real atomic entry must authenticate mutable physical custody");
        let observed = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
            .unwrap()
            .authenticate_import_session(tcx)
            .unwrap();
        let inventory =
            build_identity_inventory_v1(tcx, &observed, &closure.collection, &closure.roots)
                .unwrap();
        assert!(
            closure
                .collection
                .functions
                .iter()
                .all(|f| f.closure_plan.is_none())
        );
        let plan = build_production_semantic_preflight_plan_v1(
            tcx,
            canonical_target_layout_v1(observed.rustc_layout()),
            inventory.functions,
            inventory.roots,
            inventory.sha256,
            &BTreeSet::new(),
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("independent live source plan for exact terminal replay");
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("real atomic entry and exact binder must complete canonical import");
        let mir = round_trip(&imported.semantic_mir);
        assert_eq!(
            imported.rustc_preflight_plan.canonical_transcript(),
            plan.canonical_transcript()
        );
        let plan = &plan;
        mir.require_complete_external_entries().unwrap();
        validate_execution_terminal_carriage_v1(tcx, plan, &imported.kernel_contexts, &mir)
            .unwrap();
        assert_eq!(mir.roots().len(), 2);
        for root in &imported.kernel_contexts.roots {
            assert_eq!(
                (root.physical_argument_count, root.logical_argument_count),
                (1, 2)
            );
            let abi = mir.functions()[root.selected_root.index() as usize].abi();
            assert_eq!(
                abi.source_argument_ownership(),
                [SemanticSourceArgumentOwnershipV1::UniqueBorrow]
            );
        }
        let mut seen = BTreeSet::new();
        let mut live_views = Vec::new();
        for (ordinal, terminal) in plan.terminal_producers().iter().enumerate() {
            if terminal.expansion != Expansion::Execution(Terminal::BindAtomicView) {
                continue;
            }
            let index = plan.function_producers().len() + ordinal;
            let SemanticCallableDeclV1::CompilerIntrinsic {
                binding, operation, ..
            } = &mir.callables()[index]
            else {
                panic!("exact atomic entry callable");
            };
            let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } = *operation
            else {
                panic!("atomic entry execution contract");
            };
            let SemanticExecutionCapabilityOperationV1::Atomic {
                kind: SemanticExecutionAtomicKindV1::BindGlobalView,
                location_input,
                element,
                address_space: SemanticExecutionMemoryAddressSpaceV1::Global,
                scope: SemanticExecutionMemoryScopeV1::System,
                ..
            } = contract.operation()
            else {
                panic!("atomic kind, element, scope and address space must remain exact");
            };
            let abi = binding.abi();
            assert_eq!(
                mir.types()[element.index() as usize].shape(),
                &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32
                })
            );
            assert_eq!(
                abi.source_argument_ownership(),
                [
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                ]
            );
            assert_eq!(location_input, abi.source_input_types()[1]);
            let SemanticTypeShapeV1::Pointer(pointer) =
                mir.types()[location_input.index() as usize].shape()
            else {
                panic!("physical slice remains an exact reference");
            };
            assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
            assert_eq!(pointer.mutability(), SemanticMutabilityV1::Mutable);
            assert!(matches!(
                mir.types()[pointer.pointee().index() as usize].shape(),
                SemanticTypeShapeV1::Slice { .. }
            ));
            let signature = tcx.instantiate_bound_regions_with_erased(
                tcx.fn_sig(terminal.instance.def_id())
                    .instantiate(tcx, terminal.instance.args),
            );
            let physical = signature.inputs()[1];
            let TyKind::Ref(region, slice, Mutability::Mut) = *physical.kind() else {
                panic!("cached device must have the new mutable entry binder");
            };
            assert_eq!(rust_mutable_slice_element_v1(physical), Some(tcx.types.u32));
            let view = rust_capability_memory_view_v1(tcx, signature.output()).unwrap();
            crate::collector::authenticate_logical_physical_kernel_argument_v1(
                tcx,
                view.kernel,
                signature.output(),
                physical,
            )
            .unwrap();
            let readonly = Ty::new_imm_ref(tcx, region, slice);
            let error = crate::collector::authenticate_logical_physical_kernel_argument_v1(
                tcx,
                view.kernel,
                signature.output(),
                readonly,
            )
            .unwrap_err();
            assert_eq!(
                error,
                "capability view physical slice changed element type or mutability"
            );
            assert!(rust_mutable_slice_element_v1(readonly).is_none());
            live_views.push((view.kernel, signature.output(), physical));

            let root = capability_memory_root_for_terminal_v1(
                tcx,
                plan,
                &imported.kernel_contexts,
                ordinal as u32,
                terminal.expansion,
            )
            .unwrap()
            .unwrap();
            assert!(seen.insert(root.selected_root));
            assert_eq!(
                contract.provenance(),
                capability_memory_provenance_v1(root, &imported.kernel_contexts).unwrap()
            );
            assert_eq!(contract.source_identity(), binding.identity());
            assert_eq!(
                *operation,
                terminal_operation_v1(
                    tcx,
                    terminal.instance,
                    terminal.expansion,
                    abi,
                    mir.types(),
                    Some(root),
                    terminal.identities.function(),
                    &imported.kernel_contexts,
                )
                .unwrap()
            );

            let wrong_ownership = abi
                .clone()
                .with_source_argument_ownership(vec![
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                ])
                .unwrap();
            let error = terminal_operation_v1(
                tcx,
                terminal.instance,
                terminal.expansion,
                &wrong_ownership,
                mir.types(),
                Some(root),
                terminal.identities.function(),
                &imported.kernel_contexts,
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("typed-global terminal FnAbi"), "{error}");
            let error = terminal_operation_v1(
                tcx,
                terminal.instance,
                terminal.expansion,
                abi,
                mir.types(),
                None,
                terminal.identities.function(),
                &imported.kernel_contexts,
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains("execution terminal lacks unique authenticated kernel-root custody"),
                "{error}"
            );
            assert_eq!(
                mir.functions()
                    .iter()
                    .flat_map(|f| f.blocks())
                    .filter(|block| {
                        matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(call)
                    if call.callee().index() as usize == index)
                    })
                    .count(),
                1,
                "retain the actual binder call, not a synthetic issuer"
            );
        }
        assert_eq!(seen.len(), 2);
        let [(left, output, physical), (right, _, _)] = live_views.as_slice() else {
            panic!("one real atomic binder per root");
        };
        assert_ne!(left, right);
        let error = crate::collector::authenticate_logical_physical_kernel_argument_v1(
            tcx, *right, *output, *physical,
        )
        .unwrap_err();
        assert_eq!(
            error,
            "capability brand does not bind this kernel marker, target, and launch"
        );
        self.completed = true;
        Compilation::Stop
    }
}

fn run(cpu: &'static str, name: &str) {
    let mut probe = AtomicProbe { completed: false };
    if harness::run_probe(cpu, &format!("atomic_entry::{name}"), &mut probe) {
        assert!(
            probe.completed,
            "must finish actual entry and binder custody checks"
        );
    }
}

#[test]
#[ignore = "requires rebuilt cached AMD device metadata with mutable atomic entry binder; never builds dependencies"]
fn atomic_entry_custody_full_import_gfx942() {
    run("gfx942", "atomic_entry_custody_full_import_gfx942");
}

#[test]
#[ignore = "requires rebuilt cached AMD device metadata with mutable atomic entry binder; never builds dependencies"]
fn atomic_entry_custody_full_import_gfx950() {
    run("gfx950", "atomic_entry_custody_full_import_gfx950");
}
