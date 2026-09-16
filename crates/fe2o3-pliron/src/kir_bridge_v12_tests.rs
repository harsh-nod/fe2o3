use super::*;
use fe2o3_kernel_ir::{
    BasicBlock as KirBlock, CanonicalKernelIrWorkBudgetV1, FixedVectorTypeV12, Function,
    MemoryAccess, Signature, ValueDef, VectorLayoutConversionV12, VectorLayoutV12,
    VectorLoadOperationV12, VectorMemoryAccessV12, VectorStoreOperationV12,
    VerificationContractKeyV12, VerificationContractOperationV12, WorkgroupPipelineEventKindV12,
};
use pliron::opts::dce::SideEffects;

std::thread_local! {
    pub(super) static FAIL_IMPORT_AFTER_BUILD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

const WORK_FLOOR: usize = 11;
const STORAGE_FLOOR: usize = 7;
const AMPLE_WORK: usize = 1_000_000_000;
const AMPLE_STORAGE: usize = 100_000_000;

include!("kir_bridge_v12_mapped_result_tests.rs");
include!("kir_bridge_v12_roster_resources_tests.rs");

#[test]
fn execution_v15_roles_and_operations_are_rejected_by_both_bridge_profiles() {
    use fe2o3_kernel_ir::{ExecutionOperationV15 as Execution, ExecutionRoleV15 as Role};
    let context = Context::new();
    for profile in [KirBridgeTypeProfileV12::Legacy, KirBridgeTypeProfileV12::V12] {
        for role in [
            Role::Context, Role::Workgroup,
            Role::MaskedTileU32 { lanes: 3, elements: 2 },
            Role::LaneFragmentU32 { lanes: 3, elements: 2 },
        ] {
            let role = Type::Execution(role);
            for ty in [
                role.clone(),
                Type::pointer(
                    Type::slice(role, AddressSpace::Global, AccessMode::ReadOnly),
                    AddressSpace::Private, AccessMode::ReadWrite,
                ),
            ] {
                assert!(matches!(profile.preflight_type(&ty), Err(KirBridgeErrorV1::UnsupportedType)));
                assert!(matches!(profile.to_pliron(&context, &ty), Err(KirBridgeErrorV1::UnsupportedType)));
            }
        }
        for execution in [
            Execution::ContextIssue,
            Execution::WorkgroupDerive { context: ValueId(0) },
            Execution::ScopeEnd { workgroup: ValueId(0), discarded: vec![ValueId(1)] },
            Execution::MaskedTileLoadU32 {
                workgroup: ValueId(0), input: ValueId(1), base: ValueId(2), lanes: 3, elements: 2,
            },
            Execution::TileIntoFragmentU32 { tile: ValueId(0), lanes: 3, elements: 2 },
            Execution::FragmentIntoPartsU32 { fragment: ValueId(0), lanes: 3, elements: 2 },
        ] {
            let kind = OperationKind::Execution(execution);
            let operation = KirOperation::new(vec![], kind.clone());
            let coordinate = KirBridgeCoordinateV1::Operation { function: 0, block: 0, operation: 0 };
            assert!(matches!(
                profile.preflight_operation(&operation, coordinate),
                Err(KirBridgeErrorV1::UnsupportedOperation { .. }),
            ));
            assert!(preserved_operation_kind(&kind).is_err());
            assert!(remap_preserved_operation(&kind, kind.operands()).is_err());
        }
    }
}

fn owner(source: &Module) -> VerifiedCanonicalKernelIrModuleV12 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(AMPLE_WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, AMPLE_STORAGE);
    VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
        source,
        &mut budget,
    )
    .unwrap()
    .0
}

