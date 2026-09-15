use super::*;
use crate::ShellLimits;
use fe2o3_kernel_ir::{PhaseOperationSourceV1, ReusablePhaseOperationV1};

// The existing backend fixture uses real KIR allocation/init/publish/read operations.
// It is inert test data, not an authenticated source or machine receipt.
#[allow(dead_code)]
mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-amdgcn-model/src/lowering/v13/reusable_phase_tests/fixture.rs"
    ));
}

fn session() -> PlironSession {
    PlironSession::new(
        ShellLimits::default(),
        [
            dialect_gpu::dialect_registration().unwrap(),
            dialect_kernel::dialect_registration().unwrap(),
        ],
    )
    .unwrap()
}

fn input() -> VerifiedCanonicalKernelIrV1 {
    VerifiedCanonicalKernelIrV1::from_module(fixture::memory(2), CanonicalKernelIrVersionV1::V14)
        .expect("complete two-phase memory fixture")
}

#[test]
fn declared_preflight_counts_phase_origins_in_its_existing_walk() {
    for phases in [1, 2] {
        let (_, _, count) = preflight(&fixture::memory(phases)).unwrap();
        assert_eq!(count, 1 + 7 * phases);
    }
}

#[test]
fn declared_preflight_keeps_legacy_phase_reservations_empty() {
    for module in fixture::legacy_modules() {
        let (_, _, count) = preflight(&module).unwrap();
        assert_eq!(count, 0);
        assert_eq!(declared_v14::reserve_phase_origins(count).unwrap().capacity(), 0);
    }
}

#[test]
fn declared_phase_origin_reservation_rejects_oversize_before_allocation() {
    assert!(matches!(
        declared_v14::reserve_phase_origins(HARD_MAX_OPERATION_TREE_ITEMS + 1),
        Err(KirBridgeErrorV1::Session(OperationHandleError::OperationTreeLimitExceeded))
    ));
}

#[test]
fn declared_v14_recursive_builder_verification_diagnostic() {
    let [legacy, _] = fixture::legacy_modules();
    let mut errors = Vec::new();
    for input in [
        input(),
        VerifiedCanonicalKernelIrV1::from_module(legacy, CanonicalKernelIrVersionV1::V13).unwrap(),
    ] {
        let mut owner = session();
        let result = owner.import_canonical_kir_declared_o0(&input);
        if let Err(error) = &result {
            let root = *owner.operations.values().next().expect("constructed root");
            errors.push(format!(
                "{:?}: {error:?}: recursive verifier {:?}",
                input.version(),
                pliron::operation::verify_operation(root, &owner.context)
            ));
        }
    }
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn declared_v14_inert_execution_attribute_does_not_impose_maximum_live_arity() {
    let module = fixture::memory(2);
    let context = Context::new();
    let mut conversion = false;
    for op in &module.functions[0].body.as_ref().unwrap().blocks[0].operations {
        let OperationKind::ExecutionCapability(contract) = &op.kind else {
            continue;
        };
        let attr = dialect_kernel::ExecutionCapabilityContractAttr::new(contract).unwrap();
        attr.verify(&context).unwrap();
        assert!(attr.contract(contract.operands.len()).is_some());
        if matches!(
            contract.operation,
            fe2o3_kernel_ir::ExecutionCapabilityOperationV1::ReusableLdsConversion(_)
        ) {
            conversion = true;
            assert!(attr.contract(0).is_none());
            assert!(attr.contract(2).is_none());
            assert!(
                attr.contract(fe2o3_kernel_ir::MAX_EXECUTION_CAPABILITY_OPERANDS_V1)
                    .is_none()
            );
        }
    }
    assert!(conversion);
}

fn phase_operations(context: &Context, root: Ptr<Operation>) -> Vec<PlironReusablePhaseOp> {
    let root_region = root.deref(context).get_region(0);
    let mut result = Vec::new();
    for block in root_region.deref(context).iter(context) {
        for function in block.deref(context).iter(context) {
            if !Operation::is_op::<FuncOp>(function, context) {
                continue;
            }
            let body = function.deref(context).get_region(0);
            for block in body.deref(context).iter(context) {
                for op in block.deref(context).iter(context) {
                    if let Some(phase) = Operation::get_op::<PlironReusablePhaseOp>(op, context) {
                        result.push(phase);
                    }
                }
            }
        }
    }
    result
}

fn first_phase(module: &Module) -> KirOperation {
    module
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|b| &b.blocks)
        .flat_map(|b| &b.operations)
        .find(|o| matches!(o.kind, OperationKind::ReusablePhase(_)))
        .unwrap()
        .clone()
}

