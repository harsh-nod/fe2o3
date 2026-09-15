use super::*;
use crate::rustc_semantic_plan_v1::build_production_semantic_preflight_plan_v1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{SemanticCallExpansionLimitsV1, SemanticCallExpansionV1};

mod defined_abis;
mod lowering;
mod terminal_abis;

pub(super) fn check(tcx: TyCtxt<'_>, lower: bool) {
    use crate::production_semantic_terminal_v1::{
        ProductionExecutionTerminalV1 as Terminal, ProductionTerminalExpansionV1 as Expansion,
    };
    use crate::production_target_v1::RetainedProductionTargetV1;

    assert_eq!(
        crate::collector::session_crate_binding(tcx),
        Some(harness::registration_binding())
    );
    let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
    let partitions = tcx.collect_and_partition_mono_items(());
    let closure = crate::collector::collect_authenticated_kernel_closure_v1(
        tcx,
        partitions.codegen_units,
        false,
        target,
    )
    .expect("collect actual registered roots and reviewed closure bodies");
    let typed_roots = closure.rederive_typed_descriptor_roots(tcx).unwrap();
    let observed_target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
        .unwrap()
        .authenticate_import_session(tcx)
        .unwrap();
    let mut observed_closure = crate::collector::collect_authenticated_kernel_closure_v1(
        tcx,
        partitions.codegen_units,
        false,
        RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap(),
    )
    .expect("independent collector custody for terminal ABI replay");
    let collected_contexts =
        collect_authenticated_kernel_contexts_v1(tcx, &mut observed_closure.collection).unwrap();
    let inventory =
        build_identity_inventory_v1(tcx, &observed_target, &closure.collection, &closure.roots)
            .unwrap();
    let observed_inventory = AuthenticatedRustcIdentityInventoryV3 {
        sha256: inventory.sha256,
        canonical_transcript: inventory.canonical_transcript.clone(),
    };
    let observed_contexts = bind_authenticated_kernel_contexts_v1(
        collected_contexts,
        &inventory.functions,
        &inventory.roots,
        &observed_inventory,
        &observed_target,
    )
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
        canonical_target_layout_v1(observed_target.rustc_layout()),
        inventory.functions,
        inventory.roots,
        inventory.sha256,
        &closure_types,
        DebugSourceCaptureRequestV2::Disabled,
    )
    .expect("retain the actual producer plan, including authenticated closure types");
    terminal_abis::check(tcx, &plan, &observed_contexts);
    let defined_abis = defined_abis::check(tcx, &plan);
    let imported =
        construct_production_semantic_mir_v1(tcx, closure, DebugSourceCaptureRequestV2::Disabled)
            .expect("real source must pass ordinary terminal authentication and canonical import");
    assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
    assert_eq!(
        imported.rustc_preflight_plan.canonical_transcript(),
        plan.canonical_transcript()
    );
    let mir = &imported.semantic_mir;
    defined_abis::check_canonical_locals(mir);
    assert_eq!(
        mir.functions()
            .iter()
            .map(|function| function.abi())
            .collect::<Vec<_>>(),
        defined_abis.iter().collect::<Vec<_>>()
    );
    assert_eq!(mir.wire_version(), SemanticMirWireVersionV1::V22);
    mir.require_complete_external_entries().unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
    assert_eq!(decoded.functions(), mir.functions());
    assert_eq!(decoded.callables(), mir.callables());
    let expansion =
        SemanticCallExpansionV1::try_new(mir, SemanticCallExpansionLimitsV1::default()).unwrap();
    expansion.verify_replay(&decoded).unwrap();
    validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, mir).unwrap();

    let mut producers = BTreeMap::new();
    let mut conversions = BTreeMap::new();
    let mut stores = BTreeMap::new();
    for (terminal_index, terminal) in plan.terminal_producers().iter().enumerate() {
        let index = plan.function_producers().len() + terminal_index;
        let callable: &SemanticCallableDeclV1 = &mir.callables()[index];
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        } = callable
        else {
            continue;
        };
        let operation: SemanticCompilerIntrinsicOperationV1 = *operation;
        let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } = operation
        else {
            continue;
        };
        let relevant = match contract.operation() {
            SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 {
                workgroup_reference,
                workgroup,
                option,
                witness,
            } => {
                assert_eq!(
                    terminal.expansion,
                    Expansion::Execution(Terminal::WorkgroupMemoryIndex1D)
                );
                assert_ne!(option, witness);
                assert_ne!(workgroup_reference, workgroup);
                assert_eq!(binding.abi().source_input_types(), [workgroup_reference]);
                assert_eq!(binding.abi().source_output_type(), option);
                assert_eq!(
                    binding.abi().source_argument_ownership(),
                    [SemanticSourceArgumentOwnershipV1::SharedBorrow]
                );
                let SemanticTypeShapeV1::Pointer(pointer) =
                    mir.types()[workgroup_reference.index() as usize].shape()
                else {
                    panic!("real source shared receiver");
                };
                assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
                assert_eq!(pointer.mutability(), SemanticMutabilityV1::Immutable);
                assert_eq!(pointer.pointee(), workgroup);
                assert_eq!(option_payload_v1(mir.types(), option).unwrap(), witness);
                assert!(
                    producers
                        .insert(contract.provenance().root(), (contract, witness))
                        .is_none()
                );
                reject_payload_substitution(
                    mir,
                    index,
                    contract,
                    workgroup_reference,
                    workgroup,
                    option,
                );
                true
            }
            SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexIntoDisjoint {
                input_witness,
                output_witness,
            } => {
                assert_eq!(terminal.expansion, Expansion::ThreadIndexIntoDisjoint);
                assert_eq!(binding.abi().source_input_types(), [input_witness]);
                assert_eq!(binding.abi().source_output_type(), output_witness);
                assert_eq!(
                    binding.abi().source_argument_ownership(),
                    [SemanticSourceArgumentOwnershipV1::ByValue]
                );
                let sig = signature(tcx, terminal.instance);
                let source =
                    thread_into_disjoint_contract_v1(tcx, sig.inputs()[0], sig.output()).unwrap();
                assert!(matches!(
                    source.mapping,
                    RustIndexMappingV1::WorkgroupMemory(_)
                ));
                assert!(rust_disjoint_index_space_v1(tcx, source.space).is_none());
                assert!(
                    conversions
                        .insert(
                            contract.provenance().root(),
                            (contract, input_witness, output_witness)
                        )
                        .is_none()
                );
                true
            }
            SemanticExecutionCapabilityOperationV1::MemoryStore { index, .. }
                if terminal.expansion
                    == Expansion::Execution(Terminal::WorkgroupMemoryDisjointStore) =>
            {
                assert!(
                    stores
                        .insert(contract.provenance().root(), (contract, index))
                        .is_none()
                );
                true
            }
            _ => false,
        };
        if !relevant {
            continue;
        }
        let root = capability_memory_root_for_terminal_v1(
            tcx,
            &plan,
            &imported.kernel_contexts,
            terminal_index as u32,
            terminal.expansion,
        )
        .unwrap()
        .expect("scoped conversion must require a real root");
        assert_eq!(
            contract.provenance(),
            capability_memory_provenance_v1(root, &imported.kernel_contexts).unwrap()
        );
        assert_eq!(contract.source_identity(), binding.identity());
        assert_eq!(
            operation,
            terminal_operation_v1(
                tcx,
                terminal.instance,
                terminal.expansion,
                binding.abi(),
                mir.types(),
                Some(root),
                terminal.identities.function(),
                &imported.kernel_contexts
            )
            .unwrap()
        );
        assert!(
            terminal_operation_v1(
                tcx,
                terminal.instance,
                terminal.expansion,
                binding.abi(),
                mir.types(),
                None,
                terminal.identities.function(),
                &imported.kernel_contexts
            )
            .is_err()
        );
        for other in &imported.kernel_contexts.roots {
            if other.selected_root != root.selected_root {
                assert!(
                    terminal_operation_v1(
                        tcx,
                        terminal.instance,
                        terminal.expansion,
                        binding.abi(),
                        mir.types(),
                        Some(other),
                        terminal.identities.function(),
                        &imported.kernel_contexts
                    )
                    .is_err(),
                    "wrong root cannot authenticate the original source brand"
                );
            }
        }
    }
    assert_eq!(mir.roots().len(), 2);
    for root in mir.roots() {
        let (producer, witness) = producers
            .get(root)
            .expect("each real root issues a scoped index");
        let (conversion, input, output) = conversions
            .get(root)
            .expect("each real root converts its existing index");
        let (store, stored) = stores
            .get(root)
            .expect("each real root uses the actual workgroup store");
        assert_eq!(witness, input);
        assert_eq!(stored, output);
        for c in [conversion, store] {
            assert_eq!(c.provenance(), producer.provenance());
            assert_eq!(c.workgroup_brand(), producer.workgroup_brand());
            assert_eq!(c.epoch_before(), producer.epoch_before());
            assert_eq!(c.epoch_after(), None);
        }
    }
    if lower {
        lowering::check(imported, typed_roots);
    }
}

fn reject_payload_substitution(
    mir: &AdmittedInertSemanticMirV1,
    index: usize,
    contract: SemanticExecutionCapabilityContractV1,
    reference: SemanticTypeIdV1,
    workgroup: SemanticTypeIdV1,
    option: SemanticTypeIdV1,
) {
    let changed = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 {
            workgroup_reference: reference,
            workgroup,
            option,
            witness: option,
        },
        contract.signature(),
        contract.provenance(),
        contract.workgroup_brand().unwrap(),
        contract.epoch_before().unwrap(),
        None,
        contract.source_identity(),
    )
    .and_then(|changed| {
        let mut callables = mir.callables().to_vec();
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut callables[index]
        else {
            unreachable!()
        };
        *operation =
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: changed };
        InertSemanticMirRequestV1::new_with_callables(
            mir.target(),
            mir.types().to_vec(),
            mir.allocations().to_vec(),
            mir.statics().to_vec(),
            mir.vtables().to_vec(),
            mir.functions().to_vec(),
            callables,
            mir.roots().to_vec(),
        )?
        .admit_current_production(SemanticMirLimitsV1::default())
    });
    assert!(
        changed.is_err(),
        "Option is not its own scoped witness payload"
    );
}