fn carriers() -> Module {
    let contiguous = FixedVectorTypeV12::new(ScalarType::F32, 4, VectorLayoutV12::Contiguous);
    let interleaved = contiguous.with_layout(VectorLayoutV12::Interleaved { factor: 2 });
    let mut first = KirBlock::new(BlockId(0));
    first.operations.push(KirOperation::new(
        vec![ValueDef::new(ValueId(3), Type::Vector(contiguous))],
        OperationKind::VectorLoad(VectorLoadOperationV12::new(
            ValueId(0),
            VectorMemoryAccessV12::new(contiguous, MemoryAccess::new(AddressSpace::Global, 16)),
        )),
    ));
    for kind in [
        WorkgroupPipelineEventKindV12::Stage,
        WorkgroupPipelineEventKindV12::Commit,
        WorkgroupPipelineEventKindV12::Wait,
        WorkgroupPipelineEventKindV12::Consume,
        WorkgroupPipelineEventKindV12::Discard,
        WorkgroupPipelineEventKindV12::Release,
    ] {
        first.operations.push(KirOperation::new(
            vec![],
            OperationKind::VerificationContract(
                VerificationContractOperationV12::WorkgroupPipelineEvent {
                    contract: VerificationContractKeyV12::new(17),
                    kind,
                    storage: ValueId(1),
                    epoch: ValueId(2),
                },
            ),
        ));
    }
    first.operations.push(KirOperation::new(
        vec![ValueDef::new(ValueId(4), Type::Vector(interleaved))],
        OperationKind::VectorLayoutConvert(VectorLayoutConversionV12::new(
            ValueId(3),
            interleaved.layout,
        )),
    ));
    first.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(4)],
    });
    let mut second = KirBlock::new(BlockId(1));
    second
        .parameters
        .push(ValueDef::new(ValueId(5), Type::Vector(interleaved)));
    let mut access = MemoryAccess::new(AddressSpace::Global, 32);
    access.volatile = true;
    second.operations.push(KirOperation::new(
        vec![],
        OperationKind::VectorStore(VectorStoreOperationV12::new(
            ValueId(0),
            ValueId(5),
            VectorMemoryAccessV12::new(interleaved, access),
        )),
    ));
    second.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    let mut module = Module::new("v12-ssa-transport");
    module.functions.push(Function::internal_helper(
        "vector-helper",
        Signature::new(
            vec![
                Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
                Type::INDEX,
            ],
            vec![Type::Vector(interleaved)],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![first, second],
    ));
    module
}

#[test]
fn all_vector_elements_layouts_and_nested_types_round_trip_without_widening_legacy() {
    let registration = dialect_gpu::dialect_registration().unwrap();
    let session = PlironSession::new(
        crate::ShellLimits::new(32, 64, 512).unwrap(),
        [registration],
    )
    .unwrap();
    for element in [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::I128,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::U128,
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        for (lanes, layout) in [
            (2, VectorLayoutV12::Contiguous),
            (4, VectorLayoutV12::Interleaved { factor: 2 }),
            (1024, VectorLayoutV12::Interleaved { factor: 512 }),
        ] {
            let vector = Type::Vector(FixedVectorTypeV12::new(element, lanes, layout));
            for ty in [
                vector.clone(),
                Type::pointer(
                    Type::slice(vector, AddressSpace::Global, AccessMode::ReadOnly),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ] {
                let live = KirBridgeTypeProfileV12::V12
                    .to_pliron(&session.context, &ty)
                    .unwrap();
                assert_eq!(
                    KirBridgeTypeProfileV12::V12
                        .decode_type(&session.context, live)
                        .unwrap(),
                    ty
                );
                assert!(matches!(
                    type_to_pliron(&session.context, &ty),
                    Err(KirBridgeErrorV1::UnsupportedType)
                ));
                assert!(matches!(
                    type_from_pliron(&session.context, live),
                    Err(KirBridgeErrorV1::UnsupportedType)
                ));
            }
        }
    }
    for vector in [
        FixedVectorTypeV12::new(ScalarType::Bool, 4, VectorLayoutV12::Contiguous),
        FixedVectorTypeV12::new(ScalarType::Index, 4, VectorLayoutV12::Contiguous),
        FixedVectorTypeV12::new(ScalarType::F32, 1, VectorLayoutV12::Contiguous),
        FixedVectorTypeV12::new(
            ScalarType::F32,
            6,
            VectorLayoutV12::Interleaved { factor: 4 },
        ),
    ] {
        assert!(
            KirBridgeTypeProfileV12::V12
                .to_pliron(&session.context, &Type::Vector(vector))
                .is_err()
        );
    }
}

#[test]
fn connected_o0_preserves_vectors_block_arguments_contract_order_and_v12_digest() {
    let source = carriers();
    let input = owner(&source);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(AMPLE_WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, AMPLE_STORAGE);
    budget.reserve_storage(STORAGE_FLOOR).unwrap();
    let (mut graph, storage) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
    assert_eq!(budget.storage(), STORAGE_FLOOR);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(graph.retained_storage(), storage.retained_storage());
    assert_eq!(graph.origins.preserved_operations.len(), 9);
    for pointer in graph.origins.preserved_operations.keys() {
        let operation =
            Operation::get_op::<PreservedOperationOp>(*pointer, &graph.session.context).unwrap();
        assert!(operation.has_side_effects(&graph.session.context));
    }
    let (output, report, output_storage) = graph
        .extract_canonical_kir_module_v12_o0(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), STORAGE_FLOOR + storage.retained_storage());
    assert_eq!(output.module(), &source);
    assert_eq!(output.canonical(), input.canonical());
    assert!(report.is_exact());
    assert_ne!(
        report.input(),
        digest(
            input.canonical().canonical_bytes(),
            KirBridgeCanonicalVersionV1::V11
        )
        .unwrap(),
    );
    assert!(output_storage.retained_storage() > output.canonical().canonical_bytes().len());
    assert!(matches!(
        preflight(&source),
        Err(KirBridgeErrorV1::UnsupportedType)
    ));
    drop(graph);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), STORAGE_FLOOR);
}