fn mutate(mutation: impl FnOnce(&mut Context, Ptr<Operation>)) -> KirBridgeErrorV1 {
    let mut owner = session();
    let graph = owner.import_canonical_kir_declared_o0(&input()).unwrap();
    owner
        .with_canonical_kir_graph_mut_for_test(&graph, mutation)
        .unwrap();
    owner
        .extract_optimized_canonical_kir_declared_v1(&graph)
        .unwrap_err()
}

#[test]
fn declared_v14_two_phase_memory_roundtrip_retains_every_source_and_token() {
    for module in [
        fixture::memory(2),
        fixture::terminal_drops(fixture::memory(2)),
    ] {
        let input = VerifiedCanonicalKernelIrV1::from_module(
            module.clone(),
            CanonicalKernelIrVersionV1::V14,
        )
        .unwrap();
        let mut owner = session();
        let graph = owner.import_canonical_kir_declared_o0(&input).unwrap();
        assert_eq!(
            graph.declared_version(),
            Some(CanonicalKernelIrVersionV1::V14)
        );
        let (o0, _) = owner.extract_canonical_kir_declared_o0(&graph).unwrap();
        let (optimized, _) = owner
            .extract_optimized_canonical_kir_declared_v1(&graph)
            .unwrap();
        assert_eq!(o0.canonical_bytes(), input.canonical_bytes());
        assert_eq!(optimized.canonical_bytes(), input.canonical_bytes());
        assert_eq!(
            fe2o3_kernel_ir::decode_module_v14(optimized.canonical_bytes()).unwrap(),
            module
        );
    }
}

#[test]
fn declared_v13_facade_stays_byte_identical_through_common_bridge() {
    for module in fixture::legacy_modules() {
        let input = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let mut owner = session();
        let old = owner.import_canonical_kir_v13_o0(&input).unwrap();
        let common = owner
            .import_canonical_kir_declared_o0(input.as_common())
            .unwrap();
        assert_eq!(old.input, common.input);
        assert_eq!(
            common.declared_version(),
            Some(CanonicalKernelIrVersionV1::V13)
        );
        assert_eq!(
            owner
                .extract_canonical_kir_v13_o0(&old)
                .unwrap()
                .0
                .canonical_bytes(),
            input.canonical_bytes()
        );
        assert_eq!(
            owner
                .extract_canonical_kir_declared_o0(&common)
                .unwrap()
                .0
                .canonical_bytes(),
            input.canonical_bytes()
        );
    }
}

#[test]
fn declared_v14_legacy_content_is_not_inferred_or_downgraded_to_v13() {
    let [module, _] = fixture::legacy_modules();
    let input =
        VerifiedCanonicalKernelIrV1::from_module(module, CanonicalKernelIrVersionV1::V14).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_declared_o0(&input).unwrap();
    assert_eq!(
        graph.declared_version(),
        Some(CanonicalKernelIrVersionV1::V14)
    );
    assert_eq!(
        owner
            .extract_canonical_kir_declared_o0(&graph)
            .unwrap()
            .0
            .canonical_bytes(),
        input.canonical_bytes()
    );
    assert!(matches!(
        owner.extract_canonical_kir_v13_o0(&graph),
        Err(KirBridgeErrorV1::GraphIdentityMismatch)
    ));
}

