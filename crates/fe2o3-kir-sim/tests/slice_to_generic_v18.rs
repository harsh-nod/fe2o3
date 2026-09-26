use fe2o3_kernel_ir::*;
use fe2o3_kir_sim::*;

fn slice(space: AddressSpace) -> Type {
    Type::slice(Type::Scalar(ScalarType::U32), space, AccessMode::ReadWrite)
}
fn pointer(space: AddressSpace) -> Type {
    Type::pointer(Type::Scalar(ScalarType::U32), space, AccessMode::ReadWrite)
}
fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn graph(transport: bool, lanes: u32) -> Module {
    let generic = slice(AddressSpace::Generic);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            2,
            generic.clone(),
            OperationKind::Cast {
                kind: CastKind::SliceToGeneric,
                value: ValueId(0),
                to: generic.clone(),
            },
        ),
        op(
            3,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(2) },
        ),
    ];
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let mut blocks = vec![entry];
    let mut helper = BasicBlock::new(BlockId(0));
    helper.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    if transport {
        blocks[0].operations.push(op(
            4,
            generic.clone(),
            OperationKind::Call {
                callee: FunctionId::new("identity"),
                arguments: vec![ValueId(2)],
            },
        ));
        blocks[0].terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(4)],
        });
        let mut next = BasicBlock::new(BlockId(1));
        next.parameters
            .push(ValueDef::new(ValueId(5), generic.clone()));
        next.operations = vec![
            op(6, Type::BOOL, OperationKind::Constant(Constant::Bool(true))),
            op(
                7,
                generic.clone(),
                OperationKind::Select {
                    condition: ValueId(6),
                    true_value: ValueId(5),
                    false_value: ValueId(5),
                },
            ),
            op(
                8,
                pointer(AddressSpace::Generic),
                OperationKind::SliceData { slice: ValueId(7) },
            ),
            op(
                9,
                Type::Scalar(ScalarType::U32),
                OperationKind::Constant(Constant::U32(73)),
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(8),
                    value: ValueId(9),
                    access: MemoryAccess::new(AddressSpace::Generic, 4),
                },
            ),
            op(
                10,
                Type::Scalar(ScalarType::U32),
                OperationKind::Load {
                    pointer: ValueId(8),
                    access: MemoryAccess::new(AddressSpace::Generic, 4),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(1),
                    value: ValueId(10),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ];
        next.terminator = Some(Terminator::Return { values: vec![] });
        blocks.push(next);
    }
    let mut module = Module::new("slice-sim");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![slice(AddressSpace::Global), pointer(AddressSpace::Global)],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        blocks,
    ));
    if transport {
        module.functions.push(Function::internal_helper(
            "identity",
            Signature::new(vec![generic.clone()], vec![generic]),
            vec![ValueId(0)],
            vec![helper],
        ));
    }
    let mut kernel = Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(lanes),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(lanes, 1, 1));
    module.kernels.push(kernel);
    module
}
fn admit(graph: &Module) -> VerifiedCanonicalKernelIrModuleV18 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        graph,
        StorageLayoutLimitsV1 {
            rows: 64,
            edges: 256,
            containment_depth: 32,
            object_bytes: 4096,
        },
        &mut budget,
    )
    .unwrap()
    .0
}
fn request(empty: bool, lanes: u32) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let input = if empty {
        BufferArgumentV1::new(
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            vec![],
            vec![],
            target,
        )
        .unwrap()
    } else {
        BufferArgumentV1::from_scalars(
            AccessMode::ReadWrite,
            4,
            &[ScalarBitsV1::u32(11)],
            target,
        )
        .unwrap()
    };
    if empty {
        assert_eq!(input.element(), ScalarType::U32);
        assert_eq!(input.element_count(target).unwrap(), 0);
        assert!(input.bytes().is_empty());
        assert!(input.initialized().is_empty());
    }
    SimulationRequestV1::new(
        "entry",
        [u64::from(lanes), 1, 1],
        [lanes, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(input),
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::from_scalars(
                    AccessMode::ReadWrite,
                    4,
                    &[ScalarBitsV1::u32(0)],
                    target,
                )
                .unwrap(),
            ),
        ],
    )
}

#[test]
fn actual_v18_descriptor_survives_helper_return_select_block_arguments_and_memory_access() {
    let owner = admit(&graph(true, 1));
    let bytes = owner.canonical_bytes().to_vec();
    let request = request(false, 1);
    let before = request.clone();
    let result = simulate_canonical_storage_v18(
        &owner,
        &request,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
    )
    .unwrap();
    for index in [0, 1] {
        assert_eq!(
            u32::from_le_bytes(result.buffer(index).unwrap().bytes().try_into().unwrap()),
            73
        );
    }
    assert_eq!(result.identity(), owner.identity());
    assert!(!result.grants_execution_authority());
    assert!(result.schedule_coverage().is_complete());
    assert_eq!(request, before);
    assert_eq!(owner.canonical_bytes(), bytes);
}

#[test]
fn zero_length_widening_is_not_an_access_and_has_three_independent_steps() {
    let owner = admit(&graph(false, 1));
    // Cast, SliceLength, Return; scalar-element type checks perform no storage
    // row lookup and the cast introduces no eager range/lifetime check.
    const STEPS: u64 = 3;
    for empty in [false, true] {
        let request = request(empty, 1);
        for short in [false, true] {
            let result = simulate_canonical_storage_v18(
                &owner,
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1 {
                    max_steps: STEPS - u64::from(short),
                    ..SimulationLimitsV1::default()
                },
            );
            if short {
                assert!(matches!(
                    result,
                    Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                        kind: SimulationExecutionErrorKindV1::StepLimit { limit: 2 },
                        ..
                    }))
                ));
            } else {
                assert_eq!(result.unwrap().steps_executed(), STEPS);
            }
        }
    }
}

#[test]
fn widened_slice_accesses_still_participate_in_race_tracking() {
    let mut source = graph(true, 2);
    source.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .pop();
    let result = simulate_canonical_storage_v18(
        &admit(&source),
        &request(false, 2),
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
    )
    .unwrap();
    assert!(matches!(
        result.race_assessment(),
        SimulationRaceAssessmentV1::RacesObserved { .. }
    ));
}

#[test]
fn generic_kernel_descriptor_abi_is_not_granted_by_internal_widening() {
    let mut source = graph(false, 1);
    source.functions[0].signature.parameters[0] = slice(AddressSpace::Generic);
    let operations = &mut source.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.remove(0);
    operations[0].kind = OperationKind::SliceLength { slice: ValueId(0) };
    assert!(matches!(
        simulate_canonical_storage_v18(
            &admit(&source),
            &request(false, 1),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default()
        ),
        Err(SimulationErrorV1::Preflight(_))
    ));
}