#[test]
fn dce_and_cse_keep_all_conservative_v12_families_and_live_ssa_types() {
    let source = carriers();
    let input = owner(&source);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(AMPLE_WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, AMPLE_STORAGE);
    let (mut graph, storage) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    // Exercise the existing closed algorithms directly in this crate-local
    // transport test; production's metered V12 executor has a separate endpoint.
    let plan = crate::PlironOptimizationPlanV1::new(
        vec![
            crate::PlironOptimizationPassV1::DeadCodeElimination,
            crate::PlironOptimizationPassV1::LocalPureCommonSubexpressionElimination,
        ],
        crate::PlironOptimizationLimitsV1::default(),
    )
    .unwrap();
    let root = graph.root.clone();
    graph.session.execute_optimization_v1(&root, &plan).unwrap();
    let (output, _, _) = graph
        .extract_optimized_canonical_kir_module_v12(&mut budget)
        .unwrap();
    assert_eq!(output.module(), &source);
    drop(graph);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn v12_operand_remapping_preserves_layout_access_and_contract_payloads() {
    for operation in carriers().functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
    {
        let original = operation.kind.operands();
        let mapped: Vec<_> = original
            .iter()
            .map(|value| ValueId(value.0 + 100))
            .collect();
        let result = remap_preserved_operation(&operation.kind, mapped.clone()).unwrap();
        assert_eq!(result.operands(), mapped);
        match (&operation.kind, result) {
            (OperationKind::VectorLoad(before), OperationKind::VectorLoad(after)) => {
                assert_eq!(before.access, after.access)
            }
            (OperationKind::VectorStore(before), OperationKind::VectorStore(after)) => {
                assert_eq!(before.access, after.access)
            }
            (
                OperationKind::VectorLayoutConvert(before),
                OperationKind::VectorLayoutConvert(after),
            ) => assert_eq!(before.to, after.to),
            (
                OperationKind::VerificationContract(before),
                OperationKind::VerificationContract(after),
            ) => {
                let VerificationContractOperationV12::WorkgroupPipelineEvent {
                    contract, kind, ..
                } = before;
                let VerificationContractOperationV12::WorkgroupPipelineEvent {
                    contract: actual_contract,
                    kind: actual_kind,
                    ..
                } = after;
                assert_eq!(*contract, actual_contract);
                assert_eq!(*kind, actual_kind);
            }
            _ => panic!("unexpected V12 transport family"),
        }
        assert!(remap_preserved_operation(&operation.kind, Vec::new()).is_err());
    }
}

#[test]
fn import_exact_and_one_under_boundaries_preserve_prefix_and_failure_history() {
    let input = owner(&Module::new("m"));
    // Input37 + root tree3 + one envelope sentinel = volume41.
    // Work = census37 + (4*41*41 + 8*41) + digest(12+45+37) = 7183.
    // Storage = fixed4096 + 64*41 = 6720 logical payload units.
    const WORK: usize = 7183;
    const STORAGE: usize = 6720;
    assert_eq!(input.canonical().canonical_bytes().len(), 37);
    for seeded in [false, true] {
        for allowance in [0, 36, 37, 7088, 7089, WORK - 1, WORK, WORK + 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK_FLOOR + allowance);
            work.charge_work(WORK_FLOOR).unwrap();
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                STORAGE_FLOOR + STORAGE,
            );
            budget.reserve_storage(STORAGE_FLOOR).unwrap();
            if seeded {
                assert!(budget.charge_work(usize::MAX).is_err());
                assert!(budget.reserve_storage(usize::MAX).is_err());
            }
            let result = KirPlironGraphV12::import(&input, &mut budget);
            assert_eq!(budget.storage(), STORAGE_FLOOR);
            assert_eq!(budget.failed_storage(), seeded.then_some(usize::MAX));
            if allowance < WORK {
                assert!(matches!(
                    result,
                    Err(KirBridgeErrorV12::Resource(
                        CanonicalKernelIrVerificationResourceErrorV1::Work(_)
                    ))
                ));
            } else {
                assert_eq!(result.unwrap().1.retained_storage(), STORAGE);
                assert_eq!(budget.work(), WORK_FLOOR + WORK);
            }
            if allowance == WORK - 1 {
                assert_eq!(budget.work(), WORK_FLOOR + 7089);
            }
            if seeded {
                assert_eq!(work.failed_work(), Some(usize::MAX));
            } else if allowance < WORK {
                assert!(work.failed_work().unwrap() > WORK_FLOOR + allowance);
            } else {
                assert_eq!(work.failed_work(), None);
            }
        }
        for allowance in [0, STORAGE - 1, STORAGE] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK_FLOOR + WORK);
            work.charge_work(WORK_FLOOR).unwrap();
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                STORAGE_FLOOR + allowance,
            );
            budget.reserve_storage(STORAGE_FLOOR).unwrap();
            let result = KirPlironGraphV12::import(&input, &mut budget);
            assert_eq!(budget.storage(), STORAGE_FLOOR);
            if allowance < STORAGE {
                assert!(matches!(result, Err(KirBridgeErrorV12::Resource(
                    CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
                )) if error.actual() == STORAGE_FLOOR + STORAGE));
                assert_eq!(budget.work(), WORK_FLOOR + 7089);
                assert_eq!(budget.failed_storage(), Some(STORAGE_FLOOR + STORAGE));
            } else {
                assert!(result.is_ok());
            }
        }
    }
}