#[test]
fn declared_v13_workgroup_publication_cannot_change_input_extent() {
    use fe2o3_kernel_ir::{
        ExecutionCapabilityOperationV1 as E, ExecutionCapabilityRoleV1 as R,
        ExecutionMemoryExtentV1, ExecutionMemoryInitializationV1,
    };
    let [_, module] = fixture::legacy_modules();
    let input = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v13_o0(&input).unwrap();
    let published = *graph
        .origins
        .logical_types
        .iter()
        .find_map(|(value, ty)| {
            matches!(ty, Type::ExecutionCapability(c) if matches!(c.role,
            R::MemoryView { initialization: ExecutionMemoryInitializationV1::Published, .. }))
            .then_some(value)
        })
        .unwrap();
    owner
        .with_canonical_kir_graph_mut_for_test(&graph, |context, _| {
            let raw = published.defining_op().unwrap();
            let publish = Operation::get_op::<PlironExecutionCapabilityOp>(raw, context).unwrap();
            assert!(matches!(
                publish.contract(context).unwrap().operation,
                E::WorkgroupMemoryPublish { .. }
            ));
            publish.verify(context).unwrap();
            let ty = published.get_type(context);
            let mut capability = ty
                .deref(context)
                .downcast_ref::<PlironExecutionCapabilityType>()
                .unwrap()
                .capability()
                .unwrap();
            let R::MemoryView { extent, .. } = &mut capability.role else {
                panic!()
            };
            *extent = ExecutionMemoryExtentV1::Static(8);
            let ty = PlironExecutionCapabilityType::get(context, &capability).unwrap();
            published.set_type(context, ty.into());
            assert!(publish.verify(context).is_err());
        })
        .unwrap();
    assert!(matches!(
        owner.extract_canonical_kir_v13_o0(&graph),
        Err(KirBridgeErrorV1::MalformedGraph)
    ));
}

#[test]
fn declared_v14_cannot_enter_v13_owner_or_existing_proof_stages() {
    let input = input();
    assert!(
        VerifiedCanonicalKernelIrV13::from_canonical_bytes(input.canonical_bytes().to_vec())
            .is_err()
    );
    let mut owner = session();
    let graph = owner.import_canonical_kir_declared_o0(&input).unwrap();
    assert!(matches!(
        owner.extract_canonical_kir_v13_o0(&graph),
        Err(KirBridgeErrorV1::GraphIdentityMismatch)
    ));
    assert!(matches!(
        owner.extract_optimized_canonical_kir_v13_v1(&graph),
        Err(KirBridgeErrorV1::GraphIdentityMismatch)
    ));
    assert_eq!(
        owner.with_canonical_kir_v13_functions(&graph, |_, _| Ok::<_, ()>(())),
        Err(KirBridgeErrorV1::GraphIdentityMismatch)
    );
    assert_eq!(
        owner.with_canonical_kir_v12_functions(&graph, |_, _| Ok::<_, ()>(())),
        Err(KirBridgeErrorV1::GraphIdentityMismatch)
    );
}

#[test]
fn declared_v14_payload_and_type_are_never_v13_singletons() {
    let operation = first_phase(&fixture::memory(2));
    assert!(CanonicalKirOperationAttr::new(&operation).is_none());
    let attribute =
        CanonicalKirOperationAttr::new_declared(&operation, CanonicalKernelIrVersionV1::V14)
            .unwrap();
    assert_eq!(
        attribute.operation_declared(CanonicalKernelIrVersionV1::V14),
        Some(operation.clone())
    );
    assert!(attribute.operation().is_none());
    assert!(
        attribute
            .operation_declared(CanonicalKernelIrVersionV1::V13)
            .is_none()
    );
    let mut owner = session();
    let graph = owner.import_canonical_kir_declared_o0(&input()).unwrap();
    owner
        .with_canonical_kir_graph_mut_for_test(&graph, |context, root| {
            let phases = phase_operations(context, root);
            for phase in phases {
                let operation = phase.contract(context).unwrap();
                for result in &operation.results {
                    if !PhaseValueTypeV14::supports(&result.ty) {
                        continue;
                    }
                    let ty = PhaseValueTypeV14::get(context, &result.ty).unwrap();
                    assert_eq!(ty.deref(context).kir_type(), Some(result.ty.clone()));
                    assert_eq!(type_from_pliron(context, ty.into()).unwrap(), result.ty);
                }
            }
            assert!(PhaseValueTypeV14::get(context, &Type::F32).is_none());
        })
        .unwrap();
}

