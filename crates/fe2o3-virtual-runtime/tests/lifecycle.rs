use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, Constant, Function, IntrinsicOperation, Kernel,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType,
    Signature, Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV7, WorkgroupSize,
};
use fe2o3_kir_sim::{AdmittedSimulationModuleV1, SimulationLimitsV1};
use fe2o3_runtime_model::{IdentityDigestV1, TransitionErrorV1};
use fe2o3_virtual_runtime::{
    VirtualArgumentV1, VirtualBufferAccessV1, VirtualCompletionStateV1, VirtualDispatchRequestV1,
    VirtualRunProgressV1, VirtualRuntimeConfigV1, VirtualRuntimeErrorV1, VirtualRuntimeLimitsV1,
    VirtualRuntimeV1, VirtualTargetProfileV1,
};

fn operation(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn admitted_fill() -> AdmittedSimulationModuleV1 {
    let element = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(element.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        operation(
            1,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        operation(
            2,
            pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(1),
            },
        ),
        operation(3, element, OperationKind::Constant(Constant::U32(17))),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "fill_impl",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut kernel = Kernel::new(
        "fill",
        "fill_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("virtual-runtime-tests::fill");
    module.functions.push(entry);
    module.kernels.push(kernel);
    let canonical = VerifiedCanonicalKernelIrV7::from_module(module).unwrap();
    AdmittedSimulationModuleV1::admit(canonical, SimulationLimitsV1::default()).unwrap()
}

fn runtime(seed: u8) -> VirtualRuntimeV1 {
    VirtualRuntimeV1::new(VirtualRuntimeConfigV1 {
        runtime_identity: IdentityDigestV1::from_untrusted_bytes([seed; 32]),
        target: VirtualTargetProfileV1::Amdgpu64TargetNeutral,
        runtime_limits: VirtualRuntimeLimitsV1::default(),
        simulation_limits: SimulationLimitsV1::default(),
    })
    .unwrap()
}

fn fill_request(
    buffer: fe2o3_virtual_runtime::VirtualBufferHandleV1,
    elements: usize,
    dependencies: Vec<fe2o3_virtual_runtime::VirtualCompletionHandleV1>,
) -> VirtualDispatchRequestV1 {
    VirtualDispatchRequestV1 {
        kernel: "fill".into(),
        grid: [elements as u64, 1, 1],
        workgroup: [64, 1, 1],
        arguments: vec![VirtualArgumentV1::Buffer {
            buffer,
            element: ScalarType::U32,
            access: AccessMode::ReadWrite,
            alignment: 4,
            byte_offset: 0,
            elements,
        }],
        dependencies,
    }
}

#[test]
fn serial_dependencies_execute_and_copy_back_typed_bytes() {
    let mut runtime = runtime(1);
    let module = runtime.register_module(admitted_fill()).unwrap();
    let queue = runtime.create_queue(8).unwrap();
    let buffer = runtime
        .allocate_buffer(16, VirtualBufferAccessV1::ReadWrite)
        .unwrap();
    runtime.copy_from_host(buffer, 0, &[0; 16]).unwrap();
    let first = runtime
        .submit(queue, module, fill_request(buffer, 4, vec![]))
        .unwrap();
    let second = runtime
        .submit(queue, module, fill_request(buffer, 4, vec![first]))
        .unwrap();

    assert!(matches!(
        runtime.run_next().unwrap(),
        VirtualRunProgressV1::Completed { completion, .. } if completion == first
    ));
    assert!(matches!(
        runtime.run_next().unwrap(),
        VirtualRunProgressV1::Completed { completion, .. } if completion == second
    ));
    assert_eq!(
        runtime.completion_state(second).unwrap(),
        VirtualCompletionStateV1::Completed
    );
    let mut output = [0; 16];
    runtime.copy_to_host(buffer, 0, &mut output).unwrap();
    assert_eq!(output, [17, 0, 0, 0, 17, 0, 0, 0, 17, 0, 0, 0, 17, 0, 0, 0]);
    let summary = runtime.completion_summary(second).unwrap().unwrap();
    assert_eq!(summary.invocations_executed, 4);
    assert!(!runtime.grants_hardware_authority());
    assert!(!runtime.predicts_performance());
}

#[test]
fn dynamic_failure_aborts_dependents_without_fabricating_completion() {
    let mut runtime = runtime(2);
    let module = runtime.register_module(admitted_fill()).unwrap();
    let queue = runtime.create_queue(8).unwrap();
    let buffer = runtime
        .allocate_buffer(4, VirtualBufferAccessV1::ReadWrite)
        .unwrap();
    runtime.copy_from_host(buffer, 0, &[0; 4]).unwrap();
    let failing = runtime
        .submit(queue, module, fill_request(buffer, 2, vec![]))
        .unwrap();
    let dependent = runtime
        .submit(queue, module, fill_request(buffer, 1, vec![failing]))
        .unwrap();

    assert!(matches!(
        runtime.run_next(),
        Err(VirtualRuntimeErrorV1::Simulation { completion, .. }) if completion == failing
    ));
    assert_eq!(
        runtime.completion_state(failing).unwrap(),
        VirtualCompletionStateV1::AbortedSimulation
    );
    assert!(matches!(
        runtime.run_next().unwrap(),
        VirtualRunProgressV1::AbortedDependency { completion, dependency }
            if completion == dependent && dependency == failing
    ));
}

#[test]
fn early_release_is_rejected_atomically_until_completion() {
    let mut runtime = runtime(3);
    let module = runtime.register_module(admitted_fill()).unwrap();
    let queue = runtime.create_queue(8).unwrap();
    let buffer = runtime
        .allocate_buffer(4, VirtualBufferAccessV1::ReadWrite)
        .unwrap();
    runtime.copy_from_host(buffer, 0, &[0; 4]).unwrap();
    runtime
        .submit(queue, module, fill_request(buffer, 1, vec![]))
        .unwrap();

    assert!(matches!(
        runtime.release_buffer(buffer),
        Err(VirtualRuntimeErrorV1::Model(
            TransitionErrorV1::ResourceInUse(_)
        ))
    ));
    runtime.run_next().unwrap();
    runtime.release_buffer(buffer).unwrap();
}

#[test]
fn ambiguous_completion_retains_resources_until_explicit_quiescence() {
    let mut runtime = runtime(4);
    let module = runtime.register_module(admitted_fill()).unwrap();
    let queue = runtime.create_queue(8).unwrap();
    let buffer = runtime
        .allocate_buffer(4, VirtualBufferAccessV1::ReadWrite)
        .unwrap();
    runtime.copy_from_host(buffer, 0, &[0; 4]).unwrap();
    let completion = runtime
        .submit(queue, module, fill_request(buffer, 1, vec![]))
        .unwrap();
    runtime.mark_completion_ambiguous(completion).unwrap();

    assert!(matches!(
        runtime.release_buffer(buffer),
        Err(VirtualRuntimeErrorV1::Model(
            TransitionErrorV1::ResourceInUse(_)
        ))
    ));
    assert!(matches!(
        runtime.settle_ambiguous_completion(completion),
        Err(VirtualRuntimeErrorV1::QueueNotQuiescent { .. })
    ));
    runtime.quiesce_queue(queue).unwrap();
    runtime.settle_ambiguous_completion(completion).unwrap();
    assert_eq!(
        runtime.completion_state(completion).unwrap(),
        VirtualCompletionStateV1::FailedQuiescent
    );
    runtime.release_buffer(buffer).unwrap();
}

#[test]
fn foreign_handles_and_uninitialized_reads_are_typed_failures() {
    let mut first = runtime(5);
    let mut second = runtime(6);
    let foreign = second
        .allocate_buffer(4, VirtualBufferAccessV1::ReadWrite)
        .unwrap();
    assert!(matches!(
        first.copy_from_host(foreign, 0, &[1]),
        Err(VirtualRuntimeErrorV1::ForeignHandle { kind: "buffer" })
    ));
    let local = first
        .allocate_buffer(4, VirtualBufferAccessV1::ReadWrite)
        .unwrap();
    let mut output = [0; 4];
    assert!(matches!(
        first.copy_to_host(local, 0, &mut output),
        Err(VirtualRuntimeErrorV1::UninitializedHostRead { offset: 0 })
    ));
    first.release_buffer(local).unwrap();
    assert!(matches!(
        first.copy_from_host(local, 0, &[1]),
        Err(VirtualRuntimeErrorV1::ReleasedHandle { kind: "buffer", .. })
    ));
}