#[test]
fn larger_sources_and_arithmetic_denial_are_admitted_before_session_construction() {
    for length in [1, 17, 1024, 4096] {
        let input = owner(&Module::new("x".repeat(length)));
        let wire = 36 + length;
        let volume = wire + 3 + 1;
        let required_work = wire + 4 * volume * volume + 8 * volume + 12 + 45 + wire;
        let required_storage = 4096 + 64 * volume;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(required_work);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, required_storage);
        let (graph, receipt) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
        assert_eq!(budget.work(), required_work);
        assert_eq!(budget.peak_storage(), required_storage);
        assert_eq!(budget.storage(), 0);
        assert_eq!(receipt.retained_storage(), required_storage);
        drop(graph);
    }
    assert!(matches!(
        bridge_envelope_v12(usize::MAX, 1),
        Err(KirBridgeErrorV12::Resource(
            CanonicalKernelIrVerificationResourceErrorV1::Arithmetic
        ))
    ));
}

#[test]
fn failure_after_import_mutation_drops_session_before_returning_to_caller_floor() {
    let input = owner(&carriers());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(AMPLE_WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, AMPLE_STORAGE);
    budget.reserve_storage(STORAGE_FLOOR).unwrap();
    FAIL_IMPORT_AFTER_BUILD.with(|fail| fail.set(true));
    assert!(matches!(
        KirPlironGraphV12::import(&input, &mut budget),
        Err(KirBridgeErrorV12::Bridge(
            KirBridgeErrorV1::UpstreamPanicked
        ))
    ));
    assert_eq!(budget.storage(), STORAGE_FLOOR);
    assert!(budget.peak_storage() > STORAGE_FLOOR);
    assert_eq!(budget.failed_storage(), None);
    let (graph, _) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
    graph.validate_custody_v12().unwrap();
    drop(graph);
    assert_eq!(budget.storage(), STORAGE_FLOOR);
}

#[test]
fn post_mutation_extraction_failure_keeps_live_session_floor_and_canonical_input() {
    let source = carriers();
    let input = owner(&source);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(AMPLE_WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, AMPLE_STORAGE);
    budget.reserve_storage(STORAGE_FLOOR).unwrap();
    let (mut graph, receipt) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let pointer = *graph.origins.preserved_operations.keys().next().unwrap();
    let operation =
        Operation::get_op::<PreservedOperationOp>(pointer, &graph.session.context).unwrap();
    operation.set_attr_gpu_preserved_operation_kind(
        &graph.session.context,
        PreservedOperationKindAttr::Barrier,
    );
    assert!(matches!(
        graph.extract_optimized_canonical_kir_module_v12(&mut budget),
        Err(KirBridgeErrorV12::Bridge(KirBridgeErrorV1::MalformedGraph))
    ));
    assert_eq!(budget.storage(), STORAGE_FLOOR + receipt.retained_storage());
    assert_eq!(input.module(), &source);
    drop(graph);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), STORAGE_FLOOR);
}

