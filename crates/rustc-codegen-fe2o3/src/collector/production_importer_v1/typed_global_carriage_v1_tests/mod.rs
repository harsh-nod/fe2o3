//! Actual AMD typed-global source carriage, stopping before SSA/target lowering.

use super::*;
use crate::production_semantic_terminal_v1::{
    ProductionExecutionTerminalV1 as Terminal, ProductionTerminalExpansionV1 as Expansion,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiArgumentV1, SemanticAbiValueV1, SemanticCapabilityMemoryAliasingV1,
    SemanticTerminatorKindV1,
};
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

mod atomic_entry;
mod blocked_lowering;
mod global_product;
mod global_exclusive_capture;
mod guarded_grid_leader;
mod harness;
mod mutations;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{
    Blocked, DisjointWrite, ExclusiveReadWrite, Global, Index1D, KernelContext, kernel,
};

macro_rules! root {
    ($name:ident) => {
        #[kernel(
            typed,
            launch(required = [1, 1, 1], max = [1, 1, 1], max_grid = [1, 1, 1])
        )]
        pub fn $name(
            context: KernelContext<'_>,
            mut data: Global<'_, f32, ExclusiveReadWrite>,
            mut blocked: Global<'_, f32, DisjointWrite<Blocked<Index1D, 1, 2>>>,
            _tag: u32,
        ) {
            let Some(value) = data.load(0) else { return; };
            if !data.store(0, value) { return; }
            let Some(block) = context.invocation().index_1d().checked_block::<1, 2>() else {
                return;
            };
            let _stored = blocked.store_block(&block, 1, value);
        }
    };
}
root!(typed_global_primary);
root!(typed_global_other);
"#;

const TERMINALS: [Terminal; 4] = [
    Terminal::GlobalBindExclusiveReadWrite,
    Terminal::GlobalExclusiveLoad,
    Terminal::GlobalExclusiveStore,
    Terminal::GlobalStoreBlock,
];

fn memory_fields(
    operation: SemanticCompilerIntrinsicOperationV1,
) -> Option<(
    SemanticTypeIdV1,
    SemanticCapabilityMemoryContractV1,
    SemanticKernelCapabilityProvenanceV1,
    SemanticFunctionIdentityV1,
)> {
    use SemanticCompilerIntrinsicOperationV1 as Op;
    match operation {
        Op::CapabilityGlobalBindExclusiveReadWrite {
            element,
            contract,
            provenance,
            source_identity,
            ..
        }
        | Op::CapabilityGlobalExclusiveLoad {
            element,
            contract,
            provenance,
            source_identity,
            ..
        }
        | Op::CapabilityGlobalExclusiveStore {
            element,
            contract,
            provenance,
            source_identity,
            ..
        }
        | Op::CapabilityGlobalStoreBlock {
            element,
            contract,
            provenance,
            source_identity,
            ..
        }
        | Op::CapabilityGlobalBindDisjointWrite {
            element,
            contract,
            provenance,
            source_identity,
            ..
        } => Some((element, contract, provenance, source_identity)),
        _ => None,
    }
}

fn round_trip(mir: &AdmittedInertSemanticMirV1) -> AdmittedInertSemanticMirV1 {
    assert_eq!(mir.wire_version(), SemanticMirWireVersionV1::V17);
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v17_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .expect("actual typed-global MIR V17 must round-trip");
    let current = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
    assert_eq!(current.canonical_encoding(), mir.canonical_encoding());
    assert_eq!(decoded.callables(), mir.callables());
    decoded
}

