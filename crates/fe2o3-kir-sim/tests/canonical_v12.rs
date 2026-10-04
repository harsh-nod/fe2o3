use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CastKind, CheckedBinaryOperator, Constant,
    FixedVectorTypeV12, Function, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module,
    Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    VectorLayoutConversionV12, VectorLayoutV12, VectorLoadOperationV12, VectorMemoryAccessV12,
    VectorStoreOperationV12, VerificationContractKeyV12, VerificationContractOperationV12,
    VerifiedCanonicalKernelIrV11, VerifiedCanonicalKernelIrV12, WorkgroupMemory,
    WorkgroupMemoryExtent, WorkgroupPipelineEventKindV12, WorkgroupSize,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, SimulationAdmissionErrorV1, SimulationArgumentV1,
    SimulationKernelIrIdentityV1, SimulationLimitsV1, SimulationPreflightErrorV1,
    SimulationRequestV1, SimulationTargetV1, UnsupportedFeatureV1,
};

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn module(parameters: Vec<Type>, operations: Vec<Operation>) -> Module {
    let ids = (0..parameters.len()).map(|id| ValueId(id as u32)).collect();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("sim-tests::canonical-v12");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(parameters, vec![]),
        ids,
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    module.kernels.push(kernel);
    module
}

fn memory_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let restricted = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    module(
        vec![pointer.clone(), pointer.clone()],
        vec![
            op(
                2,
                restricted.clone(),
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value: ValueId(0),
                    to: restricted,
                },
            ),
            op(
                3,
                scalar.clone(),
                OperationKind::Load {
                    pointer: ValueId(2),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            op(4, scalar.clone(), OperationKind::Constant(Constant::U32(9))),
            Operation::checked_binary(
                ValueDef::new(ValueId(5), scalar.clone()),
                ValueDef::new(ValueId(6), Type::BOOL),
                CheckedBinaryOperator::Add,
                ValueId(3),
                ValueId(4),
            ),
            op(7, scalar.clone(), OperationKind::Constant(Constant::U32(0))),
            op(
                8,
                scalar,
                OperationKind::Select {
                    condition: ValueId(6),
                    true_value: ValueId(7),
                    false_value: ValueId(5),
                },
            ),
            op(9, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
            op(
                10,
                pointer,
                OperationKind::GetElementPointer {
                    base: ValueId(1),
                    offset: ValueId(9),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(10),
                    value: ValueId(8),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ],
    )
}

fn buffer(element: ScalarType, alignment: u32, bytes: Vec<u8>) -> SimulationArgumentV1 {
    let initialized = vec![true; bytes.len()];
    SimulationArgumentV1::Buffer(
        BufferArgumentV1::new(
            element,
            AccessMode::ReadWrite,
            alignment,
            bytes,
            initialized,
            SimulationTargetV1::amdgpu_64(),
        )
        .unwrap(),
    )
}

fn request(arguments: Vec<SimulationArgumentV1>) -> SimulationRequestV1 {
    SimulationRequestV1::new("entry", [1, 1, 1], [1, 1, 1], arguments)
}

fn canonical() -> VerifiedCanonicalKernelIrV12 {
    VerifiedCanonicalKernelIrV12::from_module(memory_module()).unwrap()
}

#[test]
fn exact_v12_identity_is_retained_without_projecting_to_v11() {
    let canonical = canonical();
    let expected = SimulationKernelIrIdentityV1::from(*canonical.identity());
    let old = VerifiedCanonicalKernelIrV11::from_module(memory_module()).unwrap();
    assert_eq!(expected.wire_version(), 12);
    assert_eq!(expected.digest(), canonical.identity().digest());
    assert_eq!(
        expected.canonical_length(),
        canonical.canonical_bytes().len() as u64
    );
    assert_ne!(expected.digest(), old.identity().digest());
    assert!(
        VerifiedCanonicalKernelIrV12::from_canonical_bytes(old.canonical_bytes().to_vec(),)
            .is_err()
    );

    let admitted =
        AdmittedSimulationModuleV1::admit_v12(canonical, SimulationLimitsV1::default()).unwrap();
    assert_eq!(admitted.identity(), &expected);
    assert_eq!(admitted.module(), &memory_module());
}

#[test]
fn exact_v12_executes_scalar_checked_arithmetic_and_pointer_restriction() {
    let admitted =
        AdmittedSimulationModuleV1::admit_v12(canonical(), SimulationLimitsV1::default()).unwrap();
    for input in [0_u32, 7, u32::MAX - 9, u32::MAX - 8, u32::MAX] {
        let request = request(vec![
            buffer(ScalarType::U32, 4, input.to_le_bytes().to_vec()),
            buffer(
                ScalarType::U32,
                4,
                [0xa5a5_a5a5_u32; 3]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            ),
        ]);
        let original = request.clone();
        let result = admitted
            .simulate(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
        let expected = input.checked_add(9).unwrap_or(0);
        assert_eq!(request, original);
        assert_eq!(result.buffer(0).unwrap().bytes(), input.to_le_bytes());
        assert_eq!(
            result.buffer(1).unwrap().bytes(),
            [0xa5a5_a5a5_u32, expected, 0xa5a5_a5a5]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn exact_v12_admission_preserves_canonical_and_resident_limits() {
    let bytes = canonical().canonical_bytes().len();
    let limits = SimulationLimitsV1 {
        max_canonical_bytes: bytes,
        ..SimulationLimitsV1::default()
    };
    AdmittedSimulationModuleV1::admit_v12(canonical(), limits).unwrap();
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v12(canonical(), SimulationLimitsV1 {
            max_canonical_bytes: bytes - 1, ..limits
        }),
        Err(SimulationAdmissionErrorV1::CanonicalBytesLimit { actual, limit })
            if actual == bytes && limit == bytes - 1
    ));

    let error = AdmittedSimulationModuleV1::admit_v12(
        canonical(),
        SimulationLimitsV1 {
            max_resident_bytes: 1,
            ..limits
        },
    )
    .unwrap_err();
    let SimulationAdmissionErrorV1::ResidentBytesLimit {
        actual, limit: 1, ..
    } = error
    else {
        panic!("unexpected resident refusal: {error:?}");
    };
    AdmittedSimulationModuleV1::admit_v12(
        canonical(),
        SimulationLimitsV1 {
            max_resident_bytes: actual,
            ..limits
        },
    )
    .unwrap();
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v12(canonical(), SimulationLimitsV1 {
            max_resident_bytes: actual - 1, ..limits
        }),
        Err(SimulationAdmissionErrorV1::ResidentBytesLimit { actual: observed, limit, .. })
            if observed == actual && limit == actual - 1
    ));
}

fn assert_inert(module: Module, request: SimulationRequestV1, operations: &[u32]) {
    let admitted = AdmittedSimulationModuleV1::admit_v12(
        VerifiedCanonicalKernelIrV12::from_module(module).unwrap(),
        SimulationLimitsV1::default(),
    )
    .unwrap();
    let error = admitted
        .preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    let SimulationPreflightErrorV1::Unsupported(report) = error else {
        panic!("unexpected inert-carrier refusal: {error:?}");
    };
    let sites = report
        .findings()
        .iter()
        .filter(|finding| finding.feature == UnsupportedFeatureV1::InertV12Carrier)
        .map(|finding| {
            assert_eq!(finding.function.as_str(), "entry");
            assert_eq!(finding.block, Some(BlockId(0)));
            finding.operation.unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(sites, operations);
}

#[test]
fn exact_v12_vectors_remain_refused_after_canonical_admission() {
    let contiguous = FixedVectorTypeV12::new(ScalarType::F32, 4, VectorLayoutV12::Contiguous);
    let interleaved = FixedVectorTypeV12::new(
        ScalarType::F32,
        4,
        VectorLayoutV12::Interleaved { factor: 2 },
    );
    let pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let module = module(
        vec![pointer],
        vec![
            op(
                1,
                Type::vector(contiguous),
                OperationKind::VectorLoad(VectorLoadOperationV12::new(
                    ValueId(0),
                    VectorMemoryAccessV12::new(
                        contiguous,
                        MemoryAccess::new(AddressSpace::Global, 16),
                    ),
                )),
            ),
            op(
                2,
                Type::vector(interleaved),
                OperationKind::VectorLayoutConvert(VectorLayoutConversionV12::new(
                    ValueId(1),
                    interleaved.layout,
                )),
            ),
            Operation::new(
                vec![],
                OperationKind::VectorStore(VectorStoreOperationV12::new(
                    ValueId(0),
                    ValueId(2),
                    VectorMemoryAccessV12::new(
                        interleaved,
                        MemoryAccess::new(AddressSpace::Global, 16),
                    ),
                )),
            ),
        ],
    );
    assert_inert(
        module,
        request(vec![buffer(ScalarType::F32, 16, vec![0; 16])]),
        &[0, 1, 2],
    );
}

#[test]
fn exact_v12_verification_events_remain_refused_after_canonical_admission() {
    let mut operations = vec![
        op(
            0,
            Type::pointer(Type::F32, AddressSpace::Workgroup, AccessMode::ReadWrite),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::F32,
                extent: WorkgroupMemoryExtent::Static(4),
                alignment: 4,
            }),
        ),
        op(1, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
    ];
    for kind in [
        WorkgroupPipelineEventKindV12::Stage,
        WorkgroupPipelineEventKindV12::Commit,
        WorkgroupPipelineEventKindV12::Wait,
        WorkgroupPipelineEventKindV12::Consume,
        WorkgroupPipelineEventKindV12::Discard,
        WorkgroupPipelineEventKindV12::Release,
    ] {
        operations.push(Operation::new(
            vec![],
            OperationKind::VerificationContract(
                VerificationContractOperationV12::WorkgroupPipelineEvent {
                    contract: VerificationContractKeyV12::new(17),
                    kind,
                    storage: ValueId(0),
                    epoch: ValueId(1),
                },
            ),
        ));
    }
    assert_inert(
        module(vec![], operations),
        request(vec![]),
        &[2, 3, 4, 5, 6, 7],
    );
}