#[test]
fn extraction_exact_and_one_under_limits_keep_graph_and_output_coexistence() {
    let input = owner(&Module::new("m"));
    let mut import_work = CanonicalKernelIrWorkBudgetV1::new(AMPLE_WORK);
    let mut import_budget =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut import_work, AMPLE_STORAGE);
    let (mut graph, graph_storage) = KirPlironGraphV12::import(&input, &mut import_budget).unwrap();
    // Census40 + opaque envelope7052 + connected canonical294 + digest94.
    const WORK: usize = 7480;
    const SCRATCH: usize = 6720;
    let canonical = std::mem::size_of::<VerifiedCanonicalKernelIrModuleV12>() + 37 + 1;
    let report = std::mem::size_of::<KirBridgeOptimizedReceiptV1>();
    let output_retained = canonical + report;
    let required_storage = SCRATCH + output_retained;
    for (work_allowance, storage_allowance) in [
        (WORK, required_storage),
        (WORK - 1, required_storage),
        (WORK, required_storage - 1),
    ] {
        let floor = STORAGE_FLOOR + graph_storage.retained_storage();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK_FLOOR + work_allowance);
        work.charge_work(WORK_FLOOR).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            floor + storage_allowance,
        );
        budget.reserve_storage(floor).unwrap();
        let result = graph.extract_canonical_kir_module_v12_o0(&mut budget);
        assert_eq!(budget.storage(), floor);
        if work_allowance < WORK {
            assert!(matches!(result, Err(KirBridgeErrorV12::Resource(
                CanonicalKernelIrVerificationResourceErrorV1::Work(error)
            )) if error.actual() == WORK_FLOOR + WORK));
            assert_eq!(budget.work(), WORK_FLOOR + 7386);
            assert_eq!(budget.peak_storage(), floor + SCRATCH + canonical);
        } else if storage_allowance < required_storage {
            assert!(matches!(result, Err(KirBridgeErrorV12::Resource(
                CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
            )) if error.actual() == floor + required_storage));
            assert_eq!(budget.work(), WORK_FLOOR + WORK);
            assert_eq!(budget.failed_storage(), Some(floor + required_storage));
        } else {
            let (output, exact, storage) = result.unwrap();
            assert_eq!(output.module(), input.module());
            assert!(exact.is_exact());
            assert_eq!(storage.retained_storage(), output_retained);
            assert_eq!(budget.work(), WORK_FLOOR + WORK);
            assert_eq!(budget.peak_storage(), floor + required_storage);
            drop(output);
            drop(exact);
        }
        assert_eq!(input.module(), &Module::new("m"));
    }
    drop(graph);
}

#[test]
fn v12_metadata_copy_contains_no_definition_bodies_or_redundant_signature_trees() {
    let mut source = carriers();
    source.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .reserve(8192);
    source.functions.push(Function::declaration(
        "external",
        Signature::new(
            vec![Type::pointer(
                Type::F32,
                AddressSpace::Global,
                AccessMode::ReadOnly,
            )],
            vec![Type::F32],
        ),
    ));
    let snapshot = source.clone();
    let metadata = module_metadata_v12(&source);
    assert_eq!(metadata.id, source.id);
    assert_eq!(metadata.kernels, source.kernels);
    assert_eq!(metadata.required_capabilities, source.required_capabilities);
    assert_eq!(metadata.functions.len(), 2);
    for (actual, expected) in metadata.functions.iter().zip(&source.functions) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.role, expected.role);
        assert_eq!(actual.required_capabilities, expected.required_capabilities);
        assert!(actual.body.is_none());
    }
    assert!(metadata.functions[0].signature.parameters.is_empty());
    assert!(metadata.functions[0].signature.results.is_empty());
    assert_eq!(
        metadata.functions[1].signature,
        source.functions[1].signature
    );
    assert_eq!(source, snapshot);
}