struct Probe {
    cpu: &'static str,
    completed: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("typed_global_carriage_source.rs".into()),
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
        .expect("collect real roots with compiler-issued contexts and separately owned views");
        let observed = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
            .unwrap()
            .authenticate_import_session(tcx)
            .unwrap();
        assert_eq!(observed.contract().cpu(), self.cpu);
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
        .expect("independently retain the actual source plan for carriage negatives");
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .unwrap_or_else(|error| {
            for (index, terminal) in plan.terminal_producers().iter().enumerate() {
                eprintln!(
                    "typed-global terminal {index}: {:?} {} args={:?} output={:?}",
                    terminal.expansion,
                    tcx.def_path_str(terminal.instance.def_id()),
                    terminal.abi.source_inputs,
                    terminal.abi.source_output
                );
            }
            panic!("all four memory terminals must complete canonical import before SSA: {error}");
        });
        assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
        assert_eq!(
            imported.rustc_preflight_plan.canonical_transcript(),
            plan.canonical_transcript()
        );
        let mir = round_trip(&imported.semantic_mir);
        mir.require_complete_external_entries().unwrap();
        assert_eq!(mir.roots().len(), 2);
        assert_eq!(imported.kernel_contexts.roots.len(), 2);
        for root in &imported.kernel_contexts.roots {
            assert!(mir.roots().contains(&root.selected_root));
            assert_eq!(root.physical_argument_count, 3);
            assert_eq!(root.logical_argument_count, 4);
            let abi = mir.functions()[root.selected_root.index() as usize].abi();
            assert_eq!(
                abi.source_input_types().len(),
                3,
                "root context is not a physical kernarg"
            );
        }
        validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, &mir)
            .expect("decoded typed-global operations must not be mistaken for ExecutionCapability");

        let f32_type = semantic_type_for_rust_v1(tcx, mir.types(), tcx.types.f32).unwrap();
        let u32_type = semantic_type_for_rust_v1(tcx, mir.types(), tcx.types.u32).unwrap();
        assert_ne!(f32_type, u32_type);
        assert_eq!(
            mir.types()[f32_type.index() as usize].layout().size_bytes(),
            Some(4)
        );
        assert_eq!(
            mir.types()[u32_type.index() as usize].layout().size_bytes(),
            Some(4)
        );
        let mut seen = BTreeSet::new();
        let mut provenances = BTreeMap::new();
        for (terminal_index, terminal) in plan.terminal_producers().iter().enumerate() {
            let Expansion::Execution(execution) = terminal.expansion else {
                continue;
            };
            if !TERMINALS.contains(&execution) {
                continue;
            }
            let index = plan.function_producers().len() + terminal_index;
            let SemanticCallableDeclV1::CompilerIntrinsic {
                binding, operation, ..
            } = &mir.callables()[index]
            else {
                panic!("missing canonical memory callable for {execution:?}");
            };
            use SemanticCompilerIntrinsicOperationV1 as Op;
            assert!(
                matches!(
                    (execution, operation),
                    (
                        Terminal::GlobalBindExclusiveReadWrite,
                        Op::CapabilityGlobalBindExclusiveReadWrite { .. }
                    ) | (
                        Terminal::GlobalExclusiveLoad,
                        Op::CapabilityGlobalExclusiveLoad { .. }
                    ) | (
                        Terminal::GlobalExclusiveStore,
                        Op::CapabilityGlobalExclusiveStore { .. }
                    ) | (
                        Terminal::GlobalStoreBlock,
                        Op::CapabilityGlobalStoreBlock { .. }
                    )
                ),
                "{execution:?} must retain its exact memory intrinsic, got {operation:?}"
            );
            let (element, contract, provenance, source) = memory_fields(*operation).unwrap();
            assert_eq!(element, f32_type);
            assert_eq!(source, terminal.identities.function());
            assert_eq!(binding.identity(), source);
            assert!(seen.insert((provenance.root(), execution.identity_tag())));
            if let Some(previous) = provenances.insert(provenance.root(), provenance) {
                assert_eq!(
                    previous, provenance,
                    "all memory uses retain the same issued root"
                );
            }
            let root = capability_memory_root_for_terminal_v1(
                tcx,
                &plan,
                &imported.kernel_contexts,
                terminal_index as u32,
                terminal.expansion,
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                provenance,
                capability_memory_provenance_v1(root, &imported.kernel_contexts).unwrap()
            );
            assert_eq!(
                *operation,
                terminal_operation_v1(
                    tcx,
                    terminal.instance,
                    terminal.expansion,
                    binding.abi(),
                    mir.types(),
                    Some(root),
                    terminal.identities.function(),
                    &imported.kernel_contexts,
                )
                .unwrap()
            );
            use SemanticSourceArgumentOwnershipV1 as Own;
            let ownership: &[Own] = match execution {
                Terminal::GlobalBindExclusiveReadWrite => &[Own::SharedBorrow, Own::UniqueBorrow],
                Terminal::GlobalExclusiveLoad => &[Own::SharedBorrow, Own::ByValue],
                Terminal::GlobalExclusiveStore => &[Own::UniqueBorrow, Own::ByValue, Own::ByValue],
                Terminal::GlobalStoreBlock => &[
                    Own::UniqueBorrow,
                    Own::SharedBorrow,
                    Own::ByValue,
                    Own::ByValue,
                ],
                _ => unreachable!(),
            };
            assert_eq!(binding.abi().source_argument_ownership(), ownership);
            let calls = mir
                .functions()
                .iter()
                .flat_map(|f| f.blocks())
                .filter_map(|block| {
                    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                        return None;
                    };
                    (call.callee().index() as usize == index).then_some(call)
                })
                .collect::<Vec<_>>();
            assert_eq!(
                calls.len(),
                1,
                "{execution:?} must have a retained real call site"
            );
            assert_eq!(calls[0].arguments().len(), ownership.len());
            if execution == Terminal::GlobalStoreBlock {
                assert_eq!(
                    contract,
                    SemanticCapabilityMemoryContractV1::global_disjoint_write(
                        contract
                            .index_space_type()
                            .expect("retained nominal index space"),
                        SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                            lanes_per_block: 1,
                            elements_per_lane: 2,
                        },
                    ),
                );
            } else {
                assert_eq!(
                    contract,
                    SemanticCapabilityMemoryContractV1::global_exclusive_read_write()
                );
            }
            mutations::check_record(&mir, index, u32_type);
            let index_ordinal = match execution {
                Terminal::GlobalExclusiveLoad | Terminal::GlobalExclusiveStore => Some(1),
                Terminal::GlobalStoreBlock => Some(2),
                _ => None,
            };
            if let Some(ordinal) = index_ordinal {
                let wrong_abi = mutations::index_abi(binding.abi(), ordinal, u32_type);
                assert!(
                    terminal_operation_v1(
                        tcx,
                        terminal.instance,
                        terminal.expansion,
                        &wrong_abi,
                        mir.types(),
                        Some(root),
                        terminal.identities.function(),
                        &imported.kernel_contexts,
                    )
                    .is_err(),
                    "{execution:?}: actual usize index/component cannot be replaced with u32"
                );
            }
        }
        for root in mir.roots() {
            for execution in TERMINALS {
                assert!(seen.contains(&(*root, execution.identity_tag())));
            }
        }
        assert_eq!(seen.len(), 8);
        assert_eq!(provenances.len(), 2);
        mutations::check_root_and_provenance(
            tcx,
            &plan,
            &imported.kernel_contexts,
            &mir,
            &provenances,
        );
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
#[ignore = "requires complete cached FE2O3_CORE_TRY_{DEVICE_RMETA,HOST_DEPS,AMDGPU_CORE,AMDGPU_BUILTINS}; never builds dependencies"]
fn typed_global_all4_full_import_gfx942_v17() {
    harness::run("gfx942", "typed_global_all4_full_import_gfx942_v17");
}

#[test]
#[ignore = "requires complete cached FE2O3_CORE_TRY_{DEVICE_RMETA,HOST_DEPS,AMDGPU_CORE,AMDGPU_BUILTINS}; never builds dependencies"]
fn typed_global_all4_full_import_gfx950_v17() {
    harness::run("gfx950", "typed_global_all4_full_import_gfx950_v17");
}
