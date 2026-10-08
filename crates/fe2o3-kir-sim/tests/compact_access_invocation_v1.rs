use std::collections::BTreeSet;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, Constant, Function, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV7,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, EventPolicyV1, ScalarBitsV1,
    SimulationArgumentV1, SimulationConflictAssessmentV1, SimulationEventKindV1,
    SimulationEventSinkErrorV1, SimulationEventSinkV1, SimulationEventV1, SimulationInvocationV1,
    SimulationLimitsV1, SimulationRaceAssessmentV1, SimulationRequestV1, SimulationTargetV1,
};

fn module(through_call: bool) -> AdmittedSimulationModuleV1 {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut writes = BasicBlock::new(BlockId(0));
    writes.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(1), scalar),
            OperationKind::Constant(Constant::U32(42)),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    writes.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("sim-tests::compact-access-identity");
    if through_call {
        let mut caller = BasicBlock::new(BlockId(0));
        caller.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: "write_helper".into(),
                arguments: vec![ValueId(0)],
            },
        ));
        caller.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            "store_impl",
            Signature::new(vec![pointer.clone()], vec![]),
            vec![ValueId(0)],
            vec![caller],
        ));
        module.functions.push(Function::internal_helper(
            "write_helper",
            Signature::new(vec![pointer], vec![]),
            vec![ValueId(0)],
            vec![writes],
        ));
    } else {
        module.functions.push(Function::kernel_entry(
            "store_impl",
            Signature::new(vec![pointer], vec![]),
            vec![ValueId(0)],
            vec![writes],
        ));
    }
    module.kernels.push(Kernel::new(
        "store",
        "store_impl",
        LaunchDomain::D3 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Dynamic,
            z: LaunchExtent::Dynamic,
        },
    ));
    let canonical = VerifiedCanonicalKernelIrV7::from_module(module).expect("verified fixture");
    AdmittedSimulationModuleV1::admit(canonical, SimulationLimitsV1::default())
        .expect("admitted fixture")
}

#[derive(Default)]
struct Writes(Vec<SimulationInvocationV1>);

impl SimulationEventSinkV1 for Writes {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        if matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. }) {
            self.0.push(event.invocation);
        }
        Ok(())
    }
}

fn check_public_identity(through_call: bool) {
    let admitted = module(through_call);
    for (size, extent) in [
        ([1, 1, 1], [3, 2, 2]),
        ([2, 3, 4], [5, 7, 9]),
        ([4, 2, 1], [7, 3, 2]),
    ] {
        let buffer = BufferArgumentV1::from_scalars(
            AccessMode::ReadWrite,
            4,
            &[ScalarBitsV1::u32(0)],
            SimulationTargetV1::amdgpu_64(),
        )
        .unwrap();
        let mut request = SimulationRequestV1::new(
            "store",
            extent,
            size,
            vec![SimulationArgumentV1::Buffer(buffer)],
        );
        request.events = EventPolicyV1::Enabled;
        let mut writes = Writes::default();
        let execution = admitted
            .simulate_with_sink(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
                &mut writes,
            )
            .unwrap();
        let expected_count = extent
            .into_iter()
            .try_fold(1_u64, |count, axis| count.checked_mul(axis))
            .unwrap();
        assert_eq!(u64::try_from(writes.0.len()).unwrap(), expected_count);
        let coordinates: BTreeSet<_> = writes.0.iter().map(|v| v.global).collect();
        assert_eq!(coordinates.len(), writes.0.len());
        for invocation in &writes.0 {
            assert!(
                invocation
                    .global
                    .iter()
                    .zip(extent)
                    .all(|(at, end)| *at < end)
            );
            let expected = SimulationInvocationV1 {
                global: invocation.global,
                workgroup: std::array::from_fn(|axis| {
                    invocation.global[axis] / u64::from(size[axis])
                }),
                local: std::array::from_fn(|axis| {
                    u32::try_from(invocation.global[axis] % u64::from(size[axis])).unwrap()
                }),
                workgroup_size: size,
                workgroup_count: std::array::from_fn(|axis| {
                    extent[axis].div_ceil(u64::from(size[axis]))
                }),
                launch_extent: extent,
            };
            assert_eq!(*invocation, expected);
        }
        let SimulationConflictAssessmentV1::ConflictsObserved {
            conflicting_bytes,
            first,
        } = execution.conflict_assessment()
        else {
            panic!(
                "expected exact ordinary conflict: {:?}",
                execution.conflict_assessment()
            );
        };
        assert_eq!(*conflicting_bytes, 4);
        assert_eq!(first.earlier, writes.0[0]);
        assert_eq!(first.later, writes.0[1]);
        assert_eq!(first.offset, 0);
        let SimulationRaceAssessmentV1::RacesObserved {
            racing_bytes,
            first: race,
            ..
        } = execution.race_assessment()
        else {
            panic!(
                "expected exact ordinary race: {:?}",
                execution.race_assessment()
            );
        };
        assert_eq!(*racing_bytes, 4);
        assert_eq!(&race.conflict, first);
        assert!(!race.earlier_atomic);
        assert!(!race.later_atomic);
    }
}

#[test]
fn canonical_conflict_reports_full_invocations_across_workgroups_and_tails() {
    check_public_identity(false);
}

#[test]
fn child_calls_preserve_the_same_full_conflict_identity() {
    check_public_identity(true);
}
