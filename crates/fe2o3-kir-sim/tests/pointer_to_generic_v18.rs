use fe2o3_kernel_ir::*;
use fe2o3_kir_sim::*;

fn pointer(space: AddressSpace) -> Type {
    Type::pointer(Type::Scalar(ScalarType::U32), space, AccessMode::ReadWrite)
}

fn graph(space: AddressSpace, lanes: u32) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    if space != AddressSpace::Global {
        let kind = if space == AddressSpace::Workgroup {
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32), extent: WorkgroupMemoryExtent::Static(1), alignment: 4,
            })
        } else {
            OperationKind::Alloca { element: Type::Scalar(ScalarType::U32), count: None, address_space: space, alignment: 4 }
        };
        block.operations.push(Operation::new(vec![ValueDef::new(ValueId(1), pointer(space))], kind));
    }
    block.operations.extend([
        Operation::new(vec![ValueDef::new(ValueId(2), pointer(AddressSpace::Generic))], OperationKind::Cast {
            kind: CastKind::PointerToGeneric, value: ValueId(if space == AddressSpace::Global { 0 } else { 1 }),
            to: pointer(AddressSpace::Generic),
        }),
        Operation::new(vec![ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))], OperationKind::Constant(Constant::U32(73))),
        Operation::new(vec![], OperationKind::Store { pointer: ValueId(2), value: ValueId(3), access: MemoryAccess::new(AddressSpace::Generic, 4) }),
        Operation::new(vec![ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32))], OperationKind::Load { pointer: ValueId(2), access: MemoryAccess::new(AddressSpace::Generic, 4) }),
        Operation::new(vec![], OperationKind::Store { pointer: ValueId(0), value: ValueId(4), access: MemoryAccess::new(AddressSpace::Global, 4) }),
    ]);
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("generic-exposure");
    module.functions.push(Function::kernel_entry("entry", Signature::new(vec![pointer(AddressSpace::Global)], vec![]), vec![ValueId(0)], vec![block]));
    let mut kernel = Kernel::new("entry", "entry", LaunchDomain::D1 { x: LaunchExtent::Static(lanes) });
    kernel.workgroup_size = Some(WorkgroupSize::new(lanes, 1, 1));
    module.kernels.push(kernel);
    module
}

fn admit(graph: &Module) -> VerifiedCanonicalKernelIrModuleV18 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(graph,
        StorageLayoutLimitsV1 { rows: 64, edges: 256, containment_depth: 32, object_bytes: 4096 }, &mut budget).unwrap().0
}

fn request(lanes: u32) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let buffer = BufferArgumentV1::from_scalars(AccessMode::ReadWrite, 4, &[ScalarBitsV1::u32(0)], target).unwrap();
    SimulationRequestV1::new("entry", [u64::from(lanes), 1, 1], [lanes, 1, 1], vec![SimulationArgumentV1::Buffer(buffer)])
}

fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 { max_call_depth: 8, max_ssa_values: 64, max_memory_access_records: 128, ..SimulationLimitsV1::default() }
}

#[test]
fn actual_v18_scalar_execution_retains_owner_and_cpu_only_classification() {
    for space in [AddressSpace::Global, AddressSpace::Private, AddressSpace::Workgroup] {
        let owner = admit(&graph(space, 1));
        let bytes = owner.canonical_bytes().to_vec();
        let request = request(1);
        let before = request.clone();
        let result = simulate_canonical_storage_v18(&owner, &request, SimulationTargetV1::amdgpu_64(), limits()).unwrap();
        assert_eq!(u32::from_le_bytes(result.buffer(0).unwrap().bytes().try_into().unwrap()), 73);
        assert_eq!(result.identity(), owner.identity());
        assert!(!result.grants_execution_authority());
        assert!(result.schedule_coverage().is_complete());
        assert_eq!(request, before);
        assert_eq!(owner.canonical_bytes(), bytes);
    }
}

#[test]
fn exposed_global_accesses_still_participate_in_race_tracking() {
    for space in [AddressSpace::Global] {
        let mut graph = graph(space, 2);
        // Remove the ordinary output store: the observed conflict must arise
        // from accesses through the exposed pointer itself.
        graph.functions[0].body.as_mut().unwrap().blocks[0].operations.pop();
        let owner = admit(&graph);
        let result = simulate_canonical_storage_v18(&owner, &request(2), SimulationTargetV1::amdgpu_64(), limits()).unwrap();
        assert!(matches!(result.race_assessment(), SimulationRaceAssessmentV1::RacesObserved { .. }));
    }
}

#[test]
fn generic_kernel_buffer_abi_is_not_enabled_by_the_new_internal_cast() {
    let mut graph = graph(AddressSpace::Global, 1);
    graph.functions[0].signature.parameters[0] = pointer(AddressSpace::Generic);
    let operations = &mut graph.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.remove(0);
    for operation in operations {
        match &mut operation.kind {
            OperationKind::Load { pointer, .. } | OperationKind::Store { pointer, .. } if *pointer == ValueId(2) => *pointer = ValueId(0),
            _ => {}
        }
        if let OperationKind::Store { access, .. } = &mut operation.kind { access.address_space = AddressSpace::Generic; }
    }
    let owner = admit(&graph);
    assert!(matches!(simulate_canonical_storage_v18(&owner, &request(1), SimulationTargetV1::amdgpu_64(), limits()), Err(SimulationErrorV1::Preflight(_))));
}

#[test]
fn exposure_has_exact_step_and_resident_limits_without_mutating_inputs() {
    let owner = admit(&graph(AddressSpace::Global, 1));
    let request = request(1);
    let before = request.clone();
    let target = SimulationTargetV1::amdgpu_64();
    let baseline = simulate_canonical_storage_v18(&owner, &request, target, limits()).unwrap();
    let steps = baseline.steps_executed();
    assert!(steps >= 6);
    for exact in [true, false] {
        let mut limits = limits();
        limits.max_steps = steps - u64::from(!exact);
        assert_eq!(simulate_canonical_storage_v18(&owner, &request, target, limits).is_ok(), exact);
    }
    let mut low = 1;
    let mut high = limits().max_resident_bytes;
    while low < high {
        let middle = low + (high - low) / 2;
        let mut limits = limits();
        limits.max_resident_bytes = middle;
        if simulate_canonical_storage_v18(&owner, &request, target, limits).is_ok() { high = middle; }
        else { low = middle + 1; }
    }
    assert!(low > owner.canonical_bytes().len());
    for exact in [true, false] {
        let mut limits = limits();
        limits.max_resident_bytes = low - usize::from(!exact);
        assert_eq!(simulate_canonical_storage_v18(&owner, &request, target, limits).is_ok(), exact);
    }
    assert_eq!(request, before);
}