#[test]
fn declared_v14_remap_changes_only_real_operand_ids() {
    let module = fixture::memory(2);
    for original in module
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|b| &b.blocks)
        .flat_map(|b| &b.operations)
        .filter(|o| matches!(o.kind, OperationKind::ReusablePhase(_)))
    {
        let ids = original
            .kind
            .operands()
            .iter()
            .enumerate()
            .map(|(i, _)| ValueId(7000 + i as u32))
            .collect::<Vec<_>>();
        let remapped = remap_canonical_operation(original.clone(), &ids).unwrap();
        let mut expected = original.clone();
        let OperationKind::ReusablePhase(p) = &mut expected.kind else {
            unreachable!()
        };
        p.operands.clone_from(&ids);
        assert_eq!(remapped, expected);
        assert!(remap_canonical_operation(original.clone(), &ids[..ids.len() - 1]).is_none());
        let mut extra = ids;
        extra.push(ValueId(9999));
        assert!(remap_canonical_operation(original.clone(), &extra).is_none());
    }
}

#[test]
fn declared_v14_source_binding_substitution_with_valid_hash_rejects() {
    assert_eq!(
        mutate(|context, root| {
            let phase = phase_operations(context, root).remove(0);
            let mut changed = phase.contract(context).unwrap();
            let OperationKind::ReusablePhase(p) = &mut changed.kind else {
                panic!()
            };
            let PhaseOperationSourceV1::Defined(source) = &mut p.source else {
                panic!()
            };
            source.source_binding = [239; 32];
            install_contract(context, &phase, changed, None, None);
            phase
                .verify(context)
                .expect("locally valid inert substitution reaches origin check");
        }),
        KirBridgeErrorV1::GraphIdentityMismatch
    );
}

fn install_contract(
    context: &mut Context,
    phase: &PlironReusablePhaseOp,
    contract: KirOperation,
    epoch: Option<[u8; 32]>,
    coordinate: Option<SourceCoordinateAttr>,
) {
    let raw = phase.get_operation();
    let operands = phase.operands(context);
    let types = (0..raw.deref(context).get_num_results())
        .map(|i| raw.deref(context).get_type(i))
        .collect();
    let epoch = epoch.unwrap_or_else(|| phase.canonical_graph_epoch(context).unwrap());
    let coordinate = coordinate.unwrap_or_else(|| phase.coordinate(context).unwrap());
    let fresh = PlironReusablePhaseOp::new(
        context,
        &contract,
        operands,
        types,
        CanonicalIdentityAttr::from_bytes(epoch),
        coordinate,
    )
    .unwrap();
    let attrs = fresh.get_operation().deref(context).attributes.clone();
    raw.deref_mut(context).attributes = attrs;
    Operation::erase(fresh.get_operation(), context);
}

#[test]
fn declared_v14_epoch_and_source_coordinate_substitutions_reject() {
    for change_epoch in [true, false] {
        assert_eq!(
            mutate(|context, root| {
                let phase = phase_operations(context, root).remove(0);
                let contract = phase.contract(context).unwrap();
                let coordinate = source_coordinate_attr(KirBridgeCoordinateV1::Operation {
                    function: 0,
                    block: 0,
                    operation: 999,
                })
                .unwrap();
                install_contract(
                    context,
                    &phase,
                    contract,
                    change_epoch.then_some([238; 32]),
                    (!change_epoch).then_some(coordinate),
                );
                phase.verify(context).unwrap();
            }),
            KirBridgeErrorV1::GraphIdentityMismatch
        );
    }
}

