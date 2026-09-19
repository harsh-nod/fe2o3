//! Synthetic KIR actually executed by the shared simulator; never source evidence.
use super::*;
use crate::{DebugTranscriptV1, DebugWaveWidthV1, TranscriptCollectorV1};
use fe2o3_kernel_ir::*;
use fe2o3_kir_sim::*;

pub(super) fn work() -> ReplayWork {
    ReplayWork::new(1_000_000).unwrap()
}
pub(super) fn limits() -> LifecycleLimits {
    LifecycleLimits::new(
        128,
        size_of::<Ledger>() + 128 * size_of::<Transition>(),
        100_000,
    )
    .unwrap()
}
pub(super) fn config(records: usize) -> capture::Configuration {
    capture::Configuration {
        target: super::super::fixtures::TARGET,
        simulation: super::super::fixtures::simulation_limits(),
        debugger: super::super::fixtures::debugger_limits(records),
        capture: SimulationDebugCaptureLimitsV1::new(8, 32, 16, 512).unwrap(),
        origins: Some(super::super::fixtures::origin_limits(records)),
        lifecycle: limits(),
        width: DebugWaveWidthV1::Wave64,
    }
}
pub(super) fn schedule(seeded: bool) -> SimulationScheduleRequestV1<'static> {
    if seeded {
        SimulationScheduleRequestV1::RecordSeeded {
            seed: 71,
            max_decisions: 512,
        }
    } else {
        SimulationScheduleRequestV1::RecordCanonical { max_decisions: 512 }
    }
}
pub(super) fn admit(mut module: Module, limits: SimulationLimitsV1) -> AdmittedSimulationModuleV1 {
    let capabilities = module
        .functions
        .iter()
        .flat_map(Function::derived_capabilities)
        .collect::<std::collections::BTreeSet<_>>();
    for function in &mut module.functions {
        function.required_capabilities = capabilities.clone();
    }
    for kernel in &mut module.kernels {
        kernel.required_capabilities = capabilities.clone();
    }
    module.required_capabilities = capabilities;
    AdmittedSimulationModuleV1::admit_v9(
        VerifiedCanonicalKernelIrV9::from_module(module).unwrap(),
        limits,
    )
    .unwrap()
}
fn alloca(id: u32, space: AddressSpace, count: bool) -> Operation {
    let scalar = Type::Scalar(ScalarType::U32);
    Operation::effect_free(
        ValueDef::new(
            ValueId(id),
            Type::pointer(scalar.clone(), space, AccessMode::ReadWrite),
        ),
        if space == AddressSpace::Workgroup {
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: scalar,
                extent: WorkgroupMemoryExtent::Static(1),
                alignment: 4,
            })
        } else {
            OperationKind::Alloca {
                element: scalar,
                count: count.then_some(ValueId(12)),
                address_space: space,
                alignment: 4,
            }
        },
    )
}
pub(super) fn private(
    fault: bool,
    zero: bool,
) -> (AdmittedSimulationModuleV1, SimulationRequestV1) {
    let (old, mut request) = super::super::fixtures::memory();
    let mut module = old.module().clone();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let call = || {
        Operation::new(
            vec![],
            OperationKind::Call {
                callee: "allocation_helper".into(),
                arguments: vec![],
            },
        )
    };
    block.operations.insert(0, call());
    block.operations.insert(1, call());
    block
        .operations
        .push(alloca(40, AddressSpace::Private, false));
    let mut helper = BasicBlock::new(BlockId(19));
    helper.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(12), Type::INDEX),
            OperationKind::Constant(Constant::Index(if zero { 0 } else { 1 })),
        ),
        alloca(10, AddressSpace::Private, true),
    ];
    if !zero {
        helper.operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(11), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(7)),
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(10),
                    value: ValueId(11),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(13), Type::Scalar(ScalarType::U32)),
                OperationKind::Load {
                    pointer: ValueId(10),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        ]);
    }
    helper.terminator = Some(if fault {
        Terminator::Unreachable
    } else {
        Terminator::Return { values: vec![] }
    });
    module.functions.push(Function::internal_helper(
        "allocation_helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![helper],
    ));
    request.arguments[0] = SimulationArgumentV1::Buffer(
        BufferArgumentV1::from_scalars(
            AccessMode::ReadWrite,
            4,
            &[
                ScalarBitsV1::u32(0),
                ScalarBitsV1::u32(0xdeadbeef),
                ScalarBitsV1::u32(0xcafebabe),
            ],
            super::super::fixtures::TARGET,
        )
        .unwrap(),
    );
    request.events = EventPolicyV1::Enabled;
    (admit(module, config(4096).simulation), request)
}
pub(super) fn workgroups() -> (AdmittedSimulationModuleV1, SimulationRequestV1) {
    let (old, mut request) = super::super::fixtures::barrier();
    let mut module = old.module().clone();
    module.functions[1].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(0, alloca(50, AddressSpace::Workgroup, false));
    request.grid.0 = [4, 1, 1];
    request.events = EventPolicyV1::Enabled;
    let mut limits = config(4096).simulation;
    limits.max_invocations = 4;
    limits.max_workgroups = 2;
    (admit(module, limits), request)
}
pub(super) fn baseline(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    seeded: bool,
    config: capture::Configuration,
) -> (
    Result<SimulationExecutionV1, SimulationErrorV1>,
    DebugTranscriptV1,
) {
    let mut collector = TranscriptCollectorV1::new(config.debugger);
    let result = module.simulate_debugged_scheduled_with_sink(
        request,
        config.target,
        config.simulation,
        schedule(seeded),
        config.capture,
        &mut collector,
    );
    let fault = match &result {
        Err(SimulationErrorV1::Execution(error)) => Some(crate::DebugTerminalFaultV1 {
            ordinal: collector.records.len() as u64,
            invocation: error.invocation,
            site: error.site.clone(),
            kind: error.kind.clone(),
        }),
        _ => None,
    };
    let transcript = collector.into_transcript(
        super::super::fixtures::identity(module),
        config.width,
        fault,
    );
    (result, transcript)
}

pub(super) fn failure_shape(shape: usize) -> (AdmittedSimulationModuleV1, SimulationRequestV1) {
    let (old, request) = private(true, false);
    let mut module = old.module().clone();
    if shape == 0 {
        let entry = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
        entry.operations.drain(..2);
        entry.terminator = Some(Terminator::Unreachable);
        module.functions.truncate(1);
    } else if shape == 2 {
        let helper = &mut module.functions[1].body.as_mut().unwrap().blocks[0];
        helper.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: "allocation_helper".into(),
                arguments: vec![],
            },
        ));
        helper.terminator = Some(Terminator::Return { values: vec![] });
    } else {
        assert_eq!(shape, 1, "bounded direct/nested/recursive fixture");
    }
    (admit(module, config(4096).simulation), request)
}