#[test]
fn v12_o0_rejects_lost_value_origins_even_when_fresh_ids_would_match() {
    let source = carriers();
    let input = owner(&source);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(AMPLE_WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, AMPLE_STORAGE);
    let (mut graph, storage) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let live = graph
        .origins
        .values
        .iter()
        .find_map(|(live, id)| (*id == ValueId(3)).then_some(*live))
        .unwrap();
    graph.origins.values.remove(&live);
    assert!(matches!(
        graph.extract_canonical_kir_module_v12_o0(&mut budget),
        Err(KirBridgeErrorV12::Bridge(
            KirBridgeErrorV1::NonExactRoundTrip
        ))
    ));
    assert_eq!(budget.storage(), storage.retained_storage());
    // Optimized extraction may deterministically assign a fresh value ID. Its
    // semantic body still comes exclusively from the live graph.
    let (output, _, _) = graph
        .extract_optimized_canonical_kir_module_v12(&mut budget)
        .unwrap();
    assert_eq!(output.module(), &source);
    drop(graph);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}

fn interleaved_functions(count: u32) -> Module {
    let mut module = Module::new("interleaved-functions");
    for index in 0..count {
        module.functions.push(Function::declaration(
            format!("external-{index}"),
            Signature::new(vec![Type::F32], vec![Type::F32]),
        ));
        let mut body = KirBlock::new(BlockId(0));
        body.operations.push(KirOperation::new(
            vec![ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32))],
            OperationKind::Constant(Constant::U32(index)),
        ));
        body.terminator = Some(Terminator::Return {
            values: vec![ValueId(0)],
        });
        module.functions.push(Function::internal_helper(
            format!("definition-{index}"),
            Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
            vec![],
            vec![body],
        ));
    }
    module
}

#[test]
fn v12_function_index_visits_each_live_definition_once_and_preserves_roster_order() {
    for count in [0, 1, 8, 32] {
        let source = interleaved_functions(count);
        let input = owner(&source);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(AMPLE_WORK);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, AMPLE_STORAGE);
        let (graph, _) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
        let context = &graph.session.context;
        let root = graph.session.operations[&graph.root.identity];
        let block = root
            .deref(context)
            .get_region(0)
            .deref(context)
            .iter(context)
            .next()
            .unwrap();
        let live: Vec<_> = block.deref(context).iter(context).collect();
        let visits = std::cell::Cell::new(0);
        let indexed = index_live_functions(
            live.iter()
                .rev()
                .copied()
                .inspect(|_| visits.set(visits.get() + 1)),
            &source,
            &graph.origins,
        )
        .unwrap();
        assert_eq!(visits.get(), count as usize);
        assert_eq!(indexed.len(), 2 * count as usize);
        for (index, live) in live.iter().enumerate() {
            assert_eq!(indexed[2 * index], None);
            assert_eq!(indexed[2 * index + 1], Some(*live));
        }
        for operation in live.iter().rev() {
            operation.unlink(context);
            operation.insert_at_back(block, context);
        }
        let (extracted, correspondence) = extract_optimized_module_graph(
            context,
            root,
            &source,
            &graph.origins,
            KirBridgeTypeProfileV12::V12,
        )
        .unwrap();
        assert_eq!(extracted, source);
        assert_eq!(correspondence.len(), 4 * count as usize);
    }
}

#[test]
fn v12_function_index_rejects_incomplete_duplicate_unknown_and_declaration_origins() {
    let source = interleaved_functions(2);
    let input = owner(&source);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(AMPLE_WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, AMPLE_STORAGE);
    let (mut graph, _) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
    let context = &graph.session.context;
    let root = graph.session.operations[&graph.root.identity];
    let block = root
        .deref(context)
        .get_region(0)
        .deref(context)
        .iter(context)
        .next()
        .unwrap();
    let live: Vec<_> = block.deref(context).iter(context).collect();
    assert_eq!(live.len(), 2);
    for candidates in [
        vec![],
        vec![live[0]],
        vec![live[0], live[0]],
        vec![live[0], live[1], root],
    ] {
        assert_eq!(
            index_live_functions(candidates, &source, &graph.origins),
            Err(KirBridgeErrorV1::MalformedGraph),
        );
    }
    let original = graph.origins.functions[&live[1]];
    for invalid_origin in [0, 1, source.functions.len(), usize::MAX] {
        graph.origins.functions.insert(live[1], invalid_origin);
        assert_eq!(
            index_live_functions(live.iter().copied(), &source, &graph.origins),
            Err(KirBridgeErrorV1::MalformedGraph),
        );
    }
    graph.origins.functions.remove(&live[1]);
    assert_eq!(
        index_live_functions(live.iter().copied(), &source, &graph.origins),
        Err(KirBridgeErrorV1::MalformedGraph),
    );
    graph.origins.functions.insert(live[1], original);
    assert!(index_live_functions(live, &source, &graph.origins).is_ok());
}