#[test]
fn declared_v14_inserting_deleting_or_replacing_phase_occurrence_rejects() {
    for mutation in 0..3 {
        assert_eq!(
            mutate(|context, root| {
                let phase = phase_operations(context, root).remove(0);
                let raw = phase.get_operation();
                if mutation != 1 {
                    let contract = phase.contract(context).unwrap();
                    let types = (0..raw.deref(context).get_num_results())
                        .map(|i| raw.deref(context).get_type(i))
                        .collect();
                    let fresh = PlironReusablePhaseOp::new(
                        context,
                        &contract,
                        phase.operands(context),
                        types,
                        CanonicalIdentityAttr::from_bytes(
                            phase.canonical_graph_epoch(context).unwrap(),
                        ),
                        phase.coordinate(context).unwrap(),
                    )
                    .unwrap();
                    fresh.get_operation().insert_before(context, raw);
                    if mutation == 2 {
                        let count = raw.deref(context).get_num_results();
                        for i in 0..count {
                            let old = raw.deref(context).get_result(i);
                            let new = fresh.get_operation().deref(context).get_result(i);
                            old.replace_all_uses_with(context, &new);
                        }
                    }
                }
                if mutation != 0 {
                    raw.unlink(context);
                }
            }),
            KirBridgeErrorV1::GraphIdentityMismatch
        );
    }
}

#[test]
fn declared_v14_wrong_operand_type_rejects_before_extraction() {
    assert_eq!(
        mutate(|context, root| {
            let phases = phase_operations(context, root);
            let begin = phases.iter().find(|p| matches!(p.contract(context).unwrap().kind,
            OperationKind::ReusablePhase(ref p) if matches!(p.operation, ReusablePhaseOperationV1::Begin {..}))).unwrap();
            let other = phases[0].get_operation().deref(context).get_operand(0);
            Operation::replace_operand(begin.get_operation(), context, 0, other);
            assert!(begin.verify(context).is_err());
        }),
        KirBridgeErrorV1::MalformedGraph
    );
}

#[test]
fn declared_v14_same_typed_prior_owner_reuse_rejects_lifecycle() {
    assert_eq!(
        mutate(|context, root| {
            let phases = phase_operations(context, root);
            let begins = phases.iter().filter(|p| matches!(p.contract(context).unwrap().kind,
            OperationKind::ReusablePhase(ref p) if matches!(p.operation, ReusablePhaseOperationV1::Begin {..}))).collect::<Vec<_>>();
            assert_eq!(begins.len(), 2);
            let prior = begins[0].get_operation().deref(context).get_operand(0);
            Operation::replace_operand(begins[1].get_operation(), context, 0, prior);
            begins[1]
                .verify(context)
                .expect("identical type still requires whole linear custody");
        }),
        KirBridgeErrorV1::MalformedGraph
    );
}

#[test]
fn declared_v14_result_token_substitution_rejects() {
    assert_eq!(
        mutate(|context, root| {
            let phases = phase_operations(context, root);
            let begins = phases.iter().filter(|p| matches!(p.contract(context).unwrap().kind,
            OperationKind::ReusablePhase(ref p) if matches!(p.operation, ReusablePhaseOperationV1::Begin {..}))).collect::<Vec<_>>();
            let other = begins[1].get_operation().deref(context).get_type(1);
            let result = begins[0].get_operation().deref(context).get_result(1);
            result.set_type(context, other);
            assert!(begins[0].verify(context).is_err());
        }),
        KirBridgeErrorV1::MalformedGraph
    );
}

#[test]
fn declared_v14_phase_reordering_does_not_bypass_ssa_or_lifecycle() {
    assert_eq!(
        mutate(|context, root| {
            let phases = phase_operations(context, root);
            let bind = phases.iter().find(|p| matches!(p.contract(context).unwrap().kind,
            OperationKind::ReusablePhase(ref p) if matches!(p.operation, ReusablePhaseOperationV1::Bind {..}))).unwrap();
            let close = phases.iter().find(|p| matches!(p.contract(context).unwrap().kind,
            OperationKind::ReusablePhase(ref p) if matches!(p.operation, ReusablePhaseOperationV1::CloseStorage {..}))).unwrap();
            let raw = close.get_operation();
            raw.unlink(context);
            raw.insert_before(context, bind.get_operation());
            close
                .verify(context)
                .expect("local attributes alone do not prove dominance");
        }),
        KirBridgeErrorV1::MalformedGraph
    );
}

#[test]
fn declared_v14_publish_retains_distinct_workgroup_and_lds_result_custody() {
    use fe2o3_kernel_ir::{
        ExecutionCapabilityOperationV1 as E, ExecutionCapabilityRoleV1 as R,
        ExecutionElementLayoutV1, ExecutionLdsStateV1, ExecutionTypeIdentityV1,
    };
    for axis in 0..8 {
        assert_eq!(
            mutate(|context, root| {
                let phases = phase_operations(context, root);
                let close = phases.iter().find(|p| matches!(p.contract(context).unwrap().kind,
                OperationKind::ReusablePhase(ref p) if matches!(p.operation, ReusablePhaseOperationV1::CloseStorage {..}))).unwrap();
                let published = close.get_operation().deref(context).get_operand(1);
                let raw = published.defining_op().unwrap();
                let publish =
                    Operation::get_op::<PlironExecutionCapabilityOp>(raw, context).unwrap();
                assert!(matches!(
                    publish.contract(context).unwrap().operation,
                    E::LdsPublish { .. }
                ));
                publish.verify(context).unwrap();
                let ty = published.get_type(context);
                let mut capability = ty
                    .deref(context)
                    .downcast_ref::<PlironExecutionCapabilityType>()
                    .unwrap()
                    .capability()
                    .unwrap();
                match axis {
                    0 => capability.source_type = ExecutionTypeIdentityV1::new([230; 32]),
                    1 => capability.provenance.target_brand = [231; 32],
                    2 => capability.workgroup_brand = Some([232; 32]),
                    3 => capability.epoch = Some([233; 32]),
                    4 => capability.role = R::Workgroup,
                    _ => {
                        let R::Lds {
                            element,
                            layout,
                            elements,
                            state,
                        } = &mut capability.role
                        else {
                            panic!()
                        };
                        match axis {
                            5 => *state = ExecutionLdsStateV1::InvocationInitialized,
                            6 => {
                                *element = ExecutionTypeIdentityV1::new([234; 32]);
                                *layout = ExecutionElementLayoutV1 {
                                    byte_size: 8,
                                    byte_alignment: 8,
                                };
                            }
                            7 => *elements += 1,
                            _ => unreachable!(),
                        }
                    }
                }
                let changed = PlironExecutionCapabilityType::get(context, &capability).unwrap();
                published.set_type(context, changed.into());
                assert!(publish.verify(context).is_err(), "axis {axis}");
            }),
            KirBridgeErrorV1::MalformedGraph,
            "axis {axis}"
        );
    }
}

#[test]
fn declared_v14_semantic_carrier_fails_closed_without_new_family_ordinal() {
    let mut owner = session();
    let graph = owner.import_canonical_kir_declared_o0(&input()).unwrap();
    owner
        .with_canonical_kir_graph_mut_for_test(&graph, |context, root| {
            let phase = phase_operations(context, root).remove(0);
            let op = Operation::get_op_dyn(phase.get_operation(), context);
            assert!(matches!(
                dialect_gpu::canonical_kir_safety_contract_v1(&*op, context),
                Err(dialect_gpu::CanonicalKirSafetyCarrierErrorV1::UnsupportedDeclaredPhase)
            ));
        })
        .unwrap();
    assert_eq!(dialect_gpu::CanonicalKirSafetyFamilyV1::ALL.len(), 17);
}

#[test]
fn declared_v14_fresh_storage_result_remaps_actual_bind_operand_not_source_payload() {
    let input = input();
    let mut owner = session();
    let graph = owner.import_canonical_kir_declared_o0(&input).unwrap();
    let mut old_result = None;
    owner.with_canonical_kir_graph_mut_for_test(&graph, |context, root| {
        let phases = phase_operations(context, root);
        let bind = phases.iter().find(|p| matches!(p.contract(context).unwrap().kind,
            OperationKind::ReusablePhase(ref p) if matches!(p.operation, ReusablePhaseOperationV1::Bind {..}))).unwrap();
        let storage = bind.get_operation().deref(context).get_operand(2);
        let producer = storage.defining_op().expect("real allocator result");
        assert_eq!(producer.deref(context).get_result(0), storage);
        let old = Operation::get_op::<PlironExecutionCapabilityOp>(producer, context).unwrap();
        assert!(matches!(old.contract(context).unwrap().operation,
            fe2o3_kernel_ir::ExecutionCapabilityOperationV1::ReusableLdsConversion(_)));
        let ty = producer.deref(context).get_type(0);
        let operands = producer.deref(context).operands().collect();
        let attrs = producer.deref(context).attributes.clone();
        let fresh = Operation::new(context, PlironExecutionCapabilityOp::get_concrete_op_info(),
            vec![ty], operands, vec![], 0);
        fresh.deref_mut(context).attributes = attrs;
        fresh.insert_before(context, producer);
        let fresh_value = fresh.deref(context).get_result(0);
        storage.replace_all_uses_with(context, &fresh_value);
        producer.unlink(context);
        old_result = Some(bind.contract(context).unwrap().kind.operands()[2]);
        bind.verify(context).unwrap();
    }).unwrap();
    let (output, _) = owner
        .extract_optimized_canonical_kir_declared_v1(&graph)
        .unwrap();
    let module = fe2o3_kernel_ir::decode_module_v14(output.canonical_bytes()).unwrap();
    let original = fe2o3_kernel_ir::decode_module_v14(input.canonical_bytes()).unwrap();
    let mut saw_remap = false;
    for (before, after) in original.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .zip(&module.functions[0].body.as_ref().unwrap().blocks[0].operations)
    {
        if let (OperationKind::ReusablePhase(a), OperationKind::ReusablePhase(b)) =
            (&before.kind, &after.kind)
        {
            assert_eq!(before.results, after.results);
            let mut expected = a.clone();
            expected.operands.clone_from(&b.operands);
            assert_eq!(
                &expected, b,
                "full source, token, provenance and obligation commitments survive"
            );
            if matches!(a.operation, ReusablePhaseOperationV1::Bind { .. })
                && a.operands[2] == old_result.unwrap()
            {
                assert_ne!(a.operands[2], b.operands[2]);
                saw_remap = true;
            }
        }
    }
    assert!(saw_remap);
    // O0 binds positional original IDs; optimized extraction assigns the new
    // live producer a fresh ID. Both preserve the same source commitments.
    assert_eq!(
        owner
            .extract_canonical_kir_declared_o0(&graph)
            .unwrap()
            .0
            .canonical_bytes(),
        input.canonical_bytes()
    );
}

#[cfg(feature = "internal-test-context-access")]
#[test]
fn declared_v14_text_cannot_mint_original_phase_occurrence_roster() {
    let mut owner = session();
    let graph = owner.import_canonical_kir_declared_o0(&input()).unwrap();
    let text = owner.canonical_kir_text_for_test(&graph).unwrap();
    assert!(text.contains("gpu.kir_reusable_phase_v14"));
    assert!(text.contains("gpu.phase_value_v14"));
    let mut fresh = session();
    let parsed = fresh
        .reparse_canonical_kir_text_for_test(&graph, &text)
        .unwrap();
    assert!(matches!(
        fresh.extract_canonical_kir_declared_o0(&parsed),
        Err(KirBridgeErrorV1::GraphIdentityMismatch)
    ));
}
